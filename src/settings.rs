use crate::{
    AppWindow, LauncherTray, SettingsWindow, config, hotkeys::Registration, launcher,
    window_placement,
};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, error::Error, rc::Rc};

pub fn create(
    ui: &AppWindow,
    registration: Rc<RefCell<Registration>>,
) -> Result<SettingsWindow, Box<dyn Error>> {
    let settings = SettingsWindow::new()?;
    settings.set_is_macos(cfg!(target_os = "macos"));
    settings.set_app_version(env!("CARGO_PKG_VERSION").into());
    settings
        .set_platform_text(format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH).into());
    settings.set_config_path(config::path()?.to_string_lossy().into_owned().into());
    settings.set_current_hotkey(registration.borrow().shortcut.clone().into());
    settings.set_edited_hotkey(registration.borrow().shortcut.clone().into());
    let weak_settings = settings.as_weak();
    let weak_search = ui.as_weak();
    settings.on_save_shortcut(move |value| {
        let Some(settings) = weak_settings.upgrade() else {
            return;
        };
        let result = registration
            .borrow_mut()
            .change(value.as_str(), config::save);
        match result {
            Ok(()) => {
                let current = registration.borrow().shortcut.clone();
                if let Some(ui) = weak_search.upgrade() {
                    ui.set_shortcut_text(current.clone().into());
                }
                settings.set_current_hotkey(current.clone().into());
                settings.set_edited_hotkey(current.into());
                settings.set_status_error(false);
                settings.set_status_message("快捷键已保存并生效。".into());
            }
            Err(error) => {
                settings.set_status_error(true);
                settings
                    .set_status_message(format!("修改失败：{error}；原快捷键保持有效。").into());
            }
        }
    });
    crate::search::bind(
        ui,
        &settings,
        Rc::new(RefCell::new(crate::search_tags::Store::load()?)),
    );
    Ok(settings)
}

pub fn bind_tray(tray: &LauncherTray, ui: &AppWindow, settings: &SettingsWindow) {
    let weak = ui.as_weak();
    tray.on_open_search(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = launcher::show(&ui);
        }
    });
    let weak = settings.as_weak();
    tray.on_open_settings(move || {
        if let Some(settings) = weak.upgrade()
            && settings.show().is_ok()
        {
            window_placement::center_when_ready(settings.as_weak(), |settings| {
                settings
                    .window()
                    .with_winit_window(|native| native.focus_window());
            });
        }
    });
    tray.on_quit_app(|| {
        let _ = slint::quit_event_loop();
    });
}
