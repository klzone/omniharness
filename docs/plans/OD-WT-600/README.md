# OD-WT-600 第四阶段：AI 文稿写作 + 事实核验 + 成果归档

> 规划会话产出（2026-11-09，用户批准 15 卡版本）。实施状态以 PLANS.md / REVIEW.md 为准。

## 文档索引

1. AI_WRITING_ARCHITECTURE.md — 总体架构、数据流、八层工作流、前三阶段边界。
2. DOCUMENT_GENERATION_DESIGN.md — WritingTask 模型、版本管理、素材包、大纲/分章、审阅、导出引擎与样式包、归档回流。
3. FACT_VERIFICATION_DESIGN.md — 核验记录结构、八类结论、双引擎、引用追溯、风险提示。
4. PHASE4_IMPLEMENTATION_PLAN.md — 15 张任务卡（601–615）、依赖图、派发节奏。
5. PHASE4_ACCEPTANCE_CRITERIA.md — 十三场景对照、验收命令总表、安全口径。
6. PHASE4_ROADMAP.md — 实施顺序、裁决记录、接口、暂缓事项、扩展方向。

## 核心闭环

写作需求（自然语言解析）→ 写作任务（九态）→ 素材收集（只取已确认）→ 素材包 → 大纲（用户确认门禁）→ 分章节写作（四分法标注）→ 事实核验（确定性+AI 双引擎）→ 审阅修订（版本快照）→ 定稿（用户触发）→ 导出（MD/Word 公文/PDF 阅读/公众号 HTML）→ 归档（MD 进知识库）→ 知识回流（治理流程）。

