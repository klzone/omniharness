# OD-ARCH-045 证据索引：功能完善夜间批量任务（六步）

- 执行日：2026-10-21（夜间批）。开工 HEAD：`22e53d3`（`main`）。
- 全程未提交、未推送、未打标签；未改 `runtime/`、未接真实模型端点、未读取凭证。
- 所有运行时数据根都在仓库外的一次性目录 `/tmp/od045-evidence/*`，模型端点为本机回环合成端点 `127.0.0.1:42191`。
- 证据等级用词：**文件事实**（读源码/文件）、**静态检查**（编译与类型检查）、**脚本运行**（真实进程/真实窗口/真实 API 调用）、**未验证**。

## 1. 第 1 步：线程持久化到数据根

| 证据 | 内容 | 等级 |
| --- | --- | --- |
| `cargo test thread_store` | 8 passed / 0 failed | 静态检查 + 脚本运行 |
| `cargo test --test thread_store_restart` | 1 passed / 0 failed（重建存储＝模拟重启后仍读回，且断言仓库内不出现线程文件） | 脚本运行 |
| `/tmp/od045-evidence/step6v1/app-data/threads/_unassigned/tmvgntwski9t5.json` | 真实窗口产生的线程文件，落在仓库外数据根，含 12 条消息与 `engineSessionId` | 脚本运行 |
| `git status` | 无任何线程记录文件进入仓库 | 文件事实 |

## 2. 第 2 步：成果目录树与「含下级目录」开关

| 截图 | 内容 |
| --- | --- |
| `shots/02-writing-subdirs-off.png` | 文案页，开关关闭：只显示顶层成果 1 条 |
| `shots/03-writing-subdirs-on.png` | 同页，开关打开：出现 `weekly/` 目录节点及其子文件，可「打开」 |
| `shots/04-open-nested-artifact.png`、`05-open-nested-artifact-content.png` | 点开子目录成果，正文正确显示 |

判定：通过（脚本运行）。该步同时修复了两个 bug：场景键 `weekly_report` 与目录 `weekly-report` 不一致；顶部卡片被透明层挡住导致无法点选。

## 3. 第 3 步：首次提交不冻结、可取消

| 截图 | 内容 |
| --- | --- |
| `shots/25-abort-during-starting.png` | 冷启动首次提交后立即点 Dock，界面已切换，未冻结 |
| `shots/26-abort-settled.png` | 取消后落定：卡片显示「已放弃本次提交」+「引擎以 stopReason=cancelled 落定」，台账 `state=cancelled` |

判定：通过（脚本运行）。`09a/11/20/21/22/23/24` 及 `dbg-*.png` 是排查过程图，**不作为验收证据**。

## 4. 第 4 步：AI 复盘建议接通引擎

| 截图 | 内容 |
| --- | --- |
| `shots/27-review-bar-weekly.png` | 权限未开：按钮置灰并说明「需要模型权限」 |
| `shots/28-review-running.png` | 权限开启后点「立即复盘」，进入执行态 |
| `shots/29-review-after.png` | 完成：公告栏出现建议摘要，成果右栏出现新文件 |

脚本运行证据：合成端点收到 1 次 chat completion；台账终态 `succeeded / engine_terminal / end_turn`；新文件 `/tmp/od045-evidence/step4v2/local-data/offline-weekly-report-20261020-175229.md` 真实落盘。

**计划偏差（已登记，未修）：** 落盘位置是数据根**顶层**，而计划要求「存数据根该场景目录」。原因是 `task_shell.rs::save_offline_artifact` 一律写顶层。本项不能声称满足子目录要求。

## 5. 第 5 步：DSH 自带 Web 应用本地 API 只读验证

见同目录 `web-api-probe.md`（结论表）与 `web-api/*.json`（原始证据）。要点：本地 HTTP API、token→HttpOnly cookie、`session/modelCatalog`、`session/create`、`session/prompt`、token 级流式、同会话续接、`session/page` 读回、`session/cancel`（`reason.kind=aborted`，19ms）、进程重启后会话仍在，均为**脚本运行**证实存在。

**未验证：** 思考块流式（合成端点不产 reasoning 内容）、工具调用事件（只发纯文本）。

## 6. 第 6 步：整批回归与收尾截图

| 检查 | 结果 |
| --- | --- |
| `cargo test --offline` | **154 passed / 0 failed**，exit 0（各 target：135 + 0 + 2 + 13 + 3 + 1 + 0） |
| `npx tsc --noEmit` | clean，exit 0 |
| `npm run build` | exit 0，built in 1.12s |

| 截图 | 内容 |
| --- | --- |
| `shots/30-final-home.png` | 首页正常，Dock/卡片/对话区版式未变 |
| `shots/31-final-weekly.png` | 周报页：双栏 + 成果档案 + 历史对话标签可用 |
| `shots/32-final-copy.png` | 文案页：双栏 + 「趋势热点」未开放的说明 |
| `shots/33-final-project.png` | 项目跟进页：「进度 / 阶段成果」页签正常 |

四张收尾截图均已在真实窗口中目视核对。

## 7. 已知缺口（本轮不修）

1. **首页对话不写线程存储**：`HomeCanvas.tsx` 渲染 `HomeChat` 时未传 `onMessagesChange`，所以首页产生的对话不会落到数据根，只有场景页/模块页的线程会持久化。
2. **第 4 步复盘成果落在数据根顶层**，未按场景分目录（见 §4）。
3. **第 5 步思考块与工具事件未验证**（见 §5）。

## 8. 复现提示

第 5 步的复现命令与隔离实例参数见 `web-api-probe.md`；第 1、6 步的命令即上表所列。所有临时数据根都已是一次性目录，可整体丢弃，不影响 `~/OmniDesk` 与 `~/.dsh`。
