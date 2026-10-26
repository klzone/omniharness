//! 知识库链接索引的宿主集成测试（OD-KB-001 P3-S04）。
//!
//! 只用 /tmp 下的合成知识库；直接走宿主 `*_for` 函数，与 Tauri 命令一致地
//! 重新校验设置里的根。验证只读、零写入、关系查询与换根拒绝旧快照。

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use omnidesk_desktop_lib::app_paths::AppPaths;
use omnidesk_desktop_lib::settings;
use omnidesk_desktop_lib::{
    knowledge_vault_note_links_for, knowledge_vault_rebuild_index_for,
    knowledge_vault_scan_for,
};
use omnidesk_desktop_lib::vault_index::{self, IndexBuildOptions};

struct Fixture {
    base: PathBuf,
    app: AppPaths,
    vault: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-vault-index-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let app = AppPaths::from_root(base.join("app"));
        fs::create_dir_all(app.config.clone()).unwrap();
        let vault = base.join("vault");
        fs::create_dir_all(vault.join("notes")).unwrap();
        fs::create_dir_all(vault.join(".obsidian")).unwrap();
        fs::write(vault.join(".obsidian/app.json"), "{}").unwrap();
        let fx = Self { base, app, vault };
        settings::set_knowledge_vault_root(&fx.app, fx.vault.to_str().unwrap()).unwrap();
        fx
    }

    fn write(&self, rel: &str, body: &str) {
        let path = self.vault.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    fn tree(&self) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    let rel = path.strip_prefix(root).unwrap().to_string_lossy().to_string();
                    out.insert(rel, fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.vault, &self.vault, &mut out);
        out
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn rebuild_returns_relations_without_writing_vault() {
    let fx = Fixture::new("relations");
    fx.write("Home.md", "---\ntitle: Home\n---\n\n[[notes/Target]] 与 [[Missing]]\n");
    fx.write(
        "notes/Target.md",
        "# Target\n\n## 细节\n\n回到 [[Home]] 与 [[Home#标题]]\n",
    );
    fx.write("notes/Mid.md", "[[Target#细节]]\n");
    fx.write("attach.pdf", "synthetic");
    let before = fx.tree();

    let index = knowledge_vault_rebuild_index_for(&fx.app, None).unwrap();
    assert!(index.complete, "{index:#?}");
    assert_eq!(index.generation, 1);
    let home = knowledge_vault_note_links_for(&index, &fx.vault, "Home.md").unwrap();
    assert!(home
        .outgoing
        .iter()
        .any(|l| l.target_path.as_deref() == Some("notes/Target.md")));
    assert!(home.outgoing.iter().any(|l| l.status == "unresolved"));

    let target = knowledge_vault_note_links_for(&index, &fx.vault, "notes/Target.md").unwrap();
    assert!(target.backlinks.len() >= 2, "{target:#?}");
    assert!(target
        .backlinks
        .iter()
        .any(|l| l.anchor.as_deref() == Some("细节")));
    assert!(!target.note.orphan);

    let after = fx.tree();
    assert_eq!(before, after, "索引构建不得改动知识库任何文件");
}

#[test]
fn generation_increments_and_external_changes_are_reflected() {
    let fx = Fixture::new("refresh");
    fx.write("A.md", "[[B]]\n");
    fx.write("B.md", "# B\n");
    let first = knowledge_vault_rebuild_index_for(&fx.app, None).unwrap();
    assert_eq!(first.generation, 1);
    fx.write("C.md", "# C\n");
    fx.write("A.md", "[[B]]\n[[C]]\n");
    let second = knowledge_vault_rebuild_index_for(&fx.app, Some(&first)).unwrap();
    assert_eq!(second.generation, 2);
    assert!(second
        .links
        .iter()
        .any(|l| l.target_path.as_deref() == Some("C.md")));
    assert_ne!(first.fingerprint, second.fingerprint);
}

#[test]
fn stale_index_from_another_root_is_rejected() {
    let first = Fixture::new("root-a");
    first.write("A.md", "# A\n");
    let index = knowledge_vault_rebuild_index_for(&first.app, None).unwrap();
    let other = first.base.join("vault-b");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("X.md"), "# X\n").unwrap();
    settings::set_knowledge_vault_root(&first.app, other.to_str().unwrap()).unwrap();
    let err = knowledge_vault_note_links_for(&index, &other, "X.md").unwrap_err();
    assert!(err.contains("另一个知识库目录"), "{err}");
}

#[test]
fn no_settings_root_reports_explicit_error() {
    let fx = Fixture::new("unset");
    settings::clear_knowledge_vault_root(&fx.app).unwrap();
    let err = knowledge_vault_rebuild_index_for(&fx.app, None).unwrap_err();
    assert!(err.contains("还没有选择知识库目录"), "{err}");
}

#[test]
fn scan_and_index_agree_on_file_inventory() {
    let fx = Fixture::new("inventory");
    fx.write("A.md", "# A\n\n[[B]]\n");
    fx.write("B.markdown", "# B\n");
    fx.write("notes/C.txt", "plain\n");
    let scan = knowledge_vault_scan_for(&fx.app).unwrap();
    let notes: Vec<&str> = scan
        .files
        .iter()
        .filter(|f| f.kind == "note")
        .map(|f| f.relative_path.as_str())
        .collect();
    let index = vault_index::build_from_scan(
        &fx.vault,
        &scan,
        IndexBuildOptions::default(),
        1,
    );
    let mut indexed: Vec<&str> = index.notes.iter().map(|n| n.relative_path.as_str()).collect();
    let mut noticed = notes.clone();
    indexed.sort();
    noticed.sort();
    assert_eq!(indexed, noticed);
    assert_eq!(index.stats.notes_scanned, notes.len());
}
