# OD-ARCH-025 证据索引：个人试用前桌面闭环验收

日期：2026-10-19　执行：BUILDER　状态：**已执行，等待 ARCHITECT 独立复核**

本目录保存「在**真实桌面窗口**里用合成引擎走通个人试用前闭环」的可复核证据：窗口截图、阶段事实 JSON、
隔离根内的设置/成果/引擎任务台账文件事实。**不含**真实工作材料、凭证、真实会话正文或用户私人路径清单。

对应计划：[PLANS.md 的 OD-ARCH-025](../../../../PLANS.md)。

---

## 0. 结论摘要（以本目录证据为准）

| 计划步骤 | 结论 | 证据等级 |
| --- | --- | --- |
| (1) 只读核对启动入口与各功能接线 | 已完成：启动器 `scripts/dev-ui.sh`（本装置改为等价直启，见 §2）、设置/成果目录/会话/任务接线已读源码核对 | 静态代码 |
| (2) 隔离根 + 真实窗口 + 合成引擎，AI 写作任务与项目事项流程，用户设置成果目录实际生效、可保存并可重开 | 见 §3 阶段 A | 真实窗口运行 + 文件事实 |
| (3) 关闭并重启，会话/任务状态按契约恢复；可控慢关闭下真实关窗，界面与宿主未卡死，记录关闭耗时与任务状态 | 见 §3 阶段 B / C | 真实窗口运行 + 文件事实 |
| (4) 桌面安装入口可启动应用；设置/成果目录重启后仍保持 | 见 §3 阶段 D | 构建产物 + 真实窗口运行 |
| (5) Rust 离线全测、前端构建、桌面任务/取消/会话回归、`git diff --check HEAD`、`runtime/` 无差异 | 见 §4 | 脚本运行 |

仍然**未验证**的部分见 §6。

---

## 1. 运行环境与边界

