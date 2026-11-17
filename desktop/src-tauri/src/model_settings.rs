//! 原生设置页的模型配置层（OD-ARCH-053 S1，OD-ARCH-065 S2/S3）。
//!
//! 事实基础（源码核实）：
//!   * 引擎实际路由来自 '~/OmniDesk/app/config/omnidesk-model.yml'（OmniDesk 自有补丁，
//!     宿主以 --patch 传给 DSH）；
//!   * DSH 的 provider 配置**只接受 apiKeyEnv 引用名，不接受明文 key**
//!     （packages/llm/llm-pi-ai/src/index.ts::resolveApiKey），密钥只能写进 DSH 凭据服务
//!     （$DSH_HOME/.credentials.yaml，经官方 credentials/set RPC）。
//!
//! 因此本模块只保存**非机密**字段，密钥值从不进入本文件、日志、返回值或 Git：
//!   * 配置文件：'<应用数据根>/config/omnidesk.model-settings.v2.json'；
//!   * 覆盖文件：'<应用数据根>/config/omnidesk-model.yml'（只有 baseURL / api / apiKeyEnv / 模型 id）。
//!
//! **补丁层是浅合并**（applyEntryPatches 对 config 整体替换），所以覆盖文件必须写出
//! 「合并后的完整路由集合」：Profile 自带的路由 + OmniDesk 自己的路由（同名覆盖）。
//! 合并所需的 Profile 路由由 model_routes::base_routes 用 --dump-config 读取（不含凭据值）。
//!
//! 文件损坏时**不静默重置**：读取返回错误，界面只读显示。
//! 写入一律临时文件 + 原子替换，权限 0600。
//!
//! 证据等级：static（源码事实）+ script（模块内单测）。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MODEL_SETTINGS_SCHEMA: &str = "omnidesk.model-settings.v2";
pub const MODEL_SETTINGS_FILE: &str = "omnidesk.model-settings.v2.json";
/// 旧版单路由配置；只在迁移时读取，迁移成功后改名为 .bak。
pub const LEGACY_SETTINGS_FILE: &str = "omnidesk.model-settings.v1.json";
pub const LEGACY_SETTINGS_SCHEMA: &str = "omnidesk.model-settings.v1";

/// DSH 支持的三种 api 协议（packages/llm/llm-pi-ai/src/provider.ts）。
pub const ALLOWED_APIS: &[&str] = &["openai-completions", "openai-responses", "anthropic-messages"];

const ALLOWED_EFFORTS: &[&str] = &["default", "low", "medium", "high", "max"];

/// 一个模型条目：id 必填，name 缺省等于 id。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// 上下文窗口（token）。写到 DSH 的 `contextWindow`；未设置则沿用端点/内置目录。
    /// 这是配置声明，不代表端点一定接受这么大；界面必须如实说明。
    #[serde(default, rename = "contextWindow", skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// 用户手动声明的思考档位映射（DSH `reasoningEfforts`，OD-ARCH-181）。
    /// 设置后优先于 Profile 继承与宿主已验证清单；未设置则沿用既有合并规则。
    /// 声明不实会在真实调用时报错，用户自行修正——这正是手动控制的语义。
    #[serde(default, rename = "reasoningEfforts", skip_serializing_if = "Option::is_none")]
    pub reasoning_efforts: Option<std::collections::BTreeMap<String, String>>,
}

impl ModelEntry {
    fn label(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }
}

/// 一个提供商的完整档案。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSettings {
    /// provider 标识（也是 DSH 配置里的键名）。
    pub provider: String,
    pub display_name: String,
    pub base_url: String,
    pub api: String,
    /// 凭据引用名（DSH 的 apiKeyEnv）。每个 provider 必须不同，避免串号。
    pub api_key_ref: String,
    pub models: Vec<ModelEntry>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    /// 思考模式：auto 跟随模型；disabled 对支持 thinking 参数的端点显式关闭。
    #[serde(default)]
    pub thinking_mode: String,
}

impl ProviderSettings {
    pub fn new(provider: &str) -> Self {
        Self {
            provider: provider.to_string(),
            display_name: String::new(),
            base_url: String::new(),
            api: "openai-completions".to_string(),
            api_key_ref: String::new(),
            models: Vec::new(),
            reasoning_effort: None,
            thinking_mode: "auto".to_string(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if !valid_ident(&self.provider) {
            return Err("provider 名称只能用字母、数字、下划线、短横线，且以字母或下划线开头".to_string());
        }
        if self.display_name.trim().is_empty() {
            return Err(format!("{}：显示名称不能为空", self.provider));
        }
        validate_base_url(&self.base_url)?;
        if !ALLOWED_APIS.contains(&self.api.as_str()) {
            return Err(format!(
                "{}：api 协议只支持：{}（收到 {}）",
                self.provider,
                ALLOWED_APIS.join(" / "),
                self.api
            ));
        }
        if !matches!(self.thinking_mode.as_str(), "" | "auto" | "disabled") {
            return Err(format!(
                "{}：thinking_mode 只支持 auto / disabled（收到 {}）",
                self.provider, self.thinking_mode
            ));
        }
        if !valid_ident(&self.api_key_ref) {
            return Err(format!(
                "{}：凭据名只能用字母、数字、下划线，且以字母或下划线开头（例如 AGNESCN_API_KEY）",
                self.provider
            ));
        }
        if self.models.is_empty() {
            return Err(format!("{}：至少填一个模型", self.provider));
        }
        let mut seen: Vec<&str> = Vec::new();
        for m in &self.models {
            let id = m.id.trim();
            if id.is_empty() || id.contains(['\r', '\n']) {
                return Err(format!("{}：模型 id 不能为空，也不能含换行", self.provider));
            }
            if seen.contains(&id) {
                return Err(format!("{}：模型 {} 重复", self.provider, id));
            }
            seen.push(id);
            if m.name.contains(['\r', '\n']) {
                return Err(format!("{}：模型名称不能含换行", self.provider));
            }
            if let Some(efforts) = &m.reasoning_efforts {
                for (level, value) in efforts {
                    if !matches!(
                        level.as_str(),
                        "off" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
                    ) {
                        return Err(format!(
                            "{} / {}：思考档位只支持 off / minimal / low / medium / high / xhigh / max（收到 {}）",
                            self.provider, id, level
                        ));
                    }
                    if value.trim().is_empty() || value.contains(['\r', '\n']) {
                        return Err(format!(
                            "{} / {}：档位 {} 的端点取值不能为空或含换行",
                            self.provider, id, level
                        ));
                    }
                }
            }
            if let Some(window) = m.context_window {
                if window == 0 || window > 100_000_000 {
                    return Err(format!(
                        "{}：模型 {} 的上下文窗口必须是 1 到 100000000 之间的整数",
                        self.provider, id
                    ));
                }
            }
        }
        if let Some(effort) = &self.reasoning_effort {
            if !ALLOWED_EFFORTS.contains(&effort.as_str()) {
                return Err(format!(
                    "{}：思考等级只支持：{}（收到 {}）",
                    self.provider,
                    ALLOWED_EFFORTS.join(" / "),
                    effort
                ));
            }
        }
        Ok(())
    }
}

/// 默认路由。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultRoute {
    pub provider: String,
    pub model: String,
}

/// 多提供商配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSettings {
    pub schema: String,
    pub providers: Vec<ProviderSettings>,
    pub default: DefaultRoute,
    /// 旧版遗留字段：任务等待上限（秒）。2026-10-23 起宿主不再向引擎下发任何
    /// 等待上限（任务只在用户取消或底座报错时结束）。字段保留只为兼容已存在的
    /// 配置文件（结构体是 deny_unknown_fields，直接删会让老文件读不出来），
    /// 读取后不使用、也不再校验。
    #[serde(default = "default_task_timeout_seconds")]
    pub task_timeout_seconds: u64,
    /// 旧版遗留字段：联网检索等待上限（秒）。同上，仅为兼容旧配置而保留。
    #[serde(default = "default_search_task_timeout_seconds")]
    pub search_task_timeout_seconds: u64,
}

