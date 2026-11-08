//! 自建 Profile 的起步骨架（OD-ARCH-122）。
//!
//! 产品口径：「OmniDesk 是干净底座，Profile 是定制工作台」。从基础标准新建一个
//! Profile 时，如果只复制 DSH 的配置，切过去之后只有平台固有入口（首页 / 知识库），
//! 用户还得自己从零写一整份 decl.yml 才有业务场景。这里给一份最小、可改的骨架：
//!   * decl.yml：四个标准场景入口，名字取自用户填的显示名；
//!   * rules.json：schema v2、空 global、空 scenes、空 skill_bindings —— 零预注入。
//!
//! 边界：
//!   * 只在「新建自定义 Profile」与「用户在设置页点填入起步模板」两处产出；
//!   * 基础标准（omnidesk-base）永远不写，目录都不建；
//!   * 永不覆盖已有文件：写前判存在，已存在就跳过并如实说明；
//!   * 写前先用真实解析器自校验，生成不出来就报错，不往盘上写半成品。

use std::fs;
use std::path::{Path, PathBuf};

use crate::profile_decl::{parse_decl, PROFILE_DECL_FILE, PROFILE_DECL_SCHEMA};
use crate::profile_rules::{parse_rules, PROFILE_RULES_FILE, PROFILE_RULES_SCHEMA_V2};

/// 起步声明里固定的四个入口。顺序即 Dock 顺序，与出厂 Profile 一致。
///
/// 图标只用 ALLOWED_ICONS 里的名字；这些场景 id 都在 ALLOWED_SCENES 内，
/// 这里仍走一次真实解析与校验，避免「模板自己不合规」。
const STARTER_SCENES: &[(&str, &str, &str, &str)] = &[
    ("home", "工作台首页", "brand", ""),
    ("report", "工作记录", "note", ""),
    ("writing", "文案写作", "book", ""),
    ("project", "项目跟踪", "checklist", ""),
];

/// 起步声明的显示名：压成单行、去掉首尾空白，最长 64 字（与声明校验同口径）。
fn clean_label(label: &str) -> String {
    let single: String = label
        .chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .collect();
    let trimmed = single.trim();
    if trimmed.is_empty() {
        return "我的工作台".to_string();
    }
    trimmed.chars().take(64).collect()
}

/// 生成一份起步声明文本（已通过 parse_decl + validate）。
pub fn decl_text(label: &str) -> Result<String, String> {
    let display = clean_label(label);
    let mut out = String::new();
    out.push_str("# OmniDesk Profile 场景声明（omnidesk.profile-decl.v1）\n");
    out.push_str("# 这份文件决定左侧 Dock 出现哪些业务场景、叫什么名字、用什么图标。\n");
    out.push_str("# 可以改名字、删掉不需要的入口；保存后工作台入口立刻更新。\n");
    out.push_str(&format!("schema: {}\n", PROFILE_DECL_SCHEMA));
    out.push_str(&format!("display_name: {}\n", display));
    out.push_str("scenes:\n");
    for (id, scene_label, icon, note) in STARTER_SCENES {
        out.push_str(&format!("  - id: {}\n", id));
        out.push_str(&format!("    label: {}\n", scene_label));
        out.push_str(&format!("    icon: {}\n", icon));
        if !note.is_empty() {
            out.push_str(&format!("    note: {}\n", note));
        }
    }
    let decl = parse_decl(&out)?;
    decl.validate()?;
    Ok(out)
}

/// 生成一份起步规则包文本（已通过 parse_rules）。
///
/// 刻意全空：没有行业红线和写作铁律，也没有 Skill 绑定。省 token、快回复，
/// 需要什么由用户在「规则资产」页自己加，或从别的 Profile 复制。
pub fn rules_text(profile: &str) -> Result<String, String> {
    let value = serde_json::json!({
        "schema": PROFILE_RULES_SCHEMA_V2,
        "profile": profile,
        "global": {},
        "scenes": {},
        "skill_bindings": {},
    });
    let mut text = serde_json::to_string_pretty(&value)
        .map_err(|e| format!("生成起步规则包失败：{}", e))?;
    text.push('\n');
    parse_rules(&text)?;
    Ok(text)
}

/// 把起步骨架写进 <app_root>/profiles/<profile>/。
///
/// 返回实际写下的文件名（已存在的不算）。永不覆盖：两个文件分别判存在。
pub fn write_starter(
    app_root: &Path,
    profile: &str,
    label: &str,
) -> Result<Vec<String>, String> {
    let dir = app_root.join("profiles").join(profile);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", dir.display(), e))?;
    let mut written: Vec<String> = Vec::new();
    let decl_path: PathBuf = dir.join(PROFILE_DECL_FILE);
    if !decl_path.is_file() {
        let text = decl_text(label)?;
        crate::model_settings::write_atomic(&decl_path, text.as_bytes())?;
        written.push(PROFILE_DECL_FILE.to_string());
    }
    let rules_path: PathBuf = dir.join(PROFILE_RULES_FILE);
    if !rules_path.is_file() {
        let text = rules_text(profile)?;
        crate::model_settings::write_atomic(&rules_path, text.as_bytes())?;
        written.push(PROFILE_RULES_FILE.to_string());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "od-profile-starter-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn decl_template_parses_and_covers_four_scenes() {
        let text = decl_text("我的工作台").unwrap();
        let decl = parse_decl(&text).unwrap();
        decl.validate().unwrap();
        assert_eq!(decl.display_name, "我的工作台");
        assert_eq!(
            decl.scenes.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["home", "report", "writing", "project"]
        );
    }

    /// 显示名里的换行会破坏单行 YAML：必须压成空格，而不是生成坏文件。
    #[test]
    fn decl_template_flattens_multiline_label() {
        let text = decl_text("我的\n工作台\r\n").unwrap();
        let decl = parse_decl(&text).unwrap();
        decl.validate().unwrap();
        assert_eq!(decl.display_name, "我的 工作台");
        let fallback = parse_decl(&decl_text("   ").unwrap()).unwrap();
        assert_eq!(fallback.display_name, "我的工作台");
    }

    #[test]
    fn rules_template_is_empty_but_valid() {
        let text = rules_text("my-work").unwrap();
        let rules = parse_rules(&text).unwrap();
        assert_eq!(rules.profile, "my-work");
        assert_eq!(rules.schema, PROFILE_RULES_SCHEMA_V2);
        assert!(rules.skill_bindings.is_empty(), "起步规则包不得预注入任何 Skill");
        assert!(rules.scenes.is_empty());
        assert!(rules.global.writing_rules.is_empty());
        assert!(rules.global.redline_keywords.is_empty());
    }

    #[test]
    fn write_starter_never_overwrites() {
        let root = temp_root("write");
        let written = write_starter(&root, "my-work", "我的工作台").unwrap();
        assert_eq!(written, vec!["decl.yml".to_string(), "rules.json".to_string()]);
        let decl_path = root.join("profiles").join("my-work").join("decl.yml");
        fs::write(&decl_path, "schema: omnidesk.profile-decl.v1\ndisplay_name: 手改\nscenes:\n  - id: home\n    label: 首页\n    icon: brand\n").unwrap();
        let again = write_starter(&root, "my-work", "另一个名字").unwrap();
        assert!(again.is_empty(), "已存在的文件不得被覆盖");
        assert!(fs::read_to_string(&decl_path).unwrap().contains("手改"));
    }
}
