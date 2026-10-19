# OmniDesk 桌面应用架构（技术栈冻结）

> 状态：**技术栈冻结**（OmniDesk P1-Desktop Phase 2 · Step 2 产物）
> 日期：2026-10-16
> 上游依据：[GOALS.md](../GOALS.md)、[架构决策](decisions.md)、[OmniDesk-Engine-Interface-Contract.md](OmniDesk-Engine-Interface-Contract.md)、[../tests/reports/p2-desktop-baseline-report.md](../tests/reports/p2-desktop-baseline-report.md)。本架构文档记录技术切片，不构成正式产品化批准。
> 说明：本文冻结**桌面外壳的技术栈与分层职责**。`desktop/` 工程已创建并可在 Linux 本机构建；实现进度与验收状态以 `PLANS.md`、`STATUS.md`、`REVIEW.md` 为准。

---

## 0. 决策摘要

| 层 | 技术 | 归属 | 状态 |
| --- | --- | --- | --- |
| 用户界面 | **React + TypeScript** | OmniDesk | 冻结 |
| 桌面宿主 | **Rust + Tauri 2.x** | OmniDesk | 冻结 |
| AI 工作逻辑 | **Python**（`engine/omnidesk_engine/`） | OmniDesk | 沿用现有 |
| 执行引擎 | **DSH**（初期，可替换） | 外部依赖 | 只读依赖 |

**明确禁止**：

1. **禁止把 DSH Electron Desktop 作为 OmniDesk UI。** 不使用、不嵌入、不换肤 DSH 官方桌面应用。
2. **禁止把 DSH Web 页面嵌入 OmniDesk。** 不加载 DSH Web Server 页面或静态资源。
3. **禁止 React/TypeScript 直接与 DSH 通信。** UI 只能调用 OmniDesk 自己定义的应用服务契约。
4. **禁止 Rust 与 Python 之间跨层实现业务功能。** 各层职责见第 2 节。

> 关于本决策的性质：产品路线图此前把 Tauri 记为"**候选、非最终选型**"。本 Step 依用户下发的 P1-Desktop Phase 2 Plan，将技术栈**收敛为定案**。收敛理由：本项目为**单一用户、单一领域**桌面应用，规模与跨平台需求明确；Tauri 2 提供 WebView 前端 + 原生宿主（进程管理、文件访问、安全边界）的组合，且宿主语言 Rust 对"启动/回收 Python 子进程、受控文件访问"这类原生职责契合。该定案**仍需在 Step 5 起用真实构建验证**；若 Step 12–16 的实测显示 Rust 带来的跨语言维护成本高于收益，按路线图约定应回退或缩小 Rust 代码面（记录为 `revise` 决策点，而非静默改栈）。

---

## 1. 目标架构

    ┌──────────────────────────────────────────────────────────┐
    │  React + TypeScript UI（OmniDesk 自有界面）               │
    │  任务输入 / 状态呈现 / 事件流 / 结果预览                   │
    └───────────────────────┬──────────────────────────────────┘
                            │ Tauri command / event（IPC-1：WebView ↔ Rust）
    ┌───────────────────────▼──────────────────────────────────┐
    │  Rust 桌面宿主（Tauri 2.x）                               │
    │  进程生命周期 / IPC / 窗口 / 受控文件访问 / 错误映射        │
    └───────────────────────┬──────────────────────────────────┘
                            │ 本机进程 IPC（IPC-2：Rust ↔ Python）
    ┌───────────────────────▼──────────────────────────────────┐
    │  Python Engine（engine/omnidesk_engine/）                 │
    │  OmniDesk 应用服务层 + 引擎适配层（统一任务契约）           │
    └───────────────────────┬──────────────────────────────────┘
                            │ 公开 CLI（--profile acp，JSON-RPC stdio）
    ┌───────────────────────▼──────────────────────────────────┐
    │  DSH Runtime（隔离数据根，只读依赖，不修改）                │
    └──────────────────────────────────────────────────────────┘

两条进程边界（IPC-1、IPC-2）是本文与后续 Step 3/7/8 的核心。**UI 永不越过 Rust 直达 Python，也永不直达 DSH。**

---

## 2. 分层职责（跨层禁令）

### 2.1 React + TypeScript（仅 UI）

**做**：渲染任务界面、收集用户输入、展示任务状态与事件流、展示结果与错误、发起提交/取消动作、把 Rust 暴露的事件映射为界面状态。

**不做**：

- 不解析 ACP/JSON-RPC 报文；不感知协议字段名。
- 不读取 DSH 会话文件、配置文件或数据根。
- 不生成 `omni_task_id`；不推断任务终态。
- 不直接派生/管理引擎子进程。

接口来源：仅 `docs/OmniDesk-Desktop-Engine-IPC.md` 定义的 Tauri 命令与事件。

### 2.2 Rust + Tauri（仅桌面宿主 / 进程生命周期 / IPC）

**做**：创建与关闭窗口；启动、监督、终止 Python Engine 子进程；在 WebView 与 Engine 之间转发消息（IPC-1 + IPC-2）；把 Engine 错误归一化后传给 UI；受控的文件读写（结果保存路径）；应用数据目录管理。

**不做**：

