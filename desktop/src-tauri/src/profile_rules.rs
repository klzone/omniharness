//! Profile 规则资产（omnidesk.profile-rules.v1）——OD-ARCH-068 S1/S2/S3。
//!
//! 平台代码只保留**机制**，不内置任何行业规则：红线词表、写作铁律、场景规则与
//! 思考档、隐私策略都写在 Profile 规则资产里。文件不存在 = **纯净底座**（零关键词、
//! 零铁律、不掩码、不改写），行为与官方 DSH-desktop 无实质区别。
//!
//! 落点（OD-ARCH-106）：`<应用数据根>/profiles/<Profile 名>/rules.json`（仓库外）。
//! 规则**随 Profile 挂载**：基础标准下没有这份文件，就是纯净底座；切到专业 Profile
//! 时才有该 Profile 自己的规则、场景与 Skill 绑定。旧单文件只做一次性兼容迁移。
//! 出厂默认只在仓库里（编译期嵌入），**首次启动不自动创建**——创建与否由用户在设置页
//! 显式决定「安装默认规则包」，这样「移走文件 → 重启 → 零拦截」才是可复核的行为。
//!
//! 失败语义：文件损坏一律报错（提交链路据此拒绝），不静默退回纯净底座——
//! 否则等于悄悄关掉了用户以为还在的保护。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（Rust 测试运行）。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::sha256::sha256_hex;

/// 规则资产 schema 标识（OD-ARCH-068 v1，OD-ARCH-082 v2）。
pub const PROFILE_RULES_SCHEMA_V1: &str = "omnidesk.profile-rules.v1";
pub const PROFILE_RULES_SCHEMA_V2: &str = "omnidesk.profile-rules.v2";
/// 当前默认/出厂 schema。
pub const PROFILE_RULES_SCHEMA: &str = PROFILE_RULES_SCHEMA_V2;

/// 程序内置的 Skill 清单（OD-ARCH-082 S2，086 加入 anysearch，089 加入 memory-report，OD-WR-503 加入 daily-summary，OD-WR-505 加入 period-review）。
///
/// OD-ARCH-131 起这只是「内置清单」，不再是绑定白名单：自建 Profile 可以带自己的
/// Skill（放在 `<应用数据根>/profiles/<名字>/skills/<skill名>/`），绑定校验改按
/// **名字形状**放行，见 `is_valid_skill_name`。
pub const ALLOWED_SKILLS: &[&str] = &[
    "material-writing",
    "knowledge-search",
    "knowledge-agent",
    "document-review",
    "memory-agent",
    "memory-report",
    "daily-summary",
    "period-review",
    "project-agent",
    "anysearch",
];
pub const MAX_SKILL_BINDINGS_PER_SCENE: usize = 3;
/// 规则资产文件名（位于 `<应用数据根>/profiles/<Profile 名>/` 下）。
pub const PROFILE_RULES_FILE: &str = "rules.json";
/// OD-ARCH-106 之前的单文件落点，只用于一次性兼容迁移（不删除、不覆盖）。
pub const LEGACY_PROFILE_RULES_FILE: &str = "discipline.rules.json";
/// 仓库内出厂默认规则包。编译期嵌入，仅由用户显式「安装/恢复默认」时写出。
pub const DEFAULT_RULES_JSON: &str = include_str!("../../../samples/profiles/discipline.rules.json");

const MAX_FILE_BYTES: usize = 128 * 1024;
const MAX_KEYWORDS: usize = 200;
const MAX_WRITING_RULES: usize = 60;
const MAX_RULE_CHARS: usize = 1200;
const MAX_KEYWORD_CHARS: usize = 40;
const MAX_SECTIONS_CHARS: usize = 400;

/// 隐私动作：mask=掩码后放行；reject=命中即拒绝；off=不处理。
const PRIVACY_MODES: &[&str] = &["mask", "reject", "off"];
/// 允许的思考档。default 表示不写该参数（跟随模型）。
pub const THINKING_LEVELS: &[&str] = &["default", "off", "low", "medium", "high", "max"];

fn default_mask() -> String {
    "mask".to_string()
}

/// 某一类隐私数据的处理方式。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyRules {
    /// 身份证号（18 位 / 17 位+X 连续数字串，以及这里列出的关键词）。
    #[serde(default = "default_mask")]
    pub id_card: String,
    /// 手机号（11 位、1 开头、第二位 3-9）。
    #[serde(default = "default_mask")]
    pub phone: String,
}

impl Default for PrivacyRules {
    fn default() -> Self {
        // 文件存在但没有 privacy 段时，按最保守的「掩码」处理：内容仍可提交，
        // 但数字串在出站前被替换。纯净底座走的是 RulePolicy::clean()，不经过这里。
        Self {
            id_card: default_mask(),
            phone: default_mask(),
        }
    }
}

