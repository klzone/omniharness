# GitHub 历史重写与发布清理审查（2026-11-17）——通过（附外部副本限制）

# OD-ARCH-173 0.2.30 发布验收（2026-11-17）——通过（附未验证项）

**审查对象：** PLANS.md OD-ARCH-173 全部阶段执行记录与证据（tests/verification/desktop/evidence/od-arch-173/）、release 提交 1d07d91bba、远程 main ac46822eff、实际源码 diff。

**逐阶段复核（源码 diff + 测试输出，非自述）：**

1. **阶段 0 基线：** diagnose-omnidesk.py 新增只读会话解析；12 个真实历史会话确认 system 5396 字符中 web-search 段占 1919、Skill 目录 2518；397 秒拆分缺原始会话，如实标未验证。
2. **阶段 1 提示词：** web-search-free 从 discipline patch 移除，persona 换台账助手底座；mock 复测 web-search 段归零、去 PTC 口径 5396→2983（−44.7%）；PTC 与预算冲突经裁决保留 PTC（D-105 记录取舍）。
3. **阶段 2 Skill：** 先用 mock 探针验证 tool-skill 全文可达（原生 + PTC run_code 两路均验证），才改 conversationPrompt；四动作 user 段降 61–72%；3 个旧断言脚本随阶段 6 更新后通过。
4. **阶段 3 工具面：** dsh-free-search 依赖从 profile.json 与 profile_assets.rs 清除；输出上限显式钉住（read 2000 行/50KB、grep 250 条/单行 500 字节、glob 100）；钩子/审批/abort 审计完成，缺口记入 tool-matrix.md。
5. **阶段 4 预检：** save() 前置档位校验（6 单测）；401/402/403 阻止保存、404 提示放行；切换路径模型目录校验显式档位；聊天期才暴露的 402/403 如实记录为缺口。
6. **阶段 5 诊断页：** diagnostics_metadata 只读命令 + 设置页「配置解释」页；7 新单测；真实窗口截图因锁屏未成，黑图证据保留。
7. **阶段 6 发布：** tsc/build/cargo check/tauri build --no-bundle 全过；cargo test 1200 全过；54 个验证脚本 41 过、13 失败均复现于 HEAD 之前快照或属环境限制；版本 0.2.30 三处同步；CHANGELOG 收口；提交 1d07d91bba 已推送 origin/main。

**边界核对：** runtime/ 未改（全程）；凭证未读取/未打印；无 force push、无标签；未使用真实业务材料。

**未验证项（不阻塞验收，需后续确认）：**

- 真实模型 smoke test（Excel/周报/知识整理）：账户 402 余额不足，无法执行；提示词结构变化已由 mock 验证，真实模型是否先读 Skill 再输出待额度恢复后实测。
- Windows 安装包构建：本机无环境。
- verify_skill_flow_injection_108.py 真实窗口运行：macOS 无 Xvfb。
- 配置解释页真实窗口目视：锁屏未成，由用户日常使用确认。
- 已有安装 Profile 不会自动移除 dsh-free-search：需在 Profile 资产页手动「恢复出厂内容」或重装。

**结论：** 通过。0.2.30 代码、版本号、变更日志已提交推送（1d07d91bba → origin/main ac46822eff）；macOS 侧 tauri build 产物已验证，可发布安装包。

**审查对象：** 重写后的 `main`/标签、当前树、远端 `main`、29 个旧 Release 与仓库外备份。

**核对结果：**

1. 重写前 bundle 完整，`git bundle verify` 通过，约 480 MB。
2. `git filter-repo` 完成重写；个人工作区、设计参考、测试证据、历史报告与旧计划在全部历史路径中命中 0。
3. 当前树个人路径/旧 Profile 名扫描命中 0（许可证署名除外）；工作树中的本次清理改动已提交。
4. 远端 `main` 为 `bf7c7fd836`，全部标签已强推；旧 Release 29 个已删除，GitHub Releases 当前为 0。
5. Rust 定向测试 6/6、91/91 通过；本次未跑完整构建。

**限制：**

- 所有提交哈希已改变，已有 clone 必须重新 clone。
- 旧标签仍存在但指向重写后的提交；旧 Release 下载已不可用。
- GitHub fork 或平台缓存可能仍保留旧对象，非本仓库能强制清除。
- `.git` 仍约 513 MB，主要是上游历史与大文件；当前树已不含个人资料。

