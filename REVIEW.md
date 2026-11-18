# GitHub 历史重写与发布清理审查（2026-11-17）——通过（附外部副本限制）

# OD-READ-001 读书阅读笔记分页初步实施审查（2026-11-18）——通过（静态/脚本范围，附未验收项）

**审查对象：** HEAD `b8b9357b9c` 之上的工作树；`desktop/src-tauri/src/reading_store.rs`、`task_shell.rs` 阅读源读取、`scripts/reading_attachment_extract.py` EPUB/纯文本分支、`desktop/src/shell/ReadingPage.tsx`、`desktop/src/readingAi.ts`、`api.ts`/`types.ts`/`lib.rs` 命令与注册、`docs/decisions.md` D-111、数据布局 §5.2。

**结论：** 本切片的存储、锚点校验、导入边界、提示词边界与构建链路**通过**；真实桌面窗口交互与真实模型输出**尚未验收**，不得标为「已验收」或「真实可用」。

**实际核对与发现（均在本次修复）：**

1. **滚动进度曾经会重写整本书 JSON。** `set_progress_at` 最初走 `write_book`，在 5 MB 级正文上每次滚动暂停都全量重写。已改为 `progress/<id>.json` + meta 小文件；`read_book_at` 读取时合并，静止后只写小文件。
2. **React StrictMode 下恢复阅读位置会失效。** 恢复标记原先在 effect 首行写入，StrictMode 首次 cleanup 后第二次 effect 直接跳过，滚动位置不恢复。已改为在 `requestAnimationFrame` 内成功设置滚动后再标记。
3. **段落标记的装饰文字会污染选区偏移。** 书签圆点原先是段落内的 `<span>▪</span>`，`prefix.toString()` 会把圆点计入偏移，导致划线定位差 1 个字符。已改为 CSS `::before`，段落 `textContent` 恢复为纯原文。
4. **快速切书会把旧书响应写进新书。** 已加 `loadSeq` 序号与 `bookIdAtCall` 校验，旧响应在切书后丢弃；切书同时清空草稿/删除确认/AI 结果并递增 AI 序号。
5. **引文歧义与跨段选择。** 偏移不符时仅在引文于段落中唯一出现时重定位，多处出现直接拒绝；跨段选择在界面明确提示暂不支持，不生成错位批注。
6. **AI 只产草稿。** 概念/整理提示词由纯函数组装并单测；整理稿只在 `reading_note_save` 落 `notes/`，不写知识库；模型结果界面标「未复核」。
7. **导入边界。** 只接受用户显式选择的绝对路径普通文件；32 MiB 上限；重复正文按 SHA-256 返回已有书；抽取失败只报原因；阅读目录/文件在 Unix 下 0700/0600。

**证据（实际运行）：** `cargo test --offline --lib -- --test-threads=1` 1225/0/1 ignored；`reading_store` 5/5；`reading_source` 2/2；`npx tsc --noEmit` 0；`npm run build` 0；`node --experimental-strip-types --test src/readingAi.test.mjs` 4/4；EPUB 与 GB18030 helper 手工样例通过；`git diff --check` 0。

**未验收项与风险：** 真实窗口选区浮条位置、点击回跳、窄屏侧栏、真实模型解释/整理质量、32 MiB 级 PDF 耗时、复杂排版 EPUB 的纯文本损失均未实测；跨段划线未实现。后续若要写知识库或接微信读书/GitHub，须按 D-111 复审触发另行立项。

# OD-READ-001 阶段 4 独立复核（2026-11-18，BUILDER 子代理执行）——通过

**复核范围：** BUILDER（deepseek-v4.1-flash-eco / high）交付的阶段 4 diff：`reading_store.rs` 引文核验与 frontmatter、`ReadingPage.tsx` 围栏剥离与核验回执、`readingAi.ts` `stripMarkdownFence`、`types.ts` `ReadingSavedNote` 两字段、`STATUS.md` 执行记录。

**ARCHITECT 实跑（非自述采信）：**

