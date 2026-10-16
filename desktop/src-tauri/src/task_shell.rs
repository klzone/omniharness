//! OmniDesk 共用任务外壳 v1（omnidesk-task-shell/1）。
//!
//! 设计来源：docs/OmniDesk-Task-Shell.md、docs/OmniDesk-Authorization-and-DataFlow-Gap-Audit.md。
//!
//! 本模块只服务**一个**已实现的纵向试点（desensitized_analysis，脱敏数据分析与生态研判），
//! 并把共用骨架固定下来：input → deterministic → explanation → confirmation → persist。
//!
//! 硬规矩（由代码强制，不是提示词约定）：
//!   1. 统计值只由本模块的确定性代码算出，模型不得重算或编造；
//!   2. 产物只写仓库外的本地数据目录，且不覆盖已有文件；
//!   3. 使用记录结构里没有自由文本字段，不记录输入输出；
//!   4. reserved 任务类型一律拒绝执行；
//!   5. 所有处理输入的入口（分析、解释提示词、保存）先过同一道启发式闸门；
//!   6. 保存必须带人工确认布尔值，且拒绝可伪造的 model 成功来源。
//!
//! 本模块**不是**通用 Agent 平台、工作流编排器、工作流编辑器或任务注册中心。
//!
//! 证据等级：本文件为 static（源码事实）；文件内单元测试为 script（脚本/Rust 测试运行）。

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::app_paths::resolve_real_path;
use crate::sha256::sha256_hex;

/// 任务外壳版本号。写进每个产物，便于将来识别格式。
pub const SHELL_VERSION: &str = "omnidesk-task-shell/1";
/// 确定性统计结果的 schema 标识。
pub const ANALYSIS_SCHEMA: &str = "omnidesk.analysis.v1";
/// 使用记录的 schema 标识。
pub const USAGE_SCHEMA: &str = "omnidesk.usage.v1";

/// 产物中的代码块围栏。用 Unicode 转义写出，避免源码里直接出现三连反引号。
const FENCE_OPEN_TEXT: &str = "\u{60}\u{60}\u{60}text";
const FENCE_CLOSE: &str = "\u{60}\u{60}\u{60}";

// ---------------------------------------------------------------------------
// 任务类型
// ---------------------------------------------------------------------------

/// 任务类型登记项。
///
/// status 只有两个取值：implemented（已实现）与 reserved（仅保留边界，未实现）。
/// reserved **不代表**获得真实材料授权，也不代表能力承诺。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskTypeInfo {
    pub task_type: &'static str,
    pub title: &'static str,
    pub status: &'static str,
    pub data_scope: &'static str,
    pub rule_asset: Option<&'static str>,
    pub note: &'static str,
}

/// 全部已登记的任务类型（本阶段仅 1 个 implemented）。
pub fn task_types() -> Vec<TaskTypeInfo> {
    vec![
        TaskTypeInfo {
            task_type: "desensitized_analysis",
            title: "脱敏数据分析与生态研判",
            status: "implemented",
            data_scope: "仅合成 / 公开 / 经明确批准的数据",
            rule_asset: Some("templates/analysis/explanation-rules.md"),
            note: "本阶段唯一已实现的纵向试点。统计值由本地确定性代码计算，模型只做解释。",
        },
        TaskTypeInfo {
            task_type: "weekly_report",
            title: "周报",
            status: "reserved",
            data_scope: "—",
            rule_asset: None,
            note: "仅保留任务类型边界：未实现，也未获得真实材料准入。",
        },
        TaskTypeInfo {
            task_type: "material_writing",
            title: "通用材料撰写",
            status: "reserved",
            data_scope: "—",
            rule_asset: None,
            note: "仅保留任务类型边界：未实现，也未获得真实材料准入。DSH 内的材料写作 Skill 不接入本外壳。",
        },
        TaskTypeInfo {
            task_type: "project_management",
            title: "项目管理",
            status: "reserved",
            data_scope: "—",
            rule_asset: None,
            note: "仅保留任务类型边界：未实现。",
        },
    ]
}

/// 是否为已实现的任务类型。reserved 一律不算实现。
pub fn is_implemented(task_type: &str) -> bool {
    task_types()
        .iter()
        .any(|t| t.task_type == task_type && t.status == "implemented")
}

/// 全部已登记的任务类型标识（含 reserved），用于使用记录的枚举校验。
fn all_task_type_ids() -> Vec<&'static str> {
    task_types().iter().map(|t| t.task_type).collect()
}

// ---------------------------------------------------------------------------
// 合成样例与解释规则（编译期嵌入，避免「看到路径就等于读到文件」的假象）
// ---------------------------------------------------------------------------

/// 仓库内合成样例。全部为虚构数据，不含任何真实人员、机构或案件信息。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SampleInfo {
    pub sample_id: &'static str,
    pub title: &'static str,
    pub note: &'static str,
    pub content: &'static str,
}

pub fn samples() -> Vec<SampleInfo> {
    vec![
        SampleInfo {
            sample_id: "ecology-basic",
            title: "合成样例 · 基础分类计数",
            note: "五个类别、量级接近，用于验证基本统计与描述性解释。纯虚构数据。",
            content: include_str!("../../../samples/analysis/synthetic-ecology-basic.csv"),
        },
        SampleInfo {
            sample_id: "ecology-outlier",
            title: "合成样例 · 含离群值",
            note: "其中一个类别数值明显偏离，用于验证离群值是否被单独说明。纯虚构数据。",
            content: include_str!("../../../samples/analysis/synthetic-ecology-outlier.csv"),
        },
        SampleInfo {
            sample_id: "ecology-flat",
            title: "合成样例 · 完全均衡",
            note: "所有类别数值相同，用于验证数据不足以支持研判时是否如实说明。纯虚构数据。",
            content: include_str!("../../../samples/analysis/synthetic-ecology-flat.csv"),
        },
    ]
}

/// 解释阶段规则（唯一权威源：templates/analysis/explanation-rules.md）。
pub fn explanation_rules() -> &'static str {
    include_str!("../../../templates/analysis/explanation-rules.md")
}

// ---------------------------------------------------------------------------
// 红线扫描（启发式：仅降低误贴概率，不是合规控制）
// ---------------------------------------------------------------------------

/// 关键词清单。仅用于提示，命中重叠时计数可能重复累加。
const RED_LINE_KEYWORDS: &[&str] = &[
    "案件",
    "案件线索",
    "线索",
    "立案",
    "初核",
    "审查调查",
    "举报",
    "信访",
    "涉案",
    "廉政档案",
    "秘密",
    "机密",
    "内部资料",
    "未公开",
    "身份证",
    "电话号码",
];

/// 一次红线命中。**只回显类别与计数，不回显命中原文**。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RedLineHit {
    pub kind: &'static str,
    pub count: u64,
    pub label: &'static str,
}

