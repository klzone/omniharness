# OmniDesk P1-Desktop Phase 2 · Step 17 报告：桌面引擎技术切片验收

- 报告日期：2026-10-16
- 阶段：OmniDesk P1-Desktop Phase 2（独立桌面技术切片）
- 对应用户 Plan Step 17：「汇总桌面技术切片的实施结果和所有运行证据」
- 证据等级约定：`static`（文件/源码事实）· `script`（脚本或进程运行） · `model`（真实模型输出） · `unverified`（未验证）

> 本报告只汇总**已被本轮或历史运行证据支持**的结论。凡未实际运行的项一律标 `unverified`；不以设计文本推断运行行为，不把静态检查当作运行测试。

---

## 0. 结论摘要

1. **桌面技术切片走通**：React → Tauri/Rust → Python Engine（`python3 -m omnidesk_engine` 独立子进程）→ DSH ACP（真实 `bin.js --profile acp`）→ 本机合成模型端点 → 回传到桌面宿主。成功、失败、取消三条链路的真实事件与终态均已留证。
2. **数据隔离成立**：桌面测试前后，`~/.dsh`、`~/.agents`、`~/.config/DSH Desktop`、`~/.local/share/io.github.hairyf.deepseek-harness-desktop` 四项受守护路径**零差异**；测试产生的新文件只在 `/tmp`。
3. **双实例隔离成立**：两个一次性实例各有独立 user-data 根与独立引擎进程；A 的任务对 B 返回标准 `unknown_task`。
4. **桌面窗口是 OmniDesk 自有身份**：真实窗口截图 + 构建产物扫描确认不加载 DSH Desktop 页面或资源，不依赖 DSH Web Server。
5. **Rust 采用范围（阶段 2 的 go/revise/stop 建议）**：本切片中实际需要原生宿主的职责是**子进程生命周期管理**与**本机 IPC 通道**——这两类由 Rust 承担并被 23 个自动化测试覆盖。UI 与状态呈现留在 React/TypeScript，AI 逻辑留在 Python。**建议 go（保留 Tauri 2 + Rust 宿主 + React/TS UI + Python 引擎的四层分工）**，但 Rust 代码面应继续保持当前的小范围（见 §6）。
6. **本阶段无未通过的验收项**；存在若干**未验证/未覆盖**边界（恢复语义、真正的 token 级流式、跨平台），已如实列入 §7。

---

## 1. 分层与调用链（`static`）

    React / TypeScript（UI 只读身份：OmniDesk 自有页面）
         │  IPC-1：Tauri 命令（invoke）
    Rust 宿主（进程生命周期 + IPC + 事件泵；不承载 AI 逻辑）
         │  IPC-2：行分隔 JSON-RPC 2.0 over stdio（进程边界）
    Python Engine（engine/omnidesk_engine/，AI 工作逻辑 + 契约归一化）
         │  ACP v1（JSON-RPC over stdio）
    DSH CLI（runtime/dsh-runtime/.../bin.js --profile acp）——只作执行引擎

Rust 宿主侧源码（`static`）：`desktop/src-tauri/src/{main.rs,lib.rs,app_paths.rs,engine_ipc.rs,engine_supervisor.rs,engine_bridge.rs}`。
Tauri 命令（`static`）：`engine_describe / engine_health / engine_submit_task / engine_task_status / engine_task_result / engine_cancel_task / engine_shutdown / app_paths_info`；事件通道 `omni://task-event`。

---

## 2. Step 12：任务完整链路（`script`）

驱动脚本：`python3 tests/verification/desktop/run_desktop_e2e.py`（本轮实跑，`exit 0`，约 21 s）。
证据文件：`tests/verification/desktop/evidence/task-chain.json`。

真实链路：driver（模拟 React 侧发命令）→ IPC-2 → `python3 -m omnidesk_engine`（真实子进程）→ ACP → 真实 DSH `bin.js --profile acp` → 本机回环**合成模型端点**（零凭证、零外网）。

### 2.1 场景结果

