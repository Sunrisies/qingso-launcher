use crate::{AppWindow, config, launcher};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use slint::ComponentHandle;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

/// Keep the manager alive on the same main thread as the native event loop.
pub struct Registration {
    manager: GlobalHotKeyManager,
    pub hotkey: HotKey,
    pub shortcut: String,
    current_id: Arc<AtomicU32>,
    held: Arc<AtomicBool>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = self.manager.unregister(self.hotkey);
    }
}

impl Registration {
    pub fn change(
        &mut self,
        value: &str,
        mut persist: impl FnMut(&str) -> std::io::Result<()>,
    ) -> Result<(), Box<dyn Error>> {
        use global_hotkey::hotkey::Modifiers;
        let value = value.trim();
        let next: HotKey = value
            .parse()
            .map_err(|_| std::io::Error::other("快捷键格式不正确，例如 Ctrl+Alt+Space"))?;
        if !next
            .mods
            .intersects(Modifiers::ALT | Modifiers::CONTROL | Modifiers::SUPER | Modifiers::META)
        {
            return Err(std::io::Error::other("请包含 Ctrl、Alt 或 Super 修饰键").into());
        }
        if next.id() == self.hotkey.id() {
            persist(value)?;
            self.shortcut = value.into();
            return Ok(());
        }
        self.manager
            .register(next)
            .map_err(|_| std::io::Error::other("快捷键已被其他程序占用，或系统无法注册"))?;
        if let Err(error) = persist(value) {
            let _ = self.manager.unregister(next);
            return Err(error.into());
        }
        if let Err(error) = self.manager.unregister(self.hotkey) {
            let _ = self.manager.unregister(next);
            persist(&self.shortcut)?;
            return Err(error.into());
        }
        self.current_id.store(next.id(), Ordering::Relaxed);
        self.held.store(false, Ordering::Relaxed);
        self.hotkey = next;
        self.shortcut = value.into();
        Ok(())
    }
}

pub fn register(ui: &AppWindow) -> Result<Registration, Box<dyn Error>> {
    let manager = GlobalHotKeyManager::new()?;
    let configured = match std::env::var("FLOW_HOTKEY") {
        Ok(value) => Some(value),
        Err(_) => config::load()?,
    };
    let mut shortcut = configured.clone().unwrap_or_else(|| "Alt+Space".into());
    let mut hotkey: HotKey = shortcut.parse()?;
    match manager.register(hotkey) {
        Ok(()) => {}
        Err(global_hotkey::Error::AlreadyRegistered(_)) if configured.is_none() => {
            shortcut = "Ctrl+Alt+Space".into();
            hotkey = shortcut.parse()?;
            manager.register(hotkey)?;
        }
        Err(error) => {
            return Err(std::io::Error::other(format!(
                "Cannot register {shortcut}: {error}. Set FLOW_HOTKEY to another shortcut."
            ))
            .into());
        }
    }
    ui.set_shortcut_text(shortcut.clone().into());
    let weak = ui.as_weak();
    let held = Arc::new(AtomicBool::new(false));
    let current_id = Arc::new(AtomicU32::new(hotkey.id()));
    let handler_id = current_id.clone();
    let handler_held = held.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.id != handler_id.load(Ordering::Relaxed) {
            return;
        }
        if event.state == HotKeyState::Released {
            handler_held.store(false, Ordering::Relaxed);
        } else if !handler_held.swap(true, Ordering::Relaxed) {
            let _ = weak.upgrade_in_event_loop(|ui| {
                if let Err(error) = launcher::toggle(&ui) {
                    eprintln!("Cannot toggle launcher: {error}");
                }
            });
        }
    }));
    Ok(Registration {
        manager,
        hotkey,
        shortcut,
        current_id,
        held,
    })
}
