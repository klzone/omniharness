//! 事实核验确定性引擎（OD-WT-609）。
//!
//! 纯函数、无 IO：输入正文全文、素材/知识依据与章节生成侧的四分法标注，
//! 输出核验结论、口径冲突提示与统计。落盘归 603 的 writing_records 函数，
//! 本模块不读写文件、不调用模型。
//!
//! 口径：
//!   * 提取数字（含百分比/金额/单位）、日期（YYYY-MM-DD 与 YYYY年X月）、
//!     机构名（「局/厅/处/科/中心/办」后缀启发式）、人员职务（职务+姓名）；
//!   * 匹配复用 501 material_store::normalize_quote 的空白折叠，再对数字与
//!     日期做等价化（去千分位、全角转半角、日期统一为 ISO 形状）；
//!   * 八类结论按 D-087：素材命中→直接来源，知识命中→间接来源，同指标
//!     不同值→来源冲突，知识过期→来源已过期，无命中→无法找到来源，
//!     分析/判断→属于分析判断，建议/计划→属于工作计划，概括句→表达性概括；
//!   * 口径冲突只输出冲突对，不合并数值；
//!   * 记录定位固定为章节号+段落序号+被核验片段（603 R4 字段）。
//!
//! 证据等级：本文件为 static（源码事实）；模块内测试为 script（cargo test 运行）。

use serde::{Deserialize, Serialize};

use crate::material_store::normalize_quote;
use crate::sha256::sha256_hex;
use crate::writing_records::{CheckConclusion, CheckObject, CitationSourceKind, FactBasis};
use crate::writing_section::SECTION_PLACEHOLDER;

pub const MAX_FACT_TEXT_CHARS: usize = 200;
pub const MAX_FULL_TEXT_CHARS: usize = 2_000_000;
pub const MAX_ANNOTATION_TEXT_CHARS: usize = 20_000;
pub const MAX_ANNOTATIONS: usize = 4_000;
pub const MAX_EVIDENCE_QUOTE_CHARS: usize = 1_200;
pub const MAX_SCOPE_CONFLICTS: usize = 100;

/// 607 四分法标注，外带「计划/概括」两个核验分类别名。
pub const ANNOTATION_KINDS: &[&str] = &["事实", "分析", "判断", "建议", "计划", "概括"];

/// 章节生成侧四分法标注（607 SectionDraft.segments 顺车产出）。
///
/// 607 目前只在生成/确认路径校验 segments，未把标注落盘；界面侧可把最近
/// 一次生成稿的标注随核验请求传入，609 按正文定位后合并。缺省为空时只
/// 产出确定性结论。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FactAnnotationInput {
    pub section_no: u32,
    pub paragraph_no: Option<u32>,
    pub kind: String,
    pub text: String,
}

/// 知识依据（界面侧或知识库读取结果）。source_id 与 601 的
/// selected_knowledge_ids 同口径；outdated 表示知识条目过期标记联动。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KnowledgeEvidenceInput {
    pub source_id: String,
    pub quote: String,
    pub outdated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FactCheckRunRequest {
    pub writing_id: String,
    pub knowledge_evidence: Vec<KnowledgeEvidenceInput>,
    pub annotations: Vec<FactAnnotationInput>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FactCheckResolveRequest {
    pub writing_id: String,
    pub record_id: String,
    pub resolution: String,
    pub note: Option<String>,
}

/// 匹配依据。素材 source_quote 与知识引用都在这里统一成同一形状。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactEvidence {
    pub source_kind: CitationSourceKind,
    pub source_id: String,
    pub quote: String,
    pub outdated: bool,
}

/// 正文中的一个核验候选。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactCandidate {
    pub section_no: u32,
    pub paragraph_no: u32,
    pub check_object: CheckObject,
    pub fact_text: String,
    /// 候选在正文中的原始写法（如「1,200万元」「2026年3月」）。
    pub surface: String,
    /// 等价化后的比较值（如「1200万元」「2026-03」）。
    pub value: String,
    /// 同指标前缀（已去掉时间/统计口径词）；用于口径冲突与依据冲突。
    pub metric: Option<String>,
    /// 人员职务的角色（如「局长」）；其他对象为空。
    pub role: Option<String>,
    pub time_scope: Option<String>,
    pub stat_scope: Option<String>,
}

/// 一条确定性核验结论；record_id 由正文位置、对象、取值与结论稳定生成。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactFinding {
    pub record_id: String,
    pub section_no: u32,
    pub paragraph_no: u32,
    pub check_object: CheckObject,
    pub conclusion: CheckConclusion,
    pub basis: Vec<FactBasis>,
    pub fact_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictSide {
    pub section_no: u32,
    pub paragraph_no: u32,
    pub fact_text: String,
    pub value: String,
    pub time_scope: Option<String>,
    pub stat_scope: Option<String>,
}

