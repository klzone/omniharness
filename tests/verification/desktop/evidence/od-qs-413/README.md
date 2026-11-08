# OD-QS-413 全量回归与安全核对证据

日期：2026-11-08

基线：`main` @ `38d839ce2d87887a59c1dec41c141f9a08dee8f2`，工作区含 401–412 的未提交改动与规划会话文档改动。未提交、未推送。

## 实跑命令

| 命令 | 结果 |
| --- | --- |
| `cargo test --offline --tests` | exit 0；13 个测试目标全部 `test result: ok`，0 FAILED；lib 770 passed / 0 failed |
| `pytest tests -q` | exit 0；187 passed，30 subtests passed |
| `npm --prefix desktop run build` | exit 0；构建成功（仅既有 chunk 体积提示） |
| `npx tsc --noEmit` | exit 0；无诊断 |

## 专项 verify

| 脚本 | 结果 |
| --- | --- |
| `verify_priority_rules_401.py` | 11/11 |
| `verify_task_quadrant_ui_402.py` | 11/11 |
| `verify_schedule_ui_403.py` | 11/11 |
| `verify_security_phase2.py` | 14/14（JSON 见同目录 `security_phase2.json`） |
| `verify_work_record_ui_301.py` | 15/15 |
| `verify_task_manage_ui_302.py` | 15/15 |
| `verify_task_review_ui_303.py` | 16/16 |
| `verify_task_linkage_304.py` | 18/18 |
| `verify_worklog_unify_305.py` | 15/15 |
| `verify_week_from_records_306.py` | 12/12（断言按 410 的占位替换更新） |
| `verify_work_cleanup_307.py` | 15/15 |
| `verify_wr_security_308.py` | 13/13 |

## 安全核对结论

- 重要性 / 象限 / 截止三个带自由文本的优先级 IPC 都经过 `gate_user_text`（红线扫描 + mask_privacy）。
- 耗时与依赖 IPC 只收 `Option<u32>` 与任务编号数组，没有自由文本通道；依赖仍做存在性、无环与不自依赖校验。
- AI 提取链路（`task_extract.py` / `task_extract.rs`）不引用排期模块，不能创建或确认排期。
- 排期生成/写入只出现在 `schedule_generate`、`schedule_adjust` 两条用户触发路径；启动流程不自动生成。
- `scheduling.rs` 无文件系统与进程调用；`schedule_store.rs` 生产代码不改任务文件。
- 新模块与任务界面没有调试日志；新增测试只写临时目录。

## 未验证事项

- 真实模型产出的 importance 建议质量（本阶段未调用真实模型）。
- 真实窗口下的四象限、今日/本周页面与 neumorphism 主题观感（未做目视验收）。
- 大量任务（数百条）下的排期生成与页面渲染性能。
- 真实用户排期冲突的处理体验（仅验证了规则与界面静态断言）。
