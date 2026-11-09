# OD-WT-604 写作需求解析 — 执行证据

日期：2026-11-09（Asia/Shanghai）
基线：HEAD ab000dfb01；工作区既有 601/602/603 未提交改动保留；605 并行改动未触碰。

## 交付文件

- engine/omnidesk_engine/writing_extract.py（提示词 + validate_writing_payload，只读 stdin 只写 stdout）
- desktop/src-tauri/src/writing_parse.rs（宿主校验 + 提示词调用 + 草稿转写作任务）
- desktop/src-tauri/src/lib.rs（新增 writing_parse_requirement / writing_parse_requirement_apply / writing_create_from_requirement）
- desktop/src/types.ts、desktop/src/api.ts（写作任务与需求草稿类型、三条封装）
- tests/verification/engine-interface/adapter/test_writing_extract_unit.py（16 项）

## 实跑命令与结果

1. /tmp/omnidesk-pytest-venv/bin/pytest tests -q
   - 结果：216 passed, 30 subtests passed（新增本卡 16 项全绿）
2. export PATH="$HOME/.cargo/bin:$PATH" && cargo test --offline --lib
   - 结果：961 passed / 3 failed（engine_bridge 与 engine_ipc 的合成子进程 PermissionDenied，既有并发竞态）
3. RUST_TEST_THREADS=4 cargo test --offline --lib
   - 结果：964 passed / 0 failed（新增 writing_parse 10 项全绿）
4. cd desktop && npx tsc --noEmit
   - 结果：退出码 0，无输出

## 裁决记录

- Ruling: apply 只校验并返回草稿（WritingRequirementApplied），不落盘；新增 writing_create_from_requirement 供用户确认后创建任务 — 理由：任务卡允许「候选不落盘直接返回结构由界面确认后再 create」，与 610 的「解析→问题清单补全→创建任务」流程一致，也避免 AI 用猜测字段直接建任务 — 错了代价：610 需显式调用创建命令（本来就是计划内步骤）。
- Ruling: questions 字段必须出现在输出中；仅在类型无法确定或任一关键条件缺失时要求非空 — 理由：任务卡要求缺失关键条件必须列入 questions、类型无法确定时必含类型询问；信息齐全时允许空数组更符合「不臆测」 — 错了代价：完整需求可无问题清单，界面直接进入确认。
- Ruling: 解析准备返回 WritingParsePrompt（AnalyzePrompt 的 prompt/user_input_text/masked 形状，去掉无意义的 record_id） — 理由：本流程没有工作记录实体 — 错了代价：前端无法复用 record_id 字段，但该字段对写作解析无用途。
- Ruling: 创建路径对标题/主题/目的/受众/周期/篇幅/风格与要点再过一次 gate_user_text — 理由：用户补全后的内容仍可能写入 work/，不能因来自 AI 草稿而绕过红线扫描 — 错了代价：多一次启发式扫描开销。

## 未做与已知限制

- 未调用真实模型验证写作质量；本卡只覆盖提示词组装、形状校验与草稿创建路径。
- 默认并行 cargo test 的 3 项合成子进程 PermissionDenied 属既有跨模块竞态，非本卡引入；按任务卡允许的 RUST_TEST_THREADS=4 复跑留痕。
- 610 界面尚未接线，前端封装未在真实窗口点击验证。

## 豁免声明

未新增 lint/测试豁免、未跳过或删除既有测试、未改低阈值；未改 runtime/；未新增模型调用通道；测试数据全虚构；未 commit/push；未改 STATUS.md/REVIEW.md/PLANS.md/decisions.md。
