# OD-ARCH-034 证据索引：真多轮接线——去掉上文重放

> 执行日：2026-10-20。计划：`PLANS.md` 的 OD-ARCH-034（用户批准执行）。
> 起因：D-024 复盘核实——ACP 本身支持同一会话内续发，我们此前每轮新建会话、靠前端重放上文模拟多轮。

## 1. 做了什么

| 层 | 改动 | 文件 |
| --- | --- | --- |
| 引擎适配层 | `SubmitTaskRequest` 新增 `engine_session_id`；`submit_task` 按三种方式取得会话：`new`（新建）/ `continued`（本进程内活会话，直接再发 prompt）/ `resumed`（不在本进程，先 `session/resume` 重开）/ `degraded`（续接失败，降级为新建并如实标注） | `engine/omnidesk_engine/acp_adapter.py` |
| 引擎协议入口 | `omni.submit_task` 透传 `engine_session_id` | `engine/omnidesk_engine/serve.py` |
| 宿主 | `engine_submit_task` 接受可选 `engine_session_id` 并透传（只透传，不解析、不猜会话归属） | `desktop/src-tauri/src/lib.rs` |
| 前端提示词 | `composeConversationPrompt` 增加 `continuation`：续接时**只发本轮输入，不重放上文** | `desktop/src/conversationPrompt.ts` |
| 前端线程 | 线程记录新增 `engineSessionId` 并持久化（本地存储，不进仓库）；新增 `setThreadEngineSession` | `desktop/src/sceneThreads.ts` |
| 前端对话 | `HomeChat` 持有/上报引擎会话号；续接时传 `engineSessionId`、不做重放；**降级时下一轮补一次重放**并显示说明 | `desktop/src/shell/HomeChat.tsx` |
| 调用方 | 场景页与「项目与会话」模块按线程传入/回写引擎会话号 | `shell/ScenePage.tsx`、`shell/Workspace.tsx` |

**任务台账**（`~/OmniDesk/app/engine-state/omni-*.json`）新增 `session_mode` 字段，
台账列表（`TaskRecordSummary`）也带这个字段 ⇒ 每轮是新建/续接/重开/降级**可查**。

## 2. 实施中发现的关键事实（与计划措辞有出入，已按事实修正）

计划原文写的是「后续轮次经 `session/resume` + `session/prompt` 续发」。实测 DSH 的
`dsh-acp/lib/index.js`：`session/resume` **只接受已落盘、可重开的会话**
（`persistence.stat(sessionId)` 取不到就报 `session is not resumable`）；
对**当前进程里还活着**的会话调用它会返回 `invalid_argument`。

因此正确的接法是两条路：

- **同一进程内**（会话还活着）→ 直接再发一次 `session/prompt`，**不能**先 resume（`continued`）；
- **进程重启后**（会话已落盘）→ `session/resume` 重开，再 `session/prompt`（`resumed`）。

两种都失败/不适用时降级为新建会话（`degraded`），回执与台账如实标注。

## 3. 验证

### 3.1 单元级（桩替身，无引擎进程、无网络、无凭证）
`tests/verification/desktop/verify_multiturn_unit.py` → **27/27 通过**
覆盖：新建分支 / 活会话续发分支（不调 resume）/ resume 分支 / 降级分支，
以及回执与台账的 `session_mode`。

### 3.2 端到端（真实锁定版 DSH 引擎 + 本机回环合成端点）
`tests/verification/desktop/verify_true_multiturn.py` → **33/33 通过**
证据文件：`true-multiturn.json`

| 断言 | 结果 |
| --- | --- |
| 第一轮 `session/new` | 通过 |
| 第二轮：同一会话续发、**不新建会话**、**不调 resume**（活会话） | 通过 |
| **引擎侧第 2 次请求的消息条数 3 → 5** ⇒ 引擎带着上文继续 | 通过 |
| 第二轮发往适配层的提示词**只有本轮输入**（没有重放第一轮） | 通过 |
| 台账 `session_mode` = new / continued | 通过 |
| **适配层重启后**第三轮走 `session/resume`，不新建会话 | 通过 |
| **重启后历史仍在**：消息条数 5 → 7 | 通过 |
| 会话号失效 → 降级为新建会话、不抛错、任务照常成功 | 通过 |

### 3.3 提示词组装（真实 TS 源码）
`tests/verification/desktop/verify_conversation_prompt.py` → **24/24 通过**（原 15 + 新增 9）
新增：续接时普通对话与带预设两条路径都不带上文、仍保留场景规则与不可关闭的写作规则。

### 3.4 真实窗口（正式实例，真实端点）
`shots/od034-ui-turn1.png`、`shots/od034-ui-turn2.png`（连发两轮的界面状态）。
台账实证（`~/OmniDesk/app/engine-state/`）：

```
23:34:28  omni-689d77ec  mode=new       session=e88c3e41
23:35:11  omni-dd31e482  mode=continued  session=e88c3e41   ← 同一会话
23:36:32  omni-f57032b3  mode=continued  session=e88c3e41   ← 同一会话
```

**限制：** XTEST 往输入框打的字符会丢/乱序（每键重渲染），所以界面里的**内容**是乱码，
**上下文是否延续**由引擎台账与合成端点证据判定，不靠这段乱码文本。

### 3.5 回归
Rust **143/143**、`tsc` 0、`npm run build` 通过、`verify_ai_draft` 27/0、`verify_scenario_materials` 5/0。

## 4. 已知边界（未变 / 新增）

- **ACP 仍没有 token 级流式**（源码级事实未变）：思考与正文在提交时同时到达；
  界面仍是逐字呈现。本计划只解决「上下文归属」，不解决「增量到达」。
- **首页对话没有线程记录**（既有缺口，未变）：首页不落线程，**同一轮次运行内**会话连续 ✓，
  但应用重启后首页会新开一个会话并重放可见上文。场景页/模块页的线程**会**持久化引擎会话号，
  重启后走 `session/resume` 续接 ✓（合成端点已验证该路径 ✓）。
- **续接失败（degraded）那一轮本身丢上下文**：此时才降级为新会话；界面已如实提示，
  且**下一轮**会自动补一次窗口化重放把历史接回去。