/// 统计数字串：返回 (疑似身份证号数量, 疑似手机号数量)。
///
/// 判定标准（刻意保守，宁可漏检也不回显原文）：
///   * 18 位连续数字 → 疑似身份证号；
///   * 17 位连续数字后紧跟 X/x → 疑似身份证号；
///   * 11 位连续数字、1 开头、第二位 3-9 → 疑似手机号。
fn scan_digit_runs(text: &str) -> (u64, u64) {
    let chars: Vec<char> = text.chars().collect();
    let mut id_cards = 0u64;
    let mut phones = 0u64;
    let mut i = 0usize;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let run_len = i - start;
        if run_len == 18 {
            id_cards += 1;
            continue;
        }
        if run_len == 17 && i < chars.len() && (chars[i] == 'X' || chars[i] == 'x') {
            id_cards += 1;
            i += 1;
            continue;
        }
        if run_len == 11 && chars[start] == '1' {
            let second = chars[start + 1];
            if ('3'..='9').contains(&second) {
                phones += 1;
            }
        }
    }
    (id_cards, phones)
}

/// 对文本做启发式红线扫描。返回值不含命中原文。
pub fn red_line_scan(text: &str) -> Vec<RedLineHit> {
    let mut hits = Vec::new();
    let (id_cards, phones) = scan_digit_runs(text);
    if id_cards > 0 {
        hits.push(RedLineHit {
            kind: "id_card",
            count: id_cards,
            label: "疑似身份证号（18 位或 17 位+X 数字串）",
        });
    }
    if phones > 0 {
        hits.push(RedLineHit {
            kind: "phone_number",
            count: phones,
            label: "疑似手机号（11 位、1 开头、第二位 3-9）",
        });
    }
    let mut keyword_total = 0u64;
    for kw in RED_LINE_KEYWORDS {
        keyword_total += text.matches(kw).count() as u64;
    }
    if keyword_total > 0 {
        hits.push(RedLineHit {
            kind: "case_keyword",
            count: keyword_total,
            label: "疑似敏感关键词（命中清单见源码；不回显命中原文）",
        });
    }
    hits
}

/// 处理前的输入闸门。命中即拒绝处理，且不把命中原文写进错误信息。
pub fn assert_input_allowed(text: &str) -> Result<(), String> {
    let hits = red_line_scan(text);
    if hits.is_empty() {
        return Ok(());
    }
    let mut detail = String::new();
    for hit in &hits {
        detail.push_str(&format!("{}（{} 处）；", hit.label, hit.count));
    }
    Err(format!(
        "输入命中红线扫描，已拒绝处理：{}。该扫描仅为启发式，不代表合规判断，也不代表获得授权；此类内容请改用单位批准的合规系统处理。",
        detail
    ))
}

// ---------------------------------------------------------------------------
// 本地数据根（产物与使用记录的落盘边界）
// ---------------------------------------------------------------------------

/// 默认本地数据目录（相对 $HOME）。刻意放在 YvanAI 数据区，与 DSH 运行环境分离。
const DEFAULT_LOCAL_SUBDIR: &str = "YvanAI/projects/Omnidesk/analysis";
/// 覆盖本地数据根的官方入口（绝对路径）。
const LOCAL_ROOT_ENV: &str = "OMNIDESK_LOCAL_DATA_ROOT";

/// 禁止作为本地数据根或落盘路径的片段（小写匹配）。
const FORBIDDEN_PATH_FRAGMENTS: &[&str] = &[
    "/.dsh",
    "/.agents",
    "/.codex",
    "/.config/dsh desktop",
    "/.local/share/io.github.hairyf.deepseek-harness-desktop",
    "deepseek-harness",
    "/myworkstation",
    "omniharness",
];

fn normalize_path(path: &Path) -> PathBuf {
    match fs::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(_) => {
            // 词法归一：解析 . 与 ..，不触碰文件系统。
            let mut out = PathBuf::new();
            for component in path.components() {
                match component {
                    Component::ParentDir => {
                        out.pop();
                    }
                    Component::CurDir => {}
                    other => out.push(other.as_os_str()),
                }
            }
            out
        }
    }
}

/// 仓库根探测：优先环境变量，其次由 CARGO_MANIFEST_DIR（desktop/src-tauri）上溯两级。
fn repo_root_hint() -> Result<PathBuf, String> {
    if let Some(raw) = std::env::var_os("OMNIDESK_REPO_ROOT") {
        if !raw.is_empty() {
            return Ok(PathBuf::from(raw));
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法从 CARGO_MANIFEST_DIR 推断仓库根".to_string())
}

/// 校验一个路径可以作为本地数据根：必须是绝对路径，且不含被禁止片段。
///
/// 同时校验**文本路径**与**解析后的真实落点**（逐级解析软链接）：
/// 任一命中禁止片段即拒绝；无法安全解析真实落点时默认拒绝。
pub fn assert_local_root_allowed(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("本地数据路径必须是绝对路径：{}", path.display()));
    }
    assert_no_forbidden_path_fragment(path)?;
    let resolved = resolve_real_path(path)?;
    assert_no_forbidden_path_fragment(&resolved)?;
    Ok(())
}

/// 校验落盘路径不在仓库内。
///
/// 文本路径与解析后的真实落点都要在仓库之外，避免符号链接把落点引回仓库。
pub fn assert_outside_repo(path: &Path) -> Result<(), String> {
    let repo = repo_root_hint()?;
    let repo_norm = normalize_path(&repo);
    let path_norm = normalize_path(path);
    if path_norm.starts_with(&repo_norm) || path.starts_with(&repo) {
        return Err(format!(
            "拒绝把产物写入仓库内：{}（仓库根 {}）",
            path.display(),
            repo.display()
        ));
    }
    let resolved = resolve_real_path(path)?;
    let resolved_repo = resolve_real_path(&repo).unwrap_or(repo_norm.clone());
    if resolved.starts_with(&resolved_repo) {
        return Err(format!(
            "拒绝把产物写入仓库内：{} 的真实落点 {} 位于仓库 {} 之内",
            path.display(),
            resolved.display(),
            repo.display()
        ));
    }
    Ok(())
}

/// 拒绝路径中包含被禁止片段（小写、反斜杠统一为斜杠后匹配）。
fn assert_no_forbidden_path_fragment(path: &Path) -> Result<(), String> {
    let normalized = path.to_string_lossy().to_lowercase().replace('\\', "/");
    for fragment in FORBIDDEN_PATH_FRAGMENTS {
        if normalized.contains(fragment) {
            return Err(format!(
                "拒绝把本地数据写入 {}：路径包含被禁止片段 {:?}（原版 DSH / agent / 仓库内）",
                path.display(),
                fragment
            ));
        }
    }
    Ok(())
}

/// 解析并校验本地数据根。
pub fn resolve_local_data_root() -> Result<PathBuf, String> {
    let root = match std::env::var_os(LOCAL_ROOT_ENV) {
        Some(raw) if !raw.is_empty() => PathBuf::from(raw),
        _ => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| "环境变量 HOME 缺失，无法确定默认本地数据目录".to_string())?;
            PathBuf::from(home).join(DEFAULT_LOCAL_SUBDIR)
        }
    };
    assert_local_root_allowed(&root)?;
    assert_outside_repo(&root)?;
    Ok(root)
}

// ---------------------------------------------------------------------------
// 确定性统计（本外壳唯一的数值来源）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CategoryStat {
    pub category: String,
    pub count: u64,
    pub sum: f64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AnalysisResult {
    pub schema: &'static str,
    pub rows: u64,
    pub distinct_categories: usize,
    pub total: f64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub top_category: Option<String>,
    pub by_category: Vec<CategoryStat>,
    pub fingerprint: String,
}

