use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::time::Duration;

/// Wait for the native window and its DPI-adjusted size before positioning.
pub fn center_when_ready<T: ComponentHandle + 'static>(weak: slint::Weak<T>) {
    slint::Timer::single_shot(Duration::from_millis(16), move || {
        if let Some(ui) = weak.upgrade() {
            if !center_window(ui.window()) {
                center_when_ready(weak);
            }
        }
    });
}

fn center_window(window: &slint::Window) -> bool {
    let position = window
        .with_winit_window(|native| {
            let monitor = native
                .current_monitor()
                .or_else(|| native.primary_monitor())?;
            let origin = monitor.position();
            let screen = monitor.size();
            let size = native.outer_size();
            if size.width == 0 || size.height == 0 {
                return None;
            }
            // Signed arithmetic preserves monitor origins to the left/above primary.
            let x = origin.x + (screen.width as i32 - size.width as i32) / 2;
            let y = origin.y + (screen.height as i32 - size.height as i32) / 2;
            Some(slint::PhysicalPosition::new(x, y))
        })
        .flatten();
    if let Some(position) = position {
        window.set_position(position);
        true
    } else {
        false
    }
}
