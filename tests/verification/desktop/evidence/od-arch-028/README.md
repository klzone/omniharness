# OD-ARCH-028 证据索引：阶段二试点——定版首页接入工作模块，以周报为首个体验闭环

> 本文件是 OD-ARCH-028 的**单一证据索引**。它记录本轮实际运行了什么、观察到什么、哪些没验证，
> 供 ARCHITECT 独立复核定位证据；**不构成审查结论**。
>
> 开工 HEAD：`5ffc7101e90dcf3a2047f214b71286d3a32208a8`（`main`）。未提交、未推送、未打标签。
> 未改动 `runtime/`、模型路由、凭证、真实材料、用户数据与第二 worktree（`/mnt/data/Code/omniharness-ui`）。

## 0. 范围与边界

- 计划：`PLANS.md` 的 OD-ARCH-028（用户 2026-10-19 批准）。
- 前置门：ARCHITECT 对 OD-ARCH-026 的独立复核（结论见 `REVIEW.md`）——**通过**，本计划才继续。
- 全部验证使用**合成材料 + 本机回环合成端点**（`_mock_model_server.py`，零凭证、零外网）。
- 真实窗口验证在**一次性 Xvfb 显示**（`:98`）与**一次性隔离数据根**（`/tmp/od028/...`）中进行；
  成果目录指向隔离根（`OMNIDESK_LOCAL_DATA_ROOT` + 设置项 `artifact_root`），**未写 `~/OmniDesk`**。
  未截取用户桌面会话。
- 用户已确认的 UI 基线（GOALS.md「已确认的 UI 基线」）是本轮的前置条件： frameless 外框 + 对话中枢首页。

## 1. 改动清单

| 层 | 文件 | 内容 |
| --- | --- | --- |
| 验收装置（新增） | `scripts/od028-acceptance.py` | 真实窗口三场景闭环 + 失败/取消负例；只读导入既有 `window_harness` / `run_window_acceptance` / OD-ARCH-027 `_step3lib`，不改共享装置 |
| 证据（新增） | `shots/od028-run1-*.png`（29 张） | 真实窗口逐步截图 |
| 证据（新增） | `od028-run1.json` | 机器可读结果（任务台账、探针命中、成果文件要素、耗时） |
| 文档（新增） | `README.md` | 本索引 |
| 宿主（改动 1 处） | `desktop/src-tauri/src/task_shell.rs` | `read_material` 拒绝仓库内文件时的报错文案由「拒绝把产物写入仓库内」改为「拒绝读取 …位于工作区仓库内，不能作为模型材料」，并补 1 条单测；**边界判定未变** |
| 产品前端 | **无改动** | 三场景接线、模型起草链路与页面版式沿用 OD-ARCH-026 与用户已确认的 UI 基线 |

## 2. 复现命令

```bash
cd /mnt/data/Code/MyWorkStation
python3 tests/verification/desktop/evidence/od-arch-028/scripts/od028-acceptance.py \
  --label od028-run1 --display :98 --own-xvfb
```

前置检查：`npx tsc --noEmit`、`npm run build`、`cargo test --offline`、`verify_ai_draft.py`、
`verify_scenario_materials.py`（见 §3）。

## 3. 实测结果

### 3.1 前置门：OD-ARCH-026 独立复核（ARCHITECT，结论见 REVIEW.md）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 前端类型与构建 | `npx tsc --noEmit` / `npm run build` | 0 错；构建通过 |
| 宿主单测 | `cargo test --offline` | **143 passed / 0 failed**（含本轮新增 1 条材料边界用例） |
| 提示词与场景回归 | `verify_ai_draft.py` | `cases=27 failed=0` → pass |
| 三场景材料进入模型请求 | `verify_scenario_materials.py` | `prompts=3 scenarios=5 failed=0` → pass |
| 桌面任务链 | `run_desktop_e2e.py chain` | success/failure/cancel 三态 pass |
| 任务工作台引擎路径 | `run_workbench_engine_path.py` | pass |
| 取消语义 | `verify_engine_cancel.py` | state 34/0、static 8/0 → pass |
| 会话连续性 | `verify_session_continuity.py` | 37/0 → pass |

### 3.2 真实窗口三场景闭环（`od028-run1.json`，29 张截图）

窗口 1280×820，一次性 Xvfb `:98` + 一次性隔离根 `/tmp/od028/od028-run1`；
`OMNIDESK_LOCAL_DATA_ROOT` 实测为隔离根（**未写 `~/OmniDesk`**）。

