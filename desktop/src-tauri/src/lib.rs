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
pub mod capabilities;
pub mod dsh_settings;
pub mod engine_bridge;
pub mod engine_ipc;
pub mod engine_lifecycle;
pub mod engine_supervisor;
pub mod external_open;
pub mod flow_pack;
pub mod knowledge_vault;
mod local_http;
pub mod model_settings;
pub mod model_routes;
pub mod pending_writes;
pub mod project_items;
pub mod process_owner;
pub mod profile_decl;
pub mod profile_choice;
pub mod profile_rules;
pub mod route_classifier;
pub mod scene_overview;
pub mod scheduler;
pub mod settings;
pub mod sha256;
pub mod task_shell;
pub mod thread_store;
pub mod vault_note;
pub mod vault_link;
pub mod vault_index;
pub mod vault_open;
pub mod vault_search;
pub mod vault_write;
pub mod worklog;

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
    /// 知识库链接索引快照（OD-KB-001 P3-S04）。
    ///
    /// 只在内存：显式刷新时整体替换；读命令只读取完整快照，绝不暴露构建中的
    /// 半成品。每次构建都重新校验设置里的根，不跨根复用旧快照。
    vault_index: Arc<Mutex<Option<vault_index::LinkIndex>>>,
}

type SharedHost = Arc<Mutex<HostState>>;

/// 端点事实读取用的只读快照（OD-ARCH-049 之后的 050 修复）。
///
/// 为什么要单独放一份：引擎冷启动时 `with_engine_call` 会**持有 SharedHost 锁**
/// 直到 `initialize` 结束（最长 90 秒）。如果「读端点事实」也去抢那把锁，界面就会
/// 在这段时间里拿不到事实，表现为「模型：未识别 / 端点事实还没读出来」。
///
/// 快照里的三样东西在运行期都不变（配置只由环境变量与磁盘覆盖文件决定，覆盖文件
/// 由分类器每次现读），因此读事实不需要任何宿主锁，也不会看到过期状态。
struct HostSnapshot {
    paths: AppPaths,
    /// 引擎配置快照。覆盖文件（provider/模型）会随用户操作变化，因此这里可更新；
    /// 更新只动这份快照与 supervisor 的同一字段，不需要宿主锁。
    config: Mutex<EngineConfig>,
    lifecycle: Arc<EngineLifecycle>,
}


/// 覆盖文件变化后，同步 supervisor 与端点事实快照里的 patch 列表（OD-ARCH-050）。
///
/// 不重新解析 env、不动隔离边界；只保证两处配置看到同一份磁盘事实。
fn sync_model_overlay(app: &tauri::AppHandle, host: &SharedHost, app_root: &std::path::Path) {
    {
        let host = host.lock().unwrap();
        host.bridge
            .supervisor()
            .lock()
            .unwrap()
            .config_mut()
            .sync_overlay(app_root);
    }
    if let Some(snap) = app.try_state::<HostSnapshot>() {
        snap.config.lock().unwrap().sync_overlay(app_root);
    }
}

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
    // OD-ARCH-074：收尾时需要引擎状态目录来核验并回收遗留子进程组。
    let engine_state_dir = supervisor.lock().unwrap().config().engine_state_dir.clone();

    // 临界区：与任务命令的“判定 + 发送”互斥（锁内只做两件 O(µs) 的事）。
    let detached = {
        let mut guard = supervisor.lock().unwrap();
        guard.begin_stop()
    };

    let outcome = match detached {
        None => Ok((false, None)),
        Some(channel) => EngineSupervisor::finish_stop(channel, grace, &engine_state_dir),
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
async fn engine_describe(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    let host_state = Arc::clone(state.inner());
    // 先准入、后触达宿主：拒绝时不得启动子进程或初始化引擎。
    // 准入与启动在同一守卫内完成（OD-ARCH-023）。
    //
    // **异步执行（OD-ARCH-048 修复）：** `with_engine_call` 会拉起引擎（冷启动
    // 数秒）且会等宿主全局锁；同步命令占住 Tauri 主线程时，启动界面与场景切换
    // 都会整页卡死、只能强制退出。改到阻塞线程池执行，等待不再落在 UI 线程。
    let joined = tauri::async_runtime::spawn_blocking(move || {
        with_engine_call(
            &sink,
            &host_state,
            admission::DESCRIBE_REFUSAL_MESSAGE,
            |_host, _grant, descriptor| Ok(descriptor),
        )
    })
    .await;
    match joined {
        Ok(result) => result,
        Err(e) => Err(serde_json::json!({
            "engine_error_code": "host_task_join_failed",
            "message": format!("读取引擎描述的后台任务异常结束：{}", e),
            "detail": { "reason": "host_task_join_failed", "enforced_by": "host" },
        })),
    }
}

#[tauri::command]
async fn engine_health(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    // OD-ARCH-048 修复：只读健康快照也要等宿主全局锁。当另一条引擎命令正在
    // 冷启动/等待模型目录（最长 60s）时，同步读锁会把这段等待压到 UI 线程上，
    // 表现为切场景整页卡死。改到阻塞线程池执行。
    // 异步命令不能借用 `State<'_, _>`（future 必须是 'static），改用 AppHandle。
    let host_state = {
        let state = app.state::<SharedHost>();
        Arc::clone(state.inner())
    };
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let mut host = host_state.lock().unwrap();
        let root = host.paths.root.display().to_string();
        host.bridge.health(&root)
    })
    .await;
    joined.map_err(|e| format!("读取引擎健康状态的后台任务异常结束：{}", e))
}

#[tauri::command]
async fn engine_submit_task(
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
    // OD-ARCH-068：红线词表、隐私策略来自 Profile 规则资产（仓库外数据根）。
    // 没有规则文件 = 纯净底座（零关键词、不掩码）；文件损坏时**拒绝提交**，
    // 不静默退回纯净底座——否则等于悄悄关掉了用户以为还在的保护。
    let profiles_dir = {
        let host = state.lock().unwrap();
        host.paths.root.join("profiles")
    };
    let rules_view = profile_rules::load_rules(&profiles_dir);
    if let Some(error) = rules_view.error.clone() {
        return Err(serde_json::json!({
            "engine_error_code": "profile_rules_invalid",
            "message": format!(
                "Profile 规则文件无法读取，已拒绝提交：{}。请到设置 →「规则资产」修正或恢复默认。",
                error
            ),
            "detail": {
                "reason": "profile_rules_invalid",
                "enforced_by": "host",
                "path": rules_view.path,
            },
        }));
    }
    let policy = match rules_view.rules.as_ref() {
        Some(rules) => profile_rules::RulePolicy::from_rules(rules),
        None => profile_rules::RulePolicy::clean(),
    };
    let profile_rules_loaded = !policy.is_clean();
    let scan_target = task_shell::scan_target(&prompt_text, user_input_text.as_deref());
    if let Err(message) = task_shell::assert_input_allowed_with(scan_target, &policy) {
        return Err(serde_json::json!({
            "engine_error_code": "red_line_rejected",
            "message": message,
            "detail": { "reason": "red_line_rejected", "enforced_by": "host" },
        }));
    }
    // 隐私掩码在**出站文本**上执行：前端拼进提示词的原文同样被覆盖。
    // 本地线程/界面上保留原文，不追溯改写。
    let (prompt_text, privacy_masked) = profile_rules::mask_privacy(&prompt_text, &policy);
    // OD-ARCH-049 第 3 步：执行入口终检。
    //
    // 只有调用方**明确声明了离线场景**（周报/文案/项目）的任务才进入拦截范围；
    // 首页闲聊不声明场景（intent 缺省值只是记录用），不按场景资源拦。
    // 命中同场景未决确认卡时整条任务拒绝，错误点名是哪张卡，不做“跳过部分内容”。
    if let Some(scene) = intent
        .as_deref()
        .and_then(pending_writes::scene_for_intent)
    {
        let dir = pending_write_dir(&state);
        if let Err(message) = pending_writes::scene_gate(&dir, scene, task_shell::now_secs()) {
            return Err(serde_json::json!({
                "engine_error_code": "pending_write_blocked",
                "message": message,
                "detail": {
                    "reason": "pending_write_blocked",
                    "enforced_by": "host",
                    "scenario": scene,
                },
            }));
        }
    }
    // 2026-10-23：不再向引擎下发任何等待上限。原版 DSH 跑一次联网检索到完成
    // 需要八分钟以上，固定秒数会误杀仍在正常运行、只是没吐字的任务。
    // 现在只有用户点「取消」才终止；进程退出、断流、上游报错仍照常落定。
    let sink = bridge_sink(app.clone());
    // OD-ARCH-045 第 3 步：引擎冷启动（ensure_initialized）可能耗时数秒，
    // 放到阻塞线程池里执行，主线程与 UI 立即返回，不再冻结窗口。
    //
    // 语义不变：准入判定、引擎启动与提交仍在同一个 EngineLifecycle 调用守卫内完成，
    // 守卫覆盖整个阻塞调用；权限变更仍无法与之交错。只是“等待”从 UI 线程挪到 worker。
    let host_state = Arc::clone(state.inner());
    let refusal = admission::SUBMIT_REFUSAL_MESSAGE;
    let prompt_text = prompt_text.clone();
    let intent = intent.clone();
    let workspace_dir = workspace_dir.clone();
    let idempotency_key = idempotency_key.clone();
    let engine_session_id = engine_session_id.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        with_engine_call(
            &sink,
            &host_state,
            refusal,
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
    })
    .await;
    match joined {
        // 回执里如实标注：本次出站文本是否被掩码、是否加载了 Profile 规则。
        // 只回传类别计数，不回显原文。
        Ok(inner) => inner.map(|mut result| {
            if let Some(obj) = result.as_object_mut() {
                obj.insert("privacy_masked".to_string(), serde_json::json!(privacy_masked));
                obj.insert(
                    "profile_rules_loaded".to_string(),
                    serde_json::json!(profile_rules_loaded),
                );
            }
            result
        }),
        Err(e) => Err(serde_json::json!({
            "engine_error_code": "host_task_join_failed",
            "message": format!("启动引擎的后台任务异常结束：{}", e),
            "detail": { "reason": "host_task_join_failed", "enforced_by": "host" },
        })),
    }
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

/// 回传用户对审批请求的决定（OD-ARCH-066）。
///
/// 与取消同属收尾动作：不启动引擎；决定是否真的送达由引擎当前待审批集合判定，
/// 失败时界面保留待审批卡片，不假装已生效。
fn approval_respond_impl(
    state: &SharedHost,
    omni_task_id: &str,
    approval_id: &str,
    allow: bool,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut host = state.lock().unwrap();
    host.bridge.approval_respond(omni_task_id, approval_id, allow)
}

/// 回传用户对结构化一次性提问的答案（OD-ARCH-088）。
///
/// 与审批同属收尾动作：不启动引擎；是否送达由引擎当前待答集合判定，失败时
/// 界面保留提问卡并显示真实错误，不假装已回答。
fn question_answer_impl(
    state: &SharedHost,
    omni_task_id: &str,
    question_id: &str,
    answers: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut host = state.lock().unwrap();
    host.bridge
        .question_answer(omni_task_id, question_id, answers)
}

#[tauri::command(async)]
fn engine_task_status(
    state: State<'_, SharedHost>,
    omni_task_id: String,
) -> Result<serde_json::Value, serde_json::Value> {
    task_status_impl(state.inner(), &omni_task_id)
}

#[tauri::command(async)]
fn engine_task_result(
    state: State<'_, SharedHost>,
    omni_task_id: String,
) -> Result<serde_json::Value, serde_json::Value> {
    task_result_impl(state.inner(), &omni_task_id)
}

#[tauri::command(async)]
fn engine_cancel_task(
    state: State<'_, SharedHost>,
    omni_task_id: String,
    timeout_seconds: Option<f64>,
) -> Result<serde_json::Value, serde_json::Value> {
    cancel_task_impl(state.inner(), &omni_task_id, timeout_seconds.unwrap_or(15.0))
}

#[tauri::command(async)]
fn engine_approval_respond(
    state: State<'_, SharedHost>,
    omni_task_id: String,
    approval_id: String,
    allow: bool,
) -> Result<serde_json::Value, serde_json::Value> {
    approval_respond_impl(state.inner(), &omni_task_id, &approval_id, allow)
}

#[tauri::command(async)]
fn engine_question_answer(
    state: State<'_, SharedHost>,
    omni_task_id: String,
    question_id: String,
    answers: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    question_answer_impl(state.inner(), &omni_task_id, &question_id, answers)
}

#[tauri::command(async)]
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
#[tauri::command(async)]
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
#[tauri::command(async)]
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

/// 会话分支：调用 DSH 原生 session/fork，按本轮 turn/end 的精确序号建子会话。
/// 与其它触达引擎的命令一样走 with_engine_call（准入与权限世代校验）。
#[tauri::command(async)]
fn engine_fork_session(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    engine_session_id: String,
    at_seq: Option<u64>,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    with_engine_call(
        &sink,
        state.inner(),
        admission::DESCRIBE_REFUSAL_MESSAGE,
        move |host, _grant, _descriptor| {
            host.bridge.fork_session(&engine_session_id, at_seq)
        },
    )
}

