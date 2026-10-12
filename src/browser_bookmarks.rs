use crate::{AppWindow, BrowserBookmarkEntry, SettingsWindow, config};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, SystemTime},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub profile: String,
    pub folder: String,
}
#[derive(Default, Deserialize, Serialize)]
struct Preferences {
    data_directory: String,
}
#[derive(Default)]
struct Snapshot {
    bookmarks: Vec<Bookmark>,
    status: String,
    error: bool,
}
#[derive(Clone, Debug, PartialEq)]
struct Source {
    path: PathBuf,
    profile: String,
    modified: Option<SystemTime>,
    size: u64,
}

pub fn default_directory() -> io::Result<PathBuf> {
    #[cfg(windows)]
    {
        Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA")
                .ok_or_else(|| io::Error::other("LOCALAPPDATA 不可用"))?,
        )
        .join("Google/Chrome/User Data"))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(
            PathBuf::from(std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME 不可用"))?)
                .join("Library/Application Support/Google/Chrome"),
        )
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Ok(
            PathBuf::from(std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME 不可用"))?)
                .join(".config/google-chrome"),
        )
    }
}

fn preferences_path() -> io::Result<PathBuf> {
    Ok(config::path()?.with_file_name("browser-bookmarks.json"))
}

fn preferences() -> io::Result<Preferences> {
    match fs::read(preferences_path()?) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(e) => Err(e),
    }
}

fn source(path: PathBuf, profile: String) -> io::Result<Source> {
    let metadata = fs::metadata(&path)?;
    Ok(Source {
        path,
        profile,
        modified: metadata.modified().ok(),
        size: metadata.len(),
    })
}

