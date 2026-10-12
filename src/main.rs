// A background launcher must not create a separate Windows console window.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::error::Error;
use std::{cell::RefCell, rc::Rc};

mod autostart;
mod browser_bookmarks;
mod config;
mod hotkeys;
mod instance;
mod launcher;
mod recommendations;
mod search;
mod search_tags;
mod settings;
mod window_placement;

slint::include_modules!();

fn main() {
    // Old Chrome extension installations must not launch a normal search window.
    if std::env::args()
        .skip(1)
        .any(|arg| arg.starts_with("chrome-extension://"))
    {
        std::process::exit(1);
    }
    if let Err(error) = run() {
        let message = format!(
            "启动失败：{error}\n\n如果快捷键被占用，请关闭其他启动器，或通过 QINGSO_HOTKEY 指定其他快捷键。"
        );
        eprintln!("{message}");
        if let Ok(dialog) = StartupErrorWindow::new() {
            dialog.set_message(message.into());
            let _ = dialog.run();
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let login_start = std::env::args().skip(1).any(|arg| arg == "--autostart");
    let Some(instance) = instance::acquire(!login_start)? else {
        return Ok(());
    };
    launcher::configure_backend()?;
    let ui = AppWindow::new()?;

    ui.set_is_macos(cfg!(target_os = "macos"));
    launcher::bind_window(&ui);
    let registration = Rc::new(RefCell::new(hotkeys::register(&ui)?));
    eprintln!("Launcher shortcut: {}", registration.borrow().shortcut);
    let preferences = settings::create(&ui, registration.clone())?;
    browser_bookmarks::bind(&ui, &preferences);
    let tray = LauncherTray::new()?;
    settings::bind_tray(&tray, &ui, &preferences);
    instance.listen(ui.as_weak())?;
    if !login_start {
        launcher::show(&ui)?;
    }
    slint::run_event_loop_until_quit()?;

    Ok(())
}
