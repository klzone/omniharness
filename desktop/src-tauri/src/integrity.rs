//! 跨实体引用体检（OD-HAR-222）。
//!
//! 只读、确定性：扫描 work/ 下的记录、任务、素材、项目与排期，报告悬空引用
//! （指向不存在的实体）。不修复、不写文件、不读知识库正文。
//!
//! 输出只含实体编号与字段名，不含标题或正文；问题列表有上限，计数不受限。

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::material_store;
use crate::project_store;
use crate::schedule_store;
use crate::task_store;
use crate::worklog_records;

pub const MAX_ISSUES: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct IntegrityIssue {
    /// record / task / material / schedule
    pub entity: String,
    pub id: String,
    pub field: String,
    pub target: String,
}

#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
pub struct IntegrityReport {
    pub schema: String,
    pub checked: usize,
    pub issues: Vec<IntegrityIssue>,
    pub issue_count: usize,
    pub truncated: bool,
    /// 读取失败、被跳过的实体数（不猜内容）。
    pub skipped: usize,
}

pub fn check(work_root: &Path) -> Result<IntegrityReport, String> {
    let records = worklog_records::list_records(work_root, &worklog_records::RecordFilter::default())?;
    let tasks = task_store::list_tasks(work_root, &task_store::TaskFilter::default())?;
    let projects = project_store::list_projects(work_root)?;
    let materials = material_store::list_materials(work_root, &material_store::MaterialFilter::default())?;
    let schedules = schedule_store::list_entries(work_root)?;

    let record_ids: BTreeSet<&str> = records.items.iter().map(|r| r.id.as_str()).collect();
    let task_ids: BTreeSet<&str> = tasks.items.iter().map(|t| t.id.as_str()).collect();
    let project_ids: BTreeSet<&str> = projects.items.iter().map(|p| p.project_id.as_str()).collect();
    let material_ids: BTreeSet<&str> = materials.items.iter().map(|m| m.material_id.as_str()).collect();

    let mut issues: Vec<IntegrityIssue> = Vec::new();
    let mut issue_count = 0usize;

    let mut push = |entity: &str, id: &str, field: &str, target: &str| {
        issue_count += 1;
        if issues.len() < MAX_ISSUES {
            issues.push(IntegrityIssue {
                entity: entity.to_string(),
                id: id.to_string(),
                field: field.to_string(),
                target: target.to_string(),
            });
        }
    };

    for record in &records.items {
        for task_id in &record.linked_task_ids {
            if !task_ids.contains(task_id.as_str()) {
                push("record", &record.id, "linked_task_ids", task_id);
            }
        }
    }
    for task in &tasks.items {
        if let Some(source) = task.source_record_id.as_deref() {
            if !record_ids.contains(source) {
                push("task", &task.id, "source_record_id", source);
            }
        }
        for record_id in &task.linked_record_ids {
            if !record_ids.contains(record_id.as_str()) {
                push("task", &task.id, "linked_record_ids", record_id);
            }
        }
        if let Some(project_id) = task.project_id.as_deref() {
            if !project_ids.contains(project_id) {
                push("task", &task.id, "project_id", project_id);
            }
        }
        for dep in &task.depends_on {
            if !task_ids.contains(dep.as_str()) {
                push("task", &task.id, "depends_on", dep);
            }
        }
    }
    for material in &materials.items {
        for task_id in &material.related_task_ids {
            if !task_ids.contains(task_id.as_str()) {
                push("material", &material.material_id, "related_task_ids", task_id);
            }
        }
        if let Some(project_id) = material.related_project_id.as_deref() {
            if !project_ids.contains(project_id) {
                push("material", &material.material_id, "related_project_id", project_id);
            }
        }
    }
    for entry in &schedules {
        if !task_ids.contains(entry.task_id.as_str()) {
            push("schedule", &entry.id, "task_id", &entry.task_id);
        }
    }

    Ok(IntegrityReport {
        schema: "omnidesk.integrity.v1".to_string(),
        checked: records.items.len() + tasks.items.len() + materials.items.len() + projects.items.len() + schedules.len(),
        issues,
        issue_count,
        truncated: issue_count > MAX_ISSUES,
        skipped: records.broken + tasks.broken + materials.broken + projects.broken,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "od-har-222-integrity-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn reports_dangling_task_record_link() {
        let root = temp_root("dangling");
        let record = worklog_records::create_record(
            &root,
            &worklog_records::RecordDraft {
                id: None,
                title: Some("合成记录".to_string()),
                record_date: Some("2026-11-12".to_string()),
                linked_task_ids: vec!["t-missing".to_string()],
                body: "合成正文".to_string(),
                ..Default::default()
            },
            1_796_000_000.0,
            480,
        )
        .unwrap();
        let report = check(&root).unwrap();
        assert_eq!(report.issue_count, 1);
        assert_eq!(report.issues[0].entity, "record");
        assert_eq!(report.issues[0].id, record.id);
        assert_eq!(report.issues[0].field, "linked_task_ids");
    }

    #[test]
    fn clean_store_reports_no_issues() {
        let root = temp_root("clean");
        worklog_records::create_record(
            &root,
            &worklog_records::RecordDraft {
                id: None,
                title: Some("合成记录".to_string()),
                record_date: Some("2026-11-12".to_string()),
                body: "合成正文".to_string(),
                ..Default::default()
            },
            1_796_000_000.0,
            480,
        )
        .unwrap();
        let report = check(&root).unwrap();
        assert_eq!(report.issue_count, 0);
        assert!(report.issues.is_empty());
    }
}
