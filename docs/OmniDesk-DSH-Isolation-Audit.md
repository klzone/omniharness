# OmniDesk 与 DSH 运行隔离审计

审计日期：2026-10-15  
审计对象：仓库内 DSH v0.2.1-alpha.2 源码及本机 DSH 安装布局  
范围：只读源码和路径检查。本次未启动 DSH、安装软件、写入 DSH 数据或修改 DSH 源码。

## 结论

源码证实 DSH 支持用官方环境变量 DSH_HOME 将主要可变数据移到独立根目录；DSH_AGENTS_HOME 可以独立指定 Agent 指令与 Skills 的共享根。因此，OmniDesk 可以通过独立 DSH 数据根运行，无需修改 DSH Runtime。

隔离 Desktop 还需处理 Electron 自己的 userData 路径。它与 DSH_HOME 是两套目录。当前系统生成的 ~/.local/bin/dsh shim 会把 DSH_HOME 固定重设为 ~/.dsh，所以不能用该 shim 启动隔离实例。仓库的 Skill 同步脚本也把目标写死为 ~/.dsh/skills，执行会触碰现有安装。

推荐先通过直接调用已安装 DSH CLI 的 OmniDesk 专用启动器验证 CLI/Web 流程，设置独立 DSH_HOME、DSH_AGENTS_HOME 和 yvan-work Profile。后续需要 Electron Desktop 时，再增加独立 --user-data-dir 并验证单实例行为。不要从现有 ~/.dsh 复制凭证、会话、插件或其他可变数据。

## 版本与审计方法

- 仓库上游源码位于 runtime/deepseek-harness，标签 dsh-v0.2.1-alpha.2，提交 d743267388641bc76f17c45ce8b4c231aed1d32c，包版本 0.2.1-alpha.2。
- 本机系统包版本为 deepseek-harness 0.2.1~alpha.2。另有旧 Desktop 用户数据，其 store 文件记录的包标签为 dsh-0.1.1-rc.2-32485170079；这表明桌面外壳数据与当前系统包版本不能简单视为同一套状态。
- 检查了路径解析、Profile 加载、基础 Bundle、Web 配置、Skills、CLI、Desktop 路径和本机启动器。没有运行 DSH。

## 已由源码证实的事实

### 数据根与启动环境

packages/util/home-paths/src/index.ts 定义 DSH_HOME 与 DSH_AGENTS_HOME。解析顺序是显式配置、环境变量、默认目录；默认分别为 ~/.dsh 和 ~/.agents。DSH_HOME 下的帮助函数 dshHomePath 与 dshCachePath 为各功能拼接路径。空白环境变量按未设置处理。

packages/boot/app-boot/src/index.ts 将 DSH_ 前缀变量视为启动级环境变量，拒绝由项目 .env 设置。隔离变量必须由启动进程环境传入。

### 数据类别及路径

| 数据 | 源码确定的默认位置 | 隔离方式 |
|---|---|---|
| Profile、补丁、插件依赖 | $DSH_HOME/profiles/<名称> | 设置独立 DSH_HOME 和 Profile 名称 |
| 会话 JSONL | $DSH_HOME/sessions | 设置独立 DSH_HOME |
| 持久化存储和会话投影 | $DSH_HOME/storages | 设置独立 DSH_HOME |
| 凭证 | $DSH_HOME/.credentials.yaml | 不要配置为现有凭证路径 |
| 附件缓存 | $DSH_HOME/cache/attachments | 设置独立 DSH_HOME |
| 通用缓存 | $DSH_HOME/cache | 设置独立 DSH_HOME |
| Desktop 下载的主运行时 | $DSH_HOME/dsh-runtimes/dsh-primary-runtime | 设置独立 DSH_HOME |
| Skills 与 Agent 指令 | 默认搜索 DSH_HOME、DSH_AGENTS_HOME 下的根；也可声明自定义目录 | 两个环境变量都设为独立路径，并审查 Skill 配置 |
| 会话全文检索 | 当前 Bundle 使用 SQLite 内存路径，且默认不打开 | 将来启用持久索引时另设隔离文件路径 |
| CLI 诊断日志 | DSH_HOME/logs | 设置独立 DSH_HOME |
| 匿名遥测身份 | DSH_HOME/.anonymous-user-id | 随 DSH_HOME 分离；可用 DSH_TELEMETRY_DISABLED 关闭 |

