//! 桌面应用设置（omnidesk.settings.v1）。
//!
//! 保存两项：用户选择的“新成果保存目录”，以及按端点类别划分的模型权限。
//!
//! 模型权限默认全部关闭（OD-ARCH-021）：本机 / 单位内部 / 外部三类各自独立，
//! 由用户显式开启并可随时撤销；未知类别没有开关，宿主一律拒绝。
//!
//! 硬边界：
//!   * 设置文件位于应用数据根的 `config/` 下，与成果目录分开；
//!   * 成果目录必须是绝对路径，不得落在仓库、原版 DSH / agent / Codex 私有目录，
//!     也不得落在应用自身数据根内；
//!   * 校验通过后才创建目录，写入用一次性探针并清理；
//!   * 改动只影响之后新写入的成果，不迁移、不覆盖、不删除既有文件。
//!
//! 生效优先级：`OMNIDESK_LOCAL_DATA_ROOT`（开发/测试隔离覆盖）> 用户设置 > 默认目录。
//! 开发隔离变量存在时会明确告知界面“本条设置被隔离变量覆盖”。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（Rust 测试运行）。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::app_paths::AppPaths;
use crate::task_shell::{assert_local_root_allowed, assert_outside_repo};

pub const SETTINGS_SCHEMA: &str = "omnidesk.settings.v1";
pub const SETTINGS_FILE: &str = "settings.v1.json";
/// 与 task_shell 保持同一个隔离覆盖变量名。
pub const LOCAL_ROOT_ENV: &str = "OMNIDESK_LOCAL_DATA_ROOT";
/// 默认新成果目录（相对 $HOME）。
const DEFAULT_ARTIFACT_SUBDIR: &str = "OmniDesk/workspace/artifacts";
/// 单条路径最大长度（字符），防止异常配置。
const MAX_PATH_CHARS: usize = 1024;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema: String,
    /// 用户显式选择的成果目录；None 表示使用默认目录。
    #[serde(default)]
    pub artifact_root: Option<String>,
    /// 按端点类别授予的模型权限。旧设置文件缺少该字段时按“全部关闭”处理。
    #[serde(default)]
    pub model_permissions: ModelPermissions,
}

/// 可配置的端点类别。未知类别不在其中，因此没有“开启”入口。
///
/// 类别只区分“本机回环 / 非回环”，**不作**组织归属承诺：地址范围无法证明端点
/// 属于哪个组织，也无法证明数据是否离开所在组织。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPermissions {
    /// 本机回环端点：可就地判定内容不出本机。
    #[serde(default)]
    pub local: bool,
    /// 非回环的私有 / 链路本地 / .local 地址。仅为地址范围事实，
    /// 不代表所属组织。旧设置文件的 `internal` 键按本类别迁移读取。
    #[serde(default, alias = "internal")]
    pub private: bool,
    /// 非回环的公网地址。是否为组织内代理无法从地址判断。
    /// 旧设置文件的 `external` 键按本类别迁移读取。
    #[serde(default, alias = "external")]
    pub public: bool,
}

impl ModelPermissions {
    /// 读取某一类别的授权状态；未知类别返回 None（不存在默认放宽）。
    pub fn for_category(&self, category: &str) -> Option<bool> {
        match category {
            "local" => Some(self.local),
            "private" => Some(self.private),
            "public" => Some(self.public),
            _ => None,
        }
    }

    /// 撤销全部权限。
    pub fn revoke_all(&mut self) {
        self.local = false;
        self.private = false;
        self.public = false;
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: SETTINGS_SCHEMA.to_string(),
            artifact_root: None,
            model_permissions: ModelPermissions::default(),
        }
    }
}

pub fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE)
}

/// 应用配置目录（与 AppPaths::resolve 的规则一致，但不做目录创建）。
///
/// 单独提供的原因：成果目录解析需要在没有 Tauri State 的纯函数中定位设置文件。
pub fn app_config_dir() -> Result<PathBuf, String> {
    match std::env::var_os("OMNIDESK_APP_DATA_ROOT") {
        Some(raw) if !raw.is_empty() => Ok(PathBuf::from(raw).join("config")),
        _ => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| "环境变量 HOME 缺失，无法确定应用配置目录".to_string())?;
            Ok(PathBuf::from(home).join("OmniDesk").join("app").join("config"))
        }
    }
}

