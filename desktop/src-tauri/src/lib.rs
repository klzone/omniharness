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
pub mod availability;
pub mod flow_pack;
pub mod knowledge_governance;
pub mod knowledge_quality;
pub mod knowledge_review;
pub mod knowledge_vault;
pub mod knowledge_ingest;
pub mod knowledge_link;
pub mod knowledge_link_store;
pub mod material_store;
pub mod material_query;
mod local_http;
pub mod model_settings;
pub mod model_routes;
pub mod pending_writes;
pub mod platform;
pub mod project_items;
pub mod project_store;
pub mod process_owner;
pub mod profile_decl;
pub mod profile_choice;
pub mod profile_assets;
pub mod profile_categories;
pub mod profile_seed;
pub mod profile_lifecycle;
pub mod profile_starter;
pub mod profile_rules;
pub mod priority_rules;
pub mod quicklog;
pub mod route_classifier;
pub mod schedule_store;
pub mod scheduling;
pub mod scene_overview;
pub mod scheduler;
pub mod settings;
pub mod sha256;
pub mod task_shell;
pub mod task_store;
pub mod task_extract;
pub mod thread_store;
pub mod vault_note;
pub mod vault_link;
pub mod vault_index;
pub mod vault_open;
pub mod vault_search;
pub mod vault_write;
pub mod vault_archive;
pub mod worklog;
pub mod worklog_records;
pub mod worklog_migrate;
pub mod work_cleanup;
pub mod writing_store;
pub mod writing_records;
pub mod writing_collect;
pub mod export_engine;
pub mod document_store;
pub mod writing_parse;
pub mod writing_outline;
pub mod writing_section;
pub mod writing_review;
pub mod fact_check;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use app_paths::AppPaths;
use engine_bridge::{EngineBridge, EventSink};
use engine_lifecycle::EngineLifecycle;
use engine_supervisor::{EngineConfig, EngineSupervisor};
use task_shell::{SaveArtifactRequest, UsageRecordInput};

/// UI 事件通道名。UI 只订阅这一个通道，拿到的都是归一化事件。
const TASK_EVENT_CHANNEL: &str = "omni://task-event";
/// 切换 Profile 后后台补装插件的结果通道（OD-ARCH-121）。
///
/// 与任务事件分开：这条只在装配收尾时出现一次，界面按它刷新缺件卡与状态条。
const PROFILE_PLUGINS_EVENT: &str = "omni://profile-plugins";
/// 用户在设置页保存场景声明后广播（OD-ARCH-123）。
///
/// **只覆盖「应用内保存」这一条路径**：外部手改文件仍由指纹轮询提示、不热重载
/// （OD-ARCH-052 第 2 步的边界）。分开的理由是「谁改的」决定「界面该不该跟着变」：
/// 用户刚点完保存，界面还按旧入口显示，等于告诉用户刚保存的东西没生效。
const PROFILE_DECL_EVENT: &str = "omni://profile-decl";
/// 自定义 Profile 清单变化后广播（OD-ARCH-125）。
///
/// 新建、移除、从回收站恢复都会改动 `profiles/index.v1.json`。顶栏只在
/// 「清单为空」时拉一次，之后不再重读；设置页里恢复完再回顶栏，用户
/// 看到的还是旧清单，切不过去。这条事件只表示「清单变了，重读一次」，
/// 不带任何业务数据。
const PROFILE_REGISTRY_EVENT: &str = "omni://profile-registry";

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

/// 换 Profile 后把新配置同时装进 supervisor 与只读快照（OD-ARCH-110）。
///
/// 抽成函数的原因：两处配置必须来自**同一次** `EngineConfig::from_env`，分别解析
/// 可能得到不同结果；而「界面 / 准入 / 引擎启动」三处读到的 Profile 必须一致。
/// 锁顺序与 `sync_model_overlay` 相同：宿主锁 → supervisor 锁，快照锁独立。
fn adopt_engine_config(
    supervisor: &Arc<Mutex<EngineSupervisor>>,
    snapshot: Option<&Mutex<EngineConfig>>,
    fresh: EngineConfig,
) {
    *supervisor.lock().unwrap().config_mut() = fresh.clone();
    if let Some(config) = snapshot {
        *config.lock().unwrap() = fresh;
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
    let rules_view = profile_rules::load_rules(&profile_asset_dir(&state));
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
                let masker_stats = obj.remove("masker");
                obj.insert(
                    "privacy_masked".to_string(),
                    serde_json::json!({
                        "id_card": privacy_masked.id_card,
                        "phone": privacy_masked.phone,
                        "masker": masker_stats,
                    }),
                );
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
// Profile 声明（OD-ARCH-051 S3 / OD-ARCH-106）
//
// 只决定 Dock 上显示哪些业务场景入口，不加载插件、不改模型路由与 Skill。
// 落点是 `<应用数据根>/profiles/<Profile 名>/decl.yml`：声明**随 Profile 挂载**，
// 基础标准（omnidesk-base）下没有这个文件，界面只有平台固有入口。
// 文件不由宿主自动创建；加载/校验失败一律返回错误，由界面显式提示。
// ---------------------------------------------------------------------------

/// 当前 Profile 的界面资产范围（OD-ARCH-106）。
///
/// `profile` 取**运行中引擎实例**的 Profile（supervisor 里的内存配置），而不是
/// 从设置文件重新推算：用户刚在顶栏改了 Profile 但还没重启时，磁盘上已经是新名字、
/// 引擎里仍是旧名字。若按磁盘算，界面会提前挂上专业场景，而模型仍在干净底座里跑，
/// 等于把场景提示词与 Skill 注入到一个没有对应插件的引擎上。
/// 引擎字段为空（异常配置）时按通道标准 Profile 处理，避免误挂专业规则。
struct ProfileAssetScope {
    profiles_root: std::path::PathBuf,
    profile: String,
    standard: bool,
}

fn profile_asset_scope(state: &State<'_, SharedHost>) -> ProfileAssetScope {
    let (app_root, running_profile, channel) = {
        let host = state.lock().unwrap();
        let cfg = host.bridge.supervisor().lock().unwrap().config().clone();
        (host.paths.root.clone(), cfg.profile, cfg.channel)
    };
    let standard_name = profile_choice::standard_profile(&channel).to_string();
    let profile = {
        let trimmed = running_profile.trim();
        if trimmed.is_empty() {
            standard_name.clone()
        } else {
            trimmed.to_string()
        }
    };
    let standard = profile == standard_name;
    ProfileAssetScope {
        profiles_root: app_root.join("profiles"),
        profile,
        standard,
    }
}

/// 以当前 Profile 名为准的资产目录。
fn profile_asset_dir(state: &State<'_, SharedHost>) -> std::path::PathBuf {
    let scope = profile_asset_scope(state);
    scope.profiles_root.join(&scope.profile)
}

/// 一次性兼容迁移（OD-ARCH-106）：旧单文件 → `profiles/<Profile 名>/`。
///
/// 只对非标准 Profile 执行；新位置已有文件一律不动；旧文件保留作备份。
/// 两种资产各自返回迁移失败原因，交给对应界面显示——否则用户只会看到
/// 「没有声明」或「未加载（纯净底座）」，不知道是自己的旧文件写坏了。
/// 幂等：新位置已有文件时两次调用都直接跳过。
fn migrate_legacy_profile_assets(
    state: &State<'_, SharedHost>,
) -> (Option<String>, Option<String>) {
    let scope = profile_asset_scope(state);
    if scope.standard {
        return (None, None);
    }
    let channel = {
        let host = state.lock().unwrap();
        host.bridge.supervisor().lock().unwrap().config().channel.clone()
    };
    let standard_name = profile_choice::standard_profile(&channel).to_string();
    let decl_error =
        profile_decl::migrate_legacy_decl(&scope.profiles_root, &scope.profile, &standard_name).err();
    let rules_error =
        profile_rules::migrate_legacy_rules(&scope.profiles_root, &scope.profile, &standard_name).err();
    (
        decl_error.map(|e| {
            format!(
                "旧声明 `{}` 未迁移（{}）。修好缩进后在设置页保存，或直接改这份旧文件的格式。",
                scope
                    .profiles_root
                    .join(profile_decl::LEGACY_PROFILE_DECL_FILE)
                    .display(),
                e
            )
        }),
        rules_error.map(|e| {
            format!(
                "旧规则包 `{}` 未迁移（{}）。修正格式后可在设置 →「规则资产」重新保存。",
                scope
                    .profiles_root
                    .join(profile_rules::LEGACY_PROFILE_RULES_FILE)
                    .display(),
                e
            )
        }),
    )
}

#[tauri::command(async)]
fn profile_decl_load(
    state: State<'_, SharedHost>,
) -> Result<Option<profile_decl::LoadedDecl>, String> {
    let (migration_error, _) = migrate_legacy_profile_assets(&state);
    let dir = profile_asset_dir(&state);
    let loaded = profile_decl::load_decl(&dir)?;
    // 新位置没有声明、旧文件又搬不过来时，把迁移原因说出来；否则用户看到的是
    // 「这个 Profile 没有场景声明」，与「旧文件写坏了」是两件事。
    if loaded.is_none() {
        if let Some(reason) = migration_error {
            return Err(reason);
        }
    }
    Ok(loaded)
}

/// 只读：声明文件当前的内容指纹（OD-ARCH-052 第 2 步）。
///
/// 界面拿它和启动时载入的指纹比对，判断用户是否已在磁盘上改过声明——
/// 改动只在下次启动生效，因此要显式提示"重启应用生效"。
/// 这条命令不创建文件、不改配置、不启动引擎。
#[tauri::command(async)]
fn profile_decl_digest(state: State<'_, SharedHost>) -> Result<Option<String>, String> {
    profile_decl::decl_digest(&profile_asset_dir(&state))
}

/// 只读：生成一份起步场景声明模板（OD-ARCH-122）。
///
/// 只返回文本，**不写盘**：用户还要在设置页里点「校验并保存」才会落文件。
/// 当前 Profile 已经有声明时额外告知，让界面提醒「保存会覆盖既有内容」。
/// 基础标准下也允许取模板（它只是文字），保存时仍走原有的校验与备份。
#[tauri::command(async)]
fn profile_decl_starter(
    state: State<'_, SharedHost>,
    label: Option<String>,
) -> Result<serde_json::Value, String> {
    let (app_root, running) = {
        let host = state.lock().unwrap();
        let cfg = host.bridge.supervisor().lock().unwrap().config().clone();
        (host.paths.root.clone(), cfg.profile)
    };
    let (channel, _dsh_home) = profile_choice_scope()?;
    let standard = profile_choice::standard_profile(&channel).to_string();
    let dir = app_root.join("profiles").join(&running);
    let existing = profile_decl::read_text(&dir)?;
    let display = label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(running.as_str())
        .to_string();
    let text = crate::profile_starter::decl_text(&display)?;
    Ok(serde_json::json!({
        "ok": true,
        "profile": running,
        "standard": running == standard,
        "has_existing": existing.is_some(),
        "text": text,
        "note": if running == standard {
            "基础标准是干净底座，不挂业务场景：模板可以看，但保存会被拒绝。请先在顶栏切到 Profile。"
        } else if existing.is_some() {
            "这个 Profile 已经有场景声明：模板只是参考，保存会覆盖现有内容（旧文件会先备份）。"
        } else {
            "已生成起步模板；改好后点「校验并保存」，工作台入口会立刻更新。"
        },
    }))
}

/// 在系统编辑器里打开场景声明文件（OD-ARCH-053 S3）。**不**接受路径参数。
#[tauri::command(async)]
fn profile_decl_open(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    migrate_legacy_profile_assets(&state);
    let dir = profile_asset_dir(&state);
    let Some(loaded) = profile_decl::load_decl(&dir)? else {
        return Err(
            "当前 Profile 没有场景声明（基础标准下只显示平台固有入口）。切到带场景的 Profile 后再编辑。"
                .to_string(),
        );
    };
    let path = std::path::PathBuf::from(&loaded.path);
    platform::open_target(&path).map_err(|e| format!("无法打开声明文件（{}）", e))?;
    Ok(serde_json::json!({
        "ok": true,
        "opened": loaded.path,
        "note": "已在系统编辑器打开声明文件；保存后需重启应用生效。",
    }))
}

/// 保存场景声明（OD-ARCH-053 S3）：先校验，校验通过才备份并原子写。
///
/// 校验失败一律拒绝保存，文件原样保留；写前把旧文件复制成 `decl.yml.bak-<epoch>`。
///
/// OD-ARCH-123：基础标准拒绝保存（干净底座不挂业务场景，与 `profile_rules_restore_default`
/// 同一判据）；保存成功后广播 `omni://profile-decl`，前端 Dock 立刻换成新入口，无需重启。
#[tauri::command(async)]
fn profile_decl_save(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    text: String,
) -> Result<serde_json::Value, String> {
    let scope = profile_asset_scope(&state);
    let dir = scope.profiles_root.join(&scope.profile);
    let saved = save_decl_into(&dir, scope.standard, &text)?;
    let _ = app.emit(
        PROFILE_DECL_EVENT,
        serde_json::json!({
            "profile": scope.profile,
            "path": saved["path"].clone(),
            "digest": saved["digest"].clone(),
        }),
    );
    Ok(saved)
}

/// `profile_decl_save` 的可测内核（OD-ARCH-123）：只碰传进来的目录，不读全局状态。
///
/// `standard = true` 时直接拒绝，**不创建目录、不落文件**——基础标准是否为干净底座
/// 要能靠「目录在不在」复核。校验失败同样一字节不写。
fn save_decl_into(
    profiles_dir: &std::path::Path,
    standard: bool,
    text: &str,
) -> Result<serde_json::Value, String> {
    if standard {
        return Err(
            "基础标准是干净底座，不挂业务场景。请先在顶栏切到 Profile，再保存场景声明。"
                .to_string(),
        );
    }
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
        "note": "声明已保存，工作台入口已更新（不需要重启）。",
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
            let home = platform::home_dir()
                .ok_or_else(|| "无法确定用户主目录（HOME 与 USERPROFILE 均缺失）".to_string())?;
            home.join("OmniDesk")
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
        .map(|s| !profile_choice::is_selectable(&channel, &app_root, s))
        .unwrap_or(false);
    // 「本次启动实际生效」只认引擎配置；引擎配置读不出来时退回按已保存选择推算，
    // 并显式带上 warning，不把推算值当成事实。
    let (effective, warning) = match engine_supervisor::EngineConfig::from_env(&app_root) {
        Ok(config) => (config.profile, None),
        Err(e) => {
            let guess = profile_choice::resolve(&dsh_home, &channel, saved.as_deref(), Some(&app_root))
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
        "options": profile_choice::list_all(&dsh_home, &channel, &app_root),
        "custom": profile_lifecycle::load_registry(&app_root)
            .profiles
            .iter()
            .map(|p| serde_json::json!({
                "name": p.name,
                "label": p.label,
                "origin": p.origin,
                "created_at": p.created_at,
            }))
            .collect::<Vec<_>>(),
        "warning": warning,
        "note": "这里列出可选 Profile 与当前生效的 Profile；切换后会立即停止旧引擎并按新 Profile 运行。",
    }))
}

/// 保存 Profile 选择（None = 回到标准模式）。
///
/// OD-ARCH-109：不再「目录不存在就拒绝」，改为**先装配、再校验、后保存**：
/// 出厂 Profile（如 discipline）在全新数据根上也能一次切换成功，配置 / 场景 /
/// 规则 / Skill 自动补齐；基础标准仍然零写入。装配有漂移只提示、不阻断切换。
///
/// OD-ARCH-110：保存后**立即换引擎**——重读配置、同步 supervisor 与只读快照、
/// 在生命周期屏障内停掉旧引擎。下一条任务即按新 Profile 启动，场景声明 / 规则 /
/// 注入 Skill 都取运行中的 Profile，因此不用重启整个应用。
#[tauri::command(async)]
fn profile_choice_set(
    app: tauri::AppHandle,
    state: State<'_, SharedHost>,
    profile: Option<String>,
) -> Result<serde_json::Value, String> {
    let (config_dir, app_root, paths) = {
        let host = state.lock().unwrap();
        (host.paths.config.clone(), host.paths.root.clone(), host.paths.clone())
    };
    // 切换前的保存值：新配置解析失败时用它回滚，避免「磁盘写了新名字、引擎还是旧名字」。
    let previous_saved = crate::settings::load(&config_dir)
        .ok()
        .and_then(|s| s.engine_profile);
    let (channel, dsh_home) = profile_choice_scope()?;
    let standard = profile_choice::standard_profile(&channel).to_string();
    let chosen = match profile.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(name) => {
            if !crate::settings::is_valid_profile_name(name) {
                return Err(format!("Profile 名不合法：{:?}", name));
            }
            // 只允许通道白名单或已登记的自定义 Profile：否则会被启动时的 resolve 静默回落，
            // 界面显示「已切换」而引擎仍在别的 Profile 上跑。
            if !profile_choice::is_selectable(&channel, &app_root, name) {
                return Err(format!(
                    "「{}」不在可选的 Profile 清单里。请先在顶栏新建一个自定义 Profile。",
                    name
                ));
            }
            Some(name.to_string())
        }
    };
    // 装配（基础标准 / 自定义 Profile 都是空操作）。
    // 仓库根只在确实存在时传下去：分发后的二进制里 CARGO_MANIFEST_DIR 是编译期路径，
    // 在用户机器上并不存在，误传会让 Skill 装配误判为「源在仓库」。
    let repo_root = crate::task_shell::repo_root_hint()
        .ok()
        .filter(|p| p.is_dir());
    let assembly = match chosen.as_deref() {
        None => {
            // 切回基础标准也必须收敛 Skill（OD-ARCH-115）：DSH 把 <dsh_home>/skills/
            // 当全局用户根无条件扫描，不收回的话纪检技能仍会被加载，底座就不是干净的。
            // 没有登记项时这里是纯空操作，不建目录、不写文件。
            let scope = profile_assets::Scope {
                dsh_home: &dsh_home,
                app_root: &app_root,
                profile: &standard,
                repo_root: repo_root.as_deref(),
            };
            profile_assets::reconcile_managed_skills(&scope, &standard)
                .map_err(|e| format!("切回基础标准时整理 Skill 失败，未切换：{}", e))?;
            None
        }
        Some(name) => {
            let scope = profile_assets::Scope {
                dsh_home: &dsh_home,
                app_root: &app_root,
                profile: name,
                repo_root: repo_root.as_deref(),
            };
            let report = profile_assets::materialize(&scope, &standard).map_err(|e| {
                format!("Profile「{}」的资产装配失败，未切换：{}", name, e)
            })?;
            // 装配后登记归属，下次切走才知道该收回哪些目录。
            profile_assets::record_managed_skills(&scope, &standard)
                .map_err(|e| format!("Profile「{}」的 Skill 登记失败，未切换：{}", name, e))?;
            Some(report)
        }
    };
    // 装配后再校验目录：自定义 Profile 仍要求已存在的 package.json。
    if let Some(name) = chosen.as_deref() {
        let manifest = dsh_home.join("profiles").join(name).join("package.json");
        if !manifest.is_file() {
            return Err(format!(
                "隔离数据根里没有这个 Profile：{}（缺少 {}）。这是自定义 Profile，请先创建它。",
                name,
                manifest.display()
            ));
        }
    }
    // 分类表也随 Profile 走（OD-ARCH-116）：先回存离开的 Profile，再切入目标的表。
    // 放在写 engine_profile 之前，失败就整体不切换，避免「Profile 换了、分类还是旧的」。
    let categories = crate::profile_categories::apply_for_switch(
        &paths,
        previous_saved.as_deref(),
        chosen.as_deref(),
        &standard,
    )
    .map_err(|e| format!("Profile「{}」的分类表同步失败，未切换：{}", chosen.clone().unwrap_or_else(|| standard.clone()), e))?;
    crate::settings::set_engine_profile(&config_dir, chosen.as_deref())?;
    // 重读配置：设置已落盘，resolve 会选中新 Profile。读不出来就回滚保存值，
    // 让「设置文件」与「实际可用配置」保持一致，而不是让用户以为切换成功。
    let fresh = match engine_supervisor::EngineConfig::from_env(&app_root) {
        Ok(config) => config,
        Err(message) => {
            let _ = crate::settings::set_engine_profile(&config_dir, previous_saved.as_deref());
            return Err(format!(
                "Profile「{}」的引擎配置无法解析，切换已回滚：{}",
                chosen.clone().unwrap_or_else(|| standard.clone()),
                message
            ));
        }
    };
    let effective = fresh.profile.clone();
    // 换 Profile 等于换权限与插件集合：按「模型权限变更」同口径独占生命周期，
    // 旧引擎没有确认停止时保留屏障（失败关闭），绝不与新 Profile 混用。
    let lifecycle = {
        let host = state.lock().unwrap();
        Arc::clone(&host.lifecycle)
    };
    let change = lifecycle.begin_change();
    // 屏障先立起来再换配置：否则「换完配置、还没停旧引擎」这一小段时间里，
    // 新任务会拿到新 Profile 的规则，却被送进仍在运行的旧 Profile 引擎。
    // 把新配置同步到两处（supervisor 决定启动参数、快照供准入与端点事实），
    // 必须用同一份 `fresh`，否则三处会看到不同的 Profile。
    {
        let host = state.lock().unwrap();
        let supervisor = host.bridge.supervisor();
        let snapshot = app.try_state::<HostSnapshot>();
        adopt_engine_config(&supervisor, snapshot.as_ref().map(|s| &s.config), fresh);
    }
    let engine_stopped = match stop_engine(state.inner(), Duration::from_secs(10)) {
        Ok(_) => {
            change.succeed();
            true
        }
        Err(message) => {
            change.fail(message);
            false
        }
    };
    let assembled = assembly
        .as_ref()
        .map(|r| {
            r.items
                .iter()
                .filter(|i| {
                    matches!(
                        i.status.as_str(),
                        profile_assets::STATUS_CREATED
                            | profile_assets::STATUS_UPDATED
                            | profile_assets::STATUS_DRIFT
                    )
                })
                .count()
        })
        .unwrap_or(0);
    // 缺失插件要在切换当下就摆出来（OD-ARCH-114）：否则用户只看得到「已装配 N 项」，
    // 却不知道这个 Profile 还缺插件、要不要联网装。这里只列出，不自动安装。
    let plugins_missing: Vec<serde_json::Value> = assembly
        .as_ref()
        .map(|r| {
            r.items
                .iter()
                .filter(|i| i.kind == "plugin" && i.status != profile_assets::STATUS_READY)
                .map(|i| {
                    serde_json::json!({
                        "package": i.key.trim_start_matches("plugin:"),
                        "status": i.status,
                        "detail": i.detail,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    // 缺插件就放到后台自动补齐（OD-ARCH-121）：切换本身已经生效，网络安装不占这条命令。
    // 只在「非基础标准 + 开关打开 + 确有缺项」时启动，其余情况行为与 114 完全一致。
    let auto_install = should_auto_install_plugins(
        plugins_missing.len(),
        chosen.as_deref(),
        &standard,
        crate::settings::profile_auto_install_plugins(&config_dir),
    );
    if auto_install {
        if let Some(target) = chosen.clone() {
            spawn_profile_plugins_autofill(
                app.clone(),
                app_root.clone(),
                dsh_home.clone(),
                target,
                standard.clone(),
            );
        }
    }
    Ok(serde_json::json!({
        "ok": true,
        "saved": chosen,
        "effective": effective.clone(),
        "effective_next_start": effective,
        "engine_stopped": engine_stopped,
        "assembly": assembly,
        "assembled_changes": assembled,
        "plugins_missing": plugins_missing,
        "plugins_auto_installing": auto_install,
        "categories": categories,
        "note": if engine_stopped {
            "已切换，资产已按清单补齐；旧引擎已停止，下一条任务按新 Profile 启动。"
        } else {
            "Profile 已保存，但旧引擎未能确认停止；在旧引擎停止前宿主会拒绝新的引擎调用。"
        },
    }))
}

/// 切换后要不要自动补装（OD-ARCH-121）：缺件非空、开关打开、且目标不是基础标准。
///
/// 抽成纯函数，便于直接覆盖四种组合，不必起窗口或联网。
pub fn should_auto_install_plugins(
    missing: usize,
    chosen: Option<&str>,
    standard: &str,
    enabled: bool,
) -> bool {
    missing > 0 && enabled && chosen.map(|n| n != standard).unwrap_or(false)
}

/// 切换之后在后台补齐缺失插件（OD-ARCH-121）。
///
/// 三个约束：
///   * **不阻塞切换**：走阻塞线程池，切换命令已经返回；
///   * **不加锁等界面**：只拿自己的路径与名字，运行中不再访问宿主状态；
///   * **结果必须回传**：装完重新探测，通过 `omni://profile-plugins` 告诉前端
///     是否已齐、还缺哪些、每包退出码，前端据此刷新缺件卡。
fn spawn_profile_plugins_autofill(
    app: tauri::AppHandle,
    app_root: std::path::PathBuf,
    dsh_home: std::path::PathBuf,
    profile: String,
    standard: String,
) {
    let repo_root = crate::task_shell::repo_root_hint()
        .ok()
        .filter(|p| p.is_dir());
    tauri::async_runtime::spawn_blocking(move || {
        let scope = profile_assets::Scope {
            dsh_home: &dsh_home,
            app_root: &app_root,
            profile: &profile,
            repo_root: repo_root.as_deref(),
        };
        let cli = profile_assets::default_dsh_cli();
        let payload = match profile_assets::ensure_plugins(&scope, &standard, &cli) {
            Ok(receipt) => serde_json::json!({
                "ok": receipt.ok,
                "profile": receipt.profile,
                "attempted": receipt.attempted,
                "results": receipt.results,
                "still_missing": receipt.still_missing,
                "note": receipt.note,
                "automatic": true,
                "error": serde_json::Value::Null,
            }),
            Err(e) => serde_json::json!({
                "ok": false,
                "profile": profile,
                "attempted": Vec::<String>::new(),
                "results": Vec::<serde_json::Value>::new(),
                "still_missing": Vec::<serde_json::Value>::new(),
                "note": "自动补齐插件未能执行。",
                "automatic": true,
                "error": e,
            }),
        };
        let _ = app.emit(PROFILE_PLUGINS_EVENT, payload);
    });
}

/// 冷启动时补齐「已经选好、但本地缺件」的 Profile 插件（OD-ARCH-124）。
///
/// 121 把自动补装接在 `profile_choice_set` 上，只有**切换**那一刻会跑。用户选好
/// 非标准 Profile 之后重启应用，缺的插件不会自己回来——界面显示「正在使用专业
/// Profile」，能力却是残的，只能靠用户进设置页手点。这里把同一套判据接到启动路径：
///   * 只对非基础标准生效（底座永不装插件）；
///   * 遵守设置页的自动补装开关；
///   * 确有缺件才起后台任务，全部就绪时零动作、不写盘、不联网；
///   * 不阻塞启动：窗口先正常打开，装完走 `omni://profile-plugins` 回报。
fn maybe_autofill_plugins_on_startup(app: &tauri::AppHandle) {
    let Some(host) = app.try_state::<SharedHost>() else {
        return;
    };
    // 只取路径与配置，锁在函数内释放：安装任务运行期间不再碰宿主状态。
    let (app_root, config_dir, config) = {
        let guard = host.lock().unwrap();
        let cfg = guard.bridge.supervisor().lock().unwrap().config().clone();
        (guard.paths.root.clone(), guard.paths.config.clone(), cfg)
    };
    if config.channel != "web-api" {
        return;
    }
    let standard = crate::profile_choice::standard_profile(&config.channel).to_string();
    let profile = config.profile.clone();
    if profile.trim() == standard {
        return;
    }
    if !crate::settings::profile_auto_install_plugins(&config_dir) {
        return;
    }
    let repo_root = crate::task_shell::repo_root_hint()
        .ok()
        .filter(|p| p.is_dir());
    let scope = profile_assets::Scope {
        dsh_home: &config.dsh_home,
        app_root: &app_root,
        profile: &profile,
        repo_root: repo_root.as_deref(),
    };
    let missing = profile_assets::missing_plugins(&scope, &standard, &config.dsh_cli);
    if !should_auto_install_plugins(
        missing.len(),
        Some(profile.as_str()),
        &standard,
        true,
    ) {
        return;
    }
    append_profile_assets_log(
        &app_root,
        &format!(
            "profile={} startup-autofill pending={}\n",
            profile,
            missing.len()
        ),
    );
    spawn_profile_plugins_autofill(app.clone(), app_root, config.dsh_home.clone(), profile, standard);
}

/// 启动期装配日志的统一落盘入口（`logs/profile-assets.log`）。
///
/// 写不进去不影响功能：日志只用于「为什么没装上」的排查，不能反过来拦住启动。
fn append_profile_assets_log(app_root: &std::path::Path, line: &str) {
    let dir = app_root.join("logs");
    let path = dir.join("profile-assets.log");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = file.write_all(format!("{} {}", chrono_like_stamp(), line).as_bytes());
    }
}

/// 读取 Profile 装配偏好（OD-ARCH-121）：目前只有「切换时自动补齐缺失插件」。
#[tauri::command(async)]
fn profile_assembly_prefs(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    Ok(serde_json::json!({
        "ok": true,
        "auto_install_plugins": settings::profile_auto_install_plugins(&config_dir),
        "settings_path": settings::settings_path(&config_dir).display().to_string(),
        "note": "开启后，切换 Profile 缺插件会在后台自动联网补齐；关闭后只提示，由你点按钮安装。",
    }))
}

/// 保存上述开关；其余设置原样保留。
#[tauri::command(async)]
fn profile_assembly_prefs_set(
    state: State<'_, SharedHost>,
    auto_install_plugins: bool,
) -> Result<serde_json::Value, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let saved = settings::set_profile_auto_install_plugins(&config_dir, auto_install_plugins)?;
    Ok(serde_json::json!({
        "ok": true,
        "auto_install_plugins": saved,
    }))
}

/// 只读：当前（或指定）Profile 的资产就绪情况（OD-ARCH-109）。
///
/// 传 `profile` 时预览该 Profile；不传时看当前运行中的实例。只判定、只读盘，
/// 不写文件——写动作只在切换 Profile 与启动引擎时发生。
#[tauri::command(async)]
fn profile_assets_view(
    state: State<'_, SharedHost>,
    profile: Option<String>,
) -> Result<profile_assets::MaterializeReport, String> {
    let (app_root, running) = {
        let host = state.lock().unwrap();
        let cfg = host.bridge.supervisor().lock().unwrap().config().clone();
        (host.paths.root.clone(), cfg.profile)
    };
    let (channel, dsh_home) = profile_choice_scope()?;
    let standard = profile_choice::standard_profile(&channel).to_string();
    let name = profile
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or(running);
    let repo_root = crate::task_shell::repo_root_hint()
        .ok()
        .filter(|p| p.is_dir());
    let scope = profile_assets::Scope {
        dsh_home: &dsh_home,
        app_root: &app_root,
        profile: &name,
        repo_root: repo_root.as_deref(),
    };
    profile_assets::inspect(&scope, &standard)
}

/// 就地修复资产（OD-ARCH-113）。
///
/// 与 \`profile_assets_view\` 的只读巡检互补：这个命令**会写盘**，但只在用户
/// 明确点「修复缺失 / 恢复出厂内容」时调用。三种情况：
///   * 缺失 / 旧版 → 按出厂内容补齐；
///   * 用户改过（drift）→ 先备份成 \`.bak-<epoch>\`，再按出厂内容覆盖；
///   * 基础标准 → 仍然零写入，返回 \`standard: true\`。
///
/// 不联网、不装插件：插件安装仍走 \`profile_plugin_install\`。
#[tauri::command(async)]
fn profile_assets_repair(
    state: State<'_, SharedHost>,
    profile: Option<String>,
) -> Result<serde_json::Value, String> {
    let (app_root, running) = {
        let host = state.lock().unwrap();
        let cfg = host.bridge.supervisor().lock().unwrap().config().clone();
        (host.paths.root.clone(), cfg.profile)
    };
    let (channel, dsh_home) = profile_choice_scope()?;
    let standard = profile_choice::standard_profile(&channel).to_string();
    let name = profile
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or(running);
    if !crate::settings::is_valid_profile_name(&name) {
        return Err(format!("Profile 名不合法：{:?}", name));
    }
    let repo_root = crate::task_shell::repo_root_hint()
        .ok()
        .filter(|p| p.is_dir());
    let scope = profile_assets::Scope {
        dsh_home: &dsh_home,
        app_root: &app_root,
        profile: &name,
        repo_root: repo_root.as_deref(),
    };
    let report = profile_assets::materialize_with(&scope, &standard, true)?;
    // 修复路径同样要维护 Skill 归属：新增的 Skill 也要能被下次切走时收回。
    if name != standard {
        profile_assets::record_managed_skills(&scope, &standard)?;
    }
    let changed = report
        .items
        .iter()
        .filter(|i| {
            matches!(
                i.status.as_str(),
                profile_assets::STATUS_CREATED | profile_assets::STATUS_UPDATED
            )
        })
        .count();
    let remaining = report
        .items
        .iter()
        .filter(|i| {
            matches!(
                i.status.as_str(),
                profile_assets::STATUS_MISSING
                    | profile_assets::STATUS_MISSING_SOURCE
                    | profile_assets::STATUS_DRIFT
            )
        })
        .count();
    Ok(serde_json::json!({
        "ok": true,
        "profile": report.profile,
        "standard": report.standard,
        "changed": changed,
        "remaining": remaining,
        "report": report,
        "note": if report.standard {
            "基础标准是干净底座，没有写入任何文件。"
        } else if changed == 0 && remaining == 0 {
            "资产已是出厂内容，没有需要修复的项。"
        } else if remaining == 0 {
            "已按出厂内容补齐并备份被覆盖的文件。"
        } else {
            "已尽可能修复；仍有条目未就位（见清单，插件需单独点「安装」）。"
        },
    }))
}

/// 打开（必要时创建）当前 Profile 自带的 Skill 源目录（OD-ARCH-131）。
///
/// 用户把自己的 Skill 放进 `<应用数据根>/profiles/<名字>/skills/<skill名>/`，
/// 切换时就会随 Profile 装配；这个命令只把那个目录摊开给人看。
/// 基础标准拒绝：干净底座不该有业务 Skill 目录。
#[tauri::command(async)]
fn profile_skill_source_open(state: State<'_, SharedHost>) -> Result<serde_json::Value, String> {
    let scope = profile_asset_scope(&state);
    if scope.standard {
        return Err(
            "基础标准是干净底座，不带 Skill 目录。请先在顶栏切到一个自定义 Profile。"
                .to_string(),
        );
    }
    // 校验真正要写入的那个根（`<应用数据根>/profiles/`），而不是引擎的 DSH_HOME：
    // 这条命令只动 Profile 目录，按目标根验才算数。
    crate::capabilities::assert_writable_root(&scope.profiles_root)?;
    let dir = profile_rules::skill_source_root(&scope.profiles_root.join(&scope.profile));
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("创建 Skill 目录失败（{}）：{}", dir.display(), e))?;
    // 走平台层：macOS 用 open、Windows 用 cmd /C start、Linux 用 xdg-open（OD-ARCH-134）。
    match crate::platform::open_target(&dir) {
        Ok(()) => Ok(serde_json::json!({
            "ok": true,
            "opened": dir.display().to_string(),
            "note": "把 Skill 放进这个目录（每个 Skill 一个子目录，至少含 SKILL.md），保存绑定后即可用。",
        })),
        Err(e) => Ok(serde_json::json!({
            "ok": false,
            "path": dir.display().to_string(),
            "note": format!("没能自动打开文件管理器（{}）。目录已经建好，可以手动打开：{}", e, dir.display()),
        })),
    }
}

/// 按名字生成一份 Profile 自带 Skill 的起步模板（OD-ARCH-131）。
///
/// 已存在就不覆盖——用户的正文永远优先。只写 `SKILL.md`，不带任何行业规则。
#[tauri::command(async)]
fn profile_skill_scaffold(
    state: State<'_, SharedHost>,
    name: String,
) -> Result<serde_json::Value, String> {
    let scope = profile_asset_scope(&state);
    if scope.standard {
        return Err(
            "基础标准是干净底座，不生成 Skill。请先在顶栏切到一个自定义 Profile。"
                .to_string(),
        );
    }
    let name = name.trim();
    if !profile_rules::is_valid_skill_name(name) {
        return Err(format!(
            "Skill 名不合法：{:?}（只允许小写字母、数字、连字符、下划线，最多 48 字符）",
            name
        ));
    }
    crate::capabilities::assert_writable_root(&scope.profiles_root)?;
    let app_dir = scope.profiles_root.join(&scope.profile);
    let dir = profile_rules::skill_source_root(&app_dir).join(name);
    let path = dir.join("SKILL.md");
    if path.is_file() {
        return Ok(serde_json::json!({
            "ok": true,
            "created": false,
            "path": path.display().to_string(),
            "note": "这个 Skill 已经有正文，未覆盖。",
        }));
    }
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("创建 Skill 目录失败（{}）：{}", dir.display(), e))?;
    let body = format!(
        "---\nname: {name}\ndescription: 说明这个 Skill 什么时候用、交付什么。\nuser-invocable: true\n---\n\n# {name}\n\n写下这个 Skill 的判断口径与输出要求。\n\n只写方法与边界，不要写具体业务数据。\n"
    );
    crate::model_settings::write_atomic(&path, body.as_bytes())?;
    Ok(serde_json::json!({
        "ok": true,
        "created": true,
        "path": path.display().to_string(),
        "note": "已生成起步模板；改好正文后回到绑定区勾选它并保存。",
    }))
}

/// 只读：停放在 `profiles/.parked-skills/` 里的 Skill（OD-ARCH-117）。
///
/// 切换 Profile 时，不属于目标 Profile 的 Skill 会被**移入**停放区而不是删除，
/// 切回需要的 Profile 时自动归位。这个命令只列清单，不建目录、不动文件，
/// 让用户在设置页能看见「哪些 Skill 现在停在边上」。
#[tauri::command(async)]
fn profile_assets_parked(
    state: State<'_, SharedHost>,
) -> Result<Vec<profile_assets::ParkedSkill>, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    Ok(profile_assets::list_parked(&app_root))
}

/// 新建 / 复制一个自定义 Profile（OD-ARCH-109 补充）。
///
/// `copy_from = None` 从基础标准起手；`Some(name)` 复制那个 Profile 的配置与
/// 场景规则。只写隔离根，不接管 DSH 的内部格式，也不装插件。
#[tauri::command(async)]
fn profile_create(
    state: State<'_, SharedHost>,
    app: AppHandle,
    name: String,
    label: String,
    copy_from: Option<String>,
) -> Result<profile_lifecycle::CreateReceipt, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let (channel, dsh_home) = profile_choice_scope()?;
    let source = copy_from
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let source = match source {
        Some(from) => {
            if !profile_choice::is_selectable(&channel, &app_root, from) {
                return Err(format!("不能从「{}」复制：它不在可选的 Profile 清单里。", from));
            }
            Some(from.to_string())
        }
        None => None,
    };
    let receipt = profile_lifecycle::create(
        &dsh_home,
        &app_root,
        &name,
        &label,
        source.as_deref(),
    )?;
    let _ = app.emit(PROFILE_REGISTRY_EVENT, ());
    Ok(receipt)
}

