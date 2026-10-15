# OmniDesk 独立 DSH 运行环境实施报告（Phase 1）

实施日期：2026-10-15
对应审计：`docs/OmniDesk-DSH-Isolation-Audit.md`
目标：在不影响日常使用的原版 DSH 前提下，为 OmniDesk Discipline 提供独立的 DSH CLI/Web 开发和测试实例。

## 一、结论摘要

OmniDesk 已能以独立实例运行，与原版 DSH 完全隔离，且未修改 DSH Runtime。

- 独立数据根：`~/OmniDesk/runtime/dsh-home`
- 独立 Agent 根：`~/OmniDesk/runtime/agents-home`
- 独立 Desktop userData（本阶段预留，未启用）：`~/OmniDesk/runtime/desktop-user-data`
- 专用 Profile：`yvan-work`
- 启动入口：直接调用系统安装的 DSH CLI 本体（asar 归档内的 bin.js），绕过会强制 `DSH_HOME=/home/kevin/.dsh` 的用户 shim。
- 隔离自检：37 项全部通过。
- 真实启动：CLI/Web 均已在隔离根成功运行，并只监听 127.0.0.1。

## 二、实际调用的 CLI 入口及依据

启动器选定并实际使用的入口：

```
CLI 入口   : /opt/DeepSeek Harness/resources/app.asar/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js
CLI 运行时 : /opt/DeepSeek Harness/deepseek-harness （asar 模式，ELECTRON_RUN_AS_NODE=1 --expose-internals）
```

依据：

1. 系统安装的 `/opt/DeepSeek Harness/deepseek-harness --version` 返回 `0.2.1-alpha.2`，与上游标签 `dsh-v0.2.1-alpha.2` 一致。
2. CLI 本体位于 `app.asar` 归档内，无法用文件存在性判断，因此启动器改用真实执行探测：用 `ELECTRON_RUN_AS_NODE=1` 以 Node 模式运行该 `bin.js`，能返回版本号即视为可用。
3. 备用入口为从归档提取的 CLI 树 `$HOME/dsh-asar/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js`，需要系统 `node >= 24`（与官方 engines 一致）；当 asar 入口不可用时才会启用。
4. 明确不使用 `~/.local/bin/dsh`：该文件是桌面应用生成的 shim，内部显式 `export DSH_HOME='/home/kevin/.dsh'`（绝对路径，非波浪号写法），会覆盖外层传入的隔离值；且它会在设置变量之前先扫描 `$PATH` 抢占其它 dsh，兜底路径指向的 `dependencies/dsh` 目录当前为空。启动器把它排除在候选之外。

关于 `dsh web`：在 DSH 中 `dsh <name>` 等价于 `dsh --profile <name>`，`web` 是一个 Profile 名而不是子命令。OmniDesk 使用 `yvan-work` Profile（已挂载 `dsh-web-app` bundle），因此 Web 旗标直接跟在 `--profile yvan-work` 之后，启动器也把单独书写的 `web` 归一化为“使用隔离 Web 旗标”。

## 三、隔离根解析结果

启动器常量：

| 变量 | 解析值 |
|---|---|
| OMNIDESK_ROOT | `~/OmniDesk`（可用环境变量覆盖） |
| DSH_HOME | `~/OmniDesk/runtime/dsh-home` |
| DSH_AGENTS_HOME | `~/OmniDesk/runtime/agents-home` |
| Desktop userData | `~/OmniDesk/runtime/desktop-user-data`（预留） |
| OMNIDESK_PROFILE | `yvan-work` |

启动前的硬性校验（任一失败即退出，不带着可疑路径启动）：

1. `DSH_HOME`、`DSH_AGENTS_HOME` 均不能为空。
2. 两个隔离根都必须位于 `OMNIDESK_ROOT` 之下。
3. 两个隔离根都不得落入 `~/.dsh` 或 `~/.agents`。
4. 两者不能相同。
5. Profile 名不得为空、含路径分隔符、`.`/`..`，且不得使用保留名 `desktop`、`node_modules`。
6. Profile 实际路径（解引用软链接后）仍须在隔离根内，且不得落入原版数据根。
7. 启动前用 `dsh-home-paths` 模块对子进程做环境变量生效性探测；探测结果必须同时包含两个隔离根，且不得包含原版数据根。

实际解析结果（真实启动日志）：

```
DSH_HOME     : ~/OmniDesk/runtime/dsh-home
AGENT_HOME   : ~/OmniDesk/runtime/agents-home
Profile      : yvan-work
Profile 路径 : ~/OmniDesk/runtime/dsh-home/profiles/yvan-work
原版数据根   : ~/.dsh / ~/.agents（本次不访问）
dsh web: http://127.0.0.1:3210/?token=...
```

隔离实例中实际落盘的数据（再次证明数据没有写进原环境）：

