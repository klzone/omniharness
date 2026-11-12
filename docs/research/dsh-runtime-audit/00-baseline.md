# DSH Runtime 运行版本与来源基线

- 基线时间：2026-11-12 15:05 +08:00
- 仓库：`main`，HEAD `5af890a84333a50a743347f45a66c9642b1dcf05`，`git status` 干净
- 状态：阶段一证据基线；阶段二静态源码审计已通过；阶段三未开始

## 1. 结论摘要

1. OmniDesk 仓库和安装包锁定的可运行 DSH 是 **0.2.1-rc.5**；它不是当前正在运行的 DSH Desktop。
2. 本机当前运行的 DSH Desktop 是 **0.2.2-rc.1**，使用 `desktop` Profile；其 Profile 配置默认路由为 **opendesign / deepseek-v4.1-flash-eco**。未读取活动会话正文，因此“默认路由已配置”不等于“当前会话实际使用”。
3. 仓库 `runtime/deepseek-harness` 子模块没有检出；gitlink 是 **d7432673**（历史 alpha.2 基线），而 `RUNTIME.lock` 要求 **b830ec95**（rc.5）。用户选择方案 1 后，精确 rc.5 源码已获取到 `/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo` 并校验；它不替代仓库子模块，也不是产品路径。
4. OmniDesk 当前引擎 Profile 是 **discipline**，默认通道按未显式设置解析为 **web-api**，不是历史 ACP。
5. OmniDesk 实际模型覆盖文件把默认模型设为 **opendesign / deepseek-v4.1-flash-eco**；web-api 默认请求的 reasoning effort 尚未被证明。
6. `dsh-llm-pi-ai` 是 DSH 的模型 SDK 适配器，依赖 `@earendil-works/pi-ai@1.4.0`；它不是 Pi Agent Runtime，不能拿它代替 DSH 与 Pi 的 Agent Loop 对照。

以上均为初步基线。未标注实测的内容仍以任务卡复核结果为准。

## 2. Runtime 身份矩阵

| 身份 | 版本 | 运行/安装位置 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 仓库锁定 CLI | `@deepseek-ai/dsh 0.2.1-rc.5` | `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh` | 可运行；`dsh --version` 返回 0.2.1-rc.5 | `F` + `R` |
| 仓库安装包 | OmniDesk `0.2.22` | `/Applications/OmniDesk.app` | 已安装；内置 `runtime/RUNTIME.lock` 同为 rc.5 | `F` |
| 当前源码版本号 | `0.2.23` | 仓库 `VERSION` | 与已安装包版本不同，不能把源码状态当成已安装包状态 | `F` |
| DSH Desktop | `0.2.2-rc.1` | `/Applications/DeepSeek Harness.app` | 正在运行；默认路由为 `opendesign / deepseek-v4.1-flash-eco`（配置事实） | `F` + `R` |
| DSH Desktop 内置机制包 | `0.2.2-rc.1` | `app.asar/dsh/node_modules/@deepseek-ai/*` | `dsh-agent-loop` 等包的版本字符串为 0.2.2-rc.1 | `F` |
| OmniDesk 当前引擎 | rc.5 CLI，Profile `discipline` | 当前无引擎进程；最近运行配置保存在 `~/OmniDesk/app/config/settings.v1.json` | 选定 Profile 与版本可确认，未启动状态下不能宣称“当前进程正在使用” | `F` + `R` |

### 2.1 当前 DSH Desktop 进程事实

- 主进程 PID 743；host 进程 PID 1398。
- host 命令行包含 `app.asar/dsh/node_modules/@deepseek-ai/dsh-desktop-host/lib/index.js`。
- host 命令行把 `~/.dsh/profiles/desktop` 作为位置参数传入；本次没有观察到 `--profile desktop` 字符串。Profile 名仍可从路径确认，但不能把“参数形式”写成已验证事实。
- 使用 `~/Library/Application Support/@deepseek-ai/dsh-desktop` 作为 Electron userData。
- 证据等级：`R`。这只证明进程入口和 Profile 路径，不证明每个内部机制的实现版本均与外壳相同。

## 3. 源码与运行产物定位

| 对象 | 路径 | 可用性 | 证据 |
| --- | --- | --- | --- |
| rc.5 完整源码预期路径 | `runtime/deepseek-harness` | **不可用**：子模块目录为空，未检出 | `F` |
| rc.5 锁定 commit | `b830ec950faba0a46dda564be3e2d854afed94f1` | 远程标签 `dsh-v0.2.1-rc.5` 指向该 commit | `F` |
| 当前 gitlink | `d743267388641bc76f17c45ce8b4c231aed1d32c` | 与 lock 不一致；对应历史 alpha.2 时期 | `F` |
| rc.5 精确审计源码 | `/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo` | 可用；detached 在 b830ec95，tag `dsh-v0.2.1-rc.5`，工作树干净 | `F` + `R` |
| rc.5 可运行产物 | `runtime/dsh-runtime/node_modules/@deepseek-ai/*/lib/*.js` | 可用；主要是编译 JS 和类型声明，不是完整 TypeScript 仓库 | `F` |
| 当前 DSH Desktop 运行产物 | `/Applications/DeepSeek Harness.app/Contents/Resources/app.asar/dsh/node_modules/@deepseek-ai/*` | 可在 asar 内运行，未解包 | `F` |
| OmniDesk 隔离数据根 | `~/OmniDesk/runtime/dsh-home` | 当前 Profile 为 `discipline` | `F` |

