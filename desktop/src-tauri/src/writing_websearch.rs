//! 写作素材联网检索（anysearch 薄封装）。
//!
//! 宿主只做三件事：检索词在 IPC 层过红线与掩码、调用本机 AnySearch CLI、
//! 解析结果。结果只返回界面，不落盘；用户点「存为素材」才经 material_create
//! 的既有闸门写入素材篮。CLI 不存在时明确报错，不安装、不伪造结果
//! （照 skills/anysearch/SKILL.md 的边界口径）。
//!
//! 证据等级：源码事实 + 模块内 cargo test（输出解析为纯函数）。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_RESULTS: u32 = 10;
/// 单次检索超时。检索词会发给外部服务；超时即 kill，不让界面无限等待。
const CLI_TIMEOUT: Duration = Duration::from_secs(45);
/// 单篇正文抓取超时（AnySearch extract 或直连回退）。
const EXTRACT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_QUERY_CHARS: usize = 200;
const MAX_ITEMS: usize = 10;
pub const MAX_EXTRACT_CHARS: usize = 20_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct WebSearchRequest {
    pub query: String,
    pub max_results: u32,
}

impl Default for WebSearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            max_results: DEFAULT_MAX_RESULTS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WebSearchItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebSearchOutcome {
    pub query: String,
    pub masked: bool,
    pub items: Vec<WebSearchItem>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebExtractRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebExtractOutcome {
    pub url: String,
    pub title: Option<String>,
    pub text: String,
    pub chars: usize,
    /// anysearch = 插件正文抽取；direct = 直连网页后去标签。
    pub source: String,
}

/// 定位本机 AnySearch CLI：环境变量优先，其次用户主目录下的既有技能目录。
pub fn locate_cli() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("ANYSEARCH_SKILL_DIR") {
        if !dir.trim().is_empty() {
            let candidate = PathBuf::from(dir).join("scripts").join("anysearch_cli.py");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let home = crate::platform::home_dir()?;
    for base in [".codex", ".agents"] {
        let candidate = home
            .join(base)
            .join("skills")
            .join("anysearch")
            .join("scripts")
            .join("anysearch_cli.py");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 内置纯 Python 依赖（requests/certifi）目录；找不到就不设置 PYTHONPATH。
fn python_vendor_dir(app_root: &Path) -> Option<PathBuf> {
    if let Some(raw) = std::env::var_os("OMNIDESK_PYTHON_VENDOR") {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            return Some(path);
        }
    }
    let candidates = [
        app_root
            .parent()
            .map(|root| root.join("runtime").join("python-vendor")),
        crate::platform::repo_root().map(|root| root.join("runtime").join("python-vendor")),
    ];
    candidates.into_iter().flatten().find(|path| path.is_dir())
}

fn run_cli(
    python: &str,
    script: &Path,
    args: &[String],
    vendor: Option<&Path>,
    timeout: Duration,
) -> Result<String, String> {
    let mut cmd = Command::new(python);
    cmd.arg(script)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(vendor) = vendor {
        let separator = if cfg!(windows) { ";" } else { ":" };
        let existing = std::env::var("PYTHONPATH").unwrap_or_default();
        let mut value = vendor.display().to_string();
        if !existing.trim().is_empty() {
            value.push_str(separator);
            value.push_str(&existing);
        }
        cmd.env("PYTHONPATH", value);
    }
    cmd.env("PYTHONUTF8", "1");
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动 AnySearch CLI 失败（{python}）：{e}"))?;
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(pipe) = stdout_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(pipe) = stderr_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("AnySearch 调用超时（{} 秒），已终止。", timeout.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("等待 AnySearch 检索失败：{e}"));
            }
        }
    };
    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    if !status.success() {
        let tail: String = String::from_utf8_lossy(&stderr)
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(format!("AnySearch 检索失败：{tail}"));
    }
    Ok(String::from_utf8_lossy(&stdout).to_string())
}

