//! The in-game item column (Phase 2). Runs as its own process,
//! `desktop --overlay`, so egui's window loop never shares a process with
//! Tauri's. The desktop app sends it views on stdin; it watches the P and
//! Esc keys itself.

mod app;
mod commands;
mod input;
mod minimap;
mod minimap_view;
mod state;
mod stdin;

pub use input::ANY_WINDOW_ENV;

use eframe::egui::ViewportBuilder;

/// Command-line flag that starts the overlay instead of the main app.
pub const OVERLAY_FLAG: &str = "--overlay";
/// Set to start with the column open, for working on it without a game.
pub const OPEN_ENV: &str = "LOL_OVERLAY_OPEN";
/// Also names the folder where egui remembers the window position.
const APP_NAME: &str = "LoL Overlay items";
/// eframe's storage key for the remembered window position.
const SAVED_WINDOW_KEY: &str = "window";

pub fn run() -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let open = std::env::var_os(OPEN_ENV).is_some();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([app::WIDTH, 80.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            // Starting must not take focus away from the game.
            .with_active(false)
            .with_mouse_passthrough(!open),
        // Remembers where the player dragged it.
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            // Nothing should slide or fade: the player is mid-game.
            cc.egui_ctx.all_styles_mut(|s| s.animation_time = 0.0);
            input::spawn(tx.clone(), cc.egui_ctx.clone());
            stdin::spawn(tx, cc.egui_ctx.clone());
            let remembered = cc
                .storage
                .and_then(|s| s.get_string(SAVED_WINDOW_KEY))
                .is_some();
            let minimap = cc
                .storage
                .and_then(|s| s.get_string(minimap_view::STORAGE_KEY))
                .and_then(|raw| minimap::Placement::parse(&raw));
            Ok(Box::new(app::OverlayApp::new(
                rx,
                open,
                !remembered,
                minimap,
            )))
        }),
    )
    .map_err(|e| e.to_string())
}
