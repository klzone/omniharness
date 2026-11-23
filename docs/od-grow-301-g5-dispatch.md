# OD-GROW-301 G5 派工：界面（整理卡、铃铛、首页提醒、习惯页）

## 任务编号与名称
OD-GROW-301 / G5：整理卡面板、铃铛通知类型、首页提醒条、习惯页

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G5 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：desktop/src/shell/NotificationCenter.tsx（只加条目与类型，不改既有逻辑）、desktop/src/types.ts（加类型）、desktop/src/api.ts（加封装）、desktop/src/shell/HomeCanvas.tsx（加首页提醒条挂载，≤40 行改动）、desktop/src/shell/TopNav.tsx 或 Dock.tsx（习惯页入口，最小改动，≤20 行）；新增 desktop/src/shell/HabitDigestPanel.tsx、desktop/src/shell/HabitCardsPage.tsx、desktop/src/shell/HabitHomeNotice.tsx、desktop/src/habitNotice.ts + habitNotice.test.mjs、desktop/src/habitCardsView.ts + habitCardsView.test.mjs；证据 tests/verification/desktop/evidence/od-grow-301/g5/
- 明确不做的事：不动 SettingsPanel.tsx 与 HomeChat.tsx 主体；不改 desktop/src-tauri/；不改路由结构（习惯页入口用 Dock 或 TopNav 最小一行）

## UI 参考流程（必做）
- 动工前用 screenshots-design-inspiration MCP（search_inspiration / get_product_screenshots）检索 2–3 个同类产品的通知/审批流/习惯清单界面；借鉴要点 ≤2 句写进执行记录。不可用则如实写明。
- 界面原则：不加多余标签和说明文字；让布局传达信息；沿用 od-neu-* 令牌与 wr-* 组件风格。

## 功能要求
1. habitNotice.ts（纯函数 + 测试）：由 G1 候选/未决问题列表算出铃铛条目 {count_items: "N 条习惯候选，M 个未决问题", redBadge: N+M>0, overdueQuestions} 与首页提醒 {show: 当天有新候选且今天未关闭}；关闭标记存 localStorage omnidesk.habits.homeNoticeDismissed（日期键）。
2. NotificationCenter.tsx：新增 kind="habits" 条目（每天一条），计入红标数字；有候选/未决才出现，处理完消失；不改既有 confirm/task/review 逻辑（只加一个数据源与一种 case）。
3. HabitHomeNotice.tsx：首页顶部条件渲染提醒条（新组件，当天只弹一次、可关闭、关闭记 localStorage）；HomeCanvas 在 upper 槽（conversationStarted 条件之外）挂载。
4. HabitDigestPanel.tsx：候选列表（文字/范围切换「仅本场景|全局」/来源（某日、某对话）/接受/驳回按钮/「全部接受」）；驳回自动写驳回记录（可选 reason_code「太绝对」/「不是我说的」，不选也行）；未决问题单独一节（搁置天数、已定/放弃、7 天未动排前）；待审定区（脱敏预览、批准/排除）。面板放在报告场景（ReviewArchive 旁）或首页入口——用最小改动，从 Dock 报告场景进入。
5. HabitCardsPage.tsx：生效卡列表（停用/归档、命中与违反次数——次数从 habits-stats 读取命令？G1 未提供统计读取命令，本任务用 habit_active_list 的 last_seen/统计近似展示或如实标注「统计读取待 G11」，不伪造数字）；整理开关/时间/模型选择（读 consolidation_config_get/set），打开开关时明确写出读取范围（如「将读取未分类对话与项目任务」），默认关闭。
6. 类型与 api：types.ts 加 HabitCard/HabitCandidate/OpenQuestion/PendingReviewItem 等类型；api.ts 加 habits 命令封装（G1 已有 8 个命令）。
7. 界面进入点：Dock 报告场景内新增入口（ScenePage 分支里加一行），或 TopNav；选最小方案并记录。

## 测试
- habitNotice.test.mjs：无候选不出现、数字计数、已处理消失、7 天未动排前、当天关闭后不显示、新的一天重置。
- habitCardsView.test.mjs：范围切换状态函数、批量接受、驳回 reason_code。
- npm run test 全绿且 passed ≥ 105 + 新增；npx tsc --noEmit 0；npm run build 0。
- 界面实机效果标「未验证」，由用户试用。

## 验证
- cd desktop && npm run test；npx tsc --noEmit；npm run build
- 证据落 tests/verification/desktop/evidence/od-grow-301/g5/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/、desktop/src-tauri/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；npm passed 数不得减少；同一失败测试修两次不过即停
- 单个既有文件改动 ≤40 行（HomeCanvas、NotificationCenter、types.ts、api.ts、Dock/TopNav、ScenePage 分支各自计）
- 不加多余标签与说明文字；空态如实显示

## 交付
- 改动文件清单（含每个既有文件改动行数）+ 测试数字 + UI 参考要点