/// 任务台账（只读元数据）。台账在 OmniDesk 自己的数据根内，不接触 DSH 数据。
#[tauri::command(async)]
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

/// 模型目录（OD-ARCH-048）：web-api 通道直读 DSH `session/modelCatalog`。
///
/// 只返回 provider/模型名/思考等级这类**非机密**目录信息；不读、不转发凭证。
/// ACP 通道不支持该能力时，引擎返回 `unsupported`，前端据此回退到 Profile 候选。
///
/// **异步执行（OD-ARCH-048 修复）：** 该调用会触达引擎（首次要冷启动 DSH web
/// 进程，RPC 超时 60s）。同步命令占住 Tauri 主线程时，首页重新挂载（场景切换）
/// 就会整页卡住、只能强制退出——与 OD-ARCH-041 同一类问题。因此改为 async +
/// `spawn_blocking`：等待发生在工作线程，准入与引擎调用仍在同一生命周期守卫内。
#[tauri::command]
async fn engine_list_models(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    refresh: Option<bool>,
) -> Result<serde_json::Value, serde_json::Value> {
    let sink = bridge_sink(app);
    let host_state = Arc::clone(state.inner());
    let refusal = admission::DESCRIBE_REFUSAL_MESSAGE;
    let refresh = refresh.unwrap_or(false);
    let joined = tauri::async_runtime::spawn_blocking(move || {
        with_engine_call(&sink, &host_state, refusal, move |host, _grant, _descriptor| {
            host.bridge.list_models(refresh)
        })
    })
    .await;
    match joined {
        Ok(result) => result,
        Err(e) => Err(serde_json::json!({
            "engine_error_code": "host_task_join_failed",
            "message": format!("读取模型目录的后台任务异常结束：{}", e),
            "detail": { "reason": "host_task_join_failed", "enforced_by": "host" },
        })),
    }
}

/// 重启后恢复任务状态：只重建可证实状态，不自动重放。
#[tauri::command(async)]
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

#[tauri::command(async)]
fn app_paths_info(state: State<'_, SharedHost>) -> serde_json::Value {
    let host = state.lock().unwrap();
    let p = &host.paths;
    serde_json::json!({
        "root": p.root.display().to_string(),
        "ui_state": p.ui_state.display().to_string(),
        "logs": p.logs.display().to_string(),
        "engine_state": p.engine_state.display().to_string(),
        "results": p.results.display().to_string(),
        "threads": p.threads.display().to_string(),
        "config": p.config.display().to_string(),
        "profiles": p.root.join("profiles").display().to_string(),
    })
}

// ---------------------------------------------------------------------------
// 纪检 Profile 声明（OD-ARCH-051 S3）
//
// 只决定 Dock 上显示哪些业务场景入口，不加载插件、不改模型路由与 Skill。
// 文件固定在应用数据根下 profiles/discipline.yml（仓库外）；首次启动创建默认声明，
// 之后不再覆盖。加载/校验失败一律返回错误，由界面显式提示并回退内置入口。
// ---------------------------------------------------------------------------

#[tauri::command(async)]
fn profile_decl_load(
    state: State<'_, SharedHost>,
) -> Result<profile_decl::LoadedDecl, String> {
    let profiles_dir = {
        let host = state.lock().unwrap();
        host.paths.root.join("profiles")
    };
    profile_decl::load_decl(&profiles_dir)
}

/// 只读：声明文件当前的内容指纹（OD-ARCH-052 第 2 步）。
///
/// 界面拿它和启动时载入的指纹比对，判断用户是否已在磁盘上改过声明——
/// 改动只在下次启动生效，因此要显式提示"重启应用生效"。
/// 这条命令不创建文件、不改配置、不启动引擎。
#[tauri::command(async)]
fn profile_decl_digest(state: State<'_, SharedHost>) -> Result<Option<String>, String> {
    let profiles_dir = {
        let host = state.lock().unwrap();
        host.paths.root.join("profiles")
    };
    profile_decl::decl_digest(&profiles_dir)
}

/// 在系统编辑器里打开场景声明文件（OD-ARCH-053 S3）。**不**接受路径参数。
#[tauri::command(async)]
fn profile_decl_open(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let profiles_dir = {
        let host = state.lock().unwrap();
        host.paths.root.join("profiles")
    };
    // 先确保文件存在（与启动时同一套 create_new 逻辑，永不覆盖）。
    let loaded = profile_decl::load_decl(&profiles_dir)?;
    let path = std::path::PathBuf::from(&loaded.path);
    let mut last_error = String::new();
    for (program, args) in [
        ("xdg-open", Vec::<String>::new()),
        ("gio", vec!["open".to_string()]),
    ] {
        match std::process::Command::new(program)
            .args(&args)
            .arg(&path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                return Ok(serde_json::json!({
                    "ok": true,
                    "opened": loaded.path,
                    "note": "已在系统编辑器打开声明文件；保存后需重启应用生效。",
                }))
            }
            Err(e) => last_error = format!("{}：{}", program, e),
        }
    }
    Err(format!("无法打开声明文件，系统缺少可用的打开器（{}）", last_error))
}

/// 保存场景声明（OD-ARCH-053 S3）：先校验，校验通过才备份并原子写。
///
/// 校验失败一律拒绝保存，文件原样保留；写前把旧文件复制成 `discipline.yml.bak-<epoch>`。
#[tauri::command(async)]
fn profile_decl_save(state: State<'_, SharedHost>, text: String) -> Result<serde_json::Value, String> {
    let profiles_dir = {
        let host = state.lock().unwrap();
        host.paths.root.join("profiles")
    };
    if text.len() > 64 * 1024 {
        return Err("声明文件过大（上限 64 KB）".to_string());
    }
    let decl = profile_decl::parse_decl(&text)?;
    decl.validate()?;
    std::fs::create_dir_all(&profiles_dir)
        .map_err(|e| format!("创建 Profile 目录失败（{}）：{}", profiles_dir.display(), e))?;
    let path = profile_decl::decl_path(&profiles_dir);
    let backup = if path.is_file() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let target = path.with_file_name(format!("{}.bak-{}", profile_decl::PROFILE_DECL_FILE, stamp));
        std::fs::copy(&path, &target)
            .map_err(|e| format!("写入前备份失败（{}）：{}", target.display(), e))?;
        Some(target.display().to_string())
    } else {
        None
    };
    model_settings::write_atomic(&path, text.as_bytes())?;
    Ok(serde_json::json!({
        "ok": true,
        "path": path.display().to_string(),
        "backup": backup,
        "digest": crate::sha256::sha256_hex(text.as_bytes()),
        "note": "声明已保存；重启应用后生效。",
    }))
}

// ---------------------------------------------------------------------------
// 引擎 Profile 选择（OD-ARCH-072 首页改版）
//
// 只决定「下一次启动引擎用哪个 Profile」。不复制 Profile、不改插件、不改模型路由。
// 选择只落一个名字到应用数据根 config/settings.v1.json；目录不存在时明确拒绝。
// ---------------------------------------------------------------------------

/// 轻量解析通道与隔离数据根：即使引擎配置不完整（缺 CLI），也要能打开弹层修正选择。
fn profile_choice_scope() -> Result<(String, std::path::PathBuf), String> {
    let channel = std::env::var("OMNIDESK_ENGINE_CHANNEL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "web-api".to_string());
    if channel != "web-api" && channel != "acp" {
        return Err(format!(
            "OMNIDESK_ENGINE_CHANNEL 只支持 web-api 或 acp，收到：{}",
            channel
        ));
    }
    let dsh_home = match std::env::var_os("OMNIDESK_DSH_HOME").filter(|v| !v.is_empty()) {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let home = std::env::var_os("HOME").ok_or_else(|| "环境变量 HOME 缺失".to_string())?;
            std::path::PathBuf::from(home)
                .join("OmniDesk")
                .join("runtime")
                .join("dsh-home")
        }
    };
    Ok((channel, dsh_home))
}

/// 只读：列出隔离数据根里真实存在的 Profile，并返回当前生效的名字。
#[tauri::command(async)]
fn profile_choice_view(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let (channel, dsh_home) = profile_choice_scope()?;
    let saved = crate::settings::load(&app_root.join("config"))
        .ok()
        .and_then(|s| s.engine_profile);
    let saved_out_of_whitelist = saved
        .as_deref()
        .map(|s| !profile_choice::is_whitelisted(&channel, s))
        .unwrap_or(false);
    // 「本次启动实际生效」只认引擎配置；引擎配置读不出来时退回按已保存选择推算，
    // 并显式带上 warning，不把推算值当成事实。
    let (effective, warning) = match engine_supervisor::EngineConfig::from_env(&app_root) {
        Ok(config) => (config.profile, None),
        Err(e) => {
            let guess = profile_choice::resolve(&dsh_home, &channel, saved.as_deref())
                .unwrap_or_else(|_| profile_choice::standard_profile(&channel).to_string());
            (guess, Some(e))
        }
    };
    Ok(serde_json::json!({
        "ok": true,
        "channel": channel,
        "standard": profile_choice::standard_profile(&channel),
        "saved": saved,
        "saved_out_of_whitelist": saved_out_of_whitelist,
        "effective": effective,
        "options": profile_choice::list(&dsh_home, &channel),
        "warning": warning,
        "note": "这里是本次启动实际使用的 Profile；改动在下次启动引擎时生效。",
    }))
}

/// 保存 Profile 选择（None = 回到标准模式）。只写设置文件，不重启引擎。
#[tauri::command(async)]
fn profile_choice_set(
    state: State<'_, SharedHost>,
    profile: Option<String>,
) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let (channel, dsh_home) = profile_choice_scope()?;
    let chosen = match profile.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(name) => {
            if !crate::settings::is_valid_profile_name(name) {
                return Err(format!("Profile 名不合法：{:?}", name));
            }
            let manifest = dsh_home.join("profiles").join(name).join("package.json");
            if !manifest.is_file() {
                return Err(format!(
                    "隔离数据根里没有这个 Profile：{}（缺少 {}）",
                    name,
                    manifest.display()
                ));
            }
            Some(name.to_string())
        }
    };
    crate::settings::set_engine_profile(&config_dir, chosen.as_deref())?;
    let effective_next =
        chosen.clone().unwrap_or_else(|| profile_choice::standard_profile(&channel).to_string());
    Ok(serde_json::json!({
        "ok": true,
        "saved": chosen,
        "effective_next_start": effective_next,
        "note": "Profile 已保存，下次启动引擎时生效；正在运行的任务不受影响。",
    }))
}

// ---------------------------------------------------------------------------
// Profile 规则资产（OD-ARCH-068 S1/S2/S4）
//
// 红线词表、写作铁律、场景规则与思考档、隐私策略都写在应用数据根下的
// `profiles/discipline.rules.json`（仓库外）。这里只做读写与校验：
//   * 文件不存在 = 纯净底座（零规则），**不自动创建**；
//   * 保存前完整解析校验，失败整次拒绝、旧文件原样保留；
//   * 保存前把旧文件复制成 `.bak-<epoch>`；
//   * 「恢复默认」把这仓库内的出厂规则包写出去（同样先备份）。
// ---------------------------------------------------------------------------

fn profile_rules_dir(state: &State<'_, SharedHost>) -> std::path::PathBuf {
    let host = state.lock().unwrap();
    host.paths.root.join("profiles")
}

/// 读取规则资产（只读；不创建文件）。
#[tauri::command(async)]
fn profile_rules_load(state: State<'_, SharedHost>) -> profile_rules::RulesView {
    let mut view = profile_rules::load_rules(&profile_rules_dir(&state));
    if view.loaded {
        let dsh_home = {
            let host = state.lock().unwrap();
            host.bridge.supervisor().lock().unwrap().config().dsh_home.clone()
        };
        let repo_skills = crate::task_shell::repo_root_hint().map(|r| r.join("skills")).ok();
        if let Some(rules) = &view.rules {
            for binding in rules.skill_bindings.values() {
                for skill_id in &binding.skills {
                    if view.skill_contents.contains_key(skill_id) {
                        continue;
                    }
                    let dsh_path = dsh_home.join("skills").join(skill_id).join("SKILL.md");
                    let content = if dsh_path.is_file() {
                        std::fs::read_to_string(&dsh_path).ok()
                    } else if let Some(repo_dir) = &repo_skills {
                        let repo_path = repo_dir.join(skill_id).join("SKILL.md");
                        std::fs::read_to_string(&repo_path).ok()
                    } else {
                        None
                    };
                    if let Some(c) = content {
                        view.skill_contents.insert(skill_id.clone(), c);
                    }
                }
            }
        }
    }
    view
}

/// 保存规则资产：先完整解析校验，通过才备份并原子写。
#[tauri::command(async)]
fn profile_rules_save(
    state: State<'_, SharedHost>,
    text: String,
) -> Result<serde_json::Value, String> {
    if text.len() > 128 * 1024 {
        return Err("规则文件过大（上限 128 KB）".to_string());
    }
    // 校验失败一律不写：避免把「以为保存成功」的坏规则留在磁盘上。
    profile_rules::parse_rules(&text)?;
    write_profile_rules(&profile_rules_dir(&state), &text)
}

