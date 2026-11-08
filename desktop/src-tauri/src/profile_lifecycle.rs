//! 自定义 Profile 生命周期（OD-ARCH-109 补充：新建 / 复制 / 移除）。
//!
//! 产品口径：OmniDesk 是干净底座，Profile 是定制工作台。除了出厂 Profile
//! （discipline）由 `profile_assets` 自动装配，用户还要能自己造一个 Profile，
//! 从基础标准起手、或复制现有 Profile 再改。
//!
//! 边界：
//!   * 只动隔离 `DSH_HOME/profiles/<name>/` 与应用数据根 `profiles/<name>/`；
//!   * 不覆盖已存在的目录（新建拒绝同名，复制拒绝同名）；
//!   * 不复制 `node_modules`：插件缺失由「Profile 资产」页探测、由用户点击安装；
//!   * 移除是**移入回收目录**，不是删除：`.trash/<name>-<epoch>-<pid>/`，
//!     并在目录里留一份 `trash.v1.json` 元数据，软件内可一键恢复（OD-ARCH-125）；
//!   * 出厂 Profile 与当前在用的 Profile 不许移除。
//!
//! 自定义 Profile 靠应用数据根 `profiles/index.v1.json` 登记，DSH 目录里
//! 历史遗留的 `web` / `yvan-work` / `headless` 不会被当成可选 Profile 露出。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const REGISTRY_FILE: &str = "index.v1.json";
pub const REGISTRY_SCHEMA: &str = "omnidesk.profiles.v1";
/// 回收目录里的元数据文件名。
///
/// 目录名只带 `<name>-<epoch>-<pid>`，而 label / origin / plugins 无从反解，
/// 恢复后必须原样写回 `index.v1.json` 才能重新出现在可选清单里。
pub const TRASH_FILE: &str = "trash.v1.json";
pub const TRASH_SCHEMA: &str = "omnidesk.profile-trash.v1";

/// 由 OmniDesk 创建的自定义 Profile。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedProfile {
    pub name: String,
    pub label: String,
    /// 创建时间（Unix 秒）。
    pub created_at: u64,
    /// `standard`（从基础标准起手）或 `copy:<源 Profile>`。
    pub origin: String,
    /// 需要的插件包（`@scope/name` 或 `name@version`）。从来源 Profile 继承，
    /// 因为复制不带 `node_modules`，缺的插件要在资产页显形并能一键安装。
    #[serde(default)]
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub schema: String,
    #[serde(default)]
    pub profiles: Vec<ManagedProfile>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            schema: REGISTRY_SCHEMA.to_string(),
            profiles: Vec::new(),
        }
    }
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn registry_path(app_root: &Path) -> PathBuf {
    app_root.join("profiles").join(REGISTRY_FILE)
}

/// 读取登记表。文件缺失、损坏都按空表处理（只影响「哪些自定义 Profile 可选」，
/// 不影响 DSH 本身能否启动）。
pub fn load_registry(app_root: &Path) -> Registry {
    let path = registry_path(app_root);
    let Ok(text) = fs::read_to_string(&path) else {
        return Registry::default();
    };
    let mut parsed: Registry = serde_json::from_str(&text).unwrap_or_default();
    parsed.schema = REGISTRY_SCHEMA.to_string();
    parsed.profiles.retain(|p| crate::settings::is_valid_profile_name(&p.name));
    parsed
}

fn save_registry(app_root: &Path, registry: &Registry) -> Result<(), String> {
    let path = registry_path(app_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建 Profile 登记目录失败（{}）：{}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("序列化 Profile 登记表失败：{}", e))?;
    crate::model_settings::write_atomic(&path, text.as_bytes())?;
    Ok(())
}

/// 已登记的自定义 Profile 名（供 `profile_choice` 解析与列表使用）。
/// 写回登记表（供资产装配把新发现的插件依赖补进清单）。
pub fn save_registry_public(app_root: &Path, registry: &Registry) -> Result<(), String> {
    save_registry(app_root, registry)
}

pub fn registered_names(app_root: &Path) -> Vec<String> {
    load_registry(app_root)
        .profiles
        .into_iter()
        .map(|p| p.name)
        .collect()
}

fn ensure_free(dsh_home: &Path, app_root: &Path, name: &str) -> Result<(), String> {
    if crate::profile_assets::asset_profile(name).is_some() {
        return Err(format!("「{}」是出厂 Profile，不能同名新建。", name));
    }
    let dsh_dir = dsh_home.join("profiles").join(name);
    if dsh_dir.exists() {
        return Err(format!(
            "隔离数据根里已经有同名目录：{}。换一个名字，或先移除它。",
            dsh_dir.display()
        ));
    }
    let app_dir = app_root.join("profiles").join(name);
    if app_dir.exists() {
        return Err(format!(
            "应用数据根里已经有同名目录：{}。换一个名字，或先移除它。",
            app_dir.display()
        ));
    }
    if load_registry(app_root).profiles.iter().any(|p| p.name == name) {
        return Err(format!("「{}」已经在自定义 Profile 清单里。", name));
    }
    Ok(())
}

/// 出厂基础标准的最小配置：隔离根里没有 `omnidesk-base` 时用它起手。
const BASE_MANIFEST: &str = include_str!("../../../samples/profiles/omnidesk-base.profile.json");
const BASE_PATCH: &str = include_str!("../../../samples/profiles/omnidesk-base.profile.patch.yml");
const EMPTY_CORDIS: &str = "# dsh profile root — an empty entry list. The tree is composed as patches:\n# each bundle in package.json's dsh.profile.bundles, then cordis.patch.yml, then any\n# --patch overlays. Edit cordis.patch.yml, not this file.\n[]\n";

/// 从某个已有 Profile 复制配置文件（不复制 node_modules 与运行缓存）。
fn copy_profile_files(from: &Path, to: &Path) -> Result<Vec<String>, String> {
    fs::create_dir_all(to)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", to.display(), e))?;
    let mut copied = Vec::new();
    for name in [
        "package.json",
        "cordis.yml",
        "cordis.patch.yml",
        "pnpm-workspace.yaml",
        "pnpm-lock.yaml",
    ] {
        let src = from.join(name);
        if !src.is_file() {
            continue;
        }
        fs::copy(&src, to.join(name))
            .map_err(|e| format!("复制 {} 失败：{}", src.display(), e))?;
        copied.push(name.to_string());
    }
    if !to.join("package.json").is_file() {
        return Err(format!(
            "源 Profile 缺少 package.json，不能作为复制来源：{}",
            from.display()
        ));
    }
    Ok(copied)
}

fn write_base_files(to: &Path, dsh_home: &Path, source: Option<&str>) -> Result<Vec<String>, String> {
    match source {
        Some(from) => {
            let from_dir = dsh_home.join("profiles").join(from);
            if !from_dir.join("package.json").is_file() {
                return Err(format!("找不到要复制的 Profile：{}", from_dir.display()));
            }
            copy_profile_files(&from_dir, to)
        }
        None => {
            // 先看隔离根里真实的基础标准，缺失才退回编译期内嵌版本。
            let base = dsh_home.join("profiles").join("omnidesk-base");
            if base.join("package.json").is_file() {
                return copy_profile_files(&base, to);
            }
            fs::create_dir_all(to)
                .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", to.display(), e))?;
            fs::write(to.join("package.json"), BASE_MANIFEST)
                .map_err(|e| format!("写入 package.json 失败：{}", e))?;
            fs::write(to.join("cordis.patch.yml"), BASE_PATCH)
                .map_err(|e| format!("写入 cordis.patch.yml 失败：{}", e))?;
            fs::write(to.join("cordis.yml"), EMPTY_CORDIS)
                .map_err(|e| format!("写入 cordis.yml 失败：{}", e))?;
            Ok(vec![
                "package.json".to_string(),
                "cordis.patch.yml".to_string(),
                "cordis.yml".to_string(),
            ])
        }
    }
}

/// 从一个 Profile 目录推断它需要的插件包。
///
/// 只看两处：`package.json` 的 `dependencies`（用户用 DSH 装过的包），以及
/// `cordis.patch.yml` 里显式 `name: '@scope/pkg'` 的条目（补丁层插入的插件）。
/// 补丁里的名字大多来自 bundle、随运行时就有，逐条列出只会变成噪声，
/// 所以调用方用 `only_missing` 再过一遍本地探测。
///
/// 补丁里的 `name: '@scope/pkg/sub/entry'` 是**子路径导出**，不是独立 npm 包
/// （OD-ARCH-119）。安装通道只接受 `@scope/pkg` 两段式，把这些子路径当包名
/// 去探测，结果是「永远缺失、永远点不动」，所以这里一律归一到包根。
pub fn detect_plugins(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Ok(text) = fs::read_to_string(dir.join("package.json")) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(deps) = value.get("dependencies").and_then(|v| v.as_object()) {
                for name in deps.keys() {
                    if !out.iter().any(|n| n == name) {
                        out.push(name.clone());
                    }
                }
            }
        }
    }
    if let Ok(patch) = fs::read_to_string(dir.join("cordis.patch.yml")) {
        for line in patch.lines() {
            let trimmed = line.trim().trim_start_matches("- ").trim();
            let Some(rest) = trimmed.strip_prefix("name:") else {
                continue;
            };
            let name = rest.trim().trim_matches('"').trim_matches('\'').trim();
            let name = package_root(name);
            if name.starts_with('@') && !out.iter().any(|n| n == name) {
                out.push(name.to_string());
            }
        }
    }
    // package.json 的 dependencies 也可能写成子路径，一并归一。
    for name in out.iter_mut() {
        let root = package_root(name).to_string();
        *name = root;
    }
    out.sort();
    out.dedup();
    out
}

/// 把 `@scope/pkg/sub/entry` 折回包根 `@scope/pkg`。
///
/// 只处理 scoped 包的两段式；非 scoped 名字（含 `my-pkg/sub`）原样返回，
/// 交给调用方的合法性判断，避免把不认识的写法悄悄改写成别的包。
pub fn package_root(name: &str) -> &str {
    let trimmed = name.trim();
    if !trimmed.starts_with('@') {
        return trimmed;
    }
    let mut parts = trimmed.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(scope), Some(pkg), Some(_rest)) if !scope.is_empty() && !pkg.is_empty() => {
            // 两段式（`@scope/pkg`）第三段为空，保持原样。
            let end = scope.len() + 1 + pkg.len();
            &trimmed[..end]
        }
        _ => trimmed,
    }
}

