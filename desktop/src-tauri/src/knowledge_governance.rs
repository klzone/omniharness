//! 知识库人工治理记录（OD-KB-201 S2）。
//!
//! 治理记录只保存在 vault 根下的 `governance/` 目录，与 `raw/` 平级；不是仓库
//! 自身的 `knowledge/` 目录。三类文件均为 Markdown + 单行元数据，不上数据库：
//!   * `raw_admissions.v1.md`：原始资料准入记录；
//!   * `review-decisions.v1.md`：审核与冲突裁决记录；
//!   * `admission-rules.v1.md`：准入规则记录。
//!
//! 写入只经宿主函数 `append_record`，没有对模型或前端的写命令。每行在落盘前
//! 都经过 `profile_rules::mask_privacy`，与工作记录链路使用同一隐私口径。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::knowledge_ingest::rfc3339_local;
use crate::profile_rules::{mask_privacy, RulePolicy};
use crate::vault_write;

/// vault 根下的治理目录，与 `raw/` 平级。
pub const GOVERNANCE_DIR: &str = "governance";

pub const KIND_RAW_ADMISSIONS: &str = "raw_admissions";
pub const KIND_REVIEW_DECISIONS: &str = "review_decisions";
pub const KIND_ADMISSION_RULES: &str = "admission_rules";

pub const RAW_ADMISSIONS_FILE: &str = "raw_admissions.v1.md";
pub const REVIEW_DECISIONS_FILE: &str = "review-decisions.v1.md";
pub const ADMISSION_RULES_FILE: &str = "admission-rules.v1.md";

/// 一条待追加的治理记录。`sha256` 缺省时索引行写 `sha256:-`。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct GovernanceEntry {
    pub action: String,
    pub subject: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub note: String,
}

/// 从治理文件读回的一条记录。`sha256` 是行内保存的哈希前缀。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct GovernanceRecord {
    pub at: String,
    pub action: String,
    pub subject: String,
    pub sha256: String,
    pub note: String,
}

fn kind_meta(kind: &str) -> Result<(&'static str, &'static str), String> {
    match kind.trim().to_ascii_lowercase().as_str() {
        KIND_RAW_ADMISSIONS => Ok((RAW_ADMISSIONS_FILE, "原始资料准入记录")),
        KIND_REVIEW_DECISIONS => Ok((REVIEW_DECISIONS_FILE, "知识审核与裁决记录")),
        KIND_ADMISSION_RULES => Ok((ADMISSION_RULES_FILE, "准入规则")),
        other => Err(format!(
            "未知的治理记录类型：{other}；只接受 raw_admissions / review_decisions / admission_rules"
        )),
    }
}

fn ensure_governance_dir(root: &Path) -> Result<PathBuf, String> {
    let dir = root.join(GOVERNANCE_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("创建治理目录失败（{}）：{}", dir.display(), e))?;
    }
    let meta = fs::symlink_metadata(&dir)
        .map_err(|e| format!("无法确认治理目录（{}）：{}", dir.display(), e))?;
    if meta.file_type().is_symlink() {
        return Err(format!("治理目录不能是符号链接：{}", dir.display()));
    }
    if !meta.is_dir() {
        return Err(format!("治理目录不是普通目录：{}", dir.display()));
    }
    Ok(dir)
}

/// 字段清洗：换行/控制字符转空格，竖线改全角，避免破坏单行记录格式。
fn clean_field(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '|' => '｜',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn masked_field(value: &str, policy: &RulePolicy) -> String {
    let cleaned = clean_field(value);
    mask_privacy(&cleaned, policy).0
}

fn short_hash(raw: &str) -> Result<String, String> {
    let hash = raw.trim().to_ascii_lowercase();
    if hash.is_empty() || hash == "-" {
        return Ok("-".to_string());
    }
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("治理记录的 sha256 只能包含十六进制字符".to_string());
    }
    Ok(hash.chars().take(12).collect())
}

fn render_line(at: &str, action: &str, subject: &str, sha: &str, note: &str) -> String {
    format!("- {at} | {action} | {subject} | sha256:{sha} | {note}")
}

fn looks_like_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.len() >= 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
}

