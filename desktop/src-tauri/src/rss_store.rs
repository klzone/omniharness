//! 健脑室的 RSS 订阅与文章入库（OD-READ-002 阶段 1）。
//!
//! 数据只落仓库外的阅读目录：
//!   rss/subscriptions.json          订阅清单（schema omnidesk.rss.v1）
//!   rss/articles/<book_id>.json     文章元数据（已读状态、原链接、订阅来源）
//!   books/<book_id>.json            与普通书共用的 ReadingBook 正文
//!
//! 边界：只对用户添加或预置清单里的 URL 发 HTTP GET；不把正文写进日志；
//! 同一 feed_url + entry key 只入库一次，不覆盖已有批注。
//!
//! RSS/Atom 解析与预置清单结构思路借鉴自 qiaomu-rss-dsh（GPL-3.0），未复制代码。
//! 独立博客清单来源：timqian/chinese-independent-blogs，MIT License。

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app_paths::AppPaths;
use crate::reading_store::{
    ensure_store_dirs, harden_private, read_book_at, write_book, ReadingBook, ReadingChapter,
    ReadingProgress, READING_SCHEMA,
};
use crate::sha256::sha256_hex;
use crate::task_shell::utc_iso8601;
use crate::worklog_records::{is_safe_id, write_new_atomically, write_replace_atomically};

pub const RSS_SCHEMA: &str = "omnidesk.rss.v1";
const SUBSCRIPTIONS_FILE: &str = "subscriptions.json";
/// 单次抓取上限：部分博客的 feed 单篇全文较大（实测有 3–10 MB），2 MiB 会误杀。
const MAX_FETCH_BYTES: usize = 8 * 1024 * 1024;
const MAX_ENTRIES_PARSED: usize = 200;
const MAX_ENTRIES_KEPT: usize = 50;
const MAX_ARTICLE_CHARS: usize = 100_000;
const FETCH_TIMEOUT_SECS: u64 = 20;
const DEFAULT_GROUP: &str = "我的订阅";

const PRESET_GROUP_AUTHORS: &str = "精选作者";
const PRESET_GROUP_BLOGS: &str = "独立博客";

struct PresetFeed {
    name: &'static str,
    url: &'static str,
    group: &'static str,
}

/// 精选作者 9 条，取自 qiaomu-rss-dsh 的 featured 数组（只读借鉴清单事实）。
const PRESET_FEEDS: &[PresetFeed] = &[
    PresetFeed { name: "潮流周刊 · Tw93", url: "https://weekly.tw93.fun/rss.xml", group: PRESET_GROUP_AUTHORS },
    PresetFeed { name: "阮一峰的网络日志", url: "https://www.ruanyifeng.com/blog/atom.xml", group: PRESET_GROUP_AUTHORS },
    PresetFeed { name: "云风", url: "https://blog.codingnow.com/atom.xml", group: PRESET_GROUP_AUTHORS },
    PresetFeed { name: "和菜头", url: "https://www.hecaitou.com/feeds/posts/default?alt=rss", group: PRESET_GROUP_AUTHORS },
    PresetFeed { name: "张鑫旭", url: "https://www.zhangxinxu.com/wordpress/feed/", group: PRESET_GROUP_AUTHORS },
    PresetFeed { name: "小众软件", url: "https://www.appinn.com/feed/", group: PRESET_GROUP_AUTHORS },
    PresetFeed { name: "月光博客", url: "https://www.williamlong.info/rss.xml", group: PRESET_GROUP_AUTHORS },
    PresetFeed { name: "Reorx", url: "https://reorx.com/feed.xml", group: PRESET_GROUP_AUTHORS },
    PresetFeed { name: "pseudoyu", url: "https://www.pseudoyu.com/zh/index.xml", group: PRESET_GROUP_AUTHORS },
    // 以下独立博客来自 timqian/chinese-independent-blogs（MIT License），按 tag「编程/读书」挑选。
    PresetFeed { name: "透明创业实验", url: "https://blog.t9t.io/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "酷 壳 – CoolShell", url: "http://coolshell.cn/feed", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "Alili丶前端大爆炸", url: "https://alili.tech/index.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "蚊子前端博客", url: "https://www.xiabingbao.com/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "DIYGod - 写代码是热爱，写到世界充满爱!", url: "https://diygod.me/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "MacTalk-池建强的随想录", url: "http://macshuo.com/?feed=rss2", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "ShrekShao", url: "http://shrekshao.github.io/feed.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "ZDDHUB 的博客", url: "https://zddhub.com/feed", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "全栈应用开发:精益实践", url: "https://www.phodal.com/blog/feeds/rss/", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "Python 工匠", url: "https://www.piglei.com/feeds/latest/", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "但行好事，莫问前程", url: "https://windard.com/feed.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "Ric's Blog", url: "https://www.lichong.work/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "罗磊的独立博客", url: "http://luolei.org/feed/", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "代码家", url: "https://daimajia.com/feed", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "开源实验室", url: "https://www.kymjs.com/feed.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "技术小黑屋", url: "https://droidyue.com/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "依云's Blog", url: "https://blog.lilydjwg.me/posts.rss", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "朽儿", url: "https://xiuer.medium.com/feed", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "王登科-DK博客", url: "https://greatdk.com/feed", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "chai2010 的博客", url: "https://chai2010.cn/index.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "笨方法学写作", url: "https://www.cnfeat.com/feed.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "云原生", url: "https://jimmysong.io/index.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "Hawstein's Blog", url: "http://hawstein.com/feed.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "Skywind Inside", url: "http://www.skywind.me/blog/feed", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "陈沙克日志", url: "http://www.chenshake.com/feed/", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "Randy's Blog", url: "https://lutaonan.com/rss.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "iTimothy", url: "https://xiaozhou.net/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "小胡子哥的个人网站", url: "http://www.barretlee.com/rss2.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "晚晴幽草轩", url: "https://www.jeffjade.com/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "the5fire的技术博客", url: "http://www.the5fire.com/rss", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "forecho 的独立博客", url: "https://blog.forecho.com/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "Debug客栈", url: "https://blog.debuginn.com/index.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "我爱自然语言处理", url: "http://www.52nlp.cn/feed", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "鸟窝", url: "https://colobu.com/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "OneV's Den", url: "https://onevcat.com/feed.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "唐巧的博客", url: "https://blog.devtang.com/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "GeekPlux", url: "https://geekplux.com/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "王子亭的博客", url: "https://jysperm.me/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "囧克斯 勾三股四", url: "https://jiongks.name/atom.xml", group: PRESET_GROUP_BLOGS },
    PresetFeed { name: "张戈博客", url: "https://zhang.ge/feed", group: PRESET_GROUP_BLOGS },
];

/// 健脑室空态展示的精选作者；只暴露作者组，不暴露独立博客与公众号。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RssPresetSource {
    pub name: String,
    pub url: String,
    pub group: String,
}

