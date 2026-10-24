//! 能力管理（OD-ARCH-072）——Skill 与插件的一处集中入口。
//!
//! 边界（Plan 影响范围）：
//!   * Skill 只读：扫 <隔离 DSH_HOME>/skills/<name>/SKILL.md 的 frontmatter，
//!     不执行、不改写、不伪造版本号（Skill 目录本身没有版本字段）。
//!   * 插件只动 <隔离 DSH_HOME>/profiles/<profile>/package.json 的
//!     dsh.profile.bundles 数组：启停 = 数组增删，dependencies 原样保留。
//!   * 写前路径硬拒绝（含 /.dsh、/.agents、deepseek-harness、仓库内），
//!     写前自动 .bak-<epoch>，写入走 0600 临时文件 + fsync + rename。
//!   * 「接入 DSH 插件市场」只是在 bundles 里加入 dshmarket；浏览/安装/更新
//!     仍由 DSH 原生界面承担，本模块不复制其功能，也不联网安装。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（Rust 测试运行）。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 与 seed_profile_if_missing 同一套被禁止的数据根片段。
const FORBIDDEN_FRAGMENTS: [&str; 3] = ["/.dsh", "/.agents", "deepseek-harness"];
/// DSH 原生插件市场包名（版本对齐用户 desktop profile 的 ^1.66.35）。
pub const MARKET_PACKAGE: &str = "dshmarket";
pub const MARKET_VERSION_HINT: &str = "^1.66.35";
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_SKILL_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SkillEntry {
    pub name: String,
    /// frontmatter 里的 description 折叠成一行；没有就为 null（不编造）。
    pub description: Option<String>,
    /// 最近修改时间（Unix 秒）；读不到为 null。
    pub modified_at: Option<u64>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PluginEntry {
    pub name: String,
    /// dependencies 里登记的版本；平台自带 bundle 没有版本号，为 null。
    pub version: Option<String>,
    /// 是否在 dsh.profile.bundles 里（= 下次启动会加载）。
    pub enabled: bool,
    /// 是否属于 DSH 平台自带（@deepseek-ai/*），不需要单独安装。
    pub builtin: bool,
    /// 第三方包是否已在 profile 的 node_modules 里。
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CapabilityView {
    pub profile_dir: String,
    pub manifest_path: String,
    pub skills_dir: String,
    pub skills: Vec<SkillEntry>,
    pub plugins: Vec<PluginEntry>,
    /// 市场包是否已安装（未安装时开关必须保持关闭并如实说明）。
    pub market_installed: bool,
    pub market_enabled: bool,
    pub market_version: Option<String>,
    /// 只读区域读取过程中的问题（例如某个 SKILL.md 读不了），不影响插件区。
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CapabilityReceipt {
    pub ok: bool,
    pub path: String,
    pub backup: Option<String>,
    pub note: String,
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().to_lowercase().replace('\\', "/")
}

/// 隔离根硬拒绝：与出厂播种同一条规则，另加「不得落在仓库内」。
pub fn assert_writable_root(dsh_home: &Path) -> Result<(), String> {
    if !dsh_home.is_absolute() {
        return Err(format!("隔离 DSH_HOME 必须是绝对路径：{}", dsh_home.display()));
    }
    let text = normalize(dsh_home);
    for forbidden in FORBIDDEN_FRAGMENTS {
        if text.contains(forbidden) {
            return Err(format!(
                "拒绝写入被禁止的数据根（含 {:?}）：{}",
                forbidden,
                dsh_home.display()
            ));
        }
    }
    if let Ok(repo) = crate::task_shell::repo_root_hint() {
        let repo_text = normalize(&repo);
        if text == repo_text || text.starts_with(&format!("{}/", repo_text)) {
            return Err(format!("拒绝把运行配置写进仓库：{}", dsh_home.display()));
        }
    }
    Ok(())
}

/// Profile 目录 = <隔离 DSH_HOME>/profiles/<profile>；profile 名必须是一个普通名字。
pub fn profile_dir(dsh_home: &Path, profile: &str) -> Result<PathBuf, String> {
    assert_writable_root(dsh_home)?;
    let trimmed = profile.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(format!("Profile 名称不合法：{:?}", profile));
    }
    Ok(dsh_home.join("profiles").join(trimmed))
}

pub fn manifest_path(dir: &Path) -> PathBuf {
    dir.join("package.json")
}

fn load_manifest(dir: &Path) -> Result<serde_json::Value, String> {
    let path = manifest_path(dir);
    let meta = fs::metadata(&path)
        .map_err(|e| format!("读取 Profile 配置失败（{}）：{}", path.display(), e))?;
    if meta.len() > MAX_MANIFEST_BYTES {
        return Err(format!("Profile 配置过大（上限 256 KB）：{}", path.display()));
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("读取 Profile 配置失败（{}）：{}", path.display(), e))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Profile 配置不是合法 JSON（{}）：{}", path.display(), e))?;
    bundles(&value)?;
    Ok(value)
}

fn bundles(value: &serde_json::Value) -> Result<&Vec<serde_json::Value>, String> {
    value
        .get("dsh")
        .and_then(|d| d.get("profile"))
        .and_then(|p| p.get("bundles"))
        .and_then(|b| b.as_array())
        .ok_or_else(|| "Profile 配置缺少 dsh.profile.bundles 数组，未改动任何文件".to_string())
}

fn bundles_mut(value: &mut serde_json::Value) -> Result<&mut Vec<serde_json::Value>, String> {
    value
        .get_mut("dsh")
        .and_then(|d| d.get_mut("profile"))
        .and_then(|p| p.get_mut("bundles"))
        .and_then(|b| b.as_array_mut())
        .ok_or_else(|| "Profile 配置缺少 dsh.profile.bundles 数组，未改动任何文件".to_string())
}

fn bundle_names(value: &serde_json::Value) -> Result<Vec<String>, String> {
    Ok(bundles(value)?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect())
}

fn dependency_version(value: &serde_json::Value, name: &str) -> Option<String> {
    value
        .get("dependencies")
        .and_then(|d| d.get(name))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn backup_of(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("package.json");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut target = path.with_file_name(format!("{}.bak-{}", file_name, stamp));
    let mut n = 1;
    while target.exists() {
        target = path.with_file_name(format!("{}.bak-{}-{}", file_name, stamp, n));
        n += 1;
    }
    fs::copy(path, &target)
        .map_err(|e| format!("备份 Profile 配置失败（{}）：{}", target.display(), e))?;
    Ok(Some(target))
}

fn write_manifest(
    dir: &Path,
    value: &serde_json::Value,
    note: &str,
) -> Result<CapabilityReceipt, String> {
    let path = manifest_path(dir);
    let backup = backup_of(&path)?;
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| format!("序列化 Profile 配置失败：{}", e))?;
    bytes.push(b'\n');
    crate::model_settings::write_atomic(&path, &bytes)?;
    Ok(CapabilityReceipt {
        ok: true,
        path: path.display().to_string(),
        backup: backup.map(|p| p.display().to_string()),
        note: note.to_string(),
    })
}

/// 启用/停用某个 bundle。停用只从 bundles 移除，dependencies 保留（便于回滚）。
pub fn set_bundle(dir: &Path, name: &str, enabled: bool) -> Result<CapabilityReceipt, String> {
    let name = name.trim();
    let scoped_ok = name.starts_with("@") && name.split('/').count() == 2;
    if name.is_empty()
        || name.contains("..")
        || (name.contains('/') && !scoped_ok)
        || name.contains('\\')
    {
        return Err(format!("插件名不合法：{:?}", name));
    }
    let mut value = load_manifest(dir)?;
    let installed = name.starts_with("@deepseek-ai/")
        || dir.join("node_modules").join(name).join("package.json").is_file();
    let current = bundle_names(&value)?;
    let present = current.iter().any(|n| n == name);
    if enabled && !present {
        if !installed {
            if name == MARKET_PACKAGE {
                return Err(format!(
                    "插件市场包还没装到这个 OmniDesk 实例里（{} 不存在）。请先在隔离环境安装 {} {}，再打开开关；本次没有改文件。",
                    dir.join("node_modules").join(MARKET_PACKAGE).display(),
                    MARKET_PACKAGE,
                    MARKET_VERSION_HINT
                ));
            }
            if dependency_version(&value, name).is_none() {
                return Err(format!("{} 既不在 node_modules 也没有依赖记录，未改动文件", name));
            }
            return Err(format!(
                "{} 已登记但尚未安装（node_modules 里没有），现在启用会导致 DSH 启动失败；本次没有改文件。",
                name
            ));
        }
        let list = bundles_mut(&mut value)?;
        list.push(serde_json::Value::String(name.to_string()));
        return write_manifest(
            dir,
            &value,
            &format!("已把 {} 加回加载列表；重启应用（引擎会重启）后生效。", name),
        );
    }
    if !enabled && present {
        let list = bundles_mut(&mut value)?;
        list.retain(|v| v.as_str() != Some(name));
        return write_manifest(
            dir,
            &value,
            &format!(
                "已从加载列表移除 {}（依赖记录保留，可随时加回）；重启应用后生效。",
                name
            ),
        );
    }
    Ok(CapabilityReceipt {
        ok: true,
        path: manifest_path(dir).display().to_string(),
        backup: None,
        note: format!("{} 已经是目标状态，没有改文件。", name),
    })
}

/// 解析 SKILL.md 的 frontmatter（只读文本，不执行）。
pub fn parse_skill_frontmatter(text: &str) -> Option<(String, Option<String>)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut name = String::new();
    let mut description: Option<String> = None;
    let mut key: Option<String> = None;
    let mut block: Vec<String> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            flush_block(&key, &mut block, &mut name, &mut description);
            break;
        }
        if trimmed.is_empty() {
            continue;
        }
        let top_level = !line.starts_with(' ') && !line.starts_with('\t');
        if top_level {
            if let Some((k, v)) = trimmed.split_once(':') {
                flush_block(&key, &mut block, &mut name, &mut description);
                let k = k.trim().to_string();
                let v = v.trim();
                if !v.is_empty() && !matches!(v, ">" | "|" | ">-" | "|-") {
                    if k == "name" {
                        name = v.to_string();
                    } else if k == "description" {
                        description = Some(collapse(v));
                    }
                    key = None;
                } else {
                    key = Some(k);
                }
            }
            continue;
        }
        if key.is_some() {
            block.push(trimmed.to_string());
        }
    }
    if name.is_empty() && description.is_none() {
        return None;
    }
    Some((name, description))
}

