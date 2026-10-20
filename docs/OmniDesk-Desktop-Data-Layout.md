# OmniDesk 桌面应用数据布局（P1-Desktop Phase 2 · Step 6）

- 文档日期：2026-10-16
- 阶段：OmniDesk P1-Desktop Phase 2（独立桌面技术切片）
- 对应用户 Plan Step 6：「为桌面应用定义独立的 OmniDesk user-data 根目录」
- 证据等级约定：`static`（文件/源码事实）· `script`（脚本或进程运行） · `model`（真实模型输出） · `unverified`（未验证）

> 本文只规定**桌面外壳**自身的数据落点。知识、模板、个人成果等业务数据仍保存在仓库外独立数据目录（见 §5），两者不混放。

---

## 1. 结论摘要

1. 桌面应用的**应用数据根**由 `AppPaths::resolve()` 在启动时解析：优先环境变量 `OMNIDESK_APP_DATA_ROOT`（必须是绝对路径），否则退回 `$HOME/OmniDesk/app`。
2. 应用数据根下有五个**固定子目录**：`ui-state/`、`logs/`、`engine-state/`、`engine-state/results/`、`config/`。全部由 `AppPaths::ensure()` 幂等创建。
3. 解析结果**必须**通过 `assert_isolated()` 边界校验，落在原版 DSH / agent / Desktop 数据或 DSH 源码路径下一律拒绝启动（见 §3 的禁止片段列表）。
4. 桌面应用**不使用** `~/.dsh`、`~/.agents`、`~/.config/DSH Desktop`、`~/.local/share/io.github.hairyf.deepseek-harness-desktop`。
5. 引擎子进程**不共用桌面应用数据根**：引擎侧另有 `engine-state`（任务台账 / 事件游标 / 结果落盘），由宿主经 `OMNIDESK_ENGINE_STATE_DIR` 显式下发；隔离 DSH 数据根由 `OMNIDESK_DSH_HOME` / `OMNIDESK_AGENTS_HOME` 下发。

---

## 2. 应用数据根与子目录

### 2.1 解析优先级（源码事实，`static`）

来源：[desktop/src-tauri/src/app_paths.rs](../desktop/src-tauri/src/app_paths.rs) 的 `AppPaths::resolve()`。

| 优先级 | 来源 | 约束 |
| --- | --- | --- |
| 1 | 环境变量 `OMNIDESK_APP_DATA_ROOT` | 非空时采用；**必须是绝对路径**，否则直接报错拒绝启动 |
| 2 | 默认 `$HOME/OmniDesk/app` | 仅在未设置环境变量时使用；`$HOME` 缺失则报错 |

多实例隔离依赖第 1 项：每个实例给一个独立绝对路径即可，无需共享任何可变状态。这一能力在 Step 15（双实例隔离）中已实测（见 `tests/verification/desktop/evidence/dual-instance-report.json`）。

### 2.2 目录结构

由 `AppPaths::from_root(root)` 派生（`static`）：

| 路径（相对应用数据根） | 用途 | 说明 |
| --- | --- | --- |
| `ui-state/` | UI 持久状态 | 窗口尺寸、上次任务输入等；**不含任务正文** |
| `logs/` | 应用日志 | 桌面宿主自身日志 |
| `engine-state/` | 引擎状态目录 | 任务台账、事件补漏游标 |
| `engine-state/results/` | 结果落盘 | 引擎产出的终态大文本（结果正文） |
| `config/` | 桌面配置 | 桌面应用自身配置 |

`AppPaths::ensure()` 会对上述全部目录调用 `create_dir_all`，因此该操作是**幂等**的。

### 2.3 本机实际存在性（`script`）

在真实环境 `ls` 核实（应用已至少运行过一次，`ensure()` 已生效）：

    /home/kevin/OmniDesk/app/
    ├── config/
    ├── engine-state/
    │   └── results/
    ├── logs/
    └── ui-state/

> 与 `from_root` 派生结构逐一对应。测试运行时使用 `OMNIDESK_APP_DATA_ROOT` 指向一次性 `/tmp` 根，不写入 `~/OmniDesk/app`（见 §6）。

---

## 3. 边界校验：禁止片段（`static`）

`assert_isolated()` 先把数据根转小写、把反斜杠统一为 `/`，再逐项匹配以下片段；命中任意一项即返回 `PathBoundaryError` 并拒绝启动：

| 禁止片段 | 对应原版数据 |
| --- | --- |
| `/.dsh` | 原版 DSH 数据根（`~/.dsh`） |
| `/.agents` | 原版 agents 根（`~/.agents`） |
| `/.config/dsh desktop` | 原版 DSH Desktop 配置 |
| `/.local/share/io.github.hairyf.deepseek-harness-desktop` | 原版 DSH Desktop 应用数据 |
| `deepseek-harness` | DSH 源码目录（含 `runtime/deepseek-harness`） |
| `/.codex` | Codex 数据根 |

