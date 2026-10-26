//! Vault 双链与链接语法解析（OD-KB-001 P3-S01）。
//!
//! 这一层只做纯文本解析：输入是已经通过 P1 有界读取与 P2 Frontmatter 解析的
//! 正文切片，输出是带原文字节区间、原始行号和上下文的引用事实。它不访问磁盘、
//! 不解析目标、不写任何文件。目标解析与关系索引在 [`crate::vault_index`]。
//!
//! 识别范围（首期矩阵）：
//!   * `[[目标]]`、`[[目标|显示名]]`、`[[目标#标题]]`、`[[目标#^块标识]]`；
//!   * `![[嵌入目标]]`，类型单独标记，不与普通链接混淆；
//!   * 标准 Markdown 行内链接 `[文字](目标)` 与图片 `![说明](目标)`；
//!   * 自动链接 `<https://…>` 归为外部链接，不参与笔记关系。
//!
//! 不识别（保留原文，不猜测）：引用式链接 `[文字][标签]`、脚注、HTML `<a>`、
//! 多行链接文字、跨行 `[[`。这些在语法矩阵里显式列为不支持，不会被静默当作
//! 普通文本以外的语义。
//!
//! 边界：代码围栏、行内代码与 Frontmatter 内的相似文本不产出链接。本层拿到的
//! 已是正文（Frontmatter 已由 [`crate::vault_note::parse_note`] 剥离），这里再处理
//! 代码围栏与行内代码。

use serde::{Deserialize, Serialize};

/// 单篇正文里最多返回的链接数，防止异常文本撑爆内存。
pub const LINK_MAX_PER_NOTE: usize = 2_000;
/// 链接目标原文的最大长度（字符）；超长视为不支持，不截断成看似有效目标。
pub const LINK_MAX_TARGET_CHARS: usize = 512;
/// 上下文片段最多保留的字符数。
const CONTEXT_CHARS: usize = 120;

/// 引用的语法类型。固定字符串，界面与索引共用同一套取值。
pub const LINK_KIND_WIKILINK: &str = "wikilink";
pub const LINK_KIND_EMBED: &str = "embed";
pub const LINK_KIND_MD_LINK: &str = "markdown_link";
pub const LINK_KIND_MD_IMAGE: &str = "markdown_image";
pub const LINK_KIND_AUTOLINK: &str = "autolink";

/// 一条正文引用事实。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRef {
    /// 语法类型：wikilink / embed / markdown_link / markdown_image / autolink。
    pub kind: String,
    /// 链接目标原文（已去掉 `[[ ]]`、`[ ]( )` 外壳，未做 URL 解码）。
    pub target: String,
    /// `|` 之后的显示名；没有时为 None。
    pub display: Option<String>,
    /// `#` 之后的锚点原文；没有时为 None。
    pub anchor: Option<String>,
    /// 锚点是否以 `^` 开头，即块引用。
    pub block_anchor: bool,
    /// 原文中的起始字节（指向整个引用语法的第一个字符，嵌入含 `!`）。
    pub start_byte: usize,
    /// 原文中的结束字节（不含）。
    pub end_byte: usize,
    /// 原文件行号（1 起），已加回 Frontmatter 占用的行数。
    pub line: usize,
    /// 该行去掉行尾的原文，用于关系面板展示上下文。
    pub context: String,
}

/// 解析时发现、无法表示成引用的可解释问题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkParseIssue {
    pub code: String,
    pub message: String,
    pub line: usize,
}

/// 一次正文解析的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkParseReport {
    pub links: Vec<LinkRef>,
    pub issues: Vec<LinkParseIssue>,
    /// 是否因单篇上限而停止；true 表示正文后半段可能还有引用未被收录。
    pub truncated: bool,
}

impl LinkRef {
    /// 是否为指向笔记内部的关系（外部 URL 不算）。
    pub fn is_internal(&self) -> bool {
        !is_external_target(&self.target)
    }
}

/// 解析一篇正文的链接事实。
///
/// `body_start_byte` / `body_start_line` 来自 [`crate::vault_note::ParsedNote`]，
/// 用于把正文内部偏移映射回原文的字节与行号。
pub fn parse_links(body: &str, body_start_byte: usize, body_start_line: usize) -> Vec<LinkRef> {
    parse_links_report(body, body_start_byte, body_start_line).links
}

