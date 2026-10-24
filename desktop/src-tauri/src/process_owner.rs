//! 引擎所有权登记的最小宿主侧实现（OD-ARCH-074）。
//!
//! 只读取引擎写的 `owner.json` 并做身份核验，然后按记录中的进程组回收遗留
//! 进程树。**不按进程名模糊扫描**，不碰 `~/.dsh`、`~/.agents` 或原版 DSH。
//!
//! Rust 依赖中不含 libc，因此只用外部 `/bin/kill` 向进程组发信号，并在发送前
//! 再次核验：pid 存活 + pgid 匹配 + boot_id 一致 + 命令行指纹一致。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct OwnerRecord {
    pub pid: i32,
    pub pgid: i32,
    pub host_pid: i32,
    pub boot_id: String,
    pub channel: String,
    pub cmdline: Vec<String>,
    pub child_pid: i32,
    pub child_pgid: i32,
}

#[derive(Debug, Clone)]
pub struct ReclaimOutcome {
    pub status: String,
    pub action: String,
    pub detail: String,
}

fn owner_path(state_dir: &Path) -> PathBuf {
    state_dir.join("owner.json")
}

fn boot_id() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|s| s.trim().to_string())
}

fn proc_stat(pid: i32) -> Option<(char, i32)> {
    if pid <= 0 {
        return None;
    }
    let raw = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let end = raw.rfind(')')?;
    let rest = raw.get(end + 1..)?.trim_start();
    let mut parts = rest.split_whitespace();
    let state = parts.next()?.chars().next()?;
    // rest 从 state 开始：state ppid pgrp ...
    let _ppid = parts.next()?;
    let pgrp: i32 = parts.next()?.parse().ok()?;
    Some((state, pgrp))
}

fn proc_cmdline(pid: i32) -> Vec<String> {
    let Ok(bytes) = std::fs::read(format!("/proc/{}/cmdline", pid)) else {
        return Vec::new();
    };
    bytes
        .split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect()
}

fn is_alive(pid: i32) -> bool {
    matches!(proc_stat(pid), Some((state, _)) if state != 'Z')
}

fn is_engine_cmd(cmd: &[String]) -> bool {
    if cmd.iter().any(|a| a.contains("omnidesk_engine")) {
        return true;
    }
    let exe = cmd
        .first()
        .map(|p| p.rsplit('/').next().unwrap_or(p))
        .unwrap_or("");
    exe.starts_with("node") && cmd.iter().skip(1).any(|a| a.ends_with("bin.js"))
}

fn group_members(pgid: i32) -> Vec<(i32, Vec<String>)> {
    if pgid <= 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(pid) = name.parse::<i32>() else { continue };
        if let Some((state, pgrp)) = proc_stat(pid) {
            if state != 'Z' && pgrp == pgid {
                out.push((pid, proc_cmdline(pid)));
            }
        }
    }
    out
}