/// 解析 CLI 的 Markdown 输出。每个结果块形如：
/// `### 1. 标题` + `- **URL**: …` + `- 摘要… date: Oct 17, 2022`。
/// 末尾的 request_id YAML 块与空行忽略；没有结果块时把首行当说明返回。
pub fn parse_search_output(text: &str) -> (Vec<WebSearchItem>, Option<String>) {
    let mut items = Vec::new();
    for block in text.split("### ").skip(1) {
        let mut lines = block.lines();
        let head = lines.next().unwrap_or("").trim();
        let title = head
            .split_once(". ")
            .map(|(_, title)| title.trim())
            .unwrap_or(head)
            .trim();
        if title.is_empty() {
            continue;
        }
        let mut url = String::new();
        let mut snippet_parts: Vec<String> = Vec::new();
        let mut date: Option<String> = None;
        for raw in lines {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let body = line.strip_prefix("- ").unwrap_or(line).trim();
            if let Some(rest) = body.strip_prefix("**URL**") {
                url = rest
                    .trim_start_matches('*')
                    .trim_start_matches(':')
                    .trim()
                    .to_string();
                continue;
            }
            if let Some(index) = body.rfind(" date: ") {
                let (snippet, tail) = body.split_at(index);
                let tail = tail.trim_start_matches(" date: ").trim();
                if !tail.is_empty() {
                    date = Some(tail.to_string());
                }
                let snippet = snippet.trim();
                if !snippet.is_empty() {
                    snippet_parts.push(snippet.to_string());
                }
                continue;
            }
            snippet_parts.push(body.to_string());
        }
        let snippet = snippet_parts.join(" ").trim().to_string();
        if url.is_empty() && snippet.is_empty() {
            continue;
        }
        items.push(WebSearchItem {
            title: title.to_string(),
            url,
            snippet,
            date,
        });
        if items.len() >= MAX_ITEMS {
            break;
        }
    }
    let note = if items.is_empty() {
        Some(
            text.trim()
                .lines()
                .next()
                .unwrap_or("没有检索到结果。")
                .to_string(),
        )
    } else {
        None
    };
    (items, note)
}

/// 完整检索入口：入参已在 IPC 层过闸门；这里只定位 CLI、执行、解析。
pub fn web_search(
    request: &WebSearchRequest,
    python: &str,
    app_root: &Path,
    masked: bool,
) -> Result<WebSearchOutcome, String> {
    let query = request.query.trim();
    if query.is_empty() {
        return Err("检索词不能为空。".to_string());
    }
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(format!("检索词过长（最多 {MAX_QUERY_CHARS} 字）。"));
    }
    let script = locate_cli().ok_or_else(|| {
        "AnySearch 未安装或路径不可用；请先在本机技能目录安装 anysearch skill。".to_string()
    })?;
    let max_results = request.max_results.clamp(1, 10);
    let args = vec![
        "search".to_string(),
        query.to_string(),
        "--max_results".to_string(),
        max_results.to_string(),
    ];
    let output = run_cli(
        python,
        &script,
        &args,
        python_vendor_dir(app_root).as_deref(),
        CLI_TIMEOUT,
    )?;
    let (items, note) = parse_search_output(&output);
    Ok(WebSearchOutcome {
        query: query.to_string(),
        masked,
        items,
        note,
    })
}

/// 清理正文抽取结果里的图片与链接语法，并尽量切掉站点导航与页脚。
pub fn clean_extracted_text(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("![") {
            continue;
        }
        let cleaned = strip_markdown_links(trimmed);
        let cleaned = cleaned.trim();
        if cleaned.is_empty() {
            continue;
        }
        // 短行里同时出现站点操作词，基本是分享/打印/登录这类控件文本。
        let boilerplate = ["分享", "打印", "纠错", "微信扫一扫", "收听本文", "请先登录", "热搜"];
        if cleaned.chars().count() < 60
            && boilerplate.iter().any(|marker| cleaned.contains(marker))
        {
            continue;
        }
        if cleaned.starts_with("*发布时间") || cleaned.starts_with("发布时间") {
            continue;
        }
        lines.push(cleaned.to_string());
    }
    // 有文章标题时从标题行开始，前面的站点导航与热搜列表直接丢弃。
    if let Some(index) = lines.iter().position(|line| line.starts_with("# ")) {
        if index > 0 {
            lines.drain(0..index);
        }
    }
    // 页脚标记之后的内容不是正文。
    if let Some(index) = lines.iter().position(|line| {
        ["版权所有", "关于我们", "免责声明", "京ICP", "责任编辑"]
            .iter()
            .any(|marker| line.contains(marker))
    }) {
        lines.truncate(index.max(1));
    }
    let mut text = lines.join("\n\n");
    if text.chars().count() > MAX_EXTRACT_CHARS {
        text = text.chars().take(MAX_EXTRACT_CHARS).collect::<String>();
        text.push_str("\n\n…（全文已截断）");
    }
    text.trim().to_string()
}