pub fn preset_sources() -> Vec<RssPresetSource> {
    PRESET_FEEDS
        .iter()
        .filter(|preset| preset.group == PRESET_GROUP_AUTHORS)
        .map(|preset| RssPresetSource {
            name: preset.name.to_string(),
            url: preset.url.to_string(),
            group: preset.group.to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RssSubscription {
    pub id: String,
    pub url: String,
    pub name: String,
    pub group: String,
    pub added_at: String,
    #[serde(default)]
    pub last_fetched_at: String,
    #[serde(default)]
    pub last_error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RssArticleMeta {
    #[serde(default = "schema_default")]
    pub schema: String,
    pub book_id: String,
    pub subscription_id: String,
    pub feed_url: String,
    pub guid: String,
    pub link: String,
    pub title: String,
    pub author: String,
    pub published_at: String,
    #[serde(default)]
    pub published_ts: i64,
    pub fetched_at: String,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub favorite: bool,
    /// 条目附带的非图片媒体（播客音频、视频、PDF 等）；旧文件缺省为空。
    #[serde(default)]
    pub attachments: Vec<RssAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RssAttachment {
    pub url: String,
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RssFetchFailure {
    pub subscription_id: String,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RssFetchReport {
    pub fetched: usize,
    pub failed: usize,
    pub added: usize,
    pub skipped: usize,
    pub errors: Vec<RssFetchFailure>,
}

/// 单篇 RSS 文章的正文回填结果：只在无批注且匹配到新条目时改写正文。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RssArticleRefreshResult {
    pub updated: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RssStoreFile {
    #[serde(default = "schema_default")]
    schema: String,
    #[serde(default)]
    subscriptions: Vec<RssSubscription>,
    /// 「允许内网订阅源」开关（OD-HARDEN-2043）。默认关：私网与 CGNAT 地址一律拒绝。
    /// 回环、链路本地、未指定、多播不受此开关影响，始终拒绝。
    #[serde(default)]
    allow_private_feeds: bool,
}

fn schema_default() -> String {
    RSS_SCHEMA.to_string()
}

fn policy_for(store: &RssStoreFile) -> crate::net_guard::NetPolicy {
    if store.allow_private_feeds {
        crate::net_guard::NetPolicy::AllowPrivate
    } else {
        crate::net_guard::NetPolicy::PublicOnly
    }
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn hash8(value: &str) -> String {
    sha256_hex(value.as_bytes()).chars().take(8).collect()
}

fn subscription_id(url: &str) -> String {
    hash8(url)
}

fn rss_dir(root: &Path) -> PathBuf {
    root.join("rss")
}

fn articles_dir(root: &Path) -> PathBuf {
    rss_dir(root).join("articles")
}

fn subscriptions_path(root: &Path) -> PathBuf {
    rss_dir(root).join(SUBSCRIPTIONS_FILE)
}

fn ensure_rss_dirs(root: &Path) -> Result<(), String> {
    ensure_store_dirs(root)?;
    let rss = rss_dir(root);
    let articles = articles_dir(root);
    fs::create_dir_all(&articles)
        .map_err(|e| format!("创建 RSS 文章目录失败（{}）：{}", articles.display(), e))?;
    harden_private(&rss, 0o700);
    harden_private(&articles, 0o700);
    Ok(())
}

fn validate_feed_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("请填写 RSS / Atom 地址。".to_string());
    }
    if trimmed.chars().count() > 2_048 {
        return Err("订阅地址过长（上限 2048 字符）。".to_string());
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("订阅地址不能包含空白或控制字符。".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err("订阅地址必须以 http:// 或 https:// 开头。".to_string());
    }
    Ok(trimmed.to_string())
}

fn feed_name_from_url(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = after_scheme.split('/').next().unwrap_or("").trim();
    if host.is_empty() {
        DEFAULT_GROUP.to_string()
    } else {
        host.to_string()
    }
}

fn preset_store(now: f64) -> RssStoreFile {
    let added_at = utc_iso8601(now);
    RssStoreFile {
        schema: RSS_SCHEMA.to_string(),
        subscriptions: PRESET_FEEDS
            .iter()
            .map(|preset| RssSubscription {
                id: subscription_id(preset.url),
                url: preset.url.to_string(),
                name: preset.name.to_string(),
                group: preset.group.to_string(),
                added_at: added_at.clone(),
                last_fetched_at: String::new(),
                last_error: String::new(),
            })
            .collect(),
        allow_private_feeds: false,
    }
}

fn save_store_at(root: &Path, store: &RssStoreFile) -> Result<(), String> {
    ensure_rss_dirs(root)?;
    let path = subscriptions_path(root);
    let raw = serde_json::to_vec_pretty(store).map_err(|e| format!("序列化订阅清单失败：{e}"))?;
    if path.exists() {
        write_replace_atomically(&path, &raw)?;
    } else {
        write_new_atomically(&path, &raw)?;
    }
    harden_private(&path, 0o600);
    Ok(())
}

/// 首次访问时写入预置清单；文件一旦存在就不再补种，因此用户删除的预置项不会复活。
pub(crate) fn seed_store_at(root: &Path, now: f64) -> Result<RssStoreFile, String> {
    ensure_rss_dirs(root)?;
    let path = subscriptions_path(root);
    if path.exists() {
        return load_store_at(root);
    }
    let store = preset_store(now);
    save_store_at(root, &store)?;
    Ok(store)
}

fn load_store_at(root: &Path) -> Result<RssStoreFile, String> {
    ensure_rss_dirs(root)?;
    let path = subscriptions_path(root);
    if !path.exists() {
        return seed_store_at(root, now_secs());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取订阅清单失败（{}）：{}", path.display(), e))?;
    let store: RssStoreFile = serde_json::from_str(&raw)
        .map_err(|e| format!("订阅清单格式不正确（{}）：{}", path.display(), e))?;
    if store.schema != RSS_SCHEMA {
        return Err(format!(
            "订阅清单版本不受支持：{}（期望 {}）",
            store.schema, RSS_SCHEMA
        ));
    }
    Ok(store)
}

pub fn list_subscriptions(paths: &AppPaths) -> Result<Vec<RssSubscription>, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    Ok(load_store_at(&root)?.subscriptions)
}

/// 读取「允许内网订阅源」开关；默认关。
pub fn allow_private_feeds(paths: &AppPaths) -> Result<bool, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    Ok(load_store_at(&root)?.allow_private_feeds)
}

/// 保存「允许内网订阅源」开关。只影响订阅源；正文图片/媒体始终只允许公网。
pub fn set_allow_private_feeds(paths: &AppPaths, allow: bool) -> Result<bool, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    let mut store = load_store_at(&root)?;
    store.allow_private_feeds = allow;
    save_store_at(&root, &store)?;
    Ok(store.allow_private_feeds)
}

pub fn add_subscription(
    paths: &AppPaths,
    url: &str,
    name: Option<&str>,
) -> Result<RssSubscription, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    add_subscription_at(&root, url, name, now_secs())
}

fn add_subscription_at(
    root: &Path,
    url: &str,
    name: Option<&str>,
    now: f64,
) -> Result<RssSubscription, String> {
    let url = validate_feed_url(url)?;
    let mut store = load_store_at(root)?;
    if let Some(existing) = store.subscriptions.iter().find(|item| item.url == url) {
        return Ok(existing.clone());
    }
    let id = subscription_id(&url);
    if store.subscriptions.iter().any(|item| item.id == id) {
        return Err("订阅编号与已有来源冲突，请换一个地址。".to_string());
    }
    let name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| clamp_chars(value, 120))
        .unwrap_or_else(|| feed_name_from_url(&url));
    let subscription = RssSubscription {
        id,
        url,
        name,
        group: DEFAULT_GROUP.to_string(),
        added_at: utc_iso8601(now),
        last_fetched_at: String::new(),
        last_error: String::new(),
    };
    store.subscriptions.push(subscription.clone());
    save_store_at(root, &store)?;
    Ok(subscription)
}

pub fn remove_subscription(paths: &AppPaths, id: &str) -> Result<(), String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    remove_subscription_at(&root, id)
}

fn remove_subscription_at(root: &Path, id: &str) -> Result<(), String> {
    if !is_safe_id(id) {
        return Err(format!("订阅编号不合法：{id:?}"));
    }
    let mut store = load_store_at(root)?;
    let before = store.subscriptions.len();
    store.subscriptions.retain(|item| item.id != id);
    if store.subscriptions.len() == before {
        return Err(format!("订阅不存在：{id}"));
    }
    save_store_at(root, &store)
}

pub fn list_articles(paths: &AppPaths) -> Result<Vec<RssArticleMeta>, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    list_articles_at(&root)
}

/// 首页统计用的文章篇数；只数真实入库的 RSS 文章元数据。
pub fn article_count(paths: &AppPaths) -> Result<usize, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    article_count_at(&root)
}

pub fn article_count_at(root: &Path) -> Result<usize, String> {
    Ok(list_articles_at(root)?.len())
}

fn list_articles_at(root: &Path) -> Result<Vec<RssArticleMeta>, String> {
    ensure_rss_dirs(root)?;
    let dir = articles_dir(root);
    let mut items = Vec::new();
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("读取 RSS 文章目录失败（{}）：{}", dir.display(), e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else { continue };
        if let Ok(item) = serde_json::from_str::<RssArticleMeta>(&raw) {
            items.push(item);
        }
    }
    items.sort_by(|a, b| {
        b.published_ts
            .cmp(&a.published_ts)
            .then_with(|| b.fetched_at.cmp(&a.fetched_at))
            .then_with(|| a.title.cmp(&b.title))
    });
    Ok(items)
}

pub fn mark_article_read(paths: &AppPaths, book_id: &str) -> Result<RssArticleMeta, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    mark_article_read_at(&root, book_id)
}

/// 切换文章收藏；只改 articles/<book_id>.json，不触碰正文与批注。
pub fn set_article_favorite(
    paths: &AppPaths,
    book_id: &str,
    favorite: bool,
) -> Result<RssArticleMeta, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    set_article_favorite_at(&root, book_id, favorite)
}

fn set_article_favorite_at(
    root: &Path,
    book_id: &str,
    favorite: bool,
) -> Result<RssArticleMeta, String> {
    let path = article_path(root, book_id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取 RSS 文章元数据失败（{}）：{}", path.display(), e))?;
    let mut meta: RssArticleMeta = serde_json::from_str(&raw)
        .map_err(|e| format!("RSS 文章元数据格式不正确（{}）：{}", path.display(), e))?;
    if meta.favorite == favorite {
        return Ok(meta);
    }
    meta.favorite = favorite;
    let bytes = serde_json::to_vec_pretty(&meta).map_err(|e| format!("序列化 RSS 文章元数据失败：{e}"))?;
    write_replace_atomically(&path, &bytes)?;
    harden_private(&path, 0o600);
    Ok(meta)
}

fn mark_article_read_at(root: &Path, book_id: &str) -> Result<RssArticleMeta, String> {
    let path = article_path(root, book_id)?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取 RSS 文章元数据失败（{}）：{}", path.display(), e))?;
    let mut meta: RssArticleMeta = serde_json::from_str(&raw)
        .map_err(|e| format!("RSS 文章元数据格式不正确（{}）：{}", path.display(), e))?;
    if meta.read {
        return Ok(meta);
    }
    meta.read = true;
    let bytes = serde_json::to_vec_pretty(&meta).map_err(|e| format!("序列化 RSS 文章元数据失败：{e}"))?;
    write_replace_atomically(&path, &bytes)?;
    harden_private(&path, 0o600);
    Ok(meta)
}

fn article_path(root: &Path, book_id: &str) -> Result<PathBuf, String> {
    if !is_safe_id(book_id) {
        return Err(format!("文章编号不合法：{book_id:?}"));
    }
    Ok(articles_dir(root).join(format!("{book_id}.json")))
}

fn write_article_meta_at(root: &Path, meta: &RssArticleMeta) -> Result<(), String> {
    let path = article_path(root, &meta.book_id)?;
    let bytes = serde_json::to_vec_pretty(meta)
        .map_err(|e| format!("序列化 RSS 文章元数据失败：{e}"))?;
    write_replace_atomically(&path, &bytes)?;
    harden_private(&path, 0o600);
    Ok(())
}

fn reading_book_exists_at(root: &Path, book_id: &str) -> bool {
    is_safe_id(book_id) && root.join("books").join(format!("{book_id}.json")).exists()
}

fn read_article_meta_at(root: &Path, book_id: &str) -> Result<Option<RssArticleMeta>, String> {
    let path = article_path(root, book_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("读取 RSS 文章元数据失败（{}）：{}", path.display(), e))?;
    let meta = serde_json::from_str(&raw)
        .map_err(|e| format!("RSS 文章元数据格式不正确（{}）：{}", path.display(), e))?;
    Ok(Some(meta))
}

/// 单篇 RSS 文章回填：重新抓取所属 feed，按 guid/link 匹配条目后重算正文。
/// 只有文章没有批注时才改写 chapters/author/source_sha256，progress 与已读状态原样保留。
pub fn refresh_article(paths: &AppPaths, book_id: &str) -> Result<RssArticleRefreshResult, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    refresh_article_at(&root, book_id)
}

fn refresh_article_at(root: &Path, book_id: &str) -> Result<RssArticleRefreshResult, String> {
    let not_updated = |reason: &str| RssArticleRefreshResult {
        updated: false,
        reason: reason.to_string(),
    };
    if !is_safe_id(book_id) {
        return Err(format!("文章编号不合法：{book_id:?}"));
    }
    let Some(meta) = read_article_meta_at(root, book_id)? else {
        return Ok(not_updated("没有该 RSS 文章的元数据，无法重新抓取。"));
    };
    if !reading_book_exists_at(root, book_id) {
        return Ok(not_updated("没有找到文章正文文件。"));
    }
    let book = read_book_at(root, book_id)?;
    if book.source_format != "rss" {
        return Ok(not_updated("这不是订阅文章，跳过回填。"));
    }
    if !book.annotations.is_empty() {
        return Ok(not_updated("文章已有批注，未回填正文。"));
    }
    let Some(feed_url) = subscription_feed_url(root, &meta) else {
        return Ok(not_updated("缺少可用的订阅源地址。"));
    };
    let policy = load_store_at(root)
        .map(|store| policy_for(&store))
        .unwrap_or(crate::net_guard::NetPolicy::PublicOnly);
    let xml = fetch_feed_xml(&feed_url, policy)?;
    refresh_article_with_xml_at(root, book_id, &xml)
}

