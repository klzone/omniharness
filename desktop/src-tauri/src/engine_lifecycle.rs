//! OD-ARCH-023：模型权限变更与引擎启动/请求之间的串行化边界。
//!
//! 背景（REVIEW.md 的 OD-ARCH-022 复核）：`engine_submit_task` 等命令在**锁外**
//! 读取准入事实并取得 `AdmissionGrant`，之后才加锁启动引擎；权限变更也是先写设置、
//! 再关引擎、最后才置位屏障。并发时，调用可以拿着变更前的 grant，在撤权停止引擎
//! 之后重新启动并执行。
//!
//! 本模块把“准入判定 + 启动引擎”放进同一把互斥锁的临界区：
//!   * [`EngineLifecycle::begin_call`]：开始一次可能启动/触达引擎的调用；
//!     屏障生效期间直接拒绝。守卫存活期间，权限变更无法开始。
//!   * [`EngineLifecycle::begin_change`]：开始一次权限变更，独占生命周期，
//!     并在整个变更期间置位屏障（失败则保留，成功则清除并自增世代）。
//!
//! 因此不存在“先取旧 grant、撤权完成后再使用”的窗口：使用 grant 的代码路径
//! 必须先持有调用守卫，而权限变更必须独占同一把锁。

use std::sync::{Mutex, MutexGuard};

/// 生命周期世代：每次权限变更自增。旧世代取得的准入结果不再代表当前权限。
const INITIAL_EPOCH: u64 = 1;

#[derive(Default)]
struct State {
    /// `Some(原因)` = 拒绝一切新的引擎启动/请求。
    barrier: Option<String>,
    epoch: u64,
}

/// 一次引擎调用的守卫。存活期间权限变更无法开始。
pub struct CallGuard<'a> {
    guard: MutexGuard<'a, State>,
    epoch: u64,
}

impl CallGuard<'_> {
    /// 本调用所属的权限世代。
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// 当前权限世代（**非重入**：直接读守卫已持有的状态，不再加锁）。
    ///
    /// OD-ARCH-024：持守卫期间**不得**调用 `EngineLifecycle::is_current()` /
    /// `epoch()` / `blocked_reason()`——它们会再次锁同一把非可重入互斥锁并自锁。
    pub fn current_epoch(&self) -> u64 {
        self.guard.epoch
    }
}

/// 一次权限变更的独占守卫。
pub struct ChangeGuard<'a> {
    guard: MutexGuard<'a, State>,
}

impl ChangeGuard<'_> {
    /// 变更成功且已确认旧引擎停止：清除屏障并自增世代。
    pub fn succeed(mut self) {
        self.guard.barrier = None;
        self.guard.epoch = self.guard.epoch.wrapping_add(1).max(INITIAL_EPOCH);
    }

    /// 变更未能确认旧引擎停止：保留屏障（失败关闭），世代同样自增，
    /// 使变更前取得的准入结果不再代表当前权限。
    pub fn fail(mut self, reason: String) {
        self.guard.barrier = Some(reason);
        self.guard.epoch = self.guard.epoch.wrapping_add(1).max(INITIAL_EPOCH);
    }
}

pub struct EngineLifecycle {
    inner: Mutex<State>,
}