/// 读取设置。文件不存在视为默认设置；文件损坏或版本不符时整次拒绝，
/// 不静默退回默认目录（否则用户以为成果仍写入自己选的位置）。
pub fn load(config_dir: &Path) -> Result<Settings, String> {
    let path = settings_path(config_dir);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(e) => return Err(format!("读取设置失败（{}）：{}", path.display(), e)),
    };
    let settings: Settings = serde_json::from_str(&text)
        .map_err(|e| format!("设置文件格式错误（{}）：{}", path.display(), e))?;
    if settings.schema != SETTINGS_SCHEMA {
        return Err(format!(
            "不支持的设置版本 {:?}；当前只接受 {:?}",
            settings.schema, SETTINGS_SCHEMA
        ));
    }
    if let Some(root) = settings.artifact_root.as_deref() {
        if root.chars().count() > MAX_PATH_CHARS {
            return Err("成果目录路径过长，拒绝使用".to_string());
        }
        if root.trim().is_empty() {
            return Err("成果目录为空，拒绝使用；请删除该设置项或填写绝对路径".to_string());
        }
    }
    Ok(settings)
}

/// 用户显式配置的目录（未配置时为 None）。
pub fn configured_root(config_dir: &Path) -> Result<Option<PathBuf>, String> {
    Ok(load(config_dir)?.artifact_root.map(PathBuf::from))
}

/// 开发/测试隔离覆盖是否生效。
pub fn env_override() -> Option<PathBuf> {
    match std::env::var_os(LOCAL_ROOT_ENV) {
        Some(raw) if !raw.is_empty() => Some(PathBuf::from(raw)),
        _ => None,
    }
}

fn default_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| "环境变量 HOME 缺失，无法确定默认成果目录".to_string())?;
    Ok(PathBuf::from(home).join(DEFAULT_ARTIFACT_SUBDIR))
}

/// 校验一个候选成果目录：绝对路径、无禁止片段、真实落点在允许范围、不在仓库或应用数据根内。
pub fn assert_artifact_root_allowed(path: &Path, app_root: Option<&Path>) -> Result<(), String> {
    assert_local_root_allowed(path)?;
    assert_outside_repo(path)?;
    if let Some(app_root) = app_root {
        let app_norm = crate::app_paths::resolve_real_path(app_root)?;
        let path_norm = crate::app_paths::resolve_real_path(path)?;
        if path_norm.starts_with(&app_norm) {
            return Err(format!(
                "成果目录不能放在应用自身数据目录内：{}（应用数据根 {}）",
                path.display(),
                app_root.display()
            ));
        }
    }
    if path.exists() && !path.is_dir() {
        return Err(format!("目标已存在且不是目录：{}", path.display()));
    }
    Ok(())
}

/// 一次性写入探针：在已校验的目录里新建、写入、刷盘、删除一个临时文件。
/// 失败时尽力清理，并如实区分“清理成功”与“清理失败”。
pub fn probe_writable(path: &Path) -> Result<(), String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let probe = path.join(format!(
        ".omnidesk-write-probe-{}-{}",
        std::process::id(),
        seq
    ));
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .map_err(|e| format!("目录不可写：{}（{}）", path.display(), e))?;
        file.write_all(b"omnidesk write probe\n")
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("目录写入探针失败：{}（{}）", path.display(), e))
    })();
    match fs::remove_file(&probe) {
        Ok(()) => result,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => result,
        Err(e) => {
            let cleanup = format!("探针文件未能清理：{}（{}）", probe.display(), e);
            Err(match result {
                Ok(()) => cleanup,
                Err(inner) => format!("{}；{}", inner, cleanup),
            })
        }
    }
}

/// 当前实际生效的新成果目录及其来源。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EffectiveRoot {
    /// env_override / user / default
    pub source: &'static str,
    pub root: PathBuf,
    pub configured_root: Option<PathBuf>,
    pub env_override: Option<PathBuf>,
}

