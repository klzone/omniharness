# OmniDesk Discipline — 进度与功能交接（供独立检验）

> 生成日：2026-10-20。仓库 `/mnt/data/Code/MyWorkStation`，远端 `github.com/klzone/omniharness`（**公开仓库**），
> HEAD = `2b3a5b4`，本地与远端一致，工作区 0 项未提交变更。
> 本文件只写**已落地的事实**与**明确未验证的部分**，不含推测。

## 0. 检验须知（先读这段）

1. **两个克隆目录，别搞混：**
   - `/mnt/data/Code/MyWorkStation` ← **本轮成果在这里**（HEAD `2b3a5b4`，已推送）。
   - `/mnt/data/Code/omniharness-ui` ← **另一个旧克隆**（HEAD `5ffc710`），带一批**未提交的**改动，
     属于更早的工作，**不是本轮成果，也不要动它**。
2. **数据在仓库外**：`~/OmniDesk/{app,runtime,workspace}`。仓库里**不应**出现会话记录、真实材料、凭证。
   检验时可用 `git status --porcelain -uall` 确认工作区干净。
3. **凭证不读不打印**：真实端点的 key 以引用形式放在 `~/OmniDesk/runtime/dsh-home/.credentials.yaml`。
   任何检验脚本都不要 cat/打印该文件，也不要把它复制进仓库。
4. **材料边界**：案件/线索/人物画像类材料**不进**工作台；成果只来自模型初稿 + 人工确认后保存。
5. **端点权限是三档开关**：本机 / 私网 / **非回环·公网**。当前用的是**公网**端点。

## 1. 产品是什么

面向个人工作的独立 AI 工作台（OmniDesk Discipline）：以**大模型对话**为主入口，左侧 Dock 进各场景模块，
任务/成果/材料/使用记录由工作台自己持有，**DSH 只是可替换的执行引擎**（当前通过 ACP 接缝驱动）。

## 2. 怎么跑起来

| 用途 | 命令 | 说明 |
| --- | --- | --- |
| 正式环境（真实端点、正式数据根） | `scripts/omnidesk.sh` / `--status` / `--stop` | 当前实例运行中（app pid 540782，前端 dev server pid 540751） |
| 隔离合成环境（合成数据） | `scripts/dev-ui.sh` | 按硬边界**拒绝** `--real` |
| 引擎运行时准备 | `scripts/bootstrap-runtime.sh`、`scripts/sync-skills.sh` | skill 同步带 `.omnidesk-managed` 标记 |
| 备份 | `scripts/backup-sessions.sh`、`scripts/backup-workspace.sh` | 会话与工作区分别备份到仓库外 |

Rust 工具链环境变量（否则 `cargo test` 会报 `javascriptcoregtk-4.1` 缺失）：

```bash
cd desktop/src-tauri && eval "$(bash ../../scripts/setup-desktop-toolchain.sh --print-env)" && cargo test
```

## 3. 功能清单（按界面模块，附文件位置）

### 3.1 外框与顶栏
- Tauri **frameless**：无系统标题栏，最小化/最大化/关闭自绘在顶栏右侧，顶栏可拖动，四边/四角保留缩放热区。
- 顶栏：全局检索、日期时间、模型/通知/设置快捷入口。
- 文件：`desktop/src/shell/WindowChrome.tsx`、`TopNav.tsx`。

### 3.2 Dock（左侧 8 个入口）
`desktop/src/shell/Dock.tsx`
1. 工作台首页　2. 周报与随记（模型起草）　3. 文案写作（模型起草）　4. 项目跟踪（模型起草）
5. 任务工作台　6. **项目与会话**　7. 成果目录与模型权限　8. 引擎诊断（只读）

### 3.3 首页（对话中枢，两层布局）
`HomeCanvas.tsx` + `SceneScaffold.tsx`
- **下层**：顶部卡片区 —— 问候 + 机器人 + 「今日重点推进」（真实空态，不编造数字）+ 「日程节律」（三周日期骨架）。
- **上层**：对话，**占满上下全屏**。
- **两层联动**：指针落在卡片带内 → 卡片显示；指针进入对话区（对话历史/输入框）→ 卡片**渐隐**，
  对话上移占满顶部（「对话历史显示在卡片上面」）；指针离开窗口 → 卡片恢复。
  判定按**指针在容器内的纵向位置**（元素 hover 会被两层互相遮挡，出现「卡片回不来」的死角）。