/// 恢复出厂默认规则包（写入编译期嵌入的样例）。
#[tauri::command(async)]
fn profile_rules_restore_default(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)?;
    write_profile_rules(&profile_rules_dir(&state), profile_rules::DEFAULT_RULES_JSON)
}

fn write_profile_rules(
    profiles_dir: &std::path::Path,
    text: &str,
) -> Result<serde_json::Value, String> {
    std::fs::create_dir_all(profiles_dir).map_err(|e| {
        format!(
            "创建 Profile 目录失败（{}）：{}",
            profiles_dir.display(),
            e
        )
    })?;
    let path = profile_rules::rules_path(profiles_dir);
    let backup = if path.is_file() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let target = path.with_file_name(format!(
            "{}.bak-{}",
            profile_rules::PROFILE_RULES_FILE,
            stamp
        ));
        std::fs::copy(&path, &target)
            .map_err(|e| format!("写入前备份失败（{}）：{}", target.display(), e))?;
        Some(target.display().to_string())
    } else {
        None
    };
    model_settings::write_atomic(&path, text.as_bytes())?;
    Ok(serde_json::json!({
        "ok": true,
        "path": path.display().to_string(),
        "backup": backup,
        "digest": crate::sha256::sha256_hex(text.as_bytes()),
        "note": "规则已保存；下一轮提交即生效，不需要重启。",
    }))
}

// ---------------------------------------------------------------------------
// 模型与端点设置（OD-ARCH-053 S1/S2）
//
// 只碰 OmniDesk 自己数据根下的两个文件：模型配置 JSON 与 DSH 补丁覆盖文件。
// 密钥值只经官方 `credentials/set` 写入 DSH 凭据服务，从不进入配置文件、日志或返回值。
// ---------------------------------------------------------------------------

/// 读取模型配置与凭据状态（只回报「已配置/来源」，不含值）。
#[tauri::command]
async fn model_settings_load(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let (config_dir, app_root) = {
        let state = app.state::<SharedHost>();
        let host = state.lock().unwrap();
        (host.paths.config.clone(), host.paths.root.clone())
    };
    let loaded = model_settings::load(&config_dir);
    let (value, error) = match &loaded {
        Ok(Some(s)) => (serde_json::to_value(s).ok(), None),
        Ok(None) => (None, None),
        Err(message) => (None, Some(message.clone())),
    };
    // 凭据状态只在配置可读时查询；失败不影响配置本身展示。
    let mut key_status: Option<serde_json::Value> = None;
    let mut key_error: Option<String> = None;
    if let Ok(Some(ref s)) = loaded {
        // 一次问全部凭据引用（只回 configured/source/writable 三项事实，不含值）。
        let refs: Vec<String> = s.providers.iter().map(|p| p.api_key_ref.clone()).collect();
        match model_settings_credentials(
            &app,
            "credentials/describe",
            serde_json::json!({ "refs": refs }),
        )
        .await
        {
            Ok(value) => key_status = Some(value),
            Err(message) => key_error = Some(message),
        }
    }
    Ok(serde_json::json!({
        "ok": error.is_none(),
        "settings": value,
        "error": error,
        "config_path": model_settings::settings_path(&config_dir).display().to_string(),
        "legacy_config_path": model_settings::legacy_settings_path(&config_dir).display().to_string(),
        "overlay_path": model_settings::overlay_path(&config_dir).display().to_string(),
        "overlay_present": model_settings::overlay_path(&config_dir).is_file(),
        "app_root": app_root.display().to_string(),
        "key_status": key_status,
        "key_error": key_error,
    }))
}

/// 调一次 DSH 官方凭据 RPC（不经过引擎准入；密钥只在请求体里出现一次）。
async fn model_settings_credentials(
    app: &tauri::AppHandle,
    method: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let (config, log_path, record_path) = {
        let snap = app.state::<HostSnapshot>();
        let config = snap.config.lock().unwrap().clone();
        (
            config,
            snap.paths.logs.join("dsh-settings.log"),
            snap.paths.engine_state.join("dsh-settings.json"),
        )
    };
    let service = std::sync::Arc::clone(app.state::<std::sync::Arc<dsh_settings::SettingsService>>().inner());
    let method = method.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        service.credentials_rpc(&config, &log_path, &record_path, &method, args)
    })
    .await
    .map_err(|e| format!("凭据接口任务异常结束：{}", e))?
}

/// 测试连接 / 拉取模型列表（OD-ARCH-054 S1/S2）。
///
/// 由 DSH 的 `llm/discoverModels` 发真实请求（宿主自己不连公网、不需要 TLS）。
/// `api_key` 可空：空时沿用 DSH 里该路由已登记的凭据；界面新填的 key 只用于这一次探测。
/// 返回值里没有密钥、没有请求头，只有分类、原因与模型 id/名称。
#[tauri::command]
async fn model_settings_probe(
    app: tauri::AppHandle,
    base_url: String,
    api: String,
    provider: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    model_settings::validate_base_url(&base_url)?;
    if !model_settings::ALLOWED_APIS.contains(&api.as_str()) {
        return Err(format!(
            "api 协议只支持：{}（收到 {}）",
            model_settings::ALLOWED_APIS.join(" / "),
            api
        ));
    }
    let key = api_key.filter(|k| !k.trim().is_empty());
    if let Some(key) = key.as_deref() {
        if key.trim() != key {
            return Err("API Key 首尾不能有空格".to_string());
        }
        if key.len() > 8192 {
            return Err("API Key 过长（上限 8192 字符）".to_string());
        }
    }
    let (config, log_path, record_path, config_dir) = {
        let snap = app.state::<HostSnapshot>();
        let config = snap.config.lock().unwrap().clone();
        (
            config,
            snap.paths.logs.join("dsh-settings.log"),
            snap.paths.engine_state.join("dsh-settings.json"),
            snap.paths.config.clone(),
        )
    };
    // 只有「表单端点与该 provider 已保存的路由一致」时才把 provider 交给 DSH：
    // 这样 DSH 才会用该路由已登记的凭据解析密钥；凭空带一个名字没有意义。
    let wanted = provider.unwrap_or_default();
    let saved = model_settings::load(&config_dir)
        .ok()
        .flatten()
        .and_then(|s| s.find(wanted.trim()).cloned());
    let route_provider = saved
        .as_ref()
        .filter(|p| {
            p.base_url.trim_end_matches('/') == base_url.trim_end_matches('/') && p.api == api
        })
        .map(|p| p.provider.clone());
    let route_mismatch = !wanted.trim().is_empty() && route_provider.is_none();
    let service = std::sync::Arc::clone(
        app.state::<std::sync::Arc<dsh_settings::SettingsService>>()
            .inner(),
    );
    let provider_for_receipt = route_provider.clone();
    // 计划要求 10 秒超时：本地 DSH 收到请求后会自己带超时去连端点。
    // 命中 DSH 内置目录时 DSH 会直接回内置表、不联网：结果必须如实标注。
    let (probe, builtin) = tauri::async_runtime::spawn_blocking(move || {
        let builtin = route_provider
            .as_ref()
            .map(|name| {
                model_routes::builtin_providers(&config)
                    .map(|list| list.iter().any(|n| n == name))
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        let probe = service.probe_models(
            &config,
            &log_path,
            &record_path,
            &base_url,
            &api,
            route_provider.as_deref(),
            key.as_deref(),
            std::time::Duration::from_secs(10),
        )?;
        Ok::<_, String>((probe, builtin))
    })
    .await
    .map_err(|e| format!("连接测试任务异常结束：{}", e))??;
    Ok(serde_json::json!({
        "ok": probe.classification == "ok",
        "classification": probe.classification,
        "message": probe.message,
        "models": probe.models,
        "key_source": probe.key_source,
        "builtin_catalog": builtin,
        "provider_route": provider_for_receipt,
        "route_mismatch": route_mismatch,
        "note": if builtin {
            "该提供商名命中 DSH 内置目录，返回的是内置模型表，未发起真实网络请求；"
        } else if route_mismatch {
            "当前端点与该提供商的已保存路由不一致，本次按未保存的新端点探测、没有沿用已存密钥；"
        } else {
            "本结果只说明这一次请求的结果；"
        },
    }))
}

/// 保存模型配置；可选同时写入密钥（只写 DSH 凭据服务）。
/// 保存后停引擎并按新覆盖文件重新判定端点。
#[tauri::command]
async fn model_settings_save(
    app: tauri::AppHandle,
    settings: model_settings::ModelSettings,
    api_key_ref: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    settings.validate()?;
    if let Some(key) = api_key.as_deref() {
        if key.trim() != key || key.trim().is_empty() {
            return Err("API Key 不能为空，首尾也不能有空格".to_string());
        }
        if key.len() > 8192 {
            return Err("API Key 过长（上限 8192 字符）".to_string());
        }
    }
    // 密钥只允许写进本次选中的那个提供商的引用名；引用名必须属于这份配置。
    // 校验放在写盘之前：引用不合法时什么都不改。
    let typed_key = api_key.filter(|k| !k.trim().is_empty());
    let key_target = match typed_key.as_deref() {
        Some(_) => {
            let wanted = api_key_ref.unwrap_or_default();
            Some(
                settings
                    .providers
                    .iter()
                    .find(|p| p.api_key_ref == wanted)
                    .map(|p| p.api_key_ref.clone())
                    .ok_or_else(|| {
                        "拒绝写入密钥：凭据引用不属于本次保存的配置。密钥未写入，配置未改动。"
                            .to_string()
                    })?,
            )
        }
        None => None,
    };
    let (config_dir, app_root) = {
        let state = app.state::<SharedHost>();
        let host = state.lock().unwrap();
        (host.paths.config.clone(), host.paths.root.clone())
    };
    let engine_config = {
        let snap = app.state::<HostSnapshot>();
        let config = snap.config.lock().unwrap().clone();
        config
    };
    // 先取引擎 Profile 当前生效的路由：补丁层对 config 是浅合并，
    // 读不到 Profile 路由就不写盘，避免抹掉 Profile 自带的其它路由。
    let base = model_routes::base_routes(&engine_config, &app_root)?;
    let (json_path, overlay) = model_settings::save(&config_dir, &settings, &base)?;

    let mut key_written = false;
    let mut key_error: Option<String> = None;
    if let (Some(key), Some(target)) = (typed_key, key_target.as_deref()) {
        match model_settings_credentials(
            &app,
            "credentials/set",
            serde_json::json!({ "ref": target, "value": key }),
        )
        .await
        {
            Ok(_) => key_written = true,
            Err(message) => key_error = Some(message),
        }
    }

    // 新覆盖文件就位：同步到 supervisor 与事实快照，停掉旧引擎（下次任务按新路由启动）。
    let state = app.state::<SharedHost>();
    sync_model_overlay(&app, state.inner(), &app_root);
    let lifecycle = {
        let host = state.lock().unwrap();
        Arc::clone(&host.lifecycle)
    };
    let change = lifecycle.begin_change();
    let stopped = match stop_engine(state.inner(), Duration::from_secs(10)) {
        Ok(_) => {
            change.succeed();
            true
        }
        Err(message) => {
            change.fail(message.clone());
            false
        }
    };
    let config = engine_supervisor::EngineConfig::from_env(&app_root)?;
    let route = route_classifier::classify(&config).ok();
    Ok(serde_json::json!({
        "ok": true,
        "config_path": json_path.display().to_string(),
        "overlay_path": overlay.display().to_string(),
        "key_written": key_written,
        "key_error": key_error,
        "engine_stopped": stopped,
        "route": route,
        "note": if key_written {
            "配置与密钥均已保存；引擎已停止，下次任务按新路由启动。"
        } else {
            "配置已保存；引擎已停止，下次任务按新路由启动。密钥未写入。"
        },
    }))
}

// ---------------------------------------------------------------------------
// 工作台线程记录（OD-ARCH-045 第 1 步）
//
// 只读写 OmniDesk 应用数据根下的 threads/，不启动引擎、不做准入判定：
// 这是纯本地文件能力，与模型权限无关。前端把 localStorage 降为缓存，
// 启动时以这里读回的内容为准。旧缓存只读兼容，不删除。
// ---------------------------------------------------------------------------

/// 读回全部线程记录（含被跳过的文件数，如实报告而不静默丢弃）。
#[tauri::command(async)]
fn thread_store_load(
    state: State<'_, SharedHost>,
) -> Result<thread_store::ThreadListing, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let store = thread_store::ThreadStore::resolve(&paths)?;
    store.load()
}

/// 写入（覆盖）一条线程记录。只接受结构完整、id 合规的记录。
#[tauri::command(async)]
fn thread_store_save(
    state: State<'_, SharedHost>,
    thread: serde_json::Value,
) -> Result<thread_store::ThreadSaveReceipt, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let store = thread_store::ThreadStore::resolve(&paths)?;
    store.save(&thread)
}

/// 删除一条线程记录（幂等；只删除本存储自己命名的文件）。
#[tauri::command(async)]
fn thread_store_delete(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let store = thread_store::ThreadStore::resolve(&paths)?;
    let removed = store.delete(&id)?;
    Ok(serde_json::json!({ "id": id, "removed": removed }))
}

// ---------------------------------------------------------------------------
// 项目事项（OD-ARCH-102 S01）
//
// 项目跟进页 v3 的唯一写入路径：应用数据根下的 project-items.v1.json。
// 纯本地文件能力，不启动引擎、不调模型、不触碰成果目录与知识库。
// ---------------------------------------------------------------------------

