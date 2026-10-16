# OmniDesk P1-Desktop Phase 2 · Step 1 报告：桌面外壳实施基线

- 报告日期：2026-10-16
- 阶段：OmniDesk P1-Desktop Phase 2（独立桌面技术切片）
- Step：1 —— 读取并冻结当前架构边界
- 证据等级约定：`static`（文件/源码事实）· `script`（脚本或进程运行） · `model`（真实模型输出） · `unverified`（未验证）

> 本报告只做**只读读取与状态冻结**，不实现任何桌面代码。凡本步未实际运行的结论都标为 `unverified`，不以设计文本推断运行行为。

> **后续更新（2026-10-16）**：本报告为 Step 1 的**基线快照**。其 §0 第 3、4 条与 §5、§8 记录的『工具链缺失』『引擎无子进程入口』『需用户授权』等**当时结论均已解决**：Step 5–16 已在纯用户级工具链下落成 `desktop/` 并跑通端到端链路（`cargo test` 23/23、E2E driver exit 0）。**本报告正文保留历史原貌，不回改**；当前实际状态以 [tests/reports/p2-desktop-engine-slice-report.md](p2-desktop-engine-slice-report.md) 与路线图为准。

---

## 0. 结论摘要

1. **架构无冲突**：现有五层架构（UI → External Data → Skills → Profile → DSH Runtime）与"OmniDesk 自有 UI 经引擎适配层调用 DSH"的目标方向一致；`engine/omnidesk_engine/` 已作为 UI 与 Runtime 之间的可替换边界就位。
2. **边界已冻结**：仓库 HEAD、工作区改动、引擎入口、DSH ACP 入口、隔离数据根、禁止修改路径均已逐条登记（见第 2–3 节）。
3. **本机不具备立即构建桌面的工具链**（本步新发现的关键事实）：
   - 无 Rust 工具链（`cargo` / `rustc` / `rustup` 均不存在）；
   - Tauri 的 Linux 构建依赖（`libwebkit2gtk-4.1-dev`、`libgtk-3-dev`、`libsoup-3.0-dev` 等 **dev 头文件**）未安装，仅存在运行时 .so；
   - `sudo` 需要交互式密码，无法非交互安装系统包。
   - 因此 Step 5（建立最小 Tauri 工程）之后的构建类步骤，在当前环境下**无法直接执行**，除非先解决工具链（属需用户决策的安装动作）。
4. **引擎缺少"可被 Rust 宿主启动的子进程入口"**：`engine/omnidesk_engine/` 目前是**纯 Python 库**（无 `__main__.py`，无 CLI/stdio 服务入口）。Step 4 / Step 8 要求"Rust 启动 Python Engine 子进程 + 本机 IPC"，这意味着需要新增一个引擎侧入口进程；这是 Step 8 的实施内容，Step 1 只如实登记该缺口。
5. **命名一致性提示**：`docs/ARCHITECTURE.md` 把 UI 层写在 `apps/desktop`，本 Plan 使用 `desktop/`。二者只需在 Step 2 文档中择一并写清，不构成架构冲突。

---

## 1. 审计对象与版本基线

| 项 | 值 | 证据 |
| --- | --- | --- |
| 仓库根 | `/mnt/data/Code/MyWorkStation`（与 `/home/kevin/Code/MyWorkStation` 为**同一 inode**：`66308:5804745`，实为同一目录） | `static` |
| 分支 | `main` | `static` |
| HEAD | `17efd20be01b2a01cacf11f9523ccff302acce61`（`17efd20 docs: 新增下一阶段规划交接简报并更正过时结论`） | `static` |
| 现存 tag | **仅** `discipline-v0.2` | `static` |
| 版本号 | `VERSION` = `0.2.1` | `static` |
| DSH 底座 | `dsh-v0.2.1-alpha.2`，commit `d743267388641bc76f17c45ce8b4c231aed1d32c` | `static` |
| 底座源码（子模块） | `runtime/deepseek-harness`，`git status` 干净，HEAD = `d7432673`，`describe` = `dsh-v0.2.1-alpha.2` | `static` |
| 可运行 CLI | `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js`（`--version` → `0.2.1-alpha.2`） | `script` |
| Node | v22.23.2 | `script` |
| Python | 3.14.4 | `script` |
| 操作系统 | Ubuntu 26.04.1 LTS（resolute），x86_64 | `static` |

### 1.1 工作区状态（本步冻结快照）

`git status --porcelain=v1` 共 **27 项**（9 处已修改 + 18 项未跟踪）。本步**未改动任何工作区文件**。

