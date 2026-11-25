# Changelog - OmniDesk (OmniHarness)

All notable changes to this project will be documented in this file.

---

## [0.2.59] - 2026-11-26

### Fixed
- 项目与任务弹层项目摘要改为单行布局：状态胶囊、日期区间和添加任务按钮同排，日期超宽时省略号截断并提供完整悬浮文本，不再折成两行撑破圆角条。

### Verification
- `npx tsc --noEmit` 0；`npm run test` 150 passed；`npm run build` 0。

## [0.2.58] - 2026-11-26

### Changed
- 项目卡片功能组移除「添加任务」按钮，添加任务统一在项目管理弹层内完成。
- 项目页图标按钮改用应用内自定义悬停提示，约 0.32 秒出现，不再等待原生 title 的约 1 秒气泡。
- 项目复盘弹层增加可见标题「项目复盘」，项目选择改为暖白自定义下拉，生成按钮增加图标；复盘查看弹层也补充「项目复盘」标识。
- 项目与任务弹层增加可见标题「项目与任务」，主项目选择改为同一套自定义下拉，图标按钮补充悬停说明。

### Verification
- `npm run test` 150 passed；`npx tsc --noEmit` 0；`npm run build` 0。
- `verify_project_601` 15/15、`verify_project_602` 18/18、`verify_project_ui_716` 9/9、`verify_task_quadrant_ui_402` 10/10、`verify_project_ui_512` 26/26。

## [0.2.57] - 2026-11-25

### Changed
- 首页空态恢复书法标语「坚持学习人生的有无限可能」，保留品牌语气。
- 项目跟踪页按验收稿重排：左侧为纵向四单元任务栏，默认展开「重要且紧急」「重要不紧急」；右侧上方甘特、下方项目卡片。
- 左侧栏宽度降到 288px，释放甘特空间；甘特项目轨移除进度条，进度只在项目卡片展示。
- 左侧任务行改为单行结构：状态圆点、重要/紧急图标、任务名、右对齐截止日或「逾期 N 天」。
- 甘特任务条的功能组移到条内右侧，编辑/删除图标在悬停时展开；项目卡片左下角功能组同理，卡片按宽度显示 3–4 个最近项目。
- 页头右上角「+」及扇形功能组移除；左上角三个状态按钮改为居中弹层，分别展示已完成、进行中、逾期任务。
- 底部已完成与项目维护模块移除，项目维护与复盘改由项目卡片功能组和弹层进入。
- 左侧栏底部新增项目跟进分类的历史会话模块，默认折叠，点击某条会话回首页继续对话。

### Verification
- `npm run test` 150 passed；`npx tsc --noEmit` 0；`npm run build` 0。
- `verify_project_601` 15/15、`verify_project_602` 18/18、`verify_project_ui_716` 7/7、`verify_task_quadrant_ui_402` 10/10。

## [0.2.56] - 2026-11-25

### Changed
- 全站 UI 收口到「极简板式 + 暖白新拟物」：项目跟踪、工作记录、文案写作、健脑室、历史会话、知识库、设置统一 22px 页头与 1280px 内容宽度；清理冷紫残留、白卡与描述性副标题，输入框和卡片改由同一块暖白材料凸起/凹陷。
- 项目跟踪页二轮重构：四象限与甘特同高拉伸，行高改为 1fr，所有任务无需内部滚动即可看到；任务卡改用状态圆点与日期图标，不再使用无稳定语义的关键词图标；甘特任务条删除状态文字，逾期改为斜纹 + 时钟符号；补齐项目完成度凹槽与暖橙填充。
- 页头功能组改为四个 34px 图标位：新建项目、新建任务、项目复盘、项目管理。悬停按 spring 曲线扇形展开，支持点击钉住、键盘 Tab 展开、Esc 或外部点击收回，并下移避开搜索框。
- 项目维护从底栏移入「管理项目和关联任务」弹窗，底栏收敛为已完成与项目复盘两张卡；删除项目目标句和任务行重复项目名，减少首屏文字与高度参差。
- 首页空态用浮起的灵感托替换书法标语；工作记录、文案写作、历史会话的空态改为图标；知识库统计去掉英文副标题与解释行。

### Verification
- `npm run test` 150 passed；`npx tsc --noEmit` 0；`npm run build` 0。
- `verify_project_601` 15/15、`verify_project_602` 18/18、`verify_project_ui_716` 7/7、`verify_task_quadrant_ui_402` 10/10。
- 独立设计复评 8.0/10（国际获奖门槛 8.0）；复评截图使用 1600×1000 合成演示数据的临时预览页，未含真实业务内容。

## [0.2.55] - 2026-11-24

### Added
- 习惯整理闭环（OD-0255）：新增宿主 consolidation_apply_output，校验模型整理输出后直接把候选卡与未决问题写入习惯存储；红线、编号、上限与去重全部复用 G1 校验，raw 原文不落盘。
- 新增 habit_stat_inject / habit_curator_review / habit_quote_audit 三个命令：注入统计落盘、生效卡确定性复核（可复核/冲突）、引用抽查结论写入 quote_audit 统计。

### Changed
- 整理运行器在任务终态后调用宿主校验与暂存，通知显示新增候选与未决问题数量；校验失败保留 raw 到本地并如实报告失败。运行器随后执行复核与引用抽查，失败只记日志。
- 章节确认可携带本次 AI 原稿：编辑器生成或修订章节时保留模型纯正文，确认时随 ai_draft_text 提交，宿主保存 AI 版本快照后再写用户确认版。
- 习惯页生效卡显示「可复核」「冲突」徽标；未决问题搁置超过 7 天（第 8 天起）才计入逾期，与宿主口径一致。

## [0.2.54] - 2026-11-23