/// 读回项目事项清单（首次使用返回空清单并说明；主文件损坏时回退 `.bak`）。
#[tauri::command(async)]
fn project_items_read(
    state: State<'_, SharedHost>,
) -> Result<project_items::ProjectItemsListing, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let store = project_items::ProjectItemsStore::resolve(&paths)?;
    store.load()
}

/// 保存整份项目事项清单：逐条校验，非法条目整批拒绝并报「第 N 条」。
#[tauri::command(async)]
fn project_items_save(
    state: State<'_, SharedHost>,
    items: Vec<project_items::ProjectItemRecord>,
) -> Result<project_items::ProjectItemsSaveReceipt, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let store = project_items::ProjectItemsStore::resolve(&paths)?;
    store.save(items, task_shell::now_secs())
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
    // OD-ARCH-050：只读快照，不取 SharedHost 锁。
    //
    // 旧实现为取 EngineConfig 去锁 SharedHost，而引擎冷启动会在持有该锁期间等
    // `initialize`（最长 90s）——于是「读事实」被堵到启动结束，界面显示「未识别」。
    let (config, config_dir, blocked) = {
        let snap = app.state::<HostSnapshot>();
        let config = snap.config.lock().unwrap().clone();
        let config_dir = snap.paths.config.clone();
        let blocked = snap.lifecycle.blocked_reason();
        (config, config_dir, blocked)
    };
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
#[tauri::command(async)]
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
#[tauri::command(async)]
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
#[tauri::command(async)]
fn task_types() -> serde_json::Value {
    serde_json::json!({
        "shell_version": task_shell::SHELL_VERSION,
        "analysis_schema": task_shell::ANALYSIS_SCHEMA,
        "types": task_shell::task_types(),
    })
}

/// 返回编译期嵌入的合成样例（纯虚构数据）。
#[tauri::command(async)]
fn task_samples() -> serde_json::Value {
    serde_json::json!({ "samples": task_shell::samples() })
}

/// 返回解释阶段规则正文（只读展示，帮助使用者了解约束）。
#[tauri::command(async)]
fn task_explanation_rules() -> serde_json::Value {
    serde_json::json!({ "rules": task_shell::explanation_rules() })
}

/// 本地数据根信息（产物与使用记录的落盘位置）。
#[tauri::command(async)]
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

/// 库根信息（OD-ARCH-086）：当前生效路径、来源、已保存选择。
/// 与成果目录分开读，避免一次命令失败同时抹掉两块状态。
#[tauri::command(async)]
fn task_library_root_info(state: State<'_, SharedHost>) -> serde_json::Value {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    match settings::effective_library_root(&config_dir) {
        Ok(eff) => serde_json::json!({
            "resolved": true,
            "library_root": eff.root.display().to_string(),
            "source": eff.source,
            "configured_root": eff.configured_root.as_ref().map(|p| p.display().to_string()),
            "env_override": eff.env_override.as_ref().map(|p| p.display().to_string()),
            "default_root": default_library_root_display(),
            "settings_path": settings::settings_path(&config_dir).display().to_string(),
            "error": null,
        }),
        Err(message) => serde_json::json!({
            "resolved": false,
            "library_root": null,
            "source": null,
            "configured_root": null,
            "env_override": null,
            "default_root": default_library_root_display(),
            "settings_path": settings::settings_path(&config_dir).display().to_string(),
            "error": message,
        }),
    }
}

fn default_library_root_display() -> Option<String> {
    std::env::var_os("HOME").map(|home| {
        std::path::PathBuf::from(home)
            .join(task_shell::DEFAULT_LIBRARY_SUBDIR)
            .display()
            .to_string()
    })
}

// ---------------------------------------------------------------------------
// 知识库只读访问（OD-KB-001 P1）
//
// 四个命令只覆盖：读状态、选根、清除选择、扫描、读一篇笔记。没有写入、删除、
// 批量改写或索引命令。根与每个读取路径都在宿主侧重新校验；界面传入的相对路径
// 不作为信任边界。
// ---------------------------------------------------------------------------

/// 知识库根状态：已保存选择是否存在、当前是否可用。不扫描、不读文件内容。
pub fn knowledge_vault_status_for(paths: &crate::app_paths::AppPaths) -> knowledge_vault::VaultRootStatus {
    match settings::configured_knowledge_vault_root(&paths.config) {
        Ok(None) => knowledge_vault::VaultRootStatus {
            configured: false,
            root: None,
            exists: false,
            is_dir: false,
            error: None,
        },
        Ok(Some(root)) => {
            let root_text = root.display().to_string();
            match knowledge_vault::assert_vault_root_allowed(&root, Some(&paths.root)) {
                Ok(_) => knowledge_vault::VaultRootStatus {
                    configured: true,
                    root: Some(root_text),
                    exists: true,
                    is_dir: true,
                    error: None,
                },
                Err(message) => knowledge_vault::VaultRootStatus {
                    configured: true,
                    root: Some(root_text),
                    exists: root.exists(),
                    is_dir: root.is_dir(),
                    error: Some(message),
                },
            }
        }
        Err(message) => knowledge_vault::VaultRootStatus {
            configured: false,
            root: None,
            exists: false,
            is_dir: false,
            error: Some(message),
        },
    }
}

/// 读取已保存且仍通过边界校验的知识库根。
///
/// 每次扫描/读取都重新走一遍校验：设置文件被手改、根被换成符号链接或目录被移走
/// 时，宿主如实报错，而不是继续使用过期路径。
pub fn configured_vault_root(paths: &crate::app_paths::AppPaths) -> Result<std::path::PathBuf, String> {
    match settings::configured_knowledge_vault_root(&paths.config)? {
        Some(root) => knowledge_vault::assert_vault_root_allowed(&root, Some(&paths.root)),
        None => Err("还没有选择知识库目录；请先在设置里选择一个已有的 Markdown 目录。".to_string()),
    }
}

/// 扫描已保存的知识库根（刷新与首次扫描走同一条只读路径）。
pub fn knowledge_vault_scan_for(
    paths: &crate::app_paths::AppPaths,
) -> Result<knowledge_vault::VaultScan, String> {
    let root = configured_vault_root(paths)?;
    knowledge_vault::scan_vault(&root, Some(&paths.root))
}

/// 读取根内一篇笔记正文；根与相对路径都在宿主侧重校验。
pub fn knowledge_vault_read_note_for(
    paths: &crate::app_paths::AppPaths,
    relative_path: &str,
) -> Result<knowledge_vault::VaultNote, String> {
    let root = configured_vault_root(paths)?;
    knowledge_vault::read_vault_note(&root, relative_path)
}

/// 解析一篇笔记的 Frontmatter 与正文（只读，P2-S01）。
pub fn knowledge_vault_parse_note_for(
    paths: &crate::app_paths::AppPaths,
    relative_path: &str,
) -> Result<vault_note::ParsedNote, String> {
    let note = knowledge_vault_read_note_for(paths, relative_path)?;
    Ok(vault_note::parse_note(&note.content))
}

/// 新建一篇根内笔记（P2-S02）。根在每次调用时重新校验。
pub fn knowledge_vault_create_note_for(
    paths: &crate::app_paths::AppPaths,
    relative_path: &str,
    content: &str,
) -> Result<knowledge_vault::VaultNote, String> {
    let root = configured_vault_root(paths)?;
    vault_write::create_note(&root, relative_path, content)
}

/// 保存一篇已有笔记，必须以打开时读到的版本哈希做冲突检查（P2-S02）。
pub fn knowledge_vault_save_note_for(
    paths: &crate::app_paths::AppPaths,
    relative_path: &str,
    content: &str,
    expected_sha256: &str,
) -> Result<vault_write::SaveOutcome, String> {
    let root = configured_vault_root(paths)?;
    vault_write::save_note(&root, relative_path, content, expected_sha256)
}

/// 扫描并在内存中搜索笔记（P2-S05）；不建立持久索引。
pub fn knowledge_vault_search_for(
    paths: &crate::app_paths::AppPaths,
    query: &vault_search::SearchQuery,
) -> Result<vault_search::SearchResponse, String> {
    let root = configured_vault_root(paths)?;
    vault_search::search_vault(&root, query)
}

/// 用系统处理程序打开根内附件（P2-S03），不读取附件正文。
pub fn knowledge_vault_open_attachment_for(
    paths: &crate::app_paths::AppPaths,
    relative_path: &str,
) -> Result<String, String> {
    let root = configured_vault_root(paths)?;
    vault_open::open_attachment(&root, relative_path)
}

/// 显式重建链接索引（P3-S04）。扫描与读取都在发布前完成；任一步失败或快照
/// 不完整都如实返回，不写知识库、不写设置、不启动后台 watcher。
pub fn knowledge_vault_rebuild_index_for(
    paths: &crate::app_paths::AppPaths,
    previous: Option<&vault_index::LinkIndex>,
) -> Result<vault_index::LinkIndex, String> {
    let root = configured_vault_root(paths)?;
    let scan = knowledge_vault::scan_vault(&root, Some(&paths.root))?;
    let options = vault_index::IndexBuildOptions {
        previous_generation: previous.map(|index| index.generation).unwrap_or(0),
        ..Default::default()
    };
    Ok(vault_index::build_from_scan(
        &root,
        &scan,
        options,
        task_shell::now_secs() as u64,
    ))
}

/// 读取一篇笔记的关系视图。只读已发布的完整快照，不触发磁盘读取或重建。
pub fn knowledge_vault_note_links_for(
    index: &vault_index::LinkIndex,
    root: &std::path::Path,
    relative_path: &str,
) -> Result<vault_index::NoteLinks, String> {
    if index.root != root.display().to_string() {
        return Err("当前链接索引属于另一个知识库目录；请先刷新索引。".to_string());
    }
    vault_index::note_links(index, relative_path)
        .ok_or_else(|| format!("链接索引里没有这篇笔记：{}", relative_path))
}

#[tauri::command(async)]
fn knowledge_vault_status(state: State<'_, SharedHost>) -> knowledge_vault::VaultRootStatus {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    knowledge_vault_status_for(&paths)
}

/// 保存知识库根。只写设置文件；不创建目录、不做写入探针、不迁移任何文件。
#[tauri::command(async)]
fn knowledge_vault_set_root(
    state: State<'_, SharedHost>,
    path: String,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = settings::set_knowledge_vault_root(&paths, &path)?;
    Ok(serde_json::json!({
        "ok": true,
        "knowledge_vault_root": root.display().to_string(),
        "note": "知识库只读：OmniDesk 只列出并读取你选择的文件，不会写入、移动或删除。",
    }))
}

#[tauri::command(async)]
fn knowledge_vault_clear_root(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    settings::clear_knowledge_vault_root(&paths)?;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command(async)]
fn knowledge_vault_scan(
    state: State<'_, SharedHost>,
) -> Result<knowledge_vault::VaultScan, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    knowledge_vault_scan_for(&paths)
}

#[tauri::command(async)]
fn knowledge_vault_read_note(
    state: State<'_, SharedHost>,
    relative_path: String,
) -> Result<knowledge_vault::VaultNote, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    knowledge_vault_read_note_for(&paths, &relative_path)
}

#[tauri::command(async)]
fn knowledge_vault_parse_note(
    state: State<'_, SharedHost>,
    relative_path: String,
) -> Result<vault_note::ParsedNote, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    knowledge_vault_parse_note_for(&paths, &relative_path)
}

#[tauri::command(async)]
fn knowledge_vault_create_note(
    state: State<'_, SharedHost>,
    relative_path: String,
    content: String,
) -> Result<knowledge_vault::VaultNote, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    knowledge_vault_create_note_for(&paths, &relative_path, &content)
}

#[tauri::command(async)]
fn knowledge_vault_save_note(
    state: State<'_, SharedHost>,
    relative_path: String,
    content: String,
    expected_sha256: String,
) -> Result<vault_write::SaveOutcome, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    knowledge_vault_save_note_for(&paths, &relative_path, &content, &expected_sha256)
}

#[tauri::command(async)]
fn knowledge_vault_search(
    state: State<'_, SharedHost>,
    query: vault_search::SearchQuery,
) -> Result<vault_search::SearchResponse, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    knowledge_vault_search_for(&paths, &query)
}

#[tauri::command(async)]
fn knowledge_vault_open_attachment(
    state: State<'_, SharedHost>,
    relative_path: String,
) -> Result<String, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    knowledge_vault_open_attachment_for(&paths, &relative_path)
}

/// 显式重建并原子发布链接索引。构建在锁外完成，只有完整快照才替换旧快照。
#[tauri::command(async)]
fn knowledge_vault_rebuild_index(
    state: State<'_, SharedHost>,
) -> Result<vault_index::LinkIndex, String> {
    let (paths, cache) = {
        let host = state.lock().unwrap();
        (host.paths.clone(), host.vault_index.clone())
    };
    let previous = {
        let guard = cache.lock().unwrap();
        guard.clone()
    };
    // 重新核对当前设置里的根：换根后不复用旧目录的 generation 或内容。
    let root = configured_vault_root(&paths)?;
    let same_root = previous
        .as_ref()
        .is_some_and(|index| index.root == root.display().to_string());
    let previous = previous.filter(|_| same_root);
    let index = knowledge_vault_rebuild_index_for(&paths, previous.as_ref())?;
    {
        let mut guard = cache.lock().unwrap();
        *guard = Some(index.clone());
    }
    Ok(index)
}

