# 阶段一第一批只读任务卡

- 目标：把当前版本、来源、Profile、插件、模型路由、历史证据和 Pi 对象边界固定下来
- 执行者：DeepSeek V4.1 Flash
- 写入范围：仅 `docs/research/dsh-runtime-audit/evidence/phase1/`
- 全局禁止：修改产品源码、`runtime/`、Profile、配置、数据库；安装依赖；初始化子模块；解包到仓库；读取凭证或会话正文；调用模型；提交/推送

## 执行后修订（2026-11-12）

首批任务已发现一个只读边界缺口：对真实 `~/OmniDesk` 数据根运行 `--dump-config` 会刷新生成文件 `profiles/discipline/cordis.yml` 的 mtime。原 P1-03/P1-04 卡片允许该命令，是任务卡设计错误。后续不得再在真实数据根执行该命令；补充约束见 [03-phase1-supplement.md](03-phase1-supplement.md)。

## 卡片总表

| Step ID | 单一动作 | 输出 |
| --- | --- | --- |
| DSH-P1-01 | 建立本机 Runtime 身份与版本矩阵 | `01-runtime-identity.json` |
| DSH-P1-02 | 建立源码与运行产物来源图 | `02-source-provenance.json` |
| DSH-P1-03 | 采集 OmniDesk discipline Profile 的生效装配摘要 | `03-profile-plugins.json` |
| DSH-P1-04 | 建立模型路由与参数来源矩阵 | `04-model-routing.json` |
| DSH-P1-05 | 建立历史文档与测试证据适用性索引 | `05-evidence-applicability.md` |
| DSH-P1-06 | 固定 Pi Agent Runtime 研究对象边界 | `06-pi-boundary.json` |

## DSH-P1-01：建立 Runtime 身份与版本矩阵

- **单一动作**：只采集并登记本机四套 Runtime 身份和精确版本。
- **输入来源**：`runtime/RUNTIME.lock`、`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/package.json`、`/Applications/OmniDesk.app/Contents/Resources/runtime/RUNTIME.lock`、`/Applications/DeepSeek Harness.app/Contents/Info.plist`、当前进程表、`~/OmniDesk/app/config/settings.v1.json` 的引擎 Profile 字段。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase1/01-runtime-identity.json`。
- **涉及文件**：只读上述文件；只写输出 JSON。
- **前置依赖**：无。
- **执行范围**：版本、路径、进程参数、Profile 名、安装包版本；不推断机制行为。
- **明确禁止**：不读取 `~/.dsh/.credentials.yaml`、`~/.dsh/sessions/`、任何 `.env`；不启动新进程；不改 `RUNTIME.lock` 或 app 包。
- **验收命令**：`python3 -m json.tool docs/research/dsh-runtime-audit/evidence/phase1/01-runtime-identity.json >/dev/null`；对每个身份核对 `ls`/`cat`/`ps` 的对应命令。
- **可自动判断的验收标准**：JSON 可解析；`identities` 至少包含 `repo_cli`、`installed_omnidesk`、`running_dsh_desktop`、`omnidesk_engine_selection`；版本值分别为 `0.2.1-rc.5`、`0.2.22`、`0.2.2-rc.1`、`0.2.1-rc.5`；每个身份都有 `path`、`evidence_level`、`command`；`boundaries.product_modified=false`。
- **必须提交的证据**：JSON、每次命令的退出码、进程 PID 与关键参数摘要、未验证项。

## DSH-P1-02：建立源码与运行产物来源图

- **单一动作**：只核对 rc.5 锁定 commit、gitlink、子模块检出状态和两个运行产物的源码可访问性。
- **输入来源**：`RUNTIME.lock`、`.gitmodules`、`git ls-tree HEAD runtime/deepseek-harness`、`git submodule status`、远程标签查询、`runtime/dsh-runtime/node_modules`、`app.asar` 的版本字符串。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase1/02-source-provenance.json`。
- **涉及文件**：只读 Git 元数据和已安装包；只写输出 JSON。
- **前置依赖**：DSH-P1-01 完成。
- **执行范围**：来源可达性、版本归属、可用/不可用布尔值；不下载源码。
- **明确禁止**：不执行 `git submodule update --init`、`git clone`、`npm install`、`asar extract`；不把 app.asar 或完整依赖树复制到仓库。
- **验收命令**：`python3 -m json.tool ...`；单独复核 `git ls-tree`、`git submodule status`、`git ls-remote --tags`、`strings app.asar` 的关键匹配。
- **可自动判断的验收标准**：`locked_commit` 为 `b830ec950faba0a46dda564be3e2d854afed94f1`；`gitlink_commit` 为 `d743267388641bc76f17c45ce8b4c231aed1d32c`；`source_checkout_present=false`；`remote_tag_points_to_lock=true`；`rc5_npm_lib_js_present=true`；`desktop_asar_version=0.2.2-rc.1`；每项有命令和证据等级。
- **必须提交的证据**：JSON、四方 SHA/版本、目录是否为空、可用与不可用原因、阶段二源码门禁建议。

