# OD-ARCH-046 证据索引：写确认制 / 自动降级 / 定时·心跳 / Agent 式设置页

- 执行日：2026-10-21。开工 HEAD：`22e53d3`（`main`）；未提交、未推送、未打标签。
- 未改 `runtime/`；未碰 `~/.dsh`、`~/.agents`、凭证与模型路由（未读取凭据内容）。
- 运行时全部在仓库外一次性目录：应用数据根 `/tmp/od046-evidence/run/app-data`，DSH 隔离根 `/tmp/od046-evidence/run/dsh-home`，成果目录 `/tmp/od046-evidence/run/local-data`（由 `OMNIDESK_LOCAL_DATA_ROOT` 覆盖，设置页对此有红字提示）。
- 模型端点为本机回环合成端点 `fake / synthetic-model`（`127.0.0.1:34621`，一次运行中被换过端口），不含任何真实模型。
- 证据等级：**文件事实**、**静态检查**（tsc / cargo build）、**脚本运行**（真实进程、真实窗口、真实宿主命令与文件读写）、**未验证**。

## 1. 设置页五张卡片

| 截图 | 观察 | 证据等级 |
| --- | --- | --- |
| `shots/01-settings-model-endpoint.png` | 「模型与端点」显示 `fake / synthetic-model`、端点 `http://127.0.0.1:.../v1`、类别「本机回环」、判定依据「IPv4 回环地址 127.0.0.0/8」、当前判定随权限实时变化 | 脚本运行 |
| `shots/02-settings-artifact-root.png` | 「成果目录」显示当前生效路径与来源；隔离变量生效时出现红字「当前是开发/测试隔离实例…此处保存的选择等隔离变量取消后才成为实际生效目录」 | 脚本运行 |
| `shots/03-settings-data-logs.png` | 「数据与日志」列出应用数据根 / 设置 / 使用日志 / 调度配置 / 待确认写入，全部位于仓库外 | 脚本运行 |
| `shots/04-settings-schedule-card.png` | 「定时与提醒」显示真实配置：AI 复盘「仅一次 9:00」、心跳「每 5 分钟」，并写明只在应用运行期间触发 | 脚本运行 |

权限开关写入后宿主状态确实反转：`shots/10-permission-off-denied.png` 中三档全关、当前判定「拒绝提交」；重新开启后恢复「允许提交」（同目录 `shots/13-auto-review-online-pending.png` 底部状态栏「模型权限：已开启」）。配置文件 `run/app-data/config/settings.v1.json` 与 `scheduler.v1.json` 均为真实读写产物。

## 2. 定时触发与写确认制

| 截图 | 观察 | 证据等级 |
| --- | --- | --- |
| `shots/05-schedule-auto-fired.png` | 前端 30 秒 tick 捕获到 `once` 到点，自动发起一次复盘；`scheduler.v1.json` 的 `last_review_at` 由 0 变为非 0 | 脚本运行 |
| `shots/06-pending-card.png` | 复盘正文只进入「待确认写入」卡片，标注来源「定时任务发起」与影响范围；此时成果目录**没有**新文件 | 脚本运行 |
| `shots/07-confirmed-written.png` | 点「确认写入」后，成果档案出现 `weekly-report/offline-weekly-report-…-015739.md` 与 `…-015859.md` | 脚本运行 |
| `shots/08-confirm-receipt.png` | 回执显示「已写入 /tmp/od046-evidence/run/local-data/weekly-report/…-015859.md」，记录随后从待确认列表消失 | 脚本运行 |

文件事实佐证（同一时刻的目录列举）：

- 触发前 `run/local-data` 只有测试夹具 `offline-weekly-report-20261021-120000.md`，`weekly-report/` 不存在。
- 触发后新增的待确认记录 `run/app-data/engine-state/pending-writes/pw-*.json` 状态为 `pending`，含正文与 `expires_at`（24 小时后失效）。
- 确认后 `pending-writes` 中对应记录变为 `consumed`，文件出现在 `local-data/weekly-report/`；人工手动保存仍落数据根顶层（OD-ARCH-045 偏差的修正只作用于模型/定时写入）。

