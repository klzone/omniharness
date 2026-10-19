//! 桌面宿主层自动化测试（Step 16）。
//!
//! 三个测试组：
//!   1. Supervisor 启停（EngineSupervisor 启动/初始化/关闭，以及边界校验拒绝）
//!   2. IPC 请求-响应（EngineChannel 的 request/notification 往返）
//!   3. Engine 错误传播（结构化错误码与五属性不丢失）
//!
//! 全部使用 tests/fixtures/fake_engine 假引擎，**不接触**真实 DSH、真实模型与任何凭证。

use std::path::PathBuf;
use std::time::Duration;

use omnidesk_desktop_lib::engine_ipc::EngineChannel;
use omnidesk_desktop_lib::engine_supervisor::{EngineConfig, EngineSupervisor};
use omnidesk_desktop_lib::admission::{self, AdmissionFacts};
use omnidesk_desktop_lib::route_classifier::RouteFacts;
use omnidesk_desktop_lib::settings::ModelPermissions;
use serde_json::json;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake_engine")
}

fn python() -> String {
    std::env::var("OMNIDESK_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME"))
}

fn spawn_fake() -> EngineChannel {
    EngineChannel::spawn(
        &python(),
        &["-m".to_string(), "omnidesk_engine".to_string()],
        &fixture_dir(),
        &[("OMNI_TEST_MARKER".to_string(), "1".to_string())],
    )
    .expect("启动假引擎失败")
}

/// 基础配置：engine_dir 指向假引擎，dsh_cli 用存在的绝对文件（本组只验证宿主层行为）。
fn base_config() -> EngineConfig {
    let app_root = std::env::temp_dir().join("omnidesk-host-tests").join("app");
    EngineConfig {
        python: python(),
        engine_dir: fixture_dir(),
        dsh_cli: PathBuf::from("/bin/true"),
        dsh_home: home().join("OmniDesk").join("runtime").join("dsh-home"),
        agents_home: home().join("OmniDesk").join("runtime").join("agents-home"),
        engine_state_dir: app_root.join("engine-state"),
        result_dir: app_root.join("engine-state").join("results"),
        // 这里只是假引擎的占位配置（不起真实 DSH、不接 ACP），对行为无影响。
        // 真实默认值为 `acp`，见 engine_supervisor.rs 默认值及 OD-BUILD-003 回归场景 R1/R1b。
        profile: "acp".to_string(),
        patch_files: vec![],
    }
}

// ---------------------------------------------------------------------------
// 第 2 组：IPC 请求-响应
// ---------------------------------------------------------------------------

#[test]
fn ipc_request_response_roundtrip() {
    let mut channel = spawn_fake();
    assert!(channel.pid() > 0, "子进程应有有效 pid");

    let init = channel
        .request("omni.initialize", json!({}), Duration::from_secs(10))
        .expect("initialize 应成功");
    assert_eq!(init["protocol_version"], "omnidesk-engine/1");
    assert_eq!(init["engine"]["engine_kind"], "fake-engine");

    let echo = channel
        .request("omni.echo", json!({"x": 1, "y": "z"}), Duration::from_secs(10))
        .expect("echo 应成功");
    assert_eq!(echo["echo"]["x"], 1);
    assert_eq!(echo["echo"]["y"], "z");

    let forced = channel.terminate(Duration::from_secs(3)).expect("terminate");
    assert!(!forced, "假引擎应能优雅退出");
}

#[test]
fn ipc_receives_notifications() {
    let mut channel = spawn_fake();
    channel
        .request("omni.initialize", json!({}), Duration::from_secs(10))
        .unwrap();
    channel
        .request("omni.submit_task", json!({"prompt_text": "合成"}), Duration::from_secs(10))
        .unwrap();

    let note = channel
        .recv_notification_timeout(Duration::from_secs(5))
        .expect("应收到 omni.event 通知");
    assert_eq!(note["method"], "omni.event");
    assert_eq!(note["params"]["type"], "task_started");
    assert_eq!(note["params"]["omni_task_id"], "omni-test-task");

    let forced = channel.terminate(Duration::from_secs(3)).expect("terminate");
    assert!(!forced);
}

#[test]
fn ipc_timeout_is_reported_as_timeout_not_success() {
    let mut channel = spawn_fake();
    let err = channel
        .request("omni.slow", json!({"seconds": 3}), Duration::from_millis(300))
        .expect_err("慢响应应触发超时错误");
    assert_eq!(err.engine_error_code(), "timeout");
    assert_eq!(err.attribute("may_lose_output").as_deref(), Some("yes"));
    let _ = channel.terminate(Duration::from_secs(3));
}

#[test]
fn ipc_request_after_exit_fails_cleanly() {
    let mut channel = spawn_fake();
    let _ = channel.request("omni.shutdown", json!({}), Duration::from_secs(10));
    let code = channel.wait_exit(Duration::from_secs(5));
    assert_eq!(code, Some(0), "假引擎收到 shutdown 后应正常退出");

    let err = channel
        .request("omni.echo", json!({}), Duration::from_secs(3))
        .expect_err("进程退出后请求应失败");
    assert_eq!(err.engine_error_code(), "engine_unavailable");
}

// ---------------------------------------------------------------------------
// 第 3 组：Engine 错误传播
// ---------------------------------------------------------------------------

