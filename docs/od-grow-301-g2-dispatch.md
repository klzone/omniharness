# OD-GROW-301 G2 派工：每日摘要收集（宿主，不调用模型）

## 任务编号与名称
OD-GROW-301 / G2：整理摘要收集、红线脱敏分流、到点判定与预演命令

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G2 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：新增 desktop/src-tauri/src/consolidation.rs、desktop/src-tauri/src/commands/consolidation.rs；lib.rs 与 commands/mod.rs 只加模块声明与命令注册；证据目录 tests/verification/desktop/evidence/od-grow-301/g2/
- 明确不做的事：不修改 scheduler.rs 与其 schema；不调模型；不做界面；不改 habit_cards.rs 的既有函数签名（可新增调用）；不改 engine/

## 背景事实（ARCHITECT 与 G0 已核实）
- 线程存储 thread_store.rs：每线程 JSON 含 id/tag/title/preset/messages/createdAt/updatedAt；消息必需 id/role/text，无通用时间戳；tag==="plain" 即未分类；thread_store::load() 全量读取无分页——本任务不要直接复用 load() 拉全部正文，改为复用其目录遍历与单文件校验逻辑新增一个"元数据列表 + 按线程读正文"的轻量读取（可在 thread_store.rs 增加 pub fn，或全部放在 consolidation.rs 内部实现，二选一，说明取舍）。
- pending_writes::scene_for_intent 只拦 weekly_report / material_writing / project_management；整理 intent 用 habit_consolidation 不受影响。
- profile_rules::RulePolicy：reject_keywords 判断 + mask_privacy(text, &policy) 返回 (脱敏后文本, MaskCounts)。命中分类：若 mask_privacy 后文本不再含 reject_keywords → 隐私类，走脱敏通过；若脱敏后仍命中 → 禁入类别，进待审定；无法判定（空文本/超限）也进待审定。
- habit_cards.rs（G1 已交付）：提供待审定条目存储（pending-review.v1）与统计追加。收集器把禁入/不确定消息写入待审定存储（脱敏预览 + 线程/消息引用 + 原因代码），不写原话。
- consolidation state（水位）已由 G1 定义（consolidation-state.v1：每线程 last_message_index、last_run_at、last_error）。

## 功能要求
1. 配置 config/consolidation.v1.json（在应用数据根 config/ 目录，与 scheduler.v1.json 同级）：schema=omnidesk.consolidation.v1、enabled（默认 false）、hour（0-23，默认 21）、minute（0-59，默认 30）、provider（可空）、model（可空）、sources（字符串数组，默认 ["plain_threads","project_tasks"]）、min_user_messages（默认 6）、min_chars（默认 1500）、max_threads（默认 20）、max_digest_chars（默认 24000）。deny_unknown_fields；损坏或未知字段明确报错。提供 load 函数与 Default。
2. consolidation_tick(now) -> {due: bool}：规则与 scheduler.rs 一致（本地时区按 utc_offset_minutes 缺省东八区；enabled=false 永不到点；错过补跑一次：last_run_at 不是今天且当前时间过 hour:minute 即 due；每天最多一次由记录 last_run_at 保证）。状态文件复用 habits/state/ 下的 consolidation-state.v1（G1 已有结构，可加字段 schedule last_run_date/last_run_at/last_error——若与 v1 冲突就用新文件 consolidation-schedule.v1 放 config/，说明取舍）。
3. 摘要收集 collect_digest(paths, &config, &policy, now) -> DigestPlan：
   - 来源=plain_threads：遍历线程目录，取 tag==="plain" 的线程，按 updatedAt 降序最多 max_threads 个；每线程只取水位之后、role==="user" 的消息。
   - 逐线程红线预扫描（逐消息分类）：隐私命中 → mask_privacy 后纳入摘要（保留原意）；禁入或不确定 → 该消息进待审定存储（写 pending-review.v1 条目：脱敏预览 ≤2000 字符、thread_id、message_id、reason_code、created_at），不计入摘要；整线程全部消息都被排除则跳过该线程并计数。
   - 线程纳入条件：水位后新增用户发言 ≥ min_user_messages 条 或 总字数 ≥ min_chars；不满足则该线程跳过（水位也推进到当前末尾，记录 skipped）。
   - 来源=project_tasks：当日状态有变化的任务（task_store 读取），只取标题与状态变化（from → to），作为上下文段落。
   - 拼接摘要：每线程一段（线程标题 + 用户发言列表，带消息号）；超出 max_digest_chars 截断并如实记录 truncated 线程数/消息数。
   - 产出 DigestPlan { threads: [...], digest_text, stats { thread_count, message_count, char_count, skipped_threads, redline_masked, redline_pending, truncated } }。
4. 预演命令 consolidation_preview：只调 collect_digest 的 dry-run 路径（不写待审定存储、不推水位、不调模型），返回将要读取的线程数/消息数/摘要字符数/跳过数/待审定数。
5. 应用命令 consolidation_collect：执行收集并写入待审定存储与水位（不调模型）；摘要文本落 habits/state/digest-<date>.json（供 G4 取用），含 digest_text 与 stats。返回 stats。
6. 命令注册放 commands/consolidation.rs：consolidation_tick、consolidation_preview、consolidation_collect、consolidation_config_get、consolidation_config_set（enabled/hour/minute/provider/model/sources 等字段校验后原子写）。

## 测试（consolidation.rs mod tests，合成线程 JSON 夹具）
覆盖：水位增量（第二次运行只取新增）、阈值判断（消息数/字数两侧）、隐私脱敏通过（脱敏后进摘要且原词不出现）、禁入进待审定（含 reason_code 与脱敏预览、不进摘要）、整线程被排除跳过计数、截断（digest_chars 上限）、到点判定（enabled=false 不到点、未到时间不到点、过点且今天未跑 → due、今天已跑 → 不 due、错过昨天 → 补跑 due）、配置损坏与未知字段报错、预演不写任何文件（目录快照对比）、project_tasks 状态变化上下文。

## 验证
- cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 全绿且 passed ≥ 1307 + 新增
- python3 tests/verification/desktop/export_commands_2043.py --write tests/verification/desktop/evidence/od-harden-2043/commands-baseline.json（列出新增命令名）
- 证据落 tests/verification/desktop/evidence/od-grow-301/g2/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/、scheduler.rs；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；cargo/npm passed 数不得减少；同一失败测试修两次不过即停
- 测试只用合成数据；案件、线索、人物画像类内容不得出现在代码、测试、日志中
- 任何待审定条目只存脱敏预览与引用，不存原话

## 交付
- 改动文件清单 + 新增命令名 + 测试数字 + 摘要结构样例（合成数据）
