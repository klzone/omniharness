# 第三阶段验收标准（OD-KM-500 规划成果四）

> 状态：已批准（2026-11-09）。任务卡验收命令见 PHASE3_IMPLEMENTATION_PLAN.md；本文件登记方案 §十 十二场景对照与安全验收口径。

## 一、十二场景对照表

| # | 场景 | 覆盖任务 | 验收证据位置 |
|---|------|---------|------------|
| 1 | 工作记录关联知识（AI 推荐+理由+原文定位） | 504/505/507/511 | verify_linkage_ui_511.py + cargo 测试 |
| 2 | 任务关联知识（稳定 ID 查询，不依赖临时文本匹配） | 503/507 | cargo 测试（related_knowledge 稳定 id 断言） |
| 3 | 自动提取写作素材（事实/成果/计划三类区分，不把计划当成果） | 504 | pytest（提示词约束+宿主校验）+ cargo 测试 |
| 4 | 重复素材识别（进展关联既有素材，保留历史） | 501/504 | cargo 测试（指纹唯一性+关联追加断言） |
| 5 | 素材来源追溯（定位原始记录/任务/知识+引用内容） | 501/506/510 | verify_material_ui_510.py + material_get payload 测试 |
| 6 | 素材人工审核（批准/退回/拒绝/暂缓；未批准不进可信集合） | 501/506/510 | cargo 测试（五态流转+可信集合过滤）+ verify_security_phase3 |
| 7 | 项目成果归集（按项目检索工作事实和成果） | 502/508/512 | verify_project_ui_512.py + project_overview 测试 |
| 8 | 知识更新影响（识别受影响关联素材并提示复核，不擅改已确认素材） | 507/511 | cargo 测试（反向查询+提示不覆盖断言） |
| 9 | 历史数据兼容（一、二阶段数据升级后完整） | 503 | cargo 测试（字节不变+serde default） |
| 10 | 敏感信息保护（脱敏生效，敏感内容不外发不进日志） | 504/506/513 | verify_security_phase3.py（合成敏感样本） |
| 11 | 知识库索引重建（原始数据不丢、关联可恢复） | 513 | verify_security_phase3.py（重建后关联恢复断言） |
| 12 | 第四阶段接口（返回正文/来源/引用位置/审核状态/关联实体） | 509 | cargo 测试（material_query payload 结构断言） |

## 二、验收命令总表（每任务卡必须全部通过）

- cargo test --offline --lib（各任务新增测试全绿）
- cargo test --offline --tests（513 全量）
- pytest tests -q（504 及全量回归）
- npm --prefix desktop run build（UI 任务）
- npx tsc --noEmit（IPC/UI 任务）
- 任务专项 verify：verify_material_store_501.py / verify_material_ui_510.py / verify_linkage_ui_511.py / verify_project_ui_512.py / verify_security_phase3.py

## 三、安全验收口径（verify_security_phase3.py 必查项）

1. 素材流新增 IPC 全部过 gate_user_text（红线扫描+脱敏掩码）。
2. AI 无审核状态写通道（material_review 仅用户触发；引擎输出只进候选暂存）。
3. 知识库写路径零改动（素材流不触碰 vault_write/knowledge_ingest 写函数）。
4. 素材篮目录被 work_cleanup 显式排除（预览与执行均不触碰）。
5. 索引重建不损 work/ 数据（场景十一）。
6. 无敏感原文进日志（合成敏感样本走完素材流后检查日志输出）。
7. 测试数据全虚构声明；禁入材料（案件/线索/人物画像）零出现。
8. 未新增 lint/测试豁免声明。

## 四、证据等级与未验证事项登记

证据分级照 IMPLEMENTED_RULES §3（文件事实/静态检查/脚本运行/真实模型输出/未验证）。以下事项验收时如实登记为未验证，不夸大：真实模型素材提取与知识推荐质量；素材篮/关联视图/项目视图真实窗口目视；数百条素材下反向查询性能。