**结论：** 通过。0.2.30 需从清理后的源码重新构建安装包并发布。

# 证据与历史报告全面归档审查（2026-11-17）——通过（当前树；历史待重写）

**审查对象：** `tests/verification/**/evidence/`、`tests/verification/desktop/od-*/`、`tests/reports/`、`docs/archive/`、`docs/plans/`、`audit/`、`.gitignore` 与归档说明。

**核对结果：**

1. 移动 37 个目录、1065 个文件，归档到 `MyWorkStation-archive/2026-11-17/test-and-process-evidence`，体积约 11 MB，并生成 SHA-256 清单。
2. `tests/verification` 下已无 `evidence` 目录与 `od-*` 目录；测试脚本与夹具保留，运行时会本地重新生成证据。
3. `tests/reports`、`docs/archive`、`docs/plans`、`audit` 已移出；历史报告链接改为归档说明。
4. `.gitignore` 改为忽略整个 evidence 与 `od-*` 目录，避免后续截图、日志和任务证据再次入库。
5. 全树扫描已无个人路径、旧 Profile 名或旧用户名（许可证署名除外）。Git 历史中的旧个人文档和证据仍在，需单独重写。

**结论：** 当前树通过。归档位置 `MyWorkStation-archive/2026-11-17/test-and-process-evidence`；历史清理另行执行。

# 公开仓库个人资料清理审查（2026-11-17）——通过（当前树；历史待重写）

**审查对象：** 个人工作区、个人数据根配置、个人初始化脚本、个人写作资产、开发 Profile 目录、`data-template/`、`docs/ARCHITECTURE.md`、`scripts/setup.sh`、`scripts/omnidesk-dev.sh`、`scripts/omnidesk-isolation-check.sh`。

**核对结果：**

1. 15 个个人资料文件与 7 个 `_design` 设计参考文件已移出工作树，归档到 `MyWorkStation-archive/2026-11-17/personal-files`，清单记录 SHA-256、大小与相对路径。
2. 个人发言稿所在 `workstations/` 已整体移出；个人写作风格库已替换为 5 个不含个人内容的通用模板。
3. 个人数据根配置与初始化脚本已移出；`data-template/` 不再引用个人配置，`scripts/setup.sh` 不再调用个人初始化脚本。
4. `profiles/omnidesk-dev/` 已改为 `profiles/omnidesk-dev/`，包名与开发启动默认值、隔离自检断言同步更新。
5. 未改产品业务逻辑；`bash -n` 通过；改名后的 Rust 定向测试 6/6、91/91 通过。GitHub 历史仍包含旧个人文件，待单独重写。

**限制：** 当前只清理工作树；Git 历史中的个人发言稿、旧写作风格和截图仍可被检出。要满足“GitHub 历史也干净”，必须重写历史并强制推送，这会改变所有提交哈希并影响已有 clone，尚未执行。

**结论：** 当前树通过。归档位置 `MyWorkStation-archive/2026-11-17/personal-files`；历史清理另行执行。

# 测试截图归档审查（2026-11-17）——通过（同日由 D-103 扩展）

**审查对象：** `tests/verification/desktop/evidence` 与 `tests/verification/desktop/od-*` 下的截图、`.gitignore`、`AGENTS.md`、`CHANGELOG.md`、`docs/decisions.md`、`STATUS.md` 与归档说明。

**核对结果：**

1. 移动 1087 张截图与任务截图，归档目录共 1088 个文件（含 1087 行 `MANIFEST.sha256`），归档体积约 152 MB；工作区 evidence 目录中图片数量为 0。
2. `tests/verification` 从约 165 MB 降到约 13 MB；`tests/verification/desktop/evidence` 保留 603 个已跟踪文字证据文件，本地另有 56 个被忽略的日志/证据文件。
3. `.gitignore` 对 evidence 与 `od-*` 目录的图片、视频扩展名生效；`git check-ignore --no-index` 抽样两条路径均命中。
4. `AGENTS.md` 的证据规则改为「文字证据与测试脚本入库，截图和视频默认仓库外归档」，与 D-101、STATUS 和归档说明一致。
5. 未改产品代码，未运行构建；`git diff --check` 通过。工作区有 1087 个截图删除，未提交未推送。