impl Default for EngineLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineLifecycle {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(State {
                barrier: None,
                epoch: INITIAL_EPOCH,
            }),
        }
    }

    /// 开始一次可能启动/触达引擎的调用。
    ///
    /// 返回 `Err(原因)` 表示当前处于屏障状态（权限变更未完成），必须拒绝。
    pub fn begin_call(&self) -> Result<CallGuard<'_>, String> {
        let guard = self.inner.lock().unwrap();
        if let Some(reason) = guard.barrier.as_deref() {
            return Err(reason.to_string());
        }
        let epoch = guard.epoch;
        Ok(CallGuard {
            guard,
            epoch,
        })
    }

    /// 开始一次权限变更。会阻塞到没有调用持有守卫为止（不设超时：变更必须等到
    /// 在途调用结束，否则就会回到“变更与旧调用交错”的竞态）。
    pub fn begin_change(&self) -> ChangeGuard<'_> {
        let mut guard = self.inner.lock().unwrap();
        // 变更开始即拒绝新调用；成功时由 `succeed()` 清除。
        guard.barrier = Some("模型权限变更进行中".to_string());
        ChangeGuard { guard }
    }

    /// 非阻塞版本的权限变更入口：已有调用持守卫时返回 None。
    /// 用于确定性并发测试，不用于生产路径。
    #[cfg(test)]
    pub fn try_begin_change(&self) -> Option<ChangeGuard<'_>> {
        let mut guard = self.inner.try_lock().ok()?;
        guard.barrier = Some("模型权限变更进行中".to_string());
        Some(ChangeGuard { guard })
    }

    /// 当前屏障原因（None = 允许新的引擎调用）。
    pub fn blocked_reason(&self) -> Option<String> {
        self.inner.lock().unwrap().barrier.clone()
    }

    /// 当前权限世代。
    pub fn epoch(&self) -> u64 {
        self.inner.lock().unwrap().epoch
    }

    /// 该世代是否仍是当前世代。旧世代取得的准入结果不得用于启动引擎。
    ///
    /// **注意（OD-ARCH-024）**：本方法会锁 `inner`。持有 [`CallGuard`] 期间调用它会
    /// 自锁；守卫内请改用 [`CallGuard::current_epoch`]。
    pub fn is_current(&self, epoch: u64) -> bool {
        self.inner.lock().unwrap().epoch == epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn successful_change_clears_barrier() {
        let life = EngineLifecycle::new();
        assert!(life.begin_call().is_ok(), "初始状态应允许调用");

        // 变更独占期间，非阻塞探针看不到锁（说明生命周期被独占）。
        {
            let change = life.begin_change();
            assert!(
                life.try_begin_change().is_none(),
                "变更独占期间不得再开始变更"
            );
            change.succeed();
        }
        assert!(life.begin_call().is_ok(), "变更成功后应恢复");
        assert_eq!(life.blocked_reason(), None);
    }

    #[test]
    fn stop_failure_keeps_blocking_until_recovered() {
        let life = EngineLifecycle::new();
        life.begin_change()
            .fail("子进程未能在 10 秒内退出".to_string());

        let err = life
            .begin_call()
            .err()
            .expect("停止失败后必须持续拒绝");
        assert!(err.contains("子进程"), "应保留失败原因：{}", err);
        assert_eq!(
            life.blocked_reason().as_deref(),
            Some("子进程未能在 10 秒内退出")
        );

        // 恢复：重试变更并确认停止。
        life.begin_change().succeed();
        assert!(life.begin_call().is_ok(), "确认停止并恢复后才重新开放");
        assert_eq!(life.blocked_reason(), None);
    }

    /// 计划要求的核心时序：调用在“准入完成之后、启动引擎之前”被暂停时，
    /// 撤权**不能**开始；因此不存在“撤权完成后仍用旧 grant 启动引擎”的窗口。
    #[test]
    fn call_holding_gate_prevents_change_from_starting() {
        let life = Arc::new(EngineLifecycle::new());
        let (held_tx, held_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let l2 = Arc::clone(&life);
        let handle = thread::spawn(move || {
            // 模拟：准入判定完成、拿到 grant，尚未启动引擎。
            let guard = l2.begin_call().expect("首次调用应被允许");
            held_tx.send(guard.epoch()).unwrap();
            release_rx.recv().unwrap(); // 暂停在这里
            drop(guard);
        });

        let epoch_before = held_rx.recv().unwrap();
        assert!(
            life.try_begin_change().is_none(),
            "调用持守卫期间，撤权不得开始（否则旧 grant 会在撤权后被使用）"
        );

        release_tx.send(()).unwrap();
        handle.join().unwrap();

        // 调用结束后撤权才能开始。
        life.try_begin_change()
            .expect("调用结束后撤权可以开始")
            .succeed();

        // 世代推进：撤权前取得的世代不再是当前世代，旧准入结果失效。
        assert!(life.epoch() > epoch_before, "撤权应推进权限世代");
        assert!(
            !life.is_current(epoch_before),
            "旧世代的准入结果不得再用于启动引擎"
        );
        // 注意：守卫本身持有互斥锁，因此取到世代后必须先释放守卫再查询，
        // 否则会在同一线程重入自锁。
        let fresh_epoch = {
            let guard = life.begin_call().expect("撤权完成后应可开始新调用");
            guard.epoch()
        };
        assert!(life.is_current(fresh_epoch));
    }

    /// 变更持锁期间，新的引擎调用**不能**开始（结构性互斥，非概率判断）。
    #[test]
    fn new_calls_cannot_start_while_change_holds_lock() {
        let life = Arc::new(EngineLifecycle::new());
        let change = life.begin_change();

        let (tx, rx) = mpsc::channel();
        let l2 = Arc::clone(&life);
        let handle = thread::spawn(move || {
            let _ = l2.begin_call();
            let _ = tx.send(());
        });

        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "变更持锁期间新调用不得开始"
        );
        change.succeed();
        assert!(
            rx.recv_timeout(Duration::from_secs(5)).is_ok(),
            "变更结束后调用应被放行"
        );
        handle.join().unwrap();
    }

    #[test]
    fn failure_also_advances_epoch() {
        let life = EngineLifecycle::new();
        let before = life.epoch();
        life.begin_change().fail("停止失败".to_string());
        assert!(life.epoch() > before, "失败的变更同样推进世代");
    }
}
