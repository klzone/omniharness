# OD-WR-312 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：d8486784c8df94b17f45f8bdabdb7c7f58e39f70（main，311 前）
- 起点库单测：660 passed
- 交付后库单测：666 passed / 0 failed（本任务新增 6 项）

## 改动文件

- 新增 desktop/src-tauri/src/vault_archive.rs：报告与成果归档、frontmatter 合并、同名幂等/冲突判断、6 项单测。
- desktop/src-tauri/src/lib.rs：archive_report_to_vault / archive_artifact_to_vault IPC 与注册。
- desktop/src-tauri/src/task_shell.rs：parse_writing_frontmatter 改为 crate 内可见，供归档判断发布态。
- desktop/src/api.ts、desktop/src/types.ts：归档回执与客户端方法。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --lib | 0 | 666 passed / 0 failed |
| npx tsc --noEmit | 0 | 无输出 |

原始输出：cargo-test-lib.log、tsc.log、exit-*.txt；库测试无 warning。

## 行为

- 报告：从 library_root/周报/<file_name> 读取，复制到 <vault_root>/报告/<file_name>；frontmatter 写 source_type=report、source_file、source_period、source_sha256、archived_at。
- 成果：只接受文案写作场景且 status=published 的成果；复制到 <vault_root>/归档成果/<file_name>；frontmatter 写 source_type=artifact、source_file、source_sha256、archived_at。
- 幂等：目标已存在且 source_sha256 相同 → 成功返回 already_archived=true，不重写；source_sha256 不同 → 拒绝覆盖。
- 安全：文件名拒绝路径分隔、隐藏名、非 .md；归档子目录解析真实落点必须在知识库根内；目标是符号链接时拒绝。

## 已知限制

- 本轮只交付 IPC 与客户端方法，没有界面按钮；实际归档动作由用户后续显式触发。
- status=finished（成文未发布）也会被拒；只放行 published。
- 报告来源周期可从周报名推断；月报/年报/项目总结需调用方显式传 period。