基础 Bundle 的会话根为 dshHomePath('sessions')，JSON 存储根为 dshHomePath('storages')。附件模块用 DSH_HOME 下的缓存目录。凭证模块默认将文件放在 DSH_HOME。Profile 目录和插件的 node_modules 也位于该根目录。

### Profile 与启动入口

CLI 支持 --profile、--from-default-profile 和 --patch。Profile 位于 DSH_HOME/profiles/<名称>，由 Bundle 补丁、Profile 自身补丁及命令行补丁按顺序组合。

本机 ~/.local/bin/dsh 是桌面应用生成的 shim，其中显式 export DSH_HOME='~/.dsh'。调用它会覆盖外层传入的隔离值。应绕过该 shim，不能为了隔离去改写它。

### Desktop、端口与锁

Desktop 使用 Electron app.getPath('userData') 保存外壳设置、快捷键和日志等；该目录不由 DSH_HOME 自动决定。开发启动器通过 DSH_DESKTOP_USER_DATA_DIR 和 Electron 的 --user-data-dir 指定独立目录。桌面源码默认保持应用名稳定，并使用 Electron 单实例锁。

Desktop Profile 及其准备操作锁位于 DSH_HOME/profiles/desktop。Desktop 进程还有 Electron 单实例锁。凭证写入另有跨进程文件锁；这些锁只保护各自的资源，不代表所有文件都有全局互斥。

Web Server 接受端口 0，由系统分配可用端口。Web Bundle 默认端口为 3080，默认地址为 127.0.0.1。隔离测试可将监听限制在回环地址并申请端口 0。

### 本机现状与 OmniDesk 仓库风险

- ~/.dsh 不是空目录，包含会话、Profiles、Skills、存储、缓存、附件、运行时、遥测和凭证等数据。
- ~/.config/DSH Desktop 存在 Electron 数据，包括 Profile 选择、日志、身份、健康快照和运行状态。
- 仓库 profiles/yvan-work/package.json 与 profiles/desktop/package.json 相同，仍使用 dsh-profile-desktop 名称和 Desktop/Web Bundle 组合；它尚未验证为专用 OmniDesk Profile。
- scripts/sync-skills.sh 将目标写死为 $HOME/.dsh/skills，并使用 rsync --delete。执行会改动现有 DSH 的 Skill 数据，并可能删除目标 Skill 目录中源端不存在的文件。隔离脚本替换之前不可运行它。

## 尚未证实的推断与边界

- 源码支持 Desktop 使用进程环境里的 DSH_HOME 构造 Profile 和运行时路径，但本次没有启动已安装桌面程序验证所有发行入口都会保留自定义环境变量。
- Electron 开发入口支持独立 userData 参数；本次没有验证已安装生产 Desktop 能否通过该参数并行启动第二个实例，也未实测单实例转发行为。
- 端口 0 的 Web Server 能力由源码证实；具体发行入口是否完整透传该参数，仍需在隔离测试根中验证。
- 尚未逐一审计所有可选插件。插件可能自行访问 DSH_HOME 之外的绝对路径、配置目录、环境变量或外部服务。
- 本机目录及文件大小是审计时观察值，不代表文件内容或恢复能力已验证。

## 推荐的独立运行方案

1. 为 OmniDesk 使用全新数据根，例如 ~/OmniDesk/runtime/dsh-home，以及独立 Agent 根 ~/OmniDesk/runtime/agents-home。启动时显式导出 DSH_HOME 和 DSH_AGENTS_HOME，并检查解析结果没有指向 ~/.dsh 或 ~/.agents。
2. 使用 yvan-work 专用 Profile，将仓库 Profile 模板复制到新数据根的 profiles/yvan-work。不要复用 desktop Profile，也不要把 Profile、插件目录软链接回现有 DSH。
3. 使用项目专用启动器直接调用系统安装目录中的 DSH CLI 入口和兼容 Node，绕过会重设 DSH_HOME 的用户 shim。启动器先显示并校验实际数据根和 Profile。
4. 首轮只验证 CLI/Web 工作流。若使用 Desktop，再为 Electron 设置独立 --user-data-dir；通过双实例测试前，不要与日常 Desktop 同时运行。
5. 回环地址绑定 Web 服务，使用端口 0 或显式空闲端口。未来启用持久 SQLite 索引时，把数据库明确放在 OmniDesk 专属数据根。
6. 将 Skill 同步目标改为由隔离 DSH_HOME 决定，再允许同步。任何同步脚本都不得默认写入 ~/.dsh/skills。
7. 个人知识和业务材料继续放在 Git 仓库以外的数据目录；测试实例不复制生产会话或凭证。

