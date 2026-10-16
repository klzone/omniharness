# 当前实施计划

> 当前无待批准或执行中的计划。OD-BUILD-002 已由 BUILDER 实施并经 ARCHITECT 独立复核通过；MVP 仍受 REVIEW.md 所列适用限制。

## 下一步建议（尚未批准）

当前不新开实施计划。OD-BUILD-002 的直接问题已关闭；真实 Tauri 窗口验收、真实模型解释质量、跨平台行为及红线扫描局限仍需独立评估。只有用户提出下一项需求后，再据当前状态和证据建立具体计划。

## 已完成计划记录

### OD-BUILD-002：拒绝桌面应用数据根落入仓库

- 状态：已实施，ARCHITECT Review 通过。
- 目标：在 `AppPaths` 数据根解析和目录创建阶段拒绝仓库内路径、指向仓库的符号链接和仿真禁区，同时允许干净的仓库外路径。
- 实施文件：`desktop/src-tauri/src/app_paths.rs`、`STATUS.md`；无须修改 `lib.rs`。
- 独立验证：`cargo test --offline` 60 项通过；`npm run build` 通过；`git diff --check` 通过。具体证据和剩余限制见 `REVIEW.md`。
- 限制：常规路径校验不能消除检查与创建之间的并发替换竞态；当前仅在 Linux 实测，不代表跨平台已验证。

### OD-BUILD-001：个人 MVP 保存确认、来源真实性与输入处理边界

- 状态：已实施；独立 Review 中 R1–R3 通过，R4 经 OD-BUILD-002 补齐后通过。
- 详细结论保留在 `REVIEW.md`，执行事实保留在 `STATUS.md`。
