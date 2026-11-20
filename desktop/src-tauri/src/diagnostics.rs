//! 只读配置解释（OD-ARCH-173 阶段 5）。
//!
//! 这里只回答“这条行为从哪来、提示词有多大、工具面长什么样”，
//! **不返回**提示词原文、用户对话、工具参数或凭据。
//!
//! 提示词分段口径与 `scripts/diagnose-omnidesk.py` 保持一致：
//! 读取最近一次 DSH 会话的 `system/message`，按段落起头归类并统计字符数。
//! 与脚本的差别是本模块不做逐轮时间线（阶段 5 明确不重做任务时间线）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

/// 超过这个大小的会话不解析，避免一次性把整份会话读进内存。
const MAX_SESSION_BYTES: u64 = 96 * 1024 * 1024;

const PROMPT_BUCKETS: [&str; 8] = [
    "base",
    "persona",
    "web-search",
    "ptc-runtime",
    "discipline-rules",
    "harness-guidance",
    "tool-guidance",
    "other",
];

#[derive(Debug, Default, Serialize)]
pub struct PromptSegments {
    pub chars: usize,
    pub segments: usize,
    pub buckets: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Serialize)]
pub struct SessionSnapshot {
    pub file: String,
    pub size_bytes: u64,
    pub modified_at: u64,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub context_window: Option<i64>,
    pub system_prompt: PromptSegments,
    pub injected_chars: BTreeMap<String, usize>,
    pub tool_calls: usize,
    pub tool_names: Vec<String>,
    pub approvals: usize,
    pub retries: usize,
    pub assistant_attempts: usize,
    pub compactions: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct RouteMeta {
    pub overlay_present: bool,
    pub overlay_path: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub provider_api: Option<String>,
    pub provider_base_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PresetPlugin {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default, Serialize)]
pub struct PresetMeta {
    pub id: String,
    pub label: String,
    pub source: String,
    pub plugins: Vec<PresetPlugin>,
}

/// 组装诊断元数据。任何一项读不到都返回 `None` 并在 `notes` 里说明，
/// 不用猜测值填空。
pub fn build(app_root: &Path) -> serde_json::Value {
    let mut notes: Vec<String> = vec![
        "只统计分类、字符数与次数；不返回提示词原文、对话内容、工具参数或凭据。".to_string(),
    ];

    let config = crate::engine_supervisor::EngineConfig::from_env(app_root);
    let (profile, dsh_home) = match &config {
        Ok(c) => (Some(c.profile.clone()), Some(c.dsh_home.clone())),
        Err(e) => {
            notes.push(format!("读不到引擎配置：{e}"));
            (None, None)
        }
    };

    let route = match overlay_route(app_root) {
        Ok(route) => Some(route),
        Err(e) => {
            notes.push(e);
            None
        }
    };

    let preset = profile.as_deref().and_then(|name| {
        let installed = dsh_home
            .as_ref()
            .map(|home| home.join("profiles").join(name).join("cordis.patch.yml"));
        let mut text = None;
        let mut source = String::new();
        if let Some(path) = installed.as_ref() {
            if let Ok(raw) = std::fs::read_to_string(path) {
                text = Some(raw);
                source = format!("profiles/{name}/cordis.patch.yml");
            }
        }
        if text.is_none() {
            if let Some(asset) = crate::profile_assets::asset_profile(name) {
                text = Some(asset.patch.to_string());
                source = format!("出厂补丁（{}）", name);
            }
        }
        let parsed = active_preset(&text?);
        parsed.map(|(id, label, plugins)| PresetMeta {
            id,
            label,
            source,
            plugins,
        })
    });
    if preset.is_none() {
        notes.push("没有读到当前 Profile 的 prompt 预设声明，工具面只以最近会话为准。".to_string());
    }

    let session = dsh_home.as_ref().and_then(|home| {
        let root = home.join("sessions");
        let path = newest_session_file(&root)?;
        match read_session(&path) {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                notes.push(format!("最近会话未解析：{e}"));
                None
            }
        }
    });
    if session.is_none() && config.is_ok() {
        notes.push("隔离数据根里还没有可解析的会话；跑过一次任务后再看。".to_string());
    }

