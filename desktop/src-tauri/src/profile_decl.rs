//! 纪检 Profile 声明（omnidesk.profile-decl.v1）——OD-ARCH-051 S3。
//!
//! 只声明**界面层面**的内容：Profile 名、Dock 上要显示哪些业务场景入口、它们的顺序。
//! 它不加载插件、不改模型路由、不改 Skill 清单、不改任何判定逻辑。
//!
//! 为什么是文件而无代码：计划要求「改声明文件、不动代码，场景入口可见变化」。
//! 因此加载失败必须能明确提示并回退到内置默认，而不是让界面空白或静默吞掉。
//!
//! 落点（OD-ARCH-106）：`<应用数据根>/profiles/<Profile 名>/decl.yml`（仓库外）。
//! 声明**随 Profile 挂载**：基础标准（omnidesk-base）下没有这个文件，界面只有平台固有
//! 入口；切到专业 Profile 时才有业务场景。文件不由宿主自动创建——没有就是没有，
//! 这样「基础标准 = 干净底座」才是可复核的事实。
//!
//! 解析方式：**不引入 YAML 依赖**（本仓库离线构建，锁文件里没有 YAML crate）。
//! 改为解析这份声明所需的受限子集：顶层 `key: value`、`scenes:` 下的列表项、
//! 列表项内的两级缩进键值。遇到超出子集的写法直接报错，不猜、不静默忽略。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（Rust 测试运行）。

use std::fs;
use std::path::{Path, PathBuf};

use crate::sha256::sha256_hex;

/// 声明文件 schema 标识。版本不符一律拒绝。
pub const PROFILE_DECL_SCHEMA: &str = "omnidesk.profile-decl.v1";
/// 声明文件名（位于 `<应用数据根>/profiles/<Profile 名>/` 下）。
pub const PROFILE_DECL_FILE: &str = "decl.yml";
/// OD-ARCH-106 之前的单文件落点，只用于一次性兼容迁移（不删除、不覆盖）。
pub const LEGACY_PROFILE_DECL_FILE: &str = "discipline.yml";
/// 允许出现在 Dock 上的场景标识。不在表里的一律拒绝——声明不能凭空造入口。
pub const ALLOWED_SCENES: &[&str] = &["home", "report", "writing", "project", "sessions"];
/// 每个场景可选的图标名（取值与前端 `IconName` 对齐，前端另有校验）。
pub const ALLOWED_ICONS: &[&str] = &["brand", "note", "book", "checklist", "history"];
/// 单个字段最大字符数。
const MAX_FIELD_CHARS: usize = 64;
/// 场景条目上限，防止异常文件把 Dock 撑爆。
const MAX_SCENES: usize = 12;

/// 仓库内默认声明（专业 Profile 的模板）。编译期嵌入；仅供显式写入使用，不自动落盘。
pub const DEFAULT_DECL: &str = include_str!("../../../samples/profiles/discipline.yml");

/// Dock 上一个业务场景入口。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SceneEntry {
    /// 场景标识，必须在 ALLOWED_SCENES 内。
    pub id: String,
    /// 界面上显示的名字。
    pub label: String,
    /// 图标名，必须在 ALLOWED_ICONS 内。
    pub icon: String,
    /// 旁边的小字提示（可为空）。
    #[serde(default)]
    pub note: String,
}

/// 解析后的声明。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProfileDecl {
    pub schema: String,
    /// Profile 显示名（Dock 与窗口标题用）。
    pub display_name: String,
    /// Dock 上按此顺序显示的场景入口。
    pub scenes: Vec<SceneEntry>,
}