| 场景 | 终态 | 关键事件 | 结果 | 判定 |
| --- | --- | --- | --- | --- |
| 成功 | `succeeded` | `task_started → text_delta → progress → task_completed` | `form=complete`，`content="合成材料草稿"`，`text_delta_joined` 与结果一致 | pass |
| 失败 | `failed` | `task_started → task_failed` | 合成上游 400 经引擎归一化为 `engine_error_code=task_failed`，**未**被报成成功 | pass |
| 取消 | `cancelled` | `task_started → cancel_requested → task_cancelled` | 取消阶段 `confirmed_cancelled`，`stop_reason=cancelled`；取消前 `running_before_cancel=true` | pass |

### 2.2 引擎能力声明（来自真实 `omni.initialize`，`script`）

| 能力 | 声明值 |
| --- | --- |
| `engine_kind` | `dsh-acp` |
| `engine_version` | `0.0.1` |
| `protocol_version` | `acp/1` |
| `compatibility` | `compatible` |
| `submit_task` / `get_task_status` / `stream_events` / `cancel_task` / `recover_session` / `get_task_result` | `implementation_supported` |
| `recover_task` | **`partial`** |

> `recover_task=partial` 与历史实测一致：进程重启后在途任务只能标为 `unknown`，不自动重放。

---

## 3. Step 13：数据隔离（`script`）

证据文件：`tests/verification/desktop/evidence/isolation-report.json`。

| 受守护路径 | before == after | 差异数 |
| --- | --- | --- |
| `/home/kevin/.dsh` | 是（快照有界，截断于 4000 项） | 0 |
| `/home/kevin/.agents` | 是 | 0 |
| `/home/kevin/.config/DSH Desktop` | 是 | 0 |
| `/home/kevin/.local/share/io.github.hairyf.deepseek-harness-desktop` | 是 | 0 |

- `unchanged=true`，`differences=[]`，`verdict=pass`。
- `test_roots_only_in_tmp=true`，`stray_roots=[]`：测试数据只出现在 `/tmp`。
- **限制**：`~/.dsh` 快照为**有界**快照（截断于 4000 项），无法排除超出该上限的极深层变化；`.agents` 为空目录。

---

## 4. Step 15：双实例隔离（`script`）

证据文件：`tests/verification/desktop/evidence/dual-instance-report.json`。

| 检查项 | 结果 |
| --- | --- |
| 两实例均初始化成功 | ✅ `both_initialized=true` |
| PID 互异 | ✅ `pid_a=403048`，`pid_b=403051` |
| 根目录不相交 | ✅ `roots_disjoint=true` |
| A 的任务对 B 不可见 | ✅ `b_can_see_a_task=false`，B 查到标准错误 `unknown_task` |
| A 能看到自己的任务 | ✅ `a_can_see_a_task=true`（`status=running`） |

6/6 检查通过，`verdict=pass`。

---

## 5. Step 14：独立窗口身份（`script` + `static`）

详见 [tests/verification/desktop/evidence/ui-identity-report.md](../verification/desktop/evidence/ui-identity-report.md)，截图证据 `tests/verification/desktop/evidence/ui-identity-window.png`。

- 窗口标题 `OmniDesk Discipline`，窗口类 `omnidesk-desktop`，1280 × 820，与 `tauri.conf.json` 一致。
- 截图显示 OmniDesk 自有三栏 UI（任务 / 事件流 / 输出）与品牌标记，无 DSH Desktop 元素。
- 构建产物扫描：唯一 DSH 相关字符串是折叠诊断面板的「DSH 数据根」/ `dsh_home`，属只读诊断展示。
- CSP `connect-src` 仅 `self` + 本地 IPC，不允许连 DSH 端口。

---

## 6. Step 16：桌面层自动化测试（`script`）

命令：`cargo test`（在 `desktop/src-tauri`），本轮实跑 **23 passed / 0 failed**，约 3.1 s。

| 测试组 | 数量 | 覆盖 |
| --- | --- | --- |
| 单元测试（`src`） | 10 | `engine_ipc` 收发/通知/超时/错误传播/未知方法；`app_paths` 路径边界与默认根 |
| 集成测试（`tests/host_integration.rs`） | 13 | Supervisor 启停、启动前不可用、拒绝缺失 CLI/相对路径/原版 DSH 根/原版 agents 根，IPC 请求-响应往返、通知接收、超时非成功、退出后请求失败、引擎错误带五属性传播、未知方法返回 JSON-RPC 错误 |