1. `cargo test --offline --lib -- --test-threads=1` → 1232 passed / 0 failed / 1 ignored（50.8s）。新增 7 条核验/frontmatter/提示块测试全部通过，且引文核验确实作用于保存路径（`save_note_at` 内 `audit_organized_citations` 调用与提示块插入位置断言）。
2. `npx tsc --noEmit` → 0；`npm run build` → 0（ReadingPage chunk 35.33 kB）；`node --experimental-strip-types --test src/readingAi.test.mjs` → 5/5；`git diff --check` → 0。
3. 边界核对：`runtime/`、`engine/`、模型路由、凭证、版本号、CHANGELOG 未改；工作树既有改动原样保留；未提交未推送。

**发现与处置：**

1. 任务书允许清单未列 `readingAi.ts`，BUILDER 因测试可导入性把 `stripMarkdownFence` 放进该纯函数模块——判断正确，属任务书遗漏而非越权，计划允许清单在 PLANS.md 阶段 4 补记该文件。
2. 界面「未核验 y 条」按列示条数计（超过 20 条时末条为「……另有 N 条」），保存文件提示块给出真实总数；已在 STATUS.md 如实记录，不构成缺陷。

**结论：** 阶段 4 通过；随后进入提交推送子任务（用户已授权）。

## OD-READ-001 提交推送终检（2026-11-18，ARCHITECT）——通过

**终检依据（实跑核对，非子代理自述）：**

1. `git log -2 --oneline`：HEAD 为 `f4c67db150`（OD-READ-001 提交），父提交 `b8b9357b9c` 与开工基线一致；无 force push、无新标签。
2. `git log origin/main -1` 与 `git status --short --branch`：origin/main 已指向 `f4c67db150`，本地与远程同步；推送后仅剩 STATUS.md 记录小节未提交（见下）。
3. `git show f4c67db150 --name-only`：20 个文件与任务书清单逐项一致，无清单外文件；提交体量 4109 insertions / 12 deletions，与工作树此前累计改动吻合。
4. 敏感内容扫描：对提交全文检索 api_key / secret / token / password / sk- / Bearer，无命中；新增测试与夹具均为合成内容。
5. 提交前验证复核：`cargo test --lib reading_store` 12/12、`git diff --check` 0 为 BUILDER 实跑；ARCHITECT 此前已实跑全量 1232/0/1 与 tsc/build，未回退。

**遗留（有意保留）：** 提交推送记录小节（STATUS.md 748 行起）是在提交之后追加的执行记录，按 OD-READ-001 的既有口径留在工作树，不追加提交；是否随下次发版收口由用户决定。

**结论：** OD-READ-001 初步实现、阶段 4 引文核验与提交推送全部通过终检；本轮闭环。真实桌面窗口交互与真实模型质量仍按 REVIEW.md 上节的未验收项执行，属用户日常使用验证。

## 0.2.36 发版复核（2026-11-18，ARCHITECT）——通过（附实机未验证项）

**核对（gh 实查，非推测）：** run `44203067532` 的 meta / macos / windows / publish 四个 job 全部 success；release `v0.2.36` 已发布，含 `OmniDesk_0.2.36_universal.dmg` 与 `OmniDesk_0.2.36_x64-setup.exe` 两项产物，未标记 prerelease。发布版本号与 `VERSION`、`package.json`、`tauri.conf.json` 三处一致。

**未验证：** Windows 实机安装、读书页导入/划线/AI 交互与真实模型输出质量；均由用户在单位电脑上验收。

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

# OD-ARCH-178 复核（2026-11-17，代码级通过；应用户指示不发版）

**背景：** 用户 407s 回合日志（omnidesk-407s-turn-log.md，ARCHITECT 已全文核对）证实三件事：①Mac 安装的 0.2.32 中普通对话把所选档位无证据下发，连续 4 次被 selectModel 拒绝；②发行链 opendesign 模型未声明 reasoningEfforts（声明只存在于 dev profile patch），未声明 = 档位永远设置不成功，模型跑默认重思考档；③404 秒回合 = 92.7% 模型生成（62 次调用、上下文 13.8 倍膨胀、单条 54KB bash grep 断崖）+ 约 10 个死胡同步 + 收敛规则无硬执行，合理成本约 8 秒。

