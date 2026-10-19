# OD-ARCH-026 证据索引：AI 对话为中枢的产品校正与三项工作场景闭环

> 本文件是 OD-ARCH-026 的**单一证据索引**。它记录本轮实际运行了什么、观察到什么、哪些没验证，
> 供 ARCHITECT 独立复核定位证据；**不构成审查结论**。
>
> 开工 HEAD：`5ffc7101e90dcf3a2047f214b71286d3a32208a8`（`main`）。未提交、未推送、未打标签。
> **2026-10-19 追加**：首页版式回归（对话为主 + 顶部三模块）见 **§4**；仍属 OD-ARCH-026，未另开编号。
> 未改动 `runtime/`、模型路由、凭证、真实材料、用户数据与第二 worktree（`/mnt/data/Code/omniharness-ui`）。

## 0. 范围与边界

- 计划：`PLANS.md` 的 OD-ARCH-026（用户批准）。产品形态校正：首页以对话为中枢；左侧进入周报与随记、
  文案写作、项目跟踪；内容整理与起草由模型完成，代码只做确定性工作与文件边界控制。
- 全部验证使用**合成/虚构内容**与**本机回环合成端点**（`_mock_model_server.py`，零凭证、零外网）。
- 真实窗口验证在**一次性 Xvfb 显示**（`:96`）与**一次性隔离数据根**（`/tmp/od26/...`）中进行；
  成果目录预置为 `/tmp/od26/artifacts*`，未写 `~/OmniDesk`。未截取用户桌面。
- 模型权限在一次性数据根中预置 `local=true`（一次性合成端点）。权限 UI 与宿主的一致性已由
  OD-ARCH-025 阶段 A 验证；本轮首页在**权限关闭**时的真实状态见 `shots/00-home.png`。

## 1. 改动清单（产品源码）

| 层 | 文件 | 内容 |
| --- | --- | --- |
| 前端 | `desktop/src/shell/HomeChat.tsx`（新增） | 首页对话：经 IPC-1 提交、流式事件、终态轮询、取消；准入状态由 `engine_admission_status` 驱动 |
| 前端 | `desktop/src/shell/HomeDashboard.tsx` | 首页改为对话中枢（左侧主要对话区）+ 三个场景卡片 + 版本边界 |
| 前端 | `desktop/src/shell/Dock.tsx` | 新增周报与随记 / 文案写作 / 项目跟踪三个入口 |
| 前端 | `desktop/src/App.tsx` | 场景接线：report/writing/project → 工作场景页（按入口进入对应场景） |
| 前端 | `desktop/src/OfflineFlows.tsx` | 三场景改为模型起草：输入 + 材料文件选择 + 可编辑流程包规则；确定性核对单独成栏且不参与保存；成果仅来自模型初稿 |
| 前端 | `desktop/src/aiDraft.ts` | 提示词加入所选材料（名称+sha256）与流程包规则；不可关闭的写作规则；可传 workspaceDir |
| 前端 | `desktop/src/api.ts`、`types.ts` | `readMaterial` 客户端与类型 |
| 宿主 | `desktop/src-tauri/src/task_shell.rs` | 新增 `read_material`：只读用户显式给出的**单个**文件（绝对路径、普通文件、扩展名白名单、512 KiB 上限、UTF-8；拒绝 DSH/agent/仓库路径） |
| 宿主 | `desktop/src-tauri/src/flow_pack.rs` | 流程包新增可选 `draft.rules/sections`（可编辑场景规则；旧包缺省回落内置默认；含纯文本校验） |
| 宿主 | `desktop/src-tauri/src/lib.rs` | 注册 `task_read_material` |
| 样例 | `samples/workflows/offline-flows.v1.json` | 三类场景默认规则与输出章节 |
| 文档 | `GOALS.md`、`docs/decisions.md`（D-019） | 产品形态与模型权限边界 |
| 测试 | `tests/verification/desktop/verify_scenario_materials.py`（新增）、`verify_ai_draft.py`（扩充） | 见 §2 |

