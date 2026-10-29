//! 对话分类随 Profile 走（OD-ARCH-116）。
//!
//! 分类表以前躺在全局 `config/settings.v1.json` 里，切回基础标准后仍是
//! 「未分类 / 工作记录 / 文案写作 / 项目跟进」。这里把分类表变成 Profile 的
//! 一部分：
//!
//! * 每个非标准 Profile 在 `<app_root>/profiles/<name>/categories.json` 存一份；
//! * 切走时把当前生效的表**回存**给离开的 Profile（用户改名、新增都不丢）；
//! * 切入时用目标 Profile 的表；没有就取出厂清单；再没有就继承当前表落盘；
//! * 基础标准（omnidesk-base）不落任何文件，收敛为只有「未分类」一类。
//!
//! 只动分类表，不改写任何既有线程的 tag，也不碰成果目录里的文件。

use crate::app_paths::AppPaths;
use crate::settings::{self, ThreadCategory};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PROFILE_CATEGORIES_SCHEMA: &str = "omnidesk.profile-categories.v1";
pub const PROFILE_CATEGORIES_FILE: &str = "categories.json";

const EMBEDDED_DISCIPLINE: &str =
    include_str!("../../../samples/profiles/discipline.categories.json");

/// `profiles/<name>/categories.json` 的磁盘格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryFile {
    pub schema: String,
    pub profile: String,
    pub categories: Vec<ThreadCategory>,
}

/// 一次切换里分类表从哪来、写到哪去（回执给前端与日志，不含正文）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SwitchCategories {
    pub categories: Vec<ThreadCategory>,
    /// standard / file / embedded / inherited
    pub source: &'static str,
    /// 切走时回存到的 Profile 名（基础标准离开时为 None）。
    pub saved_to: Option<String>,
}

pub fn path(app_root: &Path, profile: &str) -> PathBuf {
    app_root
        .join("profiles")
        .join(profile)
        .join(PROFILE_CATEGORIES_FILE)
}

/// 出厂 Profile 的分类清单（编译期嵌入，分发后的二进制里也有）。
pub fn embedded_for(profile: &str) -> Option<&'static str> {
    match profile {
        "discipline" => Some(EMBEDDED_DISCIPLINE),
        _ => None,
    }
}

/// 解析一份清单：schema / profile 名必须对得上，分类表本身还要过 settings 的校验。
/// 损坏就报错，不猜、不静默退回默认四类。
pub fn parse(raw: &str, profile: &str) -> Result<Vec<ThreadCategory>, String> {
    let file: CategoryFile = serde_json::from_str(raw)
        .map_err(|e| format!("分类清单解析失败（{}）：{}", profile, e))?;
    if file.schema != PROFILE_CATEGORIES_SCHEMA {
        return Err(format!(
            "分类清单 schema 不匹配（{}）：期望 {}，实际 {}",
            profile, PROFILE_CATEGORIES_SCHEMA, file.schema
        ));
    }
    if file.profile != profile {
        return Err(format!(
            "分类清单归属不符：文件写的是「{}」，当前 Profile 是「{}」",
            file.profile, profile
        ));
    }
    settings::validate_categories(&file.categories)
        .map_err(|e| format!("分类清单（{}）不合法：{}", profile, e))?;
    Ok(file.categories)
}

/// 读某个 Profile 的分类表；文件不存在返回 `Ok(None)`（由调用方决定出厂/继承）。
pub fn load(app_root: &Path, profile: &str) -> Result<Option<Vec<ThreadCategory>>, String> {
    let file = path(app_root, profile);
    if !file.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&file)
        .map_err(|e| format!("读取分类清单失败（{}）：{}", file.display(), e))?;
    parse(&raw, profile).map(Some)
}

/// 写某个 Profile 的分类表：先校验，再原子替换。
pub fn save(app_root: &Path, profile: &str, categories: &[ThreadCategory]) -> Result<(), String> {
    settings::validate_categories(categories)?;
    let file = CategoryFile {
        schema: PROFILE_CATEGORIES_SCHEMA.to_string(),
        profile: profile.to_string(),
        categories: categories.to_vec(),
    };
    let text = serde_json::to_vec_pretty(&file)
        .map_err(|e| format!("序列化分类清单失败（{}）：{}", profile, e))?;
    crate::model_settings::write_atomic(&path(app_root, profile), &text)
}

