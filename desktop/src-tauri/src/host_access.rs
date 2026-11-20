//! 宿主授权的本地路径（OD-HARDEN-2043 任务 B）。
//!
//! 前端不再直接调用 `dialog:allow-open`；文件/目录由宿主侧弹出选择框，
//! 选中的路径规范化后登记到这里的注册表，再返回给前端。读取类命令只接受
//! 「注册表内」或「成果根/库根子路径」的路径，网页层被注入时也拿不到
//! `~/.ssh`、`~/.aws` 这类未登记的文件。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// 一次选择的有效期：30 分钟。命中时刷新（用户可能先从对话框选文件，稍后才发送）。
pub const SELECTION_TTL: Duration = Duration::from_secs(30 * 60);

/// 注册表上限；超出时淘汰最早的一次选择。
pub const MAX_SELECTIONS: usize = 256;

#[derive(Default)]
pub struct HostAccess {
    entries: Mutex<HashMap<PathBuf, Instant>>,
}

static REGISTRY: OnceLock<HostAccess> = OnceLock::new();

pub fn registry() -> &'static HostAccess {
    REGISTRY.get_or_init(HostAccess::default)
}

impl HostAccess {
    /// 登记一个已经规范化（真实落点）的路径。数量上限按最早选择淘汰。
    pub fn register_at(&self, canonical: PathBuf, now: Instant) {
        let mut entries = self.lock();
        entries.insert(canonical, now);
        if entries.len() > MAX_SELECTIONS {
            let oldest = entries
                .iter()
                .min_by_key(|(_, at)| *at)
                .map(|(path, _)| path.clone());
            if let Some(oldest) = oldest {
                entries.remove(&oldest);
            }
        }
    }

    /// 该路径是否仍是一次有效选择；命中时刷新时间（滑动过期）。
    pub fn is_authorized_at(&self, canonical: &Path, now: Instant) -> bool {
        let mut entries = self.lock();
        match entries.get_mut(canonical) {
            Some(at) if now.saturating_duration_since(*at) < SELECTION_TTL => {
                *at = now;
                true
            }
            Some(_) => {
                entries.remove(canonical);
                false
            }
            None => false,
        }
    }

    pub fn is_authorized(&self, canonical: &Path) -> bool {
        self.is_authorized_at(canonical, Instant::now())
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<PathBuf, Instant>> {
        match self.entries.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// 把一个真实落点登记进全局注册表。
pub fn remember(canonical: &Path) {
    registry().register_at(canonical.to_path_buf(), Instant::now());
}

/// 校验一个真实落点是否已授权；未授权时给出可操作的拒绝原因。
pub fn assert_authorized(canonical: &Path) -> Result<(), String> {
    if registry().is_authorized(canonical) {
        return Ok(());
    }
    Err(format!(
        "拒绝读取 {}：这个路径不是本次从文件选择框选中的文件，也不在成果目录/库根内。请重新用「选择文件」挑选。",
        canonical.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "omnidesk-host-access-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    #[test]
    fn unregistered_path_is_rejected() {
        let dir = temp_dir("unregistered");
        let file = dir.join("note.txt");
        fs::write(&file, "hello").expect("写测试文件");
        let canonical = fs::canonicalize(&file).expect("规范化");
        let registry = HostAccess::default();
        assert!(registry.is_authorized(&canonical) == false);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn registered_path_is_allowed() {
        let dir = temp_dir("registered");
        let file = dir.join("note.txt");
        fs::write(&file, "hello").expect("写测试文件");
        let canonical = fs::canonicalize(&file).expect("规范化");
        let registry = HostAccess::default();
        registry.register_at(canonical.clone(), Instant::now());
        assert!(registry.is_authorized(&canonical));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn expired_registration_is_rejected() {
        let dir = temp_dir("expired");
        let file = dir.join("note.txt");
        fs::write(&file, "hello").expect("写测试文件");
        let canonical = fs::canonicalize(&file).expect("规范化");
        let registry = HostAccess::default();
        let now = Instant::now();
        registry.register_at(canonical.clone(), now);
        let later = now + SELECTION_TTL + Duration::from_secs(1);
        assert!(!registry.is_authorized_at(&canonical, later));
        assert_eq!(registry.len(), 0, "过期项应被清掉");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hit_refreshes_expiry() {
        let dir = temp_dir("refresh");
        let file = dir.join("note.txt");
        fs::write(&file, "hello").expect("写测试文件");
        let canonical = fs::canonicalize(&file).expect("规范化");
        let registry = HostAccess::default();
        let now = Instant::now();
        registry.register_at(canonical.clone(), now);
        let mid = now + SELECTION_TTL / 2;
        assert!(registry.is_authorized_at(&canonical, mid));
        // 命中把时间刷新到 mid，所以再从 mid 起算仍在有效期内。
        let almost = mid + SELECTION_TTL - Duration::from_secs(1);
        assert!(registry.is_authorized_at(&canonical, almost));
        let too_late = almost + SELECTION_TTL;
        assert!(!registry.is_authorized_at(&canonical, too_late));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn symlink_to_forbidden_dir_resolves_to_target() {
        #[cfg(unix)]
        {
            let dir = temp_dir("symlink");
            let secret = dir.join(".ssh");
            fs::create_dir_all(&secret).expect("创建假 .ssh");
            let target = secret.join("id_rsa");
            fs::write(&target, "private").expect("写假私钥");
            let link = dir.join("harmless.txt");
            std::os::unix::fs::symlink(&target, &link).expect("建符号链接");
            let registry = HostAccess::default();
            // 即便登记的是链接自身的路径，真实落点仍是 ~/.ssh 下的文件；
            // 读取侧的凭证黑名单按真实落点拒绝。
            registry.register_at(fs::canonicalize(&link).expect("规范化"), Instant::now());
            assert!(registry.is_authorized(&fs::canonicalize(&target).expect("规范化")));
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn parent_traversal_is_normalized_before_lookup() {
        let dir = temp_dir("traversal");
        let nested = dir.join("inner");
        fs::create_dir_all(&nested).expect("创建子目录");
        let public = nested.join("ok.txt");
        fs::write(&public, "ok").expect("写文件");
        let secret = dir.join("secret.txt");
        fs::write(&secret, "secret").expect("写文件");
        let registry = HostAccess::default();
        registry.register_at(fs::canonicalize(&public).expect("规范化"), Instant::now());
        let sneaky = nested.join("..").join("secret.txt");
        let resolved = fs::canonicalize(&sneaky).expect("规范化");
        assert!(!registry.is_authorized(&resolved), ".. 穿越到未登记文件必须拒绝");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_is_capped_by_oldest_selection() {
        let registry = HostAccess::default();
        let now = Instant::now();
        for index in 0..(MAX_SELECTIONS + 5) {
            registry.register_at(PathBuf::from(format!("/tmp/omnidesk-pick-{index}")), now + Duration::from_millis(index as u64));
        }
        assert!(registry.len() <= MAX_SELECTIONS);
        assert!(!registry.is_authorized_at(Path::new("/tmp/omnidesk-pick-0"), now));
        let last = PathBuf::from(format!("/tmp/omnidesk-pick-{}", MAX_SELECTIONS + 4));
        assert!(registry.is_authorized_at(&last, now));
    }
}
