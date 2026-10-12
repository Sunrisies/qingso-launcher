use crate::{AppWindow, RecommendationEntry, SettingsWindow, config};
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    error::Error,
    io,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub name: String,
    pub kind: i32,
    pub target: String,
}
pub struct Store {
    pub items: Vec<Item>,
    path: PathBuf,
}

fn validate(item: &Item) -> Result<(), Box<dyn Error>> {
    if item.name.trim().is_empty() || item.target.trim().is_empty() {
        return Err(io::Error::other("请填写名称和打开目标").into());
    }
    let path = Path::new(&item.target);
    let valid = match item.kind {
        0 => {
            path.is_absolute()
                && (path.is_file()
                    || (cfg!(target_os = "macos")
                        && path.is_dir()
                        && item.target.ends_with(".app")))
        }
        1 => path.is_absolute() && path.is_dir(),
        2 => url::Url::parse(&item.target)
            .is_ok_and(|u| matches!(u.scheme(), "https" | "http") && u.host_str().is_some()),
        _ => false,
    };
    if !valid {
        return Err(io::Error::other(
            "应用或文件夹请填写存在的绝对路径，网页请填写 http/https 地址",
        )
        .into());
    }
    Ok(())
}
impl Store {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        Self::load_from(config::path()?.with_file_name("recommendations.json"))
    }
    fn load_from(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        let items = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Vec<Item>>(&bytes)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => defaults(),
            Err(e) => return Err(e.into()),
        };
        // Missing targets stay editable rather than preventing startup.
        Ok(Self { items, path })
    }
    pub fn save(
        &mut self,
        index: i32,
        name: &str,
        kind: i32,
        target: &str,
    ) -> Result<(), Box<dyn Error>> {
        if index < -1 || index >= self.items.len() as i32 {
            return Err(io::Error::other("推荐项目不存在").into());
        }
        let item = Item {
            name: name.trim().into(),
            kind,
            target: target.trim().trim_matches('"').into(),
        };
        validate(&item)?;
        let mut next = self.items.clone();
        if index == -1 {
            next.push(item);
        } else {
            next[index as usize] = item;
        }
        self.persist(next)
    }
    pub fn delete(&mut self, index: i32) -> Result<(), Box<dyn Error>> {
        if index < 0 || index >= self.items.len() as i32 {
            return Err(io::Error::other("推荐项目不存在").into());
        }
        let mut next = self.items.clone();
        next.remove(index as usize);
        self.persist(next)
    }
    fn persist(&mut self, next: Vec<Item>) -> Result<(), Box<dyn Error>> {
        config::write_atomic(&self.path, &serde_json::to_vec_pretty(&next)?)?;
        self.items = next;
        Ok(())
    }
    fn filtered(&self, query: &str) -> Vec<Item> {
        let query = query.trim().to_lowercase();
        self.items
            .iter()
            .filter(|i| {
                i.name.to_lowercase().contains(&query) || i.target.to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }
}
fn defaults() -> Vec<Item> {
    let mut items = vec![Item {
        name: "GitHub".into(),
        kind: 2,
        target: "https://github.com".into(),
    }];
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        for (name, path) in [
            ("个人文件夹", PathBuf::from(&home)),
            ("下载", PathBuf::from(&home).join("Downloads")),
            ("文档", PathBuf::from(&home).join("Documents")),
        ] {
            if path.is_dir() {
                items.push(Item {
                    name: name.into(),
                    kind: 1,
                    target: path.to_string_lossy().into(),
                });
            }
        }
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        let path = PathBuf::from(root).join("System32/calc.exe");
        if path.is_file() {
            items.push(Item {
                name: "计算器".into(),
                kind: 0,
                target: path.to_string_lossy().into(),
            });
        }
    }
    #[cfg(target_os = "macos")]
    for (name, path) in [
        ("终端", "/System/Applications/Utilities/Terminal.app"),
        ("计算器", "/System/Applications/Calculator.app"),
    ] {
        if Path::new(path).exists() {
            items.push(Item {
                name: name.into(),
                kind: 0,
                target: path.into(),
            });
        }
    }
    items
}
fn model(items: &[Item]) -> ModelRc<RecommendationEntry> {
    Rc::new(VecModel::from(
        items
            .iter()
            .map(|i| RecommendationEntry {
                name: i.name.clone().into(),
                kind: i.kind,
                target: i.target.clone().into(),
            })
            .collect::<Vec<_>>(),
    ))
    .into()
}
fn refresh(ui: &AppWindow, settings: &SettingsWindow, store: &Store) {
    settings.set_recommendations(model(&store.items));
    ui.set_recommendations(model(&store.filtered(ui.get_query_text().as_str())));
    ui.set_selected_index(0);
    ui.set_results_scroll_y(0.0);
}
pub fn bind(ui: &AppWindow, settings: &SettingsWindow, store: Rc<RefCell<Store>>) {
    bind_with_opener(ui, settings, store, |item| {
        validate(item).map_err(|e| io::Error::other(e.to_string()))?;
        if item.kind == 2 {
            webbrowser::open(&item.target)
        } else {
            open::that(&item.target)
        }
    });
}
pub fn bind_with_opener(
    ui: &AppWindow,
    settings: &SettingsWindow,
    store: Rc<RefCell<Store>>,
    open: impl Fn(&Item) -> io::Result<()> + 'static,
) {
    refresh(ui, settings, &store.borrow());
    let weak = ui.as_weak();
    let query_store = store.clone();
    ui.on_recommendation_query(move |query| {
        if let Some(ui) = weak.upgrade() {
            ui.set_recommendations(model(&query_store.borrow().filtered(query.as_str())));
            ui.set_selected_index(0);
            ui.set_results_scroll_y(0.0);
        }
    });
    let weak = ui.as_weak();
    let open_store = store.clone();
    ui.on_open_recommendation(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let items = open_store.borrow().filtered(ui.get_query_text().as_str());
        let result = items
            .get(index as usize)
            .ok_or_else(|| io::Error::other("没有匹配的推荐，请在设置中添加或修改搜索内容"))
            .and_then(&open);
        match result {
            Ok(()) => {
                let _ = ui.hide();
            }
            Err(e) => {
                ui.set_query_error(true);
                ui.set_query_hint(format!("打开失败：{e}").into());
            }
        }
    });
    let weak = settings.as_weak();
    let edit_store = store.clone();
    settings.on_edit_recommendation(move |index| {
        if let Some(settings) = weak.upgrade()
            && let Some(item) = edit_store.borrow().items.get(index as usize)
        {
            settings.set_recommendation_index(index);
            settings.set_recommendation_name(item.name.clone().into());
            settings.set_recommendation_kind(item.kind);
            settings.set_recommendation_target(item.target.clone().into());
            settings.set_recommendation_status("".into());
            settings.set_recommendation_error(false);
            settings.set_recommendation_editor_open(true);
        }
    });
    let weak = settings.as_weak();
    let weak_ui = ui.as_weak();
    let save_store = store.clone();
    settings.on_save_recommendation(move |index, name, kind, target| {
        let Some(settings) = weak.upgrade() else {
            return;
        };
        let result = save_store
            .borrow_mut()
            .save(index, name.as_str(), kind, target.as_str());
        match result {
            Ok(()) => {
                settings.set_recommendation_editor_open(false);
                settings.set_recommendation_error(false);
                settings.set_recommendation_status("推荐已保存，立即生效。".into());
                if let Some(ui) = weak_ui.upgrade() {
                    refresh(&ui, &settings, &save_store.borrow());
                }
            }
            Err(e) => {
                settings.set_recommendation_error(true);
                settings.set_recommendation_status(e.to_string().into());
            }
        }
    });
    let weak = settings.as_weak();
    let weak_ui = ui.as_weak();
    settings.on_delete_recommendation(move |index| {
        let Some(settings) = weak.upgrade() else {
            return;
        };
        let result = store.borrow_mut().delete(index);
        match result {
            Ok(()) => {
                settings.set_recommendation_error(false);
                settings.set_recommendation_status("推荐已删除。".into());
                if let Some(ui) = weak_ui.upgrade() {
                    refresh(&ui, &settings, &store.borrow());
                }
            }
            Err(e) => {
                settings.set_recommendation_error(true);
                settings.set_recommendation_status(e.to_string().into());
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crud_filters_and_persistence() {
        let path = std::env::temp_dir().join(format!(
            "qingso-recommendations-{}.json",
            std::process::id()
        ));
        let mut store = Store {
            items: vec![],
            path: path.clone(),
        };
        assert!(store.save(-1, "", 2, "https://example.com").is_err());
        assert!(store.save(-1, "bad", 2, "javascript:alert(1)").is_err());
        assert!(store.save(-1, "missing", 0, "missing.exe").is_err());
        store.save(-1, "Example", 2, "https://example.com").unwrap();
        store.save(0, "Updated", 2, "https://example.org").unwrap();
        assert_eq!(store.filtered("UPDATED").len(), 1);
        assert!(store.filtered("absent").is_empty());
        assert_eq!(Store::load_from(path.clone()).unwrap().items, store.items);
        store.delete(0).unwrap();
        assert!(Store::load_from(path.clone()).unwrap().items.is_empty());
        std::fs::remove_file(path).unwrap();
    }
}
