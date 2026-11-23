# OD-PRJ-604 派工：项目跟踪六项实测修复

## 任务编号与名称
OD-PRJ-604：任务完成状态机、甘特计划/进度双条、刷新即时性、项目汇总进度与层级、复盘大弹窗

## 获批依据
- 计划位置：PLANS.md 顶部 OD-PRJ-604 小节（ARCHITECT 补记，用户 2026-11-23 实测反馈六项）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：desktop/src/shell/TaskManager.tsx、desktop/src/shell/ProjectMaintenance.tsx、desktop/src/shell/ProjectReviewCard.tsx、desktop/src/projectGanttGroups.ts、desktop/src/projectGanttGroups.test.mjs、desktop/src/theme.css（追加样式段）、desktop/src/shell/TaskManager 相关新测试文件（可新增 projectGanttProgress.ts/.test.mjs）；证据 tests/verification/desktop/evidence/od-prj-604/
- 明确不做的事：不改 desktop/src-tauri/（纯前端修复）；不改 SettingsPanel/HomeChat；不新增依赖

## 六项修复

### F1 任务无法标记完成（状态机前置链）
TaskManager.tsx markComplete：preState 处理已覆盖 待确认→待开始、暂停→进行中，但漏了 待开始→进行中。补上：task.state === "待开始" 时先转 进行中（若失败报错文案同现有），再转 已完成。不改宿主状态机白名单。

### F2 新建/追加任务后四象限与甘特不显示（刷新传播）
根因：ProjectMaintenance 内的 refresh() 只 bump 自己的 tick，TaskManager 的 useEffect 依赖 [tick] 不知道。给 ProjectMaintenance 与 ProjectReviewCard 增加 onChanged?: () => void prop，在各写操作成功后（createProject/addTask/挂接/删除/taskUpdate/confirm/stage 等）调用；TaskManager 挂载处传 onChanged={() => setTick(n => n+1)}。这样项目/任务变化后四象限、甘特、统计立即重载。铃铛 60 秒自刷新不受影响。

### F3+F5 甘特：计划条+进度条、项目汇总进度、名称层级
- 任务行：把现在单一状态色条改为「计划条 + 进度覆盖」双层：底=计划条（按任务状态色的浅色/低饱和变体），上层覆盖=进度填充（深色状态色）。进度值由新纯函数 taskProgress(task, nowMs) 计算：已完成=1；已取消/暂停=保持当前时间进度；其余=clamp((now - created)/(due - created), 0..1)（无 due 或 due<=created 时为 0）。now 由组件传入（不读时钟，纯函数可测）。
- 项目汇总行（组头）：汇总带同样双层——底=项目计划区间浅色带（project.start→project.end；无 end 时用 start→start+3 个月，无 start 时用组内最早任务 created→+3 个月），上层=同款时间进度填充；区间文字旁保持现状。project.start/end 都缺且组内无任务条时保持纯浅带。
- 名称层级：组头项目名 text-[13px] font-bold（比任务名高一级），任务名 text-[11.5px] font-normal text-[color:var(--od-neu-ink-2)]，状态字 text-[10px] text-[color:var(--od-neu-ink-3)]。项目行与任务行颜色差异：项目名 ink，任务名 ink-2。
- 进度几何放进新纯函数文件 projectGanttProgress.ts：export function taskProgressRatio(task, nowMs): number；export function projectWindow(project, fallbackStartMs): {startMs, endMs}（默认 3 个月 = 90 天）；export function projectProgressRatio(project, rows, nowMs)：有起止用起止，否则用汇总带区间。测试文件 projectGanttProgress.test.mjs 表驱动（已完成=1、进行中 50%、未开始 0、无 due=0、越界 clamp、项目默认 90 天、进度填充区间计算）。
- 样式：theme.css 追加 .od-gantt-plan-* / .od-gantt-progress-* 语义类（浅底用现有 --od-neu-*-soft 令牌，进度填充用状态色），不改既有 .od-task-bar--*（保留给进度填充）。

### F6 项目复盘大弹窗
ProjectReviewCard：AI 复盘生成完成与查看复盘时，正文不再挤在侧栏 max-h-[300px] 内；新增居中模态（覆盖层 fixed inset-0 z-50 bg-black/30，内容 wr-dialog 风格、宽 min(76vw, 1100px)、高 76vh 居中，内滚）。入口：①生成成功后自动打开；②卡片上复盘状态行加「查看」按钮（复盘存在即可打开）。弹窗头部=项目名+状态徽标+关闭；底部=确认归档、提炼知识卡（把现有按钮移入弹窗底部，侧栏保留「AI 复盘/重新生成」入口与状态徽标）。Esc 与点遮罩关闭。侧栏卡片保留：选择项目、生成入口、阶段提示、通知；正文区改为「查看复盘」按钮 + 状态徽标（不再渲染 300px 内滚正文）。

## 测试
- projectGanttProgress.test.mjs（新增，≥10 项）
- projectGanttGroups.test.mjs 既有 9 项保持通过（不改其断言；若组头新增字段需补构造，补字段不算改断言）
- npm run test 全绿且 passed ≥ 132 + 新增；npx tsc --noEmit 0；npm run build 0

## 验证
- cd desktop && npm run test；npx tsc --noEmit；npm run build
- 证据落 tests/verification/desktop/evidence/od-prj-604/verify_output.txt
- 真实窗口交互（弹窗、点击流）标「未验证」，由用户实机复测六项

## UI 参考（必做）
- 动工前用 screenshots-design-inspiration MCP 检索 2-3 个同类（gantt progress、project timeline、review modal），借鉴要点 ≤2 句写进执行记录；不可用如实说明。

## 硬约束
- 不修改 desktop/src-tauri/、engine/、runtime/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；npm passed 数不得减少；同一失败测试修两次不过即停
- 单个既有文件改动不加多余标签与说明文字；沿用 od-neu-* 令牌

## 交付
- 改动文件清单（含行数）+ 测试数字 + UI 参考要点 + 六项修复对照