/// 过滤出真正装不上的那些（依赖声明里的包 + 本地探测不到补丁插件）。
fn required_plugins(dir: &Path, dsh_home: &Path) -> Vec<String> {
    let cli = crate::profile_assets::default_dsh_cli();
    let deps = fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("dependencies").and_then(|d| d.as_object().cloned()))
        .map(|m| m.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    detect_plugins(dir)
        .into_iter()
        .filter(|name| {
            if deps.iter().any(|d| d == name) {
                return true;
            }
            // 补丁里提到、但本地解析得到的包（随运行时自带）不算需要安装
            !crate::profile_assets::package_available(dsh_home, name, &cli)
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateReceipt {
    pub ok: bool,
    pub profile: ManagedProfile,
    pub directory: String,
    pub files: Vec<String>,
    pub note: String,
}

/// 新建 / 复制一个自定义 Profile。
///
/// `copy_from = None` 表示从基础标准起手；`Some(name)` 表示复制那个 Profile 的配置。
/// 复制只带配置与场景/规则资产，不带 `node_modules`：插件缺失会在资产页显形。
pub fn create(
    dsh_home: &Path,
    app_root: &Path,
    name: &str,
    label: &str,
    copy_from: Option<&str>,
) -> Result<CreateReceipt, String> {
    crate::capabilities::assert_writable_root(dsh_home)?;
    let name = name.trim();
    if !crate::settings::is_valid_profile_name(name) {
        return Err(format!(
            "Profile 名不合法：{:?}（只允许小写字母、数字、连字符、下划线，最多 48 字符）",
            name
        ));
    }
    let label = label.trim();
    if label.is_empty() || label.chars().count() > 32 {
        return Err("显示名要填 1–32 个字。".to_string());
    }
    ensure_free(dsh_home, app_root, name)?;
    let source = copy_from.map(str::trim).filter(|s| !s.is_empty());
    if let Some(from) = source {
        if from == name {
            return Err("不能复制到自己。".to_string());
        }
    }

    let dsh_dir = dsh_home.join("profiles").join(name);
    let mut files = match write_base_files(&dsh_dir, dsh_home, source) {
        Ok(files) => files,
        Err(e) => {
            let _ = fs::remove_dir_all(&dsh_dir);
            return Err(e);
        }
    };
    // 从基础标准起手时，起步骨架写失败不阻断创建，但要在回执里说清楚。
    let mut starter_error: Option<String> = None;

    // 复制来源的场景声明、规则包与分类表（自定义 Profile 才走这条；出厂装配另有清单）。
    //
    // 分类表也要带（OD-ARCH-117）：它现在是 Profile 资产，复制一个新 Profile 却不带
    // 分类，切过去时只能走「继承当前表」那条兜底——用户既然是从某套工作台复制的，
    // 就该拿到那套的分类，而不是靠「当时恰好切到哪个 Profile」决定。
    // 失败不阻断创建（与 decl/rules 同口径）：新 Profile 大不了走「继承一次」。
    let app_dir = app_root.join("profiles").join(name);
    if let Some(from) = source {
        let from_app = app_root.join("profiles").join(from);
        for file in ["decl.yml", "rules.json"] {
            if from_app.join(file).is_file() {
                let _ = fs::create_dir_all(&app_dir);
                let _ = fs::copy(from_app.join(file), app_dir.join(file));
            }
        }
        // 来源正在生效时，盘上的 categories.json 可能是上次切走时的旧快照
        // （用户在当前 Profile 里改过分类、还没切走就不会回存）。此时以当前生效设置
        // 为准，否则复制出来的分类表会比用户看到的少一截。
        let live = crate::settings::load(&app_root.join("config"))
            .ok()
            .filter(|s| s.engine_profile.as_deref() == Some(from))
            .map(|s| s.thread_categories);
        if let Some(categories) = live
            .or_else(|| crate::profile_categories::load(app_root, from).ok().flatten())
            .or_else(|| {
                crate::profile_categories::embedded_for(from)
                    .and_then(|raw| crate::profile_categories::parse(raw, from).ok())
            })
        {
            let _ = crate::profile_categories::save(app_root, name, &categories);
        }
    } else {
        // 从基础标准起手（OD-ARCH-122 缺口 B）：没有来源可复制，给一份最小工作台骨架，
        // 否则切过去只有平台固有入口，用户得从零写一整份声明。
        // 写失败不阻断创建（与复制路径同口径），但要在回执里如实说，不能假装写成功了。
        match crate::profile_starter::write_starter(app_root, name, label) {
            Ok(written) => {
                for file in written {
                    if !files.iter().any(|f| f == &file) {
                        files.push(file);
                    }
                }
            }
            Err(e) => starter_error = Some(e),
        }
    }

    let profile = ManagedProfile {
        name: name.to_string(),
        label: label.to_string(),
        created_at: now_epoch(),
        origin: match source {
            Some(from) => format!("copy:{}", from),
            None => "standard".to_string(),
        },
        plugins: required_plugins(&dsh_dir, dsh_home),
    };
    let mut registry = load_registry(app_root);
    registry.profiles.push(profile.clone());
    save_registry(app_root, &registry)?;

    Ok(CreateReceipt {
        ok: true,
        profile,
        directory: dsh_dir.display().to_string(),
        files,
        note: if source.is_some() {
            "已复制配置与场景规则；插件需在「Profile 资产」页按需安装。切到该 Profile 后立即生效（切换会停掉旧引擎）。"
                .to_string()
        } else if let Some(e) = starter_error {
            format!(
                "已从基础标准起手，但起步骨架没写成（{}）。切过去后可在设置 →「对话分类 → 场景声明」里填入模板，或手工填写。",
                e
            )
        } else {
            "已从基础标准起手，并写入一份最小场景声明与空规则包（零预注入）。切到该 Profile 后立即生效（切换会停掉旧引擎）。"
                .to_string()
        },
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteReceipt {
    pub ok: bool,
    pub name: String,
    pub trashed_dsh: Option<String>,
    pub trashed_app: Option<String>,
    pub note: String,
}

/// 回收目录里的元数据（OD-ARCH-125）。
///
/// 目录名只带 `<name>-<epoch>-<pid>`，label / origin / plugins 无从反解；
/// 恢复后要原样写回登记表，用户看到的显示名与插件清单才不会丢。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashMeta {
    pub schema: String,
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default)]
    pub trashed_at: u64,
}

/// 回收站里的一项（只读巡检结果）。
#[derive(Debug, Clone, Serialize)]
pub struct TrashEntry {
    /// 回收目录名，恢复时原样传回。
    pub id: String,
    pub name: String,
    pub label: String,
    pub origin: String,
    pub trashed_at: u64,
    /// 配置目录还在（`dsh_home/profiles/.trash/<id>`）。
    pub has_dsh: bool,
    /// 应用资产目录还在（`app_root/profiles/.trash/<id>`）。
    pub has_app: bool,
    /// 元数据是否完整（缺失时按目录名推算，只恢复名字与登记）。
    pub has_meta: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreReceipt {
    pub ok: bool,
    pub profile: ManagedProfile,
    pub directory: String,
    pub note: String,
}

fn trash_dir(root: &Path) -> PathBuf {
    root.join("profiles").join(".trash")
}

fn write_trash_meta(dir: &Path, meta: &TrashMeta) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("创建回收目录失败（{}）：{}", dir.display(), e))?;
    let text = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("序列化回收元数据失败：{}", e))?;
    crate::model_settings::write_atomic(&dir.join(TRASH_FILE), text.as_bytes())
}

/// 从目录名 `name-epoch-pid` 反解出原 Profile 名（元数据缺失时的兜底）。
///
/// 名字本身可以含 `-`，所以只从右往左切两段；切出来的两段必须都是纯数字，
/// 否则宁可认为这不是回收目录，也不猜一个错名字出来。
fn name_from_trash_id(id: &str) -> Option<String> {
    let mut parts = id.rsplit('-');
    let pid = parts.next()?;
    let epoch = parts.next()?;
    if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if epoch.is_empty() || !epoch.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let name = parts.collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("-");
    crate::settings::is_valid_profile_name(&name).then_some(name)
}

fn read_trash_meta(dir: &Path, id: &str) -> Option<TrashMeta> {
    let text = fs::read_to_string(dir.join(TRASH_FILE)).ok()?;
    let mut meta: TrashMeta = serde_json::from_str(&text).ok()?;
    if !crate::settings::is_valid_profile_name(&meta.name) {
        // 元数据坏了就当作没有，退回目录名推算。
        return None;
    }
    if meta.label.trim().is_empty() {
        meta.label = meta.name.clone();
    }
    if meta.origin.trim().is_empty() {
        meta.origin = "restored".to_string();
    }
    let _ = id;
    Some(meta)
}

/// 只读：列出回收站里可恢复的 Profile，按移除时间倒序。
///
/// 以应用数据根的 `.trash` 为准（移除时两个目录共用同一个 id）；同时也收
/// 隔离根里只剩配置目录的孤儿项，避免旧版本留下的条目永远看不见。
pub fn list_trash(app_root: &Path, dsh_home: &Path) -> Vec<TrashEntry> {
    let mut ids: Vec<String> = Vec::new();
    for dir in [trash_dir(app_root), trash_dir(dsh_home)] {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if !kind.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().to_string();
            if !ids.iter().any(|x| x == &id) {
                ids.push(id);
            }
        }
    }

    let mut out: Vec<TrashEntry> = Vec::new();
    for id in ids {
        let app_dir = trash_dir(app_root).join(&id);
        let dsh_dir = trash_dir(dsh_home).join(&id);
        let meta = read_trash_meta(&app_dir, &id).or_else(|| read_trash_meta(&dsh_dir, &id));
        let Some(name) = meta
            .as_ref()
            .map(|m| m.name.clone())
            .or_else(|| name_from_trash_id(&id))
        else {
            continue;
        };
        let has_dsh = dsh_dir.is_dir();
        let has_app = app_dir.is_dir();
        let has_meta = meta.is_some();
        let trashed_at = meta
            .as_ref()
            .map(|m| m.trashed_at)
            .filter(|v| *v > 0)
            .or_else(|| {
                let mut parts = id.rsplit('-');
                let _ = parts.next();
                parts.next().and_then(|v| v.parse::<u64>().ok())
            })
            .unwrap_or(0);
        out.push(TrashEntry {
            label: meta
                .as_ref()
                .map(|m| m.label.clone())
                .unwrap_or_else(|| name.clone()),
            origin: meta
                .as_ref()
                .map(|m| m.origin.clone())
                .unwrap_or_else(|| "restored".to_string()),
            has_meta,
            note: match (has_meta, has_dsh, has_app) {
                (true, true, true) => "配置与应用资产都在回收目录里，可以恢复。".to_string(),
                (true, _, _) if !has_dsh && !has_app => {
                    "只剩登记信息；恢复后目录需重新装配。".to_string()
                }
                (true, _, _) => "只有部分目录在回收目录里，恢复后缺失的部分要重新装配。".to_string(),
                (false, _, _) => {
                    "没有元数据，只能按目录名恢复名字与登记；显示名与插件清单需重新填写。".to_string()
                }
            },
            name,
            trashed_at,
            has_dsh,
            has_app,
            id,
        });
    }
    out.sort_by(|a, b| b.trashed_at.cmp(&a.trashed_at).then_with(|| a.id.cmp(&b.id)));
    out
}

