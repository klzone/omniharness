# BUILDER 执行状态

> 更新日期：2026-10-16。此文件记录 BUILDER 的执行事实，不替代 ARCHITECT 的独立 Review。OD-BUILD-002 已实施并通过独立复核；MVP 仍受 REVIEW.md 中未验证项与数据准入限制约束。

## 最近完成任务

- 任务：[PLANS.md](PLANS.md) 中的 **OD-BUILD-002：拒绝桌面应用数据根落入仓库**（已完成）。
- 授权：用户下达“总监审定有一项工作不通过，检查 plan 并按计划实施”的执行指令，本计划视为已批准执行。
- 背景：OD-BUILD-001 的 R1–R3 经 ARCHITECT 复核通过；R4 因桌面应用数据根 `AppPaths` 缺少仓库边界校验不通过（见 [REVIEW.md](REVIEW.md)）。OD-BUILD-002 只补齐该遗漏。
- 角色：BUILDER（实施工程师），范围严格限定在计划“影响范围”内，未扩大。

## 已完成内容

按 OD-BUILD-002 补齐 `AppPaths` 的仓库边界校验：

- **数据根拒绝仓库内路径。** 新增 `assert_outside_repo_boundary()`，并在 `assert_isolated()` 内对**文本路径**与 `resolve_real_path()` 解析后的**真实落点**同时比较仓库根；任一落入仓库即拒绝。仓库根沿用与 `task_shell` 相同的 `OMNIDESK_REPO_ROOT` / `CARGO_MANIFEST_DIR` 约定，两类数据根共享同一边界。
- **创建目录前先校验。** `AppPaths::ensure()` 在调用任何 `create_dir_all` 之前先执行 `assert_isolated()`；校验失败即以 `PermissionDenied` 返回，绝不在可疑路径上创建 `ui-state/`、`logs/` 等目录。
- **解析失败默认拒绝。** 数据根本身 `resolve_real_path()` 失败即拒绝；仅仓库根这一本机已知固定目录在解析失败时退回词法归一结果。
- **回归测试。** 新增 5 个 `app_paths` 单测，覆盖普通仓库路径、软链接指回仓库、软链接指向仿真禁区、干净仓库外路径，以及 `ensure()` 拒绝创建且事后无残留。

## 修改文件

本轮由 BUILDER 改动（均在 OD-BUILD-002 允许清单内）：

| 文件 | 改动 |
| --- | --- |
| desktop/src-tauri/src/app_paths.rs | 新增 `assert_outside_repo_boundary`、`repo_root_hint`、`normalize_lexical`；`assert_isolated()` 纳入仓库边界；`ensure()` 创建前先校验；新增 5 个单测 |
| STATUS.md | 本执行记录 |

未改动 `runtime/`、模型路由、凭证、`~/.dsh`、`~/.agents` 或任何真实工作材料。`desktop/src-tauri/src/lib.rs` 本轮未改：`build_state()` 现有顺序已是先 `resolve()` 后 `ensure()`，而 `ensure()` 现在自身拒绝仓库内路径，无需调整启动校验次序。

## 测试命令与结果（本轮实测，证据等级：脚本/运行）

| 命令 | 结果 |
| --- | --- |
| `cd desktop/src-tauri && cargo test --offline` | **60 passed / 0 failed**：lib 44（较修复前 39 增加 5）、host_integration 13、task_shell_e2e 3；doc-tests 0 |
| `cargo build --offline` | 无 warning、无 error |
| `cd desktop && npm run build`（tsc --noEmit + vite build） | 通过；22 modules，245.68 kB JS / 7.48 kB CSS |

新增回归测试（均实测通过）：

- `rejects_plain_path_inside_repository`：普通仓库内目录（无软链接、无禁区片段）被拒绝。
- `rejects_symlink_pointing_into_repository`：词法名字干净、真实落点在仓库内的软链接被拒绝。
- `rejects_symlink_pointing_to_simulated_forbidden_root`：词法名字干净、真实落点为仿真禁区（`.dsh`）的软链接被拒绝。
- `accepts_clean_path_outside_repository`：干净的仓库外临时目录通过。
- `ensure_refuses_to_create_directories_inside_repository`：`ensure()` 拒绝在仓库内创建目录，并断言事后该目录不存在，证明拒绝路径不会在仓库留下内容。