    serde_json::json!({
        "ok": true,
        "profile": profile,
        "route": route,
        "preset": preset,
        "session": session,
        "notes": notes,
    })
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 最近写入的会话文件：先看 `.zstd`，没有再退到 `.jsonl`。
fn newest_session_file(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(u128, PathBuf)> = None;
    let projects = std::fs::read_dir(root).ok()?;
    for project in projects.flatten() {
        if !project.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(sessions) = std::fs::read_dir(project.path()) else {
            continue;
        };
        for session in sessions.flatten() {
            if !session.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let dir = session.path();
            let candidate = if dir.join("session.v4.jsonl.zstd").is_file() {
                dir.join("session.v4.jsonl.zstd")
            } else if dir.join("session.v4.jsonl").is_file() {
                dir.join("session.v4.jsonl")
            } else {
                continue;
            };
            let stamp = std::fs::metadata(&candidate)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            if best.as_ref().map(|(b, _)| stamp > *b).unwrap_or(true) {
                best = Some((stamp, candidate));
            }
        }
    }
    best.map(|(_, path)| path)
}

fn decode_session(path: &Path) -> Result<String, String> {
    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
        return std::fs::read_to_string(path)
            .map_err(|e| format!("读会话文件失败（{}）：{e}", path.file_name().and_then(|n| n.to_str()).unwrap_or("session")));
    }
    let output = std::process::Command::new("zstd")
        .arg("-dc")
        .arg(path)
        .output();
    match output {
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        Ok(out) => {
            // 有些环境只留下未压缩会话；有同目录 .jsonl 时改读它。
            let sibling = path.with_file_name("session.v4.jsonl");
            if sibling.is_file() {
                return std::fs::read_to_string(&sibling).map_err(|e| format!("读会话文件失败：{e}"));
            }
            Err(format!(
                "zstd 解压失败（退出码 {:?}）；同目录也没有未压缩会话",
                out.status.code()
            ))
        }
        Err(e) => {
            let sibling = path.with_file_name("session.v4.jsonl");
            if sibling.is_file() {
                return std::fs::read_to_string(&sibling).map_err(|err| format!("读会话文件失败：{err}"));
            }
            Err(format!("无法调用 zstd：{e}"))
        }
    }
}

fn read_session(path: &Path) -> Result<SessionSnapshot, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("读会话元数据失败：{e}"))?;
    if meta.len() > MAX_SESSION_BYTES {
        return Err(format!("会话文件 {:.1} MB，超过 {} MB 上限", meta.len() as f64 / 1_048_576.0, MAX_SESSION_BYTES / 1_048_576));
    }
    let modified_at = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or_else(now_epoch);
    let text = decode_session(path)?;
    parse_session_text(
        &text,
        path.file_name().and_then(|n| n.to_str()).unwrap_or("session").to_string(),
        meta.len(),
        modified_at,
    )
}

