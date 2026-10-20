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

pub mod admission;
pub mod app_paths;
pub mod engine_bridge;
pub mod engine_ipc;
pub mod engine_lifecycle;
pub mod engine_supervisor;
pub mod flow_pack;
pub mod model_routes;
pub mod route_classifier;
pub mod settings;
pub mod sha256;
pub mod task_shell;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager, State};

use app_paths::AppPaths;
use engine_bridge::{EngineBridge, EventSink};
use engine_lifecycle::EngineLifecycle;
use engine_supervisor::{EngineConfig, EngineSupervisor};
use task_shell::{SaveArtifactRequest, UsageRecordInput};

/// UI 事件通道名。UI 只订阅这一个通道，拿到的都是归一化事件。
const TASK_EVENT_CHANNEL: &str = "omni://task-event";

struct HostState {
    paths: AppPaths,
    bridge: EngineBridge,
    /// OD-ARCH-023：引擎调用与模型权限变更的串行化边界。
    ///
    /// 所有可能启动/触达引擎的命令都必须先取得调用守卫，并在同一守卫内完成
    /// 「准入 → 启动引擎 → 动作」；权限变更则独占同一把锁。这样就不存在
    /// “先取旧准入结果、撤权完成后再启动引擎”的窗口。
    lifecycle: Arc<EngineLifecycle>,
}

type SharedHost = Arc<Mutex<HostState>>;

fn bridge_sink(app: tauri::AppHandle) -> EventSink {
    Arc::new(move |event: serde_json::Value| {
        // 归一化事件原样转发；UI 侧负责按 seq 去重与排序。
        let _ = app.emit(TASK_EVENT_CHANNEL, event);
    })
}

/// 当前生效的引擎配置与设置目录（只读快照），用于触达引擎前的准入判定。
fn admission_context(state: &SharedHost) -> Result<(EngineConfig, std::path::PathBuf), serde_json::Value> {
    let host = state.lock().unwrap();
    let config = host.bridge.supervisor().lock().unwrap().config().clone();
    Ok((config, host.paths.config.clone()))
}

/// 屏障拒绝时的统一结构（OD-ARCH-023）。
fn engine_call_blocked(reason: &str) -> serde_json::Value {
    serde_json::json!({
        "engine_error_code": "permission_change_blocked",
        "message": format!(
            "模型权限变更未完成（{}）。为避免用变更前的权限处理新输入，宿主已拒绝一切新的引擎调用。请在设置中重试该权限变更，或先停止引擎。",
            reason
        ),
        "detail": { "reason": "permission_change_blocked", "enforced_by": "host" },
    })
}

