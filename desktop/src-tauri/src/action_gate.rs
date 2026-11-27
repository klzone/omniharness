//! 动作执行前的统一判定入口。
//!
//! 所有会触达宿主之外的动作，先经过本模块，得到三种结论：
//!   * `Allow`，放行；
//!   * `Deny`，拒绝并给出稳定错误码；
//!   * `NeedConfirm`，需要用户确认，并返回待确认记录。
//!
//! 本模块只做路由与审计。具体规则仍由 `admission`、`pending_writes`、`net_guard`
//! 负责，避免把三套语义合并成一个巨大的 if-else。
//!
//! 审计只记录动作类别、目标类别、结论与错误码，不记录输入正文、URL 或完整路径。
//! 审计写入失败不阻断业务动作，与 `event_log` 的口径一致。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::admission::Admitted;
use crate::engine_supervisor::EngineConfig;
use crate::net_guard::NetPolicy;
use crate::pending_writes::{CreatePendingWrite, PendingWriteRecord};

pub const AUDIT_SCHEMA: &str = "omnidesk.action-decision.v1";
pub const AUDIT_DIR: &str = "action-gate";
pub const AUDIT_FILE: &str = "decisions.jsonl";

/// 一次动作的判定结果。三种结果与宿主对外的说法保持一致。
#[derive(Debug)]
pub enum Decision<T> {
    Allow(T),
    Deny {
        code: String,
        message: String,
        detail: Value,
    },
    NeedConfirm {
        pending_id: String,
        permit: T,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    EngineCall,
    ModelWrite,
    ExternalFetch,
}

impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Action::EngineCall => "engine_call",
            Action::ModelWrite => "model_write",
            Action::ExternalFetch => "external_fetch",
        }
    }
}

fn audit_path(audit_dir: Option<&Path>) -> Option<PathBuf> {
    audit_dir.map(|dir| dir.join(AUDIT_DIR).join(AUDIT_FILE))
}

/// 审计只记录已知场景；未知字符串不进入日志，避免把前端内容带进审计文件。
fn scenario_target(scenario: &str) -> &'static str {
    match scenario {
        "weekly_report" => "weekly_report",
        "material_writing" => "material_writing",
        "project_management" => "project_management",
        _ => "unknown",
    }
}

fn record_decision(
    audit_dir: Option<&Path>,
    action: Action,
    target: &str,
    decision: &str,
    code: &str,
) {
    let Some(path) = audit_path(audit_dir) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let row = json!({
        "schema": AUDIT_SCHEMA,
        "ts": crate::task_shell::now_secs(),
        "action": action.as_str(),
        "target": target,
        "decision": decision,
        "code": code,
    });
    let Ok(line) = serde_json::to_string(&row) else {
        return;
    };
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(format!("{}\n", line).as_bytes()));
}

