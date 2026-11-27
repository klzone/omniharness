# 执行状态

## 写作需求解析兜底与创建流程收敛（2026-11-27，0.2.62）

- **代码（file fact）：** knowledge_ingest::parse_result_text 增加代码围栏剥离、外层对象截取，以及全角结构标点、正文裸引号、字符串内裸换行和尾逗号的保守修复；无法修复时仍报原始 JSON 错误。writing_parse 单测覆盖围栏/解释文字、全角逗号、中文引号、裸引号标题、裸换行、尾逗号与截断输出。
- **前端（file fact）：** WritingWorkbench 从对话带入后自动整理；整理失败按原文生成可编辑草稿并保留错误详情与重试；核对页保留类型/标题/周期/篇幅，其余条件折叠；创建成功即推进到「素材收集中」。WritingDesk 打开稿件前刷新任务状态，书桌抽屉在带入时显示「带入写作」。
- **验证（script，实际命令与结果）：** `cargo test writing_parse --lib` → 15 passed；`cargo test knowledge_ingest --lib` → 29 passed；`cargo test --lib` → 1374 passed / 1 ignored；`npx tsc --noEmit` → 0；`npm run test` → 150 passed；`npm run build` → 0；`verify_writing_094` → 70/70；`verify_writing_ui_610` → 26/26；`verify_writing_review_612` → 28/28；`verify_hardening_2043` → 77/77。
- **未验证：** 真实模型输出下的长对话整理、真实 Tauri 窗口交互、Windows 实机。

## OD-0255 执行记录（2026-11-24）

- **基线（file fact）：** main 最新提交 0.2.54（dccc5cc215）；开工 git status --porcelain 仅未跟踪 docs/od-0255-dispatch.md，既有改动原样保留，未 reset/clean/stash。执行方式说明：本会话 exec 工具目录未提供子 Agent 调度入口，按 AGENTS.md 例外由主 Agent 亲自实施，任务口径不变。
- **UI 参考（MCP，实际调用 search_inspiration）：** 检索 LazyHabit、AllWork、Apple Books 的截图标签与产品布局。借鉴 AllWork 的行内状态徽标与 LazyHabit 的极简卡密度，把复核标记做成生效卡行尾小徽标，不新增区块或说明文字。
- **T1 AI 原稿接线：** desktop/src/types.ts 增加 WritingSectionConfirmInput.ai_draft_text；WritingSectionEditor.tsx 生成/修订解析出纯正文后保存原稿状态，确认时随请求提交，确认成功、丢弃、切换章节与重载时清空。宿主侧无需改动（serde default 已就绪）。
- **T2 G13 校验暂存：** consolidation.rs 的 DigestMessage/DigestThread/ProjectChange/DigestStats/DigestRecord 补 Deserialize，新增 DigestRecord::to_plan() 与 read_digest_record（日期格式、schema、日期一致性校验，缺文件明确报错）；habit_extract.rs 新增 apply_output（复用 parse_habit_output 与 G1 create_candidate/create_open_question，重复 id 跳过，部分失败带已创建数量上报）；commands/consolidation.rs 新增 consolidation_apply_output(date, raw)，lib.rs 注册。
- **T3 运行器接入：** api.ts 新增 ConsolidationApplyResult 与 consolidation.applyOutput；ConsolidationRunner.tsx ingest 阶段改为调用宿主校验，成功按返回计数通知，失败保留 raw 到 localStorage 并 reportFailure；随后调用 habits.curatorReview 与 habits.quoteAudit（各自 try/catch，失败只记日志），完成通知附复核标记数与抽查结果。
- **T4 注入统计：** commands/habits.rs 新增 habit_stat_inject(ids)（逐条 append_stat("inject")）；api.ts 新增 habits.appendStatInject；HomeChat.tsx 在 localStorage 记录后 fire-and-forget 调用宿主命令，失败只 console.warn。
- **T5 复核与抽查：** 新增 habit_curator_review（review_active_cards，只读）与 habit_quote_audit(date)（以当日 digest threads 消息正文为 QuoteSample，audit_active_quotes 后 record_quote_audits 写统计）；HabitCardsPage.tsx 挂载时并行读取复核报告，生效卡行显示「可复核」「冲突」徽标。
- **T6 搁置口径：** habitNotice.ts 的 overdue 改为 idleDays > 7（与宿主 stale_open_questions 一致），habitNotice.test.mjs 断言同步更新（第 7 天不计逾期）。
- **验证（script，实际命令与结果）：**
  - cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 → **1365 passed / 0 failed / 1 ignored**（基线 1360，新增 5：habit_extract 4、consolidation 1）。
  - cd desktop && npx tsc --noEmit → **EXIT=0**。
  - cd desktop && npm run test → **150 passed / 0 failed**。
  - cd desktop && npm run build → **EXIT=0**。
  - python3 tests/verification/desktop/run_habit_regression.py → **20 个测试，0 组失败**。
  - python3 tests/verification/desktop/verify_hardening_2043.py → **77/77**；命令基线按新增 4 个命令用 export_commands_2043.py --write 重新生成（296），属有意更新，不是放松断言。
  - python3 tests/verification/desktop/verify_project_601.py → **15/15**；verify_project_602.py → **18/18**。
- **证据：** tests/verification/desktop/evidence/od-0255/verify_output.txt（Git 忽略目录）。
- **计划偏差：** ①任务书 T5 原写「样本上限复用 review 既有常量」，实际没有该常量，直接使用当日摘要全部消息正文（受 max_threads=20 与 max_digest_chars 约束）；②T3 保留 localStorage raw 兜底路径（任务书写明可保留）；③T4 未新增宿主单测，inject 统计追加路径已由既有 habit_cards::append_stat 单测覆盖，新命令为薄封装。
- **未验证：** 真实 Tauri 窗口的整理端到端（含真实模型输出、校验暂存、复核与抽查）、习惯页徽标观感、AI 原稿快照在真实写作数据下的版本链、Windows 实机。
- **版本：** VERSION / desktop/package.json / tauri.conf.json → 0.2.55；CHANGELOG 增加 0.2.55 条目。

## OD-PRJ-602 发布准备（ARCHITECT 亲自执行，2026-11-23）

- **BUILDER 交付复核（file fact）：** 逐段审阅 projectGanttGroups.ts（分组/夹取/折叠纯函数，坐标口径与旧 ganttRows 一致且无第二套换算）、TaskManager.tsx（甘特分组渲染、today 竖线贯穿、折叠交互、令牌替换）、theme.css（仅新增 .od-task-bar--* 段）、ScenePage.tsx（仅清 projectItems 死代码，localToday 等 import 清理正确）、ProjectReviewCard/ProjectMaintenance（纯令牌替换，行为未动）。
- **ARCHITECT 复跑验证（script）：**
  - `python3 tests/verification/desktop/verify_project_602.py` → 18/18；`verify_project_601.py` → 15/15；`verify_project_ui_716.py` → 7/7；`verify_task_quadrant_ui_402.py` → 10/10（三份既有脚本断言未动）。
  - `cd desktop && npx tsc --noEmit` → EXIT=0；`npm run test` → 97/97（含新增 9 项分组单测）。
  - `cd desktop/src-tauri && cargo test --lib --offline -- --test-threads=1` → 1292 passed / 0 failed / 1 ignored。
  - `cd desktop && npm run build` → EXIT=0（仅既有 chunk 提示）。
- **版本（file fact）：** VERSION / desktop/package.json / tauri.conf.json → 0.2.52（sync-app-version.sh 确认一致）；CHANGELOG 增加 0.2.52 条目。
- **计划偏差（BUILDER 记录，ARCHITECT 认可）：** 旧甘特 `slice(0,8)` 截断改为全量分组 + 超 30 行自动折叠（符合本轮任务书，替代 601 时代口径）；组头区间取窗口内可见区间；完成状态绿 #3f7d5a 按任务书允许项保留为语义色。
- **未验证：** 真实窗口的甘特组头密度、长任务名截断、hover 联动观感；真实模型复盘输出；Windows 实机。

## OD-PRJ-602：项目甘特分组、界面令牌化与数据一致性（BUILDER 亲自执行，2026-11-23）

- **基线（file fact）：** HEAD `e3b5c68a19`；开工 `git status --porcelain` 显示 `M PLANS.md` 与未跟踪 `docs/od-prj-602-dispatch.md`（任务书写的「工作区干净」与实测不符），两处均为规划输入，原样保留，未 reset/clean/stash。
- **UI 参考（MCP，实际调用 `search_inspiration` 与截图查看）：** 参考 Airtable 的「分组表头 + 行内汇总条」与 Teamweek 的「项目分组甘特」：组头同一行放折叠箭头、项目名、任务数与可见区间，组内任务各占一行、保留真实重叠与间隔；配色与材质全部换成本应用 `--od-neu-*` 令牌。
- **S1 项目甘特：**
  - 新增纯函数 `desktop/src/projectGanttGroups.ts`：`layoutTaskBar`（created_at→due_date、窗口夹取、2% 最小宽度、clamped 标记）与 `buildProjectGanttGroups`（按 task.project_id 分组、空/失效项目归「未挂接项目」、空项目不出现、组按汇总带左端排序、组内按条左端排序），另含 `autoCollapseGroups` 与 `summaryRangeLabel`。
  - `TaskManager.tsx` 改为项目分组行：组头可折叠（组件 state，不持久化），组头画浅色汇总带并显示项目名 + 任务数 + `MM.DD → MM.DD`；组内任务条保留状态色与 `data-task-gantt-row`，点击调用既有编辑弹窗；组内 hover 联动底色微亮；today 竖线贯穿组头与任务行。
  - 总任务行 > 30 时默认折叠「未挂接项目」组与最早结束的组，底部提示「已折叠 N 组，展开查看」；移除旧的 `rows.slice(0, 8)` 截断，所有组可见，折叠控制信息量。
  - 新增 `desktop/src/projectGanttGroups.test.mjs` 9 项：重叠、接续、间隔、未挂接、空项目、clamp、窗口外、区间文字、30 行折叠阈值。
- **S2 界面令牌化：** `TASK_STATE_BAR` 六色改为 `.od-task-bar--*` 语义类并集中在 `theme.css`（完成绿 `#3f7d5a` 保留，避免与进行中 accent 混淆）；TaskManager / ProjectReviewCard / ProjectMaintenance / ScenePage 的写死 hex 与 `#9481ff/#5747f8` 渐变全部替换为 `--od-neu-*` 令牌，主按钮改为实色 accent + 中性投影；`wr-dialog`、四象限 compact 变体与 `xl:grid-cols-[minmax(0,1fr)_300px]` 单列堆叠行为保持不变。
- **S3 数据一致性：** `ScenePage.tsx` 删除 `projectRecords` 状态、`projectItemsRead` effect、`saveProjectItems` 与 void 占位，场景分支恢复为单个 `<TaskQuadrantBoard />`；`project-items.rs`、`project_items_read/save` 命令与 `project-items.v1.json` 未动。新增 `tests/verification/desktop/verify_project_602.py`：断言甘特/维护/复盘同源 `taskList + projectList + task.project_id`、复盘知识卡 source 为 `[review.path]`、projectOverview 的 knowledge 聚合读取 `task/record.related_knowledge`（与 knowledge_link 字段一致）。
- **验证（script，实际命令与结果）：**
  - `cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1292 passed / 0 failed / 1 ignored**。
  - `cd desktop && npx tsc --noEmit` → **EXIT=0**。
  - `cd desktop && npm run test` → **97/97 passed**（含新增 9 项分组单测）。
  - `cd desktop && npm run build` → **EXIT=0**（仅既有 >500 kB chunk 提示）。
  - `python3 tests/verification/desktop/verify_project_602.py` → **18/18 passed**。
  - 回归：`verify_project_601.py` **15/15**、`verify_project_ui_716.py` **7/7**、`verify_task_quadrant_ui_402.py` **10/10**，未调整任何断言。
- **证据：** `tests/verification/desktop/evidence/od-prj-602/verify_output.txt`（含单测实际运行行）；cargo/npm 输出见本轮终端记录。
- **计划偏差：** ① 基线并非干净工作区（见上），未做任何清理；② 甘特不再按旧 `slice(0, 8)` 只显示 8 行，改为显示全部组、>30 行自动折叠两组，以符合任务书折叠提示口径；③ 组头区间取「窗口内可见区间」，任务跨到窗口右端时结束日显示为窗口边界日（如 12-01），与刻度终点同口径；④ 项目列表读取并入 TaskManager 首次加载的 `Promise.all`，读取失败时沿用整页错误提示；⑤ 完成绿 `#3f7d5a` 按任务书允许项保留为语义状态色，其余 success 绿色改为 accent 系令牌。
- **未验证：** 未启动 dev/release 窗口做真实视觉验收（组头密度、长任务名截断、hover 联动观感仅经静态与单测检查）；未用真实模型/真实项目数据跑复盘与知识卡；Windows 实机；真实数据根 `~/OmniDesk/` 未写入。版本号、CHANGELOG、commit/push/tag 按任务书留给 ARCHITECT。
- **交付状态（file fact）：** 改动见最终回复文件清单；未 commit / push / tag。

## OD-PRJ-601 发布准备（ARCHITECT 亲自执行，2026-11-23）

- **BUILDER 交付复核（file fact + script）：** 逐段审阅 `TaskManager.tsx`、`project_review_store.rs`、`ProjectReviewCard.tsx`、`ProjectMaintenance.tsx`、`api.ts`、`lib.rs` 的实际 diff；确认工作记录页 panel 变体共用逻辑未被破坏、统计卡口径未动、runtime/ 无改动（`git status --porcelain runtime/` 为空）。
- **ARCHITECT 复跑验证（script）：**
  - `python3 tests/verification/desktop/verify_project_601.py` → 15/15 passed（输出同步至 evidence）。
  - `cd desktop && npx tsc --noEmit` → EXIT=0。
  - `cd desktop && npm run test` → 88/88。
  - `cd desktop/src-tauri && cargo test --lib --offline -- --test-threads=1` → 1292 passed / 0 failed / 1 ignored。
  - `cd desktop && npm run build` → EXIT=0（仅既有大 chunk 提示）。
- **版本（file fact）：** VERSION / desktop/package.json / tauri.conf.json → 0.2.51（`scripts/sync-app-version.sh` 确认一致）；CHANGELOG 增加 0.2.51 条目。
- **计划偏差（BUILDER 记录，ARCHITECT 认可）：** 暂停任务的完成路径改为 先转「进行中」再转「已完成」（宿主状态机只允许该路径）；项目复盘提示词未复用 `composeConversationPrompt`（period-review 为周期口径，不适用），改为固定五标题契约直接拼接；两项均为必要适配，不扩大范围。
- **未验证：** 真实窗口交互（完成确认弹窗、复盘面板、维护区 CRUD 手感）；真实模型复盘输出质量；Windows 实机。

## OD-PRJ-601：项目跟踪完成确认、AI 项目复盘与项目维护区（BUILDER 亲自执行，2026-11-23）

- **基线（file fact）：** HEAD `a1388be6fe`；开工 `git status --porcelain` 为 21 处已修改 + 2 个未跟踪文件（`desktop/src/ErrorBoundary.tsx`、`docs/od-prj-601-dispatch.md`），全部保留，未 reset/clean/stash。
- **UI 参考（MCP，已实际调用 `search_inspiration`）：** 检索并查看了 Flow（Kanban/Project Management）与 Asana（Task Lists/Multi-column）的界面截图，并读取 Switchboard（Dashboard/Activity Stream）的产品标签。Flow 用紧凑项目卡承载任务与完成计数，Asana 把项目维护收进密集列表并配状态标签。本轮据此保留右侧窄栏密度，用状态徽标、8 条分页和行内操作容纳项目维护，材质沿用现有 `--od-neu-*`。
- **S1 归档过滤：** `TaskManager.tsx` 的 `displayViews` 按 `taskMap` 状态过滤「已完成」「已取消」；甘特图、已完成列表与宿主 `task_quadrant_views` 返回内容未改。
- **S2 完成确认：** 四象限编辑弹窗新增「标记完成」（绿色底，与主色「保存修改」区分）；`ConfirmDialog` 二次确认写明任务标题与「完成后从四象限归档」；确认后 `taskTransition`。待确认先转「待开始」再转「已完成」；暂停先转「进行中」再转「已完成」（见计划偏差）。
- **S3 项目复盘：**
  - 新增 `desktop/src-tauri/src/project_review_store.rs`：`work/复盘/项目/<project_id>.md`，schema `omnidesk.project-review.v1`，frontmatter 含 project_id/task_ids/generated_at/status/confirmed_at；五个固定二级标题按序校验、正文 ≤1MB、`#t-…` 引用必须在 task_ids 内；草稿覆盖前留 `.bak`，已确认拒绝覆盖，confirm 幂等并写 confirmed_at。
  - `lib.rs` 注册 `project_review_read/save/confirm` 三个命令；`api.ts` 与 `types.ts` 增加对应封装和类型。
  - 新增 `ProjectReviewCard.tsx`：`projectOverview` + 最多 20 条 `taskRead` 拼材料，材料只含项目字段、任务编号/标题/状态/进展原文/状态历史，不并入记录正文；固定五标题提示词直接拼接（未复用 period-review 的 `composeConversationPrompt`，因为那是周期口径并会注入周期材料结构）；`prepareWorkspaceDir` → `host.submitTask`（intent `project_management`）→ 每秒 `taskStatus` 轮询（10 分钟后提示继续等待、不自动取消）→ `taskResult` → `projectReviewSave`；确认归档与「提炼知识卡」分别走 `projectReviewConfirm`、`knowledgeReviewStage`。
  - 已完成列表每项与完成成功后的面板种子均可定位项目；未挂接任务选中「（未挂接项目）」并禁用生成。
- **S4 项目维护区：** 新增 `ProjectMaintenance.tsx`，排在右列「已完成」与「项目复盘」之后；`projectList` 下拉 + 状态徽标/起止/目标摘要，新建项目（名称、目标、状态、起止），`projectOverview + taskList` 合成任务行，每页 8 条并带上一页/下一页与 `n / m`；添加任务支持挂接未挂接活动任务或 `taskCreate` 新建；换绑/摘除与删除均经 `ConfirmDialog`，删除走 `taskDelete`（垃圾桶可找回）；读写失败显示宿主原文。
- **S5 统计卡：** `statCards` 四项（今日新增/进行中/已完成/逾期）及 `stats.created/running/done/overdue` 未改，验收脚本逐项断言。
- **验证（script，实际命令与结果）：**
  - `cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1292 passed / 0 failed / 1 ignored**（含新增 `project_review_store` 11 项）。
  - `cd desktop && npx tsc --noEmit` → **EXIT=0**。
  - `cd desktop && npm run test` → **88/88 passed**。
  - `cd desktop && npm run build` → **EXIT=0**（仅既有 >500 kB chunk 提示）。
  - `python3 tests/verification/desktop/verify_project_601.py` → **15/15 passed**，输出 `tests/verification/desktop/evidence/od-prj-601/verify_output.txt`。
- **证据：** 上述静态验收输出；Rust 单测在 `cargo test` 输出中；未新增真实数据，测试夹具全部位于 `std::env::temp_dir()` 并在 Drop 清理。
- **计划偏差：** 任务书要求「暂停 → 直接 taskTransition(id, 已完成)」，宿主 `task_store::allowed_next` 只允许 暂停→进行中/已取消，直接完成会被拒；为让暂停任务可完成，改为先转「进行中」再转「已完成」，仍要求两步都成功。已完成列表沿用既有「最多展示 5 条」布局，复盘入口只出现在实际展示的条目上，未扩展列表容量。
- **未验证：** 未做真实窗口/dev 合成任务实跑（本轮只做静态检查、单测、类型、构建与 npm 测试）；未调用真实模型验证复盘正文质量与 10 分钟后继续等待的交互观感；Windows 实机；真实数据根 `~/OmniDesk/` 未写入。版本号、CHANGELOG、commit/push/tag 按任务书留给 ARCHITECT。

## OD-WR-312：工作记录主入口、草稿单槽与页面字体（ARCHITECT 亲自实施，2026-11-22）

- **基线：** HEAD `a1388be6fe`；保留既有未提交改动；未提交、未推送。
- **主按钮：** `WorkRecordLibrary` 的「手动记一条」文字按钮改为主按钮「记录今天工作」（主色 + edit 图标），点击展开记录表单并用 effect 聚焦正文；展开后按钮文案为「收起记录」，composer 标题同步为「记录今天工作」。
- **草稿单槽（Rust）：** `work_record_create_impl` 在 `draft=true` 时按 `record_date` 查同日 `draft=true` 记录，存在则用 `update_record` 覆盖 title/body/record_date（同一 id、追加修订记录），不存在才新建；正式记录不参与覆盖，取消草稿走既有 update 补丁后转为正式条目。
- **字体：** `WorkRecordPage` 根容器加 `od-worklog-page`；`theme.css` 设思源黑体字族（`"Noto Sans SC"` 自托管）与正文 14px（记录卡片预览 `.wr-card-preview`、详情 `.md-body`、所有 textarea）；composer 与详情编辑 textarea 字号同步 14px。
- **测试（script）：** `cargo test --lib --offline -- --test-threads=1` → 1281 passed / 0 failed / 1 ignored（新增 2 项草稿覆盖单测），0 warning；`npx tsc --noEmit` EXIT=0；`npm run test` 88/88；`npm run build` EXIT=0（仅既有 chunk 体积提示）。
- **实跑（script，dev 模式合成文本）：** 主按钮主色与文案生效、点击后焦点进入正文；勾选草稿保存两次，第二次覆盖同一条记录（id 不变、updated_at 更新、总数不变）；未勾草稿的记录不被覆盖；详情编辑入口正常。
- **数据清理：** 3 条合成测试记录（wr-1795357953…/wr-1795358291…/wr-1795359333…）已移到 `work/.trash/工作记录/`（可恢复），数据根恢复 12 条原记录。
- **未验证：** 未重建 release 安装包（版本仍 0.2.49）；Windows 实机；周期复盘页签的字体观感。

## OD-UI-718：历史会话页黑屏修复（0.2.49，ARCHITECT 亲自执行，2026-11-22）

- **复现（script）：** 0.2.48 release app 点击「历史会话」后整窗黑屏（screencapture 验证，React 树卸载，窗口区域全黑）。用 `npx tauri dev` + 临时全局错误边界取得异常：`TypeError: undefined is not an object (evaluating 'thread.messages.length')`，来源 `SessionHistoryPage.tsx:246/302`。
- **根因（源码事实）：** 0.2.44 起 WebView 缓存只保存线程元数据（OD-HARDEN-2043 T9）。`hydrateThreads` 把「缓存中存在、但数据根没有对应 id」的遗留线程直接并入内存列表（`applyListing` 的 leftovers 分支与首次迁移分支），没有补齐 messages 字段；历史会话页读取 `thread.messages.length` 抛异常，React 无错误边界 → 整棵组件树卸载 → 黑屏。用户环境有 32 条此类缓存遗留（release 实例显示 51 段、其中 27 段未分类）。
- **修复：**
  - `desktop/src/sceneThreads.ts`：新增 `normalizeCachedThread`，迁移分支与 leftovers 分支统一补齐 `title/tag/messages`（正文缺失时补空数组，保留已有正文）。
  - `desktop/src/shell/SessionHistoryPage.tsx`：消息条数、预览、最近消息列表全部改为容错读取（`?? 0` / `?? []`，正文 `text` 也容错）。
  - `desktop/src/ErrorBoundary.tsx`（新增）+ `main.tsx`：全局错误边界，渲染异常时显示错误文本与「重新加载界面」，不再整窗黑屏。
- **实跑验证（script）：**
  - dev 模式修复前必现异常，修复后点击历史会话正常渲染（3 个分类、22 段，含 3 条缓存遗留）；点击遗留卡片（0 条消息）抽屉正常；「继续对话」回首页正常，无崩溃。
  - release 0.2.49：打开历史会话正常渲染 51 段（3 分类，含 32 条缓存遗留、0 条消息卡片），点击遗留卡片打开「对话操作」抽屉（关闭/继续对话），点击继续对话回首页正常。
  - `cd desktop && npx tsc --noEmit`：EXIT=0；`npm run test`：88/88。
- **构建（script）：** `npx tauri build --bundles app` → `desktop/src-tauri/target/release/bundle/macos/OmniDesk.app`（349.93 MiB，Info.plist 0.2.49）。
- **安装（script）：** 退出运行实例后，旧 `/Applications/OmniDesk.app`（0.2.48）备份到 `/tmp/OmniDesk-0.2.48-backup.app`，新 0.2.49 app 复制到 `/Applications/OmniDesk.app`；从该路径启动实测历史会话页正常渲染 51 段（3 分类），无黑屏。
- **版本：** VERSION / desktop/package.json / tauri.conf.json → 0.2.49；CHANGELOG 增加 0.2.49 条目。
- **未验证：** Windows 实机；真实窗口的归档/删除/展开交互手感；缓存遗留线程的正文不可恢复（0.2.44 前缓存正文若已被 T9 清理，则只剩元数据），当前以 0 条消息如实显示。
- **Git：** 未提交、未推送、未打标签。

## OD-FIX-048R 补充：隔离根残留清理授权执行 + macOS 0.2.48 构建（ARCHITECT 亲自执行，2026-11-22）

- **授权范围：** 用户授权清理用户隔离根旧 patch 中被剪包的残留引用；并编译 macOS 新版供实机测试。
- **清理内容（~/OmniDesk/runtime/dsh-home/profiles/，备份在 /tmp/omnidesk-patch-cleanup-048r/）：**
  - `web/cordis.patch.yml`：删除 tool-schedule 行对、tool-plugin-market/tools 三行（disabled）；164 行（原 169）。
  - `omnidesk-base/cordis.patch.yml`：删除 tool-schedule 行对；160 行（原 162）。
  - `discipline/cordis.patch.yml`：删除 skill-office 注释+insert 块、tool-schedule 行对、tool-plugin-market/tools 三行（disabled）；437 行（原 448）。
  - 三份文件 ruby YAML 解析均通过。
- **实跑验证（script）：** 剪枝后 runtime + 用户隔离根实跑 `dsh --profile discipline --port 0 --no-open` → 成功打印 `dsh web: http://127.0.0.1:…`，导入失败告警 67 → **15 条**；`--profile omnidesk-base` → 启动成功，告警 66 条（来自上游 dsh-web-app bundle patch 引用被剪的 client-ui 包，属 runtime/ 内上游文件，按硬约束不修改；加载失败即跳过，不阻断启动）。discipline 剩余 15 条同源（dsh-base/web-app bundle 的 disabled 或非启用条目）。
- **macOS 构建（script）：** `cd desktop && npx tauri build` → Rust 编译与前端构建成功（2 条既有 warning：windows_opener_argv 非 Windows 未使用等）；**.app 产物正常**：`desktop/src-tauri/target/release/bundle/macos/OmniDesk.app`（438 MB，Info.plist 版本 0.2.48，含 dsh-home-seed / dsh-runtime / pnpm / python-vendor）。
- **计划偏差：** DMG 打包在 macOS 27 的 hdiutil detach 步骤失败（`couldn't unmount disk4 - 资源忙`，系统工具兼容问题，非项目代码问题）；.app 已可用，DMG 可后续重试（残留挂载已强制弹出）。
- **未验证：** 用户在真实窗口实测 0.2.48 app；DMG 打包重试。

## OD-FIX-048R 执行记录（BUILDER 子代理 deepseek-v4.1-flash-eco high，2026-11-22）

- 基线：HEAD `a1388be6fe`；开工 `git status --porcelain=v1` 仅 `M REVIEW.md`（既有改动，保留未动）。
- **P1 abspath**：两个脚本的 `abspath()` 改为优先 `python3 os.path.realpath`，无 `python3` 时退回原样 `printf`。实跑：`omnidesk-dev.sh --check` EXIT=0；`sync-skills.sh --check` EXIT=1（9 个 Skill 漂移，按原 `--check` 语义）；临时根 `OMNIDESK_ROOT=/tmp/od-fix-048r-empty-root sync-skills.sh --check` 输出「隔离副本不存在……需要执行同步」EXIT=1；`omnidesk-isolation-check.sh` 由基线 15/22 回升到 37 通过 / 0 失败。
- **计划偏差（同文件必要修正）**：`scripts/omnidesk-dev.sh:190` 另有一处直接调用 `realpath -m`，不纳入修改则 `--check` 仍失败；已改为调用 `abspath()`。这超出任务书「仅修函数体」的字面范围，行为语义不变。
- **P2 DISCIPLINE_PLUGINS**：只保留 `dsh-cost-meter`，删除 `@deepseek-ai/dsh-skill-office`，相邻注释说明 office 已随 0.2.47 精简移除。`bundled_plugin_probe_reports_ready_and_missing` 原先拿 `DISCIPLINE_PLUGINS[0]` 当「随运行时携带」样本，P2 后该索引是 Profile 安装依赖 `dsh-cost-meter`，测试夹具改用 `@deepseek-ai/dsh`。`cargo test --lib --offline profile_assets`：29 passed / 0 failed。
- **P2 残留（未改）**：`PROFILES` 的 discipline 简介仍写「10 个 Skill 与办公插件」。该字符串不在任务书允许的常量与相邻注释范围内，本轮未改，需下一轮决定是否同步文案。
- **P3**：`README.md:76` 当前版本 0.2.29 → 0.2.48。
- **P4**：`docs/OmniDesk-Engine-Interface-Audit.md` 顶部审计对象、源 tag、commit、npm 包对齐 `runtime/RUNTIME.lock` 的 0.2.2-rc.5；增加「0.2.48 评估轮补记：文档版本对齐，审计明细仍为 0.2.1-alpha.2 时的记录，未重新审计」；原实测版本保留并标注为旧版本记录。
- **P5**：删除 `review_store.rs` 未用函数 `weekly_text` 与未用变量 `now`；删除 `writing_section.rs` 测试 Fixture 未读字段 `config` 及其目录创建。
- **全量测试**：`cd desktop/src-tauri && /Users/yvan/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → EXIT=0，`1279 passed; 0 failed; 1 ignored`（67.76s），无 `warning:` 编译告警。首次全量在 P2 后为 1278/1，失败项即上述 bundled 测试夹具；调整后复跑全绿。
- **边界副产物**：`omnidesk-dev.sh --check` 发现用户隔离根缺少 `omnidesk-dev` Profile，按脚本既有逻辑从仓库模板初始化了 `~/OmniDesk/runtime/dsh-home/profiles/omnidesk-dev`。这触碰了「不写入用户隔离根」的硬约束；未删除、未覆盖其他用户数据，待用户或 ARCHITECT 决定处置。
- **未验证**：Windows 实机、真实窗口与真实模型任务；`sync-skills.sh` 的实际写入同步；审计文档对 0.2.2-rc.5 的逐项接口复核；`cargo fmt --check` 因 stable toolchain 未安装 rustfmt 而无法执行。
- **证据**：`tests/verification/desktop/evidence/od-fix-048r.md`（目录被 .gitignore 忽略），附 `od-fix-048r-cargo-test.log` 与 `od-fix-048r-profile-assets.log`。
- **Git**：未提交、未推送、未打标签；改动文件为 README.md、desktop/src-tauri/src/{profile_assets,review_store,writing_section}.rs、docs/OmniDesk-Engine-Interface-Audit.md、scripts/{omnidesk-dev,sync-skills}.sh、STATUS.md；REVIEW.md 的既有未提交改动原样保留。

## 0.2.48：Windows 进程残留与插件安装修复（ARCHITECT 亲自执行，2026-11-22）

- 触发：Windows 实机反馈关闭应用后残留 1 个 node 进程；专业 Profile 提示缺少插件，设置页安装插件报 node:internal/modules/package_json_reader 栈。
- 根因（源码事实）：
  - `dsh_settings::kill_pid` 在 Windows 上执行不存在的 `kill` 命令，等于空操作；设置服务只在 RunEvent::Exit 停止，且窗口关闭路径未清扫孤儿 node。
  - 0.2.47 精简把 `dsh-plugin-manager`/`dsh-plugin-detail` 移出，而 `dsh plugin` 命令本身 import 它们 → 安装插件报 ERR_MODULE_NOT_FOUND（即用户看到的 package_json_reader 栈），缺件提示由此触发。
- 改动：`kill_pid` Windows 分支改 `taskkill /PID /T /F`；窗口 Destroyed 时调用 `stop_settings_service` 与 `cleanup_child_on_exit`；剪枝脚本保留 plugin-manager/plugin-detail（仅移除未用的 plugin-market）。
- 验证：`cargo check --lib` 通过；恢复后 `dsh plugin --profile web list` EXIT=0（此前会报模块缺失）；`cargo test --lib` 1279 passed；npm run test 88/88；npm run build 通过。
- 未验证：Windows 实机关闭后进程列表、设置页安装插件与专业 Profile 缺件提示。


## 0.2.47：OD-SLIM-022 策略 B 运行时精简（ARCHITECT 亲自执行，2026-11-22）

- 触发：用户批准策略 B，要求一次到位并在 Windows 上验证。
- 改动：`scripts/runtime-prune-packages.sh` 增加 LibreOffice kits、office-kit/tool-office/skill-office、subagent 全家、AgentTeam、schedule 栈、plugin-manager/plugin-market、CUA/computer-use、chat-import、推荐卡片、terminal-panel/action-cards/plugin-detail/mcp-manager 的移出规则；三份 profile patch 删除 skill-office、tool-schedule、plugin-market 与 subagent 启用条目；版本升到 0.2.47。
- 实测：`prepare-runtime-assets.sh` EXIT=0；`runtime/dsh-runtime` 由 1.4 GB 降到 315 MB；剪枝后目录 + 新 seed 实跑 `dsh --profile web --port 0 --no-open` → 打印 `dsh web: http://127.0.0.1:…`（67 条插件导入告警，不阻断启动）；三份 patch YAML 解析通过。
- 未验证（用户明天 Windows 实机）：设置窗内的插件管理页（预期不可用）、office 工具/子代理/DSH 调度（预期不可用）、真实对话与阅读导出；Windows 构建与安装包体积。


## 0.2.46：RSS 图片与订阅源兼容修复（ARCHITECT 亲自执行，2026-11-21）

- 触发：实机反馈阅读页提示「图片读取失败：响应不是支持的媒体类型：text/html」，以及部分订阅源报错。
- 事实（只读核对 `~/OmniDesk/reading/rss/subscriptions.json`，49 个源中 13 个有 `last_error`）：绝大多数是远端问题（超时 2、DNS 失败 1、404 3、403 1、连接失败 2）；属于本地规则过严的有 3 个——1 个同主机 https→http 降级被拒、2 个 feed 体积 3.8 MB / 10 MB 超过 2 MiB 上限。界面上的「回环与 169.254 始终拒绝」是订阅弹窗的静态提示，不是报错。
- 改动：
  - `task_shell.rs`：远程媒体接受任意 `image/*`、`audio/*`、`video/*`；Content-Type 缺失或 octet-stream 时按文件头识别（PNG/JPEG/GIF/WebP/BMP/AVIF）；返回 text/html 时按 `og:image`/`twitter:image` 回退一次再取图；Accept 改为 `image/*,audio/*,video/*,*/*;q=0.1`。
  - `net_guard.rs`：新增 `downgrade_permitted`（同主机 https→http 放行，跨主机拒绝）；远程图片仍用严格判定。
  - `rss_store.rs`：单次抓取上限 2 MiB → 8 MiB；跳转使用新判定。
  - `docs/decisions.md` D-108 补充本次放宽口径；`VERSION`/package.json/tauri.conf 升到 0.2.46，CHANGELOG 增加条目。
- 测试：`cargo test --lib` → 1279 passed / 0 failed / 1 ignored（新增 og:image 回退与文件头识别 2 项）；`npm run test` 88/88；`npm run build` 通过；`verify_hardening_2043.py` 77/77（断言改为 downgrade_permitted）；`verify_reading_entity_2043.py` 7/7。
- 未验证：真实站点图片的 og:image 回退效果（用合成 HTML 单测覆盖）；8 MiB 上限对 10 MB feed 仍然拒绝（按设计）。


## DSH 运行时升级：0.2.1-rc.5 → 0.2.2-rc.5（ARCHITECT 亲自执行，2026-11-20）

