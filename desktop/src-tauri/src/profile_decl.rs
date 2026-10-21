//! 纪检 Profile 声明（omnidesk.profile-decl.v1）——OD-ARCH-051 S3。
//!
//! 只声明**界面层面**的内容：Profile 名、Dock 上要显示哪些业务场景入口、它们的顺序。
//! 它不加载插件、不改模型路由、不改 Skill 清单、不改任何判定逻辑，也不是多 Profile
//! 管理器的雏形：本阶段只有一个生效文件，第二个 Profile 只预留**目录位置**。
//!
//! 为什么是文件而无代码：计划要求「改声明文件、不动代码，场景入口可见变化」。
//! 因此加载失败必须能明确提示并回退到内置默认，而不是让界面空白或静默吞掉。
//!
//! 落点：`<应用数据根>/profiles/discipline.yml`（仓库外，用户自己的数据根）。
//! 首次启动时由仓库内嵌入的默认声明创建；文件一旦存在就不再自动覆盖。
//!
//! 解析方式：**不引入 YAML 依赖**（本仓库离线构建，锁文件里没有 YAML crate）。
//! 改为解析这份声明所需的受限子集：顶层 `key: value`、`scenes:` 下的列表项、
//! 列表项内的两级缩进键值。遇到超出子集的写法直接报错，不猜、不静默忽略。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（Rust 测试运行）。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 声明文件 schema 标识。版本不符一律拒绝。
pub const PROFILE_DECL_SCHEMA: &str = "omnidesk.profile-decl.v1";
/// 声明文件名（位于应用数据根下的 profiles/ 下）。
pub const PROFILE_DECL_FILE: &str = "discipline.yml";
/// 允许出现在 Dock 上的场景标识。不在表里的一律拒绝——声明不能凭空造入口。
pub const ALLOWED_SCENES: &[&str] = &["home", "report", "writing", "project", "sessions"];
/// 每个场景可选的图标名（取值与前端 `IconName` 对齐，前端另有校验）。
pub const ALLOWED_ICONS: &[&str] = &["brand", "note", "book", "checklist", "history"];
/// 单个字段最大字符数。
const MAX_FIELD_CHARS: usize = 64;
/// 场景条目上限，防止异常文件把 Dock 撑爆。
const MAX_SCENES: usize = 12;

/// 仓库内默认声明。编译期嵌入，作为首次创建的文件内容。
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
    /// true 表示本次首次创建了默认文件（不是覆盖用户修改）。
    pub created: bool,
    /// 加载告警（例如目录里出现了第二个 Profile 目录，本阶段不加载它）。
    pub warnings: Vec<String>,
}

/// 声明文件路径（`profiles/` 目录由调用方给出，必须是绝对路径）。
pub fn decl_path(profiles_dir: &Path) -> PathBuf {
    profiles_dir.join(PROFILE_DECL_FILE)
}

/// 加载声明：不存在则创建默认文件；存在则读取并校验。
///
/// 目录里若还有别的子目录（预留给第二个 Profile），只记录一条告警，不加载、不管理。
pub fn load_decl(profiles_dir: &Path) -> Result<LoadedDecl, String> {
    if !profiles_dir.is_absolute() {
        return Err(format!(
            "Profile 声明目录必须是绝对路径：{}",
            profiles_dir.display()
        ));
    }
    fs::create_dir_all(profiles_dir)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", profiles_dir.display(), e))?;
    let path = decl_path(profiles_dir);
    let mut created = false;
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(DEFAULT_DECL.as_bytes())
                .map_err(|e| format!("写入默认 Profile 声明失败（{}）：{}", path.display(), e))?;
            created = true;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Err(format!(
                "无法创建 Profile 声明文件（{}）：{}",
                path.display(),
                e
            ))
        }
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("读取 Profile 声明失败（{}）：{}", path.display(), e))?;
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

    let mut warnings = Vec::new();
    if let Ok(entries) = fs::read_dir(profiles_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p != path && p.is_dir() {
                warnings.push(format!(
                    "发现额外 Profile 目录「{}」，本阶段只加载 {}，不管理其它 Profile。",
                    p.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                    PROFILE_DECL_FILE
                ));
            }
        }
    }

    Ok(LoadedDecl {
        decl,
        path: path.display().to_string(),
        created,
        warnings,
    })
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
    fn load_creates_default_once_then_reads_user_edit() {
        let root = std::env::temp_dir().join(format!(
            "od-profile-decl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = root.join("profiles");
        let first = load_decl(&dir).expect("首次加载应创建默认文件");
        assert!(first.created);
        assert_eq!(first.decl, parse_decl(DEFAULT_DECL).unwrap());

        // 用户改文件（去掉 writing）：再加载必须读新内容，且不再标记 created。
        let edited = "schema: omnidesk.profile-decl.v1\ndisplay_name: 我的工作台\nscenes:\n  - id: home\n    label: 首页\n    icon: brand\n";
        fs::write(decl_path(&dir), edited).unwrap();
        let second = load_decl(&dir).expect("用户修改后应能加载");
        assert!(!second.created);
        assert_eq!(second.decl.display_name, "我的工作台");
        assert_eq!(second.decl.scenes.len(), 1);

        // 第二个 Profile 目录只告警，不被加载或管理。
        fs::create_dir_all(dir.join("education")).unwrap();
        let third = load_decl(&dir).unwrap();
        assert_eq!(third.warnings.len(), 1);
        assert!(third.warnings[0].contains("education"));

        // 坏文件必须报错，不悄悄退回默认。
        fs::write(decl_path(&dir), "schema: nope\n").unwrap();
        assert!(load_decl(&dir).is_err());

        let _ = fs::remove_dir_all(&root);
    }
}
