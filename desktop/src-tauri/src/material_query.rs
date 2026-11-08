//! 第四阶段接口：写作素材查询（OD-KM-509）。
//!
//! 纯只读命令：输入写作主题关键词、素材类型、时间范围、项目范围与审核状态。
//! 默认只返回「已确认」素材，显式传入审核状态时按该状态过滤。
//! 关键词只作用于素材标题与正文，沿用 `vault_search` 的同一词法口径：
//! Unicode 大小写不敏感的字面子串匹配，不做分词、正则或模糊匹配；命中给出一行
//! 有界上下文。检索条件不落盘、不外发，也不经过 `gate_user_text`（506 裁决 R5）。
//!
//! 本模块只调用 `material_store` 的公开读取接口，不改变 501–508 的文件结构。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::material_store::{self, MaterialFilter, WritingMaterial, REVIEW_CONFIRMED};
use crate::project_store;
use crate::task_store;

/// 正文命中上下文最多保留的字符数，与 vault_search 一致。
const CONTEXT_CHARS: usize = 120;

/// 查询入参。所有字段可省略；`review_status` 省略或为空时只查已确认素材。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialQueryInput {
    pub keyword: String,
    pub material_type: Option<String>,
    /// 事件日期闭区间，YYYY-MM-DD；只填一端也可。
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub project_id: Option<String>,
    pub review_status: Option<String>,
}

/// 来源四要素：类型、编号、原文引用、引用位置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MaterialQuerySource {
    pub source_type: String,
    pub source_id: String,
    pub source_quote: String,
    pub source_location: Option<String>,
}

/// 关联实体定位。`found=false` 时 `note` 说明原因，不让整条查询失败。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MaterialQueryEntity {
    pub id: String,
    pub found: bool,
    pub title: Option<String>,
    pub location: Option<String>,
    pub note: Option<String>,
}

/// 关联实体：任务 / 项目 / 知识。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MaterialQueryRelations {
    pub tasks: Vec<MaterialQueryEntity>,
    pub project: Option<MaterialQueryEntity>,
    pub knowledge: Vec<MaterialQueryEntity>,
}

/// 关键词命中事实。关键词为空时全部为默认值。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MaterialQueryMatch {
    pub title_hit: bool,
    pub content_hit: bool,
    pub keyword_hits: usize,
    pub snippet: Option<String>,
}

/// 单条素材查询结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MaterialQueryItem {
    pub material_id: String,
    pub title: String,
    pub content: String,
    pub material_type: String,
    pub source: MaterialQuerySource,
    pub review_status: String,
    pub confirmation_status: String,
    pub event_date: Option<String>,
    pub tags: Vec<String>,
    pub related: MaterialQueryRelations,
    pub match_info: MaterialQueryMatch,
}

/// 查询回执。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MaterialQueryResponse {
    /// 归一化后的生效条件；`review_status` 已填入默认「已确认」。
    pub query: MaterialQueryInput,
    pub items: Vec<MaterialQueryItem>,
    pub matched: usize,
    /// 读不出或 frontmatter 不合法的素材文件数。
    pub broken: usize,
}

fn normalized(input: &MaterialQueryInput) -> MaterialQueryInput {
    let trimmed = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    MaterialQueryInput {
        keyword: input.keyword.trim().to_string(),
        material_type: trimmed(&input.material_type),
        date_from: trimmed(&input.date_from),
        date_to: trimmed(&input.date_to),
        project_id: trimmed(&input.project_id),
        // 默认只查已确认素材；显式状态优先于默认值。
        review_status: trimmed(&input.review_status)
            .or_else(|| Some(REVIEW_CONFIRMED.to_string())),
    }
}

fn keyword_context(text: &str, needle: &str) -> Option<String> {
    for line in text.lines() {
        if line.to_lowercase().contains(needle) {
            let trimmed = line.trim();
            let chars: Vec<char> = trimmed.chars().collect();
            let text = if chars.len() > CONTEXT_CHARS {
                let mut head: String = chars[..CONTEXT_CHARS].iter().collect();
                head.push('…');
                head
            } else {
                trimmed.to_string()
            };
            return Some(text);
        }
    }
    None
}

