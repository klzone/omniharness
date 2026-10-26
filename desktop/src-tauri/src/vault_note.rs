//! 单篇笔记的只读解析（OD-KB-001 P2-S01）。
//!
//! 这一层只做**读**：把已经由 P1 有界读取的 UTF-8 文本切成 Frontmatter 原始片段、
//! 正文和可识别的属性，从不改写、不规范化、不生成回写内容。保存时用到的永远是
//! 用户自己的原文，Frontmatter 解析结果只用于展示与筛选。
//!
//! 边界（由代码强制）：
//!   * 只认文件开头第一行的 `---` 围栏；没闭合、YAML 非法或根节点不是映射时，
//!     原文照常返回，正文原样可见，只追加警告 —— 不隐藏笔记、不阻断编辑；
//!   * 只有标量、标量列表这类可无损展示的结构才进入 `properties`；嵌套映射、
//!     带锚点/标签的对象等一律跳过并给出按字段名的警告；
//!   * 标签与别名只读识别：Frontmatter 的 `tags` / `aliases` 与正文 `#tag`；
//!     代码围栏与行内代码里的 `#` 不算标签；
//!   * 标签规范化只用于比较（去 `#`、去首尾空白、转小写），原文一律不改。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 可识别属性的取值。只保留能原样展示的形态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PropertyValue {
    Text(String),
    /// 数字按 YAML 原文形态保存为字符串，避免浮点/大整数在往返中变形。
    Number(String),
    Bool(bool),
    Null,
    List(Vec<String>),
}

/// 一个可展示的属性。`key` 保持 Frontmatter 里的原始大小写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    pub value: PropertyValue,
}

/// 解析警告：固定代码 + 可读说明。代码供界面聚合，说明给用户看。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseWarning {
    pub code: String,
    pub message: String,
    /// Frontmatter 里出问题的字段名（与字段无关时为 None）。
    pub key: Option<String>,
}

/// Frontmatter 在原文里的位置切片。原文一字不改，位置用字节偏移定位。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontmatterSlice {
    /// 开合围栏之间的原始文本（不含两条 `---` 行）。
    pub raw: String,
    /// 开围栏行起始字节（恒为 0，除非有 BOM）。
    pub start_byte: usize,
    /// 闭围栏行结束字节（含换行）。
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    /// `yaml` = YAML 可解析且是映射；`invalid` = 保留原文但不可解析；
    /// `unsupported` = 可解析但根节点不是映射。
    pub status: String,
}

/// 一篇笔记的解析结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedNote {
    /// 原始全文（含 Frontmatter 与换行原样）。
    pub raw: String,
    pub has_frontmatter: bool,
    pub frontmatter: Option<FrontmatterSlice>,
    /// 正文：闭围栏之后的内容；无 Frontmatter 时为全文。
    pub body: String,
    /// 正文起始字节偏移，用于把正文里的位置映射回原文。
    pub body_start_byte: usize,
    /// 正文起始行号（1 起）。
    pub body_start_line: usize,
    pub properties: Vec<Property>,
    /// 规范化后的标签（不含 `#`，小写），已去重并保持首次出现顺序。
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub warnings: Vec<ParseWarning>,
}

/// 原文的一行：起始字节、结束字节（含换行）与去掉行尾的文本。
struct Line<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

fn lines_of(text: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (idx, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            let mut body_end = idx;
            // 行尾 `\r` 归入换行，不算正文内容。
            if body_end > start && text.as_bytes()[body_end - 1] == b'\r' {
                body_end -= 1;
            }
            out.push(Line {
                start,
                end: idx + 1,
                text: &text[start..body_end],
            });
            start = idx + 1;
        }
    }
    if start <= text.len() {
        out.push(Line {
            start,
            end: text.len(),
            text: &text[start..],
        });
    }
    out
}

fn is_fence_line(text: &str) -> bool {
    let t = text.trim_end();
    t == "---" || t == "..."
}

/// 标签规范化：去 `#`、trim、转小写。只用于比较与展示，不改原文。
pub fn normalize_tag(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('#')
        .trim()
        .to_lowercase()
}

/// 一个字符是否可作为标签的一部分。
fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '/'
}

