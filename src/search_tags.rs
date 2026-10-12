use crate::config;
use serde::{Deserialize, Serialize};
use std::{error::Error, io, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Tag {
    pub keyword: String,
    pub name: String,
    pub url_template: String,
}

pub struct Store {
    pub tags: Vec<Tag>,
    path: PathBuf,
}

pub struct Request {
    pub url: String,
    pub name: String,
    pub query: String,
}

fn invalid(message: &str) -> Box<dyn Error> {
    io::Error::other(message).into()
}

fn validate(tag: &Tag) -> Result<(), Box<dyn Error>> {
    if tag.keyword.is_empty()
        || tag.keyword.chars().count() > 24
        || !tag
            .keyword
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(invalid(
            "标签只能包含字母、数字、下划线或连字符，长度为 1～24 个字符",
        ));
    }
    if tag.name.trim().is_empty() {
        return Err(invalid("请输入搜索网站名称"));
    }
    if !tag.url_template.contains("{query}") {
        return Err(invalid("网址模板必须包含 {query}"));
    }
    let parsed = url::Url::parse(&tag.url_template.replace("{query}", "example"))
        .map_err(|_| invalid("请输入有效的网址模板"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(invalid("网址模板必须是 http 或 https 网站链接"));
    }
    Ok(())
}

impl Store {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        Self::load_from(config::path()?.with_file_name("search-tags.json"))
    }

    fn load_from(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        let tags = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Vec<Tag>>(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => vec![
                Tag {
                    keyword: "b".into(),
                    name: "百度".into(),
                    url_template: "https://www.baidu.com/s?wd={query}".into(),
                },
                Tag {
                    keyword: "n".into(),
                    name: "npm".into(),
                    url_template: "https://www.npmjs.com/search?q={query}".into(),
                },
            ],
            Err(error) => return Err(error.into()),
        };
        let mut normalized = Vec::<Tag>::new();
        for mut tag in tags {
            tag.keyword = tag.keyword.trim().to_lowercase();
            validate(&tag)?;
            if normalized.iter().any(|old| old.keyword == tag.keyword) {
                return Err(invalid("保存的搜索标签重复"));
            }
            normalized.push(tag);
        }
        Ok(Self {
            tags: normalized,
            path,
        })
    }

    pub fn save_tag(
        &mut self,
        index: i32,
        keyword: &str,
        name: &str,
        template: &str,
    ) -> Result<(), Box<dyn Error>> {
        if index < -1 || index >= self.tags.len() as i32 {
            return Err(invalid("要编辑的标签不存在"));
        }
        let tag = Tag {
            keyword: keyword.trim().to_lowercase(),
            name: name.trim().into(),
            url_template: template.trim().into(),
        };
        validate(&tag)?;
        if self
            .tags
            .iter()
            .enumerate()
            .any(|(position, old)| old.keyword == tag.keyword && position as i32 != index)
        {
            return Err(invalid("标签已存在，请使用其他标签"));
        }
        let mut next = self.tags.clone();
        if index == -1 {
            next.push(tag);
        } else {
            next[index as usize] = tag;
        }
        self.persist(next)
    }

    pub fn delete(&mut self, index: i32) -> Result<(), Box<dyn Error>> {
        if index < 0 || index >= self.tags.len() as i32 {
            return Err(invalid("要删除的标签不存在"));
        }
        let mut next = self.tags.clone();
        next.remove(index as usize);
        self.persist(next)
    }

    fn persist(&mut self, next: Vec<Tag>) -> Result<(), Box<dyn Error>> {
        config::write_atomic(&self.path, &serde_json::to_vec_pretty(&next)?)?;
        self.tags = next;
        Ok(())
    }

    pub fn preview<'a>(&'a self, input: &'a str) -> Option<(&'a Tag, &'a str)> {
        let (keyword, query) = input.trim_start().split_once(char::is_whitespace)?;
        let keyword = keyword.to_lowercase();
        self.tags
            .iter()
            .find(|tag| tag.keyword == keyword)
            .map(|tag| (tag, query.trim()))
    }

    pub fn resolve(&self, input: &str) -> Result<Request, Box<dyn Error>> {
        let (tag, query) = self
            .preview(input)
            .ok_or_else(|| invalid("请输入已配置的标签和关键词，例如 b Rust"))?;
        if query.is_empty() {
            return Err(invalid("请输入搜索内容"));
        }
        let encoded = url::form_urlencoded::byte_serialize(query.as_bytes())
            .collect::<String>()
            .replace('+', "%20");
        let target = url::Url::parse(&tag.url_template.replace("{query}", &encoded))?;
        Ok(Request {
            url: target.into(),
            name: tag.name.clone(),
            query: query.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::load_from(std::env::temp_dir().join(format!(
                "flow-tags-{}-{}.json",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )))
        .unwrap()
    }

    #[test]
    fn defaults_encode_unicode_and_reserved_characters() {
        let store = store();
        for (input, host, key, expected) in [
            (" b  中文 C++ & #? ", "www.baidu.com", "wd", "中文 C++ & #?"),
            (
                "N @scope/my package",
                "www.npmjs.com",
                "q",
                "@scope/my package",
            ),
        ] {
            let result = store.resolve(input).unwrap();
            let parsed = url::Url::parse(&result.url).unwrap();
            assert_eq!(parsed.host_str(), Some(host));
            assert_eq!(
                parsed.query_pairs().collect::<Vec<_>>(),
                vec![(key.into(), expected.into())]
            );
        }
        assert!(store.resolve("b ").is_err());
        assert!(store.resolve("unknown rust").is_err());
    }

    #[test]
    fn custom_tags_edit_delete_and_survive_reload() {
        let mut store = store();
        store
            .save_tag(
                -1,
                "g",
                "示例",
                "https://example.com/?q={query}&source=flow",
            )
            .unwrap();
        assert!(
            store
                .save_tag(-1, "B", "重复", "https://example.com/{query}")
                .is_err()
        );
        assert!(
            store
                .save_tag(-1, "x", "无效", "javascript:{query}")
                .is_err()
        );
        assert!(
            store
                .save_tag(-1, "x", "无效", "https://example.com/")
                .is_err()
        );
        store
            .save_tag(2, "g", "更新", "https://example.com/search?q={query}")
            .unwrap();
        let reloaded = Store::load_from(store.path.clone()).unwrap();
        assert_eq!(reloaded.tags, store.tags);
        assert_eq!(reloaded.resolve("g hello").unwrap().name, "更新");
        while !store.tags.is_empty() {
            store.delete(0).unwrap();
        }
        assert!(
            Store::load_from(store.path.clone())
                .unwrap()
                .tags
                .is_empty()
        );
        std::fs::remove_file(store.path).unwrap();
    }
}