fn sources(directory: &Path) -> io::Result<Vec<Source>> {
    // A custom path can be a Chrome data root or a single profile directory.
    if directory.join("Bookmarks").is_file() {
        return Ok(vec![source(
            directory.join("Bookmarks"),
            directory
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
        )?]);
    }
    let info = fs::read(directory.join("Local State"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let mut result = vec![];
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("Bookmarks");
        if !path.is_file() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let name = info
            .as_ref()
            .and_then(|v| v["profile"]["info_cache"][&id]["name"].as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&id);
        result.push(source(path, format!("{name} · {id}"))?);
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

fn parse(bytes: &[u8], profile: &str) -> io::Result<Vec<Bookmark>> {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let document: Value = serde_json::from_slice(bytes).map_err(io::Error::other)?;
    let roots = document["roots"]
        .as_object()
        .ok_or_else(|| io::Error::other("书签文件缺少 roots 节点"))?;
    let mut bookmarks = vec![];
    for root in roots.values() {
        visit(root, profile, &[], &mut bookmarks, 0);
    }
    Ok(bookmarks)
}

fn visit(
    node: &Value,
    profile: &str,
    folders: &[String],
    bookmarks: &mut Vec<Bookmark>,
    depth: usize,
) {
    if depth > 64 {
        return;
    }
    if node["type"] == "url" {
        let Some(url) = node["url"].as_str() else {
            return;
        };
        // Bookmarklets are scripts rather than pages, and cannot be opened as URLs.
        if !url::Url::parse(url).is_ok_and(|v| {
            matches!(v.scheme(), "http" | "https" | "file")
                && (v.scheme() == "file" || v.host_str().is_some())
        }) {
            return;
        }
        let title = node["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(url);
        bookmarks.push(Bookmark {
            title: title.into(),
            url: url.into(),
            profile: profile.into(),
            folder: folders.join(" / "),
        });
        return;
    }
    if let Some(children) = node["children"].as_array() {
        let mut folders = folders.to_vec();
        if let Some(name) = node["name"].as_str().filter(|s| !s.is_empty()) {
            folders.push(name.into());
        }
        for child in children {
            visit(child, profile, &folders, bookmarks, depth + 1);
        }
    }
}

fn load(sources: &[Source], previous: &[Bookmark]) -> Snapshot {
    let mut result = Snapshot::default();
    let mut failures = 0;
    for source in sources {
        let parsed = fs::File::open(&source.path).and_then(|f| {
            if source.size > 32 * 1024 * 1024 {
                return Err(io::Error::other("书签文件过大"));
            }
            let mut bytes = vec![];
            f.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
            parse(&bytes, &source.profile)
        });
        match parsed {
            Ok(bookmarks) => result.bookmarks.extend(bookmarks),
            Err(_) => {
                failures += 1;
                // Keep the last successfully read profile during a partial write.
                result.bookmarks.extend(
                    previous
                        .iter()
                        .filter(|b| b.profile == source.profile)
                        .cloned(),
                );
            }
        }
    }
    result.error = failures > 0;
    result.status = if sources.is_empty() {
        "没有找到 Bookmarks 文件，请检查 Chrome 数据目录。".into()
    } else if failures > 0 {
        format!(
            "已载入 {} 个书签；{failures} 个配置文件暂时无法读取，会自动重试。",
            result.bookmarks.len()
        )
    } else {
        format!(
            "已读取 {} 个 Chrome 配置文件 · {} 个书签",
            sources.len(),
            result.bookmarks.len()
        )
    };
    result
}

fn matching(bookmarks: &[Bookmark], query: &str) -> Vec<Bookmark> {
    let query = query.to_lowercase();
    let words: Vec<_> = query.split_whitespace().collect();
    bookmarks
        .iter()
        .filter(|b| {
            let text = format!("{} {}", b.title, b.url).to_lowercase();
            words.iter().all(|word| text.contains(word))
        })
        .cloned()
        .collect()
}

enum Command {
    Reload,
    Directory(PathBuf),
}

pub fn bind(ui: &AppWindow, settings: &SettingsWindow) {
    bind_with_opener(ui, settings, webbrowser::open);
}

pub fn bind_with_opener(
    ui: &AppWindow,
    settings: &SettingsWindow,
    open: impl Fn(&str) -> io::Result<()> + 'static,
) {
    let default = default_directory().unwrap_or_default();
    let preference = preferences().unwrap_or_default();
    let directory = if preference.data_directory.is_empty() {
        default.clone()
    } else {
        PathBuf::from(&preference.data_directory)
    };
    settings.set_bookmark_directory(directory.to_string_lossy().into_owned().into());
    settings.set_edited_bookmark_directory(preference.data_directory.into());
    let snapshot = Arc::new(Mutex::new(Snapshot {
        status: "正在读取 Chrome 本地书签…".into(),
        ..Default::default()
    }));
    let (sender, receiver) = mpsc::channel();
    let state = snapshot.clone();
    let weak = ui.as_weak();
    let weak_settings = settings.as_weak();
    let worker = thread::Builder::new()
        .name("chrome-bookmarks".into())
        .spawn(move || {
            let mut directory = directory;
            let mut fingerprints = None;
            let mut force = true;
            loop {
                let found = sources(&directory);
                match found {
                    Ok(found) => {
                        let retry = state.lock().unwrap().error;
                        if force || retry || fingerprints.as_ref() != Some(&found) {
                            let previous = state.lock().unwrap().bookmarks.clone();
                            *state.lock().unwrap() = load(&found, &previous);
                            fingerprints = Some(found);
                            notify(&weak, &weak_settings, &state);
                        }
                    }
                    Err(e) => {
                        if force || !state.lock().unwrap().error {
                            *state.lock().unwrap() = Snapshot {
                                bookmarks: vec![],
                                status: format!("无法读取 Chrome 书签目录：{e}"),
                                error: true,
                            };
                            notify(&weak, &weak_settings, &state);
                        }
                    }
                }
                force = false;
                match receiver.recv_timeout(Duration::from_secs(2)) {
                    Ok(Command::Reload) => force = true,
                    Ok(Command::Directory(path)) => {
                        directory = path;
                        fingerprints = None;
                        force = true;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
    if let Err(error) = worker {
        settings.set_bookmark_status(format!("书签读取无法启动：{error}").into());
        settings.set_bookmark_error(true);
    }

    let visible = Rc::new(RefCell::new(Vec::<Bookmark>::new()));
    let weak = ui.as_weak();
    let state = snapshot.clone();
    let matches = visible.clone();
    let last_query = RefCell::new(String::new());
    ui.on_bookmark_query(move |_| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let input = ui.get_query_text();
        let Some(query) = crate::search::bookmark_keyword(input.as_str()) else {
            return;
        };
        let same = *last_query.borrow() == input.as_str();
        let previous = same
            .then(|| {
                matches
                    .borrow()
                    .get(ui.get_selected_index() as usize)
                    .cloned()
            })
            .flatten();
        let failure = (same && ui.get_query_error()).then(|| ui.get_query_hint());
        let state = state.lock().unwrap();
        let next = matching(&state.bookmarks, query);
        let model: ModelRc<BrowserBookmarkEntry> = Rc::new(VecModel::from(
            next.iter()
                .map(|b| BrowserBookmarkEntry {
                    title: b.title.clone().into(),
                    url: b.url.clone().into(),
                    source: b.profile.clone().into(),
                })
                .collect::<Vec<_>>(),
        ))
        .into();
        ui.set_browser_bookmarks(model);
        let index = previous
            .and_then(|old| next.iter().position(|b| b == &old))
            .unwrap_or(0);
        ui.set_selected_index(index as i32);
        if !same {
            ui.set_results_scroll_y(0.0);
        }
        ui.set_query_error(state.error);
        ui.set_query_hint(if state.error || state.bookmarks.is_empty() {
            state.status.clone().into()
        } else if next.is_empty() {
            "没有匹配的 Chrome 书签".into()
        } else {
            format!("Chrome 书签 · {} 个匹配结果 · 回车打开", next.len()).into()
        });
        if let Some(message) = failure {
            ui.set_query_error(true);
            ui.set_query_hint(message);
        }
        *matches.borrow_mut() = next;
        *last_query.borrow_mut() = input.to_string();
    });
    let weak = ui.as_weak();
    ui.on_open_bookmark(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let result = visible
            .borrow()
            .get(index as usize)
            .ok_or_else(|| io::Error::other("没有可打开的书签"))
            .and_then(|b| open(&b.url));
        match result {
            Ok(()) => {
                let _ = ui.hide();
            }
            Err(e) => {
                ui.set_query_error(true);
                ui.set_query_hint(format!("书签打开失败：{e}").into());
            }
        }
    });
    let refresh = sender.clone();
    let weak = settings.as_weak();
    settings.on_refresh_bookmarks(move || {
        if let Some(settings) = weak.upgrade() {
            settings.set_bookmark_status("正在刷新书签…".into());
        }
        let _ = refresh.send(Command::Reload);
    });
    let weak = settings.as_weak();
    settings.on_save_bookmark_directory(move |input| {
        let Some(settings) = weak.upgrade() else {
            return;
        };
        let value = input.trim().trim_matches('"');
        let path = if value.is_empty() {
            default.clone()
        } else {
            PathBuf::from(value)
        };
        let result = if !path.is_absolute() || !path.is_dir() {
            Err(io::Error::other(
                "请输入存在的 Chrome 数据目录完整路径，留空使用默认目录",
            ))
        } else {
            preferences_path().and_then(|file| {
                config::write_atomic(
                    &file,
                    &serde_json::to_vec_pretty(&Preferences {
                        data_directory: value.into(),
                    })
                    .map_err(io::Error::other)?,
                )
            })
        };
        match result {
            Ok(()) => {
                settings.set_bookmark_error(false);
                settings.set_bookmark_directory(path.to_string_lossy().into_owned().into());
                settings.set_bookmark_status("已保存，正在读取书签…".into());
                let _ = sender.send(Command::Directory(path));
            }
            Err(e) => {
                settings.set_bookmark_error(true);
                settings.set_bookmark_status(format!("目录保存失败：{e}").into());
            }
        }
    });
}

fn notify(
    ui: &slint::Weak<AppWindow>,
    settings: &slint::Weak<SettingsWindow>,
    state: &Arc<Mutex<Snapshot>>,
) {
    let settings = settings.clone();
    let state = state.clone();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        if let Some(settings) = settings.upgrade() {
            let state = state.lock().unwrap();
            settings.set_bookmark_status(state.status.clone().into());
            settings.set_bookmark_error(state.error);
        }
        if crate::search::bookmark_keyword(ui.get_query_text().as_str()).is_some() {
            ui.invoke_bookmark_query(ui.get_query_text());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_bookmarks_unicode_urls_and_invalid_entries() {
        let bytes = br#"{"roots":{"bookmark_bar":{"name":"bar","type":"folder","children":[{"type":"folder","name":"Rust","children":[{"type":"url","name":"Book","url":"https://doc.rust-lang.org/book"}]},{"type":"url","name":"script","url":"javascript:alert(1)"}]},"other":{"type":"folder","children":[{"type":"url","name":"","url":"https://example.com"},{"type":"url","url":"not a url"}]}}}"#;
        let result = parse(bytes, "Default").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].folder, "bar / Rust");
        assert_eq!(matching(&result, "BOOK RUST").len(), 1);
        assert_eq!(matching(&result, "missing").len(), 0);
        let bytes = "\u{feff}{\"roots\":{\"other\":{\"children\":[{\"type\":\"url\",\"name\":\"中文\",\"url\":\"https://example.com\"}]}}}";
        assert_eq!(parse(bytes.as_bytes(), "用户").unwrap()[0].title, "中文");
        assert!(parse(b"{", "Default").is_err());
    }
    #[test]
    fn profiles_reload_and_partial_write_keep_previous_data() {
        let directory =
            std::env::temp_dir().join(format!("qingso-bookmarks-{}", std::process::id()));
        for id in ["Default", "Profile 1"] {
            fs::create_dir_all(directory.join(id)).unwrap();
            fs::write(directory.join(id).join("Bookmarks"), br#"{"roots":{"other":{"children":[{"type":"url","name":"Example","url":"https://example.com"}]}}}"#).unwrap();
        }
        let found = sources(&directory).unwrap();
        assert_eq!(found.len(), 2);
        let initial = load(&found, &[]);
        assert_eq!(initial.bookmarks.len(), 2);
        fs::write(directory.join("Default/Bookmarks"), "{").unwrap();
        let next = load(&sources(&directory).unwrap(), &initial.bookmarks);
        assert!(next.error);
        assert_eq!(next.bookmarks.len(), 2);
        for id in ["Default", "Profile 1"] {
            fs::remove_file(directory.join(id).join("Bookmarks")).unwrap();
            fs::remove_dir(directory.join(id)).unwrap();
        }
        fs::remove_dir(directory).unwrap();
    }
}
