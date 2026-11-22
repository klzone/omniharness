# OD-PRJ-601 BUILDER 任务指令：项目跟踪模块完善迭代

> 依 `docs/BUILDER-Task-Instruction-Template.md` 撰写。计划正文以 `PLANS.md` 的 OD-PRJ-601 小节为唯一实施口径；本文只负责把范围、顺序、证据与禁止项说清楚，不放宽任何一条。

## 任务编号与名称

OD-PRJ-601：项目跟踪模块完善迭代（任务完成确认 + AI 复盘 + 项目维护区）

## 获批依据

- 计划位置：`PLANS.md` → 「OD-PRJ-601：项目跟踪模块完善迭代」。
- 批准时间与批准人：2026-11-23，用户当轮明确授权执行（含子代理派工：模型 `OpenDesign/deepseek-v4.1-flash-eco`，思考等级 `high`）。
- 开工基线（ARCHITECT 实测 2026-11-23）：HEAD `a1388be6fe`，工作区有 21 处既有未提交改动 + 1 个未跟踪文件 `desktop/src/ErrorBoundary.tsx`（0.2.49/0.2.50 已验证成果）。全部保留，不得 reset / clean / stash / checkout 任何既有改动。

允许修改的文件或目录：

| 范围 | 具体路径 | 说明 |
| --- | --- | --- |
| 前端 | `desktop/src/shell/TaskManager.tsx` | 四象限完成确认、复盘入口、项目维护区、分页 |
| 前端 | `desktop/src/api.ts`、`desktop/src/types.ts` | 新增复盘命令封装与类型 |
| 前端 | 必要时新增 `desktop/src/shell/ProjectMaintenance.tsx`、`ProjectReviewCard.tsx` | 若拆文件，从 TaskManager 挂载 |
| 宿主 | 新增 `desktop/src-tauri/src/project_review_store.rs` | 项目复盘存档 |
| 宿主 | `desktop/src-tauri/src/lib.rs` | 新增 3 个复盘命令 + 注册 |
| 验证 | `tests/verification/desktop/verify_project_601.py`（新增）、`tests/verification/desktop/evidence/od-prj-601/`（新增） | 静态检查 + 证据 |
| 状态 | `STATUS.md` 顶部追加执行记录 | 只追加，不改既有段落 |

明确不做：

- 不修改 `runtime/` 内任何文件；不动模型路由、凭证、Profile。
- 不改右侧四张统计卡（今日新增/进行中/已完成/逾期）的数字口径。
- 不删除或改动 `desktop/src/shell/ProjectBoard.tsx`（历史文件，不挂载也不删）。
- 不改 `priority_rules.rs` 的分类算法；不改周期复盘（period review）既有链路。
- 不做版本号变更、CHANGELOG、git commit / push / tag（由 ARCHITECT 复核后统一处理）。
- 不读取、不落盘任何真实业务材料；测试只用合成数据。

## 开工前检查（只读）

1. `cd /Users/yvan/Code/MyWorkStation && git status --porcelain && git rev-parse --short HEAD`，记录快照；确认 `git status --porcelain runtime/` 为空。
2. 精读以下现状（ARCHITECT 已核实，不要重复考古）：
   - `desktop/src/shell/TaskManager.tsx`：TaskQuadrantBoard（886 行起）page 变体 = 左列 matrix + gantt、右列 statsPanel + completedPanel；任务点击只开编辑弹窗（约 1447 行）；shell.taskTransition 在列表视图 188 行已用，四象限弹窗未接。
   - displayViews（约 948 行）把「待评估」并进 Q2 展示；views 含已完成任务（active=false），当前未过滤。
   - `desktop/src/api.ts`：taskTransition（1140 行）、taskUpdate（1132 行，patch 含 project_id）、taskCreate、taskDelete、projectCreate（2826 行）、projectUpdate、projectList、projectOverview（2845 行）、knowledgeReviewStage（2172 行）全部已存在。
   - `desktop/src-tauri/src/project_store.rs`：项目实体 work/项目/<id>.md，create/update/list 齐备。
   - `desktop/src-tauri/src/review_store.rs`：write_review（731 行）的校验模式（章节标题白名单 + #编号 引用校验 + 草稿/已确认状态机 + .bak 备份），项目复盘 store 模仿这套模式。
   - `desktop/src/shell/ReviewArchive.tsx`（260–400 行）：prepareWorkspaceDir → host.submitTask → taskStatus 轮询 → taskResult → 存档命令的既有模式，直接复用。
   - `desktop/src/shell/ConfirmDialog.tsx`：二次确认弹窗组件，复用。