**限制：**

- `.git` 仍约 542 MB；这次只移出当前树，历史中的截图、视频和重复 schema 未清理。要缩小 clone 体积必须重写历史并强制推送，未执行。
- 历史 `STATUS.md`、`REVIEW.md`、`PLANS.md` 以及部分 dispatch 文档仍引用已归档的 PNG，链接会失效；当前决策记录与用户测试清单已改为引用文字证据，归档说明已登记。

**结论：** 当时通过。归档位置为 `MyWorkStation-archive/2026-11-17/test-evidence-screenshots`；同日 D-103 将文字证据与历史报告也移出 Git。

# 授权变更审查（2026-11-16）——通过（文件级）

**审查对象：** `LICENSE`、`COMMERCIAL-LICENSE.md`、`THIRD-PARTY-NOTICES.md`、`README.md`、`desktop/package.json`、`CHANGELOG.md`、`docs/decisions.md`。

**核对结果：**

1. `LICENSE` 的主体是 SPDX 官方 PolyForm Noncommercial 1.0.0 文本，未改动条款；顶部加入 `Copyright (c) 2026 Yvan` 与 `Required Notice`。
2. `THIRD-PARTY-NOTICES.md` 明确列出 DeepSeek Harness 的 MIT 许可证、版权、锁定 tag 与提交；`runtime/` 未被重新授权。
3. `README.md` 写清自有代码与 runtime 的分层授权、商业授权入口，以及 0.2.30 的生效边界和 0.2.29 及更早版本的 Apache-2.0 历史授权。
4. `desktop/package.json` 增加 SPDX 标识 `PolyForm-Noncommercial-1.0.0`，JSON 解析通过。
5. `COMMERCIAL-LICENSE.md` 只描述申请与授权范围，明确商业授权不自动授予，具体条款以书面协议为准。

**验证：** 许可证文本与 SPDX 官方文本逐字比对通过；`git diff --check` 通过；本次 diff 无 `runtime/` 路径。未改代码，未运行构建。

**结论：** 文件级授权变更通过。未提交、未推送。

**未验证：** GitHub 的许可证识别结果、安装包内的第三方许可证清单是否完整、商业授权合同的实际法律条款。

> 更早的审查记录已归档到仓库外 `MyWorkStation-archive/2026-11-17/process-docs/`。

# 诊断：volcengine / deepseek-v4.1-flash 切换报错（2026-11-17，诊断级）

**背景：** 用户测试切换火山引擎模型：模型可正常对话，但显式思考档位切换被阻（错误一）；模型发现探测对 `ark.cn-beijing.volces.com/api/plan/v3/models` 返回 404（错误二）。

**核对结果（文件事实 / 静态检查，未跑引擎）：**

1. 错误一是 `desktop/src/api.ts` `setModelRoute` 的切换前预检（约 1930–1960 行）：仅当请求档位非 `default` 且目录中该模型存在、`efforts` 为空时阻断。目录来自 DSH `session/modelCatalog`（`engine_bridge.rs` `list_models` → `omni.list_models`），与 Profile 路由声明的 `reasoningEfforts` 同口径（`model_settings.rs` `declared_efforts`）。
2. `profiles/omnidesk-dev/cordis.patch.yml` 中只有 **opendesign** 的 deepseek-v4.1-flash 声明了 7 档 `reasoningEfforts`；volcengine 未声明该字段，内置目录也无 → 切到 volcengine 时该模型目录 `efforts` 为空，显式档位被预检拒绝。这是设计内保护（显式档位会在首次调用被 DSH 拒绝），不是路由故障；「跟随模型默认」可正常切换，与"模型能正常对话"观察一致。
3. 错误二来自 DSH `llm/discoverModels` 探测（`dsh_settings.rs` 注释指向 `packages/llm/llm-pi-ai/src/discovery.ts`，runtime/ 内不改）。消息本身已声明"本次请求的结果；没有保存任何设置"，不阻断手填模型 id 保存。
4. 模型自述只知道 DeepSeek 驱动、不知道思考等级：属模型自述局限与档位未声明的事实叠加，非路由证据；不据此判定切换失败。
5. `scripts/diagnose-omnidesk.py` 实测：当前默认路由 opendesign/deepseek-v4.1-flash；volcengine 字符串仅出现在 runtime/ 的 dsh-cost-meter、dsh-free-search 模块中。