/// 与隐私动作绑定的关键词（例如「身份证」「电话号码」）。
///
/// 单独分组的原因：这两个词本身不是「不得处理」的业务红线，而是「需要掩码」的提示。
/// 把它们混进 redline_keywords 会让 mask 模式自相矛盾（关键词先拒、正则再放行）。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyKeywords {
    #[serde(default)]
    pub id_card: Vec<String>,
    #[serde(default)]
    pub phone: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalRules {
    /// 写作铁律：逐条强制带进写作提示词。空数组 = 不带该段。
    #[serde(default)]
    pub writing_rules: Vec<String>,
    /// 命中即拒绝处理的关键词（业务红线）。
    #[serde(default)]
    pub redline_keywords: Vec<String>,
    #[serde(default)]
    pub privacy: PrivacyRules,
    #[serde(default)]
    pub privacy_keywords: PrivacyKeywords,
}

/// 单个场景的规则与默认思考档。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneRules {
    #[serde(default)]
    pub rules: String,
    /// 期望输出章节（每行一个）。为空表示不强制章节。
    #[serde(default)]
    pub sections: String,
    /// 场景默认思考档；用户手动选档始终优先。
    #[serde(default = "default_thinking")]
    pub thinking: String,
}

fn default_thinking() -> String {
    "low".to_string()
}

fn default_max_injected() -> usize {
    3
}

/// 单个场景的 Skill 绑定声明（OD-ARCH-082 S2）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSkillBinding {
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default = "default_max_injected")]
    pub max_injected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileRules {
    pub schema: String,
    pub profile: String,
    #[serde(default)]
    pub global: GlobalRules,
    #[serde(default)]
    pub scenes: BTreeMap<String, SceneRules>,
    #[serde(default)]
    pub skill_bindings: BTreeMap<String, SceneSkillBinding>,
}

impl ProfileRules {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PROFILE_RULES_SCHEMA_V1 && self.schema != PROFILE_RULES_SCHEMA_V2 {
            return Err(format!(
                "schema 必须是 {} 或 {}，实际是 {}",
                PROFILE_RULES_SCHEMA_V1, PROFILE_RULES_SCHEMA_V2, self.schema
            ));
        }
        if self.profile.trim().is_empty() {
            return Err("profile 不能为空".to_string());
        }
        if self.global.redline_keywords.len() > MAX_KEYWORDS {
            return Err(format!("redline_keywords 最多 {MAX_KEYWORDS} 条"));
        }
        for (label, list) in [
            ("redline_keywords", &self.global.redline_keywords),
            ("privacy_keywords.id_card", &self.global.privacy_keywords.id_card),
            ("privacy_keywords.phone", &self.global.privacy_keywords.phone),
        ] {
            for kw in list {
                if kw.trim().is_empty() {
                    return Err(format!("{label} 里不能有空词条"));
                }
                if kw.chars().count() > MAX_KEYWORD_CHARS {
                    return Err(format!("{label} 的词条超过 {MAX_KEYWORD_CHARS} 个字符"));
                }
            }
        }
        if self.global.writing_rules.len() > MAX_WRITING_RULES {
            return Err(format!("writing_rules 最多 {MAX_WRITING_RULES} 条"));
        }
        for rule in &self.global.writing_rules {
            if rule.trim().is_empty() {
                return Err("writing_rules 里不能有空条目".to_string());
            }
            if rule.chars().count() > MAX_RULE_CHARS {
                return Err(format!("单条写作铁律超过 {MAX_RULE_CHARS} 个字符"));
            }
        }
        for mode in [&self.global.privacy.id_card, &self.global.privacy.phone] {
            if !PRIVACY_MODES.contains(&mode.as_str()) {
                return Err(format!(
                    "privacy 取值必须是 {} 之一，实际是 {:?}",
                    PRIVACY_MODES.join(" / "),
                    mode
                ));
            }
        }
        if self.scenes.len() > 12 {
            return Err("scenes 最多 12 个场景".to_string());
        }
        for (key, scene) in &self.scenes {
            if key.trim().is_empty() {
                return Err("场景键不能为空".to_string());
            }
            if scene.rules.chars().count() > MAX_RULE_CHARS {
                return Err(format!("场景 {key} 的 rules 超过 {MAX_RULE_CHARS} 个字符"));
            }
            if scene.sections.chars().count() > MAX_SECTIONS_CHARS {
                return Err(format!("场景 {key} 的 sections 超过 {MAX_SECTIONS_CHARS} 个字符"));
            }
            if !THINKING_LEVELS.contains(&scene.thinking.as_str()) {
                return Err(format!(
                    "场景 {key} 的 thinking 必须是 {} 之一",
                    THINKING_LEVELS.join(" / ")
                ));
            }
        }
        if self.skill_bindings.len() > 12 {
            return Err("skill_bindings 最多 12 个场景".to_string());
        }
        for (key, binding) in &self.skill_bindings {
            if key.trim().is_empty() {
                return Err("skill_bindings 场景键不能为空".to_string());
            }
            if binding.max_injected == 0 || binding.max_injected > MAX_SKILL_BINDINGS_PER_SCENE {
                return Err(format!("场景 {key} 的 max_injected 必须在 1..={MAX_SKILL_BINDINGS_PER_SCENE} 之间"));
            }
            if binding.skills.len() > binding.max_injected {
                return Err(format!("场景 {key} 绑定的技能数（{}）超过上限 {}", binding.skills.len(), binding.max_injected));
            }
            let mut seen = std::collections::BTreeSet::new();
            for s in &binding.skills {
                if !is_valid_skill_name(s) {
                    return Err(format!(
                        "场景 {key} 绑定的技能 {s:?} 名字不合法（只允许小写字母、数字、连字符、下划线，最多 48 字符）"
                    ));
                }
                if !seen.insert(s) {
                    return Err(format!("场景 {key} 绑定的技能 {s:?} 重复"));
                }
            }
        }
        Ok(())
    }

    pub fn scene(&self, key: &str) -> Option<&SceneRules> {
        self.scenes.get(key)
    }
}

