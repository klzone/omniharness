# OD-ARCH-031 证据索引：模型设置跳转 DSH（Part A）+ 对话流工作区（Part B）

> 执行日：2026-10-20。开工 HEAD：`82a6b26`（OD-ARCH-030 收口后的远端 HEAD）。
> 未改 `runtime/`、模型路由、凭证；未合并 `omniharness-ui` worktree；未提交真实材料或用户数据。

## Part A：设置页跳转 DSH + 刷新入口始终可见

### A.1 改了什么

| 层 | 文件 | 内容 |
| --- | --- | --- |
| 宿主 | `desktop/src-tauri/src/task_shell.rs` | 新增 `open_dsh_settings()`：固定白名单的 DSH Desktop 启动器（`gtk-launch <app-id>` → 回退 `xdg-open <desktop file>`），**不拼接 shell、不接受前端传入的命令/路径/URL、不读不写不打印任何凭证**；未安装时**明确报错**。另抽出 `find_dsh_desktop_file()` 供单测在不启动任何东西的前提下断言「装没装」。 |
| 宿主 | `desktop/src-tauri/src/lib.rs` | 注册命令 `task_open_dsh_settings`（**无参数**：前端只能请求这一个固定动作）。 |
| 前端 | `desktop/src/api.ts` | `shell.openDshSettings()` 绑定。 |
| 前端 | `desktop/src/SettingsPanel.tsx` | 「模型权限」区块顶部**常驻**「打开 DSH 设置（登记 key）」与「刷新端点信息」；端点事实读不出时给出明确说明与同样两个入口；`facts && !facts.configurable` 的旧提示合并为一条；底部只留「撤销全部模型权限」。**三类开关与默认全关语义未动。** |

### A.2 实测（真实窗口，用户会话，正式环境）

- 设置页按钮（AT-SPI 实读）：`['撤销全部模型权限', '刷新端点信息', '打开 DSH 设置（登记 key）', '刷新状态', '打开当前目录', '校验并保存', …]` — **两个入口在端点已识别时可见可点**。
- 点「打开 DSH 设置」→ **DSH Desktop 真的被拉起**（`/opt/DeepSeek Harness/deepseek-harness` 进程出现，窗口在用户桌面），
  界面给出提示与 DSH 数据根路径；宿主全程未读、未写、未打印任何凭证值。
- 端点**不可识别**状态：另起一个实例（空 DSH 数据根）复核 → 两个入口同样可见；权限开关在端点可识别时正常渲染（3 个）。
  **限制：** 本机 DSH 在没有自定义 baseURL 时会回落到 DeepSeek 默认端点，因此**自然构造不出「端点不可识别」的状态**；
  该分支是按代码条件渲染的（`!facts || factsError`），已由源码核对，**未做真实不可识别状态的截图**。
- 回归：Rust **144/144**（新增 1 条 `dsh_desktop_lookup_finds_only_existing_desktop_files`）、`npx tsc --noEmit` 0、
  `npm run build` 通过。

### A.3 需要 ARCHITECT 注意的**计划偏离**

计划原文写「宿主不新增命令」，但 Part A 第 1 条要求设置页能**拉起 DSH Desktop 的设置界面**——
Tauri v2 前端没有可用的外部启动器（`@tauri-apps/plugin-opener` 未安装），不做宿主命令就无法实现。
因此新增了**一个**最小命令 `task_open_dsh_settings`：无参数、固定白名单、不读写凭证、失败即明确报错。
这是**唯一**的新增宿主能力（不涉及协议、不涉及通信层），已在 REVIEW.md 标注，供复核确认。

## Part B：对话流工作区（项目 → 会话 → 对话）

### B.1 改了什么（全部在前端，**未新增宿主命令、未改协议**）

