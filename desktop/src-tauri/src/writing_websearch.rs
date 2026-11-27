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

pub const DEFAULT_MAX_RESULTS: u32 = 6;
/// 单次检索超时。检索词会发给外部服务；超时即 kill，不让界面无限等待。
const CLI_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_QUERY_CHARS: usize = 200;
const MAX_ITEMS: usize = 10;

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
    query: &str,
    max_results: u32,
    vendor: Option<&Path>,
) -> Result<String, String> {
    let mut cmd = Command::new(python);
    cmd.arg(script)
        .arg("search")
        .arg(query)
        .arg("--max_results")
        .arg(max_results.to_string())
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
    let deadline = Instant::now() + CLI_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!(
                    "AnySearch 检索超时（{} 秒），已终止。",
                    CLI_TIMEOUT.as_secs()
                ));
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
    let output = run_cli(
        python,
        &script,
        query,
        max_results,
        python_vendor_dir(app_root).as_deref(),
    )?;
    let (items, note) = parse_search_output(&output);
    Ok(WebSearchOutcome {
        query: query.to_string(),
        masked,
        items,
        note,
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
}