fn subscription_feed_url(root: &Path, meta: &RssArticleMeta) -> Option<String> {
    if let Ok(url) = validate_feed_url(&meta.feed_url) {
        return Some(url);
    }
    if !subscriptions_path(root).exists() {
        return None;
    }
    let store = load_store_at(root).ok()?;
    store
        .subscriptions
        .into_iter()
        .find(|item| item.id == meta.subscription_id || item.url == meta.feed_url)
        .map(|item| item.url)
}

fn refresh_article_with_xml_at(
    root: &Path,
    book_id: &str,
    xml: &str,
) -> Result<RssArticleRefreshResult, String> {
    let not_updated = |reason: &str| RssArticleRefreshResult {
        updated: false,
        reason: reason.to_string(),
    };
    let Some(meta) = read_article_meta_at(root, book_id)? else {
        return Ok(not_updated("没有该 RSS 文章的元数据，无法重新抓取。"));
    };
    let mut book = match read_book_at(root, book_id) {
        Ok(book) => book,
        Err(_) => return Ok(not_updated("没有找到文章正文文件。")),
    };
    if book.source_format != "rss" {
        return Ok(not_updated("这不是订阅文章，跳过回填。"));
    }
    if !book.annotations.is_empty() {
        return Ok(not_updated("文章已有批注，未回填正文。"));
    }
    let entries = parse_feed(xml)?;
    let matched = entries.into_iter().find(|entry| {
        let guid_hit = !meta.guid.trim().is_empty()
            && entry.guid.trim() == meta.guid.trim();
        let link_hit = !meta.link.trim().is_empty()
            && entry.link.trim() == meta.link.trim();
        guid_hit || link_hit
    });
    let Some(entry) = matched else {
        return Ok(not_updated("订阅源中没有匹配的文章条目（guid/link 不一致）。"));
    };
    if meta.attachments != entry.attachments {
        let mut updated = meta.clone();
        updated.attachments = entry.attachments.clone();
        write_article_meta_at(root, &updated)?;
    }
    let base_url = if !entry.link.trim().is_empty() {
        entry.link.trim().to_string()
    } else if !meta.link.trim().is_empty() {
        meta.link.trim().to_string()
    } else {
        meta.feed_url.trim().to_string()
    };
    let mut paragraphs = html_to_paragraphs(&entry.content_html, &base_url);
    let has_media = paragraphs
        .iter()
        .any(|item| item.contains("![img]") || item.contains("odmedia:"));
    if !has_media && !entry.image_url.is_empty() {
        paragraphs.insert(0, format!("![img]({})", markdown_url(&entry.image_url)));
    }
    if paragraphs.is_empty() {
        return Ok(not_updated("订阅源正文为空，未回填。"));
    }
    let existing: Vec<String> = book
        .chapters
        .iter()
        .flat_map(|chapter| chapter.paragraphs.iter().cloned())
        .collect();
    if paragraphs == existing {
        return Ok(not_updated("订阅源正文与本地一致，无需回填。"));
    }
    if !has_media && paragraphs.iter().any(|item| item.contains("[图片：")) {
        return Ok(not_updated("订阅源正文仍含旧图片占位，未找到可用图片地址。"));
    }
    let author = if entry.author.trim().is_empty() {
        book.author.clone()
    } else {
        clamp_chars(entry.author.trim(), 200)
    };
    book.chapters = vec![ReadingChapter {
        title: "正文".to_string(),
        paragraphs,
    }];
    book.author = author;
    book.source_sha256 = sha256_hex(entry.content_html.as_bytes());
    book.media_version = 1;
    write_book(root, &book)?;
    Ok(RssArticleRefreshResult {
        updated: true,
        reason: String::new(),
    })
}

pub fn fetch(paths: &AppPaths, subscription_id: Option<&str>) -> Result<RssFetchReport, String> {
    let root = crate::reading_store::resolve_reading_root(paths)?;
    let mut store = load_store_at(&root)?;
    let targets: Vec<usize> = match subscription_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => {
            if !is_safe_id(id) {
                return Err(format!("订阅编号不合法：{id:?}"));
            }
            let index = store
                .subscriptions
                .iter()
                .position(|item| item.id == id)
                .ok_or_else(|| format!("订阅不存在：{id}"))?;
            vec![index]
        }
        None => (0..store.subscriptions.len()).collect(),
    };
    let policy = policy_for(&store);
    let mut report = RssFetchReport {
        fetched: 0,
        failed: 0,
        added: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    let fetched_at = utc_iso8601(now_secs());
    for index in targets {
        let subscription = store.subscriptions[index].clone();
        match fetch_feed_xml(&subscription.url, policy) {
            Ok(xml) => {
                let summary = ingest_feed_at(&root, &subscription, &xml, now_secs());
                match summary {
                    Ok(summary) => {
                        report.fetched += 1;
                        report.added += summary.added;
                        report.skipped += summary.skipped;
                        for message in summary.errors {
                            report.errors.push(RssFetchFailure {
                                subscription_id: subscription.id.clone(),
                                name: subscription.name.clone(),
                                message,
                            });
                        }
                        if let Some(item) = store.subscriptions.get_mut(index) {
                            item.last_fetched_at = fetched_at.clone();
                            item.last_error.clear();
                        }
                    }
                    Err(error) => {
                        report.failed += 1;
                        if let Some(item) = store.subscriptions.get_mut(index) {
                            item.last_fetched_at = fetched_at.clone();
                            item.last_error = error.clone();
                        }
                        report.errors.push(RssFetchFailure {
                            subscription_id: subscription.id.clone(),
                            name: subscription.name.clone(),
                            message: error,
                        });
                    }
                }
            }
            Err(error) => {
                report.failed += 1;
                if let Some(item) = store.subscriptions.get_mut(index) {
                    item.last_fetched_at = fetched_at.clone();
                    item.last_error = error.clone();
                }
                report.errors.push(RssFetchFailure {
                    subscription_id: subscription.id.clone(),
                    name: subscription.name.clone(),
                    message: error,
                });
            }
        }
    }
    save_store_at(&root, &store)?;
    Ok(report)
}

/// 订阅源最多 3 跳重定向（OD-HARDEN-2043）。
const MAX_FEED_REDIRECTS: usize = 3;

/// 每一跳都重新校验地址并钉住解析结果，防 DNS 重绑定。
fn feed_client(
    policy: crate::net_guard::NetPolicy,
    url: &reqwest::Url,
) -> Result<reqwest::blocking::Client, String> {
    let addrs = crate::net_guard::assert_url_addresses(url, policy)?;
    let builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(10))
        .user_agent("OmniDesk-RSS/0.1 (+https://omnidesk.local)")
        .redirect(reqwest::redirect::Policy::none());
    crate::net_guard::pin_client(builder, url, &addrs)
        .build()
        .map_err(|e| format!("创建 RSS 网络客户端失败：{e}"))
}

fn fetch_feed_xml(url: &str, policy: crate::net_guard::NetPolicy) -> Result<String, String> {
    let url = validate_feed_url(url)?;
    let mut current =
        reqwest::Url::parse(&url).map_err(|_| "订阅地址不是有效的网址。".to_string())?;
    let mut hops = 0usize;
    loop {
        let client = feed_client(policy, &current)?;
        let response = client
        .get(current.clone())
        .header(
            reqwest::header::ACCEPT,
            "application/rss+xml, application/atom+xml, application/xml, text/xml, */*",
        )
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                format!("拉取超时（单请求 {}s）：{e}", FETCH_TIMEOUT_SECS)
            } else if e.is_connect() {
                format!("网络连接失败（DNS/连接错误）：{e}")
            } else {
                format!("请求失败：{e}")
            }
        })?;
        let status = response.status();
        if status.is_redirection() {
            if hops >= MAX_FEED_REDIRECTS {
                return Err(format!("订阅地址重定向超过 {} 次。", MAX_FEED_REDIRECTS));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "订阅重定向缺少 Location。".to_string())?;
            let next = current
                .join(location)
                .map_err(|_| "订阅重定向地址无效。".to_string())?;
            if !crate::net_guard::downgrade_permitted(&current, &next) {
                return Err("订阅地址不允许从 https 降级到 http。".to_string());
            }
            if !matches!(next.scheme(), "http" | "https") {
                return Err("订阅重定向只允许 http/https。".to_string());
            }
            // 下一跳在 feed_client 里重新校验并重新钉住。
            current = next;
            hops += 1;
            continue;
        }
        if !status.is_success() {
            return Err(format!("HTTP 状态 {}：{}", status.as_u16(), current));
        }
        if let Some(length) = response.content_length() {
            if length > MAX_FETCH_BYTES as u64 {
                return Err(format!(
                    "响应体 {} KB，超过 {} KB 上限。",
                    length / 1024,
                    MAX_FETCH_BYTES / 1024
                ));
            }
        }
        let mut reader = response.take((MAX_FETCH_BYTES + 1) as u64);
        let mut body = Vec::new();
        reader
            .read_to_end(&mut body)
            .map_err(|e| format!("读取响应体失败：{e}"))?;
        if body.len() > MAX_FETCH_BYTES {
            return Err(format!(
                "响应体超过 {} KB 上限，已拒绝解析。",
                MAX_FETCH_BYTES / 1024
            ));
        }
        return Ok(String::from_utf8_lossy(&body).to_string());
    }
}

#[derive(Debug, Clone, PartialEq)]
struct FeedEntry {
    key: String,
    guid: String,
    link: String,
    title: String,
    published_at: String,
    published_ts: i64,
    author: String,
    content_html: String,
    summary: String,
    image_url: String,
    attachments: Vec<RssAttachment>,
}

#[derive(Default)]
struct IngestSummary {
    added: usize,
    skipped: usize,
    errors: Vec<String>,
}

