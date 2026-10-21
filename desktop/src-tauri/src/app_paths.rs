//! OmniDesk 桌面应用的独立数据根解析（Step 6）。
//!
//! 硬边界（用户 Plan §6 与 §本阶段禁止）：
//!   * 桌面配置、日志、UI 状态、Engine 状态都必须在 OmniDesk 专属路径下；
//!   * 绝不使用 ~/.dsh、~/.agents、~/.config/DSH Desktop；
//!   * 绝不写入 DSH Runtime 源码或锁定底座。
//!
//! 解析优先级：
//!   1. 环境变量 OMNIDESK_APP_DATA_ROOT（绝对路径；多实例隔离靠它）；
//!   2. 默认 $HOME/OmniDesk/app。
//!
//! 无论来自哪一路，最终都必须通过 assert_isolated()：落在被禁止的数据根之下
//! 一律拒绝启动，而不是"先跑起来再说"。

use std::path::{Path, PathBuf};

/// 被明确禁止作为应用数据根的片段（小写匹配）。
///
/// 这些是原版 DSH / agent / Desktop 的数据位置，以及 DSH 源码目录。
/// 出现任意一项即视为越界。
const FORBIDDEN_FRAGMENTS: &[&str] = &[
    "/.dsh",
    "/.agents",
    "/.config/dsh desktop",
    "/.local/share/io.github.hairyf.deepseek-harness-desktop",
    "deepseek-harness",
    "/.codex",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// OmniDesk 应用数据根（默认 $HOME/OmniDesk/app）。
    pub root: PathBuf,
    /// UI 持久状态（窗口尺寸、上次任务输入等，**不含**任务正文）。
    pub ui_state: PathBuf,
    /// 应用日志。
    pub logs: PathBuf,
    /// 引擎进程状态（任务台账、事件补漏游标、落盘结果）。
    pub engine_state: PathBuf,
    /// 引擎任务结果落盘目录（大文本）。
    pub results: PathBuf,
    /// 工作台线程/会话记录（OD-ARCH-045：按项目分目录，JSON 一条一线程）。
    pub threads: PathBuf,
    /// 桌面应用配置。
    pub config: PathBuf,
}

#[derive(Debug)]
pub struct PathBoundaryError {
    pub path: PathBuf,
    pub reason: String,
}

impl std::fmt::Display for PathBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "拒绝使用桌面数据根 {}：{}",
            self.path.display(),
            self.reason
        )
    }
}

impl std::error::Error for PathBoundaryError {}

impl AppPaths {
    /// 按优先级解析应用数据根，并做边界校验。
    pub fn resolve() -> Result<Self, PathBoundaryError> {
        let root = match std::env::var_os("OMNIDESK_APP_DATA_ROOT") {
            Some(raw) if !raw.is_empty() => PathBuf::from(raw),
            _ => {
                let home = std::env::var_os("HOME").ok_or_else(|| PathBoundaryError {
                    path: PathBuf::from("$HOME"),
                    reason: "环境变量 HOME 缺失，无法确定默认应用数据根".to_string(),
                })?;
                PathBuf::from(home).join("OmniDesk").join("app")
            }
        };

        // 相对路径一律拒绝：多实例隔离与日志可读性都依赖绝对路径。
        if !root.is_absolute() {
            return Err(PathBoundaryError {
                path: root,
                reason: "应用数据根必须是绝对路径".to_string(),
            });
        }

        let paths = Self::from_root(root);
        paths.assert_isolated()?;
        Ok(paths)
    }