## DSH-P1-03：采集 OmniDesk discipline Profile 的生效装配摘要

- **单一动作**：只从已存在的 Profile 与 `--dump-config` 结果提取生效 bundle、补丁、核心插件和工具面摘要。
- **输入来源**：`~/OmniDesk/runtime/dsh-home/profiles/discipline/{package.json,cordis.patch.yml}`、`~/OmniDesk/app/config/omnidesk-model.yml`、rc.5 CLI `--dump-config`、`desktop/src-tauri/src/engine_supervisor.rs` 的默认 Profile 解析。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase1/03-profile-plugins.json`。
- **涉及文件**：只读上述文件和临时 dump；只写输出 JSON；临时 dump 放 `/tmp` 并在完成后删除。
- **前置依赖**：DSH-P1-01 完成。
- **执行范围**：Profile 选择、bundle 顺序、插入插件、补丁警告、核心插件存在性/disabled 状态、agent preset 列表；不推导实际执行行为。
- **明确禁止**：不改 Profile；不启动 web 服务；不读取凭据；不把完整 dump 放进 Git；不把“配置中存在”写成“模型实际收到”。
- **验收命令**：`python3 -m json.tool ...`；复核 dump 命令退出码；复核顶层 ID 计数、bundle 和关键插件名。
- **可自动判断的验收标准**：`profile=discipline`；`bundles` 精确为 `@deepseek-ai/dsh-base` 与 `@deepseek-ai/dsh-web-app`；`inserted_plugins` 包含 `skill-office`、`web-search-free`、`cost-meter`；`top_level_id_count=194`（若实际不同，必须记录原因）；`patch_warning` 包含 `entry "acp" not found`；`tool_availability_is_runtime_verified=false`；`full_dump_committed=false`。
- **必须提交的证据**：JSON、dump 命令与退出码、配置摘要、警告原文、未验证项。

## DSH-P1-04：建立模型路由与参数来源矩阵

- **单一动作**：只登记两套运行环境的 Provider、模型、reasoning、上下文/输出上限及来源优先级。
- **输入来源**：`~/OmniDesk/app/config/omnidesk-model.yml`、`~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.patch.yml`、`~/.dsh/profiles/desktop/{package.json,cordis.patch.yml}`、rc.5 `dsh-llm-pi-ai` 配置类型/声明、desktop 0.2.2 的配置字符串。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase1/04-model-routing.json`。
- **涉及文件**：只读配置与包元数据；只写输出 JSON。
- **前置依赖**：DSH-P1-01、DSH-P1-03 完成。
- **执行范围**：静态配置与实际 `--dump-config` 结果；不发起模型请求。
- **明确禁止**：不读取密钥值；不改路由；不启动真实模型；不把 `apiKeyEnv` 变量名当成已配置凭证。
- **验收命令**：`python3 -m json.tool ...`；复核覆盖文件与 dump 中 `agent-default-model`、`llm-pi-ai` 的关键值。
- **可自动判断的验收标准**：OmniDesk 默认 provider/model 为 `opendesign / deepseek-v4.1-flash-eco`；DSH Desktop 默认 provider/model 同为该组合；OmniDesk 路由模型数为 4；`acp` 覆盖未命中；web-api 默认 reasoning 标为 `unknown` 或给出实际证据；`credentials_values_captured=false`。
- **必须提交的证据**：JSON、来源顺序、未命中项、两套环境的差异、未验证项。