fn parse_line(line: &str) -> Option<GovernanceRecord> {
    let rest = line.trim().strip_prefix("- ")?;
    let mut parts = rest.splitn(5, '|');
    let at = parts.next()?.trim();
    let action = parts.next()?.trim();
    let subject = parts.next()?.trim();
    let hash_part = parts.next()?.trim();
    let note = parts.next()?.trim();
    if !looks_like_timestamp(at) || action.is_empty() || subject.is_empty() {
        return None;
    }
    let sha256 = hash_part.strip_prefix("sha256:")?.trim().to_string();
    Some(GovernanceRecord {
        at: at.to_string(),
        action: action.to_string(),
        subject: subject.to_string(),
        sha256,
        note: note.to_string(),
    })
}

/// 追加一条治理记录。目录幂等创建；记录以原子写入落盘，失败不留下半截行。
/// 所有文本字段在写入前都经过 `mask_privacy`。
pub fn append_record(
    root: &Path,
    kind: &str,
    entry: &GovernanceEntry,
    policy: &RulePolicy,
    utc_offset_minutes: i32,
) -> Result<GovernanceRecord, String> {
    append_record_at(
        root,
        kind,
        entry,
        policy,
        crate::task_shell::now_secs() as i64,
        utc_offset_minutes,
    )
}

