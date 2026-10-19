# OD-ARCH-027 证据索引：桌面端 WebKit 渲染性能治理（OD-ARCH-026 支线）

> 本文件是 OD-ARCH-027 的**单一证据索引**。计划文档见
> [`docs/OmniDesk-Desktop-Render-Performance-Plan.md`](../../../../../docs/OmniDesk-Desktop-Render-Performance-Plan.md)。
> **Step 0 归档时未修改任何产品代码**；本目录存放 Step 0（证据归档）、**Step 2（CSS 优化，§6）**、**Step 3（DMABUF/显示后端对照，§7）**与 **Step 4（无限动画与重渲染治理，§8）**的产物。

## 1. Step 0 归档：存在性与处置

对 `/tmp` 的清点结果：

| 项目 | 状态 | 处置 |
| --- | --- | --- |
| 可复现脚本（`/tmp/od025-perf.py`、`perf2.py`、`perf3.py`） | **存在** | 原件归档至 `scripts/`（未改写） |
| 对照页（`/tmp/od025-evidence/baseline/{simple,heavy}.html`） | **存在** | 归档至 `baseline-pages/` |
| 运行环境事实（合成 provider 覆盖层、Xvfb 日志、app 日志） | **存在** | 归档至 `env/` |
| 运行截图（perf 运行结束时的窗口帧，debug / release） | **存在** | 归档至 `screenshots/` |
| **原始测量结果**（空闲 CPU、导航延迟的具体数值输出） | **不存在**：脚本当时只把结果打印到标准输出，未写文件；对应会话输出文件已被清理 | **不补造**。基线在 Step 2 重新采集并**落盘** |

## 2. 归档内容说明

### 2.1 `scripts/`（逐字归档，未改写）

| 文件 | 当时测量口径 |
| --- | --- |
| `od025-perf.py` | 在同一 `Xvfb :95` 上分别启动 debug+vite 与 release+内嵌产物，测「窗口停在首页 5 s 的空闲 CPU（App 主进程 + WebKit 子进程）」与「Dock 导航延迟（点击后轮询截图直到画面变化，每 150 ms）」。**导航延迟含截图读回开销，只能作相对参考。** |
| `od025-perf2.py` | 只测一次 release 运行，按进程拆分空闲 CPU（App 主进程 / WebKitNetworkProcess / WebKitWebProcess），用于确认是谁在占 CPU。 |
| `od025-perf3.py` | 用系统自带 `MiniBrowser` 做**对照实验**：同一一次性显示下，「简单页」与「复刻本应用特效层的页面」的空闲 CPU 对比；并对同一特效页分别测 `WEBKIT_DISABLE_DMABUF_RENDERER=1/0`。 |

### 2.2 `baseline-pages/`

- `simple.html`：无模糊、无动画的普通页面（对照下界）。
- `heavy.html`：复刻本应用的候选高成本 CSS——三个全屏 `blur(120–140px)` 渐变光斑、`backdrop-filter: blur(40px)` 的顶栏与 Dock、5 个 `backdrop-filter: blur(20px)` 面板、`blur(28px)` 的 plate，以及 `pulse-subtle` / `float-companion` 两个无限动画。
  **注意：`heavy.html` 是「把若干可疑因素放在一起」的复刻页，不能用于把成本归因到某一个具体因素。**

### 2.3 `env/`

- `synthetic-provider.debug.yml` / `synthetic-provider.release.yml`：两次运行使用的合成 provider 覆盖层（回环合成端点，零凭证、零外网）。
- `app.debug.log` / `app.release.log`：应用标准输出（含 vite 相关行）。
- `xvfb.log`：一次性显示的日志（为空）。

### 2.4 `screenshots/`

- `perf-debug.png` / `perf-release.png`：两次运行结束时的窗口帧（用于说明当时窗口尺寸与页面状态，**不是**性能证据）。

## 3. 会话内观测（**线索，非基线**）

计划文档 §1.2 记录了会话内观测到的数量级：空闲时 `WebKitWebProcess` 约 100%（一个核）、App 主进程约 1.7%；
debug 与 release 的空闲 CPU 基本一致；MiniBrowser 对照中简单页约 0%、复刻特效页约 100%、该页开启 DMABUF 后约 34.5%。

**这些数值没有原始输出文件留存，只作为排查线索。** 它们**不构成** OD-ARCH-027 的验收基线；
Step 2 必须按计划 §6 重新采集并落盘（JSON + 环境快照 + 前后截图）。

## 4. 复现（当前可复现到什么程度）

```
# 对照页可独立复现（需要一次性显示；MiniBrowser 位于 webkit2gtk-4.1 目录）
Xvfb :95 -screen 0 1440x1000x24 &
python3 tests/verification/desktop/evidence/od-arch-027/scripts/od025-perf3.py

# 应用侧对比需要与当前工作区一致的构建与隔离数据根：
#   scripts/od025-perf.py 与 perf2.py 依赖与本仓库一致的 debug/release 二进制、
#   一次性数据根（/tmp/od025-evidence/perf*）与窗口装置 tests/verification/desktop/window_harness.py。
# Step 2 会基于这些脚本重写为落盘版（结果 JSON + 环境快照 + 前后截图）。
```

## 5. 边界

- 归档只新增文件，未改动任何产品代码、脚本默认值或其他证据目录。
- 未读取/复制凭证；合成端点、合成页面与一次性数据根中不含真实材料。

---

## 6. Step 2：CSS 性能优化（2026-10-19，BUILDER 执行记录 / 非审查结论）