/// 数字格式化：整数不带小数点，其余保留 6 位并去掉尾零。用于稳定指纹与展示。
fn fmt_num(value: f64) -> String {
    if !value.is_finite() {
        return "NaN".to_string();
    }
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let text = format!("{:.6}", value);
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

/// 按给定顺序求和（保持代码路径可预测，不使用迭代器折叠的隐式顺序）。
fn sum_in_order(values: &[f64]) -> f64 {
    let mut acc = 0.0f64;
    for value in values {
        acc += *value;
    }
    acc
}

/// 由规范化字符串计算统计指纹。
///
/// 指纹只依赖「类别名升序 + 各类统计量 + 总体统计量」，因此与数据行顺序无关。
fn fingerprint_of(
    rows: u64,
    total: f64,
    mean: f64,
    min: f64,
    max: f64,
    by_category: &[CategoryStat],
) -> String {
    let mut canonical = String::new();
    canonical.push_str(ANALYSIS_SCHEMA);
    canonical.push('\n');
    canonical.push_str(&format!("rows={}\n", rows));
    canonical.push_str(&format!("total={}\n", fmt_num(total)));
    canonical.push_str(&format!("mean={}\n", fmt_num(mean)));
    canonical.push_str(&format!("min={}\n", fmt_num(min)));
    canonical.push_str(&format!("max={}\n", fmt_num(max)));
    for stat in by_category {
        canonical.push_str(&format!(
            "{}|{}|{}|{}|{}|{}\n",
            stat.category,
            stat.count,
            fmt_num(stat.sum),
            fmt_num(stat.mean),
            fmt_num(stat.min),
            fmt_num(stat.max)
        ));
    }
    sha256_hex(canonical.as_bytes())
}

/// 对 CSV 文本做确定性统计。
///
/// 表头按列名定位「类别」「数值」两列，因此列顺序可以不同。
/// 空行与以井号开头的注释行会被忽略。错误信息带行号。
pub fn deterministic_analysis(csv_text: &str) -> Result<AnalysisResult, String> {
    let mut category_idx: Option<usize> = None;
    let mut value_idx: Option<usize> = None;
    let mut header_seen = false;
    let mut groups: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut all_values: Vec<f64> = Vec::new();
    let mut rows: u64 = 0;

    for (index, raw_line) in csv_text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells: Vec<&str> = line
            .split([',', '\u{ff0c}'])
            .map(|cell| cell.trim())
            .collect();

        if !header_seen {
            header_seen = true;
            category_idx = cells.iter().position(|cell| cell.contains("类别"));
            value_idx = cells.iter().position(|cell| cell.contains("数值"));
            match (category_idx, value_idx) {
                (Some(ci), Some(vi)) if ci != vi => {}
                _ => {
                    return Err(format!(
                        "第 {} 行：表头必须包含互不相同的「类别」和「数值」两列",
                        line_no
                    ))
                }
            }
            continue;
        }

        let ci = match category_idx {
            Some(ci) => ci,
            None => return Err(format!("第 {} 行：表头缺少「类别」列", line_no)),
        };
        let vi = match value_idx {
            Some(vi) => vi,
            None => return Err(format!("第 {} 行：表头缺少「数值」列", line_no)),
        };
        let needed = if ci > vi { ci } else { vi };
        if cells.len() <= needed {
            return Err(format!("第 {} 行：列数不足，缺少「类别」或「数值」", line_no));
        }
        let category = cells[ci].to_string();
        if category.is_empty() {
            return Err(format!("第 {} 行：类别名为空", line_no));
        }
        let value = match cells[vi].parse::<f64>() {
            Ok(parsed) if parsed.is_finite() => parsed,
            _ => {
                return Err(format!(
                    "第 {} 行：数值列无法解析为有限数字（错误信息不回显原值）",
                    line_no
                ))
            }
        };
        groups.entry(category).or_default().push(value);
        all_values.push(value);
        rows += 1;
    }

    if !header_seen {
        return Err("输入为空：未找到表头行".to_string());
    }
    if rows == 0 {
        return Err("没有可用数据行（除表头外没有有效数据）".to_string());
    }

    let mut by_category: Vec<CategoryStat> = Vec::with_capacity(groups.len());
    for (category, values) in &groups {
        let count = values.len() as u64;
        let sum = sum_in_order(values);
        let group_min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let group_max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        by_category.push(CategoryStat {
            category: category.clone(),
            count,
            sum,
            mean: sum / count as f64,
            min: group_min,
            max: group_max,
        });
    }

    // 合计按类别升序累加：既保证与行顺序无关，也保证与指纹口径一致。
    let category_sums: Vec<f64> = by_category.iter().map(|stat| stat.sum).collect();
    let total = sum_in_order(&category_sums);
    let mean = total / rows as f64;
    let min = all_values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = all_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    // 合计最高的类别；并列时取类别名升序最小者（BTreeMap 已升序，严格大于才替换）。
    let mut top: Option<&CategoryStat> = None;
    for stat in &by_category {
        match top {
            Some(current) if stat.sum <= current.sum => {}
            _ => top = Some(stat),
        }
    }

    let fingerprint = fingerprint_of(rows, total, mean, min, max, &by_category);

    Ok(AnalysisResult {
        schema: ANALYSIS_SCHEMA,
        rows,
        distinct_categories: by_category.len(),
        total,
        mean,
        min,
        max,
        top_category: top.map(|stat| stat.category.clone()),
        by_category,
        fingerprint,
    })
}

// ---------------------------------------------------------------------------
// 冻结统计块与解释提示词
// ---------------------------------------------------------------------------

/// 生成冻结统计块。模型只能解释这块文本，不得重算。
pub fn frozen_stats_block(result: &AnalysisResult) -> String {
    let mut out = String::new();
    out.push_str("【冻结统计块 · 由本地确定性代码计算，禁止重算或修改】\n");
    out.push_str(&format!("schema: {}\n", result.schema));
    out.push_str(&format!("数据行数: {}\n", result.rows));
    out.push_str(&format!("类别数: {}\n", result.distinct_categories));
    out.push_str(&format!("合计: {}\n", fmt_num(result.total)));
    out.push_str(&format!("平均: {}\n", fmt_num(result.mean)));
    out.push_str(&format!("最小: {}\n", fmt_num(result.min)));
    out.push_str(&format!("最大: {}\n", fmt_num(result.max)));
    out.push_str(&format!(
        "合计最高的类别: {}\n",
        result.top_category.as_deref().unwrap_or("（无）")
    ));
    out.push_str("按类别（类别名升序）:\n");
    out.push_str("类别 | 条数 | 合计 | 平均 | 最小 | 最大\n");
    for stat in &result.by_category {
        out.push_str(&format!(
            "{} | {} | {} | {} | {} | {}\n",
            stat.category,
            stat.count,
            fmt_num(stat.sum),
            fmt_num(stat.mean),
            fmt_num(stat.min),
            fmt_num(stat.max)
        ));
    }
    let short = if result.fingerprint.len() >= 16 {
        &result.fingerprint[..16]
    } else {
        result.fingerprint.as_str()
    };
    out.push_str(&format!("统计指纹(SHA-256 前 16 位): {}\n", short));
    out
}

