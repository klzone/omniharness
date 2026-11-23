# OD-GROW-301 G10 派工：未决问题提及刷新与搁置提醒

## 任务编号与名称
OD-GROW-301 / G10：未决问题的确定性提及刷新（字符二元组重叠）与搁置提醒

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G10 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：desktop/src-tauri/src/consolidation.rs（只加刷新逻辑与测试，不改既有函数签名）；desktop/src-tauri/src/habit_cards.rs（只加 open question last_seen 更新函数，若已有则复用）；证据 tests/verification/desktop/evidence/od-grow-301/g10/
- 明确不做的事：不自动关闭任何问题；不接复盘引用校验；不改前端；不做界面

## 功能要求
1. 提及刷新：新一轮摘要（DigestPlan 的用户消息文本）与每个 open 问题做字符二元组（bigram）重叠判定：
   - 函数 question_bigram_overlap(question_text, message_text) -> f64（|bigrams(q) ∩ bigrams(m)| / |bigrams(q)|，去重集合）。
   - 阈值常量 QUESTION_BIGRAM_THRESHOLD: f64 = 0.34（测试固定并含边界样例：恰好等于阈值算命中）。
   - 任一用户消息与问题的重叠 ≥ 阈值 → 刷新该问题 last_seen（写文件，原子写），不新建重复项。
2. 搁置判定：stale_days(first_seen/last_seen, today) → 超过 7 天未动的 open 问题进「提醒区」（返回列表，供 G2 摘要的提醒段与 G5 铃铛；本任务只在宿主提供 stale_open_questions() 读取函数 + 单测；G5 已有前端排序，宿主这份供摘要与统计用）。
3. 单测：去重刷新（同一问题两轮摘要不重复新建）、阈值边界（0.34 恰好命中、0.33 不命中）、无重叠不刷新、状态非 open 不刷新、7 天搁置判定边界（第 7 天不算、第 8 天算）、last_seen 更新原子性。

## 验证
- cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 全绿且 passed ≥ 1329 + 新增
- 证据落 tests/verification/desktop/evidence/od-grow-301/g10/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/、前端；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；cargo passed 数不得减少；同一失败测试修两次不过即停
- 测试只用合成数据

## 交付
- 改动文件清单 + 测试数字 + 阈值与判定函数说明
