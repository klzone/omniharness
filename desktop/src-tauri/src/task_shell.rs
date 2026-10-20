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
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::app_paths::resolve_real_path;
use crate::flow_pack::FlowPack;
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

/// 提交时红线扫描的对象：优先用户原始输入，缺省时退回完整提示词。
///
/// 提示词模板含边界说明用词（案件、线索、未公开），扫描全文会让
/// 正常提交全部被误拒；用户原文才是需要判定是否禁入的内容。
pub fn scan_target<'a>(prompt_text: &'a str, user_input_text: Option<&'a str>) -> &'a str {
    user_input_text.unwrap_or(prompt_text)
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

/// 覆盖本地数据根的官方入口（绝对路径）。
#[cfg(test)]
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
///
/// 优先级：`OMNIDESK_LOCAL_DATA_ROOT`（开发/测试隔离）→ 应用设置中的成果目录 →
/// 默认 `~/OmniDesk/workspace/artifacts`。只影响**未来新写入**的文件，
/// 不读取、不迁移、不改名任何旧路径中的内容。
pub fn resolve_local_data_root() -> Result<PathBuf, String> {
    let config = crate::settings::app_config_dir()?;
    let root = crate::settings::effective_root(&config)?.root;
    assert_local_root_allowed(&root)?;
    assert_outside_repo(&root)?;
    Ok(root)
}

/// 幂等准备本地数据根，供引擎任务用作工作目录。
///
/// 为什么需要单独一个入口：引擎侧把工作目录收敛为唯一物理身份时要求该目录
/// **已经存在且可解析**（见 engine/omnidesk_engine/acp_adapter.py 的 canonical_cwd），
/// 不存在会被判为 invalid_argument。这里复用与保存入口同一套边界校验后创建目录本身，
/// 不写入任何内容；调用是幂等的。
pub fn prepare_local_data_root() -> Result<PathBuf, String> {
    let root = resolve_local_data_root()?;
    fs::create_dir_all(&root).map_err(|e| format!("创建本地数据目录失败：{}", e))?;
    // 目录真正建立后再校验一次，防止符号链接等把落点引到禁区。
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

    write_new_file_durable(&target, &content)?;

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
    /// 是否愿意在类似任务中再次使用。只是布尔值，不承载任何理由文本。
    /// 旧记录没有该字段时默认为 false，保证历史记录仍可读。
    #[serde(default)]
    pub reuse_willingness: bool,
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
    #[serde(default = "default_reuse_willingness")]
    pub reuse_willingness: bool,
}

fn default_reuse_willingness() -> bool {
    false
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
        reuse_willingness: input.reuse_willingness,
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
// 离线场景流程（不调用模型）
//
// 这三个流程是**纯本地**的：只做格式校验、确定性统计和 Markdown 生成。
// 模型撰写步骤在端点准入关闭期间一律标为「受准入阻塞、未验证」。
// 它们不是通用工作流引擎，各自只有一套固定字段和固定规则。
// ---------------------------------------------------------------------------

/// 周记录汇总的状态取值。
pub const WEEKLY_STATUSES: &[&str] = &["done", "pending", "blocked"];
/// 项目跟进的状态取值。
pub const PROJECT_STATUSES: &[&str] = &["not_started", "in_progress", "blocked", "done"];

fn scenario_err(line_no: usize, message: &str) -> String {
    format!("第 {} 行：{}", line_no, message)
}

/// 某年某月的实际天数（含公历闰年规则）。
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// 日期校验：YYYY-MM-DD，按实际月份天数与闰年规则拒绝不存在的日期。
/// 周记录日期、项目期限与逾期判定日共用本函数。
fn parse_date(text: &str) -> Option<(i32, u32, u32)> {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    // 只接受 ASCII 数字，避免非数字字符被 parse 拒绝时产生歧义。
    let digits_ok = bytes
        .iter()
        .enumerate()
        .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit());
    if !digits_ok {
        return None;
    }
    let year: i32 = text[0..4].parse().ok()?;
    let month: u32 = text[5..7].parse().ok()?;
    let day: u32 = text[8..10].parse().ok()?;
    if !(1900..=2999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    if day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some((year, month, day))
}

fn date_cmp(a: (i32, u32, u32), b: (i32, u32, u32)) -> std::cmp::Ordering {
    a.cmp(&b)
}

/// 周记录中的一条事项。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct WeeklyItem {
    pub date: String,
    pub item: String,
    pub status: String,
}

/// 周记录汇总结果。所有计数都由本模块确定性计算。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct WeeklySummary {
    pub schema: &'static str,
    pub total: u64,
    pub done: u64,
    pub pending: u64,
    pub blocked: u64,
    pub items: Vec<WeeklyItem>,
    pub notes: Vec<String>,
}

pub const WEEKLY_SCHEMA: &str = "omnidesk.weekly.v1";

/// 每行格式：`日期 | 事项 | 状态`。字段缺失、日期非法、状态未知都按行号报错。
pub fn summarize_week(input: &str) -> Result<WeeklySummary, String> {
    assert_input_allowed(input)?;
    let mut items: Vec<WeeklyItem> = Vec::new();
    for (index, raw) in input.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(|c| c.trim()).collect();
        if cells.len() != 3 {
            return Err(scenario_err(
                line_no,
                "每行必须是「日期 | 事项 | 状态」三个字段，用竖线分隔",
            ));
        }
        if cells[1].is_empty() {
            return Err(scenario_err(line_no, "事项不能为空"));
        }
        if parse_date(cells[0]).is_none() {
            return Err(scenario_err(
                line_no,
                "日期必须是存在的 YYYY-MM-DD（例如 2026-02-30 不存在）",
            ));
        }
        if !WEEKLY_STATUSES.contains(&cells[2]) {
            return Err(scenario_err(
                line_no,
                "状态只允许 done / pending / blocked",
            ));
        }
        items.push(WeeklyItem {
            date: cells[0].to_string(),
            item: cells[1].to_string(),
            status: cells[2].to_string(),
        });
    }
    if items.is_empty() {
        return Err("没有可用事项：每行必须是「日期 | 事项 | 状态」".to_string());
    }

    let mut done = 0u64;
    let mut pending = 0u64;
    let mut blocked = 0u64;
    for item in &items {
        match item.status.as_str() {
            "done" => done += 1,
            "pending" => pending += 1,
            _ => blocked += 1,
        }
    }

    let mut notes = Vec::new();
    let mut dates: Vec<(i32, u32, u32)> = items
        .iter()
        .filter_map(|item| parse_date(&item.date))
        .collect();
    dates.sort_unstable();
    dates.dedup();
    if dates.len() != items.len() {
        notes.push(
            "同一天有多条事项：未按自然日去重，周报中的条数按事项数统计。".to_string(),
        );
    }
    if pending > 0 || blocked > 0 {
        notes.push(format!(
            "有 {} 条未完成事项（pending {} / blocked {}），下周计划需逐条确认责任人与期限。",
            pending + blocked,
            pending,
            blocked
        ));
    }
    if items.iter().any(|item| item.item.trim().is_empty()) {
        notes.push("存在事项内容为空：请补充后再生成周报。".to_string());
    }
    notes.push(
        "本汇总只统计已填写的记录；未记录的工作不会自动出现，缺失信息需人工确认。"
            .to_string(),
    );

    Ok(WeeklySummary {
        schema: WEEKLY_SCHEMA,
        total: items.len() as u64,
        done,
        pending,
        blocked,
        items,
        notes,
    })
}

/// 把周记录汇总渲染为 Markdown。模型步骤明确标注为受准入阻塞。
pub fn render_weekly_markdown(summary: &WeeklySummary) -> String {
    render_weekly_markdown_with(summary, &crate::flow_pack::default_pack())
}

/// 带流程包的渲染：只替换展示文案，统计与固定声明仍由本模块给出。
pub fn render_weekly_markdown_with(summary: &WeeklySummary, pack: &FlowPack) -> String {
    let copy = &pack.weekly;
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", copy.result_title.trim()));
    out.push_str("- 数据来源：使用者填写的周记录，纯本地处理\n");
    out.push_str("- 模型撰写：受准入阻塞，未验证（本次未调用任何模型）\n");
    out.push_str(&format!("- 事项总数：{}\n", summary.total));
    out.push_str(&format!("- 已完成：{}\n", summary.done));
    out.push_str(&format!("- 待完成：{}\n", summary.pending));
    out.push_str(&format!("- 受阻：{}\n\n", summary.blocked));
    out.push_str(&format!("## {}\n\n", copy.details_heading.trim()));
    out.push_str("| 日期 | 事项 | 状态 |\n| --- | --- | --- |\n");
    for item in &summary.items {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            item.date, item.item, item.status
        ));
    }
    out.push_str(&format!("\n## {}\n\n", copy.confirm_heading.trim()));
    for note in &summary.notes {
        out.push_str(&format!("- {}\n", note));
    }
    out
}

