//! Export the settings layout without registering a global shortcut.
use slint::ComponentHandle;
use std::{error::Error, io::Write, time::Duration};
slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let ui = SettingsWindow::new()?;
    ui.set_is_macos(cfg!(target_os = "macos"));
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());
    ui.set_platform_text(format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH).into());
    ui.set_current_hotkey("Ctrl+Alt+Space".into());
    ui.set_edited_hotkey("Ctrl+Alt+Space".into());
    let tags_page = std::env::args().nth(1).as_deref() == Some("tags");
    ui.set_show_search_tags(tags_page);
    ui.set_search_tags(
        std::rc::Rc::new(slint::VecModel::from(vec![
            SearchTagEntry {
                keyword: "b".into(),
                name: "百度".into(),
                url_template: "https://www.baidu.com/s?wd={query}".into(),
            },
            SearchTagEntry {
                keyword: "n".into(),
                name: "npm".into(),
                url_template: "https://www.npmjs.com/search?q={query}".into(),
            },
        ]))
        .into(),
    );
    let config = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("FlowLauncherPreview/hotkey.txt");
    ui.set_config_path(config.to_string_lossy().into_owned().into());
    ui.show()?;
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(350), move || {
        let ui = weak.upgrade().unwrap();
        let pixels = ui.window().take_snapshot().unwrap();
        let (width, height) = (pixels.width(), pixels.height());
        let mut file = std::fs::File::create(if tags_page {
            "tags-preview.bmp"
        } else {
            "settings-preview.bmp"
        })
        .unwrap();
        file.write_all(b"BM").unwrap();
        file.write_all(&(54 + width * height * 4).to_le_bytes())
            .unwrap();
        file.write_all(&[0; 4]).unwrap();
        file.write_all(&54u32.to_le_bytes()).unwrap();
        file.write_all(&40u32.to_le_bytes()).unwrap();
        file.write_all(&(width as i32).to_le_bytes()).unwrap();
        file.write_all(&(-(height as i32)).to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&32u16.to_le_bytes()).unwrap();
        file.write_all(&[0; 24]).unwrap();
        for pixel in pixels.as_slice() {
            file.write_all(&[pixel.b, pixel.g, pixel.r, pixel.a])
                .unwrap();
        }
        println!("Settings rendered: {width} × {height}");
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
