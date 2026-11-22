# OD-PRJ-602 BUILDER 任务指令：项目甘特分组、界面规范化与数据一致性

> 依 `docs/BUILDER-Task-Instruction-Template.md` 撰写。计划正文以 `PLANS.md` 的 OD-PRJ-602 小节为唯一实施口径；本文只负责把范围、顺序、证据与禁止项说清楚，不放宽任何一条。

## 任务编号与名称

OD-PRJ-602：项目跟踪页项目甘特、界面规范化与数据一致性验证

## 获批依据

- 计划位置：`PLANS.md` → 「OD-PRJ-602：项目跟踪页项目甘特、界面规范化与数据一致性验证」。
- 批准时间与批准人：2026-11-23，用户当轮明确授权执行（含子代理派工：模型 `OpenDesign/deepseek-v4.1-flash-eco`，思考等级 `high`；ARCHITECT 只规划与复核）。
- 开工基线（ARCHITECT 实测 2026-11-23）：HEAD `e3b5c68a19`，工作区干净（0.2.51 已提交推送）。

允许修改的文件或目录：

| 范围 | 具体路径 | 说明 |
| --- | --- | --- |
| 前端 | `desktop/src/shell/TaskManager.tsx` | 甘特项目分组、令牌化色板、弹窗材质 |
| 前端 | `desktop/src/theme.css` | 新增 .od-task-bar--* 状态条令牌段（只加不改既有段） |
| 前端 | `desktop/src/shell/ProjectReviewCard.tsx`、`ProjectMaintenance.tsx` | 材质/字号/圆角对齐令牌；不改行为逻辑 |
| 前端 | `desktop/src/shell/ScenePage.tsx` | 仅清 projectItems 死代码（157-164、250-290、838 行附近），其余不动 |
| 纯函数 | `desktop/src/projectGantt.ts` 或新增 `desktop/src/projectGanttGroups.ts` | 项目分组行布局纯函数 + 单测（npm run test 可跑） |
| 验证 | `tests/verification/desktop/verify_project_602.py`（新增）、`tests/verification/desktop/evidence/od-prj-602/`（新增） | 静态检查 + 证据 |
| 状态 | `STATUS.md` 顶部追加执行记录 | 只追加 |

明确不做：

- 不修改 `runtime/`；不动模型路由、凭证、Profile。
- 不改右侧四张统计卡（今日新增/进行中/已完成/逾期）数字口径。
- 不删 `desktop/src/shell/ProjectBoard.tsx`；不删 `project-items.rs` 宿主模块与 `project-items.v1.json` 命令（只清 ScenePage 前端死代码）。
- 不做甘特拖拽调整日期；不做四象限分类算法改动；不做项目实体存储格式改动。
- 不做版本号变更、CHANGELOG、git commit / push / tag（ARCHITECT 复核后统一处理）。
- 测试只用合成数据，不写真实数据根 ~/OmniDesk/。

## 开工前检查（只读）

1. `git status --porcelain`（预期干净）与 `git rev-parse --short HEAD`（预期 e3b5c68a19），记录快照。
2. 精读现状（ARCHITECT 已核实，不要重复考古）：
   - TaskManager.tsx：`ganttRows`（约 1019 行）任务条逻辑、`timeline`/`ticks`/`todayPercent`、`TASK_STATE_BAR`（834 行）六色写死；甘特 JSX（约 1282-1365 行）；编辑弹窗头部渐变（1568、1706 行）与绿色按钮（1425、1435、1686 行）写死色值；四象限 compact 变体（weekly 页共用）不得回归。
   - theme.css 125-160 行：--od-neu-* 全套令牌（bg/surface/surface-high/sunken/sunken-deep/ink 1-3/accent/accent-soft/accent-ink/focus/hi/lo/line 三档/raise 1-3/inset 1-2）。
   - ReviewArchive.tsx 与 WorkRecordLibrary.tsx 是已令牌化的参照（无写死靛蓝渐变）。
   - ScenePage.tsx：157-164 行 projectItems 状态、250-290 行 projectItemsRead effect 与 saveProjectItems、838 行 void 占位——本轮删掉这些，不动其他 project 逻辑（TaskQuadrantBoard 挂载不变）。
   - projectGantt.ts：现有纯函数（toDayNumber/ganttWindow/ganttTicks/layoutItems/todayPct）。新分组函数放这里或新文件，配 .test.mjs 单测（npm run test 会自动发现）。
   - ProjectReviewCard / ProjectMaintenance：材质常量已是 NEU_CARD/NEU_TILE/NEU_INSET，但内部仍有 #eaf8ef/#2f7a51/#8a877f/#34343b 等写死色值，统一替换为 ink 令牌与语义色。