/// 解析一篇正文，并返回引用之外的可解释问题与截断状态。
pub fn parse_links_report(
    body: &str,
    body_start_byte: usize,
    body_start_line: usize,
) -> LinkParseReport {
    let mut out: Vec<LinkRef> = Vec::new();
    let mut issues: Vec<LinkParseIssue> = Vec::new();
    let mut truncated = false;
    let mut fence: Option<char> = None;
    let mut offset = 0usize;
    for (index, raw_line) in body.split_inclusive('\n').enumerate() {
        let line_start = offset;
        offset += raw_line.len();
        let line = raw_line.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        let mut opened: Option<char> = None;
        for marker in ['`', '~'] {
            if trimmed.starts_with(&marker.to_string().repeat(3)) {
                opened = Some(marker);
                break;
            }
        }
        if let Some(marker) = opened {
            fence = Some(marker);
            continue;
        }
        if out.len() >= LINK_MAX_PER_NOTE {
            break;
        }
        let line_no = body_start_line + index;
        let masked = mask_inline_code(line);
        let mut cursor = 0usize;
        let mut unsupported_reported: Vec<&'static str> = Vec::new();
        while cursor < masked.len() {
            if out.len() >= LINK_MAX_PER_NOTE {
                if next_link_start(&masked, cursor).is_some() {
                    truncated = true;
                }
                break;
            }
            let Some(start) = next_link_start(&masked, cursor) else {
                break;
            };
            let Some(end) = span_end(&masked, start) else {
                if let Some((code, message)) = unsupported_reason(&masked[start..]) {
                    if !unsupported_reported.contains(&code) {
                        issues.push(LinkParseIssue {
                            code: code.to_string(),
                            message: message.to_string(),
                            line: line_no,
                        });
                        unsupported_reported.push(code);
                    }
                }
                cursor = start + 1;
                continue;
            };
            let raw = &line[start..end];
            match classify_report(raw, line_start + start) {
                Ok(mut link) => {
                    link.start_byte += body_start_byte;
                    link.end_byte += body_start_byte;
                    link.line = line_no;
                    link.context = context_of(line);
                    out.push(link);
                }
                Err(Some((code, message))) => issues.push(LinkParseIssue {
                    code: code.to_string(),
                    message: message.to_string(),
                    line: line_no,
                }),
                Err(None) => {}
            }
            cursor = end;
        }
    }
    LinkParseReport {
        links: out,
        issues,
        truncated,
    }
}

/// 判断一个“看起来像链接但解析失败”的起点属于哪种不支持/不完整语法。
///
/// 这里只解释**已经以链接记号开头**的片段，不扫描普通正文里的孤立 `[`、`<`，
/// 避免把代码、数学式或普通括号误报成链接问题。返回的 code 会进入索引的
/// `issues`，原文始终保持不变。
fn unsupported_reason(rest: &str) -> Option<(&'static str, &'static str)> {
    if rest.starts_with("![[") || rest.starts_with("[[") {
        if !rest.contains("]]") {
            return Some((
                "unclosed_wikilink",
                "双链没有在本行闭合，未收录为引用；原文保持不变",
            ));
        }
        return None;
    }
    if rest.starts_with("![") || rest.starts_with('[') {
        let open = rest.find('[')?;
        let after_open = &rest[open + 1..];
        let Some(close) = find_unescaped(after_open, ']') else {
            return None;
        };
        let after_close = &after_open[close + 1..];
        if after_close.starts_with('[') && !after_close.starts_with("[[") {
            return Some((
                "unsupported_reference_link",
                "引用式链接 `[文字][标签]` 暂不支持；原文保持不变",
            ));
        }
        if after_close.starts_with('(') {
            if !rest.contains(')') {
                return Some((
                    "unclosed_markdown_link",
                    "Markdown 链接缺少右括号，未收录为引用；原文保持不变",
                ));
            }
            return None;
        }
        if after_open.starts_with('^') {
            return Some(("unsupported_footnote", "脚注引用暂不支持；原文保持不变"));
        }
        return None;
    }
    if rest.starts_with('<') {
        let lower = rest.to_ascii_lowercase();
        if lower.starts_with("<a ") || lower.starts_with("<a>") {
            return Some(("unsupported_html_link", "HTML 链接暂不支持；原文保持不变"));
        }
    }
    None
}