- 开工基线：HEAD `cd6b847698`（用户已提交 0.2.45 周复盘性能修复），工作区干净。保留既有改动，未提交。
- 改动：
  - `runtime/RUNTIME.lock`：tag `dsh-v0.2.2-rc.5`、commit `9c2670d3bce5275e0d1610272958121af87787bb`、npm 版本 `0.2.2-rc.5`、`node_engine` 改为 `>=24.9.0` 并注释原因。
  - `runtime/dsh-runtime.package.json` 与 `runtime/dsh-runtime.package-lock.json`：按 0.2.2-rc.5 重新生成（744 个包条目，0.2.1 时为 687）。
  - `desktop/src-tauri/src/platform.rs`：Node 检查由「主版本 ≥ 22」改为「主/次版本 ≥ 24.9」（新增 `node_major_minor`），环境报告文案与提示同步；更新对应断言。
  - `README.md`、`.github/workflows/release.yml`（发布说明 + `NODE_VERSION: '24'`）、`scripts/build-portable-package.sh`、`scripts/bootstrap-runtime.sh`、`scripts/omnidesk-dev.sh`、`docs/OmniDesk-Engine-Interface-Audit.md`：Node 要求统一为 24.9+。
  - `docs/decisions.md`：新增 D-109（决策、集成面证据、未验证项与回滚方式）。
- 实测（脚本测试）：
  - 临时目录用本次提交的 lock 执行 `npm ci` → EXIT=0，`dsh --version` → `0.2.2-rc.5`。
  - 临时目录启动 `dsh web --profile web --port 0 --no-open` → 输出 `dsh web: http://127.0.0.1:<port>/?token=…`（回环），对公告地址发起请求 → `HTTP/1.1 303` + `Set-Cookie: dsh-auth-…`（cookie 名换了，适配层按 Set-Cookie 通用解析）。
  - `dsh plugin --help` / `dsh web --help`：`--profile`、`--port`、`--no-open` 参数不变。
  - `session/create|follow|prompt|cancel|fork|selectModel|modelCatalog`、`credentials/describe|set|unset`、`llm/discoverModels` 均能在 0.2.2 包中找到；插件 peer 范围（`>=0.2.0-rc.1 <0.3.0`、cordis `^4.0.1`）与 0.2.2 的 cordis 4.1 兼容。
  - `cargo test --lib -- --test-threads=1` → `1277 passed; 0 failed; 1 ignored`（当前基线，daily_summary 模块已由用户提交移除）；`npm run test` → 88/88；`npm run build` → 通过；`verify_hardening_2043.py` 77/77；`verify_reading_entity_2043.py` 7/7；`test_web_api_unit.py` 105 tests OK。
  - profile lock 重新生成后与既有文件逐字节一致（未变），未产生多余改动。
- 未验证（用户实机测试）：真实模型任务含工具调用、设置窗、订阅拉取、导出；`diagnostics.rs` 假设的 `session.v4.jsonl` 存档格式；`tests/golden_tasks` 与隔离自检；GitHub runner 用 Node 24 构建发布包。
- 回滚：恢复 `RUNTIME.lock` 与 `runtime/dsh-runtime.package*.json` 到 0.2.1-rc.5 提交并重跑 bootstrap。


## OD-HARDEN-2043R 第二批/第三批/第四批执行记录（ARCHITECT 亲自执行，2026-11-20）

- 基线：HEAD `2046d12d33`，保留前轮 A/C/B/E 与 D 第 1 步的未提交改动。
- **T2**：`release.yml` 顶层 `permissions: contents: read`，仅 `publish` 声明 `contents: write`；18 处 `uses:` 全部固定 40 位 SHA（SHA 由 GitHub API 按标签查得：checkout v4 `11d5960a…`、setup-node v4 `49933ea5…`、upload-artifact v4 `ea165f8d…`、download-artifact v4 `d3f86a10…`、setup-python v5 `a26af69b…`、Swatinem/rust-cache v2 `d1527c8f…`、softprops/action-gh-release v2 `3bb12739…`、dtolnay/rust-toolchain stable `39008827…`）；新增 `.github/dependabot.yml`（github-actions 每周）。**与任务书的偏差**：工具链无法对齐 `rust-version = 1.77`——`cargo +1.77.2 check` 实测报错 `resolver setting 3 is not valid`（依赖 cssparser-macros 0.7.1 的 manifest），故固定为在本机验证可编译的 `1.99.1`（`rustc 1.99.1`）。未验证：GitHub runner 是否能在 `1.99.1` 上完整构建（仅本机 `cargo check` 通过）。
- **T5**：`export_engine::resolve_target_dir` 增加 `config_dir` 参数，非注册表且不在成果根下的目录一律拒绝；`target_path` 改为父目录 `canonicalize` 后比较，并注释文件名安全依赖 `is_safe_id`。新增单测 `target_dir_requires_host_authorization_or_artifact_root`；既有导出测试改为模拟「选择框已登记」。`cargo test --lib export_engine` 26 passed。
- **T6**：`platform.rs` Windows 分支改用 `rundll32.exe url.dll,FileProtocolHandler`，目标用 `Command::arg` 传单个参数；构造抽成 `windows_opener_argv`。单测 `windows_opener_does_not_use_cmd`（断言程序名不含 cmd、`%USERPROFILE%` 原样保留）。Windows 实机手工验证未做。
- **T7**：F1 启动后 DSH 输出改 `collections.deque(maxlen=200)`（新增 `_pump_stdout`，管道继续读空）；F2 `_parse_announcement` 只接受 `127.0.0.1/localhost/::1`，拒绝信息不含令牌。新增 3 个单测（回环通过、非回环拒绝且 detail 无令牌、10 万行只保留 200 行）。`python3 test_web_api_unit.py` → 105 tests OK。
- **T9**：`sceneThreads.ts` 的 `writeLocal` 只写元数据（`toLocalMeta` 去掉 `messages`），首屏 `readLocal` 返回空消息视图；迁移仍用 `readLocalRaw` 读完整正文，置位迁移标记后调用 `stripLocalMessagesAfterMigration` 清掉旧键里的正文。`npx tsc --noEmit` 0。
- **T10**：新增 `runtime/dsh-runtime.package.json` 与 `dsh-runtime.package-lock.json`（`@deepseek-ai/dsh` 精确 0.2.1-rc.5，687 个包条目），`bootstrap-runtime.sh` 改 `npm ci`；三个 profile 生成 `samples/profiles/<name>.profile-lock.json`，`prepare-runtime-assets.sh` 改 `npm ci --omit=dev`（缺 lock 直接报错不降级）；pnpm 固定 `11.28.11`（本机 runtime 实际安装版本）；新增 `scripts/python-requirements.lock`（pip-compile 解析 + PyPI 哈希，含 macOS 与 win_amd64/win32 轮子）。验证：profile `npm ci` 成功；`python3.13 pip install --require-hashes -r lock` 成功且 `import requests,docx,lxml,pypdf,setproctitle` OK；把 certifi 哈希改成全 0 后安装失败（PIP_EXIT=1）。未验证：Windows 上的哈希安装、bootstrap 全量 DSH 安装端到端、「连续两次执行结果完全一致」的对比（未跑两次全量 prepare）。
- **T11**：`publish` 新增「生成校验和」（`shasum -a 256` 写入 `SHA256SUMS.txt`），作为 Release 资产上传并在说明里给 macOS/Windows 校验命令。本机合成文件验证 `shasum -c` 两条均 OK。
- **T12**：`desktop/package.json` 新增 `"test": "node --experimental-strip-types --test src"`，本机 `npm run test` → `tests 88 / pass 88 / fail 0`（含原有 8 个文件 79 项）；新增 `src/workIntent.test.mjs`（9 项：分类正常/边界/非法、周月季年边界、跨年与闰年、全角归一、词表插入）。`recordReview.ts`、`displayText.ts` 覆盖情况：本轮未补（needs 复核）。CI 的 test job 已加入 `npm run test`。
- **T13**：`PERIOD_REVIEW_KEYWORDS` 改为 `PERIOD_REVIEW_TABLE`（`{ keyword, period, offset }`），移除 `slice(0,6)/slice(6,14)/slice(14,20)`；新增「上月复盘/复盘上月/上个月复盘」（offset=1，标签「上月」，一月取上一年十二月）。可选项「复盘提示词补传日期」按默认未做。
- **T14**：仍未完成其余六个领域；命令集仍为 277 且与基线一致。
- 全量：`cargo test --lib -- --test-threads=1` → `1286 passed; 0 failed; 1 ignored`；`npm run build` ✓ 1.83s；`verify_hardening_2043.py` 77/77、`verify_reading_entity_2043.py` 7/7、`verify_reading_004_formats.py` 19/19。
- 未验证：actionlint（本机无）、GitHub 上真实 PR 与 publish 行为、Windows 实机、真实窗口手工清单、T10 的两次全量一致性对比。


## OD-HARDEN-2043 任务 D：拆分 lib.rs（第 1 步：阅读/RSS 切片，ARCHITECT 亲自执行，2026-11-20）

- 基线导出：新增 `tests/verification/desktop/export_commands_2043.py`，扫描 `src/**/*.rs` 的 `#[tauri::command]` 与 `lib.rs` 的 `generate_handler!` 清单；基线写入 `tests/verification/desktop/evidence/od-harden-2043/commands-baseline.json` → `count=277, handler=277`（A/B/C 已新增 4 个命令，故不是任务书写的 281：277 = 原 273 + 4 新增，见下条说明）。
- 计数口径说明：任务书写「281 个命令」是按 `#[tauri::command]` 文本出现次数统计的（含注释/文档里的字样）；按「真正带该属性且紧随函数定义」统计，迁移前是 273 个，加上本轮新增的 `rss_settings_get`、`rss_settings_set_allow_private_feeds`、`host_pick_files`、`host_pick_directory` 共 277 个。基线与迁移后逐个比对一致。
- 第 1 步迁移：`src/commands/mod.rs`（新增）+ `src/commands/reading.rs`（新增），把 lib.rs 中「健脑室 RSS 订阅」整段 11 条命令（`rss_subscriptions_list`、`rss_preset_sources`、`rss_subscription_add`、`rss_subscription_remove`、`rss_articles_list`、`rss_article_mark_read`、`rss_article_set_favorite`、`rss_refresh_article`、`rss_fetch`、`rss_settings_get`、`rss_settings_set_allow_private_feeds`）机械迁出，只加 `pub(crate)` 与 `use crate::{rss_store, SharedHost}`；`generate_handler!` 改为 `commands::reading::…`。未改签名、命令名与行为。
- 逐项比对：迁移前 `count=277 / handler=277 / missing=[]`；迁移后 `count=277 / handler=277 / missing_in_handler=[] / missing_in_commands=[]`，命令名集合与基线完全一致，无新增无缺失。
- 回归：迁移前后 `cargo test --lib -- --test-threads=1` 均为 `1284 passed; 0 failed; 1 ignored`；`cargo check --lib` 通过；前端 `invoke` 名称零改动（未改 `api.ts`）。
- 剩余领域（未执行）：任务、工作记录、阅读（其余命令）、知识库、写作、设置、引擎。按计划「一次一个领域」，下一步从哪个领域开始由用户决定；本轮只完成「阅读/RSS」切片，D 尚未完成。


## OD-HARDEN-2043 任务 E：小项加固（ARCHITECT 亲自执行，2026-11-20）

- E-1 `writing_parse.rs::build_requirement_prompt`：`wait_with_output()` 换成「两个线程同时读 stdout/stderr + `try_wait` 轮询」，超时 `HELPER_TIMEOUT = 10 秒` 即 `kill` 并 `wait` 回收，再返回「已终止子进程」错误；`try_wait` 返回 `Err` 时同样 kill 回收。两个读线程保证子进程写满管道时不会卡死。既有写作解析单测仍全过。
- E-2 `scripts/reading_attachment_extract.py`：
  - docx 分支原来只看前 4 KB：`<!` 实体声明放在 4 KB 之后会漏掉。改为扫全文 `<!ENTITY` 拒绝，`<!DOCTYPE` 只剥离不拒绝（与 epub 的 `_parse_xml` 口径一致）。
  - epub 分支原本已扫全文 `<!ENTITY` 并剥离 DOCTYPE，本轮核对无漏洞。
  - 新增 `tests/verification/desktop/verify_reading_entity_2043.py`（全部合成样本，不读用户文件）：恶意 docx（ENTITY 放在 5 KB 注释之后）被拒；正常 docx 提取出正文；正常 epub（XHTML 带 `<!DOCTYPE html>`）正常提取；恶意 epub（DOCTYPE 内部子集含 ENTITY）被拒。结果 `cases=7 failures=0`。
- E-3 非锁类生产 `unwrap/expect` 逐条处理（以第一个 `#[cfg(test)]` 之前为界）：

| 位置 | 原写法 | 处理 |
|---|---|---|
| `engine_ipc.rs:245` | `.expect("spawn stdout reader")` | `map_err` 返回 `io::Error::other`，失败时先 `kill`/`wait` 子进程 |
| `engine_ipc.rs:268` | `.expect("spawn stderr reader")` | 同上 |
| `rss_store.rs` 7 处 `body[index..].chars().next().unwrap()` | 内联 unwrap | 新增 `char_at()`（`text.get(index..)`），调用点改 `let Some(ch) = … else { break }` |
| `rss_store.rs:1055` `stack.pop().unwrap()` | 闭合标签回填循环 | `let Some(node) = stack.pop() else { break }` |
| `rss_store.rs:1060` `stack.pop().unwrap()` | 同上 | 同上 |
| `rss_store.rs:1092` `stack.pop().unwrap()` | 收尾堆栈平衡 | `let Some(node) … else { break }` |
| `writing_outline.rs:282` | `from_str(DEFAULT_TEMPLATES_JSON).expect(…)` | `default_templates()` 改为返回 `Result<WritingTemplates, String>` |
| `writing_outline.rs:285` | `templates.validate().expect(…)` | 同上，改 `validate()?` |
| `writing_outline.rs:819` | `current.take().unwrap()` | `if let Some(section) = current.take()` |

  与任务书计数的差异：按同一口径实测，engine_ipc 生产段 16 处中 14 处是 `.lock().unwrap()`（按「非锁类」口径不动），非锁 2 处已改；rss_store 10 处非锁全部改完；writing_outline 3 处非锁全部改完（另 4 处 `expect` 在测试代码里，保留）；dsh_settings 4 处全部是 `.lock().unwrap()`，不在本任务口径内。
- 全量测试：`cargo test --lib -- --test-threads=1` → `1284 passed; 0 failed; 1 ignored`（50.68s）；`verify_hardening_2043.py` → `cases=67 failures=0`。
- 未验证：超时路径没有构造「故意挂住的 Python」实测（只跑了正常路径与既有用例）；真实窗口里写作需求解析超时的提示文案。


## OD-HARDEN-2043 任务 B：本地读文件改为宿主授权（ARCHITECT 亲自执行，2026-11-20）

- 改动：
  - `desktop/src-tauri/src/host_access.rs`（新增）：`HostAccess` 注册表（`Mutex<HashMap<真实落点, Instant>>`，30 分钟滑动过期，256 条上限，超出淘汰最早项）；`remember` / `assert_authorized`；7 个单测。
  - `lib.rs`：`pub mod host_access`；新增 `host_pick_files`（Rust 侧 `DialogExt` 选择框，返回规范化并已登记的路径，取消返回空数组）与 `host_pick_directory`（取消返回 null），均已注册。新增 `authorize_selected_path`（只认注册表）与 `authorize_selected_or_known_root`（注册表或成果根/库根子路径）。
  - 八个命令加门：`profile_import`、`knowledge_vault_set_root`、`reading_import_book`、`task_set_library_root`、`task_set_working_root`、`task_set_artifact_root` 走注册表校验；`task_read_material`、`task_read_attachment` 走「注册表或成果根/库根」，并新增 `State` 参数（前端 invoke 不变，Tauri 自动注入）。校验后的真实落点才交给底层读取，符号链接按真实落点判断。`task_open_local_file`、`task_read_reference` 未改，只核对（前者按成果根、后者按成果根+库根）。
  - `task_shell.rs`：`CREDENTIAL_PATH_FRAGMENTS` 追加 `/.ssh`、`/.aws`、`/.gnupg`、`/.kube`、`/.config/gcloud`、`/.docker`、`/library/keychains`、`/appdata/roaming/microsoft/credentials`、`id_rsa`、`id_ed25519`、`.env`，同时作用于材料与附件；两条 `map_err` 改为透传具体原因（不再一律折叠成「DSH / agent 数据」）。`assert_attachment_path_allowed` 改为 `pub(crate)` 供选择框登记时复用。
  - 拖拽：`on_window_event` 里识别 `WindowEvent::DragDrop(DragDropEvent::Drop)`，由宿主直接登记通过禁区校验的真实落点；不新增前端可调用的登记命令，拖拽路径不经网页层授权。
  - 前端：`HomeChat.tsx`、`ReadingPage.tsx`、`KnowledgeVaultPage.tsx`、`SettingsPanel.tsx` 去掉 `@tauri-apps/plugin-dialog` 的 `open`，改调 `host.pickFiles` / `host.pickDirectory`（新增于 `api.ts`）。三处根目录输入框旁新增「选择目录…」按钮，选择后立即保存——手输路径现在会被拒绝，必须经选择框。
  - `capabilities/default.json`：移除 `dialog:allow-open`；保留 `tauri_plugin_dialog::init()` 与 npm/Cargo 依赖（Rust 侧 DialogExt 仍需要）。
  - `tests/verification/desktop/verify_home_polish_078.py`：断言由「capability 存在」改为「capability 已移除且宿主选择命令存在」。
- 测试（脚本测试）：
  - `cargo test --lib -- --test-threads=1` → `1284 passed; 0 failed; 1 ignored`（49.54s）。
  - 新增单测：未登记路径拒绝；登记放行；过期拒绝且清理；命中刷新过期时间；符号链接指向 `.ssh/id_rsa` 经真实落点被凭证黑名单拒绝（附件与材料两条都断言）；`..` 穿越到未登记文件拒绝；注册表上限按最早选择淘汰。
  - `npm run build`（含 `tsc --noEmit`）→ EXIT=0；`npx tsc --noEmit` 单独跑亦 EXIT=0。
  - `verify_hardening_2043.py` → `cases=56 failures=0`（含八个命令的授权门、capability 移除、前端不再 import plugin-dialog、拖拽登记、11 个凭证片段）。
- 手工清单（未验证，需要实机窗口）：附件选择（「+」按钮）、阅读导入、知识库根目录设置（选择框）、成果目录设置（文本框与「选择目录…」两条路）。另需实机确认：拖拽一个桌面文件进输入区后能读取；重启应用后旧根目录仍能正常列出（注册表只在内存，重启后靠「成果根/库根子路径」这条规则放行）。
- 未验证：Windows 下的 `blocking_pick_*` 行为；macOS 沙盒/权限提示；`verify_home_polish_078.py` 的真实窗口部分（需要 Xvfb）。


## OD-HARDEN-2043 任务 C：外联 URL 统一校验（ARCHITECT 亲自执行，2026-11-20）

- 改动：
  - `desktop/src-tauri/src/net_guard.rs`（新增）：`NetPolicy::PublicOnly / AllowPrivate`；`assert_public_http_url`、`assert_http_url`、`assert_url_addresses`、`pin_client`、`rejects_scheme_downgrade`。只放行 http/https、拒绝用户名密码、拒绝空主机；回环、未指定、链路本地（含 169.254.0.0/16）、多播、广播在任何策略下拒绝；RFC1918 私网、fc00::/7、100.64/10 只在 `AllowPrivate` 放行；`::ffff:*` 先还原成 IPv4 再判断。
  - `rss_store.rs`：`RssStoreFile` 增加 `allow_private_feeds`（serde default=false，兼容旧 store.json）；删除 `http_client`，改为每跳 `feed_client(policy, url)`（校验 + `resolve` 钉扎 + `redirect::Policy::none()`）；`fetch_feed_xml(url, policy)` 手动最多 3 跳，每跳重新校验，禁止 https→http 降级；新增 `allow_private_feeds` / `set_allow_private_feeds` 支撑函数。
  - `task_shell.rs::read_remote_media`：每跳 `assert_url_addresses(…, PublicOnly)` + `pin_client`，禁止 https 降级；`REMOTE_IMAGE_MAX_BYTES` 与注释/错误文案统一为 8 MiB（原 32 MiB）。
  - `lib.rs`：`pub mod net_guard`；新增并注册 `rss_settings_get`、`rss_settings_set_allow_private_feeds`。
  - `desktop/src/api.ts`、`shell/ReadingPage.tsx`：订阅弹窗新增「允许内网订阅源」勾选（默认关，打开弹窗时读取、切换即保存），同一行右侧注明「回环与 169.254 始终拒绝」。
  - `docs/decisions.md`：新增 D-108，写明策略分档与系统代理下的残余风险。
- 界面借鉴（OD-ARCH-035，动工前用 screenshots-design-inspiration 检索）：Gemini（Android，「Toggle Settings」标签）与 Acuity Scheduling（Web 表单页）都是「左侧标签 + 右侧控件」的单行开关，不配说明段落。本改动照此做成一行紧凑勾选，边界提示压到同一行右侧。
- 测试（脚本测试）：
  - `cargo test --lib -- --test-threads=1` → `1276 passed; 0 failed; 1 ignored`（50.63s；基线 1266 + 新增 10：net_guard 7、rss 重定向 2、媒体回环拒绝 1）。
  - `net_guard` 表驱动覆盖：0.0.0.0、127.0.0.0/8、169.254/16、224/4、239/4、255.255.255.255、::、::1、fe80::/10、ff02::/16 在 AllowPrivate 下也拒绝；10/8、172.16/12、192.168/16、100.64/10、fd00::、fc00:: 在 PublicOnly 拒绝、AllowPrivate 放行；::ffff:127.0.0.1、::ffff:169.254.169.254、::ffff:10.0.0.1、::ffff:100.64.0.1 拒绝，::ffff:8.8.8.8 放行。
  - 订阅源 302 到 169.254.169.254 → 拒绝（错误含「禁止范围」）；连续 302 → 第 4 跳拒绝；图片地址 127.0.0.1 / `[::ffff:127.0.0.1]` / 169.254.169.254 → 拒绝。
  - 既有 `fetches_local_http_feed_and_ingests_entries` 与两条本地媒体单测改用 `#[cfg(test)]` 的线程局部放行入口（`allow_loopback_in_tests`），生产构建不存在该开关。
  - `npm run build`（含 `tsc --noEmit`）→ `✓ built in 1.37s`，EXIT=0；`verify_hardening_2043.py` → `cases=30 failures=0`。
- 未验证：系统代理（`HTTPS_PROXY`）下钉扎失效的实际表现；真实窗口里开关的点击与持久化；真实公网订阅源的跳转链；IPv4/IPv6 双栈解析差异。


## OD-HARDEN-2043 任务 A：发布流程加测试门禁（ARCHITECT 亲自执行，2026-11-20）

- 开工基线：HEAD `2046d12d33c874131ba20319cf1b8686f7b401ad`；`git status --short` 仅 `M PLANS.md`（本任务书计划节）。
- 改动：
  - `.github/workflows/release.yml`：新增 `pull_request` 触发；新增 `test` job（ubuntu-latest），装 Tauri 2 Linux 依赖（libwebkit2gtk-4.1-dev、libgtk-3-dev、libayatana-appindicator3-dev、librsvg2-dev、patchelf），跑 `cargo test --lib -- --test-threads=1`（CI 不带 `--offline`）、`npm ci && npm run build`、`pip install -r scripts/python-requirements.txt` 后跑选定验证脚本；cargo 用 `Swatinem/rust-cache@v2`、npm 用 `actions/setup-node` 的 `cache: npm`（与既有 macos/windows job 同一套缓存）。
  - 依赖图：`meta` 加 `if: github.event_name != 'pull_request'`（PR 上不查 release、不置 publish 标志）；`macos`、`windows` 改为 `needs: [meta, test]`；`publish` 改为 `needs: [meta, test, macos, windows]`。test 失败时 publish 不会被调度。
  - `tests/verification/desktop/verify_hardening_2043.py`（新增）：核对上面这些 YAML 事实（PR 触发、test job、apt 依赖清单、单线程 cargo、无 `--offline`、npm ci/build、依赖图、无 `continue-on-error`），并作为 CI 的门禁脚本之一。
- 本机基线实测（静态检查 / 脚本测试）：
  - `cargo test --lib -- --test-threads=1` → `test result: ok. 1266 passed; 0 failed; 1 ignored`（50.49s），EXIT=0。
  - `npm run build`（脚本为 `tsc --noEmit && vite build`）→ `✓ built in 1.86s`，EXIT=0；仅有既有的 chunk >500 kB 体积警告。
  - `python3 verify_reading_004_formats.py` → `cases=19 failures=0`；`python3 verify_knowledge_staging_202.py` → `cases=14 failures=0`；`python3 verify_hardening_2043.py` → `cases=16 failures=0`。
  - 基线无失败，未使用 `continue-on-error`，未删改任何既有测试。
- Python 子集取舍：CI 只跑 `verify_reading_004_formats.py` 与 `verify_hardening_2043.py`。理由：`tests/verification/desktop` 下 196 个脚本中 62 个依赖 `window_harness`/Xvfb（CI 的 ubuntu runner 无 xvfb-run，且需要真实窗口点击），其余多为一次性验收脚本；`verify_knowledge_staging_202.py` 内部固定调用 `cargo test --offline`，CI 首次运行没有可离线命中的依赖缓存时会直接失败，故不进 CI，仅在本机记录其结果。
- YAML 检查：本机无 `actionlint`（`actionlint not found`）——标「未验证」。替代证据：`ruby -ryaml -e 'YAML.load_file(".github/workflows/release.yml")'` 解析成功，读出 `jobs` 键为 `["meta","test","macos","windows","publish"]`、`publish.needs` 为 `["meta","test","macos","windows"]`；`verify_hardening_2043.py` 的 needs/if 断言全部 PASS（人工核对依赖图：test 不经 meta，publish 经 test）。
- 未验证：Linux runner 上 apt 依赖与 `cargo test` 是否与 macOS 同样通过（本机为 macOS）；`actions/setup-python@v5` + pip 安装 `python-requirements.txt` 的实际耗时与 `lxml` 轮子可用性；GitHub 上 PR 触发的实际行为；actionlint。

## OD-KB-202 暂存知识卡进入「知识审核」再入库（ARCHITECT 亲自执行，2026-11-19）

- 触发：用户反馈模型写在 workspace/staging/kb 的知识卡没有出现在「知识审核」；「同步到知识库」直接落盘，缺少人工确认。
- 改动：
  - desktop/src-tauri/src/knowledge_vault.rs：新增 list_staged_cards（只读列表，含标题/类型/字数/预览/ready 与原因）、sync_selected_from_workspace（只同步勾选卡，未选保留）、discard_staged_cards（拒绝符号链接与路径穿越）；sync_from_workspace 重构为共享内部实现，落库规则不变。
  - desktop/src-tauri/src/lib.rs：新增并注册 knowledge_staging_list / knowledge_staging_sync / knowledge_staging_discard；审核入库后向治理记录追加 approve_card（来源 staging），失败只追加说明不掩盖落库结果。
  - desktop/src/api.ts：新增暂存列表/选中入库/丢弃类型与调用。
  - desktop/src/shell/KnowledgeStagingInbox.tsx（新增）：审核页「暂存卡待审核」区，多选、全选、入库选中/全部入库/丢弃选中，均带二次确认；显示标题、类型、字数、正文预览与不可入库原因。
  - desktop/src/shell/KnowledgeVaultPage.tsx：审核页挂载暂存卡区，入库/丢弃后刷新扫描、审核队列与治理数据。
  - desktop/src/shell/KnowledgeIngestPanel.tsx：「同步到知识库」改为「全部入库」，必须二次确认。
  - tests/verification/desktop/verify_knowledge_staging_202.py（新增）：静态 + 真实 Rust 定向单测。
- 验证：cargo test --offline --lib -- --test-threads=1 → 1250 passed / 0 failed / 1 ignored（新增暂存列表/选中同步/丢弃/路径穿越单测）；npx tsc --noEmit → 0；npm run build → 0；verify_knowledge_staging_202.py → 14/14。
- 证据等级：文件事实、静态检查、脚本运行（cargo/tsc/build/verify）。真实窗口点击与用户 5 张卡的实际入库仍待 0.2.42 实测。
- 数据边界：未修改用户暂存卡；验证只用 /tmp 合成根。

## OD-ARCH-187 审批请求静默丢弃修复（ARCHITECT 亲自执行，2026-11-19）

- 触发：用户实测「未分类对话要求存入知识库」两次提权但没有看到审批弹窗，第三次模型改用 artifacts/staging 后暂时成功。
- 根因（源码事实）：`web_api_adapter._on_approval_request` 用审批帧 `agentId` 精确匹配 `_sessions`，匹配失败直接 `return`；当工具运行在子代理/其它 agent 会话时，`agentId` 与任务的 `engine_session_id` 不一致，审批被静默丢弃——界面收不到审批卡，DSH 等不到应答后按取消收尾。
- 修改：`engine/omnidesk_engine/web_api_adapter.py` 改为三级归属：精确匹配 → 唯一活跃任务（标注 `single_active`）→ ambiguous/unmatched；未归属请求登记 `_unattributed_approvals`（上限 20）并向最近活跃任务发 `engine_ask` 可见提示；取消帧同步清理。`skills/knowledge-search/SKILL.md` 补充「父目录 workspace/staging 是历史错误路径、不会被同步」。
- 验证：`python3 -m unittest test_web_api_unit` → 102/102（新增唯一活跃归属与歧义提示两条用例，原「不归属」断言更新为新语义）；`python3 tests/verification/desktop/verify_knowledge_sync_152.py` → 48/48；`py_compile` 通过。前端与 Rust 本轮未改。
- 证据等级：文件事实（源码/diff）、静态检查（verify 脚本）、单元测试。真实窗口审批弹窗与子代理场景仍待 0.2.41 实测。
- 数据边界：用户已暂存的 5 张卡仍在 `~/OmniDesk/workspace/artifacts/staging/kb/知识库/方法/`，未改动未迁移；可在知识库页执行同步落库。

## OD-READ-004 首页统计下移 + MOBI/PDF 原生导入（ARCHITECT 亲自执行，2026-11-18）

- 开工基线：工作树 `/Users/yvan/.codex/worktrees/2221/MyWorkStation`，分支 `od-read-002-brain-room`，HEAD `f008ef94bb`；未提交的 OD-READ-003 改动全部保留，本任务在其之上继续。改动已于 2026-11-19 合并进 main 并推送（见 Git 记录）。
- 改动清单：
  - `desktop/src/shell/HomeCanvas.tsx`：统计条从问候区移出，渲染在 `HomeChat` 下方（`home-reading-stats-slot`，`pt-4` 间距）；删除「最近项目/项目跟踪」快捷入口及 `projects` 状态、`projectList` 效果与 `ProjectInfo` 导入；对话容器增加 `pt-2 md:pt-3` 下移间距。
  - `scripts/reading_attachment_extract.py`：新增 `extract_mobi`——Palm Database 记录切分、PalmDOC LZ77 解压（压缩 0/1/2）、MOBI 头文本编码（65001/1252）、HTML 转纯文本、EXTH 封面（201/202 + 首图记录）；HUFF/CDIC 与 DRM 明确拒绝。
  - `desktop/src-tauri/src/task_shell.rs`：`READING_EXTRACT_EXTS` 增加 `mobi`；错误提示格式清单同步；新增单测 `reads_reading_source_mobi_via_helper`（合成 MOBI 经 helper 读入正文）。
  - `desktop/src/shell/ReadingPage.tsx`：导入文件过滤器增加 `mobi`。
  - `scripts/python-requirements.txt`、`scripts/prepare-runtime-assets.sh`、`docs/OmniDesk-Desktop-Data-Layout.md`：PDF 依赖 pypdf 内置到 python-vendor，格式清单补 mobi。
  - `tests/verification/desktop/verify_reading_004_formats.py`（新增）：19 项静态/脚本断言。
  - 本地 `runtime/python-vendor` 安装 pypdf 6.21.4（未跟踪构建工件，仅用于本机验证与开发构建；发布构建按 requirements 自动内置）。
- 真实命令与结果：
  - `cargo test --offline --lib -- --test-threads=1`（`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation`） → 1249 passed / 0 failed / 1 ignored（50.53s，含新增 MOBI 单测）；定向 `reads_reading_source_mobi_via_helper` → 1/1。
  - `npx tsc --noEmit` → exit 0；`npm run build` → exit 0。
  - `python3 tests/verification/desktop/verify_reading_004_formats.py` → cases=19 failures=0（含 MOBI 未压缩/PalmDOC/HUFF 拒绝/DRM 拒绝、PDF 文字提取）。
  - `python3 tests/verification/desktop/verify_reading_003_selection.py` → cases=25 failed=0（既有回归）。
  - `git diff --check` → 干净。
- 证据等级：文件事实（源码/diff）；静态检查（verify 脚本、tsc）；脚本运行（cargo、build、Python helper 合成样例）。
- 未验证项：真实用户 MOBI 文件（含 HUFF/CDIC 压缩的常见亚马逊书仍会拒绝）、真实 PDF 中文内嵌字体/扫描件、真实窗口下首页统计条位置与间距手感、Windows 实机。
- 已知限制：MOBI 仅支持未压缩/PalmDOC 压缩且无 DRM；PDF 扫描件无 OCR；AZW3/KF8 未专门支持。
- 本机调试构建（2026-11-18）：`npx tauri build --debug --bundles app` → exit 0，产物 `desktop/src-tauri/target/debug/bundle/macos/OmniDesk.app`（486.81 MiB，未签名调试包）；未启动窗口（本机已有正式版实例占用同一数据根）。

## OD-READ-003 健脑室：首页统计 + 左栏分页 + 富媒体阅读 + 选区浮条修复（2026-11-18）

- 开工基线：工作树 `/Users/yvan/.codex/worktrees/2221/MyWorkStation`，分支 `od-read-002-brain-room`，HEAD `f008ef94bb`；`git status --short --branch` 显示既有 `M STATUS.md` 与 4 个未跟踪 `runtime/*` 构建工件，全部原样保留，未还原/清理/stash。
- UI 参考（screenshots-design-inspiration MCP，实查 3 组）：`epub reader highlights` → My Reader AI；`rss reader typography` → My Reader AI；`book library covers` → Apple Books。借鉴要点：左栏用页签分流书籍/文章，订阅页按来源折叠；书架封面卡用暖色占位与真实封面并置，正文保持单列低干扰排版。
- 改动清单：
  - `desktop/src/shell/ReadingPage.tsx`：段落补 `data-paragraph`；新增 document 级 `selectionchange`（150ms 去抖）与 mouseup 共用 `readSelectionRef`；左栏「本地书籍 / 订阅文章」页签、封面卡与暖底首字占位、订阅来源折叠分组、未读点；`![img](…)` / `[audio](…)` / `[video](…)` 行内媒体渲染为 figure/原生控件、图片点击放大、`odmedia:` 经命令取 base64 并缓存；段落内粗体/行内代码/链接用偏移保真渲染（同步保留隐藏语法锚点）。
  - `desktop/src/reading.css`：书架页签、封面、媒体卡（14px 圆角/描边/柔和阴影）、图片 lightbox、首页统计条与语法锚点样式。
  - `desktop/src/shell/HomeCanvas.tsx`：问候区下新增紧凑阅读统计条，点击跳转健脑室；读取失败显示错误，不显示假 0。
  - `desktop/src-tauri/src/reading_store.rs`：`reading_stats_at`（书籍本数排除 rss；已读完 `progress_percent >= 0.95`；字数/批注/笔记/闪记求和，闪记=`kind == "thought"`）；`ReadingBookMeta` 增加 `cover`、`thought_count`；导入 EPUB 时把 OPF cover-image（≤2MB）写入 `meta/<book_id>.cover.<ext>` 并提供封面读取。
  - `desktop/src-tauri/src/rss_store.rs`：`sanitize_html` 的 img 改为保留 data:/http(s) 的 `![img](原src)`，audio/video（含 `<source>`）输出对应标记，其余协议丢弃；新增 `article_count`。
  - `desktop/src-tauri/src/task_shell.rs`：helper 输出 cover 与 base64 解码；新增 `read_reading_media`（绝对路径 + EPUB + 路径防穿越 + ≤10MB + image/audio/video 白名单），base64 编解码辅助。
  - `desktop/src-tauri/src/lib.rs`：注册 `reading_stats`（合并 RSS 文章篇数）、`reading_media_extract`、`reading_cover_image`。
  - `desktop/src/api.ts`、`desktop/src/types.ts`：阅读统计、媒体/封面回执、`thought_count`/`cover` 类型与调用。
  - `scripts/reading_attachment_extract.py`：`_html_blocks` 不再丢弃 img/audio/video，输出 `![img](odmedia:<zip路径>)` / `[audio](…)` / `[video](…)`；OPF cover-image 提取；新增 `--media` 单条目 base64 提取（10MB 上限、类型白名单）。
  - `tests/verification/desktop/verify_reading_003_selection.py`（新增，未跟踪）。
