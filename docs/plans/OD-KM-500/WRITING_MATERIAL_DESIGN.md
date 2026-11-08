# 写作素材篮设计（OD-KM-500 规划成果二）

> 状态：已批准（2026-11-09）。配套任务卡见 PHASE3_IMPLEMENTATION_PLAN.md。

## 一、数据模型

WritingMaterial 落 work/素材篮/<material_id>.md（schema omnidesk.material.v1，Markdown+frontmatter，照 schedule_store 先例）：

- material_id / title / content
- material_type（十类白名单）：工作事实/工作成果/数据素材/经验做法/典型案例/问题不足/制度依据/工作计划/领导部署/特色亮点
- source_type（白名单）：work_record/task/project/knowledge/user
- source_id / source_quote（必填）/ source_location
- related_task_ids / related_project_id / related_knowledge_ids
- event_date / created_at / updated_at / confidence
- review_status（五态）：待审核/已确认/已退回/已拒绝/已归档
- confirmation_status（用户确认标记）/ tags
- ## 审核记录 追加段（照任务调整记录先例，只追加不删改）

## 二、AI 提取

搭既有逐条记录分析调用顺车（D-070 模式，零新增模型调用）：task_extract.py 输出 material_candidates 数组 + validate_material_payload；宿主校验 quote 非空、类型白名单、content ≠ quote（防复制冒充提炼）；候选进暂存不落正式素材；提示词约束照方案 §四（只提取有事实依据内容、区分事实/判断/计划/推测、不把计划写成成果、不夸大、不凭空生成）。

重复识别：确定性指纹（D-081）+ AI match_material_id 建议；同一事项后续进展关联既有素材追加引用，不覆盖。

## 三、人工审核

五态流转由用户触发：批准→已确认（进入可信集合）；退回/拒绝须填理由；暂缓保持待审核；已归档供长期沉淀。低置信度候选默认待审核。AI 无写审核状态通道（513 verify 断言）。

## 四、检索

按类型/状态/时间/任务/项目/知识依据过滤；material_query（509）默认只返回已确认素材，返回正文+来源+引用位置+审核状态+关联实体，供第四阶段检索（写作主题/类型/时间范围/项目范围条件）。

## 五、入口

素材篮页签挂文案写作场景（WritingStudio）内（2026-11-09 用户裁决）：列表（类型分组+状态筛选，待审核置顶）、审核就地操作、详情含来源追溯与打开原文、用户主动收藏新建。

