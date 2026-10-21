//! 原生设置页的模型配置层（OD-ARCH-053 S1）。
//!
//! 事实基础（源码核实，见 STATUS 记录）：
//!   * 引擎实际路由来自 `~/OmniDesk/app/config/omnidesk-model.yml`（由 `scripts/model-routes.mjs`
//!     生成的 OmniDesk 自有补丁，宿主以 `--patch` 传给 DSH）；
//!   * DSH 的 provider 配置**只接受 `apiKeyEnv` 引用名，不接受明文 key**
//!     （`packages/llm/llm-pi-ai/src/index.ts::resolveApiKey`），密钥只能写进 DSH 凭据服务
//!     （`$DSH_HOME/.credentials.yaml`，经官方 `credentials/set` RPC）。
//!
//! 因此本模块只保存**非机密**字段，密钥值从不进入本文件、日志、返回值或 Git：
//!   * 配置文件：`<应用数据根>/config/omnidesk.model-settings.v1.json`；
//!   * 覆盖文件：`<应用数据根>/config/omnidesk-model.yml`（只有 baseURL / api / apiKeyEnv / 模型 id）。
//!
//! 文件损坏时**不静默重置**：读取返回错误，界面显示「配置损坏，只能只读查看」，
//! 用户在界面上重新保存才会覆盖。写入一律临时文件 + 原子替换，权限 0600。
//!
//! 证据等级：static（源码事实）+ script（模块内单测）。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MODEL_SETTINGS_SCHEMA: &str = "omnidesk.model-settings.v1";
pub const MODEL_SETTINGS_FILE: &str = "omnidesk.model-settings.v1.json";

/// DSH 支持的三种 api 协议（`packages/llm/llm-pi-ai/src/provider.ts`）。
pub const ALLOWED_APIS: &[&str] = &["openai-completions", "openai-responses", "anthropic-messages"];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSettings {
    pub schema: String,
    /// provider 标识（也是 DSH 配置里的键名）。
    pub provider: String,
    pub display_name: String,
    pub base_url: String,
    pub api: String,
    pub default_model: String,
    /// 凭据引用名（DSH 的 `apiKeyEnv`），例如 `AGNESCN_API_KEY`。
    pub api_key_ref: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
}

impl ModelSettings {
    pub fn new() -> Self {
        Self {
            schema: MODEL_SETTINGS_SCHEMA.to_string(),
            provider: String::new(),
            display_name: String::new(),
            base_url: String::new(),
            api: "openai-completions".to_string(),
            default_model: String::new(),
            api_key_ref: String::new(),
            reasoning_effort: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != MODEL_SETTINGS_SCHEMA {
            return Err(format!(
                "配置版本不受支持：{}（本版只认 {}）",
                self.schema, MODEL_SETTINGS_SCHEMA
            ));
        }
        if !valid_ident(&self.provider) {
            return Err("provider 名称只能用字母、数字、下划线、短横线，且以字母或下划线开头".to_string());
        }
        if self.display_name.trim().is_empty() {
            return Err("显示名称不能为空".to_string());
        }
        validate_base_url(&self.base_url)?;
        if !ALLOWED_APIS.contains(&self.api.as_str()) {
            return Err(format!(
                "api 协议只支持：{}（收到 {}）",
                ALLOWED_APIS.join(" / "),
                self.api
            ));
        }
        if self.default_model.trim().is_empty() || self.default_model.contains(['\r', '\n']) {
            return Err("默认模型不能为空，也不能含换行".to_string());
        }
        if !valid_ident(&self.api_key_ref) {
            return Err("凭据名只能用字母、数字、下划线，且以字母或下划线开头（例如 AGNESCN_API_KEY）".to_string());
        }
        if let Some(effort) = &self.reasoning_effort {
            const OK: &[&str] = &["default", "low", "medium", "high", "max"];
            if !OK.contains(&effort.as_str()) {
                return Err(format!("思考等级只支持：{}（收到 {}）", OK.join(" / "), effort));
            }
        }
        Ok(())
    }

    /// 生成 DSH 补丁层（只含非机密字段）。字段按固定顺序手写，避免引入 YAML 依赖。
    pub fn render_overlay(&self) -> String {
        let effort = self
            .reasoning_effort
            .as_deref()
            .filter(|e| !e.is_empty() && *e != "default");
        let mut out = String::new();
        out.push_str("- id: llm-pi-ai\n  name: \"@deepseek-ai/dsh-llm-pi-ai\"\n  config:\n    providers:\n");
        out.push_str(&format!("      {}:\n", self.provider));
        out.push_str(&format!("        displayName: {}\n", yaml_scalar(&self.display_name)));
        out.push_str(&format!("        api: {}\n", self.api));
        out.push_str(&format!("        apiKeyEnv: {}\n", self.api_key_ref));
        out.push_str(&format!("        baseURL: {}\n", yaml_scalar(&self.base_url)));
        out.push_str("        models:\n");
        out.push_str(&format!(
            "          - id: {}\n            name: {}\n",
            yaml_scalar(&self.default_model),
            yaml_scalar(&self.default_model)
        ));
        out.push_str("- id: acp\n  name: \"@deepseek-ai/dsh-acp\"\n  config:\n");
        out.push_str(&format!(
            "    provider: {}\n    model: {}\n",
            self.provider,
            yaml_scalar(&self.default_model)
        ));
        if let Some(effort) = effort {
            out.push_str(&format!("    reasoningEffort: {}\n", effort));
        }
        out.push_str("- id: agent-default-model\n  name: \"@deepseek-ai/dsh-agent-default-model\"\n  config:\n");
        out.push_str(&format!(
            "    provider: {}\n    model: {}\n",
            self.provider,
            yaml_scalar(&self.default_model)
        ));
        out
    }
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self::new()
    }
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
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':' | '@' | '+' | '~'))
        && !value.starts_with(['-', ':', '?', '@', '`', '&', '*', '!', '%', '|', '>', '{', '['])
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

