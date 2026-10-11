// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The in-game item column runs as a second process of this executable.
    if std::env::args().any(|a| a == overlay::OVERLAY_FLAG) {
        if let Err(e) = overlay::run() {
            eprintln!("overlay failed: {e}");
            std::process::exit(1);
        }
        return;
    }
    desktop_lib::run();
}