/// 把 `[标题](链接)` 换成标题文本，整段图片语法直接去掉。
fn strip_markdown_links(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '!' && chars.get(index + 1) == Some(&'[') {
            if let Some(end) = markdown_target_end(&chars, index + 1) {
                index = end;
                continue;
            }
        }
        if chars[index] == '[' {
            if let Some(end) = markdown_target_end(&chars, index) {
                let label_start = index + 1;
                let label_end = chars[label_start..]
                    .iter()
                    .position(|ch| *ch == ']')
                    .map(|offset| label_start + offset)
                    .unwrap_or(label_start);
                out.extend(chars[label_start..label_end].iter());
                index = end;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

/// 从 `[` 或 `![` 的位置找到 `](…)` 结束后的下标（不含）。
fn markdown_target_end(chars: &[char], bracket: usize) -> Option<usize> {
    let label_end = chars[bracket..]
        .iter()
        .position(|ch| *ch == ']')
        .map(|offset| bracket + offset)?;
    if chars.get(label_end + 1) != Some(&'(') {
        return None;
    }
    let target_end = chars[label_end + 1..]
        .iter()
        .position(|ch| *ch == ')')
        .map(|offset| label_end + 1 + offset)?;
    Some(target_end + 1)
}

/// 抓取单页正文：先校验地址为公网 http/https，再调用 AnySearch extract。
pub fn extract_page(
    request: &WebExtractRequest,
    python: &str,
    app_root: &Path,
) -> Result<WebExtractOutcome, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("网页地址不能为空。".to_string());
    }
    let parsed = reqwest::Url::parse(url).map_err(|_| "网页地址不是有效的网址。".to_string())?;
    crate::net_guard::assert_public_http_url(parsed.as_str())?;
    // 先走 AnySearch 抽取（能处理一部分带反爬的站点）。
    let plugin_text = locate_cli().and_then(|script| {
        let args = vec!["extract".to_string(), url.to_string()];
        run_cli(
            python,
            &script,
            &args,
            python_vendor_dir(app_root).as_deref(),
            EXTRACT_TIMEOUT,
        )
        .ok()
    });
    if let Some((title, text)) = plugin_text
        .as_deref()
        .and_then(|output| parse_extract_output(output).ok())
        .map(|page| (page.title, clean_extracted_text(&page.content)))
    {
        if text.chars().count() >= 150 {
            return Ok(WebExtractOutcome {
                url: url.to_string(),
                title: title.filter(|value| !value.trim().is_empty()),
                chars: text.chars().count(),
                text,
                source: "anysearch".to_string(),
            });
        }
    }
    // 插件抽不到时直连网页：按响应头或 HTML meta 的 charset 解码，再去标签。
    let (title, raw_text) = fetch_direct(url).map_err(|_| {
        "这个页面没有可用的文字正文（可能是图片版或需登录），可换一条来源再试。".to_string()
    })?;
    let text = clean_extracted_text(&raw_text);
    if text.chars().count() < 80 {
        return Err(
            "这个页面没有可用的文字正文（可能是图片版或需登录），可换一条来源再试。"
                .to_string(),
        );
    }
    Ok(WebExtractOutcome {
        url: url.to_string(),
        title,
        chars: text.chars().count(),
        text,
        source: "direct".to_string(),
    })
}

/// 直连网页：手动跟随最多 3 次重定向，每一跳重新校验公网地址并钉住解析结果。
fn fetch_direct(url: &str) -> Result<(Option<String>, String), String> {
    let mut current =
        reqwest::Url::parse(url).map_err(|_| "网页地址不是有效的网址。".to_string())?;
    let mut response: Option<reqwest::blocking::Response> = None;
    for hop in 0..=3 {
        let addrs = crate::net_guard::assert_url_addresses(
            &current,
            crate::net_guard::NetPolicy::PublicOnly,
        )?;
        let builder = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (compatible; OmniDesk/0.2)")
            .redirect(reqwest::redirect::Policy::none());
        let client = crate::net_guard::pin_client(builder, &current, &addrs)
            .build()
            .map_err(|error| format!("创建网页请求失败：{error}"))?;
        let resp = client
            .get(current.clone())
            .header(
                reqwest::header::ACCEPT,
                "text/html,application/xhtml+xml,text/plain;q=0.9,*/*;q=0.8",
            )
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    "网页请求超时".to_string()
                } else {
                    format!("网页请求失败：{error}")
                }
            })?;
        if resp.status().is_redirection() {
            if hop == 3 {
                return Err("网页重定向次数过多。".to_string());
            }
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "网页重定向缺少目标地址。".to_string())?;
            current = current
                .join(location)
                .map_err(|_| "网页重定向地址无效。".to_string())?;
            continue;
        }
        if !resp.status().is_success() {
            return Err(format!("网页返回状态 {}。", resp.status()));
        }
        response = Some(resp);
        break;
    }
    let response = response.ok_or_else(|| "网页请求没有完成。".to_string())?;
    if response.content_length().unwrap_or(0) > 3_000_000 {
        return Err("网页过大，已跳过。".to_string());
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let bytes = response
        .bytes()
        .map_err(|error| format!("读取网页内容失败：{error}"))?;
    if bytes.len() > 3_000_000 {
        return Err("网页过大，已跳过。".to_string());
    }
    let decoded = detect_charset(content_type.as_deref(), &bytes).decode(&bytes).0;
    let title = html_title(&decoded);
    let text = strip_html(&decoded);
    Ok((title, text))
}