pub fn overlay_path(config_dir: &Path) -> PathBuf {
    config_dir.join("omnidesk-model.yml")
}

/// 读取配置。
///   * 不存在 → `Ok(None)`（尚未配置，界面显示引导）；
///   * 损坏 / 版本不符 → `Err`（**不**静默重置，界面只读并显示原因）。
pub fn load(config_dir: &Path) -> Result<Option<ModelSettings>, String> {
    let path = settings_path(config_dir);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
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
/// 返回（配置文件, 覆盖文件）。
pub fn save(config_dir: &Path, settings: &ModelSettings) -> Result<(PathBuf, PathBuf), String> {
    settings.validate()?;
    let json_path = settings_path(config_dir);
    let text = serde_json::to_vec_pretty(settings)
        .map_err(|e| format!("序列化模型配置失败：{}", e))?;
    write_atomic(&json_path, &text)?;
    let overlay = overlay_path(config_dir);
    write_atomic(&overlay, settings.render_overlay().as_bytes())?;
    Ok((json_path, overlay))
}

/// 日志/界面里只允许出现配置的**非机密摘要**。
pub fn summary(settings: &ModelSettings) -> String {
    format!(
        "{}（{}，{}，模型 {}，凭据引用 {}）",
        settings.provider, settings.base_url, settings.api, settings.default_model, settings.api_key_ref
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ModelSettings {
        ModelSettings {
            schema: MODEL_SETTINGS_SCHEMA.to_string(),
            provider: "agnescn".to_string(),
            display_name: "Agnes CN".to_string(),
            base_url: "https://api.agnes-ai.cn/v1".to_string(),
            api: "openai-completions".to_string(),
            default_model: "agnes-3.0-flash".to_string(),
            api_key_ref: "AGNESCN_API_KEY".to_string(),
            reasoning_effort: Some("low".to_string()),
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("omni-model-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn overlay_contains_only_non_secret_fields() {
        let yml = sample().render_overlay();
        assert!(yml.contains("apiKeyEnv: AGNESCN_API_KEY"));
        assert!(yml.contains("baseURL: https://api.agnes-ai.cn/v1"));
        assert!(yml.contains("reasoningEffort: low"));
        assert!(!yml.to_ascii_lowercase().contains("api_key:"));
        assert!(!yml.to_ascii_lowercase().contains("apikey:"));
    }

    #[test]
    fn overlay_quotes_odd_model_names() {
        let mut s = sample();
        s.default_model = "vendor/model:beta".to_string();
        let yml = s.render_overlay();
        assert!(yml.contains("vendor/model:beta"), "{}", yml);
        s.default_model = "bad model".to_string();
        assert!(s.render_overlay().contains("\"bad model\""));
    }

    #[test]
    fn save_then_load_roundtrip_and_file_mode() {
        let dir = temp_dir("roundtrip");
        let (json_path, overlay) = save(&dir, &sample()).unwrap();
        assert!(json_path.is_file() && overlay.is_file());
        assert_eq!(load(&dir).unwrap().unwrap(), sample());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&json_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "配置文件必须是 0600");
        }
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
        s.api_key_ref = "1BAD".to_string();
        assert!(s.validate().is_err());
        s.api_key_ref = "GOOD_REF".to_string();
        s.api = "grpc".to_string();
        assert!(s.validate().is_err());
    }

    #[test]
    fn summary_never_contains_key_material() {
        let text = summary(&sample());
        assert!(text.contains("AGNESCN_API_KEY"));
        assert!(!text.to_ascii_lowercase().contains("sk-"));
    }
}
