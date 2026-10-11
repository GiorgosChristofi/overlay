//! Runs the in-game item column as a child process (`<this exe> --overlay`):
//! views go to it as JSON lines on its stdin, the player's choices come back
//! as JSON lines on its stdout. The child exits when its stdin closes, so it
//! never outlives the app.

use ipc_types::{OverlayCommand, OverlayMessage};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Called with each command the player gives in the overlay.
pub type CommandHandler = Arc<dyn Fn(OverlayCommand) + Send + Sync>;

#[derive(Debug, Default)]
struct Running {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
}

#[derive(Default)]
pub struct OverlayHost {
    /// Whether the player wants the overlay (Settings).
    enabled: Mutex<bool>,
    running: Mutex<Running>,
    on_command: Mutex<Option<CommandHandler>>,
}

impl std::fmt::Debug for OverlayHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayHost")
            .field("running", &self.running)
            .finish_non_exhaustive()
    }
}

impl OverlayHost {
    /// Where the player's choices in the overlay go. Set before enabling.
    pub fn on_command(&self, handler: CommandHandler) {
        *self
            .on_command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handler);
    }

    fn command_handler(&self) -> Option<CommandHandler> {
        self.on_command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn running(&self) -> std::sync::MutexGuard<'_, Running> {
        self.running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn is_enabled(&self) -> bool {
        *self
            .enabled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn set_enabled(&self, on: bool) {
        *self
            .enabled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = on;
    }

    /// Turns the overlay on and shows `current` in it.
    pub fn enable(&self, current: &OverlayMessage) {
        self.set_enabled(true);
        self.send(current);
    }

    /// Turns the overlay off and closes it.
    pub fn disable(&self) {
        self.set_enabled(false);
        let mut running = self.running();
        running.stdin = None; // EOF: the overlay closes itself
        if let Some(mut child) = running.child.take() {
            if let Err(e) = child.kill() {
                tracing::debug!(error = %e, "overlay already gone");
            }
            let _ = child.wait();
        }
    }

    /// Sends a view, starting (or restarting) the overlay if it is enabled
    /// but not running.
    pub fn send(&self, message: &OverlayMessage) {
        if !self.is_enabled() {
            return;
        }
        let line = match serde_json::to_string(message) {
            Ok(line) => line,
            Err(e) => return tracing::error!(error = %e, "cannot encode overlay message"),
        };
        let mut running = self.running();
        let alive = running
            .child
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)));
        if !alive {
            if running.child.is_some() {
                tracing::warn!("overlay exited; restarting it");
            }
            *running = match spawn(self.command_handler()) {
                Ok(r) => r,
                Err(e) => return tracing::error!(error = %e, "cannot start the overlay"),
            };
        }
        if let Some(stdin) = running.stdin.as_mut() {
            if let Err(e) = writeln!(stdin, "{line}").and_then(|()| stdin.flush()) {
                tracing::warn!(error = %e, "could not reach the overlay");
            }
        }
    }
}

fn spawn(on_command: Option<CommandHandler>) -> std::io::Result<Running> {
    let exe = std::env::current_exe()?;
    let mut child = Command::new(exe)
        .arg(overlay::OVERLAY_FLAG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                match serde_json::from_str::<OverlayCommand>(&line) {
                    Ok(command) => {
                        if let Some(handle) = &on_command {
                            handle(command);
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, %line, "unreadable overlay command"),
                }
            }
        });
    }
    // The overlay's own messages end up in our log file.
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                tracing::warn!(target: "overlay", "{line}");
            }
        });
    }
    tracing::info!(pid = child.id(), "overlay started");
    Ok(Running {
        stdin: child.stdin.take(),
        child: Some(child),
    })
}