3. 运行基线：`python3 tests/verification/desktop/verify_project_601.py`（15/15）、`verify_project_ui_716.py`（7/7）、`verify_task_quadrant_ui_402.py`（10/10）应全过——这三份脚本本轮结束后必须仍然全过（402/716 若因合理重构需微调断言，改动必须逐条列出并说明，不得删除断言遮问题）。

## UI 参考（涉及界面时必填）

动工前调用 screenshots-design-inspiration MCP（search_inspiration，关键词如 gantt project grouping、timeline grouped rows），看 2-3 个同类产品，STATUS.md 写 2 句以内借鉴要点。不可用则如实记录。参考对象：TeamGantt/Asana timeline 的「项目分组 + 组内任务条」信息结构；借鉴的是分组行视觉层级与折叠交互，配色与材质全部用本应用令牌。

## 实施要求（按顺序做，每步可独立验收）

### S1 项目甘特分组（核心功能）

1. **数据层（纯函数）**：新增 `buildProjectGanttGroups(tasks, projects, timeline)`——输入任务列表与项目列表，输出有序分组：`{ projectId, projectName, collapsed 默认 false, summary: { start, end, leftPct, widthPct }, rows: [{ task, left, width, clamped }] }`。「未挂接项目」与「无项目任务」归入特殊组 projectId=""，组名「未挂接项目」。无任务的项目不出现在甘特。条形坐标逻辑与现有 ganttRows 一致（created_at→due_date、clamp 到窗口、宽度下限），复用/抽取现有实现，不写第二套坐标换算。
2. **项目汇总带**：组头行画一条浅色汇总带（--od-neu-accent-soft 或 sunken，无状态含义），从组内最早条左端到最晚条右端，表达「这个项目整体跨了这段时间」；组内任务条保持状态色。
3. **折叠联动**：组头点击切换折叠（仅组内行收起，汇总带保留）；折叠状态存组件 state，不持久化。组头显示项目名 + 任务数 + 汇总区间文字（如 11.01 → 11.28）。
4. **行交互**：任务行保留 data-task-gantt-row，点击打开既有编辑弹窗；行 hover 时同组行底色微亮（联动反馈，简单 CSS 即可）。
5. **排序与上限**：组按汇总带左端排序；组内按任务条左端排序。总任务行 > 30 时自动折叠「未挂接项目」组与最早结束的组，并在甘特底部提示「已折叠 N 组，展开查看」。
6. **空态**：没有任何带截止日期任务时保留现有空态文案（改成「当前范围内没有带截止日期的任务」即可，不新增功能）。
7. 甘特卡头部「时间范围」切换、today 竖线、tick 行全部保留；today 竖线要贯穿组头汇总带与任务行。

### S2 界面规范化（令牌化重构）

1. `TASK_STATE_BAR` 六个写死色值删除，改 `className` 走 theme.css 新增段：

```css
/* OD-PRJ-602 任务甘特状态条（语义与 OD-UI-717 前一致，色值全部入令牌） */
.od-task-bar--pending-confirm { background: #c0855f; }  /* 待确认 */
.od-task-bar--not-started { background: #8a7f6e; }      /* 待开始 */
.od-task-bar--in-progress { background: #a24e2c; }      /* 进行中 */
.od-task-bar--done { background: var(--od-neu-accent); } /* 已完成 */
.od-task-bar--cancelled { background: #b5a896; }        /* 已取消 */
.od-task-bar--paused { background: #8a5a12; }           /* 暂停 */
```

