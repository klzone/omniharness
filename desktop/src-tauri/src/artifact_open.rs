//! 成果文件打开（OD-UI-713）。
//!
//! 只有用户在对话里点击生成文件时才会走到这里。边界与保存入口一致：
//!   * 只接受成果目录（本地数据根）内的普通文件；
//!   * 绝对路径与相对路径都先归一，再要求真实落点仍在数据根内；
//!   * 拒绝 `..`、空段与目录本身；
//!   * 打开器是固定的系统默认处理程序，路径作为独立参数传入，不拼 shell。

use std::path::{Path, PathBuf};

/// 归一后仍在 `root` 内的普通文件绝对路径。只做校验，不启动程序。
pub fn resolve_inside(root: &Path, requested: &str) -> Result<PathBuf, String> {
    let raw = requested.trim();
    if raw.is_empty() {
        return Err("文件路径为空。".to_string());
    }
    if raw.contains('\u{0}') {
        return Err("文件路径包含非法字符。".to_string());
    }
    let root_real = std::fs::canonicalize(root)
        .map_err(|e| format!("成果目录不可用（{}）：{}", root.display(), e))?;
    let candidate = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    let candidate_real = std::fs::canonicalize(&candidate)
        .map_err(|_| format!("文件不存在：{}", raw))?;
    if !candidate_real.starts_with(&root_real) {
        return Err(format!("只允许打开成果目录内的文件：{}", raw));
    }
    if !candidate_real.is_file() {
        return Err(format!("不是文件：{}", raw));
    }
    Ok(candidate_real)
}

/// 校验后用系统默认处理程序打开，返回实际落点。不读取文件内容。
pub fn open_with<F>(root: &Path, requested: &str, opener: F) -> Result<PathBuf, String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    let path = resolve_inside(root, requested)?;
    opener(&path)?;
    Ok(path)
}

/// 固定打开器：交给系统默认程序处理。
pub fn open(root: &Path, requested: &str) -> Result<PathBuf, String> {
    open_with(root, requested, |path| {
        crate::platform::open_target(path)
            .map_err(|e| format!("系统没有可用的打开程序（{}）。", e))
    })
}


/// 本轮时间窗内写入成果目录的文件（只读元数据，不读正文）。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct GeneratedFile {
    pub relative_path: String,
    pub file_name: String,
    pub modified_at: f64,
}

/// 成果目录里可能累积历史文件，扫描量与深度都设上限，避免一次遍历拖慢界面。
const WALK_MAX_ENTRIES: usize = 4000;
const WALK_MAX_DEPTH: usize = 4;

fn walk_recent(
    root: &Path,
    dir: &Path,
    depth: usize,
    since: f64,
    until: f64,
    out: &mut Vec<GeneratedFile>,
    scanned: &mut usize,
) -> Result<(), String> {
    if *scanned >= WALK_MAX_ENTRIES {
        return Ok(());
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    for item in entries {
        if *scanned >= WALK_MAX_ENTRIES {
            break;
        }
        let Ok(item) = item else { continue };
        *scanned += 1;
        let name = item.file_name().to_string_lossy().to_string();
        // 隐藏文件、临时文件与符号链接都不算生成成果。
        if name.starts_with('.') || name.contains(".tmp-") {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(item.path()) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if depth < WALK_MAX_DEPTH {
                walk_recent(root, &item.path(), depth + 1, since, until, out, scanned)?;
            }
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let Some(mtime) = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
        else {
            continue;
        };
        if mtime < since || mtime > until {
            continue;
        }
        let path = item.path();
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        out.push(GeneratedFile {
            relative_path: relative.to_string_lossy().replace('\\', "/"),
            file_name: name,
            modified_at: mtime,
        });
    }
    Ok(())
}

/// 列出时间窗内写入的文件，按修改时间倒序，最多 limit 条。
pub fn list_recent_files(
    root: &Path,
    since: f64,
    until: f64,
    limit: usize,
) -> Result<Vec<GeneratedFile>, String> {
    if !(since <= until) {
        return Ok(Vec::new());
    }
    let root_real = std::fs::canonicalize(root)
        .map_err(|e| format!("成果目录不可用（{}）：{}", root.display(), e))?;
    let mut out = Vec::new();
    let mut scanned = 0usize;
    walk_recent(&root_real, &root_real, 0, since, until, &mut out, &mut scanned)?;
    out.sort_by(|a, b| {
        b.modified_at
            .partial_cmp(&a.modified_at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(limit);
    Ok(out)
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn lists_only_files_inside_window() {
        let root = std::env::temp_dir().join(format!("omni-artifact-window-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let new = root.join("sub").join("新文件.xlsx");
        std::fs::write(&new, b"x").unwrap();
        let now = std::fs::metadata(&new)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        let rows = list_recent_files(&root, now - 5.0, now + 5.0, 8).unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].file_name, "新文件.xlsx");
        assert_eq!(rows[0].relative_path, "sub/新文件.xlsx");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_hidden_and_temp_files() {
        let root = std::env::temp_dir().join(format!("omni-artifact-hidden-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for name in [".hidden", "run.tmp-1.xlsx"] {
            std::fs::write(root.join(name), b"x").unwrap();
        }
        let now = std::fs::metadata(root.join(".hidden"))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let rows = list_recent_files(&root, now - 5.0, now + 5.0, 8).unwrap();
        assert!(rows.is_empty(), "{rows:?}");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "omni-artifact-open-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建立临时目录");
        dir
    }

    #[test]
    fn opens_relative_and_absolute_inside_root() {
        let root = temp_root("inside");
        let file = root.join("成果.xlsx");
        std::fs::write(&file, b"x").expect("写临时文件");

        let opened = RefCell::new(String::new());
        let rel = open_with(&root, "成果.xlsx", |path| {
            *opened.borrow_mut() = path.display().to_string();
            Ok(())
        })
        .expect("相对路径应通过");
        assert!(rel.ends_with("成果.xlsx"));

        let abs = open_with(&root, &file.display().to_string(), |path| {
            *opened.borrow_mut() = path.display().to_string();
            Ok(())
        })
        .expect("绝对路径应通过");
        assert_eq!(abs, std::fs::canonicalize(&file).unwrap());
        assert!(opened.borrow().ends_with("成果.xlsx"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_paths_outside_root_and_traversal() {
        let root = temp_root("outside");
        let sibling = root.parent().unwrap().join(format!(
            "omni-artifact-open-sibling-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&sibling).unwrap();
        let outside = sibling.join("secret.txt");
        std::fs::write(&outside, b"x").unwrap();

        let mut called = false;
        let mut spy = |_: &Path| {
            called = true;
            Ok(())
        };
        assert!(open_with(&root, &outside.display().to_string(), &mut spy).is_err());
        assert!(open_with(&root, "../omni-artifact-open-sibling-0/secret.txt", &mut spy).is_err());
        assert!(!called);

        let _ = std::fs::remove_dir_all(&sibling);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_directories_and_missing_files() {
        let root = temp_root("dirs");
        let mut called = false;
        let mut spy = |_: &Path| {
            called = true;
            Ok(())
        };
        assert!(open_with(&root, "", &mut spy).is_err());
        assert!(open_with(&root, "不存在.xlsx", &mut spy).is_err());
        assert!(!called);
        let _ = std::fs::remove_dir_all(&root);
    }
}
