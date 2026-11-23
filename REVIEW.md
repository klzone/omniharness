## OD-PRJ-602 复核（2026-11-23，ARCHITECT 复核 BUILDER 交付）：通过（源码 diff + 本机复跑）

**范围：** 项目甘特分组行（组头 + 汇总带 + 折叠联动）、页面令牌化重构、ScenePage 死代码清除、数据一致性静态验收、版本 0.2.52 发布准备。

**源码事实（ARCHITECT 逐段审阅实际 diff）：**
1. `projectGanttGroups.ts`：分组/夹取/排序/折叠纯函数，坐标口径与旧 ganttRows 完全一致（created_at→due_date、窗口夹取、2% 宽度下限、clamped 标记），无第二套换算；未挂接口径与宿主 project_id 空串一致；空项目与窗口外任务不出现。
2. `TaskManager.tsx`：甘特渲染分组行——组头（项目名 + 任务数 + 汇总区间 + 浅色汇总带，可点击折叠）+ 组内任务行（状态色条、点击开编辑弹窗、hover 联动）；today 竖线贯穿；超 30 行自动折叠未挂接组与最早结束组并提示；TASK_STATE_BAR 改语义类。
3. `theme.css` 只新增 `.od-task-bar--*` 六段（既有段未动）；三个组件内写死 hex 全部换 `--od-neu-*` 令牌，靛蓝渐变按钮改实色 accent；布局与 compact 变体不变。
4. `ScenePage.tsx` 只删 projectItems 前端死代码（状态、effect、saveProjectItems、void 占位、import），localToday 仍被 NotificationCenter/WeeklyTimeline 使用（各自独立定义/引用，清理正确）；宿主 project-items 命令与用户文件保留。

**ARCHITECT 复跑（script，2026-11-23）：**
- `verify_project_602.py` 18/18；`verify_project_601.py` 15/15；`verify_project_ui_716.py` 7/7；`verify_task_quadrant_ui_402.py` 10/10（既有断言全部未动）。
- `npx tsc --noEmit` EXIT=0；`npm run test` 97/97；`cargo test --lib --offline` 1292/0/1；`npm run build` EXIT=0。
- `git status --porcelain runtime/` → 空。

**未验证（如实声明）：** 真实窗口甘特观感（组头密度、长任务名截断、hover 联动）、真实模型复盘输出、Windows 实机、`cargo fmt`（rustfmt 未安装）。

**结论：** 验收通过；版本 0.2.52（VERSION/package.json/tauri.conf.json/CHANGELOG 已同步），可提交推送。

---

## OD-PRJ-601 复核（2026-11-23，ARCHITECT 复核 BUILDER 交付）：通过（源码 diff + 本机复跑）

**范围：** 项目跟踪页四象限完成确认（二次确认 + 归档展示）、AI 项目复盘（存档/确认/知识卡提炼）、项目维护区（新建/挂接/换绑/删除 + 分页）、版本 0.2.51 发布准备。

**源码事实（ARCHITECT 逐段审阅实际 diff）：**
1. `TaskManager.tsx`：`displayViews` 过滤已完成/已取消任务（暂停保留）；「标记完成」按钮只对非终态显示；`ConfirmDialog` 确认后才调 `taskTransition`，待确认先转待开始、暂停先转进行中（两步迁移，中途失败如实报错并刷新）；完成后设置 `reviewSeed` 让复盘面板选中该项目；统计卡数字口径未改。
2. `project_review_store.rs`：五段固定标题按序校验、`#t-…` 引用必须落在 task_ids、≤1MB、草稿覆盖留 `.bak`、已确认拒绝覆盖、confirm 幂等；与周期复盘 `review_store` 的既有模式一致（两者都不做内容闸门，口径一致）。
3. `ProjectReviewCard.tsx`：材料只含项目字段与任务编号/标题/状态/进展/历史原文（≤20 条，不调模型）；固定五标题契约提示词；`prepareWorkspaceDir → submitTask → taskStatus` 轮询（10 分钟后继续等待不取消）→ `projectReviewSave`；确认归档、提炼知识卡（`knowledgeReviewStage`）齐全；未挂接项目时如实禁用并提示。
4. `ProjectMaintenance.tsx`：新建项目（projectCreate）、项目下拉（projectList）、任务分页（每页 8 条，page 越界自动收敛）、挂接/新建任务（taskUpdate.project_id / taskCreate）、换绑/摘除与删除均过 ConfirmDialog；读写失败显示原文错误。

**ARCHITECT 复跑（script，2026-11-23）：**
- `python3 tests/verification/desktop/verify_project_601.py` → 15/15 passed。
- `cd desktop && npx tsc --noEmit` → EXIT=0；`npm run test` → 88/88；`npm run build` → EXIT=0。
- `cd desktop/src-tauri && cargo test --lib --offline -- --test-threads=1` → 1292 passed / 0 failed / 1 ignored。
- `git status --porcelain runtime/` → 空。

**未验证（如实声明）：** 真实窗口交互手感、真实模型复盘输出质量、Windows 实机、`cargo fmt`（rustfmt 未安装）。

**结论：** 验收通过；版本 0.2.51（VERSION/package.json/tauri.conf.json/CHANGELOG 已同步），可提交推送。

---

## OD-WR-312 复核（2026-11-22，ARCHITECT 实施并复核）：通过（源码 + 本机实跑）

**范围：** 工作记录页主入口「记录今天工作」、草稿单槽覆盖、同日草稿在周视图可编辑、页面正文字体思源黑体 14px。

**源码事实：**
1. `work_record_create_impl` 在 `draft=true` 时按 `record_date` 查同日草稿并 `update_record`（title/body/record_date），不新增；`draft=false` 走原新建路径，正式记录不参与覆盖。
2. `WorkRecordLibrary` 主按钮改为主色按钮并自动聚焦正文；记录卡片原有的「草稿」徽标与详情编辑（含草稿勾选保存）继续可用。
3. `WorkRecordPage` 根容器应用 `od-worklog-page`，`theme.css` 设思源黑体字族与正文 14px（卡片预览、`.md-body`、textarea）；字体由 `@fontsource-variable/noto-sans-sc` 自托管加载。

**ARCHITECT 实跑：**
- `cargo test --lib --offline -- --test-threads=1` → 1281 passed / 0 failed / 1 ignored，0 warning（新增同日草稿覆盖、正式记录不被覆盖 2 项单测）。
- `npx tsc --noEmit` EXIT=0；`npm run test` 88/88；`npm run build` EXIT=0。
- dev 模式合成文本：勾选草稿连续保存，第二次覆盖同一条记录（id 不变、updated_at 更新、总数不变）；未勾草稿的记录保持独立；按钮与字体在截图中生效。

**数据处置：** 合成测试记录 3 条移入 `work/.trash/工作记录/`，原 12 条记录未改。

**未验证：** release 安装包未重建（版本仍 0.2.49）；Windows 实机；周期复盘页签字体观感。

---

# 0.2.48 全量评估（2026-11-22，ARCHITECT 亲自执行，聚焦问题发现与解决）

## OD-UI-718 复核（2026-11-22，ARCHITECT 实施并复核）：通过

**问题：** 0.2.48 release 版打开「历史会话」页后整窗黑屏（可稳定复现，screencapture 确认为黑色空白窗口，WebView AX 树只剩窗口本身）。

**根因（源码事实 + dev 实跑）：** `sceneThreads.ts` 的 `hydrateThreads` 把「浏览器缓存中存在、但数据根没有对应 id」的线程作为 leftovers 直接并入内存列表。0.2.44 起缓存只写元数据（T9），这些 leftovers 没有 `messages` 字段；`SessionHistoryPage` 读取 `thread.messages.length` 抛 `TypeError`，React 无错误边界，整棵组件树卸载，Tauri 窗口显示黑色。dev 模式加错误边界后取得完整堆栈：`undefined is not an object (evaluating 'thread.messages.length')`（SessionHistoryPage.tsx:246/302）。

