//! P3-S02 / P3-S03 合成 Vault 夹具：目标解析、关系一致性、锚点定位与快照完整性。

use super::*;
use std::fs;
use std::path::PathBuf;

fn temp_vault(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "od-kb-p3-index-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&base).unwrap();
    base
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

/// 固定合成库。内容只使用虚构名称，不读取任何真实知识库。
fn sample_vault(tag: &str) -> PathBuf {
    let root = temp_vault(tag);
    write(
        &root,
        "Alpha.md",
        "---\ntitle: Alpha\naliases: [甲, 共同别名]\n---\n# 开始\n\n[[Beta]]\n[[Beta#细节]]\n[[Gamma|显示名]]\n[[#开始]]\n[[缺失笔记]]\n[[重复]]\n[[图.png]]\n[外部](https://example.com)\n\n## 细节\n\n段落 ^blk-1\n",
    );
    write(
        &root,
        "Beta.md",
        "# Beta\n\n## 细节\n\n回链 [[Alpha]] 与 [[共同别名]]\n",
    );
    write(
        &root,
        "Gamma.md",
        "---\naliases: [共同别名]\n---\n\n[[Alpha#^blk-1]]\n",
    );
    write(&root, "重复.md", "# 根重复\n");
    write(&root, "sub/重复.md", "# 子目录重复\n");
    write(
        &root,
        "sub/Path.md",
        "[[../Alpha.md]]\n[[Alpha.md]]\n[[ALPHA]]\n[[../Beta]]\n",
    );
    write(&root, "Orphan.md", "# Orphan\n\n没有入链。\n");
    write(&root, "图.png", "synthetic");
    root
}

fn build(root: &Path) -> LinkIndex {
    build_from_vault(root, None, IndexBuildOptions::default(), 1_700_000_000).unwrap()
}

fn link<'a>(index: &'a LinkIndex, source: &str, target_raw: &str) -> &'a IndexLink {
    index
        .links
        .iter()
        .find(|l| l.source_path == source && l.target_raw == target_raw)
        .unwrap_or_else(|| panic!("缺少引用 {source} -> {target_raw}: {:#?}", index.links))
}

#[test]
fn resolves_outgoing_backlinks_and_aliases() {
    let root = sample_vault("relations");
    let index = build(&root);

    let beta = link(&index, "Alpha.md", "Beta");
    assert_eq!(beta.status, RESOLVED);
    assert_eq!(beta.target_path.as_deref(), Some("Beta.md"));

    let alpha_links = note_links(&index, "Alpha.md").unwrap();
    assert_eq!(alpha_links.outgoing.len(), 8, "{:#?}", alpha_links.outgoing);
    let back = note_links(&index, "Beta.md").unwrap();
    assert!(back
        .backlinks
        .iter()
        .any(|l| l.source_path == "Alpha.md" && l.target_path.as_deref() == Some("Beta.md")));
    let note = index.notes.iter().find(|n| n.relative_path == "Beta.md").unwrap();
    assert!(note.backlink_count >= 2, "{note:#?}");

    // 别名冲突：甲唯一命中 Alpha；共同别名同时命中 Alpha 与 Gamma，必须判歧义。
    let alias = link(&index, "Alpha.md", "Gamma");
    assert_eq!(alias.display.as_deref(), Some("显示名"));
    assert_eq!(alias.target_path.as_deref(), Some("Gamma.md"));
}

#[test]
fn same_name_notes_are_ambiguous_and_never_pick_one() {
    let root = sample_vault("ambiguous");
    let index = build(&root);
    let dup = link(&index, "Alpha.md", "重复");
    assert_eq!(dup.status, AMBIGUOUS);
    assert_eq!(dup.target_path, None);
    assert_eq!(dup.candidates, vec!["sub/重复.md", "重复.md"]);
    assert_eq!(dup.anchor_status, None);
}

#[test]
fn alias_collision_is_ambiguous() {
    let root = sample_vault("alias-collision");
    let index = build(&root);
    let shared = link(&index, "Beta.md", "共同别名");
    assert_eq!(shared.status, AMBIGUOUS);
    assert_eq!(shared.target_path, None);
    assert_eq!(shared.candidates, vec!["Alpha.md", "Gamma.md"]);
}