/// 固定时间戳入口，便于测试；生产路径由 `append_record` 调用。
pub(crate) fn append_record_at(
    root: &Path,
    kind: &str,
    entry: &GovernanceEntry,
    policy: &RulePolicy,
    unix_seconds: i64,
    utc_offset_minutes: i32,
) -> Result<GovernanceRecord, String> {
    let (file_name, heading) = kind_meta(kind)?;
    let dir = ensure_governance_dir(root)?;
    let at = rfc3339_local(unix_seconds, utc_offset_minutes);
    let action = masked_field(&entry.action, policy);
    let subject = masked_field(&entry.subject, policy);
    let note = masked_field(&entry.note, policy);
    if action.is_empty() {
        return Err("治理记录的 action 不能为空".to_string());
    }
    if subject.is_empty() {
        return Err("治理记录的 subject 不能为空".to_string());
    }
    let sha = short_hash(&entry.sha256)?;
    let line = render_line(&at, &action, &subject, &sha, &note);
    let relative = format!("{GOVERNANCE_DIR}/{file_name}");
    let target = dir.join(file_name);

    if target.exists() {
        let meta = fs::symlink_metadata(&target)
            .map_err(|e| format!("无法确认治理文件（{}）：{}", target.display(), e))?;
        if meta.file_type().is_symlink() {
            return Err(format!("治理文件不能是符号链接：{}", target.display()));
        }
        if !meta.is_file() {
            return Err(format!("治理文件不是普通文件：{}", target.display()));
        }
        let current = fs::read_to_string(&target)
            .map_err(|e| format!("读取治理文件失败（{}）：{}", target.display(), e))?;
        let expected = vault_write::content_sha256(&current);
        let mut next = if current.trim().is_empty() {
            format!("# {heading}\n\n")
        } else {
            current
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(&line);
        next.push('\n');
        vault_write::save_note(root, &relative, &next, &expected)
            .map_err(|e| format!("追加治理记录失败：{e}"))?;
    } else {
        let initial = format!("# {heading}\n\n{line}\n");
        vault_write::create_note(root, &relative, &initial)
            .map_err(|e| format!("创建治理记录失败：{e}"))?;
    }

    Ok(GovernanceRecord {
        at,
        action,
        subject,
        sha256: sha,
        note,
    })
}

/// 读取某类治理记录；文件不存在返回空列表，损坏行跳过而不 panic。
pub fn list_records(root: &Path, kind: &str) -> Result<Vec<GovernanceRecord>, String> {
    let (file_name, _) = kind_meta(kind)?;
    let path = root.join(GOVERNANCE_DIR).join(file_name);
    match fs::symlink_metadata(&path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(format!("治理文件不能是符号链接：{}", path.display()));
            }
            if !meta.is_file() {
                return Err(format!("治理文件不是普通文件：{}", path.display()));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(format!("无法确认治理文件（{}）：{}", path.display(), e));
        }
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("读取治理文件失败（{}）：{}", path.display(), e))?;
    Ok(text.lines().filter_map(parse_line).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_rules::ProfileRules;

    fn root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-governance-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    fn entry(action: &str, subject: &str, sign: &str, note: &str) -> GovernanceEntry {
        GovernanceEntry {
            action: action.to_string(),
            subject: subject.to_string(),
            sha256: sign.to_string(),
            note: note.to_string(),
        }
    }

    fn masking_policy() -> RulePolicy {
        let rules: ProfileRules = serde_json::from_str(crate::profile_rules::DEFAULT_RULES_JSON)
            .expect("出厂规则 JSON 必须可解析");
        RulePolicy::from_rules(&rules)
    }

    #[test]
    fn governance_directory_is_created_once() {
        let root = root("dir");
        let first = ensure_governance_dir(&root).unwrap();
        let second = ensure_governance_dir(&root).unwrap();
        assert_eq!(first, second);
        assert!(root.join(GOVERNANCE_DIR).is_dir());
        assert_eq!(
            fs::read_dir(root.join(GOVERNANCE_DIR)).unwrap().count(),
            0
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn append_then_list_returns_the_same_record() {
        let root = root("roundtrip");
        let policy = RulePolicy::clean();
        let first = append_record_at(
            &root,
            KIND_RAW_ADMISSIONS,
            &entry("admit", "raw/report.pdf", "abcdef012345", "原名：report.pdf；5 字节"),
            &policy,
            0,
            480,
        )
        .unwrap();
        let listed = list_records(&root, KIND_RAW_ADMISSIONS).unwrap();
        assert_eq!(listed, vec![first.clone()]);
        assert_eq!(first.at, "1970-01-01T08:00:00+08:00");
        assert_eq!(first.sha256, "abcdef012345");
        assert!(root.join(GOVERNANCE_DIR).join(RAW_ADMISSIONS_FILE).is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn damaged_lines_are_skipped_without_panic() {
        let root = root("damaged");
        ensure_governance_dir(&root).unwrap();
        let text = "# 原始资料准入记录\n\n- broken\n- 2026-11-07T10:00:00+08:00 | admit | raw/a.md | sha256:abc | ok\n- not-a-line | x | y | z | q\n";
        fs::write(root.join(GOVERNANCE_DIR).join(RAW_ADMISSIONS_FILE), text).unwrap();
        let listed = list_records(&root, KIND_RAW_ADMISSIONS).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].subject, "raw/a.md");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn governance_directory_is_not_scanned_as_notes() {
        let root = root("scan");
        let policy = RulePolicy::clean();
        append_record_at(
            &root,
            KIND_REVIEW_DECISIONS,
            &entry("approve", "知识库/概念/x.md", "abcdef012345", "合成审核"),
            &policy,
            0,
            480,
        )
        .unwrap();
        let scan = crate::knowledge_vault::scan_vault(&root, None).unwrap();
        assert!(scan
            .files
            .iter()
            .all(|file| !file.relative_path.starts_with("governance/")));
        assert!(scan.skipped.iter().any(|item| item.reason == "governance"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn append_masks_privacy_before_writing() {
        let root = root("mask");
        let policy = masking_policy();
        append_record_at(
            &root,
            KIND_ADMISSION_RULES,
            &entry("rule", "admission-rules", "abcdef012345", "联系 13800138000"),
            &policy,
            0,
            480,
        )
        .unwrap();
        let listed = list_records(&root, KIND_ADMISSION_RULES).unwrap();
        assert_eq!(listed[0].note, "联系 138******00");
        let text = fs::read_to_string(root.join(GOVERNANCE_DIR).join(ADMISSION_RULES_FILE)).unwrap();
        assert!(!text.contains("13800138000"), "明文手机号不得落盘：{text}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fields_are_sanitized_before_writing() {
        let root = root("sanitize");
        let policy = RulePolicy::clean();
        append_record_at(
            &root,
            KIND_REVIEW_DECISIONS,
            &entry("reject", "raw/a|b.md", "abcdef012345", "第一行\n第二行"),
            &policy,
            0,
            480,
        )
        .unwrap();
        let listed = list_records(&root, KIND_REVIEW_DECISIONS).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].subject, "raw/a｜b.md");
        assert_eq!(listed[0].note, "第一行 第二行");
        let _ = fs::remove_dir_all(&root);
    }
}