/// 读取当前已发布索引的元信息（不触发重建）。没有索引时返回 null。
#[tauri::command(async)]
fn knowledge_vault_index_status(
    state: State<'_, SharedHost>,
) -> Result<Option<vault_index::LinkIndex>, String> {
    let cache = {
        let host = state.lock().unwrap();
        host.vault_index.clone()
    };
    let snapshot = cache.lock().unwrap().clone();
    Ok(snapshot)
}

/// 按笔记读取正向链接与反向链接；只读当前快照，不访问磁盘。
#[tauri::command(async)]
fn knowledge_vault_note_links(
    state: State<'_, SharedHost>,
    relative_path: String,
) -> Result<vault_index::NoteLinks, String> {
    let snapshot = {
        let host = state.lock().unwrap();
        let paths = host.paths.clone();
        let snapshot = host.vault_index.lock().unwrap().clone();
        (paths, snapshot)
    };
    let (paths, snapshot) = snapshot;
    let index = snapshot.ok_or_else(|| "还没有构建链接索引；请先刷新索引。".to_string())?;
    let root = configured_vault_root(&paths)?;
    knowledge_vault_note_links_for(&index, &root, &relative_path)
}

// ---------------------------------------------------------------------------
// 工作记录确定性落盘（OD-ARCH-089 S2）
//
// 界面在收到随手记时**先调这里写盘，成功后才叫模型做缺项分析**。
// 这样模型调用失败、超时或被取消时，用户的事实已经留在库里。
// ---------------------------------------------------------------------------

/// 本地时区偏移（分钟）：复用调度配置里的 `utc_offset_minutes`，缺省东八区。
fn worklog_offset_minutes(state: &State<'_, SharedHost>) -> i32 {
    let dir = scheduler_config_dir(state);
    scheduler::load(&dir)
        .map(|config| config.utc_offset_minutes)
        .unwrap_or(scheduler::DEFAULT_UTC_OFFSET_MINUTES)
}

fn worklog_library_root(state: &State<'_, SharedHost>) -> Result<std::path::PathBuf, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    settings::effective_library_root(&config_dir).map(|eff| eff.root)
}

/// 追加一条随手记。返回结构化回执，失败原因原样带回，不假装已记录。
#[tauri::command(async)]
fn worklog_append(
    state: State<'_, SharedHost>,
    text: String,
    destination: Option<String>,
    entry_date: Option<String>,
) -> serde_json::Value {
    let offset = worklog_offset_minutes(&state);
    let now = task_shell::now_secs();
    let week = worklog::week_label(now, offset);
    let today = worklog::date_label(now, offset);
    // OD-ARCH-103 S2：输入里认出的补录日期由界面带进来；缺省仍是当天。
    // 不合法一律拒绝并说明原因——绝不静默回退成今天，否则又回到「以为记到周一、实际记到周三」。
    let requested = entry_date
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty());
    if let Some(day) = requested {
        if let Err(message) = worklog::assert_entry_date(&week, day, &today) {
            return serde_json::json!({
                "ok": false,
                "path": null,
                "week": week,
                "date": day,
                "today": today,
                "appended": false,
                "was_new_file": false,
                "continued": false,
                "masked": false,
                "masked_id_card": 0,
                "masked_phone": 0,
                "entry_index": 0,
                "entry_text": "",
                "error": message,
            });
        }
    }
    let date = requested.unwrap_or(today.as_str()).to_string();
    let root = match worklog_library_root(&state) {
        Ok(root) => root,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "path": null,
                "week": week,
                "date": date,
                "appended": false,
                "was_new_file": false,
                "continued": false,
                "masked": false,
                "masked_id_card": 0,
                "masked_phone": 0,
                "entry_index": 0,
                "entry_text": "",
                "error": message,
            })
        }
    };
    let policy = match current_rule_policy(&state) {
        Ok(policy) => policy,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "path": null,
                "week": week,
                "date": date,
                "appended": false,
                "was_new_file": false,
                "continued": false,
                "masked": false,
                "masked_id_card": 0,
                "masked_phone": 0,
                "entry_index": 0,
                "entry_text": "",
                "error": message,
            })
        }
    };
    // 红线：命中即拒绝，不静默落盘。错误里不回显命中原文。
    let hits = task_shell::red_line_scan_with(&text, &policy);
    if !hits.is_empty() {
        let mut detail = String::new();
        for hit in &hits {
            detail.push_str(&format!("{}（{} 处）；", hit.label, hit.count));
        }
        return serde_json::json!({
            "ok": false,
            "path": null,
            "week": week,
            "date": date,
            "appended": false,
            "was_new_file": false,
            "continued": false,
            "masked": false,
            "masked_id_card": 0,
            "masked_phone": 0,
            "entry_index": 0,
            "entry_text": "",
            "error": format!(
                "本次内容命中红线扫描，已拒绝写入工作记录：{}。该扫描仅为启发式，不代表合规判断。",
                detail
            ),
        });
    }
    let (masked_text, counts) = profile_rules::mask_privacy(&text, &policy);
    let dest = destination
        .as_deref()
        .map(|d| d.trim())
        .filter(|d| !d.is_empty())
        .unwrap_or(worklog::DEFAULT_DESTINATION);
    match worklog::append_entry(&root, &week, &date, &masked_text, dest) {
        Ok((mut receipt, _payload)) => {
            receipt.masked = counts.any();
            receipt.masked_id_card = counts.id_card;
            receipt.masked_phone = counts.phone;
            serde_json::json!(receipt)
        }
        Err(message) => serde_json::json!({
            "ok": false,
            "path": worklog::week_path(&root, &week).display().to_string(),
            "week": week,
            "date": date,
            "appended": false,
            "was_new_file": false,
            "continued": false,
            "masked": counts.any(),
            "masked_id_card": counts.id_card,
            "masked_phone": counts.phone,
            "entry_index": 0,
            "entry_text": "",
            "error": message,
        }),
    }
}

/// 改一条记录的「去向」（OD-ARCH-099 S02）。
///
/// 界面上的点选只做两件事：改这条记录在周文件里的去向字段，以及让周报上下文随之
/// 取到或不取到它。这里不写周报、不碰别的条目；正文由界面带回用于一致性校验。
#[tauri::command(async)]
fn worklog_set_destination(
    state: State<'_, SharedHost>,
    week: String,
    date: String,
    index: usize,
    destination: String,
    expect_text: String,
) -> serde_json::Value {
    let root = match worklog_library_root(&state) {
        Ok(root) => root,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "path": null,
                "week": week,
                "date": date,
                "index": index,
                "destination": destination,
                "previous": null,
                "backup": null,
                "error": message,
            })
        }
    };
    match worklog::set_destination(&root, &week, &date, index, &destination, &expect_text) {
        Ok(receipt) => serde_json::json!(receipt),
        Err(message) => serde_json::json!({
            "ok": false,
            "path": worklog::week_path(&root, &week).display().to_string(),
            "week": week,
            "date": date,
            "index": index,
            "destination": destination,
            "previous": null,
            "backup": null,
            "error": message,
        }),
    }
}

/// 改一条记录的日期（OD-ARCH-103 S3）。
///
/// 引擎侧看不到宿主命令（ACP 会话的 mcpServers 恒为空），所以模型无法直接调用它；
/// 这条命令由界面在识别出「记错了，应该是周一」这类纯日期纠正时调用，不叫模型。
/// 只在本周文件内把这一条从原日期段移到目标日期段，内容不改写。
#[tauri::command(async)]
fn worklog_redate(
    state: State<'_, SharedHost>,
    week: String,
    from_date: String,
    index: usize,
    to_date: String,
    expect_text: String,
) -> serde_json::Value {
    let root = match worklog_library_root(&state) {
        Ok(root) => root,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "path": null,
                "week": week,
                "from_date": from_date,
                "to_date": to_date,
                "index": index,
                "new_index": 0,
                "backup": null,
                "error": message,
            })
        }
    };
    match worklog::move_entry(&root, &week, &from_date, index, &to_date, &expect_text) {
        Ok(receipt) => serde_json::json!(receipt),
        Err(message) => serde_json::json!({
            "ok": false,
            "path": worklog::week_path(&root, &week).display().to_string(),
            "week": week,
            "from_date": from_date,
            "to_date": to_date,
            "index": index,
            "new_index": 0,
            "backup": null,
            "error": message,
        }),
    }
}


/// 读当周工作记录（周报「有米」用）。只读，不建文件、不改文件。
#[tauri::command(async)]
fn worklog_read_week(state: State<'_, SharedHost>, week: Option<String>) -> serde_json::Value {
    let offset = worklog_offset_minutes(&state);
    let resolved_week = match week.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
        Some(w) => w.to_string(),
        None => worklog::week_label(task_shell::now_secs(), offset),
    };
    let root = match worklog_library_root(&state) {
        Ok(root) => root,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "exists": false,
                "week": resolved_week,
                "path": null,
                "content": "",
                "error": message,
            })
        }
    };
    match worklog::read_week(&root, &resolved_week) {
        Ok((exists, content, path)) => serde_json::json!({
            "ok": true,
            "exists": exists,
            "week": resolved_week,
            "path": path.display().to_string(),
            "content": content,
            "error": null,
        }),
        Err(message) => serde_json::json!({
            "ok": false,
            "exists": false,
            "week": resolved_week,
            "path": worklog::week_path(&root, &resolved_week).display().to_string(),
            "content": "",
            "error": message,
        }),
    }
}

/// 列工作记录与周报归档（OD-ARCH-091）。只读文件系统元数据，不读正文、不调模型。
#[tauri::command(async)]
fn worklog_archive_list(state: State<'_, SharedHost>) -> serde_json::Value {
    let offset = worklog_offset_minutes(&state);
    let current = worklog::week_label(task_shell::now_secs(), offset);
    let root = match worklog_library_root(&state) {
        Ok(root) => root,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "library_root": null,
                "current_week": current,
                "weeks": [],
                "reports": [],
                "skipped": 0,
                "error": message,
            })
        }
    };
    match (worklog::list_weeks(&root), worklog::list_reports(&root)) {
        (Ok((weeks, skipped_weeks)), Ok((reports, skipped_reports))) => serde_json::json!({
            "ok": true,
            "library_root": root.display().to_string(),
            "current_week": current,
            "weeks": weeks,
            "reports": reports,
            "skipped": skipped_weeks + skipped_reports,
            "error": null,
        }),
        (Err(message), _) | (_, Err(message)) => serde_json::json!({
            "ok": false,
            "library_root": root.display().to_string(),
            "current_week": current,
            "weeks": [],
            "reports": [],
            "skipped": 0,
            "error": message,
        }),
    }
}

/// 读一份周报正文。`file_name` 必须形如 `2026-W44-周报.md`（可带 `-2`），否则拒绝。
#[tauri::command(async)]
fn worklog_read_report(
    state: State<'_, SharedHost>,
    file_name: String,
) -> serde_json::Value {
    let root = match worklog_library_root(&state) {
        Ok(root) => root,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "exists": false,
                "file_name": file_name,
                "path": null,
                "content": "",
                "error": message,
            })
        }
    };
    match worklog::read_report(&root, &file_name) {
        Ok((exists, content, path)) => serde_json::json!({
            "ok": true,
            "exists": exists,
            "file_name": file_name,
            "path": path.display().to_string(),
            "content": content,
            "error": null,
        }),
        Err(message) => serde_json::json!({
            "ok": false,
            "exists": false,
            "file_name": file_name,
            "path": null,
            "content": "",
            "error": message,
        }),
    }
}

/// 保存新的库根。只影响之后的 @ 引用：不迁移、不覆盖、不删除旧文件。
#[tauri::command(async)]
fn task_set_library_root(
    state: State<'_, SharedHost>,
    path: String,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = settings::set_library_root(&paths, &path)?;
    Ok(serde_json::json!({
        "ok": true,
        "library_root": root.display().to_string(),
        "note": "仅影响之后的 @ 引用；既有文件不会移动、覆盖或删除。",
    }))
}

/// 保存用户选择的成果目录。只影响之后新写入的成果：不迁移、不覆盖、不删除旧文件。
///
/// 校验顺序：绝对路径 → 仓库/DSH/应用数据根边界 → 创建目录 → 再次校验真实落点
/// → 一次性写入探针（写完即删）→ 原子替换设置文件。任一步失败都不写入设置。
#[tauri::command(async)]
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

/// 对话分类（OD-ARCH-062）：只读写 settings.v1 的 `thread_categories`。
/// 不迁移既有线程标签、不改成果目录与模型权限；越界目录整次拒绝。
#[tauri::command(async)]
fn category_settings_load(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let settings = settings::load(&config_dir)?;
    let mut rows = Vec::with_capacity(settings.thread_categories.len());
    for c in &settings.thread_categories {
        let resolved = settings::resolve_category_target(&config_dir, c);
        rows.push(serde_json::json!({
            "key": c.key,
            "label": c.label,
            "folder": c.folder,
            "vault": c.vault,
            "builtin": settings::BUILTIN_CATEGORY_KEYS.contains(&c.key.as_str()),
            "resolved_dir": resolved.as_ref().ok().map(|p| p.display().to_string()),
            "resolve_error": resolved.err(),
        }));
    }
    Ok(serde_json::json!({
        "ok": true,
        "settings_path": settings::settings_path(&config_dir).display().to_string(),
        "categories": rows,
    }))
}

