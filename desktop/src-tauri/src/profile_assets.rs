//! Profile 资产自装配（OD-ARCH-109）。
//!
//! 产品口径（用户 2026-10-29）：「OmniDesk 是干净和初始的环境，而 Profile 是定制的，
//! 所以切换 Profile 时要能自动写入所需的配置、Skill 和插件，如有缺失插件需要提供
//! 自动安装的功能。」
//!
//! 因此本模块只做一件事：把某个 Profile 需要的**出厂资产**补齐到隔离数据根，
//! 让「切换 Profile」等于「装好一个专属工作台」。边界：
//!   * 标准 Profile（omnidesk-base）**零写入**：目录都不创建，保持干净底座；
//!   * 只补缺失，不覆盖用户改过的文件：靠 `.assets.v1.json` 记录「上次我们写过什么」；
//!   * 改前先备份为 `.bak-<epoch>`，与规则写入同口径；
//!   * 插件只做**本地探测**，联网安装必须由用户点按钮触发；
//!   * 不写 runtime/，不碰 `~/.dsh` / `~/.agents`（由 `assert_writable_root` 兜底）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// 单个插件安装命令的超时（OD-ARCH-121）。
///
/// 切换 Profile 后会**在后台**自动补装缺失插件，所以要有一道上界：
/// 断网或 registry 挂住时，任务必须能自己收尾，而不是永远占着一条阻塞线程。
/// 180 秒足够装一个带构建脚本的包（120 的实跑约 8 秒），也远短到用户能感知为「失败」。
pub const INSTALL_TIMEOUT: Duration = Duration::from_secs(180);

/// 装配状态文件（放在应用数据根的 `profiles/<name>/` 下，与规则资产同处）。
pub const ASSET_STATE_FILE: &str = ".assets.v1.json";
pub const ASSET_STATE_SCHEMA: &str = "omnidesk.profile-assets.v1";

/// 记录「OmniDesk 往全局 skills/ 里写过哪些 Skill」。
/// 全局目录会被 DSH 的 skill-filesystem 无条件扫描，所以要能按 Profile 收回。
pub const MANAGED_SKILLS_FILE: &str = ".omnidesk-managed";
/// 停放目录（相对应用数据根）：切走时移到这里，切回来再归位。
pub const PARKED_SKILLS_DIR: &str = "profiles/.parked-skills";

/// 条目的状态词。界面按这套词显示，不在前端再翻译一遍。
pub const STATUS_READY: &str = "ready";
pub const STATUS_CREATED: &str = "created";
pub const STATUS_UPDATED: &str = "updated";
pub const STATUS_UNCHANGED: &str = "unchanged";
pub const STATUS_DRIFT: &str = "drift";
/// 文件还是我们上次写的旧版，出厂内容已更新：切换/修复时可以安全覆盖。
pub const STATUS_STALE: &str = "stale";
pub const STATUS_MISSING_SOURCE: &str = "missing_source";
pub const STATUS_MISSING: &str = "missing";

/// 一个插件依赖。`version` 为 `None` 表示「装上就行」，用于随运行时自带的包：
/// 运行时升级后不该因为版本号对不上就报红。
pub struct PluginRequirement {
    pub package: &'static str,
    pub version: Option<&'static str>,
}

/// 一个 Profile 的出厂资产清单。正文全部编译期嵌入，不依赖仓库路径。
pub struct AssetProfile {
    pub name: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    /// `profiles/<name>/package.json`（DSH_HOME 目录内）。
    pub manifest: &'static str,
    /// `profiles/<name>/cordis.patch.yml`（DSH_HOME 目录内）。
    pub patch: &'static str,
    /// 场景声明；落 `profiles/<name>/decl.yml`（应用数据根）。
    pub decl: Option<&'static str>,
    /// 行业规则包；落 `profiles/<name>/rules.json`（应用数据根）。
    pub rules: Option<&'static str>,
    /// 需要的 Skill 目录名（从 `<repo>/skills/<name>` 补到 `<dsh_home>/skills/<name>`）。
    pub skills: &'static [&'static str],
    /// 需要的插件包。
    pub plugins: &'static [PluginRequirement],
}

const DISCIPLINE_PLUGINS: &[PluginRequirement] = &[PluginRequirement {
    package: "@deepseek-ai/dsh-skill-office",
    version: None,
}];

const DISCIPLINE_SKILLS: &[&str] = &[
    "anysearch",
    "document-review",
    "knowledge-agent",
    "knowledge-search",
    "material-writing",
    "memory-agent",
    "memory-report",
    "project-agent",
];

/// 出厂清单。新增 Profile = 在这里加一条 + 在 `samples/profiles/` 放资产。
pub static PROFILES: &[AssetProfile] = &[AssetProfile {
    name: "discipline",
    label: "纪检专业",
    summary: "工作记录 / 文案写作 / 项目跟踪三个业务场景，含行业规则、8 个 Skill 与办公插件。",
    manifest: include_str!("../../../samples/profiles/discipline.profile.json"),
    patch: include_str!("../../../samples/profiles/discipline.profile.patch.yml"),
    decl: Some(include_str!("../../../samples/profiles/discipline.yml")),
    rules: Some(include_str!("../../../samples/profiles/discipline.rules.json")),
    skills: DISCIPLINE_SKILLS,
    plugins: DISCIPLINE_PLUGINS,
}];

/// 按名字找清单；不是出厂 Profile 时返回 `None`（自定义 Profile 不装配）。
pub fn asset_profile(name: &str) -> Option<&'static AssetProfile> {
    PROFILES.iter().find(|p| p.name == name.trim())
}

/// 装配范围。两个根要分开传：DSH 的 Profile 目录在 `dsh_home` 下，
/// 界面资产（声明/规则/状态）在应用数据根下，两者历来不是同一处。
pub struct Scope<'a> {
    pub dsh_home: &'a Path,
    pub app_root: &'a Path,
    pub profile: &'a str,
    pub repo_root: Option<&'a Path>,
}