fn ingest_feed_at(
    root: &Path,
    subscription: &RssSubscription,
    xml: &str,
    now: f64,
) -> Result<IngestSummary, String> {
    let mut entries = parse_feed(xml)?;
    entries.sort_by(|a, b| b.published_ts.cmp(&a.published_ts));
    entries.truncate(MAX_ENTRIES_KEPT);
    ensure_rss_dirs(root)?;
    let fetched_at = utc_iso8601(now);
    let mut summary = IngestSummary::default();
    for entry in entries {
        let key = if entry.key.trim().is_empty() {
            hash8(&format!("{}\n{}\n{}", entry.title, entry.link, entry.published_at))
        } else {
            clamp_chars(entry.key.trim(), 500)
        };
        let book_id = format!(
            "rss-{}-{}",
            hash8(&format!("{}\n{}", subscription.url, key)),
            hash8(&key)
        );
        if reading_book_exists_at(root, &book_id) || article_path(root, &book_id)?.exists() {
            if let Ok(Some(mut meta)) = read_article_meta_at(root, &book_id) {
                if meta.attachments != entry.attachments {
                    meta.attachments = entry.attachments.clone();
                    write_article_meta_at(root, &meta)?;
                }
            }
            if let Ok(mut book) = read_book_at(root, &book_id) {
                if book.source_format == "rss" && book.annotations.is_empty() {
                    let mut paragraphs = html_to_paragraphs(&entry.content_html, &entry.link);
                    let has_media = paragraphs
                        .iter()
                        .any(|item| item.contains("![img]") || item.contains("odmedia:"));
                    if !has_media && !entry.image_url.is_empty() {
                        paragraphs.insert(
                            0,
                            format!("![img]({})", markdown_url(&entry.image_url)),
                        );
                    }
                    if has_media || !entry.image_url.is_empty() {
                        let author = if entry.author.trim().is_empty() {
                            subscription.name.clone()
                        } else {
                            clamp_chars(entry.author.trim(), 200)
                        };
                        book.chapters = vec![ReadingChapter {
                            title: "正文".to_string(),
                            paragraphs,
                        }];
                        book.author = author;
                        book.source_sha256 = sha256_hex(entry.content_html.as_bytes());
                        book.media_version = 1;
                        write_book(root, &book)?;
                    }
                }
            }
            summary.skipped += 1;
            continue;
        }
        let mut paragraphs = html_to_paragraphs(&entry.content_html, &entry.link);
        let has_media = paragraphs
            .iter()
            .any(|item| item.contains("![img]") || item.contains("odmedia:"));
        if !has_media && !entry.image_url.is_empty() {
            paragraphs.insert(
                0,
                format!("![img]({})", markdown_url(&entry.image_url)),
            );
        }
        if paragraphs.is_empty() {
            summary.skipped += 1;
            continue;
        }
        let title = if entry.title.trim().is_empty() {
            "无标题文章".to_string()
        } else {
            clamp_chars(entry.title.trim(), 300)
        };
        let author = if entry.author.trim().is_empty() {
            subscription.name.clone()
        } else {
            clamp_chars(entry.author.trim(), 200)
        };
        let source_sha256 = sha256_hex(entry.content_html.as_bytes());
        let book = ReadingBook {
            schema: READING_SCHEMA.to_string(),
            book_id: book_id.clone(),
            title: title.clone(),
            author: author.clone(),
            source_name: subscription.name.clone(),
            source_format: "rss".to_string(),
            source_path: clamp_chars(&entry.link, 2_000),
            source_sha256,
            cover: String::new(),
            imported_at: fetched_at.clone(),
            updated_at: fetched_at.clone(),
            chapters: vec![ReadingChapter {
                title: "正文".to_string(),
                paragraphs,
            }],
            annotations: Vec::new(),
            notes: Vec::new(),
            progress: ReadingProgress::default(),
            media_version: 1,
        };
        write_book(root, &book)?;
        let meta = RssArticleMeta {
            schema: RSS_SCHEMA.to_string(),
            book_id: book_id.clone(),
            subscription_id: subscription.id.clone(),
            feed_url: subscription.url.clone(),
            guid: clamp_chars(&entry.guid, 2_000),
            link: clamp_chars(&entry.link, 2_000),
            title,
            author,
            published_at: clamp_chars(&entry.published_at, 120),
            published_ts: entry.published_ts,
            fetched_at: fetched_at.clone(),
            read: false,
            favorite: false,
            attachments: entry.attachments.clone(),
        };
        let path = article_path(root, &book_id)?;
        let raw = serde_json::to_vec_pretty(&meta)
            .map_err(|e| format!("序列化 RSS 文章元数据失败：{e}"))?;
        write_new_atomically(&path, &raw)?;
        harden_private(&path, 0o600);
        summary.added += 1;
    }
    Ok(summary)
}

#[derive(Debug, Default)]
struct XmlNode {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<XmlNode>,
    text: String,
}

fn parse_xml(input: &str) -> XmlNode {
    let mut root = XmlNode {
        name: "#document".to_string(),
        ..XmlNode::default()
    };
    let mut stack: Vec<XmlNode> = vec![XmlNode {
        name: "#document".to_string(),
        ..XmlNode::default()
    }];
    let mut index = 0usize;
    while index < input.len() {
        let rest = &input[index..];
        if let Some(position) = rest.find('<') {
            if position > 0 {
                let text = decode_entities(&rest[..position]);
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&text);
                }
                index += position;
                continue;
            }
        } else {
            let text = decode_entities(rest);
            if let Some(current) = stack.last_mut() {
                current.text.push_str(&text);
            }
            break;
        }
        let rest = &input[index..];
        if rest.starts_with("<!--") {
            index += rest.find("-->").map(|p| p + 3).unwrap_or(rest.len());
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            let end = rest.find("]]>").unwrap_or(rest.len());
            let content = &rest[9..end.min(rest.len())];
            if let Some(current) = stack.last_mut() {
                current.text.push_str(content);
            }
            index += if end < rest.len() { end + 3 } else { rest.len() };
            continue;
        }
        if rest.starts_with("<?") {
            index += rest.find("?>").map(|p| p + 2).unwrap_or(rest.len());
            continue;
        }
        if rest.starts_with("<!") {
            index += rest.find('>').map(|p| p + 1).unwrap_or(rest.len());
            continue;
        }
        let Some(end) = find_tag_end(rest) else {
            let text = decode_entities(rest);
            if let Some(current) = stack.last_mut() {
                current.text.push_str(&text);
            }
            break;
        };
        let inner = &rest[1..end];
        if let Some(name) = inner.strip_prefix('/') {
            let closing = local_name(name.trim());
            if let Some(position) = stack.iter().rposition(|node| node.name == closing) {
                while stack.len() > position + 1 {
                    let Some(node) = stack.pop() else { break };
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    }
                }
                let Some(node) = stack.pop() else { break };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                }
            }
            index += end + 1;
            continue;
        }
        let (name, attrs, self_closing) = parse_start_tag(inner);
        if name.is_empty() {
            if let Some(current) = stack.last_mut() {
                current.text.push('<');
            }
            index += 1;
            continue;
        }
        let node = XmlNode {
            name,
            attrs,
            children: Vec::new(),
            text: String::new(),
        };
        if self_closing {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            }
        } else {
            stack.push(node);
        }
        index += end + 1;
    }
    while stack.len() > 1 {
        let Some(node) = stack.pop() else { break };
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        }
    }
    root = stack.pop().unwrap_or(root);
    root
}

fn find_tag_end(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let mut quote: Option<u8> = None;
    for (i, byte) in bytes.iter().enumerate().skip(1) {
        if let Some(q) = quote {
            if *byte == q {
                quote = None;
            }
            continue;
        }
        if *byte == b'\"' || *byte == b'\'' {
            quote = Some(*byte);
        } else if *byte == b'>' {
            return Some(i);
        }
    }
    None
}

/// 取当前下标处的字符；越界返回 None，避免在 release（panic = abort）下直接杀进程。
fn char_at(text: &str, index: usize) -> Option<char> {
    text.get(index..).and_then(|rest| rest.chars().next())
}

fn parse_start_tag(inner: &str) -> (String, Vec<(String, String)>, bool) {
    let trimmed = inner.trim();
    let self_closing = trimmed.ends_with('/');
    let body = trimmed.trim_end_matches('/').trim_end();
    let mut chars = body.char_indices();
    let mut name_end = body.len();
    for (index, ch) in chars.by_ref() {
        if ch.is_whitespace() {
            name_end = index;
            break;
        }
    }
    let raw_name = &body[..name_end];
    let name = local_name(raw_name);
    let mut attrs = Vec::new();
    let mut index = name_end;
    while index < body.len() {
        while index < body.len() {
            let Some(ch) = char_at(body, index) else { break };
            if ch.is_whitespace() {
                index += ch.len_utf8();
            } else {
                break;
            }
        }
        if index >= body.len() {
            break;
        }
        let key_start = index;
        while index < body.len() {
            let Some(ch) = char_at(body, index) else { break };
            if ch.is_whitespace() || ch == '=' {
                break;
            }
            index += ch.len_utf8();
        }
        let key = local_name(&body[key_start..index]);
        while index < body.len() {
            let Some(ch) = char_at(body, index) else { break };
            if ch.is_whitespace() {
                index += ch.len_utf8();
            } else {
                break;
            }
        }
        if index < body.len() && body[index..].starts_with('=') {
            index += 1;
            while index < body.len() {
                let Some(ch) = char_at(body, index) else { break };
                if ch.is_whitespace() {
                    index += ch.len_utf8();
                } else {
                    break;
                }
            }
            if index < body.len() {
                let Some(quote) = char_at(body, index) else { break };
                let value = if quote == '\"' || quote == '\'' {
                    index += 1;
                    let start = index;
                    while index < body.len() {
                        let Some(ch) = char_at(body, index) else { break };
                        if ch == quote {
                            break;
                        }
                        index += ch.len_utf8();
                    }
                    let value = decode_entities(&body[start..index]);
                    if index < body.len() {
                        index += 1;
                    }
                    value
                } else {
                    let start = index;
                    while index < body.len() {
                        let Some(ch) = char_at(body, index) else { break };
                        if ch.is_whitespace() {
                            break;
                        }
                        index += ch.len_utf8();
                    }
                    decode_entities(&body[start..index])
                };
                if !key.is_empty() {
                    attrs.push((key, value));
                }
            }
        }
    }
    (name, attrs, self_closing)
}

fn local_name(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).trim().to_ascii_lowercase()
}

fn node_children<'a>(node: &'a XmlNode, name: &str) -> Vec<&'a XmlNode> {
    let wanted = name.to_string();
    node.children
        .iter()
        .filter(|child| child.name == wanted)
        .collect()
}