已修改（9）：

    .gitignore
    README.md
    docs/ARCHITECTURE.md
    docs/OmniDesk-Design-Principles-v0.3.md
    docs/OmniDesk-Next-Phase-Brief.md
    knowledge/regulations/README.md
    profiles/yvan-work/cordis.patch.yml
    scripts/setup.sh
    skills/knowledge-search/SKILL.md

未跟踪（18）：

    docs/OmniDesk-Engine-Interface-Audit.md
    docs/OmniDesk-Engine-Interface-Contract.md
    docs/OmniDesk-Engine-Interface-Test-Matrix.md
    docs/OmniDesk-Knowledge-Ingestion-and-Desensitization-Policy.md
    docs/OmniDesk-Knowledge-Retrieval-Acceptance-Spec.md
    docs/OmniDesk-Product-and-Architecture-Roadmap.md
    engine/
    knowledge/README.md
    knowledge/learning-materials/
    knowledge/reflections/
    scripts/ingest-documents.py
    tests/reports/p1-engine-adapter-prototype-report.md
    tests/reports/p1-engine-interface-contract-report.md
    tests/reports/phase1-verification-report.md
    tests/reports/phase2-webui-writing-e2e-report.md
    tests/reports/retrieval-verification-report.md
    tests/verification/
    workstations/discipline/drafts/

> 上述未提交改动属既有成果，Step 1 保留不动；本 Plan 全程"不提交、不推送、不打 Tag"。

---

## 2. 关键入口与数据根（逐条核实）

### 2.1 Python Engine 入口

| 项 | 值 | 证据 |
| --- | --- | --- |
| 包路径 | `engine/omnidesk_engine/` | `static` |
| 模块 | `__init__.py` · `contract.py` · `acp_adapter.py` · `jsonrpc.py` | `static` |
| 对外类 | `AcpEngineAdapter`（`initialize` / `submit_task` / `get_task_status` / `stream_events` / `get_task_result` / `cancel_task` / `recover_session` / `recover_task` / `shutdown`） | `static` |
| 依赖 | 仅 Python 标准库（零第三方依赖） | `static` |
| **子进程入口** | **不存在**（无 `__main__.py`，无关键字 CLI 解析；当前只提供**进程内 Python API**） | `static` |
| 构造便利方法 | `AcpEngineAdapter.for_dsh(dsh_bin, cwd=..., profile="acp", patch_files=...) ` → `["node", dsh_bin, "--profile", "acp", ...]` | `static` |
| 驱动测试 | `tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py`（E01–E20） | `static`（历史运行结论：20/20 pass） |

> **缺口登记（供 Step 8 使用）**：契约要求的是**进程边界**（Rust 宿主 ↔ Python Engine 子进程）。当前适配层只能在调用它的同一 Python 进程内使用。Step 8 需要新增一个"引擎入口进程"，把 `AcpEngineAdapter` 包成一个可由 Rust 启动、经本机 IPC 通信的服务。Step 1 只登记，不实现。

### 2.2 DSH ACP 入口

| 项 | 值 | 证据 |
| --- | --- | --- |
| 调用形式 | `node <abs>/runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js --profile acp [--patch <path>...]` | `static` |
| 通道协议 | ACP v1（JSON-RPC over stdio） | `static` |
| 方法面 | `initialize` / `session/new` / `session/list` / `session/resume` / `session/close` / `session/setConfigOption` / `session/prompt`；通知 `session/cancel`、`session/update` | `static`（历史审计） |
| 已知语义缺口 | ACP 在整轮结束一次性投递 assistant 文本（**消息级**，非 token 级）；`stopReason=end_turn` 无法区分 completed 与 blocked | `script`（历史实测） |

### 2.3 隔离数据根

| 路径 | 角色 | 现状 | 证据 |
| --- | --- | --- | --- |
| `~/OmniDesk/` | 隔离运行根 | 存在（含 `runtime/`、`quarantine/`） | `static` |
| `~/OmniDesk/runtime/dsh-home/` | 隔离 DSH 数据根（会话/storages/profiles/skills） | 存在 | `static` |
| `~/OmniDesk/runtime/agents-home/` | 隔离 `DSH_AGENTS_HOME` | 存在（空） | `static` |
| `~/OmniDesk/runtime/desktop-user-data/` | 隔离 Electron userData | 存在 | `static` |
| `~/YvanAI/` | 外部数据中枢（knowledge/memory/projects/templates/backups/logs） | 存在 | `static` |
| 生效 Profile | `~/OmniDesk/runtime/dsh-home/profiles/yvan-work/cordis.patch.yml`（运行时副本） | 存在，mtime 00:41 | `static` |
| 生效配置源头 | 仓库 `profiles/yvan-work/cordis.patch.yml` | 存在（默认 `opendesign / deepseek-v4.1-flash` · high） | `static` |
| 隔离启动器 | `scripts/omnidesk-dev.sh`（强制校验 DSH_HOME/AGENTS_HOME 落在 `~/OmniDesk` 之下，拒绝透传 `--patch/--profile`） | `static` |

