use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::time::Duration;

/// Wait for the native window and its DPI-adjusted size before positioning.
pub fn center_when_ready<T: ComponentHandle + 'static>(
    weak: slint::Weak<T>,
    ready: impl FnOnce(&T) + 'static,
) {
    slint::Timer::single_shot(Duration::from_millis(16), move || {
        if let Some(ui) = weak.upgrade() {
            if !ui.window().is_visible() {
                return;
            }
            if !center_window(ui.window()) {
                center_when_ready(weak, ready);
            } else {
                // Moving across displays may change the native pixel size on the
                // next event-loop turn. Recenter after the DPI event is applied.
                slint::Timer::single_shot(Duration::from_millis(32), move || {
                    if let Some(ui) = weak.upgrade()
                        && ui.window().is_visible()
                    {
                        center_window(ui.window());
                        ready(&ui);
                    }
                });
            }
        }
    });
}

pub fn center_window(window: &slint::Window) -> bool {
    let position = window
        .with_winit_window(|native| {
            let monitor = cursor_monitor(native)
                .or_else(|| native.current_monitor())
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
            #[cfg(not(target_os = "macos"))]
            {
                Some(slint::WindowPosition::Physical(
                    slint::PhysicalPosition::new(x, y),
                ))
            }
            #[cfg(target_os = "macos")]
            {
                let _ = (x, y);
                Some(mac_center_position(native, &monitor))
            }
        })
        .flatten();
    if let Some(position) = position {
        window.set_position(position);
        true
    } else {
        false
    }
}

#[cfg(target_os = "macos")]
fn mac_center_position(
    native: &slint::winit_030::winit::window::Window,
    monitor: &slint::winit_030::winit::monitor::MonitorHandle,
) -> slint::WindowPosition {
    use slint::winit_030::winit::platform::macos::MonitorHandleExtMacOS;
    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    struct Size {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    struct Rect {
        origin: Point,
        size: Size,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDisplayBounds(display: u32) -> Rect;
    }
    let bounds = unsafe { CGDisplayBounds(monitor.native_id()) };
    let size = native.outer_size().to_logical::<f64>(native.scale_factor());
    slint::WindowPosition::Logical(slint::LogicalPosition::new(
        (bounds.origin.x + (bounds.size.width - size.width) / 2.0) as f32,
        (bounds.origin.y + (bounds.size.height - size.height) / 2.0) as f32,
    ))
}

#[cfg(windows)]
fn contains(origin: (i32, i32), size: (u32, u32), point: (i32, i32)) -> bool {
    let (x, y) = (i64::from(point.0), i64::from(point.1));
    x >= i64::from(origin.0)
        && y >= i64::from(origin.1)
        && x < i64::from(origin.0) + i64::from(size.0)
        && y < i64::from(origin.1) + i64::from(size.1)
}

#[cfg(windows)]
fn cursor_monitor(
    native: &slint::winit_030::winit::window::Window,
) -> Option<slint::winit_030::winit::monitor::MonitorHandle> {
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
    }
    let mut point = Point { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return None;
    }
    native.available_monitors().find(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        contains(
            (origin.x, origin.y),
            (size.width, size.height),
            (point.x, point.y),
        )
    })
}

#[cfg(target_os = "macos")]
fn cursor_monitor(
    native: &slint::winit_030::winit::window::Window,
) -> Option<slint::winit_030::winit::monitor::MonitorHandle> {
    use slint::winit_030::winit::platform::macos::MonitorHandleExtMacOS;
    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: *const std::ffi::c_void) -> *const std::ffi::c_void;
        fn CGEventGetLocation(event: *const std::ffi::c_void) -> Point;
        fn CGGetDisplaysWithPoint(
            point: Point,
            count: u32,
            displays: *mut u32,
            found: *mut u32,
        ) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *const std::ffi::c_void);
    }
    let event = unsafe { CGEventCreate(std::ptr::null()) };
    if event.is_null() {
        return None;
    }
    let point = unsafe { CGEventGetLocation(event) };
    unsafe {
        CFRelease(event);
    }
    let mut id = 0;
    let mut found = 0;
    if unsafe { CGGetDisplaysWithPoint(point, 1, &mut id, &mut found) } != 0 || found == 0 {
        return None;
    }
    native.available_monitors().find(|m| m.native_id() == id)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn cursor_monitor(
    _: &slint::winit_030::winit::window::Window,
) -> Option<slint::winit_030::winit::monitor::MonitorHandle> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn negative_coordinates_and_shared_edges() {
        assert!(contains((-2560, -400), (2560, 1440), (-100, 100)));
        assert!(!contains((-2560, -400), (2560, 1440), (0, 100)));
        assert!(contains((0, 0), (1920, 1080), (0, 100)));
        assert!(!contains((0, 0), (1920, 1080), (1920, 1080)));
    }
}
