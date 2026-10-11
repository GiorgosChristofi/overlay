//! Messages from the desktop app: one JSON [`OverlayMessage`] per stdin line.
//! End of input means the app is gone, so the overlay closes too.

use crate::state::Input;
use ipc_types::OverlayMessage;
use std::io::BufRead;
use std::sync::mpsc::Sender;

/// `None` for blank or unreadable lines; a bad line never stops the overlay.
pub fn parse_line(line: &str) -> Option<OverlayMessage> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    match serde_json::from_str(line) {
        Ok(message) => Some(message),
        Err(e) => {
            eprintln!("overlay: ignoring unreadable message: {e}");
            None
        }
    }
}

pub fn spawn(tx: Sender<Input>, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if let Some(message) = parse_line(&line) {
                if tx.send(Input::Message(message)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        }
        // Ignored if the window is already gone.
        let _ = tx.send(Input::Quit);
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_messages_and_skips_noise() {
        assert_eq!(
            parse_line(r#"{"type":"clear"}"#),
            Some(OverlayMessage::Clear)
        );
        assert_eq!(parse_line("   "), None);
        assert_eq!(parse_line("not json"), None);
    }
}