该方案使用 DSH 官方路径环境变量和 Profile 机制，不需要修改 Runtime。需要新增或调整的实现仅限 OmniDesk 仓库的启动、初始化和 Skill 同步脚本。

## 实施步骤与隔离验收

### 实施步骤

1. 查明本机 DSH CLI 入口和兼容 Node 路径；创建项目启动器，显式设置两个隔离根，并拒绝空路径或落入现有数据根的路径。
2. 在全新隔离根初始化 yvan-work Profile 和测试 Skill；不复制生产凭证、会话或插件。
3. 检查 Profile 和插件补丁，确认没有绝对指向 ~/.dsh 或 ~/.agents 的写入路径。
4. 如需 Desktop，另设 Electron userData，并单独验证其锁与启动行为。
5. 测试前后记录现有 ~/.dsh、~/.agents、~/.config/DSH Desktop 和桌面应用数据目录的文件清单、修改时间与内容哈希。

### 验收标准

- [ ] 启动记录显示 DSH_HOME、DSH_AGENTS_HOME 和 Profile 都位于 OmniDesk 测试根。
- [ ] DSH 成功启动；Web 服务只绑定 127.0.0.1，并使用系统分配端口或明确空闲端口。
- [ ] 创建带有明显测试标记的会话，确认会话文件只出现在隔离根 sessions/。
- [ ] 修改无敏感信息的测试配置，确认只写入隔离根 profiles/yvan-work/。
- [ ] 加载专用测试 Skill，确认其来源为隔离 DSH 根或 Agent 根。
- [ ] 若测试 Desktop，其 userData 位于专用目录，且单实例行为已验证。
- [ ] 测试前后对比显示 ~/.dsh、~/.agents、~/.config/DSH Desktop 和现有 Desktop 数据目录均没有测试造成的新增、修改或删除。
- [ ] runtime/deepseek-harness 仍与上游提交一致，没有 Runtime 源码改动。

## 风险清单

| 风险 | 证据状态 | 应对方式 |
|---|---|---|
| 用户 shim 重设 DSH_HOME | 当前 shim 源码已证实 | 绕过 shim，直接调用 DSH CLI 入口并校验启动环境 |
| Electron 数据仍共享 | Desktop 路径与 DSH_HOME 分离已证实；生产双实例尚未实测 | 指定独立 userData，验收前不同时运行 |
| 误用原 Profile 或插件数据 | Profile 与插件位于 DSH_HOME 下已证实 | 全新 DSH_HOME、专用 Profile，不跨根软链接 |
| Skill 同步触碰生产目录 | 当前仓库脚本已证实 | 替换目标逻辑前不运行 sync-skills.sh |
| 端口占用 | 默认端口 3080，端口 0 可用已证实 | 回环绑定并申请系统分配端口 |
| SQLite 文件共用 | 当前默认内存且关闭自动打开已证实 | 持久化时明确配置隔离文件路径 |
| 插件在外部路径写数据 | 尚未逐项审计 | 安装前检查每个插件的路径和副作用 |

## 关键源码位置

- runtime/deepseek-harness/packages/util/home-paths/src/index.ts：数据根解析。
- runtime/deepseek-harness/packages/boot/app-boot/src/index.ts：启动环境和 .env 规则。
- runtime/deepseek-harness/packages/boot/app-boot/src/profile.ts、packages/cli/src/args.ts：Profile 加载及 CLI 参数。
- runtime/deepseek-harness/packages/bundle/base/cordis.patch.yml：会话、存储、检索默认设置。
- runtime/deepseek-harness/packages/bundle/web-app/cordis.patch.yml、packages/host/webserver/src/index.ts：Web 地址和端口。
- runtime/deepseek-harness/packages/skill/skill-filesystem/src/index.ts：Skills 根目录。
- runtime/deepseek-harness/apps/desktop/src/main.ts、apps/desktop/src/paths.ts、apps/desktop/scripts/dev.ts：Electron 与 DSH 数据路径。
- scripts/sync-skills.sh、profiles/yvan-work/package.json：OmniDesk 仓库中待处理的隔离缺口。
