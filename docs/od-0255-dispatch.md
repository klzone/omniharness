# OD-0255 任务书：0.2.55 计划收尾（OD-GROW-301 接线补齐）

> 执行 Agent：OpenDesign/deepseek-v4.1-flash-eco，思考等级 high。主 Agent 负责验收、版本与提交。
> 基线：main 最新提交（0.2.54）。开工时保留工作区既有未提交/未跟踪改动，不 reset/clean/stash。
> 硬约束：不改 runtime/、engine/、模型路由与凭证；不提交、不推送、不打标签。

## 背景

OD-GROW-301 主体（T1/G0/G1-G6/G10/G8/G9/H3/H5/H6+G11/H7+G12/T13）已在 0.2.53 落地，OD-PRJ-604 在 0.2.54 落地。本轮补齐 STATUS.md 与 REVIEW.md 记录的全部接线缺口，让习惯整理链路形成完整闭环。

## T1：G8 前端接线（AI 原稿快照可触发）

宿主已支持 SectionConfirmRequest.ai_draft_text（writing_section.rs:106），前端未传。

1. desktop/src/types.ts：WritingSectionConfirmInput 增加 ai_draft_text?: string | null。
2. desktop/src/shell/WritingSectionEditor.tsx：
   - generateSection 与 reviseSection 拿到模型原文 text 后，先 parseSectionDraft(text, …) 得到纯正文 content，保存为「本次 AI 原稿」状态（修订稿同样保留，修订时覆盖上一份未确认原稿；确认成功、丢弃按钮、切换章节 selectedNo effect 时清空）。
   - confirmDraft 提交时带 ai_draft_text；原稿为空时传 undefined（行为与现状一致）。
3. 不传该字段的既有调用方行为不变（serde default）。

## T2：G13 整理输出宿主校验与暂存命令

新增 commands/consolidation.rs 的 consolidation_apply_output 命令（lib.rs 注册 + commands/mod.rs 导出）：

1. 输入：date（YYYY-MM-DD，对应 habits/state/digest-<date>.json）与 raw（模型原始输出字符串）。raw 原文不落盘，只参与校验。
2. 流程：读取当日 digest-<date>.json（DigestRecord，含 threads 与 digest_text）→ 组装 habit_extract::DigestPlanRef（DigestPlanRef::from_plan 需要的 threads 结构；消息 role 固定 user，与 collect 口径一致）→ ExistingCardsRef::from_cards(候选+生效卡合并) + store.rejection_memory() → parse_habit_output(raw, plan, policy, existing, rejection, now) → 逐条落盘：candidates 走 store.create_candidate（含红线检查），open_questions 走 store.create_open_question。
3. 返回 serde_json::Value：{ date, candidates_created, open_questions_created, rejected: [reason_code…] }。部分失败时如实返回错误（含已创建数量），不伪造成功；已存在的 id 视为跳过不报错。
4. 无当日摘要文件、raw 为空或解析失败时明确报错；被拒候选只记 reason_code。
5. 单测覆盖：正常创建候选+问题、红线候选拒绝、重复 id 跳过、缺摘要文件报错、raw 畸形报错、rejected 计数。至少 +5 项。

## T3：G4/G5 前端接入校验命令

desktop/src/api.ts：consolidation.applyOutput(date, raw) 封装。ConsolidationRunner.tsx 在 ingest 阶段读取到 raw 正文后改为调用 applyOutput（替代仅存 localStorage 的 storeRawOutput 主路径）；调用失败时回退记录 localStorage raw 并 reportFailure，成功后按返回计数 notify done。localStorage 兜底记录保留（字段与现状一致）。同步更新 consolidationFlow.ts / ConsolidationRunner 相关单测文案与断言。

## T4：G6 注入统计宿主落盘

新增 commands/habits.rs 的 habit_stat_inject 命令：输入 ids: Vec<String>，非空时对每个 id 调 HabitStore::append_stat("inject", Some(id), None, now)，已存在 append_stat 校验（inject 在 STAT_TYPES）。返回 { recorded: n }。

desktop/src/api.ts：habits.appendStatInject(ids)。desktop/src/shell/HomeChat.tsx 在 recordHabitInjection(applied.injected) 之后对同一批 ids 调用宿主命令，fire-and-forget（catch 只 console.warn，不影响对话提交）。habitInjection.ts 的 localStorage 记录保留作前端兜底；HabitCardsPage 指标页若直接可读宿主 inject 统计则一并切换（改动小才做，否则留待后续）。至少 1 项宿主单测（append inject + read_stats 可见）。

## T5：G12/H7 接入整理运行器

新增 commands/habits.rs 两个命令：

1. habit_curator_review：读生效卡 + read_stats（全部统计行）→ habit_audit::review_active_cards(cards, &stats, now) → 返回 CuratorReport。不改卡。
2. habit_quote_audit：读生效卡 + violations 统计 → 以本次 digest threads 用户消息正文为 QuoteSample（context_id=thread_id，样本上限复用 review 既有常量；无样本如实 Unknown）→ audit_active_quotes → record_quote_audits 写 quote_audit 统计 → 返回 QuoteAuditReport。

接入点：ConsolidationRunner 在 applyOutput 成功后调用上述两命令（各自独立 try/catch，失败只 console.warn，不阻断 done）。通知文案追加复核标记数与抽查结果（有标记/不相符时才提）。

界面（动工前先用 screenshots-design-inspiration MCP 搜 2–3 个同类「卡健康/复核状态」布局，记录 ≤2 句借鉴要点）：HabitCardsPage 生效卡行增加「可复核」「冲突」小徽标（挂载时调 habit_curator_review 一次；失败静默）。单个既有文件改动尽量 ≤40 行。

## T6：搁置口径统一

desktop/src/habitNotice.ts overdue 条件 idleDays >= 7 改为 idleDays > 7（第 8 天起算逾期，与宿主 stale_open_questions 一致），同步更新 habitNotice.test.mjs 断言。

## 验证

1. cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1（基线 1360 passed，只增不减，0 warning）
2. cd desktop && npx tsc --noEmit → EXIT=0
3. cd desktop && npm run test（基线 150，只增不减）
4. cd desktop && npm run build → EXIT=0
5. python3 tests/verification/desktop/run_habit_regression.py → 16 PASS
6. python3 tests/verification/desktop/verify_hardening_2043.py → 77/77
7. python3 tests/verification/desktop/verify_project_601.py → 15/15；verify_project_602.py → 18/18
8. 证据落 tests/verification/desktop/evidence/od-0255/verify_output.txt

## 记录

STATUS.md 追加「OD-0255 执行记录」，逐项写实际命令与结果、计划偏差、未验证事项；不改 STATUS/REVIEW/PLANS 既有内容。明确不做：不接 writing_diff 自动候选（保持只进摘要）、不做 G9 界面、不做 T14、不动四象限与甘特。
