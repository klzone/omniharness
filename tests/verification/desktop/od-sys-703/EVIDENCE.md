# OD-SYS-703 验收证据

- 基线：HEAD `f4422b1da7b4812368ecfdf70899b1b49661915c`（2026-11-09，本机 Asia/Shanghai）。
- 任务：等待计时器按消息上的 `startedAtMs` 计算；引擎空闲时显示轻量提示，不自动停止。

## 实跑结果

| 命令 | 退出码 | 证据文件 |
| --- | --- | --- |
| `cd desktop && npx tsc --noEmit` | 0 | `tsc.txt`（空 = 无诊断输出）、`tsc.exit.txt` |
| `cd desktop && npm --prefix . run build` | 0 | `build.txt`、`build.exit.txt` |
| `python3 tests/verification/desktop/verify_sys_timer_703.py` | 0（13/13 PASS） | `verify_sys_timer_703.txt`、`verify_sys_timer_703.json` |

## 证据等级

- 静态源码检查：专项 verify 的 13 项断言（含设置页手动停止入口）。
- 构建检查：TypeScript 无诊断；Vite 生产构建成功（仅有既有的 chunk 体积提示，非错误）。
- 未验证：真实窗口中的计时恢复与提示出现时机（需运行应用并等待任务结束后 30 秒）。
