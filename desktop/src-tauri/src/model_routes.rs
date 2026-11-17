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

/// 已验证的模型思考档位（OD-ARCH-178 阶段 A）。
///
/// 发行链的 Profile 没有声明 `reasoningEfforts`，DSH 便会拒绝一切显式档位；这份清单
/// 只记录**经真实调用实测**过的映射，宿主在生成 modelCatalog 与覆盖文件时补齐。
/// 未实测的模型一律不入表：宁可保持「跟随模型默认」，也不替端点猜测能力。
///
/// 条目按 `(provider, model, [(DSH 档位, 端点取值)])` 排列；`off` 与 `none` 这类
/// 会被 YAML 当作布尔的词，在渲染时由 `yaml_scalar` 加引号。
pub const VERIFIED_EFFORTS: &[(&str, &str, &[(&str, &str)])] = &[(
    "opendesign",
    "deepseek-v4.1-flash",
    &[
        ("off", "none"),
        ("minimal", "minimal"),
        ("low", "low"),
        ("medium", "medium"),
        ("high", "high"),
        ("xhigh", "xhigh"),
        ("max", "max"),
    ],
), (
    // 同族继承的保守声明（2026-11-18）：eco 与 flash 同网关同族，先只声明
    // off 一个档位供用户实测；通过真实调用确认后再补其余档位。不一次猜全。
    "opendesign",
    "deepseek-v4.1-flash-eco",
    &[("off", "none")],
)];

/// 查出某个模型在清单里声明的档位映射；未命中返回 `None`。
pub fn verified_efforts(provider: &str, model: &str) -> Option<serde_json::Value> {
    VERIFIED_EFFORTS
        .iter()
        .find(|(p, m, _)| *p == provider && *m == model)
        .map(|(_, _, efforts)| {
            let mut map = serde_json::Map::new();
            for (level, value) in efforts.iter() {
                map.insert((*level).to_string(), Value::String((*value).to_string()));
            }
            Value::Object(map)
        })
}

/// 把一个 model entry 并入清单声明。
///
/// 只在 Profile 完全没写 `reasoningEfforts` 键时补；已有声明（含 `false` 或其它
/// 形状）一律原样保留，避免宿主覆盖 Profile 的能力事实。
pub fn merge_verified_efforts(entry: &mut Value, provider: &str, model: &str) {
    let Some(obj) = entry.as_object_mut() else {
        return;
    };
    if obj.contains_key("reasoningEfforts") {
        return;
    }
    if let Some(efforts) = verified_efforts(provider, model) {
        obj.insert("reasoningEfforts".to_string(), efforts);
    }
}

/// `task_list_model_routes` 返回的 modelCatalog 也要带清单声明（前端证据门的数据源）。
///
/// 输入是 `{ ok, profiles: [{ profile, providers: [{ id, modelEntries }] }] }` 形态；
/// 每个 model entry 按 `provider + id` 命中清单。已有声明不覆盖。
pub fn merge_verified_efforts_in_catalog(value: &mut Value) {
    let Some(profiles) = value.get_mut("profiles").and_then(Value::as_array_mut) else {
        return;
    };
    for profile in profiles {
        let Some(providers) = profile.get_mut("providers").and_then(Value::as_array_mut) else {
            continue;
        };
        for provider in providers {
            let Some(provider_id) = provider.get("id").and_then(Value::as_str).map(str::to_string)
            else {
                continue;
            };
            let Some(entries) = provider.get_mut("modelEntries").and_then(Value::as_array_mut)
            else {
                continue;
            };
            for entry in entries {
                let Some(model) = entry.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let model = model.to_string();
                merge_verified_efforts(entry, &provider_id, &model);
            }
        }
    }
}

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

/// 取 stderr 的有界尾部（OD-ARCH-095 S04）。
///
/// 只用于让失败可诊断：最多 3 条非空行、总长 ≤ 600 字符，与脚本侧口径一致。
/// 返回空串表示没有可用线索，调用方据此退回原有文案。
fn stderr_tail(raw: &str) -> String {
    let lines: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let start = lines.len().saturating_sub(3);
    lines[start..].join(" | ").chars().take(600).collect()
}

