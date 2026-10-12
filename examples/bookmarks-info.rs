//! Read-only check: output counts/status, never bookmark titles or URLs.
use slint::ComponentHandle;
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let ui = AppWindow::new()?;
    let settings = SettingsWindow::new()?;
    browser_bookmarks::bind(&ui, &settings);
    let weak = settings.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(150),
        move || {
            let settings = weak.upgrade().unwrap();
            if settings.get_bookmark_status().contains("正在") {
                return;
            }
            println!("Local Chrome Bookmarks: {}", settings.get_bookmark_status());
            assert!(!settings.get_bookmark_error());
            slint::quit_event_loop().unwrap();
        },
    );
    slint::run_event_loop_until_quit()?;
    Ok(())
}
