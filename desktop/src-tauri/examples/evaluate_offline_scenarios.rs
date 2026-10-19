//! OD-ARCH-010 证据生成器：三个离线合成场景的确定性输出。
//!
//! 运行（在 desktop/src-tauri 下）：
//!   cargo run --offline --example evaluate_offline_scenarios
//!
//! 只读取仓库内合成样例，不调用模型、不接触用户数据、不写任何文件。

use omnidesk_desktop_lib::{settings, task_shell};

const WEEKLY: &str = include_str!("../../../samples/workflows/synthetic-weekly.txt");
const PROJECT: &str = include_str!("../../../samples/workflows/synthetic-project.txt");

fn main() {
    println!("== 场景 1：周记录 → 周报（本地确定性） ==");
    match task_shell::weekly_flow(WEEKLY) {
        r if r.ok => {
            let s = r.summary.as_ref().expect("summary");
            println!(
                "判定: 通过 | 事项 {} / done {} / pending {} / blocked {}",
                s.total, s.done, s.pending, s.blocked
            );
            println!("模型步骤: {}", r.model_step);
        }
        r => println!("判定: 失败 | {}", r.error.unwrap_or_default()),
    }

    println!("\n== 场景 2：一般文案结构建议 ==");
    // 2a：缺事实 → 不得编造，标待确认；2b：有事实 → 给出结构。
    for (label, facts) in [("缺事实", ""), ("有合成事实", "合成样例共四条事项，均为虚构。")] {
        let r = task_shell::writing_flow("说明合成流程", "项目评审人", "三条以内", facts);
        let plan = r.plan.as_ref().expect("plan");
        println!(
            "{}: 判定通过 | 模板={:?} | 待确认 {} 项 | 首节={}",
            label,
            plan.template_used,
            plan.missing.len(),
            plan.sections.first().cloned().unwrap_or_default()
        );
    }

    println!("\n== 场景 3：项目跟进汇总（today=2026-10-15） ==");
    let r = task_shell::project_flow(PROJECT, Some("2026-10-15"));
    let s = r.summary.as_ref().expect("summary");
    println!(
        "判定: 通过 | 事项 {} / done {} / in_progress {} / blocked {} / not_started {} / overdue {}",
        s.total, s.done, s.in_progress, s.blocked, s.not_started, s.overdue
    );
    println!("模型步骤: {}", r.model_step);

    println!("\n== 模型准入（只读设置，不调用模型） ==");
    match settings::app_config_dir().and_then(|dir| settings::load(&dir)) {
        Ok(s) => println!(
            "权限 local={} private={} public={}（未知类别恒拒绝）",
            s.model_permissions.local, s.model_permissions.private, s.model_permissions.public
        ),
        Err(e) => println!("无法读取设置（将按默认全关处理）: {}", e),
    }
    println!("端点分类在宿主内按实际配置执行；本示例未启动引擎、未发送任何输入。");

    println!("\n== 日期边界（OD-ARCH-011） ==");
    let weekly_bad = task_shell::weekly_flow("2026-02-30 | 合成事项 | done");
    println!(
        "周记录 2026-02-30: ok={} | {}",
        weekly_bad.ok,
        weekly_bad.error.unwrap_or_default()
    );
    let weekly_leap = task_shell::weekly_flow("2024-02-29 | 合成事项 | done");
    println!("周记录 2024-02-29（闰日）: ok={}", weekly_leap.ok);
    let row = "合成事项 | P1 | 2026-10-14 | in_progress | 继续";
    for today in ["2026-10-14", "2026-10-15", "2026-02-30"] {
        match task_shell::project_flow(row, Some(today)) {
            r if r.ok => println!(
                "逾期判定 today={}: overdue={}",
                today,
                r.summary.as_ref().map(|s| s.overdue).unwrap_or(0)
            ),
            r => println!("逾期判定 today={}: 拒绝 | {}", today, r.error.unwrap_or_default()),
        }
    }
    match task_shell::resolve_local_data_root() {
        Ok(root) => println!("新成果默认根: {}", root.display()),
        Err(e) => println!("新成果默认根: 无法解析 | {}", e),
    }
}
