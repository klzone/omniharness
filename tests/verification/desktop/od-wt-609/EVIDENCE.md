# OD-WT-609 事实核验引擎证据

- 日期：2026-11-09
- 基线 HEAD：ab000dfb01（工作区保留 601–608 既有未提交/未跟踪改动，本次不改其交付）
- 证据等级：文件事实（源码）、静态脚本检查、Rust 测试、TypeScript 类型检查

## 实跑命令与结果

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib | 1045 passed, 0 failed, exit 0 | cargo-lib.log |
| python3 tests/verification/desktop/verify_fact_check_609.py | 31/31 通过, exit 0 | verify.log / verify.json |
| cd desktop && npx tsc --noEmit | exit 0（无输出） | tsc.log |

本轮新增 26 项 Rust 测试（21 项 fact_check 纯函数 + 5 项 IPC 闸门/落盘），
覆盖场景六、场景七、口径冲突三原因、注释合并、过期知识、重跑去重与
单条处理优先级。

## 改动文件

- desktop/src-tauri/src/fact_check.rs（新增：确定性提取/匹配/八类结论/冲突提示/统计）
- desktop/src-tauri/src/lib.rs（仅事实核验命令区：模块声明、两条 IPC、实现与测试）
- desktop/src/types.ts（核验记录/结论/冲突/运行与处理回执类型）
- desktop/src/api.ts（writingFactcheckRun / writingFactcheckResolve 两条封装）
- tests/verification/desktop/verify_fact_check_609.py（新增静态验收脚本）
- tests/verification/desktop/od-wt-609/（本证据目录）

未修改 601/602/603/606/607 交付文件；603 只被调用（append_fact_check /
update_resolution / list_fact_checks / list_citations）。

## 裁决记录

- Ruling: 607 的 SectionDraft.segments 只在生成/确认路径校验，未落盘；609 用
  FactCheckRunRequest.annotations 接收界面侧最近稿件的四分法标注，再按正文
  定位合并。理由：不改 607 交付、不新增 sidecar 格式；错代价：界面未随车
  传入标注时，本次只产出确定性 1/2/3/4/5 类结论。
- Ruling: 知识依据来源为三路：请求显式 evidence、citations.jsonl 已记录知识
  引用、已配置知识库时按 selected_knowledge_ids 只读读取；过期标记联动
  knowledge_quality 的确定性报告。理由：不改知识库写路径，也不把外部搜索当
  正式事实；错代价：知识库未配置或笔记读不到时，对应候选可能落「无法找到来源」。
- Ruling: 核验记录编号由章节/段落/对象/取值/片段/结论哈希稳定生成，重跑跳过
  已存在编号。理由：追加式记录不能因重跑重复堆积，且不能重置用户处理状态；
  错代价：结论变化会生成新编号并追加新记录，历史保留。
- Ruling: 同一指标在依据中出现不同数值时，冲突结论优先于直接来源。理由：
  场景六要求错误数据不得判成已核实；错代价：来源并列时用户需先处理冲突。
- Ruling: 分析/判断/建议标注命中候选时，保留候选原对象（数字仍是数字），只改
  结论；纯标注句才映射到工作事项/案例事实/工作成果。理由：面板筛选仍能按
  原对象定位；错代价：对象与结论不是一一对应，界面需同时展示两者。
- Ruling: 核验处理备注过红线与掩码闸门，但 603 记录没有备注字段，不落盘。
  理由：不扩 603 契约；错代价：备注只在本次回执可见。
- Ruling: 四位年份 1900–2100 的裸数字不按数字对象提取，只按时间对象；概括句
  用固定概括词触发，避免把普通叙述全量写成表达性概括。理由：降低噪声。

## 未做与已知限制

- 机构名与人员职务是后缀/姓氏启发式，不是 NER；复杂简称、同名异写与职务简称
  可能漏提或误提。
- 口径冲突只对数字对象做同指标比较；机构/日期的跨来源取值冲突未单独建对。
- 607 标注不落盘，未接入界面的 drafting 状态时，分析/判断/计划结论不会自动出现。
- 知识库未配置时不会自动展开知识正文；请求显式 evidence 或已记录引用仍可用。
- 未做真实模型生成稿、真实长文稿、真实知识库目录的目视或性能验证。
- 核验面板、定位跳转与批量处理留待 OD-WT-612。

## 豁免声明

- 未新增 lint/测试豁免，未跳过或删除既有测试，未降低验收阈值。
- 未修改 runtime/，未新增模型通道，未自动删除或编造来源。
- 未提交、推送、打标签；未改动 STATUS.md、REVIEW.md、PLANS.md、docs/decisions.md。
