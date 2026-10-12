use crate::{AppWindow, SearchTagEntry, SettingsWindow, search_tags::Store};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

fn refresh(settings: &SettingsWindow, store: &Store) {
    let model: ModelRc<SearchTagEntry> = Rc::new(VecModel::from(
        store
            .tags
            .iter()
            .map(|tag| SearchTagEntry {
                keyword: tag.keyword.clone().into(),
                name: tag.name.clone().into(),
                url_template: tag.url_template.clone().into(),
            })
            .collect::<Vec<_>>(),
    ))
    .into();
    settings.set_search_tags(model);
}

fn preview(ui: &AppWindow, store: &Store) {
    let input = ui.get_query_text();
    if let Some((tag, query)) = store.preview(input.as_str()) {
        ui.set_web_search_active(true);
        ui.set_web_search_title(format!("使用 {} 搜索", tag.name).into());
        ui.set_web_search_description(if query.is_empty() {
            "输入关键词后按回车搜索".into()
        } else {
            query.into()
        });
        ui.set_query_hint(format!("{} · 按回车搜索", tag.name).into());
        ui.set_query_error(false);
        ui.set_selected_index(0);
    } else {
        ui.set_web_search_active(false);
        ui.set_query_hint("".into());
        ui.set_query_error(false);
    }
}

pub fn bind(ui: &AppWindow, settings: &SettingsWindow, store: Rc<RefCell<Store>>) {
    bind_with_opener(ui, settings, store, webbrowser::open);
}

pub fn bind_with_opener(
    ui: &AppWindow,
    settings: &SettingsWindow,
    store: Rc<RefCell<Store>>,
    open: impl Fn(&str) -> std::io::Result<()> + 'static,
) {
    refresh(settings, &store.borrow());
    let weak = ui.as_weak();
    let changed_store = store.clone();
    ui.on_query_changed(move |_| {
        if let Some(ui) = weak.upgrade() {
            preview(&ui, &changed_store.borrow());
        }
    });
    let weak = ui.as_weak();
    let submitted_store = store.clone();
    ui.on_query_submitted(move |input| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let result = submitted_store.borrow().resolve(input.as_str());
        match result {
            Ok(request) => {
                ui.set_web_search_title(format!("使用 {} 搜索", request.name).into());
                ui.set_web_search_description(request.query.into());
                match open(&request.url) {
                    Ok(()) => {
                        let _ = ui.hide();
                    }
                    Err(error) => {
                        ui.set_query_error(true);
                        ui.set_query_hint(format!("浏览器打开失败：{error}").into());
                    }
                }
            }
            Err(error) => {
                ui.set_query_error(true);
                ui.set_query_hint(error.to_string().into());
            }
        }
    });

    let weak = settings.as_weak();
    let edit_store = store.clone();
    settings.on_edit_tag(move |index| {
        if let Some(settings) = weak.upgrade()
            && let Some(tag) = edit_store.borrow().tags.get(index as usize)
        {
            settings.set_editing_tag_index(index);
            settings.set_tag_keyword(tag.keyword.clone().into());
            settings.set_tag_name(tag.name.clone().into());
            settings.set_tag_template(tag.url_template.clone().into());
            settings.set_tag_status("编辑后点击保存修改。".into());
            settings.set_tag_error(false);
        }
    });
    let weak = settings.as_weak();
    let weak_ui = ui.as_weak();
    let save_store = store.clone();
    settings.on_save_tag(move |index, keyword, name, template| {
        let Some(settings) = weak.upgrade() else {
            return;
        };
        match save_store.borrow_mut().save_tag(
            index,
            keyword.as_str(),
            name.as_str(),
            template.as_str(),
        ) {
            Ok(()) => {
                // Drop the mutable borrow before refreshing models below.
                settings.set_tag_error(false);
                settings.set_tag_status("标签已保存，立即生效。".into());
            }
            Err(error) => {
                settings.set_tag_error(true);
                settings.set_tag_status(error.to_string().into());
                return;
            }
        }
        refresh(&settings, &save_store.borrow());
        settings.invoke_reset_tag_editor();
        if let Some(ui) = weak_ui.upgrade() {
            preview(&ui, &save_store.borrow());
        }
    });
    let weak = settings.as_weak();
    let weak_ui = ui.as_weak();
    settings.on_delete_tag(move |index| {
        let Some(settings) = weak.upgrade() else {
            return;
        };
        if let Err(error) = store.borrow_mut().delete(index) {
            settings.set_tag_error(true);
            settings.set_tag_status(error.to_string().into());
            return;
        }
        refresh(&settings, &store.borrow());
        settings.invoke_reset_tag_editor();
        settings.set_tag_error(false);
        settings.set_tag_status("标签已删除。".into());
        if let Some(ui) = weak_ui.upgrade() {
            preview(&ui, &store.borrow());
        }
    });
}