**修复（3 处）：**
1. `sceneThreads.ts` 新增 `normalizeCachedThread`，迁移分支与 leftovers 分支统一补齐 title/tag/messages，保留已有正文。
2. `SessionHistoryPage` 所有 `messages` 读取改为容错（条数、预览、最近消息列表、正文 `text`）。
3. 新增 `ErrorBoundary` 并在 `main.tsx` 包裹 App，异常时显示可读错误与「重新加载界面」，避免整窗黑屏。

**ARCHITECT 实跑：**
- dev 模式：修复前必现异常，修复后历史会话页正常渲染；点击 0 条消息的缓存遗留卡片打开抽屉正常；继续对话回首页正常。
- release 0.2.49：打开历史会话渲染 51 段（3 分类，含 32 条缓存遗留），点击遗留卡片、继续对话均正常，无崩溃。
- `npx tsc --noEmit` EXIT=0；`npm run test` 88/88。

**遗留说明：** 缓存遗留线程只剩元数据（0.2.44 前缓存中的正文若已被 T9 清理则不可恢复），页面以「0 条消息」如实显示，不伪造消息；这些线程仍可继续对话。Windows 实机与归档/删除交互手感未验证。

**版本：** 0.2.49（VERSION / package.json / tauri.conf.json / CHANGELOG）。

**Git：** 未提交、未推送、未打标签。

---

**评估对象：** HEAD `a1388be6fe`（0.2.48）及 0.2.46–0.2.48 三个提交：DSH 0.2.2-rc.5 升级、运行时精简（OD-SLIM-022 策略 B）、Windows 进程收尾与插件依赖修复。

**ARCHITECT 实跑（本机 macOS 27.2 / Node 26.13.0）：**
- `cargo test --lib --offline -- --test-threads=1`：1279 passed / 0 failed / 1 ignored（83.42s，与 STATUS 记录一致）。
- `cd desktop && npm run test`：88/88 通过；`npx tsc --noEmit` EXIT=0。
- `dsh --version`（仓库 runtime，剪枝后）→ 0.2.2-rc.5；`dsh plugin --profile web list` EXIT=0。
- `python3 scripts/diagnose-omnidesk.py`：引擎未运行；模型路由元数据正常（open-design 端点、discipline Profile）。
- `bash scripts/omnidesk-isolation-check.sh`：15 通过 / 22 失败（见 P1）。

## 评估结论（按严重程度）

### P1（高，开发链路阻断，根因已定位）：隔离自检与 dev 脚本在 macOS 27 上全部失败
**事实：** `scripts/omnidesk-dev.sh --check`、`scripts/sync-skills.sh --check/--dry-run`、`omnidesk-isolation-check.sh` 均在 `realpath -m --` 处以 `illegal option -- m` 退出 1；隔离自检 22 项失败全部由此级联（脚本提前退出导致各项断言判 FAIL）。
**根因：** macOS 27.2 更新了 /bin/realpath（2026-10-23 构建），不再支持 `-m` 选项；本机无 GNU coreutils，`abspath()` 的 fallback 永远不触发。属系统升级引入的回归，与 0.2.46–0.2.48 三个提交无关。桌面 App 本体不受影响（Rust 侧不依赖 shell realpath）。
**修复方向（待批准）：** 改两处 `abspath()`（`scripts/omnidesk-dev.sh:66`、`scripts/sync-skills.sh:62`）为不依赖 GNU 选项的实现（如 python3 os.path.realpath 或 pwd -P 拼接）。

### P2（中，一致性遗留）：DISCIPLINE_PLUGINS 仍声明 dsh-skill-office 为必需插件
**事实：** 0.2.47 已从 discipline patch 删除 skill-office insert 且运行时已剪枝该包；但 `profile_assets.rs:78-82` 仍把它列为必需插件。纪检专业 Profile 资产页会持续显示缺失并可一键装回，与策略 B 冲突。用户隔离根的旧 patch 还残留 skill-office insert 与未禁用的 tool-schedule 行（被剪包），是启动时 67 条导入告警的主要来源。
**修复方向（待批准）：** 从 DISCIPLINE_PLUGINS 移除 dsh-skill-office；清理用户隔离根旧 patch 的残留引用（涉及隔离根写入，须用户授权）。

### P3（低，文档漂移）：README 版本行停留 0.2.29（README.md:76），实际 0.2.48。
### P4（低，文档漂移）：docs/OmniDesk-Engine-Interface-Audit.md 仍以 dsh-v0.2.1-alpha.2 为审计对象，与 RUNTIME.lock（0.2.2-rc.5）脱节。
### P5（低，测试卫生）：cargo test 有 3 条 warning（review_store.rs 未用变量 now / 未用函数 weekly_text、writing_section.rs 未读字段 config）。

### 已验证正常的部分
- **0.2.48 Windows 修复本体**：kill_pid 的 taskkill 分支、窗口 Destroyed 收尾（stop_settings_service + cleanup_child_on_exit）、dsh plugin 对 plugin-manager/plugin-detail 的依赖恢复均逐行核对，本机 plugin list 实测通过；Windows 实机行为仍待用户验证。
- **DSH 0.2.2-rc.5 适配面**：启动参数、303+Set-Cookie 鉴权、session/credentials/llm RPC、插件 peer 版本（cost-meter 1.9.6 / free-search 0.9.5 要求 >=0.2.0-rc.1 <0.3.0）均兼容。
- **精简完整性**：seed 三份 profile 与 samples patch 无对被剪包的悬空启用引用。
- **Rust 安全边界**（net_guard / host_access / export_engine 授权门）：1279 项测试覆盖通过。

**未验证：** Windows 实机关闭后进程列表、设置页安装插件、缺件提示消失；DSH 0.2.2 真实模型任务含工具调用；golden_tasks 40 项真实模型评测。

**下一轮建议（待批准）：** ①修 P1 两处 abspath 并重跑 isolation-check 全绿；②P2 删 skill-office 声明；③P3/P4/P5 文档与 warning 清理。合计约 1 小时，不涉及引擎协议改动。

## OD-FIX-048R 复核（2026-11-22，ARCHITECT 独立复核）：通过

**派工与执行：** BUILDER 子代理（deepseek-v4.1-flash-eco / high）按上述建议执行；ARCHITECT 逐文件审阅实际 diff 并独立复跑关键验证。

**逐项核对（源码事实 + 独立实跑）：**
1. **P1 通过。** 两处 `abspath()` 改为 `python3 os.path.realpath`（无 python3 时退回原样 printf），`omnidesk-dev.sh:190` 另一处裸 `realpath -m` 改走 abspath（同文件必要修正，已记为计划偏差）。全仓 `rg 'realpath -m' scripts/` 无残留。ARCHITECT 实跑：`omnidesk-dev.sh --check` EXIT=0；`sync-skills.sh --check` EXIT=1 且输出 9 个 Skill 漂移（--check 对既有漂移的原有语义）；`omnidesk-isolation-check.sh` **37 通过 / 0 失败**（修复前 15/22）。
2. **P2 通过。** `DISCIPLINE_PLUGINS` 仅剩 dsh-cost-meter，注释说明 office 已随 0.2.47 精简移除。bundled 探测测试的夹具从 `DISCIPLINE_PLUGINS[0]`（现为 Profile 安装依赖）改为 `@deepseek-ai/dsh`，测试语义不变；首次全量 1278/1 的失败即此夹具，调整后复绿。残留：`PROFILES` discipline 简介文案仍写「10 个 Skill 与办公插件」，超出本轮白名单未改，记入后续待办。
3. **P3/P4 通过。** README 版本行 0.2.48；审计文档对齐 0.2.2-rc.5 并以补记明确「审计明细仍是 0.2.1-alpha.2 时的记录，未重新审计」，无虚构结论。
4. **P5 通过。** 删除 review_store 未用函数/变量与 writing_section Fixture 未读字段；ARCHITECT 独立复跑 `cargo test --lib --offline -- --test-threads=1`：**1279 passed / 0 failed / 1 ignored**（55.93s），编译输出 **0 条 warning**。