**审查对象：** 子代理 Heisenberg（阶段 A，Rust）、Zeno（阶段 B+C，前端），均为 deepseek-v4.1-flash-eco / high。

**复核结果：**

1. **阶段 A：** `VERIFIED_EFFORTS` 静态清单（唯一实测条目 opendesign/deepseek-v4.1-flash 7 档）同时并入 `task_list_model_routes` 与覆盖文件渲染；Profile 已声明不覆盖；`yaml_scalar` 修正 off/none 的 YAML 1.1 引号。复核通过。
2. **阶段 B：** 逐站点审计（7 类 submitTask 站点 + SettingsPanel）确认全部走证据门；普通对话/未分类场景为 0.2.32 缺口站点，已接同一证据门；api.ts `gatedReasoningEffort` 作为最后一道防线（漏传证据静默降级 null）。复核通过。
3. **阶段 C：** ModelPicker 删除固定五档，按钮由 `modelEffortOptions`（声明集合 + 跟随模型默认）渲染，未声明只出默认——与 DSH 原版目录驱动行为一致。复核通过。
4. **ARCHITECT 实跑验证：** cargo test --lib 单线程 **1217/0**（本人复跑确认）；tsc / npm run build 通过；verify_arch_178 20+24、arch_176 71/71、175 12+12、177 11/11、202 39/40（既有失败）。
5. **真实 DSH 双向验证（ARCHITECT 亲自执行）：** 负向——未声明模型传 off → 适配器降级、任务成功（verify_effort_select_degrade 4/4，0.2.33 已有）；**正向——覆盖文件声明 reasoningEfforts 后，真实 selectModel 接受 off 且任务成功**（新增正向用例 PASS），声明链路端到端打通。

**结论：** OD-ARCH-178 阶段 A/B/C 代码级通过。按用户指示**不发新版本**，全部改动保留在工作树（HEAD 95d11d0169）未提交。阶段 D（bash 字节上限、步数硬拦截、报错回灌缺陷）与发版（0.2.34）待用户后续指令。已安装用户获得声明需升级后点「修复缺失 / 恢复出厂内容」或经设置页保存一次路由。

# OD-ARCH-179 阶段 D 立项结题复核（2026-11-17，通过；ARCHITECT 亲自执行）

**审查对象：** seed discipline persona 追加「自查收敛规则」两条；docs/decisions.md D-109（档位能力架构 + runtime 上游缺口登记）；STATUS/PLANS 记录。

**复核结果：**

1. **D 项三件事的定性（全部有 runtime 源码或事件流证据）：** ①bash 截断已存在（尾部保留 + spill 落盘），阈值内部常量不可配（tool-bash Config 仅两个开关）——上游缺口；②agent-loop 无步数预算配置面——上游缺口；③`agent/inbox/spliced` 为 DSH 原版失败自适应机制（session-d87d30e7 事件流第 24 行实证），非缺陷。极简口径下不做宿主侧 hack，缺口登记 D-109 并设复审触发。
2. **persona 规则极简且对症：** 两条规则直接针对 404 秒回合的实证浪费（答案文件第 44 步才被读到；54KB grep 断崖）；work-review-core 无 shell 不适用故不加；规则总量约 190 字符，不违反 0.2.30 的提示词瘦身口径。
3. **验证：** `probe_work_review_preset_213.py` 真实 DSH CLI 引导 seed profile 成功（YAML 被 DSH 解析器实吃，强于静态校验）；work-review 断言不受影响；ARCHITECT 实跑确认。

**结论：** 阶段 D 立项结题。发版（0.2.34）与 Windows/Mac 双端实测仍待用户指令；本轮所有改动保留工作树未提交。

# OD-ARCH-178/179 发版复核（2026-11-17，通过；0.2.34 已推送）

**背景：** 用户批准登记 swamp 借鉴候选（D-110）并推送发新版本实测思考等级。ARCHITECT 亲自执行发版链。

**复核结果：**

