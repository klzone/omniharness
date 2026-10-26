//! 知识库单篇笔记的创建与保存（OD-KB-001 P2-S02）。
//!
//! 这是知识库唯一的写入通道，边界一律由宿主强制，界面传什么都不能放宽：
//!   * 目标必须是根内相对路径、Markdown 扩展名、非隐藏/非排除目录；
//!   * 逐层拒绝符号链接、`..`、绝对路径与根外落点（复用 P1 `resolve_vault_file`）；
//!   * 正文大小不超过 P1 的单文件上限，且必须是合法 UTF-8 文本；
//!   * `create` 绝不覆盖已存在的目标；`save` 必须带打开时读到的原文哈希，
//!     保存前重新读取并比对，不一致就返回冲突，**不写入**；
//!   * 写入走同目录临时文件 → fsync → rename，失败清理临时文件，原文件保持不变。
//!
//! 已知边界（不宣称已消除）：普通文件 API 没有"比较并交换"语义，重新读取与最终
//! rename 之间仍可能被别的进程改写。这个窗口在 `save` 的返回值里如实标注为
//! `race_window`，不把它说成完全消除的跨进程竞态。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::app_paths::resolve_real_path;
use crate::knowledge_vault::{
    resolve_vault_file, VaultNote, VAULT_MAX_NOTE_BYTES, VAULT_NOTE_EXTS,
};
use crate::sha256::sha256_hex;

/// 冲突错误固定前缀。界面据此区分「文件被外部改动」与普通失败，绝不能在
/// 冲突时提示保存成功。
pub const CONFLICT_PREFIX: &str = "[conflict]";

/// 保存成功后的结果。不回传正文之外的任何派生数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SaveOutcome {
    pub relative_path: String,
    pub bytes: u64,
    pub sha256: String,
    /// 保存后重新读取得到的正文。界面直接以它为准，不用本地草稿冒充落盘内容。
    pub content: String,
    /// 固定说明：重新读取→rename 之间仍存在跨进程窗口。
    pub race_window: &'static str,
}

/// 冲突错误里带的现场信息，附加在固定前缀之后的可读文本里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictDetail {
    pub relative_path: String,
    /// 打开时界面持有的版本。
    pub expected_sha256: String,
    /// 保存前重新读取到的当前版本。
    pub actual_sha256: String,
}

