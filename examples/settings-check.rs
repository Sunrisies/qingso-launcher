//! Validate native tray callbacks, hotkey replacement, errors and persistence.
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
    time::Duration,
};
#[path = "../src/config.rs"]
mod config;
#[path = "../src/hotkeys.rs"]
mod hotkeys;
#[path = "../src/launcher.rs"]
mod launcher;
#[path = "../src/search.rs"]
mod search;
#[path = "../src/search_tags.rs"]
mod search_tags;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/window_placement.rs"]
mod window_placement;
slint::include_modules!();

#[cfg(windows)]
fn native_tray_exists() -> bool {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowExW(
            parent: *mut c_void,
            after: *mut c_void,
            class: *const u16,
            title: *const u16,
        ) -> *mut c_void;
        fn GetWindowThreadProcessId(window: *mut c_void, process: *mut u32) -> u32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    let class: Vec<u16> = "SlintSystemTrayWindow"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut after = std::ptr::null_mut();
    loop {
        // Tray callbacks use a message-only HWND, excluded by EnumThreadWindows.
        let window = unsafe {
            FindWindowExW(
                -3isize as *mut c_void,
                after,
                class.as_ptr(),
                std::ptr::null(),
            )
        };
        if window.is_null() {
            return false;
        }
        if unsafe { GetWindowThreadProcessId(window, std::ptr::null_mut()) == GetCurrentThreadId() }
        {
            return true;
        }
        after = window;
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let directory =
        std::env::temp_dir().join(format!("flow-settings-check-{}", std::process::id()));
    // Set the isolated test profile before initializing any background threads.
    unsafe {
        std::env::set_var("QINGSO_CONFIG_DIR", &directory);
    }
    launcher::configure_backend()?;
    let ui = AppWindow::new()?;
    let registration = Rc::new(RefCell::new(hotkeys::register(&ui)?));
    let original = registration.borrow().hotkey;
    let preferences = settings::create(&ui, registration.clone())?;
    let tray = LauncherTray::new()?;
    settings::bind_tray(&tray, &ui, &preferences);
    let competitor = global_hotkey::GlobalHotKeyManager::new()?;
    let occupied: global_hotkey::hotkey::HotKey = "Ctrl+Alt+F10".parse()?;
    competitor.register(occupied)?;
    let stage = Cell::new(0);
    let weak_ui = ui.as_weak();
    let weak_settings = preferences.as_weak();
    let weak_tray = tray.as_weak();
    let test_registration = registration.clone();
    let timer = slint::Timer::default();
    tray.invoke_open_settings();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(350), move || {
        let ui = weak_ui.upgrade().unwrap();
        let preferences = weak_settings.upgrade().unwrap();
        let tray = weak_tray.upgrade().unwrap();
        match stage.get() {
            0 => {
                #[cfg(windows)]
                assert!(native_tray_exists(), "Native system tray icon must be created");
                assert!(preferences.window().is_visible());
                assert!(preferences.window().with_winit_window(|native| native.has_focus()).unwrap());
                preferences.invoke_save_shortcut("invalid-shortcut".into());
                assert!(preferences.get_status_error());
                assert_eq!(test_registration.borrow().hotkey, original);
                preferences.invoke_save_shortcut("Ctrl+Alt+F10".into());
                assert!(preferences.get_status_error());
                assert_eq!(test_registration.borrow().hotkey, original);
                assert_eq!(config::load().unwrap(), None);
                let result = test_registration.borrow_mut().change("Ctrl+Alt+F11", |_| Err(std::io::Error::other("simulated disk failure")));
                assert!(result.is_err());
                assert_eq!(test_registration.borrow().hotkey, original);
                println!("PASS: invalid format, occupied shortcut and save failure preserve the original");
                preferences.invoke_save_shortcut("Ctrl+Alt+F12".into());
                assert!(!preferences.get_status_error());
                assert_eq!(config::load().unwrap().as_deref(), Some("Ctrl+Alt+F12"));
                assert_eq!(ui.get_shortcut_text(), "Ctrl+Alt+F12");
                preferences.invoke_save_shortcut("Ctrl+Alt+F12".into());
                assert!(!preferences.get_status_error(), "Saving an unchanged shortcut must succeed");
                println!("PASS: successful shortcut replacement updates the UI and saves preferences");
                tray.invoke_open_search();
            }
            1 => {
                assert!(ui.window().is_visible());
                assert!(preferences.window().is_visible(), "Settings are independent of the search popup");
                preferences.hide().unwrap();
                tray.invoke_open_settings();
            }
            2 => {
                assert!(preferences.window().is_visible());
                assert_eq!(preferences.get_current_hotkey(), "Ctrl+Alt+F12");
                println!("PASS: tray opens search/settings; settings can reopen with saved values");
                tray.invoke_quit_app();
            }
            _ => unreachable!(),
        }
        stage.set(stage.get() + 1);
    });
    slint::run_event_loop_until_quit()?;
    preferences.hide()?;
    ui.hide()?;
    preferences.on_save_shortcut(|_| {});
    timer.start(slint::TimerMode::SingleShot, Duration::from_secs(10), || {});
    drop(preferences);
    drop(timer);
    drop(registration);
    let restored = hotkeys::register(&ui)?;
    assert_eq!(restored.shortcut, "Ctrl+Alt+F12");
    println!(
        "PASS: restarting the hotkey manager restores saved settings; tray quit ends the loop"
    );
    drop(restored);
    competitor.unregister(occupied)?;
    std::fs::remove_file(directory.join("hotkey.txt"))?;
    std::fs::remove_dir(directory)?;
    Ok(())
}