/// 在**引擎调用守卫**内完成「准入 → 启动引擎 → 动作」（OD-ARCH-023）。
///
/// 关键：准入判定与引擎启动在同一临界区内完成，且凭据被盖上当前权限世代。
/// 守卫存活期间 [`apply_model_permission_change`] 无法开始，因此不存在
/// “先取得旧准入结果、撤权完成后再启动引擎”的时序。
fn with_engine_call<F>(
    sink: &EventSink,
    state: &SharedHost,
    refusal: &str,
    act: F,
) -> Result<serde_json::Value, serde_json::Value>
where
    F: FnOnce(
        &mut HostState,
        &admission::AdmissionGrant,
        serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value>,
{
    let lifecycle = {
        let host = state.lock().unwrap();
        Arc::clone(&host.lifecycle)
    };
    // 屏障期间直接拒绝；守卫存活期间权限变更无法开始。
    let gate = lifecycle
        .begin_call()
        .map_err(|reason| engine_call_blocked(&reason))?;

    let (config, config_dir) = admission_context(state)?;
    let admitted = admission::check(&config, &config_dir, refusal)
        .map_err(admission_denied)?
        .stamp_epoch(gate.epoch());
    // 非重入校验：守卫已持有互斥锁，直接读它持有的世代，**不得**再调用
    // lifecycle.is_current()（会自锁 —— OD-ARCH-024 修复）。
    if gate.current_epoch() != admitted.grant().epoch() {
        return Err(engine_call_blocked("权限世代已变化"));
    }

    let mut host = state.lock().unwrap();
    let descriptor = host.bridge.ensure_initialized(sink, admitted.grant())?;
    act(&mut host, admitted.grant(), descriptor)
}

/// 只做准入、**不启动引擎**的受守卫调用（OD-ARCH-023）。
///
/// 用于「有副作用但不启动引擎」的入口（例如提交前准备隔离工作目录）：
/// 同样必须在生命周期守卫内完成，避免与权限变更交错。
fn with_call_gate<F>(
    state: &SharedHost,
    refusal: &str,
    act: F,
) -> Result<serde_json::Value, serde_json::Value>
where
    F: FnOnce(&admission::AdmissionGrant) -> Result<serde_json::Value, serde_json::Value>,
{
    let lifecycle = {
        let host = state.lock().unwrap();
        Arc::clone(&host.lifecycle)
    };
    let gate = lifecycle
        .begin_call()
        .map_err(|reason| engine_call_blocked(&reason))?;
    let (config, config_dir) = admission_context(state)?;
    let admitted = admission::check(&config, &config_dir, refusal)
        .map_err(admission_denied)?
        .stamp_epoch(gate.epoch());
    // 非重入校验（同 with_engine_call）：不得在守卫内再次加锁。
    if gate.current_epoch() != admitted.grant().epoch() {
        return Err(engine_call_blocked("权限世代已变化"));
    }
    act(admitted.grant())
}

/// 停止引擎（OD-ARCH-024）。
///
/// 这是“任务命令 vs 关闭”唯一并发边界的落地点，顺序与锁范围都是关键：
///   1. 取出 supervisor 句柄（宿主锁**只在此短暂持有**）；
///   2. 在 **supervisor 锁内原子地** `begin_stop()`：置位“正在关闭”并摘除通道。
///      任务命令的「判定 + 发送」也在这把锁内（`EngineBridge::rpc` →
///      `EngineSupervisor::with_channel`），因此不存在“检查通过后被关闭抢先”的
///      窗口：命令要么在关闭前把请求发出去，要么在锁内看到“关闭中”并**立即**
///      得到“未发送”的明确结论。
///   3. 耗时的协议关闭与进程收尾（`finish_stop`）在**不持有任何锁**的情况下执行，
///      状态/结果/取消因此不会被排队到关闭结束；
///      耗时上界为 `grace + 5s`（协议关闭）+ `grace`（进程终止），当前 `grace = 10s`
///      时单次最长约 25 秒，但这段等待**不占用宿主锁与 supervisor 锁**。
///   4. 收尾：重置 bridge 的“已初始化”状态，并清除“正在关闭”。
fn stop_engine(
    state: &SharedHost,
    grace: Duration,
) -> Result<serde_json::Value, String> {
    let supervisor = {
        let host = state.lock().unwrap();
        host.bridge.supervisor()
    };

    // 临界区：与任务命令的“判定 + 发送”互斥（锁内只做两件 O(µs) 的事）。
    let detached = {
        let mut guard = supervisor.lock().unwrap();
        guard.begin_stop()
    };

    let outcome = match detached {
        None => Ok((false, None)),
        Some(channel) => EngineSupervisor::finish_stop(channel, grace),
    };

    // 通道已被摘除：无论关闭成功与否，旧引擎都不再可用，如实重置自身状态。
    {
        let mut host = state.lock().unwrap();
        host.bridge.mark_stopped();
    }
    {
        let mut guard = supervisor.lock().unwrap();
        guard.end_stop();
    }

    match outcome {
        Ok((forced, exit_code)) => Ok(serde_json::json!({
            "forced_terminate": forced,
            "process_exit_code": exit_code,
            "note": if forced { "超出优雅等待，已强制终止" } else { "引擎已优雅退出" },
        })),
        Err(message) => Err(message),
    }
}

/// 窗口销毁时的引擎收尾（OD-ARCH-024 第四轮：统一关闭入口）。
///
/// 生产窗口销毁回调**只能**经本函数收尾引擎，不自行持有 `SharedHost` 锁等待关闭：
/// 它复用与命令触发停止相同的 `stop_engine()`——宿主锁只在取 supervisor 句柄与
/// “进入关闭”时短暂持有，耗时的协议关闭与进程收尾在**锁外**完成。因此窗口销毁期间
/// 状态/结果/取消仍能立刻取得宿主锁，并按既定语义快速返回（关闭前送达，或“关闭中、
/// 未发送”），不会被排队到关闭结束。
fn on_window_destroyed_shutdown(state: &SharedHost) {
    let _ = stop_engine(state, Duration::from_secs(5));
}

/// 准入拒绝时的统一结构（与 admission.rs 的错误码一致）。
fn admission_denied(err: serde_json::Value) -> serde_json::Value {
    err
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
    // 先准入、后触达宿主：拒绝时不得启动子进程或初始化引擎。
    // 准入与启动在同一守卫内完成（OD-ARCH-023）。
    with_engine_call(
        &sink,
        state.inner(),
        admission::DESCRIBE_REFUSAL_MESSAGE,
        |_host, _grant, descriptor| Ok(descriptor),
    )
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
    user_input_text: Option<String>,
    engine_session_id: Option<String>,
) -> Result<serde_json::Value, serde_json::Value> {
    // 提交链路的固定顺序：红线扫描 → 端点与授权准入 → 才触达引擎。
    // 红线拒绝发生在任何引擎动作之前，且不回显命中原文或输入内容。
    //
    // 红线扫描必须针对用户原始输入，而不是拼好之后的完整提示词：
    // 提示词里的写作规则/解释规则本身包含「案件」「线索」「未公开」等
    // 边界说明用词，扫描全文会把每一次正常提交都误判为禁入内容。
    // 调用方未提供用户原文时（旧调用方），退回扫描完整提示词。
    let scan_target = task_shell::scan_target(&prompt_text, user_input_text.as_deref());
    if let Err(message) = task_shell::assert_input_allowed(scan_target) {
        return Err(serde_json::json!({
            "engine_error_code": "red_line_rejected",
            "message": message,
            "detail": { "reason": "red_line_rejected", "enforced_by": "host" },
        }));
    }
    let sink = bridge_sink(app);
    // 准入判定与引擎启动在同一守卫内完成；屏障由守卫本身表达（OD-ARCH-023）。
    with_engine_call(
        &sink,
        state.inner(),
        admission::SUBMIT_REFUSAL_MESSAGE,
        move |host, _grant, _descriptor| {
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
            // OD-ARCH-034：带上既有引擎会话号即续接同一会话（真多轮）；
            // 不带就是新建会话。宿主只做透传，不解析、不猜测会话归属。
            if let Some(sid) = engine_session_id {
                let sid = sid.trim();
                if !sid.is_empty() {
                    params["engine_session_id"] = serde_json::json!(sid);
                }
            }
            host.bridge.submit_task(params)
        },
    )
}

// ---------------------------------------------------------------------------
// 状态 / 结果 / 取消：生命周期语义（OD-ARCH-024）
//
// 这三个命令**既不会启动引擎、也不会创建引擎资源**，只是对已存在的任务做查询或收尾。
// 因此它们**不取得 EngineLifecycle 调用守卫**，也**不检查屏障**：
//   * 若取得守卫，权限变更（`begin_change` 会一直等到没有守卫）就会把取消/查询
//     一起堵住；计划明确要求“避免权限变更等待任务结束时把取消永久堵住”。
//   * 屏障只用于阻断“启动/触达引擎”的新调用；在此处拒绝反而会让用户在引擎可能
//     仍在运行时无法读取状态或取消。
// 它们只共用宿主互斥锁；关闭路径**不在这把锁上等待**（见 `stop_engine`：
// 只有取 supervisor 句柄与“进入关闭”两次 O(µs) 的加锁，耗时的关闭收尾在锁外进行）。
// “关闭中 vs 发送”的判定放在 supervisor 锁内原子完成，因此既不会把状态/结果/取消
// 排队拖住，也不存在“检查通过后被关闭抢先”的窗口。
// ---------------------------------------------------------------------------

/// 查询任务状态（只读；不启动引擎，不受权限屏障阻断）。
///
/// 关闭进行中时由**锁内的并发边界**决定语义：`stop_engine` 在 supervisor 锁内
/// 置位“正在关闭”并摘除通道，本命令在同一把锁内判定，因此只会得到两种结果之一：
/// 关闭前送达，或立即返回 `engine_shutting_down`（明确“未发送”），绝不会先通过
/// 判定、再被关闭抢先而排队到关闭结束。
fn task_status_impl(
    state: &SharedHost,
    omni_task_id: &str,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut host = state.lock().unwrap();
    host.bridge.get_task_status(omni_task_id)
}

/// 读取任务结果（只读；语义同 `task_status_impl`）。
fn task_result_impl(
    state: &SharedHost,
    omni_task_id: &str,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut host = state.lock().unwrap();
    host.bridge.get_task_result(omni_task_id, false)
}

/// 请求取消任务（收尾动作；不启动引擎，不受权限屏障阻断）。
///
/// **不排队**：关闭进行中时立即返回 `engine_shutting_down`，消息明确“请求未发送”，
/// 避免用户以为取消已送达。判定与发送在 supervisor 锁内原子完成（见 `stop_engine`）。
fn cancel_task_impl(
    state: &SharedHost,
    omni_task_id: &str,
    timeout_seconds: f64,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut host = state.lock().unwrap();
    host.bridge.cancel_task(omni_task_id, timeout_seconds)
}

#[tauri::command]
fn engine_task_status(
    state: State<'_, SharedHost>,
    omni_task_id: String,
) -> Result<serde_json::Value, serde_json::Value> {
    task_status_impl(state.inner(), &omni_task_id)
}

#[tauri::command]
fn engine_task_result(
    state: State<'_, SharedHost>,
    omni_task_id: String,
) -> Result<serde_json::Value, serde_json::Value> {
    task_result_impl(state.inner(), &omni_task_id)
}

#[tauri::command]
fn engine_cancel_task(
    state: State<'_, SharedHost>,
    omni_task_id: String,
    timeout_seconds: Option<f64>,
) -> Result<serde_json::Value, serde_json::Value> {
    cancel_task_impl(state.inner(), &omni_task_id, timeout_seconds.unwrap_or(15.0))
}

#[tauri::command]
fn engine_shutdown(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let lifecycle = {
        let host = state.lock().unwrap();
        Arc::clone(&host.lifecycle)
    };
    // 独占生命周期：等在途调用结束后才开始，期间拒绝新的引擎调用（OD-ARCH-023）。
    let change = lifecycle.begin_change();
    let result = stop_engine(state.inner(), Duration::from_secs(10));
    match result {
        Ok(value) => {
            // 确认停止：清除屏障（权限变更失败后的恢复路径之一）。
            change.succeed();
            Ok(value)
        }
        Err(message) => {
            // 未能确认停止：保持失败关闭。
            change.fail(message.clone());
            Err(message)
        }
    }
}

/// 会话清单：需要引擎启动，因此与描述命令共用同一准入判定。
#[tauri::command]
fn engine_list_sessions(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    cwd: Option<String>,
    after_cursor: Option<String>,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    with_engine_call(
        &sink,
        state.inner(),
        admission::DESCRIBE_REFUSAL_MESSAGE,
        move |host, _grant, _descriptor| {
            host.bridge
                .list_sessions(cwd.as_deref(), after_cursor.as_deref())
        },
    )
}

/// 重开既有会话：恢复会话不等于恢复任务，也不会重放任何轮次。
#[tauri::command]
fn engine_recover_session(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    engine_session_id: String,
    workspace_token: Option<String>,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    with_engine_call(
        &sink,
        state.inner(),
        admission::DESCRIBE_REFUSAL_MESSAGE,
        move |host, _grant, _descriptor| {
            host.bridge
                .recover_session(&engine_session_id, workspace_token.as_deref())
        },
    )
}

/// 任务台账（只读元数据）。台账在 OmniDesk 自己的数据根内，不接触 DSH 数据。
#[tauri::command]
fn engine_task_records(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    limit: Option<u32>,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    with_engine_call(
        &sink,
        state.inner(),
        admission::DESCRIBE_REFUSAL_MESSAGE,
        move |host, _grant, _descriptor| host.bridge.list_task_records(limit.unwrap_or(100)),
    )
}

/// 重启后恢复任务状态：只重建可证实状态，不自动重放。
#[tauri::command]
fn engine_recover_task(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    omni_task_id: String,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    with_engine_call(
        &sink,
        state.inner(),
        admission::DESCRIBE_REFUSAL_MESSAGE,
        move |host, _grant, _descriptor| host.bridge.recover_task(&omni_task_id),
    )
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

/// 模型端点与授权现状（只读，不启动引擎）。
///
/// 即使准入被拒绝也照常返回：用户必须先看到“端点在哪里、是否离开本机”，
/// 才能决定是否开启。分类失败同样如实返回错误，不伪报为“本机”。
///
/// **异步执行（OD-ARCH-041）：** 端点分类要起一个 node 进程（~1-2 秒）。
/// 同步命令会占住 Tauri 主线程，界面在它返回前无法重绘——场景切换时就会表现为
/// 整页卡住。改成 async 后这段等待发生在异步运行时上，界面照常可交互。
#[tauri::command]
async fn engine_admission_status(app: tauri::AppHandle) -> serde_json::Value {
    // OD-ARCH-023：屏障状态取自生命周期守卫；先释放宿主锁再读，避免与
    // 「生命周期 → 宿主」的加锁顺序形成死锁。
    //
    // OD-ARCH-041：异步命令不能借用 `State<'_, _>`（future 必须是 'static），
    // 所以改用 AppHandle 取一次状态、立刻克隆出需要的值并放掉锁，
    // 后面那段 node 分类的等待不占任何宿主锁。
    let (lifecycle, config, config_dir) = {
        let host = app.state::<SharedHost>();
        let host = host.lock().unwrap();
        (
            Arc::clone(&host.lifecycle),
            host.bridge.supervisor().lock().unwrap().config().clone(),
            host.paths.config.clone(),
        )
    };
    let blocked = lifecycle.blocked_reason();
    match admission::read_facts(&config, &config_dir) {
        Ok(facts) => serde_json::json!({
            "ok": true,
            "facts": admission::describe_facts(&facts),
            "error": null,
            "blocked": blocked.is_some(),
            "block_reason": blocked,
        }),
        Err(err) => serde_json::json!({
            "ok": false,
            "facts": null,
            "error": err,
            "blocked": blocked.is_some(),
            "block_reason": blocked,
        }),
    }
}

/// 权限变更的公共实现（OD-ARCH-023）。
///
/// 顺序与并发语义：
///   1. 先取得**权限变更守卫**：它会阻塞到在途的引擎调用结束，并在整个变更期间
///      置位屏障（新的引擎调用一律被拒绝）。因此不存在“变更与旧调用交错”。
///   2. 持久化新权限（用户意图必须落盘）。
///   3. 停止旧引擎；**只有确认已停止**才报告变更生效并清除屏障。
///   4. 停止失败则失败关闭：保留屏障、拒绝一切新的引擎调用，并给出可恢复路径。
fn apply_model_permission_change(
    state: &State<'_, SharedHost>,
    persist: impl FnOnce(&std::path::Path) -> Result<settings::ModelPermissions, String>,
) -> Result<serde_json::Value, serde_json::Value> {
    let lifecycle = {
        let host = state.lock().unwrap();
        Arc::clone(&host.lifecycle)
    };
    // 独占生命周期；失败路径由 succeed()/fail() 决定屏障是否保留。
    let change = lifecycle.begin_change();

    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };

    let permissions = match persist(&config_dir) {
        Ok(p) => p,
        Err(message) => {
            // 持久化失败：权限未被改变，解除变更状态并如实报错（不假报成功）。
            change.succeed();
            return Err(serde_json::json!({
                "engine_error_code": "settings_write_failed",
                "message": message,
                "detail": { "reason": "settings_write_failed", "enforced_by": "host" },
            }));
        }
    };

    let payload = serde_json::json!({
        "local": permissions.local,
        "private": permissions.private,
        "public": permissions.public,
    });

    // 关闭期间不持有宿主锁与 supervisor 锁（见 stop_engine）：取消/状态不会被排队拖到关闭之后。
    let stop = stop_engine(state, Duration::from_secs(10)).map(|_| ());

    match stop {
        Ok(()) => {
            change.succeed();
            Ok(serde_json::json!({
                "ok": true,
                "permissions": payload,
                "engine_stopped": true,
                "blocked": false,
                "note": "旧引擎已停止并确认不再接收任务；新权限自下次提交起生效。",
            }))
        }
        Err(message) => {
            change.fail(message.clone());
            Err(serde_json::json!({
                "engine_error_code": "permission_change_incomplete",
                "message": format!(
                    "权限已保存，但未能确认旧引擎已停止（{}）。为避免旧引擎继续按变更前的权限接收任务，宿主已拒绝一切新的引擎调用。请在设置中重试该权限变更，或先停止引擎；确认停止后会自动恢复。",
                    message
                ),
                "detail": {
                    "reason": "permission_change_incomplete",
                    "enforced_by": "host",
                    "permissions": payload,
                    "engine_stopped": false,
                    "blocked": true,
                    "block_reason": message,
                },
            }))
        }
    }
}

/// 设置某一端点类别的模型权限（失败关闭，见上）。
#[tauri::command]
fn model_permission_set(
    state: State<'_, SharedHost>,
    category: String,
    enabled: bool,
) -> Result<serde_json::Value, serde_json::Value> {
    apply_model_permission_change(&state, |config_dir| {
        settings::set_model_permission(config_dir, &category, enabled)
    })
}

/// 一次性撤销全部模型权限（失败关闭，见上）。
#[tauri::command]
fn model_permission_revoke_all(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, serde_json::Value> {
    apply_model_permission_change(&state, |config_dir| {
        settings::revoke_all_model_permissions(config_dir)
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
    let config_dir = host.paths.config.clone();
    let settings_path = settings::settings_path(&config_dir).display().to_string();
    match settings::effective_root(&config_dir) {
        Ok(eff) => serde_json::json!({
            "resolved": true,
            "artifact_root": eff.root.display().to_string(),
            "source": eff.source,
            "configured_root": eff.configured_root.as_ref().map(|p| p.display().to_string()),
            "env_override": eff.env_override.as_ref().map(|p| p.display().to_string()),
            "settings_path": settings_path,
            "usage_log": usage_path,
            "app_data_root": app_root,
            "error": null,
        }),
        Err(message) => serde_json::json!({
            "resolved": false,
            "artifact_root": null,
            "source": null,
            "configured_root": null,
            "env_override": null,
            "settings_path": settings_path,
            "usage_log": usage_path,
            "app_data_root": app_root,
            "error": message,
        }),
    }
}

/// 保存用户选择的成果目录。只影响之后新写入的成果：不迁移、不覆盖、不删除旧文件。
///
/// 校验顺序：绝对路径 → 仓库/DSH/应用数据根边界 → 创建目录 → 再次校验真实落点
/// → 一次性写入探针（写完即删）→ 原子替换设置文件。任一步失败都不写入设置。
#[tauri::command]
fn task_set_artifact_root(
    state: State<'_, SharedHost>,
    path: String,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = settings::set_artifact_root(&paths, &path)?;
    Ok(serde_json::json!({
        "ok": true,
        "artifact_root": root.display().to_string(),
        "note": "仅影响之后新建的成果；既有文件不会移动、覆盖或删除。",
    }))
}

/// 在系统文件管理器中打开当前成果目录（只读入口）。
///
/// 不拼接 shell 命令：只执行固定的 `xdg-open` / `gio open`，目录作为单独参数传入，
/// 且必须通过与保存同一条边界校验。找不到可用的打开器时如实返回错误。
#[tauri::command]
fn task_open_artifact_root(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let eff = settings::effective_root(&config_dir)?;
    task_shell::assert_local_root_allowed(&eff.root)?;
    task_shell::assert_outside_repo(&eff.root)?;
    if !eff.root.is_dir() {
        return Err(format!(
            "成果目录尚不存在：{}（保存第一份成果后会自动创建）",
            eff.root.display()
        ));
    }
    let mut last_error = String::new();
    for (program, args) in [
        ("xdg-open", Vec::<String>::new()),
        ("gio", vec!["open".to_string()]),
    ] {
        match std::process::Command::new(program)
            .args(&args)
            .arg(&eff.root)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(serde_json::json!({ "ok": true, "opened": eff.root.display().to_string() })),
            Err(e) => last_error = format!("{}：{}", program, e),
        }
    }
    Err(format!("无法打开目录，系统缺少可用的打开器（{}）", last_error))
}

/// 拉起已安装的 DSH Desktop（模型/凭据设置界面）。
///
/// 固定动作、**无参数**：前端只能请求「打开 DSH 设置」，不能传命令、路径或 URL。
/// 宿主不拼接 shell、不读不写不打印任何凭证；只把设置界面拉起来，
/// 并把 `DSH_HOME` 指向**引擎正在用的那份** DSH 数据根——否则用户在那里登记的模型配置
/// 落在另一处，OmniDesk 读不到（这正是实测到的问题）。
#[tauri::command]
fn task_open_dsh_settings(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let (dsh_home, agents_home, dsh_cli) = match engine_supervisor::EngineConfig::from_env(&app_root) {
        Ok(cfg) => (cfg.dsh_home, cfg.agents_home, cfg.dsh_cli),
        Err(err) => {
            return Err(format!(
                "无法确定引擎使用的 DSH 数据根（{}）；为避免打开另一份配置，本次不启动 DSH。",
                err
            ))
        }
    };
    let log_path = {
        let host = state.lock().unwrap();
        host.paths.logs.join("dsh-settings.log")
    };
    task_shell::open_dsh_settings(&dsh_home, &agents_home, &dsh_cli, &log_path)
}

/// 列出各 DSH Profile 生效的 provider / 模型（只取非机密字段；不读凭证值）。
///
/// 供工作台的模型下拉使用：**不同 Profile 的模型选择互不相通**（只有凭据共享），
/// 这里把各 Profile 的候选都列出来，由用户选一个。
///
/// **异步执行（OD-ARCH-041）：** 列出候选同样要起 node 进程（~1-2 秒）；
/// 同步执行会占住主线程，把场景切换卡成整页无响应。改成 async 后不阻塞界面。
#[tauri::command]
async fn task_list_model_routes(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    // OD-ARCH-041：同 engine_admission_status——异步命令用 AppHandle 取状态。
    let app_root = {
        let host = app.state::<SharedHost>();
        let host = host.lock().unwrap();
        host.paths.root.clone()
    };
    let config = engine_supervisor::EngineConfig::from_env(&app_root)?;
    let profiles = model_routes::discover_profiles(&config.dsh_home, &config.profile);
    let mut result = model_routes::list_routes(&config, &profiles)?;
    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "engine_profile".to_string(),
            serde_json::Value::String(config.profile.clone()),
        );
        obj.insert(
            "overlay".to_string(),
            serde_json::Value::String(
                model_routes::overlay_path(&app_root).display().to_string(),
            ),
        );
    }
    Ok(result)
}

/// 选定模型：把该 Profile 的 provider 配置 + 选中的模型写成 OmniDesk 自己的覆盖文件，
/// 然后停掉引擎（下次任务按新路由重启）并重新判定端点。
///
/// 只写 OmniDesk 数据根下的那一个文件（只含非机密字段），**不碰用户的 DSH Profile**；
/// key 仍由共享凭据服务按 provider 的 `apiKeyEnv` 提供。
#[tauri::command]
fn task_set_model_route(
    state: State<'_, SharedHost>,
    source_profile: String,
    provider: String,
    model: String,
    reasoning_effort: Option<String>,
) -> Result<serde_json::Value, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let config = engine_supervisor::EngineConfig::from_env(&app_root)?;
    let overlay = model_routes::overlay_path(&app_root);
    let written = model_routes::select_route(
        &config,
        &overlay,
        &source_profile,
        &provider,
        &model,
        reasoning_effort.as_deref(),
    )?;

    // 引擎已在跑时旧路由仍在生效：先停掉它，下次任务会按新覆盖文件重启。
    let lifecycle = {
        let host = state.lock().unwrap();
        Arc::clone(&host.lifecycle)
    };
    let change = lifecycle.begin_change();
    match stop_engine(state.inner(), Duration::from_secs(10)) {
        Ok(_) => {
            change.succeed();
        }
        Err(message) => {
            change.fail(message.clone());
            return Err(format!(
                "模型路由已写入，但未能确认引擎停止（{}）；请到设置页手动停止引擎后再试。",
                message
            ));
        }
    }

    // 用新的覆盖文件重新判定端点（配置里已包含该覆盖）。
    let config = engine_supervisor::EngineConfig::from_env(&app_root)?;
    let facts = route_classifier::classify(&config)?;
    Ok(serde_json::json!({
        "ok": true,
        "written": written,
        "route": facts,
        "note": "已切换模型路由：引擎与端点判定现在用同一份路由（OmniDesk 数据根下的覆盖文件，只含非机密字段）。",
    }))
}

