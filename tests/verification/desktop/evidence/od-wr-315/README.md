# OD-WR-315 验收证据

- 实施日期：2026-11-08
- 基线 HEAD：ba3a1c7c9bcf661b63ba5096a8dbc88ce1382507（main）
- 范围说明：未修改 STATUS.md / REVIEW.md / PLANS.md。

## 改动文件

- desktop/src-tauri/src/settings.rs：新增 working_root 可选字段、effective_working_root、set_working_root 与 3 项单测。
- desktop/src-tauri/src/lib.rs：work_root 改用 settings 解析；新增 work_root_info / task_set_working_root IPC 与注册。
- desktop/src/api.ts：workRootInfo / setWorkingRoot。
- desktop/src/types.ts：WorkingRootInfo。
- desktop/src/SettingsPanel.tsx：「工作记录临时层」设置入口。

## 生效顺序

OMNIDESK_WORK_ROOT（开发/测试隔离）> settings.v1 的 working_root > 默认 $HOME/OmniDesk/work。三种路径统一走成果目录同款越界校验。

## 实跑命令与结果

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| cargo test --offline --lib | 0 | 632 passed / 0 failed（含本任务 3 项） |
| npx tsc --noEmit | 0 | 无输出 |
| npm --prefix desktop run build | 0 | vite 构建成功（2.96s，额外核对） |

原始输出：tsc.log、npm-build.log、exit-*.txt；库单测日志与 304 同轮，落在 od-wr-304/cargo-test-lib.log。

## 已知限制

- 未在真实 Tauri 窗口点过新设置入口。
- 保存只影响之后新建的记录与任务；既有文件不迁移。
