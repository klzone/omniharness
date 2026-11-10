# OD-UI-711 前端执行时间线：执行证据

日期：2026-11-11
基线 HEAD：`149df677dba6ced26d522415f83de6bb00c4c295`

## 基线工作区

开工时 `git status --short` 已有 4 项未提交/未跟踪改动：

- `M REVIEW.md`（OD-UI-710 复检记录）
- `M engine/omnidesk_engine/web_api_adapter.py`、`M tests/verification/engine-interface/adapter/test_web_api_unit.py`（710 引擎事件层交付）
- `?? tests/verification/desktop/od-ui-710/`（710 证据）

以上均原样保留；本任务未改 `runtime/`、未改 710 的引擎侧文件。

## 验收命令（本轮实跑）

| 命令 | 结果文件 | 结果 |
| --- | --- | --- |
| `cd desktop && npx tsc --noEmit` | `tsc.txt` | exit 0，无输出 |
| `cd desktop && npm --prefix . run build` | `build.txt` | exit 0，Vite `built in 8.24s`；仅有既有 >500 kB 分块提示 |
| `python3 tests/verification/desktop/verify_ui_timeline_711.py` | `verify.txt` | 50 项通过，0 项失败 |

`verify_ui_timeline_711.py` 内含：`timelineModel.ts` 单独编译后由 Node 执行的纯函数行为检查（追加、归并、思考累积、unknown_chunk 聚合、摘要、时间格式、输入不可变），以及 HomeChat 渲染顺序、展开语义、旧消息回退、runtime 零改动、时间线代码无自动停止调用的源码检查。

## UI 参考要点

来源：`screenshots-design-inspiration` MCP，检索 `AI agent chat`、`terminal activity feed`，实看 AgentGPT、Superagent、Warp 的 Web 截图。

AgentGPT 用纵向状态块把“完成/新增/开始”逐条铺开，图标与动词承担识别；Superagent 用缩进的小字步骤列表，当前项深、已完成项淡。

本实现取两者的共同点：左侧符号 + 类型词 + 标题的紧凑行，状态历史只在展开后出现，避免把每个状态做成独立卡片。

## 改动文件

- `desktop/src/shell/HomeChat.tsx`：消息新增 `timeline`；tool_activity / thought_delta / unknown_chunk 改为追加；正文前渲染时间线；终态摘要折叠；旧 `toolNote` 仅作无时间线时的回退。
- `desktop/src/shell/timelineModel.ts`（新增）：时间线条目类型、追加/聚合纯函数、摘要与时间格式。
- `desktop/src/theme.css`：新增 `.od-timeline*` 样式，沿用既有色板令牌。
- `tests/verification/desktop/verify_ui_timeline_711.py`（新增）。
- `tests/verification/desktop/od-ui-711/`（本目录，证据）。

## 裁决记录

1. **正文区与时间线区两段式**：`text_delta` 仍只写正文，时间线始终在正文前；这是任务卡允许的“时间线区+正文区”简化，不追求 dsh 的逐条严格交错。
2. **unknown_chunk 聚合但不渲染**：数据中保留一条 `note` 条目（`count` 与 `N 个后台块`），界面过滤掉 note；摘要只数命令与思考，避免长任务刷屏。
3. **工具状态归并**：优先用 `call_id` 归并同一命令；没有 `call_id` 的 `tool/result` 挂到最近一条命令；引擎空状态用事件类型显示为“调用/结果/更新”，不猜造具体状态。
4. **持久化恢复**：`timeline` 随消息档案保存并在恢复时直接使用；现有事件通道没有历史回放接口，不重放、不按任务状态补造条目，因此切页离开期间错过的事件会留下缺口。
5. **移除旧的“本轮没有思考内容”提示**：时间线为空即表示没有可回看的执行记录，继续增加说明文字违背条目即信息的原则。
6. **纯函数外置**：时间线追加语义放在 `timelineModel.ts`，使 verify 能用 tsc + node 做行为断言，而不是只做字符串检查。

## 未做与已知限制

- 未做真实窗口目视；本轮证据为构建、纯函数行为与静态接线检查。
- 切页恢复只保留已落盘时间线，离开期间的新事件不会补录（无历史事件接口，拒绝伪造）。
- 未在真实长任务中核对 dsh 的 tool 事件序列；当前归并规则以 710 已确认的 `tool/call`、`tool/result`、`call_id` 字段为据。
- 工具标题沿用 710：`title` 缺失时回退工具名，因此多显示 `bash` 等工具名，不是命令日志正文。

## 豁免声明

未新增 lint/测试豁免；未跳过或删除既有测试；未改低验收阈值；未改 `runtime/`；未 commit、push 或打标签；未改 `STATUS.md`、`REVIEW.md`、`PLANS.md`、`docs/decisions.md`。