**边界事件（需用户知悉）：** `omnidesk-dev.sh --check` 按脚本既有逻辑在用户隔离根初始化了 `~/OmniDesk/runtime/dsh-home/profiles/omnidesk-dev`（仅 package.json + cordis.patch.yml，来自仓库模板）。这是脚本既有行为，非本轮代码引入；目录未删除未覆盖其他数据，处置（保留 / 删除）待用户决定。

**遗留与未验证：** 用户隔离根旧 patch 的被剪包残留引用（skill-office insert / tool-schedule 行）本轮明确不做，仍待授权清理；skill 同步的写入动作未执行（9 个 Skill 漂移待同步）；Windows 实机与真实模型任务仍未验证；rustfmt 未安装，`cargo fmt --check` 未执行。

**Git：** 未提交、未推送、未打标签。工作区改动：README.md、STATUS.md、REVIEW.md、scripts/{omnidesk-dev,sync-skills}.sh、desktop/src-tauri/src/{profile_assets,review_store,writing_section}.rs、docs/OmniDesk-Engine-Interface-Audit.md。

---

**（0.2.47 当时证据存档）** prepare 后运行时由 1.4 GB 降到 315 MB；剪枝后的运行目录能启动 `dsh web` 并打印回环公告；缺失插件只产生导入告警。三份 patch YAML 解析通过。精简符合批准的策略 B；已知代价是 DSH Web UI 插件管理页、office 工具、子代理与 DSH 调度不可用。Windows 实机行为与安装包体积未验证。


# DSH 0.2.2-rc.5 升级复核（2026-11-20，ARCHITECT 实施并复核）——本机范围通过，实机任务待用户验证

**对象：** `runtime/RUNTIME.lock`、`runtime/dsh-runtime.package*.json`、`platform.rs` 的 Node 门槛、README/脚本/发布说明、`docs/decisions.md` D-109。

**实跑证据：** 提交的 lock 在临时目录 `npm ci` 成功，`dsh --version` = `0.2.2-rc.5`；`dsh web` 公告与 303 鉴权实测兼容；`cargo test --lib` 1277 passed / 0 failed / 1 ignored；`npm run test` 88/88；`npm run build` 通过；`test_web_api_unit.py` 105 tests OK；验证脚本 77/77、7/7。

**结论：** 升级已落地，集成面（启动参数、公告、鉴权、RPC、插件 peer）在本机核对兼容；Node 门槛同步到 24.9。未验证：真实模型任务、设置窗、订阅与导出、`session.v4` 存档格式、golden_tasks、CI 用 Node 24 的完整构建。回滚方式写在 D-109。


# OD-HARDEN-2043 复核（2026-11-20，ARCHITECT 实施并复核）——A/C/B/E 通过（静态/脚本范围）；D 仅第 1 步，未完成

**复核对象：** 发布门禁（`release.yml`）、外联 URL 校验（`net_guard.rs` + `rss_store` + `task_shell::read_remote_media`）、宿主授权（`host_access.rs` + 八个命令 + 四个前端入口）、小项加固（`writing_parse` 超时、实体检查、unwrap 清理）、`lib.rs` 拆分第 1 步。

**实跑证据：** `cargo test --lib -- --test-threads=1` 1266（A 基线）→ 1276（C）→ 1284（B）→ 1284（E、D 第 1 步）全绿；`npm run build`（含 `tsc --noEmit`）每次 EXIT=0；`verify_hardening_2043.py` 67/67；`verify_reading_entity_2043.py` 7/7；`verify_reading_004_formats.py` 19/19。命令名集合迁移前后均为 277，`generate_handler` 与定义一一对应。

**逐项结论：**
- **A 通过（本机范围）**：`pull_request` 触发 + `test` job + `needs` 依赖图成立（`publish` 依赖 `test`），无 `continue-on-error`；Kotlin 无关。未验证：actionlint 未安装（用 ruby YAML 解析 + 脚本断言替代）；Linux runner 的 apt 依赖与首次 cargo 编译、GitHub 上的真实 PR 行为。
- **C 通过**：地址校验覆盖回环/链路本地/未指定/多播/广播/私网/CGNAT/ULA 与 `::ffff:*`；订阅按 3 跳逐跳校验并禁止 https 降级，远程媒体固定公网档；上限统一 8 MiB；系统代理残余风险已写入 `docs/decisions.md` D-108。未验证：真实代理环境与真实公网跳转链。
- **B 通过（本机范围）**：注册表 30 分钟滑动过期、256 条上限、按真实落点判断；八个命令都有授权门；`dialog:allow-open` 已移除且前端不再 import plugin-dialog；拖拽路径在宿主侧登记；凭证黑名单 11 项。未验证：真实窗口四条手工路径、重启后旧根目录可用性、Windows 选择框。
- **E 通过**：超时路径 10 秒 kill；docx 实体检查改为全文扫描（原 4 KB 之后可漏）；9 处非锁 unwrap 逐条处理，剩余均为锁（engine_ipc 14、dsh_settings 4）按口径保留。未验证：故意挂住的子进程实际被 kill 的端到端。
- **D 未完成**：只迁出阅读/RSS 11 条命令到 `src/commands/reading.rs`，命令集合与测试结果不变；任务、工作记录、阅读其余、知识库、写作、设置、引擎六个领域未迁。

**结论：** A、C、B、E 可按现状进入实机复测；D 不得标为完成，下一轮按领域继续，每迁一个领域重跑命令基线比对与全量单测。


# OD-HARDEN-2043R 复核（2026-11-20，ARCHITECT 实施并复核）——T2/T5/T6/T7/T9/T10/T11/T12/T13 通过（静态/脚本范围）；T14 未完成

**实跑证据：** cargo test --lib 1286 passed / 0 failed / 1 ignored；`npm run test` 88/88；`npm run build` 通过；test_web_api_unit.py 105 tests OK；verify_hardening_2043.py 77/77；verify_reading_entity_2043.py 7/7；profile `npm ci` 成功；Python 锁文件 `--require-hashes` 安装成功且篡改哈希后失败。

**结论：**
- T2 通过（本机范围）。偏差：工具链固定 1.99.1 而非 rust-version 1.77，因为 1.77.2 实测无法解析依赖 manifest（resolver 3）；属任务书「无法编译先报告」的适用场景，已记录。未验证：GitHub runner 实际构建。
- T5、T6、T7 通过；T6 的 Windows 手工验证未做。
- T9 通过静态/类型检查，缺单测（依赖 window.localStorage，未补 mock）——按验收标准属未完成项。
- T10 通过本机可验证部分（profile npm ci、Python 哈希锁、pnpm 固定）；bootstrap 全量安装与 Windows 哈希未验证。
- T11 通过本机合成校验；T12、T13 通过单测。
- T14 仅第 1 步，不得标完成。

# OD-KB-202 复核（2026-11-19，ARCHITECT 实施并复核）——通过（脚本/静态范围；真实窗口待 0.2.42 实测）

**复核对象：** 暂存卡审核链路（knowledge_vault 列表/选中同步/丢弃、lib.rs 命令与治理记录、审核页暂存卡区、整理材料弹窗二次确认）。

**实跑证据：** cargo 全量 1250/0/1；tsc/build 通过；verify_knowledge_staging_202.py 14/14（含真实 Rust 单测：仅勾选卡入库、未选保留、丢弃成功、../escape.md 被拒）。静态核对：选中同步沿用原有 frontmatter/字数/冲突/索引规则；落库成功后才删除暂存文件；治理记录失败只追加说明。

**结论：** 暂存卡现在先出现在「知识审核」的待审区，用户逐张或批量确认后才入库；通过复核，可随 0.2.42 发布复测。

# OD-ARCH-187 复核（2026-11-19，ARCHITECT 实施并复核）——通过（单元/静态范围；真实窗口待 0.2.41 实测）

**复核对象：** `engine/omnidesk_engine/web_api_adapter.py` 审批归属逻辑、`tests/verification/engine-interface/adapter/test_web_api_unit.py`、`skills/knowledge-search/SKILL.md`。

