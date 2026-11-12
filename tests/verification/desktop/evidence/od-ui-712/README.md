# OD-UI-712 / 713 证据

- 日期：2026-11-13
- 基线 HEAD：`5af890a84333a50a743347f45a66c9642b1dcf05`（工作区有既有未提交改动，本次未提交、未推送）
- 改动：对话时间线改为「思考 / 命令」两组，收起时各显示最新一条，展开显示该组全部条目；本轮生成的成果文件以可点击链接呈现，点击用系统默认程序打开。

## 设计参考（OD-ARCH-035）

用 screenshots-design-inspiration 搜了同类产品的对话与折叠布局（ChatGPT Web、Airtable Chat、Bloomberg Terminal 的 Accordion 列表）。借鉴要点：过程信息收成单行低对比度摘要，正文优先；展开动作交给行尾箭头，层级只靠缩进与小号次数区分。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `desktop/src/shell/HomeChat.tsx` | `TimelineGroup`（思考/命令两组，默认收起，单选展开）；`ArtifactLinks`（合并文本路径与本轮生成文件，点击调用 `shell.openLocalFile`）；成果目录从 `localRootInfo().artifact_root` 读取 |
| `desktop/src/theme.css` | `.od-timeline-group-body`、`.od-timeline-status`、`.od-timeline-tally`，沿用既有色板与圆角 |
| `desktop/src/shell/artifactLinks.ts` | 纯函数：只认成果目录内的绝对路径，跳过代码块/行内代码，扩展名白名单，去重与上限 8 |
| `desktop/src/api.ts` | `openLocalFile`、`listGeneratedFiles` |
| `desktop/src-tauri/src/artifact_open.rs` | 校验成果目录内普通文件后交给系统默认程序；拒绝 `..`、目录外与目录本身 |
| `desktop/src-tauri/src/lib.rs` | 新命令 `task_open_local_file`、`task_list_generated_files`（按任务时间窗列出生成文件元数据） |
| `tests/verification/desktop/verify_ui_timeline_711.py` | 断言改为分组收起/展开结构 |
| `tests/verification/desktop/verify_chat_artifact_links_713.py` | 新增：链接识别纯函数 + 前后端接线检查 |

## 命令与结果

| 命令 | 结果 |
| --- | --- |
| `npx tsc --noEmit` | exit 0 |
| `npm run build` | exit 0 |
| `verify_ui_timeline_711.py` | 53/53 |
| `verify_chat_artifact_links_713.py` | 28/28，含纯函数用例（去代码块、去重、目录外忽略、扩展名白名单、上限 8） |
| `cargo test --offline --lib artifact_open` | 3/3（相对/绝对路径通过；目录外与 `..` 拒绝；目录与缺失文件拒绝） |
| 合成模型探针（`_mock_model_server`，profile=discipline，/tmp 隔离根） | 任务 succeeded；`tool/call` 载荷 `tool_name=bash`、标题长度 4；`tool/result` 无名并归到同一命令 |
| `npm run tauri:build -- --bundles app` | rc=0；425.54 MiB |

## 未验证

- 真实模型下的收起/展开视觉与首次生成文件的点击打开（需用户实测；渲染逻辑由 tsc/build 与静态断言覆盖）。
- 历史消息里若既未写路径、也不在本轮时间窗内（例如换机器后），不会出现链接。
- 任务时间窗按消息提交/结束时刻 ±2 秒过滤，同一窗口内的其它写入会被一并列出。

## 说明

- 工具名「未知工具」只在引擎没给展示字段时出现；合成探针确认当前 `web-api` 通道的 `tool/call` 带工具名。
- 时间线数据仍只存展示字段（工具名/状态/思考文本），不存工具入参与结果。
- 打开文件只接受成果目录内的普通文件，由系统默认程序处理，不拼 shell、不读正文。

## 修正记录（2026-11-13）

- 第一版复用 list_offline_artifacts 列本轮文件，实测返回 0：该函数只认 OmniDesk 自己的 offline-<场景>-<时间戳>.md。改为 artifact_open::list_recent_files 专用遍历（深度 4、扫描上限 4000、跳过隐藏与临时文件，按修改时间倒序、上限 8）。
- 界面探针确认 root=/Users/yvan/OmniDesk/workspace/artifacts、时间窗与消息一致、旧函数 items=0；探针已移除。
- 桌面在 02:30 后进入锁屏，收起/展开与链接点击未能取得最终截图；构建产物已含修复，解锁后打开应用即可看到该条消息下方的文件链接。
