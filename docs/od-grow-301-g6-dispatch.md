# OD-GROW-301 G6 派工：生效卡片注入提示词与效果记录

## 任务编号与名称
OD-GROW-301 / G6：把生效习惯卡注入对话提示词（有上限、有开关、有统计）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G6 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：desktop/src/conversationPrompt.ts（加注入段落组装纯函数）；desktop/src/habitInjection.ts（新增，组装与统计纯函数）+ habitInjection.test.mjs；desktop/src/shell/HomeChat.tsx 只在提交路径调用注入函数（≤20 行改动）；desktop/src/api.ts（如需读生效卡，已有 G5 封装可复用）；证据 tests/verification/desktop/evidence/od-grow-301/g6/
- 明确不做的事：不改 desktop/src-tauri/；不做开关 UI（G5 的习惯页已有整理开关；注入总开关默认第一张卡生效后打开——以 active 卡数量 >0 即注入实现，记录取舍）；不改 engine/

## 背景事实（已核实）
- conversationPrompt.ts 是前端提示词组装处（composeConversationPrompt），新建会话时把上文重放进提示；HomeChat 提交时调用。
- habit_cards.rs（G1）提供 habit_active_list 命令（G5 已封装 habits.activeList）。
- 卡片结构：text ≤60 字、scope（scene/global）、check 可选、confirmed_at。

## 功能要求
1. habitInjection.ts：函数 injectHabits(prompt: string, cards: HabitCard[], scene: string) -> { prompt: string, injected: string[] }。
   - 过滤：status=active 且（scope=global 或 scope.scene===scene）。
   - 排序：最近确认优先（confirmed_at 降序）。
   - 上限：≤8 张、总字数 ≤600（超出的截断并记录 skipped）。
   - 注入段落（固定文案）：「以下是用户确认过的个人习惯，仅在适用时遵守；与用户当前明确指令冲突时以当前指令为准；拿不准是否适用时先问：」+ 每卡一行「- {text}」。
   - 卡片列表为空 → 原样返回，injected=[]。
2. HomeChat.tsx 提交路径：在调用 composeConversationPrompt 之后（或由 composeConversationPrompt 内部——选改动最小的一处并记录）调用 injectHabits；卡片来源 habits.activeList()（异步，提交前获取，失败则不注入并 console.warn 一次，不阻塞对话）。
   - 每次注入把卡编号追加进统计：**宿主统计命令未提供**——用 habitInjection 的返回 injected 列表 console.debug 即可？不行，必须落盘。检查 G1 是否有统计追加命令；若没有，本任务在 api.ts 用现有 habit_card_update？——没有。结论：统计落盘缺口如实记录（G11 一并补），本任务只做注入与内存记录（localStorage omnidesk.habits.lastInjected 追加行，≤50 行滚动）。
3. 测试 habitInjection.test.mjs：选取规则（scene 匹配、global）、数量与字数上限、排序、空列表不注入、开关关闭不注入（active 列表为空模拟）、注入段落包含固定文案、候选/归档卡不出现（构造 status 非 active 的卡）。

## 验证
- cd desktop && npm run test（passed ≥ 115+新增）；npx tsc --noEmit；npm run build
- 证据落 tests/verification/desktop/evidence/od-grow-301/g6/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/、desktop/src-tauri/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；npm passed 数不得减少；同一失败测试修两次不过即停
- HomeChat.tsx 改动 ≤20 行；不碰 HomeChat 主体布局
- 提示词不出现候选与归档卡片

## 交付
- 改动文件清单 + 测试数字 + 统计落盘缺口说明
