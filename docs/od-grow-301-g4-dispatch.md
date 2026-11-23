# OD-GROW-301 G4 派工：无人值守整理运行器

## 任务编号与名称
OD-GROW-301 / G4：每日整理无人值守运行器（前端不渲染组件 + 纯函数状态机 + api 接入）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G4 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：新增 desktop/src/shell/ConsolidationRunner.tsx、desktop/src/consolidationFlow.ts、desktop/src/consolidationFlow.test.mjs；修改 desktop/src/api.ts（只加 consolidation 命令封装与类型）、desktop/src/shell/ScenePage.tsx（在挂载 AnnouncementBar 的位置同级加一行挂载运行器）；证据 tests/verification/desktop/evidence/od-grow-301/g4/
- 明确不做的事：不改 SettingsPanel、HomeChat 主体；不改宿主命令（G2 已提供 consolidation_tick/preview/collect）；不改 engine/；不做 HabitDigestPanel（G5）

## 背景事实（已核实）
- G2 已提供宿主命令：consolidation_tick（返回 due）、consolidation_preview（只读统计）、consolidation_collect（收集+落盘摘要文件+待审定+水位，不调模型）、consolidation_config_get/set。摘要落 habits/state/digest-<date>.json。
- T1 已交付：engine_submit_task 缺省 workspace_dir 由宿主决定，无需前端 prepareWorkspaceDir。
- scene_for_intent 只拦 weekly_report / material_writing / project_management；整理 intent=habit_consolidation 不受 scene_gate 影响。
- engine_submit_task 参数：prompt_text、intent、user_input_text、reasoning_effort、provider、model；提交前宿主做红线与脱敏；回执 omni_task_id；taskStatus/taskResult 按任务号查询；无引擎侧时限，调用方自设轮询上限。
- G3 已提供 habit_extract.rs（宿主校验）；但宿主校验需要通过一个命令暴露给前端调用——检查 G2/G3 是否已加 habit_output_validate 命令；若没有，本任务在 api.ts 用 consolidation_collect 之后的输出交给宿主：**宿主校验入口**：若 G3 的 parse_habit_output 未暴露命令，则改为在 G4 里把模型输出存到摘要文件旁（digest 任务的 raw_output 由宿主 consolidation_collect 之后的 apply 阶段处理）——**执行前先核实现状，如实记录选择**。
  - 可行方案 A（推荐，若宿主缺命令）：G4 前端只做「提交→等终态→取结果」，把 raw output 写进 state（内存），再调用一个新增的宿主命令完成校验与暂存——但本任务书不允许改宿主。因此**方案 B**：前端把 raw output 作为 user_input_text 之外的部分，直接调 consolidation_apply_output——同样需要宿主命令。
  - 结论：若宿主没有输出校验命令，G4 只做「提交→等终态→读结果→记录到本地状态并提示用户打开整理卡确认」，把「校验+暂存」留给 G5 配合宿主补命令（在执行记录里写明该缺口，不阻塞 G4 的状态机与提交流程）。G2 的 consolidation_collect 已经把摘要落盘，G4 提交用的 prompt 从摘要文件读取（通过 consolidation_collect 的返回值）。
- 前端轮询先例：WorkRecordLibrary parseSelected（1 秒间隔、900 秒上限）。整理运行器轮询上限用 1800 秒。

## 功能要求
1. consolidationFlow.ts：纯函数 nextStep(state) -> Step；Step = "idle"|"prepare"|"submit"|"await"|"ingest"|"done"|"failed"。状态含 {phase, tickDue, digestEmpty, submitRejected, taskDone, taskSucceeded, timedOut, attemptDate, runningChat}。
   转移规则（代码常量，不写进提示词）：
   - idle + tick.due && !runningChat && 今天未跑 → prepare；
   - idle + runningChat → idle（让位）；
   - prepare + digestEmpty → done（记完成，不调模型）；
   - prepare + 非空 → submit；
   - submit + 提交被拒 → failed（记录错误；不重试）；
   - submit + 成功 → await；
   - await + 任务终态成功 → ingest；
   - await + 终态失败/超时（1800 秒）→ failed；
   - ingest + 校验/暂存完成（或按上面结论只记录 raw）→ done；
   - 任意状态 + 今天已跑 → idle（每天最多一次）；
   - 应用重启（state 无 attemptDate）→ 重新按 tick 判定（补跑一次）。
   - 表驱动测试覆盖全部转移，含重复触发、重启补跑、空摘要、提交被拒、超时。
2. ConsolidationRunner.tsx：不渲染任何界面（return null）；每 30 秒调 consolidation_tick；due 且无进行中对话（读 api 现有状态——若无全局对话状态可读，则用 window.localStorage 或由 HomeChat 写入的 inflight 标记；先核实 sceneThreads 的 inflightTaskId 与全局会话状态，选择最小实现并记录）时执行 nextStep 流程；每步调 api 封装；失败写 console.warn 并通过 store 事件通知（G5 接铃铛）；记录 lastRun 日期到 localStorage（omnidesk.consolidation.lastRun）。
3. api.ts：加 consolidationTick/consolidationPreview/consolidationCollect 封装与类型（不改既有函数）。
4. ScenePage.tsx：在 <AnnouncementBar .../> 同级加 <ConsolidationRunner /> 一行。
5. 提交参数：intent="habit_consolidation"、provider/model 取 consolidation_config_get、reasoning_effort 取配置（若配置无该字段则用 "low"；模型目录 VERIFIED_EFFORTS 中 deepseek-v4.1-flash 支持 low；-eco 只验证 off——若配置的模型是 -eco，effort 用 off）。prompt = consolidation_collect 返回的 digest_text（含截断报告）。
6. 终态后：读取 taskResult，把 raw content 记录到 localStorage（omnidesk.consolidation.lastOutput，≤20000 字符）并 console.info 摘要条数；G5 将接宿主校验与暂存。此步在执行记录里写明「宿主校验+暂存缺口」。

## 测试
- consolidationFlow.test.mjs：表驱动全转移（≥20 行）。
- npm run test 全绿且 passed ≥ 101 + 新增；npx tsc --noEmit 0；npm run build 0。

## 验证
- cd desktop && npm run test；npx tsc --noEmit；npm run build
- 证据落 tests/verification/desktop/evidence/od-grow-301/g4/verify_output.txt
- 真实模型端到端试跑标「未验证」——由用户实机执行一次并回报。

## 硬约束
- 不修改 runtime/、engine/、desktop/src-tauri/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；npm passed 数不得减少；同一失败测试修两次不过即停
- 不弹系统错误弹窗；失败只 console + 状态记录（铃铛接入留给 G5）
- ConsolidationRunner 不渲染任何可见界面

## 交付
- 改动文件清单 + 测试数字 + 状态机转移表 + 宿主校验缺口说明