另有**相等性**单独校验：当数据根本身就是 `$HOME/.dsh`、`$HOME/.agents`、`$HOME/.codex` 时也拒绝（此时不含尾部子路径，片段匹配可能漏判）。

该规则由 Rust 单元测试覆盖（`app_paths::tests` 内 `rejects_stock_dsh_home` / `rejects_stock_agents_home` / `rejects_root_equal_to_stock_home` / `rejects_dsh_source_tree` 等）。

---

## 4. 引擎侧数据根（与桌面根分离）

`EngineConfig::from_env(app_root)`（来源 [desktop/src-tauri/src/engine_supervisor.rs](../desktop/src-tauri/src/engine_supervisor.rs)）解析引擎子进程配置。所有关键项由环境变量显式下发，**缺失时返回可读错误，不凭空造默认值**（DSH 相关路径除外，见下表默认列）：

| 环境变量 | 含义 | 默认 |
| --- | --- | --- |
| `OMNIDESK_PYTHON` | Python 解释器 | `python3` |
| `OMNIDESK_ENGINE_DIR` | 引擎包目录（含 `omnidesk_engine/`），作为子进程 cwd | `<repo>/engine` |
| `OMNIDESK_DSH_CLI` | DSH CLI 绝对路径（`bin.js`） | `<repo>/runtime/dsh-runtime/.../bin.js` |
| `OMNIDESK_DSH_HOME` | 隔离 `DSH_HOME` | `$HOME/OmniDesk/runtime/dsh-home` |
| `OMNIDESK_AGENTS_HOME` | 隔离 agents | `$HOME/OmniDesk/runtime/agents-home` |
| `OMNIDESK_ENGINE_STATE_DIR` | 引擎状态目录 | `<app root>/engine-state` |
| `OMNIDESK_DSH_PROFILE` | 引擎子进程的 Profile 名（该子进程只讲 ACP，因此默认与内置模板 `acp` 对齐） | `acp` |
| `OMNIDESK_DSH_PATCHES` | 逗号分隔的补丁绝对路径 | 空 |
| `OMNIDESK_REPO_ROOT` | 仓库根（用于推导默认 repo 相对路径） | 自动探测 |

隔离运行根（仓库外，`static`）：

    ~/OmniDesk/
    ├── app/                     # 桌面外壳应用数据根（本文 §2）
    ├── runtime/
    │   ├── dsh-home/            # 隔离 DSH_HOME（会话 / storages / profiles / skills）
    │   ├── agents-home/         # 隔离 DSH_AGENTS_HOME
    │   └── desktop-user-data/   # 隔离 Electron userData（历史路径，桌面外壳不依赖）
    └── quarantine/              # 隔离/待处理区

DSH 侧的重定向走 `DSH_HOME` / `DSH_AGENTS_HOME` 两个**官方支持**的环境变量，不修改 DSH 源码、不打补丁。

---

## 5. 业务数据（不在桌面数据根内）

个人知识、模板与成果保存在仓库外独立目录，以可迁移文件格式为权威源：

**新成果的规范位置（`static`，OD-ARCH-011 起）：**

    ~/OmniDesk/workspace/artifacts/      # 桌面新生成的离线成果与分析成果

默认值定义在 `desktop/src-tauri/src/task_shell.rs` 的 `DEFAULT_LOCAL_SUBDIR = "OmniDesk/workspace/artifacts"`，可用 `OMNIDESK_LOCAL_DATA_ROOT` 显式覆盖（测试用它指向一次性 `/tmp` 根）。两条保存路径共用 `write_new_file_durable()`：`create_new` 拒绝覆盖同名文件，写入或刷盘失败时删除本次半成品。

**历史位置（`static`，不迁移、不读取正文）：**

    ~/YvanAI/
    ├── knowledge/   memory/   projects/   templates/   backups/   logs/

以及运行期知识资产：`knowledge/regulations/`、`knowledge/writing-style/`、`knowledge/excellent-cases/`（仓库内权威源，经显式同步进入隔离运行根）。

`~/YvanAI/projects/Omnidesk/analysis/` 是旧版分析成果的落点，可能仍有真实成果。OD-ARCH-011 只把**将来**的新文件改到 `~/OmniDesk/workspace/artifacts`，没有读取、迁移、改名或覆盖其中任何文件。**因此当前不能推断资产已统一**；是否迁移旧成果须另行评估并经用户明确授权。

桌面与引擎数据根都**不承载**业务正文；这保证更换 DSH 或替换桌面外壳时业务数据可迁移。注意 `~/OmniDesk` 下同时存在运行时数据根（`runtime/`、`app/`）与业务成果目录（`workspace/artifacts/`），二者语义不同，备份与清理时不得混为一谈。

