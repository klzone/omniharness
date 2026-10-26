//! 知识库附件安全打开（OD-KB-001 P2-S03）。
//!
//! 只有用户明确点击附件时才调用这里。校验与「交给系统处理程序」分开：
//!   * 复用 P1 `resolve_vault_file`，逐层拒绝根外、`..`、符号链接、隐藏/排除目录、
//!     非普通文件与不存在的目标；
//!   * 只接受附件（非 Markdown/纯文本），不读取、不提取、不回传附件正文；
//!   * 打开器是固定的 `xdg-open` / `gio open`，文件路径作为独立参数传入，
//!     不拼 shell、不接受调用方提供的任意命令或 URL。
//!
//! 本仓库没有安装 Tauri shell/opener 插件；复用 `external_open.rs` 只允许 http(s)，
//! 不能打开本地文件。因此这里沿用仓库既有的 `Command::new(...).arg(...)` 方式。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::knowledge_vault::{resolve_vault_file, VAULT_NOTE_EXTS};

/// 校验附件路径并返回真实落点。不启动任何程序。
pub fn resolve_attachment(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let resolved = resolve_vault_file(root, relative)?;
    let extension = resolved
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if VAULT_NOTE_EXTS.contains(&extension.as_str()) {
        return Err(format!(
            "{} 是笔记，已在知识库内预览；附件打开只接受非 {} 文件。",
            relative,
            VAULT_NOTE_EXTS.join(" / ")
        ));
    }
    Ok(resolved)
}

/// 用系统默认处理程序打开附件。仅在校验通过后调用 `opener`。
/// 返回值是已校验的相对路径，不带附件正文。
pub fn open_attachment_with<F>(root: &Path, relative: &str, opener: F) -> Result<String, String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    let path = resolve_attachment(root, relative)?;
    opener(&path)?;
    Ok(relative.trim().replace('\\', "/"))
}

/// 固定打开器：只尝试 `xdg-open` 与 `gio open`，路径作为独立参数。
pub fn open_attachment(root: &Path, relative: &str) -> Result<String, String> {
    open_attachment_with(root, relative, system_open)
}

fn system_open(path: &Path) -> Result<(), String> {
    let mut last_error = String::new();
    for (program, args) in [
        ("xdg-open", Vec::<&str>::new()),
        ("gio", vec!["open"]),
    ] {
        match Command::new(program)
            .args(&args)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                // 打开器存在但立刻失败（例如没有注册处理程序）时要让用户看到错误，
                // 不能把 spawn 成功当成打开成功。短暂等待后仍在运行的视为已启动。
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1200);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) if status.success() => return Ok(()),
                        Ok(Some(status)) => {
                            last_error = format!("{}：退出状态 {}", program, status);
                            break;
                        }
                        Ok(None) if std::time::Instant::now() < deadline => {
                            std::thread::sleep(std::time::Duration::from_millis(40));
                        }
                        Ok(None) => return Ok(()),
                        Err(e) => {
                            last_error = format!("{}：无法确认打开结果（{}）", program, e);
                            break;
                        }
                    }
                }
            }
            Err(e) => last_error = format!("{}：{}", program, e),
        }
    }
    Err(format!(
        "系统没有可用的附件打开器（{}）。知识库浏览与编辑不受影响。",
        last_error
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-vault-open-{tag}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&base);
            let root = base.join("vault");
            fs::create_dir_all(root.join("assets")).unwrap();
            fs::create_dir_all(root.join("notes")).unwrap();
            fs::write(root.join("assets/pic.png"), b"synthetic png").unwrap();
            fs::write(root.join("notes/a.md"), b"# A").unwrap();
            Self { base, root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn opens_only_validated_attachment() {
        let fx = Fixture::new("ok");
        let calls = AtomicUsize::new(0);
        let opened = open_attachment_with(&fx.root, "assets/pic.png", |path| {
            calls.fetch_add(1, Ordering::SeqCst);
            assert!(path.ends_with("assets/pic.png"));
            assert_eq!(fs::read(path).unwrap(), b"synthetic png");
            Ok(())
        })
        .unwrap();
        assert_eq!(opened, "assets/pic.png");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn invalid_paths_never_reach_the_opener() {
        let fx = Fixture::new("reject");
        let outside = fx.base.join("outside.png");
        fs::write(&outside, b"outside").unwrap();
        std::os::unix::fs::symlink(&outside, fx.root.join("assets/link.png")).unwrap();
        fs::create_dir_all(fx.root.join(".hidden")).unwrap();
        fs::write(fx.root.join(".hidden/x.png"), b"hidden").unwrap();
        fs::create_dir_all(fx.root.join("node_modules")).unwrap();
        fs::write(fx.root.join("node_modules/x.png"), b"excluded").unwrap();

        for bad in [
            "../outside.png",
            "/etc/passwd",
            "assets/link.png",
            ".hidden/x.png",
            "node_modules/x.png",
            "notes/a.md",
            "missing.png",
            "assets",
            "",
        ] {
            let mut called = false;
            let result = open_attachment_with(&fx.root, bad, |_| {
                called = true;
                Ok(())
            });
            assert!(result.is_err(), "应拒绝 {bad:?}");
            assert!(!called, "非法路径不得调用打开器：{bad:?}");
        }
    }

    #[test]
    fn opener_failure_is_visible_and_does_not_pretend_success() {
        let fx = Fixture::new("failure");
        let err = open_attachment_with(&fx.root, "assets/pic.png", |_| {
            Err("合成：没有打开器".to_string())
        })
        .unwrap_err();
        assert!(err.contains("没有打开器"), "{err}");
    }
}