## 2. 复现命令与实际结果

### 2.1 前端

```
cd desktop && npx tsc --noEmit           # EXIT=0
cd desktop && npm run build              # ✓ built（index-*.js 331.98 kB / gzip 99.54 kB）
python3 tests/verification/desktop/verify_ai_draft.py
#   compile_exit=0 probe_exit=0 cases=27 failed=0  RESULT: pass
#   （新增用例：材料名称/正文/sha 进入提示词；可编辑场景规则与章节；流程包不可关闭的写作规则；
#     材料正文进入红线扫描目标而规则文本不进入；workspaceDir 透传）
```

### 2.2 宿主（Rust，当前源码新编译）

```
cd desktop/src-tauri && cargo test --offline
#   running 124 tests  -> 124 passed; 0 failed      （lib 单元测试，含 read_material 6 项与流程包 draft 2 项）
#   running   2 tests  ->   2 passed
#   running  13 tests  ->  13 passed                （host_integration）
#   running   3 tests  ->   3 passed                （task_shell_e2e）
#   合计 142 passed / 0 failed
```
环境：`scripts/setup-desktop-toolchain.sh --print-env` 给出的 PATH/PKG_CONFIG_*/LIBRARY_PATH。

### 2.3 引擎层：三类场景 + 所选材料（新增脚本）

```
python3 tests/verification/desktop/verify_scenario_materials.py --json tests/verification/desktop/evidence/od-arch-026/scenario-materials.json
#   prompts=3 scenarios=5 failed=0   RESULT: pass
```
- 提示词由**真实源码** `desktop/src/aiDraft.ts` 的编译产物生成（不是脚本自己拼的）；三类场景各一份。
- 三类场景（`weekly_report` / `material_writing` / `project_management`）各跑一次：
  终态 `succeeded`、合成端点**收到请求**且**材料标记命中**（材料正文进入模型请求）、结果回填可见。
- `error_400`：终态 `failed`，材料仍进入请求，不伪报成功。
- `hang + cancel`：取消语义正确（`confirmed_cancelled`、未伪报成功）；取消发生在模型请求之前，故该路径不要求 probe 命中。

### 2.4 真实窗口（一次性 Xvfb + 隔离数据根 + 回环合成端点）

1. 冒烟（`smoke.json` + `shots/00-home.png`、`shots/01-{report,writing,project}.png`）：
   真实窗口渲染首页对话中枢（`模型权限：未开启，提交已关闭`，由宿主回报）与三个工作场景入口；Dock 可导航。
2. 周报场景纵向闭环（`gui_flow.json` + `shots/20-*.png` … `shots/29-saved.png` + `artifact-weekly-gui.md`）：
   载入合成样例 → 在界面输入材料路径并「读取」（显示 `material.md · 85 字节 · sha256:46913042cc45`）
   → 「生成模型初稿」（阶段 done、宿主状态 succeeded，显示模型输出「合成回复内容」）
   → 勾选人工确认 → 「保存成果」→ 落盘 `offline-weekly-report-20261019-074115.md`（401 字节）。
   产物含模型初稿正文，并含 OmniDesk 追加的**材料来源**块（`material.md` + sha256 + 字节数），
   该块明确标注「由 OmniDesk 记录，非模型输出」。

### 2.5 回归

```
python3 tests/verification/desktop/run_desktop_e2e.py chain
#   [chain] pass {'success': ('pass','succeeded'), 'failure': ('pass','failed'), 'cancel': ('pass','cancelled')}
python3 tests/verification/desktop/run_workbench_engine_path.py
#   [workbench-engine-path] pass {...}
python3 tests/verification/desktop/verify_engine_cancel.py
#   state cases: 34 total, 0 failed; static checks: 8 total, 0 failed
python3 tests/verification/desktop/verify_session_continuity.py
#   checks: 37 total, 0 failed
git diff --check HEAD          # clean
git diff --stat HEAD -- runtime/   # 空（无差异）
```

