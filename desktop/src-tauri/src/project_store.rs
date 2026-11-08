//! 最小项目实体（OD-KM-502）。
//!
//! 每个项目一个 Markdown 文件：<work_root>/项目/<project_id>.md。
//! frontmatter 存项目字段，文件没有正文；项目跟踪事项表仍由 project_items.rs
//! 独立维护，本模块不读取、不写入那张表。
//! 名称、状态、起止日期都是有限规则：名称非空，更新改名时不与既有项目重名，
//! 状态只允许 进行中 / 已完成 / 已搁置，日期只接受 YYYY-MM-DD 且开始不晚于结束。
//!
//! 反向派生查询 project_refs 是纯函数：不落盘、不读写文件，只把任务、素材与
//! 工作记录按 project_id 分组，供后续 IPC 层组装项目概览。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::worklog_records::{
    assert_work_root, is_date_shape, is_safe_id, new_file_id, normalize_opt, split_frontmatter,
    write_new_atomically, write_replace_atomically, yaml_opt, yaml_scalar,
};

pub const PROJECT_SCHEMA: &str = "omnidesk.project.v1";
pub const PROJECT_DIR: &str = "项目";

pub const STATUS_ACTIVE: &str = "进行中";
pub const STATUS_DONE: &str = "已完成";
pub const STATUS_PAUSED: &str = "已搁置";
pub const PROJECT_STATUSES: &[&str] = &[STATUS_ACTIVE, STATUS_DONE, STATUS_PAUSED];

const MAX_ID_CHARS: usize = 80;
const MAX_NAME_CHARS: usize = 200;
const MAX_GOAL_CHARS: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub schema: String,
    pub project_id: String,
    pub name: String,
    pub goal: String,
    pub status: String,
    /// YYYY-MM-DD；未设置时为 null。
    pub start: Option<String>,
    /// YYYY-MM-DD；未设置时为 null。
    pub end: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectDraft {
    pub project_id: Option<String>,
    pub name: String,
    pub goal: String,
    /// 省略时默认「进行中」。
    pub status: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

/// 更新补丁。None = 不修改；空字符串 = 清空可选项。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub goal: Option<String>,
    pub status: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectListing {
    pub items: Vec<Project>,
    pub broken: usize,
}

/// 反向派生查询的单个关联条目。project_id 为空表示不关联任何项目。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectRefItem {
    pub project_id: Option<String>,
    pub id: String,
    pub title: String,
}

/// 反向派生查询输入：三类实体各自带 project_id 字段。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectRefsInput {
    pub tasks: Vec<ProjectRefItem>,
    pub materials: Vec<ProjectRefItem>,
    pub records: Vec<ProjectRefItem>,
}

/// 按 project_id 分组的关联清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectRefsGroup {
    pub project_id: String,
    pub tasks: Vec<ProjectRefItem>,
    pub materials: Vec<ProjectRefItem>,
    pub records: Vec<ProjectRefItem>,
}

pub fn project_dir(root: &Path) -> PathBuf {
    root.join(PROJECT_DIR)
}

pub fn project_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(id) || id.chars().count() > MAX_ID_CHARS {
        return Err(format!("项目编号不合法：{id:?}"));
    }
    Ok(project_dir(root).join(format!("{id}.md")))
}

fn validate_status(value: &str) -> Result<(), String> {
    if !PROJECT_STATUSES.contains(&value) {
        return Err(format!("项目状态不认识：{value}"));
    }
    Ok(())
}

fn validate_dates(start: Option<&String>, end: Option<&String>) -> Result<(), String> {
    for (value, field) in [(start, "开始日期"), (end, "结束日期")] {
        if let Some(date) = value {
            if !is_date_shape(date) {
                return Err(format!("{field}形状不正确：{date}"));
            }
        }
    }
    if let (Some(start), Some(end)) = (start, end) {
        if start > end {
            return Err("开始日期不能晚于结束日期。".to_string());
        }
    }
    Ok(())
}