impl ProfileDecl {
    /// 校验：schema 一致、至少一个场景、字段非空且有界、标识与图标在白名单内、无重复。
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PROFILE_DECL_SCHEMA {
            return Err(format!(
                "schema 必须是 {}，实际是 {}",
                PROFILE_DECL_SCHEMA, self.schema
            ));
        }
        if self.display_name.trim().is_empty() {
            return Err("display_name 不能为空".to_string());
        }
        if self.display_name.chars().count() > MAX_FIELD_CHARS {
            return Err(format!("display_name 超过 {MAX_FIELD_CHARS} 个字符"));
        }
        if self.scenes.is_empty() {
            return Err("scenes 不能为空；至少保留一个入口".to_string());
        }
        if self.scenes.len() > MAX_SCENES {
            return Err(format!("scenes 最多 {MAX_SCENES} 项"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (idx, scene) in self.scenes.iter().enumerate() {
            let at = format!("scenes[{idx}]");
            if !ALLOWED_SCENES.contains(&scene.id.as_str()) {
                return Err(format!(
                    "{at}.id「{}」不在允许列表内（可选：{}）",
                    scene.id,
                    ALLOWED_SCENES.join(" / ")
                ));
            }
            if !seen.insert(scene.id.clone()) {
                return Err(format!("{at}.id「{}」重复", scene.id));
            }
            if scene.label.trim().is_empty() {
                return Err(format!("{at}.label 不能为空"));
            }
            if scene.label.chars().count() > MAX_FIELD_CHARS
                || scene.note.chars().count() > MAX_FIELD_CHARS
            {
                return Err(format!("{at} 的 label/note 超过 {MAX_FIELD_CHARS} 个字符"));
            }
            if !ALLOWED_ICONS.contains(&scene.icon.as_str()) {
                return Err(format!(
                    "{at}.icon「{}」不在允许列表内（可选：{}）",
                    scene.icon,
                    ALLOWED_ICONS.join(" / ")
                ));
            }
            if scene.label.contains(['\n', '\r']) || scene.note.contains(['\n', '\r']) {
                return Err(format!("{at} 的 label/note 不能换行"));
            }
        }
        Ok(())
    }
}

/// 已加载的声明及其来源。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoadedDecl {
    pub decl: ProfileDecl,
    pub path: String,
    /// 本次载入时文件内容的 SHA-256。用于判断文件在运行期间是否被改动，
    /// 从而提示"重启应用生效"（OD-ARCH-052 第 2 步）。只做比对，不做热重载。
    pub content_sha256: String,
    /// 声明原文（供设置页内嵌编辑框载入；只读展示，保存仍走 `profile_decl_save` 校验）。
    pub text: String,
}

/// 只读探测：当前 Profile 目录下声明文件的内容指纹。
///
/// - 文件不存在 → `Ok(None)`（基础标准下就是没有声明，不是错误）。
/// - 读取失败 → `Err`（不猜内容）。
///
/// 刻意**不**创建文件、不改解析逻辑：这条路径只服务于"是否已经改了"的判断。
pub fn decl_digest(profile_dir: &Path) -> Result<Option<String>, String> {
    if !profile_dir.is_absolute() {
        return Err(format!(
            "Profile 目录必须是绝对路径：{}",
            profile_dir.display()
        ));
    }
    let path = decl_path(profile_dir);
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(sha256_hex(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!(
            "无法读取 Profile 声明（{}）：{}",
            path.display(),
            e
        )),
    }
}

/// 声明文件路径（`profiles/<Profile 名>/` 目录由调用方给出，必须是绝对路径）。
pub fn decl_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join(PROFILE_DECL_FILE)
}

