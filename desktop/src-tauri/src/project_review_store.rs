//! 项目复盘存档（OD-PRJ-601）。
//!
//! 边界：
//!   * 一个项目只保留一份最新复盘：`<work_root>/复盘/项目/<project_id>.md`；
//!   * 正文由界面侧模型生成，宿主只校验五段固定标题、引用编号与体量；
//!   * 草稿可覆盖，覆盖前把旧稿备份为 `.bak`；已确认的复盘拒绝覆盖；
//!   * 本模块不创建、不改动任何任务，也不写入项目文件。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::worklog_records::{
    assert_work_root, is_safe_id, split_frontmatter, write_new_atomically,
    write_replace_atomically, yaml_scalar,
};

pub const REVIEW_DIR: &str = "复盘";
pub const PROJECT_REVIEW_DIR: &str = "项目";
pub const PROJECT_REVIEW_SCHEMA: &str = "omnidesk.project-review.v1";
pub const STATUS_DRAFT: &str = "草稿";
pub const STATUS_CONFIRMED: &str = "已确认";

pub const SECTIONS: [&str; 5] = [
    "项目概况",
    "任务完成情况",
    "问题与不足",
    "经验与做法",
    "后续建议",
];

const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ReviewFront {
    #[serde(default)]
    schema: Option<String>,
    project_id: String,
    #[serde(default)]
    task_ids: Vec<String>,
    #[serde(default)]
    generated_at: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    confirmed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectReviewRead {
    pub exists: bool,
    pub path: String,
    pub project_id: String,
    pub task_ids: Vec<String>,
    pub generated_at: Option<String>,
    pub status: Option<String>,
    pub confirmed_at: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectReviewReceipt {
    pub ok: bool,
    pub path: String,
    pub project_id: String,
    pub task_ids: Vec<String>,
    pub generated_at: String,
    pub status: String,
    pub confirmed_at: Option<String>,
    pub backup: Option<String>,
}

pub fn review_dir(root: &Path) -> PathBuf {
    root.join(REVIEW_DIR).join(PROJECT_REVIEW_DIR)
}

/// 复盘文件 `<root>/复盘/项目/<project_id>.md`。项目编号先校验形状。
pub fn review_path(root: &Path, project_id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(project_id) {
        return Err(format!("项目编号不合法：{project_id:?}"));
    }
    Ok(review_dir(root).join(format!("{project_id}.md")))
}

fn validate_task_ids(task_ids: &[String]) -> Result<Vec<String>, String> {
    let mut normalized: Vec<String> = Vec::new();
    for id in task_ids {
        if !is_safe_id(id) {
            return Err(format!("任务编号不合法：{id:?}"));
        }
        if !normalized.iter().any(|existing| existing == id) {
            normalized.push(id.clone());
        }
    }
    Ok(normalized)
}

fn section_title(line: &str) -> Option<&'static str> {
    let value = line.trim_start().strip_prefix("## ")?.trim();
    SECTIONS.iter().find(|title| **title == value).copied()
}

/// 取出正文里的 `#t-…` 任务引用编号；同名只记一次。
fn task_citations(text: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find('#') {
        let tail = &rest[at + 1..];
        let id: String = tail
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
            .collect();
        let skip = id.len().max(1);
        if id.starts_with("t-") && !ids.iter().any(|existing| existing == &id) {
            ids.push(id);
        }
        rest = &tail[skip..];
    }
    ids
}

fn validate_body(content: &str, task_ids: &[String]) -> Result<String, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("项目复盘正文为空，未写入".to_string());
    }
    let body = match split_frontmatter(trimmed) {
        Ok((_front, body)) => body,
        Err(_) => trimmed.to_string(),
    };
    if body.len() > MAX_BODY_BYTES {
        return Err("项目复盘正文过大（超过 1MB），未写入".to_string());
    }

    let mut found: usize = 0;
    for line in body.lines() {
        if line.trim_start().starts_with("## ") {
            let Some(title) = section_title(line) else {
                return Err(format!(
                    "项目复盘出现约定之外的二级标题：{}，未写入",
                    line.trim()
                ));
            };
            let expected = SECTIONS.get(found).copied().unwrap_or("");
            if title != expected {
                return Err(format!("项目复盘段落重复或顺序不对：{title}，未写入"));
            }
            found += 1;
        }
    }
    if found != SECTIONS.len() {
        let missing = SECTIONS.get(found).copied().unwrap_or("");
        return Err(format!(
            "项目复盘缺少段落或段落顺序不对（应为五个固定二级标题）：{missing}，未写入"
        ));
    }

    for id in task_citations(&body) {
        if !task_ids.iter().any(|existing| existing == &id) {
            return Err(format!(
                "项目复盘引用了材料里没有的任务编号：#{id}，未写入"
            ));
        }
    }
    Ok(body.trim().to_string())
}