fn validate_project(project: &Project) -> Result<(), String> {
    if project.schema != PROJECT_SCHEMA {
        return Err(format!("项目 schema 不认识：{}", project.schema));
    }
    if !is_safe_id(&project.project_id) || project.project_id.chars().count() > MAX_ID_CHARS {
        return Err(format!("项目编号不合法：{:?}", project.project_id));
    }
    if project.name.trim().is_empty() {
        return Err("项目名称不能为空。".to_string());
    }
    if project.name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("项目名称过长（最多 {MAX_NAME_CHARS} 字）。"));
    }
    if project.goal.chars().count() > MAX_GOAL_CHARS {
        return Err(format!("项目目标过长（最多 {MAX_GOAL_CHARS} 字）。"));
    }
    validate_status(&project.status)?;
    validate_dates(project.start.as_ref(), project.end.as_ref())?;
    Ok(())
}

fn render_project(project: &Project) -> String {
    let mut fm = String::new();
    fm.push_str(&format!("schema: {}\n", yaml_scalar(&project.schema)));
    fm.push_str(&format!(
        "project_id: {}\n",
        yaml_scalar(&project.project_id)
    ));
    fm.push_str(&format!("name: {}\n", yaml_scalar(&project.name)));
    fm.push_str(&format!("goal: {}\n", yaml_scalar(&project.goal)));
    fm.push_str(&format!("status: {}\n", yaml_scalar(&project.status)));
    fm.push_str(&format!("start: {}\n", yaml_opt(&project.start)));
    fm.push_str(&format!("end: {}\n", yaml_opt(&project.end)));
    fm.push_str(&format!(
        "created_at: {}\n",
        yaml_scalar(&project.created_at)
    ));
    fm.push_str(&format!(
        "updated_at: {}\n",
        yaml_scalar(&project.updated_at)
    ));
    format!("---\n{fm}---\n")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFront {
    #[serde(default)]
    schema: Option<String>,
    project_id: String,
    name: String,
    #[serde(default)]
    goal: String,
    status: String,
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    end: Option<String>,
    created_at: String,
    updated_at: String,
}

fn parse_project(raw: &str) -> Result<Project, String> {
    let (front_raw, _body) = split_frontmatter(raw)?;
    let front: ProjectFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("项目 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != PROJECT_SCHEMA {
            return Err(format!("项目 schema 不认识：{schema}"));
        }
    }
    let project = Project {
        schema: PROJECT_SCHEMA.to_string(),
        project_id: front.project_id,
        name: front.name,
        goal: front.goal,
        status: front.status,
        start: normalize_opt(front.start),
        end: normalize_opt(front.end),
        created_at: front.created_at,
        updated_at: front.updated_at,
    };
    validate_project(&project)?;
    Ok(project)
}

fn read_all(root: &Path) -> Result<(Vec<Project>, usize), String> {
    assert_work_root(root)?;
    let dir = project_dir(root);
    let mut items = Vec::new();
    let mut broken = 0usize;
    if !dir.exists() {
        return Ok((items, broken));
    }
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取项目目录失败（{}）：{}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取项目目录项失败：{e}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        match fs::read_to_string(&path).and_then(|raw| {
            parse_project(&raw).map_err(|error| std::io::Error::other(error))
        }) {
            Ok(project) => items.push(project),
            Err(_) => broken += 1,
        }
    }
    Ok((items, broken))
}

fn persist(root: &Path, project: &Project) -> Result<(), String> {
    write_replace_atomically(
        &project_path(root, &project.project_id)?,
        render_project(project).as_bytes(),
    )
}

/// 名称冲突检查：与其它既有项目的名称（去首尾空白后）相同即冲突。
fn name_conflict(root: &Path, name: &str, exclude_id: &str) -> Result<Option<String>, String> {
    let (items, _) = read_all(root)?;
    for project in items {
        if project.project_id == exclude_id {
            continue;
        }
        if project.name.trim() == name {
            return Ok(Some(project.project_id));
        }
    }
    Ok(None)
}