**未验证：**

- 错误二 URL 中 `/api/plan/v3` 段的来源：标准 Ark OpenAI 兼容 base 为 `/api/v3`；该路径疑似与 dsh-cost-meter 的 plan 计费通道相关，未读该模块源码（诊断预算用尽），列为下一步。
- volcengine 提供商配置的实际存放位置（app config v2 中只有 opendesign），以及该端点真实档位取值。

**结论：** 两条提示均非路由故障。错误一按提示选「跟随模型默认」即可切换；若要求 volcengine 也支持档位，需实测端点后在其 Profile 条目声明 `reasoningEfforts`（实施项，须计划批准）。错误二不阻断保存，手填模型 id 即可；建议复核 volcengine base_url 是否混入 `/api/plan/v3` 计费路径。

# OD-ARCH-175 阶段 1–3 复核（2026-11-17，代码级通过）

**审查对象：** 子代理 Plato（阶段 1 只读核查）、Aristotle（阶段 3 前端）、Fermat（阶段 2 Rust）的实际改动：`desktop/src/api.ts`、`desktop/src/types.ts`、`desktop/src/shell/ModelPicker.tsx`、`desktop/src/shell/HomeChat.tsx`、`desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/model_routes.rs`、`desktop/src-tauri/src/model_effort_fallback.rs`（新增）及验证脚本。

**ARCHITECT 独立复核结果：**

1. **阶段 1（执行主体修正已落 STATUS.md）：** 错误形态结论基于锁定安装产物（DSH 0.2.1-rc.5 / pi-ai 1.4.0）源码行，触发条件（note 稳定文本 + provider/model/effort 三元精确匹配）被阶段 2 单测逐条覆盖。新事实：未声明模型显式 `off` 可用（等价关闭思考下发），已纳入设计。
2. **阶段 3：** `planModelRouteEffort` 用「归一化目录 + Profile 原始声明」两层证据正确区分未声明（照传档位，运行期回退兜底）与声明为空/不匹配（本地直接 default + 一句话告知）。次要备注：`listModelRoutes()` 用缓存，Profile 刚改声明时证据可能滞后一拍，后果只是多走一次运行期回退，无害。
3. **阶段 2：** 解析器只认完整稳定句式，配置空档位错误不误触发；guard 四态（InFlight/Applied/Failed/未占位）防重复改写与提前告知；改写覆盖文件同步完成，stop_engine 在后台线程持 lifecycle 守卫执行，轮询路径不阻塞。已知限制：回退只尝试默认 Profile（切换路径是多候选），跨 Profile 路由时安全降级为不触发。
4. **ARCHITECT 实跑验证：** `cargo test --lib` 1209 passed / 0 failed（另一次运行出现 3 个 engine spawn 类环境性 flaky，两次复跑不复现，非本改动回归）；`npx tsc --noEmit` 通过；`npm run build` 通过；`verify_model_route_precheck_175.py` 12/12 + static 11/11；`verify_home_refine_202.py` 39/40（唯一失败为既有 `home_scene_backdrop`，与未改动的 App.tsx 相关）；`git diff --check` 通过。

**结论：** 阶段 1–3 代码级通过。**阶段 4 未验证**：真实 volcengine 切档 → DSH 拒绝 → 宿主自动回退 → 引擎重启 → 前端 notice 的端到端链路需重编译桌面应用并实际切换，属运行证据，留待用户当轮执行后补录。未提交、未推送。

# 诊断：工作记录场景周期复盘失败（2026-11-17，诊断级）

**现象：** 用户在工作记录场景依次提问：①「上周工作复盘（11-9至11-15）」→ 判为复盘请求不落盘，引导走周期复盘/周报流程；②「周复盘（11-9至11-15）」→ 模型口头确认起草，但称需工作台汇集材料、不自取，之后无下文；③「你自己去搜索app暂存的…记录」→ 被原文落盘为 2026-11-09 随手记并进入记录分析。

