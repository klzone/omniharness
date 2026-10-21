# OmniDesk UI v2（Aetheria OS 新壳）— 成果交接说明

> 用途：供 codex（BUILDER）审阅本分支的实现路径是否正确，再决定提交与合并方式。
> 状态：**未提交**、未推送；主仓库 `MyWorkStation` 工作区未被改动（指纹 `93dc57ce…` 全程一致）。

## 1. 分支与基线关系（先看这条）

- 分支：`feat/desktop-ui`（worktree：`/mnt/data/Code/omniharness-ui`），基线 `5ffc710`。
- **本分支 = codex 的未提交成果 + 本轮的 UI 改造**：
  - codex 侧改动以只读补丁（`git diff HEAD`，39 文件）由 ARCHITECT 应用进来，`git apply` 无冲突；
  - 因此本分支已包含：事件泵**锁饥饿修复**、工作台**引擎接线**、`task_prepare_workspace_dir`、cwd 规范化及相关测试。
- 归因：
  - **codex 的**：`desktop/src-tauri/src/*.rs`、`tests/host_integration.rs`、`engine/omnidesk_engine/acp_adapter.py`、`desktop/src/api.ts`、`desktop/src/workbench.tsx`、仓库根/`docs/` 的共享状态文件。
  - **本轮新增的**：见第 3 节。
  - 两侧文件**基本不重叠**（codex 未改 `App.tsx`；本轮未改 `api.ts` / `workbench.tsx` / 任何 `.rs`）。

## 2. 技术栈

| 层 | 选型 | 说明 |
| --- | --- | --- |
| 前端框架 | React 19.3.1 + react-dom 19.3.1 | 无状态库、无路由库、无 UI 组件库 |
| 语言/构建 | TypeScript 7.0.2 + Vite 8.3.4 + @vitejs/plugin-react 6.1.2 | 构建 `tsc --noEmit && vite build` → `desktop/dist` |
| 样式 | **Tailwind CSS 4.3.3（构建依赖，非 CDN）** + `@tailwindcss/vite` 4.3.3 | 令牌经 `@theme` 定义；旧深色样式保留但降级进 `@layer legacy` |
| 字体 | `@fontsource/*` 5.3.0（Sora / Plus Jakarta Sans / JetBrains Mono，仅 latin 子集） | **自托管**，运行时零外链，符合 CSP；CJK 回退系统字体 |
| 图标 | 自研内联 SVG（`components/Icon.tsx`） | 替代 Google Material Symbols 外链字体（被 CSP 拦截） |
| 桌面壳 | Tauri 2.12.4（既有，未改） | 窗口 label `main`，1280×820（最小 960×640） |
| IPC | **IPC-1**：`host.*` / `shell.*` / 事件 `omni://task-event` | UI 不接触 ACP / DSH 私有字段 |

**CSP（既有，未改）**：`default-src 'self'; img-src 'self' data: asset: http://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost`
→ 因此：不引外链脚本/样式/字体/图片；Tailwind 走构建；字体本地打包；图标内联。

## 3. 改动清单（本轮）

**新增（前端）**
| 文件 | 行数 | 作用 |
| --- | --- | --- |
| `desktop/src/theme.css` | 170 | Tailwind 入口 + Aetheria OS 令牌（颜色/字体/圆角/间距）+ 玻璃拟态类 |
| `desktop/src/components/Icon.tsx` | 181 | 24 个内联 SVG 图标 |
| `desktop/src/shell/TopNav.tsx` | 132 | 悬浮胶囊顶栏（品牌 / 真实 IPC 状态 / 搜索 / 时间 / 设置 / 通知 / 头像） |
| `desktop/src/shell/Dock.tsx` | 92 | 左 Dock（场景选择；未实现项禁用并标注） |
| `desktop/src/shell/HomeCanvas.tsx` | 125 | 首页画布（问候 + 两卡片 + 吉祥物） |
| `desktop/src/shell/HomeChat.tsx` | 346 | 首页 DSH 对话（IPC-1 全链路） |
| `desktop/src/shell/ThreeWeekCalendar.tsx` | 117 | 三周节律卡（上周/本周/下周） |
| `desktop/src/shell/EngineTechSlice.tsx` | 352 | 原引擎技术切片，收进顶栏「设置」 |
| `desktop/public/mascot.png` | 364 KB | 吉祥物（用户提供的 Stitch 素材，抠背景后 480×745） |
| `scripts/dev-ui.sh` | 160 | 一条命令启动：前端 + 隔离数据根 + 合成端点 + App |

**修改（本轮）**
| 文件 | 改动 |
| --- | --- |
| `desktop/src/App.tsx` | 重写为应用主壳：顶栏 + Dock + 场景（home / analysis）+ 技术切片入口 |
| `desktop/src/main.tsx` | 引入自托管字体（latin 子集）+ `theme.css` |
| `desktop/vite.config.ts` | 接入 `@tailwindcss/vite` |
| `desktop/src/styles.css` | 整体包进 `@layer legacy`（修复旧样式压过 Tailwind 的级联层冲突） |
| `desktop/package.json`(+lock) | 新增 5 个 dev 依赖：`tailwindcss`、`@tailwindcss/vite`、3 个 `@fontsource/*` |

**临时文件（建议交接前删除）**：`desktop/public/_preview.html`（仅用于窄窗口桌面视口验证）。

## 4. 设计系统（来自用户 Stitch 稿 `DESIGN.md`）