1. **D-110 登记核对：** 候选方向（周期流程声明式定义，借鉴 swamp 类型/定义分离）写明「未采纳、真实需求出现才立项」与三条复审触发；授权冲突（AGPL vs PolyForm）、账号前提冲突、平行事实源三个不可融合理由如实记录。
2. **seed 修正（关键）：** 本轮初推前发现 `runtime/dsh-home-seed/` 在 .gitignore（打包期由 prepare-runtime-assets.sh 从 samples/ 生成），我此前只改了 seed 副本——**发行包不会带上自查规则**。已把「自查收敛规则」两条同步进权威源 `samples/profiles/discipline.profile.patch.yml`（discipline-core 与 work-review-core 两处 persona 的 suffix；work-review 无 shell，规则文本仍随 preset 声明、实际不触发）。seed 与 samples 的 web-search-free 差异为历史遗留（samples 已按 OD-ARCH-173 移除、seed 副本未重建），以 samples 为准，不回改。
3. **版本链：** 0.2.33 → 0.2.34（VERSION / package.json / tauri.conf.json），CHANGELOG 0.2.34 条目含升级说明；推送后 release.yml 以 v0.2.34 构建发布。

**结论：** 0.2.34 发布通过。升级后用户需在 Profile 资产页点「修复缺失 / 恢复出厂内容」获得档位声明与自查规则；实测「思考等级是否真正生效」后启动 swamp 借鉴设计立项。

# OD-ARCH-181 与 0.2.35 发版复核（2026-11-18，通过）

**背景：** 用户实机确认逐模型档位生效后提出两处微调（档位按钮缩小一半、对话选择器去掉「跟随模型默认」），批准随 0.2.35 发版。

**复核结果：**

1. **逐模型声明链路：** ModelEntry 新增 reasoningEfforts（用户声明优先于 Profile 与已验证清单）；设置页 7 档 chips 写入该字段；cargo 单测覆盖“用户声明覆盖清单”。
2. **微调实现：** 按钮字号 9px/紧凑内边距（约默认一半）；modelEffortOptions 声明非空时只列档位（去掉跟随默认）、为空时保留单按钮兜底——避免无档位模型的空档位区。
3. **验证：** tsc / build 通过；verify_arch_178 20/20+25/25、arch_176 71/71、202 39/40（既有失败）；cargo 单线程 1218/0。本地 tauri build 的 .app 正常（DMG 步骤因旧实例占盘失败，与代码无关）。

**结论：** 0.2.35 发版通过。既往遗留：OD-ARCH-175/176 阶段 4 真实验证随 0.2.35 更新后一并复测；docs/skills-spec.md 草稿待用户评审。

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


# OD-ARCH-183 Windows 引擎进程树回收独立复核（2026-11-18）——通过（静态/脚本范围，Windows 实机待验收）

**复核对象：** HEAD `cb0a0b31f0` 之上的未提交改动：`process_owner.py`、`web_api_adapter.py`、`serve.py`、`platform.rs`、`process_owner.rs`、`lib.rs`、`verify_process_platform_133.py`、`test_process_owner_unit.py`。

**结论：** POSIX 行为未回退；Windows 清理链路在代码层面补全（引擎树杀 + 宿主登记回收 + 死父进程清扫 + 退出兜底）；Windows 实机行为未验收，不得标为「已在 Windows 生效」。

**证据（实际运行）：** `cargo test --offline --lib` 1232/0/1；engine adapter `unittest discover` 249 通过（含新增 5 个 Windows 分支单测）；`verify_process_platform_133` 28/28；`verify_sys_process_701` 40/40；`verify_proc_alias_149` 34/34；`py_compile` 通过。

**风险与限制：**
1. Windows 专属代码无法在本机构建（`x86_64-pc-windows-msvc` 目标已装，但构建脚本缺 `llvm-rc`），只能人工审查 + 单测用注入假实现覆盖判定分支；`taskkill`/`tasklist`/PowerShell 的真实行为需 Windows 实机验证。
2. `verify_process_lifecycle_074` 的 4 项失败在 HEAD 纯净副本复现，属既有环境问题（非本改动引入）。
3. 命令行指纹清扫只杀「`dsh-runtime` + `bin.js` + `--profile` 且父进程已退出」的 node，不按裸 node 名全杀；若用户同时运行另一份 OmniDesk 且其引擎父进程存活，不会被误杀。