- 验证命令与真实结果：
  - `cd desktop/src-tauri && PATH="$HOME/.cargo/bin:$PATH" OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation cargo test --offline --lib -- --test-threads=1` → `1248 passed; 0 failed; 1 ignored`（50.16s）。注：工作树位于 `~/.codex`，若把 `OMNIDESK_REPO_ROOT` 指向工作树，`app_paths` 3 个仓库边界测试会先命中 `/.codex` 禁区而失败；按既有 OD-READ-002 做法指向主仓库后全部通过，本任务未改 `app_paths.rs`。
  - `cd desktop && npx tsc --noEmit` → exit 0；`npm run build` → exit 0（ReadingPage chunk 51.54 kB，仅既有 500 kB chunk 提示）。
  - `python3 tests/verification/desktop/verify_reading_003_selection.py` → `cases=25 failed=0`（含 data-paragraph、selectionchange、浮条条件、统计条/页签、thought 口径、odmedia 标记与 cover payload）。
  - 合成 EPUB 端到端（`OMNIDESK_READING_ROOT` 指向 /tmp 一次性根）：`cargo test --offline --lib end_to_end_epub_cover_and_media_under_env_root -- --test-threads=1 --nocapture` → `book=bk_dd255810dbcd0e19 cover=bk_dd255810dbcd0e19.cover.jpg cover_exists=true`、`media mime=image/png bytes=6`，测试 1/1 通过。
  - `git diff --check` → exit 0。
- 计划偏差：
  1. 「闪记」按用户当轮语义落地为 `thought` 批注总数（计划原文曾写“不做闪记”）。
  2. 段落内行内 Markdown 未直接用 react-markdown 输出：其会移除 `**`/反引号/链接目标，导致 DOM 前缀长度与原文不一致、划线偏移错位；改为偏移保真的轻量渲染（隐藏语法锚点）。笔记与 AI 结果仍走既有 `MarkdownText`（react-markdown）。
  3. 为在书架/阅读头部显示封面，新增 `reading_cover_image` 命令（计划只要求存 `meta/<book_id>.cover.*` 与 meta cover 字段）。
  4. cover 字节由既有 Python EPUB 解析 helper 从 OPF 取出，`reading_store.rs` 负责落盘与 meta 登记；未新增 Rust ZIP 依赖（`Cargo.toml` 不在允许改动清单）。
- 证据等级：文件事实（源码/diff）；静态检查（tsc、verify 脚本）；脚本运行（cargo test、build、端到端合成 EPUB）；未做真实桌面窗口点击、真实模型调用、真实用户 EPUB/RSS 媒体源。
- 未验证项：真实窗口选区浮条位置与拖选手感、封面/长图的视觉排版、真实 RSS 音视频远端加载、真实模型解释/整理质量。
- Git：改动已于 2026-11-19 合并进 main 并推送；`runtime/*` 4 个未跟踪工件保持原样、未入库。`git diff --stat`（提交前）→ STATUS.md 40+ / lib.rs 68+ / reading_store.rs 276+ / rss_store.rs 67+ / task_shell.rs 304+ / api.ts 23+ / reading.css 193+ / HomeCanvas.tsx 85+ / ReadingPage.tsx 582+ / types.ts 22+ / reading_attachment_extract.py 232+（11 files, 1775 insertions, 117 deletions）；`tests/verification/desktop/verify_reading_003_selection.py` 为新增文件。

## OD-READ-001 修版 0.2.38（ARCHITECT，2026-11-18）

- 触发：用户反馈 AI 查询结果占屏过大；要求右侧面板可向左展开/收缩。
- 改动：`ReadingPage.tsx` 增加面板宽度状态与左缘拖拽手柄（300–640px），双击在 340/560px 切换；宽屏默认展开、窄屏默认收起；顶栏「笔记」按钮与面板关闭按钮都可开合；`reading.css` 增加 `reading-panel-resizer`。
- AI 结果仍限制在面板内滚动；阅读区随面板收放自动变宽/变窄。
- 验证：`npx tsc --noEmit`、`npm run build` 通过；`verify_reading_001_fixes.py` 11/11（新增面板拖拽/开合断言）。
- 未验证：Windows 实机拖拽手感与窄屏覆盖效果。

## OD-READ-001 修版 0.2.37 发版（ARCHITECT，2026-11-18）

- 提交 `33fa63b596`；run `44229046992` 全绿（meta / macos / windows / publish）。
- release `v0.2.37` 已发布，产物：`OmniDesk_0.2.37_universal.dmg`（181 MB）、`OmniDesk_0.2.37_x64-setup.exe`（84 MB）。
- Windows 直链：`https://github.com/klzone/omniharness/releases/download/v0.2.37/OmniDesk_0.2.37_x64-setup.exe`
- 说明：EPUB 修复针对 XHTML DOCTYPE 与 div-only 正文；AI 报错中的 `getaddrinfo ENOTFOUND` 属模型端点/网络 DNS 环境，代码只改为可操作提示，未改端点或凭证。
- 证据等级：脚本运行（gh run/release 实查）；真实用户 EPUB 与 Windows 实机未复测。

## OD-READ-001 修版 0.2.37（ARCHITECT，2026-11-18）

- 触发：用户 Windows 实测反馈三处问题：EPUB 导入提示未提取正文并闪控制台黑框；选中文字后没有出现标记/笔记/查询；AI 阅读助手报 `getaddrinfo ENOTFOUND`。
- 修复 1（EPUB）：`scripts/reading_attachment_extract.py` 原先把 XHTML 的 `<!DOCTYPE …>` 当作不安全 XML 直接拒绝，导致正常 EPUB 所有章节被跳过；现剥离 DOCTYPE、拒绝内部实体，并对只含 `<div>` 的正文与 div 章节名做兜底。合成 EPUB 验证：DOCTYPE + `<p>` 章节、纯 `<div>` 章节均能解出标题与正文。
- 修复 2（Windows 黑框）：`task_shell.rs` 的 Python 抽取子进程在 Windows 使用 `CREATE_NO_WINDOW`，不再弹出控制台。
- 修复 3（选区工具栏）：`ReadingPage.tsx` 改为 document 级 mouseup + 键盘 selection 兜底，工具栏避开顶栏；划线/标记/想法保存后自动打开右侧「笔记」，并给出回执。
- 修复 4（AI 错误）：`describeAiError` 将 `getaddrinfo/ENOTFOUND` 转成「模型端点域名解析失败，请检查网络/代理/服务商地址」，保留原始错误。
- 验证：`cargo test --lib -- --test-threads=1` 1232/0/1；`npx tsc --noEmit`、`npm run build` 通过；`verify_reading_001_fixes.py` 9/9。
- 未验证：真实用户这本 EPUB 原文（只用合成/公开结构样例）；Windows 实机安装与选区手感；AI DNS 属用户网络/端点环境，未在其电脑上复测。

## OD-READ-001 发版 0.2.36（ARCHITECT 亲自执行，2026-11-18）

- 触发：用户裁决「推送并编译最新版本，去单位电脑测试 Windows 版本」；属 OD-READ-001 收尾与发版，未扩大功能范围。
- 版本收口：`VERSION` 与 `desktop/package.json` 0.2.35 → 0.2.36，`scripts/sync-app-version.sh` 同步 `tauri.conf.json`；提交 `2415894cf6`（含 CHANGELOG 0.2.36、REVIEW/STATUS 复核记录、0.2.35 未发布说明）。
- 流水线：`gh run list` 显示读书提交触发的旧 run `44200944724`（当时会以未发版的 0.2.35 为 tag）被并发组取消；新 run `44203067532` 全绿——meta / macos / windows / publish 均 success。
- 发布：`v0.2.36`，非草稿、非预发布，产物两项：`OmniDesk_0.2.36_universal.dmg`（181 MB）与 `OmniDesk_0.2.36_x64-setup.exe`（84 MB）。
- Windows 安装包直链：`https://github.com/klzone/omniharness/releases/download/v0.2.36/OmniDesk_0.2.36_x64-setup.exe`；Release 页：`https://github.com/klzone/omniharness/releases/tag/v0.2.36`。
- 证据等级：脚本运行（gh run / gh release 实查）；未在 Windows 实机安装与操作，未做真实模型输出验收。
- 使用前提（沿用既有发版说明）：目标机需 Node.js 22.19+ 与 Python 3.10+；安装包未签名，SmartScreen 选「仍要运行」。

## OD-READ-001 读书阅读笔记分页（ARCHITECT 亲自执行，2026-11-18，待复核）

- 开工基线：`git status --short --branch` 为 `## main...origin/main`，HEAD `b8b9357b9c`；任务期间用户另有一次发版提交，已保留、未回退、未改写历史。本轮未提交、未推送、未打标签。
- 改动清单：
  - `desktop/src-tauri/src/reading_store.rs`（新增）：仓库外阅读目录 `~/OmniDesk/reading/`（books/meta/progress/notes/exports），书目按正文 SHA-256 去重；章节/段落切分；批注按章节+段落+code point 偏移+引文锚定，落盘前复核并拒绝歧义引文；进度单独落盘，避免滚动时重写整本书；确定性 Markdown 导出；Unix 下目录 0700、文件 0600。
  - `desktop/src-tauri/src/task_shell.rs`：`read_reading_source` 与 32 MiB 阅读源上限；非 UTF-8/PDF/DOCX/EPUB 复用 Python helper，阅读抽取超时 120 秒；新增 UTF-8 与 GB18030 两条单测。
  - `scripts/reading_attachment_extract.py`：新增 EPUB（container/OPF/spine/XHTML 块级正文）与纯文本 UTF-8/UTF-16/GB18030 解码；保留 DOCX/PDF 原有路径。
  - `desktop/src-tauri/src/lib.rs`：新增 10 个 `reading_*` Tauri 命令并注册；只读用户显式选择的源文件，不扫描目录、不写知识库。
  - `desktop/src/types.ts`、`desktop/src/api.ts`：阅读书目/批注/整理稿/进度类型与调用。
  - `desktop/src/readingAi.ts`（新增）与 `readingAi.test.mjs`（新增）：概念提示词与整理提示词纯函数；只带当前段落/选中引文/已有批注，整理稿最多 80 条并标注未复核边界。
  - `desktop/src/shell/ReadingPage.tsx`（新增）：书架、阅读器、选区浮条（划线/想法/标段/问 AI）、笔记侧栏、回跳/编辑/删除、随时问概念、AI 整理与保存、导出、删除确认。
  - `desktop/src/App.tsx`、`shell/Dock.tsx`、`main.tsx`、`reading.css`（新增）：懒加载「读书」平台入口与阅读排版。
  - `docs/decisions.md` D-111、`docs/OmniDesk-Desktop-Data-Layout.md` §5.2、`PLANS.md` OD-READ-001 执行结果。
- 实际命令与结果：
  - `cargo test --offline --lib -- --test-threads=1` → 1225 passed / 0 failed / 1 ignored（50.12s）。
  - `cargo test --offline --lib reading_store` → 5/5；`reading_source` → 2/2；`cargo check --offline` → exit 0。
  - `npx tsc --noEmit` → exit 0；`npm run build` → exit 0（ReadingPage 懒加载 chunk 约 34.0 kB）。
  - `node --experimental-strip-types --test src/readingAi.test.mjs` → 4/4。
  - `python3 scripts/reading_attachment_extract.py /tmp/od-reading-helper-test/sample.epub --format epub` → 两章正文正确；GB18030 纯文本样例正确。
  - `git diff --check` → exit 0。
- 证据等级：文件事实（源码与 diff）；脚本运行（cargo/tsc/build/node/Python）；未做真实桌面窗口点击与真实模型调用。
- 未验证项：真实窗口选区浮条与回跳定位；真实模型概念解释/整理质量；32 MiB 级 PDF 抽取耗时；跨段选择（当前明确拒绝并提示）；EPUB 复杂样式/图表书可能只得到块级文字。
- 数据边界：测试全部使用 `/tmp` 或一次性目录；仓库内未写入书正文、批注或真实材料；未改 `runtime/`、`engine/`、凭证或模型路由。

## OD-ARCH-175 阶段 3 执行记录（子代理，2026-11-17）

- 开工基线：`git status --short` 为 `M PLANS.md`、`M REVIEW.md`；HEAD `5754da202c`。执行期间 STATUS.md 出现阶段 1 子代理追加的只读核查记录，本阶段原样保留，未改写其内容。
- 改动清单：
  - `desktop/src/api.ts`：新增 `planModelRouteEffort` 纯函数与 `collectModelEffortEvidence`；`setModelRoute` 移除两处阻断式 throw。目录非空声明按集合判定（情形 2 正常、情形 3 改发 `default`）；目录不可用/模型未列出/目录空声明时再看 Profile `modelEntries.reasoningEfforts`（`false` 或空对象视为声明为空）；两处都无法确证时照所选档位提交并返回 advisory。返回值新增 `reasoningEffort` 与 `advisory`，并留阶段 2/4 TODO。
  - `desktop/src/types.ts`：`ModelCatalogEntry.efforts` 改为可选（`ModelCatalogEffort[] | null`），反映归一化目录可能缺项。
  - `desktop/src/shell/ModelPicker.tsx`：档位提示改为「未声明思考档位；若不支持会按默认档切换。」/「该模型不支持「X」，切换会按默认档。」；保留 `data-testid="effort-advisory"`；模型列表对 `efforts` 可选做空值保护。
  - `desktop/src/shell/HomeChat.tsx`：切换成功时优先展示 `r.advisory`（一句话、info 提示条）；情形 3 用 `setModelEffort(r.reasoningEffort)` 同步界面到实际生效的默认档；留阶段 2/4 运行期回退通知 TODO。
  - `tests/verification/desktop/verify_model_route_precheck_175.py`（新增）：用 `tsc --ignoreConfig` 将真实 `api.ts`/`engineAdmission.ts` 转译到临时目录，在 Node 中 mock `invoke` 调用真实 `shell.setModelRoute`；覆盖情形 1（未列出/目录不可用/efforts 缺失/空目录+Profile 未声明）、情形 2（目录集合内/Profile 集合内）、情形 3（目录集合外/Profile false/Profile 空对象/未列出但 Profile 集合外）、requested default 不做预检；断言实际下发档位与 advisory 文案。
  - `tests/verification/desktop/verify_home_refine_202.py`：更新 `effort_advisory` 断言到新文案（原断言依赖「可能被忽略」）。
  - `STATUS.md`：本小节。
- 判定口径：DSH 归一化目录把「没有 reasoning 声明」与「显式声明为空」都压成 `efforts: []`（`engine/omnidesk_engine/web_api_adapter.py` 只读核对），所以空目录不能单独当必败证据；能读到 Profile `reasoningEfforts: false` 或空对象时才本地改发默认档。无法区分 raw `reasoning` 是否显式为空数组的情形，仍可能把「声明为空」当作「未声明」而尝试一次；属已知限制，未改 engine/runtime。
- 实际命令与结果：
  - `cd desktop && npx tsc --noEmit` → exit 0。
  - `python3 tests/verification/desktop/verify_model_route_precheck_175.py` → cases 12/12、static 11/11，exit 0；证据 `tests/verification/desktop/evidence/od-arch-175/precheck.json`（evidence 被 `.gitignore` 忽略）。
  - `python3 tests/verification/desktop/verify_home_refine_202.py` → 39/40，唯一失败 `home_scene_backdrop`（`App.tsx` 未含 `app-scape-art`；本轮未改该文件，属既有失败）。
  - `git diff --check` → exit 0。
- 证据等级：文件事实（源码与 `web_api_adapter.py` 只读核对）；静态检查（`verify_home_refine_202` 文案断言）；脚本运行（TypeScript 编译 + Node 调用真实 `setModelRoute` + mock invoke）。未做真实桌面窗口点击与真实模型调用。
- 未验证项：真实窗口下 ModelPicker/HomeChat 提示条的渲染与自动消失；真实 volcengine 切换后 DSH 是否按预期接受 `off` 或被阶段 2 回退链路处理（阶段 2/4）；raw `session/modelCatalog` 中 `reasoning` 显式为空与未声明的区分（归一化目录丢失该信息）。
- Git：HEAD `5754da202c`；未提交、未推送、未打标签。`desktop/src-tauri/`、`runtime/`、`engine/` 未改；模型路由与凭证未动；未接触真实业务材料。

## OD-ARCH-174 阶段 C：核心技能正文重写（2026-11-17，待复核）

- 重写三个技能正文，frontmatter 未动：`memory-agent` 1754 → 1228 字符（正文 1578 → 1052，66.7%）；`memory-report` 1099 → 749（正文 950 → 600，63.2%）；`period-review` 1820 → 1268（正文 1668 → 1116，66.9%）。三份全文比例均在 68%–70%。
- 删除：平台写盘边界/材料已提供等重复说明、真实地名示例、占位符还原机制、周报中转说明、按需加载其他技能（与主指针冲突）、自检清单、四组 Markdown 围栏（标题与顺序逐字改写成紧凑序列）、草稿/确认的平台流程说明。保留：7 条随手记动作规则、日期窗口、verdict 块与四字段 JSON、六要素、去向、“本机是工作记录唯一存储地，可记录案件/审查调查日常工作事实”；周报五段标题、`本周报引用`、`不得补造`、精确短语“只有用户明确确认后才落盘”、`不覆盖`、`-2` 规则；复盘四周期完整标题序列、`#wr-`/`#t-` 引用格式、`不补造`/`不得虚构`、只交 Markdown 正文、数据边界。
- verify：`verify_skill_bindings_082.py` 69/69、`verify_worklog_089.py` 86/86、`verify_wr_503.py` 58/58、`verify_wr_505.py` 71/71 全过。
- mock：`run_mock_session.py --label stageC` 的 excel / weekly / knowledge `succeeded`（weekly 对应 memory-report），tool_probe 30 秒上限内仍 `running`；note 与 review 用 `stage2_prompt_session.py` 补跑，user 634 / 701 字符，均 `succeeded`；`skill-catalog` 2401。
- 真实模型（overlay `~/OmniDesk/app/config/omnidesk-model.yml`，weekly 与 knowledge 分开 root 跑）：weekly `succeeded` 20.07s / ttfb 17.56s / 8375 tokens（cacheRead 7808）；knowledge `succeeded` 66.24s / ttfb 61.99s / 14310 tokens（cacheRead 13568），三张卡落 `workspace/staging/kb/知识库/方法/`。weekly 产出四节（本周完成/进行中/风险/下周计划），与 0.2.30 smoke 的四节行为一致；`run_synthetic_tasks.py` 发的是裸提示词，不经过桌面五段契约。
- **关键限制：** 两次真实任务 `tool_event_count=0`，模型没有调用 `skill`，因此重写后的正文没有被真实模型读取；真实回归只证明任务可跑通、产出仍正常，正文质量与模型读后行为未验证。
- 同步：`sync-skills.sh`（沿用 `/tmp/od174-bin/realpath` 包装）exit 0，`--check` 一致；未改其他 7 个技能、`runtime/`、`aiDraft.ts`、凭证或模型路由。
- 证据：`tests/verification/desktop/evidence/od-arch-174/stageC.md`、`mock-run-stageC.log`、`mock-prompt-note-review-stageC.log`、`real-weekly-stageC.log`、`real-knowledge-stageC.log`（evidence 被 `.gitignore` 忽略）。
- Git：HEAD `f1e06a3`；新增 `M skills/memory-agent/SKILL.md`、`M skills/memory-report/SKILL.md`、`M skills/period-review/SKILL.md`；未提交、未推送、未打标签。

## OD-ARCH-174 阶段 B：目录瘦身（2026-11-17，已按裁决收缩，待复核）

- 最终改动：仅 `skills/document-review/SKILL.md` 保留 `disable-model-invocation: true`；`material-writing` / `knowledge-search` / `anysearch` 的标记已撤回（前两个是绑定指针要求模型读正文，anysearch 是阶段 1 明确保留的按需检索能力）。其余 6 个技能未动；未改 `runtime/`、`aiDraft.ts`、凭证或模型路由。
- 裁决依据：全量 4 标记曾把目录压到 2039 字符（−479），但实测 `skill` 工具会拒绝模型读取被标记技能（`Error: skill "material-writing" is not available for model invocation`，69 字符、无正文），与阶段 A/A2 指针冲突。ARCHITECT 裁决“能力优先、目录开销其次”，仅保留无绑定指针指向的 `document-review`。
- 最终 mock（`run_mock_session.py --label stageB-final`）：`skill-catalog` 2518 → 2401 字符（−117，−4.65%），目录技能 19 → 16；只有 `document-review` 消失，`anysearch` / `material-writing` / `knowledge-search` 仍在目录中。4 条会话均为 2401。
- 同步：`sync-skills.sh` 在 macOS 上因 BSD `realpath` 不支持 `-m` 直接失败，未改脚本，用 `/tmp/od174-bin/realpath` 兼容包装后执行；最终状态先报 3 个 Skill 漂移，同步后 `--check` 一致（exit 0）。隔离副本核对：仅 `document-review` 带标记。同步副作用：全量同步时删除旧备份 `knowledge-search/SKILL.md.bak-1794761541` 并重新物化两个 references 目录（同步前已有漂移），均为隔离运行时副本。
- 回归（最终状态补跑）：`verify_conversation_prompt.py` 33/33、`verify_skill_bindings_082.py` 69/69、`verify_skill_scope_107.py` 33/33、`verify_worklog_089.py` 86/86 通过；`sync-skills.sh --check` 一致。
- 未验证：真实模型行为；用户手点 `document-review` 的真实窗口路径；macOS 下 `sync-skills.sh` 的直接可运行性。
- 证据：`tests/verification/desktop/evidence/od-arch-174/stageB.md`、`mock-run-stageB-final.log`、`stageB-probe-material-writing.log`（全量试验的阻断证据，evidence 被 `.gitignore` 忽略）。
- Git：HEAD `f1e06a3`；`skills/` 下唯一改动是 `M skills/document-review/SKILL.md`；未提交、未推送、未打标签。

## OD-ARCH-174 阶段 A2：knowledge 专用绑定键（2026-11-17，待 ARCHITECT 复核）

- 改动：`samples/profiles/discipline.rules.json` 新增 `"knowledge": {"skills":["knowledge-search"],"max_injected":1}`；`conversationPrompt.ts` 的 `ConversationPreset` 与组装输入增加可选 `knowledgeSkills`，知识指针改从该字段取首个非空项（不再从 `preset.skills` 找）；`HomeCanvas.tsx` 独立读取 `skill_bindings["knowledge"]` 并透传；`HomeChat.tsx` 只加字段透传。未改 `aiDraft.ts`、`runtime/`、凭证或模型路由。
- 普通对话的透传理由：未分类/自定义分类对话 `preset=null`，知识检索随时可能发生；因此 `HomeCanvas` 用独立 `useMemo` 读绑定，`HomeChat` 透传 `knowledgeSkills` 给组装器。不混入 `preset.skills`/`skillsByFlow`，四场景主指针不受影响。
- 规则校验：`profile_rules.rs` 的绑定键校验无场景白名单（只要求非空、每场景 ≤3、总键数 ≤12），`knowledge` 键直接通过；出厂规则单测同步改为「`knowledge-search` 只进专用 `knowledge` 绑定，其余场景绑定仍不得预注入它」。设置页把非固定流程键作为额外键原样保留，整表保存不会冲掉。
- 决策记录：`docs/decisions.md` 在 D-035 之后新增 D-108，写明指针机制使注入成本从 4901 字正文降为 151 字指针、原「没有注入点」前提失效；`knowledge` 绑定独立于四场景主指针；复审触发四条。
- 探针（真实源码编译后 Node 调用，与 `HomeCanvas` 同构）：weekly-note 无检索 → `memory-agent`；weekly-report → `memory-report`；writing → `material-writing`；project → `project-agent`；weekly-note + 检索上下文 → 主指针仍 `memory-agent` + 知识指针（855 字符）；writing + 检索上下文 + `knowledgeSkills` → 主指针 `material-writing` + 知识指针（1115）；普通对话（`preset=null`）+ 检索上下文 + `knowledgeSkills` → 只有知识指针（235）。知识指针 151 字符。
- 结果：`cd desktop && npx tsc --noEmit` exit 0；`verify_conversation_prompt.py` 33/33、`verify_skill_bindings_082.py` 69/69、`verify_worklog_089.py` 86/86、`verify_work_review_213.py` 25/25、`verify_skill_scope_107.py` 33/33、`verify_profile_assets_109.py` 41/41 通过；`verify_profile_rules_068.py` 26/27，唯一失败仍是既有 `16_keywords`（本地拦截词期望 16、实际 2）；`verify_capabilities_072.py` 因本机无 Xvfb 未运行；`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1200 passed / 0 failed / 1 ignored（49.02s）。
- 未验证：真实模型检索行为；现有安装的 `~/OmniDesk/app/profiles/discipline/rules.json` 未迁移（只改仓库出厂资产，按约束不动用户数据），需「恢复默认规则包」或重装后 `knowledge` 绑定才生效；设置页不显示可勾选的 `knowledge` 固定行（仅作为额外键保留）；真实窗口提交链路未运行。
- 证据：`tests/verification/desktop/evidence/od-arch-174/stageA.md` 第 6 节与 `stageA_probe.json`（evidence 被 `.gitignore` 忽略）。
- Git：HEAD `f1e06a3`；本阶段在阶段 A 工作树上新增 `M docs/decisions.md`、`M desktop/src/shell/HomeCanvas.tsx`、`M desktop/src/shell/HomeChat.tsx`、`M desktop/src-tauri/src/profile_rules.rs`、`M samples/profiles/discipline.rules.json`、`M verify_worklog_089.py`；未提交、未推送、未打标签。

## OD-ARCH-174 阶段 A：指针补全与绑定修正（2026-11-17，待 ARCHITECT 复核）

- 改动：`desktop/src/conversationPrompt.ts` 新增 `defaultSceneSkills()` 与 `knowledgeSkillPointer()`（`KNOWLEDGE_POINTER_MAX_CHARS = 260`）。默认分支不再无条件取 `presetSkillsFor(preset, "report")`：仅当 `preset.key === "weekly"` 且 `mode="report"` 或宿主传了 `worklog` 时用 `weekly-report` 绑定，writing / project / 无意图一律取本场景 `preset.skills`。`knowledgeContext` 非空且 `preset.skills` 绑定 `knowledge-search` 时，在检索上下文后追加 151 字符检索指针（先读 `库/知识库/索引.md`、按标签定位、只加载 1–2 张卡、未命中如实说明）；未绑定或没上下文不注入，正文不入提示词。note / daily-summary / period-review 分支与 weekly-report 指针不变。
- 选择最小改法的理由：`preset.key` 已是场景键（与 `scenes.ts`、`HomeCanvas` 一致），周报流程用宿主已传入的 `worklog`/`mode` 判定，不改 `HomeCanvas`/`HomeChat`/`aiDraft.ts`，也不新增 preset 字段。
- 验证脚本：`verify_skill_bindings_082.py` 新增 4 条断言（writing/project 残留 `skillsByFlow.report` 时仍只取场景绑定；weekly 无 mode/worklog 用 `memory-agent`；weekly 有 worklog 用 `memory-report`）；`verify_conversation_prompt.py` 新增 7 条 knowledge 指针断言（上下文顺序、≤260、正文不入、无绑定/无上下文不注入）。旧断言全部保留。
- 四场景直接组装（真实源码编译后 Node 调用，与 `HomeCanvas` 同构）：weekly-note → `memory-agent`（642 字符）、weekly-report → `memory-report`（918）、writing → `material-writing`（899）、project → `project-agent`（865）；knowledge（写作场景追加 `knowledge-search` 绑定 + 检索上下文）→ 主指针 `material-writing` + 检索指针 `knowledge-search`（151 字符）。
- 结果：`cd desktop && npx tsc --noEmit` exit 0；`verify_conversation_prompt.py` 33/33、`verify_skill_bindings_082.py` 62/62、`verify_worklog_089.py` 85/85、`verify_attachments_077.py` 25/25、`verify_resume_token_savings.py` 11/11、`verify_ai_draft.py` 27/27 通过。`verify_profile_rules_068.py` 26/27，唯一失败 `16_keywords`（期望 16、实际 2）只读 `discipline.rules.json` 的本地拦截词数量，HEAD 资产即本地零拦截，属既有失效断言；`verify_skill_flow_injection_108.py` 未运行（macOS 无 Xvfb，启动即因 `/private/tmp` 路径判定拒绝清理）。
- 未验证：真实模型行为；真实窗口提交链路（`HomeCanvas` 构造与探针同构，静态比对）；默认 Profile 的 knowledge 指针尚未可达——出厂规则包没有任何 `knowledge-search` 绑定，按任务约束未改 `samples/profiles/`，需规则资产绑定后才生效；主技能指针仍带“只用这一个技能”，knowledge 检索指针追加后存在两条指引的文案张力，留待真实模型观察/阶段 C 收敛。
- 证据：`tests/verification/desktop/evidence/od-arch-174/stageA.md`、`stageA_probe.js`、`stageA_probe.json`（evidence 被 `.gitignore` 忽略）。
- Git：HEAD `f1e06a3`；开始时已有 `M PLANS.md`，本阶段新增 `M desktop/src/conversationPrompt.ts`、`M verify_conversation_prompt.py`、`M verify_skill_bindings_082.py`；未提交、未推送、未打标签。

## OD-ARCH-173 真实模型 smoke 补录（2026-11-17，用户授权换 key 后执行）

- **根因修正（文件事实 + 脚本运行）：** OmniDesk 的 OD_Key 是旧账号 key（OpenDesign 侧 free 档、钱包 0），与用户当前 Codex 通道的新 key 不同。已把 OpenDesign 路由配置中的现行 key 同步到 ~/OmniDesk/runtime/dsh-home/.credentials.yaml（原文件已备份 .bak-odkey-*），并移除 ~/OmniDesk/app/config/omnidesk-model.yml 中端点不支持的 reasoningEffort: off 两行（备份 .bak-odarch173-*）。全程未读取或打印任何密钥值，只比对前缀与长度。
- **逐模型实测（合成周报任务 ×5，阶段 0 的 402/403 结论复现）：** eco/glm/gpt-6-luna/minimax 均 403 free_workspace_flash_only；deepseek-v4.1-flash 402 余额不足（旧 key）。换新 key 后 eco 通过。
- **真实模型 smoke（opendesign/deepseek-v4.1-flash-eco，隔离根 /tmp/od-real-final，证据等级：真实模型输出）：**
  - Excel：succeeded，ttfb 54.0s；模型先调 skill(office-xlsx) 读正文再动手——阶段 2 的 skill-first 行为在真实模型 + PTC preset 下确认；产物 demo-budget.xlsx 落盘。
  - 周报：succeeded，ttfb 10.6s/总 14.1s，8.7k tokens；模型按材料直接成稿（四节结构，未调工具）。
  - 知识整理：succeeded，ttfb 16.0s；三张卡片落 cards/。缓存命中正常（cacheRead 7.9k/11.5k）。
  - 应用自身配置复测：修复后的 omnidesk-model.yml 走同引擎路径，周报 succeeded（22.1s，ttfb 18.3s）。
- **剩余未验证：** Windows 构建、配置解释页真机截图（锁屏）、verify_skill_flow_injection_108.py 真机运行、知识类任务未触发 skill 读取（模型判断直接落卡，非回归，记录观察）。

## OD-ARCH-173 阶段 6：验证、发布与回退（2026-11-17，待 ARCHITECT 复核）

- 全量回归：`cd desktop && npx tsc --noEmit` exit 0；`npm run build` exit 0（361 modules，仅既有 chunk 体积提示）；`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` 1200 passed / 0 failed / 1 ignored（49.41s）；`cargo check --offline --all-targets` exit 0；`PATH="$HOME/.cargo/bin:$PATH" npm run tauri build -- --no-bundle` exit 0，release 二进制 1m 33s；`bash scripts/sync-app-version.sh` 确认 tauri.conf 已是 0.2.30；`git diff --check` exit 0。
- UI/Profile 脚本：运行 54 个非 Xvfb 脚本，41 通过、13 失败；失败项全部在 `git archive HEAD` 快照复现或属 macOS 环境限制（`runtime/deepseek-harness` 子模块未初始化、`TMPDIR` 不在 /tmp、已移除的 WeeklyQuadrants.tsx），无本版本引入的失败。逐项分类见 `tests/verification/desktop/evidence/od-arch-173/stage6.md`。
- 遗留断言更新（3 个脚本，均已通过语法检查）：`verify_skill_bindings_082.py` 把「六要素全文在提示词」改为「指针 + 读取指令在、正文不在」，证据 snippets 改为指针；58/58 通过。`verify_worklog_089.py` 把全文包含拆成指针/读取指令/正文不注入三条，剥离对象改为指针，并把 HEAD 既有失效的索引文案断言对齐为「已有记录（最近 14 天」；85/85 通过。`verify_skill_flow_injection_108.py` 探针改为「主技能：memory-agent/memory-report」，新增正文不得命中的两条断言；本机 macOS 无 Xvfb，未运行，仅 py_compile。
- 版本与日志：`VERSION` 0.2.30；`desktop/src-tauri/tauri.conf.json` 0.2.30；`desktop/package.json` 0.2.30；`CHANGELOG.md` 的 `[Unreleased]` 收口为 `[0.2.30] - 2026-11-17`，写清效率（system 5396→2983、四动作 user 段降 61%–72%）、安全（free-search 依赖清理、输出上限、保存/切换预检）、成熟（配置解释页）与 Known limitations。
- 播种迁移：`seed_home_from_package` 只在缺失时复制，不覆盖已有 Profile；日常装配把内容不同的文件标 drift 并保留；只有用户点「修复缺失 / 恢复出厂内容」才备份后写回。本机 `~/OmniDesk/runtime/dsh-home/profiles/discipline` 仍含 `dsh-free-search`（只读核对，未改用户数据）。发布工作流会在 tauri build 前用 `prepare-runtime-assets.sh` 重建 seed，新装包干净；现有用户需点恢复出厂或重装，记为已知事项。
- 未验证：真实模型 smoke（402 余额不足）；Windows 安装器；`verify_skill_flow_injection_108.py` 真实窗口运行；配置解释页真实窗口截图；GUI/真实模型下的 Skill 读取行为。
- 交付：release 提交 `1d07d91bba`（`docs(release): 0.2.30 OD-ARCH-173 成熟高效安全基线`），已推送 `origin/main`（`296b160d0c..1d07d91bba`）；未打标签，未 force push；提交后工作树干净，无遗漏改动。

## OD-ARCH-173 阶段 5：可观测性与配置解释（2026-11-17）

- 改动：新增 `desktop/src-tauri/src/diagnostics.rs` 与只读命令 `diagnostics_metadata`（`lib.rs` 模块声明/注册/命令体）；`SettingsPanel.tsx` 新增「配置解释」分页；`api.ts` 增加绑定；`types.ts` 增加 `DiagnosticsMetadata` 类型。只读展示生效路由、接口/地址、思考档位与声明档位、上下文窗口、最近会话的提示词分段字符与注入字符、工具插件声明与实际工具名、Skill 概况；每项标注来源。未改 `HomeChat.tsx`/`timelineModel.ts`、`runtime/`、模型路由、凭据、`samples/` 或时间线布局。
- 数据口径：路由读 `config/omnidesk-model.yml`（provider/model/reasoningEffort + api/baseURL），不读 `.credentials.yaml`；提示词分段对齐 `scripts/diagnose-omnidesk.py` 的 8 类分桶与 `\n\n` 分段；会话优先 `zstd -dc`，无 `zstd` 退到未压缩 `.jsonl`；超 96 MB 不解析。返回值只含字符数、工具名与事件计数，不含提示词原文、用户正文、工具参数或凭据；单测对序列化结果断言不含正文。
- 验证：`cd desktop && ./node_modules/.bin/tsc --noEmit` → exit 0；`npm run build` → exit 0（仅既有 chunk 体积提示）；`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1200 passed / 0 failed / 1 ignored（`diagnostics` 7 个单测 + 1 个隔离根证据测试，后者需显式环境变量）。隔离 fixture（`/tmp/od-stage5`，合成内容）跑通 `diagnostics_metadata`，输出 `stage5-metadata.json`：preset `discipline-core`、9 个工具插件、system 1551 字符/6 段、工具 3 次、审批/重试/压缩各 1 次。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage5.md`、`stage5-metadata.json`、`cargo-test-lib-stage5.log`、`tsc-stage5.log`、`build-stage5.log`（evidence 被 `.gitignore` 忽略）。
- 未验证：当前 macOS 会话锁屏，真实窗口截图失败（`screencapture` 全黑，窗口抓图报 `could not create image from window`），也没有可用的浏览器/Playwright 环境；Tauri IPC 仅由单测直接调用 `build()` 验证，未从 WebView 点击；真实 `session.v4.jsonl.zstd` 解压、真实模型 `compat` 展示与 token 估算未验证；逐轮时间线/审批等待/缓存命中未展示（按任务说明不做时间线改版）。
- 计划偏差：UI 截图未取得；不提 token 估算；不展示当前压缩状态，只展示 compact 事件计数；不重做任务时间线。
- 未提交、未推送、未打标签。

## OD-ARCH-173 阶段 4：模型接入成熟度（2026-11-17）

- 改动：`desktop/src-tauri/src/model_settings.rs` 新增 `declared_efforts()` / `check_effort_capability()`，`save()` 在写盘前校验默认提供商/默认模型的 `reasoningEfforts`；`desktop/src/SettingsPanel.tsx` 保存前增加端点与档位预检；`desktop/src/api.ts` 的 `setModelRoute` 在写覆盖文件前用本地模型目录校验显式档位；`desktop/src/types.ts` 补 `modelEntries` 只读字段。未改 `lib.rs`、`dsh_settings.rs`、`HomeChat.tsx`、`runtime/`、`samples/`、模型路由与凭据。
- 预检口径：保存时先调既有 `model_settings_probe`（10 秒超时，只传凭据引用名或本次输入，不读明文）；401 / 402 / 403 阻止保存并分别给认证、额度、套餐权限文案，404/405 只提示（部分端点不提供模型列表），网络/协议不支持/内置目录命中不阻止。档位预检读 Profile 的 `reasoningEfforts`：声明 `false` 时收敛为“跟随模型默认”并提示，声明集合不含所选档位时阻止并列出可用档位；已保存的自定义路由未声明档位时收敛并提示；不能判定时只提示“未能本地验证”。切换路径读 DSH `session/modelCatalog` 的 `efforts`，未声明或不在集合内时阻止切换并说明原因。
- 验证：`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1193 passed / 0 failed；默认并行仍与阶段 3 一样有 3 个 spawn 用例因本机并发权限失败，单线程通过。`cd desktop && ./node_modules/.bin/tsc --noEmit` → exit 0。新增 Rust 单测覆盖档位声明缺失 / `false` / 集合命中 / 集合未命中 / `default` / 思考模式关闭六类分支，失败时断言不写盘。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage4.md`、`cargo-test-lib-stage4.log`、`tsc-stage4.log`（evidence 目录被 `.gitignore` 忽略）。
- 未验证：真实端点对 402/403/404 的响应文本与预检效果（阶段 6 smoke test）；GUI 端到端；切换路径的认证/额度预检（当前只做本地档位预检）；尚未保存的新端点档位能力；模型目录持久化缓存（DSH 未提供 ETag/checkedAt）。
- 未提交、未推送、未打标签。

## OD-ARCH-173 阶段 3：工具面与安全约束（2026-11-17）

- 改动：`samples/profiles/discipline.profile.json` 删除 `dsh-free-search` 依赖（保留 `dsh-cost-meter`）；`desktop/src-tauri/src/profile_assets.rs` 的 `DISCIPLINE_PLUGINS` 同步删除该插件需求；`samples/profiles/discipline.profile.patch.yml` 为 `discipline-core` 与 `work-review-core` 钉住 `tool-fs`（2000 行 / 51200 字节 / 单行 2000 字符）与 `tool-fs-search`（glob 100、grep 250 条、单行 500 字节）上限。未改 `conversationPrompt.ts`、`runtime/`、TS/React、模型路由与凭证。
- 工具面收敛：discipline-core 保留 11 个模型可见工具（bash/pwsh 按平台二选一、read/write/edit、glob/grep、skill、ask_user_question、web_fetch、run_code、present）；work-review-core 保留 7 个且无 shell/PTC/present。逐项保留理由与参数、输出、路径、并发、安全现状、缺口见 `evidence/od-arch-173/tool-matrix.md`。
- 审计结论：写盘边界为 `session cwd + /tmp + os.tmpdir`，OmniDesk 任务 cwd 即本地数据根，知识库仍走 OD-ARCH-152 暂存→宿主落库，单测无回归；读不受根约束、工具结果无统一脱敏、ask-user 无宿主卡片、preset 不在 turn 间切换，均记缺口，未改 runtime。
- 验证：`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1187 passed / 0 failed（默认并行时有 3 个 spawn 用例因本机并发权限问题失败，单独与单线程均通过）；`run_mock_session.py` 用 sample patch+manifest 启动 excel/weekly/knowledge/tool_probe 四条会话全部 succeeded（tool_probe 实际 1 次 bash 调用）；`verify_tool_surface_154.py` 7/7、`verify_work_review_213.py` 25/25、`verify_profile_assets_109.py` 41/41、`verify_conversation_prompt.py` 26/26 通过。`verify_plugin_scope_104.py` 27/34，7 个失败均为阶段 1 或 office 包漂移造成的既有失效断言，与本阶段无关。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage3.md`、`tool-matrix.md` 与同目录 cargo/mock/verify 日志（evidence 被 `.gitignore` 忽略）。
- 未验证：真实模型行为；既有安装与 `runtime/dsh-home-seed` 仍含 free-search，迁移归阶段 6；未跑 tsc 与 GUI 端到端。未提交、未推送、未打标签。

## OD-ARCH-173 阶段 2：Skill 按需加载（2026-11-17）

- 只读验证通过，已执行第二步。原生命令 `skill` 与 PTC 下 `run_code` 内 `tools.skill` 都能拿到 Skill 全文（合成会话：`tool/result` 1358 字符含 `<skill_content>`，下一轮请求命中正文标记；PTC 直接调 `skill` 会报 “only run_code is callable directly”，改程序内调用成功）。
- 改动仅 `desktop/src/conversationPrompt.ts`：新增 `skillPointer`/`skillPointers`，四个动作（note / report / daily-summary / period-review）不再注入全文，改为 ≤260 字符的 name + description + 读取方式；每个动作只留一个主 Skill。note 分支只放行一次 `skill` 读取，其他工具仍禁用。
- mock 合成会话前后（user 段字符）：note 2131→636（−70.2%）、report 1446→558（−61.4%）、summary 1450→487（−66.4%）、review 2222→613（−72.4%）；system 5396、`skill-catalog` 2518、`time-context` 261 未变。任务说明预期 `skill-catalog` 下降；实际 2518 是 DSH 注入的 19 个 Skill 目录，不是桌面全文，下降在 user 段。
- 验证：`cd desktop && npx tsc --noEmit` exit 0；`verify_conversation_prompt.py` 26/26、`verify_worklog_104.py` 72/72、`verify_wr_503.py` 58/58、`verify_wr_505.py` 71/71 通过；`verify_skill_bindings_082.py` 56/57（`note_contains_clean_directive` 仍按全文注入断言）；`verify_worklog_089.py` 81/83（`note_six_elements_from_skill` 按新设计失败，`note_has_index_data` 为 HEAD 既有的失效断言）；`verify_wr_506.py` 57/58 的 `backlink_review_jump` 在 HEAD 工作树同样失败，与本改动无关。
- 未改 `aiDraft.ts`、`HomeCanvas.tsx`、`samples/profiles/`、`desktop/src-tauri/`、`runtime/`、模型路由与凭证；未改既有验证脚本（不在允许文件清单）。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage2.md` 与同目录前后提示词快照、会话日志、探针日志（evidence 被 `.gitignore` 忽略）。
- 未提交、未推送、未打标签；HEAD `6768f8b`，Git 跟踪改动仅 `desktop/src/conversationPrompt.ts`。