/// 一般文案的结构建议结果。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct WritingPlan {
    pub schema: &'static str,
    pub purpose: String,
    pub audience: String,
    pub sections: Vec<String>,
    pub missing: Vec<String>,
    pub template_used: Option<String>,
}

pub const WRITING_SCHEMA: &str = "omnidesk.writing-plan.v1";

/// 按用途与受众给出结构建议；信息不足时只提出结构，不强行套模板。
///
/// 刻意不按关键词匹配固定模板：目的、受众、明确要求缺失时，先列待确认项。
pub fn plan_writing(
    purpose: &str,
    audience: &str,
    requirements: &str,
    facts: &str,
) -> Result<WritingPlan, String> {
    assert_input_allowed(purpose)?;
    assert_input_allowed(audience)?;
    assert_input_allowed(requirements)?;
    assert_input_allowed(facts)?;

    let purpose = purpose.trim();
    let audience = audience.trim();
    if purpose.is_empty() {
        return Err("缺少材料目的：请先说明这份材料要解决什么问题、用于什么场合".to_string());
    }
    if audience.is_empty() {
        return Err("缺少受众：请说明材料交给谁看、由谁使用".to_string());
    }

    let mut missing = Vec::new();
    if requirements.trim().is_empty() {
        missing.push("明确要求（篇幅、时限、必须包含的内容）待确认".to_string());
    }
    if facts.trim().is_empty() {
        missing.push("可用事实与数据待确认：没有事实依据时不得编造".to_string());
    }
    if facts
        .lines()
        .any(|line| line.contains("TODO") || line.contains("待补"))
    {
        missing.push("事实清单中仍有未填写的占位项，需补齐后再定稿".to_string());
    }

    let mut sections = vec![
        format!("开头：说明背景与目的（面向{}）", audience),
        format!("主体：围绕「{}」展开事实与做法", purpose),
        "结果：只写有依据的成效，无数据则说明未统计".to_string(),
        "下一步：列出待办与需要确认的事项".to_string(),
    ];
    if !requirements.trim().is_empty() {
        sections.push("按要求补充的章节（见使用者填写的明确要求）".to_string());
    }
    if facts.trim().is_empty() {
        sections.insert(0, "【待确认】事实依据缺失，暂不能形成初稿".to_string());
    }

    sections.push("模型撰写：受准入阻塞，未验证（本次未调用任何模型）".to_string());

    Ok(WritingPlan {
        schema: WRITING_SCHEMA,
        purpose: purpose.to_string(),
        audience: audience.to_string(),
        sections,
        missing,
        template_used: None,
    })
}

/// 把结构建议渲染为 Markdown。
pub fn render_writing_markdown(plan: &WritingPlan) -> String {
    render_writing_markdown_with(plan, &crate::flow_pack::default_pack())
}

/// 带流程包的渲染：标题、章节名可由流程包调整，结构内容仍来自确定性计划。
pub fn render_writing_markdown_with(plan: &WritingPlan, pack: &FlowPack) -> String {
    let copy = &pack.writing;
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", copy.result_title.trim()));
    out.push_str(&format!("- 目的：{}\n", plan.purpose));
    out.push_str(&format!("- 受众：{}\n", plan.audience));
    out.push_str(&format!(
        "- 模板：{}\n\n",
        match &plan.template_used {
            Some(name) => format!("使用「{}」", name),
            None => "未使用模板（没有与用途匹配的模板，按用途组织）".to_string(),
        }
    ));
    out.push_str(&format!("## {}\n\n", copy.structure_heading.trim()));
    for (index, section) in plan.sections.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", index + 1, section));
    }
    out.push_str(&format!("\n## {}\n\n", copy.pending_heading.trim()));
    if plan.missing.is_empty() {
        out.push_str("- 无（仍需人工核对事实）\n");
    } else {
        for item in &plan.missing {
            out.push_str(&format!("- {}\n", item));
        }
    }
    out
}

/// 项目跟进中的一条事项。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProjectItem {
    pub item: String,
    pub owner: String,
    pub due: String,
    pub status: String,
    pub next_step: String,
    pub overdue: bool,
}

/// 项目跟进汇总。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProjectSummary {
    pub schema: &'static str,
    pub total: u64,
    pub done: u64,
    pub in_progress: u64,
    pub blocked: u64,
    pub not_started: u64,
    pub overdue: u64,
    pub items: Vec<ProjectItem>,
    pub notes: Vec<String>,
}

pub const PROJECT_SCHEMA: &str = "omnidesk.project.v1";

/// 每行格式：`事项 | 负责人代号 | 期限 | 状态 | 下一步`。
///
/// `today` 由调用方显式传入（YYYY-MM-DD），用于判定逾期；不读系统时钟，
/// 保证同样的输入与日期得到同样的输出。
pub fn summarize_project(input: &str, today: &str) -> Result<ProjectSummary, String> {
    assert_input_allowed(input)?;
    let today_date = parse_date(today)
        .ok_or_else(|| {
            "today 必须是存在的 YYYY-MM-DD 日期（按实际月份天数与闰年校验），用于判定逾期"
                .to_string()
        })?;
    let mut items: Vec<ProjectItem> = Vec::new();
    for (index, raw) in input.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(|c| c.trim()).collect();
        if cells.len() != 5 {
            return Err(scenario_err(
                line_no,
                "每行必须是「事项 | 负责人代号 | 期限 | 状态 | 下一步」五个字段",
            ));
        }
        for (pos, label) in ["事项", "负责人代号", "下一步"].iter().enumerate() {
            let cell = match pos {
                0 => cells[0],
                1 => cells[1],
                _ => cells[4],
            };
            if cell.is_empty() {
                return Err(scenario_err(line_no, &format!("{}不能为空", label)));
            }
        }
        if !cells[1].chars().next().map(|c| c == 'P').unwrap_or(false)
            || !cells[1][1..].chars().all(|c| c.is_ascii_digit())
        {
            return Err(scenario_err(
                line_no,
                "负责人只填代号（例如 P1），不得填写真实姓名",
            ));
        }
        let due = parse_date(cells[2])
            .ok_or_else(|| {
                scenario_err(
                    line_no,
                    "期限必须是存在的 YYYY-MM-DD（例如 2026-02-30 不存在）",
                )
            })?;
        if !PROJECT_STATUSES.contains(&cells[3]) {
            return Err(scenario_err(
                line_no,
                "状态只允许 not_started / in_progress / blocked / done",
            ));
        }
        let status = cells[3];
        let overdue = status != "done" && date_cmp(due, today_date) == std::cmp::Ordering::Less;
        items.push(ProjectItem {
            item: cells[0].to_string(),
            owner: cells[1].to_string(),
            due: cells[2].to_string(),
            status: status.to_string(),
            next_step: cells[4].to_string(),
            overdue,
        });
    }
    if items.is_empty() {
        return Err("没有可用事项：每行必须是五个字段".to_string());
    }

    let mut done = 0u64;
    let mut in_progress = 0u64;
    let mut blocked = 0u64;
    let mut not_started = 0u64;
    let mut overdue = 0u64;
    for item in &items {
        match item.status.as_str() {
            "done" => done += 1,
            "in_progress" => in_progress += 1,
            "blocked" => blocked += 1,
            _ => not_started += 1,
        }
        if item.overdue {
            overdue += 1;
        }
    }
    let mut notes = Vec::new();
    if blocked > 0 {
        notes.push(format!("有 {} 条受阻事项，需要明确解除条件。", blocked));
    }
    if overdue > 0 {
        notes.push(format!(
            "有 {} 条已过期限且未完成，需要重新确认期限或范围。",
            overdue
        ));
    }
    notes.push(format!(
        "逾期判定基于调用方给定的日期 {}；未记录的事项不会自动出现。",
        today
    ));
    notes.push("模型撰写：受准入阻塞，未验证（本次未调用任何模型）".to_string());

    Ok(ProjectSummary {
        schema: PROJECT_SCHEMA,
        total: items.len() as u64,
        done,
        in_progress,
        blocked,
        not_started,
        overdue,
        items,
        notes,
    })
}

/// 把项目跟进汇总渲染为 Markdown。
pub fn render_project_markdown(summary: &ProjectSummary) -> String {
    render_project_markdown_with(summary, &crate::flow_pack::default_pack())
}

/// 带流程包的渲染：只替换标题与章节名，逾期判定等结果不变。
pub fn render_project_markdown_with(summary: &ProjectSummary, pack: &FlowPack) -> String {
    let copy = &pack.project;
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", copy.result_title.trim()));
    out.push_str(&format!("- 事项总数：{}\n", summary.total));
    out.push_str(&format!(
        "- 状态：done {} / in_progress {} / blocked {} / not_started {}\n",
        summary.done, summary.in_progress, summary.blocked, summary.not_started
    ));
    out.push_str(&format!("- 逾期：{}\n\n", summary.overdue));
    out.push_str("| 事项 | 负责人 | 期限 | 状态 | 下一步 |\n| --- | --- | --- | --- | --- |\n");
    for item in &summary.items {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            item.item, item.owner, item.due, item.status, item.next_step
        ));
    }
    out.push_str(&format!("\n## {}\n\n", copy.confirm_heading.trim()));
    for note in &summary.notes {
        out.push_str(&format!("- {}\n", note));
    }
    out
}