#[test]
fn ipc_propagates_engine_error_with_attributes() {
    let mut channel = spawn_fake();
    channel
        .request("omni.initialize", json!({}), Duration::from_secs(10))
        .unwrap();

    let err = channel
        .request("omni.fail", json!({}), Duration::from_secs(10))
        .expect_err("应返回结构化错误");
    assert_eq!(err.code, -32000);
    assert_eq!(err.engine_error_code(), "task_failed");
    assert_eq!(err.attribute("needs_human").as_deref(), Some("yes"));
    assert_eq!(err.attribute("may_duplicate_execution").as_deref(), Some("yes"));
    assert_eq!(err.attribute("may_lose_output").as_deref(), Some("no"));
    assert_eq!(err.task_id().as_deref(), Some("omni-test-task"));

    let _ = channel.terminate(Duration::from_secs(3));
}

#[test]
fn ipc_unknown_method_returns_jsonrpc_error() {
    let mut channel = spawn_fake();
    let err = channel
        .request("omni.nope", json!({}), Duration::from_secs(10))
        .expect_err("未知方法应返回 JSON-RPC 错误");
    assert_eq!(err.code, -32601);
    let _ = channel.terminate(Duration::from_secs(3));
}

// ---------------------------------------------------------------------------
// 第 1 组：Supervisor 启停与边界校验
// ---------------------------------------------------------------------------

#[test]
fn admission_refuses_before_any_process_state_exists() {
    // 准入判定只依赖端点事实与用户权限，不需要先校验/启动引擎。
    // 这里用坏配置构造监督器，验证拒绝路径不会创建目录或子进程。
    let base = std::env::temp_dir().join(format!(
        "omnidesk-admission-{}-{}",
        std::process::id(),
        line!()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let mut config = base_config();
    config.engine_dir = base.join("missing-engine");
    config.dsh_cli = base.join("missing-cli.js");
    config.dsh_home = base.join("dsh-home");
    config.agents_home = base.join("agents-home");
    config.engine_state_dir = base.join("app").join("engine-state");
    config.result_dir = config.engine_state_dir.join("results");

    let mut supervisor = EngineSupervisor::new(config);
    let route = RouteFacts {
        provider: "synthetic".to_string(),
        model: "synthetic".to_string(),
        endpoint: Some("http://127.0.0.1:9/v1".to_string()),
        endpoint_host: Some("127.0.0.1:9".to_string()),
        category: "local".to_string(),
        category_reason: "回环地址".to_string(),
        basis: vec!["provider=synthetic".to_string()],
        leaves_device: Some(false),
    };
    let denied = AdmissionFacts { route: route.clone(), permissions: ModelPermissions::default() };
    let err = admission::require_allowed(&denied, admission::SUBMIT_REFUSAL_MESSAGE)
        .expect_err("默认权限下必须拒绝");
    assert_eq!(err["engine_error_code"], admission::REFUSAL_CODE);
    assert_eq!(supervisor.pid(), None, "拒绝后不得存在子进程");
    assert!(!supervisor.is_running());
    assert!(!base.exists(), "拒绝路径不得创建任何目录：{}", base.display());

    let allowed = AdmissionFacts {
        route,
        permissions: ModelPermissions { local: true, private: false, public: false },
    };
    assert!(admission::require_allowed(&allowed, admission::SUBMIT_REFUSAL_MESSAGE).is_ok());

    let mut unknown = allowed.clone();
    unknown.route.category = "unknown".to_string();
    unknown.permissions = ModelPermissions { local: true, private: true, public: true };
    let err = admission::require_allowed(&unknown, admission::SUBMIT_REFUSAL_MESSAGE)
        .expect_err("未知类别必须拒绝");
    assert_eq!(err["engine_error_code"], admission::UNKNOWN_ENDPOINT_CODE);
}

#[test]
fn supervisor_refuses_missing_dsh_cli() {
    let mut config = base_config();
    config.dsh_cli = PathBuf::from("/nonexistent/dsh/bin.js");
    let err = config.validate().expect_err("缺失 CLI 应被拒绝");
    assert!(err.contains("DSH CLI 入口不存在"), "实际错误：{}", err);
}

#[test]
fn supervisor_refuses_stock_dsh_home() {
    let mut config = base_config();
    config.dsh_home = home().join(".dsh");
    let err = config.validate().expect_err("不得使用 ~/.dsh 作为隔离根");
    assert!(err.contains("/.dsh"), "实际错误：{}", err);
}

#[test]
fn supervisor_refuses_stock_agents_home() {
    let mut config = base_config();
    config.agents_home = home().join(".agents");
    let err = config.validate().expect_err("不得使用 ~/.agents 作为隔离根");
    assert!(err.contains("/.agents"), "实际错误：{}", err);
}

#[test]
fn supervisor_refuses_non_absolute_cli() {
    let mut config = base_config();
    config.dsh_cli = PathBuf::from("relative/bin.js");
    let err = config.validate().expect_err("CLI 必须是绝对路径");
    assert!(err.contains("绝对路径"), "实际错误：{}", err);
}

#[test]
fn supervisor_start_fails_with_clear_error_when_engine_dir_missing() {
    let mut config = base_config();
    config.engine_dir = PathBuf::from("/nonexistent/engine");
    let err = config.validate().expect_err("缺少 omnidesk_engine/ 应被拒绝");
    assert!(err.contains("omnidesk_engine"), "实际错误：{}", err);
}

#[test]
fn supervisor_reports_engine_unavailable_before_start() {
    let mut supervisor = EngineSupervisor::new(base_config());
    let err = supervisor
        .with_channel(|channel| channel.request("omni.echo", json!({}), Duration::from_secs(1)))
        .expect_err("未启动时应返回通道错误");
    assert_eq!(err.engine_error_code(), "engine_unavailable");
}

// 说明：原「supervisor 生命周期」与「事件泵不饿死宿主锁」两项测试需要挂载假引擎通道，
// 而该入口是 `#[cfg(test)]` 的 crate 内部接口，集成测试无法也不应访问。
// 两项已迁入 crate 单元测试（engine_supervisor.rs / engine_bridge.rs）。