（色值本身保留现有视觉，只是集中进 theme.css 语义类；「已完成」若用 --od-neu-accent 视觉突兀可保持 #3f7d5a 并注释来源。六个类一一对应，不要发明新状态。）
2. 弹窗头部与「保存修改」「标记完成」按钮的 `#9481ff/#5747f8` 渐变、`#eaf8ef/#2f7a51` 绿色、`#5747f8/80` 搜索图标色：改为既有令牌（`--od-neu-accent` 系 / 已有语义色类）。渐变按钮统一改成与 ReviewArchive「重新生成复盘」一致的实色 accent 风格（去掉多色渐变与彩色投影）。
3. ProjectReviewCard / ProjectMaintenance 内部写死色值（#eaf8ef、#2f7a51、#8a877f、#34343b、#85858c、#5f5f67 等）全部替换为 `text-[color:var(--od-neu-ink-3)]` 一类令牌；圆角/间距保持现有值不改版。
4. 响应式检查：右列 xl:grid-cols-[minmax(0,1fr)_300px] 在 1024-1280（lg）时允许单列堆叠（补 lg 断点或验证现有行为不破）；甘特卡在窄宽度标签列不挤压（保持 minmax(150px,168px)）。
5. 视觉验收基准：改完后 verify_project_ui_716.py 的「无冷蓝色板」「neu 令牌」「wr-dialog」断言仍过；整页除语义状态色外无新的写死 hex。

### S3 数据一致性验证与清理

1. ScenePage 死代码清除（见允许修改清单）后 `npx tsc --noEmit` 必须过；project-items 宿主命令不动。
2. 一致性静态断言（写进 verify_project_602.py）：
   - 甘特分组数据源 = task.project_id + projectList，与 ProjectMaintenance 分页列表同源（都来自 taskList/projectList）；
   - 复盘卡片 knowledgeReviewStage 的 sourcePaths 是 review.path（复盘 md 实路径）；
   - projectOverview 的 knowledge 聚合来源是 task/record 的 related_knowledge（引用 knowledge_link 既有字段名）；
   - ProjectReviewCard 与 ProjectMaintenance 的项目列表都来自 shell.projectList，无第二套项目数据源。
3. 纯函数单测：分组构建覆盖——重叠（两任务区间交叠各占一行）、接续（前条结束日=后条开始日）、间隔（留白不画假条）、未挂接组、空项目不出现、clamp 边界。放 projectGantt（或新文件）.test.mjs。

## 验证（每项标注证据等级：文件事实 / 静态检查 / 脚本运行 / 未验证）

1. `cd desktop/src-tauri && cargo test --lib --offline -- --test-threads=1` 全绿。
2. `cd desktop && npx tsc --noEmit` → EXIT=0。
3. `cd desktop && npm run test` → 全过（含新增分组单测）。
4. `cd desktop && npm run build` → EXIT=0。
5. `python3 tests/verification/desktop/verify_project_602.py` → 全过；`verify_project_601.py` / `verify_project_ui_716.py` / `verify_task_quadrant_ui_402.py` 保持全过（如断言需调整，逐条列出）。
6. 输出与说明落 `tests/verification/desktop/evidence/od-prj-602/verify_output.txt`。

## 硬约束

- 不修改 runtime/；不动凭证与模型路由；不提交、不推送、不打标签。
- 工作区保持干净起步；若开发中产生临时文件，交付前清理。
- 复用既有组件与令牌；不压缩行、不 golf；新逻辑有单测。
- STATUS.md 只追加执行记录（含 UI 借鉴 2 句、命令与结果、证据路径、未验证项、计划偏差）。

## 交付

最终回复给出：改动文件清单（含新增）、每项验证实际命令与结果、证据路径、未验证项、计划偏差、`git status --porcelain` 尾部快照。写完 STATUS.md 后停下，等待 ARCHITECT 复核。