### Fixed
- 项目跟踪页六项实测修复（OD-PRJ-604）：「标记完成」补全状态前置链（待开始/暂停 → 进行中 → 已完成，待确认 → 待开始 → 已完成），不再报非法状态迁移；项目维护区与复盘卡写操作后经 onChanged 即时刷新四象限、甘特与统计，新建项目/任务不再需要切页刷新。

### Changed
- 甘特图双层条：底层浅色计划条（created → due）+ 状态色进度填充（按时间比例，完成=100%）；项目汇总带同样双层，项目缺结束日默认按 90 天计划；项目名 13px 粗体与任务名 11.5px 分级，颜色分层。
- 项目复盘改为居中大弹窗（约 76vw × 76vh）：生成成功自动打开，正文内滚，底部承载确认归档与提炼知识卡；Esc 与点遮罩关闭；侧栏保留生成入口、状态徽标与「查看」。
- 新增 projectGanttProgress.ts 纯函数（任务/项目进度比、默认 90 天窗口）与 18 项单测；theme.css 新增 .od-gantt-plan-* / .od-gantt-progress-* 语义类。

## [0.2.53] - 2026-11-23

### Added
- 每日习惯整理（OD-GROW-301）：每天定点从未分类对话与项目任务收集摘要，隐私命中自动脱敏保留原意，禁入类进待审定队列；模型提取习惯候选与未决问题（omnidesk.habit-candidates.v1），宿主逐条校验（消息存在、信号词白名单、场景/全局判定、去重、红线），经整理卡确认后生效。
- 习惯存储：应用数据根 habits/ 目录（候选/生效/归档/未决/状态/待审定），8 个 habit_* 命令；生效卡 ≤80、候选 ≤100，七天未处理自动归档；驳回两次的同短语不再提出。
- 无人值守整理运行器：30 秒询问到点，空闲才跑，每天最多一次，失败记录不重试、次日补跑；失败进铃铛问题提示，不弹错误框。
- 界面：铃铛新增习惯条目（N 条候选、M 个未决问题，计入红标）；首页当天一次提醒条；整理卡面板（逐条接受/驳回/全部接受、范围切换、待审定批准/排除、未决问题搁置提醒）；习惯页（生效卡管理、整理开关默认关闭、时间与模型选择）。
- 生效卡注入对话提示词：最多 8 张、合计 600 字，最近确认优先，带「与当前指令冲突以当前指令为准」标注；开关关闭或列表为空不注入。
- 习惯检查：复盘确认与写作审阅路径读取生效卡三类规则（禁用短语/段落字数上限/必备短语），违规只记统计不拦截。
- 复盘失败自动重试一轮：模型未完成/校验失败/结果为空时追加原因重试一次，取消与准入拒绝不重试。
- 写作差异习惯来源（G8）：章节确认可保存 AI 原稿快照（VersionSource::Ai）；宿主逐章节确定性差异，同一片段在两份不同稿件被删才进入候选摘要（origin=writing_diff），仍走候选确认。
- 写作范例进知识库候选（G9）：定稿范例段（50–300 字）经红线+脱敏后进入知识审核待审队列（inbox/写作范例/），不绕过准入。
- 未决问题提及刷新：新一轮摘要与 open 问题字符二元组重叠 ≥0.34 刷新 last_seen；搁置超 7 天进提醒区。
- 引用抽查与确定性复核：习惯卡引用抽查（结果只进指标）；90 天无命中标「可复核」、同范围矛盾卡标「冲突」，只提示不自动改删。
- 习惯回归入口 tests/verification/desktop/run_habit_regression.py（改动提取提示词/白名单/校验逻辑前必跑）。

### Changed
- engine_submit_task 工作目录缺省时由宿主用本地数据根决定（T1）。
- 复盘周期关键词匹配改为最长命中优先并可注入词表（T13 续做）。
- 命令基线更新为当前源码全集口径（292 条，含新增 habit/consolidation 命令）。

## [0.2.52] - 2026-11-23

### Added
- 项目跟踪甘特图改为项目分组行：每个项目一个可折叠组头（项目名、任务数、汇总区间）与浅色汇总带，组内任务一行一条状态色条，重叠/接续/间隔时间自然呈现；未挂接任务归「未挂接项目」组；总行数超过 30 自动折叠部分组并提示；任务行点击进入既有编辑弹窗。
- 新增 `projectGanttGroups.ts` 分组纯函数与 9 项单测（重叠/接续/间隔/未挂接/空项目/夹取/窗口外/折叠阈值）。

### Changed
- 项目跟踪页令牌化重构：甘特状态条六色移入 theme.css `.od-task-bar--*` 语义类；弹窗、按钮、卡片内写死的靛蓝渐变与绿/灰色值全部替换为 `--od-neu-*` 令牌，布局与四象限 compact 形态不变。
- ScenePage 清除旧事项表（project-items.v1.json）前端死代码，宿主命令与既有用户文件保留。

## [0.2.51] - 2026-11-23

### Added
- 项目跟踪页四象限：任务弹窗新增「标记完成」，二次确认后执行状态迁移（待确认/暂停按宿主状态机先转中间态）；已完成与已取消任务不再显示在四象限，只进「已完成」列表。
- 项目复盘：任务完成后可汇集项目任务上下文交给模型生成复盘，存档 `work/复盘/项目/<project_id>.md`（草稿可覆盖留 .bak、确认后锁定），支持一键提炼知识卡进知识审核队列。
- 项目维护区：右侧卡片下方新增项目卡片——新建项目、项目下拉、项目任务分页列表（每页 8 条）、挂接现有任务或新建任务、换绑/摘除、删除任务（确认后进数据根垃圾桶）。项目文件保持 `work/项目/<id>.md` YAML frontmatter，Obsidian 可读。

### Changed
- 新增宿主模块 `project_review_store.rs` 与 `project_review_read/save/confirm` 三个命令；项目复盘正文校验五个固定二级标题与 `#t-…` 引用编号。

## [0.2.50] - 2026-11-22

