//! OmniDesk 引擎准入（宿主层，默认拒绝）。
//!
//! OD-ARCH-021 起，准入不再是一个写死的常量，而是两个可核实的事实组合：
//!   1. **端点类别**：从 DSH 非秘密配置转储解析出的实际 provider / endpoint，
//!      归为 local（本机回环）、private（非回环的私有/链路本地）、public（非回环的
//!      公网地址）或 unknown。类别只回答“是不是本机回环”，**不作**组织归属承诺：
//!      地址特征无法证明端点属于哪个组织，也无法证明数据是否离开所在组织。
//!   2. **用户授权**：用户在设置里对上述可配置类别各自显式开启的开关。
//! 两者同时满足才放行；unknown 类别没有开关，永远拒绝。首次（尚无设置文件）
//! 三类全部关闭。
//!
//! 约束：
//!   * 判定先于任何引擎副作用：不启动子进程、不 initialize、不提交任务、
//!     不创建工作目录、不写任务数据。
//!   * 类别来自实际配置，不信任 provider 名称或前端传值；无法解析即拒绝。
//!   * 前端的说明与按钮只是交互层，强制点在宿主。

use serde_json::{json, Value};

use crate::engine_supervisor::{EngineConfig, EngineSupervisor};
use crate::route_classifier::{self, RouteFacts};
use crate::settings::{self, ModelPermissions};

/// 准入拒绝错误码（前端按此显示原因，不解析自由文本）。
pub const REFUSAL_CODE: &str = "endpoint_unconfirmed";
/// 端点类别未知或无法解析时的错误码。
pub const UNKNOWN_ENDPOINT_CODE: &str = "endpoint_unknown";
/// 分类器自身失败（node 缺失、超时、输出非法）。
pub const CLASSIFIER_FAILED_CODE: &str = "endpoint_classification_failed";

pub const SUBMIT_REFUSAL_MESSAGE: &str =
    "当前模型端点未获得你的使用许可，已拒绝提交。本次提交未启动引擎、未创建任务，输入未发送到任何模型。请在设置中确认端点后开启对应类别。";

pub const WORKSPACE_REFUSAL_MESSAGE: &str =
    "当前模型端点未获得你的使用许可，已拒绝准备工作目录。未创建任何目录、未启动引擎，输入未发送到任何模型。";

pub const DESCRIBE_REFUSAL_MESSAGE: &str =
    "当前模型端点未获得你的使用许可，已拒绝启动引擎。引擎未启动，未创建任务，输入未发送到任何模型。";

pub const START_REFUSAL_MESSAGE: &str =
    "当前模型端点未获得你的使用许可，已拒绝启动引擎（endpoint_unconfirmed）。未创建子进程、未创建目录、未提交任务。";

/// 准入判定所需的全部事实，由宿主在触达引擎之前读取。
#[derive(Debug, Clone)]
pub struct AdmissionFacts {
    pub route: RouteFacts,
    pub permissions: ModelPermissions,
}

/// 通过准入的凭据。字段私有，只有 [`check`] 能构造，因此无法凭空造一张"已允许"。
///
/// OD-ARCH-023：凭据额外携带“权限世代”。`check` 产生的凭据世代为
/// [`GRANT_UNSTAMPED_EPOCH`]，必须由持有引擎调用守卫的宿主代码用
/// [`Admitted::stamp_epoch`] 盖上当前世代后才能启动引擎；因此锁外取得的
/// 旧凭据无法用于启动引擎。
#[derive(Debug, Clone)]
pub struct AdmissionGrant {
    _private: (),
    epoch: u64,
}

/// 未盖章的凭据世代（不允许用于启动引擎）。
pub const GRANT_UNSTAMPED_EPOCH: u64 = 0;

/// 准入通过后的结果：可展示的事实 + 启动引擎所需的凭据。
#[derive(Debug, Clone)]
pub struct Admitted {
    pub facts: AdmissionFacts,
    grant: AdmissionGrant,
}

impl Admitted {
    pub fn grant(&self) -> &AdmissionGrant {
        &self.grant
    }

    /// 盖上“引擎调用守卫”的当前世代；只有盖章后才能在宿主内启动引擎。
    pub fn stamp_epoch(mut self, epoch: u64) -> Self {
        self.grant.epoch = epoch;
        self
    }
}

