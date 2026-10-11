//! The player's choices, sent back to the desktop app as one JSON
//! [`OverlayCommand`] per stdout line.

use ipc_types::OverlayCommand;
use std::io::Write;

pub fn send(command: OverlayCommand) {
    let line = match serde_json::to_string(&command) {
        Ok(line) => line,
        Err(e) => return eprintln!("overlay: cannot encode {command:?}: {e}"),
    };
    let mut out = std::io::stdout().lock();
    if let Err(e) = writeln!(out, "{line}").and_then(|()| out.flush()) {
        eprintln!("overlay: cannot reach the app: {e}");
    }
}