**实跑证据：** 单元测试 102/102（含新增唯一活跃任务归属、歧义时 engine_ask 提示、精确匹配回归）；知识库同步验证 48/48；`py_compile` 通过。修复点：审批帧 agentId 不匹配任务的 engine_session_id 时，不再静默丢弃；唯一活跃任务接手并在事件里标注归属方式，多活跃任务则上报可见的 engine_ask 提示并保留诊断索引。旧断言「unmatched 不归属」已按新语义更新（静默丢弃正是本次故障本身）。

**结论：** 代码与测试通过复核，可随 0.2.41 发布复测；未在真实窗口复现用户那次子代理审批场景。

# OD-READ-004 ARCHITECT 复核（2026-11-18）——通过（脚本/静态范围；真实 MOBI/PDF/窗口待用户验收）

**复核对象：** 工作树 `od-read-002-brain-room` 上 OD-READ-004 改动：`HomeCanvas.tsx` 布局、`ReadingPage.tsx` 格式过滤器、`task_shell.rs` 扩展名白名单与 MOBI 单测、`reading_attachment_extract.py` MOBI 解析、`python-requirements.txt`/prepare 脚本/数据布局文档、新增 `verify_reading_004_formats.py`。

**ARCHITECT 实跑：** `cargo test --offline --lib -- --test-threads=1` → 1249 passed / 0 failed / 1 ignored；`reads_reading_source_mobi_via_helper` → 1/1；`npx tsc --noEmit` → 0；`npm run build` → 0；`verify_reading_004_formats.py` → 19/19；`verify_reading_003_selection.py` → 25/25；`git diff --check` → 干净。

**结论：** 首页统计条已移到对话下方（原「项目跟踪」快捷入口已移除，对话与统计条之间保留间距）；MOBI 扩展名已进入 Rust 白名单与界面过滤器，PalmDOC 解压与 HTML 转文本经合成样例验证；PDF 依赖 pypdf 已写入随包 requirements 并实际完成文字提取验证。HUFF/CDIC 压缩与 DRM 加密会明确拒绝，属文档化限制。真实用户 MOBI/PDF 与窗口视觉仍待用户验收。

# OD-READ-003 ARCHITECT 复核（2026-11-18）——通过（脚本/静态范围；真实窗口交互待用户验收）

**复核对象：** 工作树 `/Users/yvan/.codex/worktrees/2221/MyWorkStation`（分支 `od-read-002-brain-room`，HEAD `f008ef94bb`）上 BUILDER（OpenDesign/deepseek-v4.1-flash-eco，思考档 high）交付的 OD-READ-003 全部改动：11 个修改文件 + 1 个新增验证脚本（1775 insertions / 117 deletions）。

**ARCHITECT 实跑（非自述采信）：**
1. `cargo test --offline --lib -- --test-threads=1`（`OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation`）→ **1248 passed / 0 failed / 1 ignored**（50.76s，含新增 stats/thought 口径、封面落盘、EPUB 含图端到端测试）。
2. `npx tsc --noEmit` → exit 0；`npm run build` → exit 0（ReadingPage chunk 51.54 kB）。
3. `python3 tests/verification/desktop/verify_reading_003_selection.py` → **cases=25 failed=0**（data-paragraph 锚点、selectionchange 通道、浮条条件、首页统计条、左栏页签、thought 闪记口径、odmedia 标记、cover payload）。
4. 关键修复逐条核对（文件事实）：`ReadingPage.tsx` 段落元素已补 `data-paragraph` 属性（原 closest("[data-paragraph]") 永远落空的确定性 bug）；selectionchange 150ms 去抖与 mouseup 共用 `readSelectionRef`；首页统计条 `home-reading-stats` 数字全部来自 `reading_stats` 聚合（books 排除 rss、finished≥0.95、thoughts=thought 批注数，与用户「闪记=想法」语义一致）；左栏「本地书籍/订阅文章」页签与订阅来源折叠分组实现完整；EPUB 图片经 `odmedia:` 标记 + `reading_media_extract`（路径防穿越/10MB/MIME 白名单）渲染；封面 ≤2MB 落 `meta/<book_id>.cover.*`。
5. 行内 Markdown 偏差评估：BUILDER 未直接用 react-markdown 渲染段落行内语法，理由成立——react-markdown 会把语法符号从 DOM 移除，破坏划线 code point 偏移；其「隐藏语法锚点」方案保证 DOM 文本严格等于原文，属合理实现而非范围扩大。笔记/AI 面板仍走既有 MarkdownText。
6. 计划偏差接受：①「闪记」按用户当轮语义落地为 thought 批注计数（覆盖计划原文"不做闪记"）；②新增 `reading_cover_image` 命令（展示封面所必需）；③封面字节由 Python helper 提取（未新增 Rust ZIP 依赖，Cargo.toml 不在允许清单内，处理正确）。

