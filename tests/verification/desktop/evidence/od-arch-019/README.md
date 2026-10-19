# OD-ARCH-019 证据索引：可编辑离线流程包与轻量试用记录

- 执行者：BUILDER；执行日期 2026-10-18。
- 状态：**BUILDER 已执行完毕，等待 ARCHITECT 独立复核。** 本目录仅记录执行事实，不构成验收结论。
- 工作区：`/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`。开工时 82 项未提交/未跟踪改动全部保留，暂存区为空；本轮未提交、未推送、未打标签，`REVIEW.md` 未修改。
- 隔离运行根：`/tmp/od019-evidence/run/od-dev-87ca1cd8`（仓库外，虚拟桌面 `Xvfb :97`）。原始流程包备份为该根下 `app-data/config/offline-flows.v1.original.json`。

## 证据等级说明

| 等级 | 含义 |
| --- | --- |
| 文件事实 | 直接读取的源码、配置或落盘文件内容 |
| 静态检查 | 类型检查、构建、diff 检查、常量检查 |
| 单元/集成测试 | `cargo test`、Python 验证脚本 |
| 隔离窗口实测 | 在隔离根启动的真实 Tauri 窗口内的可观察行为 |
| 未验证 | 本轮没有执行，不得视为通过 |

## 1. 流程包实现（文件事实 + 单元测试）

- `desktop/src-tauri/src/flow_pack.rs`（新增）：schema `omnidesk.offline-flows.v1`；运行时文件位于应用数据根的 `config/offline-flows.v1.json`；首次加载自动创建默认包，之后不覆盖用户修改；未知版本、未知字段、缺项、空字段、超长字段、URL/脚本/iframe 文本、相对目录一律整包拒绝并返回可读错误。
- `samples/workflows/offline-flows.v1.json`（新增）：三场景的标签、字段说明、占位文本、样例按钮文字、结果标题与章节名。
- `desktop/src-tauri/src/task_shell.rs`：新增 `*_with` 渲染函数读取流程包；原函数保留并委托默认包，避免既有测试与行为回退；日期校验、状态计数、逾期判定逻辑未改。
- `desktop/src-tauri/src/lib.rs`：注册 `flow_pack` 模块与 `offline_flow_pack` 命令；三个离线流程先加载流程包，失败时返回 `ok: false` 与 `pack_error: true`，不退回默认文案。
- `desktop/src/OfflineFlows.tsx`、`api.ts`、`types.ts`：三场景的标签、说明、占位符、样例按钮、结果标题改为读取流程包；显示流程包路径与错误，提供“重载流程包”；配置不可用时拒绝运行。
- `desktop/src/UsageLog.tsx`（新增）：可选记录完成、可用、返工类别、障碍、粗耗时区间与“是否愿意再用”；可查看、逐条删除、全部清空。

流程包的运行路径（隔离根内实测）：

```text
/tmp/od019-evidence/run/od-dev-87ca1cd8/
├── app-data/config/offline-flows.v1.json          # 首次启动自动创建
├── app-data/config/offline-flows.v1.original.json # 修改前备份
├── app-data/logs/usage.jsonl                      # 使用记录（逐条删除后为 0 字节）
└── local-data/offline-weekly-report-20261018-065807.md
```