> 隔离根由启动级环境变量 `DSH_HOME` / `DSH_AGENTS_HOME` 决定，是 DSH **官方支持**的重定向手段，无需补丁（`static`，见隔离审计 §2.1）。

---

## 3. 禁止修改 / 禁止触碰路径（硬边界）

本 Plan 全程不得修改或写入以下路径：

| 路径 | 说明 |
| --- | --- |
| `runtime/deepseek-harness/` | DSH 上游源码子模块，必须与官方发布一致（已核实 `git status` 干净） |
| `runtime/dsh-runtime/` | 由 `scripts/bootstrap-runtime.sh` 安装的锁定版本底座，已被 `.gitignore` 排除 |
| `~/.dsh/` | 原版 DSH 数据根（用户日常使用，**本机 Desktop 正在运行并写入**） |
| `~/.agents/` | 原版 agents 根 |
| `~/.config/DSH Desktop` / `~/.local/share/io.github.hairyf.deepseek-harness-desktop/` | 原版 Desktop 外壳数据 |
| 任何凭证文件 | 不读取、不打印、不复制（含 `~/.dsh/.credentials.yaml` 与 `~/OmniDesk/runtime/dsh-home/.credentials.yaml`） |

补充：`~/OmniDesk/runtime/dsh-home/.credentials.yaml` 存在（229 B，mode 600）。**按约束本步未读取其内容**，仅登记存在性与元数据。

---

## 4. 架构冲突检查

对照 `docs/ARCHITECTURE.md` 五层模型、`docs/OmniDesk-Engine-Interface-Contract.md` 九条边界原则与本 Plan 的分层要求，逐项核对：

| 检查项 | Plan 要求 | 现状 | 结论 |
| --- | --- | --- | --- |
| UI 不得直接调用 DSH ACP | Step 3：UI 不直接调用 ACP | 契约 §1.1 第 4/7 条已禁止；`engine/` 已封装 ACP | ✅ 一致 |
| DSH 仅作可替换执行引擎 | Step 4：不改/不复制 DSH Runtime | `omnidesk-dev.sh` 只走公开 CLI；子模块干净 | ✅ 一致 |
| 数据资产独立于运行环境 | Step 6：不得用 `~/.dsh` 等 | 已有 `~/OmniDesk`、`~/YvanAI` 双根 | ✅ 一致 |
| 跨层职责 | Step 3/13 约束：React=UI、Rust=宿主/IPC、Python=AI 逻辑、DSH=引擎 | 契约 §1.2 分层表语义相同 | ✅ 一致 |
| UI 目录命名 | Plan 用 `desktop/` | ARCHITECTURE.md 写 `apps/desktop` | ⚠️ 命名需在 Step 2 统一（非冲突） |
| 引擎进程入口 | Step 4/8 需 Rust 启动 Python 子进程 | 当前仅有进程内库，无 `__main__` | ⚠️ 需在 Step 8 新增（已在第 2.1 节登记） |
| 技术栈冻结 | Step 2 冻结 Tauri 2 + React + TS | 路线图当前把 Tauri 记为"候选、非最终定案" | ⚠️ Plan Step 2 属**用户授权的收敛决策**，须在 Step 2 文档中记录该决策与理由 |

**结论：不存在阻断性架构冲突。** 三项 ⚠️ 均为"待落实的具体决定/待补齐实现"，不是设计矛盾。

---

## 5. 桌面工具链就绪度（本步关键发现）

Step 5 起需要构建 Tauri 应用；本步实测本机就绪度如下：

> **更新（2026-10-16，Phase 2 Step 5–16 执行时）**：下表为本步（Step 1）的**原始只读快照**，记录当时工具链缺口。该缺口随后**已由用户级方案解决**，不再阻塞：新增 `scripts/setup-desktop-toolchain.sh`，用纯用户级方式安装 Rust（`~/.cargo`、`~/.rustup`，不改系统、不改 shell profile）并把 Tauri 构建期依赖 `.deb` 解到私有前缀 `~/.local/omnidesk-tauri-deps`（`PKG_CONFIG_SYSROOT_DIR` 合并系统头/库，无需 sudo）。现状经实测：`bash scripts/setup-desktop-toolchain.sh --check` → `RESULT: ready`（exit 0）；`cargo 1.99.1`、`rustc 1.99.1` 可用；`desktop/` 已成功构建并运行，`cargo test` 23/23 通过。下表的"未安装/不可用"字段保留其历史含义，**不代表当前状态**。


