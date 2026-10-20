# OD-ARCH-035 证据：UI 设计参考 MCP 接入（Codex）

日期：2026-10-20。安装位置：**Codex**（`~/.codex/config.toml`），OpenCode 未作任何改动。

## 配置

```toml
[mcp_servers.screenshots_design_inspiration]
command = "npx"
args = ["--yes", "screenshots-design-inspiration-mcp@latest"]
startup_timeout_sec = 120
tool_timeout_sec = 180

[mcp_servers.screenshots_design_inspiration.env]
NODE_USE_ENV_PROXY = "1"
```

改前备份：`~/.codex/config.toml.bak.od035`。TOML 解析通过，mcp_servers 键为
`garmin_connect, garmin_health, node_repl, screenshots_design_inspiration`。

## 验证（脚本运行，非真实对话）

外部探测器直接以 stdio 启动该 MCP（`npx --yes screenshots-design-inspiration-mcp@latest`），
依次 `initialize` → `notifications/initialized` → `tools/list` → `tools/call`。

- 服务标识：`design-screenshots` v1.1.0。
- 工具集：`search_by_tags`、`get_product_screenshots`、`browse_by_platform`、
  `search_inspiration`、`list_tags`、`search_screenshots_by_tags`、`get_random_inspiration`。
- `search_inspiration`，query=`AI chat workspace dashboard`：`isError=false`，返回 20 个产品，
  约 2.9 s；含 Dovetail、Hoops AI、Airtable、ChatGPT、Harvey AI 等，每条带 gallery 与图片直链。
- `search_inspiration`，query=`sidebar dock task dashboard`：`isError=false`，返回 20 个产品，
  约 0.03 s（命中缓存）。

原始结构化结果见 `mcp-probe.json`。

## 已排障的实际故障（记录，避免重犯）

1. 首次调用报 `MCP error -32602 ... Required at tags`：探测器误选了 `search_by_tags`，属探测器缺陷，
   改为显式指定 `search_inspiration` 后消失。
2. 随后 `search_inspiration` 返回 `fetch failed`：本机设置了 `HTTPS_PROXY` 等环境变量，
   而 Node 原生 fetch 默认不读取它们。`curl` 与 `node -e fetch` 对照证实：
   直连 200 / 显式启用 Node 环境代理 200 / 默认 fetch 失败。
   因此在 Codex 的该 MCP 专属 env 中设置 `NODE_USE_ENV_PROXY=1`，此后搜索成功。
   该设置只作用于这一个 MCP 进程，不改全局 Node 配置。

## 证据等级

- 文件事实：`~/.codex/config.toml` 已含该段；`docs/BUILDER-Task-Instruction-Template.md` 已建；
  AGENTS.md 已加派工条款。
- 脚本运行：上述 stdio 探测真实通过，返回真实产品与截图链接。
- 未验证：**当前会话内的工具挂载**。Codex 需重启后才会加载新 MCP，故本会话看不到
  `mcp__screenshots_design_inspiration__*`；重启后的首次真实调用需另行确认。
  `search_inspiration` 的 `count` 参数在 schema 中存在，但探测返回恒为 20 条，未细究分页语义。