#[tauri::command(async)]
fn category_update(
    state: State<'_, SharedHost>,
    key: String,
    label: Option<String>,
    folder: Option<String>,
    vault: Option<String>,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    // 空字符串表示“清除该映射”，None 表示“这一项不动”。
    let folder_patch = folder.map(|f| if f.trim().is_empty() { None } else { Some(f) });
    let vault_patch = vault.map(|v| if v.trim().is_empty() { None } else { Some(v) });
    let categories = settings::update_thread_category(
        &paths,
        &key,
        label,
        folder_patch,
        vault_patch,
    )?;
    Ok(serde_json::json!({ "ok": true, "count": categories.len() }))
}

/// 新增一个自定义分类。键由这里生成（custom:<slug>），不接受前端任意键。
/// 新增分类只进入归类下拉与成果映射，不会自动出现在 Dock。
#[tauri::command(async)]
fn category_add(
    state: State<'_, SharedHost>,
    label: String,
    folder: Option<String>,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let mut settings_now = settings::load(&paths.config)?;
    let exists = |list: &[settings::ThreadCategory], wanted: &str| {
        list.iter().any(|c| c.label.trim() == wanted)
    };
    let trimmed = label.trim();
    if exists(&settings_now.thread_categories, trimmed) {
        return Err(format!("已经有同名分类：{}", trimmed));
    }
    let mut seq = 1;
    let key = loop {
        let candidate = format!("custom:cat-{}", seq);
        if !settings_now.thread_categories.iter().any(|c| c.key == candidate) {
            break candidate;
        }
        seq += 1;
        if seq > settings::MAX_CATEGORIES {
            return Err("自定义分类数量已达上限".to_string());
        }
    };
    settings_now.thread_categories.push(settings::ThreadCategory {
        key: key.clone(),
        label: trimmed.to_string(),
        folder: folder.map(|f| f.trim().to_string()).filter(|f| !f.is_empty()),
        vault: None,
    });
    settings::set_thread_categories(&paths, settings_now.thread_categories)?;
    Ok(serde_json::json!({ "ok": true, "key": key }))
}

/// 删除自定义分类。内置四类不可删；被线程引用的自定义分类也不删（避免线程失去归属）。
#[tauri::command(async)]
fn category_remove(
    state: State<'_, SharedHost>,
    key: String,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    if settings::BUILTIN_CATEGORY_KEYS.contains(&key.as_str()) {
        return Err(format!("内置分类不能删除：{}", key));
    }
    let mut current = settings::load(&paths.config)?;
    current.thread_categories.retain(|c| c.key != key);
    settings::set_thread_categories(&paths, current.thread_categories)?;
    Ok(serde_json::json!({ "ok": true }))
}

/// 建分类对应的目录骨架：只创建缺失目录，已存在跳过，不删不改任何文件。
#[tauri::command(async)]
fn category_init_folders(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let categories = settings::load(&config_dir)?.thread_categories;
    let rows = settings::init_category_folders(&config_dir, &categories)?;
    let created = rows.iter().filter(|r| r["created"] == true).count();
    Ok(serde_json::json!({
        "ok": true,
        "created": created,
        "items": rows,
        "note": "只创建缺失目录；已存在的目录与其中的文件原样保留。",
    }))
}

/// 在系统文件管理器中打开当前成果目录（只读入口）。
///
/// 不拼接 shell 命令：只执行固定的 `xdg-open` / `gio open`，目录作为单独参数传入，
/// 且必须通过与保存同一条边界校验。找不到可用的打开器时如实返回错误。
#[tauri::command(async)]
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

/// 在系统默认浏览器里打开正文中的 http(s) 链接（OD-ARCH-081）。
///
/// 只放行 `http` / `https`，其余 scheme 一律拒绝，避免由模型输出诱发本地协议跳转。
#[tauri::command(async)]
fn open_external_url(url: String) -> Result<serde_json::Value, String> {
    let opened = external_open::open_in_browser(&url)?;
    Ok(serde_json::json!({ "ok": true, "opened": opened }))
}

// ---------------------------------------------------------------------------
// 能力管理（OD-ARCH-072）
//
// 只读：Skill 列表（扫隔离 DSH_HOME 的 skills/）与插件列表（profile 的 package.json）。
// 受控写：插件启停 —— 只改 <隔离 DSH_HOME>/profiles/<profile>/package.json
// 的 dsh.profile.bundles，写前备份，路径越界一律拒绝。
//
// D-030（插件冻结）：不提供「新增插件 / 接入插件市场」入口，插件清单随 OmniDesk
// 版本发布固定；这里只保留对清单内已有条目的查看与启停。
// ---------------------------------------------------------------------------

#[tauri::command(async)]
fn capability_view(state: State<'_, SharedHost>) -> Result<capabilities::CapabilityView, String> {
    let (dsh_home, profile) = {
        let host = state.lock().unwrap();
        let cfg = host.bridge.supervisor().lock().unwrap().config().clone();
        (cfg.dsh_home, cfg.profile)
    };
    capabilities::load(&dsh_home, &profile)
}

/// 启用/停用一个插件（= 在 bundles 数组里增删）。`name` 只能是清单里已有的条目。
#[tauri::command(async)]
fn capability_set_plugin(
    state: State<'_, SharedHost>,
    name: String,
    enabled: bool,
) -> Result<capabilities::CapabilityReceipt, String> {
    let (dsh_home, profile) = {
        let host = state.lock().unwrap();
        let cfg = host.bridge.supervisor().lock().unwrap().config().clone();
        (cfg.dsh_home, cfg.profile)
    };
    let dir = capabilities::profile_dir(&dsh_home, &profile)?;
    capabilities::set_bundle(&dir, &name, enabled)
}

/// 只读：在系统文件管理器中打开 Skill 目录。**不**接受路径参数。
#[tauri::command(async)]
fn capability_open_skills(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let dsh_home = {
        let host = state.lock().unwrap();
        host.bridge.supervisor().lock().unwrap().config().dsh_home.clone()
    };
    capabilities::assert_writable_root(&dsh_home)?;
    let dir = dsh_home.join("skills");
    if !dir.is_dir() {
        return Err(format!("Skill 目录还不存在：{}", dir.display()));
    }
    let mut last_error = String::new();
    for (program, args) in [
        ("xdg-open", Vec::<String>::new()),
        ("gio", vec!["open".to_string()]),
    ] {
        match std::process::Command::new(program)
            .args(&args)
            .arg(&dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                return Ok(serde_json::json!({
                    "ok": true,
                    "opened": dir.display().to_string(),
                    "note": "Skill 在这里只读展示；改动目录里的文件后重启应用生效。",
                }))
            }
            Err(e) => last_error = format!("{}：{}", program, e),
        }
    }
    Err(format!("无法打开 Skill 目录，系统缺少可用的打开器（{}）", last_error))
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
#[tauri::command(async)]
fn task_set_model_route(
    app: tauri::AppHandle,
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
    // OD-ARCH-048：web-api 通道的模型目录按 provider 列模型，provider 未必定义在
    // 引擎 Profile 里（它可能来自共享凭据层的另一个 Profile）。先试指定 Profile，
    // 只有「该 Profile 没有这个 provider」时才依次尝试其余已知 Profile；
    // 其它错误原样返回，不做猜测性回退。
    let mut candidates = vec![source_profile.clone()];
    for profile in model_routes::discover_profiles(&config.dsh_home, &config.profile) {
        if !candidates.contains(&profile) {
            candidates.push(profile);
        }
    }
    let mut written = None;
    let mut last_error = String::new();
    for candidate in &candidates {
        match model_routes::select_route(
            &config,
            &overlay,
            candidate,
            &provider,
            &model,
            reasoning_effort.as_deref(),
        ) {
            Ok(value) => {
                written = Some(value);
                break;
            }
            Err(message) => {
                let provider_missing = message.contains("没有这个 provider")
                    || message.contains("provider_not_found");
                if !provider_missing {
                    return Err(message);
                }
                last_error = message;
            }
        }
    }
    let written = written.ok_or_else(|| {
        if last_error.is_empty() {
            "没有可写入该模型的 Profile。".to_string()
        } else {
            format!("{}（已尝试 Profile：{}）", last_error, candidates.join(", "))
        }
    })?;

    // 覆盖文件刚写入：先让 supervisor 与事实快照看到它，再停引擎。
    sync_model_overlay(&app, state.inner(), &app_root);

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

/// 改为跟随 DSH 设置（OD-ARCH-050）。
///
/// 归档 OmniDesk 自己的模型覆盖文件（**不删除**，改名留在原地便于回滚），停掉引擎；
/// 下次任务按 DSH Profile 里配置的 provider 与默认模型启动。
#[tauri::command(async)]
fn task_clear_model_route(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let overlay = model_routes::overlay_path(&app_root);
    let archived = if overlay.is_file() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let target = overlay.with_file_name(format!("omnidesk-model.yml.bak-{}", stamp));
        std::fs::rename(&overlay, &target)
            .map_err(|e| format!("归档模型覆盖文件失败（{}）：{}", overlay.display(), e))?;
        Some(target)
    } else {
        None
    };

    // 覆盖文件已归档：先让 supervisor 与事实快照去掉这个补丁，再停引擎。
    sync_model_overlay(&app, state.inner(), &app_root);

    // 引擎已在跑时仍按旧覆盖文件启动：停掉它，下次任务按 DSH 配置重启。
    let lifecycle = {
        let host = state.lock().unwrap();
        Arc::clone(&host.lifecycle)
    };
    let change = lifecycle.begin_change();
    match stop_engine(state.inner(), Duration::from_secs(10)) {
        Ok(_) => change.succeed(),
        Err(message) => {
            change.fail(message.clone());
            return Err(format!(
                "已改为跟随 DSH 设置，但未能确认引擎停止（{}）；请到设置页手动停止引擎后再试。",
                message
            ));
        }
    }

    let note = match &archived {
        Some(path) => format!(
            "已改为跟随 DSH 设置：OmniDesk 的模型覆盖文件已归档到 {}，下次任务将使用 DSH 里配置的 provider 与默认模型。",
            path.display()
        ),
        None => "当前已经是跟随 DSH 设置（没有 OmniDesk 覆盖文件）。".to_string(),
    };
    Ok(serde_json::json!({
        "ok": true,
        "archived": archived.map(|p| p.display().to_string()),
        "note": note,
    }))
}

