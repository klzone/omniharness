# OD-WR-506 复盘档案页 + 记录反链：验收证据

日期：2026-11-11
基线：eec28fd635（OD-WR-500 规划）；501–505 的未提交改动全部保留，未重置、未提交、未推送。

## UI 参考借鉴

参考 AlphaSense Web 的左列文档列表 + 右侧详情分栏、Apple Books 的封面网格 + 筛选 chips、Ghost Web 的内容列表：档案页采用左列表右详情，列表行只保留周期类型、起止范围与状态三要素。筛选 chips 置顶不折叠，样式沿用现有 Neo-Skeuomorphism 2.0 token。

## 实跑命令与结果

| 命令 | 结果 | 输出 |
| --- | --- | --- |
| cd desktop && npx tsc --noEmit | exit 0 | tsc.txt |
| cd desktop && npm --prefix . run build | exit 0 | build.txt |
| export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib | 1154 passed；3 failed（均为已知 process_owner 沙箱误报） | cargo-test.txt |
| python3 tests/verification/desktop/verify_wr_506.py | 58/58 通过 | verify-506.txt、verify-506.json |
| python3 tests/verification/desktop/verify_wr_505.py | 71/71 通过 | verify-505.txt |
| python3 tests/verification/desktop/verify_wr_504.py | 41/41 通过 | verify-504.txt |
| python3 tests/verification/desktop/verify_wr_503.py | 58/58 通过 | verify-503.json |
| python3 tests/verification/desktop/verify_work_record_ui_301.py | 15/15 通过 | verify-301.txt |

## cargo 已知误报

process_owner 的 3 项失败（live_instance_owned_by_another_host_is_refused、pid_reuse_with_unrelated_cmdline_is_never_killed、record_from_other_boot_is_removed_without_killing）是沙箱内 PID/进程所有权测试的已知误报；ARCHITECT 在沙箱外复跑为全绿。本次未修改该模块。review_store 模块 22 项测试全部通过（原 16 项 + 新增 6 项）。

## 边界与限制

* 反向引用为只读扫描：复盘 record_ids 与写作任务 selected_material_ids → 素材 source_type=work_record 的回指；记录文件不写入反向指针。
* 写作任务反链只在素材仍存在且来源确为该记录时出现；素材被删除后反链自然消失。
* 档案页没有新增确认/删除动作，复盘写路径仍由 505 的生成/确认/备份语义负责。
* “重新生成”走既有 collect → composeConversationPrompt → host.submitTask → generate 链路；已确认复盘按钮禁用并在提交前再次拒绝。
