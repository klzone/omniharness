//! 报告与成果归档进知识库（OD-WR-312）。
//!
//! 报告复制进 <vault_root>/报告/，成果复制进 <vault_root>/归档成果/。
//! 目标同名同来源幂等；同名不同来源拒绝覆盖。frontmatter 记录来源周期或文件名
//! 与来源哈希，用户可在 Obsidian 直接追溯。

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::sha256::sha256_hex;
use crate::task_shell;
use crate::worklog;
use crate::worklog_records::{write_new_atomically, yaml_scalar};

pub const REPORT_ARCHIVE_DIR: &str = "报告";
pub const ARTIFACT_ARCHIVE_DIR: &str = "归档成果";
const MAX_NAME_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveReceipt {
    pub ok: bool,
    pub target_path: String,
    pub bytes: usize,
    pub source_sha256: String,
    pub source_period: Option<String>,
    pub already_archived: bool,
    pub error: Option<String>,
}

fn assert_archive_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("归档文件名不能为空。".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err("归档文件名过长。".to_string());
    }
    if name.starts_with('.') || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("归档文件名不合法：{name:?}"));
    }
    if !name.ends_with(".md") {
        return Err(format!("只归档 Markdown 文件：{name}"));
    }
    Ok(())
}

fn assert_vault_dir(root: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(root)
        .map_err(|e| format!("知识库目录不可用（{}）：{}", root.display(), e))?;
    if !meta.file_type().is_dir() {
        return Err(format!("知识库根不是目录：{}", root.display()));
    }
    Ok(())
}

/// 读取正文开头 frontmatter 里的一个字段；只认第一段围栏。
pub fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    let text = content.strip_prefix("\u{feff}").unwrap_or(content);
    let mut lines = text.lines();
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    for line in lines {
        if line.trim_end() == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix(&format!("{key}:")) {
            let value = rest.trim();
            return Some(
                serde_json::from_str::<String>(value)
                    .unwrap_or_else(|_| value.trim_matches('"').to_string()),
            );
        }
    }
    None
}

