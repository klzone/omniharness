//! 引擎 Profile 选择（OD-ARCH-072 首页改版的 Profile 徽标）。
//!
//! 范围：只决定「下一次启动引擎时用哪个 DSH Profile」，不做多 Profile 管理、
//! 不复制 Profile、不改插件与模型路由。选择结果只存 profile **名字**，
//! 落在应用数据根的 `config/settings.v1.json`（`engine_profile` 字段）。
//!
//! 为什么存名字而不是路径：Profile 目录固定在隔离 DSH_HOME 下，存路径会在
//! 数据根变化后失效；名字还能在下次启动时核对目录是否真实存在。
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

/// 当前通道的默认 Profile 名。
pub fn standard_profile(channel: &str) -> &'static str {
    if channel == "acp" { "acp" } else { "web" }
}

/// 列出隔离 DSH_HOME 下真实存在的 Profile（含 `package.json` 的目录）。
///
/// 只读：不创建目录、不安装依赖。标准 Profile 即使目录缺失也会列出来，
/// 但 `available=false` 由界面按存在性提示，不伪造可用。
pub fn list(dsh_home: &Path, channel: &str) -> Vec<ProfileOption> {
    let standard = standard_profile(channel);
    let mut out: Vec<ProfileOption> = Vec::new();
    let dir = dsh_home.join("profiles");
    let mut names: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || !path.join("package.json").is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if crate::settings::is_valid_profile_name(name) {
                names.push(name.to_string());
            }
        }
    }
    if !names.iter().any(|n| n == standard) {
        names.push(standard.to_string());
    }
    names.sort();
    for name in names {
        let manifest = dir.join(&name).join("package.json");
        let label = std::fs::read_to_string(&manifest)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string))
            .unwrap_or_else(|| name.clone());
        out.push(ProfileOption {
            standard: name == standard,
            name,
            label,
        });
    }
    out
}

/// 解析本次启动实际使用的 Profile：
///   * 用户没有选择（None）→ 通道标准 Profile；
///   * 用户选了，但目录/清单不存在 → 明确报错，不静默回退（否则界面显示与实际不符）。
pub fn resolve(dsh_home: &Path, channel: &str, saved: Option<&str>) -> Result<String, String> {
    let Some(name) = saved.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(standard_profile(channel).to_string());
    };
    if !crate::settings::is_valid_profile_name(name) {
        return Err(format!("保存的 Profile 名不合法：{:?}", name));
    }
    let manifest = dsh_home.join("profiles").join(name).join("package.json");
    if !manifest.is_file() {
        return Err(format!(
            "你选择的 Profile「{}」在隔离数据根里不存在（缺少 {}）。\
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
            format!("{{\"name\":\"{}\",\"dsh\":{{\"profile\":{{}}}}}}", label),
        )
        .unwrap();
    }

    #[test]
    fn resolve_defaults_to_standard_profile() {
        let root = temp_root("default");
        assert_eq!(resolve(&root, "web-api", None).unwrap(), "web");
        assert_eq!(resolve(&root, "acp", None).unwrap(), "acp");
        assert_eq!(resolve(&root, "web-api", Some("  ")).unwrap(), "web");
    }

    #[test]
    fn resolve_rejects_missing_profile_instead_of_falling_back() {
        let root = temp_root("missing");
        let err = resolve(&root, "web-api", Some("yvan-work")).unwrap_err();
        assert!(err.contains("yvan-work"), "错误里要带 Profile 名：{err}");
        assert!(err.contains("不存在"), "错误要说清是不存在：{err}");
    }

    #[test]
    fn resolve_accepts_existing_profile() {
        let root = temp_root("exists");
        seed(&root, "yvan-work", "dsh-profile-yvan");
        assert_eq!(&resolve(&root, "web-api", Some("yvan-work")).unwrap(), "yvan-work");
    }

    #[test]
    fn list_includes_standard_and_real_dirs_only() {
        let root = temp_root("list");
        seed(&root, "web", "dsh-profile-web");
        seed(&root, "yvan-work", "dsh-profile-yvan");
        std::fs::create_dir_all(root.join("profiles").join("no-manifest")).unwrap();
        std::fs::create_dir_all(root.join("profiles").join("Bad Name")).unwrap();
        let items = list(&root, "web-api");
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"web"), "标准 Profile 应出现：{names:?}");
        assert!(names.contains(&"yvan-work"));
        assert!(!names.contains(&"no-manifest"), "没有 package.json 的目录不是 Profile");
        assert!(!names.contains(&"Bad Name"), "非法名字不列出");
        let web = items.iter().find(|i| i.name == "web").unwrap();
        assert!(web.standard);
        assert_eq!(web.label, "dsh-profile-web");
    }

    #[test]
    fn list_always_offers_standard_even_when_absent() {
        let root = temp_root("absent");
        let items = list(&root, "acp");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "acp");
        assert!(items[0].standard);
    }
}
