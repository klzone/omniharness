//! OmniDesk 桌面宿主（Tauri 2）——IPC-1 服务端。
//!
//! 分层（用户 Plan §本阶段禁止 第 13 条）：
//!   React/TypeScript  只做 UI；
//!   Rust（本层）      只做桌面宿主、进程生命周期、IPC；
//!   Python Engine     做 OmniDesk AI 工作逻辑；
//!   DSH               只作为底层执行引擎。
//!
//! 本层把 IPC-2（Rust ↔ Python）归一化结果，再以 Tauri 命令/事件的形式交给 UI（IPC-1）。
//! UI 永远看不到 ACP / DSH 内部字段。

pub mod app_paths;
pub mod engine_bridge;
pub mod engine_ipc;
pub mod engine_supervisor;
pub mod sha256;
pub mod task_shell;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager, State};

use app_paths::AppPaths;
use engine_bridge::{EngineBridge, EventSink};
use engine_supervisor::{EngineConfig, EngineSupervisor};
use task_shell::{SaveArtifactRequest, UsageRecordInput};

/// UI 事件通道名。UI 只订阅这一个通道，拿到的都是归一化事件。
const TASK_EVENT_CHANNEL: &str = "omni://task-event";

struct HostState {
    paths: AppPaths,
    bridge: EngineBridge,
}

type SharedHost = Arc<Mutex<HostState>>;

fn bridge_sink(app: tauri::AppHandle) -> EventSink {
    Arc::new(move |event: serde_json::Value| {
        // 归一化事件原样转发；UI 侧负责按 seq 去重与排序。
        let _ = app.emit(TASK_EVENT_CHANNEL, event);
    })
}

/// 读取引擎描述（会确保子进程已启动并完成 initialize）。
///
/// 描述符来自引擎 omni.initialize 的真实返回，本层不虚构能力表。
#[tauri::command]
fn engine_describe(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    let mut host = state.lock().unwrap();
    host.bridge.ensure_initialized(&sink)
}

#[tauri::command]
fn engine_health(state: State<'_, SharedHost>) -> serde_json::Value {
    let mut host = state.lock().unwrap();
    let root = host.paths.root.display().to_string();
    host.bridge.health(&root)
}

#[tauri::command]
fn engine_submit_task(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    prompt_text: String,
    intent: Option<String>,
    workspace_dir: Option<String>,
    idempotency_key: Option<String>,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    let mut host = state.lock().unwrap();
    host.bridge.ensure_initialized(&sink)?;
    let mut params = serde_json::json!({
        "prompt_text": prompt_text,
        "intent": intent.unwrap_or_else(|| "material_writing".to_string()),
    });
    if let Some(dir) = workspace_dir {
        params["workspace_dir"] = serde_json::json!(dir);
    }
    if let Some(key) = idempotency_key {
        params["idempotency_key"] = serde_json::json!(key);
    }
    host.bridge.submit_task(params)
}

#[tauri::command]
fn engine_task_status(
    state: State<'_, SharedHost>,
    omni_task_id: String,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut host = state.lock().unwrap();
    host.bridge.get_task_status(&omni_task_id)
}

#[tauri::command]
fn engine_task_result(
    state: State<'_, SharedHost>,
    omni_task_id: String,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut host = state.lock().unwrap();
    host.bridge.get_task_result(&omni_task_id, false)
}

#[tauri::command]
fn engine_cancel_task(
    state: State<'_, SharedHost>,
    omni_task_id: String,
    timeout_seconds: Option<f64>,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut host = state.lock().unwrap();
    host.bridge
        .cancel_task(&omni_task_id, timeout_seconds.unwrap_or(15.0))
}

#[tauri::command]
fn engine_shutdown(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let mut host = state.lock().unwrap();
    host.bridge.shutdown(Duration::from_secs(10))
}

#[tauri::command]
fn app_paths_info(state: State<'_, SharedHost>) -> serde_json::Value {
    let host = state.lock().unwrap();
    let p = &host.paths;
    serde_json::json!({
        "root": p.root.display().to_string(),
        "ui_state": p.ui_state.display().to_string(),
        "logs": p.logs.display().to_string(),
        "engine_state": p.engine_state.display().to_string(),
        "results": p.results.display().to_string(),
        "config": p.config.display().to_string(),
    })
}

// ---------------------------------------------------------------------------
// 任务外壳命令（IPC-1）
//
// 这些命令只暴露 OmniDesk 自己的概念：任务类型、合成样例、确定性统计、解释提示词、
// 产物保存、使用记录。**不**暴露任何引擎 / ACP / DSH 内部字段。
//
// 数值唯一来源是 task_shell 的确定性代码；UI 不得自行计算统计值。
// ---------------------------------------------------------------------------

/// 列出已登记任务类型（含 reserved 边界，但只有 implemented 可执行）。
#[tauri::command]
fn task_types() -> serde_json::Value {
    serde_json::json!({
        "shell_version": task_shell::SHELL_VERSION,
        "analysis_schema": task_shell::ANALYSIS_SCHEMA,
        "types": task_shell::task_types(),
    })
}

/// 返回编译期嵌入的合成样例（纯虚构数据）。
#[tauri::command]
fn task_samples() -> serde_json::Value {
    serde_json::json!({ "samples": task_shell::samples() })
}

/// 返回解释阶段规则正文（只读展示，帮助使用者了解约束）。
#[tauri::command]
fn task_explanation_rules() -> serde_json::Value {
    serde_json::json!({ "rules": task_shell::explanation_rules() })
}