#[test]
fn paths_extensions_and_case_fallbacks_are_ordered() {
    let root = sample_vault("paths");
    let index = build(&root);
    for raw in ["../Alpha.md", "Alpha.md", "ALPHA", "../Beta"] {
        let l = link(&index, "sub/Path.md", raw);
        assert_eq!(l.status, RESOLVED, "{raw} -> {l:#?}");
    }
    assert_eq!(
        link(&index, "sub/Path.md", "../Alpha.md").target_path.as_deref(),
        Some("Alpha.md")
    );
    assert_eq!(
        link(&index, "sub/Path.md", "ALPHA").target_path.as_deref(),
        Some("Alpha.md")
    );
}

#[test]
fn attachment_external_and_unresolved_are_distinct() {
    let root = sample_vault("kinds");
    let index = build(&root);
    let attachment = link(&index, "Alpha.md", "图.png");
    assert_eq!(attachment.status, ATTACHMENT);
    assert_eq!(attachment.attachment_path.as_deref(), Some("图.png"));
    assert_eq!(attachment.target_path, None);

    let external = link(&index, "Alpha.md", "https://example.com");
    assert_eq!(external.status, EXTERNAL);
    assert_eq!(external.target_path, None);

    let missing = link(&index, "Alpha.md", "缺失笔记");
    assert_eq!(missing.status, UNRESOLVED);
    assert_eq!(missing.target_path, None);
    assert!(missing.candidates.is_empty());

    // 附件不进入笔记节点。
    assert!(!index.notes.iter().any(|n| n.relative_path == "图.png"));
}

#[test]
fn orphans_match_incoming_links() {
    let root = sample_vault("orphans");
    let index = build(&root);
    let orphan_paths = orphans(&index);
    assert!(orphan_paths.contains(&"Orphan.md".to_string()), "{orphan_paths:#?}");
    assert!(!orphan_paths.contains(&"Beta.md".to_string()));
    let orphan = index.notes.iter().find(|n| n.relative_path == "Orphan.md").unwrap();
    assert!(orphan.orphan);
    assert_eq!(orphan.backlink_count, 0);
}

#[test]
fn headings_slug_duplicates_and_blocks_are_deterministic() {
    let body = "# 开始\n\n## 细节\n\n## 细节\n\n段落 ^blk-1\n\n```\n# 代码里的标题\n^fake\n```\n";
    let headings = heading_anchors(body, 5);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].slug, "开始");
    assert_eq!(headings[0].line, 5);
    assert_eq!(headings[1].slug, "细节");
    assert_eq!(headings[2].slug, "细节-1");
    let blocks = block_anchors(body, 5);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].id, "blk-1");
    assert_eq!(blocks[0].line, 11);

    let doc = parse_note(body);
    assert_eq!(resolve_anchor(&doc, "细节-1"), (LOCATED.to_string(), Some(5)));
    assert_eq!(resolve_anchor(&doc, "^blk-1"), (LOCATED.to_string(), Some(7)));
    assert_eq!(resolve_anchor(&doc, "不存在的标题"), (UNLOCATED.to_string(), None));
}

#[test]
fn heading_links_and_block_links_resolve_to_lines() {
    let root = sample_vault("anchors");
    let index = build(&root);
    let heading = link(&index, "Alpha.md", "Beta");
    let with_heading = index
        .links
        .iter()
        .find(|l| l.source_path == "Alpha.md" && l.target_raw == "Beta" && l.anchor.is_some())
        .unwrap();
    assert_eq!(heading.anchor, None);
    assert_eq!(with_heading.status, RESOLVED);
    assert_eq!(with_heading.anchor.as_deref(), Some("细节"));
    assert_eq!(with_heading.anchor_status.as_deref(), Some(LOCATED));
    assert_eq!(with_heading.anchor_line, Some(3));

    let block = link(&index, "Gamma.md", "Alpha");
    assert!(block.block_anchor);
    assert_eq!(block.anchor_status.as_deref(), Some(LOCATED));
    assert_eq!(block.anchor_line, Some(18));

    let local = link(&index, "Alpha.md", "");
    assert_eq!(local.target_path.as_deref(), Some("Alpha.md"));
    assert_eq!(local.anchor_status.as_deref(), Some(LOCATED));
    assert_eq!(local.anchor_line, Some(5));
}

