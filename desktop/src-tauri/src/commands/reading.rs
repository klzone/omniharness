//! 阅读 / RSS 命令（OD-HARDEN-2043 任务 D：从 lib.rs 机械迁出，签名与行为不变）。

use tauri::State;

use crate::rss_store;
use crate::SharedHost;

// ---------------------------------------------------------------------------
// 健脑室 RSS 订阅（OD-READ-002 阶段 1）
//
// 订阅清单与文章元数据只落在阅读目录的 rss/ 下；文章正文与普通书共用
// books/<book_id>.json，批注继续走 reading_store 的既有命令。
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub(crate) fn rss_subscriptions_list(
    state: State<'_, SharedHost>,
) -> Result<Vec<rss_store::RssSubscription>, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    rss_store::list_subscriptions(&paths)
}

#[tauri::command(async)]
pub(crate) fn rss_preset_sources() -> Vec<rss_store::RssPresetSource> {
    rss_store::preset_sources()
}

#[tauri::command(async)]
pub(crate) fn rss_subscription_add(
    state: State<'_, SharedHost>,
    url: String,
    name: Option<String>,
) -> Result<rss_store::RssSubscription, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    rss_store::add_subscription(&paths, &url, name.as_deref())
}

#[tauri::command(async)]
pub(crate) fn rss_subscription_remove(
    state: State<'_, SharedHost>,
    id: String,
) -> Result<(), String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    rss_store::remove_subscription(&paths, &id)
}

#[tauri::command(async)]
pub(crate) fn rss_articles_list(
    state: State<'_, SharedHost>,
) -> Result<Vec<rss_store::RssArticleMeta>, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    rss_store::list_articles(&paths)
}

#[tauri::command(async)]
pub(crate) fn rss_article_mark_read(
    state: State<'_, SharedHost>,
    book_id: String,
) -> Result<rss_store::RssArticleMeta, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    rss_store::mark_article_read(&paths, &book_id)
}

/// 收藏/取消收藏一篇订阅文章；只写 articles/<book_id>.json。
#[tauri::command(async)]
pub(crate) fn rss_article_set_favorite(
    state: State<'_, SharedHost>,
    book_id: String,
    favorite: bool,
) -> Result<rss_store::RssArticleMeta, String> {
    let paths = {
        let host = state.lock().map_err(|_| "宿主状态锁不可用。".to_string())?;
        host.paths.clone()
    };
    rss_store::set_article_favorite(&paths, &book_id, favorite)
}

/// 「允许内网订阅源」开关（OD-HARDEN-2043）。默认关；只影响订阅源，正文媒体不受影响。
#[tauri::command(async)]
pub(crate) fn rss_settings_get(state: State<'_, SharedHost>) -> Result<bool, String> {
    let paths = {
        let host = state.lock().map_err(|_| "宿主状态锁不可用。".to_string())?;
        host.paths.clone()
    };
    rss_store::allow_private_feeds(&paths)
}

/// 保存「允许内网订阅源」开关。
#[tauri::command(async)]
pub(crate) fn rss_settings_set_allow_private_feeds(
    state: State<'_, SharedHost>,
    allow: bool,
) -> Result<bool, String> {
    let paths = {
        let host = state.lock().map_err(|_| "宿主状态锁不可用。".to_string())?;
        host.paths.clone()
    };
    rss_store::set_allow_private_feeds(&paths, allow)
}

/// 单篇 RSS 文章正文回填：重新抓取所属 feed，按 guid/link 匹配后补齐媒体标记。
#[tauri::command(async)]
pub(crate) async fn rss_refresh_article(
    state: State<'_, SharedHost>,
    book_id: String,
) -> Result<rss_store::RssArticleRefreshResult, String> {
    let paths = {
        let host = state.lock().map_err(|_| "宿主状态锁不可用。".to_string())?;
        host.paths.clone()
    };
    tauri::async_runtime::spawn_blocking(move || rss_store::refresh_article(&paths, &book_id))
        .await
        .map_err(|e| format!("RSS 文章回填任务异常退出：{e}"))?
}

#[tauri::command(async)]
pub(crate) async fn rss_fetch(
    state: State<'_, SharedHost>,
    subscription_id: Option<String>,
) -> Result<rss_store::RssFetchReport, String> {
    let paths = {
        let host = state.lock().unwrap();
        host.paths.clone()
    };
    tauri::async_runtime::spawn_blocking(move || {
        rss_store::fetch(&paths, subscription_id.as_deref())
    })
    .await
    .map_err(|e| format!("RSS 拉取任务异常退出：{e}"))?
}
