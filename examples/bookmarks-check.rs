//! Verify local Bookmarks discovery, ll search, opening, auto-refresh and settings.
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
#[path = "../src/browser_bookmarks.rs"]
#[allow(dead_code)]
mod browser_bookmarks;
#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;
#[path = "../src/search.rs"]
#[allow(dead_code)]
mod search;
#[path = "../src/search_tags.rs"]
#[allow(dead_code)]
mod search_tags;
slint::include_modules!();

fn data(count: usize) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"roots":{"bookmark_bar":{"type":"folder","name":"收藏夹","children":(0..count).map(|id| serde_json::json!({"type":"url","name":format!("Rust 中文 {id}"),"url":format!("https://example.com/rust/{id}")})).collect::<Vec<_>>()}}})).unwrap()
}
fn main() -> Result<(), Box<dyn Error>> {
    let directory =
        std::env::temp_dir().join(format!("qingso-bookmarks-check-{}", std::process::id()));
    let browser = directory.join("User Data");
    for (profile, count) in [("Default", 5), ("Profile 1", 1)] {
        std::fs::create_dir_all(browser.join(profile))?;
        std::fs::write(browser.join(profile).join("Bookmarks"), data(count))?;
    }
    unsafe {
        std::env::set_var("QINGSO_CONFIG_DIR", &directory);
    }
    config::write_atomic(
        &directory.join("browser-bookmarks.json"),
        &serde_json::to_vec(&serde_json::json!({"data_directory":browser}))?,
    )?;
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let ui = AppWindow::new()?;
    let settings = SettingsWindow::new()?;
    search::bind(
        &ui,
        &settings,
        Rc::new(RefCell::new(search_tags::Store::load()?)),
    );
    let opened = Rc::new(RefCell::new(vec![]));
    let urls = opened.clone();
    let fail = Rc::new(Cell::new(false));
    let failure = fail.clone();
    browser_bookmarks::bind_with_opener(&ui, &settings, move |url| {
        if failure.get() {
            return Err(std::io::Error::other("test browser failure"));
        }
        urls.borrow_mut().push(url.to_string());
        Ok(())
    });
    ui.show()?;
    ui.invoke_focus_search();
    let weak = ui.as_weak();
    let preferences = settings.as_weak();
    let stage = Cell::new(0);
    let ticks = Cell::new(0);
    let fixture = browser.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(150), move || {
        ticks.set(ticks.get()+1); assert!(ticks.get()<90, "Bookmark check timeout");
        let ui = weak.upgrade().unwrap(); let settings = preferences.upgrade().unwrap();
        match stage.get() {
            0 => {
                if !settings.get_bookmark_status().contains("6 个书签") {return;}
                ui.set_query_text("ll RUST 中文".into()); ui.invoke_query_changed(ui.get_query_text());
                assert!(ui.get_bookmark_search_active()); assert_eq!(ui.get_browser_bookmarks().row_count(), 6);
            }
            1 => {for _ in 0..6 {ui.invoke_move_selection(1);}}
            2 => {
                assert_eq!(ui.get_selected_index(), 5); assert_eq!(ui.get_results_scroll_y(), -162.0);
                ui.window().dispatch_event(WindowEvent::KeyPressed {text: Key::Return.into()});
                ui.window().dispatch_event(WindowEvent::KeyReleased {text: Key::Return.into()});
                assert!(!ui.window().is_visible()); assert_eq!(opened.borrow().len(), 1);
                assert_eq!(opened.borrow()[0], "https://example.com/rust/0");
                ui.show().unwrap(); fail.set(true);
                ui.set_query_text("ll /rust/1".into()); ui.invoke_query_changed(ui.get_query_text());
                assert_eq!(ui.get_browser_bookmarks().row_count(), 1); ui.invoke_open_bookmark(0);
                assert!(ui.get_query_error()); assert!(ui.window().is_visible());
                ui.set_query_text("ll no-match".into()); ui.invoke_query_changed(ui.get_query_text());
                assert_eq!(ui.get_browser_bookmarks().row_count(), 0);
                std::fs::write(fixture.join("Default/Bookmarks"), data(1)).unwrap();
            }
            3 => {
                if !settings.get_bookmark_status().contains("2 个书签") {return;}
                ui.set_query_text("ll ".into()); ui.invoke_query_changed(ui.get_query_text());
                assert_eq!(ui.get_browser_bookmarks().row_count(), 2);
                let original = settings.get_bookmark_directory();
                settings.invoke_save_bookmark_directory("Z:\\this-directory-does-not-exist".into());
                assert!(settings.get_bookmark_error()); assert_eq!(settings.get_bookmark_directory(), original);
                settings.invoke_save_bookmark_directory(fixture.join("Default").to_string_lossy().into_owned().into());
            }
            4 => {
                if !settings.get_bookmark_status().contains("1 个书签") {return;}
                assert_eq!(ui.get_browser_bookmarks().row_count(), 1);
                println!("PASS: local multi-profile Bookmarks, ll Unicode/title/URL filtering, scrolling, Enter/click opening, error retention, auto-refresh and custom directory");
                slint::quit_event_loop().unwrap();
            }
            _ => unreachable!(),
        }
        stage.set(stage.get()+1);
    });
    slint::run_event_loop_until_quit()?;
    drop(ui);
    drop(settings);
    drop(timer);
    for profile in ["Default", "Profile 1"] {
        std::fs::remove_file(browser.join(profile).join("Bookmarks"))?;
        std::fs::remove_dir(browser.join(profile))?;
    }
    std::fs::remove_dir(browser)?;
    std::fs::remove_file(directory.join("browser-bookmarks.json"))?;
    std::fs::remove_dir(directory)?;
    Ok(())
}