const RACE_WINDOW: &str = "已重新读取校验后写入；读取与替换之间的跨进程改写窗口仍存在，未被消除";

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// 校验写入目标的相对路径形态（不触碰文件系统）。
fn check_target_shape(relative: &str) -> Result<PathBuf, String> {
    let raw = relative.trim();
    if raw.is_empty() {
        return Err("请提供知识库内的 Markdown 相对路径。".to_string());
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(format!("只接受知识库内的相对路径：{raw}"));
    }
    for component in path.components() {
        match component {
            std::path::Component::Normal(name) => {
                let name = name.to_string_lossy();
                if name.starts_with('.') {
                    return Err(format!("目标不能落在隐藏文件或目录内：{raw}"));
                }
                if crate::knowledge_vault::is_excluded_dir_name(&name) {
                    return Err(format!("目标不能落在被排除的目录内：{raw}"));
                }
            }
            _ => return Err(format!("路径包含非法片段：{raw}")),
        }
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !VAULT_NOTE_EXTS.contains(&extension.as_str()) {
        return Err(format!(
            "只能新建或保存 {} 文件：{raw}",
            VAULT_NOTE_EXTS.join(" / ")
        ));
    }
    Ok(path.to_path_buf())
}

/// 校验正文：必须是合法 UTF-8（调用方已保证，因为入参是 String）且不超过上限。
fn check_content_size(content: &str) -> Result<(), String> {
    let bytes = content.len() as u64;
    if bytes > VAULT_MAX_NOTE_BYTES {
        return Err(format!(
            "正文 {bytes} 字节，超过单文件上限 {VAULT_MAX_NOTE_BYTES} 字节；请拆分后再保存。"
        ));
    }
    Ok(())
}

/// 逐层确认目标路径上的每一级目录都存在、是普通目录且不是符号链接。
/// 新建时父目录必须已存在：本阶段不隐式创建目录。
fn assert_parent_chain(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let root_real = resolve_real_path(root)?;
    let mut current = root_real.clone();
    let components: Vec<_> = relative.components().collect();
    let dirs = &components[..components.len().saturating_sub(1)];
    for component in dirs {
        let name = match component {
            std::path::Component::Normal(name) => name,
            _ => return Err("路径包含非法片段".to_string()),
        };
        current.push(name);
        let meta = fs::symlink_metadata(&current)
            .map_err(|e| format!("上级目录不存在：{}（{}）", current.display(), e))?;
        if meta.file_type().is_symlink() {
            return Err(format!("拒绝写入符号链接目录：{}", current.display()));
        }
        if !meta.is_dir() {
            return Err(format!("上级路径不是目录：{}", current.display()));
        }
    }
    Ok(current)
}

fn temp_path(dir: &Path, name: &str) -> PathBuf {
    let stamp = format!(
        "{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    );
    dir.join(format!(".{name}.{stamp}.tmp"))
}

/// 创建同目录临时文件并写入、fsync。返回临时文件路径；调用方负责发布或清理。
fn write_temp(dir: &Path, name: &str, bytes: &[u8], mode_from: Option<&Path>) -> Result<PathBuf, String> {
    let temp = temp_path(dir, name);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| format!("无法创建临时文件（{}）：{}", temp.display(), e))?;
    // 覆盖已有文件时沿用原权限，避免保存把 0600 变成 0644。
    if let Some(target) = mode_from {
        if let Ok(meta) = fs::metadata(target) {
            file.set_permissions(meta.permissions()).map_err(|e| {
                let _ = fs::remove_file(&temp);
                format!("无法沿用目标文件权限（{}）：{}", target.display(), e)
            })?;
        }
    }
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(format!("写入临时文件失败（{}）：{}", temp.display(), e));
    }
    Ok(temp)
}

/// 覆盖写入：同目录临时文件 + fsync + rename；失败清理临时文件。
fn write_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = target
        .parent()
        .ok_or_else(|| format!("目标路径没有父目录：{}", target.display()))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "note".to_string());
    let temp = write_temp(dir, &name, bytes, Some(target))?;
    let result = (|| -> Result<(), String> {
        fs::rename(&temp, target)
            .map_err(|e| format!("替换目标文件失败（{}）：{}", target.display(), e))?;
        // 目录 fsync 让 rename 在崩溃后也有机会保留；失败不谎报写入失败，
        // 因为文件本身已经替换成功。
        let _ = fs::File::open(dir).and_then(|dir| dir.sync_all());
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 新建写入：同目录临时文件写完后用硬链接发布。`hard_link` 在目标已存在时
/// 原子失败，因此不会像 rename 那样覆盖另一进程刚创建的文件。
fn write_new_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = target
        .parent()
        .ok_or_else(|| format!("目标路径没有父目录：{}", target.display()))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "note".to_string());
    let temp = write_temp(dir, &name, bytes, None)?;
    let result = match fs::hard_link(&temp, target) {
        Ok(()) => {
            let _ = fs::File::open(dir).and_then(|dir| dir.sync_all());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(format!("目标已存在，新建不会覆盖：{}", target.display()))
        }
        Err(e) => Err(format!(
            "发布新文件失败（{}）：{}；目标未被改动",
            target.display(),
            e
        )),
    };
    let _ = fs::remove_file(&temp);
    result
}