/// 从正文提取 `#tag`。跳过围栏代码块与行内代码；`#` 必须位于词首。
pub fn inline_tags(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut fence: Option<&str> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        for marker in ["```", "~~~"] {
            if trimmed.starts_with(marker) {
                fence = Some(marker);
                break;
            }
        }
        if fence.is_some() {
            continue;
        }
        // 先抹掉行内代码，避免 `#x` 被当标签。
        let masked = mask_inline_code(line);
        let chars: Vec<char> = masked.chars().collect();
        let mut idx = 0usize;
        while idx < chars.len() {
            if chars[idx] != '#' {
                idx += 1;
                continue;
            }
            // 词首判定：行首或前一个字符是空白/括号/引号/常见分隔符。
            let at_token_start = idx == 0
                || !(chars[idx - 1].is_alphanumeric()
                    || chars[idx - 1] == '_'
                    || chars[idx - 1] == '-'
                    || chars[idx - 1] == '/');
            if !at_token_start {
                idx += 1;
                continue;
            }
            let mut end = idx + 1;
            while end < chars.len() && is_tag_char(chars[end]) {
                end += 1;
            }
            if end > idx + 1 {
                let raw: String = chars[idx + 1..end].iter().collect();
                // Obsidian 语义：全是数字的不是标签。
                if raw.chars().any(|c| !c.is_ascii_digit()) {
                    push_unique(&mut out, normalize_tag(&raw));
                }
            }
            idx = end.max(idx + 1);
        }
    }
    out
}

/// 把行内代码替换成等长空格（保持字节位置不变，不影响标签边界判定）。
fn mask_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push('`');
            continue;
        }
        if in_code {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if value.is_empty() {
        return;
    }
    if !list.iter().any(|v| v == &value) {
        list.push(value);
    }
}