| 依赖 | 期望 | 实测 | 证据 |
| --- | --- | --- | --- |
| `cargo` | 存在 | **不存在** | `script` |
| `rustc` | 存在 | **不存在** | `script` |
| `rustup` / `~/.cargo` / `~/.rustup` | 存在 | **不存在** | `script` |
| `libwebkit2gtk-4.1-dev`（Tauri Linux 必需 dev 头） | 已装 | **未安装**（仅运行时 `libwebkit2gtk-4.1.so.0`） | `script` |
| `libgtk-3-dev` | 已装 | **未安装**（仅运行时 `libgtk-3.so.0`） | `script` |
| `libsoup-3.0-dev` | 已装 | **未安装**（仅运行时 `libsoup-3.0.so.0`） | `script` |
| `pkg-config` 可见 `gtk+-3.0` / `webkit2gtk-4.1` | 可见 | **不可见** | `script` |
| 非交互提权 | 可用 | **不可用**（`sudo` 需交互密码；用户 uid=1000，属 sudo 组） | `script` |
| Node / npm / pnpm | 可用 | 可用（node v22.23.2、npm 10.9.8、pnpm 11.28.5） | `script` |
| Rust 安装源可达性 | — | `static.rust-lang.org` 返回 200；`static.crates.io` 返回 200；npm registry 可达 | `script` |

**推断（`unverified`）**：若安装 rustup 工具链并安装上述 `*-dev` 包，本机理论上具备构建 Tauri 2 应用的前提。该推断**未实测**，需在取得安装授权后验证。

**影响**：

- Step 1–4（读取边界、技术栈文档、IPC 契约、启动方式文档）**不依赖工具链**，可完整执行。
- Step 5–16（建工程、构建、运行桌面）**依赖** Rust + WebKit/GTK dev 头 + 非交互安装能力。当前条件下无法完成，只能记录阻塞。
- 安装工具链属**改变主机环境**的动作，超出本 Plan 的"只读/仓库内变更"边界，需用户明确授权后再执行。

---

## 6. 验收标准自检（Step 1）

| 验收要求 | 结果 |
| --- | --- |
| 报告明确记录当前 HEAD | ✅ `17efd20be01b2a01cacf11f9523ccff302acce61` |
| 记录工作区状态 | ✅ 27 项（9 modified + 18 untracked，Step 1 时的快照），本步未改动任何文件 |
| 记录 Python Engine 入口 | ✅ `engine/omnidesk_engine/`（并登记"无子进程入口"缺口） |
| 记录 DSH ACP 入口 | ✅ `node .../dsh/lib/bin.js --profile acp` |
| 记录隔离数据根 | ✅ `~/OmniDesk/runtime/{dsh-home,agents-home,desktop-user-data}`、`~/YvanAI/` |
| 记录禁止修改的 Runtime 路径 | ✅ 第 3 节 |
| 确认不存在架构冲突 | ✅ 无阻断性冲突；三项待落实决定已登记 |

---

## 7. 本步明确未做的事

- 未创建 `desktop/` 或任何 Tauri/React 代码。
- 未安装 Rust、未 `apt install`、未提权、未改动系统。
- 未运行任何模型任务；未读取任何凭证。
- 未提交、未推送、未打 Tag；未改动既有 27 项工作区变更。

---

## 8. 需用户决策的阻塞点（本节结论已更新）

> **更新（2026-10-16）**：本节原列的两个阻塞点**均已解决**，见下。

1. **桌面工具链安装授权（已解决，无需 sudo）**：Step 1 曾判定需安装 Rust 工具链与 Tauri 构建依赖并需用户授权。实际采用**纯用户级方案** `scripts/setup-desktop-toolchain.sh` 解决——Rust 装到 `~/.cargo`/`~/.rustup`，构建期依赖 `.deb` 解到 `~/.local/omnidesk-tauri-deps`，`PKG_CONFIG_SYSROOT_DIR` 合并系统头/库，**未 apt 安装、未提权、未改系统**。实测 `--check` → `RESULT: ready`（exit 0）、`cargo test` 23/23 通过，Step 5–16 已在此环境下完成。
   - ~~若不授权…~~ 该阻塞已解决。
2. **UI 目录命名（已解决）**：统一为 Plan 的 `desktop/`；`docs/ARCHITECTURE.md` 的 UI 层指向已同步修订并加注说明（原占位名 `apps/desktop` 已弃用）。

---

## 9. 下一步

Step 2：确定桌面技术栈（`docs/OmniDesk-Desktop-Architecture.md`），冻结 React/TypeScript（UI）+ Rust/Tauri（宿主）+ Python（AI 引擎）分工，并明确禁止把 DSH Electron Desktop 作为 OmniDesk UI。该步不依赖第 8 节的两个决策项，可继续执行。