/// 本地数据根信息（产物与使用记录的落盘位置）。
#[tauri::command]
fn task_local_root_info(state: State<'_, SharedHost>) -> serde_json::Value {
    let host = state.lock().unwrap();
    let app_root = host.paths.root.display().to_string();
    let usage_path = host.paths.logs.join("usage.jsonl").display().to_string();
    match task_shell::resolve_local_data_root() {
        Ok(root) => serde_json::json!({
            "resolved": true,
            "artifact_root": root.display().to_string(),
            "usage_log": usage_path,
            "app_data_root": app_root,
            "error": null,
        }),
        Err(message) => serde_json::json!({
            "resolved": false,
            "artifact_root": null,
            "usage_log": usage_path,
            "app_data_root": app_root,
            "error": message,
        }),
    }
}

/// 运行确定性统计（本地代码，不联网、不调用模型）。
///
/// 返回统计结果与红线扫描命中；两者都**不回显命中原文**。
///
/// 先过统一输入闸门：命中红线扫描时不返回统计结果（与解释提示词、保存入口同一道闸门）。
/// 扫描为启发式，仅降低误贴概率，不构成合规判断或完整个人信息检测。
#[tauri::command]
fn task_run_analysis(input_text: String) -> serde_json::Value {
    let hits = task_shell::red_line_scan(&input_text);
    match task_shell::analyze_input(&input_text) {
        Ok(result) => serde_json::json!({
            "ok": true,
            "result": result,
            "red_line_hits": hits,
            "error": null,
        }),
        Err(message) => serde_json::json!({
            "ok": false,
            "result": null,
            "red_line_hits": hits,
            "error": message,
        }),
    }
}

/// 构造模型解释提示词。内含**冻结统计块**，并在本地复核指纹。
#[tauri::command]
fn task_explanation_prompt(
    task_type: String,
    input_text: String,
    fingerprint: String,
) -> Result<serde_json::Value, String> {
    task_shell::build_explanation_prompt(&task_type, &input_text, &fingerprint)
        .map(|prompt| serde_json::json!({ "prompt": prompt }))
}

/// 保存产物到仓库外本地数据目录（不覆盖已有文件）。
#[tauri::command]
fn task_save_artifact(request: SaveArtifactRequest) -> Result<serde_json::Value, String> {
    let now = task_shell::now_secs();
    task_shell::save_artifact(&request, now).map(|receipt| serde_json::json!(receipt))
}

/// 追加一条轻量使用记录（不含输入输出、不含自由文本）。
#[tauri::command]
fn task_usage_record(
    state: State<'_, SharedHost>,
    input: UsageRecordInput,
) -> Result<serde_json::Value, String> {
    let path = {
        let host = state.lock().unwrap();
        host.paths.logs.join("usage.jsonl")
    };
    let record = task_shell::build_usage_record(&input, task_shell::now_secs())?;
    task_shell::append_usage_record(&path, &record)?;
    serde_json::to_value(record).map_err(|e| e.to_string())
}

/// 查看使用记录（本地可读、可删）。
#[tauri::command]
fn task_usage_view(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let path = {
        let host = state.lock().unwrap();
        host.paths.logs.join("usage.jsonl")
    };
    task_shell::read_usage_records(&path).and_then(|r| serde_json::to_value(r).map_err(|e| e.to_string()))
}

/// 删除单条使用记录。
#[tauri::command]
fn task_usage_delete(state: State<'_, SharedHost>, record_id: String) -> Result<serde_json::Value, String> {
    let path = {
        let host = state.lock().unwrap();
        host.paths.logs.join("usage.jsonl")
    };
    task_shell::delete_usage_record(&path, &record_id)
        .map(|deleted| serde_json::json!({ "deleted": deleted }))
}

/// 清空全部使用记录。
#[tauri::command]
fn task_usage_clear(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let path = {
        let host = state.lock().unwrap();
        host.paths.logs.join("usage.jsonl")
    };
    task_shell::clear_usage_records(&path).map(|had_file| serde_json::json!({ "cleared": had_file }))
}

/// 构建宿主状态。任何边界校验失败都在这里**硬失败**，不带可疑路径启动。
fn build_state() -> Result<HostState, String> {
    let paths = AppPaths::resolve().map_err(|e| e.to_string())?;
    paths
        .ensure()
        .map_err(|e| format!("创建应用数据目录失败（{}）：{}", paths.root.display(), e))?;
    let config = EngineConfig::from_env(&paths.root)?;
    let supervisor = EngineSupervisor::new(config);
    Ok(HostState {
        bridge: EngineBridge::new(Arc::new(Mutex::new(supervisor))),
        paths,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = match build_state() {
        Ok(state) => state,
        Err(message) => {
            // 边界校验失败：给出明确原因并退出，而不是继续运行。
            eprintln!("[OmniDesk][致命] {}", message);
            std::process::exit(2);
        }
    };

    let shared: SharedHost = Arc::new(Mutex::new(state));

    tauri::Builder::default()
        .manage(Arc::clone(&shared))
        .invoke_handler(tauri::generate_handler![
            engine_describe,
            engine_health,
            engine_submit_task,
            engine_task_status,
            engine_task_result,
            engine_cancel_task,
            engine_shutdown,
            app_paths_info,
            task_types,
            task_samples,
            task_explanation_rules,
            task_local_root_info,
            task_run_analysis,
            task_explanation_prompt,
            task_save_artifact,
            task_usage_record,
            task_usage_view,
            task_usage_delete,
            task_usage_clear,
        ])
        .setup(|app| {
            // 启动时只设置窗口标题，不拉起引擎（引擎按需启动）。
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("OmniDesk Discipline");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // 窗口关闭即收尾引擎，不留孤儿进程。
                if let Some(state) = window.app_handle().try_state::<SharedHost>() {
                    if let Ok(mut host) = state.lock() {
                        let _ = host.bridge.shutdown(Duration::from_secs(5));
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("启动 OmniDesk 桌面应用失败");
}