/// 移除一个自定义 Profile：移入 `.trash`，可恢复；出厂/当前 Profile 拒绝。
#[tauri::command(async)]
fn profile_remove(
    state: State<'_, SharedHost>,
    app: AppHandle,
    name: String,
) -> Result<profile_lifecycle::DeleteReceipt, String> {
    let (app_root, current) = {
        let host = state.lock().unwrap();
        (
            host.paths.root.clone(),
            host.bridge.supervisor().lock().unwrap().config().profile.clone(),
        )
    };
    let (_, dsh_home) = profile_choice_scope()?;
    let receipt = profile_lifecycle::remove(&dsh_home, &app_root, &name, Some(&current))?;
    let _ = app.emit(PROFILE_REGISTRY_EVENT, ());
    Ok(receipt)
}

/// 只读：回收站里可恢复的自定义 Profile（OD-ARCH-125）。
///
/// 与「已停放 Skill」同口径：只列清单，不建目录、不改文件、不联网。
#[tauri::command(async)]
fn profile_trash_list(
    state: State<'_, SharedHost>,
) -> Result<Vec<profile_lifecycle::TrashEntry>, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let (_, dsh_home) = profile_choice_scope()?;
    Ok(profile_lifecycle::list_trash(&app_root, &dsh_home))
}

/// 改自定义 Profile 的显示名（OD-ARCH-126）。
///
/// 只写登记表的 label 与场景声明的 display_name；目录名与 --profile 标识不变，
/// 因此 Skill 绑定、插件清单、线程分类与会话目录都不受影响。
#[tauri::command(async)]
fn profile_rename(
    state: State<'_, SharedHost>,
    app: AppHandle,
    name: String,
    label: String,
) -> Result<profile_lifecycle::RenameReceipt, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let receipt = profile_lifecycle::rename_label(&app_root, &name, &label)?;
    let _ = app.emit(PROFILE_REGISTRY_EVENT, ());
    Ok(receipt)
}

/// 把回收站里的一项恢复成可选 Profile。
///
/// 纯本地：两次 `rename` 加一次登记写回。同名已存在时拒绝且不动任何文件；
/// 中途失败会把已经搬走的目录搬回回收站。不联网、不装插件。
#[tauri::command(async)]
fn profile_trash_restore(
    state: State<'_, SharedHost>,
    app: AppHandle,
    id: String,
) -> Result<profile_lifecycle::RestoreReceipt, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let (_, dsh_home) = profile_choice_scope()?;
    let receipt = profile_lifecycle::restore(&dsh_home, &app_root, &id)?;
    let _ = app.emit(PROFILE_REGISTRY_EVENT, ());
    Ok(receipt)
}

/// 把一个自定义 Profile 导出成单文件迁移包（OD-ARCH-127）。
///
/// 纯读 + 写一个 JSON 文件：不联网、不装插件、不动登记表，因此不广播事件。
#[tauri::command(async)]
fn profile_export(
    state: State<'_, SharedHost>,
    name: String,
    dest_dir: Option<String>,
) -> Result<profile_lifecycle::ExportReceipt, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let (_, dsh_home) = profile_choice_scope()?;
    let dest = dest_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from);
    profile_lifecycle::export_bundle(&dsh_home, &app_root, &name, dest.as_deref())
}

/// 从迁移包导入一个自定义 Profile（OD-ARCH-127）。
///
/// 先校验后落盘、同名一律拒绝；插件本体不进包，导入后由既有的
/// 「缺件提示 + 自动补装」通道按清单安装。
#[tauri::command(async)]
fn profile_import(
    state: State<'_, SharedHost>,
    app: AppHandle,
    path: String,
    rename_to: Option<String>,
    label: Option<String>,
) -> Result<profile_lifecycle::ImportReceipt, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let (_, dsh_home) = profile_choice_scope()?;
    let trim = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let bundle = std::path::PathBuf::from(path.trim());
    let receipt = profile_lifecycle::import_bundle(
        &bundle,
        &dsh_home,
        &app_root,
        trim(&rename_to).as_deref(),
        trim(&label).as_deref(),
    )?;
    let _ = app.emit(PROFILE_REGISTRY_EVENT, ());
    Ok(receipt)
}

/// 只读：导出目录里已有的迁移包（供设置页一键导入）。
#[tauri::command(async)]
fn profile_export_list(
    state: State<'_, SharedHost>,
) -> Result<Vec<profile_lifecycle::BundleListing>, String> {
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    Ok(profile_lifecycle::list_exports(&app_root))
}

/// 在系统文件管理器里打开导出目录（目录不存在时先建，再打开）。
#[tauri::command(async)]
fn profile_export_open(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    let dir = {
        let host = state.lock().unwrap();
        host.paths.root.join("exports")
    };
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("创建导出目录失败（{}）：{}", dir.display(), e))?;
    // 走平台层：macOS 用 open、Windows 用 cmd /C start、Linux 用 xdg-open（OD-ARCH-134）。
    crate::platform::open_target(&dir)
        .map_err(|e| format!("没能打开导出目录（{}）：{}", dir.display(), e))?;
    Ok(serde_json::json!({
        "ok": true,
        "opened": dir.display().to_string(),
    }))
}

/// 安装某个 Profile 需要的插件（用户点「安装」才调用，会联网）。
#[tauri::command(async)]
fn profile_plugin_install(
    state: State<'_, SharedHost>,
    profile: Option<String>,
    package: String,
) -> Result<profile_assets::InstallReceipt, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let (_, dsh_home) = profile_choice_scope()?;
    let name = profile
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            crate::settings::load(&config_dir)
                .ok()
                .and_then(|s| s.engine_profile)
        })
        .ok_or_else(|| "没有指定要安装到哪个 Profile".to_string())?;
    if !crate::settings::is_valid_profile_name(&name) {
        return Err(format!("Profile 名不合法：{:?}", name));
    }
    let cli = profile_assets::default_dsh_cli();
    let version = profile_assets::asset_profile(&name)
        .and_then(|a| a.plugins.iter().find(|p| p.package == package.trim()))
        .and_then(|p| p.version);
    Ok(profile_assets::install_plugin(
        &cli,
        &dsh_home,
        &name,
        &package,
        version,
    ))
}

/// 给自建 Profile 声明一个插件（OD-ARCH-128）。
///
/// 这是「从零配出整套工作台」的最后一个缺口：此前加插件只能手改 DSH 侧配置，
/// 而登记表 `index.v1.json` 才是装配与补装共用的真源。这里先探测、缺才联网装，
/// 装成就写回登记表，之后切换 / 冷启动的后台补装对它与内置项一视同仁。
#[tauri::command(async)]
fn profile_plugins_add(
    state: State<'_, SharedHost>,
    profile: Option<String>,
    package: String,
) -> Result<profile_assets::AddPluginReceipt, String> {
    let (app_root, config_dir) = {
        let host = state.lock().unwrap();
        (host.paths.root.clone(), host.paths.config.clone())
    };
    let (channel, dsh_home) = profile_choice_scope()?;
    let standard = profile_choice::standard_profile(&channel).to_string();
    let name = profile
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            crate::settings::load(&config_dir)
                .ok()
                .and_then(|s| s.engine_profile)
        })
        .unwrap_or_else(|| standard.clone());
    let repo_root = crate::task_shell::repo_root_hint()
        .ok()
        .filter(|p| p.is_dir());
    let scope = profile_assets::Scope {
        dsh_home: &dsh_home,
        app_root: &app_root,
        profile: &name,
        repo_root: repo_root.as_deref(),
    };
    let cli = profile_assets::default_dsh_cli();
    profile_assets::add_plugin(&scope, &standard, &cli, &package)
}

/// 一次性补齐某个 Profile 缺的插件（OD-ARCH-114）。
///
/// 只装清单里缺的包，单个失败不中断其余；联网安装仍只由用户点按钮触发。
#[tauri::command(async)]
fn profile_plugins_ensure(
    state: State<'_, SharedHost>,
    profile: Option<String>,
) -> Result<profile_assets::EnsureReceipt, String> {
    let (app_root, config_dir) = {
        let host = state.lock().unwrap();
        (host.paths.root.clone(), host.paths.config.clone())
    };
    let (channel, dsh_home) = profile_choice_scope()?;
    let standard = profile_choice::standard_profile(&channel).to_string();
    let name = profile
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            crate::settings::load(&config_dir)
                .ok()
                .and_then(|s| s.engine_profile)
        })
        .unwrap_or_else(|| standard.clone());
    let repo_root = crate::task_shell::repo_root_hint()
        .ok()
        .filter(|p| p.is_dir());
    let scope = profile_assets::Scope {
        dsh_home: &dsh_home,
        app_root: &app_root,
        profile: &name,
        repo_root: repo_root.as_deref(),
    };
    let cli = profile_assets::default_dsh_cli();
    profile_assets::ensure_plugins(&scope, &standard, &cli)
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
    profile_asset_dir(state)
}

/// 读取规则资产（只读；不创建文件）。
#[tauri::command(async)]
fn profile_rules_load(state: State<'_, SharedHost>) -> profile_rules::RulesView {
    profile_rules_view(&state)
}

/// 规则资产的唯一读取实现（OD-ARCH-107）。
///
/// 设置页与能力管理页都走这里，避免两处各自解析 rules.json 得出不同结论。
/// 迁移在这里跑，幂等；只读，不创建文件。
fn profile_rules_view(state: &State<'_, SharedHost>) -> profile_rules::RulesView {
    // 规则也可能先于声明被读到（用户直接进设置页）：迁移同样要先跑一次，且幂等。
    let (_, migration_error) = migrate_legacy_profile_assets(&state);
    let mut view = profile_rules::load_rules(&profile_rules_dir(&state));
    // 旧规则搬不过来不塞进 `error`：那个字段会让引擎拒绝提交（等于把损坏的旧文件
    // 变成新的功能故障）。改用单独的提示字段，界面按「未加载（纯净底座）」显示，
    // 同时把原因说清楚。
    if !view.loaded {
        view.migration_warning = migration_error;
    }
    // OD-ARCH-106：宿主知道当前是哪个 Profile，据此告诉界面「恢复默认」能不能用。
    // 只标原因、不禁读：基础标准下若用户自己粘贴保存了规则，照样按文件生效。
    if profile_asset_scope(&state).standard {
        view.default_blocked_reason = Some(
            "基础标准是干净底座，不安装行业规则包。切到专业 Profile 才能安装。".to_string(),
        );
    }
    if view.loaded {
        let dsh_home = {
            let host = state.lock().unwrap();
            host.bridge.supervisor().lock().unwrap().config().dsh_home.clone()
        };
        let repo_skills = crate::task_shell::repo_root_hint()
            .ok()
            .filter(|p| p.is_dir())
            .map(|r| r.join("skills"));
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
                        std::fs::read_to_string(&repo_path).ok().or_else(|| {
                            // 分发环境下仓库源不存在，回落到编译期内嵌正文。
                            crate::profile_seed::skill(skill_id)
                                .and_then(|s| s.files.iter().find(|f| f.rel == "SKILL.md"))
                                .map(|f| f.body.to_string())
                        })
                    } else {
                        crate::profile_seed::skill(skill_id)
                            .and_then(|s| s.files.iter().find(|f| f.rel == "SKILL.md"))
                            .map(|f| f.body.to_string())
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
    save_rules_into(&profile_rules_dir(&state), profile_asset_scope(&state).standard, &text)
}

/// 保存「场景 → Skill」绑定（OD-ARCH-129）。
///
/// 只替换规则包里的 `skill_bindings`，其余内容原样保留；仍走同一套校验与备份。
/// 基础标准与规则保存同口径：拒绝且不建目录——干净底座不该有业务 Skill 预注入。
#[tauri::command(async)]
fn profile_skill_bindings_save(
    state: State<'_, SharedHost>,
    bindings: Vec<profile_rules::BindingChoice>,
) -> Result<serde_json::Value, String> {
    let dir = profile_rules_dir(&state);
    let asset_scope = profile_asset_scope(&state);
    let mut receipt = save_skill_bindings_into(&dir, asset_scope.standard, &bindings)?;
    // OD-ARCH-130：绑定写盘成功就立刻收敛运行时 skills/。
    // 规则已经写进文件就不回滚（那是用户的决定），补齐失败只在回执里如实说明。
    // 路径取运行中引擎的配置：与切换 / 启动时的装配用同一个 dsh_home，
    // 否则「补到了另一个根」会让界面显示补齐、运行时仍看不到。
    let (app_root, dsh_home, channel) = {
        let host = state.lock().unwrap();
        let cfg = host.bridge.supervisor().lock().unwrap().config().clone();
        (host.paths.root.clone(), cfg.dsh_home, cfg.channel)
    };
    let standard = profile_choice::standard_profile(&channel).to_string();
    let repo_root = crate::task_shell::repo_root_hint()
        .ok()
        .filter(|p| p.is_dir());
    let scope = profile_assets::Scope {
        dsh_home: &dsh_home,
        app_root: &app_root,
        profile: &asset_scope.profile,
        repo_root: repo_root.as_deref(),
    };
    let skills = match profile_assets::sync_bound_skills(&scope, &standard) {
        Ok(sync) => serde_json::to_value(sync).unwrap_or(serde_json::Value::Null),
        Err(e) => serde_json::json!({
            "ok": false,
            "note": format!("绑定已保存，但没能立刻写入运行时 Skill 目录：{}", e),
        }),
    };
    if let Some(map) = receipt.as_object_mut() {
        map.insert("skills".to_string(), skills);
    }
    Ok(receipt)
}

/// 绑定保存的真实边界，抽成纯函数便于直接测「基础标准零写入」与「只改绑定」。
///
/// 规则文件不存在时用起步骨架打底：这样新 Profile 不用先点「恢复默认规则包」
/// 就能配出绑定，也不会顺带装上任何行业规则。
fn save_skill_bindings_into(
    profile_dir: &std::path::Path,
    standard: bool,
    bindings: &[profile_rules::BindingChoice],
) -> Result<serde_json::Value, String> {
    if standard {
        return Err(
            "基础标准是干净底座，不保存 Skill 绑定。请先在顶栏切到一个自定义 Profile 再配。"
                .to_string(),
        );
    }
    let path = profile_rules::rules_path(profile_dir);
    let base = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let name = profile_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            crate::profile_starter::rules_text(&name)?
        }
        Err(e) => return Err(format!("读取规则文件失败（{}）：{}", path.display(), e)),
    };
    let text = profile_rules::set_skill_bindings(&base, bindings)?;
    let mut receipt = write_profile_rules(profile_dir, &text)?;
    if let Some(map) = receipt.as_object_mut() {
        map.insert(
            "note".to_string(),
            serde_json::Value::String(
                "Skill 绑定已保存；其余规则内容未改动，旧文件已备份。".to_string(),
            ),
        );
    }
    Ok(receipt)
}

/// 规则保存的真实边界，抽成纯函数便于直接测「基础标准连目录都不建」。
///
/// OD-ARCH-128：原先只有「恢复默认」拦基础标准，手动粘贴保存这条能直接
/// `create_dir_all` 写盘，等于给干净底座开了后门；规则又是准入策略的输入，
/// 写进去就不再是「干净」了。
fn save_rules_into(
    profiles_dir: &std::path::Path,
    standard: bool,
    text: &str,
) -> Result<serde_json::Value, String> {
    if standard {
        return Err(
            "基础标准是干净底座，不保存行业规则包。请先在顶栏切到一个自定义 Profile 再保存。"
                .to_string(),
        );
    }
    if text.len() > 128 * 1024 {
        return Err("规则文件过大（上限 128 KB）".to_string());
    }
    // 校验失败一律不写：避免把「以为保存成功」的坏规则留在磁盘上。
    profile_rules::parse_rules(text)?;
    write_profile_rules(profiles_dir, text)
}

/// 恢复出厂默认规则包（写入编译期嵌入的样例）。
#[tauri::command(async)]
fn profile_rules_restore_default(
    state: State<'_, SharedHost>,
) -> Result<serde_json::Value, String> {
    // OD-ARCH-106：出厂规则包属于专业 Profile。基础标准下点「恢复默认」不能把
    // 纪律规则写进干净底座，否则切换 Profile 就不再是「有没有业务规则」的分水岭。
    let scope = profile_asset_scope(&state);
    if scope.standard {
        return Err(
            "基础标准是干净底座，不安装行业规则包。请先在顶栏切到专业 Profile 再安装。"
                .to_string(),
        );
    }
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
    // 模型设置不经过引擎启动；首次打开时先补齐包内 seed（OD-ARCH-137）。
    engine_config.seed_profile_if_missing()?;
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

// ---------------------------------------------------------------------------
// 项目实体与项目概览（OD-KM-508）
//
// 薄封装 project_store.rs：项目文件仍是 work/项目/<project_id>.md。项目与
// 任务/记录/素材/知识的关联不存项目文件，全部在读取时反向派生；素材来自
// material_store.rs（只读），知识计数来自任务与记录 frontmatter 的
// related_knowledge 字段。名称/目标属于用户文本，落盘前过统一闸门。
// ---------------------------------------------------------------------------

/// 知识条目在项目概览中的一条引用来源。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectKnowledgeReference {
    /// task / record。
    pub source_kind: String,
    pub source_id: String,
    pub source_title: String,
    pub reason: String,
    pub linked_at: String,
}

/// 按知识条目去重后的项目知识引用。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectKnowledgeRef {
    pub knowledge_id: String,
    pub references: Vec<ProjectKnowledgeReference>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectOverview {
    pub project: project_store::Project,
    pub tasks: Vec<project_store::ProjectRefItem>,
    pub records: Vec<project_store::ProjectRefItem>,
    pub materials: Vec<project_store::ProjectRefItem>,
    pub knowledge: Vec<ProjectKnowledgeRef>,
    pub task_count: usize,
    pub record_count: usize,
    pub material_count: usize,
    pub knowledge_count: usize,
    /// 读取失败、未进入列表的文件数（如实上报，不静默丢弃）。
    pub broken_tasks: usize,
    pub broken_records: usize,
    pub broken_materials: usize,
}

fn clean_project_ref(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// 任务保存路径的项目存在性校验。空编号视为未关联；错误信息带项目提示。
fn ensure_project_exists(root: &std::path::Path, value: &str) -> Result<(), String> {
    let id = value.trim();
    if id.is_empty() {
        return Ok(());
    }
    let path = project_store::project_path(root, id).map_err(|err| {
        format!("关联项目编号不合法：{id}；{err}。请在「项目」中选择已有项目。")
    })?;
    if !path.exists() {
        return Err(format!(
            "关联项目不存在：{id}。请先在「项目」中创建该项目，或在任务中选择已有项目后再保存。"
        ));
    }
    project_store::read_project(root, id)
        .map(|_| ())
        .map_err(|err| format!("关联项目无法读取：{id}；{err}"))
}

fn project_create_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut draft: project_store::ProjectDraft,
    now: f64,
) -> Result<project_store::Project, String> {
    let (name, _) = gate_user_text(&draft.name, policy, "项目名称")?;
    draft.name = name;
    let (goal, _) = gate_user_text(&draft.goal, policy, "项目目标")?;
    draft.goal = goal;
    project_store::create_project(root, &draft, now)
}

fn project_update_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    id: &str,
    mut patch: project_store::ProjectPatch,
    now: f64,
) -> Result<project_store::Project, String> {
    if let Some(name) = patch.name.clone() {
        let (masked, _) = gate_user_text(&name, policy, "项目名称")?;
        patch.name = Some(masked);
    }
    if let Some(goal) = patch.goal.clone() {
        let (masked, _) = gate_user_text(&goal, policy, "项目目标")?;
        patch.goal = Some(masked);
    }
    project_store::update_project(root, id, &patch, now)
}

fn project_overview_impl(
    root: &std::path::Path,
    project_id: &str,
) -> Result<ProjectOverview, String> {
    let id = project_id.trim();
    let project = project_store::read_project(root, id)?;
    let tasks = task_store::list_tasks(root, &task_store::TaskFilter::default())?;
    let records = worklog_records::list_records(root, &worklog_records::RecordFilter::default())?;
    let materials = material_store::list_materials(root, &material_store::MaterialFilter::default())?;

    let task_refs: Vec<project_store::ProjectRefItem> = tasks
        .items
        .iter()
        .map(|task| project_store::ProjectRefItem {
            project_id: clean_project_ref(task.project_id.as_deref()).map(str::to_string),
            id: task.id.clone(),
            title: task.title.clone(),
        })
        .collect();

    // 记录自身没有 project_id：经 linked_task_ids 指向的任务派生所属项目。
    let task_projects: std::collections::BTreeMap<&str, &str> = tasks
        .items
        .iter()
        .filter_map(|task| {
            clean_project_ref(task.project_id.as_deref())
                .map(|project| (task.id.as_str(), project))
        })
        .collect();
    let mut record_refs: Vec<project_store::ProjectRefItem> = Vec::new();
    let mut record_projects: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for record in &records.items {
        let mut projects: Vec<&str> = Vec::new();
        for task_id in &record.linked_task_ids {
            if let Some(project) = task_projects.get(task_id.as_str()) {
                if !projects.contains(project) {
                    projects.push(project);
                }
            }
        }
        for project in &projects {
            record_refs.push(project_store::ProjectRefItem {
                project_id: Some(project.to_string()),
                id: record.id.clone(),
                title: record.title.clone(),
            });
        }
        record_projects.insert(record.id.as_str(), projects);
    }

    let material_refs: Vec<project_store::ProjectRefItem> = materials
        .items
        .iter()
        .map(|material| project_store::ProjectRefItem {
            project_id: clean_project_ref(material.related_project_id.as_deref())
                .map(str::to_string),
            id: material.material_id.clone(),
            title: material.title.clone(),
        })
        .collect();

    let groups = project_store::project_refs(&project_store::ProjectRefsInput {
        tasks: task_refs,
        materials: material_refs,
        records: record_refs,
    });
    let group = groups
        .into_iter()
        .find(|group| group.project_id == id)
        .unwrap_or_else(|| project_store::ProjectRefsGroup {
            project_id: id.to_string(),
            tasks: Vec::new(),
            materials: Vec::new(),
            records: Vec::new(),
        });

    let mut knowledge: std::collections::BTreeMap<String, ProjectKnowledgeRef> =
        std::collections::BTreeMap::new();
    let mut push_link = |link: &task_store::KnowledgeLink,
                         source_kind: &str,
                         source_id: &str,
                         source_title: &str| {
        knowledge
            .entry(link.id.clone())
            .or_insert_with(|| ProjectKnowledgeRef {
                knowledge_id: link.id.clone(),
                references: Vec::new(),
            })
            .references
            .push(ProjectKnowledgeReference {
                source_kind: source_kind.to_string(),
                source_id: source_id.to_string(),
                source_title: source_title.to_string(),
                reason: link.reason.clone(),
                linked_at: link.linked_at.clone(),
            });
    };
    for task in &tasks.items {
        if clean_project_ref(task.project_id.as_deref()) != Some(id) {
            continue;
        }
        for link in &task.related_knowledge {
            push_link(link, "task", &task.id, &task.title);
        }
    }
    for record in &records.items {
        let in_project = record_projects
            .get(record.id.as_str())
            .map(|projects| projects.contains(&id))
            .unwrap_or(false);
        if !in_project {
            continue;
        }
        for link in &record.related_knowledge {
            push_link(link, "record", &record.id, &record.title);
        }
    }
    drop(push_link);

    Ok(ProjectOverview {
        project,
        task_count: group.tasks.len(),
        record_count: group.records.len(),
        material_count: group.materials.len(),
        knowledge_count: knowledge.len(),
        tasks: group.tasks,
        records: group.records,
        materials: group.materials,
        knowledge: knowledge.into_values().collect(),
        broken_tasks: tasks.broken,
        broken_records: records.broken,
        broken_materials: materials.broken,
    })
}

#[tauri::command(async)]
fn project_create(
    state: State<'_, SharedHost>,
    draft: project_store::ProjectDraft,
) -> Result<project_store::Project, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    project_create_impl(&root, &policy, draft, task_shell::now_secs())
}

#[tauri::command(async)]
fn project_update(
    state: State<'_, SharedHost>,
    id: String,
    patch: project_store::ProjectPatch,
) -> Result<project_store::Project, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    project_update_impl(&root, &policy, &id, patch, task_shell::now_secs())
}

#[tauri::command(async)]
fn project_list(
    state: State<'_, SharedHost>,
) -> Result<project_store::ProjectListing, String> {
    let root = work_root(&state)?;
    project_store::list_projects(&root)
}

#[tauri::command(async)]
fn project_overview(
    state: State<'_, SharedHost>,
    project_id: String,
) -> Result<ProjectOverview, String> {
    let root = work_root(&state)?;
    project_overview_impl(&root, &project_id)
}

// ---------------------------------------------------------------------------
// 写作素材 IPC 六命令（OD-KM-506）
//
// 薄封装 material_store.rs（501）。所有用户文本（标题/正文/来源引用/标签/
// 编辑理由/审核理由）落盘前过统一闸门；查询条件不落盘，只做白名单与形状校验。
// AI 只能经既有分析链路暂存素材候选，正式素材与审核状态没有 AI 写通道。
// ---------------------------------------------------------------------------

/// 来源追溯中的单个实体：记录 / 任务 / 项目 / 知识条目。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MaterialTraceEntity {
    pub id: String,
    /// 文件是否读到；false 时 note 说明原因，不静默丢掉。
    pub found: bool,
    pub title: Option<String>,
    /// work/ 内相对路径或知识库内相对路径；读不到时为可推断的位置。
    pub location: Option<String>,
    pub note: Option<String>,
}

/// 素材来源追溯：来源实体 + 素材保存的原始引用。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MaterialSourceTrace {
    pub source_type: String,
    pub source_id: String,
    /// 素材保存的原始引用原文。
    pub quote: String,
    /// 引用是否能按原文在来源实体里定位。
    pub quote_found: bool,
    /// 素材自带的 source_location 优先，否则用来源实体位置。
    pub location: Option<String>,
    pub entity: MaterialTraceEntity,
    /// 引用前后的原文片段（有界），供界面展示来源上下文。
    pub excerpt: Option<String>,
}

/// material_get 回执：素材本身 + 来源与关联实体定位。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MaterialDetail {
    pub material: material_store::WritingMaterial,
    pub source: MaterialSourceTrace,
    pub related_tasks: Vec<MaterialTraceEntity>,
    pub related_project: Option<MaterialTraceEntity>,
    pub related_knowledge: Vec<MaterialTraceEntity>,
}

fn gate_material_text(
    value: &str,
    policy: &profile_rules::RulePolicy,
    target: &str,
) -> Result<String, String> {
    Ok(gate_user_text(value, policy, target)?.0)
}

fn gate_material_tags(
    tags: &[String],
    policy: &profile_rules::RulePolicy,
) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(tags.len());
    for tag in tags {
        out.push(gate_material_text(tag, policy, "素材标签")?);
    }
    Ok(out)
}

fn material_create_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut draft: material_store::MaterialDraft,
    now: f64,
) -> Result<material_store::WritingMaterial, String> {
    draft.title = gate_material_text(&draft.title, policy, "素材标题")?;
    draft.content = gate_material_text(&draft.content, policy, "素材正文")?;
    if draft.source_type.trim().is_empty() {
        // 用户主动收藏不指定来源时按 user 收藏落盘；source_quote 仍必填（501 R1）。
        draft.source_type = material_store::SOURCE_USER.to_string();
    }
    draft.source_quote = gate_material_text(&draft.source_quote, policy, "素材来源引用")?;
    if let Some(location) = draft.source_location.clone() {
        draft.source_location = Some(gate_material_text(&location, policy, "素材来源定位")?);
    }
    draft.tags = gate_material_tags(&draft.tags, policy)?;
    // user 来源的 source_quote 仍必填（501 裁决 R1）；界面可把正文作为引用传入。
    material_store::create_material(root, &draft, now)
}

fn material_edit_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    id: &str,
    mut patch: material_store::MaterialPatch,
    now: f64,
) -> Result<material_store::WritingMaterial, String> {
    if let Some(value) = patch.title.clone() {
        patch.title = Some(gate_material_text(&value, policy, "素材标题")?);
    }
    if let Some(value) = patch.content.clone() {
        patch.content = Some(gate_material_text(&value, policy, "素材正文")?);
    }
    if let Some(value) = patch.source_quote.clone() {
        patch.source_quote = Some(gate_material_text(&value, policy, "素材来源引用")?);
    }
    if let Some(value) = patch.source_location.clone() {
        patch.source_location = Some(gate_material_text(&value, policy, "素材来源定位")?);
    }
    if let Some(tags) = patch.tags.clone() {
        patch.tags = Some(gate_material_tags(&tags, policy)?);
    }
    if let Some(reason) = patch.reason.clone() {
        patch.reason = Some(gate_material_text(&reason, policy, "素材编辑理由")?);
    }
    material_store::update_material(root, id, &patch, now)
}

fn normalize_material_review_action(action: &str) -> Result<String, String> {
    let action = action.trim();
    let mapped = match action {
        "confirm" | material_store::ACTION_CONFIRM => material_store::ACTION_CONFIRM,
        "return" | material_store::ACTION_RETURN => material_store::ACTION_RETURN,
        "reject" | material_store::ACTION_REJECT => material_store::ACTION_REJECT,
        "defer" | material_store::ACTION_DEFER => material_store::ACTION_DEFER,
        "archive" | material_store::ACTION_ARCHIVE => material_store::ACTION_ARCHIVE,
        other => return Err(format!("审核动作不认识：{other}")),
    };
    Ok(mapped.to_string())
}

fn material_review_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    id: &str,
    action: &str,
    reason: Option<String>,
    now: f64,
) -> Result<material_store::WritingMaterial, String> {
    let action = normalize_material_review_action(action)?;
    let reason = match reason {
        Some(value) if !value.trim().is_empty() => {
            Some(gate_material_text(&value, policy, "素材审核理由")?)
        }
        _ => None,
    };
    // 退回/拒绝的理由必填由 501 流转函数统一强制。
    material_store::review_material(root, id, &action, reason.as_deref(), now)
}

fn relative_work_path(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let count = value.chars().count();
    if count <= limit {
        return value.to_string();
    }
    let head: String = value.chars().take(limit).collect();
    format!("{head}…")
}