## 3. 未验证项与限制

- **真实模型**：全部为合成端点输出。真实模型的质量、风格与合规表现未验证。
- **GUI 覆盖范围**：真实窗口只验证了「周报」一个场景的完整闭环（含材料选择与保存）；
  「文案写作」「项目跟踪」两场景的**界面**闭环未逐个点击走完（其提示词与引擎链路已由 §2.3 覆盖，
  界面骨架与周报共用同一组件）。
- **材料解析**：材料以「原样文本进入提示词」的方式使用；没有实现表格/文档的结构化解析，
  也未实现由模型调用预制 Python 处理表格/文档/数据库（计划已明确后置）。
- **保存后的「重新打开」**：本轮 GUI 闭环未点击重开按钮；重开是既有的只读路径，已由 OD-ARCH-019/025 验证，
  本轮未重跑窗口内的该步骤。
- **平台与系统**：跨平台、真实显示器/窗口管理器、系统级 `.deb` 安装未验证。
- 本轮为验证新 UI 停止了一个**遗留的、应用进程已退出的** main 树 vite（占用 5273）；未改动任何仓库文件。
- 工作区条目数由 104 变为 106（新增 `verify_scenario_materials.py` 与证据目录 `evidence/od-arch-026/`）；全部既有改动保留。

## 4. 首页版式回归：对话为主 + 顶部三模块（2026-10-19）

> 用户已批准的三项口径：**默认静态**（不用浮动/脉冲动画与卡片背景模糊）、**不伪造数据**（今日提醒沿用诚实空态）、
> **权限提示压缩**为输入框底部一行状态。本项是 OD-ARCH-026 的延续，不登记为新编号。

### 4.1 改动（产品源码）

| 文件 | 变动 |
| --- | --- |
| `desktop/src/shell/HomeCanvas.tsx` | **新增**。版式取自只读设计原件 `_design/stitch_omnidesk`：问候 → 一行三模块（机器人 `/mascot.png` + 今日重点推进 + 日程节律）→ 下半屏整幅 `HomeChat`。刻意差异见 §4.2 |
| `desktop/src/App.tsx` | 首页场景改渲染 `HomeCanvas`（其余场景接线不变） |
| `desktop/src/shell/HomeDashboard.tsx` | **删除**（右栏与「对话中枢」蓝框的来源；删除前确认只有 `App.tsx` 引用） |
| `desktop/src/shell/HomeChat.tsx` | 只改展示：删除空态说明段与三个快捷入口；整块「模型权限」banner 压缩为输入框底部一行状态；`workspaceNote` 仅在非空时以一行小字显示 |

未改动：提交/轮询/取消/准入逻辑、`Dock.tsx`、`TopNav.tsx`、Rust、`scripts/dev-ui.sh`、Step 0/2/3/4 证据。

### 4.2 三项口径如何落实

1. **默认静态**：吉祥物不再浮动、状态点不再脉冲、两张卡片不用背景模糊；层次由 `bg-white/80` + 描边 + 阴影表达。
   实测首页连续 5 帧（间隔 2 s）**签名种类 = 1**（无持续重绘）；空闲 60 s 的 `WebKitWebProcess` **0.08%**、App **0.15%**
   （与 OD-ARCH-027 Step 4 的 0.03–0.10% 同量级，未因新首页回升）。
2. **不伪造数据**：问候语后只有「· 欢迎回到 OmniDesk」，不写「今天有 N 项…」；「今日重点推进」徽标为
   **「未接入数据源」**，正文说明待办/项目数据源尚未接入、不展示任何待办或统计数字；「日程节律」只给三周日期骨架
   （已过/今天/将来），徽标 **「未同步」**。
3. **权限提示压缩**：已开启 →「模型权限：已开启 · 本机」；未开启 →「模型权限：未开启，提交已关闭」+「去设置」。
   端点主机/已开启类别/「只承载公开或合成内容」的边界说明移入该行的 `title` 提示。管理员事实仍来自宿主回报。