// ---------------------------------------------------------------------------
// 离线流程：Tauri 命令用的薄封装
//
// 三个场景都**不调用模型**。返回值里 fixed 字段显式携带「模型步骤受准入阻塞」，
// UI 不得把它显示成已生成。
// ---------------------------------------------------------------------------

/// 周记录汇总响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WeeklyFlowResponse {
    pub ok: bool,
    pub summary: Option<WeeklySummary>,
    pub markdown: Option<String>,
    pub model_step: &'static str,
    pub error: Option<String>,
}

pub const MODEL_STEP_BLOCKED: &str = "受准入阻塞、未验证：本次未调用任何模型";

pub fn weekly_flow(input: &str) -> WeeklyFlowResponse {
    weekly_flow_with(input, &crate::flow_pack::default_pack())
}

/// 带流程包的周报流程。计算不变，仅换展示文案。
pub fn weekly_flow_with(input: &str, pack: &FlowPack) -> WeeklyFlowResponse {
    match summarize_week(input) {
        Ok(summary) => WeeklyFlowResponse {
            ok: true,
            markdown: Some(render_weekly_markdown_with(&summary, pack)),
            summary: Some(summary),
            model_step: MODEL_STEP_BLOCKED,
            error: None,
        },
        Err(message) => WeeklyFlowResponse {
            ok: false,
            summary: None,
            markdown: None,
            model_step: MODEL_STEP_BLOCKED,
            error: Some(message),
        },
    }
}

/// 一般文案结构建议响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WritingFlowResponse {
    pub ok: bool,
    pub plan: Option<WritingPlan>,
    pub markdown: Option<String>,
    pub model_step: &'static str,
    pub error: Option<String>,
}

pub fn writing_flow(
    purpose: &str,
    audience: &str,
    requirements: &str,
    facts: &str,
) -> WritingFlowResponse {
    writing_flow_with(
        purpose,
        audience,
        requirements,
        facts,
        &crate::flow_pack::default_pack(),
    )
}

/// 带流程包的文案结构流程。
pub fn writing_flow_with(
    purpose: &str,
    audience: &str,
    requirements: &str,
    facts: &str,
    pack: &FlowPack,
) -> WritingFlowResponse {
    match plan_writing(purpose, audience, requirements, facts) {
        Ok(plan) => WritingFlowResponse {
            ok: true,
            markdown: Some(render_writing_markdown_with(&plan, pack)),
            plan: Some(plan),
            model_step: MODEL_STEP_BLOCKED,
            error: None,
        },
        Err(message) => WritingFlowResponse {
            ok: false,
            plan: None,
            markdown: None,
            model_step: MODEL_STEP_BLOCKED,
            error: Some(message),
        },
    }
}

/// 项目跟进汇总响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectFlowResponse {
    pub ok: bool,
    pub summary: Option<ProjectSummary>,
    pub markdown: Option<String>,
    pub model_step: &'static str,
    pub error: Option<String>,
}

/// `today` 为空时使用 UTC 当天日期；显式传入则完全可复现。
pub fn project_flow(input: &str, today: Option<&str>) -> ProjectFlowResponse {
    project_flow_with(input, today, &crate::flow_pack::default_pack())
}

/// 带流程包的项目跟进流程。
pub fn project_flow_with(input: &str, today: Option<&str>, pack: &FlowPack) -> ProjectFlowResponse {
    let resolved = match today {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => utc_iso(now_secs())[..10].to_string(),
    };
    match summarize_project(input, &resolved) {
        Ok(summary) => ProjectFlowResponse {
            ok: true,
            markdown: Some(render_project_markdown_with(&summary, pack)),
            summary: Some(summary),
            model_step: MODEL_STEP_BLOCKED,
            error: None,
        },
        Err(message) => ProjectFlowResponse {
            ok: false,
            summary: None,
            markdown: None,
            model_step: MODEL_STEP_BLOCKED,
            error: Some(message),
        },
    }
}

/// 离线流程保存请求：不涉及模型，不需要统计指纹。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SaveOfflineRequest {
    pub scenario: String,
    pub title: String,
    pub markdown: String,
    /// 人工确认是硬前提，缺失该字段会让反序列化失败。
    pub user_confirmed: bool,
    /// 成果是否包含模型生成的初稿。缺省为 false（纯本地确定性处理）。
    /// 只用于在产物里如实标注来源，不影响任何计算或准入判断。
    #[serde(default)]
    pub ai_assisted: bool,
}

/// 产物落盘的三步文件操作。生产路径固定为真实实现；仅供 Rust 测试注入故障。
///
/// 刻意不实现 `Default`、不从环境变量或配置读取，也没有任何 pub 导出：
/// 宿主命令、前端、环境变量都无法把它换成失败版本。
struct NewFileOps {
    write: fn(&mut std::fs::File, &[u8]) -> io::Result<()>,
    sync: fn(&mut std::fs::File) -> io::Result<()>,
    remove: fn(&Path) -> io::Result<()>,
}

impl NewFileOps {
    /// 生产用：真实写入、真实刷盘、真实删除。
    const REAL: NewFileOps = NewFileOps {
        write: |file, bytes| file.write_all(bytes),
        sync: |file| file.sync_all(),
        remove: |path| fs::remove_file(path),
    };
}

/// 新建并写入一个产物文件：绝不覆盖既有文件，失败时清掉本次半成品。
///
/// 两条保存路径（分析成果、离线场景成果）共用这里，避免行为分叉。
/// `create_new(true)` 同时承担「存在即拒绝」和「只删自己刚建的文件」两个作用。
fn write_new_file_durable(target: &Path, content: &str) -> Result<(), String> {
    write_new_file_durable_with(target, content, &NewFileOps::REAL)
}

fn write_new_file_durable_with(
    target: &Path,
    content: &str,
    ops: &NewFileOps,
) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| {
            format!(
                "无法创建产物文件 {}：{}（已存在或不可写；本阶段不覆盖已有文件）",
                target.display(),
                e
            )
        })?;
    let write_result = (ops.write)(&mut file, content.as_bytes()).and_then(|_| (ops.sync)(&mut file));
    drop(file);
    if let Err(e) = write_result {
        // 失败时删除本次新建的半成品；既有同名文件绝不会被触及。
        let suffix = match (ops.remove)(target) {
            Ok(()) => "已删除本次未完成的半成品".to_string(),
            Err(cleanup_err) => format!("清理半成品失败：{}", cleanup_err),
        };
        return Err(format!("写入产物失败：{}；{}", e, suffix));
    }
    Ok(())
}

/// 保存离线场景成果到仓库外本地目录（不覆盖）。
pub fn save_offline_artifact(
    request: &SaveOfflineRequest,
    now: f64,
) -> Result<SaveArtifactReceipt, String> {
    if !matches!(
        request.scenario.as_str(),
        "weekly_report" | "material_writing" | "project_management"
    ) {
        return Err(format!("未知离线场景：{:?}", request.scenario));
    }
    if !request.user_confirmed {
        return Err("必须显式人工确认后才能保存离线成果。".to_string());
    }
    if request.markdown.trim().is_empty() {
        return Err("成果内容为空，拒绝保存。".to_string());
    }
    let root = resolve_local_data_root()?;
    fs::create_dir_all(&root).map_err(|e| format!("创建本地数据目录失败：{}", e))?;
    assert_local_root_allowed(&root)?;
    assert_outside_repo(&root)?;

    let file_name = format!(
        "offline-{}-{}.md",
        request.scenario.replace('_', "-"),
        utc_stamp(now)
    );
    let target = root.join(&file_name);
    assert_local_root_allowed(&target)?;
    assert_outside_repo(&target)?;

    let mut content = String::new();
    content.push_str(&format!("# {}\n\n", request.title.trim()));
    content.push_str(&format!("- 场景：{}\n", request.scenario));
    content.push_str(&format!("- 生成时间 (UTC)：{}\n", utc_iso(now)));
    if request.ai_assisted {
        content.push_str("- 处理方式：本地确定性代码 + 模型初稿（模型来源未复核，已由人工编辑核对）\n");
    } else {
        content.push_str("- 处理方式：本地确定性代码；未调用模型\n");
    }
    content.push_str("- 数据边界：仅合成 / 公开 / 经明确批准的数据\n\n");
    content.push_str(&request.markdown);

    write_new_file_durable(&target, &content)?;

    Ok(SaveArtifactReceipt {
        path: target.display().to_string(),
        fingerprint: "offline".to_string(),
        bytes: content.len() as u64,
        written_at: now,
    })
}