/// 在原文里围绕引用位置截取有界上下文；引用不在原文里时返回 None。
fn excerpt_around(text: &str, quote: &str, radius: usize) -> Option<String> {
    let needle = quote.trim();
    if needle.is_empty() {
        return None;
    }
    let start = text.find(needle)?;
    let match_start = text[..start].chars().count();
    let match_len = needle.chars().count();
    let chars: Vec<char> = text.chars().collect();
    let begin = match_start.saturating_sub(radius);
    let end = (match_start + match_len + radius).min(chars.len());
    let slice: String = chars[begin..end].iter().collect();
    let mut out = String::new();
    if begin > 0 {
        out.push('…');
    }
    out.push_str(&slice);
    if end < chars.len() {
        out.push('…');
    }
    Some(out)
}

fn trace_work_record(root: &std::path::Path, id: &str) -> MaterialTraceEntity {
    let location = worklog_records::record_path(root, id)
        .ok()
        .map(|path| relative_work_path(root, &path));
    match worklog_records::read_record(root, id) {
        Ok(record) => MaterialTraceEntity {
            id: id.to_string(),
            found: true,
            title: Some(record.title),
            location,
            note: None,
        },
        Err(error) => MaterialTraceEntity {
            id: id.to_string(),
            found: false,
            title: None,
            location,
            note: Some(error),
        },
    }
}

fn trace_task(root: &std::path::Path, id: &str) -> MaterialTraceEntity {
    let location = task_store::task_path(root, id)
        .ok()
        .map(|path| relative_work_path(root, &path));
    match task_store::read_task(root, id) {
        Ok(task) => MaterialTraceEntity {
            id: id.to_string(),
            found: true,
            title: Some(task.title),
            location,
            note: None,
        },
        Err(error) => MaterialTraceEntity {
            id: id.to_string(),
            found: false,
            title: None,
            location,
            note: Some(error),
        },
    }
}

fn trace_project(root: &std::path::Path, id: &str) -> MaterialTraceEntity {
    let location = project_store::project_path(root, id)
        .ok()
        .map(|path| relative_work_path(root, &path));
    match project_store::read_project(root, id) {
        Ok(project) => MaterialTraceEntity {
            id: id.to_string(),
            found: true,
            title: Some(project.name),
            location,
            note: None,
        },
        Err(error) => MaterialTraceEntity {
            id: id.to_string(),
            found: false,
            title: None,
            location,
            note: Some(error),
        },
    }
}

/// 知识条目只读定位；vault 未配置时如实返回未找到，不让整条素材详情失败。
fn trace_knowledge(
    vault: Option<&std::path::Path>,
    id: &str,
) -> (MaterialTraceEntity, Option<String>) {
    let Some(vault) = vault else {
        return (
            MaterialTraceEntity {
                id: id.to_string(),
                found: false,
                title: None,
                location: Some(id.to_string()),
                note: Some("知识库未配置或不可读，无法定位知识条目。".to_string()),
            },
            None,
        );
    };
    match knowledge_vault::read_vault_note(vault, id) {
        Ok(note) => (
            MaterialTraceEntity {
                id: id.to_string(),
                found: true,
                title: Some(note.file_name),
                // 以调用方引用的知识编号为准；不依赖 VaultNote 在符号链接根下
                // 可能退化成的绝对路径。
                location: Some(id.to_string()),
                note: None,
            },
            Some(note.content),
        ),
        Err(error) => (
            MaterialTraceEntity {
                id: id.to_string(),
                found: false,
                title: None,
                location: Some(id.to_string()),
                note: Some(error),
            },
            None,
        ),
    }
}

fn material_source_trace(
    root: &std::path::Path,
    vault: Option<&std::path::Path>,
    material: &material_store::WritingMaterial,
) -> MaterialSourceTrace {
    let id = material.source_id.clone();
    let quote = material.source_quote.clone();
    let mut quote_found = false;
    let mut excerpt: Option<String> = None;
    let entity = match material.source_type.as_str() {
        material_store::SOURCE_WORK_RECORD => {
            match worklog_records::read_record(root, &id) {
                Ok(record) => {
                    quote_found = record.body.contains(&quote);
                    excerpt = excerpt_around(&record.body, &quote, 60);
                }
                Err(_) => {}
            }
            trace_work_record(root, &id)
        }
        material_store::SOURCE_TASK => {
            match task_store::read_task(root, &id) {
                Ok(task) => {
                    quote_found = task
                        .source_quote
                        .as_deref()
                        .map(|value| value.contains(&quote) || quote.contains(value))
                        .unwrap_or(false);
                    excerpt = task.source_quote.as_deref().map(|value| truncate_chars(value, 400));
                }
                Err(_) => {}
            }
            trace_task(root, &id)
        }
        material_store::SOURCE_PROJECT => {
            match project_store::read_project(root, &id) {
                Ok(project) => {
                    quote_found = project.goal.contains(&quote);
                    excerpt = excerpt_around(&project.goal, &quote, 60);
                }
                Err(_) => {}
            }
            trace_project(root, &id)
        }
        material_store::SOURCE_KNOWLEDGE => {
            let (entity, content) = trace_knowledge(vault, &id);
            if let Some(content) = content.as_deref() {
                quote_found = content.contains(&quote);
                excerpt = excerpt_around(content, &quote, 60);
            }
            entity
        }
        material_store::SOURCE_USER => {
            quote_found = true;
            MaterialTraceEntity {
                id: if id.trim().is_empty() {
                    material_store::SOURCE_USER.to_string()
                } else {
                    id.clone()
                },
                found: true,
                title: Some("用户收藏".to_string()),
                location: None,
                note: None,
            }
        }
        _ => MaterialTraceEntity {
            id,
            found: false,
            title: None,
            location: None,
            note: Some("未知来源类型。".to_string()),
        },
    };
    MaterialSourceTrace {
        source_type: material.source_type.clone(),
        source_id: material.source_id.clone(),
        quote,
        quote_found,
        location: material
            .source_location
            .clone()
            .or_else(|| entity.location.clone()),
        entity,
        excerpt,
    }
}

fn material_get_impl(
    root: &std::path::Path,
    vault: Option<&std::path::Path>,
    id: &str,
) -> Result<MaterialDetail, String> {
    let material = material_store::read_material(root, id)?;
    let source = material_source_trace(root, vault, &material);
    let related_tasks = material
        .related_task_ids
        .iter()
        .map(|task_id| trace_task(root, task_id))
        .collect();
    let related_project = material
        .related_project_id
        .as_deref()
        .map(|project_id| trace_project(root, project_id));
    let related_knowledge = material
        .related_knowledge_ids
        .iter()
        .map(|knowledge_id| trace_knowledge(vault, knowledge_id).0)
        .collect();
    Ok(MaterialDetail {
        material,
        source,
        related_tasks,
        related_project,
        related_knowledge,
    })
}

/// AI 素材提取准备：复用既有分析链路（同一门禁、同一提示词），
/// 提示词已注入既有素材清单。模型调用仍由界面随后提交；
/// 本命令不写正式素材，结果只能经 worklog_record_analyze_apply 落暂存候选。
#[tauri::command(async)]
fn material_extract_from_record(
    state: State<'_, SharedHost>,
    record_id: String,
) -> Result<task_extract::AnalyzePrompt, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    worklog_record_analyze_impl(&root, &app_root, &policy, &record_id)
}

#[tauri::command(async)]
fn material_create(
    state: State<'_, SharedHost>,
    draft: material_store::MaterialDraft,
) -> Result<material_store::WritingMaterial, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    material_create_impl(&root, &policy, draft, task_shell::now_secs())
}

#[tauri::command(async)]
fn material_list(
    state: State<'_, SharedHost>,
    filter: material_store::MaterialFilter,
) -> Result<material_store::MaterialListing, String> {
    let root = work_root(&state)?;
    material_store::list_materials(&root, &filter)
}

#[tauri::command(async)]
fn material_get(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<MaterialDetail, String> {
    let root = work_root(&state)?;
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let vault = configured_vault_root(&paths).ok();
    material_get_impl(&root, vault.as_deref(), &id)
}

#[tauri::command(async)]
fn material_review(
    state: State<'_, SharedHost>,
    id: String,
    action: String,
    reason: Option<String>,
) -> Result<material_store::WritingMaterial, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    material_review_impl(&root, &policy, &id, &action, reason, task_shell::now_secs())
}

#[tauri::command(async)]
fn material_edit(
    state: State<'_, SharedHost>,
    id: String,
    patch: material_store::MaterialPatch,
) -> Result<material_store::WritingMaterial, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    material_edit_impl(&root, &policy, &id, patch, task_shell::now_secs())
}

/// 第四阶段接口：纯只读素材查询（OD-KM-509）。
/// 默认只返回已确认素材；关键词只在宿主本地匹配标题与正文，不落盘、不外发。
#[tauri::command(async)]
fn material_query(
    state: State<'_, SharedHost>,
    query: material_query::MaterialQueryInput,
) -> Result<material_query::MaterialQueryResponse, String> {
    let root = work_root(&state)?;
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let vault = configured_vault_root(&paths).ok();
    material_query::query_materials(&root, vault.as_deref(), &query)
}

// ---------------------------------------------------------------------------
// 第四阶段：智能素材收集 IPC（OD-WT-605）
//
// 组合 509 material_query、知识库词法检索与 508 项目概览的只读查询，不落盘。
// `writing_id` 是用户可见编号，过统一闸门；过滤条件不落盘，按 506 R5 口径
// 只做白名单与形状校验，不进入红线扫描。用户选择走 601 写作任务更新，
// 采纳/排除/重点说明写进追加状态记录。
// ---------------------------------------------------------------------------

fn writing_collect_materials_impl(
    root: &std::path::Path,
    vault: Option<&std::path::Path>,
    policy: &profile_rules::RulePolicy,
    request: writing_collect::CollectRequest,
) -> Result<writing_collect::CollectedMaterials, String> {
    let (writing_id, _) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    writing_collect::collect_materials_from(
        root,
        vault,
        &request.filter,
        &writing_id,
        &request.knowledge_hints,
        request.include_pending,
    )
}

fn writing_select_materials_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut request: writing_collect::SelectionRequest,
    now: f64,
) -> Result<writing_collect::SelectionOutcome, String> {
    let (writing_id, _) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    request.writing_id = writing_id;
    if let Some(note) = request.note.clone() {
        if !note.trim().is_empty() {
            let (masked, _) = gate_user_text(&note, policy, "素材选择说明")?;
            request.note = Some(masked);
        }
    }
    writing_collect::apply_selection(root, &request, now)
}

#[tauri::command(async)]
fn writing_collect_materials(
    state: State<'_, SharedHost>,
    request: writing_collect::CollectRequest,
) -> Result<writing_collect::CollectedMaterials, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let vault = configured_vault_root(&paths).ok();
    writing_collect_materials_impl(&root, vault.as_deref(), &policy, request)
}

#[tauri::command(async)]
fn writing_select_materials(
    state: State<'_, SharedHost>,
    request: writing_collect::SelectionRequest,
) -> Result<writing_collect::SelectionOutcome, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_select_materials_impl(&root, &policy, request, task_shell::now_secs())
}

// ---------------------------------------------------------------------------
// 第四阶段：大纲生成与确认 IPC（OD-WT-606）
//
// 宿主组提示词，模型调用仍在界面侧；生成命令不写文件。确认命令先把用户
// 可编辑的大纲文本全部过红线与掩码，再由 602 写 outline.md 与快照，最后经
// 601 状态表迁移到「写作中」（必须带 is_outline_confirmed=true）。
// 类型模板从应用数据根 config/ 读取；文件不存在时使用内嵌默认，零写入。
// ---------------------------------------------------------------------------

fn writing_outline_generate_impl(
    root: &std::path::Path,
    config_dir: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    request: writing_outline::OutlineGenerateRequest,
) -> Result<writing_outline::WritingOutlinePrompt, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    let mut prompt = writing_outline::generate_outline_prompt(
        root,
        config_dir,
        &writing_outline::OutlineGenerateRequest { writing_id },
    )?;
    prompt.masked = counts.any();
    prompt.masked_id_card = counts.id_card;
    prompt.masked_phone = counts.phone;
    Ok(prompt)
}

fn gate_outline_text(
    outline: &mut writing_outline::WritingOutline,
    policy: &profile_rules::RulePolicy,
) -> Result<profile_rules::MaskCounts, String> {
    let mut total = profile_rules::MaskCounts::default();
    let mut apply = |value: &mut String, target: &str| -> Result<(), String> {
        let (masked, extra) = gate_user_text(value, policy, target)?;
        *value = masked;
        add_counts(&mut total, extra);
        Ok(())
    };
    apply(&mut outline.title, "大纲标题")?;
    apply(&mut outline.writing_purpose, "大纲写作目的")?;
    for point in outline.core_points.iter_mut() {
        apply(point, "大纲核心观点")?;
    }
    for section in outline.sections.iter_mut() {
        apply(&mut section.heading, "大纲章节标题")?;
        apply(&mut section.focus, "大纲章节写作重点")?;
        for value in section.sub_headings.iter_mut() {
            apply(value, "大纲小节标题")?;
        }
    }
    Ok(total)
}

fn writing_outline_confirm_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut request: writing_outline::OutlineConfirmRequest,
    now: f64,
) -> Result<writing_outline::WritingOutlineConfirmed, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    request.writing_id = writing_id;
    let mut total = counts;
    if let Some(note) = request.note.clone() {
        if !note.trim().is_empty() {
            let (masked, extra) = gate_user_text(&note, policy, "大纲确认说明")?;
            request.note = Some(masked);
            add_counts(&mut total, extra);
        }
    }
    let outline_counts = gate_outline_text(&mut request.outline, policy)?;
    add_counts(&mut total, outline_counts);
    let mut confirmed = writing_outline::confirm_outline(root, &request, now)?;
    confirmed.masked = total.any();
    confirmed.masked_id_card = total.id_card;
    confirmed.masked_phone = total.phone;
    Ok(confirmed)
}

fn writing_outline_get_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    writing_id: &str,
) -> Result<writing_outline::WritingOutlineConfirmed, String> {
    let (writing_id, counts) = gate_user_text(writing_id, policy, "写作任务编号")?;
    let mut confirmed = writing_outline::read_confirmed_outline(root, &writing_id)?;
    confirmed.masked = counts.any();
    confirmed.masked_id_card = counts.id_card;
    confirmed.masked_phone = counts.phone;
    Ok(confirmed)
}

/// OD-WT-606：生成大纲提示词。只读任务与素材包，不写任何文件。
#[tauri::command(async)]
fn writing_outline_generate(
    state: State<'_, SharedHost>,
    request: writing_outline::OutlineGenerateRequest,
) -> Result<writing_outline::WritingOutlinePrompt, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    writing_outline_generate_impl(&root, &config_dir, &policy, request)
}

/// OD-WT-606：用户确认可编辑大纲 → outline.md + 快照 → 写作中。
#[tauri::command(async)]
fn writing_outline_confirm(
    state: State<'_, SharedHost>,
    request: writing_outline::OutlineConfirmRequest,
) -> Result<writing_outline::WritingOutlineConfirmed, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_outline_confirm_impl(&root, &policy, request, task_shell::now_secs())
}

/// OD-WT-606：读取已确认大纲；未确认时明确报错。
#[tauri::command(async)]
fn writing_outline_get(
    state: State<'_, SharedHost>,
    writing_id: String,
) -> Result<writing_outline::WritingOutlineConfirmed, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_outline_get_impl(&root, &policy, &writing_id)
}

// ---------------------------------------------------------------------------
// 第四阶段：分章节写作 IPC（OD-WT-607）
//
// 照 606 模式：宿主组提示词，模型调用仍在界面侧；生成/修订命令不写文件。
// 确认命令先把章节稿正文、引用原文、四分法文本与说明全部过红线与掩码，
// 再由 602 写正文.md 与完整正文快照，由 603 追加本章引用。AI 没有直接
// 定稿写通道，章节稿只有用户调用 confirm 才会落盘。
// ---------------------------------------------------------------------------

fn gate_section_draft(
    draft: &mut writing_section::SectionDraft,
    policy: &profile_rules::RulePolicy,
) -> Result<profile_rules::MaskCounts, String> {
    let mut total = profile_rules::MaskCounts::default();
    let (content, extra) = gate_user_text(&draft.content, policy, "章节稿正文")?;
    draft.content = content;
    add_counts(&mut total, extra);
    for citation in draft.citations.iter_mut() {
        let (quote, extra) = gate_user_text(&citation.quote, policy, "章节引用原文")?;
        citation.quote = quote;
        add_counts(&mut total, extra);
    }
    for segment in draft.segments.iter_mut() {
        let (text, extra) = gate_user_text(&segment.text, policy, "四分法标注文本")?;
        segment.text = text;
        add_counts(&mut total, extra);
        if let Some(quote) = segment.quote.clone() {
            let (masked, extra) = gate_user_text(&quote, policy, "四分法标注引用")?;
            segment.quote = Some(masked);
            add_counts(&mut total, extra);
        }
    }
    Ok(total)
}

fn writing_section_generate_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    request: writing_section::SectionGenerateRequest,
) -> Result<writing_section::WritingSectionPrompt, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    let mut prompt = writing_section::generate_section_prompt(
        root,
        &writing_section::SectionGenerateRequest {
            writing_id,
            section_no: request.section_no,
        },
    )?;
    prompt.masked = counts.any();
    prompt.masked_id_card = counts.id_card;
    prompt.masked_phone = counts.phone;
    Ok(prompt)
}

fn writing_section_confirm_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut request: writing_section::SectionConfirmRequest,
    now: f64,
) -> Result<writing_section::WritingSectionConfirmed, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    request.writing_id = writing_id;
    let mut total = counts;
    if let Some(note) = request.note.clone() {
        if !note.trim().is_empty() {
            let (masked, extra) = gate_user_text(&note, policy, "章节确认说明")?;
            request.note = Some(masked);
            add_counts(&mut total, extra);
        }
    }
    let draft_counts = gate_section_draft(&mut request.draft, policy)?;
    add_counts(&mut total, draft_counts);
    let mut confirmed = writing_section::confirm_section(root, &request, now)?;
    confirmed.masked = total.any();
    confirmed.masked_id_card = total.id_card;
    confirmed.masked_phone = total.phone;
    Ok(confirmed)
}

fn writing_section_get_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    writing_id: &str,
    section_no: u32,
) -> Result<writing_section::WritingSectionState, String> {
    let (writing_id, counts) = gate_user_text(writing_id, policy, "写作任务编号")?;
    let mut state = writing_section::read_section_state(root, &writing_id, section_no)?;
    state.masked = counts.any();
    state.masked_id_card = counts.id_card;
    state.masked_phone = counts.phone;
    Ok(state)
}

fn writing_section_revise_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut request: writing_section::SectionReviseRequest,
) -> Result<writing_section::WritingSectionRevisePrompt, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    request.writing_id = writing_id;
    let mut total = counts;
    if !request.instruction.trim().is_empty() {
        let (instruction, extra) = gate_user_text(&request.instruction, policy, "修订补充说明")?;
        request.instruction = instruction;
        add_counts(&mut total, extra);
    }
    let mut prompt = writing_section::revise_section_prompt(root, &request)?;
    prompt.masked = total.any();
    prompt.masked_id_card = total.id_card;
    prompt.masked_phone = total.phone;
    Ok(prompt)
}

/// OD-WT-607：生成章节提示词。只读任务、大纲与本章素材，不写任何文件。
#[tauri::command(async)]
fn writing_section_generate(
    state: State<'_, SharedHost>,
    request: writing_section::SectionGenerateRequest,
) -> Result<writing_section::WritingSectionPrompt, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_section_generate_impl(&root, &policy, request)
}

/// OD-WT-607：用户确认章节稿 → 正文.md + 快照 + 本章引用。
#[tauri::command(async)]
fn writing_section_confirm(
    state: State<'_, SharedHost>,
    request: writing_section::SectionConfirmRequest,
) -> Result<writing_section::WritingSectionConfirmed, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_section_confirm_impl(&root, &policy, request, task_shell::now_secs())
}

/// OD-WT-607：读取当前正文中的指定章节。
#[tauri::command(async)]
fn writing_section_get(
    state: State<'_, SharedHost>,
    writing_id: String,
    section_no: u32,
) -> Result<writing_section::WritingSectionState, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_section_get_impl(&root, &policy, &writing_id, section_no)
}

/// OD-WT-607：构建章节修订提示词；应用修订仍走 writing_section_confirm。
#[tauri::command(async)]
fn writing_section_revise(
    state: State<'_, SharedHost>,
    request: writing_section::SectionReviseRequest,
) -> Result<writing_section::WritingSectionRevisePrompt, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_section_revise_impl(&root, &policy, request)
}

// ---------------------------------------------------------------------------
// 第四阶段：事实核验 IPC（OD-WT-609）
//
// 确定性引擎在 fact_check.rs（纯函数、无 IO）；本组命令只负责闸门、读取
// 已选素材/知识依据、调用 603 追加核验记录与用户处理单条结论。无法核实
// 只落「无法找到来源」，不删除来源、不编造依据；口径冲突只提示不合并。
// ---------------------------------------------------------------------------

fn knowledge_subject_matches(subject: &str, source_id: &str) -> bool {
    let subject = subject.trim().trim_start_matches("知识库/");
    let source_id = source_id.trim();
    !subject.is_empty()
        && !source_id.is_empty()
        && (subject == source_id || subject.ends_with(source_id) || source_id.ends_with(subject))
}

fn fact_check_knowledge_evidence(
    root: &std::path::Path,
    task: &writing_store::WritingTask,
    request_evidence: &[fact_check::KnowledgeEvidenceInput],
    vault: Option<&std::path::Path>,
    config_dir: &std::path::Path,
    now: f64,
    offset_minutes: i32,
    warnings: &mut Vec<String>,
) -> Result<Vec<fact_check::FactEvidence>, String> {
    let mut evidence: Vec<fact_check::FactEvidence> = Vec::new();
    for item in request_evidence {
        let source_id = item.source_id.trim();
        if source_id.is_empty() {
            return Err("知识核验依据编号不能为空。".to_string());
        }
        let quote = item.quote.trim();
        if quote.is_empty() {
            continue;
        }
        if quote.chars().count() > 200_000 {
            return Err(format!("知识核验依据 {source_id} 的引用原文过长。"));
        }
        evidence.push(fact_check::FactEvidence {
            source_kind: writing_records::CitationSourceKind::Knowledge,
            source_id: source_id.to_string(),
            quote: quote.to_string(),
            outdated: item.outdated,
        });
    }
    let citations = writing_records::list_citations(root, &task.writing_id)?;
    let mut knowledge_ids: Vec<String> = task.selected_knowledge_ids.clone();
    for citation in &citations {
        if citation.source_kind == writing_records::CitationSourceKind::Knowledge {
            knowledge_ids.push(citation.source_id.clone());
            evidence.push(fact_check::FactEvidence {
                source_kind: writing_records::CitationSourceKind::Knowledge,
                source_id: citation.source_id.clone(),
                quote: citation.quote.clone(),
                outdated: false,
            });
        }
    }
    knowledge_ids.retain(|id| !id.trim().is_empty());
    knowledge_ids.sort();
    knowledge_ids.dedup();
    let expired_subjects: Vec<String> = match vault {
        Some(vault) => match knowledge_quality::load_config(config_dir) {
            Ok(config) => knowledge_quality::check_expiry(
                vault,
                now as i64,
                offset_minutes,
                config.expiry_days,
            )
            .map(|report| report.expired.into_iter().map(|item| item.subject).collect())
            .unwrap_or_default(),
            Err(error) => {
                warnings.push(format!("知识过期配置读取失败：{error}"));
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    if let Some(vault) = vault {
        for id in &knowledge_ids {
            if evidence
                .iter()
                .any(|item| item.source_id == *id && !item.quote.trim().is_empty())
            {
                continue;
            }
            match knowledge_vault::read_vault_note(vault, id) {
                Ok(note) => evidence.push(fact_check::FactEvidence {
                    source_kind: writing_records::CitationSourceKind::Knowledge,
                    source_id: id.clone(),
                    quote: note.content,
                    outdated: false,
                }),
                Err(error) => warnings.push(format!("知识条目 {id} 读取失败：{error}")),
            }
        }
    }
    let mut merged: Vec<fact_check::FactEvidence> = Vec::new();
    for item in evidence {
        if let Some(existing) = merged.iter_mut().find(|existing| {
            existing.source_kind == item.source_kind
                && existing.source_id == item.source_id
                && existing.quote == item.quote
        }) {
            existing.outdated |= item.outdated;
        } else {
            merged.push(item);
        }
    }
    for item in merged.iter_mut() {
        if item.source_kind == writing_records::CitationSourceKind::Knowledge
            && expired_subjects
                .iter()
                .any(|subject| knowledge_subject_matches(subject, &item.source_id))
        {
            item.outdated = true;
        }
    }
    Ok(merged)
}

fn writing_factcheck_run_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    request: fact_check::FactCheckRunRequest,
    now: f64,
    vault: Option<&std::path::Path>,
    config_dir: &std::path::Path,
    offset_minutes: i32,
) -> Result<fact_check::FactCheckRunResponse, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    let task = writing_store::read_writing_task(root, &writing_id)?;
    let allowed = task.status == writing_store::STATUS_WRITING
        || task.status == writing_store::STATUS_FACT_CHECKING
        || task.status == writing_store::STATUS_PENDING_REVIEW;
    if !allowed {
        return Err(format!(
            "写作任务当前状态为「{}」；只有「{}」「{}」或「{}」可以运行事实核验。",
            task.status,
            writing_store::STATUS_WRITING,
            writing_store::STATUS_FACT_CHECKING,
            writing_store::STATUS_PENDING_REVIEW
        ));
    }
    let full_text = writing_section::compose_full_text_for(root, &writing_id)?;
    let mut warnings = Vec::new();
    let mut evidence = Vec::new();
    for id in &task.selected_material_ids {
        let material = material_store::read_material(root, id)
            .map_err(|error| format!("读取已选素材 {id} 失败：{error}"))?;
        let quote = if material.source_quote.trim().is_empty() {
            material.content.clone()
        } else {
            material.source_quote.clone()
        };
        if quote.trim().is_empty() {
            warnings.push(format!("素材 {id} 没有可核验的来源原文。"));
            continue;
        }
        evidence.push(fact_check::FactEvidence {
            source_kind: writing_records::CitationSourceKind::Material,
            source_id: id.clone(),
            quote,
            outdated: false,
        });
    }
    evidence.extend(fact_check_knowledge_evidence(
        root,
        &task,
        &request.knowledge_evidence,
        vault,
        config_dir,
        now,
        offset_minutes,
        &mut warnings,
    )?);
    let outcome = fact_check::run_fact_check(&full_text, &evidence, &request.annotations)?;
    let existing_ids: Vec<String> = writing_records::list_fact_checks(root, &writing_id)?
        .into_iter()
        .map(|record| record.record_id)
        .collect();
    let mut appended = 0usize;
    let mut skipped_existing = 0usize;
    for finding in &outcome.findings {
        if existing_ids
            .iter()
            .any(|record_id| record_id == &finding.record_id)
        {
            skipped_existing += 1;
            continue;
        }
        writing_records::append_fact_check(
            root,
            &writing_id,
            &writing_records::FactCheckDraft {
                record_id: Some(finding.record_id.clone()),
                section_no: finding.section_no,
                paragraph_no: finding.paragraph_no,
                check_object: finding.check_object,
                conclusion: finding.conclusion,
                basis: finding.basis.clone(),
                fact_text: Some(finding.fact_text.clone()),
            },
            now,
        )?;
        appended += 1;
    }
    let records = writing_records::list_fact_checks(root, &writing_id)?;
    Ok(fact_check::FactCheckRunResponse {
        writing_id: task.writing_id,
        status: task.status,
        records,
        conflicts: outcome.conflicts,
        stats: outcome.stats,
        appended,
        skipped_existing,
        warnings,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

fn writing_factcheck_resolve_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    request: fact_check::FactCheckResolveRequest,
) -> Result<fact_check::FactCheckResolveResponse, String> {
    let (writing_id, mut counts) =
        gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    let resolution = writing_records::ResolutionStatus::from_label(&request.resolution)?;
    if resolution == writing_records::ResolutionStatus::Unresolved {
        return Err("核验处理状态只能由用户改为「已接受」「已修改」或「已删除来源项」。"
            .to_string());
    }
    let note = match request.note {
        Some(note) if !note.trim().is_empty() => {
            if note.chars().count() > 500 {
                return Err("核验处理备注最多 500 字符。".to_string());
            }
            let (masked, extra) = gate_user_text(&note, policy, "核验处理备注")?;
            add_counts(&mut counts, extra);
            Some(masked)
        }
        _ => None,
    };
    let record = writing_records::update_resolution(
        root,
        &writing_id,
        &request.record_id,
        &request.resolution,
    )?;
    Ok(fact_check::FactCheckResolveResponse {
        record,
        note,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

/// OD-WT-609：运行事实核验。只读正文与已选依据，结论经 603 追加落盘。
#[tauri::command(async)]
fn writing_factcheck_run(
    state: State<'_, SharedHost>,
    request: fact_check::FactCheckRunRequest,
) -> Result<fact_check::FactCheckRunResponse, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let vault = configured_vault_root(&paths).ok();
    let offset = worklog_offset_minutes(&state);
    writing_factcheck_run_impl(
        &root,
        &policy,
        request,
        task_shell::now_secs(),
        vault.as_deref(),
        &paths.config,
        offset,
    )
}

/// OD-WT-609：用户处理单条核验结论，走 603 按行重写。
#[tauri::command(async)]
fn writing_factcheck_resolve(
    state: State<'_, SharedHost>,
    request: fact_check::FactCheckResolveRequest,
) -> Result<fact_check::FactCheckResolveResponse, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_factcheck_resolve_impl(&root, &policy, request)
}

// ---------------------------------------------------------------------------
// 第四阶段：审阅修改与版本 IPC（OD-WT-608）
//
// 架构契合：薄封装 607 的修订确认通道 + 602 版本存储 + 603 记录；十维审阅
// 检查单只作为宿主组提示词，模型调用仍在界面侧，零新增模型通道。审阅结果
// 只是问题清单，不写正文；用户应用某条问题后，修订稿仍经 607 confirm_section
// 同一路径写入正文与快照，再经 601 追加应用留痕。版本恢复必须显式确认，
// 状态不变、只追加「恢复 vX」状态记录。
// ---------------------------------------------------------------------------

fn writing_review_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    request: writing_review::ReviewRequest,
) -> Result<writing_review::WritingReviewPrompt, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    let mut total = counts;
    let mut instruction = request.instruction;
    if !instruction.trim().is_empty() {
        let (masked, extra) = gate_user_text(&instruction, policy, "审阅补充说明")?;
        instruction = masked;
        add_counts(&mut total, extra);
    }
    let mut prompt = writing_review::build_review_prompt(
        root,
        &writing_review::ReviewRequest {
            writing_id,
            instruction,
        },
    )?;
    prompt.masked = total.any();
    prompt.masked_id_card = total.id_card;
    prompt.masked_phone = total.phone;
    Ok(prompt)
}

fn writing_review_apply_finding_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut request: writing_review::ReviewApplyRequest,
    now: f64,
) -> Result<writing_review::ReviewFindingApplied, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    request.writing_id = writing_id;
    let mut total = counts;
    if let Some(note) = request.note.clone() {
        if !note.trim().is_empty() {
            let (masked, extra) = gate_user_text(&note, policy, "审阅修订说明")?;
            request.note = Some(masked);
            add_counts(&mut total, extra);
        }
    }
    let (finding_id, extra) =
        gate_user_text(&request.finding.finding_id, policy, "审阅问题编号")?;
    request.finding.finding_id = finding_id;
    add_counts(&mut total, extra);
    let (description, extra) =
        gate_user_text(&request.finding.description, policy, "审阅问题描述")?;
    request.finding.description = description;
    add_counts(&mut total, extra);
    if !request.finding.suggestion.trim().is_empty() {
        let (suggestion, extra) =
            gate_user_text(&request.finding.suggestion, policy, "审阅修改建议")?;
        request.finding.suggestion = suggestion;
        add_counts(&mut total, extra);
    }
    let draft_counts = gate_section_draft(&mut request.draft, policy)?;
    add_counts(&mut total, draft_counts);
    let mut applied = writing_review::apply_finding(root, &request, now)?;
    applied.masked = total.any();
    applied.masked_id_card = total.id_card;
    applied.masked_phone = total.phone;
    Ok(applied)
}

fn writing_version_list_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    writing_id: &str,
) -> Result<writing_review::WritingVersionList, String> {
    let (writing_id, _) = gate_user_text(writing_id, policy, "写作任务编号")?;
    writing_review::list_version_summaries(root, &writing_id)
}

fn writing_version_diff_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut request: writing_review::VersionDiffRequest,
) -> Result<writing_review::WritingVersionDiff, String> {
    let (writing_id, _) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    request.writing_id = writing_id;
    writing_review::diff_versions(root, &request)
}

fn writing_version_restore_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut request: writing_review::VersionRestoreRequest,
    now: f64,
) -> Result<writing_review::WritingVersionRestored, String> {
    let (writing_id, _) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    request.writing_id = writing_id;
    writing_review::restore_version(root, &request, now)
}

/// OD-WT-608：构建十维审阅提示词。只读任务、大纲与正文，不写任何文件。
#[tauri::command(async)]
fn writing_review(
    state: State<'_, SharedHost>,
    request: writing_review::ReviewRequest,
) -> Result<writing_review::WritingReviewPrompt, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_review_impl(&root, &policy, request)
}

/// OD-WT-608：用户应用某条审阅问题 → 607 确认通道写正文/快照 + 状态记录留痕。
#[tauri::command(async)]
fn writing_review_apply_finding(
    state: State<'_, SharedHost>,
    request: writing_review::ReviewApplyRequest,
) -> Result<writing_review::ReviewFindingApplied, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_review_apply_finding_impl(&root, &policy, request, task_shell::now_secs())
}

/// OD-WT-608：版本列表（含每版说明与段落差异摘要）。
#[tauri::command(async)]
fn writing_version_list(
    state: State<'_, SharedHost>,
    writing_id: String,
) -> Result<writing_review::WritingVersionList, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_version_list_impl(&root, &policy, &writing_id)
}

/// OD-WT-608：两份历史版本的段落级对比。
#[tauri::command(async)]
fn writing_version_diff(
    state: State<'_, SharedHost>,
    request: writing_review::VersionDiffRequest,
) -> Result<writing_review::WritingVersionDiff, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_version_diff_impl(&root, &policy, request)
}

