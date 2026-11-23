# OD-GROW-301 G9 派工：写作范例作为知识库候选

## 任务编号与名称
OD-GROW-301 / G9：定稿段落作为知识库暂存卡候选（走现有准入，不绕过）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G9 段，2026-11-23 用户批准）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：desktop/src-tauri/src/writing_section.rs（confirm_section 路径加知识库暂存调用，≤30 行改动）；desktop/src-tauri/src/knowledge_examples.rs（新增：范例段落提取纯函数 + 测试）；desktop/src-tauri/src/lib.rs 只加模块声明（knowledge_review_stage 已存在，不加命令）；证据 tests/verification/desktop/evidence/od-grow-301/g9/
- 明确不做的事：不直接写知识库（必须走 knowledge_review_stage 待审核队列）；不做界面；不改前端；不改 knowledge_* 既有模块

## 背景事实（已核实）
- knowledge_review_stage 命令已存在（lib.rs 约 5750 行；api.ts 2176 行），签名 (resultText, sourcePaths)，进入知识审核待审队列，由用户逐条审定。
- confirm_section 是章节确认唯一入口（含修订路径）。
- 定稿段落 = 用户确认的章节正文。

## 功能要求
1. knowledge_examples.rs（新增）：函数 extract_example_paragraphs(body: &str) -> Vec<String>：
   - 按空行分段，取长度 ≥50 字符且 ≤300 字符的段落（太短不成范例，太长截断不取）；
   - 过滤纯标题段（## 开头）与列表符号开头的段；
   - 最多取 3 段（每章）；纯函数无 IO。
2. confirm_section 接入：确认成功后，对本次确认的章节正文调 extract_example_paragraphs；对每个段落先过 RulePolicy::reject_keywords（命中跳过该段，不报错），再过 mask_privacy 脱敏；然后把脱敏后文本调 knowledge_review_stage 的内部等价函数（review_store 或 knowledge_review.rs 的 stage 函数，先核实现有 stage 入口再调用），sourcePaths=[writing_id + 章节]；进入待审核队列，由用户审定。
   - 若 stage 失败，记 console/log 但不影响确认主流程（章节确认不因知识库候选失败而失败）。
   - 加开关：不做新配置文件——用现有 knowledge 治理的准入（stage 本身就是待审队列，不需要额外开关），如实记录取舍。
3. 单测（knowledge_examples.rs mod tests）：提取规则（长度边界 49/50/300/301、标题段过滤、列表段过滤、3 段上限）；confirm 接入测试（合成 body 确认后 stage 被调、红线段跳过）；stage 失败不影响确认。

## 验证
- cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 全绿且 passed ≥ 1356 + 新增
- 证据落 tests/verification/desktop/evidence/od-grow-301/g9/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/、前端；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言（可追加）；cargo passed 数不得减少；同一失败测试修两次不过即停
- 不绕过知识库准入：只进待审核队列，不直接写知识库
- 测试只用合成数据；不把案件、线索、人物画像类内容写进代码、测试或日志

## 交付
- 改动文件清单 + 测试数字 + stage 接入点说明
