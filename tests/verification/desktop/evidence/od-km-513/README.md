# OD-KM-513 全量回归 + 安全核对 + 阶段收口 — 本轮证据

证据等级：脚本运行（cargo/pytest/build/tsc/verify）+ 静态源码检查（安全脚本）+ 既有测试覆盖断言。
未开真实窗口、未调用模型、未读取用户真实资料。测试数据全部虚构或合成。

## 基线核对

- 工作目录：`/Users/yvan/Code/MyWorkStation`
- HEAD：`836c230fb2e7c4a2c6057b1922948f68add0e7f7`（未提交、未推送）
- 501–512 的改动与证据保持工作区原样；513 只新增 `tests/verification/desktop/verify_security_phase3.py` 与本证据目录。
- 未改 `STATUS.md` / `REVIEW.md` / `PLANS.md` / `docs/decisions.md`，未改业务代码。
  收口时工作区快照见 `git-status.txt`（其中包含 513 新增项；501–512 项与任务开始时一致）。

## 全量命令结果

| 命令 | 结果 | 证据 |
| --- | --- | --- |
| `export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && cargo test --offline --tests` | exit 0；`950 passed / 0 failed`（883 lib + 67 集成，11 个测试二进制） | `cargo-tests.txt` |
| `/tmp/omnidesk-pytest-venv/bin/pytest tests -q` | exit 0；`200 passed, 30 subtests passed` | `pytest.txt` |
| `cd desktop && npm --prefix . run build` | exit 0；仅既有 >500 kB chunk 提示 | `npm-build.txt` |
| `cd desktop && npx tsc --noEmit` | exit 0；无输出（另以 `--extendedDiagnostics` 核对 TypeScript 7.0.2 真实类型检查，Check time 0.147s） | `tsc-noemit.txt` |

本轮首次运行即 0 failed，macOS 偶发单测失败未出现，因此没有使用“单测复跑即过”口径。

## 阶段专项 verify

| 命令 | 结果 | 证据 |
| --- | --- | --- |
| `python3 tests/verification/desktop/verify_material_store_501.py` | exit 0；19/19 | `verify_material_store_501.txt` / `.exit` |
| `python3 tests/verification/desktop/verify_material_ui_510.py` | exit 0；23/23 | `verify_material_ui_510.txt` / `.exit` |
| `python3 tests/verification/desktop/verify_linkage_ui_511.py` | exit 0；25/25 | `verify_linkage_ui_511.txt` / `.exit` |
| `python3 tests/verification/desktop/verify_project_ui_512.py` | exit 0；26/26 | `verify_project_ui_512.txt` / `.exit` |
| `python3 tests/verification/desktop/verify_security_phase3.py` | exit 0；25/25 | `verify-security-phase3.txt` / `.exit` / `.json` |

## verify_security_phase3 八项对照

| 验收口径 | 检查项 | 结果 |
| --- | --- | --- |
| 1 素材流新 IPC 闸门 | `gate_definition`、`gate_covers_material_create`、`gate_covers_material_edit`、`gate_covers_material_review_confirm`、`gate_covers_knowledge_link`、`gate_covers_project_text` | PASS；标题/正文/来源引用/定位/标签/编辑理由/审核理由/项目名称与目标均过 `gate_user_text` |
| 2 AI 无审核状态写通道 | `material_review_only_user_command`、`ai_output_only_stages_candidates`、`candidate_defaults_pending` | PASS；`review_material` 生产代码仅 1 处用户命令入口；模型输出只进 `.candidates/`，默认待审核 |
| 3 知识库写路径零改动 | `knowledge_write_paths_untouched`、`knowledge_link_is_read_only`、`material_query_read_only` | PASS；无 `vault_write`、知识入库落盘或知识笔记写函数调用 |
| 4 素材篮清理排除 | `cleanup_excludes_material_dir`、`cleanup_material_test_exists`、`cleanup_scope_does_not_scan_link_dir` | PASS；显式排除 + 预览/执行双重路径检查 + 既有字节不变测试 |
| 5 索引重建不损 work/ | `rebuild_index_separated_from_work`、`vault_root_outside_app_root`、`rebuild_links_recoverable_test` | PASS；重建只读知识库根，应用数据根内的知识库根被拒绝 |
| 6 无敏感原文进日志 | `gate_error_does_not_echo_text`、`no_sensitive_logs` | PASS；拒绝信息只报标签与次数；新模块/界面/引擎无调试日志输出 |
| 7 测试数据全虚构 | `tests_use_temp_roots`、`test_data_fictional_declared`、`no_forbidden_material_terms` | PASS；测试只落临时目录，夹具标「虚构/合成」，禁入材料字样零出现 |
| 8 无新增豁免 | `no_new_waivers` | PASS；本阶段 diff 新增行与新源码均无 lint/测试跳过标记 |

另含 IPC 注册与提取命令边界检查：`phase3_ipc_registered`（15 个新命令）、`gate_error_does_not_echo_text`。总计 25 项，25 通过，0 失败。

## 十二场景对照

