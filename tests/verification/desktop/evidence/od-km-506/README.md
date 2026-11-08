# OD-KM-506 素材 IPC 六命令：执行证据

- 基线：HEAD `836c230fb2e7c4a2c6057b1922948f68add0e7f7`（main），工作区含 501–505/508 未提交交付与 507 并行改动；本卡未提交、未推送。
- 测试数据全部为虚构字符串；未读取真实工作记录或凭证。
- `cargo-lib-test.txt`：`export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && cargo test --offline --lib` → **876 passed / 0 failed**，退出码 0；其中 `material_ipc_tests` 7 项全绿。
- `pytest-task-extract-unit.txt`：`/tmp/omnidesk-pytest-venv/bin/pytest tests/verification/engine-interface/adapter/test_task_extract_unit.py -q` → **21 passed**，退出码 0（含既有素材清单注入 3 项）。
- `tsc-noemit.txt`：`cd desktop && npx tsc --noEmit` → 无输出，退出码 0。
- `npm-build.txt`：`cd desktop && npm --prefix . run build` → 构建成功（仅既有 >500 kB chunk 提示），退出码 0。
- 附加回归：`python3 tests/verification/desktop/verify_material_store_501.py` → **19/19 通过**（501 既有验收未被本卡破坏）。

## 本卡新增覆盖

- 创建闸门与 user 来源引用必填；不指定来源时按 user 收藏落盘。
- 列表按类型/状态/日期/项目/任务过滤，待审核置顶。
- 审核流转（确认/退回/拒绝/暂缓/归档入口）、退回与拒绝理由必填、审核记录追加。
- 审核理由与编辑正文的红线闸门：拒绝时不改写文件字节。
- 编辑正文/类型/标签并追加编辑记录。
- 来源追溯 payload：源记录定位、引用原文命中与上下文摘录、关联任务/项目/知识条目定位。
- 缺失来源与 user 来源如实报告，不伪造定位。

## 未验证

- 真实模型素材提取质量、真实知识库路径下的知识条目定位、真实界面操作未验证（本卡为 IPC 层）。
