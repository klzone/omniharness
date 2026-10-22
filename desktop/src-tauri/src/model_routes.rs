//! 模型路由的**列出**与**选择**（OD-ARCH-034）。
//!
//! 背景：DSH 的「模型选择」是**按 Profile 存**的——用户在 DSH 界面（web profile）里配的自定义
//! provider 与默认模型，引擎跑的另一个 profile（acp）**看不到**；两边只有**凭据**是共享的
//! （`$DSH_HOME/.credentials.yaml` 里的 ref）。于是引擎仍走旧路由，任务失败或只拿到部分结果，
//! 而工作台里的「刷新端点」如实显示的也正是引擎那条旧路由。
//!
//! 本模块做两件事：
//!   1. **列出**各 Profile 生效的 provider / model（只用 DSH 公开的 `--dump-config`，不读凭证值）；
//!   2. **选择**：把某个 Profile 里的 provider 配置 + 选中的默认模型写成一份 **OmniDesk 自己的**
//!      patch 覆盖文件，宿主以 `--patch` 传给引擎与端点分类器 —— 于是引擎与界面判定用同一份路由。
//!
//! 安全边界：
//!   * 写出的覆盖文件只含**非机密**字段（baseURL / api / apiKeyEnv / 模型 id）；key 仍由共享凭据服务提供。
//!   * 只写 OmniDesk 自己数据根下的那一个文件，**不碰用户的 DSH Profile**。
//!   * 参数只接受「已列出的 profile / provider / model」组合，不接受任意命令或路径。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::engine_supervisor::EngineConfig;

const ROUTES_SCRIPT: &str = include_str!("../scripts/model-routes.mjs");
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(90);

/// OmniDesk 自己数据根下的模型路由覆盖文件（不含任何机密）。
pub fn overlay_path(app_root: &Path) -> PathBuf {
    app_root.join("config").join("omnidesk-model.yml")
}

/// 引擎应当附加的覆盖文件（存在才附加）。
pub fn overlay_for_engine(app_root: &Path) -> Option<PathBuf> {
    let p = overlay_path(app_root);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// DSH 数据根下的 Profile 列表（引擎当前用的那个永远包含在内）。
pub fn discover_profiles(dsh_home: &Path, engine_profile: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![engine_profile.to_string()];
    if let Ok(entries) = std::fs::read_dir(dsh_home.join("profiles")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

fn run_script(payload: Value) -> Result<Value, String> {
    let node = std::env::var("OMNIDESK_NODE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "node".to_string());

    let mut child = Command::new(&node)
        .arg("--input-type=module")
        .env("OMNIDESK_ROUTE_ARGS", payload.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("无法启动模型路由脚本（{}）：{}", node, e))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "模型路由脚本 stdin 不可用".to_string())?;
        stdin
            .write_all(ROUTES_SCRIPT.as_bytes())
            .map_err(|e| format!("无法把模型路由脚本交给 node：{}", e))?;
        // 显式关闭，让 node 读到 EOF 后开始执行。
    }

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "模型路由脚本 stdout 不可用".to_string())?;
    let reader = thread::spawn(move || {
        let mut buffer = String::new();
        let _ = stdout.read_to_string(&mut buffer);
        buffer
    });

    let deadline = Instant::now() + SCRIPT_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("模型路由脚本超时（DSH 配置转储未在 90 秒内完成）".to_string());
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(format!("等待模型路由脚本失败：{}", e));
            }
        }
    }

    let text = reader
        .join()
        .map_err(|_| "模型路由脚本输出读取失败".to_string())?;
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| "模型路由脚本没有输出可解析的 JSON".to_string())?;
    let value: Value = serde_json::from_str(line.trim())
        .map_err(|e| format!("模型路由脚本输出不是合法 JSON：{}", e))?;
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        let message = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("模型路由脚本失败");
        return Err(message.to_string());
    }
    Ok(value)
}

/// 列出各 Profile 生效的 provider / model。
pub fn list_routes(config: &EngineConfig, profiles: &[String]) -> Result<Value, String> {
    run_script(json!({
        "dshCli": config.dsh_cli.display().to_string(),
        "dshHome": config.dsh_home.display().to_string(),
        "profiles": profiles,
    }))
}

/// 只读：引擎 Profile 当前生效的**非机密**路由集合（OD-ARCH-065 S3）。
///
/// 用于在写覆盖文件前做「Profile 路由 + OmniDesk 路由」合并：补丁层对 config 是
/// 浅合并，若只写自己那几条会静默抹掉 Profile 自带的路由。
///
/// 必须带上引擎实际会加载的所有补丁（否则只存在于补丁里的路由会被当成不存在而抹掉），
/// 但**排除 OmniDesk 自己那份覆盖文件**：它正是本次要重算的内容，读进来会把已删除的
/// 提供商带回来。
pub fn base_routes(
    config: &EngineConfig,
    app_root: &Path,
) -> Result<Vec<crate::model_settings::ProviderRoute>, String> {
    let overlay = overlay_path(app_root);
    let patches: Vec<String> = config
        .patch_files
        .iter()
        .filter(|p| p.as_path() != overlay.as_path())
        .map(|p| p.display().to_string())
        .collect();
    let value = run_script(json!({
        "dshCli": config.dsh_cli.display().to_string(),
        "dshHome": config.dsh_home.display().to_string(),
        "profiles": [config.profile],
        "patches": patches,
        "dumpRoutes": true,
    }))?;
    let raw = value
        .get("providers")
        .cloned()
        .ok_or_else(|| "DSH 配置转储里没有 providers 字段".to_string())?;
    serde_json::from_value(raw).map_err(|e| {
        format!(
            "无法解析 DSH Profile 的路由集合（{}）；为避免写出一份不完整的覆盖文件，本次不保存。",
            e
        )
    })
}

/// 只读：DSH 内置（pi-ai 自带）的 provider 名清单。
///
/// \`llm/discoverModels\` 遇到这些名字会直接回内置模型表、不发网络请求，
/// 因此探测结果必须据此标注，不能当作连通性证明（OD-ARCH-065 S1）。
pub fn builtin_providers(config: &EngineConfig) -> Result<Vec<String>, String> {
    let value = run_script(json!({
        "dshCli": config.dsh_cli.display().to_string(),
        "dshHome": config.dsh_home.display().to_string(),
        "builtinProviders": true,
    }))?;
    let raw = value
        .get("providers")
        .cloned()
        .ok_or_else(|| "内置 provider 目录没有返回清单".to_string())?;
    serde_json::from_value(raw).map_err(|e| format!("无法解析内置 provider 目录：{}", e))
}

/// 选定一个 provider/model，写出覆盖文件（只含非机密字段）。
pub fn select_route(
    config: &EngineConfig,
    overlay: &Path,
    source_profile: &str,
    provider: &str,
    model: &str,
    reasoning_effort: Option<&str>,
) -> Result<Value, String> {
    if source_profile.trim().is_empty() || provider.trim().is_empty() || model.trim().is_empty() {
        return Err("请选择要使用的 Profile、provider 与模型。".to_string());
    }
    let mut payload = json!({
        "dshCli": config.dsh_cli.display().to_string(),
        "dshHome": config.dsh_home.display().to_string(),
        "profiles": [source_profile],
        "writeOverlay": overlay.display().to_string(),
        "provider": provider,
        "model": model,
    });
    if let Some(effort) = reasoning_effort {
        payload["reasoningEffort"] = Value::String(effort.to_string());
    }
    run_script(payload)
}