- 会话：`~/OmniDesk/runtime/dsh-home/sessions/--tmp--/session-...`
- 存储：`~/OmniDesk/runtime/dsh-home/storages/{workspace.json,session_projcache}`
- 凭证：`~/OmniDesk/runtime/dsh-home/.credentials.yaml`（隔离实例自动生成的新文件，不是从原环境复制）
- 遥测身份：`~/OmniDesk/runtime/dsh-home/.anonymous-user-id`
- Profile：`~/OmniDesk/runtime/dsh-home/profiles/{yvan-work,omnidesk-test}`
- Skills：`~/OmniDesk/runtime/dsh-home/skills/`

## 四、修改文件清单

本阶段改动全部位于 OmniDesk 仓库，未触碰 `runtime/deepseek-harness` 与系统安装。

新增：

| 文件 | 说明 |
|---|---|
| `scripts/omnidesk-dev.sh` | OmniDesk 专用启动器；支持 `--check`/`--dry-run`/`--help`，含上述全部硬性隔离校验 |
| `scripts/omnidesk-isolation-check.sh` | 隔离自检脚本（37 项，静态与启动前检查，含原环境零变化复核） |
| `docs/OmniDesk-DSH-Isolation-Audit.md` | 源码级隔离审计报告（上一阶段交付） |
| `docs/OmniDesk-DSH-Isolation-Implementation.md` | 本实施报告 |
| `docs/OmniDesk-Web-Notice-Error-Fix.md` | Web 界面“暂时无法保存确认状态”问题的定位与修复说明 |

修改：

| 文件 | 变化 |
|---|---|
| `scripts/sync-skills.sh` | 重写为隔离安全版：目标改为由隔离根推导；拒绝写入 `~/.dsh/skills` 与 `~/.agents/skills`；支持 `--dry-run` 与 `--to-agents`；空源、解析异常、软链接、越界目标一律退出 |

顺带修复（仅 OmniDesk 仓库层，未触碰原版 DSH 数据）：以下三处把 `welcomeNoticeVersion` 由过期值 `2026-08-13.1` 更新为当前运行时期望值 `2026-09-28.1`。原因与验证见 `docs/OmniDesk-Web-Notice-Error-Fix.md`。

| 文件 | 变化 |
|---|---|
| `profiles/yvan-work/cordis.patch.yml` | 第 7 行 `2026-08-13.1` -> `2026-09-28.1` |
| `profiles/desktop/cordis.patch.yml` | 第 7 行 `2026-08-13.1` -> `2026-09-28.1` |
| `workstations/discipline/profile.yaml` | 第 30 行 `2026-08-13.1` -> `2026-09-28.1` |

备份（改动前，仅针对仓库文件，未触碰原 DSH 数据）：

```
/tmp/omnidesk-backup-20261015/sync-skills.sh.bak
/tmp/omnidesk-backup-20261015/yvan-work-cordis.patch.yml.bak
/tmp/omnidesk-backup-20261015/yvan-work-package.json.bak
```

### 启动器安全边界（透传参数白名单）

允许透传：`--port`、`--no-open`、`web`（别名）等普通旗标。

拒绝透传：

| 参数 | 拒绝原因 |
|---|---|
| `--profile` | 会覆盖隔离 Profile |
| `--patch` / `--from-default-profile` | 会覆盖 Profile 补丁层或来源，破坏隔离边界 |
| `--host` | 固定绑定 127.0.0.1，避免对外暴露 |
| `--public-url` / `--trusted-host` | 属于对外暴露与信任配置 |
| `--dump-config` 系列 | 会读取 Profile 组合树 |

默认行为：`--host 127.0.0.1 --port 0 --no-open`（系统分配端口，不自动开浏览器），不自动启动 Desktop、不自动安装插件、不读取原环境凭证。

### Skill 同步脚本修正要点

- 目标目录由已校验的 `DSH_HOME` 或 `DSH_AGENTS_HOME` 推导，不接受任意路径。
- 任何情况下都拒绝写入 `~/.dsh/skills`、`~/.agents/skills`，并拒绝目标等于原版数据根。
- 目标为软链接、隔离根未初始化、源目录为空时立即退出（避免用空源执行 `--delete`）。
- `rsync --delete` 只作用于通过校验的 OmniDesk 目标。
- 提供 `--dry-run` 预览。
- 自检脚本第 8、9 项专门验证：拒绝同步到原版目录，且 dry-run 不改变隔离根结构。

## 五、执行过的测试与完整结果

### 1. 隔离自检（scripts/omnidesk-isolation-check.sh）

结果：通过 37 项，失败 0 项，退出码 0。覆盖：

- Bash 语法检查（两个脚本）
- 启动器默认隔离边界（`--check`、`--dry-run` 输出只引用隔离根，不含原版路径）
- 危险隔离根拒绝（`OMNIDESK_ROOT` 指向/落在 `~/.dsh`、`~/.agents`）
- Profile 名校验（`../evil`、`a/b`、`desktop`、`node_modules`、`..`）
- 透传参数白名单（拒绝 `--host=0.0.0.0`、`--profile desktop`、`--patch`、`--from-default-profile`、`--public-url`、`--trusted-host`、`--dump-config`）
- 允许的透传（`--port 3099`、`--no-open`、`web` 归一化）
- Skill 同步目标校验与 dry-run 无写入
- 原环境零变化复核

### 2. 真实启动（CLI/Web）

在隔离根实际启动成功：

