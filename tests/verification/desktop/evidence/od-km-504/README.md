# OD-KM-504 AI 素材提取搭车：执行证据

基线：HEAD 836c230fb2e7c4a2c6057b1922948f68add0e7f7（2026-11-09 开工前记录）。
工作区已有的 501/502/503/505 未提交改动全部保留，本任务未重置、清理或提交任何既有改动。

## 改动范围

- engine/omnidesk_engine/task_extract.py：提示词增 material_candidates 输出规范与事实约束；新增纯形状校验 validate_material_payload。
- desktop/src-tauri/src/task_extract.rs：增素材候选宿主校验、素材篮/.candidates/ 暂存、指纹去重与既有素材关联、同一次分析调用内任务/素材候选双落盘。
- tests/verification/engine-interface/adapter/test_task_extract_unit.py：素材提示词与形状校验测试。
- 本证据目录。

未改 runtime/、knowledge_vault 写路径、lib.rs、material_store.rs、project_store.rs、task_store.rs、worklog_records.rs、STATUS.md、REVIEW.md、PLANS.md、docs/decisions.md。任务候选结构未扩展。

## 实跑命令与结果

- /tmp/omnidesk-pytest-venv/bin/pytest tests -q → 197 passed, 30 subtests passed（输出：pytest-tests.txt）。
- export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && cargo test --offline --lib → 856 passed; 0 failed（输出：cargo-lib-test.txt）。
- 专项过滤 cargo test --offline --lib task_extract::tests → 41 passed; 0 failed（净增 16 项）。

## 裁决

- 素材候选暂存目录选 work/素材篮/.candidates/：素材篮已由 work_cleanup 整体排除，正式素材读取只认顶层 .md，不会误读候选 JSON。
- 指纹直接复用 material_store::material_fingerprint(record_id, source_quote)；重复时跳过创建并在 AnalyzeApplied.material_duplicates 回报既有素材或候选编号。
- content 与 source_quote 的复制检查在折叠空白和去掉全部空白两级比较，避免仅靠加空格绕过。

## 已知限制

- 现有 worklog_record_analyze（lib.rs，本任务允许清单外）未注入已有素材清单，因此提示词要求模型在无清单时把 match_material_id / match_reason 留 null；宿主仍完整支持已有素材编号校验，供后续 506 接入真实清单。
- Python validate_material_payload 是工具侧形状校验；宿主 validate_material_payload 才是权威校验。
- 真实模型提取质量未在本任务验证（无新增模型调用，未产生真实模型输出）。
