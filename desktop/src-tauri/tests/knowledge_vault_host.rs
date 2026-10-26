//! 知识库只读宿主接口集成测试（OD-KB-001 P1-S04）。
//!
//! 覆盖：设置 → 状态 → 扫描 → 刷新 → 读取笔记；根外/穿越/符号链接/非普通文件/
//! 超限/非 UTF-8 在宿主函数被拒绝；Obsidian 配置目录整棵跳过且不被修改。
//!
//! 全部使用 /tmp 下的合成目录，不接触真实知识库、真实 DSH 或模型。

use std::fs;
use std::path::{Path, PathBuf};

use omnidesk_desktop_lib::app_paths::AppPaths;
use omnidesk_desktop_lib::knowledge_vault;
use omnidesk_desktop_lib::settings;
use omnidesk_desktop_lib::{
    configured_vault_root, knowledge_vault_read_note_for, knowledge_vault_scan_for,
    knowledge_vault_status_for,
};

struct Fixture {
    base: PathBuf,
    app: AppPaths,
    vault: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-vault-host-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let app_root = base.join("app");
        fs::create_dir_all(app_root.join("config")).unwrap();
        let vault = base.join("vault");
        fs::create_dir_all(&vault).unwrap();
        Self {
            base,
            app: AppPaths::from_root(app_root),
            vault,
        }
    }

    fn write(&self, rel: &str, body: &[u8]) {
        let path = self.vault.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    fn settings_path(&self) -> PathBuf {
        settings::settings_path(&self.app.config)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// 目录树快照：相对路径 + 内容 + 修改时间。用来证明扫描/读取没有改动文件。
fn snapshot(root: &Path) -> Vec<(String, Vec<u8>, Option<std::time::SystemTime>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).unwrap();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if meta.is_dir() {
                out.push((rel, Vec::new(), None));
                stack.push(path);
            } else if meta.is_file() {
                out.push((rel, fs::read(&path).unwrap(), meta.modified().ok()));
            } else {
                out.push((rel, Vec::new(), None));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn set_scan_refresh_and_read_roundtrip() {
    let fx = Fixture::new("roundtrip");
    fx.write("index.md", "# 合成首页\n\n正文".as_bytes());
    fx.write("notes/项目.md", "# 项目\n\n[[index]]".as_bytes());
    fx.write("notes/readme.txt", "纯文本".as_bytes());
    fx.write("assets/pic.png", b"PNG-not-real");
    fx.write(".obsidian/app.json", br#"{"legacyEditor":false}"#);
    fx.write(".obsidian/workspace.json", b"{}");
    // 设置文件写在应用数据根，知识库目录本身必须一字节不变。
    let before = snapshot(&fx.vault);

    // 未选择时：状态为未配置，扫描给出可解释错误而不是空结果。
    let status = knowledge_vault_status_for(&fx.app);
    assert!(!status.configured);
    assert!(status.root.is_none());
    assert!(knowledge_vault_scan_for(&fx.app).is_err());

    // 设置 → 状态。
    let saved = settings::set_knowledge_vault_root(&fx.app, fx.vault.to_str().unwrap()).unwrap();
    assert_eq!(saved, fs::canonicalize(&fx.vault).unwrap());
    let status = knowledge_vault_status_for(&fx.app);
    assert!(status.configured);
    assert!(status.exists && status.is_dir);
    assert_eq!(status.root.as_deref(), Some(saved.to_str().unwrap()));
    assert!(status.error.is_none());

    // 扫描：只收普通文件，隐藏目录整棵跳过。
    let scan = knowledge_vault_scan_for(&fx.app).unwrap();
    let paths: Vec<&str> = scan.files.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(
        paths,
        vec!["assets/pic.png", "index.md", "notes/readme.txt", "notes/项目.md"]
    );
    assert_eq!(scan.stats.notes, 3);
    assert_eq!(scan.stats.attachments, 1);
    assert_eq!(scan.stats.errors, 0);
    assert_eq!(scan.stats.skipped_hidden, 1, "只有 .obsidian 一个隐藏目录");
    assert!(scan
        .skipped
        .iter()
        .any(|s| s.relative_path == ".obsidian" && s.reason == "hidden"));
    assert!(!paths.iter().any(|p| p.contains(".obsidian")));

    // 刷新走同一只读路径，结果稳定。
    let refreshed = knowledge_vault_scan_for(&fx.app).unwrap();
    assert_eq!(scan, refreshed);

    // 读取笔记。
    let note = knowledge_vault_read_note_for(&fx.app, "notes/项目.md").unwrap();
    assert_eq!(note.content, "# 项目\n\n[[index]]");
    assert_eq!(note.relative_path, "notes/项目.md");
    assert_eq!(note.extension, "md");
    assert_eq!(note.sha256.len(), 64);

    assert_eq!(
        snapshot(&fx.vault),
        before,
        "设置/扫描/读取不得改动知识库内任何文件"
    );
}

#[test]
fn host_rejects_traversal_outside_symlink_non_regular_and_oversize() {
    let fx = Fixture::new("reject");
    fx.write("ok.md", b"x");
    fx.write(
        "big.md",
        &vec![b'a'; (knowledge_vault::VAULT_MAX_NOTE_BYTES as usize) + 8],
    );
    fx.write("binary.md", &[0xff, 0xfe, 0x00]);
    fx.write("pic.png", b"x");
    let outside = fx.base.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.md"), b"outer secret").unwrap();
    std::os::unix::fs::symlink(outside.join("secret.md"), fx.vault.join("link.md")).unwrap();
    std::os::unix::fs::symlink(&outside, fx.vault.join("linkdir")).unwrap();
    let fifo = fx.vault.join("pipe.md");
    let fifo_ok = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    settings::set_knowledge_vault_root(&fx.app, fx.vault.to_str().unwrap()).unwrap();

    for bad in [
        "../outside/secret.md",
        "/etc/passwd",
        "notes/../../outside/secret.md",
        "",
        "./ok.md",
        "link.md",
        "linkdir/secret.md",
        "big.md",
        "binary.md",
        "pic.png",
    ] {
        assert!(
            knowledge_vault_read_note_for(&fx.app, bad).is_err(),
            "应拒绝 {:?}",
            bad
        );
    }

    let scan = knowledge_vault_scan_for(&fx.app).unwrap();
    assert_eq!(scan.stats.skipped_symlink, 2);
    assert!(!scan
        .files
        .iter()
        .any(|f| f.relative_path.contains("secret")));
    if fifo_ok {
        assert_eq!(scan.stats.skipped_non_file, 1);
    }
    // 错误信息只报路径与原因，不回显文件内容。
    for bad in ["link.md", "big.md"] {
        let err = knowledge_vault_read_note_for(&fx.app, bad).unwrap_err();
        assert!(!err.contains("outer secret"), "{}", err);
    }
}

#[test]
fn saved_root_reverified_on_every_call() {
    let fx = Fixture::new("reverify");
    fx.write("a.md", b"x");
    settings::set_knowledge_vault_root(&fx.app, fx.vault.to_str().unwrap()).unwrap();
    assert!(configured_vault_root(&fx.app).is_ok());

    // 设置文件被改成禁区后，下一次扫描必须拒绝，而不是沿用旧结果。
    let mut stored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fx.settings_path()).unwrap()).unwrap();
    stored["knowledge_vault_root"] = serde_json::json!("/home/u/.codex/vault");
    fs::write(fx.settings_path(), stored.to_string()).unwrap();
    let status = knowledge_vault_status_for(&fx.app);
    assert!(status.configured);
    assert!(status.error.is_some(), "{:?}", status);
    assert!(knowledge_vault_scan_for(&fx.app).is_err());

    // 根被删除后明确报未就绪。
    stored["knowledge_vault_root"] = serde_json::json!(fx.vault);
    fs::write(fx.settings_path(), stored.to_string()).unwrap();
    fs::remove_dir_all(&fx.vault).unwrap();
    let status = knowledge_vault_status_for(&fx.app);
    assert!(!status.exists);
    assert!(status.error.is_some());
}

#[test]
fn library_root_and_settings_are_not_rewritten_by_vault_use() {
    let fx = Fixture::new("separation");
    fx.write("a.md", b"x");
    let library = fx.base.join("library");
    fs::create_dir_all(&library).unwrap();
    settings::set_library_root(&fx.app, library.to_str().unwrap()).unwrap();
    let library_before = settings::configured_library_root(&fx.app.config)
        .unwrap()
        .unwrap();

    settings::set_knowledge_vault_root(&fx.app, fx.vault.to_str().unwrap()).unwrap();
    let _ = knowledge_vault_scan_for(&fx.app).unwrap();
    assert_eq!(
        settings::configured_library_root(&fx.app.config)
            .unwrap()
            .unwrap(),
        library_before,
        "知识库扫描不得改写库根"
    );
    assert_eq!(
        settings::effective_library_root(&fx.app.config).unwrap().root,
        library_before
    );
    assert!(!fx.vault.join("知识库").exists(), "不得创建知识库子目录");
}

/// P1-S06：普通 Markdown 根（没有 .obsidian）同样可扫描，隐藏项计数为 0。
#[test]
fn plain_markdown_root_needs_no_obsidian_metadata() {
    let fx = Fixture::new("plain");
    fx.write("a.md", b"# A");
    fx.write("b.markdown", b"# B");
    fx.write("c.txt", b"plain");
    fx.write("sub/d.md", b"# D");
    fx.write("sub/attach.pdf", b"%PDF-1.4 fake");
    settings::set_knowledge_vault_root(&fx.app, fx.vault.to_str().unwrap()).unwrap();

    let scan = knowledge_vault_scan_for(&fx.app).unwrap();
    assert_eq!(scan.stats.files, 5);
    assert_eq!(scan.stats.notes, 4);
    assert_eq!(scan.stats.attachments, 1);
    assert_eq!(scan.stats.skipped_hidden, 0);
    assert_eq!(scan.stats.errors, 0);
    assert!(!fx.vault.join(".obsidian").exists(), "扫描不得创建 Obsidian 配置目录");
    let note = knowledge_vault_read_note_for(&fx.app, "b.markdown").unwrap();
    assert_eq!(note.extension, "markdown");
    assert_eq!(note.content, "# B");
}

/// P1-S06：固定规模的合成根，重复实测冷扫描耗时并输出可解析指标。
///
/// 规模与耗时都是实测值，不是目标值；输出一行 `KB_P1_SCAN_METRIC {json}` 供
/// tests/verification/desktop/verify_knowledge_vault_p1.py 收集。
#[test]
fn cold_scan_scale_is_measured_on_synthetic_root() {
    let fx = Fixture::new("scale");
    const DIRS: usize = 60;
    const NOTES_PER_DIR: usize = 20;
    for d in 0..DIRS {
        let dir = format!("notes/group-{:02}", d);
        for n in 0..NOTES_PER_DIR {
            fx.write(
                &format!("{}/note-{:03}.md", dir, n),
                format!("# 合成笔记 {}-{}\n\n边界测试正文。", d, n).as_bytes(),
            );
        }
    }
    fx.write(".obsidian/app.json", b"{}");
    fx.write("assets/one.png", b"x");
    settings::set_knowledge_vault_root(&fx.app, fx.vault.to_str().unwrap()).unwrap();

    let mut runs = Vec::new();
    for _ in 0..3 {
        let started = std::time::Instant::now();
        let scan = knowledge_vault_scan_for(&fx.app).unwrap();
        runs.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(scan.stats.files, DIRS * NOTES_PER_DIR + 1);
        assert_eq!(scan.stats.notes, DIRS * NOTES_PER_DIR);
        assert_eq!(scan.stats.skipped_hidden, 1);
        assert!(!scan.stats.truncated, "规模应低于扫描上限");
    }
    let files = DIRS * NOTES_PER_DIR + 1;
    let median = {
        let mut sorted = runs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted[sorted.len() / 2]
    };
    println!(
        "KB_P1_SCAN_METRIC {}",
        serde_json::json!({
            "synthetic_dirs": DIRS,
            "synthetic_files": files,
            "synthetic_notes": DIRS * NOTES_PER_DIR,
            "runs_ms": runs.iter().map(|v| (v * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
            "median_ms": (median * 1000.0).round() / 1000.0,
        })
    );
}

