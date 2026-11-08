# OD-WR-314 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 范围说明：按用户本轮任务卡约束，未修改 STATUS.md / REVIEW.md / PLANS.md。

## 改动文件

- desktop/src-tauri/src/process_owner.rs：仅 #[cfg(test)] 测试夹具。生产代码零改动。

## 根因与修复

旧夹具用 sh -c "sleep 30" omnidesk_engine 冒充引擎。第一轮改成 python3 后仍复现：macOS 的 /usr/bin/python3 与 framework bin/pythonX 都是 launcher，会在启动后 exec 成 Python.app，argv 在「写 owner.json」与「reclaim 身份核验」之间改变，identity_ok 变假，错误落到「无法核验身份」分支。

最终夹具先解析 python 的 realpath，再在存在时直接指向 Python.app/Contents/MacOS/Python。该进程从 exec 起 argv 稳定，不再出现中途替换。

## 实跑命令与结果

连续两轮：

| 轮次 | 命令 | 退出码 | 结果 |
| --- | --- | --- | --- |
| 1 | cargo test --offline --tests | 0 | 13 个测试目标全部 ok；lib 614 passed / 0 failed；process_owner 夹具 ok |
| 2 | cargo test --offline --tests | 0 | 13 个测试目标全部 ok；lib 614 passed / 0 failed；process_owner 夹具 ok |

原始输出：cargo-test-tests-round1.log、cargo-test-tests-round2.log、exit-round1.txt、exit-round2.txt。