fn first_child<'a>(node: &'a XmlNode, names: &[&str]) -> Option<&'a XmlNode> {
    node.children
        .iter()
        .find(|child| names.iter().any(|name| *name == child.name))
}

fn node_all_text(node: &XmlNode) -> String {
    let mut out = node.text.clone();
    for child in &node.children {
        let child_text = node_all_text(child);
        if child_text.trim().is_empty() {
            continue;
        }
        if !out.is_empty()
            && !out
                .chars()
                .last()
                .map(|ch| ch.is_whitespace())
                .unwrap_or(true)
        {
            out.push(' ');
        }
        out.push_str(&child_text);
    }
    out
}

fn attr_value<'a>(node: &'a XmlNode, name: &str) -> Option<&'a str> {
    node.attrs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn child_text(node: &XmlNode, names: &[&str]) -> String {
    first_child(node, names)
        .map(node_all_text)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn collect_entries<'a>(node: &'a XmlNode, out: &mut Vec<&'a XmlNode>) {
    for child in &node.children {
        if child.name == "item" || child.name == "entry" {
            out.push(child);
        }
        collect_entries(child, out);
    }
}

fn parse_feed(xml: &str) -> Result<Vec<FeedEntry>, String> {
    let root = parse_xml(xml);
    let mut nodes = Vec::new();
    collect_entries(&root, &mut nodes);
    if nodes.is_empty() {
        if xml.contains("<rss") || xml.contains("<feed") || xml.contains("<rdf") {
            return Err("Feed 中没有可解析的文章条目。".to_string());
        }
        return Err("无法识别 RSS/Atom：未找到 item 或 entry。".to_string());
    }
    nodes.truncate(MAX_ENTRIES_PARSED);
    let mut entries = Vec::new();
    for node in nodes {
        entries.push(parse_entry(node));
    }
    Ok(entries)
}

fn parse_entry(node: &XmlNode) -> FeedEntry {
    let title = child_text(node, &["title"]);
    let link = atom_or_text_link(node);
    let guid = {
        let text = child_text(node, &["guid", "id"]);
        if text.is_empty() {
            attr_value(node, "about").unwrap_or("").to_string()
        } else {
            text
        }
    };
    let published_at = child_text(node, &["pubdate", "published", "updated", "date"]);
    let published_ts = parse_feed_date(&published_at).unwrap_or(0);
    let author = child_text(node, &["author", "creator"]);
    let content_html = {
        let encoded = child_text(node, &["encoded"]);
        let content = child_text(node, &["content"]);
        if !encoded.is_empty() {
            encoded
        } else if !content.is_empty() {
            content
        } else {
            child_text(node, &["description"])
        }
    };
    let summary = {
        let description = child_text(node, &["description"]);
        if !description.is_empty() {
            clamp_chars(&html_to_plain(&description, &link), 500)
        } else {
            clamp_chars(&html_to_plain(&child_text(node, &["summary"]), &link), 500)
        }
    };
    let key = if guid.trim().is_empty() {
        if link.trim().is_empty() {
            hash8(&format!("{title}\n{published_at}"))
        } else {
            link.clone()
        }
    } else {
        guid.clone()
    };
    let image_url = entry_image_url(node, &link);
    let attachments = entry_attachments(node, &link);
    FeedEntry {
        key,
        guid,
        link,
        title,
        published_at,
        published_ts,
        author,
        content_html,
        summary,
        image_url,
        attachments,
    }
}

/// 条目自带的图片地址：enclosure 或 media:content（localName content + url 属性）。
///
/// 只接受 http/https 且声明为 image/* 的地址；Atom 的 HTML content 没有 url 属性，不会被误判。
fn entry_image_url(node: &XmlNode, base_url: &str) -> String {
    for child in &node.children {
        if child.name != "enclosure" && child.name != "content" {
            continue;
        }
        let Some(url) = attr_value(child, "url") else {
            continue;
        };
        let kind = attr_value(child, "type").unwrap_or_default();
        let lower = kind.trim().to_ascii_lowercase();
        let medium = attr_value(child, "medium")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let is_image = lower.starts_with("image/") || (lower.is_empty() && medium == "image");
        if !is_image {
            continue;
        }
        if let Some(url) = sanitize_media_url(&url, base_url) {
            let lower_url = url.to_ascii_lowercase();
            if lower_url.starts_with("http://") || lower_url.starts_with("https://") {
                return url;
            }
        }
    }
    String::new()
}

/// 条目附带的非图片媒体：enclosure / media:content 中声明为音频、视频或其他文件。
/// 只保留 http(s) 地址，最多 20 条，供阅读区底部「附件」模块使用。
fn entry_attachments(node: &XmlNode, base_url: &str) -> Vec<RssAttachment> {
    let mut attachments = Vec::new();
    for child in &node.children {
        if child.name != "enclosure" && child.name != "content" {
            continue;
        }
        let Some(url) = attr_value(child, "url") else {
            continue;
        };
        let Some(mime) = attachment_mime(child, &url) else {
            continue;
        };
        let Some(url) = sanitize_media_url(&url, base_url) else {
            continue;
        };
        if !url.to_ascii_lowercase().starts_with("http") {
            continue;
        }
        if url.len() > 2_000 {
            continue;
        }
        if attachments.iter().any(|item: &RssAttachment| item.url == url) {
            continue;
        }
        let title = attr_value(child, "title")
            .map(|value| clamp_chars(value.trim(), 160))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| attachment_title_from_url(&url));
        let size_bytes = attr_value(child, "length")
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(0);
        attachments.push(RssAttachment {
            url,
            mime,
            title,
            size_bytes,
        });
        if attachments.len() >= 20 {
            break;
        }
    }
    attachments
}

fn attachment_mime(node: &XmlNode, url: &str) -> Option<String> {
    let kind = attr_value(node, "type").unwrap_or_default();
    let lower = kind.trim().to_ascii_lowercase();
    if lower.starts_with("image/") {
        return None;
    }
    if !lower.is_empty() {
        return Some(lower);
    }
    let medium = attr_value(node, "medium")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if medium == "image" {
        return None;
    }
    if medium == "audio" || medium == "video" {
        return Some(format!("{medium}/unknown"));
    }
    attachment_mime_from_extension(url)
}

fn attachment_mime_from_extension(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path.rsplit('.').next()?.trim().to_ascii_lowercase();
    let mime = match ext.as_str() {
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "pdf" => "application/pdf",
        "epub" => "application/epub+zip",
        "zip" => "application/zip",
        _ => return None,
    };
    Some(mime.to_string())
}

fn attachment_title_from_url(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or("附件");
    if name.trim().is_empty() {
        "附件".to_string()
    } else {
        clamp_chars(name.trim(), 160)
    }
}

