# OD-GROW-301 G0 前置核实笔记

## 范围与基线

本笔记只记录 G0 只读核实，实施口径以 `docs/od-grow-301-g0-dispatch.md` 为准。开工前 HEAD 为 `5dc5df4b622815f4803ec4cff07d448e9824dbf1`，git status 快照为 PLANS.md 已修改、`docs/od-grow-301-g0-dispatch.md` 未跟踪。既有改动全部保留，本任务未改源码、配置、测试，未提交、未推送、未打标签。

证据等级按任务书分为文件事实、静态检查、脚本运行、未核实。本笔记只使用只读检索与源码阅读，没有运行测试或真实模型。

## 1. 写作模块的版本链与按章对比

**结论**

写作文稿落在 `<work_root>/写作/<writing_id>/`，包含 `正文.md`、`versions/v<n>_<label>.md` 与 `manifest.json`。版本写入的唯一入口是 `document_store::save_with_snapshot`，它先写不可覆盖的快照，再替换当前文稿，最后更新 manifest，版本号从 1 连续递增。

当前版本链记录的是每一次确认后的完整 `正文.md`。章节确认会先读取已有正文，替换本章内容，再按大纲顺序渲染整篇正文，最后以 `VersionScope::Section` 和 `VersionSource::User` 保存快照。因此每次章节确认的快照都是全文，并非只含该章片段。

AI 生成的章节稿只作为界面上的「AI 建议」预览，确认前不落盘。全仓检索未发现生产路径以 `VersionSource::Ai` 保存版本，作为保存来源的 `VersionSource::Ai` 只出现在 `document_store.rs` 的模块测试中。因此目前没有「AI 原稿与用户定稿」并列的版本链。若 G2 需要比较二者，必须先新增 AI 候选稿的独立落盘方式。

按章节对比在数据层可行，因为每个全文快照都含 `## <n>. <标题>`，`writing_section::parse_body_sections` 已能按章拆分。当前没有对外暴露的按章差异函数，`writing_version_diff` 与 `WritingVersionPanel` 只按整篇正文的段落序号比较。整篇段号比较在插入新段后会把后续段落整体标成修改，不适合逐章判断。

**依据**

- 文稿目录与文件名为 `desktop/src-tauri/src/document_store.rs:3-7`、`39-42`，路径校验在 `220-243`。
- 写作任务通过 `document_id` 关联文稿见 `desktop/src-tauri/src/writing_store.rs:104-105`，写作目录定义见 `167-176`。
- 版本来源、范围与 manifest 条目见 `desktop/src-tauri/src/document_store.rs:76-120`、`131-169`。
- 唯一写入入口、快照先写与 manifest 后更新见 `desktop/src-tauri/src/document_store.rs:727-777`，读取历史版本见 `815-840`，恢复见 `842-859`。
- 整篇段落差异见 `desktop/src-tauri/src/document_store.rs:887-935`。
- 章节确认插入本章、渲染全文、保存 `Section/User` 快照见 `desktop/src-tauri/src/writing_section.rs:794-843`，全文渲染见 `443-459`，按 `## n.` 拆分见 `397-437`。
- 大纲确认也以 `User` 保存见 `desktop/src-tauri/src/writing_outline.rs:968-974`，定稿快照也以 `User` 保存见 `desktop/src-tauri/src/export_engine.rs:1369-1375`。
- 审阅修订应用走同一个章节确认通道见 `desktop/src-tauri/src/writing_review.rs:563-571`。
- AI 建议仅在界面预览、确认后才写入的说明见 `desktop/src/shell/WritingSectionEditor.tsx:3-6`，确认动作见 `427-456`。
- 版本页签见 `desktop/src/shell/WritingWorkbench.tsx:621-627` 与 `719-725`，版本面板的整篇对比见 `desktop/src/shell/WritingVersionPanel.tsx:65-76`、`172-197`、`338-408`。
- 版本差异命令只比较同一文稿类型的整篇内容见 `desktop/src-tauri/src/writing_review.rs:631-710`。

**证据等级**

文件事实与静态检查。结论「没有生产路径保存 AI 版本」来自全仓 `rg` 检索，真实按章差异 UI 尚未实现，标为未核实能力。