### Added
- 工作记录页主入口改为「记录今天工作」主按钮，点击展开表单并聚焦正文。
- 草稿单槽：同一天勾选草稿再次保存时覆盖同一条草稿记录，不新增；取消草稿后转为正式条目，后续草稿不再覆盖。
- 工作记录页正文统一使用自托管思源黑体，正文 14px。

## [0.2.49] - 2026-11-22

### Fixed
- 历史会话页黑屏：WebView 缓存只存线程元数据（0.2.44 起），缓存中「数据根没有对应 id」的遗留条目作为内存列表补充项时缺少 messages 字段，历史会话页读取消息条数时抛出 TypeError，React 无错误边界导致整窗黑屏。缓存补充项现在统一补齐 title/tag/messages 字段；历史会话页对消息字段读取全部改为容错。
- 界面崩溃兜底：新增全局错误边界，页面异常时显示可读错误与「重新加载界面」入口，不再整窗黑屏。

## [0.2.48] - 2026-11-22

### Fixed
- Windows 关闭应用后遗留 node 进程：设置服务的 kill_pid 原来调用不存在的 kill 命令（空操作），改用 taskkill /T /F；窗口关闭时也停掉设置服务并清扫孤儿 DSH node。
- Windows 设置页安装插件报 package_json_reader 栈：运行时精简误删了 dsh plugin 命令依赖的 plugin-manager/plugin-detail，已恢复并调整剪枝脚本；专业 Profile 的缺件提示随之消失。

## [0.2.47] - 2026-11-22

### Changed
- 运行时二次精简（OD-SLIM-022 策略 B）：移出 LibreOffice kits、office-kit/tool-office/skill-office、subagent 与 AgentTeam、DSH schedule、plugin-manager/plugin-market，以及 0.2.2 新增的 CUA、会话导入、推荐卡片、terminal-panel/action-cards/plugin-detail/mcp-manager 等未使用模块。
- profile patch 同步删除 skill-office、tool-schedule、plugin-market、subagent 相关启用条目；dsh web 冷启动与公告验证通过。
- 运行时 node_modules 由约 1.1 GB 降到约 315 MB（本机 prepare 后实测）。

### Known
- 移除后 DSH Web UI 的插件管理页与 office 工具、子代理、DSH 调度不可用；OmniDesk 自有功能不受影响。

## [0.2.46] - 2026-11-21