/// 把回收目录里的一项恢复成可用 Profile（OD-ARCH-125）。
///
/// 只做本地的两次 `rename` 加一次登记写回；不联网、不装插件、不覆盖任何
/// 已存在的目录或登记项。任一步失败都会把已经搬过的目录搬回去。
pub fn restore(dsh_home: &Path, app_root: &Path, id: &str) -> Result<RestoreReceipt, String> {
    crate::capabilities::assert_writable_root(dsh_home)?;
    let id = id.trim();
    if id.is_empty()
        || id.contains('/')
        || id.contains('\\')
        || id == "."
        || id == ".."
    {
        return Err(format!("回收目录标识不合法：{:?}", id));
    }
    let app_trash = trash_dir(app_root).join(id);
    let dsh_trash = trash_dir(dsh_home).join(id);
    if !app_trash.is_dir() && !dsh_trash.is_dir() {
        return Err(format!("回收站里找不到「{}」，可能已经恢复或已被清理。", id));
    }
    let meta = read_trash_meta(&app_trash, id).or_else(|| read_trash_meta(&dsh_trash, id));
    let name = meta
        .as_ref()
        .map(|m| m.name.clone())
        .or_else(|| name_from_trash_id(id))
        .ok_or_else(|| format!("无法从「{}」还原 Profile 名，请手动处理回收目录。", id))?;

    if crate::profile_assets::asset_profile(&name).is_some()
        || crate::profile_choice::is_whitelisted("web-api", &name)
    {
        return Err(format!("「{}」是出厂 Profile 名，不能从这里恢复。", name));
    }
    let mut registry = load_registry(app_root);
    if registry.profiles.iter().any(|p| p.name == name) {
        return Err(format!("「{}」已经在自定义 Profile 清单里，不用恢复。", name));
    }
    let dsh_dest = dsh_home.join("profiles").join(&name);
    let app_dest = app_root.join("profiles").join(&name);
    if dsh_dest.exists() || app_dest.exists() {
        return Err(format!(
            "已经有同名目录，未做任何改动：{}",
            if app_dest.exists() { app_dest } else { dsh_dest }.display()
        ));
    }

    let meta_view = meta.clone().unwrap_or(TrashMeta {
        schema: TRASH_SCHEMA.to_string(),
        label: name.clone(),
        name: name.clone(),
        created_at: 0,
        origin: "restored".to_string(),
        plugins: Vec::new(),
        trashed_at: 0,
    });

    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    fn step(
        moved: &mut Vec<(PathBuf, PathBuf)>,
        from: PathBuf,
        to: PathBuf,
    ) -> Result<(), String> {
        fs::rename(&from, &to).map_err(|e| format!("恢复 {} 失败：{}", from.display(), e))?;
        moved.push((from, to));
        Ok(())
    }
    let result = (|| -> Result<(), String> {
        if dsh_trash.is_dir() {
            step(&mut moved, dsh_trash.clone(), dsh_dest.clone())?;
        }
        if app_trash.is_dir() {
            step(&mut moved, app_trash.clone(), app_dest.clone())?;
            // 元数据只是回收目录的附属物，恢复后不该留在 Profile 目录里。
            let _ = fs::remove_file(app_dest.join(TRASH_FILE));
        }
        Ok(())
    })();
    if let Err(e) = result {
        for (from, to) in moved.iter().rev() {
            let _ = fs::rename(to, from);
        }
        return Err(e);
    }

    let profile = ManagedProfile {
        name: name.clone(),
        label: meta_view.label.clone(),
        created_at: if meta_view.created_at > 0 {
            meta_view.created_at
        } else {
            now_epoch()
        },
        origin: meta_view.origin.clone(),
        plugins: meta_view.plugins.clone(),
    };
    registry.profiles.push(profile.clone());
    if let Err(e) = save_registry(app_root, &registry) {
        // 登记写不回去，把目录搬回回收站，保持「要么全恢复、要么没动过」。
        for (from, to) in moved.iter().rev() {
            let _ = fs::rename(to, from);
        }
        return Err(e);
    }

    Ok(RestoreReceipt {
        ok: true,
        directory: dsh_dest.display().to_string(),
        profile,
        note: "已从回收站恢复，并重新登记为可选 Profile。切过去即生效（切换会停掉旧引擎）。".to_string(),
    })
}

/// 移除自定义 Profile：移入 `.trash/<name>-<epoch>/`，不做不可恢复的删除。
pub fn remove(
    dsh_home: &Path,
    app_root: &Path,
    name: &str,
    current: Option<&str>,
) -> Result<DeleteReceipt, String> {
    crate::capabilities::assert_writable_root(dsh_home)?;
    let name = name.trim();
    if !crate::settings::is_valid_profile_name(name) {
        return Err(format!("Profile 名不合法：{:?}", name));
    }
    if crate::profile_assets::asset_profile(name).is_some()
        || crate::profile_choice::is_whitelisted("web-api", name)
    {
        return Err(format!("「{}」是出厂 Profile，不能移除。", name));
    }
    if current == Some(name) {
        return Err("这是当前选中的 Profile，请先切到其他 Profile 再移除。".to_string());
    }
    let mut registry = load_registry(app_root);
    let Some(managed) = registry.profiles.iter().find(|p| p.name == name).cloned() else {
        return Err(format!("「{}」不在自定义 Profile 清单里。", name));
    };
    let stamp = format!("{}-{}", now_epoch(), std::process::id());
    let trash_id = format!("{}-{}", name, stamp);
    let mut trashed_dsh = None;
    let mut trashed_app = None;

    let dsh_dir = dsh_home.join("profiles").join(name);
    if dsh_dir.exists() {
        let dest = dsh_home.join("profiles").join(".trash").join(&trash_id);
        fs::create_dir_all(dest.parent().unwrap())
            .map_err(|e| format!("创建回收目录失败：{}", e))?;
        fs::rename(&dsh_dir, &dest)
            .map_err(|e| format!("移入回收目录失败（{}）：{}", dsh_dir.display(), e))?;
        trashed_dsh = Some(dest.display().to_string());
    }
    let app_dir = app_root.join("profiles").join(name);
    if app_dir.exists() {
        let dest = app_root.join("profiles").join(".trash").join(&trash_id);
        fs::create_dir_all(dest.parent().unwrap())
            .map_err(|e| format!("创建回收目录失败：{}", e))?;
        fs::rename(&app_dir, &dest)
            .map_err(|e| format!("移入回收目录失败（{}）：{}", app_dir.display(), e))?;
        trashed_app = Some(dest.display().to_string());
    }

    // 两个目录都搬好了再写元数据：目录名反解不出显示名与插件清单，
    // 恢复时要靠这份文件把登记原样写回去（OD-ARCH-125）。
    // 写失败不阻断移除——没有元数据仍能按目录名恢复，只是显示名会退回名字。
    let meta = TrashMeta {
        schema: TRASH_SCHEMA.to_string(),
        name: managed.name.clone(),
        label: managed.label.clone(),
        created_at: managed.created_at,
        origin: managed.origin.clone(),
        plugins: managed.plugins.clone(),
        trashed_at: now_epoch(),
    };
    if let Some(path) = trashed_app.as_deref().map(PathBuf::from) {
        let _ = write_trash_meta(&path, &meta);
    }
    if let Some(path) = trashed_dsh.as_deref().map(PathBuf::from) {
        let _ = write_trash_meta(&path, &meta);
    }
    registry.profiles.retain(|p| p.name != name);
    save_registry(app_root, &registry)?;

    Ok(DeleteReceipt {
        ok: true,
        name: name.to_string(),
        trashed_dsh,
        trashed_app,
        note: "已移入回收站（不是永久删除），可在「Profile 资产」页的回收站里一键恢复。"
            .to_string(),
    })
}

/// 改名的回执（OD-ARCH-126）。
#[derive(Debug, Clone, Serialize)]
pub struct RenameReceipt {
    pub ok: bool,
    /// 改名后的登记项（名字、来源、插件清单都没变，只有 label 变）。
    pub profile: ManagedProfile,
    pub previous_label: String,
    /// 是否同时改了场景声明里的 display_name。
    pub decl_updated: bool,
    /// 声明改写前留下的备份文件（没改声明时为 None）。
    pub backup: Option<String>,
    pub note: String,
}

/// 把显示名压成一行、去掉首尾空白。
///
/// 与 create 同口径（1–32 字）；换行和制表符会破坏声明文件的单行键值，
/// 直接压成空格而不是拒绝——用户粘贴多行文字时不该看到一串报错。
fn clean_display_label(label: &str) -> String {
    let single: String = label
        .chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .collect();
    single.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 生成声明里的 display_name 行。
///
/// 声明是受限 YAML 子集：`#` 会被当成行尾注释切开，`"` 没有转义写法。
/// 这两种字符出现在显示名里就明确拒绝，不写一份自己都解析不回来的文件。
fn display_name_line(label: &str) -> Result<String, String> {
    if label.contains(['"', '#']) {
        return Err("显示名里不能有英文双引号或 # 号。".to_string());
    }
    if label.contains('\'') {
        return Ok(format!("display_name: \"{}\"", label));
    }
    Ok(format!("display_name: {}", label))
}

/// 只替换声明里顶层的 display_name 那一行，其余字节原样保留。
///
/// 逐行改写而不是整份重排版：用户手写的注释、缩进、字段顺序都留着，
/// 改名字不该顺手把别人的文件重排一遍。替换完再走一次真实解析校验。
fn with_display_name(text: &str, label: &str) -> Result<String, String> {
    let replacement = display_name_line(label)?;
    let mut out = String::with_capacity(text.len() + replacement.len());
    let mut replaced = false;
    let ends_with_newline = text.ends_with('\n');
    let lines: Vec<&str> = text.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let is_top_level = !line.starts_with(' ') && !line.trim_start().starts_with('#');
        let is_display_name = is_top_level
            && line
                .split_once(':')
                .map(|(key, _)| key.trim() == "display_name")
                .unwrap_or(false);
        if is_display_name {
            if replaced {
                return Err("声明里有重复的 display_name，请先在设置页手工改好".to_string());
            }
            out.push_str(&replacement);
            replaced = true;
        } else {
            out.push_str(line);
        }
        if idx + 1 < lines.len() || ends_with_newline {
            out.push('\n');
        }
    }
    if !replaced {
        return Err("声明里没有 display_name 行，未做任何改动".to_string());
    }
    Ok(out)
}

/// 改自定义 Profile 的显示名（OD-ARCH-126）。
///
/// 只动两处「显示名真身」：登记表的 label 与场景声明的 display_name。
/// 目录名、--profile 标识、规则包里的 profile 字段、Skill 绑定与插件清单一律不动。
///
/// 顺序是「先声明、后登记」：声明解析或校验失败就整体拒绝；登记表写不回去时
/// 用备份把声明还原，不留一个改了一半的名字。
pub fn rename_label(app_root: &Path, name: &str, label: &str) -> Result<RenameReceipt, String> {
    let name = name.trim();
    if !crate::settings::is_valid_profile_name(name) {
        return Err(format!("Profile 名不合法：{:?}", name));
    }
    if crate::profile_assets::asset_profile(name).is_some()
        || crate::profile_choice::is_whitelisted("web-api", name)
    {
        return Err(format!(
            "「{}」是出厂 Profile，显示名由程序决定，不能在软件里改名。",
            name
        ));
    }
    let label = clean_display_label(label);
    if label.is_empty() || label.chars().count() > 32 {
        return Err("显示名要填 1–32 个字。".to_string());
    }

    let mut registry = load_registry(app_root);
    let Some(idx) = registry.profiles.iter().position(|p| p.name == name) else {
        return Err(format!(
            "「{}」不在自定义 Profile 清单里，未做任何改动。",
            name
        ));
    };
    let previous_label = registry.profiles[idx].label.clone();
    if previous_label == label {
        return Ok(RenameReceipt {
            ok: true,
            profile: registry.profiles[idx].clone(),
            previous_label,
            decl_updated: false,
            backup: None,
            note: "显示名没有变化，未改动任何文件。".to_string(),
        });
    }

    // 声明存在就先算出新文本并自校验；任何一步不通过都整体拒绝，登记表不动。
    let app_dir = app_root.join("profiles").join(name);
    let decl_path = crate::profile_decl::decl_path(&app_dir);
    let mut pending: Option<(Vec<u8>, String)> = None;
    if decl_path.is_file() {
        let original = fs::read(&decl_path)
            .map_err(|e| format!("读取 Profile 声明失败（{}）：{}", decl_path.display(), e))?;
        if original.len() > 64 * 1024 {
            return Err(format!(
                "声明文件超过 64 KB（{} 字节），请先用系统编辑器处理：{}",
                original.len(),
                decl_path.display()
            ));
        }
        let text = String::from_utf8(original.clone())
            .map_err(|_| "声明文件不是 UTF-8 文本，未做任何改动".to_string())?;
        let parsed = crate::profile_decl::parse_decl(&text)
            .map_err(|e| format!("声明解析失败，未做任何改动：{}", e))?;
        parsed
            .validate()
            .map_err(|e| format!("声明校验失败，未做任何改动：{}", e))?;
        if parsed.display_name != label {
            let next = with_display_name(&text, &label)?;
            let check = crate::profile_decl::parse_decl(&next)
                .map_err(|e| format!("改名后的声明解析失败，未做任何改动：{}", e))?;
            check
                .validate()
                .map_err(|e| format!("改名后的声明校验失败，未做任何改动：{}", e))?;
            if check.display_name != label {
                return Err("改名后的声明回读不一致，未做任何改动。".to_string());
            }
            pending = Some((original, next));
        }
    }

    // 先写声明（留一份 .bak），再写登记表。
    let mut backup: Option<PathBuf> = None;
    if let Some((original, next)) = &pending {
        let target = decl_path.with_file_name(format!(
            "{}.bak-{}",
            crate::profile_decl::PROFILE_DECL_FILE,
            now_epoch()
        ));
        fs::write(&target, original)
            .map_err(|e| format!("写入前备份失败（{}）：{}", target.display(), e))?;
        if let Err(e) = crate::model_settings::write_atomic(&decl_path, next.as_bytes()) {
            let _ = fs::write(&decl_path, original);
            let _ = fs::remove_file(&target);
            return Err(e);
        }
        backup = Some(target);
    }

    registry.profiles[idx].label = label.clone();
    if let Err(e) = save_registry(app_root, &registry) {
        if let Some((original, _)) = &pending {
            let _ = crate::model_settings::write_atomic(&decl_path, original);
        }
        if let Some(path) = &backup {
            let _ = fs::remove_file(path);
        }
        return Err(format!("{}（场景声明已还原，显示名保持原样）", e));
    }

    let profile = registry.profiles[idx].clone();
    let decl_updated = pending.is_some();
    Ok(RenameReceipt {
        ok: true,
        profile,
        previous_label,
        decl_updated,
        backup: backup.map(|p| p.display().to_string()),
        note: if decl_updated {
            "显示名已更新（登记表与场景声明），目录名与绑定没有变。".to_string()
        } else {
            "显示名已更新；这个 Profile 没有场景声明，所以只改了登记表。".to_string()
        },
    })
}