/// 切换 Profile 时同步分类表。
///
/// 顺序是「先回存、再切入、后写设置」：任何一步失败都直接返回错误，调用方
/// 据此中止整次切换，避免出现「Profile 换了、分类还是旧的」。
pub fn apply_for_switch(
    paths: &AppPaths,
    leaving: Option<&str>,
    entering: Option<&str>,
    standard: &str,
) -> Result<SwitchCategories, String> {
    let current = settings::load(&paths.config)
        .map_err(|e| format!("读取当前分类表失败：{}", e))?
        .thread_categories;

    // 1. 切走前回存：用户在该 Profile 下的改名 / 新增不能因为换 Profile 就丢了。
    let saved_to = match leaving.map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) if name != standard => {
            save(&paths.root, name, &current)
                .map_err(|e| format!("Profile「{}」的分类表回存失败：{}", name, e))?;
            Some(name.to_string())
        }
        _ => None,
    };

    // 2. 决定切入后的分类表。
    let target_name = entering.map(str::trim).filter(|s| !s.is_empty());
    let (target, source) = match target_name {
        None => (settings::standard_thread_categories(), "standard"),
        Some(name) if name == standard => (settings::standard_thread_categories(), "standard"),
        Some(name) => match load(&paths.root, name)
            .map_err(|e| format!("Profile「{}」的分类表读取失败：{}", name, e))?
        {
            Some(categories) => (categories, "file"),
            None => match embedded_for(name) {
                Some(raw) => {
                    let categories = parse(raw, name)?;
                    save(&paths.root, name, &categories).map_err(|e| {
                        format!("Profile「{}」的分类表落盘失败：{}", name, e)
                    })?;
                    (categories, "embedded")
                }
                None => {
                    // 自定义 Profile 没写过分类：继承当前表（保留用户的 custom: 项），
                    // 立刻落盘，之后它就有自己的分类表了。
                    save(&paths.root, name, &current).map_err(|e| {
                        format!("Profile「{}」的分类表初始化失败：{}", name, e)
                    })?;
                    (current.clone(), "inherited")
                }
            },
        },
    };

    // 3. 写进当前生效设置。与现状一致时跳过写盘，减少无谓的文件改动。
    let categories = if target == current {
        target
    } else {
        settings::set_thread_categories(paths, target)?
    };

    Ok(SwitchCategories {
        categories,
        source,
        saved_to,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_app(tag: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "od-profile-categories-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("config")).unwrap();
        AppPaths::from_root(root)
    }

    fn app_root(paths: &AppPaths) -> &Path {
        &paths.root
    }

    #[test]
    fn missing_file_is_none_not_error() {
        let app = temp_app("missing");
        assert!(load(app_root(&app), "ghost").unwrap().is_none());
        assert!(!app.root.join("profiles").exists(), "只读不建目录");
    }

    #[test]
    fn broken_file_is_rejected() {
        let app = temp_app("broken");
        let file = path(app_root(&app), "discipline");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "{ not json").unwrap();
        let err = load(app_root(&app), "discipline").unwrap_err();
        assert!(err.contains("解析失败"), "{}", err);

        fs::write(
            &file,
            r#"{"schema":"other","profile":"discipline","categories":[]}"#,
        )
        .unwrap();
        let err = load(app_root(&app), "discipline").unwrap_err();
        assert!(err.contains("schema"), "{}", err);

        fs::write(
            &file,
            format!(
                r#"{{"schema":"{}","profile":"web","categories":[]}}"#,
                PROFILE_CATEGORIES_SCHEMA
            ),
        )
        .unwrap();
        let err = load(app_root(&app), "discipline").unwrap_err();
        assert!(err.contains("归属不符"), "{}", err);
    }

    #[test]
    fn switching_to_standard_leaves_only_plain_and_writes_no_profile_file() {
        let app = temp_app("standard");
        // 先切到 discipline：出厂清单 → 四类，并落盘。
        let first = apply_for_switch(&app, None, Some("discipline"), "omnidesk-base").unwrap();
        assert_eq!(first.source, "embedded");
        assert_eq!(first.categories.len(), 4);
        assert!(path(app_root(&app), "discipline").is_file());

        // 再切回基础标准：只剩未分类，且不再写任何 Profile 文件。
        let before = fs::read_to_string(path(app_root(&app), "discipline")).unwrap();
        let back = apply_for_switch(&app, Some("discipline"), None, "omnidesk-base").unwrap();
        assert_eq!(back.source, "standard");
        assert_eq!(back.saved_to.as_deref(), Some("discipline"));
        assert_eq!(back.categories.len(), 1);
        assert_eq!(back.categories[0].key, "plain");
        assert_eq!(
            fs::read_to_string(path(app_root(&app), "discipline")).unwrap(),
            before,
            "切走只回存，不改写内容"
        );
        assert!(!path(app_root(&app), "omnidesk-base").exists());

        let persisted = settings::load(&app.config).unwrap().thread_categories;
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].key, "plain");
    }

    #[test]
    fn user_edits_survive_a_round_trip() {
        let app = temp_app("roundtrip");
        apply_for_switch(&app, None, Some("discipline"), "omnidesk-base").unwrap();
        let mut edited = settings::load(&app.config).unwrap().thread_categories;
        edited[1].label = "本周记录".into();
        edited.push(ThreadCategory {
            key: "custom:reading".into(),
            label: "读书笔记".into(),
            folder: Some("reading".into()),
            vault: None,
        });
        settings::set_thread_categories(&app, edited.clone()).unwrap();

        apply_for_switch(&app, Some("discipline"), None, "omnidesk-base").unwrap();
        assert_eq!(settings::load(&app.config).unwrap().thread_categories.len(), 1);

        let back = apply_for_switch(&app, None, Some("discipline"), "omnidesk-base").unwrap();
        assert_eq!(back.source, "file");
        assert_eq!(back.categories, edited, "切回来必须原地恢复用户改过的表");
    }

    #[test]
    fn custom_profile_inherits_current_table_once() {
        let app = temp_app("inherit");
        apply_for_switch(&app, None, Some("discipline"), "omnidesk-base").unwrap();
        let first = apply_for_switch(&app, Some("discipline"), Some("mine"), "omnidesk-base").unwrap();
        assert_eq!(first.source, "inherited");
        assert_eq!(first.categories.len(), 4);
        assert!(path(app_root(&app), "mine").is_file(), "继承后要落盘");

        let second = apply_for_switch(&app, Some("mine"), Some("mine"), "omnidesk-base").unwrap();
        assert_eq!(second.source, "file");
    }
}