## 2. 线程消息字段、plain 识别与 load 耗时

**结论**

线程记录的最小字段为 `id`、`tag`、`title`、`preset`、`messages`、`createdAt`、`updatedAt`，另有可选的 `projectPath`、`engineSessionId`、`inflightTaskId`、`forkOrigin`。

消息的必需字段是 `id`、`role`、`text`。`ChatMessage` 还有可选的 `taskId`、`state`、`stopReason`、`error`、`notice`、`retryable`、`toolNote`、`timeline`、`sentText`、`startedAtMs`、`endedAtMs`、`timing`、`forkSeq`、`local`、`executed`、`citations`。没有每条消息通用的时间戳字段。`startedAtMs` 与 `endedAtMs` 只记录助手轮次的提交与终态时刻，不能覆盖所有消息。宿主也只校验 `messages` 是数组，不校验每条消息结构，旧线程可以只有 `id/role/text`。

plain 线程按 `tag === "plain"` 识别。`normalizeTag` 把未知标签统一回落到 `plain`，`preset` 是另一字段且默认也为 `plain`，但分类应以 `tag` 为准。内置分类表把 `plain` 显示为「未分类」。

`thread_store::load` 会遍历线程根下的所有项目分目录和文件，逐个读取整个 JSON、执行迁移、校验、解析全部消息，再按 `updatedAt` 降序排序后一次性返回。没有 limit、offset、游标或按线程读取的入口。前端首屏有 4 秒等待上限，超时后先用本地元数据缓存渲染。

耗时特征为大致线性于文件数与总字节数，主要成本是逐文件读盘、JSON 解析与全量内存排序。真实线程数量和实际耗秒未测量，标为未核实。G2 不建议直接沿用全量正文的 `load` 扫描全部线程，建议新增只返回 `id/tag/title/updatedAt` 的元数据列表，带默认上限或分页，再按 `max_threads=20` 逐条读取正文。这样既能筛选 plain 线程，也不会把全部历史正文一次搬进内存。

**依据**

- 线程字段见 `desktop/src/sceneThreads.ts:32-59`，消息字段见 `desktop/src/shell/HomeChat.tsx:166-216`。
- 宿主只校验 `messages` 为数组见 `desktop/src-tauri/src/thread_store.rs:424-458`，旧样例只有 `id/role/text` 见 `673-704`。
- plain 类型与说明见 `desktop/src/sceneThreads.ts:25-35`，标签归一化见 `489-493`，新建默认见 `380-402`。
- 内置未分类显示见 `desktop/src/threadCategories.ts:22-35`，未知键回落到未分类见 `124-126`。
- 全量读取、逐文件解析、全量排序见 `desktop/src-tauri/src/thread_store.rs:104-195`，单文件读取与大小上限见 `26-27`、`197-228`。
- 4 秒首屏上限与缓存兜底见 `desktop/src/sceneThreads.ts:84-91`、`222-260`。
- 现有界面没有线程分页，只对分组内显示条数做折叠见 `desktop/src/shell/SessionHistoryPage.tsx:55-61`、`172-193`。

**证据等级**

文件事实与静态检查。真实线程数下的耗时未实测，标为未核实。

## 3. 无人值守提交、结果取回与专用命令挂点

**结论**

`WorkRecordLibrary` 的解析链路是可用先例，顺序为 `worklog_record_analyze` 取提示词，`prepareWorkspaceDir` 取工作目录，`host.submitTask` 提交，每秒轮询 `taskStatus`，成功后取 `taskResult`，最后交 `worklog_record_analyze_apply` 校验并落暂存候选。整条链路不使用聊天组件，状态与结果都按 `omni_task_id` 直接查询。

纯文本提示词可以提交，`worklog_extract` 就是同类先例。宿主与引擎的提交参数没有工具白名单或「禁用工具」字段，所以无法在接口层面强制无工具运行。G4 只能在提示词里要求只输出约定 JSON、不调用工具，并保持工具审批的人工确认，不能把「无工具」当作已由代码保证。