**核对结果（文件事实/静态检查）：**

1. 周期复盘的宿主材料汇集（`worklogPeriodReviewCollect`，OD-WR-505：正式记录+任务统计+既往复盘标题）只在 `HomeChat.tsx:2961-2988` 的 `intent === "period-review"` 分支触发；而 `HomeChat.tsx:2723-2724` 规定 `weeklyScene = preset?.key === "weekly"`，`intent = weeklyScene ? detectWorkIntent(text) : null` —— **工作记录场景 intent 恒为 null，汇集链路在该场景永远不触发**。
2. `skills/period-review/SKILL.md:9` 明文契约：「工作台已提供本周期正式记录、任务统计与既往复盘标题；不要自己找文件」。模型在第 ②问的「我不会自行找文件取数、等材料送达」正是执行该契约；但该场景宿主不发材料 → 死等，无初稿。
3. 第 ①问走随手记分析提示词（`HomeChat.tsx:2090-2125`，只判冲突/缺要素/可执行任务），判「复盘请求不含可记录事实」不落盘；第 ③问被判为可记录事实原样落盘（日期取文中 11-09），随后的「分析」同样是记录质量审查，无本地检索能力或授权。
4. 引擎侧无异常：diagnose 脚本轮询正常，无 error 关键词日志；任务快照无新增失败。本问题与 OD-ARCH-175 模型路由改造无关。

**结论：** 不是模型故障，是**场景门控设计缺口**——周期复盘流程只接进了周报场景，工作记录场景没有接线；period-review skill 的「等材料不自取」契约在该场景成为死路。三条用户路径分别落入「引导离开」「死等材料」「误当随手记」。

**修复方向（待用户裁决后立计划）：** A. 把 intent 分流扩展到工作记录场景（需设计该场景落盘/展示交互）；B. 工作记录场景检测到 period-review 时内联触发材料汇集（只读汇集，材料入本轮提示词 + skill 指针）；C. 最小改：该场景识别到复盘请求时确定性引导切到周报场景，消除死等。

# OD-ARCH-176 阶段 1–2 复核（2026-11-17，代码级通过）

**背景：** 用户批准按自然语言理解重构工作记录场景（意图模型判定、指令不落盘、复盘场景内出稿、默认 off 直出 + 手动档位覆盖），子代理 Kierkegaard（阶段 1 全栈）、Aristotle（阶段 2 补充）执行，均为 deepseek-v4.1-flash-eco / high。

**审查对象与结论：**

1. **意图契约（`workIntentDecision.ts` 新增）：** `<omnidesk-intent>` 统一 JSON 契约（note/instruction/chat/period-review/report/daily-summary/uncertain），解析失败或枚举外按 uncertain 走提问卡，不猜不落盘；`decideWorkIntentRoute` 明确只有 note 写盘。复核通过。
2. **路由落地（HomeChat）：** 陈述句快路径（OD-WR-105 `isPlainStatementNote`）在工作记录场景退出；note 落盘 + 质量分析并一次调用；instruction/chat 注入周记录索引回复；period-review 内联复用 `worklogPeriodReviewCollect`（OD-WR-505）；周报场景关键词通道未动。复核通过。
3. **off 档位管线：** 桌面提交链新增 `reasoning_effort/provider/model` 参数 → engine RPC → `web_api_adapter._apply_reasoning_effort` 在 session/prompt 前按会话 `select_model`（带 per-session memo，缺 provider/model 不装档，失败如实报错）。已核实 DSH `SessionPromptRequest` 无按次 effort 字段，会话级通道是正确选择。复核通过。
4. **off 能力预判（阶段 2 补充）：** `pickWorkIntentEffort` 增加两层证据判定——声明集合明确不含 off → 传 null 跟随默认；未声明/目录不可用/声明 false → 传 off。堵住了 OD-ARCH-175 阶段 1 证实的「声明集合不含 off 会被运行期拒绝」死角。复核通过。
5. **技能文本：** `skills/period-review/SKILL.md` 改为「材料随请求送达直接起草，材料为空如实说明」，硬约束（引用编号/案件禁入/确认后不可覆盖）未动。注意：**安装副本仍是旧文本**（sync-skills.sh 受 macOS realpath 限制未实际同步），生效需「恢复默认规则包」或修复同步脚本——记入阶段 4 迁移项。