/// 读取（必要时首次创建）离线流程包。
///
/// 不创建模型会话、不启动引擎；只读一个仓库外的纯文本 JSON。
#[tauri::command]
fn offline_flow_pack(state: State<'_, SharedHost>) -> serde_json::Value {
    let config = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    match flow_pack::load_pack(&config) {
        Ok(loaded) => serde_json::json!({
            "ok": true,
            "pack": loaded.pack,
            "path": loaded.path,
            "created": loaded.created,
            "error": null,
        }),
        Err(message) => serde_json::json!({
            "ok": false,
            "pack": null,
            "path": flow_pack::pack_path(&config).display().to_string(),
            "created": false,
            "error": message,
        }),
    }
}

/// 幂等准备引擎任务的隔离工作目录，并返回其绝对路径。
///
/// 目录取「本地数据根」（与产物落点同一处，边界校验完全一致，界面阶段 5 已展示该路径），
/// 因此不引入用户看不见的新目录。引擎要求工作目录已存在且可解析，这是调用引擎前的必要一步；
/// 只创建空目录本身，不写入内容，也不新增任何下发给引擎的环境变量。
#[tauri::command]
fn task_prepare_workspace_dir(state: State<'_, SharedHost>) -> Result<serde_json::Value, serde_json::Value> {
    // 先准入、后创建目录：拒绝时不得有任何文件系统副作用。
    // OD-ARCH-023：准入与目录创建在生命周期守卫内完成，权限变更无法插入。
    with_call_gate(state.inner(), admission::WORKSPACE_REFUSAL_MESSAGE, |_grant| {
        prepare_workspace_dir()
    })
}