### 4.3 真实窗口（一次性 Xvfb `:97` + 隔离数据根 + 回环合成端点）

脚本：`home-layout/scripts/home-layout-check.py`；数据：`home-layout/home-layout-run1.json`；环境：`home-layout/env.json`。

| 截图 | 观察 |
| --- | --- |
| `home-layout/shots/home-layout-run1-01-home-empty.png` | 初始空对话：问候、机器人、两张卡、全宽对话、空消息区（无占位文案）、输入框底部一行「模型权限：已开启 · 本机」 |
| `...-02-home-input-filled.png` | 输入框中键入合成提示；发送按钮由禁用变为可用 |
| `...-03-home-after-send.png` | 回车提交后：右侧用户气泡 + 左侧「模型输出 · 未复核 · 已完成 / 合成回复内容」 |
| `...-04-home-reply.png` | 任务终态后（与 03 同画面，间隔 3 s）：消息区稳定 |
| `...-10-page-{home,workbench,project,settings}.png` | 四页逐页：无右栏、无「当前版本边界 / 查看引擎状态 / OMNIDESK · LOCAL FIRST」；三模块与全宽对话成立 |
| `...-20-stability-0..4.png` | 5 帧稳定序列（签名 = 1） |

- **消息真的走通了合成端点**（文件事实）：`/tmp/od26-home/home-layout-run1/app-data/engine-state/omni-bf3cc099-*.json`
  记录 `state=succeeded`、`stop_reason=end_turn`、`content=合成回复内容`。
- **顶部渐隐可见**：消息区容器 `chat-fade-mask`（顶部约 32% 渐隐），截图中首条用户气泡被柔化（与设计稿一致）。
- 四页截图判据：无黑屏/空白（`black_screen_suspect=false` / `blank_suspect=false`，`mean_luma` 243.6–246.2）。

### 4.4 源码 / 构建检查

```
cd desktop && npx tsc --noEmit     # EXIT=0
cd desktop && npm run build        # ✓ built（index-*.css 70.59 kB / index-*.js 331.26 kB）
```

- 源码：`HomeCanvas.tsx` / `HomeChat.tsx` 的 `className` 中**不含** `animate-float-mascot`、`animate-pulse`、`backdrop-blur`；
  右栏文案（「当前版本边界」「查看引擎状态」「OMNIDESK · LOCAL FIRST」「对话中枢」「直接下达任务或记录想法」）在源码与构建产物中均不存在。
- 构建 CSS：无 `.animate-pulse{` 与 `.backdrop-blur-md{`；保留 `.backdrop-blur-2xl{`（TopNav，OD-ARCH-027 Step 2 有意保留）
  与 `.animate-float-mascot{`（`theme.css` 中的定义，当前无 JSX 使用，reduced-motion 下停用）。

### 4.5 可复现命令

```bash
cd desktop && npx tsc --noEmit && npm run build
cd /mnt/data/Code/MyWorkStation
python3 tests/verification/desktop/evidence/od-arch-026/home-layout/scripts/home-layout-check.py   --label home-layout-run1 --display :97 --own-xvfb
```

### 4.6 未验证项与限制

1. **真实模型输出**：本轮回复来自回环合成端点（`合成回复内容`），不代表真实模型质量。
2. **平台**：仅一次性 Xvfb（无 GPU / 无 DRI3）与 1280×820；真实显示器、Wayland、release 内嵌产物、窗口缩放与窄窗口折行未验证。
3. **输入方式**：只用 XTEST 注入 ASCII 字符；输入法、粘贴、长对话滚动与大量消息下的渐隐表现未验证。
4. **设计还原度**：机器人列使用本树 `mascot.png`（480×745），在某些窗口高度下会比设计稿显小/显大；未做像素级比对。
5. **工作区条目数**：仍为 **108**（新增/删除的文件都在原本未跟踪的 `desktop/src/shell/` 与 `evidence/od-arch-026/` 内）。