## UI 参考（涉及界面时必填）

动工前先调用 screenshots-design-inspiration MCP（search_inspiration，关键词如 project task dashboard、kanban completion）看 2–3 个同类产品，在 STATUS.md 执行记录里写 2 句以内借鉴要点。MCP 不可用时如实记录，不得编造。视觉沿用本应用暖纸新拟物令牌（--od-neu-*、NEU_CARD/NEU_TILE/NEU_INSET），不引入新配色。

## 实施要求（按顺序做，每步可独立验收）

### S1 四象限只显示未完成任务（归档展示）

- displayViews 过滤：taskMap 中 state 为 已完成 或 已取消 的条目不再进入四象限 matrix（甘特图与已完成列表口径不动）。暂停任务保留显示。
- 不改宿主 task_quadrant_views 返回内容（工作记录页 panel 变体共用，前端过滤即可）。

### S2 任务完成确认（防误点）

- 四象限编辑弹窗底部加「标记完成」按钮，与「保存修改」视觉区分。
- 点击后弹 ConfirmDialog 二次确认，正文明确任务标题与「完成后从四象限归档」；确认后才执行：
  - 状态 待开始/进行中/暂停 → 直接 taskTransition(id, "已完成")；
  - 状态 待确认 → 先 taskTransition(id, "待开始") 再 taskTransition(id, "已完成")（两步都成功才算完成；中途失败如实报错）。
- 完成后刷新（setTick），任务从四象限消失、出现在已完成列表。

### S3 AI 辅助复盘（完成后入口 + 存档 + 知识卡）

宿主（Rust，新增 project_review_store.rs）：

- 路径：work/复盘/项目/<project_id>.md（一个项目保留最新一份复盘）。
- frontmatter：schema omnidesk.project-review.v1、project_id、task_ids（列表）、generated_at、status（草稿/已确认）。正文五个固定二级标题（顺序不变）：## 项目概况、## 任务完成情况、## 问题与不足、## 经验与做法、## 后续建议。
- save_project_review(root, project_id, task_ids, content, now)：校验 project_id 形状（复用 worklog_records::is_safe_id）、正文非空、≤1MB、五个标题齐全不重复、正文里 #t-… 引用必须都在 task_ids 内；草稿可覆盖写（已有文件先 .bak 备份，复用 write_replace_atomically），status=已确认 拒绝覆盖并报错。
- read_project_review(root, project_id)：不存在返回 exists=false，不建文件；校验 frontmatter 与 project_id 一致。
- confirm_project_review(root, project_id)：仅草稿 → 已确认，记录 confirmed_at。
- 三个命令 project_review_read / project_review_save / project_review_confirm 注册进 lib.rs invoke_handler。
- 单测（模仿 review_store 测试）：保存+读取 round-trip、标题缺失/重复拒绝、#t-… 野引用拒绝、草稿覆盖留 .bak、已确认拒绝覆盖、confirm 状态机。

前端（TaskManager 或新组件）：

