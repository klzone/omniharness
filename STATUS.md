# BUILDER 执行状态

## OD-ARCH-037：对话延迟与「看起来没回复」（2026-10-20，ARCHITECT 执行）

- **延迟构成（台账 + 模型自述）：** 「不用你管了」17.06s、「继续」4.19s，更长几轮 97.45/62.51/35.63s；
  模型自述慢在「模型推理延迟」+「回答前做了后台动作（读文件、读 skill 资料）」。
  DSH 原版快是因为 profile/技能组合不同，且**原版是服务端续会话（不重放上下文）**。
- **已改：** 轮询 1500→300ms；生成中显示已等待秒数；生成中再发不再静默丢弃（给可见提示）；
  成功但正文为空时显示明确说明与「重新查询」（不再留空气泡）；上下文重放窗口化（最近 4 用户 + 2 助手、上限 2500 字符）。
- **回归：** verify_conversation_prompt 15/15（含新增轮次窗口 3 例）、tsc 0、构建通过。
- **遗留：** 「继续」的界面根因未抓到（引擎确实回复了）；真正的「同一会话续发」需给适配层加动词（待批准）。

## OD-ARCH-035：对话观感对齐 DSH（2026-10-20，ARCHITECT 执行）

- **起因（用户实测）：** 回复一次性到达（等 3–5 秒）、模型下拉太丑、缺少思考等级。
- **架构事实（先讲清）：** DSH 的 **ACP 通道不提供 token 级流式**——ACP 更新按「已提交的助手消息」产出
  （`dsh-acp` 的 `assistantUpdates()` 注释即「Convert one committed assistant message」），
  网页端的逐字效果走的是它自己的 harness 事件流。所以引擎侧能给的最细粒度是**块**。
- **已做：** ①**思考链**真实透出（适配层此前只传字数、丢文本）并在助手气泡里可折叠展示；
  ②**逐字呈现**的本地呈现效果（不改写正文，界面与文档标注为呈现效果而非引擎流式）；
  ③**模型选择器**重做为自绘下拉（与工作台同风格），并加**思考等级**（默认/关闭/低/中/高，对应 DSH 的 `reasoningEffort`）。
- **实测：** 真实窗口截图 `evidence/od-arch-035/shots/`（下拉展开态含思考等级区）；Rust 143/143、tsc 0、构建通过。

## OD-ARCH-034：模型路由切换（2026-10-20，ARCHITECT 执行）

- **起因（用户实测）：** 在 DSH 界面配了自定义模型（agnescn / agnes-3.0-flash）后，回 OmniDesk「刷新端点」
  仍是 deepseek，任务只拿到 partial。**根因：** DSH 的模型选择**按 Profile 存**——用户改的是 `web` Profile，
  引擎跑的是 `acp` Profile；两边**只有凭据共享**（`AGNESCN_API_KEY` 在共享凭据里 ✓）。界面显示 deepseek 是
  如实反映引擎那条路。
- **已实现：** 对话区**右下角、发送按钮前**的模型下拉（显示当前路由 + 各 Profile 候选）；选中即写
  OmniDesk 自己的路由覆盖文件（`~/OmniDesk/app/config/omnidesk-model.yml`，**只含非机密字段**），
  引擎与端点分类器共用同一份（引擎 patch 列表附加该文件），并停掉/重启引擎、重新判定端点。
- **实测：** 覆盖后判定为 `agnescn / agnes-3.0-flash @ https://api.agnes-ai.cn/v1`（public）；应用内下拉显示
  「模型：agnescn / agnes-3.0-flash」；覆盖文件内**无任何 key**；截图
  `evidence/od-arch-034/shots/od034-chat-model-dropdown.png`。回归 Rust 143/143、tsc 0、构建通过。
- **未验证：** 切换后的真实模型往返（需真实 key，属用户真实使用范围）。

## OD-ARCH-033：场景模块页（2026-10-20，ARCHITECT 执行）

- **执行方式：** 原计划派 super-worker；本会话的 agent 目录发现不到项目 agent（工具返回
  `Unknown agent: super-worker`，见 `docs/od-arch-033-dispatch.md`），按用户指示**由主会话直接执行**。
- **已实现：** ①**骨架插槽化**——新增 `SceneScaffold`，首页与三个场景页共用（首页版式**未变**）；
  ②**场景页**——周报/文案双栏（左=本场景历史对话，右=成果档案）、项目跟进右栏「进度 / 阶段成果」页签、
  顶部横向**提醒/建议栏**；③**会话打标**——入口即标签、事后可改挂、标签持久化；
  ④**v3 提醒/建议栏**——规则提醒（既有确定性汇总，不花 token、不编数字）+ AI 复盘建议
  （前端定时器、频率可配、**默认关闭**、按钮注明「会调用一次模型」、输入只取本场景成果与对话、
  结论存为���场景成果文件、显示最新一条并可开历史）；「趋势热点」**未做**（需外部数据源，用户未授权）。
- **停下并报告（按计划停止条件，不擅自新增宿主能力）：** 目标 4「线程记录落到仓库外数据根并按项目分目录」
  ——现为浏览器本地存储（**已确认不在仓库内**，但不是数据根）；目标 5「成果目录是否含下级目录」——
  宿主成果列举是平铺匹配，无目录树能力。另「数据分析占位页」未新增（Dock 现有「任务工作台」即分析试点）。
- **实测：** 真实窗口截图 6 张（`evidence/od-arch-033/shots/`：首页、周报、文案、项目、阶段成果页签、模块页）；
  回归 Rust **144/144**、`tsc` 0、构建通过、`verify_conversation_prompt` 12/12、`verify_ai_draft` 27/0；
  `git status` 中**无任何会话记录文件**。
- **未验证：** 真实窗口合成键盘驱动对话（字符进不了输入框，见 OD-ARCH-031 §B.4）→ 端到端打字链路需人工点验；
  AI 复盘的真实跑通。

## OD-ARCH-033 派工尝试（2026-10-20，ARCHITECT）：本会话无法调起 super-worker

- **授权：** 用户「批准让 super-worker agent 去执行 033 号任务」。
- **尝试与结果：** 在本会话调用 subagent 工具传 `agent: "super-worker"` → 工具返回
  **`Unknown agent: super-worker`**（2026-10-20 实测）。本会话的 agent 目录里**没有**这两个项目 agent；
  它们创建于 2026-10-19 22:47 之后，而**该会话的 agent 目录建立得更早**。这与 T-001 记录的
  「另一会话能发现、另一会话不能」的不一致同源，**根因未查清**（不写进「已验证」也不写进「已失败」）。
- **可执行路径已备好：** 完整的派工任务书（必读、现状、纪律、交付与验证、停止条件）落在
  [docs/od-arch-033-dispatch.md](docs/od-arch-033-dispatch.md)，在**能发现项目 agent 的会话**里
  （写 032/033 计划的那条会话，或新开会话）由 super-plan 原样派给 super-worker 即可；
  主会话随后做独立复核（源码 + 差异 + 测试 + 真实窗口截图）并统一登记入库。
- **未执行任何 033 的代码改动**（工作区仅新增这一个派工文档）。

## OD-ARCH-032 已制定待批准（2026-10-20，ARCHITECT）

计划登记，非执行记录。OD-ARCH-031 Part A 复核通过（含一个已认可的最小宿主命令偏离）；Part B 结构方向正确但首页摆放违反用户裁定（首页不得有项目树/分栏），且登记后未提交的 ProjectRail 修改导致当前构建 4 个 TS 错误（独立复测证实）。OD-ARCH-032：首页还原全宽对话定版 + 项目/会话收进 Dock 模块页 + 修复构建。全文见 PLANS.md 顶部。

## OD-ARCH-031：模型设置跳转 DSH + 对话流工作区（2026-10-20，ARCHITECT 执行）

- **授权：** 用户「授权执行 OD-ARCH-031」。**Part A 完成并实测**（设置页常驻「打开 DSH 设置（登记 key）」与
  「刷新端点信息」；点击**真的把与引擎同款的 DSH 拉起来**（`node <runtime>/dsh/lib/bin.js web`），并**指向引擎正在用的
  那份 DSH 数据根**——首版拉起的是用户自己那份 DSH，「模型配置没导入过来」（用户实测指出）已修；
  启动失败时明确报错；端点读不出时两个入口同样可见）。
  宿主为此新增**一个**无参数命令 `task_open_dsh_settings`（固定白名单 `gtk-launch`/`xdg-open`，不拼接命令、
  不读不写不打印凭证）——这是相对计划「宿主不新增命令」的**必要偏离**（前端无外部启动器能力），已标注待复核。
- **Part B（项目与会话模块）已按用户纠正落地：** 首页**回到用户确认的定版版式**——没有项目目录树、没有左右分栏，
  左侧仍只有 Dock 进各场景；DSH 原本的「项目 / 会话」收进**一个模块页**（Dock 入口「会话与任务」→「项目与会话」）：
  左侧两个分页（**引擎项目**：DSH 真实工作区分组与其只读会话元数据；**本机对话**：工作台草稿，第一轮提交后由
  引擎产生 DSH 会话并归入引擎项目），右侧对话区，底部折叠「OmniDesk 任务记录（只读）」。三场景降级为**开场预设**
  （普通对话 / 周报与随记 / 文案写作 / 项目跟踪），向导页按计划**保留**在 Dock。新增纯函数模块
  `conversationPrompt.ts`（上下文累积，上限 4000 字符并显式标注截断；预设走 `aiDraft` 同一套组装器，
  带上流程包真实规则、期望章节与不可关闭的写作规则）。**刻意去掉**了早先那版「手填绝对路径当项目」的入口
  ——宿主给的是不透明工作区 token，界面不得自行拼路径。
- **实测：** 真实窗口截图 `evidence/od-arch-031/shots/od031-home-no-rail.png`（首页无目录树/无分栏）与
  `od031-module-projects.png`（模块页结构）；新增脚本级验证
  `verify_conversation_prompt.py` **12/12 通过**（上下文累积、预设注入、截断、规则不丢）；
  回归 `tsc` 0、构建通过、`verify_ai_draft` 27/0、Rust 144/144。
- **未能自动化（不当作通过）：** 真实窗口里 XTEST 注入的字符**进不了**输入框（能聚焦、字符不落；
  已排除「别的应用抢焦点」仍复现），故「GUI 打字 → 端点收到带上下文的请求」无端到端证据，留待**人工点一次**。
- **架构事实：** 宿主↔引擎**没有**「向既有会话再发一条」的动词（`submit_task` 每次 `session/new`；
  `recover_session` 只 `session/resume` 且不重放）→ 工作台连续对话 = 每轮一个新 DSH 会话 + 工作台重放上文；
  要真正的同会话多轮须给适配层加动词，超出本计划边界，须另行批准。
- **提交：** Part A `32200a8`、Part B `153b0fa`（含证据与 `verify_conversation_prompt.py`）。

## OD-ARCH-031 已制定待批准（2026-10-20，ARCHITECT）

计划登记，非执行记录。依据用户首次实测反馈制定：Part A 模型设置跳转 DSH（含刷新入口始终可见的修复）；Part B 对话流工作区改造（项目→会话结构、三场景降级为开场预设、向导页保留待用户后定去留）。全文见 PLANS.md 顶部。批准后交 BUILDER 连续执行，完成后 ARCHITECT 一次性复核。

## 正式环境启动器与首次真实使用就绪（2026-10-20，ARCHITECT）

- **背景：** 用户准备人工审核并跑真实流程。`scripts/dev-ui.sh` 按 OD-ARCH-004 的硬边界**故意不提供**真实端点开关，
  因此**另建**一个独立启动器 `scripts/omnidesk.sh`（不改开发启动器的边界）。
- **新增：** `scripts/omnidesk.sh`（正式环境）。与 dev-ui.sh 的分工：dev-ui.sh = 隔离合成环境（零凭证、/tmp 数据根）；
  omnidesk.sh = 正式数据根 + 真实模型配置。启动器**不设置**任何 `OMNIDESK_*` 隔离变量，宿主按默认解析到
  `~/OmniDesk/app`（设置/台账/使用记录）、`~/OmniDesk/workspace/artifacts`（成果）、
  `~/OmniDesk/runtime/dsh-home`（真实模型配置与凭证，**脚本不读取**）；**不注入**合成 provider 覆盖；
  **不开启任何模型权限**（默认全关，由用户在设置页自行确认端点后开启）。支持 `--no-build` / `--status` / `--stop`，
  停止前按命令行指纹核对进程身份。后台进程用 `setsid` 完全脱离，停止不留孤儿。