单测证据（脚本运行）：`cargo test pending_writes` → 5 passed，覆盖「一次性消费」「过期拒绝」「负载与记录不一致拒绝」「人工来源与未知动作拒绝」「放弃后落定」。

## 3. 自动降级（不产模板）

| 截图 | 观察 | 证据等级 |
| --- | --- | --- |
| `shots/09-degrade-model-down.png` | 合成端点随脚本退出后，公告栏出现「AI 暂不可用（任务 failed），已用本地规则结果。」与「复盘未成功（failed），没有写入建议文件。」；无任何新文件 | 脚本运行 |
| `shots/12-draft-refused-no-template.png` | 权限全关时在周报页发起起草：助手区显示 `Error.endpoint_unconfirmed`，明示「已拒绝准备工作目录。未创建任何目录、未启动引擎，输入未发送到任何模型。」；成果目录文件数保持 3 不变 | 脚本运行 |

两次检查都用同一时刻的 `find run/local-data -type f | wc -l` 前后对比（3 → 3），并核对 `pending-writes` 记录数未增加。

## 4. 权限关闭时定时任务不执行

| 截图 | 观察 | 证据等级 |
| --- | --- | --- |
| `shots/11-schedule-skipped-permission-off.png` | 权限全关后人为把 `once_at` 改到过去并清零 `last_review_at`，等待多轮 tick：公告栏出现「AI 暂不可用（模型权限未开启），已用本地规则结果。」与「定时复盘未执行：模型权限未开启（复盘会调用模型，必须先到设置里开启对应类别）。」；`last_review_at` 保持 0，待确认记录数与文件数均未变 | 脚本运行 |

## 5. 心跳

- 心跳到点只刷新公告栏的本地规则提醒，并写 `last_heartbeat_at`；`run/app-data/config/scheduler.v1.json` 中该值在 5 分钟间隔下真实推进（脚本运行 + 文件事实）。
- `scheduler::heartbeat_scan` 与手动 `project_flow_with` 输出一致性由单测 `heartbeat_scan_matches_manual_scan` 覆盖（脚本运行），未在窗口中单独截图。
- 未验证：窗口侧对「心跳前后规则提醒文本一致」的逐字比对。

## 6. 回归

| 检查 | 结果 | 等级 |
| --- | --- | --- |
| `cd desktop && npx tsc --noEmit` | 0 错误，exit 0 | 静态检查 |
| `cd desktop && npm run build` | 成功，`built in 551ms`（最后一次代码改动之后运行） | 静态检查 |
| `cargo test --offline --manifest-path desktop/src-tauri/Cargo.toml` | lib **146 passed / 0 failed**；其余 target 2 / 13 / 3 / 1 passed，全 0 failed，exit 0 | 脚本运行 |
| `cargo build --offline` | 成功 | 静态检查 |
| `cargo fmt` | **未验证**：本机未安装 rustfmt | 未验证 |

## 7. 未验证与限制

1. 调度是「前端运行期定时器 + 宿主判定命令」，无常驻线程、无系统级定时；应用关闭期间不执行，错过则下次打开补跑一次。这是计划内的实现选择，但「应用长时间挂起后补跑」只在 once 场景实测过。
2. 待确认记录含待写正文，存放在应用数据根（仓库外），24 小时后失效；「过期后界面提示」只有单测证据，未在窗口中制造真实过期记录。
3. 取消/恢复语义属于引擎适配层，本轮未改；定时复盘任务若在等待中被取消，界面按 `failed/cancelled` 走降级分支，未单独截图。
4. 所有窗口证据都来自一次性隔离实例与合成端点，不代表真实模型的质量或真实业务效果。