/// 一次加载的结果。UI 需要区分「没有规则」与「规则坏了」，所以不合并成 Option。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RulesView {
    /// true = 文件存在且解析通过；false = 不存在（纯净底座）或损坏。
    pub loaded: bool,
    pub path: String,
    /// 磁盘原文（损坏时也返回，供设置页只读展示）。
    pub text: Option<String>,
    pub content_sha256: Option<String>,
    /// 解析后的规则（仅 loaded=true 时给出）。
    pub rules: Option<ProfileRules>,
    /// 损坏时的原因；不存在时为 None。
    pub error: Option<String>,
    /// 出厂默认规则的摘要，供设置页提示「可安装默认规则包」。
    pub default_available: bool,
    /// 当前 Profile 不允许安装出厂规则包时的原因（OD-ARCH-106）。
    ///
    /// 只有基础标准会给出原因：行业规则包属于专业 Profile，装进干净底座会让
    /// 「切 Profile 才有没有业务规则」这条分界失效。`load_rules` 本身不知道
    /// Profile 名，因此该字段恒为 `None`，由命令层按实际 Profile 填写。
    #[serde(default)]
    pub default_blocked_reason: Option<String>,
    /// 旧单文件迁移到当前 Profile 目录失败的原因（OD-ARCH-106）。
    ///
    /// 与 `error` 分开：`error` 表示「当前规则文件坏了，提交会被拒绝」；这里表示
    /// 「旧位置的规则没能搬过来」，只影响这一份旧资产，不该拦住模型提交。
    #[serde(default)]
    pub migration_warning: Option<String>,
    /// 已绑定 Skill 的正文缓存（OD-ARCH-082 S4 注入链路）。
    #[serde(default)]
    pub skill_contents: BTreeMap<String, String>,
    /// 可绑定的 Skill 清单（OD-ARCH-129）。
    ///
    /// 由后端下发而不是前端硬编码：绑定要过 ALLOWED_SKILLS 的校验，
    /// 界面若另存一份清单，加一项 Skill 就会出现「能选、存不进去」。
    #[serde(default)]
    pub allowed_skills: Vec<String>,
    /// 该 Profile 自带的 Skill（OD-ARCH-131）。
    ///
    /// 放在 `<应用数据根>/profiles/<名字>/skills/<skill名>/` 的就是这一份；
    /// 界面据此标出「自带」，与内置清单区分开。
    #[serde(default)]
    pub local_skills: Vec<String>,
}

/// 可绑定的 Skill 清单（供界面渲染勾选片）。
pub fn allowed() -> Vec<String> {
    ALLOWED_SKILLS.iter().map(|s| s.to_string()).collect()
}

/// Skill 名是否可用作目录名（OD-ARCH-131）。
///
/// 绑定与装配都按名字拼路径，所以形状必须比「非空」更严：`..`、分隔符、
/// 大写、空白、中文标点都写不出一个可预期的目录。校验用这一条，不再用内置清单，
/// 这样用户自己写的 Skill 也能绑。
pub fn is_valid_skill_name(name: &str) -> bool {
    crate::settings::is_valid_profile_name(name)
}

/// Profile 目录下自带 Skill 源的根目录（OD-ARCH-131）。
///
/// 约定：`<应用数据根>/profiles/<名字>/skills/<skill名>/SKILL.md`。
/// 只在用户真的放/生成 Skill 时才建，基础标准下永远不建。
pub fn skill_source_root(profile_dir: &Path) -> PathBuf {
    profile_dir.join("skills")
}

/// 该 Profile 自带哪些 Skill（只有含 `SKILL.md` 的子目录算数）。
///
/// 只读：不建目录。名字不合形状的目录直接跳过——它们不可能被绑定选中。
pub fn local_skills(profile_dir: &Path) -> Vec<String> {
    let root = skill_source_root(profile_dir);
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| is_valid_skill_name(n))
        .filter(|n| root.join(n).join("SKILL.md").is_file())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// 界面可选的完整清单：内置 ∪ 该 Profile 自带（OD-ARCH-131）。
