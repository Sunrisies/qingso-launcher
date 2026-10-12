use crate::{AppWindow, window_placement};
use slint::{
    ComponentHandle,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use std::{cell::Cell, time::Duration};

thread_local! {
    static FOCUS_WATCHER: slint::Timer = slint::Timer::default();
}

pub fn configure_backend() -> Result<(), slint::PlatformError> {
    let mut backend = slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(|attributes| {
            let attributes = attributes.with_theme(Some(winit::window::Theme::Light));
            #[cfg(target_os = "windows")]
            {
                use winit::platform::windows::WindowAttributesExtWindows;
                attributes.with_skip_taskbar(true)
            }
            #[cfg(not(target_os = "windows"))]
            {
                attributes
            }
        });
    // This launcher is mostly static UI: software avoids initializing GPU
    // driver contexts. Retain an explicit GPU override for animated workloads.
    if let Ok(renderer) = std::env::var("QINGSO_RENDERER") {
        backend = backend.renderer_name(renderer);
    } else if std::env::var_os("SLINT_BACKEND").is_none() {
        backend = backend.renderer_name("software".into());
    }
    #[cfg(target_os = "macos")]
    let backend = {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        let mut builder =
            winit::event_loop::EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
        builder.with_activation_policy(ActivationPolicy::Accessory);
        backend.with_winit_event_loop_builder(builder)
    };
    backend.select()
}

pub fn bind_window(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.hide();
        }
    });
    ui.on_quit_requested(|| {
        let _ = slint::quit_event_loop();
    });
    let weak = ui.as_weak();
    let modifiers = Cell::new(winit::keyboard::ModifiersState::empty());
    ui.window().on_winit_window_event(move |_window, event| {
        if let Some(ui) = weak.upgrade() {
            handle_window_event(&ui, event, &modifiers)
        } else {
            EventResult::Propagate
        }
    });
}

pub fn handle_window_event(
    ui: &AppWindow,
    event: &winit::event::WindowEvent,
    modifiers: &Cell<winit::keyboard::ModifiersState>,
) -> EventResult {
    use winit::{
        event::{ElementState, WindowEvent},
        keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    };
    match event {
        WindowEvent::Focused(false) => {
            modifiers.set(winit::keyboard::ModifiersState::empty());
            // An old native window can report blur after a rapid hide/show.
            // Recheck the current window after queued focus events are processed.
            let weak = ui.as_weak();
            slint::Timer::single_shot(Duration::from_millis(32), move || {
                if let Some(ui) = weak.upgrade() {
                    let focused = ui
                        .window()
                        .with_winit_window(|native| native.has_focus())
                        .unwrap_or(false);
                    if !focused {
                        let _ = ui.hide();
                    }
                }
            });
            return EventResult::Propagate;
        }
        WindowEvent::CloseRequested => {
            let _ = ui.hide();
            return EventResult::PreventDefault;
        }
        WindowEvent::ModifiersChanged(value) => modifiers.set(value.state()),
        WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
            if modifiers.get().control_key()
                && modifiers.get().shift_key()
                && event.physical_key == PhysicalKey::Code(KeyCode::KeyQ)
            {
                ui.invoke_quit_requested();
                return EventResult::PreventDefault;
            }
            let delta = match event.logical_key {
                Key::Named(NamedKey::ArrowUp) => Some(-1),
                Key::Named(NamedKey::ArrowDown) => Some(1),
                Key::Named(NamedKey::Escape) => {
                    let _ = ui.hide();
                    return EventResult::PreventDefault;
                }
                _ => None,
            };
            if let Some(delta) = delta {
                ui.invoke_move_selection(delta);
                return EventResult::PreventDefault;
            }
        }
        _ => {}
    }
    EventResult::Propagate
}

pub fn toggle(ui: &AppWindow) -> Result<(), slint::PlatformError> {
    if ui.window().is_visible() {
        ui.hide()
    } else {
        show(ui)
    }
}

pub fn show(ui: &AppWindow) -> Result<(), slint::PlatformError> {
    ui.show()?;
    bind_window(ui);
    window_placement::center_when_ready(ui.as_weak(), |ui| {
        ui.window()
            .with_winit_window(|native| native.focus_window());
        ui.invoke_focus_search();
        watch_focus(ui.as_weak());
    });
    Ok(())
}

fn watch_focus(weak: slint::Weak<AppWindow>) {
    // Some native blur messages are lost when Windows rapidly recreates a popup.
    // Poll only while visible; the hidden launcher leaves this timer stopped.
    FOCUS_WATCHER.with(|timer| {
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(80),
            move || {
                let Some(ui) = weak.upgrade() else {
                    FOCUS_WATCHER.with(|timer| timer.stop());
                    return;
                };
                if !ui.window().is_visible() {
                    FOCUS_WATCHER.with(|timer| timer.stop());
                    return;
                }
                if ui.window().with_winit_window(|native| native.has_focus()) == Some(false) {
                    let _ = ui.hide();
                    FOCUS_WATCHER.with(|timer| timer.stop());
                }
            },
        )
    });
}
