# 工作事实与知识库联动架构（OD-KM-500 规划成果一）

> 状态：已批准（2026-11-09 用户裁决：提交推送二阶段后从干净基线起步；项目线保留；素材篮放文案写作页签）。实施状态以 PLANS.md / REVIEW.md 为准。

## 一、实体关系与数据流

```
WorkRecord（work/工作记录/，事实源，AI 不可覆盖）
   ├─ linked_task_ids ──→ Task（work/任务/，正式实体；新增 project_id 单向挂接、related_knowledge）
   │                          └─→ Project（work/项目/，最小实体：名称/目标/状态/起止）
   ├─ related_knowledge ──→ Knowledge（knowledge_vault，只读引用，不回写 work/）
   └─ AI 分析顺车 ──→ 素材候选（暂存，照 .candidates 模式）
                          │ 用户审核（批准/退回/拒绝/暂缓）
                          ▼
                    WritingMaterial（work/素材篮/<material_id>.md，五态审核状态）
                          ├─ related_task_ids / related_project_id / related_knowledge_ids
                          ├─ source_type + source_id + source_quote + source_location（来源追溯）
                          └─→ material_query（已确认素材接口）──→ 第四阶段文稿生成
                                                                    └─ 用户确认成果 ──→ vault_archive 回流知识库
```

## 二、分层纪律

事实提取 → 实体关联 → 知识推荐 → 素材提取 → 人工审核 → 知识沉淀（方案 §六）。AI 无任何直接写正式任务状态、排期、素材审核状态、已审核知识的通道；所有确认由用户触发、宿主落盘。

## 三、来源追溯机制

- 每条素材必填 source_type（work_record/task/project/knowledge/user）+ source_id + source_quote（原文引用）+ source_location（文件内定位）。
- 素材指纹 = sha256(source_id + 归一化 source_quote)，同指纹拒绝重复创建；后续进展识别为关联既有素材并追加引用，不覆盖历史内容。
- 反向查询（知识条目 → 引用它的记录/任务/素材）全量扫描 work/ frontmatter 派生，不落盘、可重建。

## 四、与知识库边界

- 知识库 Raw/Wiki/Governance/Index 四层零改动；本阶段只读引用 + 反向查询。
- work/ 临时层（记录/任务/排期/素材篮）永不进入知识库；素材篮不参与 work_cleanup 定期清理（显式排除）。
- 素材引用继承源数据权限；用户确认素材不解除数据安全限制；AI 调用零新增通道（搭既有逐条记录分析顺车，继承 gate_user_text + masker 脱敏链）。

## 五、复用清单（不重复建设）

WorkRecord/Task 实体与双向回写（OD-WR-307）、TaskCandidate 暂存-确认流、vault_search 词法检索、vault_index 可重建索引、vault_link 双链解析、knowledge_governance/knowledge_review 治理、vault_archive 成果归档通道（OD-WR-312）、priority_rules 象限视图、Markdown+frontmatter 原子写纪律、serde default 零迁移模式、sha256.rs。