fn render_fields(fields: &[(String, String)]) -> String {
    fields
        .iter()
        .map(|(key, value)| format!("{key}: {}", yaml_scalar(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 给已有 frontmatter 合并字段，或给无 frontmatter 的正文补一段。
fn with_archive_fields(
    content: &str,
    fields: &[(String, String)],
) -> Result<String, String> {
    let text = content.strip_prefix("\u{feff}").unwrap_or(content);
    let stripped = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"));
    let Some(body) = stripped else {
        return Ok(format!("---\n{}\n---\n\n{text}", render_fields(fields)));
    };
    let Some(end) = body.find("\n---") else {
        return Err("正文 frontmatter 没有闭合围栏，拒绝归档。".to_string());
    };
    let meta = &body[..end];
    let mut lines: Vec<String> = meta
        .lines()
        .filter(|line| {
            !fields
                .iter()
                .any(|(key, _)| line.starts_with(&format!("{key}:")))
        })
        .map(str::to_string)
        .collect();
    lines.extend(fields.iter().map(|(key, value)| format!("{key}: {}", yaml_scalar(value))));
    let after = &body[end + 4..];
    Ok(format!("---\n{}\n---{after}", lines.join("\n")))
}

fn archive_text(
    vault_root: &Path,
    subdir: &str,
    file_name: &str,
    target_content: &str,
    source_hash: &str,
    period: Option<&str>,
) -> Result<ArchiveReceipt, String> {
    assert_vault_dir(vault_root)?;
    assert_archive_name(file_name)?;
    let root_real = crate::app_paths::resolve_real_path(vault_root)?;
    let dir = vault_root.join(subdir);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("创建归档目录失败（{}）：{}", dir.display(), e))?;
    let dir_real = crate::app_paths::resolve_real_path(&dir)?;
    if !dir_real.starts_with(&root_real) {
        return Err(format!("归档子目录落在知识库之外，拒绝写入：{}", dir.display()));
    }
    let target = dir.join(file_name);
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.file_type().is_symlink() {
            return Err(format!("归档目标是符号链接，拒绝写入：{}", target.display()));
        }
        if !meta.file_type().is_file() {
            return Err(format!("归档目标不是普通文件：{}", target.display()));
        }
    }
    if target.exists() {
        let existing = fs::read_to_string(&target)
            .map_err(|e| format!("读取已归档文件失败（{}）：{}", target.display(), e))?;
        let existing_hash = frontmatter_value(&existing, "source_sha256");
        if existing_hash.as_deref() == Some(source_hash) {
            return Ok(ArchiveReceipt {
                ok: true,
                target_path: target.display().to_string(),
                bytes: existing.len(),
                source_sha256: source_hash.to_string(),
                source_period: period.map(str::to_string),
                already_archived: true,
                error: None,
            });
        }
        return Err(format!(
            "归档目标已存在且来源不同，拒绝覆盖：{}",
            target.display()
        ));
    }
    write_new_atomically(&target, target_content.as_bytes())?;
    Ok(ArchiveReceipt {
        ok: true,
        target_path: target.display().to_string(),
        bytes: target_content.len(),
        source_sha256: source_hash.to_string(),
        source_period: period.map(str::to_string),
        already_archived: false,
        error: None,
    })
}

/// 归档一份报告：来源在 library_root/周报/，目标在 <vault_root>/报告/。
pub fn archive_report(
    vault_root: &Path,
    library_root: &Path,
    file_name: &str,
    period: Option<&str>,
    now: f64,
) -> Result<ArchiveReceipt, String> {
    assert_archive_name(file_name)?;
    let source = library_root.join(worklog::REPORT_DIR).join(file_name);
    let meta = fs::symlink_metadata(&source)
        .map_err(|e| format!("读取待归档报告失败（{}）：{}", source.display(), e))?;
    if !meta.file_type().is_file() {
        return Err(format!("待归档报告不是普通文件：{}", source.display()));
    }
    let content = fs::read_to_string(&source)
        .map_err(|e| format!("读取待归档报告失败（{}）：{}", source.display(), e))?;
    let resolved_period = period
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| worklog::report_file_week(file_name))
        .ok_or_else(|| "无法从文件名推断来源周期；请提供周期（例如 2026-11）。".to_string())?;
    if resolved_period.chars().count() > 40 || resolved_period.contains('\n') {
        return Err("来源周期不合法。".to_string());
    }
    let source_hash = sha256_hex(content.as_bytes());
    let fields = vec![
        ("source_type".to_string(), "report".to_string()),
        ("source_file".to_string(), file_name.to_string()),
        ("source_period".to_string(), resolved_period.clone()),
        ("source_sha256".to_string(), source_hash.clone()),
        ("archived_at".to_string(), task_shell::utc_iso8601(now)),
    ];
    let archived = with_archive_fields(&content, &fields)?;
    archive_text(
        vault_root,
        REPORT_ARCHIVE_DIR,
        file_name,
        &archived,
        &source_hash,
        Some(&resolved_period),
    )
}

/// 归档一段已发布成果正文；草稿与未发布成文拒绝。
pub fn archive_artifact_text(
    vault_root: &Path,
    file_name: &str,
    content: &str,
    now: f64,
) -> Result<ArchiveReceipt, String> {
    assert_archive_name(file_name)?;
    let scenario = frontmatter_value(content, "scenario").unwrap_or_default();
    if scenario.replace('-', "_") != "material_writing" {
        return Err("只归档文案写作场景的已发布成果。".to_string());
    }
    let (status, _kind) = task_shell::parse_writing_frontmatter(content);
    if status != "published" {
        return Err(format!(
            "只有已发布成果可以归档；当前状态：{status}（草稿与未发布成文不归档）。"
        ));
    }
    let source_hash = sha256_hex(content.as_bytes());
    let fields = vec![
        ("source_type".to_string(), "artifact".to_string()),
        ("source_file".to_string(), file_name.to_string()),
        ("source_sha256".to_string(), source_hash.clone()),
        ("archived_at".to_string(), task_shell::utc_iso8601(now)),
    ];
    let archived = with_archive_fields(content, &fields)?;
    archive_text(
        vault_root,
        ARTIFACT_ARCHIVE_DIR,
        file_name,
        &archived,
        &source_hash,
        None,
    )
}

/// 从本机成果目录读取一条已发布成果并归档。
pub fn archive_artifact(
    vault_root: &Path,
    relative_path: &str,
    now: f64,
) -> Result<ArchiveReceipt, String> {
    let artifact = task_shell::read_offline_artifact(relative_path)?;
    let file_name = artifact
        .file_name
        .rsplit('/')
        .next()
        .unwrap_or(&artifact.file_name)
        .to_string();
    archive_artifact_text(vault_root, &file_name, &artifact.content, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_300_000.0;

    struct Temp {
        vault: PathBuf,
        library: PathBuf,
    }

    impl Temp {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-wr312-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&base);
            let vault = base.join("vault");
            let library = base.join("library");
            fs::create_dir_all(&vault).unwrap();
            fs::create_dir_all(library.join("周报")).unwrap();
            Self { vault, library }
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.vault.parent().unwrap());
        }
    }

    fn write_report(temp: &Temp, name: &str, content: &str) {
        fs::write(temp.library.join("周报").join(name), content).unwrap();
    }

    #[test]
    fn report_archive_adds_period_frontmatter_and_is_idempotent() {
        let temp = Temp::new("report");
        write_report(&temp, "2026-W45-周报.md", "# 周报正文\n\n合成内容");
        let first = archive_report(&temp.vault, &temp.library, "2026-W45-周报.md", None, NOW)
            .unwrap();
        assert!(first.ok);
        assert!(!first.already_archived);
        assert_eq!(first.source_period.as_deref(), Some("2026-W45"));
        let target = temp.vault.join("报告").join("2026-W45-周报.md");
        let archived = fs::read_to_string(&target).unwrap();
        assert!(archived.contains("source_period: \"2026-W45\""), "{archived}");
        assert!(archived.contains("source_type: \"report\""));
        assert!(archived.contains("# 周报正文"));
        let second = archive_report(&temp.vault, &temp.library, "2026-W45-周报.md", None, NOW + 60.0)
            .unwrap();
        assert!(second.already_archived);
        assert_eq!(second.bytes, first.bytes);
        let after = fs::read_to_string(&target).unwrap();
        assert_eq!(after, archived, "幂等时不得重写");
    }

    #[test]
    fn report_archive_rejects_same_name_different_source() {
        let temp = Temp::new("report-conflict");
        write_report(&temp, "2026-W45-周报.md", "旧内容");
        archive_report(&temp.vault, &temp.library, "2026-W45-周报.md", None, NOW).unwrap();
        write_report(&temp, "2026-W45-周报.md", "新内容");
        let err = archive_report(&temp.vault, &temp.library, "2026-W45-周报.md", None, NOW).unwrap_err();
        assert!(err.contains("来源不同"), "{err}");
    }

    #[test]
    fn report_requires_period_for_non_week_name() {
        let temp = Temp::new("report-period");
        write_report(&temp, "十一月总结.md", "合成");
        let err = archive_report(&temp.vault, &temp.library, "十一月总结.md", None, NOW).unwrap_err();
        assert!(err.contains("来源周期"), "{err}");
        let ok = archive_report(&temp.vault, &temp.library, "十一月总结.md", Some("2026-11"), NOW)
            .unwrap();
        assert_eq!(ok.source_period.as_deref(), Some("2026-11"));
    }

    #[test]
    fn report_archive_preserves_existing_frontmatter() {
        let temp = Temp::new("report-frontmatter");
        write_report(
            &temp,
            "2026-W45-周报.md",
            "---\ntitle: 合成周报\nstatus: finished\n---\n正文",
        );
        archive_report(&temp.vault, &temp.library, "2026-W45-周报.md", None, NOW).unwrap();
        let archived = fs::read_to_string(temp.vault.join("报告").join("2026-W45-周报.md")).unwrap();
        assert!(archived.contains("title: 合成周报"), "{archived}");
        assert!(archived.contains("status: finished"), "{archived}");
        assert!(archived.contains("source_period: \"2026-W45\""));
    }

    #[test]
    fn artifact_archive_requires_published() {
        let temp = Temp::new("artifact");
        let draft = "---\nscenario: material_writing\nstatus: draft\n---\n正文";
        let err = archive_artifact_text(&temp.vault, "offline-material-writing-1.md", draft, NOW)
            .unwrap_err();
        assert!(err.contains("草稿") || err.contains("只有已发布"), "{err}");
        let finished = "---\nscenario: material_writing\nstatus: finished\n---\n正文";
        let err = archive_artifact_text(&temp.vault, "offline-material-writing-2.md", finished, NOW)
            .unwrap_err();
        assert!(err.contains("只有已发布"), "{err}");
        let published = "---\nscenario: material_writing\nstatus: published\n---\n正文";
        let receipt = archive_artifact_text(
            &temp.vault,
            "offline-material-writing-3.md",
            published,
            NOW,
        )
        .unwrap();
        assert!(receipt.ok);
        let target = temp.vault.join("归档成果").join("offline-material-writing-3.md");
        let archived = fs::read_to_string(&target).unwrap();
        assert!(archived.contains("source_type: \"artifact\""));
        assert!(archived.contains("status: published"));
        let second = archive_artifact_text(
            &temp.vault,
            "offline-material-writing-3.md",
            published,
            NOW + 60.0,
        )
        .unwrap();
        assert!(second.already_archived);
    }

    #[test]
    fn archive_names_reject_traversal_and_non_markdown() {
        let temp = Temp::new("names");
        let err = archive_report(&temp.vault, &temp.library, "../逃亡.md", None, NOW).unwrap_err();
        assert!(err.contains("不合法"), "{err}");
        let err = archive_report(&temp.vault, &temp.library, "总结.txt", None, NOW).unwrap_err();
        assert!(err.contains("Markdown"), "{err}");
    }
}
