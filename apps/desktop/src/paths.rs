use std::path::PathBuf;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Env var to point at a `riotgames.pem` during development.
const RIOT_PEM_ENV: &str = "LOL_OVERLAY_RIOT_PEM";

pub struct AppDirs {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
}

impl AppDirs {
    pub fn resolve(app: &AppHandle) -> tauri::Result<Self> {
        let p = app.path();
        Ok(Self {
            data: p.app_data_dir()?,
            cache: p.app_cache_dir()?,
            logs: p.app_log_dir()?,
        })
    }
}

/// Logs to a daily rotating file and stderr. `RUST_LOG` overrides the level.
pub fn init_logging(dir: &std::path::Path) -> std::io::Result<WorkerGuard> {
    std::fs::create_dir_all(dir)?;
    let (writer, guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::daily(dir, "lol-overlay.log"));
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let result = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(writer)
        .try_init();
    if let Err(e) = result {
        eprintln!("logging already initialised: {e}");
    }
    Ok(guard)
}

/// Env var naming a lockfile to connect to instead of discovering League,
/// for `mock-league`. Debug builds only, like [`RIOT_PEM_ENV`].
const MOCK_LOCKFILE_ENV: &str = "LOL_OVERLAY_LCU_LOCKFILE";

pub fn mock_lockfile() -> Option<PathBuf> {
    cfg!(debug_assertions)
        .then(|| std::env::var_os(MOCK_LOCKFILE_ENV))
        .flatten()
        .map(PathBuf::from)
}

/// Riot root certificate: env override first (debug builds only), then the
/// bundled resource.
pub fn riot_root_pem(app: &AppHandle) -> Option<Vec<u8>> {
    // In a release build the override would let anyone who can set the
    // user's environment choose which certificate the client is trusted by.
    let candidates = cfg!(debug_assertions)
        .then(|| std::env::var_os(RIOT_PEM_ENV))
        .flatten()
        .map(PathBuf::from)
        .into_iter()
        .chain(
            app.path()
                .resolve("resources/riotgames.pem", BaseDirectory::Resource)
                .ok(),
        );
    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            tracing::info!(path = %path.display(), "loaded Riot root certificate");
            return Some(bytes);
        }
    }
    tracing::error!("riotgames.pem not found; run scripts/fetch-riot-cert first");
    None
}