/// OD-WT-608：恢复历史版本（须用户确认；状态不变，追加「恢复 vX」留痕）。
#[tauri::command(async)]
fn writing_version_restore(
    state: State<'_, SharedHost>,
    request: writing_review::VersionRestoreRequest,
) -> Result<writing_review::WritingVersionRestored, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_version_restore_impl(&root, &policy, request, task_shell::now_secs())
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
    platform::home_dir().map(|home| {
        home
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

/// 建立收件箱/概念/方法/原文骨架与空索引（幂等）。
#[tauri::command(async)]
fn knowledge_ingest_init(
    state: State<'_, SharedHost>,
) -> Result<knowledge_ingest::IngestInit, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = configured_vault_root(&paths)?;
    knowledge_ingest::init_vault(&root)
}

/// 列出收件箱前 5 份材料，脱敏后组装模型提示词。
#[tauri::command(async)]
fn knowledge_ingest_prepare(
    state: State<'_, SharedHost>,
) -> Result<knowledge_ingest::PrepareResult, String> {
    let (paths, config) = {
        let host = state.lock().unwrap();
        let paths = host.paths.clone();
        let config = EngineConfig::from_env(&paths.root)?;
        (paths, config)
    };
    let root = configured_vault_root(&paths)?;
    let user_dict = paths.config.join("sensitive-dict.v1.json");
    let dictionary = if user_dict.is_file() {
        Some(user_dict)
    } else {
        crate::platform::repo_root()
            .map(|repo| repo.join("config/sensitive-dict.sample.v1.json"))
            .filter(|path| path.is_file())
    };
    knowledge_ingest::prepare_inbox(
        &root,
        &config.dsh_home,
        &config.python,
        &config.engine_dir,
        dictionary.as_deref(),
    )
}

/// 校验模型返回的 JSON，逐卡落盘、追加索引并归档原文。
#[tauri::command(async)]
fn knowledge_ingest_apply(
    state: State<'_, SharedHost>,
    result_text: String,
) -> Result<knowledge_ingest::ApplyReport, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = configured_vault_root(&paths)?;
    knowledge_ingest::init_vault(&root)?;
    let scan = knowledge_vault::scan_vault(&root, Some(&paths.root))?;
    let value = knowledge_ingest::parse_result_text(&result_text)?;
    Ok(knowledge_ingest::apply_payload(&root, &value, &scan))
}

/// 把一批原始资料字节准入到 raw/，并记录哈希、准入时间与字节数。
/// 先幂等初始化骨架；同哈希拒绝，同名不同内容换名，raw 永不覆盖。
#[tauri::command(async)]
fn knowledge_raw_admit(
    state: State<'_, SharedHost>,
    files: Vec<knowledge_ingest::RawFileInput>,
) -> Result<knowledge_ingest::RawAdmitReport, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = configured_vault_root(&paths)?;
    let offset = worklog_offset_minutes(&state);
    let policy = current_rule_policy(&state)?;
    knowledge_ingest::init_vault(&root)?;
    Ok(knowledge_ingest::admit_raw_files_with_policy(
        &root, &files, &policy, offset,
    ))
}

/// 从系统文件选择框给出的绝对路径读取并准入 raw/。前端不直接读文件字节。
#[tauri::command(async)]
fn knowledge_raw_admit_paths(
    state: State<'_, SharedHost>,
    paths: Vec<String>,
) -> Result<knowledge_ingest::RawAdmitReport, String> {
    let paths_state = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = configured_vault_root(&paths_state)?;
    let offset = worklog_offset_minutes(&state);
    let policy = current_rule_policy(&state)?;
    knowledge_ingest::init_vault(&root)?;
    let mut files = Vec::new();
    let mut rejected = Vec::new();
    for raw in &paths {
        match knowledge_ingest::read_raw_file(std::path::Path::new(raw)) {
            Ok(file) => files.push(file),
            Err(reason) => rejected.push(knowledge_ingest::IngestRejection {
                relative_path: raw.clone(),
                reason,
            }),
        }
    }
    let mut report = knowledge_ingest::admit_raw_files_with_policy(
        &root, &files, &policy, offset,
    );
    if !rejected.is_empty() {
        report.rejected.splice(0..0, rejected);
        if report.admitted.is_empty() {
            report.ok = false;
        }
    }
    Ok(report)
}

/// 读取人工治理记录（只读）。治理文件没有对前端的写命令；写入只发生在宿主内部。
#[tauri::command(async)]
fn knowledge_governance_list(
    state: State<'_, SharedHost>,
    kind: String,
) -> Result<Vec<knowledge_governance::GovernanceRecord>, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = configured_vault_root(&paths)?;
    knowledge_governance::list_records(&root, &kind)
}

/// 把模型整理结果暂存为人工审核项；未通过校验时不进入队列。
#[tauri::command(async)]
fn knowledge_review_stage(
    state: State<'_, SharedHost>,
    result_text: String,
    source_paths: Option<Vec<String>>,
) -> Result<knowledge_review::ReviewItem, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let offset = worklog_offset_minutes(&state);
    let policy = current_rule_policy(&state)?;
    let root = configured_vault_root(&paths)?;
    knowledge_ingest::init_vault(&root)?;
    let review_dir = knowledge_review::dir(&paths.engine_state);
    knowledge_review::stage(
        &root,
        &review_dir,
        &result_text,
        source_paths.as_deref().unwrap_or(&[]),
        &policy,
        offset,
    )
}

#[tauri::command(async)]
fn knowledge_review_list(
    state: State<'_, SharedHost>,
) -> Result<knowledge_review::ReviewListing, String> {
    let engine_state = {
        let host = state.lock().unwrap();
        host.paths.engine_state.clone()
    };
    Ok(knowledge_review::list(&knowledge_review::dir(&engine_state)))
}

#[tauri::command(async)]
fn knowledge_review_decide(
    state: State<'_, SharedHost>,
    id: String,
    decision: String,
) -> Result<knowledge_review::ReviewDecisionReport, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let offset = worklog_offset_minutes(&state);
    let policy = current_rule_policy(&state)?;
    let root = configured_vault_root(&paths)?;
    let review_dir = knowledge_review::dir(&paths.engine_state);
    knowledge_review::decide(
        &root,
        &review_dir,
        &id,
        &decision,
        &policy,
        offset,
        Some(&paths.root),
    )
}

/// 冲突/异常的人工裁决；只写治理记录，不改知识文件。
#[tauri::command(async)]
fn knowledge_conflict_resolve(
    state: State<'_, SharedHost>,
    subject: String,
    decision: String,
    note: String,
) -> Result<knowledge_governance::GovernanceRecord, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let offset = worklog_offset_minutes(&state);
    let policy = current_rule_policy(&state)?;
    let root = configured_vault_root(&paths)?;
    knowledge_review::resolve_conflict(
        &root, &subject, &decision, &note, &policy, offset,
    )
}

/// 确定性过期检查：索引日期 + raw 准入日期，不调用模型。
#[tauri::command(async)]
fn knowledge_quality_report(
    state: State<'_, SharedHost>,
) -> Result<knowledge_quality::QualityReport, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let offset = worklog_offset_minutes(&state);
    let root = configured_vault_root(&paths)?;
    let config = knowledge_quality::load_config(&paths.config)?;
    knowledge_quality::check_expiry(
        &root,
        task_shell::now_secs() as i64,
        offset,
        config.expiry_days,
    )
}

// ---------------------------------------------------------------------------
// 知识关联 IPC（OD-KM-507）
//
// 组合 505 的只读检索与 503 的 related_knowledge 字段。AI 只产生候选，落
// work/知识关联/.candidates/；用户确认后宿主写记录/任务 frontmatter。
// 知识库只读，本组命令不触碰 knowledge_vault 的写路径。
// ---------------------------------------------------------------------------

fn knowledge_link_suggest_impl(
    root: &std::path::Path,
    vault_root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    subject_kind: knowledge_link_store::LinkSubjectKind,
    subject_id: &str,
    knowledge_hints: Vec<String>,
    now: f64,
) -> Result<knowledge_link_store::KnowledgeLinkStageReport, String> {
    let mut gated = Vec::with_capacity(knowledge_hints.len());
    for hint in knowledge_hints {
        let (masked, _) = gate_user_text(&hint, policy, "知识主题提示")?;
        gated.push(masked);
    }
    knowledge_link_store::suggest_and_stage(
        root,
        vault_root,
        subject_kind,
        subject_id.trim(),
        &gated,
        now,
    )
}

fn knowledge_link_confirm_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    candidate_id: &str,
    reason: &str,
    now: f64,
) -> Result<knowledge_link_store::KnowledgeLinkReport, String> {
    let (reason, _) = gate_user_text(reason, policy, "关联理由")?;
    knowledge_link_store::confirm_candidate(root, candidate_id.trim(), &reason, now)
}

fn knowledge_link_remove_impl(
    root: &std::path::Path,
    subject_kind: knowledge_link_store::LinkSubjectKind,
    subject_id: &str,
    knowledge_id: &str,
    now: f64,
) -> Result<knowledge_link_store::KnowledgeLinkReport, String> {
    knowledge_link_store::remove_link(
        root,
        subject_kind,
        subject_id.trim(),
        knowledge_id.trim(),
        now,
    )
}

fn knowledge_link_by_knowledge_impl(
    root: &std::path::Path,
    knowledge_id: &str,
) -> Result<knowledge_link_store::KnowledgeLinkReferences, String> {
    knowledge_link_store::references_for_knowledge(root, knowledge_id.trim())
}

#[tauri::command(async)]
fn knowledge_link_suggest(
    state: State<'_, SharedHost>,
    subject_kind: knowledge_link_store::LinkSubjectKind,
    subject_id: String,
    knowledge_hints: Vec<String>,
) -> Result<knowledge_link_store::KnowledgeLinkStageReport, String> {
    let root = work_root(&state)?;
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let vault = configured_vault_root(&paths)?;
    let policy = current_rule_policy(&state)?;
    knowledge_link_suggest_impl(
        &root,
        &vault,
        &policy,
        subject_kind,
        &subject_id,
        knowledge_hints,
        task_shell::now_secs(),
    )
}

#[tauri::command(async)]
fn knowledge_link_confirm(
    state: State<'_, SharedHost>,
    candidate_id: String,
    reason: String,
) -> Result<knowledge_link_store::KnowledgeLinkReport, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    knowledge_link_confirm_impl(
        &root,
        &policy,
        &candidate_id,
        &reason,
        task_shell::now_secs(),
    )
}

#[tauri::command(async)]
fn knowledge_link_remove(
    state: State<'_, SharedHost>,
    subject_kind: knowledge_link_store::LinkSubjectKind,
    subject_id: String,
    knowledge_id: String,
) -> Result<knowledge_link_store::KnowledgeLinkReport, String> {
    let root = work_root(&state)?;
    knowledge_link_remove_impl(
        &root,
        subject_kind,
        &subject_id,
        &knowledge_id,
        task_shell::now_secs(),
    )
}

