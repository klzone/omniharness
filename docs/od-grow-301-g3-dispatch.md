# OD-GROW-301 G3 派工：提取契约与宿主校验

## 任务编号与名称
OD-GROW-301 / G3：习惯候选提取契约（Python 提示词组装）与宿主校验（Rust）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G3 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：新增 engine/omnidesk_engine/habit_extract.py、desktop/src-tauri/src/habit_extract.rs；对应测试（habit_extract.rs mod tests + engine/omnidesk_engine/ 下新增测试文件）；证据 tests/verification/desktop/evidence/od-grow-301/g3/
- 明确不做的事：不改 task_extract.rs / habit_cards.rs / consolidation.rs 既有函数；不做界面；不实际调用模型

## 背景事实（已核实）
- 分工先例：task_extract.rs 的 Python 只组装提示词、Rust 校验模型输出。habit_extract.py 同样只组装提示词（不落盘、不调模型），habit_extract.rs 负责解析与校验模型 JSON 输出。
- habit_cards.rs（G1）已提供：候选/生效/驳回存储、RulePolicy 红线检查辅助、CandidateVerdict 风格参考 task_store::CandidateVerdict（schema=omnidesk.verdict.v1）。
- consolidation.rs（G2）已提供：DigestPlan（threads/messages/digest_text/stats）与摘要格式【线程】… + [m-N] …。
- 模型输出的 JSON 结构必须能从纯文本中解析（G4 会取 taskResult 的 content 交给本模块）。

## 功能要求
1. habit_extract.py：函数 build_habit_prompt(digest_text, threads_meta) -> str。中文提示词，包含：
   - 接地规则：只保存能指到某条用户消息的内容；不得根据话题、语气、使用的工具推断用户特质或偏好；一次性请求不得变成长期偏好，除非用户用「以后、一律、总是、从现在起、记住、别再、不要再」这类信号明确说了长期要求，或明确表达长期好恶；拿不准就不要。
   - 三条补充：不提取判断、假设、结论性观点；用户转述别人的话、AI 建议而用户没采纳的，一律不算；每条必须给出来源消息编号和命中的信号词。
   - 输出契约 omnidesk.habit-candidates.v1：{"items":[{"text","kind","scope_hint","message_id","cue","check"?}],"open_questions":[{"text","message_id"}]}，items ≤10、open_questions ≤10；text ≤60 字、问题 text ≤80 字；check 三种（banned_phrases/paragraph_max_chars/required_phrases）只能给结构化对象。
   - 输出必须是纯 JSON，不加解释文字、不调用工具。
   - scope_hint 只能是 "scene" 或 "global"。
2. habit_extract.rs：函数 parse_habit_output(raw: &str, digest: &DigestPlanRef, policy: &RulePolicy, existing: &ExistingCardsRef, rejection: &RejectionRef, now) -> HabitParseResult。
   - 解析：从模型输出文本提取 JSON（容忍代码块围栏）；解析失败返回明确错误，不猜测。
   - 校验每条 item：
     a. message_id 必须在本次摘要内且 role==="user"；
     b. cue 必须是该消息文本的子串，且 cue 整体在信号词白名单（SIGNAL_WORDS 常量：以后/一律/总是/从现在起/记住/别再/不要再/我习惯/我喜欢/我不喜欢/以后都用/每次都；白名单在代码里，测试固定）；
     c. 无明确信号（cue 不在白名单）的条目：只有 existing.repeated_texts 能字面匹配到同一 text 在两天及以上出现才收，origin=repeated；否则拒绝（reason=NO_SIGNAL）；
     d. scope：默认 scene（取来源线程场景）；scope_hint=global 且消息文本含「所有/一律/任何」→ global；否则降级为 scene（reason 记录 GLOBAL_DOWNGRADED，不算拒绝）；
     e. text 长度 ≤60 字符（超限拒绝 TEXT_TOO_LONG）、非空、无控制字符；
     f. 红线：text 或 check.phrases 含 reject_keywords → 拒绝（REDLINE），错误只记 reason_code 不存正文；
     g. 与 existing 卡片去重（同 text 已存在 candidate/active）→ 拒绝 DUPLICATE；rejection 记录次数 ≥2 → 拒绝 REJECTED_BEFORE；
     h. check 字段：三种之一，结构合法（phrases 非空数组且每条 ≤20 字符；limit 10..=2000）。
   - open_questions：message_id 必须在摘要内且 role==="user"；text ≤80 字；必须包含未决表达白名单词之一（UNCERTAIN_WORDS：还没想好/待定/拿不准/要不要/再说/回头再定/没定；常量在代码里），否则拒绝。
   - 数量：items >10 时只取前 10 并记录 OVERFLOW；open_questions 同理。
   - 每条通过的候选附 CandidateVerdict（复用 task_store::CandidateVerdict::new("habit-candidate.v1", checks)），checks 包含 cue_in_source、signal_whitelisted、scope_valid、not_duplicate、redline_clean 各一条 VerdictCheck（pass 布尔）。
   - 被拒条目只计数 + reason_code，不落正文。
   - 产出 HabitParseResult { candidates: Vec<StagedHabitCandidate>, open_questions: Vec<StagedOpenQuestion>, rejected: Vec<RejectedEntry{reason_code}>, verdicts }。
3. StagedHabitCandidate 结构对齐 habit_cards.rs 的卡片（可直接复用其类型转换函数或 From 实现）。

## 测试
- habit_extract.rs mod tests：≥20 组表驱动合成样例，覆盖：明确表述通过（含 verdict 全 pass）；一次性请求被拒（NO_SIGNAL）；假设类语句被拒（NO_SIGNAL）；转述被拒（NO_SIGNAL）；AI 建议未被采纳被拒（NO_SIGNAL）；编号不存在被拒（MESSAGE_NOT_FOUND）；信号词不在原文被拒（CUE_NOT_SUBSTRING）；global 无依据降级（仍收但 scope=scene + GLOBAL_DOWNGRADED）；重复被拒（DUPLICATE）；已驳回两次被拒（REJECTED_BEFORE）；红线词被拒（REDLINE 且错误不回显命中词）；text 超长被拒；check 非法被拒；未决问题通过；未决问题缺不确定词被拒；JSON 损坏被拒；items 超 10 截断；cue 在白名单但消息是 assistant 被拒。
- habit_extract.py：≥3 组测试（提示词包含接地规则关键词、包含输出契约 schema 字段、包含信号词列表）。
- 允许在 engine/omnidesk_engine/ 新增 habit_extract 测试文件（命名 habit_extract_test.py 或并入现有测试风格——先看现有 task_extract 的测试组织再定）。

## 验证
- cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 全绿且 passed ≥ 1317 + 新增
- cd engine && python3 -m pytest omnidesk_engine -q 全绿（若 engine 无 pytest 配置则用 python3 -m unittest discover；先查现有测试组织再定，如实记录）
- 证据落 tests/verification/desktop/evidence/od-grow-301/g3/verify_output.txt（两端测试输出）

## 硬约束
- 不修改 runtime/；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言；cargo/npm passed 数不得减少；同一失败测试修两次不过即停
- 测试只用合成数据；案件、线索、人物画像类内容不得出现在代码、测试、日志中
- 提示词与校验不引用真实用户材料

## 交付
- 改动文件清单 + 测试数字 + 表驱动样例数