pub fn allowed_with_local(profile_dir: &Path) -> Vec<String> {
    let mut out = allowed();
    for name in local_skills(profile_dir) {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

pub fn rules_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join(PROFILE_RULES_FILE)
}

/// OD-ARCH-106 一次性迁移：把旧的单文件规则包复制进 `profiles/<Profile 名>/rules.json`。
///
/// 只对**非标准 Profile**执行；新位置已有文件一律不动（永不覆盖）。旧文件原样保留，
/// 作为迁移前的备份。内容先按当前 schema 校验，坏文件不搬。
/// 返回 `true` 表示本次复制发生了。
pub fn migrate_legacy_rules(
    profiles_root: &Path,
    profile: &str,
    standard_profile: &str,
) -> Result<bool, String> {
    if profile.trim().is_empty() || profile == standard_profile {
        return Ok(false);
    }
    let legacy = profiles_root.join(LEGACY_PROFILE_RULES_FILE);
    if !legacy.is_file() {
        return Ok(false);
    }
    let target_dir = profiles_root.join(profile);
    let target = rules_path(&target_dir);
    if target.exists() {
        return Ok(false);
    }
    let raw = fs::read(&legacy)
        .map_err(|e| format!("读取旧规则文件失败（{}）：{}", legacy.display(), e))?;
    if raw.len() > MAX_FILE_BYTES {
        return Err("旧规则文件过大，未迁移".to_string());
    }
    let text = String::from_utf8_lossy(&raw).to_string();
    parse_rules(&text)?;
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", target_dir.display(), e))?;
    fs::copy(&legacy, &target).map_err(|e| {
        format!(
            "迁移旧规则文件失败（{} → {}）：{}",
            legacy.display(),
            target.display(),
            e
        )
    })?;
    Ok(true)
}

pub fn parse_rules(text: &str) -> Result<ProfileRules, String> {
    let rules: ProfileRules = serde_json::from_str(text)
        .map_err(|e| format!("规则文件不是合法 JSON 或字段不符：{}（行 {} 列 {}）", e, e.line(), e.column()))?;
    rules.validate()?;
    Ok(rules)
}

/// 场景 → Skill 绑定的一次编辑输入（OD-ARCH-129）。
///
/// 空数组表示「这个场景不预注入任何 Skill」。`max_injected` 不从这里来：
/// 界面上绑定几个就是几个，上限由 `validate` 的 3 条兜底。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingChoice {
    pub scene: String,
    #[serde(default)]
    pub skills: Vec<String>,
}

/// 用一份完整的「场景 → Skill」选择替换规则包里的 `skill_bindings`（OD-ARCH-129）。
///
/// 刻意只动这一个字段：红线词、写作铁律、隐私策略与场景规则一个字节不改，
/// 用户手写过的其它内容因此不会被界面上的一次点选冲掉。整体校验失败就不产出
/// 任何文本——调用方据此拒绝写盘，磁盘停在原样。
pub fn set_skill_bindings(text: &str, choices: &[BindingChoice]) -> Result<String, String> {
    let mut rules = parse_rules(text)?;
    let mut bindings: BTreeMap<String, SceneSkillBinding> = BTreeMap::new();
    for choice in choices {
        let scene = choice.scene.trim();
        if scene.is_empty() {
            return Err("场景键不能为空".to_string());
        }
        let mut skills: Vec<String> = Vec::new();
        for raw in &choice.skills {
            let skill = raw.trim();
            if skill.is_empty() {
                continue;
            }
            if !skills.iter().any(|s| s == skill) {
                skills.push(skill.to_string());
            }
        }
        // 空选择 = 显式解绑：写进映射是为了让「刚清空」与「从来没配过」在文件里
        // 可区分，装配侧对两者的处理一致（都不预注入）。
        bindings.insert(
            scene.to_string(),
            SceneSkillBinding {
                max_injected: skills.len().max(1),
                skills,
            },
        );
    }
    rules.skill_bindings = bindings;
    rules.validate()?;
    let mut out = serde_json::to_string_pretty(&rules)
        .map_err(|e| format!("生成规则文本失败：{}", e))?;
    out.push('\n');
    Ok(out)
}

/// 读取规则资产。文件不存在返回 `loaded=false`（不是错误）；损坏返回 error 且 loaded=false。
pub fn load_rules(profile_dir: &Path) -> RulesView {
    let path = rules_path(profile_dir);
    let shown = path.display().to_string();
    let raw = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return RulesView {
                loaded: false,
                path: shown,
                text: None,
                content_sha256: None,
                rules: None,
                error: None,
                default_available: true,
                default_blocked_reason: None,
                migration_warning: None,
                skill_contents: BTreeMap::new(),
                allowed_skills: allowed_with_local(profile_dir),
                local_skills: local_skills(profile_dir),
            }
        }
        Err(e) => {
            return RulesView {
                loaded: false,
                path: shown,
                text: None,
                content_sha256: None,
                rules: None,
                error: Some(format!("读取规则文件失败：{}", e)),
                default_available: true,
                default_blocked_reason: None,
                migration_warning: None,
                skill_contents: BTreeMap::new(),
                allowed_skills: allowed_with_local(profile_dir),
                local_skills: local_skills(profile_dir),
            }
        }
    };
    if raw.len() > MAX_FILE_BYTES {
        return RulesView {
            loaded: false,
            path: shown,
            text: None,
            content_sha256: None,
            rules: None,
            error: Some(format!("规则文件过大（上限 {} KB）", MAX_FILE_BYTES / 1024)),
            default_available: true,
            default_blocked_reason: None,
            migration_warning: None,
            skill_contents: BTreeMap::new(),
                allowed_skills: allowed_with_local(profile_dir),
                local_skills: local_skills(profile_dir),
        };
    }
    let text = String::from_utf8_lossy(&raw).to_string();
    let sha = sha256_hex(&raw);
    match parse_rules(&text) {
        Ok(rules) => RulesView {
            loaded: true,
            path: shown,
            text: Some(text),
            content_sha256: Some(sha),
            rules: Some(rules),
            error: None,
            default_available: true,
            default_blocked_reason: None,
            migration_warning: None,
            skill_contents: BTreeMap::new(),
                allowed_skills: allowed_with_local(profile_dir),
                local_skills: local_skills(profile_dir),
        },
        Err(error) => RulesView {
            loaded: false,
            path: shown,
            text: Some(text),
            content_sha256: Some(sha),
            rules: None,
            error: Some(error),
            default_available: true,
            default_blocked_reason: None,
            migration_warning: None,
            skill_contents: BTreeMap::new(),
                allowed_skills: allowed_with_local(profile_dir),
                local_skills: local_skills(profile_dir),
        },
    }
}

