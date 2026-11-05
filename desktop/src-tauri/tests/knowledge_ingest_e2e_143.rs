//! OD-ARCH-143 S2：知识库整理 + 主链脱敏 + 检索命中的合成端到端。
//!
//! 只使用 /tmp 合成数据；不启动真实模型、不触碰用户 Vault。

use std::fs;
use std::path::{Path, PathBuf};

use omnidesk_desktop_lib::knowledge_ingest::{
    apply_payload, init_vault, prepare_inbox, smoke_search, INDEX_REL,
};
use omnidesk_desktop_lib::knowledge_vault;
use serde_json::json;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn python() -> String {
    std::env::var("OMNIDESK_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

#[test]
fn synthetic_inbox_masks_then_creates_card_and_searches() {
    let base = std::env::temp_dir().join(format!(
        "omnidesk-ingest-e2e-143-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let root = base.join("vault");
    let dsh_home = base.join("dsh-home");
    let skills = dsh_home.join("skills");
    fs::create_dir_all(root.join("inbox")).unwrap();
    fs::create_dir_all(skills.join("knowledge-agent")).unwrap();
    fs::create_dir_all(skills.join("knowledge-search")).unwrap();
    fs::write(
        skills.join("knowledge-agent/SKILL.md"),
        "# synthetic knowledge-agent\n卡片正文不超过 300 字，带 sources。\n",
    )
    .unwrap();
    fs::write(
        skills.join("knowledge-search/SKILL.md"),
        "# synthetic knowledge-search\n索引一行一卡，双链引用。\n",
    )
    .unwrap();
    let dict = base.join("sensitive-dict.v1.json");
    fs::write(
        &dict,
        r#"{"schema":"omnidesk.sensitive-dict.v1","entries":[{"type":"PERSON","value":"林青禾"}]}"#,
    )
    .unwrap();

    init_vault(&root).unwrap();
    fs::write(
        root.join("inbox/合成制度.md"),
        "林青禾负责专项检查，每季度不少于一次。",
    )
    .unwrap();

    let prepared = prepare_inbox(
        &root,
        &dsh_home,
        &python(),
        &repo_root().join("engine"),
        Some(&dict),
    )
    .expect("整理提示词应组装成功");
    assert!(!prepared.prompt.contains("林青禾"), "人名不得明文出站");
    let start = prepared.prompt.find("[PERSON_").expect("应有 PERSON 占位符");
    let rest = &prepared.prompt[start..];
    let end = rest.find(']').expect("占位符应闭合") + 1;
    let placeholder = &rest[..end];

    let index_line = "- [专项检查频次](概念/专项检查频次.md) | 标签：监督 | 每季度一次";
    let payload = json!({
        "cards": [{
            "relative_path": "知识库/概念/专项检查频次.md",
            "title": "专项检查频次",
            "content": format!("---\ntags: [监督, 专项检查]\nsources: [inbox/合成制度.md]\ndate: 2026-11-05\n---\n专项检查由 {} 负责，每季度不少于一次。", placeholder),
            "sources": ["inbox/合成制度.md"]
        }],
        "index_lines": [index_line],
        "archive_moves": [{"from": "inbox/合成制度.md", "to": "知识库/原文/合成制度.md"}]
    });
    let scan = knowledge_vault::scan_vault(&root, None).unwrap();
    let report = apply_payload(&root, &payload, &scan);
    assert!(report.ok, "{:?} {:?}", report.errors, report.rejected_cards);
    assert!(root.join("知识库/概念/专项检查频次.md").is_file());
    assert!(root.join("知识库/原文/合成制度.md").is_file());
    let index = fs::read_to_string(root.join(INDEX_REL)).unwrap();
    assert!(index.contains("专项检查频次"));
    assert!(smoke_search(&root, "专项检查").unwrap() >= 1);
    // 模型链路看到的是占位符；本地归档原文仍保留用户材料。
    let archived = fs::read_to_string(root.join("知识库/原文/合成制度.md")).unwrap();
    assert!(archived.contains("林青禾"));

    let _ = fs::remove_dir_all(base);
}