结果可以在没有聊天组件时取回。`host.taskStatus` 与 `host.taskResult` 只需要任务号。任务未到终态时 `get_task_result` 返回 `not_ready` 与当前已有片段，终态后才给完整结果。引擎进程重启后，`engine_recover_task` 只能按本地台账恢复状态，不能重新取回正文结果，因此 G4 必须在同一引擎进程内完成终态读取并落盘摘要。

当前没有引擎侧的任务运行时限。提交 RPC 只等 60 秒拿到回执，状态 RPC 等 30 秒，结果 RPC 等 60 秒，任务本身默认不按墙钟收尾。`WorkRecordLibrary` 的 900 秒轮询是调用方自设上限，达到后把记录标为失败，但没有自动取消任务。取消只能显式调用 `cancelTask`，默认等 15 秒，并返回取消阶段与最终状态。启动阶段取消会本地落定为失败，运行中取消要等 `session/cancel` 得到终态确认，超时可能是 `timeout_unknown`。

G4 的专用整理提交命令建议落在 `desktop/src-tauri/src/commands/consolidation.rs`，并在 `lib.rs` 注册。先把现有 `engine_submit_task` 中的红线扫描、准入、`scene_gate`、工作目录与 `bridge.submit_task` 主体抽成可复用函数，新命令只负责校验批准记录与组装摘要。intent 可取一个不在 `pending_writes::scene_for_intent` 白名单内的专用值，例如 `habit_consolidation`，这样不会误触发周报、写作或项目场景的待确认卡门禁。批准记录的放行必须按具体记录匹配，不能把专用 intent 变成全局绕过。

工作目录目前仍是引擎必填项。Rust 命令把 `workspace_dir` 视为可选，但引擎的 `canonical_cwd` 要求绝对路径且真实存在，`submit_task` 会直接校验。`WorkRecordLibrary` 因此显式调用 `prepareWorkspaceDir`。G4 可以继续显式准备，也可以等 T1 完成后由宿主决定缺省工作目录。

**依据**

- 解析链路见 `desktop/src/shell/WorkRecordLibrary.tsx:610-664`，入口按钮见 `1734-1745`。
- 前端提交、状态、结果、取消接口见 `desktop/src/api.ts:431-494`。
- 宿主提交顺序与 stdin 参数见 `desktop/src-tauri/src/lib.rs:465-635`，无任务等待上限的说明见 `541-543`。
- 结果与取消实现见 `desktop/src-tauri/src/lib.rs:786-806`、`837-868`。
- 工作目录准备见 `desktop/src-tauri/src/lib.rs:8998-9030`，引擎要求绝对且存在的路径见 `engine/omnidesk_engine/acp_adapter.py:111-142`，提交时校验见 `engine/omnidesk_engine/web_api_adapter.py:1485-1500`。
- 提交请求没有工具字段见 `engine/omnidesk_engine/task_core.py:161-185`，RPC 时限见 `desktop/src-tauri/src/engine_bridge.rs:221-267`。
- 引擎默认不按墙钟收尾见 `engine/omnidesk_engine/web_api_adapter.py:500-522`，结果未就绪的返回见 `engine/omnidesk_engine/task_core.py:744-764`，重启恢复只恢复状态见 `engine/omnidesk_engine/task_core.py:903-975`。
- 取消阶段与超时未知见 `engine/omnidesk_engine/web_api_adapter.py:2740-2830`。
- 场景 intent 白名单见 `desktop/src-tauri/src/pending_writes.rs:427-438`，已有 `worklog_extract` 不在白名单内见 `desktop/src/shell/WorkRecordLibrary.tsx:620-625`。
- `commands/` 目录与模块注册现状见 `desktop/src-tauri/src/commands/mod.rs:1-4`、`desktop/src-tauri/src/lib.rs:39`。

**证据等级**

文件事实与静态检查。纯文本无工具的真实模型行为未运行，标为未核实。任务超时结论以源码为准，未用真实长任务复测。

## 4. 首页提醒条与关闭状态

**结论**

`AnnouncementBar` 只做调度心跳与待确认刷新，最终返回 null，没有可见横幅。它只挂在 `ScenePage`，`HomeCanvas` 没有挂载。因此首页顶部没有现成横幅位。

