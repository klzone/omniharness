//! 本地业务事件表（OD-HAR-202，Phase B）。
//!
//! 事件表是「候选→确认→落盘」链路的可追溯索引，不是消息总线：
//!   * 单文件追加（`<work 根>/events.jsonl`），一行一个事件；
//!   * 只由宿主动作点调用，模型与前端没有写入口；
//!   * 追加失败不阻断业务动作（调用方忽略错误），事件缺席时以实体自身历史为准；
//!   * 不存正文：detail 只允许枚举短语，entity/ref 只有 id。
//!
//! 序号在同一进程内串行分配（现有部署为单宿主进程，多窗口共享同一进程）。
//! 跨进程并发写不在本轮范围；文件损坏行在读取时跳过并计数。

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const EVENTS_FILE: &str = "events.jsonl";
pub const EVENTS_SCHEMA: &str = "omnidesk.event.v1";
/// 单行事件上限；超出即拒绝（防止把正文塞进事件）。
pub const MAX_EVENT_CHARS: usize = 512;
/// 读取默认上限。
pub const DEFAULT_RECENT_LIMIT: usize = 200;

static SEQUENCE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EventRef {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EventRow {
    pub schema: String,
    pub seq: u64,
    pub ts: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub actor: String,
    pub entity: EventRef,
    #[serde(default)]
    pub refs: Vec<EventRef>,
    pub status: String,
    #[serde(default)]
    pub detail: Option<String>,
}

pub fn events_path(work_root: &Path) -> PathBuf {
    work_root.join(EVENTS_FILE)
}

fn valid_token(value: &str, max: usize) -> bool {
    let count = value.chars().count();
    !value.is_empty()
        && count <= max
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'))
}

/// 追加一条事件。调用方为业务动作成功后；返回错误仅供测试与诊断，
/// 业务调用点应忽略错误（事件缺席不影响主流程）。
pub fn append(
    work_root: &Path,
    kind: &str,
    actor: &str,
    entity: (&str, &str),
    refs: &[(&str, &str)],
    detail: Option<&str>,
) -> Result<EventRow, String> {
    if !valid_token(kind, 64) || !kind.contains('.') {
        return Err(format!("事件类型不合法：{kind:?}"));
    }
    if !valid_token(actor, 32) {
        return Err(format!("事件来源不合法：{actor:?}"));
    }
    if !valid_token(entity.0, 32) || entity.1.trim().is_empty() || entity.1.chars().count() > 120 {
        return Err("事件实体不合法。".to_string());
    }
    let mut normalized_refs = Vec::new();
    for (ref_kind, ref_id) in refs {
        if !valid_token(ref_kind, 32) || ref_id.trim().is_empty() || ref_id.chars().count() > 120 {
            return Err("事件引用不合法。".to_string());
        }
        normalized_refs.push(EventRef {
            kind: ref_kind.to_string(),
            id: ref_id.to_string(),
        });
    }
    if let Some(note) = detail {
        if note.chars().count() > 120 {
            return Err("事件备注过长。".to_string());
        }
    }

    let _guard = SEQUENCE_LOCK.lock().map_err(|_| "事件表锁不可用".to_string())?;
    let path = events_path(work_root);
    let seq = last_seq(&path)? + 1;
    let row = EventRow {
        schema: EVENTS_SCHEMA.to_string(),
        seq,
        ts: crate::task_shell::utc_iso8601(crate::task_shell::now_secs()),
        kind: kind.to_string(),
        actor: actor.to_string(),
        entity: EventRef {
            kind: entity.0.to_string(),
            id: entity.1.to_string(),
        },
        refs: normalized_refs,
        status: "processed".to_string(),
        detail: detail.map(str::to_string),
    };
    let line = serde_json::to_string(&row).map_err(|e| format!("事件序列化失败：{e}"))?;
    if line.chars().count() > MAX_EVENT_CHARS {
        return Err("事件行超过长度上限，已拒绝写入。".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建事件目录失败：{e}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("打开事件文件失败：{e}"))?;
    file.write_all(line.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_data())
        .map_err(|e| format!("写入事件失败：{e}"))?;
    Ok(row)
}

fn last_seq(path: &Path) -> Result<u64, String> {
    let mut max_seq = 0u64;
    if !path.exists() {
        return Ok(0);
    }
    let file = fs::File::open(path).map_err(|e| format!("读取事件文件失败：{e}"))?;
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(seq) = value.get("seq").and_then(|v| v.as_u64()) {
                max_seq = max_seq.max(seq);
            }
        }
    }
    Ok(max_seq)
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RecentEvents {
    pub items: Vec<EventRow>,
    pub skipped_invalid: usize,
    pub limit: usize,
}

/// 读取最近的 N 条事件（按 seq 升序返回）。损坏行跳过并计数，不猜内容。
pub fn recent(work_root: &Path, limit: usize) -> RecentEvents {
    let capped = limit.clamp(1, 2000);
    let path = events_path(work_root);
    if !path.exists() {
        return RecentEvents {
            items: Vec::new(),
            skipped_invalid: 0,
            limit: capped,
        };
    }
    let Ok(file) = fs::File::open(&path) else {
        return RecentEvents {
            items: Vec::new(),
            skipped_invalid: 1,
            limit: capped,
        };
    };
    let mut rows: Vec<EventRow> = Vec::new();
    let mut skipped = 0usize;
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            skipped += 1;
            continue;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<EventRow>(trimmed) {
            Ok(row) if row.schema == EVENTS_SCHEMA => rows.push(row),
            _ => skipped += 1,
        }
    }
    if rows.len() > capped {
        rows = rows.split_off(rows.len() - capped);
    }
    RecentEvents {
        items: rows,
        skipped_invalid: skipped,
        limit: capped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "od-har-202-event-{tag}-{}-{}",
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
    fn append_assigns_monotonic_sequence_and_persists() {
        let root = temp_root("seq");
        let first = append(&root, "work_record.created", "user", ("work_record", "wr-1"), &[], None).unwrap();
        let second = append(
            &root,
            "task_candidate.confirmed",
            "user",
            ("task_candidate", "tc-1"),
            &[("task", "t-1")],
            Some("confirmed"),
        )
        .unwrap();
        assert_eq!(first.seq, 1);
        assert_eq!(second.seq, 2);
        let recent_rows = recent(&root, 10);
        assert_eq!(recent_rows.items.len(), 2);
        assert_eq!(recent_rows.items[1].refs.len(), 1);
        assert_eq!(recent_rows.items[1].refs[0].id, "t-1");
    }

    #[test]
    fn rejects_bad_tokens_and_long_detail() {
        let root = temp_root("reject");
        assert!(append(&root, "no_dot", "user", ("work_record", "wr-1"), &[], None).is_err());
        assert!(append(&root, "a.b", "user", ("work_record", ""), &[], None).is_err());
        let long = "x".repeat(400);
        assert!(append(&root, "a.b", "user", ("work_record", "wr-1"), &[], Some(&long)).is_err());
    }

    #[test]
    fn recent_skips_corrupt_lines_without_guessing() {
        let root = temp_root("corrupt");
        append(&root, "work_record.created", "user", ("work_record", "wr-1"), &[], None).unwrap();
        let path = events_path(&root);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{not json}\n").unwrap();
        let listing = recent(&root, 10);
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.skipped_invalid, 1);
    }

    #[test]
    fn recent_caps_to_limit_and_keeps_order() {
        let root = temp_root("limit");
        for index in 0..5 {
            append(
                &root,
                "work_record.created",
                "user",
                ("work_record", &format!("wr-{index}")),
                &[],
                None,
            )
            .unwrap();
        }
        let listing = recent(&root, 2);
        assert_eq!(listing.items.len(), 2);
        assert_eq!(listing.items[0].entity.id, "wr-3");
        assert_eq!(listing.items[1].entity.id, "wr-4");
    }
}