**ARCHITECT 实跑验证：** `cargo test --lib` 1209/1209（另一次运行 3 个 engine spawn 类 flaky 不复现，与前两日一致）；`tsc`、`npm run build`、`git diff --check` 通过；`verify_arch_176.py` 70/70、conversationPrompt 33/33、skill bindings 69/69、089 86/86、103 52/52、104 73/73、503 通过、505 71/71、202 39/40（既有失败）。OD-ARCH-175 改动与单测无回归。

**阶段 4 待验证（运行证据，需用户执行）：** ①真实模型下工作记录场景三类输入（事实/复盘请求/指令）的端到端行为；②DSH `selectModel` 持久化「最近一次选择」的影响面——若跨会话泄漏，off 可能影响后续非工作场景任务（起草轮不传 effort、依赖默认档，理论上不受影响，需实测确认）；③声明不含 off 模型的实际运行表现；④安装副本技能文本迁移。未提交、未推送。

# OD-ARCH-177 复核与发布验收（2026-11-17，通过）

**审查对象：** ARCHITECT 亲自实施（用户批准执行并授权提交推送发版）：`companionPlugins.ts`（新增）、`SettingsPanel.tsx` 配套插件区块、`skills/anysearch/SKILL.md` 环境适配、`verify_companion_plugins_177.py`（新增）、版本 0.2.32 与 CHANGELOG。

**复核结果：**

1. 清单为纯静态元数据（名称/上游地址/建议版本/用途），无自动安装、无联网版本检查、无插件实体入库，符合用户裁决（插件升级过期不由仓库承担）。
2. 技能文本只加环境预期与回退指引，红线约束（不装不改上游、不伪造结果、敏感信息禁检索）逐字保留。
3. ARCHITECT 实跑：verify_companion_plugins_177 11/11；tsc、npm run build 通过；verify_arch_176 70/70、175 12/12+11/11 回归通过；cargo test --lib 单线程 1209/0（并发 3 个 spawn 类环境性失败历次复跑均不复现，记为既有 flaky）。
4. 初版两处脚本瑕疵（ROOT 层级、拼接字符串断言）在提交前修正，未入库错误版本。

**发布验收：** 版本 0.2.31 → 0.2.32（package.json + tauri.conf.json），CHANGELOG 0.2.32 条目含 Added/Changed/Verification/升级说明；提交推送后 release.yml 自动构建。阶段 4（真实模型端到端、selectModel 持久化影响面）留待 0.2.32 Windows 实测后补录。

# 线上回归：工作记录场景 engine_unavailable（2026-11-17，ARCHITECT 责任事故，已修复于 0.2.33）

**现象：** 0.2.32 用户实测工作记录场景每条消息报 `engine_unavailable`，未分类 / 文案写作场景正常；与思考档位、提供商无关。

**根因（ARCHITECT 的实现判断错误）：** OD-ARCH-176 阶段 1 依据 prompt 路径行为推断「未声明模型传 off 必被接受」——该结论只对 session/prompt 成立。真实 DSH 的 **session/selectModel** 走 `resolveCallConfig` 校验，对 reasoning=false（未声明档位）的模型拒绝任何显式 effort（含 off），RemoteError 为 `session/model-unavailable`，被适配层 `_map_rpc_failure` 兜底映射成 ENGINE_UNAVAILABLE。工作记录场景每轮分类都带 off+provider/model，故必挂；其他场景不带这些参数，故正常。阶段 2 的两层证据预判只覆盖「声明集合不含 off」，未覆盖「未声明」这一更普遍情形；单元测试 mock 了 select_model、harness 场景不带档位参数，真实 DSH 行为从未实测——**教训：凡是新增引擎 RPC 调用，必须有真实引擎回归，mock 全绿不等于可用。**

**修复（0.2.33）：**
1. 前端 `workIntentDecision.ts`：新增 `effortEvidenceIncludes`，档位必须被证据「证明支持」才下发；未声明 / 目录不可用一律不传（未声明模型无思考可关，等效直出）；用户手动档位同样需要证据支持。
2. 适配器 `_apply_reasoning_effort`：select_model 失败捕获 EngineError、记 stderr、按模型默认继续，不再炸整轮任务。
3. `_map_rpc_failure`：`session/model-unavailable` 改判 INVALID_ARGUMENT，呈现真实原因。

