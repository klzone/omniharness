//! 知识库写入与附件打开的宿主集成测试（OD-KB-001 P2-S02 / P2-S03）。
//!
//! 全部使用 /tmp 下的合成目录。这里直接调用 `*_for` 宿主函数，路径由宿主重新
//! 校验，和被 Tauri 命令调用的那条路径完全一致。
//!
//! 覆盖：新建成功且可被 P1 读回；拒绝已存在目标、越界、隐藏/排除目录、符号链接、
//! 非 Markdown、超限正文；正常保存；哈希冲突不写盘；保存后原文不被重排；
//! 附件只在校验通过后交给打开器，非法路径绝不到达打开器。

use std::fs;
use std::path::{Path, PathBuf};

use omnidesk_desktop_lib::app_paths::AppPaths;
use omnidesk_desktop_lib::knowledge_vault::{self, VAULT_MAX_NOTE_BYTES};
use omnidesk_desktop_lib::settings;
use omnidesk_desktop_lib::vault_open;
use omnidesk_desktop_lib::vault_write::{self, CONFLICT_PREFIX};
use omnidesk_desktop_lib::{
    knowledge_vault_create_note_for, knowledge_vault_parse_note_for,
    knowledge_vault_read_note_for, knowledge_vault_save_note_for,
};

struct Fixture {
    base: PathBuf,
    app: AppPaths,
    vault: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-vault-write-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let app_root = base.join("app");
        fs::create_dir_all(app_root.join("config")).unwrap();
        let vault = base.join("vault");
        fs::create_dir_all(vault.join("notes")).unwrap();
        let fx = Self {
            base,
            app: AppPaths::from_root(app_root),
            vault,
        };
        settings::set_knowledge_vault_root(&fx.app, fx.vault.to_str().unwrap()).unwrap();
        fx
    }

    fn write(&self, rel: &str, body: &[u8]) {
        let path = self.vault.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn create_then_read_and_parse_roundtrip() {
    let fx = Fixture::new("create");
    let content = "---\ntitle: 合成笔记\ntags: [项目/alpha]\n---\n\n# 正文\n\n第一行。\n";
    let created = knowledge_vault_create_note_for(&fx.app, "notes/新笔记.md", content).unwrap();
    assert_eq!(created.relative_path, "notes/新笔记.md");
    assert_eq!(created.content, content);
    assert_eq!(fs::read_to_string(fx.vault.join("notes/新笔记.md")).unwrap(), content);

    // 通过 P1 的只读通道读回，且解析结果的原文与刚写入的字节一致。
    let read = knowledge_vault_read_note_for(&fx.app, "notes/新笔记.md").unwrap();
    assert_eq!(read.content, content);
    let parsed = knowledge_vault_parse_note_for(&fx.app, "notes/新笔记.md").unwrap();
    assert_eq!(parsed.raw, content);
    assert_eq!(parsed.tags, vec!["项目/alpha"]);

    // 新建绝不覆盖已有目标。
    let err = knowledge_vault_create_note_for(&fx.app, "notes/新笔记.md", "覆盖").unwrap_err();
    assert!(err.contains("已存在"), "{err}");
    assert_eq!(fs::read_to_string(fx.vault.join("notes/新笔记.md")).unwrap(), content);

    // 不隐式创建父目录。
    assert!(knowledge_vault_create_note_for(&fx.app, "missing/a.md", "x").is_err());
    assert!(!fx.vault.join("missing").exists());
}

#[test]
fn create_rejects_unsafe_targets_and_oversize_content() {
    let fx = Fixture::new("reject");
    fx.write("existing.md", b"keep");
    fx.write("notes/ok.md", b"ok");
    fs::create_dir_all(fx.vault.join(".hidden")).unwrap();
    fs::create_dir_all(fx.vault.join("node_modules")).unwrap();
    let outside = fx.base.join("outside");
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, fx.vault.join("linkdir")).unwrap();
    std::os::unix::fs::symlink(fx.base.join("existing.md"), fx.vault.join("link.md")).unwrap();

    for bad in [
        "../escape.md",
        "/tmp/absolute.md",
        ".hidden/note.md",
        "node_modules/note.md",
        "linkdir/note.md",
        "link.md",
        "existing.md",
        "notes/pic.png",
        "notes/no-extension",
        "",
    ] {
        let result = knowledge_vault_create_note_for(&fx.app, bad, "x");
        assert!(result.is_err(), "应拒绝新建 {:?}", bad);
    }

    let big = "a".repeat((VAULT_MAX_NOTE_BYTES as usize) + 1);
    let err = knowledge_vault_create_note_for(&fx.app, "notes/big.md", &big).unwrap_err();
    assert!(err.contains("上限"), "{err}");
    assert!(!fx.vault.join("notes/big.md").exists());
    // 被拒绝的请求没有改动任何已有文件。
    assert_eq!(fs::read(fx.vault.join("existing.md")).unwrap(), b"keep");
    assert!(!outside.join("note.md").exists());
}