pub const DEFAULT_TASK_TIMEOUT_SECONDS: u64 = 300;
pub const DEFAULT_SEARCH_TASK_TIMEOUT_SECONDS: u64 = 600;

fn default_task_timeout_seconds() -> u64 {
    DEFAULT_TASK_TIMEOUT_SECONDS
}

fn default_search_task_timeout_seconds() -> u64 {
    DEFAULT_SEARCH_TASK_TIMEOUT_SECONDS
}

impl ModelSettings {
    pub fn new() -> Self {
        Self {
            schema: MODEL_SETTINGS_SCHEMA.to_string(),
            providers: Vec::new(),
            default: DefaultRoute {
                provider: String::new(),
                model: String::new(),
            },
            task_timeout_seconds: DEFAULT_TASK_TIMEOUT_SECONDS,
            search_task_timeout_seconds: DEFAULT_SEARCH_TASK_TIMEOUT_SECONDS,
        }
    }

    pub fn find(&self, provider: &str) -> Option<&ProviderSettings> {
        self.providers.iter().find(|p| p.provider == provider)
    }

    pub fn find_mut(&mut self, provider: &str) -> Option<&mut ProviderSettings> {
        self.providers.iter_mut().find(|p| p.provider == provider)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != MODEL_SETTINGS_SCHEMA {
            return Err(format!(
                "配置版本不受支持：{}（本版只认 {}）",
                self.schema, MODEL_SETTINGS_SCHEMA
            ));
        }
        if self.providers.is_empty() {
            return Err("至少要有一个提供商；没有提供商就没有可用路由".to_string());
        }
        let mut names: Vec<&str> = Vec::new();
        let mut refs: Vec<&str> = Vec::new();
        for p in &self.providers {
            p.validate()?;
            if names.contains(&p.provider.as_str()) {
                return Err(format!("提供商 {} 重复", p.provider));
            }
            names.push(&p.provider);
            if refs.contains(&p.api_key_ref.as_str()) {
                return Err(format!(
                    "凭据引用 {} 被两个提供商共用；每个提供商必须有独立的凭据引用，否则会串号",
                    p.api_key_ref
                ));
            }
            refs.push(&p.api_key_ref);
        }
        let target = self.find(&self.default.provider).ok_or_else(|| {
            format!("默认路由指向不存在的提供商：{}", self.default.provider)
        })?;
        if !target.models.iter().any(|m| m.id == self.default.model) {
            return Err(format!(
                "默认模型 {} 不在提供商 {} 的模型列表里",
                self.default.model, self.default.provider
            ));
        }
        Ok(())
    }

    /// 生成 DSH 补丁层（只含非机密字段）。
    ///
    /// base 是引擎 Profile 当前生效的路由（来自 --dump-config）。合并口径：
    /// 先按 Profile 顺序输出，OmniDesk 同名项覆盖；OmniDesk 独有的追加在后。
    pub fn render_overlay(&self, base: &[ProviderRoute]) -> String {
        let mut out = String::new();
        out.push_str("- id: llm-pi-ai\n  name: \"@deepseek-ai/dsh-llm-pi-ai\"\n  config:\n    providers:\n");

        let mut emitted: Vec<String> = Vec::new();
        for route in base.iter().filter(|r| r.renderable()) {
            if let Some(own) = self.find(&route.id) {
                out.push_str(&render_provider(own, Some(route)));
                emitted.push(own.provider.clone());
            } else {
                out.push_str(&render_route(route));
                emitted.push(route.id.clone());
            }
        }
        for own in &self.providers {
            if !emitted.iter().any(|e| e == &own.provider) {
                out.push_str(&render_provider(own, None));
            }
        }

        let effort = self
            .find(&self.default.provider)
            .filter(|p| p.thinking_mode != "disabled")
            .and_then(|p| p.reasoning_effort.as_deref())
            .filter(|e| !e.is_empty() && *e != "default");
        out.push_str("- id: acp\n  name: \"@deepseek-ai/dsh-acp\"\n  config:\n");
        out.push_str(&format!(
            "    provider: {}\n    model: {}\n",
            self.default.provider,
            yaml_scalar(&self.default.model)
        ));
        if let Some(effort) = effort {
            out.push_str(&format!("    reasoningEffort: {}\n", effort));
        }
        // OD-ARCH-153：web-api 通道会话读 `agent-default-model`，不读 `acp`；
        // 思考等级必须两处都写，否则界面上选了档位、web 会话仍是 provider 默认档。
        out.push_str("- id: agent-default-model\n  name: \"@deepseek-ai/dsh-agent-default-model\"\n  config:\n");
        out.push_str(&format!(
            "    provider: {}\n    model: {}\n",
            self.default.provider,
            yaml_scalar(&self.default.model)
        ));
        if let Some(effort) = effort {
            out.push_str(&format!("    reasoningEffort: {}\n", effort));
        }
        out
    }
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self::new()
    }
}

/// 引擎 Profile 里的一条非机密路由（model-routes.mjs --dump-routes 的输出）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRoute {
    pub id: String,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default, rename = "apiKeyEnv")]
    pub api_key_env: Option<String>,
    #[serde(default, rename = "baseURL")]
    pub base_url: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    /// Profile 路由里每个模型的完整非机密字段（name / contextWindow / maxTokens /
    /// input / reasoningEfforts / compat）。写回时原样保留，避免补丁把能力字段抹掉。
    #[serde(default, rename = "modelEntries")]
    pub model_entries: Vec<serde_json::Value>,
}

impl ProviderRoute {
    /// 缺 api 或 baseURL 的条目无法渲染成合法 provider 配置。
    fn renderable(&self) -> bool {
        self.api.as_deref().map(|a| !a.is_empty()).unwrap_or(false)
            && self.base_url.as_deref().map(|b| !b.is_empty()).unwrap_or(false)
    }
}