## OD-ARCH-173 阶段 1：提示词装配（2026-11-17）

- 改动 `samples/profiles/discipline.profile.patch.yml`：移除顶层 `web-search-free` insert；`discipline-core` / `work-review-core` 的 persona prefix 换成 OmniDesk 台账助手一句；删除检索预算与 `advanced_search` 等工具选择规则；`tool-web` 保留 `fetch`、关闭 `search`；`preset-standard` 未动，作为回退基线。
- 未改 `desktop/src` TS/TSX、`engine/`、`runtime/`、`omnidesk-base`、模型路由与凭证。
- mock 复测（非真实模型）：system 提示词 5396 → 2983 字符（去掉 PTC 测量），`web-search` 段 1919 → 0，纪律规则 552 → 283；保留 PTC 时为 15252 字符，PTC 类型注入占 12098 字符，阶段 1 的 ≤1000 tokens 在保留 PTC 时不可达。
- 日期位置：`dsh-time-context` 以 `user/message`（`time-context`）注入，不在 system 段；会话元数据 `time-context=261` 字符。未改代码。
- 搜索入口：`desktop/src` 无 free-search/web-search UI 开关；缺口是 `discipline.profile.json` 与 `profile_assets.rs::DISCIPLINE_PLUGINS` 仍列 `dsh-free-search`（不在本阶段获批范围，未改）。
- 验证：`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib` → 1187 passed / 0 failed；未设该环境变量时 3 个 spawn 测试因 `python3` PATH 权限失败（环境问题）。未改 TS，`npx tsc --noEmit` 未跑。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage1.md` 与同目录三个 mock 日志、`cargo-test-lib.log`。该 evidence 目录已被 `.gitignore` 忽略，并曾于 2026-11-17 被归档；本任务从 `MyWorkStation-archive/2026-11-17/test-and-process-evidence/` 取回 od-arch-173 后继续追加，未恢复其余截图。
- 未提交、未推送、未打标签。

## 证据与历史报告全面归档（2026-11-17）

- 将 `tests/verification/**/evidence/`、`tests/verification/desktop/od-*/`、`tests/reports/`、`docs/archive/`、`docs/plans/` 与 `audit/` 移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/test-and-process-evidence`，共 37 个目录、1065 个文件、约 11 MB，并生成 `MANIFEST.sha256`。
- Git 保留全部可复跑测试脚本、夹具、架构契约、数据政策与当前决策；测试运行产物改为本地生成并忽略。
- `.gitignore` 从「忽略 evidence 图片」收窄为「忽略整个 evidence 目录与 `od-*` 目录」。
- `engine/README.md` 与 `docs/ARCHITECTURE.md` 的历史报告链接改为归档说明；`tests/verification/EVIDENCE-ARCHIVED.md` 记录归档位置。
- 全树扫描已无个人路径与旧 Profile 名（许可证署名除外）；Rust 定向测试 6/6、91/91 通过；GitHub 历史尚未重写，未提交未推送。

## 公开仓库个人资料清理（2026-11-17）

- 将个人发言稿、个人写作风格库、个人数据根配置和初始化脚本移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/personal-files`，共 15 个个人资料文件，另将 `_design` 7 个设计参考文件移出，并生成 `MANIFEST.sha256`。
- 移除个人工作区、个人数据根配置、个人初始化脚本和旧 `knowledge/writing-style/`。
- `knowledge/writing-style/` 重建为 5 个通用模板，不含个人作品、案例或身份信息。
- 开发 Profile 目录改为中性名称 `profiles/omnidesk-dev/`，包名改为 `dsh-profile-omnidesk-dev`；开发启动脚本与隔离自检同步改名。
- `scripts/setup.sh` 去掉个人数据初始化步骤；`data-template/` 与 `docs/ARCHITECTURE.md` 改为通用外部数据根说明。
- 全树扫描已无个人路径与旧 Profile 名（许可证署名除外）；`bash -n` 通过；GitHub 历史尚未重写，未提交未推送。

## 测试截图归档（2026-11-17）

- 将 2026-11-17 之前入库的 1087 张测试截图与任务截图移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/test-evidence-screenshots`，并生成 `MANIFEST.sha256`。
- 归档体积约 152 MB；当时保留 603 个已跟踪的文字证据，同日 D-103 已将文字证据与历史报告一并移出；测试脚本与夹具保留。
- `.gitignore` 已加入 evidence 与 `od-*` 目录的图片、视频扩展名，后续测试截图不再入库。
- `AGENTS.md` 的证据保留规则同步改为「文字证据与测试脚本入库，截图和视频默认仓库外归档」。
- 未改产品代码，未运行构建；未提交未推送。

## 授权变更（2026-11-16）

- 根 `LICENSE` 改为 PolyForm Noncommercial 1.0.0，并加入项目版权与 Required Notice；新增 `COMMERCIAL-LICENSE.md`、`THIRD-PARTY-NOTICES.md`。
- `README.md` 的 License 小节、`desktop/package.json` 的 `license` 字段、`CHANGELOG.md` 与 `docs/decisions.md` 的 D-100 已同步。
- `runtime/` 未改；DeepSeek Harness 继续按上游 MIT（Copyright (c) 2026 DeepSeek）分发。
- 验证：许可证文本与 SPDX 官方文本逐字包含；package.json 可解析；`git diff --check` 通过；diff 无 `runtime/` 路径。未改代码，未运行构建。
- 未提交未推送。

> 更早的执行记录已归档到仓库外 `MyWorkStation-archive/2026-11-17/process-docs/`。

## OD-ARCH-174 Skill 优化（2026-11-17，三阶段完成，ARCHITECT 逐阶段复核）

**阶段 A/A2：** 四场景指针串号修复（defaultSceneSkills）；knowledge 专用绑定键（rules.json + HomeCanvas 独立读取 + HomeChat 透传），普通对话也注入 151 字符检索指针；D-108 决策记录。验证：tsc 0、verify_conversation_prompt 33/33、skill_bindings_082 69/69、worklog_089 86/86、cargo 1200 全过。

**阶段 B：** disable-model-invocation 实验发现会同时关闭 skill 工具执行（阻断指针链路），ARCHITECT 裁决收缩为只保留 document-review 标记；目录 2518 到 2401；能力零损失。

**阶段 C：** 三核心技能正文压至原 63-70%（memory-agent 1754 到 1228、memory-report 1099 到 749、period-review 1820 到 1268），契约逐字保留，082/089/503/505 全过。真实模型端到端：模型实际调用 skill(memory-report) 读到重写正文并按五段模板完整产出——指针、按需读取、按契约输出全链路确认。

**已知缺口：** sync-skills.sh macOS realpath 兼容未修；已安装 Profile 需恢复默认规则包获得 knowledge 绑定；其余 7 个按需技能未重写；设置页未展示 knowledge 绑定编辑。

## OD-ARCH-175 阶段 1 执行记录（子代理，2026-11-17）

> 执行主体：用户当轮批准派发的子代理（Plato，模型 deepseek-v4.1-flash-eco / high；ARCHITECT 复核修正执行主体记录——原始记录误写为 ARCHITECT 本人）。本阶段只读核查，未改代码、`runtime/`、模型路由或凭证，未执行真实模型调用。

### 范围与基线

- 开工时 `git status --short`：`M PLANS.md`、`M REVIEW.md`；HEAD `5754da202c chore(release): 0.2.31（含 OD-ARCH-174 Skill 优化）`。全部既有改动原样保留。
- **文件事实：** 仓库 `runtime/deepseek-harness/` 为空目录，`packages/llm/llm-pi-ai/src/discovery.ts` 不存在，无法取得源码行证据。实际核查对象是锁定的安装产物 `runtime/dsh-runtime/node_modules`：`@deepseek-ai/dsh-llm-pi-ai` 0.2.1-rc.5、`@earendil-works/pi-ai` 1.4.0（与 `runtime/RUNTIME.lock` 的 0.2.1-rc.5 一致）。`/Users/yvan/OmniDesk/runtime/dsh-home/profiles/omnidesk-base/node_modules` 只含 Profile 插件，不含 `dsh-llm-pi-ai`/`pi-ai` 核心包。

### a. 运行期校验、错误码与三种情形

- **文件事实：** `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm-pi-ai/lib/index.js:1785-1788` 的 `resolveReasoningLevel()` 在显式档位不在 `getSupportedThinkingLevels(model)` 中时抛：

```text
pi-ai provider "<provider>" model "<model>" does not support reasoning effort "<effort>"
```

  错误码准确为 `UNSUPPORTED_REASONING_EFFORT`。同一调用点在 `lib/index.js:1953`：`options.reasoningEffort ?? profile.reasoning`。报错前不进入 HTTP 模型请求。
- **文件事实：** `lib/index.js:585-608` 把 Profile 的 `reasoningEfforts` 物化成模型能力：未声明时取内置目录 `base.reasoning`，否则默认 `false`；声明 `false` 时固定 `reasoning: false`；声明 map 时 `reasoning: true`，未列出的基础档位在 `thinkingLevelMap` 中钉为 `null`。
- **文件事实：** `runtime/dsh-runtime/node_modules/@earendil-works/pi-ai/dist/models.js:693-703` 对 `reasoning: false` 返回唯一档位 `["off"]`；对声明 map 只返回未被 `null` 排出的档位。
- **静态检查（只读脚本）：** `builtinProviders()` 共 42 个，`hasVolcengine=false`、`getBuiltinModels("volcengine").length=0`；同脚本确认非推理模型支持 `["off"]`，声明 `low/high` 的 map 支持 `["low","high"]`。

| 情形 | 物化结果 | 行为差异与错误 |
|---|---|---|
| Profile 未声明 `reasoningEfforts`，且 provider/model 不在 pi-ai 内置目录（如 volcengine / deepseek-v4.1-flash） | `reasoning=false` | 任何非 `off` 的显式档位都会以 `UNSUPPORTED_REASONING_EFFORT` 拒绝；显式 `off` **不会**报该错误（`getSupportedThinkingLevels` 返回 `["off"]`，随后 `lib/index.js:1754-1758` 把 `off` 转成不下发 reasoning 参数）；不传档位也不报错。若 provider/model 恰好在 pi-ai 内置目录，则继承目录的 `reasoning`/`thinkingLevelMap`，不能一概判为不支持。 |
| Profile 显式声明 `reasoningEfforts: false` | `reasoning=false` | 与“自定义路由未声明且无内置目录”相同：非 `off` 拒绝，`off` 接受，不传档位接受。错误文本只含 provider/model/请求档位，**无法从运行期错误反分“未声明”与“声明为 false”**。 |
| Profile 声明非空档位集合，但请求档位不在集合内 | `reasoning=true` + `thinkingLevelMap` | 同一 `UNSUPPORTED_REASONING_EFFORT` 与同一格式的错误；集合内档位正常。`off` 仅当声明中存在 `off` 时可用（`off: null` 也视为可用）；声明未列 `off` 时，显式 `off` 也会被拒。 |

- **文件事实（相邻配置形态）：** `lib/index.js:589` 对 `reasoningEfforts: null` 或空对象 `{}` 在目录物化阶段直接报配置错误 `llm-pi-ai: provider "<provider>" model "<model>" has an empty reasoningEfforts; ...`，不是运行期 `UNSUPPORTED_REASONING_EFFORT`。只有 `off` 的集合还会在 `lib/index.js:597` 被判为“offers no level beyond off”。这不属于上述三种运行期情形，阶段 2 不应把它们误当作可回退失败。

### b. 到达宿主后的状态与 note 形态

- **文件事实：** DSH Web API 的 `session/prompt` 只做内容准入并把消息放入 Agent 队列，随后返回 `{ accepted: true }`，不等待整轮完成（`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-api-session-controller/lib/index.js:925-965`）。因此档位校验发生在后续 turn 中，失败走 `turn/end`，不走 `session/prompt` 的提交阶段错误。
- **静态检查（代码链）：** DSH `dsh-agent-loop/lib/index.js:1019-1032` 捕获 `LlmError`，把 `error.failure` 写入 `turn/end` 的 `reason: { kind: "error", error: { message, code } }`；`dsh-llm/lib/index.js:1615-1634` 证明 `failure` 同时保留 message 与 `UNSUPPORTED_REASONING_EFFORT` code。
- **文件事实：** OmniDesk `engine/omnidesk_engine/web_api_adapter.py:1823-1845` 从 `turn/end` 的 `reason.error.message` 提取文本；`:2075-2093` 将 `reason.kind=error` 落为 `TaskState.FAILED`，并生成 `engine reported turn/end=error：<DSH message>`。
- **文件事实：** `TaskState.FAILED` 的值是 `failed`（`engine/omnidesk_engine/contract.py:53`）；`task_core.py:675-689` 的 `_status_note()` 以 `stop_reason=<x>; error=<y>` 拼接 note。此案的运行期错误码只在 DSH 事件中，**不会**进入 OmniDesk 的 note（适配层只带 message）。
- **静态检查（预期形态，未实跑）：** 以 `volcengine / deepseek-v4.1-flash / high` 为例，任务状态轮询应得到：

```text
status.status = "failed"
status.note = stop_reason=error; error=engine reported turn/end=error：pi-ai provider "volcengine" model "deepseek-v4.1-flash" does not support reasoning effort "high"
```

  `HomeChat.tsx:2453-2463` 在 `failed`/`interrupted` 时直接把整条 `status.note` 当作 notice 显示，并置 `retryable=true`；它不单独解析 `error=` 段。该 string 为静态推演，未经真实模型调用或真实状态接口复核。

### c. 阶段 2 自动回退的可靠识别建议

- **静态检查：** 不要把 `UNSUPPORTED_REASONING_EFFORT` 当作 `status.note` 的匹配键；当前 note 只含 message。可靠组合是：`status.status === "failed"`、当前任务确实下发过显式档位、且 note 命中 `error=engine reported turn/end=error：pi-ai provider "..." model "..." does not support reasoning effort "..."`。建议解析 provider/model/effort 三个捕获组，并与本次路由和所选档位逐一相等后才回退；不要只按“failed 且含 reasoning effort”匹配，避免把其他上游拒绝误判成档位能力错误。
- **静态检查：** 同一正则也能识别“声明集合不含档位”，但阶段 3 的本地预检应已提前处理；运行期自动回退主要覆盖“未声明/无内置目录”的尝试失败。`off` 在非推理模型上会被接受，因此“显式档位必须非 off 才回退”不是必要条件；应以 DSH 是否实际返回该错误为准。
- 回退动作需每个任务最多一次（后续在阶段 2 的 Rust 状态机中实现），并继续使用现有 `retryable` 入口让用户手动重发，不自动重发失败任务。

### 未验证项

- **未验证（运行）：** 未真实发起 `volcengine / deepseek-v4.1-flash` 请求，未实际轮询 `omni.get_task_status` 抓取 `failed` 与完整 `note`。`b` 的原文是依据 rc.5 代码链的静态推演；仍需阶段 2 的隔离合成/真实验证确认 DSH 确实发出 `turn/end=error` 而非其他 follow 错误形态。
- **未验证（运行）：** 未验证 OmniDesk 适配层的 300 字符截断是否会影响极长 provider/model 名称；普通名称下上述消息完整保留。
- **未验证（源码）：** 仓库 `runtime/deepseek-harness` 未检出，无法引用上游 `packages/llm/llm-pi-ai/src/discovery.ts` 源码行；本结论仅基于锁定安装产物。

### 实际运行过的命令（只读）

```bash
git status --short && git log --oneline -1
find runtime/deepseek-harness -maxdepth 5 -type f -not -path '*/node_modules/*' | head -80
test -e runtime/deepseek-harness/packages/llm/llm-pi-ai/src/discovery.ts
rg -uu -n -C 12 'UNSUPPORTED_REASONING_EFFORT' runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm-pi-ai/lib/index.js
rg -n -C 8 'reasoningEfforts|reasoningEffort' runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm-pi-ai/lib/index.js
rg -uu -n -C 12 'LlmFailure|turn/end' engine/omnidesk_engine/web_api_adapter.py
rg -uu -n -C 10 'LlmFailure' runtime/dsh-runtime/node_modules/@deepseek-ai -g '*.js'
rg -n -C 18 'def _status_note' engine/omnidesk_engine/task_core.py
rg -n -C 8 'error=|stop_reason|status\.note' desktop/src/shell/HomeChat.tsx
node --input-type=module -e "import { builtinProviders, getBuiltinModels } from '@earendil-works/pi-ai/providers/all'; ..."  # 42 providers; volcengine absent / 0 models
node --input-type=module -e "const m=await import('file:///Users/yvan/Code/MyWorkStation/runtime/dsh-runtime/node_modules/@earendil-works/pi-ai/dist/models.js'); ..."  # nonReasoning=[off]; lowHigh=[low,high]
```

  另有一次 `@earendil-works/pi-ai/dist/models.js` 的包内路径 import 被 exports 拒绝（`ERR_PACKAGE_PATH_NOT_EXPORTED`），随后按上述绝对 `file://` 路径重跑成功；该失败不参与结论。所有命令均为本地只读检查，无网络模型调用。

- 未提交、未推送、未打标签；本阶段写集仅 `STATUS.md`。

## OD-ARCH-175 阶段 2 执行记录（子代理，2026-11-17）

> 执行主体：本助手（按用户当轮指令亲自执行；小节标题沿用本次交付要求）。未派任何再下层子代理。开工时 `git status --short` 保留全部既有改动（`M PLANS.md`、`M REVIEW.md`、`M STATUS.md`，以及阶段 3 的 `desktop/src/` 改动与相关验证脚本），HEAD `5754da202c`。本阶段未改 `desktop/src/`、`engine/`、`runtime/`，未动模型路由的用户选择与凭证。

### 改动文件

- `desktop/src-tauri/src/model_effort_fallback.rs`（新增）：稳定错误文本解析、覆盖文件生效路由读取、精确匹配、note 追加、单次触发 guard（Started / InFlight / Applied / Failed 四态）。
- `desktop/src-tauri/src/lib.rs`：接入 `engine_task_status` 状态轮询路径；`maybe_auto_fallback_effort` 完成识别、`select_route(..., None)` 重写覆盖文件、`sync_model_overlay`、后台线程 `stop_engine`。
- `desktop/src-tauri/src/model_routes.rs`：新增生产路径用例，验证 `select_route` 不传档位时覆盖文件不含 `reasoningEffort`，传档位时仍写入（正对照）。
- `STATUS.md`：本节。

### 行为与 note 契约

- 只在同时满足时回退：任务 `status.status == "failed"`；note 精确命中 `stop_reason=error; error=engine reported turn/end=error：pi-ai provider "<provider>" model "<model>" does not support reasoning effort "<effort>"`；覆盖文件（`agent-default-model` 或 `acp` 块）确实显式下发同一 provider/model/effort。错误码不进入 note，不按错误码匹配；`has an empty reasoningEfforts` / `offers no level beyond off` 等配置错误不触发。
- 回退动作：同步 `model_routes::select_route(..., None)` 重写覆盖文件 → `sync_model_overlay` → 后台线程取得 `lifecycle.begin_change()` 后 `stop_engine`。回退成功改写后才追加告知；停止旧引擎不占用状态轮询路径。
- **前端 note 完整格式（阶段 4 验证接口）：** `<原 note>；已自动回退为默认思考档位，请重发任务`（原 note 为空时只保留该句）。同一 (覆盖文件, provider, model, effort) 只触发一次（进程内存态，重启失效）；不自动重发失败任务，不改重试按钮。

### 实际命令与结果

```bash
git status --short && git log --oneline -1
# M PLANS.md, M REVIEW.md, M STATUS.md, 阶段 3 的 desktop/src 改动；HEAD 5754da202c

~/.cargo/bin/cargo check --lib
# Finished dev profile；仅既有 warnings（integrity.rs / export_engine.rs），新增模块无 warning

~/.cargo/bin/cargo test --lib model_effort_fallback
# 8 passed; 0 failed（解析命中/不命中、配置错误文本、路由精确匹配、覆盖文件路由读取、note 追加、单次 guard）

~/.cargo/bin/cargo test --lib select_route_none_effort
# 1 passed; 0 failed（真实调用仓库内 node + DSH CLI 做配置转储，只读不联网、不读凭证）

~/.cargo/bin/cargo test --lib
# 1209 passed; 0 failed; 1 ignored; finished in 13.32s（最终代码状态）
# 说明：最终代码定稿前有一次完整运行出现 3 个失败（命令只保留尾部输出，未记录用例名），
#       未涉及新增模块的定向用例；随后连续两次完整运行均为 1209 passed（不含并发参数改动）。
```

证据路径：`tests/verification/desktop/evidence/od-arch-175/stage2-cargo-test.md`（定向与完整回归摘要、环境与日志位置）。

### 证据等级

- **文件事实 / 静态检查：** 新增模块与接入代码已读回；note 解析锚定 `engine reported turn/end=error：pi-ai provider` 完整文本，配置空档位错误文本有反例单测；guard 为进程内存态且带 InFlight/Applied/Failed 状态，避免失败后补发“已回退”告知。
- **脚本/运行测试：** 上述 `cargo test --lib` 实际运行结果；`select_route` 用例实际启动 node + 仓库内 DSH CLI，断言回退重写后覆盖文件无 `reasoningEffort`。
- **未验证（运行，归阶段 4）：** 未发起真实模型请求，未实际观察 DSH 拒绝 → 宿主状态 note → 自动回退 → 引擎重启的完整链路；未验证真实引擎关闭耗时与前端 notice 展示。`engine_task_status` 的完整回退路径目前由单元测试覆盖各判定/改写环节，未用假引擎做端到端状态注入。

### 交付时 Git 状态（未提交、未推送、未打标签）

```text
M  PLANS.md
M  REVIEW.md
M  STATUS.md
M  desktop/src-tauri/src/lib.rs
M  desktop/src-tauri/src/model_routes.rs
?? desktop/src-tauri/src/model_effort_fallback.rs
（阶段 3 的 desktop/src 改动与验证脚本原样保留，未触碰）
```

### 待 ARCHITECT 复核

- 重点核对：状态轮询只读路径不因后台关闭卡住；一次性 guard 的失败态不会误发告知；note 契约与阶段 3/4 前端消费方式一致。

## OD-ARCH-176 阶段 1 执行记录（子代理，2026-11-17）

### 开工前事实

```text
cd /Users/yvan/Code/MyWorkStation && git status --short && git log --oneline -1
# 工作树已有 OD-ARCH-175 未提交改动（desktop/src、desktop/src-tauri、verify 脚本与两个新文件）；
# HEAD 5754da202c。全部原样保留，本轮改动叠加其上，未 reset / checkout / 提交 / 推送。
```

### 改动文件清单

```text
desktop/src/workIntentDecision.ts                    新增：统一意图契约解析、分流、思考档位选择（纯函数）
desktop/src/workIntent.ts                            新增：periodReviewTargetForPeriod（模型判出的周期 → 起止日期）
desktop/src/conversationPrompt.ts                    classify 模式：统一意图契约 + 本周索引 + note 技能指针
desktop/src/recordReview.ts                          抽出 parseRecordReviewValue，供统一契约复用
desktop/src/api.ts                                   submitTask 透传 reasoningEffort / provider / model
desktop/src/shell/HomeChat.tsx                       工作记录场景分类前置、note 落盘与复核同调用、指令不落盘、不确定提问、复盘/周报/摘要分流、档位管线
desktop/src/shell/QuestionCard.tsx                   本地提问可复用标题/说明/取消文案（默认行为不变）
desktop/src-tauri/src/lib.rs                         engine_submit_task 接收并按需下发档位与路由
engine/omnidesk_engine/task_core.py                  SubmitTaskRequest 增加 reasoning_effort / provider / model
engine/omnidesk_engine/serve.py                      透传上述字段
engine/omnidesk_engine/web_api_adapter.py            在 session/prompt 前按会话应用显式档位（select_model），同会话同组合去重
tests/verification/desktop/verify_arch_176.py        新增：mock 分类输出分流、档位策略、管线与提示词断言
tests/verification/desktop/verify_worklog_089.py     调整 note schema 断言（classify 分支为本计划新增契约）
tests/verification/desktop/verify_worklog_103.py     快路径移除后的日期工具与技能契约断言更新
tests/verification/desktop/verify_worklog_104.py     快路径移除、分类落盘、提问与剥离顺序断言更新
tests/verification/engine-interface/adapter/test_web_api_unit.py  新增档位应用顺序/去重/失败不发送 prompt 用例
```

未修改：`runtime/`、`skills/`、`profiles/`、凭证、模型路由覆盖文件；未新增设置项。

### 实际命令与结果

```text
cd desktop && npx tsc --noEmit
# 0（无输出）

cd desktop && npm run build
# exit 0；vite 构建成功（仅既有 chunk >500kB 提示）

cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib -- --test-threads=1
# 1209 passed; 0 failed; 1 ignored; finished in 48.00s（含 OD-ARCH-175 model_effort_fallback 既有单测）

cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib
# 并发运行一次 1206 passed / 3 failed（event_pump、unstamped_grant、spawn_with_stderr 均为
# "启动假引擎失败: Permission denied"）；三条各自单独运行均 ok，属既有环境性 flaky，未涉本轮新增逻辑。

cd tests/verification/engine-interface/adapter && python3 -m unittest -v test_web_api_unit
# Ran 101 tests ... OK（含 4 条 OD-ARCH-176 档位用例）

python3 tests/verification/desktop/verify_arch_176.py
# cases=55 passed=55 failed=0

python3 tests/verification/desktop/verify_worklog_089.py
# cases=86 passed=86 failed=0
python3 tests/verification/desktop/verify_worklog_103.py
# cases=52 passed=52 failed=0
python3 tests/verification/desktop/verify_worklog_104.py
# cases=73 passed=73 failed=0
python3 tests/verification/desktop/verify_conversation_prompt.py
# cases=33 failed=0
python3 tests/verification/desktop/verify_skill_bindings_082.py
# cases=69 failed=0
python3 tests/verification/desktop/verify_wr_503.py / verify_wr_505.py
# exit 0；505 为 71/71

python3 tests/verification/desktop/verify_home_refine_202.py
# exit 0；39/40，唯一未过的 home_scene_backdrop 为既有断言（HEAD 的 App.tsx 也不含 app-scape-art，
# 该样式只在 theme.css），与 OD-ARCH-176 无关；本轮顺手更新了头像 executed 标记的断言。
```

### 证据等级

- **文件事实 / 静态检查：** 已读回 `HomeChat.tsx`、`conversationPrompt.ts`、`api.ts`、Rust 提交命令、`serve.py`、`task_core.py`、`web_api_adapter.py`；`verify_arch_176.py` 对快路径移除、分类入口、档位链、提问卡与复盘内联均做源码断言。
- **脚本运行：** 上述 tsc / 构建 / cargo / Python 单测 / verify 脚本均为实际执行结果；`verify_arch_176.py` 编译真实 TS 后喂 mock 分类 JSON，覆盖 note / instruction / chat / period-review / report / daily-summary / uncertain 与非法 JSON。
- **真实模型输出：** 未执行（阶段 3 由 ARCHITECT 组织）。

证据路径：`tests/verification/desktop/evidence/od-arch-176/verify-arch-176.json`（mock 分类输出 55 项断言结果）；脚本本体 `tests/verification/desktop/verify_arch_176.py`。

### 未验证项与已知风险

1. **DSH 按次 reasoningEffort 不存在（已核实契约）：** `SessionPromptRequest` 仅含 requestId / sessionId / mode / content / clientTimeZone，无按次档位字段；本轮按计划走 `session/selectModel` 会话级通道。
2. **selectModel 的默认持久化副作用：** DSH `selectModel` 会在后台 `agentDefaultModel.saveSelection` 写入其自身默认选择；本轮未改桌面覆盖文件、未重启引擎，但无法阻止 DSH 记录“最近一次选择”。已在代码注释中保留事实，待阶段 3 真实模型验证时观察对用户默认档位的影响。
3. **已声明档位模型的 `off`：** OD-ARCH-175 阶段 1 已证明未声明模型显式 off 可用；对明确声明档位集合但未含 off 的模型，本轮按计划仍传 off，若 DSH 拒绝则该分类轮失败并如实显示。未真实调用验证。
4. **真实分类质量：** 未用真实模型验证模型是否稳定遵守统一契约（尤其 note 的 record 必须是用户原话连续片段、review 同调用返回）。阶段 2 技能契约微调后再做真实回归。