/// 运行时策略：由规则资产编译出来的一次性查表结构。
#[derive(Debug, Clone, Default)]
pub struct RulePolicy {
    pub profile: Option<String>,
    reject_keywords: Vec<String>,
    privacy_id_keywords: Vec<String>,
    privacy_phone_keywords: Vec<String>,
    id_card_mode: PrivacyMode,
    phone_mode: PrivacyMode,
    pub writing_rules: Vec<String>,
    pub scenes: BTreeMap<String, SceneRules>,
    pub skill_bindings: BTreeMap<String, SceneSkillBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrivacyMode {
    #[default]
    Off,
    Mask,
    Reject,
}

fn parse_mode(raw: &str) -> PrivacyMode {
    match raw {
        "mask" => PrivacyMode::Mask,
        "reject" => PrivacyMode::Reject,
        _ => PrivacyMode::Off,
    }
}

impl RulePolicy {
    /// 纯净底座：零关键词、零铁律、不掩码、不改写。
    pub fn clean() -> Self {
        Self::default()
    }

    pub fn from_rules(rules: &ProfileRules) -> Self {
        Self {
            profile: Some(rules.profile.clone()),
            reject_keywords: rules.global.redline_keywords.clone(),
            privacy_id_keywords: rules.global.privacy_keywords.id_card.clone(),
            privacy_phone_keywords: rules.global.privacy_keywords.phone.clone(),
            id_card_mode: parse_mode(&rules.global.privacy.id_card),
            phone_mode: parse_mode(&rules.global.privacy.phone),
            writing_rules: rules.global.writing_rules.clone(),
            scenes: rules.scenes.clone(),
            skill_bindings: rules.skill_bindings.clone(),
        }
    }

    pub fn is_clean(&self) -> bool {
        self.profile.is_none()
    }

    pub fn id_card_mode(&self) -> PrivacyMode {
        self.id_card_mode
    }

    pub fn phone_mode(&self) -> PrivacyMode {
        self.phone_mode
    }

    pub fn reject_keywords(&self) -> &[String] {
        &self.reject_keywords
    }

    pub fn privacy_keywords(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        if self.id_card_mode != PrivacyMode::Off {
            out.extend(self.privacy_id_keywords.iter().map(|s| s.as_str()));
        }
        if self.phone_mode != PrivacyMode::Off {
            out.extend(self.privacy_phone_keywords.iter().map(|s| s.as_str()));
        }
        out
    }

    /// 关键词是否需要「命中即拒绝」。
    pub fn privacy_keyword_rejects(&self, keyword: &str) -> bool {
        (self.id_card_mode == PrivacyMode::Reject && self.privacy_id_keywords.iter().any(|k| k == keyword))
            || (self.phone_mode == PrivacyMode::Reject && self.privacy_phone_keywords.iter().any(|k| k == keyword))
    }

    pub fn scene_rules(&self, key: &str) -> Option<&SceneRules> {
        self.scenes.get(key)
    }
}

/// 掩码计数：只回传类别与数量，不回显原文。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct MaskCounts {
    pub id_card: u64,
    pub phone: u64,
}

impl MaskCounts {
    pub fn any(&self) -> bool {
        self.id_card > 0 || self.phone > 0
    }
}

/// 掩码：保留前 3 位与后 2 位，中间替换为 `*`，长度不变。
fn mask_run(chars: &[char]) -> String {
    if chars.len() <= 5 {
        return chars.iter().collect();
    }
    let mut out = String::new();
    for (idx, ch) in chars.iter().enumerate() {
        if idx < 3 || idx + 2 >= chars.len() {
            out.push(*ch);
        } else {
            out.push('*');
        }
    }
    out
}

