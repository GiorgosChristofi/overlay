use crate::overlay_host::OverlayHost;
use app_core::EventSink;
use ipc_types::{ImportReportDto, OverlayMessage, StatusDto, EVENT_IMPORT, EVENT_STATUS};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Forwards core events to every webview as Tauri events, and overlay views
/// to the overlay process.
pub struct TauriSink {
    app: AppHandle,
    overlay: Arc<OverlayHost>,
}

impl TauriSink {
    pub fn new(app: AppHandle, overlay: Arc<OverlayHost>) -> Self {
        Self { app, overlay }
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

    fn overlay(&self, message: &OverlayMessage) {
        self.overlay.send(message);
    }
}