/// 准入通过后创建并返回隔离工作目录。
///
/// 目录取「本地数据根」（与产物落点同一处，边界校验完全一致，界面阶段 5 已展示该路径），
/// 因此不引入用户看不见的新目录。引擎要求工作目录已存在且可解析，这是调用引擎前的必要一步；
/// 只创建空目录本身，不写入内容，也不新增任何下发给引擎的环境变量。
fn prepare_workspace_dir() -> Result<serde_json::Value, serde_json::Value> {
    match task_shell::prepare_local_data_root() {
        Ok(root) => Ok(serde_json::json!({
            "ok": true,
            "workspace_dir": root.display().to_string(),
            "error": null,
        })),
        Err(message) => Ok(serde_json::json!({
            "ok": false,
            "workspace_dir": null,
            "error": message,
        })),
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

// ---------------------------------------------------------------------------
// 离线工作场景（合成数据，不调用模型）
//
// 三个场景只做确定性校验、统计和 Markdown 渲染。模型撰写步骤一律返回
//「受准入阻塞、未验证」，不得在 UI 上被表述成已生成。
// ---------------------------------------------------------------------------

/// 编译期嵌入的纯虚拟流程样例。
#[tauri::command]
fn workflow_samples() -> serde_json::Value {
    serde_json::json!({
        "samples": [
            {
                "sample_id": "weekly-basic",
                "title": "合成样例 · 一周事项记录",
                "note": "四条虚构事项，含 done / pending / blocked；无真实人员与机构。",
                "content": include_str!("../../../samples/workflows/synthetic-weekly.txt"),
            },
            {
                "sample_id": "writing-basic",
                "title": "合成样例 · 一般文案要素",
                "note": "虚构目的、受众、要求与事实；不含真实单位与人员，且不提供效率数据。",
                "content": include_str!("../../../samples/workflows/synthetic-writing.txt"),
            },
            {
                "sample_id": "project-basic",
                "title": "合成样例 · 项目跟进清单",
                "note": "四条虚构事项，负责人只用 P1/P2/P3 代号；含逾期与受阻项。",
                "content": include_str!("../../../samples/workflows/synthetic-project.txt"),
            },
        ]
    })
}

/// 读取当前流程包；失败时命令必须整次拒绝，不得悄悄用默认文案冒充成功。
fn current_flow_pack(state: &State<'_, SharedHost>) -> Result<flow_pack::LoadedPack, String> {
    let config = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    flow_pack::load_pack(&config)
}

fn pack_refusal(message: String) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "pack_error": true,
        "model_step": task_shell::MODEL_STEP_BLOCKED,
        "error": message,
    })
}

/// 周记录 → 周报（本地确定性汇总）。
#[tauri::command]
fn task_weekly_summary(state: State<'_, SharedHost>, input_text: String) -> serde_json::Value {
    let loaded = match current_flow_pack(&state) {
        Ok(loaded) => loaded,
        Err(message) => return pack_refusal(message),
    };
    serde_json::to_value(task_shell::weekly_flow_with(&input_text, &loaded.pack)).unwrap_or_else(
        |e| serde_json::json!({ "ok": false, "error": format!("序列化结果失败：{}", e) }),
    )
}

/// 一般文案 → 结构建议（缺信息时只列待确认，不强行套模板）。
#[tauri::command]
fn task_writing_plan(
    state: State<'_, SharedHost>,
    purpose: String,
    audience: String,
    requirements: String,
    facts: String,
) -> serde_json::Value {
    let loaded = match current_flow_pack(&state) {
        Ok(loaded) => loaded,
        Err(message) => return pack_refusal(message),
    };
    serde_json::to_value(task_shell::writing_flow_with(
        &purpose,
        &audience,
        &requirements,
        &facts,
        &loaded.pack,
    ))
    .unwrap_or_else(|e| {
        serde_json::json!({ "ok": false, "error": format!("序列化结果失败：{}", e) })
    })
}

/// 项目跟进 → 汇总（含逾期判定；today 留空用 UTC 当天）。
#[tauri::command]
fn task_project_summary(
    state: State<'_, SharedHost>,
    input_text: String,
    today: Option<String>,
) -> serde_json::Value {
    let loaded = match current_flow_pack(&state) {
        Ok(loaded) => loaded,
        Err(message) => return pack_refusal(message),
    };
    serde_json::to_value(task_shell::project_flow_with(
        &input_text,
        today.as_deref(),
        &loaded.pack,
    ))
    .unwrap_or_else(|e| serde_json::json!({ "ok": false, "error": format!("序列化结果失败：{}", e) }))
}

/// 保存离线场景成果（不覆盖已有文件，必须人工确认）。
#[tauri::command]
fn task_save_offline(request: task_shell::SaveOfflineRequest) -> Result<serde_json::Value, String> {
    task_shell::save_offline_artifact(&request, task_shell::now_secs())
        .map(|receipt| serde_json::json!(receipt))
}

/// 列出已保存的离线成果（只读元数据，仓库外数据根）。
#[tauri::command]
fn task_list_offline_artifacts() -> Result<serde_json::Value, String> {
    task_shell::list_offline_artifacts().map(|items| serde_json::json!({ "items": items }))
}

/// 重新打开一条已保存的离线成果（只读，不修改原文件）。
#[tauri::command]
fn task_read_offline_artifact(file_name: String) -> Result<serde_json::Value, String> {
    task_shell::read_offline_artifact(&file_name)
        .map(|item| serde_json::json!(item))
}

/// 只读读取用户**明确给出路径**的一个材料文件（OD-ARCH-026）。
///
/// 一次一个文件，不列目录、不递归、不解析数据库；绝对路径、普通文件、
/// 扩展名白名单、大小上限与 UTF-8 全部由 task_shell::read_material 强制。
#[tauri::command]
fn task_read_material(path: String) -> Result<serde_json::Value, String> {
    task_shell::read_material(&path).map(|item| serde_json::json!(item))
}

#[cfg(test)]
mod command_tests {
    use super::*;

    /// 直接调用宿主命令（不经 UI）：端点无法确认或权限关闭时必须拒绝，且不创建任何目录。
    /// OD-ARCH-023：准备目录的入口同样受守卫保护——屏障生效期间必须被拒绝，
    /// 且不得产生任何文件系统副作用。
    #[test]
    fn prepare_workspace_is_refused_while_permission_change_blocked() {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-prepare-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&base);
        // 指向一个尚不存在的本地数据根；若入口越过屏障，就会把它建出来。
        std::env::set_var("OMNIDESK_LOCAL_DATA_ROOT", &base);

        let life = EngineLifecycle::new();
        life.begin_change()
            .fail("子进程未能在 10 秒内退出".to_string());

        let blocked = life
            .begin_call()
            .err()
            .expect("屏障生效期间，准备目录入口必须被拒绝");
        assert!(blocked.contains("子进程"), "拒绝原因应保留：{}", blocked);
        assert!(!base.exists(), "拒绝路径不得创建目录：{}", base.display());

        // 恢复后才重新开放。
        life.begin_change().succeed();
        assert!(life.begin_call().is_ok(), "确认停止并恢复后才重新开放");

