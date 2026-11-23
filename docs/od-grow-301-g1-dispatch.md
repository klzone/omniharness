# OD-GROW-301 G1 派工：习惯卡片与候选的存储（Rust）

## 任务编号与名称
OD-GROW-301 / G1：习惯卡片、未决问题、驳回记录与待审定条目的存储层

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G1 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：新增 desktop/src-tauri/src/habit_cards.rs、desktop/src-tauri/src/commands/habits.rs；desktop/src-tauri/src/lib.rs 与 desktop/src-tauri/src/commands/mod.rs 只加模块声明与命令注册；tests/verification/desktop/evidence/od-grow-301/g1/（证据）
- 明确不做的事：不使用 pending_writes；不碰知识库根；不做任何界面；不改 engine/；不新增依赖

## 结构定义（v1，全部带 schema 字段）
存储根：<应用数据根>/habits/，子目录 candidates/、active/、archive/、open-questions/、state/、pending-review/。根目录构造沿用 AppPaths 模式（必须过 assert_local_root_allowed + assert_outside_repo 边界校验，参考 thread_store.rs 的 resolve/for_root 写法）。

1. 习惯卡 habit-card.v1：id、kind（rule/preference/format）、scope（{type:"scene",scene:string} 或 {type:"global"}，默认取来源线程场景）、text（1..=60 字符）、check（可选枚举：{type:"banned_phrases",phrases:string[]}、{type:"paragraph_max_chars",limit:uint}、{type:"required_phrases",phrases:string[]}）、evidence（thread_id、message_id、date，不存原话）、origin（explicit/repeated/writing_diff）、status（candidate/active/disabled/archived）、created_at、confirmed_at（可空）、last_seen_at、expect（可选 ≤60 字符）。
2. 未决问题 open-question.v1：id、text（1..=80 字符）、thread_id、message_id、first_seen、last_seen、status（open/closed/dropped）、reminded（uint，默认 0）。
3. 整理状态 consolidation-state.v1：每线程水位 {thread_id: last_message_index}、last_run_at、last_error（可空）。
4. 统计 habits-stats.jsonl：只追加行 {ts, type: inject|violation|accept|reject|consolidation_run|consolidation_fail, id?, reason_code?}，不存正文；损坏行跳过并计数（参考 event_log.rs）。
5. 驳回记录 rejection-memory.v1：短语哈希（sha256_hex，沿用 sha256.rs）→ 次数；次数 ≥2 不再作为候选提出（校验函数提供判断，调用方在 G3）。

## 存储行为
- 每张卡一个 JSON 文件（id 做文件名，id 只允许安全字符，长度 ≤64，沿用 thread_store 的 id 校验风格）；候选 ≤100 条、生效 ≤80 张，超限明确报错。
- 文件不存在 = 空列表（不报错）；损坏 = 明确报错（列文件名与原因），不静默放行、不猜测修补。
- 写入用临时文件 + fsync + rename 原子替换，沿用 worklog_records.rs 的 write_replace_atomically/write_new_atomically 写法（可直接复用其函数或同款实现）。
- 保存任何卡片文本（text/check.phrases/expect）前必须过 RulePolicy::reject_keywords 检查（profile_rules::RulePolicy），命中拒绝且错误信息不回显命中词。
- 七天未处理候选（created_at 距今 >7 天且仍 candidate）在候选列表读取时转入归档，不生效；无锁文件、无常驻线程。

## 命令（放 commands/habits.rs，lib.rs 只注册）
habit_candidate_list、habit_candidate_decide（批量：每条 accept/reject，accept 可改 scope，reject 可选 reason_code；reject 写驳回记录与统计）、habit_active_list、habit_card_update（停用/归档/复用，只归档不删除）、habit_open_question_list、habit_open_question_set_status、habit_pending_review_list、habit_pending_review_decide（批准/排除）。每个命令返回结构体（含 items、broken 计数），错误用 String，风格对齐 commands/reading.rs。

## 测试（mod tests in habit_cards.rs）
覆盖：结构校验（长度/枚举/上限）、红线拒绝（合成规则资产）、批量接受与驳回（含改 scope、驳回哈希累计、两次驳回后判定函数返回拒绝）、七天归档（注入旧 created_at）、损坏文件明确报错、旧版本兼容（无新可选字段的旧 v1 文件可读）、原子写（写入失败不留半文件）、统计追加与损坏行跳过。

## 验证
- cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 全绿且 passed ≥ 1296 + 新增
- python3 tests/verification/desktop/export_commands_2043.py 有意更新基线：python3 tests/verification/desktop/export_commands_2043.py --write tests/verification/desktop/evidence/od-harden-2043/commands-baseline.json（新增命令名要列出）
- 证据落 tests/verification/desktop/evidence/od-grow-301/g1/verify_output.txt

## 硬约束
- 不修改 runtime/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；cargo/npm passed 数不得减少；同一失败测试修两次不过即停
- 测试只用合成数据；案件、线索、人物画像类内容不得出现在卡片、测试、日志中

## 交付
- 改动文件清单 + 新增命令名列表 + 测试数字