/// 构造解释阶段的提示词。
///
/// 会先在本地重新计算一遍并与传入指纹比对；不一致即拒绝，避免解释引用了别的统计块。
pub fn build_explanation_prompt(
    task_type: &str,
    input_text: &str,
    expected_fingerprint: &str,
) -> Result<String, String> {
    if !is_implemented(task_type) {
        return Err(format!(
            "任务类型 {:?} 尚未实现（reserved 类型不执行）",
            task_type
        ));
    }
    // 统一输入闸门：命中红线扫描就不产生提示词（与保存入口同一道闸门）。
    assert_input_allowed(input_text)?;
    let recomputed = deterministic_analysis(input_text)?;
    if recomputed.fingerprint != expected_fingerprint {
        return Err(
            "统计指纹不一致：输入内容与提交的指纹不匹配，请重新运行确定性统计。".to_string(),
        );
    }
    Ok(format!(
        "你在协助完成 OmniDesk 任务类型「脱敏数据分析与生态研判」。\n下面的统计块由本地确定性代码算出，是本次任务唯一允许引用的数值来源。\n只做解释，不要重算、不要补齐、不要引入任何外部数字或来源。\n\n{frozen}\n【解释规则】\n{rules}\n【输出要求】\n1) 事实：从统计块直接读出的数字与排序；\n2) 分析：能对应到具体数字的差异、集中度、离群值描述；\n3) 判断：显式标注为「判断」，并说明依据薄弱处；\n4) 数据不足时直接说明「现有数据不足以支持研判，未检索到足够依据」，不要用套话补满。\n",
        frozen = frozen_stats_block(&recomputed),
        rules = explanation_rules(),
    ))
}

/// 运行确定性统计前的统一输入闸门。
///
/// 分析入口与解释提示词入口共用这一道闸门：命中即不产生统计结果。
/// 闸门本身是**启发式**，不能替代授权、准入判断或人工审查。
pub fn assert_analysis_input_allowed(input_text: &str) -> Result<(), String> {
    assert_input_allowed(input_text)
}

/// 带输入闸门的确定性统计入口。
///
/// 这是分析阶段**唯一**应被宿主与测试调用的入口：先过闸门，命中即不返回统计结果。
pub fn analyze_input(input_text: &str) -> Result<AnalysisResult, String> {
    assert_analysis_input_allowed(input_text)?;
    deterministic_analysis(input_text)
}

// ---------------------------------------------------------------------------
// 产物保存（只写仓库外，不覆盖）
// ---------------------------------------------------------------------------

/// 允许的解释来源。
///
/// 只保留两个**调用方可如实自证**的来源：
///   * user —— 使用者手写；
///   * model_unverified —— 模型输出，但本阶段无法在代码层复核其成功终态。
///
/// 刻意**不含** model：此前 model 来源仅凭调用方传入的 task_state 字符串判断，
/// 该字符串可由 UI 或任何命令调用方伪造，因此视为不可信，予以移除。
/// 可信模型来源（能证明结果确来自已成功终态的引擎任务）作为后续独立设计，不在本阶段实现。
pub const ALLOWED_EXPLANATION_SOURCES: &[&str] = &["user", "model_unverified"];

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SaveArtifactRequest {
    pub task_type: String,
    pub input_text: String,
    pub explanation_text: String,
    pub explanation_source: String,
    /// 人工确认：必须由调用方显式给出 true 才允许保存。
    ///
    /// 该字段**没有** serde 默认值：请求里缺失该字段会让反序列化直接失败，
    /// 从而无法绕过 UI 用命令直接落盘未确认产物。确认备注仍是可选说明，不替代本字段。
    pub user_confirmed: bool,
    #[serde(default)]
    pub confirmation_note: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SaveArtifactReceipt {
    pub path: String,
    pub fingerprint: String,
    pub bytes: u64,
    pub written_at: f64,
}

/// 保存分析产物到仓库外的本地数据目录。
///
/// 保存前复核链条：任务类型已实现 → 来源合法 → model 来源要求成功终态 →
/// 输入通过红线扫描 → 指纹与重算结果一致 → 目标路径在仓库外 → 文件不存在。
pub fn save_artifact(
    request: &SaveArtifactRequest,
    now: f64,
) -> Result<SaveArtifactReceipt, String> {
    if !is_implemented(&request.task_type) {
        return Err(format!("任务类型 {:?} 尚未实现，拒绝保存", request.task_type));
    }
    // R1：人工确认是保存的硬前提。缺失（反序列化失败）或 false 都不允许落盘。
    if !request.user_confirmed {
        return Err(
            "必须显式人工确认（user_confirmed = true）后才能保存；确认备注不能替代该确认。"
                .to_string(),
        );
    }
    if !ALLOWED_EXPLANATION_SOURCES.contains(&request.explanation_source.as_str()) {
        return Err(format!(
            "解释来源 {:?} 不在允许列表内（只允许 user 或 model_unverified；可信模型来源尚未实现）",
            request.explanation_source
        ));
    }
    assert_input_allowed(&request.input_text)?;

    let recomputed = deterministic_analysis(&request.input_text)?;
    if recomputed.fingerprint != request.fingerprint {
        return Err(
            "统计指纹不一致：保存前复核失败，拒绝写入（避免产物与实际数据不符）。".to_string(),
        );
    }

    let root = resolve_local_data_root()?;
    fs::create_dir_all(&root).map_err(|e| format!("创建本地数据目录失败：{}", e))?;
    // 目录真正建立后再校验一次，防止符号链接等把落点引到禁区。
    assert_local_root_allowed(&root)?;
    assert_outside_repo(&root)?;

    let short = if request.fingerprint.len() >= 12 {
        &request.fingerprint[..12]
    } else {
        request.fingerprint.as_str()
    };
    let file_name = format!("{}-{}.md", utc_stamp(now), short);
    let target = root.join(&file_name);
    assert_local_root_allowed(&target)?;
    assert_outside_repo(&target)?;

    let content = render_artifact(request, &recomputed, now);

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| {
            format!(
                "无法创建产物文件 {}：{}（已存在或不可写；本阶段不覆盖已有文件）",
                target.display(),
                e
            )
        })?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入产物失败：{}", e))?;
    file.sync_all()
        .map_err(|e| format!("刷新产物到磁盘失败：{}", e))?;

    Ok(SaveArtifactReceipt {
        path: target.display().to_string(),
        fingerprint: request.fingerprint.clone(),
        bytes: content.len() as u64,
        written_at: now,
    })
}