**验证：** 真实 DSH 回归 `verify_effort_select_degrade.py` 4/4（selectModel 拒绝 → stderr 降级 → 任务按默认档成功）；verify_arch_176 71/71（用例按新语义更新）；cargo 1209/0 单线程；tsc / build 通过。ARCHITECT 对本事故负全责：阶段 1 的「off 必被接受」结论与阶段 4 未实测即发版，是两个叠加的管理失误。

# 诊断：工作记录不落盘/不显示 + anysearch 技能缺失（2026-11-17，诊断级，预算 8 轮用满）

**问题一：工作记录对话称已写入，工作记录分页无显示。**

核对结果（文件事实/脚本运行）：
1. `/Users/yvan/OmniDesk/app/engine-state` 最后一份任务快照停在 11-16 01:01（`worklog_extract`，state=running 已停滞）；**11-17 全天零新增任务快照**；数据根当天仅 `omnidesk-model.yml` 变更（上午切火山引擎与种子备份）。
2. 今天的 DSH 会话目录全部来自子代理冒烟测试工作区（`od-real-smoke*`、`od174-real*`），没有用户本人工作区的会话；但用户拿到了真实模型回复（含 anysearch 轮的真实联网检索）。
3. 代码侧新意图路由链路正确：契约解析失败会显式提示「没有读出意图判定」，仅 `note` 走 `worklogAppend`。`working_root` 配置为 null，实际库根走默认推导，本轮未定位到（预算用尽）。

**阶段结论：** 用户测试的对话实例与仓库侧数据根**不是同一份**（疑似另有一个 app 实例/数据根在别处），仓库侧无法直接看到那次对话的任务与落盘文件；需要用户确认测试所用实例（开发版 dev 数据根即 /Users/yvan/OmniDesk/app，发布版在别处）才能定位断点。次要嫌疑（代码确证）：记录日期取文内日期——此前已出现把指令句记到文中提及的 2026-11-09 的实例；若消息带日期词，记录会落到别的日期分组，按日期流水看今天则「看不到」。

**问题二：anysearch 技能缺失（结论明确）。**

`skills/anysearch/SKILL.md` 自述为薄包装：实体 CLI 位于 `$HOME/.codex/skills/anysearch/scripts/anysearch_cli.py`，仓库不复制脚本（文件事实）。OmniDesk 引擎环境没有 Codex 技能目录与 `ANYSEARCH_SKILL_DIR` 变量 → 模型按规范如实报缺并回退 web_fetch 直连（行为正确，非故障）。Google News 被拦截、维基 DNS 异常、新华网 403 属运行环境网络边界，与技能无关。

**修复方向（待用户裁决后立计划）：** A. 把 CLI 脚本打包进 `skills/anysearch/scripts/`（需确认依赖与凭证边界）；B. 改写技能文本，明确引擎环境无本机 CLI 时直接走 web_fetch，避免浪费探索轮次；C. 引擎环境配置 `ANYSEARCH_SKILL_DIR` 指向真实存在的用户级技能目录。推荐 B+C。

**2026-11-17 补充（用户 Windows 实测回执，问题一收口）：**

1. **问题一非故障收口：** 用户重启软件后测试成功——记录已落 11 月 17 日并可见；库根为 `~/omnidesk/work/工作记录/`。此前"看不到"的原因叠加：测试实例与开发数据根不同源（Windows 发布版 0.2.31，不含 OD-ARCH-175/176 未提交改动）+ 需重启后新开对话。
2. **场景边界（用户确证的行为）：** 必须新开对话且分类选「工作记录」才能落盘；「未分类」对话不落盘（无 preset 的对话不走工作记录写入路径，属现行设计）。如需"未分类也能记"属新需求，须另立计划。
3. **OD-ARCH-175/176 阶段 4 仍未实测：** 用户 Windows 上是旧版本；新功能验证须提交推送并发版后进行（提交推送待用户授权）。
4. **OD-ARCH-177 更新：** 常备插件清单按用户裁决定为 anysearch + dsh-cost-meter 两项。