/// 把 Frontmatter 里的 tags/aliases 值读成字符串列表。
/// 支持标量（逗号或空白分隔，Obsidian 两种写法都常见）与标量列表。
fn strings_from_value(value: &serde_yaml::Value) -> Option<Vec<String>> {
    match value {
        serde_yaml::Value::String(s) => Some(
            s.split(',')
                .flat_map(|part| part.split_whitespace())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        ),
        serde_yaml::Value::Sequence(items) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    serde_yaml::Value::String(s) => out.push(s.clone()),
                    serde_yaml::Value::Number(n) => out.push(n.to_string()),
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn scalar_text(value: &serde_yaml::Value) -> Option<PropertyValue> {
    match value {
        serde_yaml::Value::String(s) => Some(PropertyValue::Text(s.clone())),
        serde_yaml::Value::Number(n) => Some(PropertyValue::Number(n.to_string())),
        serde_yaml::Value::Bool(b) => Some(PropertyValue::Bool(*b)),
        serde_yaml::Value::Null => Some(PropertyValue::Null),
        _ => None,
    }
}

fn scalar_list(value: &serde_yaml::Value) -> Option<PropertyValue> {
    let serde_yaml::Value::Sequence(items) = value else {
        return None;
    };
    let mut out = Vec::new();
    for item in items {
        match scalar_text(item)? {
            PropertyValue::Text(s) => out.push(s),
            PropertyValue::Number(n) => out.push(n),
            PropertyValue::Bool(b) => out.push(b.to_string()),
            PropertyValue::Null => out.push(String::new()),
            PropertyValue::List(_) => return None,
        }
    }
    Some(PropertyValue::List(out))
}

/// 解析一篇笔记的原文。
pub fn parse_note(raw: &str) -> ParsedNote {
    let mut warnings: Vec<ParseWarning> = Vec::new();
    let mut body_start_byte = 0usize;
    let mut body_start_line = 1usize;

    let lines = lines_of(raw);
    let has_bom = raw.starts_with('\u{feff}');
    if has_bom {
        warnings.push(ParseWarning {
            code: "bom".to_string(),
            message: "文件以 UTF-8 BOM 开头；已按纯文本保留，不把 BOM 当作 Frontmatter 起点。"
                .to_string(),
            key: None,
        });
    }

    let opens = !has_bom && lines.first().map(|l| l.text == "---").unwrap_or(false);
    if opens {
        match lines.iter().skip(1).position(|l| is_fence_line(l.text)) {
            Some(rel) => {
                let close = rel + 1;
                let open_line = &lines[0];
                let close_line = &lines[close];
                let slice_raw = &raw[open_line.end..close_line.start];
                let (status, parsed) = parse_frontmatter_yaml(slice_raw, &mut warnings);
                body_start_byte = close_line.end;
                body_start_line = close + 2;
                let body = &raw[body_start_byte..];
                let mut doc = ParsedNote {
                    raw: raw.to_string(),
                    has_frontmatter: true,
                    frontmatter: Some(FrontmatterSlice {
                        raw: slice_raw.to_string(),
                        start_byte: open_line.start,
                        end_byte: close_line.end,
                        start_line: 1,
                        end_line: close + 1,
                        status: status.to_string(),
                    }),
                    body: body.to_string(),
                    body_start_byte,
                    body_start_line,
                    properties: Vec::new(),
                    tags: Vec::new(),
                    aliases: Vec::new(),
                    warnings,
                };
                if let Some(map) = parsed {
                    collect_properties(&map, &mut doc);
                }
                finish_tags(&mut doc);
                return doc;
            }
            None => {
                warnings.push(ParseWarning {
                    code: "unterminated_frontmatter".to_string(),
                    message: "开头有 `---` 但到文件末尾都没有闭合；已按纯正文处理，可继续编辑。"
                        .to_string(),
                    key: None,
                });
            }
        }
    }

    let mut doc = ParsedNote {
        raw: raw.to_string(),
        has_frontmatter: false,
        frontmatter: None,
        body: raw[body_start_byte..].to_string(),
        body_start_byte,
        body_start_line,
        properties: Vec::new(),
        tags: Vec::new(),
        aliases: Vec::new(),
        warnings,
    };
    finish_tags(&mut doc);
    doc
}

/// 解析 Frontmatter 的 YAML。返回状态名与（可选的）根映射。
fn parse_frontmatter_yaml(
    raw: &str,
    warnings: &mut Vec<ParseWarning>,
) -> (&'static str, Option<serde_yaml::Mapping>) {
    match serde_yaml::from_str::<serde_yaml::Value>(raw) {
        Ok(serde_yaml::Value::Mapping(map)) => ("yaml", Some(map)),
        Ok(serde_yaml::Value::Null) => ("yaml", Some(serde_yaml::Mapping::new())),
        Ok(_) => {
            warnings.push(ParseWarning {
                code: "unsupported_frontmatter_shape".to_string(),
                message: "Frontmatter 不是键值映射；原文已保留，属性不展示。".to_string(),
                key: None,
            });
            ("unsupported", None)
        }
        Err(err) => {
            warnings.push(ParseWarning {
                code: "invalid_yaml".to_string(),
                message: format!("Frontmatter 不是有效 YAML（{}）；原文已保留，正文仍可读。", one_line(&err.to_string())),
                key: None,
            });
            ("invalid", None)
        }
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 从根映射收集属性、标签与别名。不支持的嵌套结构逐字段告警并跳过。
fn collect_properties(map: &serde_yaml::Mapping, doc: &mut ParsedNote) {
    // BTreeMap 只用于稳定排序；输出顺序按 key 排序，便于界面与测试复现。
    let mut sorted: BTreeMap<String, serde_yaml::Value> = BTreeMap::new();
    for (key, value) in map {
        let key_text = match key {
            serde_yaml::Value::String(s) => s.clone(),
            other => format!("{:?}", other),
        };
        sorted.insert(key_text, value.clone());
    }
    for (key, value) in sorted {
        let lower = key.to_lowercase();
        if lower == "tags" || lower == "tag" {
            match strings_from_value(&value) {
                Some(items) => {
                    for item in items {
                        push_unique(&mut doc.tags, normalize_tag(&item));
                    }
                }
                None => doc.warnings.push(ParseWarning {
                    code: "unsupported_value".to_string(),
                    message: format!("`{key}` 的结构不是标量或标量列表；原文已保留，该项不参与筛选。"),
                    key: Some(key.clone()),
                }),
            }
        }
        if lower == "aliases" || lower == "alias" {
            match strings_from_value(&value) {
                Some(items) => {
                    for item in items {
                        let trimmed = item.trim().to_string();
                        push_unique(&mut doc.aliases, trimmed);
                    }
                }
                None => doc.warnings.push(ParseWarning {
                    code: "unsupported_value".to_string(),
                    message: format!("`{key}` 的结构不是标量或标量列表；原文已保留。"),
                    key: Some(key.clone()),
                }),
            }
        }
        let value = match scalar_text(&value).or_else(|| scalar_list(&value)) {
            Some(value) => value,
            None => {
                doc.warnings.push(ParseWarning {
                    code: "unsupported_value".to_string(),
                    message: format!("`{key}` 是嵌套结构，本阶段只展示标量与标量列表；原文已保留。"),
                    key: Some(key.clone()),
                });
                continue;
            }
        };
        doc.properties.push(Property { key, value });
    }
}

/// 合并正文里的 `#tag`。Frontmatter 标签优先，保持首次出现顺序。
fn finish_tags(doc: &mut ParsedNote) {
    for tag in inline_tags(&doc.body) {
        push_unique(&mut doc.tags, tag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTHETIC: &str = "---\ntitle: 合成笔记\ntags:\n  - 项目/Alpha\n  - 待办\naliases: [别名一, 别名二]\npriority: 3\narchived: false\nreviewed:\n---\n\n# 正文标题\n\n正文提到 #正文标签 与 `#不是标签` 两种写法。\n\n```md\n# 代码里的井号\n```\n";

    #[test]
    fn parses_scalars_lists_tags_and_aliases() {
        let doc = parse_note(SYNTHETIC);
        assert!(doc.has_frontmatter);
        // 第 1 行开围栏、第 10 行闭围栏，正文从第 11 行的空行开始。
        assert_eq!(doc.body_start_line, 11);
        let fm = doc.frontmatter.as_ref().unwrap();
        assert_eq!(fm.status, "yaml");
        assert_eq!(fm.start_byte, 0);
        assert_eq!(&doc.raw[..3], "---");
        assert_eq!(&doc.raw[fm.end_byte..fm.end_byte + 1], "\n");
        assert!(fm.raw.contains("title: 合成笔记"));
        // 属性顺序稳定（按 key 排序），值形态可无损展示。
        let keys: Vec<&str> = doc.properties.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["aliases", "archived", "priority", "reviewed", "tags", "title"]
        );
        assert_eq!(
            doc.properties.iter().find(|p| p.key == "title").unwrap().value,
            PropertyValue::Text("合成笔记".to_string())
        );
        assert_eq!(
            doc.properties.iter().find(|p| p.key == "priority").unwrap().value,
            PropertyValue::Number("3".to_string())
        );
        assert_eq!(
            doc.properties.iter().find(|p| p.key == "archived").unwrap().value,
            PropertyValue::Bool(false)
        );
        assert_eq!(
            doc.properties.iter().find(|p| p.key == "reviewed").unwrap().value,
            PropertyValue::Null
        );
        assert_eq!(doc.tags, vec!["项目/alpha", "待办", "正文标签"]);
        assert_eq!(doc.aliases, vec!["别名一", "别名二"]);
        // 切点是闭围栏行的换行之后，正文首字符就是紧随其后的空行。
        assert!(doc.body.starts_with("\n# 正文标题"), "{:?}", &doc.body[..20]);
    }

    #[test]
    fn raw_text_is_never_altered() {
        let doc = parse_note(SYNTHETIC);
        assert_eq!(doc.raw, SYNTHETIC);
        assert_eq!(
            &doc.raw[doc.body_start_byte..],
            doc.body.as_str(),
            "正文必须是原文的连续切片"
        );
        let fm = doc.frontmatter.as_ref().unwrap();
        assert_eq!(&doc.raw[fm.start_byte..fm.end_byte], format!("---\n{}---\n", fm.raw).as_str());
    }

    #[test]
    fn plain_note_without_frontmatter_keeps_whole_body() {
        let raw = "# 只有正文\n\n#标签 在这里。\n";
        let doc = parse_note(raw);
        assert!(!doc.has_frontmatter);
        assert!(doc.frontmatter.is_none());
        assert_eq!(doc.body, raw);
        assert_eq!(doc.body_start_byte, 0);
        assert_eq!(doc.body_start_line, 1);
        assert_eq!(doc.tags, vec!["标签"]);
        assert!(doc.properties.is_empty());
        assert!(doc.warnings.is_empty());
    }

    #[test]
    fn invalid_yaml_keeps_text_and_warns() {
        let raw = "---\ntitle: 未闭合的引号\nbroken: [1, 2\n---\n正文仍然可读\n";
        let doc = parse_note(raw);
        assert!(doc.has_frontmatter);
        assert_eq!(doc.frontmatter.as_ref().unwrap().status, "invalid");
        assert!(doc.properties.is_empty());
        assert_eq!(doc.body, "正文仍然可读\n");
        assert!(doc.warnings.iter().any(|w| w.code == "invalid_yaml"));
        assert_eq!(doc.raw, raw);
    }

    #[test]
    fn unterminated_frontmatter_treated_as_body() {
        let raw = "---\ntitle: 没有闭合\n正文\n";
        let doc = parse_note(raw);
        assert!(!doc.has_frontmatter);
        assert_eq!(doc.body, raw);
        assert!(doc
            .warnings
            .iter()
            .any(|w| w.code == "unterminated_frontmatter"));
    }

    #[test]
    fn separator_inside_body_is_not_a_frontmatter_close() {
        let raw = "---\ntitle: 有正文分隔符\n---\n正文\n\n---\n\n后面的正文\n";
        let doc = parse_note(raw);
        assert_eq!(doc.frontmatter.as_ref().unwrap().end_line, 3);
        assert!(doc.body.contains("\n---\n"), "{}", doc.body);
        assert_eq!(doc.raw, raw);
    }

    #[test]
    fn crlf_is_preserved_and_body_offset_is_exact() {
        let raw = "---\r\ntitle: CRLF\r\n---\r\n正文\r\n";
        let doc = parse_note(raw);
        assert!(doc.has_frontmatter);
        assert_eq!(doc.frontmatter.as_ref().unwrap().status, "yaml");
        assert_eq!(doc.properties[0].value, PropertyValue::Text("CRLF".to_string()));
        assert_eq!(doc.body, "正文\r\n");
        assert_eq!(&doc.raw[doc.body_start_byte..], "正文\r\n");
        assert_eq!(doc.body_start_line, 4);
    }

    #[test]
    fn utf8_boundaries_and_bom_are_handled() {
        let raw = "\u{feff}---\ntitle: BOM\n---\n正文😀\n";
        let doc = parse_note(raw);
        assert!(!doc.has_frontmatter, "BOM 开头不当作 Frontmatter");
        assert_eq!(doc.body, raw);
        assert!(doc.warnings.iter().any(|w| w.code == "bom"));
        // 多字节正文的偏移必须落在字符边界上。
        let doc2 = parse_note("---\ntitle: 中文\n---\n😀表情\n");
        assert!(doc2.raw.is_char_boundary(doc2.body_start_byte));
        assert_eq!(doc2.body, "😀表情\n");
    }

    #[test]
    fn nested_values_are_skipped_with_warning_but_kept_raw() {
        let raw = "---\ntitle: 嵌套\nlayout:\n  kind: board\n  size: 3\nplain: 保留\n---\n正文\n";
        let doc = parse_note(raw);
        assert!(doc.warnings.iter().any(|w| {
            w.code == "unsupported_value" && w.key.as_deref() == Some("layout")
        }));
        let keys: Vec<&str> = doc.properties.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, vec!["plain", "title"]);
        assert_eq!(doc.raw, raw);
    }

    #[test]
    fn tags_ignore_fences_and_inline_code() {
        let body = "#真标签\n\n`#行内代码`\n\n```\n#围栏内\n```\n\n~~~\n#波浪围栏\n~~~\n\n不是词首 a#b\n";
        assert_eq!(inline_tags(body), vec!["真标签"]);
    }

    #[test]
    fn tags_are_normalized_but_deduplicated() {
        let raw = "---\ntags: [Alpha, alpha, Beta]\n---\n\n#Alpha 与 #gamma\n";
        let doc = parse_note(raw);
        assert_eq!(doc.tags, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn scalar_tag_string_supports_comma_and_space() {
        let raw = "---\ntags: 一个, 两个 三个\n---\n正文\n";
        let doc = parse_note(raw);
        assert_eq!(doc.tags, vec!["一个", "两个", "三个"]);
    }

    #[test]
    fn unknown_keys_are_kept_as_properties() {
        let raw = "---\ncustom_field: 未知但可展示\n---\n正文\n";
        let doc = parse_note(raw);
        assert_eq!(doc.properties.len(), 1);
        assert_eq!(doc.properties[0].key, "custom_field");
        assert_eq!(
            doc.properties[0].value,
            PropertyValue::Text("未知但可展示".to_string())
        );
        assert!(doc.warnings.is_empty());
    }

    #[test]
    fn empty_and_whitespace_notes_do_not_panic() {
        for raw in ["", "\n", "---\n---\n", "---\n"] {
            let doc = parse_note(raw);
            assert_eq!(doc.raw, raw);
        }
    }
}