/// 渲染产物 Markdown。刻意把来源、模板缺席与范例缺席都写清楚，方便日后复核。
fn render_artifact(request: &SaveArtifactRequest, result: &AnalysisResult, now: f64) -> String {
    let note = if request.confirmation_note.trim().is_empty() {
        "（无）".to_string()
    } else {
        request.confirmation_note.trim().to_string()
    };
    let mut out = String::new();
    out.push_str("# OmniDesk 分析产物\n\n");
    out.push_str(&format!("- 外壳版本: {}\n", SHELL_VERSION));
    out.push_str(&format!("- 任务类型: {}\n", request.task_type));
    out.push_str(&format!("- 生成时间 (UTC): {}\n", utc_iso(now)));
    out.push_str(&format!("- 统计指纹 (SHA-256): {}\n", request.fingerprint));
    out.push_str(&format!("- 统计 schema: {}\n", result.schema));
    out.push_str(&format!("- 解释来源: {}\n", request.explanation_source));
    out.push_str(&format!(
        "- 人工确认: {}\n",
        if request.user_confirmed { "已确认（user_confirmed = true）" } else { "未确认" }
    ));
    out.push_str("- 模型可信度: 本产物未绑定可核验的引擎成功终态；模型来源统一记为未复核\n");
    out.push_str("- 模板: 未使用模板（本任务不使用材料模板）\n");
    out.push_str(
        "- 历史范例: 未使用历史范例（本次未加载任何范例；knowledge/excellent-cases/ 当前无常驻范文）\n",
    );
    out.push_str(&format!("- 人工确认备注: {}\n", note));
    out.push_str(
        "- 数据边界: 仅合成 / 公开 / 经明确批准的数据；红线扫描为启发式，不构成合规判断\n",
    );
    out.push_str("\n## 确定性统计（本地代码计算，可复现）\n\n");
    out.push_str(FENCE_OPEN_TEXT);
    out.push('\n');
    out.push_str(&frozen_stats_block(result));
    out.push_str(FENCE_CLOSE);
    out.push_str("\n\n## 解释（");
    out.push_str(&request.explanation_source);
    out.push_str("）\n\n");
    out.push_str(request.explanation_text.trim());
    out.push_str("\n\n## 原始输入（原文保留，未修改）\n\n");
    out.push_str(FENCE_OPEN_TEXT);
    out.push('\n');
    out.push_str(&request.input_text);
    if !request.input_text.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(FENCE_CLOSE);
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// 轻量使用记录（不含输入输出；不含自由文本）
// ---------------------------------------------------------------------------

pub const ALLOWED_REWORK_KINDS: &[&str] = &[
    "none",
    "structure",
    "facts",
    "wording",
    "format",
    "requirements_misread",
];
pub const ALLOWED_BLOCKERS: &[&str] = &[
    "none",
    "engine_unavailable",
    "credentials_missing",
    "input_unclear",
    "template_missing",
    "other_technical",
];
pub const ALLOWED_DURATION_BUCKETS: &[&str] =
    &["under_5m", "5_15m", "15_30m", "30_60m", "over_60m"];

static USAGE_SEQ: AtomicU64 = AtomicU64::new(0);

/// 一条使用记录。字段全部是枚举、布尔、时间或标识，没有自由文本字段。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UsageRecord {
    pub record_id: String,
    pub schema: String,
    pub task_type: String,
    pub recorded_at: f64,
    pub completed: bool,
    pub usable: bool,
    pub rework_kind: String,
    pub blocker: String,
    pub duration_bucket: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UsageRecordInput {
    pub task_type: String,
    pub completed: bool,
    pub usable: bool,
    pub rework_kind: String,
    pub blocker: String,
    #[serde(default)]
    pub duration_bucket: Option<String>,
}

pub fn build_usage_record(input: &UsageRecordInput, now: f64) -> Result<UsageRecord, String> {
    if !all_task_type_ids().contains(&input.task_type.as_str()) {
        return Err(format!("未知任务类型：{:?}", input.task_type));
    }
    if !ALLOWED_REWORK_KINDS.contains(&input.rework_kind.as_str()) {
        return Err(format!("未知返工类别：{:?}", input.rework_kind));
    }
    if !ALLOWED_BLOCKERS.contains(&input.blocker.as_str()) {
        return Err(format!("未知障碍类别：{:?}", input.blocker));
    }
    if let Some(bucket) = input.duration_bucket.as_deref() {
        if !ALLOWED_DURATION_BUCKETS.contains(&bucket) {
            return Err(format!("未知耗时区间：{:?}", bucket));
        }
    }
    let seq = USAGE_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(UsageRecord {
        record_id: format!("{}-{:04}", utc_stamp(now), seq),
        schema: USAGE_SCHEMA.to_string(),
        task_type: input.task_type.clone(),
        recorded_at: now,
        completed: input.completed,
        usable: input.usable,
        rework_kind: input.rework_kind.clone(),
        blocker: input.blocker.clone(),
        duration_bucket: input.duration_bucket.clone(),
    })
}

/// 追加一条使用记录（JSONL）。只允许写在仓库外。
pub fn append_usage_record(path: &Path, record: &UsageRecord) -> Result<(), String> {
    assert_local_root_allowed(path)?;
    assert_outside_repo(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建使用记录目录失败：{}", e))?;
    }
    let line = serde_json::to_string(record).map_err(|e| format!("序列化使用记录失败：{}", e))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("打开使用记录文件失败：{}", e))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("写入使用记录失败：{}", e))?;
    file.write_all(b"\n")
        .map_err(|e| format!("写入使用记录失败：{}", e))?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageReadResult {
    pub records: Vec<UsageRecord>,
    pub unparsable_lines: u64,
    pub path: String,
}

/// 读取使用记录。无法解析的行只计数，不阻塞其余记录。
pub fn read_usage_records(path: &Path) -> Result<UsageReadResult, String> {
    assert_local_root_allowed(path)?;
    assert_outside_repo(path)?;
    let mut records = Vec::new();
    let mut unparsable_lines = 0u64;
    if path.exists() {
        let text = fs::read_to_string(path).map_err(|e| format!("读取使用记录失败：{}", e))?;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<UsageRecord>(trimmed) {
                Ok(record) => records.push(record),
                Err(_) => unparsable_lines += 1,
            }
        }
    }
    Ok(UsageReadResult {
        records,
        unparsable_lines,
        path: path.display().to_string(),
    })
}

/// 按 record_id 删除一条使用记录，返回删除条数。
pub fn delete_usage_record(path: &Path, record_id: &str) -> Result<u64, String> {
    assert_local_root_allowed(path)?;
    assert_outside_repo(path)?;
    if !path.exists() {
        return Ok(0);
    }
    let text = fs::read_to_string(path).map_err(|e| format!("读取使用记录失败：{}", e))?;
    let mut kept: Vec<String> = Vec::new();
    let mut deleted = 0u64;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<UsageRecord>(trimmed) {
            Ok(record) if record.record_id == record_id => deleted += 1,
            _ => kept.push(trimmed.to_string()),
        }
    }
    let mut body = String::new();
    for line in &kept {
        body.push_str(line);
        body.push('\n');
    }
    fs::write(path, body).map_err(|e| format!("写回使用记录失败：{}", e))?;
    Ok(deleted)
}