## DSH-P1-05：建立历史文档与测试证据适用性索引

- **单一动作**：只给现有 DSH/接口相关材料建立版本与适用性索引。
- **输入来源**：`docs/DSH-Architecture-Analysis.md`、`docs/OmniDesk-DSH-Isolation-Audit.md`、`docs/OmniDesk-DSH-Isolation-Implementation.md`、`docs/OmniDesk-Engine-Interface-Audit.md`、`docs/OmniDesk-Engine-Interface-Contract.md`、`docs/OmniDesk-Engine-Interface-Test-Matrix.md`、`docs/OmniDesk-Session-Import-Compatibility.md`、`docs/decisions.md` 的 D-002/D-012/D-016/D-024/D-025、`tests/reports/p1-*`、`p2-*`、`phase1-*`、`v0.1-*`、`audit/dsh-java-review/*` 的元数据。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase1/05-evidence-applicability.md`。
- **涉及文件**：只读上述文档；只写输出 Markdown。
- **前置依赖**：DSH-P1-01 完成。
- **执行范围**：文档元数据、结论摘要、证据等级、适用版本、失效部分、需复跑项。
- **明确禁止**：不把文档自述当实测；不修改历史文档；不删除证据；不把 Java 端口审计算作 DSH Runtime 实现。
- **验收命令**：`rg -n '^\|' <输出>` 统计行数；`rg -n 'alpha\.2|rc\.5|0\.2\.2-rc\.1|U|需复跑' <输出>` 核对版本标记。
- **可自动判断的验收标准**：至少覆盖 10 份材料；每行含 `材料`、`版本/日期`、`可保留结论`、`失效或不适用`、`证据等级`、`下一步`；明确列出 rc.5 升级后 E17/E20 回归；将 `audit/dsh-java-review` 标为“非 DSH 实现证据”。
- **必须提交的证据**：Markdown 索引、版本/日期、引用路径、过期项、复跑命令建议。

## DSH-P1-06：固定 Pi Agent Runtime 研究对象边界

- **单一动作**：只把 Pi 的研究对象、三层边界、本机可用性和待固定版本写成基线。
- **输入来源**：`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm-pi-ai/package.json`、`@earendil-works/pi-ai/package.json`、本机 PATH/全局 npm 清单、官方 `earendil-works/pi` 仓库的只读标签信息、官方 README 的层次说明。
- **输出目标**：`docs/research/dsh-runtime-audit/evidence/phase1/06-pi-boundary.json`。
- **涉及文件**：只读本地包与远程标签；只写输出 JSON。
- **前置依赖**：DSH-P1-01 完成。
- **执行范围**：只定义对象，不安装、不运行、不比较性能。
- **明确禁止**：不安装 `pi` 或任何 npm 包；不调用模型；不把 `dsh-llm-pi-ai` 当作 Pi Agent Runtime；不固定未验证的最新版本。
- **验收命令**：`python3 -m json.tool ...`；复核 `command -v pi` 为空、`@earendil-works/pi-ai` 版本、远程标签查询退出码。
- **可自动判断的验收标准**：明确写出 `pi_ai_is_model_sdk=true`；`pi_agent_core_candidate=true`；`pi_coding_agent_candidate=true`；`local_pi_runtime_present=false`；`dsh_pi_ai_dependency_version=1.4.0`；`exact_pi_runtime_version` 为已核实值或 `unresolved`；`installation_performed=false`；`model_calls_made=false`。
- **必须提交的证据**：JSON、官方仓库 URL、标签/commit 候选、本机不可用证据、阶段三前置条件。

## 统一交付格式

- 每个 JSON 必须包含 `step_id`、`generated_at`、`evidence_levels`、`facts`、`commands`、`unknowns`、`boundaries`。
- 每个任务完成后立即停止，不顺手执行下一张卡。
- 执行者回报必须列出：实际改动文件、命令与退出码、发现的反证、未验证项、Git 状态。
- 研究负责人复核前，不得把任何结果写成阶段一通过。