/// 对文本做隐私掩码。识别口径与 `task_shell::scan_digit_runs` 一致：
/// 18 位连续数字、17 位+X、11 位 1[3-9] 开头。其余内容原样保留。
pub fn mask_privacy(text: &str, policy: &RulePolicy) -> (String, MaskCounts) {
    if policy.id_card_mode != PrivacyMode::Mask && policy.phone_mode != PrivacyMode::Mask {
        return (text.to_string(), MaskCounts::default());
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut counts = MaskCounts::default();
    let mut i = 0usize;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let run_len = i - start;
        let mut consumed_x = false;
        if run_len == 18 && policy.id_card_mode == PrivacyMode::Mask {
            counts.id_card += 1;
            out.push_str(&mask_run(&chars[start..i]));
            continue;
        }
        if run_len == 17
            && i < chars.len()
            && (chars[i] == 'X' || chars[i] == 'x')
            && policy.id_card_mode == PrivacyMode::Mask
        {
            counts.id_card += 1;
            out.push_str(&mask_run(&chars[start..i]));
            out.push(chars[i]);
            i += 1;
            consumed_x = true;
        } else if run_len == 11
            && chars[start] == '1'
            && ('3'..='9').contains(&chars[start + 1])
            && policy.phone_mode == PrivacyMode::Mask
        {
            counts.phone += 1;
            out.push_str(&mask_run(&chars[start..i]));
            continue;
        }
        if !consumed_x {
            out.extend(&chars[start..i]);
        }
    }
    (out, counts)
}

