//! OD-ARCH-175 阶段 2：运行期思考档位拒绝的单次自动回退（宿主层）。
//!
//! **触发条件（只认精确组合，宽泛匹配一律不触发）：**
//!   1. 任务状态为 `failed`；
//!   2. note 精确命中 DSH 的运行期拒绝文本
//!      `stop_reason=error; error=engine reported turn/end=error：pi-ai provider
//!      "<provider>" model "<model>" does not support reasoning effort "<effort>"`
//!      （解析 provider / model / effort 三个捕获组）；
//!   3. 这三个值与当前覆盖文件里**实际生效**的路由完全一致；
//!   4. 该路由确实下发了显式 `reasoningEffort`，且与捕获的 effort 相等。
//!
//! 错误码 `UNSUPPORTED_REASONING_EFFORT` 不会进入 note（适配层只保留 message），
//! 因此本模块不匹配错误码。DSH 对 `reasoningEfforts` 为 null / 空对象 / 仅 off 的
//! 配置错误消息是另外两句（"has an empty reasoningEfforts" / "offers no level beyond
//! off"），也不得当作可回退失败。
//!
//! **回退动作：** 用 `model_routes::select_route(..., None)` 重写覆盖文件去掉显式
//! 档位 → `sync_model_overlay` → `stop_engine`（下次任务按新路由重启）。
//! 同一 (覆盖文件, provider, model, effort) 只触发一次（进程内存态，重启后失效）。
//! 引擎停止在后台线程执行，状态轮询路径不等待关闭；新调用由 lifecycle 屏障挡住。
//!
//! **note 契约（阶段 4 验证接口）：** 命中并把覆盖文件改写成功后，返回给前端的
//! status note 追加一句，完整格式为
//! `<原 note>；已自动回退为默认思考档位，请重发任务`
//! （原 note 为空时只保留该句）。不自动重发失败任务，重发由用户走现有重试入口。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// 回退后追加到状态 note 的固定句子（中文，一句）。
pub const FALLBACK_NOTE_SENTENCE: &str = "已自动回退为默认思考档位，请重发任务";

/// 从运行期失败 note 解析出的拒绝路由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffortRejection {
    pub provider: String,
    pub model: String,
    pub effort: String,
}

/// 覆盖文件里当前实际生效的一条路由（只含非机密字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayRoute {
    pub provider: String,
    pub model: String,
    /// 覆盖文件**显式下发**的档位；None / 空串 / "default" 都表示未显式下发。
    pub effort: Option<String>,
}

/// 解析 DSH 的稳定拒绝文本。必须命中完整句式；其它 failed 原因或配置错误文本
/// 返回 None。允许 note 后面继续跟 `;` 分隔的宿主附加段（如 persist_failed）。
pub fn parse_effort_rejection(note: &str) -> Option<EffortRejection> {
    let marker = "error=engine reported turn/end=";
    let start = note.find(marker)? + marker.len();
    let rest = &note[start..];
    let (reason_kind, rest) = rest.split_once('：')?;
    if reason_kind != "error" && reason_kind != "failed" {
        return None;
    }
    let rest = rest.strip_prefix("pi-ai provider \"")?;
    let (provider, rest) = rest.split_once("\" model \"")?;
    if provider.is_empty() {
        return None;
    }
    let (model, rest) = rest.split_once("\" does not support reasoning effort \"")?;
    if model.is_empty() {
        return None;
    }
    let (effort, tail) = rest.split_once('"')?;
    if effort.is_empty() {
        return None;
    }
    // 句末之后只允许宿主拼接的分段（"; "），其它形状说明不是这条稳定文本。
    if !(tail.is_empty() || tail.starts_with(';') || tail.starts_with('；')) {
        return None;
    }
    Some(EffortRejection {
        provider: provider.to_string(),
        model: model.to_string(),
        effort: effort.to_string(),
    })
}

