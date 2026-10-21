//! 工作台线程/会话记录的仓库外存储（OD-ARCH-045 第 1 步）。
//!
//! 背景：此前线程记录只放在 webview 的 localStorage 里（清理缓存/换机器即丢）。
//! 现在记录落到 OmniDesk 应用数据根：<root>/threads/<项目分目录>/<thread-id>.json。
//! localStorage 降级为缓存，仅用于：
//!   * 首次迁移（数据根为空时，把旧缓存一次性写进数据根，**不删除**旧缓存）；
//!   * 宿主不可用（纯浏览器开发）时回退。
//!
//! 边界（与 app_paths / task_shell 一致）：
//!   * 目录必须通过隔离校验，且真实落点不得在仓库内；
//!   * 线程 id 只允许安全字符，不使用任何用户输入拼路径的其它部分；
//!   * 不跟随符号链接；非普通文件一律跳过，并计入 skipped；
//!   * 单文件有大小上限；写入走同目录临时文件 + rename（原子替换）；
//!   * 读出的 JSON 结构不合规就跳过并报告条数，不猜测、不修补。

use std::path::{Path, PathBuf};

use crate::app_paths::{resolve_real_path, AppPaths};
use crate::sha256::sha256_hex;
use crate::task_shell::{assert_local_root_allowed, assert_outside_repo};

/// 单个线程文件的上限（对话正文可能较长，但异常大小说明文件不属于本流程）。
pub const THREAD_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// 线程 id 上限（与前端生成规则一致，留足余量）。
pub const THREAD_ID_MAX_LEN: usize = 64;