| 文件 | 内容 |
| --- | --- |
| `desktop/src/shell/ProjectRail.tsx`（新） | 左侧「项目 / 会话」栏：项目列表（绝对路径，可加入）、项目下的**工作台对话**（可继续）、**引擎会话**（DSH 真实会话，只读元数据）、开场预设芯片、「新会话」入口。项目路径只做**记住**，不创建/删除目录。 |
| `desktop/src/shell/Workspace.tsx`（新） | 工作区外壳：项目/会话状态、预设选择、对话线程的本地持久化（`localStorage`，只在本机前端）。 |
| `desktop/src/conversationPrompt.ts`（新） | 纯函数：上下文累积（上限 4000 字符，超出显式标注截断）与提示词组装（有预设走 `aiDraft` 同一套组装器，无预设发上下文本身）。 |
| `desktop/src/shell/HomeChat.tsx` | 接入预设、项目目录、线程恢复与消息回调；流式输出沿用既有 `text_delta` 事件。 |
| `desktop/src/shell/HomeCanvas.tsx` | 支持左侧栏与对话区的附加参数。 |
| `desktop/src/App.tsx` | 首页改为 `Workspace`（项目 → 会话 → 对话）；三场景向导页**保留在 Dock**（按计划「去留后置」）。 |
| `tests/verification/desktop/verify_conversation_prompt.py`（新） | 脚本级验证：上下文累积、预设注入、截断、不可关闭规则不丢。 |

### B.2 实测

- **界面（真实窗口截图 `shots/od031-workspace-rail.png`）：** 左侧「项目」栏（预设芯片：普通对话 / 周报与随记 / 文案写作 / 项目跟踪；项目输入框 + 「加入」；空态说明），
  下方「会话」栏（工作台对话（可继续）/ 引擎会话（只读）两个分组，各自空态说明），右侧沿用用户已确认的首页版式
  （问候 + 机器人 + 今日重点推进 + 日程节律 + 全宽对话 + 权限/Token 行）。
- **脚本级验证 `verify_conversation_prompt.py`：12/12 通过**（compile_exit=0、probe_exit=0、failed=0）：
  - 普通对话的提示词 = 上下文（含上一轮用户与助手的话）；
  - 带周报预设时，提示词同时含**流程包真实规则**、**期望章节**与上下文；
  - 预设路径**没有丢掉**不可关闭的写作规则（禁入案件材料那条仍在）；
  - 超过 4000 字符时只保留最近部分并显式标注截断。
- **回归：** `tsc` 0、`npm run build` 通过、`verify_ai_draft` 27/0、Rust 144/144。

### B.3 没能自动化的部分（诚实记录）

- **真实窗口里用合成键盘输入驱动对话，本轮做不到**：XTEST 注入的字符**进不了**输入框
  （输入框能被点中并获得焦点，但字符不落；同一窗口里普通表单输入框历史上可以）。
  已排除的原因：DSH Desktop 等其它应用抢占合成器焦点（已关闭后仍复现）。
  因此「在 GUI 里打字 → 端点收到带上下文的请求」这一段**没有**端到端自动化证据；
  改由**脚本级**验证提示词组装（上面 12 例）+ 既有 028 验收（提交链路本身）覆盖。
  证据：`partb-run1.json`（记录了「点了发送但没有任务」的现场）与 `shots/od031-xtest-focus.png`。
- **「同一引擎会话里的多轮」在当前协议下不存在**：宿主↔引擎的动词里只有
  `submit_task`（每次 `session/new`）、`list_sessions`、`recover_session`（`session/resume`，不重放），
  **没有**「向既有会话再发一条」的动词。因此工作台的连续对话是**每轮一个新 DSH 会话 + 工作台把上文重放进下一轮提示**，
  这一点已写进 `Workspace.tsx` 的文件头注释与本索引，不含糊。
- **「AI 自主调 Skill」未验证**：DSH profile 是否在 headless/acp 模式下加载 Skill，本轮未测。

## 遗留与后续

- Part B 的端到端打字链路需要一次**人工**验证（用户在真实桌面点一次即可）。
- 若要「同一 DSH 会话多轮」，需要给适配层加一个动词 —— 超出本计划「不新增通信层」的边界，须另行批准。

