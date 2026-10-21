//! 从非秘密配置识别“实际任务会走哪个模型端点”（OD-ARCH-021）。
//!
//! 做法：用 DSH 公开的 `--dump-config` 读取合成后的 Profile（含 `--patch`），
//! 由仓库内脚本 `scripts/dump-model-route.mjs` 解析成 provider / model / endpoint /
//! 类别（local / internal / external / unknown）。脚本以 stdin 传给 node，
//! 不在磁盘上落临时文件，也不读取任何凭证值。
//!
//! 约束：
//!   * 类别只表明“端点在哪”，不代表已获授权；授权由 settings 中的用户选择决定。
//!   * 任何异常（node 缺失、超时、JSON 非法、provider 无法识别）一律视为不可解析，
//!     由宿主拒绝，绝不当成“本机”或“已允许”。
//!   * 输出不含凭证、完整环境或用户数据。

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::engine_supervisor::EngineConfig;

/// 分类器脚本（与手工 / 验证脚本使用同一份源码）。
const CLASSIFIER_SCRIPT: &str = include_str!("../scripts/dump-model-route.mjs");
/// 分类超时：只做一次配置转储，正常在 1 秒内完成。
const CLASSIFY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RouteFacts {
    pub provider: String,
    pub model: String,
    pub endpoint: Option<String>,
    pub endpoint_host: Option<String>,
    /// local / private / public / unknown
    pub category: String,
    pub category_reason: String,
    pub basis: Vec<String>,
    /// true = 内容离开本机；false = 仅本机；null = 无法判断。
    ///
    /// 注意：该字段只表达“是否离开本机”，**不代表**所属组织，也不判断数据是否
    /// 离开所在单位。非回环地址的归属与转发路径一律不作承诺。
    pub leaves_device: Option<bool>,
}

impl RouteFacts {
    /// 该类别是否属于用户可自行开启的类别之一。未知类别没有开启入口。
    ///
    /// 类别只区分“本机回环 / 非回环”，不承诺组织归属：地址特征无法证明端点属于
    /// 哪个组织，也无法证明数据是否离开所在组织。
    pub fn is_configurable(&self) -> bool {
        matches!(self.category.as_str(), "local" | "private" | "public")
    }
}

/// 调用分类器。返回 Ok 时类别可能是 `unknown`（由调用方拒绝）。
pub fn classify(config: &EngineConfig) -> Result<RouteFacts, String> {
    let node = std::env::var("OMNIDESK_NODE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "node".to_string());

    let route_args = serde_json::json!({
        "dshCli": config.dsh_cli.display().to_string(),
        "dshHome": config.dsh_home.display().to_string(),
        "profile": config.profile,
        // OD-ARCH-048：通道决定实际生效的路由条目（web-api → agent-default-model）。
        "channel": config.channel,
        "patches": config.patch_files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
    })
    .to_string();

    let mut child = Command::new(&node)
        .arg("--input-type=module")
        .env("OMNIDESK_ROUTE_ARGS", route_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("无法启动端点分类器（{}）：{}", node, e))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "分类器 stdin 不可用".to_string())?;
        stdin
            .write_all(CLASSIFIER_SCRIPT.as_bytes())
            .map_err(|e| format!("无法把分类器脚本交给 node：{}", e))?;
        // 显式关闭，让 node 读到 EOF 后开始执行。
    }

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "分类器 stdout 不可用".to_string())?;
    let reader = thread::spawn(move || {
        let mut buffer = String::new();
        let _ = stdout.read_to_string(&mut buffer);
        buffer
    });

    let deadline = Instant::now() + CLASSIFY_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("端点分类超时（模型配置转储未在 30 秒内完成）".to_string());
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(format!("等待端点分类器失败：{}", e));
            }
        }
    }

    let text = reader
        .join()
        .map_err(|_| "端点分类器输出读取失败".to_string())?;
    parse_output(&text)
}