## 2. 自动测试（实际运行）

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test --offline`（`desktop/src-tauri`） | 100 通过 / 0 失败（84 lib + 13 host_integration + 3 e2e） | [logs/rust-cargo-test.log](logs/rust-cargo-test.log) |
| `cargo run --offline --example evaluate_offline_scenarios` | 三场景判定通过；准入拒绝生效；日期边界正确 | [logs/offline-scenarios.log](logs/offline-scenarios.log) |
| `npx --no-install tsc --noEmit`（`desktop`） | 退出码 0 | [logs/tsc.log](logs/tsc.log) |
| `npm run build`（`desktop`） | 退出码 0 | [logs/frontend-build.log](logs/frontend-build.log) |
| `python3 tests/verification/desktop/verify_session_continuity.py` | 37/37 通过 | [logs/session-continuity.log](logs/session-continuity.log) |
| `python3 tests/verification/desktop/verify_engine_cancel.py` | 状态/IPC 34/34、静态 8/8，`RESULT: pass` | [logs/engine-cancel.log](logs/engine-cancel.log) |
| `git diff --check HEAD` | 退出码 0 | — |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0，无差异 | — |

新增 Rust 单元测试 6 项（`flow_pack` 模块）：默认包合法、首次创建且不覆盖用户修改、拒绝未知版本、拒绝缺项与坏 JSON、拒绝未知字段与外链、拒绝相对配置目录。另新增 1 项测试验证“改流程包会改变标题与章节，但统计结果一致”。使用记录方面新增 1 项“旧记录缺少 `reuse_willingness` 字段仍可读取与删除”的兼容测试。

## 3. 隔离窗口实测（合成数据）

窗口截图见 [shots/](shots/)。以下为已逐张核对过的关键证据。

| 场景 | 证据 | 结果 |
| --- | --- | --- |
| 流程包首次创建 | 根目录 `app-data/config/offline-flows.v1.json` 落盘，界面显示流程包路径（[window-29.png](shots/window-29.png)） | 通过 |
| 修改周报标签与结果标题后重载 | 界面标签不变（默认名）但结果标题变为“自定义周报标题（流程包生效验证）”（[window-10.png](shots/window-10.png)、[window-22.png](shots/window-22.png)） | 通过 |
| 周报合成样例 | 4 项；done 2 / pending 1 / blocked 1；明细表与“需要人工确认”章节正确（[window-10.png](shots/window-10.png)） | 通过 |
| 成果保存到仓库外 | `local-data/offline-weekly-report-20261018-065807.md`，999 字节（[window-14.png](shots/window-14.png)） | 通过 |
| 成果列表与只读重开 | 列表显示文件名、字节数与场景；重开内容与落盘文件一致（[window-14.png](shots/window-14.png)） | 通过 |
| 使用记录写入 | 记录一次后出现一行摘要，字段只有时间/场景/完成/可用/返工/障碍/耗时/愿意再用（[window-20.png](shots/window-20.png)、[window-21.png](shots/window-21.png)） | 通过 |
| “愿意再用”字段 | 勾选后写入 `"reuse_willingness":true`，表格“愿意再用”显示“是”；测试后逐条删除，`usage.jsonl` 回到 0 字节（[window-57.png](shots/window-57.png)、[window-59.png](shots/window-59.png)、[window-62.png](shots/window-62.png)、[window-63.png](shots/window-63.png)） | 通过 |
| 使用记录逐条删除 | 点“删除”后列表清空，`usage.jsonl` 为 0 字节 | 通过 |
| 非法流程包（未知版本） | 界面顶部与红色告警同时显示可读错误，未回退默认文案（[window-24.png](shots/window-24.png)、[window-27.png](shots/window-27.png)） | 通过 |
| 恢复有效流程包 | 修正 schema 后点击“重载流程包”，告警消失，流程恢复可用（[window-29.png](shots/window-29.png)） | 通过 |
| 项目跟进合成样例 | 4 项；done 1 / in_progress 1 / blocked 1 / not_started 1；逾期 2；列出受阻与逾期待确认（[window-36.png](shots/window-36.png)、[window-37.png](shots/window-37.png)） | 通过 |
| 修改项目/文案标签与标题后重载 | 标签变为“项目跟进（自定义）”“文案结构（自定义）”，项目结果标题为“自定义项目汇总标题”（[window-30.png](shots/window-30.png)、[window-41.png](shots/window-41.png)） | 通过 |
| 一般文案结构（自定义标题） | 结果标题为“自定义文案结构标题”；正文写明“未使用模板（没有与用途匹配的模板，按用途组织）”，并列出“待确认”项（[window-45.png](shots/window-45.png)、[window-46.png](shots/window-46.png)） | 通过 |
| 模型步骤标注 | 所有场景均显示“受准入阻塞、未验证：本次未调用任何模型”（各结果页截图） | 通过 |
| 成果标题跨场景残留（本轮新发现并修复） | 修复前切换场景后仍保留上一场景标题（[window-42.png](shots/window-42.png)、[window-46.png](shots/window-46.png)）；修复后切场景重跑标题为空（[window-49.png](shots/window-49.png)、[window-50.png](shots/window-50.png)） | 已修复并复测 |

## 4. 使用记录字段白名单（代码静态检查 + 运行证据）

`desktop/src/types.ts` 的 `UsageRecord` 只含：

```text
record_id, schema, task_type, recorded_at, completed, usable, rework_kind, blocker, duration_bucket, reuse_willingness
```

不含输入/输出正文、标题、文件路径、会话标识、模型名或凭证。Rust 侧对应测试：

- `usage_record_has_no_free_text_fields`：拒绝白名单以外的字段。
- `usage_record_rejects_invalid_enums`：只接受枚举值。
- `usage_append_read_delete_roundtrip_stays_outside_repo`：写入、读取、删除往返都落在仓库外。
- `usage_record_without_reuse_field_still_reads`：旧记录缺少 `reuse_willingness` 时默认为 `false`，仍可读取与删除。
- `usage_records_cannot_be_written_into_repo`：拒绝把记录写进仓库目录。

运行期实测写入的一条记录原文（删除前，已核对不含正文）：

```json
{"record_id":"20261018-071814-0000","schema":"omnidesk.usage.v1","task_type":"weekly_report","recorded_at":1792307894.7844074,"completed":true,"usable":true,"rework_kind":"none","blocker":"none","duration_bucket":null,"reuse_willingness":true}
```

## 5. 边界核对

| 项 | 证据 | 结果 |
| --- | --- | --- |
| 端点准入常量 | `desktop/src/engineAdmission.ts:16`、`desktop/src-tauri/src/admission.rs:15` 均为 `false` | 未变 |
| DSH Runtime | `git diff --exit-code HEAD -- runtime/` 退出码 0 | 无改动 |
| 用户数据与凭证 | 未读取、未打印、未复制；未触碰 `~/.dsh`、`~/.agents` | 未触碰 |
| 真实工作材料 | 本轮所有输入均为合成样例 | 未使用 |
| 模型调用 | 三场景均标注未调用模型 | 未调用 |
| 凭证/密钥特征扫描 | 对新增/修改源码与样例做 `sk-`、`Bearer`、`api_key`、`password` 模式扫描 | 无命中 |
| 提交状态 | `git status` 显示改动仍未提交，暂存区为空 | 未提交 |

## 6. 未验证事项

1. 真实模型撰写未验证：端点准入仍为 `false`，三场景的模型步骤只显示受准入阻塞。
2. 真实工作材料未验证：本轮只用合成样例。
3. 跨平台（Windows/macOS）未验证，仅在 Linux 虚拟桌面 `Xvfb :97` 下运行。
4. 长周期使用记录的真实可用性未验证：只用一条合成记录走完写入、展示与删除；旧格式兼容仅由单元测试覆盖。
5. 流程包只覆盖现有离线流程的表达层；不代表支持任意工作流、脚本或模型调用。
6. 使用时长依赖使用者自行判断，区间值本身不精确。