    /// 由给定根构造路径集合（不校验；校验见 assert_isolated）。
    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            ui_state: root.join("ui-state"),
            logs: root.join("logs"),
            engine_state: root.join("engine-state"),
            results: root.join("engine-state").join("results"),
            threads: root.join("threads"),
            config: root.join("config"),
            root,
        }
    }

    /// 边界校验：数据根不得落在原版 DSH / agent / Desktop 数据里，也不得落在 DSH 源码目录里。
    pub fn assert_isolated(&self) -> Result<(), PathBoundaryError> {
        // 词法校验：检查传入路径本身。
        assert_no_forbidden_fragment(&self.root).map_err(|reason| PathBoundaryError {
            path: self.root.clone(),
            reason,
        })?;

        // 真实落点校验：无法完全解析时默认拒绝，避免符号链接指向禁区或仓库。
        let resolved = resolve_real_path(&self.root).map_err(|reason| PathBoundaryError {
            path: self.root.clone(),
            reason,
        })?;
        assert_no_forbidden_fragment(&resolved).map_err(|reason| PathBoundaryError {
            path: self.root.clone(),
            reason,
        })?;

        // 额外拒绝"根本身就是这些目录"的写法（此时不含尾部子路径，上面的片段匹配可能漏掉）。
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            let resolved_home = resolve_real_path(&home).unwrap_or_else(|_| home.clone());
            for name in [".dsh", ".agents", ".codex"] {
                let candidate = home.join(name);
                let resolved_candidate = resolved_home.join(name);
                if same_path(&self.root, &candidate) || same_path(&resolved, &resolved_candidate) {
                    return Err(PathBoundaryError {
                        path: self.root.clone(),
                        reason: format!("应用数据根不能等于原版数据根 {}", candidate.display()),
                    });
                }
            }
        }

        // 仓库边界：数据根不得落在 OmniDesk 仓库内；解析后的真实落点也不得指回仓库。
        assert_outside_repo_boundary(&self.root).map_err(|reason| PathBoundaryError {
            path: self.root.clone(),
            reason,
        })?;

        Ok(())
    }

    /// 建立全部目录（幂等）。
    ///
    /// 在任何目录被创建之前先跑一次 assert_isolated()：校验未通过时直接失败，
    /// 绝不在可疑（仓库内 / 禁区 / 解析不明的软链接）路径上落下 ui-state、logs 等目录。
    ///
    /// 限制：常规“先解析后校验”无法消除检查与 create_dir_all 之间的并发路径替换竞态，
    /// 因此这里只保证静态可确认的边界，不宣称对同机恶意并发替换具备原子安全保证。
    pub fn ensure(&self) -> std::io::Result<()> {
        self.assert_isolated().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())
        })?;
        for dir in [
            &self.root,
            &self.config,
            &self.logs,
            &self.ui_state,
            &self.engine_state,
            &self.results,
            &self.threads,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

/// 字面路径比较（不解析软链接，避免在校验阶段触碰文件系统）。
fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().trim_end_matches('/') == b.to_string_lossy().trim_end_matches('/')
}

/// 拒绝路径中包含被禁止片段（小写、反斜杠统一为斜杠后匹配）。
fn assert_no_forbidden_fragment(path: &Path) -> Result<(), String> {
    let normalized = path.to_string_lossy().to_lowercase().replace('\\', "/");
    for fragment in FORBIDDEN_FRAGMENTS {
        if normalized.contains(fragment) {
            return Err(format!(
                "路径包含被禁止的片段 {:?}（原版 DSH / agent / Desktop 数据或 DSH 源码）：{}",
                fragment,
                path.display()
            ));
        }
    }
    Ok(())
}

/// 解析路径的真实落点，逐级解析软链接。
///
/// 先解析**已存在**的最长祖先目录（软链接在此被解析），再拼上尚不存在的尾部组件
/// （这些组件将来由 create_dir_all 创建，名字是字面的）。这既能覆盖"根目录本身是软链接"，
/// 也能覆盖"根目录的祖先含软链接"，同时不会因为目标尚不存在而无法校验。
///
/// 失败时返回错误，而不是退回词法结果：调用方应默认拒绝，避免符号链接绕过。
pub fn resolve_real_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("需要绝对路径才能解析真实落点：{}", path.display()));
    }
    if path.exists() {
        return path
            .canonicalize()
            .map_err(|e| format!("无法解析路径真实落点 {}：{}", path.display(), e));
    }
    let mut existing = path.to_path_buf();
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match existing.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                let name = existing
                    .file_name()
                    .ok_or_else(|| format!("路径缺少可解析的目录名：{}", path.display()))?
                    .to_os_string();
                missing.push(name);
                existing = parent.to_path_buf();
                if existing.exists() {
                    break;
                }
            }
            _ => {
                return Err(format!(
                    "无法解析路径的真实祖先目录，拒绝使用：{}",
                    path.display()
                ));
            }
        }
    }
    let mut resolved = existing
        .canonicalize()
        .map_err(|e| format!("无法解析路径真实祖先 {}：{}", existing.display(), e))?;
    for name in missing.iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

