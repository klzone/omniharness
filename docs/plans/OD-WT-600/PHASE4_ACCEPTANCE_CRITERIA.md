# 第四阶段验收标准（OD-WT-600 规划成果五）

> 状态：已批准。任务卡验收命令见 PHASE4_IMPLEMENTATION_PLAN.md；本文件登记方案 §九 十三场景对照与安全验收口径。

## 一、十三场景对照表

| # | 场景 | 覆盖任务 | 验收证据位置 |
|---|------|---------|------------|
| 1 | 写作需求解析（类型/主题/读者/时间/篇幅/重点） | 604 | pytest（writing_extract 校验）+ cargo 测试 |
| 2 | 素材智能收集（检索+来源+关联显示） | 605 | cargo 测试（writing_collect_materials） |
| 3 | 素材审核边界（未确认不进可信集合） | 605 | cargo 断言 + verify_security_phase4 |
| 4 | 大纲生成（各章目标/重点/素材映射） | 606 | cargo 测试 + pytest |
| 5 | 分章节写作（独立保存/确认/全文组合） | 607 | cargo 测试 + verify_writing_editor_611 |
| 6 | 事实核验（正确/错误/无来源分类定位） | 609 | cargo 测试 + verify_fact_check_609 |
| 7 | 数据口径检查（差异提示不合并） | 609 | cargo 测试 |
| 8 | 修改和版本管理（差异可查/历史可恢复） | 602/608 | cargo 测试 + verify_writing_review_612 |
| 9 | 文稿导出（DOCX/PDF 正文标题表格中文完整） | 613 | verify_writing_export_613（真实产出结构断言） |
| 10 | 成果归档（关联任务/素材/知识/核验结果） | 614 | cargo 测试 + verify_writing_archive_614 |
| 11 | 知识回流（仅用户确认内容进治理） | 614 | cargo 断言 |
| 12 | 历史兼容（前三阶段数据完整） | 601（work_cleanup 排除有测试） | cargo 测试 |
| 13 | 敏感信息保护（外发限制/导出日志检查） | 613/615 | verify_security_phase4 |

## 二、验收命令总表

- cargo test --offline --lib（各任务新增测试全绿）
- cargo test --offline --tests（615 全量）
- pytest tests -q（604/606/607 及全量）
- npm --prefix desktop run build + npx tsc --noEmit（UI 任务）
- 任务专项 verify：verify_writing_store_601 / verify_fact_check_609 / verify_writing_ui_610 / verify_writing_editor_611 / verify_writing_review_612 / verify_writing_export_613 / verify_writing_archive_614 / verify_security_phase4

## 三、安全验收口径（verify_security_phase4.py 必查项）

1. 写作流新增 IPC 全部过 gate_user_text（落盘用户文本）。
2. AI 无定稿/归档/导出写通道（三动作仅用户命令触发；引擎输出只进候选暂存）。
3. 大纲未确认不得生成正文（writing_section_generate 校验断言）。
4. 未确认素材不进事实集合（默认已确认过滤断言，场景三）。
5. 导出前敏感词扫描生效；导出为本地处理无网络调用（场景十三）。
6. 归档仅 MD（DOCX/PDF/HTML 不入 vault，断言）。
7. 无敏感原文进日志（合成样本检查）。
8. 测试数据全虚构声明；禁入材料零出现。
9. 无新增豁免声明。
10. runtime/ 零改动断言。

## 四、证据等级与未验证事项

证据分级照 IMPLEMENTED_RULES §3。以下如实登记为未验证，不夸大：真实模型需求解析/大纲/章节写作质量；真实窗口目视（610/611/612/613 四处 UI）；长文稿多章节性能；DOCX 真实格式兼容性（textutil 简化版与 office-docx 公文版在真实 Word 打开效果）；office-docx 技能在真实模型会话中的实际调用。