### 3.1 为什么这是一个门禁

阶段二不能用“最新源码”冒充 rc.5，也不能只拿 README 描述代替实现。当前路径：

G2 已满足：GitHub tag `dsh-v0.2.1-rc.5` 检出到 `b830ec95`，源码位于 `/tmp` 且未修改。仓库内子模块仍为空，不影响本轮只读审计，但不能把它当成仓库内已检出的源码。

没有安装依赖、没有初始化仓库子模块、没有解包 app.asar。

## 4. OmniDesk 启动 Profile 与插件组合

### 4.1 Profile 选择

- `~/OmniDesk/app/config/settings.v1.json` 的 `engine_profile` 为 `discipline`。
- `engine_channel` 为 `null`；按当前宿主默认逻辑解析为 `web-api`。
- 显式保存的 `discipline` 经 Profile 白名单校验后生效。若保存值为空，`web-api` 的标准 Profile 是 `omnidesk-base`；`web` 是历史模板名，不是当前 `web-api` 的标准 Profile。
- 引擎 CLI 的 Profile 名由宿主解析后传入；`engine/omnidesk_engine/web_api_adapter.py` 的通道本身是 web-api。
- 证据等级：`F` + `S` + `R`（诊断脚本读取 settings 与 Profile 包）。

### 4.2 discipline Profile 的声明层

Profile 包 `~/OmniDesk/runtime/dsh-home/profiles/discipline/package.json` 声明：

```text
bundles: @deepseek-ai/dsh-base, @deepseek-ai/dsh-web-app
dependencies: dsh-cost-meter 1.9.6, dsh-free-search 0.9.5
```

Profile 补丁层 `cordis.patch.yml` 额外插入：

```text
skill-office
web-search-free (provider: bing, market: zh-CN)
cost-meter
```

并覆盖 `preset-standard` 的 persona、指令、工具、planning、compaction、delegation 和 web 工具面；末尾把 `web` 的搜索/抓取 provider 设为 `ddg` / `fs-quality-fetch`。

### 4.3 组装后的初步结构

对 rc.5 CLI 使用隔离数据根、`--profile discipline --dump-config --patch ~/OmniDesk/app/config/omnidesk-model.yml` 实跑：

- 退出码 `0`。
- 顶层可见 `- id:` 条目 **194** 个。
- 存在 `session`、`session-persistence-jsonl`、`session-projection`、`session-projection-cache`、`tools`、`system-prompt`、`agent-loop`、`subagent`、`subagent-fork-in-process`、`tool-subagent`、`tool-subagent-fork` 等核心条目。
- 顶层与 `preset-standard` 中同时存在工具条目，部分顶层条目标记 `disabled: true`；每个 Agent 最终可用的工具面由 preset/agent 实例解析，不能从顶层 disabled 状态直接下结论。
- 运行时有警告：`patch: entry "acp" not found`。这说明模型覆盖文件中的 `acp` 条目未作用于 discipline Profile。

证据等级：`R`。完整 dump 只保留摘要，不进入 Git。

### 4.4 DSH Desktop 的声明层

`~/.dsh/profiles/desktop/package.json` 声明 bundles：

```text
dsh-base, dsh-web-app, experimental-badge-skill-bundle,
experimental-plugin-market-bundle, anysearch-dsh, experimental-auto-review,
cost-meter, office-desk, kayle-growth, web-search-exa
```

这说明桌面日常 DSH 与 OmniDesk 的插件面不同，不能直接把桌面体验当作 OmniDesk 的插件组合。

## 5. 模型路由

### 5.1 OmniDesk 当前生效层

覆盖文件 `~/OmniDesk/app/config/omnidesk-model.yml` 指定：

| 项 | 值 | 证据 |
| --- | --- | --- |
| provider | `opendesign` | `F` |
| API 协议 | `openai-completions` | `F` |
| 端点宿主 | `amr-link.open-design.ai` | `F` |
| 默认模型 | `deepseek-v4.1-flash-eco` | `F` + `R` |
| 路由模型数 | 4（deepseek-v4.1-flash、deepseek-v4.1-flash-eco、glm-5.3-flash、minimax-m3.1-flash） | `F` |
| `acp` 条目 | 指定 reasoning `low`，但 discipline 内无 `acp` entry，补丁未生效 | `R` |
| web-api 默认 reasoning | 尚未从实际请求或 loop 状态机证实 | `U` |

凭据只记录引用名 `OD_Key`，不读取其值。

### 5.2 DSH Desktop 当前生效层

| 项 | 值 | 证据 |
| --- | --- | --- |
| Provider | `opendesign` | `F` |
| 默认模型 | `deepseek-v4.1-flash-eco` | `F` |
| Provider 默认 reasoning | `high` | `F` |
| 默认最大输出 | 128000（eco 模型） | `F` |
| Agent Loop 安装包 | `dsh-agent-loop 0.2.2-rc.1` | `F` |