/// 引擎调用前判定。拒绝时保留 `admission` 的结构化错误，便于前端显示原因。
pub fn authorize_engine(
    config: &EngineConfig,
    config_dir: &Path,
    refusal: &str,
    audit_dir: Option<&Path>,
) -> Decision<Admitted> {
    match crate::admission::check(config, config_dir, refusal) {
        Ok(admitted) => {
            let target = admitted.facts.route.category.clone();
            record_decision(audit_dir, Action::EngineCall, &target, "allow", "admitted");
            Decision::Allow(admitted)
        }
        Err(err) => {
            let code = err
                .get("engine_error_code")
                .and_then(Value::as_str)
                .unwrap_or("admission_denied")
                .to_string();
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("模型端点未通过准入判定。")
                .to_string();
            let detail = err.get("detail").cloned().unwrap_or_else(|| json!({}));
            let target = detail
                .get("admission")
                .and_then(|value| value.get("category"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            record_decision(audit_dir, Action::EngineCall, &target, "deny", &code);
            Decision::Deny {
                code,
                message,
                detail,
            }
        }
    }
}

/// 模型或定时任务发起写动作时，登记待确认记录。
pub fn authorize_model_write(
    dir: &Path,
    request: &CreatePendingWrite,
    now: f64,
    audit_dir: Option<&Path>,
) -> Decision<PendingWriteRecord> {
    let target = scenario_target(&request.payload.scenario);
    match crate::pending_writes::create(dir, request, now) {
        Ok(record) => {
            record_decision(
                audit_dir,
                Action::ModelWrite,
                target,
                "need_confirm",
                "pending_write",
            );
            Decision::NeedConfirm {
                pending_id: record.id.clone(),
                permit: record,
            }
        }
        Err(message) => {
            record_decision(
                audit_dir,
                Action::ModelWrite,
                target,
                "deny",
                "pending_write_rejected",
            );
            Decision::Deny {
                code: "pending_write_rejected".to_string(),
                message,
                detail: json!({}),
            }
        }
    }
}

/// 用户确认后、真正落盘前的判定。
pub fn authorize_confirmed_write(
    dir: &Path,
    id: &str,
    now: f64,
    audit_dir: Option<&Path>,
) -> Decision<PendingWriteRecord> {
    match crate::pending_writes::load(dir, id) {
        Ok(record) => {
            let payload = record.payload.clone();
            match crate::pending_writes::validate_for_write(dir, id, &payload, now) {
                Ok(validated) => {
                    let target = scenario_target(&validated.payload.scenario);
                    record_decision(
                        audit_dir,
                        Action::ModelWrite,
                        target,
                        "allow",
                        "confirmed_write",
                    );
                    Decision::Allow(validated)
                }
                Err(message) => {
                    record_decision(
                        audit_dir,
                        Action::ModelWrite,
                        scenario_target(&record.payload.scenario),
                        "deny",
                        "confirmed_write_rejected",
                    );
                    Decision::Deny {
                        code: "confirmed_write_rejected".to_string(),
                        message,
                        detail: json!({}),
                    }
                }
            }
        }
        Err(message) => {
            record_decision(
                audit_dir,
                Action::ModelWrite,
                "unknown",
                "deny",
                "confirmed_write_rejected",
            );
            Decision::Deny {
                code: "confirmed_write_rejected".to_string(),
                message,
                detail: json!({}),
            }
        }
    }
}

/// 外联地址判定。返回已校验的地址列表，供调用方钉住 DNS 解析结果。
pub fn authorize_external_fetch(
    url: &reqwest::Url,
    policy: NetPolicy,
    audit_dir: Option<&Path>,
) -> Decision<Vec<SocketAddr>> {
    let target = match policy {
        NetPolicy::PublicOnly => "public_only",
        NetPolicy::AllowPrivate => "allow_private",
    };
    match crate::net_guard::assert_url_addresses(url, policy) {
        Ok(addrs) => {
            record_decision(audit_dir, Action::ExternalFetch, target, "allow", "external_fetch");
            Decision::Allow(addrs)
        }
        Err(message) => {
            record_decision(
                audit_dir,
                Action::ExternalFetch,
                target,
                "deny",
                "external_fetch_rejected",
            );
            Decision::Deny {
                code: "external_fetch_rejected".to_string(),
                message,
                detail: json!({}),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "omnidesk-action-gate-{}-{}-{}",
                label,
                std::process::id(),
                stamp
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_request(source: &str, scenario: &str) -> CreatePendingWrite {
        CreatePendingWrite {
            action: "save_offline".to_string(),
            source: source.to_string(),
            summary: "保存离线成果".to_string(),
            impact: "只新增一份成果文件".to_string(),
            payload: crate::pending_writes::PendingWritePayload {
                scenario: scenario.to_string(),
                title: "合成成果".to_string(),
                markdown: "# 合成成果\n\n正文。".to_string(),
                ai_assisted: true,
            },
        }
    }

    fn read_audit(dir: &Path) -> String {
        fs::read_to_string(dir.join(AUDIT_DIR).join(AUDIT_FILE)).unwrap_or_default()
    }

    #[test]
    fn model_write_returns_need_confirm_and_logs_without_body() {
        let temp = TempDir::new("write-need-confirm");
        let pending_dir = temp.0.join("pending-writes");
        let audit_dir = temp.0.join("audit");
        let now = 1_700_000_000.0;
        match authorize_model_write(
            &pending_dir,
            &write_request("model", "weekly_report"),
            now,
            Some(&audit_dir),
        ) {
            Decision::NeedConfirm { pending_id, permit } => {
                assert_eq!(permit.id, pending_id);
                let log = read_audit(&audit_dir);
                assert!(log.contains("\"decision\":\"need_confirm\""));
                assert!(log.contains("\"action\":\"model_write\""));
                assert!(log.contains("\"target\":\"weekly_report\""));
                assert!(!log.contains("合成成果"));
                assert!(!log.contains("正文"));
            }
            other => panic!("应返回待确认：{:?}", other),
        }
    }

    #[test]
    fn invalid_model_write_is_denied_and_logged() {
        let temp = TempDir::new("write-deny");
        let pending_dir = temp.0.join("pending-writes");
        let audit_dir = temp.0.join("audit");
        let now = 1_700_000_000.0;
        match authorize_model_write(
            &pending_dir,
            &write_request("human", "weekly_report"),
            now,
            Some(&audit_dir),
        ) {
            Decision::Deny { code, .. } => {
                assert_eq!(code, "pending_write_rejected");
                assert!(read_audit(&audit_dir).contains("\"decision\":\"deny\""));
            }
            other => panic!("应拒绝：{:?}", other),
        }
    }

    #[test]
    fn private_fetch_is_denied_by_default() {
        let temp = TempDir::new("fetch-deny");
        let audit_dir = temp.0.join("audit");
        let url = reqwest::Url::parse("http://127.0.0.1:9/feed").unwrap();
        match authorize_external_fetch(&url, NetPolicy::PublicOnly, Some(&audit_dir)) {
            Decision::Deny { code, .. } => {
                assert_eq!(code, "external_fetch_rejected");
                let log = read_audit(&audit_dir);
                assert!(log.contains("\"action\":\"external_fetch\""));
                assert!(!log.contains("127.0.0.1"));
            }
            other => panic!("应拒绝回环地址：{:?}", other),
        }
    }

    #[test]
    fn confirmed_write_rejects_unknown_record() {
        let temp = TempDir::new("confirm-deny");
        let pending_dir = temp.0.join("pending-writes");
        let audit_dir = temp.0.join("audit");
        match authorize_confirmed_write(&pending_dir, "pw-missing", 1_700_000_000.0, Some(&audit_dir))
        {
            Decision::Deny { code, .. } => {
                assert_eq!(code, "confirmed_write_rejected");
                assert!(read_audit(&audit_dir).contains("\"decision\":\"deny\""));
            }
            other => panic!("未知记录应拒绝：{:?}", other),
        }
    }
}