fn render_review(front: &ReviewFront, body: &str) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!(
        "schema: {}\n",
        yaml_scalar(PROJECT_REVIEW_SCHEMA)
    ));
    out.push_str(&format!(
        "project_id: {}\n",
        yaml_scalar(&front.project_id)
    ));
    if front.task_ids.is_empty() {
        out.push_str("task_ids: []\n");
    } else {
        out.push_str("task_ids:\n");
        for id in &front.task_ids {
            out.push_str(&format!("  - {}\n", yaml_scalar(id)));
        }
    }
    out.push_str(&format!(
        "generated_at: {}\n",
        yaml_scalar(front.generated_at.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "status: {}\n",
        yaml_scalar(front.status.as_deref().unwrap_or(STATUS_DRAFT))
    ));
    if let Some(confirmed_at) = front.confirmed_at.as_deref() {
        out.push_str(&format!("confirmed_at: {}\n", yaml_scalar(confirmed_at)));
    }
    out.push_str("---\n\n");
    out.push_str(body.trim());
    out.push('\n');
    out
}

fn read_document(path: &Path) -> Result<(ReviewFront, String), String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("读取项目复盘失败（{}）：{e}", path.display()))?;
    let (front_raw, body) = split_frontmatter(&raw)?;
    let front: ReviewFront = serde_yaml::from_str(&front_raw)
        .map_err(|e| format!("项目复盘 frontmatter 不是约定字段：{e}"))?;
    if let Some(schema) = front.schema.as_deref() {
        if schema != PROJECT_REVIEW_SCHEMA {
            return Err(format!("项目复盘 schema 不认识：{schema}"));
        }
    }
    Ok((front, body))
}