> 本节记录 Step 2 的落盘证据与实测数字。**结论不在本文件**，由 ARCHITECT 独立复核。
> 开工 HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`；工作区未提交/未跟踪改动条目
> **保持 108 项不变**（Step 2 新增文件全部落在原本就已未跟踪的 `evidence/od-arch-027/`
> 目录内，因此没有新增顶层条目）；`git diff --check HEAD` 干净；`runtime/` 无差异；
> **未改动任何 Rust 文件**（15 个既有 `.rs` 差异全部早于本次会话，最新 mtime 14:59）。

### 6.1 改了什么（只动样式与既有类；可独立回退）

| 文件 | 跟踪状态 | 改动 |
| --- | --- | --- |
| `desktop/src/App.tsx` | 已跟踪（M） | 三个全屏装饰光斑：`bg-gradient-to-*/from-*/to-* + blur-[130px]/[140px]/[120px]` → `ambient-glow ambient-glow-a/b/c`（去掉高半径 `filter: blur`）。位置/尺寸/圆角类不变 |
| `desktop/src/theme.css` | 未跟踪（既有） | ① `.plate-sculpted` 删除 `backdrop-filter: blur(28px) saturate(170%)`；② 新增 `.ambient-glow` / `-a` / `-b` / `-c`：用 `radial-gradient(closest-side, color-mix(...), transparent)` 表达同色系静态光晕 |
| `desktop/src/workbench-ui.css` | 未跟踪（既有） | `.wb-v2 .pane` 删除 `backdrop-filter: blur(20px) saturate(160%)`（每个阶段卡片一层） |
| `desktop/src/offline-flows.css` | 未跟踪（既有） | `.ofl-v2 .pane` 删除同上（每个面板一层） |
| `desktop/src/shell/HomeDashboard.tsx` | 未跟踪（既有） | 三处卡片去掉 `backdrop-blur-md`（×2）/`backdrop-blur-xl`（×1）；结构与 props 不变 |
| `desktop/src/shell/Dock.tsx` | 未跟踪（既有） | Dock 去掉 `backdrop-blur-2xl`、底色 `.85→.95`；悬浮提示去掉 `backdrop-blur-md`、底色 `.90→.95` |

**保留的关键视觉层：** `desktop/src/shell/TopNav.tsx` 顶栏的 `backdrop-blur-2xl`（Step 2 中**唯一保留**的玻璃层）。
替代观感 = 半透明底色 + 阴影 + 描边；背景光斑由「模糊的实心渐变块」变为「边缘自然淡出的柔和光晕」。
范围外未动：`shell/EngineTechSlice.tsx` 的 `backdrop-blur-sm`（只读覆盖层，默认不出现），
以及全部无限动画与 `TopNav` 定时重渲染（属 Step 4）。

**注意（供复核）：** 上表中 5 个文件在本次开工前就是**未跟踪**状态（OD-ARCH-026 新增/改写），
因此 `git diff HEAD` **看不到** Step 2 对它们的改动，只能用当前文件内容与本节记录核对；
`App.tsx` 可看到 diff，但其 diff 主要来自 OD-ARCH-026 的重写。

### 6.2 落盘证据

```
step2/scripts/step2-perf.py                     应用级落盘测量脚本（v1）
step2/scripts/step2-minibrowser-control.py      MiniBrowser 受控补充脚本（v1）
step2/step2-before-debug-vite.json + 6 张 .png  改前：主测量 + 逐页截图 + idle 起止帧
step2/step2-after-debug-vite.json  + 6 张 .png  改后：同上
step2/step2-before-minibrowser-xvfb-dmabuf1.json / step2-after-minibrowser-xvfb-dmabuf1.json
step2/calibration.json + calibration/*.png      Dock 坐标校准（8 项逐个截图）
baseline-pages/heavy-after.html                 复刻「改后 CSS」的对照页（原件 heavy/simple 未改）
```

可复现命令（与本次实际执行一致）：

```bash
# 应用级（debug + vite dev；每次自带一次性 Xvfb :91）
python3 tests/verification/desktop/evidence/od-arch-027/step2/scripts/step2-perf.py \
  --label step2-before-debug-vite --variant before --own-xvfb --display :91 \
  --root /tmp/od027-step2/before --idle-seconds 60 --repeats 5
# 改完 CSS 后同样命令，--label step2-after-debug-vite --variant after --root /tmp/od027-step2/after
# Dock 坐标校准（可选）
python3 .../step2-perf.py --calibrate --own-xvfb --display :91 --root /tmp/od027-step2/calib
# MiniBrowser 受控补充（一次性 Xvfb :92；DMABUF=1）
python3 .../step2-minibrowser-control.py --label step2-after-minibrowser-xvfb-dmabuf1 \
  --pages simple,heavy,heavy-after --own-xvfb --display :92 --seconds 40 --dmabuf 1
```

环境快照（两组应用级测量完全一致）：debug + vite dev；数据根 `/tmp/od027-step2/{before,after}`；
Xvfb `:91`（无窗口管理器，`GDK_BACKEND=x11`、清空 `WAYLAND_DISPLAY/SOCKET`）；
`WEBKIT_DISABLE_DMABUF_RENDERER=1`（AppSession 固定值，与 `scripts/dev-ui.sh` 默认一致；
Step 2 保持 DMABUF 默认，不与变量混同）；窗口 1280×820；空闲测量窗口 60.03 s ×2。
MiniBrowser 补充组：Xvfb `:92`，DMABUF=1，每页 40 s。

### 6.3 实测数字

**空闲 CPU（连续 60.03 s，`/proc/<pid>/stat` utime+stime 增量 ÷ 墙钟）**

| 进程 | before | after |
| --- | --- | --- |
| App 主进程 | 0.07% | 0.07% |
| **WebKitWebProcess** | **0.33%** | **0.07%** |
| WebKitNetworkProcess | 0.02% | 0.00% |
| WebKitGPUProcess | 无该进程 | 无该进程 |

**导航延迟（含 GDK 截图读回开销，只作相对参考；每段 n=5）**

| 路径段 | before P50 / P95 (s) | after P50 / P95 (s) |
| --- | --- | --- |
| 首页→任务工作台 | 2.945 / 3.268 | 2.134 / 2.145 |
| 任务工作台→离线（项目跟踪） | 1.921 / 3.235 | 1.713 / 1.728 |
| 离线→设置 | 1.720 / 1.735 | 2.546 / 2.583 |
| 设置→首页 | 1.719 / 1.731 | 2.562 / 2.574 |
| 全部 20 次 | 1.821 / 3.350 | 1.941 / 2.577 |

**MiniBrowser 受控补充（不替代应用级证据）**

| 页面 | 改前 CSS 组 | 改后 CSS 组 |
| --- | --- | --- |
| `simple.html`（无模糊、无动画） | 0.0% | 0.0% |
| `heavy.html`（模糊 + 动画叠加，Step 0 归档原件） | **100.2%** | **100.2%** |
| `heavy-after.html`（改后 CSS，动画保持不变） | — | **19.6%** |

### 6.4 说明与未验证项

1. **当前工作区的首页空闲 CPU 已经很低（0.33% / 0.07%），远低于计划 §5 的 <10% 目标。**
   计划 §1.2 记录的「约 100%」在本工作区**未复现**。两个可核对的事实：
   `animate-pulse-ambient` / `animate-float-mascot` 在 `theme.css` 中仍有定义，但当前 JSX
   **没有任何地方使用它们**；而同样用「模糊 + 无限动画」叠加的 `heavy.html` 仍稳定测得 100.2%。
   **推测**：改前的 ~100% 可能来自 OD-ARCH-026 之前的首页（当时背景光斑带无限脉冲动画）。
   **该推测未验证**：缺一次「动画单因素」对照，旧首页形态已不可取回，不作结论。
2. 因此 **before/after 的 CPU 差异（0.33% → 0.07%）在噪声量级**，不足以单独证明 Step 2 的收益；
   导航延迟 P50 在两次单次运行间甚至略升（1.821 → 1.941 s），波动大于差异。**不宣称性能提升**。
3. 受控补充显示：在**动画持续驱动重绘**的页面上，去掉 blur/glass 使 CPU 从 100.2% 降到 19.6%
   （同一 DMABUF 取值、同一显示、同一次运行内对照）。这说明 CSS 模糊/玻璃是放大器，但不是唯一来源；
   剩余的 19.6% 仍含保留的顶栏 `backdrop-blur`、两个无限动画与大面积重绘。
   `heavy-after.html` 只把 CSS 作为单因素，动画原样保留；**未做动画单因素对照**，故不能把剩余成本归给单一因素。
4. 视觉：逐页前后截图（首页/任务工作台/离线/设置）层级、对比度、布局一致，无黑屏/闪烁；
   背景光斑从「模糊实心块」变为「柔和光晕」，卡片玻璃感由半透明+阴影+描边表达。
   截图是 Xvfb 软件合成的静态帧，**未做真实桌面肉眼观感确认**（Step 5 范围）。
5. 未验证：真实 Wayland 桌面、真实显示器/窗口管理器、release + 内嵌产物构建、
   动画单因素 / 输入响应 / 设置页连续输入丢字（Step 4 范围）、以及第二次重复运行的稳定性
   （本步每个组合只跑了 1 次）。
6. 边界：只用合成端点与 `/tmp/od027-step2` 一次性数据根；未读凭证、未改 `runtime/`、
   未改模型路由、未碰第二 worktree `/mnt/data/Code/omniharness-ui`、未改 `scripts/dev-ui.sh` 默认值；
   未提交/推送/打标签。

---

## 7. Step 3：DMABUF 与显示后端独立对照（2026-10-19，BUILDER 执行记录 / 非审查结论）

> **零产品代码改动。** 本步只新增 `step3/` 下的脚本与证据，并更新本文件、`STATUS.md`、`PLANS.md`。
> 未改 `desktop/src/**`、`desktop/src-tauri/**`、`scripts/dev-ui.sh`、`runtime/`、Step 0 归档件与 Step 2 证据。
> `git status --porcelain` 条目 **保持 108 项不变**；`git diff --check HEAD` 干净；`runtime/` 无差异；
> `window_harness.py` **未改动**（其 DMABUF 默认值仍为 `1`）。

### 7.1 变量与配置

| 自变量 | 取值 | 说明 |
| --- | --- | --- |
| `WEBKIT_DISABLE_DMABUF_RENDERER` | `1`（现状默认，`scripts/dev-ui.sh:361`） / `0`（称「启用」） | 应用级通过 `AppSession.env_extra` 覆盖（该方法在 `app_env()` 里最后 `env.update(env_extra)`，无需改共享装置）；页面级直接设环境变量 |
| 显示后端 | **只有 `GDK_BACKEND=x11` + 一次性 Xvfb**（`:93` 页面级 / `:94` 应用级） | **Wayland↔XWayland 对照未做**：本沙箱是无 GPU 的一次性 Xvfb，且**不得由 BUILDER 连接/启动用户的真实 Wayland 会话**；该对照留待 Step 5 由用户在真实桌面执行。**本步没有产生任何 Wayland 数据，也没有伪造该组。** |
| 其它 | 与 Step 2 一致：debug + vite dev、1280×820、窗口停首页、合成端点、一次性数据根 | 每组注明窗口尺寸、DMABUF 取值、显示后端 |

每配置 **3 次独立运行**（A 组每页每档 3 次 = 12 次；B 组每档 3 次 = 6 次）；另有 4 次环境探针。

### 7.2 环境事实（实测，来自运行日志）

- WebKit2GTK **2.52.6**；`/dev/dri` 设备存在（`card0/card1/renderD128/renderD129`），
  但本用户 `groups` 里没有 `video`/`render`；Xvfb **没有 DRI3 扩展**。
- 四种探针处理的 MiniBrowser 标准错误里都出现：
  `libEGL warning: DRI3 error: Could not get DRI3 device` /
  `libEGL warning: Ensure your X server supports DRI3 to get accelerated rendering`
  → **该显示下 EGL/加速渲染不可用**。

### 7.3 A 组：页面级（MiniBrowser，决定性、不需构建）

同一 Xvfb `:93`、`heavy.html`/`simple.html`（Step 0 归档原件，未改写）、每次 30 s 采样。

| 页面 | `=1` WebKitWebProcess（3 次） | `=0` WebKitWebProcess（3 次） | WebKitGPUProcess | 黑屏 |
| --- | --- | --- | --- | --- |
| `simple.html` | 0.00 / 0.03 / 0.00 % | 0.00 / 0.00 / 0.00 % | **两次配置都没有** | 无 |
| `heavy.html` | 100.17 / 100.15 / 100.18 % | 100.22 / 100.26 / 98.76 % | **两次配置都没有** | 无 |

- 进程集合：两档（`=1`/`=0`）下都只有 1 个 `WebKitWebProcess` + 1 个网络进程；**没有 GPU 进程**，
  因此「DMABUF 打开后多出 GPU 进程」在本环境**未观察到**。
- 每次运行的 `/proc/<pid>/environ` 都核对到 `WEBKIT_DISABLE_DMABUF_RENDERER` 确为 `1`/`0`（变量确实传到了渲染进程）。
- 截图（整屏 1440×1000）经内容区判据分析，`heavy` 页内容区 `mean_luma≈240`、`dark_fraction≈0.002`，**无黑屏/空白**。

**结论（观测）：`=1` 与 `=0` 在本沙箱**完全等价**（重页都 ~100%，轻页都 ~0%）。**

### 7.4 B 组：应用级（真实 OmniDesk 窗口）

同一 Xvfb `:94`、debug + vite dev、1280×820、空闲窗口 60.05 s ×6、导航每段 n=5。

| 指标 | `=1`（run1/2/3） | `=0`（run1/2/3） |
| --- | --- | --- |
| 空闲 `WebKitWebProcess` CPU | 0.07 / 0.07 / 0.08 % | 0.07 / 0.08 / 0.08 % |
| 空闲 App 主进程 CPU | 0.10 / 0.08 / 0.10 % | 0.08 / 0.08 / 0.08 % |
| 网络进程 CPU | 0.00 % ×3 | 0.00 % ×3 |
| `WebKitGPUProcess` | **不存在 ×3** | **不存在 ×3** |
| 导航延迟 P50 / P95（全 20 次） | 1.737/2.578、1.940/2.563、1.726/2.562 s | 1.724/2.566、1.728/2.563、1.729/2.565 s |
| 设置页输入固定串（丢字检查） | **ok ×3**（均第 1 次尝试即逐字写入设置文件） | **ok ×3**（同） |
| 空闲 5 帧稳定性 | 1 / 2 / 2 个不同签名 | 2 / 1 / 1 个不同签名 |
| 黑屏/空白 | 无 | 无 |
| `/proc/<pid>/environ` 核对 | app=`1`、WebKitWebProcess=`1` | app=`0`、WebKitWebProcess=`0` |

- 两档在 CPU、导航延迟、输入响应、进程集合、画面状态上**没有可辨差异**。
- 稳定性：6 次运行中有 3 次出现「2 个不同签名」，但内容区 `mean_luma` 恒为 245.67、
  仅 1–2 个抽样色彩数不同 —— 与顶栏状态点的 `animate-pulse` 一致，**不是黑屏/闪烁**（该动画属 Step 4 范围）。
- 输入测试用的是 `run_window_acceptance` 的主按钮定位 + `type_text`（分块 4 字符）节奏，
  逐次核对设置文件是否**逐字**包含所输入路径（`/tmp/od027s3/<tag>`）。6 次都一次命中，未观察到丢字。

### 7.5 环境探针（受控补充，非应用级证据）

`heavy.html`、同一显示、每处理 1 次（30 s），**保留 MiniBrowser 标准错误**：

| 处理 | `WEBKIT_DISABLE_DMABUF_RENDERER` | WebKitWebProcess | GPU 进程 | 绘制推进（30 帧/约 4 s） |
| --- | --- | --- | --- | --- |
| `unset` | 完全不设 | 99.91%（首轮 100.19%） | 无 | 5/29 帧对变化，mean_delta 0.00277 |
| `0` | `=0` | 99.98%（首轮 100.19%） | 无 | 5/29，0.00212 |
| `1` | `=1` | 100.12%（首轮 100.16%） | 无 | 6/29，0.00320 |
| `force` | `WEBKIT_FORCE_DMABUF_RENDERER=1` | **62.07%**（首轮 60.27%） | 无 | **29/29，0.01112** |

（两轮独立探针结果一致，故上表给两次数值。）

**这说明：**
1. 在本环境里 `WEBKIT_DISABLE_DMABUF_RENDERER` **无论设 `0`、`1` 还是不设，都没有可观测差异**
   —— 它不是本沙箱里可控的自变量（很可能因为 DMABUF 渲染路径在无 DRI3/EGL 的 Xvfb 上根本不会启用）。
   因此 **7.3/7.4 的「无差异」是构造性的空结果，不能读成「DMABUF 在真实桌面不重要」。**
2. 真正能改变代码路径的是 `WEBKIT_FORCE_DMABUF_RENDERER=1`（CPU 100%→62%，页面渲染结果一致，
   无黑屏/损坏帧）。**但低 CPU 不等于更好**：该「绘制推进」指标是近似量（抓图有开销），
   只能排除「帧数为 0/停摆」，不能证明画面更流畅；撕裂、局部更新等未验证。
   **不得据此判定为改进**（计划 §5「不得仅凭 CPU 下降判定成功」）。
3. 计划 §1.2 记录的「同一特效页 DMABUF 打开 34.5%」**在本环境未复现**（三次探针都为 ~100% 或 ~62%）。
   原因未验证；可能与当时环境/WebKit 版本或测量方式不同有关。

### 7.6 画面异常汇总

`step3/scripts/step3-analyze-frames.py` 对 step3 下**全部 82 张截图**（A 组 12 + 探针 4 + B 组 66）
按「非暗像素包围盒」重算内容区判据：**疑似黑屏/空白 0 张，疑似闪烁 0 张**。
逐帧结果见 `step3/step3-frame-analysis.json`（含每个文件的 `bbox`、内容区 `mean_luma`、`luma_std`、
`dark_fraction`、抽样色彩数与判定）。A 组截图整屏暗像素占比恒为 0.455，来自一次性 Xvfb 上**未使用的黑色桌面**，
非页面内容。

### 7.7 落盘证据与可复现命令

```
step3/scripts/_step3lib.py              进程 CPU / 一次性 Xvfb / 画面判据 / 绘制推进
step3/scripts/step3-pages.py            A 组页面级对照
step3/scripts/step3-app.py             B 组应用级对照（含输入响应）
step3/scripts/step3-dmabuf-probe.py     环境探针（unset / 0 / 1 / force）
step3/scripts/step3-analyze-frames.py   全量截图异常判据汇总
step3/step3-A-{simple,heavy}-dmabuf{0,1}-run{1..3}.json + .png   12 组
step3/step3-A-summary-dmabuf{0,1}.json
step3/step3-B-dmabuf{0,1}-run{1..3}.json                         6 组
step3/step3-B-summary-dmabuf{0,1}.json
step3/step3-probe-{unset,0,1,force}.{json,log,png} + step3-probe-summary.json
step3/step3-frame-analysis.json         82 张截图的异常判据汇总
step3/shots/                            B 组逐页/空闲/稳定性截图
```

```bash
S=tests/verification/desktop/evidence/od-arch-027/step3/scripts
# A 组（Xvfb :93，页面级，每档 3 次 × 30 s）
python3 $S/step3-pages.py --pages simple,heavy --dmabuf 1 --runs 3 --seconds 30 --own-xvfb --display :93
python3 $S/step3-pages.py --pages simple,heavy --dmabuf 0 --runs 3 --seconds 30 --own-xvfb --display :93
# 环境探针
python3 $S/step3-dmabuf-probe.py --own-xvfb --display :93 --seconds 30
# B 组（Xvfb :94，应用级，每档 3 次 × 60 s 空闲 + 导航 + 输入）
python3 $S/step3-app.py --dmabuf 1 --runs 3 --display :94 --own-xvfb --input-test
python3 $S/step3-app.py --dmabuf 0 --runs 3 --display :94 --own-xvfb --input-test
# 全量截图判据
python3 $S/step3-analyze-frames.py
```
（每脚本自带一次性 Xvfb 的启动与回收；运行前后都校验系统上只有本次这一个 WebKit 应用。）

### 7.8 说明与未验证项

1. **本步不能给出「DMABUF 该开还是该关」的结论。** 7.5 已说明该变量在本沙箱**不可控**，
   7.3/7.4 的无差异是构造性的空结果。真实价值需在**有 GPU 的真实桌面**上复测（Step 5）。
2. **黑屏未出现**（82 张截图 0 异常），但这也**不能**说明真实桌面开 DMABUF 不会黑屏
   （本环境根本没走 DMABUF 路径）。`scripts/dev-ui.sh:361` 默认值**未改**，
   拟改默认值须单独批准并保留旧默认（计划 §4 Step 3 回退条款）。
3. B 组测的是**当前工作树**（含 Step 2 的 CSS 改动，未回退）。因此本步绝对值**不可与 Step 2 之前的数据直接比较**；
   B 组内部的 `=1`/`=0` 比较仍是单变量比较。
4. 近似量：导航延迟含 GDK 截图读回开销；「绘制推进」指标抓图有开销，只能排除停摆。
5. 未验证：Wayland↔XWayland 与真实显示器/窗口管理器；release + 内嵌产物构建；
   真实 GPU 下的 DMABUF 行为与黑屏风险；`WEBKIT_FORCE_DMABUF_RENDERER` 的观感/撕裂；
   跨会话长时间稳定性；真实模型与真实材料（本步只用合成端点与一次性数据根）。
6. 边界：未读凭证、未改 `runtime/`、模型路由、产品代码与 `dev-ui.sh` 默认值；
   未碰第二 worktree；未提交/推送/打标签。

---

## 8. Step 4：无限动画与 React 重渲染治理（2026-10-19，BUILDER 执行记录 / 非审查结论）

> 本步含 **Phase A（诊断，不改产品代码）** 与 **Phase B（最小治理改动）**。
> 未改 `runtime/`、Rust（`desktop/src-tauri/**`）、`scripts/dev-ui.sh`、Step 0 归档件、Step 2/3 证据。
> `git status --porcelain` 条目**保持 108 项不变**（新增文件都在原本已未跟踪的目录内）；
> `git diff --check HEAD` 干净；`runtime/` 无差异；无 `.rs` 改动（最新 mtime 14:59，早于本会话）。

### 8.1 Phase A：无限动画 × 大面积模糊「单因素对照」（受控页面证据）

`baseline-pages/` **新增** 6 个同构对照页（`generate-iso-pages.py` 生成；`simple.html`/`heavy.html` 原件
未改写，sha256 与 Step 2/3 一致）。所有页面**布局/尺寸/颜色完全相同**，只切换两个因素；
MiniBrowser、一次性 Xvfb `:95`、`WEBKIT_DISABLE_DMABUF_RENDERER=1`、每次 **30 s**、每页 **3 次**（共 18 次）。

| 页面 | 模糊/玻璃 | 无限动画 | WebKitWebProcess（3 次） | 均值 | GPU 进程 | 绘制推进 |
| --- | --- | --- | --- | --- | --- | --- |
| `iso-blur-only` | ON | 无 | 0.00 / 0.00 / 0.00 % | **0.00%** | 无 | 0/29 |
| `iso-anim-only` | OFF | pulse + float | 21.45 / 23.32 / 33.24 % | **26.00%** | 无 | 29/29 |
| `iso-anim-blur` | ON | pulse + float | 94.90 / 100.28 / 100.25 % | **98.48%** | 无 | 6/29 |
| `iso-anim-pulse-only` | OFF | 仅 pulse | 23.55 / 21.32 / 22.22 % | **22.36%** | 无 | 29/29 |
| `iso-anim-float-only` | OFF | 仅 float | 23.61 / 23.38 / 23.31 % | **23.43%** | 无 | 29/29 |
| `iso-navpulse` | ON | **仅一个 6×6 状态点脉冲** | 100.21 / 100.15 / 100.22 % | **100.19%** | 无 | 9–10/29 |
| （参照）`heavy.html`（Step 3，同 DMABUF） | ON | pulse + float | 100.17 / 100.15 / 100.18 % | 100.17% | 无 | — |

`iso-anim-blur` 的样式块与 `heavy.html` 逐行相同（仅少 3 行注释），实测 98.48% vs 100.17%，互为交叉验证。

**归因（受控页面证据，不直接等于真实应用）：**

1. **模糊/玻璃本身不贵**：全部静态模糊层（三个全屏 `blur(120–140px)` + `backdrop-filter blur(40px)` 顶栏与 Dock
   + 5 个 `blur(20px)` 面板 + `blur(28px)` plate）= **0.00%**，绘制推进 0/29（画面完全静止）。
2. **只有动画、没有模糊 ≈ 22–26%**：单个动画（pulse 22.36% 或 float 23.43%）就足以持续重绘；
   两个同时开 26.00%，**不是简单相加**（合并进同一合成流程），关键变量是「有没有持续动画」。
3. **动画 × 模糊 = 98–100%**：每个动画帧都要重新执行大面积模糊/背景模糊 → 吃满一个核。
4. **最关键的对照 `iso-navpulse`**：与 `iso-blur-only` **唯一差别**是加了一个 **6×6 px 的脉冲小圆点**，
   代价从 0.00% 变成 **100.19%**。也就是说——
   **「一个极小元素 + 大面积模糊」就能吃掉一个核；成本来自「持续重绘 × 模糊层」，与动画元素大小无关。**
   这解释了原计划记录的「空闲 ~100%」，也解释了 Step 3 里 `heavy.html` 为什么在 100% 时 X 可见帧反而更少（6/29）。

### 8.2 应用级：顶栏状态点曾是当前产品里唯一在跑的无限动画（→ 已按计划 §5 目标改为默认静态）

`theme.css` 里的 `.animate-pulse-ambient` / `.animate-float-mascot` **在 JSX 里没有任何使用**；
真正在跑的是 `shell/TopNav.tsx` 顶栏状态点的 Tailwind `animate-pulse`（2 s 无限，`engineHealthy` 为真时）。
而顶栏保留着 Step 2 唯一留下的 `backdrop-blur-2xl`（Phase A 的单因素对照指向的正是这种组合）。

为了测到真实产品，`step4-app.py --engine-healthy` 在**启动前**把 `model_permissions.local = true`
预写入隔离数据根，使启动期准入通过、`engineHealthy === true`、状态点真的在脉冲
（用截图里状态点色相判定 emerald/amber，并核对截图变化）。

| 运行（debug+vite，Xvfb `:96`，DMABUF=1，空闲 60 s 以上） | 状态点 | WebKitWebProcess | App（UI 进程） | 网络 | GPU | 5 帧签名 |
| --- | --- | --- | --- | --- | --- | --- |
| `step4-B-before-engine-healthy-run1`（历史：只加了类，仍脉冲） | emerald **脉冲** | **18.82%** | 10.26% | 0.00% | 无 | 5/5 变化 |
| `step4-B-after-engine-healthy-run1`（历史：仍脉冲） | emerald **脉冲** | **18.47%** | 10.32% | 0.00% | 无 | 5/5 变化 |
| `step4-B-after-reduced-motion-run1`（历史：`prefers-reduced-motion`） | emerald（不脉冲） | **0.10%** | 0.13% | 0.00% | 无 | **1/1** |
| `step4-C-default-run1`（历史：小步改动后、权限关闭） | amber 静态 | **0.05%** | 0.10% | 0.00% | 无 | — |
| **`step4-D-final-engine-healthy-run1`（最终：默认静态 + 引擎健康）** | **emerald 静态** | **0.10%** | **0.12%** | 0.00% | 无 | **1/1** |
| **`step4-D-final-default-run1`（最终：出厂默认，权限关闭）** | amber 静态 | **0.03%** | **0.10%** | 0.02% | 无 | — |

（App 进程即 WebKitGTK 的 UI 进程，重绘调度也在其中，所以它随状态点一起升高。）

**最终改动的 60 s 曲线（`step4-D-final-engine-healthy-run1`，每 5 s 一点）：**

- `WebKitWebProcess`：0.60 → 0.30 → 0.20 → 0.15 → 0.12 → 0.10 → 0.17 → 0.15 → 0.13 → 0.12 → 0.11 → **0.10%**
  （启动瞬态之后稳定在 0.10–0.17%，**无持续重绘**，5/5 帧签名相同 = 1/1）
- App（UI 进程）：0.40 → 0.20 → 0.13 → 0.10 → 0.08 → 0.10 → 0.11 → 0.12 → 0.13 → 0.14 → 0.13 → **0.12%**
- 出厂默认：`WebKitWebProcess` 全程 0.00–0.04 → **0.03%**；App **0.10%**

**读法：**
- **满足计划 §5 目标（空闲稳定后 60 s 平均 <10%）**：引擎健康场景 **0.10%（渲染）/ 0.12%（UI）**，
  出厂默认 **0.03% / 0.10%**；两者均无 GPU 进程、无网络活动。
- 状态点**颜色语义保留**：健康 = emerald `LOCAL IPC ACTIVE`、非健康 = amber `ENGINE OFFLINE`
  （截图裁剪：`shots-step4-D-final-engine-healthy-run1/...-chip-6x.png` 与 `shots-step4-D-final-default-run1/...-chip-6x.png`）。
  位置、尺寸（6×6）、布局与文案均未改。
- **历史数字保留**：脉冲时 18.82%（改动前）/ 18.47%（仅改类名后）/ reduced-motion 0.10% / C 默认 0.05%。
  即「此前 ~29% 的合计持续占用」完全来自那一个脉冲动画；关掉后渲染进程降到 1/100 量级。

> **改动与回退（有意的一行视觉改动）：** `desktop/src/theme.css` 中
> `.status-dot-pulse { animation: none; }`（键帧 `status-dot-pulse` 保留，供将来显式启用）。
> 恢复脉冲只需把该行改回 `animation: status-dot-pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;`，
> 代价是引擎健康场景回到 ~18.8% 的持续渲染占用。`shell/TopNav.tsx` 本轮**未再改动**（仍按
> `engineHealthy` 着色为 emerald/amber），`@media (prefers-reduced-motion: reduce)` 守卫保留。

### 8.3 Phase B：逐项「是否复现 / 是否改动 / 为什么最小」

| 项 | 复现情况 | 本轮改动 | 为什么最小 |
| --- | --- | --- | --- |
| 1a `.animate-pulse-ambient` / `.animate-float-mascot` | **当前 JSX 未使用**（`grep` 全 `desktop/src` 仅 `theme.css` 有定义）；但 Phase A 证明这类动画一旦用在模糊背景上就是 100% | `theme.css`：`@media (prefers-reduced-motion: reduce)` 下 `animation: none` | **防御性**改动（防将来重新引入），4 行 CSS；不删类、不删键帧 |
| 1b 顶栏状态点 `animate-pulse` | **在用**（`engineHealthy` 时），且实测 18.8%（合计约 29%），**不满足 §5 的 <10%** | ① 由 Tailwind `animate-pulse` 改为等价本地类 `.status-dot-pulse`；② **最终**把该类默认改为 `animation: none`（颜色语义保留，键帧保留） | 一行 CSS、可一行回退；不改位置/尺寸/布局/文案，不改 `TopNav.tsx` 逻辑 |
| 2 `TopNav` 每 30 s `setInterval` | 存在；但**只重渲染 TopNav 子树**，不是整树，且 30 s 一次，对 60 s 均值无可测影响 | 抽出 `NavClock` 组件自持时钟状态；`TopNav` 变为无状态，tick 只重渲染时钟 | 组件内抽取、显示内容与周期不变；不改 props/结构语义 |
| 3 `SettingsPanel` 输入丢字 | **未复现**：设置页输入固定串，`fast`（`type_ascii` settle=0.02，约 25–30 字符/秒，本装置上限）与 `slow`（分块 4 + 0.6 s 停顿）**都逐字写入设置文件**（`exact_hit`） | **未改** | 复现不了就不改。整面板重渲染在 Step 2 后该页已无 `backdrop-filter`，成本很低；无证据支持的架构改动会扩大风险 |

`SettingsPanel.tsx` **未被修改**（mtime 仍是 2026-10-18 20:12）。

### 8.4 验证（本轮实际运行）

| 检查 | 结果 |
| --- | --- |
| `npx tsc --noEmit` | **0** |
| `npm run build` | **通过**（最终：`index-DfhCJBA6.css` 70.79 kB / `index-D3B_kTff.js` 331.78 kB，526 ms） |
| 构建产物核对 | `.status-dot-pulse{animation:none}`（默认静态）；`@media (prefers-reduced-motion:reduce){.animate-pulse-ambient,.animate-float-mascot,.status-dot-pulse{animation:none}}` 仍在；构建产物中**剩下的 `infinite` 只有两个未被 JSX 使用的动画**（`pulse-subtle` / `float-companion`）；**无独立 `.animate-pulse{` 规则** |
| 真实窗口冒烟（最终） | 一次性 Xvfb + `GDK_BACKEND=x11` + 隔离数据根；首页对话中枢与 4 个页面逐页截图，`verdict=ok`，无黑屏/空白（`mean_luma` 243.78–246.39，`dark_fraction` ≤0.0072） |
| 顶栏状态点 | `step4-D-final-engine-healthy-run1` 裁剪顶栏：emerald 圆点 + `LOCAL IPC ACTIVE`；`step4-D-final-default-run1`：amber 圆点 + `ENGINE OFFLINE`。位置/尺寸/布局不变 |
| 顶栏时钟（`NavClock` 抽取） | 截图仍显示「10月19日星期一 · 19:12」，格式/位置不变 |
| 设置页输入（`step4-C-default-run1`） | `fast` 与 `slow` 均 `exact_hit`（逐字） |
| 应用级空闲 CPU：引擎健康 + 默认静态（最终） | `WebKitWebProcess` **0.10%**、App **0.12%**、网络 0.00%、**无 GPU 进程**（60.07 s；曲线 0.60→0.10） |
| 应用级空闲 CPU：出厂默认（权限关闭，最终） | `WebKitWebProcess` **0.03%**、App **0.10%**、网络 0.02%、无 GPU 进程（60.07 s） |
| 导航延迟（引擎健康，最终） | 全 20 次 **P50 2.548 s / P95 2.597 s**（0 超时） |
| 导航延迟（出厂默认，最终） | 全 20 次 **P50 2.567 s / P95 2.593 s**（0 超时） |
| 与 Step 3 截图对比 | `任务工作台` 仅时钟区域不同（x 1017–1043, y 39–46）；`首页` 差异仅在顶栏+问候语（时间不同）；`离线`/`设置` 的差异来自页面显示的一次性数据根路径。**未发现布局/样式回退** |

> **导航延迟口径限制（必须如实标注）：** 该指标用「点击后轮询整窗截图直到内容变化」测量，单次整窗截图
> 读回本身约 2.5 s，因此**读数在 ~2.5 s 处饱和**（Step 2/3 的 P95 也正好在 2.56–2.58 s）。
> 本轮两次运行的 P50 都贴近该天花板（2.548 / 2.567），而 Step 2 的 P50 为 1.941 s ——
> 这**不能**解读为导航变慢或变快，只能说明本轮更多样本落到了截图读回的天花板上（与机器负载有关）。
> 该指标只适合同口径粗比，不具备亚秒级分辨力；**本轮不据此下任何性能结论**。

### 8.5 落盘证据与可复现命令

```
step4/scripts/generate-iso-pages.py     生成 6 个单因素对照页（唯一生成来源）
step4/scripts/step4-iso.py              Phase A 测量（MiniBrowser，复用 step3/_step3lib 只读导入）
step4/scripts/step4-app.py              Phase B 应用级（含 --engine-healthy / --reduced-motion）
step4/step4-A-iso-<page>-run{1..3}.json + .png     18 组页面级运行
step4/step4-A-iso-summary.json          由各次落盘 JSON 汇总（6 页各 3 次）
step4/step4-B-{before-engine-healthy,after-engine-healthy,after-reduced-motion}-run1.json   历史
step4/step4-C-default-run1.json         历史：默认（权限关闭）应用级 + 输入测试
step4/step4-smoke-engine-healthy.json   10 s 冒烟（已被 60 s 运行取代，仅留痕）
step4/step4-D-final-engine-healthy-run1.json   最终：默认静态 + 引擎健康，60 s + 导航
step4/step4-D-final-default-run1.json          最终：出厂默认，60 s + 导航 + 四页冒烟
step4/shots-<label>/                    逐页/输入/顶栏截图（含 `-chip-6x.png` 状态点放大）
baseline-pages/iso-*.html + iso-pages-manifest.json
```

```bash
S=tests/verification/desktop/evidence/od-arch-027/step4/scripts
python3 $S/generate-iso-pages.py
# Phase A（Xvfb :95，每页 3 次 × 30 s，DMABUF=1）
python3 $S/step4-iso.py --pages iso-blur-only,iso-anim-only,iso-anim-blur,iso-anim-pulse-only,iso-anim-float-only \
  --runs 3 --seconds 30 --own-xvfb --display :95
python3 $S/step4-iso.py --pages iso-navpulse --runs 3 --seconds 30 --own-xvfb --display :95
# Phase B（Xvfb :96）最终复测
python3 $S/step4-app.py --label step4-D-final-engine-healthy-run1 --engine-healthy --idle-seconds 60 --settle 14 --own-xvfb --display :96
python3 $S/step4-app.py --label step4-D-final-default-run1 --idle-seconds 60 --settle 12 --own-xvfb --display :96
# 历史（保留可复现）
python3 $S/step4-app.py --label step4-C-default-run1 --input-test --own-xvfb --display :96
python3 $S/step4-app.py --label step4-B-after-engine-healthy-run1 --engine-healthy --settle 14 --own-xvfb --display :96
python3 $S/step4-app.py --label step4-B-after-reduced-motion-run1 --engine-healthy --reduced-motion --settle 14 --own-xvfb --display :96
```

### 8.6 未验证项与限制

1. **导航延迟无可用的亚秒分辨力**（见 §8.4 末注）：整窗截图读回约 2.5 s，读数在 ~2.5 s 饱和；
   本轮不作为性能结论依据。要拿到真实导航耗时需改测量方式（例如页面内计时或 CDP），属后续工作。
2. 「状态点导致 18.8%」的归因依据是：唯一差异是脉冲动画（颜色/文案只变一次），reduced-motion 关掉脉冲后
   降到 0.10%，**最终默认静态后同场景 0.10%**。**没有**做「同一 healthy 状态下仅隐藏状态点元素」的变体；
   但 `iso-navpulse` 的受控对照（0.00% → 100.19%，唯一差别是一个 6×6 脉冲点）独立支持该机制。
3. 输入的「快速注入」受装置限制：`XSession.key()` 固定 press→release 间隔 0.03 s，实际约 25–30 字符/秒，
   **无法构造更快的注入**（未改共享装置）。因此「更极端速度下是否丢字」未验证；本文只报告该速度下未丢字。
4. Phase A 是**受控页面**，不等于真实应用；真实应用只有顶栏一个保留的模糊层，所以应用级代价（历史 18.8%）
   低于受控页的 100%，两者不可直接互换。
5. 未验证：真实 Wayland 桌面与真实 GPU/显示器；release + 内嵌产物；真实用户的速度/输入法；
   `prefers-reduced-motion` 在真实桌面（GNOME「减少动画」开关）下是否同样生效（本步只在 Xvfb 用
   `gtk-enable-animations=0` 验证了映射）；长时间稳定性（本轮最长 60 s 连续采样）。
6. 「默认静态」是有意的视觉改动：状态点不再脉冲，但颜色语义、位置、尺寸、布局与文案不变，
   可一行回退（见 §8.2 末注）。若产品后续希望保留「活着」的动态感，建议改为**状态变化时的一次性高亮**
   而不是无限脉冲。
7. 边界：只用合成端点与 `/tmp/od027-step4` 一次性数据根（`model_permissions.local=true` 只写在该一次性根内，
   不涉及用户真实配置）；未读凭证、未改 `runtime/`/模型路由/Rust/`dev-ui.sh`；未碰第二 worktree；未提交/推送/打标签。




## 9. Step 5：真实桌面回归（2026-10-20，ARCHITECT 执行记录 / 非审查结论）

> 在**用户自己的 Wayland 会话**（`XDG_SESSION_TYPE=wayland`、`WAYLAND_DISPLAY=wayland-0`、GNOME 50.1）
> 里跑真实 OmniDesk 窗口。**四组分别记录，不混为一组**：原生 Wayland × DMABUF{1,0}、XWayland × DMABUF{1,0}。
> 脚本：`step5/scripts/step5-real-desktop.py`；数据：`step5/step5-*.json`；截图：`step5/shots-*/`。

### 9.1 测量通道与为什么这样分

- **原生 Wayland 组**：`GDK_BACKEND=wayland`、清空 `DISPLAY`，应用**不经过 XWayland**。
  本机原生 Wayland **没有可用的截图通道**（实测：GNOME Shell `ScreenshotArea` D-Bus 返回
  `AccessDenied: ScreenshotArea is not allowed`；`org.gnome.Shell.Eval` 返回 `(false, '')`；
  用 GDK 抓 X 根窗口得到**全黑**图）。因此该组**不产出像素证据**，改用 **AT-SPI**（可访问性总线）
  驱动界面并以**可访问性文本指纹**判定页面切换。
- **XWayland 组**：`GDK_BACKEND=x11` + `DISPLAY=:0`（用户的 XWayland），窗口真实显示在桌面上。
  像素用 `XGetImage` 读**应用窗口自身**（读根窗口同样是全黑），输入用 XTEST，交互用 AT-SPI。
- 两组都用**一次性隔离数据根**（`/tmp/od027-step5/...`）+ 回环合成端点，预置
  `model_permissions.local=true`（使顶栏状态点处于「引擎健康」这一最坏情形）。**未写 `~/OmniDesk`**，
  未使用真实材料或凭证。

### 9.2 实测结果（每组：空闲 60 s + 导航延迟 4 页 × 5 次循环 + 5 帧稳定性 + 输入校验）

| 组 | 显示后端 | DMABUF | 空闲 `WebKitWebProcess` 均值 | App 主进程均值 | 导航 P50（最小–最大） | 导航 P95（最大） | 超时 | 5 帧签名 | 连续输入 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `step5-wayland-dmabuf1` | 原生 Wayland | 1 | **0.08%** | 0.07% | 0.45–1.18 s | 1.22 s | 0 | 未测（无像素通道） | 未测（无合成输入通道） |
| `step5-wayland-dmabuf0` | 原生 Wayland | 0 | **0.17%** | 0.08% | 0.40–1.22 s | 1.63 s | 0 | 未测 | 未测 |
| `step5-xwayland-dmabuf1` | XWayland | 1 | **0.15%** | 0.12% | 0.31–0.77 s | 0.85 s | 0 | **1**（无持续重绘） | **exact_hit** |
| `step5-xwayland-dmabuf0` | XWayland | 0 | **0.28%** | 0.12% | 0.32–0.81 s | 0.85 s | 0 | **1** | **exact_hit** |

- **空闲 CPU 目标（§5：稳定后 60 s 平均 < 10%）在真实桌面达成**，四组均在 **0.08%–0.28%**，
  比目标低两个数量级；原生 Wayland 的 60 s 曲线稳定在 0.00–0.09%（`step5-wayland-dmabuf1.json`）。
  **没有 WebKitGPUProcess**（本机无 DRI3/无 GPU 加速渲染）。
- **连续输入无丢字**：设置页「成果目录」输入框注入固定串（一个**合法绝对路径**）后点「校验并保存」，
  设置文件**逐字包含**该串（`status=exact_hit`）。此前用非路径字符串会被产品**正确拒绝**
  （必须是绝对路径），这属于产品行为，不是丢字。
- **视觉**：`shots-step5-xwayland-dmabuf1/` 四页截图为**真实桌面**像素（均值亮度 244.9–247.4，
  非黑像素占比 1.00），首页与用户 2026-10-19 确认的 UI 基线一致（frameless 外框 + 机器人 +
  今日重点推进 + 日程节律 + 全宽对话 + 底部权限/Token 行）。
- **DMABUF 启用/禁用分别记录**：在真实桌面同样不改变量级（0.15% vs 0.28%、0.08% vs 0.17%），
  与 Step 3 在本沙箱「该变量不可控」的结论一致；**不能据此说 DMABUF 在真实桌面重要或不重要**，
  只能说本机（无 GPU）两种取值都没有出现持续占用。

### 9.3 说明与未验证项

1. **原「空闲 ~100%」症状在真实桌面同样没有复现**（0.08%–0.28%）。Step 2/4 的结论保持不变：
   当前工作区的首页**没有**持续动画与大面积模糊叠加，症状不复现；**不得**据此宣称本轮带来了性能提升。
2. **原生 Wayland 组的像素与输入指标缺失**是环境限制（截图 D-Bus 被拒、无合成输入工具），不是漏测；
   对应的指标由 XWayland 组单独提供，两组不混算。
3. **未验证**：release 内嵌产物（仍是 debug + vite dev）；真实模型质量；多显示器/缩放/HiDPI；
   长时间（>60 s）稳定性；真实用户的输入法与粘贴；GNOME「减少动画」开关在真实桌面下的映射效果。
4. 本组**只读测量**，不改任何产品代码；窗口在用户桌面上真实显示，测量结束后已关闭。