/// 只读读取声明原文（OD-ARCH-053 设置页内嵌编辑框）。
///
/// 只读、不创建、不改写；文件不存在返回 `Ok(None)`。上限与保存端一致（64 KB）。
pub fn read_text(profile_dir: &Path) -> Result<Option<String>, String> {
    if !profile_dir.is_absolute() {
        return Err(format!(
            "Profile 目录必须是绝对路径：{}",
            profile_dir.display()
        ));
    }
    let path = decl_path(profile_dir);
    match fs::metadata(&path) {
        Ok(meta) if meta.len() > 64 * 1024 => Err(format!(
            "声明文件超过 64 KB（{} 字节），请用系统编辑器处理：{}",
            meta.len(),
            path.display()
        )),
        Ok(_) => fs::read_to_string(&path)
            .map(Some)
            .map_err(|e| format!("无法读取 Profile 声明（{}）：{}", path.display(), e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!(
            "无法读取 Profile 声明（{}）：{}",
            path.display(),
            e
        )),
    }
}

/// 加载当前 Profile 的声明。
///
/// **文件不存在就返回 `Ok(None)`**：基础标准本来就没有场景声明，这不是错误，
/// 更不能顺手写一份默认声明把干净底座变成带场景的工作台。
pub fn load_decl(profile_dir: &Path) -> Result<Option<LoadedDecl>, String> {
    if !profile_dir.is_absolute() {
        return Err(format!(
            "Profile 目录必须是绝对路径：{}",
            profile_dir.display()
        ));
    }
    let path = decl_path(profile_dir);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(format!(
                "读取 Profile 声明失败（{}）：{}",
                path.display(),
                e
            ))
        }
    };
    let content_sha256 = sha256_hex(text.as_bytes());
    let decl = parse_decl(&text).map_err(|e| {
        format!(
            "Profile 声明格式错误（{}）：{}。该声明已被拒绝，界面使用内置默认入口。",
            path.display(),
            e
        )
    })?;
    decl.validate().map_err(|e| {
        format!(
            "Profile 声明内容无效（{}）：{}。该声明已被拒绝，界面使用内置默认入口。",
            path.display(),
            e
        )
    })?;

    Ok(Some(LoadedDecl {
        decl,
        path: path.display().to_string(),
        content_sha256,
        text,
    }))
}

/// OD-ARCH-106 一次性迁移：把旧的单文件声明复制进 `profiles/<Profile 名>/decl.yml`。
///
/// 只对**非标准 Profile**执行；新位置已有文件一律不动（永不覆盖）。旧文件原样保留，
/// 作为迁移前的备份。返回 `true` 表示本次复制发生了。
pub fn migrate_legacy_decl(
    profiles_root: &Path,
    profile: &str,
    standard_profile: &str,
) -> Result<bool, String> {
    if profile.trim().is_empty() || profile == standard_profile {
        return Ok(false);
    }
    let legacy = profiles_root.join(LEGACY_PROFILE_DECL_FILE);
    if !legacy.is_file() {
        return Ok(false);
    }
    let target_dir = profiles_root.join(profile);
    let target = target_dir.join(PROFILE_DECL_FILE);
    if target.exists() {
        return Ok(false);
    }
    // 先按旧文件内容校验：坏文件不搬，交给界面如实报错。
    let text = fs::read_to_string(&legacy)
        .map_err(|e| format!("读取旧声明失败（{}）：{}", legacy.display(), e))?;
    parse_decl(&text)?.validate()?;
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", target_dir.display(), e))?;
    fs::copy(&legacy, &target).map_err(|e| {
        format!(
            "迁移旧声明失败（{} → {}）：{}",
            legacy.display(),
            target.display(),
            e
        )
    })?;
    Ok(true)
}