#[tauri::command(async)]
fn knowledge_link_by_knowledge(
    state: State<'_, SharedHost>,
    knowledge_id: String,
) -> Result<knowledge_link_store::KnowledgeLinkReferences, String> {
    let root = work_root(&state)?;
    knowledge_link_by_knowledge_impl(&root, &knowledge_id)
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
    let root = match work_root(&state) {
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
    match quicklog::append(
        &root,
        &date,
        &masked_text,
        destination.as_deref(),
        now,
        offset,
    ) {
        Ok(mut receipt) => {
            receipt.masked = counts.any();
            receipt.masked_id_card = counts.id_card;
            receipt.masked_phone = counts.phone;
            serde_json::json!(receipt)
        }
        Err(message) => serde_json::json!({
            "ok": false,
            "path": worklog_records::records_dir(&root).display().to_string(),
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
    let root = match work_root(&state) {
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
    match quicklog::set_destination(
        &root,
        &week,
        &date,
        index,
        &destination,
        &expect_text,
        task_shell::now_secs(),
    ) {
        Ok(receipt) => serde_json::json!(receipt),
        Err(message) => serde_json::json!({
            "ok": false,
            "path": worklog_records::records_dir(&root).display().to_string(),
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
    let offset = worklog_offset_minutes(&state);
    let root = match work_root(&state) {
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
    match quicklog::redate(
        &root,
        &week,
        &from_date,
        index,
        &to_date,
        &expect_text,
        task_shell::now_secs(),
        offset,
    ) {
        Ok(receipt) => serde_json::json!(receipt),
        Err(message) => serde_json::json!({
            "ok": false,
            "path": worklog_records::records_dir(&root).display().to_string(),
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


/// 批量改日期（OD-ARCH-104）。
///
/// 随手记的语义判定由模型完成，界面拿回判定后把 moves 一次性交给这里：
/// 一次读入、逐条按 expect_text 校验、全部通过才重写一次。任一条对不上就整批拒绝，
/// 文件一字不动。跨周与晚于今天的日期同样拒绝。
#[tauri::command(async)]
fn worklog_move_many(
    state: State<'_, SharedHost>,
    week: String,
    moves: Vec<worklog::MoveRequest>,
) -> serde_json::Value {
    let offset = worklog_offset_minutes(&state);
    let today = worklog::date_label(task_shell::now_secs(), offset);
    let root = match work_root(&state) {
        Ok(root) => root,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "path": null,
                "week": week,
                "moves": [],
                "backup": null,
                "error": message,
            })
        }
    };
    if let Err(message) = worklog::assert_week_label(&week) {
        return serde_json::json!({
            "ok": false,
            "path": null,
            "week": week,
            "moves": [],
            "backup": null,
            "error": message,
        });
    }
    match quicklog::move_many(&root, &week, &moves, &today, task_shell::now_secs()) {
        Ok(receipt) => serde_json::json!(receipt),
        Err(message) => serde_json::json!({
            "ok": false,
            "path": worklog_records::records_dir(&root).display().to_string(),
            "week": week,
            "moves": [],
            "backup": null,
            "error": message,
        }),
    }
}

/// 把一句补充并入已有记录（OD-ARCH-132）。
///
/// 与追加同口径：先过红线扫描，命中即拒绝；身份证/手机号掩码后才落盘。
/// 定位用 date + index + expect_text，对不上就整份拒绝，文件一字不动。
#[tauri::command(async)]
fn worklog_merge(
    state: State<'_, SharedHost>,
    week: String,
    date: String,
    index: usize,
    expect_text: String,
    text: String,
) -> serde_json::Value {
    let failure = |path: Option<String>, message: String, counts: (u64, u64, bool)| {
        serde_json::json!({
            "ok": false,
            "path": path,
            "week": week,
            "date": date,
            "index": index,
            "entry_text": "",
            "added": "",
            "merged": false,
            "masked": counts.2,
            "masked_id_card": counts.0,
            "masked_phone": counts.1,
            "backup": null,
            "error": message,
        })
    };
    let root = match work_root(&state) {
        Ok(root) => root,
        Err(message) => return failure(None, message, (0, 0, false)),
    };
    let path = worklog_records::records_dir(&root).display().to_string();
    let policy = match current_rule_policy(&state) {
        Ok(policy) => policy,
        Err(message) => return failure(Some(path), message, (0, 0, false)),
    };
    let hits = task_shell::red_line_scan_with(&text, &policy);
    if !hits.is_empty() {
        let mut detail = String::new();
        for hit in &hits {
            detail.push_str(&format!("{}（{} 处）；", hit.label, hit.count));
        }
        return failure(
            Some(path),
            format!(
                "本次内容命中红线扫描，已拒绝写入工作记录：{}。该扫描仅为启发式，不代表合规判断。",
                detail
            ),
            (0, 0, false),
        );
    }
    let (masked_text, counts) = profile_rules::mask_privacy(&text, &policy);
    let counts = (counts.id_card, counts.phone, counts.any());
    match quicklog::merge(
        &root,
        &week,
        &date,
        index,
        &expect_text,
        &masked_text,
        task_shell::now_secs(),
    ) {
        Ok(mut receipt) => {
            receipt.masked = counts.2;
            receipt.masked_id_card = counts.0;
            receipt.masked_phone = counts.1;
            serde_json::json!(receipt)
        }
        Err(message) => failure(Some(path), message, counts),
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
    let work = match work_root(&state) {
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
    if let Err(message) = worklog::assert_week_label(&resolved_week) {
        return serde_json::json!({
            "ok": false,
            "exists": false,
            "week": resolved_week,
            "path": null,
            "content": "",
            "error": message,
        });
    }
    match quicklog::week_entries(&work, &resolved_week) {
        Ok(entries) if !entries.is_empty() => {
            let content = quicklog::render_week_markdown(&work, &resolved_week)
                .unwrap_or_default();
            serde_json::json!({
                "ok": true,
                "exists": true,
                "week": resolved_week,
                "path": worklog_records::records_dir(&work).display().to_string(),
                "content": content,
                "error": null,
                "source": "records",
            })
        }
        Ok(_) => match worklog_library_root(&state) {
            Ok(root) => match worklog::read_week(&root, &resolved_week) {
                Ok((exists, content, path)) => serde_json::json!({
                    "ok": true,
                    "exists": exists,
                    "week": resolved_week,
                    "path": path.display().to_string(),
                    "content": content,
                    "error": null,
                    "source": "legacy",
                }),
                Err(message) => serde_json::json!({
                    "ok": false,
                    "exists": false,
                    "week": resolved_week,
                    "path": worklog::week_path(&root, &resolved_week).display().to_string(),
                    "content": "",
                    "error": message,
                    "source": "legacy",
                }),
            },
            Err(_) => serde_json::json!({
                "ok": true,
                "exists": false,
                "week": resolved_week,
                "path": null,
                "content": "",
                "error": null,
                "source": "none",
            }),
        },
        Err(message) => serde_json::json!({
            "ok": false,
            "exists": false,
            "week": resolved_week,
            "path": null,
            "content": "",
            "error": message,
        }),
    }
}

/// 结构化周视图（OD-WR-311）：按记录库的 ISO 周过滤，供时间线与四象限直接消费。
#[tauri::command(async)]
fn worklog_week_view(
    state: State<'_, SharedHost>,
    week: Option<String>,
) -> Result<quicklog::WeekView, String> {
    let offset = worklog_offset_minutes(&state);
    let resolved = match week.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.to_string(),
        None => worklog::week_label(task_shell::now_secs(), offset),
    };
    worklog::assert_week_label(&resolved)?;
    let root = work_root(&state)?;
    quicklog::week_view(&root, &resolved)
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
    let work = match work_root(&state) {
        Ok(work) => work,
        Err(message) => {
            return serde_json::json!({
                "ok": false,
                "library_root": root.display().to_string(),
                "current_week": current,
                "weeks": [],
                "reports": [],
                "skipped": 0,
                "error": message,
            })
        }
    };
    let record_weeks = quicklog::list_record_weeks(&work).unwrap_or_default();
    match (worklog::list_weeks(&root), worklog::list_reports(&root)) {
        (Ok((mut weeks, skipped_weeks)), Ok((reports, skipped_reports))) => {
            for record_week in record_weeks {
                if weeks.iter().any(|week| week.week == record_week.week) {
                    continue;
                }
                weeks.push(worklog::WeekFile {
                    week: record_week.week.clone(),
                    file_name: format!("{}.md", record_week.week),
                    path: worklog_records::records_dir(&work).display().to_string(),
                    bytes: 0,
                    modified_at: 0.0,
                });
            }
            weeks.sort_by(|a, b| a.week.cmp(&b.week));
            serde_json::json!({
                "ok": true,
                "library_root": root.display().to_string(),
                "current_week": current,
                "weeks": weeks,
                "reports": reports,
                "skipped": skipped_weeks + skipped_reports,
                "error": null,
            })
        }
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

// ---------------------------------------------------------------------------
// 工作记录 / 任务 / 候选 IPC（OD-WR-302）
//
// 存储层只认绝对根与已校验编号；工作根由 settings 解析（OD-WR-315）：
//   OMNIDESK_WORK_ROOT > settings.v1 working_root > 默认 $HOME/OmniDesk/work。
// 写入前统一过红线扫描与 mask_privacy；命中红线整次拒绝，错误里不回显原文。
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkRecordSaved {
    pub record: worklog_records::WorkRecord,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskSaved {
    pub task: task_store::Task,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskDeleted {
    pub id: String,
    pub path: String,
}

fn work_root(state: &State<'_, SharedHost>) -> Result<std::path::PathBuf, String> {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    settings::effective_working_root(&config_dir).map(|eff| eff.root)
}

/// 红线扫描 + 隐私掩码的统一入口。命中红线直接拒绝；返回掩码后的文本与计数。
fn gate_user_text(
    text: &str,
    policy: &profile_rules::RulePolicy,
    target: &str,
) -> Result<(String, profile_rules::MaskCounts), String> {
    let hits = task_shell::red_line_scan_with(text, policy);
    if !hits.is_empty() {
        let mut detail = String::new();
        for hit in &hits {
            detail.push_str(&format!("{}（{} 处）；", hit.label, hit.count));
        }
        return Err(format!(
            "{target}命中红线扫描，已拒绝写入：{detail}该扫描仅为启发式，不代表合规判断。"
        ));
    }
    Ok(profile_rules::mask_privacy(text, policy))
}

fn add_counts(total: &mut profile_rules::MaskCounts, extra: profile_rules::MaskCounts) {
    total.id_card += extra.id_card;
    total.phone += extra.phone;
}

fn work_record_create_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut draft: worklog_records::RecordDraft,
    now: f64,
    utc_offset_minutes: i32,
) -> Result<WorkRecordSaved, String> {
    let mut counts = profile_rules::MaskCounts::default();
    if let Some(title) = draft.title.clone() {
        if !title.trim().is_empty() {
            let (masked, extra) = gate_user_text(&title, policy, "记录标题")?;
            add_counts(&mut counts, extra);
            draft.title = Some(masked);
        }
    }
    let (body, extra) = gate_user_text(&draft.body, policy, "记录正文")?;
    add_counts(&mut counts, extra);
    draft.body = body;
    let record = worklog_records::create_record(root, &draft, now, utc_offset_minutes)?;
    Ok(WorkRecordSaved {
        record,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

fn work_record_update_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    id: &str,
    mut patch: worklog_records::RecordPatch,
    now: f64,
) -> Result<WorkRecordSaved, String> {
    let mut counts = profile_rules::MaskCounts::default();
    if let Some(title) = patch.title.clone() {
        if !title.trim().is_empty() {
            let (masked, extra) = gate_user_text(&title, policy, "记录标题")?;
            add_counts(&mut counts, extra);
            patch.title = Some(masked);
        }
    }
    if let Some(body) = patch.body.clone() {
        let (masked, extra) = gate_user_text(&body, policy, "记录正文")?;
        add_counts(&mut counts, extra);
        patch.body = Some(masked);
    }
    let record = worklog_records::update_record(root, id, &patch, now)?;
    Ok(WorkRecordSaved {
        record,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

fn task_create_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut draft: task_store::TaskDraft,
    now: f64,
) -> Result<TaskSaved, String> {
    let (title, counts) = gate_user_text(&draft.title, policy, "任务标题")?;
    draft.title = title;
    if let Some(project_id) = draft.project_id.as_deref() {
        ensure_project_exists(root, project_id)?;
    }
    let task = task_store::create_task(root, &draft, now)?;
    Ok(TaskSaved {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

fn task_update_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    id: &str,
    mut patch: task_store::TaskPatch,
    now: f64,
) -> Result<TaskSaved, String> {
    let mut counts = profile_rules::MaskCounts::default();
    if let Some(title) = patch.title.clone() {
        if !title.trim().is_empty() {
            let (masked, extra) = gate_user_text(&title, policy, "任务标题")?;
            add_counts(&mut counts, extra);
            patch.title = Some(masked);
        }
    }
    if let Some(project_id) = patch.project_id.as_deref() {
        ensure_project_exists(root, project_id)?;
    }
    let task = task_store::update_task(root, id, &patch, now)?;
    Ok(TaskSaved {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

fn task_transition_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    id: &str,
    to_state: &str,
    note: Option<String>,
    now: f64,
) -> Result<TaskSaved, String> {
    let mut counts = profile_rules::MaskCounts::default();
    let note = match note {
        Some(raw) => {
            let (masked, extra) = gate_user_text(&raw, policy, "状态备注")?;
            add_counts(&mut counts, extra);
            Some(masked)
        }
        None => None,
    };
    let task = task_store::transition_task(root, id, to_state, note.as_deref(), now)?;
    Ok(TaskSaved {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

fn task_append_progress_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    id: &str,
    text: &str,
    now: f64,
) -> Result<TaskSaved, String> {
    let (masked, counts) = gate_user_text(text, policy, "任务进展")?;
    let task = task_store::append_progress(root, id, &masked, now)?;
    Ok(TaskSaved {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

fn task_candidate_confirm_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    candidate_id: &str,
    mut edits: task_store::ConfirmEdits,
    now: f64,
) -> Result<TaskSaved, String> {
    let mut counts = profile_rules::MaskCounts::default();
    // 候选标题来自模型；确认落盘前同样要过红线与掩码，不能因为「用户没改」就跳过。
    let final_title = match edits.title.clone() {
        Some(title) => title,
        None => task_store::read_pending_candidate(root, candidate_id)?.title,
    };
    if !final_title.trim().is_empty() {
        let (masked, extra) = gate_user_text(&final_title, policy, "任务标题")?;
        add_counts(&mut counts, extra);
        edits.title = Some(masked);
    }
    if let Some(note) = edits.note.clone() {
        let (masked, extra) = gate_user_text(&note, policy, "确认备注")?;
        add_counts(&mut counts, extra);
        edits.note = Some(masked);
    }
    let task = task_store::confirm_candidate(root, candidate_id, &edits, now)?;
    Ok(TaskSaved {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

fn task_candidate_reject_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    candidate_id: &str,
    note: Option<String>,
    now: f64,
) -> Result<task_store::TaskCandidate, String> {
    let note = match note {
        Some(raw) => Some(gate_user_text(&raw, policy, "拒绝备注")?.0),
        None => None,
    };
    task_store::reject_candidate(root, candidate_id, note.as_deref(), now)
}

#[tauri::command(async)]
fn work_record_create(
    state: State<'_, SharedHost>,
    draft: worklog_records::RecordDraft,
) -> Result<WorkRecordSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let offset = worklog_offset_minutes(&state);
    work_record_create_impl(&root, &policy, draft, task_shell::now_secs(), offset)
}

#[tauri::command(async)]
fn work_record_read(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<worklog_records::WorkRecord, String> {
    let root = work_root(&state)?;
    worklog_records::read_record(&root, &id)
}

#[tauri::command(async)]
fn work_record_update(
    state: State<'_, SharedHost>,
    id: String,
    patch: worklog_records::RecordPatch,
) -> Result<WorkRecordSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    work_record_update_impl(&root, &policy, &id, patch, task_shell::now_secs())
}

#[tauri::command(async)]
fn work_record_delete(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<worklog_records::TrashMove, String> {
    let root = work_root(&state)?;
    worklog_records::delete_record(&root, &id, task_shell::now_secs())
}

#[tauri::command(async)]
fn work_record_list(
    state: State<'_, SharedHost>,
    filter: worklog_records::RecordFilter,
) -> Result<worklog_records::RecordListing, String> {
    let root = work_root(&state)?;
    worklog_records::list_records(&root, &filter)
}

/// 存量周文件迁移预览：只解析、不写盘。
#[tauri::command(async)]
fn worklog_migrate_preview(
    state: State<'_, SharedHost>,
) -> Result<worklog_migrate::MigrationPreview, String> {
    let library = worklog_library_root(&state)?;
    let work = work_root(&state)?;
    worklog_migrate::preview(&library, &work)
}

/// 执行存量周文件迁移。必须显式 confirm；成功后源文件移入 work/周文件存档/。
#[tauri::command(async)]
fn worklog_migrate_apply(
    state: State<'_, SharedHost>,
    confirm: bool,
) -> Result<worklog_migrate::MigrationReport, String> {
    if !confirm {
        return Err("迁移会移动周文件，必须显式确认后才能执行。".to_string());
    }
    let library = worklog_library_root(&state)?;
    let work = work_root(&state)?;
    worklog_migrate::migrate(&library, &work, task_shell::now_secs())
}

/// 临时层清理预览：只读，不创建目录、不移动文件（OD-WR-313）。
#[tauri::command(async)]
fn work_cleanup_preview(
    state: State<'_, SharedHost>,
) -> Result<work_cleanup::CleanupPreview, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let work = work_root(&state)?;
    let vault = configured_vault_root(&paths).ok();
    work_cleanup::cleanup_preview(&work, vault.as_deref(), task_shell::now_secs())
}

/// 临时层清理执行：必须显式 confirm；只移动到 .trash/。
#[tauri::command(async)]
fn work_cleanup_apply(
    state: State<'_, SharedHost>,
    confirm: bool,
) -> Result<work_cleanup::CleanupReport, String> {
    if !confirm {
        return Err("清理会移动记录与任务，必须显式确认后才能执行。".to_string());
    }
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let work = work_root(&state)?;
    let vault = configured_vault_root(&paths).ok();
    work_cleanup::cleanup_apply(&work, vault.as_deref(), task_shell::now_secs())
}

/// 读取工作时间配置（OD-QS-406）。没有文件时返回默认值且不写盘。
#[tauri::command(async)]
fn availability_get(
    state: State<'_, SharedHost>,
) -> Result<availability::AvailabilityConfig, String> {
    let work = work_root(&state)?;
    availability::load(&work)
}

/// 保存工作时间配置。先校验后原子替换，失败时旧文件保持不变。
#[tauri::command(async)]
fn availability_set(
    state: State<'_, SharedHost>,
    mut config: availability::AvailabilityConfig,
) -> Result<availability::AvailabilityConfig, String> {
    let work = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    for block in config.unavailable.iter_mut() {
        let (masked, _counts) = gate_user_text(&block.reason, &policy, "不可用原因")?;
        block.reason = masked;
    }
    availability::save(&work, config)
}

/// 解析准备：校验门禁、过红线与掩码、组装提示词。模型调用由界面随后提交。
fn worklog_record_analyze_impl(
    root: &std::path::Path,
    app_root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    record_id: &str,
) -> Result<task_extract::AnalyzePrompt, String> {
    let record = worklog_records::read_record(root, record_id)?;
    task_extract::assert_analysis_allowed(&record)?;
    let (title, title_counts) = gate_user_text(&record.title, policy, "记录标题")?;
    let (body, body_counts) = gate_user_text(&record.body, policy, "记录正文")?;
    let config = EngineConfig::from_env(app_root)?;
    let tasks = task_store::unfinished_tasks(root)?;
    // OD-KM-506：把既有素材 id+title 注入提示词，模型才能正确给 match_material_id。
    // 素材正文不外发；宿主随后仍按素材篮里的真实编号校验候选。
    let materials = material_store::list_materials(
        root,
        &material_store::MaterialFilter::default(),
    )?
    .items;
    let prompt = task_extract::build_prompt_with_materials(
        &config.python,
        &config.engine_dir,
        &record,
        &tasks,
        &materials,
        &title,
        &body,
    )?;
    Ok(task_extract::AnalyzePrompt {
        record_id: record_id.to_string(),
        prompt,
        user_input_text: format!("{title}\n{body}"),
        masked: title_counts.any() || body_counts.any(),
        masked_id_card: title_counts.id_card + body_counts.id_card,
        masked_phone: title_counts.phone + body_counts.phone,
    })
}

/// 解析准备：校验门禁、过红线与掩码、组装提示词。模型调用由界面随后提交。
#[tauri::command(async)]
fn worklog_record_analyze(
    state: State<'_, SharedHost>,
    record_id: String,
) -> Result<task_extract::AnalyzePrompt, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    worklog_record_analyze_impl(&root, &app_root, &policy, &record_id)
}

/// 解析落地：校验模型输出、暂存候选、记录标记为已解析；失败只改状态。
#[tauri::command(async)]
fn worklog_record_analyze_apply(
    state: State<'_, SharedHost>,
    record_id: String,
    result_text: String,
) -> Result<task_extract::AnalyzeApplied, String> {
    let root = work_root(&state)?;
    task_extract::apply_analysis(&root, &record_id, &result_text, task_shell::now_secs())
}

/// 模型调用失败或超时时把记录置为解析失败；正文不动。
#[tauri::command(async)]
fn worklog_record_analyze_failed(
    state: State<'_, SharedHost>,
    record_id: String,
) -> Result<worklog_records::WorkRecord, String> {
    let root = work_root(&state)?;
    task_extract::mark_analysis_failed(&root, &record_id, task_shell::now_secs())
}

/// OD-WT-604：写作需求解析准备。宿主过红线与掩码后组装提示词；
/// 模型调用仍由界面侧 submitTask 提交，本命令不写任何文件。
#[tauri::command(async)]
fn writing_parse_requirement(
    state: State<'_, SharedHost>,
    text: String,
) -> Result<writing_parse::WritingParsePrompt, String> {
    let policy = current_rule_policy(&state)?;
    let app_root = {
        let host = state.lock().unwrap();
        host.paths.root.clone()
    };
    let (masked_text, counts) = gate_user_text(&text, &policy, "写作需求")?;
    let config = EngineConfig::from_env(&app_root)?;
    let prompt = writing_parse::build_requirement_prompt(
        &config.python,
        &config.engine_dir,
        &masked_text,
    )?;
    Ok(writing_parse::WritingParsePrompt {
        prompt,
        user_input_text: masked_text,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

/// OD-WT-604：解析落地。校验模型输出并返回草稿；本命令不落盘，
/// 用户确认后才经 writing_create_from_requirement 创建任务。
#[tauri::command(async)]
fn writing_parse_requirement_apply(
    result_text: String,
) -> Result<writing_parse::WritingRequirementApplied, String> {
    writing_parse::validate_requirement_payload(&result_text)
}

/// 用户确认草稿后的创建路径：所有字段再过一次红线与掩码，
/// 再交给 writing_store 落盘，初始状态固定为「需求待整理」。
fn writing_create_from_requirement_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    mut draft: writing_parse::WritingRequirementDraft,
    now: f64,
) -> Result<writing_parse::WritingTaskCreated, String> {
    let mut counts = profile_rules::MaskCounts::default();
    for (value, target) in [
        (&mut draft.title, "写作标题"),
        (&mut draft.topic, "写作主题"),
        (&mut draft.writing_purpose, "写作目的"),
        (&mut draft.audience, "目标受众"),
        (&mut draft.writing_period, "写作周期"),
        (&mut draft.expected_length, "预计篇幅"),
        (&mut draft.writing_style, "写作风格"),
    ] {
        if let Some(text) = value.as_mut() {
            let (masked, extra) = gate_user_text(text, policy, target)?;
            *text = masked;
            add_counts(&mut counts, extra);
        }
    }
    for item in draft.requirements.iter_mut() {
        let (masked, extra) = gate_user_text(item, policy, "写作要求")?;
        *item = masked;
        add_counts(&mut counts, extra);
    }
    let task = writing_parse::create_writing_task_from_requirement(root, &draft, now)?;
    Ok(writing_parse::WritingTaskCreated {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

/// OD-WT-604：用户确认后创建写作任务（调 601 writing_store create）。
#[tauri::command(async)]
fn writing_create_from_requirement(
    state: State<'_, SharedHost>,
    draft: writing_parse::WritingRequirementDraft,
) -> Result<writing_parse::WritingTaskCreated, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_create_from_requirement_impl(&root, &policy, draft, task_shell::now_secs())
}

/// OD-WT-610：写作任务列表（只读）。
///
/// 604-609 提供了解析、素材、大纲与章节命令，但没有把 601 的
/// list_writing_tasks 暴露给界面；工作台需要按状态/类型分组展示任务，
/// 因此补一条只读命令。过滤条件不落盘，排序与白名单校验沿用 601。
#[tauri::command(async)]
fn writing_list(
    state: State<'_, SharedHost>,
    filter: Option<writing_store::WritingTaskFilter>,
) -> Result<writing_store::WritingTaskListing, String> {
    let root = work_root(&state)?;
    writing_store::list_writing_tasks(&root, &filter.unwrap_or_default())
}

/// OD-WT-613：用户触发的写作状态迁移。
///
/// 状态迁移只接受 601 的合法表；从「待审核」进入「已定稿」时先写
/// label=final 的正文快照，再推进状态。AI 与本命令无调用通道。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WritingStatusTransitionRequest {
    writing_id: String,
    to: String,
    #[serde(default)]
    reason: Option<String>,
}

fn writing_status_transition_impl(
    root: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    request: WritingStatusTransitionRequest,
    now: f64,
) -> Result<writing_store::WritingTask, String> {
    let (writing_id, counts) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    let _ = counts;
    let reason = match request.reason {
        Some(value) if !value.trim().is_empty() => {
            let (masked, _) = gate_user_text(&value, policy, "状态迁移理由")?;
            Some(masked)
        }
        _ => None,
    };
    let to = request.to.trim().to_string();
    let task = writing_store::read_writing_task(root, &writing_id)?;
    let outline_confirmed = if task.status == writing_store::STATUS_OUTLINE_PENDING
        && to == writing_store::STATUS_WRITING
    {
        writing_outline::read_confirmed_outline(root, &writing_id).is_ok()
    } else {
        false
    };
    if to == writing_store::STATUS_FINALIZED
        && task.status == writing_store::STATUS_PENDING_REVIEW
        && writing_store::is_allowed_transition(&task.status, &to, outline_confirmed)
    {
        export_engine::finalize_snapshot(root, &writing_id, now)?;
    }
    writing_store::transition_writing_status(
        root,
        &writing_id,
        &to,
        outline_confirmed,
        reason.as_deref(),
        now,
    )
}

/// OD-WT-613：用户触发任务状态迁移（600 迁移表门禁）。
#[tauri::command(async)]
fn writing_status_transition(
    state: State<'_, SharedHost>,
    request: WritingStatusTransitionRequest,
) -> Result<writing_store::WritingTask, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    writing_status_transition_impl(&root, &policy, request, task_shell::now_secs())
}

/// OD-WT-613：citations / factcheck 只读查询。
#[tauri::command(async)]
fn writing_records_query(
    state: State<'_, SharedHost>,
    request: export_engine::RecordsQueryRequest,
) -> Result<export_engine::RecordsQueryResult, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let (writing_id, _) = gate_user_text(&request.writing_id, &policy, "写作任务编号")?;
    let mut request = request;
    request.writing_id = writing_id;
    export_engine::query_records(&root, &request)
}

/// OD-WT-613：文稿导出。MD 三态、公文 DOCX、阅读 PDF、公众号 HTML。
///
/// 导出前先过当前 Profile 的敏感词扫描；导出只写用户指定目录或默认
/// 导出目录，随后经 602 追加 manifest 回执，不触达网络。
#[tauri::command(async)]
fn writing_export(
    state: State<'_, SharedHost>,
    request: export_engine::ExportRequest,
) -> Result<export_engine::ExportOutcome, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let (writing_id, _) = gate_user_text(&request.writing_id, &policy, "写作任务编号")?;
    let (app_root, config_dir) = {
        let host = state.lock().unwrap();
        (host.paths.root.clone(), host.paths.config.clone())
    };
    let mut request = request;
    request.writing_id = writing_id;
    let tools = export_engine::ExportTools::discover();
    export_engine::export_writing(
        &root,
        &app_root,
        &config_dir,
        &request,
        &policy,
        &tools,
        task_shell::now_secs(),
    )
}

/// OD-WT-614：归档预览/执行入参。preview=true 只读清单，不写知识库。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WritingArchiveRequest {
    writing_id: String,
    #[serde(default)]
    preview: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WritingFactCheckCount {
    pub conclusion: String,
    pub count: u64,
}

/// 知识回流候选：用户勾选后才提交，未勾选内容不进任何治理通道。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritingKnowledgeSelection {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub source_quote: String,
}

/// 归档记录关联：写作任务、素材、知识、核验统计、版本与导出清单。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WritingArchiveAssociation {
    pub writing_id: String,
    pub title: String,
    pub status: String,
    pub version: u64,
    pub selected_material_ids: Vec<String>,
    pub selected_knowledge_ids: Vec<String>,
    pub factcheck_counts: Vec<WritingFactCheckCount>,
    pub export_receipts: Vec<document_store::ExportReceipt>,
    pub already_archived: bool,
    pub archive_path: Option<String>,
    pub knowledge_candidates: Vec<WritingKnowledgeSelection>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WritingArchiveOutcome {
    pub preview: bool,
    pub can_archive: bool,
    pub blocked_reason: Option<String>,
    pub association: WritingArchiveAssociation,
    pub vault: Option<vault_archive::ArchiveReceipt>,
    pub manifest_receipt: Option<document_store::ExportReceipt>,
    pub status_after: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WritingKnowledgeFeedbackRequest {
    writing_id: String,
    selections: Vec<WritingKnowledgeSelection>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WritingKnowledgeFeedbackOutcome {
    pub writing_id: String,
    pub ok: bool,
    pub selected_count: usize,
    pub admitted: Vec<knowledge_ingest::AdmittedRawFile>,
    pub rejected: Vec<knowledge_ingest::IngestRejection>,
    pub index_updated: bool,
    pub index_path: String,
    pub errors: Vec<String>,
    pub note: String,
}

fn writing_archive_factcheck_counts(
    root: &std::path::Path,
    writing_id: &str,
) -> Result<Vec<WritingFactCheckCount>, String> {
    let records = writing_records::list_fact_checks(root, writing_id)?;
    Ok(writing_records::CHECK_CONCLUSIONS
        .iter()
        .map(|conclusion| WritingFactCheckCount {
            conclusion: conclusion.as_str().to_string(),
            count: records
                .iter()
                .filter(|record| record.conclusion == *conclusion)
                .count() as u64,
        })
        .collect())
}

fn writing_archive_candidates(
    root: &std::path::Path,
    writing_id: &str,
    outline: &writing_outline::WritingOutline,
) -> Vec<WritingKnowledgeSelection> {
    let mut candidates = Vec::new();
    for section in &outline.sections {
        let Ok(state) = writing_section::read_section_state(root, writing_id, section.no) else {
            continue;
        };
        let Some(content) = state.content else {
            continue;
        };
        let content = export_engine::strip_internal(&content);
        if content.trim().is_empty() {
            continue;
        }
        let quote: String = content
            .split("\n\n")
            .next()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(160)
            .collect();
        candidates.push(WritingKnowledgeSelection {
            title: state.heading,
            content,
            source_quote: quote,
        });
    }
    candidates
}

fn writing_archive_association(
    root: &std::path::Path,
    task: &writing_store::WritingTask,
) -> Result<WritingArchiveAssociation, String> {
    // 预览允许未确认大纲的任务查看归档入口；执行时 status 门禁与正文读取仍会拒绝。
    let confirmed = writing_outline::read_confirmed_outline(root, &task.writing_id).ok();
    let manifest = document_store::read_manifest(root, &task.writing_id)?;
    let version = manifest
        .versions
        .iter()
        .filter(|entry| entry.note.contains("定稿快照"))
        .map(|entry| entry.version)
        .max()
        .unwrap_or(manifest.current_version);
    let exports: Vec<document_store::ExportReceipt> = manifest
        .export_receipts
        .iter()
        .filter(|receipt| receipt.format != "archive")
        .cloned()
        .collect();
    let archive_path = manifest
        .export_receipts
        .iter()
        .rev()
        .find(|receipt| receipt.format == "archive")
        .map(|receipt| receipt.path.clone());
    let title = confirmed
        .as_ref()
        .map(|outline| outline.outline.title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| task.title.clone());
    let knowledge_candidates = confirmed
        .as_ref()
        .map(|outline| writing_archive_candidates(root, &task.writing_id, &outline.outline))
        .unwrap_or_default();
    Ok(WritingArchiveAssociation {
        writing_id: task.writing_id.clone(),
        title,
        status: task.status.clone(),
        version,
        selected_material_ids: task.selected_material_ids.clone(),
        selected_knowledge_ids: task.selected_knowledge_ids.clone(),
        factcheck_counts: writing_archive_factcheck_counts(root, &task.writing_id)?,
        export_receipts: exports,
        already_archived: archive_path.is_some(),
        archive_path,
        knowledge_candidates,
    })
}

fn writing_archive_impl(
    root: &std::path::Path,
    vault: Option<&std::path::Path>,
    policy: &profile_rules::RulePolicy,
    request: WritingArchiveRequest,
    now: f64,
) -> Result<WritingArchiveOutcome, String> {
    let (writing_id, _) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    let task = writing_store::read_writing_task(root, &writing_id)?;
    let association = writing_archive_association(root, &task)?;
    if request.preview {
        let blocked_reason = if association.already_archived {
            Some("该写作任务已有归档回执；归档为终态，不重复写入。".to_string())
        } else if task.status != writing_store::STATUS_FINALIZED {
            Some(format!(
                "只有「{}」的写作任务可以归档；当前状态为「{}」。",
                writing_store::STATUS_FINALIZED,
                task.status
            ))
        } else {
            None
        };
        return Ok(WritingArchiveOutcome {
            preview: true,
            can_archive: blocked_reason.is_none(),
            blocked_reason,
            association,
            vault: None,
            manifest_receipt: None,
            status_after: task.status,
        });
    }
    if task.status == writing_store::STATUS_ARCHIVED {
        return Err("写作任务已归档：归档为终态，不重复写入知识库。".to_string());
    }
    if task.status != writing_store::STATUS_FINALIZED {
        return Err(format!(
            "只有「{}」的写作任务可以归档；当前状态为「{}」。",
            writing_store::STATUS_FINALIZED,
            task.status
        ));
    }
    if !writing_store::is_allowed_transition(
        &task.status,
        writing_store::STATUS_ARCHIVED,
        false,
    ) {
        return Err(format!(
            "601 迁移表不允许 {} → {}。",
            task.status,
            writing_store::STATUS_ARCHIVED
        ));
    }
    let vault = vault.ok_or_else(|| "还没有选择知识库目录；归档前请先在设置里选择知识库。".to_string())?;
    let raw = writing_section::compose_full_text_for(root, &writing_id)?;
    let content = export_engine::strip_internal(&raw);
    if content.trim().is_empty() {
        return Err("文稿正文为空，拒绝归档。".to_string());
    }
    let title = {
        let clean = export_engine::strip_internal(&association.title);
        if clean.trim().is_empty() {
            export_engine::strip_internal(&task.title)
        } else {
            clean
        }
    };
    let hits = task_shell::red_line_scan_with(&format!("{title}\n{content}"), policy);
    if !hits.is_empty() {
        let total: u64 = hits.iter().map(|hit| hit.count).sum();
        return Err(format!(
            "归档前敏感词扫描命中 {total} 处，已拒绝归档（不回显命中原文）。"
        ));
    }
    let meta = vault_archive::WritingArchiveMeta {
        version: association.version,
        selected_material_ids: association.selected_material_ids.clone(),
        selected_knowledge_ids: association.selected_knowledge_ids.clone(),
        factcheck_counts: association
            .factcheck_counts
            .iter()
            .map(|count| (count.conclusion.clone(), count.count))
            .collect(),
        export_receipts: association.export_receipts.len(),
    };
    let vault_receipt =
        vault_archive::archive_writing_final(vault, &writing_id, &title, &content, &meta, now)?;
    let (_manifest, manifest_receipt) = document_store::record_export_receipt(
        root,
        &writing_id,
        "archive",
        &vault_receipt.target_path,
        now,
    )
    .map_err(|error| {
        format!(
            "归档文件已写入 {}，但 manifest 归档回执写入失败：{error}",
            vault_receipt.target_path
        )
    })?;
    let task_after = writing_store::transition_writing_status(
        root,
        &writing_id,
        writing_store::STATUS_ARCHIVED,
        false,
        Some("用户归档"),
        now,
    )?;
    Ok(WritingArchiveOutcome {
        preview: false,
        can_archive: false,
        blocked_reason: None,
        association,
        vault: Some(vault_receipt),
        manifest_receipt: Some(manifest_receipt),
        status_after: task_after.status,
    })
}

fn knowledge_feedback_file_name(title: &str, now: f64, index: usize) -> String {
    let mut cleaned: String = title
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    while cleaned.contains("..") {
        cleaned = cleaned.replace("..", ".");
    }
    cleaned = cleaned.trim().trim_start_matches('.').trim().to_string();
    if cleaned.is_empty() {
        cleaned = "未命名".to_string();
    }
    let cleaned: String = cleaned.chars().take(80).collect();
    let date: String = task_shell::utc_iso8601(now).chars().take(10).collect();
    format!("文稿回流-{cleaned}-{date}-{index}.md")
}

fn writing_knowledge_feedback_impl(
    root: &std::path::Path,
    vault: &std::path::Path,
    policy: &profile_rules::RulePolicy,
    request: WritingKnowledgeFeedbackRequest,
    now: f64,
    utc_offset_minutes: i32,
) -> Result<WritingKnowledgeFeedbackOutcome, String> {
    let (writing_id, _) = gate_user_text(&request.writing_id, policy, "写作任务编号")?;
    let task = writing_store::read_writing_task(root, &writing_id)?;
    if !matches!(
        task.status.as_str(),
        writing_store::STATUS_FINALIZED | writing_store::STATUS_ARCHIVED
    ) {
        return Err(format!(
            "只有「{}」或「{}」的文稿可以回流知识；当前状态为「{}」。",
            writing_store::STATUS_FINALIZED,
            writing_store::STATUS_ARCHIVED,
            task.status
        ));
    }
    if request.selections.is_empty() {
        return Err("没有勾选的可复用内容；未提交任何治理通道。".to_string());
    }
    let mut files = Vec::new();
    for (index, selection) in request.selections.iter().enumerate() {
        if selection.title.trim().is_empty() {
            return Err(format!("第 {} 条知识回流缺少标题。", index + 1));
        }
        let (title, _) = gate_user_text(&selection.title, policy, "知识回流标题")?;
        let (content, _) = gate_user_text(&selection.content, policy, "知识回流内容")?;
        if content.trim().is_empty() {
            return Err(format!("第 {} 条知识回流内容为空。", index + 1));
        }
        let source_quote = if selection.source_quote.trim().is_empty() {
            String::new()
        } else {
            gate_user_text(&selection.source_quote, policy, "知识回流来源原文")?.0
        };
        let note = format!(
            "---\nschema: \"omnidesk.writing-knowledge-feedback.v1\"\ntitle: {}\nsource_writing_id: {}\nsource_quote: {}\n---\n\n{content}\n",
            worklog_records::yaml_scalar(title.trim()),
            worklog_records::yaml_scalar(&writing_id),
            worklog_records::yaml_scalar(source_quote.trim()),
        );
        files.push(knowledge_ingest::RawFileInput {
            name: knowledge_feedback_file_name(title.trim(), now, index + 1),
            bytes: note.into_bytes(),
        });
    }
    knowledge_ingest::init_vault(vault)?;
    let report = knowledge_ingest::admit_raw_files_with_policy(
        vault,
        &files,
        policy,
        utc_offset_minutes,
    );
    if report.admitted.is_empty() {
        let detail = report
            .errors
            .iter()
            .chain(report.rejected.iter().map(|item| &item.reason))
            .cloned()
            .collect::<Vec<_>>()
            .join("；");
        return Err(format!(
            "知识回流未进入治理队列：{}",
            if detail.is_empty() {
                "raw 准入没有接收任何文件".to_string()
            } else {
                detail
            }
        ));
    }
    Ok(WritingKnowledgeFeedbackOutcome {
        writing_id,
        ok: report.ok,
        selected_count: request.selections.len(),
        admitted: report.admitted,
        rejected: report.rejected,
        index_updated: report.index_updated,
        index_path: report.index_path,
        errors: report.errors,
        note: "已入 raw 收件待审队列；不自动生成知识条目，仍须既有审核裁决。".to_string(),
    })
}

/// OD-WT-614：用户触发归档；preview=true 只读归档清单，AI 无调用通道。
#[tauri::command(async)]
fn writing_archive(
    state: State<'_, SharedHost>,
    request: WritingArchiveRequest,
) -> Result<WritingArchiveOutcome, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let vault = if request.preview {
        None
    } else {
        let paths = {
            let host = state.lock().unwrap();
            host.paths.clone()
        };
        Some(configured_vault_root(&paths)?)
    };
    writing_archive_impl(
        &root,
        vault.as_deref(),
        &policy,
        request,
        task_shell::now_secs(),
    )
}

/// OD-WT-614：用户勾选的可复用内容进入既有 raw 收件治理队列；不自动定稿。
#[tauri::command(async)]
fn writing_knowledge_feedback(
    state: State<'_, SharedHost>,
    request: WritingKnowledgeFeedbackRequest,
) -> Result<WritingKnowledgeFeedbackOutcome, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let vault = configured_vault_root(&paths)?;
    let offset = worklog_offset_minutes(&state);
    writing_knowledge_feedback_impl(
        &root,
        &vault,
        &policy,
        request,
        task_shell::now_secs(),
        offset,
    )
}

#[tauri::command(async)]
fn task_create(
    state: State<'_, SharedHost>,
    draft: task_store::TaskDraft,
) -> Result<TaskSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    task_create_impl(&root, &policy, draft, task_shell::now_secs())
}

#[tauri::command(async)]
fn task_read(state: State<'_, SharedHost>, id: String) -> Result<task_store::Task, String> {
    let root = work_root(&state)?;
    task_store::read_task(&root, &id)
}

#[tauri::command(async)]
fn task_update(
    state: State<'_, SharedHost>,
    id: String,
    patch: task_store::TaskPatch,
) -> Result<TaskSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    task_update_impl(&root, &policy, &id, patch, task_shell::now_secs())
}

#[tauri::command(async)]
fn task_transition(
    state: State<'_, SharedHost>,
    id: String,
    to_state: String,
    note: Option<String>,
) -> Result<TaskSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    task_transition_impl(
        &root,
        &policy,
        &id,
        &to_state,
        note,
        task_shell::now_secs(),
    )
}

#[tauri::command(async)]
fn task_append_progress(
    state: State<'_, SharedHost>,
    id: String,
    text: String,
) -> Result<TaskSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    task_append_progress_impl(&root, &policy, &id, &text, task_shell::now_secs())
}

/// 用户调整重要性（OD-QS-405）：source 固定为 user，理由必填并过统一闸门。
#[tauri::command(async)]
fn task_set_importance(
    state: State<'_, SharedHost>,
    id: String,
    importance: String,
    reason: String,
) -> Result<TaskSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let mut counts = profile_rules::MaskCounts::default();
    let (reason, extra) = gate_user_text(&reason, &policy, "重要性理由")?;
    add_counts(&mut counts, extra);
    let task = task_store::set_importance(
        &root,
        &id,
        &importance,
        task_store::IMPORTANCE_SOURCE_USER,
        Some(&reason),
        task_shell::now_secs(),
    )?;
    Ok(TaskSaved {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

/// 用户指定或清除象限。quadrant 为 None 表示回到规则派生。
#[tauri::command(async)]
fn task_set_quadrant(
    state: State<'_, SharedHost>,
    id: String,
    quadrant: Option<String>,
    reason: Option<String>,
) -> Result<TaskSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let mut counts = profile_rules::MaskCounts::default();
    let reason = match reason {
        Some(text) => {
            let (masked, extra) = gate_user_text(&text, &policy, "象限调整理由")?;
            add_counts(&mut counts, extra);
            Some(masked)
        }
        None => None,
    };
    let task = task_store::set_quadrant_override(
        &root,
        &id,
        quadrant.as_deref(),
        reason.as_deref(),
        task_shell::now_secs(),
    )?;
    Ok(TaskSaved {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

/// 用户调整截止日期与性质。理由必填。
#[tauri::command(async)]
fn task_set_due(
    state: State<'_, SharedHost>,
    id: String,
    due_date: Option<String>,
    due_kind: String,
    reason: String,
) -> Result<TaskSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let mut counts = profile_rules::MaskCounts::default();
    let (reason, extra) = gate_user_text(&reason, &policy, "截止调整理由")?;
    add_counts(&mut counts, extra);
    let task = task_store::set_due(
        &root,
        &id,
        due_date.as_deref(),
        &due_kind,
        &reason,
        task_shell::now_secs(),
    )?;
    Ok(TaskSaved {
        task,
        masked: counts.any(),
        masked_id_card: counts.id_card,
        masked_phone: counts.phone,
    })
}

/// 用户补充预计耗时；None 表示清除。无自由文本入参。
#[tauri::command(async)]
fn task_set_estimated(
    state: State<'_, SharedHost>,
    id: String,
    minutes: Option<u32>,
) -> Result<task_store::Task, String> {
    let root = work_root(&state)?;
    task_store::set_estimated_minutes(&root, &id, minutes, task_shell::now_secs())
}

/// 用户设置前置任务；存在性、无环与不自依赖由存储层校验。
#[tauri::command(async)]
fn task_set_dependency(
    state: State<'_, SharedHost>,
    id: String,
    depends_on: Vec<String>,
) -> Result<task_store::Task, String> {
    let root = work_root(&state)?;
    task_store::set_depends_on(&root, &id, &depends_on, task_shell::now_secs())
}

/// 任务四象限视图（OD-QS-410）：只读派生，不落盘。
#[tauri::command(async)]
fn task_quadrant_views(
    state: State<'_, SharedHost>,
    utc_offset_minutes: Option<i32>,
) -> Result<Vec<priority_rules::QuadrantView>, String> {
    let root = work_root(&state)?;
    priority_rules::list_quadrant_views_at(
        &root,
        task_shell::now_secs(),
        utc_offset_minutes.unwrap_or(0),
    )
}

#[derive(Debug, serde::Serialize)]
struct ScheduleGenerateReport {
    created: Vec<schedule_store::ScheduleEntry>,
    expired: Vec<String>,
    conflicts: Vec<scheduling::SchedulingConflict>,
    unscheduled: Vec<scheduling::UnscheduledTask>,
    stale: bool,
    warnings: Vec<String>,
    generated_at: Option<String>,
}

fn schedule_generate_impl(
    root: &std::path::Path,
    now: f64,
    utc_offset_minutes: i32,
) -> Result<ScheduleGenerateReport, String> {
    let tasks = task_store::list_tasks(root, &task_store::TaskFilter::default())?.items;
    let views = priority_rules::list_quadrant_views_at(root, now, utc_offset_minutes)?;
    let availability = availability::load(root)?;
    let expired = schedule_store::expire_stale_suggestions(root, &tasks, now)?;
    let entries = schedule_store::list_entries(root)?;
    let mut occupied: Vec<schedule_store::ScheduleEntry> = entries
        .into_iter()
        .filter(|entry| {
            entry.is_locked
                || matches!(
                    entry.status.as_str(),
                    schedule_store::STATUS_SUGGESTED | schedule_store::STATUS_CONFIRMED
                )
        })
        .collect();
    let mut existing_tasks = std::collections::HashSet::new();
    for entry in &occupied {
        if matches!(
            entry.status.as_str(),
            schedule_store::STATUS_SUGGESTED | schedule_store::STATUS_CONFIRMED
        ) {
            existing_tasks.insert(entry.task_id.clone());
        }
    }
    // 408 只把已确认/锁定当固定层；增量生成时把既有建议也视为已占位，避免新建议重叠。
    for entry in &mut occupied {
        if entry.status == schedule_store::STATUS_SUGGESTED {
            entry.is_locked = true;
        }
    }
    let generated = scheduling::generate_suggestions_at(
        &tasks,
        &views,
        &availability,
        &occupied,
        now,
        utc_offset_minutes,
    );
    let mut created = Vec::new();
    let mut warnings = Vec::new();
    for suggestion in &generated.suggestions {
        if existing_tasks.contains(&suggestion.task_id) {
            continue;
        }
        let draft = schedule_store::ScheduleDraft {
            id: None,
            task_id: suggestion.task_id.clone(),
            start_at: suggestion.start_at.clone(),
            end_at: suggestion.end_at.clone(),
            duration: Some(suggestion.duration),
            is_locked: Some(false),
        };
        match schedule_store::create_entry(root, &draft, now) {
            Ok(entry) => created.push(entry),
            Err(error) => warnings.push(format!("{}：{error}", suggestion.task_id)),
        }
    }
    let generated_at = schedule_store::mark_generated_at(root, now)?;
    let stale = schedule_store::is_stale(root, &tasks)?;
    Ok(ScheduleGenerateReport {
        created,
        expired,
        conflicts: generated.conflicts,
        unscheduled: generated.unscheduled,
        stale,
        warnings,
        generated_at: Some(generated_at),
    })
}

/// 重新生成建议：不动已确认/锁定，过期已结束任务的旧建议，只补新建议。
#[tauri::command(async)]
fn schedule_generate(
    state: State<'_, SharedHost>,
    utc_offset_minutes: Option<i32>,
) -> Result<ScheduleGenerateReport, String> {
    let root = work_root(&state)?;
    schedule_generate_impl(&root, task_shell::now_secs(), utc_offset_minutes.unwrap_or(0))
}

#[tauri::command(async)]
fn schedule_confirm(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<schedule_store::ScheduleEntry, String> {
    let root = work_root(&state)?;
    schedule_store::confirm_entry(&root, &id, task_shell::now_secs())
}

#[tauri::command(async)]
fn schedule_adjust(
    state: State<'_, SharedHost>,
    id: String,
    start_at: String,
    end_at: String,
    reason: String,
) -> Result<schedule_store::AdjustmentReport, String> {
    let root = work_root(&state)?;
    schedule_store::adjust_entry(
        &root,
        &id,
        &start_at,
        &end_at,
        &reason,
        task_shell::now_secs(),
    )
}

#[tauri::command(async)]
fn schedule_cancel(
    state: State<'_, SharedHost>,
    id: String,
    reason: Option<String>,
) -> Result<schedule_store::ScheduleEntry, String> {
    let root = work_root(&state)?;
    schedule_store::cancel_entry(&root, &id, reason.as_deref(), task_shell::now_secs())
}

#[tauri::command(async)]
fn schedule_lock(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<schedule_store::ScheduleEntry, String> {
    let root = work_root(&state)?;
    schedule_store::lock_entry(&root, &id, task_shell::now_secs())
}

#[tauri::command(async)]
fn schedule_list(
    state: State<'_, SharedHost>,
) -> Result<schedule_store::ScheduleListing, String> {
    let root = work_root(&state)?;
    schedule_store::list_entry_views(&root, task_shell::now_secs())
}

#[tauri::command(async)]
fn task_delete(state: State<'_, SharedHost>, id: String) -> Result<TaskDeleted, String> {
    let root = work_root(&state)?;
    let path = task_store::delete_task(&root, &id, task_shell::now_secs())?;
    Ok(TaskDeleted { id, path })
}

#[tauri::command(async)]
fn task_list(
    state: State<'_, SharedHost>,
    filter: task_store::TaskFilter,
) -> Result<task_store::TaskListing, String> {
    let root = work_root(&state)?;
    task_store::list_tasks(&root, &filter)
}

#[tauri::command(async)]
fn task_candidate_list(
    state: State<'_, SharedHost>,
) -> Result<task_store::CandidateListing, String> {
    let root = work_root(&state)?;
    task_store::list_candidates(&root)
}

#[tauri::command(async)]
fn task_candidate_confirm(
    state: State<'_, SharedHost>,
    candidate_id: String,
    edits: task_store::ConfirmEdits,
) -> Result<TaskSaved, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    task_candidate_confirm_impl(
        &root,
        &policy,
        &candidate_id,
        edits,
        task_shell::now_secs(),
    )
}

#[tauri::command(async)]
fn task_candidate_reject(
    state: State<'_, SharedHost>,
    candidate_id: String,
    note: Option<String>,
) -> Result<task_store::TaskCandidate, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    task_candidate_reject_impl(&root, &policy, &candidate_id, note, task_shell::now_secs())
}

/// 采纳关联建议：追加进展并双向回写；进展文本同样过红线与掩码。
#[tauri::command(async)]
fn task_candidate_link(
    state: State<'_, SharedHost>,
    candidate_id: String,
    progress_text: Option<String>,
    note: Option<String>,
) -> Result<task_store::LinkReport, String> {
    let root = work_root(&state)?;
    let policy = current_rule_policy(&state)?;
    let progress = match progress_text {
        Some(raw) => Some(gate_user_text(&raw, &policy, "任务进展")?.0),
        None => None,
    };
    let note = match note {
        Some(raw) => Some(gate_user_text(&raw, &policy, "关联备注")?.0),
        None => None,
    };
    task_store::link_candidate(
        &root,
        &candidate_id,
        progress.as_deref(),
        note.as_deref(),
        task_shell::now_secs(),
    )
}

/// 拒绝关联建议：清空匹配字段，候选继续留在待审核。
#[tauri::command(async)]
fn task_candidate_dismiss_match(
    state: State<'_, SharedHost>,
    candidate_id: String,
) -> Result<task_store::TaskCandidate, String> {
    let root = work_root(&state)?;
    task_store::dismiss_match(&root, &candidate_id, task_shell::now_secs())
}

/// 把一份报告复制进知识库「报告/」（OD-WR-312）。同名同来源幂等，不同来源拒绝。
#[tauri::command(async)]
fn archive_report_to_vault(
    state: State<'_, SharedHost>,
    file_name: String,
    period: Option<String>,
) -> Result<vault_archive::ArchiveReceipt, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let vault = configured_vault_root(&paths)?;
    let library = settings::effective_library_root(&paths.config)?.root;
    vault_archive::archive_report(
        &vault,
        &library,
        &file_name,
        period.as_deref(),
        task_shell::now_secs(),
    )
}

/// 把一条已发布成果复制进知识库「归档成果/」；草稿与未发布成文拒绝。
#[tauri::command(async)]
fn archive_artifact_to_vault(
    state: State<'_, SharedHost>,
    relative_path: String,
) -> Result<vault_archive::ArchiveReceipt, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let vault = configured_vault_root(&paths)?;
    vault_archive::archive_artifact(&vault, &relative_path, task_shell::now_secs())
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

/// 工作记录根信息（OD-WR-315）：当前生效路径、来源与已保存选择。
#[tauri::command(async)]
fn work_root_info(state: State<'_, SharedHost>) -> serde_json::Value {
    let config_dir = {
        let host = state.lock().unwrap();
        host.paths.config.clone()
    };
    let settings_path = settings::settings_path(&config_dir).display().to_string();
    let default_root = platform::home_dir().map(|home| {
        home.join(settings::DEFAULT_WORK_SUBDIR)
            .display()
            .to_string()
    });
    match settings::effective_working_root(&config_dir) {
        Ok(eff) => serde_json::json!({
            "resolved": true,
            "working_root": eff.root.display().to_string(),
            "source": eff.source,
            "configured_root": eff.configured_root.as_ref().map(|p| p.display().to_string()),
            "env_override": eff.env_override.as_ref().map(|p| p.display().to_string()),
            "default_root": default_root,
            "settings_path": settings_path,
            "error": null,
        }),
        Err(message) => serde_json::json!({
            "resolved": false,
            "working_root": null,
            "source": null,
            "configured_root": null,
            "env_override": null,
            "default_root": default_root,
            "settings_path": settings_path,
            "error": message,
        }),
    }
}

/// 保存工作记录根（OD-WR-315）。只影响之后新建的记录与任务。
#[tauri::command(async)]
fn task_set_working_root(
    state: State<'_, SharedHost>,
    path: String,
) -> Result<serde_json::Value, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    let root = settings::set_working_root(&paths, &path)?;
    Ok(serde_json::json!({
        "ok": true,
        "working_root": root.display().to_string(),
        "note": "仅影响之后新建的工作记录与任务；既有文件不会移动、覆盖或删除。",
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
    platform::open_target(&eff.root).map_err(|e| format!("无法打开目录（{}）", e))?;
    Ok(serde_json::json!({ "ok": true, "opened": eff.root.display().to_string() }))
}

/// 在系统默认浏览器里打开正文中的 http(s) 链接（OD-ARCH-081）。
///
/// 只放行 `http` / `https`，其余 scheme 一律拒绝，避免由模型输出诱发本地协议跳转。
#[tauri::command(async)]
fn open_external_url(url: String) -> Result<serde_json::Value, String> {
    let opened = external_open::open_in_browser(&url)?;
    Ok(serde_json::json!({ "ok": true, "opened": opened }))
}

/// 运行时依赖体检（OD-ARCH-134）：只读探测 node / python 是否就绪。
///
/// 启动页据此决定是否显示「环境检查」页。缺依赖时把安装入口与修复方法直接摆出来，
/// 而不是让引擎在后台静默失败、用户只看到黑框一闪。
#[tauri::command(async)]
fn environment_check() -> Result<serde_json::Value, String> {
    let report = crate::platform::environment_report();
    serde_json::to_value(report).map_err(|e| format!("序列化环境体检结果失败：{}", e))
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
    // OD-ARCH-107：把「这个 Profile 预注入哪些 Skill」一并交给界面。
    // 复用 profile_rules_load 的同一条读取路径（含迁移与正文回填），
    // 不在 capabilities 模块里再解析一次 rules.json。
    capabilities::load_with_scope(&dsh_home, &profile, &skill_scope(&state))
}

/// 从当前 Profile 的规则资产汇总出 Skill 作用域（只读）。
///
/// 与 profile_rules_load 保持同一口径：先跑一次幂等迁移，再读规则文件。
/// 规则没加载（标准模式 / 文件不存在）时不伪造绑定——那就是「零预注入」。
fn skill_scope(state: &State<'_, SharedHost>) -> capabilities::SkillScope {
    let with_contents = profile_rules_view(state);
    let standard = profile_asset_scope(state).standard;
    let mut scope = capabilities::SkillScope {
        standard,
        rules_loaded: with_contents.loaded,
        rules_error: with_contents.error.clone(),
        rules_profile: with_contents.rules.as_ref().map(|r| r.profile.clone()),
        resolved: with_contents.skill_contents.keys().cloned().collect(),
        scenes: Vec::new(),
    };
    if let Some(rules) = &with_contents.rules {
        for (scene, binding) in &rules.skill_bindings {
            scope.scenes.push(capabilities::SceneScope {
                scene: scene.clone(),
                max_injected: binding.max_injected,
                skills: binding.skills.clone(),
            });
        }
    }
    scope
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
    platform::open_target(&dir).map_err(|e| format!("无法打开 Skill 目录（{}）", e))?;
    Ok(serde_json::json!({
        "ok": true,
        "opened": dir.display().to_string(),
        "note": "Skill 在这里只读展示；改动目录里的文件后重启应用生效。",
    }))
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
    // 与启动路径同口径：模型下拉可能比引擎更早被打开（OD-ARCH-137）。
    config.seed_profile_if_missing()?;
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
    // 首次选模型时 dsh-home 可能还没初始化，先按包内 seed 补齐（OD-ARCH-137）。
    config.seed_profile_if_missing()?;
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
    let view = profile_rules::load_rules(&profile_asset_dir(state));
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

/// 单元测试共用的进程环境互斥量（OD-ARCH-133）。
///
/// `cargo test` 的用例跑在同一进程里，环境变量是全局状态。凡是写 `OMNIDESK_*` / `HOME`
/// 的用例都必须经过这里，否则并行时分不清自己读到的是自己的临时根还是别人的。
/// 注意不可重入：已持锁的用例不要再调用会取锁的辅助函数。
#[cfg(test)]
pub(crate) mod test_env_lock {
    use std::sync::Mutex;

    pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
mod command_tests {
    use super::*;

    /// OD-ARCH-137 回归：新机器首次启动、还没起引擎就打开「设置模型」时，
    /// DSH_HOME 必须已经有包内 seed 的 Profile，否则 DSH 转储会报
    /// `profile "omnidesk-base" does not exist`。
    ///
    /// 这里直接跑生产启动入口 `build_state()`，验证它会在装配资产之前完成播种。
    #[test]
    fn build_state_seeds_bundled_home_before_model_settings() {
        let _guard = crate::test_env_lock::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let base = std::env::temp_dir().join(format!(
            "omnidesk-startup-seed-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let app_root = base.join("app");
        let dsh_home = base.join("runtime/dsh-home");
        let agents_home = base.join("runtime/agents-home");
        let bundle = base.join("bundle");
        let engine_dir = bundle.join("engine");
        std::fs::create_dir_all(engine_dir.join("omnidesk_engine")).unwrap();
        let cli = bundle.join("runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js");
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, "// stub\n").unwrap();
        let python = base.join("python3-stub");
        std::fs::write(&python, "#!/bin/sh\nexit 0\n").unwrap();

        let seed = bundle.join("runtime/dsh-home-seed");
        let base_profile = seed.join("profiles/omnidesk-base");
        std::fs::create_dir_all(&base_profile).unwrap();
        std::fs::write(
            base_profile.join("package.json"),
            "{\"name\":\"dsh-profile-omnidesk-base\"}\n",
        )
        .unwrap();
        std::fs::write(base_profile.join("cordis.patch.yml"), "# seed\n").unwrap();
        let web_profile = seed.join("profiles/web");
        std::fs::create_dir_all(&web_profile).unwrap();
        std::fs::write(web_profile.join("package.json"), "{\"name\":\"dsh-profile-web\"}\n")
            .unwrap();
        let skill = seed.join("skills/demo");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "# demo\n").unwrap();

        let keys = [
            "OMNIDESK_APP_DATA_ROOT",
            "OMNIDESK_DSH_HOME",
            "OMNIDESK_AGENTS_HOME",
            "OMNIDESK_ENGINE_DIR",
            "OMNIDESK_DSH_CLI",
            "OMNIDESK_REPO_ROOT",
            "OMNIDESK_DSH_PROFILE",
            "OMNIDESK_ENGINE_CHANNEL",
            "OMNIDESK_DSH_PATCHES",
            "OMNIDESK_PYTHON",
        ];
        let snapshot: Vec<(&str, Option<std::ffi::OsString>)> = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect();
        std::env::set_var("OMNIDESK_APP_DATA_ROOT", &app_root);
        std::env::set_var("OMNIDESK_DSH_HOME", &dsh_home);
        std::env::set_var("OMNIDESK_AGENTS_HOME", &agents_home);
        std::env::set_var("OMNIDESK_ENGINE_DIR", &engine_dir);
        std::env::set_var("OMNIDESK_DSH_CLI", &cli);
        std::env::set_var("OMNIDESK_REPO_ROOT", &bundle);
        std::env::set_var("OMNIDESK_PYTHON", &python);
        std::env::remove_var("OMNIDESK_DSH_PROFILE");
        std::env::remove_var("OMNIDESK_ENGINE_CHANNEL");
        std::env::remove_var("OMNIDESK_DSH_PATCHES");

        let result = super::build_state();
        let manifest = dsh_home.join("profiles/omnidesk-base/package.json");
        let copied_skill = dsh_home.join("skills/demo/SKILL.md");
        let log = app_root.join("logs/profile-assets.log");
        let seeded = manifest.is_file();
        let skill_copied = copied_skill.is_file();
        let log_text = std::fs::read_to_string(&log).unwrap_or_default();

        for (key, value) in snapshot {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        let _ = std::fs::remove_dir_all(&base);

        assert!(result.is_ok(), "启动入口应成功：{:?}", result.err());
        assert!(
            seeded,
            "首次启动必须把包内 seed 的 omnidesk-base Profile 铺到 {}，实际日志：{}",
            dsh_home.display(),
            log_text
        );
        assert!(skill_copied, "seed 里的 Skill 也必须随包铺开");
        assert!(log_text.contains("seed=ready"), "启动日志应记录播种结果：{log_text}");
    }

    /// 直接调用宿主命令（不经 UI）：端点无法确认或权限关闭时必须拒绝，且不创建任何目录。
    /// OD-ARCH-023：准备目录的入口同样受守卫保护——屏障生效期间必须被拒绝，
    /// 且不得产生任何文件系统副作用。
    #[test]
    fn prepare_workspace_is_refused_while_permission_change_blocked() {
        // 与其它写环境变量的用例共用一把锁，结束后还原原值（并行跑时不能互相覆盖）。
        let _guard = crate::test_env_lock::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var_os("OMNIDESK_LOCAL_DATA_ROOT");
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

        match previous {
            Some(value) => std::env::set_var("OMNIDESK_LOCAL_DATA_ROOT", value),
            None => std::env::remove_var("OMNIDESK_LOCAL_DATA_ROOT"),
        }
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
    // OD-ARCH-137：首次启动先把包内 seed 铺到隔离 DSH_HOME。模型设置与端点分类
    // 不经过引擎启动，不能等 `start()` 才播种；否则新机器上打开「设置模型」会
    // 直接报 profile/omnidesk-base 不存在。
    seed_dsh_home_on_startup(&paths, &config);
    // OD-ARCH-109：资产装配必须在**应用启动**时完成，而不是等引擎第一次被拉起。
    //
    // 引擎只在提交首条任务时才 `start()`，冷启动后什么都不做时把装配挂在 `start()`
    // 上等于永远不执行——实测表现是「切到 discipline，设置页资产全空、Skill 一个
    // 都没复制」。因此这里在窗口创建前同步装配一次，与切换 Profile 时的装配同一入口。
    assemble_profile_assets_on_startup(&paths, &config);
    let supervisor = EngineSupervisor::new(config);
    Ok(HostState {
        bridge: EngineBridge::new(Arc::new(Mutex::new(supervisor))),
        paths,
        lifecycle: Arc::new(EngineLifecycle::new()),
        vault_index: Arc::new(Mutex::new(None)),
    })
}

/// 启动期把包内 `runtime/dsh-home-seed` 铺到隔离 DSH_HOME（OD-ARCH-137）。
///
/// 只在缺失时写，失败不阻断启动：界面后续仍会给出模型配置错误，
/// 但日志必须留下「为什么没初始化」，不能静默。
fn seed_dsh_home_on_startup(paths: &AppPaths, config: &EngineConfig) {
    let manifest = config
        .dsh_home
        .join("profiles")
        .join(&config.profile)
        .join("package.json");
    let outcome = match config.seed_profile_if_missing() {
        Ok(true) => "created".to_string(),
        Ok(false) if manifest.is_file() => "ready".to_string(),
        Ok(false) => "skipped".to_string(),
        Err(e) => format!("error: {}", e),
    };
    append_profile_assets_log(
        &paths.root,
        &format!(
            "profile={} channel={} seed={}\n",
            config.profile, config.channel, outcome
        ),
    );
    if outcome.starts_with("error") {
        eprintln!(
            "[OmniDesk][seed] profile={} channel={} seed={}",
            config.profile, config.channel, outcome
        );
    }
}

/// 启动期装配 Profile 资产，并把结果写进 `logs/profile-assets.log`。
///
/// 不阻断启动：Skill 目录权限之类的问题只应让资产页显示缺项，不该让整机打不开。
/// 但也不能静默吞掉——否则「切换 Profile 后什么都没装」将无从排查。
fn assemble_profile_assets_on_startup(paths: &AppPaths, config: &EngineConfig) {
    let outcome = match config.assemble_profile_assets() {
        Ok(true) => "ok".to_string(),
        Ok(false) => "skipped".to_string(),
        Err(e) => format!("error: {}", e),
    };
    let line = format!(
        "{} profile={} channel={} result={}\n",
        chrono_like_stamp(),
        config.profile,
        config.channel,
        outcome
    );
    let path = paths.logs.join("profile-assets.log");
    let _ = std::fs::create_dir_all(&paths.logs);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = file.write_all(line.as_bytes());
    }
    if outcome.starts_with("error") {
        eprintln!("[OmniDesk][profile-assets] {}", line.trim_end());
    }
}

/// 启动日志只需要「可排序、可读」，不引入额外依赖：用 epoch 秒替代带时区的格式化时间。
fn chrono_like_stamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

/// 启动失败时的可读提示（OD-ARCH-135）。
///
/// 打包成 GUI 子系统程序后没有终端：双击安装好的应用，失败就是「完全没反应」。
/// Windows 用系统消息框把原因摆到屏幕上；其它平台写 stderr（终端启动时可见）。
fn report_fatal_startup(message: &str) {
    eprintln!("[OmniDesk][致命] {}", message);
    #[cfg(target_os = "windows")]
    {
        use std::process::Stdio;
        // 文本走环境变量：消息里有中文、路径与引号，拼进 -Command 会踩转义。
        let script = concat!(
            "$m = $env:OMNIDESK_FATAL_TEXT; ",
            "Add-Type -AssemblyName PresentationFramework; ",
            "[System.Windows.MessageBox]::Show($m, 'OmniDesk 启动失败', 'OK', 'Error') | Out-Null"
        );
        let mut cmd = std::process::Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .env("OMNIDESK_FATAL_TEXT", message)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        crate::platform::hide_console(&mut cmd);
        let _ = cmd.status();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = match build_state() {
        Ok(state) => state,
        Err(message) => {
            // 边界校验失败：给出明确原因并退出，而不是继续运行。
            report_fatal_startup(&message);
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
            profile_decl_starter,
            profile_choice_view,
            profile_choice_set,
            profile_create,
            profile_remove,
            profile_rename,
            profile_trash_list,
            profile_trash_restore,
            profile_export,
            profile_import,
            profile_export_list,
            profile_export_open,
            profile_assets_view,
            profile_assets_repair,
            profile_assets_parked,
            profile_skill_source_open,
            profile_skill_scaffold,
            profile_assembly_prefs,
            profile_assembly_prefs_set,
            profile_plugins_ensure,
            profile_plugins_add,
            profile_plugin_install,
            profile_rules_load,
            profile_rules_save,
            profile_skill_bindings_save,
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
            work_root_info,
            task_set_working_root,
            task_open_artifact_root,
            open_external_url,
            environment_check,
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
            knowledge_ingest_init,
            knowledge_ingest_prepare,
            knowledge_ingest_apply,
            knowledge_raw_admit,
            knowledge_raw_admit_paths,
            knowledge_governance_list,
            knowledge_review_stage,
            knowledge_review_list,
            knowledge_review_decide,
            knowledge_conflict_resolve,
            knowledge_quality_report,
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
            worklog_move_many,
            worklog_merge,
            worklog_read_week,
            worklog_week_view,
            worklog_archive_list,
            worklog_read_report,
            archive_report_to_vault,
            archive_artifact_to_vault,
            work_record_create,
            work_record_read,
            work_record_update,
            work_record_delete,
            work_record_list,
            worklog_migrate_preview,
            worklog_migrate_apply,
            work_cleanup_preview,
            work_cleanup_apply,
            availability_get,
            availability_set,
            worklog_record_analyze,
            worklog_record_analyze_apply,
            worklog_record_analyze_failed,
            writing_parse_requirement,
            writing_parse_requirement_apply,
            writing_create_from_requirement,
            writing_list,
            writing_status_transition,
            writing_records_query,
            writing_export,
            writing_archive,
            writing_knowledge_feedback,
            writing_outline_generate,
            writing_outline_confirm,
            writing_outline_get,
            writing_section_generate,
            writing_section_confirm,
            writing_section_get,
            writing_section_revise,
            writing_review,
            writing_review_apply_finding,
            writing_version_list,
            writing_version_diff,
            writing_version_restore,
            writing_factcheck_run,
            writing_factcheck_resolve,
            task_create,
            task_read,
            task_update,
            task_transition,
            task_append_progress,
            task_set_importance,
            task_set_quadrant,
            task_set_due,
            task_set_estimated,
            task_set_dependency,
            task_quadrant_views,
            schedule_generate,
            schedule_confirm,
            schedule_adjust,
            schedule_cancel,
            schedule_lock,
            schedule_list,
            task_delete,
            task_list,
            task_candidate_list,
            task_candidate_confirm,
            task_candidate_reject,
            task_candidate_link,
            task_candidate_dismiss_match,
            project_create,
            project_update,
            project_list,
            project_overview,
            knowledge_link_suggest,
            knowledge_link_confirm,
            knowledge_link_remove,
            knowledge_link_by_knowledge,
            material_extract_from_record,
            material_create,
            material_list,
            material_get,
            material_review,
            material_edit,
            material_query,
            writing_collect_materials,
            writing_select_materials,
        ])
        .setup(|app| {
            // 启动时只设置窗口标题，不拉起引擎（引擎按需启动）。
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("OmniDesk Discipline");
            }
            // OD-ARCH-124：已选好的非标准 Profile 冷启动时若缺插件，后台自愈。
            // 只是探测 + 起后台任务，不阻塞窗口；基础标准与开关关闭时零动作。
            maybe_autofill_plugins_on_startup(app.handle());
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
mod schedule_ipc_tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const NOW: f64 = 1_791_000_000.0;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "omnidesk-qs409-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn root(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn generate_persists_suggestions_and_stale_tracks_task_updates() {
        let temp = TempRoot::new("generate");
        let task = task_store::create_task(
            temp.root(),
            &task_store::TaskDraft {
                title: "写周报".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        task_store::set_importance(
            temp.root(),
            &task.id,
            task_store::IMPORTANCE_HIGH,
            task_store::IMPORTANCE_SOURCE_RULE,
            Some("本周评审要用"),
            NOW,
        )
        .unwrap();
        task_store::set_estimated_minutes(temp.root(), &task.id, Some(60), NOW).unwrap();
        task_store::set_due(
            temp.root(),
            &task.id,
            Some("2026-11-20"),
            task_store::DUE_KIND_HARD,
            "评审日期",
            NOW,
        )
        .unwrap();

        let first = schedule_generate_impl(temp.root(), NOW, 0).unwrap();
        assert!(!first.created.is_empty(), "{first:?}");
        assert!(!first.stale);
        assert!(first.generated_at.is_some());

        let second = schedule_generate_impl(temp.root(), NOW + 1.0, 0).unwrap();
        assert!(second.created.is_empty(), "已建议条目不应被重复补建");

        task_store::update_task(
            temp.root(),
            &task.id,
            &task_store::TaskPatch {
                title: Some("改过标题".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        let tasks = task_store::list_tasks(temp.root(), &task_store::TaskFilter::default())
            .unwrap()
            .items;
        assert!(schedule_store::is_stale(temp.root(), &tasks).unwrap());
    }
}

#[cfg(test)]
mod engine_lifecycle_wiring_tests {
    use super::*;

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

    /// OD-ARCH-110：切换 Profile 必须同步 supervisor 与只读快照两份配置。
    ///
    /// 只比对“两边都被写成了同一份新配置”。若只更新其中一处，界面读到的 Profile
    /// 与引擎启动用的 Profile 就会分叉——这正是本轮修复的缺陷。
    #[test]
    fn adopt_engine_config_updates_supervisor_and_snapshot_together() {
        let base = std::env::temp_dir().join("omnidesk-od110-switch");
        let old = EngineConfig {
            python: "python3".to_string(),
            engine_dir: base.join("engine"),
            dsh_cli: base.join("dsh.js"),
            dsh_home: base.join("dsh-home"),
            agents_home: base.join("agents-home"),
            engine_state_dir: base.join("app").join("engine-state"),
            result_dir: base.join("app").join("engine-state").join("results"),
            profile: "omnidesk-base".to_string(),
            channel: "web-api".to_string(),
            patch_files: vec![],
        };
        let mut next = old.clone();
        next.profile = "discipline".to_string();

        let supervisor = Arc::new(Mutex::new(EngineSupervisor::new(old.clone())));
        let snapshot = Mutex::new(old);
        adopt_engine_config(&supervisor, Some(&snapshot), next.clone());

        assert_eq!(
            supervisor.lock().unwrap().config().profile,
            "discipline",
            "supervisor 必须改用新 Profile（下一条任务按它启动）"
        );
        assert_eq!(
            snapshot.lock().unwrap().profile,
            "discipline",
            "只读快照必须与 supervisor 一致（准入与端点事实读它）"
        );
        // 其它字段也要一起换成新配置，而不是只改 profile。
        assert_eq!(
            supervisor.lock().unwrap().config().dsh_home,
            next.dsh_home,
            "必须整份替换配置，不能只改 profile 字段"
        );
    }

    /// OD-ARCH-110：切换流程必须是「先立屏障 → 再换配置 → 再停旧引擎」。
    ///
    /// 顺序反了会有一个窗口：配置已换、旧引擎还在跑，新任务带着新 Profile 的规则
    /// 被送进旧引擎。这里用源码顺序钉住这个约束。
    #[test]
    fn profile_switch_barrier_precedes_config_swap_and_stop() {
        let src = include_str!("lib.rs");
        let body = src.split("\n#[cfg(test)]").next().expect("源码前缀");
        let start = body
            .find("fn profile_choice_set(")
            .expect("找不到 profile_choice_set");
        let rest = &body[start..];
        let end = rest[1..]
            .find("\n#[tauri::command")
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let code = &rest[..end];
        let barrier = code.find("begin_change()").expect("必须在屏障内切换");
        let swap = code
            .find("adopt_engine_config(")
            .expect("必须同步新配置到 supervisor 与快照");
        let stop = code
            .find("stop_engine(")
            .expect("必须停止旧引擎，否则新 Profile 不会真正接管");
        assert!(
            barrier < swap && swap < stop,
            "顺序必须是 begin_change → adopt_engine_config → stop_engine"
        );
        assert!(
            code.contains("change.succeed()") && code.contains("change.fail("),
            "停引擎成功才清屏障，失败必须保留屏障（失败关闭）"
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

#[cfg(test)]
mod profile_autofill_tests {
    //! OD-ARCH-121：切换后是否自动补装的判定。
    //!
    //! 四种组合逐条钉住：基础标准永远不装（干净底座的承诺），
    //! 开关关闭时行为回到 114 的「只提示」，没有缺件时不白跑一次网络。

    use super::should_auto_install_plugins;

    const BASE: &str = "omnidesk-base";

    #[test]
    fn installs_only_when_missing_and_enabled_and_not_standard() {
        assert!(should_auto_install_plugins(2, Some("discipline"), BASE, true));
    }

    #[test]
    fn standard_profile_never_installs() {
        assert!(!should_auto_install_plugins(2, None, BASE, true), "跟随标准模式");
        assert!(!should_auto_install_plugins(2, Some(BASE), BASE, true), "显式选基础标准");
    }

    #[test]
    fn disabled_switch_falls_back_to_prompt_only() {
        assert!(!should_auto_install_plugins(2, Some("discipline"), BASE, false));
    }

    #[test]
    fn nothing_missing_is_a_noop() {
        assert!(!should_auto_install_plugins(0, Some("discipline"), BASE, true));
    }
}

#[cfg(test)]
mod profile_decl_save_tests {
    //! OD-ARCH-123：场景声明的保存边界。
    //!
    //! 钉住两条事实：基础标准连目录都不建（干净底座要能靠目录复核），
    //! 非标准 Profile 保存成功并真的落到 decl.yml。

    use super::save_decl_into;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("od123-{tag}-{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn standard_refuses_and_writes_nothing() {
        let root = temp_dir("standard");
        let target = root.join("profiles").join("omnidesk-base");
        let text = crate::profile_starter::decl_text("x").unwrap();
        let err = save_decl_into(&target, true, &text).expect_err("基础标准必须拒绝保存声明");
        assert!(err.contains("基础标准"), "错误文案要说清原因：{err}");
        assert!(!target.exists(), "拒绝保存时不得创建 Profile 目录");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn custom_profile_writes_decl() {
        let root = temp_dir("custom");
        let target = root.join("profiles").join("my-desk");
        let text = crate::profile_starter::decl_text("我的工作台").unwrap();
        let receipt = save_decl_into(&target, false, &text).expect("非标准 Profile 应能保存");
        assert_eq!(receipt["ok"], serde_json::json!(true));
        assert!(target.join(crate::profile_decl::PROFILE_DECL_FILE).is_file());
        assert!(receipt["note"].as_str().unwrap().contains("不需要重启"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn invalid_text_never_touches_disk() {
        let root = temp_dir("invalid");
        let target = root.join("profiles").join("my-desk");
        assert!(save_decl_into(&target, false, "schema: nope\n").is_err());
        assert!(!target.exists(), "校验失败不得创建目录");
        std::fs::remove_dir_all(&root).ok();
    }
}

#[cfg(test)]
mod profile_rules_save_tests {
    //! OD-ARCH-128：规则资产的保存边界与声明同口径。
    //!
    //! 基础标准连目录都不建；自建 Profile 才写盘；校验失败不落盘。

    use super::save_rules_into;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("od128-rules-{tag}-{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn standard_refuses_and_writes_nothing() {
        let root = temp_dir("standard");
        let target = root.join("profiles").join("omnidesk-base");
        let err = save_rules_into(&target, true, crate::profile_rules::DEFAULT_RULES_JSON)
            .expect_err("基础标准必须拒绝保存规则");
        assert!(err.contains("干净底座"), "错误文案要说清原因：{err}");
        assert!(!target.exists(), "拒绝保存时不得创建 Profile 目录");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn custom_profile_writes_rules() {
        let root = temp_dir("custom");
        let target = root.join("profiles").join("my-desk");
        let receipt = save_rules_into(&target, false, crate::profile_rules::DEFAULT_RULES_JSON)
            .expect("自建 Profile 应能保存规则");
        assert_eq!(receipt["ok"], serde_json::json!(true));
        assert!(target.join(crate::profile_rules::PROFILE_RULES_FILE).is_file());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn invalid_rules_never_touch_disk() {
        let root = temp_dir("invalid");
        let target = root.join("profiles").join("my-desk");
        assert!(save_rules_into(&target, false, "{ not json").is_err());
        assert!(!target.exists(), "校验失败不得创建目录");
        std::fs::remove_dir_all(&root).ok();
    }
}

#[cfg(test)]
mod profile_skill_bindings_tests {
    //! OD-ARCH-129：场景 → Skill 绑定的保存边界。
    //!
    //! 三条事实：基础标准零写入；新 Profile 无规则文件时用起步骨架打底；
    //! 保存绑定只换 skill_bindings，红线词与写作铁律逐字节保留。

    use super::save_skill_bindings_into;
    use crate::profile_rules::BindingChoice;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("od129-bindings-{tag}-{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn choice(scene: &str, skills: &[&str]) -> BindingChoice {
        BindingChoice {
            scene: scene.to_string(),
            skills: skills.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn standard_refuses_and_writes_nothing() {
        let root = temp_dir("standard");
        let target = root.join("profiles").join("omnidesk-base");
        let err = save_skill_bindings_into(&target, true, &[choice("weekly", &["memory-agent"])])
            .expect_err("基础标准必须拒绝保存绑定");
        assert!(err.contains("干净底座"), "错误文案要说清原因：{err}");
        assert!(!target.exists(), "拒绝保存时不得创建 Profile 目录");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn new_profile_starts_from_starter_rules() {
        let root = temp_dir("fresh");
        let target = root.join("profiles").join("my-desk");
        save_skill_bindings_into(&target, false, &[choice("weekly", &["memory-agent"])])
            .expect("没有规则文件时应以起步骨架打底");
        let text =
            std::fs::read_to_string(target.join(crate::profile_rules::PROFILE_RULES_FILE)).unwrap();
        let rules = crate::profile_rules::parse_rules(&text).unwrap();
        assert_eq!(rules.skill_bindings["weekly"].skills, vec!["memory-agent"]);
        assert_eq!(rules.skill_bindings["weekly"].max_injected, 1);
        assert!(rules.global.redline_keywords.is_empty(), "起步规则包不得带行业红线");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn keeps_other_rule_content_intact() {
        let root = temp_dir("keep");
        let target = root.join("profiles").join("my-desk");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(
            target.join(crate::profile_rules::PROFILE_RULES_FILE),
            crate::profile_rules::DEFAULT_RULES_JSON,
        )
        .unwrap();
        save_skill_bindings_into(&target, false, &[choice("report", &["memory-report"])])
            .expect("已有规则文件时应能改绑定");
        let text =
            std::fs::read_to_string(target.join(crate::profile_rules::PROFILE_RULES_FILE)).unwrap();
        let rules = crate::profile_rules::parse_rules(&text).unwrap();
        assert_eq!(rules.skill_bindings.len(), 1, "整表替换，旧的场景绑定不残留");
        assert_eq!(rules.skill_bindings["report"].skills, vec!["memory-report"]);
        let original = crate::profile_rules::parse_rules(crate::profile_rules::DEFAULT_RULES_JSON)
            .unwrap();
        assert_eq!(rules.global, original.global, "红线词 / 铁律 / 隐私策略必须原样保留");
        assert_eq!(rules.scenes, original.scenes, "场景规则必须原样保留");
        std::fs::remove_dir_all(&root).ok();
    }

    /// OD-ARCH-131 起，绑定只校验名字形状（名字合法但暂时没有正文来源可以保存，
    /// 由资产页提示「找不到正文来源」）；真正必须挡住的是写不成目录的非法名字。
    #[test]
    fn bad_skill_name_never_touches_disk() {
        let root = temp_dir("bad");
        let target = root.join("profiles").join("my-desk");
        std::fs::create_dir_all(&target).unwrap();
        let path = target.join(crate::profile_rules::PROFILE_RULES_FILE);
        std::fs::write(&path, crate::profile_rules::DEFAULT_RULES_JSON).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(save_skill_bindings_into(&target, false, &[choice("weekly", &["../escape"])])
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before, "被拒时文件逐字节不变");
        std::fs::remove_dir_all(&root).ok();
    }
}

#[cfg(test)]
mod work_store_ipc_tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const NOW: f64 = 1_780_100_000.0;
    const OFFSET: i32 = 480;
    const BODY: &str = "今天整理周报，明天写总结。";

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-wr302-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn root(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn policy_with(redline: &[&str], mask: bool) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        let mode = if mask { "mask" } else { "off" };
        rules.global.privacy.id_card = mode.to_string();
        rules.global.privacy.phone = mode.to_string();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn record_draft(title: &str, body: &str) -> worklog_records::RecordDraft {
        worklog_records::RecordDraft {
            title: Some(title.to_string()),
            body: body.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn record_create_masks_privacy_and_reports_counts() {
        let temp = TempRoot::new("mask");
        let policy = policy_with(&[], true);
        let saved = work_record_create_impl(
            temp.root(),
            &policy,
            record_draft("隐私记录", "身份证 110101199003077777，电话 13800138000"),
            NOW,
            OFFSET,
        )
        .unwrap();
        assert!(saved.record.body.contains("110*************77"), "{}", saved.record.body);
        assert!(saved.record.body.contains("138******00"), "{}", saved.record.body);
        assert!(saved.masked);
        assert_eq!(saved.masked_id_card, 1);
        assert_eq!(saved.masked_phone, 1);
        let stored =
            worklog_records::read_record(temp.root(), &saved.record.id).expect("落盘记录必须可读");
        assert!(stored.body.contains("110*************77"));
        assert!(!stored.body.contains("110101199003077777"));
    }

    #[test]
    fn record_create_rejects_red_line_without_writing() {
        let temp = TempRoot::new("redline");
        let policy = policy_with(&["机密"], false);
        let err = work_record_create_impl(
            temp.root(),
            &policy,
            record_draft("命中", "这段包含机密信息"),
            NOW,
            OFFSET,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!err.contains("机密信息"), "错误不得回显命中原文：{err}");
        let listing = worklog_records::list_records(
            temp.root(),
            &worklog_records::RecordFilter::default(),
        )
        .unwrap();
        assert!(listing.items.is_empty(), "{:?}", listing.items);
    }

    #[test]
    fn record_crud_round_trip_through_ipc_impl() {
        let temp = TempRoot::new("crud");
        let policy = policy_with(&[], false);
        let saved = work_record_create_impl(
            temp.root(),
            &policy,
            worklog_records::RecordDraft {
                title: Some("周一复盘".to_string()),
                record_date: Some("2026-11-02".to_string()),
                category: Some("项目".to_string()),
                body: "讨论了验收口径".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap();
        let listing = worklog_records::list_records(
            temp.root(),
            &worklog_records::RecordFilter {
                keyword: Some("验收".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(listing.items.len(), 1);
        let updated = work_record_update_impl(
            temp.root(),
            &policy,
            &saved.record.id,
            worklog_records::RecordPatch {
                title: Some("周一复盘（已归档）".to_string()),
                draft: Some(false),
                ai_analysis_status: Some(worklog_records::AI_ANALYZED.to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(updated.record.created_at, saved.record.created_at);
        assert_eq!(updated.record.ai_analysis_status, worklog_records::AI_ANALYZED);
        let moved = worklog_records::delete_record(temp.root(), &saved.record.id, NOW).unwrap();
        assert!(Path::new(&moved.path).exists());
        assert!(worklog_records::read_record(temp.root(), &saved.record.id).is_err());
        let listing = worklog_records::list_records(
            temp.root(),
            &worklog_records::RecordFilter::default(),
        )
        .unwrap();
        assert!(listing.items.is_empty());
    }

    #[test]
    fn candidate_stays_out_of_task_list_until_confirmed() {
        let temp = TempRoot::new("candidate");
        let policy = policy_with(&[], false);
        let saved = work_record_create_impl(
            temp.root(),
            &policy,
            record_draft("工作记录", BODY),
            NOW,
            OFFSET,
        )
        .unwrap();
        let staged = task_store::stage_candidates(
            temp.root(),
            &saved.record.id,
            &[task_store::CandidateInput {
                title: "整理周报".to_string(),
                source_quote: "整理周报".to_string(),
                confidence: Some(0.7),
                ..Default::default()
            }],
            NOW,
        )
        .unwrap();
        assert_eq!(staged.staged.len(), 1);
        let tasks =
            task_store::list_tasks(temp.root(), &task_store::TaskFilter::default()).unwrap();
        assert!(tasks.items.is_empty(), "候选确认前不得进入正式清单");
        let candidates = task_store::list_candidates(temp.root()).unwrap();
        assert_eq!(candidates.items.len(), 1);
        let confirmed = task_candidate_confirm_impl(
            temp.root(),
            &policy,
            &staged.staged[0].id,
            task_store::ConfirmEdits::default(),
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(confirmed.task.state, task_store::STATE_NOT_STARTED);
        assert_eq!(
            confirmed.task.source_record_id.as_deref(),
            Some(saved.record.id.as_str())
        );
        let tasks =
            task_store::list_tasks(temp.root(), &task_store::TaskFilter::default()).unwrap();
        assert_eq!(tasks.items.len(), 1);
        let candidates = task_store::list_candidates(temp.root()).unwrap();
        assert!(candidates.items.is_empty());
    }

    #[test]
    fn task_flows_mask_title_note_and_progress() {
        let temp = TempRoot::new("task-mask");
        let policy = policy_with(&[], true);
        let saved = task_create_impl(
            temp.root(),
            &policy,
            task_store::TaskDraft {
                title: "联系 13800138000".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        assert_eq!(saved.task.title, "联系 138******00");
        assert_eq!(saved.masked_phone, 1);
        let moved = task_transition_impl(
            temp.root(),
            &policy,
            &saved.task.id,
            task_store::STATE_IN_PROGRESS,
            Some("回电 13800138000".to_string()),
            NOW + 60.0,
        )
        .unwrap();
        assert_eq!(
            moved.task.history[0].note.as_deref(),
            Some("回电 138******00")
        );
        let progressed = task_append_progress_impl(
            temp.root(),
            &policy,
            &saved.task.id,
            "身份证 110101199003077777",
            NOW + 120.0,
        )
        .unwrap();
        assert_eq!(progressed.task.progress[0].text, "身份证 110*************77");
    }

    #[test]
    fn red_line_blocks_task_progress_without_changing_task() {
        let temp = TempRoot::new("task-redline");
        let clean = policy_with(&[], false);
        let saved = task_create_impl(
            temp.root(),
            &clean,
            task_store::TaskDraft {
                title: "写周报".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let redline = policy_with(&["机密"], false);
        let err = task_append_progress_impl(
            temp.root(),
            &redline,
            &saved.task.id,
            "记录机密内容",
            NOW + 60.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        let stored = task_store::read_task(temp.root(), &saved.task.id).unwrap();
        assert!(stored.progress.is_empty());
    }

    #[test]
    fn candidate_confirm_rejects_red_line_in_ai_title() {
        let temp = TempRoot::new("candidate-redline");
        let clean = policy_with(&[], false);
        let saved = work_record_create_impl(
            temp.root(),
            &clean,
            record_draft("工作记录", BODY),
            NOW,
            OFFSET,
        )
        .unwrap();
        let staged = task_store::stage_candidates(
            temp.root(),
            &saved.record.id,
            &[task_store::CandidateInput {
                title: "机密事项".to_string(),
                source_quote: "整理周报".to_string(),
                ..Default::default()
            }],
            NOW,
        )
        .unwrap();
        let redline = policy_with(&["机密"], false);
        let err = task_candidate_confirm_impl(
            temp.root(),
            &redline,
            &staged.staged[0].id,
            task_store::ConfirmEdits::default(),
            NOW + 60.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        let tasks =
            task_store::list_tasks(temp.root(), &task_store::TaskFilter::default()).unwrap();
        assert!(tasks.items.is_empty());
        let candidates = task_store::list_candidates(temp.root()).unwrap();
        assert_eq!(candidates.items.len(), 1, "被拒后候选仍留在待确认");
    }

    #[test]
    fn work_root_boundary_matches_artifact_rules() {
        let base = TempRoot::new("root");
        let app_root = base.root().join("app");
        std::fs::create_dir_all(&app_root).unwrap();
        let err =
            settings::resolve_working_root(&app_root, Some(PathBuf::from("relative/work")))
                .unwrap_err();
        assert!(err.contains("绝对路径"), "{err}");
        let err =
            settings::resolve_working_root(&app_root, Some(app_root.join("work"))).unwrap_err();
        assert!(err.contains("应用自身数据目录"), "{err}");
        let resolved =
            settings::resolve_working_root(&app_root, Some(base.root().join("work"))).unwrap();
        assert!(resolved.starts_with(base.root()));
    }
}

#[cfg(test)]
mod project_ipc_tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const NOW: f64 = 1_796_000_000.0;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-km508-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn root(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn policy_with(redline: &[&str], mask: bool) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        let mode = if mask { "mask" } else { "off" };
        rules.global.privacy.id_card = mode.to_string();
        rules.global.privacy.phone = mode.to_string();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn project_draft(name: &str) -> project_store::ProjectDraft {
        project_store::ProjectDraft {
            name: name.to_string(),
            goal: "虚构项目目标。".to_string(),
            ..Default::default()
        }
    }

    /// 在 frontmatter 结束围栏前追加字段，用来构造 503 交付的关联字段。
    fn add_frontmatter_fields(raw: &str, extra: &str) -> String {
        let close = 4 + raw[4..]
            .find("\n---\n")
            .expect("必须存在 frontmatter 结束围栏")
            + 1;
        format!("{}{}{}", &raw[..close], extra, &raw[close..])
    }

    fn add_knowledge_link(path: &Path, knowledge_id: &str) {
        let raw = std::fs::read_to_string(path).unwrap();
        let extra = format!(
            "related_knowledge:\n  - id: \"{knowledge_id}\"\n    reason: \"虚构关联依据\"\n    linked_at: \"2026-02-02T08:15:00Z\"\n"
        );
        std::fs::write(path, add_frontmatter_fields(&raw, &extra)).unwrap();
    }

    #[test]
    fn project_create_gates_user_text_without_writing_on_red_line() {
        let temp = TempRoot::new("project-gate");
        let policy = policy_with(&["机密"], false);
        let mut draft = project_draft("虚构项目");
        draft.goal = "虚构目标包含机密样例".to_string();
        let err = project_create_impl(temp.root(), &policy, draft, NOW).unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!err.contains("机密样例"), "错误不得回显命中原文：{err}");
        assert!(
            !project_store::project_dir(temp.root()).exists(),
            "被拒绝时不得创建项目目录"
        );
    }

    #[test]
    fn task_create_rejects_missing_project_with_project_hint() {
        let temp = TempRoot::new("task-missing-project");
        let policy = policy_with(&[], false);
        let err = task_create_impl(
            temp.root(),
            &policy,
            task_store::TaskDraft {
                title: "虚构任务".to_string(),
                project_id: Some("p-missing-508".to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("关联项目不存在"), "{err}");
        assert!(err.contains("项目"), "错误必须带项目提示：{err}");
        let listing =
            task_store::list_tasks(temp.root(), &task_store::TaskFilter::default()).unwrap();
        assert!(listing.items.is_empty(), "被拒绝时不得写任务文件");
    }

    #[test]
    fn task_update_rejects_missing_project_and_leaves_file_unchanged() {
        let temp = TempRoot::new("task-update-missing-project");
        let policy = policy_with(&[], false);
        let saved = task_create_impl(
            temp.root(),
            &policy,
            task_store::TaskDraft {
                title: "虚构任务".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let path = task_store::tasks_dir(temp.root()).join(format!("{}.md", saved.task.id));
        let before = std::fs::read(&path).unwrap();
        let err = task_update_impl(
            temp.root(),
            &policy,
            &saved.task.id,
            task_store::TaskPatch {
                project_id: Some("p-missing-508".to_string()),
                ..Default::default()
            },
            NOW + 60.0,
        )
        .unwrap_err();
        assert!(err.contains("关联项目不存在"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), before, "拒绝时任务文件字节不变");
        let stored = task_store::read_task(temp.root(), &saved.task.id).unwrap();
        assert_eq!(stored.project_id, None);
    }

    #[test]
    fn task_save_accepts_existing_project_and_overview_counts_it() {
        let temp = TempRoot::new("task-existing-project");
        let policy = policy_with(&[], false);
        let project = project_create_impl(temp.root(), &policy, project_draft("虚构项目甲"), NOW)
            .unwrap();
        let saved = task_create_impl(
            temp.root(),
            &policy,
            task_store::TaskDraft {
                title: "虚构任务".to_string(),
                project_id: Some(project.project_id.clone()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        assert_eq!(saved.task.project_id.as_deref(), Some(project.project_id.as_str()));

        let overview = project_overview_impl(temp.root(), &project.project_id).unwrap();
        assert_eq!(overview.project.project_id, project.project_id);
        assert_eq!(overview.task_count, 1);
        assert_eq!(overview.tasks[0].id, saved.task.id);
        assert_eq!(overview.record_count, 0);
        assert_eq!(overview.material_count, 0);
        assert_eq!(overview.knowledge_count, 0);
    }

    #[test]
    fn project_overview_derives_records_materials_and_knowledge() {
        let temp = TempRoot::new("overview");
        let policy = policy_with(&[], false);
        let project_a =
            project_create_impl(temp.root(), &policy, project_draft("虚构项目甲"), NOW).unwrap();
        let project_b =
            project_create_impl(temp.root(), &policy, project_draft("虚构项目乙"), NOW).unwrap();

        let task = task_create_impl(
            temp.root(),
            &policy,
            task_store::TaskDraft {
                title: "虚构任务".to_string(),
                project_id: Some(project_a.project_id.clone()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let task_path = task_store::tasks_dir(temp.root()).join(format!("{}.md", task.task.id));
        add_knowledge_link(&task_path, "notes/synthetic-508.md");

        let record = worklog_records::create_record(
            temp.root(),
            &worklog_records::RecordDraft {
                title: Some("虚构记录".to_string()),
                body: "虚构工作事实。".to_string(),
                linked_task_ids: vec![task.task.id.clone()],
                ..Default::default()
            },
            NOW,
            480,
        )
        .unwrap();
        let record_path =
            worklog_records::records_dir(temp.root()).join(format!("{}.md", record.id));
        add_knowledge_link(&record_path, "notes/synthetic-508.md");

        material_store::create_material(
            temp.root(),
            &material_store::MaterialDraft {
                title: "虚构素材".to_string(),
                content: "虚构素材正文。".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_USER.to_string(),
                source_quote: "虚构来源引用。".to_string(),
                related_project_id: Some(project_a.project_id.clone()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();

        let overview = project_overview_impl(temp.root(), &project_a.project_id).unwrap();
        assert_eq!(overview.task_count, 1);
        assert_eq!(overview.record_count, 1);
        assert_eq!(overview.material_count, 1);
        assert_eq!(overview.materials[0].title, "虚构素材");
        assert_eq!(overview.knowledge_count, 1, "同一知识条目按 id 去重");
        assert_eq!(overview.knowledge[0].knowledge_id, "notes/synthetic-508.md");
        assert_eq!(overview.knowledge[0].references.len(), 2);
        assert!(overview
            .knowledge[0]
            .references
            .iter()
            .any(|reference| reference.source_kind == "task"));
        assert!(overview
            .knowledge[0]
            .references
            .iter()
            .any(|reference| reference.source_kind == "record"));

        let other = project_overview_impl(temp.root(), &project_b.project_id).unwrap();
        assert_eq!(other.task_count, 0);
        assert_eq!(other.record_count, 0);
        assert_eq!(other.material_count, 0);
        assert_eq!(other.knowledge_count, 0);
    }
}

#[cfg(test)]
mod material_ipc_tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const NOW: f64 = 1_796_000_000.0;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omnidesk-km506-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn root(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn policy_with(redline: &[&str], mask: bool) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        let mode = if mask { "mask" } else { "off" };
        rules.global.privacy.id_card = mode.to_string();
        rules.global.privacy.phone = mode.to_string();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn user_draft(content: &str) -> material_store::MaterialDraft {
        material_store::MaterialDraft {
            title: "虚构素材标题".to_string(),
            content: content.to_string(),
            material_type: material_store::TYPE_WORK_FACT.to_string(),
            source_type: material_store::SOURCE_USER.to_string(),
            // 501 R1：user 来源引用必填，界面可把正文作为引用传入。
            source_quote: content.to_string(),
            ..Default::default()
        }
    }

    fn material_file(root: &Path, id: &str) -> PathBuf {
        material_store::material_path(root, id).unwrap()
    }

    #[test]
    fn material_create_gates_text_and_requires_user_quote() {
        let temp = TempRoot::new("create-gate");
        let redline = policy_with(&["机密"], false);
        let mut draft = user_draft("虚构正文包含机密样例");
        draft.title = "虚构标题".to_string();
        let err = material_create_impl(temp.root(), &redline, draft, NOW).unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!err.contains("机密样例"), "错误不得回显命中原文：{err}");
        assert!(
            !material_store::material_dir(temp.root()).exists(),
            "被拒绝时不得创建素材篮目录"
        );

        let clean = policy_with(&[], false);
        let mut missing_quote = user_draft("虚构正文");
        missing_quote.source_quote = String::new();
        let err = material_create_impl(temp.root(), &clean, missing_quote, NOW).unwrap_err();
        assert!(err.contains("缺少来源原文引用"), "{err}");
        assert!(!material_store::material_dir(temp.root()).exists());

        let saved = material_create_impl(temp.root(), &clean, user_draft("虚构正文"), NOW).unwrap();
        assert_eq!(saved.source_id, material_store::SOURCE_USER);
        assert_eq!(saved.source_quote, "虚构正文");
        assert_eq!(saved.review_status, material_store::REVIEW_PENDING);
        let duplicate = material_create_impl(temp.root(), &clean, user_draft("虚构正文"), NOW + 1.0)
            .unwrap_err();
        assert!(duplicate.contains("同指纹素材"), "{duplicate}");

        let mut no_source = user_draft("虚构无来源正文");
        no_source.source_type = String::new();
        let collected =
            material_create_impl(temp.root(), &clean, no_source, NOW + 2.0).unwrap();
        assert_eq!(
            collected.source_type,
            material_store::SOURCE_USER,
            "不指定来源时按用户收藏落盘"
        );
    }

    #[test]
    fn material_list_supports_type_status_date_project_and_task_filters() {
        let temp = TempRoot::new("list-filter");
        let policy = policy_with(&[], false);
        let task = task_store::create_task(
            temp.root(),
            &task_store::TaskDraft {
                title: "虚构任务".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let project_b = project_store::create_project(
            temp.root(),
            &project_store::ProjectDraft {
                name: "虚构项目乙".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();

        let first = material_store::create_material(
            temp.root(),
            &material_store::MaterialDraft {
                title: "虚构素材甲".to_string(),
                content: "虚构素材甲正文。".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_USER.to_string(),
                source_quote: "虚构素材甲正文。".to_string(),
                related_task_ids: vec![task.id.clone()],
                event_date: Some("2026-10-01".to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let second = material_store::create_material(
            temp.root(),
            &material_store::MaterialDraft {
                title: "虚构素材乙".to_string(),
                content: "虚构素材乙正文。".to_string(),
                material_type: material_store::TYPE_WORK_RESULT.to_string(),
                source_type: material_store::SOURCE_USER.to_string(),
                source_quote: "虚构素材乙正文。".to_string(),
                related_project_id: Some(project_b.project_id.clone()),
                event_date: Some("2026-10-05".to_string()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_review_impl(
            temp.root(),
            &policy,
            &second.material_id,
            "confirm",
            None,
            NOW + 10.0,
        )
        .unwrap();

        let by_type = material_store::list_materials(
            temp.root(),
            &material_store::MaterialFilter {
                material_type: Some(material_store::TYPE_WORK_FACT.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_type.items.len(), 1);
        assert_eq!(by_type.items[0].material_id, first.material_id);

        let by_status = material_store::list_materials(
            temp.root(),
            &material_store::MaterialFilter {
                review_status: Some(material_store::REVIEW_CONFIRMED.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_status.items.len(), 1);
        assert_eq!(by_status.items[0].material_id, second.material_id);

        let by_date = material_store::list_materials(
            temp.root(),
            &material_store::MaterialFilter {
                date_from: Some("2026-10-04".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_date.items.len(), 1, "只保留事件日期在区间内的素材");
        assert_eq!(by_date.items[0].material_id, second.material_id);

        let by_project = material_store::list_materials(
            temp.root(),
            &material_store::MaterialFilter {
                project_id: Some(project_b.project_id.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_project.items.len(), 1);

        let by_task = material_store::list_materials(
            temp.root(),
            &material_store::MaterialFilter {
                task_id: Some(task.id.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_task.items.len(), 1);
        assert_eq!(by_task.items[0].material_id, first.material_id);

        let all = material_store::list_materials(
            temp.root(),
            &material_store::MaterialFilter::default(),
        )
        .unwrap();
        assert_eq!(all.items.len(), 2);
        assert_eq!(all.items[0].review_status, material_store::REVIEW_PENDING, "待审核置顶");
    }

    #[test]
    fn material_review_flow_requires_reasons_and_appends_records() {
        let temp = TempRoot::new("review-flow");
        let policy = policy_with(&[], false);
        let saved = material_create_impl(temp.root(), &policy, user_draft("虚构审核正文甲"), NOW)
            .unwrap();

        let err = material_review_impl(
            temp.root(),
            &policy,
            &saved.material_id,
            "return",
            None,
            NOW + 1.0,
        )
        .unwrap_err();
        assert!(err.contains("退回素材必须填写理由"), "{err}");

        let returned = material_review_impl(
            temp.root(),
            &policy,
            &saved.material_id,
            "return",
            Some("虚构退回理由".to_string()),
            NOW + 2.0,
        )
        .unwrap();
        assert_eq!(returned.review_status, material_store::REVIEW_RETURNED);
        let last = returned.review_records.last().unwrap();
        assert_eq!(last.action, material_store::ACTION_RETURN);
        assert_eq!(last.reason, "虚构退回理由");

        let err = material_review_impl(
            temp.root(),
            &policy,
            &saved.material_id,
            "reject",
            None,
            NOW + 3.0,
        )
        .unwrap_err();
        assert!(err.contains("拒绝素材必须填写理由"), "{err}");
        let rejected = material_review_impl(
            temp.root(),
            &policy,
            &saved.material_id,
            "reject",
            Some("虚构拒绝理由".to_string()),
            NOW + 4.0,
        )
        .unwrap();
        assert_eq!(rejected.review_status, material_store::REVIEW_REJECTED);
        assert_eq!(rejected.review_records.len(), 3, "创建 + 退回 + 拒绝");

        let pending = material_create_impl(temp.root(), &policy, user_draft("虚构审核正文乙"), NOW + 5.0)
            .unwrap();
        let err = material_review_impl(
            temp.root(),
            &policy,
            &pending.material_id,
            "archive",
            None,
            NOW + 6.0,
        )
        .unwrap_err();
        assert!(err.contains("只有已确认素材可以归档"), "{err}");
        let confirmed = material_review_impl(
            temp.root(),
            &policy,
            &pending.material_id,
            "confirm",
            None,
            NOW + 7.0,
        )
        .unwrap();
        assert_eq!(confirmed.review_status, material_store::REVIEW_CONFIRMED);
        assert_eq!(confirmed.confirmation_status, material_store::CONFIRMATION_CONFIRMED);
        let archived = material_review_impl(
            temp.root(),
            &policy,
            &pending.material_id,
            "archive",
            None,
            NOW + 8.0,
        )
        .unwrap();
        assert_eq!(archived.review_status, material_store::REVIEW_ARCHIVED);
        assert_eq!(archived.review_records.last().unwrap().action, material_store::ACTION_ARCHIVE);
    }

    #[test]
    fn material_review_gates_reason_without_writing() {
        let temp = TempRoot::new("review-gate");
        let clean = policy_with(&[], false);
        let saved = material_create_impl(temp.root(), &clean, user_draft("虚构审核正文"), NOW)
            .unwrap();
        let path = material_file(temp.root(), &saved.material_id);
        let before = std::fs::read(&path).unwrap();

        let redline = policy_with(&["机密"], false);
        let err = material_review_impl(
            temp.root(),
            &redline,
            &saved.material_id,
            "confirm",
            Some("理由包含机密样例".to_string()),
            NOW + 1.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!err.contains("机密样例"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), before, "被拒时素材文件字节不变");
        let stored = material_store::read_material(temp.root(), &saved.material_id).unwrap();
        assert_eq!(stored.review_status, material_store::REVIEW_PENDING);
    }

    #[test]
    fn material_edit_gates_text_and_appends_edit_record() {
        let temp = TempRoot::new("edit");
        let clean = policy_with(&[], false);
        let saved = material_create_impl(temp.root(), &clean, user_draft("虚构编辑正文"), NOW)
            .unwrap();
        let path = material_file(temp.root(), &saved.material_id);
        let before = std::fs::read(&path).unwrap();

        let redline = policy_with(&["机密"], false);
        let err = material_edit_impl(
            temp.root(),
            &redline,
            &saved.material_id,
            material_store::MaterialPatch {
                content: Some("虚构正文包含机密样例".to_string()),
                ..Default::default()
            },
            NOW + 1.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), before);

        let edited = material_edit_impl(
            temp.root(),
            &clean,
            &saved.material_id,
            material_store::MaterialPatch {
                content: Some("虚构编辑后的正文。".to_string()),
                material_type: Some(material_store::TYPE_WORK_RESULT.to_string()),
                tags: Some(vec!["虚构标签".to_string()]),
                reason: Some("按用户要求整理".to_string()),
                ..Default::default()
            },
            NOW + 2.0,
        )
        .unwrap();
        assert_eq!(edited.content, "虚构编辑后的正文。");
        assert_eq!(edited.material_type, material_store::TYPE_WORK_RESULT);
        assert_eq!(edited.tags, vec!["虚构标签".to_string()]);
        let last = edited.review_records.last().unwrap();
        assert_eq!(last.action, material_store::ACTION_EDIT);
        assert_eq!(last.reason, "按用户要求整理");
    }

    #[test]
    fn material_get_returns_source_and_related_trace_payload() {
        let temp = TempRoot::new("trace");
        let record = worklog_records::create_record(
            temp.root(),
            &worklog_records::RecordDraft {
                title: Some("虚构来源记录".to_string()),
                body: "虚构工作事实：完成季度巡检，并形成三页记录。".to_string(),
                ..Default::default()
            },
            NOW,
            480,
        )
        .unwrap();
        let task = task_store::create_task(
            temp.root(),
            &task_store::TaskDraft {
                title: "虚构关联任务".to_string(),
                source_quote: Some("完成季度巡检".to_string()),
                source_record_id: Some(record.id.clone()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let project = project_store::create_project(
            temp.root(),
            &project_store::ProjectDraft {
                name: "虚构关联项目".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let vault_root = temp.root().join("synthetic-vault");
        std::fs::create_dir_all(vault_root.join("notes")).unwrap();
        std::fs::write(
            vault_root.join("notes").join("知识甲.md"),
            "知识甲正文：完成季度巡检属于可复用事实。",
        )
        .unwrap();

        let material = material_store::create_material(
            temp.root(),
            &material_store::MaterialDraft {
                title: "虚构巡检素材".to_string(),
                content: "虚构正文：季度巡检已完成。".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: record.id.clone(),
                source_quote: "完成季度巡检".to_string(),
                source_location: Some("记录第 2 段".to_string()),
                related_task_ids: vec![task.id.clone()],
                related_project_id: Some(project.project_id.clone()),
                related_knowledge_ids: vec!["notes/知识甲.md".to_string()],
                ..Default::default()
            },
            NOW,
        )
        .unwrap();

        let detail = material_get_impl(temp.root(), Some(&vault_root), &material.material_id).unwrap();
        assert_eq!(detail.material.material_id, material.material_id);
        assert_eq!(detail.source.source_type, material_store::SOURCE_WORK_RECORD);
        assert_eq!(detail.source.quote, "完成季度巡检");
        assert!(detail.source.quote_found, "引用必须能在来源记录正文里定位");
        assert_eq!(detail.source.location.as_deref(), Some("记录第 2 段"));
        assert!(detail.source.entity.found);
        assert_eq!(detail.source.entity.title.as_deref(), Some("虚构来源记录"));
        assert!(detail
            .source
            .entity
            .location
            .as_deref()
            .unwrap_or("")
            .contains("记录"));
        let excerpt = detail.source.excerpt.as_deref().unwrap_or("");
        assert!(excerpt.contains("完成季度巡检"), "{excerpt}");
        assert_eq!(detail.related_tasks.len(), 1);
        assert!(detail.related_tasks[0].found);
        assert_eq!(detail.related_tasks[0].title.as_deref(), Some("虚构关联任务"));
        assert_eq!(detail.related_project.as_ref().unwrap().title.as_deref(), Some("虚构关联项目"));
        assert_eq!(detail.related_knowledge.len(), 1);
        assert!(detail.related_knowledge[0].found);
        assert_eq!(detail.related_knowledge[0].location.as_deref(), Some("notes/知识甲.md"));
    }

    #[test]
    fn material_get_reports_missing_source_and_user_trace_truthfully() {
        let temp = TempRoot::new("trace-missing");
        let missing = material_store::create_material(
            temp.root(),
            &material_store::MaterialDraft {
                title: "虚构缺失来源素材".to_string(),
                content: "虚构正文。".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: "r-missing-km506".to_string(),
                source_quote: "虚构引用".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let detail = material_get_impl(temp.root(), None, &missing.material_id).unwrap();
        assert!(!detail.source.entity.found, "缺失来源必须如实报告");
        assert!(detail.source.entity.note.is_some());
        assert!(!detail.source.quote_found);

        let user = material_store::create_material(
            temp.root(),
            &material_store::MaterialDraft {
                title: "虚构用户素材".to_string(),
                content: "虚构用户收藏正文。".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_USER.to_string(),
                source_quote: "虚构用户收藏正文。".to_string(),
                ..Default::default()
            },
            NOW + 1.0,
        )
        .unwrap();
        let detail = material_get_impl(temp.root(), None, &user.material_id).unwrap();
        assert!(detail.source.entity.found);
        assert_eq!(detail.source.entity.title.as_deref(), Some("用户收藏"));
        assert!(detail.source.quote_found);
    }
}

#[cfg(test)]
mod knowledge_link_ipc_tests {
    //! OD-KM-507：知识关联 IPC 的闸门与写回路径。

    use super::*;
    use knowledge_link_store::LinkSubjectKind;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_100_000.0;
    const OFFSET: i32 = 480;

    struct TempRoot {
        base: PathBuf,
        work: PathBuf,
        vault: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-knowledge-link-ipc-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&base);
            let work = base.join("work");
            let vault = base.join("vault");
            std::fs::create_dir_all(&work).unwrap();
            std::fs::create_dir_all(vault.join("notes")).unwrap();
            Self { base, work, vault }
        }

        fn root(&self) -> &Path {
            &self.work
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn policy_with(redline: &[&str], mask: bool) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        let mode = if mask { "mask" } else { "off" };
        rules.global.privacy.id_card = mode.to_string();
        rules.global.privacy.phone = mode.to_string();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn record(root: &Path) -> worklog_records::WorkRecord {
        worklog_records::create_record(
            root,
            &worklog_records::RecordDraft {
                title: Some("虚构记录".to_string()),
                body: "虚构工作事实。".to_string(),
                ..Default::default()
            },
            NOW,
            OFFSET,
        )
        .unwrap()
    }

    fn vault_note(temp: &TempRoot) {
        std::fs::write(
            temp.vault.join("notes/alpha.md"),
            "---\ntitle: Alpha 项目复盘\n---\n\nAlpha 复盘提到了关键决策。\n",
        )
        .unwrap();
    }

    #[test]
    fn knowledge_link_suggest_gates_hints_before_staging() {
        let temp = TempRoot::new("hint-redline");
        let record = record(temp.root());
        let policy = policy_with(&["机密"], false);
        let err = knowledge_link_suggest_impl(
            temp.root(),
            &temp.vault,
            &policy,
            LinkSubjectKind::Record,
            &record.id,
            vec!["机密".to_string()],
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!knowledge_link_store::candidates_dir(temp.root()).exists());
    }

    #[test]
    fn knowledge_link_confirm_gates_reason_and_leaves_candidate_pending() {
        let temp = TempRoot::new("reason-redline");
        vault_note(&temp);
        let record = record(temp.root());
        let open_policy = policy_with(&[], false);
        let staged = knowledge_link_suggest_impl(
            temp.root(),
            &temp.vault,
            &open_policy,
            LinkSubjectKind::Record,
            &record.id,
            vec!["Alpha".to_string()],
            NOW,
        )
        .unwrap();
        assert_eq!(staged.staged.len(), 1);
        let candidate_id = staged.staged[0].id.clone();

        let policy = policy_with(&["机密"], false);
        let err = knowledge_link_confirm_impl(
            temp.root(),
            &policy,
            &candidate_id,
            "虚构机密关联依据",
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        let stored = worklog_records::read_record(temp.root(), &record.id).unwrap();
        assert!(stored.related_knowledge.is_empty());
        assert!(knowledge_link_store::candidates_dir(temp.root())
            .join(format!("{candidate_id}.json"))
            .exists(), "闸门拒绝后候选仍待处理");
    }
}

#[cfg(test)]
mod writing_collect_ipc_tests {
    //! OD-WT-605：收集命令的写入口径与选择命令的闸门。

    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_300_000.0;

    struct TempRoot {
        base: PathBuf,
        work: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-collect-ipc-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&base);
            let work = base.join("work");
            std::fs::create_dir_all(&work).unwrap();
            Self { base, work }
        }

        fn root(&self) -> &Path {
            &self.work
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn policy_with(redline: &[&str]) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn writing(root: &Path) -> writing_store::WritingTask {
        writing_store::create_writing_task(
            root,
            &writing_store::WritingTaskDraft {
                writing_id: Some("w-ipc".to_string()),
                title: "虚构月度总结".to_string(),
                document_type: writing_store::TYPE_MONTHLY_SUMMARY.to_string(),
                topic: "虚构主题".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap()
    }

    fn confirmed_material(root: &Path) -> String {
        let material = material_store::create_material(
            root,
            &material_store::MaterialDraft {
                material_id: Some("m-ipc".to_string()),
                title: "虚构素材".to_string(),
                content: "虚构正文".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: "src-ipc".to_string(),
                source_quote: "虚构引用".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_store::review_material(
            root,
            &material.material_id,
            material_store::ACTION_CONFIRM,
            None,
            NOW + 1.0,
        )
        .unwrap();
        material.material_id
    }

    #[test]
    fn collect_gates_writing_id_before_any_read() {
        let temp = TempRoot::new("collect-gate");
        let err = writing_collect_materials_impl(
            temp.root(),
            None,
            &policy_with(&["机密"]),
            writing_collect::CollectRequest {
                writing_id: "机密写作编号".to_string(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!writing_store::writing_dir(temp.root()).exists());
    }

    #[test]
    fn query_filters_and_hints_do_not_enter_red_line_gate() {
        let temp = TempRoot::new("filter-not-gated");
        let task = writing(temp.root());
        let response = writing_collect_materials_impl(
            temp.root(),
            None,
            &policy_with(&["机密"]),
            writing_collect::CollectRequest {
                writing_id: task.writing_id.clone(),
                filter: material_store::MaterialFilter {
                    date_from: Some("2025-01-01".to_string()),
                    ..Default::default()
                },
                knowledge_hints: vec!["机密主题提示".to_string()],
                include_pending: false,
            },
        )
        .unwrap();
        assert_eq!(response.writing_id, task.writing_id);
        assert_eq!(response.material_count, 0);
        assert!(response
            .knowledge_note
            .as_deref()
            .unwrap_or_default()
            .contains("知识库未配置"));
    }

    #[test]
    fn select_gates_note_and_leaves_selection_unchanged_on_red_line() {
        let temp = TempRoot::new("select-gate");
        let task = writing(temp.root());
        let material_id = confirmed_material(temp.root());
        let before = writing_store::read_writing_task(temp.root(), &task.writing_id).unwrap();
        let err = writing_select_materials_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_collect::SelectionRequest {
                writing_id: task.writing_id.clone(),
                adopt_material_ids: vec![material_id],
                note: Some("虚构机密说明".to_string()),
                ..Default::default()
            },
            NOW + 2.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        let after = writing_store::read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert!(after.selected_material_ids.is_empty());
        assert_eq!(after.status_records.len(), before.status_records.len());
    }
}

#[cfg(test)]
mod writing_outline_ipc_tests {
    //! OD-WT-606：生成/确认/读取命令的红线与写入顺序。

    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_500_000.0;

    struct TempRoot {
        base: PathBuf,
        work: PathBuf,
        config: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-outline-ipc-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&base);
            let work = base.join("work");
            let config = base.join("config");
            std::fs::create_dir_all(&work).unwrap();
            std::fs::create_dir_all(&config).unwrap();
            Self { base, work, config }
        }

        fn root(&self) -> &Path {
            &self.work
        }

        fn config(&self) -> &Path {
            &self.config
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn policy_with(redline: &[&str]) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn confirmed_material(root: &Path) -> String {
        let material = material_store::create_material(
            root,
            &material_store::MaterialDraft {
                material_id: Some("m-outline-ipc".to_string()),
                title: "虚构素材".to_string(),
                content: "虚构正文".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: "src-outline-ipc".to_string(),
                source_quote: "虚构引用".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_store::review_material(
            root,
            &material.material_id,
            material_store::ACTION_CONFIRM,
            None,
            NOW + 1.0,
        )
        .unwrap();
        material.material_id
    }

    fn collecting_task(root: &Path, material_id: String) -> writing_store::WritingTask {
        let task = writing_store::create_writing_task(
            root,
            &writing_store::WritingTaskDraft {
                writing_id: Some("w-outline-ipc".to_string()),
                title: "虚构年度总结".to_string(),
                document_type: writing_store::TYPE_ANNUAL_SUMMARY.to_string(),
                topic: "虚构主题".to_string(),
                selected_material_ids: vec![material_id],
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        writing_store::transition_writing_status(
            root,
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap()
    }

    fn outline(task: &writing_store::WritingTask) -> writing_outline::WritingOutline {
        writing_outline::WritingOutline {
            title: task.title.clone(),
            writing_purpose: "虚构写作目的".to_string(),
            core_points: vec!["虚构核心观点".to_string()],
            sections: vec![writing_outline::OutlineSection {
                no: 1,
                heading: "工作概况".to_string(),
                sub_headings: vec![],
                focus: "虚构写作重点".to_string(),
                material_ids: task.selected_material_ids.clone(),
                knowledge_ids: vec![],
                expected_words: 500,
            }],
        }
    }

    #[test]
    fn generate_gates_writing_id_before_any_read() {
        let temp = TempRoot::new("generate-gate");
        let err = writing_outline_generate_impl(
            temp.root(),
            temp.config(),
            &policy_with(&["机密"]),
            writing_outline::OutlineGenerateRequest {
                writing_id: "机密写作编号".to_string(),
            },
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!writing_store::writing_dir(temp.root()).exists());
    }

    #[test]
    fn confirm_gates_editable_outline_before_writing() {
        let temp = TempRoot::new("confirm-gate");
        let material_id = confirmed_material(temp.root());
        let task = collecting_task(temp.root(), material_id);
        let mut edited = outline(&task);
        edited.title = "虚构机密标题".to_string();
        let err = writing_outline_confirm_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_outline::OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: edited,
                note: Some("虚构确认说明".to_string()),
            },
            NOW + 3.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        let after = writing_store::read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(after.status, writing_store::STATUS_COLLECTING_MATERIALS);
        assert!(!document_store::outline_path(temp.root(), &task.writing_id)
            .unwrap()
            .exists());
    }

    #[test]
    fn get_gates_writing_id_before_read() {
        let temp = TempRoot::new("get-gate");
        let err = writing_outline_get_impl(
            temp.root(),
            &policy_with(&["机密"]),
            "机密写作编号",
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
    }
}

#[cfg(test)]
mod writing_section_ipc_tests {
    //! OD-WT-607：章节生成/确认/读取/修订命令的红线与写入顺序。

    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_600_000.0;

    struct TempRoot {
        base: PathBuf,
        work: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-section-ipc-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&base);
            let work = base.join("work");
            std::fs::create_dir_all(&work).unwrap();
            Self { base, work }
        }

        fn root(&self) -> &Path {
            &self.work
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn policy_with(redline: &[&str]) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn ready_task(root: &Path) -> writing_store::WritingTask {
        let material = material_store::create_material(
            root,
            &material_store::MaterialDraft {
                material_id: Some("m-section-ipc".to_string()),
                title: "虚构素材".to_string(),
                content: "虚构正文".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: "src-section-ipc".to_string(),
                source_quote: "虚构引用".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_store::review_material(
            root,
            &material.material_id,
            material_store::ACTION_CONFIRM,
            None,
            NOW + 1.0,
        )
        .unwrap();
        let task = writing_store::create_writing_task(
            root,
            &writing_store::WritingTaskDraft {
                writing_id: Some("w-section-ipc".to_string()),
                title: "虚构年度总结".to_string(),
                document_type: writing_store::TYPE_ANNUAL_SUMMARY.to_string(),
                topic: "虚构主题".to_string(),
                selected_material_ids: vec![material.material_id.clone()],
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let task = writing_store::transition_writing_status(
            root,
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap();
        writing_outline::confirm_outline(
            root,
            &writing_outline::OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: writing_outline::WritingOutline {
                    title: task.title.clone(),
                    writing_purpose: "虚构写作目的".to_string(),
                    core_points: vec!["虚构核心观点".to_string()],
                    sections: vec![writing_outline::OutlineSection {
                        no: 1,
                        heading: "工作概况".to_string(),
                        sub_headings: vec![],
                        focus: "虚构写作重点".to_string(),
                        material_ids: task.selected_material_ids.clone(),
                        knowledge_ids: vec![],
                        expected_words: 500,
                    }],
                },
                note: None,
            },
            NOW + 3.0,
        )
        .unwrap();
        writing_store::read_writing_task(root, &task.writing_id).unwrap()
    }

    fn draft(content: &str) -> writing_section::SectionDraft {
        writing_section::SectionDraft {
            section_no: 1,
            content: content.to_string(),
            citations: vec![writing_section::SectionCitation {
                source_kind: writing_records::CitationSourceKind::Material,
                source_id: "m-section-ipc".to_string(),
                quote: "虚构引用".to_string(),
            }],
            segments: vec![writing_section::SectionSegment {
                kind: "事实".to_string(),
                text: "虚构正文".to_string(),
                quote: Some("虚构引用".to_string()),
            }],
        }
    }

    #[test]
    fn generate_gates_writing_id_before_any_read() {
        let temp = TempRoot::new("generate-gate");
        let err = writing_section_generate_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_section::SectionGenerateRequest {
                writing_id: "机密写作编号".to_string(),
                section_no: 1,
            },
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!writing_store::writing_dir(temp.root()).exists());
    }

    #[test]
    fn confirm_gates_content_before_writing() {
        let temp = TempRoot::new("confirm-content-gate");
        let task = ready_task(temp.root());
        let err = writing_section_confirm_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_section::SectionConfirmRequest {
                writing_id: task.writing_id.clone(),
                draft: draft("虚构机密内容"),
                note: None,
            },
            NOW + 4.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!document_store::body_path(temp.root(), &task.writing_id)
            .unwrap()
            .exists());
    }

    #[test]
    fn confirm_gates_citation_and_segment_text_before_writing() {
        let temp = TempRoot::new("confirm-quote-gate");
        let task = ready_task(temp.root());
        let mut bad_citation = draft("虚构正常正文。");
        bad_citation.citations[0].quote = "机密引用".to_string();
        let err = writing_section_confirm_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_section::SectionConfirmRequest {
                writing_id: task.writing_id.clone(),
                draft: bad_citation,
                note: None,
            },
            NOW + 4.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");

        let mut bad_segment = draft("虚构正常正文。");
        bad_segment.segments[0].text = "机密标注文本".to_string();
        let err = writing_section_confirm_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_section::SectionConfirmRequest {
                writing_id: task.writing_id.clone(),
                draft: bad_segment,
                note: None,
            },
            NOW + 5.0,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
        assert!(!document_store::body_path(temp.root(), &task.writing_id)
            .unwrap()
            .exists());
    }

    #[test]
    fn revise_and_get_gate_user_text_before_read_or_prompt() {
        let temp = TempRoot::new("revise-gate");
        let task = ready_task(temp.root());
        writing_section_confirm_impl(
            temp.root(),
            &policy_with(&[]),
            writing_section::SectionConfirmRequest {
                writing_id: task.writing_id.clone(),
                draft: draft("虚构正文段落。"),
                note: None,
            },
            NOW + 4.0,
        )
        .unwrap();

        let err = writing_section_revise_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_section::SectionReviseRequest {
                writing_id: task.writing_id.clone(),
                section_no: 1,
                paragraph_no: None,
                instruction_kind: "精简".to_string(),
                instruction: "机密修订说明".to_string(),
            },
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");

        let err = writing_section_get_impl(
            temp.root(),
            &policy_with(&["机密"]),
            "机密写作编号",
            1,
        )
        .unwrap_err();
        assert!(err.contains("红线扫描"), "{err}");
    }
}

#[cfg(test)]
mod writing_review_ipc_tests {
    //! OD-WT-608：审阅/应用/版本命令的红线与写入顺序。

    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_600_000.0;

    struct TempRoot {
        base: PathBuf,
        work: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-review-ipc-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&base);
            let work = base.join("work");
            std::fs::create_dir_all(&work).unwrap();
            Self { base, work }
        }

        fn root(&self) -> &Path {
            &self.work
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn policy_with(redline: &[&str]) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn ready_task(root: &Path) -> writing_store::WritingTask {
        let material = material_store::create_material(
            root,
            &material_store::MaterialDraft {
                material_id: Some("m-review-ipc".to_string()),
                title: "虚构素材".to_string(),
                content: "虚构正文".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: "src-review-ipc".to_string(),
                source_quote: "虚构引用".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_store::review_material(
            root,
            &material.material_id,
            material_store::ACTION_CONFIRM,
            None,
            NOW + 1.0,
        )
        .unwrap();
        let task = writing_store::create_writing_task(
            root,
            &writing_store::WritingTaskDraft {
                writing_id: Some("w-review-ipc".to_string()),
                title: "虚构年度总结".to_string(),
                document_type: writing_store::TYPE_ANNUAL_SUMMARY.to_string(),
                topic: "虚构主题".to_string(),
                requirements: "只写已确认事实".to_string(),
                selected_material_ids: vec![material.material_id.clone()],
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let task = writing_store::transition_writing_status(
            root,
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap();
        writing_outline::confirm_outline(
            root,
            &writing_outline::OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: writing_outline::WritingOutline {
                    title: task.title.clone(),
                    writing_purpose: "虚构写作目的".to_string(),
                    core_points: vec!["虚构核心观点".to_string()],
                    sections: vec![writing_outline::OutlineSection {
                        no: 1,
                        heading: "工作概况".to_string(),
                        sub_headings: vec![],
                        focus: "虚构写作重点".to_string(),
                        material_ids: task.selected_material_ids.clone(),
                        knowledge_ids: vec![],
                        expected_words: 500,
                    }],
                },
                note: None,
            },
            NOW + 3.0,
        )
        .unwrap();
        writing_store::read_writing_task(root, &task.writing_id).unwrap()
    }

    fn draft(content: &str) -> writing_section::SectionDraft {
        writing_section::SectionDraft {
            section_no: 1,
            content: content.to_string(),
            citations: vec![writing_section::SectionCitation {
                source_kind: writing_records::CitationSourceKind::Material,
                source_id: "m-review-ipc".to_string(),
                quote: "虚构引用".to_string(),
            }],
            segments: vec![writing_section::SectionSegment {
                kind: "事实".to_string(),
                text: "虚构正文".to_string(),
                quote: Some("虚构引用".to_string()),
            }],
        }
    }

    fn finding(description: &str) -> writing_review::ReviewFinding {
        writing_review::ReviewFinding {
            finding_id: "F1".to_string(),
            dimension: writing_review::ReviewDimension::FactAccuracy,
            section_no: 1,
            paragraph_no: 1,
            description: description.to_string(),
            suggestion: "虚构修改建议".to_string(),
        }
    }

    #[test]
    fn review_gates_writing_id_and_instruction_before_read() {
        let temp = TempRoot::new("review-gate");
        let error = writing_review_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_review::ReviewRequest {
                writing_id: "机密写作编号".to_string(),
                instruction: String::new(),
            },
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");
        assert!(!writing_store::writing_dir(temp.root()).exists());

        let task = ready_task(temp.root());
        let error = writing_review_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_review::ReviewRequest {
                writing_id: task.writing_id.clone(),
                instruction: "机密审阅说明".to_string(),
            },
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");
    }

    #[test]
    fn apply_finding_gates_description_and_draft_before_writing() {
        let temp = TempRoot::new("apply-gate");
        let task = ready_task(temp.root());
        writing_section_confirm_impl(
            temp.root(),
            &policy_with(&[]),
            writing_section::SectionConfirmRequest {
                writing_id: task.writing_id.clone(),
                draft: draft("虚构正常正文。"),
                note: None,
            },
            NOW + 4.0,
        )
        .unwrap();
        let before = document_store::read_manifest(temp.root(), &task.writing_id)
            .unwrap()
            .current_version;

        let error = writing_review_apply_finding_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_review::ReviewApplyRequest {
                writing_id: task.writing_id.clone(),
                finding: finding("机密问题描述"),
                draft: draft("虚构修订正文。"),
                note: None,
            },
            NOW + 5.0,
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");

        let error = writing_review_apply_finding_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_review::ReviewApplyRequest {
                writing_id: task.writing_id.clone(),
                finding: finding("虚构问题描述"),
                draft: draft("虚构机密修订正文。"),
                note: None,
            },
            NOW + 6.0,
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");
        assert_eq!(
            document_store::read_manifest(temp.root(), &task.writing_id)
                .unwrap()
                .current_version,
            before
        );
        let body = document_store::read_body(temp.root(), &task.writing_id)
            .unwrap()
            .unwrap();
        assert!(!body.contains("虚构机密修订正文"), "{body}");
    }

    #[test]
    fn version_commands_gate_writing_id_before_read() {
        let temp = TempRoot::new("version-gate");
        let error = writing_version_list_impl(
            temp.root(),
            &policy_with(&["机密"]),
            "机密写作编号",
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");
        assert!(!writing_store::writing_dir(temp.root()).exists());

        let error = writing_version_restore_impl(
            temp.root(),
            &policy_with(&["机密"]),
            writing_review::VersionRestoreRequest {
                writing_id: "机密写作编号".to_string(),
                version: 1,
                confirmed: true,
            },
            NOW,
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");
    }
}

#[cfg(test)]
mod writing_factcheck_ipc_tests {
    //! OD-WT-609：事实核验运行/处理命令的红线、状态门禁与 603 落盘顺序。

    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_790_000_000.0;

    struct TempRoot {
        base: PathBuf,
        work: PathBuf,
        config: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-factcheck-ipc-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&base);
            let work = base.join("work");
            let config = base.join("config");
            std::fs::create_dir_all(&work).unwrap();
            std::fs::create_dir_all(&config).unwrap();
            Self { base, work, config }
        }

        fn root(&self) -> &Path {
            &self.work
        }

        fn config(&self) -> &Path {
            &self.config
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn policy_with(redline: &[&str]) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn ready_task(root: &Path, content: &str) -> writing_store::WritingTask {
        let material = material_store::create_material(
            root,
            &material_store::MaterialDraft {
                material_id: Some("m-factcheck-ipc".to_string()),
                title: "虚构核验素材".to_string(),
                content: "虚构正文".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: "src-factcheck-ipc".to_string(),
                source_quote: "2026年3月完成投资1200万元。".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_store::review_material(
            root,
            &material.material_id,
            material_store::ACTION_CONFIRM,
            None,
            NOW + 1.0,
        )
        .unwrap();
        let task = writing_store::create_writing_task(
            root,
            &writing_store::WritingTaskDraft {
                writing_id: Some("w-factcheck-ipc".to_string()),
                title: "虚构核验年度总结".to_string(),
                document_type: writing_store::TYPE_ANNUAL_SUMMARY.to_string(),
                topic: "虚构核验主题".to_string(),
                selected_material_ids: vec![material.material_id.clone()],
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let task = writing_store::transition_writing_status(
            root,
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap();
        writing_outline::confirm_outline(
            root,
            &writing_outline::OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: writing_outline::WritingOutline {
                    title: task.title.clone(),
                    writing_purpose: "虚构核验目的".to_string(),
                    core_points: vec!["虚构核验观点".to_string()],
                    sections: vec![writing_outline::OutlineSection {
                        no: 1,
                        heading: "工作概况".to_string(),
                        sub_headings: vec![],
                        focus: "虚构核验重点".to_string(),
                        material_ids: task.selected_material_ids.clone(),
                        knowledge_ids: vec![],
                        expected_words: 500,
                    }],
                },
                note: None,
            },
            NOW + 3.0,
        )
        .unwrap();
        writing_section::confirm_section(
            root,
            &writing_section::SectionConfirmRequest {
                writing_id: task.writing_id.clone(),
                draft: writing_section::SectionDraft {
                    section_no: 1,
                    content: content.to_string(),
                    citations: vec![],
                    segments: vec![],
                },
                note: None,
            },
            NOW + 4.0,
        )
        .unwrap();
        writing_store::read_writing_task(root, &task.writing_id).unwrap()
    }

    fn run(
        root: &Path,
        policy: &profile_rules::RulePolicy,
        request: fact_check::FactCheckRunRequest,
        now: f64,
        config: &Path,
    ) -> Result<fact_check::FactCheckRunResponse, String> {
        writing_factcheck_run_impl(
            root,
            policy,
            request,
            now,
            None,
            config,
            480,
        )
    }

    fn request(writing_id: &str) -> fact_check::FactCheckRunRequest {
        fact_check::FactCheckRunRequest {
            writing_id: writing_id.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn run_gates_writing_id_and_status_before_reading_body() {
        let temp = TempRoot::new("run-gate");
        let error = writing_factcheck_run_impl(
            temp.root(),
            &policy_with(&["机密"]),
            request("机密写作编号"),
            NOW,
            None,
            temp.config(),
            480,
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");
        assert!(!writing_store::writing_dir(temp.root()).exists());

        let task = writing_store::create_writing_task(
            temp.root(),
            &writing_store::WritingTaskDraft {
                writing_id: Some("w-factcheck-unready".to_string()),
                title: "虚构未就绪任务".to_string(),
                document_type: writing_store::TYPE_ANNUAL_SUMMARY.to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        let error = writing_factcheck_run_impl(
            temp.root(),
            &policy_with(&[]),
            request(&task.writing_id),
            NOW + 1.0,
            None,
            temp.config(),
            480,
        )
        .unwrap_err();
        assert!(error.contains("需求待整理"), "{error}");
        assert!(error.contains("只有"), "{error}");
    }

    #[test]
    fn run_appends_records_and_rerun_skips_existing_ids() {
        let temp = TempRoot::new("run-append");
        let content = "2026年3月完成投资1200万元。\n\n完成投资1500万元。\n\n完成培训480人次。\n\n总体来看，各项工作取得明显成效。";
        let task = ready_task(temp.root(), content);
        let response = run(
            temp.root(),
            &policy_with(&[]),
            request(&task.writing_id),
            NOW + 5.0,
            temp.config(),
        )
            .unwrap();
        assert!(response.appended >= 4, "{:?}", response.stats);
        assert_eq!(response.appended, response.records.len());
        assert_eq!(response.stats.direct_source, 2);
        assert_eq!(response.stats.source_conflict, 1);
        assert_eq!(response.stats.source_missing, 1);
        assert_eq!(response.stats.expressive_summary, 1);
        assert!(response.records.iter().any(|record| {
            record.conclusion == writing_records::CheckConclusion::DirectSource
        }));
        assert!(response
            .records
            .iter()
            .all(|record| record.resolution == writing_records::ResolutionStatus::Unresolved));
        let path = writing_records::factcheck_path(temp.root(), &task.writing_id).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw.lines().count(), response.appended);

        let again = run(
            temp.root(),
            &policy_with(&[]),
            request(&task.writing_id),
            NOW + 6.0,
            temp.config(),
        )
        .unwrap();
        assert_eq!(again.appended, 0);
        assert_eq!(again.skipped_existing, response.appended);
        let raw_again = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw_again.lines().count(), response.appended);
    }

    #[test]
    fn run_merges_analysis_annotations_and_outdated_knowledge() {
        let temp = TempRoot::new("run-merge");
        let content = "预计明年完成投资2000万元。\n\n2026年4月完成投资800万元。";
        let task = ready_task(temp.root(), content);
        let mut input = request(&task.writing_id);
        input.annotations = vec![fact_check::FactAnnotationInput {
            section_no: 1,
            paragraph_no: Some(1),
            kind: "判断".to_string(),
            text: "预计明年完成投资2000万元。".to_string(),
        }];
        input.knowledge_evidence = vec![fact_check::KnowledgeEvidenceInput {
            source_id: "notes/synthetic-609.md".to_string(),
            quote: "2026年4月完成投资800万元。".to_string(),
            outdated: true,
        }];
        let response = run(
            temp.root(),
            &policy_with(&[]),
            input,
            NOW + 5.0,
            temp.config(),
        )
        .unwrap();
        assert_eq!(response.stats.analysis_judgment, 1);
        assert_eq!(response.stats.source_outdated, 1);
        let analysis: Vec<_> = response
            .records
            .iter()
            .filter(|record| {
                record.conclusion == writing_records::CheckConclusion::AnalysisJudgment
            })
            .collect();
        assert_eq!(analysis.len(), 1);
        assert!(analysis[0].basis.is_empty());
        assert!(!response.records.iter().any(|record| {
            record.conclusion == writing_records::CheckConclusion::DirectSource
                && record.fact_text.as_deref().is_some_and(|text| text.contains("2000"))
        }));
    }

    #[test]
    fn resolve_updates_only_target_and_gates_note() {
        let temp = TempRoot::new("resolve");
        let task = ready_task(temp.root(), "2026年3月完成投资1200万元。");
        let response = run(
            temp.root(),
            &policy_with(&[]),
            request(&task.writing_id),
            NOW + 5.0,
            temp.config(),
        )
            .unwrap();
        assert!(response.records.len() >= 2);
        let target = response.records[0].record_id.clone();
        let resolved = writing_factcheck_resolve_impl(
            temp.root(),
            &policy_with(&[]),
            fact_check::FactCheckResolveRequest {
                writing_id: task.writing_id.clone(),
                record_id: target.clone(),
                resolution: "已接受".to_string(),
                note: Some("虚构处理备注".to_string()),
            },
        )
        .unwrap();
        assert_eq!(resolved.record.resolution, writing_records::ResolutionStatus::Accepted);
        assert_eq!(resolved.note.as_deref(), Some("虚构处理备注"));
        let listed = writing_records::list_fact_checks(temp.root(), &task.writing_id).unwrap();
        assert_eq!(
            listed
                .iter()
                .filter(|record| record.resolution == writing_records::ResolutionStatus::Accepted)
                .count(),
            1
        );
        assert_eq!(
            listed
                .iter()
                .filter(|record| record.resolution == writing_records::ResolutionStatus::Unresolved)
                .count(),
            listed.len() - 1
        );

        let error = writing_factcheck_resolve_impl(
            temp.root(),
            &policy_with(&["机密"]),
            fact_check::FactCheckResolveRequest {
                writing_id: task.writing_id.clone(),
                record_id: target,
                resolution: "已修改".to_string(),
                note: Some("机密备注".to_string()),
            },
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");
        let listed = writing_records::list_fact_checks(temp.root(), &task.writing_id).unwrap();
        assert_eq!(
            listed
                .iter()
                .filter(|record| record.resolution == writing_records::ResolutionStatus::Modified)
                .count(),
            0
        );
    }

    #[test]
    fn resolve_rejects_unresolved_and_missing_record() {
        let temp = TempRoot::new("resolve-invalid");
        let task = ready_task(temp.root(), "2026年3月完成投资1200万元。");
        let response = run(
            temp.root(),
            &policy_with(&[]),
            request(&task.writing_id),
            NOW + 5.0,
            temp.config(),
        )
            .unwrap();
        let record_id = response.records[0].record_id.clone();
        let error = writing_factcheck_resolve_impl(
            temp.root(),
            &policy_with(&[]),
            fact_check::FactCheckResolveRequest {
                writing_id: task.writing_id.clone(),
                record_id: record_id.clone(),
                resolution: "未处理".to_string(),
                note: None,
            },
        )
        .unwrap_err();
        assert!(error.contains("只能由用户改为"), "{error}");
        let error = writing_factcheck_resolve_impl(
            temp.root(),
            &policy_with(&[]),
            fact_check::FactCheckResolveRequest {
                writing_id: task.writing_id.clone(),
                record_id: "fact-does-not-exist".to_string(),
                resolution: "已接受".to_string(),
                note: None,
            },
        )
        .unwrap_err();
        assert!(error.contains("找不到核验记录"), "{error}");
    }
}

#[cfg(test)]
mod writing_archive_ipc_tests {
    //! OD-WT-614：归档门禁、回执字段、敏感拒绝、终态与知识回流范围。

    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const NOW: f64 = 1_780_700_000.0;
    const OFFSET: i32 = 480;

    struct TempRoot {
        base: PathBuf,
        work: PathBuf,
        vault: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "omnidesk-writing-archive-ipc-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&base);
            let work = base.join("work");
            let vault = base.join("vault");
            std::fs::create_dir_all(&work).unwrap();
            std::fs::create_dir_all(&vault).unwrap();
            Self { base, work, vault }
        }

        fn root(&self) -> &Path {
            &self.work
        }

        fn vault(&self) -> &Path {
            &self.vault
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn policy_with(redline: &[&str]) -> profile_rules::RulePolicy {
        let mut rules = profile_rules::parse_rules(profile_rules::DEFAULT_RULES_JSON)
            .expect("默认规则包必须可解析");
        rules.global.redline_keywords = redline.iter().map(|k| (*k).to_string()).collect();
        profile_rules::RulePolicy::from_rules(&rules)
    }

    fn clean_policy() -> profile_rules::RulePolicy {
        policy_with(&[])
    }

    fn create_task(root: &Path) -> writing_store::WritingTask {
        let material = material_store::create_material(
            root,
            &material_store::MaterialDraft {
                material_id: Some("m-archive-ipc".to_string()),
                title: "虚构归档素材".to_string(),
                content: "虚构素材正文".to_string(),
                material_type: material_store::TYPE_WORK_FACT.to_string(),
                source_type: material_store::SOURCE_WORK_RECORD.to_string(),
                source_id: "src-archive-ipc".to_string(),
                source_quote: "虚构素材引用".to_string(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        material_store::review_material(
            root,
            &material.material_id,
            material_store::ACTION_CONFIRM,
            None,
            NOW + 1.0,
        )
        .unwrap();
        let task = writing_store::create_writing_task(
            root,
            &writing_store::WritingTaskDraft {
                writing_id: Some("w-archive-ipc".to_string()),
                title: "虚构归档年度总结".to_string(),
                document_type: writing_store::TYPE_ANNUAL_SUMMARY.to_string(),
                topic: "虚构归档主题".to_string(),
                writing_period: "2026 年度".to_string(),
                selected_material_ids: vec![material.material_id.clone()],
                selected_knowledge_ids: vec!["k-archive-ipc".to_string()],
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
        writing_store::transition_writing_status(
            root,
            &task.writing_id,
            writing_store::STATUS_COLLECTING_MATERIALS,
            false,
            None,
            NOW + 2.0,
        )
        .unwrap()
    }

    fn outlined_task(root: &Path, sections: &[(&str, &str)]) -> writing_store::WritingTask {
        let task = create_task(root);
        writing_outline::confirm_outline(
            root,
            &writing_outline::OutlineConfirmRequest {
                writing_id: task.writing_id.clone(),
                outline: writing_outline::WritingOutline {
                    title: task.title.clone(),
                    writing_purpose: "虚构归档目的".to_string(),
                    core_points: vec!["虚构归档观点".to_string()],
                    sections: sections
                        .iter()
                        .enumerate()
                        .map(|(index, (heading, _))| writing_outline::OutlineSection {
                            no: index as u32 + 1,
                            heading: heading.to_string(),
                            sub_headings: vec![],
                            focus: "虚构章节重点".to_string(),
                            material_ids: task.selected_material_ids.clone(),
                            knowledge_ids: vec![],
                            expected_words: 300,
                        })
                        .collect(),
                },
                note: None,
            },
            NOW + 3.0,
        )
        .unwrap();
        writing_store::read_writing_task(root, &task.writing_id).unwrap()
    }

    fn finalized_task(root: &Path, sections: &[(&str, &str)]) -> writing_store::WritingTask {
        let task = outlined_task(root, sections);
        for (index, (_, content)) in sections.iter().enumerate() {
            writing_section::confirm_section(
                root,
                &writing_section::SectionConfirmRequest {
                    writing_id: task.writing_id.clone(),
                    draft: writing_section::SectionDraft {
                        section_no: index as u32 + 1,
                        content: content.to_string(),
                        citations: vec![],
                        segments: vec![],
                    },
                    note: None,
                },
                NOW + 4.0 + index as f64,
            )
            .unwrap();
        }
        let task = writing_store::read_writing_task(root, &task.writing_id).unwrap();
        assert_eq!(task.status, writing_store::STATUS_FACT_CHECKING, "{}", task.status);
        writing_store::transition_writing_status(
            root,
            &task.writing_id,
            writing_store::STATUS_PENDING_REVIEW,
            false,
            None,
            NOW + 8.0,
        )
        .unwrap();
        writing_status_transition_impl(
            root,
            &clean_policy(),
            WritingStatusTransitionRequest {
                writing_id: task.writing_id.clone(),
                to: writing_store::STATUS_FINALIZED.to_string(),
                reason: Some("用户定稿".to_string()),
            },
            NOW + 9.0,
        )
        .unwrap()
    }

    fn preview(root: &Path, writing_id: &str, preview: bool) -> Result<WritingArchiveOutcome, String> {
        writing_archive_impl(
            root,
            None,
            &clean_policy(),
            WritingArchiveRequest {
                writing_id: writing_id.to_string(),
                preview,
            },
            NOW + 20.0,
        )
    }

    fn archive(
        root: &Path,
        vault: &Path,
        policy: &profile_rules::RulePolicy,
        writing_id: &str,
        now: f64,
    ) -> Result<WritingArchiveOutcome, String> {
        writing_archive_impl(
            root,
            Some(vault),
            policy,
            WritingArchiveRequest {
                writing_id: writing_id.to_string(),
                preview: false,
            },
            now,
        )
    }

    fn add_factchecks(root: &Path, writing_id: &str) {
        writing_records::append_fact_check(
            root,
            writing_id,
            &writing_records::FactCheckDraft {
                record_id: None,
                section_no: 1,
                paragraph_no: 1,
                check_object: writing_records::CheckObject::Number,
                conclusion: writing_records::CheckConclusion::DirectSource,
                basis: vec![writing_records::FactBasis {
                    source_kind: writing_records::CitationSourceKind::Material,
                    source_id: "m-archive-ipc".to_string(),
                    quote: "虚构素材引用".to_string(),
                }],
                fact_text: Some("虚构数字".to_string()),
            },
            NOW + 10.0,
        )
        .unwrap();
        writing_records::append_fact_check(
            root,
            writing_id,
            &writing_records::FactCheckDraft {
                record_id: None,
                section_no: 1,
                paragraph_no: 2,
                check_object: writing_records::CheckObject::Time,
                conclusion: writing_records::CheckConclusion::SourceMissing,
                basis: vec![],
                fact_text: Some("虚构日期".to_string()),
            },
            NOW + 11.0,
        )
        .unwrap();
    }

    #[test]
    fn archive_rejects_before_finalized_and_leaves_vault_empty() {
        let temp = TempRoot::new("not-final");
        let task = outlined_task(temp.root(), &[("工作概况", "虚构正文。")]);
        let error = archive(temp.root(), temp.vault(), &clean_policy(), &task.writing_id, NOW)
            .unwrap_err();
        assert!(error.contains("已定稿"), "{error}");
        assert!(error.contains(&task.status), "{error}");
        assert!(!temp.vault().join("报告").exists());
        assert!(!document_store::read_manifest(temp.root(), &task.writing_id)
            .unwrap()
            .export_receipts
            .iter()
            .any(|receipt| receipt.format == "archive"));
    }

    #[test]
    fn archive_preview_before_outline_is_read_only_and_blocked() {
        let temp = TempRoot::new("preview-early");
        let task = create_task(temp.root());
        let before = preview(temp.root(), &task.writing_id, true).unwrap();
        assert!(!before.can_archive);
        assert!(before.blocked_reason.unwrap().contains("已定稿"));
        assert_eq!(before.association.title, task.title);
        assert!(before.association.knowledge_candidates.is_empty());
        assert!(!temp.vault().join("报告").exists());
        assert!(!document_store::read_manifest(temp.root(), &task.writing_id)
            .unwrap()
            .export_receipts
            .iter()
            .any(|receipt| receipt.format == "archive"));
    }

    #[test]
    fn archive_preview_soft_lists_exports_and_allows_without_export() {
        let temp = TempRoot::new("preview");
        let task = finalized_task(temp.root(), &[("工作概况", "虚构正文。")]);
        let before = preview(temp.root(), &task.writing_id, true).unwrap();
        assert!(before.can_archive);
        assert!(before.blocked_reason.is_none());
        assert!(before.association.export_receipts.is_empty());
        assert_eq!(before.association.status, writing_store::STATUS_FINALIZED);
        document_store::record_export_receipt(
            temp.root(),
            &task.writing_id,
            "md-formal",
            "/tmp/omnidesk-synthetic-export.md",
            NOW + 12.0,
        )
        .unwrap();
        let after = preview(temp.root(), &task.writing_id, true).unwrap();
        assert_eq!(after.association.export_receipts.len(), 1);
        assert_eq!(after.association.export_receipts[0].format, "md-formal");
        assert!(after.can_archive, "未导出或未全部导出不阻止归档");
    }

    #[test]
    fn archive_receipt_has_scene_ten_associations_and_terminal_status() {
        let temp = TempRoot::new("fields");
        let task = finalized_task(
            temp.root(),
            &[("工作概况", "虚构正文一。"), ("重点任务", "虚构正文二。")],
        );
        add_factchecks(temp.root(), &task.writing_id);
        document_store::record_export_receipt(
            temp.root(),
            &task.writing_id,
            "docx",
            "/tmp/omnidesk-synthetic-export.docx",
            NOW + 12.0,
        )
        .unwrap();
        let outcome = archive(temp.root(), temp.vault(), &clean_policy(), &task.writing_id, NOW + 21.0)
            .unwrap();
        assert!(!outcome.preview);
        assert_eq!(outcome.status_after, writing_store::STATUS_ARCHIVED);
        assert_eq!(outcome.association.writing_id, task.writing_id);
        assert!(!outcome.association.title.is_empty());
        assert_eq!(outcome.association.selected_material_ids, vec!["m-archive-ipc"]);
        assert_eq!(outcome.association.selected_knowledge_ids, vec!["k-archive-ipc"]);
        assert_eq!(outcome.association.factcheck_counts.len(), 8);
        assert_eq!(
            outcome
                .association
                .factcheck_counts
                .iter()
                .map(|count| count.count)
                .sum::<u64>(),
            2
        );
        assert!(outcome.association.version > 0);
        assert_eq!(outcome.association.export_receipts.len(), 1);
        let vault = outcome.vault.expect("归档必须返回知识库回执");
        assert!(vault.target_path.contains("/报告/文稿-"), "{}", vault.target_path);
        assert!(vault.target_path.ends_with(".md"));
        assert_eq!(outcome.manifest_receipt.unwrap().format, "archive");
        let stored = writing_store::read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(stored.status, writing_store::STATUS_ARCHIVED);
        let manifest = document_store::read_manifest(temp.root(), &task.writing_id).unwrap();
        assert_eq!(
            manifest
                .export_receipts
                .iter()
                .filter(|receipt| receipt.format == "archive")
                .count(),
            1
        );
        let entries = std::fs::read_dir(temp.vault().join("报告")).unwrap();
        for entry in entries {
            let name = entry.unwrap().file_name().to_string_lossy().to_string();
            assert!(name.ends_with(".md"), "归档目录只允许 Markdown：{name}");
        }
    }

    #[test]
    fn archive_rejects_sensitive_content_before_writing() {
        let temp = TempRoot::new("sensitive");
        let task = finalized_task(temp.root(), &[("工作概况", "机密虚构正文。")]);
        let error = archive(
            temp.root(),
            temp.vault(),
            &policy_with(&["机密"]),
            &task.writing_id,
            NOW + 21.0,
        )
        .unwrap_err();
        assert!(error.contains("敏感词扫描命中"), "{error}");
        assert!(error.contains("不回显"), "{error}");
        assert!(!temp.vault().join("报告").exists());
        let stored = writing_store::read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(stored.status, writing_store::STATUS_FINALIZED);
        assert!(!document_store::read_manifest(temp.root(), &task.writing_id)
            .unwrap()
            .export_receipts
            .iter()
            .any(|receipt| receipt.format == "archive"));
    }

    #[test]
    fn archive_second_attempt_is_rejected_without_duplicate_receipt() {
        let temp = TempRoot::new("terminal");
        let task = finalized_task(temp.root(), &[("工作概况", "虚构正文。")]);
        archive(temp.root(), temp.vault(), &clean_policy(), &task.writing_id, NOW + 21.0)
            .unwrap();
        let error = archive(temp.root(), temp.vault(), &clean_policy(), &task.writing_id, NOW + 22.0)
            .unwrap_err();
        assert!(error.contains("已归档"), "{error}");
        let stored = writing_store::read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(stored.status, writing_store::STATUS_ARCHIVED);
        let manifest = document_store::read_manifest(temp.root(), &task.writing_id).unwrap();
        assert_eq!(
            manifest
                .export_receipts
                .iter()
                .filter(|receipt| receipt.format == "archive")
                .count(),
            1
        );
        let after = preview(temp.root(), &task.writing_id, true).unwrap();
        assert!(after.association.already_archived);
        assert!(!after.can_archive);
    }

    fn feedback(
        root: &Path,
        vault: &Path,
        policy: &profile_rules::RulePolicy,
        writing_id: &str,
        selections: Vec<WritingKnowledgeSelection>,
    ) -> Result<WritingKnowledgeFeedbackOutcome, String> {
        writing_knowledge_feedback_impl(
            root,
            vault,
            policy,
            WritingKnowledgeFeedbackRequest {
                writing_id: writing_id.to_string(),
                selections,
            },
            NOW + 30.0,
            OFFSET,
        )
    }

    #[test]
    fn knowledge_feedback_admits_only_selected_content_and_does_not_publish() {
        let temp = TempRoot::new("feedback");
        let task = finalized_task(
            temp.root(),
            &[("已勾选", "已勾选虚构内容。"), ("未勾选", "未勾选虚构内容。")],
        );
        let outcome = feedback(
            temp.root(),
            temp.vault(),
            &clean_policy(),
            &task.writing_id,
            vec![WritingKnowledgeSelection {
                title: "已勾选".to_string(),
                content: "已勾选虚构内容。".to_string(),
                source_quote: "已勾选虚构内容。".to_string(),
            }],
        )
        .unwrap();
        assert!(outcome.ok);
        assert_eq!(outcome.selected_count, 1);
        assert_eq!(outcome.admitted.len(), 1);
        let raw = temp.vault().join("raw");
        let entries: Vec<PathBuf> = std::fs::read_dir(&raw)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(entries.len(), 1, "未勾选内容不得进入 raw 收件");
        let text = std::fs::read_to_string(&entries[0]).unwrap();
        assert!(text.contains("已勾选虚构内容"));
        assert!(!text.contains("未勾选虚构内容"), "{text}");
        assert!(!temp.vault().join("知识库/概念").read_dir().unwrap().any(|_| true));
        assert!(!temp.vault().join("知识库/方法").read_dir().unwrap().any(|_| true));
        let governance = knowledge_governance::list_records(
            temp.vault(),
            knowledge_governance::KIND_RAW_ADMISSIONS,
        )
        .unwrap();
        assert!(governance.iter().any(|record| record.action == "admit"));
        let stored = writing_store::read_writing_task(temp.root(), &task.writing_id).unwrap();
        assert_eq!(stored.status, writing_store::STATUS_FINALIZED, "回流不自动归档或定稿");
    }

    #[test]
    fn knowledge_feedback_rejects_sensitive_selection_before_any_vault_write() {
        let temp = TempRoot::new("feedback-sensitive");
        let task = finalized_task(temp.root(), &[("工作概况", "虚构正文。")]);
        let error = feedback(
            temp.root(),
            temp.vault(),
            &policy_with(&["机密"]),
            &task.writing_id,
            vec![WritingKnowledgeSelection {
                title: "机密回流".to_string(),
                content: "机密虚构内容。".to_string(),
                source_quote: String::new(),
            }],
        )
        .unwrap_err();
        assert!(error.contains("红线扫描"), "{error}");
        assert!(!temp.vault().join("raw").exists());
        assert!(!temp.vault().join("知识库").exists());
    }

    #[test]
    fn knowledge_feedback_empty_selection_submits_nothing() {
        let temp = TempRoot::new("feedback-empty");
        let task = finalized_task(temp.root(), &[("工作概况", "虚构正文。")]);
        let error = feedback(
            temp.root(),
            temp.vault(),
            &clean_policy(),
            &task.writing_id,
            vec![],
        )
        .unwrap_err();
        assert!(error.contains("没有勾选"), "{error}");
        assert!(!temp.vault().join("raw").exists());
    }
}
