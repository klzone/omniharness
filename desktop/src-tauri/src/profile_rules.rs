//! Profile 规则资产（omnidesk.profile-rules.v1）——OD-ARCH-068 S1/S2/S3。
//!
//! 平台代码只保留**机制**，不内置任何行业规则：红线词表、写作铁律、场景规则与
//! 思考档、隐私策略都写在 Profile 规则资产里。文件不存在 = **纯净底座**（零关键词、
//! 零铁律、不掩码、不改写），行为与官方 DSH-desktop 无实质区别。
//!
//! 落点：`<应用数据根>/profiles/discipline.rules.json`（仓库外，用户自己的数据根）。
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

/// 允许绑定的八项 Skill 清单（OD-ARCH-082 S2，086 加入 anysearch，089 加入 memory-report）。
pub const ALLOWED_SKILLS: &[&str] = &[
    "material-writing",
    "knowledge-search",
    "knowledge-agent",
    "document-review",
    "memory-agent",
    "memory-report",
    "project-agent",
    "anysearch",
];
pub const MAX_SKILL_BINDINGS_PER_SCENE: usize = 3;
/// 规则资产文件名（位于应用数据根下的 profiles/ 下）。
pub const PROFILE_RULES_FILE: &str = "discipline.rules.json";
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
                if !ALLOWED_SKILLS.contains(&s.as_str()) {
                    return Err(format!("场景 {key} 绑定的技能 {s:?} 不在合法技能清单内"));
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
    /// 已绑定 Skill 的正文缓存（OD-ARCH-082 S4 注入链路）。
    #[serde(default)]
    pub skill_contents: BTreeMap<String, String>,
}

pub fn rules_path(profiles_dir: &Path) -> PathBuf {
    profiles_dir.join(PROFILE_RULES_FILE)
}

pub fn parse_rules(text: &str) -> Result<ProfileRules, String> {
    let rules: ProfileRules = serde_json::from_str(text)
        .map_err(|e| format!("规则文件不是合法 JSON 或字段不符：{}（行 {} 列 {}）", e, e.line(), e.column()))?;
    rules.validate()?;
    Ok(rules)
}

/// 读取规则资产。文件不存在返回 `loaded=false`（不是错误）；损坏返回 error 且 loaded=false。
pub fn load_rules(profiles_dir: &Path) -> RulesView {
    let path = rules_path(profiles_dir);
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
            skill_contents: BTreeMap::new(),
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
            skill_contents: BTreeMap::new(),
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
            skill_contents: BTreeMap::new(),
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
            skill_contents: BTreeMap::new(),
        },
        Err(error) => RulesView {
            loaded: false,
            path: shown,
            text: Some(text),
            content_sha256: Some(sha),
            rules: None,
            error: Some(error),
            default_available: true,
            skill_contents: BTreeMap::new(),
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
        // OD-ARCH-089：weekly 拆成随手记 + 周报两个技能，anysearch 移出预注入但仍在白名单。
        assert!(ALLOWED_SKILLS.contains(&"anysearch"));
        assert!(ALLOWED_SKILLS.contains(&"memory-report"));
        let weekly = rules.skill_bindings.get("weekly").expect("weekly 必须有绑定");
        assert_eq!(weekly.skills.len(), 3);
        assert_eq!(
            weekly.skills,
            vec!["memory-agent", "memory-report", "knowledge-search"]
        );
        assert!(weekly.skills.len() <= weekly.max_injected);
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
                skills: vec!["unknown-skill".to_string()],
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
}