fn parse_output(stdout: &str) -> Result<RouteFacts, String> {
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| "端点分类器没有输出可解析的 JSON".to_string())?;
    let value: Value = serde_json::from_str(line.trim())
        .map_err(|e| format!("端点分类器输出不是合法 JSON：{}", e))?;

    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        let code = value
            .pointer("/error/code")
            .and_then(Value::as_str)
            .unwrap_or("classifier_error");
        let message = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("端点无法解析");
        return Err(format!("{}：{}", code, message));
    }

    let route = value
        .get("route")
        .ok_or_else(|| "端点分类器未返回 route".to_string())?;
    let provider = route
        .get("provider")
        .and_then(Value::as_str)
        .ok_or_else(|| "端点分类器未返回 provider".to_string())?
        .to_string();
    let model = route
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| "端点分类器未返回 model".to_string())?
        .to_string();
    let category = route
        .get("category")
        .and_then(Value::as_str)
        .ok_or_else(|| "端点分类器未返回 category".to_string())?;
    if !matches!(category, "local" | "private" | "public" | "unknown") {
        return Err(format!("端点分类器返回了未定义的类别：{}", category));
    }

    Ok(RouteFacts {
        provider,
        model,
        endpoint: route.get("endpoint").and_then(Value::as_str).map(str::to_string),
        endpoint_host: route
            .get("endpoint_host")
            .and_then(Value::as_str)
            .map(str::to_string),
        category: category.to_string(),
        category_reason: route
            .get("category_reason")
            .and_then(Value::as_str)
            .unwrap_or("未提供原因")
            .to_string(),
        basis: route
            .get("basis")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        leaves_device: route.get("leaves_device").and_then(Value::as_bool),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_route() {
        let text = r#"{"ok":true,"route":{"provider":"fake","model":"m","endpoint":"http://127.0.0.1:9/v1","endpoint_host":"127.0.0.1:9","category":"local","category_reason":"IPv4 回环地址","basis":["provider=fake"],"leaves_device":false}}"#;
        let facts = parse_output(text).expect("应解析成功");
        assert_eq!(facts.category, "local");
        assert_eq!(facts.leaves_device, Some(false));
        assert!(facts.is_configurable());
    }

    #[test]
    fn unresolved_and_undefined_categories_are_errors() {
        let unresolved = r#"{"ok":false,"error":{"code":"route_unresolved","message":"无法确定端点"}}"#;
        assert!(parse_output(unresolved).unwrap_err().contains("route_unresolved"));

        let undefined = r#"{"ok":true,"route":{"provider":"p","model":"m","category":"somewhere"}}"#;
        assert!(parse_output(undefined).is_err());

        assert!(parse_output("not json").is_err());
    }

    #[test]
    fn unknown_category_is_not_configurable() {
        let text = r#"{"ok":true,"route":{"provider":"p","model":"m","category":"unknown","category_reason":"缺少端点"}}"#;
        let facts = parse_output(text).unwrap();
        assert!(!facts.is_configurable());
        assert_eq!(facts.leaves_device, None);
    }

    /// OD-ARCH-022：类别只区分“本机回环 / 非回环”，不再出现承诺组织归属的类别名。
    #[test]
    fn configured_categories_exclude_org_attribution_names() {
        for (category, configurable) in
            [("local", true), ("private", true), ("public", true), ("unknown", false)]
        {
            let text = format!(
                r#"{{"ok":true,"route":{{"provider":"p","model":"m","category":"{}","category_reason":"r"}}}}"#,
                category
            );
            let facts = parse_output(&text).expect("应解析成功");
            assert_eq!(facts.is_configurable(), configurable, "类别 {}", category);
        }
        // 旧类别名（internal = 单位内部、external = 外部服务）不再被接受，
        // 避免把组织归属断言重新带回界面。
        for legacy in ["internal", "external"] {
            let text = format!(
                r#"{{"ok":true,"route":{{"provider":"p","model":"m","category":"{}","category_reason":"r"}}}}"#,
                legacy
            );
            assert!(parse_output(&text).is_err(), "旧类别 {} 应被拒绝", legacy);
        }
    }
}