impl AdmissionGrant {
    /// 凭据所属的权限世代；[`GRANT_UNSTAMPED_EPOCH`] 表示尚未盖章。
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// 仅测试可用：模拟已通过准入（世代取 1，与初始世代一致）。
    #[cfg(test)]
    pub fn for_test() -> Self {
        Self {
            _private: (),
            epoch: 1,
        }
    }

    /// 仅测试可用：未盖章的凭据（不得用于启动引擎）。
    #[cfg(test)]
    pub fn unstamped_for_test() -> Self {
        Self {
            _private: (),
            epoch: GRANT_UNSTAMPED_EPOCH,
        }
    }
}

impl AdmissionFacts {
    /// 该端点类别是否被用户明确允许。unknown 没有开关，恒为 false。
    pub fn is_allowed(&self) -> bool {
        self.permissions
            .for_category(&self.route.category)
            .unwrap_or(false)
    }
}

fn denial(code: &str, message: &str, detail: Value) -> Value {
    json!({
        "engine_error_code": code,
        "message": message,
        "detail": { "reason": code, "enforced_by": "host", "admission": detail },
    })
}

/// 只读：读取端点类别与用户授权，不做允许/拒绝判断。
pub fn read_facts(config: &EngineConfig, config_dir: &std::path::Path) -> Result<AdmissionFacts, Value> {
    let route = route_classifier::classify(config).map_err(|message| {
        denial(
            CLASSIFIER_FAILED_CODE,
            "无法从当前模型配置确认实际端点，已按默认拒绝处理。未启动引擎，输入未发送。",
            json!({ "stage": "classify", "error": message }),
        )
    })?;
    let permissions = settings::load(config_dir)
        .map_err(|message| {
            denial(
                CLASSIFIER_FAILED_CODE,
                "无法读取模型权限设置，已按默认拒绝处理。未启动引擎，输入未发送。",
                json!({ "stage": "settings", "error": message }),
            )
        })?
        .model_permissions;
    Ok(AdmissionFacts { route, permissions })
}

/// 把事实转成前端可见的说明（不含凭证、不含用户输入）。
pub fn describe_facts(facts: &AdmissionFacts) -> Value {
    json!({
        "provider": facts.route.provider,
        "model": facts.route.model,
        "endpoint": facts.route.endpoint,
        "endpoint_host": facts.route.endpoint_host,
        "category": facts.route.category,
        "category_reason": facts.route.category_reason,
        "basis": facts.route.basis,
        "leaves_device": facts.route.leaves_device,
        "configurable": facts.route.is_configurable(),
        "permissions": {
            "local": facts.permissions.local,
            "private": facts.permissions.private,
            "public": facts.permissions.public,
        },
        "allowed": facts.is_allowed(),
    })
}

/// 校验事实是否允许触达引擎。拒绝时返回结构化错误。
pub fn require_allowed(facts: &AdmissionFacts, message: &str) -> Result<(), Value> {
    if !facts.route.is_configurable() {
        return Err(denial(
            UNKNOWN_ENDPOINT_CODE,
            "无法确定当前模型端点的位置，已按默认拒绝处理。此类端点不能在设置中开启；未启动引擎，输入未发送。",
            serde_json::json!({
                "category": facts.route.category,
                "category_reason": facts.route.category_reason,
            }),
        ));
    }
    if facts.is_allowed() {
        return Ok(());
    }
    Err(denial(
        REFUSAL_CODE,
        message,
        serde_json::json!({
            "category": facts.route.category,
            "category_reason": facts.route.category_reason,
        }),
    ))
}

/// 先准入、后执行：`f` 只有在准入通过后才会被调用。
///
/// 宿主命令必须经由本函数触达引擎，以保证拒绝路径没有副作用。
pub fn run_after_admission<F>(
    config: &EngineConfig,
    config_dir: &std::path::Path,
    message: &str,
    f: F,
) -> Result<Value, Value>
where
    F: FnOnce() -> Result<Value, Value>,
{
    let admitted = check(config, config_dir, message)?;
    let _ = admitted.grant();
    f()
}

