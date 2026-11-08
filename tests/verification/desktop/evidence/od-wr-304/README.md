# OD-WR-304 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 起点库单测：614 passed（303/305 收口状态）
- 交付后库单测：632 passed / 0 failed（本任务新增 15 项）

## 改动文件

- 新增 engine/omnidesk_engine/task_extract.py：任务候选提示词组装（纯函数 + python -m CLI，不调模型）。
- 新增 desktop/src-tauri/src/task_extract.rs：宿主 validate_task_payload、解析落地与失败置状态。
- 修改 desktop/src-tauri/src/lib.rs：worklog_record_analyze / _apply / _failed 三个 IPC 与注册。
- 新增 tests/verification/engine-interface/adapter/test_task_extract_unit.py：5 项 Python 单测。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --lib | 0 | 632 passed / 0 failed |
| pytest tests -q | 0 | 184 passed，30 subtests passed |

原始输出：cargo-test-lib.log、pytest.log、exit-*.txt。

说明：系统自带的 python3 没有 pytest。本轮在 /tmp 建了临时 venv 并安装 pytest，再执行同一命令；未改动仓库依赖清单。

## 安全与失败行为断言

- 校验失败、暂存失败或模型失败（专用 IPC）都把 ai_analysis_status 置为 failed；正文与标题逐字节不动（单测断言）。
- source_quote 必须是记录正文的连续原文，改写或拼接的片段整条拒绝。
- due_date 只在正文出现同一字面日期时接受；模糊时间不得落精确日期。
- 已解析记录拒绝重复解析；failed 允许重试。
- 候选只进 .candidates 暂存，不进入正式任务清单。

## 已知限制

- 真实模型输出质量未验证；只验证了结构校验与假的合成数据。
- 数据脱敏使用既有 mask_privacy 口径；DataMasker 词典未接入本条链路，属未验证事项。
- 界面尚未消费这三个 IPC：解析按钮仍是 303 的诚实占位，等 OD-WR-306 接审核台时一并接线。