| # | 场景 | 覆盖任务 | 本轮证据位置 | 结论 |
| --- | --- | --- | --- | --- |
| 1 | 工作记录关联知识（推荐+理由+原文定位） | 504/505/507/511 | `verify_linkage_ui_511.py` 25/25；cargo `knowledge_link_ipc_tests` / `knowledge_link_store` 测试 | 覆盖（静态+脚本）；推荐相关性未验证 |
| 2 | 任务关联知识（稳定 ID） | 503/507 | cargo `task_store` / `knowledge_link_store` 测试（`related_knowledge` 稳定 ID） | 覆盖（脚本） |
| 3 | 自动提取写作素材 | 504 | pytest 200+30；cargo `task_extract` 提示词约束与宿主校验测试 | 覆盖（脚本）；真实模型提取质量未验证 |
| 4 | 重复素材识别 | 501/504 | cargo 指纹唯一性、同指纹拒绝、候选重复跳过测试 | 覆盖（脚本） |
| 5 | 素材来源追溯 | 501/506/510 | `verify_material_ui_510.py` 23/23；cargo `material_get` 来源/关联 payload 测试 | 覆盖（静态+脚本）；真实窗口未验证 |
| 6 | 素材人工审核 | 501/506/510/513 | cargo 五态流转、理由必填、否定态过滤测试；`verify_security_phase3.py` | 覆盖（脚本） |
| 7 | 项目成果归集 | 502/508/512 | `verify_project_ui_512.py` 26/26；cargo `project_overview` 测试 | 覆盖（静态+脚本）；真实窗口未验证 |
| 8 | 知识更新影响 | 507/511 | cargo 反向查询、候选暂存、确认写回、移除留痕测试；511 verify | 覆盖（脚本） |
| 9 | 历史数据兼容 | 503 | cargo 字节不变 + `serde(default)` 兼容测试 | 覆盖（脚本） |
| 10 | 敏感信息保护 | 504/506/513 | cargo 素材闸门测试；`verify_security_phase3.py` 合成样本静态检查 | 覆盖（静态+脚本）；未走真实模型外发 |
| 11 | 知识库索引重建 | 513 | `verify_security_phase3.py`；cargo `rebuild_after_add_modify_delete_matches_from_scratch` / `external_edits_are_visible_after_rebuild` | 覆盖（静态+单元测试）；未在用户真实知识库执行 |
| 12 | 第四阶段接口 | 509 | cargo `material_query` payload 结构、默认已确认过滤测试 | 覆盖（脚本） |

## 未验证事项（如实登记）

- 真实模型素材提取质量、事实/成果/计划区分质量：未调用模型。
- 知识推荐相关性：未调用模型，也未做人工相关性评审。
- 素材篮、关联视图、项目视图三张 UI 卡的真实窗口目视：只做静态 UI verify 与构建，未截图。
- 大量素材下的反向查询、项目概览全量扫描性能：未构造数百条素材做基准；代码当前按全量列表过滤/派生。
- 真实用户知识库上的索引重建：只跑单元测试，未触碰用户真实库。

## 遗留登记（不在本卡修复）

1. `verify_ui_simplify_v1.py` 仍读取已不存在的 `desktop/src/shell/WeeklyQuadrants.tsx`，运行于第 51 行抛 `FileNotFoundError`，exit 1。属 OD-QS-410 遗留，不是 513 引入；本卡不修码，证据见 `legacy-verify-ui-simplify-v1.txt` / `.exit`。
2. `work/知识关联/` 清理排除：`work_cleanup` 预览只枚举 `worklog_records` 与 `task_store`，执行只移动这两类条目，不做目录级遍历；`CLEANUP_EXCLUDED_DIRS` 仅含素材篮。当前扫描范围不会触碰 `知识关联/`，因此不需要新增显式排除测试。若未来清理改为遍历 work/ 子目录，须补显式排除与测试。

## 裁决记录

- Ruling: `cargo test --offline --tests` 首次 0 failed —— 无需按 macOS 偶发失败口径复跑 —— 错了代价：漏掉一次性偶发；本轮保留完整输出，后续可复跑。
- Ruling: 安全脚本采用静态源码 + 既有测试断言，合成敏感样本只作为静态样例 —— 避免启动应用/调用模型/读用户数据 —— 错了代价：未来若新增日志 sink，本脚本需同步扩展。
- Ruling: `work/知识关联/` 不新增清理排除测试 —— 现有清理不遍历该目录 —— 错了代价：清理范围若扩大，会漏掉显式回归测试。
- Ruling: `npx tsc --noEmit` 极快仍判定为真实检查 —— 已用 `--extendedDiagnostics` 确认 TypeScript 7.0.2 执行类型检查（Check time 0.147s）—— 错了代价：无，命令 exit 0 且诊断输出非空。

## 豁免声明

- 未新增 lint/测试豁免，未跳过或删除既有测试，未改低验收阈值。
- 未改业务代码，未 commit，未 push，未打标签。
- 测试数据全部虚构或合成；未读取、打印或提交凭证；未使用真实业务材料。