#[test]
fn fingerprint_is_stable_and_generation_increments() {
    let root = sample_vault("fingerprint");
    let first = build_from_vault(&root, None, IndexBuildOptions::default(), 100).unwrap();
    let second = build_from_vault(
        &root,
        None,
        IndexBuildOptions {
            previous_generation: first.generation,
            ..Default::default()
        },
        200,
    )
    .unwrap();
    assert_eq!(first.fingerprint, second.fingerprint);
    assert_eq!(first.generation, 1);
    assert_eq!(second.generation, 2);
    assert_eq!(first.built_at, 100);
    assert_eq!(second.built_at, 200);
    assert!(first.complete);
}

#[test]
fn external_edits_are_visible_after_rebuild() {
    let root = sample_vault("refresh");
    let first = build(&root);
    assert_eq!(link(&first, "Alpha.md", "缺失笔记").status, UNRESOLVED);
    write(&root, "缺失笔记.md", "# 后来新建\n");
    let second = build(&root);
    assert_eq!(link(&second, "Alpha.md", "缺失笔记").status, RESOLVED);
    assert_ne!(first.fingerprint, second.fingerprint);
}

#[test]
fn changes_between_scan_and_read_make_snapshot_incomplete() {
    let root = sample_vault("changed");
    let scan = crate::knowledge_vault::scan_vault(&root, None).unwrap();
    // 扫描后改写：长度变化必须被识别，不能发布完整快照。
    write(&root, "Beta.md", "# Beta\n\n更长的新内容，导致长度不同。\n");
    let index = build_from_scan(&root, &scan, IndexBuildOptions::default(), 1);
    assert!(!index.complete, "{index:#?}");
    assert!(index
        .incomplete_reason
        .as_deref()
        .unwrap_or_default()
        .contains("Beta.md"));
    assert!(index.issues.iter().any(|i| i.relative_path == "Beta.md"));
}

#[test]
fn scan_errors_and_skips_are_visible() {
    use crate::knowledge_vault::{ScanStats, SkippedItem, VaultError, VaultScan};
    let root = temp_vault("scan-issues");
    write(&root, "A.md", "# A\n");
    let scan = VaultScan {
        root: root.display().to_string(),
        files: vec![VaultFile {
            relative_path: "A.md".to_string(),
            file_name: "A.md".to_string(),
            extension: "md".to_string(),
            bytes: 4,
            modified: None,
            kind: "note",
        }],
        skipped: vec![SkippedItem {
            relative_path: "secret".to_string(),
            reason: "hidden",
        }],
        errors: vec![VaultError {
            relative_path: "broken".to_string(),
            message: "无法读取目录".to_string(),
        }],
        stats: ScanStats {
            files: 1,
            notes: 1,
            ..Default::default()
        },
    };
    let index = build_from_scan(&root, &scan, IndexBuildOptions::default(), 1);
    assert!(index.issues.iter().any(|i| i.code == "scan_skipped_hidden"));
    assert!(index.issues.iter().any(|i| i.code == "scan_error"));
}

#[test]
fn too_long_targets_are_reported_not_silently_dropped() {
    let root = temp_vault("long-target");
    let long = "x".repeat(600);
    write(&root, "A.md", &format!("[[{long}]]\n[[正常]]\n"));
    let index = build(&root);
    assert!(index
        .issues
        .iter()
        .any(|i| i.code == "target_too_long" && i.relative_path == "A.md"));
}

