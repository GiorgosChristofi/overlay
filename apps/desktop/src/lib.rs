//! Tauri shell. Wires adapters to `app-core`, exposes commands, owns the
//! window and tray. No game logic lives here.

mod commands;
mod paths;
mod sink;
mod tray;

use app_core::{Core, SeedBuilds, SettingsStore};
use domain::builds::BuildCatalog;
use domain::ports::BuildSource;
use lcu_client::{run_connector, ConnectorConfig};
use opgg::{OpggClient, OPGG_API_URL};
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

pub struct AppState {
    pub core: Arc<Core>,
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
            let core = Core::new(
                sources,
                store,
                Arc::new(sink::TauriSink::new(app.handle().clone())),
            );
            let cancel = CancellationToken::new();

            spawn_static_data(core.clone(), dirs.cache.join("ddragon"), cancel.clone());
            spawn_lcu(
                core.clone(),
                paths::riot_root_pem(app.handle()),
                cancel.clone(),
            );

            let start_hidden =
                core.settings().start_minimized || std::env::args().any(|a| a == MINIMIZED_FLAG);
            app.manage(AppState {
                core,
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
            match dd.load().await {
                Ok(catalog) => {
                    tracing::info!(version = %catalog.version, "static data ready");
                    core.set_catalog(Arc::new(catalog));
                    return;
                }
                Err(e) => tracing::warn!(error = %e, "static data unavailable; retrying"),
            }
            tokio::select! {
                () = cancel.cancelled() => return,
                () = tokio::time::sleep(DATA_DRAGON_RETRY) => {}
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