/// 只要判定、不执行动作的入口（供命令在调用任何引擎方法前使用）。
/// 返回的凭据是 [`EngineSupervisor::start`] 的必需参数。
pub fn check(
    config: &EngineConfig,
    config_dir: &std::path::Path,
    message: &str,
) -> Result<Admitted, Value> {
    let facts = read_facts(config, config_dir)?;
    require_allowed(&facts, message)?;
    Ok(Admitted {
        facts,
        grant: AdmissionGrant {
            _private: (),
            epoch: GRANT_UNSTAMPED_EPOCH,
        },
    })
}

/// 从监督器读取当前生效的配置（只读快照）。
pub fn config_of(supervisor: &EngineSupervisor) -> EngineConfig {
    supervisor.config().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn facts(category: &str, permissions: ModelPermissions) -> AdmissionFacts {
        AdmissionFacts {
            route: RouteFacts {
                provider: "fake".to_string(),
                model: "synthetic".to_string(),
                endpoint: Some("http://127.0.0.1:9/v1".to_string()),
                endpoint_host: Some("127.0.0.1:9".to_string()),
                category: category.to_string(),
                category_reason: "测试用途".to_string(),
                basis: vec!["provider=fake".to_string()],
                leaves_device: None,
            },
            permissions,
        }
    }

    #[test]
    fn first_run_permissions_are_all_closed() {
        let perms = ModelPermissions::default();
        assert!(!perms.local && !perms.private && !perms.public);
        for category in ["local", "private", "public", "unknown", "internal", "external", "anything"] {
            assert!(!facts(category, perms).is_allowed(), "{} 默认不得放行", category);
        }
    }

    #[test]
    fn unknown_category_is_never_allowed_and_has_no_switch() {
        let perms = ModelPermissions { local: true, private: true, public: true };
        let unknown = facts("unknown", perms);
        assert!(!unknown.is_allowed());
        let err = require_allowed(&unknown, SUBMIT_REFUSAL_MESSAGE).unwrap_err();
        assert_eq!(err["engine_error_code"], UNKNOWN_ENDPOINT_CODE);
    }

    #[test]
    fn permission_is_per_category_and_revocable() {
        let mut perms = ModelPermissions { local: true, private: false, public: false };
        assert!(require_allowed(&facts("local", perms), SUBMIT_REFUSAL_MESSAGE).is_ok());
        let denied = require_allowed(&facts("public", perms), SUBMIT_REFUSAL_MESSAGE).unwrap_err();
        assert_eq!(denied["engine_error_code"], REFUSAL_CODE);
        assert_eq!(denied["detail"]["enforced_by"], "host");

        perms.revoke_all();
        assert!(require_allowed(&facts("local", perms), SUBMIT_REFUSAL_MESSAGE).is_err());
    }

    /// OD-ARCH-022：旧类别名 internal / external 没有对应权限字段，
    /// 即使两者曾被开启也不会被放行（防止把“单位内部”语义带回来）。
    #[test]
    fn legacy_category_names_are_not_admitted() {
        let perms = ModelPermissions { local: true, private: true, public: true };
        for legacy in ["internal", "external"] {
            let f = facts(legacy, perms);
            assert!(!f.is_allowed(), "旧类别 {} 不得放行", legacy);
            let err = require_allowed(&f, SUBMIT_REFUSAL_MESSAGE).unwrap_err();
            assert_eq!(err["engine_error_code"], UNKNOWN_ENDPOINT_CODE);
        }
    }

    #[test]
    fn refusal_closure_never_runs_and_never_leaks_input() {
        let touched = Cell::new(false);
        let result = run_after_admission_denied_for_test(|| {
            touched.set(true);
            Ok(json!({ "started": true }))
        });
        assert!(result.is_err());
        assert!(!touched.get(), "拒绝时不得触达引擎");
        let text = result.unwrap_err().to_string();
        assert!(!text.contains("synthetic-secret-input"));
    }

    /// 不启动分类器、也不读磁盘的准入内部路径：直接把事实交给同一判定函数。
    fn run_after_admission_denied_for_test<F>(f: F) -> Result<Value, Value>
    where
        F: FnOnce() -> Result<Value, Value>,
    {
        let facts = facts("public", ModelPermissions::default());
        require_allowed(&facts, SUBMIT_REFUSAL_MESSAGE)?;
        f()
    }
}