fn render_provider(p: &ProviderSettings, base: Option<&ProviderRoute>) -> String {
    let mut out = String::new();
    out.push_str(&format!("      {}:\n", p.provider));
    out.push_str(&format!("        displayName: {}\n", yaml_scalar(&p.display_name)));
    out.push_str(&format!("        api: {}\n", p.api));
    out.push_str(&format!("        apiKeyEnv: {}\n", p.api_key_ref));
    out.push_str(&format!("        baseURL: {}\n", yaml_scalar(&p.base_url)));
    out.push_str("        models:\n");
    for m in &p.models {
        // 同名提供商会整体覆盖 Profile 里的路由。Profile 中该模型声明的能力字段
        // （思考档位映射、模态、输出上限、兼容项）必须带过来，否则补丁会静默抹掉
        // 它们——表现为「界面能选档位，但不发出去」。
        let inherited = base
            .and_then(|route| {
                route
                    .model_entries
                    .iter()
                    .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(m.id.as_str()))
            })
            .cloned();
        let mut entry = match inherited {
            Some(mut entry) => {
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert("id".to_string(), serde_json::Value::String(m.id.clone()));
                    if !m.name.is_empty() {
                        obj.insert(
                            "name".to_string(),
                            serde_json::Value::String(m.name.clone()),
                        );
                    }
                    if let Some(window) = m.context_window {
                        obj.insert("contextWindow".to_string(), serde_json::json!(window));
                    }
                }
                entry
            }
            None => {
                let mut entry = serde_json::json!({ "id": m.id, "name": m.label() });
                if let Some(window) = m.context_window {
                    entry["contextWindow"] = serde_json::json!(window);
                }
                entry
            }
        };
        // OD-ARCH-181：用户在设置页逐模型手动声明的档位优先——覆盖 Profile 继承，
        // 也覆盖宿主已验证清单（merge_verified_efforts 只补缺失键）。
        if let Some(efforts) = &m.reasoning_efforts {
            if !efforts.is_empty() {
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert(
                        "reasoningEfforts".to_string(),
                        serde_json::to_value(efforts)
                            .unwrap_or_else(|_| serde_json::json!({})),
                    );
                }
            }
        }
        if p.thinking_mode == "disabled" {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("reasoning".to_string(), serde_json::json!(true));
                let compat = obj
                    .entry("compat")
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(map) = compat.as_object_mut() {
                    map.insert(
                        "thinkingFormat".to_string(),
                        serde_json::json!("deepseek"),
                    );
                }
            }
        }
        out.push_str(&render_model_entry(&p.provider, &entry, &m.id));
    }
    out
}

/// 标识符：字母/数字/下划线/短横线，首字符必须是字母或下划线。
fn valid_ident(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 端点只接受 http(s) 绝对地址：必须是 127.0.0.1 / localhost / 普通域名。
/// 拒绝把 key 写进 URL（user:pass@host）以及查询串里夹带密钥。
pub fn validate_base_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed != url {
        return Err("端点地址首尾不能有空格".to_string());
    }
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .ok_or_else(|| "端点地址必须以 http:// 或 https:// 开头".to_string())?;
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err("端点地址缺少主机名".to_string());
    }
    if authority.contains('@') {
        return Err("端点地址里不能带用户名/密码；密钥请填到「API Key」一栏".to_string());
    }
    if let Some(query) = rest.split('?').nth(1) {
        let low = query.to_ascii_lowercase();
        for needle in ["key=", "token=", "secret=", "password="] {
            if low.contains(needle) {
                return Err("端点地址的查询串里疑似带密钥；请把密钥填到「API Key」一栏".to_string());
            }
        }
    }
    Ok(())
}

/// 单行 YAML 标量：可裸写就裸写，否则用双引号并按 YAML 规则转义。
fn yaml_scalar(value: &str) -> String {
    // YAML 1.1 把 off/on/yes/no 当作布尔，把 null/~ 当作空值。思考档位里 `off` 是
    // 键名、端点取值里有 `none`；不识别这些词会让映射键变成布尔 false、值变成 null。
    // 统一加引号，保证 DSH 读到的是字符串而不是被 YAML 改写的类型。
    let reserved = matches!(
        value.to_ascii_lowercase().as_str(),
        "off" | "on" | "yes" | "no" | "true" | "false" | "null" | "~" | "none"
    );
    let plain = !value.is_empty()
        && !reserved
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':' | '@' | '+' | '~'))
        && !value.starts_with(['-', ':', '?', '@', '&', '*', '!', '%', '|', '>', '{', '['])
        && !value.ends_with(':');
    if plain {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(MODEL_SETTINGS_FILE)
}

pub fn legacy_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(LEGACY_SETTINGS_FILE)
}

pub fn overlay_path(config_dir: &Path) -> PathBuf {
    config_dir.join("omnidesk-model.yml")
}

/// 旧版单路由配置（只用于迁移）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySettings {
    schema: String,
    provider: String,
    display_name: String,
    base_url: String,
    api: String,
    default_model: String,
    api_key_ref: String,
    #[serde(default)]
    reasoning_effort: Option<String>,
}

impl LegacySettings {
    fn into_v2(self) -> Result<ModelSettings, String> {
        if self.schema != LEGACY_SETTINGS_SCHEMA {
            return Err(format!(
                "旧配置版本不受支持：{}（期望 {}）",
                self.schema, LEGACY_SETTINGS_SCHEMA
            ));
        }
        Ok(ModelSettings {
            schema: MODEL_SETTINGS_SCHEMA.to_string(),
            providers: vec![ProviderSettings {
                provider: self.provider.clone(),
                display_name: self.display_name,
                base_url: self.base_url,
                api: self.api,
                api_key_ref: self.api_key_ref,
                models: vec![ModelEntry {
                    id: self.default_model.clone(),
                    name: self.default_model.clone(),
                    context_window: None,
                    reasoning_efforts: None,
                }],
                reasoning_effort: self.reasoning_effort,
                thinking_mode: "auto".to_string(),
            }],
            default: DefaultRoute {
                provider: self.provider,
                model: self.default_model,
            },
            task_timeout_seconds: DEFAULT_TASK_TIMEOUT_SECONDS,
            search_task_timeout_seconds: DEFAULT_SEARCH_TASK_TIMEOUT_SECONDS,
        })
    }
}

