//! 引擎 Profile 选择（OD-ARCH-072 / OD-ARCH-082 首页改版与白名单管控）。
//!
//! 范围：只决定「下一次启动引擎时用哪个 DSH Profile」，不做多 Profile 管理、
//! 不复制 Profile、不改插件与模型路由。选择结果只存 profile **名字**，
//! 落在应用数据根的 `config/settings.v1.json`（`engine_profile` 字段）。
//!
//! OD-ARCH-082 S6 白名单口径：
//!   * web-api 通道只列 `omnidesk-base`（默认）与 `discipline`。
//!   * acp 通道只列 `acp`。
//!   * 既有保存值（如 yvan-work）不在白名单时：**保留设置原值不变**，本次回落默认，
//!     并通过 `saved_out_of_whitelist: true` 显式告知前端做诚实提示。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（Rust 测试运行）。

use std::path::Path;

/// 一个可选 Profile。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProfileOption {
    /// Profile 目录名（也是 DSH 的 profile 名）。
    pub name: String,
    /// 界面显示名：优先取 package.json 的 `name`，否则用目录名。
    pub label: String,
    /// 该 Profile 是否是当前通道的默认（「标准模式」）。
    pub standard: bool,
}

pub const WEB_API_WHITELIST: &[&str] = &["omnidesk-base", "discipline"];
pub const ACP_WHITELIST: &[&str] = &["acp"];

/// 当前通道的白名单清单。
pub fn whitelist(channel: &str) -> &'static [&'static str] {
    if channel == "acp" {
        ACP_WHITELIST
    } else {
        WEB_API_WHITELIST
    }
}

/// 检查给定名字是否在当前通道的白名单中。
pub fn is_whitelisted(channel: &str, name: &str) -> bool {
    let trimmed = name.trim();
    whitelist(channel).contains(&trimmed)
}

/// 当前通道的默认 Profile 名（OD-ARCH-082：web-api 默认 omnidesk-base）。
pub fn standard_profile(channel: &str) -> &'static str {
    if channel == "acp" { "acp" } else { "omnidesk-base" }
}

/// 列出隔离 DSH_HOME 下受白名单约束且真实存在的 Profile。
///
/// 只读：不创建目录、不安装依赖。标准 Profile 即使目录缺失也会列出来。
pub fn friendly_label(name: &str) -> &'static str {
    match name {
        "omnidesk-base" => "基础标准",
        "discipline" => "纪检专业",
        "acp" => "ACP 协议通道",
        _ => "未知 Profile",
    }
}

/// 列出隔离 DSH_HOME 下受白名单约束且真实存在的 Profile。
///
/// 只读：以白名单为准返回固定友好名称（如「基础标准」「纪检专业」），
/// 不再从 package.json 的 name（如 dsh-profile-*）取名，避免技术包名泄漏到界面。
pub fn list(_dsh_home: &Path, channel: &str) -> Vec<ProfileOption> {
    let standard = standard_profile(channel);
    let allowed = whitelist(channel);
    let mut out: Vec<ProfileOption> = Vec::new();

    for &name in allowed {
        let label = friendly_label(name).to_string();
        out.push(ProfileOption {
            standard: name == standard,
            name: name.to_string(),
            label,
        });
    }
    out
}

/// 解析本次启动实际使用的 Profile：
///   * 用户没有选择（None）→ 通道标准 Profile；
///   * 用户选了但不在白名单内（如旧值 yvan-work）→ **保留设置原样，本次回落默认**；
///   * 用户选了且在白名单内，但目录/清单不存在 → 明确报错，不静默回退。
pub fn resolve(dsh_home: &Path, channel: &str, saved: Option<&str>) -> Result<String, String> {
    let Some(name) = saved.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(standard_profile(channel).to_string());
    };
    if !crate::settings::is_valid_profile_name(name) {
        return Err(format!("保存的 Profile 名不合法：{:?}", name));
    }
    // 白名单约束：不在白名单内的值不报错、不抹掉，回落默认 Profile
    if !is_whitelisted(channel, name) {
        return Ok(standard_profile(channel).to_string());
    }
    let manifest = dsh_home.join("profiles").join(name).join("package.json");
    if !manifest.is_file() {
        return Err(format!(
            "你选择的 Profile「{}」在隔离数据根里不存在（缺少 {}）。
请在顶栏 Profile 徽标里重新选择，或先创建该 Profile。",
            name,
            manifest.display()
        ));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "od-profile-choice-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn seed(root: &Path, name: &str, label: &str) {
        let dir = root.join("profiles").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            format!(r#"{{"name":"{}","dsh":{{"profile":{{}}}}}}"#, label),
        )
        .unwrap();
    }

    #[test]
    fn resolve_defaults_to_standard_profile() {
        let root = temp_root("default");
        assert_eq!(resolve(&root, "web-api", None).unwrap(), "omnidesk-base");
        assert_eq!(resolve(&root, "acp", None).unwrap(), "acp");
        assert_eq!(resolve(&root, "web-api", Some("  ")).unwrap(), "omnidesk-base");
    }

    #[test]
    fn resolve_falls_back_for_out_of_whitelist_saved_profile() {
        let root = temp_root("out-of-whitelist");
        seed(&root, "yvan-work", "dsh-profile-yvan");
        // yvan-work 真实存在但在白名单外，回落到默认 omnidesk-base
        assert_eq!(resolve(&root, "web-api", Some("yvan-work")).unwrap(), "omnidesk-base");
    }

    #[test]
    fn resolve_rejects_missing_profile_when_in_whitelist() {
        let root = temp_root("missing");
        let err = resolve(&root, "web-api", Some("discipline")).unwrap_err();
        assert!(err.contains("discipline"), "错误里要带 Profile 名：{err}");
        assert!(err.contains("不存在"), "错误要说清是不存在：{err}");
    }

    #[test]
    fn resolve_accepts_existing_whitelisted_profile() {
        let root = temp_root("exists");
        seed(&root, "discipline", "dsh-profile-discipline");
        assert_eq!(&resolve(&root, "web-api", Some("discipline")).unwrap(), "discipline");
    }

    #[test]
    fn list_includes_whitelisted_only_and_uses_friendly_labels() {
        let root = temp_root("list");
        seed(&root, "omnidesk-base", "dsh-profile-omnidesk-base");
        seed(&root, "discipline", "dsh-profile-discipline");
        seed(&root, "yvan-work", "dsh-profile-yvan");
        let items = list(&root, "web-api");
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["omnidesk-base", "discipline"]);
        assert!(!names.contains(&"yvan-work"), "yvan-work 不应出现在白名单列表中");

        let base = items.iter().find(|i| i.name == "omnidesk-base").unwrap();
        assert!(base.standard);
        assert_eq!(base.label, "基础标准", "即便 package.json 是 dsh-profile-*，label 必须是友好名");

        let disc = items.iter().find(|i| i.name == "discipline").unwrap();
        assert!(!disc.standard);
        assert_eq!(disc.label, "纪检专业");

        for item in &items {
            assert!(!item.label.contains("dsh-profile-"), "友好名不得包含 dsh-profile-：{}", item.label);
        }
    }
}

