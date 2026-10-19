# OD-ARCH-021 证据索引（BUILDER 执行完成；ARCHITECT 复核部分通过）

本目录保存「可配置模型权限 + AI 任务闭环 + 桌面安装验收」的可复核证据。所有验证只使用纯合成数据与本地合成端点；未读取或写入真实 DSH 数据、用户材料或凭证，未改动 `runtime/`、模型路由和原版配置。

运行环境：仓库 `/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`；隔离运行根 `/tmp/od021-evidence/run/od-dev-87ca1cd8`；虚拟桌面 `Xvfb :98`；合成模型服务 `127.0.0.1:35146/35147/35149`。

## 结论摘要

| 项目 | 结果 | 证据等级 |
| --- | --- | --- |
| 端点分类与默认关闭 | 从 DSH 非秘密配置读出模型与 baseURL，按来源分类；首次全部关闭，界面显示类别、判定依据和当前判定 | 真实窗口运行 |
| 用户逐项开启 | 开启「本机模型」后当前判定变为「允许提交」，内网与外部保持关闭 | 真实窗口运行 |
| 权限持久化 | 关闭并重启应用后，本机模型仍为开启，其余仍关闭 | 真实窗口运行 + `settings.v1.json` |
| 权限撤销 | 撤销后提交被拒，提示引擎未启动、输入未发送 | 真实窗口运行 |
| 未知端点拒绝 | `baseURL` 指向不可判断地址时，类别显示「未知（去向无法判断）」，无可开启开关；提交报 `Error:endpoint_unknown`，无任务标识、无新成果、上游请求计数未增加 | 真实窗口运行 + 文件事实 |
| 禁入内容拒绝 | 含模拟敏感词的合成输入在调用引擎前被红线拒绝，提示改用单位合规系统 | 真实窗口运行 |
| 合成 AI 成功闭环 | 统计 → 引擎生成解释（流式）→ 人工确认 → 保存到隔离本地目录 | 真实窗口运行 |
| 引擎失败 | 上游 400 时任务进入 `failed`，界面显示错误，解释框为空，保存不可用，无新成果 | 真实窗口运行 + 状态文件 |
| 取消 | 挂起端点下提交后可发起取消，终态 `cancelled`、`stop_reason=cancelled`，结果为空，成果数不变 | 真实窗口运行 + 状态文件 |
| 安装包 | `.deb` 产出成功（`omni-desk` 0.1.0，amd64，2.46 MiB），含 32–512 图标、`OmniDesk.desktop`、`usr/bin/omnidesk-desktop`；解包后启动首页正常渲染 | 构建产物 + `dpkg-deb` + 真实窗口运行 |
| 真实模型质量 | **未验证**：按计划未调用真实供应商端点 | 未执行 |

## 截图

| 文件 | 内容 |
| --- | --- |
| `shots/01-home-default-off.png` | 首次启动，端点未开放 |
| `shots/02-permissions-panel.png` | 模型权限区块，三类均关闭 |
| `shots/03-permission-enabled.png` | 开启本机模型后「当前判定：允许提交」 |
| `shots/04-ai-reply.png` | 确定性统计结果（合计 204、平均 40.8，本地计算） |
| `shots/04a-ai-stream-and-source.png` | 引擎流式预览与「模型输出 未复核」来源标注 |
| `shots/05a-confirm-gate.png` | 未勾选人工确认时保存按钮禁用 |
| `shots/05-saved.png` | 保存成功，显示路径、1714 字节与写入时间 |
| `shots/06-revoked.png` | 撤销权限后的状态 |
| `shots/06a-cancel-submitted.png`、`06b-cancel-requested.png`、`06c-cancel-result.png` | 取消前、取消请求、终态「已取消」 |
| `shots/07-engine-failure.png` | 上游 400 失败，任务 `omni-39209d3a…`，解释框为空 |
| `shots/08-unknown-endpoint-settings.png`、`09-unknown-endpoint-class.png` | 未知端点设置页与类别 |
| `shots/10-statistics.png` | 未知端点下仍可用的本地统计 |
| `shots/11-unknown-refused.png` | `Error:endpoint_unknown`，明确「未启动引擎，输入未发送」 |
| `shots/12-permission-persisted.png` | 重启后本机模型仍开启 |
| `shots/13-redline-refusal.png` | 红线扫描拒绝，说明为启发式、不构成合规判断 |
| `shots/14-deb-unpacked-launch.png` | 从 `.deb` 解包目录启动的首页 |

## 关键运行事实

- 失败任务 `omni-39209d3a-793b-4921-a626-3627884eb301` 状态文件：`state=failed`、`evidence_source=engine_error`、`content=""`，错误为合成上游 400。
- 取消任务 `omni-a5d0a18c-eb62-4ef3-8928-326c36578abc` 状态文件：`state=cancelled`、`stop_reason=cancelled`、`content=""`；取消后 `local-data` 成果数保持 11 份不变。
- 未知端点提交前后，合成错误端点请求计数保持 1（此前失败测试那次），未新增任务文件与成果文件。
- 权限持久化文件：`app-data/config/settings.v1.json`，内容为 `local: true`、`internal: false`、`external: false`。

## 未验证与限制

1. **真实模型未调用**：所有生成均来自本地合成端点，输出质量、真实供应商行为与费用未验证。
2. **半截流 UI 状态未在窗口内观察到**：`E19` 只由适配器脚本验证（截断流），真实窗口未复现中途断流。
3. **头像点击卡死**：在该 WebKit/Xvfb 环境下点击右上头像稳定触发无响应（CPU 100%），临时禁用模糊样式未解决，`dist/index.html` 已恢复；同一前端在 Chrome 中正常。属于 Tauri/WebKit 渲染问题，根因未查明，未修改源码。
4. 仅 Linux 虚拟桌面；Windows/macOS 未验证。AppImage 未产出（`linuxdeploy` 阶段失败）。`.deb` 只做了解包启动，未在真实桌面安装。
5. 红线扫描是启发式关键词检查，不是合规判断，也不等于授权或脱敏保证。
6. 设置中的成果目录为路径输入框，仍未使用原生目录选择器。

## 回归命令与结果（本轮实测）

| 命令 | 结果 |
| --- | --- |
| `npx --no-install tsc --noEmit` | 退出码 0 |
| `npm run build` | 成功 |
| `cargo test --offline` | 98 lib + 2 admission + 13 host_integration + 3 e2e，全部通过 |
| `dump-model-route.mjs --self-test` | 18/18，0 失败，`RESULT: pass`（ARCHITECT 独立复跑；BUILDER 原记录误写为 22） |
| `verify_ai_draft.py` | 22/22 |
| `verify_session_continuity.py` | 37/37 |
| `verify_engine_cancel.py` | 34/34 + 8/8 |
| `run_desktop_e2e.py chain` | 成功/失败/取消均符合预期 |
| `run_engine_adapter_scenarios.py E19` | pass（截断流） |

上述命令在最后仅做证据整理与文档修改之前执行；此后未改动产品源码。