- **首页没有**项目/会话目录树，**没有**左右分栏（这是用户现场确认过的定版版式）。

### 3.4 场景页（周报与随记 / 文案写作 / 项目跟踪）
`ScenePage.tsx` + `AnnouncementBar.tsx`
- 双栏：左=本场景历史对话，右=本场景成果档案（只读本场景自己保存的文件）。
- 项目跟踪页右栏带「进度 / 阶段成果」两个页签。
- 顶部提醒/建议栏：**规则提醒**＝纯本地确定性汇总（不花 token）；**AI 复盘建议**＝可选、默认关闭、
  频率可配、只取本场景材料、每次调用前显示成本提示、用量计入现有用量显示。

### 3.5 对话区（核心）
`HomeChat.tsx`、`desktop/src/conversationPrompt.ts`
- 开场预设：普通对话 / 周报与随记 / 文案写作 / 项目跟踪（**场景已从"页面"降为"预设"**）。
- **模型选择下拉**（自绘）+ **思考等级**（默认/关闭/中/高）。
  **思考链**以「思考过程（模型产出，不计入最终结果）」区块**默认展开**显示；本轮无思考时如实写明原因。
- **逐字呈现**（界面层效果，**不是**引擎流式，见 §6）。
- 上下文累积与窗口化：最近 4 条用户 + 2 条助手，单轮上限 2500 字符。
- 红线扫描**只扫用户原文** `userInputText`（不扫拼好的提示词，否则场景写作规则里的「案件/线索/未公开」会误判）。
- 状态行：模型权限档位、Token 消耗（引擎不上报时显示 `—`，不估算）、工作区路径。

### 3.6 项目与会话模块
`ProjectRail.tsx` / `Workspace.tsx`：项目分组、会话列表、进行中的工作台对话、新建对话与预设选择。
会话**入口即打标**、事后可改挂、标签持久化。

### 3.7 设置页
- 「打开 DSH 设置」：启动**引擎实际使用的那份隔离 DSH**（`node <runtime>/dsh/lib/bin.js web`，
  `DSH_HOME`/`DSH_AGENTS_HOME` 指向引擎同款数据根），不是用户个人的 DSH Desktop。
- 常驻「刷新端点信息」。
- 模型权限三档开关 + 路由覆盖文件 `~/OmniDesk/app/config/omnidesk-model.yml`（只含 baseURL/api/apiKeyEnv/模型 id，**不含 key**）。

### 3.8 引擎诊断（只读）
`EngineTechSlice.tsx`：引擎描述、健康、任务台账、会话列表、取消。

### 3.9 成果与材料边界
- 成果只能来自**模型初稿 + 人工确认**后保存，保存到**仓库外**数据目录。
- 失败/取消**不产生任何成果文件**，界面也不显示模型正文。
- 读取材料有白名单与路径检查（`task_shell.rs` 的 `read_material` / `assert_input_allowed` 等）。

## 4. 引擎侧（ACP 接缝）

`engine/omnidesk_engine/`（Python）：`acp_adapter.py`（ACP 适配层）、`contract.py`（状态机与契约）、
`jsonrpc.py`、`serve.py`。宿主侧：`desktop/src-tauri/src/engine_bridge.rs` / `engine_ipc.rs` / `engine_supervisor.rs`。

适配层对外能力：`initialize` / `submit_task` / `get_task_status` / `stream_events` / `get_task_result` /
`cancel_task` / `list_sessions` / `list_task_records` / `recover_session`（走 ACP `session/resume`）/
`recover_task` / `shutdown`。

Tauri 命令共 41 个（`lib.rs` 的 `invoke_handler`）：`engine_*` 12 个、`task_*` 24 个、`model_*` 2 个、
`app_paths_info` 1 个、`flow_pack` / `flow_samples` 2 个。

## 5. 已验证 / 未验证（附证据位置）