/// 单个资产的装配结果。
#[derive(Debug, Clone, Serialize)]
pub struct AssetItem {
    /// 逻辑键：`package.json` / `skill:memory-agent` / `plugin:@deepseek-ai/...`。
    pub key: String,
    /// 界面分组：config / patch / decl / rules / skill / plugin。
    pub kind: String,
    pub label: String,
    pub path: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterializeReport {
    pub profile: String,
    pub label: String,
    /// 这个 Profile 做什么的（给界面做副标题；自定义 Profile 为空）。
    pub summary_note: Option<String>,
    /// 标准 Profile：没有装配任何东西（也没有创建目录）。
    pub standard: bool,
    pub items: Vec<AssetItem>,
    pub notes: Vec<String>,
    /// 仓库根不可用时的提示（Skill 无法补齐，其余资产仍然可用）。
    pub repo_missing: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AssetState {
    #[serde(default)]
    schema: String,
    /// 逻辑键 → 上次由本模块写入内容的 sha256。
    #[serde(default)]
    written: BTreeMap<String, String>,
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn backup_of(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("asset")
        .to_string();
    let stamp = now_epoch();
    let mut target = path.with_file_name(format!("{}.bak-{}", file_name, stamp));
    let mut n = 1;
    while target.exists() {
        target = path.with_file_name(format!("{}.bak-{}-{}", file_name, stamp, n));
        n += 1;
    }
    fs::copy(path, &target)
        .map_err(|e| format!("备份 {} 失败（{}）：{}", file_name, target.display(), e))?;
    Ok(Some(target))
}

// ---------------------------------------------------------------------------
// Skill 归属（OD-ARCH-115）：全局 skills/ 要能按 Profile 收回
// ---------------------------------------------------------------------------

/// 某个 Profile 需要哪些 Skill 落在全局 skills/ 目录。
///
/// 基础标准是空集——它不该看到任何业务 Skill；出厂 Profile 用清单；
/// 自定义 Profile 用 rules.json 里的 skill_bindings。
pub fn wanted_skills(scope: &Scope<'_>, standard: &str) -> Vec<String> {
    let profile = scope.profile.trim();
    if profile == standard {
        return Vec::new();
    }
    if let Some(asset) = asset_profile(profile) {
        return asset.skills.iter().map(|s| s.to_string()).collect();
    }
    let mut out: Vec<String> = Vec::new();
    let rules_path = scope
        .app_root
        .join("profiles")
        .join(profile)
        .join("rules.json");
    if let Ok(text) = fs::read_to_string(&rules_path) {
        if let Ok(rules) = crate::profile_rules::parse_rules(&text) {
            for binding in rules.skill_bindings.values() {
                for name in &binding.skills {
                    if crate::settings::is_valid_profile_name(name) && !out.contains(name) {
                        out.push(name.clone());
                    }
                }
            }
        }
    }
    out
}

fn managed_skills_path(dsh_home: &Path) -> PathBuf {
    dsh_home.join("skills").join(MANAGED_SKILLS_FILE)
}

fn load_managed_skills(dsh_home: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(managed_skills_path(dsh_home)) else {
        return Vec::new();
    };
    let mut out: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|l| crate::settings::is_valid_profile_name(l))
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// 写登记表；空集时删掉文件，避免在干净底座里留一个空壳。
fn save_managed_skills(dsh_home: &Path, names: &[String]) -> Result<(), String> {
    let path = managed_skills_path(dsh_home);
    if names.is_empty() {
        if path.is_file() {
            fs::remove_file(&path)
                .map_err(|e| format!("清理 Skill 登记表失败（{}）：{}", path.display(), e))?;
        }
        return Ok(());
    }
    let mut text = String::from(
        "# OmniDesk 写入的 Skill。切到不需要它们的 Profile 时会移入 profiles/.parked-skills/。\n",
    );
    for name in names {
        text.push_str(name);
        text.push('\n');
    }
    crate::model_settings::write_atomic(&path, text.as_bytes())
}

fn parked_root(app_root: &Path) -> PathBuf {
    app_root.join(PARKED_SKILLS_DIR)
}

/// 停放区里的一个 Skill（OD-ARCH-117）。
///
/// 停放是「移走不删除」：切到不需要它的 Profile 时目录被挪进
/// `profiles/.parked-skills/`，切回需要它的 Profile 会自动归位。这里给出只读
/// 清单，让用户在设置页看得见「现在有哪些 Skill 正停在边上」。
#[derive(Debug, Clone, Serialize)]
pub struct ParkedSkill {
    pub name: String,
    pub path: String,
    /// 目录里的条目数（含子目录与文件），用于判断是不是只剩一个空壳。
    pub entries: usize,
}

/// 列出停放区里的 Skill。只读：目录不存在就返回空表，绝不顺手建目录——
/// 基础标准态要保证干净底座不被看一眼就多出一个文件夹。
pub fn list_parked(app_root: &Path) -> Vec<ParkedSkill> {
    let root = parked_root(app_root);
    let Ok(read) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out: Vec<ParkedSkill> = read
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                return None;
            }
            let path = e.path();
            let entries = fs::read_dir(&path).map(|d| d.count()).unwrap_or(0);
            Some(ParkedSkill {
                name,
                path: path.display().to_string(),
                entries,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 目标需要、但还停在停放区的 Skill 归位；已有目录不覆盖（保留用户改动）。
fn restore_parked(scope: &Scope<'_>, wanted: &[String]) -> Vec<String> {
    let skills_root = scope.dsh_home.join("skills");
    let parked = parked_root(scope.app_root);
    let mut returned = Vec::new();
    for name in wanted {
        let from = parked.join(name);
        let to = skills_root.join(name);
        if from.is_dir() && !to.exists() {
            if fs::create_dir_all(&skills_root).is_ok() && fs::rename(&from, &to).is_ok() {
                returned.push(name.clone());
            }
        }
    }
    returned
}

fn backup_dir(dir: &Path) -> Result<PathBuf, String> {
    let stamp = now_epoch();
    let mut target = PathBuf::from(format!("{}.bak-{}", dir.display(), stamp));
    let mut n = 1;
    while target.exists() {
        target = PathBuf::from(format!("{}.bak-{}-{}", dir.display(), stamp, n));
        n += 1;
    }
    fs::rename(dir, &target)
        .map_err(|e| format!("备份目录 {} 失败（{}）：{}", dir.display(), target.display(), e))?;
    Ok(target)
}

/// 把 OmniDesk 管过、但目标 Profile 不需要的 Skill 移入停放目录（移动，不删除）。
fn retire_unwanted(scope: &Scope<'_>, wanted: &[String]) -> Vec<String> {
    let skills_root = scope.dsh_home.join("skills");
    let parked = parked_root(scope.app_root);
    let mut moved = Vec::new();
    for name in load_managed_skills(scope.dsh_home) {
        if wanted.iter().any(|w| w == &name) {
            continue;
        }
        let from = skills_root.join(&name);
        if !from.is_dir() {
            continue;
        }
        let to = parked.join(&name);
        if to.exists() {
            let _ = backup_dir(&to);
        }
        if fs::create_dir_all(&parked).is_ok() && fs::rename(&from, &to).is_ok() {
            moved.push(name);
        }
    }
    moved
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SkillScopeReport {
    /// 目标需要、从停放区归位的 Skill。
    pub returned: Vec<String>,
    /// 目标不需要、被移入停放区的 Skill。
    pub parked: Vec<String>,
}

/// 让全局 skills/ 与目标 Profile 一致（OD-ARCH-115）。
///
/// 只在切换 Profile 与启动引擎时调用。基础标准且从未装配过时是纯空操作：
/// 不建目录、不写文件，干净底座的前提不被破坏。
pub fn reconcile_managed_skills(
    scope: &Scope<'_>,
    standard: &str,
) -> Result<SkillScopeReport, String> {
    let wanted = wanted_skills(scope, standard);
    let returned = restore_parked(scope, &wanted);
    let parked = retire_unwanted(scope, &wanted);
    // 登记表只保留「目标需要且确实在全局目录里」的项。
    let skills_root = scope.dsh_home.join("skills");
    let mut keep: Vec<String> = wanted
        .iter()
        .filter(|n| skills_root.join(n).join("SKILL.md").is_file())
        .cloned()
        .collect();
    keep.sort();
    keep.dedup();
    save_managed_skills(scope.dsh_home, &keep)?;
    Ok(SkillScopeReport { returned, parked })
}

/// 装配完成后登记「这些 Skill 是 OmniDesk 写的」（OD-ARCH-115）。
///
/// reconcile 在装配之前跑，那时目标 Skill 还没落盘，没法登记；装配完再补这一步，
/// 下次切到别的 Profile 才知道哪些目录应该收回。只登记目标需要且确实存在的目录，
/// 基础标准不登记任何东西（登记表会被清掉）。
pub fn record_managed_skills(scope: &Scope<'_>, standard: &str) -> Result<(), String> {
    let wanted = wanted_skills(scope, standard);
    if wanted.is_empty() {
        return Ok(());
    }
    let skills_root = scope.dsh_home.join("skills");
    let mut keep = load_managed_skills(scope.dsh_home);
    for name in wanted {
        if skills_root.join(&name).join("SKILL.md").is_file() && !keep.contains(&name) {
            keep.push(name);
        }
    }
    keep.sort();
    keep.dedup();
    save_managed_skills(scope.dsh_home, &keep)
}

/// 一次「保存绑定后立刻收敛 Skill」的结果（OD-ARCH-130）。
///
/// 保存 Skill 绑定只写 rules.json 是不够的：运行时真正能扫到的是
/// `<dsh_home>/skills/`。所以保存成功后要按**新的**绑定把这份目录收敛一遍，
/// 并把明细回传给界面——用户需要知道「点完是不是真的能用了」。
#[derive(Debug, Clone, Default, Serialize)]
pub struct BoundSkillSync {
    pub profile: String,
    pub standard: bool,
    /// 绑定里点到的 Skill（按场景顺序去重）。
    pub wanted: Vec<String>,
    /// 这次新补进去（或按出厂内容更新）的。
    pub filled: Vec<String>,
    /// 本来就位、这次没动的。
    pub kept: Vec<String>,
    /// 从停放区归位的。
    pub returned: Vec<String>,
    /// 解绑后移入停放区的。
    pub parked: Vec<String>,
    /// 找不到正文来源的（仓库与内嵌副本都没有）。
    pub missing_source: Vec<String>,
    /// 写盘失败的，逐条带原因。
    pub failed: Vec<String>,
    pub note: String,
}

/// 按当前 Profile 的 `rules.json` 收敛一次全局 `skills/`（OD-ARCH-130）。
///
/// 与 `materialize` 的区别只在**触发点**：这条是「用户刚改完绑定」时用的窄入口，
/// 不写出厂配置、不探测插件、不联网。三件事复用既有原语：
///   * 需要的从停放区归位、不要的移入停放区（`reconcile_managed_skills`）；
///   * 缺的按「仓库优先、内嵌兜底」补写，用户改过的不覆盖（`sync_skill_files`）；
///   * 补完登记归属，切走时才知道该收回哪些。
///
/// 基础标准恒为空操作：不建目录、不写登记表——干净底座不能被一次保存动作弄脏。
pub fn sync_bound_skills(scope: &Scope<'_>, standard: &str) -> Result<BoundSkillSync, String> {
    let profile = scope.profile.trim().to_string();
    if !crate::settings::is_valid_profile_name(&profile) {
        return Err(format!("Profile 名不合法：{:?}", profile));
    }
    if profile == standard {
        return Ok(BoundSkillSync {
            profile,
            standard: true,
            note: "基础标准是干净底座：不写入任何 Skill。".to_string(),
            ..Default::default()
        });
    }
    crate::capabilities::assert_writable_root(scope.dsh_home)?;
    let wanted = wanted_skills(scope, standard);
    let mut report = BoundSkillSync {
        profile: profile.clone(),
        wanted: wanted.clone(),
        ..Default::default()
    };
    // 先收敛归属：解绑的移入停放区，绑回来的从停放区归位。
    let flipped = reconcile_managed_skills(scope, standard)?;
    report.returned = flipped.returned;
    report.parked = flipped.parked;
    if wanted.is_empty() {
        report.note = "这个 Profile 现在没有绑定任何 Skill；原先由 OmniDesk 写入的已移入停放区。"
            .to_string();
        return Ok(report);
    }
    let app_profile_dir = scope.app_root.join("profiles").join(&profile);
    let state_path = app_profile_dir.join(ASSET_STATE_FILE);
    let mut state = load_state(&state_path);
    let skills_root = scope.dsh_home.join("skills");
    let repo_skills = scope.repo_root.map(|r| r.join("skills"));
    for name in &wanted {
        let dst = skills_root.join(name);
        let Some((files, _embedded)) = expected_skill_files(scope, name, repo_skills.as_deref()) else {
            report.missing_source.push(name.clone());
            continue;
        };
        match sync_skill_files(name, &dst, files, &mut state, false) {
            Ok((status, _detail)) => match status.as_str() {
                STATUS_CREATED | STATUS_UPDATED => report.filled.push(name.clone()),
                STATUS_MISSING | STATUS_MISSING_SOURCE => report.missing_source.push(name.clone()),
                _ => report.kept.push(name.clone()),
            },
            Err(e) => report.failed.push(format!("{}：{}", name, e)),
        }
    }
    // 装配状态记录「哪些文件是 OmniDesk 写的」，缺了它下次巡检会把出厂内容当成用户改动。
    // 写不进去不影响本次补齐，但必须在回执里说明。
    let state_error = fs::create_dir_all(&app_profile_dir)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", app_profile_dir.display(), e))
        .and_then(|_| save_state(&state_path, &mut state));
    if let Err(e) = state_error {
        report.failed.push(format!("装配状态未写入：{}", e));
    }
    record_managed_skills(scope, standard)?;
    report.note = match (report.filled.len(), report.kept.len(), report.missing_source.len()) {
        (0, 0, 0) => "绑定里没有 Skill，运行时目录未改动。".to_string(),
        (filled, 0, 0) => format!("已把 {} 个 Skill 放进运行时目录，下一轮提交即可用。", filled),
        (0, kept, 0) => format!("{} 个 Skill 本来就在运行时目录里，无需重复写入。", kept),
        (filled, kept, 0) => format!(
            "已补 {} 个 Skill 进运行时目录，另 {} 个本来就位；下一轮提交即可用。",
            filled, kept
        ),
        (filled, kept, missing) => format!(
            "已补 {} 个、本来就位 {} 个，还有 {} 个找不到正文来源（仓库与内嵌副本都没有）。",
            filled, kept, missing
        ),
    };
    Ok(report)
}

/// 三态写入：新建 / 更新（上次是我们写的）/ 不覆盖（用户改过）。
///
/// \`overwrite_drift\` 只在用户显式点「恢复出厂内容」时为 true：此时被改过的
/// 文件先备份成 \`.bak-<epoch>\` 再按出厂内容写回；日常装配永远传 false。
fn sync_bytes(
    key: &str,
    path: &Path,
    bytes: &[u8],
    state: &mut AssetState,
    overwrite_drift: bool,
) -> Result<(String, String), String> {
    let want = crate::sha256::sha256_hex(bytes);
    let existing = match fs::read(path) {
        Ok(v) => Some(v),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("读取 {} 失败：{}", path.display(), e)),
    };
    match existing {
        None => {
            crate::model_settings::write_atomic(path, bytes)?;
            state.written.insert(key.to_string(), want);
            Ok((STATUS_CREATED.to_string(), "已补写出厂内容".to_string()))
        }
        Some(cur) => {
            let cur_sha = crate::sha256::sha256_hex(&cur);
            if cur_sha == want {
                state.written.insert(key.to_string(), want);
                Ok((STATUS_UNCHANGED.to_string(), "已是最新出厂内容".to_string()))
            } else if state.written.get(key) == Some(&cur_sha) {
                let backup = backup_of(path)?;
                crate::model_settings::write_atomic(path, bytes)?;
                state.written.insert(key.to_string(), want);
                Ok((
                    STATUS_UPDATED.to_string(),
                    match backup {
                        Some(b) => format!("已更新出厂内容，旧文件备份为 {}", b.display()),
                        None => "已更新出厂内容".to_string(),
                    },
                ))
            } else if overwrite_drift {
                // 用户显式要求恢复：先备份，再按出厂内容写回。
                let backup = backup_of(path)?;
                crate::model_settings::write_atomic(path, bytes)?;
                state.written.insert(key.to_string(), want);
                Ok((
                    STATUS_UPDATED.to_string(),
                    match backup {
                        Some(b) => format!("已按出厂内容恢复，旧文件备份为 {}", b.display()),
                        None => "已按出厂内容恢复".to_string(),
                    },
                ))
            } else {
                // 内容不等于出厂值，也不是我们上次写的 → 用户改过，一律不动。
                Ok((
                    STATUS_DRIFT.to_string(),
                    "文件已被修改，未覆盖；可在资产页点「恢复出厂内容」按出厂版覆盖（会先备份）"
                        .to_string(),
                ))
            }
        }
    }
}

fn load_state(path: &Path) -> AssetState {
    let Ok(text) = fs::read_to_string(path) else {
        return AssetState::default();
    };
    match serde_json::from_str::<AssetState>(&text) {
        Ok(mut s) if s.schema == ASSET_STATE_SCHEMA => {
            s.schema = ASSET_STATE_SCHEMA.to_string();
            s
        }
        // 状态文件坏了就当作「没有记录」：结果只会是更保守的 drift，绝不会误覆盖。
        _ => AssetState::default(),
    }
}

fn save_state(path: &Path, state: &mut AssetState) -> Result<(), String> {
    state.schema = ASSET_STATE_SCHEMA.to_string();
    let mut bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| format!("序列化装配状态失败：{}", e))?;
    bytes.push(b'\n');
    crate::model_settings::write_atomic(path, &bytes)
}

fn collect_files(src: &Path, dst: &Path, out: &mut Vec<(PathBuf, PathBuf)>) -> Result<(), String> {
    let entries = fs::read_dir(src)
        .map_err(|e| format!("读取资产目录失败（{}）：{}", src.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取资产目录项失败（{}）：{}", src.display(), e))?;
        let name = entry.file_name();
        let s = src.join(&name);
        let d = dst.join(&name);
        if s.is_dir() {
            collect_files(&s, &d, out)?;
        } else if s.is_file() {
            out.push((s, d));
        }
    }
    Ok(())
}

/// 把一个 Skill 的所有文件按「三态」写入，并统一登记 hash。
///
/// 仓库源与内嵌副本都走这里，因此同一份 Skill 在两处来源之间切换时，
/// 状态是「unchanged / updated」，不会因为换了来源被判成「用户改过」。
fn sync_skill_files(
    name: &str,
    dst: &Path,
    files: Vec<(String, Vec<u8>)>,
    state: &mut AssetState,
    overwrite_drift: bool,
) -> Result<(String, String), String> {
    let known = files
        .iter()
        .any(|(rel, _)| state.written.contains_key(&format!("skill:{}:{}", name, rel)));
    let mut created = 0usize;
    let mut updated = 0usize;
    let mut drift = 0usize;
    for (rel, body) in files {
        let key = format!("skill:{}:{}", name, rel);
        let (status, _) =
            sync_bytes(&key, &dst.join(&rel), &body, state, overwrite_drift)?;
        match status.as_str() {
            STATUS_CREATED => created += 1,
            STATUS_UPDATED => updated += 1,
            STATUS_DRIFT => drift += 1,
            _ => {}
        }
    }
    if created > 0 {
        return Ok((
            STATUS_CREATED.to_string(),
            format!("已补入 {} 个文件", created),
        ));
    }
    if updated > 0 {
        return Ok((
            STATUS_UPDATED.to_string(),
            format!("已更新 {} 个文件（旧版已备份）", updated),
        ));
    }
    if drift > 0 && known {
        return Ok((
            STATUS_DRIFT.to_string(),
            "文件已被你修改，未覆盖".to_string(),
        ));
    }
    Ok((
        STATUS_UNCHANGED.to_string(),
        if drift > 0 {
            "已存在，保留现有内容".to_string()
        } else {
            "已是最新内容".to_string()
        },
    ))
}

/// 读取仓库里某个 Skill 目录的全部文件，转成与内嵌副本相同的形态。
fn repo_skill_files(src: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    collect_files(src, Path::new(""), &mut pairs)?;
    let mut out = Vec::with_capacity(pairs.len());
    for (path, rel) in pairs {
        let name = rel.to_string_lossy().replace('\\', "/");
        let body = fs::read(&path)
            .map_err(|e| format!("读取 Skill 源失败（{}）：{}", path.display(), e))?;
        out.push((name, body));
    }
    Ok(out)
}

/// Skill 正文的三种来源（OD-ARCH-131）。
///
/// 自建 Profile 可以把 Skill 放进自己的目录，此时它是**这个 Profile 的定制**，
/// 优先级最高；内置 8 个 Skill 仍按「仓库优先、内嵌兜底」取，保持出厂行为不变。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillSource {
    /// `<应用数据根>/profiles/<名字>/skills/<skill名>/`
    Profile,
    /// 仓库 `skills/<skill名>/`（开发环境）
    Repo,
    /// 编译期内嵌副本（分发环境）
    Embedded,
}

impl SkillSource {
    fn is_embedded(self) -> bool {
        matches!(self, SkillSource::Embedded)
    }
}

/// 某个 Skill 的正文清单，附来源标记，供装配与巡检共用。
///
/// 返回 \`None\` 表示三处都没有源，调用方应报 \`missing_source\`。
fn expected_skill_files(
    scope: &Scope<'_>,
    name: &str,
    repo_skills: Option<&Path>,
) -> Option<(Vec<(String, Vec<u8>)>, SkillSource)> {
    // 名字先过形状校验再拼路径：三处来源都按名字拼目录，`..` 不能逃逸。
    if !crate::profile_rules::is_valid_skill_name(name) {
        return None;
    }
    // 1) 该 Profile 自己带的（仅自建 Profile 有这一层；基础标准不会走到这里）。
    let local = scope
        .app_root
        .join("profiles")
        .join(scope.profile.trim())
        .join("skills")
        .join(name)
        .join("SKILL.md");
    if local.is_file() {
        let dir = local.parent().map(Path::to_path_buf)?;
        if let Ok(files) = repo_skill_files(&dir) {
            return Some((files, SkillSource::Profile));
        }
    }
    // 2) 仓库源（开发环境）。
    if let Some(src) = repo_skills
        .map(|root| root.join(name))
        .filter(|p| p.join("SKILL.md").is_file())
    {
        if let Ok(files) = repo_skill_files(&src) {
            return Some((files, SkillSource::Repo));
        }
    }
    // 3) 编译期内嵌副本（分发环境）。
    crate::profile_seed::skill(name).map(|seed| {
        let files = seed
            .files
            .iter()
            .map(|f| (f.rel.to_string(), f.body.as_bytes().to_vec()))
            .collect();
        (files, SkillSource::Embedded)
    })
}

/// 巡检一个 Skill：逐文件比摘要，得出「齐了 / 有新版 / 被改过 / 缺文件」。
///
/// 只看 \`SKILL.md\` 在不在会把「用户改了正文」当成「已就位」，那是假绿。
fn inspect_skill(
    name: &str,
    dst: &Path,
    files: &[(String, Vec<u8>)],
    state: &AssetState,
) -> (String, String) {
    let mut missing = 0usize;
    let mut drift = 0usize;
    let mut stale = 0usize;
    for (rel, body) in files {
        let path = dst.join(rel);
        let cur = match fs::read(&path) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                missing += 1;
                continue;
            }
            Err(e) => {
                return (
                    STATUS_MISSING.to_string(),
                    format!("读取 {} 失败：{}", path.display(), e),
                )
            }
        };
        let cur_sha = crate::sha256::sha256_hex(&cur);
        let want = crate::sha256::sha256_hex(body);
        if cur_sha == want {
            continue;
        }
        let key = format!("skill:{}:{}", name, rel);
        if state.written.get(&key) == Some(&cur_sha) {
            stale += 1;
        } else {
            drift += 1;
        }
    }
    if drift > 0 {
        return (
            STATUS_DRIFT.to_string(),
            format!("{} 个文件与出厂内容不同（可在资产页恢复出厂内容）", drift),
        );
    }
    if missing > 0 {
        return (
            STATUS_MISSING.to_string(),
            format!("缺 {} 个文件，切换 Profile 或重启时会补写", missing),
        );
    }
    if stale > 0 {
        return (
            STATUS_STALE.to_string(),
            format!("有新版出厂内容（{} 个文件），切换时会自动更新", stale),
        );
    }
    (STATUS_READY.to_string(), "已就位".to_string())
}

/// 写作类 Skill 的 `references/` 是生成副本：只补缺失，用户改过的同样不覆盖。
fn materialize_references(
    key_prefix: &str,
    skill_dir: &Path,
    repo_root: Option<&Path>,
    state: &mut AssetState,
    overwrite_drift: bool,
) -> Result<(String, String), String> {
    // 与 scripts/sync-skills.sh 第 5 节写入的说明保持一致，避免两处说法不同。
    const REFERENCES_README: &str = "# 材料写作资产（运行时物化副本）\n\n\
本目录由 OmniDesk 从仓库权威源自动生成，请勿在此处直接编辑。\n\
修改请回到仓库源，然后重新切换 Profile 或重新同步。\n\n\
| 运行时路径 | 仓库权威源 |\n\
| --- | --- |\n\
| references/templates/ | templates/discipline/ |\n\
| references/writing-style/ | knowledge/writing-style/ |\n\
| references/excellent-cases/ | knowledge/excellent-cases/ |\n\n\
历史范例目录若仅有 README.md，说明尚无范例；Skill 必须如实说明未使用历史范例。\n";
    let pairs: [(&str, PathBuf); 3] = [
        (
            "templates",
            PathBuf::from("templates").join("discipline"),
        ),
        (
            "writing-style",
            PathBuf::from("knowledge").join("writing-style"),
        ),
        (
            "excellent-cases",
            PathBuf::from("knowledge").join("excellent-cases"),
        ),
    ];
    let repo_root = repo_root.filter(|r| r.is_dir());
    let mut created = 0usize;
    let mut drift = 0usize;
    let mut unchanged = 0usize;
    let mut missing = 0usize;
    // 说明文件本身就属于 references/，先写它，内容固定。
    let (readme_status, _) = sync_bytes(
        &format!("{}:_README", key_prefix),
        &skill_dir.join("references").join("README.md"),
        REFERENCES_README.as_bytes(),
        state,
        overwrite_drift,
    )?;
    match readme_status.as_str() {
        STATUS_CREATED => created += 1,
        STATUS_DRIFT => drift += 1,
        _ => unchanged += 1,
    }
    for (sub, rel_src) in pairs {
        let src = repo_root.map(|r| r.join(&rel_src));
        let dst = skill_dir.join("references").join(sub);
        // 仓库源优先；分发到用户机器后仓库不存在，改用编译期内嵌副本。
        if let Some(src) = src.filter(|p| p.is_dir()) {
            let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
            collect_files(&src, &dst, &mut files)?;
            for (s, d) in files {
                let rel = s
                    .strip_prefix(&src)
                    .unwrap_or(&s)
                    .to_string_lossy()
                    .replace('\\', "/");
                let body = fs::read(&s)
                    .map_err(|e| format!("读取资产源失败（{}）：{}", s.display(), e))?;
                let key = format!("{}:{}/{}", key_prefix, sub, rel);
                let (status, _) = sync_bytes(&key, &d, &body, state, overwrite_drift)?;
                match status.as_str() {
                    STATUS_CREATED => created += 1,
                    STATUS_DRIFT => drift += 1,
                    _ => unchanged += 1,
                }
            }
            continue;
        }
        let Some(seed) = crate::profile_seed::ASSETS.iter().find(|a| a.sub == sub) else {
            missing += 1;
            continue;
        };
        for file in seed.files {
            let key = format!("{}:{}/{}", key_prefix, sub, file.rel);
            let (status, _) = sync_bytes(
                &key,
                &dst.join(file.rel),
                file.body.as_bytes(),
                state,
                overwrite_drift,
            )?;
            match status.as_str() {
                STATUS_CREATED => created += 1,
                STATUS_DRIFT => drift += 1,
                _ => unchanged += 1,
            }
        }
    }
    if drift > 0 {
        return Ok((
            STATUS_DRIFT.to_string(),
            format!("{} 个引用文件被修改，未覆盖", drift),
        ));
    }
    if created > 0 {
        return Ok((
            STATUS_CREATED.to_string(),
            format!("已物化 {} 个引用文件", created),
        ));
    }
    if missing > 0 {
        return Ok((
            STATUS_MISSING_SOURCE.to_string(),
            "缺少写作资产源且无内嵌副本，references 未完整物化".to_string(),
        ));
    }
    Ok((
        STATUS_UNCHANGED.to_string(),
        format!("{} 个引用文件已就位", unchanged),
    ))
}

fn record_file(
    key: &str,
    kind: &str,
    label: &str,
    path: &Path,
    body: &str,
    state: &mut AssetState,
    overwrite_drift: bool,
) -> Result<AssetItem, String> {
    let (status, detail) = sync_bytes(key, path, body.as_bytes(), state, overwrite_drift)?;
    Ok(AssetItem {
        key: key.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        path: path.display().to_string(),
        status,
        detail,
    })
}

fn runtime_cli_under(root: &Path) -> PathBuf {
    root.join("runtime")
        .join("dsh-runtime")
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join("lib")
        .join("bin.js")
}

/// 随运行时携带的 DSH CLI 路径（只用于插件探测与安装命令）。
///
/// 开发机按仓库根找；用户机器上仓库不存在，于是再试应用数据根同级的
/// `<OmniDesk>/runtime/dsh-runtime` 与 `$HOME/OmniDesk/runtime/...`。
/// 都没有就返回首选候选路径，由上层如实报「找不到 DSH CLI」。
pub fn default_dsh_cli() -> PathBuf {
    let repo = crate::task_shell::repo_root_hint().ok();
    let app_root = crate::app_paths::AppPaths::resolve()
        .ok()
        .map(|paths| paths.root);
    default_dsh_cli_for(repo.as_deref(), app_root.as_deref())
}

/// 同一套候选顺序的显式版本（OD-ARCH-112）。
///
/// 引擎启动与插件探测必须得出**同一个** CLI：传同一组 `repo_root` / `app_root`，
/// 不要在两边各写一遍推导。`app_root` 是应用数据根（`<OmniDesk>/app`），
/// 运行时在它的同级 `runtime/dsh-runtime`。
pub fn default_dsh_cli_for(repo_root: Option<&Path>, app_root: Option<&Path>) -> PathBuf {
    if let Some(raw) = std::env::var_os("OMNIDESK_DSH_CLI") {
        if !raw.is_empty() {
            return PathBuf::from(raw);
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    locate_cli(repo_root, app_root, home.as_deref())
}

/// 纯路径推导（不读环境变量），便于单元测试与「两边必须同源」的比对。
pub fn locate_cli(
    repo_root: Option<&Path>,
    app_root: Option<&Path>,
    home: Option<&Path>,
) -> PathBuf {
    let mut fallback: Vec<PathBuf> = Vec::new();
    if let Some(repo) = repo_root {
        let candidate = runtime_cli_under(repo);
        if candidate.is_file() {
            return candidate;
        }
        fallback.push(candidate);
    }
    if let Some(app_root) = app_root {
        // 默认布局：$HOME/OmniDesk/{app,runtime}，运行时与 app 平级。
        if let Some(sibling) = app_root.parent() {
            let candidate = runtime_cli_under(sibling);
            if candidate.is_file() {
                return candidate;
            }
            fallback.push(candidate);
        }
    }
    if let Some(home) = home {
        let candidate = runtime_cli_under(&home.join("OmniDesk"));
        if candidate.is_file() {
            return candidate;
        }
        fallback.push(candidate);
    }
    fallback.into_iter().next().unwrap_or_default()
}

/// 按出厂清单把资产补齐到隔离根。标准 Profile 直接返回、零写入。
pub fn materialize(scope: &Scope<'_>, standard: &str) -> Result<MaterializeReport, String> {
    materialize_with(scope, standard, false)
}

/// 显式版本（OD-ARCH-113）：\`overwrite_drift = true\` 表示用户点了「恢复出厂内容」，
/// 被改过的文件会先备份成 \`.bak-<epoch>\` 再按出厂内容写回。
pub fn materialize_with(
    scope: &Scope<'_>,
    standard: &str,
    overwrite_drift: bool,
) -> Result<MaterializeReport, String> {
    let profile = scope.profile.trim();
    if profile == standard {
        return Ok(MaterializeReport {
            profile: profile.to_string(),
            label: "基础标准".to_string(),
            standard: true,
            items: Vec::new(),
            summary_note: None,
            notes: vec!["基础标准是干净底座：不安装任何 Profile 资产。".to_string()],
            repo_missing: None,
        });
    }
    if !crate::settings::is_valid_profile_name(profile) {
        return Err(format!("Profile 名不合法：{:?}", profile));
    }
    crate::capabilities::assert_writable_root(scope.dsh_home)?;
    let dsh_profile_dir = scope.dsh_home.join("profiles").join(profile);
    let app_profile_dir = scope.app_root.join("profiles").join(profile);
    let state_path = app_profile_dir.join(ASSET_STATE_FILE);
    let mut state = load_state(&state_path);

    let Some(asset) = asset_profile(profile) else {
        let report = materialize_custom(scope, overwrite_drift);
        // 自定义 Profile 补进去的 Skill 同样归 OmniDesk 管，下次切走要能收回。
        record_managed_skills(scope, standard)?;
        return Ok(report);
    };

    fs::create_dir_all(&dsh_profile_dir)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", dsh_profile_dir.display(), e))?;
    fs::create_dir_all(&app_profile_dir)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", app_profile_dir.display(), e))?;

    let mut items: Vec<AssetItem> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    items.push(record_file(
        "package.json",
        "config",
        "Profile 配置",
        &dsh_profile_dir.join("package.json"),
        asset.manifest,
        &mut state,
        overwrite_drift,
    )?);
    items.push(record_file(
        "cordis.patch.yml",
        "patch",
        "场景与插件补丁",
        &dsh_profile_dir.join("cordis.patch.yml"),
        asset.patch,
        &mut state,
        overwrite_drift,
    )?);
    if let Some(decl) = asset.decl {
        items.push(record_file(
            "decl.yml",
            "decl",
            "场景声明",
            &app_profile_dir.join("decl.yml"),
            decl,
            &mut state,
            overwrite_drift,
        )?);
    }
    if let Some(rules) = asset.rules {
        items.push(record_file(
            "rules.json",
            "rules",
            "行业规则包",
            &app_profile_dir.join("rules.json"),
            rules,
            &mut state,
            overwrite_drift,
        )?);
    }

    // ---- Skill：Profile 自带优先，其次仓库 skills/，分发环境用编译期内嵌副本 ----
    let skills_root = scope.dsh_home.join("skills");
    let repo_skills = scope.repo_root.map(|r| r.join("skills"));
    let mut repo_missing: Option<String> = None;
    for name in asset.skills {
        let dst = skills_root.join(name);
        let mut item = AssetItem {
            key: format!("skill:{}", name),
            kind: "skill".to_string(),
            label: format!("Skill · {}", name),
            path: dst.display().to_string(),
            status: STATUS_UNCHANGED.to_string(),
            detail: "已存在，保留现有内容".to_string(),
        };
        match expected_skill_files(scope, name, repo_skills.as_deref()) {
            Some((files, source)) => {
                let (status, detail) =
                    sync_skill_files(name, &dst, files, &mut state, overwrite_drift)?;
                item.status = status;
                item.detail = if source.is_embedded() {
                    format!("{}（内嵌）", detail)
                } else {
                    detail
                };
            }
            None => {
                repo_missing = Some(format!(
                    "Skill {} 在 Profile 目录、仓库与内嵌清单里都找不到。",
                    name
                ));
                item.status = STATUS_MISSING_SOURCE.to_string();
                item.detail = "找不到该 Skill 的正文来源".to_string();
            }
        }
        // 写作类 Skill 的 references 生成副本
        if matches!(*name, "material-writing" | "document-review") && dst.is_dir() {
            if let Ok((ref_status, ref_detail)) = materialize_references(
                &format!("ref:{}", name),
                &dst,
                scope.repo_root,
                &mut state,
                overwrite_drift,
            ) {
                if ref_status != STATUS_UNCHANGED && item.status == STATUS_UNCHANGED {
                    item.status = ref_status;
                }
                item.detail = format!("{}；{}", item.detail, ref_detail);
            }
        }
        items.push(item);
    }

    // ---- 插件：只探测，不联网 ----
    let dsh_cli = default_dsh_cli();
    for req in asset.plugins {
        let probe = probe_plugin(req, scope.dsh_home, &dsh_profile_dir, &dsh_cli);
        items.push(AssetItem {
            key: format!("plugin:{}", req.package),
            kind: "plugin".to_string(),
            label: format!("插件 · {}", req.package),
            path: probe.path,
            status: probe.status,
            detail: probe.detail,
        });
    }

    save_state(&state_path, &mut state)?;
    if items.iter().any(|i| i.status == STATUS_DRIFT) {
        notes.push("有文件被你改过，本模块没有覆盖。需要出厂内容时先备份再删除该文件，下次切换会补回。".to_string());
    }
    notes.push(format!(
        "装配状态记录在 {}（只记摘要，不记内容）。",
        state_path.display()
    ));

    // 装配完成后登记归属：下次切到不需要这些 Skill 的 Profile 时才收得回。
    record_managed_skills(scope, standard)?;

    Ok(MaterializeReport {
        profile: profile.to_string(),
        label: asset.label.to_string(),
        summary_note: Some(asset.summary.to_string()),
        standard: false,
        items,
        notes,
        repo_missing,
    })
}

/// 自建 Profile 的装配：不写出厂配置（它没有出厂清单），只做两件「跟着走」的事：
///   1. 规则里 `skill_bindings` 点到的 Skill，缺哪个从仓库 `skills/` 补哪个；
///   2. 依赖里声明、本地又解析不到的插件，登记进清单，供资产页一键安装。
/// 已有的 Skill 目录与登记过的插件列表都不覆盖；未登记的历史目录仍然零写入。
fn materialize_custom(scope: &Scope<'_>, overwrite_drift: bool) -> MaterializeReport {
    let profile = scope.profile.trim();
    let mut registry = crate::profile_lifecycle::load_registry(scope.app_root);
    let Some(entry) = registry.profiles.iter().find(|p| p.name == profile).cloned() else {
        return inspect_custom(scope, profile);
    };
    let mut items: Vec<AssetItem> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let app_profile_dir = scope.app_root.join("profiles").join(profile);
    let dsh_profile_dir = scope.dsh_home.join("profiles").join(profile);

    // ---- Skill：从规则声明的绑定里取，缺则补 ----
    // 取源顺序统一在 expected_skill_files 里：Profile 自带 → 仓库 → 内嵌（OD-ARCH-131）。
    let wanted = wanted_skills(scope, "");
    let skills_root = scope.dsh_home.join("skills");
    let repo_skills = scope.repo_root.map(|r| r.join("skills"));
    let mut repo_missing: Option<String> = None;
    let state_path = app_profile_dir.join(ASSET_STATE_FILE);
    let mut state = load_state(&state_path);
    for name in &wanted {
        let dst = skills_root.join(name);
        let mut item = AssetItem {
            key: format!("skill:{}", name),
            kind: "skill".to_string(),
            label: format!("Skill · {}", name),
            path: dst.display().to_string(),
            status: STATUS_UNCHANGED.to_string(),
            detail: "已存在，保留现有内容".to_string(),
        };
        match expected_skill_files(scope, name, repo_skills.as_deref()) {
            Some((files, source)) => {
                match sync_skill_files(name, &dst, files, &mut state, overwrite_drift) {
                    Ok((status, detail)) => {
                        item.status = status;
                        item.detail = if source.is_embedded() {
                            format!("{}（内嵌）", detail)
                        } else {
                            detail
                        };
                    }
                    Err(e) => {
                        item.status = STATUS_MISSING.to_string();
                        item.detail = e;
                    }
                }
            }
            None => {
                repo_missing = Some(format!(
                    "Skill {} 在 Profile 目录、仓库与内嵌清单里都找不到。",
                    name
                ));
                item.status = STATUS_MISSING_SOURCE.to_string();
                item.detail = "找不到该 Skill 的正文来源".to_string();
            }
        }
        items.push(item);
    }
    if !wanted.is_empty() {
        notes.push(format!(
            "Skill 绑定来自该 Profile 的 rules.json（{} 个），已补齐缺失项，不覆盖已有目录。",
            wanted.len()
        ));
    }
    fs::create_dir_all(&app_profile_dir).ok();
    if let Err(e) = save_state(&state_path, &mut state) {
        notes.push(format!("Skill 装配状态未写入：{}", e));
    }

    // ---- 插件：登记项 + 新发现的缺失依赖都探测一遍 ----
    let cli = default_dsh_cli();
    let detected = crate::profile_lifecycle::detect_plugins(&dsh_profile_dir);
    let mut packages = entry.plugins.clone();
    let mut newly = Vec::new();
    for name in detected {
        if packages.iter().any(|p| p == &name) {
            continue;
        }
        let dep = fs::read_to_string(dsh_profile_dir.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("dependencies").and_then(|d| d.as_object().cloned()))
            .map(|m| m.contains_key(&name))
            .unwrap_or(false);
        if dep || !package_available(scope.dsh_home, &name, &cli) {
            newly.push(name.clone());
            packages.push(name);
        }
    }
    for name in &packages {
        let probe = probe_package(name, None, scope.dsh_home, &dsh_profile_dir, &cli);
        items.push(AssetItem {
            key: format!("plugin:{}", name),
            kind: "plugin".to_string(),
            label: format!("插件 · {}", name),
            path: probe.path,
            status: probe.status,
            detail: probe.detail,
        });
    }
    if !newly.is_empty() {
        if let Some(entry) = registry.profiles.iter_mut().find(|p| p.name == profile) {
            entry.plugins = packages.clone();
        }
        if let Err(e) = crate::profile_lifecycle::save_registry_public(scope.app_root, &registry) {
            notes.push(format!("插件清单未写入登记表：{}", e));
        }
    }
    if packages.is_empty() {
        notes.push("这个 Profile 没有声明额外插件。".to_string());
    }
    notes.push(
        "自定义 Profile 不写出厂配置；只用规律补 Skill、把缺的插件列出来供你点装。".to_string(),
    );

    MaterializeReport {
        profile: profile.to_string(),
        label: entry.label.clone(),
        summary_note: Some(if entry.origin == "standard" {
            "自定义 Profile · 从基础标准起手".to_string()
        } else {
            format!("自定义 Profile · 复制自 {}", entry.origin.trim_start_matches("copy:"))
        }),
        standard: false,
        items,
        notes,
        repo_missing,
    }
}

/// 但本地解析不到」的插件列出来，让用户能看见缺什么并按需安装。
fn inspect_custom(scope: &Scope<'_>, profile: &str) -> MaterializeReport {
    let registry = crate::profile_lifecycle::load_registry(scope.app_root);
    let entry = registry.profiles.iter().find(|p| p.name == profile);
    if entry.is_none() {
        return MaterializeReport {
            profile: profile.to_string(),
            label: profile.to_string(),
            standard: false,
            items: Vec::new(),
            summary_note: None,
            notes: vec![
                "这是历史遗留的自定义 Profile（不在 OmniDesk 的自定义清单里）：本模块不写入任何资产。"
                    .to_string(),
            ],
            repo_missing: None,
        };
    }
    let label = entry
        .map(|p| p.label.clone())
        .unwrap_or_else(|| profile.to_string());
    let dsh_profile_dir = scope.dsh_home.join("profiles").join(profile);
    let cli = default_dsh_cli();
    let mut items: Vec<AssetItem> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut repo_missing: Option<String> = None;

    // ---- Skill：与装配同一张清单（wanted_skills），只判在不在、内容对不对 ----
    //
    // OD-ARCH-122 缺口 A：装配会按 rules.json 的 skill_bindings 补 Skill，体检却只看
    // 插件，于是「用户删了目录」在设置页看不出来，要等下一次切换才被悄悄补回。
    // 这里复用装配用的 expected_skill_files / inspect_skill，状态词与出厂 Profile 一致。
    // 只读：不建目录、不写状态文件、不补任何文件。
    let wanted = wanted_skills(scope, "");
    let skills_root = scope.dsh_home.join("skills");
    let repo_skills = scope.repo_root.map(|r| r.join("skills"));
    let state = load_state(&scope.app_root.join("profiles").join(profile).join(ASSET_STATE_FILE));
    for name in &wanted {
        let dst = skills_root.join(name);
        let (status, detail) = match expected_skill_files(scope, name, repo_skills.as_deref()) {
            Some((files, source)) => {
                let (status, detail) = inspect_skill(name, &dst, &files, &state);
                let suffix = source.is_embedded() && status != STATUS_READY;
                (
                    status,
                    if suffix && detail != "已就位" {
                        format!("{}（内嵌）", detail)
                    } else {
                        detail
                    },
                )
            }
            None => {
                repo_missing = Some(format!(
                    "Skill {} 在 Profile 目录、仓库与内嵌清单里都找不到。",
                    name
                ));
                (
                    STATUS_MISSING_SOURCE.to_string(),
                    "没有该 Skill 的正文来源".to_string(),
                )
            }
        };
        items.push(AssetItem {
            key: format!("skill:{}", name),
            kind: "skill".to_string(),
            label: format!("Skill · {}", name),
            path: dst.display().to_string(),
            status,
            detail,
        });
    }
    if !wanted.is_empty() {
        notes.push(format!(
            "本 Profile 的 rules.json 绑定了 {} 个 Skill；切换或重启时自动补齐缺失项，不覆盖已有目录。",
            wanted.len()
        ));
    }

    // 插件清单与装配同源（OD-ARCH-128）：登记表 ∪ 磁盘上真实存在的依赖。
    // 只读路径不能回写登记表，但至少要如实列出「命令行装过、只是没切换过」的包，
    // 否则同一份工作台在巡检页和装配时的结论会不一样。
    let mut packages = entry.map(|p| p.plugins.clone()).unwrap_or_default();
    for name in crate::profile_lifecycle::detect_plugins(&dsh_profile_dir) {
        if !packages.iter().any(|p| parse_plugin_spec(p).package == name) {
            packages.push(name);
        }
    }
    for name in packages {
        let spec = parse_plugin_spec(&name);
        let probe = probe_package(
            &spec.package,
            spec.version.as_deref(),
            scope.dsh_home,
            &dsh_profile_dir,
            &cli,
        );
        items.push(AssetItem {
            // 键用包根：界面拿它当安装参数回传，带版本或子路径会被安装通道拒绝。
            key: format!("plugin:{}", spec.package),
            kind: "plugin".to_string(),
            label: format!("插件 · {}", name),
            path: probe.path,
            status: probe.status,
            detail: probe.detail,
        });
    }
    MaterializeReport {
        profile: profile.to_string(),
        label,
        standard: false,
        items,
        summary_note: entry.map(|p| {
            if p.origin == "standard" {
                "自建 Profile · 从基础标准起手".to_string()
            } else {
                format!("自建 Profile · 复制自 {}", p.origin.trim_start_matches("copy:"))
            }
        }),
        notes: {
            notes.push(
                "自定义 Profile 不写出厂配置：只按 rules.json 补 Skill、按登记表检查插件。"
                    .to_string(),
            );
            notes
        },
        repo_missing,
    }
}

/// 只读巡检：和 `materialize` 用同一张清单判定「这一项在不在、内容对不对」，
/// 但**不写任何文件**（设置页打开时用，不能因为看一眼就改盘）。
pub fn inspect(scope: &Scope<'_>, standard: &str) -> Result<MaterializeReport, String> {
    let profile = scope.profile.trim();
    if profile == standard {
        return Ok(MaterializeReport {
            profile: profile.to_string(),
            label: "基础标准".to_string(),
            standard: true,
            items: Vec::new(),
            summary_note: None,
            notes: vec!["基础标准是干净底座：不安装任何 Profile 资产，也不写入行业内容。".to_string()],
            repo_missing: None,
        });
    }
    let Some(asset) = asset_profile(profile) else {
        return Ok(inspect_custom(scope, profile));
    };
    let dsh_profile_dir = scope.dsh_home.join("profiles").join(profile);
    let app_profile_dir = scope.app_root.join("profiles").join(profile);
    let state = load_state(&app_profile_dir.join(ASSET_STATE_FILE));
    let mut items: Vec<AssetItem> = Vec::new();

    let mut inspect_file = |key: &str, kind: &str, label: &str, path: &Path, body: &str| {
        let status = match fs::read(path) {
            Ok(cur) => {
                let cur_sha = crate::sha256::sha256_hex(&cur);
                let want = crate::sha256::sha256_hex(body.as_bytes());
                if cur_sha == want {
                    STATUS_READY
                } else if state.written.get(key) == Some(&cur_sha) {
                    // 还是我们上次写的旧版本：可以安全更新
                    "stale"
                } else {
                    STATUS_DRIFT
                }
            }
            Err(_) => STATUS_MISSING,
        };
        items.push(AssetItem {
            key: key.to_string(),
            kind: kind.to_string(),
            label: label.to_string(),
            path: path.display().to_string(),
            status: status.to_string(),
            detail: match status {
                STATUS_READY => "已就位".to_string(),
                "stale" => "有新版出厂内容，切换时会自动更新".to_string(),
                STATUS_DRIFT => {
                    "文件与出厂内容不同，切换时不会覆盖；可在资产页恢复出厂内容".to_string()
                }
                _ => "缺失，切换 Profile 或重启时会补写，也可在资产页立即修复".to_string(),
            },
        });
    };
    inspect_file(
        "package.json",
        "config",
        "Profile 配置",
        &dsh_profile_dir.join("package.json"),
        asset.manifest,
    );
    inspect_file(
        "cordis.patch.yml",
        "patch",
        "场景与插件补丁",
        &dsh_profile_dir.join("cordis.patch.yml"),
        asset.patch,
    );
    if let Some(decl) = asset.decl {
        inspect_file(
            "decl.yml",
            "decl",
            "场景声明",
            &app_profile_dir.join("decl.yml"),
            decl,
        );
    }
    if let Some(rules) = asset.rules {
        inspect_file(
            "rules.json",
            "rules",
            "行业规则包",
            &app_profile_dir.join("rules.json"),
            rules,
        );
    }

    let skills_root = scope.dsh_home.join("skills");
    let repo_skills = scope.repo_root.map(|r| r.join("skills"));
    let mut repo_missing: Option<String> = None;
    for name in asset.skills {
        let dst = skills_root.join(name);
        // 逐文件比摘要，而不是只看 SKILL.md 在不在（OD-ARCH-113）。
        let (status, detail) = match expected_skill_files(scope, name, repo_skills.as_deref()) {
            Some((files, source)) => {
                let (status, detail) = inspect_skill(name, &dst, &files, &state);
                let suffix = source.is_embedded() && status != STATUS_READY;
                (
                    status,
                    if suffix && detail != "已就位" {
                        format!("{}（内嵌）", detail)
                    } else {
                        detail
                    },
                )
            }
            None => {
                repo_missing = Some(format!("Skill {} 在 Profile 目录、仓库与内嵌清单里都找不到。", name));
                (
                    STATUS_MISSING_SOURCE.to_string(),
                    "没有该 Skill 的正文来源".to_string(),
                )
            }
        };
        items.push(AssetItem {
            key: format!("skill:{}", name),
            kind: "skill".to_string(),
            label: format!("Skill · {}", name),
            path: dst.display().to_string(),
            status,
            detail,
        });
    }

    let cli = default_dsh_cli();
    for req in asset.plugins {
        let probe = probe_plugin(req, scope.dsh_home, &dsh_profile_dir, &cli);
        items.push(AssetItem {
            key: format!("plugin:{}", req.package),
            kind: "plugin".to_string(),
            label: format!("插件 · {}", req.package),
            path: probe.path,
            status: probe.status,
            detail: probe.detail,
        });
    }

    Ok(MaterializeReport {
        profile: profile.to_string(),
        label: asset.label.to_string(),
        summary_note: Some(asset.summary.to_string()),
        standard: false,
        items,
        notes: vec!["这里只做判定，不改文件；真正写入发生在切换 Profile 与启动引擎时。".to_string()],
        repo_missing,
    })
}

// ---------------------------------------------------------------------------
// 插件：本地探测 + 用户点击后安装
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct PluginProbe {
    pub package: String,
    pub expected_version: Option<String>,
    pub actual_version: Option<String>,
    pub path: String,
    pub status: String,
    pub detail: String,
}

fn candidates(dsh_cli: &Path, dsh_home: &Path, profile_dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut cur = dsh_cli.parent();
    while let Some(dir) = cur {
        if dir.file_name().and_then(|n| n.to_str()) == Some("node_modules") {
            out.push(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    out.push(dsh_home.join("node_modules"));
    out.push(profile_dir.join("node_modules"));
    out
}

fn package_version(pkg_dir: &Path) -> Option<String> {
    let text = fs::read_to_string(pkg_dir.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("version")?.as_str().map(str::to_string)
}

/// 读运行中 DSH CLI 的版本（OD-ARCH-120）。
///
/// 从 CLI 入口向上找到最近的 `package.json`，取 `version`。CLI 的安装布局是
/// `node_modules/@deepseek-ai/dsh/lib/bin.js`，包根在 `lib/` 的上一级。
pub fn dsh_runtime_version(dsh_cli: &Path) -> Option<String> {
    let mut cur = dsh_cli.parent();
    // 只向上找有限层：bin.js → lib → 包根，再多就跑到外层仓库去了。
    for _ in 0..4 {
        let dir = cur?;
        if dir.join("package.json").is_file() {
            let version = package_version(dir)?;
            // 只认 DSH 自己的包，避免捡到外层 aggregator 的版本号。
            let text = fs::read_to_string(dir.join("package.json")).ok()?;
            let value: serde_json::Value = serde_json::from_str(&text).ok()?;
            let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name == "@deepseek-ai/dsh" {
                return Some(version);
            }
        }
        cur = dir.parent();
    }
    None
}

/// 安装时该用哪个版本：声明里写了就用它，没写就钉到运行时版本（OD-ARCH-120）。
///
/// 不钉版本的后果是实测过的：registry 默认给 `0.1.6-alpha.2`，与运行中的
/// `0.2.1-alpha.2` 不兼容，DSH 直接拒绝安装。钉版本才能让「点安装」真的装上。
pub fn install_version(explicit: Option<&str>, dsh_cli: &Path) -> Option<String> {
    match explicit.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => Some(v.to_string()),
        None => dsh_runtime_version(dsh_cli),
    }
}

/// 本地解析插件：先查随运行时携带的 installAnchor，再查 DSH_HOME 与 Profile 目录。
///
/// 只问「这个名字能不能被解析到」；版本校验的入口仍是 `probe_plugin`。
pub fn package_available(dsh_home: &Path, package: &str, dsh_cli: &Path) -> bool {
    let any_profile = dsh_home.join("profiles");
    candidates(dsh_cli, dsh_home, &any_profile)
        .iter()
        .any(|modules| modules.join(package).join("package.json").is_file())
}

pub fn probe_plugin(
    req: &PluginRequirement,
    dsh_home: &Path,
    profile_dir: &Path,
    dsh_cli: &Path,
) -> PluginProbe {
    probe_package(req.package, req.version, dsh_home, profile_dir, dsh_cli)
}

pub fn probe_package(
    package: &str,
    version: Option<&str>,
    dsh_home: &Path,
    profile_dir: &Path,
    dsh_cli: &Path,
) -> PluginProbe {
    let dirs = candidates(dsh_cli, dsh_home, profile_dir);
    for modules in dirs.iter() {
        let pkg_dir = modules.join(package);
        if pkg_dir.join("package.json").is_file() {
            let actual = package_version(&pkg_dir);
            let mismatch = match (version, actual.as_deref()) {
                (Some(want), Some(have)) => want != have,
                _ => false,
            };
            return PluginProbe {
                package: package.to_string(),
                expected_version: version.map(str::to_string),
                actual_version: actual,
                path: pkg_dir.display().to_string(),
                status: if mismatch {
                    "version_mismatch".to_string()
                } else {
                    STATUS_READY.to_string()
                },
                detail: if mismatch {
                    "已安装的版本与要求不一致".to_string()
                } else {
                    "已就绪，无需安装".to_string()
                },
            };
        }
    }
    PluginProbe {
        package: package.to_string(),
        expected_version: version.map(str::to_string),
        actual_version: None,
        path: dirs
            .first()
            .map(|p| p.join(package).display().to_string())
            .unwrap_or_default(),
        status: "missing".to_string(),
        detail: "未找到该插件包，可点「安装」由 DSH 的插件通道安装".to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallReceipt {
    pub ok: bool,
    pub package: String,
    pub profile: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub note: String,
}

/// 一个待探测/待安装的插件（所有权版本，供自定义 Profile 的登记表使用）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginSpec {
    pub package: String,
    pub version: Option<String>,
}

impl PluginSpec {
    /// 探测时借用自身字段，避免为 &'static str 造泄漏。
    fn probe(&self, dsh_home: &Path, profile_dir: &Path, dsh_cli: &Path) -> PluginProbe {
        probe_package(
            &self.package,
            self.version.as_deref(),
            dsh_home,
            profile_dir,
            dsh_cli,
        )
    }
}

/// 某个 Profile 需要的插件清单（包名 + 期望版本）。
///
/// 出厂 Profile 用清单；自定义 Profile 用登记表里继承下来的依赖。
/// 基础标准恒为空：底座不额外装任何插件。
pub fn required_plugins(scope: &Scope<'_>, standard: &str) -> Vec<PluginSpec> {
    let profile = scope.profile.trim();
    if profile == standard {
        return Vec::new();
    }
    if let Some(asset) = asset_profile(profile) {
        return asset
            .plugins
            .iter()
            .map(|p| PluginSpec {
                package: p.package.to_string(),
                version: p.version.map(str::to_string),
            })
            .collect();
    }
    let registry = crate::profile_lifecycle::load_registry(scope.app_root);
    let Some(entry) = registry.profiles.iter().find(|p| p.name == profile) else {
        return Vec::new();
    };
    entry.plugins.iter().map(|raw| parse_plugin_spec(raw)).collect()
}

/// 把登记表里的一行 `pkg` / `pkg@1.2.3` 解析成包根 + 可选版本（OD-ARCH-128）。
///
/// 登记表是用户可见的文本，可能写成补丁里的子路径（OD-ARCH-119）；安装通道只认
/// 包根，所以先折回根名再交给安装器。抽成纯函数，便于直接覆盖写法组合。
pub fn parse_plugin_spec(raw: &str) -> PluginSpec {
    let name = raw.trim();
    let spec = match name.rsplit_once('@') {
        Some((p, v)) if !p.is_empty() && !v.is_empty() && !p.ends_with('/') => PluginSpec {
            package: p.to_string(),
            version: Some(v.to_string()),
        },
        _ => PluginSpec {
            package: name.to_string(),
            version: None,
        },
    };
    PluginSpec {
        package: crate::profile_lifecycle::package_root(&spec.package).to_string(),
        version: spec.version,
    }
}

/// 校验用户输入的插件包名，返回归一后的「包根」与可选版本（OD-ARCH-128）。
///
/// 只做形状校验，不判断包在不在、能不能装——那是安装通道的事。拒绝的情况：
/// 空、含空白或反斜杠、`..`、绝对路径、单个 `/` 但又不是 `@scope/pkg` 两段式、
/// 长度超 214（npm 包名上限）。scoped 包允许三段（子路径），统一折回包根。
pub fn normalize_plugin_input(input: &str) -> Result<PluginSpec, String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err("请填写插件包名，例如 @scope/plugin 或 plugin-name".to_string());
    }
    if raw.chars().count() > 214 {
        return Err("插件包名过长（上限 214 字符）".to_string());
    }
    if raw
        .chars()
        .any(|c| c.is_whitespace() || c == '\\' || c == '\0')
    {
        return Err("插件包名不能包含空白或反斜杠".to_string());
    }
    if raw.contains("..") || raw.starts_with('/') || raw.starts_with('.') {
        return Err("插件包名不能是路径（不接受 ..、绝对路径或点开头）".to_string());
    }
    let spec = parse_plugin_spec(raw);
    let root = spec.package.as_str();
    // 两段都要有内容：`@/abs/path` 会被 package_root 折成 `@/abs`，只看段数会漏判。
    let scoped_ok = match root.strip_prefix('@') {
        Some(rest) => match rest.split_once('/') {
            Some((scope, pkg)) => {
                !scope.is_empty() && !pkg.is_empty() && !pkg.contains('/')
            }
            None => false,
        },
        None => false,
    };
    let plain_ok = !root.starts_with('@') && !root.contains('/');
    if !scoped_ok && !plain_ok {
        return Err(format!(
            "「{}」不是合法包名：应写成 @scope/name 或 name",
            root
        ));
    }
    if root.ends_with('/') || root.trim().is_empty() {
        return Err(format!("「{}」不是合法包名", root));
    }
    Ok(spec)
}

/// 往自建 Profile 里加一个插件的结果（OD-ARCH-128）。
#[derive(Debug, Clone, Serialize)]
pub struct AddPluginReceipt {
    pub ok: bool,
    pub profile: String,
    /// 归一后的包根（可能带 `@version`）。
    pub package: String,
    /// 加之前登记表里就已经有这一项。
    pub already_registered: bool,
    /// 本次真的跑了安装命令。
    pub installed: bool,
    /// 安装后的探测结果；失败时为 null。
    pub status: Option<String>,
    pub detail: String,
    pub note: String,
}

/// 给自建 Profile 声明并安装一个插件（OD-ARCH-128）。
///
/// 顺序刻意是「先探测 → 缺才装 → 装成功后登记」：
///   * 本地已就绪：只补登记，不跑安装命令（不联网）；
///   * 缺件：走 DSH 官方插件通道装一次，成功后登记；
///   * 装失败：**登记表不动**，避免把「没装上」写成这个工作台的依赖。
///
/// 只接受已登记的自定义 Profile：基础标准是干净底座，出厂 Profile 的插件清单
/// 随版本固定，两者都不在运行时改。
pub fn add_plugin(
    scope: &Scope<'_>,
    standard: &str,
    dsh_cli: &Path,
    input: &str,
) -> Result<AddPluginReceipt, String> {
    let profile = scope.profile.trim();
    if profile == standard {
        return Err(
            "基础标准是干净底座，不额外安装插件。请先切到一个自定义 Profile 再声明。".to_string(),
        );
    }
    if asset_profile(profile).is_some() {
        return Err(format!(
            "「{}」是出厂 Profile，插件清单随版本固定；请复制成自定义 Profile 后再改。",
            profile
        ));
    }
    let spec = normalize_plugin_input(input)?;
    let mut registry = crate::profile_lifecycle::load_registry(scope.app_root);
    let Some(entry) = registry.profiles.iter().find(|p| p.name == profile) else {
        return Err(format!(
            "「{}」不在自定义 Profile 清单里，未做任何改动。",
            profile
        ));
    };
    let already_registered = entry
        .plugins
        .iter()
        .any(|p| parse_plugin_spec(p).package == spec.package);
    let profile_dir = scope.dsh_home.join("profiles").join(profile);
    let probe_before = probe_package(
        &spec.package,
        spec.version.as_deref(),
        scope.dsh_home,
        &profile_dir,
        dsh_cli,
    );
    let mut installed = false;
    let mut install_note: Option<String> = None;
    if probe_before.status != STATUS_READY {
        installed = true;
        let receipt = install_plugin(
            dsh_cli,
            scope.dsh_home,
            profile,
            &spec.package,
            spec.version.as_deref(),
        );
        install_note = Some(receipt.note.clone());
    }
    let probe_after = probe_package(
        &spec.package,
        spec.version.as_deref(),
        scope.dsh_home,
        &profile_dir,
        dsh_cli,
    );
    let ok = probe_after.status == STATUS_READY;
    // 只有真的就绪才登记：登记表代表「这个工作台需要它」，不能替失败的安装背书记录。
    if ok && !already_registered {
        let record = match spec.version.as_deref() {
            Some(v) => format!("{}@{}", spec.package, v),
            None => spec.package.clone(),
        };
        if let Some(entry) = registry.profiles.iter_mut().find(|p| p.name == profile) {
            entry.plugins.push(record);
        } else {
            return Err(format!(
                "「{}」在写入登记表前不见了，未做任何改动。",
                profile
            ));
        }
        crate::profile_lifecycle::save_registry_public(scope.app_root, &registry)?;
    }
    let note = if ok && !installed {
        if already_registered {
            format!("{} 已经在「{}」的插件清单里，本地也已就绪。", spec.package, profile)
        } else {
            format!("{} 本地已就绪，已加入「{}」的插件清单。", spec.package, profile)
        }
    } else if ok {
        format!(
            "已安装 {} 并加入「{}」的插件清单。{}",
            spec.package,
            profile,
            install_note.unwrap_or_default()
        )
    } else {
        format!(
            "{} 安装后仍不可用，未写入插件清单。{}",
            spec.package,
            install_note.unwrap_or_default()
        )
    };
    Ok(AddPluginReceipt {
        ok,
        profile: profile.to_string(),
        package: spec.package,
        already_registered,
        installed,
        status: Some(probe_after.status.clone()),
        detail: probe_after.detail,
        note,
    })
}

fn clip(text: &str) -> String {
    const MAX: usize = 4000;
    if text.chars().count() <= MAX {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX).collect();
    out.push_str("…（输出已截断）");
    out
}

/// 批量安装回执：逐项保留 DSH 的原样输出，不在前端再加工。
#[derive(Debug, Clone, Serialize)]
pub struct EnsureReceipt {
    pub ok: bool,
    pub profile: String,
    /// 安装前探测到缺失（或版本不符）的包。
    pub attempted: Vec<String>,
    /// 逐项安装回执，顺序与 attempted 一致。
    pub results: Vec<InstallReceipt>,
    /// 安装完成后重新探测仍然不 ready 的包 → 状态说明。
    pub still_missing: Vec<PluginProbe>,
    pub note: String,
}

/// 补齐某个 Profile 需要的插件：只装缺失项，单个失败不中断其余。
///
/// 这是网络写操作。调用点只有三处，且都不阻塞界面：
///   * 用户在界面上点「安装」（`profile_plugins_ensure`）；
///   * 切换 Profile 之后的后台补装（OD-ARCH-121）；
///   * 冷启动发现缺件后的后台补装（OD-ARCH-124）。
/// 装配（写配置 / 声明 / 规则 / Skill）本身**不联网**，这条边界不变。
pub fn ensure_plugins(
    scope: &Scope<'_>,
    standard: &str,
    dsh_cli: &Path,
) -> Result<EnsureReceipt, String> {
    let profile = scope.profile.trim();
    if !crate::settings::is_valid_profile_name(profile) {
        return Err(format!("Profile 名不合法：{:?}", profile));
    }
    if profile == standard {
        return Ok(EnsureReceipt {
            ok: true,
            profile: profile.to_string(),
            attempted: Vec::new(),
            results: Vec::new(),
            still_missing: Vec::new(),
            note: "基础标准不额外安装插件：底座与官方 DSH 一致。".to_string(),
        });
    }
    let profile_dir = scope.dsh_home.join("profiles").join(profile);
    let specs = required_plugins(scope, standard);
    let mut attempted: Vec<String> = Vec::new();
    let mut results: Vec<InstallReceipt> = Vec::new();
    for spec in &specs {
        let probe = spec.probe(scope.dsh_home, &profile_dir, dsh_cli);
        if probe.status == STATUS_READY {
            continue;
        }
        attempted.push(spec.package.clone());
        results.push(install_plugin(
            dsh_cli,
            scope.dsh_home,
            profile,
            &spec.package,
            spec.version.as_deref(),
        ));
    }
    let still_missing: Vec<PluginProbe> = specs
        .iter()
        .map(|s| s.probe(scope.dsh_home, &profile_dir, dsh_cli))
        .filter(|p| p.status != STATUS_READY)
        .collect();
    let ok = still_missing.is_empty();
    let note = if specs.is_empty() {
        "这个 Profile 没有声明额外插件。".to_string()
    } else if attempted.is_empty() {
        format!("{} 个插件本地就绪，无需安装。", specs.len())
    } else if ok {
        format!("已安装 {} 个缺失插件，现在全部就绪。", attempted.len())
    } else {
        format!(
            "尝试安装 {} 个插件，仍有 {} 个未就绪（回执见各项）。",
            attempted.len(),
            still_missing.len()
        )
    };
    Ok(EnsureReceipt {
        ok,
        profile: profile.to_string(),
        attempted,
        results,
        still_missing,
        note,
    })
}

/// 只读探测：这个 Profile 还差哪些插件。
///
/// 不联网、不写盘、不建目录：只按 `required_plugins` 逐项看本地能不能解析到。
/// 冷启动时用它决定「要不要起后台补装」，与 `ensure_plugins` 用同一张清单，
/// 避免出现「启动说不缺、安装时又说缺」的分叉。
pub fn missing_plugins(
    scope: &Scope<'_>,
    standard: &str,
    dsh_cli: &Path,
) -> Vec<PluginProbe> {
    let profile = scope.profile.trim();
    if profile == standard {
        return Vec::new();
    }
    let profile_dir = scope.dsh_home.join("profiles").join(profile);
    required_plugins(scope, standard)
        .iter()
        .map(|spec| spec.probe(scope.dsh_home, &profile_dir, dsh_cli))
        .filter(|probe| probe.status != STATUS_READY)
        .collect()
}

/// 通过 DSH 官方插件通道安装：`dsh plugin --profile <p> add <pkg>[@ver]`。
/// 这是网络写操作，只由用户在界面上点「安装」触发。
///
/// pnpm 11 起把「依赖带构建脚本、但没人批准」判成安装失败
/// （`ERR_PNPM_IGNORED_BUILDS`）。DSH 的安装是非交互的，界面上没有批准入口，
/// 所以第一次失败后关掉该 Profile 的 `strictDepBuilds`，再重试一次（OD-ARCH-120）。
fn relax_dep_builds(profile_dir: &Path) -> bool {
    let file = profile_dir.join("pnpm-workspace.yaml");
    let existing = fs::read_to_string(&file).unwrap_or_default();
    let mut text = existing.clone();
    let mut replaced = false;
    let mut out = String::new();
    for line in existing.lines() {
        if line.trim_start().starts_with("strictDepBuilds:") {
            out.push_str("strictDepBuilds: false\n");
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        if out.trim().is_empty() {
            out.push_str("packages:\n  - .\n");
        }
        out.push_str("strictDepBuilds: false\n");
    }
    if out == existing {
        return true;
    }
    text.clear();
    text.push_str(&out);
    fs::create_dir_all(profile_dir).is_ok() && fs::write(&file, text).is_ok()
}

fn needs_build_relax(stderr: &str) -> bool {
    stderr.contains("ERR_PNPM_IGNORED_BUILDS") || stderr.contains("Ignored build scripts")
}

/// 一次安装命令的原始结果。超时与「命令失败」分开记：前者要说明进程被杀掉，
/// 后者才有 DSH 的原样输出可看。
struct InstallRun {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

impl InstallRun {
    fn success(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    /// 失败判定要看 stdout + stderr：pnpm 把构建脚本拦截写在 stdout（OD-ARCH-120）。
    fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// 执行一次 `dsh plugin add`，带超时与进程回收（OD-ARCH-121）。
///
/// 轮询 `try_wait` + deadline + `kill` + `wait`，与 `model_routes` /
/// `route_classifier` 同一范式：不引入新依赖，也不会像 `output()` 那样在
/// 断网时永久阻塞。stdout/stderr 由独立线程读完，避免管道写满把子进程卡住。
fn run_install_command(
    dsh_cli: &Path,
    dsh_home: &Path,
    profile: &str,
    spec: &str,
    timeout: Duration,
) -> Result<InstallRun, String> {
    let node = crate::platform::resolve_node()?;
    let mut cmd = std::process::Command::new(node);
    cmd.arg(dsh_cli)
        .arg("plugin")
        .arg("--profile")
        .arg(profile)
        .arg("add")
        .arg(spec)
        // 装到隔离数据根，而不是默认的 ~/.dsh：Profile 目录由 DSH CLI 按 DSH_HOME 解析。
        .env("DSH_HOME", dsh_home);
    if let Some(dir) = dsh_cli.parent().and_then(|p| p.parent()) {
        cmd.current_dir(dir);
    }
    run_command_with_timeout(cmd, timeout)
}

/// 轮询等待任意命令结束；抽出这一层是为了让测试能用一个可控的慢命令
/// 验证「超时真的会 kill 并回收」，而不是只测一个包装函数。
fn run_command_with_timeout(
    mut cmd: std::process::Command,
    timeout: Duration,
) -> Result<InstallRun, String> {
    use std::process::Stdio;

    cmd
        // 非交互：没有终端时 DSH/pnpm 也不该等输入。
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // 自成进程组：`node → pnpm → 构建脚本` 是一棵树，超时只 kill 直接子进程
    // 会留下孙进程继续攥着管道、甚至继续写 Profile 目录（OD-ARCH-121 实测）。
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动安装命令：{}", e))?;
    // 整组回收要按 PID 反推组号：进程自成组后，组号就是它自己的 PID。
    #[cfg(unix)]
    let group: Option<i32> = Some(child.id() as i32);
    #[cfg(not(unix))]
    let group: Option<i32> = None;

    let out_pipe = child
        .stdout
        .take()
        .ok_or_else(|| "安装命令的 stdout 不可用".to_string())?;
    let (out_buffer, out_done) = collect_pipe(out_pipe);
    let err_pipe = child
        .stderr
        .take()
        .ok_or_else(|| "安装命令的 stderr 不可用".to_string())?;
    let (err_buffer, err_done) = collect_pipe(err_pipe);

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut exit_code = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                break;
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                // 超时：整组杀掉并回收，不让子孙进程留在后台继续写 Profile。
                kill_tree(&mut child, group);
                let _ = child.wait();
                timed_out = true;
                break;
            }
            Err(e) => {
                kill_tree(&mut child, group);
                let _ = child.wait();
                return Err(format!("等待安装命令失败：{}", e));
            }
        }
    }
    // 进程已退出（或被回收），管道应当很快关闭；仍给一道收尾上限，
    // 万一有脱组进程攥着管道，也不能把结果拖死在这里。
    let stdout = drain_pipe(out_buffer, out_done);
    let stderr = drain_pipe(err_buffer, err_done);
    Ok(InstallRun {
        exit_code,
        stdout,
        stderr,
        timed_out,
    })
}

/// 进程退出后等管道读完的上限。正常情况毫秒级；这里只防脱组进程。
const PIPE_DRAIN_GRACE: Duration = Duration::from_secs(2);

type PipeBuffer = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;

/// 起一个线程把管道读空，同时把已读到的内容放到共享缓冲里，
/// 这样即使读取线程还没收尾，调用方也能拿到已经产出的输出。
fn collect_pipe(
    mut pipe: impl std::io::Read + Send + 'static,
) -> (PipeBuffer, std::sync::mpsc::Receiver<()>) {
    let buffer: PipeBuffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = buffer.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(mut guard) = sink.lock() {
                        guard.extend_from_slice(&chunk[..n]);
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(());
    });
    (buffer, rx)
}

fn drain_pipe(buffer: PipeBuffer, done: std::sync::mpsc::Receiver<()>) -> String {
    let _ = done.recv_timeout(PIPE_DRAIN_GRACE);
    let bytes = buffer.lock().map(|g| g.clone()).unwrap_or_default();
    String::from_utf8_lossy(&bytes).to_string()
}

/// 杀整个进程组（含孙进程），再兜底单杀直接子进程。
///
/// `node → pnpm → 构建脚本` 是树状的，只 `child.kill()` 会漏掉后面两层；
/// 漏掉的那层还攥着 stdout/stderr，收尾就会一直等到它自己结束。
fn kill_tree(child: &mut std::process::Child, group: Option<i32>) {
    #[cfg(unix)]
    if let Some(pgid) = group {
        // SAFETY: killpg 只接受整数进程组号，失败也只是返回 -1，没有内存安全问题。
        unsafe {
            libc::killpg(pgid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    let _ = group;
    let _ = child.kill();
}

pub fn install_plugin(
    dsh_cli: &Path,
    dsh_home: &Path,
    profile: &str,
    package: &str,
    version: Option<&str>,
) -> InstallReceipt {
    install_plugin_with_timeout(dsh_cli, dsh_home, profile, package, version, INSTALL_TIMEOUT)
}

/// 安装的完整实现；超时单独传参，便于测试用极短 deadline 验证「真的会被 kill」。
fn install_plugin_with_timeout(
    dsh_cli: &Path,
    dsh_home: &Path,
    profile: &str,
    package: &str,
    version: Option<&str>,
    timeout: Duration,
) -> InstallReceipt {
    let name = package.trim();
    let scoped_ok = name.starts_with('@') && name.split('/').count() == 2;
    let bad = name.is_empty()
        || name.contains("..")
        || name.contains('\\')
        || (name.contains('/') && !scoped_ok);
    if bad || !crate::settings::is_valid_profile_name(profile) {
        return InstallReceipt {
            ok: false,
            package: package.to_string(),
            profile: profile.to_string(),
            command: String::new(),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            note: "插件名或 Profile 名不合法，未执行任何命令".to_string(),
        };
    }
    if !dsh_cli.is_file() {
        return InstallReceipt {
            ok: false,
            package: name.to_string(),
            profile: profile.to_string(),
            command: String::new(),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            note: format!("找不到 DSH CLI：{}，无法安装", dsh_cli.display()),
        };
    }
    // 不钉版本会被兼容检查拒掉（OD-ARCH-120 实测）：registry 默认给旧 alpha，
    // 与运行中的 DSH 不兼容。这里默认钉到运行时版本。
    let pinned = install_version(version, dsh_cli);
    let spec = match pinned.as_deref() {
        Some(v) => format!("{}@{}", name, v),
        None => name.to_string(),
    };
    let command = format!(
        "node {} plugin --profile {} add {}",
        dsh_cli.display(),
        profile,
        spec
    );
    let profile_dir = dsh_home.join("profiles").join(profile);
    let run = || run_install_command(dsh_cli, dsh_home, profile, &spec, timeout);
    let mut relaxed = false;
    let mut out = match run() {
        Ok(out) => out,
        Err(e) => {
            return InstallReceipt {
                ok: false,
                package: name.to_string(),
                profile: profile.to_string(),
                command,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                note: e,
            };
        }
    };
    if !out.success() {
        // pnpm 把拦截信息写在 stdout，DSH 自己的失败摘要写在 stderr，两处都要看。
        // 超时不算「需要放行构建脚本」，不触发重试。
        if !out.timed_out && needs_build_relax(&out.combined()) && relax_dep_builds(&profile_dir)
        {
            relaxed = true;
            if let Ok(second) = run() {
                out = second;
            }
        }
    }
    let stdout = clip(&out.stdout);
    let stderr = clip(&out.stderr);
    let code = out.exit_code;
    InstallReceipt {
        ok: out.success(),
        package: name.to_string(),
        profile: profile.to_string(),
        command,
        exit_code: code,
        stdout,
        stderr,
        note: if out.timed_out {
            format!(
                "安装超时：DSH 命令在 {} 秒内没有结束，已终止。网络恢复后可重试。",
                timeout.as_secs()
            )
        } else if out.success() {
            if relaxed {
                "安装命令已完成（该 Profile 已允许依赖的构建脚本）。重新打开本页可确认探测状态"
                    .to_string()
            } else {
                "安装命令已完成；重新打开本页可确认探测状态".to_string()
            }
        } else {
            "安装失败，下面是 DSH 的原样输出".to_string()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "od-profile-assets-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn repo() -> PathBuf {
        crate::task_shell::repo_root_hint().unwrap()
    }

    /// 停放区只读清单：空目录不存在时返回空表，且不得顺手建目录（OD-ARCH-117）。
    #[test]
    fn list_parked_is_read_only_and_sorted() {
        let root = temp_root("parked");
        let app_root = root.join("app");
        fs::create_dir_all(&app_root).unwrap();
        assert!(list_parked(&app_root).is_empty());
        assert!(
            !app_root.join(PARKED_SKILLS_DIR).exists(),
            "只是看一眼不能把停放目录建出来"
        );

        let parked = parked_root(&app_root);
        for name in ["weekly-report", "anysearch"] {
            fs::create_dir_all(parked.join(name)).unwrap();
            fs::write(parked.join(name).join("SKILL.md"), "x").unwrap();
        }
        fs::write(parked.join("readme.txt"), "x").unwrap();
        let listed = list_parked(&app_root);
        assert_eq!(
            listed.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            vec!["anysearch", "weekly-report"]
        );
        assert!(listed.iter().all(|p| p.entries >= 1));
    }

    #[test]
    fn standard_profile_writes_nothing() {
        let root = temp_root("standard");
        let dsh_home = root.join("dsh-home");
        fs::create_dir_all(&dsh_home).unwrap();
        let app_root = root.join("app");
        let repo = repo();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "omnidesk-base",
            repo_root: Some(&repo),
        };
        let report = materialize(&scope, "omnidesk-base").unwrap();
        assert!(report.standard);
        assert!(report.items.is_empty());
        assert!(!dsh_home.join("profiles").exists(), "标准 Profile 不得创建目录");
        assert!(!app_root.join("profiles").exists(), "标准 Profile 不得创建目录");
    }

    /// OD-ARCH-122 缺口 A：自建 Profile 的体检必须看得见 Skill。
    ///
    /// 之前只看插件，用户删掉 Skill 目录后设置页仍是「没有可查项」，只有下一次切换
    /// 才被悄悄补回。这里钉住三件事：缺了显示 missing、补上显示 ready、巡检不写盘。
    #[test]
    fn inspect_custom_reports_bound_skills() {
        let root = temp_root("custom-skill");
        let dsh_home = root.join("dsh-home");
        let app_root = root.join("app");
        fs::create_dir_all(&dsh_home).unwrap();
        fs::create_dir_all(&app_root).unwrap();
        crate::profile_lifecycle::save_registry_public(
            &app_root,
            &crate::profile_lifecycle::Registry {
                schema: crate::profile_lifecycle::REGISTRY_SCHEMA.to_string(),
                profiles: vec![crate::profile_lifecycle::ManagedProfile {
                    name: "my-work".to_string(),
                    label: "我的工作台".to_string(),
                    created_at: 0,
                    origin: "standard".to_string(),
                    plugins: Vec::new(),
                }],
            },
        )
        .unwrap();
        let app_profile = app_root.join("profiles").join("my-work");
        fs::create_dir_all(&app_profile).unwrap();
        fs::write(
            app_profile.join("rules.json"),
            r#"{
  "schema": "omnidesk.profile-rules.v2",
  "profile": "my-work",
  "skill_bindings": { "weekly": { "skills": ["memory-agent"], "max_injected": 1 } }
}"#,
        )
        .unwrap();

        let repo = repo();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "my-work",
            repo_root: Some(&repo),
        };

        let before = inspect(&scope, "omnidesk-base").unwrap();
        let missing = before
            .items
            .iter()
            .find(|i| i.key == "skill:memory-agent")
            .expect("体检清单里必须有绑定声明的 Skill");
        assert_eq!(missing.status, STATUS_MISSING);
        assert!(
            !dsh_home.join("skills").exists(),
            "只读巡检不得创建 skills 目录"
        );
        assert!(
            !app_profile.join(ASSET_STATE_FILE).exists(),
            "只读巡检不得写装配状态文件"
        );

        materialize(&scope, "omnidesk-base").unwrap();
        let after = inspect(&scope, "omnidesk-base").unwrap();
        let ready = after
            .items
            .iter()
            .find(|i| i.key == "skill:memory-agent")
            .expect("补齐后仍要列出该 Skill");
        assert_eq!(ready.status, STATUS_READY);
    }

    #[test]
    fn sync_bound_skills_fills_runtime_dir_and_parks_unbound() {
        let root = temp_root("bound-sync");
        let dsh_home = root.join("dsh-home");
        let app_root = root.join("app");
        fs::create_dir_all(&dsh_home).unwrap();
        let app_profile = app_root.join("profiles").join("my-work");
        fs::create_dir_all(&app_profile).unwrap();
        let rules = app_profile.join("rules.json");
        fs::write(
            &rules,
            r#"{
  "schema": "omnidesk.profile-rules.v2",
  "profile": "my-work",
  "skill_bindings": {
    "weekly": { "skills": ["memory-agent", "memory-report"], "max_injected": 2 }
  }
}"#,
        )
        .unwrap();
        let repo = repo();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "my-work",
            repo_root: Some(&repo),
        };
        let first = sync_bound_skills(&scope, "omnidesk-base").unwrap();
        assert_eq!(first.wanted, vec!["memory-agent", "memory-report"]);
        assert_eq!(first.filled.len(), 2, "两个 Skill 都该在保存后立刻落盘");
        for name in &first.wanted {
            assert!(
                dsh_home.join("skills").join(name).join("SKILL.md").is_file(),
                "{} 必须真的出现在运行时目录里",
                name
            );
        }
        assert!(
            dsh_home.join("skills").join(MANAGED_SKILLS_FILE).is_file(),
            "补进运行时目录的 Skill 要登记归属，切走时才收得回"
        );

        // 二次保存同一份绑定：不重复写，全部记为 keeping。
        let second = sync_bound_skills(&scope, "omnidesk-base").unwrap();
        assert!(second.filled.is_empty(), "已就位的不该再写一遍");
        assert_eq!(second.kept.len(), 2);

        // 解绑 memory-report：目录移入停放区，不删除。
        fs::write(
            &rules,
            r#"{
  "schema": "omnidesk.profile-rules.v2",
  "profile": "my-work",
  "skill_bindings": {
    "weekly": { "skills": ["memory-agent"], "max_injected": 1 }
  }
}"#,
        )
        .unwrap();
        let third = sync_bound_skills(&scope, "omnidesk-base").unwrap();
        assert_eq!(third.parked, vec!["memory-report"]);
        assert!(!dsh_home.join("skills").join("memory-report").exists());
        assert!(parked_root(&app_root).join("memory-report").is_dir());
        assert!(
            dsh_home.join("skills").join("memory-agent").join("SKILL.md").is_file(),
            "仍然绑定的那个不该被一起收走"
        );
    }

    /// 干净底座：保存绑定这条路径在基础标准下不产生任何目录与登记表。
    #[test]
    fn sync_bound_skills_is_noop_for_standard_profile() {
        let root = temp_root("bound-sync-standard");
        let dsh_home = root.join("dsh-home");
        let app_root = root.join("app");
        fs::create_dir_all(&dsh_home).unwrap();
        fs::create_dir_all(&app_root).unwrap();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "omnidesk-base",
            repo_root: None,
        };
        let report = sync_bound_skills(&scope, "omnidesk-base").unwrap();
        assert!(report.standard);
        assert!(report.wanted.is_empty());
        assert!(!dsh_home.join("skills").exists(), "基础标准不得创建 skills 目录");
        assert!(!dsh_home.join("profiles").exists());
    }

    /// OD-ARCH-131：Profile 自带的 Skill 是这一份定制，装配时优先于仓库与内嵌。
    #[test]
    fn profile_owned_skill_wins_over_repo_source() {
        let (root, dsh_home, app_root, _dsh_cli) = custom_profile_fixture("owned-skill", Vec::new());
        let app_profile = app_root.join("profiles").join("my-work");
        let own = app_profile.join("skills").join("office-note");
        fs::create_dir_all(own.join("references")).unwrap();
        fs::write(own.join("SKILL.md"), "# 我自己的随手记\n\n本单位的写法。\n").unwrap();
        fs::write(own.join("references").join("extra.md"), "reference\n").unwrap();
        fs::write(
            app_profile.join("rules.json"),
            r#"{
  "schema": "omnidesk.profile-rules.v2",
  "profile": "my-work",
  "skill_bindings": {
    "weekly-note": { "skills": ["office-note"], "max_injected": 1 }
  }
}"#,
        )
        .unwrap();
        let repo = repo();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "my-work",
            repo_root: Some(&repo),
        };
        let sync = sync_bound_skills(&scope, "omnidesk-base").unwrap();
        assert!(sync.missing_source.is_empty(), "自带源不该被判缺源：{sync:?}");
        assert_eq!(sync.filled, vec!["office-note"]);
        let landed = dsh_home.join("skills").join("office-note").join("SKILL.md");
        let text = fs::read_to_string(&landed).unwrap();
        assert!(text.contains("本单位"), "落盘的是 Profile 自带正文，实际：{text}");
        assert!(
            dsh_home
                .join("skills")
                .join("office-note")
                .join("references")
                .join("extra.md")
                .is_file(),
            "自带 Skill 的子目录也要一起带过去"
        );
        // 巡检与装配同源：自带 Skill 在资产页不是「缺源」。
        let view = inspect(&scope, "omnidesk-base").unwrap();
        let item = view
            .items
            .iter()
            .find(|i| i.key == "skill:office-note")
            .expect("巡检要列出自带 Skill");
        assert_eq!(item.status, STATUS_READY, "详情：{}", item.detail);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discipline_materializes_then_is_idempotent() {
        let root = temp_root("discipline");
        let dsh_home = root.join("dsh-home");
        fs::create_dir_all(&dsh_home).unwrap();
        let app_root = root.join("app");
        let repo = repo();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "discipline",
            repo_root: Some(&repo),
        };
        let first = materialize(&scope, "omnidesk-base").unwrap();
        assert!(!first.standard);
        for key in ["package.json", "cordis.patch.yml", "decl.yml", "rules.json"] {
            assert!(
                first
                    .items
                    .iter()
                    .any(|i| i.key == key && i.status == STATUS_CREATED),
                "{} 应被补写",
                key
            );
        }
        assert!(dsh_home.join("profiles/discipline/package.json").is_file());
        assert!(app_root.join("profiles/discipline/decl.yml").is_file());
        assert!(dsh_home.join("skills/memory-agent/SKILL.md").is_file());

        let second = materialize(&scope, "omnidesk-base").unwrap();
        assert!(
            second.items.iter().all(|i| i.status != STATUS_CREATED),
            "第二次装配不应再新建任何文件：{:?}",
            second.items
        );
    }

    #[test]
    fn user_edit_is_not_overwritten_and_reported_as_drift() {
        let root = temp_root("drift");
        let dsh_home = root.join("dsh-home");
        fs::create_dir_all(&dsh_home).unwrap();
        let app_root = root.join("app");
        let repo = repo();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "discipline",
            repo_root: Some(&repo),
        };
        materialize(&scope, "omnidesk-base").unwrap();
        let rules = app_root.join("profiles/discipline/rules.json");
        fs::write(&rules, "{\"user\":\"edited\"}").unwrap();
        let report = materialize(&scope, "omnidesk-base").unwrap();
        let item = report.items.iter().find(|i| i.key == "rules.json").unwrap();
        assert_eq!(item.status, STATUS_DRIFT);
        assert_eq!(fs::read_to_string(&rules).unwrap(), "{\"user\":\"edited\"}");
    }

