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
    /// 使用的隔离 Profile 名。
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
    ///   OMNIDESK_DSH_PROFILE       Profile 名，默认 yvan-work
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

        let profile = env_or("OMNIDESK_DSH_PROFILE", "yvan-work");
        let patch_files = std::env::var("OMNIDESK_DSH_PATCHES")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

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

/// 引擎监督器：持有子进程通道，负责启动、健康检查与关闭。
pub struct EngineSupervisor {
    config: EngineConfig,
    channel: Option<EngineChannel>,
}

impl EngineSupervisor {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            channel: None,
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
    pub fn start(&mut self) -> Result<(), String> {
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
        Ok(())
    }

    /// 借用通道执行一次请求；若引擎未启动则返回 engine_unavailable。
    pub fn with_channel<T>(
        &mut self,
        f: impl FnOnce(&mut EngineChannel) -> Result<T, EngineRpcError>,
    ) -> Result<T, EngineRpcError> {
        match self.channel.as_mut() {
            Some(channel) => f(channel),
            None => Err(EngineRpcError::transport("引擎尚未启动")),
        }
    }

    /// 初始化引擎（会真正拉起 DSH 连接）。
    pub fn initialize(&mut self, timeout: Duration) -> Result<serde_json::Value, EngineRpcError> {
        let params = self.config.initialize_params();
        self.with_channel(|channel| channel.request("omni.initialize", params, timeout))
    }

    /// 关闭引擎：优先优雅（关 stdin），超时后强杀。返回 (是否强杀, 退出码)。
    pub fn stop(&mut self, grace: Duration) -> Result<(bool, Option<i32>), String> {
        let mut channel = match self.channel.take() {
            Some(c) => c,
            None => return Ok((false, None)),
        };

        // 先尝试协议级优雅关闭；失败不影响随后的进程收尾。
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