### 已验证（本轮实跑）
| 项 | 结果 | 证据 |
| --- | --- | --- |
| Rust 单元/集成测试 | **143/143 通过**（125+2+13+3，0 失败） | `cargo test`（需 §2 的环境变量） |
| 前端类型与构建 | `tsc` 0 错、`npm run build` 通过 | — |
| 提示词组装 | 15/15 | `tests/verification/desktop/verify_conversation_prompt.py` |
| 模型起草闭环 | 27/0 | `verify_ai_draft.py` |
| 场景材料校验 | 5/0 | `verify_scenario_materials.py` |
| 两层布局真实窗口 | 首页/场景页各两态截图 | `tests/verification/desktop/evidence/od-arch-040/shots/` |
| 历史各轮证据 | 025–040 共 16 个证据目录 | `tests/verification/desktop/evidence/` |

### 未验证（别当已实现）
- **真实模型的思考内容**：合成端点不返回 reasoning ⇒ 只验证了「有思考时能显示」这条路径的**界面**，
  没有验证过真实模型的思考文本质量。
- **token 级流式**：ACP 通道**不提供**（源码级确认，见 §6），界面是逐字呈现效果。
- **真多轮**：每轮仍**新建**引擎会话 + 上文重放，不是同一会话续发（§7 第 1 步）。
- **真实桌面（Wayland / release 包）**、**触屏/无指针设备**的降级路径。
- **真实使用指标**（澄清/返工次数）与真实 token 用量（引擎不上报 usage ⇒ 一律记未知，不估算）。

## 6. 已知问题与诚实限制（逐条）

1. **ACP 无 token 级流式**：`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-acp/lib/index.js` 的
   `assistantUpdates()` 把**已提交**的消息转成更新 ⇒ 思考与正文在提交时**同时**到达。
   界面上的逐字呈现是**界面层效果**，界面与文档均已标注，不是引擎流式。
2. **真多轮没接上**：ACP 有 `session/new`/`session/prompt`/`session/resume`，我们的适配层**也已实现**
   `session/resume`（`recover_session`），但 `submit_task` 每次都 `session/new` ⇒
   「同一线程 → 同一引擎会话」没连上，现在靠**上文重放**模拟多轮。界面已如实标注。
3. **思考链不是边想边出**：见 1。已改为文本透传 + 默认展开显示。
4. **延迟来自模型侧**：用户实测思考等级 关=秒回 / 中≈5s / 高>1min ⇒ `reasoningEffort` 确实生效；
   任务台账里 4.19s / 17.06s / 35.63s / 62.51s / 97.45s。模型自述慢在「模型推理延迟」+「回答前读文件/读 skill 资料」。
5. **引擎带 6 个 skill**：`document-review` / `knowledge-agent` / `knowledge-search` / `material-writing` /
   `memory-agent` / `project-agent`（来自 `scripts/sync-skills.sh`，带 `.omnidesk-managed` 标记；
   **不含**个人免脱敏材料 `knowledge/reflections/`）—— 是否精简待用户决定（skill 资料读取是延迟来源之一）。
6. **模型路由覆盖**（我们唯一写入的模型文件）：`~/OmniDesk/app/config/omnidesk-model.yml`，
   只含 `baseURL` / `api` / `apiKeyEnv` / 模型 id，**不含 key**；同时写入 `acp` 条目（ACP 实际路由所在处）
   与 `agent-default-model`。用户原有的 `~/.dsh` **未被写入模型**。
7. **两个 DSH profile 的模型选择不互通**：`acp`（引擎实际使用）与 `web`（设置界面）只**共享凭据**，
   不共享模型选择 —— 这是 DSH 的既有行为，不是我们的实现缺陷。
8. **曾出现「继续 → 没有任何回复」**：引擎当时确实回复了（4.19s），但界面没显示，根因未抓到
   （无法自动复现：无边框输入框注入不了中文）。已加可见诊断（「引擎报告已完成但没有返回任何正文」+「重新查询」），
   **仍需用户复现并提供任务号**才能定位。