fn run_script(payload: Value) -> Result<Value, String> {
    let node = crate::platform::resolve_node()?;

    let mut cmd = Command::new(&node);
    cmd
        .arg("--input-type=module")
        .env("OMNIDESK_ROUTE_ARGS", payload.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // OD-ARCH-095 S04：stderr 不再丢弃。脚本自身会把 DSH 的失败原因附在错误消息里，
        // 但脚本崩在 `main()` 之外（语法/依赖问题）时不会有 JSON，唯一线索就在 stderr。
        .stderr(Stdio::piped());
    // Windows：node 是控制台子系统程序，不隐藏会弹黑框（OD-ARCH-134）。
    crate::platform::hide_console(&mut cmd);
    let mut child = cmd
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

    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "模型路由脚本 stderr 不可用".to_string())?;
    let err_reader = thread::spawn(move || {
        let mut buffer = String::new();
        let _ = stderr.read_to_string(&mut buffer);
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
                let _ = err_reader.join();
                return Err("模型路由脚本超时（DSH 配置转储未在 90 秒内完成）".to_string());
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                let _ = err_reader.join();
                return Err(format!("等待模型路由脚本失败：{}", e));
            }
        }
    }

    let text = reader
        .join()
        .map_err(|_| "模型路由脚本输出读取失败".to_string())?;
    let err_text = err_reader.join().unwrap_or_default();
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| {
            let tail = stderr_tail(&err_text);
            if tail.is_empty() {
                "模型路由脚本没有输出可解析的 JSON".to_string()
            } else {
                format!("模型路由脚本没有输出可解析的 JSON：{}", tail)
            }
        })?;
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
///
/// OD-ARCH-095 S01：必须带上引擎实际加载的补丁集合（含 OmniDesk 自己的覆盖文件）。
/// 之前只传了 profile 名，导致只在覆盖文件里定义的 provider 在候选里完全消失；
/// 界面拿到空候选后，场景思考档又用空 Profile 发起切换，于是登录即报错。
/// 这里与 `base_routes` 的差别是**要**包含覆盖文件：列表要展示的正是「引擎此刻会走的路由」。
pub fn list_routes(config: &EngineConfig, profiles: &[String]) -> Result<Value, String> {
    let mut value = run_script(list_payload(config, profiles))?;
    merge_verified_efforts_in_catalog(&mut value);
    Ok(value)
}

/// 候选枚举的请求体（抽出来便于断言：OD-ARCH-095 S01 的补丁集合就在这份 payload 里）。
fn list_payload(config: &EngineConfig, profiles: &[String]) -> Value {
    json!({
        "dshCli": config.dsh_cli.display().to_string(),
        "dshHome": config.dsh_home.display().to_string(),
        "profiles": profiles,
        "patches": config
            .patch_files
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>(),
    })
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
    let mut payload = select_payload(config, overlay, source_profile, provider, model);
    if let Some(effort) = reasoning_effort {
        payload["reasoningEffort"] = Value::String(effort.to_string());
    }
    run_script(payload)
}