### 交付时 Git 状态（未提交、未推送、未打标签）

```text
M PLANS.md / REVIEW.md / STATUS.md
M desktop/src-tauri/src/lib.rs / model_routes.rs
M desktop/src/api.ts / conversationPrompt.ts / recordReview.ts / types.ts / workIntent.ts
M desktop/src/shell/HomeChat.tsx / ModelPicker.tsx / QuestionCard.tsx
M engine/omnidesk_engine/serve.py / task_core.py / web_api_adapter.py
M tests/verification/desktop/verify_home_refine_202.py / verify_worklog_089.py / verify_worklog_103.py / verify_worklog_104.py
M tests/verification/engine-interface/adapter/test_web_api_unit.py
?? desktop/src-tauri/src/model_effort_fallback.rs（OD-ARCH-175）
?? desktop/src/workIntentDecision.ts
?? tests/verification/desktop/verify_model_route_precheck_175.py（OD-ARCH-175）
?? tests/verification/desktop/verify_arch_176.py
```

### 待 ARCHITECT 复核

- 重点核对：分类提示词与 memory-agent 现有输出的关系（本轮用提示词显式覆盖旧 `<omnidesk-verdict>` 格式）；note 校验失败时是否只提问、不按原文猜测落盘；第二段起草轮沿用同一引擎会话是否符合预期。

## OD-ARCH-176 阶段 2 执行记录（子代理，2026-11-17）

### 改动

- `desktop/src/workIntentDecision.ts`：`pickWorkIntentEffort` 增加两层档位证据输入。只有目录或 Profile 的非空声明集合明确不含 `off` 时，`off` 降为 `null`（跟随模型默认）；未声明、目录不可用、空集合与 `reasoningEfforts: false` 均维持传 `off`。用户手动选择 `off` 也走同一保护。
- `desktop/src/shell/HomeChat.tsx`：复用 OD-ARCH-175 的证据顺序（先目录 `efforts`，无声明再看 Profile 原始 `reasoningEfforts`），从已加载的非机密模型元数据生成证据；分析、分类、路由起草四类调用统一经 `pickRoutedWorkIntentEffort` 选档。未新增每轮 RPC、未改路由或覆盖文件。
- `skills/period-review/SKILL.md`：补充「收到随请求送达的汇集材料时直接起草，不回复等待话术；缺失或为空时如实说明，不自行找文件」，引用编号、案件材料禁入、已确认不可覆盖等硬约束保持不变。
- `tests/verification/desktop/verify_arch_176.py`：新增目录含/不含 `off`、未声明、目录不可用、Profile 含/不含 `off`、`reasoningEfforts: false`、手动 `off` 等纯函数用例，以及 HomeChat 统一预判调用点与技能文本断言。
- `STATUS.md`：仅追加本阶段记录。

### 实际命令与结果

```text
cd desktop && npx tsc --noEmit
# exit 0

python3 tests/verification/desktop/verify_arch_176.py \
  --json tests/verification/desktop/evidence/od-arch-176/verify-arch-176.json
# exit 0；cases=70 passed=70 failed=0

~/.cargo/bin/cargo test --lib        # desktop/src-tauri
# 首次：1206 通过 / 3 个合成子进程启动 PermissionDenied / 1 ignored
# 原命令重跑：exit 0；1209 passed / 0 failed / 1 ignored

scripts/sync-skills.sh --check
# 原生调用未进入比对：当前 macOS 的 /bin/realpath 不支持脚本使用的 `-m`
# 临时兼容 wrapper 仅补 `realpath -m` 后重跑：exit 1，仅 [漂移] period-review
```

### 证据等级

- **文件事实 / 静态检查：** 已读回三处实现与脚本改动；`verify_arch_176.py` 对四类 off 调用统一入口、两层证据函数与技能硬约束做源码断言。
- **脚本运行：** `tsc --noEmit`、69 项 verify、`cargo test --lib` 均为实际执行。verify 编译真实 TS 模块后运行纯函数与 mock 契约输入，不调用模型。
- **环境限制：** cargo 首跑的 3 个失败均为合成子进程 `PermissionDenied`，原命令重跑全过，未复现；`sync-skills.sh --check` 受 BSD `realpath` 缺陷影响，兼容 wrapper 下确认唯一漂移为本次修改的 `period-review`。未执行实际同步，安装副本仍是旧文本。

证据路径：`tests/verification/desktop/evidence/od-arch-176/verify-arch-176.json`（70 项断言）；脚本本体：`tests/verification/desktop/verify_arch_176.py`。

### 未验证项

1. 未用真实声明不含 `off` 的模型发起工作记录分类/分析请求；本轮只验证证据判定与提交参数。
2. 未重编译桌面应用，也未实际同步隔离 Skill 副本；用户当前安装副本需在获准后执行 `scripts/sync-skills.sh`，或在下次构建/初始化时采用内嵌新文本。
3. 未修改 `runtime/`、`engine/`、`desktop/src-tauri/`、`profiles/` 或凭证；未提交、未推送、未打标签。

### 交付时 Git 状态

```text
M PLANS.md / REVIEW.md / STATUS.md
M desktop/src-tauri/src/lib.rs / model_routes.rs
M desktop/src/api.ts / conversationPrompt.ts / recordReview.ts / types.ts / workIntent.ts
M desktop/src/shell/HomeChat.tsx / ModelPicker.tsx / QuestionCard.tsx
M engine/omnidesk_engine/serve.py / task_core.py / web_api_adapter.py
M skills/period-review/SKILL.md
M tests/verification/desktop/verify_home_refine_202.py / verify_worklog_089.py / verify_worklog_103.py / verify_worklog_104.py
M tests/verification/engine-interface/adapter/test_web_api_unit.py
?? desktop/src-tauri/src/model_effort_fallback.rs（OD-ARCH-175）
?? desktop/src/workIntentDecision.ts
?? tests/verification/desktop/verify_model_route_precheck_175.py（OD-ARCH-175）
?? tests/verification/desktop/verify_arch_176.py
```

### 待 ARCHITECT 复核

- 确认使用已加载目录/Profile 元数据而不在每轮调用 `collectModelEffortEvidence` 是否满足“复用两层证据”边界；若要求直接调用该函数，则需额外获批导出 `api.ts` 的私有收集器。
- 确认安装副本漂移的处理时机；本轮未写入运行时副本。

## OD-ARCH-177 执行记录（ARCHITECT 亲自执行，2026-11-17）

> 用户批准 OD-ARCH-177 全部执行并授权提交推送发版；按常驻指令本轮未派子代理，本助手亲自实施。

### 改动

- `desktop/src/companionPlugins.ts`（新增）：常备插件只读清单——AnySearch CLI（github.com/anysearch-ai/anysearch-skill，建议 v2.1.0）、dsh-cost-meter（github.com/Han-1413141/dsh-cost-meter，建议 1.9.6）；仓库不打包插件实体、不自动安装、不联网检查版本。
- `desktop/src/SettingsPanel.tsx`：Profile 资产页新增「配套插件推荐」只读区块（testid: companion-plugins / companion-plugin-*），列表自传达、不加长说明。
- `skills/anysearch/SKILL.md`：第五节新增引擎环境预期——无本机 Codex 技能目录与 ANYSEARCH_SKILL_DIR 属预期，检测不到 CLI 直接回退 web_fetch 并如实标注来源，不按报错指引反复重试；既有红线约束未动。
- `tests/verification/desktop/verify_companion_plugins_177.py`（新增）：清单/渲染/技能文本 11 项断言。

### 验证（实际命令与结果）

- `python3 tests/verification/desktop/verify_companion_plugins_177.py`：11/11（初版 ROOT 层级与一处拼接字符串断言有误，已修正后全过——文件事实）。
- `npx tsc --noEmit`、`npm run build`：通过（脚本运行）。
- `verify_arch_176.py` 70/70、`verify_model_route_precheck_175.py` 12/12+11/11：回归通过（脚本运行）。
- `cargo test --lib -- --test-threads=1`：1209/0（并发运行偶现 3 个引擎 spawn 类 PermissionDenied 环境性失败，单线程与复跑均不复现，非本改动回归）。

### 发布

- 版本 0.2.31 → 0.2.32（package.json + tauri.conf.json），CHANGELOG 0.2.32 条目已写；提交推送后由 release.yml（push to main）自动构建发布。
- 修正：release.yml 的发布版本源是仓库根 `VERSION` 文件（非 package.json），初推时 VERSION 仍为 0.2.31 会 publish=false 空跑；已补 commit 6e3dab1520（VERSION → 0.2.32）并推送，本次 CI 应以 v0.2.32 构建发布。发布提交链：c718aa6e65（feat）→ a1cb0da7ab（docs）→ 99cb0c70f9（release）→ 1e3ec584dc（test 补录）→ 6e3dab1520（VERSION 修正）。

## 0.2.33 线上回归修复执行记录（ARCHITECT 亲自执行，2026-11-17）

> 用户实测 0.2.32：工作记录场景每轮 engine_unavailable，其他场景正常。ARCHITECT 复现并定根因（session/selectModel 拒绝未声明模型的 off，错误兜底误映射），属 OD-ARCH-176 阶段 1 实现判断错误；详见 REVIEW.md 事故记录。本轮修复由 ARCHITECT 亲自完成。

### 改动

- `desktop/src/workIntentDecision.ts`：新增 `effortEvidenceIncludes`；`pickWorkIntentEffort` 改为「证据证明支持才下发」——未声明 / 目录不可用 / 声明不含该档位一律不传；用户手动档位同样需要证据。
- `engine/omnidesk_engine/web_api_adapter.py`：`_apply_reasoning_effort` 捕获 EngineError、记 stderr、按模型默认继续（新增 `import sys`）；`_map_rpc_failure` 将 `session/model-unavailable` 改判 INVALID_ARGUMENT。
- `tests/verification/desktop/verify_arch_176.py`：档位用例按新语义更新（71 项）。
- `tests/verification/engine-interface/verify_effort_select_degrade.py`（新增）：真实 DSH 回归——selectModel 拒绝未声明模型 off → 适配器降级 → 任务成功，4 项断言。

### 验证（实际命令与结果）

- 真实 DSH 回归：`python3 tests/verification/engine-interface/verify_effort_select_degrade.py` → 4/4（此前同参数复现出 `[engine_unavailable] provider "fake" model "synthetic-model" does not support reasoning effort "off"`，修复后任务成功完成）。
- `verify_arch_176.py` 71/71、`verify_companion_plugins_177.py` 11/11、`verify_model_route_precheck_175.py` 12/12+11/11、`cargo test --lib --test-threads=1` 1209/0、`tsc` / `npm run build` 通过。

### 发布

- 0.2.32 → 0.2.33（VERSION / package.json / tauri.conf.json），CHANGELOG 0.2.33 条目已写；推送后 CI 以 v0.2.33 构建发布（cancel-in-progress 会取消未完成的 0.2.32 构建，只发布 0.2.33）。

## OD-ARCH-178/179 发版执行记录 0.2.34（ARCHITECT 亲自执行，2026-11-17）

> 用户批准：登记 swamp 借鉴候选（docs/decisions.md D-110）并推送发新版本供实测「思考等级是否真正生效」；swamp 借鉴设计待用户测试后启动。

### 改动

- 提交内容 = OD-ARCH-178 阶段 A/B/C（档位声明进发行链 + 全路径证据门 + 目录驱动选择器）+ OD-ARCH-179（discipline persona 自查收敛规则）+ D-109/D-110 决策记录 + 全部 verify 脚本。
- 版本 0.2.33 → 0.2.34（VERSION / package.json / tauri.conf.json），CHANGELOG 0.2.34 条目已写。

### 验证（发版前最终门）

- `cargo test --lib --test-threads=1` 1217/0；`tsc` / `npm run build` 通过；verify_arch_178 20+24、arch_176 71/71、175 12+12、177 11/11、202 39/40（既有失败）；真实 DSH 双向验证（声明后 selectModel 接受 off PASS；未声明优雅降级 4/4）。

## 2026-11-18 补充：eco 档位保守声明 + Skill 规范草稿

- 用户实测回执：仅 deepseek-v4.1-flash 显示分档（7 档），eco 与其余模型只有「跟随模型默认」——符合「清单驱动、未实测不声明」设计（VERIFIED_EFFORTS 唯一条目为 flash）。
- 改动：`desktop/src-tauri/src/model_routes.rs` VERIFIED_EFFORTS 增加 opendesign/deepseek-v4.1-flash-eco 的保守声明（仅 off→none，同族继承、标注待实测）；同步更新 4 处断言（flash 7 档、eco 仅 off、glm 不声明）。cargo test --lib 单线程 1217/0 实跑通过。
- 用户已验证 slot：eco 的 off 档位需新构建（0.2.35 或 dev 运行）才可见；其余档位实测通过后再补。
- `docs/skills-spec.md`（草稿 v1）已写，待用户评审修改；视为 OD-ARCH-180 阶段 1 进展。

## 0.2.35 微调与发版（2026-11-18，ARCHITECT 亲自执行）

- 用户实机回执：0.2.34 逐模型档位可用（自选档位可执行/可开关）。
- 微调：设置页档位按钮缩小约一半（settings-panel.css .model-efforts 字号 9px、紧凑内边距）；ModelPicker 去掉「跟随模型默认」（modelEffortOptions：声明非空只列档位，为空保留单按钮兜底）。
- 验证：tsc / npm run build 通过；verify_arch_178 20/20+25/25、arch_176 71/71 全过；cargo 单线程 1218/0（本轮未触 Rust，沿用同日结果）。
- 发版：0.2.34 → 0.2.35（VERSION / package.json / tauri.conf.json），CHANGELOG 0.2.35 已写；本地 tauri build 已验证 .app 正常（DMG 打包失败为旧实例占盘，CI 不受影响）。

## OD-ARCH-179 阶段 D 立项执行记录（ARCHITECT 亲自执行，2026-11-17）

> 勘察（真实 runtime 源码核实）：①`dsh-tool-bash` Config 仅 enableRunInBackground/promoteOnTimeout 两个配置项，输出截断（尾部保留 + spill 落盘）阈值是内部常量不可配，54KB 级输出在阈值内整段回灌；②`dsh-agent-loop` 与 pi-ai 无 maxSteps/stepBudget 配置面；③会话事件流 session-d87d30e7 第 24 行存在 `agent/inbox/spliced` —— turn 2 失败后 DSH 引擎把错误原因拼进 agent 收件箱，属原版自适应设计，「报错回灌」非 OmniDesk 缺陷。

### 改动

- `runtime/dsh-home-seed/profiles/discipline/cordis.patch.yml`：discipline-core persona 追加「自查收敛规则」两条（OmniDesk 自查先读 app/config 两文件；bash 禁扫 storages/node_modules，大范围检索走 tool-fs-search）。work-review-core 无 shell 不适用。
- `docs/decisions.md`：新增 D-109（宿主已验证清单 + 证据门架构、runtime 三项上游缺口登记、复审触发条件）。

### 验证

- seed patch YAML 可解析（python yaml.safe_load 实跑通过）；`probe_work_review_preset_213.py` 所断言的 work-review preset 块未被触碰；按用户指示不推送不发版，改动保留工作树。

## OD-ARCH-178 阶段 A 执行记录（子代理，2026-11-17）

> 执行范围：仅 Rust 宿主层。未改 `desktop/src/`、`engine/`、`runtime/`、`skills/`、Profile 或凭证；未提交、未推送。另一子代理并行修改的前端文件与本记录无关。

### 改动

- `desktop/src-tauri/src/model_routes.rs`：新增静态清单 `VERIFIED_EFFORTS`，唯一条目 `opendesign / deepseek-v4.1-flash`，7 档映射与 `profiles/omnidesk-dev/cordis.patch.yml` 已实测值一致（off→none、minimal/low/medium/high/xhigh/max 同名）。新增 `verified_efforts`、`merge_verified_efforts`、`merge_verified_efforts_in_catalog`；`list_routes` 在脚本返回后对每个 Profile 的 `modelEntries` 补声明。只在 `reasoningEfforts` 键完全缺失时补，已有声明（含 false）不改。
- `desktop/src-tauri/src/model_settings.rs`：`render_model_entry` 在写覆盖文件时同样调用清单合并，覆盖 Profile 路由与 OmniDesk 自有 provider 两条路径；Profile 已声明优先。`yaml_scalar` 对 `off/on/yes/no/true/false/null/~/none` 加引号，避免 YAML 1.1 把映射键 `off` 读成布尔、把 `none` 读成空值。
- 单测：清单命中合并、Profile 已声明不覆盖、provider/model 未命中不变；overlay 断言 deepseek-v4.1-flash 出现 `reasoningEfforts` 而 eco/glm 不出现；自有 opendesign provider 路径；另加一条隔离临时 patch + 仓库内 DSH CLI 的 `list_routes` 真实配置转储用例。

### 验证（实际命令与结果）

- `cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib -- --test-threads=1`：**1217 passed / 0 failed / 1 ignored**（共 1218）。
- 新增用例单独复跑：3 个 catalog 合并用例、4 个 overlay/自有 provider 用例、1 个隔离 `list_routes` 转储用例均通过。
- 证据等级：源码事实 + Rust 单测（实际执行）；`list_routes` 用例实际启动仓库内 DSH CLI 做只读 `--dump-config`，未发网络请求、未读凭证值。
- 证据位置：`desktop/src-tauri/src/model_routes.rs`（清单与 catalog 合并）、`desktop/src-tauri/src/model_settings.rs`（overlay 渲染与 YAML 引号）；本轮未生成独立证据文件，以命令输出为准。

### 未验证项

1. 未重编译桌面应用，未在安装后的 DSH modelCatalog / session/selectModel 上做真实选档调用；“两端一致”由单元级 `list_routes` 与 `render_overlay` 断言覆盖，真实引擎行为待 ARCHITECT 复核后验证。
2. `model_routes::select_route` 的覆盖文件由既有 `model-routes.mjs` 写出；它会保留当前覆盖文件中已有的声明，但不会为旧安装中缺失的声明主动补写。首次经设置页 `model_settings::save` 会写入声明；该路径之外的补写未在本阶段实现。
3. 未为 eco/glm/其它模型声明任何档位；未改开发 Profile 的实测映射。

### 交付时 Git 状态

```text
M PLANS.md（ARCHITECT 计划改动，非本次）
M desktop/src-tauri/src/model_routes.rs
M desktop/src-tauri/src/model_settings.rs
M STATUS.md
并行前端子代理另有 desktop/src/api.ts、HomeChat.tsx、ModelPicker.tsx、modelEffortEvidence.ts 等改动（非本次，实时以 git status 为准）
```

工作树改动全部保留，未提交、未推送。

## OD-ARCH-178 阶段 B+C 执行记录（子代理，2026-11-17）

> 执行范围：`desktop/src/` 前端、`tests/verification/desktop/` 相关 verify 与本节 STATUS 追加。未改 `desktop/src-tauri/`、`engine/`、`runtime/`、`skills/`、Profile 或凭证；未提交、未推送。阶段 A 的 Rust 改动与本节并行写入同一工作树，互不覆盖。

### 改动

- 新增 `desktop/src/modelEffortEvidence.ts`：档位证据的单一目录口径。`resolveModelEffortDeclaration` 先读 `listEngineModels` 的当前模型 `efforts`，目录为空/缺省时才回退 Profile 的 `reasoningEfforts`；`modelEffortOptions` 始终只给「跟随模型默认」+ 声明集合；`gatedReasoningEffort` 作为提交前裁决。
- `desktop/src/shell/HomeChat.tsx`：4 个 `host.submitTask` 站点全部接入同一证据门并显式传 `effortEvidence`；普通对话/未分类场景由原来的「非周报场景恒 null」改为 `pickRoutedWorkIntentEffort(intent ?? "chat")`，未声明仍返回 null。初始 `modelEffort` 改为 `default`；模型声明 off 且用户未主动改档时界面自动沿用 off（与提交侧直出一致）。模型切换与场景套档先按目标模型证据裁决，不支持时回退「跟随模型默认」并在用户主动操作时提示一句。
- `desktop/src/shell/ModelPicker.tsx`：删除固定五档（默认/关闭/低/中/高），按钮改为当前 provider+model 的声明集合；未声明只显示「跟随模型默认」。未知声明档位按 id 显示，已知档位沿用中文标签；保留 `neu-effort-btn`、`effort-advisory` 与新增 `effort-option-*` testid。
- `desktop/src/api.ts`：`host.submitTask` 新增 `effortEvidence` 参数；转发前统一走 `gatedReasoningEffort`，调用方漏传证据或证据不含档位时静默降级 null。`collectModelEffortEvidence` 改为复用同一解析模块，目录非空时不再额外读 Profile。
- `desktop/src/workIntentDecision.ts`：证据类型与 `effortEvidenceIncludes/Excludes` 迁到 `modelEffortEvidence.ts` 并保留原导出，避免 api 层为一道门禁拉入工作记录判定链。
- `tests/verification/desktop/verify_arch_178.py`（新增）：20 项静态 + 24 项运行断言；`verify_arch_176.py`、`verify_model_route_precheck_175.py`、`verify_home_refine_202.py` 的旧文案/转发断言按新契约更新。

### 逐站点审计结论

1. `HomeChat.tsx` 分析轮（`analyzeRecordAfterWrite`，`reasoningEffort: pickRoutedWorkIntentEffort("analyze")`）：已走两层证据 + api 层复验。
2. `HomeChat.tsx` 工作意图轮（`submitRoutedWorkIntent`，`reasoningEffort: effort` 来自 `pickRoutedWorkIntentEffort(action)`）：已走两层证据 + api 层复验。
3. `HomeChat.tsx` 分类轮（`mode: "classify"`，`reasoningEffort: effort` 来自 `pickRoutedWorkIntentEffort("classify")`）：已走两层证据 + api 层复验。
4. `HomeChat.tsx` 普通提交轮（原 `weeklyScene && intent` 分支）：现统一为 `pickRoutedWorkIntentEffort(intent ?? "chat")`；普通对话/未分类场景也先证明可用，未声明传 null + api 层复验。这是本轮修复的缺口站点。
5. `HomeChat.tsx` 模型切换写覆盖文件（`shell.setModelRoute`）：提交前用同一 `effortEvidenceFromLoaded` 裁决，目标模型不支持时改发 `default`；`api.ts` 的 `setModelRoute` 继续保留 OD-ARCH-175 的 `planModelRouteEffort` 两层预检。
6. 其余 `host.submitTask` 站点：`WritingWorkbench.tsx`、`ReviewArchive.tsx`、`WritingOutlineEditor.tsx`、`KnowledgeIngestPanel.tsx`、`WritingSectionEditor.tsx`、`WorkRecordLibrary.tsx`、`aiDraft.ts` 均不下发 `reasoningEffort`（等效 null）；api 层门禁使后续误加显式档位也不会无证据下发。
7. `SettingsPanel.tsx` 只读 `reasoningEfforts` 做配置展示/保存前预检，不直接提交任务；不构成下发路径。

### 验证（实际命令与结果）

- `cd desktop && npx tsc --noEmit` → 0。
- `cd desktop && npm run build` → 成功（vite 构建完成，仅既有 chunk 大小提示）。
- `python3 tests/verification/desktop/verify_arch_176.py` → **cases=71 passed=71 failed=0**。
- `python3 tests/verification/desktop/verify_model_route_precheck_175.py` → **cases 12/12; static 12/12**。
- `python3 tests/verification/desktop/verify_arch_178.py` → **static=20/20 runtime=24/24 failed=0**。
- `python3 tests/verification/desktop/verify_home_refine_202.py` → 39/40；`effort_advisory` 按新文案通过，唯一失败 `home_scene_backdrop` 在 HEAD 即缺少 `App.tsx` 的 `app-scape-art`（与本节改动无关）。
- 未运行 cargo（阶段 A 并行改 `src-tauri`，按任务约束由 ARCHITECT 复核时统一跑）。

### 证据等级与位置

- 文件事实 / 静态检查：4 个 HomeChat 站点、api 门禁、ModelPicker 目录映射、其余 submit 站点无档位。
- 脚本运行：真实 `desktop/src` 的 `modelEffortEvidence.ts` 与 `workIntentDecision.ts` 经 tsc 编译后在 Node 执行，覆盖未声明、声明含 off、目录优先、Profile 回退、按钮集合与门禁。
- 证据文件：`tests/verification/desktop/evidence/od-arch-178/verify.json`、`evidence/od-arch-175/precheck.json`、`evidence/od-arch-176/verify.json`。

### 未验证项

1. 未在真实 DSH `session/selectModel` 上做端到端选档调用；本轮的提交门禁与切档门禁由静态检查和纯函数运行覆盖，真实引擎行为待 ARCHITECT 复核后统一验证。
2. 未重编译桌面应用、未跑窗口级 UI 截图；ModelPicker 的按钮集合由纯函数 + 静态断言覆盖，实际渲染样式未做像素级验收。
3. 目录与 Profile 若在真实运行中给出互相矛盾的声明，按现有门禁以目录非空声明为准；该矛盾场景未用真实端点复现。

### 交付时 Git 状态

```text
M PLANS.md（ARCHITECT 计划改动，非本次）
M STATUS.md
M desktop/src/api.ts
M desktop/src/shell/HomeChat.tsx
M desktop/src/shell/ModelPicker.tsx
M desktop/src/workIntentDecision.ts
?? desktop/src/modelEffortEvidence.ts
M tests/verification/desktop/verify_arch_176.py
M tests/verification/desktop/verify_home_refine_202.py
M tests/verification/desktop/verify_model_route_precheck_175.py
?? tests/verification/desktop/verify_arch_178.py
M desktop/src-tauri/src/model_routes.rs（阶段 A 并行改动，非本次）
M desktop/src-tauri/src/model_settings.rs（阶段 A 并行改动，非本次）
```

工作树改动全部保留，未提交、未推送。

## OD-READ-001 阶段 4 执行记录（BUILDER 子代理，2026-11-18）

- 开工前快照：git status --short --branch 为 ## main...origin/main；HEAD b8b9357b9c。既有未提交改动（PLANS.md、REVIEW.md、STATUS.md、desktop/src-tauri/src/lib.rs、task_shell.rs、desktop/src/App.tsx、api.ts、main.tsx、shell/Dock.tsx、types.ts、docs 两处、scripts/reading_attachment_extract.py）与未跟踪文件（reading_store.rs、reading.css、readingAi.ts、readingAi.test.mjs、ReadingPage.tsx、docs/od-read-001-stage4-dispatch.md）全部原样保留；未 stash/reset/clean、未提交、未推送、未打标签。
- 改动清单（仅本任务新增/修改）：
  - desktop/src-tauri/src/reading_store.rs：新增 verify_organized_markdown 与引用行提取、空白/引号规范化、未核验最多 20 条（超出合并「……另有 N 条」）；save_note_at 调用核验，未核验 > 0 时在 frontmatter 之后插入「⚠ 引文核验」提示块，不阻断保存；无 frontmatter 时补写 title/created/book/source/annotation_count 五字段（YAML 双引号转义），已有 frontmatter 原样保留；ReadingSavedNote 新增 verified_citations: usize 与 unverified_citations: Vec<String>（均 serde default）。
  - desktop/src/shell/ReadingPage.tsx：AI 整理结果进入展示/保存前调用 stripMarkdownFence；保存成功后显示「已核验引文 x 条 / 未核验 y 条」并把条目数写进保存回执。
  - desktop/src/readingAi.ts：新增 stripMarkdownFence 纯函数（仅整段被单个三反引号/三波浪线围栏包裹时剥一层，内部围栏保留）。
  - desktop/src/readingAi.test.mjs：新增围栏剥离断言（整段围栏、波浪线、内部围栏、尾部多余文本、无围栏、空串）。
  - desktop/src/types.ts：ReadingSavedNote 增加 verified_citations: number 与 unverified_citations: string[]。
  - STATUS.md：本节追加。
- 实际命令与结果：
  - cd desktop/src-tauri && /Users/yvan/.cargo/bin/cargo test --offline --lib -- --test-threads=1 → 1232 passed / 0 failed / 1 ignored（50.72s，exit 0）；其中 reading_store 过滤运行 12/12（新增 7 条：全部命中、部分未命中、中文空白差异、无引文/无批注、20 条上限、frontmatter 补写与不改写、未核验提示与计数）。
  - cd desktop && npx tsc --noEmit → exit 0，无输出。
  - cd desktop && npm run build → exit 0；ReadingPage chunk 35.33 kB；仅既有 chunk 大小提示。
  - cd desktop && node --experimental-strip-types --test src/readingAi.test.mjs → 5 pass / 0 fail（含新增围栏断言）。
  - git diff --check → exit 0。
- 证据等级：文件事实（源码与 diff）；静态检查（tsc --noEmit、git diff --check）；脚本运行（cargo test、npm run build、node --test）。测试全部为合成内容（/tmp 一次性目录与单元测试字符串）。
- 未验证项：真实桌面窗口的保存点击、回执视觉排版与 Markdown 渲染；真实模型输出同时含围栏与不可核验引文的端到端样例；已有 frontmatter 且含未核验引文时的提示块位置仅有静态路径覆盖，未做窗口级检查。
- 计划偏差：任务书允许清单未列 desktop/src/readingAi.ts，但围栏剥离需要可被 readingAi.test.mjs 直接 import 的纯函数，故放在该纯函数模块；types.ts 按硬约束中「ReadingSavedNote 字段新增」明确允许而修改；未新增 tests/verification 脚本。
- 交付时 Git 状态（工作树保留，未提交）：
~~~text
## main...origin/main
 M PLANS.md
 M REVIEW.md
 M STATUS.md
 M desktop/src-tauri/src/lib.rs
 M desktop/src-tauri/src/task_shell.rs
 M desktop/src/App.tsx
 M desktop/src/api.ts
 M desktop/src/main.tsx
 M desktop/src/shell/Dock.tsx
 M desktop/src/types.ts
 M docs/OmniDesk-Desktop-Data-Layout.md
 M docs/decisions.md
 M scripts/reading_attachment_extract.py
?? desktop/src-tauri/src/reading_store.rs
?? desktop/src/reading.css
?? desktop/src/readingAi.test.mjs
?? desktop/src/readingAi.ts
?? desktop/src/shell/ReadingPage.tsx
?? docs/od-read-001-stage4-dispatch.md
~~~


## OD-READ-001 提交与推送记录（BUILDER 子代理，2026-11-18）

- 开工基线：`git status --short --branch` 为 `## main...origin/main`；HEAD `b8b9357b9c`，与任务书要求一致。未改动历史、未 stash/reset/clean。
- 提交前验证：`git diff --check` → exit 0；`cd desktop/src-tauri && /Users/yvan/.cargo/bin/cargo test --offline --lib reading_store -- --test-threads=1` → 12 passed / 0 failed / 0 ignored（1221 filtered out，exit 0）。
- 暂存清单：按任务书精确 `git add` 20 个文件；`git diff --cached --name-only` 与清单逐项一致，清单外改动 0 个。`git diff --cached` 目视核对：无密钥/凭证/个人正文，新增测试与夹具均为合成字符串（如「合成书」「第一章」）。
- 提交：`f4c67db150`（一次提交，提交信息按任务书第 2 条原文，经 `git commit -F` 保留多行；20 files changed, 4109 insertions(+), 12 deletions(-)）。
- 推送：`git push origin main` → exit 0；回执 `b8b9357b9c..f4c67db150  main -> main`（https://github.com/klzone/omniharness.git）。未 force、未打标签、未重试。
- 推送后 `git status --short --branch` 快照：`## main...origin/main`（工作树干净，与 origin/main 同步）。
- 清单外未暂存文件：无。本节追加后 STATUS.md 重新处于已修改状态（未提交），留待 ARCHITECT 终检。

## 2026-11-18 组合诊断：0.2.38 外发脱敏核查与输入侧规则补强（ARCHITECT 亲自执行）

> 用户 Windows 自检：含实名、企业名、金额、职务的线索材料经公网端点外发。核查结论：提示词通道脱敏器默认开启且在工作；泄漏通道是「模型用工具读取工作区文件」——工具结果经 DSH 引擎直接进模型上下文，OmniDesk 脱敏器不接入该链路；叠加字典为空、金额无规则、人名/机构启发式漏遮。

### 改动

- `engine/omnidesk_engine/security/masker.py`：新增金额规则（万/亿/元）、名单式人名规则（顿号串）、无后缀企业名回扫（两家/三家/等+企业）、扩展姓名触发词（涉及/涉案/名单/股东/法定代表人等）、常见词防误遮（金额/金融/黄金等）；占位类型新增 AMOUNT。
- `desktop/src/shell/HomeChat.tsx`：新增 `maskNoticeFromReceipt`，两条对话路径统一显示「已自动脱敏（人名 X 处、银行卡 Y 处…）」。
- `tests/verification/engine-interface/adapter/test_web_api_unit.py`：修正自 0.2.33 起失效的旧断言（selectModel 拒绝档位应为降级继续而非抛错）；新增 masker 回归用例（名单/无后缀企业/金额/防误遮）。

### 验证（实际命令与结果）

- masker 单测 46/46（新增用例）、markdown 38/38、adapter 101/101；tsc / npm run build 通过；verify_masker_audit_142 15/15、verify_masker_ui_142 7/7。
- 复现样例：名单式人名、四家企业（一家带后缀一家不带）、1000 万金额、银行卡、电话全部命中；「涉及金额」不再误遮。

### 未覆盖与告知

- 工具读取的工作区文件内容仍不经过脱敏（OD-ARCH-182 阶段 C 镜像方案未实施；本轮按用户裁决只做输入侧）。
- 本改动未推送；是否发 0.2.39 待用户指令。

## 0.2.39 发版门禁记录（2026-11-18，ARCHITECT 亲自执行）

- 用户指令：汇总各会话改动，提交推送 0.2.39。
- 分支状态：本地领先 origin 4 个提交（含其他会话的 0.2.39 release 提交 fb2fb2f66b 与 od-read-002 合并）。
- 发布前门禁实跑：`cargo test --lib --test-threads=1` **1243/0**；`tsc` / `npm run build` 通过；引擎单测 adapter 101/101、masker 46/46、markdown 38/38、process_owner 全部通过；桌面 verify：process_platform_133 28/28、masker_ui 7/7、masker_audit 15/15、arch_178 20/20+25/25、arch_176 71/71、model_route_precheck_175 11/11、companion_plugins_177 全过、od_read_002_stage1 全过。
- 修正：skills 规范对齐后 period-review 文案变化导致 arch_176 三条断言失效，已按新权威文案同步断言（语义契约不变：材料随请求直接起草、为空如实说明、已确认不可覆盖）。
- CHANGELOG 0.2.39 补充「输入侧脱敏提示计数 + selectModel 降级单测修正」与「10 个 Skill 规范对齐」两条；版本号沿用其他会话已抬的 0.2.39。

## 2026-11-18 工作记录回归修复（ARCHITECT 亲自执行）

- 现象 1：记录落盘后出现「这句没有可用的回复」——意图契约 reply 为空时的兜底文案。修复：note 路由兜底改为「本条信息已记录。」（写入失败仍由 finishNote 覆盖为错误）。
- 现象 2：用户说「复盘上周的工作」却按本周出稿。根因：detectPeriodReviewTarget 词表只收固定搭配（复盘上周/上周复盘），「复盘一下上周」漏配 → 目标为空 → 回退模型周期（current week）。修复：总结动词 + 相对周期词组合也识别，支持上周/上上周/上个月/上季度/去年；起止仍由宿主演算。
- 现象 3：复盘正文引用写成空括号「记录、、」。核查：宿主材料含 `#wr-…` 编号（review_store::build_material），属模型未按契约复制；period-review SKILL 增加「编号必须原样写 #wr-…，不得留空括号」硬约束。
- 验证：verify_wr_505 74/74（新增上周/上上周/上个月用例）、arch_176 71/71、arch_178 20+25、tsc/build 通过。改动未推送。

## 2026-11-19 Windows 残留 DSH node 二次修复（OD-ARCH-184，ARCHITECT 亲自执行）

