//! Python Engine 子进程的启动与关闭管理（Step 7）。
//!
//! Rust 宿主生命周期：启动引擎 → 初始化 → 任务 → 关闭。
//! 引擎**不是**系统级常驻服务，只由本宿主按需拉起（契约 §1.1）。
//!
//! 边界：
//!   * 不修改、不复制 DSH Runtime；引擎只走 DSH 的公开 CLI（--profile acp）。
//!   * 隔离数据根由宿主**显式下发**，不从环境猜测。
//!   * 不读取、不打印、不落盘任何凭证。

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::engine_ipc::{EngineChannel, EngineRpcError};

/// 引擎启动参数（全部由环境变量或显式配置决定，绝不猜测）。
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Python 解释器（默认 python3）。
    pub python: String,
    /// 引擎包目录（含 omnidesk_engine/），作为子进程 cwd。
    pub engine_dir: PathBuf,
    /// DSH CLI 的绝对路径（bin.js）。
    pub dsh_cli: PathBuf,
    /// 隔离 DSH_HOME。
    pub dsh_home: PathBuf,
    /// 隔离 DSH_AGENTS_HOME。
    pub agents_home: PathBuf,
    /// 引擎自身状态目录（任务台账 / 事件游标 / 结果落盘）。
    pub engine_state_dir: PathBuf,
    /// 结果落盘目录。
    pub result_dir: PathBuf,
    /// 引擎子进程使用的隔离 Profile 名（默认 `acp`，即该子进程始终只讲 ACP）。
    /// 注意：这与 Web UI 的 Profile（`scripts/omnidesk-dev.sh` 的 `yvan-work`）是两件事。
    pub profile: String,
    /// 额外 Profile 补丁（绝对路径，可为空）。
    pub patch_files: Vec<PathBuf>,
}

impl EngineConfig {
    /// 从环境变量解析配置；缺失关键项时返回可读错误，而不是凭空造默认值。
    ///
    /// 支持的变量：
    ///   OMNIDESK_PYTHON            解释器，默认 python3
    ///   OMNIDESK_ENGINE_DIR        引擎包目录，默认 <repo>/engine
    ///   OMNIDESK_DSH_CLI           DSH CLI 绝对路径，默认 <repo>/runtime/dsh-runtime/.../bin.js
    ///   OMNIDESK_DSH_HOME          隔离 DSH_HOME，默认 $HOME/OmniDesk/runtime/dsh-home
    ///   OMNIDESK_AGENTS_HOME       隔离 agents，默认 $HOME/OmniDesk/runtime/agents-home
    ///   OMNIDESK_ENGINE_STATE_DIR  引擎状态目录，默认 <app root>/engine-state
    ///   OMNIDESK_DSH_PROFILE       Profile 名，默认 acp
    ///   OMNIDESK_DSH_PATCHES       逗号分隔的补丁绝对路径
    pub fn from_env(app_root: &Path) -> Result<Self, String> {
        let repo_root = Self::repo_root();

        let python = env_or("OMNIDESK_PYTHON", "python3");
        let engine_dir = match env_path("OMNIDESK_ENGINE_DIR") {
            Some(p) => p,
            None => repo_root
                .as_ref()
                .map(|r| r.join("engine"))
                .ok_or_else(|| "无法定位引擎目录：请设置 OMNIDESK_ENGINE_DIR".to_string())?,
        };
        let dsh_cli = match env_path("OMNIDESK_DSH_CLI") {
            Some(p) => p,
            None => repo_root
                .as_ref()
                .map(|r| {
                    r.join("runtime")
                        .join("dsh-runtime")
                        .join("node_modules")
                        .join("@deepseek-ai")
                        .join("dsh")
                        .join("lib")
                        .join("bin.js")
                })
                .ok_or_else(|| "无法定位 DSH CLI：请设置 OMNIDESK_DSH_CLI".to_string())?,
        };

        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "环境变量 HOME 缺失".to_string())?;
        let dsh_home = env_path("OMNIDESK_DSH_HOME")
            .unwrap_or_else(|| home.join("OmniDesk").join("runtime").join("dsh-home"));
        let agents_home = env_path("OMNIDESK_AGENTS_HOME")
            .unwrap_or_else(|| home.join("OmniDesk").join("runtime").join("agents-home"));
        let engine_state_dir = env_path("OMNIDESK_ENGINE_STATE_DIR")
            .unwrap_or_else(|| app_root.join("engine-state"));

