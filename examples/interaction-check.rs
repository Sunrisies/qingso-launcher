//! Native-window smoke test for keyboard selection and background lifecycle.
use slint::{
    ComponentHandle,
    platform::{Key, WindowEvent},
    winit_030::WinitWindowAccessor,
};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    time::Duration,
};
#[allow(dead_code)]
#[path = "../src/config.rs"]
mod config;
#[path = "../src/hotkeys.rs"]
#[allow(dead_code)]
mod hotkeys;
#[path = "../src/launcher.rs"]
mod launcher;
#[path = "../src/window_placement.rs"]
mod window_placement;
slint::include_modules!();

#[cfg(windows)]
fn post_hotkey_message(id: u32) {
    use std::ffi::c_void;
    type Hwnd = *mut c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumThreadWindows(
            thread: u32,
            callback: unsafe extern "system" fn(Hwnd, isize) -> i32,
            data: isize,
        ) -> i32;
        fn GetClassNameW(window: Hwnd, name: *mut u16, size: i32) -> i32;
        fn PostMessageW(window: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    unsafe extern "system" fn find_manager(window: Hwnd, data: isize) -> i32 {
        let mut name = [0u16; 128];
        let length = unsafe { GetClassNameW(window, name.as_mut_ptr(), name.len() as i32) };
        if String::from_utf16_lossy(&name[..length as usize]) == "global_hotkey_app" {
            unsafe {
                *(data as *mut Hwnd) = window;
            }
            return 0;
        }
        1
    }
    let mut window: Hwnd = std::ptr::null_mut();
    // Inject a message only into this test's own manager window on its main
    // thread, verifying the OS-message -> hotkey-handler -> Slint event path.
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            find_manager,
            &mut window as *mut Hwnd as isize,
        );
    }
    assert!(!window.is_null(), "Hotkey manager window not found");
    assert_ne!(
        unsafe { PostMessageW(window, 0x0312, id as usize, (0x20 << 16) | 3) },
        0
    );
}

fn key(ui: &AppWindow, key: Key) {
    let text: slint::SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

#[cfg(windows)]
fn assert_cursor_screen_centered(ui: &AppWindow) {
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
    assert_ne!(unsafe { GetCursorPos(&mut point) }, 0);
    ui.window().with_winit_window(|native| {
        let monitors: Vec<_> = native.available_monitors().collect();
        let target = monitors
            .iter()
            .find(|m| {
                let p = m.position();
                let s = m.size();
                point.x >= p.x
                    && point.y >= p.y
                    && point.x < p.x + s.width as i32
                    && point.y < p.y + s.height as i32
            })
            .unwrap();
        let origin = target.position();
        let screen = target.size();
        let size = native.outer_size();
        let position = native.outer_position().unwrap();
        assert!((position.x - origin.x - (screen.width as i32 - size.width as i32) / 2).abs() <= 2);
        assert!(
            (position.y - origin.y - (screen.height as i32 - size.height as i32) / 2).abs() <= 2
        );
        println!(
            "PASS: cursor-screen centering among {} monitor(s), origin {:?}, scale {}",
            monitors.len(),
            origin,
            target.scale_factor()
        );
    });
}

fn main() -> Result<(), Box<dyn Error>> {
    launcher::configure_backend()?;
    // Exercise real OS registration. Registration errors must be surfaced.
    let ui = AppWindow::new()?;
    ui.set_recommendations(
        std::rc::Rc::new(slint::VecModel::from(
            (0..6)
                .map(|index| RecommendationEntry {
                    name: format!("Test {index}").into(),
                    kind: 2,
                    target: "https://example.com".into(),
                })
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
    ui.set_is_macos(cfg!(target_os = "macos"));
    let registration = hotkeys::register(&ui)?;
    println!("Registered shortcut: {}", registration.shortcut);
    let hotkey_id = registration.hotkey.id();
    launcher::bind_window(&ui);
    launcher::show(&ui)?;
    let stage = Cell::new(0);
    let other_window = RefCell::new(None::<AppWindow>);
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(350),
        move || {
            let ui = weak.upgrade().unwrap();
            match stage.get() {
                0 => {
                    assert!(ui.window().is_visible());
                    #[cfg(windows)]
                    assert_cursor_screen_centered(&ui);
                    ui.invoke_focus_search();
                    key(&ui, Key::UpArrow);
                    assert_eq!(ui.get_selected_index(), 0);
                    for _ in 0..6 {
                        key(&ui, Key::DownArrow);
                    }
                    assert_eq!(ui.get_selected_index(), 5);
                    assert_eq!(ui.get_results_scroll_y(), -162.0);
                    for _ in 0..6 {
                        key(&ui, Key::UpArrow);
                    }
                    assert_eq!(ui.get_selected_index(), 0);
                    assert_eq!(ui.get_results_scroll_y(), 0.0);
                    ui.set_query_text("preserved search".into());
                    key(&ui, Key::Escape);
                    assert!(!ui.window().is_visible());
                    println!("PASS: keyboard boundaries, automatic scrolling, Escape hides");
                }
                1 => {
                    assert!(
                        !ui.window().is_visible(),
                        "Hidden event loop must remain alive"
                    );
                    #[cfg(windows)]
                    post_hotkey_message(hotkey_id);
                    #[cfg(not(windows))]
                    launcher::toggle(&ui).unwrap();
                }
                2 => {
                    assert!(ui.window().is_visible());
                    assert_eq!(ui.get_query_text(), "preserved search");
                    launcher::toggle(&ui).unwrap();
                    assert!(!ui.window().is_visible());
                    launcher::toggle(&ui).unwrap();
                    println!("PASS: repeated toggle and search persistence");
                }
                3 => {
                    assert!(ui.window().is_visible());
                    let other = AppWindow::new().unwrap();
                    other.show().unwrap();
                    window_placement::center_when_ready(other.as_weak(), |other| {
                        other
                            .window()
                            .with_winit_window(|native| native.focus_window());
                    });
                    *other_window.borrow_mut() = Some(other);
                }
                4 => {
                    assert!(!ui.window().is_visible());
                    if let Some(other) = other_window.borrow_mut().take() {
                        other.hide().unwrap();
                    }
                    println!("PASS: real native focus loss hides the launcher");
                    #[cfg(windows)]
                    post_hotkey_message(hotkey_id);
                    #[cfg(not(windows))]
                    launcher::toggle(&ui).unwrap();
                }
                5 => {
                    assert!(ui.window().is_visible());
                    println!(
                        "PASS: window can be shown again after focus loss; global hotkey registered"
                    );
                    slint::quit_event_loop().unwrap();
                }
                _ => unreachable!(),
            }
            stage.set(stage.get() + 1);
        },
    );
    slint::run_event_loop_until_quit()?;
    Ok(())
}