/// 优先响应头 charset，其次 HTML meta；都认不出时按 UTF-8 解码。
fn detect_charset(content_type: Option<&str>, bytes: &[u8]) -> &'static encoding_rs::Encoding {
    let from_header = content_type.and_then(|value| {
        let lower = value.to_ascii_lowercase();
        let index = lower.find("charset=")?;
        Some(lower[index + 8..].to_string())
    });
    let label = from_header.or_else(|| {
        let head = &bytes[..bytes.len().min(4096)];
        let lower = String::from_utf8_lossy(head).to_ascii_lowercase();
        let index = lower.find("charset=")?;
        Some(lower[index + 8..].to_string())
    });
    let label = label
        .map(|value| {
            value
                .trim_start_matches(['"', '\'', ' '])
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
                .collect::<String>()
        })
        .filter(|value| !value.is_empty());
    label
        .as_deref()
        .and_then(|value| encoding_rs::Encoding::for_label(value.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8)
}

fn html_title(html: &str) -> Option<String> {
    let open = find_ci(html, "<title")?;
    let start = html[open..].find('>')? + open + 1;
    let end = find_ci(&html[start..], "</title>")? + start;
    let title = html[start..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

/// 去掉 script/style 块后剥离标签；块级标签换成换行，实体做最小还原。
fn strip_html(html: &str) -> String {
    let without_script = remove_block_ci(html, "script");
    let without_style = remove_block_ci(&without_script, "style");
    let mut out = String::new();
    let mut rest = without_style.as_str();
    loop {
        let Some(lt) = rest.find('<') else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..lt]);
        let Some(gt) = rest[lt..].find('>') else {
            break;
        };
        let tag = &rest[lt + 1..lt + gt];
        let name = tag
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "br" | "p" | "div" | "li" | "tr" | "td" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                | "section" | "article" | "blockquote"
        ) {
            out.push('\n');
        }
        rest = &rest[lt + gt + 1..];
    }
    decode_entities(&out)
}

fn remove_block_ci(html: &str, name: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    loop {
        let Some(open) = find_ci(rest, &format!("<{name}")) else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        match find_ci(after, &format!("</{name}>")) {
            Some(end) => rest = &after[end + name.len() + 3..],
            None => break,
        }
    }
    out
}

fn decode_entities(text: &str) -> String {
    let mut out = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&ldquo;", "“")
        .replace("&rdquo;", "”")
        .replace("&mdash;", "—");
    while out.contains("&amp;") {
        out = out.replace("&amp;", "&");
    }
    out
}

#[derive(Debug, Deserialize)]
struct ExtractedPage {
    title: Option<String>,
    content: String,
}