/// 读取（必要时首次创建）离线流程包。
///
/// 不创建模型会话、不启动引擎；只读一个仓库外的纯文本 JSON。
#[tauri::command(async)]
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
#[tauri::command(async)]
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
#[tauri::command(async)]
fn task_run_analysis(state: State<'_, SharedHost>, input_text: String) -> serde_json::Value {
    // OD-ARCH-068：规则损坏时整次拒绝，不静默按纯净底座计算。
    let policy = match current_rule_policy(&state) {
        Ok(policy) => policy,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "result": null,
                "red_line_hits": [],
                "error": message,
            })
        }
    };
    let hits = task_shell::red_line_scan_with(&input_text, &policy);
    match task_shell::analyze_input_with(&input_text, &policy) {
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
#[tauri::command(async)]
fn task_explanation_prompt(
    state: State<'_, SharedHost>,
    task_type: String,
    input_text: String,
    fingerprint: String,
) -> Result<serde_json::Value, String> {
    let policy = current_rule_policy(&state)?;
    task_shell::build_explanation_prompt_with(&task_type, &input_text, &fingerprint, &policy)
        .map(|prompt| serde_json::json!({ "prompt": prompt }))
}

/// 保存产物到仓库外本地数据目录（不覆盖已有文件）。
#[tauri::command(async)]
fn task_save_artifact(
    state: State<'_, SharedHost>,
    request: SaveArtifactRequest,
) -> Result<serde_json::Value, String> {
    let now = task_shell::now_secs();
    let policy = current_rule_policy(&state)?;
    task_shell::save_artifact_with(&request, now, &policy)
        .map(|receipt| serde_json::json!(receipt))
}

/// 追加一条轻量使用记录（不含输入输出、不含自由文本）。
#[tauri::command(async)]
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
#[tauri::command(async)]
fn task_usage_view(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let path = {
        let host = state.lock().unwrap();
        host.paths.logs.join("usage.jsonl")
    };
    task_shell::read_usage_records(&path).and_then(|r| serde_json::to_value(r).map_err(|e| e.to_string()))
}

/// 删除单条使用记录。
#[tauri::command(async)]
fn task_usage_delete(state: State<'_, SharedHost>, record_id: String) -> Result<serde_json::Value, String> {
    let path = {
        let host = state.lock().unwrap();
        host.paths.logs.join("usage.jsonl")
    };
    task_shell::delete_usage_record(&path, &record_id)
        .map(|deleted| serde_json::json!({ "deleted": deleted }))
}

/// 清空全部使用记录。
#[tauri::command(async)]
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
#[tauri::command(async)]
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

/// 当前生效的 Profile 规则策略（OD-ARCH-068）。
///
/// 文件不存在 → 纯净底座；文件损坏 → 明确报错，由调用方整次拒绝。
fn current_rule_policy(state: &State<'_, SharedHost>) -> Result<profile_rules::RulePolicy, String> {
    let profiles_dir = {
        let host = state.lock().unwrap();
        host.paths.root.join("profiles")
    };
    let view = profile_rules::load_rules(&profiles_dir);
    if let Some(error) = view.error {
        return Err(format!(
            "Profile 规则文件无法读取，已拒绝本次操作：{}。请到设置 →「规则资产」修正或恢复默认。",
            error
        ));
    }
    Ok(match view.rules.as_ref() {
        Some(rules) => profile_rules::RulePolicy::from_rules(rules),
        None => profile_rules::RulePolicy::clean(),
    })
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
#[tauri::command(async)]
fn task_weekly_summary(state: State<'_, SharedHost>, input_text: String) -> serde_json::Value {
    let loaded = match current_flow_pack(&state) {
        Ok(loaded) => loaded,
        Err(message) => return pack_refusal(message),
    };
    let policy = match current_rule_policy(&state) {
        Ok(policy) => policy,
        Err(message) => return pack_refusal(message),
    };
    serde_json::to_value(task_shell::weekly_flow_with_policy(
        &input_text,
        &loaded.pack,
        &policy,
    ))
    .unwrap_or_else(|e| serde_json::json!({ "ok": false, "error": format!("序列化结果失败：{}", e) }))
}

/// 一般文案 → 结构建议（缺信息时只列待确认，不强行套模板）。
#[tauri::command(async)]
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
    let policy = match current_rule_policy(&state) {
        Ok(policy) => policy,
        Err(message) => return pack_refusal(message),
    };
    serde_json::to_value(task_shell::writing_flow_with_policy(
        &purpose,
        &audience,
        &requirements,
        &facts,
        &loaded.pack,
        &policy,
    ))
    .unwrap_or_else(|e| {
        serde_json::json!({ "ok": false, "error": format!("序列化结果失败：{}", e) })
    })
}

/// 项目跟进 → 汇总（含逾期判定；today 留空用 UTC 当天）。
#[tauri::command(async)]
fn task_project_summary(
    state: State<'_, SharedHost>,
    input_text: String,
    today: Option<String>,
) -> serde_json::Value {
    let loaded = match current_flow_pack(&state) {
        Ok(loaded) => loaded,
        Err(message) => return pack_refusal(message),
    };
    let policy = match current_rule_policy(&state) {
        Ok(policy) => policy,
        Err(message) => return pack_refusal(message),
    };
    serde_json::to_value(task_shell::project_flow_with_policy(
        &input_text,
        today.as_deref(),
        &loaded.pack,
        &policy,
    ))
    .unwrap_or_else(|e| serde_json::json!({ "ok": false, "error": format!("序列化结果失败：{}", e) }))
}

/// 保存离线场景成果（不覆盖已有文件，必须人工确认）。
#[tauri::command(async)]
fn task_save_offline(request: task_shell::SaveOfflineRequest) -> Result<serde_json::Value, String> {
    task_shell::save_offline_artifact(&request, task_shell::now_secs())
        .map(|receipt| serde_json::json!(receipt))
}

/// 列出已保存的离线成果（只读元数据，仓库外数据根）。
#[tauri::command(async)]
fn task_list_offline_artifacts(
    include_subdirs: Option<bool>,
) -> Result<serde_json::Value, String> {
    task_shell::list_offline_artifacts_with(include_subdirs.unwrap_or(false))
        .map(|items| serde_json::json!({ "items": items }))
}

/// 重新打开一条已保存的离线成果（只读，不修改原文件）。
#[tauri::command(async)]
fn task_read_offline_artifact(file_name: String) -> Result<serde_json::Value, String> {
    task_shell::read_offline_artifact(&file_name)
        .map(|item| serde_json::json!(item))
}

/// 只读读取用户**明确给出路径**的一个材料文件（OD-ARCH-026）。
///
/// 一次一个文件，不列目录、不递归、不解析数据库；绝对路径、普通文件、
/// 扩展名白名单、大小上限与 UTF-8 全部由 task_shell::read_material 强制。
#[tauri::command(async)]
fn task_read_material(path: String) -> Result<serde_json::Value, String> {
    task_shell::read_material(&path).map(|item| serde_json::json!(item))
}

// ---------------------------------------------------------------------------
// 对话附件与 @ 引用（OD-ARCH-077 S1）
//
// 只允许两个来源根：当前成果根与库根（OD-ARCH-086）。根路径由宿主解析并校验，
// 不接受前端指定根；列表有上限，读取复用 read_material 的全部校验。
// ---------------------------------------------------------------------------

/// 当前允许引用的来源根（成果根 + 库根），不存在的不列出。
#[tauri::command(async)]
fn task_reference_roots(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let artifact_root = settings::effective_root(&config_dir)?.root;
    let library = settings::effective_library_root(&config_dir).ok().map(|e| e.root);
    let mut roots = vec![serde_json::json!({
        "kind": "artifacts",
        "label": "成果目录",
        "path": artifact_root.display().to_string(),
        "exists": artifact_root.is_dir(),
    })];
    if let Some(library) = library {
        if library != artifact_root {
            roots.push(serde_json::json!({
                "kind": "library",
                "label": "库根",
                "path": library.display().to_string(),
                "exists": library.is_dir(),
            }));
        }
    }
    Ok(serde_json::json!({ "roots": roots }))
}

/// 列出可引用文件（只读；有界遍历，不创建目录）。
#[tauri::command(async)]
fn task_list_references(
    state: State<'_, SharedHost>,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let artifact_root = settings::effective_root(&config_dir)?.root;
    let library_root = settings::effective_library_root(&config_dir)?.root;
    let roots = task_shell::reference_roots(&artifact_root, Some(&library_root));
    let items = task_shell::list_reference_files(
        &roots,
        query.as_deref(),
        limit.unwrap_or(60),
    )?;
    Ok(serde_json::json!({
        "items": items,
        "limit": task_shell::REFERENCE_LIST_LIMIT,
        "roots": roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
    }))
}

/// 读取一个被引用的附件正文与来源事实；越界路径由宿主拒绝。
#[tauri::command(async)]
fn task_read_reference(
    state: State<'_, SharedHost>,
    path: String,
) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let artifact_root = settings::effective_root(&config_dir)?.root;
    let library_root = settings::effective_library_root(&config_dir)?.root;
    let roots = task_shell::reference_roots(&artifact_root, Some(&library_root));
    task_shell::read_reference_material(&path, &roots).map(|item| serde_json::json!(item))
}

/// 读取用户通过**系统文件选择框**显式挑出的附件（OD-ARCH-078 S1）。
///
/// 与 `task_read_reference` 的区别只在来源范围：这里不要求落在成果根或知识库内，
/// 因为文件是用户在文件框里亲手选的（桌面、下载目录等都合法）。
/// 禁区（DSH / agent / 仓库）、普通文件、大小上限、真实 UTF-8 文本仍由
/// `task_shell::read_attachment` 强制；二进制一律如实报错，不生成假摘要。
#[tauri::command(async)]
fn task_read_attachment(path: String) -> Result<serde_json::Value, String> {
    task_shell::read_attachment(&path).map(|item| serde_json::json!(item))
}

// ---------------------------------------------------------------------------
// 待确认写操作（OD-ARCH-046 写确认制）
//
// 模型 / 定时任务想写文件时，先由前端提交一条“待确认记录”（含动作摘要、影响范围
// 与完整待写正文）。宿主把它存到应用数据根，**不写成果文件**。
// 用户在确认卡片上点“确认”后，宿主重新从磁盘读取该记录、校验未过期未消费、
// 正文与记录一致，才真正落盘；写失败则保留 pending 状态，不会标成成功。
//
// 不做“前端说已确认就放行”：校验只认磁盘上的记录。
// ---------------------------------------------------------------------------

/// 同一时刻只允许一个确认动作在执行，避免双击/重放把同一条记录写两次。
static PENDING_CONFIRM_LOCK: Mutex<()> = Mutex::new(());

fn pending_write_dir(state: &State<'_, SharedHost>) -> std::path::PathBuf {
    let host = state.lock().unwrap();
    pending_writes::dir(&host.paths.engine_state)
}

/// 场景状态汇总（OD-ARCH-060 S1/S5）。
///
/// 只读取宿主机自己落盘的两类元数据：待确认写入记录（不含正文）与成果目录列表
/// （只有路径、大小、时间）。不读成果正文、不调模型、不写任何文件。
/// 传入 `scenarios` 时只算这几项；缺省时算三个已知场景。
#[tauri::command(async)]
fn scene_overview(
    state: State<'_, SharedHost>,
    scenarios: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let now = task_shell::now_secs();
    let dir = pending_write_dir(&state);
    let (records, broken) = pending_writes::list(&dir, now)?;
    let views: Vec<pending_writes::PendingWriteView> =
        records.iter().map(|r| r.view(now)).collect();
    // 成果列表读失败时不能当作「零成果」：如实报错，让界面去显示未知。
    let artifacts = task_shell::list_offline_artifacts_with(true)?;
    let wanted: Vec<String> = scenarios.unwrap_or_else(|| {
        vec![
            "weekly_report".to_string(),
            "material_writing".to_string(),
            "project_management".to_string(),
        ]
    });
    let items: Vec<scene_overview::SceneOverview> = wanted
        .iter()
        .map(|s| {
            let mut one = scene_overview::summarize(s, &views, &artifacts, now);
            one.pending_broken = broken;
            one
        })
        .collect();
    Ok(serde_json::json!({
        "ok": true,
        "now": now,
        "items": items,
    }))
}

/// 登记一条待确认写操作。只有模型 / 定时来源可以登记；人工保存不走这里。
#[tauri::command(async)]
fn pending_write_create(
    state: State<'_, SharedHost>,
    request: pending_writes::CreatePendingWrite,
) -> Result<serde_json::Value, String> {
    let dir = pending_write_dir(&state);
    let now = task_shell::now_secs();
    pending_writes::create(&dir, &request, now)
        .map(|record| serde_json::json!(record.view(now)))
}

/// 列出待确认记录（新的在前）。损坏的记录只计数，不伪造内容。
#[tauri::command(async)]
fn pending_write_list(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let dir = pending_write_dir(&state);
    let now = task_shell::now_secs();
    let (records, broken) = pending_writes::list(&dir, now)?;
    let items: Vec<pending_writes::PendingWriteView> =
        records.iter().map(|record| record.view(now)).collect();
    Ok(serde_json::json!({ "items": items, "broken": broken }))
}