/// 解析声明的受限 YAML 子集。
///
/// 支持的写法（与 `samples/profiles/discipline.yml` 一致）：
/// ```text
/// schema: omnidesk.profile-decl.v1
/// display_name: OmniDesk Discipline
/// scenes:
///   - id: home
///     label: 工作台首页
///     icon: brand
///     note: 可选
/// ```
/// 允许整行 `#` 注释与空行。其它结构（嵌套映射、锚点、多文档、flow 语法）一律报错。
pub fn parse_decl(text: &str) -> Result<ProfileDecl, String> {
    let mut schema: Option<String> = None;
    let mut display_name: Option<String> = None;
    let mut scenes: Vec<SceneEntry> = Vec::new();
    let mut current: Option<SceneEntry> = None;
    let mut in_scenes = false;
    let mut last_line = 1usize;

    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        last_line = line_no;
        let line = strip_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        if line.contains('\t') {
            return Err(format!("第 {line_no} 行含制表符；请用空格缩进"));
        }
        let indent = line.len() - line.trim_start_matches(' ').len();
        let body = line.trim_start_matches(' ');

        if indent == 0 {
            if let Some(entry) = current.take() {
                scenes.push(entry);
            }
            let (key, value) = split_kv(body, line_no)?;
            match key {
                "schema" => schema = Some(value),
                "display_name" => display_name = Some(value),
                "scenes" => {
                    if !value.is_empty() {
                        return Err(format!("第 {line_no} 行：scenes 下必须是列表"));
                    }
                    in_scenes = true;
                }
                other => {
                    return Err(format!(
                        "第 {line_no} 行：未知顶层键「{other}」（本声明只认 schema / display_name / scenes）"
                    ))
                }
            }
            continue;
        }

        if !in_scenes {
            return Err(format!("第 {line_no} 行：缩进行只能出现在 scenes 下"));
        }

        if let Some(rest) = body.strip_prefix("- ") {
            if let Some(entry) = current.take() {
                scenes.push(entry);
            }
            let mut entry = SceneEntry {
                id: String::new(),
                label: String::new(),
                icon: String::new(),
                note: String::new(),
            };
            let (key, value) = split_kv(rest, line_no)?;
            apply_scene_field(&mut entry, key, value, line_no)?;
            current = Some(entry);
        } else if body == "-" {
            return Err(format!("第 {line_no} 行：列表项必须写成 `- key: value`"));
        } else {
            let entry = current.as_mut().ok_or_else(|| {
                format!("第 {line_no} 行：缩进行前面缺少 `- ` 列表项")
            })?;
            let (key, value) = split_kv(body, line_no)?;
            apply_scene_field(entry, key, value, line_no)?;
        }
    }
    if let Some(entry) = current.take() {
        scenes.push(entry);
    }

    Ok(ProfileDecl {
        schema: schema.ok_or_else(|| format!("缺少 schema（读到第 {last_line} 行）"))?,
        display_name: display_name.ok_or("缺少 display_name")?,
        scenes,
    })
}

fn apply_scene_field(
    entry: &mut SceneEntry,
    key: &str,
    value: String,
    line_no: usize,
) -> Result<(), String> {
    let slot = match key {
        "id" => &mut entry.id,
        "label" => &mut entry.label,
        "icon" => &mut entry.icon,
        "note" => &mut entry.note,
        other => {
            return Err(format!(
                "第 {line_no} 行：未知场景字段「{other}」（只认 id / label / icon / note）"
            ))
        }
    };
    if !slot.is_empty() {
        return Err(format!("第 {line_no} 行：字段「{key}」重复"));
    }
    *slot = value;
    Ok(())
}

/// 去掉行尾注释（`#` 前必须是行首或空白，避免切坏含 `#` 的值）。
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            return &line[..i];
        }
    }
    line
}