/// 读取覆盖文件里实际生效的路由（acp 与 agent-default-model 两个块都读）。
///
/// 顺序上 `agent-default-model` 在前（web-api 会话实际读取），`acp` 在后。只读
/// 非机密字段，不接触凭据。两个块理论上由同一份设置写出；如果用户手改造成分叉，
/// 调用方按「任一块精确匹配」判断，避免漏掉当前通道真正下发的档位。
pub fn routes_from_yaml(raw: &str) -> Option<Vec<OverlayRoute>> {
    let doc: serde_yaml::Value = serde_yaml::from_str(raw).ok()?;
    let items = doc.as_sequence()?;
    let mut out = Vec::new();
    for wanted in ["agent-default-model", "acp"] {
        let Some(item) = items
            .iter()
            .find(|item| item.get("id").and_then(|v| v.as_str()) == Some(wanted))
        else {
            continue;
        };
        let Some(config) = item.get("config") else {
            continue;
        };
        let provider = config.get("provider").and_then(|v| v.as_str()).map(str::trim);
        let model = config.get("model").and_then(|v| v.as_str()).map(str::trim);
        let (Some(provider), Some(model)) = (provider, model) else {
            continue;
        };
        if provider.is_empty() || model.is_empty() {
            continue;
        }
        let effort = config
            .get("reasoningEffort")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|e| !e.is_empty() && *e != "default")
            .map(str::to_string);
        out.push(OverlayRoute {
            provider: provider.to_string(),
            model: model.to_string(),
            effort,
        });
    }
    Some(out)
}

/// 读取覆盖文件里首选的一条路由（agent-default-model 优先，缺失时退回 acp）。
pub fn effective_route_from_yaml(raw: &str) -> Option<OverlayRoute> {
    routes_from_yaml(raw)?.into_iter().next()
}

/// 从磁盘读取当前覆盖文件的所有生效路由（文件缺失 / 损坏时返回 None）。
pub fn read_routes(path: &Path) -> Option<Vec<OverlayRoute>> {
    let raw = std::fs::read_to_string(path).ok()?;
    routes_from_yaml(&raw)
}

/// 捕获的拒绝是否与覆盖文件当前下发完全一致（provider / model / effort 三者）。
pub fn rejection_matches_route(rejection: &EffortRejection, route: &OverlayRoute) -> bool {
    rejection.provider == route.provider
        && rejection.model == route.model
        && route.effort.as_deref() == Some(rejection.effort.as_str())
}

/// 状态是否已经是失败终态。
pub fn status_is_failed(status: &serde_json::Value) -> bool {
    status.get("status").and_then(|v| v.as_str()) == Some("failed")
}

/// 追加回退告知句；重复调用不会重复追加。
pub fn append_fallback_note(status: &mut serde_json::Value) -> bool {
    let Some(existing) = status.get("note").and_then(|v| v.as_str()) else {
        return false;
    };
    if existing.ends_with(FALLBACK_NOTE_SENTENCE) {
        return true;
    }
    let note = if existing.is_empty() {
        FALLBACK_NOTE_SENTENCE.to_string()
    } else {
        format!("{}；{}", existing, FALLBACK_NOTE_SENTENCE)
    };
    status["note"] = serde_json::Value::String(note);
    true
}

/// 一次 guard 占位的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackClaim {
    /// 本次是首次触发，调用方执行回退；只有回写成功后 status note 才追加告知句。
    Started,
    /// 同一路由的回退正在进行：不要重复改写，也不要提前告知“已回退”。
    InFlight,
    /// 同一路由已回退成功：后续轮询可直接补齐同一条告知句。
    Applied,
    /// 同一路由已尝试但未成功（例如脚本失败）：不再重复触发，也不告知已回退。
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GuardState {
    InFlight,
    Applied,
    Failed,
}

/// 同一路由的单次触发 guard（进程内存态，重启后失效可接受）。
#[derive(Default)]
pub struct FallbackGuard {
    states: Mutex<HashMap<String, GuardState>>,
}