        std::env::remove_var("OMNIDESK_LOCAL_DATA_ROOT");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// 离线命令本身不需要引擎，必须能在准入关闭时正常完成。
    #[test]
    fn offline_commands_do_not_need_engine_admission() {
        let pack = flow_pack::default_pack();
        let weekly = task_shell::weekly_flow_with(
            "2026-10-12 | 合成事项 | done\n2026-10-13 | 合成事项二 | pending",
            &pack,
        );
        let weekly = serde_json::to_value(weekly).unwrap();
        assert_eq!(weekly["ok"], true);
        assert!(weekly["model_step"].as_str().unwrap_or_default().contains("受准入阻塞"));

        let writing = serde_json::to_value(task_shell::writing_flow_with(
            "说明合成流程",
            "评审人",
            "",
            "",
            &pack,
        ))
        .unwrap();
        assert_eq!(writing["ok"], true);
        assert_eq!(writing["plan"]["template_used"], serde_json::Value::Null);

        let project = serde_json::to_value(task_shell::project_flow_with(
            "合成事项 | P1 | 2026-10-10 | in_progress | 补齐说明",
            Some("2026-10-15"),
            &pack,
        ))
        .unwrap();
        assert_eq!(project["ok"], true);
        assert_eq!(project["summary"]["overdue"], 1);
    }
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
        lifecycle: Arc::new(EngineLifecycle::new()),
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
            engine_list_sessions,
            engine_recover_session,
            engine_task_records,
            engine_recover_task,
            engine_admission_status,
            model_permission_set,
            model_permission_revoke_all,
            app_paths_info,
            task_types,
            task_samples,
            task_explanation_rules,
            task_local_root_info,
            task_set_artifact_root,
            task_open_artifact_root,
            task_open_dsh_settings,
            task_list_model_routes,
            task_set_model_route,
            task_prepare_workspace_dir,
            task_run_analysis,
            task_explanation_prompt,
            task_save_artifact,
            task_usage_record,
            task_usage_view,
            task_usage_delete,
            task_usage_clear,
            offline_flow_pack,
            workflow_samples,
            task_weekly_summary,
            task_writing_plan,
            task_project_summary,
            task_save_offline,
            task_list_offline_artifacts,
            task_read_offline_artifact,
            task_read_material,
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
                //
                // OD-ARCH-024 第四轮：**不得**在这里自行 `state.lock()` 后等待关闭——
                // 那条路径会持有整个宿主锁直到关闭结束，把并发的状态/结果/取消命令
                // 一起排队堵住。统一走共享入口，其关闭等待不占用宿主锁。
                if let Some(state) = window.app_handle().try_state::<SharedHost>() {
                    on_window_destroyed_shutdown(state.inner());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("启动 OmniDesk 桌面应用失败");
}

#[cfg(test)]
mod host_wrapper_tests {
    //! OD-ARCH-024：宿主包装器的组合路径测试。
    //!
    //! 用**真实**的 `with_engine_call` / `with_call_gate`（含真实路由分类与真实宿主锁）
    //! 配合**假引擎**，验证：
    //!   * 包装器正常返回、不自锁（旧实现会在守卫内重入同一把互斥锁而永久等待）；
    //!   * 准入到动作的闭环；
    //!   * 调用在途时权限变更不能开始（互斥）；
    //!   * 屏障生效期间包装器被拒绝、状态/结果/取消仍可用且不被堵住。
    //!
    //! 所有会启动假引擎的测试都在断言之前先收尾，避免子进程残留阻塞测试框架。

    use super::*;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("仓库根")
            .to_path_buf()
    }

    fn dsh_cli() -> PathBuf {
        repo_root().join("runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js")
    }