- 不实现 AI 任务逻辑、Skill 选择、模板渲染等业务规则（属 Python Engine）。
- 不解析 DSH/ACP 内部协议（属 Python Engine 适配层）。
- 不硬编码 DSH 路径或数据根；只接受 Engine 描述。

### 2.3 Python Engine（仅 OmniDesk AI 工作逻辑）

**做**：实现统一任务契约（九态状态机、事件归一化、取消/恢复仲裁、结果持久化）；驱动引擎适配层（当前为 ACP）；承载领域工作流（材料写作、知识检索等）与 Skill 资产定位。

**不做**：不创建/管理窗口；不承担 UI 状态；不修改 DSH Runtime。

### 2.4 DSH Runtime（仅执行引擎）

**做**：实际执行 AI 任务与模型调用。

**不做**（OmniDesk 侧约束）：不修改其源码；不向其内部数据目录写 OmniDesk 数据；不把私有格式当稳定接口。

---

## 3. 目录约定

仓库内桌面工程位置统一为 `desktop/`：

    desktop/
    ├── src/                      # React + TypeScript（UI）
    ├── src-tauri/
    │   ├── src/
    │   │   ├── main.rs
    │   │   ├── engine_supervisor.rs   # Step 7：子进程生命周期
    │   │   ├── engine_ipc.rs          # Step 8：Rust ↔ Python 通道
    │   │   └── engine_bridge.rs       # Step 9：对接 engine/omnidesk_engine/
    │   ├── tests/                     # Step 16：Rust 自动化测试
    │   ├── Cargo.toml
    │   └── tauri.conf.json
    ├── package.json
    └── tsconfig.json

> 命名收敛：`docs/ARCHITECTURE.md` 中 UI 层曾写作 `apps/desktop`（历史占位）。本项目统一为 `desktop/`；ARCHITECTURE.md 已就近标注，避免两处不一致。

---

## 4. 数据与配置归属

| 数据 | 归属 | 路径约定 |
| --- | --- | --- |
| 桌面应用配置 / UI 状态 | OmniDesk 桌面 | `~/OmniDesk/app/`（见 Step 6 数据布局文档） |
| 桌面日志 | OmniDesk 桌面 | `~/OmniDesk/app/logs/` |
| Engine 状态与任务结果 | OmniDesk Engine | `~/OmniDesk/app/engine-state/`（仓库外，可迁移） |
| 知识 / 记忆 / 项目资产 | 用户 | `~/YvanAI/`（与运行层解耦） |
| DSH 会话 / storages / 凭证 | DSH（隔离根） | `~/OmniDesk/runtime/dsh-home/` |

**硬性禁止**：任何 OmniDesk 桌面数据不得写入 `~/.dsh`、`~/.agents`、`~/.config/DSH Desktop` 或 `~/.local/share/io.github.hairyf.deepseek-harness-desktop/`。

---

## 5. 不变量（Invariants）

1. UI 不直接依赖 DSH：任何 DSH 相关字段名（`sessionId`、`stopReason`、ACP 方法名）不得出现在 React 代码中。
2. 引擎可替换：更换引擎（ACP → headless/SDK/其他）只应改动 `engine/omnidesk_engine/` 的适配层，UI 与 Rust 宿主不变。
3. 生命周期单向：Python Engine 由 Rust 宿主启动与回收，不作为系统级常驻服务。
4. DSH Runtime 只读：不修改、不复制 `runtime/deepseek-harness/`。
5. 数据最小化：不把用户数据写入仓库；不把凭证写入日志/测试夹具/报告。

---

## 6. 当前限制与未验证项

| 项 | 状态 | 说明 |
| --- | --- | --- |
| `desktop/` 工程 | 已创建 | Tauri 2 + React + Rust 宿主；技术切片已验证，非正式产品化批准 |
| Rust 工具链 | 可用（经项目脚本） | 先执行 `eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)"` |
| Tauri Linux 构建依赖 | 本机可构建 | `.deb` 已实际产出；构建前提见下文 |
| Tauri 构建产物 | 部分通过 | `desktop/src-tauri/target/release/bundle/deb/OmniDesk_0.1.0_amd64.deb`（包名 `omni-desk`、版本 `0.1.0`）；AppImage 因 `linuxdeploy` 失败未产出；`.deb` 未安装 |
| Python Engine 子进程入口 | 已存在 | 由 Rust 宿主启动与回收；调用能力见引擎接口契约与验证报告 |
| Windows / macOS 行为 | `unverified` | 未实测，不声称支持 |

---

## 7. 与其它文档的关系

| 文档 | 关系 |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | 五层基线架构；本文是其"UI 层 = 独立桌面应用"的具体化 |
| [../GOALS.md](../GOALS.md) 与 [decisions.md](decisions.md) | 当前产品范围、决策和阶段状态；历史阶段报告保留于 tests/reports/ |
| [OmniDesk-Engine-Interface-Contract.md](OmniDesk-Engine-Interface-Contract.md) | Python Engine 对上层暴露的语义契约 |
| [OmniDesk-Desktop-Engine-IPC.md](OmniDesk-Desktop-Engine-IPC.md) | Step 3/4：Rust ↔ Python 的进程通信契约 |
| [../tests/reports/p2-desktop-baseline-report.md](../tests/reports/p2-desktop-baseline-report.md) | Step 1：边界与工具链就绪度证据 |