/// 已保存成果的一条只读摘要：只含自生成文件名、大小与时间，不回读正文。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OfflineArtifactEntry {
    pub file_name: String,
    pub bytes: u64,
    pub modified_at: f64,
    pub scenario: String,
}

/// 读取一条已保存成果的正文，供用户重新打开确认。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OfflineArtifactContent {
    pub file_name: String,
    pub content: String,
}

/// 只接受本模块自己生成的 `offline-<scenario>-<stamp>.md` 文件名。
///
/// 重新打开是只读操作，但仍要求严格的文件名形状与所在目录校验：
/// 不允许借 `..`、符号链接或任意路径读取数据根之外的任何文件。
fn offline_artifact_name_allowed(name: &str) -> bool {
    let Some((_, stamp)) = split_offline_artifact_name(name) else {
        return false;
    };
    let bytes = stamp.as_bytes();
    if bytes.len() != 15 || bytes[8] != b'-' {
        return false;
    }
    bytes
        .iter()
        .enumerate()
        .all(|(i, b)| i == 8 || b.is_ascii_digit())
}

/// 把 `offline-<scenario>-<YYYYMMDD>-<HHMMSS>.md` 拆成场景名与时间戳。
///
/// 场景名本身含连字符，因此按已登记场景前缀精确匹配，不能从末尾切分。
fn split_offline_artifact_name(name: &str) -> Option<(&str, &str)> {
    let stem = name.strip_prefix("offline-")?.strip_suffix(".md")?;
    for scenario in ["weekly-report", "material-writing", "project-management"] {
        if let Some(rest) = stem.strip_prefix(scenario) {
            if let Some(stamp) = rest.strip_prefix('-') {
                return Some((scenario, stamp));
            }
        }
    }
    None
}

/// 列出数据根中由本模块写入的离线成果（只读元数据，按修改时间倒序）。
pub fn list_offline_artifacts() -> Result<Vec<OfflineArtifactEntry>, String> {
    let root = resolve_local_data_root()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&root).map_err(|e| format!("读取成果目录失败：{}", e))?;
    for item in read_dir {
        let item = item.map_err(|e| format!("读取成果目录失败：{}", e))?;
        let name = item.file_name().to_string_lossy().to_string();
        if !offline_artifact_name_allowed(&name) {
            continue;
        }
        // 符号链接即使在数据根内也不跟随：只接受普通文件。
        let meta = match fs::symlink_metadata(item.path()) {
            Ok(m) if m.file_type().is_file() => m,
            _ => continue,
        };
        let scenario = split_offline_artifact_name(&name)
            .map(|(s, _)| s.to_string())
            .unwrap_or_default();
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        entries.push(OfflineArtifactEntry {
            file_name: name,
            bytes: meta.len(),
            modified_at,
            scenario,
        });
    }
    entries.sort_by(|a, b| {
        b.modified_at
            .partial_cmp(&a.modified_at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(entries)
}

/// 重新打开一条已保存成果的内容（只读，不修改文件）。
pub fn read_offline_artifact(file_name: &str) -> Result<OfflineArtifactContent, String> {
    if !offline_artifact_name_allowed(file_name) {
        return Err("只允许重新打开 OmniDesk 自己生成的离线成果文件。".to_string());
    }
    let root = resolve_local_data_root()?;
    assert_local_root_allowed(&root)?;
    let target = root.join(file_name);
    assert_local_root_allowed(&target)?;
    assert_outside_repo(&target)?;
    let meta = fs::symlink_metadata(&target)
        .map_err(|e| format!("无法读取成果 {}：{}", file_name, e))?;
    if !meta.file_type().is_file() {
        return Err("目标不是普通文件，拒绝读取。".to_string());
    }
    // 读取上限：离线成果是本地渲染的 Markdown，异常大小说明文件不属于本流程。
    const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024;
    if meta.len() > MAX_ARTIFACT_BYTES {
        return Err("成果文件超过 1 MiB，拒绝在界面中打开。".to_string());
    }
    let content = fs::read_to_string(&target)
        .map_err(|e| format!("无法读取成果 {}：{}", file_name, e))?;
    Ok(OfflineArtifactContent {
        file_name: file_name.to_string(),
        content,
    })
}

// ---------------------------------------------------------------------------
// 用户明确选择的材料文件（OD-ARCH-026）
//
// 只读、单文件、有界。这不是目录扫描器：一次只读用户显式给出的那一个文件，
// 也不解析数据库。表格/文档的“解析”就是把该文件的文本原样交给同一条引擎适配链路。
// ---------------------------------------------------------------------------

/// 允许作为模型材料的文本/表格扩展名。目录、二进制与未知类型一律拒绝。
pub const MATERIAL_ALLOWED_EXTS: &[&str] = &[
    "md", "markdown", "txt", "text", "csv", "tsv", "json", "yaml", "yml", "log",
];

/// 单个材料文件的最大字节数。超过即拒绝，不做静默截断。
pub const MATERIAL_MAX_BYTES: u64 = 512 * 1024;

/// 材料读取结果：正文 + 可核验的来源事实（供界面记录来源，不额外保存副本）。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MaterialRead {
    pub path: String,
    pub file_name: String,
    pub bytes: u64,
    pub chars: usize,
    pub extension: String,
    pub sha256: String,
    pub content: String,
}

/// 只读读取用户**明确给出路径**的一个材料文件。
///
/// 边界全部由本函数强制，而不是提示词约定：
///   * 必须是绝对路径；路径与解析后的真实落点都不得命中被禁止片段（DSH / agent / 仓库内）；
///   * 必须是已存在的普通文件（不跟随目录、设备、管道）；
///   * 扩展名必须命中白名单；
///   * 大小不超过 `MATERIAL_MAX_BYTES`；
///   * 必须是合法 UTF-8 文本。
///
/// 不递归、不列目录、不接受通配符：一次一个文件，由用户显式选择。
pub fn read_material(path_text: &str) -> Result<MaterialRead, String> {
    let raw = path_text.trim();
    if raw.is_empty() {
        return Err("请提供要读取的材料文件路径。".to_string());
    }
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err(format!("材料路径必须是绝对路径：{}", raw));
    }
    assert_no_forbidden_path_fragment(path).map_err(|_| {
        format!(
            "拒绝读取 {}：该路径位于被禁止的目录（DSH / agent / 仓库内）。",
            path.display()
        )
    })?;
    let resolved = resolve_real_path(path)?;
    assert_no_forbidden_path_fragment(&resolved).map_err(|_| {
        format!(
            "拒绝读取 {}：解析后的真实落点位于被禁止的目录（DSH / agent / 仓库内）。",
            resolved.display()
        )
    })?;
    assert_outside_repo(&resolved).map_err(|_| {
        format!(
            "拒绝读取 {}：该文件位于工作区仓库内，不能作为模型材料。",
            resolved.display()
        )
    })?;

    let meta = fs::metadata(&resolved)
        .map_err(|e| format!("无法读取材料文件 {}：{}", resolved.display(), e))?;
    if !meta.is_file() {
        return Err(format!("{} 不是普通文件，拒绝读取。", resolved.display()));
    }
    let extension = resolved
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !MATERIAL_ALLOWED_EXTS.contains(&extension.as_str()) {
        return Err(format!(
            "不支持的材料类型 {:?}；只接受纯文本与表格：{}。",
            extension,
            MATERIAL_ALLOWED_EXTS.join(" / ")
        ));
    }
    if meta.len() > MATERIAL_MAX_BYTES {
        return Err(format!(
            "材料文件 {} 字节，超过上限 {} 字节；请先自行拆分，或改用更小的文件。",
            meta.len(),
            MATERIAL_MAX_BYTES
        ));
    }
    let bytes = fs::read(&resolved)
        .map_err(|e| format!("无法读取材料文件 {}：{}", resolved.display(), e))?;
    let content = String::from_utf8(bytes)
        .map_err(|_| "材料文件不是合法 UTF-8 文本，拒绝读取。".to_string())?;
    let file_name = resolved
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| resolved.display().to_string());
    Ok(MaterialRead {
        path: resolved.display().to_string(),
        file_name,
        bytes: meta.len(),
        chars: content.chars().count(),
        extension,
        sha256: sha256_hex(content.as_bytes()),
        content,
    })
}

// ---------------------------------------------------------------------------
// 打开 DSH Desktop（模型/凭据设置界面）
// ---------------------------------------------------------------------------