fn count_keyword(text: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let lower = text.to_lowercase();
    if !lower.contains(needle) {
        return 0;
    }
    lower.match_indices(needle).count()
}

fn evaluate_keyword(material: &WritingMaterial, needle: &str) -> MaterialQueryMatch {
    if needle.is_empty() {
        return MaterialQueryMatch::default();
    }
    let title_hit = material.title.to_lowercase().contains(needle);
    let keyword_hits = count_keyword(&material.content, needle);
    MaterialQueryMatch {
        title_hit,
        content_hit: keyword_hits > 0,
        keyword_hits,
        snippet: keyword_context(&material.content, needle),
    }
}

fn relative_work_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn resolve_task(root: &Path, id: &str) -> MaterialQueryEntity {
    let location = task_store::task_path(root, id)
        .ok()
        .map(|path| relative_work_path(root, &path));
    match task_store::read_task(root, id) {
        Ok(task) => MaterialQueryEntity {
            id: id.to_string(),
            found: true,
            title: Some(task.title),
            location,
            note: None,
        },
        Err(error) => MaterialQueryEntity {
            id: id.to_string(),
            found: false,
            title: None,
            location,
            note: Some(error),
        },
    }
}

fn resolve_project(root: &Path, id: &str) -> MaterialQueryEntity {
    let location = project_store::project_path(root, id)
        .ok()
        .map(|path| relative_work_path(root, &path));
    match project_store::read_project(root, id) {
        Ok(project) => MaterialQueryEntity {
            id: id.to_string(),
            found: true,
            title: Some(project.name),
            location,
            note: None,
        },
        Err(error) => MaterialQueryEntity {
            id: id.to_string(),
            found: false,
            title: None,
            location,
            note: Some(error),
        },
    }
}

fn resolve_knowledge(vault: Option<&Path>, id: &str) -> MaterialQueryEntity {
    let Some(vault) = vault else {
        return MaterialQueryEntity {
            id: id.to_string(),
            found: false,
            title: None,
            location: Some(id.to_string()),
            note: Some("知识库未配置或不可读，无法定位知识条目。".to_string()),
        };
    };
    match crate::knowledge_vault::read_vault_note(vault, id) {
        Ok(note) => MaterialQueryEntity {
            id: id.to_string(),
            found: true,
            title: Some(note.file_name),
            location: Some(id.to_string()),
            note: None,
        },
        Err(error) => MaterialQueryEntity {
            id: id.to_string(),
            found: false,
            title: None,
            location: Some(id.to_string()),
            note: Some(error),
        },
    }
}

fn related_entities(
    root: &Path,
    vault: Option<&Path>,
    material: &WritingMaterial,
) -> MaterialQueryRelations {
    MaterialQueryRelations {
        tasks: material
            .related_task_ids
            .iter()
            .map(|id| resolve_task(root, id))
            .collect(),
        project: material
            .related_project_id
            .as_deref()
            .map(|id| resolve_project(root, id)),
        knowledge: material
            .related_knowledge_ids
            .iter()
            .map(|id| resolve_knowledge(vault, id))
            .collect(),
    }
}

fn query_item(
    root: &Path,
    vault: Option<&Path>,
    material: WritingMaterial,
    match_info: MaterialQueryMatch,
) -> MaterialQueryItem {
    let related = related_entities(root, vault, &material);
    MaterialQueryItem {
        material_id: material.material_id,
        title: material.title,
        content: material.content,
        material_type: material.material_type,
        source: MaterialQuerySource {
            source_type: material.source_type,
            source_id: material.source_id,
            source_quote: material.source_quote,
            source_location: material.source_location,
        },
        review_status: material.review_status,
        confirmation_status: material.confirmation_status,
        event_date: material.event_date,
        tags: material.tags,
        related,
        match_info,
    }
}

