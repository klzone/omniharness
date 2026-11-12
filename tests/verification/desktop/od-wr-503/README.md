# OD-WR-503 验收证据

- 任务：当日摘要（intent / Skill / 宿主汇集校验落盘 / IPC / 前端入口）。
- 基线：HEAD eec28fd635（OD-WR-500 规划提交），工作区既有 501/502 改动保留。

## 布局参照（OD-ARCH-035，动工前检索）

参照 Raycast 的通知型列表（紧凑列表 + 右侧状态）、Apple Health 的摘要卡（大标题、
轻边框、状态词）与 Claude 的对话侧栏（内容优先、少说明文字）。摘要卡沿用现有白卡
圆角与轻阴影，只保留标题、日期、状态、引用 chips；正文展开后直接阅读，引用记录
点开即定位，不增加第二套视觉语言。

## 命令与结果

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| `/tmp/omnidesk-pytest-venv/bin/pytest tests -q` | 234 passed | pytest.log |
| `RUST_TEST_THREADS=4 cargo test --offline --lib` | 1135 passed / 0 failed（新增 12 项） | cargo-lib.log |
| `npx tsc --noEmit` | exit 0 | tsc.log |
| `npm --prefix . run build` | exit 0 | build.log |
| `python3 tests/verification/desktop/verify_wr_503.py` | 58/58 | verify.log / verify_wr_503.json |
| `python3 tests/verification/desktop/verify_worklog_089.py` | 83/83（旧随手记词表回归） | verify_worklog_089.log |

## 备注

- 测试全部使用虚构记录标题与正文，不包含真实业务材料。
- `/tmp/omnidesk-pytest-venv` 在本会话首次运行前不存在，已创建并安装 pytest；
  这是临时测试环境，未写入仓库依赖。