fn split_kv(body: &str, line_no: usize) -> Result<(&str, String), String> {
    let (key, value) = body
        .split_once(':')
        .ok_or_else(|| format!("第 {line_no} 行：缺少冒号"))?;
    let key = key.trim();
    if key.is_empty() {
        return Err(format!("第 {line_no} 行：冒号前没有键名"));
    }
    let mut value = value.trim().to_string();
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let quoted = (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'');
        if quoted {
            value = value[1..value.len() - 1].to_string();
        } else if value.starts_with('"') || value.starts_with('\'') {
            return Err(format!("第 {line_no} 行：引号没有闭合"));
        }
    }
    Ok((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_default_decl_is_valid() {
        let decl = parse_decl(DEFAULT_DECL).expect("内嵌默认声明必须可解析");
        decl.validate().expect("内嵌默认声明必须通过校验");
        assert_eq!(decl.display_name, "OmniDesk Discipline");
        assert!(decl.scenes.iter().any(|s| s.id == "report"));
    }

    #[test]
    fn removing_a_scene_changes_the_declaration() {
        let text = "schema: omnidesk.profile-decl.v1\ndisplay_name: T\nscenes:\n  - id: home\n    label: 首页\n    icon: brand\n  - id: writing\n    label: 文案\n    icon: book\n";
        let decl = parse_decl(text).unwrap();
        decl.validate().unwrap();
        assert_eq!(decl.scenes.len(), 2);
        assert_eq!(decl.scenes[1].id, "writing");
    }

    #[test]
    fn unknown_scene_id_is_rejected() {
        let text = "schema: omnidesk.profile-decl.v1\ndisplay_name: T\nscenes:\n  - id: shell\n    label: X\n    icon: brand\n";
        let decl = parse_decl(text).unwrap();
        let err = decl.validate().unwrap_err();
        assert!(err.contains("不在允许列表内"), "实际：{err}");
    }

    #[test]
    fn duplicate_scene_is_rejected() {
        let text = "schema: omnidesk.profile-decl.v1\ndisplay_name: T\nscenes:\n  - id: home\n    label: A\n    icon: brand\n  - id: home\n    label: B\n    icon: note\n";
        let decl = parse_decl(text).unwrap();
        let err = decl.validate().unwrap_err();
        assert!(err.contains("重复"), "实际：{err}");
    }

    #[test]
    fn wrong_schema_is_rejected() {
        let text = "schema: omnidesk.something-else\ndisplay_name: T\nscenes:\n  - id: home\n    label: A\n    icon: brand\n";
        let decl = parse_decl(text).unwrap();
        assert!(decl.validate().is_err());
    }

    #[test]
    fn nested_mapping_is_rejected_not_guessed() {
        let text = "schema: omnidesk.profile-decl.v1\ndisplay_name: T\nscenes:\n  - id: home\n    label:\n      nested: x\n";
        // 超出受限子集的嵌套映射必须直接报错，而不是猜成某个字段值。
        let err = parse_decl(text).expect_err("嵌套映射应被拒绝");
        assert!(err.contains("nested") || err.contains("未能识别"), "实际：{err}");
    }

    #[test]
    fn tabs_are_rejected() {
        let text = "schema: omnidesk.profile-decl.v1\ndisplay_name:\tT\nscenes:\n";
        assert!(parse_decl(text).is_err());
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let text = "# 头部注释\nschema: omnidesk.profile-decl.v1\n\ndisplay_name: T  # 行尾注释\nscenes:\n  # 条目注释\n  - id: home\n    label: 首页\n    icon: brand\n";
        let decl = parse_decl(text).unwrap();
        decl.validate().unwrap();
        assert_eq!(decl.display_name, "T");
    }

    #[test]
    fn missing_file_is_none_and_never_creates_a_default() {
        // OD-ARCH-106：基础标准下没有声明就是没有场景。加载不得写文件，
        // 否则「干净底座」会在第一次启动后变成带场景的工作台。
        let root = std::env::temp_dir().join(format!(
            "od-profile-decl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = root.join("profiles").join("omnidesk-base");
        assert!(load_decl(&dir).expect("缺文件不是错误").is_none());
        assert!(!dir.exists(), "只读加载不得创建 Profile 目录或声明文件");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_user_edit_and_rejects_broken_file() {
        let root = temp_profiles_dir("edit-read");
        fs::create_dir_all(&root).unwrap();
        let edited = "schema: omnidesk.profile-decl.v1\ndisplay_name: 我的工作台\nscenes:\n  - id: home\n    label: 首页\n    icon: brand\n";
        fs::write(decl_path(&root), edited).unwrap();
        let loaded = load_decl(&root).expect("应能加载").expect("文件存在应返回 Some");
        assert_eq!(loaded.decl.display_name, "我的工作台");
        assert_eq!(loaded.decl.scenes.len(), 1);

        // 坏文件必须报错，不悄悄退回默认。
        fs::write(decl_path(&root), "schema: nope\n").unwrap();
        assert!(load_decl(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn migrates_legacy_decl_to_profile_dir_once_without_overwrite() {
        let root = temp_profiles_dir("migrate");
        fs::create_dir_all(&root).unwrap();
        let legacy_text = "schema: omnidesk.profile-decl.v1\ndisplay_name: 旧声明\nscenes:\n  - id: home\n    label: 首页\n    icon: brand\n";
        fs::write(root.join(LEGACY_PROFILE_DECL_FILE), legacy_text).unwrap();

        // 标准 Profile 不迁移：干净底座不得被旧场景污染。
        assert!(!migrate_legacy_decl(&root, "omnidesk-base", "omnidesk-base").unwrap());
        assert!(!root.join("omnidesk-base").exists());

        assert!(migrate_legacy_decl(&root, "discipline", "omnidesk-base").unwrap());
        let target = root.join("discipline").join(PROFILE_DECL_FILE);
        assert_eq!(fs::read_to_string(&target).unwrap(), legacy_text);
        // 旧文件保留，作为迁移前备份。
        assert!(root.join(LEGACY_PROFILE_DECL_FILE).is_file());

        // 新位置已有文件：不再覆盖。
        fs::write(&target, "schema: omnidesk.profile-decl.v1\ndisplay_name: 用户改过\nscenes:\n  - id: home\n    label: 首页\n    icon: brand\n").unwrap();
        assert!(!migrate_legacy_decl(&root, "discipline", "omnidesk-base").unwrap());
        assert!(fs::read_to_string(&target).unwrap().contains("用户改过"));
        let _ = fs::remove_dir_all(&root);
    }

    fn temp_profiles_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "od-profile-digest-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn digest_matches_loaded_content_and_changes_after_edit() {
        let dir = temp_profiles_dir("edit");
        fs::create_dir_all(&dir).unwrap();
        fs::write(decl_path(&dir), DEFAULT_DECL).unwrap();
        let loaded = load_decl(&dir).expect("应能加载").expect("文件存在应返回 Some");
        // 载入后未改动：指纹一致 —— 界面此时不应提示重启。
        assert_eq!(
            decl_digest(&dir).expect("应能读指纹"),
            Some(loaded.content_sha256.clone())
        );

        // 用户改了文件：指纹变化 —— 界面据此提示"重启应用生效"。
        let edited = "schema: omnidesk.profile-decl.v1\ndisplay_name: 我的工作台\nscenes:\n  - id: home\n    label: 首页\n    icon: brand\n";
        fs::write(decl_path(&dir), edited).unwrap();
        let after = decl_digest(&dir).expect("应能读指纹");
        assert!(after.is_some());
        assert_ne!(after, Some(loaded.content_sha256));

        // 探测是只读的：文件内容与修改时间不被它改写。
        assert_eq!(fs::read_to_string(decl_path(&dir)).unwrap(), edited);
        // 此时真正的加载才会拿到新内容，证明"重启生效"的说法属实。
        assert_eq!(
            load_decl(&dir).unwrap().unwrap().decl.display_name,
            "我的工作台"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn digest_of_missing_file_is_none_and_does_not_create_it() {
        let dir = temp_profiles_dir("missing");
        // 目标目录与文件都不存在：探测不得创建任何东西。
        assert_eq!(decl_digest(&dir).expect("缺失应返回 None"), None);
        assert!(!dir.exists(), "只读探测不得创建 Profile 目录");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn digest_rejects_relative_dir() {
        assert!(decl_digest(Path::new("profiles")).is_err());
    }
}