/// 读取配置。
///   * v2 不存在但 v1 存在 → 迁移（先写 v2，成功后才把 v1 改名为 .bak）；
///   * 都不存在 → Ok(None)；
///   * 损坏 / 版本不符 → Err（**不**静默重置，界面只读）。
pub fn load(config_dir: &Path) -> Result<Option<ModelSettings>, String> {
    let path = settings_path(config_dir);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return migrate_if_needed(config_dir);
        }
        Err(e) => return Err(format!("无法读取模型配置（{}）：{}", path.display(), e)),
    };
    let parsed: ModelSettings = serde_json::from_str(&text).map_err(|e| {
        format!(
            "模型配置无法解析（{}）：{}。文件未被改动，本页只读显示；重新保存会覆盖它。",
            path.display(),
            e
        )
    })?;
    parsed.validate().map_err(|e| {
        format!(
            "模型配置内容无效（{}）：{}。文件未被改动，本页只读显示。",
            path.display(),
            e
        )
    })?;
    Ok(Some(parsed))
}

fn migrate_if_needed(config_dir: &Path) -> Result<Option<ModelSettings>, String> {
    let legacy_path = legacy_settings_path(config_dir);
    let text = match fs::read_to_string(&legacy_path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(format!(
                "无法读取旧版模型配置（{}）：{}。文件未被改动。",
                legacy_path.display(),
                e
            ))
        }
    };
    let legacy: LegacySettings = serde_json::from_str(&text).map_err(|e| {
        format!(
            "旧版模型配置无法解析（{}）：{}。文件未被改动，本页只读显示。",
            legacy_path.display(),
            e
        )
    })?;
    let upgraded = legacy.into_v2()?;
    upgraded.validate().map_err(|e| {
        format!(
            "旧版模型配置内容无效（{}）：{}。文件未被改动。",
            legacy_path.display(),
            e
        )
    })?;
    let bytes = serde_json::to_vec_pretty(&upgraded)
        .map_err(|e| format!("序列化升级后的模型配置失败：{}", e))?;
    // 只有 v2 真正落盘后才动 v1，避免「迁移一半把原文件弄没」。
    write_atomic(&settings_path(config_dir), &bytes)?;
    let mut backup = legacy_path.with_file_name("omnidesk.model-settings.v1.bak.json");
    if backup.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        backup = legacy_path.with_file_name(format!("omnidesk.model-settings.v1.bak-{}.json", stamp));
    }
    if let Err(e) = fs::rename(&legacy_path, &backup) {
        // v2 已经写好，旧文件留着也不影响读取；只报告备份未完成。
        eprintln!(
            "旧版模型配置已升级，但改名备份失败（{}）：{}",
            legacy_path.display(),
            e
        );
    }
    Ok(Some(upgraded))
}

/// 原子写：同目录临时文件（0600）→ fsync → rename。
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建配置目录失败（{}）：{}", parent.display(), e))?;
    }
    let tmp = path.with_file_name(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("cfg"),
        std::process::id()
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|e| format!("创建临时配置文件失败（{}）：{}", tmp.display(), e))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("写入临时配置文件失败（{}）：{}", tmp.display(), e)
        })?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("替换配置文件失败（{}）：{}", path.display(), e)
    })
}

/// 保存配置：写 JSON 与补丁文件，两者都是原子替换。
///
/// base 必须是引擎 Profile 当前生效的路由；合并失败时调用方**不应**调用本函数
/// （否则浅合并会抹掉 Profile 自带的路由）。
pub fn save(
    config_dir: &Path,
    settings: &ModelSettings,
    base: &[ProviderRoute],
) -> Result<(PathBuf, PathBuf), String> {
    settings.validate()?;
    check_effort_capability(settings, base)?;
    let overlay_text = settings.render_overlay(base);
    let json_path = settings_path(config_dir);
    let text = serde_json::to_vec_pretty(settings)
        .map_err(|e| format!("序列化模型配置失败：{}", e))?;
    write_atomic(&json_path, &text)?;
    let overlay = overlay_path(config_dir);
    write_atomic(&overlay, overlay_text.as_bytes())?;
    Ok((json_path, overlay))
}

/// Profile 为某个模型显式声明的思考档位（`reasoningEfforts` 的键）。
///
/// 返回值的三种含义必须分开用：
///   * `None`：Profile 没有声明，DSH 可能回退到内置目录；宿主不知道能力，不能据此
///     阻止保存，也不能假装它一定支持；
///   * `Some(空)`：声明 `reasoningEfforts: false`，该模型不支持思考；任何显式档位
///     都会在首次模型调用时被 DSH 拒绝；
///   * `Some(非空)`：声明了这些档位，显式档位必须落在集合内。
///
/// 只读 Profile 路由的非机密元数据；不接触凭据。
pub fn declared_efforts(
    routes: &[ProviderRoute],
    provider: &str,
    model: &str,
) -> Option<Vec<String>> {
    let route = routes.iter().find(|r| r.id == provider)?;
    let entry = route
        .model_entries
        .iter()
        .find(|entry| entry.get("id").and_then(|v| v.as_str()) == Some(model))?;
    match entry.get("reasoningEfforts") {
        None => None,
        Some(serde_json::Value::Bool(false)) => Some(Vec::new()),
        Some(serde_json::Value::Object(map)) => Some(map.keys().cloned().collect()),
        // null 或其它形状：DSH 的配置校验会直接拒绝，宿主不替它猜测能力。
        Some(_) => None,
    }
}

/// 保存前的思考档位预检（只查 Profile 明确声明的能力）。
///
/// 为什么只在“已声明”时阻断：Profile 未声明 `reasoningEfforts` 时，DSH 会回退到
/// pi-ai 内置目录；宿主没有同口径的目录读取接口，猜“不支持”会误伤内置 provider。
/// 未声明但实际不支持的端点，由设置页用配置转储的 `modelEntries` 与连接预检的
/// 内置目录标志补查（切换路径读 `engine_list_models` 的 `efforts`），或由运行期的
/// `UNSUPPORTED_REASONING_EFFORT` 如实报错。
pub fn check_effort_capability(
    settings: &ModelSettings,
    base: &[ProviderRoute],
) -> Result<(), String> {
    let provider = settings
        .find(&settings.default.provider)
        .ok_or_else(|| format!("默认路由指向不存在的提供商：{}", settings.default.provider))?;
    // 思考模式关闭时 render_overlay 不写 reasoningEffort；不要拦一个不会下发的档位。
    if provider.thinking_mode == "disabled" {
        return Ok(());
    }
    let effort = match provider
        .reasoning_effort
        .as_deref()
        .filter(|e| !e.is_empty() && *e != "default")
    {
        Some(effort) => effort,
        None => return Ok(()),
    };
    let Some(levels) = declared_efforts(base, &settings.default.provider, &settings.default.model)
    else {
        return Ok(());
    };
    if levels.is_empty() {
        return Err(format!(
            "{} / {}：Profile 声明该模型不支持思考档位（reasoningEfforts: false），当前选了「{}」；请把思考等级改为「跟随模型默认」后保存（显式档位会在首次模型调用时被 DSH 拒绝）。",
            settings.default.provider, settings.default.model, effort
        ));
    }
    if !levels.iter().any(|level| level == effort) {
        return Err(format!(
            "{} / {}：Profile 声明的可用思考档位只有 {}，没有「{}」；请改选其中一个，或把思考等级改为「跟随模型默认」后保存（显式档位会在首次模型调用时被 DSH 拒绝）。",
            settings.default.provider,
            settings.default.model,
            levels.join(" / "),
            effort
        ));
    }
    Ok(())
}

