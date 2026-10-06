//! Tauri commands. Each one converts input, calls `app-core`, and maps the
//! result to a DTO. Nothing else.

use crate::AppState;
use app_core::CoreError;
use ipc_types::{AppErrorDto, ImportReportDto, RuneCatalogDto, SettingsDto, StatusDto};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

fn to_dto(e: &CoreError) -> AppErrorDto {
    AppErrorDto {
        code: e.code().into(),
        message: e.to_string(),
    }
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> StatusDto {
    state.core.status()
}

#[tauri::command]
pub fn get_last_import(state: State<'_, AppState>) -> Option<ImportReportDto> {
    state.core.last_import()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> SettingsDto {
    state.core.settings().into()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: SettingsDto,
) -> Result<SettingsDto, AppErrorDto> {
    let previous = state.core.settings();
    state
        .core
        .save_settings(settings.clone().into())
        .map_err(|e| to_dto(&e))?;
    if previous.start_with_windows != settings.start_with_windows {
        let autolaunch = app.autolaunch();
        let result = if settings.start_with_windows {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "could not change start-with-Windows");
            return Err(AppErrorDto {
                code: "autostart".into(),
                message: "Settings saved, but Windows refused to change the start-up entry".into(),
            });
        }
    }
    Ok(settings)
}

#[tauri::command]
pub async fn reimport(state: State<'_, AppState>) -> Result<ImportReportDto, AppErrorDto> {
    state.core.reimport().await.map_err(|e| to_dto(&e))
}

#[tauri::command]
pub async fn apply_rune_page(
    state: State<'_, AppState>,
    index: u32,
) -> Result<ImportReportDto, AppErrorDto> {
    let index = usize::try_from(index).unwrap_or(usize::MAX);
    state
        .core
        .apply_rune_page(index)
        .await
        .map_err(|e| to_dto(&e))
}

#[tauri::command]
pub fn get_rune_catalog(state: State<'_, AppState>) -> Option<RuneCatalogDto> {
    state.core.rune_catalog()
}

#[tauri::command]
pub fn open_logs_folder(state: State<'_, AppState>) -> Result<(), AppErrorDto> {
    let fail = |e: std::io::Error| AppErrorDto {
        code: "open_logs".into(),
        message: format!("Could not open the logs folder: {e}"),
    };
    #[cfg(windows)]
    std::process::Command::new("explorer")
        .arg(&state.log_dir)
        .spawn()
        .map_err(fail)?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(&state.log_dir)
        .spawn()
        .map_err(fail)?;
    #[cfg(all(unix, not(target_os = "macos")))]
    std::process::Command::new("xdg-open")
        .arg(&state.log_dir)
        .spawn()
        .map_err(fail)?;
    Ok(())
}

#[tauri::command]
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