> 用户 0.2.42 实测：关闭 app 后仍残留 node.exe（PID 8660，父 6152，命令行 dsh-runtime bin.js --profile discipline）。核查源码发现三条真实缺口：

### 修复

1. engine_supervisor::finish_stop：原实现 terminate 失败即提前返回，跳过 cleanup_engine_group（DSH 子树兜底回收）。改为先取 terminate 结果、始终执行兜底回收，最后再上报 terminate 错误。
2. process_owner::cleanup_child_on_exit（RunEvent::Exit）：登记属于旧宿主/记录缺失时原实现直接跳过。改为：登记匹配时按 child_pid 树杀；无论是否匹配，再执行「父进程已退出的 DSH node」命令行指纹清扫（另一实例的 node 父进程存活，不会被误杀）。
3. process_owner::cleanup_engine_group_windows：记录不匹配时原实现直接返回，现改为记录匹配才按 child_pid 杀，之后同样执行指纹清扫。
4. platform::kill_process_tree / sweep_orphan_dsh_processes 与 engine/process_owner.py terminate_group：taskkill / powershell 改为 SystemRoot 下绝对路径（打包环境 PATH 被裁剪时裸名会静默失败）。

### 验证与限制

- 本机：cargo test --lib 单线程 1250/0；process_owner 单测 OK；py_compile OK；tsc/build 通过。改动未推送。
- Windows 交叉编译检查在 tauri-winres 的 llvm-rc 依赖处失败（宿主缺 Windows 资源编译器，与代码无关），Windows cfg 代码等 CI/实机验证；**Windows 实机复验待用户**：关闭 app 后 Get-CimInstance 过滤 dsh-runtime 残留应为 0。

## 2026-11-19 OD-ARCH-185 阶段 1 执行记录（ARCHITECT 亲自执行）

- 新增 `engine/omnidesk_engine/security/alias_store.py`（稳定别名表：0600、原子写、损坏容错、金额不持久化）；masker 接入稳定别名（跨会话同一实体同一掩码），unmask 增加持久别名还原（report 增 alias_restored）；宿主 `OMNIDESK_ALIAS_MAP` 注入（与敏感词典同目录，默认启用无弹窗）。
- 验证：新增 test_alias_store_unit 6/6（稳定性、双人区分、无会话映射还原、金额不持久化、0600、损坏容错）；masker 45/45、markdown 38/38、adapter 101/101、knowledge_ingest OK；cargo child_env 2/2（含新增 alias map 路径断言）。改动未推送。
- 阶段 2（镜像工作区）未实施：受 runtime 工具读取边界缺口限制，镜像只能靠约定抑制；待真实“读取原文文件”需求或 runtime 钩子出现再立项。


## OD-ARCH-183 Windows 引擎进程树回收（2026-11-18，ARCHITECT 亲自实施）

