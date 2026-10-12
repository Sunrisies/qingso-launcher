// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

mod window_placement;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    let ui = AppWindow::new()?;

    ui.set_is_macos(cfg!(target_os = "macos"));
    let weak = ui.as_weak();
    ui.on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.hide();
        }
    });

    // Wait for the native window rather than guessing when initialization ends.
    window_placement::center_when_ready(ui.as_weak());

    ui.run()?;

    Ok(())
}