- 实例 A：`DSH_HOME=~/OmniDesk/runtime/dsh-home` → Web `http://127.0.0.1:3210/`，inspector 9230。
- 实例 B（复现实验根）：`OMNIDESK_ROOT=/tmp/omnidesk-repro` → Web `http://127.0.0.1:3300/`，inspector 9231。

两次启动日志都显示入口、数据根、Profile 均位于隔离根；Web 服务只绑定 127.0.0.1。带 token 的真实 HTTP 探测：不带 token 返回 401，带 token 正常放行，说明 Web 服务真实可用。

### 3. 会话 / 配置 / Skill 落盘位置

- 创建测试会话：落在 `~/OmniDesk/runtime/dsh-home/sessions/`（以及复现根 `/tmp/omnidesk-repro/runtime/dsh-home/sessions/`）。
- 修改测试配置（非敏感）：通过 `settings/mutate` 写入隔离根 `profiles/yvan-work/cordis.patch.yml`，返回 `ok:true`。
- 加载测试 Skill：隔离根 `skills/` 下八个自定义 Skill（`material-writing`、`knowledge-search`、`document-review`、`memory-agent`、`knowledge-agent`、`writing-agent`、`project-agent`、`coding-agent`）与仓库 `skills/` 完全对应；同步测试用临时构造的 `omnidesk-test` Profile 验证。

### 4. 原环境零变化复核（关键证据）

测试前对以下目录记录文件清单、修改时间和内容哈希：

- `~/.dsh`
- `~/.agents`
- `~/.config/deepseek-harness`（实测当前生效的 Electron userData，由 `--user-data-dir` 指定）及 `~/.config/DSH Desktop`、`~/.config/@deepseek-ai` 两个陈旧目录

然后在隔离实例中创建会话、改配置、加载 Skill，之后再次记录并逐项比对：

```
diff -rq /tmp/omnidesk-isolation/pretest-20261015_142925/ \
         /tmp/omnidesk-isolation/posttest-20261015_155356/
=> 8 个文件全部一致（无输出），退出码 0
```

结论：从文件清单、时间戳和哈希三个维度看，原版 DSH 的会话、配置、Skill 均没有因本轮测试产生新增、修改或删除。

### 5. Runtime 未改动复核

```
cd runtime/deepseek-harness && git status --short   => 无输出（RT_DIRTY_LINES=0）
git describe --tags   => dsh-v0.2.1-alpha.2
git rev-parse HEAD    => d743267388641bc76f17c45ce8b4c231aed1d32c
```

Runtime 源码与上游标签一致，未做任何修改。

## 六、未完成事项与风险

未完成 / 未启用：

1. Desktop（Electron）隔离未启用。`desktop-user-data` 目录已预留，但本阶段只验证 CLI/Web；生产 Desktop 能否通过 `--user-data-dir` 并行启动第二实例、单实例锁的转发行为，仍未经实测。
2. Profile 尚未专用化。`profiles/yvan-work/package.json` 与 `profiles/desktop/package.json` 当前内容一致（仍为 `dsh-profile-desktop`，bundle 组合为 base + web-app + inspector）。它可以被加载并可运行，但仍沿用 Desktop/Web 组合名，尚未整理成真正独立的 OmniDesk Profile。
3. 可选插件未逐项审计。插件可能自行访问 `DSH_HOME` 之外的绝对路径、配置目录、环境变量或外部服务；安装前需要逐项检查副作用。
4. 未做持久化 SQLite 检索隔离。当前 bundle 默认使用内存路径且不自动打开；将来启用持久索引时需要显式配置隔离文件路径。

风险与应对：

| 风险 | 状态 | 应对 |
|---|---|---|
| 用户 shim 重设 DSH_HOME | 已证实（shim 源码） | 启动器绕过 shim，改为直接调用 CLI 入口并校验 |
| 误用原 Profile/插件数据 | 已证实（位于 DSH_HOME 下） | 全新 DSH_HOME、专用 Profile、禁止跨根软链接 |
| Skill 同步触碰生产目录 | 已修复 | 新脚本按隔离根推导目标并硬性拒绝原版路径 |
| Desktop 数据仍共享 | 部分未证实 | 后续启用 Desktop 时指定独立 userData，验收前不同时运行 |
| 端口占用 | 已缓解 | 回环绑定 + 端口 0（系统分配） |
| 插件在外部路径写数据 | 未逐项审计 | 安装前逐项检查路径与副作用 |

## 七、如何运行（本地边测边开发）

```
# 只做隔离校验，不启动任何进程
scripts/omnidesk-dev.sh --check

# 打印将要执行的完整命令，不执行
scripts/omnidesk-dev.sh --dry-run

# 启动隔离 Web UI（系统分配端口，只绑定 127.0.0.1）
scripts/omnidesk-dev.sh

# 指定端口
scripts/omnidesk-dev.sh --port 3090

# 同步自定义 Skill 到隔离根（先预览）
scripts/sync-skills.sh --dry-run
scripts/sync-skills.sh
```

原版 DSH 桌面应用、原 `~/.dsh` 与原 `~/.agents` 都无需改动，可继续日常使用。
