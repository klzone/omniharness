# OD-ARCH-088 派工指令：场景提问卡闭环 + 场景 intent 透传 + Skill 绑定可见性

> 依据 `docs/BUILDER-Task-Instruction-Template.md` 撰写。计划正文以 `PLANS.md` 的 OD-ARCH-088 小节为唯一实施口径；本文只把范围、顺序、证据与禁止项说清楚，不放宽任何一条。

## 获批依据

- 计划位置：`PLANS.md` → OD-ARCH-088
- 批准时间与批准人：2026-10-25，用户明确批复「批准 OD-ARCH-088」
- 开工基线：分支 `main`，HEAD `0d17b1f`。**工作区存在大量既有未提交/未跟踪改动（084/085/086 遗留 + OD-ARCH-087 的 Cargo.toml 与 scripts/omnidesk.sh），一律原样保留，不得回退、清理、暂存或提交。**
- 允许修改：`engine/omnidesk_engine/web_api_adapter.py`、`engine/omnidesk_engine/serve.py`、`desktop/src-tauri/src/engine_bridge.rs`、`desktop/src-tauri/src/lib.rs`、`desktop/src/api.ts`、`desktop/src/types.ts`、`desktop/src/shell/HomeChat.tsx`、`desktop/src/shell/HomeCanvas.tsx`、`desktop/src/SettingsPanel.tsx`、`desktop/src/theme.css` 或 `desktop/src/markdown.css`（只加卡片样式）、新增 `desktop/src/shell/QuestionCard.tsx`（文件名可自定）、`tests/` 下新增验证脚本与 `tests/verification/desktop/evidence/od-arch-088/`、`STATUS.md`
- 明确不做：不改 `runtime/`（DSH 引擎一行都不动）；不动模型路由、凭证、数据根；不改 `AGENTS.md`；不重做顶栏/导航/配色；不引入新依赖；不做通用表单引擎或问卷系统；不自动替用户回答任何提问；不静默迁移用户规则包；不提交、不推送、不打标签。

## 开工前检查（只读）

1. `git rev-parse HEAD` 与 `git status --porcelain` 快照，写入 STATUS.md；确认既有改动一项不少。
2. 只读核对：
   - `engine/omnidesk_engine/web_api_adapter.py:356-380`（waterfall 分发：非 approval 一律 `_on_engine_ask`）、`:1739-1770`（`_on_engine_ask` 只上报不应答）、`:1830-1900`（`respond_approval` 的 `$events/result` 调用形态）、`_ApprovalChannel.ensure_started()`（连接与生命周期复用点）。
   - `runtime/deepseek-harness/packages/interaction/user-questions/src/types.ts:37-72`（题目与答案结构）、`:139-175`（request 事件声明）、`projection.ts:33-60`（**线上题目字段是 snake_case `multi_select`，投影后才是 `multiSelect`**）、`:105-120`（答案批次 schema）。
   - `engine/omnidesk_engine/serve.py:600-620`（DISPATCH 表）、`:449-470`（`m_approval_respond`）。
   - `desktop/src-tauri/src/engine_bridge.rs:265-290`、`:665-680`（`lib.rs` 命令注册区）。
   - `desktop/src/api.ts:253-266`、`desktop/src/shell/HomeChat.tsx:755-800`（engine_ask 渲染）、`:890-940`（审批回传口径）、`:1800-1830`（审批卡片渲染）、`:1150-1200`（`submitTask` 调用点）、`desktop/src/shell/HomeCanvas.tsx:296-320`（skill_bindings 注入）、`desktop/src/SettingsPanel.tsx:1636-1700`（规则资产分页）。

## UI 参考（涉及界面，必填）

- 检索关键词：`clarifying question option chips inline chat`、`chat assistant suggestion chips quick replies`、`approval prompt options card`
- 参考产品：ChatGPT（https://screenshots.designknowledgebase.com/Web/ChatGPT/ ）、Claude（https://screenshots.designknowledgebase.com/Web/Claude/ ）、Catalant（https://screenshots.designknowledgebase.com/Web/Catalant/ ，含 Approve Extension Proposal 审批页）
- 借鉴要点（≤2 句）：提问卡直接长在对话流里、紧贴触发它的那条助手消息，不做居中模态；选项做成整行可点的列表项（标题 + 一行灰色说明），选中只用一个细边框和淡底，和现有审批卡共用同一套圆角、间距与字号刻度。

## 实施要求

### S1 适配层：解析提问、登记待答、回传答案（Python）

1. `_on_engine_ask` 拆分：`event == "user-questions/request"` 且 `request.questions` 可解析时走新的 `_on_question_request`，其余类型维持现有 `engine_ask` 提示（不得回归成静默）。
2. 新增提问登记表（与 `_approval_index` 并列），按 `event_id` 存：归属任务、`agent_id`、`call_id`（`request.wait.callId`，可能缺省）、题目数组。解析时**同时接受 `multi_select`（线上）与 `multiSelect`**。
3. 下发 `PROGRESS kind="question_request"`，payload 含 `question_id`（用 `eventId`）、`questions`（id / header / question / detail / options[{label,description}] / multi_select）、`note`。**payload 只进内存事件流，不进 `_record_payload`**——任务状态文件仍只存类型、id 与计数（可加 `pending_questions` 计数），绝不写题干与选项正文。
4. 新增 `respond_question(omni_task_id, question_id, answers)`：校验任务非终态、`question_id` 确属本任务待答；校验答案覆盖每个题目的 id 恰好一次、`selected` 取值必须是该题给出的选项标签（除 `custom` 外）、`multi_select=false` 时 `selected` 至多一项；超限或非法一律抛 `INVALID_ARGUMENT` 并保留待答项，**不静默修正**。
5. 通过既有 `_ApprovalChannel`（同一客户端 id、同一连接）发 `$events/result`，`outcome = {"kind":"result","value":{"answers":[{"id","selected","custom"?}]}}`。发送失败保留待答项并如实报错；成功后出队并发 `PROGRESS kind="question_answered"`（含 `question_id`，不回显答案正文）。
6. 清理：收到同 `event_id` 的 `cancel` 帧、或任务进入终态时，移除待答项并发 `PROGRESS kind="question_closed"`（带 `reason`：`cancelled` / `task_terminal`）。等待超时由引擎侧处理，外壳**不得**自行伪造答案。
7. 上限：题目 ≤ 8、题干与 detail ≤ 2000 字符、选项 ≤ 12 且标签 ≤ 200 字符、自定义答案 ≤ 4000 字符；超限登记但仍如实标注不可答并在回答时报错（不要静默截断或丢弃）。