/// 新建一篇笔记。目标已存在时拒绝，绝不覆盖。
pub fn create_note(root: &Path, relative: &str, content: &str) -> Result<VaultNote, String> {
    check_target_shape(relative)?;
    check_content_size(content)?;
    let root = crate::knowledge_vault::assert_vault_root_allowed(root, None)?;
    let parent = assert_parent_chain(&root, Path::new(relative))?;
    let target = parent.join(Path::new(relative).file_name().unwrap());
    match fs::symlink_metadata(&target) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(format!("目标位置是符号链接，新建不会写入：{relative}"));
            }
            return Err(format!("目标已存在，新建不会覆盖：{relative}"));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(format!("无法确认目标是否存在（{}）：{}", relative, e));
        }
    }
    let resolved = resolve_real_path(&target).unwrap_or_else(|_| target.clone());
    if !resolved.starts_with(&root) {
        return Err(format!("拒绝写入知识库根之外：{relative}"));
    }
    write_new_atomically(&target, content.as_bytes())?;
    // 以重新读取的结果为准，避免把内存里的草稿当成已落盘事实。
    crate::knowledge_vault::read_vault_note(&root, relative)
}

/// 保存一篇已有笔记。`expected_sha256` 必须是界面打开时读到的原文哈希。
pub fn save_note(
    root: &Path,
    relative: &str,
    content: &str,
    expected_sha256: &str,
) -> Result<SaveOutcome, String> {
    check_target_shape(relative)?;
    check_content_size(content)?;
    let expected = expected_sha256.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("保存请求缺少有效的原文版本标识（sha256）。".to_string());
    }
    let root = crate::knowledge_vault::assert_vault_root_allowed(root, None)?;
    let target = resolve_vault_file(&root, relative)?;
    // 冲突检查：重新读盘再比对，不信任界面传来的旧内容。
    let current = crate::knowledge_vault::read_vault_note(&root, relative)?;
    if current.sha256 != expected {
        let detail = ConflictDetail {
            relative_path: current.relative_path.clone(),
            expected_sha256: expected,
            actual_sha256: current.sha256.clone(),
        };
        return Err(format!(
            "{CONFLICT_PREFIX} 文件已在编辑期间被改动，保存已取消（未写入）。路径：{}；当前版本：{}；打开时版本：{}",
            detail.relative_path, detail.actual_sha256, detail.expected_sha256
        ));
    }
    write_atomically(&target, content.as_bytes())?;
    let after = crate::knowledge_vault::read_vault_note(&root, relative)?;
    if after.content != content {
        return Err("写入后重新读取的内容与请求不一致；请检查该文件是否被其他程序改写。".to_string());
    }
    let bytes = after.bytes;
    Ok(SaveOutcome {
        relative_path: after.relative_path,
        bytes,
        sha256: after.sha256,
        content: after.content,
        race_window: RACE_WINDOW,
    })
}

/// 计算一段文本的哈希（界面用它确认草稿版本，不写盘）。
pub fn content_sha256(content: &str) -> String {
    sha256_hex(content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture {
        base: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicUsize = AtomicUsize::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-vault-atomic-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&base);
            fs::create_dir_all(&base).unwrap();
            Self { base }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn temp_leftovers(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect()
    }

    #[test]
    fn new_write_never_replaces_an_existing_target() {
        let fx = Fixture::new("exists");
        let target = fx.base.join("note.md");
        fs::write(&target, b"first").unwrap();
        let err = write_new_atomically(&target, b"second").unwrap_err();
        assert!(err.contains("已存在"), "{err}");
        assert_eq!(fs::read(&target).unwrap(), b"first");
        assert!(temp_leftovers(&fx.base).is_empty());
    }

    #[test]
    fn new_write_publishes_content_and_cleans_temp() {
        let fx = Fixture::new("success");
        let target = fx.base.join("note.md");
        write_new_atomically(&target, "# 合成\n".as_bytes()).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "# 合成\n");
        assert!(temp_leftovers(&fx.base).is_empty());
        // 发布结果是普通文件，不是指向临时文件的符号链接。
        let meta = fs::symlink_metadata(&target).unwrap();
        assert!(meta.file_type().is_file());
        assert!(!meta.file_type().is_symlink());
    }

    #[test]
    fn new_write_failure_leaves_no_target() {
        let fx = Fixture::new("fail");
        // 目标父目录不存在：写入必须在创建任何目标之前失败。
        let target = fx.base.join("missing/note.md");
        assert!(write_new_atomically(&target, b"x").is_err());
        assert!(!target.exists());
        assert!(!fx.base.join("missing").exists());
    }
}