fn atom_or_text_link(node: &XmlNode) -> String {
    let links: Vec<&XmlNode> = node_children(node, "link");
    if let Some(link) = links.iter().find(|link| {
        attr_value(*link, "rel")
            .map(|rel| rel.eq_ignore_ascii_case("alternate"))
            .unwrap_or(false)
    }) {
        if let Some(href) = attr_value(link, "href") {
            return href.trim().to_string();
        }
    }
    for link in &links {
        if let Some(href) = attr_value(*link, "href") {
            return href.trim().to_string();
        }
    }
    links
        .iter()
        .map(|link| node_all_text(*link).trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut index = 0usize;
    while index < input.len() {
        let rest = &input[index..];
        if let Some(position) = rest.find('&') {
            out.push_str(&rest[..position]);
            index += position;
            let entity_rest = &input[index..];
            if let Some(semicolon) = entity_rest.find(';') {
                if semicolon <= 32 {
                    let entity = &entity_rest[1..semicolon];
                    if let Some(decoded) = decode_entity(entity) {
                        out.push_str(&decoded);
                        index += semicolon + 1;
                        continue;
                    }
                }
            }
            out.push('&');
            index += 1;
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

fn decode_entity(entity: &str) -> Option<String> {
    if let Some(number) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
        return u32::from_str_radix(number, 16)
            .ok()
            .and_then(char::from_u32)
            .map(|ch| ch.to_string());
    }
    if let Some(number) = entity.strip_prefix('#') {
        return number
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(|ch| ch.to_string());
    }
    let value = match entity {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" | "ldquo" | "rdquo" => "\"",
        "apos" | "lsquo" | "rsquo" => "'",
        "nbsp" => " ",
        "mdash" => "—",
        "ndash" => "–",
        "hellip" => "…",
        "middot" => "·",
        "copy" => "©",
        _ => return None,
    };
    Some(value.to_string())
}

fn html_to_paragraphs(html: &str, base_url: &str) -> Vec<String> {
    let plain = sanitize_html(html, base_url);
    let mut paragraphs = Vec::new();
    for block in plain.split("\n\n") {
        let normalized = block.split_whitespace().collect::<Vec<_>>().join(" ");
        if !normalized.is_empty() {
            paragraphs.push(normalized);
        }
    }
    let mut joined = String::new();
    for paragraph in &paragraphs {
        if !joined.is_empty() {
            joined.push_str("\n\n");
        }
        joined.push_str(paragraph);
    }
    if joined.chars().count() > MAX_ARTICLE_CHARS {
        let clipped = clamp_chars(&joined, MAX_ARTICLE_CHARS);
        paragraphs = clipped
            .split("\n\n")
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect();
    }
    paragraphs
}

fn html_to_plain(html: &str, base_url: &str) -> String {
    sanitize_html(html, base_url)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_html(html: &str, base_url: &str) -> String {
    let mut out = String::new();
    let mut index = 0usize;
    let mut open_media: Option<String> = None;
    while index < html.len() {
        let rest = &html[index..];
        if rest.starts_with('<') {
            let sample: String = rest.chars().take(16).collect();
            let lower = sample.to_ascii_lowercase();
            if lower.starts_with("<script") || lower.starts_with("<style") {
                let tag = if lower.starts_with("<script") { "script" } else { "style" };
                let close = format!("</{tag}");
                if let Some(position) = rest.to_ascii_lowercase().find(&close) {
                    if let Some(end) = rest[position..].find('>') {
                        index += position + end + 1;
                        continue;
                    }
                }
            }
            let Some(end) = find_tag_end(rest) else {
                out.push('<');
                index += 1;
                continue;
            };
            let inner = &rest[1..end];
            let trimmed = inner.trim();
            let closing = trimmed.starts_with('/');
            let tag = trimmed
                .trim_start_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches('/')
                .to_ascii_lowercase();
            if tag == "br" {
                out.push('\n');
            } else if tag == "li" && !closing {
                out.push_str("\n\n· ");
            } else if tag == "p"
                || tag == "div"
                || tag == "blockquote"
                || tag == "pre"
                || tag == "figure"
                || tag == "figcaption"
                || tag == "section"
                || tag == "article"
                || tag == "tr"
                || tag == "td"
                || tag == "th"
                || tag == "li"
                || tag == "ul"
                || tag == "ol"
                || tag == "hr"
                || (tag.len() == 2 && tag.starts_with('h') && tag.ends_with(|c: char| c.is_ascii_digit()))
            {
                out.push_str("\n\n");
            } else if tag == "img" {
                if let Some(src) = attr_value_from_tag(inner, "src") {
                    if let Some(url) = sanitize_media_url(&src, base_url) {
                        out.push_str(&format!("![img]({})", markdown_url(&url)));
                    }
                }
            } else if tag == "audio" || tag == "video" {
                if closing {
                    open_media = None;
                } else {
                    open_media = Some(tag.clone());
                    if let Some(src) = attr_value_from_tag(inner, "src") {
                        if let Some(url) = sanitize_media_url(&src, base_url) {
                            out.push_str(&format!("[{}]({})", tag, markdown_url(&url)));
                        }
                    }
                }
            } else if tag == "source" && !closing {
                // 常见写法：<audio><source src="…"/></audio>
                if let (Some(kind), Some(src)) = (open_media.as_ref(), attr_value_from_tag(inner, "src")) {
                    if let Some(url) = sanitize_media_url(&src, base_url) {
                        out.push_str(&format!("[{}]({})", kind, markdown_url(&url)));
                        open_media = None;
                    }
                }
            }
            index += end + 1;
            continue;
        }
        if let Some(position) = rest.find('<') {
            out.push_str(&decode_entities(&rest[..position]));
            index += position;
        } else {
            out.push_str(&decode_entities(rest));
            break;
        }
    }
    out.replace("\r", "\n").replace('\u{00a0}', " ")
}

fn attr_value_from_tag(inner: &str, wanted: &str) -> Option<String> {
    let (_, attrs, _) = parse_start_tag(inner);
    attrs
        .into_iter()
        .find(|(key, _)| key == wanted)
        .map(|(_, value)| value)
}

/// RSS 正文只允许 data: 与 http(s) 媒体直接交给 WebView；其余协议丢弃，不代理不落盘。
/// 相对地址按条目 link 解析成绝对 http(s) 后再放行。
fn sanitize_media_url(src: &str, base_url: &str) -> Option<String> {
    let value = src.trim();
    if value.is_empty() || value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("data:") {
        return Some(value.to_string());
    }
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(value.to_string());
    }
    if let Some(colon) = value.find(':') {
        let path_start = value
            .find(['/', '?', '#'])
            .unwrap_or(value.len());
        if colon < path_start {
            // javascript:/mailto:/file: 等未知协议一律丢弃，避免被当成相对路径解析。
            return None;
        }
    }
    let resolved = resolve_media_url(value, base_url)?;
    let resolved_lower = resolved.to_ascii_lowercase();
    if resolved_lower.starts_with("http://") || resolved_lower.starts_with("https://") {
        Some(resolved)
    } else {
        None
    }
}

/// 把 `//cdn/x`、`/img/x`、`images/x`、`../x` 这类相对地址基于条目 link 解析为绝对地址。
fn resolve_media_url(value: &str, base_url: &str) -> Option<String> {
    let (scheme, rest) = base_url.trim().split_once("://")?;
    let scheme_lower = scheme.to_ascii_lowercase();
    if scheme_lower != "http" && scheme_lower != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    let base_no_fragment = rest
        .split(['?', '#'])
        .next()
        .unwrap_or("");
    let base_dir = match base_no_fragment.rfind('/') {
        Some(index) => &base_no_fragment[..=index],
        None => "/",
    };
    let absolute = if value.starts_with("//") {
        format!("{}://{}", scheme_lower, value.trim_start_matches('/'))
    } else if value.starts_with('/') {
        format!("{}://{}{}", scheme_lower, authority, value)
    } else if value.starts_with('?') || value.starts_with('#') {
        format!("{}://{}{}", scheme_lower, base_no_fragment, value)
    } else {
        format!("{}://{}{}", scheme_lower, base_dir, value)
    };
    Some(normalize_absolute_url(&absolute))
}

/// 折叠 `.`/`..` 段，保留协议、主机与 query/hash。
fn normalize_absolute_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let (path_only, suffix) = match path.find(['?', '#']) {
        Some(index) => (&path[..index], &path[index..]),
        None => (path, ""),
    };
    let mut segments: Vec<&str> = Vec::new();
    for segment in path_only.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    let mut out = format!("{}://{}/", scheme.to_ascii_lowercase(), authority);
    out.push_str(&segments.join("/"));
    out.push_str(suffix);
    out
}

fn markdown_url(value: &str) -> String {
    value.replace(' ', "%20").replace(')', "%29")
}

fn clamp_chars(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        text.to_string()
    } else {
        chars.into_iter().take(max).collect()
    }
}

fn parse_feed_date(raw: &str) -> Option<i64> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(ts) = parse_iso_date(value) {
        return Some(ts);
    }
    parse_rfc2822_date(value)
}

fn parse_iso_date(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = value.get(0..4)?.parse::<i64>().ok()?;
    let month = value.get(5..7)?.parse::<u32>().ok()?;
    let day = value.get(8..10)?.parse::<u32>().ok()?;
    let mut secs = days_from_civil(year, month, day) * 86_400;
    if value.len() > 11 && (bytes[10] == b'T' || bytes[10] == b' ') {
        let time = value.get(11..19)?;
        let hour = time.get(0..2)?.parse::<i64>().ok()?;
        let minute = time.get(3..5)?.parse::<i64>().ok()?;
        let second = time.get(6..8)?.parse::<i64>().ok()?;
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        secs += hour * 3_600 + minute * 60 + second;
    }
    let rest = value.get(19..).unwrap_or("").trim();
    if rest.starts_with('+') || rest.starts_with('-') {
        let sign = if rest.starts_with('-') { -1 } else { 1 };
        let digits = rest[1..].replace(':', "");
        if digits.len() >= 4 {
            let hours = digits.get(0..2)?.parse::<i64>().ok()?;
            let minutes = digits.get(2..4)?.parse::<i64>().ok()?;
            secs -= sign * (hours * 3_600 + minutes * 60);
        }
    }
    Some(secs)
}