fn flush_block(
    key: &Option<String>,
    block: &mut Vec<String>,
    name: &mut String,
    description: &mut Option<String>,
) {
    if let Some(k) = key {
        if !block.is_empty() {
            let joined = collapse(&block.join(" "));
            if k == "name" {
                *name = joined;
            } else if k == "description" {
                *description = Some(joined);
            }
        }
    }
    block.clear();
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn list_skills(dsh_home: &Path, notes: &mut Vec<String>) -> Vec<SkillEntry> {
    let dir = dsh_home.join("skills");
    let mut out = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => {
            notes.push(format!("Skill 目录不存在或不可读：{}", dir.display()));
            return out;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        let fallback = entry.file_name().to_string_lossy().to_string();
        let (name, description) = match fs::metadata(&skill_file) {
            Ok(meta) if meta.len() <= MAX_SKILL_BYTES => match fs::read_to_string(&skill_file) {
                Ok(text) => match parse_skill_frontmatter(&text) {
                    Some((n, d)) => (if n.is_empty() { fallback.clone() } else { n }, d),
                    None => {
                        notes.push(format!(
                            "{} 没有可识别的 frontmatter，按目录名显示",
                            skill_file.display()
                        ));
                        (fallback.clone(), None)
                    }
                },
                Err(e) => {
                    notes.push(format!("读取 {} 失败：{}", skill_file.display(), e));
                    (fallback.clone(), None)
                }
            },
            _ => (fallback.clone(), None),
        };
        let modified_at = fs::metadata(&skill_file)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        out.push(SkillEntry {
            name,
            description,
            modified_at,
            path: path.display().to_string(),
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn load(dsh_home: &Path, profile: &str) -> Result<CapabilityView, String> {
    let dir = profile_dir(dsh_home, profile)?;
    let mut notes = Vec::new();
    let skills = list_skills(dsh_home, &mut notes);
    let value = load_manifest(&dir)?;
    let enabled = bundle_names(&value)?;
    let mut plugins: Vec<PluginEntry> = Vec::new();
    for name in &enabled {
        let builtin = name.starts_with("@deepseek-ai/");
        let installed = builtin
            || dir
                .join("node_modules")
                .join(name)
                .join("package.json")
                .is_file();
        plugins.push(PluginEntry {
            name: name.clone(),
            version: dependency_version(&value, name),
            enabled: true,
            builtin,
            installed,
        });
    }
    if let Some(deps) = value.get("dependencies").and_then(|d| d.as_object()) {
        let mut disabled: Vec<PluginEntry> = deps
            .keys()
            .filter(|k| !enabled.iter().any(|e| e == *k))
            .map(|k| PluginEntry {
                name: k.clone(),
                version: dependency_version(&value, k),
                enabled: false,
                builtin: false,
                installed: dir.join("node_modules").join(k).join("package.json").is_file(),
            })
            .collect();
        disabled.sort_by(|a, b| a.name.cmp(&b.name));
        plugins.extend(disabled);
    }
    let market_dir = dir.join("node_modules").join(MARKET_PACKAGE);
    let market_installed = market_dir.join("package.json").is_file();
    let market_version = if market_installed {
        fs::read_to_string(market_dir.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("version").and_then(|x| x.as_str()).map(|s| s.to_string()))
    } else {
        None
    };
    let market_enabled = enabled.iter().any(|n| n == MARKET_PACKAGE);
    Ok(CapabilityView {
        profile_dir: dir.display().to_string(),
        manifest_path: manifest_path(&dir).display().to_string(),
        skills_dir: dsh_home.join("skills").display().to_string(),
        skills,
        plugins,
        market_installed,
        market_enabled,
        market_version,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-capabilities/{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let dir = base.join("dsh-home").join("profiles").join("web");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("package.json"),
            r#"{"name":"dsh-profile-web","private":true,"dependencies":{"dsh-cost-meter":"1.9.6","dsh-free-search":"0.9.5"},"dsh":{"profile":{"bundles":["@deepseek-ai/dsh-base","@deepseek-ai/dsh-web-app","dsh-free-search","dsh-cost-meter"]}}}"#,
        )
        .unwrap();
        for dep in ["dsh-cost-meter", "dsh-free-search"] {
            fs::create_dir_all(dir.join("node_modules").join(dep)).unwrap();
            fs::write(dir.join("node_modules").join(dep).join("package.json"), "{}").unwrap();
        }
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn disable_keeps_dependency_and_backs_up() {
        let dir = fixture("disable");
        let receipt = set_bundle(&dir, "dsh-cost-meter", false).expect("停用应成功");
        assert!(receipt.backup.is_some(), "停用必须留备份");
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
        let names = bundle_names(&value).unwrap();
        assert!(!names.iter().any(|n| n == "dsh-cost-meter"));
        assert_eq!(
            dependency_version(&value, "dsh-cost-meter").as_deref(),
            Some("1.9.6"),
            "dependencies 必须原样保留"
        );
        cleanup(&dir);
    }

    #[test]
    fn enable_restores_bundle_once() {
        let dir = fixture("enable");
        set_bundle(&dir, "dsh-cost-meter", false).unwrap();
        set_bundle(&dir, "dsh-cost-meter", true).unwrap();
        let again = set_bundle(&dir, "dsh-cost-meter", true).unwrap();
        assert!(again.backup.is_none(), "已是目标状态不应重复写");
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
        let count = bundle_names(&value)
            .unwrap()
            .iter()
            .filter(|n| n.as_str() == "dsh-cost-meter")
            .count();
        assert_eq!(count, 1);
        cleanup(&dir);
    }

    #[test]
    fn market_requires_installed_package() {
        let dir = fixture("market");
        let err = set_bundle(&dir, MARKET_PACKAGE, true).unwrap_err();
        assert!(err.contains("还没装"), "未安装时不能只改 bundles：{}", err);
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
        assert!(!bundle_names(&value).unwrap().iter().any(|n| n == MARKET_PACKAGE));
        cleanup(&dir);
    }

    #[test]
    fn forbidden_roots_are_rejected() {
        for bad in [
            "/home/u/.dsh/x",
            "/home/u/.agents",
            "/x/deepseek-harness/runtime",
        ] {
            assert!(profile_dir(Path::new(bad), "web").is_err(), "{} 应被拒绝", bad);
        }
        assert!(profile_dir(Path::new("/home/u/OmniDesk/runtime/dsh-home"), "../etc").is_err());
        assert!(profile_dir(Path::new("/home/u/OmniDesk/runtime/dsh-home"), "web").is_ok());
    }

    #[test]
    fn frontmatter_parses_name_and_wrapped_description() {
        let text = "---\nname: material-writing\ndescription: >\n  第一句。\n  第二句。\nuser-invocable: true\n---\n\n# 标题\n";
        let (name, description) = parse_skill_frontmatter(text).expect("应能解析");
        assert_eq!(name, "material-writing");
        assert_eq!(description.as_deref(), Some("第一句。 第二句。"));
        assert!(parse_skill_frontmatter("# 没有 frontmatter").is_none());
    }

    #[test]
    fn list_reads_all_skill_dirs() {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-capabilities/list-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("skills/a")).unwrap();
        fs::create_dir_all(base.join("skills/b")).unwrap();
        fs::write(
            base.join("skills/a/SKILL.md"),
            "---\nname: a\ndescription: 甲\n---\n",
        )
        .unwrap();
        fs::write(
            base.join("skills/b/SKILL.md"),
            "---\nname: b\ndescription: 乙\n---\n",
        )
        .unwrap();
        let mut notes = Vec::new();
        let skills = list_skills(&base, &mut notes);
        assert_eq!(skills.len(), 2);
        assert!(skills[0].modified_at.is_some());
        let _ = fs::remove_dir_all(&base);
    }
}

