//! 离线流程包（omnidesk.offline-flows.v1）。
//!
//! 只承载**展示文本**：标签、提示、结果标题与章节名。它不是工作流引擎：
//! 不执行脚本、不加载外链、不改路径权限、不触发模型，也不能改变任何确定性计算。
//!
//! 落点是 OmniDesk 应用数据根下的 `config/offline-flows.v1.json`（仓库外）。
//! 首次启动时由仓库内嵌入的默认包创建；文件一旦存在就不再自动覆盖，
//! 用户修改后通过界面上的显式“重载流程包”生效。格式错误时整包拒绝，
//! 不静默回退成默认文案。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（Rust 测试运行）。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 流程包 schema 标识。版本不符一律拒绝。
pub const FLOW_PACK_SCHEMA: &str = "omnidesk.offline-flows.v1";
/// 流程包文件名（位于应用数据根的 config/ 下）。
pub const FLOW_PACK_FILE: &str = "offline-flows.v1.json";
/// 单个文本字段的最大长度（字符）。超出即视为异常配置。
const MAX_FIELD_CHARS: usize = 240;
/// 可编辑模型规则的最大长度（字符）。规则比标签长，但仍需有界。
const MAX_DRAFT_RULES_CHARS: usize = 1200;
/// 可编辑输出章节文本的最大长度（字符）。
const MAX_DRAFT_SECTIONS_CHARS: usize = 400;

/// 仓库内默认流程包。编译期嵌入，作为首次创建的文件内容。
pub const DEFAULT_PACK_JSON: &str = include_str!("../../../samples/workflows/offline-flows.v1.json");

/// 单个场景交给模型的**可编辑规则**（OD-ARCH-026）。
///
/// 只是纯文本：`rules` 拼进提示词的“场景规则”，`sections` 是期望的输出章节（每行一个）。
/// 缺省时界面使用内置默认规则；提供时不能为空、不能含外链或脚本。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftRules {
    pub rules: String,
    #[serde(default)]
    pub sections: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeeklyCopy {
    pub tab_label: String,
    pub input_label: String,
    pub input_placeholder: String,
    pub sample_label: String,
    pub result_title: String,
    pub details_heading: String,
    pub confirm_heading: String,
    /// 可选：该场景的可编辑模型规则。旧版流程包没有此字段时使用内置默认规则。
    #[serde(default)]
    pub draft: Option<DraftRules>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritingCopy {
    pub tab_label: String,
    pub purpose_label: String,
    pub audience_label: String,
    pub requirements_label: String,
    pub facts_label: String,
    pub sample_label: String,
    pub result_title: String,
    pub structure_heading: String,
    pub pending_heading: String,
    /// 可选：该场景的可编辑模型规则。
    #[serde(default)]
    pub draft: Option<DraftRules>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCopy {
    pub tab_label: String,
    pub input_label: String,
    pub input_placeholder: String,
    pub today_label: String,
    pub today_placeholder: String,
    pub sample_label: String,
    pub result_title: String,
    pub details_heading: String,
    pub confirm_heading: String,
    /// 可选：该场景的可编辑模型规则。
    #[serde(default)]
    pub draft: Option<DraftRules>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowPack {
    pub schema: String,
    pub weekly: WeeklyCopy,
    pub writing: WritingCopy,
    pub project: ProjectCopy,
}

impl FlowPack {
    /// 结构之外的文本规则：不能有外链、脚本或空字段。
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FLOW_PACK_SCHEMA {
            return Err(format!(
                "不支持的流程包版本 {:?}；当前只接受 {:?}",
                self.schema, FLOW_PACK_SCHEMA
            ));
        }
        let mut fields: Vec<(&str, &str)> = Vec::new();
        let w = &self.weekly;
        fields.extend([
            ("weekly.tab_label", w.tab_label.as_str()),
            ("weekly.input_label", w.input_label.as_str()),
            ("weekly.input_placeholder", w.input_placeholder.as_str()),
            ("weekly.sample_label", w.sample_label.as_str()),
            ("weekly.result_title", w.result_title.as_str()),
            ("weekly.details_heading", w.details_heading.as_str()),
            ("weekly.confirm_heading", w.confirm_heading.as_str()),
        ]);
        let g = &self.writing;
        fields.extend([
            ("writing.tab_label", g.tab_label.as_str()),
            ("writing.purpose_label", g.purpose_label.as_str()),
            ("writing.audience_label", g.audience_label.as_str()),
            ("writing.requirements_label", g.requirements_label.as_str()),
            ("writing.facts_label", g.facts_label.as_str()),
            ("writing.sample_label", g.sample_label.as_str()),
            ("writing.result_title", g.result_title.as_str()),
            ("writing.structure_heading", g.structure_heading.as_str()),
            ("writing.pending_heading", g.pending_heading.as_str()),
        ]);
        let p = &self.project;
        fields.extend([
            ("project.tab_label", p.tab_label.as_str()),
            ("project.input_label", p.input_label.as_str()),
            ("project.input_placeholder", p.input_placeholder.as_str()),
            ("project.today_label", p.today_label.as_str()),
            ("project.today_placeholder", p.today_placeholder.as_str()),
            ("project.sample_label", p.sample_label.as_str()),
            ("project.result_title", p.result_title.as_str()),
            ("project.details_heading", p.details_heading.as_str()),
            ("project.confirm_heading", p.confirm_heading.as_str()),
        ]);
        for (name, value) in fields {
            validate_pack_text(name, value, MAX_FIELD_CHARS)?;
        }
        // 可编辑的模型规则走同一道纯文本检查。旧流程包没有 draft 字段时跳过，
        // 界面回落到内置默认规则（不把缺失当作错误）。
        for (name, draft) in [
            ("weekly.draft", self.weekly.draft.as_ref()),
            ("writing.draft", self.writing.draft.as_ref()),
            ("project.draft", self.project.draft.as_ref()),
        ] {
            let Some(draft) = draft else { continue };
            if draft.rules.trim().is_empty() {
                return Err(format!(
                    "流程包字段 {}.rules 不能为空：要么删掉 draft 使用默认规则，要么给出规则文本",
                    name
                ));
            }
            validate_pack_text(&format!("{}.rules", name), &draft.rules, MAX_DRAFT_RULES_CHARS)?;
            if !draft.sections.trim().is_empty() {
                validate_pack_text(
                    &format!("{}.sections", name),
                    &draft.sections,
                    MAX_DRAFT_SECTIONS_CHARS,
                )?;
            }
        }
        Ok(())
    }
}

/// 单个文本字段检查：非空、长度有界、不含外链、脚本或内联框架。
fn validate_pack_text(name: &str, value: &str, max_chars: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("流程包字段 {} 不能为空", name));
    }
    if value.chars().count() > max_chars {
        return Err(format!("流程包字段 {} 超过 {} 个字符", name, max_chars));
    }
    let lowered = value.to_lowercase();
    for needle in ["http://", "https://", "file://", "javascript:", "<script", "<iframe"] {
        if lowered.contains(needle) {
            return Err(format!(
                "流程包字段 {} 含不允许的内容（{}）；流程包只能是纯文本",
                name, needle
            ));
        }
    }
    Ok(())
}

