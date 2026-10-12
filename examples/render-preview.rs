//! Export the actual Slint window to a BMP for visual review.
use slint::ComponentHandle;
use slint::winit_030::WinitWindowAccessor;
use std::{error::Error, io::Write, time::Duration};
#[path = "../src/window_placement.rs"]
mod window_placement;
slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "preview.bmp".into());
    let ui = AppWindow::new()?;
    ui.set_is_macos(cfg!(target_os = "macos"));
    if std::env::args().nth(2).as_deref() == Some("scroll") {
        ui.set_results_scroll_y(-162.0);
    }
    ui.show()?;
    window_placement::center_when_ready(ui.as_weak());
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(500), move || {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let ui = weak.upgrade().ok_or("Window closed")?;
            ui.window().with_winit_window(|native| {
                if let (Some(monitor), Ok(position)) = (native.current_monitor(), native.outer_position()) {
                    let origin = monitor.position();
                    let screen = monitor.size();
                    let size = native.outer_size();
                    let expected_x = origin.x + (screen.width as i32 - size.width as i32) / 2;
                    let expected_y = origin.y + (screen.height as i32 - size.height as i32) / 2;
                    assert!((position.x - expected_x).abs() <= 1, "Window not horizontally centered");
                    assert!((position.y - expected_y).abs() <= 1, "Window not vertically centered");
                    println!("Center verified: window {size:?} at {position:?}, screen {screen:?} at {origin:?}");
                }
            });
            let pixels = ui.window().take_snapshot()?;
            let (width, height) = (pixels.width(), pixels.height());
            let mut file = std::fs::File::create(&path)?;
            file.write_all(b"BM")?;
            file.write_all(&(54 + width * height * 4).to_le_bytes())?;
            file.write_all(&[0; 4])?;
            file.write_all(&54u32.to_le_bytes())?;
            file.write_all(&40u32.to_le_bytes())?;
            file.write_all(&(width as i32).to_le_bytes())?;
            file.write_all(&(-(height as i32)).to_le_bytes())?;
            file.write_all(&1u16.to_le_bytes())?;
            file.write_all(&32u16.to_le_bytes())?;
            file.write_all(&[0; 24])?;
            for pixel in pixels.as_slice() {
                file.write_all(&[pixel.b, pixel.g, pixel.r, pixel.a])?;
            }
            println!("Rendered {width} × {height} to {path}");
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Preview export failed: {error}");
        }
        let _ = slint::quit_event_loop();
    });
    slint::run_event_loop()?;
    Ok(())
}