/// 把行内代码替换成等长空格，保持字节位置不变。
fn mask_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    let mut ticks = 0usize;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            if in_code {
                ticks = 1;
            }
            out.push('`');
            continue;
        }
        if in_code {
            ticks += 1;
            for _ in 0..c.len_utf8() {
                out.push(' ');
            }
            continue;
        }
        out.push(c);
    }
    let _ = ticks;
    out
}

/// 找到下一个可能是链接的起点字节。只做粗定位，具体语法由 [`span_end`] 与
/// [`classify`] 校验；校验失败会从下一位继续找，不会丢掉后续真实链接。
fn next_link_start(text: &str, from: usize) -> Option<usize> {
    let mut iter = text.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if i < from {
            continue;
        }
        if !matches!(c, '!' | '[' | '<') {
            continue;
        }
        if text[i..].starts_with("![[")
            || text[i..].starts_with("[[")
            || text[i..].starts_with("![")
            || text[i..].starts_with('[')
            || text[i..].starts_with('<')
        {
            return Some(i);
        }
    }
    None
}

/// 返回从 `start` 开始的完整引用语法结束字节（不含）。
fn span_end(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start..];
    if let Some(inner) = rest.strip_prefix("![[").or_else(|| rest.strip_prefix("[[")) {
        let rel = inner.find("]]")?;
        // 双链不能跨过另一个 `[[` 或换行：那说明这一对括号并未配对，
        // 交给下一位重新寻找，避免把两段文本吞成一个链接。
        let target_head = &inner[..rel];
        if target_head.contains("[[") || target_head.contains('\n') {
            return None;
        }
        return Some(start + rest.len() - inner.len() + rel + 2);
    }
    if rest.starts_with("![") || rest.starts_with('[') {
        let open = rest.find('[')?;
        let after_open = &rest[open + 1..];
        let close = find_unescaped(after_open, ']')?;
        let paren_at = open + 1 + close + 1;
        if !rest[paren_at..].starts_with('(') {
            return None;
        }
        let mut depth = 0usize;
        for (idx, c) in rest[paren_at..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(start + paren_at + idx + 1);
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    if rest.starts_with('<') {
        let close = rest.find('>')?;
        if is_external_target(&rest[1..close]) {
            return Some(start + close + 1);
        }
    }
    None
}

fn find_unescaped(text: &str, needle: char) -> Option<usize> {
    let mut escaped = false;
    for (idx, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == needle {
            return Some(idx);
        }
    }
    None
}

/// 把一段原始链接文本解析成事实。`Err(None)` 表示不构成引用；
/// `Err(Some(..))` 表示语法成立但无法安全表示，需要让上层记录问题。
fn classify_report(raw: &str, local_start: usize) -> Result<LinkRef, Option<(&'static str, &'static str)>> {
    let (kind, inner) = if let Some(rest) = raw.strip_prefix("![[") {
        (LINK_KIND_EMBED, rest.strip_suffix("]]").ok_or(None)?)
    } else if let Some(rest) = raw.strip_prefix("[[") {
        (LINK_KIND_WIKILINK, rest.strip_suffix("]]").ok_or(None)?)
    } else if let Some(rest) = raw.strip_prefix("![") {
        (LINK_KIND_MD_IMAGE, rest.strip_suffix(')').ok_or(None)?)
    } else if let Some(rest) = raw.strip_prefix('[') {
        (LINK_KIND_MD_LINK, rest.strip_suffix(')').ok_or(None)?)
    } else if let Some(rest) = raw.strip_prefix('<') {
        (LINK_KIND_AUTOLINK, rest.strip_suffix('>').ok_or(None)?)
    } else {
        return Err(None);
    };
    // 标准 Markdown 目标里的 `#` 是 URL 片段，属于同一个路径，不能再切分；
    // 只有 Obsidian 双链的 `#` 才表示笔记内锚点。
    let (display, target_raw, anchor_raw) = if kind == LINK_KIND_AUTOLINK {
        (None, inner, None)
    } else if kind == LINK_KIND_MD_LINK || kind == LINK_KIND_MD_IMAGE {
        let close = inner.find("](").ok_or(None)?;
        let target = &inner[close + 2..];
        let target = target.trim();
        // 丢弃可选标题：`a.md "标题"`，目标本身不变。
        let target = match target.find(" \"") {
            Some(at) => target[..at].trim_end(),
            None => target,
        };
        (Some(inner[..close].to_string()), target, None)
    } else {
        match inner.split_once('|') {
            Some((target, display)) => {
                let (t, a) = match target.split_once('#') {
                    Some((t, a)) => (t, Some(a)),
                    None => (target, None),
                };
                (Some(display.to_string()), t, a)
            }
            None => {
                let (t, a) = match inner.split_once('#') {
                    Some((t, a)) => (t, Some(a)),
                    None => (inner, None),
                };
                (None, t, a)
            }
        }
    };
    let trimmed_target = target_raw.trim();
    if trimmed_target.chars().count() > LINK_MAX_TARGET_CHARS {
        return Err(Some((
            "target_too_long",
            "链接目标超过 512 字符，未收录为引用；原文保持不变",
        )));
    }
    let anchor = anchor_raw.map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
    let block_anchor = anchor.as_deref().map(|a| a.starts_with('^')).unwrap_or(false);
    Ok(LinkRef {
        kind: kind.to_string(),
        target: trimmed_target.to_string(),
        display,
        anchor,
        block_anchor,
        start_byte: local_start,
        end_byte: local_start + raw.len(),
        line: 1,
        context: String::new(),
    })
}

fn context_of(line: &str) -> String {
    if line.chars().count() <= CONTEXT_CHARS {
        return line.trim().to_string();
    }
    let head: String = line.chars().take(CONTEXT_CHARS).collect();
    format!("{}…", head.trim_end())
}

/// 是否是外部目标（协议开头或 `mailto:`）。这类目标不参与笔记关系。
pub fn is_external_target(target: &str) -> bool {
    let lower = target.trim().to_lowercase();
    if lower.starts_with("//") {
        return true;
    }
    for scheme in ["http://", "https://", "mailto:", "ftp://", "tel:"] {
        if lower.starts_with(scheme) {
            return true;
        }
    }
    // 其它显式协议（含 `obsidian://`）按外部处理，不做自作主张的映射。
    match lower.split_once("://") {
        Some((scheme, _)) => {
            !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(links: &[LinkRef]) -> Vec<&str> {
        links.iter().map(|l| l.kind.as_str()).collect()
    }

    #[test]
    fn wikilink_variants_keep_target_display_and_anchor() {
        let body = "a [[笔记]] b [[笔记|别名]] c [[笔记#标题]] d [[笔记#^blk-1]]\n";
        let links = parse_links(body, 0, 1);
        assert_eq!(links.len(), 4, "{links:#?}");
        assert_eq!(links[0].target, "笔记");
        assert_eq!(links[0].display, None);
        assert_eq!(links[1].display.as_deref(), Some("别名"));
        assert_eq!(links[1].target, "笔记");
        assert_eq!(links[2].anchor.as_deref(), Some("标题"));
        assert!(!links[2].block_anchor);
        assert_eq!(links[3].anchor.as_deref(), Some("^blk-1"));
        assert!(links[3].block_anchor);
        assert_eq!(links[3].target, "笔记");
    }

    #[test]
    fn embed_is_a_distinct_kind() {
        let links = parse_links("看图片 ![[图.png]] 和 ![[另一篇]]\n", 0, 1);
        assert_eq!(kinds(&links), vec![LINK_KIND_EMBED, LINK_KIND_EMBED]);
        assert_eq!(links[0].target, "图.png");
        assert!(links[0].is_internal());
    }

    #[test]
    fn markdown_links_images_and_autolinks() {
        let body = "[文档](../docs/a.md) ![图](img/p.png) <https://example.com> <me@example.com>\n";
        let links = parse_links(body, 0, 1);
        assert_eq!(kinds(&links), vec![LINK_KIND_MD_LINK, LINK_KIND_MD_IMAGE, LINK_KIND_AUTOLINK]);
        assert_eq!(links[0].target, "../docs/a.md");
        assert_eq!(links[0].display.as_deref(), Some("文档"));
        assert_eq!(links[1].target, "img/p.png");
        assert!(links[2].is_internal() == false);
    }

    #[test]
    fn code_fences_and_inline_code_never_produce_links() {
        let body = "```\n[[围栏内]]\n```\n`[[行内]]` 正常 [[真链接]]\n~~~rust\n[t](x.md)\n~~~\n";
        let links = parse_links(body, 0, 1);
        assert_eq!(links.len(), 1, "{links:#?}");
        assert_eq!(links[0].target, "真链接");
    }

    #[test]
    fn byte_offsets_and_lines_map_back_to_original_file() {
        // 模拟 4 行 Frontmatter + 闭围栏：正文从原文件第 6 行、第 20 字节开始。
        let body = "第一行\n第二行 [[目标]]\n";
        let links = parse_links(body, 20, 6);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].line, 7);
        // “第一行\n” = 3 汉字(9) + 换行(1) = 10 字节；“第二行 ” = 3 汉字(9) + 空格 = 10 字节。
        assert_eq!(links[0].start_byte, 20 + 10 + 10);
        // 切片确实落在 `[[` 上。
        let original = format!("....................{body}");
        assert!(original[links[0].start_byte..].starts_with("[["));
    }

    #[test]
    fn unmatched_or_unsupported_forms_fall_back_to_text() {
        let body = "[[未闭合 [文字]( [文字][引用] [[{{empty}}]] [[a\nb]]\n";
        let links = parse_links(body, 0, 1);
        assert_eq!(links.len(), 1, "{links:#?}");
        assert_eq!(links[0].target, "{{empty}}");
    }

    #[test]
    fn nested_parens_in_markdown_target_are_kept() {
        let links = parse_links("[a](x(y).md) 与 [[x]]\n", 0, 1);
        assert_eq!(links[0].target, "x(y).md");
        assert_eq!(links[1].target, "x");
    }

    #[test]
    fn external_targets_are_detected() {
        assert!(is_external_target("https://a.example/x"));
        assert!(is_external_target("mailto:a@b.c"));
        assert!(is_external_target("obsidian://open?vault=x"));
        assert!(!is_external_target("笔记"));
        assert!(!is_external_target("a.md"));
    }

    #[test]
    fn empty_target_and_markdown_fragment_keep_expected_shape() {
        // `[[#标题]]` 指当前笔记内部锚点：目标为空，锚点保留。
        let links = parse_links("[[#本地标题]] 与 [x](a.md#片段)\n", 0, 1);
        assert_eq!(links[0].target, "");
        assert_eq!(links[0].anchor.as_deref(), Some("本地标题"));
        assert_eq!(links[1].target, "a.md#片段");
        assert_eq!(links[1].anchor, None);
    }

    #[test]
    fn markdown_optional_title_is_not_part_of_target() {
        let links = parse_links("[x](a.md \"提示\")\n", 0, 1);
        assert_eq!(links[0].target, "a.md");
    }

    #[test]
    fn unsupported_forms_produce_explicit_issues_once_per_line() {
        let body = "[文字][标签] [^1] <a href=\"x.md\">x</a> [[未闭合\n正常 [[目标]]\n";
        let report = parse_links_report(body, 0, 1);
        assert_eq!(report.links.len(), 1, "{report:#?}");
        assert_eq!(report.links[0].target, "目标");
        let codes: Vec<&str> = report.issues.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"unsupported_reference_link"), "{codes:?}");
        assert!(codes.contains(&"unsupported_footnote"), "{codes:?}");
        assert!(codes.contains(&"unsupported_html_link"), "{codes:?}");
        assert!(codes.contains(&"unclosed_wikilink"), "{codes:?}");
        assert!(report.issues.iter().all(|i| i.line == 1), "{:#?}", report.issues);
    }

    #[test]
    fn plain_brackets_and_comparisons_are_not_reported_as_unsupported() {
        let report = parse_links_report("普通 [文字] 与 a < b 与 f(x) > 1\n", 0, 1);
        assert!(report.links.is_empty(), "{report:#?}");
        assert!(report.issues.is_empty(), "{:#?}", report.issues);
    }
}