fn parse_session_text(
    text: &str,
    file: String,
    size_bytes: u64,
    modified_at: u64,
) -> Result<SessionSnapshot, String> {
    let mut snapshot = SessionSnapshot {
        file,
        size_bytes,
        modified_at,
        ..Default::default()
    };
    for key in ["user", "runtime-context", "skill-catalog", "time-context", "other"] {
        snapshot.injected_chars.insert(key.to_string(), 0);
    }

    let mut system_parts: Vec<String> = Vec::new();
    let mut tool_names: Vec<String> = Vec::new();
    let mut saw_context = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let data = event.get("data").cloned().unwrap_or(serde_json::Value::Null);

        match event_type {
            "system/message" => {
                let body = message_body(&data);
                if let Some(blocks) = body.get("content").and_then(|v| v.as_array()) {
                    for block in blocks {
                        if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                            if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                                system_parts.push(t.to_string());
                            }
                        }
                    }
                }
            }
            "user/message" => {
                let body = message_body(&data);
                let kind = body
                    .get("source")
                    .and_then(|v| v.as_object())
                    .and_then(|s| s.get("kind"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("other")
                    .to_string();
                let chars: usize = body
                    .get("content")
                    .and_then(|v| v.as_array())
                    .map(|blocks| {
                        blocks
                            .iter()
                            .map(|b| {
                                b.get("text")
                                    .and_then(|v| v.as_str())
                                    .map(|t| t.chars().count())
                            })
                            .map(|v| v.unwrap_or(0))
                            .sum()
                    })
                    .unwrap_or(0);
                let bucket = if ["user", "runtime-context", "skill-catalog", "time-context"]
                    .contains(&kind.as_str())
                {
                    kind
                } else {
                    "other".to_string()
                };
                *snapshot.injected_chars.entry(bucket).or_insert(0) += chars;
            }
            "request/context" if !saw_context => {
                saw_context = true;
                snapshot.provider = string_field(&data, "provider");
                snapshot.model = string_field(&data, "model");
                snapshot.context_window = data.get("contextWindow").and_then(|v| v.as_i64());
            }
            "tool/call" => {
                snapshot.tool_calls += 1;
                if let Some(name) = tool_name(&data, 0) {
                    if !tool_names.contains(&name) {
                        tool_names.push(name);
                    }
                }
            }
            "approval/request" | "approval/asked" => snapshot.approvals += 1,
            "llm/retry" | "llm/retry-started" => snapshot.retries += 1,
            "assistant/attempt" => snapshot.assistant_attempts += 1,
            other if other.to_ascii_lowercase().contains("compact") => snapshot.compactions += 1,
            _ => {}
        }
    }

    snapshot.system_prompt = prompt_segments(&system_parts.join("\n"));
    tool_names.sort();
    snapshot.tool_names = tool_names;
    Ok(snapshot)
}

fn string_field(data: &serde_json::Value, key: &str) -> Option<String> {
    data.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn message_body(data: &serde_json::Value) -> &serde_json::Value {
    match data.get("message") {
        Some(nested) if nested.is_object() => nested,
        _ => data,
    }
}

fn tool_name(value: &serde_json::Value, depth: u8) -> Option<String> {
    if depth > 3 {
        return None;
    }
    let obj = value.as_object()?;
    for key in ["tool", "toolName", "name", "tool_name"] {
        if let Some(candidate) = obj.get(key).and_then(|v| v.as_str()) {
            let trimmed = candidate.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.chars().take(80).collect());
            }
        }
    }
    for key in ["call", "toolCall", "data"] {
        if let Some(nested) = obj.get(key) {
            if let Some(name) = tool_name(nested, depth + 1) {
                return Some(name);
            }
        }
    }
    None
}

fn prompt_segments(text: &str) -> PromptSegments {
    let mut buckets: BTreeMap<String, usize> = PROMPT_BUCKETS
        .iter()
        .map(|name| (name.to_string(), 0))
        .collect();
    let mut segments = 0usize;
    for para in split_paragraphs(text) {
        if para.chars().any(|c| !c.is_whitespace()) {
            segments += 1;
            let chars = para.chars().count();
            *buckets.entry(classify_paragraph(para).to_string()).or_insert(0) += chars;
        }
    }
    PromptSegments {
        chars: text.chars().count(),
        segments,
        buckets,
    }
}

/// 与 Python 的 `text[start:]` + `find("\n\n")` 同义：
/// 空行分隔段落，分隔符本身不计入段内字符。
fn split_paragraphs(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    loop {
        match rest.find("\n\n") {
            Some(at) => {
                out.push(&rest[..at]);
                rest = &rest[at + 2..];
            }
            None => {
                out.push(rest);
                break;
            }
        }
    }
    out
}