| 场景 | 任务终态 | 材料标记进入模型请求 | 成果文件 | 要素自检 | 耗时 |
| --- | --- | --- | --- | --- | --- |
| 周报与随记 | `succeeded`（`end_turn`） | ✓（探针命中 `OD028-WEEKLY-MARKER-3a91`） | `offline-weekly-report-20261019-153832.md`（427 B） | 标题/模型正文/人工编辑/材料来源 全部 ✓ | 196.0 s |
| 文案写作 | `succeeded` | ✓（`OD028-WRITING-MARKER-6c42`） | `offline-material-writing-20261019-154024.md`（432 B） | 全部 ✓ | 111.8 s |
| 项目跟踪 | `succeeded` | ✓（`OD028-PROJECT-MARKER-9b17`） | `offline-project-management-20261019-154224.md`（434 B） | 全部 ✓ | 120.4 s |

每个场景的完整链路：**左侧 Dock 进入 → 载入合成样例 → 粘贴材料绝对路径并「读取材料文件」（界面显示
文件名/字节数/sha256）→「生成模型初稿」（阶段 running → succeeded）→ 在初稿文本框内人工追加
`(human-edit-ok)` → 勾选人工确认 → 「保存成果」→ 成果列表出现该文件 → 点「重新打开」读回**。
成果文件头形如：

```
# OD028 weekly synthetic

- 场景：weekly_report
- 生成时间 (UTC)：2026-10-19 15:38:32 UTC
- 处理方式：本地确定性代码 + 模型初稿（模型来源未复核，已由人工编辑核对）
- 数据边界：仅合成 / 公开 / 经明确批准的数据

> 材料来源（由 OmniDesk 记录，非模型输出）：
> - material-weekly.md（sha256:…，124 字节）
合成回复内容 (human-edit-ok)
```

- 读回：周报场景保存时列表内**只有该一件**，因此「重新打开」读回的就是刚保存的那一件
  （截图 `od028-run1-09-weekly-reopened.png` 可见完整正文与来源块）；写作/项目场景保存时列表已有多件，
  宿主不保证顺序，**不据此断言读回对象**，只以文件事实为准（JSON 内 `readback_targets_newest=false`）。

### 3.3 负例：失败与取消不得伪装成功

| 负例 | 终态 | 界面/文件事实 |
| --- | --- | --- |
| 端点 400（`error_400`） | `failed`，`error_message` = `Internal error: turn failed: 400: {"message":"synthetic upstream failure",…}` | **未产生任何成果文件**（`new_artifacts=[]`）；截图 `…-27-neg-project-error_400-failed.png` |
| 端点挂起后点「取消生成」 | `cancelled` | **未产生任何成果文件**（`new_artifacts=[]`）；截图 `…-29-neg-project-hang-cancelled.png` |

### 3.4 试点评价（按计划要求记录，不外推）

- **执行请求数：** 成功路径 3 次模型请求（每场景 1 次，探针命中）；负例另有 1 次 400 与 1 次挂起请求。
- **人工澄清数 / 返工轮数：** 本轮是**自动化技术验收，无真人参与**，记 0；这**不是**真实体验数据。
  真实首次使用的澄清/返工需用户在「使用记录」里逐次记录，不从本轮推断。
- **耗时：** 每个场景 112–196 s（含大量等待、滚动与截图，不代表真人用时）。
- **Token：** 引擎本次**未上报 usage**（`progress` 里没有 `{kind:"usage"}`），首页同样显示
  `Token 上下文：—（引擎未上报）`。按计划口径记为**未知，不估算、不累加**。
- **与历史任务比较：** 本轮是三场景「材料 → 模型起草 → 人工编辑 → 确认保存」的自动化闭环，
  范围与 OD-ARCH-026 的单场景闭环不同，**不据此推导普遍比例**。

## 4. 未验证项与限制

1. **真实模型**：全部为回环合成端点（`合成回复内容`）。真实模型的质量、风格、合规与
   「不虚构」表现未验证。
2. **平台**：一次性 Xvfb（无 GPU/无 DRI3、无窗口管理器）+ 1280×820；真实 Wayland 桌面、
   release 内嵌产物、窗口缩放与窄窗口折行未验证。
3. **输入方式**：只用 XTEST 注入 ASCII；输入法、粘贴、中文输入未验证。
4. **读回对象**：写作/项目场景的「重新打开」未逐件断言指向哪一件（宿主不保证列表顺序）。
5. **本轮只读/未改**：产品源码仅一处错误文案修正（`read_material` 拒绝仓库内文件时的提示由
   「拒绝把产物写入仓库内」改为「拒绝读取 …位于工作区仓库内」，并补 1 条单测）；
   其余功能、接线与 UI 均未改动，页面版式沿用用户已确认的 UI 基线。
6. **真实工作材料、真实端点授权与同事试用不在本计划范围。**
7. **工作区条目：** `git status --porcelain` 由 **111 变为 113**（未跟踪目录折叠计数）：
   新增 `evidence/od-arch-028/` 目录 + 修改 `desktop/src-tauri/src/task_shell.rs`；该目录下另有 32 个新文件
   （脚本 1、JSON 1、README 1、截图 29）。`git diff --check HEAD` 干净；`runtime/` 无差异；
   未提交、未推送、未打标签。