/// 清空全部使用记录。返回是否发生过清空（文件此前存在）。
pub fn clear_usage_records(path: &Path) -> Result<bool, String> {
    assert_local_root_allowed(path)?;
    assert_outside_repo(path)?;
    if !path.exists() {
        return Ok(false);
    }
    fs::write(path, b"").map_err(|e| format!("清空使用记录失败：{}", e))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// UTC 时间（不引入 chrono；用 Hinnant 算法从 Unix 秒换算日期）
// ---------------------------------------------------------------------------

/// 由 Unix 秒换算 (年, 月, 日, 时, 分, 秒)，全部为 UTC。
pub fn utc_parts(secs: f64) -> (i64, u32, u32, u32, u32, u32) {
    let total = secs.floor() as i64;
    let days = total.div_euclid(86_400);
    let rem = total.rem_euclid(86_400);
    let hour = (rem / 3_600) as u32;
    let minute = ((rem % 3_600) / 60) as u32;
    let second = (rem % 60) as u32;
    let (year, month, day) = civil_from_days(days);
    (year, month, day, hour, minute, second)
}

/// Howard Hinnant 的 civil_from_days：天数（1970-01-01 起）→ (年, 月, 日)。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { y + 1 } else { y }, month, day)
}

/// YYYYMMDD-HHMMSS（UTC），用于产物文件名与记录 ID。
pub fn utc_stamp(secs: f64) -> String {
    let (y, mo, d, h, mi, s) = utc_parts(secs);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y, mo, d, h, mi, s)
}

/// YYYY-MM-DD HH:MM:SS UTC，用于产物内页。
pub fn utc_iso(secs: f64) -> String {
    let (y, mo, d, h, mi, s) = utc_parts(secs);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        y, mo, d, h, mi, s
    )
}