pub fn effective_root(config_dir: &Path) -> Result<EffectiveRoot, String> {
    let env = env_override();
    let configured = configured_root(config_dir)?;
    let root = if let Some(env) = env.clone() {
        env
    } else if let Some(user) = configured.clone() {
        user
    } else {
        default_root()?
    };
    // 隔离覆盖是开发/测试入口，沿用既有 task_shell 的校验；用户设置与默认目录
    // 还必须与应用自身数据根分开（config 的父目录即应用根）。
    let app_root = config_dir.parent();
    if env.is_some() {
        assert_local_root_allowed(&root)?;
        assert_outside_repo(&root)?;
    } else {
        assert_artifact_root_allowed(&root, app_root)?;
    }
    Ok(EffectiveRoot {
        source: if env.is_some() {
            "env_override"
        } else if configured.is_some() {
            "user"
        } else {
            "default"
        },
        root,
        configured_root: configured,
        env_override: env,
    })
}

/// 原子写入设置文件：先写临时文件并刷盘，再改名。
fn write_settings(config_dir: &Path, settings: &Settings) -> Result<(), String> {
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("创建设置目录失败（{}）：{}", config_dir.display(), e))?;
    let target = settings_path(config_dir);
    let temp = config_dir.join(format!("{}.tmp-{}", SETTINGS_FILE, std::process::id()));
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化设置失败：{}", e))?;
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)
            .map_err(|e| format!("写入临时设置文件失败：{}", e))?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入临时设置文件失败：{}", e))?;
        fs::rename(&temp, &target).map_err(|e| format!("替换设置文件失败：{}", e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 应用并持久化新的成果目录：校验 → 创建 → 探针 → 写入设置。
/// 探针或校验失败时不会写入设置文件，旧设置保持不变。
/// 写入时保留同一文件中的其他设置（模型权限），不得整份覆盖。
pub fn set_artifact_root(paths: &AppPaths, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("请输入绝对路径，例如 /home/you/Documents/OmniDesk".to_string());
    }
    if trimmed.chars().count() > MAX_PATH_CHARS {
        return Err("路径过长，拒绝使用".to_string());
    }
    let candidate = PathBuf::from(trimmed);
    if !candidate.is_absolute() {
        return Err(format!("必须是绝对路径：{}", candidate.display()));
    }
    assert_artifact_root_allowed(&candidate, Some(&paths.root))?;
    fs::create_dir_all(&candidate)
        .map_err(|e| format!("创建成果目录失败（{}）：{}", candidate.display(), e))?;
    // 创建后再次校验真实落点，防止符号链接把目录引到禁区。
    assert_artifact_root_allowed(&candidate, Some(&paths.root))?;
    probe_writable(&candidate)?;
    // 设置文件损坏时整次拒绝，避免在用户不知情时重置模型权限。
    let mut settings = load(&paths.config)?;
    settings.artifact_root = Some(candidate.display().to_string());
    write_settings(&paths.config, &settings)?;
    Ok(candidate)
}

/// 设置单个类别（local / private / public）的模型权限并持久化。
/// 未知类别直接拒绝：不存在“未知也放行”的入口。
///
/// 旧类别名 `internal` / `external` 作为别名接受，按新类别迁移，
/// 以免旧前端或旧记录写入时被误判为未知类别。
pub fn set_model_permission(
    config_dir: &Path,
    category: &str,
    enabled: bool,
) -> Result<ModelPermissions, String> {
    let mut settings = load(config_dir)?;
    match category {
        "local" => settings.model_permissions.local = enabled,
        "private" | "internal" => settings.model_permissions.private = enabled,
        "public" | "external" => settings.model_permissions.public = enabled,
        other => {
            return Err(format!(
                "不支持修改该端点类别的权限：{}（只支持 local / private / public）",
                other
            ))
        }
    }
    write_settings(config_dir, &settings)?;
    Ok(settings.model_permissions)
}

/// 一次性撤销全部模型权限。
pub fn revoke_all_model_permissions(config_dir: &Path) -> Result<ModelPermissions, String> {
    let mut settings = load(config_dir)?;
    settings.model_permissions.revoke_all();
    write_settings(config_dir, &settings)?;
    Ok(settings.model_permissions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_app(tag: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "omnidesk-settings-tests/{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("config")).unwrap();
        AppPaths::from_root(root)
    }

    fn with_no_env<T>(body: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var_os(LOCAL_ROOT_ENV);
        std::env::remove_var(LOCAL_ROOT_ENV);
        let out = body();
        match previous {
            Some(value) => std::env::set_var(LOCAL_ROOT_ENV, value),
            None => std::env::remove_var(LOCAL_ROOT_ENV),
        }
        out
    }

    #[test]
    fn defaults_to_home_artifacts_when_unset() {
        with_no_env(|| {
            let app = temp_app("default");
            let eff = effective_root(&app.config).unwrap();
            assert_eq!(eff.source, "default");
            assert!(eff.root.ends_with("OmniDesk/workspace/artifacts"));
        });
    }

    #[test]
    fn set_root_persists_and_probe_leaves_no_file() {
        with_no_env(|| {
            let app = temp_app("persist");
            let target = std::env::temp_dir().join(format!("omnidesk-artifacts-{}", std::process::id()));
            let _ = fs::remove_dir_all(&target);
            set_artifact_root(&app, target.to_str().unwrap()).unwrap();
            assert!(target.is_dir());
            let leftovers: Vec<_> = fs::read_dir(&target)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with(".omnidesk-write-probe"))
                .collect();
            assert!(leftovers.is_empty(), "探针文件必须清理干净");
            let eff = effective_root(&app.config).unwrap();
            assert_eq!(eff.source, "user");
            assert_eq!(eff.root, target);
            // 设置与成果目录分开存：成果目录里不应出现设置文件。
            assert!(!target.join(SETTINGS_FILE).exists());
            assert!(settings_path(&app.config).exists());
        });
    }

    #[test]
    fn model_permissions_default_closed_and_persist_per_category() {
        with_no_env(|| {
            let app = temp_app("permissions");
            assert_eq!(load(&app.config).unwrap().model_permissions, ModelPermissions::default());

            let saved = set_model_permission(&app.config, "public", true).unwrap();
            assert!(saved.public && !saved.local && !saved.private);
            // 重新读取（模拟重启）仍然是同一状态。
            assert!(load(&app.config).unwrap().model_permissions.public);

            let saved = set_model_permission(&app.config, "local", true).unwrap();
            assert!(saved.local && saved.public && !saved.private);
            let saved = set_model_permission(&app.config, "public", false).unwrap();
            assert!(saved.local && !saved.public);
        });
    }

    #[test]
    fn unknown_category_cannot_be_enabled() {
        with_no_env(|| {
            let app = temp_app("unknown-category");
            let err = set_model_permission(&app.config, "unknown", true).unwrap_err();
            assert!(err.contains("不支持"), "实际错误：{}", err);
            assert_eq!(load(&app.config).unwrap().model_permissions, ModelPermissions::default());
        });
    }

    /// OD-ARCH-022：旧设置文件的 `internal` / `external` 键必须迁移到
    /// `private` / `public`，不得因改名而丢失用户已授予的权限或静默重置。
    #[test]
    fn legacy_category_keys_migrate_to_private_and_public() {
        with_no_env(|| {
            let app = temp_app("legacy-keys");
            fs::create_dir_all(&app.config).unwrap();
            fs::write(
                settings_path(&app.config),
                r#"{"schema":"omnidesk.settings.v1","artifact_root":null,"model_permissions":{"local":true,"internal":true,"external":false}}"#,
            )
            .unwrap();

            let loaded = load(&app.config).unwrap().model_permissions;
            assert!(loaded.local, "local 应保留");
            assert!(loaded.private, "旧 internal 键应迁移到 private");
            assert!(!loaded.public, "旧 external=false 应迁移到 public=false");

            // 旧类别名仍可写入，但落在新字段上（兼容尚未更新的调用方）。
            assert!(set_model_permission(&app.config, "external", true).unwrap().public);
            assert!(set_model_permission(&app.config, "internal", true).unwrap().private);
        });
    }

    #[test]
    fn changing_artifact_root_preserves_model_permissions() {
        with_no_env(|| {
            let app = temp_app("preserve-permissions");
            set_model_permission(&app.config, "private", true).unwrap();
            let target = std::env::temp_dir()
                .join(format!("omnidesk-preserve-{}", std::process::id()));
            let _ = fs::remove_dir_all(&target);
            set_artifact_root(&app, target.to_str().unwrap()).unwrap();

            let settings = load(&app.config).unwrap();
            assert!(settings.model_permissions.private, "改成果目录不得重置模型权限");
            assert_eq!(settings.artifact_root.as_deref(), target.to_str());
            let _ = fs::remove_dir_all(&target);
        });
    }

    #[test]
    fn revoke_all_clears_every_category() {
        with_no_env(|| {
            let app = temp_app("revoke");
            set_model_permission(&app.config, "local", true).unwrap();
            set_model_permission(&app.config, "private", true).unwrap();
            set_model_permission(&app.config, "public", true).unwrap();
            let revoked = revoke_all_model_permissions(&app.config).unwrap();
            assert_eq!(revoked, ModelPermissions::default());
            assert_eq!(load(&app.config).unwrap().model_permissions, ModelPermissions::default());
        });
    }

    #[test]
    fn corrupt_settings_are_not_silently_reset() {
        with_no_env(|| {
            let app = temp_app("corrupt");
            fs::write(settings_path(&app.config), "{ not json").unwrap();
            assert!(set_model_permission(&app.config, "public", true).is_err());
            let target = std::env::temp_dir()
                .join(format!("omnidesk-corrupt-{}", std::process::id()));
            let _ = fs::remove_dir_all(&target);
            assert!(set_artifact_root(&app, target.to_str().unwrap()).is_err());
            // 损坏文件保持原样，不被默认值覆盖。
            assert_eq!(fs::read_to_string(settings_path(&app.config)).unwrap(), "{ not json");
            let _ = fs::remove_dir_all(&target);
        });
    }

    #[test]
    fn rejects_relative_forbidden_repo_and_app_roots() {
        with_no_env(|| {
            let app = temp_app("reject");
            assert!(set_artifact_root(&app, "relative/dir").is_err());
            assert!(set_artifact_root(&app, "/home/example/.dsh/out").is_err());
            assert!(set_artifact_root(&app, "/home/example/.agents/out").is_err());
            assert!(set_artifact_root(&app, "/home/example/.codex/out").is_err());
            let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|p| p.parent())
                .expect("仓库根可推断")
                .to_path_buf();
            let inside_repo = repo.join("out-artifacts");
            assert!(set_artifact_root(&app, inside_repo.to_str().unwrap()).is_err());
            let inside_app = app.root.join("artifacts");
            assert!(set_artifact_root(&app, inside_app.to_str().unwrap()).is_err());
            // 失败不得留下设置文件或目录。
            assert!(!settings_path(&app.config).exists());
            assert!(!inside_app.exists());
        });
    }

    #[test]
    fn env_override_wins_and_is_reported() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let app = temp_app("env");
        let env_root = std::env::temp_dir().join(format!("omnidesk-env-root-{}", std::process::id()));
        let previous = std::env::var_os(LOCAL_ROOT_ENV);
        std::env::set_var(LOCAL_ROOT_ENV, &env_root);
        let eff = effective_root(&app.config).unwrap();
        assert_eq!(eff.source, "env_override");
        assert_eq!(eff.root, env_root);
        match previous {
            Some(v) => std::env::set_var(LOCAL_ROOT_ENV, v),
            None => std::env::remove_var(LOCAL_ROOT_ENV),
        }
    }

    #[test]
    fn corrupt_settings_are_rejected_not_silently_ignored() {
        with_no_env(|| {
            let app = temp_app("corrupt");
            fs::write(settings_path(&app.config), "{not json").unwrap();
            assert!(load(&app.config).is_err());
            assert!(effective_root(&app.config).is_err());
            fs::write(settings_path(&app.config), r#"{"schema":"omnidesk.settings.v99"}"#).unwrap();
            assert!(load(&app.config).is_err());
        });
    }
}