运行 `cargo` 前需注入 Tauri 依赖的 `PKG_CONFIG_*` 与 `LIBRARY_PATH`（前缀 `$HOME/.local/omnidesk-tauri-deps`），否则链接阶段失败；此为本机约定，未改动仓库。

## 未验证 / 未运行事项

- 未运行任何真实模型调用；未做真实 Tauri 窗口的人工视觉验收；未做跨平台验收。
- 仓库边界与符号链接行为只在当前 Linux 环境用 Rust 测试覆盖。`resolve_real_path()` 与 `create_dir_all` 之间仍存在“检查后 / 创建前”的并发路径替换竞态，本修复未消除该竞态，也未采用无跟随链接的目录句柄方案；该限制保留如实记录，不宣称对同机并发替换具备原子安全保证。
- 真实工作材料、凭证、原版 DSH 数据根与 DSH Runtime 本轮均未接触。

## 是否发现 Plan 与实际代码存在冲突

- 未发现阻断性冲突。计划第 2–4 步与现有 `AppPaths`、`task_shell` 的实际结构一致：两者已共享同一仓库根约定，可直接复用。
- 计划把 `lib.rs` 列为“仅在确需调整启动校验次序时”允许改动；实施确认无需改动，理由见上。

## ARCHITECT 独立复核状态

- OD-BUILD-002：Review **通过（本计划范围）**。独立重跑 `cargo test --offline`：60 passed、0 failed（44 单元、13 宿主集成、3 E2E）；独立重跑 `npm run build`：通过。
- `AppPaths::assert_isolated()` 同时执行词法和真实落点仓库边界检查；`ensure()` 在创建目录前再次执行边界校验。新增测试覆盖计划指定的仓库内路径、软链接仓库、仿真禁区、仓库外临时路径及失败后无目录残留。
- 路径解析与 `create_dir_all()` 之间的并发替换竞态未消除；符号链接仅当前 Linux 验证。此结论不是全 MVP 或真实工作材料准入验收，详见 [REVIEW.md](REVIEW.md)。
- 当前无待批准或执行中的 Builder 计划；[PLANS.md](PLANS.md) 中下一步只是 ARCHITECT 的后续评估建议。

## 建议 ARCHITECT 下一步关注

1. 基于实际 diff 与上述测试命令独立复核 R4 是否闭合：`assert_isolated()` 是否同时校验词法路径与真实落点、`ensure()` 是否在创建目录前拦截、软链接测试是否覆盖“真实落点”而非仅词法比较。
2. 确认“无原子保证”的限制已在 STATUS 与审查材料中如实保留，未把符号链接检查描述为抵御同机并发路径替换的原子安全边界。
3. 决定是否将 R4 的跨平台行为、以及检查与创建之间的竞态列为独立后续项。
4. 复核后更新 [REVIEW.md](REVIEW.md)；在通过前 MVP 维持未验收，不得据此扩大真实材料使用。

---

## 历史记录（保留，未改动其结论）

### OD-BUILD-001（R1–R4 实施；ARCHITECT 复核 R1–R3 通过、R4 不通过）

- 实施 R1 保存强制人工确认、R2 移除可伪造的 `model + succeeded` 来源、R3 统一输入闸门、R4 路径真实落点校验。
- ARCHITECT 复核：R1–R3 通过；R4 不通过（P1）——产物路径侧已覆盖真实落点，但桌面应用数据根 `AppPaths` 未验证仓库边界，可被指向仓库。
- 实施中一并删除了在移除 `model` 后已无使用者的 `task_state` 字段（R2 的直接结果，未改变数据模型方向）；端到端测试文件按“相邻测试”范围适配 R1 字段变更。
- 当时 HEAD `17efd20`；工作区 50 项既有改动全部保留，暂存区为空，未提交、推送或打标签。

### 更早状态（OD-ARCH-001 文档收敛与审查基线）

- 最近完成的 ARCHITECT 工作：OD-ARCH-001 独立审查与共享文档基线收敛；审查结论 MVP 不通过（R1–R4）。
- 5 份历史规划材料移至 `docs/archive/` 保留，未删除；协作基线集中在仓库根 `AGENTS.md`、`GOALS.md`、`PLANS.md`、`STATUS.md`、`REVIEW.md` 与 `docs/decisions.md`。
- 审查时 HEAD `17efd20`；工作区 175→181 项，暂存区为空；所有差异保留，未提交、未推送、未打标签。
