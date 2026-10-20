# OD-ARCH-034 证据索引：模型路由切换（对话流里的模型下拉）

> 执行日：2026-10-20。起因：用户实测发现「在 DSH 界面里配了自定义模型（agnescn / agnes-3.0-flash），
> 回 OmniDesk 刷新端点却仍是 deepseek，任务只拿到 partial 结果」。

## 0. 根因（实测，不是推测）

DSH 的**模型选择按 Profile 存**：

| Profile | 用途 | 生效的默认模型 |
| --- | --- | --- |
| `acp` | **OmniDesk 引擎跑的就是它**（`--profile acp`） | `deepseek-official / deepseek-flash` |
| `web` | DSH 界面（用户配置的地方） | `agnescn / agnes-3.0-flash`（baseURL `https://api.agnes-ai.cn/v1`） |

两边**只有凭据是共享的**（`$DSH_HOME/.credentials.yaml` 里的 ref `AGNESCN_API_KEY` ✓），
**模型选择不共享** → 引擎仍走 deepseek（而 deepseek 的 key 没有），于是任务 partial/失败；
而「刷新端点」如实显示的也正是**引擎那条路**（deepseek），所以界面没有骗人，是配置本来就对不上。

## 1. 做了什么

| 文件 | 内容 |
| --- | --- |
| `desktop/src-tauri/scripts/model-routes.mjs`（新） | ① `--list`：列出各 Profile 生效的 provider / model（只用 DSH 公开的 `--dump-config`，**不读凭证值**）；② `--write-overlay`：把某 Profile 的 provider 配置 + 选中的模型写成一份 **OmniDesk 自己的** patch 覆盖文件（`~/OmniDesk/app/config/omnidesk-model.yml`，**只含非机密字段**：baseURL / api / apiKeyEnv / 模型 id；任何 key/secret/token 一律剔除）。 |
| `desktop/src-tauri/src/model_routes.rs`（新） | 宿主侧调用上面的脚本（只用 stdin + `OMNIDESK_ROUTE_ARGS`，不落临时文件），并提供「覆盖文件路径」与「Profile 列表」。 |
| `desktop/src-tauri/src/engine_supervisor.rs` | 引擎的 patch 列表**附加**这份覆盖文件 → 引擎与端点分类器用**同一份**路由。 |
| `desktop/src-tauri/src/lib.rs` | 两个命令：`task_list_model_routes`（列出候选）、`task_set_model_route`（选定 → 写覆盖 → 停引擎 → 重新判定端点）。 |
| `desktop/src/api.ts` / `desktop/src/shell/HomeChat.tsx` | 对话区**右下角、发送按钮前**加模型下拉：显示当前路由，列出各 Profile 的候选；选中即切换（会重启引擎），并显示切换结果。 |

## 2. 实测

- **路由判定（用覆盖文件重跑分类器）：**
  `provider=agnescn`、`model=agnes-3.0-flash`、`endpoint=https://api.agnes-ai.cn/v1`、类别 `public`，
  依据含「端点为 llm-pi-ai.providers.agnescn.baseURL」✓。
- **应用内：** 正式实例的对话下拉显示「**模型：agnescn / agnes-3.0-flash**」✓，位置即右下角、发送按钮前
  （截图 `shots/od034-chat-model-dropdown.png`）；设置页的端点事实随之变为该 provider ✓。
- **覆盖文件内容：** 只含 `displayName / api / apiKeyEnv / baseURL / models` 与 `provider / model`——
  **没有任何 key**（key 仍由共享凭据服务按 `AGNESCN_API_KEY` 提供）✓。
- **回归：** Rust **143/143**、`tsc` 0、构建通过。

## 3. 边界与未验证

- 写出的覆盖文件在 **OmniDesk 自己的数据根**下，**不碰用户的 DSH Profile**；只写非机密字段。
- 切换会**停掉并重启引擎**（路由在引擎进程启动时确定）；若停引擎未确认，命令**明确报错**并要求手动停止。
- **未验证**：切换后真实模型调用的完整往返（需要真实 key 与真实端点，属用户真实使用范围）；
  切换对**已发出**任务无效（引擎重启后旧任务不再继续）。
- 本轮新增的宿主能力（模型路由列举/选择）超出此前「宿主不新增命令」的边界，属**用户明确要求**的功能，
  已在 REVIEW.md 标注待复核。