- 编辑弹窗完成确认成功后，以及「已完成」列表每项，提供「AI 复盘」入口。复盘面板放在右列「已完成」卡片下方，标题「项目复盘」：
  1. 项目选择器（projectList）+ 材料汇集：projectOverview(projectId) 拿项目字段与任务清单，对该项目任务（≤20 条）逐条 taskRead 取进展与状态历史，拼成材料文本（只含编号、标题、状态、进展原文，不调模型汇总）。
  2. 提示词：固定契约 = 五个标题 + 只引用材料里出现过的 #t-… 编号 + 材料没写的不编造 + 只返回 Markdown 正文；不复用 composeConversationPrompt（period-review 是周期口径），直接拼提示词并在 STATUS.md 说明理由。
  3. 提交：prepareWorkspaceDir → host.submitTask({ promptText, intent: "project_management", workspaceDir, userInputText: "项目复盘 " + projectId }) → 轮询 taskStatus（1 秒间隔；10 分钟后如实提示继续等待，不自动取消）→ taskResult 取正文。
  4. 落盘：project_review_save(projectId, taskIds, content) → MarkdownText 渲染草稿；「确认归档」按钮 → project_review_confirm；已确认后不可再生成覆盖（如实提示）。
  5. 「提炼知识卡」按钮：对已保存的复盘正文调 shell.knowledgeReviewStage(content, [复盘文件路径])，成功后提示去「知识审核」队列裁决；失败如实报错。
- 未挂接项目的任务完成时，复盘面板下拉选「（未挂接项目）」并禁用生成，提示先在项目维护区挂接。

### S4 项目维护区（右侧卡片下方，重点）

右列在 completedPanel 之后新增「项目」卡片（复用 NEU_CARD 材质）：

1. 项目列表：projectList 下拉选择器 + 状态徽标（进行中/已完成/已搁置）+ 起止日期 + 目标摘要。
2. 新建项目：按钮弹窗 → 名称（必填）、目标、状态（默认进行中）、开始/结束日期（可选）→ projectCreate；成功后刷新并选中新项目。
3. 项目任务列表（分页）：projectOverview 的任务 + taskList 状态合成；每页 8 条，底部分页控件（上一页/下一页 + n / m）。每行：任务标题、状态徽标、截止日期、所属项目。
4. 添加任务：弹窗两种方式 = ① 从未挂接项目的现有任务中选（下拉列出 project_id 为空且非终态任务）→ taskUpdate(id, { project_id })；② 新建任务（标题必填 + 截止可选）→ taskCreate({ title, due_date, project_id })（以 types.ts 实际字段为准，不臆造字段）。
5. 换绑/摘除：行内下拉把任务换到其他项目或（未挂接）（taskUpdate.project_id 置空串），操作前 ConfirmDialog 确认。
6. 删减：行内「删除」按钮 → ConfirmDialog（明确「删除任务文件，可在数据根垃圾桶找回」）→ taskDelete；项目本身不提供删除。
7. 读写失败都如实显示错误原文，不把失败当成空列表。

### S5 右侧四卡片

statCards 数字口径与渲染完全不动；新增卡片放在统计卡与已完成之后，不挤压原布局。

## 验证（每项标注证据等级：文件事实 / 静态检查 / 脚本运行 / 未验证）

1. `cd desktop/src-tauri && cargo test --lib --offline -- --test-threads=1` 全绿（含新增 project_review 单测）。
2. `cd desktop && npx tsc --noEmit` → EXIT=0。
3. `cd desktop && npm run test` → 全过。
4. `cd desktop && npm run build` → EXIT=0。
5. 新增 `python3 tests/verification/desktop/verify_project_601.py`（静态检查：完成确认二次弹窗、四象限过滤已完成、复盘五标题契约、项目维护区 CRUD 接线、分页控件、统计卡未被改动）→ 全过；输出写入 `tests/verification/desktop/evidence/od-prj-601/verify_output.txt`。
6. 所有合成测试数据走临时目录或宿主测试夹具，不写入真实数据根 ~/OmniDesk/；如 dev 实跑验证界面，用合成任务并在结束后清理，清理动作记录进 STATUS.md。

## 硬约束

- 不修改 runtime/；不动凭证与模型路由；不提交、不推送、不打标签。
- 保留全部既有未提交改动；发现既有改动与本任务冲突时停止相关文件并写 STATUS.md。
- 单文件改动保持可读：不压缩行、不 golf；复用既有组件（ConfirmDialog、Icon、MarkdownText、NEU_* 材质常量）。

## 交付

完成后在最终回复里给出：改动文件清单（含新增）、每项验证的实际命令与结果、证据路径、未验证项、计划偏差、`git status --porcelain` 尾部快照。写完 STATUS.md 执行记录后停下，等待 ARCHITECT 复核。