---

## 5.1 使用记录（usage.jsonl）：位置、格式与清理规则

**位置（`static` + `script`，OD-ARCH-030 实测）：**

    ~/OmniDesk/app/logs/usage.jsonl     # 正式使用：应用数据根的 logs/ 子目录（仓库外）
    /tmp/<一次性根>/app-data/logs/usage.jsonl   # 测试：OMNIDESK_APP_DATA_ROOT 指向一次性根时

**格式（一行一条 JSON，`static`）：** 只含枚举与时间，**不含**输入、输出、正文、标题、路径或会话：

| 字段 | 含义 |
| --- | --- |
| `record_id` / `schema` | 记录标识与模式版本（`omnidesk.usage.v1`） |
| `task_type` | 场景：`weekly_report` / `material_writing` / `project_management` |
| `recorded_at` | 记录时间（UTC 时间戳） |
| `completed` / `usable` | 是否完成、结果是否可直接用 |
| `rework_kind` / `blocker` | 返工类别、障碍类别（枚举） |
| `duration_bucket` | 耗时分档（枚举） |
| `reuse_willingness` | 是否愿意再做类似任务 |

**写入规则：** 只在用户点「记录一次」时追加；**默认不写**。界面「查看记录（N）」可展开、逐条删除或整体清除。

**保留期限与清理规则（登记，暂不实现自动清理）：**

1. 该文件是**试点评价的唯一来源**（人工澄清数、返工轮数、耗时），因此**验证/测试产生的记录必须在验证后删除**，
   不得混进真实使用的数据。OD-ARCH-030 的验证即按此执行：写入 2 条合成记录 → 走产品自带的逐条删除 →
   文件保留为 0 字节，正式根保持干净。
2. 默认**不做自动清理**。当记录条数明显增多或文件体积变大时，由用户在界面里逐条或整体清除；
   清除是产品自带路径，不需要手工改文件。
3. 该文件在**仓库外**（`~/OmniDesk/app/`），**不进 Git**；备份与清理时与 `~/OmniDesk/app` 的其余内容一致对待。
4. **不得**与 `~/OmniDesk/quarantine/`（真实案件材料）或 `~/OmniDesk/runtime/`（含凭证）混放、混备或互拷。

**同目录下的其它文件：** 应用首次运行会把可编辑的流程包副本写到 `~/OmniDesk/app/config/offline-flows.v1.json`
（OD-ARCH-030 实测新增 2918 字节）。它是**可编辑的场景规则**（`draft.rules` / `draft.sections`），
不含凭证与用户数据；改动它等于改动工作台的整理规则。

---

## 6. 测试期行为（`script`）

`tests/verification/desktop/run_desktop_e2e.py` 与 Rust 集成测试都以 `OMNIDESK_APP_DATA_ROOT` / `OMNIDESK_ENGINE_STATE_DIR` / `OMNIDESK_DSH_HOME` 指向**一次性 `/tmp` 根**，并在提交前用 `validate_isolated_root()` 自检：每个根都必须以 `/tmp/` 开头，且不含 `/.dsh`、`/.agents`、`deepseek-harness`。

Step 13 的隔离报告（`tests/verification/desktop/evidence/isolation-report.json`）显示：受守护路径（`~/.dsh`、`~/.agents`、`~/.config/DSH Desktop`、`~/.local/share/io.github.hairyf.deepseek-harness-desktop`）在桌面测试前后**完全一致**，`differences` 为空、`unchanged=true`，测试产生的新文件**只出现在 `/tmp`**。

---

## 7. 验收标准自检（Step 6）

| 验收要求 | 结果 | 证据 |
| --- | --- | --- |
| 桌面配置有明确专属路径 | ✅ `<app root>/config/` | `static` |
| 日志有明确专属路径 | ✅ `<app root>/logs/` | `static` |
| UI 状态有明确专属路径 | ✅ `<app root>/ui-state/`（不含任务正文） | `static` |
| 引擎状态有明确专属路径 | ✅ `<app root>/engine-state/`（含 `results/`） | `static` |
| 不使用 `~/.dsh` / `~/.agents` | ✅ `assert_isolated()` 硬拒绝 | `static` + 单元测试 |
| 不使用 `~/.config/DSH Desktop` 等 Desktop 数据 | ✅ 同上 | `static` |
| 多实例可隔离 | ✅ `OMNIDESK_APP_DATA_ROOT` 各自独立 | `script`（Step 15） |

**结论：Step 6 验收通过。** 桌面外壳的应用数据、引擎状态与业务数据三者路径分离，且桌面数据根不会被解析到任何原版 DSH / agent / Desktop 数据目录。