pub fn read_owner(state_dir: &Path) -> Option<OwnerRecord> {
    let raw = std::fs::read_to_string(owner_path(state_dir)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let pid = value.get("pid")?.as_i64()? as i32;
    let pgid = value.get("pgid")?.as_i64()? as i32;
    let cmdline = value
        .get("cmdline")?
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    Some(OwnerRecord {
        pid,
        pgid,
        host_pid: value.get("host_pid").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        boot_id: value.get("boot_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        channel: value.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        cmdline,
        child_pid: value.get("child_pid").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        child_pgid: value.get("child_pgid").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
    })
}

fn kill_group(pgid: i32, signal: &str) -> bool {
    if pgid <= 1 || pgid == std::process::id() as i32 {
        return false;
    }
    Command::new("/bin/kill")
        .arg("-s")
        .arg(signal)
        .arg("--")
        .arg(format!("-{}", pgid))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 回收一个已核验的进程组：SIGTERM → 等待 → SIGKILL。
fn terminate_group(pgid: i32, leader: Option<i32>, grace: Duration) -> Result<bool, String> {
    let members = group_members(pgid);
    if members.is_empty() {
        return Ok(false);
    }
    if let Some(leader) = leader {
        if is_alive(leader) {
            match proc_stat(leader) {
                Some((state, pgrp)) if state != 'Z' && pgrp == pgid => {}
                _ => return Err("子进程组组长身份不一致，拒绝终止".to_string()),
            }
            if !is_engine_cmd(&proc_cmdline(leader)) {
                return Err("子进程组组长不是本产品进程，拒绝终止".to_string());
            }
        } else if !members.iter().all(|(_, cmd)| is_engine_cmd(cmd)) {
            return Err("子进程组包含非本产品进程，拒绝终止".to_string());
        }
    } else if !members.iter().all(|(_, cmd)| is_engine_cmd(cmd)) {
        return Err("进程组包含非本产品进程，拒绝终止".to_string());
    }

    kill_group(pgid, "TERM");
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if group_members(pgid).is_empty() {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    kill_group(pgid, "KILL");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !group_members(pgid).is_empty() {
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(true)
}

/// 宿主启动引擎前调用：只有身份核验通过才回收，否则返回可读错误并拒绝启动。
pub fn reclaim_before_start(state_dir: &Path, grace: Duration) -> Result<ReclaimOutcome, String> {
    let path = owner_path(state_dir);
    if !path.exists() {
        return Ok(ReclaimOutcome {
            status: "none".into(),
            action: "none".into(),
            detail: String::new(),
        });
    }
    let Some(record) = read_owner(state_dir) else {
        return Err(format!(
            "引擎所有者登记无法解析，拒绝覆盖：{}",
            path.display()
        ));
    };
    let current_boot = boot_id().unwrap_or_default();
    if current_boot.is_empty() || record.boot_id != current_boot {
        let _ = std::fs::remove_file(&path);
        return Ok(ReclaimOutcome {
            status: "stale_boot".into(),
            action: "removed_stale_record".into(),
            detail: "登记来自上一次内核启动，已删除".into(),
        });
    }

    let me = std::process::id() as i32;
    let alive = is_alive(record.pid);
    let identity_ok = alive
        && matches!(proc_stat(record.pid), Some((s, pgrp)) if s != 'Z' && pgrp == record.pgid)
        && proc_cmdline(record.pid) == record.cmdline;
    let host_alive = record.host_pid > 0 && is_alive(record.host_pid);

    // 先判定「是否别人的活跃实例」：任何情况下都不能先动它的子进程组。
    if alive && identity_ok && host_alive && record.host_pid != me {
        return Err("另一个 OmniDesk 实例正在使用同一数据根，未启动新引擎".into());
    }
    // 身份不一致的存活进程一律不碰，也不继续往下回收。
    if alive && !identity_ok {
        return Err(format!(
            "同数据根存在无法核验身份的进程（pid={}），拒绝启动，不杀进程；请人工检查 {}",
            record.pid,
            path.display()
        ));
    }

    if let Some(child_pgid) = Some(record.child_pgid).filter(|p| *p > 1 && *p != record.pgid) {
        if !group_members(child_pgid).is_empty() {
            let forced = terminate_group(child_pgid, Some(record.child_pid), grace)?;
            let _ = std::fs::remove_file(&path);
            return Ok(ReclaimOutcome {
                status: "orphan_child_group".into(),
                action: if forced { "sigkill_group" } else { "sigterm_group" }.into(),
                detail: "已回收上一实例遗留的 DSH 子进程组".into(),
            });
        }
    }

    if alive && identity_ok {
        let forced = terminate_group(record.pgid, Some(record.pid), grace)?;
        let _ = std::fs::remove_file(&path);
        return Ok(ReclaimOutcome {
            status: "active_engine".into(),
            action: if forced { "sigkill_group" } else { "sigterm_group" }.into(),
            detail: "已回收上一实例遗留的引擎进程组".into(),
        });
    }

    if alive || !group_members(record.pgid).is_empty() {
        return Err(format!(
            "同数据根存在无法核验身份的进程（pid={}），拒绝启动，不杀进程；请人工检查 {} ",
            record.pid,
            path.display()
        ));
    }

    let _ = std::fs::remove_file(&path);
    Ok(ReclaimOutcome {
        status: "stale_record".into(),
        action: "removed_stale_record".into(),
        detail: "登记对应的进程已不存在，已删除".into(),
    })
}

/// 宿主收尾兜底：按当前引擎登记回收子进程组（不杀外部 DSH）。
pub fn cleanup_engine_group(state_dir: &Path, engine_pid: i32, grace: Duration) -> Option<String> {
    let record = read_owner(state_dir)?;
    if record.pid != engine_pid {
        return None;
    }
    let mut notes = Vec::new();
    if record.child_pgid > 1 && record.child_pgid != record.pgid {
        if let Err(err) = terminate_group(record.child_pgid, Some(record.child_pid), grace) {
            notes.push(err);
        }
    }
    if !notes.is_empty() {
        Some(notes.join("; "))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_state(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "od074-{}-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            seq
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_owner_is_noop() {
        let dir = temp_state("none");
        let out = reclaim_before_start(&dir, Duration::from_millis(50)).unwrap();
        assert_eq!(out.status, "none");
        assert!(!owner_path(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_owner_is_refused_not_deleted() {
        let dir = temp_state("corrupt");
        std::fs::write(owner_path(&dir), "{not json").unwrap();
        let err = reclaim_before_start(&dir, Duration::from_millis(50)).unwrap_err();
        assert!(err.contains("无法解析"));
        assert!(owner_path(&dir).exists(), "损坏登记必须保留供人工检查");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn record_from_other_boot_is_removed_without_killing() {
        let dir = temp_state("boot");
        // 用一个必然存活的进程（本进程）但伪造旧 boot_id：只删登记，不杀进程。
        let record = serde_json::json!({
            "pid": std::process::id(),
            "pgid": 1,
            "host_pid": 0,
            "boot_id": "previous-boot",
            "channel": "web-api",
            "started_at": 0.0,
            "cmdline": ["python3", "-m", "omnidesk_engine"],
            "child_pid": 0,
            "child_pgid": 0,
        });
        std::fs::write(owner_path(&dir), record.to_string()).unwrap();
        let out = reclaim_before_start(&dir, Duration::from_millis(50)).unwrap();
        assert_eq!(out.action, "removed_stale_record");
        assert!(!owner_path(&dir).exists());
        assert!(is_alive(std::process::id() as i32));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pid_reuse_with_unrelated_cmdline_is_never_killed() {
        let dir = temp_state("reuse");
        let (_, pgrp) = proc_stat(std::process::id() as i32).unwrap();
        let record = serde_json::json!({
            "pid": std::process::id(),
            "pgid": pgrp,
            "host_pid": 0,
            "boot_id": boot_id().unwrap_or_default(),
            "channel": "web-api",
            "started_at": 0.0,
            "cmdline": ["definitely-not-our-engine"],
            "child_pid": 0,
            "child_pgid": 0,
        });
        std::fs::write(owner_path(&dir), record.to_string()).unwrap();
        let err = reclaim_before_start(&dir, Duration::from_millis(50)).unwrap_err();
        assert!(err.contains("拒绝启动"));
        assert!(is_alive(std::process::id() as i32));
        assert!(owner_path(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_instance_owned_by_another_host_is_refused() {
        let dir = temp_state("shared");
        // 用一个子进程冒充「另一个宿主仍在使用的引擎」：cmdline 与登记完全一致，
        // 但登记里的 host_pid 指向别的存活进程。
        // `sleep 30 omnidesk_engine` 在 GNU 实现里是非法时长、会立刻报错退出（旧夹具因此
        // 偶发/必现失败）。这里改用 sh 常驻 30 秒，cmdline 里仍带 omnidesk_engine 字样。
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .arg("omnidesk_engine")
            .spawn()
            .expect("spawn sh");
        let pid = child.id() as i32;
        // 刚 spawn 的子进程可能尚未 exec，此刻读到的 cmdline 仍继承自测试进程；
        // 等它真正变成 sh 再登记，否则会落到「身份不符」分支而非「他人实例」分支。
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline
            && !proc_cmdline(pid).iter().any(|arg| arg.contains("omnidesk_engine"))
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        let pgrp = proc_stat(pid).expect("stat child").1;
        let record = serde_json::json!({
            "pid": pid,
            "pgid": pgrp,
            "host_pid": 1,
            "boot_id": boot_id().unwrap_or_default(),
            "channel": "web-api",
            "started_at": 0.0,
            "cmdline": proc_cmdline(pid),
            "child_pid": 0,
            "child_pgid": 0,
        });
        std::fs::write(owner_path(&dir), record.to_string()).unwrap();
        let err = reclaim_before_start(&dir, Duration::from_millis(50)).unwrap_err();
        assert!(err.contains("另一个 OmniDesk 实例"));
        assert!(is_alive(pid), "另一实例的引擎不得被回收");
        assert!(owner_path(&dir).exists(), "另一实例的登记不得被删除");
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