9. **公共远端**：`github.com/klzone/omniharness` 是**公开仓库**；每次提交前都跑敏感信息扫描 + 人工核对文件清单
   （会话日志、真实材料、凭证、仓库外数据一律不入库）。

## 7. 待你/用户裁定的事项（未实施）

主记录：`docs/decisions.md` **D-024**（候选方案，未实施）。

- **第 1 步（我方欠账，不换接缝）**：把「线程 ↔ 引擎会话」接起来（`submit_task` 接受既有 `engine_session_id`，
  前端线程持有该 id），去掉上文重放 ⇒ 真多轮 + 原版延迟。
- **第 2 步（只验证、不改产品）**：起 DSH 自带的 web 应用，走它的本地 API 发一条消息，核实
  token 级流式 / 思考块 / 模型目录 / 会话续接是否真的存在，形成证据。
- **第 3 步（裁定）**：第 2 步成立再决定：路径 A（以 DSH 应用为底座改 UI）还是路径 B（保留我们的 UI、
  引擎侧换成 DSH 本地 web API）。**代价**：DSH 这些是内部接口（非公开契约，需锁 runtime 版本），
  且与 D-001/D-002 的「产品独立、DSH 可替换」有张力。
- 其他未决：是否精简引擎带的 skill；是否在 DSH 界面登记 `DEEPSEEK_API_KEY`（对应 provider 的 `apiKeyEnv`）。

## 8. 建议 Codex 直接跑的检验步骤

```bash
cd /mnt/data/Code/MyWorkStation
git log --oneline -5 && git status --porcelain -uall && git ls-remote origin main | cut -c1-12

# 前端
cd desktop && npx tsc --noEmit && npm run build && cd ..

# Python 验证脚本
python3 tests/verification/desktop/verify_conversation_prompt.py   # 期望 cases=15 failed=0
python3 tests/verification/desktop/verify_ai_draft.py              # 期望 cases=27 failed=0
python3 tests/verification/desktop/verify_scenario_materials.py    # 期望 scenarios=5 failed=0

# Rust（注意环境变量）
cd desktop/src-tauri && eval "$(bash ../../scripts/setup-desktop-toolchain.sh --print-env)" && cargo test
```

**重点复核项（按风险排序）：**
1. §6.1 的 ACP 流式限制是否与源码一致（`dsh-acp/lib/index.js` 的 `assistantUpdates()`）。
2. §6.2 的「`submit_task` 每次 `session/new`」是否属实（`engine/omnidesk_engine/acp_adapter.py`）。
3. 两层布局的**指针位置判定**是否真的避免了「卡片回不来」死角（`SceneScaffold.tsx`）。
4. 红线扫描口径是否只作用于 `userInputText`。
5. 仓库里是否混入了真实材料 / 会话日志 / 凭证（`git grep` + 文件清单核对）。
6. 文档与实现是否一致（界面上的诚实标注是否还成立 —— 别把「逐字呈现」写成「流式」）。

## 9. 关键文件索引

| 关注点 | 文件 |
| --- | --- |
| 首页与两层骨架 | `desktop/src/shell/HomeCanvas.tsx`、`SceneScaffold.tsx` |
| 对话区 | `desktop/src/shell/HomeChat.tsx`、`desktop/src/conversationPrompt.ts` |
| 模型选择与思考等级 | `desktop/src/shell/ModelPicker.tsx`、`desktop/src-tauri/src/model_routes.rs`、`scripts/model-routes.mjs` |
| 场景页 | `desktop/src/shell/ScenePage.tsx`、`AnnouncementBar.tsx` |
| 引擎适配层 | `engine/omnidesk_engine/acp_adapter.py`、`contract.py` |
| 宿主命令与边界 | `desktop/src-tauri/src/task_shell.rs`、`engine_bridge.rs`、`engine_supervisor.rs` |
| 正式启动器 | `scripts/omnidesk.sh`（隔离合成：`scripts/dev-ui.sh`） |
| 状态与决策 | `GOALS.md`、`PLANS.md`、`STATUS.md`、`REVIEW.md`、`docs/decisions.md` |
| 证据 | `tests/verification/desktop/evidence/od-arch-0{25..40}/` |