### 5.3 待验证点

- 模型路由在请求前由哪个组件做原因别转换和上下文窗口归一。
- `reasoningEffort` 的最终来源：Profile、会话设置、Agent 预设，还是请求级覆盖。
- `dsh-llm-pi-ai` 与 `dsh-llm-deepseek` 同时挂载时，provider 路由冲突和回退规则。
- Token 统计中的 reasoning、cache read、cache write 如何进入最终 usage。

## 6. 现有文档与测试证据适用性

| 材料 | 版本/日期 | 当前用途 | 限制 |
| --- | --- | --- | --- |
| `docs/DSH-Architecture-Analysis.md` | v0.2.1-alpha.2，2026-10-14 | 架构假设、组件清单线索 | 不能作为 rc.5 源码证据；插件面与模型路由已变化 |
| `docs/OmniDesk-DSH-Isolation-Audit.md` | alpha.2，2026-10-15 | 隔离边界、数据根解析线索 | 需在 rc.5/当前宿主上重新验证 |
| `docs/OmniDesk-DSH-Isolation-Implementation.md` | alpha.2，2026-10-15 | 隔离启动历史证据 | 不是当前运行时证据 |
| `docs/OmniDesk-Engine-Interface-Audit.md` | alpha.2，2026-10-16 | 接口历史审计 | 文件行号与 ACP 默认路径已过时 |
| `docs/OmniDesk-Engine-Interface-Contract.md` | alpha.2 起，v0.2 | 产品侧接口语义 | 需要区分“契约语义仍有效”与“上游实现行号失效” |
| `docs/OmniDesk-Engine-Interface-Test-Matrix.md` | alpha.2 起，v0.2 | 历史验收矩阵 | `REVIEW.md` 第 1366 行记录 rc.5 升级后 E17/E20 回归失败；该记录也存在于历史提交 `d846af3a89`，矩阵未整体复跑 |
| `docs/OmniDesk-Session-Import-Compatibility.md` | alpha.2，2026-10-18 | 历史 ACP 导入结论 | 不能用于 web-api 当前路径 |
| `tests/reports/phase1-verification-report.md`、`p1-*`、`p2-*`、`v0.1-*` | alpha.2 | 历史基线 | 只能作为历史证据，需标记版本 |
| `audit/dsh-java-review/*` | Java 端口审计 | 对照思路 | 不是 DSH Runtime 实现，不能作为 DSH 源码证据 |
| `audit/token-*.md` | 历史 Token 研究 | 计量口径线索 | 不是 Runtime 机制或同条件对照实验 |

## 7. Pi 对象边界

- 本机 `PATH` 中没有 `pi` 或 `pi-agent` 可执行文件；没有本地 Pi Agent Runtime 源码。
- DSH 内已有 `@earendil-works/pi-ai@1.4.0`，它是模型 Provider/流式协议 SDK。
- 官方 Pi 项目把能力分为 `pi-ai`、`pi-agent-core`、`pi-coding-agent` 三层。阶段三的“Pi”应指 Agent Runtime 或 Coding Agent，具体固定到哪一层、哪个 commit，必须先写成实验前置条件。
- 当前不能用 `dsh-llm-pi-ai` 替代 Pi Agent Runtime 做对照；那会把模型适配层差异误判为 Agent Loop 差异。

## 8. 基线缺口

1. 仓库内 rc.5 子模块仍未检出，gitlink 与 lock 不一致；本轮审计使用 `/tmp` 的精确源码，未改变该事实。
2. 尚无当前引擎进程，无法从活动进程直接观察 Profile、工具面和请求参数。
3. 现有审计文档几乎全部基于 alpha.2；不能直接拿来证明 rc.5 行为。
4. web-api 默认 reasoning effort、实际工具面、并行调度参数和子 Agent 配置尚未取到运行级证据。
5. Pi 仅确认本机不可用，尚未把官方仓库、版本和实验入口固定成可复现基线。
6. 尚无同模型、同任务、同工具条件下的对照实验数据；因此现在不能作出迁移或改造决定。
7. `--dump-config` 对真实数据根运行会刷新 `~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.yml` 的 mtime。当前内容仍是标准空列表，但预执行哈希未采集，不能证明字节未变。后续不得再次对真实数据根执行该命令。

## 9. 阶段一验收所需证据

- 三套 Runtime 身份和版本矩阵：`F` + `R`
- 源码/gitlink/lock 三方一致性：`F`
- OmniDesk 启动 Profile、插件层与生效工具面摘要：`S` + `R`
- 模型 Provider、模型 ID、参数与来源优先级：`F` + `S` + `R`
- 历史材料适用性索引：`F`
- Pi 对象边界和本机可用性：`F` + `D` + `U`

只有上述证据由 DeepSeek 执行、研究负责人独立复核后，阶段一才可通过。当前复核结论见 [04-phase1-review.md](04-phase1-review.md)：P1-05 需修正，阶段二门禁仍关闭。