/// 原文被掩码修改过吗（用于判断是否需要在回执里提示）。
pub fn masked_differs(original: &str, masked: &str) -> bool {
    original != masked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discipline() -> ProfileRules {
        parse_rules(DEFAULT_RULES_JSON).expect("出厂默认规则包必须能解析")
    }

    /// 测试用的唯一后缀（并行跑单测时避免共用同一个临时目录）。
    fn now_stamp() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    }

    #[test]
    fn default_pack_is_valid_and_has_expected_shape() {
        let rules = discipline();
        assert_eq!(rules.schema, PROFILE_RULES_SCHEMA);
        assert_eq!(rules.profile, "discipline");
        let total = rules.global.redline_keywords.len()
            + rules.global.privacy_keywords.id_card.len()
            + rules.global.privacy_keywords.phone.len();
        assert_eq!(total, 16, "16 个词条必须完整保留（14 红线 + 2 隐私归类）");
        assert_eq!(rules.global.writing_rules.len(), 7);
        assert_eq!(rules.scene("weekly").unwrap().thinking, "low");
        assert_eq!(rules.scene("writing").unwrap().thinking, "low");
        assert_eq!(rules.scene("project").unwrap().thinking, "off");
        assert!(!rules.scene("weekly").unwrap().sections.is_empty());
        // OD-ARCH-108：绑定按流程拆，每条流程只绑自己的底线技能；其余能力按需加载。
        assert!(ALLOWED_SKILLS.contains(&"anysearch"));
        assert!(ALLOWED_SKILLS.contains(&"memory-report"));
        // 旧键 weekly 保留为随手记的兼容别名，不能指向周报技能。
        let weekly = rules.skill_bindings.get("weekly").expect("weekly 必须有绑定");
        assert_eq!(weekly.skills, vec!["memory-agent"]);
        let note = rules
            .skill_bindings
            .get("weekly-note")
            .expect("随手记流程必须有绑定");
        assert_eq!(note.skills, vec!["memory-agent"]);
        let report = rules
            .skill_bindings
            .get("weekly-report")
            .expect("周报流程必须有绑定");
        assert_eq!(report.skills, vec!["memory-report"]);
        assert!(weekly.skills.len() <= weekly.max_injected);
        assert!(note.skills.len() <= note.max_injected);
        assert!(report.skills.len() <= report.max_injected);
        // anysearch / knowledge-search / document-review / knowledge-agent 不进任何绑定。
        for scene in rules.skill_bindings.values() {
            for ondemand in ["anysearch", "knowledge-search", "document-review", "knowledge-agent"] {
                assert!(
                    !scene.skills.iter().any(|s| s == ondemand),
                    "{ondemand} 不应被预注入"
                );
            }
        }
    }

    #[test]
    fn clean_policy_masks_nothing_and_rejects_nothing() {
        let policy = RulePolicy::clean();
        assert!(policy.is_clean());
        assert!(policy.reject_keywords().is_empty());
        let text = "虚构案件占位 110101199003077777 13800138000";
        let (masked, counts) = mask_privacy(text, &policy);
        assert_eq!(masked, text);
        assert!(!counts.any());
        assert!(!policy.privacy_keyword_rejects("身份证"));
    }

    #[test]
    fn mask_keeps_first_three_and_last_two() {
        let policy = RulePolicy::from_rules(&discipline());
        let (masked, counts) = mask_privacy("证件 110101199003077777 电话 13800138000", &policy);
        assert_eq!(counts.id_card, 1);
        assert_eq!(counts.phone, 1);
        assert!(masked.contains("110*************77"), "实际：{masked}");
        assert!(masked.contains("138******00"), "实际：{masked}");
        // 长度不变，其余文字不动。
        assert_eq!(masked.chars().count(), "证件 110101199003077777 电话 13800138000".chars().count());
        assert!(masked.starts_with("证件 "));
    }

    #[test]
    fn mask_handles_x_suffix_and_ignores_other_runs() {
        let policy = RulePolicy::from_rules(&discipline());
        let (masked, counts) = mask_privacy("11010119900307777X 12345678901 123456789", &policy);
        assert_eq!(counts.id_card, 1);
        assert_eq!(counts.phone, 0, "1 开头的 11 位才按手机号处理");
        assert!(masked.starts_with("110************77X"), "实际：{masked}");
        assert!(masked.ends_with("12345678901 123456789"));
    }

    #[test]
    fn reject_mode_leaves_text_untouched() {
        let mut rules = discipline();
        rules.global.privacy.id_card = "reject".to_string();
        rules.global.privacy.phone = "reject".to_string();
        let policy = RulePolicy::from_rules(&rules);
        let text = "110101199003077777";
        let (masked, counts) = mask_privacy(text, &policy);
        assert_eq!(masked, text);
        assert!(!counts.any());
        assert!(policy.privacy_keyword_rejects("身份证"));
        assert!(policy.privacy_keyword_rejects("电话号码"));
    }

    #[test]
    fn damaged_json_is_an_error_not_a_silent_reset() {
        assert!(parse_rules("{").is_err());
        let mut wrong = discipline();
        wrong.schema = "omnidesk.profile-rules.v0".to_string();
        assert!(wrong.validate().is_err());
        let mut bad_skill = discipline();
        bad_skill.skill_bindings.insert(
            "weekly".to_string(),
            SceneSkillBinding {
                // OD-ARCH-131：白名单改为名字形状校验，`unknown-skill` 形状合法、应当放行；
                // 这里换成写不出目录的形状（带空格与大写），仍须整份拒绝。
                skills: vec!["Bad Skill".to_string()],
                max_injected: 3,
            },
        );
        assert!(bad_skill.validate().is_err());
        let mut bad_mode = discipline();
        bad_mode.global.privacy.phone = "ignore".to_string();
        assert!(bad_mode.validate().is_err());
        let mut bad_thinking = discipline();
        bad_thinking.scenes.get_mut("weekly").unwrap().thinking = "turbo".to_string();
        assert!(bad_thinking.validate().is_err());
    }

    #[test]
    fn missing_file_is_clean_not_error() {
        let base = std::env::temp_dir().join(format!("omnidesk-rules-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let view = load_rules(&base);
        assert!(!view.loaded);
        assert!(view.error.is_none());
        assert!(view.text.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn damaged_file_reports_error_and_keeps_text() {
        let base = std::env::temp_dir().join(format!("omnidesk-rules-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(rules_path(&base), "{ not json").unwrap();
        let view = load_rules(&base);
        assert!(!view.loaded);
        assert!(view.error.is_some());
        assert!(view.text.is_some());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn v1_rules_without_skill_bindings_parses_and_validates() {
        let v1_json = r#"{
            "schema": "omnidesk.profile-rules.v1",
            "profile": "discipline",
            "global": {},
            "scenes": {}
        }"#;
        let parsed = parse_rules(v1_json).expect("v1 规则必须原样可解析");
        assert_eq!(parsed.schema, PROFILE_RULES_SCHEMA_V1);
        assert!(parsed.skill_bindings.is_empty());
        let policy = RulePolicy::from_rules(&parsed);
        assert!(policy.skill_bindings.is_empty());
    }

    /// OD-ARCH-106：旧单文件只对非标准 Profile 迁移一次，新位置已有文件一律不覆盖。
    #[test]
    fn migrates_legacy_rules_once_and_never_overwrites() {
        let root = std::env::temp_dir().join(format!(
            "od-rules-migrate-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let legacy = root.join(LEGACY_PROFILE_RULES_FILE);
        std::fs::write(&legacy, DEFAULT_RULES_JSON).unwrap();

        // 标准 Profile 不迁移：干净底座不能因为旧文件存在就长出行业规则。
        assert!(!migrate_legacy_rules(&root, "omnidesk-base", "omnidesk-base").unwrap());
        assert!(!root.join("omnidesk-base").exists());

        assert!(migrate_legacy_rules(&root, "discipline", "omnidesk-base").unwrap());
        let target = rules_path(&root.join("discipline"));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), DEFAULT_RULES_JSON);
        // 旧文件保留，作为迁移前备份。
        assert!(legacy.is_file());

        // 新位置已有文件：不再覆盖用户改动。
        std::fs::write(&target, r#"{"schema":"omnidesk.profile-rules.v1","profile":"用户改过","global":{},"scenes":{}}"#).unwrap();
        assert!(!migrate_legacy_rules(&root, "discipline", "omnidesk-base").unwrap());
        assert!(std::fs::read_to_string(&target).unwrap().contains("用户改过"));

        // 坏文件不搬：宁可如实报错，也不把解析不过的内容复制进新位置。
        std::fs::write(&target, "{ not json").unwrap();
        std::fs::write(&legacy, "{ not json").unwrap();
        let _ = std::fs::remove_dir_all(root.join("discipline"));
        assert!(migrate_legacy_rules(&root, "discipline", "omnidesk-base").is_err());
        assert!(!rules_path(&root.join("discipline")).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// OD-ARCH-129：整表替换绑定，只动 skill_bindings。
    #[test]
    fn set_skill_bindings_replaces_only_bindings() {
        let choices = vec![
            BindingChoice { scene: "weekly".into(), skills: vec!["memory-agent".into()] },
            BindingChoice { scene: "weekly-note".into(), skills: vec!["memory-agent".into()] },
            BindingChoice { scene: "weekly-report".into(), skills: vec!["memory-report".into()] },
        ];
        let text = set_skill_bindings(DEFAULT_RULES_JSON, &choices).unwrap();
        let parsed = parse_rules(&text).unwrap();
        assert_eq!(parsed.skill_bindings.len(), 3);
        assert_eq!(parsed.skill_bindings["weekly-report"].skills, vec!["memory-report"]);
        let original = parse_rules(DEFAULT_RULES_JSON).unwrap();
        assert_eq!(parsed.global, original.global);
        assert_eq!(parsed.scenes, original.scenes);
        assert_eq!(parsed.profile, original.profile);
    }

    /// OD-ARCH-129：空选择表示解绑，且 max_injected 不会因此变成 0（校验下限是 1）。
    #[test]
    fn set_skill_bindings_allows_explicit_unbind() {
        let choices = vec![BindingChoice { scene: "weekly".into(), skills: Vec::new() }];
        let text = set_skill_bindings(DEFAULT_RULES_JSON, &choices).unwrap();
        let parsed = parse_rules(&text).unwrap();
        assert!(parsed.skill_bindings["weekly"].skills.is_empty());
        assert_eq!(parsed.skill_bindings["weekly"].max_injected, 1);
    }

    /// OD-ARCH-129：重复项去重，避免界面连点生成一个存不进去的规则包。
    #[test]
    fn set_skill_bindings_dedupes_and_sizes_max_injected() {
        let choices = vec![BindingChoice {
            scene: "writing".into(),
            skills: vec!["material-writing".into(), "material-writing".into(), "anysearch".into()],
        }];
        let text = set_skill_bindings(DEFAULT_RULES_JSON, &choices).unwrap();
        let parsed = parse_rules(&text).unwrap();
        assert_eq!(parsed.skill_bindings["writing"].skills, vec!["material-writing", "anysearch"]);
        assert_eq!(parsed.skill_bindings["writing"].max_injected, 2);
    }

    /// OD-ARCH-129：超过每场景 3 条上限与写不成目录的名字，都在这里被拦下。
    ///
    /// OD-ARCH-131：白名单换成名字形状校验后，`made-up-skill` 这种形状合法的名字
    /// 必须**放行**（它可能是这个 Profile 自带的 Skill）；真正该拦的是写不出目录的形状。
    #[test]
    fn set_skill_bindings_rejects_over_limit_and_bad_shape() {
        let too_many = vec![BindingChoice {
            scene: "weekly".into(),
            skills: vec![
                "memory-agent".into(),
                "memory-report".into(),
                "anysearch".into(),
                "knowledge-search".into(),
            ],
        }];
        assert!(set_skill_bindings(DEFAULT_RULES_JSON, &too_many).is_err());
        let bad_shape = vec![BindingChoice {
            scene: "weekly".into(),
            skills: vec!["../escape".into()],
        }];
        assert!(set_skill_bindings(DEFAULT_RULES_JSON, &bad_shape).is_err());
        let spaced = vec![BindingChoice {
            scene: "weekly".into(),
            skills: vec!["Bad Skill".into()],
        }];
        assert!(set_skill_bindings(DEFAULT_RULES_JSON, &spaced).is_err());
    }

    /// OD-ARCH-131：自建 Profile 自带的 Skill 名（形状合法、不在内置清单）能存进绑定。
    #[test]
    fn set_skill_bindings_accepts_custom_shaped_name() {
        let choices = vec![BindingChoice {
            scene: "weekly-note".into(),
            skills: vec!["my-office-note".into()],
        }];
        let text = set_skill_bindings(DEFAULT_RULES_JSON, &choices).unwrap();
        let parsed = parse_rules(&text).unwrap();
        assert_eq!(parsed.skill_bindings["weekly-note"].skills, vec!["my-office-note"]);
    }

    /// OD-ARCH-131：界面清单 = 内置 ∪ 该 Profile 自带；不带目录时就是内置 8 项。
    #[test]
    fn allowed_with_local_unions_profile_owned_skills() {
        let root = std::env::temp_dir().join(format!(
            "omnidesk-skill-src-{}-{}",
            std::process::id(),
            now_stamp()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("skills").join("my-office-note")).unwrap();
        fs::write(
            root.join("skills").join("my-office-note").join("SKILL.md"),
            "# demo\n",
        )
        .unwrap();
        // 没有 SKILL.md 的目录与形状不合法的目录都不算。
        fs::create_dir_all(root.join("skills").join("empty-one")).unwrap();
        fs::create_dir_all(root.join("skills").join("Bad Name")).unwrap();
        fs::write(root.join("skills").join("Bad Name").join("SKILL.md"), "# x\n").unwrap();
        let list = allowed_with_local(&root);
        assert!(list.contains(&"my-office-note".to_string()));
        assert!(!list.contains(&"empty-one".to_string()));
        assert!(!list.iter().any(|n| n == "Bad Name"));
        assert_eq!(list.len(), ALLOWED_SKILLS.len() + 1);
        let _ = fs::remove_dir_all(&root);
    }
}