# OD-READ-002 阶段 1 独立复核（2026-11-18，BUILDER 子代理 Godel / deepseek-v4.1-flash-eco / high）——通过（静态/脚本范围，附未验收项）

**复核范围：** 工作树 `/Users/yvan/.codex/worktrees/2221/MyWorkStation`（分支 od-read-002-brain-room，HEAD cb0a0b31f0 不变）上 BUILDER 的未提交改动：`rss_store.rs`（新增，1560 行）、`lib.rs` 6 个 RSS 命令注册、`reading_store.rs` 三个辅助函数提升 `pub(crate)`、`Cargo.toml`/`Cargo.lock` reqwest 依赖、`ReadingPage.tsx` 健脑室改名 + 书架双分组 + 订阅模态框 + 未读圆点、`Dock.tsx`/`App.tsx` 文案、`api.ts`/`types.ts` 六命令四类型、`verify_od_read_002_stage1.py`。

**ARCHITECT 实跑（非自述采信）：**

1. `cargo test --offline --lib -- --test-threads=1`（OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation）→ **1243 passed / 0 failed / 1 ignored**（48.9s）。不带该环境变量时 3 项 `app_paths` 路径边界测试失败，原因是 worktree 路径 `~/.codex/...` 先命中 `/.codex` 禁用片段——环境工件，与 RSS 改动无关；同样测试在主仓库路径全过。rss_store 新增 11 条测试全部通过（含本地回环合成 HTTP 回放）。
2. `npx tsc --noEmit` → 0；`npm run build` → 0（ReadingPage chunk 45.66 kB）；`python3 tests/verification/desktop/verify_od_read_002_stage1.py` → 全部检查通过（42 项）；`git diff --check` → 0。
3. 关键 diff 人工核对：HTTP 客户端只向订阅 URL 发 GET（`fetch_feed_xml` 校验 scheme、20s 超时、2MB 上限、UA OmniDesk-RSS/0.1）；预置清单 9+40 条含 MIT 归属注释、`preset_seed_is_idempotent_and_does_not_resurrect_deleted_items` 保证用户删除不被复活；文章入库复用 `ReadingBook` 且去重不覆盖批注；划线浮条四按钮与 `data-paragraph` 结构未动。

**发现与处置：**

1. **Cargo.lock +215 行（TLS 传递依赖）**：任务书允许引入 reqwest，BUILDER 选择 `native-tls`（macOS 走系统 Security Framework）+ `system-proxy`，理由成立；锁文件增量是 TLS 后端的正常代价，接受。
2. **BUILDER 报告的偏差属实**：新增 `rss_articles_list`/`rss_article_mark_read` 支撑未读圆点；正文以净化纯文本段落入库（不保留 Markdown 标记，避免阅读器把 `#`/`**` 渲染为正文）——两者均为合理的小范围扩展，不越出计划边界。
3. **worktree runtime/ 下曾出现 4 个指向主仓库的符号链接**（构建脚本需要），BUILDER 交付前已删除；ARCHITECT 复跑测试时重建了同样的 4 个符号链接（`runtime/dsh-runtime`、`dsh-home-seed`、`pnpm`、`python-vendor` → 主仓库对应目录），它们未被 .gitignore 覆盖、会出现在 `git status` 未跟踪区。**提交时不得加入这 4 个符号链接；如后续在主仓库构建，可直接删除。**
4. 阮一峰 `atom.xml` 在默认 UA 下被 Cloudflare 拦截返回 HTML，运行时会写入可读 `last_error` 并在订阅模态框显示——行为符合「失败不静默」要求；真实效果留窗口验收。

**通过范围与未验收项：** 存储边界、解析正确性、去重、幂等预置、构建链路**通过**；真实桌面窗口的订阅模态框交互、49 源串行拉取耗时、RSS 文章内划线/想法/标段/问 AI 端到端、真实模型输出**未验收**，不得标为「真实可用」。提交推送需用户裁决后另行派工。