/// 日志/界面里只允许出现配置的**非机密摘要**。
pub fn summary(settings: &ModelSettings) -> String {
    let providers = settings
        .providers
        .iter()
        .map(|p| {
            let models: Vec<&str> = p.models.iter().map(|m| m.id.as_str()).collect();
            format!(
                "{}（{}，{}，模型 {}，凭据引用 {}）",
                p.provider,
                p.base_url,
                p.api,
                models.join(" / "),
                p.api_key_ref
            )
        })
        .collect::<Vec<_>>()
        .join("；");
    format!(
        "{}；默认 {}/{}",
        providers, settings.default.provider, settings.default.model
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(name: &str) -> ProviderSettings {
        ProviderSettings {
            provider: name.to_string(),
            display_name: name.to_string(),
            base_url: format!("https://{}.example.com/v1", name),
            api: "openai-completions".to_string(),
            api_key_ref: format!("{}_API_KEY", name.to_ascii_uppercase()),
            models: vec![ModelEntry {
                id: format!("{}-flash", name),
                name: format!("{}-flash", name),
                context_window: None,
                reasoning_efforts: None,
            }],
            reasoning_effort: None,
            thinking_mode: "auto".to_string(),
        }
    }

    fn sample() -> ModelSettings {
        let mut s = ModelSettings::new();
        let mut p = provider("agnescn");
        p.reasoning_effort = Some("low".to_string());
        s.providers.push(p);
        s.default = DefaultRoute {
            provider: "agnescn".to_string(),
            model: "agnescn-flash".to_string(),
        };
        s
    }

    fn base_routes() -> Vec<ProviderRoute> {
        vec![ProviderRoute {
            id: "agnescn".to_string(),
            display_name: Some("Agnes CN".to_string()),
            api: Some("openai-completions".to_string()),
            api_key_env: Some("AGNESCN_API_KEY".to_string()),
            base_url: Some("https://api.agnes-ai.cn/v1".to_string()),
            models: vec!["agnes-3.0-flash".to_string()],
            model_entries: vec![],
        }]
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("omni-model-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn overlay_contains_only_non_secret_fields() {
        let yml = sample().render_overlay(&[]);
        assert!(yml.contains("apiKeyEnv: AGNESCN_API_KEY"));
        assert!(yml.contains("baseURL: https://agnescn.example.com/v1"));
        assert!(yml.contains("reasoningEffort: low"));
        // OD-ARCH-153：agent-default-model 也必须带思考等级（web-api 通道读它）。
        let agent_block = yml
            .split("- id: agent-default-model")
            .nth(1)
            .expect("agent-default-model 块必须存在");
        assert!(agent_block.contains("reasoningEffort: low"), "{}", agent_block);
        assert!(!yml.to_ascii_lowercase().contains("api_key:"));
        assert!(!yml.to_ascii_lowercase().contains("apikey:"));
        assert!(!yml.to_ascii_lowercase().contains("secret"));
    }

    #[test]
    fn overlay_omits_effort_when_default_or_unset() {
        // effort 为 default 时不写 reasoningEffort（provider 默认档），两个块都不写。
        let mut s = sample();
        s.providers[0].reasoning_effort = Some("default".to_string());
        let yml = s.render_overlay(&[]);
        assert!(!yml.contains("reasoningEffort"), "{}", yml);
        s.providers[0].reasoning_effort = None;
        assert!(!s.render_overlay(&[]).contains("reasoningEffort"));
    }

    #[test]
    fn overlay_quotes_odd_model_names() {
        let mut s = sample();
        s.providers[0].models[0].id = "vendor/model:beta".to_string();
        s.default.model = "vendor/model:beta".to_string();
        let yml = s.render_overlay(&[]);
        assert!(yml.contains("vendor/model:beta"), "{}", yml);
        s.providers[0].models[0].id = "bad model".to_string();
        s.default.model = "bad model".to_string();
        assert!(s.render_overlay(&[]).contains("\"bad model\""));
    }

    #[test]
    fn multi_route_merge_keeps_profile_route() {
        let mut s = sample();
        let mut own = provider("opendesign");
        own.models.push(ModelEntry {
            id: "glm-5.3-flash".to_string(),
            name: "GLM 5.3 Flash".to_string(),
            context_window: Some(1_048_576),
            reasoning_efforts: None,
        });
        s.providers.push(own);
        s.default = DefaultRoute {
            provider: "opendesign".to_string(),
            model: "glm-5.3-flash".to_string(),
        };
        // 用一条不与 OmniDesk 重名的 Profile 路由验证「保留」；同名覆盖见下一条用例。
        let mut routes = base_routes();
        routes[0].id = "agnescn-profile".to_string();
        let yml = s.render_overlay(&routes);
        assert!(yml.contains("      opendesign:"), "{}", yml);
        assert!(yml.contains("      agnescn-profile:"), "{}", yml);
        assert!(yml.contains("https://api.agnes-ai.cn/v1"), "{}", yml);
        assert!(yml.contains("    provider: opendesign"), "{}", yml);
        let agnes = yml.find("agnescn-profile:").unwrap();
        let open = yml.find("opendesign:").unwrap();
        assert!(agnes < open);
    }

    #[test]
    fn same_name_overrides_profile_route() {
        let mut s = sample();
        s.providers[0].base_url = "https://agnes.example.com/v2".to_string();
        let yml = s.render_overlay(&base_routes());
        assert!(yml.contains("https://agnes.example.com/v2"));
        assert!(!yml.contains("https://api.agnes-ai.cn/v1"));
        assert_eq!(yml.matches("agnescn:\n").count(), 1);
    }

    #[test]
    fn profile_routes_without_endpoint_are_skipped() {
        let mut routes = base_routes();
        routes.push(ProviderRoute {
            id: "broken".to_string(),
            display_name: None,
            api: None,
            api_key_env: None,
            base_url: None,
            models: vec![],
            model_entries: vec![],
        });
        let yml = sample().render_overlay(&routes);
        assert!(!yml.contains("broken:"));
    }

    #[test]
    fn rejects_shared_credential_reference() {
        let mut s = sample();
        let mut second = provider("opendesign");
        second.api_key_ref = s.providers[0].api_key_ref.clone();
        s.providers.push(second);
        let err = s.validate().unwrap_err();
        assert!(err.contains("被两个提供商共用"), "{}", err);
    }

    #[test]
    fn rejects_empty_providers_and_unknown_default() {
        let s = ModelSettings::new();
        assert!(s.validate().is_err());
        let mut good = sample();
        good.default.provider = "nope".to_string();
        assert!(good.validate().unwrap_err().contains("不存在的提供商"));
        let mut good = sample();
        good.default.model = "nope".to_string();
        assert!(good.validate().unwrap_err().contains("不在提供商"));
        let mut good = sample();
        good.providers[0].models.clear();
        assert!(good.validate().is_err());
    }

    #[test]
    fn save_then_load_roundtrip_and_file_mode() {
        let dir = temp_dir("roundtrip");
        let (json_path, overlay) = save(&dir, &sample(), &[]).unwrap();
        assert!(json_path.is_file() && overlay.is_file());
        assert_eq!(load(&dir).unwrap().unwrap(), sample());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&json_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "配置文件必须是 0600");
            let mode = fs::metadata(&overlay).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "覆盖文件必须是 0600");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    fn routes_with_efforts(efforts: serde_json::Value) -> Vec<ProviderRoute> {
        let mut routes = base_routes();
        routes[0].models = vec!["agnescn-flash".to_string()];
        routes[0].model_entries = vec![serde_json::json!({
            "id": "agnescn-flash",
            "reasoningEfforts": efforts,
        })];
        routes
    }

    #[test]
    fn declared_efforts_distinguishes_absent_false_and_levels() {
        let routes = base_routes();
        assert_eq!(declared_efforts(&routes, "agnescn", "agnescn-flash"), None);

        let routes = routes_with_efforts(serde_json::Value::Bool(false));
        assert_eq!(
            declared_efforts(&routes, "agnescn", "agnescn-flash"),
            Some(Vec::new())
        );

        let routes = routes_with_efforts(serde_json::json!({ "off": null, "high": "high" }));
        let mut levels = declared_efforts(&routes, "agnescn", "agnescn-flash").unwrap();
        levels.sort();
        assert_eq!(levels, vec!["high".to_string(), "off".to_string()]);
    }

    #[test]
    fn save_rejects_effort_the_profile_does_not_list() {
        let dir = temp_dir("effort-block");
        let routes = routes_with_efforts(serde_json::json!({ "off": null, "high": "high" }));
        let err = save(&dir, &sample(), &routes).unwrap_err();
        assert!(err.contains("没有「low」"), "{}", err);
        assert!(err.contains("off") && err.contains("high"), "{}", err);
        assert!(!settings_path(&dir).exists(), "被拒绝的保存不得写盘");
        assert!(!overlay_path(&dir).exists(), "被拒绝的保存不得写覆盖文件");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_allows_effort_the_profile_lists() {
        let dir = temp_dir("effort-ok");
        let routes = routes_with_efforts(serde_json::json!({ "off": null, "high": "high" }));
        let mut s = sample();
        s.providers[0].reasoning_effort = Some("high".to_string());
        let (json_path, overlay) = save(&dir, &s, &routes).unwrap();
        assert!(json_path.is_file() && overlay.is_file());
        assert_eq!(
            load(&dir).unwrap().unwrap().providers[0]
                .reasoning_effort
                .as_deref(),
            Some("high")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_rejects_effort_when_profile_declares_no_reasoning() {
        let dir = temp_dir("effort-false");
        let routes = routes_with_efforts(serde_json::Value::Bool(false));
        let err = save(&dir, &sample(), &routes).unwrap_err();
        assert!(err.contains("不支持思考档位"), "{}", err);
        assert!(!settings_path(&dir).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_skips_effort_check_when_profile_is_silent_or_default_is_selected() {
        // 未声明：宿主不知道内置目录能力，不能猜“不支持”。
        let dir = temp_dir("effort-unknown");
        let routes = base_routes();
        save(&dir, &sample(), &routes).unwrap();
        let _ = fs::remove_dir_all(&dir);

        // 选择「跟随模型默认」时不发显式档位，声明 false 也不阻断。
        let dir = temp_dir("effort-default");
        let routes = routes_with_efforts(serde_json::Value::Bool(false));
        let mut s = sample();
        s.providers[0].reasoning_effort = Some("default".to_string());
        save(&dir, &s, &routes).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_skips_effort_check_when_thinking_is_disabled() {
        // thinking_mode=disabled 时覆盖文件根本不写 reasoningEffort；档位声明不该误拦。
        let dir = temp_dir("effort-disabled");
        let routes = routes_with_efforts(serde_json::Value::Bool(false));
        let mut s = sample();
        s.providers[0].thinking_mode = "disabled".to_string();
        save(&dir, &s, &routes).unwrap();
        let yml = fs::read_to_string(overlay_path(&dir)).unwrap();
        assert!(!yml.contains("reasoningEffort:"), "{}", yml);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn v1_config_is_migrated_and_backed_up() {
        let dir = temp_dir("migrate");
        let legacy = serde_json::json!({
            "schema": LEGACY_SETTINGS_SCHEMA,
            "provider": "opendesign",
            "display_name": "OpenDesign",
            "base_url": "https://amr-link.open-design.ai/v1",
            "api": "openai-completions",
            "default_model": "glm-5.3-flash",
            "api_key_ref": "opendesign_Key",
            "reasoning_effort": "medium"
        });
        fs::write(legacy_settings_path(&dir), serde_json::to_vec(&legacy).unwrap()).unwrap();
        let loaded = load(&dir).unwrap().unwrap();
        assert_eq!(loaded.schema, MODEL_SETTINGS_SCHEMA);
        assert_eq!(loaded.providers.len(), 1);
        assert_eq!(loaded.providers[0].api_key_ref, "opendesign_Key");
        assert_eq!(loaded.providers[0].reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(loaded.default.model, "glm-5.3-flash");
        assert!(settings_path(&dir).is_file());
        assert!(!legacy_settings_path(&dir).exists(), "旧文件应改名为 .bak");
        assert!(dir.join("omnidesk.model-settings.v1.bak.json").is_file());
        assert_eq!(load(&dir).unwrap().unwrap(), loaded);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_v1_is_kept_untouched() {
        let dir = temp_dir("badmigrate");
        let path = legacy_settings_path(&dir);
        fs::write(&path, "{ not json").unwrap();
        let err = load(&dir).unwrap_err();
        assert!(err.contains("旧版模型配置无法解析"), "{}", err);
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
        assert!(!settings_path(&dir).exists(), "迁移失败不得写 v2");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_is_reported_not_reset() {
        let dir = temp_dir("corrupt");
        let path = settings_path(&dir);
        fs::write(&path, "{ not json").unwrap();
        let err = load(&dir).unwrap_err();
        assert!(err.contains("无法解析"), "{}", err);
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_means_unconfigured() {
        let dir = temp_dir("missing");
        assert!(load(&dir).unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_secrets_in_url_and_bad_idents() {
        assert!(validate_base_url("https://user:pw@api.example.com/v1").is_err());
        assert!(validate_base_url("https://api.example.com/v1?key=abc").is_err());
        assert!(validate_base_url("ftp://api.example.com").is_err());
        assert!(validate_base_url("https://api.example.com/v1").is_ok());
        let mut s = sample();
        s.providers[0].api_key_ref = "1BAD".to_string();
        assert!(s.validate().is_err());
        s.providers[0].api_key_ref = "GOOD_REF".to_string();
        s.providers[0].api = "grpc".to_string();
        assert!(s.validate().is_err());
    }

    #[test]
    fn summary_never_contains_key_material() {
        let text = summary(&sample());
        assert!(text.contains("AGNESCN_API_KEY"));
        assert!(!text.to_ascii_lowercase().contains("sk-"));
    }

    #[test]
    fn context_window_is_rendered_for_own_routes() {
        let mut s = sample();
        s.providers[0].models[0].context_window = Some(1_048_576);
        let yml = s.render_overlay(&[]);
        assert!(yml.contains("contextWindow: 1048576"), "{}", yml);
        // 未设置的模型不写这个键，避免替端点凭空声明能力。
        s.providers[0].models[0].context_window = None;
        assert!(!s.render_overlay(&[]).contains("contextWindow"));
    }

    fn opendesign_routes(entries: serde_json::Value) -> Vec<ProviderRoute> {
        let models = entries
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|e| e.get("id").and_then(|v| v.as_str()).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        vec![ProviderRoute {
            id: "opendesign".to_string(),
            display_name: Some("OpenDesign".to_string()),
            api: Some("openai-completions".to_string()),
            api_key_env: Some("OPENDESIGN_API_KEY".to_string()),
            base_url: Some("https://amr-link.open-design.ai/v1".to_string()),
            models,
            model_entries: entries.as_array().cloned().unwrap_or_default(),
        }]
    }

    fn opendesign_block(yml: &str) -> &str {
        yml.split("      opendesign:")
            .nth(1)
            .and_then(|block| block.split("      agnescn:").next())
            .expect("覆盖文件里应有 opendesign 路由")
    }

    #[test]
    fn overlay_adds_verified_efforts_when_profile_is_silent() {
        let routes = opendesign_routes(serde_json::json!([
            { "id": "deepseek-v4.1-flash", "name": "DeepSeek V4.1 Flash" },
        ]));
        let yml = sample().render_overlay(&routes);
        let block = opendesign_block(&yml);
        assert_eq!(block.matches("reasoningEfforts:").count(), 1, "{}", block);
        // YAML 1.1 里 off/none 必须加引号，否则会被解析成布尔与空值。
        assert!(block.contains("\"off\": \"none\""), "{}", block);
        assert!(block.contains("xhigh: xhigh"), "{}", block);
    }

    #[test]
    fn user_declared_efforts_override_profile_and_verified_list() {
        let mut s = sample();
        let mut own = provider("opendesign");
        own.models = vec![ModelEntry {
            id: "deepseek-v4.1-flash".to_string(),
            name: "DeepSeek V4.1 Flash".to_string(),
            context_window: None,
            reasoning_efforts: Some(
                [("high".to_string(), "custom-high".to_string())].into_iter().collect(),
            ),
        }];
        s.providers.push(own);
        let yml = s.render_overlay(&[]);
        let flash = yml
            .split("- id: deepseek-v4.1-flash")
            .nth(1)
            .and_then(|section| section.split("- id:").next())
            .expect("应有 deepseek-v4.1-flash");
        assert!(flash.contains("reasoningEfforts:"), "{}", flash);
        assert!(flash.contains("custom-high"), "用户取值必须原样写出：{}", flash);
        assert!(!flash.contains("none"), "用户声明应完整覆盖宿主清单：{}", flash);
    }

    #[test]
    fn overlay_keeps_profile_effort_declaration() {
        let routes = opendesign_routes(serde_json::json!([{
            "id": "deepseek-v4.1-flash",
            "name": "DeepSeek V4.1 Flash",
            "reasoningEfforts": { "off": "none", "high": "high" },
        }]));
        let yml = sample().render_overlay(&routes);
        let block = opendesign_block(&yml);
        assert_eq!(block.matches("reasoningEfforts:").count(), 1, "{}", block);
        assert!(block.contains("high: high"), "{}", block);
        assert!(!block.contains("max: max"), "不得追加清单里的 max：{}", block);
    }

    #[test]
    fn overlay_only_declares_models_on_the_verified_list() {
        let routes = opendesign_routes(serde_json::json!([
            { "id": "deepseek-v4.1-flash", "name": "DeepSeek V4.1 Flash" },
            { "id": "deepseek-v4.1-flash-eco", "name": "DeepSeek V4.1 Flash Eco" },
            { "id": "glm-5.3-flashx", "name": "GLM 5.3 FlashX" },
        ]));
        let yml = sample().render_overlay(&routes);
        let block = opendesign_block(&yml);
        assert_eq!(block.matches("reasoningEfforts:").count(), 2, "{}", block);
        let eco = block
            .split("- id: deepseek-v4.1-flash-eco")
            .nth(1)
            .and_then(|section| section.split("- id:").next())
            .expect("应有 eco 条目");
        assert!(eco.contains("reasoningEfforts"), "{}", eco);
        assert!(eco.contains("\"off\": \"none\""), "{}", eco);
        let glm = block
            .split("- id: glm-5.3-flashx")
            .nth(1)
            .expect("应有 glm 条目");
        assert!(!glm.contains("reasoningEfforts"), "{}", glm);
    }

    #[test]
    fn own_opendesign_route_also_gets_verified_efforts() {
        let mut s = sample();
        let mut own = provider("opendesign");
        own.models = vec![
            ModelEntry {
                id: "deepseek-v4.1-flash".to_string(),
                name: "DeepSeek V4.1 Flash".to_string(),
                context_window: None,
                reasoning_efforts: None,
            },
            ModelEntry {
                id: "deepseek-v4.1-flash-eco".to_string(),
                name: "DeepSeek V4.1 Flash Eco".to_string(),
                context_window: None,
                reasoning_efforts: None,
            },
        ];
        s.providers.push(own);
        let yml = s.render_overlay(&[]);
        let flash = yml
            .split("- id: deepseek-v4.1-flash")
            .nth(1)
            .and_then(|section| section.split("- id:").next())
            .expect("应有 deepseek-v4.1-flash");
        assert!(flash.contains("reasoningEfforts:"), "{}", flash);
        let eco = yml
            .split("- id: deepseek-v4.1-flash-eco")
            .nth(1)
            .expect("应有 eco");
        assert!(eco.contains("reasoningEfforts"), "{}", eco);
        assert!(eco.contains("\"off\": \"none\""), "{}", eco);
    }

    #[test]
    fn profile_model_capabilities_survive_merge() {
        let mut routes = base_routes();
        routes[0].model_entries = vec![serde_json::json!({
            "id": "agnes-3.0-flash",
            "name": "Agnes Flash",
            "contextWindow": 1000000,
            "maxTokens": 65536,
        })];
        // 同名 provider 会被 OmniDesk 覆盖，这里换成不重名的一条。
        routes[0].id = "agnescn-extra".to_string();
        let yml = sample().render_overlay(&routes);
        assert!(yml.contains("agnescn-extra:"), "{}", yml);
        assert!(yml.contains("contextWindow: 1000000"), "{}", yml);
        assert!(yml.contains("maxTokens: 65536"), "{}", yml);
        assert!(yml.contains("name: \"Agnes Flash\""), "{}", yml);
    }

    #[test]
    fn same_name_override_keeps_profile_model_capabilities() {
        // 同名 provider 由 OmniDesk 覆盖时，Profile 里该模型的能力字段（尤其
        // reasoningEfforts 映射）必须保留；否则补丁会把档位映射抹掉，
        // 表现为「界面能选思考等级，但参数没发给端点」。
        let mut s = sample();
        s.providers[0].models[0].context_window = Some(1_048_576);
        let mut routes = base_routes();
        routes[0].model_entries = vec![serde_json::json!({
            "id": "agnescn-flash",
            "name": "Agnes CN Flash",
            "maxTokens": 65536,
            "input": ["text", "image"],
            "reasoningEfforts": { "off": "none", "high": "high" },
        })];
        let yml = s.render_overlay(&routes);
        assert_eq!(yml.matches("agnescn:\n").count(), 1, "{}", yml);
        assert!(yml.contains("contextWindow: 1048576"), "{}", yml);
        assert!(yml.contains("maxTokens: 65536"), "{}", yml);
        assert!(yml.contains("reasoningEfforts:"), "{}", yml);
        assert!(yml.contains("image"), "{}", yml);
        // 界面填的显示名优先于 Profile 的旧名字。
        assert!(yml.contains("name: agnescn-flash"), "{}", yml);
    }

    #[test]
    fn thinking_disabled_writes_deepseek_compat_and_omits_effort() {
        let mut s = ModelSettings::new();
        let mut p = provider("volc");
        p.base_url = "https://ark.cn-beijing.volces.com/api/plan/v3".to_string();
        p.thinking_mode = "disabled".to_string();
        p.reasoning_effort = Some("high".to_string());
        s.providers.push(p);
        s.default = DefaultRoute {
            provider: "volc".to_string(),
            model: "volc-flash".to_string(),
        };
        let yml = s.render_overlay(&[]);
        assert!(yml.contains("thinkingFormat: deepseek"), "{}", yml);
        assert!(yml.contains("reasoning: true"), "{}", yml);
        assert!(!yml.contains("reasoningEffort"), "{}", yml);
    }
}



fn render_route(r: &ProviderRoute) -> String {
    let api = r.api.as_deref().unwrap_or("");
    let base = r.base_url.as_deref().unwrap_or("");
    let mut out = String::new();
    out.push_str(&format!("      {}:\n", r.id));
    out.push_str(&format!(
        "        displayName: {}\n",
        yaml_scalar(r.display_name.as_deref().unwrap_or(&r.id))
    ));
    out.push_str(&format!("        api: {}\n", api));
    if let Some(reference) = r.api_key_env.as_deref().filter(|v| !v.is_empty()) {
        out.push_str(&format!("        apiKeyEnv: {}\n", reference));
    }
    out.push_str(&format!("        baseURL: {}\n", yaml_scalar(base)));
    if r.models.is_empty() {
        out.push_str("        models: []\n");
    } else {
        out.push_str("        models:\n");
        for id in &r.models {
            // 优先用 Profile 里的完整模型条目，保住 contextWindow / maxTokens /
            // input / reasoningEfforts / compat；只有缺失时才退回 id。
            match r
                .model_entries
                .iter()
                .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
            {
                Some(entry) => out.push_str(&render_model_entry(&r.id, entry, id)),
                None => out.push_str(&format!(
                    "          - id: {}\n            name: {}\n",
                    yaml_scalar(id),
                    yaml_scalar(id)
                )),
            }
        }
    }
    out
}

/// 把 Profile 里的一个模型条目原样写成 YAML 序列项（只保留非机密字段白名单）。
///
/// OD-ARCH-178 阶段 A：Profile 没声明 `reasoningEfforts` 时，按宿主已验证清单补齐；
/// 已声明则保持 Profile 的原值，不做双写或覆盖。
fn render_model_entry(provider: &str, entry: &serde_json::Value, id: &str) -> String {
    const KEEP: &[&str] = &[
        "id",
        "name",
        "contextWindow",
        "maxTokens",
        "input",
        "reasoning",
        "reasoningEfforts",
        "compat",
    ];
    let mut entry = entry.clone();
    crate::model_routes::merge_verified_efforts(&mut entry, provider, id);
    let mut map = serde_json::Map::new();
    if let Some(obj) = entry.as_object() {
        for key in KEEP {
            if let Some(value) = obj.get(*key) {
                map.insert((*key).to_string(), value.clone());
            }
        }
    }
    if !map.contains_key("id") {
        map.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    }
    if !map.contains_key("name") {
        map.insert("name".to_string(), serde_json::Value::String(id.to_string()));
    }
    // 手写最小 YAML：这几个字段只可能是标量、字符串数组或一层浅字典。
    // 不新增生产依赖（serde_yaml 只是 dev-dependency）。
    let mut lines: Vec<String> = Vec::new();
    for (key, value) in map.iter() {
        push_yaml_field(&mut lines, key, value, 0);
    }
    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            out.push_str(&format!("          - {}\n", line));
        } else {
            out.push_str(&format!("            {}\n", line));
        }
    }
    out
}

fn push_yaml_field(lines: &mut Vec<String>, key: &str, value: &serde_json::Value, depth: usize) {
    let pad = "  ".repeat(depth);
    match value {
        serde_json::Value::Object(obj) if !obj.is_empty() => {
            lines.push(format!("{}{}:", pad, yaml_scalar(key)));
            for (k, v) in obj {
                push_yaml_field(lines, k, v, depth + 1);
            }
        }
        serde_json::Value::Array(items) if items.iter().all(|i| i.is_string()) => {
            if items.is_empty() {
                lines.push(format!("{}{}: []", pad, yaml_scalar(key)));
            } else {
                let joined = items
                    .iter()
                    .filter_map(|i| i.as_str())
                    .map(yaml_scalar)
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("{}{}: [{}]", pad, yaml_scalar(key), joined));
            }
        }
        serde_json::Value::String(s) => {
            lines.push(format!("{}{}: {}", pad, yaml_scalar(key), yaml_scalar(s)))
        }
        serde_json::Value::Bool(b) => lines.push(format!("{}{}: {}", pad, yaml_scalar(key), b)),
        serde_json::Value::Number(n) => {
            lines.push(format!("{}{}: {}", pad, yaml_scalar(key), n))
        }
        other => {
            // 其它形状（嵌套数组、null、空对象）不写出：宁可退回默认，也不猜测格式。
            let _ = other;
        }
    }
}

