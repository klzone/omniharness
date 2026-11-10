//! 引擎所有权登记的最小宿主侧实现（OD-ARCH-074）。
//!
//! 只读取引擎写的 `owner.json` 并做身份核验，然后按记录中的进程组回收遗留
//! 进程树。**不按进程名模糊扫描**，不碰 `~/.dsh`、`~/.agents` 或原版 DSH。
//!
//! Rust 依赖中不含 libc，因此只用外部 `/bin/kill` 向进程组发信号，并在发送前
//! 再次核验：pid 存活 + pgid 匹配 + boot_id 一致 + 命令行指纹一致。
//!
//! 进程的读取方式按平台差异交给 `platform`（OD-ARCH-133）：Linux 读 procfs，
//! macOS 走 ps / sysctl。本文件只保留「什么情况下可以杀」的判定。

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

// 平台差异（procfs / ps / sysctl）集中在 `platform`，这里只做薄委托。
fn boot_id() -> Option<String> {
    crate::platform::boot_id()
}

fn proc_stat(pid: i32) -> Option<(char, i32)> {
    crate::platform::process_stat(pid)
}

fn proc_cmdline(pid: i32) -> Vec<String> {
    crate::platform::process_cmd(pid)
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
        .map(|p| p.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(p))
        .unwrap_or("");
    is_node_image(exe) && cmd.iter().skip(1).any(|a| a.ends_with("bin.js"))
}

/// node 镜像名是否属于本产品。
///
/// OD-PROC-149a 之后 Windows 改用副本名 `omnidesk-node.exe` 启动 DSH；只认
/// `node` 前缀会让子进程组身份核验失明（结果是拒绝回收，不会误杀，但遗留
/// 进程清不掉）。两平台的分隔符都要认：owner.json 里的路径可能来自任一平台。
fn is_node_image(exe: &str) -> bool {
    let name = exe.to_ascii_lowercase();
    name.starts_with("node") || name.starts_with("omnidesk-node")
}

fn group_members(pgid: i32) -> Vec<(i32, Vec<String>)> {
    if pgid <= 1 {
        return Vec::new();
    }
    crate::platform::process_table()
        .into_iter()
        .filter(|row| row.state != 'Z' && row.pgid == pgid)
        .map(|row| (row.pid, row.cmd))
        .collect()
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
    // OD-SYS-701：pgid=0 是「没有 POSIX 进程组可回收」（Windows 语义），
    // 不是「有一个 0 号组」。此时跳过按组兜底，进程树由 Windows Job Object
    // 在句柄关闭时回收；不在这里反复打印无意义日志。
    if record.pgid <= 0 {
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

    /// OD-WR-314：找一个从 exec 起 argv 就稳定的 Python 解释器。
    ///
    /// macOS 的 /usr/bin/python3 与 framework 的 bin/pythonX 都是 launcher，
    /// 启动后会把进程 exec 成 Python.app，argv 途中改变；身份核验会因此判成
    /// 「无法核验身份」。先解析 realpath，再直接指向 Python.app（存在时）。
    fn engine_stub_python() -> String {
        let base = std::env::var("OMNIDESK_PYTHON")
            .ok()
            .map(|raw| raw.trim().to_string())
            .filter(|raw| !raw.is_empty())
            .unwrap_or_else(|| "python3".to_string());
        let Ok(output) = Command::new(&base)
            .arg("-c")
            .arg("import os,sys; print(os.path.realpath(sys.executable))")
            .output()
        else {
            return base;
        };
        if !output.status.success() {
            return base;
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            return base;
        }
        let Some(parent) = std::path::Path::new(&path).parent() else {
            return path;
        };
        let app = parent.join("../Resources/Python.app/Contents/MacOS/Python");
        if app.is_file() {
            return app.display().to_string();
        }
        path
    }

    #[test]
    fn missing_owner_is_noop() {
        let dir = temp_state("none");
        let out = reclaim_before_start(&dir, Duration::from_millis(50)).unwrap();
        assert_eq!(out.status, "none");
        assert!(!owner_path(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// OD-PROC-149a：Windows 副本名必须仍被认成自家进程，否则回收失明。
    #[test]
    fn engine_image_accepts_alias_names() {
        let cmd = |parts: &[&str]| parts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(is_engine_cmd(&cmd(&["omnidesk-node.exe", "bin.js"])));
        assert!(is_engine_cmd(&cmd(&[
            "C:\\Program Files\\OmniDesk\\runtime\\bin\\omnidesk-node.exe",
            "C:\\r\\bin.js",
        ])));
        assert!(is_engine_cmd(&cmd(&["node", "/r/bin.js"])));
        assert!(is_engine_cmd(&cmd(&[
            "omnidesk-python.exe",
            "-m",
            "omnidesk_engine",
        ])));
        assert!(!is_engine_cmd(&cmd(&["node", "other.js"])));
        assert!(!is_engine_cmd(&cmd(&["python3", "-m", "other"])));
    }

    /// OD-SYS-701：写一份最小 owner.json（只填 cleanup 会读的字段）。
    fn write_owner_for_cleanup(dir: &Path, pid: i32, pgid: i32, child_pgid: i32) {
        let record = serde_json::json!({
            "pid": pid,
            "pgid": pgid,
            "host_pid": 0,
            "boot_id": "test-boot",
            "channel": "web-api",
            "started_at": 0.0,
            "cmdline": ["python3", "-m", "omnidesk_engine"],
            "child_pid": 0,
            "child_pgid": child_pgid,
        });
        std::fs::write(owner_path(dir), record.to_string()).unwrap();
    }

    #[test]
    fn cleanup_skips_posix_group_reclaim_when_pgid_is_zero() {
        // Windows 的 current_pgid() 返回 0：语义是「无组可回收」，不是 0 号进程组。
        let dir = temp_state("pgid-zero");
        write_owner_for_cleanup(&dir, 4321, 0, 0);
        assert_eq!(
            cleanup_engine_group(&dir, 4321, Duration::from_millis(50)),
            None,
            "pgid=0 必须跳过兜底回收且不产生错误日志"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_skips_when_registered_pid_differs() {
        let dir = temp_state("pid-mismatch");
        write_owner_for_cleanup(&dir, 1111, 0, 0);
        assert_eq!(
            cleanup_engine_group(&dir, 2222, Duration::from_millis(50)),
            None,
            "登记不属于当前引擎时不得回收"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_skips_negative_pgid_without_noop_log() {
        let dir = temp_state("pgid-negative");
        write_owner_for_cleanup(&dir, 3333, -1, 0);
        assert_eq!(
            cleanup_engine_group(&dir, 3333, Duration::from_millis(50)),
            None,
            "负 pgid 同样表示无组可回收"
        );
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
        // OD-WR-314：旧夹具用 sh -c "sleep 30"，sh 可能以 exec 方式执行单条命令，
        // argv 会在登记与 reclaim 之间变成 sleep 30，测试偶发落到「身份不符」分支。
        // 改用 python 常驻：解释器持住 argv，不会把自己替换成 sleep。
        let python = engine_stub_python();
        let mut child = Command::new(&python)
            .arg("-c")
            .arg("import time; time.sleep(30)")
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
        assert!(err.contains("另一个 OmniDesk 实例"), "实际错误：{err}");
        assert!(is_alive(pid), "另一实例的引擎不得被回收");
        assert!(owner_path(&dir).exists(), "另一实例的登记不得被删除");
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