- 工作区：`/mnt/data/Code/MyWorkStation`，分支 `main`，开工 HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`；
  开工时 101 项未提交/未跟踪改动全部保留，本轮结束未提交、未推送、未打标签。
- 隔离运行根：`/tmp/od025-evidence/run`（仓库外一次性根：`app-data/`、`dsh-home/`、`agents/`、`logs/`）。
  成果目录由**用户在设置界面里改到** `/tmp/od25a`（同样在仓库外）。
- 显示：一次性 `Xvfb :95`（1440×1000），窗口为 `tauri.conf.json` 默认 1280×820，标题 `OmniDesk Discipline`。
- 合成模型端点：本机回环 `127.0.0.1`（`tests/verification/engine-interface/adapter/_mock_model_server.py`），
  通过合成 provider patch 注入；`OMNI_FAKE_KEY` 为占位值，**不是凭证**。
- 未读取、未列出、未修改 `~/.dsh`、`~/.agents`、`~/OmniDesk`、既有用户数据目录或任何真实会话；
  未调用任何真实模型端点；未改动 `runtime/`、模型路由、端点权限或系统级安装。
- **未安装任何系统依赖、未提权**；只使用容器内已有的 `python3-xlib` / PyGObject / PIL。

### 1.1 环境事实上被记录的两件事

1. **本机是 Wayland 会话**（环境变量 `GDK_BACKEND=wayland`、`WAYLAND_DISPLAY=wayland-0`）。
   若不强制 `GDK_BACKEND=x11` 并清除 `WAYLAND_DISPLAY`，GTK/WebKit 会优先连用户的 Wayland 会话而不是本次
   一次性 Xvfb 显示——那既破坏隔离，也可能干扰用户正在使用的桌面。装置已强制 X11（`window_harness.py`）。
2. 无窗口管理器时 GTK 不发送 FocusIn，键盘事件会被丢弃；装置显式 `XSetInputFocus`。
   无窗口管理器时 WebKit 的重绘会滞后（DOM 已切换、像素仍是上一页），装置用 1px 尺寸微调强制重绘后再截图。

### 1.2 为什么本装置不使用 `scripts/dev-ui.sh` 的 `OMNIDESK_LOCAL_DATA_ROOT`

`dev-ui.sh` 会设置 `OMNIDESK_LOCAL_DATA_ROOT`，按既有优先级它**覆盖用户设置的成果目录**（设置界面也会
明确提示这一点）。而本计划要验证的恰恰是「用户设置的成果目录实际生效」，因此本装置用等价的直启环境、
**不设置**该变量；其余隔离变量（应用数据根、DSH 根、合成 patch、密钥环境变量、禁用 DMABUF）与脚本一致。

---

## 2. 验证方法（可复现）

装置与脚本（本轮新增，均在本仓库 `tests/verification/desktop/`）：

| 文件 | 作用 |
| --- | --- |
| `window_harness.py` | 一次性 Xvfb 上的窗口驱动：XTEST 注入鼠标/键盘、GDK 抓图（稳定帧）、显式输入焦点、回收 GTK 原生弹窗、窗口查找/几何、`WM_DELETE_WINDOW` 关窗；以及隔离启动（合成端点 + 前端 dev server + 应用进程）与按身份回收 |
| `run_window_acceptance.py` | 四个阶段的验收场景与事实判定（`--stage A|B|C|D`），逐屏截图 + 阶段 JSON |

为什么自建：本机没有 `xdotool` / `import` / `scrot`，X 侧只有 `xwininfo` / `xprop`，图形侧可用
PyGObject（抓图）与 python-xlib（XTEST 注入）。截图与注入都在同一次性显示上做，不触碰用户桌面。

复现命令（`DISPLAY` 上的一次性 Xvfb 需先存在）：

```bash
Xvfb :95 -screen 0 1440x1000x24 &
cd /mnt/data/Code/MyWorkStation
python3 tests/verification/desktop/run_window_acceptance.py --stage A --only all   # 主闭环
python3 tests/verification/desktop/run_window_acceptance.py --stage B             # 重启后状态
python3 tests/verification/desktop/run_window_acceptance.py --stage C             # 可控慢关闭
python3 tests/verification/desktop/run_window_acceptance.py --stage D             # 安装入口
```

每个阶段都会写出 `stage-<X>.json`（判据 + 文件事实）与 `shots/<X>/*.png`（逐屏截图）。

---

## 3. 阶段结果

四个阶段全部 **verdict=pass**；逐条判据见 `stage-A.json` / `stage-B.json` / `stage-C.json` / `stage-D.json`，
截图见 `shots/A|B|C|D/`（共 44 张）。执行时一次性根为 `/tmp/od025-evidence/run`，成果目录为 `/tmp/od25a`。

### 阶段 A：主闭环（`stage-A.json`，截图 `shots/A/01..22`）

| 判据 | 结果 | 文件事实 |
| --- | --- | --- |
| 真实桌面窗口出现 | PASS | 标题 `OmniDesk Discipline`，几何 1280×820 @ (80,90)，显示 `:95` |
| 用户设置的成果目录写入设置文件 | PASS | `app-data/config/settings.v1.json` → `"artifact_root": "/tmp/od25a"` |
| 成果目录已按设置创建 | PASS | `/tmp/od25a` 存在 |
| 开启本机模型后权限写入设置文件 | PASS | `"local": true`（private/public 仍为 false） |
| 引擎任务完成并落盘（合成端点） | PASS | `engine-state/omni-d2fc2276…json`：`state=succeeded`、`stop_reason=end_turn`、`content=合成回复内容`、`workspace_dir=/tmp/od25a` |
| AI 任务成果保存到用户设置的目录 | PASS | `20261019-035655-320dae745102.md`（1714 字节） |
| 离线项目流程成果保存到用户设置的目录 | PASS | `offline-project-management-20261019-035831.md`（1059 字节） |
| 成果列表可重新打开（只读） | PASS | `22_reopened.png`：界面出现「已重新打开的成果（只读）」内容区 |

界面侧同时可核对（截图）：设置页「当前生效」显示用户设置的 `/tmp/od25a`（`03_artifact_dir_saved.png`、`04_settings_permissions.png`）；
工作台阶段 3「解释来源」= `模型输出·未复核（model_unverified）`、解释框回填引擎输出（`13_engine_result.png`、`14_before_confirm.png`）；
阶段 5「产物目录」= `/tmp/od25a`（`16_saved.png`）。

### 阶段 B：关闭并重启后的状态（`stage-B.json`，截图 `shots/B/01..06`）

| 判据 | 结果 | 依据 |
| --- | --- | --- |
| 重启后真实窗口出现 | PASS | 同一数据根再启一次 |
| 重启后成果目录设置保持 | PASS | 设置文件仍为 `/tmp/od25a`；界面「当前生效」同值 |
| 重启后模型权限保持 | PASS | `"local": true` |
| 重启后引擎任务记录仍在 | PASS | `omni-d2fc2276…`：`succeeded` |
| 重启后成果目录内文件仍在 | PASS | 两份成果文件都在 |
| 重启后可重新打开成果（只读） | PASS | `05_reopened_after_restart.png` |
| 会话与任务只读视图 | PASS | `06_sessions_after_restart.png`：列出引擎会话与任务记录（元数据） |

### 阶段 C：可控慢关闭 → 真实关闭窗口（`stage-C.json`，截图 `shots/C/01..04`）

场景：把合成端点切到 `hang`（不返回响应），提交引擎任务，任务持续进行中，然后按窗口管理器的正常路径
（`WM_DELETE_WINDOW`，等价于点标题栏关闭）真实关闭窗口。

| 判据 | 结果 | 依据 |
| --- | --- | --- |
| 关闭前引擎子进程在运行（场景前提） | PASS | `python3 -m omnidesk_engine` + DSH `bin.js --profile acp` |
| 任务进行中界面仍可响应 | PASS | 关闭前从工作台切到设置页再切回（`02_ui_responsive_while_task_running.png`、`03_back_to_running_task.png`） |
| 真实关闭窗口后应用进程自行退出（未被强杀） | PASS | 0.25s 后进程退出、`exit_code=0`（未发 SIGKILL） |
| 关闭后显示上不再有应用窗口 | PASS | 窗口查找失败 |
| 关闭后无孤儿引擎进程 | PASS | 进程表无 `omnidesk_engine` / `dsh` 残留 |
| 被关闭打断的任务未留下伪造的成功终态 | PASS | 新建台账 `omni-f03b1a56…`：`state=starting`（非 succeeded） |

### 阶段 D：桌面安装入口（`stage-D.json`，截图 `shots/D/01..03`）

| 判据 | 结果 | 依据 |
| --- | --- | --- |
| 安装包存在（当前源码构建） | PASS | `desktop/src-tauri/target/release/bundle/deb/OmniDesk_0.1.0_amd64.deb`（本轮 `npm run tauri:build -- --bundles deb` 重建，release 编译 1m32s） |
| 安装包可解包 | PASS | `dpkg-deb -x` 到 `/tmp/od025-evidence/deb-root` |
| 安装入口与桌面项存在 | PASS | `usr/bin/omnidesk-desktop`、`usr/share/applications/OmniDesk.desktop` |
| 安装入口启动出真实窗口 | PASS | 用解包后的二进制启动，出现真实窗口 1280×820 |
| 安装实例沿用同一数据根：成果目录设置保持 | PASS | 设置文件 `artifact_root=/tmp/od25a`、`local=true` |
| 安装实例可见既有成果文件 | PASS | 两份成果文件仍在并在界面可见 |
| 任务台账文件可读（终态如实记录） | PASS | `succeeded` 与 `starting` 各自如实记录；会话页显示「恢复状态未验证」并提供「读取状态」，不自动重放 |

> 注：阶段 D 只验证「安装入口可启动」，未执行系统级 `dpkg -i`（见 §6）。


---

## 4. 计划要求的检查

本目录 `.log` 为原始输出（含 `EXIT=`）。运行目录：仓库根 `/mnt/data/Code/MyWorkStation`。

| 检查 | 命令 | 结果 | 日志 |
| --- | --- | --- | --- |
| Rust 离线全测（当前源码） | `cd desktop/src-tauri && eval "$(bash ../../scripts/setup-desktop-toolchain.sh --print-env)" && cargo test --offline` | **134 passed / 0 failed / 0 ignored**；编译日志含 `Compiling omnidesk-desktop`、`Finished test profile in 1m 07s` | `cargo-test-offline.log` |
| 前端构建 | `cd desktop && npm run build` | 成功（vite build，`✓ built`） | `npm-build.log` |
| TypeScript 类型检查 | `cd desktop && npx tsc --noEmit` | EXIT=0 | `tsc-noemit.log` |
| 桌面引擎链路回归 | `python3 tests/verification/desktop/run_desktop_e2e.py chain` | success/failure/cancel 三场景全 pass | `e2e-chain.log` |
| 工作台引擎路径回归 | `python3 tests/verification/desktop/run_workbench_engine_path.py` | pass（success / failure / cancel） | `workbench-engine-path.log` |
| AI 初稿回归 | `python3 tests/verification/desktop/verify_ai_draft.py` | 22 例 0 失败 | `ai-draft.log` |
| 取消回归 | `python3 tests/verification/desktop/verify_engine_cancel.py` | 状态 34 例 0 失败；静态检查 8 项 0 失败 | `engine-cancel.log` |
| 会话连续性回归 | `python3 tests/verification/desktop/verify_session_continuity.py` | 37 项 0 失败 | `session-continuity.log` |
| 空白错误检查 | `git diff --check HEAD` | EXIT=0（无输出） | `git-diff-check.log` |
| `runtime/` 无差异 | `git diff --stat HEAD -- runtime/` | 空（无差异） | `runtime-diff.log` |

关于「测试是否代表当前源码」：本计划**未改动任何 Rust/前端产品源码**（只新增了 `tests/verification/desktop/window_harness.py`、
`run_window_acceptance.py` 与证据/状态文档）。`desktop/src-tauri/src/lib.rs` 的最后修改时间与 debug 测试二进制一致
（均为 `10-19 07:35`，即 OD-ARCH-024 第四轮收尾），`cargo test` 本轮重新编译并运行，134 项通过。


---

## 5. 判定口径说明（避免过度表述）

- 截图是「真实窗口像素」，但**不构成**对界面文案的逐字校对；文案结论另外以源码与文件事实为准。
- 「成果可重开」的判据是：界面出现「已重新打开的成果（只读）」内容区，且磁盘文件存在、内容与之一致。
- 引擎输出固定记为 `model_unverified`（未复核）；本计划不评价输出质量与文风。
- 关闭耗时是「发送 `WM_DELETE_WINDOW` 到进程退出」的墙钟时间，仅代表本机这次运行。

---

## 6. 未验证 / 不适用

- 真实模型的文风与质量、真实工作材料、长期易用性：**未验证**（按计划不属于技术验收）。
- 跨平台（Windows/macOS）、真实显示器与真实窗口管理器的行为：**未验证**（本轮为 Xvfb + 无窗口管理器）。
- 真实工作区（非隔离根）下的成果落盘与用户数据迁移：**未验证**，本轮全部在仓库外一次性根内完成。
- 系统级 `.deb` 安装（`dpkg -i`、/usr 写入、桌面项注册）：**未执行**；本轮只做解包后启动入口验证。
- 全局检索框、真实端点：入口仍按既有实现保持关闭/未实现。
