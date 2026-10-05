use app_core::EventSink;
use ipc_types::{ImportReportDto, StatusDto, EVENT_IMPORT, EVENT_STATUS};
use tauri::{AppHandle, Emitter};

/// Forwards core events to every webview as Tauri events.
pub struct TauriSink {
    app: AppHandle,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl std::fmt::Debug for TauriSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TauriSink")
    }
}

impl EventSink for TauriSink {
    fn status(&self, status: &StatusDto) {
        if let Err(e) = self.app.emit(EVENT_STATUS, status) {
            tracing::warn!(error = %e, "could not emit status");
        }
    }

    fn import(&self, report: &ImportReportDto) {
        if let Err(e) = self.app.emit(EVENT_IMPORT, report) {
            tracing::warn!(error = %e, "could not emit import report");
        }
    }
}
