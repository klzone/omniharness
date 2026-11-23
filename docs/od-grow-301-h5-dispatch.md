# OD-GROW-301 H5 派工：复盘失败自动重试一轮

## 任务编号与名称
OD-GROW-301 / H5：周期复盘生成或校验失败后自动重试一轮（D1=允许）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（H5 段）
- 批准时间与批准人：2026-11-23 / 用户（D1 答复：允许重试）
- 允许修改的文件或目录：desktop/src/shell/ReviewArchive.tsx（复盘生成流程处）、desktop/src/reviewRetry.ts（新增纯函数）与其测试 reviewRetry.test.mjs；desktop/src/api.ts（如需类型）；证据 tests/verification/desktop/evidence/od-grow-301/h5/
- 明确不做的事：不改 scene_gate、不改超时与取消行为、不改 engine/、不改 lib.rs 宿主命令、不做界面新组件（只在既有生成流程加重试）

## 背景事实（已核实）
- ReviewArchive.tsx 的生成流程：composeConversationPrompt → host.submitTask → 轮询 taskStatus → taskResult → worklogPeriodReviewGenerate（宿主校验 + 落草稿）。任一步失败目前直接 setNotice 错误并结束。
- H1 的结构化校验由宿主 worklog_period_review_generate 内部完成（review_store::validate_body 硬门禁 + H3 软检查），失败会返回 Err。
- H3 已在 confirm_review 回执加 habits 字段。

## 功能要求
1. 新增 desktop/src/reviewRetry.ts：纯函数 nextReviewStep(state) -> "idle" | "submit" | "await" | "ingest" | "retry" | "failed"。
   - state 含 attempt（0/1）、phase、failReason。
   - 规则：attempt=0 失败 → retry（重新组装提示词：在原提示词末尾追加「上一次尝试失败，原因：…；请修正后重新输出」）；attempt=1 失败 → failed（不再重试）；成功 → ingest。
   - 表驱动测试覆盖全部转移：首轮成功、首轮失败进重试、重试成功、重试失败终止、重复触发不叠加（attempt 已为 1 时不再 retry）。
2. ReviewArchive.tsx 的生成函数接入：失败时若 attempt=0 且失败原因可重试（模型未完成、正文校验失败、结果为空——即非用户取消/非准入拒绝），组装重试提示词再提交一次；attempt=1 后失败照旧进铃铛错误提示（setNotice err），不弹系统弹窗。
   - 重试在界面上显示「第一次生成未成功，正在重试…」一次。
   - 重试提交的统计：不新增宿主接口，重试信息记入STATUS即可（统计文件接入留给 G11）。
3. api.ts：如需给 submitTask 增加 opts 参数已存在（provider/model/reasoning_effort），不改签名；不改函数行为。

## 测试
- reviewRetry.test.mjs：表驱动全转移 + 取消/准入拒绝不重试 + attempt 边界。
- npm run test 全绿且 passed ≥ 现有 97 + 新增；npx tsc --noEmit 0；npm run build 0。

## 验证
- cd desktop && npm run test；npx tsc --noEmit；npm run build
- 证据落 tests/verification/desktop/evidence/od-grow-301/h5/verify_output.txt
- 真实模型端到端重试行为标「未验证」（需要实机）

## 硬约束
- 不修改 runtime/、engine/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；npm passed 数不得减少；同一失败测试修两次不过即停
- ReviewArchive.tsx 既有功能（生成、确认、导出、筛选）行为不变

## 交付
- 改动文件清单 + 测试数字