- 开工基线：HEAD `cb0a0b31f0`；既有未提交改动（PLANS.md、skills/*、engine/security/masker.py、desktop/src/shell/HomeChat.tsx 等）原样保留，未 stash/reset/clean。
- 改动：`engine/omnidesk_engine/process_owner.py`（Windows 分支：tasklist 存活判定、PowerShell 命令行、taskkill 树杀、镜像名核验、pgid=0 走 child_pid）、`web_api_adapter.py`（shutdown 先树杀）、`serve.py`（关停失败保留登记）、`desktop/src-tauri/src/platform.rs`（Windows process_stat/cmd/table/image/kill_tree/sweep）、`process_owner.rs`（Windows 启动回收、收尾兜底、退出兜底）、`lib.rs`（RunEvent::Exit 兜底）、`tests/verification/desktop/verify_process_platform_133.py`（断言更新）、`tests/verification/engine-interface/adapter/test_process_owner_unit.py`（新增 5 个 Windows 判定单测）。
- 实跑：`cargo test --offline --lib -- --test-threads=1` → 1232 passed / 0 failed / 1 ignored（49.5s）；`python3 -m unittest discover -p "test_*.py"`（engine adapter）→ 249 passed；`verify_process_platform_133` → 28/28；`verify_sys_process_701` → 40/40；`verify_proc_alias_149` → 34/34；`python3 -m py_compile` 两处引擎文件通过。
- 既有失败：`verify_process_lifecycle_074` 4 项失败（N/O/P/R），在 HEAD 纯净 worktree 上复现同样失败，属环境性问题。
- 未验证：Windows 专属编译与行为（本机无法交叉构建，缺 `llvm-rc`）；Windows 实机开关两次后残留为 0 的验收。

## OD-READ-002 阶段 1 执行记录（BUILDER 子代理，2026-11-18）

- 开工基线：`git status --short --branch` 为 `## od-read-002-brain-room`；HEAD `cb0a0b31f077525b10fd4e9c9fc7a12bca1327f4`。未提交、未推送、未打标签；未改 `runtime/`、`engine/` 或凭证。
- 实现：`rss_store.rs` 新增 `rss/subscriptions.json`（schema `omnidesk.rss.v1`）、预置 49 条（9 位精选作者 + 40 条独立博客，MIT 归属注释，不含公众号）、手写 RSS 2.x/RDF/Atom 解析、HTML 净化与段落提取、文章作为单章 `ReadingBook` 入库、已读状态与逐源失败信息。`lib.rs` 注册 6 个命令；`reading_store.rs` 仅将目录/原子写辅助函数提升为 `pub(crate)`。前端 `ReadingPage.tsx` 更名「健脑室」，书架分「书籍」「订阅文章」，订阅模态框支持添加/删除/刷新，未读圆点与既有四按钮划线浮条保留。
- 验证：`cargo test --offline --lib rss_store -- --test-threads=1` → 11 passed / 0 failed（含 127.0.0.1 合成 HTTP 回放，不访问外网）；`npx tsc --noEmit` → exit 0；`npm run build` → exit 0；`python3 tests/verification/desktop/verify_od_read_002_stage1.py` → 42 checks passed；`git diff --check` → exit 0。
- 全量单测：当前 worktree 直接运行 `cargo test --offline --lib -- --test-threads=1` → 1240 passed / 3 failed / 1 ignored；3 项均为既有 `app_paths` 路径边界测试，因 worktree 位于 `~/.codex/...` 先命中禁用片段 `/.codex`，错误文本不含“仓库”。设置 `OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation` 后同一源码重跑 → 1243 passed / 0 failed / 1 ignored（exit 0，50.17s）。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage1.md`（被 `.gitignore` 忽略）；公开源 curl 连通：weekly.tw93.fun、reorx.com、pseudoyu.com、appinn.com 与阮一峰 atom.xml（带 `OmniDesk-RSS/0.1` UA）均 HTTP 200。
- 计划偏差：工作树内 `docs/od-read-002-stage1-dispatch.md` 与 PLANS.md 采用 `subscriptions.json` + 每篇一条 RSS 书；本轮按该详细任务书实现。`reqwest` 启用 `blocking + native-tls + system-proxy`，`Cargo.lock` 新增约 215 行 TLS/代理传递 crate；原“不改变锁文件”假设不成立，需 ARCHITECT 复核。
- 未验证：真实 Tauri 窗口中的订阅模态框布局/焦点/滚动、添加删除与拉取全部（49 源串行）、RSS 文章划线/想法/标段/问 AI 端到端；应用内 `rss_fetch` 的真实网络行为。
- 构建期间为找到 `runtime/dsh-runtime` 等资源临时在 worktree `runtime/` 建了 4 个指向主工作树的符号链接，验证结束后已删除，不在最终工作树留下未跟踪文件。

## OD-READ-002 阶段 1 提交与推送记录（BUILDER 子代理，2026-11-18）

- 开工基线（按任务书写入）：`git status --short --branch` 为 `## od-read-002-brain-room`；HEAD `cb0a0b31f0`（`cb0a0b31f077525b10fd4e9c9fc7a12bca1327f4`），与任务书要求一致；分支 `od-read-002-brain-room`。未 stash/reset/clean，未改清单外文件。
- 提交前验证：`git diff --check` → exit 0；`cd desktop/src-tauri && OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation /Users/yvan/.cargo/bin/cargo test --offline --lib rss_store -- --test-threads=1` → 11 passed / 0 failed / 0 ignored（1233 filtered out，exit 0）。
- 暂存：按任务书清单精确 `git add` 16 个文件；`git diff --cached --name-only` 与清单逐项一致，清单外文件 0 个。`git diff --cached` 敏感模式扫描（sk-/api key/secret/password/私钥/Bearer 等）0 命中；无凭证、无真实业务材料。
- 提交：`f008ef94bb`（`f008ef94bb69f9dfd0223dd7b4a7bd7e3784d70c`），一次 `git commit -F`，提交信息按任务书原文；16 files changed, 2607 insertions(+), 47 deletions(-)。
- 推送：`git push origin od-read-002-brain-room` → exit 0；回执 `* [new branch] od-read-002-brain-room -> od-read-002-brain-room`（https://github.com/klzone/omniharness.git）；`git ls-remote origin refs/heads/od-read-002-brain-room` 返回 `f008ef94bb69f9dfd0223dd7b4a7bd7e3784d70c`，与本地 HEAD 一致。未 push main、未 force、未打标签、未重试。
- 推送后 `git status --short --branch` 快照：`## od-read-002-brain-room`；仅剩 4 个未跟踪构建符号链接 `runtime/dsh-home-seed`、`runtime/dsh-runtime`、`runtime/pnpm`、`runtime/python-vendor`（任务书明确不暂存，未加入提交）。
- 清单外文件：仅上述 4 个 `runtime/` 未跟踪符号链接，未暂存、未修改。本节追加后 STATUS.md 重新处于已修改状态（未提交、未再推送），留待 ARCHITECT 终检。

## OD-READ-002 阶段 3 执行记录（BUILDER 子代理，2026-11-19）

- 基线：`483e2b41772f1c2571fca343e1bfbe74514f7286`；工作树 `od-read-002-brain-room`。未提交、未推送、未打标签；未改 `runtime/`、`engine/`、RSS 抓取链路或划线锚定结构。
- ①右侧栏：`panelOpen` 初始 `false`；收起态为 40px 贴右缘图标栏（笔记图标 + 批注总数角标、AI 图标），点击展开并切 notes/ai；`click/keydown/scroll/pointerdown` 捕获重置 90 秒计时，切书或关闭时清除；宽度 200ms 过渡。
- ②首页统计条：槽位 `shrink-0 flex justify-center`，统计算 `width: fit-content; margin: 0.5rem auto 0.75rem`；字数项 label 改为「字数」，仍可整条点击跳健脑室。
- ③④ 验证：`rss_store.rs` 的 `![img]({})` 转换、`reading_store.rs` 的 RSS 字数聚合单测、前端「字数」项均在；未重做实现。
- 测试：`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation cargo test --offline --lib -- --test-threads=1` → 1249 passed / 0 failed / 1 ignored（49.20s）；`npm run build` → exit 0；verify_reading_003 25/25、verify_reading_004 19/19、verify_od_read_002_stage3 18/18；`git diff --check` → exit 0。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage3.md`（`.gitignore` 忽略）。
- 未验证：真实窗口 90 秒计时/重置、图标栏在 `<xl` 与 `xl` 的视觉位置、划线动作条真实点击展开、首页统计条最终贴底排版。保留既有 4 个未跟踪 `runtime/` 构建符号链接，未暂存、未修改。

## OD-READ-002 阶段 4 执行记录（BUILDER 子代理，2026-11-19）

- 基线：`483e2b41772f1c2571fca343e1bfbe74514f7286`；保留阶段 3 未提交改动。未提交、未推送、未打标签；未改划线锚定、RSS 抓取与首页其它区域，未引入新依赖。
- 视觉：`reading.css` 新增 `.reading-page` 作用域暖纸新拟物变量；阅读纸面 26px 凸起卡 + 34/44 内边距 + 宋体 16.5px/2.0/justify；琥珀划线/主色想法；媒体内凹窗 + 图注；RSS 引文井；章节头进度轨；书架 288↔56 see-saw；右侧面板卡/tabs/批注卡/AI 井；浮动条 raised 胶囊。
- 布局：`ReadingPage.tsx` 书架满宽 288px、面板展开时 56px 三钮图标栏（书籍/订阅/搜索，aria-label）；面板默认 320px；搜索胶囊本地过滤现有书架条目；右侧面板收起仍 40px 图标栏。
- 测试：`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation cargo test --offline --lib -- --test-threads=1` → 1249 passed / 0 failed / 1 ignored（49.22s）；`npx tsc --noEmit` exit 0；`npm run build` exit 0；verify_reading_003 25/25、verify_reading_004 19/19、verify_od_read_002_stage3 18/18、verify_reading_005_design 34/34；`git diff --check` exit 0。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage4.md`（`.gitignore` 忽略）。
- 未验证：真实窗口视觉层次、288↔56/320 过渡、窄屏 overlay、浮动条 hover/active、搜索胶囊焦点与过滤手感。既有 4 个未跟踪 `runtime/` 构建符号链接保留，未暂存、未修改。

## OD-READ-002 阶段 5 执行记录（BUILDER 子代理，2026-11-19）

- 基线：`483e2b41772f1c2571fca343e1bfbe74514f7286`；保留阶段 3/4 未提交改动。未提交、未推送、未打标签；未改 RSS 抓取、批注锚定与首页其它区域，未引入新依赖。
- 仪表盘：新增 `ReadingDashboard.tsx`；无书时替换旧空态。顶部最近在读书（实际进度、封面、格式章数、上次日期）+ 1000 分钟目标（本月书籍 chars×progress/300，标注估算、无 knob）；五格按字数/今年已读/批注/订阅文章/整理笔记；最近 4 本读完含封面、作者、日期、100% 环；RSS 花瓣固定底色层 + 按本月已读比例的缩放数据层；空态两卡 + 精选作者 chips。
- 宿主/前端：`rss_store::preset_sources` 返回 9 位精选作者，新增 `rss_preset_sources` 命令与 `RssPresetSource` 类型/API。
- 测试：`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation cargo test --offline --lib -- --test-threads=1` → 1250 passed / 0 failed / 1 ignored（48.78s）；`npx tsc --noEmit` exit 0；`npm run build` exit 0；verify_reading_003 25/25、verify_reading_004 19/19、verify_reading_005 34/34、verify_od_read_002_stage3 18/18、verify_reading_006_dashboard 34/34；`git diff --check` exit 0。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage5.md`（`.gitignore` 忽略）。
- 未验证：真实窗口仪表盘视觉、目标轨/五格/花瓣/空态排版、查看全部/导入/书籍行/chip/订阅入口的真实点击；既有 4 个未跟踪 `runtime/` 构建符号链接保留，未暂存、未修改。

## OD-READ-002 阶段 6 执行记录（BUILDER 子代理，2026-11-19）

- 基线：`483e2b41772f1c2571fca343e1bfbe74514f7286`；保留阶段 1–5 未提交改动。未提交、未推送、未打标签；未改 RSS 抓取链路、批注锚定、exports，未新增依赖。
- 书架与删除：`ReadingPage` 仅在 `book` 非空时渲染左侧书架；仪表盘「查看全部」打开书架管理弹窗（全部本地书、封面/格式/章数/进度、打开/删除、Esc）；书架行与管理行加 trash 删除（stopPropagation）与 ConfirmDialog；删除后 refreshLibrary，当前书清理进度定时器并关闭阅读器。
- 后端：`reading_store::delete_book_at`（安全编号、拒绝 rss、删 books/meta/progress/notes/meta cover、exports 保留、部分失败回报已删项）；`read_cover_at` 缺封面时从 source_path 回提取并原子写回 meta.cover；`reading_stats` 完成阈值 0.995；新增 `reading_delete_book`、`reading_remote_media` 与 `task_shell::read_remote_media`（http/https、≤2 跳逐跳校验、10s、8 MiB、图片白名单）。
- 前端修正：DashboardCover/BookCover 不再因 cover 为空早退；远程正文图片走 `readingRemoteMedia`（缓存、失败文案）；仪表盘完成判定 0.995、目标条 <12% 隐藏条内文字并在条下显示剩余、花瓣图自适应、容器滚动约束保留。
- 测试：`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation cargo test --offline --lib -- --test-threads=1` → 1253 passed / 0 failed / 1 ignored（50.14s）；`npx tsc --noEmit` exit 0；`npm run build` exit 0；verify_reading_003 25/25、004 19/19、005 34/34、006 34/34、007 40/40；`git diff --check` exit 0。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage6.md`（`.gitignore` 忽略）。
- 未验证：真实窗口书架显隐/管理弹窗/删除交互、目标条窄宽排版、花瓣图滚动完整性；真实 RSS 远程图片（CDN 重定向/SVG/超大响应）端到端；旧书源文件是否仍在原路径。既有 4 个未跟踪 `runtime/` 构建符号链接保留，未暂存、未修改。

## OD-READ-002 阶段 7 执行记录（BUILDER 子代理，2026-11-19）

- 基线：`483e2b41772f1c2571fca343e1bfbe74514f7286`；保留阶段 1–6 未提交改动。未提交、未推送、未打标签；未改批注锚定结构、未删导出、未新增依赖。
- 真实阅读时长：progress/<id>.json 增加 daily_seconds（UTC 日期→秒，serde default）；set_progress_at 保留已有计时；新增 reading_time_add（单次 ≤3600 秒，累加）；reading_stats 增加 month_minutes（当月秒数四舍五入）与 streak_days（今天为 0 时从昨天起算）；ReadingPage 在可见且聚焦时每 15 秒累计、60 秒 flush，切书/关闭/隐藏/失焦 flush。
- 仪表盘：标题右侧恢复连续天数/本月分钟两枚胶囊；目标区改用 month_minutes，删除估算与字数×进度公式；目标条左下「继续阅读」（最近在读书，无书禁用）；花瓣中心改为 16px 主色大写 R；下部两栏 ≥1000px 并排、<1000px 堆叠；作者列始终显示，空作者显示「未知作者」。
- EPUB 作者：helper 解析 OPF dc:creator → ReadingSource.author → 新导入与回填写 meta.author。
- 旧内容媒体回填：reading_refresh_content 仅 epub/mobi，重新 read_reading_source 取带媒体标记的章节/作者/封面；按 quote 先原段落再全章重定位批注，找不到保留原位置并返回 unresolved；新导入与回填置 media_version=1，旧书 openBook 自动回填一次并重载，失败置 1 防重复并 toast。
- RSS 旧文章回填：parse_entry 解析 enclosure / media:content（仅 http/https、image/*），正文无图时前置 ![img]；ingest_feed_at 对已存在且无批注文章，在新正文含媒体标记或条目带图时更新 chapters/author/source_sha256，保留 progress/read。
- 测试：`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation cargo test --offline --lib -- --test-threads=1` → 1259 passed / 0 failed / 1 ignored（89.23s）；`npx tsc --noEmit` exit 0；`npm run build` exit 0；verify_reading_003 25/25、004 19/19、005 34/34、006 34/34、007 40/40、新增 008 49/49；`git diff --check` exit 0。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage7.md`（`.gitignore` 忽略）。
- 未验证：真实窗口计时节奏/切书与隐藏 flush、胶囊/继续阅读/R 字/1000px 切换视觉；真实用户 EPUB/MOBI 源文件自动回填（合成 EPUB 已覆盖端到端）；真实 RSS enclosure 图片显示；多作者 EPUB 只取第一个 creator。既有 4 个未跟踪 `runtime/` 构建符号链接保留，未暂存、未修改。

## OD-READ-002 阶段 8 执行记录（BUILDER 子代理，2026-11-19）

- 基线：`483e2b41772f1c2571fca343e1bfbe74514f7286`；保留阶段 1–7 未提交改动。未提交、未推送、未打标签；未改批注锚定结构、未动用户导出、未新增依赖。
- 顶部并排：`reading.css` 的 `.reading-dash-top` 基准改为 `minmax(0, 0.95fr) minmax(0, 1.05fr)` 两列；删除 ≤1280 断点的顶部单列覆盖与 hero 左内边距归零；新增 ≤900 断点（gap 20px、hero 间距 14px、数字 64px、hero 左内边距 0）。窗口最小宽度 960px 起始终保持并排。
- RSS 正文回填：`rss_store` 的 `html_to_paragraphs`/`sanitize_html`/`sanitize_media_url` 接收条目 link 作为 base_url；新增 `resolve_media_url`/`normalize_absolute_url` 解析 `/x`、`//cdn`、`../x`、`?query` 为绝对 http(s)，未知协议不解析，解析失败不生成图片标记；`entry_image_url` 同步支持相对地址。
- 单篇回填命令：新增 `rss_refresh_article`（lib.rs 注册、api.ts/types.ts 同步）；`refresh_article` 读取 `rss/articles/<id>.json` 与 `books/<id>.json`，重新抓取该 feed 并按 guid 或 link 匹配；仅当文章无批注时重算正文并更新 chapters/author/source_sha256（media_version=1），progress/imported_at 保留；有批注、未匹配、正文一致或仍只有旧占位时返回 updated:false + reason 且不改文件。
- 前端：`ReadingPage` 新增 `hasLegacyImagePlaceholder` 与 `rssRefreshAttempted` 一次标记；openBook 对无批注且含 `[图片：` 的 RSS 文章自动调用回填并重读一次，成功刷新书架，失败/无需回填 toast 原因，不循环重试。
- 测试：`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation cargo test --offline --lib -- --test-threads=1` → 1262 passed / 0 failed / 1 ignored（87.00s）；`npx tsc --noEmit` exit 0；`npm run build` exit 0；verify_reading_003 25/25、004 19/19、005 34/34、006 34/34、007 40/40、008 49/49、新增 009 36/36；`git diff --check` exit 0。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage8.md`（`.gitignore` 忽略）。
- 未验证：真实窗口 960–1280px 顶部排版手感；真实 appinn feed 在线回填（单测为合成 RSS）；既有文章 guid/link 是否仍匹配当前 feed；回填后远程图片实际加载；有批注文章按设计跳过，旧占位可能保留。既有 4 个未跟踪 `runtime/` 构建符号链接保留，未暂存、未修改。

## OD-READ-002 阶段 9 执行记录（BUILDER 子代理，2026-11-19）

- 基线：`483e2b41772f1c2571fca343e1bfbe74514f7286`；保留阶段 1–8 未提交改动。未提交、未推送、未打标签；未改批注锚定结构、未动用户导出、未新增依赖。
- 仪表盘分割线：`reading.css` 的 `.reading-dash-divider` 改为 3px 高、surface 底色、2px 圆角，并用上下两道 inset 阴影形成内凹凹槽；仪表盘上、中、下三段之间原有两条分割线都保留。
- RSS 打开：`ReadingPage.openBook` 初次 `readingBook` 成功后立即 `setBook(loaded)`，busy 只覆盖初次读取；随后用独立异步任务调用 `rssRefreshArticle`，期间 toast「正在补齐文章图片…」，失败提示原因。成功后重读文章并再次 `setBook`，保留当前滚动位置、阅读进度与批注面板上下文，不重置选择/批注状态；一次标记仍防循环。
- 宿主：`rss_refresh_article` 改为 async 命令，内部用 `tauri::async_runtime::spawn_blocking` 执行 `rss_store::refresh_article`，与 `rss_fetch` 相同模式；后台任务异常转成可读错误返回。
- 测试：`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation cargo test --offline --lib -- --test-threads=1` → 1262 passed / 0 failed / 1 ignored（52.88s）；`npx tsc --noEmit` exit 0；`npm run build` exit 0；verify_reading_003 25/25、004 19/19、005 34/34、006 34/34、007 40/40、008 49/49、009 36/36、新增 010 24/24；`git diff --check` exit 0。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage9.md`（`.gitignore` 忽略）。
- 未验证：真实窗口中凹槽线的手感与两条线的视觉分隔；真实订阅文章点击后立即进入阅读、20s+ 网络回填期间的 toast 与可交互性；回填完成后的真实滚动位置/批注面板上下文保持（当前为静态断言 + 构建验证）。既有 4 个未跟踪 `runtime/` 构建符号链接保留，未暂存、未修改。

## OD-READ-002 阶段 10 执行记录（BUILDER 子代理，2026-11-20）

- 基线：`483e2b41772f1c2571fca343e1bfbe74514f7286`；保留阶段 1–9 未提交改动。未提交、未推送、未打标签；未改批注锚定结构、未动用户导出、未新增依赖。
- 侧栏排版：`ReadingPage` 的「健脑室」标题改为 16px / 700 / `.08em` 字距；书籍标题 14px semibold、副信息 12px、文章行标题 13.5px、时间与徽标 11.5px；`reading.css` 的 tabs 12.5px、搜索输入 13px，新增铅笔按钮显隐样式。
- 自动刷新：新增 30 分钟常量与 `maybeAutoRefreshRss`；读取 `subscriptions` 最大 `last_fetched_at`，挂载与窗口 focus 时超期则后台 `shell.rssFetch()`，不占 `rssBusy`，完成后 `refreshLibrary` + `refreshRss`；`lastAutoRefreshRef` 记录触发时间、`autoRefreshRunning` 防并发。
- RSS 过滤与收藏：`RssArticleMeta` 增加 `favorite`（serde default）；`rss_store::set_article_favorite` 只改写 `articles/<id>.json`；新增 `rss_article_set_favorite` 命令、`rssArticleSetFavorite` 与前端类型。侧栏按 `published_ts`（否则 `fetched_at`）过滤 30 天，favorite 例外；分组/页签计数随过滤结果；文章行显示星标与日期。头栏对 RSS 文章用星标替代导入按钮，实心主色表示已收藏，toast「已收藏/已取消收藏」。
- 改名：`reading_store::rename_book_at` 校验安全 id、拒绝 rss、标题 1..=300 字符，用 `serde_json::Value` 仅改 `books/<id>.json` 与 `meta/<id>.json` 的 title；`reading_rename_book` / `readingRenameBook` 已注册。侧栏书籍行与书架管理弹窗各加铅笔按钮，保存后刷新 library，并同步当前打开书的 header 标题。
- 订阅折叠：`collapsedSources` 默认 `true`；文章列表首次可用后 `Math.random` 选一个来源展开，`randomSourceExpanded` ref 只执行一次。
- 按钮与文案：侧栏「订阅管理」「拉取全部」已删除，仪表盘「管理订阅」入口保留；RSS 空态改为指向仪表盘订阅入口。
- 测试：`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation /Users/yvan/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1264 passed / 0 failed / 1 ignored（55.82s）；`npx tsc --noEmit` exit 0；`npm run build` exit 0；verify_reading_003 25/25、004 19/19、005 34/34、006 34/34、007 40/40、008 49/49、009 36/36、010 22/22、新增 011 47/47；`git diff --check` exit 0。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage10.md`（`.gitignore` 忽略）。
- 未验证：真实窗口的侧栏字号层级/铅笔按钮触控可见性、真实 >30 天文章过滤与 favorite 例外、真实挂载/聚焦后台刷新耗时与失败展示、真实改名后 header/书架即时刷新、随机展开分布与手动折叠持久性。既有 4 个未跟踪 `runtime/` 构建符号链接保留，未暂存、未修改。

## OD-READ-002 阶段 11 执行记录（ARCHITECT 亲自实施，2026-11-20）

- 基线：`2046d12d33c874131ba20319cf1b8686f7b401ad`；保留全部既有未提交/未跟踪改动；未提交、未推送、未打标签。
- 阅读设置：`ReadingPage` 顶栏右侧 `Aa` 按钮（`order-last` 视觉置右）与弹层；字体选项「系统默认/思源黑体/思源宋体/霞鹜文楷」，字号 14–26，排列「左/两端/右」；`localStorage` 持久化（`omnidesk.reading.settings.v1`）、恢复默认、点击外部/Esc 关闭。设置以 `--rd-reader-font/size/align` 落在 `.reading-page` 根，只有 `.reading-page .reading-paragraph` 消费；章节标题、顶栏、侧栏与文末模块不跟随；未提供宽度/颜色。
- 字体资源：新增 `@fontsource-variable/noto-sans-sc`、`@fontsource-variable/noto-serif-sc`、`lxgw-wenkai-webfont`（SIL OFL 1.1）；`main.tsx` 自托管引入；OFL 文本复制到 `desktop/public/fonts/`；`THIRD-PARTY-NOTICES.md` 增列。构建产物约 17 MB，运行时无外链。
- RSS 附件：`RssArticleMeta` 增加 `attachments`（serde default）；解析 `enclosure`/`media:content` 非图片媒体（http(s)、≤20 条、标题/MIME/字节数）；旧文章刷新时补写元数据，不动正文/批注；单测覆盖解析与旧 meta 补写。
- RSS 文末：仅 `source_format === "rss"` 渲染；附件井（音频/视频「播放」后按需走 MediaBlock，另给「打开原文件」）、「打开原文」、`完` 标记、`约 N 分钟读完`（400 字/分钟，至少 1 分钟）、同源更早文章的下一篇卡片（点击直接打开）。本地书籍不显示。
- 测试：`cargo test --offline --lib rss_store::` 23/23；全量 lib 1288 passed / 0 failed / 1 ignored（顺带把材料拒绝文案改为中性描述，修复既有断言）；`npx tsc --noEmit` exit 0；`npm run build` exit 0；`npm run test` 88/88；`verify_reading_012_settings_and_footer.py` 43/43；003 25/25、004 19/19、005 34/34、008 全过；006/007/009/010/011 的既有静态断言因 OD-HARDEN-2043 重构失败（命令迁到 `commands/reading.rs`、媒体上限 8 MiB、错误文案调整）。
- 证据：`tests/verification/desktop/evidence/od-read-002/stage11.md`（Git 忽略）。
- 未验证：真实窗口字体字形/弹层阴影/字号与对齐切换手感；真实 enclosure 音频/PDF 的播放与外链打开；CJK 子集首次按需加载耗时；下一篇切换后的滚动位置。既有 4 个未跟踪 `runtime/` 构建符号链接保留，未暂存、未修改。
- 提交后 CI 修正：Linux test job 首轮暴露 `diagnostics::newest_session_prefers_latest_mtime`（mtime 秒级截断）与 `attachment_reads_repo_files_but_still_refuses_runtime_data`（附件禁区 `omniharness` 误伤 runner 仓库路径）；分别改为纳秒与从附件禁区移除该项。第二轮暴露 Node 22 不接受 `--test src` 目录参数，`npm test` 改为 `node --experimental-strip-types --test` 递归发现。第三轮暴露 `workIntent.test.mjs` 三个测试误嵌在父测试内，Node 22 取消子测试；已补父测试收尾并移除多余括号。Node 22.23.4 与本机均 88/88；本机全量 1288 passed / 0 failed / 1 ignored。

## OD-HOME-0245 执行记录（ARCHITECT 亲自实施，2026-11-20）

- 基线：`63c7756edd`；保留已推送的 0.2.44 工作区。
- 首页：阅读统计槽位仅在空白对话渲染，有消息后与空态标语一起隐藏，不占版面。
- 当日摘要：删除前端 `dailySummary.ts`、HomeChat 全部摘要上下文/确认/落盘路径、WorkRecordLibrary 摘要入口与卡片、conversationPrompt daily-summary 模式、workIntent/workIntentDecision 意图；删除宿主 `daily_summary.rs`、`worklog_daily_summary_generate/read` 与注册；内置 Skill 清单、seed、样本 rules 解绑并删除 `skills/daily-summary`。历史摘要文件未删除。
- 引擎文案：新增 `engineLabels.ts`（DSH Web API / 版本未知 / 协议兼容），顶栏与引擎技术切片共用；不再直接显示 `unknown · compatible`。
- 通知与待办：新增 `NotificationCenter`，聚合待确认写入、到期/逾期/待确认任务、未完成任务总览与 `review_due`；徽标数量、空态、错误提示、点击场景跳转齐全；顶栏未实现占位已移除。
- 周复盘：ReviewArchive 草稿态新增「确认归档」按钮（`worklogPeriodReviewConfirm`），确认后刷新详情/列表；草稿提示确认后锁定，已确认仍禁止覆盖；重新生成沿用 `.bak` 备份。
- 测试：`cargo test --offline --lib -- --test-threads=1` → 1276 passed / 0 failed / 1 ignored（删除 12 项已下线的摘要测试）；`npx tsc --noEmit`、`npm run build`、`npm test` 88/88（Node 22.23.4 同）；`verify_home_work_0244.py` 31/31、`verify_hardening_2043.py` 77/77、`verify_reading_004_formats.py` 19/19、`verify_reading_entity_2043.py` 7/7。
- 未验证：真实窗口通知弹层视觉与任务计数、真实待确认/到期数据、复盘确认后的列表刷新手感；旧历史摘要文件仅保留未清理。未提交、未推送。

## OD-PERF-0245 执行记录（ARCHITECT 亲自实施，2026-11-20）

- 基线：OD-HOME-0245 未提交工作区（0.2.44 已发布）。
- 引擎轮询：新增 `engine_describe_if_running`（只读、spawn_blocking）；App 首探与 5 秒轮询不再启动引擎，仅启动后单次预热；`engineLabel` 不再显示未暴露的版本。
- 编码：`child_env` 注入 `PYTHONUTF8=1` / `PYTHONIOENCODING=utf-8:replace`；`serve.main` 重配 stdin/stdout；`web_api_adapter.Popen` 显式 UTF-8 解码；Rust 两个读取线程改 `read_until` + `from_utf8_lossy`，新增坏字节单测。
- `getpgrp`：`current_pgid` 非 POSIX 返回 0，`AttributeError`/`OSError` 都回退 0；新增 2 项 Python 单测。
- 分段计时：`ChatMessage.timing` 记录 `classifyMs/materialMs/submitMs`；路由提交使用原始 `ctx.startedAtMs`，消息显示「判定 / 素材 / 提交」，保留「首字」「总耗时」。
- 测试：Rust 全量 1277 passed / 0 failed / 1 ignored；`npm test` 88/88；`npm run build` 通过；`verify_review_perf_0245.py` 21/21、`verify_home_work_0244.py` 31/31；process_owner 18/18、web-api 105/105。
- 未验证：Windows 实机 150 秒目标、真实引擎重启次数与 GBK 输出；分段计时只覆盖前端可观测段，模型内部排队仍以 TTFT 近似。未提交、未推送。

## OD-GROW-301 G0 执行记录（BUILDER，2026-11-23）

- 基线为 `5dc5df4b622815f4803ec4cff07d448e9824dbf1`，开工前 git status 为 `M PLANS.md` 与未跟踪 `docs/od-grow-301-g0-dispatch.md`，全部既有改动保留。执行期间状态复查出现新的未跟踪 `docs/od-grow-301-t1-dispatch.md`，未读取、未修改、未删除。只读核实，未改源码、配置、测试，未提交、未推送、未打标签。
- 实际执行的主要命令均为只读检索与源码查看。
  - `git status --porcelain=v1 -b`、`git rev-parse HEAD`、`git diff --stat`、`git diff --cached --stat`。
  - `rg --files -g 'writing_section.rs' -g 'writing_store.rs' -g 'document_store.rs' -g 'WritingWorkbench.tsx' -g 'thread_store.rs' -g 'sceneThreads.ts' -g 'threadCategories.ts' -g 'WorkRecordLibrary.tsx' -g 'lib.rs' -g 'pending_writes.rs' -g 'ReviewArchive.tsx' -g 'AnnouncementBar*' -g 'HomeCanvas*' -g 'api.ts'`。
  - `nl -ba desktop/src-tauri/src/document_store.rs | sed -n '612,960p'`、`nl -ba desktop/src-tauri/src/writing_section.rs | sed -n '380,540p;780,930p'`。
  - `nl -ba desktop/src/shell/WritingVersionPanel.tsx | sed -n '1,450p'`、`nl -ba desktop/src/shell/WritingSectionEditor.tsx | sed -n '1,12p;120,160p;424,466p'`。
  - `nl -ba desktop/src-tauri/src/thread_store.rs | sed -n '1,280p;420,480p;640,720p'`、`nl -ba desktop/src/sceneThreads.ts | sed -n '280,520p'`。
  - `nl -ba desktop/src/shell/WorkRecordLibrary.tsx | sed -n '580,670p;1700,1770p'`、`nl -ba desktop/src/api.ts | sed -n '300,510p'`。
  - `nl -ba desktop/src-tauri/src/lib.rs | sed -n '380,560p;760,900p;8980,9040p'`、`nl -ba desktop/src-tauri/src/engine_bridge.rs | sed -n '200,290p'`。
  - `nl -ba engine/omnidesk_engine/task_core.py | sed -n '150,215p;400,470p;580,670p;680,760p;900,975p'`、`nl -ba engine/omnidesk_engine/web_api_adapter.py | sed -n '500,530p;1480,1620p;2740,2830p'`。
  - `nl -ba desktop/src-tauri/src/pending_writes.rs | sed -n '420,450p;580,610p'`、`nl -ba desktop/src/shell/AnnouncementBar.tsx | sed -n '1,300p'`、`nl -ba desktop/src/shell/HomeCanvas.tsx | sed -n '1,120p;300,620p'`。
  - `nl -ba desktop/src/shell/ScenePage.tsx | sed -n '95,120p;590,615p'`、`nl -ba desktop/src-tauri/src/app_paths.rs | sed -n '20,115p'`、`nl -ba desktop/src/shell/ReviewArchive.tsx | sed -n '1,200p;400,800p'`、`nl -ba desktop/src-tauri/src/review_store.rs | sed -n '720,800p;1135,1205p'`。
  - `rg -n 'save_with_snapshot|VersionSource::Ai|VersionSource::User|SnapshotSpec' desktop/src-tauri/src --glob '*.rs'`、`rg -n 'ui_state|ui-state' desktop/src-tauri/src`、`rg -n 'worklog_period_review_(read|confirm|list|generate|save|write|update)' desktop/src-tauri/src/lib.rs desktop/src/api.ts`。
- 五项结论摘要，细节与逐条来源行号见 `docs/od-g0-design-notes.md`。
  1. 写作版本链。章节确认保存的是整篇 `正文.md` 的 `Section/User` 快照，AI 草稿只在界面预览、确认前不落盘，生产路径没有 `VersionSource::Ai` 保存。按章对比数据上可行，但当前只有整篇段落 diff，且插入新段会错位后续段号。证据等级为文件事实与静态检查。
  2. 线程。`ChatMessage` 必需 `id/role/text`，没有每条消息通用时间戳，助手轮次可选 `startedAtMs/endedAtMs`。plain 以 `tag === "plain"` 识别，未知标签归一为 plain。`thread_store::load` 逐文件读全部 JSON 和消息、全量排序，无分页，前端有 4 秒等待上限。建议 G2 用元数据列表加默认上限或分页，再按 20 条读取正文。证据等级为文件事实与静态检查，实际耗时未核实。
  3. 无人值守提交。`WorkRecordLibrary` 的 prepare→workspace→submit→轮询→result→apply 可无聊天组件复用。纯文本提示词可提交，但接口没有工具禁用字段。结果按任务号直接取回，重启后只能恢复状态不能取回正文。没有引擎侧任务时限，900 秒只是调用方轮询上限，取消需显式调用且默认等 15 秒。挂点为新增 `commands/consolidation.rs`，用不在 `scene_for_intent` 白名单的 intent，并按批准记录放行。证据等级为文件事实与静态检查，真实模型行为未核实。
  4. 首页提醒。`AnnouncementBar` 返回 null 且只挂 `ScenePage`。最小方案是在 `HomeCanvas` 的 `upper` 槽顶部条件渲染，且放在 `conversationStarted` 条件之外。`ui-state` 路径已定义，但没有读写命令，关闭状态持久化需 G5 新增最小入口。证据等级为文件事实与静态检查，持久化能力未核实。
  5. 周期复盘。`ReviewArchive` 只支持生成、重新生成、确认、导出与交给 AI 分析，正文只读，宿主没有编辑命令，确认只改状态与时间。H1/H2 不依赖应用内编辑。证据等级为文件事实。
- 改动文件只有 `docs/od-g0-design-notes.md` 新增和 `STATUS.md` 本节追加。未改任何源码、配置、测试，未提交、未推送、未打标签。

## OD-GROW-301 T1 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`；开工时保留 `M PLANS.md`、`M STATUS.md` 与未跟踪 `docs/od-g0-design-notes.md`、`docs/od-grow-301-g0-dispatch.md`、`docs/od-grow-301-t1-dispatch.md`，未改动、未删除。未提交、未推送、未打标签。
- 改动：`desktop/src-tauri/src/lib.rs` 的 `engine_submit_task` 在 `workspace_dir` 为 `None` 或空白字符串时，先经宿主 `resolve_submit_workspace_dir` 调用现有 `task_shell::prepare_local_data_root()`，把本地数据根作为工作目录下发；显式传入的非空白路径原样保留。目录准备失败返回 `engine_error_code=workspace_unavailable` 的 serde_json 错误，且发生在 `with_engine_call` 之前，不触达引擎。未改 engine/、runtime/、前端，未新增依赖。
- 新增单测：缺省/空白使用数据根、显式路径不受影响且不触发准备、准备失败映射为 `workspace_unavailable`、提交入口在触达引擎前解析目录。既有静态接线断言未改，全部保持通过。
- 测试：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → 1296 passed / 0 failed / 1 ignored（52.71s，编译 18.25s）；基线 1292 passed 加新增 4 项。证据：`tests/verification/desktop/evidence/od-grow-301/t1/verify_output.txt`。
- 未验证：未跑真实模型的端到端提交流程；未验证 Windows 实机目录准备失败路径；目录创建失败用例以注入的失败函数验证错误映射，生产准备函数本身由既有 `task_shell` 单测覆盖。

## OD-GROW-301 G1 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`。开工前快照为 `M PLANS.md`、`M STATUS.md`、`M desktop/src-tauri/src/lib.rs`，另有未跟踪 `docs/od-g0-design-notes.md`、`docs/od-grow-301-g0-dispatch.md`、`docs/od-grow-301-g1-dispatch.md`、`docs/od-grow-301-t1-dispatch.md`。上述既有改动全部保留，未读取或改写任务书之外的文件内容；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src-tauri/src/habit_cards.rs`（新增）：习惯卡、未决问题、整理水位、统计 jsonl、驳回哈希与待审定条目的存储和校验。根目录为 `<应用数据根>/habits/`，子目录 `candidates/`、`active/`、`archive/`、`open-questions/`、`state/`、`pending-review/`；根与子目录都过 `task_shell::assert_local_root_allowed`、`assert_outside_repo` 与真实落点校验。
  - `desktop/src-tauri/src/commands/habits.rs`（新增）：8 个 Tauri 命令，取宿主数据根与当前 Profile 规则后调用存储层。
  - `desktop/src-tauri/src/commands/mod.rs`：只加 `pub mod habits;`。
  - `desktop/src-tauri/src/lib.rs`：只加 `pub mod habit_cards;` 与 8 条 `generate_handler` 注册。
- 新增命令：`habit_candidate_list`、`habit_candidate_decide`、`habit_active_list`、`habit_card_update`、`habit_open_question_list`、`habit_open_question_set_status`、`habit_pending_review_list`、`habit_pending_review_decide`。
- 存储行为：候选上限 100、生效上限 80；每卡一个 JSON，id 只接受字母数字 `-` `_` 且 ≤64；文件不存在按空列表，损坏文件在返回的 `issues` 中给出文件名与原因并计入 `broken`，不猜测修补。写入复用 `worklog_records::write_new_atomically` / `write_replace_atomically`。卡片 `text`、`check.phrases`、`expect` 保存前用 `RulePolicy::reject_keywords` 做 contains-any-of 检查，错误信息不回显命中词。候选列表读取时把 `created_at` 早于 7 天的候选移入 `archive/` 并置 `archived`，不激活。驳回累计以规范化卡片文本的 sha256 记录；`rejection_blocks` 在次数 ≥2 时返回真。停用/归档/复用只移动文件并改状态，不删除卡片。
- 任务书未给出待审定条目的具体字段，本层按 `pending-review.v1` 实现为脱敏预览 `text`、来源 `thread_id`/`message_id`、可选 `reason_code`、`status`（pending/approved/excluded）与时间字段；G2 落盘时仍需自行确保 `text` 是脱敏预览。时间字段统一使用 UTC ISO8601（`YYYY-MM-DDTHH:MM:SSZ`），证据日期保持 `YYYY-MM-DD`。
- 单测覆盖：长度/枚举/上限、红线拒绝且不回显、批量接受与驳回（含改 scope、驳回次数累计到二次阻断）、七天归档、损坏文件明确报告、无新可选字段的旧 v1 读取、原子写失败不留临时/半文件、统计追加与损坏行跳过，以及未决问题、待审定与整理状态往返。
- 实际执行：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1307 passed / 0 failed / 1 ignored**（50.35s；基线 1296，新增 11 项）。
- 命令基线：`python3 tests/verification/desktop/export_commands_2043.py --write tests/verification/desktop/evidence/od-harden-2043/commands-baseline.json` → `count=287 handler=287`，`missing_in_handler` 与 `missing_in_commands` 均为空。更新前该磁盘文件为 277，HEAD 源码为 279，差额 2 是此前未纳入基线的既有命令；本次新增的 8 条为上列 `habit_*` 命令。
- 证据：`tests/verification/desktop/evidence/od-grow-301/g1/verify_output.txt`（全量测试输出与基线命令、命令名列表；证据目录受 `.gitignore` 忽略）。
- 未验证：未新增或运行前端、真实 Tauri 窗口或真实模型链路；未验证真实数据根上的并发写、强制断电恢复与符号链接恶意替换；G2/G3 尚未接入未决问题生成、待审定分流与候选提取。

## OD-GROW-301 G2 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`。开工前快照为 `M PLANS.md`、`M STATUS.md`、`M desktop/src-tauri/src/commands/mod.rs`、`M desktop/src-tauri/src/lib.rs`，另有未跟踪 `desktop/src-tauri/src/commands/habits.rs`、`desktop/src-tauri/src/habit_cards.rs` 与 `docs/od-*.md`。上述既有改动全部保留，未改写其既有内容；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src-tauri/src/consolidation.rs`（新增）：配置、到点判定、纯收集、摘要渲染、待审定与水位的落盘。
  - `desktop/src-tauri/src/commands/consolidation.rs`（新增）：5 个 Tauri 命令；从宿主取应用数据根、当前规则策略与工作记录根。
  - `desktop/src-tauri/src/thread_store.rs`（只增）：新增 `ThreadMetadata`、`ThreadMetadataListing`、`list_metadata()` 与 `read_thread(id)`。目录遍历和单文件结构校验复用原有读取口径，但元数据列表不累积消息正文；选中线程再按 id 读正文。
  - `desktop/src-tauri/src/commands/mod.rs`：只加 `pub mod consolidation;`。
  - `desktop/src-tauri/src/lib.rs`：只加 `pub mod consolidation;` 与 5 条 `generate_handler` 注册。
- 新增命令：`consolidation_tick`、`consolidation_preview`、`consolidation_collect`、`consolidation_config_get`、`consolidation_config_set`。
- 配置与到点：配置在 `<应用数据根>/config/consolidation.v1.json`，字段按任务书，另加 `utc_offset_minutes`（缺省东八区 480）用于本地日期与触发时间；未知字段、损坏 JSON、越界小时/分钟、未知来源、重复来源都整次报错。到点规则为开关关闭永不到点、当前本地时间未过 hour:minute 不到点、`last_run_at` 不是本地今天且已过点则 due、今天已跑则不再 due。
- 状态取舍：直接复用 G1 的 `habits/state/consolidation-state.v1.json`，用已有 `last_run_at` / `last_error` 记每日执行与失败，不新增 `consolidation-schedule.v1`，也不改 G1 结构或函数签名。水位按线程消息下标记录最后扫描位置；未达到阈值或整线程被排除时，仍按任务书把水位推进到当前末尾并计入 `skipped_threads`。
- 收集分流：先按 `updatedAt` 降序选 `tag === "plain"` 的线程并截到 `max_threads`；每线程只处理水位之后的 `role === "user"` 消息。逐消息先做 `mask_privacy`，脱敏后仍含 `reject_keywords` 则进待审定，否则脱敏通过；空文本、缺消息 id、超 10000 字符或无法判定也进待审定。待审定条目只写脱敏预览（≤2000 字符，并把红线关键词替换为“〔已隐去〕”）、线程/消息引用、原因代码与时间，不写原话。
- 预演与落盘的取舍：`collect_digest` 保持纯读取，返回 `DigestPlan`；预演命令只调用它，状态读取走不建目录的只读加载。`consolidation_collect` 再调用 `apply_digest`，先写待审定条目，再把 `digest-<本地日期>.json` 原子替换写入 `habits/state/`，最后保存水位与 `last_run_at`，避免摘要文件失败时水位已推进。失败时记录 `last_error` 并记本次 `last_run_at`，避免同一天自动反复重试。
- 摘要结构样例（合成数据）：`【线程】合成线程` 后接 `- [m-0] 以后每天先写三行总结`；项目任务段为 `【项目任务状态变化】` 后接 `- 合成任务：待开始 → 进行中`。超过 `max_digest_chars` 时按整条消息截断，统计 `truncated`（线程数）与 `truncated_messages`。
- 实际执行：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1317 passed / 0 failed / 1 ignored**（52.38s；基线 1307，新增 10 项）。
- 命令基线：`python3 tests/verification/desktop/export_commands_2043.py --write tests/verification/desktop/evidence/od-harden-2043/commands-baseline.json` → `count=292 handler=292`，`missing_in_handler` 与 `missing_in_commands` 均为空；相对 G1 的 287 增加 5 条，即上列 `consolidation_*` 命令。
- 证据：`tests/verification/desktop/evidence/od-grow-301/g2/verify_output.txt`（全量测试输出、命令基线输出与新增命令名；证据目录受 `.gitignore` 忽略）。
- 未验证：未运行前端或真实 Tauri 窗口；未调用真实模型；未验证真实数据根上的并发写、强制断电恢复与符号链接恶意替换；项目任务只依赖 `task_store` 的本地状态变化记录，真实历史数据未抽样核对。

## OD-GROW-301 G3 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`。开工前快照为 `M PLANS.md`、`M STATUS.md`、`M desktop/src-tauri/src/commands/mod.rs`、`M desktop/src-tauri/src/lib.rs`、`M desktop/src-tauri/src/thread_store.rs`，另有未跟踪 G1/G2 模块与 `docs/od-*.md`。既有改动全部保留；未提交、未推送、未打标签。
- 改动文件：
  - `engine/omnidesk_engine/habit_extract.py`（新增）：`build_habit_prompt(digest_text, threads_meta)` 只组装中文提示词，写明接地规则、`omnidesk.habit-candidates.v1` 契约、禁入表达、信号词与未决表达、数量/长度/check 限制；不调模型、不落盘、不调用工具。另提供只输出 `schema + prompt` 的 stdin CLI。
  - `engine/omnidesk_engine/habit_extract_test.py`（新增）：4 个 unittest，覆盖接地规则关键词、输出契约字段、信号词/未决词和 CLI 输出。
  - `desktop/src-tauri/src/habit_extract.rs`（新增）：模型文本解析、代码围栏容忍、逐条候选/未决问题校验、32 组表驱动合成样例、`StagedHabitCandidate::into_card` 与 `StagedOpenQuestion::into_question`。
  - `desktop/src-tauri/src/lib.rs`：为让 `cargo test --lib` 实际编译新模块，只新增一行 `pub mod habit_extract;`；既有未提交改动未改写。
  - `tests/verification/desktop/evidence/od-grow-301/g3/verify_output.txt`：两端完整测试输出与命令、结果摘要。
- 校验口径：候选来源必须是本次摘要中的用户消息；cue 必须是原文子串且整体命中 12 项信号词白名单之一。无白名单信号时，只有 `existing.repeated_texts` 中同一文本累计 ≥2 天才收为 `origin=repeated`，否则 `NO_SIGNAL`。`global` 仅在同一条来源消息出现「所有/一律/任何」时保留，否则降级为来源线程的 scene 并记录 `GLOBAL_DOWNGRADED`。text 非空、无控制字符且 ≤60 字；check 仅接受三种结构化形态，短语非空且每条 ≤20 字，段落上限 10..=2000。text/check.phrases 命中红线返回 `REDLINE`，`RejectedEntry` 只含原因编号。候选/生效卡同文本拒绝 `DUPLICATE`，驳回次数 ≥2 拒绝 `REJECTED_BEFORE`。open_questions 只查用户来源、非空/无控制字符/≤80 字，并要求命中 7 项未决表达之一。两类数组都只保留先到的 10 条，超出逐条记 `OVERFLOW`。
- 判定取舍：为满足任务书中的“假设、转述、AI 建议未采纳必须被拒”样例，宿主在文本层增加明确标记词检查（如「假设/假如/如果/同事说/听说/AI 建议/我认为」），命中按 `NO_SIGNAL` 拒绝；不做语气或话题推断。通过与重复候选均附 `CandidateVerdict::new("habit-candidate.v1", …)`，固定五项检查 `cue_in_source`、`signal_whitelisted`、`scope_valid`、`not_duplicate`、`redline_clean`。
- 实际执行：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1319 passed / 0 failed / 1 ignored**（51.79s；相较 G2 的 1317 新增 2 个测试，其中表驱动测试含 32 组样例）。
- Python 组织探测：仓库未找到 `requirements*.txt`、`pyproject.toml`、`pytest.ini`；`/usr/bin/python3` 与 Homebrew `python3.13` 的 `pytest` 均为 `No module named pytest`。按任务书允许的回退路径采用 engine 现有 unittest 风格。`cd engine && python3 -m unittest discover -s omnidesk_engine -p habit_extract_test.py -v` → **Ran 4 tests / OK**。
- 未验证：未调用真实模型，未运行前端或 Tauri 窗口，未抽样真实摘要；未新增依赖，未修改 `runtime/`、`scheduler.rs`、`habit_cards.rs`、`consolidation.rs` 既有函数或既有测试断言。

## OD-GROW-301 H3 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`。开工时 `git status` 为 `M PLANS.md`、`M STATUS.md`、`M desktop/src-tauri/src/commands/mod.rs`、`M desktop/src-tauri/src/lib.rs`、`M desktop/src-tauri/src/thread_store.rs`，另有未跟踪 `desktop/src-tauri/src/commands/consolidation.rs`、`desktop/src-tauri/src/commands/habits.rs`、`desktop/src-tauri/src/consolidation.rs`、`desktop/src-tauri/src/habit_cards.rs`、`desktop/src-tauri/src/habit_extract.rs`、`engine/omnidesk_engine/habit_extract*.py` 与 `docs/od-*.md`。上述既有改动全部保留，未改写其既有内容；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src-tauri/src/habit_checks.rs`（新增）：`check_habits(scope_scene, body, store) -> HabitCheckReport` 与 `store_for_work_root(work_root) -> Option<HabitStore>`。报告只含 `checked_cards` 与 `violations {card_id, kind, detail_code}`，不含正文或命中的原文；违规经 `HabitStore::append_stat` 追加 `type=violation` 行，统计失败不回滚结论、不阻断主流程。
  - `desktop/src-tauri/src/lib.rs`：只新增一行 `pub mod habit_checks;`；未加命令或改既有函数。
  - `desktop/src-tauri/src/review_store.rs`：`PeriodReviewReceipt` 新增 `habits` 字段；`confirm_review` 在原有校验与落盘通过后，以 `scope=report` 调用检查并随回执返回。已确认分支返回空报告，不重复记统计；既有确认、幂等与防覆盖逻辑不变。另新增 1 个集成测试。
  - `desktop/src-tauri/src/writing_review.rs`：`WritingReviewPrompt` 新增 `habits` 字段；`build_review_prompt` 在原有校验通过、拼完提示词后，以 `scope=material_writing` 对已确认章节正文调用检查。既有十维提示词、状态门禁与文档只读行为不变。另新增 1 个集成测试。
  - `tests/verification/desktop/evidence/od-grow-301/h3/verify_output.txt`：全量测试原始输出。
- 检查口径：只读 `list_active()` 中 `status=active` 且带 `check` 的卡；`global` 始终参与，`scene` 仅在 `scope_scene` 相等时参与。`banned_phrases` 任一命中、`paragraph_max_chars` 任一按空行分段后的段落字符数超过上限（等于上限不报）、`required_phrases` 任一缺失，各记一条违规；`detail_code` 固定为 `banned_phrase_match` / `paragraph_over_limit` / `required_phrase_missing`，同时作为统计 `reason_code`。
- 存储定位取舍：`review_store::confirm_review` 与 `writing_review::build_review_prompt` 只持有工作根，不持有应用数据根，且任务书限定 `lib.rs` 只加模块声明。为此 `store_for_work_root` 按只读探测顺序寻找 `<OMNIDESK_APP_DATA_ROOT>/habits`、工作根同级 `app/habits`、工作根内 `habits`，全部不存在时返回空报告；不创建目录。默认 `~/OmniDesk/work` 布局可命中同级 `~/OmniDesk/app/habits`；自定义工作根若不在应用根同侧，需要显式设置 `OMNIDESK_APP_DATA_ROOT` 才会执行检查。这是为遵守改动边界而采用的约定，未做真实自定义根验证。
- 实际执行：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1329 passed / 0 failed / 1 ignored**（52.79s；G3 基线 1319，新增 8 个检查单测与 2 个接入集成测试）。
- 未验证：未运行真实 Tauri 窗口、前端或真实模型链路；未验证自定义工作根下的存储定位；未验证符号链接替换与并发追加统计。未新增依赖，未修改 `runtime/`、`engine/`、前端、`habit_cards.rs` 既有函数或既有测试断言。

## OD-GROW-301 H5 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`。开工时 `git status` 为 `M PLANS.md`、`M STATUS.md`、`M desktop/src-tauri/src/commands/mod.rs`、`M desktop/src-tauri/src/lib.rs`、`M desktop/src-tauri/src/review_store.rs`、`M desktop/src-tauri/src/thread_store.rs`、`M desktop/src-tauri/src/writing_review.rs`，另有未跟踪 G1/G2/H3 模块、`engine/omnidesk_engine/habit_extract*.py` 与 `docs/od-*.md`。上述既有改动全部保留，未改写其内容；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src/reviewRetry.ts`（新增）：`nextReviewStep`、`isRetryableReviewFailure`、`reviewRetryPrompt` 纯函数。
  - `desktop/src/reviewRetry.test.mjs`（新增）：表驱动覆盖全部状态转移、取消/准入拒绝、attempt 边界与提示词追加。
  - `desktop/src/shell/ReviewArchive.tsx`：`regenerate` 的提交→轮询→取结果→宿主校验改为最多两轮循环；首轮可重试失败时追加失败原因后重新提交一次，并显示一次「第一次生成未成功，正在重试…」；第二轮仍失败沿用 `setNotice({kind:"err"})`，不弹系统弹窗。既有生成、确认、导出、筛选行为不变。
  - `tests/verification/desktop/evidence/od-grow-301/h5/verify_output.txt`：本次三条验证命令的原始输出与结果摘要。
- 重试口径：仅模型未完成、正文校验失败、结果为空进入重试。用户取消（`cancelled`/取消/中断）与准入拒绝（`endpoint_unconfirmed`、`endpoint_unknown`、`red_line_rejected`、`pending_write_blocked`、`workspace_unavailable`、准入/红线）优先排除；未知错误保守地不重试。重试提示词为原提示词末尾追加「上一次尝试失败，原因：…；请修正后重新输出」。
- 统计边界：按任务书不新增宿主接口，本轮重试次数只记录在本条执行记录；习惯统计文件接入留给 G11。
- 实际执行：`cd desktop && npm run test` → **101 passed / 0 failed**（基线 97，新增 4 项）；`npx tsc --noEmit` → 0；`npm run build` → 成功（退出码 0，仅有既有 chunk 体积警告）。
- 未验证：真实模型端到端重试行为未验证，需要实机与真实模型端点；未运行真实 Tauri 窗口。未新增依赖，未修改 `runtime/`、`engine/`、`desktop/src-tauri/` 或既有测试断言。

## OD-GROW-301 G4 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`。开工时 `git status` 为 `M PLANS.md`、`M STATUS.md`、`M desktop/src-tauri/src/commands/mod.rs`、`M desktop/src-tauri/src/lib.rs`、`M desktop/src-tauri/src/review_store.rs`、`M desktop/src-tauri/src/thread_store.rs`、`M desktop/src-tauri/src/writing_review.rs`、`M desktop/src/shell/ReviewArchive.tsx`，另有未跟踪 G1/G2/G3/H3/H5 模块、`engine/omnidesk_engine/habit_extract*.py`、`desktop/src/reviewRetry*` 与 `docs/od-*.md`。上述既有改动全部保留，未改写既有内容；未提交、未推送、未打标签。
- 开工前核实：① `desktop/src/api.ts` 原先没有任何 consolidation 封装或类型，G2 只提供了宿主命令；② `sceneThreads.ts` 的 `inflightTaskId` 是逐线程的在途任务号，`HomeChat` 拿到任务号后写入、终态后清空。工作台现有唯一可读的全局“进行中对话”判定是 `listThreads().some(t => Boolean(t.inflightTaskId))`；G4 直接复用它，不新增全局 store；③ `consolidation.rs::collect_digest` 的 `DigestPlan` 与 `DigestRecord` 都有 `digest_text` 和 `stats`，但宿主命令 `consolidation_collect` 实际只返回 `DigestStats`（字段为 `thread_count`、`message_count`、`char_count`、`skipped_threads`、`redline_masked`、`redline_pending`、`truncated`、`truncated_messages`、`truncated_project_changes`、`project_changes`）。
- 改动文件：
  - `desktop/src/consolidationFlow.ts`（新增）：纯函数 `nextStep(state)`、本地日期换算、每日本地存储键与 G5 通知事件/类型。
  - `desktop/src/consolidationFlow.test.mjs`（新增）：28 行表驱动全转移，加“今日已跑对所有相位都回 idle”、纯函数不改入参、UTC 偏移日期换算 3 个补充测试。
  - `desktop/src/shell/ConsolidationRunner.tsx`（新增）：`return null`，每 30 秒调 `consolidation_tick`；到点且无在途对话才执行；宿主失败、提交被拒、终态失败与 1800 秒超时都只 `console.warn` 并派发 `omnidesk:consolidation-notice`，不弹系统错误框。
  - `desktop/src/api.ts`（只增）：`consolidation_config_get/tick/preview/collect` 封装及 `ConsolidationConfig`、`ConsolidationDigestStats`、`ConsolidationTick`、`ConsolidationCollectResult` 类型；未改既有函数。
  - `desktop/src/shell/ScenePage.tsx`（只增两行）：在 `<AnnouncementBar />` 同级挂载 `<ConsolidationRunner />`。
  - `tests/verification/desktop/evidence/od-grow-301/g4/verify_output.txt`：三条验证命令的原始输出。
- 状态机转移（代码常量，不写进提示词）：`attemptDate === today` 是全相位最高优先闸门，返回 `idle`。其余：`idle + due + 无对话 → prepare`；`idle + 无对话但未到点/有对话 → idle`；`prepare + 空摘要 → done`；`prepare + 非空 → submit`；`submit + 被拒 → failed`；`submit + 成功 → await`；`await + 未终态 → await`；`await + 成功终态 → ingest`；`await + 失败终态/超时 → failed`；`ingest → done`；`done → done`；`failed → failed`。Runner 从 localStorage 的 `omnidesk.consolidation.lastRun` 恢复当日闸门；昨天或无记录时由宿主 tick 决定是否补跑。
- 提交与终态：`intent=habit_consolidation`，provider/model 取 `consolidation_config_get`（配置为空时按宿主当前路由传 null）。整理配置当前没有 `reasoning_effort` 字段；按任务书缺省 `low`，模型名含 `eco` 时用 `off`，配置将来有该字段则直接使用。提交仍走既有 `effortEvidence` 门，目录/Profile 没有声明该档位时会静默降级为不显式传档位。任务终态成功后读 `taskResult.content`，按 Unicode 字符截到 20000，写 `omnidesk.consolidation.lastOutput`；`console.info` 只报 thread/message/char 与输出字符数，不打印正文。
- 摘要正文缺口：宿主 `consolidation_collect` 不返回 `digest_text`，且没有普通文件读取命令。G4 未改宿主，`api.ts` 在宿主未直接返回正文时，用 `task_local_root_info` 取 `app_data_root`，按整理配置的 UTC 偏移定位 `<应用数据根>/habits/state/digest-<date>.json`，再经既有只读 `task_read_attachment` 取回并校验 `schema=omnidesk.digest.v1` 与 `digest_text`。这是前端适配，真实摘要文件读取尚未在 Tauri 窗口验证；G5 若在宿主增加直接返回正文或专用读取命令，应移除该回退。
- 宿主校验与暂存缺口：G3 的解析/校验没有以 `habit_output_validate` 或 `consolidation_apply_output` 命令暴露，G4 按任务书方案 B 只做“提交→等终态→读结果→记录 raw 并通知”，不伪造校验或暂存；真正校验、纳入生效卡与待审定处理留给 G5 配合宿主补齐。
- 实际执行：`cd desktop && npm test` → **105 passed / 0 failed / 0 skipped**（基线 101，新增 4 个测试；表内 28 行转移）；`npx tsc --noEmit` → exit 0；`npm run build` → exit 0（`tsc` 通过，vite 1.22s，仅有既有 chunk 体积警告）。证据见 `tests/verification/desktop/evidence/od-grow-301/g4/verify_output.txt`。
- 未验证：真实模型端到端整理未运行，需用户实机执行一次并回报；真实 Tauri 窗口中的 30 秒定时、摘要文件回退读取、localStorage 写入与 G5 事件消费未验证。未新增依赖，未修改 `runtime/`、`engine/`、`desktop/src-tauri/` 或既有测试断言。
+
## OD-GROW-301 G5 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`。开工时 `git status` 为 `M PLANS.md`、`M STATUS.md`、`M desktop/src-tauri/src/commands/mod.rs`、`M desktop/src-tauri/src/lib.rs`、`M desktop/src-tauri/src/review_store.rs`、`M desktop/src-tauri/src/thread_store.rs`、`M desktop/src-tauri/src/writing_review.rs`、`M desktop/src/api.ts`、`M desktop/src/shell/ReviewArchive.tsx`、`M desktop/src/shell/ScenePage.tsx`，另有未跟踪 G1/G2/G3/H3/H5 与 G4 前端模块、`engine/omnidesk_engine/habit_extract*.py`、`docs/od-*.md`。既有改动全部保留；未提交、未推送、未打标签。
- UI 参考：用 `screenshots-design_inspiration` 的 `search_inspiration` 检索并查看了 Superhuman 收件箱与 Asana 任务列表截图，另检索到 Branch Messenger 通知徽标页元数据。借鉴点：收件箱式列表把标题、来源与操作收在同一行，右侧只留一个主动作；分组计数与状态徽标沿用暖纸卡片层级，7 天未动的问题用错误色先被看见。
- 改动文件（行数为相对开工前快照的本轮改动）：
  - `desktop/src/habitNotice.ts`（新增，114 行）：铃铛计数、7 天未决排序、当天新候选、首页关闭日期与打开事件。
  - `desktop/src/habitNotice.test.mjs`（新增，109 行）：6 项测试覆盖无候选、计数、处理完消失、7 天排序、当天关闭与新一天重置、UTC 跨午夜。
  - `desktop/src/habitCardsView.ts`（新增，62 行）：候选范围切换、批量接受与驳回原因代码纯函数。
  - `desktop/src/habitCardsView.test.mjs`（新增，73 行）：4 项测试覆盖默认/切换范围、按编号保持选择、批量接受与可选原因。
  - `desktop/src/shell/HabitDigestPanel.tsx`（新增，312 行）：候选、未决问题、待审定三个区；接受/驳回/全部接受与批准/排除直接走 G1 命令。
  - `desktop/src/shell/HabitCardsPage.tsx`（新增，335 行）：习惯浮层，含生效卡停用/归档/复用、整理开关/时间/服务商/模型与保存；命中与违反次数如实标注「待 G11」。
  - `desktop/src/shell/HabitHomeNotice.tsx`（新增，97 行）：首页顶部条件提醒条；当天只弹一次，查看或关闭写 `omnidesk.habits.homeNoticeDismissed`。
  - `desktop/src/types.ts`：+40 行，新增 HabitScope、HabitCheck、HabitCard、OpenQuestion、PendingReviewItem、HabitListing 与候选决定类型。
  - `desktop/src/api.ts`：+26 行，新增 `consolidation.setConfig` 与 8 个 `habits` 命令封装；G4 既有 consolidation 段未改写。
  - `desktop/src/shell/NotificationCenter.tsx`：+22/-4 行（净 +18），新增 `habits` 条目与刷新事件；confirm/task/review 既有逻辑未改。
  - `desktop/src/shell/HomeCanvas.tsx`：+6/-2 行（净 +4），在 upper 槽的 `conversationStarted` 条件外挂载首页提醒条。
  - `desktop/src/shell/TopNav.tsx`：+14 行，增加习惯图标入口并挂载 `HabitsOverlayHost`。
- 进入点取舍：任务书允许 Dock 或 TopNav；本轮选 TopNav 与首页提醒，习惯浮层不改 `Scene`/应用路由，报告页与 `WorkRecordPage` 均未改动。HabitDigestPanel 从首页提醒或顶栏进入浮层，符合「首页入口」备选。
- 功能口径：铃铛每天只加一条 `kind=habits`，数字为「N 条习惯候选，M 个未决问题」，候选或问题清零后消失；首页只在当天存在 `candidate` 且当天未关闭时显示。面板范围切换写入候选决定的 `scope`；驳回原因可选「太绝对」(`too_absolute`)/「不是我说的」(`not_mine`)/不填，驳回哈希由宿主记录。整理开关默认取宿主配置（默认关闭），开启后显示读取范围「未分类对话、项目任务」。
- 实际执行：`cd desktop && npm run test` → **115 passed / 0 failed / 0 skipped**（基线 105，新增 10 项；新增 6+4 项）；`npx tsc --noEmit` → exit 0；`npm run build` → exit 0（vite 1.21s，仅有既有 chunk 体积警告）。证据：`tests/verification/desktop/evidence/od-grow-301/g5/verify_output.txt`。
- 未验证：真实 Tauri 窗口与实机视觉未验证；未用真实候选数据走一遍接受/驳回/待审定/整理开关；行为依赖宿主 G1 命令与 localStorage，均只经纯函数单测与编译验证。未新增依赖，未修改 `runtime/`、`engine/`、`desktop/src-tauri/`、既有测试断言或模型路由与凭证。

## OD-GROW-301 G6 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`（main）。开工 `git status` 为 `M PLANS.md`、`M STATUS.md`、`M desktop/src-tauri/src/commands/mod.rs`、`M desktop/src-tauri/src/lib.rs`、`M desktop/src-tauri/src/review_store.rs`、`M desktop/src-tauri/src/thread_store.rs`、`M desktop/src-tauri/src/writing_review.rs`、`M desktop/src/api.ts`、`M desktop/src/shell/HomeCanvas.tsx`、`M desktop/src/shell/NotificationCenter.tsx`、`M desktop/src/shell/ReviewArchive.tsx`、`M desktop/src/shell/ScenePage.tsx`、`M desktop/src/shell/TopNav.tsx`、`M desktop/src/types.ts`，另有 G1-G5/H3/H5 未跟踪模块、`engine/omnidesk_engine/habit_extract*.py`、`docs/od-*.md`。以上既有改动全部保留；未改写 `desktop/src-tauri/*`、`engine/`、`HabitCardsPage`/`HabitDigestPanel`/`habitNotice`/`habitCardsView`/`consolidationFlow`；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src/habitInjection.ts`（新增，126 行）：纯函数 `selectHabitCards` / `injectHabits`（过滤 active 与 global/当前场景、最近确认优先、≤8 张、卡文字合计 ≤600、超限整张跳过并记 `skipped`、固定说明段 + 每卡「- {text}」、空列表原样返回）；`injectionLogLine` / `rollInjectionLog` / `recordHabitInjection` 负责 localStorage 滚动记录（键 `omnidesk.habits.lastInjected`，≤50 行，含日期与卡编号，不写卡正文）。
  - `desktop/src/habitInjection.test.mjs`（新增，9 项测试）：生效/候选/停用/归档筛选、global 与场景匹配、最近确认排序、8 张上限、600 字上限与超出跳过、空 active 列表（开关关闭模拟）不注入、固定文案与每卡一行、50 行滚动与日期/编号格式、localStorage 写入与失败不外抛。
  - `desktop/src/shell/HomeChat.tsx`（+18/−2 行，共 20 行改动）：新增模块级 `withHabitInjection`，在两个用户提交路径的 `composeConversationPrompt` 之后调用；读取 `habits.activeList()` 失败时 `console.warn` 并原样提交，不阻塞对话。
  - `tests/verification/desktop/evidence/od-grow-301/g6/verify_output.txt`（新增）：三条验证命令的原始输出与退出码。
- 注入接入点取舍：未改 `conversationPrompt.ts`。`composeConversationPrompt` 保持同步纯组装；注入放在 HomeChat 的提交边缘（普通对话提交，以及周报/周期复盘草稿提交），分类调用（`mode=classify`）不注入，避免习惯卡影响意图判定。理由是注入需要异步读 `habits.activeList()`，放在提交前最贴近“本轮发什么给模型”，且 HomeChat 改动控制在 20 行。
- 功能口径：只注入 `status=active` 且 `scope=global` 或 `scope.scene===activeTag` 的卡；总开关按“active 卡数量 > 0 即注入”实现，active 列表为空即不注入；固定说明段用任务书原文；卡片列表为空时提示词原样返回。
- 统计落盘缺口（如实标注）：宿主没有“习惯注入统计”追加命令，本任务按任务书在 localStorage 键 `omnidesk.habits.lastInjected` 追加“日期\t卡编号”行并滚动保留 50 行；**正式统计落盘待 G11 宿主命令**。G11 补命令后应迁移并移除该前端记录。
- 实际执行：`cd desktop && npm run test` → **124 passed / 0 failed / 0 skipped**（基线 115，新增 9 项）；`npx tsc --noEmit` → exit 0；`npm run build` → exit 0（`tsc` 通过，vite 1.23s，仅有既有 chunk 体积警告）。证据见 `tests/verification/desktop/evidence/od-grow-301/g6/verify_output.txt`。
- 未验证：未在真实 Tauri 窗口里提交一轮对话，未验证 `habit_active_list` 的实际返回、注入后模型的真实行为、localStorage 写入与 50 行滚动在应用内的表现；以上只经纯函数单测、类型检查与构建验证。未新增依赖，未修改既有测试断言、`runtime/`、`engine/`、`desktop/src-tauri/` 或模型路由与凭证。

## OD-GROW-301 G10 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`（main）。开工 `git status` 与 G5/G6 记录一致：`M PLANS.md`、`M STATUS.md`、既有宿主与前端改动，以及 G1-G6/H3/H5 未跟踪模块、`engine/omnidesk_engine/habit_extract*.py`、`docs/od-*.md`。既有改动全部保留；未改 `PLANS.md` 既有内容、前端、`engine/`、`runtime/`、`desktop/src-tauri/src/commands/*` 或 `docs/od-*.md`；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src-tauri/src/consolidation.rs`：新增 `QUESTION_BIGRAM_THRESHOLD = 0.34`、`question_bigram_overlap(question_text, message_text) -> f64`、`refresh_open_questions(paths, plan, now) -> Result<usize, String>`；`apply_digest` 在待审定落盘后、摘要与水位写入前调用刷新。既有函数签名未改。
  - `desktop/src-tauri/src/habit_cards.rs`：新增 `QUESTION_STALE_DAYS = 7`、`touch_open_question_last_seen(id, now)`（仅 open 状态写 `last_seen`，复用 `write_replace_atomically`）、`open_question_idle_days(last_seen, today, utc_offset_minutes)`、`stale_open_questions(now, utc_offset_minutes)` 与 `StaleOpenQuestion(List)`；既有函数签名未改。
  - `tests/verification/desktop/evidence/od-grow-301/g10/verify_output.txt`（新增）：全量 cargo 测试原始输出。
- 判定口径：字符二元组按问题文本去重集合计算，分母为问题自身二元组数；ASCII 大小写归一、连续空白压成一个空格。重叠 `>= 0.34` 命中，34/100 与常量字面值相等；33/100 不命中。刷新只更新既有 `open` 问题文件，不新建、不改 `closed`/`dropped`。搁置按本地日期差 `> 7` 天（第 8 天起）进入提醒列表，第 7 天不算。
- 与既有前端的口径差异：G5 的 `desktop/src/habitNotice.ts` 用 `idleDays >= 7` 标记前端逾期，宿主 G10 按任务书用 `> 7`。任务书要求本轮不改前端，因此未动；后续 G5/铃铛若改用宿主 `stale_open_questions`，需统一到第 8 天。
- 实际执行：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1334 passed / 0 failed / 1 ignored**（共 1335 项；G10 新增 5 项：consolidation 3 项、habit_cards 2 项；满足 `>= 1329 + 5`）。证据见 `tests/verification/desktop/evidence/od-grow-301/g10/verify_output.txt`。
- 未验证：未在真实应用数据目录中跑一轮真实整理摘要，未用真实未决问题验证提及刷新与搁置提醒的端到端表现；只经合成数据单测。`cargo fmt --check` 未运行成功（当前工具链未安装 rustfmt），未改动依赖或安装组件。

## OD-GROW-301 H6+G11 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`（main）。开工 `git status` 与 G10 记录一致：`M PLANS.md`、`M STATUS.md`、既有宿主与前端改动，以及 G1-G10/H3/H5 未跟踪模块、`engine/omnidesk_engine/habit_extract*.py`、`docs/od-*.md`。既有改动全部保留；未改 `PLANS.md` 存量内容、`runtime/`、`engine/`、`desktop/src-tauri/`、`docs/od-*.md` 或既有测试断言；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src/metricsView.ts`（新增）：纯函数 `buildMetrics` / `rejectionTop` / `formatMetric` / `formatRejectionTop` / `formatViolationTrend` 与输入输出类型。接受率按「接受 /（接受 + 驳回）」计算（%，保留 1 位小数）；驳回原因按次数降序取前三，同次数按原因码升序；违反趋势按卡片 `lastDate` 合并计数后取最近 7 个日期升序；整理成功率按「成功 /（成功 + 失败）」计算。任一必需数字缺失或分母为 0 返回 `null` / `[]`，界面显示 `—`，不估算、不补零。
  - `desktop/src/metricsView.test.mjs`（新增，5 项）：接受率（含 0 分母与缺失返回 `null`）、Top3 排序与同次数稳定排序、违反趋势同日合并与 7 日期窗口、整理成功率与缺失、`—` 与带单位格式化。
  - `desktop/src/shell/HabitCardsPage.tsx`（仅追加 30 行，≤30）：底部只读指标区「接受率 / 驳回原因 Top3 / 违反趋势（最近 7 次）/ 整理成功率」，显示本地 `omnidesk.habits.lastInjected` 的最近注入行数，并注明「统计读取待 G11 宿主命令；缺失显示 —」。不拦截任何流程，不改既有区块。
  - `tests/verification/desktop/run_habit_regression.py`（新增，141 行）：习惯回归入口。依次运行 `cargo test --lib --offline -- habit_extract`、`cargo test --lib --offline -- habit_checks`、`cd engine && python3 -m unittest discover -s omnidesk_engine -p habit_extract_test.py`；任一失败退出 1，输出每组 PASS/FAIL 与通过数。自动在 `PATH` 与 `~/.cargo/bin` 查找 cargo；`--output <文件>` 可把完整输出落盘。用法：`python3 tests/verification/desktop/run_habit_regression.py [--output <文件>]`。后续改提取提示词、信号词白名单、宿主校验、规则种类或相关 SKILL 前先跑此脚本。
  - `tests/verification/desktop/evidence/od-grow-301/h6-g11/verify_output.txt`（新增）：回归脚本完整输出 + `npm run test` / `npx tsc --noEmit` / `npm run build` 原始输出与退出码。
- 回归集口径：`habit_extract` 过滤命中 2 项测试，其中表驱动宿主校验测试内部含 ≥20 组合成样例（明确表述、一次性请求、假设、转述、AI 建议未采纳、编号不存在、信号词不在原文、global 无据降级、重复与已驳回、红线词等）；`habit_checks` 10 项（含复盘/写作路径软检查）；engine 提示词 4 项。合计 16 项，未新增样例断言，仅提供统一入口。
- 指标数据来源缺口（如实标注）：宿主 G1 目前没有候选决定、`habits-stats.jsonl`、整理统计的读取命令（`commands/habits.rs` 只有 candidate/active/open-question/pending-review 列表与决定命令）。因此本页接受率、驳回 Top3、违反趋势、整理成功率均显示 `—`，并注明待 G11 宿主命令；只展示真实可读的 `habits.activeList()` 生效卡与 localStorage 最近注入行数，不伪造数字。
- 实际执行：`cd desktop && npm run test` → **129 passed / 0 failed / 0 skipped**（基线 124，新增 5 项）；`npx tsc --noEmit` → exit 0；`npm run build` → exit 0（仅有既有 chunk 体积警告）；`python3 tests/verification/desktop/run_habit_regression.py` → **habit_extract 2 + habit_checks 10 + engine 4 = 16 passed / 0 failed**。证据见 `tests/verification/desktop/evidence/od-grow-301/h6-g11/verify_output.txt`。
- 未验证：未在真实 Tauri 窗口查看指标区布局与真实统计；本轮只跑定向 cargo 过滤测试，未重跑全量 `cargo test --lib`；宿主统计读取命令与真实候选决定数据接入仍未存在。

## OD-GROW-301 H7+G12 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`（main）。开工 `git status` 与 H6+G11 记录一致：`M PLANS.md`、`M STATUS.md`、既有宿主改动 `M desktop/src-tauri/src/commands/mod.rs`、`M desktop/src-tauri/src/lib.rs`、`M review_store.rs`、`M thread_store.rs`、`M writing_review.rs`，既有前端改动 `M desktop/src/api.ts`、`M HomeCanvas.tsx`、`M HomeChat.tsx`、`M NotificationCenter.tsx`、`M ReviewArchive.tsx`、`M ScenePage.tsx`、`M TopNav.tsx`、`M types.ts`，以及 G1-G10/H3/H5/H6+G11 未跟踪模块、`engine/omnidesk_engine/habit_extract*.py`、`docs/od-*.md`、`tests/verification/desktop/run_habit_regression.py`。既有改动全部保留；未改 `PLANS.md` 既有内容、`STATUS.md` 既有内容、前端、`engine/`、`runtime/`、`docs/od-*.md` 或 `consolidation.rs`；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src-tauri/src/habit_audit.rs`（新增）：H7 纯逻辑 `audit_quote` / `audit_active_quotes` / `record_quote_audits` / `read_quote_audits`，G12 纯逻辑 `review_active_cards`；报告与统计不含正文。15 项合成数据单测。
  - `desktop/src-tauri/src/lib.rs`（仅加 1 行）：`pub mod habit_audit;`。
  - `desktop/src-tauri/src/habit_cards.rs`（只加不改）：新增 `STAT_QUOTE_AUDIT` 常量并加入既有允许类型列表，新增 `stats_path()` 与 `utc_iso8601_seconds()` 读取辅助；既有函数体与签名未改。
  - `tests/verification/desktop/evidence/od-grow-301/h7-g12/verify_output.txt`（新增）：全量 cargo 测试原始输出与最终 `test result`。
- H7 口径：只审 `status=active` 且带 `BannedPhrases` / `RequiredPhrases` 的卡；统计中有该卡 `violation` 行才抽查，否则 Skipped。禁用短语卡在样本正文找到任一声称短语即 Matched；必备短语卡把 violation 视为「缺少必备短语」，任一样本缺少任一必备短语即 Matched，所有样本都包含全部必备短语则 Mismatched。无样本返回 Unknown，`checked=0` 时比例为 None，不做估算。只给 Matched / Mismatched 写统计行，`unknown` / `skipped` 不写。
- 统计行结构：`{"ts": ..., "type": "quote_audit", "id": <card_id>, "mismatch": <bool>}`。既有 `HabitStatRow` 不能承载布尔字段，本轮未改其结构，改由审计模块经 `stats_path()` 追加；已把 `quote_audit` 加入允许统计类型，`read_stats` 可读取该行（未知的 `mismatch` 字段在既有列表视图中被忽略，布尔值由审计模块的 `read_quote_audits` 读取）。
- 汇总结构：`QuoteAuditReport { checked, mismatches, skipped, unknown, items: [QuoteAuditResult { card_id, status }] }`，`mismatch_ratio() = mismatches / checked`，`checked=0` 返回 `None`。
- G12 口径：只处理生效卡；`review_due` 要求统计窗口内无该卡 `inject` 记录且 `last_seen_at` 严格早于 90 天前（第 90 天整不发标记，第 91 天发）；`conflict` 只在 scope 完全相同的两张卡之间判定，同一短语一边在 `banned_phrases`、另一边在 `required_phrases` 时双方都标；跨 scope 不计。报告只列至少有一个标记的卡：`{ items: [{ card_id, flags: { review_due, conflict } }] }`。`stats_cursor` 为调用方传入的统计行窗口，函数只读、不改文件、不改卡。
- 实际执行：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1349 passed / 0 failed / 1 ignored**（共 1350 项；基线 1334 passed / 1 ignored，新增 15 项；满足 ≥ 1334 + 15）。证据见 `tests/verification/desktop/evidence/od-grow-301/h7-g12/verify_output.txt`。
- 未验证：G12 复核函数尚未接入整理运行器调用点，本轮只交付纯逻辑与单测；未在真实应用数据目录上跑一轮抽查或复核，未做界面展示；报告与统计只经合成数据验证。未新增依赖，未修改既有测试断言。

## OD-GROW-301 T13 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`（main）。开工 `git status` 与 H7+G12 记录一致，既有未提交/未跟踪改动全部保留（`PLANS.md`、`STATUS.md`、`desktop/src-tauri/*`、`desktop/src/*`、`docs/od-*.md`、`engine/`、`tests/verification/desktop/run_habit_regression.py` 等）；未改 `runtime/`、`engine/`、`desktop/src-tauri/`；未提交、未推送、未打标签。
- **实施前事实**：HEAD 的 `desktop/src/workIntent.ts` 已含 `PERIOD_REVIEW_TABLE`（`{ keyword, period, offset }`）与「上月复盘 / 复盘上月 / 上个月复盘」，由 0.2.45（`cd6b847698`）引入；`PERIOD_REVIEW_KEYWORDS` 仅作派生字符串视图，周期判断已无 `slice` 下标。本轮不再重复造表，只补齐词条优先级与可验证性。
- 改动文件：
  - `desktop/src/workIntent.ts`（新增 `matchPeriodReviewEntry`）：按最长命中选词条，插入「月复盘」这类短词不会遮住「上月复盘」的长词偏移；`detectPeriodReviewTarget` 增加可选第 4 参数 `table`（默认 `PERIOD_REVIEW_TABLE`，既有调用行为不变），周偏移在词表无命中时回退到 `matched.offset`。默认调用结果与改动前一致。
  - `desktop/src/workIntent.test.mjs`（只追加 3 项，既有 9 项断言未改）：每个词条在固定日期的精确起止与标签；插入短词后的最长匹配；「上月复盘」年末/年初跨年（2026-12 → 2026-11、2027-01 → 2026-12）。
  - `tests/verification/desktop/evidence/od-grow-301/t13/verify_output.txt`（新增）：命令、数字、退出码与开工快照。
- 实际执行：`cd desktop && npm run test` → **tests 132 / pass 132 / fail 0 / cancelled 0 / skipped 0**（改动前同命令基线 129 pass，新增 3，exit 0）；`cd desktop && npx tsc --noEmit` → 无输出，exit 0。证据见 `tests/verification/desktop/evidence/od-grow-301/t13/verify_output.txt`。
- 未验证：无。本轮只涉及前端纯函数与 Node 单测，未新增依赖。可选项「复盘提示词补传今天日期」按任务书默认未做。


## OD-GROW-301 ARCHITECT 验收复核（2026-11-23）

- 复跑命令与结果（全部实际执行）：
  - `cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → 1349 passed / 0 failed / 1 ignored
  - `cd desktop && npm run test` → 132 passed / 0 failed；`npx tsc --noEmit` → 0；`npm run build` → 0
  - `cd engine && python3 -m unittest discover -s omnidesk_engine -p habit_extract_test.py -v` → 4 tests OK
  - `python3 tests/verification/desktop/run_habit_regression.py` → 16 PASS / 0 FAIL
  - `python3 tests/verification/desktop/verify_hardening_2043.py` → 77/77（含 verify 脚本 retired/added 记账修正，见 REVIEW.md 偏差 1）
  - `verify_home_work_0244.py` 31/31；`verify_conversation_prompt.py` 33/33；`verify_harness_220.py` 14/14
- 未实施（按计划留下轮）：G8（需先出 AI 原稿快照方案并经用户批准）、G9（可选，需批准）、T14（拆 lib.rs 余下领域）、T12 余项（npm test 接入 CI test job）。
- 未验证：真实 Tauri 窗口界面、真实模型整理端到端、复盘重试端到端（需实机）。

## OD-GROW-301 G8 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`（main）。开工 `git status` 与 T13 记录一致，既有未提交/未跟踪改动全部保留；未改 `PLANS.md` 既有内容、`STATUS.md`/`REVIEW.md` 既有内容、前端、`engine/`、`runtime/`、`docs/od-*.md` 或既有测试断言；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src-tauri/src/writing_diff.rs`（新增）：纯函数 `section_diffs(text_a, text_b)`。按 `parse_body_sections` 拆章，章内按空行分段，只报被删/新增段落中在另一侧整篇章节里不存在的句子；按 `。！？!?；` 与换行切句，单句超过 100 字符按字符切成 ≤100 字符片段；同章去重，章节按编号升序。解析失败返回空列表。5 项合成数据单测（未变/章删除/章新增/改写句/长片段切分/空与畸形文本）。
  - `desktop/src-tauri/src/writing_section.rs`：`SectionConfirmRequest` 新增 `#[serde(default)] ai_draft_text: Option<String>`（前端不传时行为不变）；`parse_body_sections` 改为 `pub(crate)` 供差异模块复用；`confirm_section` 在用户版快照前，先以 `VersionScope::Section` + `VersionSource::Ai` 保存 label `ai-draft-section-<n>`、note「AI 原稿」的快照（内容为当前已确认章节 + 本章 AI 纯正文）；空白 AI 文本视为未提供，含 Markdown 标题行的文本拒绝（避免破坏正文分节）。1 项单测覆盖带/不带/空白 AI 原稿的版本数与当前正文。
  - `desktop/src-tauri/src/consolidation.rs`：新增 `WritingDiffHint { origin, writing_id, section_no, fragment }`、`collect_writing_diffs(work_root)`：扫描 `work/写作/` 下 ≥2 版的文稿，取 manifest 最近两版做逐章节差异；同一被删片段出现在 ≥2 个不同 `writing_id` 才输出，单份删除不提。`collect_digest` 将其追加为摘要段落「【写作差异】」并受 `max_digest_chars` 预算约束；`DigestStats` 新增 `writing_diff_candidates` 与 `truncated_writing_diffs`。1 项单测覆盖两份同片段命中、单份不命中、单版本不扫描，以及摘要接入。
  - `desktop/src-tauri/src/lib.rs`：只加 `pub mod writing_diff;`；`writing_section_confirm_impl` 对 `ai_draft_text` 与既有章节文本一样过红线与隐私掩码；测试结构体字面量补 `ai_draft_text: None`（编译所需，不改断言）。
  - `desktop/src-tauri/src/writing_review.rs`、`desktop/src-tauri/src/export_engine.rs`：既有测试结构体字面量补 `ai_draft_text: None`（编译所需，不改断言）。
  - `tests/verification/desktop/evidence/od-grow-301/g8/verify_output.txt`（新增）：全量 cargo 测试原始输出。
- AI 快照口径与偏差：`document_store::normalize_label` 只允许 ASCII 字母数字与 `-`/`_`，无法使用中文 label「AI 原稿」，故 label 取 `ai-draft-section-<n>`、中文说明放在 `note`（测试断言 note 为「AI 原稿」）。接入点沿用现有唯一写入口 `save_with_snapshot`：提供 AI 原稿时先写入 AI 版快照（当前正文临时为 AI 内容），紧接着写用户确认版并覆盖当前正文；因此一次确认的版本号递增 2（大纲 v1、AI 原稿 v2、用户版 v3）。AI 原稿文本在本轮按纯章节正文保存，不存原始 JSON 包。
- 实际执行：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1356 passed / 0 failed / 1 ignored**（基线 1349 passed / 1 ignored，新增 7 项：writing_diff 5、writing_section 1、consolidation 1；满足 ≥ 1349 + 7）。证据见 `tests/verification/desktop/evidence/od-grow-301/g8/verify_output.txt`。
- 前端接线缺口（本轮按任务书不改前端）：`desktop/src/types.ts` 的 `WritingSectionConfirmInput` 尚无 `ai_draft_text` 字段；`desktop/src/shell/WritingSectionEditor.tsx` 的 `confirmDraft` 只传 `draft`，且生成阶段拿到模型原文后未保留原稿文本。下一轮界面任务需要：在编辑器保存本次模型返回的纯章节正文，确认时以 `ai_draft_text` 一并提交；现有调用不传该字段时宿主行为与改动前一致。
- 未验证：未在真实应用数据目录上跑一轮 AI 原稿落盘与两版差异；未接 G3 校验与 G5 展示，writing_diff 只作为摘要来源与统计计数，不自动写入生效卡。未用真实模型输出验证原稿清洗口径（本轮要求界面传入纯正文）。


## OD-GROW-301 G9 执行记录（BUILDER，2026-11-23）

- 基线：`5dc5df4b622815f4803ec4cff07d448e9824dbf1`（main）。开工 `git status` 与 G8 记录一致，既有未提交/未跟踪改动全部保留；未改 `PLANS.md` 既有内容、`STATUS.md`/`REVIEW.md` 既有内容、前端、`engine/`、`runtime/`、`docs/od-*.md` 或 `writing_diff.rs`；未提交、未推送、未打标签。
- 改动文件：
  - `desktop/src-tauri/src/knowledge_examples.rs`（新增）：纯函数 `extract_example_paragraphs(body)` 按空行分段，取 50–300 字符、跳过 `#` 标题与列表开头段落、每章最多 3 段；`stage_section_examples(...)` 对候选段先过 `RulePolicy::reject_keywords`（命中跳过该段），再过 `mask_privacy`，然后把脱敏正文写入 `inbox/写作范例/` 源材料（`writing_id + 章节 + 版本` 可追溯），最后调用既有 `knowledge_review::stage` 进入待审核队列；`stage_confirmed_examples(paths, ...)` 解析已配置知识库根与审核目录后再暂存。`knowledge_examples.rs` 内 4 项合成数据单测：49/50/300/301 长度边界、标题与列表过滤、3 段上限、确认后暂存并跳过红线、暂存失败不影响确认。
  - `desktop/src-tauri/src/writing_section.rs`：新增 `confirm_section_with_stage`（先执行既有 `confirm_section`，再执行范例暂存；暂存失败只 `eprintln!` 记录，返回仍为确认成功）。既有章节确认逻辑与测试断言未改。
  - `desktop/src-tauri/src/lib.rs`：新增 `pub mod knowledge_examples;`；`writing_section_confirm` 命令路径改为 `writing_section_confirm_impl_with_stage`，在章节确认后以闭包调用 `knowledge_examples::stage_confirmed_examples`；`writing_review_apply_finding` 命令路径改为 `writing_review_apply_finding_impl_with_stage`，应用审阅修订后调用 `knowledge_examples::stage_text_examples`（覆盖 `confirm_section` 的修订入口）；原 `writing_section_confirm_impl` 与 `writing_review_apply_finding_impl` 保留为无暂存的兼容入口（既有单测不变）。
  - `tests/verification/desktop/evidence/od-grow-301/g9/verify_output.txt`（新增）：全量 cargo 测试原始输出与最终 `test result`。
- stage 接入点：①章节确认 `writing_section_confirm`（`lib.rs`）→ `writing_section_confirm_impl_with_stage` → `writing_section::confirm_section_with_stage` → `knowledge_examples::stage_confirmed_examples`；②审阅修订 `writing_review_apply_finding`（`lib.rs`）→ `writing_review_apply_finding_impl_with_stage` → `knowledge_examples::stage_text_examples`；两条都落到 `knowledge_review::stage(vault_root, review_dir, result_text, [inbox/写作范例/<writing_id>-第N章-vV.md], policy, offset)`。未新增命令、未绕过审核队列；知识卡只有用户在「知识审核」批准后才由既有 `knowledge_review::decide` 落盘。
- 实施偏差（任务书与现有准入的差异，如实记录）：①任务书允许 `lib.rs` 只加模块声明，但 `writing_section::confirm_section` 只拿到工作根，拿不到应用数据根、知识库根与审核目录；因此暂存放在命令包装层（章节确认与审阅修订两处约 45 行改动），否则无法调用现有 `knowledge_review::stage`。②任务书写 `sourcePaths=[writing_id + 章节]`，但现有准入要求 `sources` 指向知识库中真实存在的 `inbox/` 或 `raw/` 文件；本轮把脱敏后的确认正文写入 `inbox/写作范例/` 源材料区（不是知识卡），来源路径包含写作编号、章节与版本，批准后由既有归档流程移到 `知识库/原文/`。③不做新开关：沿用 `knowledge_review` 待审核队列本身作为准入；没有配置知识库根时暂存失败只记日志，章节确认照常完成。
- 实际执行：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1360 passed / 0 failed / 1 ignored**（基线 1356 passed / 1 ignored，新增 4 项；满足 ≥ 1356 + 4）。证据见 `tests/verification/desktop/evidence/od-grow-301/g9/verify_output.txt`。
- 未验证：未在真实应用数据目录与真实知识库上跑一轮“确认章节 → 审核队列 → 批准落卡”的端到端；未接界面展示；未用真实模型输出验证范例质量。测试全部为合成数据，未写入案件、线索或人物画像类内容。未新增依赖，未修改既有测试断言。

## OD-GROW-301 G8/G9 ARCHITECT 验收复核（2026-11-23）

- G8：cargo → 1356 passed / 0 failed / 1 ignored（+7）；AI 原稿快照走 VersionSource::Ai（一次确认版本号 +2）；writing_diff 候选只进摘要来源，不自动生效。前端 ai_draft_text 接线留下一轮界面任务。
- G9：cargo → 1360 passed / 0 failed / 1 ignored（+4）；范例段经红线+脱敏后进 knowledge_review 待审核队列（inbox/写作范例/ 源材料），未绕过准入；lib.rs 包装层约 15 行改动（原因见 REVIEW.md）。
- 最终全量：cargo 1360 / npm 132 / tsc 0 / build 0 / 回归 16 PASS / verify_hardening 77/77。
- 版本：0.2.53（用户批准提交推送）。

## OD-PRJ-604 执行记录（BUILDER，2026-11-23）

- 基线：HEAD `203748eef9`。开工 `git status` 为 `M PLANS.md` 与未跟踪 `docs/od-prj-604-dispatch.md`，两者原样保留；`desktop/src/` 下目标文件开工时无未提交改动。未改 `desktop/src-tauri/`、`engine/`、`runtime/`；未新增依赖；未提交、未推送、未打标签。
- UI 参考：`screenshots-design-inspiration` 检索 gantt progress（AlphaSense）、project timeline（Teamweek、Flow）、review modal（Glassdoor），并查看 Teamweek 甘特截图。Teamweek 的泳道式甘特把左侧名称与右侧密集时间条清楚分开，适合沿用现有分组行；Glassdoor 的宽弹窗承载复核正文，适合把项目复盘移出侧栏。
- 改动文件与行数：
  - `desktop/src/shell/TaskManager.tsx`：+134 / −40（文件 2216 行）。F1 状态前置链、F2 右列 `onChanged` 接线、F3/F5 项目汇总带与任务双层条/名称层级。
  - `desktop/src/shell/ProjectMaintenance.tsx`：+8 / −1（文件 668 行）。新增 `onChanged`，写操作成功后经 `refresh()` 传播。
  - `desktop/src/shell/ProjectReviewCard.tsx`：+144 / −46（文件 529 行）。新增 `onChanged`、复盘居中模态、自动打开、Esc/遮罩关闭与底部动作迁移。
  - `desktop/src/projectGanttProgress.ts`：新增 76 行；`taskProgressRatio`、`projectWindow`、`projectProgressRatio`。
  - `desktop/src/projectGanttProgress.test.mjs`：新增 140 行、18 项表驱动用例。
  - `desktop/src/theme.css`：+31 行；追加 `.od-gantt-plan--*` 与 `.od-gantt-progress--*`，既有 `.od-task-bar--*` 未改。
  - `tests/verification/desktop/evidence/od-prj-604/verify_output.txt`：新增运行证据（Git 忽略的 evidence 目录）。
- 六项修复对照：
  1. 任务完成状态机：`待开始` 与 `暂停` 均先转 `进行中`，`待确认` 先转 `待开始`，随后标记 `已完成`；失败文案沿用现有路径。
  2. 写入后即时刷新：ProjectMaintenance 的新建项目、添加/挂接任务、换绑/摘除、删除任务成功后调用 `onChanged`；ProjectReviewCard 的复盘保存、确认归档、知识卡暂存成功后调用；TaskManager 挂载处 bump `tick`，四象限、甘特、项目列表和统计同轮重载。
  3. 任务甘特双层条：浅色计划条按 created→due 铺满，状态色进度覆盖按 `taskProgressRatio(task, nowMs)` 填充；完成=1，待确认/待开始=0，进行中按创建至截止夹取，取消/暂停保留当前时间进度。
  4. 项目汇总进度：汇总带按 project.start→end，缺结束日补 90 天，缺开始日取组内最早任务，叠加 `projectProgressRatio` 时间填充；区间文字同步使用实际计划区间。
  5. 名称层级：项目名 13px 粗体 ink，任务名 11.5px 常规 ink-2，状态字 10px ink-3。
  6. 项目复盘大弹窗：`min(76vw,1100px)` × 76vh 居中，正文内滚；生成成功自动打开，卡片保留 AI 复盘/重新生成与状态徽标及「查看」；头部项目名+状态+关闭，底部确认归档、提炼知识卡；Esc 与点遮罩关闭。
- 实际执行与结果：`cd desktop && npm run test` → **tests 150 / pass 150 / fail 0 / skipped 0，EXIT=0**（按 STATUS 既有基线 132，新增 18 项）；`cd desktop && npx tsc --noEmit` → 无输出，**EXIT=0**；`cd desktop && npm run build` → tsc + Vite 构建成功，**EXIT=0**（仅既有的 >500 kB chunk 提示）。证据见 `tests/verification/desktop/evidence/od-prj-604/verify_output.txt`。
- 未验证：真实 Tauri 窗口中的六项交互（标记完成点击流、跨卡片即时刷新、甘特视觉、真实项目日期汇总、弹窗打开/Esc/遮罩/底部按钮）由用户实机复测；未在真实项目数据上核对进度填充与计划带重合。静态回归脚本 `verify_project_601.py` 当前 13/15：S2 旧断言在开工前已与本轮计划口径不符，S3 因新增 `onChanged` 后 JSX 精确字符串不再匹配；本轮未修改任何既有测试断言或该脚本。

## OD-PRJ-604 ARCHITECT 验收复核（2026-11-23）

- 复跑：npm test 150/150（+18）；tsc 0；build 0；verify_project_601 15/15；verify_project_602 18/18；verify_hardening_2043 77/77。
- 静态断言适配：verify_project_601.py S2/S3、verify_project_602.py S1 needle 按获批 JSX 更新，语义不变（见 REVIEW.md）。
- 版本：0.2.54（用户批准提交推送）。

## OD-UI-900 全站 UI 收口执行记录（2026-11-25）

- 基线：HEAD `74f9b70321`（0.2.55 + OD-UI-900 阶段1–5）。开工保留既有未提交的 `desktop/src/shell/TopNav.tsx` 三处暖材质输入框改动。
- 项目跟踪页二轮：`TaskQuadrantBoard` 四象限与甘特改为 stretch + 1fr 行高，Q1/Q2 三项目全部可见；任务卡移除关键词图标，改状态圆点 + 时钟日期；甘特条删除状态文字，逾期加斜纹与时钟；补齐 `.pj-proj-track`/`.pj-proj-fill`；页头功能组拆出独立 `fanOpen` 状态，4×34px 子项、Tab 可达、Esc/外部点击收回，管理动作打开项目维护弹窗；底栏收敛为「已完成 + 项目复盘」两卡。
- 跨页收口：工作记录/历史会话/知识库/设置页头统一 22px；内容宽度统一 1280（含取消写作页 `-translate-x-8`）；设置页标题、选中卡与输入框回到暖橙材质；知识库白卡由属性选择器统一到暖材质；首页空态书法标语换为浮起灵感托；工作记录/写作/历史空态改图标。
- 评审：Openference/DeepSeek-V4.1-Flash-1017-Free（high）首轮 7.0，提出四象限裁切、状态文字、进度轨道缺失、扇形碰撞等 P0/P1；修复后复评 8.0。第二次调用该模型返回 402「Free models require credit balance above $1」，复评由 `OpenDesign/glm-5.3-flash`（high）完成，仍为独立只读评审。
- 实际执行：`cd desktop && npm run test` → 150 passed / 0 failed；`npx tsc --noEmit` → 0；`npm run build` → 0；`verify_project_601` 15/15、`verify_project_602` 18/18、`verify_project_ui_716` 7/7、`verify_task_quadrant_ui_402` 10/10。
- 未验证：真实 Tauri 窗口的悬停扇形、外部点击收回、管理弹窗、键盘焦点顺序与 1600/1280 两档真实数据布局；复评截图来自临时 1600×1000 合成演示预览，预览文件已在提交前删除。

## OD-UI-901 项目跟踪验收修正（2026-11-25）

- 基线：0.2.56（`619dc6f6d5`）。按用户验收截图重排项目跟踪页：左栏 288px 纵向四单元，默认展开 Q1/Q2；右栏甘特 + 最近项目卡。
- 移除甘特项目轨进度条；左栏任务行改为状态点 + 重要/紧急图标 + 标题 + 右对齐截止日/逾期天数；甘特任务条功能组内嵌条内右侧，项目卡片功能组在左下，均保留 spring 展开；移除右上 + 功能组与底部已完成/维护模块。
- 左上状态按钮改为居中弹层（已完成/进行中/逾期）；左栏底部新增项目跟进历史会话模块，默认折叠，点击回首页继续。首页恢复书法标语「坚持学习人生的有无限可能」。
- 实际执行：`npm run test` 150 passed；`npx tsc --noEmit` 0；`npm run build` 0；`verify_project_601` 15/15、`verify_project_602` 18/18、`verify_project_ui_716` 7/7、`verify_task_quadrant_ui_402` 10/10。
- 未验证：真实项目跟进历史会话数量与点击回首页链路、任务条/项目卡悬停展开的真实指针动效、窗口较窄时的 3 卡回落与单列布局。

## OD-UI-902 验收细节修正（2026-11-26）

- 项目卡片功能组去掉添加任务，只保留「管理项目与任务」「生成项目复盘」；添加任务从项目管理弹层进入。
- 图标按钮统一改用全局 `data-hover-hint`，约 320ms 出现暖白提示卡；移除任务条/卡片/文件夹/关闭按钮上的原生 `title`，避免约 1 秒后才出现的系统气泡。
- 复盘弹层增加可见标题「项目复盘」，项目选择换成自定义暖白下拉，生成按钮加 spark 图标；复盘正文弹层补充「项目复盘」标识。
- 项目与任务弹层增加可见标题「项目与任务」，主项目选择换成同一套自定义下拉，图标按钮补齐悬停说明。
- 实际执行：`npx tsc --noEmit` 0；`npm run test` 150 passed；`npm run build` 0；`verify_project_601` 15/15、`verify_project_602` 18/18、`verify_project_ui_716` 9/9、`verify_task_quadrant_ui_402` 10/10、`verify_project_ui_512` 26/26。
- 未验证：用户本轮自行验收；未在真实 Tauri 窗口逐项检查原生下拉替换后的键盘操作与 320ms 提示的实际手感。

## OD-UI-903 项目摘要换行修正（2026-11-26）

- 项目与任务弹层的项目摘要原为「状态一行 + 日期一行」，在窄宽度下日期区间会折行并撑破圆角背景。现改为单行：状态胶囊、`YYYY-MM-DD → YYYY-MM-DD`、添加任务按钮同排；日期 `truncate`，完整区间放 `title`。
- 实际执行：`npx tsc --noEmit` 0；`npm run test` 150 passed；`npm run build` 0。
- 未验证：用户实机窗口宽度下的最终视觉。