/// 默认流程包（解析内嵌 JSON）。解析失败属于构建缺陷，测试会直接拦住。
pub fn default_pack() -> FlowPack {
    serde_json::from_str(DEFAULT_PACK_JSON).expect("内嵌默认流程包必须是合法 JSON")
}

/// 已加载的流程包及其来源。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoadedPack {
    pub pack: FlowPack,
    pub path: String,
    /// true 表示本次首次创建了默认文件（不是覆盖用户修改）。
    pub created: bool,
}

/// 流程包文件路径（config 目录由调用方给出，必须是绝对路径）。
pub fn pack_path(config_dir: &Path) -> PathBuf {
    config_dir.join(FLOW_PACK_FILE)
}

/// 加载流程包：不存在则创建默认文件；存在则读取并校验。
///
/// 任何解析或校验失败都返回可读错误，调用方必须拒绝本次流程，
/// 不得悄悄退回默认文案。
pub fn load_pack(config_dir: &Path) -> Result<LoadedPack, String> {
    if !config_dir.is_absolute() {
        return Err(format!(
            "流程包目录必须是绝对路径：{}",
            config_dir.display()
        ));
    }
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("创建流程包目录失败（{}）：{}", config_dir.display(), e))?;
    let path = pack_path(config_dir);
    let mut created = false;
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(DEFAULT_PACK_JSON.as_bytes())
                .map_err(|e| format!("写入默认流程包失败（{}）：{}", path.display(), e))?;
            created = true;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Err(format!(
                "无法创建流程包文件（{}）：{}",
                path.display(),
                e
            ))
        }
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("读取流程包失败（{}）：{}", path.display(), e))?;
    let pack: FlowPack = serde_json::from_str(&text).map_err(|e| {
        format!(
            "流程包格式错误（{}）：{}。该配置已被拒绝，未使用任何默认文案替代。",
            path.display(),
            e
        )
    })?;
    pack.validate().map_err(|e| {
        format!(
            "流程包内容无效（{}）：{}。该配置已被拒绝，未使用任何默认文案替代。",
            path.display(),
            e
        )
    })?;
    Ok(LoadedPack {
        pack,
        path: path.display().to_string(),
        created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "omnidesk-flowpack-{}-{}-{}",
            std::process::id(),
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn default_pack_is_valid() {
        let pack = default_pack();
        pack.validate().unwrap();
        assert_eq!(pack.schema, FLOW_PACK_SCHEMA);
    }

    #[test]
    fn first_load_creates_default_and_keeps_user_edit() {
        let dir = temp_dir("create");
        let first = load_pack(&dir).expect("首次加载应创建默认文件");
        assert!(first.created);
        let file = pack_path(&dir);
        assert!(file.exists());

        let mut edited = first.pack.clone();
        edited.weekly.tab_label = "自定义周报标签".to_string();
        fs::write(&file, serde_json::to_string_pretty(&edited).unwrap()).unwrap();

        let second = load_pack(&dir).expect("再次加载应读取用户文件");
        assert!(!second.created, "已存在的文件不得被重新创建或覆盖");
        assert_eq!(second.pack.weekly.tab_label, "自定义周报标签");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unknown_version() {
        let dir = temp_dir("version");
        fs::create_dir_all(&dir).unwrap();
        let mut pack = default_pack();
        pack.schema = "omnidesk.offline-flows.v99".to_string();
        fs::write(pack_path(&dir), serde_json::to_string(&pack).unwrap()).unwrap();
        let err = load_pack(&dir).unwrap_err();
        assert!(err.contains("不支持的流程包版本"), "{}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_missing_field_and_bad_json() {
        let dir = temp_dir("missing");
        fs::create_dir_all(&dir).unwrap();
        let mut value = serde_json::to_value(default_pack()).unwrap();
        value["weekly"].as_object_mut().unwrap().remove("result_title");
        fs::write(pack_path(&dir), serde_json::to_string(&value).unwrap()).unwrap();
        let err = load_pack(&dir).unwrap_err();
        assert!(err.contains("result_title"), "{}", err);
        assert!(err.contains("已被拒绝"), "{}", err);

        fs::write(pack_path(&dir), "{ not json").unwrap();
        let err = load_pack(&dir).unwrap_err();
        assert!(err.contains("格式错误"), "{}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unknown_field_and_links() {
        let dir = temp_dir("extra");
        fs::create_dir_all(&dir).unwrap();
        let mut value = serde_json::to_value(default_pack()).unwrap();
        value["weekly"]["script"] = serde_json::json!("run me");
        fs::write(pack_path(&dir), serde_json::to_string(&value).unwrap()).unwrap();
        assert!(load_pack(&dir).is_err(), "未知字段必须被拒绝");

        let mut pack = default_pack();
        pack.writing.result_title = "结果 https://example.com".to_string();
        fs::write(pack_path(&dir), serde_json::to_string(&pack).unwrap()).unwrap();
        let err = load_pack(&dir).unwrap_err();
        assert!(err.contains("不允许的内容"), "{}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_relative_config_dir() {
        assert!(load_pack(Path::new("relative/config")).is_err());
    }

    #[test]
    fn default_pack_carries_editable_draft_rules() {
        let pack = default_pack();
        for (name, draft) in [
            ("weekly", pack.weekly.draft.as_ref()),
            ("writing", pack.writing.draft.as_ref()),
            ("project", pack.project.draft.as_ref()),
        ] {
            let draft = draft.unwrap_or_else(|| panic!("{} 应有默认 draft 规则", name));
            assert!(!draft.rules.trim().is_empty(), "{} 规则不能为空", name);
            assert!(!draft.sections.trim().is_empty(), "{} 章节不能为空", name);
        }
    }

    #[test]
    fn old_pack_without_draft_still_loads_and_draft_links_are_rejected() {
        let dir = temp_dir("draft");
        fs::create_dir_all(&dir).unwrap();

        // 旧流程包（没有 draft 字段）必须仍能加载：缺省即使用内置默认规则。
        let mut value = serde_json::to_value(default_pack()).unwrap();
        for key in ["weekly", "writing", "project"] {
            value[key].as_object_mut().unwrap().remove("draft");
        }
        fs::write(pack_path(&dir), serde_json::to_string(&value).unwrap()).unwrap();
        let loaded = load_pack(&dir).expect("旧流程包应仍能加载");
        assert!(loaded.pack.weekly.draft.is_none());

        // draft 里的规则同样不能含外链。
        let mut pack = default_pack();
        if let Some(draft) = pack.project.draft.as_mut() {
            draft.rules = "见 https://example.com".to_string();
        }
        fs::write(pack_path(&dir), serde_json::to_string(&pack).unwrap()).unwrap();
        let err = load_pack(&dir).unwrap_err();
        assert!(err.contains("不允许的内容"), "{}", err);
        let _ = fs::remove_dir_all(&dir);
    }
}
