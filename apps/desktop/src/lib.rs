//! Tauri shell. Wires adapters to `app-core`, exposes commands, owns the
//! window and tray. No game logic lives here.

mod commands;
mod overlay_host;
mod paths;
mod sink;
mod tray;

use app_core::{Core, SeedBuilds, SettingsStore};
use domain::builds::BuildCatalog;
use domain::ports::BuildSource;
use lcu_client::{run_connector, ConnectorConfig, LiveClient, LIVE_CLIENT_URL};
use opgg::{OpggClient, OPGG_API_URL};
use overlay_host::OverlayHost;
use static_data::{DataDragon, DATA_DRAGON_URL};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tokio_util::sync::CancellationToken;

/// Shipped with the binary; used when OP.GG cannot be reached.
const SEED_BUILDS: &str = include_str!("../../../data/builds/seed.json");
/// OP.GG region whose statistics the builds come from. `global` combines
/// every server, about ten times the games of a single region.
const OPGG_REGION: &str = opgg::GLOBAL_REGION;
const MINIMIZED_FLAG: &str = "--minimized";
const DATA_DRAGON_RETRY: Duration = Duration::from_secs(60);
/// The app lives in the tray for days; a new patch's items and runes must
/// not read as "removed this patch" until it restarts.
const DATA_DRAGON_REFRESH: Duration = Duration::from_secs(3 * 60 * 60);
/// How often the running game is asked for gold and items.
const LIVE_GAME_POLL: Duration = Duration::from_secs(1);

pub struct AppState {
    pub core: Arc<Core>,
    pub overlay: Arc<OverlayHost>,
    pub log_dir: std::path::PathBuf,
    cancel: CancellationToken,
    _log_guard: tracing_appender::non_blocking::WorkerGuard,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("log_dir", &self.log_dir)
            .finish_non_exhaustive()
    }
}

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_FLAG]),
        ))
        .setup(|app| {
            let dirs = paths::AppDirs::resolve(app.handle())?;
            let log_guard = paths::init_logging(&dirs.logs)?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");

            let seed = BuildCatalog::from_json(SEED_BUILDS)?;
            let mut sources: Vec<Arc<dyn BuildSource>> = Vec::new();
            match OpggClient::new(OPGG_API_URL, OPGG_REGION) {
                Ok(opgg) => sources.push(Arc::new(opgg)),
                Err(e) => tracing::error!(error = %e, "cannot create OP.GG client"),
            }
            sources.push(Arc::new(SeedBuilds(seed)));
            let store = SettingsStore::new(dirs.data.join("settings.json"));
            let overlay = Arc::new(OverlayHost::default());
            let core = Core::new(
                sources,
                store,
                Arc::new(sink::TauriSink::new(app.handle().clone(), overlay.clone())),
            );
            // Weak: the core owns the sink that owns the host.
            let weak_core = Arc::downgrade(&core);
            overlay.on_command(Arc::new(move |command| {
                if let Some(core) = weak_core.upgrade() {
                    core.overlay_command(command);
                }
            }));
            if core.settings().overlay_enabled {
                overlay.enable(&core.overlay());
            }
            let cancel = CancellationToken::new();

            let riot_root_pem = paths::riot_root_pem(app.handle());
            spawn_static_data(core.clone(), dirs.cache.join("ddragon"), cancel.clone());
            spawn_live_game(core.clone(), riot_root_pem.as_deref(), cancel.clone());
            spawn_lcu(core.clone(), riot_root_pem, cancel.clone());

            let start_hidden =
                core.settings().start_minimized || std::env::args().any(|a| a == MINIMIZED_FLAG);
            app.manage(AppState {
                core,
                overlay,
                log_dir: dirs.logs,
                cancel,
                _log_guard: log_guard,
            });

            tray::create(app.handle())?;
            if !start_hidden {
                tray::show_main_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(e) = window.hide() {
                    tracing::warn!(error = %e, "could not hide window");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_last_import,
            commands::get_settings,
            commands::save_settings,
            commands::reimport,
            commands::apply_rune_page,
            commands::get_rune_catalog,
            commands::open_logs_folder,
            commands::app_version,
        ])
        .build(tauri::generate_context!());

    match result {
        Ok(app) => app.run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = handle.try_state::<AppState>() {
                    state.cancel.cancel();
                    state.overlay.disable();
                }
            }
        }),
        Err(e) => {
            // No window yet, so stderr is the only place to report this.
            eprintln!("LoL Overlay failed to start: {e}");
            std::process::exit(1);
        }
    }
}

fn spawn_static_data(core: Arc<Core>, cache_dir: std::path::PathBuf, cancel: CancellationToken) {
    tauri::async_runtime::spawn(async move {
        let dd = match DataDragon::new(DATA_DRAGON_URL, cache_dir, "en_US") {
            Ok(dd) => dd,
            Err(e) => return tracing::error!(error = %e, "cannot create Data Dragon client"),
        };
        loop {
            let wait = match dd.load().await {
                Ok(catalog) => {
                    if core.status().patch.as_deref() != Some(catalog.version.as_str()) {
                        tracing::info!(version = %catalog.version, "static data ready");
                        core.set_catalog(Arc::new(catalog));
                    }
                    DATA_DRAGON_REFRESH
                }
                Err(e) => {
                    tracing::warn!(error = %e, "static data unavailable; retrying");
                    DATA_DRAGON_RETRY
                }
            };
            tokio::select! {
                () = cancel.cancelled() => return,
                () = tokio::time::sleep(wait) => {}
            }
        }
    });
}

/// While a game runs, reads gold and items once a second for the item column.
fn spawn_live_game(core: Arc<Core>, riot_root_pem: Option<&[u8]>, cancel: CancellationToken) {
    let tls = riot_root_pem.and_then(|pem| match lcu_client::tls::riot_client_config(pem) {
        Ok(config) => Some(config),
        Err(e) => {
            tracing::warn!(error = %e, "no TLS for the live game data");
            None
        }
    });
    let client = match LiveClient::new(LIVE_CLIENT_URL, tls) {
        Ok(c) => c,
        Err(e) => return tracing::warn!(error = %e, "live game data unavailable"),
    };
    tauri::async_runtime::spawn(async move {
        use domain::ports::LiveGamePort;
        loop {
            tokio::select! {
                () = cancel.cancelled() => return,
                () = tokio::time::sleep(LIVE_GAME_POLL) => {}
            }
            if !core.in_game() {
                continue;
            }
            match client.snapshot().await {
                Ok(snapshot) => core.live_tick(snapshot),
                Err(e) => tracing::debug!(error = %e, "live game data not read"),
            }
        }
    });
}

fn spawn_lcu(core: Arc<Core>, riot_root_pem: Option<Vec<u8>>, cancel: CancellationToken) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    tauri::async_runtime::spawn(run_connector(
        ConnectorConfig::new(riot_root_pem),
        tx,
        cancel,
    ));
    tauri::async_runtime::spawn(async move {
        // One consumer, so events are handled strictly in order.
        while let Some(event) = rx.recv().await {
            core.handle(event).await;
        }
    });
}