### Changed
- DSH 运行时升级到 0.2.2-rc.5（commit 9c2670d3）；Node 最低要求同步提高到 24.9，README、发布说明、bootstrap 与构建脚本一致更新。
- 订阅源单次抓取上限从 2 MiB 提到 8 MiB，避免大 feed 被误判超限。
- 订阅源重定向允许同一主机的 https→http（老博客常见），跨主机降级仍然拒绝。
- 远程图片接受任意 image/*、audio/*、video/*；Content-Type 不明确时按文件头识别；图片地址实际返回网页时回退读取 og:image。

### Fixed
- RSS 阅读页图片提示「响应不是支持的媒体类型：text/html」后无法显示。

## [0.2.45] - 2026-11-20

### Changed
- 首页阅读统计条只在空白对话显示；有对话后与空对话标语一起隐藏，不再占用版面。
- 顶栏引擎状态改为中文可读文案（DSH Web API / 协议兼容），不再直接显示 `unknown · compatible`。
- 引擎不暴露版本时直接省略版本段，只显示「DSH Web API · 协议兼容」。
- 顶栏「通知与待办」实装：聚合待确认写入、到期/逾期/待确认任务、未完成任务总览与到点复盘，点击跳到对应场景。
- 周期复盘草稿新增「确认归档」；确认后锁定不可覆盖，重新生成继续备份旧草稿。
- 引擎描述轮询改为只读，不再每次探测都启动引擎；启动后仅预热一次，避免初始化失败时每 5 秒重启约 60 次。
- Windows 引擎协议与 DSH 子进程输出统一 UTF-8（`PYTHONUTF8` / `PYTHONIOENCODING`、`sys.stdin/stdout` 重配、Popen 显式解码）；Rust 读取线程改为按字节读取 + 容错解码，单个坏字节不再终止线程。
- `getpgrp` 在非 POSIX 或调用失败时回退为 0，不再让初始化失败。
- 每轮对话显示「判定 / 素材 / 提交」分段耗时；原有总耗时与首字时间保留，可量化素材汇集、引擎启动/排队与模型生成各占多少。

### Removed
- 当日摘要功能与相关代码整体下线（前后端命令、提示词模式、意图判定、入口与 Skill）；磁盘上已有摘要文件保留不删。

### Verification
- `cargo test --offline --lib -- --test-threads=1`：1276 通过 / 0 失败 / 1 忽略；`npx tsc --noEmit`、`npm run build`、`npm test`（Node 22）通过。
- `verify_home_work_0244.py` 31/31、`verify_hardening_2043.py` 77/77、`verify_reading_004_formats.py` 19/19、`verify_reading_entity_2043.py` 7/7。

## [0.2.44] - 2026-11-20

### Added
- 健脑室正文阅读设置：顶栏 `Aa` 弹层可选系统默认 / 思源黑体 / 思源宋体 / 霞鹜文楷、字号 14–26、左/两端/右对齐；设置本地记住，只作用于正文段落。三款中文字体随包自托管（SIL OFL 1.1），运行时无外链。
- RSS 文末模块：附件（音频、视频、PDF 等 `enclosure` / `media:content`）、打开原文、`完` 标记、按字数估算的阅读时间、同源下一篇标题；音频/视频点播放后才加载。本地书籍不显示该模块。
- 发布门禁：`release.yml` 增加 Linux 测试 job（Rust 单测、前端构建与测试、Python 验证），PR 只跑测试不构建；发布 job 生成安装包 SHA-256 校验和。

### Changed
- 本地文件选择改由宿主登记授权：移除前端 `plugin-dialog` 直开，八个读写命令统一走 30 分钟有效的授权注册表；拖拽路径同样登记。
- 订阅源与远程媒体统一 URL 校验：拒绝回环、私网、CGNAT、链路本地与协议降级，逐跳固定解析结果；订阅源最多 3 跳，媒体统一 8 MiB 上限。
- 新增 `net_guard`、`host_access` 与 `commands/reading.rs` 切片；阅读/RSS 11 条命令从 `lib.rs` 迁出，命令名与注册清单保持一致。
- DSH runtime、三个 profile 与 Python 依赖改为 lock 文件安装，pnpm 固定 `11.28.11`。
- 材料读取的禁区文案改为中性描述，不再把只读拒绝误写为“写入”。
- 诊断会话按纳秒级 mtime 取最新，避免同一秒内写入时误选旧会话；附件读取不再因仓库目录名 `omniharness` 误拒仓库内文档。

### Verification
- `cargo test --offline --lib -- --test-threads=1`：1288 通过 / 0 失败 / 1 忽略。
- `npx tsc --noEmit`、`npm run build`、`npm run test`（88 项）通过。
- `verify_hardening_2043.py` 77/77、`verify_reading_entity_2043.py` 7/7、`verify_reading_004_formats.py` 19/19、`verify_reading_012_settings_and_footer.py` 43/43；`verify_reading_003` 25/25、`004` 19/19、`005` 34/34、`008` 全过。
- 真实窗口字体字形、弹层手感与真实 enclosure 播放待 Windows/macOS 实机复测。

## [0.2.43] - 2026-11-20

### Added
- 健脑室（原「读书」）融合 RSS 订阅：新增仪表盘（当前书进度、1000 分钟月目标、五格数据、已读书籍与订阅花瓣图）、书架双页签、导入/改名/删除、订阅管理与收藏。
- RSS/EPUB 正文富媒体：图片、音频、视频，EPUB 包内媒体按需提取，RSS 远程媒体由宿主拉取（含 AVIF/MP4 等类型，上限 32 MiB）；旧正文与旧占位图可自动回填。
- 右侧笔记/AI 面板新拟物改版：默认折叠图标栏、划线动作条自动展开、90 秒无操作自动收起；书架与面板此消彼长，阅读区保持可读宽度。

### Changed
- 正文宋体阅读、行距 35px、琥珀划线、引文井与媒体内凹窗；划线浮动条改为深色 raised 胶囊。
- 进入健脑室时距上次 RSS 抓取超过 30 分钟自动刷新；超过 30 天且未收藏的文章不再显示；订阅源默认折叠并随机展开一个。
- 阅读时长改为真实计时（可见且聚焦时累计），仪表盘「本月分钟」「连续天数」来自真实记录。

### Verification
- `cargo test --offline --lib` 1264 passed；`npx tsc --noEmit`、`npm run build` 通过；`verify_reading_003`–`011` 全部通过。

## [0.2.42] - 2026-11-19

### Changed
- 知识库暂存卡改为先进入「知识审核」：review 页新增「暂存卡待审核」区，列出标题、类型、字数与正文预览；支持勾选后「入库选中」「丢弃选中」，全部入库需二次确认。落库成功后删除对应暂存文件，并追加逐卡治理记录。
- 「整理材料」弹窗里的「同步到知识库」改为「全部入库」，必须先确认。

### Verification
- `cargo test --offline --lib`：1250 通过 / 0 失败 / 1 忽略（新增暂存列表、选中同步、丢弃与路径穿越单测）。
- `npx tsc --noEmit`、`npm run build` 通过；`verify_knowledge_staging_202.py` 14/14。

## [0.2.41] - 2026-11-19

### Fixed
- 审批弹窗不出现：工具在子代理或其它 agent 会话发起提权时，审批帧 `agentId` 与任务的 `engine_session_id` 不一致，适配层原先静默丢弃请求，界面看不到审批卡、引擎按取消收尾。现改为「精确匹配 → 唯一活跃任务 → 可见提示」三级归属，多任务歧义时也会显示「引擎在等待确认」提示，不再无反馈。
- 知识卡暂存路径：明确父目录 workspace/staging 是历史错误路径（沙箱外且不会被宿主同步），减少无效提权尝试。

### Verification
- 适配层单元测试 102/102（新增唯一活跃归属与歧义提示用例）；`verify_knowledge_sync_152.py` 48/48。

## [0.2.40] - 2026-11-19

### Added
- 首页阅读统计：书籍本数、已读完（进度 ≥95%）、阅读字数、批注、笔记、闪记（想法批注）与订阅文章篇数，聚合自本机健脑室数据，点击进入健脑室；统计条位于对话框下方。
- 健脑室左栏改为「本地书籍 / 订阅文章」两页签：书籍展示 EPUB 封面（无封面用书名首字占位）与进度；订阅文章按来源分组折叠，保留未读点。
- 阅读区支持图片、音频、视频与段落内行内 Markdown；EPUB 内嵌媒体经本地命令按需读取，RSS 图片直接加载。
- 阅读导入原生支持 MOBI（PalmDOC 压缩、HTML 转文本、EXTH 封面）与 PDF（随包 pypdf 逐页提取文字）。

### Fixed
- 阅读界面选中文字后不出现划线/笔记浮条：修复段落锚点属性缺失，并增加 selectionchange 兜底通道。
- PDF 导入在发布包中因缺少 pypdf 而退化为「正文未提取」。

### Changed
- 取消首页「项目跟踪」快捷入口；对话区下移并与统计条留出间距。
- 清理 integrity 未使用变量与 export_engine stderr 字段告警。

### Verification
- `cargo test --offline --lib`：1249 通过 / 0 失败 / 1 忽略（含 MOBI 导入链路单测）。
- `npx tsc --noEmit`、`npm run build` 通过；`verify_reading_003_selection.py` 25/25、`verify_reading_004_formats.py` 19/19。
- 合成 EPUB/MOBI 与最小 PDF 端到端通过；真实用户书籍、扫描件 PDF 与 Windows 实机仍待验收。

## [0.2.39] - 2026-11-18

### Added
- 健脑室：读书页更名为「健脑室」，书架分「书籍」「订阅文章」两组；新增 RSS/Atom 订阅管理（添加、删除、刷新全部）与 49 条预置源（9 位精选作者 + 40 条独立博客，MIT 归属）。
- RSS 文章复用既有阅读器与划线/想法/标段/问 AI 浮条，AI 走既有概念解释与批注整理链路；订阅与文章只落本机 `~/OmniDesk/reading/`。

### Fixed
- Windows 关闭应用后残留 DSH node 进程：引擎与宿主补齐 Windows 进程树回收（taskkill 树杀、owner.json 记录 child_pid、启动前按命令行指纹清扫死父进程遗留、退出兜底），不再只依赖 Job Object。
- 知识卡 staging 路径歧义：skill 改为以引擎当前工作目录为基准写 `staging/kb/`，避免模型误写 `workspace` 父目录导致建卡失败。
- 输入侧脱敏补强：新增金额、名单式人名、无后缀企业名规则与常见词防误遮；对话两条路径统一显示「已自动脱敏（类别 处数）」；修正 0.2.33 起失效的 selectModel 降级单测。

### Changed
- 10 个 Skill 按统一规范对齐（触发与排除、依据规则、步骤、输出契约与质量清单、失败处理六段结构），同步更新相关 verify 脚本与注入断言。

### Verification
- `cargo test --offline --lib`：1232 通过 / 0 失败 / 1 忽略（含 rss_store 11 项）。
- `npx tsc --noEmit`、`npm run build` 通过；`verify_od_read_002_stage1.py` 42 项全过；`verify_process_platform_133.py` 28/28。
- Windows 专属行为需实机复验：开关两次后 `Get-CimInstance` 过滤 `dsh-runtime` 残留应为 0。

## [0.2.38] - 2026-11-18

### Changed
- 读书页右侧「笔记 / AI」面板改为可调宽度：拖动左边缘在 300–640px 之间调整，双击在 340/560px 之间切换；宽屏默认展开，窄屏默认收起，顶栏「笔记」按钮可随时开合。
- AI 结果仍限制在面板内滚动，不再把阅读区挤成一条窄缝。

### Verification
- `npx tsc --noEmit`、`npm run build` 通过；`verify_reading_001_fixes.py` 11/11（含面板拖拽/开合静态断言）。

## [0.2.37] - 2026-11-18

### Fixed
- 读书导入 EPUB 时，XHTML 的 `<!DOCTYPE ...>` 与只含 `<div>` 的章节正文会被误判为「未提取到正文」；现在剥离 DOCTYPE、拒绝内部实体，并对无 `<p>` 的 div 正文与章节名做兜底解析。
- Windows 下调用 Python 抽取 EPUB/PDF/DOCX 时会闪出控制台黑框；子进程现在使用 `CREATE_NO_WINDOW`。
- 正文选区工具栏改为 document 级 mouseup + 键盘 selection 兜底，并避开顶栏；划线/标记/想法保存后自动打开右侧「笔记」，不再只剩一个消失的浮条。
- AI 阅读助手遇到 `getaddrinfo/ENOTFOUND` 时给出明确提示（检查网络、代理或设置中的服务商地址），不再只显示引擎原始错误。

### Verification
- `cargo test --lib -- --test-threads=1`：1232 passed / 0 failed / 1 ignored。
- `npx tsc --noEmit`、`npm run build` 通过；`verify_reading_001_fixes.py` 9/9（合成 EPUB 的 DOCTYPE/div 正文、Windows 无控制台、选区监听与 AI 提示断言）。

## [0.2.36] - 2026-11-18

### Added
- 读书阅读笔记分页（OD-READ-001）：Dock 新增平台内置「读书」入口，左侧书架、中间阅读器、右侧笔记与 AI 面板。
- 阅读时选中文字可划线、写思考碎片、标记当前段落、问 AI；批注按「章节 + 段落 + 字符偏移 + 引文」锚定，侧栏可回到原文、编辑、删除。
- 支持导入用户显式选择的 `.md/.txt/.pdf/.docx/.epub`（上限 32 MiB）：纯文本按 UTF-8 / UTF-16 / GB18030 读取，EPUB 用标准库解析 container/OPF/spine 章节，PDF/DOCX 复用既有抽取链路。
- AI 可解释选中文字、随时回答概念问题，也可把本书的划线/标记/想法整理成 Markdown 草稿；
  草稿经确定性引文核验（逐条比对本书批注与原文），未核验条目在保存稿顶部插入提示块，不阻断保存。

### Changed
- 保存的整理稿在无 frontmatter 时自动补 `title/created/book/source/annotation_count` 五字段，已有 frontmatter 原样保留；
  AI 输出被整段 Markdown 围栏包裹时自动剥一层。保存回执显示「已核验引文 x 条 / 未核验 y 条」。
- 阅读进度单独落盘（`progress/<book_id>.json`），滚动时不再重写整本书的大 JSON。

### Notes
- 书目、批注、进度与整理稿只保存在仓库外 `~/OmniDesk/reading/`；导入与 AI 任务不改动工作记录、知识库或成果目录；
  重复导入同一正文按 SHA-256 返回已有书，不覆盖。
- 本版一并包含 0.2.35 的逐模型思考档位手动声明（0.2.35 因 macOS 构建未获得 runner 而未发布）。

### Verification
- `cargo test --lib -- --test-threads=1`：1232 passed / 0 failed / 1 ignored。
- `npx tsc --noEmit`、`npm run build` 通过；`node --experimental-strip-types --test src/readingAi.test.mjs` 5/5；`git diff --check` 0。
- 真实桌面窗口交互与真实模型输出质量尚未验收，属日常使用验证项。

## [0.2.32] - 2026-11-17

### Added
- 工作记录场景自然语言意图重构（OD-ARCH-176）：每轮先由模型按统一意图契约分类（事实 / 指令 / 闲聊 / 复盘 / 周报 / 当日摘要），只有事实才落盘；指令与闲聊转为对话回复并带本周记录索引；复盘请求在工作记录场景内直接汇集材料出稿（OD-WR-505）；拿不准走提问卡确认。陈述句启发式快路径退出该场景，周报场景关键词快速通道不变。
- 模型运行期思考档位自动回退（OD-ARCH-175）：未声明档位的模型其显式档位被引擎运行期拒绝时，宿主自动重写覆盖文件去掉档位、重启引擎并提示重发；同一 (路由, 档位) 只触发一次；切换前的阻断式预检改为本地回退 + 一句话告知。
- 设置页「配套插件推荐」（OD-ARCH-177）：AnySearch CLI 与 dsh-cost-meter 的名称、上游地址、建议版本；仓库不打包插件实体，不自动安装、不联网检查版本。
- 工作记录场景思考档位策略：分类 / 分析 / 指令回复默认 off 模型直出（按会话覆盖，不重启引擎），用户手动选档即按所选；声明集合不含 off 的模型自动降级为跟随默认。

### Changed
- skills/period-review：工作台汇集材料随请求送达时直接起草，不再输出等待类话术；skills/anysearch：引擎环境无本机 CLI 属预期，直接回退 web_fetch 并如实标注来源与限制。
- 模型目录契约：`ModelCatalogEntry.efforts` 改为可选（归一化目录可能缺项）。

### Verification
- cargo test --lib：1209/0（并发运行偶现 3 个引擎 spawn 类环境性失败，单线程与复跑均不复现）。
- npx tsc --noEmit、npm run build 通过；verify_arch_176 70/70、model_route_precheck_175 12/12 + 11/11、companion_plugins_177 11/11、worklog 089 86/86 / 103 52/52 / 104 73/73 / 505 71/71、conversationPrompt 33/33、skill_bindings 69/69；home_refine_202 39/40（唯一失败为既有项）。
- 升级说明：已安装用户需在 Profile 资产页点「恢复默认规则包」获得更新后的 period-review / anysearch 技能文本。

## [0.2.35] - 2026-11-18

> 未发布：0.2.35 的 macOS 构建未获得 runner，相关改动随 0.2.36 发布。

### Added
- 逐模型思考档位手动声明（OD-ARCH-181）：设置 → 模型与端点中每个模型行下新增 7 档切换按钮（关闭/最低/低/中/高/很高/最高），off 按网关惯用取值 none；声明写入覆盖文件并优先于 Profile 与宿主已验证清单，真实调用不接受时取消即可回退。

### Changed
- 档位按钮尺寸收小约一半（字号 9px、紧凑内边距）。
- 对话模型选择器去掉「跟随模型默认」按钮：档位存在时直接列出声明的档位；档位为空时保留单按钮兜底。
- eco 模型保守声明「关闭」档（同族继承、待实测），其余模型仍由用户手动声明。

### Verification
- cargo test --lib 单线程 1218/0（含「用户声明覆盖 Profile 与清单」单测）；tsc / npm run build 通过；verify_arch_178 20/20 + 25/25、arch_176 71/71、202 39/40（既有失败）。
- 用户实机确认 0.2.34 档位生效；本版微调经本地构建目视。

## [0.2.34] - 2026-11-17

### Added
- 思考档位声明进发行链（OD-ARCH-178 阶段 A）：宿主已验证清单（opendesign / deepseek-v4.1-flash 的 7 档实测映射）并入 modelCatalog 与覆盖文件渲染，Profile 已声明不覆盖。
- 全部提交路径接入两层档位证据门（OD-ARCH-178 阶段 B）：普通对话 / 未分类场景纳入证据门，未声明模型不传档位、静默降级；api 层 gatedReasoningEffort 作为末道防线。
- ModelPicker 目录驱动（OD-ARCH-178 阶段 C，DSH 原版行为）：档位按钮按当前模型声明集合渲染，未声明模型只显示「跟随模型默认」。
- discipline preset persona 新增自查收敛规则（OD-ARCH-179）：OmniDesk 自查类问题先读 app/config 配置文件；bash 禁扫 storages/ 与 node_modules/。

### Changed
- 档位证据解析收敛到单一模块 modelEffortEvidence.ts（目录优先、Profile 兜底），界面渲染与提交门禁同口径。

### Verification
- cargo test --lib 单线程 1217/0；tsc / npm run build 通过；verify_arch_178 20+24、arch_176 71/71、model_route_precheck_175 12+12、companion_plugins_177 11/11、home_refine_202 39/40（既有失败）。
- 真实 DSH 双向验证：声明档位后 selectModel 接受 off 且任务成功（正向新增用例 PASS）；未声明模型传 off 优雅降级不炸任务（verify_effort_select_degrade 4/4）。

### 升级说明
- 已安装用户需在 Profile 资产页点「修复缺失 / 恢复出厂内容」获得档位声明与自查规则；升级后模型选择器只显示当前模型声明过的思考档位。

## [0.2.33] - 2026-11-17

### Fixed
- 工作记录场景每轮报 engine_unavailable 的线上回归（OD-ARCH-176）：真实 DSH 的 session/selectModel 会拒绝未声明思考档位模型的任何显式档位（含 off），此前「未声明即传 off」的设计使该场景所有消息必然失败，且错误被兜底映射成 engine_unavailable 误导排查。现在档位必须被两层证据「证明支持」才下发（未声明模型不传档位——本就没有思考可关，不传即等效直出）；适配器对 select_model 失败优雅降级（记 stderr、按模型默认继续，不炸整轮）；session/model-unavailable 错误改判 invalid_argument，呈现真实原因而非误报引擎不可用。

### Verification
- 新增真实 DSH 回归 verify_effort_select_degrade.py 4/4：selectModel 拒绝 → stderr 降级记录 → 任务按默认档成功完成。
- verify_arch_176 71/71（档位用例按「证明支持才下发」新语义更新）；cargo test --lib 1209/0（单线程）；tsc / npm run build 通过。

## [0.2.23] - 2026-11-12

### Added
- 工作记录模块 1.0：原始素材与正式记录分离、修订历史、当日摘要、周/月/季/年周期复盘、复盘档案、记录反链与「交给 AI 继续分析」。
- 工作记录分页新增右栏本月概览、一键本月复盘与四个快捷操作；记录详情新增关联任务管理。

### Changed
- 工作记录分页收敛为「工作记录 / 周期复盘」两个页签；审核候选改为浮层，任务管理回到项目跟踪分页。
- 时间线按日期倒序、组内按当天时间正序；记录卡补齐时间、分类、摘要、来源、关联任务与解析状态。

### Verification
- cargo test --offline --lib：1157/0。
- npx tsc --noEmit 与 npm run build：通过。
- 工作记录相关 verify 脚本 301/502/503/504/505/506/507/508/509 全通过。

## [0.2.31] - 2026-11-17

### Changed
- Skill 优化（OD-ARCH-174）：四场景技能指针各归其位（修 writing/project 可能错拿周报指针）；知识检索专用绑定键 `knowledge → knowledge-search`，宿主检索命中时追加 151 字符检索指针，普通对话同样生效（D-108）；模型技能目录 2518 → 2401 字符（document-review 转纯手点）；核心技能正文重写——memory-agent 1754→1228、memory-report 1099→749、period-review 1820→1268，输出契约与硬约束逐字保留。

### Verified
- 「指针 → 模型调用 skill 读取 → 按契约输出」全链路真实模型验证：模型实际调用 skill(memory-report) 读到重写正文并按五段模板完整产出。
- cargo test 1200 全过；tsc / npm run build 零错误；verify_conversation_prompt 33/33、skill_bindings_082 69/69、worklog_089 86/86、wr_503 58/58、wr_505 71/71。

### 升级说明
- 已安装用户需在 Profile 资产页点一次「恢复默认规则包」以获得 knowledge 检索绑定。

## [0.2.30] - 2026-11-17

### Added
- 首页 2.0 版式：问候带系统用户名（只读命令 `desktop_user_name`）、四张真实入口卡片、底部真实场景快捷入口、右上淡蓝山海背景插画（`desktop/public/home-scape.png`）。
- 线程记录新增 `forkOrigin`（父会话号与切点），新分支线程顶部显示一行灰色来源说明；普通新建线程不显示。
- 线程存储新增“未知前端字段原样保存与读回”的 Rust 回归测试。
- 设置页新增「配置解释」只读页（`diagnostics_metadata`）：生效路由、接口与地址、思考档位及声明、上下文窗口、提示词分段字符、工具面、Skill 概况，均可回答“这条行为从哪来”；只出元数据，不出提示词正文或用户内容。
- 模型保存与切换前预检：保存时按 Profile 的 `reasoningEfforts` 校验档位，401/402/403 给出认证、额度、套餐权限提示并阻止保存；切换时用 DSH 模型目录校验显式档位。

### Changed
- 授权：OmniDesk 自有代码自 0.2.30 起改用 PolyForm Noncommercial 1.0.0，商业用途须取得书面商业授权；0.2.29 及更早版本仍按 Apache-2.0 授权；`runtime/` 维持上游 MIT。
- 效率（OD-ARCH-173）：discipline preset 系统提示词 5396 → 2983 字符（去 PTC 口径；保留 PTC 时 15252），web-search 段 1919 → 0；随手记/周报/当日摘要/周期复盘的 user 段 2131/1446/1450/2222 → 636/558/487/613 字符（−61%～−72%）；Skill 全文改为 ≤260 字符指针，模型按需调用 `skill` 取正文（PTC 下在 `run_code` 内调 `tools.skill`）。
- 安全（OD-ARCH-173）：discipline Profile 移除 `dsh-free-search` 依赖与宿主插件要求，公开检索改由 anysearch Skill 按需承担；read/grep/glob 输出上限写入 preset（read 2000 行 / 51200 字节 / 单行 2000 字符，grep 250 条 / 单行 500 字节，glob 100 条）；模型保存/切换增加可用性预检，预检不读明文凭证。
- 工程：2026-11-17 之前的测试截图、任务证据与历史报告移出 Git，归档到仓库外；Git 保留测试脚本、夹具与架构契约。
- 仓库清理：个人发言稿、个人写作风格、个人数据根配置和初始化脚本移出 Git；写作风格改为通用模板，开发 Profile 改为中性名称，公开仓库不再包含个人文档。
- 发布：2026-11-17 重写 Git 历史并清理旧个人文档/证据，旧安装包已下线；0.2.30 将从清理后的源码重新构建。
- 发布形态：0.2.18 起只提供安装包（macOS universal dmg、Windows NSIS setup.exe），发布工作流不再组装便携 zip。
- 首页二次修订：去掉四张入口卡片、问候下方说明句与手掌表情；底部胶囊改为最近五个项目；对话开始后收起问候区，历史记录占满上方空间。
- 首页按新参考图重排：顶栏 30px 圆角；左侧 Dock 收进胶囊导轨，当前项为品牌靛实心圆；输入面板改为 26px 凸起带淡紫光晕；空态标语改为两侧分隔线夹一行演示夏行楷（靛蓝灰 `#615A85`）。
- Neo-Skeuomorphism 2.0 校准：象牙灰底材、近白凸起面、品牌靛 `#5747F8`；顶栏 34px 圆角，输入面板 24px，空态标语 30px / 0.12em 字距。
- 首页新对话空态标语改用品牌靛蓝，不再使用黑色正文色；保持演示夏行楷与 700 加粗。

### Fixed
- 首页归类菜单、模型下拉和 Profile 弹层统一收窄：归类菜单 176px，模型 248px，Profile 190–250px。
- 分支来源在读取与创建两端都走同一 `normalizeForkOrigin`，空父会话号或非法切点按“无来源”处理，避免当前会话谎称“由分支创建”。

### Verification
- `verify_fork_origin_202.py`：9/9。
- `cargo test --offline --lib thread_store::tests`：13/13。
- `npx tsc --noEmit`、`npm run build`：通过。
- `cargo test --offline --lib -- --test-threads=1`：1200 passed / 0 failed / 1 ignored。
- OD-ARCH-173 相关 `verify_conversation_prompt.py`、`verify_worklog_104.py`、`verify_wr_503.py`、`verify_wr_505.py`、`verify_tool_surface_154.py`、`verify_work_review_213.py`、`verify_profile_assets_109.py` 全通过；Skill 指针相关 `verify_skill_bindings_082.py`、`verify_worklog_089.py` 已按新行为更新断言后通过。

### Known limitations
- 真实模型 smoke test（Excel、周报、知识整理）未执行：账户返回 402 余额不足，属账户状态而非代码失败；提示词结构变化已由 mock 合成会话证明，真实模型行为待账户恢复后复测。
- Windows 安装包未在本机构建验证；macOS 侧完成 tsc、vite、cargo 与 Tauri 编译链路验证。
- 既有安装的 discipline Profile 不会自动删除已装 `dsh-free-search`：宿主装配只在缺失时写入、不覆盖用户文件；升级用户可在资产页点「恢复出厂内容」（写前备份），或重装 Profile。新装包由发布工作流重建 seed 后生效。
- 配置解释页未展示逐轮耗时、审批等待与缓存命中（阶段 5 明确不做时间线改版）；DSH 注入的 Skill 目录（约 2518 字符）仍存在，压目录需改上游 tool-skill 配置。

## [0.2.1] - 2026-10-15 (discipline-v0.2 / 结构收敛)

### Changed
- **统一资产权威源**：Skill 正文以 `skills/`、材料模板以 `templates/discipline/`、
  写作风格与优秀范例以 `knowledge/` 为唯一开发源；`workstations/discipline/` 下的
  副本（knowledge/、templates/、skills/）已删除，替换为指向说明。
- `workstations/discipline/profile.yaml` 重新标注为「规划声明 · 非生效配置」，
  生效 Profile 源明确为 `profiles/omnidesk-dev/`；其 package.json 更名为 `dsh-profile-omnidesk-dev`。
- 移除与 `profiles/omnidesk-dev/` 完全重复的 `profiles/desktop/` 参考快照。

### Added
- **运行底座锁定与引导**：新增 `runtime/RUNTIME.lock` 与 `scripts/bootstrap-runtime.sh`，
  一条命令即可得到与维护者验证一致的 DSH 版本（0.2.1-alpha.2），无需另找安装包。
- **DSH 源码底本可复现**：新增 `.gitmodules`，`runtime/deepseek-harness` 以子模块形式
  固定在 `dsh-v0.2.1-alpha.2`（commit d7432673），克隆即得同一底本。
- 新增 `data-template/README.md`，使文档承诺的外部数据骨架真实存在于仓库。

### Fixed
- **同步器安全缺陷**：`scripts/sync-skills.sh` 原先会把用户在隔离根自建的 Skill
  当作「多余」删除。现引入托管清单 `.omnidesk-managed`，只删除受同步器管理、
  且源目录已移除的 Skill；用户自有目录一律保留并提示。
- `--check` 不再把未托管目录计为漂移；新增 dry-run 汇总（新增/更新/删除/保留）。
- 同步中途失败会明确提示「可能处于半更新状态」，不再静默。

---

## [0.2.0] - 2026-10-14 (discipline-v0.2)

### Added
- **Discipline 知识库六大结构** (`knowledge/`)：
  - `regulations/`：法规制度库
  - `policies/`：上级政策与工作要点库
  - `writing-style/`：个人写作风格库
  - `excellent-cases/`：历史优秀范文库
  - `meeting-materials/`：会议纪要素材库
  - `methodology/`：纪检工作方法论库
- **个人写作风格规范资产** (`knowledge/writing-style/`)：
  - `style-guide.md`：10 大核心写作铁律
  - `common-phrases.md`：高频规范表达与对仗语料
  - `structure-patterns.md`：成熟段落三步递进与机制提炼模型
  - `avoid-list.md`：去 AI 味与空泛表述避坑清单
- **技能升级与新增**：
  - 升级 `skills/material-writing/SKILL.md` (v0.2)：三位一体上下文驱动（模板+风格库+案例比对）
  - 新增 `skills/document-review/SKILL.md`：四维度文稿质检评估能力（结构、事实、避坑、合规）
- **评测与测试基准**：
  - 黄金任务集扩展至 40 个真实脱敏用例 (`tests/golden_tasks/tasks.json`)
  - 建立 20 次真实履职人机协同工时记录 (`memory/usage-log/`)，平均单项提效 87.1%
  - 建立 30 题法规检索抽查测试报告 (`tests/reports/citation-audit-report.md`)，准确率 96.7%
- **版本归档机制**：
  - 增加 `VERSION` 文件与 Git Tag `discipline-v0.2`

---

## [0.1.0] - 2026-10-14 (discipline-v0.1)

### Added
- 建立 `workstations/discipline/` 工作站规范与 Profile 定制配置
- 确立《OmniDesk 系统设计原则与设计方案 v0.3》（先产品后平台、数据分级保护）
- 落地第一版 `material-writing` 与 `knowledge-search` 技能
- 确立 3 份基础公文模板 (`templates/discipline/`) 与首批 20 个黄金任务基准
- 实现会话与工作区自动镜像归档脚本 (`scripts/backup-workspace.sh`)
- 冻结系统 Phase 0 五层解耦架构与三大红线