impl FallbackGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// 只读查看当前状态，不占位。用于已回退成功的路由在覆盖文件改写后仍能
    /// 稳定补齐告知（此时覆盖文件已不再显式下发档位，无法再按原 route 匹配）。
    pub fn peek(&self, key: &str) -> Option<FallbackClaim> {
        let states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match states.get(key) {
            Some(GuardState::InFlight) => Some(FallbackClaim::InFlight),
            Some(GuardState::Applied) => Some(FallbackClaim::Applied),
            Some(GuardState::Failed) => Some(FallbackClaim::Failed),
            None => None,
        }
    }

    /// 首次触发返回 Started；同一 key 再次出现返回它当前的状态。
    pub fn claim(&self, key: &str) -> FallbackClaim {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match states.get(key) {
            Some(GuardState::InFlight) => FallbackClaim::InFlight,
            Some(GuardState::Applied) => FallbackClaim::Applied,
            Some(GuardState::Failed) => FallbackClaim::Failed,
            None => {
                states.insert(key.to_string(), GuardState::InFlight);
                FallbackClaim::Started
            }
        }
    }

    /// 覆盖文件已成功去掉显式档位。
    pub fn mark_applied(&self, key: &str) {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        states.insert(key.to_string(), GuardState::Applied);
    }

    /// 本次回退没有成功；保持一次性占用，避免同一错误在每次轮询里反复改写。
    pub fn mark_failed(&self, key: &str) {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        states.insert(key.to_string(), GuardState::Failed);
    }
}

static EFFORT_FALLBACK_GUARD: OnceLock<FallbackGuard> = OnceLock::new();

/// 宿主进程内的回退 guard：同一路由的同一错误只触发一次。
pub fn global_guard() -> &'static FallbackGuard {
    EFFORT_FALLBACK_GUARD.get_or_init(FallbackGuard::new)
}

