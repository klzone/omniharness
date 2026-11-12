# OD-PROFILE-092 证据

- 日期：2026-11-12
- 改动：新增 `discipline-core` Agent preset，默认指向它；保留 `preset-standard`
- 基线 HEAD：`5af890a84333a50a743347f45a66c9642b1dcf05`
- 未升级 DSH，未比较 Pi，未启用子 Agent，未调用模型

## 配置事实

| 对象 | 插件行数 | 内容 |
| --- | ---: | --- |
| `preset-standard` | 22 | 完整上游副本，保留回滚 |
| `discipline-core` | 9 | persona、agent-instructions、time-context、tool-fs（read-only）、skill-filesystem、tool-skill、compaction、tool-ask-user、tool-web |

`preset-discipline-core` 通过 `insert` 加入 Profile；`agent-preset-registry.config.default=discipline-core`。`preset-standard`、`preset-ptc`、`preset-minimal`、`preset-cordis` 在活动 Profile 中均 disabled。实际数据根写入前备份：

`~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.patch.yml.bak-20261112T222209`

## 命令与结果

| 命令 | 结果 |
| --- | --- |
| Ruby YAML parse（sample/seed/actual） | 5/5/6 个顶层条目，均通过 |
| 临时 `DSH_HOME` + `--dump-config` | rc=0；default=discipline-core；2 个 Profile patch 解析通过；tool-fs=`read:true, readImage:false, write:false, edit:false` |
| 临时 DSH 启动探针（当前 Profile） | 返回 `dsh web: http://127.0.0.1:<port>` |
| 临时 DSH 启动探针（旧备份 Profile） | 返回 web 地址；与当前 Profile 相同，均出现既有 `dsh-free-search` peer import warning |
| `npx tsc --noEmit` | exit 0 |
| `npm run build` | exit 0 |
| `cargo test --offline --lib profile_rules::tests` | 16/16 |
| `verify_profile_scope_106.py` | 43/43 |
| `verify_profile_assets_109.py` | 41/41 |

## 未验证

- 真实模型下的四组业务体验与工具选择质量。
- 已打开 session 的 composition（DSH 设计上保留创建时的 preset revision）。
- UI 用户 `selectedDefault` 是否覆盖 Profile 默认。
- `dsh-free-search` peer import warning 为旧/新 Profile 共有的既有现象，本次未修复。

## 体积与内存对照（2026-11-12 实测）

- 当前构建的 `OmniDesk.app`（0.2.23）：`du -sh` **631 MB**；Tauri 构建日志记 **542.06 MiB**。前一个 `0.2.22` 的 `/Applications/OmniDesk.app` 当前已不存在，不能做可靠的整包 A/B；不要用旧版历史数字代替。
- Profile 配置本身：旧备份 **7,957 bytes**，新 patch **10,563 bytes**，增加 **2,606 bytes**。本次没有删除插件包或 runtime 包，所以不会因为工具面精简而减少整包体积。
- 同版本 DSH 隔离启动（仅空载，不加载真实模型；三轮中位数）：
  - 旧完整 preset：`ps` RSS **377,936 KB（约 369.1 MiB）**；`footprint` **334 MB**。
  - 新 `discipline-core` 且 built-in presets 全部 disabled：`ps` RSS **361,488 KB（约 353.0 MiB）**；`footprint` **316 MB**。
  - 变化约 **-16.1 MiB RSS / -18 MB footprint**。
- 原因：四个 built-in preset 均在活动 Profile 中 disabled，不再预先激活其插件树。插件包仍在 runtime 中，所以安装包体积不变。

### Runtime 剪枝实验（仅在 `/tmp` 副本，未改仓库 runtime）

基线 runtime `runtime/dsh-runtime` 约 **545 MB**、完整 app `du -sh` **631 MB**。