/// 当前 Unix 秒（浮点）。
pub fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// 单元测试
//
// 证据等级：本模块为 script（Rust 测试运行），不是静态检查推断。
// 刻意串行化所有会改环境变量的用例，避免相互干扰。
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "omnidesk-task-shell-tests/{}-{}",
            tag,
            std::process::id()
        ))
    }

    /// 在临时本地数据根下执行用例，结束后清理并还原环境变量。
    fn with_env_root<T>(tag: &str, body: impl FnOnce(&Path) -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = temp_root(tag);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("创建临时数据根");
        std::env::set_var(LOCAL_ROOT_ENV, &root);
        let out = body(&root);
        std::env::remove_var(LOCAL_ROOT_ENV);
        let _ = fs::remove_dir_all(&root);
        out
    }

    fn repo_dir() -> PathBuf {
        repo_root_hint().expect("仓库根可推断")
    }

    fn valid_request(input: &str, source: &str) -> SaveArtifactRequest {
        let analysis = deterministic_analysis(input).expect("样例应可分析");
        SaveArtifactRequest {
            task_type: "desensitized_analysis".to_string(),
            input_text: input.to_string(),
            explanation_text: "这是合成测试用的解释文本。".to_string(),
            explanation_source: source.to_string(),
            user_confirmed: true,
            confirmation_note: "人工确认：合成测试".to_string(),
            fingerprint: analysis.fingerprint,
        }
    }

    // --- 确定性统计 ---

    #[test]
    fn analysis_computes_expected_values() {
        let result = deterministic_analysis("类别,数值\n甲,10\n甲,20\n乙,5\n").unwrap();
        assert_eq!(result.rows, 3);
        assert_eq!(result.distinct_categories, 2);
        assert_eq!(fmt_num(result.total), "35");
        assert_eq!(fmt_num(result.mean), "11.666667");
        assert_eq!(fmt_num(result.min), "5");
        assert_eq!(fmt_num(result.max), "20");
        assert_eq!(result.top_category.as_deref(), Some("甲"));
        assert_eq!(result.by_category.len(), 2);
        // by_category 按类别名的 Unicode 码位升序排列（乙 U+4E59 在 甲 U+7532 之前），
        // 因此这里按名字查找而不是按下标断言。
        let jia = result
            .by_category
            .iter()
            .find(|stat| stat.category == "甲")
            .expect("甲 应在结果中");
        assert_eq!(jia.count, 2);
        assert_eq!(fmt_num(jia.sum), "30");
        assert_eq!(fmt_num(jia.mean), "15");
        let names: Vec<&str> = result
            .by_category
            .iter()
            .map(|stat| stat.category.as_str())
            .collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "by_category 应按类别名升序");
    }

    #[test]
    fn analysis_is_reproducible() {
        let input = samples()[0].content;
        let first = deterministic_analysis(input).unwrap();
        let second = deterministic_analysis(input).unwrap();
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first, second);
    }

    #[test]
    fn analysis_fingerprint_changes_with_data() {
        let a = deterministic_analysis("类别,数值\n甲,10\n").unwrap();
        let b = deterministic_analysis("类别,数值\n甲,11\n").unwrap();
        assert_ne!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn analysis_fingerprint_independent_of_row_order() {
        let a = deterministic_analysis("类别,数值\n甲,10\n乙,5\n甲,20\n").unwrap();
        let b = deterministic_analysis("类别,数值\n甲,20\n甲,10\n乙,5\n").unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a, b);
    }

    #[test]
    fn analysis_ignores_comments_and_blank_lines() {
        let with_noise =
            "# 注释\n类别,数值\n\n甲,10\n# 中间注释\n乙,5\n\n";
        let clean = "类别,数值\n甲,10\n乙,5\n";
        assert_eq!(
            deterministic_analysis(with_noise).unwrap(),
            deterministic_analysis(clean).unwrap()
        );
    }

    #[test]
    fn analysis_rejects_bad_input_without_panicking() {
        let error = deterministic_analysis("类别,数值\n甲,abc\n").unwrap_err();
        assert!(error.contains("第 2 行"), "错误信息应带行号：{}", error);

        let header_error = deterministic_analysis("名称,数量\n甲,1\n").unwrap_err();
        assert!(header_error.contains("表头"));

        let empty_error = deterministic_analysis("").unwrap_err();
        assert!(empty_error.contains("未找到表头"));

        let no_rows_error = deterministic_analysis("类别,数值\n").unwrap_err();
        assert!(no_rows_error.contains("没有可用数据行"));
    }

    // --- 红线扫描 ---

    #[test]
    fn red_line_scan_flags_synthetic_patterns_without_echoing_content() {
        let text = "联系人：110101199003077777，电话：13800138000";
        let hits = red_line_scan(text);
        assert!(hits.iter().any(|h| h.kind == "id_card" && h.count == 1));
        assert!(hits.iter().any(|h| h.kind == "phone_number" && h.count == 1));
        for hit in &hits {
            assert!(!hit.label.contains("110101199003077777"));
            assert!(!hit.label.contains("13800138000"));
        }
        let error = assert_input_allowed(text).unwrap_err();
        assert!(!error.contains("110101199003077777"));
        assert!(!error.contains("13800138000"));
        assert!(error.contains("启发式"));
    }

    #[test]
    fn red_line_scan_is_quiet_on_clean_synthetic_data() {
        for sample in samples() {
            assert!(
                red_line_scan(sample.content).is_empty(),
                "样例 {} 不应命中红线扫描",
                sample.sample_id
            );
        }
    }

    // --- 本地数据根边界 ---

    #[test]
    fn local_root_rejects_repo_and_forbidden_paths() {
        assert!(assert_local_root_allowed(Path::new("/tmp/demo/.dsh/x")).is_err());
        assert!(assert_local_root_allowed(Path::new("/tmp/demo/.agents/x")).is_err());
        assert!(assert_local_root_allowed(Path::new("/tmp/demo/.codex/x")).is_err());
        assert!(assert_local_root_allowed(Path::new("/tmp/demo/deepseek-harness/x")).is_err());
        assert!(assert_local_root_allowed(Path::new("relative/dir")).is_err());
        assert!(assert_outside_repo(&repo_dir()).is_err());
        assert!(assert_outside_repo(&repo_dir().join("knowledge")).is_err());
    }

    #[test]
    fn local_root_accepts_temp_dir() {
        let root = temp_root("accepts");
        assert!(assert_local_root_allowed(&root).is_ok());
        assert!(assert_outside_repo(&root).is_ok());
    }

    // --- 产物保存 ---

    #[test]
    fn save_writes_outside_repo_and_refuses_overwrite() {
        with_env_root("save", |root| {
            let input = samples()[0].content;
            let request = valid_request(input, "user");
            let now = 1_760_572_800.0;
            let receipt = save_artifact(&request, now).expect("首次保存应成功");
            assert!(receipt.path.starts_with(&root.to_string_lossy().to_string()));
            assert!(receipt.bytes > 0);

            let written = fs::read_to_string(&receipt.path).unwrap();
            assert!(written.contains(SHELL_VERSION));
            assert!(written.contains(&request.fingerprint));
            assert!(written.contains("未使用历史范例"));
            assert!(written.contains("未使用模板"));
            assert!(written.contains(&frozen_stats_block(&deterministic_analysis(input).unwrap())));

            // 同一时刻 + 同一指纹 → 同一文件名 → 第二次必须拒绝，绝不覆盖。
            let again = save_artifact(&request, now).unwrap_err();
            assert!(again.contains("已存在") || again.contains("不覆盖"));
        });
    }

    #[test]
    fn save_rejects_fingerprint_mismatch() {
        with_env_root("mismatch", |_root| {
            let mut request = valid_request(samples()[0].content, "user");
            request.fingerprint = "0".repeat(64);
            let error = save_artifact(&request, 1_760_572_800.0).unwrap_err();
            assert!(error.contains("指纹不一致"));
        });
    }

    #[test]
    fn save_rejects_removed_model_source() {
        with_env_root("model-source", |_root| {
            // R2：model 来源已从允许列表移除，任何调用方都无法再伪造"已复核的模型成功结果"。
            let request = valid_request(samples()[0].content, "model");
            let error = save_artifact(&request, 1_760_572_800.0).unwrap_err();
            assert!(
                error.contains("不在允许列表内"),
                "model 来源应被拒绝：{}",
                error
            );

            // 如实标为 model_unverified 才允许保存。
            let ok = valid_request(samples()[0].content, "model_unverified");
            assert!(save_artifact(&ok, 1_760_572_800.0).is_ok());
        });
    }

    #[test]
    fn save_refuses_reserved_task_types() {
        with_env_root("reserved", |_root| {
            let mut request = valid_request(samples()[0].content, "user");
            request.task_type = "weekly_report".to_string();
            let error = save_artifact(&request, 1_760_572_800.0).unwrap_err();
            assert!(error.contains("尚未实现"));
        });
    }

    // --- R1：人工确认必须在 Rust 边界强制 ---

    #[test]
    fn save_requires_explicit_confirmation() {
        with_env_root("confirm", |_root| {
            // 未确认（false）必须拒绝，即便备注写得很像已确认。
            let mut request = valid_request(samples()[0].content, "user");
            request.user_confirmed = false;
            request.confirmation_note = "已确认".to_string();
            let error = save_artifact(&request, 1_760_572_800.0).unwrap_err();
            assert!(
                error.contains("人工确认"),
                "未确认必须被拒绝：{}",
                error
            );

            // 只有 true 才允许保存。
            let ok = valid_request(samples()[0].content, "user");
            assert!(ok.user_confirmed);
            assert!(save_artifact(&ok, 1_760_572_800.0).is_ok());
        });
    }

    #[test]
    fn save_request_rejects_missing_confirmation_field() {
        // R1：请求里缺失 user_confirmed 字段时反序列化直接失败，命令层无法用旧请求绕过确认。
        let json = r#"{
            "task_type": "desensitized_analysis",
            "input_text": "类别,数值\n甲,1\n",
            "explanation_text": "x",
            "explanation_source": "user",
            "confirmation_note": "已确认",
            "fingerprint": "0"
        }"#;
        let parsed = serde_json::from_str::<SaveArtifactRequest>(json);
        assert!(
            parsed.is_err(),
            "缺失 user_confirmed 的请求必须反序列化失败"
        );
    }

    // --- R3：分析与提示词入口统一闸门 ---

    #[test]
    fn analysis_gate_blocks_flagged_input() {
        let flagged = "类别,数值\n甲,110101199003077777\n";
        let error = analyze_input(flagged).unwrap_err();
        assert!(error.contains("红线扫描"), "应命中闸门：{}", error);
        assert!(!error.contains("110101199003077777"), "错误信息不得回显原文");
        assert!(assert_analysis_input_allowed(flagged).is_err());

        // 干净合成输入不受影响，原有统计与指纹不回归。
        let clean = samples()[0].content;
        let via_gate = analyze_input(clean).expect("干净输入应可分析");
        assert_eq!(via_gate, deterministic_analysis(clean).unwrap());
    }

    #[test]
    fn explanation_prompt_gate_blocks_flagged_input() {
        let flagged = "类别,数值\n甲,110101199003077777\n";
        let error =
            build_explanation_prompt("desensitized_analysis", flagged, &"0".repeat(64)).unwrap_err();
        assert!(
            error.contains("红线扫描"),
            "命中应优先于指纹校验：{}",
            error
        );
        assert!(!error.contains("110101199003077777"));
    }

    // --- R4：符号链接指向禁区 / 仓库时必须拒绝 ---

    #[cfg(unix)]
    #[test]
    fn symlink_to_repo_and_forbidden_roots_are_rejected() {
        use std::os::unix::fs::symlink;
        let base = temp_root("symlink");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("创建符号链接测试基目录");

        // 1) 指向仓库的软链接目录。
        let repo = repo_dir();
        let link_to_repo = base.join("link-to-repo");
        symlink(&repo, &link_to_repo).expect("创建指向仓库的软链接");
        assert!(
            assert_local_root_allowed(&link_to_repo.join("out")).is_err(),
            "软链接指向仓库时必须拒绝（禁区片段）"
        );
        assert!(
            assert_outside_repo(&link_to_repo.join("out")).is_err(),
            "软链接指向仓库时必须拒绝（仓库边界）"
        );

        // 2) 指向仿真禁区名的软链接（词法路径本身不含禁区片段）。
        let safe_target = base.join("plain-target");
        fs::create_dir_all(&safe_target).expect("创建普通目标目录");
        let disguised = base.join("disguised");
        symlink(&safe_target, &disguised).expect("创建普通软链接");
        assert!(
            assert_local_root_allowed(&disguised.join("out")).is_ok(),
            "普通软链接目录（真实落点干净）应可通过"
        );

        // 3) 软链接真实落点落到含禁区片段的目录：词法名字干净，解析后才暴露。
        let secret = base.join(".dsh");
        fs::create_dir_all(&secret).expect("创建仿真禁区目录");
        let masked = base.join("innocent-name");
        symlink(&secret, &masked).expect("创建指向禁区的软链接");
        assert!(
            assert_local_root_allowed(&masked.join("out")).is_err(),
            "真实落点为禁区时，即便词法名字干净也必须拒绝"
        );

        let _ = fs::remove_dir_all(&base);
    }

    // --- 解释提示词 ---

    #[test]
    fn explanation_prompt_contains_frozen_block_and_refuses_mismatch() {
        let input = samples()[1].content;
        let analysis = deterministic_analysis(input).unwrap();
        let prompt =
            build_explanation_prompt("desensitized_analysis", input, &analysis.fingerprint).unwrap();
        assert!(prompt.contains("冻结统计块"));
        assert!(prompt.contains(&analysis.fingerprint[..16]));
        assert!(prompt.contains("只做解释，不要重算"));

        let mismatch =
            build_explanation_prompt("desensitized_analysis", input, "0".repeat(64).as_str())
                .unwrap_err();
        assert!(mismatch.contains("指纹不一致"));

        let reserved = build_explanation_prompt("weekly_report", input, &analysis.fingerprint)
            .unwrap_err();
        assert!(reserved.contains("尚未实现"));
    }

    // --- 使用记录 ---

    #[test]
    fn usage_record_has_no_free_text_fields() {
        let record = build_usage_record(
            &UsageRecordInput {
                task_type: "desensitized_analysis".to_string(),
                completed: true,
                usable: true,
                rework_kind: "none".to_string(),
                blocker: "none".to_string(),
                duration_bucket: None,
            },
            0.0,
        )
        .unwrap();
        let value = serde_json::to_value(&record).unwrap();
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "blocker",
                "completed",
                "duration_bucket",
                "record_id",
                "recorded_at",
                "rework_kind",
                "schema",
                "task_type",
                "usable"
            ]
        );
    }

    #[test]
    fn usage_record_rejects_invalid_enums() {
        let base = UsageRecordInput {
            task_type: "desensitized_analysis".to_string(),
            completed: true,
            usable: true,
            rework_kind: "none".to_string(),
            blocker: "none".to_string(),
            duration_bucket: None,
        };
        assert!(build_usage_record(&base, 0.0).is_ok());

        let mut bad_type = base.clone();
        bad_type.task_type = "not_a_type".to_string();
        assert!(build_usage_record(&bad_type, 0.0).is_err());

        let mut bad_rework = base.clone();
        bad_rework.rework_kind = "随便写的说明".to_string();
        assert!(build_usage_record(&bad_rework, 0.0).is_err());

        let mut bad_blocker = base.clone();
        bad_blocker.blocker = "真实业务细节".to_string();
        assert!(build_usage_record(&bad_blocker, 0.0).is_err());

        let mut bad_bucket = base.clone();
        bad_bucket.duration_bucket = Some("很久".to_string());
        assert!(build_usage_record(&bad_bucket, 0.0).is_err());
    }

    #[test]
    fn usage_append_read_delete_roundtrip_stays_outside_repo() {
        with_env_root("usage", |root| {
            let path = root.join("logs/usage.jsonl");
            let first = build_usage_record(
                &UsageRecordInput {
                    task_type: "desensitized_analysis".to_string(),
                    completed: true,
                    usable: true,
                    rework_kind: "wording".to_string(),
                    blocker: "none".to_string(),
                    duration_bucket: Some("5_15m".to_string()),
                },
                1_760_572_800.0,
            )
            .unwrap();
            let second = build_usage_record(
                &UsageRecordInput {
                    task_type: "desensitized_analysis".to_string(),
                    completed: false,
                    usable: false,
                    rework_kind: "none".to_string(),
                    blocker: "credentials_missing".to_string(),
                    duration_bucket: None,
                },
                1_760_572_900.0,
            )
            .unwrap();
            append_usage_record(&path, &first).unwrap();
            append_usage_record(&path, &second).unwrap();

            let read = read_usage_records(&path).unwrap();
            assert_eq!(read.records.len(), 2);
            assert_eq!(read.unparsable_lines, 0);
            assert!(read.path.starts_with(&root.to_string_lossy().to_string()));

            assert_eq!(delete_usage_record(&path, &first.record_id).unwrap(), 1);
            let after = read_usage_records(&path).unwrap();
            assert_eq!(after.records.len(), 1);
            assert_eq!(after.records[0].record_id, second.record_id);

            assert!(clear_usage_records(&path).unwrap());
            assert_eq!(read_usage_records(&path).unwrap().records.len(), 0);
        });
    }

    #[test]
    fn usage_records_cannot_be_written_into_repo() {
        let inside = repo_dir().join("tests/reports/usage.jsonl");
        let record = build_usage_record(
            &UsageRecordInput {
                task_type: "desensitized_analysis".to_string(),
                completed: true,
                usable: true,
                rework_kind: "none".to_string(),
                blocker: "none".to_string(),
                duration_bucket: None,
            },
            0.0,
        )
        .unwrap();
        assert!(append_usage_record(&inside, &record).is_err());
        assert!(read_usage_records(&inside).is_err());
        assert!(clear_usage_records(&inside).is_err());
    }

    // --- 时间换算 ---

    #[test]
    fn utc_formatting_matches_known_epochs() {
        assert_eq!(utc_stamp(0.0), "19700101-000000");
        assert_eq!(utc_stamp(1_760_572_800.0), "20251016-000000");
        assert_eq!(utc_stamp(1_760_000_000.0), "20251009-085320");
        assert_eq!(utc_stamp(1_700_000_000.0), "20231114-221320");
        assert_eq!(utc_iso(0.0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc_iso(1_760_572_800.0), "2025-10-16 00:00:00 UTC");
    }

    // --- 任务类型与样例 ---

    #[test]
    fn only_desensitized_analysis_is_implemented() {
        assert!(is_implemented("desensitized_analysis"));
        assert!(!is_implemented("weekly_report"));
        assert!(!is_implemented("material_writing"));
        assert!(!is_implemented("project_management"));
        assert!(!is_implemented("does_not_exist"));

        let implemented: Vec<&str> = task_types()
            .into_iter()
            .filter(|t| t.status == "implemented")
            .map(|t| t.task_type)
            .collect();
        assert_eq!(implemented, vec!["desensitized_analysis"]);
    }

    #[test]
    fn samples_are_synthetic_and_analyzable() {
        let all = samples();
        assert_eq!(all.len(), 3);
        for sample in &all {
            assert!(sample.note.contains("纯虚构数据"), "{}", sample.sample_id);
            let result = deterministic_analysis(sample.content).unwrap();
            assert!(result.rows >= 3);
            assert!(result.distinct_categories >= 3);
        }
        // 离群值样例应确实捕捉到离群点。
        let outlier = deterministic_analysis(all[1].content).unwrap();
        assert!(outlier.max >= 100.0);
        // 均衡样例应确实完全均衡。
        let flat = deterministic_analysis(all[2].content).unwrap();
        assert_eq!(fmt_num(flat.min), fmt_num(flat.max));
    }
}
