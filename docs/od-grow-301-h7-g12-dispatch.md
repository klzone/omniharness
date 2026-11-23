# OD-GROW-301 H7+G12 派工：引用抽查与确定性复核

## 任务编号与名称
OD-GROW-301 / H7：习惯卡引用抽查；OD-GROW-301 / G12：生效卡确定性复核（可复核/冲突标记）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（H7、G12 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：新增 desktop/src-tauri/src/habit_audit.rs（H7 抽查 + G12 复核纯逻辑 + 测试）；desktop/src-tauri/src/lib.rs 只加模块声明；desktop/src-tauri/src/habit_cards.rs 只加必要的读取辅助（若有）；证据 tests/verification/desktop/evidence/od-grow-301/h7-g12/
- 明确不做的事：不自动改动、不自动删除、不自动归档；不做界面（复核结果只进统计/日志，展示留给后续）；不改既有校验路径

## H7 引用抽查（结果只进指标，不拦截）
1. 函数 audit_quote(card, sample_bodies: &[（context_id, body)]) -> QuoteAuditResult：
   - 对带 banned_phrases/required_phrases 的生效卡：检查卡声称命中（habits-stats 有 violation 记录）时，sample bodies 里是否真的能找到对应短语；找不到 → 不相符（mismatch）。
   - 产出 mismatch 比例（mismatches/checked），写统计行 type=quote_audit（含 card_id 与 mismatch 布尔，不含正文）。
2. 测试：命中相符（比例 0）、不符（比例 1）、无 violation 记录的卡跳过、无样本时返回 unknown（不估算）。

## G12 确定性复核
1. 函数 review_active_cards(cards, stats_cursor, now) -> CuratorReport：
   - 90 天没有被注入命中（stats 里无 inject 记录且 last_seen_at 早于 90 天前）→ 标 review_due=true；
   - 同 scope 内矛盾卡：同一短语既出现在一张卡的 banned_phrases 又出现在另一张卡的 required_phrases → 双方都标 conflict=true；
   - 只产出报告（Vec<{card_id, flags}>），不改动任何文件。
2. 测试：90 天边界（第 89/90/91 天）、矛盾判定（同 scope 命中、跨 scope 不算）、多卡多短语交叉、空输入。

## 验证
- cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 全绿且 passed ≥ 1334 + 新增
- 证据落 tests/verification/desktop/evidence/od-grow-301/h7-g12/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/、前端；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；cargo passed 数不得减少；同一失败测试修两次不过即停
- 测试只用合成数据；报告不含正文

## 交付
- 改动文件清单 + 测试数字 + 报告结构说明