/// 未指定项目时的分目录名。
const UNASSIGNED_DIR: &str = "_unassigned";

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ThreadListing {
    /// 实际使用的线程目录（绝对路径；供界面显示与排障）。
    pub dir: String,
    /// 读回的线程（原始 JSON 对象，前端按自己的类型解释；宿主不做业务解析）。
    pub threads: Vec<serde_json::Value>,
    /// 被跳过的文件数（非法名字、非普通文件、结构不合规、超限）。
    pub skipped: usize,
    /// 人类可读的一句话说明。
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ThreadSaveReceipt {
    pub id: String,
    pub path: String,
    pub bytes: u64,
}

pub struct ThreadStore {
    root: PathBuf,
}

impl ThreadStore {
    /// 由应用数据根构造，并校验边界、按需创建线程目录。
    pub fn resolve(paths: &AppPaths) -> Result<Self, String> {
        let store = Self::for_root(paths.threads.clone());
        store.assert_allowed()?;
        std::fs::create_dir_all(&store.root)
            .map_err(|e| format!("创建线程目录失败（{}）：{}", store.root.display(), e))?;
        // 目录建立后再校验一次真实落点，防止软链接把写入引到仓库或禁区。
        store.assert_allowed()?;
        Ok(store)
    }

    /// 直接指定线程根（测试用；不做创建）。
    pub fn for_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn assert_allowed(&self) -> Result<(), String> {
        assert_local_root_allowed(&self.root)?;
        assert_outside_repo(&self.root)?;
        let resolved = resolve_real_path(&self.root)?;
        assert_local_root_allowed(&resolved)?;
        assert_outside_repo(&resolved)?;
        Ok(())
    }

    /// 读回全部线程。目录不存在时返回空列表（不是错误）。
    pub fn load(&self) -> Result<ThreadListing, String> {
        self.assert_allowed()?;
        let mut threads = Vec::new();
        let mut skipped = 0usize;
        if !self.root.exists() {
            return Ok(ThreadListing {
                dir: self.root.display().to_string(),
                threads,
                skipped,
                note: "线程目录尚不存在（首次保存时创建）".to_string(),
            });
        }
        let dirs = std::fs::read_dir(&self.root)
            .map_err(|e| format!("读取线程目录失败（{}）：{}", self.root.display(), e))?;
        for dir_item in dirs {
            let dir_item = match dir_item {
                Ok(item) => item,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            // 项目分目录：只接受普通目录，不跟随符号链接。
            let meta = match std::fs::symlink_metadata(dir_item.path()) {
                Ok(m) => m,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if !meta.file_type().is_dir()
                || !project_dir_name_allowed(&dir_item.file_name().to_string_lossy())
            {
                skipped += 1;
                continue;
            }
            let files = match std::fs::read_dir(dir_item.path()) {
                Ok(f) => f,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            for file_item in files {
                match self.read_one(&file_item) {
                    Some(value) => threads.push(value),
                    None => skipped += 1,
                }
            }
        }
        threads.sort_by(|a, b| {
            let ua = a.get("updatedAt").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let ub = b.get("updatedAt").and_then(|v| v.as_f64()).unwrap_or(0.0);
            ub.partial_cmp(&ua).unwrap_or(std::cmp::Ordering::Equal)
        });
        let note = if skipped > 0 {
            format!("{} 条线程已读回；跳过 {} 个不合规文件", threads.len(), skipped)
        } else {
            format!("{} 条线程已从数据根读回", threads.len())
        };
        Ok(ThreadListing {
            dir: self.root.display().to_string(),
            threads,
            skipped,
            note,
        })
    }

    fn read_one(
        &self,
        item: &Result<std::fs::DirEntry, std::io::Error>,
    ) -> Option<serde_json::Value> {
        let item = item.as_ref().ok()?;
        let name = item.file_name().to_string_lossy().to_string();
        let id = name.strip_suffix(".json")?;
        if !thread_id_allowed(id) {
            return None;
        }
        let meta = std::fs::symlink_metadata(item.path()).ok()?;
        if !meta.file_type().is_file() || meta.len() > THREAD_MAX_BYTES {
            return None;
        }
        let text = std::fs::read_to_string(item.path()).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        if !thread_value_allowed(&value, id) {
            return None;
        }
        Some(value)
    }

    /// 写入（覆盖）一条线程。若所属项目分目录变化，会删除旧文件并写新文件。
    pub fn save(&self, thread: &serde_json::Value) -> Result<ThreadSaveReceipt, String> {
        self.assert_allowed()?;
        let id = thread
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "线程缺少数值字符串 id，拒绝保存。".to_string())?;
        if !thread_id_allowed(id) {
            return Err(format!("线程 id 不合规，拒绝保存：{}", id));
        }
        if !thread_value_allowed(thread, id) {
            return Err(
                "线程结构不完整（需要 tag/title/messages/createdAt/updatedAt），拒绝保存。"
                    .to_string(),
            );
        }
        let encoded =
            serde_json::to_vec_pretty(thread).map_err(|e| format!("序列化线程失败：{}", e))?;
        if encoded.len() as u64 > THREAD_MAX_BYTES {
            return Err(format!(
                "线程记录 {} 字节，超过上限 {} 字节，拒绝保存。",
                encoded.len(),
                THREAD_MAX_BYTES
            ));
        }
        let dir_name = project_dir_for(thread);
        let target_dir = self.root.join(&dir_name);
        self.assert_allowed()?;
        std::fs::create_dir_all(&target_dir)
            .map_err(|e| format!("创建项目分目录失败（{}）：{}", target_dir.display(), e))?;
        if !std::fs::symlink_metadata(&target_dir)
            .map(|m| m.file_type().is_dir())
            .unwrap_or(false)
        {
            return Err(format!(
                "项目分目录不是普通目录，拒绝写入：{}",
                target_dir.display()
            ));
        }
        let target = target_dir.join(format!("{}.json", id));
        write_atomic(&target, &encoded)?;
        // 项目改挂后旧分目录里的同名文件不应残留（只删除本存储自己命名的文件）。
        self.remove_other_copies(id, &target);
        Ok(ThreadSaveReceipt {
            id: id.to_string(),
            path: target.display().to_string(),
            bytes: encoded.len() as u64,
        })
    }

    /// 删除一条线程（仅删除本存储命名规则内的文件）。返回删除的文件数。
    pub fn delete(&self, id: &str) -> Result<usize, String> {
        if !thread_id_allowed(id) {
            return Err(format!("线程 id 不合规，拒绝删除：{}", id));
        }
        self.assert_allowed()?;
        let mut removed = 0usize;
        for dir in self.project_dirs()? {
            let target = dir.join(format!("{}.json", id));
            match std::fs::symlink_metadata(&target) {
                Ok(meta) if meta.file_type().is_file() => {
                    std::fs::remove_file(&target)
                        .map_err(|e| format!("删除线程失败（{}）：{}", target.display(), e))?;
                    removed += 1;
                }
                _ => {}
            }
        }
        Ok(removed)
    }

    fn project_dirs(&self) -> Result<Vec<PathBuf>, String> {
        let mut out = Vec::new();
        if !self.root.exists() {
            return Ok(out);
        }
        let dirs = std::fs::read_dir(&self.root)
            .map_err(|e| format!("读取线程目录失败（{}）：{}", self.root.display(), e))?;
        for item in dirs {
            let item = match item {
                Ok(i) => i,
                Err(_) => continue,
            };
            let meta = match std::fs::symlink_metadata(item.path()) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.file_type().is_dir()
                && project_dir_name_allowed(&item.file_name().to_string_lossy())
            {
                out.push(item.path());
            }
        }
        Ok(out)
    }

    fn remove_other_copies(&self, id: &str, keep: &Path) {
        if let Ok(dirs) = self.project_dirs() {
            for dir in dirs {
                let candidate = dir.join(format!("{}.json", id));
                if candidate == *keep {
                    continue;
                }
                if let Ok(meta) = std::fs::symlink_metadata(&candidate) {
                    if meta.file_type().is_file() {
                        let _ = std::fs::remove_file(&candidate);
                    }
                }
            }
        }
    }
}

/// 线程 id：字母数字、减号、下划线，不以点开头，长度受限。
pub fn thread_id_allowed(id: &str) -> bool {
    if id.is_empty() || id.len() > THREAD_ID_MAX_LEN || id.starts_with('.') {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 项目分目录名：小写字母数字、减号、下划线；由保存逻辑自己生成，不接受任意输入。
fn project_dir_name_allowed(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 线程的最小结构校验：前端能安全解释的字段必须存在且类型正确。
fn thread_value_allowed(value: &serde_json::Value, id: &str) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    if obj.get("id").and_then(|v| v.as_str()) != Some(id) {
        return false;
    }
    if obj.get("tag").and_then(|v| v.as_str()).is_none() {
        return false;
    }
    if obj.get("title").and_then(|v| v.as_str()).is_none() {
        return false;
    }
    if obj.get("preset").and_then(|v| v.as_str()).is_none() {
        return false;
    }
    if obj.get("messages").and_then(|v| v.as_array()).is_none() {
        return false;
    }
    if obj.get("createdAt").and_then(|v| v.as_f64()).is_none() {
        return false;
    }
    if obj.get("updatedAt").and_then(|v| v.as_f64()).is_none() {
        return false;
    }
    if let Some(project) = obj.get("projectPath") {
        if !project.is_null() && project.as_str().is_none() {
            return false;
        }
    }
    true
}

/// 由线程的 projectPath 派生分目录名：<可读前缀>-<路径哈希前 8 位>。
///
/// 前缀只是为了人眼可读；哈希负责区分同名项目，避免两条路径落到同一目录。
fn project_dir_for(thread: &serde_json::Value) -> String {
    let raw = thread
        .get("projectPath")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let Some(path) = raw else {
        return UNASSIGNED_DIR.to_string();
    };
    let base = Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "project".to_string());
    let mut prefix: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while prefix.contains("--") {
        prefix = prefix.replace("--", "-");
    }
    let prefix = prefix.trim_matches('-').to_string();
    let prefix: String = if prefix.is_empty() {
        "project".to_string()
    } else {
        prefix.chars().take(40).collect()
    };
    let hash = sha256_hex(path.as_bytes());
    format!("{}-{}", prefix, &hash[..8])
}

/// 同目录临时文件 + rename，避免半写状态；失败时清理临时文件。
fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = target
        .parent()
        .ok_or_else(|| format!("目标路径没有父目录：{}", target.display()))?;
    let stamp = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let temp = dir.join(format!(".thread-{}.tmp", stamp));
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temp)
            .map_err(|e| format!("写入线程临时文件失败（{}）：{}", temp.display(), e))?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                format!("写入线程临时文件失败（{}）：{}", temp.display(), e)
            })?;
    }
    std::fs::rename(&temp, target).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("替换线程文件失败（{}）：{}", target.display(), e)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_store(tag: &str) -> (ThreadStore, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-thread-store-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&base);
        (ThreadStore::for_root(base.join("threads")), base)
    }

    fn thread(id: &str, project: Option<&str>, updated: f64) -> serde_json::Value {
        json!({
            "id": id,
            "tag": "weekly",
            "projectPath": project,
            "title": "合成线程",
            "preset": "weekly",
            "messages": [{"id": "m1", "role": "user", "text": "合成输入"}],
            "createdAt": 1.0,
            "updatedAt": updated,
        })
    }

    #[test]
    fn saves_per_project_dir_and_loads_back() {
        let (store, base) = temp_store("roundtrip");
        let receipt = store
            .save(&thread("tabc1", Some("/tmp/synthetic project"), 5.0))
            .expect("save");
        assert!(receipt.path.contains("/threads/"));
        assert!(!receipt.path.contains(' '));
        let listing = store.load().expect("load");
        assert_eq!(listing.threads.len(), 1);
        assert_eq!(listing.threads[0]["id"], json!("tabc1"));
        assert_eq!(listing.skipped, 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn unassigned_threads_go_to_unassigned_dir() {
        let (store, base) = temp_store("unassigned");
        let receipt = store.save(&thread("tplain", None, 1.0)).expect("save");
        assert!(receipt.path.ends_with("_unassigned/tplain.json"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn moving_project_relocates_without_duplicates() {
        let (store, base) = temp_store("move");
        store
            .save(&thread("tmove", Some("/tmp/a"), 1.0))
            .expect("save a");
        store
            .save(&thread("tmove", Some("/tmp/b"), 2.0))
            .expect("save b");
        let listing = store.load().expect("load");
        assert_eq!(listing.threads.len(), 1);
        assert_eq!(listing.threads[0]["updatedAt"], json!(2.0));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_unsafe_ids_and_bad_shapes() {
        let (store, base) = temp_store("unsafe");
        let long = "x".repeat(80);
        for id in ["../escape", "a/b", ".hidden", "", long.as_str()] {
            assert!(
                store.save(&thread(id, None, 1.0)).is_err(),
                "应拒绝 {}",
                id
            );
            assert!(store.delete(id).is_err(), "应拒绝删除 {}", id);
        }
        let mut bad = thread("tok", None, 1.0);
        bad.as_object_mut().unwrap().remove("messages");
        assert!(store.save(&bad).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn skips_symlinks_and_invalid_files() {
        let (store, base) = temp_store("skip");
        store.save(&thread("tgood", None, 1.0)).expect("save");
        let dir = base.join("threads").join(UNASSIGNED_DIR);
        std::fs::write(dir.join("bad name.json"), "{}").unwrap();
        std::fs::write(dir.join("tbroken.json"), "{not json").unwrap();
        std::fs::write(dir.join("tbadshape.json"), r#"{"id":"tbadshape"}"#).unwrap();
        #[cfg(unix)]
        {
            let outside = base.join("outside.json");
            std::fs::write(
                &outside,
                serde_json::to_vec(&thread("tlink", None, 9.0)).unwrap(),
            )
            .unwrap();
            std::os::unix::fs::symlink(&outside, dir.join("tlink.json")).unwrap();
        }
        let listing = store.load().expect("load");
        assert_eq!(listing.threads.len(), 1);
        assert_eq!(listing.threads[0]["id"], json!("tgood"));
        assert!(listing.skipped >= 3, "skipped={}", listing.skipped);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn delete_removes_file_and_is_idempotent() {
        let (store, base) = temp_store("delete");
        store.save(&thread("tdel", None, 1.0)).expect("save");
        assert_eq!(store.delete("tdel").expect("delete"), 1);
        assert_eq!(store.delete("tdel").expect("delete again"), 0);
        assert!(store.load().expect("load").threads.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn no_temp_files_left_behind() {
        let (store, base) = temp_store("atomic");
        store.save(&thread("tatomic", None, 1.0)).expect("save");
        let dir = base.join("threads").join(UNASSIGNED_DIR);
        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["tatomic.json".to_string()]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn load_orders_by_updated_at_desc() {
        let (store, base) = temp_store("order");
        store.save(&thread("told", None, 1.0)).expect("old");
        store.save(&thread("tnew", None, 3.0)).expect("new");
        let listing = store.load().expect("load");
        assert_eq!(listing.threads[0]["id"], json!("tnew"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