/// 拉起**与引擎同款、同一份数据根**的 DSH 设置界面（web profile）。
///
/// 为什么不是直接打开系统里那个 DSH Desktop：它读的是**另一份** DSH 数据根（用户自己那份），
/// 在那里登记的模型配置 OmniDesk 根本读不到——这正是实测到的问题。
/// 这里用**引擎运行时自带**的 DSH CLI 启动 `web` profile，并把 `DSH_HOME` / `DSH_AGENTS_HOME`
/// 指向引擎实际使用的那两个目录，于是「设置界面」与「引擎」共享同一份凭据与 Profile。
///
/// 边界：
///   * 只执行固定的 `node <dsh_cli> web`，**不拼接 shell 命令**，不接受前端传入的
///     命令、路径或 URL；前端只能请求「打开 DSH 设置」这一个固定动作。
///   * 启动失败时**明确报错**，不静默失败、不假装已打开。
///   * 本函数**不读、不写、不打印任何凭证文件**；只把设置界面拉起来。
///     界面上显示的路径只是「配置落在哪」，不是机密内容。
pub fn open_dsh_settings(
    dsh_home: &Path,
    agents_home: &Path,
    dsh_cli: &Path,
    log_path: &Path,
) -> Result<serde_json::Value, String> {
    if !dsh_cli.is_file() {
        return Err(format!(
            "找不到引擎运行时里的 DSH CLI：{}；请先完成运行时引导（scripts/bootstrap-runtime.sh）。",
            dsh_cli.display()
        ));
    }
    let node = std::env::var("OMNIDESK_NODE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "node".to_string());
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| format!("打开 DSH 启动日志失败（{}）：{}", log_path.display(), e))?;
    let err = out
        .try_clone()
        .map_err(|e| format!("复制日志句柄失败：{}", e))?;
    let child = std::process::Command::new(&node)
        .arg(dsh_cli)
        .arg("web")
        .env("DSH_HOME", dsh_home)
        .env("DSH_AGENTS_HOME", agents_home)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(out))
        .stderr(std::process::Stdio::from(err))
        .spawn()
        .map_err(|e| format!("启动 DSH（{}）失败：{}", node, e))?;
    Ok(serde_json::json!({
        "ok": true,
        "launched": "dsh web",
        "pid": child.id(),
        "dsh_home": dsh_home.display().to_string(),
        "agents_home": agents_home.display().to_string(),
        "credentials_file": dsh_home.join(".credentials.yaml").display().to_string(),
        "log_path": log_path.display().to_string(),
        "note": "已启动与引擎同款的 DSH（web profile），并指向引擎正在用的那份 DSH 数据根——\
                 同一份凭据与 Profile，你在这里登记的模型配置引擎能直接读到。\
                 本地地址与启动日志见上面的日志文件；本应用不读、不写、不复制任何凭证值。",
    }))
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

    /// 边界规则正文本身包含「案件/线索/未公开」等词；扫描应以用户原文为准。
    #[test]
    fn prompt_rules_do_not_trip_red_line_scan() {
        assert!(
            !red_line_scan(explanation_rules()).is_empty(),
            "前提：规则正文含边界说明用词，直接扫描全文会误判正常提交"
        );
        assert!(
            red_line_scan(samples()[0].content).is_empty(),
            "用户原始合成输入必须通过扫描"
        );
    }

    #[test]
    fn submit_scan_target_prefers_user_input() {
        let prompt = "规则说明：不处理案件、线索或未公开信息；素材：合成数据";
        assert_eq!(scan_target(prompt, Some("合成数据")), "合成数据");
        assert_eq!(scan_target(prompt, None), prompt);
        assert!(assert_input_allowed(scan_target(prompt, Some("合成数据"))).is_ok());
        assert!(assert_input_allowed(scan_target(prompt, Some("虚构案件占位"))).is_err());
        // 场景预设自带这些词：只有把**用户原文**交上去扫描，规则本身才不会被误判成敏感内容；
        // 不给用户原文时（前端漏传）整条提示词会被扫描 → 规则里的词导致误拒。
        assert!(assert_input_allowed(scan_target(prompt, Some("请把本周进展整理成周报"))).is_ok());
        assert!(assert_input_allowed(scan_target(prompt, None)).is_err());
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
                reuse_willingness: true,
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
                "reuse_willingness",
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
            reuse_willingness: true,
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
                    reuse_willingness: true,
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
                    reuse_willingness: true,
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
    fn usage_record_without_reuse_field_still_reads() {
        with_env_root("usage_legacy", |root| {
            let path = root.join("logs/usage.jsonl");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            // 上一版本写入的记录没有 reuse_willingness 字段。
            let legacy = "{\"record_id\":\"20261018-070000-0000\",\"schema\":\"omnidesk.usage.v1\",\"task_type\":\"weekly_report\",\"recorded_at\":1792306800.0,\"completed\":true,\"usable\":true,\"rework_kind\":\"none\",\"blocker\":\"none\",\"duration_bucket\":null}\n";
            std::fs::write(&path, legacy).unwrap();

            let read = read_usage_records(&path).unwrap();
            assert_eq!(read.records.len(), 1);
            assert_eq!(read.unparsable_lines, 0);
            assert!(!read.records[0].reuse_willingness);

            // 旧记录仍可删除。
            assert_eq!(
                delete_usage_record(&path, "20261018-070000-0000").unwrap(),
                1
            );
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
                reuse_willingness: true,
            },
            0.0,
        )
        .unwrap();
        assert!(append_usage_record(&inside, &record).is_err());
        assert!(read_usage_records(&inside).is_err());
        assert!(clear_usage_records(&inside).is_err());
    }

    // --- 引擎工作目录准备 ---

    #[test]
    fn prepare_local_data_root_is_idempotent_and_outside_repo() {
        with_env_root("prepare", |root| {
            // 目录尚不存在时应被创建出来（引擎要求工作目录已存在且可解析）。
            let nested = root.join("engine-run");
            std::env::set_var(LOCAL_ROOT_ENV, &nested);
            assert!(!nested.exists(), "前置条件：目录应先不存在");

            let first = prepare_local_data_root().unwrap();
            assert_eq!(first, nested);
            assert!(first.is_dir(), "首次调用应创建目录");

            // 幂等：再次调用仍然成功且不报错、不改变路径。
            let second = prepare_local_data_root().unwrap();
            assert_eq!(second, first);
            assert!(second.is_dir());

            // 边界校验仍然生效：落点必须在仓库之外。
            assert_outside_repo(&first).unwrap();
        });
    }

    #[test]
    fn prepare_local_data_root_rejects_forbidden_root() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var_os(LOCAL_ROOT_ENV);
        // 指向原版 DSH 数据根下的目录：应在创建之前就被边界校验拒绝。
        std::env::set_var(LOCAL_ROOT_ENV, "/home/example/.dsh/engine-work");
        let outcome = prepare_local_data_root();
        match previous {
            Some(value) => std::env::set_var(LOCAL_ROOT_ENV, value),
            None => std::env::remove_var(LOCAL_ROOT_ENV),
        }
        assert!(
            outcome.is_err(),
            "禁止片段 / .dsh 应被拒绝，不得创建目录"
        );
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

    // --- 离线工作场景（周报 / 文案结构 / 项目跟进）---

    const WEEKLY_SAMPLE: &str = include_str!("../../../samples/workflows/synthetic-weekly.txt");
    const PROJECT_SAMPLE: &str = include_str!("../../../samples/workflows/synthetic-project.txt");

    #[test]
    fn weekly_sample_is_synthetic_and_summarizes_deterministically() {
        assert!(WEEKLY_SAMPLE.contains("纯虚构"));
        let summary = summarize_week(WEEKLY_SAMPLE).expect("合成周记录应可汇总");
        assert_eq!(summary.schema, WEEKLY_SCHEMA);
        assert_eq!(summary.total, 4);
        assert_eq!(summary.done, 2);
        assert_eq!(summary.pending, 1);
        assert_eq!(summary.blocked, 1);
        // 同一输入重复运行必须给出完全一致的结果。
        assert_eq!(summary, summarize_week(WEEKLY_SAMPLE).unwrap());
        let md = render_weekly_markdown(&summary);
        assert!(md.contains("受准入阻塞，未验证"));
        assert!(md.contains("本次未调用任何模型"));
        assert!(!md.contains("model_generated"));
    }

    #[test]
    fn weekly_rejects_bad_rows_with_line_numbers() {
        let err = summarize_week("2026-10-12 | 合成事项\n").unwrap_err();
        assert!(err.contains("第 1 行"), "{}", err);
        assert!(err.contains("三个字段"), "{}", err);
        let err = summarize_week("2026-13-01 | 合成事项 | done").unwrap_err();
        assert!(err.contains("YYYY-MM-DD"), "{}", err);
        let err = summarize_week("2026-10-12 | 合成事项 | finished").unwrap_err();
        assert!(err.contains("done / pending / blocked"), "{}", err);
        assert!(summarize_week("# 只有注释\n").unwrap_err().contains("没有可用事项"));
    }

    #[test]
    fn calendar_dates_follow_month_lengths_and_leap_years() {
        // 合法：闰日与非闰年的月末。
        assert!(parse_date("2024-02-29").is_some());
        assert!(parse_date("2026-02-28").is_some());
        assert!(parse_date("2024-01-31").is_some());
        assert!(parse_date("2026-12-31").is_some());
        // 非法：不存在的日期。
        for bad in [
            "2026-02-29",
            "2026-02-30",
            "2024-02-30",
            "2026-04-31",
            "2026-06-31",
            "2026-09-31",
            "2026-11-31",
            "2026-00-10",
            "2026-13-10",
            "2026-01-00",
            "2026-01-32",
        ] {
            assert!(parse_date(bad).is_none(), "应拒绝 {}", bad);
        }
        // 世纪闰年规则：2000 是闰年，1900 与 2100 不是。
        assert!(parse_date("2000-02-29").is_some());
        assert!(parse_date("1900-02-29").is_none());
        assert!(parse_date("2100-02-29").is_none());
        // 形式与字符约束。
        assert!(parse_date("2026-2-03").is_none());
        assert!(parse_date("2026-02-3a").is_none());
        assert!(parse_date("2026/02/03").is_none());
    }

    #[test]
    fn weekly_and_project_reject_calendar_impossible_dates() {
        let err = summarize_week("2026-02-30 | 合成事项 | done").unwrap_err();
        assert!(err.contains("第 1 行"), "{}", err);
        assert!(err.contains("YYYY-MM-DD"), "{}", err);
        // 合法闰日必须被接受。
        assert!(summarize_week("2024-02-29 | 合成事项 | done").is_ok());

        let row = "合成事项 | P1 | 2026-02-30 | in_progress | 补齐说明";
        let err = summarize_project(row, "2026-10-15").unwrap_err();
        assert!(err.contains("第 1 行"), "{}", err);
        assert!(err.contains("YYYY-MM-DD"), "{}", err);
        let row = "合成事项 | P1 | 2024-02-29 | in_progress | 补齐说明";
        assert!(summarize_project(row, "2026-10-15").is_ok());
        // 判定日同样要按日历校验。
        assert!(summarize_project(row, "2026-02-30").is_err());
        assert!(summarize_project(row, "2026-13-01").is_err());
    }

    #[test]
    fn overdue_boundary_and_cross_year_comparison_are_deterministic() {
        // 期限与判定日相同不算逾期；差一天即逾期。
        let same_day = "合成事项 | P1 | 2026-10-15 | in_progress | 继续";
        assert_eq!(summarize_project(same_day, "2026-10-15").unwrap().overdue, 0);
        let one_day_late = "合成事项 | P1 | 2026-10-14 | in_progress | 继续";
        assert_eq!(summarize_project(one_day_late, "2026-10-15").unwrap().overdue, 1);
        // 已完成事项即使期限已过也不计逾期。
        let done_late = "合成事项 | P1 | 2026-10-01 | done | 无";
        assert_eq!(summarize_project(done_late, "2026-10-15").unwrap().overdue, 0);
        // 跨年比较：期限在去年。
        let last_year = "合成事项 | P1 | 2025-12-31 | blocked | 说明";
        assert_eq!(summarize_project(last_year, "2026-01-01").unwrap().overdue, 1);
        // 跨年但未到期。
        let next_year = "合成事项 | P1 | 2027-01-02 | not_started | 启动";
        assert_eq!(summarize_project(next_year, "2026-12-31").unwrap().overdue, 0);
    }

    #[test]
    fn default_new_artifact_root_is_omnidesk_not_legacy_yvanai() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = temp_root("default-home");
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).expect("创建临时 HOME");
        let previous_local = std::env::var_os(LOCAL_ROOT_ENV);
        let previous_home = std::env::var_os("HOME");
        std::env::remove_var(LOCAL_ROOT_ENV);
        std::env::set_var("HOME", &home);
        let expected = home.join("OmniDesk/workspace/artifacts");
        let resolved = resolve_local_data_root();
        let legacy_exists = home.join("YvanAI").exists();
        match previous_local {
            Some(v) => std::env::set_var(LOCAL_ROOT_ENV, v),
            None => std::env::remove_var(LOCAL_ROOT_ENV),
        }
        match previous_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        let _ = fs::remove_dir_all(&home);

        let resolved = resolved.expect("默认本地数据根应可解析");
        assert_eq!(resolved, expected, "新成果默认落在 OmniDesk 之下：{}", resolved.display());
        // 不指向旧试点路径，解析过程也不会创建旧路径。
        assert!(!resolved.to_string_lossy().contains("YvanAI"));
        assert!(!legacy_exists, "解析默认路径不得创建旧路径");
    }

    #[test]
    fn writing_plan_uses_purpose_and_flags_missing_facts() {
        let plan = plan_writing("说明合成样例的处理流程", "项目内部评审人", "", "")
            .expect("缺失事实时仍应给出结构建议");
        assert_eq!(plan.schema, WRITING_SCHEMA);
        assert!(plan.template_used.is_none(), "没有匹配模板时不得声称套用模板");
        assert!(plan.missing.iter().any(|m| m.contains("可用事实")));
        assert!(plan.missing.iter().any(|m| m.contains("明确要求")));
        assert!(plan.sections[0].contains("待确认"));
        assert!(plan.sections.iter().any(|s| s.contains("项目内部评审人")));
        let md = render_writing_markdown(&plan);
        assert!(md.contains("未使用模板"));
        assert!(md.contains("受准入阻塞，未验证"));

        let err = plan_writing(" ", "评审人", "", "").unwrap_err();
        assert!(err.contains("缺少材料目的"));
        let err = plan_writing("说明流程", "", "", "").unwrap_err();
        assert!(err.contains("缺少受众"));
    }

    #[test]
    fn project_sample_flags_overdue_and_rejects_real_names() {
        assert!(PROJECT_SAMPLE.contains("纯虚构"));
        let summary = summarize_project(PROJECT_SAMPLE, "2026-10-15").expect("应可汇总");
        assert_eq!(summary.total, 4);
        assert_eq!(summary.done, 1);
        assert_eq!(summary.blocked, 1);
        // 逾期两条：10-10 in_progress、10-12 blocked（10-08 已完成不计）。
        assert_eq!(summary.overdue, 2);
        assert_eq!(summary, summarize_project(PROJECT_SAMPLE, "2026-10-15").unwrap());
        let md = render_project_markdown(&summary);
        assert!(md.contains("受准入阻塞，未验证"));

        let err = summarize_project("合成事项 | 张三 | 2026-10-10 | done | 无", "2026-10-15")
            .unwrap_err();
        assert!(err.contains("代号"), "{}", err);
        let err = summarize_project("合成事项 | P1 | 2026-10-10 | finished | 无", "2026-10-15")
            .unwrap_err();
        assert!(err.contains("not_started"), "{}", err);
    }

    #[test]
    fn flows_expose_model_step_as_blocked() {
        let weekly = weekly_flow(WEEKLY_SAMPLE);
        assert!(weekly.ok);
        assert!(weekly.model_step.contains("受准入阻塞"));
        let writing = writing_flow("说明流程", "评审人", "", "合成事实一条");
        assert!(writing.ok);
        assert!(writing.model_step.contains("未调用任何模型"));
        let project = project_flow(PROJECT_SAMPLE, Some("2026-10-15"));
        assert!(project.ok);
        assert!(project.model_step.contains("受准入阻塞"));
        // 空输入必须明确失败，而不是返回空成功。
        assert!(!weekly_flow("").ok);
        assert!(!project_flow("", Some("2026-10-15")).ok);
    }

    #[test]
    fn flow_pack_changes_copy_but_not_computation() {
        let mut pack = crate::flow_pack::default_pack();
        pack.weekly.result_title = "自定义周报标题".to_string();
        pack.weekly.details_heading = "自定义明细".to_string();
        pack.writing.result_title = "自定义文案标题".to_string();
        pack.project.result_title = "自定义项目标题".to_string();

        let base = weekly_flow(WEEKLY_SAMPLE);
        let themed = weekly_flow_with(WEEKLY_SAMPLE, &pack);
        assert!(themed.ok);
        assert_eq!(base.summary, themed.summary, "统计结果不得随文案变化");
        let md = themed.markdown.unwrap();
        assert!(md.contains("自定义周报标题"));
        assert!(md.contains("自定义明细"));
        assert!(md.contains("受准入阻塞，未验证"));
        assert!(md.contains("本次未调用任何模型"));

        let writing = writing_flow_with("说明流程", "评审人", "", "合成事实一条", &pack);
        assert!(writing.markdown.unwrap().contains("自定义文案标题"));

        let project = project_flow_with(PROJECT_SAMPLE, Some("2026-10-15"), &pack);
        assert_eq!(project.summary.as_ref().unwrap().overdue, 2);
        assert!(project.markdown.unwrap().contains("自定义项目标题"));
    }

    #[test]
    fn offline_save_requires_confirmation_and_never_overwrites() {
        with_env_root("offline-save", |root| {
            let markdown = render_weekly_markdown(&summarize_week(WEEKLY_SAMPLE).unwrap());
            let request = SaveOfflineRequest {
                scenario: "weekly_report".to_string(),
                title: "合成周报".to_string(),
                markdown: markdown.clone(),
                user_confirmed: true,
                ai_assisted: false,
            };
            let receipt = save_offline_artifact(&request, 1_760_572_800.0).expect("首次应成功");
            assert!(receipt.path.starts_with(&root.to_string_lossy().to_string()));
            let written = fs::read_to_string(&receipt.path).unwrap();
            assert!(written.contains("未调用模型"));
            assert!(written.contains("合成周报"));
            assert!(!written.contains("engine_pid"));

            let again = save_offline_artifact(&request, 1_760_572_800.0).unwrap_err();
            assert!(again.contains("不覆盖") || again.contains("已存在"));

            let mut unconfirmed = request.clone();
            unconfirmed.user_confirmed = false;
            let err = save_offline_artifact(&unconfirmed, 1_760_572_900.0).unwrap_err();
            assert!(err.contains("人工确认"));

            let mut unknown = request.clone();
            unknown.scenario = "case_review".to_string();
            assert!(save_offline_artifact(&unknown, 1_760_572_900.0).is_err());
        });
    }

    #[test]
    fn shared_writer_never_clobbers_an_existing_file() {
        with_env_root("shared-writer", |root| {
            let target = root.join("artifact.md");
            fs::write(&target, "既有内容").expect("准备既有文件");
            let err = write_new_file_durable(&target, "新内容").unwrap_err();
            assert!(err.contains("不覆盖"), "{}", err);
            assert_eq!(fs::read_to_string(&target).unwrap(), "既有内容");

            let fresh = root.join("fresh.md");
            write_new_file_durable(&fresh, "新内容").expect("新文件应写入成功");
            assert_eq!(fs::read_to_string(&fresh).unwrap(), "新内容");
        });
    }

    /// 写入失败：先写进一部分字节再报错，模拟磁盘写满/中断。
    fn ops_with_write_failure() -> NewFileOps {
        NewFileOps {
            write: |file, bytes| {
                file.write_all(&bytes[..bytes.len() / 2])?;
                Err(io::Error::new(io::ErrorKind::WriteZero, "injected write failure"))
            },
            ..NewFileOps::REAL
        }
    }

    /// 刷盘失败：内容已写进文件，但 sync_all 失败。
    fn ops_with_sync_failure() -> NewFileOps {
        NewFileOps {
            sync: |_file| Err(io::Error::new(io::ErrorKind::Other, "injected sync failure")),
            ..NewFileOps::REAL
        }
    }

    #[test]
    fn write_failure_removes_only_the_partial_file() {
        with_env_root("write-failure", |root| {
            let sibling = root.join("existing-artifact.md");
            fs::write(&sibling, "既有成果正文").expect("准备既有文件");
            let target = root.join("partial.md");

            let err = write_new_file_durable_with(&target, "半成品内容", &ops_with_write_failure())
                .unwrap_err();
            assert!(err.contains("写入产物失败"), "{}", err);
            assert!(err.contains("已删除本次未完成的半成品"), "{}", err);
            // 半成品必须消失，且错误信息里不能带出正文。
            assert!(!target.exists(), "半成品未被清理");
            assert!(!err.contains("半成品内容"), "错误信息泄露了正文：{}", err);
            // 同目录既有文件不受影响。
            assert_eq!(fs::read_to_string(&sibling).unwrap(), "既有成果正文");
            let left: Vec<_> = fs::read_dir(root)
                .unwrap()
                .filter_map(|e| e.ok())
                .map(|e| e.file_name())
                .collect();
            assert_eq!(left.len(), 1, "应只剩既有文件：{:?}", left);
        });
    }

    #[test]
    fn sync_failure_removes_the_written_file() {
        with_env_root("sync-failure", |root| {
            let target = root.join("unsynced.md");
            let err = write_new_file_durable_with(&target, "已写入但未刷盘", &ops_with_sync_failure())
                .unwrap_err();
            assert!(err.contains("已删除本次未完成的半成品"), "{}", err);
            assert!(!target.exists(), "刷盘失败后文件仍存在");
            assert!(!err.contains("已写入但未刷盘"), "错误信息泄露了正文：{}", err);
            assert_eq!(fs::read_dir(root).unwrap().count(), 0, "目录应被清理干净");
        });
    }

    #[test]
    fn cleanup_failure_is_reported_honestly_not_as_success() {
        let ops = NewFileOps {
            sync: |_file| Err(io::Error::new(io::ErrorKind::Other, "injected sync failure")),
            remove: |_path| Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected remove failure")),
            ..NewFileOps::REAL
        };
        with_env_root("cleanup-failure", |root| {
            let target = root.join("stuck.md");
            let err = write_new_file_durable_with(&target, "内容", &ops).unwrap_err();
            assert!(err.contains("清理半成品失败"), "{}", err);
            // 清理失败时文件会残留；报告必须承认，不能假装原子成功。
            assert!(target.exists(), "本用例假定半成品残留");
        });
    }

    #[test]
    fn failing_write_ops_cannot_be_reached_from_production_entry_points() {
        // 生产入口没有 ops 参数，产物保存函数只调用 REAL 版本；
        // 这里以编译期签名约束证明注入口不会从命令层传入。
        let _: fn(&Path, &str) -> Result<(), String> = write_new_file_durable;
        with_env_root("production-entry", |root| {
            let target = root.join("ok.md");
            write_new_file_durable(&target, "生产路径内容").expect("生产写入应成功");
            assert_eq!(fs::read_to_string(&target).unwrap(), "生产路径内容");
        });
    }

    #[test]
    fn offline_artifacts_list_and_reopen_roundtrip() {
        with_env_root("artifact-roundtrip", |root| {
            let weekly = SaveOfflineRequest {
                scenario: "weekly_report".to_string(),
                title: "合成周报".to_string(),
                markdown: "## 本周完成\n\n- 合成条目\n".to_string(),
                user_confirmed: true,
                ai_assisted: false,
            };
            let project = SaveOfflineRequest {
                scenario: "project_management".to_string(),
                title: "合成跟进".to_string(),
                markdown: "## 风险\n\n- 合成风险\n".to_string(),
                user_confirmed: true,
                ai_assisted: false,
            };
            let first = save_offline_artifact(&weekly, 1_760_572_800.0).expect("周报应保存");
            let second = save_offline_artifact(&project, 1_760_572_900.0).expect("跟进应保存");

            let listed = list_offline_artifacts().expect("应可列出成果");
            assert_eq!(listed.len(), 2, "只应列出本模块写入的两个成果");
            let names: Vec<&str> = listed.iter().map(|e| e.file_name.as_str()).collect();
            assert!(names.contains(&"offline-weekly-report-20251016-000000.md"));
            assert!(names.contains(&"offline-project-management-20251016-000140.md"));
            let scenarios: Vec<&str> = listed.iter().map(|e| e.scenario.as_str()).collect();
            assert!(scenarios.contains(&"weekly-report"));
            assert!(scenarios.contains(&"project-management"));
            for entry in &listed {
                assert!(entry.bytes > 0);
                assert!(entry.modified_at > 0.0);
            }

            let before = fs::read(&first.path).expect("读取保存前正文");
            let reopened = read_offline_artifact("offline-weekly-report-20251016-000000.md")
                .expect("应可重新打开");
            assert_eq!(reopened.file_name, "offline-weekly-report-20251016-000000.md");
            assert!(reopened.content.contains("合成周报"));
            assert!(reopened.content.contains("## 本周完成"));
            assert!(reopened.content.contains("未调用模型"));
            let after = fs::read(&first.path).expect("读取重开后的正文");
            assert_eq!(before, after, "重新打开必须是只读操作");
            let weekly_entry = listed
                .iter()
                .find(|e| e.file_name == reopened.file_name)
                .expect("周报应出现在列表中");
            assert_eq!(weekly_entry.bytes, after.len() as u64);
            assert!(Path::new(&second.path).starts_with(root));
        });
    }

    #[test]
    fn offline_artifact_list_ignores_unrelated_and_non_regular_files() {
        with_env_root("artifact-list-filter", |root| {
            let valid = SaveOfflineRequest {
                scenario: "material_writing".to_string(),
                title: "合成文案".to_string(),
                markdown: "正文\n".to_string(),
                user_confirmed: true,
                ai_assisted: false,
            };
            save_offline_artifact(&valid, 1_760_572_800.0).expect("合法成果应保存");
            // 文件名形状不符或场景未登记的文件不得进入列表。
            for name in [
                "notes.md",
                "offline-weekly-report.md",
                "offline-case-review-20251018-080000.md",
                "offline-weekly-report-20251018-080000.txt",
                "offline-weekly-report-2025-10-18.md",
            ] {
                fs::write(root.join(name), "不该被列出").expect("准备干扰文件");
            }
            // 与合法成果同名的目录也不得被当成成果。
            fs::create_dir_all(root.join("offline-weekly-report-20251018-090000.md"))
                .expect("准备同名目录");

            let listed = list_offline_artifacts().expect("应可列出成果");
            assert_eq!(listed.len(), 1, "只应保留一个合法成果");
            assert_eq!(listed[0].file_name, "offline-material-writing-20251016-000000.md");
            assert_eq!(listed[0].scenario, "material-writing");
        });
    }

    #[test]
    fn offline_artifact_read_rejects_unknown_names_and_oversize() {
        with_env_root("artifact-read-reject", |root| {
            let name = "offline-weekly-report-20251018-080000.md";
            fs::write(root.join(name), "合法正文").expect("准备合法文件");
            fs::write(root.join("notes.md"), "无关文件").expect("准备无关文件");

            for bad in [
                "../offline-weekly-report-20251018-080000.md",
                "/etc/passwd",
                "notes.md",
                "offline-case-review-20251018-080000.md",
                "offline-weekly-report-2025-10-18.md",
                "",
            ] {
                let err = read_offline_artifact(bad).unwrap_err();
                assert!(err.contains("只允许"), "{} 应被文件名校验拒绝：{}", bad, err);
            }

            let missing = read_offline_artifact("offline-weekly-report-20991231-235959.md").unwrap_err();
            assert!(missing.contains("无法读取"), "不存在的成果应报读取失败：{}", missing);

            fs::create_dir_all(root.join("offline-project-management-20251018-090000.md"))
                .expect("准备同名目录");
            let dir_err = read_offline_artifact("offline-project-management-20251018-090000.md").unwrap_err();
            assert!(dir_err.contains("普通文件"), "目录不得被当成成果：{}", dir_err);

            let big_name = "offline-weekly-report-20251018-100000.md";
            fs::write(root.join(big_name), vec![b'a'; 1024 * 1024 + 1]).expect("准备超大文件");
            let big_err = read_offline_artifact(big_name).unwrap_err();
            assert!(big_err.contains("1 MiB"), "超大文件应被拒绝：{}", big_err);

            let ok = read_offline_artifact(name).expect("合法成果应可读取");
            assert_eq!(ok.content, "合法正文");
        });
    }

    #[cfg(unix)]
    #[test]
    fn offline_artifact_symlink_is_not_listed_or_read() {
        with_env_root("artifact-symlink", |root| {
            let name = "offline-weekly-report-20251018-080000.md";
            let outside = root.parent().unwrap().join(format!(
                "omnidesk-symlink-target-{}-{}.md",
                std::process::id(),
                1_760_572_800u64
            ));
            let _ = fs::remove_file(&outside);
            fs::write(&outside, "数据根之外的合成正文").expect("准备外部文件");
            let link = root.join(name);
            match std::os::unix::fs::symlink(&outside, &link) {
                Ok(()) => {}
                Err(e) => {
                    let _ = fs::remove_file(&outside);
                    eprintln!("跳过符号链接用例：当前文件系统不支持创建符号链接（{}）", e);
                    return;
                }
            }

            let listed = list_offline_artifacts().expect("应可列出成果");
            assert!(listed.is_empty(), "符号链接不得出现在成果列表");
            let err = read_offline_artifact(name).unwrap_err();
            assert!(err.contains("普通文件"), "符号链接不得被读取：{}", err);
            let _ = fs::remove_file(&link);
            let _ = fs::remove_file(&outside);
        });
    }

    #[test]
    fn offline_save_default_goes_to_omnidesk_and_leaves_legacy_files_alone() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = temp_root("save-home");
        let _ = fs::remove_dir_all(&home);
        let legacy = home.join("YvanAI/projects/Omnidesk/analysis");
        fs::create_dir_all(&legacy).expect("创建旧路径");
        let legacy_file = legacy.join("existing-artifact.md");
        fs::write(&legacy_file, "旧成果正文").expect("写入旧成果");

        let previous_local = std::env::var_os(LOCAL_ROOT_ENV);
        let previous_home = std::env::var_os("HOME");
        std::env::remove_var(LOCAL_ROOT_ENV);
        std::env::set_var("HOME", &home);

        let request = SaveOfflineRequest {
            scenario: "weekly_report".to_string(),
            title: "合成周报".to_string(),
            markdown: render_weekly_markdown(&summarize_week(WEEKLY_SAMPLE).unwrap()),
            user_confirmed: true,
            ai_assisted: false,
        };
        let saved = save_offline_artifact(&request, 1_760_572_800.0);
        let legacy_after = fs::read_to_string(&legacy_file);
        let legacy_entries = fs::read_dir(&legacy)
            .map(|it| it.filter_map(|e| e.ok()).map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default();

        match previous_local {
            Some(v) => std::env::set_var(LOCAL_ROOT_ENV, v),
            None => std::env::remove_var(LOCAL_ROOT_ENV),
        }
        match previous_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        let receipt = saved.expect("默认位置应可保存");
        let expected_root = home.join("OmniDesk/workspace/artifacts");
        assert!(
            Path::new(&receipt.path).starts_with(&expected_root),
            "新成果应落在 {}，实际 {}",
            expected_root.display(),
            receipt.path
        );
        assert!(!Path::new(&receipt.path).starts_with(&legacy));
        assert_eq!(legacy_after.ok().as_deref(), Some("旧成果正文"), "旧文件正文不得被改写");
        assert_eq!(legacy_entries.len(), 1, "旧目录不得新增或删除文件");
        let _ = fs::remove_dir_all(&home);
    }
}