/// P2-F02：`.txt` 仍是只读笔记，但不能经由 Markdown 写入接口创建。
#[test]
fn create_rejects_plain_text_target_without_side_effects() {
    let fx = Fixture::new("txt-create");
    let err = knowledge_vault_create_note_for(&fx.app, "notes/纯文本.txt", "不应写入")
        .unwrap_err();
    assert!(err.contains("md / markdown"), "写入白名单必须只列 Markdown：{err}");
    assert!(!fx.vault.join("notes/纯文本.txt").exists());
    assert!(fs::read_dir(fx.vault.join("notes"))
        .unwrap()
        .all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
}

/// P2-F02：已有 `.txt` 不能被保存覆盖；同时它仍作为笔记可读取、可搜索。
#[test]
fn save_rejects_plain_text_target_and_keeps_read_only_support() {
    let fx = Fixture::new("txt-save");
    let original = "纯文本原文，关键词 needle。\n";
    fx.write("notes/readme.txt", original.as_bytes());
    let before = fs::read(fx.vault.join("notes/readme.txt")).unwrap();
    let opened = knowledge_vault_read_note_for(&fx.app, "notes/readme.txt").unwrap();
    assert_eq!(opened.content, original);

    let err = knowledge_vault_save_note_for(&fx.app, "notes/readme.txt", "改写", &opened.sha256)
        .unwrap_err();
    assert!(err.contains("md / markdown"), "写入白名单必须只列 Markdown：{err}");
    assert_eq!(
        fs::read(fx.vault.join("notes/readme.txt")).unwrap(),
        before,
        "被拒绝的保存不得改目标"
    );
    assert!(fs::read_dir(fx.vault.join("notes"))
        .unwrap()
        .all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));

    // `.txt` 仍是 P1 的只读笔记：扫描归类为 note，搜索能命中正文。
    let scan = knowledge_vault::scan_vault(&fx.vault, None).unwrap();
    assert!(scan
        .files
        .iter()
        .any(|f| f.relative_path == "notes/readme.txt" && f.kind == "note"));
    let result = omnidesk_desktop_lib::vault_search::search_scanned(
        &fx.vault,
        &scan.files,
        &omnidesk_desktop_lib::vault_search::SearchQuery {
            keyword: "NEEDLE".into(),
            ..Default::default()
        },
    );
    assert_eq!(result.notes_examined, 1);
    assert_eq!(result.hits.len(), 1, "{:?}", result.hits);
    assert_eq!(result.hits[0].relative_path, "notes/readme.txt");
    assert_eq!(result.hits[0].match_areas, vec!["body"]);
}

/// P2-F02：`.md` 与 `.markdown` 仍然是可用的写入目标。
#[test]
fn create_and_save_still_accept_both_markdown_extensions() {
    let fx = Fixture::new("markdown-exts");
    for (path, content) in [
        ("notes/a.md", "# A\n"),
        ("notes/b.markdown", "# B\n"),
    ] {
        let created = knowledge_vault_create_note_for(&fx.app, path, content).unwrap();
        assert_eq!(created.content, content);
        let saved = knowledge_vault_save_note_for(
            &fx.app,
            path,
            &format!("{content}追加\n"),
            &created.sha256,
        )
        .unwrap();
        assert_eq!(saved.content, format!("{content}追加\n"));
    }
}

#[test]
fn save_writes_and_preserves_unknown_or_invalid_frontmatter_verbatim() {
    let fx = Fixture::new("save");
    // 未知键、奇怪的缩进和 CRLF：保存层不得重排或规范化。
    let original = "---\r\ntitle: 原样\r\nweird:\r\n  nested: true\r\n---\r\n\r\n正文\r\n";
    fx.write("notes/raw.md", original.as_bytes());
    let opened = knowledge_vault_read_note_for(&fx.app, "notes/raw.md").unwrap();

    let edited = "---\r\ntitle: 原样\r\nweird:\r\n  nested: true\r\n---\r\n\r\n正文改了\r\n";
    let saved =
        knowledge_vault_save_note_for(&fx.app, "notes/raw.md", edited, &opened.sha256).unwrap();
    assert_eq!(saved.content, edited);
    assert_eq!(saved.relative_path, "notes/raw.md");
    assert!(!saved.race_window.is_empty());
    assert_eq!(fs::read_to_string(fx.vault.join("notes/raw.md")).unwrap(), edited);

    // 无效 YAML 也必须原样保留。
    let broken = "---\ntitle: [未闭合\n---\n正文\n";
    fx.write("notes/broken.md", broken.as_bytes());
    let note = knowledge_vault_read_note_for(&fx.app, "notes/broken.md").unwrap();
    let next = "---\ntitle: [未闭合\n---\n正文已改\n";
    knowledge_vault_save_note_for(&fx.app, "notes/broken.md", next, &note.sha256).unwrap();
    assert_eq!(fs::read_to_string(fx.vault.join("notes/broken.md")).unwrap(), next);
    assert!(note.content.starts_with("---\ntitle: [未闭合"));
}

