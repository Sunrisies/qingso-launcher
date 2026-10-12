use slint::Model;
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
};
#[path = "../src/config.rs"]
#[allow(dead_code)]
mod config;
#[path = "../src/recommendations.rs"]
#[allow(dead_code)]
mod recommendations;
slint::include_modules!();
fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::temp_dir().join(format!("qingso-rec-check-{}", std::process::id()));
    unsafe {
        std::env::set_var("QINGSO_CONFIG_DIR", &directory);
    }
    let ui = AppWindow::new()?;
    let settings = SettingsWindow::new()?;
    let opened = Rc::new(RefCell::new(vec![]));
    let urls = opened.clone();
    let fail = Rc::new(Cell::new(false));
    let failure = fail.clone();
    recommendations::bind_with_opener(
        &ui,
        &settings,
        Rc::new(RefCell::new(recommendations::Store::load()?)),
        move |item| {
            if failure.get() {
                return Err(std::io::Error::other("simulated failure"));
            }
            urls.borrow_mut().push(item.target.clone());
            Ok(())
        },
    );
    let initial = settings.get_recommendations().row_count();
    settings.invoke_begin_add_recommendation();
    settings.invoke_save_recommendation(-1, "".into(), 2, "bad".into());
    assert!(settings.get_recommendation_error());
    assert!(settings.get_recommendation_editor_open());
    settings.invoke_save_recommendation(-1, "UniqueTest".into(), 2, "https://example.com".into());
    assert!(!settings.get_recommendation_editor_open());
    assert_eq!(settings.get_recommendations().row_count(), initial + 1);
    ui.set_query_text("uniquetest".into());
    ui.invoke_recommendation_query("uniquetest".into());
    assert_eq!(ui.get_recommendations().row_count(), 1);
    ui.invoke_open_recommendation(0);
    assert_eq!(&opened.borrow()[0], "https://example.com");
    fail.set(true);
    ui.invoke_open_recommendation(0);
    assert!(ui.get_query_error());
    settings.invoke_edit_recommendation(initial as i32);
    assert_eq!(settings.get_recommendation_name(), "UniqueTest");
    settings.invoke_save_recommendation(
        initial as i32,
        "UniqueTest".into(),
        1,
        directory.to_string_lossy().into_owned().into(),
    );
    assert!(!settings.get_recommendation_error());
    assert_eq!(recommendations::Store::load()?.items[initial].kind, 1);
    settings.invoke_delete_recommendation(initial as i32);
    assert_eq!(settings.get_recommendations().row_count(), initial);
    std::fs::remove_file(directory.join("recommendations.json"))?;
    std::fs::remove_dir(directory)?;
    println!("PASS: recommendations CRUD, filtering, opening, failure feedback and persistence");
    Ok(())
}