pub fn create_project(
    root: &Path,
    draft: &ProjectDraft,
    now: f64,
) -> Result<Project, String> {
    assert_work_root(root)?;
    let name = draft.name.trim().to_string();
    let goal = draft.goal.trim().to_string();
    let status = draft
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(STATUS_ACTIVE)
        .to_string();
    let id = match draft.project_id.clone().map(|value| value.trim().to_string()) {
        Some(id) if !id.is_empty() => {
            if !is_safe_id(&id) || id.chars().count() > MAX_ID_CHARS {
                return Err(format!("项目编号不合法：{id:?}"));
            }
            if project_path(root, &id)?.exists() {
                return Err(format!("项目已存在：{id}"));
            }
            id
        }
        _ => new_file_id("p", now),
    };
    let stamp = crate::task_shell::utc_iso8601(now);
    let project = Project {
        schema: PROJECT_SCHEMA.to_string(),
        project_id: id.clone(),
        name,
        goal,
        status,
        start: normalize_opt(draft.start.clone()),
        end: normalize_opt(draft.end.clone()),
        created_at: stamp.clone(),
        updated_at: stamp,
    };
    validate_project(&project)?;
    fs::create_dir_all(project_dir(root))
        .map_err(|e| format!("创建项目目录失败：{e}"))?;
    write_new_atomically(
        &project_path(root, &id)?,
        render_project(&project).as_bytes(),
    )?;
    Ok(project)
}

pub fn read_project(root: &Path, id: &str) -> Result<Project, String> {
    assert_work_root(root)?;
    let path = project_path(root, id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取项目失败（{}）：{}", path.display(), e))?;
    parse_project(&raw)
}

pub fn update_project(
    root: &Path,
    id: &str,
    patch: &ProjectPatch,
    now: f64,
) -> Result<Project, String> {
    let mut project = read_project(root, id)?;
    if let Some(value) = patch.name.clone() {
        let name = value.trim().to_string();
        if let Some(existing) = name_conflict(root, &name, id)? {
            return Err(format!("项目名称与既有项目重复（{existing}）：{name}"));
        }
        project.name = name;
    }
    if let Some(value) = patch.goal.clone() {
        project.goal = value.trim().to_string();
    }
    if let Some(value) = patch.status.clone() {
        project.status = value.trim().to_string();
    }
    if let Some(value) = patch.start.clone() {
        project.start = normalize_opt(Some(value));
    }
    if let Some(value) = patch.end.clone() {
        project.end = normalize_opt(Some(value));
    }
    project.updated_at = crate::task_shell::utc_iso8601(now);
    validate_project(&project)?;
    persist(root, &project)?;
    Ok(project)
}

pub fn list_projects(root: &Path) -> Result<ProjectListing, String> {
    assert_work_root(root)?;
    let (mut items, broken) = read_all(root)?;
    items.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.project_id.cmp(&b.project_id))
    });
    Ok(ProjectListing { items, broken })
}

fn push_ref_kind(
    groups: &mut BTreeMap<String, ProjectRefsGroup>,
    item: &ProjectRefItem,
    push: impl FnOnce(&mut ProjectRefsGroup, ProjectRefItem),
) {
    let Some(project_id) = item
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let group = groups
        .entry(project_id.to_string())
        .or_insert_with(|| ProjectRefsGroup {
            project_id: project_id.to_string(),
            tasks: Vec::new(),
            materials: Vec::new(),
            records: Vec::new(),
        });
    let normalized = ProjectRefItem {
        project_id: Some(project_id.to_string()),
        id: item.id.trim().to_string(),
        title: item.title.trim().to_string(),
    };
    push(group, normalized);
}