#[test]
fn save_conflict_does_not_touch_the_file() {
    let fx = Fixture::new("conflict");
    fx.write("notes/a.md", "# 版本 1\n".as_bytes());
    let opened = knowledge_vault_read_note_for(&fx.app, "notes/a.md").unwrap();

    // 模拟外部编辑器在打开之后改动了文件。
    fx.write("notes/a.md", "# 外部版本\n".as_bytes());
    let before = fs::read(fx.vault.join("notes/a.md")).unwrap();

    let err = knowledge_vault_save_note_for(&fx.app, "notes/a.md", "# 我的草稿\n", &opened.sha256)
        .unwrap_err();
    assert!(err.starts_with(CONFLICT_PREFIX), "{err}");
    assert!(err.contains("未写入"), "{err}");
    assert_eq!(
        fs::read(fx.vault.join("notes/a.md")).unwrap(),
        before,
        "冲突时必须保持外部写入的内容不变"
    );

    // 缺失或格式错误的版本标识同样拒绝。
    for bad in ["", "abc", &"z".repeat(64)] {
        assert!(knowledge_vault_save_note_for(&fx.app, "notes/a.md", "x", bad).is_err());
    }

    // 目标不存在（新建请走 create）与附件路径同样拒绝保存。
    assert!(knowledge_vault_save_note_for(&fx.app, "notes/missing.md", "x", &"a".repeat(64)).is_err());
    fx.write("notes/pic.png", b"png");
    let png = knowledge_vault_read_note_for(&fx.app, "notes/pic.png");
    assert!(png.is_err());
}

#[test]
fn failed_temp_write_leaves_original_intact() {
    let fx = Fixture::new("tempfail");
    fx.write("notes/a.md", "# 原始\n".as_bytes());
    let before = fs::read(fx.vault.join("notes/a.md")).unwrap();
    // 只读目录：临时文件无法创建，写入必须失败且原文件不动。
    let mut perms = fs::metadata(fx.vault.join("notes")).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(fx.vault.join("notes"), perms).unwrap();
    let opened = knowledge_vault_read_note_for(&fx.app, "notes/a.md").unwrap();
    let result = knowledge_vault_save_note_for(&fx.app, "notes/a.md", "# 新\n", &opened.sha256);
    let mut perms = fs::metadata(fx.vault.join("notes")).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(fx.vault.join("notes"), perms).unwrap();
    assert!(result.is_err(), "只读目录应写入失败");
    assert_eq!(fs::read(fx.vault.join("notes/a.md")).unwrap(), before);
    // 失败后没有遗留临时文件。
    let leftovers: Vec<_> = fs::read_dir(fx.vault.join("notes"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "留下了临时文件：{:?}", leftovers);
}

#[test]
fn attachment_open_rejects_everything_unvalidated() {
    let fx = Fixture::new("attach");
    fx.write("assets/pic.png", b"synthetic");
    fx.write("notes/a.md", b"# A");
    fs::create_dir_all(fx.vault.join(".hidden")).unwrap();
    fs::write(fx.vault.join(".hidden/x.png"), b"x").unwrap();
    fs::create_dir_all(fx.vault.join("node_modules")).unwrap();
    fs::write(fx.vault.join("node_modules/x.png"), b"x").unwrap();
    let outside = fx.base.join("outside.png");
    fs::write(&outside, b"outside").unwrap();
    std::os::unix::fs::symlink(&outside, fx.vault.join("link.png")).unwrap();

    let ok = vault_open::open_attachment_with(&fx.vault, "assets/pic.png", |p| {
        assert_eq!(fs::read(p).unwrap(), b"synthetic");
        Ok(())
    })
    .unwrap();
    assert_eq!(ok, "assets/pic.png");

    for bad in [
        "../outside.png",
        "/etc/passwd",
        "link.png",
        ".hidden/x.png",
        "node_modules/x.png",
        "notes/a.md",
        "missing.png",
        "assets",
        "",
    ] {
        let mut called = false;
        let result = vault_open::open_attachment_with(&fx.vault, bad, |_| {
            called = true;
            Ok(())
        });
        assert!(result.is_err(), "应拒绝附件 {:?}", bad);
        assert!(!called, "非法附件不得调用打开器：{:?}", bad);
    }
}

/// 校验 `create` / `save` 通过宿主函数时使用的根，与直接调用的根一致。
#[test]
fn host_write_paths_resolve_to_config() {
    let fx = Fixture::new("hostpath");
    assert_eq!(
        omnidesk_desktop_lib::configured_vault_root(&fx.app).unwrap(),
        fs::canonicalize(&fx.vault).unwrap()
    );
    let content = "# 直接调用\n";
    let root = fx.vault.clone();
    vault_write::create_note(&root, "notes/direct.md", content).unwrap();
    assert!(Path::new(&fx.vault).join("notes/direct.md").is_file());
    assert_eq!(
        knowledge_vault::read_vault_note(&root, "notes/direct.md").unwrap().content,
        content
    );
}
