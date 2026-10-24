//! OD-ARCH-072 验收：停用插件后，真实隔离实例启动日志里不再加载它。
//!
//! 只有在设置 OMNIDESK_VERIFY_DSH_HOME（一个可丢弃的隔离副本）和 OMNIDESK_VERIFY_DSH_CLI
//! 时才运行；否则跳过并说明，避免测试自己去复制用户数据根。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let meta = entry.metadata()?;
        if meta.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if meta.file_type().is_symlink() {
            let link = fs::read_link(entry.path())?;
            let _ = std::os::unix::fs::symlink(link, &target);
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[test]
fn disabled_plugin_is_absent_from_startup_log() {
    let (Ok(src), Ok(cli)) = (
        std::env::var("OMNIDESK_VERIFY_DSH_HOME"),
        std::env::var("OMNIDESK_VERIFY_DSH_CLI"),
    ) else {
        eprintln!("跳过：未设置 OMNIDESK_VERIFY_DSH_HOME / OMNIDESK_VERIFY_DSH_CLI");
        return;
    };
    let base = PathBuf::from(src);
    let temp = std::env::temp_dir().join(format!("omnidesk-cap-verify-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    copy_dir(&base, &temp).expect("复制隔离副本");
    // 环境变量本身就是 DSH_HOME（隔离副本的根），复制后内容直接落在 temp 下。
    let home = temp.clone();
    let profile_dir = home.join("profiles").join("web");

    let receipt = omnidesk_desktop_lib::capabilities::set_bundle(&profile_dir, "dsh-cost-meter", false)
        .expect("停用应成功");
    assert!(receipt.backup.is_some(), "必须留备份");
    let backup = PathBuf::from(receipt.backup.clone().unwrap());
    assert!(backup.is_file(), "备份文件应真实存在");

    let log = temp.join("startup.log");
    let mut child = Command::new("node")
        .arg(&cli)
        .arg("web")
        .arg("--no-open")
        .arg("--port")
        .arg("0")
        .env("DSH_HOME", &home)
        .env("DSH_AGENTS_HOME", temp.join("agents-home"))
        .stdout(Stdio::from(fs::File::create(&log).expect("建日志")))
        .stderr(Stdio::from(fs::OpenOptions::new().append(true).open(&log).expect("开日志")))
        .spawn()
        .expect("启动 DSH");

    let deadline = Instant::now() + Duration::from_secs(40);
    let mut ready = false;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(300));
        let text = fs::read_to_string(&log).unwrap_or_default();
        if text.contains("dsh web: http") {
            ready = true;
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();

    let text = fs::read_to_string(&log).unwrap_or_default();
    assert!(ready, "隔离实例没起来：{}\n{}", log.display(), text);
    assert!(
        !text.contains("[dsh-cost-meter]"),
        "停用后启动日志不应加载 dsh-cost-meter：\n{}",
        text
    );

    let restart = omnidesk_desktop_lib::capabilities::set_bundle(&profile_dir, "dsh-cost-meter", true)
        .expect("启用应成功");
    assert!(restart.backup.is_some());
    let manifest = fs::read_to_string(profile_dir.join("package.json")).unwrap();
    assert!(manifest.contains("\"dsh-cost-meter\""));

    let mut again = Command::new("node")
        .arg(&cli)
        .arg("web")
        .arg("--no-open")
        .arg("--port")
        .arg("0")
        .env("DSH_HOME", &home)
        .env("DSH_AGENTS_HOME", temp.join("agents-home"))
        .stdout(Stdio::from(fs::File::create(&log).expect("建日志")))
        .stderr(Stdio::from(fs::OpenOptions::new().append(true).open(&log).expect("开日志")))
        .spawn()
        .expect("再次启动 DSH");
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut ready = false;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(300));
        let text = fs::read_to_string(&log).unwrap_or_default();
        if text.contains("dsh web: http") {
            ready = true;
            break;
        }
    }
    let _ = again.kill();
    let _ = again.wait();
    let text = fs::read_to_string(&log).unwrap_or_default();
    assert!(ready, "重新启用后实例没起来：\n{}", text);
    assert!(text.contains("[dsh-cost-meter]"), "重新启用后日志应加载插件：\n{}", text);

    let _ = fs::remove_dir_all(&temp);
}