/// 反向派生查询骨架：纯函数，不读写磁盘，按 project_id 分组返回关联清单。
/// 项目编号为空或只有空白的输入不进入任何分组；分组顺序按 project_id 升序。
pub fn project_refs(input: &ProjectRefsInput) -> Vec<ProjectRefsGroup> {
    let mut groups: BTreeMap<String, ProjectRefsGroup> = BTreeMap::new();
    for item in &input.tasks {
        push_ref_kind(&mut groups, item, |group, item| group.tasks.push(item));
    }
    for item in &input.materials {
        push_ref_kind(&mut groups, item, |group, item| group.materials.push(item));
    }
    for item in &input.records {
        push_ref_kind(&mut groups, item, |group, item| group.records.push(item));
    }
    groups.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: f64 = 1_795_000_000.0;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-km502-project-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn root(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn draft(name: &str) -> ProjectDraft {
        ProjectDraft {
            name: name.to_string(),
            goal: "合成项目目标。".to_string(),
            ..Default::default()
        }
    }

    fn create(temp: &TempRoot, name: &str) -> Project {
        create_project(temp.root(), &draft(name), NOW).unwrap()
    }

    #[test]
    fn create_writes_frontmatter_with_schema_and_defaults() {
        let temp = TempRoot::new("create");
        let project = create(&temp, "试点项目");
        let raw =
            fs::read_to_string(project_path(temp.root(), &project.project_id).unwrap()).unwrap();
        assert!(raw.contains("schema: \"omnidesk.project.v1\""), "{raw}");
        assert!(raw.contains("name: \"试点项目\""), "{raw}");
        assert_eq!(project.status, STATUS_ACTIVE);
        assert_eq!(project.start, None);
        assert_eq!(project.end, None);
        assert!(project.project_id.starts_with("p-"));
    }

    #[test]
    fn create_rejects_empty_name() {
        let temp = TempRoot::new("empty-name");
        let err = create_project(temp.root(), &draft("   "), NOW).unwrap_err();
        assert!(err.contains("项目名称不能为空"), "{err}");
    }

    #[test]
    fn create_rejects_unknown_status() {
        let temp = TempRoot::new("bad-status");
        let mut input = draft("状态项目");
        input.status = Some("已取消".to_string());
        let err = create_project(temp.root(), &input, NOW).unwrap_err();
        assert!(err.contains("项目状态不认识"), "{err}");
    }

    #[test]
    fn create_rejects_bad_date_shape() {
        let temp = TempRoot::new("bad-date");
        let mut input = draft("日期项目");
        input.start = Some("2026/11/09".to_string());
        let err = create_project(temp.root(), &input, NOW).unwrap_err();
        assert!(err.contains("开始日期形状不正确"), "{err}");
        let mut input = draft("日期项目二");
        input.end = Some("2026-13-01".to_string());
        let err = create_project(temp.root(), &input, NOW).unwrap_err();
        assert!(err.contains("结束日期形状不正确"), "{err}");
    }

    #[test]
    fn create_rejects_start_after_end() {
        let temp = TempRoot::new("date-order");
        let mut input = draft("倒序日期项目");
        input.start = Some("2026-12-01".to_string());
        input.end = Some("2026-11-01".to_string());
        let err = create_project(temp.root(), &input, NOW).unwrap_err();
        assert!(err.contains("开始日期不能晚于结束日期"), "{err}");
    }

    #[test]
    fn create_rejects_supplied_id_collision() {
        let temp = TempRoot::new("id-collision");
        let mut input = draft("第一个项目");
        input.project_id = Some("p-fixed".to_string());
        create_project(temp.root(), &input, NOW).unwrap();
        let mut second = draft("第二个项目");
        second.project_id = Some("p-fixed".to_string());
        let err = create_project(temp.root(), &second, NOW).unwrap_err();
        assert!(err.contains("项目已存在"), "{err}");
    }

    #[test]
    fn read_round_trips_optional_fields() {
        let temp = TempRoot::new("roundtrip");
        let mut input = draft("完整项目");
        input.status = Some(STATUS_PAUSED.to_string());
        input.start = Some("2026-11-01".to_string());
        input.end = Some("2026-12-31".to_string());
        let created = create_project(temp.root(), &input, NOW).unwrap();
        let loaded = read_project(temp.root(), &created.project_id).unwrap();
        assert_eq!(loaded, created);
        assert_eq!(loaded.status, STATUS_PAUSED);
        assert_eq!(loaded.start.as_deref(), Some("2026-11-01"));
        assert_eq!(loaded.end.as_deref(), Some("2026-12-31"));
    }

    #[test]
    fn update_changes_fields_but_keeps_id_and_created_at() {
        let temp = TempRoot::new("update");
        let created = create(&temp, "原项目");
        let patch = ProjectPatch {
            name: Some("改后项目".to_string()),
            goal: Some("新的合成目标。".to_string()),
            status: Some(STATUS_DONE.to_string()),
            start: Some("2026-01-02".to_string()),
            end: Some("2026-03-04".to_string()),
        };
        let updated = update_project(temp.root(), &created.project_id, &patch, NOW + 60.0).unwrap();
        assert_eq!(updated.project_id, created.project_id);
        assert_eq!(updated.created_at, created.created_at);
        assert_ne!(updated.updated_at, created.updated_at);
        assert_eq!(updated.name, "改后项目");
        assert_eq!(updated.status, STATUS_DONE);
        assert_eq!(updated.start.as_deref(), Some("2026-01-02"));
        assert_eq!(updated.end.as_deref(), Some("2026-03-04"));
        assert_eq!(read_project(temp.root(), &created.project_id).unwrap(), updated);
    }

    #[test]
    fn update_rejects_name_conflict_with_existing() {
        let temp = TempRoot::new("name-conflict");
        let first = create(&temp, "项目甲");
        let second = create(&temp, "项目乙");
        let err = update_project(
            temp.root(),
            &second.project_id,
            &ProjectPatch {
                name: Some("项目甲".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap_err();
        assert!(err.contains("项目名称与既有项目重复"), "{err}");
        assert_eq!(
            read_project(temp.root(), &second.project_id).unwrap().name,
            "项目乙"
        );
        assert_eq!(read_project(temp.root(), &first.project_id).unwrap().name, "项目甲");
    }

    #[test]
    fn update_allows_same_name_with_surrounding_spaces() {
        let temp = TempRoot::new("same-name");
        let project = create(&temp, "同名项目");
        let updated = update_project(
            temp.root(),
            &project.project_id,
            &ProjectPatch {
                name: Some("  同名项目  ".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(updated.name, "同名项目");
    }

    #[test]
    fn update_missing_project_is_reported() {
        let temp = TempRoot::new("missing");
        let err = update_project(
            temp.root(),
            "p-missing",
            &ProjectPatch::default(),
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("读取项目失败"), "{err}");
    }

    #[test]
    fn list_reads_valid_files_and_counts_broken() {
        let temp = TempRoot::new("list");
        let first = create(&temp, "甲项目");
        let second = create(&temp, "乙项目");
        fs::write(project_dir(temp.root()).join("broken.md"), "not a project").unwrap();
        let listing = list_projects(temp.root()).unwrap();
        assert_eq!(listing.items.len(), 2);
        assert_eq!(listing.broken, 1);
        assert!(listing
            .items
            .iter()
            .any(|project| project.project_id == first.project_id));
        assert!(listing
            .items
            .iter()
            .any(|project| project.project_id == second.project_id));
    }

    #[test]
    fn project_refs_groups_by_project_and_skips_unassigned() {
        let input = ProjectRefsInput {
            tasks: vec![
                ProjectRefItem {
                    project_id: Some("p-001".to_string()),
                    id: "t-001".to_string(),
                    title: "任务一".to_string(),
                },
                ProjectRefItem {
                    project_id: None,
                    id: "t-002".to_string(),
                    title: "无项目任务".to_string(),
                },
                ProjectRefItem {
                    project_id: Some("  ".to_string()),
                    id: "t-003".to_string(),
                    title: "空白项目任务".to_string(),
                },
            ],
            materials: vec![ProjectRefItem {
                project_id: Some(" p-002 ".to_string()),
                id: "m-001".to_string(),
                title: "素材一".to_string(),
            }],
            records: vec![ProjectRefItem {
                project_id: Some("p-001".to_string()),
                id: "r-001".to_string(),
                title: "记录一".to_string(),
            }],
        };
        let groups = project_refs(&input);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].project_id, "p-001");
        assert_eq!(groups[0].tasks.len(), 1);
        assert_eq!(groups[0].records.len(), 1);
        assert!(groups[0].materials.is_empty());
        assert_eq!(groups[1].project_id, "p-002");
        assert_eq!(groups[1].materials.len(), 1);
        assert_eq!(groups[1].materials[0].project_id.as_deref(), Some("p-002"));
    }

    #[test]
    fn project_refs_keeps_multiple_items_per_kind_in_input_order() {
        let input = ProjectRefsInput {
            tasks: vec![
                ProjectRefItem {
                    project_id: Some("p-001".to_string()),
                    id: "t-002".to_string(),
                    title: "任务二".to_string(),
                },
                ProjectRefItem {
                    project_id: Some("p-001".to_string()),
                    id: "t-001".to_string(),
                    title: "任务一".to_string(),
                },
            ],
            ..Default::default()
        };
        let groups = project_refs(&input);
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].tasks.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            vec!["t-002", "t-001"]
        );
        assert!(groups[0].materials.is_empty());
        assert!(groups[0].records.is_empty());
    }
}