/// CLI extract 输出是 `{"url","title","content"}`；个别版本直接打印正文，按文本兜底。
fn parse_extract_output(output: &str) -> Result<ExtractedPage, String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err("AnySearch 没有返回正文。".to_string());
    }
    if let Ok(page) = serde_json::from_str::<ExtractedPage>(trimmed) {
        return Ok(page);
    }
    Ok(ExtractedPage {
        title: None,
        content: trimmed.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "## Search Results (2 results, 1234ms)\n\n### 1. 二十大报告要点速览\n- **URL**: https://www.12371.cn/a\n- 学习贯彻要点 date: Oct 17, 2022\n\n### 2. 关键词解读\n- **URL**: https://cpc.people.com.cn/b\n- 全党必须牢记，全面从严治党永远在路上\n\n```yaml\nrequest_id: c19c\n```\n";

    #[test]
    fn parses_titles_urls_and_dates() {
        let (items, note) = parse_search_output(SAMPLE);
        assert!(note.is_none());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "二十大报告要点速览");
        assert_eq!(items[0].url, "https://www.12371.cn/a");
        assert_eq!(items[0].date.as_deref(), Some("Oct 17, 2022"));
        assert_eq!(items[1].date, None);
        assert!(items[1].snippet.contains("全面从严治党"));
    }

    #[test]
    fn zero_results_reports_note() {
        let (items, note) = parse_search_output("## Search Results (0 results, 88ms)\n");
        assert!(items.is_empty());
        assert!(note.is_some());
    }

    #[test]
    fn block_without_url_keeps_title_and_snippet() {
        let (items, _) = parse_search_output("### 3. 只有标题的条目\n- 摘要文字\n");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "只有标题的条目");
        assert_eq!(items[0].snippet, "摘要文字");
        assert_eq!(items[0].url, "");
    }

    #[test]
    fn clean_drops_navigation_and_footer() {
        let raw = "![](https://img.example/a.png)\n\n[首页](https://example.com/)\n\n[热搜](https://example.com/hot)\n\n# 报告要点\n\n第一段正文。\n\n第二段正文。\n\n关于我们\n\n版权所有\n";
        let text = clean_extracted_text(raw);
        assert!(text.starts_with("# 报告要点"), "{}", text);
        assert!(text.contains("第一段正文"), "{}", text);
        assert!(!text.contains("首页"), "{}", text);
        assert!(!text.contains("版权所有"), "{}", text);
    }

    #[test]
    fn image_only_page_leaves_short_text() {
        let raw = "# 标题\n\n*共产党员网* 分享 打印 纠错\n\n请先登录\n\n微信扫一扫 ×\n\n*发布时间：2022年10月17日* 分享 打印\n";
        assert!(clean_extracted_text(raw).chars().count() < 150);
    }

    #[test]
    fn strip_html_drops_script_and_keeps_paragraphs() {
        let html = "<html><head><script>var x = 1;</script><style>p{}</style></head><body><p>第一段</p><p>第二段&amp;说明</p></body></html>";
        let text = strip_html(html);
        assert!(text.contains("第一段"), "{text}");
        assert!(text.contains("第二段&说明"), "{text}");
        assert!(!text.contains("var x"), "{text}");
    }

    #[test]
    fn charset_prefers_header_and_falls_back_to_meta() {
        let bytes = b"<html><head><meta charset=\"gbk\"></head></html>";
        let from_header = detect_charset(Some("text/html; charset=utf-8"), bytes);
        assert_eq!(from_header.name(), "UTF-8");
        let from_meta = detect_charset(None, bytes);
        assert!(from_meta.name().to_ascii_lowercase().contains("gbk"), "{}", from_meta.name());
    }

    #[test]
    fn extract_output_accepts_json_and_plain_text() {
        let json = r#"{"url":"https://example.com/a","title":"标题","content":"正文内容"}"#;
        let page = parse_extract_output(json).unwrap();
        assert_eq!(page.title.as_deref(), Some("标题"));
        assert_eq!(page.content, "正文内容");
        let plain = parse_extract_output("直接打印的正文").unwrap();
        assert_eq!(plain.content, "直接打印的正文");
        assert!(plain.title.is_none());
    }
}