**边界核对：** 未触碰 runtime/、engine/、模型路由、凭证；工作树既有未提交 STATUS.md 与 runtime/* 未跟踪工件原样保留；测试数据全部在 /tmp 或一次性目录；改动已于 2026-11-19 合并进 main 并推送。

**未验收项（不阻塞合入，留用户实机确认）：** 真实桌面窗口选区浮条位置与手感、封面/长图视觉排版、真实 RSS 音视频远端加载、真实模型解释与整理质量。`OMNIDESK_REPO_ROOT` 工作树路径限制已记录（工作树位于 ~/.codex 触发 app_paths 禁区检查，按既有做法指向主仓库跑测试）。

**结论：** OD-READ-003 通过复核；改动已于 2026-11-19 合并进 main 并推送，真实窗口验收仍待用户完成。

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


## OD-READ-002 阶段 3 复核（2026-11-19，ARCHITECT 独立复核：通过）

**范围：** 健脑室右侧面板常态化折叠 + 收起图标栏 + 90 秒自动收起；首页读书统计条底部居中；③富媒体/Markdown 显示与 ④RSS 字数统计为既有实现（483e2b4177）验证。

**源码事实（ARCHITECT 逐项核对 diff）：**
1. `ReadingPage.tsx`：`panelOpen` 默认 `false`（不再按视口宽度）；收起态渲染 40px 图标栏（笔记图标+批注角标 99+ 上限、AI 图标），点击展开并切对应 tab；`PANEL_AUTO_CLOSE_MS = 90_000`，click/keydown/scroll/pointerdown 四类事件 capture 监听**仅挂面板容器**（正文滚动不续命，注释明示）；切书/收起/卸载清除计时器；划线动作条 划线/想法/标段→notes、问 AI→ai 的既有 `setPanelOpen(true)` 全部保留；宽度 200ms 过渡，窄屏 fixed overlay 保留。
2. `HomeCanvas.tsx` + `reading.css`：统计槽位 `shrink-0 flex justify-center`，`.home-reading-stats` 改 `width: fit-content` 居中；字数项在列且含订阅文章聚合。
3. ③：rss_store `![img](url)` 转换与段落偏移兼容断言均在；④：`stats.chars == 书籍 + RSS` 单测在。

**ARCHITECT 实跑（脚本运行证据）：**
- `OMNIDESK_REPO_ROOT=... cargo test --offline --lib -- --test-threads=1`：**1249 passed / 0 failed / 1 ignored**（ARCHITECT 本机独立实跑，48.94s）。
- `npx tsc --noEmit`：exit 0。
- `verify_od_read_002_stage3.py` 18/18、`verify_reading_003_selection.py` 25/25、`verify_reading_004_formats.py` 19/19。
- Boyle 自跑 `npm run build` exit 0（ReadingPage chunk 53.52 kB，既有提示）。

**结论：** 阶段 3 通过验收，可进入提交环节（提交/推送另行授权执行）。

**未验证（真实窗口，用户实测）：** 90 秒自动收起与面板内操作重置的真实手感、图标栏宽/窄屏视觉位置、划线动作条展开、首页统计条最终贴底排版、RSS 文章图片/链接实际渲染效果。

## OD-READ-002 阶段 4 复核（2026-11-19，ARCHITECT 独立复核：通过）

**范围：** 健脑室阅读区新拟物视觉改版（书 + RSS 文章），含划线浮动条同步设计、书架 288↔56 see-saw、右侧面板 320 默认宽；设计样稿见 /Users/yvan/.codex/visualizations/2026/11/18/01a1ed5a-db65-7780-a2e2-c620b62cb67d/reading-neumorphic-mockup.html。

**源码事实（ARCHITECT 核对 diff）：**
1. `reading.css` +404 行：新增 `.reading-page` 作用域变量 `--rd-raise / --rd-raise-sm / --rd-inset / --rd-inset-sm / --rd-ring / --rd-speed: 220ms`；阅读纸面 26px 圆角 + 34/44 内边距 + `--rd-raise`；正文宋体栈 16.5px / line-height 2 / justify；划线琥珀 `rgba(212,164,74,.4)`、想法主色下划线；媒体内凹窗 18px；RSS 引文井 + 3px 主色左缘；章节进度轨内凹；书架卡/搜索胶囊/tabs/进度轨/面板卡/批注卡/AI 井均接入 raise-inset 双态；架空浮动条改 14px raised 胶囊 + hover/active 双态 + 主色。
2. `ReadingPage.tsx`：书架 `w-[288px]` ↔ `reading-shelf-is-rail w-14`（56px）由 `panelOpen` 联动，220ms 宽度过渡；56px 态渲染三枚圆钮（本地书籍/订阅文章/搜索），aria-label/aria-pressed 齐全，可恢复完整列表；书架新增本地搜索过滤（书籍与文章按标题/来源/作者匹配）；面板默认 320、可拖拽；纸面/引文/进度轨/素材井接入新类；批注锚点 `data-paragraph`/`data-chapter-index`/`data-paragraph-index` 原样保留。
3. `verify_reading_005_design.py` 34 项断言覆盖设计变量、纸面、媒体、引文、书架 see-saw、面板两态、浮动条与锚点未改。

**ARCHITECT 实跑（脚本运行证据）：**
- `cargo test --offline --lib -- --test-threads=1`（OMNIDESK_REPO_ROOT 指向主仓库）：**1249 passed / 0 failed / 1 ignored**。
- `npx tsc --noEmit`：exit 0。
- `verify_reading_005_design.py` 34/34、`verify_reading_003_selection.py` 25/25、`verify_reading_004_formats.py` 19/19、`verify_od_read_002_stage3.py` 18/18。
- Boyle 自跑 `npm run build` exit 0（ReadingPage chunk 54.38 kB，既有提示）。

**结论：** 阶段 4 通过验收。剩余为用户真实窗口视觉终验（阴影层次、宋体字重、过渡手感、窄屏 overlay、浮动条 hover/active）；提交与推送待用户终验后另行授权。


## OD-READ-002 阶段 4 复核（2026-11-19，ARCHITECT 独立复核：通过）

**范围：** 健脑室阅读区新拟物视觉改版（书 + RSS 文章），含划线浮动条同步设计、书架 288↔56 see-saw、右侧面板 320 默认宽；设计样稿见 /Users/yvan/.codex/visualizations/2026/11/18/01a1ed5a-db65-7780-a2e2-c620b62cb67d/reading-neumorphic-mockup.html。

**源码事实（ARCHITECT 核对 diff）：**
1. `reading.css` +404 行：新增 `.reading-page` 作用域变量 `--rd-raise / --rd-raise-sm / --rd-inset / --rd-inset-sm / --rd-ring / --rd-speed: 220ms`；阅读纸面 26px 圆角 + 34/44 内边距 + `--rd-raise`；正文宋体栈 16.5px / line-height 2 / justify；划线琥珀 `rgba(212,164,74,.4)`、想法主色下划线；媒体内凹窗 18px；RSS 引文井 + 3px 主色左缘；章节进度轨内凹；书架卡/搜索胶囊/tabs/进度轨/面板卡/批注卡/AI 井均接入 raise-inset 双态；架空浮动条改 14px raised 胶囊 + hover/active 双态 + 主色。
2. `ReadingPage.tsx`：书架 `w-[288px]` ↔ `reading-shelf-is-rail w-14`（56px）由 `panelOpen` 联动，220ms 宽度过渡；56px 态渲染三枚圆钮（本地书籍/订阅文章/搜索），aria-label/aria-pressed 齐全，可恢复完整列表；书架新增本地搜索过滤；面板默认 320、可拖拽；纸面/引文/进度轨/素材井接入新类；批注锚点 `data-paragraph`/`data-chapter-index`/`data-paragraph-index` 原样保留。
3. `verify_reading_005_design.py` 34 项断言覆盖设计变量、纸面、媒体、引文、书架 see-saw、面板两态、浮动条与锚点未改。

**ARCHITECT 实跑（脚本运行证据）：**
- `cargo test --offline --lib -- --test-threads=1`（OMNIDESK_REPO_ROOT 指向主仓库）：**1249 passed / 0 failed / 1 ignored**。
- `npx tsc --noEmit`：exit 0。
- `verify_reading_005_design.py` 34/34、`verify_reading_003_selection.py` 25/25、`verify_reading_004_formats.py` 19/19、`verify_od_read_002_stage3.py` 18/18。
- Boyle 自跑 `npm run build` exit 0（ReadingPage chunk 54.38 kB，既有提示）。

**结论：** 阶段 4 通过验收。剩余为用户真实窗口视觉终验（阴影层次、宋体字重、过渡手感、窄屏 overlay、浮动条 hover/active）；提交与推送待用户终验后另行授权。

## OD-READ-002 阶段 5 复核（2026-11-19，ARCHITECT 独立复核：通过）

**范围：** 健脑室仪表盘落地（空态 + 有数据态），按定稿样稿 reading-dashboard-v2.html。

**源码事实：**
1. 新增 `ReadingDashboard.tsx`（496 行）：顶部当前书籍封面+进度（封面取 `meta.cover`，缺省首字/图标占位）、1000 分钟目标（估算 = 本月书籍 Σ(char×progress)/300，标注「估算」）、进度条无黑色刻度针；五格数据顺序为 阅读字数（K 格式化）→ 今年已读（本年 progress≥1）→ 批注 → 订阅文章（含「N 读完」）→ 整理笔记；无历史基线的涨跌小字不显示（未虚构）。
2. 下部分栏：左列最近 4 本已读书籍（封面/书名/格式·章数·日期/作者「著」/日期/100% 环），头部仅「+ 导入书籍」「查看全部」右对齐相邻；右列「喜欢的 RSS 订阅」花瓣图两层（固定底色层 + 数据层按本月已读占比），标签在花瓣外侧；本月无已读时给空态说明。
3. 空态：两张引导卡（导入第一本书 / 订阅第一篇资讯）+ 9 位精选作者 chips（新增 `rss_preset_sources` 命令，返回内置精选清单，带单测）。
4. 接线：导入复用文件框、查看全部展开书架、书籍行打开阅读器、自定义 RSS/管理订阅复用现有弹窗、chip 点击即订阅。
5. 样式 `.reading-dash-*` 落 reading.css，复用 --rd-* 变量。

**ARCHITECT 实跑：**
- `cargo test --offline --lib -- --test-threads=1`（OMNIDESK_REPO_ROOT 指向主仓库）：**1250 passed / 0 failed / 1 ignored**。
- `npx tsc --noEmit`：exit 0。
- `verify_reading_006_dashboard.py` 34/34、`verify_reading_005_design.py` 34/34、`verify_reading_003_selection.py` 25/25、`verify_reading_004_formats.py` 19/19、`verify_od_read_002_stage3.py` 18/18。
- Boyle 自跑 `npm run build` exit 0（ReadingPage chunk 64.92 kB，既有提示）。

**结论：** 阶段 5 通过验收。剩余为用户真实窗口视觉终验与交互点击验证；提交与推送待用户终验后授权。
## OD-READ-002 阶段 6–10 复核（2026-11-20，ARCHITECT 独立复核：通过）

**范围：** 用户真机反馈的五轮修复：书架显隐与书籍删除、封面与正文图片、阅读区视觉、仪表盘细节、顶部并排、RSS 旧占位回填、中层凸起卡、视频媒体、侧栏排版、RSS 收藏与 30 分钟自动刷新、书籍改名、来源随机展开、头栏星标。

**源码事实（逐轮核对 diff）：**
- 阶段 6：书架仅在书籍打开时渲染；`reading_delete_book` 删除正文/摘要/进度/笔记并保留导出；封面缺失按需回填；`reading_remote_media` 远程图片；完成阈值 0.995。
- 阶段 7：真实阅读时长 `daily_seconds` + `reading_time_add`；`month_minutes`/`streak_days`；EPUB `dc:creator`；`reading_refresh_content` 旧正文媒体回填（quote 重定位）；RSS enclosure/media:content 与旧文章回填。
- 阶段 8：顶部两模块全宽度并排；相对图片地址按条目 link 解析；`rss_refresh_article` 单篇回填。
- 阶段 9：RSS/EPUB 正文先显示、后台回填；`rss_refresh_article` 走 spawn_blocking；中层数据区最终改为凸起卡（分割线移除）。
- 阶段 10：侧栏字号 +2 与 16px/700 标题；移除订阅管理/拉取全部，30 分钟自动刷新；30 天过滤 + `favorite` 收藏；书籍改名；来源默认折叠并随机展开一个；头栏加粗与星标收藏。
- 媒体白名单含 AVIF 与视频/音频，单文件上限 32 MiB；媒体/封面/回填命令均为 async + spawn_blocking。

**ARCHITECT 实跑（最终轮）：**
- `cargo test --offline --lib -- --test-threads=1`：**1264 passed / 0 failed / 1 ignored**。
- `npx tsc --noEmit`、`npm run build`：通过（仅既有 chunk 体积提示）。
- `verify_reading_003`–`011` 共九套静态核验全部通过（含 011 的 47 项）。
- `git diff --check`：干净。

**结论：** 阶段 6–10 通过验收，随 0.2.39 发版提交。剩余未验证项：Windows 真机与真实外部 CDN 视频/AVIF 端到端；由用户在 Windows 环境实测。

## OD-READ-002 阶段 11 复核（2026-11-20，ARCHITECT 独立复核：通过）

**范围：** 正文阅读设置（字体/字号/排列，仅正文）与 RSS 文末模块（附件、结束标记、预计阅读时间、下一篇）。

**源码事实：**
1. `ReadingPage.tsx` 新增 `ReaderSettings` 与 `READER_FONTS`（系统默认、思源黑体、思源宋体、霞鹜文楷）、三档排列、14–26 字号；设置只以 `--rd-reader-font/size/align` 写到 `.reading-page` 根，CSS 仅 `.reading-page .reading-paragraph` 消费，章节标题规则不含这些变量；未提供宽度/颜色控件；`Aa` 按钮 `order-last` 位于顶栏右侧，弹层支持恢复默认、外部点击与 Esc 关闭，设置持久化到 `localStorage`。
2. 字体依赖 `@fontsource-variable/noto-sans-sc`、`@fontsource-variable/noto-serif-sc`、`lxgw-wenkai-webfont` 在 `main.tsx` 自托管引入；三份 OFL 文本进入 `desktop/public/fonts/`，`THIRD-PARTY-NOTICES.md` 已增列。
3. `rss_store.rs` 新增 `RssAttachment` 与 `RssArticleMeta.attachments`（serde default）；`entry_attachments` 只解析 `enclosure`/`media:content` 的非图片媒体，仅 http(s)、上限 20、记录标题/MIME/字节数；旧文章在 ingest 时补写附件元数据，`refresh_article_with_xml_at` 同样更新元数据，正文/批注路径未改。
4. RSS 文末仅在 `source_format === "rss"` 渲染：附件井、打开原文、`完` 标记、400 字/分钟估算的阅读时间、同源更早文章的下一篇卡片；音频/视频附件需点击「播放」才触发 MediaBlock 与远程媒体读取，另提供「打开原文件」。本地书籍无文末模块。
5. `verify_reading_012_settings_and_footer.py` 43 项覆盖设置、字体、变量作用域、附件模型、旧元数据补写、文末条件渲染与样式；`verify_reading_005_design.py` 的旧固定字号断言已改为检查阅读设置变量。

**ARCHITECT 实跑：**
- `cargo test --offline --lib rss_store:: -- --test-threads=1`：23 passed / 0 failed（含新增附件解析与旧 meta 补写单测）。
- `cargo test --offline --lib -- --test-threads=1`：1288 passed / 0 failed / 1 ignored；同时修复材料只读拒绝文案（中性描述「路径包含被禁止片段…」，不再误写「写入」）。
- `npx tsc --noEmit`：exit 0；`npm run build`：exit 0（新增 3 套 CJK 字体子集，产物约 17 MB，仅既有 chunk 体积提示）。
- `verify_reading_012` 43/43；003 25/25、004 19/19、005 34/34、008 全过；006/007/009/010/011 的失败均由本工作区既有 OD-HARDEN-2043 重构（命令迁移、媒体上限 8 MiB、文案变化）造成。

**结论：** 阶段 11 通过验收。剩余未验证项为用户真实窗口的字体字形、弹层阴影、字号/对齐切换手感与真实 enclosure 播放；提交与推送待用户另行授权。

**提交后 CI 修正（2026-11-20）：** 首轮 Linux test job 失败两项，均与本轮功能无关但阻断发布：①`newest_session_file` 用 `as_secs()` 比较 mtime，同一秒内两张会话表相等，改为 `as_nanos()`（单测 `newest_session_prefers_latest_mtime`）；②附件禁区含宽泛片段 `omniharness`，GitHub runner 的仓库路径 `/home/runner/work/omniharness/omniharness` 使仓库内 AGENTS.md 被误拒，按既有「附件可读仓库文档」的契约从附件禁区移除该项（DSH/agent 数据仍由 `.dsh`/`deepseek-harness` 等拦截）。第二轮暴露 Node 22 不接受 `--test src` 目录参数，`npm test` 改为 `node --experimental-strip-types --test` 让其自行递归发现 `.test.mjs`；第三轮暴露 `workIntent.test.mjs` 中三个测试误嵌套在父测试内，Node 22 在父测试结束后取消子测试，已补上父测试的收尾并移除多余括号。Node 22.23.4 与本机均 88/88。本机全量复跑 1288 passed / 0 failed / 1 ignored。

## OD-HOME-0245 复核（2026-11-20，ARCHITECT 实施并复核）：通过（脚本/静态范围）

**范围：** 首页阅读统计显隐、当日摘要下线、引擎状态文案、顶栏通知与待办、周复盘确认入口。

**源码事实：**
1. `HomeCanvas` 的阅读统计槽位包在 `conversationStarted ? null` 内；有任一对话消息后不再渲染，空对话仍保留统计与标语。
2. 当日摘要从产品面完整移除：前端 `dailySummary.ts`、HomeChat 摘要上下文/确认/落盘、WorkRecordLibrary 摘要入口/卡片、conversationPrompt daily-summary、workIntent/workIntentDecision 意图均删除；宿主 `daily_summary.rs` 与两条命令/注册删除；内置 Skill 清单、seed、样本 rules 解绑并删除 `skills/daily-summary`。历史摘要文件未删除。
3. 引擎状态新增 `engineLabels.ts`：`dsh-web-api · unknown · compatible` 显示为「DSH Web API · 版本未知 · 协议兼容」；`unknown` 只表示 web-api 不暴露版本，不等于不兼容。顶栏与 EngineTechSlice 共用该映射。
4. `NotificationCenter` 聚合 pending 待确认写入、到期/逾期/待确认任务、未完成任务总览与 `review_due`；徽标、空态、读取失败提示、点击跳转齐全；TopNav 未实现占位移除，App 传入 `selectScene`。
5. `ReviewArchive` 草稿态新增「确认归档」按钮，调用 `worklogPeriodReviewConfirm` 后重读详情并刷新列表；确认后锁定不可覆盖；重新生成仍走既有 `.bak` 备份与草稿覆盖路径。
6. `verify_home_work_0244.py` 31 项覆盖上述 UI/代码事实；`verify_hardening_2043.py` 的命令基线增加「已下线两条摘要命令」的显式允许集合，其余命令仍一一对应。

**ARCHITECT 实跑：**
- `cargo test --offline --lib -- --test-threads=1`：1276 passed / 0 failed / 1 ignored（删除 12 项随功能下线的摘要测试）。
- `npx tsc --noEmit`、`npm run build`、`npm test` 88/88；Node 22.23.4 亦 88/88。
- `verify_home_work_0244.py` 31/31、`verify_hardening_2043.py` 77/77、`verify_reading_004_formats.py` 19/19、`verify_reading_entity_2043.py` 7/7。

**结论：** 本轮微调通过验收。剩余未验证：真实窗口通知弹层的密度与计数、真实任务/待确认数据、复盘确认后列表刷新的手感；未提交、未推送。

## OD-PERF-0245 复核（2026-11-20，ARCHITECT 实施并复核）：通过（单元/静态范围）

**范围：** 周复盘 278 秒的引擎重启、编码冲突、getpgrp 失败与分段计时。

**源码事实：**
1. `engine_describe_if_running` 只返回已初始化描述，不触达 `ensure_initialized`；App 首探与轮询走它，只在启动后单次 `engine_describe` 预热；失败不在 5 秒轮询里重试。
2. 编码统一覆盖四层：宿主 `child_env` 固定 `PYTHONUTF8`/`PYTHONIOENCODING`；Python `serve.main` 重配 stdin/stdout；DSH `Popen` 显式 `encoding=utf-8, errors=replace`；Rust stdout/stderr 线程按字节读取并容错解码。
3. `current_pgid` 非 POSIX 返回 0，`AttributeError`/`OSError` 回退 0；`ensure_process_group` 仍只在 POSIX 调用 setsid。
4. 消息新增 `timing.classifyMs/materialMs/submitMs`；路由提交使用用户提交时刻，总耗时计算不再从任务回执时刻起算；界面并列显示判定/素材/提交、首字与总耗时。
5. `verify_review_perf_0245.py` 21 项覆盖只读探测、编码、getpgrp、分段计时；`stderr_reader_survives_non_utf8_bytes` 与 process_owner 2 项单测覆盖坏字节和 OSError。

**ARCHITECT 实跑：**
- `cargo test --offline --lib -- --test-threads=1`：1277 passed / 0 failed / 1 ignored。
- `npm test` 88/88；`npm run build` 通过；Node 22 下 88/88。
- process_owner 单测 18/18；web-api 单测 105/105；`verify_review_perf_0245.py` 21/21、`verify_home_work_0244.py` 31/31。

**结论：** 性能修复通过本机验证。150 秒目标必须靠 Windows 实机复测确认；分段计时可定位剩余耗时落在判定、素材、提交/排队、首字还是生成。未提交、未推送。

**评估范围：** HEAD `a1388be6fe`（0.2.48），涵盖 0.2.46–0.2.48 三个提交（DSH 0.2.2-rc.5 升级、运行时精简、Windows 进程收尾与插件依赖修复）。

**ARCHITECT 实跑（本机 macOS 27.2 / Node 26.13.0）：**
- `cargo test --lib --offline -- --test-threads=1`：**1279 passed / 0 failed / 1 ignored**（83.42s；与 STATUS.md 记录一致）。
- `cd desktop && npm run test`：**88/88 通过**（15.05s）。
- `cd desktop && npx tsc --noEmit`：EXIT=0。
- `dsh --version`（仓库 runtime，剪枝后）→ `0.2.2-rc.5`；`dsh plugin --profile web list` → EXIT=0。
- `python3 scripts/diagnose-omnidesk.py`：引擎未运行，模型路由元数据正常（open-design 端点、discipline Profile）。
- `bash scripts/omnidesk-isolation-check.sh`：**15 通过 / 22 失败**（见问题 P1）。

## 评估结论（按严重程度）

### P1（高，功能阻断 · 已定位根因）：隔离自检与开发脚本在 macOS 27 上全军覆没
**事实：** `scripts/omnidesk-dev.sh --check`、`scripts/sync-skills.sh --check/--dry-run`、`omnidesk-isolation-check.sh` 均在 `realpath -m --` 处以 `illegal option -- m` 退出 1。隔离自检 22 项失败全部由此级联（拒收路径断言、dry-run 断言、sync 断言全部因脚本提前退出而判 FAIL）。
**根因：** macOS 27.2 更新了 `/bin/realpath`（2026-10-23 构建），移除了 BSD 时代曾支持（或第三方 coreutils 提供）的 `-m` 选项，只保留 `[-q] [path...]`。两个脚本的 `abspath()` 把 `command -v realpath` 命中当作「支持 GNU 选项」，但本机无 GNU coreutils（`/opt/homebrew/opt/coreutils` 不存在），因此 fallback 永远不触发。此问题在 macOS 26.x 上不存在，属系统升级引入的兼容回归，与 0.2.46–0.2.48 三个提交无关。
**影响：** 开发者无法用 `--check`/隔离自检验证隔离边界；`sync-skills.sh` 完全不可用。桌面 App 本体不受影响（Rust 侧用自家 abspath，不依赖 shell realpath）。
**修复方向（待批准）：** 两处 `abspath()` 改为不依赖 GNU 选项的实现——`cd \"$(dirname \"$1\")\" && printf '%s/%s' \"$(pwd -P)\" \"$(basename \"$1\")\"`，或直接 `python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))'`。改动面只有 `scripts/omnidesk-dev.sh:66` 与 `scripts/sync-skills.sh:62` 两处函数体。

### P2（中，一致性遗留）：`profile_assets.rs` 的 DISCIPLINE_PLUGINS 仍声明 `@deepseek-ai/dsh-skill-office` 为必需插件
**事实：** 0.2.47 已从 `samples/profiles/discipline.profile.patch.yml` 删除 skill-office insert（用户隔离根的旧 patch 里还残留一条），且运行时已剪枝 `dsh-skill-office`；但 `profile_assets.rs:78-82` 的 `DISCIPLINE_PLUGINS` 仍把它列为 discipline Profile 的必需插件。
**影响：** 纪检专业 Profile 的资产页会持续显示该插件「缺失」，用户可点「安装」装回（dsh plugin add 走 pnpm、从 npm 源装到 profile 目录，不在剪枝范围），与策略 B「office 工具随包精简」的决定冲突；且用户隔离根的旧 patch 仍引用被剪的 `dsh-tool-schedule`/`dsh-tool-subagent-control`/`dsh-plugin-market/tools`（后两个已 disabled，前一个未禁用），每次启动产生 67 条导入告警中的相当部分。
**修复方向（待批准）：** 从 `DISCIPLINE_PLUGINS` 移除 `dsh-skill-office`（保留 dsh-cost-meter）；同步清理用户隔离根三份旧 patch 中的 `skill-office` insert 与 `tool-schedule` 行（须用户授权，涉及隔离根写入）。

### P3（低，文档漂移）：README 版本行停留在 0.2.29
**事实：** `README.md:76` 写「当前版本 0.2.29」，实际 VERSION/tauri.conf/package.json 均为 0.2.48。
**修复方向：** 一次文本更新。

### P4（低，文档漂移）：`docs/OmniDesk-Engine-Interface-Audit.md` 仍以 `dsh-v0.2.1-alpha.2` 为审计对象
**事实：** 与 RUNTIME.lock（0.2.2-rc.5）脱节；审计文档自 10 月起未随升级刷新。

### P5（低，测试卫生）：`cargo test` 有 3 条 warning
**事实：** `review_store.rs:1357` 未用变量 `now`、`review_store.rs:1298` 未用函数 `weekly_text`、`writing_section.rs:1096` 未读字段 `config`。

### 已验证正常的部分
- **0.2.48 Windows 修复本体**：`kill_pid` 的 Windows 分支（taskkill /T /F）、窗口 Destroyed 收尾（`stop_settings_service` + `cleanup_child_on_exit`）、`dsh plugin` 对 plugin-manager/plugin-detail 的依赖恢复均已在源码逐行核对，与本机实测（plugin list EXIT=0）一致；Windows 实机行为仍待用户验证（继续标记未验证）。
- **DSH 0.2.2-rc.5 适配面**：启动参数（--profile/--port/--no-open）、303+Set-Cookie 鉴权、session/credentials/llm RPC、插件 peer 版本（cost-meter 1.9.6 / free-search 0.9.5 要求 >=0.2.0-rc.1 <0.3.0）均核对兼容。
- **运行时精简的完整性**：用户可见产物（seed 三份 profile）已同步删除被剪包的引用；`samples/profiles/*.patch.yml` 无对被剪包的悬空启用引用。
- **Rust 侧安全边界**（net_guard、host_access、export_engine 授权门）经既有 1279 项测试覆盖通过。

**未验证（需要 Windows 实机或真实窗口）：** 0.2.48 的关闭后进程列表、设置页安装插件、专业 Profile 缺件提示消失；DSH 0.2.2 真实模型任务含工具调用；golden_tasks 40 项真实模型评测。

**下一轮建议（供批准）：** ①修 P1 两处 abspath 并重跑 isolation-check 全绿；②P2 删 DISCIPLINE_PLUGINS 里的 skill-office 声明；③P3/P4/P5 文档与 warning 清理。合计约 1 小时工作量，不涉及 Windows 与引擎协议改动。

---

# 0.2.47 复核（2026-11-22，ARCHITECT 实施并复核）——本机启动验证通过，Windows 实机待用户验证


## OD-GROW-301 ARCHITECT 复核（2026-11-23）

- **范围：** T1、G0、G1、G2、G3、H3、H5、G4、G5、G6、G10、H6+G11、H7+G12、T13；G8（写作差异）按 G0 结论属情形 2（AI 原稿未落盘），需另行方案与用户批准，本轮未实施；G9（可选）与 T14（拆 lib.rs 余下领域）未实施。未提交、未推送、未打标签。
- **ARCHITECT 复跑（script）：**
  - cargo：`cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib --offline -- --test-threads=1` → **1349 passed / 0 failed / 1 ignored**（基线 1292 → T1 +4 → G1 +11 → G2 +10 → G3 +2 → H3 +10 → G10 +5 → H7+G12 +15）。
  - npm：`cd desktop && npm run test` → **132 passed / 0 failed**（基线 97 → H5 +4 → G4 +4 → G5 +10 → G6 +9 → H6/G11 +5 → T13 +3）；`npx tsc --noEmit` → 0；`npm run build` → 0。
  - Python：`cd engine && python3 -m unittest discover -s omnidesk_engine -p habit_extract_test.py` → 4 tests OK。
  - 回归：`python3 tests/verification/desktop/run_habit_regression.py` → 16 PASS / 0 FAIL。
  - 既有门禁：`verify_hardening_2043.py` 77/77（见下方偏差 1）；`verify_home_work_0244.py` 31/31；`verify_conversation_prompt.py` 33/33；`verify_harness_220.py` 14/14。
- **偏差与处理：**
  1. verify_hardening_2043.py 的任务 D 基线校验原含 0.2.45 的 retired/added 差额（daily_summary 两条移除、engine_describe_if_running 新增）；G1 起基线改为「当前源码全集」口径重新生成后，双重加减导致 292≠291。ARCHITECT 将该脚本的 retired/added 集合改为空（命令名集合与基线一一对应的断言保留不变）。这是对既有校验脚本的记账性修正，非放松断言。
  2. G13 缺口（G4 记录）：宿主暂无 habit 输出校验/暂存命令，G4 只做提交→终态→读结果→记录 raw；G3 解析器待后续以命令暴露后接入整理链路。
  3. G6 统计落盘：宿主无统计追加命令，注入记录暂存 localStorage（≤50 行滚动）；正式统计待后续宿主命令。
  4. 搁置口径：G5 前端 idleDays >= 7 与 G10 宿主 > 7 存在一天差异（第 7 天前端算逾期、宿主不算）；已记录，后续统一到第 8 天。
- **未验证（需实机）：** 真实 Tauri 窗口界面（HabitDigestPanel/HabitCardsPage/HabitHomeNotice/铃铛条目）、真实模型整理端到端（G4）、复盘失败自动重试（H5）、真实数据根下的并发与断电恢复。
- **结论：** 已批准范围内的任务全部实施并通过 ARCHITECT 复跑验收；G8/G9/T14 留待后续批准。



## OD-GROW-301 G8/G9 ARCHITECT 复核（2026-11-23）

- **G8 写作差异（用户批准实施）：** writing_section.rs 确认路径加 ai_draft_text 可选参数（Option/serde default，向后兼容），提供时以 VersionSource::Ai 保存「AI 原稿」快照（label 用 ASCII ai-draft-section-<n>，中文在 note，一次确认版本号递增 2）；writing_diff.rs 纯函数逐章节差异（片段 ≤100 字符）；consolidation.rs 收集 writing_diff 来源（同一被删片段在 ≥2 个不同稿件出现才提，仍走 G3 校验与用户确认，不自动写入生效卡）。ARCHITECT 复跑 cargo → **1356 passed / 0 failed / 1 ignored**（+7）。前端接线缺口（WritingSectionEditor 传 ai_draft_text）记入 STATUS，属下一轮界面任务。
- **G9 范例进知识库（用户批准实施）：** knowledge_examples.rs 纯函数提取范例段（50–300 字、跳标题/列表、每章 ≤3 段），过 reject_keywords（命中跳段）+ mask_privacy 后写入 inbox/写作范例/ 源材料并调既有 knowledge_review::stage 进待审核队列；确认章节失败不影响主流程。未新增命令、未绕过审核队列。实施偏差：暂存放在命令包装层（confirm_section 拿不到数据根），lib.rs 改动约 15 行（超出任务书「只加模块声明」，原因与影响已记录）；来源路径改为真实存在的 inbox 文件（准入要求）。ARCHITECT 复跑 cargo → **1360 passed / 0 failed / 1 ignored**（+4）。
- **G12/H7+G12 复核接入缺口：** review_active_cards/audit_quote 纯逻辑已交付，未接入整理运行器调用点（下轮界面/联调任务）。
- **T14：** 用户裁定为下一步专项计划，等功能理顺后启动；本轮未实施。
- **未验证：** 真实 Tauri 窗口、真实模型、真实知识库端到端（用户将在 Windows 实机验证）。



## OD-PRJ-604 ARCHITECT 复核（2026-11-23）

- **范围：** Windows 实测六项修复（F1 完成状态机前置链、F2 写操作 onChanged 即时刷新、F3/F5 甘特双层条与项目汇总进度、名称层级、F6 复盘 76vw×76vh 居中弹窗）。UI 参考实调 screenshots-design-inspiration（Teamweek 甘特、Glassdoor 弹窗）。
- **ARCHITECT 复跑（script）：** npm test 150/150；tsc 0；build 0；verify_project_601 15/15；verify_project_602 18/18；verify_hardening_2043 77/77。
- **静态断言适配（ARCHITECT 亲自改，非放松）：** verify_project_601.py S2/S3 与 verify_project_602.py S1 的精确字符串 needle 按 OD-PRJ-604 获批 JSX 变化更新（多行三元、onChanged 挂载、rangeLabel 局部变量）；断言语义不变（状态前置链、双卡挂载、纯函数调用仍逐一断言）。
- **未验证：** 真实 Tauri 窗口六项交互由用户实机复测。