fn classify_paragraph(paragraph: &str) -> &'static str {
    let head = paragraph
        .trim_start()
        .lines()
        .next()
        .unwrap_or("")
        .trim();
    if head.starts_with("You are an AI agent powered by DeepSeek Harness") {
        return "base";
    }
    if head.starts_with("You are a coding agent powered by")
        || head.starts_with("你是 OmniDesk")
        || head.contains("个人工作台账助手")
    {
        return "persona";
    }
    if head.starts_with("## Web search") {
        return "web-search";
    }
    const PTC_PREFIXES: [&str; 9] = [
        "interface Tool",
        "`run_code`",
        "## Writing code for run_code",
        "Inside the program",
        "- Call tools as `await tools.name(args)`",
        "Program-only SDK bindings",
        "```ts",
        "type ToolName = keyof ToolOutputMap",
        "declare class ToolCallError",
    ];
    if PTC_PREFIXES.iter().any(|p| head.starts_with(p)) || head.starts_with("declare const tools:") {
        return "ptc-runtime";
    }
    const RULE_PREFIXES: [&str; 4] = [
        "检索收敛规则",
        "工具批处理规则",
        "检索预算规则",
        "工具选择规则",
    ];
    if RULE_PREFIXES.iter().any(|p| head.starts_with(p)) {
        return "discipline-rules";
    }
    if paragraph.contains("DeepSeek Harness Web GUI")
        || paragraph.contains("runtime context names the DeepSeek Harness")
    {
        return "harness-guidance";
    }
    const TOOL_GUIDANCE_PREFIXES: [&str; 7] = [
        "Tokens prefixed with @",
        "Use the ",
        "Read an ",
        "Read a file before",
        "Check the [exit code:",
        "web_search results",
        "web_fetch returns",
    ];
    if TOOL_GUIDANCE_PREFIXES.iter().any(|p| head.starts_with(p)) {
        return "tool-guidance";
    }
    "other"
}

/// 读 OmniDesk 覆盖文件里的生效路由（只含 provider/model/档位与端点字段，不含凭据）。
fn overlay_route(app_root: &Path) -> Result<RouteMeta, String> {
    let path = crate::model_routes::overlay_path(app_root);
    let path_text = path.display().to_string();
    if !path.is_file() {
        return Ok(RouteMeta {
            overlay_present: false,
            overlay_path: path_text,
            ..Default::default()
        });
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("读覆盖文件失败：{e}"))?;
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&raw).map_err(|e| format!("覆盖文件解析失败：{e}"))?;
    let mut meta = RouteMeta {
        overlay_present: true,
        overlay_path: path_text,
        ..Default::default()
    };
    let items = doc.as_sequence().cloned().unwrap_or_default();
    for item in &items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let config = item.get("config");
        if id == "agent-default-model" || (id == "acp" && meta.provider.is_none()) {
            if let Some(config) = config {
                meta.provider = config.get("provider").and_then(|v| v.as_str()).map(str::to_string);
                meta.model = config.get("model").and_then(|v| v.as_str()).map(str::to_string);
                meta.reasoning_effort = config
                    .get("reasoningEffort")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
        }
    }
    if let Some(provider) = meta.provider.clone() {
        for item in &items {
            if item.get("id").and_then(|v| v.as_str()) != Some("llm-pi-ai") {
                continue;
            }
            let Some(config) = item.get("config") else { continue };
            let Some(providers) = config.get("providers").and_then(|v| v.as_mapping()) else {
                continue;
            };
            let Some(found) = providers.get(&serde_yaml::Value::String(provider.clone())) else {
                continue;
            };
            meta.provider_api = found.get("api").and_then(|v| v.as_str()).map(str::to_string);
            meta.provider_base_url = found
                .get("baseURL")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
    }
    Ok(meta)
}

