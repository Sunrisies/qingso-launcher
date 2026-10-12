//! Exercise Enter and settings callbacks using a captured browser opener.
use slint::{
    ComponentHandle, Model,
    platform::{Key, WindowEvent},
};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
    time::Duration,
};
#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;
#[path = "../src/launcher.rs"]
#[allow(dead_code)]
mod launcher;
#[path = "../src/recommendations.rs"]
#[allow(dead_code)]
mod recommendations;
#[path = "../src/search.rs"]
#[allow(dead_code)]
mod search;
#[path = "../src/search_tags.rs"]
mod search_tags;
#[path = "../src/window_placement.rs"]
mod window_placement;
slint::include_modules!();

fn submit(ui: &AppWindow, input: &str) {
    ui.show().unwrap();
    ui.invoke_focus_search();
    ui.set_query_text(input.into());
    ui.invoke_query_changed(input.into());
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Return.into(),
    });
}

fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::temp_dir().join(format!("flow-search-check-{}", std::process::id()));
    // Configure the isolated profile before the native backend starts threads.
    unsafe {
        std::env::set_var("QINGSO_CONFIG_DIR", &directory);
    }
    launcher::configure_backend()?;
    let ui = AppWindow::new()?;
    let preferences = SettingsWindow::new()?;
    let store = Rc::new(RefCell::new(search_tags::Store::load()?));
    let opened = Rc::new(RefCell::new(Vec::<String>::new()));
    let fail = Rc::new(Cell::new(false));
    let urls = opened.clone();
    let failure = fail.clone();
    search::bind_with_opener(&ui, &preferences, store, move |url| {
        if failure.get() {
            return Err(std::io::Error::other("test browser error"));
        }
        urls.borrow_mut().push(url.into());
        Ok(())
    });
    ui.show()?;
    let urls = opened.clone();
    recommendations::bind_with_opener(
        &ui,
        &preferences,
        Rc::new(RefCell::new(recommendations::Store::load()?)),
        move |item| {
            urls.borrow_mut().push(item.target.clone());
            Ok(())
        },
    );
    let weak = ui.as_weak();
    let settings_weak = preferences.as_weak();
    slint::Timer::single_shot(Duration::from_millis(350), move || {
        let ui = weak.upgrade().unwrap();
        let settings = settings_weak.upgrade().unwrap();
        submit(&ui, "GitHub");
        assert!(!ui.window().is_visible());
        assert_eq!(opened.borrow().as_slice(), &["https://github.com"]);
        opened.borrow_mut().clear();
        submit(&ui, "b 中文 C++ & Rust");
        assert!(!ui.window().is_visible());
        assert_eq!(opened.borrow().len(), 1);
        let url = url::Url::parse(&opened.borrow()[0]).unwrap();
        assert_eq!(url.query_pairs().next().unwrap().1, "中文 C++ & Rust");
        assert_eq!(url.host_str(), Some("www.baidu.com"));
        submit(&ui, "x rust");
        assert!(ui.window().is_visible());
        assert!(ui.get_query_error());
        settings.invoke_begin_add_tag();
        assert!(settings.get_editor_open());
        settings.invoke_save_tag(
            -1,
            "g".into(),
            "GitHub".into(),
            "https://github.com/search?q={query}".into(),
        );
        assert!(!settings.get_tag_error());
        assert!(!settings.get_editor_open());
        assert_eq!(settings.get_search_tags().row_count(), 3);
        settings.invoke_begin_add_tag();
        settings.invoke_save_tag(
            -1,
            "B".into(),
            "重复".into(),
            "https://example.com/?q={query}".into(),
        );
        assert!(settings.get_tag_error());
        assert!(settings.get_editor_open());
        settings.invoke_cancel_tag_editor();
        assert!(!settings.get_editor_open());
        assert_eq!(settings.get_search_tags().row_count(), 3);
        settings.invoke_edit_tag(2);
        assert!(settings.get_editor_open());
        assert_eq!(settings.get_tag_keyword(), "g");
        settings.invoke_save_tag(
            2,
            "gh".into(),
            "GitHub".into(),
            "https://github.com/search?q={query}".into(),
        );
        assert!(!settings.get_tag_error());
        submit(&ui, "gh serde");
        assert!(!ui.window().is_visible());
        assert_eq!(opened.borrow().len(), 2);
        assert_eq!(
            url::Url::parse(&opened.borrow()[1]).unwrap().host_str(),
            Some("github.com")
        );
        fail.set(true);
        submit(&ui, "n @scope/my package");
        assert!(ui.window().is_visible());
        assert!(ui.get_query_error());
        submit(&ui, "n ");
        assert!(ui.window().is_visible());
        assert!(ui.get_query_error());
        assert_eq!(opened.borrow().len(), 2);
        settings.invoke_delete_tag(2);
        assert_eq!(settings.get_search_tags().row_count(), 2);
        assert_eq!(search_tags::Store::load().unwrap().tags.len(), 2);
        println!(
            "PASS: Enter opens encoded search URLs and hides; CRUD takes effect immediately and persists"
        );
        println!(
            "PASS: duplicate tags, unknown tags, empty queries and browser failures remain visible with errors"
        );
        settings.set_show_search_tags(true);
        settings.show().unwrap();
        settings.invoke_begin_add_tag();
        settings.set_tag_keyword("k".into());
        settings.set_tag_name("键盘测试".into());
        settings.set_tag_template("https://example.com/?q={query}".into());
        let weak = settings.as_weak();
        slint::Timer::single_shot(Duration::from_millis(350), move || {
            let settings = weak.upgrade().unwrap();
            settings.window().dispatch_event(WindowEvent::KeyPressed {
                text: Key::Return.into(),
            });
            settings.window().dispatch_event(WindowEvent::KeyReleased {
                text: Key::Return.into(),
            });
            assert!(
                !settings.get_editor_open(),
                "Enter should save and close the dialog"
            );
            assert_eq!(settings.get_search_tags().row_count(), 3);
            settings.invoke_begin_add_tag();
            let weak = settings.as_weak();
            slint::Timer::single_shot(Duration::from_millis(350), move || {
                let settings = weak.upgrade().unwrap();
                settings.window().dispatch_event(WindowEvent::KeyPressed {
                    text: Key::Return.into(),
                });
                assert!(
                    settings.get_editor_open(),
                    "Invalid data must leave the dialog open"
                );
                assert!(settings.get_tag_error());
                settings.window().dispatch_event(WindowEvent::KeyReleased {
                    text: Key::Return.into(),
                });
                settings.window().dispatch_event(WindowEvent::KeyPressed {
                    text: Key::Escape.into(),
                });
                assert!(
                    !settings.get_editor_open(),
                    "Escape should cancel the dialog"
                );
                assert_eq!(settings.get_search_tags().row_count(), 3);
                println!(
                    "PASS: add/edit dialogs, cancel, Enter save, validation retention and Escape dismissal"
                );
                slint::quit_event_loop().unwrap();
            });
        });
    });
    slint::run_event_loop_until_quit()?;
    std::fs::remove_file(directory.join("search-tags.json"))?;
    std::fs::remove_dir(directory)?;
    Ok(())
}