        // 默认 Profile 必须与引擎子进程的协议对齐：本子进程始终只讲 ACP，而仓库内
        // profiles/yvan-work 打包的是 Web bundle（不产生 ACP 会话）。Web UI 的 Profile 由
        // scripts/omnidesk-dev.sh 独立控制，两者不是同一个用途。`acp` 是 DSH 内置模板，
        // 首次使用时会自动初始化（dsh-base + dsh-acp-app），无需仓库另存 Profile 模板。
        let profile = env_or("OMNIDESK_DSH_PROFILE", "acp");
        let mut patch_files = std::env::var("OMNIDESK_DSH_PATCHES")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        // OmniDesk 自己的模型路由覆盖（用户在设置里选定的 provider/model，只有非机密字段）。
        // 与引擎、端点分类器共用同一份，保证「界面显示的路由」＝「引擎实际走的路由」。
        if let Some(overlay) = crate::model_routes::overlay_for_engine(app_root) {
            if !patch_files.contains(&overlay) {
                patch_files.push(overlay);
            }
        }

        let config = Self {
            python,
            engine_dir,
            dsh_cli,
            dsh_home,
            agents_home,
            result_dir: engine_state_dir.join("results"),
            engine_state_dir,
            profile,
            patch_files,
        };
        config.validate()?;
        Ok(config)
    }

    /// 隔离边界校验：隔离根必须落在 OmniDesk 之下，且 CLI 入口存在。
    pub fn validate(&self) -> Result<(), String> {
        if !self.dsh_cli.is_absolute() {
            return Err(format!("DSH CLI 必须是绝对路径：{}", self.dsh_cli.display()));
        }
        if !self.dsh_cli.exists() {
            return Err(format!(
                "DSH CLI 入口不存在：{}（请先运行 scripts/bootstrap-runtime.sh）",
                self.dsh_cli.display()
            ));
        }
        if !self.engine_dir.is_absolute() {
            return Err(format!("引擎目录必须是绝对路径：{}", self.engine_dir.display()));
        }
        if !self.engine_dir.join("omnidesk_engine").is_dir() {
            return Err(format!(
                "引擎包目录缺少 omnidesk_engine/：{}",
                self.engine_dir.display()
            ));
        }
        for (label, path) in [("DSH_HOME", &self.dsh_home), ("DSH_AGENTS_HOME", &self.agents_home)] {
            let text = path.to_string_lossy().to_lowercase().replace('\\', "/");
            for forbidden in ["/.dsh", "/.agents", "deepseek-harness"] {
                if text.contains(forbidden) {
                    return Err(format!(
                        "隔离根 {} 落在被禁止路径（含 {:?}）：{}",
                        label,
                        forbidden,
                        path.display()
                    ));
                }
            }
        }
        Ok(())
    }

    /// 仓库根：优先环境变量，其次编译期 MANIFEST 的相对路径（仅开发构建可用）。
    fn repo_root() -> Option<PathBuf> {
        if let Some(p) = env_path("OMNIDESK_REPO_ROOT") {
            return Some(p);
        }
        let manifest = option_env!("CARGO_MANIFEST_DIR")?;
        // desktop/src-tauri -> 仓库根
        PathBuf::from(manifest)
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
    }

    /// 传给引擎子进程的环境变量（显式、最小集）。
    pub fn child_env(&self) -> Vec<(String, String)> {
        vec![
            ("OMNIDESK_DSH_HOME".to_string(), self.dsh_home.display().to_string()),
            ("OMNIDESK_AGENTS_HOME".to_string(), self.agents_home.display().to_string()),
            ("OMNIDESK_DSH_CLI".to_string(), self.dsh_cli.display().to_string()),
            ("OMNIDESK_DSH_PROFILE".to_string(), self.profile.clone()),
            (
                "OMNIDESK_ENGINE_STATE_DIR".to_string(),
                self.engine_state_dir.display().to_string(),
            ),
        ]
    }

    /// omni.initialize 的 params。
    pub fn initialize_params(&self) -> serde_json::Value {
        serde_json::json!({
            "protocol_version": "omnidesk-engine/1",
            "cwd": self.engine_dir.display().to_string(),
            "engine_state_dir": self.engine_state_dir.display().to_string(),
            "result_dir": self.result_dir.display().to_string(),
            "dsh": {
                "cli_entry": self.dsh_cli.display().to_string(),
                "profile": self.profile,
                "dsh_home": self.dsh_home.display().to_string(),
                "agents_home": self.agents_home.display().to_string(),
                "patch_files": self
                    .patch_files
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>(),
            },
        })
    }
}