/// 同一指标不同时间范围/统计口径/取值的并列提示；只提示，不合并。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeConflict {
    pub metric: String,
    pub reason: String,
    pub left: ConflictSide,
    pub right: ConflictSide,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FactCheckStats {
    pub candidates: usize,
    pub findings: usize,
    pub scope_conflicts: usize,
    pub annotations_used: usize,
    pub annotations_skipped: usize,
    pub direct_source: usize,
    pub indirect_source: usize,
    pub source_conflict: usize,
    pub source_outdated: usize,
    pub source_missing: usize,
    pub analysis_judgment: usize,
    pub work_plan: usize,
    pub expressive_summary: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactCheckOutcome {
    pub findings: Vec<FactFinding>,
    pub conflicts: Vec<ScopeConflict>,
    pub stats: FactCheckStats,
}

/// 核验运行回执：records 为运行后 factcheck.jsonl 的完整清单（含历史处理状态）。
#[derive(Debug, Clone, Serialize)]
pub struct FactCheckRunResponse {
    pub writing_id: String,
    pub status: String,
    pub records: Vec<crate::writing_records::FactCheckRecord>,
    pub conflicts: Vec<ScopeConflict>,
    pub stats: FactCheckStats,
    pub appended: usize,
    pub skipped_existing: usize,
    pub warnings: Vec<String>,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FactCheckResolveResponse {
    pub record: crate::writing_records::FactCheckRecord,
    /// 备注只做闸门与掩码校验；603 记录没有备注字段，不落盘。
    pub note: Option<String>,
    pub masked: bool,
    pub masked_id_card: u64,
    pub masked_phone: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullTextParagraph {
    pub section_no: u32,
    pub paragraph_no: u32,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnnotationClass {
    Fact,
    Analysis,
    Judgment,
    Plan,
    Summary,
}

impl AnnotationClass {
    fn as_str(self) -> &'static str {
        match self {
            AnnotationClass::Fact => "事实",
            AnnotationClass::Analysis => "分析",
            AnnotationClass::Judgment => "判断",
            AnnotationClass::Plan => "建议",
            AnnotationClass::Summary => "概括",
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedAnnotation {
    section_no: u32,
    paragraph_no: u32,
    class: AnnotationClass,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DateToken {
    start: usize,
    end: usize,
    surface: String,
    canonical: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NumberToken {
    start: usize,
    end: usize,
    surface: String,
    numeric: String,
    unit: String,
    value: String,
    metric: Option<String>,
    time_scope: Option<String>,
    stat_scope: Option<String>,
}

impl NumberToken {
    fn matches(&self, candidate: &FactCandidate) -> bool {
        if self.numeric != numeric_part(&candidate.value) {
            return false;
        }
        let candidate_unit = unit_part(&candidate.value);
        candidate_unit.is_empty() || candidate_unit == self.unit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersonToken {
    start: usize,
    end: usize,
    surface: String,
    role: String,
    name: String,
}

fn numeric_part(value: &str) -> String {
    value
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

fn unit_part(value: &str) -> String {
    value
        .chars()
        .skip_while(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

fn ascii_digit(ch: char) -> Option<u32> {
    match ch {
        '0'..='9' => Some(ch as u32 - '0' as u32),
        '０'..='９' => Some(ch as u32 - '０' as u32),
        _ => None,
    }
}

fn map_wide(ch: char) -> char {
    match ch {
        '０'..='９' => char::from_u32('0' as u32 + (ch as u32 - '０' as u32)).unwrap_or(ch),
        '％' => '%',
        '．' => '.',
        '，' => ',',
        '－' | '—' | '–' => '-',
        other => other,
    }
}

/// 501 normalize_quote 的空白折叠 + 全角转半角 + 去空白，用于确定性比较。
pub fn canonical_text(value: &str) -> String {
    let collapsed = normalize_quote(value);
    collapsed
        .chars()
        .map(map_wide)
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn is_cjk(ch: char) -> bool {
    !ch.is_ascii() && ch.is_alphabetic()
}

fn starts_with_chars(cs: &[char], start: usize, needle: &str) -> bool {
    let needle: Vec<char> = needle.chars().collect();
    start + needle.len() <= cs.len() && cs[start..start + needle.len()] == needle[..]
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap { 29 } else { 28 }
        }
        _ => 0,
    }
}

fn parse_date_at(cs: &[char], start: usize) -> Option<DateToken> {
    if start + 4 > cs.len() {
        return None;
    }
    let mut year = 0u32;
    for index in start..start + 4 {
        year = year * 10 + ascii_digit(cs[index])?;
    }
    if start + 10 <= cs.len()
        && cs[start + 4] == '-'
        && cs[start + 5..start + 7]
            .iter()
            .all(|ch| ascii_digit(*ch).is_some())
        && cs[start + 7] == '-'
        && cs[start + 8..start + 10]
            .iter()
            .all(|ch| ascii_digit(*ch).is_some())
    {
        let month = ascii_digit(cs[start + 5])? * 10 + ascii_digit(cs[start + 6])?;
        let day = ascii_digit(cs[start + 8])? * 10 + ascii_digit(cs[start + 9])?;
        if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return None;
        }
        let surface: String = cs[start..start + 10].iter().collect();
        return Some(DateToken {
            start,
            end: start + 10,
            surface,
            canonical: format!("{year:04}-{month:02}-{day:02}"),
        });
    }
    if start + 5 > cs.len() || cs[start + 4] != '年' {
        return None;
    }
    let mut index = start + 5;
    let mut month = 0u32;
    let mut month_digits = 0usize;
    while index < cs.len() && month_digits < 2 {
        let Some(digit) = ascii_digit(cs[index]) else {
            break;
        };
        month = month * 10 + digit;
        month_digits += 1;
        index += 1;
    }
    if month_digits == 0 {
        return Some(DateToken {
            start,
            end: start + 5,
            surface: cs[start..start + 5].iter().collect(),
            canonical: format!("{year:04}"),
        });
    }
    if month == 0 || month > 12 || index >= cs.len() || cs[index] != '月' {
        return None;
    }
    index += 1;
    let mut day = None;
    let mut day_digits = 0usize;
    while index < cs.len() && day_digits < 2 {
        let Some(digit) = ascii_digit(cs[index]) else {
            break;
        };
        let next = day.unwrap_or(0) * 10 + digit;
        if next > 31 {
            break;
        }
        day = Some(next);
        day_digits += 1;
        index += 1;
    }
    let end = if day.is_some() && index < cs.len() && cs[index] == '日' {
        index + 1
    } else {
        index
    };
    if let Some(value) = day {
        if value == 0 || value > days_in_month(year, month) {
            return None;
        }
        return Some(DateToken {
            start,
            end,
            surface: cs[start..end].iter().collect(),
            canonical: format!("{year:04}-{month:02}-{value:02}"),
        });
    }
    Some(DateToken {
        start,
        end,
        surface: cs[start..end].iter().collect(),
        canonical: format!("{year:04}-{month:02}"),
    })
}

fn date_tokens(cs: &[char]) -> Vec<DateToken> {
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < cs.len() {
        if ascii_digit(cs[index]).is_some() {
            if let Some(token) = parse_date_at(cs, index) {
                index = token.end;
                tokens.push(token);
                continue;
            }
        }
        index += 1;
    }
    tokens
}

fn date_mask(cs: &[char], dates: &[DateToken]) -> Vec<bool> {
    let mut mask = vec![false; cs.len()];
    for token in dates {
        for slot in mask.iter_mut().take(token.end).skip(token.start) {
            *slot = true;
        }
    }
    mask
}

const UNITS: &[&str] = &[
    "个百分点", "平方米", "公里", "千米", "万元", "亿元", "小时", "分钟", "美元", "元",
    "万", "亿", "吨", "亩", "家", "所", "项", "次", "人", "户", "条", "件", "台", "套",
    "米", "个", "倍", "天", "%", "％",
];

fn match_unit(cs: &[char], start: usize) -> Option<(String, usize)> {
    for unit in UNITS {
        if starts_with_chars(cs, start, unit) {
            let normalized = map_wide(unit.chars().next()?).to_string()
                + &unit.chars().skip(1).collect::<String>();
            return Some((normalized, start + unit.chars().count()));
        }
    }
    None
}

fn canonical_numeric(raw: &str) -> String {
    let mut integer = String::new();
    let mut fraction = String::new();
    let mut seen_dot = false;
    for ch in raw.chars() {
        if ch == '.' {
            seen_dot = true;
            continue;
        }
        if !seen_dot {
            integer.push(ch);
        } else {
            fraction.push(ch);
        }
    }
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        integer.to_string()
    } else {
        format!("{integer}.{fraction}")
    }
}

fn parse_number_at(cs: &[char], start: usize) -> Option<NumberToken> {
    if start >= cs.len() || ascii_digit(cs[start]).is_none() {
        return None;
    }
    let mut digits = String::new();
    let mut index = start;
    let mut seen_dot = false;
    loop {
        if index >= cs.len() {
            break;
        }
        if let Some(digit) = ascii_digit(cs[index]) {
            digits.push(char::from_u32('0' as u32 + digit)?);
            index += 1;
            continue;
        }
        if !seen_dot && (cs[index] == '.' || cs[index] == '．') {
            seen_dot = true;
            digits.push('.');
            index += 1;
            continue;
        }
        if (cs[index] == ',' || cs[index] == '，')
            && index + 1 < cs.len()
            && ascii_digit(cs[index + 1]).is_some()
        {
            index += 1;
            continue;
        }
        break;
    }
    if digits.is_empty() || digits == "." {
        return None;
    }
    let mut surface_start = start;
    if start > 0 && matches!(cs[start - 1], '¥' | '￥' | '$') {
        surface_start = start - 1;
    }
    let (unit, end) = match match_unit(cs, index) {
        Some((unit, end)) => (unit, end),
        None => (String::new(), index),
    };
    let numeric = canonical_numeric(&digits);
    if unit.is_empty() {
        if numeric.chars().filter(|ch| ch.is_ascii_digit()).count() < 2 {
            return None;
        }
        if numeric.len() == 4 {
            if let Ok(year) = numeric.parse::<u32>() {
                if (1900..=2100).contains(&year) {
                    return None;
                }
            }
        }
        if end < cs.len() && matches!(cs[end], '年' | '月' | '日' | '时' | '分' | '秒') {
            return None;
        }
    }
    let surface: String = cs[surface_start..end].iter().collect();
    let value = format!("{numeric}{unit}");
    Some(NumberToken {
        start,
        end,
        surface,
        numeric,
        unit,
        value,
        metric: metric_from_sentence(cs, start),
        time_scope: scope_from_sentence(cs, start),
        stat_scope: stat_scope_from_sentence(cs, start),
    })
}

fn number_tokens(cs: &[char]) -> Vec<NumberToken> {
    let dates = date_tokens(cs);
    let mask = date_mask(cs, &dates);
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < cs.len() {
        if !mask[index] && ascii_digit(cs[index]).is_some() {
            if let Some(token) = parse_number_at(cs, index) {
                index = token.end;
                tokens.push(token);
                continue;
            }
        }
        index += 1;
    }
    tokens
}

fn sentence_bounds(cs: &[char], focus: usize) -> (usize, usize) {
    let boundary = |ch: char| matches!(ch, '。' | '！' | '？' | '；' | ';' | '\n' | '\r');
    let mut start = focus.min(cs.len());
    while start > 0 && !boundary(cs[start - 1]) {
        start -= 1;
    }
    let mut end = focus.min(cs.len());
    while end < cs.len() && !boundary(cs[end]) {
        end += 1;
    }
    (start, end)
}

fn window_text(cs: &[char], start: usize, end: usize, focus: usize, max: usize) -> String {
    let total = end.saturating_sub(start);
    if total == 0 {
        return String::new();
    }
    if total <= max {
        return cs[start..end].iter().collect::<String>().trim().to_string();
    }
    let half = max / 2;
    let mut window_start = focus.saturating_sub(half);
    window_start = window_start.max(start);
    let window_end = (window_start + max).min(end);
    if window_end == end {
        window_start = end.saturating_sub(max).max(start);
    }
    let mut out = String::new();
    if window_start > start {
        out.push('…');
    }
    out.extend(cs[window_start..window_end].iter());
    if window_end < end {
        out.push('…');
    }
    out.trim().to_string()
}

fn fact_text_from(cs: &[char], focus: usize, focus_end: usize) -> String {
    let (start, end) = sentence_bounds(cs, focus);
    let center = focus.saturating_add(focus_end) / 2;
    window_text(cs, start, end, center, MAX_FACT_TEXT_CHARS)
}

fn metric_part(cs: &[char], start: usize) -> String {
    let (sentence_start, _) = sentence_bounds(cs, start);
    let before: String = cs[sentence_start..start].iter().collect();
    let clause = before
        .rsplit(|ch| matches!(ch, '，' | ',' | '、' | '：' | ':' | '（' | '('))
        .next()
        .unwrap_or(&before)
        .trim()
        .to_string();
    strip_metric_prefixes(&clause)
}

fn metric_from_sentence(cs: &[char], start: usize) -> Option<String> {
    let metric = canonical_text(&metric_part(cs, start));
    if metric.chars().count() < 2 || metric.chars().all(|ch| ch.is_ascii_digit()) {
        None
    } else {
        Some(metric)
    }
}

const METRIC_PREFIXES: &[&str] = &[
    "规模以上", "其中", "全年", "上半年", "下半年", "一季度", "二季度", "三季度",
    "四季度", "本月", "上月", "本周", "上周", "今年", "去年", "前年", "累计", "共计",
    "合计", "完成", "实现", "达到", "同比", "环比", "我市", "全市", "全县", "全区",
    "全省", "全镇", "全乡", "当前", "目前", "共", "的",
];

fn strip_leading_date(input: &str) -> Option<String> {
    let cs: Vec<char> = input.chars().collect();
    if let Some(token) = parse_date_at(&cs, 0) {
        return Some(cs[token.end..].iter().collect());
    }
    let mut index = 0usize;
    let mut digits = 0usize;
    while index < cs.len() && digits < 2 {
        let Some(digit) = ascii_digit(cs[index]) else {
            break;
        };
        let _ = digit;
        digits += 1;
        index += 1;
    }
    if digits == 0 || index >= cs.len() || !matches!(cs[index], '-' | '－' | '—') {
        return None;
    }
    index += 1;
    let mut digits_after = 0usize;
    while index < cs.len() && digits_after < 2 {
        let Some(digit) = ascii_digit(cs[index]) else {
            break;
        };
        let _ = digit;
        digits_after += 1;
        index += 1;
    }
    if digits_after == 0 || index >= cs.len() || cs[index] != '月' {
        return None;
    }
    Some(cs[index + 1..].iter().collect())
}

fn strip_metric_prefixes(input: &str) -> String {
    let mut rest = input.trim().to_string();
    loop {
        let mut changed = false;
        if let Some(after) = strip_leading_date(&rest) {
            rest = after.trim().to_string();
            changed = true;
        }
        for prefix in METRIC_PREFIXES {
            if let Some(after) = rest.strip_prefix(prefix) {
                rest = after.trim().to_string();
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    rest.trim_start_matches("为").trim().to_string()
}

const TIME_SCOPES: &[&str] = &[
    "一季度", "二季度", "三季度", "四季度", "上半年", "下半年", "全年", "本月",
    "上月", "本周", "上周", "今年", "去年", "前年", "1-6月", "1-9月", "1-12月",
];

const STAT_SCOPES: &[&str] = &[
    "全省", "全市", "全县", "全区", "全镇", "全乡", "市直", "县直", "区直",
    "规模以上", "规上", "抽样", "全面统计", "部门", "单位", "系统",
];

fn scope_from_sentence(cs: &[char], focus: usize) -> Option<String> {
    let (start, end) = sentence_bounds(cs, focus);
    for token in date_tokens(&cs[start..end]) {
        return Some(token.canonical);
    }
    let sentence: String = cs[start..end].iter().collect();
    for scope in TIME_SCOPES {
        if canonical_text(&sentence).contains(&canonical_text(scope)) {
            return Some((*scope).to_string());
        }
    }
    None
}

fn stat_scope_from_sentence(cs: &[char], focus: usize) -> Option<String> {
    let (start, end) = sentence_bounds(cs, focus);
    let sentence = canonical_text(&cs[start..end].iter().collect::<String>());
    for scope in STAT_SCOPES {
        if sentence.contains(&canonical_text(scope)) {
            return Some((*scope).to_string());
        }
    }
    None
}

const INSTITUTION_SUFFIXES: &[&str] = &["办公室", "中心", "办", "局", "厅", "处", "科"];

fn is_institution_stop(ch: char) -> bool {
    matches!(
        ch,
        '局' | '厅' | '处' | '科' | '办' | '室' | '心' | '长'
            | '的' | '了' | '在' | '由' | '与' | '和' | '及' | '等' | '这' | '该'
            | '各' | '本' | '我' | '你' | '他' | '她' | '它' | '是' | '为' | '有'
            | '将' | '已' | '并' | '把' | '被' | '从' | '向' | '下' | '其'
    )
}

fn institutions_in_text(cs: &[char]) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < cs.len() {
        let matched = INSTITUTION_SUFFIXES
            .iter()
            .find(|suffix| starts_with_chars(cs, index, suffix));
        let Some(suffix) = matched else {
            index += 1;
            continue;
        };
        let end = index + suffix.chars().count();
        let mut walk = index;
        let mut prefix = 0usize;
        while walk > 0 && prefix < 12 {
            let ch = cs[walk - 1];
            if is_cjk(ch) && !is_institution_stop(ch) {
                walk -= 1;
                prefix += 1;
            } else {
                break;
            }
        }
        if prefix >= 2 {
            let surface: String = cs[walk..end].iter().collect();
            out.push((walk, end, surface));
        }
        index = end;
    }
    out
}

const ROLE_TITLES: &[&str] = &[
    "秘书长", "总经理", "董事长", "负责人", "主任", "书记", "局长", "厅长", "处长",
    "科长", "部长", "市长", "县长", "镇长", "省长", "主席", "院长", "校长", "站长",
    "所长", "社长", "经理", "组长", "队长", "总监", "主管", "专员",
];

const COMPOUND_SURNAMES: &[&str] = &[
    "欧阳", "司马", "上官", "诸葛", "东方", "皇甫", "尉迟", "公孙", "慕容", "长孙",
    "宇文", "司徒", "司空", "令狐", "夏侯",
];

const SURNAMES: &str = "赵钱孙李周吴郑王冯陈褚卫蒋沈韩杨朱秦尤许何吕施张孔曹严华金魏陶姜戚谢邹喻柏水窦章云苏潘葛奚范彭郎鲁韦昌马苗凤花方俞任袁柳鲍史唐费廉岑薛雷贺倪汤滕殷罗毕郝邬安常乐于时傅皮卞齐康伍余元卜顾孟平黄和穆萧尹姚邵湛汪祁毛禹狄米贝明臧计伏成戴谈宋茅庞熊纪舒屈项祝董梁杜阮蓝闵席季麻强贾路娄危江童颜郭梅盛林刁钟徐邱骆高夏蔡田樊胡凌霍虞万支柯昝管卢莫经房裘缪干解应宗丁宣邓郁单杭洪包诸左石崔吉龚程邢裴陆荣翁荀羊於惠甄曲家封芮羿储靳汲邴糜松井段富巫乌焦巴弓牧隗山谷车侯宓蓬全郗班仰秋仲伊宫宁仇栾甘厉戎祖武符刘景詹束龙叶幸司韶郜黎蓟薄印宿白怀蒲邰从鄂索咸籍赖卓蔺屠蒙池乔阴胥能苍双闻莘党翟谭贡劳逄姬申扶堵冉宰郦雍却璩桑桂濮牛寿通边扈燕冀郏浦尚农温别庄晏柴瞿阎充慕连茹习宦艾鱼容向古易慎戈廖庾终暨居衡步都耿满弘匡国文寇广禄阙东殴殳沃利蔚越夔隆师巩厍聂晁勾敖融冷訾辛阚那简饶空曾毋沙乜养鞠须丰巢关蒯相查后荆红游竺权逯盖益桓公";

fn has_single_surname(ch: char) -> bool {
    SURNAMES.contains(ch)
}

fn compound_surname_at(cs: &[char], start: usize) -> Option<usize> {
    COMPOUND_SURNAMES
        .iter()
        .find(|surname| starts_with_chars(cs, start, surname))
        .map(|surname| surname.chars().count())
}

const NAME_STOPS: &str = "的了是在由与和及等这该各本我你他她它是为有将被从向下对将其并而但或也都就同又再还只很更最非常能会可要需应该";

const FOLLOW_VERBS: &[&str] = &[
    "主持", "指出", "表示", "强调", "介绍", "陪同", "出席", "参加", "讲话", "发言",
    "汇报", "部署", "要求", "一行", "同志", "介绍", "安排", "召集", "带队", "调研",
];

fn name_boundary_at(cs: &[char], index: usize) -> bool {
    if index >= cs.len() {
        return true;
    }
    let ch = cs[index];
    if !is_cjk(ch) {
        return true;
    }
    if NAME_STOPS.contains(ch) {
        return true;
    }
    if matches!(ch, '局' | '厅' | '处' | '科' | '长') {
        return true;
    }
    FOLLOW_VERBS.iter().any(|verb| starts_with_chars(cs, index, verb))
}

fn parse_name_at(cs: &[char], start: usize) -> Option<(usize, String)> {
    if start >= cs.len() || !is_cjk(cs[start]) {
        return None;
    }
    let surname_len = compound_surname_at(cs, start).unwrap_or_else(|| {
        if has_single_surname(cs[start]) { 1 } else { 0 }
    });
    if surname_len == 0 {
        return None;
    }
    for given in [2usize, 1usize] {
        let end = start + surname_len + given;
        if end > cs.len() {
            continue;
        }
        let valid = cs[start + surname_len..end]
            .iter()
            .all(|ch| is_cjk(*ch) && !NAME_STOPS.contains(*ch));
        if valid && name_boundary_at(cs, end) {
            return Some((end, cs[start..end].iter().collect()));
        }
    }
    None
}

fn persons_in_text(cs: &[char]) -> Vec<PersonToken> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < cs.len() {
        let matched = ROLE_TITLES
            .iter()
            .find(|role| starts_with_chars(cs, index, role));
        let Some(role) = matched else {
            index += 1;
            continue;
        };
        let role_end = index + role.chars().count();
        let name_start = if role_end < cs.len() && cs[role_end] == '兼' {
            role_end + 1
        } else {
            role_end
        };
        if let Some((end, name)) = parse_name_at(cs, name_start) {
            out.push(PersonToken {
                start: index,
                end,
                surface: cs[index..end].iter().collect(),
                role: (*role).to_string(),
                name,
            });
            index = end;
        } else {
            index = role_end;
        }
    }
    out
}

fn push_candidate(out: &mut Vec<FactCandidate>, candidate: FactCandidate) {
    let duplicate = out.iter().any(|existing| {
        existing.section_no == candidate.section_no
            && existing.paragraph_no == candidate.paragraph_no
            && existing.check_object == candidate.check_object
            && existing.value == candidate.value
            && existing.fact_text == candidate.fact_text
    });
    if !duplicate {
        out.push(candidate);
    }
}

fn extract_paragraph_candidates(paragraph: &FullTextParagraph) -> Vec<FactCandidate> {
    let cs: Vec<char> = paragraph.text.chars().collect();
    let dates = date_tokens(&cs);
    let numbers = number_tokens(&cs);
    let institutions = institutions_in_text(&cs);
    let persons = persons_in_text(&cs);
    let mut out = Vec::new();
    for date in &dates {
        push_candidate(
            &mut out,
            FactCandidate {
                section_no: paragraph.section_no,
                paragraph_no: paragraph.paragraph_no,
                check_object: CheckObject::Time,
                fact_text: fact_text_from(&cs, date.start, date.end),
                surface: date.surface.clone(),
                value: date.canonical.clone(),
                metric: metric_from_sentence(&cs, date.start),
                role: None,
                time_scope: Some(date.canonical.clone()),
                stat_scope: stat_scope_from_sentence(&cs, date.start),
            },
        );
    }
    for number in &numbers {
        push_candidate(
            &mut out,
            FactCandidate {
                section_no: paragraph.section_no,
                paragraph_no: paragraph.paragraph_no,
                check_object: CheckObject::Number,
                fact_text: fact_text_from(&cs, number.start, number.end),
                surface: number.surface.clone(),
                value: number.value.clone(),
                metric: number.metric.clone(),
                role: None,
                time_scope: number.time_scope.clone(),
                stat_scope: number.stat_scope.clone(),
            },
        );
    }
    for (start, end, surface) in &institutions {
        push_candidate(
            &mut out,
            FactCandidate {
                section_no: paragraph.section_no,
                paragraph_no: paragraph.paragraph_no,
                check_object: CheckObject::Institution,
                fact_text: fact_text_from(&cs, *start, *end),
                surface: surface.clone(),
                value: canonical_text(surface),
                metric: None,
                role: None,
                time_scope: scope_from_sentence(&cs, *start),
                stat_scope: stat_scope_from_sentence(&cs, *start),
            },
        );
    }
    for person in &persons {
        push_candidate(
            &mut out,
            FactCandidate {
                section_no: paragraph.section_no,
                paragraph_no: paragraph.paragraph_no,
                check_object: CheckObject::PersonRole,
                fact_text: fact_text_from(&cs, person.start, person.end),
                surface: person.surface.clone(),
                value: canonical_text(&person.name),
                metric: None,
                role: Some(person.role.clone()),
                time_scope: scope_from_sentence(&cs, person.start),
                stat_scope: stat_scope_from_sentence(&cs, person.start),
            },
        );
    }
    out
}

fn parse_section_number(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("## ")?;
    let (number, _) = rest.split_once(". ")?;
    number.trim().parse().ok()
}

fn flush_paragraph(
    out: &mut Vec<FullTextParagraph>,
    section_no: u32,
    paragraph_no: &mut u32,
    lines: &mut Vec<String>,
) {
    let text = lines.join("\n").trim().to_string();
    lines.clear();
    if text.is_empty() || text == SECTION_PLACEHOLDER {
        return;
    }
    *paragraph_no += 1;
    out.push(FullTextParagraph {
        section_no,
        paragraph_no: *paragraph_no,
        text,
    });
}

/// 按 607 compose_full_text 的格式拆章节与段落；未确认章节占位跳过。
pub fn split_full_text(full_text: &str) -> Vec<FullTextParagraph> {
    let mut out = Vec::new();
    let mut section_no = 1u32;
    let mut paragraph_no = 0u32;
    let mut lines: Vec<String> = Vec::new();
    for raw in full_text.lines() {
        let line = raw.trim();
        if let Some(number) = parse_section_number(line) {
            flush_paragraph(&mut out, section_no, &mut paragraph_no, &mut lines);
            section_no = number;
            paragraph_no = 0;
            continue;
        }
        if line.starts_with("# ") {
            flush_paragraph(&mut out, section_no, &mut paragraph_no, &mut lines);
            continue;
        }
        if line.is_empty() {
            flush_paragraph(&mut out, section_no, &mut paragraph_no, &mut lines);
            continue;
        }
        lines.push(line.to_string());
    }
    flush_paragraph(&mut out, section_no, &mut paragraph_no, &mut lines);
    out
}

/// 提取正文候选；纯函数，不依赖素材。
pub fn extract_candidates(full_text: &str) -> Vec<FactCandidate> {
    split_full_text(full_text)
        .iter()
        .flat_map(extract_paragraph_candidates)
        .collect()
}

fn annotation_class(kind: &str) -> Result<AnnotationClass, String> {
    match kind.trim() {
        "事实" => Ok(AnnotationClass::Fact),
        "分析" => Ok(AnnotationClass::Analysis),
        "判断" => Ok(AnnotationClass::Judgment),
        "建议" | "计划" | "工作方案" => Ok(AnnotationClass::Plan),
        "概括" => Ok(AnnotationClass::Summary),
        other => Err(format!(
            "四分法标注类型「{other}」不在核验允许范围（{}）。",
            ANNOTATION_KINDS.join(" / ")
        )),
    }
}

fn conclusion_for_class(class: AnnotationClass) -> (CheckConclusion, CheckObject) {
    match class {
        AnnotationClass::Fact => (CheckConclusion::SourceMissing, CheckObject::CaseFact),
        AnnotationClass::Analysis | AnnotationClass::Judgment => {
            (CheckConclusion::AnalysisJudgment, CheckObject::CaseFact)
        }
        AnnotationClass::Plan => (CheckConclusion::WorkPlan, CheckObject::WorkItem),
        AnnotationClass::Summary => {
            (CheckConclusion::ExpressiveSummary, CheckObject::WorkResult)
        }
    }
}

fn paragraph_contains(paragraph: &FullTextParagraph, text: &str) -> bool {
    let paragraph_key = canonical_text(&paragraph.text);
    let text_key = canonical_text(text);
    if text_key.is_empty() {
        return false;
    }
    paragraph_key.contains(&text_key) || text_key.contains(&paragraph_key)
}

fn resolve_annotations(
    paragraphs: &[FullTextParagraph],
    inputs: &[FactAnnotationInput],
) -> Result<(Vec<ResolvedAnnotation>, usize), String> {
    if inputs.len() > MAX_ANNOTATIONS {
        return Err(format!(
            "四分法标注最多 {MAX_ANNOTATIONS} 条，当前 {} 条。",
            inputs.len()
        ));
    }
    let mut resolved = Vec::new();
    let mut skipped = 0usize;
    for (index, input) in inputs.iter().enumerate() {
        if input.section_no == 0 {
            return Err(format!("annotations[{index}].section_no 必须从 1 开始。"));
        }
        let text = input.text.trim();
        if text.is_empty() {
            return Err(format!("annotations[{index}].text 不能为空。"));
        }
        if text.chars().count() > MAX_ANNOTATION_TEXT_CHARS {
            return Err(format!(
                "annotations[{index}].text 最多 {MAX_ANNOTATION_TEXT_CHARS} 字符。"
            ));
        }
        let class = annotation_class(&input.kind)
            .map_err(|message| format!("annotations[{index}].kind：{message}"))?;
        if let Some(paragraph_no) = input.paragraph_no {
            if paragraph_no == 0 {
                return Err(format!(
                    "annotations[{index}].paragraph_no 必须从 1 开始。"
                ));
            }
        }
        let location = paragraphs.iter().find(|paragraph| {
            paragraph.section_no == input.section_no
                && input
                    .paragraph_no
                    .map(|number| paragraph.paragraph_no == number)
                    .unwrap_or(true)
                && paragraph_contains(paragraph, text)
        });
        let Some(paragraph) = location else {
            skipped += 1;
            continue;
        };
        resolved.push(ResolvedAnnotation {
            section_no: paragraph.section_no,
            paragraph_no: paragraph.paragraph_no,
            class,
            text: text.to_string(),
        });
    }
    Ok((resolved, skipped))
}

fn overlaps(candidate: &FactCandidate, annotation: &ResolvedAnnotation) -> bool {
    let annotation_key = canonical_text(&annotation.text);
    let sentence_key = canonical_text(&candidate.fact_text);
    let surface_key = canonical_text(&candidate.surface);
    !annotation_key.is_empty()
        && (annotation_key.contains(&sentence_key)
            || sentence_key.contains(&annotation_key)
            || annotation_key.contains(&surface_key))
}

fn date_compatible(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    if left.len() == 7 && right.starts_with(&format!("{left}-")) {
        return true;
    }
    if right.len() == 7 && left.starts_with(&format!("{right}-")) {
        return true;
    }
    false
}

fn metric_compatible(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        _ => true,
    }
}

fn evidence_supports(candidate: &FactCandidate, quote: &str) -> bool {
    let quote_key = canonical_text(quote);
    match candidate.check_object {
        CheckObject::Time => date_tokens(&quote.chars().collect::<Vec<_>>())
            .iter()
            .any(|token| date_compatible(&candidate.value, &token.canonical)),
        CheckObject::Number => {
            let cs: Vec<char> = quote.chars().collect();
            if number_tokens(&cs).iter().any(|token| {
                token.matches(candidate)
                    && metric_compatible(candidate.metric.as_deref(), token.metric.as_deref())
            }) {
                return true;
            }
            candidate.metric.is_none()
                && quote_key.contains(&canonical_text(&candidate.surface))
        }
        CheckObject::Institution => quote_key.contains(&canonical_text(&candidate.surface)),
        CheckObject::PersonRole => {
            quote_key.contains(&canonical_text(&candidate.surface))
                || quote_key.contains(&candidate.value)
        }
        _ => quote_key.contains(&canonical_text(&candidate.surface)),
    }
}

fn evidence_conflict(candidate: &FactCandidate, quote: &str) -> Option<String> {
    let cs: Vec<char> = quote.chars().collect();
    match candidate.check_object {
        CheckObject::Number => {
            let metric = candidate.metric.as_deref()?;
            for token in number_tokens(&cs) {
                if token.metric.as_deref() == Some(metric) && !token.matches(candidate) {
                    return Some(token.surface);
                }
            }
            None
        }
        CheckObject::PersonRole => {
            let role = candidate.role.as_deref()?;
            for person in persons_in_text(&cs) {
                if person.role == role && canonical_text(&person.name) != candidate.value {
                    return Some(person.surface);
                }
            }
            None
        }
        _ => None,
    }
}

fn window_chars(text: &str, focus: usize, max: usize) -> String {
    let cs: Vec<char> = text.chars().collect();
    if cs.len() <= max {
        return text.trim().to_string();
    }
    let half = max / 2;
    let mut start = focus.saturating_sub(half).min(cs.len().saturating_sub(max));
    if start + max > cs.len() {
        start = cs.len().saturating_sub(max);
    }
    let end = start + max;
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(cs[start..end].iter());
    if end < cs.len() {
        out.push('…');
    }
    out
}

fn quote_snippet(quote: &str, candidate: &FactCandidate) -> String {
    let cleaned = quote.trim();
    if cleaned.chars().count() <= MAX_EVIDENCE_QUOTE_CHARS {
        return cleaned.to_string();
    }
    for needle in [candidate.surface.as_str(), candidate.value.as_str()] {
        if let Some(position) = cleaned.find(needle) {
            let focus = cleaned[..position].chars().count();
            return window_chars(cleaned, focus, MAX_EVIDENCE_QUOTE_CHARS);
        }
    }
    window_chars(cleaned, 0, MAX_EVIDENCE_QUOTE_CHARS)
}

fn basis_of(evidence: &FactEvidence, candidate: &FactCandidate) -> FactBasis {
    FactBasis {
        source_kind: evidence.source_kind,
        source_id: evidence.source_id.clone(),
        quote: quote_snippet(&evidence.quote, candidate),
    }
}

fn dedupe_basis(items: Vec<FactBasis>) -> Vec<FactBasis> {
    let mut out: Vec<FactBasis> = Vec::new();
    for item in items {
        if !out.iter().any(|existing| {
            existing.source_kind == item.source_kind
                && existing.source_id == item.source_id
                && existing.quote == item.quote
        }) {
            out.push(item);
        }
    }
    out
}

fn check_candidate(candidate: &FactCandidate, evidence: &[FactEvidence]) -> (CheckConclusion, Vec<FactBasis>) {
    let mut material_hits = Vec::new();
    let mut knowledge_hits = Vec::new();
    let mut outdated_hits = Vec::new();
    let mut conflicts = Vec::new();
    for item in evidence {
        if evidence_conflict(candidate, &item.quote).is_some() {
            conflicts.push(basis_of(item, candidate));
        }
        if !evidence_supports(candidate, &item.quote) {
            continue;
        }
        match item.source_kind {
            CitationSourceKind::Material => material_hits.push(basis_of(item, candidate)),
            CitationSourceKind::Knowledge if item.outdated => {
                outdated_hits.push(basis_of(item, candidate))
            }
            CitationSourceKind::Knowledge => knowledge_hits.push(basis_of(item, candidate)),
        }
    }
    if !conflicts.is_empty() {
        let mut basis = conflicts;
        basis.extend(material_hits);
        basis.extend(knowledge_hits);
        basis.extend(outdated_hits);
        return (CheckConclusion::SourceConflict, dedupe_basis(basis));
    }
    if !material_hits.is_empty() {
        return (CheckConclusion::DirectSource, dedupe_basis(material_hits));
    }
    if !outdated_hits.is_empty() {
        return (CheckConclusion::SourceOutdated, dedupe_basis(outdated_hits));
    }
    if !knowledge_hits.is_empty() {
        return (CheckConclusion::IndirectSource, dedupe_basis(knowledge_hits));
    }
    (CheckConclusion::SourceMissing, Vec::new())
}

fn record_id_for(
    section_no: u32,
    paragraph_no: u32,
    object: CheckObject,
    value: &str,
    fact_text: &str,
    conclusion: CheckConclusion,
) -> String {
    let key = format!(
        "{section_no}{paragraph_no}{}{value}{fact_text}{}",
        object.as_str(),
        conclusion.as_str()
    );
    let hash = sha256_hex(key.as_bytes());
    format!("fact-{}", hash.chars().take(16).collect::<String>())
}

fn push_finding(out: &mut Vec<FactFinding>, finding: FactFinding) {
    if !out.iter().any(|existing| existing.record_id == finding.record_id) {
        out.push(finding);
    }
}

fn deterministic_finding(candidate: &FactCandidate, evidence: &[FactEvidence]) -> FactFinding {
    let (conclusion, basis) = check_candidate(candidate, evidence);
    FactFinding {
        record_id: record_id_for(
            candidate.section_no,
            candidate.paragraph_no,
            candidate.check_object,
            &candidate.value,
            &candidate.fact_text,
            conclusion,
        ),
        section_no: candidate.section_no,
        paragraph_no: candidate.paragraph_no,
        check_object: candidate.check_object,
        conclusion,
        basis,
        fact_text: candidate.fact_text.clone(),
    }
}

const SUMMARY_MARKERS: &[&str] = &[
    "总体来看", "总的来看", "总的来讲", "总体而言", "总体来说", "整体来看",
    "整体而言", "综上所述", "概括起来", "总的看", "总体上",
];

fn looks_like_summary(text: &str) -> bool {
    let canonical = canonical_text(text);
    SUMMARY_MARKERS
        .iter()
        .any(|marker| canonical.contains(&canonical_text(marker)))
}

fn conflict_side(candidate: &FactCandidate) -> ConflictSide {
    ConflictSide {
        section_no: candidate.section_no,
        paragraph_no: candidate.paragraph_no,
        fact_text: candidate.fact_text.clone(),
        value: candidate.value.clone(),
        time_scope: candidate.time_scope.clone(),
        stat_scope: candidate.stat_scope.clone(),
    }
}

fn detect_scope_conflicts(candidates: &[FactCandidate]) -> Vec<ScopeConflict> {
    let mut conflicts = Vec::new();
    'outer: for left in 0..candidates.len() {
        for right in left + 1..candidates.len() {
            let a = &candidates[left];
            let b = &candidates[right];
            if a.check_object != CheckObject::Number || b.check_object != CheckObject::Number {
                continue;
            }
            let (Some(left_metric), Some(right_metric)) = (a.metric.as_deref(), b.metric.as_deref())
            else {
                continue;
            };
            if left_metric != right_metric {
                continue;
            }
            let value_changed = numeric_part(&a.value) != numeric_part(&b.value)
                || unit_part(&a.value) != unit_part(&b.value);
            let time_changed = a.time_scope != b.time_scope;
            let stat_changed = a.stat_scope != b.stat_scope;
            if !value_changed && !time_changed && !stat_changed {
                continue;
            }
            let reason = if value_changed {
                "同一指标不同数值"
            } else if time_changed {
                "同一指标不同时间范围"
            } else {
                "同一指标不同统计口径"
            };
            conflicts.push(ScopeConflict {
                metric: left_metric.to_string(),
                reason: reason.to_string(),
                left: conflict_side(a),
                right: conflict_side(b),
            });
            if conflicts.len() >= MAX_SCOPE_CONFLICTS {
                break 'outer;
            }
        }
    }
    conflicts.sort_by(|a, b| {
        a.metric
            .cmp(&b.metric)
            .then_with(|| a.left.section_no.cmp(&b.left.section_no))
            .then_with(|| a.left.paragraph_no.cmp(&b.left.paragraph_no))
            .then_with(|| a.right.section_no.cmp(&b.right.section_no))
            .then_with(|| a.right.paragraph_no.cmp(&b.right.paragraph_no))
    });
    conflicts
}

fn finalize_stats(
    candidates: usize,
    findings: &[FactFinding],
    conflicts: &[ScopeConflict],
    annotations_used: usize,
    annotations_skipped: usize,
) -> FactCheckStats {
    let mut stats = FactCheckStats {
        candidates,
        findings: findings.len(),
        scope_conflicts: conflicts.len(),
        annotations_used,
        annotations_skipped,
        ..Default::default()
    };
    for finding in findings {
        match finding.conclusion {
            CheckConclusion::DirectSource => stats.direct_source += 1,
            CheckConclusion::IndirectSource => stats.indirect_source += 1,
            CheckConclusion::SourceConflict => stats.source_conflict += 1,
            CheckConclusion::SourceOutdated => stats.source_outdated += 1,
            CheckConclusion::SourceMissing => stats.source_missing += 1,
            CheckConclusion::AnalysisJudgment => stats.analysis_judgment += 1,
            CheckConclusion::WorkPlan => stats.work_plan += 1,
            CheckConclusion::ExpressiveSummary => stats.expressive_summary += 1,
        }
    }
    stats
}

/// 确定性核验入口：正文 + 依据 + 生成侧四分法标注 → 结论、冲突与统计。
pub fn run_fact_check(
    full_text: &str,
    evidence: &[FactEvidence],
    annotations: &[FactAnnotationInput],
) -> Result<FactCheckOutcome, String> {
    if full_text.chars().count() > MAX_FULL_TEXT_CHARS {
        return Err(format!(
            "正文过长（最多 {MAX_FULL_TEXT_CHARS} 字符），请分章核验。"
        ));
    }
    let paragraphs = split_full_text(full_text);
    let (resolved_annotations, annotations_skipped) =
        resolve_annotations(&paragraphs, annotations)?;
    let mut candidates = Vec::new();
    for paragraph in &paragraphs {
        candidates.extend(extract_paragraph_candidates(paragraph));
    }
    let mut findings = Vec::new();
    for candidate in &candidates {
        let annotation = resolved_annotations.iter().find(|annotation| {
            annotation.class != AnnotationClass::Fact
                && annotation.section_no == candidate.section_no
                && annotation.paragraph_no == candidate.paragraph_no
                && overlaps(candidate, annotation)
        });
        if let Some(annotation) = annotation {
            let (conclusion, _) = conclusion_for_class(annotation.class);
            push_finding(
                &mut findings,
                FactFinding {
                    record_id: record_id_for(
                        candidate.section_no,
                        candidate.paragraph_no,
                        candidate.check_object,
                        &candidate.value,
                        &candidate.fact_text,
                        conclusion,
                    ),
                    section_no: candidate.section_no,
                    paragraph_no: candidate.paragraph_no,
                    check_object: candidate.check_object,
                    conclusion,
                    basis: Vec::new(),
                    fact_text: candidate.fact_text.clone(),
                },
            );
        } else {
            push_finding(&mut findings, deterministic_finding(candidate, evidence));
        }
    }
    for annotation in &resolved_annotations {
        if annotation.class == AnnotationClass::Fact {
            continue;
        }
        if candidates.iter().any(|candidate| {
            candidate.section_no == annotation.section_no
                && candidate.paragraph_no == annotation.paragraph_no
                && overlaps(candidate, annotation)
        }) {
            continue;
        }
        let (conclusion, object) = conclusion_for_class(annotation.class);
        push_finding(
            &mut findings,
            FactFinding {
                record_id: record_id_for(
                    annotation.section_no,
                    annotation.paragraph_no,
                    object,
                    annotation.class.as_str(),
                    &annotation.text,
                    conclusion,
                ),
                section_no: annotation.section_no,
                paragraph_no: annotation.paragraph_no,
                check_object: object,
                conclusion,
                basis: Vec::new(),
                fact_text: annotation.text.chars().take(MAX_FACT_TEXT_CHARS).collect(),
            },
        );
    }
    for paragraph in &paragraphs {
        let has_candidate = candidates.iter().any(|candidate| {
            candidate.section_no == paragraph.section_no
                && candidate.paragraph_no == paragraph.paragraph_no
        });
        let has_annotation = resolved_annotations.iter().any(|annotation| {
            annotation.section_no == paragraph.section_no
                && annotation.paragraph_no == paragraph.paragraph_no
        });
        if has_candidate || has_annotation || !looks_like_summary(&paragraph.text) {
            continue;
        }
        push_finding(
            &mut findings,
            FactFinding {
                record_id: record_id_for(
                    paragraph.section_no,
                    paragraph.paragraph_no,
                    CheckObject::WorkResult,
                    "概括",
                    &paragraph.text,
                    CheckConclusion::ExpressiveSummary,
                ),
                section_no: paragraph.section_no,
                paragraph_no: paragraph.paragraph_no,
                check_object: CheckObject::WorkResult,
                conclusion: CheckConclusion::ExpressiveSummary,
                basis: Vec::new(),
                fact_text: paragraph
                    .text
                    .chars()
                    .take(MAX_FACT_TEXT_CHARS)
                    .collect(),
            },
        );
    }
    findings.sort_by(|a, b| {
        (a.section_no, a.paragraph_no)
            .cmp(&(b.section_no, b.paragraph_no))
            .then_with(|| a.check_object.as_str().cmp(b.check_object.as_str()))
            .then_with(|| a.fact_text.cmp(&b.fact_text))
            .then_with(|| a.record_id.cmp(&b.record_id))
    });
    let conflicts = detect_scope_conflicts(&candidates);
    let stats = finalize_stats(
        candidates.len(),
        &findings,
        &conflicts,
        resolved_annotations.len(),
        annotations_skipped,
    );
    Ok(FactCheckOutcome {
        findings,
        conflicts,
        stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writing_records::{CheckConclusion, CheckObject, CitationSourceKind};

    fn material(quote: &str) -> FactEvidence {
        FactEvidence {
            source_kind: CitationSourceKind::Material,
            source_id: "mat-synthetic-609".to_string(),
            quote: quote.to_string(),
            outdated: false,
        }
    }

    fn knowledge(quote: &str, outdated: bool) -> FactEvidence {
        FactEvidence {
            source_kind: CitationSourceKind::Knowledge,
            source_id: "notes/synthetic-609.md".to_string(),
            quote: quote.to_string(),
            outdated,
        }
    }

    fn annotation(section_no: u32, kind: &str, text: &str) -> FactAnnotationInput {
        FactAnnotationInput {
            section_no,
            paragraph_no: None,
            kind: kind.to_string(),
            text: text.to_string(),
        }
    }

    fn run(text: &str, evidence: &[FactEvidence]) -> FactCheckOutcome {
        run_fact_check(text, evidence, &[]).unwrap()
    }

    #[test]
    fn extracts_date_formats_and_canonicalizes() {
        let text = "# 虚构稿\n\n## 1. 概况\n\n2026年3月完成试点，2026-04-05验收。";
        let candidates = extract_candidates(text);
        let dates: Vec<&FactCandidate> = candidates
            .iter()
            .filter(|candidate| candidate.check_object == CheckObject::Time)
            .collect();
        assert_eq!(dates.len(), 2);
        assert!(dates.iter().any(|date| date.value == "2026-03"));
        assert!(dates.iter().any(|date| date.value == "2026-04-05"));
    }

    #[test]
    fn extracts_number_percent_amount_units() {
        let text = "完成投资1200万元，同比增长12%，覆盖3个点位。";
        let candidates = extract_candidates(text);
        let values: Vec<&str> = candidates
            .iter()
            .filter(|candidate| candidate.check_object == CheckObject::Number)
            .map(|candidate| candidate.value.as_str())
            .collect();
        assert!(values.contains(&"1200万元"), "{values:?}");
        assert!(values.contains(&"12%"), "{values:?}");
        assert!(values.contains(&"3个"), "{values:?}");
    }

    #[test]
    fn bare_year_is_time_not_number() {
        let text = "2026年完成虚构工作。";
        let candidates = extract_candidates(text);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.check_object == CheckObject::Time && candidate.value == "2026"));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.check_object == CheckObject::Number));
    }

    #[test]
    fn extracts_institution_by_suffix() {
        let text = "虚构市财政局与虚构区政务服务中心联合开展。";
        let candidates = extract_candidates(text);
        let values: Vec<&str> = candidates
            .iter()
            .filter(|candidate| candidate.check_object == CheckObject::Institution)
            .map(|candidate| candidate.value.as_str())
            .collect();
        assert!(values.contains(&"虚构市财政局"), "{values:?}");
        assert!(values.contains(&"虚构区政务服务中心"), "{values:?}");
    }

    #[test]
    fn extracts_person_role_and_name() {
        let text = "局长张甲主持会议，处长李乙参加会议。";
        let candidates = extract_candidates(text);
        let surfaces: Vec<&str> = candidates
            .iter()
            .filter(|candidate| candidate.check_object == CheckObject::PersonRole)
            .map(|candidate| candidate.surface.as_str())
            .collect();
        assert!(surfaces.contains(&"局长张甲"), "{surfaces:?}");
        assert!(surfaces.contains(&"处长李乙"), "{surfaces:?}");
    }

    #[test]
    fn paragraph_locations_and_placeholder_skipped() {
        let text = "# 虚构稿\n\n## 1. 概况\n\n第一段。\n\n第二段。\n\n## 2. 详情\n\n（本章待写作）\n\n## 3. 收尾\n\n第三段。";
        let paragraphs = split_full_text(text);
        assert_eq!(paragraphs.len(), 3);
        assert_eq!(
            paragraphs
                .iter()
                .map(|paragraph| (paragraph.section_no, paragraph.paragraph_no))
                .collect::<Vec<_>>(),
            vec![(1, 1), (1, 2), (3, 1)]
        );
    }

    #[test]
    fn scenario_six_correct_data_gets_direct_source() {
        let text = "## 1. 概况\n\n2026年3月完成投资1200万元。";
        let evidence = vec![material("2026年3月完成投资1200万元，同比增长10%。")];
        let outcome = run(text, &evidence);
        let date = outcome
            .findings
            .iter()
            .find(|finding| finding.check_object == CheckObject::Time)
            .unwrap();
        let number = outcome
            .findings
            .iter()
            .find(|finding| finding.check_object == CheckObject::Number)
            .unwrap();
        assert_eq!(date.conclusion, CheckConclusion::DirectSource);
        assert_eq!(number.conclusion, CheckConclusion::DirectSource);
        assert!(!number.basis.is_empty());
        assert_eq!(outcome.stats.direct_source, 2);
    }

    #[test]
    fn scenario_six_wrong_value_gets_conflict() {
        let text = "## 1. 概况\n\n完成投资1500万元。";
        let evidence = vec![material("2026年3月完成投资1200万元。")];
        let outcome = run(text, &evidence);
        let number = outcome
            .findings
            .iter()
            .find(|finding| finding.check_object == CheckObject::Number)
            .unwrap();
        assert_eq!(number.conclusion, CheckConclusion::SourceConflict);
        assert!(!number.basis.is_empty(), "冲突结论必须带冲突依据");
    }

    #[test]
    fn scenario_six_unsourced_statement_is_missing_not_verified() {
        let text = "## 1. 概况\n\n完成培训480人次。";
        let evidence = vec![material("完成投资1200万元。")];
        let outcome = run(text, &evidence);
        let number = outcome
            .findings
            .iter()
            .find(|finding| finding.check_object == CheckObject::Number)
            .unwrap();
        assert_eq!(number.conclusion, CheckConclusion::SourceMissing);
        assert!(number.basis.is_empty());
        assert!(!outcome
            .findings
            .iter()
            .any(|finding| finding.conclusion == CheckConclusion::DirectSource));
    }

    #[test]
    fn knowledge_quote_is_indirect_source() {
        let text = "## 1. 概况\n\n2026年5月完成投资900万元。";
        let evidence = vec![knowledge("2026年5月完成投资900万元。", false)];
        let outcome = run(text, &evidence);
        let number = outcome
            .findings
            .iter()
            .find(|finding| finding.check_object == CheckObject::Number)
            .unwrap();
        assert_eq!(number.conclusion, CheckConclusion::IndirectSource);
    }

    #[test]
    fn same_number_under_other_metric_is_not_direct() {
        let text = "## 1. 概况\n\n完成投资1200万元。";
        let evidence = vec![material("完成培训1200万元。")];
        let outcome = run(text, &evidence);
        let number = outcome
            .findings
            .iter()
            .find(|finding| finding.check_object == CheckObject::Number)
            .unwrap();
        assert_eq!(number.conclusion, CheckConclusion::SourceMissing);
    }

    #[test]
    fn outdated_knowledge_is_marked() {
        let text = "## 1. 概况\n\n2026年4月完成投资800万元。";
        let evidence = vec![knowledge("2026年4月完成投资800万元。", true)];
        let outcome = run(text, &evidence);
        let number = outcome
            .findings
            .iter()
            .find(|finding| finding.check_object == CheckObject::Number)
            .unwrap();
        assert_eq!(number.conclusion, CheckConclusion::SourceOutdated);
    }

    #[test]
    fn analysis_annotation_does_not_masquerade_as_fact() {
        let text = "## 1. 概况\n\n预计明年完成投资2000万元。";
        let evidence = vec![material("完成投资2000万元。")];
        let annotations = vec![annotation(1, "判断", "预计明年完成投资2000万元。")];
        let outcome = run_fact_check(text, &evidence, &annotations).unwrap();
        let number = outcome
            .findings
            .iter()
            .find(|finding| finding.check_object == CheckObject::Number)
            .unwrap();
        assert_eq!(number.conclusion, CheckConclusion::AnalysisJudgment);
        assert!(number.basis.is_empty());
        assert!(!outcome
            .findings
            .iter()
            .any(|finding| finding.conclusion == CheckConclusion::DirectSource));
    }

    #[test]
    fn plan_annotation_is_work_plan() {
        let text = "## 1. 概况\n\n建议下一步加强培训。";
        let annotations = vec![annotation(1, "建议", "建议下一步加强培训。")];
        let outcome = run_fact_check(text, &[], &annotations).unwrap();
        assert!(outcome
            .findings
            .iter()
            .any(|finding| finding.conclusion == CheckConclusion::WorkPlan));
    }

    #[test]
    fn summary_without_facts_is_expressive_summary() {
        let text = "## 1. 概况\n\n总体来看，各项工作取得明显成效。";
        let outcome = run(text, &[]);
        assert_eq!(outcome.stats.expressive_summary, 1);
        assert_eq!(
            outcome.findings[0].check_object,
            CheckObject::WorkResult
        );
    }

    #[test]
    fn scenario_seven_scope_conflict_hints_without_merge() {
        let text = "## 1. 概况\n\n一季度完成投资1200万元。\n\n二季度完成投资1500万元。";
        let evidence = vec![material("一季度完成投资1200万元，二季度完成投资1500万元。")];
        let outcome = run(text, &evidence);
        assert_eq!(outcome.conflicts.len(), 1);
        assert_eq!(outcome.conflicts[0].reason, "同一指标不同数值");
        assert_eq!(outcome.conflicts[0].metric, "投资");
        assert_eq!(
            outcome
                .findings
                .iter()
                .filter(|finding| finding.conclusion == CheckConclusion::SourceConflict)
                .count(),
            2
        );
        assert!(!outcome
            .findings
            .iter()
            .any(|finding| finding.fact_text.contains("2700")));
    }

    #[test]
    fn scenario_seven_same_value_different_time_range_conflicts() {
        let text = "## 1. 概况\n\n一季度完成投资1200万元。\n\n上半年完成投资1200万元。";
        let outcome = run(text, &[]);
        assert_eq!(outcome.conflicts.len(), 1);
        assert_eq!(outcome.conflicts[0].reason, "同一指标不同时间范围");
    }

    #[test]
    fn scenario_seven_same_value_different_stat_scope_conflicts() {
        let text = "## 1. 概况\n\n全市完成投资1200万元。\n\n全县完成投资1200万元。";
        let outcome = run(text, &[]);
        assert_eq!(outcome.conflicts.len(), 1);
        assert_eq!(outcome.conflicts[0].reason, "同一指标不同统计口径");
    }

    #[test]
    fn record_ids_stable_and_locations_present() {
        let text = "## 1. 概况\n\n2026年3月完成投资1200万元。";
        let evidence = vec![material("2026年3月完成投资1200万元。")];
        let first = run(text, &evidence);
        let second = run(text, &evidence);
        assert_eq!(
            first
                .findings
                .iter()
                .map(|finding| finding.record_id.clone())
                .collect::<Vec<_>>(),
            second
                .findings
                .iter()
                .map(|finding| finding.record_id.clone())
                .collect::<Vec<_>>()
        );
        for finding in &first.findings {
            assert!(finding.section_no >= 1);
            assert!(finding.paragraph_no >= 1);
            assert!(!finding.fact_text.trim().is_empty());
        }
    }

    #[test]
    fn annotation_kind_whitelist_rejects_unknown() {
        let text = "## 1. 概况\n\n虚构段落。";
        let annotations = vec![annotation(1, "评价", "虚构段落。")];
        let error = run_fact_check(text, &[], &annotations).unwrap_err();
        assert!(error.contains("四分法标注类型"), "{error}");
    }

    #[test]
    fn stats_counts_each_conclusion() {
        let text = "## 1. 概况\n\n2026年3月完成投资1200万元。\n\n完成培训480人次。\n\n总体来看，各项工作取得明显成效。";
        let evidence = vec![material("2026年3月完成投资1200万元。")];
        let outcome = run(text, &evidence);
        assert_eq!(outcome.stats.direct_source, 2);
        assert_eq!(outcome.stats.source_missing, 1);
        assert_eq!(outcome.stats.expressive_summary, 1);
        assert_eq!(
            outcome.stats.findings,
            outcome.stats.direct_source
                + outcome.stats.indirect_source
                + outcome.stats.source_conflict
                + outcome.stats.source_outdated
                + outcome.stats.source_missing
                + outcome.stats.analysis_judgment
                + outcome.stats.work_plan
                + outcome.stats.expressive_summary
        );
    }
}



