# OD-GROW-301 G0 派工：前置核实（只读，BUILDER 执行）

## 任务编号与名称
OD-GROW-301 / G0：每日习惯整理——前置核实（只读，不写功能代码）

## 获批依据
- 计划位置：PLANS.md 顶部「OD-GROW-301」小节（G0 段）
- 批准时间与批准人：2026-11-23 / 用户
- 允许修改的文件或目录：仅 STATUS.md（追加执行记录）与新增 docs/od-g0-design-notes.md；不写任何功能代码
- 明确不做的事：不改任何 .rs/.ts/.py 源码；不改配置；不调模型

## 开工前检查（只读）
- git status 与 HEAD 快照；保留全部既有未提交 / 未跟踪 / 暂存改动
- 只读完成本步骤必需的代码与文档

## 实施要求（五项逐条核实，写进 docs/od-g0-design-notes.md）
1. 写作模块：AI 原稿与用户定稿版本链、保存位置（document_store 版本 + writing_section 确认快照 + WritingWorkbench 版本页签），能否按章节对比；结论标注源码文件与行号。
2. 线程：消息字段清单（已知 id/role/text，无时间戳——确认）；plain 线程识别方式；thread_store::load() 读取全部线程的耗时特征，线程很多时是否需要分页（给出建议：分页/上限/沿用）。
3. 无人值守提交：WorkRecordLibrary 的 workRecordAnalyze 链路（prepare → submitTask → 轮询 taskStatus → taskResult → apply）作为先例；确认纯文本无工具提交是否可行；结果在无聊天组件时如何取回；任务超时与取消行为；为 G4 的「专用整理提交命令」给出可行挂点建议（新命令放 commands/）。
4. 首页提醒：AnnouncementBar 渲染 null 且只挂 ScenePage；确认 HomeCanvas 顶部加条件渲染提醒条为最小方案，说明挂载与关闭状态存放（ui-state）。
5. 周期复盘草稿：确认目前无应用内编辑（ReviewArchive 只有生成与确认）；H1/H2 不依赖编辑能力。
产出末尾附「对 G2/G4 设计的影响」三句以内小结。完成后在 STATUS.md 追加执行记录（命令与结论、证据等级），然后停下，等 ARCHITECT 与用户确认后再开工后续任务。

## UI 参考
不涉及界面改动，不适用。

## 验证
- 只读核实，无测试命令；STATUS.md 记录实际执行的检索/查看命令与结论
- 证据：docs/od-g0-design-notes.md + STATUS.md 记录

## 硬约束
- 不修改 runtime/ 内 DSH Runtime；不动模型路由与凭证
- 不读取真实业务材料；测试与示例只用合成数据
- 不提交、不推送、不打标签
- 只写 STATUS.md 与 docs/od-g0-design-notes.md 两个文件

## 交付
- docs/od-g0-design-notes.md（五项结论 + 设计影响小结，每项标证据等级）
- STATUS.md 追加 G0 执行记录
- 回报格式：改动文件清单 + 五项结论摘要
