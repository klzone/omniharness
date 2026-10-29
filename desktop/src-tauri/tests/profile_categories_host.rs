//! 对话分类随 Profile 走的宿主级集成测试（OD-ARCH-116）。
//!
//! 只用 /tmp 下的合成根：基础标准干净、专业 Profile 带回四类、用户改动往返不丢、
//! 损坏清单被拒绝且不改写设置。

use std::fs;
use std::path::PathBuf;

use omnidesk_desktop_lib::app_paths::AppPaths;
use omnidesk_desktop_lib::profile_categories;
use omnidesk_desktop_lib::settings::{self, ThreadCategory};

const STANDARD: &str = "omnidesk-base";

struct Fixture {
    base: PathBuf,
    paths: AppPaths,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-profile-categories-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("config")).unwrap();
        let paths = AppPaths::from_root(&base);
        Self { base, paths }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn category_file(fx: &Fixture, profile: &str) -> PathBuf {
    fx.paths.root.join("profiles").join(profile).join("categories.json")
}

#[test]
fn standard_after_discipline_keeps_only_plain() {
    let fx = Fixture::new("standard");
    let first =
        profile_categories::apply_for_switch(&fx.paths, None, Some("discipline"), STANDARD).unwrap();
    assert_eq!(first.source, "embedded");
    assert_eq!(first.categories.len(), 4);
    let weekly = first
        .categories
        .iter()
        .find(|c| c.key == "weekly")
        .expect("工作记录分类");
    assert_eq!(weekly.folder.as_deref(), Some("weekly-report"));

    let back =
        profile_categories::apply_for_switch(&fx.paths, Some("discipline"), None, STANDARD).unwrap();
    assert_eq!(back.source, "standard");
    assert_eq!(back.saved_to.as_deref(), Some("discipline"));
    assert_eq!(back.categories.len(), 1);
    assert_eq!(back.categories[0].key, "plain");

    // 基础标准本身不产生任何 Profile 文件。
    assert!(!category_file(&fx, STANDARD).exists());
    // 设置文件里也确实只剩这一类（重启后读回来的就是它）。
    let persisted = settings::load(&fx.paths.config).unwrap().thread_categories;
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].key, "plain");
}

#[test]
fn user_edits_survive_switching_away_and_back() {
    let fx = Fixture::new("roundtrip");
    profile_categories::apply_for_switch(&fx.paths, None, Some("discipline"), STANDARD).unwrap();

    let mut edited = settings::load(&fx.paths.config).unwrap().thread_categories;
    let weekly = edited.iter_mut().find(|c| c.key == "weekly").unwrap();
    weekly.label = "本周记录".to_string();
    edited.push(ThreadCategory {
        key: "custom:reading".into(),
        label: "读书笔记".into(),
        folder: Some("reading".into()),
        vault: None,
    });
    settings::set_thread_categories(&fx.paths, edited.clone()).unwrap();

    profile_categories::apply_for_switch(&fx.paths, Some("discipline"), None, STANDARD).unwrap();
    assert_eq!(
        settings::load(&fx.paths.config).unwrap().thread_categories.len(),
        1
    );

    let back =
        profile_categories::apply_for_switch(&fx.paths, None, Some("discipline"), STANDARD).unwrap();
    assert_eq!(back.source, "file");
    assert_eq!(back.categories, edited, "用户改名与自定义分类必须原样回来");
}

#[test]
fn broken_category_file_is_rejected_without_touching_settings() {
    let fx = Fixture::new("broken");
    let file = category_file(&fx, "discipline");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "{ broken").unwrap();

    let before = fs::read_to_string(fx.paths.config.join("settings.v1.json")).ok();
    let err =
        profile_categories::apply_for_switch(&fx.paths, None, Some("discipline"), STANDARD)
            .unwrap_err();
    assert!(err.contains("解析失败"), "{}", err);
    let after = fs::read_to_string(fx.paths.config.join("settings.v1.json")).ok();
    assert_eq!(before, after, "切换失败不得改写设置文件");
}

#[test]
fn custom_profile_inherits_once_and_keeps_its_own_copy() {
    let fx = Fixture::new("custom");
    profile_categories::apply_for_switch(&fx.paths, None, Some("discipline"), STANDARD).unwrap();
    let inherited =
        profile_categories::apply_for_switch(&fx.paths, Some("discipline"), Some("mine"), STANDARD)
            .unwrap();
    assert_eq!(inherited.source, "inherited");
    assert!(category_file(&fx, "mine").is_file());

    // 在 mine 里删掉一个业务分类，再往返一次：不能因为继承而复活。
    let mut trimmed = settings::load(&fx.paths.config).unwrap().thread_categories;
    trimmed.retain(|c| c.key != "project");
    settings::set_thread_categories(&fx.paths, trimmed.clone()).unwrap();

    profile_categories::apply_for_switch(&fx.paths, Some("mine"), None, STANDARD).unwrap();
    let back =
        profile_categories::apply_for_switch(&fx.paths, None, Some("mine"), STANDARD).unwrap();
    assert_eq!(back.source, "file");
    assert_eq!(back.categories, trimmed);
    assert!(!back.categories.iter().any(|c| c.key == "project"));
}