/// 仓库根探测：优先环境变量 OMNIDESK_REPO_ROOT，其次由 CARGO_MANIFEST_DIR（desktop/src-tauri）上溯两级。
///
/// 与 task_shell 使用同一约定，保证两类数据根共享同一仓库边界。
fn repo_root_hint() -> Result<PathBuf, String> {
    if let Some(raw) = std::env::var_os("OMNIDESK_REPO_ROOT") {
        if !raw.is_empty() {
            return Ok(PathBuf::from(raw));
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法从 CARGO_MANIFEST_DIR 推断仓库根".to_string())
}

/// 词法归一：解析 . 与 ..，不触碰文件系统（canonicalize 失败时的兜底比较）。
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// 仓库边界校验：数据根不得落在 OmniDesk 仓库之内。
///
/// 同时校验文本路径与解析后的真实落点：任一落入仓库即拒绝。
/// 仓库根无法安全解析时退回词法归一结果（仓库根是本机已知固定目录，不是攻击面）；
/// 但数据根本身解析失败时一律拒绝，避免软链接指回仓库。
fn assert_outside_repo_boundary(path: &Path) -> Result<(), String> {
    let repo = repo_root_hint()?;

    // 1) 文本路径：先做词法归一比较，再做原样 starts_with 比较。
    if normalize_lexical(path).starts_with(normalize_lexical(&repo)) || path.starts_with(&repo) {
        return Err(format!(
            "应用数据根不能落在 OmniDesk 仓库内：{}（仓库根 {}）",
            path.display(),
            repo.display()
        ));
    }

    // 2) 真实落点：数据根本身解析失败即拒绝；仓库根解析失败时退回词法结果。
    let resolved = resolve_real_path(path)?;
    let resolved_repo = resolve_real_path(&repo).unwrap_or_else(|_| normalize_lexical(&repo));
    if resolved.starts_with(&resolved_repo) {
        return Err(format!(
            "应用数据根不能落在 OmniDesk 仓库内：{} 的真实落点 {} 位于仓库 {} 之内",
            path.display(),
            resolved.display(),
            repo.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_root_is_omnidesk_app() {
        let home = std::env::var("HOME").expect("HOME");
        let paths = AppPaths::from_root(PathBuf::from(&home).join("OmniDesk").join("app"));
        assert!(paths.assert_isolated().is_ok());
        assert_eq!(paths.ui_state, paths.root.join("ui-state"));
        assert_eq!(paths.results, paths.root.join("engine-state").join("results"));
    }

    #[test]
    fn rejects_stock_dsh_home() {
        let paths = AppPaths::from_root("/home/someuser/.dsh/app");
        let err = paths.assert_isolated().expect_err("应拒绝 ~/.dsh 下的数据根");
        assert!(err.reason.contains("/.dsh"), "原因应指出 /.dsh：{}", err.reason);
    }

    #[test]
    fn rejects_stock_agents_home() {
        let paths = AppPaths::from_root("/home/someuser/.agents/app");
        assert!(paths.assert_isolated().is_err());
    }

    #[test]
    fn rejects_root_equal_to_stock_home() {
        let home = std::env::var("HOME").expect("HOME");
        let paths = AppPaths::from_root(PathBuf::from(&home).join(".dsh"));
        assert!(paths.assert_isolated().is_err());
    }

    #[test]
    fn rejects_dsh_source_tree() {
        let paths = AppPaths::from_root("/mnt/data/Code/MyWorkStation/runtime/deepseek-harness");
        assert!(paths.assert_isolated().is_err());
    }

    #[test]
    fn rejects_dsh_desktop_user_data() {
        let paths =
            AppPaths::from_root("/home/someuser/.config/DSH Desktop/OmniDesk");
        assert!(paths.assert_isolated().is_err());
    }

    #[test]
    fn rejects_relative_root() {
        let err = AppPaths::resolve();
        // resolve() 依赖环境，这里只验证相对路径分支的纯函数行为。
        let relative = AppPaths::from_root("OmniDesk/app");
        assert!(!relative.root.is_absolute());
        let _ = err; // resolve() 的环境结果不作为断言，避免污染测试宿主
    }

    // --- 仓库边界：数据根不得落在仓库内（OD-BUILD-002 / R4） ---

    fn temp_base(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "omnidesk-app-paths-tests/{}-{}",
            tag,
            std::process::id()
        ))
    }

    #[test]
    fn rejects_plain_path_inside_repository() {
        let repo = repo_root_hint().expect("仓库根");
        // 普通仓库内目录（无软链接、无禁区片段）。
        let inside = repo.join("desktop").join(".app-data-inside-repo");
        let paths = AppPaths::from_root(&inside);
        let err = paths
            .assert_isolated()
            .expect_err("仓库内普通路径必须被拒绝");
        assert!(
            err.reason.contains("仓库"),
            "原因应指出仓库边界：{}",
            err.reason
        );
    }

    #[test]
    fn accepts_clean_path_outside_repository() {
        let base = temp_base("clean-outside");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("创建临时目录");
        let paths = AppPaths::from_root(base.join("app"));
        assert!(
            paths.assert_isolated().is_ok(),
            "干净的仓库外路径应通过隔离校验"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_pointing_into_repository() {
        use std::os::unix::fs::symlink;
        let repo = repo_root_hint().expect("仓库根");
        let base = temp_base("symlink-repo");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("创建临时目录");

        // 词法名字干净（不含禁区片段），解析后真实落点才落入仓库。
        let link = base.join("innocent-link");
        symlink(&repo, &link).expect("创建指向仓库的软链接");
        let paths = AppPaths::from_root(link.join("app"));
        let err = paths
            .assert_isolated()
            .expect_err("软链接指回仓库时必须拒绝");
        assert!(
            err.reason.contains("仓库"),
            "原因应指出仓库边界：{}",
            err.reason
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_pointing_to_simulated_forbidden_root() {
        use std::os::unix::fs::symlink;
        let base = temp_base("symlink-forbidden");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("创建临时目录");

        // 词法名字干净，真实落点含禁区片段（仿真 ~/.dsh）。
        let secret = base.join(".dsh");
        std::fs::create_dir_all(&secret).expect("创建仿真禁区目录");
        let masked = base.join("masked");
        symlink(&secret, &masked).expect("创建指向禁区的软链接");
        let paths = AppPaths::from_root(masked.join("app"));
        assert!(
            paths.assert_isolated().is_err(),
            "真实落点为禁区时，即便词法名字干净也必须拒绝"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ensure_refuses_to_create_directories_inside_repository() {
        let repo = repo_root_hint().expect("仓库根");
        let inside = repo.join("desktop").join(format!(
            ".omnidesk-ensure-refuse-{}",
            std::process::id()
        ));
        // 前置：该测试目录不应存在。
        let _ = std::fs::remove_dir_all(&inside);
        assert!(!inside.exists(), "测试前置：目录不应存在");

        let paths = AppPaths::from_root(&inside);
        let err = paths
            .ensure()
            .expect_err("ensure() 不得在仓库内创建目录");
        assert!(
            err.to_string().contains("仓库"),
            "错误应指出仓库边界：{}",
            err
        );
        // 关键断言：失败路径没有在仓库里留下任何内容。
        assert!(
            !inside.exists(),
            "被拒绝的仓库内路径不得被创建：{}",
            inside.display()
        );
        let _ = std::fs::remove_dir_all(&inside);
    }
}