#[test]
fn metadata_check_detects_same_length_rewrite_with_newer_mtime() {
    let root = temp_vault("mtime");
    write(&root, "A.md", "# A\n");
    let scan = crate::knowledge_vault::scan_vault(&root, None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    write(&root, "A.md", "# B\n");
    let index = build_from_scan(&root, &scan, IndexBuildOptions::default(), 1);
    assert!(!index.complete, "{index:#?}");
    assert!(index.stats.links == 0);
}

#[test]
fn budget_exhaustion_marks_incomplete_without_partial_notes() {
    let root = sample_vault("budget");
    let scan = crate::knowledge_vault::scan_vault(&root, None).unwrap();
    // 预算小于第一篇正文：第一篇都不能读入，更不能把截断内容当成完整笔记。
    let index = build_from_scan(
        &root,
        &scan,
        IndexBuildOptions {
            byte_budget: Some(8),
            ..Default::default()
        },
        1,
    );
    assert!(!index.complete, "{index:#?}");
    assert_eq!(index.stats.bytes_read, 0);
    assert!(index.stats.notes < index.stats.notes_scanned, "{index:#?}");
    assert_eq!(index.stats.links, 0);
    assert!(index
        .incomplete_reason
        .as_deref()
        .unwrap_or_default()
        .contains("预算"));
    assert!(index
        .issues
        .iter()
        .any(|i| i.code == "budget_exhausted"));
    assert!(index.stats.bytes_read <= index.stats.budget_bytes);
}

#[test]
fn rebuild_after_add_modify_delete_matches_from_scratch() {
    let root = sample_vault("consistency");
    let first = build(&root);

    // 外部变化：新增一篇、改写一篇、删除一篇。
    write(&root, "Delta.md", "# Delta\n\n[[Alpha]] 与 [[缺失笔记]]\n");
    write(&root, "Beta.md", "# Beta\n\n改过的正文 [[Delta]]\n");
    fs::remove_file(root.join("Orphan.md")).unwrap();

    let incremental = build_from_vault(
        &root,
        None,
        IndexBuildOptions {
            previous_generation: first.generation,
            ..Default::default()
        },
        1_700_000_100,
    )
    .unwrap();
    let scratch = build_from_vault(&root, None, IndexBuildOptions::default(), 1_700_000_100)
        .unwrap();

    assert!(incremental.complete, "{incremental:#?}");
    assert_eq!(incremental.generation, first.generation + 1);
    assert_eq!(incremental.fingerprint, scratch.fingerprint);
    assert_eq!(incremental.notes, scratch.notes);
    assert_eq!(incremental.links, scratch.links);
    assert_eq!(incremental.stats, scratch.stats);
    assert_ne!(first.fingerprint, incremental.fingerprint);

    // 删除 Orphan 后不再有它；新增 Delta 的入链与出链都进入关系表。
    assert!(!incremental.notes.iter().any(|n| n.relative_path == "Orphan.md"));
    let delta = link(&incremental, "Delta.md", "Alpha");
    assert_eq!(delta.status, RESOLVED);
    let delta_links = note_links(&incremental, "Delta.md").unwrap();
    assert_eq!(delta_links.note.backlink_count, 1);
    assert!(delta_links
        .outgoing
        .iter()
        .any(|l| l.target_raw == "缺失笔记" && l.status == UNRESOLVED));
}

#[test]
fn unsupported_syntax_is_reported_and_original_text_stays() {
    let root = temp_vault("unsupported");
    write(
        &root,
        "A.md",
        "# A\n\n[文字][标签] 与 [^1] 与 <a href=\"x.md\">x</a> 与 [[未闭合\n\n正常 [[B]]\n",
    );
    write(&root, "B.md", "# B\n");
    let index = build(&root);

    let codes: Vec<&str> = index.issues.iter().map(|i| i.code.as_str()).collect();
    assert!(codes.contains(&"unsupported_reference_link"), "{codes:?}");
    assert!(codes.contains(&"unsupported_footnote"), "{codes:?}");
    assert!(codes.contains(&"unsupported_html_link"), "{codes:?}");
    assert!(codes.contains(&"unclosed_wikilink"), "{codes:?}");
    // 不支持的写法不进关系表，也不阻断同文件里的正常链接。
    assert_eq!(index.links.len(), 1, "{:#?}", index.links);
    assert_eq!(index.links[0].target_raw, "B");
    assert_eq!(index.links[0].status, RESOLVED);
    assert!(index.complete, "不支持语法是局部问题，不应让整个快照失效");
    assert_eq!(
        fs::read_to_string(root.join("A.md")).unwrap(),
        "# A\n\n[文字][标签] 与 [^1] 与 <a href=\"x.md\">x</a> 与 [[未闭合\n\n正常 [[B]]\n"
    );
}
