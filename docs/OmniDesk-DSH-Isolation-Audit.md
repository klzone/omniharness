# OmniDesk 与 DSH 运行隔离审计报告

- 审计日期：2026-10-15
- 审计对象：本仓库内 DSH 源码（v0.2.1-alpha.2）与当前主机上已安装的 DSH 发行形态
- 审计方式：只读源码追踪 + 只读进程/文件系统取证 + 既有隔离脚本的自动化自检
- 本次**未**安装、未覆盖、未迁移、未删除任何 DSH 数据，未修改系统安装，未修改 runtime/deepseek-harness

本报告按四类严格区分内容：**已证实事实**（源码或实测直接支持）、**未证实推断**（尚待验证）、**推荐方案及理由**、**实施步骤与验收标准**。

---

## 0. 结论摘要

**OmniDesk 可以作为独立实例运行，且不需要修改 DSH Runtime。**

支撑结论的三条硬证据：

1. 源码证实 DSH 的用户数据根由进程环境变量 `DSH_HOME` 与 `DSH_AGENTS_HOME` 决定，解析优先级为「显式配置 → 环境变量 → `~/.dsh` / `~/.agents`」。这两个变量属于启动级变量，只能由继承的进程环境提供。
2. 实测三套 OmniDesk 实例与原版 Desktop 同时运行，各自独立数据根、各自回环端口，互斥文件锁只在各自根内出现。
3. 三套 OmniDesk 实例的文件描述符清单中**没有任何** `~/.dsh` 或 `~/.agents` 下的条目；同一时刻原版 Desktop 的 host 进程持有 `~/.dsh/profiles/desktop` 与 `~/.dsh/sessions/.../session.lock`。

因此隔离方案完全建立在 DSH 官方支持的能力之上：环境变量 + Profile 机制 + 回环绑定 + 系统分配端口。仓库需要维护的只是启动、初始化与 Skill 同步这几层脚本护栏，不涉及任何 Runtime 补丁。

---

## 1. 审计对象与版本基线

### 1.1 仓库内源码

| 项目 | 值 |
|---|---|
| 位置 | runtime/deepseek-harness |
| 标签 | dsh-v0.2.1-alpha.2 |
| 提交 | d743267388641bc76f17c45ce8b4c231aed1d32c |
| 工作区状态 | 干净（git status --porcelain 输出 0 行） |

仓库位于 /mnt/data/Code/MyWorkStation；该路径与 /home/kevin/Code/MyWorkStation 指向同一设备号与 inode（66308:5804745），是同一份检出。

### 1.2 本机 DSH 的三条不同入口（重要：容易被混为一谈）

| 入口 | 实际形态 | 审计结论 |
|---|---|---|
| `~/.local/bin/dsh` | 桌面应用安装时生成的 shim（2045 字节，/bin/sh） | **不能用于隔离启动**，见 2.6 |
| `/opt/DeepSeek Harness/deepseek-harness` | Electron 二进制（228 MB），`ELECTRON_RUN_AS_NODE=1` 时可当 Node 使用，内置 Node v24.21.0 | 当前隔离实例实际使用的入口 |
| `/opt/DeepSeek Harness/resources/app.asar` | 148 MB 打包归档，内含 dsh/node_modules/@deepseek-ai/dsh/lib/bin.js | CLI 本体所在，见下方「归档路径无法用 -f 探测」说明 |

**归档路径探测限制（已证实）**：app.asar 是打包归档而非目录，用 ls -d 探测 asar 内的 bin.js 会返回 Not a directory。任何基于 bash `-f` / `realpath` 的存在性判断在这里都会失败。因此仓库启动器采用「执行探测」：`ELECTRON_RUN_AS_NODE=1 <electron> --expose-internals <asar内bin.js> --version`，只有真正跑出非空版本号才认该入口可用。

另有一条备选入口 `~/dsh-asar/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js`（从 asar 提取出的真实目录树，存在），但要求系统 Node ≥ 24；本机系统 Node 为 v22.23.2，不满足门槛，故当前走的是 asar 入口。

---

## 2. 已由源码证实的事实

### 2.1 数据根解析规则

packages/util/home-paths/src/index.ts：

