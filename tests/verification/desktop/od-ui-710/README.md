# OD-UI-710 引擎事件层补齐：执行证据

日期：2026-11-11
基线 HEAD：149df677dba6ced26d522415f83de6bb00c4c295
基线工作区：git status --short --branch 仅输出 ## main...origin/main，无未提交/未跟踪改动。

## 验收命令（本轮实跑）

| 命令 | 结果文件 | 结果 |
| --- | --- | --- |
| /tmp/omnidesk-pytest-venv/bin/pytest tests -q | pytest.txt / pytest-exit.txt | 234 passed，35 subtests passed，exit 0 |
| python3 -m py_compile engine/omnidesk_engine/web_api_adapter.py | py_compile.txt / py_compile-exit.txt | exit 0 |

执行前基线为 227 passed、30 subtests passed；本轮净增 7 项测试、5 个 subtest。

## DSH 思考块字段依据

实读本机 DSH 运行时包（未修改 runtime/）：

- runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm/lib/typert.host.js 的 StreamChunk 声明包含 { type: 'reasoning-delta'; index: number; text: string; }。
- runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm/lib/index.js 的 reasoning-delta 分支读取 chunk.text 并归入 reasoning-chunks；text-delta 与 reasoning-delta 是并列的流块类型。
- runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-agent-loop/lib/index.js 的 AssistantStreamAttempt.push() 原样把上游 chunk 放入 assistant-stream 帧的 chunk 字段。

因此 Web API 适配层按 reasoning-delta + text 映射 thought_delta，并对含 reasoning / thought / thinking 语义的未知类型保持同一映射。

## 改动与边界

- 只改 engine/omnidesk_engine/web_api_adapter.py 与对应单元测试。
- 未知流块透出 PROGRESS 事件：{"kind": "unknown_chunk", "chunk_type": ...}，只带类型字符串。
- 思考流块透出 PROGRESS 事件：{"kind": "thought_delta", ...}，沿用 ACP 的展示口径，不计入最终结果。
- 工具事件新增 title 展示字段，取不到回退 tool_name；不读取 arguments / 结果原文。
- 事件序号由既有 task_core._emit 分配，EngineEvent.to_dict() 已包含 seq；本轮以单元测试核对。
- 未改协议、未改 ACP 通道、零新增模型调用、未改 runtime/、未提交或推送。

## 证据等级

- 文件事实：上游 DSH 类型声明与实现（静态阅读）。
- 脚本运行：pytest 全量 234 passed / 0 failed；py_compile exit 0。
- 真实模型输出：未涉及（本任务只验证事件翻译与载荷形状）。
