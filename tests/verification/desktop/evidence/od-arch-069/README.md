# OD-ARCH-069 证据摘要（工具可见化 + 超时治理）

## 场景结果（证据等级：脚本运行 + 真实 DSH web 进程 + 本机合成端点）

脚本：`tests/verification/engine-interface/adapter/run_visibility_scenarios.py`
原始归一化记录：`visibility-scenarios.json`

| 场景 | 判定 | 关键证据 |
| --- | --- | --- |
| V01 工具名可见 | 通过 | 收到 `tool_activity`，`tool_name=bash`（合成端点指定的工具），终态 `succeeded` |
| V02 超时可配 | 通过 | `total_seconds=8`，8.2 秒落 `unknown`，note 含「8 秒」「未自动重试」「重发」 |
| V03 未知询问只提示 | 通过 | 收到 `engine_ask`（event 原文保留），未发送任何应答帧，任务继续等待直到超时 |
| V04 台账边界 | 通过 | 台账存在，未出现合成命令行与授权理由文本 |

## 本轮修复的两个实现缺陷

1. **心跳续期导致总超时失效。** DSH 每 2 秒发一次 WebSocket ping，旧实现给每一小段读取都重设 socket 超时，心跳不断续期，8 秒总超时永远到不了（修复前实测 60 秒仍 `running`，最终以「事件流异常中断」落定）。现改为按每次 `recv` 的总截止时间重算剩余预算。回归用例：`test_recv_budget_survives_heartbeat_frames`。
2. **超时文案未进入任务状态。** `_settle` 的 `note` 只写事件，`get_task_status` 由 `stop_reason`/`error_message` 组成，界面拿到空串。现于超时分支同时写入 `error_code`/`error_message`，状态 note 可见。

## 限制

- 上述均为合成端点场景，未跑真实模型写作任务。
- 界面渲染由用户在实际窗口中确认；本轮只做同 PID 可访问性树检查（49 节点，含「系统偏好设置 / 引擎技术切片」），未做肉眼截图（Wayland 锁屏下截图为空）。
- 「引擎发出未知确认」在生产中是否出现，取决于上游事件类型；本场景用合成帧注入验证，不代表已观测到真实发生。
