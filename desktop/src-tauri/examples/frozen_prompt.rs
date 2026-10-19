//! 打印内置合成样例的「冻结解释提示词」与其工作目录，供桌面引擎路径验证脚本取用。
//!
//! 存在理由：阶段 3 的提示词只能由本地 Rust 代码（`task_shell::build_explanation_prompt`）
//! 产出（本地重算 + 指纹比对）。验证脚本若在 Python 里自行拼一份提示词，就等于绕过了这条
//! 唯一数值来源，测出来的不是产物。本示例把 Rust 真实产出的提示词原样交给脚本。
//!
//! 边界：只读取编译期嵌入的合成样例与解释规则；不联网、不启动引擎、不读凭证。
//! 工作目录来自 `task_shell::prepare_local_data_root`，与界面阶段 5 用的是同一个函数，
//! 受同一套「禁止片段 / 不得位于仓库内」校验约束；目录由环境变量 `OMNIDESK_LOCAL_DATA_ROOT` 指定。
//!
//! 两种用法：
//!   1) 默认：打印冻结提示词与工作目录的 JSON（前四段取值）。
//!   2) 设置 `OMNIDESK_SAVE_EXPLANATION`：把该文本当作**阶段 3 的回填结果**执行一次
//!      `task_shell::save_artifact`（阶段 4/5 的真实保存入口，强制人工确认与指纹比对），
//!      打印保存回执。这样「引擎产出 → 人工确认 → 保存」由同一份 Rust 代码串起来。

use omnidesk_desktop_lib::task_shell;

fn main() {
    let samples = task_shell::samples();
    let sample = samples.first().expect("应至少有一个合成样例");

    let analysis = task_shell::deterministic_analysis(sample.content).expect("合成样例应可分析");
    let prompt = task_shell::build_explanation_prompt(
        "desensitized_analysis",
        sample.content,
        &analysis.fingerprint,
    )
    .expect("应能生成冻结解释提示词");

    let workspace = task_shell::prepare_local_data_root();

    // 可选：用传入的解释文本走一遍真实保存入口（阶段 4 + 5）。
    let save = match std::env::var("OMNIDESK_SAVE_EXPLANATION") {
        Ok(text) if !text.is_empty() => {
            let request = task_shell::SaveArtifactRequest {
                task_type: "desensitized_analysis".to_string(),
                input_text: sample.content.to_string(),
                explanation_text: text,
                // 来源固定 model_unverified：界面在无法证明引擎终态时不允许声明「已复核」。
                explanation_source: "model_unverified".to_string(),
                user_confirmed: true,
                confirmation_note: "人工确认：回环合成端点验证".to_string(),
                fingerprint: analysis.fingerprint.clone(),
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            match task_shell::save_artifact(&request, now) {
                Ok(receipt) => serde_json::json!({
                    "ok": true,
                    "path": receipt.path,
                    "fingerprint": receipt.fingerprint,
                    "error": serde_json::Value::Null,
                }),
                Err(message) => serde_json::json!({
                    "ok": false,
                    "path": serde_json::Value::Null,
                    "fingerprint": serde_json::Value::Null,
                    "error": message,
                }),
            }
        }
        _ => serde_json::Value::Null,
    };

    let out = serde_json::json!({
        "sample_id": sample.sample_id,
        "input_text": sample.content,
        "fingerprint": analysis.fingerprint,
        "rows": analysis.rows,
        "total": analysis.total,
        "mean": analysis.mean,
        "top_category": analysis.top_category,
        "prompt": prompt,
        "workspace_dir": match &workspace {
            Ok(path) => serde_json::Value::String(path.display().to_string()),
            Err(_) => serde_json::Value::Null,
        },
        "workspace_error": match &workspace {
            Ok(_) => serde_json::Value::Null,
            Err(message) => serde_json::Value::String(message.clone()),
        },
        "save": save,
    });
    println!("{}", out);
}