/// 写覆盖文件的请求体（抽出来便于断言：OD-ARCH-095 的补丁集合必须在这里）。
///
/// 与 `base_routes` 不同，这里**包含** OmniDesk 自己的覆盖文件：用户在下拉里看到的
/// 候选就来自「Profile + 全部补丁」，若写的时候少读一份补丁，只在覆盖文件里定义的
/// provider 就会被判成「该 Profile 里没有这个 provider」，选中随即失败。
fn select_payload(
    config: &EngineConfig,
    overlay: &Path,
    source_profile: &str,
    provider: &str,
    model: &str,
) -> Value {
    json!({
        "dshCli": config.dsh_cli.display().to_string(),
        "dshHome": config.dsh_home.display().to_string(),
        "profiles": [source_profile],
        "patches": config
            .patch_files
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>(),
        "writeOverlay": overlay.display().to_string(),
        "provider": provider,
        "model": model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OD-ARCH-095 S04：失败线索只取有界尾部，避免把大段栈信息带回界面。
    #[test]
    fn stderr_tail_keeps_only_last_three_lines() {
        let raw = "a\n\nb\nc\nd\n";
        assert_eq!(stderr_tail(raw), "b | c | d");
    }

    #[test]
    fn stderr_tail_is_empty_for_blank_output() {
        assert_eq!(stderr_tail("\n  \n"), "");
    }

    #[test]
    fn stderr_tail_caps_total_length() {
        let long = "x".repeat(2000);
        assert!(stderr_tail(&long).chars().count() <= 600);
    }

    fn catalog_with(entries: Value) -> Value {
        json!({
            "ok": true,
            "profiles": [{
                "profile": "web",
                "providers": [{
                    "id": "opendesign",
                    "models": ["deepseek-v4.1-flash", "deepseek-v4.1-flash-eco", "glm-5.3-flashx"],
                    "modelEntries": entries,
                }],
            }],
        })
    }

    #[test]
    fn catalog_merges_verified_efforts_when_profile_is_silent() {
        let mut catalog = catalog_with(json!([
            { "id": "deepseek-v4.1-flash", "name": "DeepSeek V4.1 Flash" },
            { "id": "deepseek-v4.1-flash-eco", "name": "DeepSeek V4.1 Flash Eco" },
            { "id": "glm-5.3-flashx", "name": "GLM 5.3 FlashX" },
        ]));
        merge_verified_efforts_in_catalog(&mut catalog);
        let efforts = catalog
            .pointer("/profiles/0/providers/0/modelEntries/0/reasoningEfforts")
            .expect("清单命中的模型必须补上 reasoningEfforts");
        let map = efforts.as_object().expect("reasoningEfforts 必须是映射");
        assert_eq!(map.len(), 7);
        assert_eq!(map.get("off").and_then(Value::as_str), Some("none"));
        assert_eq!(map.get("max").and_then(Value::as_str), Some("max"));
        let eco = catalog
            .pointer("/profiles/0/providers/0/modelEntries/1/reasoningEfforts")
            .and_then(Value::as_object)
            .expect("eco 应只声明 off");
        assert_eq!(eco.len(), 1);
        assert_eq!(eco.get("off").and_then(Value::as_str), Some("none"));
        assert!(catalog
            .pointer("/profiles/0/providers/0/modelEntries/2/reasoningEfforts")
            .is_none());
    }

    #[test]
    fn catalog_keeps_profile_declaration_untouched() {
        let mut catalog = catalog_with(json!([{
            "id": "deepseek-v4.1-flash",
            "reasoningEfforts": { "off": "none", "high": "high" },
        }]));
        merge_verified_efforts_in_catalog(&mut catalog);
        let efforts = catalog
            .pointer("/profiles/0/providers/0/modelEntries/0/reasoningEfforts")
            .and_then(Value::as_object)
            .expect("已有声明必须保留");
        assert_eq!(efforts.len(), 2, "不得把清单声明追加上去");
        assert_eq!(efforts.get("high").and_then(Value::as_str), Some("high"));
        assert!(!efforts.contains_key("max"));
    }

    #[test]
    fn catalog_leaves_unlisted_provider_and_models_unchanged() {
        let mut catalog = json!({
            "ok": true,
            "profiles": [{
                "profile": "web",
                "providers": [{
                    "id": "volcengine",
                    "models": ["deepseek-v4.1-flash"],
                    "modelEntries": [{ "id": "deepseek-v4.1-flash" }],
                }],
            }],
        });
        merge_verified_efforts_in_catalog(&mut catalog);
        assert!(catalog
            .pointer("/profiles/0/providers/0/modelEntries/0/reasoningEfforts")
            .is_none());
    }

    /// OD-ARCH-095 S01：枚举候选的 payload 必须带上引擎实际加载的补丁集合。
    /// 这条断言防的是「覆盖文件里的 provider 在候选里消失」再次发生。
    #[test]
    fn list_payload_includes_engine_patches() {
        let config = test_config();
        let payload = list_payload(&config, &["web".to_string()]);
        let patches = payload
            .get("patches")
            .and_then(Value::as_array)
            .expect("候选枚举必须传 patches");
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].as_str(), Some("/data/config/omnidesk-model.yml"));
    }

    /// OD-ARCH-095：写覆盖文件时也必须带补丁集合。
    ///
    /// 窗口实测踩到过：候选能列出只存在于覆盖文件里的 provider，但写的时候不带补丁，
    /// 于是 DSH 转储里看不到它，选中即报「该 Profile 里没有这个 provider」。
    #[test]
    fn select_payload_includes_engine_patches() {
        let config = test_config();
        let payload = select_payload(
            &config,
            Path::new("/data/config/omnidesk-model.yml"),
            "web",
            "p",
            "m",
        );
        let patches = payload
            .get("patches")
            .and_then(Value::as_array)
            .expect("写覆盖文件必须传 patches");
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].as_str(), Some("/data/config/omnidesk-model.yml"));
        assert_eq!(payload.get("provider").and_then(Value::as_str), Some("p"));
    }

    /// OD-ARCH-095 S02/S03：空 Profile 在宿主侧仍被拒绝（前端守卫是第一道，不是唯一一道）。
    #[test]
    fn select_route_rejects_blank_profile() {
        let config = test_config();
        let err = select_route(&config, Path::new("/data/config/omnidesk-model.yml"), "  ", "p", "m", None)
            .expect_err("空 Profile 必须被拒绝");
        assert!(err.contains("请选择要使用的 Profile"));
    }

    /// OD-ARCH-175 阶段 2：回退用 `select_route(..., None)` 重写覆盖文件后，文件里
    /// **不再含** `reasoningEffort`；带显式档位时仍会写入（正对照）。
    ///
    /// 这条用例直接调用生产路径（node 脚本 + 仓库内 DSH CLI，只做配置转储，不发网络
    /// 请求、不读凭证）；环境缺少 node / DSH CLI 时按既有惯例跳过。
    #[test]
    fn select_route_none_effort_writes_overlay_without_reasoning_effort() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("仓库根")
            .to_path_buf();
        let cli = repo.join("runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js");
        if !cli.is_file() || crate::platform::resolve_node().is_err() {
            eprintln!("跳过：本机没有 node 或仓库内 DSH CLI（{}）", cli.display());
            return;
        }
        let base = std::env::temp_dir().join(format!(
            "omnidesk-select-route-{}-{}",
            std::process::id(),
            line!()
        ));
        let overlay = base.join("omnidesk-model.yml");
        let _ = std::fs::remove_dir_all(&base);
        let config = EngineConfig {
            python: "/usr/bin/python3".to_string(),
            engine_dir: repo.join("engine"),
            dsh_cli: cli.clone(),
            dsh_home: repo.join("runtime/dsh-home-seed"),
            agents_home: base.join("agents"),
            result_dir: base.join("results"),
            engine_state_dir: base.join("engine-state"),
            profile: "web".to_string(),
            channel: "web-api".to_string(),
            patch_files: Vec::new(),
        };

        // 正对照：显式档位会写入。
        select_route(
            &config,
            &overlay,
            "web",
            "agnescn",
            "agnes-3.0-flash",
            Some("high"),
        )
        .expect("带档位重写覆盖文件应成功");
        let text = std::fs::read_to_string(&overlay).expect("读回覆盖文件");
        assert!(
            text.contains("reasoningEffort"),
            "显式档位应写入覆盖文件：{text}"
        );

        // 回退路径：引擎当前 Profile 本身没有这个 provider，它只存在于覆盖文件里；
        // 必须仍能靠 patches 合并找到该路由并去掉显式档位（与生产落点一致）。
        let engine_config = EngineConfig {
            profile: "omnidesk-base".to_string(),
            patch_files: vec![overlay.clone()],
            ..config
        };
        select_route(
            &engine_config,
            &overlay,
            "omnidesk-base",
            "agnescn",
            "agnes-3.0-flash",
            None,
        )
        .expect("不带档位重写覆盖文件应成功");
        let text = std::fs::read_to_string(&overlay).expect("读回覆盖文件");
        assert!(
            !text.contains("reasoningEffort"),
            "回退重写后覆盖文件不得再含 reasoningEffort：{text}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// OD-ARCH-178 阶段 A：发行链 Profile 没写 `reasoningEfforts` 时，
    /// `task_list_model_routes` 的数据源也要补上已验证声明。
    ///
    /// 用仓库内 DSH CLI 做一次真实配置转储（只读，不发网络请求、不读凭证），
    /// 以隔离的临时 patch 定义 opendesign，避免依赖本机已安装数据。
    #[test]
    fn list_routes_merges_verified_efforts_from_profile_without_declaration() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("仓库根")
            .to_path_buf();
        let cli = repo.join("runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js");
        if !cli.is_file() || crate::platform::resolve_node().is_err() {
            eprintln!("跳过：本机没有 node 或仓库内 DSH CLI（{}）", cli.display());
            return;
        }
        let base = std::env::temp_dir().join(format!(
            "omnidesk-list-routes-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("建立临时目录");
        let patch = base.join("opendesign.patch.yml");
        std::fs::write(
            &patch,
            "- id: llm-pi-ai\n  name: \"@deepseek-ai/dsh-llm-pi-ai\"\n  config:\n    providers:\n      opendesign:\n        displayName: OpenDesign\n        api: openai-completions\n        baseURL: https://amr-link.open-design.ai/v1\n        models:\n          - id: deepseek-v4.1-flash\n            name: DeepSeek V4.1 Flash\n          - id: deepseek-v4.1-flash-eco\n            name: DeepSeek V4.1 Flash Eco\n",
        )
        .expect("写入临时 patch");
        let config = EngineConfig {
            python: "/usr/bin/python3".to_string(),
            engine_dir: repo.join("engine"),
            dsh_cli: cli,
            dsh_home: repo.join("runtime/dsh-home-seed"),
            agents_home: base.join("agents"),
            result_dir: base.join("results"),
            engine_state_dir: base.join("engine-state"),
            profile: "web".to_string(),
            channel: "web-api".to_string(),
            patch_files: vec![patch],
        };
        let value = list_routes(&config, &["web".to_string()]).expect("列出路由应成功");
        let provider = value
            .pointer("/profiles/0/providers")
            .and_then(Value::as_array)
            .and_then(|providers| {
                providers
                    .iter()
                    .find(|p| p.get("id").and_then(Value::as_str) == Some("opendesign"))
            })
            .expect("Profile 里应有 opendesign");
        let entries = provider
            .get("modelEntries")
            .and_then(Value::as_array)
            .expect("应有 modelEntries");
        let flash = entries
            .iter()
            .find(|e| e.get("id").and_then(Value::as_str) == Some("deepseek-v4.1-flash"))
            .expect("应有 deepseek-v4.1-flash");
        let efforts = flash
            .get("reasoningEfforts")
            .and_then(Value::as_object)
            .expect("未声明模型应合并清单档位");
        assert_eq!(efforts.len(), 7);
        assert_eq!(efforts.get("off").and_then(Value::as_str), Some("none"));
        let eco = entries
            .iter()
            .find(|e| e.get("id").and_then(Value::as_str) == Some("deepseek-v4.1-flash-eco"))
            .expect("应有 eco");
        let eco_efforts = eco
            .get("reasoningEfforts")
            .and_then(Value::as_object)
            .expect("eco 应只声明经保守确认的档位");
        assert_eq!(eco_efforts.len(), 1, "eco 先只声明 off，待实测再补");
        assert_eq!(eco_efforts.get("off").and_then(Value::as_str), Some("none"));

        let _ = std::fs::remove_dir_all(&base);
    }

    fn test_config() -> EngineConfig {
        EngineConfig {
            python: "/usr/bin/python3".to_string(),
            engine_dir: PathBuf::from("/data/engine"),
            dsh_cli: PathBuf::from("/data/dsh/bin.js"),
            dsh_home: PathBuf::from("/data/dsh-home"),
            agents_home: PathBuf::from("/data/agents-home"),
            result_dir: PathBuf::from("/data/engine-state/results"),
            engine_state_dir: PathBuf::from("/data/engine-state"),
            profile: "web".to_string(),
            channel: "web-api".to_string(),
            patch_files: vec![PathBuf::from("/data/config/omnidesk-model.yml")],
        }
    }
}
