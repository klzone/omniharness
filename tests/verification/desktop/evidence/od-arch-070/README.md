# OD-ARCH-070 证据摘要（DSH 插件装载 + 上游错误透传）

证据目录：`tests/verification/desktop/evidence/od-arch-070/`

## 1. 隔离 web Profile 的插件装载（文件事实 + 脚本运行）

改动前原件（可回滚）：

- `web-profile.package.before.json`
- `web-profile.patch.before.yml`

装载结果（`~/OmniDesk/runtime/dsh-home/profiles/web/`，仓库外运行时数据）：

- `package.json` 依赖：`dsh-free-search@0.9.5`、`dsh-cost-meter@1.9.6`；bundles 同步登记两项。
- `cordis.patch.yml`：新增 `web-search-deepseek`（只有 `apiKeyEnv: DEEPSEEK_API_KEY` 与 `maxUses: 20`，**不含密钥值**）与 `web-search-free`（`provider: bing`、`bingMarket: zh-CN`）。
- `pnpm install` 在该 profile 内执行成功；`node_modules/.modules.yaml` 显示两者均已 hoist。

出厂资产（编译期嵌入，仓库内、无密钥）：

- `samples/profiles/web.profile.json`
- `samples/profiles/web.profile.patch.yml`

`--dump-config` 合成结果（真实 DSH CLI，profile=web）见 `dump-config-plugin-entries.txt`：
`web-search-deepseek`、`web-search-free`、`cost-meter` 各出现一次。

## 2. 真实 DSH 启动装载（脚本运行）

以隔离 `DSH_HOME` 启动真实 DSH web 服务（40 秒后由 `timeout` 结束，退出码 124 属预期）：

- 日志出现 `[dsh-cost-meter] 已加载,账本:<隔离根>/storages/cost-meter/ledger.json`
- 服务在回环端口正常起服（启动行已脱敏，`token=<redacted>`）
- 无 error / fail 级日志

原始日志：`dsh-web-startup.redacted.log`

## 3. 免费搜索引擎可达性（脚本运行 + 真实外网）

`omni070-net-probe.json`：Bing 搜索页 HTTP 200、99 KB、命中结果标记，467 ms；DuckDuckGo HTML 端点本次 `fetch failed`。
与 patch 中 `provider: bing` 一致，首选引擎可用。

## 4. 端到端 web_search 工具调用（脚本运行 + 真实网络 + 合成模型端点）

在**一次性 /tmp 数据根**内复制隔离 web Profile（含插件与依赖），用本机合成模型端点代替真实模型，
要求它发起 `web_search`。真实 DSH 执行了该工具，不是被模型跳过：

- 工具事件：`tool/call`（`web_search`）→ `tool/result`
- 模型第二轮收到的 tool 消息文本约 8.4 KB（检索结果被真实回填）
- 终态 `succeeded`，`ttfb_seconds=0.582`
- 环境无 `DEEPSEEK_API_KEY`，因此未走需要密钥的 `web-search-deepseek`

原始记录：`tool-call-probe.json`

**能力边界（源码事实）：** `dsh-free-search` 并不新增工具名，它接管 DSH 核心的 `web_search` / `web_fetch` provider
（`lib/index.js` 约 4041–4067 行：未设置 `searchProvider` 时接管）。因此「已装载」的可观察证据是上述真实工具调用成功，而非出现新工具名。

## 5. 上游错误透传与搜索超时（脚本运行，单元级）

`tests/verification/engine-interface/adapter/test_web_api_unit.py` 新增覆盖：上游错误映射为 `task_failed`、
长错误截断、真实 `HTTPError` 400 携带 upstream body、HTTP 500 仍为 `engine_unavailable`、
`search_seconds` 读取与非法值拒绝、搜索工具名识别。
结果：57 passed（见 `pytest-adapter.txt`）。

## 6. 回归

见 `regression.txt`：cargo 227+2+13+3+1 全绿；pytest 57 通过；tsc 0 错误；npm build 成功；`git status --porcelain runtime/` 为空。

## 未验证 / 限制

- **未运行真实模型**：本轮所有模型侧均为本机合成端点，未消耗真实额度，也未验证真实模型在收到检索结果后的作答质量。
- **未在 OmniDesk 桌面窗口中实测**：未由人肉眼确认界面上的工具活动行与超时提示。
- **上游 400 透传**只有单测级证据，未在真实上游网关上复现。
- `web-search-deepseek` 需要用户自备 `DEEPSEEK_API_KEY`；未配置时该条目不产生可用搜索，属预期。
- 出厂播种只在运行时 profile **缺失**时写入，不做 `pnpm install`；新机器首次启动仍需联网安装依赖。

## 回滚

用两份 `.before.*` 覆盖回隔离 profile 对应文件即可撤销插件装载；仓库内无用户数据受影响。
