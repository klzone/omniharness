# 阶段一独立复核记录

- 复核时间：2026-11-12
- 复核对象：`evidence/phase1/01-runtime-identity.json` 至 `06-pi-boundary.json`
- 复核方法：逐文件读取、JSON 结构检查、关键命令独立复跑、Git 事实核对、边界检查
- 当前状态：**阶段一证据验收有条件通过；用户选择方案 1，G2 已满足，阶段二源码审计开始**

## 1. 初审判定

| 任务 | 结果 | 说明 |
| --- | --- | --- |
| DSH-P1-01 身份矩阵 | 通过（有补充） | 版本矩阵完整；执行者未运行 `dsh --version`，研究负责人已独立复跑并得到 `0.2.1-rc.5` |
| DSH-P1-02 来源图 | 通过 | 锁定 commit、gitlink、空检出和运行产物可用性均已取证 |
| DSH-P1-03 Profile/插件 | 通过但有边界偏差 | 装配摘要可用；`--dump-config` 刷新了真实用户数据文件的 mtime，需记录为流程偏差 |
| DSH-P1-04 模型路由 | 通过但有相同边界偏差 | 路由结论可用；执行者按要求停止后未继续 dump |
| DSH-P1-05 历史证据索引 | 通过（A1 修正后） | 执行者误称当前 `REVIEW.md` 不含 E17/E20 记录；实际第 1366 行包含该记录。补充卡已修正并复核 |
| DSH-P1-06 Pi 边界 | 通过 | 已正确区分 pi-ai SDK 与 Pi Agent Runtime；仅远程固定候选版本，未安装 |

## 2. 独立复核事实

### 2.1 版本与来源

- `runtime/dsh-runtime/node_modules/.bin/dsh --version` 实际输出 `0.2.1-rc.5`（研究负责人复跑）。
- `RUNTIME.lock` 的 tag 为 `dsh-v0.2.1-rc.5`，commit 为 `b830ec950faba0a46dda564be3e2d854afed94f1`。
- `git ls-tree HEAD runtime/deepseek-harness` 返回 gitlink `d743267388641bc76f17c45ce8b4c231aed1d32c`；子模块未检出，目录为空。
- 官方 upstream 的 tag 查询确认 tag 指向 lock commit。
- 当前运行的 DSH Desktop 外壳为 `0.2.2-rc.1`；host 进程命令行传入 `~/ .dsh/profiles/desktop` 路径。它没有出现 `--profile desktop` 字符串，不能用“命令行含 `--profile`”作为证据。

### 2.2 OmniDesk 启动 Profile

- `settings.v1.json` 的 `engine_profile` 是 `discipline`。
- 没有显式 `OMNIDESK_DSH_PROFILE` 时，保存的 `discipline` 经白名单校验后生效；如果保存值为空，`web-api` 的标准 Profile 是 `omnidesk-base`，不是 `web`。
- `engine_channel` 未写入 settings；`EngineConfig::from_env` 的默认值为 `web-api`。
- `discipline` 的 bundle 顺序为 `dsh-base` → `dsh-web-app`；Profile 补丁插入 `skill-office`、`web-search-free`、`cost-meter`。
- `--dump-config` 产生 194 个顶层 id，并报告 `patch: entry "acp" not found`。

### 2.3 模型路由

- OmniDesk 的覆盖文件把 `agent-default-model` 设为 `opendesign / deepseek-v4.1-flash-eco`。
- 覆盖文件声明 4 个 opendesign 模型；`acp` 条目不命中 discipline 的组装树。
- web-api 的默认 reasoning 仍为 `U`；静态适配器显示缺省方向为 `false` 的提示，但没有请求级证据。
- DSH Desktop 的 `desktop` Profile 设置 provider `reasoning: high`，且 eco 模型 `maxTokens: 128000`；这是配置文件事实，不是平台默认。

### 2.4 历史证据

- 当前 `REVIEW.md` 第 1366 行确实包含 E17/E20 在 rc.5 升级后回归失败的记录。
- 同一记录也可在历史提交 `d846af3a89:REVIEW.md` 中找到。P1-05 的“当前没有”陈述错误，阶段的后续依赖必须按当前第 1366 行引用。

### 2.5 Pi 对象

- 本机 `pi`、`pi-agent`、`pi-coding-agent` 不在 PATH；全局 npm 仅安装 npm。
- 本地 `@earendil-works/pi-ai@1.4.0` 是 DSH 依赖，不是 Pi Agent Loop。
- 远程 v1.4.0 的 `pi-agent-core`/`pi-coding-agent` 清单均为 1.4.0；这是候选实验版本，不是已安装或已验证运行版本。

## 3. 只读边界偏差

对真实数据根运行 `--dump-config` 刷新了：

`~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.yml`

- 当前内容为 223 字节的标准空列表 `[]` 及生成注释。
- P1-03 记录 mtime `2026-11-12T15:12:45`；P1-04 记录 `2026-11-12T15:15:04`。
- 预执行哈希未采集，因此不能证明字节未变。
- 未恢复、未编辑该文件；未读取凭证或会话正文；未修改仓库内 `runtime/`。
- 处理结论：这是一个流程偏差，不是产品源码改动；仍应记为“未完全满足只读约束”。后续禁止重复执行。

## 4. 已检查的拒绝点

- 没有在证据目录发现凭证值、Bearer、Cookie、`.env` 内容或会话正文。
- 所有 JSON 均可解析，且都含 `step_id`、`commands`、`unknowns`、`boundaries`。
- 没有修改产品源码、`runtime/`、`RUNTIME.lock` 或 Profile patch 文件。
- 没有安装 Pi、启动引擎、调用模型或解包 app.asar。

## 5. DSH-P1-A1 复核结果

- `05-evidence-applicability.md` 第 44 行已改为“当前 `REVIEW.md` 第 1366 行包含这段记录”，并同时保留 `d846af3a89` 历史提交的交叉核对。
- 第 57–58 行登记了 `rg -n 'E17/E20|0\.2\.1-rc\.5' REVIEW.md` 的证据。
- 第 82 行已删除“当前 `REVIEW.md` 没有这段”的依赖结论。
- 修正只影响引用事实，未改变版本结论、复跑要求或历史材料适用性判断。

## 6. 最终结论

- **阶段一事实基线：有条件通过。** 版本、来源、Profile、模型路由、历史证据适用性和 Pi 对象边界均有可复核证据。
- **研究纪律：有条件通过，存在已记录的偏差。** 真实用户数据文件 `profiles/discipline/cordis.yml` 的 mtime 被 `--dump-config` 刷新；内容仍为标准空列表，但缺少预执行哈希，不能声称完全只读。没有修改产品源码、仓库内 `runtime/`、凭证或会话正文。
- **阶段二：已满足源码门禁。** 用户选择方案 1；rc.5 精确源码已固定在 `/tmp`，见 [05-source-baseline.md](05-source-baseline.md)。
- **技术决策：未作出。** 本阶段没有足够证据支持继续 DSH、专用化改造 DSH 或迁移 Pi 中的任何一项。

## 7. 下一步需要的用户选择

1. 允许在 `/tmp` 获取 `b830ec95` 的精确 rc.5 源码并只读审计（不安装依赖、不改仓库 `runtime/`）。
2. 允许退而审计 rc.5 已安装的 `lib/*.js` 和 `lib/d.ts`，接受源码注释、原始类型源与部分布局信息缺失。

用户已选择方案 1。当前已获取并校验精确源码，阶段二仍只做源码审计，不安装依赖、不运行真实模型。