    struct Fixture {
        base: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-hostwrapper-{}-{}",
                tag,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&base);
            for dir in [
                "app/ui-state",
                "app/logs",
                "app/engine-state/results",
                "config",
                "dsh-home",
                "agents",
            ] {
                std::fs::create_dir_all(base.join(dir)).expect("建夹具目录");
            }
            // 合成 provider 覆盖层：端点为本机回环地址 → 分类为 local。
            std::fs::write(
                base.join("overlay.yml"),
                [
                    "- id: acp",
                    "  name: \"@deepseek-ai/dsh-acp\"",
                    "  config:",
                    "    provider: synthetic",
                    "    model: synthetic-model",
                    "- id: llm-pi-ai",
                    "  name: \"@deepseek-ai/dsh-llm-pi-ai\"",
                    "  config:",
                    "    providers:",
                    "      synthetic:",
                    "        apiKeyEnv: OMNIDESK_SYNTHETIC_KEY",
                    "        api: openai-completions",
                    "        baseURL: http://127.0.0.1:9/v1",
                    "        models:",
                    "          - id: synthetic-model",
                    "            name: synthetic-model",
                ]
                .join("\n")
                    + "\n",
            )
            .expect("写覆盖层");
            // 设置：仅允许本机类别。
            std::fs::write(
                base.join("config/settings.v1.json"),
                r#"{"schema":"omnidesk.settings.v1","artifact_root":null,"model_permissions":{"local":true,"private":false,"public":false}}"#,
            )
            .expect("写设置");
            Self { base }
        }

        fn paths(&self) -> AppPaths {
            AppPaths {
                root: self.base.join("app"),
                ui_state: self.base.join("app/ui-state"),
                logs: self.base.join("app/logs"),
                engine_state: self.base.join("app/engine-state"),
                results: self.base.join("app/engine-state/results"),
                config: self.base.join("config"),
            }
        }

        fn config(&self) -> EngineConfig {
            EngineConfig {
                python: std::env::var("OMNIDESK_PYTHON").unwrap_or_else(|_| "python3".to_string()),
                engine_dir: repo_root().join("desktop/src-tauri/tests/fixtures/fake_engine"),
                dsh_cli: dsh_cli(),
                dsh_home: self.base.join("dsh-home"),
                agents_home: self.base.join("agents"),
                engine_state_dir: self.base.join("app/engine-state"),
                result_dir: self.base.join("app/engine-state/results"),
                profile: "acp".to_string(),
                patch_files: vec![self.base.join("overlay.yml")],
            }
        }

        /// 真实宿主状态：真实锁与生命周期，引擎通道换成假引擎。
        fn host(&self) -> SharedHost {
            self.host_with_env(&[])
        }

        /// 同上，但给假引擎追加环境变量（方法日志 / 关闭延迟）。
        fn host_with_env(&self, extra_env: &[(String, String)]) -> SharedHost {
            let supervisor =
                engine_supervisor::supervisor_with_fake_channel_env(self.config(), extra_env);
            Arc::new(Mutex::new(HostState {
                paths: self.paths(),
                bridge: EngineBridge::new(Arc::new(Mutex::new(supervisor))),
                lifecycle: Arc::new(EngineLifecycle::new()),
            }))
        }

        /// 假引擎收到的方法日志（需启用 `OMNI_TEST_METHOD_LOG`）。
        fn method_log(&self) -> Vec<String> {
            std::fs::read_to_string(self.base.join("methods.log"))
                .unwrap_or_default()
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        }

        /// 等到假引擎收到某个方法（用于确定性地等待关闭进入协议阶段）。
        fn wait_for_method(&self, name: &str, timeout: Duration) -> bool {
            let deadline = std::time::Instant::now() + timeout;
            while std::time::Instant::now() < deadline {
                if self.method_log().iter().any(|m| m == name) {
                    return true;
                }
                thread::sleep(Duration::from_millis(10));
            }
            false
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn noop_sink() -> EventSink {
        Arc::new(|_| {})
    }

    /// 断言某命令在“关闭中”返回的结论是明确且一致的（OD-ARCH-024）：
    /// 错误码 `engine_shutting_down`、消息说明“未发送”、且标记请求没有发给引擎。
    fn assert_shutting_down_refusal(
        label: &str,
        attempt: Result<serde_json::Value, serde_json::Value>,
    ) {
        let err = attempt.err().unwrap_or_else(|| panic!("{} 应明确失败", label));
        assert_eq!(
            err["engine_error_code"], "engine_shutting_down",
            "{} 应返回 engine_shutting_down：{}",
            label, err
        );
        let message = err["message"].as_str().unwrap_or_default();
        assert!(
            message.contains("未发送"),
            "{} 的消息须明确说明未发送：{}",
            label,
            message
        );
        assert_eq!(
            err["detail"]["engine_detail"]["sent_to_engine"], false,
            "{} 必须标明请求没有发送给引擎：{}",
            label, err
        );
    }

    /// 描述包装器：准入 → 启动（假引擎）→ 动作，必须返回而不是自锁。
    #[test]
    fn describe_wrapper_returns_without_self_lock() {
        if !dsh_cli().exists() {
            eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
            return;
        }
        let fx = Fixture::new("describe");
        let host = fx.host();
        let sink = noop_sink();
        let (tx, rx) = mpsc::channel();
        let h = Arc::clone(&host);
        let worker = thread::spawn(move || {
            let out = with_engine_call(
                &sink,
                &h,
                admission::DESCRIBE_REFUSAL_MESSAGE,
                |_host, _grant, descriptor| Ok(descriptor),
            );
            let _ = tx.send(out);
        });

        let received = rx.recv_timeout(Duration::from_secs(90));
        // 无论成功还是超时，都先收尾假引擎，避免残留子进程阻塞测试框架。
        let _ = host.lock().unwrap().bridge.shutdown(Duration::from_secs(5));
        let _ = worker.join();

        let out = received.expect("with_engine_call 自锁/超时（OD-ARCH-024 回归）");
        let value = out.expect("准入通过后应返回描述符");
        assert_eq!(value["engine_kind"], "fake-engine");
    }

    /// 非启动包装器同样必须返回、不自锁。
    #[test]
    fn call_gate_wrapper_returns_without_self_lock() {
        if !dsh_cli().exists() {
            eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
            return;
        }
        let fx = Fixture::new("gate");
        let host = fx.host();
        let (tx, rx) = mpsc::channel();
        let h = Arc::clone(&host);
        let worker = thread::spawn(move || {
            let out = with_call_gate(&h, admission::WORKSPACE_REFUSAL_MESSAGE, |_grant| {
                Ok(serde_json::json!({ "ok": true, "note": "action ran" }))
            });
            let _ = tx.send(out);
        });

        let out = rx
            .recv_timeout(Duration::from_secs(90))
            .expect("with_call_gate 自锁/超时（OD-ARCH-024 回归）");
        worker.join().unwrap();

        let value = out.expect("准入通过后应执行动作");
        assert_eq!(value["ok"], true);
        // 非启动包装器不得启动引擎。
        assert_eq!(
            host.lock().unwrap().bridge.health("/tmp/omnidesk-host-tests/app")["note"],
            "引擎子进程在运行（未初始化）"
        );
    }

    /// 调用在途（已进入动作）时，权限变更不能开始；释放后可以开始。
    #[test]
    fn in_flight_call_blocks_permission_change() {
        if !dsh_cli().exists() {
            eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
            return;
        }
        let fx = Fixture::new("inflight");
        let host = fx.host();
        // 提前取出生命周期句柄：动作执行期间宿主锁被调用方持有。
        let lifecycle = Arc::clone(&host.lock().unwrap().lifecycle);

        let (in_action_tx, in_action_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let sink = noop_sink();
        let h = Arc::clone(&host);
        let worker = thread::spawn(move || {
            with_engine_call(&sink, &h, admission::DESCRIBE_REFUSAL_MESSAGE, move |_host, _grant, _d| {
                let _ = in_action_tx.send(());
                let _ = release_rx.recv();
                Ok(serde_json::json!({ "ok": true }))
            })
        });

        let entered = in_action_rx.recv_timeout(Duration::from_secs(90));
        if entered.is_err() {
            // 未进入动作（多半是自锁）：先释放并收尾，再报错，避免残留子进程。
            let _ = release_tx.send(());
            let _ = worker.join();
            let _ = host.lock().unwrap().bridge.shutdown(Duration::from_secs(5));
            panic!("with_engine_call 未进入动作（可能自锁，OD-ARCH-024 回归）");
        }
        let blocked_change = lifecycle.try_begin_change().is_none();
        release_tx.send(()).unwrap();
        let result = worker.join().expect("调用线程应正常结束");

        // 调用结束后权限变更可以开始；先收尾引擎再看结果。
        let change = lifecycle
            .try_begin_change()
            .expect("调用结束后变更可开始");
        change.succeed();
        let _ = host.lock().unwrap().bridge.shutdown(Duration::from_secs(5));

        assert!(
            blocked_change,
            "调用在途（持守卫）时权限变更不得开始"
        );
        assert!(result.is_ok(), "包装器应返回成功：{:?}", result);
    }

    /// 屏障生效（权限变更未完成）时：包装器被拒绝，任务命令仍可用且不被堵住。
    #[test]
    fn barrier_refuses_wrappers_but_task_commands_stay_usable() {
        let fx = Fixture::new("barrier");
        let host = fx.host();
        let lifecycle = Arc::clone(&host.lock().unwrap().lifecycle);
        lifecycle
            .begin_change()
            .fail("子进程未能在 10 秒内退出".to_string());

        // 包装器：必须被拒绝，且不得触达引擎。
        let sink = noop_sink();
        let refused = with_engine_call(&sink, &host, admission::DESCRIBE_REFUSAL_MESSAGE, |_h, _g, d| {
            Ok(d)
        })
        .err()
        .expect("屏障期间包装器必须被拒绝");
        assert_eq!(refused["engine_error_code"], "permission_change_blocked");

        // 任务命令：不取生命周期守卫，不得被屏障或权限变更堵住。
        let (tx, rx) = mpsc::channel();
        let h = Arc::clone(&host);
        let worker = thread::spawn(move || {
            let codes: Vec<Option<String>> = [
                task_status_impl(&h, "omni-not-a-task").err(),
                task_result_impl(&h, "omni-not-a-task").err(),
                cancel_task_impl(&h, "omni-not-a-task", 2.0).err(),
            ]
            .into_iter()
            .map(|e| {
                e.and_then(|v| {
                    v.get("engine_error_code")
                        .and_then(|c| c.as_str())
                        .map(str::to_string)
                })
            })
            .collect();
            let _ = tx.send(codes);
        });
        let codes = rx
            .recv_timeout(Duration::from_secs(30))
            .expect("任务命令被生命周期锁堵住（OD-ARCH-024 回归）");
        worker.join().unwrap();

        for code in &codes {
            assert_ne!(
                code.as_deref(),
                Some("permission_change_blocked"),
                "状态/结果/取消不得因权限屏障被拒绝（否则取消会被不当堵住）"
            );
        }

        // 恢复后包装器重新放行。
        lifecycle.begin_change().succeed();
        assert!(lifecycle.begin_call().is_ok(), "确认停止并恢复后应重新放行");
    }

    /// OD-ARCH-024 补测 1：**真实运行中的任务**在关闭之前请求取消时，取消**送达引擎**。
    ///
    /// 不是“不存在的任务 ID + 只看错误码”：这里先通过真实包装器提交任务（假引擎建立
    /// 真实在途任务），再取消，并用假引擎的方法日志证明它确实收到了 `omni.cancel_task`。
    #[test]
    fn cancel_is_delivered_before_shutdown_for_a_running_task() {
        if !dsh_cli().exists() {
            eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
            return;
        }
        let fx = Fixture::new("cancel-delivered");
        let host = fx.host_with_env(&[(
            "OMNI_TEST_METHOD_LOG".to_string(),
            fx.base.join("methods.log").display().to_string(),
        )]);
        let sink = noop_sink();

        let receipt = with_engine_call(
            &sink,
            &host,
            admission::SUBMIT_REFUSAL_MESSAGE,
            |host, _grant, _descriptor| {
                host.bridge.submit_task(serde_json::json!({
                    "prompt_text": "合成任务：仅用于宿主并发测试",
                    "intent": "material_writing",
                }))
            },
        )
        .expect("提交应通过准入并返回回执");
        let task_id = receipt["omni_task_id"]
            .as_str()
            .expect("回执应含任务标识")
            .to_string();
        assert_eq!(receipt["status"], "queued");

        // 关闭之前取消：必须送达引擎，而不是仅返回错误码。
        let cancelled = cancel_task_impl(&host, &task_id, 5.0).expect("取消应被引擎受理");
        let outcome = (
            cancelled["final_state"].as_str().map(str::to_string),
            cancelled["phase"].as_str().map(str::to_string),
        );
        let _ = stop_engine(&host, Duration::from_secs(5));
        let methods = fx.method_log();

        assert!(
            methods.iter().any(|m| m == "omni.cancel_task"),
            "引擎应实际收到 omni.cancel_task（取消须在关闭前送达），实际方法序列：{:?}",
            methods
        );
        assert_eq!(outcome.0.as_deref(), Some("cancelled"));
        assert_eq!(outcome.1.as_deref(), Some("confirmed_cancelled"));
    }

    /// OD-ARCH-024 补测 2：关闭**已进入耗时阶段**时，取消/状态/结果必须立即给出
    /// 明确结论（“关闭中、未发送”），不得排队到关闭结束之后才普通失败。
    ///
    /// 用假引擎的可控关闭延迟制造确定性窗口；用方法日志证明关闭开始之后没有 cancel
    /// 被送达引擎。
    #[test]
    fn task_commands_return_promptly_when_shutdown_is_in_progress() {
        if !dsh_cli().exists() {
            eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
            return;
        }
        let fx = Fixture::new("cancel-shutdown");
        let host = fx.host_with_env(&[
            (
                "OMNI_TEST_METHOD_LOG".to_string(),
                fx.base.join("methods.log").display().to_string(),
            ),
            // 制造 4 秒的“关闭进行中”窗口。
            ("OMNI_TEST_SHUTDOWN_DELAY_MS".to_string(), "4000".to_string()),
        ]);
        let sink = noop_sink();

        let receipt = with_engine_call(
            &sink,
            &host,
            admission::SUBMIT_REFUSAL_MESSAGE,
            |host, _grant, _descriptor| {
                host.bridge.submit_task(serde_json::json!({
                    "prompt_text": "合成任务：仅用于宿主并发测试",
                    "intent": "material_writing",
                }))
            },
        )
        .expect("提交应通过准入并返回回执");
        let task_id = receipt["omni_task_id"].as_str().unwrap().to_string();

        // 后台开始关闭。
        let stopper = {
            let h = Arc::clone(&host);
            thread::spawn(move || stop_engine(&h, Duration::from_secs(10)))
        };

        // 等到关闭进入**协议关闭阶段**（假引擎已收到 omni.shutdown，正在延迟）。
        assert!(
            fx.wait_for_method("omni.shutdown", Duration::from_secs(10)),
            "关闭应进入协议关闭阶段，实际：{:?}",
            fx.method_log()
        );

        // 取消：必须立即返回明确结论，不得等到关闭结束。
        let started = std::time::Instant::now();
        let attempted = cancel_task_impl(&host, &task_id, 5.0);
        let waited = started.elapsed();
        let status_attempt = task_status_impl(&host, &task_id);
        let result_attempt = task_result_impl(&host, &task_id);

        let stopped = stopper.join().expect("关闭线程应正常结束");

        assert!(
            waited < Duration::from_millis(1500),
            "取消不得排队等待关闭（等了 {:?}）",
            waited
        );
        assert_shutting_down_refusal("取消", attempted);
        assert_shutting_down_refusal("状态", status_attempt);
        assert_shutting_down_refusal("结果", result_attempt);
        assert!(stopped.is_ok(), "关闭应成功：{:?}", stopped);

        // 关键断言：关闭开始之后，引擎没有再收到 cancel。
        let methods = fx.method_log();
        let shutdown_at = methods.iter().position(|m| m == "omni.shutdown");
        assert!(shutdown_at.is_some(), "应记录到 omni.shutdown：{:?}", methods);
        assert!(
            !methods[shutdown_at.unwrap()..]
                .iter()
                .any(|m| m == "omni.cancel_task"),
            "取消不得在关闭开始之后才送达引擎：{:?}",
            methods
        );
    }

    /// OD-ARCH-024 补测 3（**确定性时序同步点**）：让关闭先取得 supervisor 锁。
    ///
    /// 复现「请求已进入发送路径、尚未取得 supervisor 锁」这一确切间隙：先用假引擎
    /// 建立真实在途任务，再让关闭流程完成 `begin_stop()`（置位“正在关闭”并摘除通道）
    /// 并进入耗时阶段；此时把取消线程放行到该同步点上，它随后才尝试取 supervisor 锁。
    /// 断言：取消**未送达**引擎、且**立即**返回明确的“关闭中、未发送”，而不是等
    /// 关闭结束后才普通失败；状态/结果同样如此。
    #[test]
    fn task_commands_are_refused_when_shutdown_wins_the_engine_lock() {
        if !dsh_cli().exists() {
            eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
            return;
        }
        use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

        let fx = Fixture::new("shutdown-wins-lock");
        let host = fx.host_with_env(&[
            (
                "OMNI_TEST_METHOD_LOG".to_string(),
                fx.base.join("methods.log").display().to_string(),
            ),
            // 关闭窗口足够长，保证“关闭先取锁”的时序有充足余量。
            ("OMNI_TEST_SHUTDOWN_DELAY_MS".to_string(), "8000".to_string()),
        ]);
        let sink = noop_sink();

        let receipt = with_engine_call(
            &sink,
            &host,
            admission::SUBMIT_REFUSAL_MESSAGE,
            |host, _grant, _descriptor| {
                host.bridge.submit_task(serde_json::json!({
                    "prompt_text": "合成任务：仅用于宿主并发测试",
                    "intent": "material_writing",
                }))
            },
        )
        .expect("提交应通过准入并返回回执");
        let task_id = receipt["omni_task_id"].as_str().unwrap().to_string();

        // 同步点：请求进入发送路径后先停在这里（尚未取得 supervisor 锁），
        // 只拦第一个请求（取消），其余请求直接放行。
        let (entered_tx, entered_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let armed = Arc::new(AtomicBool::new(true));
        let gate: Arc<dyn Fn() + Send + Sync> = {
            let release_rx = Arc::clone(&release_rx);
            let armed = Arc::clone(&armed);
            Arc::new(move || {
                if armed.swap(false, AtomicOrdering::SeqCst) {
                    let _ = entered_tx.send(());
                    let _ = release_rx.lock().unwrap().recv();
                }
            })
        };
        host.lock().unwrap().bridge.set_pre_lock_hook(Some(gate));

        // 1) 关闭流程先走：取宿主锁（短暂）→ 锁内 begin_stop → 进入耗时阶段。
        let stopper = {
            let h = Arc::clone(&host);
            thread::spawn(move || stop_engine(&h, Duration::from_secs(10)))
        };
        assert!(
            fx.wait_for_method("omni.shutdown", Duration::from_secs(10)),
            "关闭应进入协议关闭阶段，实际：{:?}",
            fx.method_log()
        );

        // 2) 现在放行被拦住的取消：它才去尝试 supervisor 锁（此时关闭已先持过该锁）。
        let canceller = {
            let h = Arc::clone(&host);
            let task_id = task_id.clone();
            thread::spawn(move || cancel_task_impl(&h, &task_id, 5.0))
        };
        entered_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("取消应停在“已进入发送路径、尚未取 supervisor 锁”的同步点");

        let started = std::time::Instant::now();
        release_tx.send(()).expect("放行取消");
        let attempted = canceller.join().expect("取消线程应正常结束");
        let waited = started.elapsed();

        // 状态/结果：同步点只拦第一个请求，这里直接验证即时拒绝。
        let status_attempt = task_status_impl(&host, &task_id);
        let result_attempt = task_result_impl(&host, &task_id);

        let stopped = stopper.join().expect("关闭线程应正常结束");
        host.lock().unwrap().bridge.set_pre_lock_hook(None);

        assert!(
            waited < Duration::from_millis(1500),
            "关闭先取得引擎锁时，取消也不得排队等待（等了 {:?}）",
            waited
        );
        assert_shutting_down_refusal("取消", attempted);
        assert_shutting_down_refusal("状态", status_attempt);
        assert_shutting_down_refusal("结果", result_attempt);
        assert!(stopped.is_ok(), "关闭应成功：{:?}", stopped);

        // 未送达：关闭开始之后引擎没有收到过任何 cancel。
        let methods = fx.method_log();
        let shutdown_at = methods
            .iter()
            .position(|m| m == "omni.shutdown")
            .unwrap_or_else(|| panic!("应记录到 omni.shutdown：{:?}", methods));
        assert!(
            !methods[shutdown_at..].iter().any(|m| m == "omni.cancel_task"),
            "取消既不能送达，也不能在关闭开始之后才送达：{:?}",
            methods
        );
    }

    /// OD-ARCH-024 第四轮：**窗口销毁关闭路径**必须与命令触发的停止共享同一语义
    /// （不在持有宿主锁时等待关闭）。
    ///
    /// 窗口对象无法在单元测试里构造，因此这里直接调用生产回调实际使用的入口
    /// `on_window_destroyed_shutdown()`，验证计划的验收标准：
    ///   1. 关闭等待期间**宿主锁仍可立刻取得**（旧实现会持锁到关闭结束）；
    ///   2. 状态/结果/取消按既定语义**快速返回**（“关闭中、未发送”），不排队；
    ///   3. 关闭开始后引擎不再收到取消。
    #[test]
    fn window_destroy_shutdown_does_not_hold_host_lock_while_waiting() {
        if !dsh_cli().exists() {
            eprintln!("跳过：本机没有仓库内 DSH CLI（{}）", dsh_cli().display());
            return;
        }
        let fx = Fixture::new("window-destroy");
        let host = fx.host_with_env(&[
            (
                "OMNI_TEST_METHOD_LOG".to_string(),
                fx.base.join("methods.log").display().to_string(),
            ),
            // 关闭窗口足够长：旧实现会在这段时间里一直持有宿主锁。
            ("OMNI_TEST_SHUTDOWN_DELAY_MS".to_string(), "8000".to_string()),
        ]);
        let sink = noop_sink();

        let receipt = with_engine_call(
            &sink,
            &host,
            admission::SUBMIT_REFUSAL_MESSAGE,
            |host, _grant, _descriptor| {
                host.bridge.submit_task(serde_json::json!({
                    "prompt_text": "合成任务：仅用于宿主并发测试",
                    "intent": "material_writing",
                }))
            },
        )
        .expect("提交应通过准入并返回回执");
        let task_id = receipt["omni_task_id"].as_str().unwrap().to_string();

        // 后台执行“窗口销毁”收尾（与生产回调同一入口）。
        let closer = {
            let h = Arc::clone(&host);
            thread::spawn(move || on_window_destroyed_shutdown(&h))
        };
        assert!(
            fx.wait_for_method("omni.shutdown", Duration::from_secs(10)),
            "窗口销毁收尾应进入协议关闭阶段，实际：{:?}",
            fx.method_log()
        );

        // 验收 1：关闭等待期间宿主锁仍可立刻取得。
        let started = std::time::Instant::now();
        {
            let guard = host.lock().unwrap();
            drop(guard);
        }
        let lock_wait = started.elapsed();
        assert!(
            lock_wait < Duration::from_millis(1500),
            "关闭等待期间宿主锁应可立刻取得（等了 {:?}）",
            lock_wait
        );

        // 验收 2：状态/结果/取消按既定语义快速返回，不排队到关闭结束。
        let started = std::time::Instant::now();
        let attempted = cancel_task_impl(&host, &task_id, 5.0);
        let waited = started.elapsed();
        let status_attempt = task_status_impl(&host, &task_id);
        let result_attempt = task_result_impl(&host, &task_id);

        closer.join().expect("窗口销毁线程应正常结束");

        assert!(
            waited < Duration::from_millis(1500),
            "窗口销毁期间取消不得排队等待关闭（等了 {:?}）",
            waited
        );
        assert_shutting_down_refusal("取消", attempted);
        assert_shutting_down_refusal("状态", status_attempt);
        assert_shutting_down_refusal("结果", result_attempt);

        // 验收 3：关闭开始后引擎不再收到取消；且收尾确实完成（不再报告引擎在运行）。
        let methods = fx.method_log();
        let shutdown_at = methods
            .iter()
            .position(|m| m == "omni.shutdown")
            .unwrap_or_else(|| panic!("应记录到 omni.shutdown：{:?}", methods));
        assert!(
            !methods[shutdown_at..].iter().any(|m| m == "omni.cancel_task"),
            "窗口销毁期间不得把取消送进正在关闭的引擎：{:?}",
            methods
        );
        let health = host.lock().unwrap().bridge.health("test");
        assert_eq!(
            health["running"],
            serde_json::json!(false),
            "窗口销毁收尾后不得再报告引擎在运行：{}",
            health
        );
    }
}

#[cfg(test)]
mod engine_lifecycle_wiring_tests {
    /// OD-ARCH-024 源码级接线检查。
    ///
    /// 这里直接扫描本文件源码，防止后续新增入口绕过生命周期边界：
    ///   1. 六个引擎启动入口都必须经 `with_engine_call`；
    ///   2. 有副作用但不启动引擎的目录准备入口必须经 `with_call_gate`；
    ///   3. 权限变更与显式停止必须取得 `begin_change()` 独占守卫；
    ///   4. 状态/结果/取消三个命令必须**不**取得生命周期守卫、不得在锁外预检关闭标志，
    ///      并复用 `*_impl` 的同一实现（否则权限变更会把取消一起堵住，或出现
    ///      “检查通过后被关闭抢先”的窗口）；
    ///   5. 关闭例程必须在 supervisor 锁内原子进入关闭、在锁外做耗时收尾；
    ///      supervisor 的 `with_channel` 必须在锁内给出“关闭中、未发送”；
    ///   6. 准入判定只允许出现在两个守卫辅助函数内部（防止锁外取准入结果）；
    ///   7. 旧机制的 `permission_barrier` 字段不得残留。
    #[test]
    fn engine_starting_commands_go_through_lifecycle_gate() {
        let src = include_str!("lib.rs");
        // 排除全部测试模块，避免匹配到测试里的字符串。
        let body = src.split("\n#[cfg(test)]").next().expect("源码前缀");

        for cmd in [
            "fn engine_describe",
            "fn engine_submit_task",
            "fn engine_list_sessions",
            "fn engine_recover_session",
            "fn engine_task_records",
            "fn engine_recover_task",
        ] {
            let start = body
                .find(cmd)
                .unwrap_or_else(|| panic!("找不到宿主命令 {}", cmd));
            let rest = &body[start..];
            let end = rest[1..]
                .find("\n#[tauri::command]")
                .map(|i| i + 1)
                .unwrap_or(rest.len());
            assert!(
                rest[..end].contains("with_engine_call("),
                "{} 必须经 with_engine_call 触达引擎（禁止锁外取准入结果后再启动）",
                cmd
            );
        }

        // 非提交但有副作用/触达引擎准备工作的入口也必须受守卫保护。
        {
            let start = body
                .find("fn task_prepare_workspace_dir")
                .expect("找不到 task_prepare_workspace_dir");
            let rest = &body[start..];
            let end = rest[1..]
                .find("\n/// ")
                .map(|i| i + 1)
                .unwrap_or(rest.len());
            assert!(
                rest[..end].contains("with_call_gate("),
                "task_prepare_workspace_dir 必须在生命周期守卫内完成准入与副作用"
            );
        }

        for cmd in ["fn apply_model_permission_change", "fn engine_shutdown"] {
            let start = body
                .find(cmd)
                .unwrap_or_else(|| panic!("找不到 {}", cmd));
            let rest = &body[start..];
            let end = rest[1..].find("\nfn ").map(|i| i + 1).unwrap_or(rest.len());
            assert!(
                rest[..end].contains("begin_change()"),
                "{} 必须取得权限变更独占守卫",
                cmd
            );
        }

        // 状态 / 结果 / 取消：按 OD-ARCH-024 定义，**不**取得生命周期守卫
        // （否则权限变更会把取消一起堵住），并复用 *_impl 的同一实现。
        for cmd in [
            "fn engine_task_status",
            "fn engine_task_result",
            "fn engine_cancel_task",
        ] {
            let start = body
                .find(cmd)
                .unwrap_or_else(|| panic!("找不到宿主命令 {}", cmd));
            let rest = &body[start..];
            let end = rest[1..]
                .find("\n#[tauri::command]")
                .map(|i| i + 1)
                .unwrap_or(rest.len());
            let segment = &rest[..end];
            assert!(
                segment.contains("_impl("),
                "{} 必须复用 *_impl 实现（保证测试覆盖的组合路径即生产路径）",
                cmd
            );
            assert!(
                !segment.contains("begin_call(") && !segment.contains("with_engine_call("),
                "{} 不得取得生命周期守卫：权限变更期间取消/查询必须仍然可用",
                cmd
            );
        }

        // 三个 *_impl：判定必须在**锁内**（由 bridge/supervisor 统一完成），
        // 不得在锁外先预检标志——那正是“检查通过后被关闭抢先”的窗口。
        for imp in ["fn task_status_impl", "fn task_result_impl", "fn cancel_task_impl"] {
            let start = body
                .find(imp)
                .unwrap_or_else(|| panic!("找不到 {}", imp));
            let rest = &body[start..];
            let end = rest[1..].find("\nfn ").map(|i| i + 1).unwrap_or(rest.len());
            let segment = &rest[..end];
            assert!(
                !segment.contains("is_shutting_down"),
                "{} 不得在锁外预检关闭标志：判定必须与发送同处一把锁内",
                imp
            );
            assert!(
                segment.contains("bridge."),
                "{} 应直接把请求交给 bridge（由锁内边界统一判定）",
                imp
            );
        }

        // 关闭例程：锁内原子进入关闭（begin_stop），耗时收尾在锁外（finish_stop）。
        {
            let start = body.find("fn stop_engine").expect("找不到 stop_engine");
            let rest = &body[start..];
            let end = rest[1..].find("\nfn ").map(|i| i + 1).unwrap_or(rest.len());
            let segment = &rest[..end];
            let begin_at = segment
                .find("begin_stop()")
                .expect("必须用 begin_stop() 在 supervisor 锁内进入关闭");
            let finish_at = segment
                .find("finish_stop(")
                .expect("必须在锁外执行 finish_stop()");
            assert!(
                begin_at < finish_at,
                "先（锁内）摘除通道，再（锁外）做耗时收尾"
            );
            assert!(
                !segment.contains(".stop(grace)"),
                "stop_engine 不得调用会持续持锁的 stop()：关闭等待期间必须不持锁"
            );
        }

        // 单一关闭入口（第四轮）：生产代码不得直接调用 bridge.shutdown()
        // ——那会让调用方手里的宿主锁一直占着，把状态/结果/取消堵住；
        // 窗口销毁回调必须走共享入口 on_window_destroyed_shutdown()。
        //
        // 注意：生产代码分布在多个 `#[cfg(test)]` 模块之间，因此这里以 `run()` 为界
        // 取「文件开头 → run() 之后第一个测试模块」作为生产前缀。
        {
            let run_at = src.find("pub fn run()").expect("找不到 run()");
            let prefix_end = src[run_at..]
                .find("\n#[cfg(test)]")
                .map(|i| run_at + i + 1)
                .unwrap_or(src.len());
            let production = &src[..prefix_end];

            let window_at = production
                .find("on_window_event")
                .expect("找不到窗口事件回调");
            let rest = &production[window_at..];
            let end = rest[1..].find("\n    .").map(|i| i + 1).unwrap_or(rest.len());
            let callback = &rest[..end];
            // 只检查代码：注释里出现的 `state.lock()` 等字样不算违规。
            let callback_code = callback
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                callback_code.contains("on_window_destroyed_shutdown("),
                "窗口销毁回调必须走共享关闭入口"
            );
            assert!(
                !callback_code.contains("bridge.shutdown(") && !callback_code.contains("state.lock()"),
                "窗口销毁回调不得自行持宿主锁并直接调用 bridge.shutdown()"
            );
            assert!(
                !production.contains("bridge.shutdown("),
                "生产代码不得直接调用 bridge.shutdown()：必须经 stop_engine()/共享关闭入口"
            );
        }

        // supervisor 自身的并发边界：with_channel 在锁内判定 stopping；
        // begin_stop 在同一处同时置位标志并摘除通道。
        let sup_src = include_str!("engine_supervisor.rs");
        {
            let start = sup_src
                .find("pub fn with_channel")
                .expect("找不到 with_channel");
            let rest = &sup_src[start..];
            let end = rest[1..]
                .find("\n    pub fn ")
                .map(|i| i + 1)
                .unwrap_or(rest.len());
            let segment = &rest[..end];
            assert!(
                segment.contains("self.stopping"),
                "with_channel 必须在 supervisor 锁内判定“正在关闭”"
            );
            assert!(
                segment.contains("shutting_down("),
                "with_channel 必须返回明确的“关闭中、未发送”错误"
            );
        }
        {
            let start = sup_src.find("pub fn begin_stop").expect("找不到 begin_stop");
            let rest = &sup_src[start..];
            let end = rest[1..]
                .find("\n    pub fn ")
                .map(|i| i + 1)
                .unwrap_or(rest.len());
            let segment = &rest[..end];
            let flag_at = segment
                .find("self.stopping = true")
                .expect("begin_stop 必须置位“正在关闭”");
            let take_at = segment
                .find("self.channel.take()")
                .expect("begin_stop 必须摘除通道");
            assert!(
                flag_at < take_at,
                "置位“正在关闭”与摘除通道必须在同一把锁内先后完成"
            );
        }

        // 准入判定只允许出现在两个守卫辅助函数内部：
        //   * with_engine_call（准入 + 启动引擎）
        //   * with_call_gate（准入 + 非启动副作用）
        // 出现第三处即说明又有入口在守卫之外取准入结果，本测试会失败。
        assert_eq!(
            body.matches("admission::check(").count(),
            2,
            "准入判定只允许出现在 with_engine_call 与 with_call_gate 内部（防止锁外取准入结果）"
        );
        assert!(
            !body.contains("permission_barrier"),
            "旧屏障字段不得残留（并发边界已改由 EngineLifecycle 表达）"
        );
    }
}
