//! 写作需求的宿主校验与提示词组装（OD-WT-604）。
//!
//! 分工与 504/506 先例一致：
//!   * Python 只组装提示词（engine/omnidesk_engine/writing_extract.py），不落盘；
//!   * 模型输出先由这里校验（十二类白名单、缺失项、字段长度），校验后的草稿
//!     直接返回界面，不写任何文件；
//!   * 用户确认草稿后，才经 writing_store::create_writing_task 创建写作任务，
//!     初始状态固定为「需求待整理」；
//!   * 本模块不新增模型调用通道，模型调用仍由界面侧 submitTask 提交。
//!
//! 证据等级：源码事实 + 模块内 cargo test。

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::writing_store::{self, DOCUMENT_TYPES};

pub const REQUIREMENT_SCHEMA: &str = "omnidesk.writing-requirement.v1";
pub const MAX_RESULT_BYTES: usize = 256 * 1024;
pub const MAX_INPUT_CHARS: usize = 20_000;
pub const MAX_TITLE_CHARS: usize = 200;
/// 单字段长度上限，照 task_extract 的分类/备注等常量先例收紧到 400 字。
pub const MAX_FIELD_CHARS: usize = 400;
pub const MAX_REQUIREMENTS: usize = 12;
pub const MAX_REQUIREMENT_CHARS: usize = 200;
pub const MAX_QUESTIONS: usize = 10;
pub const MAX_QUESTION_CHARS: usize = 200;

/// 解析准备回执：提示词与送模型前的原文（已过红线与掩码）。
#[derive(Debug, Clone, Serialize)]
pub struct WritingParsePrompt {
    pub prompt: String,
    pub user_input_text: String,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

/// 校验后的写作需求草稿。只返回界面，不落盘。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct WritingRequirementDraft {
    pub document_type: Option<String>,
    pub title: Option<String>,
    pub topic: Option<String>,
    pub writing_purpose: Option<String>,
    pub audience: Option<String>,
    pub writing_period: Option<String>,
    pub expected_length: Option<String>,
    pub writing_style: Option<String>,
    pub requirements: Vec<String>,
    pub questions: Vec<String>,
    pub confidence: Option<f64>,
}

/// 解析落地回执：草稿 + 是否已具备创建写作任务的最低条件。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WritingRequirementApplied {
    pub draft: WritingRequirementDraft,
    /// 文稿类型与标题都已确定时才能创建任务（writing_store 的最低校验）。
    pub ready_to_create: bool,
}

/// 用户确认创建后的回执：任务实体 + 掩码计数。
#[derive(Debug, Clone, Serialize)]
pub struct WritingTaskCreated {
    pub task: writing_store::WritingTask,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct RawRequirement {
    document_type: Option<String>,
    title: Option<String>,
    topic: Option<String>,
    writing_purpose: Option<String>,
    audience: Option<String>,
    writing_period: Option<String>,
    expected_length: Option<String>,
    writing_style: Option<String>,
    requirements: Option<Vec<String>>,
    questions: Option<Vec<String>>,
    confidence: Option<f64>,
}

fn clean_optional(
    value: Option<String>,
    field: &str,
    limit: usize,
) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if trimmed.chars().count() > limit {
                return Err(format!("写作需求结果的 {field} 过长（最多 {limit} 字）。"));
            }
            if trimmed.contains('\n') {
                return Err(format!("写作需求结果的 {field} 不得含换行。"));
            }
            Ok(Some(trimmed))
        }
    }
}

