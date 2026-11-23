# OD-GROW-301 H3 派工：习惯检查（生效卡验证器）

## 任务编号与名称
OD-GROW-301 / H3：读取生效习惯卡的 check 规则，在复盘与写作校验路径执行并记统计

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（H3 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：新增 desktop/src-tauri/src/habit_checks.rs；desktop/src-tauri/src/lib.rs 只加模块声明；review_store.rs / writing_review.rs 只在现有校验通过后追加 habit 检查调用（不动既有断言逻辑）；证据 tests/verification/desktop/evidence/od-grow-301/h3/
- 明确不做的事：不改 habit_cards.rs 既有函数（只读调用）；不做界面；不改 engine/；不改既有软检查行为

## 背景事实（已核实）
- habit_cards.rs（G1）提供：active 目录读取（生效卡）、卡片结构含 check（三种：banned_phrases{phrases}、paragraph_max_chars{limit}、required_phrases{phrases}）、scope（scene/global）、统计追加（STAT_VIOLATION）。
- review_store.rs 的 validate_body 是硬门禁（复盘正文确认前校验）。
- writing_review.rs 有写作校验路径。
- 统计文件 habits-stats.jsonl 由 G1 定义，追加 type=violation 行（含卡片 id）。

## 功能要求
1. 新增 habit_checks.rs：函数 check_habits(scope_scene: Option<&str>, body: &str, store: &HabitStore) -> HabitCheckReport。
   - 读取生效卡（status=active），过滤 scope 匹配（卡 scope=global 或 scene 等于当前场景）。
   - banned_phrases：body 含任一短语 → violation（记录卡片 id + 命中的短语在 body 中的存在性，不记 body 正文）。
   - paragraph_max_chars：按空行分段，任一段 > limit → violation。
   - required_phrases：body 缺少任一必备短语 → violation。
   - 产出 HabitCheckReport { checked_cards: Vec<id>, violations: Vec<{card_id, kind, detail_code}> }；无正文引用。
   - 把 violation 追加进 habits-stats.jsonl（type=violation，id=card_id）。
2. 接入点（软检查，不硬拒）：
   - review_store.rs：复盘确认（confirm_review）路径上调用 check_habits（scope=复盘周期对应场景 report），violation 只记统计与随确认回执返回（receipt 加 habits 字段），不阻止确认。
   - writing_review.rs：写作核验路径上调用 check_habits（scope=material_writing），同样只记不拦。
3. 单测（habit_checks.rs mod tests）：
   - banned 命中/未命中；paragraph 超限边界（limit 恰好等于段长/超 1 字）；required 缺失/齐备；scope 匹配（global 生效、scene 相等生效、scene 不等不查）；violation 写统计（读回验证）；无生效卡时空报告；报告不含正文。

## 验证
- cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 全绿且 passed ≥ 1319 + 新增
- 证据落 tests/verification/desktop/evidence/od-grow-301/h3/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；cargo passed 数不得减少；同一失败测试修两次不过即停
- 测试只用合成数据；不把案件、线索、人物画像类内容写进卡片、测试或日志

## 交付
- 改动文件清单 + 测试数字
