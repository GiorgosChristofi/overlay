//! Watches the P and Esc keys and which window is focused. Reads key state
//! only (`GetAsyncKeyState`), so the key still reaches the game and the shop
//! opens as usual.

use crate::state::Input;
use std::path::Path;
use std::sync::mpsc::Sender;

/// Set to treat every window as the game, for testing without one.
pub const ANY_WINDOW_ENV: &str = "LOL_OVERLAY_ANY_WINDOW";

/// The in-game process (the client is `LeagueClientUx.exe`).
const GAME_EXE: &str = "League of Legends.exe";

pub fn is_game(process_path: &Path) -> bool {
    process_path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(GAME_EXE))
}

#[cfg(windows)]
pub fn spawn(tx: Sender<Input>, ctx: eframe::egui::Context) {
    use crate::state::KeyEdge;
    use device_query::{DeviceQuery, DeviceState, Keycode};
    use std::time::Duration;

    const POLL: Duration = Duration::from_millis(25);
    /// Focus is checked every this many polls (250 ms).
    const FOCUS_EVERY: u32 = 10;

    let any_window = std::env::var_os(ANY_WINDOW_ENV).is_some();
    let own_pid = u64::from(std::process::id());
    let focused = move || {
        any_window
            || active_win_pos_rs::get_active_window()
                .is_ok_and(|w| w.process_id == own_pid || is_game(&w.process_path))
    };

    std::thread::spawn(move || {
        let keyboard = DeviceState::new();
        let (mut p, mut esc) = (KeyEdge::default(), KeyEdge::default());
        let mut was_focused = None;
        let send = |input: Input| {
            let ok = tx.send(input).is_ok();
            ctx.request_repaint();
            ok
        };
        for tick in 0u32.. {
            if tick % FOCUS_EVERY == 0 {
                let now = focused();
                if was_focused != Some(now) {
                    was_focused = Some(now);
                    if !send(Input::Focus(now)) {
                        return; // the window closed
                    }
                }
            }
            let keys = keyboard.get_keys();
            let p_pressed = p.pressed(keys.contains(&Keycode::P));
            let esc_pressed = esc.pressed(keys.contains(&Keycode::Escape));
            if (p_pressed || esc_pressed) && focused() {
                let input = if p_pressed {
                    Input::TogglePressed
                } else {
                    Input::EscPressed
                };
                if !send(input) {
                    return;
                }
            }
            std::thread::sleep(POLL);
        }
    });
}

/// Key and focus watching are Windows-only, like League; elsewhere the
/// overlay acts as if focused so it can still be looked at.
#[cfg(not(windows))]
pub fn spawn(tx: Sender<Input>, _ctx: eframe::egui::Context) {
    let _ = tx.send(Input::Focus(true));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_game_but_not_the_client() {
        assert!(is_game(Path::new(
            r"C:\Riot Games\League of Legends\Game\League of Legends.exe"
        )));
        assert!(is_game(Path::new("league of legends.EXE")));
        assert!(!is_game(Path::new(
            r"C:\Riot Games\League of Legends\LeagueClientUx.exe"
        )));
        assert!(!is_game(Path::new("")));
    }
}
