//! 任务外壳纵向流程的端到端测试（服务 desktop 的 task_* 命令所用同一批函数）。
//!
//! 覆盖 UI 实际走的五段骨架：
//!   输入 → 确定性统计 → 解释提示词 → 人工确认 → 保存到仓库外本地目录。
//!
//! 证据等级：script（Rust 测试运行）。这里**不**调用真实模型；解释阶段用
//! 手写文本代替，并如实把来源标为 user。模型是否遵守提示词约束属 model 级证据，
//! 需要真实模型输出才能判定，本文件不声称验证过。
//!
//! 全部用例集中在单个 #[test] 内：环境变量是进程级共享的，串行执行才能避免相互干扰。

use std::fs;
use std::path::{Path, PathBuf};

use omnidesk_desktop_lib::task_shell;

fn temp_root() -> PathBuf {
    std::env::temp_dir().join(format!("omnidesk-task-shell-e2e-{}", std::process::id()))
}

#[test]
fn vertical_slice_walks_all_five_stages_outside_repo() {
    let root = temp_root();
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("创建临时本地数据根");
    std::env::set_var("OMNIDESK_LOCAL_DATA_ROOT", &root);

    // --- 阶段 1：载入合成样例（纯虚构数据） ---
    let samples = task_shell::samples();
    assert!(!samples.is_empty(), "应至少有合成样例");
    let sample = &samples[0];
    assert!(
        task_shell::red_line_scan(sample.content).is_empty(),
        "合成样例不应命中红线扫描"
    );

    // --- 阶段 2：确定性统计（本地代码，不调用模型） ---
    let first = task_shell::deterministic_analysis(sample.content).expect("样例应可分析");
    let second = task_shell::deterministic_analysis(sample.content).expect("样例应可分析");
    assert_eq!(
        first.fingerprint, second.fingerprint,
        "统计必须可复现：两次运行指纹一致"
    );
    assert_eq!(first, second);

    // --- 阶段 3：生成解释提示词（含冻结统计块） ---
    let prompt = task_shell::build_explanation_prompt(
        "desensitized_analysis",
        sample.content,
        &first.fingerprint,
    )
    .expect("应能生成解释提示词");
    assert!(prompt.contains("冻结统计块"));
    assert!(prompt.contains(&first.fingerprint[..16]));

    // 提示词指纹与实际数据不符时必须拒绝，而不是照常给出。
    assert!(
        task_shell::build_explanation_prompt(
            "desensitized_analysis",
            sample.content,
            &"0".repeat(64),
        )
        .is_err(),
        "指纹不匹配应被拒绝"
    );

    // --- 阶段 4 + 5：人工确认后保存到仓库外 ---
    let request = task_shell::SaveArtifactRequest {
        task_type: "desensitized_analysis".to_string(),
        input_text: sample.content.to_string(),
        explanation_text: "（合成测试）按冻结统计块作描述性说明。".to_string(),
        explanation_source: "user".to_string(),
        user_confirmed: true,
        confirmation_note: "人工确认：合成测试".to_string(),
        fingerprint: first.fingerprint.clone(),
    };
    let now = 1_760_572_800.0;
    let receipt = task_shell::save_artifact(&request, now).expect("保存应成功");

    let saved = Path::new(&receipt.path);
    assert!(saved.starts_with(&root), "产物必须落在仓库外本地数据根");
    assert!(!saved.starts_with(env!("CARGO_MANIFEST_DIR")), "不得落进仓库");

    let body = fs::read_to_string(saved).expect("产物应可读");
    assert!(body.contains(task_shell::SHELL_VERSION));
    assert!(body.contains(&first.fingerprint));
    assert!(body.contains("未使用模板"));
    assert!(body.contains("未使用历史范例"), "必须如实说明未使用历史范例");
    assert!(body.contains(sample.content.trim()), "原始输入应原样保留");
    assert!(
        body.contains("人工确认: 已确认"),
        "产物应记录人工确认状态"
    );

    // 同参数再次保存必须被拒绝（不覆盖），且原有内容保持不变。
    assert!(
        task_shell::save_artifact(&request, now).is_err(),
        "已存在文件不得被覆盖"
    );
    assert_eq!(
        fs::read_to_string(saved).expect("产物仍可读"),
        body,
        "拒绝覆盖后原有产物内容不得改变"
    );

    // --- 使用记录：不含输入输出正文 ---
    let usage_path = root.join("logs/usage.jsonl");
    let record = task_shell::build_usage_record(
        &task_shell::UsageRecordInput {
            task_type: "desensitized_analysis".to_string(),
            completed: true,
            usable: true,
            rework_kind: "none".to_string(),
            blocker: "none".to_string(),
            duration_bucket: Some("5_15m".to_string()),
        },
        now,
    )
    .expect("应能构造使用记录");
    task_shell::append_usage_record(&usage_path, &record).expect("应能追加使用记录");

    let raw = fs::read_to_string(&usage_path).expect("使用记录应可读");
    assert!(!raw.contains(sample.content.trim()), "使用记录不得包含输入正文");
    assert!(
        !raw.contains("按冻结统计块作描述性说明"),
        "使用记录不得包含输出正文"
    );
    let view = task_shell::read_usage_records(&usage_path).expect("应能读取使用记录");
    assert_eq!(view.records.len(), 1);

    // --- 边界：仓库内不得写入 ---
    let inside_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/usage.jsonl");
    assert!(
        task_shell::append_usage_record(&inside_repo, &record).is_err(),
        "不得把使用记录写进仓库"
    );

    std::env::remove_var("OMNIDESK_LOCAL_DATA_ROOT");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn reserved_task_types_are_not_executable() {
    // reserved 类型既不能生成提示词，也不能保存。
    assert!(!task_shell::is_implemented("weekly_report"));
    assert!(task_shell::build_explanation_prompt("weekly_report", "类别,数值\n甲,1\n", "x").is_err());
}

#[test]
fn red_line_gate_and_confirmation_hold_at_the_shell_boundary() {
    let clean = task_shell::samples()[0].content.to_string();

    // R3：命中启发式闸门的输入不能产生统计结果，也不能产生解释提示词。
    let flagged = "类别,数值\n甲,110101199003077777\n";
    assert!(
        task_shell::analyze_input(flagged).is_err(),
        "命中闸门的输入不得产出统计结果"
    );
    assert!(
        task_shell::build_explanation_prompt("desensitized_analysis", flagged, &"0".repeat(64))
            .is_err(),
        "命中闸门的输入不得产出解释提示词"
    );

    // 干净输入的统计与指纹不回归。
    let via_gate = task_shell::analyze_input(&clean).expect("干净输入应可分析");
    assert_eq!(via_gate, task_shell::deterministic_analysis(&clean).unwrap());

    // R1/R2：未确认不得保存；model 来源已移除。
    let analysis = task_shell::deterministic_analysis(&clean).unwrap();
    let base = task_shell::SaveArtifactRequest {
        task_type: "desensitized_analysis".to_string(),
        input_text: clean.clone(),
        explanation_text: "（合成测试）描述性解释。".to_string(),
        explanation_source: "user".to_string(),
        user_confirmed: false,
        confirmation_note: "备注不能替代确认".to_string(),
        fingerprint: analysis.fingerprint.clone(),
    };
    assert!(
        task_shell::save_artifact(&base, 1_760_572_800.0).is_err(),
        "未确认不得保存"
    );

    let mut forged = base.clone();
    forged.user_confirmed = true;
    forged.explanation_source = "model".to_string();
    assert!(
        task_shell::save_artifact(&forged, 1_760_572_800.0).is_err(),
        "可伪造的 model 来源必须被拒绝"
    );
}