fn backup_path(target: &Path, now: f64) -> PathBuf {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "project-review.md".to_string());
    let first = dir.join(format!("{name}.bak"));
    if !first.exists() {
        return first;
    }
    let stamp = now.floor() as i64;
    let stamped = dir.join(format!("{name}.bak-{stamp}"));
    if !stamped.exists() {
        return stamped;
    }
    let mut index = 1u64;
    loop {
        let candidate = dir.join(format!("{name}.bak-{stamp}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
        index += 1;
    }
}

fn backup_existing(target: &Path, now: f64) -> Result<Option<PathBuf>, String> {
    if !target.exists() {
        return Ok(None);
    }
    let meta = fs::symlink_metadata(target)
        .map_err(|e| format!("读取旧项目复盘失败（{}）：{e}", target.display()))?;
    if !meta.file_type().is_file() {
        return Err(format!(
            "旧项目复盘不是普通文件，未覆盖：{}",
            target.display()
        ));
    }
    let path = backup_path(target, now);
    fs::copy(target, &path).map_err(|e| {
        format!(
            "备份旧项目复盘失败（{} → {}）：{e}；主文件未改动",
            target.display(),
            path.display()
        )
    })?;
    Ok(Some(path))
}

/// 校验并落盘一份项目复盘草稿；已确认的复盘拒绝覆盖。
pub fn save_project_review(
    root: &Path,
    project_id: &str,
    task_ids: &[String],
    content: &str,
    now: f64,
) -> Result<ProjectReviewReceipt, String> {
    assert_work_root(root)?;
    if !is_safe_id(project_id) {
        return Err(format!("项目编号不合法：{project_id:?}"));
    }
    let task_ids = validate_task_ids(task_ids)?;
    let body = validate_body(content, &task_ids)?;
    let target = review_path(root, project_id)?;

    if target.exists() {
        let (front, _) = read_document(&target)?;
        if front.project_id != project_id {
            return Err(format!(
                "项目复盘文件与项目编号不一致：文件 {project_id}，frontmatter {}",
                front.project_id
            ));
        }
        match front.status.as_deref() {
            Some(STATUS_DRAFT) => {}
            Some(STATUS_CONFIRMED) => {
                return Err(format!(
                    "该项目复盘已确认，不允许重新生成覆盖：{}",
                    target.display()
                ))
            }
            other => {
                return Err(format!(
                    "项目复盘状态不认识（{}），为防覆盖未写入：{}",
                    other.unwrap_or("空"),
                    target.display()
                ))
            }
        }
    }

    fs::create_dir_all(review_dir(root)).map_err(|e| {
        format!(
            "创建项目复盘目录失败（{}）：{e}",
            review_dir(root).display()
        )
    })?;
    let backup = backup_existing(&target, now)?;
    let generated_at = crate::task_shell::utc_iso8601(now);
    let front = ReviewFront {
        schema: Some(PROJECT_REVIEW_SCHEMA.to_string()),
        project_id: project_id.to_string(),
        task_ids: task_ids.clone(),
        generated_at: Some(generated_at.clone()),
        status: Some(STATUS_DRAFT.to_string()),
        confirmed_at: None,
    };
    let rendered = render_review(&front, &body);
    if backup.is_some() {
        write_replace_atomically(&target, rendered.as_bytes())?;
    } else {
        write_new_atomically(&target, rendered.as_bytes())?;
    }

    Ok(ProjectReviewReceipt {
        ok: true,
        path: target.display().to_string(),
        project_id: project_id.to_string(),
        task_ids,
        generated_at,
        status: STATUS_DRAFT.to_string(),
        confirmed_at: None,
        backup: backup.map(|path| path.display().to_string()),
    })
}

/// 读取项目复盘；不存在是合法状态，不建文件。
pub fn read_project_review(root: &Path, project_id: &str) -> Result<ProjectReviewRead, String> {
    assert_work_root(root)?;
    let path = review_path(root, project_id)?;
    if !path.exists() {
        return Ok(ProjectReviewRead {
            exists: false,
            path: path.display().to_string(),
            project_id: project_id.to_string(),
            task_ids: Vec::new(),
            generated_at: None,
            status: None,
            confirmed_at: None,
            body: String::new(),
        });
    }
    let (front, body) = read_document(&path)?;
    if front.project_id != project_id {
        return Err(format!(
            "项目复盘文件与项目编号不一致：文件 {project_id}，frontmatter {}",
            front.project_id
        ));
    }
    let task_ids = validate_task_ids(&front.task_ids)?;
    Ok(ProjectReviewRead {
        exists: true,
        path: path.display().to_string(),
        project_id: project_id.to_string(),
        task_ids,
        generated_at: front.generated_at,
        status: front.status,
        confirmed_at: front.confirmed_at,
        body: body.trim().to_string(),
    })
}

/// 确认草稿：草稿 → 已确认并写 confirmed_at；重复确认幂等。
pub fn confirm_project_review(
    root: &Path,
    project_id: &str,
    now: f64,
) -> Result<ProjectReviewReceipt, String> {
    assert_work_root(root)?;
    let path = review_path(root, project_id)?;
    if !path.exists() {
        return Err(format!("项目复盘不存在，无法确认：{}", path.display()));
    }
    let (mut front, body) = read_document(&path)?;
    if front.project_id != project_id {
        return Err("项目复盘文件与项目编号不一致，未确认".to_string());
    }
    match front.status.as_deref() {
        Some(STATUS_CONFIRMED) => {
            return Ok(ProjectReviewReceipt {
                ok: true,
                path: path.display().to_string(),
                project_id: project_id.to_string(),
                task_ids: front.task_ids,
                generated_at: front.generated_at.unwrap_or_default(),
                status: STATUS_CONFIRMED.to_string(),
                confirmed_at: front.confirmed_at,
                backup: None,
            });
        }
        Some(STATUS_DRAFT) => {}
        other => {
            return Err(format!(
                "项目复盘状态不认识（{}），未确认：{}",
                other.unwrap_or("空"),
                path.display()
            ))
        }
    }
    let confirmed_at = crate::task_shell::utc_iso8601(now);
    front.status = Some(STATUS_CONFIRMED.to_string());
    front.confirmed_at = Some(confirmed_at.clone());
    let rendered = render_review(&front, &body);
    let backup = backup_existing(&path, now)?;
    write_replace_atomically(&path, rendered.as_bytes())?;
    Ok(ProjectReviewReceipt {
        ok: true,
        path: path.display().to_string(),
        project_id: project_id.to_string(),
        task_ids: front.task_ids,
        generated_at: front.generated_at.unwrap_or_default(),
        status: STATUS_CONFIRMED.to_string(),
        confirmed_at: Some(confirmed_at),
        backup: backup.map(|path| path.display().to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-project-review-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
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

    const NOW: f64 = 1_790_000_000.0;

    fn content() -> String {
        SECTIONS
            .iter()
            .map(|title| format!("## {title}\n占位正文。参考 #t-task-a 与 #t-task-b。\n"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn task_ids() -> Vec<String> {
        vec!["t-task-a".to_string(), "t-task-b".to_string()]
    }

    #[test]
    fn save_and_read_round_trip() {
        let temp = TempRoot::new("round-trip");
        let saved = save_project_review(
            temp.root(),
            "p-demo",
            &task_ids(),
            &content(),
            NOW,
        )
        .unwrap();
        assert_eq!(saved.status, STATUS_DRAFT);
        assert_eq!(saved.task_ids, task_ids());
        let read = read_project_review(temp.root(), "p-demo").unwrap();
        assert!(read.exists);
        assert_eq!(read.project_id, "p-demo");
        assert_eq!(read.status.as_deref(), Some(STATUS_DRAFT));
        assert!(read.body.contains("## 项目概况"));
    }

    #[test]
    fn missing_section_is_rejected_without_writing() {
        let temp = TempRoot::new("missing-section");
        let mut body = content();
        body = body.replace("## 问题与不足\n", "");
        let err = save_project_review(temp.root(), "p-demo", &task_ids(), &body, NOW).unwrap_err();
        assert!(err.contains("顺序不对"));
        assert!(!review_path(temp.root(), "p-demo").unwrap().exists());
    }

    #[test]
    fn duplicate_or_out_of_order_section_is_rejected() {
        let temp = TempRoot::new("duplicate-section");
        let body = content() + "\n## 项目概况\n重复段落。\n";
        let err = save_project_review(temp.root(), "p-demo", &task_ids(), &body, NOW).unwrap_err();
        assert!(err.contains("顺序不对"));

        let body = format!(
            "## 任务完成情况\n先写任务。\n## 项目概况\n再写概况。\n## 问题与不足\n无。\n## 经验与做法\n无。\n## 后续建议\n无。\n"
        );
        let err = save_project_review(temp.root(), "p-demo", &task_ids(), &body, NOW).unwrap_err();
        assert!(err.contains("顺序不对"));
    }

    #[test]
    fn citation_outside_material_is_rejected() {
        let temp = TempRoot::new("citation");
        let body = content().replace("#t-task-b", "#t-ghost");
        let err = save_project_review(temp.root(), "p-demo", &["t-task-a".to_string()], &body, NOW)
            .unwrap_err();
        assert!(err.contains("#t-ghost"));
    }

    #[test]
    fn extra_second_level_heading_is_rejected() {
        let temp = TempRoot::new("extra-heading");
        let body = content().replace(
            "## 项目概况\n",
            "## 项目概况\n占位正文。\n## 附录\n附录正文。\n",
        );
        let err = save_project_review(temp.root(), "p-demo", &task_ids(), &body, NOW).unwrap_err();
        assert!(err.contains("约定之外的二级标题"));
    }

    #[test]
    fn draft_overwrite_keeps_bak_backup() {
        let temp = TempRoot::new("backup");
        save_project_review(temp.root(), "p-demo", &task_ids(), &content(), NOW).unwrap();
        let path = review_path(temp.root(), "p-demo").unwrap();
        let first = fs::read_to_string(&path).unwrap();
        let second = content().replace("占位正文", "第二版正文");
        let saved = save_project_review(temp.root(), "p-demo", &task_ids(), &second, NOW + 60.0)
            .unwrap();
        assert!(saved.backup.is_some());
        let backup = PathBuf::from(saved.backup.unwrap());
        assert!(backup.exists());
        assert_eq!(fs::read_to_string(&backup).unwrap(), first);
        let read = read_project_review(temp.root(), "p-demo").unwrap();
        assert!(read.body.contains("第二版正文"));
    }

    #[test]
    fn confirmed_review_is_never_overwritten() {
        let temp = TempRoot::new("confirmed");
        save_project_review(temp.root(), "p-demo", &task_ids(), &content(), NOW).unwrap();
        confirm_project_review(temp.root(), "p-demo", NOW + 10.0).unwrap();
        let err = save_project_review(
            temp.root(),
            "p-demo",
            &task_ids(),
            &content().replace("占位正文", "覆盖正文"),
            NOW + 20.0,
        )
        .unwrap_err();
        assert!(err.contains("已确认"));
        let read = read_project_review(temp.root(), "p-demo").unwrap();
        assert_eq!(read.status.as_deref(), Some(STATUS_CONFIRMED));
        assert!(!read.body.contains("覆盖正文"));
    }

    #[test]
    fn confirm_is_idempotent_and_sets_timestamp() {
        let temp = TempRoot::new("confirm");
        save_project_review(temp.root(), "p-demo", &task_ids(), &content(), NOW).unwrap();
        let first = confirm_project_review(temp.root(), "p-demo", NOW + 30.0).unwrap();
        assert_eq!(first.status, STATUS_CONFIRMED);
        assert!(first.confirmed_at.is_some());
        let second = confirm_project_review(temp.root(), "p-demo", NOW + 90.0).unwrap();
        assert_eq!(second.confirmed_at, first.confirmed_at);
    }

    #[test]
    fn read_missing_does_not_create_files() {
        let temp = TempRoot::new("missing");
        let read = read_project_review(temp.root(), "p-demo").unwrap();
        assert!(!read.exists);
        assert!(!review_dir(temp.root()).exists());
    }

    #[test]
    fn unsafe_project_id_is_rejected() {
        let temp = TempRoot::new("unsafe");
        let err = read_project_review(temp.root(), "../escape").unwrap_err();
        assert!(err.contains("项目编号不合法"));
        let err = save_project_review(temp.root(), "../escape", &task_ids(), &content(), NOW)
            .unwrap_err();
        assert!(err.contains("项目编号不合法"));
    }

    #[test]
    fn empty_body_is_rejected() {
        let temp = TempRoot::new("empty");
        let err = save_project_review(temp.root(), "p-demo", &task_ids(), "   ", NOW).unwrap_err();
        assert!(err.contains("正文为空"));
    }
}