/// 用户确认后执行写入：记录 → 校验 → 落盘 → 标记已消费。
#[tauri::command(async)]
fn pending_write_confirm(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<serde_json::Value, String> {
    let _guard = PENDING_CONFIRM_LOCK
        .lock()
        .map_err(|_| "确认通道不可用（锁已损坏），未执行写入。".to_string())?;
    let dir = pending_write_dir(&state);
    let now = task_shell::now_secs();
    // 先载入以拿到登记时的正文；随后用同一份正文回校验，任何前端改动都会被拒。
    let record = pending_writes::load(&dir, &id)?;
    let validated =
        pending_writes::validate_for_write(&dir, &id, &record.payload, now)?;
    let request = task_shell::SaveOfflineRequest::from_record(&validated);
    // 写失败时保持 pending：调用方能看到错误，也能重试或放弃。
    let receipt = task_shell::save_offline_artifact(&request, now)?;
    let settled = pending_writes::mark_resolved(&dir, &id, "consumed", task_shell::now_secs())?;
    Ok(serde_json::json!({
        "ok": true,
        "id": settled.id,
        "status": settled.status,
        "path": receipt.path,
        "bytes": receipt.bytes,
        "written_at": receipt.written_at,
    }))
}

/// 放弃一条待确认写操作：只改记录状态，不产生任何成果文件。
#[tauri::command(async)]
fn pending_write_reject(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<serde_json::Value, String> {
    let dir = pending_write_dir(&state);
    let settled =
        pending_writes::mark_resolved(&dir, &id, "rejected", task_shell::now_secs())?;
    Ok(serde_json::json!({ "ok": true, "id": settled.id, "status": settled.status }))
}

// ---------------------------------------------------------------------------
// 应用运行期调度（OD-ARCH-046）
//
// 宿主只做配置与到点判定；触发由前端运行期定时器负责。不常驻、不起系统定时、
// 不开线程。心跳只做本地确定性扫描（与手动逾期扫描同一条代码路径）。
// ---------------------------------------------------------------------------

fn scheduler_config_dir(state: &State<'_, SharedHost>) -> std::path::PathBuf {
    let host = state.lock().unwrap();
    host.paths.config.clone()
}

fn scheduler_payload(config: &scheduler::SchedulerConfig) -> serde_json::Value {
    serde_json::json!({
        "config": config,
        "hint": scheduler::schedule_hint(config),
    })
}

#[tauri::command(async)]
fn scheduler_config_get(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let dir = scheduler_config_dir(&state);
    scheduler::load(&dir).map(|config| scheduler_payload(&config))
}

/// 保存调度配置。校验失败整次拒绝，不写半份文件；不覆盖用户成果。
#[tauri::command(async)]
fn scheduler_config_set(
    state: State<'_, SharedHost>,
    config: scheduler::SchedulerConfig,
) -> Result<serde_json::Value, String> {
    let dir = scheduler_config_dir(&state);
    scheduler::write(&dir, &config)?;
    scheduler::load(&dir).map(|saved| scheduler_payload(&saved))
}

/// 到点判定：一次返回复盘与心跳两种状态，前端据此决定是否触发。
#[tauri::command(async)]
fn scheduler_tick(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let dir = scheduler_config_dir(&state);
    let config = scheduler::load(&dir)?;
    let now = task_shell::now_secs();
    Ok(serde_json::json!({
        "now": now,
        "review_due": scheduler::review_due(&config, now),
        "heartbeat_due": scheduler::heartbeat_due(&config, now),
        "config": config,
        "hint": scheduler::schedule_hint(&config),
    }))
}

/// 记录一次复盘的实际执行时间（用于“错过只补跑一次”与去重）。
#[tauri::command(async)]
fn scheduler_mark_reviewed(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let dir = scheduler_config_dir(&state);
    let mut config = scheduler::load(&dir)?;
    config.last_review_at = task_shell::now_secs();
    scheduler::write(&dir, &config)?;
    scheduler::load(&dir).map(|saved| scheduler_payload(&saved))
}

/// 记录一次心跳的实际执行时间。心跳本身不产生文件。
#[tauri::command(async)]
fn scheduler_mark_heartbeat(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let dir = scheduler_config_dir(&state);
    let mut config = scheduler::load(&dir)?;
    config.last_heartbeat_at = task_shell::now_secs();
    scheduler::write(&dir, &config)?;
    scheduler::load(&dir).map(|saved| scheduler_payload(&saved))
}

/// 心跳扫描：与手动“逾期扫描”共用 project_flow_with，结果必须一致。
#[tauri::command(async)]
fn scheduler_heartbeat_scan(
    state: State<'_, SharedHost>,
    input_text: String,
    today: Option<String>,
) -> Result<serde_json::Value, String> {
    let loaded = current_flow_pack(&state)?;
    let policy = current_rule_policy(&state)?;
    let scan = scheduler::heartbeat_scan_with(
        &input_text,
        today.as_deref(),
        &loaded.pack,
        &policy,
    );
    serde_json::to_value(scan).map_err(|e| format!("序列化心跳结果失败：{}", e))
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
        vault_index: Arc::new(Mutex::new(None)),
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

    // OD-ARCH-050：端点事实用的只读快照；构造时取一次，之后不再加宿主锁。
    let snapshot = {
        let host = shared.lock().unwrap();
        HostSnapshot {
            paths: host.paths.clone(),
            config: Mutex::new(host.bridge.supervisor().lock().unwrap().config().clone()),
            lifecycle: Arc::clone(&host.lifecycle),
        }
    };

    tauri::Builder::default()
        // OD-ARCH-076 S1：必须注册为第一个插件。第二次启动时把已有主窗口叫到前台，
        // 然后由插件结束这个新进程（不在这里手动 exit，避免与插件自身的收尾竞争）。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        // OD-ARCH-078 S1：附件入口需要系统文件选择框（只用于「挑文件」，不给前端文件系统权限）。
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::clone(&shared))
        .manage(snapshot)
        // OD-ARCH-050：DSH 设置服务（临时端口实例 + 应用内窗口）。
        .manage(Arc::new(dsh_settings::SettingsService::new()))
        .invoke_handler(tauri::generate_handler![
            engine_describe,
            engine_health,
            engine_submit_task,
            engine_task_status,
            engine_task_result,
            engine_cancel_task,
            engine_approval_respond,
            engine_question_answer,
            engine_shutdown,
            engine_list_sessions,
            engine_recover_session,
            engine_fork_session,
            engine_task_records,
            engine_list_models,
            engine_recover_task,
            engine_admission_status,
            model_permission_set,
            model_permission_revoke_all,
            app_paths_info,
            profile_decl_load,
            profile_decl_digest,
            profile_decl_open,
            profile_decl_save,
            profile_choice_view,
            profile_choice_set,
            profile_rules_load,
            profile_rules_save,
            profile_rules_restore_default,
            model_settings_load,
            model_settings_probe,
            model_settings_save,
            task_types,
            task_samples,
            task_explanation_rules,
            task_local_root_info,
            task_set_artifact_root,
            task_library_root_info,
            task_set_library_root,
            task_open_artifact_root,
            open_external_url,
            capability_view,
            capability_set_plugin,
            capability_open_skills,
            category_settings_load,
            category_update,
            category_add,
            category_remove,
            category_init_folders,
            task_list_model_routes,
            task_set_model_route,
            task_clear_model_route,
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
            task_reference_roots,
            task_list_references,
            task_read_reference,
            task_read_attachment,
            knowledge_vault_status,
            knowledge_vault_set_root,
            knowledge_vault_clear_root,
            knowledge_vault_scan,
            knowledge_vault_read_note,
            knowledge_vault_parse_note,
            knowledge_vault_create_note,
            knowledge_vault_save_note,
            knowledge_vault_search,
            knowledge_vault_open_attachment,
            knowledge_vault_rebuild_index,
            knowledge_vault_index_status,
            knowledge_vault_note_links,
            thread_store_load,
            thread_store_save,
            thread_store_delete,
            project_items_read,
            project_items_save,
            pending_write_create,
            pending_write_list,
            pending_write_confirm,
            pending_write_reject,
            scene_overview,
            scheduler_config_get,
            scheduler_config_set,
            scheduler_tick,
            scheduler_mark_reviewed,
            scheduler_mark_heartbeat,
            scheduler_heartbeat_scan,
            worklog_append,
            worklog_set_destination,
            worklog_redate,
            worklog_read_week,
            worklog_archive_list,
            worklog_read_report,
        ])
        .setup(|app| {
            // 启动时只设置窗口标题，不拉起引擎（引擎按需启动）。
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("OmniDesk Discipline");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // OD-ARCH-076 S2：DSH 原生设置窗口已移除，只有主窗口会走到这里。
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
        .build(tauri::generate_context!())
        .expect("启动 OmniDesk 桌面应用失败")
        .run(|app, event| {
            // OD-ARCH-050：应用退出时停掉自起的 DSH 设置服务，避免再留占用端口的僵尸进程。
            if let tauri::RunEvent::Exit = event {
                if let Some(service) = app.try_state::<Arc<dsh_settings::SettingsService>>() {
                    let record = app
                        .try_state::<HostSnapshot>()
                        .map(|snap| snap.paths.engine_state.join("dsh-settings.json"))
                        .unwrap_or_default();
                    service.stop(&record);
                }
            }
        });
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
                threads: self.base.join("app/threads"),
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
                // 这组用例只验证宿主包装与锁行为：假引擎不接真实 DSH，
                // 端点分类仍按 ACP Profile + 合成覆盖层跑，保持原有夹具。
                profile: "acp".to_string(),
                channel: "acp".to_string(),
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
                vault_index: Arc::new(Mutex::new(None)),
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
            "fn engine_list_models",
            "fn engine_recover_task",
            "fn engine_fork_session",
        ] {
            let start = body
                .find(cmd)
                .unwrap_or_else(|| panic!("找不到宿主命令 {}", cmd));
            let rest = &body[start..];
            let end = rest[1..]
                .find("\n#[tauri::command")
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
                .find("\n#[tauri::command")
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

    /// OD-ARCH-050：读端点事实不得抢宿主锁（否则引擎冷启动期间会被堵成「未识别」）。
    #[test]
    fn admission_status_reads_snapshot_without_host_lock() {
        let src = include_str!("lib.rs");
        let body = src.split("\n#[cfg(test)]").next().expect("源码前缀");
        let start = body
            .find("fn engine_admission_status")
            .expect("找不到 engine_admission_status");
        let rest = &body[start..];
        let end = rest[1..]
            .find("\n#[tauri::command")
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let code = rest[..end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("host.lock()"),
            "读端点事实不得抢宿主锁：引擎冷启动最长持有 90 秒"
        );
        assert!(
            code.contains("HostSnapshot"),
            "端点事实必须从只读快照读取"
        );
    }

    /// OD-ARCH-076 S2：DSH 原生设置窗口已移除，窗口收尾逻辑只服务主窗口。
    ///
    /// 旧测试检查「非主窗口不触发引擎收尾」。窗口入口删除后该分支已成死代码，
    /// 这里改为钉住新事实：收尾回调只处理主窗口，且不再存在设置窗口事件分支。
    #[test]
    fn window_teardown_has_no_dsh_settings_window_branch() {
        let src = include_str!("lib.rs");
        let start = src
            .find(".on_window_event(|window, event| {")
            .expect("找不到窗口事件回调");
        let rest = &src[start..];
        let end = rest
            .find(".build(tauri::generate_context!")
            .expect("找不到窗口事件回调的结束位置");
        let callback = &rest[..end];
        assert!(
            !callback.contains("dsh-settings-closed"),
            "DSH 原生设置窗口入口已删除，不应残留它的事件"
        );
        assert!(
            callback.contains("on_window_destroyed_shutdown("),
            "主窗口关闭仍必须收尾引擎"
        );
    }

    /// OD-ARCH-049 第 3 步：执行入口终检必须发生在触达引擎之前。
    ///
    /// 提交入口在红线扫描之后、`with_engine_call` 之前检查“同场景未决确认卡”：
    /// 命中即整条任务拒绝（错误码 `pending_write_blocked`），不进入引擎、不做部分执行。
    #[test]
    fn submit_entry_checks_pending_scene_gate_before_touching_engine() {
        let src = include_str!("lib.rs");
        let body = src.split("\n#[cfg(test)]").next().expect("源码前缀");
        let start = body
            .find("fn engine_submit_task")
            .expect("找不到 engine_submit_task");
        let rest = &body[start..];
        let end = rest[1..]
            .find("\n#[tauri::command")
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let cmd = &rest[..end];
        let gate = cmd
            .find("pending_writes::scene_gate(")
            .expect("提交入口必须做同场景未决确认卡终检");
        let engine = cmd
            .find("with_engine_call(")
            .expect("提交入口必须经 with_engine_call");
        assert!(
            gate < engine,
            "终检必须先于触达引擎执行（否则会先跑任务再拒绝）"
        );
        assert!(
            cmd.contains("\"pending_write_blocked\""),
            "拒绝必须带可识别的错误码，便于界面如实说明原因"
        );
    }

    /// 宿主命令一律不得在 Tauri 主线程上等待（OD-ARCH-048 卡死回归）。
    ///
    /// Tauri 2 的同步命令（`#[tauri::command]` 直接修饰同步 fn）在 UI 线程执行。
    /// 本文件几乎所有命令都会取 `SharedHost` 的互斥锁，并可能等引擎冷启动
    /// （实测可达数十秒）。只要有一条同步命令占住 UI 线程，窗口就无法重绘、
    /// 场景无法切换，只能强制退出——这正是用户报告的卡死现象。
    ///
    /// 因此本测试把“所有命令都标注 `(async)`”固定为不变量：新增命令若漏写，
    /// 会在此处失败，而不是等到用户端整窗冻结才发现。
    #[test]
    fn every_tauri_command_runs_off_the_ui_thread() {
        let src = include_str!("lib.rs");
        // 生产代码只到 `run()` 之后第一个测试模块为止；测试模块内的字符串不算。
        let run_at = src.find("pub fn run()").expect("找不到 run()");
        let prefix_end = src[run_at..]
            .find("\n#[cfg(test)]")
            .map(|i| run_at + i + 1)
            .unwrap_or(src.len());
        let production = &src[..prefix_end];

        // 逐行扫描：属性行之后的第一个 `fn` 才是它修饰的函数。
        // 不能用 `split("\nfn ")` 之类的做法——会跨过下一条命令的属性，
        // 把别的函数名误配给本条属性
        // 先剥掉注释行，避免注释里出现的属性样例被当成真实命令。
        let code: Vec<&str> = production
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect();
        let mut blocking: Vec<String> = Vec::new();
        for (index, line) in code.iter().enumerate() {
            if line.trim() != "#[tauri::command]" {
                continue;
            }
            let signature = code[index + 1..]
                .iter()
                .map(|l| l.trim())
                .find(|l| {
                    let l = l.trim_start_matches("pub ");
                    l.starts_with("fn ") || l.starts_with("async fn ")
                });
            let Some(signature) = signature else {
                blocking.push("<unknown>".to_string());
                continue;
            };
            let bare = signature.trim_start_matches("pub ");
            // 已经是 `async fn` 的命令本来就跑在异步运行时上，无需属性标注。
            if bare.starts_with("async fn ") {
                continue;
            }
            blocking.push(
                bare.trim_start_matches("fn ")
                    .split('(')
                    .next()
                    .unwrap_or("<unknown>")
                    .trim()
                    .to_string(),
            );
        }

        assert!(
            blocking.is_empty(),
            "以下命令仍是同步命令，会占用 UI 线程并导致整窗卡死：{:?}。\
             请改为 #[tauri::command(async)]（同步函数体保持不变即可）。",
            blocking
        );
    }
}