// ---------------------------------------------------------------------------
// Profile 迁移包（OD-ARCH-127）
//
// 产品口径：OmniDesk 是干净底座，Profile 是定制工作台。一套调好的工作台
// 不该只活在一台机器的两个目录里，但也不能靠整盘拷数据根搬走——那会把会话、
// 缓存与凭证引用一起带出去。
//
// 边界：
//   * 包是**单文件 JSON**，明文可审阅、可手工抢救；不引入归档依赖。
//   * 只收白名单配置文件；`node_modules`、`.bak-*`、回收元数据、装配状态一律不收。
//   * 导入**先校验、后落盘**：schema、路径、摘要、名字、声明、规则、分类全部
//     通过才写第一个字节；任一失败磁盘零变化。
//   * 同名一律拒绝、绝不覆盖：合并两套工作台资产是不可逆操作。
//   * 导入路径不联网；插件照旧由「切换装配 + 缺件补装」这条既有通道安装。
// ---------------------------------------------------------------------------

/// 迁移包的 schema 标识。版本不符一律拒绝。
pub const BUNDLE_SCHEMA: &str = "omnidesk.profile-bundle.v1";
/// 包文件扩展名（导出时用作默认后缀）。
pub const BUNDLE_EXT: &str = "omniprofile.json";
/// 单个包的大小上限：全是 KB 级文本，2 MB 已经宽到能装下异常大的规则包。
const BUNDLE_MAX_BYTES: u64 = 2 * 1024 * 1024;
/// 单个成员文件的大小上限，防止用一份巨型规则文件撑爆界面。
const BUNDLE_FILE_MAX_BYTES: u64 = 1024 * 1024;
/// 成员文件数量上限：八份配置 + 自带 Skill 的正文（OD-ARCH-131），留足余量。
const BUNDLE_FILE_MAX_COUNT: usize = 256;

/// DSH 侧 `profiles/<name>/` 下允许进包的配置文件名。
const DSH_BUNDLE_FILES: &[&str] = &[
    "package.json",
    "cordis.yml",
    "cordis.patch.yml",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
];

/// 应用侧 `profiles/<name>/` 下允许进包的资产文件名。
const APP_BUNDLE_FILES: &[&str] = &["decl.yml", "rules.json", "categories.json"];

/// 应用侧 `profiles/<name>/skills/` 下的自带 Skill（OD-ARCH-131）。
///
/// 这一类的 `path` 是多段的：`<skill名>/<相对路径>`，逐段校验。
const SKILL_BUNDLE_ROOT: &str = "skill";
/// 一个 Skill 目录里最多收多少文件，挡住异常大的目录把包撑爆。
const SKILL_FILE_MAX_COUNT: usize = 64;

/// 包里的一个成员文件。`text` 而不是二进制：这些资产全是 UTF-8 文本。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFile {
    /// 单段文件名（不含目录）。导入时按白名单与这一段校验。
    pub path: String,
    /// `dsh` 或 `app`：落到哪个数据根。
    pub root: String,
    pub sha256: String,
    pub bytes: u64,
    pub text: String,
}

/// 包内登记的 Profile 元数据（与 `index.v1.json` 的登记项同形）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleProfile {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub plugins: Vec<String>,
}

/// 迁移包本体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub schema: String,
    pub exported_at: u64,
    #[serde(default)]
    pub app_version: String,
    pub profile: BundleProfile,
    pub files: Vec<BundleFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportReceipt {
    pub ok: bool,
    pub profile: String,
    pub label: String,
    pub path: String,
    pub bytes: u64,
    pub files: Vec<String>,
    /// 需要在新机器上安装的插件包（只是清单，不联网）。
    pub plugins: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReceipt {
    pub ok: bool,
    pub profile: ManagedProfile,
    pub directory: String,
    pub files: Vec<String>,
    /// 包内声明的插件；界面据此显示「需要安装 N 个」。
    pub plugins: Vec<String>,
    /// 导入时改了名字的情况下，原包里写的名字。
    pub original_name: Option<String>,
    pub note: String,
}

/// 导出目录里的一个迁移包（只读清单，供「一键导入」用）。
#[derive(Debug, Clone, Serialize)]
pub struct BundleListing {
    pub file: String,
    pub path: String,
    pub profile: String,
    pub label: String,
    pub exported_at: u64,
    pub bytes: u64,
}

/// 列出应用数据根 exports 目录里能识别的迁移包。**只读**：不建目录、不改文件。
///
/// 只认 .omniprofile.json 且 schema 匹配的文件；坏包跳过而不是报错，
/// 这样用户手动删改导出目录不会把整个区块变成一条错误。
pub fn list_exports(app_root: &Path) -> Vec<BundleListing> {
    let dir = app_root.join("exports");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<BundleListing> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !path.is_file() || !name.ends_with(".omniprofile.json") || name.starts_with('.') {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() > BUNDLE_MAX_BYTES {
            continue;
        }
        let Ok(raw) = fs::read(&path) else { continue };
        let Ok(bundle) = serde_json::from_slice::<Bundle>(&raw) else {
            continue;
        };
        if bundle.schema != BUNDLE_SCHEMA {
            continue;
        }
        out.push(BundleListing {
            file: name,
            path: path.display().to_string(),
            profile: bundle.profile.name.clone(),
            label: bundle.profile.label.clone(),
            exported_at: bundle.exported_at,
            bytes: meta.len(),
        });
    }
    out.sort_by(|a, b| b.exported_at.cmp(&a.exported_at).then(a.file.cmp(&b.file)));
    out
}

fn bundle_file_name(name: &str) -> String {
    let stamp = {
        // 无 chrono：用 Unix 秒自己拆出年月日时分秒，误差只影响文件名可读性。
        let secs = now_epoch() as i64;
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            y,
            m,
            d,
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        )
    };
    format!("{}-{}.{}", name, stamp, BUNDLE_EXT)
}

/// 由「1970-01-01 起的天数」换算公历年月日（Howard Hinnant 的算法）。
/// 只用来给导出文件起名，不参与任何业务判断。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 收集应用侧 `profiles/<name>/skills/` 下的自带 Skill 文件（OD-ARCH-131）。
///
/// 只收文本文件、只收 `SKILL.md` 存在的目录；跳过隐藏项与 `node_modules`。
/// 路径统一写成 `<skill名>/<相对路径>`，导入侧按同一形状还原。
fn collect_skill_files(app_dir: &Path) -> Result<Vec<BundleFile>, String> {
    let root = crate::profile_rules::skill_source_root(app_dir);
    let names = crate::profile_rules::local_skills(app_dir);
    let mut out: Vec<BundleFile> = Vec::new();
    for name in names {
        let dir = root.join(&name);
        let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
        collect_skill_dir(&dir, Path::new(""), &mut pairs)?;
        if pairs.len() > SKILL_FILE_MAX_COUNT {
            return Err(format!(
                "Skill「{}」有 {} 个文件，超过上限 {}，未导出。",
                name,
                pairs.len(),
                SKILL_FILE_MAX_COUNT
            ));
        }
        for (path, rel) in pairs {
            let rel_text = rel.to_string_lossy().replace('\\', "/");
            let bundle_path = format!("{}/{}", name, rel_text);
            let bytes = fs::read(&path)
                .map_err(|e| format!("读取 {} 失败：{}", path.display(), e))?;
            if bytes.len() as u64 > BUNDLE_FILE_MAX_BYTES {
                return Err(format!(
                    "{} 超过单个文件上限（{} 字节），未导出。",
                    bundle_path,
                    BUNDLE_FILE_MAX_BYTES
                ));
            }
            let text = String::from_utf8(bytes.clone()).map_err(|e| {
                format!("{} 不是 UTF-8 文本，未导出：{}", bundle_path, e)
            })?;
            out.push(BundleFile {
                path: bundle_path,
                root: SKILL_BUNDLE_ROOT.to_string(),
                sha256: crate::sha256::sha256_hex(&bytes),
                bytes: bytes.len() as u64,
                text,
            });
        }
    }
    Ok(out)
}

/// 递归收集一个 Skill 目录里的普通文件；跳过隐藏项与 `node_modules`。
fn collect_skill_dir(
    dir: &Path,
    rel: &Path,
    out: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), String> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("读取 {} 失败：{}", dir.display(), e)),
    };
    let mut listed: Vec<_> = entries.flatten().collect();
    listed.sort_by_key(|e| e.file_name());
    for entry in listed {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let path = entry.path();
        let next = rel.join(&name);
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            collect_skill_dir(&path, &next, out)?;
        } else if meta.is_file() {
            out.push((path, next));
        }
    }
    Ok(())
}

/// 收集一个 Profile 的可迁移文件；不存在的一律跳过（一个 Profile 本来就可能没有声明）。
fn collect_bundle_files(
    dsh_home: &Path,
    app_root: &Path,
    name: &str,
) -> Result<Vec<BundleFile>, String> {
    let mut out: Vec<BundleFile> = Vec::new();
    let dsh_dir = dsh_home.join("profiles").join(name);
    let app_dir = app_root.join("profiles").join(name);
    for (root, dir, names) in [
        ("dsh", &dsh_dir, DSH_BUNDLE_FILES),
        ("app", &app_dir, APP_BUNDLE_FILES),
    ] {
        for file in names {
            let path = dir.join(file);
            if !path.is_file() {
                continue;
            }
            let bytes = fs::read(&path)
                .map_err(|e| format!("读取 {} 失败：{}", path.display(), e))?;
            if bytes.len() as u64 > BUNDLE_FILE_MAX_BYTES {
                return Err(format!(
                    "{} 超过单个文件上限（{} 字节），未导出。",
                    file, BUNDLE_FILE_MAX_BYTES
                ));
            }
            let text = String::from_utf8(bytes.clone())
                .map_err(|e| format!("{} 不是 UTF-8 文本，未导出：{}", file, e))?;
            out.push(BundleFile {
                path: file.to_string(),
                root: root.to_string(),
                sha256: crate::sha256::sha256_hex(&bytes),
                bytes: bytes.len() as u64,
                text,
            });
        }
    }
    out.extend(collect_skill_files(&app_dir)?);
    Ok(out)
}

