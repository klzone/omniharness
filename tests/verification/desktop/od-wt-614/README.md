# OD-WT-614 归档与知识回流证据

日期：2026-11-09。测试数据均为虚构；未读取用户知识库或真实业务材料。

| 证据 | 文件 | 命令 | 结果 |
| --- | --- | --- | --- |
| 宿主单测 | `cargo-test-lib.log` | `cargo test --offline --lib` | 1082 passed / 0 failed；614 新增 12 项 |
| 前端类型 | `tsc.log` | `npx tsc --noEmit` | exit=0 |
| 前端构建 | `build.log` | `npm --prefix . run build` | exit=0 |
| 专项验证 | `verify_writing_archive_614.log` / `.json` | `python3 tests/verification/desktop/verify_writing_archive_614.py` | 23/23 checks passed |
| 定向回归 | `verify_writing_archive_614_cargo.log` | 同上加 `--cargo-tests` | 23/23；614 Rust 用例 12 passed / 0 failed |

证据等级：cargo 与专项脚本为脚本运行；tsc/build 为退出码；UI 未做真实窗口目视。

场景十：归档回执含 writing_id / title / selected_material_ids / selected_knowledge_ids /
八类 factcheck 计数 / version / 导出清单 / vault 路径，任务进入已归档终态。

场景十一：知识回流只接收用户勾选的 selections，只入既有 raw 收件待审队列；
未勾选内容不进入任何治理通道，不自动建卡、裁决或定稿。