fn parse_rfc2822_date(value: &str) -> Option<i64> {
    let cleaned = value.replace(',', " ");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    let months = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let month_pos = tokens
        .iter()
        .position(|token| months.iter().any(|month| token.eq_ignore_ascii_case(month)))?;
    if month_pos == 0 || month_pos + 1 >= tokens.len() {
        return None;
    }
    let day = tokens[month_pos - 1].parse::<u32>().ok()?;
    let year = tokens[month_pos + 1].parse::<i64>().ok()?;
    let month = months
        .iter()
        .position(|month| tokens[month_pos].eq_ignore_ascii_case(month))? as u32
        + 1;
    let mut secs = days_from_civil(year, month, day) * 86_400;
    if let Some(time) = tokens.get(month_pos + 2) {
        let parts: Vec<&str> = time.split(':').collect();
        if parts.len() >= 2 {
            let hour = parts[0].parse::<i64>().ok()?;
            let minute = parts[1].parse::<i64>().ok()?;
            let second = parts.get(2).and_then(|part| part.parse::<i64>().ok()).unwrap_or(0);
            secs += hour * 3_600 + minute * 60 + second;
        }
    }
    if let Some(zone) = tokens.get(month_pos + 3) {
        if zone.starts_with('+') || zone.starts_with('-') {
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let digits = zone[1..].to_string();
            if digits.len() >= 4 {
                let hours = digits.get(0..2)?.parse::<i64>().ok()?;
                let minutes = digits.get(2..4)?.parse::<i64>().ok()?;
                secs -= sign * (hours * 3_600 + minutes * 60);
            }
        }
    }
    Some(secs)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = if adjusted_year >= 0 { adjusted_year } else { adjusted_year - 399 } / 400;
    let year_of_era = adjusted_year - era * 400;
    let month_index = month as i64 - 3 + if month > 2 { 0 } else { 12 };
    let day_of_year = (153 * month_index + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reading_store::read_book_at;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_root(tag: &str) -> PathBuf {
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "omnidesk-rss-test-{tag}-{}-{seq}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn sample_subscription() -> RssSubscription {
        RssSubscription {
            id: "testfeed".to_string(),
            url: "https://example.test/feed.xml".to_string(),
            name: "测试订阅".to_string(),
            group: DEFAULT_GROUP.to_string(),
            added_at: "2026-11-18T00:00:00Z".to_string(),
            last_fetched_at: String::new(),
            last_error: String::new(),
        }
    }

    #[test]
    fn parses_rss2_with_chinese_cdata_and_entities() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel><title>合成源</title>
    <item>
      <title>中文 &amp; 标题</title>
      <link>https://example.test/posts/1</link>
      <guid>post-1</guid>
      <pubDate>Tue, 18 Nov 2026 08:00:00 GMT</pubDate>
      <dc:creator>作者甲</dc:creator>
      <content:encoded><![CDATA[<p>第一段 &amp; 引用。</p><p>第二段 含 <strong>重点</strong>。</p><script>alert(1)</script>]]></content:encoded>
    </item>
  </channel>
</rss>"#;
        let entries = parse_feed(xml).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "中文 & 标题");
        assert_eq!(entries[0].link, "https://example.test/posts/1");
        assert_eq!(entries[0].author, "作者甲");
        assert_eq!(entries[0].published_ts, 1_794_988_800);
        let paragraphs = html_to_paragraphs(&entries[0].content_html, &entries[0].link);
        assert_eq!(paragraphs.len(), 2);
        assert!(paragraphs[0].contains("第一段 & 引用"));
        assert!(paragraphs[1].contains("第二段 含 重点"));
    }

    #[test]
    fn parses_atom_with_namespace_prefixes_and_link_href() {
        let xml = r#"<?xml version="1.0"?>
<atom:feed xmlns:atom="http://www.w3.org/2005/Atom">
  <atom:entry>
    <atom:id>tag:example.test,2026:1</atom:id>
    <atom:title>Atom 示例</atom:title>
    <atom:link rel="alternate" href="https://example.test/atom/1"/>
    <atom:updated>2026-11-18T09:30:00+08:00</atom:updated>
    <atom:author><atom:name>作者乙</atom:name></atom:author>
    <atom:content type="html">&lt;p&gt;Alice &amp;amp; Bob&lt;/p&gt;&lt;p&gt;第二段&lt;/p&gt;</atom:content>
  </atom:entry>
</atom:feed>"#;
        let entries = parse_feed(xml).unwrap();
        assert_eq!(entries[0].title, "Atom 示例");
        assert_eq!(entries[0].link, "https://example.test/atom/1");
        assert_eq!(entries[0].author, "作者乙");
        assert_eq!(entries[0].published_ts, 1_794_965_400);
        let paragraphs = html_to_paragraphs(&entries[0].content_html, &entries[0].link);
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0], "Alice & Bob");
    }

    #[test]
    fn extracts_image_from_enclosure_and_media_content() {
        let xml = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
  <channel><item>
    <title>带图文章</title><link>https://example.test/photo</link><guid>photo</guid>
    <enclosure url="https://example.test/cover.jpg" type="image/jpeg" length="1234"/>
    <media:content url="https://example.test/inline.png" type="image/png" medium="image"/>
    <description>正文没有图片</description>
  </item></channel>
</rss>"#;
        let entries = parse_feed(xml).unwrap();
        assert_eq!(entries[0].image_url, "https://example.test/cover.jpg");
        let xml_media_only = r#"<rss xmlns:media="http://search.yahoo.com/mrss/"><channel><item>
<title>媒体图</title><link>https://example.test/m</link><guid>m</guid>
<media:content url="https://example.test/inline.png" type="image/png"/>
<description>正文</description></item></channel></rss>"#;
        let entries = parse_feed(xml_media_only).unwrap();
        assert_eq!(entries[0].image_url, "https://example.test/inline.png");
    }

    #[test]
    fn parses_audio_and_file_enclosures_as_attachments() {
        let xml = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
  <channel><item>
    <title>播客文章</title><link>https://example.test/podcast</link><guid>podcast</guid>
    <enclosure url="https://example.test/cover.jpg" type="image/jpeg" length="1234"/>
    <enclosure url="https://example.test/episode.mp3" type="audio/mpeg" length="52428800" title="节目音频"/>
    <media:content url="https://example.test/notes.pdf" type="application/pdf" length="204800" title="节目文稿"/>
    <description>正文</description>
  </item></channel>
</rss>"#;
        let entries = parse_feed(xml).unwrap();
        assert_eq!(entries[0].image_url, "https://example.test/cover.jpg");
        assert_eq!(entries[0].attachments.len(), 2);
        assert_eq!(entries[0].attachments[0].title, "节目音频");
        assert_eq!(entries[0].attachments[0].mime, "audio/mpeg");
        assert_eq!(entries[0].attachments[0].size_bytes, 52_428_800);
        assert_eq!(entries[0].attachments[1].title, "节目文稿");
        assert_eq!(entries[0].attachments[1].mime, "application/pdf");
    }

    #[test]
    fn existing_article_metadata_gains_new_attachments() {
        let root = temp_root("attachments");
        let subscription = sample_subscription();
        let plain = r#"<rss><channel><item><title>旧播客</title><link>https://example.test/old-pod</link><guid>old-pod</guid><description>只有文字</description></item></channel></rss>"#;
        ingest_feed_at(&root, &subscription, plain, 1_800_000_000.0).unwrap();
        let items = list_articles_at(&root).unwrap();
        assert!(items[0].attachments.is_empty());
        let with_audio = r#"<rss><channel><item><title>旧播客</title><link>https://example.test/old-pod</link><guid>old-pod</guid><description>只有文字</description><enclosure url="https://example.test/old.mp3" type="audio/mpeg" length="1048576"/></item></channel></rss>"#;
        let summary = ingest_feed_at(&root, &subscription, with_audio, 1_800_000_010.0).unwrap();
        assert_eq!(summary.added, 0);
        assert_eq!(summary.skipped, 1);
        let refreshed = list_articles_at(&root).unwrap();
        assert_eq!(refreshed[0].attachments.len(), 1);
        assert_eq!(refreshed[0].attachments[0].url, "https://example.test/old.mp3");
        assert_eq!(refreshed[0].attachments[0].size_bytes, 1_048_576);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backfills_old_article_media_without_annotations() {
        let root = temp_root("backfill");
        let subscription = sample_subscription();
        let plain = r#"<rss><channel><item><title>旧文章</title><link>https://example.test/old</link><guid>old</guid><description>只有文字</description></item></channel></rss>"#;
        ingest_feed_at(&root, &subscription, plain, 1_800_000_000.0).unwrap();
        let nl = char::from_u32(10).unwrap();
        let book_id = format!(
            "rss-{}-{}",
            hash8(&format!("{}{}{}", subscription.url, nl, "old")),
            hash8("old")
        );
        crate::reading_store::set_progress_at(&root, &book_id, 0.5, 1_800_000_010.0).unwrap();
        let with_image = r#"<rss><channel><item><title>旧文章</title><link>https://example.test/old</link><guid>old</guid><description>&lt;p&gt;文字&lt;/p&gt;&lt;img src="https://example.test/pic.png"/&gt;</description></item></channel></rss>"#;
        let summary = ingest_feed_at(&root, &subscription, with_image, 1_800_000_020.0).unwrap();
        assert_eq!(summary.added, 0);
        let book = read_book_at(&root, &book_id).unwrap();
        assert_eq!(book.media_version, 1);
        assert!(book.chapters[0]
            .paragraphs
            .iter()
            .any(|item| item.contains("![img](https://example.test/pic.png)")));
        assert!((book.progress.percent - 0.5).abs() < 1e-9, "回填不得改动阅读进度");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_newest_fifty_and_parses_at_most_two_hundred_entries() {
        let mut xml = String::from("<rss><channel>");
        for index in 0..250u64 {
            let published = utc_iso8601(1_800_000_000.0 + index as f64 * 86_400.0);
            xml.push_str(&format!(
                "<item><title>条目 {index}</title><link>https://example.test/{index}</link><guid>{index}</guid><published>{published}</published><description>正文 {index}</description></item>"
            ));
        }
        xml.push_str("</channel></rss>");
        let entries = parse_feed(&xml).unwrap();
        assert_eq!(entries.len(), 200);
        let mut sorted = entries;
        sorted.sort_by(|a, b| b.published_ts.cmp(&a.published_ts));
        sorted.truncate(MAX_ENTRIES_KEPT);
        assert_eq!(sorted.len(), 50);
        assert_eq!(sorted[0].title, "条目 199");
        assert_eq!(sorted[49].title, "条目 150");
    }

    #[test]
    fn ingest_deduplicates_and_does_not_overwrite_annotations() {
        let root = temp_root("dedupe");
        let subscription = sample_subscription();
        let xml = r#"<rss><channel><item><title>去重样例</title><link>https://example.test/one</link><guid>one</guid><description>第一段</description></item></channel></rss>"#;
        let first = ingest_feed_at(&root, &subscription, xml, 1_800_000_000.0).unwrap();
        assert_eq!(first.added, 1);
        let book_id = format!(
            "rss-{}-{}",
            hash8(&format!("{}\n{}", subscription.url, "one")),
            hash8("one")
        );
        let draft = crate::reading_store::ReadingAnnotationDraft {
            kind: "highlight".to_string(),
            chapter_index: 0,
            paragraph_index: 0,
            start: Some(0),
            end: Some(3),
            quote: "第一段".to_string(),
            body: String::new(),
            color: "amber".to_string(),
            source: "user".to_string(),
        };
        crate::reading_store::add_annotation_at(&root, &book_id, draft, 1_800_000_001.0).unwrap();
        let second = ingest_feed_at(&root, &subscription, xml, 1_800_000_002.0).unwrap();
        assert_eq!(second.added, 0);
        assert_eq!(second.skipped, 1);
        let book = read_book_at(&root, &book_id).unwrap();
        assert_eq!(book.annotations.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preset_seed_is_idempotent_and_does_not_resurrect_deleted_items() {
        let root = temp_root("preset");
        let first = seed_store_at(&root, 1_800_000_000.0).unwrap();
        assert_eq!(first.subscriptions.len(), 49);
        let removed_id = first.subscriptions[0].id.clone();
        remove_subscription_at(&root, &removed_id).unwrap();
        let after_delete = load_store_at(&root).unwrap();
        assert_eq!(after_delete.subscriptions.len(), 48);
        let second_seed = seed_store_at(&root, 1_800_000_100.0).unwrap();
        assert_eq!(second_seed.subscriptions.len(), 48);
        assert!(!second_seed.subscriptions.iter().any(|item| item.id == removed_id));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn adding_same_url_twice_returns_the_existing_subscription() {
        let root = temp_root("add");
        seed_store_at(&root, 1_800_000_000.0).unwrap();
        let before = load_store_at(&root).unwrap().subscriptions.len();
        let first = add_subscription_at(&root, "https://example.test/new.xml", Some("新源"), 1_800_000_100.0).unwrap();
        assert_eq!(first.name, "新源");
        assert_eq!(first.group, DEFAULT_GROUP);
        let second = add_subscription_at(&root, "https://example.test/new.xml", Some("改名"), 1_800_000_200.0).unwrap();
        assert_eq!(first, second);
        assert_eq!(load_store_at(&root).unwrap().subscriptions.len(), before + 1);
        assert!(add_subscription_at(&root, "ftp://example.test/feed", None, 1_800_000_300.0).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preset_sources_returns_nine_featured_authors() {
        let sources = preset_sources();
        assert_eq!(sources.len(), 9);
        assert!(sources.iter().all(|source| source.group == PRESET_GROUP_AUTHORS));
        assert!(sources.iter().any(|source| source.name == "阮一峰的网络日志"));
    }

    #[test]
    fn truncates_long_article_body_to_the_limit() {
        let root = temp_root("truncate");
        let subscription = sample_subscription();
        let long = "长".repeat(MAX_ARTICLE_CHARS + 5_000);
        let xml = format!(
            "<rss><channel><item><title>长文</title><link>https://example.test/long</link><guid>long</guid><description>{long}</description></item></channel></rss>"
        );
        let summary = ingest_feed_at(&root, &subscription, &xml, 1_800_000_000.0).unwrap();
        assert_eq!(summary.added, 1);
        let book_id = format!(
            "rss-{}-{}",
            hash8(&format!("{}\n{}", subscription.url, "long")),
            hash8("long")
        );
        let book = read_book_at(&root, &book_id).unwrap();
        let chars: usize = book.chapters.iter().flat_map(|c| c.paragraphs.iter()).map(|p| p.chars().count()).sum();
        assert!(chars <= MAX_ARTICLE_CHARS);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn marks_article_read_without_touching_body() {
        let root = temp_root("read");
        let subscription = sample_subscription();
        let xml = r#"<rss><channel><item><title>已读样例</title><link>https://example.test/read</link><guid>read</guid><description>正文</description></item></channel></rss>"#;
        ingest_feed_at(&root, &subscription, xml, 1_800_000_000.0).unwrap();
        let items = list_articles_at(&root).unwrap();
        assert_eq!(items.len(), 1);
        assert!(!items[0].read);
        let updated = mark_article_read_at(&root, &items[0].book_id).unwrap();
        assert!(updated.read);
        assert_eq!(list_articles_at(&root).unwrap()[0].read, true);
        assert!(read_book_at(&root, &items[0].book_id).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn favorites_article_metadata_only() {
        let root = temp_root("favorite");
        let subscription = sample_subscription();
        let xml = r#"<rss><channel><item><title>收藏样例</title><link>https://example.test/star</link><guid>star</guid><description>正文</description></item></channel></rss>"#;
        ingest_feed_at(&root, &subscription, xml, 1_800_000_000.0).unwrap();
        let items = list_articles_at(&root).unwrap();
        assert_eq!(items.len(), 1);
        assert!(!items[0].favorite);
        let book_path = root.join("books").join(format!("{}.json", items[0].book_id));
        let book_before = fs::read_to_string(&book_path).unwrap();
        let updated = set_article_favorite_at(&root, &items[0].book_id, true).unwrap();
        assert!(updated.favorite);
        assert_eq!(list_articles_at(&root).unwrap()[0].favorite, true);
        assert_eq!(fs::read_to_string(&book_path).unwrap(), book_before, "收藏不得改写正文");
        let cleared = set_article_favorite_at(&root, &items[0].book_id, false).unwrap();
        assert!(!cleared.favorite);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sanitizer_drops_script_and_preserves_block_paragraphs() {
        let html = "<h1>标题</h1><p>一段 &amp; 二段</p><script>bad()</script><p>第三段<br>换行</p>";
        let paragraphs = html_to_paragraphs(html, "https://example.test/posts/1");
        assert_eq!(paragraphs.len(), 3);
        assert_eq!(paragraphs[0], "标题");
        assert_eq!(paragraphs[1], "一段 & 二段");
        assert_eq!(paragraphs[2], "第三段 换行");
        assert!(!paragraphs.join(" ").contains("bad()"));
    }

    #[test]
    fn sanitizer_keeps_whitelisted_media_and_drops_other_schemes() {
        let html = concat!(
            "<p>图</p><img src=\"https://example.test/a.png\"/>",
            "<img src=\"javascript:alert(1)\"/>",
            "<audio src=\"https://example.test/a.mp3\"></audio>",
            "<video><source src=\"data:video/mp4;base64,AAAA\"/></video>"
        );
        let sanitized = sanitize_html(html, "https://example.test/posts/1");
        assert!(sanitized.contains("![img](https://example.test/a.png)"), "{sanitized}");
        assert!(!sanitized.contains("javascript"), "{sanitized}");
        assert!(sanitized.contains("[audio](https://example.test/a.mp3)"), "{sanitized}");
        assert!(sanitized.contains("[video](data:video/mp4;base64,AAAA)"), "{sanitized}");
    }

    #[test]
    fn resolves_relative_image_sources_against_entry_link() {
        let html = concat!(
            "<p>图</p><img src=\"/assets/a.png\"/>",
            "<img src=\"//cdn.example.test/b.png\"/>",
            "<img src=\"../img/c.png\"/>",
            "<img src=\"javascript:alert(1)\"/>"
        );
        let paragraphs = html_to_paragraphs(html, "https://example.test/posts/2026/11/art");
        let joined = paragraphs.join("\n");
        assert!(joined.contains("![img](https://example.test/assets/a.png)"), "{joined}");
        assert!(joined.contains("![img](https://cdn.example.test/b.png)"), "{joined}");
        assert!(joined.contains("![img](https://example.test/posts/2026/img/c.png)"), "{joined}");
        assert!(!joined.contains("javascript"), "{joined}");
    }

    #[test]
    fn refresh_article_rebuilds_body_and_keeps_progress() {
        let root = temp_root("refresh");
        let subscription = sample_subscription();
        let old_xml = r#"<rss><channel><item>
<title>ALTRun 旧文</title><link>https://example.test/posts/alt</link><guid>alt</guid>
<description>&lt;p&gt;正文&lt;/p&gt;&lt;p&gt;[图片：ALTRun … 21]&lt;/p&gt;</description>
</item></channel></rss>"#;
        ingest_feed_at(&root, &subscription, old_xml, 1_800_000_000.0).unwrap();
        let items = list_articles_at(&root).unwrap();
        assert_eq!(items.len(), 1);
        let book_id = items[0].book_id.clone();
        let mut book = read_book_at(&root, &book_id).unwrap();
        book.progress.percent = 0.42;
        write_book(&root, &book).unwrap();

        let new_xml = r#"<rss xmlns:content="http://purl.org/rss/1.0/modules/content/"><channel><item>
<title>ALTRun 旧文</title><link>https://example.test/posts/alt</link><guid>alt</guid>
<content:encoded><![CDATA[<p>正文</p><img src="/uploads/alt.png"/>]]></content:encoded>
</item></channel></rss>"#;
        let result = refresh_article_with_xml_at(&root, &book_id, new_xml).unwrap();
        assert!(result.updated, "{:?}", result.reason);
        let updated = read_book_at(&root, &book_id).unwrap();
        let joined = updated
            .chapters
            .iter()
            .flat_map(|chapter| chapter.paragraphs.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("![img](https://example.test/uploads/alt.png)"), "{joined}");
        assert!(!joined.contains("[图片："), "{joined}");
        assert_eq!(updated.progress.percent, 0.42);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_article_skips_when_annotations_exist() {
        use crate::reading_store::ReadingAnnotation;
        let root = temp_root("refresh-annotated");
        let subscription = sample_subscription();
        let old_xml = r#"<rss><channel><item>
<title>有批注的旧文</title><link>https://example.test/posts/note</link><guid>note</guid>
<description>&lt;p&gt;正文&lt;/p&gt;&lt;p&gt;[图片：ALTRun … 21]&lt;/p&gt;</description>
</item></channel></rss>"#;
        ingest_feed_at(&root, &subscription, old_xml, 1_800_000_000.0).unwrap();
        let items = list_articles_at(&root).unwrap();
        let book_id = items[0].book_id.clone();
        let mut book = read_book_at(&root, &book_id).unwrap();
        book.annotations.push(ReadingAnnotation {
            id: "ann-1".to_string(),
            kind: "highlight".to_string(),
            chapter_index: 0,
            paragraph_index: 0,
            start: 0,
            end: 2,
            quote: "正文".to_string(),
            body: String::new(),
            color: "amber".to_string(),
            source: "user".to_string(),
            created_at: "2026-11-18T00:00:00Z".to_string(),
            updated_at: "2026-11-18T00:00:00Z".to_string(),
        });
        write_book(&root, &book).unwrap();
        let new_xml = r#"<rss xmlns:content="http://purl.org/rss/1.0/modules/content/"><channel><item>
<title>有批注的旧文</title><link>https://example.test/posts/note</link><guid>note</guid>
<content:encoded><![CDATA[<p>正文</p><img src="/uploads/note.png"/>]]></content:encoded>
</item></channel></rss>"#;
        let result = refresh_article_with_xml_at(&root, &book_id, new_xml).unwrap();
        assert!(!result.updated, "{:?}", result.reason);
        assert!(result.reason.contains("批注"), "{:?}", result.reason);
        let unchanged = read_book_at(&root, &book_id).unwrap();
        assert_eq!(unchanged.chapters, book.chapters);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preset_source_size_stays_under_twenty_kilobytes() {
        let mut bytes = 0usize;
        for preset in PRESET_FEEDS {
            bytes += preset.name.len() + preset.url.len() + preset.group.len();
        }
        assert!(bytes < 20_000);
        assert_eq!(PRESET_FEEDS.len(), 49);
    }

    #[test]
    fn fetches_local_http_feed_and_ingests_entries() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let xml = r#"<rss><channel><item><title>本地回放</title><link>https://example.test/local</link><guid>local</guid><description>第一段</description><description>第二段</description></item></channel></rss>"#;
        let payload = xml.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = std::io::Read::read(&mut stream, &mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        // 本机回环监听器：只在这个测试里临时放行回环（生产代码没有这个开关）。
        crate::net_guard::allow_loopback_in_tests(true);
        let fetched =
            fetch_feed_xml(&format!("http://{address}/feed.xml"), crate::net_guard::NetPolicy::PublicOnly)
                .unwrap();
        crate::net_guard::allow_loopback_in_tests(false);
        server.join().unwrap();
        let root = temp_root("http");
        let subscription = sample_subscription();
        let summary = ingest_feed_at(&root, &subscription, &fetched, 1_800_000_000.0).unwrap();
        assert_eq!(summary.added, 1);
        let items = list_articles_at(&root).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "本地回放");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn feed_redirect_to_link_local_is_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let _ = std::io::Read::read(&mut stream, &mut request);
            let response = "HTTP/1.1 302 Found\r\nLocation: http://169.254.169.254/latest/meta-data\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).unwrap();
        });
        crate::net_guard::allow_loopback_in_tests(true);
        let result =
            fetch_feed_xml(&format!("http://{address}/feed.xml"), crate::net_guard::NetPolicy::PublicOnly);
        crate::net_guard::allow_loopback_in_tests(false);
        server.join().unwrap();
        let error = result.expect_err("跳到 169.254.169.254 必须被拒绝");
        assert!(error.contains("169.254.169.254"), "{error}");
        assert!(error.contains("禁止范围"), "{error}");
    }

    #[test]
    fn feed_redirect_over_limit_is_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let payload = format!("http://{address}/again");
        let server = std::thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut request);
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    payload
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        crate::net_guard::allow_loopback_in_tests(true);
        let result =
            fetch_feed_xml(&format!("http://{address}/feed.xml"), crate::net_guard::NetPolicy::PublicOnly);
        crate::net_guard::allow_loopback_in_tests(false);
        server.join().unwrap();
        let error = result.expect_err("第 4 跳必须被拒绝");
        assert!(error.contains("重定向超过"), "{error}");
    }
}