fn clean_list(
    value: Vec<String>,
    field: &str,
    max_items: usize,
    item_limit: usize,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for item in value {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            return Err(format!("写作需求结果的 {field} 里不能有空条目。"));
        }
        if trimmed.chars().count() > item_limit {
            return Err(format!("写作需求结果的 {field} 条目过长（最多 {item_limit} 字）。"));
        }
        if trimmed.contains('\n') {
            return Err(format!("写作需求结果的 {field} 条目不得含换行。"));
        }
        if !out.iter().any(|existing| existing == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    if out.len() > max_items {
        return Err(format!("写作需求结果的 {field} 过多（最多 {max_items} 条）。"));
    }
    Ok(out)
}

fn mentions_document_type(question: &str) -> bool {
    ["文稿类型", "文档类型", "文种", "文体", "哪种类型"]
        .iter()
        .any(|marker| question.contains(*marker))
}

fn validate_raw(raw: RawRequirement) -> Result<WritingRequirementApplied, String> {
    let document_type = match raw.document_type {
        None => None,
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                return Err(
                    "文档类型无法确定时 document_type 必须填 null，不得写空字符串。"
                        .to_string(),
                );
            }
            if !DOCUMENT_TYPES.contains(&trimmed.as_str()) {
                return Err(format!("文档类型不在十二类白名单内：{trimmed}"));
            }
            Some(trimmed)
        }
    };
    let title = clean_optional(raw.title, "标题", MAX_TITLE_CHARS)?;
    let topic = clean_optional(raw.topic, "主题", MAX_FIELD_CHARS)?;
    let writing_purpose = clean_optional(raw.writing_purpose, "写作目的", MAX_FIELD_CHARS)?;
    let audience = clean_optional(raw.audience, "目标受众", MAX_FIELD_CHARS)?;
    let writing_period = clean_optional(raw.writing_period, "时间范围", MAX_FIELD_CHARS)?;
    let expected_length = clean_optional(raw.expected_length, "预计篇幅", MAX_FIELD_CHARS)?;
    let writing_style = clean_optional(raw.writing_style, "写作风格", MAX_FIELD_CHARS)?;
    let requirements = match raw.requirements {
        Some(list) => clean_list(list, "写作要点", MAX_REQUIREMENTS, MAX_REQUIREMENT_CHARS)?,
        None => {
            return Err("写作需求解析结果缺少 requirements 数组。".to_string());
        }
    };
    let questions = match raw.questions {
        Some(list) => clean_list(list, "待确认问题", MAX_QUESTIONS, MAX_QUESTION_CHARS)?,
        None => {
            return Err("写作需求解析结果缺少 questions 数组。".to_string());
        }
    };
    let confidence = match raw.confidence {
        None => None,
        Some(value) => {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err("写作需求结果的置信度必须落在 0 到 1 之间。".to_string());
            }
            Some(value)
        }
    };
    let draft = WritingRequirementDraft {
        document_type,
        title,
        topic,
        writing_purpose,
        audience,
        writing_period,
        expected_length,
        writing_style,
        requirements,
        questions,
        confidence,
    };
    if draft.document_type.is_none() {
        if draft.questions.is_empty() {
            return Err(
                "文稿类型无法确定时，questions 必须至少包含一条类型询问。".to_string(),
            );
        }
        if !draft
            .questions
            .iter()
            .any(|question| mentions_document_type(question))
        {
            return Err(
                "文稿类型无法确定时，questions 必须包含一条询问文稿类型的问题。".to_string(),
            );
        }
    }
    let missing_core = draft.title.is_none()
        || draft.topic.is_none()
        || draft.writing_purpose.is_none()
        || draft.audience.is_none()
        || draft.writing_period.is_none()
        || draft.expected_length.is_none();
    if missing_core && draft.questions.is_empty() {
        return Err(
            "关键写作条件缺失时 questions 不得为空，必须逐条列出待补充问题。".to_string(),
        );
    }
    let ready_to_create = draft.document_type.is_some() && draft.title.is_some();
    Ok(WritingRequirementApplied {
        draft,
        ready_to_create,
    })
}