// ---------------------------------------------------------------------------
// 材料读取的边界测试（OD-ARCH-026）
//
// 证据等级：script（Rust 测试运行）。全部使用系统临时目录，不触碰用户数据或仓库内容。
// ---------------------------------------------------------------------------

#[cfg(test)]
mod material_tests {
    use super::*;

    fn material_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "omnidesk-material-{}-{}-{}",
            std::process::id(),
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_plain_text_material_with_provenance() {
        let dir = material_dir("ok");
        let file = dir.join("notes.md");
        fs::write(&file, "# 合成材料\n事项 A | done\n").unwrap();
        let read = read_material(file.to_str().unwrap()).expect("应读到材料");
        assert_eq!(read.extension, "md");
        assert_eq!(read.file_name, "notes.md");
        assert!(read.content.contains("合成材料"));
        assert_eq!(read.bytes, fs::metadata(&file).unwrap().len());
        assert_eq!(read.chars, read.content.chars().count());
        assert_eq!(read.sha256.len(), 64);
        assert_eq!(read.sha256, sha256_hex(read.content.as_bytes()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_relative_path_and_empty() {
        assert!(read_material("   ").is_err());
        let err = read_material("relative/notes.md").unwrap_err();
        assert!(err.contains("绝对路径"), "{}", err);
    }

    #[test]
    fn rejects_directory_and_disallowed_extension() {
        let dir = material_dir("dir_ext");
        let err = read_material(dir.to_str().unwrap()).unwrap_err();
        assert!(err.contains("不是普通文件"), "{}", err);

        let bin = dir.join("payload.bin");
        fs::write(&bin, b"PK\x03\x04").unwrap();
        let err = read_material(bin.to_str().unwrap()).unwrap_err();
        assert!(err.contains("不支持的材料类型"), "{}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_forbidden_directory_fragment() {
        let err = read_material("/home/someone/.dsh/notes.md").unwrap_err();
        assert!(err.contains("被禁止"), "{}", err);
    }

    #[test]
    fn rejects_missing_file_and_oversize() {
        let dir = material_dir("missing_big");
        let err = read_material(dir.join("nope.md").to_str().unwrap()).unwrap_err();
        assert!(err.contains("无法读取材料文件"), "{}", err);

        let big = dir.join("big.txt");
        fs::write(&big, vec![b'a'; (MATERIAL_MAX_BYTES as usize) + 16]).unwrap();
        let err = read_material(big.to_str().unwrap()).unwrap_err();
        assert!(err.contains("超过上限"), "{}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_non_utf8_material() {
        let dir = material_dir("utf8");
        let file = dir.join("raw.txt");
        fs::write(&file, [0xff, 0xfe, 0x00, 0x41]).unwrap();
        let err = read_material(file.to_str().unwrap()).unwrap_err();
        assert!(err.contains("UTF-8"), "{}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_material_inside_repo_with_reading_wording() {
        // 仓库内的文件（扩展名合法）也不得作为材料；提示必须是「读取」语义，
        // 不能复用产物写入路径的报错文案。
        let repo = repo_root_hint().expect("应能推断仓库根");
        let inside = repo.join("samples").join("workflows").join("offline-flows.v1.json");
        if !inside.is_file() {
            return;
        }
        let err = read_material(inside.to_str().unwrap()).unwrap_err();
        assert!(err.contains("拒绝读取"), "{}", err);
        assert!(err.contains("仓库内"), "{}", err);
        assert!(!err.contains("写入"), "{}", err);
    }
}
