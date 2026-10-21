//! OD-ARCH-045 第 1 步验收：线程记录落在数据根，重建存储（模拟重启）后仍能读回。
//!
//! 这个二进制里只有一个测试，因此对 OMNIDESK_APP_DATA_ROOT 的设置在进程内是安全的；
//! 它不触碰 ~/.dsh、~/.agents，也不写仓库。

use std::path::{Path, PathBuf};

use omnidesk_desktop_lib::app_paths::AppPaths;
use omnidesk_desktop_lib::thread_store::ThreadStore;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn thread_survives_store_rebuild_and_never_lands_in_repo() {
    let root = std::env::temp_dir().join(format!(
        "omnidesk-thread-restart-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::env::set_var("OMNIDESK_APP_DATA_ROOT", &root);

    let paths = AppPaths::resolve().expect("数据根应被接受（仓库外绝对路径）");
    paths.ensure().expect("创建数据根");
    assert!(paths.threads.starts_with(&root));

    let store = ThreadStore::resolve(&paths).expect("线程存储应可用");
    store
        .save(&serde_json::json!({
            "id": "trestart",
            "tag": "weekly",
            "projectPath": "/tmp/synthetic-project",
            "title": "合成对话",
            "preset": "weekly",
            "messages": [{"id": "m1", "role": "user", "text": "合成输入"}],
            "createdAt": 1.0,
            "updatedAt": 2.0,
        }))
        .expect("保存线程");
    drop(store);

    // 模拟重启：重新解析数据根、重建存储，只从磁盘读回。
    let paths = AppPaths::resolve().expect("重启后重新解析数据根");
    let reopened = ThreadStore::resolve(&paths).expect("重建线程存储");
    let listing = reopened.load().expect("读回线程");
    assert_eq!(listing.threads.len(), 1, "重启后应从数据根恢复 1 条线程");
    assert_eq!(listing.threads[0]["id"], serde_json::json!("trestart"));
    assert_eq!(listing.skipped, 0);
    assert!(
        listing.threads[0]["messages"][0]["text"] == serde_json::json!("合成输入"),
        "正文应完整保留"
    );

    // 数据根之外（仓库内）不得出现线程记录。
    assert!(!root.starts_with(repo_root()), "数据根必须在仓库外");
    let leaked = std::fs::read_dir(repo_root())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().contains("trestart"));
    assert!(!leaked, "仓库根不应出现线程记录文件");

    std::env::remove_var("OMNIDESK_APP_DATA_ROOT");
    let _ = std::fs::remove_dir_all(&root);
}
