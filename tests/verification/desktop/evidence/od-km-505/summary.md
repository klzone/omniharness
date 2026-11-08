# OD-KM-505 知识推荐检索器实跑证据

- 日期：2026-11-09
- 基线 HEAD：836c230fb2e7c4a2c6057b1922948f68add0e7f7（main）
- 开工前工作区：PLANS.md、REVIEW.md、lib.rs、task_store.rs、work_cleanup.rs、
  worklog_records.rs 等已有未提交改动；本卡保留原样，未重置、未提交、未推送。
- 验收命令：

    export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && cargo test --offline --lib

- 本轮结果：856 passed; 0 failed; 0 ignored（退出码 0）。总数含并行任务在同一工作区新增的测试。
- 本卡 domain：knowledge_link::tests 14 项全部通过（输出全文见 cargo-test-lib.txt）。
- 证据等级：脚本运行（cargo test 实际输出）。
- 未新增 lint/测试豁免；未修改 runtime/、knowledge_vault.rs、vault_search.rs。