    #[test]
    fn deleted_file_is_healed_on_next_run() {
        let root = temp_root("heal");
        let dsh_home = root.join("dsh-home");
        fs::create_dir_all(&dsh_home).unwrap();
        let app_root = root.join("app");
        let repo = repo();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "discipline",
            repo_root: Some(&repo),
        };
        materialize(&scope, "omnidesk-base").unwrap();
        fs::remove_file(dsh_home.join("profiles/discipline/package.json")).unwrap();
        let report = materialize(&scope, "omnidesk-base").unwrap();
        let item = report
            .items
            .iter()
            .find(|i| i.key == "package.json")
            .unwrap();
        assert_eq!(item.status, STATUS_CREATED);
        assert!(dsh_home.join("profiles/discipline/package.json").is_file());
    }

    #[test]
    fn previously_written_file_is_updated_with_backup() {
        let root = temp_root("update");
        let dsh_home = root.join("dsh-home");
        fs::create_dir_all(&dsh_home).unwrap();
        let app_root = root.join("app");
        let repo = repo();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "discipline",
            repo_root: Some(&repo),
        };
        materialize(&scope, "omnidesk-base").unwrap();
        // 模拟「上一版出厂内容」：内容不是当前默认，但状态文件记着它是我们写的。
        let patch = dsh_home.join("profiles/discipline/cordis.patch.yml");
        let old = "# old factory content\n- insert: []\n";
        fs::write(&patch, old).unwrap();
        let state_path = app_root.join("profiles/discipline").join(ASSET_STATE_FILE);
        let mut state = load_state(&state_path);
        state.written.insert(
            "cordis.patch.yml".to_string(),
            crate::sha256::sha256_hex(old.as_bytes()),
        );
        save_state(&state_path, &mut state).unwrap();

        let report = materialize(&scope, "omnidesk-base").unwrap();
        let item = report
            .items
            .iter()
            .find(|i| i.key == "cordis.patch.yml")
            .unwrap();
        assert_eq!(item.status, STATUS_UPDATED);
        assert_eq!(
            fs::read_to_string(&patch).unwrap(),
            asset_profile("discipline").unwrap().patch
        );
        let has_backup = fs::read_dir(patch.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains(".bak-"));
        assert!(has_backup, "更新前必须留下备份");
    }

    #[test]
    fn bundled_plugin_probe_reports_ready_and_missing() {
        let root = temp_root("probe");
        let repo = repo();
        let dsh_cli = repo.join("runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js");
        let dsh_home = root.join("dsh-home");
        let profile_dir = dsh_home.join("profiles/discipline");
        fs::create_dir_all(&profile_dir).unwrap();
        let req = &DISCIPLINE_PLUGINS[0];
        let ready = probe_plugin(req, &dsh_home, &profile_dir, &dsh_cli);
        if dsh_cli.is_file() {
            assert_eq!(
                ready.status, STATUS_READY,
                "随运行时携带的插件应就绪：{:?}",
                ready
            );
        }
        let missing = probe_plugin(
            &PluginRequirement {
                package: "@deepseek-ai/does-not-exist-od109",
                version: None,
            },
            &dsh_home,
            &profile_dir,
            &dsh_cli,
        );
        assert_eq!(missing.status, "missing");
    }

    /// 运行时版本必须能被读到，否则安装回退成裸包名，被兼容检查拒掉（OD-ARCH-120）。
    #[test]
    fn runtime_version_reads_dsh_package() {
        let repo = repo();
        let dsh_cli = repo.join("runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js");
        if !dsh_cli.is_file() {
            return;
        }
        let version = dsh_runtime_version(&dsh_cli).expect("应能读到 DSH 版本");
        assert!(
            version.starts_with(char::is_numeric),
            "版本号形态异常：{version}"
        );
        assert!(
            dsh_runtime_version(&repo.join("nodesuch/cli.js")).is_none(),
            "CLI 不存在时不得乱猜版本"
        );
    }

    #[test]
    fn install_version_prefers_explicit_then_runtime() {
        let repo = repo();
        let dsh_cli = repo.join("runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js");
        assert_eq!(
            install_version(Some(" 1.2.3 "), &dsh_cli).as_deref(),
            Some("1.2.3")
        );
        assert_eq!(install_version(Some("   "), &dsh_cli), dsh_runtime_version(&dsh_cli));
        assert_eq!(install_version(None, &dsh_cli), dsh_runtime_version(&dsh_cli));
    }

    /// pnpm 11 的构建脚本拦截：给已有配置补开关，不破坏原有键（OD-ARCH-120）。
    #[test]
    fn relax_dep_builds_keeps_existing_keys() {
        let root = temp_root("relax");
        let dir = root.join("dsh-home/profiles/probe");
        fs::create_dir_all(&dir).unwrap();
        assert!(relax_dep_builds(&dir));
        let written = fs::read_to_string(dir.join("pnpm-workspace.yaml")).unwrap();
        assert!(written.contains("strictDepBuilds: false"), "{written}");
        assert!(written.contains("packages:"), "空目录也要给出最小可用配置：{written}");

        fs::write(
            dir.join("pnpm-workspace.yaml"),
            "packages:\n  - .\n\nnodeLinker: hoisted\nstrictDepBuilds: true\n",
        )
        .unwrap();
        assert!(relax_dep_builds(&dir));
        let rewritten = fs::read_to_string(dir.join("pnpm-workspace.yaml")).unwrap();
        assert!(rewritten.contains("nodeLinker: hoisted"), "原有键不能被删：{rewritten}");
        assert!(!rewritten.contains("strictDepBuilds: true"), "{rewritten}");
        assert_eq!(rewritten.matches("strictDepBuilds").count(), 1, "{rewritten}");

        assert!(needs_build_relax("...\n[ERR_PNPM_IGNORED_BUILDS] Ignored build scripts: koffi@3.1.1"));
        assert!(!needs_build_relax("installation rejected: incompatible"));
    }

    /// 安装命令必须有上界（OD-ARCH-121）：断网时不能永久占住线程。
    /// 用可控慢命令验证「到点就 kill、并如实判失败」，同时确认输出仍能回收。
    #[test]
    fn run_install_command_kills_on_timeout() {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg("sleep 30; echo late");
        let started = Instant::now();
        let receipt = run_command_with_timeout(cmd, Duration::from_millis(400))
            .expect("超时不是启动失败，应当返回结果");
        let elapsed = started.elapsed();
        assert!(receipt.timed_out, "应当判定为超时");
        assert!(!receipt.success(), "超时不能算成功");
        assert_eq!(receipt.exit_code, None, "被杀掉的进程没有退出码");
        assert!(
            elapsed < Duration::from_secs(10),
            "必须在 deadline 附近收尾，实际 {elapsed:?}"
        );
    }

    /// 正常命令的 stdout / stderr 都要完整收回，退出码照实记录。
    #[test]
    fn run_install_command_collects_both_streams() {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c")
            .arg("echo to-stdout; echo to-stderr 1>&2; exit 3");
        let receipt =
            run_command_with_timeout(cmd, Duration::from_secs(10)).expect("应当启动成功");
        assert!(!receipt.timed_out);
        assert_eq!(receipt.exit_code, Some(3));
        assert!(receipt.stdout.contains("to-stdout"), "{:?}", receipt.stdout);
        assert!(receipt.stderr.contains("to-stderr"), "{:?}", receipt.stderr);
        assert!(!receipt.success());
        assert!(receipt.combined().contains("to-stdout"));
        assert!(receipt.combined().contains("to-stderr"));
    }

    // OD-ARCH-124：冷启动插件自愈的判据必须与切换后补装同源。
    //
    // 这里只钉纯探测：基础标准恒空、登记了但本地没有就报缺、把包放进来就归零，
    // 以及探测本身绝不建 node_modules（否则「看一眼」会留下假就绪的目录）。

    /// 造一个只含登记表的自建 Profile，返回 (root, dsh_home, app_root, dsh_cli)。
    fn custom_profile_fixture(tag: &str, plugins: Vec<String>) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let root = temp_root(tag);
        let dsh_home = root.join("dsh-home");
        let app_root = root.join("app");
        fs::create_dir_all(&dsh_home).unwrap();
        fs::create_dir_all(&app_root).unwrap();
        crate::profile_lifecycle::save_registry_public(
            &app_root,
            &crate::profile_lifecycle::Registry {
                schema: crate::profile_lifecycle::REGISTRY_SCHEMA.to_string(),
                profiles: vec![crate::profile_lifecycle::ManagedProfile {
                    name: "my-work".to_string(),
                    label: "我的工作台".to_string(),
                    created_at: 0,
                    origin: "standard".to_string(),
                    plugins,
                }],
            },
        )
        .unwrap();
        // CLI 放在普通目录下：candidates 只会落到 dsh_home / profile 的 node_modules，
        // 探测结果因此完全由夹具决定，不受宿主安装影响。
        let dsh_cli = root.join("cli/lib/bin.js");
        fs::create_dir_all(dsh_cli.parent().unwrap()).unwrap();
        fs::write(&dsh_cli, "// stub").unwrap();
        (root, dsh_home, app_root, dsh_cli)
    }

    // OD-ARCH-128：自建 Profile 的插件入口。
    //
    // 三个纯形状用例钉住「用户能写什么」，四个带夹具的用例钉住副作用边界：
    // 基础标准不建目录、本地已就绪不联网、重复声明幂等、装失败不写登记表。

    #[test]
    fn parse_plugin_spec_folds_subpaths_and_versions() {
        let plain = parse_plugin_spec("  @acme/plugin  ");
        assert_eq!(plain.package, "@acme/plugin");
        assert_eq!(plain.version, None);

        let with_version = parse_plugin_spec("@acme/plugin@1.2.3");
        assert_eq!(with_version.package, "@acme/plugin");
        assert_eq!(with_version.version.as_deref(), Some("1.2.3"));

        // 补丁里的子路径写法要折回包根，否则安装通道会当成另一个包。
        let subpath = parse_plugin_spec("@acme/plugin/skill/office@2.0.0");
        assert_eq!(subpath.package, "@acme/plugin");
        assert_eq!(subpath.version.as_deref(), Some("2.0.0"));

        let bare = parse_plugin_spec("office-tools");
        assert_eq!(bare.package, "office-tools");
        assert_eq!(bare.version, None);
    }

    #[test]
    fn normalize_plugin_input_rejects_paths_and_keeps_package_roots() {
        let ok = normalize_plugin_input("@acme/plugin/sub@1.0.0").expect("合法包名");
        assert_eq!(ok.package, "@acme/plugin");
        assert_eq!(ok.version.as_deref(), Some("1.0.0"));
        assert_eq!(normalize_plugin_input("office-tools").unwrap().package, "office-tools");

        for bad in [
            "",
            "   ",
            "../escape",
            "@/abs/path",
            ".hidden/plugin",
            "@scope",
            "plain/path",
            "back\\slash",
            "with space",
        ] {
            assert!(
                normalize_plugin_input(bad).is_err(),
                "应当拒绝非法写法：{bad:?}"
            );
        }
        assert!(normalize_plugin_input(&"a".repeat(215)).is_err(), "超长包名要拒");
    }

    #[test]
    fn add_plugin_refuses_standard_profile_without_touching_disk() {
        let root = temp_root("add-standard");
        let dsh_home = root.join("dsh-home");
        let app_root = root.join("app");
        fs::create_dir_all(&dsh_home).unwrap();
        fs::create_dir_all(&app_root).unwrap();
        let dsh_cli = root.join("cli/lib/bin.js");
        fs::create_dir_all(dsh_cli.parent().unwrap()).unwrap();
        fs::write(&dsh_cli, "// stub").unwrap();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "omnidesk-base",
            repo_root: None,
        };
        let err = add_plugin(&scope, "omnidesk-base", &dsh_cli, "@acme/plugin")
            .expect_err("基础标准必须拒绝声明插件");
        assert!(err.contains("干净底座"), "错误文案要说清原因：{err}");
        assert!(!dsh_home.join("profiles").exists(), "拒绝时不得创建 Profile 目录");
        assert_eq!(
            fs::read_dir(&app_root).unwrap().count(),
            0,
            "拒绝时不得在应用数据根写入任何条目"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn add_plugin_registers_already_ready_package_without_installing() {
        let (root, dsh_home, app_root, dsh_cli) =
            custom_profile_fixture("add-ready", Vec::new());
        let pkg = dsh_home.join("node_modules/@acme/plugin");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("package.json"),
            r#"{"name":"@acme/plugin","version":"1.0.0"}"#,
        )
        .unwrap();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "my-work",
            repo_root: None,
        };
        let receipt = add_plugin(&scope, "omnidesk-base", &dsh_cli, "@acme/plugin@1.0.0")
            .expect("已就绪的包应当只补登记");
        assert!(receipt.ok);
        assert!(!receipt.installed, "本地已就绪不该再跑安装命令");
        assert!(!receipt.already_registered);
        assert_eq!(receipt.status.as_deref(), Some(STATUS_READY));
        let registry = crate::profile_lifecycle::load_registry(&app_root);
        let entry = registry
            .profiles
            .iter()
            .find(|p| p.name == "my-work")
            .expect("登记表里应有这个 Profile");
        assert_eq!(entry.plugins, vec!["@acme/plugin@1.0.0".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn add_plugin_is_idempotent_for_the_same_package() {
        let (root, dsh_home, app_root, dsh_cli) =
            custom_profile_fixture("add-twice", Vec::new());
        let pkg = dsh_home.join("node_modules/@acme/plugin");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("package.json"),
            r#"{"name":"@acme/plugin","version":"1.0.0"}"#,
        )
        .unwrap();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "my-work",
            repo_root: None,
        };
        add_plugin(&scope, "omnidesk-base", &dsh_cli, "@acme/plugin").unwrap();
        let second = add_plugin(&scope, "omnidesk-base", &dsh_cli, "@acme/plugin")
            .expect("重复声明应当照常成功");
        assert!(second.ok);
        assert!(second.already_registered, "第二次应识别为已登记");
        assert!(!second.installed);
        assert!(second.note.contains("已经在"), "回执要说清没做重复动作：{}", second.note);
        let registry = crate::profile_lifecycle::load_registry(&app_root);
        let entry = registry.profiles.iter().find(|p| p.name == "my-work").unwrap();
        assert_eq!(entry.plugins.len(), 1, "不得重复登记：{:?}", entry.plugins);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn add_plugin_leaves_registry_alone_when_install_fails() {
        let (root, dsh_home, app_root, dsh_cli) =
            custom_profile_fixture("add-fail", Vec::new());
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "my-work",
            repo_root: None,
        };
        // 存根 CLI 是空文件，node 跑起来必失败；这正是「装不上」的真实形态。
        let receipt = add_plugin(&scope, "omnidesk-base", &dsh_cli, "@acme/plugin")
            .expect("安装失败应当作为回执返回，而不是报错");
        assert!(!receipt.ok, "装不上不能报成功");
        assert!(receipt.installed, "应当真的尝试过安装");
        assert!(receipt.note.contains("未写入插件清单"), "回执：{}", receipt.note);
        let registry = crate::profile_lifecycle::load_registry(&app_root);
        let entry = registry.profiles.iter().find(|p| p.name == "my-work").unwrap();
        assert!(entry.plugins.is_empty(), "失败的包不得进登记表：{:?}", entry.plugins);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_plugins_is_empty_for_standard_profile() {
        let (root, dsh_home, app_root, dsh_cli) =
            custom_profile_fixture("startup-standard", vec!["@deepseek-ai/dsh-skill-office".to_string()]);
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "omnidesk-base",
            repo_root: None,
        };
        assert!(
            missing_plugins(&scope, "omnidesk-base", &dsh_cli).is_empty(),
            "基础标准是干净底座，冷启动不得探测出待装插件"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_plugins_reports_registered_but_absent_package() {
        let (root, dsh_home, app_root, dsh_cli) =
            custom_profile_fixture("startup-missing", vec!["@deepseek-ai/dsh-skill-office".to_string()]);
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "my-work",
            repo_root: None,
        };
        // 夹具本身会写登记表；要证明的是「探测」前后一致，所以先拍快照。
        let app_before: Vec<_> = fs::read_dir(&app_root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        let probes = missing_plugins(&scope, "omnidesk-base", &dsh_cli);
        assert_eq!(probes.len(), 1, "登记了一个插件就应报一项缺件：{probes:?}");
        assert_eq!(probes[0].package, "@deepseek-ai/dsh-skill-office");
        assert_eq!(probes[0].status, STATUS_MISSING);
        assert!(
            !dsh_home.join("node_modules").exists() && !dsh_home.join("profiles").exists(),
            "只读探测不得创建 node_modules 或 Profile 目录"
        );
        let app_after: Vec<_> = fs::read_dir(&app_root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(
            app_before.len() == app_after.len(),
            "只读探测不得在应用数据根下新增任何条目：{app_before:?} → {app_after:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_plugins_clears_once_package_lands_locally() {
        let (root, dsh_home, app_root, dsh_cli) =
            custom_profile_fixture("startup-ready", vec!["@deepseek-ai/dsh-skill-office".to_string()]);
        let pkg = dsh_home.join("node_modules/@deepseek-ai/dsh-skill-office");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("package.json"), "{\"name\":\"x\",\"version\":\"1.0.0\"}").unwrap();
        let scope = Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: "my-work",
            repo_root: None,
        };
        assert!(
            missing_plugins(&scope, "omnidesk-base", &dsh_cli).is_empty(),
            "包已在本地就绪时不应再报缺件、也不该起安装任务"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
