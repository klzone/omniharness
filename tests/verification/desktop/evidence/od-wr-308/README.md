# OD-WR-308 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 范围说明：未修改 STATUS.md / REVIEW.md / PLANS.md；未提交、未推送。

## 全量回归

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --tests | 0 | 13 个测试目标全 ok；lib 643 passed / 0 failed |
| pytest tests -q | 0 | 186 passed，30 subtests passed |
| npm --prefix desktop run build | 0 | vite 构建成功 |
| npx tsc --noEmit | 0 | 无输出 |
| verify_work_record_ui_301.py | 0 | 15/15 |
| verify_task_manage_ui_302.py | 0 | 15/15 |
| verify_task_review_ui_303.py | 0 | 16/16 |
| verify_task_linkage_304.py | 0 | 16/16 |
| verify_wr_security_308.py | 0 | 13/13（新增安全核对） |

原始输出：cargo-test-tests.log、pytest.log、npm-build.log、tsc.log、各 verify_*.log、exit-*.txt。

## 安全断言汇总

- 红线与掩码：记录标题/正文、任务标题、进展、状态备注、确认/拒绝备注、关联进展都过 gate_user_text；引擎提交唯一汇合点仍过红线与 mask_privacy。
- DataMasker：引擎 Task 4 主链已接；真实词典覆盖与质量未验证。
- AI 无写通道：task_extract.py 只读 stdin、只写 stdout；候选只由宿主校验后暂存；AI 不直接改状态或删记录。
- 端点默认关闭：local / private / public 三类默认关闭，旧设置缺字段按关闭；提交前有红线与准入两道门。
- 错误不回显原文：红线拒绝只报类别与数量；掩码回执只给计数。
- 测试数据：新模块测试用合成文本与 /tmp，不引用用户成果或正式制度材料。

## 四份规划文档

落盘在 docs/plans/OD-WR-308/：

1. 01-产品方案.md
2. 02-技术架构.md
3. 03-数据与安全.md
4. 04-迁移归档与清理.md

来源说明：外部方案的 §十二 原文不在仓库；四份文档按已批准计划、已实施代码与证据整理，若与方案原文不一致，以原文为准并由规划会话校正。

## 未验证事项

- 真实模型提取质量与误报/漏报。
- DataMasker 词典覆盖与回程还原质量。
- 真实 Tauri 窗口观感与交互。
- 跨文件写入失败的中间态恢复（只有单文件原子性）。
- 大量记录/任务下的列表性能。