- **实测（本机）：** `--status` / `--stop` 正常；`--no-build` 启动成功（app pid 与 vite pid 均登记），应用跑在
  **原生 Wayland**（无系统标题栏，符合已确认 UI 基线）；设置页三个权限开关状态与设置文件一致；
  场景页「使用记录」显示 `查看记录（0）」，落点为正式根 `~/OmniDesk/app/logs/usage.jsonl`。
- **发现并如实记录（不是产品缺陷）：** 正式根的 `config/settings.v1.json` 在 13:07:20 出现，三个权限全为 `true`。
  复现结论：**启动不会写设置文件**（用一次性全新应用数据根启动，`config/` 为空），代码里也只有
  `set_model_permission` / `revoke_all` / `set_artifact_root` 三个写点，均由显式命令触发 →
  该文件来自**界面上的权限勾选操作**，不是默认放行。同一时段（13:08）有两条引擎台账，
  两次都**失败**并给出真实原因：`no API key for provider route "deepseek-official"`（凭据服务里只有
  `OPENDESIGN_API_KEY`，没有该路由的 key）——失败是响亮的，未写出任何正文。补 key 的两条正规路径见
  [docs/pilot-first-real-use.md](docs/pilot-first-real-use.md)。
- **未做：** 不代取、不打印、不写入任何 key；不代用户开启权限；不改模型路由。

## OD-ARCH-030 复核通过（2026-10-20，ARCHITECT）

计划登记，非执行记录。四项（端点引导修正与假端点全链路、使用记录持久化至正式根并清空测试数据、遗留杂物删除、3+1 批提交推送）复核通过，详见 REVIEW.md 顶部。T-001 双 agent 四项复测闭环作为超计划收益一并认可。真实服务商端点与真实模型质量未验证，属用户首次真实使用范围。当前无待批准计划，等待用户体验反馈。

## OD-ARCH-030：真实使用前置收口（2026-10-20，ARCHITECT 执行）

- **授权：** 用户「批准执行 OD-ARCH-030」。开工快照：HEAD `653bf4a`（与 `origin/main` 一致）、工作区 6 项登记差异
  + 1 个遗留 0 字节文件；未改 `runtime/`、模型路由、凭证、真实材料与用户数据；未合并 `omniharness-ui` worktree。

### ① 端点设置引导与验证（通过，含 1 处文案修正）

- **文案核实（源码 + 渲染截图）：** 设置页「模型权限」引导覆盖 8 项要点，逐项核对为真：本机回环 / 非回环
  （私有·链路本地、公网）分类说明、默认全部关闭、无法确认位置的端点永不放行、自行确认发布方的授权与数据去向、
  端点来自 DSH 侧配置、不读取凭证值、刷新端点信息的路径。
- **修正（唯一的产品代码改动）：** 原文「请先在**隔离环境**中把模型配置指向明确的地址」会让人以为要在隔离环境里
  改端点；实际端点来自 DSH 侧模型配置。已改为「请先把 DSH 侧的模型配置指向一个明确的地址，再回到本页点
  「刷新端点信息」」，并在权限区块补一段：端点不在这里配、本应用只读**非机密**的 provider/model/地址用于判定、
  **不保存端点也不读取凭证值**。未改路由代码与凭证。
- **假端点验证（真实窗口 + 回环假端点 + 一次性隔离根）：**
  - 默认：三个权限开关全部未勾选；
  - 权限关闭时提交：输入框占位提示变为「模型权限未开启：可在设置中确认端点后启用」、发送按钮不可用、
    **未产生任何任务**（20 s 内台账无记录）→ **不出错、不静默降级**；
  - 界面开启「本机模型」→ 设置文件写入 `model_permissions.local=true` → 首页占位提示变回可提交文案（准入已传播）；
  - 开启后真实调用：场景页「读取材料 → 生成模型初稿」终态 `succeeded`，返回合成端点内容。
  → **端点判定 → 权限开关 → 配置落盘 → 提交放行 → 模型返回**链路打通。

### ② 使用记录持久化到正式数据根（通过）

- **落盘位置** `~/OmniDesk/app/logs/usage.jsonl`（应用数据根的 logs/，仓库外）。此前自检落在 `/tmp`
  只是因为测试覆盖了 `OMNIDESK_APP_DATA_ROOT`；**产品代码无需改动**即可落到正式根。
- **格式：** 只含枚举与时间（`record_id/schema/task_type/recorded_at/completed/usable/rework_kind/blocker/
  duration_bucket/reuse_willingness`），**无输入、输出、正文、标题、路径或会话**。
- **读回：** 界面「查看记录（2）」，展开后逐条列出。
- **清理：** 用产品自带的**逐条删除**清掉本轮 2 条合成记录 → 文件回到 **0 字节**，正式根干净，
  用户的首次真实使用不会混入测试数据。
- **边界核对（目录快照前后比对）：** `~/OmniDesk/quarantine/`（真实案件材料）与 `~/OmniDesk/runtime/`（含凭证）
  **均未新增、未改动、未删除**；`~/OmniDesk/app/config/offline-flows.v1.json` 是应用首启写入的**可编辑流程包副本**
  （2918 字节，无凭证无用户数据），属正常首启行为。
- **保留期限与清理规则已登记：** `docs/OmniDesk-Desktop-Data-Layout.md` §5.1（默认不自动清理；由用户在界面
  逐条/整体清除；验证数据必须在验证后删除；不进 Git；不得与 `quarantine/`、`runtime/` 混放混备）。

### ③ 遗留杂物清理（完成）

删除仓库根 0 字节文件 `当前没有正在执行中的`（2026-10-16 遗留、无内容、无引用；用户批准本计划即视为授权）。

### ④ 提交与推送（完成）

沿用 OD-ARCH-029 的红线扫描流程（逐批暂存 → 人工核对文件清单 → 扫描新增行 → 提交），3 批：

| 批 | 内容 | 提交号 |
| --- | --- | --- |
| 1 | 产品文案修正 + 数据布局文档 §5.1 | `b5fb000` |
| 2 | 状态登记（PLANS/STATUS/REVIEW/decisions/agent-toolchain-retest） | `2a9ddf5` |
| 3 | OD-ARCH-030 证据（脚本 + 结果 JSON + 截图 + README + iterations） | `8645413` |

- 推送：`git push origin main` → `653bf4a..8645413 main -> main`（成功）；
  **远端 refs/heads/main 与本地 HEAD 一致**（`8645413`）。
- 三批扫描均**无关键凭证命中**；提交内容只有产品文案、文档、状态登记与合成证据。
- **工作区现为 0 项**（遗留 0 字节文件已删，本轮改动全部入库）；`git diff --check HEAD` 干净；`runtime/` 无差异。

### 未验证 / 限制

- **真实端点未验证**：全程用回环假端点；真实服务商端点的配置与判定未验证。
- **真实模型质量**：内容来自合成端点。
- **原生 Wayland 未覆盖**：本轮用 XWayland（用户会话），因为原生 Wayland 无合成输入通道（见 OD-ARCH-027 §9）。
- **T-001 双 agent 复测本轮未执行**：本会话的 subagent 仍返回 `Unknown agent: super-plan`；①②③ 已由另一会话的
  **真实 subagent 调用**闭环并登记在 `docs/agent-toolchain-retest.md`（随 ④ 一并提交），
  ④（super-plan → super-worker 派工）因 OpenCode 子代理深度限制**未通过**，两条可选路径待用户决策。
- **工作区：** 开工 7 项 → 本轮结束后仅剩明确说明的未跟踪项（0 字节文件已删，剩余项为提交前状态登记差异）。

## OD-ARCH-030 已制定待批准（2026-10-20，ARCHITECT）

计划登记，非执行记录。OD-ARCH-029 复核通过后制定：真实使用前置收口（端点设置引导验证、使用记录持久化到 ~/OmniDesk 正式根、遗留 0 字节文件清理、文档登记提交推送）。全文见 PLANS.md 顶部。批准后执行；完成后交 ARCHITECT 复核，通过即进入用户首次真实使用。

## OD-ARCH-029：备份收口 + 双 Agent 复测 + 使用记录通道自检（2026-10-20，ARCHITECT 执行）

- **授权：** 用户「批准 OD-ARCH-029」。该计划明确「批准即视同授权其中的 Git 提交与推送动作」。
  开工快照：HEAD `5ffc710`、工作区 682 项（折叠计数 115）、远端 `git@github.com:klzone/omniharness.git`
  （**公开仓库**）、分支 `main`、本机无未推送提交。执行中未合并 `omniharness-ui` worktree，
  未改业务代码、`runtime/`、模型路由、凭证与用户数据。

### 1) 备份收口：分批提交并推送（完成）

- **提交前扫描：** 对全部 681 个待提交文件做敏感信息启发式扫描（凭证形态、私钥、JWT、GitHub/AWS/Slack token、
  真实环境变量赋值、案件类词、身份证/手机号形态、家目录凭证路径），命中 28 条**逐条人工核对**，
  全部为**无害命中**：红线关键词表本身（`task_shell.rs` 的禁入词表、`aiDraft.ts` 的写作规则）、
  Rust 单测里的**合成**号码（`110101199003077777` / `13800138000`，且断言不回显）、
  `workspace_token_for(key)` 之类的**函数调用**、以及文档中「不读取 .credentials.yaml」的说明。
  **无真实凭证、无真实业务材料、无会话日志、无仓库外数据。**
- **分批提交（8 批，每批单独扫描 + 核对文件清单）：**
  | 批 | 内容 | 提交号 |
  | --- | --- | --- |
  | 1 | 桌面前端（`desktop/src`、`public`、`vite.config`、依赖） | `2705148` |
  | 2 | 桌面宿主 Rust（`src-tauri/src`、`tests`、`capabilities`、`tauri.conf`、`examples`、`scripts`） | `90e2731` |
  | 3 | 引擎适配层（`engine/omnidesk_engine`） | `1be6992` |
  | 4 | 流程包与合成样例（`samples/workflows`） | `b5e7a1e` |
  | 5 | 本地脚本（`scripts/dev-ui.sh`、`scripts/lib/port-owner.sh`） | `bfff57f` |
  | 6 | 双 Agent 配置（`.opencode/agents`） | `593c49f` |
  | 7 | 文档、协作状态与 Token 审计（`docs/`、`GOALS/PLANS/STATUS/REVIEW/README`、`audit/`） | `2b4be2d` |
  | 8 | 验收装置与证据（`tests/verification/**`，605 个文件 / 28134 行） | `3f84ca5` |
- **推送：** `git push origin main` → `5ffc710..3f84ca5 main -> main`（成功）。
  远端 `refs/heads/main` 与本地 HEAD **一致**（`3f84ca5`）。
- **无丢失核对：** 提交前 682 项 = 提交后 681 个已提交文件 + 1 个剩余未跟踪项，逐项对齐。
- **剩余未跟踪项（明确说明）：** 仅 1 个文件 `当前没有正在执行中的`——**2026-10-16 遗留的 0 字节空文件**，
  无内容、无引用、非本计划产物；**未提交也未删除**（删除属计划外动作），建议后续单独清理。

### 2) T-001 三项复测：本会话内无法执行（如实记录）

- 实际调用 subagent 工具传 `agent: "super-plan"` → 返回 `Unknown agent: super-plan`（2026-10-20 实测两次）。
  官方文档确认项目级 agent 目录就是 `.opencode/agents/<name>.md`（**路径没写错**）；两个文件创建于
  2026-10-19 22:47 之后，而**本会话的 agent 目录早于文件创建时间**。
- 按计划风险条款「复测延后不阻塞备份」处理：备份已完成；复测步骤仍是
  [docs/agent-toolchain-retest.md](docs/agent-toolchain-retest.md) 的三段照做式提示词，**新开会话**即可跑完。

### 3) 使用记录通道自检（完成）

- 真实窗口（用户会话 XWayland）+ 一次性隔离根 `/tmp/od029-usage/od029-usage-run1` + 回环合成端点：
  进入「项目跟踪」场景 → 用 AT-SPI 勾选使用记录的两个复选框 → 点「记录一次」。
- 结果：记录数 **0 → 1**；落盘文件 `/tmp/od029-usage/.../app-data/logs/usage.jsonl`（**在仓库外**）；
  记录内容**只有枚举与时间**：`record_id / schema / task_type / recorded_at / completed / usable /
  rework_kind / blocker / duration_bucket / reuse_willingness`，**无任何输入、输出、正文、标题或路径**；
  界面读回显示「查看记录（1）」。
  证据：`tests/verification/desktop/evidence/od-arch-029/od029-usage-run1.json` 与两张真实窗口截图。
- 说明：本次点击把默认已勾选的「结果可以直接用」翻转成了 `usable=false`（枚举值，非缺陷）。

## OD-ARCH-029 已制定待批准（2026-10-20，ARCHITECT）

本条为计划登记，非执行记录。OD-ARCH-028 复核通过后，ARCHITECT 判定项目未偏离（首页对话中枢基线 + 三模块真实窗口闭环 + 数据分析后置，与用户 2026-10-20 现场验收结论一致）。当前最高风险为 115 项工作区改动未提交备份；T-001 三项复测与使用记录通道自检合并入 OD-ARCH-029 一并执行。计划全文见 PLANS.md 顶部。

## 任务清单收尾（2026-10-20，ARCHITECT）：Step 5 已完成；T-001 复测受会话限制

- **OD-ARCH-027 Step 5（真实桌面回归）：已完成。** 见本文件「OD-ARCH-027 Step 5」小节与
  `evidence/od-arch-027/README.md` §9。四组（原生 Wayland × DMABUF{1,0}、XWayland × DMABUF{1,0}）
  空闲 60 s 的 `WebKitWebProcess` 均值 **0.08% / 0.17% / 0.15% / 0.28%**，导航 0 超时，5 帧签名 = 1，
  连续输入 `exact_hit`。**原始 ~100% 症状在真实桌面同样未复现，本轮不构成性能提升。**
- **T-001 三项运行复测：本会话内无法执行（已查证并给出可执行清单）。** 实测事实：subagent 工具对
  `super-plan` 返回 `Unknown agent: super-plan`；OpenCode V2 文档确认项目级 agent 目录就是
  `.opencode/agents/<name>.md`（**路径没写错**），而这两个文件是 2026-10-19 22:47 之后创建的，
  **本会话的 agent 目录早于文件创建时间**，所以查不到。复测清单已写成照做式文档：
  [docs/agent-toolchain-retest.md](docs/agent-toolchain-retest.md)（含三段可直接粘贴的提示词与
  「两种结果都要如实记录」的判据）。**新开会话即可发现这两个 agent**，届时可一次跑完。
- **OD-ARCH-028 的「真实使用」：待用户本人执行。** 自动化验收只证明链路通，不产生体验数据。
  操作提示已写成 [docs/pilot-first-real-use.md](docs/pilot-first-real-use.md)（选材 → 读取 → 起草 →
  编辑 → 确认保存 → 用「使用记录」记澄清/返工/耗时；Token 未上报即记未知）。
- **工作区：** 条目 113 → **115**（新增 `evidence/od-arch-027/step5/`、`docs/agent-toolchain-retest.md`、
  `docs/pilot-first-real-use.md`）；`git diff --check HEAD` 干净；`runtime/` 无差异；未提交/推送/打标签。
- **证据刷新说明（如实记录）：** 今天重跑的回归脚本（`run_desktop_e2e.py chain`、`run_workbench_engine_path.py`、
  `verify_engine_cancel.py`、`verify_session_continuity.py`）按其**既有行为**重新生成了
  `tests/verification/**/evidence/*.json`（时间戳、临时目录名等），断言结果与上次一致且**全部通过**；
  这些文件在开工前就已是未提交改动，**没有被覆盖或丢弃**，只是内容刷新为本次运行结果。

## OD-ARCH-027 Step 5：真实桌面回归（2026-10-20，ARCHITECT 执行）

- **授权与范围：** 用户「继续往下推进，直到任务列表中的任务完成」；OD-ARCH-027 Step 5 是任务清单里最后一项
  技术任务。在**用户自己的 Wayland 会话**（GNOME 50.1，`WAYLAND_DISPLAY=wayland-0`）启动真实 OmniDesk 窗口做只读测量；
  只用一次性隔离数据根与回环合成端点，**未写 `~/OmniDesk`**、未用真实材料或凭证、未改任何产品代码、
  未提交/推送/打标签。测量结束即关闭窗口。
- **新增文件：** `tests/verification/desktop/evidence/od-arch-027/step5/scripts/step5-real-desktop.py`（测量脚本，
  只读复用既有 `window_harness` / `run_window_acceptance` / OD-ARCH-027 `_step3lib`）、`step5/step5-*.json`（4 组）、
  `step5/shots-step5-xwayland-dmabuf1/`（真实桌面 4 页 + 5 帧截图）、证据索引 §9。
- **实测（四组分别记录）：**
  - 原生 Wayland（`GDK_BACKEND=wayland`，不经 XWayland）：DMABUF=1 空闲 60 s `WebKitWebProcess` **0.08%**、App 0.07%；
    DMABUF=0 **0.17%**、App 0.08%。导航 P50 0.40–1.22 s、P95 ≤1.63 s、**0 超时**。
  - XWayland（`GDK_BACKEND=x11` + `DISPLAY=:0`）：DMABUF=1 **0.15%**、App 0.12%；DMABUF=0 **0.28%**、App 0.12%。
    导航 P50 0.31–0.81 s、P95 ≤0.85 s、0 超时；5 帧签名 **1**（无持续重绘）；设置页连续输入固定串
    **逐字落盘**（`exact_hit`）；四页截图为真实桌面像素（亮度 244.9–247.4，非黑占比 1.00），
    首页与用户确认的 UI 基线一致。
  - 四组**均无 WebKitGPUProcess**（本机无 DRI3/无 GPU 加速渲染）。
- **通道限制（实测结论，不是假设）：** 原生 Wayland 下 GNOME Shell `ScreenshotArea` D-Bus 返回
  `AccessDenied`、`org.gnome.Shell.Eval` 返回 `(false,'')`、Gdk 抓 X 根窗口为**全黑**；
  故原生 Wayland 组**无像素证据**、**无合成输入通道**，改用 **AT-SPI**（可访问性总线）驱动界面并以文本指纹
  判定页面切换。对应指标由 XWayland 组单独提供，两组不混算。
- **结论纪律：** 原始「空闲约 100%」症状在**真实桌面同样没有复现**（0.08%–0.28%）；本轮**不得**被表述为性能提升。
- **未验证：** release 内嵌产物（仍 debug + vite dev）；真实模型质量；多显示器/缩放/HiDPI；>60 s 长稳；
  真实用户输入法与粘贴；GNOME「减少动画」开关在真实桌面的映射。
- **工作区：** 条目 113 → **114**（新增 `evidence/od-arch-027/step5/` 未跟踪目录）；`git diff --check HEAD` 干净；
  `runtime/` 无差异。

## OD-ARCH-028：阶段二试点（2026-10-19，ARCHITECT 集中执行 + 用户已批准）

- **授权：** 用户「批准执行 OD-ARCH-028」。开工 HEAD `5ffc710`，工作区 111 项差异全部保留；未提交、未推送、未打标签；未改 `runtime/`、模型路由、凭证、真实材料、用户数据与第二 worktree。
- **前置门：** ARCHITECT 对 OD-ARCH-026 作独立复核（源码 + 差异 + 测试），**通过**后继续；四个待核对点结论见 REVIEW.md 同名小节。
- **实际做了什么：** 三个工作场景在真实窗口（一次性 Xvfb `:98` + 一次性隔离根 `/tmp/od028/od028-run1` + 回环合成端点）跑通
  「左侧 Dock 进入 → 载入合成样例 → 明确选择材料文件并读取 → 模型起草 → 人工编辑 → 勾选确认 → 仓库外保存 → 重新打开读回」；
  另做两个负例（端点 400、端点挂起后取消），确认不产生任何成果文件。
- **实测数字：** 周报/写作/项目三场景终态均 `succeeded`，各 1 次模型请求且**材料标记探针全部命中**；
  成果 3 份（427 / 432 / 434 字节）均含标题、模型正文、人工追加与材料来源块；
  失败例终态 `failed`（原样显示上游 400），取消例终态 `cancelled`，两者 `new_artifacts=[]`；
  Rust **143/143**、`tsc` 0、构建通过、`verify_ai_draft` 27/0、`verify_scenario_materials` 5 场景 0 failed、
  桌面任务链/工作台引擎路径/取消语义/会话连续性全部 pass。
- **代码改动：** 仅 `desktop/src-tauri/src/task_shell.rs` 一处错误文案（材料读仓库内文件时不再说「写入」）+ 1 条单测；
  其余产品源码未改，页面沿用用户已确认的 UI 基线。新增验收装置与证据：
  `tests/verification/desktop/evidence/od-arch-028/`（脚本 + JSON + 29 张截图 + README）。
- **未验证/限制：** 真实模型质量；真实 Wayland 桌面、release 内嵌产物、窗口缩放、输入法与粘贴；
  材料内容中的凭证不被检测（白名单按扩展名）；真实使用体验指标（澄清/返工）未采集，Token 引擎未上报 usage（记未知）。
- **工作区：** 条目 111 → **113**（`git status --porcelain`，未跟踪目录折叠计数）：新增 `evidence/od-arch-028/` 目录 + 修改 `task_shell.rs`；该目录下另有 32 个新文件（脚本 1、JSON 1、README 1、截图 29）。`git diff --check HEAD` 干净；`runtime/` 无差异。

## T-001：OmniDesk 开发固定双 Agent「超级Plan / 超级劳工」（2026-10-19，ARCHITECT 本会话直接实施）

- **授权与范围：** 用户 2026-10-19 明确要求创建两个固定模型的项目 agent（超级Plan 负责调研与规划、产出计划交给超级劳工执行），并逐项指定模型：super-plan = `opencode/glm-5.3-flash-free`、super-worker = `opencode/deepseek-v4.1-flash-free`（后者由用户在同一会话追加指定）。计划登记见 PLANS.md「工具链计划 T-001」。本次由 ARCHITECT 直接实施（小型配置任务，未派工 BUILDER）。未提交、未推送、未打标签。
- **改动：** 新增 `.opencode/agents/super-plan.md`（mode: all，model 同上；权限：默认 `*/*` deny 后放行 read/grep/glob、仅 `edit PLANS.md`、只读 git `status/log/diff/show/rev-parse`、仅可 `subagent super-worker`；read 额外拒绝 `**/.env*`、`**/*credentials*`、`**/*secret*`、`**/*.pem`、`**/*.key`；system prompt 内嵌 8 条协作纪律）与 `.opencode/agents/super-worker.md`（mode: all，model deepseek-v4.1-flash-free，不加额外权限限制=build 默认；system prompt 内嵌范围刚性、开工快照、STATUS.md 记录、证据区分、停止条件、边界红线、完成即停）。**未**创建 `opencode.json`、**未**设 `default_agent`、**未**改 provider/model 配置。
- **实测（subagent 工具真实调用）：** ① super-plan 成功被发现并加载；read/glob 可用（docs/ 下 .md 计数 26，`.git/refs/heads/main` 可读）；单条 `git status --porcelain`（workdir=仓库根）**执行成功**；`git rev-parse HEAD` 与 `find docs …` 等未放行命令**被正确拒绝**（首轮 agent 误判为「shell 通道不可用」）；未写入任何文件。② super-worker 成功被发现并加载，自述当前模型为 DeepSeek V4.1 Flash Free；按指令未改文件、未跑 shell、未写 STATUS.md。
- **配置修正：** 首测发现白名单缺 `git rev-parse*`（HEAD 快照必需），已补入 `super-plan.md` 权限。
- **静态校验（部署后自检）：** 两文件 frontmatter 以 YAML 解析通过；super-plan 16 条权限规则顺序正确（`*/*` deny 在首、read/grep/glob allow、edit 仅 `PLANS.md`、只读 git 五条 allow、subagent 仅 `super-worker`、凭证类 read deny 五条在末）；super-worker 无 permissions 段。system prompt 长度：super-plan 1166 字符、super-worker 744 字符。
- **未验证/限制（挂起原因：GLM 与 DeepSeek 双通道限流，且用户 2026-10-19 指示「先别测试 agent，先部署好」——属主动暂缓，非失败）：** 补白名单后 `git rev-parse*` 的复测、含管道/复合命令的匹配语义、super-plan→super-worker 派工端到端链路；agent 的模型绑定为配置事实，运行时实际模型由 OpenCode 解析决定。
- **复测完成记录（2026-10-20 晚，限期解除后，ARCHITECT，subagent 真实调用）：** ① `git rev-parse HEAD` **成功**（白名单放行生效）——首次尝试因 agent 自报 workdir 笔误被 `external_directory` 拒绝（反而证实了外路径防线），补跑后通过；② **管道命令被拒**：`git status --porcelain | wc -l` 返回 `Permission denied: shell`，对比此前单条 `git status --porcelain` 成功 → **白名单只按单条命令前缀匹配，管道整体不匹配**，超级Plan 需逐条运行只读命令，语义安全合理；③ `git diff --check HEAD` **成功**（输出为空）；④ super-plan→super-worker 派工**未通过**：super-plan 自身作为子 agent 时再嵌套派工撞运行时深度限制（默认 1，错误原文 `Subagent depth limit reached (1). Increase "experimental.subagent_depth"...`，该键已在官方 schema 核实）。**④ 的解决（用户批准 B 方案）：** 新增项目级 `.opencode/opencode.json`（内容仅 `$schema` + `experimental.subagent_depth: 2`），配置写入后**同一会话即时生效**；端到端验证：ARCHITECT → super-plan（深度1，GLM-5.3-Flash Free）→ super-worker（深度2，DeepSeek V4.1 Flash Free）**派工成功**，super-worker 只读执行 `git rev-parse HEAD` 并回报（当时 HEAD `581c964`，与 ARCHITECT 本会话独立核实一致）。历史事实：B 之前 ④ 曾被默认深度 1 拦截（`Subagent depth limit reached (1)`）。同时直连验证：super-worker 加载正常并自述模型 `deepseek-v4.1-flash-free`（会话配置自报，非逐 token 凭据）。
- **复测重试记录（2026-10-19 23:5x → 2026-10-20 07:2x，ARCHITECT）：** 用户指示「隔 5 分钟测试一次，重试 99 次再失败就结束任务」。按 5 分钟间隔连续重试 **99 次**，super-plan 与 super-worker 交替探测，**99/99 全部返回 `Rate limit exceeded. Please try again later.`**。**关键诊断：** 同一重试窗口内，**内置 `general` agent（继承本会话模型 MiniMax M3.1 Flash Free）调用成功** → 限流是 **GLM-5.3-Flash Free 与 DeepSeek V4.1 Flash Free 两个模型自身的免费额度**，不是 OpenCode 子 agent 通道故障、不是本项目配置缺陷、不是 agent 发现机制问题。本会话内两个 agent **曾被成功发现并加载**（见上「实测」条），与另一会话记录的 `Unknown agent: super-plan` 现象不一致，**原因未查证**。与另一会话记录的 `Unknown agent: super-plan` 现象**并存，原因未查证**；本会话后续实测发现两 agent 均可被调用（见上「实测」条与「复测完成记录」），两处证据并存。**复测结论（②）**：管道/复合命令匹配语义已明确——白名单只按单条命令前缀匹配；**（④）** super-plan→super-worker 派工链路未通过，原因是子 agent 嵌套深度限制（默认 1），两条路径待用户选择，见 [docs/agent-toolchain-retest.md](docs/agent-toolchain-retest.md)。
- **工作区外部变动（非本会话所为，事实记录）：** 等待期间工作区被**外部提交并推送**——HEAD `5ffc710` → `653bf4a`；两个 agent 文件进入提交 `593c49f`（`chore(opencode): 新增「超级Plan / 超级劳工」两个项目级 agent`）；另有 `2b4be2d` / `3f84ca5` / `653bf4a` 三个后续提交。本会话**未执行任何 commit/push**。当前工作区仅剩 1 个 2026-10-16 遗留的 0 字节未跟踪文件 `当前没有正在执行中的`。
- **工作区：** 新增 `.opencode/` 目录（2 个文件）；既有 111 项差异未触碰；未触碰 `runtime/`、业务代码、模型路由与凭证。

## 窗口外框一体化（frameless，2026-10-19，ARCHITECT 直接实施 + 用户验收）

- **授权：** 用户要求系统标题栏（— □ ×）融入软件界面。只改窗口外框与顶栏控件，不动页面内容/业务逻辑。未提交、未推送、未打标签。
- **改动：** `src-tauri/tauri.conf.json` 主窗口 `decorations: false`；`src-tauri/capabilities/default.json` 增加最小窗口权限；新增 `src/shell/WindowChrome.tsx`（自绘 — □ × + 8 个边缘/角落缩放热区，只调用真实 Tauri 窗口 API）；`TopNav.tsx` 加 `data-tauri-drag-region` 并嵌入控件；`App.tsx` 挂载缩放热区。
- **实测：** `npx tsc --noEmit` 0；`npm run build` 通过；`cargo build --offline` 通过（1m10s）。一次性 Xvfb `:99` + 隔离数据根冒烟：窗口正常打开、顶栏右侧三个控件渲染、页面完整、无黑屏。
- **用户验收（2026-10-19）：** 完整重启（`--stop` 后 `--no-build`）后确认：系统标题栏消失；顶栏拖动、最小化、最大化/还原、关闭均正常。已确认为 UI 基线。
- **未显式验证：** 顶栏双击最大化、四边/四角缩放热区、最大化状态的图标切换未被用户逐项点名确认；Xvfb 无窗口管理器，不能替代真实桌面证据。
- **工作区：** 条目 108 → **111**（`tauri.conf.json`、`capabilities/default.json` 修改 + 新增 `WindowChrome.tsx`）；`git diff --check HEAD` 干净；`runtime/` 无差异。

## OD-ARCH-027（OD-ARCH-026 支线）：桌面端 WebKit 渲染性能治理 — Step 4 无限动画与重渲染治理（2026-10-19）

- **状态：Step 4 已执行完成（含最终默认静态改动），待 ARCHITECT 独立复核；Step 5 起待批准。** 未提交、未推送、未打标签。开工 HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`；`git status --porcelain` 条目**保持 108 项不变**；未改 `runtime/`、Rust（`desktop/src-tauri/**`）、`scripts/dev-ui.sh`、Step 0 归档件与 Step 2/3 证据；`SettingsPanel.tsx` 未修改。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-027/README.md](tests/verification/desktop/evidence/od-arch-027/README.md) §8（单因素对照表、改动清单、60 s 曲线、可复现命令、未验证项）。
- **Phase A 单因素对照（新增 6 个同构页，MiniBrowser，Xvfb :95，DMABUF=1，每页 3 次 × 30 s，共 18 次）：** 只有模糊/玻璃（静态）= **0.00%**；只有动画（无模糊）= **26.00%**（仅 pulse 22.36%、仅 float 23.43%）；动画+模糊 = **98.48%**（≈ `heavy.html` 的 100.17%）；**在与「静态模糊」页唯一差别是多一个 6×6 px 脉冲点的页面上 = 100.19%**。**归因：成本来自「持续重绘 × 大面积模糊」，与动画元素大小无关；模糊静态时几乎为零，动画本身约 1/4 核，两者相乘才到满核。** 这解释了原计划记录的「空闲 ~100%」。
- **Phase B 应用级关键发现：** `theme.css` 的 `.animate-pulse-ambient` / `.animate-float-mascot` **在当前 JSX 中无任何使用**；真正在跑的是 `TopNav` 顶栏状态点的 `animate-pulse`（`engineHealthy` 为真时），而顶栏保留着 Step 2 唯一的 `backdrop-blur-2xl`。用启动前预写 `model_permissions.local=true` 使状态点真正脉冲后实测（60 s）：`WebKitWebProcess` **18.82%**、App（UI 进程）**10.26%**（合计约 29%，**不满足计划 §5 的 <10% 目标**）。加 `prefers-reduced-motion` 后 **0.10% / 0.13%**，状态点颜色（信息）保留。
- **最终改动（按计划 §5 目标）：** `desktop/src/theme.css` 中 `.status-dot-pulse` 默认改为 **`animation: none`**（状态点默认静态、颜色语义保留：emerald=健康 / amber=其它；位置、尺寸、布局、文案不变；键帧 `status-dot-pulse` 保留供显式启用；一行可回退）。其余同前：`@media (prefers-reduced-motion: reduce)` 守卫保留（覆盖 `.animate-pulse-ambient` / `.animate-float-mascot` / `.status-dot-pulse`）；`shell/TopNav.tsx` 抽出 `NavClock` 使 30 s tick 只重渲染时钟，状态点仍按 `engineHealthy` 着色（**本轮未再改 TopNav**）。`SettingsPanel` 输入丢字**未复现**（fast≈25–30 字符/秒与 slow 均逐字写入），**未改**。
- **最终复测（debug+vite，Xvfb :96，DMABUF=1，窗口 1280×820，均 60.07 s）：**
  - 引擎健康 + 默认静态：`WebKitWebProcess` **0.10%**（曲线 0.60→0.30→0.20→0.15→0.12→0.10→0.17→0.15→0.13→0.12→0.11→0.10）、App **0.12%**、网络 **0.00%**、**无 GPU 进程**；5 帧签名 **1/1**（无持续重绘）；状态点 emerald 静态。
  - 出厂默认（权限关闭）：`WebKitWebProcess` **0.03%**、App **0.10%**、网络 0.02%、无 GPU 进程；状态点 amber 静态。
  - 两者均满足 <10% 目标；顶栏裁剪截图证明状态点仍在且颜色仍表达状态（`-chip-6x.png`）。
- **验证：** `npx tsc --noEmit` **0**；`npm run build` **通过**（`index-DfhCJBA6.css` 70.79 kB / `index-D3B_kTff.js` 331.78 kB）；产物中 `.status-dot-pulse{animation:none}`，剩余 `infinite` 仅两个未被 JSX 使用的动画；真实窗口冒烟 4 页 `verdict=ok`、无黑屏（mean_luma 243.78–246.39，dark_fraction ≤0.0072）；时钟格式/位置不变；设置页输入 fast/slow 均 `exact_hit`；与 Step 3 截图对比差异仅限时钟/问候语/一次性数据根路径，**未见布局或样式回退**。
- **导航延迟（同口径）：** 引擎健康 **P50 2.548 / P95 2.597 s**（20 次，0 超时）；出厂默认 **P50 2.567 / P95 2.593 s**（20 次，0 超时）。**该指标受整窗截图读回（约 2.5 s）饱和，不具亚秒分辨力，本轮不据此下性能结论**（详见证据索引 §8.4 注）。
- **未验证/限制：** 导航延迟口径限制（同上）；未做「healthy 状态下仅隐藏状态点」的变体（机制由 `iso-navpulse` 受控对照支持）；输入注入受装置限制（约 25–30 字符/秒）；真实 Wayland/GPU/release 构建与真实桌面「减少动画」开关；Phase A 是受控页面证据，不直接等于真实应用；最长连续采样 60 s。详见证据索引 §8.6。
- **检查：** `git diff --check HEAD` 干净；`git status --porcelain runtime/` 为空；无 `.rs` 改动；一次性 Xvfb 已回收、无遗留进程与端口占用。

## OD-ARCH-027（OD-ARCH-026 支线）：桌面端 WebKit 渲染性能治理 — Step 3 DMABUF/显示后端独立对照（2026-10-19）

- **状态：Step 3 已执行完成，待 ARCHITECT 独立复核；Step 4 起待批准。零产品代码改动。** 未提交、未推送、未打标签。开工 HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`；`git status --porcelain` 条目**保持 108 项不变**；未改 `desktop/src/**`、`desktop/src-tauri/**`、`scripts/dev-ui.sh`、`runtime/`、Step 0 归档件与 Step 2 证据；**`window_harness.py` 未改动**（DMABUF 默认仍为 1，应用级用 `AppSession.env_extra` 覆盖）。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-027/README.md](tests/verification/desktop/evidence/od-arch-027/README.md) §7（变量、每配置次数、汇总表、异常帧、可复现命令、未验证项）。
- **变量：** `WEBKIT_DISABLE_DMABUF_RENDERER` = `1`（现状默认）vs `0`；显示后端**只有一次性 Xvfb + `GDK_BACKEND=x11`**。**Wayland↔XWayland 对照未做**（沙箱无 GPU，且不得由 BUILDER 连接用户真实 Wayland 会话）——留待 Step 5，本步**没有产生任何 Wayland 数据**。
- **A 组（MiniBrowser，Xvfb :93，`heavy.html`/`simple.html` 各 3 次 × 30 s）：** `heavy` 在 `=1` 为 100.17/100.15/100.18%，`=0` 为 100.22/100.26/98.76%；`simple` 两档均 ≈0%。两档都**没有 WebKitGPUProcess**、都只有 1 个渲染进程 + 1 个网络进程；`/proc/<pid>/environ` 确认变量已传入（`1`/`0`）。**两档无差异。**
- **B 组（真实窗口，Xvfb :94，debug+vite，1280×820，空闲 60.05 s ×6）：** `WebKitWebProcess` `=1` 0.07/0.07/0.08% vs `=0` 0.07/0.08/0.08%；App 主进程 0.08–0.10%；网络进程 0.00%；**两档都无 GPU 进程**；导航 P50/P95（全 20 次）`=1` 1.737/2.578、1.940/2.563、1.726/2.562 s，`=0` 1.724/2.566、1.728/2.563、1.729/2.565 s；设置页输入固定串**6/6 次一次命中、未观察到丢字**。**两档无可辨差异。**
- **画面异常：** 对 step3 下**全部 82 张截图**做完内容区像素判据 —— **疑似黑屏/空白 0 张、闪烁 0 张**（逐帧见 `step3/step3-frame-analysis.json`）。
- **环境探针（受控补充）：** 本环境 **`WEBKIT_DISABLE_DMABUF_RENDERER` 不可控** —— `unset`/`0`/`1` 三处理在 `heavy.html` 上都是 ~100%（两轮独立探针一致），而 `WEBKIT_FORCE_DMABUF_RENDERER=1` 才改变代价（**62.07%**，首轮 60.27%）。日志显示 `libEGL warning: DRI3 error: Could not get DRI3 device` → 该 Xvfb 下 EGL/加速渲染不可用；WebKit2GTK 2.52.6；`/dev/dri` 存在但本用户不在 `video`/`render` 组。**因此 A/B 组的「无差异」是构造性空结果，不能读成「DMABUF 在真实桌面不重要」。** 强制启用时 CPU 更低，但「绘制推进」只是近似指标（可排除停摆、不能证明更流畅），**不得据此判定为改进**。
- **未验证/限制：** 真实 GPU/Wayland 下的 DMABUF 行为与黑屏风险（Step 5）；`WEBKIT_DISABLE_DMABUF_RENDERER` 的真实语义与为何在本环境无效；计划 §1.2 的「特效页 DMABUF 34.5%」**未复现**（原因未验证）；release 构建；`WEBKIT_FORCE_DMABUF_RENDERER` 的观感/撕裂；B 组测的是含 Step 2 CSS 改动的当前工作树，绝对值不可与 Step 2 之前直接比较。详见证据索引 §7.8。
- **检查：** `git diff --check HEAD` 干净；`git status --porcelain runtime/` 为空；无产品代码 / `dev-ui.sh` 默认值改动；运行前后均校验系统上只有本次这一个 WebKit 应用，一次性 Xvfb 已回收。

## OD-ARCH-027（OD-ARCH-026 支线）：桌面端 WebKit 渲染性能治理 — Step 2 CSS 性能优化（2026-10-19）

- **状态：Step 2 已执行完成，待 ARCHITECT 独立复核；Step 3 起待批准。** 未提交、未推送、未打标签。开工 HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`；工作区未提交/未跟踪条目保持 **108 项不变**（Step 2 新增文件都落在原本就已未跟踪的 `evidence/od-arch-027/` 内）；未改动 `runtime/`、模型路由、凭证、用户数据、真实材料、第二 worktree，也未改 `scripts/dev-ui.sh` 默认值；**零 Rust 改动**。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-027/README.md](tests/verification/desktop/evidence/od-arch-027/README.md) §6（含改动文件清单、落盘路径、可复现命令、未验证项）。
- **改动（只动样式与既有类，可独立回退）：** `App.tsx` 三个全屏 `blur-[120–140px]` 光斑 → 静态径向渐变光晕（`theme.css` 新增 `.ambient-glow-*`）；`theme.css` `.plate-sculpted`、`workbench-ui.css` `.wb-v2 .pane`、`offline-flows.css` `.ofl-v2 .pane` 删除卡片级 `backdrop-filter`；`shell/HomeDashboard.tsx` 三处卡片去掉 `backdrop-blur-md/xl`；`shell/Dock.tsx` 去掉 `backdrop-blur-2xl/md`。**保留唯一关键视觉层：`shell/TopNav.tsx` 顶栏 `backdrop-blur-2xl`。** 未动结构与交互，未动任何无限动画或定时重渲染（Step 4 范围）。
- **实测（debug + vite dev，Xvfb :91，`WEBKIT_DISABLE_DMABUF_RENDERER=1`，窗口 1280×820，空闲窗口 60.03 s）：** `WebKitWebProcess` before **0.33%** → after **0.07%**；App 主进程 0.07% → 0.07%；网络进程 0.02% → 0.00%；无 GPU 进程。导航延迟（含截图读回开销，每段 n=5，全 20 次）before P50/P95 **1.821 / 3.350 s** → after **1.941 / 2.577 s**。
- **受控补充（MiniBrowser，Xvfb :92，DMABUF=1，每页 40 s）：** `simple.html` 0.0% / 0.0%；`heavy.html`（Step 0 归档原件，模糊+动画叠加）**100.2% / 100.2%**；`heavy-after.html`（复刻改后 CSS、动画保持）**19.6%**。
- **未验证/限制：** ① **当前工作区首页空闲 CPU 本来就低（0.33%），计划 §1.2 的「约 100%」未复现**，before/after 的 CPU 差异在噪声量级，**不宣称性能提升**；② before/after 各只跑了 1 次，导航 P50 波动大于差异；③ CSS 模糊在「有无限动画」的页面上是放大器（100.2%→19.6%），但未做动画单因素对照，剩余成本不能归给单一因素；④ 真实 Wayland 桌面、release 构建、输入丢字与动画治理（Step 4）均未验证。详见证据索引 §6.4。
- **检查：** 前端 `npx tsc --noEmit` 0、`npm run build` 通过（构建产物已确认不再含 `blur(1xx px)`）；`git diff --check HEAD` 干净；`git status --porcelain runtime/` 为空；无 `.rs` 文件被本次改动。

> **ARCHITECT 独立复核（2026-10-19）：OD-ARCH-027 Step 2 按获批范围通过（附关键限定）。** 我独立核对：6 个改动文件与描述一致（其中 5 个开工前即未跟踪，`git diff` 看不到增量，用开工前读取记录核对）；证据 JSON/截图/脚本齐全且时序正确（before 16:37 → 改动 16:42–16:43 → after 16:51）；我自行复跑 MiniBrowser 对照（simple 0.0% / heavy 100.3% / heavy-after 26.3%）与应用级短跑（空闲 25 s，`WebKitWebProcess` 0.32%），与执行方结果同量级；逐页前后截图差异 ≤0.22%（基本只差时钟），无视觉回退；`tsc` 0、build 通过、`git diff --check` 干净、`runtime/` 无差异、108 项工作区差异保留。**关键限定：** 计划 §1.2 的「空闲 `WebKitWebProcess` 约 100%」在当前工作区未复现（before 0.33%，我复跑 0.32%），因此**本步没有可宣称的应用级性能收益**，不得当作性能提升上报；原始成因需 Step 4 的动画单因素对照与真实桌面复测判定，不得据本步结案。**交付缺陷：** `step2-perf.py` 显式传 `--shots-dir` 时为 `str` 而非 `Path`，会 `AttributeError`（默认路径不受影响，本次证据未用该参数），脚本作为交付物应修。详见 REVIEW.md 顶部。未提交、未推送、未打标签。

## OD-ARCH-027（OD-ARCH-026 支线）：桌面端 WebKit 渲染性能治理 — 计划建立与证据归档（2026-10-19）

- **状态：Step 0（证据归档）与 Step 1（计划文档）完成；Step 2 已执行完成（见上一节），待 ARCHITECT 复核；Step 3 起待批准。** 未提交、未推送、未打标签。
- **计划文档：** [docs/OmniDesk-Desktop-Render-Performance-Plan.md](docs/OmniDesk-Desktop-Render-Performance-Plan.md)（范围、非目标、P0–P3 拆分、Step 0–5 顺序与门禁、验收阈值、测量规范、回退方式、停止条件）。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-027/README.md](tests/verification/desktop/evidence/od-arch-027/README.md)。
- **Step 0 归档清点：** 可复现脚本（`/tmp/od025-perf.py`、`perf2.py`、`perf3.py`）与对照页（`baseline/{simple,heavy}.html`）**仍存在**，已逐字归档；运行环境事实与两张运行截图已归档。**原始测量结果未落盘**（当时只打印到标准输出，会话输出已被清理）——按要求**不补造**，基线在 Step 2 重新采集并落盘。
- **本支线口径：** CSS 特效与 DMABUF 作为两个独立变量分别对照；不把相关性当作全部根因；不全面删除玻璃特效；不顺带重构 UI 架构；Xvfb 与真实桌面结果分别记录。
- **未改动：** 产品源码（含 CSS）、`runtime/`、模型路由、凭证、用户数据、真实材料、第二 worktree 与 `scripts/dev-ui.sh` 默认值。

## OD-ARCH-026：AI 对话为中枢的产品校正与三项工作场景闭环（OpenCode 执行记录，2026-10-19）

- **执行状态：已执行完成；待 ARCHITECT 独立复核。** 未提交、未推送、未打标签。开工 HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`；既有未提交/未跟踪改动全部保留（工作区条目 104→106：新增 `verify_scenario_materials.py` 与 `evidence/od-arch-026/`）；未改动 `runtime/`、模型路由、凭证、用户数据、真实材料与第二 worktree。
- **授权：** 用户「批准执行 OD-ARCH-026」（先经澄清：指 PLANS.md 中已有的那份产品校正计划，非界面性能修复）。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-026/README.md](tests/verification/desktop/evidence/od-arch-026/README.md)。
- **产品改动：** 首页改为对话中枢（`shell/HomeChat.tsx` 新增，准入由宿主 `engine_admission_status` 驱动；关闭时如实显示并给出「去设置」）；Dock 新增周报与随记、文案写作、项目跟踪三个入口；三个场景改为**模型起草**（选择材料文件 → 生成初稿 → 人工编辑确认 → 保存），确定性核对单独成栏且不作为成果保存；流程包新增可选可编辑规则 `draft.rules/sections`；宿主新增只读单文件材料读取 `task_read_material`（绝对路径、普通文件、扩展名白名单、512 KiB 上限、UTF-8、拒绝 DSH/agent/仓库路径）。
- **实测结果：** 前端 `tsc --noEmit` 0、`npm run build` 通过；Rust 当前源码新编译全测 **142/142**；`verify_ai_draft` 27/0；新增 `verify_scenario_materials` 3 提示词/5 场景 0 failed（三类场景材料进入模型请求、结果可见；失败与取消）；`run_desktop_e2e chain` pass；`run_workbench_engine_path` pass；`verify_engine_cancel` 34+8/0；`verify_session_continuity` 37/0；真实窗口（一次性 Xvfb + 隔离数据根）冒烟 + 周报场景纵向闭环 pass（界面选择材料文件 → 模型初稿 → 人工确认 → 保存，产物含模型正文与材料来源 sha256）；`git diff --check HEAD` clean；`runtime/` 无差异。
- **未验证/限制：** 真实模型质量与风格；GUI 只逐点走完「周报」一个场景（另两场景的引擎链路已覆盖）；材料为原样文本，无结构化解析，也未实现模型调用预制 Python；跨平台、真实显示器/窗口管理器、系统级安装未验证。详见证据索引 §3。

### 首页版式回归：对话为主 + 顶部三模块（2026-10-19，执行方记录，非审查结论）

- **状态：已执行完成，待 ARCHITECT 复核。** 仍属 OD-ARCH-026（用户明确「继续 026」，不另开编号）。未提交、未推送、未打标签；工作区条目仍为 **108**（新增/删除文件都在原本未跟踪的 `desktop/src/shell/` 与 `evidence/od-arch-026/` 内）。
- **授权口径（用户已批准三项）：** ① **默认静态**（不用 `animate-float-mascot` / `animate-pulse` / 卡片 `backdrop-blur-md`，保持 OD-ARCH-027 Step 2/4 成果）；② **不伪造数据**（今日提醒沿用诚实空态，日程只给三周日期骨架）；③ **权限提示压缩**为输入框底部一行状态。
- **改动：** 新增 `desktop/src/shell/HomeCanvas.tsx`（问候 → 一行三模块：机器人 `/mascot.png` + 「今日重点推进」+「日程节律」→ 下半屏整幅 `HomeChat`）；`desktop/src/App.tsx` 首页改渲染 `HomeCanvas`；**删除** `desktop/src/shell/HomeDashboard.tsx`（删除前确认只有 `App.tsx` 引用）；`desktop/src/shell/HomeChat.tsx` 只改展示（删空态说明与三个快捷入口、整块权限 banner → 输入框底部一行状态、「模型权限：已开启 · 本机」/「未开启，提交已关闭」+「去设置」、`workspaceNote` 仅在非空时一行小字）。未改提交/轮询/取消/准入逻辑、`Dock.tsx`、`TopNav.tsx`、Rust、`runtime/`、`scripts/dev-ui.sh` 与 Step 0/2/3/4 证据。
- **三项口径落实：** 吉祥物/状态点/卡片均静态（层次由 `bg-white/80` + 描边 + 阴影表达）；问候后只写「· 欢迎回到 OmniDesk」，不写「今天有 N 项…」；两卡分别标注「未接入数据源」「未同步」，正文不给任何业务数字；权限细节（端点主机、已开启类别、仅公开/合成内容的边界）压缩进该行 `title`。
- **实测：** `npx tsc --noEmit` **0**；`npm run build` **通过**（CSS 70.59 kB / JS 331.26 kB）；源码 `className` 与构建产物均无 `animate-float-mascot` / `animate-pulse` / `backdrop-blur-md` 及右栏文案（构建 CSS 无 `.animate-pulse{`、`.backdrop-blur-md{`；保留 TopNav 的 `.backdrop-blur-2xl{`）。真实窗口（一次性 Xvfb `:97` + 隔离数据根 + 回环合成端点，1280×820）：首页连续 5 帧**签名 = 1**（无持续重绘）；输入并提交一条合成提示后任务台账 `state=succeeded / stop_reason=end_turn / content=合成回复内容`，界面显示「模型输出 · 未复核 · 已完成」；首页/任务工作台/离线/设置四页逐页截图无黑屏空白、无右栏；空闲 60 s `WebKitWebProcess` **0.08%**、App **0.15%**、网络 0.00%、无 GPU 进程。`git diff --check HEAD` clean；`git status --porcelain runtime/` 为空；无 `.rs` 改动；一次性 Xvfb/应用/引擎进程均已回收（端口 5273 空闲；注意 9230 是开工前就存在的用户 DSH 进程，未触碰）。
- **未验证/限制：** 回复来自合成端点（非真实模型质量）；仅 Xvfb/1280×820（真实 Wayland、release 内嵌产物、窗口缩放未验证）；XTEST 只能注入 ASCII（输入法/粘贴、长对话滚动未验证）；未做像素级设计比对。截图与数据见 [home-layout](tests/verification/desktop/evidence/od-arch-026/home-layout/env.json) 与证据索引 §4。

> **ARCHITECT 独立复核（2026-10-19）：OD-ARCH-026 首页版式回归按批准口径通过。** 我核对源码与构建产物：右栏整列与「对话中枢」说明/快捷入口均已消失；首页 = 问候 → 机器人 + 今日重点推进 + 日程节律 → 下半屏整幅对话；三项口径（默认静态 / 不伪造数据 / 权限一行）均落实，无 `animate-float-mascot` / `animate-pulse` / 卡片 `backdrop-blur-md`；`tsc` 0、build 通过（我独立重跑）；工作区仍 108 项、`runtime/` 无差异、无 `.rs` 改动。窗口与 60 s 空闲测量采用执行方落盘证据（5 帧签名=1、`WebKitWebProcess` 0.08%），未独立重跑。未验证：真实模型质量、真实 Wayland/release、窗口缩放、输入法/粘贴、像素级设计比对。**后续 OD-ARCH-026 功能复核以本版首页为准。** 详见 REVIEW.md 顶部。**同日微调：** 按用户反馈把「模型权限」状态行移到输入框**下方**，并与新增的「Token 上下文：used / size」同处一行；Token 只取引擎 `progress/usage` 上报的真实数值，未上报显示「—」，不自行估算；`tsc`/build 复跑通过。**同日布局修正：** 根容器 `h-screen`、`HomeChat` 根 `flex-1 min-h-0`、消息区 `min-h-0`，顶栏/今日提醒/日程/输入框/状态行固定，仅对话历史滚动；滚动条改浅色并设 `color-scheme: light`。

## OD-ARCH-025 ARCHITECT 独立复核（2026-10-19）

- **结论：技术验证通过，产品目标未通过。** 自动化证据证明合成链路、窗口操作、保存与重启等底层行为；没有证明目标首页和三类 AI 工作场景已实现。
- 当前首页仍是仪表盘，聊天与任务工作台分离；周报、文案、项目跟踪的主流程仍有本地确定性汇总，模型生成仅是单独的合成测试路径。用户明确指出的 UI 和功能偏差成立。
- 详细审查见 REVIEW.md 顶部；下一步方案 OD-ARCH-026 已列于 PLANS.md，待用户批准。
- 本次未改产品代码，未合并 UI worktree，未提交或推送；HEAD 5ffc710，所有现有工作区差异保留。

> ARCHITECT 独立复核（2026-10-19）：OD-ARCH-024 第四轮按获批范围通过。窗口关闭复用 stop_engine()，等待期间不占宿主锁；Rust 134/134 有执行方当前源码编译证据，本轮因无 Rust 工具链未独立重跑。真实窗口销毁、真实模型和跨平台未验证。结论及证据见 REVIEW.md 顶部与 OD-ARCH-024 证据索引。

> ARCHITECT 独立复核更新（2026-10-19）：OD-ARCH-024 第三轮补测仍部分通过、暂不验收。生产 `WindowEvent::Destroyed` 回调在持有宿主锁时调用 `bridge.shutdown()`；耗时关闭会阻塞状态/结果/取消命令，绕过 `stop_engine()` 的锁外收尾边界。需在现有获批范围内统一关闭入口并补宿主级回归。详见 REVIEW.md 顶部及 PLANS.md 的 OD-ARCH-024。

> ARCHITECT 独立复核（2026-10-19）：OD-ARCH-024 仍部分通过。执行方补测覆盖了真实合成任务和关闭标志置位后的快速拒绝，但未覆盖“任务命令先检查未关闭、关闭流程随后先取得 supervisor 锁”的竞态；该路径可能等关闭后才返回普通错误。定向补测要求见 REVIEW.md 顶部和 PLANS.md 的 OD-ARCH-024。

> ARCHITECT 独立复核：OD-ARCH-023 不通过。with_engine_call / with_call_gate 在持有生命周期锁时再次调用会重锁的 is_current()，受保护命令会自锁；状态/结果/取消入口也未纳入生命周期边界。修复计划 OD-ARCH-024 已由用户批准并执行完成（见下方顶部记录），待 ARCHITECT 复核。详见 REVIEW.md 与 PLANS.md。

> OD-ARCH-022 部分通过，暂不验收：端点说明、21/21 分类自测、前端构建、隔离安装证据通过；发现撤权与并发引擎启动之间存在旧准入结果竞态。该竞态的修复实现 OD-ARCH-023 未通过独立复核，不能视为关闭。

> OD-ARCH-021 已由 ARCHITECT 于 2026-10-18 独立复核：部分通过，暂不整体验收。其问题曾由 OD-ARCH-022 处理；OD-022 现已由 ARCHITECT 复核为部分通过，仍有权限切换并发竞态，修复计划 OD-ARCH-023 待批准。

> OD-ARCH-020 的 BUILDER 执行记录已由 ARCHITECT 部分复核：安装包产出、成果目录设置、离线成果保存/重开范围通过；用户可配置模型权限与 AI 任务闭环未完成，后续计划见 PLANS.md OD-ARCH-021。

> 更新日期：2026-10-18。此文件记录 BUILDER 的执行事实，不替代 ARCHITECT 的独立 Review。OD-ARCH-018 已由 ARCHITECT 有条件通过：三个离线场景在 Linux 虚拟桌面中完成保存与重开；Rust 完整测试由 BUILDER 报告 92/92，当时 ARCHITECT 未能独立重跑。真实模型生成、真实窗口取消、真实工作材料、跨平台及长期使用未验证。模型准入仍关闭。OD-ARCH-019 已由 ARCHITECT 有条件通过复核。OD-ARCH-020 已由 BUILDER 执行完成，下一步为 ARCHITECT 独立复核。

## OD-ARCH-025：个人试用前桌面闭环验收（OpenCode 执行记录，2026-10-19）

- **执行状态：已执行完成；待 ARCHITECT 独立复核。** 未提交、未推送、未打标签。开工 HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`，既有未提交/未跟踪改动全部保留；未改动 `runtime/`、模型路由、凭证、用户数据、真实材料、第二 worktree，**未改动任何 Rust 或前端产品源码**。
- **授权：** 用户本轮「授权执行 OD-ARCH-025」。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-025/README.md](tests/verification/desktop/evidence/od-arch-025/README.md)。

### 结论（执行方记录，非审查结论）

在**一次性隔离数据根 + 一次性 Xvfb 显示**里，用真实 OmniDesk 桌面窗口（1280×820）与合成引擎走通了四个阶段，
四个阶段 verdict 全部 pass（逐条判据与截图见证据索引）：

1. **阶段 A 主闭环**：用户在设置界面把成果目录改到 `/tmp/od25a` 并生效；开启「本机模型」权限后，
   工作台「统计 → 生成解释提示词 → 用引擎生成解释 → 人工确认 → 保存」走通（合成端点，任务终态 `succeeded`，
   来源固定记为 `model_unverified`）；离线「项目跟进」流程本地确定性生成、人工核对后保存；
   **两份成果都落在用户设置的目录**：`20261019-035655-320dae745102.md`（1714 B）与
   `offline-project-management-20261019-035831.md`（1059 B）；成果列表可只读重开。
2. **阶段 B 重启**：关闭并重启后，成果目录设置、模型权限、引擎任务台账、两份成果文件都在，成果仍可重开。
3. **阶段 C 可控慢关闭**：合成端点置为 `hang`、任务持续进行中时，先确认界面仍可切换场景，再按窗口管理器正常路径
   （`WM_DELETE_WINDOW`，等价于点标题栏关闭）**真实关闭窗口**：进程 **0.25 s** 自行退出（未被强杀，`exit_code=0`），
   显示上不再有窗口，**无孤儿引擎进程**；被打断的任务台账为 `starting`，**未留下伪造的成功终态**。
4. **阶段 D 安装入口**：按当前源码重建 `.deb`，解包后 `usr/bin/omnidesk-desktop` 能启动真实窗口；沿用同一数据根时
   设置、成果目录与既有成果都保持；会话与任务页按只读元数据展示（`恢复状态未验证`、`读取状态`，不自动重放）。

### 修改文件

- 新增：`tests/verification/desktop/window_harness.py`（XTEST 注入 + GDK 稳定抓图 + 无窗口管理器适配 + 隔离启动/回收）、
  `tests/verification/desktop/run_window_acceptance.py`（四阶段验收装置）、
  `tests/verification/desktop/evidence/od-arch-025/`（`README.md`、`stage-{A,B,C,D}.json`、`shots/{A,B,C,D}/`、
  10 个检查日志）。
- 修改：`STATUS.md`（本节）、`PLANS.md`（状态与证据链接）、`REVIEW.md`（执行方记录，非结论）。
- 未改：`runtime/`、`desktop/src/**`、`desktop/src-tauri/src/**`、`REVIEW.md` 的既有审查结论、模型路由、凭证、第二 worktree。

### 实际运行的检查（本轮实测）

| 检查 | 结果 |
| --- | --- |
| 窗口四阶段验收 | A/B/C/D 全 pass（44 张截图 + 4 个 JSON） |
| Rust 离线全测（当前源码重新编译） | 134 passed / 0 failed / 0 ignored |
| 前端构建 / 类型检查 | 成功 / EXIT=0 |
| 桌面引擎链路（success/failure/cancel） | 全 pass |
| 工作台引擎路径 | pass |
| AI 初稿 / 取消 / 会话连续性 | 22 / 34+8 / 37 项，0 失败 |
| `git diff --check HEAD` | 无空白错误 |
| `git diff --stat HEAD -- runtime/` | 无差异 |

### 环境事实与限制（如实记录）

- 本机为 **Wayland 会话**（`GDK_BACKEND=wayland`、`WAYLAND_DISPLAY=wayland-0`）：装置显式强制 `GDK_BACKEND=x11`
  并清除 `WAYLAND_DISPLAY`，只用一次性 Xvfb 显示，避免连到用户桌面会话。
- 无窗口管理器：需显式 `XSetInputFocus`（否则键盘事件被丢弃）、截图前需 1px 尺寸微调强制重绘（否则像素滞后）、
  需回收 GTK 原生弹窗；滚轮滚动需慢节奏（10 次/轮、1 s 间隔）才稳定，且步长有 ±30 px 漂移，
  因此按钮改为**按主色在帧内定位后再点击**，并以文件事实（设置文件、成果文件、任务台账）判定成败。
- **未验证**：真实模型文风与质量、真实工作材料、跨平台、真实显示器与真实窗口管理器、
  系统级 `dpkg -i` 安装、真实工作区（非隔离根）下的落盘与用户数据迁移。
- 关闭耗时（0.25 s）只代表本机这次运行；`hang` 场景下窗口关闭路径没有等待慢关闭，与 OD-ARCH-024 的设计一致。

## OD-ARCH-024：修复生命周期自锁并补齐宿主并发验证（OpenCode 执行记录，2026-10-18）

- **执行状态：已执行完成；待 ARCHITECT 独立复核。** 未提交、未推送、未打标签。开工 HEAD `5ffc710`，既有未提交/未跟踪改动全部保留；`REVIEW.md` 未改；未改动 `runtime/`、前端、模型路由、凭证、用户数据、真实材料与第二 worktree。
- **授权：** 用户本轮「批准 OD-ARCH-024 执行」。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-024/README.md](tests/verification/desktop/evidence/od-arch-024/README.md)。

### 已完成

1. **消除自锁（不削弱世代校验）。** 新增 `CallGuard::current_epoch()`（非重入：直接读守卫已持有的状态）；`with_engine_call` 与 `with_call_gate` 改用它，不再于持守卫时调用会重锁的 `EngineLifecycle::is_current()`；`is_current()` 保留并加注“不得在持守卫时调用”。准入凭据仍绑定权限世代，未盖章凭据仍被 `ensure_initialized` 拒绝。
2. **状态/结果/取消语义明确并落实。** 三者**不启动引擎、不取得生命周期守卫、不检查屏障**（否则权限变更会把取消堵住），只共用宿主锁；权限变更仅在**有界**（≤10 s）的关闭等待期间持有该锁。改造为 `task_status_impl` / `task_result_impl` / `cancel_task_impl`，命令与测试共用同一实现。
3. **宿主包装器组合测试（4 条）。** 使用真实包装器 + 假引擎 + 真实路由分类：描述包装器闭环且不自锁；非启动包装器闭环且不启动引擎；调用在途时权限变更无法开始；屏障期间包装器被拒而状态/结果/取消仍可用。**实测证明**：临时还原旧写法后 `describe_wrapper_returns_without_self_lock` 以“自锁/超时”在 90 s 时失败，恢复修复版后通过。
4. **源码接线测试扩展。** 新增对三个任务命令的检查（不得取得守卫、必须复用 `*_impl`）。
5. **回归。** Rust 离线测试（当前源码新编译）、前端类型检查与构建、路由自测、四个既有桌面验证脚本、`git diff --check HEAD`、`runtime/` 无差异检查全部通过。

### 修改文件

- 修改：`desktop/src-tauri/src/engine_lifecycle.rs`、`desktop/src-tauri/src/lib.rs`；`PLANS.md`、`STATUS.md`。
- 新增：`tests/verification/desktop/evidence/od-arch-024/`（`README.md`、`cargo-test-offline.log`、`dump-model-route-self-test.json`）。
- 修正：`tests/verification/desktop/evidence/od-arch-023/README.md` 增加“覆盖结论修正”说明。
- 未改：`runtime/`、前端、`REVIEW.md`、`docs/decisions.md`、模型路由、凭证、第二 worktree。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cargo test --offline`（当前源码新编译；二进制 23:44:08 > 源码 23:43:41） | 112 lib + 2 admission + 13 host + 3 e2e = **130 通过 / 0 失败**，无警告 | Rust 离线回归 |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 成功 | 前端构建 |
| `dump-model-route.mjs --self-test` | 21/21 | 脚本运行 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass | 脚本运行 |
| `verify_engine_cancel.py` / `verify_ai_draft.py` / `verify_session_continuity.py` | pass / pass / 37 项 0 失败 | 脚本运行 |
| `git diff --check HEAD` / `git diff --exit-code HEAD -- runtime/` | 退出码 0 / 无差异 | 文件事实 |

### 未验证与限制

1. **真实模型输出与质量**未验证。
2. **停止失败路径**仍以屏障逻辑与组合测试覆盖，**未在真实窗口制造真实子进程无法退出**。
3. **端到端窗口验证**未做（未在真实 Tauri 窗口触发“变更中提交/取消”）。
4. 宿主包装器测试使用**假引擎**：验证宿主生命周期与接线，不验证真实 DSH/ACP 行为。
5. 仅 Linux；跨平台未验证。

### 补测与收尾（2026-10-19）

- **动机：** ARCHITECT 要求用**可控假引擎建立真实运行中的任务**，确定性并发触发关闭与取消，
  验证 status/result/cancel 的**实际行为**，并落实一致语义（不能只用不存在的任务 ID 或只看错误码）。
- **发现并修复：** `bridge.shutdown()` 需 `&mut EngineBridge`，**全程持有宿主锁**，`stop()` 又全程持
  supervisor 锁 → 关闭期间的取消会**排队到关闭之后**，再命中 `!initialized` 报出误导性错误
  （旧文案“引擎尚未初始化，请先调用 engine_describe”）。
- **落实的行为：**
  1. `EngineLifecycle` 增加**无锁可读**的原子信号 `shutting_down`；
  2. 新增 `stop_engine()`：取 supervisor 句柄（宿主锁短暂持有）→ **先**置位“关闭中” → `stop()`
     （**关闭等待期间不持宿主锁**）→ `mark_stopped()` → 清除信号；权限变更与 `engine_shutdown` 共用；
  3. `cancel`/`status`/`result` 在关闭进行中**立即**返回 `engine_shutting_down`，消息明确
     “**请求未发送**”，不再排队；关闭**之前**发起的取消仍**送达引擎**；
  4. `engine_bridge::rpc` 的未初始化错误改为如实描述“未运行或已停止（可能正被关闭），本次请求未发送”。
- **测试：** 新增 2 条宿主级测试（真实提交建立运行中任务后取消 → 假引擎方法日志确有
  `omni.cancel_task`；4 秒关闭窗口内取消/状态/结果均 <1.5 s 返回明确结论，且 `omni.shutdown`
  之后不再出现 `omni.cancel_task`）；源码接线测试扩展（三个 impl 须含 `is_shutting_down()`；
  `stop_engine` 须先置位信号再 `stop`）。假引擎夹具同步扩展（status/result/cancel + 方法日志 +
  关闭延迟）。
- **反向验证：** 临时移除“关闭中”检查后，该补测以
  `取消不得排队等待关闭（等了 4.031304621s）` 失败；恢复修复版后通过。
- **本轮检查：** `cargo test --offline`（当前源码新编译）**132 通过 / 0 失败**（114 lib + 2 admission
  + 13 host + 3 e2e）；`tsc --noEmit` 0；`npm run build` 成功；路由自测 21/21；
  `run_desktop_e2e.py chain` 全 pass；取消 34+8、草稿 22、会话 37 全 pass；
  `git diff --check HEAD` 0；`runtime/` 无差异。
- **补充证据：** `cargo-test-offline-supplement.log`；详见
  [OD-ARCH-024 证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)「补测与收尾」。
- **本轮新增改动文件：** `desktop/src-tauri/src/engine_lifecycle.rs`、`engine_bridge.rs`、
  `engine_supervisor.rs`、`lib.rs`、`tests/fixtures/fake_engine/omnidesk_engine/__main__.py`。

### 第三轮：可线性化的最小并发边界（2026-10-19，用户批准后的定向补测/修正）

- **复现的缺口（ARCHITECT）：** 上一轮把“关闭中”做成 `EngineLifecycle` 的**无锁原子标志**，命令
  **先读标志、再取 supervisor 锁**；关闭流程可能在这两步之间取得该锁，导致请求**排队到关闭结束**
  才拿到普通错误（`engine_unavailable`）。
- **结构性修正：** 判定搬进 supervisor 锁内，与“摘除通道”同一临界区完成，窗口在结构上消失。
  1. `EngineSupervisor` 增加 `stopping` 与 `begin_stop()`（锁内：置位 + `channel.take()`）、
     `end_stop()`、`finish_stop(channel, grace)`（**锁外**慢路径）；`with_channel` 在锁内拒绝；
     `start()` 在关闭未收尾时拒绝启动新实例；
  2. `EngineBridge::rpc` 持锁后**先**判 `is_stopping()`（稳定返回 `engine_shutting_down`，含
     `sent_to_engine=false`），再判 `initialized`；`shutdown()` 改两段式；
  3. `stop_engine()` 两段式：宿主锁/supervisor 锁都只短暂持有，关闭等待**不占任何锁**；
  4. **移除** `EngineLifecycle` 的无锁原子标志，避免两套机制；三个 `*_impl` 去掉锁外预检。
- **新增确定性时序测试（本计划第 (1)(2) 项）：**
  `task_commands_are_refused_when_shutdown_wins_the_engine_lock` 用 `#[cfg(test)]` 同步点把取消
  停在“**已进入发送路径、尚未取得 supervisor 锁**”，让关闭先完成 `begin_stop()` 并进入耗时阶段，
  再放行：取消**未送达**、**<1.5 s** 返回明确结论；状态/结果同样。另有
  `task_commands_return_promptly_when_shutdown_is_in_progress` 与送达路径测试
  `cancel_is_delivered_before_shutdown_for_a_running_task`。
- **反向验证（变异，均已还原）：** A 把耗时关闭放回锁内 → 补测以
  `取消也不得排队等待（等了 8.042621669s）` 失败；C 移除两处判定 → 退回误导性的
  `engine_unavailable` 并失败；B 只移除 `with_channel` 判定 → 仍通过（双层防御独立有效）。
- **本轮检查：** `cargo test --offline`（当前源码新编译，二进制 mtime 06:50:43 > 源码 06:50:41）
  **133 通过 / 0 失败**（115 lib + 2 admission + 13 host + 3 e2e），无警告；`tsc --noEmit` 0；
  `npm run build` 成功；路由自测 21/21；桌面 success/failure/cancel 链全 pass；取消 34+8、
  草稿 22、会话 37 全 pass；`git diff --check HEAD` 0；`runtime/` 无差异；HEAD 仍 `5ffc710`。
- **证据：** `cargo-test-offline-round3.log`；详见
  [OD-ARCH-024 证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)「第三轮」。
- **本轮改动文件：** `desktop/src-tauri/src/engine_ipc.rs`、`engine_supervisor.rs`、
  `engine_bridge.rs`、`engine_lifecycle.rs`、`lib.rs`。
- **未验证：** 关闭窗口由假引擎延迟制造；未在真实 Tauri 窗口触发该并发；真实模型与跨平台未验证。

### 第四轮：窗口销毁关闭路径补齐（2026-10-19，用户批准的计划）

- **缺口（ARCHITECT 复核）：** `lib.rs` 的 `WindowEvent::Destroyed` 回调先 `state.lock()` 再
  `host.bridge.shutdown(5s)`——`bridge.shutdown()` 自身虽已把协议关闭移到 supervisor 锁外，但
  **调用方手里的宿主锁**会一直占着，窗口关闭期间状态/结果/取消仍被排队。
- **修正：** 新增 `on_window_destroyed_shutdown(&SharedHost)`，**复用共享入口 `stop_engine()`**；
  窗口销毁回调只调它，不再自行加锁、不再直连 `bridge.shutdown()`。至此关闭入口唯一
  （命令路径与窗口销毁路径 → `stop_engine()`），两条路径都不在等待关闭时持有宿主锁。
  `EngineBridge::shutdown()` 文档补充“调用方不得在持有宿主锁时等待本方法”。
- **新增回归：** `window_destroy_shutdown_does_not_hold_host_lock_while_waiting`（真实在途任务 +
  8 s 关闭窗口，后台执行**生产回调所用入口**）：关闭等待期间宿主锁 <1.5 s 可取得；取消/状态/结果
  <1.5 s 返回明确结论；`omni.shutdown` 之后引擎未收到取消；收尾后 `health()` 不再报告在运行。
  接线测试新增“生产前缀不得出现 `bridge.shutdown(`”“窗口回调必须走共享入口且不得含 `state.lock()`”。
- **反向验证（变异，已还原）：** 把该入口改回“持宿主锁 + `bridge.shutdown`”后，行为测试以
  `关闭等待期间宿主锁应可立刻取得（等了 8.041135831s）` 失败，接线测试同时失败。
- **本轮检查：** `cargo test --offline`（当前源码新编译，二进制 mtime 07:35:34 > 源码 07:35:29）
  **134 通过 / 0 失败**（116 lib + 2 admission + 13 host + 3 e2e），无警告；`tsc --noEmit` 0；
  `npm run build` 成功；路由自测 21/21；桌面链全 pass；取消 34+8、草稿 22、会话 37 全 pass；
  `git diff --check HEAD` 0；`runtime/` 无差异；HEAD 仍 `5ffc710`。
- **证据：** `cargo-test-offline-round4.log`；详见
  [OD-ARCH-024 证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)「第四轮」。
- **本轮改动文件：** `desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/engine_bridge.rs`。
- **未验证：** 窗口销毁回调仍在事件循环线程同步等待关闭（与修正前一致，本轮只保证不占宿主锁）；
  未在真实窗口实际销毁验证；真实模型与跨平台未验证。

## OD-ARCH-023：修复模型权限变更与引擎启动之间的并发竞态（OpenCode 执行记录，2026-10-18）

> ⚠️ **本条记录已被 ARCHITECT 复核为不通过（自锁 + 覆盖不足），并由 OD-ARCH-024 修复。**
> 原记录中的“126 通过”等运行事实仍成立，但其中的**覆盖结论**（例如“源码级接线测试证明所有
> 启动入口受保护”“组合路径已覆盖”）**过度表述**：当时 `with_engine_call`/`with_call_gate`
> 实际会自锁，且没有宿主级组合测试。以 OD-ARCH-024 与 REVIEW.md 顶部结论为准。
> 本文件仅保留该条原始执行事实，不再作为当前状态依据。

> BUILDER 执行记录保留如下；ARCHITECT 独立复核结论为不通过。具体阻断证据见 REVIEW.md 顶部，后续修复计划见 PLANS.md 中 OD-ARCH-024。

- **执行状态：已执行完成；待 ARCHITECT 独立复核。** 未提交、未推送、未打标签。开工 HEAD `5ffc710`，既有未提交/未跟踪改动全部保留；`REVIEW.md` 未改；未改动 `runtime/`、前端、模型路由、凭证与 DSH 数据；未改第二 worktree。
- **授权：** 用户本轮「OD-ARCH-023 授权开始运行」。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-023/README.md](tests/verification/desktop/evidence/od-arch-023/README.md)。

### 已完成

1. **串行化边界。** 新增 `engine_lifecycle.rs`：`begin_call()`（引擎调用守卫；屏障期间拒绝；存活期间权限变更无法开始）与 `begin_change()`（权限变更独占；开始即置位屏障，成功清除、失败保留，并推进权限世代）。
2. **入口全覆盖。** 六个会启动引擎的命令（提交、描述、会话列表、会话恢复、任务台账、任务恢复）改为经 `with_engine_call`，在同一守卫内完成「准入（盖章）→ 启动引擎 → 动作」；有副作用但不启动引擎的 `task_prepare_workspace_dir` 改为经 `with_call_gate`。
3. **凭据绑定世代。** `AdmissionGrant` 携带权限世代；未盖章凭据在 `EngineBridge::ensure_initialized` 被拒（`grant_not_stamped`），锁外取得的准入结果无法启动引擎。
4. **加锁顺序统一为「生命周期 → 宿主」**；`engine_admission_status` 先释放宿主锁再读屏障，避免反向死锁。删除旧的 `permission_barrier` 与纯函数失败映射。
5. **并发回归。** 新增 5 条 `EngineLifecycle` 确定性并发测试（含“调用持守卫期间撤权无法开始”的核心时序与跨线程互斥）、1 条假引擎集成测试（未盖章凭据不得启动引擎）、1 条目录准备入口的屏障拒绝测试、1 条源码级接线测试（六入口必须经守卫、准入判定只允许出现在两个守卫内）。
6. **回归。** Rust 全部离线测试、前端类型检查与构建、路由自测、四个既有桌面验证脚本、`git diff --check HEAD`、`runtime/` 无差异检查全部通过。

### 修改文件

- 新增：`desktop/src-tauri/src/engine_lifecycle.rs`；`tests/verification/desktop/evidence/od-arch-023/`（`README.md`、`cargo-test-offline.log`、`dump-model-route-self-test.json`）。
- 修改：`desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/admission.rs`、`desktop/src-tauri/src/engine_bridge.rs`；`PLANS.md`、`STATUS.md`。
- 未改：`runtime/`、前端、`REVIEW.md`、`docs/decisions.md`、模型路由、凭证、第二 worktree。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cargo test --offline` | 108 lib + 2 admission + 13 host + 3 e2e = **126 通过 / 0 失败**，无警告 | Rust 离线回归 |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 成功 | 前端构建 |
| `dump-model-route.mjs --self-test` | 21/21 | 脚本运行 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass | 脚本运行 |
| `verify_engine_cancel.py` / `verify_ai_draft.py` / `verify_session_continuity.py` | pass / pass / 37 项 0 失败 | 脚本运行 |
| `git diff --check HEAD` / `git diff --exit-code HEAD -- runtime/` | 退出码 0 / 无差异 | 文件事实 |

### 未验证与限制

1. **真实模型输出与质量**未验证。
2. **停止失败路径**仍以屏障逻辑与假引擎/逻辑测试覆盖，**未在真实窗口制造真实子进程无法退出的场景**。
3. **端到端窗口验证**未做（未在真实 Tauri 窗口触发“变更中提交”）。
4. 仅 Linux；跨平台未验证。

## OD-ARCH-022：端点去向说明、权限切换一致性与安装验收（OpenCode 执行记录，2026-10-18）

- **执行状态：BUILDER 已执行完成；ARCHITECT 于 2026-10-18 独立复核为部分通过，暂不验收。** 未提交、未推送、未打标签。开工 HEAD `5ffc710`，既有未提交/未跟踪改动全部保留；BUILDER 未改 `REVIEW.md`；未改动 `runtime/`、`~/.dsh`、`~/.agents`、模型路由与凭证；未改第二 worktree `/mnt/data/Code/omniharness-ui`。
- **授权：** 用户本轮「授权你来执行：OD-ARCH-022」。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-022/README.md](tests/verification/desktop/evidence/od-arch-022/README.md)。

### 已完成

1. **端点去向说明不再承诺组织归属。** 类别由 `local/internal/external/unknown` 改为 `local/private/public/unknown`：只回答“是不是本机回环”；非回环一律在说明文字里声明“不代表所属组织或实际转发路径”。`leaves_device` 语义限定为“是否离开本机”（非回环为 `null`）。改动 `dump-model-route.mjs`、`route_classifier.rs`、`settings.rs`、`admission.rs`、`lib.rs`、`types.ts`、`SettingsPanel.tsx`、`EngineTechSlice.tsx`；旧设置文件的 `internal/external` 键经 `serde(alias)` 迁移到 `private/public`（有迁移单测）。
2. **权限切换失败关闭。** 顺序改为“持久化 → 停止旧引擎 → 确认停止才报生效”；停止失败返回 `permission_change_incomplete` 并置位宿主屏障，`engine_submit_task` 直接拒绝（不启动引擎、不发送输入）；`engine_shutdown` 确认停止后清除屏障；设置页显示失败原因与“停止引擎以恢复”按钮。新增 2 条单测覆盖成功/失败语义。
3. **用例数校准。** OD-ARCH-021 的“22 项”为误记（实际 18，已在 OD-021 证据注明）；本次改名并新增 3 条“私有地址不声称组织归属”断言后，`--self-test` 实际为 **21 项**。
4. **隔离安装验收。** 在用户级隔离根 `/tmp/od022-install` 用 `dpkg --root/--admindir/--instdir --force-not-root --force-depends` 完成安装（状态 `ii`）→ `gtk-launch OmniDesk` 从桌面入口启动并正常渲染 → `dpkg -r` 卸载；启动器与 desktop 条目随包生成并随卸载移除。全程 `uid=1000`，未提权、未改系统配置。
5. **回归。** 前端类型检查与构建、Rust 完整离线测试、路由自测、引擎链路/取消/草稿/会话四个既有验证脚本、`git diff --check HEAD`、`runtime/` 无差异检查全部通过。

### 修改文件

- 修改：`desktop/src-tauri/scripts/dump-model-route.mjs`；`desktop/src-tauri/src/{route_classifier,settings,admission,lib}.rs`；`desktop/src-tauri/tests/{admission_classification,host_integration}.rs`；`desktop/src-tauri/examples/evaluate_offline_scenarios.rs`；`desktop/src/types.ts`、`desktop/src/api.ts`、`desktop/src/SettingsPanel.tsx`、`desktop/src/shell/EngineTechSlice.tsx`；`PLANS.md`、`STATUS.md`。
- 新增：`tests/verification/desktop/evidence/od-arch-022/`（`README.md`、`installed-launch.png`、`dump-model-route-self-test.json`）。
- 未改：`runtime/`、`REVIEW.md`、`docs/decisions.md`、模型路由、凭证、DSH 数据、第二 worktree。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 成功（JS 317.96 kB / CSS 71.95 kB） | 前端构建 |
| `cargo test --offline` | 103 lib + 2 admission + 13 host + 3 e2e = **121 通过 / 0 失败** | Rust 离线回归 |
| `node src-tauri/scripts/dump-model-route.mjs --self-test` | **21/21**，`ok: true` | 脚本运行 |
| `run_desktop_e2e.py chain` | success/failure/cancel 全 pass | 脚本运行（合成端点） |
| `verify_engine_cancel.py` | `RESULT: pass` | 脚本运行 |
| `verify_ai_draft.py` | `RESULT: pass` | 脚本运行 |
| `verify_session_continuity.py` | 37 项 0 失败 | 脚本运行 |
| `.deb` 隔离安装 → 入口启动 → 卸载 | 通过 | 运行 |
| `git diff --check HEAD` / `git diff --exit-code HEAD -- runtime/` | 退出码 0 / 无差异 | 文件事实 |

### 未验证与限制

1. **真实模型输出与质量**未验证（未调用真实端点、未读凭证）。
2. **停止失败路径**仅以纯函数单测覆盖，未在真实窗口制造子进程无法退出的场景。
3. **屏障的端到端窗口验证**未做（未在真实窗口触发一次“停止失败”）。
4. 安装为**用户级隔离根模拟**，不等于系统级安装；AppImage 仍未产出；仅 Linux。

## OD-ARCH-021：可配置模型权限与 AI 任务闭环、桌面安装验收（BUILDER 执行记录，2026-10-18）

- **执行状态：BUILDER 已执行完成；待 ARCHITECT 独立审查。** 未提交、未推送、未打标签。开工 HEAD `5ffc710`，原有未提交/未跟踪改动全部保留，暂存区为空；`REVIEW.md` 未改；未改动 `runtime/`、`~/.dsh`、`~/.agents`、模型路由与凭证。
- **授权：** 用户本轮「OD-ARCH-021 授权执行」。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-021/README.md](tests/verification/desktop/evidence/od-arch-021/README.md)（含 19 张截图与运行事实）。

### 已完成

1. **端点分类与准入。** 从 DSH 非秘密配置读出 provider、model 与 `baseURL`，分为本机（回环地址）、单位内部（私有网段）、外部（公网域名）与未知四类；未知类别没有可开启开关，宿主始终拒绝。首次使用三类权限全部关闭，设置页显示类别、判定依据与当前判定。新增 `desktop/src-tauri/scripts/dump-model-route.mjs`、`src-tauri/src/{route_classifier,settings,admission}.rs` 与 `desktop/src/SettingsPanel.tsx`。
2. **宿主强制执行。** 提交前在宿主侧判断准入，未开启时引擎不启动、输入不发送；前端不再硬编码允许；IPC 接受可选 `user_input_text`，红线扫描使用原始输入而非仅提示词。
3. **权限持久化与撤销。** `settings.v1.json` 保存三类开关，逐项开启与「撤销全部模型权限」即时生效；重启后保留。
4. **AI 任务闭环的真实窗口验证（合成端点）：** 统计 → 引擎流式解释 → 人工确认门禁 → 保存到隔离本地目录；失败、取消、未知端点、红线拒绝分别实测。
5. **安装包。** `.deb` 重建成功：`desktop/src-tauri/target/release/bundle/deb/OmniDesk_0.1.0_amd64.deb`，2.46 MiB；`dpkg-deb -f` 显示 `Package: omni-desk`、`Version: 0.1.0`、`Architecture: amd64`；包含 32/64/128/256/512 图标、`usr/share/applications/OmniDesk.desktop`、`usr/bin/omnidesk-desktop`；`ldd` 无缺失。解包到 `/tmp/od021-evidence/unpack3` 后在 `Xvfb :98` 启动，首页正常渲染。**未安装到系统，AppImage 仍未产出。**

### 修改文件

- 新增：`desktop/src-tauri/scripts/dump-model-route.mjs`、`desktop/src-tauri/src/route_classifier.rs`、`desktop/src-tauri/src/settings.rs`、`desktop/src-tauri/src/admission.rs`、`desktop/src/SettingsPanel.tsx`、`tests/verification/desktop/evidence/od-arch-021/`（README 与 `shots/`）。
- 修改：`desktop/src/App.tsx`、`workbench.tsx`、`api.ts`、`types.ts`、`styles.css`、`main.tsx`；`desktop/src-tauri/src/lib.rs`、`engine_bridge.rs`、`engine_ipc.rs`、`engine_supervisor.rs`、`task_shell.rs`、`tests/host_integration.rs`、`tests/task_shell_e2e.rs`、`Cargo.toml`/打包配置；`PLANS.md`、`STATUS.md`。
- 未改：`runtime/`、`REVIEW.md`、`docs/decisions.md`、模型路由、凭证、原版 DSH 数据。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 成功 | 前端构建 |
| `cargo test --offline` | 98 lib + 2 admission + 13 host + 3 e2e，全通过 | Rust 完整离线回归 |
| `dump-model-route.mjs --self-test` | 18/18，0 失败，`RESULT: pass`（ARCHITECT 独立复跑） | 脚本运行 |
| `verify_ai_draft.py` | 22/22 | 隔离合成根脚本运行 |
| `verify_session_continuity.py` | 37/37 | 隔离合成根脚本运行 |
| `verify_engine_cancel.py` | 34/34 + 8/8 | 编译真实源码后执行 |
| `run_desktop_e2e.py chain` | 成功/失败/取消均符合预期 | 脚本运行 |
| `run_engine_adapter_scenarios.py E19` | pass（截断流） | 脚本运行 |
| `.deb` 构建与 `dpkg-deb` 核对 | 成功 | 构建产物 |
| 解包启动 | 首页渲染正常 | 真实窗口运行 |
| `git diff --check HEAD` / `git diff --exit-code HEAD -- runtime/` | 退出码 0 | 文件事实 |

### 未验证与限制

1. **真实模型质量未验证。** 未调用任何真实供应商端点，未读取凭证；AI 输出质量与真实费用无结论。
2. **半截流 UI 未在真实窗口观察到。** 仅有 `E19` 适配器脚本证据（截断流），未在窗口内制造中途断流。
3. **头像点击卡死（已知缺陷，未修）。** 该 WebKit/Xvfb 环境中点击右上头像稳定无响应（CPU 100%）；临时禁用模糊样式的注入未解决，`dist/index.html` 已恢复；同一前端在 Chrome 中正常。属 Tauri/WebKit 渲染问题，根因未查明，未改源码。建议另立计划。
4. 仅 Linux 虚拟桌面；Windows/macOS 未验证；AppImage 未产出；`.deb` 仅解包启动，未在真实桌面安装。
5. 红线扫描为启发式关键词检查，不是合规判断，也不代表授权或脱敏保证。
6. 成果目录仍为路径输入框，无原生目录选择器。

### 与计划的一致性

计划 (1)(2)(3) 已实现并实测；(4) 的成功、失败、取消、未知端点、红线拒绝均有真实窗口证据，首次关闭/开启/撤销/持久化已实测；(5) 完成构建与解包启动，未做系统安装（计划允许用户级/隔离环境，此处停在解包启动）；(6) 回归全部通过，`runtime/` 无差异。未发现需改架构方向的计划冲突。

## OD-ARCH-020：个人试用版闭环——安装入口、成果目录设置与 AI 工作流程（BUILDER 执行记录，2026-10-18）

- **执行状态：BUILDER 已完成；待 ARCHITECT 独立审查。** 未提交、未推送、未打标签。开工 HEAD `5ffc710`，原有未提交/未跟踪改动全部保留，暂存区为空；未改动 `/mnt/data/Code/omniharness-ui` worktree。`REVIEW.md` 未改，留给 ARCHITECT。
- **授权：** 用户本轮明确「授权执行OD-ARCH-020任务」。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`。隔离运行根 `/tmp/od020-evidence/run/od-dev-87ca1cd8`，虚拟桌面 `Xvfb :98`。

### 已完成

1. **桌面发行入口与打包。** Linux `.deb` 实际产出：`desktop/src-tauri/target/release/bundle/deb/OmniDesk_0.1.0_amd64.deb`；已核对包名 `omni-desk`、版本 `0.1.0`。应用名、图标与启动器条目随包生成。**AppImage 未产出**，打包在 `linuxdeploy` 阶段失败，不得宣称通过。`.deb` 未安装，未改系统或桌面配置。
2. **成果目录设置。** 设置页显示当前生效目录、来源（用户选择/默认/环境变量覆盖）与设置文件位置；保存采用路径输入框，**未实现 Tauri 原生目录选择器**。拒绝仓库内路径、DSH/Agent/Codex 私有目录、相对路径与应用数据根本身；可写性以一次性探针检查并清理。更改只影响之后新建的成果，明确提示旧文件不移动、不覆盖、不删除。应用配置目录与成果目录分开。
3. **重启保留。** 不设 `OMNIDESK_LOCAL_DATA_ROOT` 重启后，界面仍显示 `/tmp/od020-evidence/artifacts`、来源“你在设置中选择的目录”；`settings.v1.json` 内容为 `{"schema":"omnidesk.settings.v1","artifact_root":"/tmp/od020-evidence/artifacts"}`。首次加载有短暂“位置未解析”，等待配置读取后恢复，判定为页面时序而非缺陷。
4. **成果保存、重开与旧文件保护。** 合成项目跟进样例 4 项：done 1 / in_progress 1 / blocked 1 / not_started 1，逾期 2；保存为 `offline-project-management-20261018-090326.md`（1077 字节）。清单可列出并以“已重新打开的成果（只读）”重开。预置哨兵文件 `pre-existing-od020.txt` 保存前后 SHA-256 均为 `aa9c07cf…a306`，未被移动、覆盖或删除。
5. **同标题保存的真实行为。** 再次用同一标题保存生成新文件 `offline-project-management-20261018-090342.md`（1077 字节），旧文件未变。源码中文件名由秒级时间戳生成，`create_new(true)` 仅在同一秒内重名时拒绝；原界面“已存在同名文件时拒绝保存”文字不准确，已改为“每次保存按时间戳新建文件，不覆盖已有文件”。同一秒并发重名拒绝只有代码静态确认，未在窗口内单独制造。
6. **AI 初稿入口。** 准入关闭状态下提交，界面显示 `阶段：refused`、任务标识“尚未取得”、宿主状态“未取得”，并明确“当前入口未获端点隔离确认，已拒绝提交。输入未发送到任何模型。”输入仍保留在本地界面，未写入成果文件。
7. **轻量使用记录。** `usage.jsonl` 两条记录，均不含输入/输出正文、标题、路径、会话、模型或凭证；界面可查看、逐条删除与清空。
8. **文档定点修订。** `docs/OmniDesk-Desktop-Architecture.md` 第 6 节去除“desktop 尚未创建 / 工具链缺失 / 构建依赖缺失”等过时表述，改为现状、工具链入口、`.deb` 路径与 AppImage 未产出、跨平台未验证；未重写历史结论。

### 修改文件

- 桌面与宿主编译相关（本轮前序工作）：`desktop/src/App.tsx`、`workbench.tsx`、`OfflineFlows.tsx`、`SettingsPanel.tsx`、`UsageLog.tsx`、`api.ts`、`types.ts`、`styles.css`、`main.tsx`、`vite.config.ts`、`package.json`、`package-lock.json`；`desktop/src-tauri/src/lib.rs`、`task_shell.rs`、`engine_bridge.rs`、`engine_ipc.rs`、`engine_supervisor.rs`、`tests/host_integration.rs`、`tests/task_shell_e2e.rs` 及打包配置。
- 本轮收尾修改：`desktop/src/OfflineFlows.tsx`（保存文案）、`docs/OmniDesk-Desktop-Architecture.md`、`PLANS.md`、`STATUS.md`、`tests/verification/desktop/evidence/od-arch-020/README.md`。
- 新增证据目录：`tests/verification/desktop/evidence/od-arch-020/`（`README.md`、`shots/`、`logs/`）。
- 未修改：`runtime/`、`REVIEW.md`、`docs/decisions.md`（本轮）、模型路由与凭证、原版 DSH 数据。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 成功 | 前端构建 |
| `cargo test --offline` | 89 lib + 13 host_integration + 3 e2e，全通过 | Rust 完整离线回归 |
| `verify_session_continuity.py` | 37/37，`RESULT: pass` | 隔离合成根脚本运行 |
| `verify_ai_draft.py` | 20/20，`failed=0`，`RESULT: pass` | 隔离合成根脚本运行 |
| `verify_engine_cancel.py` | 34/34 + 8/8，`RESULT: pass` | 编译真实源码后执行 |
| `.deb` 构建 | 成功，产物存在 | 构建产物 + `dpkg-deb` 元数据 |
| AppImage 构建 | 失败（`linuxdeploy`），无产物 | 构建日志 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0 | 文件事实 |

Rust 测试前需 `eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)"`。

### 未验证事项与限制

1. **真实模型生成与输出质量未验证。** 端点准入常量仍为 `false`，未读取凭证、未调用任何模型；三个场景的模型撰写步骤均标为受准入阻塞。
2. **取消/失败重试的真实模型路径未验证。** 仅验证了准入拒绝，未在真实模型任务中触发取消或超时。
3. 成果目录为路径输入框，不是原生目录选择器；非法路径拒绝与探针清理已测，但未覆盖所有文件系统边界（如跨设备、只读挂载）。
4. AppImage 未生成；`.deb` 未安装，安装后启动器与图标未在真实桌面环境验证。仅 Linux 虚拟桌面，Windows/macOS 未验证。
5. 同标题同一秒并发重名拒绝仅有源码依据；跨进程并发保存未做压力测试。
6. 一个无害编译警告：`task_shell.rs` 中 `LOCAL_ROOT_ENV is never used`。
7. 证据目录中的截图与日志已排除私人路径、凭证、会话正文与 `app.log`。

### 与计划的一致性

计划第 2、3 步（发行包、成果目录设置）已实现；第 4 步三个场景的人工确认、保存与重开已实测，但 AI 提交链路因准入关闭只验证了拒绝分支；第 5 步客户可配置准入与第 6 步严格默认开关、撤销、未知端点等细分交互**未实现、未验证**，准入仍为编译期常量关闭；第 7 步安装后启动、真实模型提交/取消/错误未完成。上述缺口不改变当前“仅离线合成试用”的结论，需由 ARCHITECT 判定是否将 OD-ARCH-020 验收拆分为“离线闭环通过、AI 准入与模型联调另立计划”。未发现需改架构方向的计划冲突。

### 建议 ARCHITECT 关注

1. 计划第 5 步要求“客户可配置模型准入、外部默认关闭但可逐项开启”，当前实现仍是编译期硬关闭，是否作为本计划未完成项退回，或拆分为后续独立计划，需要明确。
2. AppImage 与 `.deb` 安装后启动是否属于本次发行验收的硬门槛；当前仅产出 `.deb`。
3. 路径输入框与原生目录选择器的体验差异是否可接受。
4. 保存文案与实际时间戳命名策略现已一致，但底层“同一秒重名拒绝”与用户预期“同名即拒绝”仍不同，是否需要改为按标题查重需产品决策。

## OD-ARCH-019：可编辑离线流程包与轻量个人试用记录（BUILDER 执行记录，2026-10-18）

- **执行状态：BUILDER 已完成，ARCHITECT 于 2026-10-18 有条件通过复核。** 未提交、未推送、未打标签。端点准入两侧常量仍为 `false`，`runtime/` 无差异。独立结论见 [REVIEW.md](REVIEW.md) 顶部；证据索引见 [tests/verification/desktop/evidence/od-arch-019/README.md](tests/verification/desktop/evidence/od-arch-019/README.md)。
- **授权：** 用户本轮明确「批准 OD-ARCH-019执行」。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`；开工时 82 项未提交/未跟踪改动全部保留，暂存区为空。`REVIEW.md` 未改，留给 ARCHITECT。

### 已完成

1. **流程包加载器（Rust）。** 新增 `flow_pack.rs`，schema `omnidesk.offline-flows.v1`，运行时文件为应用数据根下 `config/offline-flows.v1.json`。首次自动创建默认包，之后不覆盖用户修改。整包校验：未知版本、未知字段、缺项、空字段、超长字段、URL/脚本/iframe 文本、相对目录一律拒绝并返回可读错误，不静默回退。
2. **三场景接线。** `task_shell.rs` 新增 `*_with` 渲染函数读取流程包；原函数委托默认包以保持兼容。日期校验、状态计数、逾期判定逻辑未改。`lib.rs` 注册 `flow_pack` 模块与 `offline_flow_pack` 命令，失败时返回 `ok:false` 且 `pack_error:true`。
3. **前端读取流程包。** 三场景的标签、字段说明、占位符、样例按钮、结果标题均来自配置；页面显示流程包路径与错误，提供“重载流程包”；配置不可用时拒绝运行。
4. **轻量使用记录。** 新增 `UsageLog.tsx`：可选记录场景、完成、可用、返工类别、障碍、粗耗时与“是否愿意再用”；可查看、逐条删除、全部清空。字段严格白名单，不含输入/输出正文、标题、路径、会话、模型或凭证。
5. **本轮新发现并修复的界面缺陷。** 切换场景后“成果标题”仍保留上一场景内容，可能把旧标题带入新成果。`resetOutput()` 增加标题复位；窗口内切场景重跑后标题为空。
6. **“愿意再用”字段补齐与旧记录兼容。** 补齐计划要求的该字段（Rust、TS、界面、测试）；区间模型显示与写入均已实测。为旧的 `usage.jsonl` 记录加 `#[serde(default)]`，缺少该字段时默认为 `false`，并增加兼容测试，避免历史记录导致整文件读取失败。

### 修改文件

- 新增 `desktop/src-tauri/src/flow_pack.rs`、`samples/workflows/offline-flows.v1.json`、`desktop/src/OfflineFlows.tsx`、`desktop/src/UsageLog.tsx`、`desktop/src/offline-flows.css`。
- 修改 `desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/task_shell.rs`、`desktop/src-tauri/tests/task_shell_e2e.rs`、`desktop/src/api.ts`、`desktop/src/types.ts`、`desktop/src/workbench.tsx`。
- 新增 `tests/verification/desktop/evidence/od-arch-019/`（`README.md`、`logs/`、`shots/`）。
- `STATUS.md`、`PLANS.md`。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cargo test --offline` | 100 通过（84 lib + 13 host_integration + 3 e2e）/ 0 失败 | Rust 完整离线回归 |
| `cargo run --offline --example evaluate_offline_scenarios` | 三场景通过；准入拒绝生效；日期边界正确 | Rust 运行实测 |
| `npx --no-install tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 退出码 0 | 构建实测 |
| `verify_session_continuity.py` | 37/37 通过 | 隔离合成根验证 |
| `verify_engine_cancel.py` | 34/34 + 8/8，RESULT pass | 编译真实源码后执行 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0 | 文件事实 |

### 隔离窗口实测结果

在 `/tmp/od019-evidence/run/od-dev-87ca1cd8` 的 `Xvfb :97` 中启动真实 Tauri 窗口，逐项验证：

- 流程包在 `app-data/config/offline-flows.v1.json` 首次生成；修改周报、项目、文案的标签与结果标题后，点“重载流程包”界面即时生效。
- 周报样例生成 4 项，done 2 / pending 1 / blocked 1；成果保存到 `local-data/offline-weekly-report-20261018-065807.md`（999 字节），列表可列出、只读重开内容一致。
- 项目跟进样例 4 项，done 1 / in_progress 1 / blocked 1 / not_started 1，逾期 2，列出受阻与待确认。
- 一般文案在没有匹配模板时明确写“未使用模板（按用途组织）”，并列出待确认项。
- 把 schema 改为 `omnidesk.offline-flows.v99` 后重载，界面显示可读错误并拒绝运行，未用默认文案冒充成功；恢复有效包后重载成功。
- 使用记录写入一条后界面显示摘要；逐条删除后 `usage.jsonl` 为 0 字节。
- 勾选“类似任务还愿意再用”后记录，`usage.jsonl` 写入 `"reuse_willingness":true`，表格显示“愿意再用：是”；随后删除记录，文件回到 0 字节。
- 所有场景均显示“受准入阻塞、未验证：本次未调用任何模型”。
- 修复后切场景重跑，成果标题为空，不再残留上一场景内容。

### 未验证事项

1. 真实模型撰写：端点准入仍为 `false`，未调用任何模型。
2. 真实工作材料：本轮全部使用合成样例。
3. 跨平台：仅在 Linux 虚拟桌面验证。
4. 长期使用记录：只走了一条合成记录的写入、展示与删除；旧格式缺字段兼容仅由单元测试覆盖。
5. 流程包只覆盖离线流程的表达层，不支持脚本、外链或模型调用。

### 与计划的一致性

未发现 Plan 与实际代码冲突。计划要求“首次创建后不覆盖用户修改”，实现为仅在文件缺失时写默认包；计划要求“非法配置不静默回退”，实现为整包拒绝并在界面显示错误。没有扩建 GUI、Core 层或插件体系。

### 建议 ARCHITECT 关注

1. 流程包校验是否覆盖了所有可被滥用的文本入口，尤其是结果标题与章节名注入 Markdown 结构的情况。
2. 使用记录的“场景”目前取当前 Tab，是否需要在记录后锁定当时场景。
3. 成果标题复位是否应同时清空表单输入，还是只清结果相关状态。

## OD-ARCH-018：个人试用准备与桌面交互验收（BUILDER 执行记录，2026-10-18）

- **执行状态：BUILDER 已完成，ARCHITECT 有条件通过复核。** 未提交、未推送、未打标签。端点准入两侧常量仍为 `false`，`runtime/` 无差异。独立结论见 [REVIEW.md](REVIEW.md) 顶部，证据索引见 [tests/verification/desktop/evidence/od-arch-018/README.md](tests/verification/desktop/evidence/od-arch-018/README.md)。
- **授权：** 用户本轮明确「批准执行 OD-ARCH-018」。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`；开工时 81 项未提交/未跟踪改动全部保留，暂存区为空。

### 已完成

1. **成果列表缺陷修复（本轮新发现）。** 原 `task_shell.rs` 用 `rsplit_once('-')` 拆解成果文件名，把 `offline-weekly-report-<时间>.md` 的场景名解析成 `report`，因此“已保存的成果”列表恒为空。新增 `split_offline_artifact_name()` 按已登记场景前缀解析，列表与读取共用，修复后三份成果均可列出并重开。
2. **成果列表布局修正。** `offline-flows.css` 新增 `.record-list` / `.record-item` 两行网格，长文件名换行、按钮固定右侧，不再相互挤压。
3. **Rust 定向测试补充 4 项。** 覆盖列表/重开往返、过滤无关文件、拒绝路径穿越与超大文件、符号链接不可读不可列。
4. **真实桌面窗口纵向验收。** 在隔离合成根 `/tmp/od018-evidence/run2/` 中启动 Tauri 窗口（Xvfb `:97`），完成三个离线场景的输入、本地处理、人工确认、保存与重新打开，并检查会话页与任务页显示。

### 修改文件

- `desktop/src-tauri/src/task_shell.rs`：新增 `split_offline_artifact_name()`，列表与读取共用；新增 4 项单元测试。
- `desktop/src/offline-flows.css`：新增成果列表网格样式。
- `tests/verification/desktop/evidence/od-arch-018/`（新增）：`README.md` 证据索引、`shots/` 24 张窗口截图。
- `STATUS.md`、`PLANS.md`。`REVIEW.md` 按 AGENTS.md 留给 ARCHITECT，本轮未改。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cargo test --offline task_shell::tests::offline_artifact` | 4 通过 / 0 失败 | Rust 单元测试 |
| `cargo test --offline` | 92 通过（76 lib + 13 host_integration + 3 e2e）/ 0 失败 | Rust 完整离线回归 |
| `npx tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `npm run build` | 退出码 0 | 构建实测 |
| `cargo run --offline --example evaluate_offline_scenarios` | 三场景通过，准入拒绝生效，日期边界正确 | Rust 运行实测 |
| `python3 tests/verification/desktop/verify_session_continuity.py` | 37/37 通过 | 隔离合成根 + 本机回环 ACP |
| `python3 tests/verification/desktop/verify_engine_cancel.py` | 状态/IPC 34/34，静态 8/8 | 编译真实源码后 Node 执行 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0，无差异 | 文件事实 |

### 窗口内可观察结果

- 周报：本地汇总生成，界面标注「模型步骤：受准入阻塞、未验证」；保存为 `offline-weekly-report-20261018-060901.md`（986 字节），重开为只读原文。
- 一般文案：输出含「未使用模板（没有与用途匹配的模板，按用途组织）」与待确认小节；保存为 `offline-material-writing-20261018-061420.md`（1038 字节）。
- 项目跟进：4 项事项，状态 done 1 / in_progress 1 / blocked 1 / not_started 1，逾期 2（判定日 2026-10-18），标注 1 条受阻、2 条逾期需人工确认；保存为 `offline-project-management-20261018-062645.md`（1059 字节），列表共 3 项且重开一致。
- 会话与任务页：准入关闭时显示「端点未确认，当前无法读取会话清单和任务台账」，未显示绝对路径、自由文本 intent 或正文。

以上三份成果均在仓库外的隔离根 `local-data/` 中，仓库内无合成正文残留。

### 未完成与限制

- **真实模型生成与写作质量未验证。** 准入关闭，三场景的模型步骤均未执行；离线流程产出不代表生成能力。
- **取消按钮的真实窗口交互未验证。** 准入关闭时任务无法进入可取消状态；取消语义仅由前端替身测试覆盖。
- **跨平台未验证。** 仅在 Linux + Xvfb 虚拟屏验证。
- **窗口交互由 XTEST 注入坐标**，等效真实输入事件，但未经人工手点确认观感。
- `cargo fmt --check` 未运行（本机无 rustfmt 组件）。

### Plan 与实际代码冲突

未发现需要 ARCHITECT 决策的冲突。成果列表解析缺陷属计划内普通实现缺陷，已自行修复；未扩大功能、未新增依赖、未改协议。

### 建议 ARCHITECT 关注

1. 复核成果列表解析修复是否还需要覆盖新的场景前缀（新增场景时须同步登记列表）。
2. 下一步若要验证模型写作或取消交互，需要单独批准端点准入或提供受控合成端点，不由 BUILDER 自行放开。

## OD-ARCH-017：会话连续性纵向能力（BUILDER 执行记录，2026-10-18）

- **执行状态：BUILDER 已完成；ARCHITECT 于 2026-10-18 有条件通过代码与合成适配层验证，真实桌面窗口交互未验收。** 未提交、未推送、未打标签。端点准入两侧常量仍为 `false`，`runtime/` 无差异。独立审查见 [REVIEW.md](REVIEW.md) 顶部。
- **授权：** 用户本轮明确「OD-ARCH-017 授权执行」。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`；开工时约 77 项未提交/未跟踪改动全部保留，暂存区为空。

### 已完成

1. **取消重复请求修复（并入本阶段）。** `workbench.tsx` 新增 `engineCancelRequested` 并传入按钮状态模型：普通 `cancelling` 期间按钮禁用；仅 `cancelling + timeout_unknown` 允许“重试取消”一次；重试前清空旧结果，重试在途仍禁用；已发出请求但宿主状态滞后时不再重复发送。`cancelEngineExplanation()` 点击入口执行同样的拒绝逻辑。
2. **会话清单与按工作区归组。** `acp_adapter.py` 新增 `SessionSummary`、`SessionListing`、`TaskRecordSummary`、`TaskLedgerListing`；`list_sessions()` 只调用公开 `session/list` 取元数据，按规范化工作区归组，不含正文；`list_task_records()` 只读取 OmniDesk 自有台账元数据并丢弃 `content`。删除了无效的 `page_size` 参数，单页大小由 DSH 配置决定。
3. **能力语义如实标注。** 原易误导的 `resolvable` 改为 `metadata_valid`，并新增 `resume_status="unverified"`；列表可见不等于可恢复。
4. **IPC 与宿主接线。** `serve.py` 新增 `omni.list_sessions`、`omni.list_task_records`；`engine_bridge.rs` 新增 `list_sessions`、`recover_session`、`list_task_records`、`recover_task`；`lib.rs` 四个 Tauri 命令全部经 `describe_after_admission`，准入关闭时拒绝且不启动子进程。
5. **桌面界面。** 新增 `desktop/src/Sessions.tsx` 及 `shell/Dock.tsx` 入口，接入 `App.tsx`。只显示会话编号、工作区、结构状态和任务状态，不显示正文；明确“列表可见不等于可恢复”；准入关闭时显示真实原因而非伪造空清单；恢复失败显示原因；任务恢复只显示可信终态，未知状态明确“不自动重放”；注明旧会话导入不受支持。
6. **旧会话导入结论。** 新增 `docs/OmniDesk-Session-Import-Compatibility.md`：锁定版 DSH `0.2.1-alpha.2` 公开 ACP 仅有 `session/new/list/resume/close/setConfigOption/prompt/cancel`，无 `session/import` 或等价入口；未复制或改写原生格式，未自造迁移格式；导入按计划记录为 **unsupported**。同数据根内的发现与重开受支持，已有合成实测。

### 修改文件

- 应用代码：`desktop/src/workbench.tsx`、`desktop/src/types.ts`、`desktop/src/api.ts`、`desktop/src/App.tsx`、`desktop/src/Sessions.tsx`（新增）、`desktop/src/shell/Dock.tsx`、`desktop/src-tauri/src/engine_bridge.rs`、`desktop/src-tauri/src/lib.rs`、`engine/omnidesk_engine/acp_adapter.py`、`engine/omnidesk_engine/serve.py`、`engine/omnidesk_engine/__init__.py`。
- 测试与证据：`tests/verification/desktop/verify_engine_cancel.py`（修正预期并新增分支）、`tests/verification/desktop/verify_session_continuity.py`（新增）、`tests/verification/desktop/evidence/od-arch-017/`（新增：`README.md`、`engine-cancel-model.json`、`session-continuity.json`、`rust-cargo-test.log`、`engine-adapter-rerun.log`、`frontend-build.log`）。
- 文档：`docs/OmniDesk-Session-Import-Compatibility.md`（新增）、`STATUS.md`、`PLANS.md`；`REVIEW.md` 按 `AGENTS.md` 留给 ARCHITECT。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cd desktop && npx tsc --noEmit` | 退出码 0 | 前端类型检查 |
| `cd desktop && npm run build` | 退出码 0，构建成功 | 构建实测 |
| `python3 tests/verification/desktop/verify_engine_cancel.py` | 23/23 状态分支、8/8 静态检查通过，`RESULT: pass` | 编译真实源码后 Node 执行 + 静态检查 |
| `python3 tests/verification/desktop/verify_session_continuity.py` | 32/32 检查通过，`RESULT: pass` | 一次性隔离根 + 本机回环合成 ACP 实测 |
| `cd desktop/src-tauri && cargo test --offline` | 72 + 13 + 3 = 88 项通过，0 失败 | Rust 离线回归实测 |
| `python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py` | E01–E20 全部 `pass` | 适配器场景回归实测 |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0，无差异 | 文件事实 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `rg` 核对两侧 `ENDPOINT_ISOLATION_CONFIRMED` | 均为 `false` | 静态代码检查 |
| 凭证特征扫描（排除 `runtime/`、`node_modules`、`.git`） | 无匹配 | 静态检查 |

### 合成纵向验证覆盖

- 创建并完成合成任务；关闭引擎、重启适配器后由 `session/list` 找到会话、按工作区归组且只含元数据；`session/resume` 成功重开；未知会话返回 `unknown_session`。
- 重启后已完成任务恢复为 `succeeded`；中断任务恢复为 `unknown/interrupted` 且 `replayed=false`；台账只含元数据，正文哨兵未泄漏；损坏台账计入 `skipped_invalid`，不猜造状态。
- 列表读取前后 `DSH_HOME` 文件哈希完全不变；重开不新建会话、不重放提示词。
- **如实记录：** `session/resume` 会使 DSH 在自身会话存储追加恢复记录，仅 `sessions/.../session.v4.jsonl.zstd` 与 `storages/session_projcache/...` 两个文件变化。这是引擎原生行为，不是 OmniDesk 改写用户数据；本阶段不声称“恢复过程零写入”。

### 未完成与限制

- **未做真实窗口点击。** 未启动 Tauri 窗口、未做真实会话 UI 操作。
- **未做真实模型与真实会话验证。** 端点准入保持关闭，界面“会话与任务”页当前不能列出真实会话；未调用真实模型。
- **跨版本旧会话导入未实现、未验证**，原因是缺少公开导入接口。
- `cargo fmt --check` 未运行：本机未安装 `rustfmt` 组件。

### Plan 与实际代码冲突

未发现影响实施的冲突。计划中“在隔离数据根中显式导入旧 DSH 会话”因底层无公开机制而按计划停止条件记录为 unsupported，其余部分独立完成。

### 建议 ARCHITECT 关注

1. 核对 `session/resume` 的原生写入行为是否需要在产品说明中对用户披露。
2. 端点准入开放前，确认会话元数据（工作区路径、标题）在真实环境中的隐私处理策略。
3. 复核旧会话导入 unsupported 结论是否可接受，或另立跨版本迁移研究计划。

## OD-ARCH-016：修复工作台引擎任务取消入口（BUILDER 执行记录，2026-10-18）

- **状态：已执行，等待 ARCHITECT 独立复核。** 未提交、未推送、未打标签。端点准入两侧常量仍为 `false`，`runtime/` 无差异。
- **授权：** 用户本轮明确「批准 OD-ARCH-016 执行」。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`；开工时 73 项未提交/未跟踪改动全部保留，暂存区为空；本轮改动后共 75 项。

### 已完成

1. **取消按钮恢复可用。** 取消按钮只受 `engineCancelButtonState` 控制，不再被 `engineBusy` 阻塞；`queued/starting/running/cancelling` 状态可点击，终态与无任务时禁用。
2. **请求与生成状态分离。** 新增 `engineCancelBusy` 与同步锁 `cancelInFlightRef`；取消请求在途时按钮显示“正在请求取消…”，快速连点只触发一次 IPC。
3. **结果口径不变。** 只有宿主返回 `confirmed_cancelled` 或回读终态为 `cancelled` 才显示“取消已确认”；请求已发但最终 `succeeded/failed`、`timeout_unknown`、`acknowledged` 均显示“取消未确认”，并附宿主最终态。取消路径不调用 `setExplanation` / `setSource`，已有解释正文不受影响。
4. **超时后可重试。** 上次取消结果为 `timeout_unknown` 且任务仍未终态时，按钮显示“重试取消”。

### 修改文件

- `desktop/src/workbench.tsx`：新增取消状态纯函数、`useRef` 同步防重入、取消结果提示；按钮改绑取消状态模型。
- `tests/verification/desktop/verify_engine_cancel.py`（新增）：抽取 `engine-cancel-model` 区域编译执行 20 个状态分支，并做 5 项静态边界检查。
- `tests/verification/desktop/evidence/od-arch-016/`（新增）：`README.md`、`engine-cancel-model.json`、`frontend-build.log`。
- `STATUS.md`、`PLANS.md`：状态记录；`REVIEW.md` 按 `AGENTS.md` 留给 ARCHITECT。

### 实际运行的检查

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cd desktop && npx tsc --noEmit` | 退出码 0，无输出 | 前端类型检查 |
| `cd desktop && npm run build` | 退出码 0，耗时 631ms；日志见证据目录 | 构建实测 |
| `python3 tests/verification/desktop/verify_engine_cancel.py --json ...` | 20/20 状态分支通过，5/5 静态检查通过，`RESULT: pass` | 编译真实源码后 Node 执行 + 静态检查 |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0 | 文件事实 |
| `git diff --check HEAD` | 退出码 0 | 文件事实 |
| `rg` 核对两侧 `ENDPOINT_ISOLATION_CONFIRMED` | 均为 `false` | 静态代码检查 |

### 未完成与限制

- **未做真实窗口点击与引擎取消。** 本轮没有启动引擎、宿主窗口或模型；测试只证明按钮判定、文案和源码接线正确，不证明真实 IPC 往返。该场景需在隔离环境用合成任务复核。
- 计划未要求改 IPC/共享状态机/Rust 层，本轮也未改；因此未重跑 Rust 回归，不引用历史结果作为本轮证据。
- 模型端点仍关闭，无法在产品端点复验模型任务取消。

### Plan 与实际代码冲突

未发现。计划明确的本文件范围足够完成修复，无需扩展。

### 建议 ARCHITECT 关注

1. 在隔离环境用合成任务验证：真实点击后宿主取消往返、终态回读、快速连点只产生一次请求。
2. 关注 `cancelling` 状态下的“重试取消”是否会造成过度重试；当前仅在 `timeout_unknown` 后显示。
3. 取消按钮在任务已成功回填后立即禁用，符合预期，但需窗口确认无视觉跳变。

## OD-ARCH-015：核对 UI 整合边界并收口共享状态（BUILDER 执行记录，2026-10-18）

- **状态：BUILDER 只读核对及 ARCHITECT 独立复核均完成。** 核对记录按范围通过；取消入口缺陷已单列为待批准计划 OD-ARCH-016。未修改应用代码、测试、UI 工作区、`runtime/`、用户数据或证据原件；未提交、未推送、未打标签。端点准入保持关闭。独立结论见 [REVIEW.md](REVIEW.md)。
- **授权：** 用户本轮明确「授权开始执行OD-ARCH-015」。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`；开工时 73 项未提交/未跟踪改动（46 项已跟踪修改、27 项未跟踪）全部保留，暂存区为空。

### 核对结论

| 核对项 | 结论 | 证据等级 |
| --- | --- | --- |
| 前端准入 | `desktop/src/engineAdmission.ts:16` 固定 `false`；`api.ts` 的 `submitTask` 在 `invoke` 前统一拒绝；工作台另有按钮禁用与提交前二次判定 | 静态代码检查 |
| 宿主准入 | `desktop/src-tauri/src/admission.rs:15` 固定 `false`；`engine_describe`、`engine_submit_task`、`task_prepare_workspace_dir` 均先准入后触达宿主；supervisor 启动前再次拒绝 | 静态代码检查 |
| 提交入口数量 | 前端 `host.submitTask` 唯一调用点为 `workbench.tsx:386`；首页与只读引擎诊断没有第二套提交入口 | 静态代码检查 |
| 人工确认 | 界面 `canSave = analysis && confirmed && explanation 非空`；宿主 `save_artifact` 再次强制 `user_confirmed`、来源白名单、指纹复算 | 静态代码检查；历史运行证据 |
| 保存边界 | 默认落点为仓库外本地数据目录，`create_new` 拒绝覆盖，写入前复算指纹；OD-ARCH-014 窗口实测写入 `/tmp/od014-dev/.../local-data/20261018-035213-320dae745102.md` | 历史运行证据（014 证据目录） |
| 失败与取消语义 | 轮询终态后仅在 `succeeded` 且内容非空时回填；失败、取消、中断不写成功，不改写已有解释；取消阶段以 `confirmed_cancelled` 为唯一成功口径 | 静态代码检查；历史脚本证据 |

### 发现的问题

1. **取消按钮实际上无法使用（真实缺陷，需单独修复计划）。** `runEngineExplanation` 提交后保持 `engineBusy = true`，直到轮询到终态或 180 秒超时才复位；而取消按钮的 `disabled` 条件是 `engineBusy || !engineStatus || 终态`。执行期间被 `engineBusy` 禁用；执行结束后状态已进入终态，又被终态条件禁用。因此界面上的「取消引擎任务」在任何时刻都点不到。底层的取消能力本身存在（OD-BUILD-005 历史脚本记录 `cancel=cancelled`），缺的是界面可用的触发点。该问题不属于端点准入放宽，也不属于保存路径丢失；由于执行前 HEAD 尚无引擎解释支路，不能归因于 OD-ARCH-014 的回退。本计划未改代码。
2. **`workbench.tsx` 执行前边界无法逐行恢复。** 当前工作区文件与 UI worktree 副本一致（`diff` 无输出），但执行前未提交版本没有快照、stash 或备份可查。OD-BUILD-005 的截图与报告能证明引擎支路在该轮之前已存在，但不能替代逐行源码比对。OD-ARCH-014 的“整体覆盖”偏差仍按原样保留，不据此宣称严格遵守文件范围。
3. **计划与角色分工冲突。** OD-ARCH-015 允许更新 `REVIEW.md`，但根目录 `AGENTS.md` 规定 `REVIEW.md` 归 ARCHITECT。BUILDER 未改 `REVIEW.md`；如需独立复核记录，应由 ARCHITECT 写入。

### 本轮实际执行的检查

| 命令/动作 | 结果 |
| --- | --- |
| `git status --short`、`git diff --name-status HEAD` | HEAD `5ffc710`；73 项工作区改动保留；暂存区为空 |
| `git diff HEAD -- desktop/src/workbench.tsx`、`git show HEAD:desktop/src/workbench.tsx` | 确认改动仅涉及引擎支路、状态展示与 `.wb-v2` 样式作用域；原统计、手动解释、确认保存代码仍在 |
| `diff` 主工作区与 UI worktree 的 `workbench.tsx` | 完全一致 |
| `rg` 核对 `submitTask`、`engine_submit_task`、`task_prepare_workspace_dir` 调用点 | 提交入口唯一；宿主入口均有准入包装 |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0，Runtime 无差异 |
| `git diff --check HEAD` | 退出码 0，无空白错误 |

本轮**未重新运行**前端构建、Rust 测试或窗口交互；这些结果不引用为本次运行证据，只保留 014 的历史记录。未启动引擎、未调用模型、未读取凭证或真实材料。

### 未验证 / 限制

- 取消按钮的缺陷仅由源码检查确认，未在真实窗口中复现，也未验证修复后的行为。
- 引擎输出与成功终态的代码级一一对应仍未实现，来源只能记为 `model_unverified`。
- 模型端点、真实工作材料、跨平台与长期使用仍未验证。
- `REVIEW.md` 未更新，等待 ARCHITECT 独立复核。

### 建议 ARCHITECT 下一步

1. 复核本轮结论并决定是否在 `REVIEW.md` 记录 OD-ARCH-015。
2. 单列一项小范围修复计划，解除取消按钮对 `engineBusy` 的依赖，并补运行级测试：任务运行中可请求取消、取消确认与正常完成竞争、超时语义不变。
3. 修复获批前不开放模型端点，不扩大桌面功能范围。

## OD-ARCH-014：以主工作区为基线选择性整合 UI 外壳（BUILDER 执行记录，2026-10-18）

- **状态：已执行；ARCHITECT 复核为有条件通过（2026-10-18）。** 复核结论见 `REVIEW.md` 顶部 OD-ARCH-014。未提交、未推送、未打标签；未修改 `runtime/`；`/mnt/data/Code/omniharness-ui` 只读，未写入。端点准入保持关闭（`ENDPOINT_ISOLATION_CONFIRMED = false`），未改模型路由与凭证。
- **授权：** 用户本轮明确「授权执行OD-ARCH-014」。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`。开工时约 59–68 项既有未提交/未跟踪改动全部保留，未重置、未覆盖、未清理。
- **证据索引：** [tests/verification/desktop/evidence/od-arch-014/README.md](tests/verification/desktop/evidence/od-arch-014/README.md)

### 计划偏差（需 ARCHITECT 判断是否可接受）

计划第 1 步的停止条件命中：UI 工作区的 `docs/omnidesk-ui-v2-handoff.md` 仍称「已包含主工作区成果」「两侧文件基本不重叠」，与事实不符，且该文档无人更正。

实际处理：依据用户明确授权执行 OD-ARCH-014，且 UI 工作区只读，**以主工作区为唯一基线逐个选取文件继续**，未修改该交接文档。这不是无瑕疵执行；若 ARCHITECT 认为必须先更正交接文档，本轮成果应视为未通过。

### 已完成内容

1. **视觉层与组件源。** 选择性接入 `theme.css`、`workbench-ui.css`、`engineAdmission.ts`、`components/Icon.tsx`、`shell/TopNav.tsx`、`shell/ThreeWeekCalendar.tsx`、`shell/EngineTechSlice.tsx`、`public/mascot.png` 与自托管字体依赖、`@tailwindcss/vite`。
2. **外壳与导航（主工作区新建）。** `shell/Dock.tsx` 只列四个已实现入口（首页 / 任务工作台 / 离线流程 / 引擎诊断）；`shell/HomeDashboard.tsx` 提供品牌、三周日历、边界说明与三张入口卡，**不设第二套引擎对话**，任务提交仍只有工作台一处。
3. **旧样式降级而非删除。** `styles.css` 全部原有规则原样包进 `@layer legacy { … }`，`.scope-notice`、`.scope-path` 等仍保留，未删任何规则。
4. **准入只收紧不放宽。** `App.tsx` 删除旧技术切片中的自由文本提交面板，引擎诊断改为只读覆盖层；`api.ts` 在 `submitTask` 开头统一做前端准入拒绝，宿主拒绝仍是最终边界。
5. **离线流程修复（本轮发现并修复的可见缺陷）。** 旧深色类名在浅色外壳下出现深底深字、按钮被压成小色块。新增 `desktop/src/offline-flows.css`，只作用于 `.ofl-v2` 子树恢复浅色可读样式；并修正选中标签悬停时文字变暗的对比度问题。修订过程仅改样式，未改文案与功能。
6. **引擎诊断对比度。** 深色事件区空态提示由 50% 提到 80% 不透明度，避免几乎不可见。

### 修改文件

- 新增：`desktop/src/shell/Dock.tsx`、`desktop/src/shell/HomeDashboard.tsx`、`desktop/src/offline-flows.css`、`desktop/src/theme.css`、`desktop/src/workbench-ui.css`、`desktop/src/engineAdmission.ts`、`desktop/src/components/Icon.tsx`、`desktop/src/shell/TopNav.tsx`、`desktop/src/shell/ThreeWeekCalendar.tsx`、`desktop/src/shell/EngineTechSlice.tsx`、`desktop/public/mascot.png`、`scripts/dev-ui.sh`、`scripts/lib/port-owner.sh`
- 修改：`desktop/src/App.tsx`、`desktop/src/api.ts`、`desktop/src/main.tsx`、`desktop/src/styles.css`、`desktop/src/workbench.tsx`、`desktop/src/OfflineFlows.tsx`、`desktop/package.json`、`desktop/package-lock.json`、`desktop/vite.config.ts`
- 证据：`tests/verification/desktop/evidence/od-arch-014/`（README、8 张窗口截图、4 份日志）

### 实际运行的测试

| 命令 | 结果 |
| --- | --- |
| `cd desktop && npx tsc --noEmit` | 退出码 0，无输出 |
| `cd desktop && npm run build` | 退出码 0；CSS 69.71 kB、JS 278.63 kB |
| `cargo test --offline`（先 `scripts/setup-desktop-toolchain.sh --print-env`） | lib 72 / host_integration 13 / e2e 3 全通过，0 失败 |
| `cargo run --offline --example evaluate_offline_scenarios` | 退出码 0；三场景通过、准入拒绝生效、日期边界正确 |
| `git diff --check HEAD` | 退出码 0 |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0（Runtime 无差异） |
| 真实窗口（Xvfb :96 + `scripts/dev-ui.sh`，合成端点、隔离数据根） | 首页、工作台、统计、准入拒绝、人工确认保存、离线周报、离线文案、只读引擎诊断 8 项均可交互；截图见证据目录 |

窗口实测关键数字：合成样例 5 行、合计 204、平均 40.8、最高 51、指纹 `320dae745102…`；保存文件 `local-data/20261018-035213-320dae745102.md`，1742 字节；进程经 `scripts/dev-ui.sh --stop` 回收（app 1、mock 1、vite 3 含子进程），未误杀其它实例；Xvfb :96 已结束。

### 未验证 / 限制

- **未做任何模型输出验证。** 端点准入关闭，模型步骤全部 blocked；截图只是拒绝提示，不是模型结果。
- **仅本机 Linux + Xvfb。** 未验证 Windows/macOS 与真实显示器合成器差异；未验证窄窗口与长清单滚动性能。
- **执行时未改 `PLANS.md`。** 当时文件仍显示「待用户批准」；ARCHITECT 已在独立复核后将其状态校准为「已执行、有条件通过」，具体结论见 `REVIEW.md`，当前计划与后续提议见 `PLANS.md`。
- **未做整体分支合并。** 仅逐个取文件；UI 工作区删除的宿主准入与缺失的离线能力未被带回。

### ARCHITECT 复核交接

- 独立复跑：前端构建通过；Rust 离线测试 88/88 通过；离线场景评估、`git diff --check HEAD` 通过；`runtime/` 无差异。
- 已抽查 8 张 Xvfb 截图，未亲自重跑窗口交互。当前 `workbench.tsx` 与 UI worktree 文件相同；历史报告支持引擎支路早于本轮存在，但无执行前快照可证明整文件覆盖未改变原工作台行为。该计划偏差已记录，不能据此声称严格遵守文件范围。
- 下一步：OD-ARCH-015 只读核对已执行，待 ARCHITECT 独立复核，详见 `PLANS.md` 与 `STATUS.md` 顶部。会话持久化、旧会话导入未开始。

### Git 状态

全部改动保留在工作区，未暂存、未提交、未推送、未打标签。`runtime/` 无差异。

## OD-ARCH-013：双工作区保护盘点与新 UI 独立审查（ARCHITECT 复核，2026-10-18）

- **状态：只读审计已执行，待 ARCHITECT 独立审查。** 未修改任一工作区源码或测试，未运行构建/测试/窗口，未启动 DSH 或模型，未读取凭证、旧会话或真实成果正文，未合并/复制/暂存/提交/推送/打标签，`runtime/` 未触碰。
- **授权：** 用户本轮明确「批准可以开始OD-ARCH-013」。范围仅为只读盘点、差异分析与整合顺序建议；本轮仅更新 `PLANS.md`、`STATUS.md`、`REVIEW.md`。

### ARCHITECT 独立复核

- **结论：** OD-ARCH-013 按范围通过。再次核对两工作区共同 HEAD `5ffc710`，主工作区为 `main`、UI 工作区为 `feat/desktop-ui`；当前分别有 59 与 71 项未提交/未跟踪状态差异，均未暂存。主工作区 `runtime/` 无差异。
- **关键事实：** UI 工作区删除了主工作区 `EngineSupervisor::start()` 中的准入拒绝，并缺少主工作区的 `OfflineFlows.tsx` 与离线流程命令；主工作区保留准入、离线流程、默认成果路径和耐久保存实现。因此不接受整分支合并。
- **限制：** 未运行构建、测试或窗口；本次为静态复核，不引用之前报告代替当前运行证据。未改两工作区源码，未合并、复制、暂存、提交或推送。
- **后续：** 提出 OD-ARCH-014：以主工作区为唯一代码基线选择性接入 UI 外壳，待用户批准，详见 `PLANS.md`。

### 基线（执行前记录）

| 工作区 | 分支 | HEAD | `git status --short` | 已暂存 | 已跟踪未暂存 | 未跟踪 |
| --- | --- | --- | --- | --- | --- | --- |
| `/mnt/data/Code/MyWorkStation` | `main` | `5ffc710` | 59 项 | 0 | 42 | 43 |
| `/mnt/data/Code/omniharness-ui` | `feat/desktop-ui` | `5ffc710` | 71 项 | 0 | 47 | 61 |

两边 HEAD 相同，属同一仓库的两个 worktree；改动各不相同，本轮全部保留。

### 已完成检查（文件事实 + 静态代码检查）

1. **逐文件比对两边的桌面端源码。** 相同：`desktop/src-tauri/src/engine_ipc.rs`、`engine/omnidesk_engine/acp_adapter.py`。不同：`App.tsx`、`api.ts`、`main.tsx`、`styles.css`、`types.ts`、`workbench.tsx`、`engine_bridge.rs`、`engine_supervisor.rs`、`lib.rs`、`task_shell.rs`、`host_integration.rs`。UI 分支独有：`theme.css`、`workbench-ui.css`、`engineAdmission.ts`、`components/`、`shell/`。主工作区独有：`OfflineFlows.tsx`、`samples/workflows/`。
2. **核实交接文档主张。** `docs/omnidesk-ui-v2-handoff.md` 第 11、15 行称本分支「已包含」主工作区成果且「两侧文件基本不重叠」。文件事实不支持该说法：UI 分支缺少主工作区后续多项后端与离线能力（见下条）。
3. **核实后端新旧与差异清单。** UI 分支缺少：`EngineSupervisor::start()` 的宿主准入拒绝（主 `engine_supervisor.rs:435`）、事件泵锁饥饿回归测试、`write_new_file_durable` 及其失败清理与 OD-ARCH-012 测试、默认成果根 `~/OmniDesk/workspace/artifacts`（UI 分支仍为 `~/YvanAI/projects/Omnidesk/analysis`）、离线流程命令 `workflow_samples` / `task_weekly_summary` / `task_writing_plan` / `task_project_summary` / `task_save_offline`。
4. **核实前端准入。** UI 分支有 `engineAdmission.ts`（常量固定 `false`），`api.ts` 在 `invoke("engine_submit_task")` 之前统一判定，`HomeChat.tsx`、`workbench.tsx` 二次拦截并禁用按钮，技术切片为只读诊断；有 `verify_engine_admission.py`（纯函数编译执行 + 静态调用点扫描）与磁盘证据 `evidence/od-arch-005`、`od-arch-006`。主工作区没有前端准入模块，但宿主 `admission.rs` 固定拒绝提交、目录准备、引擎描述与直接启动，并由 `host_submit_admission` / `host_describe_admission` / `host_workspace_prepare_admission` 定向测试覆盖；主工作区 `App.tsx:146` 的旧技术切片仍保留自由文本提交入口。
5. **核实启动器隔离。** `scripts/dev-ui.sh`：固定拒绝 `--real`；数据根固定在 `/tmp/omnidesk-dev/<工作区哈希>` 并将 `OMNIDESK_APP_DATA_ROOT`、`OMNIDESK_LOCAL_DATA_ROOT`、`DSH_HOME`、`DSH_AGENTS_HOME`、`OMNIDESK_DSH_HOME`、`OMNIDESK_AGENTS_HOME` 全部指向该目录；只读引用 `runtime/` 内的 DSH CLI，不写入 `runtime/`；5273 端口被任何服务占用即失败，不发送信号、不复用；563 行合成端点强制校验 `127.0.0.1`/`localhost`，覆盖层由本次运行一次性生成，`OMNI_FAKE_KEY` 为明示占位串。`_mock_model_server.py` 监听 `127.0.0.1` 随机端口，只回固定合成文本，仅记录是否命中探针而不保存请求正文。
6. **核实 UI 分支证据与限制。** 证据目录 `od-arch-005`（准入闸门 11/11、启动器身份 6/6、截图两张）、`od-arch-006`（端口冲突、整树回收、窗口截图、宿主拒绝日志）、`od-ui-002`（五阶段工作台与窄窗口截图）均存在且 README 与所述实现对应。仍未验证：真实模型输出、跨平台、低于 960px 的布局、技术切片窗口内点击。

### 未运行与限制

- **未运行任何构建或测试。** 两边均有大量未提交改动，构建会写入各自 worktree 的 `target/`、`node_modules/` 与共享缓存，超出本计划只读边界。因此本轮结论全部为**文件事实与静态代码检查**，不含脚本运行或窗口运行证据。
- **未启动 UI 分支窗口。** 交接文档中的窗口与交互结论沿用其自身证据，本轮未独立复现。
- **未评估合并可行性。** 只读比较不能证明两边整合后能编译或行为一致，需另立计划做实际整合与回归。
- **准入状态未变。** 两边端点准入常量均为 `false`，本轮不放宽、不改变模型路由或凭证。

### Git 状态

本轮仅追加本段与 `REVIEW.md`、`PLANS.md` 的状态记录；两边源码、测试、证据文件与既有未提交改动全部保留，未提交、未推送、未打标签。

## OD-ARCH-012：补齐成果写入失败清理验证（BUILDER 执行记录，2026-10-18）

- **状态：已执行，待 ARCHITECT 独立审查。** 未调用模型、未读取凭证或真实业务材料、未修改 `runtime/`、未提交/推送/打标签。端点准入保持关闭（`ENDPOINT_ISOLATION_CONFIRMED = false`）。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`。执行前后既有未提交/未跟踪改动全部保留；`git status --short` 由 58 行变为 59 行，差额为本轮新增的 OD-ARCH-012 证据目录。
- **授权：** 用户本轮明确「授权执行OD-ARCH-012」。范围仅为补足 OD-ARCH-011 缺失的写入失败与刷盘失败清理证据及计划规定回归。

### 已完成内容

1. **测试专用故障注入。** `task_shell.rs` 新增 crate 内私有 `NewFileOps`（`write` / `sync` / `remove`）。生产入口 `write_new_file_durable(target, content)` 签名不变，固定使用 `NewFileOps::REAL`；故障版本只能由测试构造，不能从 UI、环境变量、普通配置或宿主请求触发。
2. **新增 4 项测试：** 写入中途失败只删本次半成品且既有文件不变；刷盘失败删除已写入文件；清理本身失败时如实报告「清理半成品失败」而不伪装成功；生产入口无法注入故障。
3. **测试有效性验证：** 两次受控变异分别产生 2 项与 1 项预期失败，随后立即还原源码（SHA-256 回到 `54c2ed22…b455`）并重跑定向测试为 45/0。变异未留在源码。
4. **OD-ARCH-011 旧记录勘误：** 其 STATUS 记录的「统一写入与失败清理」当时只有静态实现，缺少本轮补上的故障注入运行证据；该项现由 OD-ARCH-012 证据补齐。

### 实际运行的测试

| 命令 | 结果 |
| --- | --- |
| `cargo test --offline task_shell`（`desktop/src-tauri`，配合 `scripts/setup-desktop-toolchain.sh --print-env`） | **45 passed / 0 failed** |
| `cargo test --offline` | lib **72 passed / 0 failed**；`host_integration` **13 passed / 0 failed**；`task_shell_e2e` **3 passed / 0 failed**；doc tests 0 |
| `npm run build`（`desktop`） | 通过（tsc + vite） |
| `cargo run --offline --example evaluate_offline_scenarios` | 通过 |
| `git diff --check HEAD` | 无输出，退出码 0 |
| `git diff -- runtime/` | 0 行；`runtime/` 无未跟踪改动 |

### 证据路径

- [tests/verification/desktop/evidence/od-arch-012/README.md](tests/verification/desktop/evidence/od-arch-012/README.md)：实现边界、三类失败场景、两次变异检查、完整回归与限制。
- 同目录日志：`od012_targeted_final.log`、`od012_full_rust.log`、`od012_frontend.log`、`od012_example.log`、`od012_mutation.log`、`od012_mutation2.log`。

### 边界核对

- `~/.dsh` mtime 仍为 `2026-10-15 20:17:43`，`~/.agents` 仍为 `2026-09-23 15:12:11`；未读取其内容。
- 未启动用户实例，未读取旧会话或旧成果正文，未接触凭证、端点或模型路由。
- 未提交、未推送、未打标签；未修改 `REVIEW.md`。

### 未验证与限制

- **模型写作仍未验证：** 准入关闭，本计划不授权模型调用；不得据此宣称真实生成质量或业务收益。
- 用途准入保持原状：周报、一般写作、项目跟进仍为 `reserved`，不构成正式能力授权。
- 文件清理只覆盖单文件失败路径；清理失败会残留半成品，代码如实报告，不承诺超出文件系统保证的原子性。
- 仅验证 Linux 本机文件系统上的注入故障，未验证磁盘满、权限突变或断电场景；未做跨平台验证。
- `rustfmt` 组件未安装，未运行格式化检查；`cargo test` 编译通过。

## OD-ARCH-011：日期校验、成果落点与窗口内保存验证（BUILDER 执行记录，2026-10-18）

- **状态：已执行，待 ARCHITECT 独立审查。** 未调用任何模型、未读取凭证或真实业务材料、未修改 `runtime/`、未提交/推送/打标签。端点准入保持关闭（`ENDPOINT_ISOLATION_CONFIRMED = false`）。
- **工作区：** `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`。执行前的未提交/未跟踪改动全部保留；`git status --short` 由 56 行变为 58 行，差额为本轮新增的 OD-ARCH-011 证据目录。

### 已完成内容

1. **日历日期校验。** `parse_date` 改为按实际月份天数与公历闰年规则校验（新增 `days_in_month`）。周记录日期、项目期限、逾期判定日共用。拒绝 `2026-02-30`、`2026-04-31`、`2026-00-10`、`2026-01-32`、`1900-02-29`、`2100-02-29`；接受 `2024-02-29`、`2000-02-29` 与普通月末。
2. **新成果默认位置。** 默认根由 `~/YvanAI/projects/Omnidesk/analysis` 改为 `~/OmniDesk/workspace/artifacts`；仅影响将来新文件。`OMNIDESK_LOCAL_DATA_ROOT` 仍可显式覆盖。旧 YvanAI 文件不读取正文、不迁移、不改名、不覆盖。
3. **统一写入与失败清理。** 新增 `write_new_file_durable()`，分析成果与离线成果共用：`create_new(true)` 拒绝覆盖同名文件；写入或刷盘失败时删除本次新建的半成品。此前离线路径只有 `write_all`/`sync_all` 报错，不会清理半成品，本轮修掉。
4. **界面提示。** 离线流程页显示：当前模型不可用；输入框不是授权入口；红线扫描是启发式、不能可靠识别全部身份与工作细节、放行不等于已脱敏或已获准；新成果只写仓库外 OmniDesk 数据目录且不覆盖同名文件；旧 YvanAI 文件不读取、不迁移。页面顶部显示从宿主解析的实际保存目录；日期提示补充「须真实存在」。

### 实际运行的测试

| 命令 | 结果 |
| --- | --- |
| `cargo test --offline`（`desktop/src-tauri`，配合 `scripts/setup-desktop-toolchain.sh --print-env`） | lib **68 passed / 0 failed**；`host_integration` **13 passed / 0 failed**；`task_shell_e2e` **3 passed / 0 failed**；doc tests 0 |
| `cargo test --offline task_shell` | **41 passed / 0 failed**（含本轮新增 6 项日期与保存测试） |
| `npm run build`（`desktop`） | 通过（tsc + vite） |
| `cargo run --offline --example evaluate_offline_scenarios` | 通过，输出见证据 `offline-scenarios.txt` |

### 真实窗口验证（一次性 Xvfb :96，真实 Tauri 窗口）

- 离线流程页：提示、实际保存目录、日期提示均正确渲染。
- 周报合成样例：载入 → 本地处理 → 4 条（done 2 / pending 1 / blocked 1）→ 勾选人工确认 → 保存成功，写入 `/tmp/od011-tYouyK/local-data/offline-weekly-report-20261018-023000.md`（986 字节）。
- 重复保存：跨秒再点保存，生成新时间戳文件（共 5 个），既有文件字节数与内容未变；同名即拒绝由单元测试另行证明。
- 非法日期：窗口内输入 `2026-02-30 | x | done` 并执行，被拒绝并显示「第 1 行：日期必须是存在的 YYYY-MM-DD」。
- 窗口内保存路径是 `OMNIDESK_LOCAL_DATA_ROOT` 指向的一次性 `/tmp` 覆盖，不代表默认路径被改写；默认根由单元测试与证据生成器分别证明为 `~/OmniDesk/workspace/artifacts`。

### 证据路径

- [tests/verification/desktop/evidence/od-arch-011/README.md](tests/verification/desktop/evidence/od-arch-011/README.md)：场景、证据等级、窗口环境、隔离边界、限制与复现说明。
- 同目录：`offline-scenarios.txt`、`saved-offline-weekly-report.md`、`window-01-scope-notice.png` 至 `window-05-invalid-date-rejected.png`。

### 边界核对

- `git diff -- runtime/` 输出 0 行。
- `~/.dsh` mtime 仍为 `2026-10-15 20:17:43`，`~/.agents` 仍为 `2026-09-23 15:12:11`；未读取其内容。
- 窗口运行用一次性 `HOME` 与独立 DSH/应用数据根；引擎未启动，隔离 `dsh-home`/`agents-home` 未被写入。
- `git diff --check HEAD` 无输出。
- 未提交、未推送、未打标签；未修改 `REVIEW.md` 验收结论。

### 未验证与限制

- **模型撰写未验证：** 准入关闭，未调用模型；不得据此声称真实生成、模型质量或业务收益。
- **用途未获准：** 周报、一般写作、项目跟进在 `task_types()` 中仍为 `reserved`，不构成正式能力或用途授权。
- 仅验证 Linux + Xvfb；跨平台未验证。
- 窗口操作经 XTEST 注入完成，属真实窗口运行，但不是人工手工点选。
- 旧 YvanAI 保护由临时 HOME 单元测试证明；未读取或改动真实旧目录内容，因此「实际旧目录里没有被动过」仍属未逐项核验。

## OD-ARCH-010：封住准入缺口并验证三个离线合成场景（BUILDER 执行记录，2026-10-18）

- **状态：已执行，待 ARCHITECT 独立审查。** 未调用任何模型、未读取凭证或真实业务材料、未修改 `runtime/`、未提交/推送/打标签。端点准入保持关闭（`ENDPOINT_ISOLATION_CONFIRMED = false`）。
- **工作区：** 唯一目标 `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`。执行前后均保留既有的 51 项未提交/未跟踪改动。另一 worktree `/mnt/data/Code/omniharness-ui` 的 71 项不同改动未复制、未合并。

### 已完成内容

1. **宿主统一拒绝闸门。** 新增 `desktop/src-tauri/src/admission.rs`；引擎描述、任务提交、任务目录准备和引擎启动四条入口都在任何副作用之前拒绝。`engine_supervisor.rs` 在 `start()` 顶部统一拒绝；`engine_bridge.rs` 顺带修正事件泵不再占锁阻塞宿主。坏配置场景同样先拒绝：不建目录、不起进程、不产生任务。
2. **三个离线场景。** `task_shell.rs` 新增周记录汇总周报、一般文案结构建议、轻量项目跟进；新增命令 `workflow_samples`、`task_weekly_summary`、`task_writing_plan`、`task_project_summary`、`task_save_offline`。前端新增「离线流程」页签（`desktop/src/OfflineFlows.tsx`），成果只写入仓库外本地数据根。
3. **模型步骤如实标注。** 三个场景中的所有模型撰写步骤均标为「受准入阻塞、未验证：本次未调用任何模型」，未声称完成真实生成或质量评估。
4. **合成样例与证据。** 新增 `samples/workflows/synthetic-weekly.txt`、`synthetic-project.txt`、`desktop/src-tauri/examples/evaluate_offline_scenarios.rs`。

### 实际运行的测试

| 命令 | 结果 |
| --- | --- |
| `cargo test --offline`（`desktop/src-tauri`，配合 `scripts/setup-desktop-toolchain.sh --print-env`） | lib 62 passed / 0 failed；`host_integration` 13 passed / 0 failed；`task_shell_e2e` 3 passed / 0 failed；doc tests 0 |
| `npm run build`（`desktop`） | 通过（tsc + vite） |
| `cargo run --offline --example evaluate_offline_scenarios` | 通过，输出见证据 `offline-scenarios.txt` |

### 真实窗口验证（一次性 Xvfb :95，真实 Tauri 窗口）

- 周报合成样例：载入 → 本地处理 → 可见 4 条（done 2 / pending 1 / blocked 1）→ 勾选确认 → 保存成功。
- 项目跟进合成样例：done 1 / in_progress 1 / blocked 1 / not_started 1 / 逾期 2。
- 一般文案：无匹配模板时显示「未使用模板」，缺事实时列出「待确认」。
- 合成红线占位文本（纯虚构「内部资料占位」）被拒绝处理，只回显类别/计数，不回显原文。
- 保存路径在仓库外：`/tmp/od010-UCZXzQ/local-data/offline-weekly-report-20261018-015939.md`，986 字节；副本已收入证据目录。

### 证据路径

- [tests/verification/desktop/evidence/od-arch-010/README.md](tests/verification/desktop/evidence/od-arch-010/README.md)：场景、证据等级、隔离边界、限制与复现步骤。
- 同目录：`offline-scenarios.txt`、`saved-offline-weekly-report.md`、`window-01-workbench.png` 至 `window-07-redline-refusal.png`。

### 边界核对

- `git diff -- runtime/` 输出 0 行。
- `~/.dsh` mtime 仍为 `2026-10-15 20:17:43`，`~/.agents` 仍为 `2026-09-23 15:12:11`；未读取其内容。
- 未提交、未推送、未打标签；未修改 `REVIEW.md` 验收结论。

### 未验证与限制

- **模型撰写未验证：** 准入仍关闭，未调用模型；不得据本文声称真实生成、模型质量或业务收益。
- **用途未获准：** 周报、一般写作、项目管理在 `task_types()` 中仍为 `reserved`，不构成正式能力或用途获准；用户转述的授权仅限特定条件下的个人数据分析与生态研判。
- `parse_date` 只做基本范围校验，不拒绝 2 月 30 日等日历非法日期。
- 离线成果与既有分析共用本地数据根，以 `offline-` 前缀区分；是否单独子目录待 ARCHITECT 决定。
- 窗口操作经 XTEST 注入完成，属真实窗口运行，但不是人工手工点选；仅在 Linux + Xvfb 验证，跨平台未验证。

## OD-ARCH-009：端点准入证据审计（BUILDER 执行记录，2026-10-18）

- **状态：** 已执行，待 ARCHITECT 独立审查。只做只读追踪与文档记录；未改代码、未改 `ENDPOINT_ISOLATION_CONFIRMED`、未调用模型、未读凭证或真实业务材料、未提交/推送/打标签。
- **审计对象：** 计划与状态源 `/mnt/data/Code/MyWorkStation`（HEAD `5ffc710`）与 BUILDER worktree `/mnt/data/Code/omniharness-ui`（同 HEAD）。两边既有未提交改动全部保留，主工作区只增改 STATUS/REVIEW/PLANS/docs/decisions。
- **结论：no-go。** 现有代码不能证明模型端点、工具或数据去向被限制在已批准范围内；宿主也没有独立的端点强制点。最小合成路径存在，但它依赖环境变量与补丁注入，宿主任一时刻都无法据代码确认其生效。保持准入常量关闭。

### 源码追踪事实（文件事实）

1. **宿主到子进程的配置链。** `lib.rs` → `EngineConfig::from_env()` → `EngineBridge`。`engine_supervisor.rs:55` 起从 `OMNIDESK_PYTHON`、`OMNIDESK_ENGINE_DIR`、`OMNIDESK_DSH_CLI`、`OMNIDESK_DSH_HOME`、`OMNIDESK_AGENTS_HOME`、`OMNIDESK_ENGINE_STATE_DIR`、`OMNIDESK_DSH_PROFILE`、`OMNIDESK_DSH_PATCHES`、`HOME` 读取配置；另有 `OMNIDESK_REPO_ROOT`、`OMNIDESK_APP_DATA_ROOT`、`OMNIDESK_LOCAL_DATA_ROOT` 影响路径。这些是普通环境变量，没有第二道校验。
2. **子进程环境不是白名单。** `engine_ipc.rs` 的 `EngineChannel::spawn` 只对显式项调用 `cmd.env(key, value)`，未调用 `env_clear()`，因此继承宿主全部环境（含 `DEEPSEEK_API_KEY`、`DEEPSEEK_BASE_URL`、代理变量等）。`child_env()` 仅显式下发隔离 DSH home、agents home、CLI、Profile、state 五项。
3. **Python Adapter 同样继承。** `engine/omnidesk_engine/serve.py` 取 `dict(os.environ)`，仅覆盖 `DSH_HOME`、`DSH_AGENTS_HOME`；`acp_adapter.py` 以 `node <dsh-bin> --profile <profile>` 启动，附加 patch，会话请求固定 `mcpServers: []`，因此 OmniDesk 不挂 MCP。`DSH_HOME` 隔离只证明数据目录，不证明模型端点。
4. **实际模型路由与可覆盖入口。** 锁定版 `dsh-acp-app/cordis.patch.yml` 使用 `provider: deepseek-official`、`model: deepseek-v4-flash`；`dsh-llm-deepseek` 默认端点 `https://api.deepseek.com/anthropic`，可被显式 `baseURL` 或 `$DEEPSEEK_BASE_URL` 覆盖；凭证查找顺序为启动环境 > 凭证文件 > 项目 `.env` > Harness home `.env`。
5. **内置工具与外发通道默认存在。** `dsh-base` 默认启用 `web`、`web-search-deepseek`（`apiKeyEnv: DEEPSEEK_API_KEY`）、`web-fetch-http`、`bash-local`、`fs-local`、`spill-local`；`session-telemetry-otel` 默认 `FEEDBACK_ONLY`，端点默认生产收集器，`DSH_TELEMETRY_OTLP_URL` 可改，非空 `DSH_TELEMETRY_DISABLED` 可退出。`storage-json` 根在 `dshHomePath('storages')`，随 `DSH_HOME` 隔离。
6. **已有合成机制。** `scripts/dev-ui.sh` 启动本机回环 mock 模型服务，生成一次性 `synthetic-provider.yml`，经 `OMNIDESK_DSH_PATCHES` 把 ACP 路由改到 fake provider，使用占位密钥、零外网。该机制只在脚本层验证，宿主无独立校验，也没有证明实际下发给子进程的端点。

### 准入旁路检查

- 已确认存在：`OMNIDESK_DSH_PATCHES` 可注入 provider/baseURL 覆盖；继承的 `DEEPSEEK_BASE_URL`/`DEEPSEEK_API_KEY`/代理变量可改变数据去向；`OMNIDESK_DSH_CLI` 可指向任意 CLI；`OMNIDESK_DSH_PROFILE` 可换 Profile 从而换工具与路由。
- 已确认不存在：UI 参数、普通配置文件打开准入的通道；`ENDPOINT_ISOLATION_CONFIRMED` 为编译期常量 `false`，只能改源码改变。
- 未验证：DSH 从项目 `.env`、Harness home `.env` 载入端点的实际运行行为；`dsh-app-boot` 的分层环境合并细节只按 README 记录，未运行验证；工具调用是否实际外发未运行验证。

### 最小 fail-closed 设计（建议，未实施）

- **准入事实来源：** 需要一个宿主可核验的显式记录，说明「模型服务商 + 端点 + 设备 + 运行环境 + 用途」已获书面批准；不能只靠环境变量或提示词。当前没有任何此类记录，故默认拒绝。
- **宿主强制点：** 在启动引擎前构造受控子进程环境（白名单 + `env_clear()`），显式下发端点、密钥引用与工具开关；拒绝任何未列入批准的 baseURL、provider、patch 与代理；校验实际 `OMNIDESK_DSH_PATCHES` 内容而非路径存在性；对合成模式限定回环地址且无真实凭证。
- **默认拒绝条件：** 无批准记录、端点非白名单、检测到未批准 patch、检测到真实凭证与合成模式混用、无法读取实际生效配置时，一律拒绝且不启动子进程。
- **所需测试：** 环境继承阻断；`DEEPSEEK_BASE_URL` 注入阻断；patch 注入阻断；无凭证启动；真实端点拒绝；工具/MCP 关闭验证；日志不含秘密；拒绝后无目录、无进程、无任务副作用。
- **启动前置条件：** 先取得书面授权与端点证据，再实现端点守卫与测试，最后才讨论是否打开准入。本轮不打开。

### 未验证与限制

- 未运行任何模型、未启动引擎、未读取任何凭证或真实业务材料；DSH 分层 `.env`、工具外发与遥测行为仅为源码/README 事实。
- 主工作区不包含 `admission.rs`（仅存在于 BUILDER worktree），本轮审查以 worktree 源码为准；两边未提交改动不一致，不能整体合并。
- 本记录不构成端点隔离、真实用途授权或产品化批准；也不扩大用户已提供的答复摘要范围。

## 最近完成任务

- **OD-ARCH-005 与 OD-UI-002（OpenCode worktree）**：BUILDER 报告完成；ARCHITECT 在各自计划范围内复核通过。准入脚本 11/11、启动器身份测试和 diff 检查独立通过；工作台截图已审阅。端点隔离未确认，Rust 提交入口尚未 fail-closed；主工作区与 OpenCode worktree 的未提交改动不一致，不得整体合并。独立结论和证据位置见 [REVIEW.md](REVIEW.md) 顶部。

- 任务：[PLANS.md](PLANS.md) 中的 **OD-BUILD-005：把已实现的引擎链路接进日常任务流程（第一个能真用的纵向流程）**（已实施；本机脚本/运行验证通过；**待 ARCHITECT 独立复核**）。
- 授权：用户下达“批准OD-BUILD-005执行”的执行指令，本计划视为已批准执行；[PLANS.md](PLANS.md) 中该计划原有的“待用户批准；未授权执行”一句由本次用户指令取代。
- 触发背景：用户在实际使用中报告真实 OmniDesk 桌面窗口**整屏全黑**并弹出系统提示「"Omnidesk-desktop"无响应」。本轮先定位并修复该阻断问题（否则本计划验收标准第 1 条的“真实窗口内走完流程”无法达成），再完成“引擎接线 + 窗口内流程验证”。
- 角色：BUILDER（实施工程师），范围限定在 OD-BUILD-005 列明的影响范围内；未开发 GUI、未新增协议、未接入真实模型、未改动模型路由或凭证。

## 已完成内容

### 一、阻断缺陷修复：引擎事件泵持锁空转饿死宿主（窗口“无响应”的根因）

- **现象**：桌面窗口启动后整屏无法渲染、系统报“无响应”。
- **根因（静态检查 + 运行证据）**：`engine_bridge.rs` 的事件泵旧实现在**持有 `EngineSupervisor` 互斥锁**的情况下做 `recv_notification_timeout(500ms)` 阻塞等待；每次超时后立即重新加锁，形成紧循环。宿主锁被永久占据，导致任何需要该锁的调用被饿死——而桌面 UI 启动即调用的 `engine_health`（`desktop/src/App.tsx`）恰好运行在 GTK 主线程上，于是主线程无法推进，窗口表现为整体“无响应”。这不是渲染或前端缺陷。
- **修复（最小改动，不改语义）**：把通知接收端改为可共享句柄，事件泵只借用**一次**锁换取该句柄，此后完全独立于 supervisor 互斥锁。
  - `engine_ipc.rs`：`notif_rx` 改为 `Arc<Mutex<Receiver<..>>>`；新增 `notification_receiver()` 与 `exited_flag()`，让泵无需再向 supervisor 取锁即可独立阻塞等待并判断子进程是否退出。
  - `engine_supervisor.rs`：新增 `notification_handle()`，一次调用返回「通知接收端 + 退出标志」；取不到句柄（引擎未启动）时返回 `None`。
  - `engine_bridge.rs`：`start_pump` 改为接收句柄，不再触碰 supervisor 锁；子进程退出时的“事件流已中断”提示改由退出标志判定，行为与旧实现一致。`ensure_initialized` 在**已持 guard** 时就地取句柄传入（该互斥锁不可重入，第一版曾因在泵内再取锁而死锁，已修正）。
- **回归测试**：`host_integration.rs` 新增 `event_pump_does_not_starve_host_lock`——泵运行期间连续 5 轮探测 `health()`，每轮必须在 2 秒内返回（旧实现下会明确失败，而不是无声挂起）。

### 二、引擎接线：工作台「阶段 3 · 解释」改为可直接调用引擎

- **修复前的实际状态**：`desktop/src/workbench.tsx` 全文只调用 `shell.*` 本地命令，从不调用引擎；引擎调用只存在于技术切片视图。因此用户此前反馈的“无法下指令、看不到结果”不是能力缺失，而是已实现的能力没有接到日常流程上。
- **本轮接线（`workbench.tsx`）**：在阶段 3 增加「用引擎生成解释」支路，顺序为——
  1. 冻结提示词（`shell.explanationPrompt`，本地重算 + 指纹比对，指纹不符即在此失败）；
  2. 准备隔离工作目录（`shell.prepareWorkspaceDir`，新增命令）；
  3. `host.submitTask` 提交（`intent` = 任务类型标识）；
  4. 轮询宿主状态快照直到终态——**终态只认宿主回读的状态快照，事件仅用于显示与流式预览**；
  5. 终态为 `succeeded` 时取 `host.taskResult` 回填解释框，来源固定 `model_unverified`。
- **新增宿主命令**：`task_prepare_workspace_dir`（`lib.rs`）→ `task_shell::prepare_local_data_root()`（幂等创建本地数据根，复用与保存入口同一套“禁入片段 / 不得位于仓库内”边界校验，只建空目录、不写内容），前端绑定为 `shell.prepareWorkspaceDir()`（`api.ts`）。新增 2 个单测覆盖幂等性与禁区拒绝。
- **保留的行为约束（未降低标准）**：输入或统计一变，引擎解释、提示词、取消状态全部作废；失败 / 取消 / 超时 / 状态未知都**只报原因、不写成功、不改写用户已写的解释**；来源下拉**不存在任何“已复核”选项**；未人工确认不得保存；沿用现有「生成提示词 → 手动粘贴」回退路径，两条路径并存且状态互不污染。

### 三、真实 Tauri 窗口五阶段流程跑通，证据入库

- 在真实 OmniDesk 桌面窗口内，用**内置合成样例**走完「输入 → 确定性统计 → 引擎解释回填 → 人工确认 → 保存到仓库外」并真实落盘产物。产物经核对：统计指纹与阶段 2 一致、冻结统计块标注禁止重算、解释来源 `model_unverified`、`user_confirmed = true`、原始输入原文保留未被覆盖。
- 证据目录：`tests/verification/desktop/evidence/od-build-005/`（README + 6 张窗口截图 + 产物副本）。该 README 同时记录两项**本机环境问题**（均非应用缺陷，供后续复现时不再误判）：
  1. **窗口全黑**：本机 Wayland + XWayland + 混合显卡下 WebKitGTK 的 DMABUF 渲染器失败；注入 `WEBKIT_DISABLE_DMABUF_RENDERER=1` 后渲染正常（distinct 颜色数约 43 → 3000+）。属**本机 WebKitGTK 渲染规避**。
  2. **点击无反应**：用户真实会话 `:0` 上 XTEST 合成的按键事件被会话层吞掉（指针移动可达、按键不可达）；已用最小对照窗口（GTK / WebKit `mousedown` / Tkinter）逐层排除，**与 OmniDesk 无关**，改用 Xvfb 干净显示器后点击正常。
- **修复后窗口渲染/响应性复核（2026-10-17 22:36，干净 Xvfb `:82`）**：直接启动修复后的 debug 二进制，窗口成功映射为 `1280x820`，可渲染（颜色数 12708，全黑时约 43），`WebKitWebProcess` 与引擎子进程均存活，**未再出现“无响应”**。截图 `smoke-window-rendered.png`。
  - **如实标注**：该次截图的前端来自同一仓库的另一个 worktree（`/mnt/data/Code/omniharness-ui`，分支 `feat/desktop-ui`，同 HEAD `5ffc710`），因为 debug 构建按 `devUrl` 从 `127.0.0.1:5273` 加载前端，而当时该端口被那个 worktree 的 Vite 占用。**因此该图只证明 Rust 宿主窗口已能渲染并保持响应（锁饥饿修复生效），图中界面内容不属于 OD-BUILD-005 产物**；本计划自有前端（含引擎解释支路）的窗口证据仍是 §3 的 `t1_after.png` … `t5_after.png`。

## 修改文件

本轮由 BUILDER 改动：

| 文件 | 改动 |
| --- | --- |
| desktop/src-tauri/src/engine_ipc.rs | `notif_rx` 改 `Arc<Mutex<Receiver<..>>>`；新增 `notification_receiver()`、`exited_flag()` |
| desktop/src-tauri/src/engine_supervisor.rs | 新增 `notification_handle()`（一次取句柄）；默认 Profile 注释澄清“引擎子进程 Profile”与“Web UI Profile”是两件事 |
| desktop/src-tauri/src/engine_bridge.rs | `start_pump` 改为使用独立句柄，不再持 supervisor 锁空转（锁饥饿修复） |
| desktop/src-tauri/src/lib.rs | 新增 `task_prepare_workspace_dir` 命令并注册 |
| desktop/src-tauri/src/task_shell.rs | 新增 `prepare_local_data_root()` 及 2 个单测（幂等、禁区拒绝） |
| desktop/src-tauri/tests/host_integration.rs | 新增 `event_pump_does_not_starve_host_lock`；假引擎夹具 Profile 注释对齐 |
| desktop/src/api.ts | 新增 `shell.prepareWorkspaceDir()` 绑定 |
| desktop/src/workbench.tsx | 阶段 3 接入引擎解释支路（提交 / 事件订阅 / 终态轮询 / 结果回填 / 取消），并保留原手动路径 |
| desktop/src-tauri/examples/frozen_prompt.rs | **新增**（未跟踪）：把 Rust 真实产出的冻结提示词与工作目录交给验证脚本，避免脚本自拼提示词绕过唯一数值来源 |
| tests/verification/desktop/run_workbench_engine_path.py | **新增**（未跟踪）：工作台「用引擎生成解释」路径的回环验证脚本 |
| tests/verification/desktop/evidence/workbench-engine-path.json | **新增**（未跟踪）：上者的证据产物 |
| tests/verification/desktop/evidence/od-build-005/ | **新增**（未跟踪）：真实窗口五阶段证据（README + 截图 + 产物副本） |
| STATUS.md | 本执行记录 |

未改动 `runtime/`（`git diff --stat -- runtime/` 为空）、模型路由、凭证、`~/.dsh`、`~/.agents`、`~/OmniDesk`、`~/YvanAI` 或任何真实工作材料。既有未提交改动（`GOALS.md`、`PLANS.md`、`README.md`、`REVIEW.md`、`docs/OmniDesk-Desktop-Data-Layout.md`、`docs/decisions.md`、`engine/omnidesk_engine/acp_adapter.py`、`tests/verification/**/evidence/*.json` 等）原样保留，未重置、未覆盖、未暂存、未提交、未推送、未打标签。

## 测试命令与结果（本轮实测）

证据等级说明：`cargo test` / `npm run build` / `py_compile` 属**静态检查与本地构建**；两个 Python 验证脚本属**脚本/运行**（真实引擎子进程 + 真实 DSH CLI + 隔离一次性数据根 + 本机回环**合成**模型端点，零凭证、零外网）；**“真实模型运行”本轮未执行**（见未验证项）。真实 Tauri 窗口五阶段流程属**脚本/运行（窗口级）**，证据见 `tests/verification/desktop/evidence/od-build-005/`。

| 命令 | 结果 | 证据等级 |
| --- | --- | --- |
| `cd desktop/src-tauri && cargo test --offline` | **64 passed / 0 failed**：lib 47（含新增 `prepare_local_data_root_is_idempotent_and_outside_repo`、`prepare_local_data_root_rejects_forbidden_root`）、host_integration 14（含新增 `event_pump_does_not_starve_host_lock`）、task_shell_e2e 3；doc-tests 0 | 静态检查 |
| `cd desktop && npm run build`（tsc --noEmit + vite build） | 通过；22 modules，250.67 kB JS / 7.48 kB CSS | 静态检查 |
| `python3 -m py_compile engine/omnidesk_engine/acp_adapter.py` | OK | 静态检查 |
| `python3 tests/verification/desktop/run_workbench_engine_path.py` | **pass** `{'success': ('pass','succeeded'), 'failure': ('pass','failed'), 'cancel': ('pass','cancelled')}` | 脚本/运行（合成端点） |
| `python3 tests/verification/desktop/run_desktop_e2e.py chain` | **pass** `{'success': ('pass','succeeded'), 'failure': ('pass','failed'), 'cancel': ('pass','cancelled')}` | 脚本/运行（合成端点） |
| 真实 Tauri 窗口五阶段（`:79` Xvfb + 合成端点） | **pass**：五阶段全走通，产物落盘 `/tmp/od011/run0/local-data/20261016-182827-320dae745102.md`（1739 字节）；截图见证据目录 | 脚本/运行（窗口级） |

`cargo` 前需注入 Tauri 依赖的 `PKG_CONFIG_*` 与 `LIBRARY_PATH`（前缀 `$HOME/.local/omnidesk-tauri-deps`，可由 `scripts/setup-desktop-toolchain.sh --print-env` 打印）；此为本机约定，未改动仓库。`cargo clippy` 在本机未安装（`cargo-clippy` 组件缺失），非计划要求的检查项。

## 未验证 / 未运行事项

- **未执行任何真实模型调用。** 上述“脚本/运行”场景的模型一端全部是本机回环**合成**端点（零凭证、零外部网络），只覆盖引擎连接、事件、终态与错误语义，**不代表任何真实模型（Agents / DeepSeek / GLM / GPT / Claude 等）的输出质量或已获授权**。合成端点通过 ≠ 真实模型运行。
- **跨平台未验证**；`/proc` 进程检测、`realpath` 语义与 WebKitGTK 渲染规避只在当前 Linux 实测。
- **窗口级自动化未验证**：本轮窗口内点击由 XTEST 合成投递完成，仓库内尚无可复现的窗口自动化脚本；浏览器/UI 自动化属独立决策，不在本阶段范围。
- **未接入正式制度库**；`~/OmniDesk` 与 234 份制度文件本轮未读取、未复制。
- **未新增仓库内桌面启动器**：`WEBKIT_DISABLE_DMABUF_RENDERER=1` 属本机 WebKitGTK 渲染规避，尚未进入仓库；`scripts/` 不在 OD-BUILD-005 计划列明的影响范围内，BUILDER 未擅自新增，见下方建议。
- **未缩短 ACP 握手等待**：非 ACP Profile 仍为约 30 秒有界失败（承 OD-BUILD-004 裁决）。

## 是否发现 Plan 与实际代码存在冲突

- 未发现阻断性冲突。计划第 1 步要求的插入点（阶段 3 的 `analysis` 指纹 / `promptText` / `explanation` / `source` / `confirmed` 状态）与实际代码一致，可直接接入。
- **两处需 ARCHITECT 决定的落差（如实登记，未自行扩大范围）：**
  1. **窗口“无响应”是本轮计划外发现的阻断缺陷。** OD-BUILD-005 的验收标准第 1 条要求“真实 Tauri 窗口内走完流程”，但窗口根本无法渲染/响应，该验收项无法达成。已按“普通实现问题可自主解决、架构/依赖/安全机制变更须停下”的规则判定：本次修复只调整事件泵的**锁使用方式**，未改变 IPC-1 契约形状、未新增依赖、未改动引擎协议语义，属普通实现缺陷，故由 BUILDER 修复并登记，**请 ARCHITECT 重点复核此边界判断是否恰当**。
  2. **是否单列“仓库内桌面开发启动器”**（含本机 `WEBKIT_DISABLE_DMABUF_RENDERER=1` 渲染规避与隔离数据根校验），以便后续无需手工拼环境变量即可复现真实窗口路径。证据 README §6 已写明该建议。BUILDER 未新增，等待 ARCHITECT 决定。
  3. **多 worktree 的开发端口冲突（本轮新发现，未处理）**：`/mnt/data/Code/MyWorkStation`（`main`）与 `/mnt/data/Code/omniharness-ui`（`feat/desktop-ui`）是同一仓库的两个 worktree，二者 `tauri.conf.json` 的 `devUrl` 都是 `http://127.0.0.1:5273` 且 Vite 为 `strictPort`。先启动的一方占用端口，另一方 `npm run tauri:dev` 会**静默加载到别人的前端**（本轮冒烟即撞上，见证据 README §7）。这是本地开发环境约定问题，不是应用缺陷；是否改为可配置端口或按 worktree 分配端口，请 ARCHITECT 决定，BUILDER 未擅自改动。

## 建议 ARCHITECT 下一步关注

1. **优先复核锁饥饿修复的边界判断**：该修复是否确实属“普通实现缺陷”而非需要预先审批的架构/安全变更；以及 `notification_handle()` 是否引入了新的并发风险（例如泵与请求路径同时持有通知接收端锁）。
2. **复核引擎接线的语义合规性**：确认终态只认宿主状态快照、来源固定 `model_unverified`、界面不存在任何“已复核”来源、未确认不得保存，且 IPC-1 形状未变、前端代码未出现 ACP/DSH 私有概念。
3. **判定是否将“仓库内桌面启动器”单列为后续计划**（OD-BUILD-006 候选之一；另一候选是本地文件选择器）。
4. 复核后更新 [REVIEW.md](REVIEW.md)。在通过前 MVP 维持未验收，不得据此扩大真实材料使用。
5. **文档过时提示（本轮未改）**：`docs/OmniDesk-Desktop-Architecture.md` §6 表格仍写“`desktop/` 工程：尚未创建”“Rust 工具链：本机缺失”，已明显过时；建议由 ARCHITECT 或单独计划收敛。

---

## 历史记录（保留，未改动其结论）

### OD-BUILD-004：闭合引擎 Profile 启动与错误验收（ARCHITECT 独立复核通过（计划范围））

- 任务：[PLANS.md](PLANS.md) 中的 **OD-BUILD-004：闭合引擎 Profile 启动与错误验收**（两项缺口均已实现并有本机运行证据；ARCHITECT 独立复核通过（计划范围），结论见 `REVIEW.md` 顶部同名章节）。
- 背景：OD-BUILD-003 遗留两项未闭合缺口——**缺口 1**：R1 场景由脚本手工注入 `acp` 并显式传入，未证明 Rust 宿主**默认配置链**本身就产出 `acp`；**缺口 2**：已存在但非 ACP 的 Profile（如 `web`）约 30 秒后只以通用 `timeout` 失败，错误里没有 Profile 名，也没说明 OmniDesk 预期 ACP。
- 已完成：**缺口 1**——在 `engine_supervisor.rs` 用新测试 `engine_config_default_profile_chain_is_acp` 取代旧单测：先清空 `OMNIDESK_DSH_PROFILE`（即真的“宿主不设置该变量”），再直接调用 `EngineConfig::from_env()`，断言 `profile == "acp"`、`child_env()` 与 `initialize_params()["dsh"]["profile"]` 均为 `acp` 并通过 `validate()`；随后验证显式覆盖贯穿两处、纯空白覆盖回退 `acp`。测试用 `OWNED_ENV_KEYS` 快照/还原，且不触碰 `HOME` / `OMNIDESK_REPO_ROOT` / `OMNIDESK_LOCAL_DATA_ROOT`。**缺口 2**——在 `acp_adapter.py` 增加 `_profile_phrase()` 与 `_profile_detail()`，只改 `_request()` 的 TIMEOUT 分支：`message` 追加“所配 Profile 期望讲 ACP 但未完成握手”的说明，`detail` 补充 `{method, expected_protocol:"acp", engine_profile, handshake:"acp/initialize", reason}`。**未修改** `serve.py`、未改错误码、未读 DSH 私有 Profile 结构，等待时长**未缩短**。
- 当时验证：`cargo test --offline` 61 项通过；`npm run build` 通过；R1/R1B/R2/R3/R5 全部 pass（R2 11/11 检查项）；E01/E08/E10/E16/E17、E20 通过；桌面 E2E chain 通过。
- 裁决：启动前预检不作为当前验收阻断项，登记为低优先级后续项（理由见 `REVIEW.md`）。

### OD-BUILD-003：引擎 Profile 默认与 cwd 规范化最小修复（ARCHITECT 部分通过）

> 以下“待修复/未验证”状态为**当时**的历史状态，其剩余缺口已由 OD-BUILD-004 闭合。

- 任务：[PLANS.md](PLANS.md) 中的 **OD-BUILD-003：引擎 Profile 默认与 cwd 规范化最小修复**（已实施，本机验证通过；ARCHITECT 部分通过）。
- 背景：OD-ARCH-003 只读审计登记两项实现问题——Finding A：桌面引擎默认 Profile `yvan-work` 打包的是 Web bundle，默认路径下不产生 ACP 会话；cwd 身份不对称——会话**存储**按原始 cwd 字符串分组，而**工作区注册表与 ACP list/resume** 按 realpath 分组，同一物理目录的两种写法会分裂成两个存储项目目录。
- 已完成：**修复 1（默认 Profile 对齐 ACP）**——`EngineConfig::from_env()` 默认 Profile 由 `yvan-work` 改为 `acp`，显式覆盖与空白回退按契约处理；**修复 2（cwd 单一规范化落点）**——`acp_adapter.py` 新增模块级 `canonical_cwd()`（`os.path.realpath(strict=True)`；不可解析路径明确抛 `INVALID_ARGUMENT`），只接入 `submit_task` 的 `workspace_dir` 与 `recover_session` 的 `target_cwd` 两处，无其它 realpath。
- 当时验证：`cargo test --offline` 61 项通过；`npm run build` 通过；R1/R1b/R3/R5 pass；E01/E08/E10、E20（除 `repo_working_tree_status_unchanged` 子项）、桌面 E2E chain 通过。
- 当时遗留并已由 OD-BUILD-004 处理的两项：宿主默认配置链未被直接证明（R1 手动注入 `acp`）；已存在但非 ACP 的 `web` Profile 约 30 秒后以通用超时失败、错误未指明 Profile 与 ACP 预期。
- 补充（Finding B 现状）：分析功能对人不可用（Finding B）已在 OD-BUILD-005 的引擎接线中一并解决——工作台阶段 3 现可直接调用引擎。

### OD-BUILD-002：拒绝桌面应用数据根落入仓库（ARCHITECT Review 通过）

- 目标：在 `AppPaths` 数据根解析和目录创建阶段拒绝仓库内路径、指向仓库的符号链接和仿真禁区，同时允许干净的仓库外路径。
- 实施：新增 `assert_outside_repo_boundary`；`assert_isolated()` 同时校验词法路径与 `resolve_real_path()` 真实落点；`ensure()` 在创建目录前先校验；新增 5 个单测。实施文件：`desktop/src-tauri/src/app_paths.rs`、`STATUS.md`；无须修改 `lib.rs`。
- 独立验证：`cargo test --offline` 60 项通过；`npm run build` 通过。checkpoint 的 `git diff --check HEAD^ HEAD` 有历史行尾空白/末尾空行问题，详见 `REVIEW.md`。
- 限制：常规路径校验不能消除检查与创建之间的并发替换竞态；当时仅在 Linux 实测，不代表跨平台已验证。

### OD-BUILD-001：个人 MVP 保存确认、来源真实性与输入处理边界

- 实施 R1 保存强制人工确认、R2 移除可伪造的 `model + succeeded` 来源、R3 统一输入闸门、R4 路径真实落点校验。
- ARCHITECT 复核：R1–R3 通过；R4 不通过（P1）——产物路径侧已覆盖真实落点，但桌面应用数据根 `AppPaths` 未验证仓库边界，可被指向仓库。R4 经 OD-BUILD-002 补齐后通过。
- 详细结论保留在 `REVIEW.md`，执行事实保留在本文件历史记录。

### OD-ARCH-003：v0.4 架构校准与会话丢失机制审计

- 只读审计 + 隔离一次性合成验证；登记 Finding A（默认 Profile 非 ACP）与 Finding B（分析功能对人不可用），**只登记不修复**。Finding A 由 OD-BUILD-003 修复，Finding B 由 OD-BUILD-005 解决。

### 更早状态（OD-ARCH-001 文档收敛与审查基线）

- 最近完成的 ARCHITECT 工作：OD-ARCH-001 独立审查与共享文档基线收敛；审查结论 MVP 不通过（R1–R4）。
- 5 份历史规划材料移至 `docs/archive/` 保留，未删除；协作基线集中在仓库根 `AGENTS.md`、`GOALS.md`、`PLANS.md`、`STATUS.md`、`REVIEW.md` 与 `docs/decisions.md`。
- 审查时 HEAD `17efd20`；工作区 175→181 项，暂存区为空；所有差异保留，未提交、未推送、未打标签。