/// guard key：覆盖文件 + 捕获路由。同一路由换用另一个无效档位算另一次尝试。
pub fn route_key(overlay: &Path, rejection: &EffortRejection) -> String {
    format!(
        "{}|{}|{}|{}",
        overlay.display(),
        rejection.provider,
        rejection.model,
        rejection.effort
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    const REJECTION_NOTE: &str = "stop_reason=error; error=engine reported turn/end=error：pi-ai provider \"volcengine\" model \"deepseek-v4.1-flash\" does not support reasoning effort \"high\"";

    fn rejection() -> EffortRejection {
        EffortRejection {
            provider: "volcengine".to_string(),
            model: "deepseek-v4.1-flash".to_string(),
            effort: "high".to_string(),
        }
    }

    fn overlay_yaml(effort: Option<&str>) -> String {
        let line = effort
            .map(|e| format!("    reasoningEffort: {}\n", e))
            .unwrap_or_default();
        format!(
            "- id: llm-pi-ai\n  config:\n    providers:\n      volcengine:\n        api: openai-completions\n        baseURL: https://example.test/v1\n- id: acp\n  config:\n    provider: volcengine\n    model: deepseek-v4.1-flash\n{line}- id: agent-default-model\n  config:\n    provider: volcengine\n    model: deepseek-v4.1-flash\n{line}"
        )
    }

    #[test]
    fn parses_stable_rejection_text() {
        assert_eq!(parse_effort_rejection(REJECTION_NOTE), Some(rejection()));
    }

    #[test]
    fn parses_when_host_appends_more_note_parts() {
        let note = format!("{}; persist_failed=disk full", REJECTION_NOTE);
        assert_eq!(parse_effort_rejection(&note), Some(rejection()));
    }

    #[test]
    fn ignores_other_failed_reasons() {
        for note in [
            "stop_reason=error; error=engine reported turn/end=error：provider request failed",
            "stop_reason=timeout; error=任务超时",
            "turn/end=error",
        ] {
            assert_eq!(parse_effort_rejection(note), None, "不应命中：{note}");
        }
    }

    #[test]
    fn ignores_empty_effort_configuration_errors() {
        for message in [
            "pi-ai provider \"volcengine\" model \"deepseek-v4.1-flash\" has an empty reasoningEfforts",
            "pi-ai provider \"volcengine\" model \"deepseek-v4.1-flash\" offers no level beyond off",
        ] {
            let note = format!("stop_reason=error; error=engine reported turn/end=error：{message}");
            assert_eq!(parse_effort_rejection(&note), None, "不应命中配置错误：{message}");
        }
    }

    #[test]
    fn requires_exact_route_and_explicit_effort() {
        let rejection = rejection();
        let mut route = OverlayRoute {
            provider: "volcengine".to_string(),
            model: "deepseek-v4.1-flash".to_string(),
            effort: Some("high".to_string()),
        };
        assert!(rejection_matches_route(&rejection, &route));

        route.provider = "other".to_string();
        assert!(!rejection_matches_route(&rejection, &route));
        route.provider = "volcengine".to_string();

        route.model = "deepseek-v4.1-pro".to_string();
        assert!(!rejection_matches_route(&rejection, &route));
        route.model = "deepseek-v4.1-flash".to_string();

        route.effort = Some("max".to_string());
        assert!(!rejection_matches_route(&rejection, &route));

        // 覆盖文件未显式下发档位：不是本链路要回退的对象。
        route.effort = None;
        assert!(!rejection_matches_route(&rejection, &route));
    }

    #[test]
    fn reads_effective_route_prefers_agent_default_model() {
        let route = effective_route_from_yaml(&overlay_yaml(Some("high"))).expect("应读出路由");
        assert_eq!(route.provider, "volcengine");
        assert_eq!(route.model, "deepseek-v4.1-flash");
        assert_eq!(route.effort.as_deref(), Some("high"));

        // 无显式档位：effort 为 None。
        let route = effective_route_from_yaml(&overlay_yaml(None)).expect("应读出路由");
        assert_eq!(route.effort, None);
    }

    #[test]
    fn appends_note_once() {
        let mut status = json!({
            "status": "failed",
            "note": format!("{}; persist_failed=disk full", REJECTION_NOTE),
        });
        assert!(append_fallback_note(&mut status));
        let expected = format!(
            "{}; persist_failed=disk full；{}",
            REJECTION_NOTE, FALLBACK_NOTE_SENTENCE
        );
        assert_eq!(status["note"].as_str(), Some(expected.as_str()));
        assert!(append_fallback_note(&mut status));
        assert_eq!(status["note"].as_str(), Some(expected.as_str()), "不得重复追加");
    }

    #[test]
    fn guard_allows_only_one_trigger_per_route() {
        let guard = FallbackGuard::new();
        let overlay = PathBuf::from("/data/config/omnidesk-model.yml");
        let key = route_key(&overlay, &rejection());
        assert_eq!(guard.peek(&key), None);
        assert_eq!(guard.claim(&key), FallbackClaim::Started, "首次应触发");
        assert_eq!(guard.peek(&key), Some(FallbackClaim::InFlight));
        assert_eq!(
            guard.claim(&key),
            FallbackClaim::InFlight,
            "同一路由的同一错误不得重复触发"
        );

        guard.mark_failed(&key);
        assert_eq!(guard.claim(&key), FallbackClaim::Failed);
        assert_eq!(guard.peek(&key), Some(FallbackClaim::Failed));

        let other_effort = EffortRejection {
            effort: "max".to_string(),
            ..rejection()
        };
        assert_eq!(
            guard.claim(&route_key(&overlay, &other_effort)),
            FallbackClaim::Started,
            "换一个档位属于另一次尝试"
        );

        guard.mark_applied(&key);
        assert_eq!(guard.claim(&key), FallbackClaim::Applied);
        assert_eq!(guard.peek(&key), Some(FallbackClaim::Applied));
    }
}