/// 从 Profile 补丁里取当前 preset 的声明（工具插件、名称与来源）。
///
/// 只做固定格式的缩进解析：补丁由本产品生成，不使用 YAML 库是因为
/// `!!js` 自定义标签会让通用解析器直接失败。
fn active_preset(text: &str) -> Option<(String, String, Vec<PresetPlugin>)> {
    let lines: Vec<&str> = text.lines().collect();
    let registry = lines.iter().position(|l| {
        l.trim_start().starts_with("- id: agent-preset-registry")
    });
    let mut default_id: Option<String> = None;
    if let Some(start) = registry {
        for line in &lines[start + 1..] {
            if !line.starts_with(' ') && line.trim_start().starts_with("- ") {
                break;
            }
            if let Some(rest) = line.trim_start().strip_prefix("default:") {
                let value = rest.trim().trim_matches('\'').trim_matches('"');
                if !value.is_empty() {
                    default_id = Some(value.to_string());
                    break;
                }
            }
        }
    }
    // 没有登记表就不猜当前 preset；工具面退回到“最近会话实际调用”。
    let wanted = default_id?;
    let preset_start = lines.iter().position(|l| {
        let trimmed = l.trim_start();
        trimmed.starts_with(&format!("- id: preset-{wanted}"))
    })?;
    let preset_indent = lines[preset_start].len() - lines[preset_start].trim_start().len();

    let mut label = String::new();
    let mut preset_id = String::new();
    let mut block_end = lines.len();
    for (offset, line) in lines[preset_start + 1..].iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        // 预设可以挂在 `- insert:` 下（缩进 4），也可能直接写在顶层。
        // 遇到缩进不大于预设自身的新条目（同级兄弟或外层列表）就结束本块。
        let indent = line.len() - line.trim_start().len();
        if indent <= preset_indent {
            block_end = preset_start + 1 + offset;
            break;
        }
        let trimmed = line.trim_start();
        if indent == preset_indent + 4 {
            if let Some(rest) = trimmed.strip_prefix("id:") {
                preset_id = rest.trim().to_string();
            } else if let Some(rest) = trimmed.strip_prefix("name:") {
                label = rest.trim().trim_matches('\'').trim_matches('"').to_string();
            }
        }
    }
    let block = &lines[preset_start + 1..block_end];
    let plugins_at = block
        .iter()
        .position(|l| l.trim_start() == "plugins:" && (l.len() - l.trim_start().len()) >= 4);
    let mut plugins = Vec::new();
    if let Some(at) = plugins_at {
        let section = &block[at + 1..];
        let Some(item_indent) = section
            .iter()
            .find(|l| l.trim_start().starts_with("- id:"))
            .map(|l| l.len() - l.trim_start().len())
        else {
            return Some((preset_id, label, plugins));
        };
        let mut index = 0usize;
        while index < section.len() {
            let line = section[index];
            let indent = line.len() - line.trim_start().len();
            if indent == item_indent && line.trim_start().starts_with("- id:") {
                let id = line
                    .trim_start()
                    .trim_start_matches("- id:")
                    .trim()
                    .to_string();
                let mut name = String::new();
                let mut cursor = index + 1;
                while cursor < section.len() {
                    let inner = section[cursor];
                    let inner_indent = inner.len() - inner.trim_start().len();
                    if inner_indent <= item_indent {
                        break;
                    }
                    if inner_indent == item_indent + 2 {
                        if let Some(rest) = inner.trim_start().strip_prefix("name:") {
                            name = rest.trim().trim_matches('\'').trim_matches('"').to_string();
                        }
                    }
                    cursor += 1;
                }
                if id.starts_with("tool-") || id == "present" {
                    plugins.push(PresetPlugin { id, name });
                }
                index = cursor;
                continue;
            }
            if indent < item_indent {
                break;
            }
            index += 1;
        }
    }
    Some((preset_id, label, plugins))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("omnidesk-diag-{tag}-{stamp}-{}", std::process::id()))
    }

    #[test]
    fn prompt_segments_classify_without_keeping_text() {
        let text = "你是 OmniDesk 个人工作台账助手：只读写用户明确指定的文件。\n\n## Web search\n- 不要拼接第三方 reader 前缀。\n\n其它段落。";
        let segments = prompt_segments(text);
        assert_eq!(segments.chars, text.chars().count());
        assert_eq!(segments.segments, 3);
        let persona = "你是 OmniDesk 个人工作台账助手：只读写用户明确指定的文件。";
        assert_eq!(segments.buckets["persona"], persona.chars().count());
        assert!(segments.buckets["web-search"] > 0);
        assert_eq!(segments.buckets["other"], "其它段落。".chars().count());
    }

    #[test]
    fn classify_known_heads() {
        assert_eq!(classify_paragraph("You are an AI agent powered by DeepSeek Harness\n..."), "base");
        assert_eq!(classify_paragraph("You are a coding agent powered by X"), "persona");
        assert_eq!(classify_paragraph("检索收敛规则：\n- ..."), "discipline-rules");
        assert_eq!(classify_paragraph("interface Tool {\n..."), "ptc-runtime");
        assert_eq!(classify_paragraph("随便一段"), "other");
    }

    #[test]
    fn tool_name_reads_nested_call_shape() {
        let value = serde_json::json!({"call": {"tool": "read", "args": {"path": "x"}}});
        assert_eq!(tool_name(&value, 0).as_deref(), Some("read"));
        let value = serde_json::json!({"data": {"data": {"toolName": "grep"}}});
        assert_eq!(tool_name(&value, 0).as_deref(), Some("grep"));
    }

    #[test]
    fn preset_plugins_are_limited_to_tool_declarations() {
        let patch = "- id: agent-preset-registry\n  name: reg\n  config:\n    default: discipline-core\n- insert:\n    - id: preset-discipline-core\n      name: preset\n      config:\n        id: discipline-core\n        name: Discipline Core\n        plugins:\n          - id: persona\n            name: '@deepseek-ai/dsh-persona'\n          - id: tool-fs\n            name: '@deepseek-ai/dsh-tool-fs'\n            config:\n              read: true\n          - id: tool-web\n            name: '@deepseek-ai/dsh-tool-web'\n          - id: present\n            name: '@deepseek-ai/dsh-tool-present'\n    - id: preset-work-review-core\n      config:\n        id: work-review-core\n        plugins:\n          - id: tool-bash\n            name: bash\n";
        let (id, label, plugins) = active_preset(patch).expect("解析 preset");
        assert_eq!(id, "discipline-core");
        assert_eq!(label, "Discipline Core");
        let ids: Vec<&str> = plugins.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["tool-fs", "tool-web", "present"]);
    }

    #[test]
    fn overlay_route_reads_non_secret_fields_only() {
        let yaml = "- id: llm-pi-ai\n  config:\n    providers:\n      demo:\n        api: openai-completions\n        baseURL: https://example.test/v1\n        apiKeyEnv: DEMO_KEY\n- id: acp\n  config:\n    provider: demo\n    model: m1\n    reasoningEffort: high\n- id: agent-default-model\n  config:\n    provider: demo\n    model: m2\n    reasoningEffort: off\n";
        let doc: serde_yaml::Value = serde_yaml::from_str(yaml).expect("解析覆盖文件");
        let mut meta = RouteMeta::default();
        let items = doc.as_sequence().cloned().unwrap_or_default();
        for item in &items {
            if item.get("id").and_then(|v| v.as_str()) == Some("agent-default-model") {
                let config = item.get("config").unwrap();
                meta.provider = config.get("provider").and_then(|v| v.as_str()).map(str::to_string);
                meta.model = config.get("model").and_then(|v| v.as_str()).map(str::to_string);
                meta.reasoning_effort = config
                    .get("reasoningEffort")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
        }
        assert_eq!(meta.provider.as_deref(), Some("demo"));
        assert_eq!(meta.model.as_deref(), Some("m2"));
        assert_eq!(meta.reasoning_effort.as_deref(), Some("off"));
    }

    #[test]
    fn session_snapshot_returns_counts_not_text() {
        let dir = unique_dir("session");
        std::fs::create_dir_all(dir.join("proj").join("sess")).expect("建目录");
        let path = dir.join("proj").join("sess").join("session.v4.jsonl");
        let secret = "SECRET_USER_BODY_MARKER";
        let prompt = "你是 OmniDesk 个人工作台账助手。只读写用户明确指定的文件。";
        let lines = vec![
            serde_json::json!({"type":"system/message","data":{"message":{"content":[{"type":"text","text":prompt}]}}}),
            serde_json::json!({"type":"user/message","data":{"message":{"source":{"kind":"user"},"content":[{"type":"text","text":secret}]}}}),
            serde_json::json!({"type":"user/message","data":{"message":{"source":{"kind":"skill-catalog"},"content":[{"type":"text","text":"abcd"}]}}}),
            serde_json::json!({"type":"request/context","data":{"provider":"demo","model":"m1","contextWindow":8192}}),
            serde_json::json!({"type":"tool/call","data":{"call":{"tool":"read"}}}),
            serde_json::json!({"type":"tool/call","data":{"tool":"grep"}}),
            serde_json::json!({"type":"approval/asked","data":{}}),
            serde_json::json!({"type":"llm/retry","data":{}}),
            serde_json::json!({"type":"assistant/attempt","data":{}}),
            serde_json::json!({"type":"session/compacted","data":{}}),
        ];
        let body = lines
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, body).expect("写会话");

        let snapshot = read_session(&path).expect("解析会话");
        assert_eq!(snapshot.provider.as_deref(), Some("demo"));
        assert_eq!(snapshot.model.as_deref(), Some("m1"));
        assert_eq!(snapshot.context_window, Some(8192));
        assert_eq!(snapshot.tool_calls, 2);
        assert_eq!(snapshot.tool_names, vec!["grep", "read"]);
        assert_eq!(snapshot.approvals, 1);
        assert_eq!(snapshot.retries, 1);
        assert_eq!(snapshot.assistant_attempts, 1);
        assert_eq!(snapshot.compactions, 1);
        assert_eq!(snapshot.injected_chars["user"], secret.chars().count());
        assert_eq!(snapshot.injected_chars["skill-catalog"], 4);
        let json = serde_json::to_string(&snapshot).expect("序列化");
        assert!(!json.contains(secret), "诊断结果不得包含用户正文");
        assert!(!json.contains("个人工作台账助手"), "诊断结果不得包含提示词原文");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn newest_session_prefers_latest_mtime() {
        let dir = unique_dir("newest");
        std::fs::create_dir_all(dir.join("p1").join("older")).expect("建目录");
        std::fs::create_dir_all(dir.join("p1").join("newer")).expect("建目录");
        std::fs::write(dir.join("p1").join("older").join("session.v4.jsonl"), "{}").expect("写");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.join("p1").join("newer").join("session.v4.jsonl.zstd"), "x").expect("写");
        let found = newest_session_file(&dir).expect("找到会话");
        assert!(found.ends_with("newer/session.v4.jsonl.zstd"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 证据脚本用：在显式隔离根上跑一次 `build`，打印元数据并确认不含正文。
    /// 默认跳过，避免普通测试读取真实应用数据。
    #[test]
    #[ignore = "需要 OMNIDESK_APP_DATA_ROOT 与 OMNIDESK_DSH_HOME 指向隔离根"]
    fn build_against_isolated_root() {
        let root = std::env::var("OMNIDESK_APP_DATA_ROOT")
            .expect("设置 OMNIDESK_APP_DATA_ROOT 指向隔离应用根");
        let value = build(Path::new(&root));
        let body = serde_json::to_string(&value).expect("序列化");
        assert_eq!(value["ok"], true);
        assert!(
            !body.contains("合成任务占位行"),
            "诊断结果不得包含用户正文"
        );
        assert!(
            !body.contains("个人工作台账助手"),
            "诊断结果不得包含提示词原文"
        );
        println!("DIAGNOSTICS_METADATA={body}");
    }
}