最小方案是在 `HomeCanvas` 传给 `SceneScaffold` 的 `upper` 槽顶部加一个条件渲染提醒条，位置在问候与待办之前。注意 `upper` 目前在有对话时整体返回 null，提醒条必须放在 `conversationStarted` 三元表达式之外，否则用户一开始对话就会消失。`AnnouncementBar` 不要改成视觉横幅，它可以继续只做心跳。

关闭状态应落在应用数据根的 `ui-state` 目录。目录已在 `AppPaths` 定义并由 `app_paths_info` 暴露路径，但目前没有读写 `ui-state` 的命令或前端封装。G5 需要新增一个最小的 ui-state 读写命令，按日期保存「当天已关闭」标记，新一天重新显示。具体文件名和命令名未实现，标为未核实。

**依据**

- `AnnouncementBar` 返回 null 见 `desktop/src/shell/AnnouncementBar.tsx:19-52`，文件头说明它只保留心跳见 `1-17`。
- 唯一挂载点见 `desktop/src/shell/ScenePage.tsx:602`，版式说明见 `108-114`。
- 首页 `upper` 槽与问候区见 `desktop/src/shell/HomeCanvas.tsx:550-563`，对话开始后隐藏上方内容的逻辑见 `487-490`。
- 公共骨架的上部槽见 `desktop/src/shell/SceneScaffold.tsx:32-54`。
- `ui-state` 路径见 `desktop/src-tauri/src/app_paths.rs:30-46`、`94-106`，仅路径暴露见 `desktop/src-tauri/src/lib.rs:1040-1054`。

**证据等级**

文件事实与静态检查。关闭状态的持久化能力尚不存在，标为未核实。

## 5. 周期复盘草稿的应用内编辑

**结论**

`ReviewArchive` 只有列表、筛选、生成、重新生成、确认归档、导出 Markdown 与交给 AI 分析。正文以 `MarkdownText` 只读渲染，没有 textarea、contentEditable 或保存正文的入口。

宿主侧同样没有编辑命令。`worklog_period_review_generate` 接收界面传来的模型正文并写草稿，`worklog_period_review_confirm` 只把草稿状态改为已确认并写 `confirmed_at`，正文原样保留。重新生成会备份旧草稿后覆盖，不属于编辑。

H1 的结构化校验与 H2 的结论明细展示都只依赖读取与生成结果，不依赖应用内编辑能力。当前链路满足这两个后续任务的前置条件。

**依据**

- 复盘按钮与详情布局见 `desktop/src/shell/ReviewArchive.tsx:656-706`，正文只读渲染见 `737-740`。
- 生成、读取、确认命令见 `desktop/src-tauri/src/lib.rs:5985-6041`，前端封装见 `desktop/src/api.ts:910-968`。
- 草稿写入与状态保护见 `desktop/src-tauri/src/review_store.rs:730-800`，确认只改状态与时间见 `1149-1205`。
- 初次生成只预填首页对话见 `desktop/src/shell/ReviewArchive.tsx:236-248`，重新生成与取文链路见 `267-330`。

**证据等级**

文件事实。未发现编辑入口或编辑命令，属于静态检查结论。

## 对 G2/G4 设计的影响

G2 可直接使用线程 `tag/updatedAt` 与消息 `id/role/text`，但不能假设每条消息有时间戳，写作差异若要区分 AI 原稿与用户定稿需先补候选稿落盘，按章对比也需新增宿主级分段差异。G4 可复用 prepare→submit→轮询→result→apply 的无聊天链路，但现有「超时」只是 15 分钟轮询上限，引擎任务本身无时限，专用整理命令应落在 `commands/consolidation.rs` 并用不在 `scene_for_intent` 白名单的 intent。首页提醒条应放在 `HomeCanvas` 的 `upper` 槽且在 `conversationStarted` 条件之外，关闭状态需新增 `ui-state` 读写入口。

## 未核实事项

- 真实线程数量与 `thread_store::load` 的实际耗时未测量，评估为静态复杂度结论。
- 纯文本无工具提交的真实模型行为未运行，只确认了接口没有工具禁用字段。
- `ui-state` 的读写命令尚不存在，关闭状态的落盘格式只是设计建议。
- G4 专用命令、G2 收集器与按章差异均未实现，本任务只核实现状与挂点。