/// 校验模型输出：白名单、缺失项、字段长度与置信度。
/// 校验通过只返回草稿，不写文件；任何失败都不落盘。
pub fn validate_requirement_payload(
    result_text: &str,
) -> Result<WritingRequirementApplied, String> {
    let text = result_text.trim();
    if text.is_empty() {
        return Err("写作需求解析结果为空。".to_string());
    }
    if text.len() > MAX_RESULT_BYTES {
        return Err(format!(
            "写作需求解析结果 {} 字节，超过 {} 字节上限。",
            text.len(),
            MAX_RESULT_BYTES
        ));
    }
    let value = crate::knowledge_ingest::parse_result_text(text)
        .map_err(|e| format!("写作需求解析结果不是合法 JSON：{e}"))?;
    let raw: RawRequirement = serde_json::from_value(value)
        .map_err(|e| format!("写作需求解析结果不符合约定结构：{e}"))?;
    validate_raw(raw)
}

/// 调用 Python 组装写作需求解析提示词。入参文本必须已经过红线与掩码。
pub fn build_requirement_prompt(
    python: &str,
    engine_dir: &Path,
    text: &str,
) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("写作需求不能为空。".to_string());
    }
    if trimmed.chars().count() > MAX_INPUT_CHARS {
        return Err(format!("写作需求过长（最多 {MAX_INPUT_CHARS} 字）。"));
    }
    let payload = serde_json::json!({
        "schema": REQUIREMENT_SCHEMA,
        "text": trimmed,
    });
    let mut child = Command::new(python)
        .arg("-m")
        .arg("omnidesk_engine.writing_extract")
        .current_dir(engine_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动写作需求提示词组装失败（{python}）：{e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.to_string().as_bytes())
            .map_err(|e| format!("写入写作需求提示词请求失败：{e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待写作需求提示词组装失败：{e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr
            .lines()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(format!("写作需求提示词组装失败：{tail}"));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("写作需求提示词组装返回的不是 JSON：{e}"))?;
    let prompt = value
        .get("prompt")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "写作需求提示词组装结果缺少 prompt".to_string())?;
    Ok(prompt.to_string())
}

/// 把已确认的草稿转成 writing_store 的创建入参。
/// 文稿类型或标题缺失时拒绝创建，不用默认值补位。
pub fn to_task_draft(
    draft: &WritingRequirementDraft,
) -> Result<writing_store::WritingTaskDraft, String> {
    let document_type = draft
        .document_type
        .clone()
        .ok_or_else(|| "文稿类型尚未确定，不能创建写作任务。".to_string())?;
    let title = draft
        .title
        .clone()
        .ok_or_else(|| "写作任务标题尚未确定，不能创建写作任务。".to_string())?;
    Ok(writing_store::WritingTaskDraft {
        writing_id: None,
        title,
        document_type,
        writing_purpose: draft.writing_purpose.clone().unwrap_or_default(),
        audience: draft.audience.clone().unwrap_or_default(),
        writing_period: draft.writing_period.clone().unwrap_or_default(),
        topic: draft.topic.clone().unwrap_or_default(),
        requirements: draft.requirements.join("\n"),
        expected_length: draft.expected_length.clone().unwrap_or_default(),
        writing_style: draft.writing_style.clone().unwrap_or_default(),
        selected_material_ids: Vec::new(),
        selected_knowledge_ids: Vec::new(),
        document_id: None,
    })
}

