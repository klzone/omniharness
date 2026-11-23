# OD-GROW-301 G8 派工：AI 原稿快照 + 写作差异习惯来源

## 任务编号与名称
OD-GROW-301 / G8：章节 AI 原稿落盘 + 确定性逐章节差异 + writing_diff 候选来源

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G8 段，2026-11-23 用户批准）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：desktop/src-tauri/src/writing_section.rs（生成/确认路径加 AI 快照保存 + 测试）；desktop/src-tauri/src/document_store.rs（只加必要的辅助，若有）；desktop/src-tauri/src/writing_diff.rs（新增：确定性逐章节差异纯逻辑 + 测试）；desktop/src-tauri/src/consolidation.rs（只加调用 writing_diff 收集 writing_diff 来源，不改既有函数签名）；desktop/src-tauri/src/lib.rs 只加模块声明；证据 tests/verification/desktop/evidence/od-grow-301/g8/
- 明确不做的事：不自动把任何候选写入生效卡（writing_diff 候选仍走 G3 校验与用户确认）；不改前端；不做界面；不改既有校验逻辑

## 背景事实（G0 已核实，文件事实）
- document_store.save 系列函数已支持 VersionSource::Ai（枚举存在，但生产路径从未使用，只出现在测试里）。
- writing_section.rs confirm_section（约 795 行）保存用户快照（VersionSource::User）。
- 章节正文用 ## n. 标题分节；writing_section::parse_body_sections（约 397-437 行）可按章拆分。
- 生成章节的入口：generate_section_prompt（约 1451 行起测试提示）；AI 输出由界面提交并确认。

## 功能要求
1. AI 原稿快照：在章节 AI 生成结果到达宿主（经现有生成确认链路）时，把 AI 原稿以 VersionSource::Ai 保存到 document_store 快照（label=「AI 原稿」）。**接入点核实后选最小改动**：若生成输出暂不落盘（只在界面预览），则在 confirm_section 增加可选参数 ai_draft_text: Option<&str>——界面传入时以 VersionSource::Ai 保存快照，不传时行为不变。前端接入（WritingSectionEditor 传入 AI 原稿）属界面改动，本轮只做宿主侧接口（Option 参数向后兼容，既有调用不受影响），前端接线记入缺口（下一轮界面任务）。
2. writing_diff.rs（新增）：函数 section_diffs(text_a: &str, text_b: &str) -> Vec<SectionDiff>：
   - 按 parse_body_sections 拆章，逐章比较；章内按段落（空行分段）比较，产出被删除与新增的句子片段（片段 ≤100 字符）。
   - 返回 {section_no, removed: Vec<String>, added: Vec<String>}；不含整章全文。
   - 纯函数，无 IO。
3. writing_diff 来源收集（consolidation.rs 只加函数 collect_writing_diffs）：扫描 work/写作/ 下有 ≥2 个版本的文稿，取最近两版做逐章节差异；「同一短语在至少两份不同稿件里被删」判定：对每个被删片段查跨稿件重复（同一片段在 ≥2 个不同 writing_id 的 removed 列表出现），命中才进候选来源列表（origin=writing_diff）；单次删除不提。产出作为额外摘要段落（附 writing_id 与 section_no，不存全文）。
4. 单测（writing_diff.rs mod tests + writing_section.rs 补测试）：
   - AI 快照：confirm_section 带 ai_draft_text 时产生 VersionSource::Ai 快照；不带时不产生（行为不变）。
   - section_diffs：章删除/新增/未变；片段截断 ≤100 字符；空文本。
   - 跨稿重复判定：两份稿件同片段命中；单份不命中。

## 验证
- cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1 全绿且 passed ≥ 1349 + 新增
- 证据落 tests/verification/desktop/evidence/od-grow-301/g8/verify_output.txt

## 硬约束
- 不修改 runtime/、engine/、前端；不动模型路由与凭证；不提交、不推送、不打标签
- 不修改既有测试断言（可追加）；cargo passed 数不得减少；同一失败测试修两次不过即停
- 测试只用合成数据；不把案件、线索、人物画像类内容写进代码、测试或日志
- 差异片段只进摘要与候选来源，不进统计正文

## 交付
- 改动文件清单 + 测试数字 + 前端接线缺口说明