- **保留 Office**：从 runtime 副本移出 subagent、workflow、bash/pwsh、jobs、schedule、goal/todo、planning、fs-search、terminal 及其 UI 包后，runtime 约 **523 MB**；按已测 runtime 差值估算完整 app 约 **609 MB**，约减少 **22 MB**。DH​S web 启动探针通过；仅保留既有的 `dsh-free-search` peer import warning。
- **再移除 Office**：继续移出 `libreoffice-kit*`、`office-to-pdf`、`skill-office`、`tool-workspace-dependencies` 后，runtime 约 **372 MB**；复制 app 实测 **435 MB**（同时移出 `pnpm` 23 MB），相对完整 app 减少约 **196 MB（约 31%）**。DSH runtime 启动探针通过；空载三轮中位数 RSS **324,112 KB**（约 316.5 MiB）、footprint **278 MB**，相对完整 preset 约减 **53.8 MiB RSS / 56 MB footprint**。
- `OmniDesk-Ultra.app` 的 app 层启动测试遇到既有 seed `skills/SKILL.md` 结构错误，且 profile 仍有 `skill-office` 依赖尝试自动安装；因此只确认 runtime 层启动，不把 app 层标为通过。

### 第一批/第二批剪枝测试

- 从原始 runtime 移除语音、`node-pty`、terminal、inspector、agent-team、documentpreview、`dsh-web-frontend` 和全部 `dsh-client-ui-*`，并停用对应 UI 行后，web 启动最初失败：`dsh-web-app` 强制要求 `dsh-web-frontend`，`directory-picker-auto` 强制要求 `dsh-client-ui-directory-picker-native`。
- 保留 `dsh-web-frontend` 和 `dsh-client-ui-directory-picker-native` 后，剪枝 runtime **420 MB**（原 545 MB），复制 app **505 MB**（原 631 MB），启动探针成功；空载三轮中位数 RSS **338,064 KB（约 330.1 MiB）**、footprint **293 MB**。
- 结论：第一批（语音/终端/inspector/agent-team/documentpreview）可删；第二批的 `dsh-web-frontend` 和 `directory-picker-native` 不能直接删，否则 `web-app` 启动失败。其余 client UI 仍需按行验证，不能把“用户自有 UI”直接理解为 DSH web app 不依赖任何 client 包。

## 092a 修订（2026-11-13）：恢复 write/edit 与平台 shell

**触发：** 用户实测 0.2.23 精简版导出 Excel，模型回复「只读能力、无写文件与脚本工具」。用户选定方案 A。

**改动（三份 patch 同步）：**

| 行 | 值 |
| --- | --- |
| `tool-bash` | `disabled: !!js process.platform === 'win32'` |
| `tool-pwsh` | `disabled: !!js process.platform !== 'win32'` |
| `tool-fs` | `read: true, readImage: false, write: true, edit: true` |
| `tool-fs-search` | `sampleOverCapGlobResults: false` |
| `tool-workspace-dependencies` | 未加入（`Config.source` 必填，OmniDesk 无 primary runtime payload） |

**命令与结果：**

| 命令 | 结果 |
| --- | --- |
| `DSH_HOME=/tmp/od-dsh-home-probe-3 dsh --profile discipline --dump-config` | rc=0；新四条工具行在 `discipline-core` 内 |
| `DSH_HOME=/tmp/od-dsh-home-probe-3 dsh --profile discipline --port 0 --no-open` | 输出 `dsh web: http://127.0.0.1:<port>`，stderr 为空 |
| `import()` 探针（7 包） | 7/7 ok：tool-bash、tool-pwsh、tool-fs、tool-fs-search、tool-workspace-dependencies、bash-sandbox、pwsh-sandbox |
| `npx tsc --noEmit` | 0 |
| `npm run build` | 0 |
| `cargo test --offline --lib profile_rules::tests` | 16/16 |
| `verify_profile_scope_106.py` | 43/43 |
| `verify_profile_assets_109.py` | 41/41 |
| `npm run tauri:build -- --bundles app` | rc=0；425.52 MiB；`du -sh` 505 MB |
| 重建后 `open -n OmniDesk.app` | app pid 44208、engine pid 44263；设置服务打印本地地址；日志无预设组合报错 |

**未验证：** 真实模型下的 Excel 导出与文案写作；既有 open session 的 composition；xlsx 依赖 openpyxl，runtime 与系统 python 均未提供，需 LibreOffice 转换或模型自带实现。

**备份：** `~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.patch.yml.bak-addback-20261113`。