/// 导出一个自定义 Profile（OD-ARCH-127）。
///
/// 只允许已登记的自定义 Profile：出厂 Profile 的资产来自编译期清单，
/// 未登记目录是历史遗留，两者导出都没有意义。`dest_dir` 为 `None` 时
/// 落到 `<应用数据根>/exports/`。
pub fn export_bundle(
    dsh_home: &Path,
    app_root: &Path,
    name: &str,
    dest_dir: Option<&Path>,
) -> Result<ExportReceipt, String> {
    let name = name.trim();
    if !crate::settings::is_valid_profile_name(name) {
        return Err(format!("Profile 名不合法：{:?}", name));
    }
    if crate::profile_assets::asset_profile(name).is_some()
        || crate::profile_choice::is_whitelisted("web-api", name)
    {
        return Err(format!(
            "「{}」是出厂 Profile，资产来自程序内清单，不需要导出来搬走。",
            name
        ));
    }
    let registry = load_registry(app_root);
    let Some(entry) = registry.profiles.iter().find(|p| p.name == name) else {
        return Err(format!(
            "「{}」不在自定义 Profile 清单里，未导出（历史遗留目录不参与迁移）。",
            name
        ));
    };

    let files = collect_bundle_files(dsh_home, app_root, name)?;
    if files.is_empty() {
        return Err(format!(
            "「{}」在磁盘上没有任何可导出的配置文件：先切过去让软件装配一次，再导出。",
            name
        ));
    }
    if !files
        .iter()
        .any(|f| f.root == "dsh" && f.path == "package.json")
    {
        return Err(format!(
            "「{}」缺少 package.json，不是一套完整的 Profile，未导出。",
            name
        ));
    }

    let bundle = Bundle {
        schema: BUNDLE_SCHEMA.to_string(),
        exported_at: now_epoch(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        profile: BundleProfile {
            name: entry.name.clone(),
            label: entry.label.clone(),
            created_at: entry.created_at,
            origin: entry.origin.clone(),
            plugins: entry.plugins.clone(),
        },
        files,
    };
    let bytes = serde_json::to_vec_pretty(&bundle)
        .map_err(|e| format!("打包失败：{}", e))?;

    let dir = match dest_dir {
        Some(d) => d.to_path_buf(),
        None => app_root.join("exports"),
    };
    fs::create_dir_all(&dir).map_err(|e| format!("创建导出目录失败（{}）：{}", dir.display(), e))?;
    let target = dir.join(bundle_file_name(name));
    crate::model_settings::write_atomic(&target, &bytes)
        .map_err(|e| format!("写入迁移包失败（{}）：{}", target.display(), e))?;

    let mut listed: Vec<String> = bundle
        .files
        .iter()
        .map(|f| format!("{}/{}", f.root, f.path))
        .collect();
    listed.sort();
    Ok(ExportReceipt {
        ok: true,
        profile: entry.name.clone(),
        label: entry.label.clone(),
        path: target.display().to_string(),
        bytes: bytes.len() as u64,
        files: listed,
        plugins: entry.plugins.clone(),
        note: format!(
            "已导出 {} 个配置文件。包内不含插件本体；在新机器导入后会按清单提示安装 {} 个插件。",
            bundle.files.len(),
            entry.plugins.len()
        ),
    })
}

/// 校验一个成员文件名：必须是白名单里的单段文件名。
fn check_bundle_path(file: &BundleFile) -> Result<(), String> {
    let path = file.path.trim();
    if path.is_empty() {
        return Err("迁移包里有一项没有文件名，未导入。".to_string());
    }
    if file.root == SKILL_BUNDLE_ROOT {
        return check_skill_bundle_path(path);
    }
    if path.contains('/') || path.contains('\\') || path.contains("..") || path.starts_with('.')
    {
        return Err(format!(
            "迁移包里的「{}」不是单段文件名，拒绝导入（不接受目录、上跳或隐藏文件）。",
            path
        ));
    }
    let names: &[&str] = match file.root.as_str() {
        "dsh" => DSH_BUNDLE_FILES,
        "app" => APP_BUNDLE_FILES,
        other => {
            return Err(format!(
                "迁移包里的「{}」标了未知归属「{}」，拒绝导入。",
                path, other
            ))
        }
    };
    if !names.contains(&path) {
        return Err(format!(
            "迁移包里有不在白名单里的文件「{}」，拒绝导入。",
            path
        ));
    }
    Ok(())
}

/// 自带 Skill 的成员路径：`<skill名>/<相对路径>`，至少两段。
///
/// 第一段必须是合法的 Skill 名（拼得成目录），其余各段拒绝空段、上跳、隐藏文件，
/// 也拒绝绝对路径与反斜杠——包是外部文件，不能让它借着路径写到别处。
fn check_skill_bundle_path(path: &str) -> Result<(), String> {
    if path.contains('\\') || path.starts_with('/') {
        return Err(format!("迁移包里的「{}」含非法分隔符，拒绝导入。", path));
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        return Err(format!(
            "迁移包里的自带 Skill 文件「{}」至少要有「Skill 名/文件名」两段。",
            path
        ));
    }
    if !crate::profile_rules::is_valid_skill_name(parts[0]) {
        return Err(format!(
            "迁移包里的 Skill 名「{}」不合法（只允许小写字母、数字、连字符、下划线）。",
            parts[0]
        ));
    }
    for part in &parts {
        if part.is_empty() || *part == "." || *part == ".." || part.starts_with('.') {
            return Err(format!(
                "迁移包里的自带 Skill 路径「{}」含空段、上跳或隐藏文件，拒绝导入。",
                path
            ));
        }
    }
    Ok(())
}

/// 解析并校验一个迁移包。**只读**：不建目录、不写任何文件。
///
/// 返回「验证过的包 + 每个成员的文件名 → 落点」，调用方据此落盘。
/// 分类表的 `profile` 字段在这里按目标名重写过（`rename_to` 允许改名导入）。
pub fn verify_bundle(
    path: &Path,
    rename_to: Option<&str>,
    label: Option<&str>,
) -> Result<(Bundle, String, String), String> {
    let meta = fs::metadata(path).map_err(|e| format!("读不到迁移包（{}）：{}", path.display(), e))?;
    if !meta.is_file() {
        return Err(format!("「{}」不是文件。", path.display()));
    }
    if meta.len() > BUNDLE_MAX_BYTES {
        return Err(format!(
            "迁移包有 {} 字节，超过上限 {} 字节，拒绝导入。",
            meta.len(),
            BUNDLE_MAX_BYTES
        ));
    }
    let raw = fs::read(path).map_err(|e| format!("读取迁移包失败（{}）：{}", path.display(), e))?;
    let bundle: Bundle = serde_json::from_slice(&raw)
        .map_err(|e| format!("迁移包不是合法 JSON：{}（行 {} 列 {}）", e, e.line(), e.column()))?;
    if bundle.schema != BUNDLE_SCHEMA {
        return Err(format!(
            "迁移包 schema 不匹配：期望 {}，实际 {}。",
            BUNDLE_SCHEMA, bundle.schema
        ));
    }
    if bundle.files.is_empty() {
        return Err("迁移包里没有任何文件，拒绝导入。".to_string());
    }
    if bundle.files.len() > BUNDLE_FILE_MAX_COUNT {
        return Err(format!(
            "迁移包里有 {} 个文件，超过上限 {}，拒绝导入。",
            bundle.files.len(),
            BUNDLE_FILE_MAX_COUNT
        ));
    }
    let original_name = bundle.profile.name.trim().to_string();
    if !crate::settings::is_valid_profile_name(&original_name) {
        return Err(format!("迁移包里的 Profile 名不合法：{:?}", original_name));
    }
    let target_name = rename_to
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&original_name)
        .to_string();
    if !crate::settings::is_valid_profile_name(&target_name) {
        return Err(format!(
            "要导入成的名字不合法：{:?}（只允许小写字母、数字、连字符、下划线，最多 48 字符）",
            target_name
        ));
    }
    let target_label = clean_display_label(
        label
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&bundle.profile.label),
    );
    if target_label.is_empty() || target_label.chars().count() > 32 {
        return Err("导入后的显示名要 1–32 个字。".to_string());
    }

    // 逐个成员校验：路径 → 大小 → 摘要 → 内容解析。
    let mut seen: Vec<(String, String)> = Vec::new();
    for file in &bundle.files {
        check_bundle_path(file)?;
        let key = (file.root.clone(), file.path.clone());
        if seen.contains(&key) {
            return Err(format!("迁移包里「{}」出现了两次，拒绝导入。", file.path));
        }
        seen.push(key);
        let bytes = file.text.as_bytes();
        if bytes.len() as u64 > BUNDLE_FILE_MAX_BYTES {
            return Err(format!("迁移包里的「{}」超过单文件上限，拒绝导入。", file.path));
        }
        if file.bytes != bytes.len() as u64 {
            return Err(format!(
                "迁移包里的「{}」字节数对不上（声明 {}，实际 {}），拒绝导入。",
                file.path,
                file.bytes,
                bytes.len()
            ));
        }
        let digest = crate::sha256::sha256_hex(bytes);
        if digest != file.sha256 {
            return Err(format!(
                "迁移包里的「{}」校验和不匹配，包可能损坏或被改过，拒绝导入。",
                file.path
            ));
        }
        match file.path.as_str() {
            // 自带 Skill 至少要有 SKILL.md，否则装上去也是一份选不中的空壳。
            p if file.root == SKILL_BUNDLE_ROOT && p.ends_with("/SKILL.md") => {
                if file.text.trim().is_empty() {
                    return Err(format!("迁移包里的「{}」是空的，拒绝导入。", file.path));
                }
            }
            "decl.yml" => {
                crate::profile_decl::parse_decl(&file.text)
                    .map_err(|e| format!("迁移包里的场景声明不合法：{}", e))?
                    .validate()
                    .map_err(|e| format!("迁移包里的场景声明不合法：{}", e))?;
            }
            "rules.json" => {
                crate::profile_rules::parse_rules(&file.text)
                    .map_err(|e| format!("迁移包里的规则包不合法：{}", e))?;
            }
            "categories.json" => {
                crate::profile_categories::parse(&file.text, &original_name)
                    .map_err(|e| format!("迁移包里的分类表不合法：{}", e))?;
            }
            "package.json" if file.root == "dsh" => {
                let value: serde_json::Value = serde_json::from_str(&file.text)
                    .map_err(|e| format!("迁移包里的 package.json 不是合法 JSON：{}", e))?;
                if value.get("dsh").and_then(|d| d.get("profile")).is_none() {
                    return Err(
                        "迁移包里的 package.json 没有 dsh.profile 字段，不像一个 DSH Profile，拒绝导入。"
                            .to_string(),
                    );
                }
            }
            _ => {}
        }
    }
    if !seen.iter().any(|(root, p)| root == "dsh" && p == "package.json") {
        return Err("迁移包里缺少 package.json，拒绝导入。".to_string());
    }
    Ok((bundle, target_name, target_label))
}