/// 用户确认后创建写作任务；初始状态由 writing_store 固定为「需求待整理」。
pub fn create_writing_task_from_requirement(
    root: &Path,
    draft: &WritingRequirementDraft,
    now: f64,
) -> Result<writing_store::WritingTask, String> {
    let task_draft = to_task_draft(draft)?;
    writing_store::create_writing_task(root, &task_draft, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_payload() -> serde_json::Value {
        serde_json::json!({
            "document_type": "月度总结",
            "title": "十一月份试点推进总结",
            "topic": "试点推进情况",
            "writing_purpose": "向上级汇报本月进展",
            "audience": "部门同事",
            "writing_period": "2026-11",
            "expected_length": "1200 字",
            "writing_style": "简洁平实",
            "requirements": ["只写已确认事实", "三点以内"],
            "questions": [],
            "confidence": 0.8
        })
    }

    #[test]
    fn accepts_complete_payload_without_questions() {
        let text = complete_payload().to_string();
        let applied = validate_requirement_payload(&text).unwrap();
        assert!(applied.ready_to_create);
        assert_eq!(applied.draft.document_type.as_deref(), Some("月度总结"));
        assert_eq!(applied.draft.requirements.len(), 2);
        assert!(applied.draft.questions.is_empty());
        assert_eq!(applied.draft.confidence, Some(0.8));
    }

    #[test]
    fn rejects_document_type_outside_whitelist() {
        let mut value = complete_payload();
        value["document_type"] = serde_json::json!("新闻稿");
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("十二类白名单"), "{err}");
    }

    #[test]
    fn rejects_empty_document_type_string() {
        let mut value = complete_payload();
        value["document_type"] = serde_json::json!("  ");
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("必须填 null"), "{err}");
    }

    #[test]
    fn rejects_unknown_document_type_without_type_question() {
        let mut value = complete_payload();
        value["document_type"] = serde_json::Value::Null;
        value["questions"] = serde_json::json!(["篇幅希望多长？"]);
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("询问文稿类型"), "{err}");
    }

    #[test]
    fn accepts_unknown_document_type_with_type_question() {
        let mut value = complete_payload();
        value["document_type"] = serde_json::Value::Null;
        value["questions"] = serde_json::json!(["这份材料属于哪种文稿类型？"]);
        let text = value.to_string();
        let applied = validate_requirement_payload(&text).unwrap();
        assert!(!applied.ready_to_create);
        assert!(applied.draft.document_type.is_none());
    }

    #[test]
    fn rejects_missing_questions_key_and_empty_question_item() {
        let mut value = complete_payload();
        let _ = value.as_object_mut().unwrap().remove("questions");
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("缺少 questions"), "{err}");
        let mut value = complete_payload();
        value["questions"] = serde_json::json!([" "]);
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("空条目"), "{err}");
    }

    #[test]
    fn rejects_missing_core_field_without_questions() {
        let mut value = complete_payload();
        value["audience"] = serde_json::Value::Null;
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("questions 不得为空"), "{err}");
        let mut value = complete_payload();
        value["audience"] = serde_json::Value::Null;
        value["questions"] = serde_json::json!(["目标受众是谁？"]);
        let text = value.to_string();
        let applied = validate_requirement_payload(&text).unwrap();
        assert!(applied.ready_to_create, "类型与标题已确定，其余可留待补全");
        assert!(applied.draft.audience.is_none());
        assert_eq!(applied.draft.questions.len(), 1);
    }

    #[test]
    fn rejects_overlong_fields_and_bad_confidence() {
        let mut value = complete_payload();
        value["topic"] = serde_json::json!("长".repeat(MAX_FIELD_CHARS + 1));
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("过长"), "{err}");
        let mut value = complete_payload();
        value["confidence"] = serde_json::json!(1.2);
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("0 到 1"), "{err}");
        let mut value = complete_payload();
        value["confidence"] = serde_json::json!(true);
        let text = value.to_string();
        let err = validate_requirement_payload(&text).unwrap_err();
        assert!(err.contains("不符合约定结构"), "{err}");
    }

    #[test]
    fn accepts_fenced_payload_and_normalizes_duplicates() {
        let fence = "\u{60}\u{60}\u{60}";
        let mut value = complete_payload();
        value["requirements"] = serde_json::json!([" 只写事实 ", "只写事实", "三点以内"]);
        let text = format!("{fence}json\n{}\n{fence}", value);
        let applied = validate_requirement_payload(&text).unwrap();
        assert_eq!(applied.draft.requirements, vec!["只写事实", "三点以内"]);
    }

    #[test]
    fn rejects_empty_result_text() {
        let err = validate_requirement_payload("   ").unwrap_err();
        assert!(err.contains("为空"), "{err}");
    }
}