- 名称 **Aetheria OS**：浅色、玻璃拟态、弥散光；MD3 风格调色板；Bento 非对称布局。
- 字形：Sora（标题）/ Plus Jakarta Sans（正文）/ JetBrains Mono（技术读数与芯片）。
- 令牌全部落在 `theme.css` 的 `@theme` 中，类名与设计稿一致（如 `bg-surface-container-lowest`、`font-headline-md`）。
- 玻璃拟态构件（`.plate-sculpted` / `.plate-soft-inset` / 遮罩 / 动画）由设计稿 `<style>` 迁入。

## 5. 与 IPC-1 的边界（关键约束，未越界）

- `HomeChat` 只调用：`host.submitTask` → 订阅 `omni://task-event` 流式 → 轮询 `host.taskStatus` 判终态 → `host.taskResult` 回填 → `host.cancelTask`。
- `workspace_dir` 取宿主提供的 `shell.prepareWorkspaceDir()`（codex 新增），回退 `shell.localRootInfo().artifact_root`；两者都是**绝对路径、仓库外、过边界校验**。
  - 依据：`lib.rs` 不补默认值 → `serve.py` 取 `params.get("workspace_dir") or ""` → `acp_adapter.py` 要求 `isabs`；不传必 `INVALID_ARGUMENT`。
- 界面代码**不出现** ACP / DSH 私有概念；`desktop/src/types.ts` 未改。

## 6. 与设计稿的刻意差异（诚实性处理）

| 设计稿 | 本轮实现 | 原因 |
| --- | --- | --- |
| 全局搜索「检索任务/制度/数据/随记」 | 禁用占位，标「未实现」 | 无检索能力 |
| 天气「晴 21°C」 | 移除，仅显示本机日期/时间 | 需外网，不可离线 |
| Dock：周报随记 / 文案工坊 / 专项监督 | 禁用并标「未实现」 | GOALS 中属 reserved/未授权 |
| 「最新对话 / 送修中」等真实业务数据 | 空态（明确标注数据源未接入） | 无数据源，不伪造 |
| 助理气泡「已调用离线引擎完成 48 份合同交叉校验、滞后 +18.4%」 | **未采用**，改为诚实的未接入说明 | 违反「不以模型输出代替确定性结论」「须区分已实现/未实现」 |
| 导航「Yvan 主任」头像与身份 | 移除，改为中性产品标记 | 未接入身份系统 |
| 通知/待办 | 禁用并标「未实现」 | 未实现 |

## 7. 已实现 / 未实现

**已实现**：应用主壳；首页（问候、两卡片、吉祥物、三周节律、对话）；`本地数据分析` 场景（复用 codex 的工作台）；引擎技术切片（收进设置）；对话经 IPC-1 接引擎；禁用态与未实现标注。

**未实现 / 待定**：
- `本地数据分析` 场景仍是**旧深色样式**，未改成 Aetheria 风格（视觉不统一）。
- 首页两张卡片（今日重点推进 / 日程节律）为**空态**：待办与日程数据源未接入。
- 对话历史不持久化；无会话列表。
- 真实模型运行未验证（见第 8 节）。
- 字号/断点只按设计稿粗对齐；未做像素级比对。

## 8. 验证证据（本轮实测）

| 项 | 结果 | 等级 |
| --- | --- | --- |
| `npm run build` | ✅ 通过（CSS 59.8 kB / JS 278 kB + 21 个字体文件） | 静态/构建 |
| `cargo build --offline` | ✅ 通过（含 codex 锁修复） | 静态/构建 |
| `cargo test --offline` | ✅ **lib 47 + host_integration 14 + task_shell_e2e 3 = 64 passed / 0 failed** | 脚本/运行 |
| `tests/verification/desktop/run_desktop_e2e.py chain` | ✅ success/failure/cancel 全 pass（真实引擎 + 真实 DSH + 本机回环**合成**端点） | 脚本/运行 |
| 真实 Tauri 窗口渲染新 UI | ✅（用户本机已确认窗口可打开、可响应） | 运行 |
| 窗口内「打字→回车→回复」 | ⚠️ **未由 ARCHITECT 自动完成**（无头环境键盘注入受限）；用户已确认窗口正常打开 | 未验证 |

**未验证**：真实模型输出（全部为合成端点）；跨平台；macOS/Windows。

## 9. 交接 / 合并建议

- 推荐：先把本分支**提交为一个 commit**，codex 再在 `MyWorkStation` 中合并或按文件取用（两个目录是同一仓库的 worktree，无需推远端）。
- 两种方式：
  - A 整体合并：会把 codex 自己的成果与我的 UI 一起落到主线。
  - B 只取 UI 文件：`desktop/src/App.tsx`、`main.tsx`、`vite.config.ts`、`styles.css`、`theme.css`、`components/`、`shell/`、`public/mascot.png`、`package.json`(+lock)、`scripts/dev-ui.sh`。
- 交接时**不要** reset/clean codex 的未提交改动。

## 10. 请 codex 评估的问题

1. **Tailwind 作为构建依赖**是否接受？（替代方案：把设计稿转成纯 CSS 令牌，零新增依赖，但工作量大、偏差风险高。）
2. **分层**：是否把 `本地数据分析` 工作台也改成 Aetheria 风格？是否把引擎对话从首页抽成可复用组件？
3. 首页两张卡片是否需要有真实数据源后才填充？（当前坚持空态，不伪造。）
4. `_design/`（Stitch 原始导出）与 `mascot.png` 的归属与是否需要入库。
5. 是否保留 `scripts/dev-ui.sh` 作为仓库内开发启动器（含本机渲染规避与隔离数据根）。