### S2 传输链路

1. `serve.py`：新增 `m_question_answer` 并登记 `omni.question_answer`；缺 `respond_question` 的适配器（如 ACP）返回 `METHOD_NOT_FOUND`，与审批同口径。
2. `engine_bridge.rs`：新增 `question_answer(omni_task_id, question_id, answers: serde_json::Value)`，超时 30s。
3. `lib.rs`：新增 `engine_question_answer` 命令并注册；错误沿用现有 `engine_error` 归一化。
4. `api.ts` / `types.ts`：`questionAnswer()` 与类型；答案形状与 Python 侧一致。

### S3 前端提问卡

1. 新组件渲染题目（可多题）：header（可选）、题干、detail（可选）、选项列表（整行可点，单选为 radio 语义、多选为 checkbox 语义）、可选自定义输入、提交按钮、`取消任务` 出口。
2. 单题且 `multi_select=false` 时，点选项即选中但不自动提交；必须由用户点提交才回传（**不自动应答**）。
3. 状态机：`pending` → `submitting` → `answered` / `failed(reason)` / `closed(reason)`。只有宿主确认送达才变 `answered`；失败保留可重试并显示真实原因；超时或取消落成 `closed` 并禁用交互。
4. 卡片挂在触发它的那条助手消息下，与审批卡同区域；键盘可达（Tab 顺序、Enter 提交、Esc 不提交）。
5. 无障碍：选项有 `aria-checked` 或原生 input；提交按钮在无有效选择且无自定义文本时 disabled 并说明原因。

### S4 场景 intent 与 Skill 绑定可见性

1. `HomeChat.tsx` 的 `submitTask()` 调用补 `intent`：场景存在时取 `SCENES[effectiveTag].scenario`，否则维持既有默认值（不传）。
2. `SettingsPanel.tsx` 规则资产分页：当 `rules.loaded` 且 `rules.rules.skill_bindings` 为空时，显示一条明确提示——出厂规则含场景 Skill 绑定，当前规则包为旧版未启用预注入，模型仍可按需自行加载技能，点「恢复默认规则包」可启用。提示只用现有 `hint` / `alert` 样式，不加新组件。
3. `HomeCanvas.tsx` / `HomeChat.tsx`：场景预设存在但 `preset.skills` 为 null（未注入）时，在对话区给一条低干扰说明（不弹窗、不重复刷屏，同一会话只显示一次）。文案必须区分「未预注入」与「技能不可用」。

## 验证

1. Rust：`eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)" && cd desktop/src-tauri && cargo test --offline -q`，全过得 0 failed。
2. Python：新增适配层单测，用合成 waterfall 帧覆盖：登记、下发生成 payload、回答帧 `$events/result` 断言（含 `clientId`/`eventId`/`outcome.value.answers`）、非法答案拒绝、超限拒绝、cancel 帧清理、终态清理。跑 `python3 -m pytest tests -q`。
3. 前端：`cd desktop && npx --no-install tsc --noEmit && npm run build`。
4. 回归：`python3 tests/verification/desktop/verify_skill_bindings_082.py`、`python3 tests/verification/phase-worklog/verify_worklog_085.py`、`bash scripts/sync-skills.sh --check`。
5. 真实窗口（一次性 Xvfb + XTEST，参考 `tests/verification/desktop/verify_ui_084.py` 的做法）：注入一条**合成**提问帧，断言卡片出现、点选项后提交、宿主收到回答、卡片变已答；再断言另有 cancel 场景落成已关闭。截图与无障碍树落 `tests/verification/desktop/evidence/od-arch-088/`。
6. 隐私断言：跑完后检查任务状态 JSON 与 `app.log`，断言不含题干/选项/答案文本（用合成标记字符串搜索）。
7. 每项结论标注证据等级：文件事实 / 静态检查 / 脚本运行 / 真实模型输出 / 未验证。**真实模型端到端（真跑一轮工作记录对话）本轮不做，留给用户实测。**

## 硬约束

- 不修改 `runtime/` 内 DSH Runtime；不动模型路由与凭证；不读打印密钥。
- 不接触真实业务材料；所有测试数据用合成文本（例如题干「合成测试：是否落盘？」），不进仓库的既有证据目录以外的地方。
- 不提交、不推送、不打标签。
- 发现计划本身的技术问题（例如 `$events/result` 的 value 形状与探针不符）：写入 STATUS.md 如实记录，不猜测、不伪造「已答」，不自行扩大范围。

## 交付

- 改动文件清单、实际运行的命令与结果、证据路径、未验证项、计划偏差、Git 状态快照。
- 完成后停下，等待 ARCHITECT 独立复核；不得自行宣布验收通过。