> 单元测试名示例（`static` 计数来自源码）：`transport_error_is_engine_unavailable`、`rejects_stock_dsh_home`、`rejects_dsh_source_tree` 等。

### 6.1 Rust 采用范围建议（阶段 2 的 go / revise / stop）

| 职责 | 归属 | 依据 |
| --- | --- | --- |
| 子进程启动/关闭/超时/强杀 | **Rust** | `engine_supervisor.rs`；覆盖 `supervisor_start_initialize_and_stop` 等 |
| 本机 IPC 通道（stdio JSON-RPC） | **Rust** | `engine_ipc.rs`；覆盖往返/通知/超时/错误 |
| 引擎事件泵 → UI | **Rust** | `engine_bridge.rs`；事件通道 `omni://task-event` |
| 路径边界强制 | **Rust** | `app_paths.rs`；`assert_isolated()` 硬拒绝 |
| UI 与状态呈现 | **TypeScript / React** | `desktop/src/` |
| AI 工作逻辑 | **Python** | `engine/omnidesk_engine/` |

**建议：go。** 继续用 Tauri 2 + Rust 宿主 + React/TS UI + Python 引擎的四层分工。Rust 的价值集中在**原生进程生命周期与安全边界**（这两类在 TS/Node 里同样可行，但当前实现小、测试覆盖完整、无额外维护负担）。Rust 代码面应继续保持当前规模，不把业务逻辑搬进 Rust。

---

## 7. 未验证 / 未覆盖项（`unverified`）

| 项 | 状态 | 说明 |
| --- | --- | --- |
| 真实模型输出的桌面链路 | `unverified` | 本轮桌面链路用**合成模型端点**；真实模型端到端已在 Web UI 路径验证（见 `phase2-webui-writing-e2e-report.md`），但**未经桌面外壳**跑通 |
| 崩溃恢复 `recover_task` | `unverified` | 能力声明为 `partial`；仅历史脚本测到"重启后在途任务标 `unknown`、不自动重放" |
| 真正的 token 级流式 | `unverified` | ACP 在整轮结束一次性投递 assistant 文本（消息级） |
| 窗口内完整链路可视化 | `unverified` | 一次窗口内跑全链路的截图渲染为空白窗口，原因未定位（不影响 Step 14 判定） |
| 跨平台（Windows / macOS） | `unverified` | 全部证据仅 Linux + Xvfb |
| `stopReason=end_turn` 区分 completed/blocked | `unverified` | 已知语义缺口 |

---

## 8. 本轮为完成验收所新增/修改的文件

| 文件 | 变动 |
| --- | --- |
| `docs/OmniDesk-Desktop-Data-Layout.md` | 新增（Step 6） |
| `tests/verification/desktop/evidence/ui-identity-report.md` | 新增（Step 14） |
| `tests/verification/desktop/evidence/ui-identity-window.png` | 新增（Step 14 截图证据） |
| `tests/verification/desktop/evidence/{task-chain,isolation-report,dual-instance-report}.json` | 本轮重跑刷新（Step 12/13/15） |
| `tests/reports/p2-desktop-engine-slice-report.md` | 新增（本报告，Step 17） |
| `tests/reports/p2-desktop-baseline-report.md` | 修订 §5/§8（工具链阻塞已解决）与工作区计数 |
| `docs/OmniDesk-Product-and-Architecture-Roadmap.md` | 更新阶段 2 实际状态（Step 18） |

未修改 `runtime/deepseek-harness/` 与 `runtime/dsh-runtime/`；未读取/打印/复制任何凭证；未提交、未推送、未打 Tag。

---

## 9. 验收标准自检（Step 17）

| 验收要求 | 结果 |
| --- | --- |
| 区分文件事实 / 静态检查 / 脚本运行 / 真实桌面运行 | ✅ 全文以 `static`/`script`/`model`/`unverified` 标注 |
| 所有通过项都有证据文件 | ✅ 见 §2–§6 的证据路径 |
| 未完成项明确列出 | ✅ 见 §7 |

**结论：Step 17 验收通过。**
