# OD-ARCH-033 派工指令（给 super-worker）

> 用途：用户已批准「让 super-worker 执行 OD-ARCH-033」。
> 本文件是一次**派工尝试**的留档：2026-10-20 在主会话里调用 subagent 传 `agent: "super-worker"`，
> 工具返回 `Unknown agent: super-worker`——**该会话的 agent 目录里没有这两个项目 agent**
> （它们创建于 2026-10-19 22:47 之后，而该会话的 agent 目录建立得更早）。
> 这与 T-001 记录的「另一会话能发现、另一会话不能」的不一致同源，尚未查清根因。
>
> **可执行路径：** 在**能发现项目 agent 的会话**里（写 032/033 计划的那条会话，或新开一个会话），
> 用 super-plan 把下面这段原样派给 super-worker。super-plan 的 system prompt 已包含协作纪律，
> 派工时只需附上本段与边界。

---

## 给 super-worker 的任务书（原文照发）

你被派工执行 **OD-ARCH-033：场景模块页设计 v3**（用户已批准执行）。仓库：/mnt/data/Code/MyWorkStation（HEAD 80fe21d）。

### 必读
1. `PLANS.md` 顶部 `### OD-ARCH-033` 段（设计定稿 v2 + v3 提醒/建议栏 + 目标 + 影响范围 + 验收标准 + 风险与停止条件）——**这是唯一权威口径**，先读它，再读代码。
2. `GOALS.md`（产品方向）、`REVIEW.md` 顶部几节（已定结论与已知限制）、`desktop/src/`（`App.tsx`、`shell/Dock.tsx`、`shell/HomeCanvas.tsx`、`shell/HomeChat.tsx`、`shell/Workspace.tsx`、`shell/ProjectRail.tsx`、`OfflineFlows.tsx`、`conversationPrompt.ts`、`api.ts`、`types.ts`）。

### 现状（主会话已完成，别重复造）
- 首页已是用户 2026-10-19 确认的定版版式（**无项目树、无左右分栏**），左侧只有 Dock 进各场景。
- 「项目与会话」已是一个 Dock 模块页（`Workspace.tsx` + `ProjectRail.tsx`）：左侧「引擎项目 / 本机对话」两分页，右侧对话区。
- `conversationPrompt.ts`：上下文累积 + 预设提示词组装（脚本级验证 12/12）。
- 三场景仍是旧的**表单向导页**（`OfflineFlows.tsx`），这正是 033 要改的部分。

### 必须遵守的纪律
- **只做获批范围**：OD-ARCH-033 的目标 1–5。不新增通信层、不改 `runtime/`、不动模型路由与凭证、不碰用户真实数据。
- **保留全部既有未提交/未跟踪/暂存改动**（开工前工作区有 1 处 PLANS.md 改动）。**不重置、不清理、不覆盖**。
- **不 commit、不 push、不打标签**（复核后由 ARCHITECT 统一入库）。
- 不读取/打印/复制凭证；不把真实业务材料带入仓库；不新增通信协议。
- 诚实性纪律必须保留在代码注释与界面文案里：**工作台每轮是一次独立任务**（引擎会话号 ≠ 多轮）；对话上下文目前存在**浏览器本地存储**，若 033 目标 4 要求挪到仓库外数据根而这需要**新增宿主命令或改协议**，**不要擅自加**——按停止条件**停下并报告**。
- 判定「成果文件夹只读本场景、下级目录是否包含做成设置项（默认不含）」「进度数据来自现有确定性汇总」时，**复用现有确定性逻辑**（`task_weekly_summary` / `task_project_summary` 等已存在的宿主能力），不要用模型算、不要编造数字。
- 「AI 复盘建议」涉及定时调模型：**默认关闭**；第一版**不做**联网趋势热点（受限能力，用户未授权数据源）。做不出来的部分**如实说明**，不要用假数据填充界面。

### 交付与验证
- 真实窗口截图（首页 + 周报页 + 项目跟进页）放到 `tests/verification/desktop/evidence/od-arch-033/shots/`；截图脚本放同目录 `scripts/`。
- 跑通并如实记录：`cd desktop && npx tsc --noEmit`、`npm run build`、`cd ../desktop/src-tauri && cargo test --offline`（工具链环境变量用 `eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)"`）。
- 执行记录（做了什么、命令与结果、**哪些没做完/为什么**、哪些需要宿主新增命令）写进 `tests/verification/desktop/evidence/od-arch-033/README.md`。
- **不要**改 `PLANS.md` / `STATUS.md` / `REVIEW.md`（由 ARCHITECT 统一登记，避免与另一会话冲突）。
- 结束时在最终回复里给出：改了哪些文件、验证命令与结果、未完成项与原因、需要决策的问题。

开工前先记录 `git status --porcelain -uall` 的条数与 HEAD，完成后确认**条数不减少**（不得丢失任何既有改动）。