- `resolveDshHome(configured?, env?)`：优先级为「显式参数 → `$DSH_HOME` → `~/.dsh`」。
- `resolveAgentsHome(configured?, env?)`：优先级为「显式参数 → `$DSH_AGENTS_HOME` → `~/.agents`」。
- 两者共用 `resolveRoot()`：**仅含空白的值按未设置处理**，注释明确说明这是为了防止空覆盖把数据根解析到当前工作目录。
- `expandHomePath()` 支持 `~`、`~/`、`~\` 前缀。
- `dshHomePath(...segments)` = join(resolveDshHome(), ...)；`dshCachePath()` = join(resolveDshHome(), 'cache', ...)。
- `dshHomeDisplay()` 只返回符号标签（`~/.dsh` 或 `$DSH_HOME`），从不打印绝对机器路径。

结论：**两个环境变量是官方支持的、不经补丁的数据根重定向手段。**

### 2.2 启动环境与 .env 规则

packages/boot/app-boot/src/index.ts：

- `BOOTSTRAP_PREFIXES = ['DSH_', 'XDG_', 'DYLD_', 'BASH_FUNC_']`，注释明确：带这些前缀的变量**任何被发现的 .env 文件都不得设置**，只能来自继承的进程环境。
- 唯一例外是 `HOME_LAYER_PROXY_NAMES`（HTTP_PROXY/HTTPS_PROXY/ALL_PROXY/NO_PROXY），仅靠 harness home 下的 .env 可以设置；CA/TLS 类变量在任何层都被拒绝。

结论：隔离变量必须由启动器在 env 中下发，而**不能**寄希望于项目目录里的 .env。这同时意味着：项目仓库里放 .env 无法劫持或改变数据根，是一项安全属性。

### 2.3 各数据类别的位置（逐条对应源码）

| 数据类别 | 源码确定的位置 | 依据 |
|---|---|---|
| Profile 目录与插件依赖 | `$DSH_HOME/profiles/<名称>` | packages/boot/app-boot/src/profile.ts:169 resolveProfileDir，PROFILES_DIR = 'profiles' |
| Profile 用户覆盖层 | `$DSH_HOME/profiles/<名称>/cordis.patch.yml` | 同上 PROFILE_PATCH_FILENAME（profile.ts:40） |
| 会话 JSONL | `$DSH_HOME/sessions` | packages/bundle/base/cordis.patch.yml：session-persistence-jsonl.config.root = !!js dshHomePath('sessions') |
| 持久化 KV / 会话投影 | `$DSH_HOME/storages` | 同一文件：storage-json.config.root = !!js dshHomePath('storages') |
| 会话全文检索索引 | 基础 Bundle 为 path: ':memory:' + openAt: never（即不落盘、不打开） | 同上；注意插件 schema 自身默认是 openAt: 'startup'（packages/session-query/session-query-sqlite/src/index.ts:208），是基础 Bundle 把它覆盖成不落盘的 |
| 凭证 | `$DSH_HOME/.credentials.yaml` | packages/credentials/credentials-local/src/index.ts:61 CREDENTIALS_FILENAME，:90 默认拼在 harness home 下；分层顺序为 环境变量 > 该文件 > `$DSH_HOME/.env` |
| 附件实体 | `$DSH_HOME/attachments/v1` | packages/attachment/attachment-local/src/index.ts:175 |
| 附件缓存 | `$DSH_HOME/cache/attachments` | 同上 :176，走 dshCachePath |
| 匿名遥测身份 | `$DSH_HOME/.anonymous-user-id` | packages/identity/anonymous-user-id/src/index.ts:69 |
| Skills（用户级） | `$DSH_HOME/skills` 与 `$DSH_AGENTS_HOME/skills` | packages/skill/skill-filesystem/src/index.ts:244-260 |
| Desktop 主运行时装 | `$DSH_HOME/dsh-runtimes/dsh-primary-runtime` | apps/desktop-host/src/index.ts:98（**仅 desktop-host 使用，CLI/Web 路径不涉及**） |
| Desktop Profile 与锁 | `$DSH_HOME/profiles/desktop`、`$DSH_HOME/profiles/desktop/lock` | apps/desktop/src/paths.ts:17 |
| CLI 启动失败诊断日志 | `$DSH_HOME/logs/startup-<时间>-<uuid>.log` | apps/cli/src/startup-diagnostics.ts:58；**只在启动失败路径写入**，正常启动不产生 |
| 插件自带日志 | `$DSH_HOME/plugins/<...>/.plugin-manager/logs` 一类，由插件自定 | packages/boot/plugin-manager/src/operations.ts:290、github-connection.ts:38 |

补充说明：`~/.dsh/patches/`、`~/.dsh/speech-to-text/`、`~/.dsh/telemetry/<插件名>/` 这些子目录在 DSH 核心源码中**找不到**对应写入者，它们来自用户自行安装的第三方插件（本机可见 dsh-cost-meter 的补丁账本、dsh-univer-office 的状态文件、sensevoice 模型缓存）。这是「第三方插件会在 DSH_HOME 下落盘」的直接证据。

### 2.4 Profile 机制

- Profile 名合法性由 resolveProfileDir 校验，拒绝含 /、\、.、..、node_modules 以及空字符串的名字。
- 组合顺序：dsh.profile.bundles 中列出的 Bundle 层（按顺序）→ Profile 自身补丁层 → 启动器 `--patch` 覆盖层。
- CLI 参数（apps/cli/src/args.ts）：`dsh <name>` 等价于 `dsh --profile <name>`；web 是 Profile 名而非子命令；支持 `--from-default-profile`、可重复的 `--patch`、`--dump-config` / `--dump-config-schema` / `--dump-default-config`，以及 `dsh plugin --profile <name> <pnpm 参数>`。
- CLI 显式拒绝以 desktop 为名的 Profile（该名字由 Electron 应用专属管理）。

**关键事实**：packages/boot/config-editor/src/index.ts:34 的 `get documentPath()` 返回 profileContext.patchPath。也就是说，**Web 界面里的配置表单写的是「当前活动 Profile 的补丁文件」，不是某个全局文档**。packages/settings/settings/src/index.ts 也只在首次把旧的 join(profile.home,'settings.yaml') 导入并重命名为 settings.yaml.imported。

结论：隔离实例里改配置，物理落点必然是 `$OMNIDESK_DSH_HOME/profiles/yvan-work/cordis.patch.yml`。

### 2.5 端口、锁与多实例

- packages/bundle/web-app/src/startup.ts:64：`--host` 必须是某一个具体 IP 字面量，**通配地址被显式拒绝**（源码注释理由：绑定全部网卡等于把远程代码执行暴露给网络）；`--port` 支持；`--no-open` 支持不自动打开浏览器。
- packages/experimental/inspector/src/host/bridge/controller.ts:37-38,137：inspector 固定绑定 127.0.0.1，端口默认 0，注释写明「已占用端口会依次后移直到找到可用端口」。
- apps/desktop/src/single-instance.ts:20：Desktop 使用 Electron requestSingleInstanceLock()。
- 会话级互斥体现为 sessions/<工作区编码>/<会话 id>/session.lock（实测见 3.2）。

结论：端口层面不存在冲突风险，隔离实例默认可安全使用 `--port 0` 由系统分配；真正的多实例风险集中在 Desktop 外壳，而非 CLI/Web。

### 2.6 Desktop（Electron）与 DSH_HOME 是两套目录

- apps/desktop/src/main.ts:941 注释写明「保持 app.name 稳定：Electron 由它推导默认 userData 目录」。
- apps/desktop/scripts/dev.ts:67,88：开发启动器会读取 `DSH_DESKTOP_USER_DATA_DIR`，并向下传 `--user-data-dir=<path>`。
- apps/desktop/src/paths.ts:17：Desktop 的 Profile 与锁仍然在 `DSH_HOME` 下。

因此 Desktop 的隔离需要**同时**处理两个维度：`DSH_HOME`（数据根）与 Electron userData（外壳设置、快捷键、crashpad、日志）。

**关于用户 shim 的三点更正事实**（上一版报告此处有误）：

1. 文件内实际写的是 `export DSH_HOME='/home/kevin/.dsh'`（绝对路径），不是 `~/.dsh`；同时还会 `export DSH_TELEMETRY_DISABLED=1`。
2. 它在设置变量**之前**先扫描 `$PATH`（跳过自身所在目录）寻找其它 dsh，一旦找到就 `exec` 它并直接接管。所以它的实际行为取决于 PATH 上还有什么。
3. 它兜底的 `DSH_BIN` 指向 `~/.local/share/io.github.hairyf.deepseek-harness-desktop/dependencies/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js`，而该 dependencies/ 目录当前为空 —— 如果没有其它 dsh 抢先接管，它会以「Harness CLI not found」失败。

结论不变但理由更充分：**隔离启动必须绕过这个 shim**，直接调用 1.2 中的 asar 入口并自行下发环境变量。

---

## 3. 已由本机实测证实的事实

### 3.1 三实例并行运行矩阵（2026-10-15 观测）

| PID | 启动时刻 | DSH_HOME | DSH_AGENTS_HOME | Web 端口 | 监听地址 |
|---|---|---|---|---|---|
| 59932 | 15:56:54 | /home/kevin/OmniDesk/runtime/dsh-home | .../runtime/agents-home | 3210 | 127.0.0.1 |
| 69851 | 17:34:03 | /tmp/omnidesk-repro/runtime/dsh-home | .../runtime/agents-home | 3300 | 127.0.0.1 |
| 109609 | 19:02:46 | /tmp/omnidesk-stale-repro/runtime/dsh-home | .../runtime/agents-home | 42415 | 127.0.0.1 |

三者都已 `ELECTRON_RUN_AS_NODE=1`，都以 `--profile yvan-work` 启动。第三例的端口 42415 是由 `--port 0` 交给系统分配得到的，直接证实了动态端口在实际发行入口中生效。

原版 Desktop 同时运行：Electron 主进程 PID 144862，host 进程 PID 145373。/proc/145373/environ 中**只有** `DSH_CLIENT_VERSION=0.2.1-alpha.2`，没有 `DSH_HOME` —— 说明它走默认解析，即 `~/.dsh`。

HTTP 探测：http://127.0.0.1:3210/ 与 http://127.0.0.1:3300/ 均返回 401 Unauthorized，说明服务确实在跑且带鉴权，不是残留监听。

### 3.2 文件描述符级隔离证据（本报告最强的一条）

对每个进程枚举 /proc/<pid>/fd，统计指向 `~/.dsh` 或 `~/.agents` 的描述符数量：

| 进程 | 指向原版数据根的 fd 数 | 实际持有的数据描述符 |
|---|---|---|
| 59932（OmniDesk） | **0** | 仅 `~/OmniDesk/runtime/dsh-home/sessions/.../session.lock` 与 `/tmp/omnidesk-web-try.log` |
| 69851（OmniDesk） | **0** | 仅 `/tmp/omnidesk-repro/runtime/dsh-home/sessions/.../session.lock` 与 `/tmp/omnidesk-repro.log` |
| 109609（OmniDesk） | **0** | 仅其自身隔离根 |
| 145373（原版 Desktop host） | **2** | `/home/kevin/.dsh/sessions/--home-kevin-Sites--/session-.../session.lock`、`/home/kevin/.dsh/sessions/--mnt-data-AI收集--/session-.../session.lock` |

这说明隔离实例不仅在配置层分离，在**运行期实际打开的文件**层面也没有触碰原版数据根；同时原版 Desktop 确实仍在正常使用它自己的 `~/.dsh`，未被干扰。

### 3.3 配置分离证据

- 仓库 profiles/yvan-work/cordis.patch.yml 与隔离实例 `~/OmniDesk/runtime/dsh-home/profiles/yvan-work/cordis.patch.yml` **字节一致**（diff 无输出）。隔离实例确实只加载了仓库提供的模板。
- 原版 `~/.dsh/profiles/desktop/cordis.patch.yml` 与仓库 profiles/desktop/cordis.patch.yml **不同**：前者 welcomeNoticeVersion 为 2026-08-13.1，后者为 2026-09-28.1；前者还多出 opendesign provider 段。这证明**仓库里的 Profile 副本不是原版 Desktop 正在读取的那一份**，两者没有共享。
- 匿名身份不同：隔离根 9f4ef34e-de77-4671-b3a0-843671253ab7，原版 23a8aefa-86e8-40fa-bb85-842664dcd60f。
- 凭证文件不同且未被复制：隔离根 .credentials.yaml 为 161 字节（自动生成），原版为 1206 字节。
- 原版 `~/.dsh/skills` 为**空目录**（0 条目），`~/.agents` 为**空目录**，`~/bin/dsh` 不存在。也就是说隔离实例的 8 个 Skill 目录没有任何可能流向原版 Skills 根。

隔离根当前布局：

    ~/OmniDesk/runtime/
    ├── dsh-home/
    │   ├── .anonymous-user-id
    │   ├── .credentials.yaml      # 自动生成，未从原版复制
    │   ├── profiles/{yvan-work,omnidesk-test}
    │   ├── sessions/{--tmp--,--mnt-data-Documents-deepseek-harness-default-workspace--}
    │   ├── skills/                # 8 个 Skill 目录
    │   └── storages/session_projcache
    ├── agents-home/               # 空
    └── desktop-user-data/         # 已预留，当前未使用

### 3.4 原版 DSH 在观测窗口内的写入归因

以 2026-10-15 00:00 为界筛选 `~/.dsh` 下的新写文件，命中对象及归因：

| 路径 | 时间 | 归因 |
|---|---|---|
| profiles/desktop/cordis.patch.yml | 14:09 | 原版 Desktop（用户在 Web 配置页的改动） |
| .credentials.yaml | 14:09 | 原版 Desktop |
| profiles/desktop/、profiles/desktop/cordis.yml | 20:17 | 原版 Desktop 启动时重写组合结果 |
| attachments/v1/tmp | 20:22 | 原版 Desktop |
| storages/session_projcache/sessions | 20:30 | 原版 Desktop 会话投影缓存 |

全部可归因于 PID 144862/145373 这条 Electron 链路；没有任何一条落在 OmniDesk 进程的写入路径上（3.2 已从 fd 层面独立佐证）。

**必须诚实记录的一点**：原版 Desktop 在观测时间窗内仍在持续写入 `~/.dsh`。这意味着任何「前后哈希比对」式的验收都不能简单地把差异判为泄露，而必须结合写入者归属判断。仓库自检脚本的结论之所以成立，是因为它在同一窗口内同时检查了两侧。

### 3.5 自动化隔离自检结果

执行 scripts/omnidesk-isolation-check.sh，结果：**通过 37 项，失败 0 项，退出码 0**。

覆盖的十组断言与全部结论：

1. 前置检查：两个脚本存在。
2. Bash 语法检查：omnidesk-dev.sh、sync-skills.sh 均通过。
3. 基线记录：已对原环境建立快照。
4. 启动器默认隔离边界：`--check` 通过；`--dry-run` 命令行确实含独立 DSH_HOME、独立 DSH_AGENTS_HOME、yvan-work Profile、回环地址、动态端口；且**命令行中不含任何原版数据路径**。
5. 危险隔离根拒绝：`OMNIDESK_ROOT=~/.dsh`、`=~/.agents`、以及落在两者之内的路径全部被拒。
6. Profile 名校验：`../evil`、`a/b`、`desktop`、`node_modules`、`..` 全部被拒。
7. 透传参数白名单：`--host=0.0.0.0`、`--profile desktop`、`--patch`、`--from-default-profile desktop`、`--public-url`、`--trusted-host`、`--dump-config` 全部被拒。
8. 允许的透传参数：`--port 3099`、`--no-open` 放行；web 别名被正确归一化。
9. Skill 同步目标校验：拒绝写入 `~/.dsh` 与 `~/.agents`；dry-run 未改变隔离根结构。
10. 原环境零变化复核：`/home/kevin/.dsh` 与 `/home/kevin/.agents` 在本轮测试中**零变化**。

---

## 4. 尚未证实的推断与边界

以下内容目前**没有**直接证据，属于推断，不应作为决策前提：

1. **生产版 Desktop 外壳能否接受独立 userData。** `DSH_DESKTOP_USER_DATA_DIR` 与 `--user-data-dir` 仅在 apps/desktop/scripts/dev.ts 中得到源码证实，那是开发启动器。已安装的 `/opt/DeepSeek Harness/deepseek-harness` 是否完整透传该参数、以及两个不同 userData 的实例能否绕开单实例锁存续，尚未实测。
2. **所有发行入口都保留自定义 DSH_HOME。** 已实测的是 asar CLI + Web bundle 这条路径。Desktop、headless、以及其它可能被封装的入口未逐一验证。
3. **可选插件的写盘边界。** 未逐项审计。已知第三方插件会在 DSH_HOME 下落盘（见 2.3 补充说明），但它们是否还写 `~/.config`、系统临时目录或访问外部服务，未做检查。
4. **观测窗口内写入的完整归因。** 3.4 的归因基于时间戳与进程归属判断，未使用内核审计子系统记录写入者 PID，因此属于强推断而非铁证。
5. **`--port 0` 之外的行为。** 显式指定占用端口时的报错形态、以及 inspector 端口在极端占用下的表现，未做压力验证。
6. **`.dsh` 与 `.agents` 之外的项目级 Skill 根。** skill-filesystem 还会扫描 `<项目根>/.dsh/skills` 与 `<项目根>/.agents/skills`（等级 100/200，优先级高于用户级）。当前仓库不存在这两个目录，但未来若创建，需明确其归属。此机制本身已由源码证实，属于设计事实而非风险；列为边界是因为它决定了「把 Skill 放在哪」这一选择。

---

## 5. 对上一版审计的更正（勘误）

上一版 docs/OmniDesk-DSH-Isolation-Audit.md（提交 f2befc1）包含四处与源码/实测不符的表述，本版已修正：

| 项 | 原表述 | 更正 |
|---|---|---|
| shim 内容 | 「显式 export DSH_HOME='~/.dsh'」 | 实为绝对路径 /home/kevin/.dsh；且它会先扫 `$PATH` 抢 exec；其兜底 DSH_BIN 目标不存在 |
| 数据类别表 | 虚构了若干默认值，并把 SQLite 描述为「内存且默认不打开」但未指出 schema 默认值与 Bundle 覆盖的区别 | 已逐条改为带源码行号的确定位置；明确区分「插件 schema 默认 openAt: 'startup'」与「基础 Bundle 覆盖为 :memory: / never」 |
| Desktop userData 命名 | 把 `~/.config/DSH Desktop` 当作当前生效目录 | 当前生效的是 `~/.config/deepseek-harness`（实测由 `--user-data-dir` 指定，今日 20:27 仍在更新）；`~/.config/DSH Desktop` 与 `~/.config/@deepseek-ai` 均为 2026-10-01 的陈旧目录 |
| 环境现状 | 未记录 `~/.dsh/skills` 与 `~/.agents` 的实际状态 | 两者均为**空目录**；`~/bin/dsh` 不存在 |

同时补充了上一版缺失的两类证据：进程文件描述符级隔离证据（3.2）与自动化自检的 37 项结果（3.5）。

---

## 6. 推荐方案及理由

### 6.1 总体方案：双根 + 专用 Profile + 专用入口

| 维度 | 取值 | 理由 |
|---|---|---|
| DSH 数据根 | `~/OmniDesk/runtime/dsh-home` | 由官方 `DSH_HOME` 重定向；源码证实它同时带走 sessions、storages、profiles、skills、credentials、cache、attachments |
| Agent 共享根 | `~/OmniDesk/runtime/agents-home` | 由官方 `DSH_AGENTS_HOME` 重定向；避免读到 `~/.agents` 这一跨工具共享根 |
| Profile | yvan-work（禁止 desktop、node_modules） | Profile 名由运行时强制校验；使用专用名可保证 config-editor 的写入落点独立 |
| 启动入口 | `/opt/DeepSeek Harness/deepseek-harness` + `ELECTRON_RUN_AS_NODE=1` + asar 内 bin.js | 唯一被实测证实可用的入口；避开会重设 DSH_HOME 的 shim |
| 监听 | `--host 127.0.0.1 --port 0 --no-open` | 源码拒绝通配地址；端口 0 由系统分配，实测得到 42415 这类随机端口 |
| 遥测 | 保留 `DSH_TELEMETRY_DISABLED`（可选） | 隔离根已自带独立匿名身份，可按需再关 |
| 知识库与业务数据 | 仓库之外的独立目录 | 数据资产与运行环境解耦；knowledge/ 内容不入库 |

### 6.2 为什么优先用官方机制而不是自研隔离

1. `DSH_HOME` 覆盖的是**整棵数据树**，不是零散几个路径。逐个路径去软链或改写，反而会引入「部分隔离」这种最难排查的状态。
2. 环境变量属于启动级变量，.env 无法篡改，这给了隔离一层结构性保障。
3. Profile 机制把「配置落点」绑定到活动 Profile 的补丁文件，天然做到一实例一配置。

### 6.3 关于 Desktop（Electron）

推荐**分阶段**：先只跑 CLI/Web 隔离实例（当前已跑通），Desktop 隔离作为独立课题，在实测出生产入口的 `--user-data-dir` 行为与单实例锁行为之后再启用。在验证完成之前，不建议让隔离 Desktop 与原版 Desktop 同时运行。

### 6.4 关于「最小改动」的具体边界

需要新增/维护的仅是仓库自身的脚本护栏，共三处：

- scripts/omnidesk-dev.sh：下发隔离环境并做硬性预检（隔离根非空、必须位于 OmniDesk 根之下、必须落在 `~/.dsh`/`~/.agents` 之外、两者不得相同、Profile 名合法、透传参数白名单）。
- scripts/sync-skills.sh：同步目标由已校验的隔离根推导，显式拒绝 `~/.dsh/skills` 与 `~/.agents/skills`、拒绝软链接目标、拒绝空源。
- scripts/omnidesk-isolation-check.sh：把上述护栏做成可重复执行的回归测试，并用前后哈希证明零改动。

**不需要**修改 Runtime、不需要打补丁、不需要 fork。

---

## 7. 实施步骤

按可独立验收的顺序排列：

1. **固化入口探测**。启动器以「执行探测」判定 asar 内 CLI 是否可用（`-f` 在归档上无效），失败时明确报出探测命令与退出码。
2. **初始化隔离根**。创建 dsh-home、agents-home，从仓库 profiles/yvan-work 模板初始化 Profile。**禁止**从 `~/.dsh` 复制凭证、会话、插件目录、node_modules。
3. **启动隔离实例**。以 `--profile yvan-work --host 127.0.0.1 --port 0 --no-open` 启动，启动前打印实际解析出的两个根与 Profile，供人工核对。
4. **建立测试会话**。在隔离实例中创建带明显标记的会话（例如工作区选择 /tmp/omnidesk-isolation-probe），确认文件只出现在隔离根 sessions/ 下。
5. **修改测试配置**。通过 Web 配置页改一个无敏感项，确认写入的是 `$OMNIDESK_DSH_HOME/profiles/yvan-work/cordis.patch.yml`，且原版 `~/.dsh/profiles/desktop/cordis.patch.yml` 的内容与 mtime 不变。
6. **加载测试 Skill**。把仓库 Skills 同步到隔离根，确认来源为隔离 skills/ 或 agents-home/skills；同步前后原版 Skills 根的状态不变。
7. **执行隔离回归**。运行 scripts/omnidesk-isolation-check.sh，要求全绿。
8. **（可选，独立课题）Desktop 隔离**。为 Electron 指定独立 userData 后单独验证单实例行为；在通过之前不与日常 Desktop 并行。

---

## 8. 隔离验收标准

- [ ] 启动日志显示 DSH_HOME、DSH_AGENTS_HOME、Profile 均位于 OmniDesk 测试根，且两者互不相同、均不在 `~/.dsh`、`~/.agents` 之内或之下。
- [ ] DSH 成功启动；Web 服务只绑定 127.0.0.1；端口来自系统分配或明确空闲端口；实测 HTTP 返回 401 而非连接失败。
- [ ] 创建测试会话后，会话文件只出现在隔离根 sessions/ 下；原版 `~/.dsh/sessions` 无新增目录。
- [ ] 修改测试配置后，改动只写入隔离 Profile 的 cordis.patch.yml；原版 Desktop 的补丁文件内容与 mtime 不变。
- [ ] 加载测试 Skill 后，其来源为隔离根；原版 `~/.dsh/skills` 与 `~/.agents` 仍为空。
- [ ] 对隔离实例进程枚举 /proc/<pid>/fd，指向 `~/.dsh` 与 `~/.agents` 的描述符数为 **0**。
- [ ] scripts/omnidesk-isolation-check.sh 全部通过（当前基线：37/37）。
- [ ] runtime/deepseek-harness 的 git status --porcelain 为空，HEAD 仍为 d743267388641bc76f17c45ce8b4c231aed1d32c。
- [ ] 隔离实例关闭后，原版 DSH 仍可正常启动并读写自己的数据。

**关于哈希比对的注意事项**：原版 Desktop 会在任意时刻写入 `~/.dsh`（3.4 已记录）。因此前后哈希比对必须同时判断写入者归属，不能把原版自身产生的差异计入隔离失败。

---

## 9. 风险清单

| 风险 | 证据状态 | 应对方式 |
|---|---|---|
| 用户 shim 重设 DSH_HOME 并抢先 exec PATH 上的其它 dsh | 源码已证实 | 绕过 shim，直接调用 asar 入口并显式下发环境变量 |
| Electron 外壳数据仍共享 | 路径分离已证实；生产入口的独立 userData 未实测 | Desktop 隔离单列课题；验证通过前不与原版并行运行 |
| 误用原 Profile / 插件数据 | Profile 与插件位于 DSH_HOME 下已证实 | 全新 DSH_HOME、专用 Profile、禁止跨根软链接 |
| Skill 同步误写生产目录 | 脚本护栏已实现并自检通过 | 保持目标由校验后的隔离根推导；执行前先 `--dry-run` |
| 端口占用 | 基础 Bundle 默认 3080；`--port 0` 实测可用（42415） | 回环绑定 + 系统分配端口 |
| 全文检索落盘 | 基础 Bundle 为 :memory: + never 已证实 | 未来若要启用持久索引，显式把 path 指向隔离根 |
| 第三方插件在 DSH_HOME 之外写盘 | 已知有插件写 DSH_HOME 内；外部边界未审计 | 安装前逐项审查副作用；隔离验证时纳入监听范围 |
| 版本升级破坏隔离 | 隔离依赖的是官方环境变量与 Profile 机制 | 升级后在隔离实例跑自检脚本与 Golden Tasks，再决定是否更新日常实例 |
| 归档路径无法用 `-f` 探测 | 已实测（Not a directory） | 统一改用执行探测 |

---

## 10. 明确不在本次范围

- 不安装任何新组件，不执行任何迁移，不删除任何原有 DSH 数据。
- 不修改 `/opt/DeepSeek Harness`、不修改 `~/.local/bin/dsh`、不修改 `~/.dsh`、`~/.agents`、`~/.config/deepseek-harness`。
- 不修改 runtime/deepseek-harness（保持与上游标签一致）。
- 不开发 GUI、不引入 Core 层、不引入 Agent 市场。
- 未审计第三方插件的完整副作用；未验证生产版 Desktop 的多实例行为。

---

## 11. 关键源码与脚本索引

源码（均在 runtime/deepseek-harness 下）：

- packages/util/home-paths/src/index.ts —— 数据根解析（resolveDshHome / resolveAgentsHome / dshHomePath / dshCachePath）
- packages/boot/app-boot/src/index.ts —— 启动级环境变量与 .env 拒绝规则
- packages/boot/app-boot/src/profile.ts —— Profile 目录、补丁文件名、名字校验
- packages/boot/app-boot/src/profile-context.ts —— 补丁层组合顺序与遥测开关
- packages/boot/config-editor/src/index.ts —— 配置写入落点 = 活动 Profile 补丁
- packages/settings/settings/src/index.ts —— 旧 settings.yaml 的一次性导入
- packages/bundle/base/cordis.patch.yml —— sessions / storages / SQLite / 遥测默认值
- packages/credentials/credentials-local/src/index.ts —— .credentials.yaml
- packages/attachment/attachment-local/src/index.ts —— attachments/v1 与 cache/attachments
- packages/identity/anonymous-user-id/src/index.ts —— .anonymous-user-id
- packages/skill/skill-filesystem/src/index.ts —— Skill 根目录与优先级
- packages/session-query/session-query-sqlite/src/index.ts —— SQLite schema 默认值
- packages/bundle/web-app/src/startup.ts —— `--host` / `--port` / `--no-open`
- packages/experimental/inspector/src/host/bridge/controller.ts —— inspector 固定回环与端口后移
- apps/cli/src/args.ts —— CLI 参数与 desktop 名字保留
- apps/cli/src/startup-diagnostics.ts —— `$DSH_HOME/logs` 仅失败时写入
- apps/desktop/src/paths.ts、apps/desktop/src/main.ts、apps/desktop/scripts/dev.ts、apps/desktop/src/single-instance.ts —— Electron 路径与单实例
- apps/desktop-host/src/index.ts —— dsh-runtimes/dsh-primary-runtime

仓库脚本：

- scripts/omnidesk-dev.sh —— 隔离启动器（预检 + 入口探测 + 环境下发）
- scripts/sync-skills.sh —— 隔离安全的 Skill 同步
- scripts/omnidesk-isolation-check.sh —— 隔离回归自检（37 项）

---

## 附：本次审计未能完成的事项

1. Desktop/Electron 生产入口的独立 userData 与双实例行为未实测（需要启动第二个 Electron 实例，超出本次「只读审核」范围）。
2. 第三方插件的外部写盘边界未审计。
3. 3.4 的写入归因基于时间戳与进程归属，未使用内核审计子系统获取写入者 PID。
4. `--port 0` 之外的端口冲突形态未做压力验证。