/// 导入一个迁移包（OD-ARCH-127）。
///
/// 顺序：校验（只读）→ 冲突检查 → 写临时目录 → 搬到位 → 写登记表。
/// 任何一步失败都把已经写下的东西清掉，不留半套 Profile。
/// 不联网：包内声明的插件只进登记表，由既有的「缺件补装」通道安装。
pub fn import_bundle(
    path: &Path,
    dsh_home: &Path,
    app_root: &Path,
    rename_to: Option<&str>,
    label: Option<&str>,
) -> Result<ImportReceipt, String> {
    crate::capabilities::assert_writable_root(dsh_home)?;
    let (bundle, target_name, target_label) = verify_bundle(path, rename_to, label)?;

    if crate::profile_assets::asset_profile(&target_name).is_some()
        || crate::profile_choice::is_whitelisted("web-api", &target_name)
    {
        return Err(format!(
            "「{}」是出厂 Profile 名，不能导入覆盖。换一个名字再试。",
            target_name
        ));
    }
    if load_registry(app_root)
        .profiles
        .iter()
        .any(|p| p.name == target_name)
    {
        return Err(format!(
            "「{}」已经在自定义 Profile 清单里。换一个名字再导入，不会覆盖已有工作台。",
            target_name
        ));
    }
    let dsh_dest = dsh_home.join("profiles").join(&target_name);
    let app_dest = app_root.join("profiles").join(&target_name);
    if dsh_dest.exists() || app_dest.exists() {
        return Err(format!(
            "已经有同名目录，未做任何改动：{}。换一个名字再导入。",
            if app_dest.exists() {
                app_dest.display().to_string()
            } else {
                dsh_dest.display().to_string()
            }
        ));
    }

    let stamp = format!("{}-{}", now_epoch(), std::process::id());
    let dsh_tmp = dsh_home.join("profiles").join(format!(".import-{}", &stamp));
    let app_tmp = app_root.join("profiles").join(format!(".import-{}", &stamp));
    let cleanup = |dirs: &[PathBuf]| {
        for dir in dirs {
            let _ = fs::remove_dir_all(dir);
        }
    };
    let mut written: Vec<String> = Vec::new();

    let result = (|| -> Result<(), String> {
        fs::create_dir_all(&dsh_tmp)
            .map_err(|e| format!("创建临时目录失败（{}）：{}", dsh_tmp.display(), e))?;
        for file in &bundle.files {
            let dir = if file.root == "dsh" { &dsh_tmp } else { &app_tmp };
            let mut text = file.text.clone();
            if file.path == "categories.json" {
                // 分类表里的归属字段随目标名改写，否则切换时会被判成别的 Profile 的表。
                let file_body: crate::profile_categories::CategoryFile =
                    serde_json::from_str(&file.text)
                        .map_err(|e| format!("分类表解析失败：{}", e))?;
                let rewritten = crate::profile_categories::CategoryFile {
                    schema: file_body.schema,
                    profile: target_name.clone(),
                    categories: file_body.categories,
                };
                text = serde_json::to_string_pretty(&rewritten)
                    .map_err(|e| format!("分类表重写失败：{}", e))?;
            }
            fs::create_dir_all(dir)
                .map_err(|e| format!("创建临时目录失败（{}）：{}", dir.display(), e))?;
            // 自带 Skill 的 path 是多段（`<skill名>/<相对路径>`），落点是
            // `profiles/<名字>/skills/<skill名>/<相对路径>`——导入后必须与导出源
            // （`profile_rules::skill_source_root`）同一位置，否则装配链看不到它。
            // 各段在 verify_bundle 里已经过形状校验，这里不再拼任何外部输入。
            let target = if file.root == SKILL_BUNDLE_ROOT {
                dir.join("skills").join(&file.path)
            } else {
                dir.join(&file.path)
            };
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!("创建目录失败（{}）：{}", parent.display(), e)
                })?;
            }
            crate::model_settings::write_atomic(&target, text.as_bytes())
                .map_err(|e| format!("写入 {} 失败：{}", target.display(), e))?;
            written.push(format!("{}/{}", file.root, file.path));
        }
        if !dsh_tmp.join("package.json").is_file() {
            return Err("临时目录里没有 package.json，导入中止。".to_string());
        }
        if let Err(e) = fs::rename(&dsh_tmp, &dsh_dest) {
            return Err(format!("把配置搬到 {} 失败：{}", dsh_dest.display(), e));
        }
        if app_tmp.is_dir() {
            if let Err(e) = fs::rename(&app_tmp, &app_dest) {
                let _ = fs::rename(&dsh_dest, &dsh_tmp);
                return Err(format!("把资产搬到 {} 失败：{}", app_dest.display(), e));
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        cleanup(&[dsh_tmp, app_tmp, dsh_dest.clone(), app_dest.clone()]);
        return Err(e);
    }

    let profile = ManagedProfile {
        name: target_name.clone(),
        label: target_label,
        created_at: if bundle.profile.created_at > 0 {
            bundle.profile.created_at
        } else {
            now_epoch()
        },
        origin: if bundle.profile.origin.trim().is_empty() {
            format!("import:{}", bundle.profile.name)
        } else {
            bundle.profile.origin.clone()
        },
        plugins: bundle.profile.plugins.clone(),
    };
    let mut registry = load_registry(app_root);
    registry.profiles.push(profile.clone());
    if let Err(e) = save_registry(app_root, &registry) {
        cleanup(&[dsh_dest.clone(), app_dest.clone()]);
        return Err(format!("{}（导入已撤回，磁盘保持原样）", e));
    }

    written.sort();
    Ok(ImportReceipt {
        ok: true,
        profile,
        directory: dsh_dest.display().to_string(),
        files: written,
        plugins: bundle.profile.plugins.clone(),
        original_name: (bundle.profile.name != target_name).then(|| bundle.profile.name.clone()),
        note: if bundle.profile.plugins.is_empty() {
            "已导入并登记。切过去即生效（切换会停掉旧引擎）；这个工作台没有额外插件。".to_string()
        } else {
            format!(
                "已导入并登记。切过去会按清单检查 {} 个插件，缺的会自动补齐（也可在资产页手动安装）。",
                bundle.profile.plugins.len()
            )
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(tag: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "od-profile-life-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let dsh = base.join("dsh-home");
        let app = base.join("app");
        fs::create_dir_all(dsh.join("profiles").join("omnidesk-base")).unwrap();
        fs::write(
            dsh.join("profiles").join("omnidesk-base").join("package.json"),
            r#"{"name":"dsh-profile-omnidesk-base","dsh":{"profile":{"bundles":[]}}}"#,
        )
        .unwrap();
        fs::write(
            dsh.join("profiles").join("omnidesk-base").join("cordis.patch.yml"),
            "[]\n",
        )
        .unwrap();
        fs::create_dir_all(&app).unwrap();
        (dsh, app)
    }

    #[test]
    fn create_from_standard_copies_config_and_registers() {
        let (dsh, app) = roots("create");
        let r = create(&dsh, &app, "my-work", "我的工作台", None).unwrap();
        assert!(r.ok);
        let dir = dsh.join("profiles").join("my-work");
        assert!(dir.join("package.json").is_file());
        assert!(dir.join("cordis.patch.yml").is_file());
        assert_eq!(registered_names(&app), vec!["my-work".to_string()]);
        // 复制基础标准时不应把整个 profiles 目录搬过去
        assert!(!dir.join("node_modules").exists());
        // OD-ARCH-122 缺口 B：从基础标准起手要自带最小工作台骨架，
        // 否则切过去只有平台固有入口，用户得从零写一整份声明。
        let app_dir = app.join("profiles").join("my-work");
        let decl = fs::read_to_string(app_dir.join("decl.yml")).expect("起手要有场景声明");
        let parsed = crate::profile_decl::parse_decl(&decl).unwrap();
        parsed.validate().unwrap();
        assert_eq!(parsed.display_name, "我的工作台");
        assert_eq!(parsed.scenes.len(), 4);
        assert!(parsed.scenes.iter().all(|s| s.id != "sessions"), "起手不再带历史会话");
        let rules = fs::read_to_string(app_dir.join("rules.json")).expect("起手要有规则包");
        let parsed_rules = crate::profile_rules::parse_rules(&rules).unwrap();
        assert_eq!(parsed_rules.profile, "my-work");
        assert!(parsed_rules.skill_bindings.is_empty(), "起手不得预注入 Skill");
        assert!(r.files.contains(&"decl.yml".to_string()));
        assert!(r.files.contains(&"rules.json".to_string()));
    }

    #[test]
    fn create_rejects_existing_and_factory_names() {
        let (dsh, app) = roots("reject");
        create(&dsh, &app, "my-work", "我的工作台", None).unwrap();
        assert!(create(&dsh, &app, "my-work", "重名", None).is_err());
        assert!(create(&dsh, &app, "discipline", "出厂", None).is_err());
        assert!(create(&dsh, &app, "omnidesk-base", "标准", None).is_err());
    }

    #[test]
    fn copy_carries_rules_but_not_node_modules() {
        let (dsh, app) = roots("copy");
        let src = dsh.join("profiles").join("source");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("package.json"), "{\"name\":\"x\"}").unwrap();
        fs::write(src.join("cordis.patch.yml"), "[]\n").unwrap();
        fs::create_dir_all(src.join("node_modules")).unwrap();
        fs::create_dir_all(app.join("profiles").join("source")).unwrap();
        fs::write(app.join("profiles").join("source").join("rules.json"), "{}").unwrap();

        let r = create(&dsh, &app, "clone", "克隆", Some("source")).unwrap();
        assert!(r.files.contains(&"package.json".to_string()));
        let dir = dsh.join("profiles").join("clone");
        assert!(dir.join("package.json").is_file());
        assert!(!dir.join("node_modules").exists());
        assert!(app.join("profiles").join("clone").join("rules.json").is_file());
        assert_eq!(r.profile.origin, "copy:source");
        // 复制路径不写起步骨架：来源没有 decl 就不该凭空造一个。
        assert!(!app.join("profiles").join("clone").join("decl.yml").exists());
    }

    #[test]
    fn remove_moves_to_trash_and_unregisters() {
        let (dsh, app) = roots("remove");
        create(&dsh, &app, "temp", "临时", None).unwrap();
        let r = remove(&dsh, &app, "temp", None).unwrap();
        assert!(r.trashed_dsh.is_some());
        assert!(!dsh.join("profiles").join("temp").exists());
        assert!(registered_names(&app).is_empty());
    }

    #[test]
    fn remove_refuses_current_and_factory() {
        let (dsh, app) = roots("guard");
        create(&dsh, &app, "temp", "临时", None).unwrap();
        assert!(remove(&dsh, &app, "temp", Some("temp")).is_err());
        assert!(remove(&dsh, &app, "discipline", None).is_err());
        assert!(remove(&dsh, &app, "omnidesk-base", None).is_err());
    }

    /// OD-ARCH-125：移除后必须在回收站里看得见，且拿回显示名与插件清单。
    #[test]
    fn trash_lists_removed_profile_with_meta() {
        let (dsh, app) = roots("trash-list");
        create(&dsh, &app, "my-work", "我的工作台", None).unwrap();
        remove(&dsh, &app, "my-work", None).unwrap();
        let entries = list_trash(&app, &dsh);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "my-work");
        assert_eq!(entries[0].label, "我的工作台");
        assert!(entries[0].has_meta);
        assert!(entries[0].has_dsh && entries[0].has_app);
        assert!(entries[0].trashed_at > 0);
        // 只读：列清单不建目录、不写盘
        assert!(app.join("profiles").join("my-work").exists() == false);
    }

    /// 恢复：登记回来、两个目录回来、回收站清空、插件清单仍在。
    #[test]
    fn restore_brings_back_registry_and_directories() {
        let (dsh, app) = roots("trash-restore");
        let src = dsh.join("profiles").join("source");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("package.json"),
            r#"{"name":"x","dependencies":{"@acme/keep":"1.0.0"}}"#,
        )
        .unwrap();
        fs::write(src.join("cordis.patch.yml"), "[]\n").unwrap();
        // 让应用数据根里也有这个 Profile 的资产目录，才能验证「两份目录一起恢复」。
        fs::create_dir_all(app.join("profiles").join("source")).unwrap();
        fs::write(
            app.join("profiles").join("source").join("rules.json"),
            "{\"profile\":\"source\"}",
        )
        .unwrap();
        create(&dsh, &app, "clone", "克隆", Some("source")).unwrap();
        remove(&dsh, &app, "clone", None).unwrap();
        let entries = list_trash(&app, &dsh);
        assert_eq!(entries.len(), 1);

        let r = restore(&dsh, &app, &entries[0].id).unwrap();
        assert!(r.ok);
        assert_eq!(r.profile.label, "克隆");
        assert!(r.profile.plugins.contains(&"@acme/keep".to_string()));
        assert_eq!(registered_names(&app), vec!["clone".to_string()]);
        assert!(dsh.join("profiles").join("clone").join("package.json").is_file());
        assert!(app.join("profiles").join("clone").is_dir());
        // 元数据不该留在恢复后的 Profile 目录里
        assert!(!app.join("profiles").join("clone").join(TRASH_FILE).exists());
        assert!(list_trash(&app, &dsh).is_empty());
    }

    /// 同名已存在时拒绝恢复，且不动任何文件。
    #[test]
    fn restore_refuses_when_same_name_exists() {
        let (dsh, app) = roots("trash-conflict");
        create(&dsh, &app, "dup", "备份", None).unwrap();
        remove(&dsh, &app, "dup", None).unwrap();
        let entries = list_trash(&app, &dsh);
        create(&dsh, &app, "dup", "新的", None).unwrap();

        let err = restore(&dsh, &app, &entries[0].id).unwrap_err();
        assert!(err.contains("已经在自定义 Profile 清单里"), "{err}");
        // 两个方向都没被动过
        assert!(dsh.join("profiles").join("dup").is_dir());
        assert!(dsh
            .join("profiles")
            .join(".trash")
            .join(&entries[0].id)
            .is_dir());
        assert_eq!(list_trash(&app, &dsh).len(), 1);
    }

    /// 元数据缺失时按目录名最后两段反解，仍能恢复名字与登记。
    #[test]
    fn restore_falls_back_to_directory_name_without_meta() {
        let (dsh, app) = roots("trash-nometa");
        create(&dsh, &app, "plain-work", "直接", None).unwrap();
        remove(&dsh, &app, "plain-work", None).unwrap();
        let entries = list_trash(&app, &dsh);
        let id = entries[0].id.clone();
        // 抹掉元数据，模拟旧版本留下的回收目录
        let _ = fs::remove_file(app.join("profiles").join(".trash").join(&id).join(TRASH_FILE));
        let _ = fs::remove_file(dsh.join("profiles").join(".trash").join(&id).join(TRASH_FILE));

        let listed = list_trash(&app, &dsh);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "plain-work");
        assert!(!listed[0].has_meta);
        let r = restore(&dsh, &app, &id).unwrap();
        assert_eq!(r.profile.name, "plain-work");
        assert_eq!(registered_names(&app), vec!["plain-work".to_string()]);
    }

    #[test]
    fn name_from_trash_id_rejects_bad_shapes() {
        assert_eq!(name_from_trash_id("my-work-123-456").as_deref(), Some("my-work"));
        assert_eq!(name_from_trash_id("plain-123-456").as_deref(), Some("plain"));
        assert!(name_from_trash_id("no-stamp").is_none());
        assert!(name_from_trash_id("name-x-456").is_none());
        assert!(name_from_trash_id("Name-123-456").is_none());
    }

    #[test]
    fn copy_records_inherited_plugin_dependencies() {
        let (dsh, app) = roots("plugins");
        let src = dsh.join("profiles").join("source");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("package.json"),
            r#"{"name":"x","dependencies":{"@acme/not-installed":"1.0.0","@acme/other":"2.0.0"}}"#,
        )
        .unwrap();
        fs::write(src.join("cordis.patch.yml"), "[]\n").unwrap();

        let r = create(&dsh, &app, "clone", "克隆", Some("source")).unwrap();
        assert!(r.profile.plugins.contains(&"@acme/not-installed".to_string()));
        assert!(r.profile.plugins.contains(&"@acme/other".to_string()));
    }

    #[test]
    fn create_rejects_invalid_names_and_labels() {
        let (dsh, app) = roots("validate");
        assert!(create(&dsh, &app, "Bad Name", "x", None).is_err());
        assert!(create(&dsh, &app, "ok-name", "", None).is_err());
    }

    /// OD-ARCH-117：复制 Profile 必须带上分类表，且归属改成新名字。
    #[test]
    fn create_from_profile_copies_categories() {
        let (dsh, app) = roots("categories");
        let src = dsh.join("profiles").join("source");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("package.json"), "{\"name\":\"x\"}").unwrap();
        fs::write(src.join("cordis.patch.yml"), "[]\n").unwrap();
        let mut categories = crate::settings::standard_thread_categories();
        categories.push(crate::settings::ThreadCategory {
            key: "custom:my-class".to_string(),
            label: "自定义类".to_string(),
            folder: None,
            vault: None,
        });
        crate::profile_categories::save(&app, "source", &categories).unwrap();

        create(&dsh, &app, "clone", "克隆", Some("source")).unwrap();
        let copied = crate::profile_categories::load(&app, "clone")
            .unwrap()
            .expect("复制出来的 Profile 应当有自己的分类表");
        assert_eq!(copied.len(), categories.len());
        assert!(copied.iter().any(|c| c.key == "custom:my-class"));
        // 归属必须写成新名字，否则 parse 会以「归属不符」拒绝
        let raw = fs::read_to_string(crate::profile_categories::path(&app, "clone")).unwrap();
        assert!(raw.contains("\"clone\""));
    }

    /// 补丁里的子路径导出不是独立 npm 包（OD-ARCH-119）：
    /// `@scope/pkg/list-agents` 必须折回 `@scope/pkg`，否则资产页会永远
    /// 报「缺失插件」、而安装通道只接受两段式包名，点了必然失败。
    #[test]
    fn detect_plugins_folds_subpath_exports_to_package_root() {
        let base = std::env::temp_dir().join(format!(
            "od-profile-life-119-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        fs::write(
            base.join("package.json"),
            r#"{"name":"p","dependencies":{"@deepseek-ai/dsh-plugin-manager/tools":"1.0.0"}}"#,
        )
        .unwrap();
        fs::write(
            base.join("cordis.patch.yml"),
            "- id: a\n  name: '@deepseek-ai/dsh-tool-subagent-control/list-agents'\n  config:\n    x: 1\n",
        )
        .unwrap();

        let got = detect_plugins(&base);
        assert_eq!(
            got,
            vec![
                "@deepseek-ai/dsh-plugin-manager".to_string(),
                "@deepseek-ai/dsh-tool-subagent-control".to_string(),
            ],
            "子路径要折回包根，并且排序去重"
        );
    }

    /// 归一化只处理 scoped 两段式；两段式本身与非 scoped 名字保持原样。
    #[test]
    fn package_root_only_strips_scoped_subpaths() {
        assert_eq!(package_root("@a/b"), "@a/b");
        assert_eq!(package_root("@a/b/c"), "@a/b");
        assert_eq!(package_root("@a/b/c/d"), "@a/b");
        assert_eq!(package_root("plain"), "plain");
        assert_eq!(package_root("plain/sub"), "plain/sub");
        assert_eq!(package_root("  @a/b/c  "), "@a/b");
    }

    /// OD-ARCH-126：改名要同时命中登记表与场景声明，目录名不动。
    #[test]
    fn rename_updates_registry_and_declaration() {
        let (dsh, app) = roots("rename");
        create(&dsh, &app, "my-work", "我的工作台", None).unwrap();
        let receipt = rename_label(&app, "my-work", "我的纪检工作台").unwrap();
        assert!(receipt.ok);
        assert_eq!(receipt.previous_label, "我的工作台");
        assert!(receipt.decl_updated, "有声明时应当一并改 display_name");
        assert!(receipt.backup.is_some(), "改声明前要留备份");

        let registry = load_registry(&app);
        let entry = registry.profiles.iter().find(|p| p.name == "my-work").unwrap();
        assert_eq!(entry.label, "我的纪检工作台");
        // 目录名与 profile 标识不变，Skill/插件/分类绑定才不会跟着抖。
        assert!(dsh.join("profiles").join("my-work").is_dir());
        assert!(!dsh.join("profiles").join("我的纪检工作台").exists());

        let raw = fs::read_to_string(
            app.join("profiles").join("my-work").join("decl.yml"),
        )
        .unwrap();
        let parsed = crate::profile_decl::parse_decl(&raw).unwrap();
        parsed.validate().unwrap();
        assert_eq!(parsed.display_name, "我的纪检工作台");
        assert!(parsed.scenes.iter().any(|s| s.id == "home"));
    }

    /// 声明被改坏时必须整体拒绝，登记表一个字节都不动。
    #[test]
    fn rename_refuses_when_declaration_is_broken() {
        let (dsh, app) = roots("rename-broken");
        create(&dsh, &app, "my-work", "我的工作台", None).unwrap();
        let decl = app.join("profiles").join("my-work").join("decl.yml");
        fs::write(&decl, "schema: nope\n").unwrap();
        let err = rename_label(&app, "my-work", "新名字").unwrap_err();
        assert!(err.contains("未做任何改动"), "实际：{err}");
        let registry = load_registry(&app);
        assert_eq!(registry.profiles[0].label, "我的工作台");
        assert_eq!(fs::read_to_string(&decl).unwrap(), "schema: nope\n");
    }

    /// 未登记的历史目录与出厂 Profile 都不许改名。
    #[test]
    fn rename_refuses_unregistered_and_factory() {
        let (dsh, app) = roots("rename-guard");
        let stray = dsh.join("profiles").join("web");
        fs::create_dir_all(&stray).unwrap();
        fs::write(stray.join("package.json"), "{}").unwrap();
        assert!(rename_label(&app, "web", "遗留").is_err());
        assert!(rename_label(&app, "discipline", "出厂").is_err());
        assert!(rename_label(&app, "omnidesk-base", "标准").is_err());
    }

    /// 显示名越界、含破坏声明语法的字符都要明确拒绝。
    #[test]
    fn rename_rejects_bad_labels() {
        let (dsh, app) = roots("rename-label");
        create(&dsh, &app, "my-work", "我的工作台", None).unwrap();
        assert!(rename_label(&app, "my-work", "   ").is_err());
        assert!(rename_label(&app, "my-work", &"长".repeat(33)).is_err());
        let err = rename_label(&app, "my-work", "带 # 号").unwrap_err();
        assert!(err.contains("#"), "实际：{err}");
        // 多行粘贴压成一行空格，不是报错。
        let receipt = rename_label(&app, "my-work", "我的\n工作台").unwrap();
        assert_eq!(receipt.profile.label, "我的 工作台");
        // 失败的那些不能留下半截名字。
        assert_eq!(load_registry(&app).profiles[0].label, "我的 工作台");
    }

    /// 没有声明时只改登记表，并如实说明。
    #[test]
    fn rename_without_declaration_only_touches_registry() {
        let (dsh, app) = roots("rename-nodecl");
        let src = dsh.join("profiles").join("source");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("package.json"), "{\"name\":\"x\"}").unwrap();
        fs::write(src.join("cordis.patch.yml"), "[]\n").unwrap();
        create(&dsh, &app, "clone", "克隆", Some("source")).unwrap();
        assert!(!app.join("profiles").join("clone").join("decl.yml").exists());
        let receipt = rename_label(&app, "clone", "新克隆").unwrap();
        assert!(!receipt.decl_updated);
        assert!(receipt.note.contains("没有场景声明"), "实际：{}", receipt.note);
        assert_eq!(load_registry(&app).profiles[0].label, "新克隆");
        assert!(!app.join("profiles").join("clone").join("decl.yml").exists());
    }

    fn seed_custom(dsh: &Path, app: &Path, name: &str) {
        create(dsh, app, name, "迁移测试", None).unwrap();
        let app_dir = app.join("profiles").join(name);
        // 分类表随 Profile 走；导入时必须把 profile 字段改成新名字。
        crate::profile_categories::save(
            app,
            name,
            &[
                crate::settings::ThreadCategory {
                    key: "plain".to_string(),
                    label: "随手记".to_string(),
                    folder: None,
                    vault: None,
                },
                crate::settings::ThreadCategory {
                    key: "weekly".to_string(),
                    label: "周报".to_string(),
                    folder: None,
                    vault: None,
                },
            ],
        )
        .unwrap();
        // 干扰项：回收元数据、备份文件、插件实体都不该进包。
        fs::write(app_dir.join("decl.yml.bak-1"), "stale").unwrap();
        fs::create_dir_all(dsh.join("profiles").join(name).join("node_modules")).unwrap();
        fs::write(
            dsh.join("profiles").join(name).join("node_modules").join("x.js"),
            "x",
        )
        .unwrap();
        let mut registry = load_registry(app);
        if let Some(entry) = registry.profiles.iter_mut().find(|p| p.name == name) {
            entry.plugins = vec!["@deepseek-ai/dsh-skill-office".to_string()];
        }
        save_registry(app, &registry).unwrap();
    }

    #[test]
    fn export_keeps_whitelist_and_drops_runtime_noise() {
        let (dsh, app) = roots("export");
        seed_custom(&dsh, &app, "my-work");
        let dest = app.join("out");
        let r = export_bundle(&dsh, &app, "my-work", Some(&dest)).unwrap();
        assert!(r.ok);
        assert!(Path::new(&r.path).is_file());
        assert!(r.files.contains(&"dsh/package.json".to_string()));
        assert!(r.files.contains(&"app/decl.yml".to_string()));
        assert!(r.files.contains(&"app/categories.json".to_string()));
        assert!(!r.files.iter().any(|f| f.contains("node_modules")));
        assert!(!r.files.iter().any(|f| f.contains(".bak-")));
        let raw = fs::read_to_string(&r.path).unwrap();
        assert!(!raw.contains("node_modules"));
        assert!(!raw.contains("decl.yml.bak"));
        assert_eq!(r.plugins, vec!["@deepseek-ai/dsh-skill-office".to_string()]);
    }

    #[test]
    fn export_refuses_factory_and_unregistered() {
        let (dsh, app) = roots("export-reject");
        fs::create_dir_all(dsh.join("profiles").join("discipline")).unwrap();
        fs::write(
            dsh.join("profiles").join("discipline").join("package.json"),
            "{}",
        )
        .unwrap();
        assert!(export_bundle(&dsh, &app, "discipline", None).is_err());
        fs::create_dir_all(dsh.join("profiles").join("legacy")).unwrap();
        fs::write(dsh.join("profiles").join("legacy").join("package.json"), "{}").unwrap();
        assert!(export_bundle(&dsh, &app, "legacy", None).is_err());
    }

    #[test]
    fn import_round_trip_rewrites_categories_and_keeps_plugins() {
        let (dsh, app) = roots("roundtrip");
        seed_custom(&dsh, &app, "my-work");
        let dest = app.join("out");
        let exported = export_bundle(&dsh, &app, "my-work", Some(&dest)).unwrap();
        remove(&dsh, &app, "my-work", None).unwrap();
        assert!(registered_names(&app).is_empty());

        let r = import_bundle(
            Path::new(&exported.path),
            &dsh,
            &app,
            Some("moved-work"),
            Some("搬来的工作台"),
        )
        .unwrap();
        assert!(r.ok);
        assert_eq!(r.original_name.as_deref(), Some("my-work"));
        assert_eq!(r.plugins, vec!["@deepseek-ai/dsh-skill-office".to_string()]);
        assert_eq!(registered_names(&app), vec!["moved-work".to_string()]);
        assert_eq!(r.profile.label, "搬来的工作台");
        // 声明可以直接解析，且显示名仍是包里那一份（改名只改登记与目录标识）。
        let decl = fs::read_to_string(app.join("profiles/moved-work/decl.yml")).unwrap();
        crate::profile_decl::parse_decl(&decl).unwrap().validate().unwrap();
        // 分类表的归属字段已改写成新名字，否则切换时会被判成别人的表。
        let cats = crate::profile_categories::load(&app, "moved-work").unwrap().unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[1].label, "周报");
        assert!(dsh.join("profiles/moved-work/package.json").is_file());
        // 临时目录不能留下。
        let leftovers: Vec<String> = fs::read_dir(app.join("profiles"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(".import-"))
            .collect();
        assert!(leftovers.is_empty(), "残留临时目录：{:?}", leftovers);
    }

    #[test]
    fn import_refuses_tampered_bundle_without_touching_disk() {
        let (dsh, app) = roots("tamper");
        seed_custom(&dsh, &app, "my-work");
        let dest = app.join("out");
        let exported = export_bundle(&dsh, &app, "my-work", Some(&dest)).unwrap();
        let raw = fs::read_to_string(&exported.path).unwrap();
        // 改一个成员文件的正文，摘要就该对不上。改 bundle.profile.label 不会触发
        // 摘要校验（它不在 files 里），所以这里必须动文件内容。
        // 换成等长文本，确保走的是摘要校验而不是先被字节数拦下。
        let tampered = raw.replace("迁移测试", "被改名字");
        assert_ne!(raw, tampered, "测试前提：正文里确实有可替换的字");
        let bad = dest.join("bad.omniprofile.json");
        fs::write(&bad, tampered).unwrap();

        let before = registered_names(&app);
        let err = import_bundle(&bad, &dsh, &app, Some("other"), None).unwrap_err();
        assert!(err.contains("校验和"), "错误应指向摘要不匹配：{}", err);
        assert_eq!(registered_names(&app), before);
        assert!(!dsh.join("profiles/other").exists());
        assert!(!app.join("profiles/other").exists());
    }

    #[test]
    fn import_refuses_traversal_paths() {
        let (dsh, app) = roots("traversal");
        let bundle = serde_json::json!({
            "schema": BUNDLE_SCHEMA,
            "exported_at": 0,
            "app_version": "test",
            "profile": {"name": "evil", "label": "evil", "created_at": 0, "origin": "standard", "plugins": []},
            "files": [
                {
                    "path": "../escape.json",
                    "root": "app",
                    "sha256": crate::sha256::sha256_hex(b"{}"),
                    "bytes": 2,
                    "text": "{}"
                }
            ]
        });
        let bad = app.join("evil.json");
        fs::write(&bad, serde_json::to_vec(&bundle).unwrap()).unwrap();
        let err = import_bundle(&bad, &dsh, &app, None, None).unwrap_err();
        assert!(err.contains("单段文件名"), "错误应拒绝路径穿越：{}", err);
        assert!(!app.join("escape.json").exists());
    }

    /// OD-ARCH-131：Profile 自带 Skill 要随包往返，并落回 `profiles/<名字>/skills/`。
    #[test]
    fn round_trip_keeps_profile_owned_skills() {
        let (dsh, app) = roots("skill-roundtrip");
        seed_custom(&dsh, &app, "my-work");
        let skill_dir = app
            .join("profiles")
            .join("my-work")
            .join("skills")
            .join("office-note");
        fs::create_dir_all(skill_dir.join("references")).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: office-note\n---\n单位写法\n").unwrap();
        fs::write(skill_dir.join("references").join("extra.md"), "补充说明\n").unwrap();

        let dest = app.join("out");
        let exported = export_bundle(&dsh, &app, "my-work", Some(&dest)).unwrap();
        assert!(
            exported.files.contains(&"skill/office-note/SKILL.md".to_string()),
            "导出清单要带上自带 Skill：{:?}",
            exported.files
        );
        assert!(exported
            .files
            .contains(&"skill/office-note/references/extra.md".to_string()));

        remove(&dsh, &app, "my-work", None).unwrap();
        let r = import_bundle(Path::new(&exported.path), &dsh, &app, Some("moved-work"), None)
            .unwrap();
        assert!(r.ok);
        let moved = app.join("profiles").join("moved-work").join("skills").join("office-note");
        assert!(moved.join("SKILL.md").is_file(), "导入后要落回 profiles/<名字>/skills/");
        assert!(moved.join("references").join("extra.md").is_file());
        assert_eq!(
            crate::profile_rules::local_skills(&app.join("profiles").join("moved-work")),
            vec!["office-note".to_string()]
        );
    }

    /// 包里的自带 Skill 路径同样不能上跳。
    #[test]
    fn import_refuses_skill_traversal_paths() {
        let (dsh, app) = roots("skill-traversal");
        let bundle = serde_json::json!({
            "schema": BUNDLE_SCHEMA,
            "exported_at": 0,
            "app_version": "test",
            "profile": {"name": "evil", "label": "evil", "created_at": 0, "origin": "standard", "plugins": []},
            "files": [
                {
                    "path": "office-note/../../escape.md",
                    "root": SKILL_BUNDLE_ROOT,
                    "sha256": crate::sha256::sha256_hex(b"x"),
                    "bytes": 1,
                    "text": "x"
                }
            ]
        });
        let bad = app.join("evil-skill.json");
        fs::write(&bad, serde_json::to_vec(&bundle).unwrap()).unwrap();
        let err = import_bundle(&bad, &dsh, &app, None, None).unwrap_err();
        assert!(err.contains("上跳"), "错误应拒绝路径穿越：{}", err);
        assert!(!app.join("profiles/evil").exists());
    }

    #[test]
    fn import_refuses_existing_name_and_leaves_disk_alone() {
        let (dsh, app) = roots("conflict");
        seed_custom(&dsh, &app, "my-work");
        let dest = app.join("out");
        let exported = export_bundle(&dsh, &app, "my-work", Some(&dest)).unwrap();
        let err = import_bundle(Path::new(&exported.path), &dsh, &app, None, None).unwrap_err();
        assert!(err.contains("已经在自定义 Profile 清单里"), "{}", err);
        assert_eq!(registered_names(&app), vec!["my-work".to_string()]);
        // 原目录内容没被覆盖。
        assert!(app.join("profiles/my-work/decl.yml").is_file());
    }
    #[test]
    fn list_exports_skips_broken_and_sorts_newest_first() {
        let (dsh, app) = roots("listen");
        seed_custom(&dsh, &app, "my-work");
        let out = app.join("exports");
        let r = export_bundle(&dsh, &app, "my-work", Some(&out)).unwrap();
        fs::write(out.join("broken.omniprofile.json"), "{not json").unwrap();
        fs::write(out.join("notes.txt"), "x").unwrap();
        let listed = list_exports(&app);
        assert_eq!(listed.len(), 1, "坏包与杂项要被跳过：{:?}", listed);
        assert_eq!(listed[0].path, r.path);
        assert_eq!(listed[0].profile, "my-work");
        assert!(listed[0].bytes > 0);
        assert!(list_exports(&app.join("nope")).is_empty(), "目录不存在时只返回空表");
    }
}