fn env_or(key: &str, fallback: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => fallback.to_string(),
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => Some(PathBuf::from(v)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 本测试模块读写的环境变量（用于结束后还原，避免影响同进程的其它测试）。
    const OWNED_ENV_KEYS: [&str; 6] = [
        "OMNIDESK_DSH_PROFILE",
        "OMNIDESK_DSH_PATCHES",
        "OMNIDESK_ENGINE_DIR",
        "OMNIDESK_DSH_CLI",
        "OMNIDESK_DSH_HOME",
        "OMNIDESK_AGENTS_HOME",
    ];

    fn owned_env_snapshot() -> Vec<(&'static str, Option<std::ffi::OsString>)> {
        OWNED_ENV_KEYS
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect()
    }

    fn restore_env(snapshot: Vec<(&'static str, Option<std::ffi::OsString>)>) {
        for (key, value) in snapshot {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }

    /// 一次性隔离夹具：本测试只验证宿主配置解析，因此只要求引擎目录含 `omnidesk_engine/`、
    /// CLI 入口是存在的绝对路径（`validate()` 的既有前置条件），不启动任何真实进程。
    struct ConfigFixture {
        base: PathBuf,
    }

    impl ConfigFixture {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "omnidesk-engine-config-tests/{}-{}",
                tag,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(base.join("engine").join("omnidesk_engine"))
                .expect("创建引擎目录");
            std::fs::create_dir_all(base.join("dsh-home")).expect("创建隔离 DSH_HOME");
            std::fs::create_dir_all(base.join("agents-home")).expect("创建隔离 agents");
            std::fs::write(base.join("stub-dsh.js"), "// stub，仅用于通过存在性校验\n")
                .expect("写占位 CLI");
            Self { base }
        }

        fn apply_to_env(&self) {
            std::env::set_var("OMNIDESK_ENGINE_DIR", self.base.join("engine"));
            std::env::set_var("OMNIDESK_DSH_CLI", self.base.join("stub-dsh.js"));
            std::env::set_var("OMNIDESK_DSH_HOME", self.base.join("dsh-home"));
            std::env::set_var("OMNIDESK_AGENTS_HOME", self.base.join("agents-home"));
        }

        fn app_root(&self) -> PathBuf {
            self.base.join("app")
        }
    }

    impl Drop for ConfigFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    /// 从 `child_env()` 中取出某个键的值（找不到返回 `None`）。
    fn child_env_value(config: &EngineConfig, key: &str) -> Option<String> {
        config
            .child_env()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// 假引擎用配置（不启真实 DSH、不接 ACP）。
    fn fake_config() -> EngineConfig {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
        let app = std::env::temp_dir().join("omnidesk-host-tests").join("app");
        EngineConfig {
            python: std::env::var("OMNIDESK_PYTHON").unwrap_or_else(|_| "python3".to_string()),
            engine_dir: fake_engine_dir(),
            dsh_cli: PathBuf::from("/bin/true"),
            dsh_home: home.join("OmniDesk").join("runtime").join("dsh-home"),
            agents_home: home.join("OmniDesk").join("runtime").join("agents-home"),
            engine_state_dir: app.join("engine-state"),
            result_dir: app.join("engine-state").join("results"),
            profile: "acp".to_string(),
            patch_files: vec![],
        }
    }

    /// 生命周期回归（自集成测试迁入）：通道已挂载后可 initialize 并优雅关闭。
    #[test]
    fn supervisor_channel_initialize_and_stop() {
        let mut supervisor = supervisor_with_fake_channel(fake_config());
        assert!(supervisor.is_running());
        assert!(supervisor.pid().unwrap_or(0) > 0);

        let init = supervisor
            .initialize(Duration::from_secs(10))
            .expect("initialize 应成功");
        assert_eq!(init["engine"]["engine_kind"], "fake-engine");

        let (forced, exit_code) = supervisor.stop(Duration::from_secs(5)).expect("关闭应成功");
        assert!(!forced, "优雅关闭不应触发强杀");
        assert_eq!(exit_code, Some(0));
        assert!(!supervisor.is_running());
    }

    /// OD-BUILD-004 回归（**直接构造宿主配置**）：不设置 `OMNIDESK_DSH_PROFILE` 时，
    /// `EngineConfig::from_env()` 解析出的 Profile 必须为 `acp`，并且 `child_env()` 与
    /// `initialize_params()` 都把该值传给子进程——即宿主默认链本身就是 ACP，
    /// **不依赖任何调用方手工注入 acp**。
    ///
    /// 环境变量争用：本模块只有这一个测试改动 `OWNED_ENV_KEYS`，且不触碰被其它测试模块读取的
    /// `HOME` / `OMNIDESK_REPO_ROOT` / `OMNIDESK_LOCAL_DATA_ROOT`；因此可与并行测试共存。
    /// 测试结束后按快照还原，避免污染同进程的其它测试。
    #[test]
    fn engine_config_default_profile_chain_is_acp() {
        let snapshot = owned_env_snapshot();
        let fixture = ConfigFixture::new("default");
        fixture.apply_to_env();
        // 前置条件：宿主（或测试宿主的外层 shell）不得已经设过 Profile；
        // 这正是「宿主不设置该变量」的场景，与 R1 那种「脚本硬塞 acp」不是一回事。
        std::env::remove_var("OMNIDESK_DSH_PROFILE");
        std::env::remove_var("OMNIDESK_DSH_PATCHES");

        let default_config =
            EngineConfig::from_env(&fixture.app_root()).expect("默认配置应能解析通过");
        assert_eq!(
            default_config.profile, "acp",
            "不设 OMNIDESK_DSH_PROFILE 时默认 Profile 必须是 acp"
        );
        assert_eq!(
            child_env_value(&default_config, "OMNIDESK_DSH_PROFILE").as_deref(),
            Some("acp"),
            "child_env() 必须把默认 Profile 传给引擎子进程"
        );
        assert_eq!(
            default_config.initialize_params()["dsh"]["profile"],
            serde_json::json!("acp"),
            "initialize_params() 必须把默认 Profile 传给引擎子进程"
        );
        // 默认配置还要真的通过隔离边界校验（否则上面的断言只是解析结果，不代表可用配置）。
        assert!(
            default_config.validate().is_ok(),
            "默认配置应通过 validate()：{:?}",
            default_config.validate().err()
        );

        // 显式覆盖仍然生效，并且同样贯穿 child_env() 与 initialize_params()。
        std::env::set_var("OMNIDESK_DSH_PROFILE", "omnidesk-test");
        let overridden =
            EngineConfig::from_env(&fixture.app_root()).expect("覆盖配置应能解析通过");
        assert_eq!(overridden.profile, "omnidesk-test");
        assert_eq!(
            child_env_value(&overridden, "OMNIDESK_DSH_PROFILE").as_deref(),
            Some("omnidesk-test")
        );
        assert_eq!(
            overridden.initialize_params()["dsh"]["profile"],
            serde_json::json!("omnidesk-test")
        );

        // 纯空白覆盖不得被当作有效值，否则会把 Profile 名澄成空字符串。
        std::env::set_var("OMNIDESK_DSH_PROFILE", "   ");
        let blank = EngineConfig::from_env(&fixture.app_root()).expect("空白覆盖应回退默认值");
        assert_eq!(blank.profile, "acp", "空白覆盖应回退到 acp");
        assert_eq!(
            child_env_value(&blank, "OMNIDESK_DSH_PROFILE").as_deref(),
            Some("acp")
        );

        restore_env(snapshot);
    }
}

/// 引擎监督器：持有子进程通道，负责启动、健康检查与关闭。
pub struct EngineSupervisor {
    config: EngineConfig,
    channel: Option<EngineChannel>,
    /// “正在关闭”（OD-ARCH-024）：`begin_stop()` 置位，`end_stop()` 清除。
    ///
    /// 它和 `channel` 一起构成**唯一的**并发边界：判定“能不能发”和“摘除通道”都在
    /// 同一把 supervisor 互斥锁内完成。因此任务命令不可能出现“先看到未关闭、随后被
    /// 关闭抢先”的窗口——它要么在关闭前把请求发出去，要么在锁内看到本标志并立刻
    /// 得到“关闭中、未发送”的明确结论。
    stopping: bool,
}

impl EngineSupervisor {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            channel: None,
            stopping: false,
        }
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    pub fn pid(&self) -> Option<u32> {
        self.channel.as_ref().map(|c| c.pid())
    }

    pub fn is_running(&mut self) -> bool {
        self.channel.as_mut().map(|c| c.is_running()).unwrap_or(false)
    }

    /// 启动引擎子进程（幂等：已在运行则直接返回）。
    ///
    /// 必须持有 [`crate::admission::AdmissionGrant`]：该凭据只能由宿主在端点类别
    /// 与用户授权都核实通过后签发，因此不存在绕过准入的启动路径。
    pub fn start(&mut self, _grant: &crate::admission::AdmissionGrant) -> Result<(), String> {
        if self.stopping {
            // 关闭尚未收尾：此时启动新实例会与正在终止的旧子进程并存。
            return Err("引擎正在关闭，不能启动新实例；请等待关闭完成".to_string());
        }
        if self.is_running() {
            return Ok(());
        }
        self.config.validate()?;

        let args = vec!["-m".to_string(), "omnidesk_engine".to_string()];
        let channel = EngineChannel::spawn(
            &self.config.python,
            &args,
            &self.config.engine_dir,
            &self.config.child_env(),
        )
        .map_err(|e| {
            format!(
                "启动引擎子进程失败（{} {}，cwd={}）：{}",
                self.config.python,
                args.join(" "),
                self.config.engine_dir.display(),
                e
            )
        })?;

        self.channel = Some(channel);
        self.stopping = false;
        Ok(())
    }

    /// 仅供 crate 内单元测试：把已创建的假引擎通道挂到 supervisor 上。
    ///
    /// `#[cfg(test)]` 使生产构建中不存在该入口，因此它不是运行期旁路。
    #[cfg(test)]
    pub(crate) fn attach_channel_for_test(&mut self, channel: EngineChannel) {
        self.channel = Some(channel);
    }

    /// 事件泵专用句柄：通知接收端 + 子进程退出标志。
    ///
    /// 返回它只借用一次锁；此后事件泵**完全独立于本 supervisor**，不再需要
    /// 反复获取该互斥锁。若引擎尚未启动则返回 `None`。
    pub fn notification_handle(
        &self,
    ) -> Option<(Arc<Mutex<Receiver<serde_json::Value>>>, Arc<AtomicBool>)> {
        self.channel
            .as_ref()
            .map(|channel| (channel.notification_receiver(), channel.exited_flag()))
    }

    /// 借用通道执行一次请求。
    ///
    /// **调用方必须已持有 supervisor 锁**。两种拒绝语义要分清：
    ///   * `stopping`：宿主**已经决定关闭**并摘除了通道 —— 请求未发送（OD-ARCH-024）；
    ///   * 通道不存在：引擎本来就没启动/已经停止。
    /// 由于判定与摘除通道在同一把锁内完成，本函数看到的结论是一致的，不存在
    /// “判定通过、随后被关闭抢先”的中间态。
    pub fn with_channel<T>(
        &mut self,
        f: impl FnOnce(&mut EngineChannel) -> Result<T, EngineRpcError>,
    ) -> Result<T, EngineRpcError> {
        if self.stopping {
            return Err(EngineRpcError::shutting_down(
                "引擎正在关闭，本次请求未发送。请等待关闭完成后重试。",
            ));
        }
        match self.channel.as_mut() {
            Some(channel) => f(channel),
            None => Err(EngineRpcError::transport("引擎尚未启动")),
        }
    }

    /// 是否正在关闭（仅供调用方在同一把锁内做分支与测试断言）。
    pub fn is_stopping(&self) -> bool {
        self.stopping
    }

    /// 进入关闭（OD-ARCH-024）：置位“正在关闭”并**摘除**通道。
    ///
    /// 两件事必须在**同一把 supervisor 锁内原子完成**，这是本计划要求的并发边界：
    /// 任何任务命令要么在此之前持锁把请求发出去，要么在此之后看到 `stopping` 并
    /// 立刻得到“关闭中、未发送”。
    ///
    /// 返回被摘除的通道；调用方必须在**释放 supervisor 锁之后**再对它执行耗时的
    /// `finish_stop()`，否则状态/结果/取消会被排队到关闭结束。
    pub fn begin_stop(&mut self) -> Option<EngineChannel> {
        self.stopping = true;
        self.channel.take()
    }

    /// 关闭尝试结束（成功或失败都调用）：清除“正在关闭”。
    ///
    /// 此后命令看到的是“没有通道”，错误语义变成如实的“引擎未运行或已停止”。
    pub fn end_stop(&mut self) {
        self.stopping = false;
    }

    /// 对**已摘除**的通道执行协议级优雅关闭 + 进程收尾。
    ///
    /// 慢路径（协议关闭最长 `grace + 5s`，进程终止等待最长 `grace`），因此
    /// **不需要也不得持有 supervisor 锁**。
    pub fn finish_stop(
        mut channel: EngineChannel,
        grace: Duration,
    ) -> Result<(bool, Option<i32>), String> {
        let _ = channel.request(
            "omni.shutdown",
            serde_json::json!({ "mode": "graceful", "grace_period_seconds": grace.as_secs() }),
            grace + Duration::from_secs(5),
        );

        let forced = channel
            .terminate(grace)
            .map_err(|e| format!("关闭引擎子进程失败：{}", e))?;
        Ok((forced, channel.exit_code()))
    }

    /// 关闭引擎：优先优雅（关 stdin），超时后强杀。返回 (是否强杀, 退出码)。
    ///
    /// 便捷包装：`begin_stop()` → `finish_stop()` → `end_stop()`。**调用方在
    /// 持有 supervisor 锁时不要用本方法**（会把锁持有到关闭结束）——宿主正常路径
    /// 请用 `begin_stop()`/`finish_stop()` 两段式（见 lib.rs 的 `stop_engine`）。
    pub fn stop(&mut self, grace: Duration) -> Result<(bool, Option<i32>), String> {
        let detached = self.begin_stop();
        let outcome = match detached {
            None => Ok((false, None)),
            Some(channel) => Self::finish_stop(channel, grace),
        };
        self.end_stop();
        outcome
    }

    /// 初始化引擎（会真正拉起 DSH 连接）。
    pub fn initialize(&mut self, timeout: Duration) -> Result<serde_json::Value, EngineRpcError> {
        let params = self.config.initialize_params();
        self.with_channel(|channel| channel.request("omni.initialize", params, timeout))
    }

    /// 强制关闭（不等待）：仅用于测试与异常收尾。
    pub fn kill(&mut self) -> Result<(), String> {
        if let Some(mut channel) = self.channel.take() {
            channel
                .hard_kill()
                .map_err(|e| format!("强杀引擎子进程失败：{}", e))?;
        }
        Ok(())
    }

    pub fn stderr_tail(&self) -> Vec<String> {
        self.channel
            .as_ref()
            .map(|c| c.stderr_tail())
            .unwrap_or_default()
    }
}

impl Drop for EngineSupervisor {
    fn drop(&mut self) {
        if let Some(mut channel) = self.channel.take() {
            // 宿主退出时不留孤儿进程：先关 stdin，再给一个短超时，最后强杀。
            let _ = channel.terminate(Duration::from_secs(3));
        }
    }
}

/// 假引擎夹具路径（仅测试使用）。
#[cfg(test)]
fn fake_engine_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake_engine")
}

/// 仅测试：创建一个挂上假引擎通道的 supervisor。
///
/// 生产构建不存在 `attach_channel_for_test`，因此这不是运行期绕行；
/// 准入关闭期间 `start()` 不可用，生命周期测试只能经由该通道验证。
#[cfg(test)]
pub(crate) fn supervisor_with_fake_channel(config: EngineConfig) -> EngineSupervisor {
    supervisor_with_fake_channel_env(config, &[])
}

/// 仅测试：同上，但可给假引擎追加环境变量。
///
/// OD-ARCH-024 用 `OMNI_TEST_METHOD_LOG`（记录收到的方法，断言取消是否送达）与
/// `OMNI_TEST_SHUTDOWN_DELAY_MS`（制造可预测的关闭窗口）。
#[cfg(test)]
pub(crate) fn supervisor_with_fake_channel_env(
    config: EngineConfig,
    extra_env: &[(String, String)],
) -> EngineSupervisor {
    let python = std::env::var("OMNIDESK_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let mut env: Vec<(String, String)> = vec![("OMNI_TEST_MARKER".to_string(), "1".to_string())];
    env.extend(extra_env.iter().cloned());
    let channel = EngineChannel::spawn(
        &python,
        &["-m".to_string(), "omnidesk_engine".to_string()],
        &fake_engine_dir(),
        &env,
    )
    .expect("启动假引擎失败");
    let mut supervisor = EngineSupervisor::new(config);
    supervisor.attach_channel_for_test(channel);
    supervisor
}

/// 启动命令的字符串形式，便于日志与测试断言（不含凭证）。
pub fn describe_launch(config: &EngineConfig) -> String {
    format!(
        "{} -m omnidesk_engine（cwd={}，DSH_CLI={}，Profile={}）",
        config.python,
        config.engine_dir.display(),
        config.dsh_cli.display(),
        config.profile
    )
}