/// 查询素材。只读：不落盘、不外发、不写审核记录。
pub fn query_materials(
    root: &Path,
    vault: Option<&Path>,
    input: &MaterialQueryInput,
) -> Result<MaterialQueryResponse, String> {
    let query = normalized(input);
    let filter = MaterialFilter {
        material_type: query.material_type.clone(),
        // 归一化后不为空；显式非法状态由 list_materials 统一报错。
        review_status: query.review_status.clone(),
        date_from: query.date_from.clone(),
        date_to: query.date_to.clone(),
        project_id: query.project_id.clone(),
        task_id: None,
    };
    let listing = material_store::list_materials(root, &filter)?;
    let needle = query.keyword.to_lowercase();
    let mut items = Vec::new();
    for material in listing.items {
        let match_info = evaluate_keyword(&material, &needle);
        if !needle.is_empty() && !match_info.title_hit && !match_info.content_hit {
            continue;
        }
        items.push(query_item(root, vault, material, match_info));
    }
    let matched = items.len();
    Ok(MaterialQueryResponse {
        query,
        items,
        matched,
        broken: listing.broken,
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
        vault: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-material-query-{tag}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&base);
            let root = base.join("work");
            let vault = base.join("vault");
            fs::create_dir_all(&root).unwrap();
            fs::create_dir_all(&vault).unwrap();
            Self { base, root, vault }
        }

        fn write_vault(&self, rel: &str, body: &str) {
            let path = self.vault.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, body).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn draft(
        title: &str,
        content: &str,
        material_type: &str,
        source_id: &str,
        event_date: Option<&str>,
    ) -> material_store::MaterialDraft {
        material_store::MaterialDraft {
            title: title.to_string(),
            content: content.to_string(),
            material_type: material_type.to_string(),
            source_type: material_store::SOURCE_USER.to_string(),
            source_id: source_id.to_string(),
            source_quote: content.to_string(),
            source_location: Some("样例位置：第 3 行".to_string()),
            event_date: event_date.map(str::to_string),
            ..Default::default()
        }
    }

    fn confirmed(root: &Path, draft: material_store::MaterialDraft) -> WritingMaterial {
        let material = material_store::create_material(root, &draft, 1_700_000_000.0).unwrap();
        material_store::review_material(
            root,
            &material.material_id,
            material_store::ACTION_CONFIRM,
            None,
            1_700_000_001.0,
        )
        .unwrap()
    }

    fn query(
        fixture: &Fixture,
        input: MaterialQueryInput,
    ) -> Result<MaterialQueryResponse, String> {
        query_materials(&fixture.root, Some(&fixture.vault), &input)
    }

    #[test]
    fn defaults_to_confirmed_and_explicit_status_overrides() {
        let fx = Fixture::new("default-status");
        let pending = material_store::create_material(
            &fx.root,
            &draft("待审核样例", "尚未审核的内容", "工作事实", "src-pending", None),
            1_700_000_000.0,
        )
        .unwrap();
        let confirmed_material = confirmed(
            &fx.root,
            draft("已确认样例", "审核通过的内容", "工作成果", "src-confirmed", None),
        );

        let response = query(&fx, MaterialQueryInput::default()).unwrap();
        assert_eq!(response.query.review_status.as_deref(), Some("已确认"));
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].material_id, confirmed_material.material_id);
        assert!(!response
            .items
            .iter()
            .any(|item| item.material_id == pending.material_id));

        let pending_query = query(
            &fx,
            MaterialQueryInput {
                review_status: Some("待审核".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(pending_query.items.len(), 1);
        assert_eq!(pending_query.items[0].material_id, pending.material_id);
    }

    #[test]
    fn filters_type_date_and_project_together() {
        let fx = Fixture::new("filters");
        let project = project_store::create_project(
            &fx.root,
            &project_store::ProjectDraft {
                project_id: Some("p-demo".to_string()),
                name: "虚构项目甲".to_string(),
                goal: "虚构目标".to_string(),
                ..Default::default()
            },
            1_700_000_000.0,
        )
        .unwrap();
        let mut match_draft = draft("匹配素材", "这是匹配范围的正文", "工作成果", "src-match", Some("2025-03-02"));
        match_draft.related_project_id = Some(project.project_id.clone());
        confirmed(&fx.root, match_draft);

        let mut wrong_type = draft("类型不符", "正文", "工作事实", "src-type", Some("2025-03-02"));
        wrong_type.related_project_id = Some(project.project_id.clone());
        confirmed(&fx.root, wrong_type);

        let mut wrong_date = draft("日期不符", "正文", "工作成果", "src-date", Some("2025-02-01"));
        wrong_date.related_project_id = Some(project.project_id.clone());
        confirmed(&fx.root, wrong_date);

        let mut wrong_project = draft("项目不符", "正文", "工作成果", "src-project", Some("2025-03-02"));
        wrong_project.related_project_id = Some("p-other".to_string());
        confirmed(&fx.root, wrong_project);

        let response = query(
            &fx,
            MaterialQueryInput {
                material_type: Some("工作成果".to_string()),
                date_from: Some("2025-03-01".to_string()),
                date_to: Some("2025-03-31".to_string()),
                project_id: Some(project.project_id.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].title, "匹配素材");
    }

    #[test]
    fn keyword_matches_title_and_content_case_insensitively() {
        let fx = Fixture::new("keyword");
        let content_hit = confirmed(
            &fx.root,
            draft("内容命中样例", "本季 Growth Plan 已经完成。", "工作事实", "src-content", None),
        );
        let title_hit = confirmed(
            &fx.root,
            draft("Brand Voice 指南", "正文不含另一个词。", "经验做法", "src-title", None),
        );
        let no_hit = confirmed(
            &fx.root,
            draft("无关样例", "完全不同的正文。", "工作事实", "src-none", None),
        );

        let response = query(
            &fx,
            MaterialQueryInput {
                keyword: "  growth  ".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(response.query.keyword, "growth");
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].material_id, content_hit.material_id);
        assert!(response.items[0].match_info.content_hit);
        assert_eq!(response.items[0].match_info.keyword_hits, 1);
        assert!(response.items[0]
            .match_info
            .snippet
            .as_deref()
            .unwrap()
            .contains("Growth Plan"));

        let title_response = query(
            &fx,
            MaterialQueryInput {
                keyword: "brand".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(title_response.items.len(), 1);
        assert_eq!(title_response.items[0].material_id, title_hit.material_id);
        assert!(title_response.items[0].match_info.title_hit);
        assert!(!title_response.items[0].match_info.content_hit);

        let missing = query(
            &fx,
            MaterialQueryInput {
                keyword: "不存在的主题".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(missing.items.is_empty());
        assert_eq!(missing.matched, 0);
        assert!(!missing.items.iter().any(|item| item.material_id == no_hit.material_id));
    }

    #[test]
    fn keyword_query_does_not_rewrite_material_files() {
        let fx = Fixture::new("readonly");
        let material = confirmed(
            &fx.root,
            draft("只读核对", "主题词出现在正文", "工作事实", "src-readonly", None),
        );
        let path = material_store::material_path(&fx.root, &material.material_id).unwrap();
        let before = fs::read(&path).unwrap();
        query(
            &fx,
            MaterialQueryInput {
                keyword: "主题词".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        let after = fs::read(&path).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn scene12_payload_contains_content_source_location_status_and_related_entities() {
        let fx = Fixture::new("scene12");
        let project = project_store::create_project(
            &fx.root,
            &project_store::ProjectDraft {
                project_id: Some("p-scene12".to_string()),
                name: "虚构项目乙".to_string(),
                goal: "虚构项目目标".to_string(),
                ..Default::default()
            },
            1_700_000_000.0,
        )
        .unwrap();
        let task = task_store::create_task(
            &fx.root,
            &task_store::TaskDraft {
                id: Some("t-scene12".to_string()),
                title: "虚构任务乙".to_string(),
                project_id: Some(project.project_id.clone()),
                ..Default::default()
            },
            1_700_000_000.0,
        )
        .unwrap();
        fx.write_vault("topic.md", "# 虚构知识\n\n这是虚构知识条目。\n");

        let mut material_draft = draft(
            "场景十二素材",
            "虚构正文：季度简报完成了三件虚构工作。",
            "工作成果",
            "record-scene12",
            Some("2025-03-10"),
        );
        material_draft.source_type = material_store::SOURCE_WORK_RECORD.to_string();
        material_draft.source_quote = "季度简报完成了三件虚构工作".to_string();
        material_draft.source_location = Some("虚构记录第 5 行".to_string());
        material_draft.related_task_ids = vec![task.id.clone()];
        material_draft.related_project_id = Some(project.project_id.clone());
        material_draft.related_knowledge_ids = vec!["topic.md".to_string()];
        let material = confirmed(&fx.root, material_draft);

        let response = query(
            &fx,
            MaterialQueryInput {
                keyword: "季度简报".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(response.items.len(), 1);
        let item = &response.items[0];
        assert_eq!(item.material_id, material.material_id);
        assert_eq!(item.content, "虚构正文：季度简报完成了三件虚构工作。");
        assert_eq!(item.source.source_type, material_store::SOURCE_WORK_RECORD);
        assert_eq!(item.source.source_id, "record-scene12");
        assert_eq!(item.source.source_quote, "季度简报完成了三件虚构工作");
        assert_eq!(item.source.source_location.as_deref(), Some("虚构记录第 5 行"));
        assert_eq!(item.review_status, "已确认");
        assert_eq!(item.related.tasks.len(), 1);
        assert!(item.related.tasks[0].found);
        assert_eq!(item.related.project.as_ref().unwrap().title.as_deref(), Some("虚构项目乙"));
        assert_eq!(item.related.knowledge.len(), 1);
        assert!(item.related.knowledge[0].found);

        let payload = serde_json::to_value(item).unwrap();
        for key in ["content", "source", "review_status", "related"] {
            assert!(payload.get(key).is_some(), "payload 缺少字段 {key}");
        }
        for key in ["source_type", "source_id", "source_quote", "source_location"] {
            assert!(
                payload["source"].get(key).is_some(),
                "source 缺少字段 {key}"
            );
        }
        for key in ["tasks", "project", "knowledge"] {
            assert!(
                payload["related"].get(key).is_some(),
                "related 缺少字段 {key}"
            );
        }
    }

    #[test]
    fn invalid_filters_are_rejected_and_broken_files_are_counted() {
        let fx = Fixture::new("invalid");
        confirmed(
            &fx.root,
            draft("有效素材", "正文", "工作事实", "src-valid", None),
        );
        fs::create_dir_all(material_store::material_dir(&fx.root)).unwrap();
        fs::write(material_store::material_dir(&fx.root).join("broken.md"), "不是素材文件\n")
            .unwrap();

        let unknown_status = query(
            &fx,
            MaterialQueryInput {
                review_status: Some("不存在的状态".to_string()),
                ..Default::default()
            },
        );
        assert!(unknown_status.is_err());

        let unknown_type = query(
            &fx,
            MaterialQueryInput {
                material_type: Some("不存在的类型".to_string()),
                ..Default::default()
            },
        );
        assert!(unknown_type.is_err());

        let reversed_range = query(
            &fx,
            MaterialQueryInput {
                date_from: Some("2025-04-01".to_string()),
                date_to: Some("2025-03-01".to_string()),
                ..Default::default()
            },
        );
        assert!(reversed_range.is_err());

        let response = query(&fx, MaterialQueryInput::default()).unwrap();
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.broken, 1);
    }

    #[test]
    fn missing_related_entities_are_reported_without_failing() {
        let fx = Fixture::new("missing-related");
        let mut material_draft = draft(
            "关联缺失样例",
            "正文",
            "工作事实",
            "src-missing",
            None,
        );
        material_draft.related_task_ids = vec!["t-missing".to_string()];
        material_draft.related_project_id = Some("p-missing".to_string());
        material_draft.related_knowledge_ids = vec!["missing.md".to_string()];
        let material = confirmed(&fx.root, material_draft);

        let response = query(&fx, MaterialQueryInput::default()).unwrap();
        assert_eq!(response.items.len(), 1);
        let item = &response.items[0];
        assert_eq!(item.material_id, material.material_id);
        assert!(!item.related.tasks[0].found);
        assert!(item.related.tasks[0].note.is_some());
        assert!(!item.related.project.as_ref().unwrap().found);
        assert!(!item.related.knowledge[0].found);
    }
}
