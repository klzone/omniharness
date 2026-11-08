# 当前实施计划

### OD-WT-600：第四阶段——AI 文稿写作 + 事实核验 + 成果归档（已批准，2026-11-09；待实施）

> **触发：** 用户《OmniDesk 第四阶段总任务》方案 + 用户裁决修订（定稿→导出→归档闭环；固定样式导出：公文 Word/阅读 PDF/公众号 HTML；归档用 MD）。核心口径：AI 只建议宿主落盘、大纲确认门禁、分章节生成、事实核验双引擎、版本快照、归档仅 MD 进知识库。

> **用户裁决（2026-11-09）：** ① 第三阶段先提交推送（a632af4369）再开工；② 写作任务实体落 work/写作/；③ 归档门禁软引导（硬门禁仅校验已定稿）；④ DOCX 复用 office-docx 技能+textutil 兜底，PDF 走 soffice/打印流，公众号 HTML 宿主渲染；⑤ 执行派发：子代理 deepseek-v4.1-flash-eco + 思考等级 high。

> **任务卡（601–615）：** 全文见 docs/plans/OD-WT-600/PHASE4_IMPLEMENTATION_PLAN.md。批次：601∥603→602 → 604∥605→606 → 607→608∥609 → 610∥611∥612→613→614 → 615。技术决策 D-083～D-091 已落 docs/decisions.md。验收对照见 PHASE4_ACCEPTANCE_CRITERIA.md（十三场景）。

> **执行进展：** 待实施。

### OD-KM-500：第三阶段——知识库联动 + 写作素材篮（已批准，2026-11-09；待实施）

> **触发：** 用户《OmniDesk 第三阶段总任务：知识库联动 + 写作素材篮》方案。建立工作事实、任务、项目、知识依据与写作素材的关联体系，素材经人工审核成为第四阶段文稿生成的可信输入。核心口径：AI 只建议、宿主落盘、用户拥有最终决定权；work/ 临时层永不进知识库。

> **用户裁决（2026-11-09）：** ① 先提交推送第二阶段（commit 18a012523c）再开工；② 项目线保留，13 卡全做；③ 素材篮入口放文案写作场景新页签（WritingStudio）；④ 执行派发：子代理 OpenDesign/deepseek-v4.1-flash-eco + 思考等级 high，被路由拒绝则停下等用户；过夜流水线执行，全部完成后本会话逐任务复核。

> **任务卡（501–513）：** 全文见 docs/plans/OD-KM-500/PHASE3_IMPLEMENTATION_PLAN.md（按 IMPLEMENTED_RULES §1 模板，含架构契合/实施步骤/验收命令/依赖）。验收对照见 PHASE3_ACCEPTANCE_CRITERIA.md（十二场景 + verify_security_phase3 必查项）。批次：501∥502∥503 → 504∥505∥508 → 506∥507 → 509∥510∥511∥512 → 513。技术决策 D-076～D-082 已落 docs/decisions.md。

> **执行进展（2026-11-09）：** OD-KM-501 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：material_store.rs（WritingMaterial 实体、十类/五态/五来源白名单、sha256 指纹去重、审核记录只追加、CRUD+过滤、work_cleanup 素材篮排除有测试）；本会话独立复跑 lib 792 passed/0 failed（净增 22）、verify 19/19。OD-KM-502 与 OD-KM-503 已并行实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：502 project_store.rs（omnidesk.project.v1、名称/状态/日期校验、update 重名冲突、project_refs 纯函数分组，14 项测试）；503 Task+WorkRecord 关联字段（project_id/related_knowledge serde default、KnowledgeLink 三字段、库级形状校验、旧文件字节不变测试，15 项测试）。本会话独立复跑 lib 821 passed/0 failed（净增 29），并发纪律与敏感词扫描核对通过。OD-KM-504 / 505 / 508 已并行实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：504 素材提取搭车（material_candidates 提示词+宿主校验+素材篮/.candidates 暂存+指纹去重，pytest 净增 10）；505 knowledge_link.rs（三类依据枚举、去重排序、上限 8，14 项测试）；508 项目 IPC 四命令（create/update/list/overview，gate 覆盖，project_id 存在性校验 5 项测试，api/types 成套）。本会话独立复跑 lib 856/0、pytest 197+30、tsc 0。OD-KM-506 / 507 已并行实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：506 素材 IPC 六命令（含追溯 payload、审核流转 gate、素材清单注入分析提示词补 504 遗留、build exit 0）；507 知识关联 IPC 四命令（候选暂存/确认/移除留痕/反向查询，frontmatter 定向写回保字节不变）。本会话独立复跑 lib 876/0、tsc 0、build 0、pytest 200+30。OD-KM-509 / 510 / 511 / 512 已并行实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：509 material_query.rs（默认已确认素材、场景十二 payload 七项测试）；510 素材篮页（WritingStudio 页签组 + WritingMaterialBasket，23/23）；511 关联视图（KnowledgeLinkPanel 落记录/任务详情 + 知识反向区块，25/25）；512 项目视图（ProjectBoard 概览区块，26/26）。本会话独立复跑 lib 883/0、tsc/build 0、三份 verify 全绿。OD-KM-513 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：全量命令全绿（cargo --tests 950/0、pytest 200+30、build/tsc）；verify_security_phase3 25/25（八项安全口径全 PASS）；十二场景 12/12 有证据；证据落 evidence/od-km-513/。**第三阶段（OD-KM-501～513，13 项任务）全部竣工（2026-11-09）。** 待办：用户批准后提交推送；三处新 UI 真实窗口目视追认；真实模型质量与大量素材性能列未验证事项。

### OD-UI-202：新拟物设计语言落地（第一阶段已实施；第二阶段待批准，2026-11-08）

> **触发：** 用户指出当前页面过于扁平，要求把参考图的新拟物（Neumorphism / Soft UI）风格用于 OmniDesk，首页顶栏也采用该风格；并到 OpenDesign 中查找现成设计系统。

> **本会话已完成（只读研究 + 设计交付，未改真实界面）：** ① 梳理新拟物的定义、光影语法、令牌、组件配方、可访问性与性能门槛，落盘 `docs/brand-and-ui/neumorphism-guideline.md`；② 制作可直接打开对照的 `docs/brand-and-ui/neumorphism-preview.html`（暖蜜 / 品牌靛两套强调色、增强边界模式、首页顶栏与控件状态）；③ 在 OpenDesign 本地官方库确认存在 `design-system-neumorphism`（0.1.0，MIT，Primary #006666 / Surface #E7E5E4），没有现成 OmniDesk 项目；④ 按 OD-ARCH-035 检索同类布局：Navigator（窄日历栏 + 搜索顶栏）、Craft（低 chrome 白底画布）、Notion（底部固定输入条），仅借鉴布局，不照搬材质。**真实页面改动尚未开始，等待用户批准与强调色选择。**

**现状核查（2026-11-08 实读源码，HEAD 38d839c）：** 当前 Aetheria 是“平面 + 玻璃 + 细描边”：顶栏在 `TopNav.tsx` 使用 `bg-surface-container-lowest/80` + `backdrop-blur-2xl` + `ring-1 ring-white/90`；首页对话使用 `plate-sculpted`（半透明白底 + 较小阴影）；Dock 与多数按钮处于同一视觉高度。设计令牌集中在 `desktop/src/theme.css`，可新增 `neu-*` 层而不动业务样式。工作区已有 OD-QS-4xx 未提交改动，本次预计触及 `theme.css`、`TopNav.tsx`、`App.tsx`、`HomeCanvas.tsx`、`HomeChat.tsx`、`Dock.tsx`，与既有修改不重叠。

**批准后的第一阶段范围（只做顶栏 + 首页，不一次重写全站）：**

1. **设计令牌层**（`desktop/src/theme.css`）：新增暖白灰底材、内容面、凹陷面、石墨文字、蜂蜜强调、双向外/内阴影、焦点环令牌；保留现有 Aetheria 令牌供未迁移页面使用，不做全局删改。
2. **首页顶栏**（`desktop/src/shell/TopNav.tsx`）：只改 className 与必要的局部样式，不改结构、事件、Profile 切换、搜索、日期弹层、窗口控制、`data-testid` 与拖拽区。顶栏改为 64–68px 凸起长条，搜索与 Profile 徽标为凹陷，日期/设置/通知为凸起；移除顶栏 `backdrop-blur`。
3. **应用底材与光晕**（`desktop/src/App.tsx`）：把淡紫玻璃光晕替换为静态暖色径向光晕（不使用 `filter: blur`），保持现有布局与滚动边界。
4. **首页主面**（`HomeCanvas.tsx` / `HomeChat.tsx`）：问候保持轻字重；对话主面改为一层 `raise-3`；输入区改为 `inset-2`；用户消息用蜂蜜浅底轻凸，助手消息不加阴影。保持“固定顶栏/输入、只有对话滚动”的既有基线。
5. **Dock**（`Dock.tsx`）：未选中为凸起圆钮，当前项为凹陷 + 蜂蜜标记；悬停只上浮 1px。
6. **可访问性补足**：所有控件保留文字/图标线索；焦点 2px、对比 ≥3:1；新增内部“增强边界”样式开关（默认关闭，仅用于低视力环境，不改变默认观感）。
7. **验证**：`npm --prefix desktop run build`、`npx tsc --noEmit`；新增 `tests/verification/desktop/verify_ui_neumorphism_202.py` 检查令牌存在、顶栏无 `backdrop-blur`、焦点环与凹陷/凸起类可被实际引用；真实窗口 1280×820 截图目视核对顶栏、首页、Dock、键盘焦点；`prefers-reduced-motion` 与增强边界各截图一次。证据放 `tests/verification/desktop/`。

**暂不纳入第一阶段：** 工作记录、文案写作、项目跟踪、设置、知识库的页面卡片迁移（另立第二阶段，避免一次性大改与回归面过大）；任何业务逻辑、IPC、数据模型、模型路由、权限、布局行为、固定/滚动基线；不引入第三方 UI 库；不做持续动画、不做渐变金属感。

**用户裁决（2026-11-08）：** 强调色采用品牌靛；批准先做“顶栏 + 首页”第一阶段，全部页面延后另批。

> **执行进展（2026-11-08，用户选定「品牌靛 + 先做顶栏/首页」）：** 已实施第一阶段。新增 `theme.css` 的 `--od-neu-*` 材质令牌与凸起/凹陷/焦点/弹层类；顶栏改为 64px 凸起长条，搜索与 Profile 徽标压入，日期/设置/通知/窗口按钮凸起，日期与 Profile 弹层改为实色无模糊；App 底材换暖白灰并保留静态暖色光晕；Dock 改为无容器浮动圆钮，当前项凹陷 + 品牌靛标记；首页输入区压入、发送钮凸起、用户消息轻凸、助手消息浅面；归类与模型弹层同步材质。真实窗口试装时曾出现“整幅对话面板”在空态下像巨大空白卡片，已撤掉该包裹，只保留导航、Dock、输入与消息的材质层级。2026-11-09 用户否决宋体界面后，中文统一改为黑体：大标题思源柔黑（回退思源黑），界面标题/正文/控件/表格思源黑；行楷只做标语，首页欢迎短句使用加粗；方正楷体仅长引用/落款，思源宋体仅导出报告/打印。**未改动**业务逻辑、IPC、数据模型、权限、模型路由、布局固定/滚动基线；未动并发中的 TaskManager 等 OD-QS 文件。

> **验证（本会话实跑）：** `npm run build` 通过（tsc + vite，仅既有 >500kB chunk 提示；中途曾被并发 OD-QS 的 TaskManager 未使用导入阻塞，待对方修复后本会话复跑通过）；`npx tsc --noEmit` 通过；新增 `verify_ui_neumorphism_202.py` **29/29**，JSON 证据 `tests/verification/desktop/verify_ui_neumorphism_202.json`；重建 macOS debug App（1280×820）实窗口目视核对顶栏、搜索/Profile 凹陷、日期/窗口按钮凸起、Dock 当前项凹陷、输入区压入、焦点环和系统黑体回退。真实窗口截图见本会话工具记录；CUA 截图 API 不回写文件，故未落盘到 tests/verification。**本机未找到思源柔黑/思源黑/演示夏行楷字体文件**；应用只引用字体名，不复制或分发字体，装入后生效，柔黑/行楷实际字形待装字体后再验。

**风险与边界：** R1 全局底材变暖会同时影响未迁移页面，第一阶段先用暖底 + 现有靛蓝文字/按钮，避免中途出现两套不协调的强调色；R2 新拟物边界弱，必须用焦点环、文字标签与增强边界补足；R3 大面积 `box-shadow` 可能拖慢 WebKitGTK，限制 L3 数量并禁止动画化阴影；R4 OD-QS-4xx 未提交改动须原样保留，动手前再核对 diff。

### OD-QS-4xx：第二阶段——任务四象限 + 智能排期（竣工：401～413 全部实施并复检通过，2026-11-09；待用户批准提交推送）

> **触发：** 用户《OmniDesk 第二阶段总任务：任务四象限 + 智能排期》方案。在第一阶段任务数据之上建立重要性 / 紧急性 / 四象限 / 个人排期能力。核心口径：AI 只建议、规则引擎计算、用户拥有最终决定权；四象限是解决工作中拖延问题的梳理工具，不是展示页面。

> **已批准裁决（2026-11-08）：** ① 技术决策 D-069～D-075 照草案批准（全文见 docs/decisions.md；用户预期任务量不大——任务过多说明工作本身有问题，象限正是用来梳理拖延的）；② 前端既有工作记录四象限占位（weeklyQuadrants.ts / WeeklyQuadrants.tsx，OD-ARCH-093）由本阶段任务四象限直接替换，不并存；③ 执行全部由用户新开会话派发任务卡，本会话只做规划与复检，不派子代理。

**第一阶段现状核查（2026-11-08 实读源码）：** 可复用——Task 实体（due_date 仅 YYYY-MM-DD、六状态白名单迁移、候选去重键、.candidates/ 暂存）、task_extract 提示词链路与 validate_task_payload、gate_user_text 安全闸门、Markdown+frontmatter 原子写纪律、work/ 临时层结构。需最小补齐——Task 无 importance/urgency/due_kind/depends_on/estimated_minutes/parent_id/quadrant_override 字段；无个人工作时间配置；无排期实体；GOALS.md 数据边界措辞修订（第一阶段裁决①遗留，已随本次批准补做）。

**技术决策（D-069～D-075，全文见 docs/decisions.md）：** D-069 importance 落盘 / urgency 派生不落盘（仅 override 落盘）；D-070 象限纯规则计算、AI importance 建议搭既有提取调用顺车（零新增模型调用）；D-071 排期实体 work/排期/ 独立文件、状态与任务状态解耦；D-072 工作时间配置 work/availability.v1.json（无文件=默认零写入）；D-073 确定性贪心排期（截止升序 × 重要性降序 × 象限序），锁定/已确认不可动、冲突显式报告不强塞；D-074 任务拆分最小实现（超最大时段报「建议拆分」，实际拆分=用户手动建子任务+parent_id，不做 AI 自动拆分）；D-075 模糊时间封闭模式集推定（明天/后天/下周X/本月底/下月底/X月X日前）标「推定」，模式外一律待评估。

**任务卡（IMPLEMENTED_RULES §1 模板；编号 OD-QS-401 起）：**

> **执行进展（2026-11-08）：** OD-QS-401 与 OD-QS-406 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：401 交付八个 Task 字段（importance/importance_source/importance_reason/due_kind/depends_on/estimated_minutes/parent_id/quadrant_override），旧文件 serde default 零迁移（字节不变有测试），用户重要性必填理由，依赖/父任务的存在性+环校验成套（15 项新测试）；406 交付 availability.rs（schema omnidesk.availability.v1，无文件=默认零写入、原子保存失败不破坏旧文件，11 项测试）+ availability_get/set IPC（unavailable 原因过 gate_user_text）+ 设置页完整编辑区。本会话独立复跑：lib 698 passed / 0 failed（净增 26）、--tests 13 目标全绿、tsc exit 0、build exit 0。**下一批（可并行）：OD-QS-402（紧急性引擎）∥ OD-QS-404（AI 重要性建议）∥ OD-QS-407（排期实体）；406 已就绪，408 待 403/406/407 齐备。**

> **执行进展（2026-11-08 续）：** OD-QS-402 / 404 / 407 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：402 交付 priority_rules.rs（逾期→高、工作日 2/5 分档、target 降档、封闭模式集推定标「推定」、模式外待评估、显式偏移接口，15 项测试 + verify 11/11）；404 交付候选 importance 字段（spec 约束 + 宿主成对校验 + 确认落 Task 写 source=ai，零新增模型调用，pytest 187 passed）；407 交付 schedule_store.rs（排期条目 Markdown + 状态机与任务解耦 + 调整留痕 + 派生已失效不改写文件 + 同任务重叠拒绝，15 项测试）。本会话独立复跑：lib 733 passed / 0 failed（净增 35）、--tests 13 目标全绿、verify_priority_rules_401 11/11。**下一任务：OD-QS-403（四象限分类引擎）→ 405 ∥ 408 → 409 → 410/411 → 412 → 413。**

> **执行进展（2026-11-08 三）：** OD-QS-403 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：QuadrantView 十字段 + classify_quadrant（待评估不强行归类 / override 优先标「用户指定」/ done+cancelled+暂停 active=false 且暂停保留重要性 / 逾期低重要→Q3 不强制 Q1 / target 变象限）+ list_quadrant_views_at（显式偏移 + active→象限序→标题稳定排序）；方案 §十二.1 十三场景全部有测试。本会话独立复跑：lib 746 passed / 0 failed（净增 13）、--tests 13 目标全绿。**附：verify_priority_rules_401 的 pure_function_no_io 断言对 std::path 误报（403 合法路径参数），随 405 修正为两项真 IO 判据。执行裁决「只有高算重要/紧急」已在 REVIEW 登记，中档归类如实测偏差属 D-069 复审触发。** 408 前置全部就绪。**下一批：OD-QS-405 ∥ OD-QS-408 可并行派发。**

> **执行进展（2026-11-08 四）：** OD-QS-405 与 OD-QS-408 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：405 交付五条 IPC（set_importance/quadrant/due/estimated/dependency，理由过 gate_user_text）+ Task 调整记录追加段（## 调整记录，只追加）+ 前端类型与调用封装（401 登记的前端映射欠账补齐、403 登记的 verify 断言修正完成，verify 11/11）；408 交付 scheduling.rs（十一优先序：锁定/已确认 fixed 不可覆盖、硬截止优先、Kahn 依赖拓扑+环冲突、容量 80% 缓冲+Q4 25% 上限+不抢 Q2、待评估不排、超时长建议拆分；17 项测试，§十二.3 十六场景全覆盖）。本会话独立复跑：lib 766 passed / 0 failed（净增 20）、--tests 13 目标全绿（首轮 1 failed 复跑即过，按 macOS 偶发噪音登记）、tsc/build exit 0。子裁决：待确认任务不入排期候选（比任务卡更严，符合 D-063）。**下一任务：OD-QS-409（排期 IPC 与确认流）→ 410/411 → 412 → 413。**

> **执行进展（2026-11-08 五）：** OD-QS-409 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：六条命令（generate/confirm/adjust/cancel/lock/list）+ 前端类型成套；生成幂等（既有建议视作占位防重叠、二跑 created 为空有测试）、锁定+已确认保护落实、generation.v1.json + stale 标志（任务改于生成后即提示重新生成）、过期处理只动已建议且锁定豁免（有测试）。本会话独立复跑：lib 770 passed / 0 failed（净增 4）、--tests 13 目标全绿 EXIT=0、tsc exit 0。登记一项待 413 复核：schedule_adjust/cancel 理由未过 gate_user_text（与 302 闸门边界一致判定通过，413 显式检查项）。工作区另有 OD-UI-202 设计稿两文件（未跟踪、与本任务无关），提交时用户裁决。**下一批：OD-QS-410 ∥ OD-QS-411 可并行派发 → 412 → 413。**

> **执行进展（2026-11-08 六）：** OD-QS-410 与 OD-QS-411 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：410 完成旧占位替换（weeklyQuadrants.ts/WeeklyQuadrants.tsx 删除、无引用残留 verify 断言、REPORT_DESTINATION 移入 weekTimeline）+ TaskQuadrantBoard 五列（Q1–Q4+待评估文字标题、双依据、用户指定/已确认/已建议标注、调整走 405 IPC）；411 交付 SchedulePlanner 今日/本周双模式（可用时段+剩余容量实算、已确认/已建议分组+就地确认、冲突显式、七日分布、截止/未排期/逾期风险、stale 提示、空态真实、显式本地偏移）。本会话独立复跑：lib 770 passed、--tests 13 目标全绿、两份 verify 各 11/11、tsc/build exit 0。**附：OD-UI-202 新拟物第一阶段（顶栏+首页+Dock+底材，纯样式层）被执行会话同批实施，与 OD-QS 无交叠、不要求回滚，验收状态「待用户追认」（真实窗口目视 + 本会话 19/19 复跑通过）。** **下一任务：OD-QS-412（任务详情扩展，最后一项功能任务）→ 413（收口）。**

> **执行进展（2026-11-08 七）：** OD-QS-412 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：详情面板四个新区块全部落地——优先级与依据（双徽标+source+用户指定标注+双依据文本）、截止与预计耗时（hard/target 下拉+理由必填 disabled 门禁+耗时留空清除）、依赖与父子（depends_on 编辑+父任务标题解析）、排期历史与调整记录（派生已失效可见+adjustments 逐条展示）；五个调整入口全走 405 IPC 无前端直写。本会话独立复跑：lib 770 passed / --tests 13 目标全绿 / 三份 verify（11/11、11/11、19/19）/ tsc / build 全部 exit 0。OD-UI-202 新拟物第一阶段执行会话已在 STATUS.md 记录真实窗口目视（品牌靛+顶栏/首页范围），PLANS 状态改「已实施，待用户最终追认」。**最后一项：OD-QS-413（全量回归+安全核对+收口）可派发。**

> **执行进展（2026-11-09，阶段竣工）：** OD-QS-413 已实施并通过 ARCHITECT 独立复检（REVIEW.md 2026-11-09 条）：全量命令全绿（cargo --tests EXIT=0、pytest 187+30、tsc、build）；verify_security_phase2 14/14（五 IPC 闸门、AI 无排期写通道、排期仅两条用户触发路径、无敏感日志、无新增豁免）；409 登记的 schedule_adjust/cancel 闸门口径已复核闭环；12 份 verify 全绿；证据落 tests/verification/desktop/evidence/od-qs-413/。未验证事项如实登记（真实模型 importance 质量、OD-QS 新页签真实窗口目视、大量任务性能）。**第二阶段（OD-QS-401～413，13 项任务）全部竣工。待办：用户批准后提交推送（含 OD-UI-202 一阶段，待用户窗口追认）；.codex-attachments/ 建议入 .gitignore 不入库。**

1. **OD-QS-401 Task 字段扩展**（task_store.rs）：新增 importance（高/中/低/待评估，默认待评估）、importance_source（ai/user/rule）、importance_reason、due_kind（hard/target，默认 hard）、depends_on（存在性+无环校验）、estimated_minutes、parent_id、quadrant_override（Q1–Q4）；serde default 兼容旧文件，零迁移；hint 注释改为「模糊时间解析输入（402 启用）」。验收：cargo test --offline --lib（新增 ≥14 项全绿）；cargo test --offline --tests。无前置；阻塞 402/403/404/405/407。
2. **OD-QS-402 紧急性规则引擎**（新增 priority_rules.rs，纯函数无 IO）：compute_urgency(task, now) →（等级+依据文本）：逾期→高（附逾期天数）；hard due 按工作日节奏分档（阈值常量注明依据，不用固定 24/48h）；target due 降档；due 为空时对 hint 封闭模式集推定日期并标「推定」，模式外待评估。测试覆盖方案 §十二.2 全部 12 个时间场景（明确日期/本月底/下周三/明天下午/近期/尽快/无时间/跨月/跨年/闰年/时区/模糊不确定性）。验收：cargo test --offline --lib；新增 tests/verification/desktop/verify_priority_rules_401.py 全绿。阻塞于 401；与 403/404/406/407 并行。
3. **OD-QS-403 四象限分类引擎**（并入 priority_rules.rs）：classify_quadrant：importance 或 urgency 待评估→象限「待评估」不强行归类；override 非空→返回 override 标「用户指定」；已完成/已取消/暂停→active=false（暂停保留重要性）；QuadrantView{象限/重要性/紧急性/双依据/evaluated_at/is_override} + list_quadrant_views 批量。测试覆盖方案 §十二.1 全部 13 场景（含逾期但低重要→紧急高但不强制 Q1、目标时间非硬截止、暂停任务）。验收：cargo test --offline --lib。阻塞于 402；与 404/406/407 并行。
4. **OD-QS-404 AI 重要性建议**（task_extract.py + validate_task_payload，零新增模型调用）：候选 JSON 增 importance（高/中/低/null）+ importance_reason（≤100 字）；提示词补约束：不得仅因领导提出即判高、无截止日期不判低；确认候选落 Task 时写 importance_source=ai。验收：pytest tests -q；cargo test --offline --lib。阻塞于 401；与 402/403/406/407 并行。
5. **OD-QS-405 优先级 IPC 与用户调整**（lib.rs + api.ts + types.ts）：task_set_importance（source=user，reason 必填，追加调整记录）、task_set_quadrant（override 设置/清除+记录）、task_set_due（date+kind+reason）、task_set_estimated、task_set_dependency（存在+无环+不自依赖）；全部过 gate_user_text；拒绝理由对界面可见。验收：cargo test --offline --lib；npx tsc --noEmit。阻塞于 401–403；阻塞 410/412。
6. **OD-QS-406 工作时间配置**（新增 availability.rs + 设置页区块，照 retention.v1.json 模式）：work/availability.v1.json（schema omnidesk.availability.v1）：workdays（默认周一–五）、daily_windows（默认 09:00–12:00 / 14:00–18:00）、breaks、unavailable（日期+时段+原因）、daily_capacity_hours（默认 6、上限 12）、timezone；availability_get / availability_set IPC；无文件=默认值且零写入。验收：cargo test --offline --lib；npx tsc --noEmit。无前置（可与 401 并行开工）；阻塞 408。
7. **OD-QS-407 排期实体**（新增 schedule_store.rs，照 task_store.rs 模式）：work/排期/<entry_id>.md（schema omnidesk.schedule.v1）：id/task_id/start_at/end_at/duration/status（已建议|已确认|已调整|已取消|已完成）/is_locked/created_at/updated_at/adjustment_reason + ## 调整记录 追加段；同任务同时段唯一活跃条目；已确认不可就地改（调整=旧条目置已调整+新条目）；任务完成/取消后未来条目读取时派生「已失效」，文件不改写。验收：cargo test --offline --lib（新增 ≥12 项）。阻塞于 401；与 402–406 并行。
8. **OD-QS-408 排期引擎**（新增 scheduling.rs，纯函数不落盘，落盘归 407/409）：generate_suggestions：输入=活跃任务+象限视图+availability+既有已确认条目+依赖图；按方案 §八.3 十一条优先序（锁定不可覆盖 > 已确认不动 > 硬截止优先 > 依赖先行 > 高重要保留连续时段 > Q2 保底投入 > Q3 批量 > Q4 少占 > 每日容量上限 > 保留机动时间）；产出 suggestions + conflicts（时间重叠/超容量/截止不可满足/依赖倒置/锁定冲突）+ unscheduled（附原因，超最大时段→建议拆分）。测试覆盖方案 §十二.3 全部 16 场景。验收：cargo test --offline --lib。阻塞于 403/406/407；阻塞 409。
9. **OD-QS-409 排期 IPC 与确认流**（lib.rs + schedule_store.rs 状态迁移入口 + api/types）：schedule_generate（重新生成不动已确认/锁定，仅补新建议+过期旧建议）、schedule_confirm、schedule_adjust、schedule_cancel、schedule_lock；任务列表变化检测（generated_at vs 任务最大 updated_at）返回 stale 标志供界面提示。验收：cargo test --offline --lib；npx tsc --noEmit。阻塞于 408；阻塞 411/412。
10. **OD-QS-410 任务四象限页 + 旧占位替换**（TaskManager 新页签 + 删除 weeklyQuadrants 占位）：四格（Q1–Q4+待评估区，文字标识不只靠颜色）；每任务显示重要性/紧急性/截止/状态/确认标记与依据摘要；手动调整走 405 IPC 并标注「用户指定」。**替换范围（用户裁决②）：** 删除 weeklyQuadrants.ts 的条目归类函数与 WeeklyQuadrants.tsx 组件，ScenePage 四象限页签改用任务象限视图；仍被引用的 REPORT_DESTINATION 等常量移至 weekTimeline.ts；动工前按 OD-ARCH-035 搜 2–3 个同类产品布局（借鉴要点 ≤2 句）。验收：npm --prefix desktop run build；新增 verify_task_quadrant_ui_402.py（断言 weeklyQuadrants.ts 已删除且无引用残留）。阻塞于 405；与 411 并行。
11. **OD-QS-411 今日 / 本周计划页**（任务页新页签组，复用象限视图与排期回执类型，不另建数据层）：今日（可用时段/已确认/AI 建议/剩余容量/冲突）+ 本周（七日分布/截止/未排期重要任务/逾期风险）+ 重新生成入口 + stale 提示；空态真实不造数。验收：npm --prefix desktop run build；新增 verify_schedule_ui_403.py。阻塞于 409；与 410 并行。
12. **OD-QS-412 任务详情扩展**（现有 TaskManager 详情面板追加，不重做）：重要/紧急/象限与依据、预计耗时、依赖（父子）、排期历史与调整记录、手动调整入口。验收：npm --prefix desktop run build；npx tsc --noEmit。阻塞于 405/409。
13. **OD-QS-413 全量回归 + 安全核对 + 阶段收口**（照 OD-WR-308 模式）：全量命令；新增 verify_security_phase2.py（红线+掩码覆盖五个新 IPC、AI 无排期写通道、排期失败不影响任务状态、无敏感日志、测试数据全虚构）；证据落 tests/verification/desktop/evidence/od-qs-*/；如实登记未验证事项（真实模型 importance 建议质量、真实窗口目视、大量任务性能）。验收：cargo test --offline --tests；pytest tests -q；npm --prefix desktop run build；npx tsc --noEmit；全部 verify 全绿。阻塞于 401–412，最后执行。

**依赖图：** 401 → 402 → 403 → 405 → {410, 412}；401 → 404（与 402/403 并行）；401 → 407 与 406 → 408 → 409 → {411, 412} → 413。并行线：402/404/407 三路并行（只阻塞于 401）；406 可与 401 同日开工；410 与 411 可并行。串行主线：401 → 402 → 403 → 408 → 409 → 413。

**风险清单：** 分类误判（AI 只建议+依据可解释+用户 override 持续生效）；模糊时间误判（封闭模式集+「推定」标注+模式外待评估）；重要任务被低估（提示词约束+修正通道+待评估不强行归类）；Q2 被挤占（排期规则第 7 条保底投入）；已确认/锁定被覆盖（双保护+生成器永不改写）；重排历史丢失（调整=旧置已调整+新条目，永不删除）；Token 消耗（零新增模型调用）；旧任务兼容（serde default 零迁移）。

**边界：** 不修改 runtime/；不重建任务管理模块；不接外部日历；不自动改用户已确认安排；测试一律虚构数据；四象限页替换旧占位但不动 WeeklyTimeline 时间线本身。

**验证总口径：** 每任务按卡验收命令执行并保留证据至 tests/verification/；阶段收尾（413）跑全量。本会话逐任务独立复检 diff 与测试证据（复跑验收命令，不信转述），结论入 REVIEW.md。

**规划文档：** 已随批准落盘 docs/plans/OD-QS-400/（TASK_PRIORITY_ARCHITECTURE / SCHEDULING_ENGINE_DESIGN / PHASE2_TASK_PLAN / PHASE2_ACCEPTANCE_CRITERIA / PHASE2_ROADMAP，含三、四阶段衔接：任务与排期为 work/ 下稳定形状文件，知识库与文案模块届时只读引用，不预建接口）。

### OD-WR-3xx：第一阶段——工作记录 + AI 任务提取（已批准，2026-11-08；执行会话从 OD-WR-301 派发，本会话负责逐任务复检）

> **触发：** 用户《OmniDesk 第一阶段总任务：工作记录 + AI 任务提取》方案。核心：通过自然语言工作记录，AI 提取任务候选，用户确认后形成可持续管理的任务清单；工作记录与任务分层存储，报告/成果经显式归档进知识库，临时层定期用户确认清理。

**已批准的三项裁决（2026-11-08）：** ① 数据边界按新规划统一：工作记录正文逐条由用户显式触发解析、端点准入默认关闭、送模型前过红线扫描 + mask_privacy，GOALS.md 旧措辞在执行会话同步修订；② 不并存：旧周文件链路收编进新记录库（OD-WR-309/310/311）；③ 存储选型：每条记录一个 Markdown 文件，两层分离（D-061 修订）。

**数据分层与存储（D-061/D-068 候选，随执行进展转正）：**

- 临时层 `$HOME/OmniDesk/work/`（新增 working_root 设置，照 artifact_root 越界校验）：`工作记录/<record_id>.md`（frontmatter：id/title/record_date/created_at/updated_at/category/tags/source/ai_analysis_status/linked_task_ids/draft）+ `任务/<task_id>.md`（frontmatter + 进展 + 状态历史追加段）+ `周文件存档/`（旧周文件字节不动）+ `.trash/`。工作记录与任务彻底移出库/，知识库扫描天然不覆盖临时层。
- 知识层 <vault_root>（既有）：知识卡 + `报告/`（归档目标，新增）+ 归档成果。报告（周报/月报/年报/项目总结）与发布状态文案成果经用户确认的显式归档动作进知识库；草稿与工作记录永远不进。
- 任务候选：宿主暂存目录（照 knowledge_review stage 模式，扫描排除），确认后才生成任务文件。
- 清理（D-068）：无自动删除。宿主圈定候选（已归档报告覆盖期记录 + 完成/取消超 90 天任务；draft 与被未完成任务引用的记录永不入选）→ 用户预览 → 确认后移 work/.trash/。保留期 90 天可配置（work/retention.v1.json）。

**技术决策候选（批准后随执行写入 docs/decisions.md）：** D-061 存储口径（见上）；D-062 关联：ID 双向引用 + source_quote 必须为原文连续片段；D-063 AI 权限：AI 只产候选与建议，宿主落盘，状态迁移仅用户触发；D-064 防重：ai_analysis_status 门禁 + (record_id, quote 哈希) 去重；D-065 解析：引擎异步任务通道 + task_extract.py 结构化 JSON + 宿主 validate_task_payload（枚举/source_quote/null 约束/模糊时间禁落精确日期），历史任务匹配注入未完成任务摘要、低置信度交人工、禁止标题相似自动合并；D-066 安全：红线 + mask_privacy + 端点默认关闭，DataMasker 未接入属未验证事项；D-067 数据边界统一（见裁决①）；D-068 清理口径（见上）。

**任务卡（IMPLEMENTED_RULES.md §1 模板；编号 OD-WR-301 起）：**

> **执行进展（2026-11-08）：** OD-WR-301 已实施并通过 ARCHITECT 独立复检（REVIEW.md 2026-11-08 OD-WR-301 条）：cargo lib 606 passed / 0 failed（新增 41 项），--tests 全绿；process_owner 单测 macOS 偶发失败为既有问题，修复另立 **OD-WR-314（新增）**：把该测试夹具从 sh -c sleep 30 改为 python 冒充引擎（持住 argv），仅改 tests 相关夹具文件；验收：cargo test --offline --tests 连续两轮全绿。可与 OD-WR-302 并行。

> **执行进展（2026-11-08 续）：** OD-WR-302 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：15 个 IPC 命令接线完成，红线扫描 + mask_privacy 统一闸门 gate_user_text 覆盖全部用户文本入参，新增 8 项单测（lib 614 passed / 0 failed）；working_root 暂以 OMNIDESK_WORK_ROOT 环境变量 + 默认 $HOME/OmniDesk/work 实现，settings.v1 正式字段留给后续任务（302 允许文件不含 settings.rs，属范围权衡，已认可）。OD-WR-314 同步实施并通过复检：夹具改为解析 realpath 指向 Python.app 的稳定 argv 解释器，--tests 连续两轮全绿 + 本会话独立复跑全绿。**下一任务：OD-WR-303（工作记录页面）与 OD-WR-305（任务管理页面），可并行派发。**

> **执行进展（2026-11-08 三）：** OD-WR-303 与 OD-WR-305 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：新增 WorkRecordPage（记录/任务/周视图三页签）+ WorkRecordLibrary（快速记录、五维筛选、详情编辑删除、解析入口诚实占位）+ TaskManager（六状态筛选、来源跳转、进展与状态历史、状态变更宿主裁决）；两份 verify 各 15/15，build/tsc 本会话独立复跑 exit 0。305 设计裁决认可：不设新建任务表单，正式任务只能来自候选确认（守住 D-063/D-064）。真实窗口目视验收仍待用户。**下一任务：OD-WR-304（AI 提取引擎链路）；OD-WR-315（working_root 设置项）可并行；306 待 304 完成后派发。**

> **执行进展（2026-11-08 四）：** OD-WR-304 与 OD-WR-315 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：304 交付 task_extract.py（86 行，提示词组装纯函数，不调模型）+ task_extract.rs（590 行，validate_task_payload：quote 连续原文、模糊日期禁落精确日期、置信度 0-1、候选上限、未知字段拒绝）+ 三个 analyze IPC（出提示词 / 落地 / 置失败）；失败路径逐字节不动记录正文（单测断言）；lib 632 passed（净增 18）+ pytest 184 passed 全部本会话独立复跑通过。315 交付 working_root 设置字段（生效顺序 env > settings > 默认）+ 设置界面入口 + work_root_info/set IPC；越界校验三路径统一。**下一任务：OD-WR-306（AI 任务审核工作台，消费 .candidates + 三个 analyze IPC 接线解析按钮）；完成后 307（联动）可派发。**

> **执行进展（2026-11-08 五）：** OD-WR-306 与 OD-WR-307 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：306 交付 TaskReviewPanel（候选队列 + 详情 + 证据高亮 + 批量采纳逐条走同一 IPC）并把解析按钮接上 prepare/submit/poll/apply/failed 全链路；307 交付关联建议（解析注入未完成任务摘要 ≤30 条、match 字段宿主校验、link_candidate 追加进展 + 双向 ID 回写、dismiss_match 保留候选、状态建议只读不自动应用）+ 三日记事归一任务单测；lib 643 passed（净增 11）+ pytest 186 passed + 三份 verify（16/16、16/16、15/15）全部本会话独立复跑通过。**主线功能任务全部完成；剩余：OD-WR-308（全量回归 + 安全核对 + 四份规划文档落盘）收口，随后 309/310/311/312/313 迁移归档清理线待用户单独授权后逐项派发。**

> **执行进展（2026-11-08 六，主线收口）：** OD-WR-308 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：全量回归九项命令全绿（cargo --tests 13 目标 / pytest 186 / build / tsc / 四份 UI+联动 verify / 新增安全核对 verify_wr_security_308 13/13，全部本会话独立复跑通过）；四份规划文档落盘 docs/plans/OD-WR-308/（产品方案 / 技术架构 / 数据与安全 / 迁移归档与清理，附 README 来源说明与未验证事项）。**第一阶段主线（301–308 + 314/315）全部竣工。** 后续：309（存量迁移，须用户单独授权）→ 310 → 311 → 312 → 313（清理，须授权）→ 316（link 半完成回执，小任务）。**建议：9 个任务改动全部未提交，先由用户批准做一次阶段性提交再开 309。**

> **执行进展（2026-11-08 七）：** OD-WR-316 与 OD-WR-309 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：316 交付半完成回执（LinkReport ok/partial + 四项写入标志 + warnings，界面显示补救提示，不引入跨文件事务）；309 交付 worklog_migrate.rs（预览/迁移/回滚/幂等 + 8 项单测 + 设置页迁移入口），**真实一次性迁移已在用户授权下完成**：2026-W45.md（1127 字节 4 条）拆条写入 work/工作记录/，原文件字节不动移入 work/周文件存档/（SHA-256 本会话独立核验一致），源目录 8 个 .bak 未触碰，重跑为空操作。lib 653 passed（净增 10）+ verify 304 升级 18/18 全部独立复跑通过。**下一任务：OD-WR-310（随手记链路切换到记录库）→ 311 → 312 → 313。**

> **执行进展（2026-11-08 八）：** OD-WR-310 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：新增 quicklog.rs（append/set_destination/redate/move_many/merge 改写记录库 + ISO 周聚合 + 旧周文件形状渲染），判定块协议与回执形状不变，周索引从记录库聚合、记录为空时回退旧周文件；**随手记主链路的周文件写路径就此退役**。lib 660 passed（净增 7）+ verify_worklog_unify_305 15/15 独立复跑全绿。**阶段性提交与推送已完成（用户执行）**：d8486784c8（127 文件，含 301–310/314–316、OD-UI-201、308 四文档与全部证据；用户成果/ 已入 .gitignore，敏感词扫描零命中）。**下一任务：OD-WR-311（周视图与周报改读记录库）→ 312 → 313。**

> **执行进展（2026-11-08 九）：** OD-WR-311 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：quicklog 新增结构化 week_view + worklog_week_view IPC；ScenePage 时间线/四象限/指标带优先消费结构化记录条目，记录为空或读取失败才回退旧周文件；WeeklyTimeline 接 parsed 入参不再只依赖 Markdown；周报经 310 的 worklogReadWeek 已走记录库渲染。lib 660 passed + verify_week_from_records_306 12/12 + build/tsc 全部独立复跑通过。**下一任务：OD-WR-312（报告与成果归档知识库）→ 313（清理，须用户授权）。**

> **执行进展（2026-11-08 十）：** OD-WR-312 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：新增 vault_archive.rs（报告/成果归档 + frontmatter 来源五字段 + 同名同源幂等/异源拒绝 + 越界/符号链接防护 + 6 项单测），两个 IPC 注册，前端类型成套。lib 666 passed（净增 6）+ tsc 独立复跑通过。本轮只交付宿主能力与 IPC，界面归档按钮后置（由用户显式触发才有归档动作，符合单向流动口径）。**最后一项：OD-WR-313（临时层清理——动工前须用户单独授权）。**

> **执行进展（2026-11-08 十一，阶段全部收口）：** OD-WR-313 已实施并通过 ARCHITECT 独立复检（REVIEW.md 同日条）：新增 work_cleanup.rs（圈定函数含 draft/被未完成任务引用/待审核候选三重保护 + retention 配置 1–3650 天默认 90 + 预览零写入 + apply 显式 confirm 门禁 + 移 .trash/ 不直接删除 + 6 项单测），IPC 与设置页预览/勾选确认入口成套。lib 672 passed（净增 6）+ verify_work_cleanup_307 15/15 + build/tsc 独立复跑通过。本轮未在真实数据执行清理（符合授权边界——代码就绪，动作等用户逐次触发）。**第一阶段 301–313 + 314–316 全部竣工。待办：最终提交推送（用户批准后执行）。**

1. **OD-WR-301 存储层**（worklog_records.rs + task_store.rs）：记录 CRUD（草稿态 frontmatter、删除进 .trash/）、列表过滤、Task CRUD + 六状态迁移校验（待确认/待开始/进行中/已完成/已取消/暂停，非法迁移拒绝）、状态历史追加段、候选暂存结构、去重键。验收：cargo test --offline --lib（新增 ≥22 项全绿）；cargo test --offline --tests。阻塞 302/304/309；无前置。
2. **OD-WR-302 IPC 接线**（lib.rs + api.ts + types.ts）：记录与任务/候选全部 IPC 命令注册 + 前端类型；候选确认前不进正式清单（集成断言）。验收：cargo test --offline --lib；npx tsc --noEmit；npm --prefix desktop run build。阻塞 303/305/306/309；阻塞于 301。
3. **OD-WR-303 工作记录页面**：列表（日期排列 + 日期/分类筛选 + 关键词检索 + 解析状态 + 关联任务数）、快速记录（自由文本/可选日期/草稿）、详情（Markdown 渲染/编辑/删除/解析入口）。Aetheria 样式，动工前按 OD-ARCH-035 搜 2–3 个同类布局。验收：npm build；新增 verify_work_record_ui_301.py。阻塞于 302；与 304/305 并行。
4. **OD-WR-304 AI 提取引擎链路**（engine task_extract.py + 宿主 validate_task_payload + IPC worklog_record_analyze）：结构化候选 JSON、失败置 ai_analysis_status=failed 不动记录、重复解析门禁、quote 篡改拒绝。验收：cargo lib 单测；pytest tests -q。阻塞 306/307；阻塞于 301；与 303/305 并行。
5. **OD-WR-305 任务管理页面**：六状态筛选列表、详情（来源跳转/状态历史时间线/进展列表）、状态变更操作（非法迁移报错可见）。验收：npm build；新增 verify_task_manage_ui_302.py。阻塞于 302；与 303/304 并行。
6. **OD-WR-306 AI 任务审核工作台**：待审核列表（置信度/不确定原因）、详情展开（原文证据高亮、字段就地编辑）、确认（可改字段）/拒绝/关联采纳与拒绝、批量确认走同一校验。验收：npm build；新增 verify_task_review_ui_303.py。阻塞 307；阻塞于 302/304。
7. **OD-WR-307 记录 ↔ 任务联动**：解析注入未完成任务摘要、匹配建议进审核台（采纳 = 追加进展 + 双向 ID 回写）、AI 状态变更建议只作候选、三日记事归一任务测试、低置信度交人工、进展不覆盖历史。验收：cargo lib 单测；新增 verify_task_linkage_304.py。阻塞于 304/306。
8. **OD-WR-308 全量回归 + 安全核对 + 规划文档落盘**：全量命令；安全断言汇总（红线/掩码/AI 无写通道/端点默认关闭/测试数据全虚构）；按方案 §十二 落盘四份规划文档；未验证事项（真实模型质量、DataMasker、真实窗口）如实记录。验收：cargo --tests；pytest；npm build；tsc；四个新 verify 全绿。最后执行。
9. **OD-WR-309 存量周文件一次性迁移**：解析既有周文件拆条写 work/工作记录/，校验条数一致后原文件移 work/周文件存档/（字节不动）；失败整体回滚；幂等。**执行前须用户单独授权**。验收：cargo lib（迁移用例：拆条/归档/回滚/幂等）。阻塞 310；阻塞于 301/302。
10. **OD-WR-310 随手记链路切换**：判定块协议不变，追加/移动/并入改写记录库；周索引从记录库按 ISO 周聚合；worklog.rs 周文件写路径退役、读函数保留供迁移。验收：cargo lib；新增 verify_worklog_unify_305.py。阻塞 311；阻塞于 309。
11. **OD-WR-311 周视图与周报改读记录库**：WeeklyTimeline/四象限数据源换记录库按周过滤；周报源切换。验收：npm build；新增 verify_week_from_records_306.py。阻塞 312；阻塞于 310。
12. **OD-WR-312 报告与成果归档知识库**：IPC archive_report_to_vault（确认后定稿复制进 <vault_root>/报告/，frontmatter 记来源周期，同名拒绝同内容幂等）+ archive_artifact_to_vault（仅发布态，草稿拒绝）。验收：cargo lib；tsc。阻塞 313；阻塞于 302/311。
13. **OD-WR-313 临时层清理**：圈定函数（含 draft 与被引用保护）、IPC work_cleanup_preview（零写入）/ apply（确认后移 .trash/）、retention 配置。验收：cargo lib；新增 verify_work_cleanup_307.py。阻塞于 312。

**依赖图：** 301 → 302 → {303 ∥ 305}；301 → 304 → 306（+302）；304/306 → 307；302 → 309 → 310 → 311 → 312 → 313；{303,305,306,307,313} → 308。两线并行：主线 301→302→303/305，副线 309→310→311→312→313。

**风险清单：** R1 已消除（无双入口）；R2 AI 错误提取（枚举 + quote 强校验 + 全候选人工）；R3 重复创建（门禁 + 哈希去重）；R4 状态错误变更（白名单迁移 + AI 不直接落正式状态）；R5 时间识别（deadline_type 五枚举，模糊时间禁落精确日期）；R6 敏感数据（红线 + 掩码 + 端点默认关闭，DataMasker 未验证）；R7 OD-UI-201 未提交改动叠加——界面类任务（303/305/306）建议待用户验收/提交 OD-UI-201 后启动，301/302/304 不受影响；R8 扩展（hint 字段本期只存不用）；R9 迁移损坏（字节归档 + 失败回滚 + 单独授权）。

**边界：** 不修改 runtime/；不改引擎协议；旧 worklog.rs 判定块协议与知识库红线机制（OD-ARCH-141/144）不变；清理与迁移均为用户显式触发，无自动删除；测试一律虚构数据。

**验证总口径：** 每任务按卡验收命令执行并保留证据至 tests/verification/；阶段收尾（308）跑全量。本会话（ARCHITECT）逐任务复检 diff 与测试证据，结论入 REVIEW.md。

### OD-UI-201：全应用去「历史会话」+ 文案写作 / 首页 / 工作记录文案与版式规范（已实施，2026-11-08，待用户验收）

> **触发：** 用户在真实界面复测后提出三条：①标准与专业 Profile 都去掉「历史会话」分页；②文案写作、首页、工作记录再设计规范；③去掉 AI 味，删除需要副标题或标签才能看懂的解释性说明。

**范围与边界：** 只改界面文案、布局与入口；不改数据模型、审核/检索语义、模型路由或业务规则。用户数据目录（~/OmniDesk）不覆盖；旧 decl.yml 里残留的 sessions 仍可解析，只在载入后隐藏。

**实施要点：**

1. **历史会话下线：** Dock / App 路由 / 默认入口 / 起手骨架 / 出厂声明移除 sessions；profile_decl 保留向后兼容解析，载入时丢弃该入口；删除不再引用的 Workspace / ProjectRail / Sessions 组件。
2. **文案写作：** 去掉全部英文副标题、卡序号与口径说明；「状态管线」改为「稿件」、「灵感架」改为「写作对话」；空态压到一句话；指标卡只保留图标、标题与数值。
3. **工作记录：** 四格指标卡统一为 Aetheria 白色浅卡；去掉英文副标题与 01–04 序号；四象限轴标改中文「紧急/不紧急」「重要/一般」；「历史对话」改为「本场景对话」，说明性短句删除。
4. **首页：** 删除标准模式说明条与「查看」跳转；移除未开放的语音按钮与说明；输入框占位改「输入消息…」；底部权限/Token/规则/目录状态行压到「仅未开启时提示」；顶栏 IPC ON/OFF 改为连接状态圆点。
5. **Dock：** 悬停提示只显示入口名，不再渲染「模型起草」「本机 Markdown」等过程说明；用户 decl 中的 note 字段保留但不再展示。

**验证：** npm --prefix desktop run build；cargo test --offline；新增 verify_ui_simplify_v1.py 24/24；写作 84/84、工作记录 093 106/106、097 34/34、100 32/32；工作记录 089 83/83、132 43/43；知识库 46/21/32；UI 美化 17/17；Profile 106 43/43。真实窗口目视核对文案写作、工作记录、首页。

**未决：** 真实窗口自动化脚本（055/078/077/122/123）依赖 Xvfb，本轮未在 macOS 上重跑；等待用户按界面验收。

### OD-KB-201：知识库重构规划——LLM-Wiki 定位与四类资产架构（已竣工：S1-S5 实施并通过审查，用户四步复测通过，收口提交 59ad3f55aa 已核验；已推送 origin/main，正式竣工）

> **触发：** 用户与 ChatGPT 完成知识库定位讨论（OD-KB），在 v0.2.10（OD-ARCH-145 发布）基础上重新规划知识库。核心结论：知识库是 AI 使用的 LLM-Wiki 知识基础设施，不是传统笔记软件；人负责治理与审核，AI 负责知识生产与维护。

#### 现状盘点（已核实，2026-11-07）

- 数据链路： `knowledge/`（仓库权威源）→ 宿主校验建卡（knowledge_ingest.rs，模型只出建议、宿主落盘）→ vault_index.rs 反链/锚点索引 → vault_search.rs 全文检索 → 对话两跳检索带引用（OD-ARCH-141/144）。
- 接收入口： KnowledgeIngestPanel（OD-ARCH-146 S3 已迁入知识库页顶部操作区，prepare→run→apply）。
- 界面现状： KnowledgeVaultPage 为传统笔记软件形态——默认进入编辑器（EditorMode），「笔记/图谱」两视图，无审核队列、无驾驶舱、无质量页。
- 架构边界已符合解耦要求： 知识是 Markdown+YAML 文件；模型调用在 engine/（可替换）；模型输出经宿主 validate_payload 校验后落盘（knowledge_ingest.rs:226）；宿主不依赖具体模型。本轮在此基础上补边界与目录，不推翻。

#### 产品定位（本计划的范围声明）

- 知识库服务对象是 AI 与其他 OmniDesk 工作模块；人的角色是治理者：提供资料、审核准入、裁决冲突、配置规则。
- 界面优先级： 知识审核中心 > 知识资产；知识驾驶舱 > 知识图谱。图谱降为驾驶舱内只读健康视图，不再是独立一级页。
- 开发优先级： 先打通「原始资料 → Wiki → AI 检索」完整闭环，图谱类可视化后置。
- 架构优先级： 知识内容与模型、Agent、前端彻底解耦；更换模型不影响知识资产。

#### 信息架构（五个一级页，替代现有「笔记/图谱」两视图）

1. **知识驾驶舱**（默认落地页）：待办计数（待审核/冲突/异常，真实数据，空态如实显示）、AI 最近活动、知识库健康概览（可折叠展开链接索引图）。
2. **知识审核中心**：四个队列——资料准入 / 新知识确认 / 知识更新 / 冲突异常；每项展示来源对照，供裁决。
3. **知识资产**：只读浏览为默认，编辑降级为显式动作；沿用现有读取、检索、反链命令。
4. **AI 运行与质量**：整理任务记录、失败原因、质量与过期风险。
5. **知识库配置**：沿用现有 vault root 设置。

> **修订（2026-11-07，用户定稿预览 v2）：** 信息架构精简为**三个一级页**——知识驾驶舱（默认）/ 知识审核 / 知识资产。原「AI 运行与质量」并入驾驶舱「知识库健康 · AI 运行质量」卡（四格：来源可追溯 / 关联覆盖率 / 索引完整性 / AI 任务健康，每格带管理动作入口）；任务失败同时进驾驶舱顶部指标行与「需要你处理」。原「知识库配置」不设独立页，融入 OmniDesk 顶栏设置（复用既有 vault root 目录选项）。驾驶舱顶部指标行固定五格：原始资料数 / Wiki 知识条目数 / 待审核 / 待处理冲突 / AI 任务失败。「需要你处理」与「AI 最近工作」左右并排。UI 美化后置：预览布局已获用户认可，样式细节在功能完成后统一打磨。

#### 分工（用户指令，2026-11-07）

本会话（ARCHITECT）只负责规划与审核；S1-S5 的代码实现由用户另开的执行会话完成。每阶段完成后由本会话对照本计划与实际 diff 独立审查，结论写入 REVIEW.md。执行会话可直接引用本条目的 S1-S5 定义、边界与验证要求。

#### S1 执行要点（供执行会话直接使用，2026-11-07 预研已核实可行）

- 改动面：`desktop/src-tauri/src/knowledge_ingest.rs`（主要）与 `desktop/src-tauri/src/lib.rs`（新增一个 IPC 命令），预计净增 350-450 行。
- `RAW_DIR` 常量（值 raw）加入 `init_vault` 的 6 目录幂等骨架；索引模板追加「## 原始资料」小节。
- 新增 `admit_raw_files(root, files, utc_offset_minutes)`：内容 SHA-256（复用 crate::sha256::sha256_hex，不引新依赖）→ 与索引「原始资料」小节既有哈希比对，同哈希拒绝（重复资料）→ 同名不同内容用「原名-哈希前12位」换名新增，绝不覆盖 → 专用原子写入（临时文件 + fsync + 硬链接发布，目标存在即拒绝；不做 .md 形态限制，raw/ 可收 PDF 等）→ 索引行格式：列表项（原名链接到 raw/原名）+ 准入时间（RFC3339）+ sha256 + 字节数。索引保存失败时回滚本轮已落盘 raw 文件，不允许「文件在、索引没记」中间态。
- 新增纯函数 `rfc3339_local(unix_seconds, utc_offset_minutes)`（civil 历法换算，不引时间库）；偏移复用宿主现有 utc_offset_minutes 读取逻辑。
- 新增 IPC 命令 `knowledge_raw_admit`（文件字节列表经 IPC 传入，先 init_vault 再准入），注册进 invoke_handler。
- 新增测试至少 5 项：RFC3339 格式（正/负偏移、纪元）；准入建文件并记索引；同哈希重复拒绝；同名不同内容换名且原文件不动；非法文件名（路径逃逸/隐藏/空名）拒绝；重复准入索引不变。
- 注意：Rust byte-string literal 不接受非 ASCII，测试夹具的中文内容用转义字节或英文替代。

#### S2 执行要点（供执行会话直接使用，2026-11-07 随 S1 审查一并下发）

- 目标：人工治理记录文件化。落盘位置 `knowledge/governance/`（vault 根下，与 raw/ 平级；不是仓库 knowledge/ 目录），目录由宿主创建，模型与前端都没有写通道。
- 记录三类文件：准入记录（raw_admissions.v1.md）、审核与裁决记录（review-decisions.v1.md）、规则（admission-rules.v1.md）。全部 Markdown + 行内元数据（沿用索引行的轻量格式），不引入数据库。
- 新增 desktop/src-tauri/src/knowledge_governance.rs：append_record(root, kind, entry)（幂等建目录 - 追加一行 - 失败即报错不落半截）、list_records(root, kind)；行格式：`- <RFC3339> | <动作> | <对象> | sha256:<hash前12位> | <说明>`。时间戳复用 rfc3339_local。
- S1 的 admit_raw_files 每次准入成功后调用 append_record 记一条准入记录（含哈希与原名）；这是治理层与 raw 层的挂钩点，其余链路不动。
- 新增 IPC 命令 knowledge_governance_list（kind 参数）与内部写入口；**不做**任何让模型或前端直接写治理文件的命令，AI 无写权是代码级事实。
- 脱敏：治理记录由 mask_privacy 处理后再落盘（与工作记录链路同一约束）。
- 测试至少 6 项：目录幂等；append 后 list 读回一致；损坏行被跳过不 panic；admit_raw_files 触发准入记录（集成断言）；治理目录不在 vault 扫描结果中显示为笔记（或按排除目录处理，执行会话二选一并写明理由）；mask 钩子被调用。
- 边界：不改 runtime/；不做规则编辑界面（归 S3/S5）；不动 141/144 红线链路。

#### S3 执行要点（供执行会话直接使用，2026-11-07 随 S2 审查下发；定稿预览 .preview/od-kb-201.html 是唯一界面基准）

- 范围：把 KnowledgeVaultPage 从「笔记/图谱两视图 + 编辑器默认」重构为定稿三页——知识驾驶舱（默认落地）/ 知识审核 / 知识资产。UI 以 .preview/od-kb-201.html 布局为准（Aetheria OS：浅色 MD3、半透明白卡片、白描边、无背景 blur、顶部胶囊页签、左列表右详情）；细节允许实现取舍，布局结构不改。
- 页签骨架：三页用顶部胶囊页签切换（复用预览的 tab 结构）；离开知识库页的脏检查（knowledgeDirty）逻辑保留：只在用户显式进入编辑态后生效。
- 驾驶舱（默认）：顶部五格指标（raw/ 文件数、知识库/ 卡片数、待审核数、待处理冲突数、AI 任务失败数），全部取真实数据，空态如实显示 0 或「暂无」；「需要你处理」与「AI 最近工作」两卡并排（待办点击跳审核页对应队列）；「知识库健康 · AI 运行质量」卡四格（来源可追溯率、关联覆盖率、索引完整性、AI 任务健康），每格带动作入口（查看缺失 / 触发关联维护 / 重建索引 / 查看任务记录）。数据来源：knowledge_vault_scan_for（raw/ 与卡片计数、索引行统计）、knowledge_governance_list（待审核/裁决计数）、既有任务记录。没有的任务类型如实显示「暂无」，不造演示数据。
- 审核中心：队列标签先做两个（新知识确认 / 冲突与异常），对应 governance review_decisions 中未决记录与 vault 冲突检测；每条展开就地显示详情（新增显示 AI 提取结果 + 原文引用，冲突显示两份来源对照）；操作按钮（通过并纳入 / 需要修改 / 驳回）调用宿主命令落 governance 决策记录。S1 的 knowledge_raw_admit 在本阶段接前端：知识库页提供「上传原始资料」入口（Tauri 文件选择），选定文件经 IPC 字节准入 raw/，回执展示准入/拒绝明细。
- 知识资产：左列表右详情；列表来自 vault scan（wiki 侧卡片 + raw/ 文件分组显示）；详情默认只读展示（元数据行、正文带来源引用、关联与反向链接复用 vault_index 命令）；「编辑此条」显式按钮进入既有编辑器流程（编辑/保存/冲突检测全走现有 vault_write 链路）。
- 图谱：KnowledgeGraphView 从一级入口撤下；在驾驶舱健康卡「知识关联覆盖率」格内提供「查看链接索引图」动作打开只读图谱（复用现有组件），不再是独立页签。
- API/类型：desktop/src/api.ts 与 types.ts 补 knowledge_raw_admit、knowledge_governance_list 的调用与类型；verify 脚本断言前端只调只读治理命令。
- 测试：verify_knowledge_ingest_ui_141.py 扩展或新增 verify_knowledge_hub_ui_s3.py：三页签结构、驾驶舱五格与两卡并排、审核两队列与展开详情、资产只读默认 + 显式编辑、图谱非一级入口、raw 上传入口、治理只读调用；cargo 侧若新增宿主命令才补测试。
- 边界：不改 runtime/；不改引擎协议与红线机制；不动 141/144；「AI 最近工作」的数据源若当前无任务记录接口，就显示「暂无运行记录」空态，不得为布局编造记录。
#### S4 执行要点（供执行会话直接使用，2026-11-07 随 S3 审查下发）

- 前置（必须先做）：真实界面端到端验证（S3 审查遗留的第一优先项）。由用户在真实窗口走「上传 raw - 整理收件箱 - 审核通过 - 资产可见 - 对话检索引用」，执行会话记录每步实测结果与问题；发现缺陷先修复再进 S4 功能。
- 前置通过后，S4 按定稿结构实现「知识库健康 · AI 运行质量」卡的三项增强（并入驾驶舱，不设独立页）：
  1. 任务记录列表：驾驶舱健康卡内可展开最近任务（engine_task_records 已接入，补失败原因字段与滚动列表；S3 已有雏形，本轮补全）。
  2. 过期/冲突检测：宿主新增确定性检查（无模型调用）——扫描索引行日期字段与 raw 准入时间，标记超过可配置阈值的条目为「可能过期」；冲突沿用 S3 buildKnowledgeConflicts。检测结果进驾驶舱四格的 AI 任务健康与「需要你处理」列表。
  3. 检索抽检入口：驾驶舱提供检索框（复用 vault_search 全文检索），展示命中与引用原文上下文，供人验证「AI 能否正确检索」。不做向量/嵌入（计划边界不变；是否升级等本轮实测证据）。
- 数据诚实：抽检结果与过期标记都是确定性产物，可复算；不生成任何示意数字。
- 测试：过期阈值检测单测（固定时间注入）；抽检走既有 vault_search 断言；verify 脚本断言健康卡三项增强的接线与「无独立质量页」。
- 边界：不改 runtime/；不改引擎协议与红线机制；不加新依赖；S2 的跨文件事务限制维持记录在案，除非端到端实测暴露问题。
#### S5 执行要点（修订版，2026-11-07；用户端到端实测发现 3 缺陷，S5 从「迁移收尾」改为「缺陷修复 + 收口」）

- 修复 A（高，阻断整理）：建卡规范与宿主校验统一到审核暂存流程。JSON_OUTPUT_SPEC 的 sources 说明改为「指向本批次真实存在的文件（inbox/ 或 raw/）」；validate_payload 的来源校验同步放行 inbox/ 与 raw/；归档动作保持「审核通过时移 原文/」。卡片长度：把规范里的目标改为 280 字内，宿主上限放宽到 350，两端同口径。
- 修复 B（中）：raw 索引行不进链接解析。推荐 buildKnowledgeConflicts 排除「知识库/索引.md 原始资料小节内指向 raw/ 的未解析链接」；vault_index 不动（改动面最小）。
- 修复 C（低，随 A 顺带）：对话检索的 notice 文案区分「知识卡命中」与「原始资料行命中」，避免模型答文口径混淆；不改检索逻辑。
- 回归：cargo 全量 + 全部 verify 脚本 + 用户重跑真实端到端（同一份真实材料：上传 - 整理 - 审核 - 检索全通）。
- 收口：端到端通过后一次性提交（S1-S4 全部改动），提交信息按 OD-KB-201 惯例；提交前用户批准。
#### S5 补充（2026-11-07，待批准）：卡片长度口径

执行会话已按 280（模型）/ 350（宿主）落地，并把「单卡超长」从整批拒绝改为单卡不进审核 + 警告。用户反馈真实写卡仍常超。评估结论：

- 现状问题：建卡规范要求每张卡含 5 个小节（核心概念 / 依据 / 提要 / 边界 / 待确认），280 字中文按 5 节分摊约每节 48 字，制度类内容普遍写不下；这是规范与上限的冲突，不是模型不听话。
- 建议方案（二选一，批准后执行会话改 3 处：engine JSON_OUTPUT_SPEC、宿主 CARD_MAX_CHARS、skills/knowledge-agent 与 knowledge-search 的规范文案）：
  A. 上限上调：模型目标 320 字，宿主上限 420 字（保留约 30% 容差带）。检索侧仍是 2 卡 / 3000 字预算，两卡最多 840 字，预算不受影响。
  B. 规范瘦身：上限维持 280/350，但把 5 节改为「按需」——事实/概念类卡 2-3 节，制度类才用全 5 节；并明确「一张卡只答一个问题，写不下就拆卡」。
- 推荐 A + B 组合：320/420 的数值给实际写作空间，按需小节减少无意义凑字；两者都在下一轮审核中一并验收。
- 不改变校验口径：长度仍按 frontmatter 之外正文计，超限卡不进审核并给出字数与建议拆卡提示。
#### S5 补充二（2026-11-07，用户已批准方向）：长度上调 + 自动拆卡

用户决定：上调数值，且再超限时模型自动拆成「卡片 1 / 卡片 2」或「（上）/（中）/（下）」，确保知识入库、方便 AI 读取、上下文不超限。执行要点（交执行会话）：

- 数值调整（三处同改）：模型目标 320 字（JSON_OUTPUT_SPEC）；宿主上限 420 字（knowledge_ingest.rs CARD_MAX_CHARS，保留约 30% 容差）；skills/knowledge-search 与 knowledge-agent 规范文案同步 320。检索预算核算：KNOWLEDGE_MAX_CARDS=2 × 420 = 840 字，KNOWLEDGE_CONTEXT_MAX_CHARS=3000 不变，两卡全注入也不超预算。
- 自动拆卡规范（写进 JSON_OUTPUT_SPEC 与 skill 文案）：单卡正文超过 320 字时，拆为多张卡并命名「原标题（上）/（中）/（下）」（两段用 卡片 1 / 卡片 2 语义即「（一）/（二）」，执行会话统一选一种后缀并在三处文案一致）；每张仍须独立 title、relative_path、index_line、sources；拆分卡之间用 index 行的「参见」互相链接。宿主不做强制拆分（宿主只校验：单卡超 420 仍拒绝该卡并提示拆卡），拆卡是模型职责。
- 审核侧：拆分卡在审核中心按批展示，标题后缀（上）（中）（下）帮助人快速识别同源拆分；不改变「单卡超限不进审核、警告随审核项展示」的既有行为。
- 测试：engine 单测断言拆卡提示文案与后缀一致性；Rust 侧补一条「3 张拆分卡同批校验通过、index 三行互相对应」的单测；verify 脚本断言三处 320/420 口径一致。
- 边界：不改 KNOWLEDGE_MAX_CARDS（检索仍最多引用 2 卡）；不改红线机制；上下文预算 3000 字不涨。
#### S5 补充三（2026-11-07，用户复测发现缺陷 D）：已处理材料自动排除 + 拆卡上限自指

缺陷 D：raw/ 材料审核纳入后会重复进整理批次（选材不读治理台账）；长文拆出的卡仍可超 420 字被跳过。修复要求（交执行会话）：

- **已处理排除**：选材时查治理台账——该 raw 路径存在 approve_card 记录（或独立 processed_raws.v1.md 登记表，执行会话选一并写明理由）且当前文件哈希与 raw_admissions 登记一致 → 跳过；哈希变化（文件更新）→ 重新进批次；无记录 → 进批次。inbox 归档机制不变。批次明细显示「已处理跳过 N 份」。
- **拆卡上限自指**：三处规范（JSON_OUTPUT_SPEC、knowledge-agent、knowledge-search）补「拆出的每张卡正文同样 ≤320 字；长文按主题段先拆再压，不允许拆完仍超」。
- 测试：Rust 单测覆盖「有 approve_card 且哈希同 → 排除 / 哈希变 → 回归 / 无记录 → 入选」三态；verify 断言批次明细含已处理跳过计数。
- 边界：不改 vault_index、检索与红线机制；治理台账仍只有宿主写通道。






#### 分阶段实施（每阶段独立可验收；S1–S3 构成完整闭环）

- **S1 资料接入层（Raw）**： vault 目录分出 raw/ 与 wiki/ 两类资产目录，init_vault 建目录骨架（幂等）；整理流程改为：资料进 raw/（带来源、导入时间、哈希、重复检测）→ AI 建议卡进 wiki/（现有校验落盘不变）；raw 文件宿主与模型均不可覆盖。前端入口不变（收件箱面板），只换落盘目标与追溯字段。
- **S2 治理层（Governance）**： 决策与规则记录文件化，落 `knowledge/governance/`：审核记录、冲突裁决、准入规则；AI 无写权，宿主落盘（沿用 mask_privacy 既有约束）；审核动作产生记录，可追溯到资料与卡。
- **S3 审核工作流（界面）**： KnowledgeVaultPage 重构为定稿的三页信息架构（驾驶舱 / 审核 / 资产；质量并入驾驶舱健康卡，配置走顶栏设置）；审核中心先接现有 ingest apply 流（apply 前出审核卡，人确认后落盘），驾驶舱计数取自扫描与治理记录的真实数据。
- **S4 质量与运行页**： AI 任务记录、失败原因、过期/冲突检测、检索抽检入口；数据来自引擎任务与索引检查，不新增模型调用。
- **S5 迁移与收尾**： 旧笔记兼容视图（存量文件不迁移、不删除）；图谱入口收进驾驶舱；全量回归与验收。

#### 边界

- 不修改 runtime/；不改引擎提交/取消协议；模型输出仍一律经宿主校验落盘。
- 案件、线索、画像等红线材料不进入知识库（红线机制不变，OD-ARCH-144 已修复扫描对象）。
- 知识格式保持 Markdown + YAML front-matter，可脱离 OmniDesk 使用。
- S1–S3 前不做检索增强（向量、嵌入）；现有全文检索加引用足够支撑闭环，是否升级按 S4 实测决定。

#### 验证

- S1：init 幂等单测、哈希与重复检测单测、raw 不可覆盖断言、ingest UI 脚本扩展。
- S2：治理记录写入/读取单测、AI 无写权断言、脱敏单测。
- S3：五页结构静态断言脚本、审核流端到端（合成资料：准入→建卡→引用检索命中）、驾驶舱空态与计数断言。
- 每阶段： cargo test --offline 全绿 + npm build + 对应 verify 脚本；S3 完成后用户真实验证。

### OD-ARCH-146：对话后台存活 + 输入框换行 + 整理入口迁移（已批准并实施，待真实界面验证；知识库页面重构另行立项）

> **触发：** 用户 0.2.10 实测反馈四项：① 对话等待中切换页面后再回来，回复丢失；② 长文本输入不换行；③ 「整理收件箱」按钮放在对话框工具行位置不合理；④ 知识库页面需要按功能重构。

#### 已核实事实（静态代码检查）
1. **切页丢回复**：[App.tsx](desktop/src/App.tsx:396) 按场景条件渲染，切到工作记录/知识库等页面时 HomeChat 整个卸载；进行中的任务号 activeTaskId 只存在 HomeChat 的 useState 里，不落线程档案。回来后 key={viewKey} 重建组件，没有任何途径恢复轮询；流式事件在卸载期间也被丢弃。任务本身仍在引擎里继续跑完，只是界面永远拿不到结果。
2. **输入不换行**：[HomeChat.tsx](desktop/src/shell/HomeChat.tsx:2912) 用的是单行 `<input>`，长文本只能横向滚动。
3. **整理入口位置**：KnowledgeIngestPanel 在 OD-ARCH-141 S6 时被放在了首页对话输入框工具行（HomeChat.tsx:3012），动机是"随手丢材料后不出对话流就能整理"；但功能归属是知识库，且该面板自带 180 秒轮询，切页同样会中断。用户判断正确，应迁走。
4. **知识库页面现状**：KnowledgeVaultPage 只有「笔记 / 图谱」两个视图，收件箱材料与整理流程完全没有入口；顶部一排按钮（新建笔记/刷新扫描/重建索引/设置）不按功能分组。

#### 方案（四件事）

**S1. 对话后台存活（修①）**
- SceneThread 增加 inflightTaskId 字段：提交拿到任务号后写入线程档案，任务终态后清空。
- HomeChat 挂载时（含切页回来）：若线程带 inflightTaskId 且界面没有活跃任务 → 恢复 activeTaskId 继续轮询终态、回读结果。卸载期间错过的事件不追，终态结果由 task_result 兜底拿回。
- 提交期间切页（还没拿到任务号）：submitTask 的 promise 回调把任务号照常写入线程档案（组件卸载不中断 promise）。
- 卸载不取消任务：引擎侧任务继续执行，与"不自动停止"哲学一致。

**S2. 输入框自动换行（修②）**
- `<input>` 换成自适应高度 `<textarea>`：1 行起，内容变高最多 4 行后内部滚动；Enter 发送、Shift+Enter 换行（沿用现有键盘逻辑）。

**S3. 整理入口迁移（修③）**
- 从对话输入框工具行移除 KnowledgeIngestPanel。
- 整理收件箱成为知识库页面的主操作（本轮落点：知识库页面顶部操作区；页面重构另行立项）。

**S4. 知识库页面重构——本轮不做（用户决定：先看案例再定方案）**
- 本轮只在知识库页面顶部操作区加入「整理收件箱」面板（S3 的落点），不调整视图结构、不动现有按钮布局。
- 页面重构（收件箱/卡片/图谱三视图等）待用户参考案例后另行立项。

#### 边界
- 不修改 runtime/；不改引擎提交/取消协议；不改 145 的 starting 兜底逻辑。
- 知识库卡片数据结构与磁盘格式不变，纯前端页面重组。
- 工作区改动保持未提交，验收后由用户决定提交时机。

#### 验证
- S1：静态断言脚本覆盖「提交后写 inflightTaskId、终态清空、挂载恢复轮询」三条路径；手动验证：提交后切到知识库再切回，回复照常出现。
- S2：构建通过 + 手动粘贴长文本确认自动换行、Shift+Enter 换行、Enter 发送。
- S3：对话工具行不再出现整理按钮；知识库页面内整理流程完整走通（prepare→run→apply）。
- 全量回归：npm build + 现有 UI 静态断言脚本 + cargo test --offline。

### OD-ARCH-145：续接会话偶发挂死诊断与在途任务可恢复（已批准并实施，待真实复现验证）

> **触发：** 用户实测素材写作，第 4 轮提交后一直"生成中/未收到输出"（等待 1124 秒无 TTFB）。同模式已在 0.2.8（11-05 22:30）与 0.2.9（11-06 10:01）各出现一次，三次成功后第四次卡死。判据：两代版本都发生、成功与卡死同模式混杂 → 指向续接会话（resumed）在 DSH 侧偶发不响应，非 141/142 回归。

#### 已核实事实
1. 卡死任务特征：state=starting、seq=0、ttfb=None、masker 字段为空——卡在宿主 with_engine_call（引擎冷启动/提交）阶段，连引擎的 masker 都没执行到。
2. 同会话此前三次提交均成功（masker 正常记录），排除词典/脱敏问题。
3. 引擎 stderr 无落盘（piped 给宿主但宿主未持久化）——卡死时无证据可查，诊断盲区。
4. 用户端显示"已静默 60 秒 · 不会自动停止"——宿主心跳提示在，但没有对 starting 超时的兜底。

#### 方案（三件事，最小改动）

**A. 引擎 stderr 落盘（诊断盲区补上）**
engine_supervisor 拉起 python 引擎时把 stderr 写入 logs/engine-stderr.log（滚动保留最近 1MB×3），与 dsh-settings.log 同惯例。这是后续一切诊断的前提。

**B. starting 阶段兜底超时（用户可控）**
提交后 N 秒（默认 120s）仍 state=starting 且无任何事件 → 前端提示"启动已超过 2 分钟，可点取消重试；连续两次建议重启应用"。不在宿主自动杀任务（保持"不自动停止"哲学），只给用户可见的动作建议。UI 已有取消按钮，仅加提示。

**C. 取消后可干净重试**
用户点取消 → 引擎 omni.cancel_task → 任务落 failed → 下一次提交新建会话（避免再进同一卡死的 resumed 通道）。验证取消在 starting 阶段有效（当前 cancel 在 starting 的行为待核）。

#### 验证
- A：日志文件出现且含引擎启动信息；单测覆盖路径。
- B：模拟卡死（假引擎不响应）→ 120 秒后前端提示出现。
- C：卡死任务取消 → failed 落定 → 新提交走 new 会话成功。
- 真实复现：用户第 4 轮卡死时取 engine-stderr.log 给 ARCHITECT 定位 DSH 侧根因（DSH 本体不动，runtime 保护）。

#### 边界
不修改 DSH runtime；不改 resumed 机制本身（数据由 141/142 链路共享）；B 不自动杀任务。
### OD-ARCH-144：知识整理链路红线扫描修复（hotfix，已批准并实施，待真实验证）

> **触发：** 用户 0.2.9 实测"整理收件箱"被拒：red_line_rejected 命中 14 处。根因实锤：Panel 把 assemble 组装后的完整提示词（含建卡规范全文 4422 字）当 promptText 提交，engine_submit_task 的 scan_target 未传 user_input_text → 退回扫描整个提示词 → 规范文本里"案件（3）线索（5）审查调查（1）举报（1）涉案（1）未公开（2）案件线索（1）"合计 14 处命中 → 拒绝。**用户材料本身零命中。**这是 141 实施时的接线缺陷（扫错了对象），不是红线机制的问题。

#### 方案（最小修复，两处）

1. **KnowledgeIngestPanel.tsx**：submitTask 增加 user_input_text 参数 = inbox 材料正文拼接（与 promptText 的来源一致——prep.files 的 content）。红线扫描回落在用户原文上；规范文本不再被扫。
2. **宿主加固（防御纵深）**：knowledge_ingest_prepare 返回的 PrepareResult 增加 user_input_text 字段（已拼好的用户原文），前端直接透传，避免前端再拼一次导致口径漂移。
3. **验证**：
   - Rust：单测——整理链路 scan_target 落在用户原文（用真规范文本+含"案件"的规范验证不误拒）。
   - verify_knowledge_ingest_ui_141.py 扩展断言 submitTask 带 user_input_text。
   - 全量回归：python 166 + cargo 524。
   - 真实验证：用户重新点"整理收件箱"，应进入模型调用阶段。

#### 影响范围
KnowledgeIngestPanel.tsx、knowledge_ingest.rs（PrepareResult 加字段）、测试脚本。零协议变更、零新依赖。

#### 顺带说明
同类隐患排查：其他"系统拼提示词"的链路（周报/文案/项目）已传 user_input_text（068 修过），唯 141 漏了。本次一并复查确认无第四处。
### OD-ARCH-143：Task 5 端到端验收与整体收口（S1/S2 已复核通过；S3 用户实测、S4/S5 收口待用户批准执行）

> **触发：** 用户指示 Task 4 完成后出 Task 5 规划；Task 1–4 全部改动积压在工作区未提交，Task 5 完成后一次性提交推送。

---

## 一、现状盘点（实读核实）

**已实现且复核通过的功能面：**
- DataMasker 安全引擎（Task 1 纯文本 32 项 + Task 2 Markdown 结构感知 38 项 + Task 4 主链强制接入）——红线→打码→双向脱敏→回程还原全链贯通。
- 知识库工作流（141）：inbox 收件箱 → AI 整理建卡 → 索引维护 → 对话两跳检索带引用。
- 工作记录四象限（093/097）、周报（memory-report）、随手记合并（132）、项目甘特（098/102）。
- 测试资产：3 个 Python 单测文件（78 项）、1 个前端 test.mjs（8 项）、12+ 个桌面验证脚本、cargo lib 524 项。

**待 Task 5 解决的三类问题：**
1. **积压 30 项未提交**——Task 1–4 全部成果无备份。
2. **5 个既有坏测试**（host_integration 3 项 /bin/true、web_api 2 项 socket/参数序）——影响"全绿"可信度。
3. **零真实环境端到端**——所有验证都在隔离/合成数据上，真实模型 + 真实 Vault 的整链路从未跑过。

## 二、任务定义

**目标：一次端到端验收 + 一次收口提交，把积压工作变成可发布状态。**

### S1 坏测试修复（独立小任务，先做——验收基线必须全绿）
- host_integration 3 项：/bin/true → tempfile 写可执行假 CLI（跨平台）。
- test_web_api_unit 2 项：recv_budget socket 超时（超时预算参数化）；for_dsh 参数序断言（改断言 resolve_node 结果而非字面 "node"）。
- 验收：cargo test --offline 全量（lib + host_integration）全绿；web_api 88/88。不放宽任何生产断言，只修测试自身缺陷。

### S2 端到端验收脚本（合成数据全链路）
verify_e2e_143.py 一条龙（隔离 /tmp 数据根 + 假模型端点）：
1. 启动 → 初始化知识库骨架（S1 of 141）；
2. inbox 投合成材料 → 整理 → 卡落盘 + 索引更新 + 原文归档；
3. 对话提交（含词典人名）→ 引擎日志断言出站占位符 → 假模型回文含占位符 → 还原正确；
4. 随手记 → 周报草稿 → 四象限归类 → 保存成果；
5. 断言全程敏感原文未出现在任何出站日志/事件。
验收：脚本全绿，证据落 evidence/od-arch-143/。

### S3 真实环境验收清单（用户执行，列出清单不代跑）
- 真实模型对话：词典人名出站占位符、回答自动还原；
- 真实 Vault：丢一份材料→整理→对话检索命中引用；
- 随手记/周报在真实工作流里各走一遍；
- 应用重启后旧会话占位符如实提示。
用户确认后填实测结论。

### S4 收口提交（用户批准后执行）
- 敏感扫描：新增文件全过红线词表与凭证模式扫描；词典样例确认全虚构；
- 分批提交（建议 5 个提交：Task 1+2 安全引擎 / Task 3 知识库 / Task 4 强制脱敏 / 文档 / 测试修复），推送 origin/main；
- GitHub Release：版本号 0.2.9（VERSION + tauri.conf 同步），tag 触发 CI 出包（macOS zip + Windows NSIS + Windows zip）；
- STATUS/REVIEW/decisions 更新（D-0xx：LLM 链路强制脱敏决策记录）。

### S5 发布确认
- CI 三产物齐全；安装包体积回归（对比 0.2.8 基线）；用户实机安装冒烟。

## 三、验收标准

1. S1 后：全部测试零失败（首次达到）。
2. S2 后：合成端到端全链路绿。
3. S3 后：用户实测清单全部打勾（或明确列出未过项与原因）。
4. S4/S5 后：GitHub main 与本地一致、0.2.9 三产物可下载、实机安装可用。

## 四、质量门禁

1. 盘点基于实读（测试资产清单、STATUS 各轮记录、git status）。
2. 无重复建设：S2 复用既有假模型端点与隔离根模式，不新建框架。
3. 无未决决策：提交分批与版本号沿用既有惯例；坏测试只修测试自身不改生产断言。
4. 全部步骤可自动验收（S3 用户清单除外——本来就是人验）。
5. 泄漏风险：S4 提交前扫描是最后一道闸；词典样例与测试数据全虚构已复核三轮。
6. 不改生产代码：S1 只修测试；S2/S4 只加脚本与文档。
### OD-ARCH-142：Task 4 LLM 调用链全量强制脱敏（实施已复核通过，见 REVIEW.md 顶部；2026-11-05）

> **触发：** 用户指示出 Task 4 规划。目标：让所有流向云端模型的文本强制经过 DataMasker 脱敏层、所有模型回文自动还原——把 139 规划时承诺的"Task 4 强制点"落地。这是数据安全的最后一块拼图。

---

## 一、现状事实（实读核实：调用链全景与已有脱敏覆盖）

1. **出站唯一入口**：宿主 engine_submit_task（lib.rs:379）是前端到模型文本的**唯一通道**。既有保护链：红线扫描（按用户原文扫）→ mask_privacy（18 位证件/17+X/11 位手机号打码，单向）→ 引擎 submit_task → DSH Web API。
2. **回程两条路**：① 流式事件 assistant-stream/text-delta（引擎翻译后经 bridge_sink → omni://task-event → 前端）；② 非流式终文 get_task_result_text。
3. **已有脱敏覆盖**：对话主链路 mask_privacy 已接（出站）；知识整理链路（141）mask_markdown 已接（引擎侧）；工作记录/知识卡落盘链路 mask_privacy 已接。
4. **缺口（Task 4 要补的三处）**：
   - a. mask_privacy 是**单向打码**：模型回文里没有原文可还原，用户看到的是 "***"，来回对话后上下文质量下降；DataMasker 双向引擎未接主对话链。
   - b. **词典识别未接主链**：干部姓名、支行机构等（sensitive-dict.v1.json）只在知识整理链路生效，对话链路的"张三"会明文出站。
   - c. **回程无还原**：模型用 [PERSON_xxx] 占位符写的回答，前端原样显示占位符——用户不知道指的是谁。
5. **会话多轮结构**：同一引擎会话内多轮 prompt；映射需要按会话存活（SessionScope），否则第二轮模型看到第一轮的占位符却查不到表。

## 二、设计决策（定死）

### D1 双层脱敏，保留打码语义
mask_privacy（证件/手机号打码）**保留不动**——它是对话落盘与本地显示的既有语义；DataMasker 在其后追加：词典实体（PERSON/BRANCH/ORG/CUSTOM）+ EMAIL + 银行卡，出站顺序 = 红线扫描 → mask_privacy → DataMasker.mask（SessionScope 按引擎会话存活）。证件/手机号已被 mask_privacy 打成星号，不会被 DataMasker 二次识别（星号串不匹配规则），无双保险冲突。

### D2 会话映射生命周期
映射表存宿主内存 HashMap<engine_session_id, SessionScope>；生命周期 = 引擎会话生命周期（会话结束/应用退出即丢）。不落盘、不进日志、不进前端。多轮对话内同实体同占位符（SessionScope 既有行为）。应用重启后新会话自然新映射——旧会话恢复（recover_session）时映射表为空，此时占位符无法还原，按"未识别占位符原样保留 + 顶部提示一次"处理（如实告知，不猜）。

### D3 回程还原（引擎侧，双路覆盖）
- 流式：引擎 _turn_worker 在 text-delta 事件转发前，按本任务映射表还原（delta 可能切断占位符——引擎在事件层做缓冲拼接：占位符完整匹配才还原，切断的按 buffer 处理；实现上对 delta 流做一个小状态机）。
- 非流式：get_task_result_text 返回前同表还原。
- 还原 report（ok/unknown）挂到任务终态事件，前端 ok=False 时顶部提示"部分占位符未能还原"，不拦截显示。

### D4 强制性（本 Task 的灵魂）
- DataMasker 接入点在**引擎 submit_task 内部**（web_api_adapter.submit_task 在拿到 text 后、发 session/prompt 前），不在前端、不在宿主——这样绕过前端/宿主直接调引擎的路径（测试、脚本、未来 API）也受保护。
- 引擎新增配置开关 masker_enabled（默认 **true**，无词典时纯规则模式仍生效）；关闭开关的途径只有显式环境变量 OMNIDESK_MASKER=off——UI 不提供关闭入口（用户 10-25 已定"权限默认收紧"哲学）。
- 验收硬指标："绕过脱敏直连模型的路径是否存在"——审计 submit_task 之后的文本流向，确认 prompt_text 进 session/prompt 前已过 masker；附件路径（attachmentPrompt 拼接的正文）同样过。

### D5 词典来源
运行时 <app_root>/config/sensitive-dict.v1.json（139 已定）。词典更新后新会话生效（SessionScope 建立时加载），存续会话不中断——避免半途换表造成映射不一致。

### D6 性能
SessionScope 建立时编译正则缓存（masker 已缓存）；每次 mask O(n)；1MB 提示词实测 <100ms（Task 1 已验证同量级），不设专门优化。

## 三、任务卡（S1–S7）

> 测试：tests/verification/engine-interface/adapter/（引擎侧 unittest）+ desktop/src-tauri（cargo 单测）+ verify_e2e 脚本。

- **S1 宿主会话映射注册表**：lib.rs 新增 SessionMaskRegistry（Mutex<HashMap<String, SessionScope>>）；engine_submit_task 在 mask_privacy 后调 DataMasker 路径……实现抉择：DataMasker 是 Python——宿主 Rust 不能直接调。**修正：接入点放引擎侧**（见 D4），宿主只负责传 engine_session_id（已传）与词典路径环境变量（OMNIDESK_SENSITIVE_DICT，指向 <app_root>/config/sensitive-dict.v1.json，settings 变更时下发）。验收：cargo 单测（环境变量组装）+ 引擎侧读配置单测。
- **S2 引擎出站接入**：web_api_adapter.submit_task 在 text 校验后、_turn_worker 前：加载词典（首次+缓存）、DataMasker().mask(text, session=registry[engine_session_id])、映射表按 session 存引擎内存、占位符文本进 _turn_worker；mask 后长度/统计写任务记录（计数，不含原文）。验收：unittest——mock DSH 前占位符已注入、同会话同实体同占位符、不同会话隔离、词典缺失降级纯规则、OMNIDESK_MASKER=off 关闭。
- **S3 流式回程还原状态机**：_turn_worker 的 text-delta 路径加 buffer 状态机（正则边界内聚合，占位符完整才查表替换）；非流式 get_task_result_text 同表还原；report 挂终态事件。验收：unittest——占位符跨 delta 切断还原、多占位符混排、unknown 保留、report 正确。
- **S4 前端展示与提示**：终态 ok=False 时顶部提示条；引用卡/脱敏统计展示复用既有 privacy_masked 通道扩展（masked counts 增加 masker 计数）。验收：verify_ui 脚本 + tsc。
- **S5 审计：绕过路径清零**：文本流向审计脚本（静态断言 submit_task → prompt 无第二条未脱敏路径；附件/知识上下文/重放历史均在 submit_task 汇合点之后）；输出进 STATUS。验收：审计脚本 0 违例。
- **S6 全链路回归**：Task 1/2 单测 32+38、141 全套、cargo 全量、tsc、build 全绿；手工烟测：对话含词典人名 → 出站占位符 → 回答还原为原文 → 本地线程记录仍为原文。
- **S7 收口**：STATUS/REVIEW 更新，停下待复核。

## 四、验收标准（用户实测）

1. 对话输入"请帮我总结张三的工作表现"（张三在词典）→ DSH 诊断/引擎日志确认出站是 [PERSON_xxx]，模型回答里"张三"自动还原。
2. 重启应用后旧会话继续问 → 占位符原样显示 + 一次性提示（不猜还原）。
3. 词典加一个新支行名 → 新对话生效。
4. 全程本地会话记录不被改写（原文保留在本地）。

## 五、质量门禁

1. 已实读：engine_submit_task 全文、引擎 submit_task/_turn_worker/text-delta 路径、bridge_sink、事件监听、既有 mask 调用点、141 检索注入。
2. 重复风险：与 mask_privacy 分工明确（D1），无双保险冲突（星号串天然不匹配 DataMasker 规则）。
3. 未决决策：无。恢复会话映射丢失的行为（如实提示，不猜）已定。
4. 自动验收：S1–S6 全部可自动；真实模型端到端列用户实测。
5. 泄漏风险：映射表只在引擎内存，随会话消失；词典缺失降级纯规则并在统计 warn；OMNIDESK_MASKER=off 是显式环境变量，UI 无关闭入口。
6. 结构破坏：主对话是纯文本路径，Task 2 的 markdown 路径不涉及。
7. 跨请求泄漏：SessionScope 按会话隔离（Task 1 已验）；恢复会话映射清空属已知边界（如实提示）。
8. 云端强制：本 Task 完成后"绕过脱敏直连"审计通过即成立——这是 139 规划承诺的最终强制点。
### OD-ARCH-141：Task 3 知识库工作流接线（实施已复核通过，见 REVIEW.md 顶部；2026-11-05）

> **触发：** 用户指示继续 Task 3。目标体验：用户只管往收件箱丢材料，AI 自动整理成知识卡；对话时 AI 自动查知识库给出更准、更适合、带引用的回答（NotebookLM 式）。Task 2 结构感知脱敏已完成，整理链路出站可安全脱敏。

---

## 一、现状事实（实读核实）

1. **知识卡规范已齐**：knowledge-search skill 定义卡片格式（frontmatter tags/sources/date + 正文≤300字自包含 + 双链）、索引.md 一行一卡格式、两跳检索硬约束、防重复建卡流程、Obsidian CLI 可用/降级双路径；knowledge-agent skill 定义建卡内容规范（五段式）。**技能层完全就绪，缺的是运行时接线。**
2. **Vault 命令族已齐**：lib.rs 已注册 knowledge_vault_scan / read_note / parse_note / create_note / save_note / search / rebuild_index / note_links 八个命令——建卡、检索、索引的宿主能力全部存在。
3. **用户 Vault 现状**：库/ 下只有 工作记录/，无 知识库/ 目录、无索引.md、无 inbox/——链路从未跑通。
4. **对话注入点已存在**：conversationPrompt.ts 的 skills 注入链（skillsByFlow 按 mode 取技能）；HomeChat 是唯一对话入口（ScenePage 不挂 Chat）。
5. **Task 1/2 脱敏引擎就绪**：整理链路若走云端模型，出站可过 mask_markdown。

## 二、设计决策（定死）

### D1 目录约定（Vault 布局）
- 库/知识库/概念/ 与 方法/：AI 建卡目标目录（skill 已定）。
- 库/知识库/原文/：原始材料归档（卡片 sources 指向这里）。
- 库/inbox/：用户唯一入口——往里丢任何 md/txt，即"待整理"。
- 索引.md：库/知识库/索引.md，AI 维护。
- 用户自有笔记区：库/笔记/（AI 只读，永不写）。首建时由 Task 3 的初始化动作创建目录骨架 + 空索引。

### D2 整理工作流（写路径）
触发：场景页/首页提供「整理收件箱」按钮（手动，不做定时——沿用 092 裁定）。流程：
1. 宿主列 inbox 文件（复用 knowledge_vault_scan 限定 inbox 前缀）。
2. 每批（≤5 文件）组装提示词：文件原文 + knowledge-agent 建卡规范 + knowledge-search 卡片格式 + 索引现状 → intent=knowledge_ingest 走 submit_task。
3. 模型返回结构化结果（JSON：cards[] + index_lines[] + archive_moves[]），宿主校验（目标路径白名单、标题防重复——与索引比对、卡正文 ≤300 字、sources 路径必须真实存在）后逐卡落盘（knowledge_vault_create_note）+ 更新索引.md（追加行）+ 原文移入 原文/。
4. 校验失败的卡不入库、列入报告；全部结果在界面展示（建了几张、跳过什么、为何跳过）。
5. 出站脱敏：提示词组装时对 inbox 原文先跑 mask_markdown（Task 2 引擎，Python 侧在引擎内做——引擎新增 ingest 前置调用，映射表随任务在内存持有，还原仅用于本地归档文件名）。

### D3 对话自动检索（读路径）
触发：用户在对话中提问且问题含"规定/制度/怎么办/流程/依据"等知识意图关键词，或显式 @知识库。流程：
1. 提交前宿主先跑 knowledge_vault_search（现成命令，预算检索）取 top3 命中。
2. 命中则把 索引.md 相关行 + 命中卡正文（≤2 张）注入提示词上下文区（带"以下为知识库检索结果，引用须与原文一致"约束）。
3. 关键词意图判定由确定性词表 + 首次模型自判（"是否需要查知识库"）双层：词表命中直接查；未命中但用户 @知识库 时必查；其余不查（省 token）。
4. 回答后前端展示"引用了哪几张卡"（可点击打开——复用 knowledge_vault_open_attachment）。

### D4 边界
- 不做向量/RAG（140 已定触发条件）。
- 不做定时整理（092 裁定沿用）。
- 模型整理结果必须经宿主校验落盘，AI 无直接写权（与 memory-agent 同哲学：AI 判语义，工作台落盘）。
- 基础标准（omnidesk-base）下不提供（无场景规则节点）。

## 三、任务卡（S1–S7）

> 测试位置：tests/verification/desktop/（宿主/集成层用既有 verification 模式）+ engine 单测。
> 实施交新会话 DeepSeek，模式同 Task 1/2。

- **S1 目录骨架初始化**：宿主命令 knowledge_ingest_init——建 概念/方法/原文/inbox/ 目录 + 空索引.md（幂等，已存在不动）；验收：隔离 /tmp 数据根跑命令，断言目录与索引生成、二次执行零改动。
- **S2 整理提示词组装**（引擎侧）：assemble_ingest_prompt(files, index, card_spec) 纯函数 + 单测；断言：含建卡规范全文、索引现状、原文 mask 后注入、JSON 输出约束；mask 映射随任务对象持有。
- **S3 整理结果校验器**：validate_ingest_result(json, index, vault_scan)——标题防重（与索引比）、路径白名单（只许 概念/方法/）、卡长校验、sources 存在性；断言：非法路径拒、重复标题拒、超长拒、sources 缺失拒、合法通过；单测 8+ 用例。
- **S4 落盘与索引更新**：host 侧 apply_ingest_result——逐卡 create_note + 索引追加 + 原文移动；失败回滚当卡（不影响已通过卡）；断言：隔离根跑通全流程、失败卡跳过且报告正确、索引行与卡一一对应。
- **S5 对话检索注入**：HomeChat 提交前钩子——词表意图判定 + knowledge_vault_search top3 + 上下文注入（截断上限 3000 字符）+ 引用卡片记录；断言：词表命中触发、@知识库必触发、无命中不注入、注入不超上限（前端 tsc + 组件测试脚本）。
- **S6 前端入口**：首页/工作记录场景「整理收件箱」按钮 + 进度与结果报告 UI + 引用卡点击打开；验收：npm build + verify_ui 脚本（固定选择器）。
- **S7 收口**：全链路冒烟（隔离根：inbox 放合成材料 → 整理 → 卡落盘 → 对话检索命中 → 引用展示）；cargo test 全量 + tsc + build；STATUS/REVIEW 更新。

## 四、验收标准（用户实测）

1. 往 inbox 丢一份合成制度材料 → 点整理 → 知识库出现合规卡 + 索引更新 + 原文归档。
2. 对话问"专项检查的流程是什么" → 回答引用刚建的卡，可点击打开。
3. 问无关问题 → 不触发检索（token 不浪费）。
4. 全程无真实材料进入 Git 或测试夹具。

## 五、质量门禁

1. 已实读：两个 knowledge skill 全文、vault 八命令、对话注入链、用户 Vault 实况。
2. 重复风险：无新检索实现（复用 vault_search）；卡格式复用 skill 既有定义，不另造。
3. 未决决策：无。RAG 触发条件沿用 140；定时整理沿用 092 裁定不做。
4. 自动验收：S1-S4、S6 可全自动；S5 词表判定可自动、模型自判属真实模型实测（列用户实测项）；S7 冒烟脚本自动。
5. 泄漏风险：出站 mask_markdown 在 S2 强制；映射内存持有不落盘；收件箱材料若含敏感值，建卡后的卡片内容已被脱敏（可回原文核对——原文在本地 原文/ 目录）。
6. 结构破坏：卡片由 create_note 新建，不碰既有文件；索引只追加（防重在校验器）。
7. 跨请求泄漏：Task 1/2 机制继承。
8. 云端强制：整理链路本任务内即过脱敏（S2）；全链路强制仍属 Task 4。
### OD-ARCH-140：Task 2 结构感知脱敏 + 知识库问答架构（Task 2 实施已复核通过，见 REVIEW.md；Task 3 规划进行中）

> **触发：** 用户批准 Task 1 后指示：① 开始规划 Task 2（DataMasker 结构适配）；② 一并回答 Obsidian 与 llm-wiki 如何融合、是否用 RAG、是否调大模型。用户定调：知识库是给 AI 准备的，我不维护不检索，我只管不断输入内容，最后 AI 借知识库给我更准的回答——参考 NotebookLM。

---

## 一、现状事实（实读核实，决定三条路线）

1. Vault 基础设施已建成：desktop/src-tauri/ 下有 vault_search.rs（带读取预算的全文检索）、vault_index.rs（标题/块锚点/双链索引/fingerprint）、vault_link.rs（双链解析）、vault_note.rs（Frontmatter/标签解析）、vault_write.rs（白名单写入）、knowledge_vault.rs（库根校验）——OD-KB-001 P2 已落地笔记浏览、属性、搜索、安全编辑。Rust 侧结构解析器已存在，Task 2 不必从零写。
2. llm-wiki 方案已部分落地：knowledge-search skill 定义了索引卡+两跳检索（读 索引.md → 按标签定位 → 只加载 1-2 张卡），knowledge-agent skill 负责整理成卡。但用户 Vault 里还没有 知识库/ 目录和索引.md——skill 有了，卡没建，链路没跑通。
3. 模型通道已通：引擎 submit_task → DSH Web API 全链路（080 协议验证过，真实模型用户已配置）。
4. DataMasker Task 1 已收口：纯文本双向脱敏可用；masker.py 无结构感知。
5. 无向量库、无 embedding 基础设施。

## 二、用户问题直接回答（决策记录，写入 decisions）

### Q1 Obsidian 与 llm-wiki 如何融合？

文件系统即接口，零转换层。Obsidian 的本质是一个 Markdown 文件夹 + 双链语法；llm-wiki（索引卡+两跳）也是 Markdown 文件夹。两者共用同一目录（~/OmniDesk/库/知识库/），Obsidian 是人看的编辑器，llm-wiki 索引卡是 AI 读的目录页。融合 = 四条约定：
- 知识卡由 AI 整理生成、用户在 Obsidian 里只读不改（改了 AI 下次整理会重建并提示冲突）；用户自有笔记放另一个目录，AI 只读不写。
- 索引.md 由 AI 维护：每次整理自动更新一行一卡；Task 2 顺手把 vault_index.rs 的链接/标签索引导出为索引.md 的校验源。
- 双向链接 [[卡片名]]：AI 建卡时按 vault_link.rs 已解析的关系自动生成，Obsidian 里点得动，AI 读索引时按标题跳。
- Obsidian 本体不安装、不依赖（沿用 10-26 基线 Obsidian 可用但非依赖）。

### Q2 要不要 RAG（向量检索）？

现在不建，触发条件明确（写入 decisions，不提前造）。理由：
- 个人纪检知识库预计百级卡片、每卡几百字——索引.md 一次读入 1-2k token，两跳检索在千卡以内绰绰有余，向量库在此量级是过度设计。
- 向量库引入 embedding 依赖、索引构建、阈值调参、检索质量评测四套成本；知识卡本身已是 AI 提炼的语义单元——卡就是最准的 chunk，再切分向量化是二次降质。
- 触发条件（满足任一才评估）：卡片 >500 张且索引读取超预算；提问频繁出现索引没有但库里其实有（召回失败）；引入大批量非卡式长文档（如 234 份制度汇编）需段落级定位。触发后优先评估 BM25/全文排序升级（vault_search 已有预算检索骨架），而非向量库。

### Q3 要不要调大模型？

要，两处，都走现有通道，不新建：
- 整理入库（写路径）：knowledge-agent skill 读原始材料 → 生成知识卡 + 更新索引。skill 已有，缺触发时机与批量工作流——纳入 Task 3 规划（用户只管输入内容 = 放进收件箱目录，整理动作手动触发或打开应用时提示）。
- 检索问答（读路径）：knowledge-search 两跳检索 → 卡片注入上下文 → 模型作答并引用。skill 与通道已有，缺对话自动带知识库上下文的接线——纳入 Task 3。

### Q4 与 NotebookLM 的差异（如实告知）

NotebookLM = 上传源文档 + 全库语义索引 + 带引用问答。OmniDesk 在带引用问答上目标一致（卡片带 sources frontmatter，可回原文核对——检索验收规格 A/B 两命题已定义）；差异在索引方式（索引卡 vs 向量），这是有意取舍：省 token、可人审、可重建、不引新依赖。千卡以内体验等价，超了按 Q2 触发升级。

## 三、Task 2 范围定义（DataMasker 结构感知，只做这一件事）

目标：mask/unmask 对 Obsidian 兼容 Markdown 安全——只改占位符该在的位置，结构逐字节保真。

区域策略表（定死）：
- YAML Frontmatter：脱敏（sources 里可能有人名/机构）
- 普通正文：脱敏（主战场）
- 代码块与行内代码：跳过（脱敏破坏代码语义）
- WikiLink 目标 [[目标]]：跳过（改目标=断链）
- WikiLink 显示文本 [[a|显示]]：脱敏（显示文本是正文）
- 嵌入 ![[目标]]：跳过（同链接目标）
- 标签 #tag：跳过（标签是检索键）
- URL：跳过（改了失效）

实现：区域扫描器（frontmatter 边界定义与 vault_note.rs 对齐；代码块/行内码/WikiLink 正则切区）→ 只对脱敏区跑 Task 1 引擎 → 拼回。禁止解析再序列化：输出 = 原文逐字节保留 + 仅替换区间改写；验收含除占位符外逐字节一致硬断言。

新接口：mask_markdown(text, session=None) 返回 MaskResult；unmask_markdown 对称。保留 mask() 纯文本路径不动（Task 4 LLM 链路用纯文本，不受影响）。

## 四、任务卡（S1-S6，unittest -k 验收，模式同 Task 1）

- S1 区域扫描器：Markdown 切区（frontmatter/代码块/行内码/WikiLink/其余正文）输出区间列表；验收 -k Region：三类 WikiLink 区分、嵌套代码块、frontmatter 未闭合告警、区间并集不重叠。
- S2 mask_markdown 主路径：只对脱敏区调 Task 1 引擎；验收 -k MaskMd：除占位符外逐字节一致；代码块内证件号不替换；frontmatter 人名替换；显示文本替换但链接目标保留。
- S3 unmask_markdown：对称还原 + report；验收 -k UnmaskMd：往返逐字节一致；跨映射拒绝同 Task 1。
- S4 畸形结构：未闭合代码块、巨量链接、1MB 文档性能上限；验收 -k Edge。
- S5 vault_write 侧对齐确认（纯文档核对，不改 Rust）：确认写入白名单与 .md 脱敏无冲突，产出检查记录进 STATUS。
- S6 收口：全量单测 + Task 1 32 项回归 + 手工烟测。

运行目录与命令形态同 Task 1：cd tests/verification/engine-interface/adapter && python3 -m unittest -v test_security_masker_markdown_unit -k <Key>。

## 五、Task 3 预告（下一轮规划，本轮不展开）

知识库只输入不管体验的落地：收件箱目录约定、knowledge-agent 批量整理工作流（建卡+索引更新+冲突提示）、对话自动带两跳检索接线、NotebookLM 式引用格式（卡片链接 + sources 回溯）。整理链路出站脱敏在 Task 4 一并接入。

## 六、质量门禁

1. 已检查真实代码：vault 六模块、两个 knowledge skill、用户 Vault 实况、DataMasker 现状均实读。
2. 重复风险：区域扫描与 vault_note.rs 的 frontmatter 语法须对齐（同一套 YAML 边界），列为 S1 验收项；不重建检索（vault_search 已有），本轮零 Rust 改动。
3. 未决决策：无——八类区域策略全部定死；RAG 设触发条件不做预设实现。
4. 自动验收：S1-S4、S6 全部 unittest -k；S5 为文档核对（产出检查记录）。
5. 泄漏风险：代码块跳过是设计决策——代码块内敏感值不脱敏，验收样例明示此边界。
6. 结构破坏风险：逐字节保真硬断言 + 链接目标永不改写。
7. 跨请求泄漏：Task 1 机制原样继承。
8. 云端强制：仍属 Task 4，本轮不变。

## 七、交付与边界

本轮新增：docs/DATAMASKER_TASK2_PLAN.md（三、四节展开）、PLANS.md 登记。实施交新会话 DeepSeek 按卡执行（模式同 Task 1），S1-S6 全过 + ARCHITECT 复核后收口。
### OD-ARCH-139：DataMasker 双向脱敏引擎（规划轮已审通过；Task 1 实施已复核通过，见 REVIEW.md 顶部与 STATUS.md）

> **触发：** 用户暂停四象限（OD-ARCH-138 挂起），下达 Task 1 规划指令：建立独立敏感数据脱敏模块 DataMasker，为知识库 / LLM-Wiki / 云端 LLM 调用提供统一数据安全基础。本轮只产出规划文档，不写功能代码。

---

## 一、Step 1 架构检查结论（已实读，全部为文件事实）

1. **Python 侧只有一个包**：engine/omnidesk_engine/（10 个模块），纯标准库（dataclasses/typing/json/pathlib/urllib/socket 等），零第三方依赖；Python 3.9 兼容是硬约束（OD-ARCH-132 曾专门修 3.9 兼容）。用户建议路径 "omniharness/security/masker.py" 中的 omniharness 包**不存在**，须落到 engine/omnidesk_engine/security/。
2. **现有脱敏能力是单向且不可还原**：Rust 侧 profile_rules.rs::mask_privacy 只做打码（18 位证件/17+X/11 位手机号 → ***），无映射、无还原；task_shell.rs::red_line_scan 是关键词命中拒绝。DataMasker 的"双向可还原"是全新能力，**与现有实现不重复、不冲突**，但口径必须对齐（同一证件/手机号识别规则），否则两条链路会各判各的。
3. **配置落点惯例**：仓库 config/ 只放样例；用户可改配置在 ~/OmniDesk/app/config/（offline-flows.v1.json、scheduler.v1.json 等 <name>.v1.json 命名模式）。sensitive_dict 应循此例：仓库放 config/sensitive-dict.sample.v1.json，运行时读 <app_root>/config/sensitive-dict.v1.json。
4. **测试惯例**：unittest（非 pytest），测试文件放 tests/verification/engine-interface/adapter/，命令 python3 -m unittest -v test_xxx；本轮新建 tests/verification/engine-interface/adapter/test_security_masker_unit.py。
5. **文档目录**：docs/ 为现行项目文档根（docs/development/ 是未跟踪新目录，来源不明，不用）；规划文档放 docs/。
6. **LLM 调用链（只看不动）**：宿主 lib.rs → 引擎 web_api_adapter.py → DSH Web API（session/prompt ws）。Task 4 集成点在引擎 submit_task 出站前（mask）与会话事件回程（unmask），本轮不碰。

## 二、Step 2 技术设计（全部定死，不留实施方选择）

### 2.1 模块与文件
- engine/omnidesk_engine/security/__init__.py（导出 DataMasker）
- engine/omnidesk_engine/security/masker.py（核心，仅标准库 re/secrets/hashlib/dataclasses/typing/json/pathlib）
- config/sensitive-dict.sample.v1.json（样例词典，全部虚构）
- 运行时词典：<app_root>/config/sensitive-dict.v1.json（masker 不写它；加载失败时只用规则路径并如实上报）

### 2.2 词典结构（sensitive-dict.v1.json）
{"schema":"omnidesk.sensitive-dict.v1","entries":[{"type":"PERSON|BRANCH|ORG|CUSTOM","value":"张三丰","aliases":["张三丰总"]},...]}
- 同 value 不同 type 视为不同实体；aliases 并入匹配集，占位符一致。
- 类型集合 Task 1 固定四类，代码用 Literal 约束，扩展留 type 注册表钩子（新增类型 = 注册 + 正则/词典项，不改核心）。

### 2.3 规则识别（正则 + 校验，不裸判长度）
- IDCARD：18 位，含 GB11643 校验码验证（加权因子算法， rejecting 校验失败的 18 位串——避免把 18 位流水号当证件）；兼容 17+X。
- BANKCARD（Luhn 校验，13–19 位数字；不区分卡组织——Task 1 不做 BIN 表）。
- PHONE：(?<!d)1[3-9]d{9}(?!d)，前后必须是数字边界。
- EMAIL：标准 RFC 简化式（本地@域，域含点），长度上限 254。
- 优先级：IDCARD > BANKCARD > PHONE > EMAIL > 词典（数字规则先于词典，防止词典词内含数字串被抢先；同为规则时按上述顺序）。

### 2.4 占位符与映射
- 占位符：[TYPE_ + secrets.token_hex(4) 大写] ，如 [PERSON_A81F29C4]；8 hex 随机、32bit 空间、同请求内重复实体查表复用，碰撞时重生成（while 循环 + 上限 1000 次，概率上不可能触达）。
- Request Mapping：MaskMapping dataclass（frozen mapping of placeholder→{type, original}），生命周期=单次 mask 调用返回值，由调用方持有；DataMasker 不留存。
- Session Mapping：SessionScope 对象由调用方创建并传入 mask(session=scope)，同 scope 内相同实体复用占位符；Task 1 只做内存态接口，不持久化。
- Local Dictionary：即 2.2 词典文件（长期存在的"识别依据"），不是映射表。
- Identity Vault：不做。

### 2.5 重叠/同名/冲突
- 匹配全部定位后按 (start, -length) 排序，先占长、位置重叠的短匹配丢弃；词典与规则在同一轮扫描中统一排序。
- 同名同 type = 同一实体一个占位符；同名不同 type = 各自占位符，还原互不干扰。
- unmask 只处理精确完整占位符（正则 [(IDCARD|BANKCARD|PHONE|EMAIL|PERSON|BRANCH|ORG|CUSTOM)_[0-9A-F]{8}] 整体匹配 + mapping 查表），未知/被改占位符原样保留并计入 issues。

### 2.6 映射安全边界
- MaskMapping 不实现 to_dict 导出原文；repr 覆盖为 "<MaskMapping n=8>"；文档明示"不得写入日志/不得随请求外发/不得跨请求复用"。
- unmask(mapping=) 必须显式传对应 mapping；DataMasker 实例不保存任何历史映射（无实例级 dict）——映射隔离由构造保证，不靠纪律。
- validate_mapping(masked, mapping) 返回 {ok, unknown:[...], missing_in_mapping:n, stats}，不改文本。

### 2.7 Markdown 结构适配的边界（Task 2 做，本轮只定策略基调）
- Task 1 按纯文本处理，不识别结构。
- Task 2 起实施区域策略：代码块/行内代码默认跳过（可配），Frontmatter 默认处理，WikiLink 目标与别名分别处理（目标脱敏会破坏链接，默认只处理显示文本——具体在 Task 2 设计文档定）。
- 永远不做"解析再序列化"。

### 2.8 还原完整性
- unmask 返回 (text, report)；report 含 restored/unknown_placeholders/modified_candidates（形如占位符但查表失败的数量）。ok 条件：unknown=0。调用方（Task 4）在 ok=False 时不得把结果当已还原内容使用——这个纪律写进接口 docstring 与 Task 4 验收。

## 三、Step 3 实施任务卡（每步一个动作、可自动验收）

**S1 词典模块**：security/__init__.py + masker.py 的 DictEntry/load_dictionary；输入 config/sensitive-dict.sample.v1.json；验收 python3 -m unittest test_security_masker_unit -k Dict（样例文件可加载、schema 错拒绝、重复 value+type 合并 aliases、非法 type 拒绝）。前置：无。
**S2 规则识别器**：四类正则+校验函数；验收 -k Rule：GB11643 合法/非法样例各若干（虚构）、Luhn 合法/非法、手机号边界（前贴数字不命中）、邮箱长短；全部虚构数据。前置 S1。
**S3 占位符与 MaskMapping**：生成、同作用域复用、碰撞重试、repr 脱敏；验收 -k Placeholder：同文本两处同名同占位符；不同请求不同占位符；repr 不含原文。前置 S1。
**S4 mask 主体与重叠处理**：统一扫描排序替换；验收 -k Mask：重叠（"张三丰" vs "张三"）长者胜；规则与词典叠加；空文本/None/超长安全返回。前置 S2 S3。
**S5 unmask + validate_mapping**：精确还原、未知占位符保留并报告、跨 mapping 还原无效、修改占位符不还原；验收 -k Unmask。前置 S4。
**S6 SessionScope**：同 session 复用占位符、不同 session 隔离；验收 -k Session。前置 S4。
**S7 收口**：全量单测 + 引擎现有两套 unittest 回归（确认零 import 波及）；npm/cargo 不涉及；更新 STATUS/REVIEW。

每步验收命令统一形态：cd tests/verification/engine-interface/adapter && python3 -m unittest -v test_security_masker_unit -k <Key>，退出码 0 且断言数 ≥ 任务卡列出的最小断言数（写入脚本 docstring）。

## 四、Step 4 后续路线图（本轮只登记不实施）
- Task 2（OD-ARCH-141 候选）：Markdown/YAML/WikiLink 结构感知脱敏（区域策略表 + 逐区域扫描，禁解析再序列化）。
- Task 3（142）：安全加固——异常审计日志（只记计数与类型，不记原文/映射）、词典文件完整性校验、并发使用约束。
- Task 4（143）：接入 LLM 调用链——引擎 submit_task 出站 mask、会话事件回程 unmask、ok=False 时拦截并提示（含映射隔离与还原失败 UX）。
- Task 5（144）：知识库批量处理与端到端验收（含 Obsidian 库实测）。

## 五、质量门禁回答

1. 已检查真实代码/测试体系：是（engine 包、Rust mask_privacy、unittest 惯例、config 命名均实读）。
2. 与现有功能重复：无双向脱敏重复实现；但识别口径必须与 mask_privacy/red_line 对齐，已列入设计（2.3）。
3. 未决技术决策：无——占位符、映射生命周期、重叠优先级、还原校验、文件落点全部定死；唯一外部依赖项是 Task 2 的 WikiLink 目标默认策略（明确划出本轮）。
4. 无法自动验收的步骤：无——7 步全部给出 unittest -k 命令与断言要点；"真实效果符合岗位直觉"类主观验收不存在于本轮。
5. 敏感信息泄漏风险：样例词典全虚构；映射 repr 脱敏 + 不提供原文导出 + 无实例留存；测试命令不打印敏感值。残留风险：调用方自行 persist mapping 无法从代码层阻止，只能靠 Task 4 的接口设计收口——已写入路线图。
6. 破坏 Markdown 结构风险：本轮按纯文本处理，结构适配整体划给 Task 2 并已定"禁解析再序列化"基调。
7. 跨请求映射泄漏：构造性杜绝（DataMasker 不持有任何映射；session 映射显式传入且不持久化）。
8. 未来云端请求必经脱敏层：不能在本轮保证——Task 1 是独立模块；真正的强制点在 Task 4（引擎出站单点接入），届时以"绕过脱敏的直连路径是否存在"为验收项。此点如实告知：Task 1 完成不等于云端调用已受保护。

## 六、交付物清单（本轮仅新增 3 份文档）
- docs/DATAMASKER_ARCHITECTURE.md（第一、二节展开成文）
- docs/DATAMASKER_TASK_PLAN.md（第三节任务卡展开成文，含每步断言清单）
- docs/DATAMASKER_ROADMAP.md（第四节展开成文）
不动任何源码/测试/配置/AGENTS.md。用户批准后按任务卡交实施。
### OD-ARCH-138：工作记录四象限 AI 优先级归类（待批准）

> **触发：** 用户裁定现行四象限"既不正确也没有意义"——只按「日期已过 + 去向」两个死规则机械分格。真实需求：AI 读当周记录里的想做、在做、未完成事项，结合岗位职责与工作重点判断优先级，推动及时完成。

#### 现状与可行路径（已实读代码核实）

- 现行归类在 weeklyQuadrants.ts（classifyWorklog），纯规则推导，卡片 WeeklyQuadrants.tsx 无模型输入。
- 模型通道复用引擎既有 submit_task → session/prompt → get_task_result_text 全链路（engine/omnidesk_engine/web_api_adapter.py:1208/1422），不新增 RPC、不改协议。
- 岗位上下文落点：rules.json 的 scenes.weekly（现有场景规则编辑器已支持编辑该节点），新增可选字段 priority_context。

#### 方案

**一句话：场景页四象限新增「AI 归类」动作——宿主组装「当周记录原文 + 岗位工作重点 + 输出格式约束」发起一次引擎任务，模型返回结构化四象限判定，卡片按 AI 结果渲染并标注来源，无结果时回落规则视图。**

1. **规则字段（profile_rules.rs / rules.json schema v2）**：scenes.weekly 新增可选 priority_context（字符串，≤2000 字），语义是"用户岗位职责与当前工作重点"，设置页场景规则编辑器加一个多行输入。缺省为空——为空时提示词里不含岗位段落，仅按记录内容判断。
2. **宿主命令 priority_review（lib.rs 新命令）**：
   - 输入：week（可选，默认当周）。
   - 组装提示词：当周工作记录原文（worklog::read_week）+ priority_context + 严格输出约束（只输出 JSON 数组：[{ "ref": "日期#序号", "status": "done|doing|todo|idea", "quadrant": "do_now|schedule|delegate|hold", "reason": "一句话" }]；不新增条目、不改写原文、不得调用工具；ref 必须来自材料中真实存在的条目，发现不了就省略）。
   - 以独立 intent（priority_review）走 submit_task 发起任务（新会话、max 轮次 1 语义由提示词约束），轮询 task_status 至 completed/failed（超时 120 秒），取 get_task_result_text，从回复中提取 JSON（容错：剥 markdown 代码围栏），校验 ref 全部命中当周条目（ref 不存在的条目丢弃并计数）。
   - 结果持久化：~/OmniDesk/app/engine-state/priority/<week>.json（含 model 标识、时间戳、条目数、丢弃数）。读命令 priority_review_get 返回缓存。
   - 红线与隐私：记录原文已在落盘时掩码；本次不新增掩码逻辑，但回执不落任何模型原始全文，只存解析后的 JSON。
3. **前端（ScenePage / WeeklyQuadrants）**：
   - 卡片头部加「AI 归类」按钮：有缓存先显示缓存 + 「重新分析」；无缓存显示规则视图 + 「AI 归类」入口。
   - AI 结果渲染：四格语义改为 立即做（重要且紧急）/ 排期做（重要不紧急）/ 可委托（紧急不重要）/ 暂缓（不急不重要）；条目显示状态徽标（想做/在做/已完成/想法）+ 一句理由；标注「模型判定 HH:MM」。
   - 条目点击仍联动时间线筛选（ref 含日期与序号）。
   - 任务进行中显示进度态；失败显示原因与重试；模型不可用（引擎未起/无路由）时按钮置灰并说明。
   - 规则视图保留为空态与降级视图，界面上两种来源都如实标注。
4. **缓存失效**：当周记录文件变化（mtime/内容 hash）后，缓存标记「已过期」并在卡片提示"记录有更新，可重新分析"，不自动重跑（省 token）。
5. **验证**：
   - Rust：priority_review 的提示词组装（含/不含 priority_context、week 缺省）、JSON 提取容错（围栏/前后缀/非法 ref 丢弃）、缓存读写、单测。
   - verify_worklog_138.py：假引擎返回固定 JSON 的端到端（提交→轮询→解析→缓存→读取）、ref 不命中丢弃、超时失败路径、缓存过期标记。
   - 前端 tsc + build；cargo 全量回归。
   - 真实模型效果属用户实测项：按你的当周真实记录跑一次，检查排序是否符合你的岗位直觉。

#### 影响范围
profile_rules.rs（新字段 + 校验）、lib.rs（两个新命令）、新模块 priority_review.rs（组装/解析/缓存）、SettingsPanel（工作重点输入）、ScenePage + WeeklyQuadrants（按钮与渲染）、weekQuadrants 语义文案、新验证脚本。不改引擎、不改协议、不改 worklog 写入链路。

#### 风险
- 模型可能不守"只输出 JSON"约束：提取器做容错剥离，解析失败如实报错并保留规则视图，不硬造结果。
- 每周多次点「重新分析」会多花 token：缓存 + 过期提示 + 手动触发，不做自动重跑。
- DSH 任务通道可能带工具调用拖慢速度：提示词显式禁止工具；若实测仍慢，下轮改走窄化 intent。

#### 明确不做
- 不改四象限的写入安全链路（本功能零写记录文件）。
- 不做自动定时分析（用户 2026-10-25 已裁定移除自动复盘入口；本功能保持手动触发）。
- 不在基础标准（omnidesk-base）下提供此功能（无场景规则节点，按钮不渲染）。

#### 验收标准
用户实机：加载 discipline → 工作记录场景填一次「工作重点」→ 点「AI 归类」→ 四象限按真实优先级排布、每条带状态与理由、点击联动时间线；重新修改记录后卡片提示可重新分析；关掉模型路由后回落规则视图且标注清楚。
### OD-ARCH-137：首次启动先播种 DSH Profile（计划者与实施者：本助手；用户反馈 0.2.8 新机器打开「设置模型」报 `profile "omnidesk-base" does not exist`；2026-11-04 已实施）

#### 触发

0.2.8 在全新的 `~/OmniDesk` 上打开「设置模型」时提示：
「DSH 配置转储失败（退出码 1）：Profile「omnidesk-base」在 DSH 里不存在」。此时 `~/OmniDesk` 只有 `app/`，没有 `runtime/`。

#### 已核实根因

1. **启动入口没有播种。** `build_state()` 只调用 `assemble_profile_assets_on_startup()`；标准 Profile（`omnidesk-base`）下装配是 no-op，`dsh-home` 仍然不存在。
2. **模型设置不经过引擎启动。** `task_list_model_routes` / `model_settings_save` / `task_set_model_route` 直接构造 `EngineConfig` 并调用 `dsh --dump-config`，不会触发 `engine.start()` 或 `dsh_settings::ensure()` 里的 `seed_profile_if_missing()`。
3. **全新数据根缺少 Profile。** 包内 `runtime/dsh-home-seed` 有 `omnidesk-base` 与 `web`，但没有被铺到用户目录；DSH 自然报 profile 不存在。

#### 方案（已实施）

1. **启动时播种**：`build_state()` 在装配资产之前调用 `seed_dsh_home_on_startup()`，把包内 seed 铺到隔离 `DSH_HOME`，并把 `seed=created/ready/error` 写进 `logs/profile-assets.log`。
2. **模型路径兜底**：`model_settings_save`、`task_list_model_routes`、`task_set_model_route` 在调用 DSH 前先执行 `seed_profile_if_missing()`，不再依赖「引擎先启动过」。
3. **部分数据根补齐**：`seed_home_from_package()` 从「有 `profiles/` 就整体跳过」改为「逐个补缺失的 Profile 与 Skill」，已有用户文件不覆盖。
4. **验证**：新增 `build_state_seeds_bundled_home_before_model_settings` 回归测试；`verify_startup_seed_137.py` 静态检查三条模型路径，并用本机 `.app` 自带 DSH 实测「空 home 退出 1、seed home 退出 0」。

#### 影响范围

`desktop/src-tauri/src/lib.rs`（启动播种 + 三条模型路径）、`desktop/src-tauri/src/engine_supervisor.rs`（seed 合并语义）、新增验证脚本与证据、版本号 0.2.9。

#### 风险与边界

- 首次启动会复制约 8MB 的 seed（Profile 依赖 + Skill）；之后启动只检查存在性。
- 已有数据根只补缺失项；用户改过的 `package.json`、`cordis.patch.yml`、`SKILL.md` 一律不覆盖。
- 包内 seed 缺失时仍会报模型配置错误，但 `profile-assets.log` 会留下 `seed=error`，不再静默。

#### 验收标准

全新 `~/OmniDesk` 启动 0.2.9 后，`runtime/dsh-home/profiles/omnidesk-base/package.json` 存在；打开「设置模型」能正常列出路由；DSH `--dump-config` 在铺开 seed 后退出 0；全量单测通过。

### OD-ARCH-136：macOS 应用自带运行时 + 安装根识别收紧（计划者与实施者：本助手；用户反馈 0.2.7 mac 版报「/Users/runner/... 安装不完整」；2026-11-04 已实施，用户本机验证通过）

> **触发：** 用户打开 0.2.7 的 macOS 包，环境检查页显示运行时目录为 `/Users/runner/work/omniharness/omniharness`，提示安装不完整。

#### 已核实根因

1. **编译期路径泄漏到发布包。** `repo_root()` 的第三级兜底是 `CARGO_MANIFEST_DIR` 上溯两级，那是构建机的绝对路径。发布包找不到运行时的时候，它把 `/Users/runner/...` 当成安装根返回，界面于是显示一个用户机器上根本不存在的路径。
2. **macOS 包依赖同目录副本。** `.app` 本身不带运行时，靠免安装目录把 `engine/`、`runtime/` 放在 `.app` 旁边。用户把 `OmniDesk.app` 拖进 `/Applications`（或系统做了 App Translocation），兄弟目录就找不到了，应用必然起不来。
3. **检查项文案错位。** `assets` 项复用了 node/python 的「版本过低」判断文案，运行时缺失时显示成「版本过低」。

#### 方案（已实施）

1. **`.app` 自带运行时**：新增 `tauri.macos.conf.json`，把 `engine/`、`runtime/RUNTIME.lock`、`runtime/dsh-runtime`、`runtime/dsh-home-seed` 打进 `Contents/Resources`；`repo_root_from_exe` 增加 Resources 查找。这样应用可以单独拖进 `/Applications`，不受同目录文件与 translocation 影响。
2. **免安装脚本去重**：macOS 分支不再把运行时复制到 `.app` 旁边，zip 里只有一个自带运行时的 `.app` 与启动说明，体积不翻倍；打包前校验 `.app` 内确有 `engine/` 与 `runtime/`，避免误用 target 里旧的空壳构建。
3. **兜底收紧**：编译期路径只有在真的包含 `engine/omnidesk_engine` 与 `runtime/RUNTIME.lock` 时才认；否则返回 None，让界面报「安装不完整」。构建机路径不再出现在用户界面上。
4. **文案修正**：运行时目录缺失显示「不完整 / 未找到」，不再叫「版本过低」；该行不再重复展示「需要 运行时目录」。
5. **验证**：`find_bundle_root` 抽出成可测函数，新增三种布局的单元测试；新增 `verify_macos_bundle_136.py` 校验资源清单、安装根识别、打包守卫与 `.app` 内四个路径；用户本机验证自带运行时的 macOS 包可启动（2026-11-04）；0.2.8 发布后已用 `unzip -Z1` 核对 zip 内结构（33,126 项，四个运行时路径齐全，无同目录副本）。

#### 影响范围
`tauri.macos.conf.json`（新增）、`platform.rs`（安装根识别与兜底）、`scripts/build-portable-package.sh`（macOS 去重）、`release.yml`（macOS 构建前准备资产）、前端体检文案、新验证脚本。

#### 风险
- macOS `.app` 变为约 480MB（zip 约 150MB），首次启动从包内复制 seed（约 8MB）。
- 未签名应用首次打开仍需右键「打开」或去隔离属性；这与包内资源无关。

#### 验收标准
macOS 用户把 `OmniDesk.app` 拖进 `/Applications` 后双击即可启动，不再出现 `/Users/runner/...` 之类的构建机路径；环境检查页在运行时缺失时显示「不完整」而不是「版本过低」。
### OD-ARCH-135：Windows 安装器补运行时资产 + 启动可读报错 + 凭据服务存活判定（计划者与实施者：本助手；用户反馈「安装后只有 exe 与 uninstall」；2026-11-04 已实施，见 REVIEW.md / STATUS.md / tests/verification/desktop/evidence/od-arch-135/facts.json）

> **触发：** 用户装完 v0.2.6 的 `x64-setup.exe`，安装目录里只有 `omnidesk-desktop.exe` 与 `uninstall.exe`；双击没有反应。同一版本还暴露出「保存模型密钥时报『地址没有留在内存里』」。

#### 已核实根因（本人实读源码 + 实机复核）

1. **NSIS 只打 exe。** `bundle.resources` 从未配置。`engine/`、`runtime/dsh-runtime`、首启 seed 只有 `scripts/build-portable-package.sh` 知道，Tauri bundler 完全不知道它们存在。
2. **启动失败在 GUI 下无声。** `resolve_python` / `locate_engine_dir` 拿不到路径就 `exit(2)`，`eprintln!` 没有终端可看；`windows_subsystem = "windows"` 下表现为「双击毫无反应」。
3. **凭据服务的存活检查在 Windows 上恒 false。** `dsh_settings::process_alive` 用 `kill -0`，Windows 没有 `kill.exe`，命令构造即失败；而 `reusable()` 要求「进程存活 && 端口监听」同时成立，于是刚拿到地址就被判成不可用。

#### 方案（已实施）

1. **Windows 专属资源清单** `desktop/src-tauri/tauri.windows.conf.json`：把 `engine/`、`runtime/RUNTIME.lock`、`runtime/dsh-runtime/`、`runtime/dsh-home-seed/` 映射到安装根目录。只挂 Windows：macOS 的 `.app` 由免安装目录带这些文件，重复打包会让包体翻倍。
2. **抽出 `scripts/prepare-runtime-assets.sh`**：裁剪 `dsh-runtime` 的 `*.ts` / `*.map` / 测试目录，重建 seed（两个内置 Profile 的 npm 依赖 + 仓库 Skill）。免安装脚本改为调用它，两处共用一份实现。
3. **CI 顺序**：Windows job 在 `tauri build` 之前跑准备脚本，否则资源清单指向不存在的目录。
4. **启动失败弹窗**：`run()` 的 `build_state` 失败分支改走 `report_fatal_startup`，Windows 用系统消息框（PowerShell `MessageBox`，文本走环境变量避免转义）把原因显示出来，其余平台保持 stderr。
5. **凭据服务存活判定**：`reusable()` 改为「端口连得上即可用」，删除依赖 `kill -0` 的 `process_alive`。真正的鉴权在随后的 `exchange_token`，不需要进程表再确认一遍。
6. **安装完整性进体检**：`environment_report` 增加第三项 `assets`（`engine/omnidesk_engine` 与 `runtime/RUNTIME.lock` 是否就位），缺了就在体检页说「安装不完整，请重新运行安装程序」。
7. **验证**：新增 `verify_windows_installer_135.py`（27 项）；本地用同一份资源清单构建 `.app` 验证目录真的进包；播种夹具隔离 `OMNIDESK_REPO_ROOT`，避免仓库里生成 seed 后单测前提变化。

#### 影响范围
`tauri.windows.conf.json`（新增）、`scripts/prepare-runtime-assets.sh`（新增）、`scripts/build-portable-package.sh`、`.github/workflows/release.yml`、`.gitignore`、`lib.rs`（启动弹窗）、`dsh_settings.rs`（存活判定）、`platform.rs`（体检第三项与单测）、前端体检页与类型。

#### 风险
- 安装器体积从 2.7MB 涨到约 200MB 量级，安装时间变长；这是自带运行时必然的代价。
- NSIS 要写约 3 万个文件，路径长度与安装耗时按实机确认；若个别机器受限，再考虑单文件归档 + 首启解压。
- `runtime/dsh-home-seed` 依赖 npm install，CI 需要网络；装不上时脚本警告并继续（基础对话不受影响）。

#### 验收标准
Windows 安装后，安装目录里能看到 `engine/`、`runtime/dsh-runtime/`、`runtime/dsh-home-seed/`；首次启动自动铺到 `%USERPROFILE%\OmniDesk\runtime\dsh-home`，双击即可打开；保存模型密钥一次成功；人为删掉 `runtime/` 后启动会看到「安装不完整」提示。

> **实施结果（2026-11-04）：** 资源清单、准备脚本、CI 顺序、启动弹窗、存活判定、体检第三项全部落地。本机用同一份清单构建 `.app`：体积 6.68 MiB → 476.63 MiB，`Resources/engine`、`Resources/runtime/dsh-runtime`、`Resources/runtime/dsh-home-seed` 均在。`cargo test --offline --lib` 505 项通过（并行连跑两次）；`npm build` 通过；`verify_windows_installer_135.py` 27/27。v0.2.7 已出包：安装器 **90.4 MB**（此前 2.7 MB），Windows/macOS 构建均成功；发布 job 因「cp 到自身」失败一次，已修，本版资产手动补挂。安装器实机表现待 Windows 用户验证。
### OD-ARCH-134：Windows 安装器与启动体验修复（计划者与实施者：本助手；用户授权「批准 执行 OD-ARCH-134」；2026-11-04 已实施并通过审查，见 REVIEW.md / STATUS.md / tests/verification/desktop/evidence/od-arch-134/facts.json）

> **触发：** 用户实测 Windows 包（v0.2.x）：① 下载到的是纯 zip，没有像原版 DSH-desktop 的安装向导；② 双击 OmniDesk.exe 后 node 终端一闪而过，应用无法启动。

#### 已核实根因（本人实读源码）

1. **无安装器是打包命令造成的。** .github/workflows/release.yml:125 用 `npx tauri build --features custom-protocol --no-bundle`，明确跳过 bundle；产物只有 build-portable-package.sh 压的 zip。Tauri 2 自带 NSIS 安装器支持，未启用。
2. **子进程在 Windows 上闪黑框。** 全仓无 CREATE_NO_WINDOW（0x08000000）：engine_ipc.rs EngineChannel::spawn（python 引擎）、dsh_settings.rs spawn（node/DSH）、platform.rs spawn_opener 都没设 creation_flags。GUI 进程拉起控制台子系统子进程时 Windows 必弹控制台窗，失败时"一闪而过"。
3. **node/python 解析无 Windows 分支。** platform.rs resolve_node 的候选路径全是 Unix（bin/node、/opt/homebrew…），nvm-windows/Program Files\nodejs 不在列；resolve_python 只在 PATH 里找。Windows 双击启动时若 PATH 没有这两样，引擎起不来且报错只进日志，用户看不到。
4. **免安装包要求用户预装 Node 22+/Python 3.10+**（启动说明.txt），这是"起不来"的直接原因之一，但没有前置检查与友好提示。

#### 方案

**一句话：启用 NSIS 安装器作为 Windows 主发布物；全部子进程加防闪窗标志；node/python 找不到或版本不对时在应用窗口内给出可操作的提示页，而不是无声失败。**

1. **NSIS 安装器（release.yml + 本地脚本）**
   - Windows job 增加 `npx tauri build --features custom-protocol --bundles nsis`，产物 `OmniDesk_\<ver\>_x64-setup.exe` 上传 Release 为主下载项；zip 免安装包保留为进阶选项。
   - tauri.conf.json 增加 bundle.windows.nsis 配置（安装模式 perMachine=both、语言简体中文、允许改安装目录）。
2. **防闪窗（Windows 专属）**
   - platform.rs 新增 `fn no_window() -> Option<CREATE_NO_WINDOW>`（cfg windows 返回 0x08000000，其余平台 None）；engine_ipc.rs、dsh_settings.rs、platform.rs spawn_opener、lib.rs 两处直接 Command spawn 全部套用。
3. **依赖探测与友好失败**
   - resolve_node 增加 Windows 候选：%ProgramFiles%\\nodejs\\node.exe、nvm-windows（%APPDATA%\\nvm）、scoop；resolve_python 增加 %LocalAppData%\\Programs\\Python 与 py 启动器探测。
   - 启动引擎前先跑一次探测：失败时前端弹独立"环境检查"页（Node/Python 版本、下载链接、OMNIDESK_NODE/OMNIDESK_PYTHON 设置说明），不再黑框一闪。
4. **验证**
   - 本地可验证：cargo test（新增 no_window 单测 + 探测函数单测，Unix 上跑逻辑分支）；npm build；防闪窗与 NSIS 走 CI 实测（tag 后看 Release 产物）。
   - 用户实测项：新安装器双击安装、改目录、桌面快捷方式；无 node 的机器上应看到提示页而非闪退；装好 node 后正常启动且全程无黑框。

#### 影响范围
.github/workflows/release.yml、tauri.conf.json、platform.rs、engine_ipc.rs、dsh_settings.rs、lib.rs（探测命令 + 启动前置检查）、前端环境检查页、启动说明.txt。

#### 风险
- NSIS 在 CI 的首跑可能有配置细节问题（语言包、图标），用 tag 后 Release 产物核验，必要时一轮修正。
- 探测分支在 macOS/Linux 上不可实测 Windows 路径，单测只覆盖逻辑，真实行为以用户实测为准。
- 防闪窗不影响 stdout/stderr 管道，但需回归引擎 IPC 全套测试确认无回归。

#### 明确不做
- 不把 node/python 运行时打进包里（体积+许可问题，等用户反馈再议）。
- 不改免安装包形态本身，仍保留。

#### 验收标准
Release 页出现 x64-setup.exe；用户在两台 Windows（有/无 node）上分别验证：一台向导安装后正常启动，另一台看到环境提示页。全程无黑色终端闪现。

> **实施结果（2026-11-04）：** 安装器、防闪窗、依赖体检三块全部落地。`cargo test --offline --lib` 504 项通过（新增 5 项平台层单测）；`npm --prefix desktop run build` 通过；`verify_windows_packaging_134.py` 36 项全通过。v0.2.6 已由 CI 出包：NSIS 产出 `OmniDesk_0.2.6_x64-setup.exe`（2.71 MiB），Release 同时挂了 macOS zip、Windows zip 与安装器。首轮发布因 artifact 目录层级导致安装器没被挂上，发布 job 已加「摊平」步骤修正。向导安装、无 node 机器上的检查页仍需 Windows 实机确认。
### OD-ARCH-133：macOS 上的引擎进程核验与回收（计划者与实施者：本助手；用户授权「授权执行OD-ARCH-133」；2026-11-03 已实施并通过审查，见 REVIEW.md / STATUS.md / tests/verification/desktop/evidence/od-arch-133/facts.json）

> **触发：** 132 收尾时跑全量单测，4 项平台失败里有 3 项落在 `process_owner`。核对后确认不是测试写错，而是**运行期缺陷**：宿主与引擎两侧的进程核验都直接读 Linux 的 `/proc`，而 macOS 发布包（v0.2.4 / v0.2.5 用户实机测试）跑的正是这套代码。

#### 已核实的前提（本人实读源码 + 实机复现）

1. **`/proc` 在 macOS 上不存在。** `process_owner.rs` 的 `boot_id` / `proc_stat` / `proc_cmdline` / `group_members` 全部读 `/proc`；`engine/omnidesk_engine/process_owner.py` 同构。macOS 上这四个函数一律返回空。
2. **后果一：单实例互斥失效。** `reclaim_before_start` 里 `current_boot.is_empty()` 直接判「上次开机残留」并删掉 `owner.json`，于是第二个实例不会被拒绝。
3. **后果二：遗留进程组收不回来。** `group_members` 恒空 → `terminate_group` 直接返回、`cleanup_engine_group` 恒返回 None，引擎强杀后遗留的 DSH 子进程只能靠用户手工 kill。
4. **Python 侧同样失效。** `inspect_owner` 在 macOS 上恒判 `pid_reused`（因为 `_proc_stat` 为空），`claim_ownership` 记下的 `boot_id` 是空串。
5. **实机基线（本机 macOS，2026-11-03）。** `python3 -m unittest test_process_owner_unit` → **10 项里 4 项失败**（`active_engine` 判成 `pid_reused`、`child_pid` 记成 0 等）；`cargo test --offline --lib` → 4 项失败，其中 3 项在 `process_owner`。
6. **macOS 上可用的等价接口（本人实测）。** `ps -A -ww -o pid=,pgid=,stat=,args=` 能给出 pid / 进程组 / 状态 / 完整命令行；`sysctl -n kern.boottime` 给出 `{ sec = 1793505130, ... }`；`kill -s TERM -- -<pgid>` 对进程组有效（实测 rc=0，BSD 只认带 `--` 的写法）。

#### 目标

让进程核验与回收在 macOS 上按与 Linux 相同的口径工作：同一数据根只允许一个引擎；身份核验通过才回收；核验不通过一律不杀。Windows 不在本轮（没有进程组概念，要用 Job Object，单独立项），但代码必须保持可编译且不误杀。

#### 实施步骤

1. **`platform.rs` 增加进程后端**（跨平台差异集中在这里，与既有「家目录 / 打开器 / Python 解释器」同一层）：`boot_id()`、`process_stat(pid)`、`process_cmd(pid)`、`process_table()`。Linux 读 `/proc`（行为不变）；macOS 用 `ps` 与 `sysctl kern.boottime`（boot id 记成 `darwin:<sec>`，与 Linux 的 UUID 天然不同源，不会互相误认）；Windows 返回 `None` / 空表并注明不参与回收。
2. **`process_owner.rs` 改调后端**：删除本文件内的 `/proc` 读取，改为委托 `platform`。判定顺序、拒绝条件、`/bin/kill` 的 `-s SIG -- -PGID` 调用与自证逻辑全部保持原样——本轮只换「怎么看进程」，不换「什么情况下能杀」。
3. **`engine/omnidesk_engine/process_owner.py` 同步**：`boot_id` / `_proc_stat` / `_proc_cmdline` / `group_members` 按 `sys.platform == "darwin"` 走 `ps` 分支，Linux 分支逐行不动。命令行在 macOS 上没有 argv 数组，统一按空白切分（同一次登记与同一次核验用同一变换，比较仍然稳定）。
4. **测试口径**：`task_shell` 那条失败是 `dir.display()` 与规范化路径比较，属测试写死 Linux 路径形态，改为两侧都 `canonicalize` 后比较。
5. **验证**：`cargo test --offline --lib`（process_owner / task_shell 全绿）；`python3 -m unittest test_process_owner_unit` 10/10；`verify_process_lifecycle_074.py` 不退化；前端 build 不受影响但照跑一次。
6. **测试隔离**：并行跑全量单测时发现 `task_shell::offline_artifact_*` 偶发失败，根因是四个测试模块各自持锁、`lib.rs::command_tests` 干脆不持锁，同进程环境变量互相覆盖。统一为 crate 级 `test_env_lock::ENV_LOCK`（`#[cfg(test)]`），四个模块共用，`lib.rs` 那条改为保存/还原原值。只改测试代码。

#### 影响范围

`platform.rs`、`process_owner.rs`、`engine/omnidesk_engine/process_owner.py`、`task_shell.rs` 的一处测试断言；以及仅测试代码的 `lib.rs` / `settings.rs` / `engine_supervisor.rs`（共用环境锁）。不动 Skill、不动 UI、不动数据目录结构；`owner.json` 字段不变。

#### 风险

- `ps` 每次核验 fork 一个进程：只在启动回收 / 关闭兜底两条低频路径上调用，不进热路径。
- macOS 的命令行按空白切分有损（路径带空格）：因此**不把 argv 相等当作唯一判据**，仍然要求 pid + pgid + boot id + 「组内全是我方进程」同时成立；宁可拒绝回收，也不误杀。
- 旧 `owner.json`（boot_id 为空）在升级后会被判为无法核验：按其原有保守语义只删登记、不杀进程，用户手动重启即恢复。

#### 验收标准

macOS 上：第二次启动同一数据根被拒绝并给出可读原因；引擎被强杀后其 DSH 子进程组能被收掉；核验不通过时一个进程都不杀（旁观进程存活）。Linux 行为零变化。
### OD-ARCH-132：随手记补充内容并入上一条（计划者与实施者：本助手；用户已批准「批准执行OD-ARCH-132」；2026-11-03 已实施并通过审查，见 REVIEW.md / STATUS.md / tests/verification/desktop/evidence/od-arch-132/facts.json）

> **触发：** 用户实测发现：先记一条，再补一句，落盘成了两条独立记录。原因有二：① Skill 规则只让模型"跟随上一条的日期"，落盘永远新起一条，没有"并入上一条"的通道；② 工作记录文件是只增不改设计（append_entry 自检"原字节不变"），从机制上就不支持合并。

#### 目标
明显续述上一条的补充（"还参观了…""另外…""补充一下…"）并进上一条同一行，不再分成两条。

#### 实施步骤

1. **worklog.rs 新增 merge_entry（合并落盘通道）**：签名对齐 move_entry（date + index + expect_text 三元组定位），确认目标无误后把补充文本以"；"接在该条同一行末尾，写临时文件 + rename 原子替换；自检为重写前后除目标行外逐字节一致，否则整份作废不落盘。追加文本照走掩码（身份证/手机号）。Tauri command：worklog_merge。
2. **skills/memory-agent/SKILL.md 规则升级**：判定输出增加 "merge" 通道：{"merge":{"date":"...","index":0,"text":"补充原话"}}；规则第 3 条改为：无日期细节且明显续述上一条主题（同一次行动的补充、展开、感受追加）→ merge；全新主题 → append 今天。改写指令、纯提问、确认句行为不变。
3. **前端与索引兼容**：Receipt 带合并后的行文本，weekTimeline.ts 重读即可，无需新 UI。执行顺序定为 merge → move → append：并入不增删条目、不改序号，放在移动之前才不会拿到被移动改过的旧序号。
4. **验证**（tests/verification/desktop/verify_worklog_132.py）：合并后文件除目标行外逐字节一致；索引错位 / expect_text 不匹配时拒绝且文件未动；追加文本含身份证号被掩码；move/append/merge 混合序列正确；cargo test worklog 全量 + tsc + build 通过。

#### 影响范围
worklog.rs、lib.rs（一个新 command）、memory-agent/SKILL.md、新增验证脚本。不动周报（memory-report）与知识卡链路。

#### 风险
- 整文件重写替代纯追加：用"临时文件 + rename"保证原子，崩溃不留半写文件。
- 模型误判"续述"导致该分开的被合并：合并后原句仍在，时间轴可见；低频问题，手工改正即可，不做自动拆分。

#### 验收标准
用户场景复现：先记"赴上杭县城现场教学"，再补"参观了紫金矿业…"，落盘为一条含两段的记录。

> **实施结果（2026-11-03）：** merge_entry / worklog_merge / 判定协议 merge 通道 / Skill 规则 / 回执展示全部落地；verify_worklog_132.py 43 项全通过，verify_worklog_104.py 70 项无退化，cargo test worklog 42 项通过，前端 build 通过。全量 cargo lib 有 4 项与本计划无关的平台失败（process_owner、task_shell，均为 Linux /proc 与 macOS 符号链接路径差异），改动未触及这两个模块。

### OD-ARCH-131：自建 Profile 能带自己的 Skill（不再只认内置 8 个）（计划者与实施者：本助手；用户授权「把 Profile 功能全部完善，中途的计划自行制定并批准，写入文档，阶段性备份」；2026-10-29 已实施并通过审查，见 REVIEW.md / STATUS.md / D-060）

> **触发：** 用户重申产品边界——「OmniDesk 是干净和初始的环境，Profile 是定制的，切换 Profile 时要能自动写入所需的配置、Skill 和插件，缺插件要能自动安装」。130 之后，切换装配、保存绑定后的即时收敛、缺插件三条补装通道都通了，逐项自查发现还剩一个会挡住真实定制的地方：**自建 Profile 只能绑程序内置的 8 个 Skill**。用户自己写的一个 Skill（比如某个单位专用的写法）放进去也存不住：规则校验按 `ALLOWED_SKILLS` 白名单拒绝，装配链也没有「这个 Profile 自带的 Skill 源码」这一层。

#### 已核实的前提（本人实读源码）

1. **白名单是硬拒绝。** `profile_rules.rs:244` 在 `validate()` 里判 `ALLOWED_SKILLS.contains`，不在 8 项内就整份规则包解析失败——连保存都进不去。
2. **取源只有两处。** `expected_skill_files`（`profile_assets.rs:696`）只认「仓库 `skills/<name>`」与「编译期内嵌副本」，没有第三个来源；`materialize_custom`（`:1186`）另外重复了一遍同样的两段挑源代码。
3. **界面只列白名单。** `allowed_skills` 由 `profile_rules::allowed()` 生成，前端 `SettingsPanel.tsx:3204` 照着渲染勾选片，所以自建 Skill 连选都选不到。
4. **迁移包不收 Skill。** `profile_lifecycle.rs` 的 `APP_BUNDLE_FILES` 只列 `decl.yml / rules.json / categories.json` 三个单段文件，Skill 目录进不了包。
5. **没有会被破坏的既有口径。** 基础标准仍然零写入；出厂 Profile 仍按编译期清单取源；现有 8 个 Skill 的装配与停放行为不变。

#### 方案

**一句话：给自建 Profile 一个属于自己的 Skill 源目录 `<应用数据根>/profiles/<名字>/skills/<skill名>/`，取源顺序变成「Profile 自带 → 仓库 → 内嵌」，并把这份目录接进装配、巡检、绑定界面与迁移包。**

- **命名校验与白名单分工（profile_rules.rs）**：`ALLOWED_SKILLS` 降为「内置清单」，新增 `is_valid_skill_name` 做**形状**校验（小写字母／数字／`-`／`_`，1–48 字符）。`validate()` 改用形状校验，非法名照旧整份拒绝；`..`、大写、空格、中文标点这类写不出目录的仍然进不去。绑定了一个不存在的名字时不再让整份规则坏掉，而是在资产页显示「找不到正文来源」。
- **取源升级（profile_assets.rs）**：`expected_skill_files(scope, name)` 改为收 `Scope`，返回带来源标记的三态（Profile 自带 / 仓库 / 内嵌）。三处调用（保存绑定收敛、自建 Profile 装配、巡检）统一走这一条，删掉 `materialize_custom` 里重复的挑源分支。`sync_skill_files` 的三态写不变：用户改过就不覆盖，缺失才补。
- **界面（SettingsPanel.tsx 规则资产页）**：`allowed_skills` 由后端拼成「内置 ∪ 该 Profile 自带」；绑定勾选片因此能选到自己写的 Skill。区块里加一行「Profile 自带 Skill」，两个动作：打开该目录（首次点才建）、按名字生成一份带 frontmatter 的模板（已存在不覆盖）。
- **命令（lib.rs）**：新增 `profile_skill_source_open` 与 `profile_skill_scaffold`，都拒绝基础标准，写前走 `assert_writable_root` 同口径路径校验。
- **迁移包（profile_lifecycle.rs）**：包成员新增 `root: "skill"`，路径形如 `<skill名>/SKILL.md`，逐段校验、拒绝上跳与隐藏文件；导出带上该 Profile 自带的全部 Skill 文件，导入按同一白名单校验后落回 `profiles/<名字>/skills/`。旧包（没有 skill 成员）照常导入。
- **测试**：新增单测覆盖「自建 Skill 能通过校验」「非法名仍被拒绝」「装配优先用 Profile 自带源」「迁移包带 Skill 往返」；现有 `bad_skill_test` 改为断言非法形状仍被拒。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

沿用 129 已验收的直角行式区块与统一悬停说明，不新增弹层；「自带 Skill」作为绑定区块内的一行动作条，与「声明插件」同一视觉层级。

#### 明确不做

- 不新增 Skill 市场、不联网下载 Skill：Skill 仍由用户放进目录、随 Profile 自带或随程序发布。
- 不改基础标准零写入口径，不改出厂 Profile 的编译期清单。
- 不改数据根与会话归属（D-052）。
- 不 commit / push（未获授权）。

### OD-ARCH-130：绑定保存后立刻把 Skill 放进当前 Profile 的运行时目录（计划者与实施者：本助手；用户授权「把 Profile 功能全部完善，中途的计划授权你制定并批准执行」；2026-10-29 已实施并通过审查，见 REVIEW.md / STATUS.md / D-059）

> **触发：** 用户重申产品边界——「omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能」。逐条对表后，切换这条链是通的（配置 / 场景 / 规则 / Skill 在切换时补齐，缺插件切换后与冷启动都会后台补装），但 **在设置页给当前 Profile 保存 Skill 绑定之后，那几个 Skill 并不立刻落到运行时目录**：profile_skill_bindings_save 只写 rules.json，要等下一次切换 Profile 或重启引擎才会由装配链补。中间的窗口里，界面显示「已绑定」，实际运行时里没有那本 Skill。

#### 已核实的前提（本人实读源码）

1. **切换即装配已经在位。** lib.rs:1233 切换时 profile_assets::materialize → record_managed_skills；engine_supervisor.rs:295 启动引擎前又走一次 reconcile_managed_skills + materialize。出厂 Profile 按编译期清单、自建 Profile 按 rules.json 的 skill_bindings（profile_assets.rs:191 / :1092）补 Skill。
2. **缺插件有三条自动通道。** 切换后 spawn_profile_plugins_autofill（:1340）、冷启动 maybe_autofill_plugins_on_startup（:1448）、设置页手动「安装」，都受 profile_auto_install_plugins 开关（默认开）约束，且基础标准恒不装。
3. **保存绑定的路径没有接上装配。** lib.rs:2120 的 profile_skill_bindings_save 只调 save_skill_bindings_into（写规则 + 备份 + 原子写）就返回，回执只说「Skill 绑定已保存」。前端 SettingsPanel.tsx:1111 因此提示「下一轮提交生效」，但运行时目录里其实还没有这些 Skill 目录。
4. **补齐所需的原语都已存在。** expected_skill_files（仓库优先、内嵌兜底）、sync_skill_files（三态写、不覆盖用户改动）、reconcile_managed_skills（把解绑的移入停放区、把需要的从停放区归位）、record_managed_skills（登记归属便于切走时收回）。缺的只是一条「保存之后立刻按新绑定收敛」的窄入口。

#### 方案

**一句话：绑定写盘成功后，立刻按同一份 rules.json 收敛当前 Profile 的全局 skills/ 目录——该补的补、该归位的归位、解绑的移入停放区，并把明细写进回执；基础标准仍零写入。**

- **后端窄函数（profile_assets.rs）**：新增 BoundSkillSync 与 sync_bound_skills(scope, standard)。逻辑复用既有原语：wanted_skills 取清单 → reconcile_managed_skills（归位 / 停放）→ 逐个 expected_skill_files + sync_skill_files（三态写，不覆盖用户改动）→ 保存装配状态 → record_managed_skills。找不到正文来源的记入 missing_source，不中断其余。
- **命令（lib.rs）**：profile_skill_bindings_save 在规则写盘成功后调用 sync_bound_skills。**规则已经写入就不回滚**（绑定是用户的决定，Skill 拷贝失败不该让配置回到旧值），但回执里的 skills 明细如实写明补了哪些、哪几个缺源；基础标准在 save_skill_bindings_into 首句就被拒，走不到这一步。
- **回执与界面**：ProfileRulesSaveReceipt 增 skills；设置页保存后的提示改为「绑定已保存 + 已补齐 / 已在位 N 个 Skill」，不再笼统写「下一轮提交生效」。
- **测试**：profile_assets 新增两个单测（自建 Profile 绑定后 Skill 目录真实出现且二次保存不重复写；基础标准下 skills/ 目录与登记表都不出现）。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

本轮不改版式，只改一条回执文案与后端行为，沿用 129 已验收的行式区块与统一悬停说明规范，不新增弹层与控件。

#### 明确不做

- 不新增 Skill 市场或联网下载：Skill 仍随 OmniDesk 发布（编译期内嵌）或由用户放进仓库目录。
- 不在保存绑定这条路径上联网安装插件（插件补装仍走切换 / 冷启动 / 手动三条既有通道）。
- 不改基础标准的零写入口径，不改出厂 Profile 的固定清单。
- 不 commit / push（未获授权）。

### OD-ARCH-129：自建 Profile 的场景 × Skill 绑定可在界面里配（不用手写 JSON）（计划者与实施者：本助手；用户授权「把 Profile 功能全部完善，中途的计划授权你制定并批准执行」；2026-10-29 已实施并通过审查，见 REVIEW.md / STATUS.md / D-058）

> **触发：** 用户的产品边界是「OmniDesk 是干净初始环境，Profile 是定制的，切过去就该是专属工作台」。128 之后建 / 复制 / 切换 / 装配 / 补装 / 巡检 / 移除 / 恢复 / 改名 / 迁移 / 声明插件都通了，但**场景绑哪些 Skill 只能靠手写 rules.json 的 skill_bindings**：普通用户在设置页只能看到一行只读摘要，配不出来「工作记录 → memory-agent 只预注入这一个」这类底线。装配链本身已就绪（wanted_skills 与 materialize_custom 都按这份绑定补 Skill），缺的是入口。

#### 已核实的前提（本人实读源码）

1. **绑定是装配的唯一来源。** profile_assets.rs:191 wanted_skills 与 :1092 materialize_custom 都只读 rules.json 的 skill_bindings；自建 Profile 的起步规则包刻意写空绑定（profile_starter.rs:79），所以新 Profile 现在必定零预注入。
2. **校验器已存在且严格。** profile_rules.rs:229 validate 限制每场景 max_injected 在 1..=3、绑定数不超过它、技能必须在 ALLOWED_SKILLS（8 项）内且不重复。界面不该另写一套规则，只该把选择交给这条校验。
3. **写入通道已有备份与原子写。** lib.rs:2157 write_profile_rules 先备份成 .bak-<epoch> 再写；128 已把「基础标准拒绝且不建目录」放在 save_rules_into 首句。新增命令复用同一入口即可，不新开写盘路径。
4. **界面目前只有只读摘要。** SettingsPanel.tsx:3049 把绑定拼成一行文字展示，真正编辑只能改下面的 JSON textarea；普通用户不知道 weekly-note / weekly-report 这类流程键该怎么写。

#### 方案

**一句话：在「规则资产」页加一块行式「场景 × Skill」勾选表，点选后整表保存；后端把整张绑定表替换进现有 rules.json（其余规则一个字节不动），仍走统一校验与备份。**

- **后端纯函数**（profile_rules.rs）：set_skill_bindings(text, bindings) -> Result<String, String>：解析现有规则 → 用传入映射重建 skill_bindings（空数组表示解绑、去重、max_injected = max(1, 绑定数)）→ validate() → 序列化回 pretty JSON。解析失败、技能不在清单、超上限一律整体拒绝，不产出半份文本。
- **命令 profile_skill_bindings_save**（lib.rs）：基础标准直接拒绝且不建目录；读现有规则文件（不存在则用 profile_starter::rules_text 的起步骨架）→ set_skill_bindings → write_profile_rules（沿用备份 + 原子写）。返回 ok / path / backup / digest / note。
- **界面**（SettingsPanel.tsx 规则资产页）：状态卡片下方新增「场景 Skill 绑定」区块，按行列出要配的场景（工作记录、工作记录 · 随手记、工作记录 · 周报、文案写作、项目跟进），每行右侧是该行可选的 Skill 勾选片；勾选只改本地草稿，行尾「保存绑定」一次性提交，未保存时显示提示。基础标准下整块不渲染（后端同样拒绝）。
- **类型 / API**：RulesView 增 allowed_skills（由后端给出，避免前后端两份清单漂移）；types.ts 同步；api.ts 增 profileSkillBindingsSave。
- **测试**：profile_rules 增 4 个单测（新建绑定、解绑清空、非法技能拒绝且原文不变、超 3 个拒绝）；lib.rs 增基础标准拒绝保存绑定的行为测试；窗口脚本覆盖勾选→保存→重读绑定生效。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

用 screenshots-design-inspiration 检索「settings page assign skills to workflow checkbox rows」，返回 Shopify 设置页一类界面：能力归属用勾选行表达，保存动作收在区块行尾。据此把绑定做成设置页既有的直角行式列表 + 行内勾选片，不新增弹层，也不再要求用户手写 JSON 才能配出专属工作台。

#### 明确不做

- 不新增 Skill 安装通道（Skill 仍随 OmniDesk 发布或由用户放进目录）。
- 不改 max_injected 上限 3，也不在界面暴露任意数值输入：绑定几个就注入几个的底线够用。
- 不自动切换 Profile、不自动重启引擎；保存后下一轮提交即生效。
- 不 commit / push（未获授权）。


### OD-ARCH-128：Profile 装配链补两处真缺口（基础标准零写入收口 + 自建 Profile 声明插件）（计划者与实施者：本助手；用户授权「把 Profile 功能全部完善，中途的计划授权你制定并批准执行」；2026-10-29 已实施并通过审查，见 REVIEW.md / D-057）

> **触发：** 127 之后主干（建 / 复制 / 切换 / 装配 / 补装 / 巡检 / 移除 / 恢复 / 改名 / 迁移）通了，但按「omnidesk 是干净和初始的环境，profile 是定制的，切换 profile 要自动写入所需配置、skill 和插件，缺插件能自动安装」逐条对表，仍有两处断点：**① 基础标准能写出规则文件**，干净底座的分界被自己打开一个口子；**② 自建 Profile 要加插件只能手改 `package.json` / `cordis.patch.yml`**，界面里没有登记入口，用户无法从零把一套定制工作台配出来。

#### 已核实的前提（本人实读源码，非推测）

1. **基础标准下 `profile_rules_save` 没有拦截。** `lib.rs:2067` 只做大小与解析校验，直接调 `write_profile_rules(&profile_rules_dir(&state), &text)`；`profile_rules_dir` 返回 `<应用数据根>/profiles/<当前 Profile>/`，`write_profile_rules` 第一句就是 `create_dir_all`。对照 `profile_decl_save`（`lib.rs:1041` 的 `save_decl_into`）已有 `standard` 拒绝分支，规则这条是漏的。后果不止「多一个目录」：`current_rule_policy` 从同一路径读规则，干净底座一旦写下业务规则就会被准入策略采用。
2. **自建 Profile 的插件清单只有两个来源。** 创建时 `required_plugins(dir, dsh_home)` 从 `package.json` 依赖与 `cordis.patch.yml` 里探测一次，写进登记表 `index.v1.json` 的 `plugins`；之后 `required_plugins(scope, standard)`（`profile_assets.rs:1660`）只读这张表。全仓没有任何命令能改 `entry.plugins`，只有 `materialize_custom` 在探测到新依赖时顺手回写（`profile_assets.rs:1202`），而这条路径要求用户先用别的方式把依赖写进 DSH 侧文件。
3. **安装通道已经存在且只装缺件。** `install_plugin(dsh_cli, dsh_home, profile, package, version)` 走 `dsh plugin --profile <p> add <pkg>[@ver]`，已处理 pnpm 的 `strictDepBuilds` 重试（OD-ARCH-120）；`ensure_plugins` 与三个触发点（手动 / 切换后 / 冷启动后）共用 `required_plugins` 这张表。所以「声明插件」只需把包名写回登记表，后续自动补装立刻对它生效，不需要新机制。
4. **只读巡检漏掉外部新装的插件。** `inspect_custom`（`profile_assets.rs:1231`）只列 `entry.plugins`；若用户在命令行装了插件但没切换过，设置页看不到它，与 `materialize_custom` 的判定不一致。
5. **规则资产页已有 JSON 编辑 + 校验保存。** 自建 Profile 声明 Skill 绑定（`rules.json` 的 `skill_bindings`）在界面上是可写的，`profile_rules::validate` 会拒非法技能名与超限。故这一轮不新增可视化 Skill 绑定器，只把「绑定后 Skill 会自动补齐」这条链打通验证。

#### 方案

**一句话：基础标准下规则保存一律拒绝且不建目录；自建 Profile 增加「声明一个插件」入口，经文 DSH 官方插件通道安装，成功后把包根写进登记表，让「切换装配 + 缺件补装」照旧接管。**

- **A. 基础标准零写入收口**
  - `profile_rules_save` 在解析前先取 `profile_asset_scope(&state)`，`standard == true` 时返回错误说明「基础标准是干净底座，不安装行业规则包；切到专业 Profile 再保存」，**不解析、不建目录、不落盘**（与 `profile_rules_restore_default` 同口径）。
  - 前端规则资产页：基础标准时「校验并保存」按钮禁用，`data-hover-hint` 说明原因；文案区仍可粘贴查看，不影响只读观察。
- **B. 自建 Profile 声明插件（新命令 `profile_plugins_add`）**
  - 入参 `profile`、`package`（可写 `pkg` 或 `pkg@1.2.3`，scoped 包同理）。校验：名字合法、必须是已登记的自定义 Profile（拒绝基础标准与出厂 Profile）、包根经 `package_root` 归一后满足「非空、无空白、无 `..`、不以 `.` 或 `/` 开头、长度 ≤ 214」。
  - 已登记且本地就绪 → 直接返回「已就绪、未重复安装」；已登记但缺件 → 走安装。
  - 安装走既有 `install_plugin`（联网，只在用户点按钮时发生）。**只有安装成功后**才把包根写入登记表 `entry.plugins`（去重、保持既有顺序、新项追加），写失败则在回执里如实说明；安装失败不改登记表。
  - 回执含 `ok / package / already_registered / installed / note`，界面一行中文说明；不弹窗、不自动重启引擎。
- **C. 只读巡检与装配同源**：`inspect_custom` 的插件清单改为 `entry.plugins` ∪ `detect_plugins(dsh_profile_dir)`（只读、不写登记表、不装任何东西），避免「命令行装过、设置页看不见」。
- **界面（`SettingsPanel.tsx`，Profile 资产页）**：插件分组下方加一行窄条「声明插件」：单行输入 + 「安装并登记」按钮，成功/失败回执写在同一行的 `.sub` 里；基础标准与出厂 Profile 不渲染该行。沿用现有直角卡片、`data-hover-hint` 与 `.asset-row` 结构，不新增弹层。
- **类型 / API**：`types.ts` 增 `ProfilePluginAddReceipt`；`api.ts` 增 `profilePluginsAdd`。
- **测试**：`profile_lifecycle` 增插件规格解析与登记去重的纯函数单测；`lib.rs` 增基础标准拒绝规则保存的行为测试（沿用既有「不写盘」断言写法）；窗口脚本覆盖基础标准按钮禁用与提示文案。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

用 `screenshots-design-inspiration` 检索 `settings page dependency list add plugin missing badge install button`，返回 Additor 等设置/详情页，共性是把「缺什么」做成行内徽标、把「补什么」做成该行行尾动作。据此把插件声明做成资产分组里的行内输入 + 行尾按钮，沿用设置页既有的行式列表与直角样式，不加新弹层。


### OD-ARCH-127：Profile 可导出 / 可导入（一套定制工作台能带走、能在别处装回来）（计划者与实施者：本助手；用户授权「把 Profile 功能全部完善，中途的计划授权你制定并批准执行」；2026-10-29 已实施）

> **触发：** 用户的产品边界——「omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能」，以及本轮补充「把 Profile 功能全部完善」。109–126 之后，建 / 复制 / 切换 / 装配 / 补装 / 巡检修复 / 移除 / 恢复 / 改名都通了，剩下的缝在**迁移**：一套调好的工作台只活在这台机器的两个目录里，换机或备份只能整盘拷数据根。D-054 的复审触发点②「出现跨设备/跨机器迁移 Profile 的需求」已经出现。

#### 已核实的前提（本人实读源码，非推测）

1. **一个自建 Profile 的资产散在两处，共五类。** DSH 侧 `dsh_home/profiles/<name>/`：`package.json`、`cordis.yml`、`cordis.patch.yml`、可选 `pnpm-lock.yaml` / `pnpm-workspace.yaml`；应用侧 `app_root/profiles/<name>/`：`decl.yml`（场景声明）、`rules.json`（规则包与 Skill 绑定）、`categories.json`（对话分类）；归属信息在 `app_root/profiles/index.v1.json`。没有任何一处能单独代表「这套工作台」。
2. **没有导出 / 导入能力。** 全仓检索只有 `profile_create` / `profile_remove` / `profile_trash_list` / `profile_trash_restore` / `profile_rename`，没有面向迁移的命令，设置页「自建 Profile」每行只有「改名 / 移除」。用户换机只能拷整个数据根，连带把会话、缓存、凭证引用一起搬过去。
3. **`node_modules` 不可移植，也不该进包。** `copy_profile_files` 从来只复制配置（不复制 `node_modules`），插件由 `required_plugins` 判定、由 `ensure_plugins` 走 DSH 官方通道安装。跨架构搬二进制会得到一个「看起来装好了、实际加载不了」的 Profile。
4. **插件清单已在代码里有单一真相。** `profile_assets::required_plugins` 对出厂 Profile 用编译期清单、对自建 Profile 用登记表 `plugins`；`missing_plugins` 与 `ensure_plugins` 共用这张表。导入后只要把 `plugins` 写回登记表，「缺 N 个插件」与「自动补装」两条既有通道立刻生效，不需要新机制。
5. **`.bak-<epoch>` 与 `trash.v1.json` 是过程文件。** 前者是改名 / 规则写入的回退点，后者是回收站元数据，两者都不属于 Profile 资产，导出时必须排除，否则会把历史残留一起带到新机器。
6. **校验器都已存在。** `profile_decl::parse_decl`、`profile_rules::parse_rules`、`profile_categories::parse`、`settings::is_valid_profile_name`、`sha256::sha256_hex` 都在，导入校验不需要新写解析器。
7. **文件选择器能力已放行。** `tauri-plugin-dialog` 已在 `Cargo.toml` 与 capabilities 的 `dialog:allow-open` 中；设置页已用它选知识库目录。导出需要 `dialog:allow-save`，当前未放行，本轮补这一条。

#### 方案

**一句话：导出一个自包含的单文件包（`omnidesk.profile-bundle.v1`），导入时先在临时目录逐文件校验，全部通过才落盘；同名一律拒绝、绝不覆盖；包内不带 `node_modules`，插件照旧由「切换装配 + 缺件补装」这条既有通道安装。**

- **格式（单文件 JSON，UTF-8）**：`{schema, exported_at, omnidesk_version, profile:{name,label,created_at,origin,plugins}, files:[{path,kind,sha256,bytes,text}]}`。选 JSON 而不是 tar 的理由：内容全是 KB 级文本，明文可审阅、可手工抢救，也不需要为离线构建新增打包依赖（本仓库锁文件里没有归档 crate）。
- **导出 `profile_lifecycle::export_bundle(dsh_home, app_root, name) -> Result<ExportReceipt, String>`**：
  - 只允许已登记的自定义 Profile；出厂 Profile（`asset_profile`）与未登记目录直接拒绝——它们的资产来自编译期清单，导出没有意义；
  - 白名单收集 DSH 侧五个配置文件名与应用侧 `decl.yml` / `rules.json` / `categories.json`，逐个记 `sha256` 与字节数；
  - 排除 `node_modules`、`.bak-*`、`trash.v1.json`、`.assets.v1.json`、隐藏临时文件与任何子目录；
  - 默认落点 `<应用数据根>/exports/<name>-<yyyymmdd-HHMMSS>.omniprofile.json`，也可由调用方指定目录；写前建目录，写失败整体报错、不留半份文件。
- **导入 `profile_lifecycle::import_bundle(path, dsh_home, app_root, rename_to, label) -> Result<ImportReceipt, String>`**：
  - 校验顺序：包大小上限（2 MB）→ schema → 每个 `path` 必须是**单段文件名白名单**（拒 `../`、绝对路径、子目录、反斜杠）→ 逐个 `sha256` 与字节数 → `name` 合法 → `decl.yml` 能 `parse_decl` → `rules.json` 能 `parse_rules` → `categories.json` 能 `parse`（且 `profile` 字段按新名重写）→ `package.json` 是合法 JSON 且含 `dsh.profile`；
  - 名字冲突（DSH 目录存在 / 应用目录存在 / 已登记 / 出厂名）→ **整体拒绝**，并在回执里说明「改成什么名字可以导入」，界面提供就地改名输入；
  - 落盘顺序：先写 `<app_root>/profiles/.import-<epoch>-<pid>/` 与 `<dsh_home>/profiles/<name>/`，DSH 目录用先写临时目录再 `rename` 到位；任一步失败就把已写的清掉，不留半套；登记表最后写，写失败则把两个目录撤回并清理；
  - 成功后 `plugins` 原样进登记表，界面照旧显示「需要 N 个插件」，走既有 `ensure_plugins` / 自动补装；导入路径**绝不联网**。
- **命令（`lib.rs`）**：`profile_export`（纯读 + 写包，不 emit）、`profile_import`（成功后 emit `omni://profile-registry`，顶栏与设置页既有订阅自动重读）、`profile_export_list`（只读列出导出目录里的包）、`profile_export_open`（在文件管理器打开导出目录）。全部 `#[tauri::command(async)]`，导出与导入都不在 UI 线程做文件 IO。
- **界面（`SettingsPanel.tsx`）**：「自建 Profile」每行行尾加「导出」，包固定落 `<应用数据根>/exports/`（不弹系统保存框：导出结果可复现，也少一次系统对话框）；区块标题行右侧加「导入」用于选外部包；「迁移包」区块列出导出目录里识别得出的包，行尾「导入这套」一键装回，同名冲突时该行就地变为「改标识后导入」输入 + 「导入 / 取消」，与 126 的行内编辑同构。回执用一句中文说明「导出了哪几个文件」「导入了哪个工作台、还缺哪些插件」，不弹窗。
- **类型 / API**：`types.ts` 增 `ProfileExportReceipt` / `ProfileImportReceipt`；`api.ts` 增 `profileExport` / `profileImport`。
- **capabilities**：`default.json` 保留原样（导出不再走系统保存框，因此没有新增 save 权限；外部包的读取沿用已有 `dialog:allow-open`）。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

用 `screenshots-design-inspiration` 检索了两组关键词：`settings export import profile json dialog`（返回 Claude 的设置轮播、ChatGPT 的权限弹层与底部面板等）与 `settings page list row action buttons backup import export`（返回 Notion 的 Inline Row Actions 等）。借鉴两点：迁移动作收在对应资源的行尾、页面级导入单独一个按钮（Notion 行内动作）；导入结果只用一行文字说明，不弹窗打断（Claude 设置页窄条列表）。据此把「导出」做成自建 Profile 的行尾动作、「导入」放在区块标题行，沿用设置页既有窄条列表与直角样式，不新增弹层。

#### 验收口径

1. 单测（`profile_lifecycle` 模块内）：导出内容只含白名单文件且排除 `node_modules` / `.bak-*`；sha256 不匹配整体拒绝且磁盘零变化；`../` 与绝对路径被拒；同名拒绝且不覆盖；导入后 `decl.yml` 可解析、`categories.json` 的 `profile` 已改成新名、`plugins` 保留；导出→导入往返得到等价清单。
2. `cargo test --offline --lib` 全绿（基线 464），`--tests`、`pytest tests -q`、`npx tsc --noEmit`、`npm run build` 通过。
3. 真实窗口脚本 `preview_profile_import_export_127.py`：全新 `/tmp` 隔离根里建 Profile → 点「导出」（包落 `exports/`）→ 移除该 Profile → 在「迁移包」区块点「导入这套」导入回来 → 设置页与顶栏都能看到 → 切过去可用（场景入口生效），全 true、EXIT=0。
4. 回归：125（回收站）、126（改名）、118（自建）三个窗口脚本 EXIT=0。

#### 明确不做

- 不做云端同步、账户体系或 Profile 市场：迁移是用户自己拿文件走。
- 不自动覆盖同名 Profile：合并两套资产属于不可逆操作，需要单独设计确认流程。
- 不把 `node_modules` 打进包：跨架构二进制不可移植，插件仍由 DSH 官方通道按平台安装。
- 不打包会话、线程、缓存与凭证引用：这些不属于「工作台配置」，且带出数据根会扩大数据边界。
- 不做「清空回收站 / 永久删除」：D-054 已定，触发点未到。
- 不 commit / push（本轮未授权）。


### OD-ARCH-126：自建 Profile 的显示名可改（计划者与实施者：本助手；用户授权「把 Profile 功能全部完善」并授权自行制定并批准计划；2026-10-29 已实施）

> **触发：** 用户的产品边界——「OmniDesk 是干净和初始的环境，Profile 是定制的，切换 Profile 时要能自动写入所需的配置、skill 和插件，如有缺失插件需要提供自动安装的功能」。用户同时授权「把 Profile 功能全部完善，中途的计划授权你制定并批准执行」。109–125 之后，建 / 复制 / 切换 / 装配 / 补装 / 巡检修复 / 移除 / 恢复都通了，剩下的缝在**命名**：自建工作台的名字建好就定死了。

#### 已核实的前提（本人实读源码，非推测）

1. **没有改名入口。** profile_lifecycle.rs 只有 create / remove / list_trash / restore；全仓检索 rename 只命中文件系统的 fs::rename，没有一个面向用户的改名命令或按钮。SettingsPanel.tsx 的「自建 Profile」列表每行只有一个「移除」按钮。
2. **显示名有两份真身。** 一份在 index.v1.json 的 ManagedProfile.label（顶栏弹层与设置页列表读的是它，profile_choice.rs:107、SettingsPanel.tsx 的 p.label）；另一份在 app_root/profiles/<name>/decl.yml 的 display_name（profile_starter.rs 起手时按 label 写入）。只改登记表，声明里的旧名字会留在场景页与资产页；只改声明，顶栏和列表还是旧名。
3. **目录名与显示名是两件事。** 顶栏与列表显示的是 label，实际目录与 DSH 的 --profile 用的是 name。因此改名可以只动 label，不必搬目录、不必改 rules.json 里的 profile 字段、不影响已绑定的 Skill 与插件——这是把「改名」做成低风险操作的前提。
4. **声明是用户可改文件。** decl.yml 走 profile_decl::parse_decl 校验；用户可能手改过 display_name，也可能改坏了。改名时若声明存在且能解析，就把 display_name 替换成新值并整份重写（先备份）；解析不了就明确报错、一个字节不动，不猜着改。
5. **基础标准与出厂 Profile 不该被改名。** 出厂 Profile 的 label 来自 profile_choice::friendly_label 的编译期映射，写登记表也没用；基础标准根本没有登记项。两条都直接拒绝。

#### 方案

**一句话：改名只动「显示名」两处真身——登记表的 label 与场景声明的 display_name；目录名、Profile 名、规则绑定、Skill、插件一律不动。**

- **内核 profile_lifecycle::rename_label(app_root, name, label) -> Result<RenameReceipt, String>**：
  - 校验：名字合法、label 去空白后 1–32 字、必须已登记（未登记 = 历史遗留目录，不碰）、不得是出厂 Profile；
  - 先算出声明文件的新文本（若 decl.yml 存在）：解析 → 替换 display_name → 重新 validate；解析或校验失败**整体拒绝**，不动登记表；
  - 再写：声明先备份成 .bak-<epoch> 并用 write_atomic 写入，随后写登记表；登记表写失败时把声明还原（用备份内容写回），避免「名字改了一半」；
  - 返回 {ok, profile, previous_label, decl_updated, note}，界面据此如实回执。
- **命令 profile_rename（lib.rs）**：取 app_root，改名成功后 emit(PROFILE_REGISTRY_EVENT)——顶栏与设置页已有的订阅会自动重读，不需要第二套刷新机制。
- **界面**：设置页「自建 Profile」每行在「移除」左侧加「改名」，点开行内出现输入框与「保存 / 取消」（不弹窗、不占额外版面），沿用现有直角行与 data-hover-hint 规范。改名成功后刷新资产视图与自建列表。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

查了 Forge Business（Enterprise / Account Settings）、Gem（SaaS Dashboard）、Navigator（Workspace）三组界面：设置类列表里「改这一行」都留在本行内解决，动作按钮固定在行尾、编辑态只把这一行变成输入框。据此沿用「自建 Profile」列表的行内编辑：点「改名」把该行换成输入框 + 行尾「保存 / 取消」，与设置页既有的窄条列表同构，不新增弹层。

#### 验收口径

1. 单测：正常改名写两处；声明解析失败时整体拒绝且登记表不变；未登记 / 出厂名拒绝；label 越界拒绝；声明不存在时只改登记表并如实说明。
2. cargo test --offline --lib 全绿（基线 459，本轮新增 5）；--tests、pytest tests -q、npx tsc --noEmit、npm run build 通过。
3. 真实窗口脚本 preview_profile_rename_126.py：建 Profile → 设置页改名 → 顶栏显示新名 → 磁盘两处都是新名 → 解析校验仍通过 → 切过去可用，全 true、EXIT=0。
4. 回归：125（回收站）、118（自建）、124（冷启动自愈）三个窗口脚本 EXIT=0。

#### 明确不做

- 不改目录名、不改 --profile 标识、不改 rules.json 的 profile 字段：改名不该牵动 Skill 绑定、插件清单、线程分类与会话目录。
- 不给出厂 Profile（基础标准 / 纪检专业）改名：它们的显示名来自编译期映射，改登记表无效，改了也只会让界面与代码不一致。
- 不做 Profile 导入 / 导出（换机迁移）：那是另一条带安全边界的通道，需单独设计确认流程。
- 不做「清空回收站 / 永久删除」：D-054 已定，触发条件仍未出现。
- 不 commit / push（本轮用户未要求提交）。

### OD-ARCH-125：Profile 回收站（计划者与实施者：本助手；用户授权「把 Profile 功能全部完善」；2026-10-29 已实施并通过窗口验收）

> **触发：** 用户的产品边界——「omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能」。用户同时授权「把 Profile 功能全部完善……阶段性备份、写文档」。124 之后「切换」「冷启动」「缺件补装」三条路都通了，剩下的缝在**移除**这一侧：软件把 Profile 搬进回收目录，回执却说「需要时可以恢复」，而软件里没有任何恢复入口。

#### 已核实的前提（本人实读源码，非推测）

1. **回执承诺了不存在的能力。** `profile_remove` 的 `ProfileRemoveReceipt.note` 原文是「已移入回收目录 `…/.trash`（不是永久删除），需要时可以从 `.trash` 恢复」。`profile_lifecycle::remove` 确实只做两次 `rename`，但应用里没有一个命令、一个按钮能把它搬回来——用户只能自己进文件系统改名，还得自己补回 `index.v1.json` 的登记项。
2. **目录名不可逆。** 移除时目录被改名成 `<name>-<epoch>`，`name` 与 `label` 就丢了；就算手工搬回，显示名、建立时间、来源 Profile、插件清单都得靠人记。这使「恢复」在实践中等于「重建一个同名的」。
3. **顶栏清单只在挂载时读一次。** `ProfileBadge` 的 `load` 只在 `useEffect(load, [load])` 里跑；设置页里移除或恢复 Profile 后，用户回到顶栏看到的还是旧清单，点不到刚恢复的那个。
4. **回收目录已经存在且是刻意设计。** `remove` 早就把两个目录（`<应用数据根>/profiles/.trash`、`<dsh_home>/profiles/.trash`）搬好，本轮不新增目录布局，只补读写与入口。
5. **118 已记录的 WebKit 行为。** 行内标题文字（`asset-row-title`）与徽标不进 AT-SPI 无障碍树，窗口脚本只能按行尾按钮判定行是否可见。

#### 方案

**一句话：把「移除」从单向的搬走，补成软件内可逆的回收站——元数据随目录一起搬，恢复是纯本地的两次 rename 加一次登记写回。**

- **元数据（`trash.v1.json`）**：`remove` 在两个目录都搬完后，往 `<id>/` 写一份 `omnidesk.profile-trash.v1`：`name` / `label` / `created_at` / `origin` / `plugins` / `trashed_at`。写失败**不阻断移除**（回收站仍可用，只是退回按目录名解析）。回收目录 id 统一为 `<name>-<epoch>-<pid>`，避免同名 Profile 反复移除时撞车。
- **列举 `list_trash`（只读）**：合并两个 `.trash`，按 `trashed_at` 倒序；没有元数据时按目录名**最后两段**反解（`name_from_trash_id`，两段必须纯数字），反解不出来就整条跳过——宁可少列一条，也不猜错名字。
- **恢复 `restore`（纯本地）**：两次 `rename` 搬回原位 + 登记写回。三道拒绝：出厂名（`omnidesk-base` / `discipline` 等由清单决定）、已登记同名、目标目录已存在。中途失败把已搬的那个目录搬回去，不留半套。成功后删掉 Profile 目录里的 `trash.v1.json`，元数据不进正式 Profile。
- **事件刷新**：新增 `omni://profile-registry`，`profile_create` / `profile_remove` / `profile_trash_restore` 成功后各发一次；`TopNav` 订阅后重读清单。这是本轮发现的**真实缺陷**：恢复后登记已写回、磁盘也对，但顶栏清单不刷新，用户点不到，等于恢复了也用不上。
- **界面**：设置页「Profile 资产」的自建列表下方加「回收站」区块（直角卡片、状态徽标、行尾「恢复」、悬停说明）。移除按钮的提示改为「移入回收站，可在下方一键恢复」，回执不再拼路径。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

回收站沿用设置页既有的窄条列表（标题 + 副行 + 行尾动作），与自建 Profile 列表同构；只在「可恢复 / 无元数据」上用徽标区分，不新增弹窗或第二套视觉。

#### 验收口径

1. `cargo test --offline --lib`：新增 5 个单测（元数据列举、恢复搬回目录与登记、同名拒绝、无元数据兜底、脏目录名拒绝），总数从 454 到 459。
2. 真实窗口 `preview_profile_trash_125.py`：建 → 移除 → 回收站可见 → 恢复 → 切过去，24 项 checks 全 true、EXIT=0。
3. 回归：`preview_profile_custom_118.py`、`preview_profile_autofill_121.py`、`preview_profile_decl_hot_123.py`、`preview_profile_startup_heal_124.py` 四个脚本 EXIT=0。

#### 明确不做

- 不做「永久删除 / 清空回收站」按钮：用户没提，且这类不可逆动作需要单独设计确认流程。
- 不做「恢复时覆盖同名目录」：宁可拒绝并让用户看见，也不静默合并两个 Profile 的资产。
- 不动数据目录（成果根 / 库根 / 知识库根 / 历史会话）——D-052 已定它们不随 Profile 走。
- 不 commit / push（用户未授权本轮提交）。


### OD-ARCH-124：冷启动插件自愈（计划者与实施者：本助手；用户授权自行制定并批准；2026-10-29 已实施）

> **触发：** 用户的产品边界——「omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能」。121 已把插件自愈接在**切换**那一刻，本轮补上另一半：选好 Profile 之后**冷启动**，缺的插件要自己回来。

#### 已核实的前提（本人实读源码，非推测）

1. **启动装配不含插件。** `assemble_profile_assets_on_startup`（lib.rs:4593）只装配配置清单、cordis 补丁、声明、规则与 Skill；全文件没有一处调用 `ensure_plugins`。
2. **自动补装只挂在切换命令上。** `spawn_profile_plugins_autofill` 的唯一调用点在 `profile_choice_set`（lib.rs 约 1333）。用户选完 Profile 关掉应用、第二天再打开，走的是启动路径，缺件不会被发现，更不会被装上；界面照旧显示「正在使用专业 Profile」，能力却是残的，只能靠用户自己进设置页手点「安装」。
3. **探测与安装的分工已经清楚。** `PluginSpec::probe` 只读本地 `node_modules`（多候选目录），不联网、不写盘；`ensure_plugins` 才走 `dsh plugin --profile <p> add`。因此可以在启动早期安全地「先探测」。
4. **总开关已存在且默认开。** `settings::profile_auto_install_plugins`（settings.rs:217，默认 true），设置页「自动补齐缺失插件」一项门控。
5. **基础标准的兜底是硬编码的。** `required_plugins` 与 `ensure_plugins` 对 `profile == standard` 都直接返回空，「底座不装插件」不依赖界面文案。

#### 方案

**一句话：把切换时用的同一套「探测 → 判定 → 后台补装 → 事件回报」接到启动路径，判定条件与切换完全同源。**

- `profile_assets::missing_plugins(scope, standard, dsh_cli) -> Vec<PluginProbe>`：只读探测，和 `required_plugins` 用同一张清单，标准 Profile 恒返回空。单独成函数是为了让「启动要不要装」这件事可以单测，而不必起窗口。
- `maybe_autofill_plugins_on_startup(app)`：在 `.setup()` 里调用。锁内只克隆路径与引擎配置随即释放；四道短路——非 `web-api` 通道、基础标准、开关关闭、探测无缺件——都不起任何任务、不写盘、不联网。真缺件时写一行 `logs/profile-assets.log`（`profile=<name> startup-autofill pending=<n>`），再复用 `spawn_profile_plugins_autofill` 起后台线程，装完照旧 `emit(PROFILE_PLUGINS_EVENT)`。
- `TopNav` 的自动补装事件回调补上「从空状态建卡」：冷启动自愈发生在用户还没开过 Profile 弹层的时候，原实现里 `prev` 为 null 直接返回，用户只会看到一句「还有 N 个未就绪」却看不到缺哪几个。
- 设置页开关标题与说明同步改为「切换或启动 Profile 时自动补齐缺失插件」，与真实行为对齐；121 的窗口脚本改用后缀包含匹配，两种标题都能命中。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

后台自愈属于「无感修复」，界面只在修不好时才说话：装齐了给一句回执，装不齐才摆出缺件卡与手动入口。
沿用项目既有直角卡片、状态徽标与 `data-hover-hint` 说明，不新增视觉语言。

#### 验收口径

- 单测：标准 Profile 恒返回空；登记了插件但本地没有时返回 1 项 `missing` 且**不新建任何目录**（`node_modules` / `profiles` / 应用数据根条目数不变）；包落地后立刻恢复为空。
- `cargo test --offline --lib` 全绿（基线 451，本轮 +3）；`--tests` 12 个 binary 全 ok；`pytest tests -q` 89 passed；`npx tsc --noEmit`、`npm run build` 通过。
- 真实窗口 `preview_profile_startup_heal_124.py` 全 true：首启自愈、删包后重启自愈、开关关闭不装、基础标准不装且不建目录。
- 回归 `preview_profile_autofill_121.py`（切换路径）与 `preview_profile_decl_hot_123.py` 全 true。
- 全程只写 `/tmp` 隔离根，不碰 `~/OmniDesk`、不读真实工作记录 / 知识库 / 凭证；`runtime/` 零差异；不 commit / push。

#### 明确不做

- 不让「装配」这一步联网：配置 / 声明 / 规则 / Skill 的装配仍然纯本地，联网只发生在用户明确知道的那一次插件安装里。
- 不给基础标准装任何插件，也不因启动而给它建 `profiles/omnidesk-base/` 目录。
- 不在启动路径上阻塞窗口：窗口先开，安装走后台，结果用事件回报。
- 不新增第二个开关：自动补装仍只有设置页那一处门控，切换与启动共用。

### OD-ARCH-123：切换即工作台的两处收尾（计划者与实施者：本助手；用户授权自行制定并批准；2026-10-29 已实施并通过审查，见 STATUS.md / REVIEW.md / D-051）

> **触发：** 用户的产品边界原话——「我想 omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能」。109 / 121 / 122 已把配置、声明、规则、Skill 的装配与缺失插件的后台补装做完，本轮收掉最后两处「切过去还不是那个工作台」的缝。

#### 已核实的前提（本人实读源码，非推测）

1. **装配链已闭环。** `profile_choice_set`（lib.rs:1108）先 `materialize` 再停旧引擎；出厂 Profile 写 package.json / cordis.patch.yml / decl.yml / rules.json / 8 个 Skill，自建 Profile 走 `materialize_custom` 按 `rules.json` 的 skill_bindings 补 Skill，切回基础标准由 `reconcile_managed_skills` 收回；缺失插件在切换后由 `spawn_profile_plugins_autofill` 后台补齐。这三段分别有 109 / 118 / 121 / 122 的窗口实跑证据。
2. **缺口 D：保存场景声明必须重启应用才生效。** `profile_decl_save`（lib.rs:1000）回执写死「声明已保存；重启应用后生效。」；`App.tsx` 的 `loadDecl` 只在挂载与切换 Profile 时调用（App.tsx:92、235），保存动作不触发任何重读；5 秒指纹轮询（App.tsx:113）只把 `declDrift` 置成 `modified`，文案同样写「重启应用后生效」，而且 `App.tsx:301` 的漂移提示在用户保存后立刻亮起，等于告诉用户「你刚保存的东西没生效」。声明只决定 Dock 入口与显示名，不参与引擎启动参数（`engine_supervisor` 只读 profile 名与插件清单），热重载是安全操作。
3. **缺口 E：基础标准下仍能保存场景声明。** `profile_rules_restore_default`（lib.rs:1795）对标准有短路拒绝，`profile_decl_save` 没有：它会 `create_dir_all` 并把 decl.yml 写进 `<应用数据根>/profiles/omnidesk-base/`。一旦写进去，基础标准就不再是「干净底座」，与用户「omnidesk 是干净和初始的环境」的边界冲突。
4. **取模板与保存是两件事。** `profile_decl_starter` 只返回文本不写盘（lib.rs:924），标准下也允许（它只是文字）。本轮只拦保存，不拦取模板。
5. **既有脚本依赖旧语义。** `verify_decl_restart_hint.py` 逐轮断言「改文件后必须重启才变」「删除文件后重启才消失」，其中「运行中改文件不热重载」是它要验证的事实本身；本轮改的是「用户在设置页保存」这条路径，不能顺手把「外部手改文件」也做成热重载，否则该脚本与 D-052 的边界一起失效。

#### 方案

**一句话：在设置页保存声明后 Dock 立刻换成新入口，外部手改文件仍然只提示不热重载；基础标准拒绝保存声明，保持干净底座。**

**A. 保存即生效（热重载只走应用内保存这一条路）。**

- 新增事件常量 `PROFILE_DECL_EVENT = "omni://profile-decl"`，与 `PROFILE_PLUGINS_EVENT` 同级放在 lib.rs 顶部。
- `profile_decl_save` 成功后 `app.emit(PROFILE_DECL_EVENT, {profile, path, digest, saved_at})`；回执 `note` 改为「声明已保存，工作台入口已更新。」。函数签名加 `app: tauri::AppHandle` 参数（Tauri 命令注入既有先例：`profile_choice_set`）。
- `App.tsx` 订阅该事件：收到后调 `loadDecl()`、清掉 `declDrift`（用户自己的保存不该触发「磁盘被改」提示）、按新声明校准当前场景（既有 effect 已经处理）。
- 外部手改文件路径不变：仍由 5 秒指纹轮询报 `modified` / `missing`，仍不热重载。文案从「重启应用后生效」改为「保存后立即生效；外部改动的文件请重新在设置页保存或切换 Profile」。这条区分要写进 `docs/decisions.md`，避免以后有人把轮询也改成热重载。
- `SettingsPanel.tsx` 的声明卡提示与 `profile_decl_starter` 的 `note` 一并改口径。

**B. 基础标准拒绝保存声明。**

- `profile_decl_save` 开头加标准短路（复用 `profile_asset_scope`，与 `profile_rules_restore_default` 同一判据）：返回「基础标准是干净底座，不挂业务场景。请先在顶栏切到 Profile 再保存声明。」
- 标准判据的副产物：`profile_decl_starter` 回执已带 `standard: true/false`，前端据此禁用「校验并保存」按钮并把原因写在按钮下面的 `hint` 里（不新造弹窗）；「填入起步模板」继续可用。
- 单测：标准下保存被拒且**不创建目录、不落文件**；非标准下保存成功并返回 digest。

**C. 验收补课：在真实窗口里跑通「取模板 → 保存 → Dock 立刻出现场景」。**

- 122 只验到「按钮可点、提示出现」，保存链路（含新的事件热重载）从未在窗口里跑过。本轮新增 `preview_profile_decl_hot_123.py`，在隔离根里：切到自建 Profile → 设置页填模板 → 校验并保存 → **不重启**断言 Dock 出现五个场景 → 切回基础标准 → 断言保存按钮禁用、Dock 只剩固有入口。
- 既有 `verify_decl_restart_hint.py` 保留原断言（外部手改仍需重启），只按新文案更新证据里的说明文字。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

参考 Claude / Acuity 一类设置面板：破坏性或受限操作就地给原因并禁用按钮，不用弹窗追赶用户。
沿用本项目既有直角卡片、状态徽标与 `data-hover-hint` 悬停说明，不新增视觉语言。

#### 验收口径

- 单测：标准 Profile 下 `profile_decl_save` 返回错误，且 `profiles/omnidesk-base/` 未被创建；非标准下写入成功、回执含 digest 与 backup 语义不变。
- 单测：声明解析 / 校验失败仍然一字节不写（既有断言不回退）。
- `cargo test --offline --lib` 全绿（基线 448，只可增）；`cargo test --offline --tests` 12 个 binary 全 ok；`python3 -m pytest tests -q` 全绿；`npx tsc --noEmit`、`npm run build` 通过。
- 真实窗口：新脚本 `preview_profile_decl_hot_123.py` 全 true（保存后不重启 Dock 即更新、标准下保存被禁）；`preview_profile_starter_122.py` 与 `verify_decl_restart_hint.py` 回归通过。
- 全程只写 `/tmp` 隔离根，不碰 `~/OmniDesk`、不读真实工作记录 / 知识库 / 凭证；`runtime/` 零差异；不 commit / push。

#### 明确不做

- 不把「外部手改声明文件」也做成热重载：那是 052 定下的可复核边界，热重载会让「磁盘上是什么」与「界面是什么」脱钩。
- 不在基础标准下写任何 Profile 资产（含空 decl.yml）。
- 不改引擎启动参数、不改 Profile 清单、不改插件安装通道。
- 不引入 Profile 市场、模板库或自动安装 Skill 的新通道。
- 不让成果根 / 库根 / 知识库根 / 历史会话随 Profile 走（收尾自查结论，见 D-052）：这几项是用户数据，不是可再生的装配资产。

### OD-ARCH-122：自建 Profile 的「体检」与「起手骨架」（计划者与实施者：本助手；用户授权自行制定并批准；2026-10-29 已实施并通过审查，见 STATUS.md / REVIEW.md）

> **触发：** 用户把本轮目标定为「OmniDesk 是干净的初始环境，Profile 是定制的；切换 Profile 时要能自动写入所需配置、Skill 和插件，缺失插件要能自动安装」，并要求把 Profile 功能全部完善到「切过去就是一个专属工作台」。

#### 已核实的前提（本人实读代码，非推测）

1. **主干已闭环。** 切换路径 `profile_choice_set`（lib.rs:1078）先 `materialize` 再停旧引擎；出厂 Profile 写配置 / 补丁 / 声明 / 规则 / 8 个 Skill，自建 Profile 走 `materialize_custom`（profile_assets.rs:1077）按 `rules.json` 的 `skill_bindings` 补 Skill；切走时 `reconcile_managed_skills` 把不属于目标的 Skill 移入停放区；插件在 121 已改为切换后台自动联网补齐。启动路径 `assemble_profile_assets_on_startup` 走同一入口。
2. **缺口 A：自建 Profile 的只读巡检看不到 Skill。** `inspect_custom`（profile_assets.rs:1231）只探测登记表里的插件，不检查 `rules.json` 声明的 Skill 是否真的在 `<dsh_home>/skills/<name>/SKILL.md`。对照 `materialize_custom` 会补 Skill，所以「装配有、体检无」：用户删了 Skill 目录，设置页看不出来，只有下一次切换才被悄悄补回。用户 107 明确说过「设置页看 skill 还是和纪检 profile 一样，我不知道哪些加载了、哪些没加载」。
3. **缺口 B：从基础标准起手的新 Profile 没有工作台骨架。** `write_base_files(None)`（profile_lifecycle.rs:165）只写 `package.json` / `cordis.patch.yml` / `cordis.yml`，不写 `decl.yml` 与 `rules.json`。结果新 Profile 切过去只有平台固有入口（首页 / 历史会话），业务场景为空，得手工填一整份声明才有工作台——与本轮「切过去就是专属工作台」的目标不符。

#### 方案

**一句话：让自建 Profile 也说得清「哪些 Skill 加载了、缺了」，并从基础标准起手时就自带一份可改的最小工作台骨架。**

- **A. 巡检补 Skill。** `inspect_custom` 复用 `wanted_skills`（与装配同一张清单）逐项调用现成的 `expected_skill_files` + `inspect_skill`，产出 `skill:<name>` 资产项，状态词沿用 `ready / drift / missing / missing_source / stale`。只读：不建目录、不写状态文件、不补文件。找不到正文来源时给 `missing_source` 并在 `notes` 里说明。
- **B. 起手骨架。** `write_base_files(None)` 之外，`create()` 在应用数据根 `profiles/<name>/` 写两份起步文件：`decl.yml`（schema + display_name=用户填的显示名 + 五个标准场景入口：工作台首页 / 工作记录 / 文案写作 / 项目跟踪 / 历史会话）与 `rules.json`（schema v2、目标 profile 名、空 global、空 scenes、空 skill_bindings —— **零预注入、零行业规则**）。写前用现有 `parse_decl` / `parse_rules` 自校验，写失败不阻断创建但要在回执里如实说。复制路径（`copy_from=Some`）行为不变。
- **C. 设置页可自助起手。** 新增只读命令 `profile_decl_starter(label?)` 复用同一份模板生成函数；场景声明页在没有声明时给一个「填入起步模板」按钮，把模板灌进编辑框（仍需用户点保存、仍走校验与备份）。已有声明不动。
- **D. 基础标准仍然零写入。** 模板只在「新建自定义 Profile」与「用户手动点填入」两条路径上产出，`omnidesk-base` 不写任何文件。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

参考 MixPanel / Linear 的设置面板：依赖项就地显示逐项状态与单一主操作，空状态给一句可执行的下一步而不是空白。
沿用本项目既有直角卡片、状态徽标与 `data-hover-hint` 悬停说明，不引入新视觉语言。

#### 验收口径

- 单测：自建 Profile 在 `rules.json` 声明 Skill 后，`inspect` 返回对应 `skill:<name>` 项；Skill 目录缺失时状态为 `missing`，补齐后为 `ready`；巡检不得创建目录、不得写状态文件。
- 单测：从基础标准新建后，应用数据根出现 `decl.yml` 与 `rules.json`，且两者都能被 `parse_decl` / `parse_rules` 通过校验；`rules.json` 的 `skill_bindings` 为空；生成的 decl 五个场景 id 均在白名单内。
- 单测：`profile_decl_starter` 产出的文本能通过 `parse_decl` + `validate`，显示名换行被压成单行。
- `cargo test --offline --lib` 全绿（基线 443，只可增不可减）；`cargo test --offline --tests` 集成全绿；`python3 -m pytest tests -q` 全绿；`verify_profile_assets_109.py` 全过；`npx tsc --noEmit`、`npm run build` 通过。
- 真实窗口验收：隔离根里新建一个从基础标准起手的 Profile → 切过去 → Dock 出现五个场景 → 设置页「Profile 资产」列出该 Profile 声明的 Skill 及状态。
- 全程只写 `/tmp` 隔离根，不碰 `~/OmniDesk`、不读真实工作记录 / 知识库 / 凭证；`runtime/` 必须零差异；不 commit / push。

#### 明确不做

- 不在切换路径上自动往自建 Profile 补写 `decl.yml`（会覆盖用户「故意不要场景」的选择）；只在「新建」与「用户点填入」处产出。
- 不给自建 Profile 预置任何行业规则、红线词或 Skill 绑定：默认零预注入，省 token、快回复。
- 不改出厂 `discipline` 的资产内容与装配顺序。
- 不自动升级、不改写用户已安装插件，不动 `runtime/`。

### OD-ARCH-121：切 Profile 自动补齐缺失插件（计划者与实施者：本助手；已实施，2026-10-29）

> **触发：** 用户明确产品边界——「OmniDesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能」。

#### 已核实的前提（本人实读代码 + 既有实跑证据，非推测）

1. **配置 / 场景 / 规则 / Skill 已经是自动的。** `profile_choice_set`（lib.rs:1078）在写 `engine_profile` 之前调 `profile_assets::materialize`，出厂 Profile 写 `package.json` / `cordis.patch.yml` / `decl.yml` / `rules.json` / 8 个 Skill；自定义 Profile 走 `materialize_custom`，按 `rules.json` 的 skill 绑定补 Skill。切回基础标准走 `reconcile_managed_skills` 收回，底座保持干净（109 / 115 / 116 / 117 已验收证据）。启动路径 `assemble_profile_assets_on_startup` 走同一入口。
2. **只有插件这一段不自动。** `lib.rs:1214` 把未 ready 的插件收进 `plugins_missing` 回执，前端 `TopNav.tsx` 弹「安装缺失插件」按钮；`ensure_plugins` 的注释与 docs/decisions.md D-043 都写明「只由用户点击触发，装配与切换路径不联网」。这正是用户这次要补的一环。
3. **安装通道本身已能真装成功（OD-ARCH-120 实跑）。** `install_version` 钉运行时版本、`relax_dep_builds` 放行 pnpm `strictDepBuilds` 后自动重试一次；联网端到端测试在隔离根里装成 `@deepseek-ai/dsh-skill-office`，重跑不再重装。
4. **当前安装调用没有超时。** `install_plugin` 用 `cmd.output()` 阻塞等待，无 deadline、无 kill。切换是同步命令，直接在切换路径里装会把「断网 / registry 慢」变成「切换一直转圈」——这正是 D-043 当初不自动的理由，必须一并解决才能自动。
5. **主进程内已有可复用的超时范式。** `model_routes.rs:122` 与 `route_classifier.rs:104` 都是「`try_wait` 轮询 + deadline + `kill` + `wait`」，本计划沿用同一写法，不引入新依赖。

#### 方案

**一句话：切换先本地落盘并立即生效，插件放在切换之后的后台任务里自动补装，装完用右下角状态条报告。**

- **A. 安装器加超时与可中断。** `profile_assets.rs` 新增 `run_install_command(...)`：`try_wait` 轮询 + 单包 deadline（初定 180 秒，常量集中一处）+ 超时 `kill` + `wait` 回收，stdout/stderr 由两个线程读完，避免管道写满卡死。`install_plugin` 改走它，回执新增「是否超时」的说明；超时按失败返回，不假装成功。既有的 pnpm 放行重试逻辑保留（重试也各自计时）。
- **B. 切换后触发自动补齐（不阻塞切换）。** `profile_choice_set` 在**已经写完设置、停完旧引擎、算出 `plugins_missing` 之后**，若缺项非空且开关打开，用 `tauri::async_runtime::spawn_blocking` 起后台任务：调用现有 `ensure_plugins`（不新写安装逻辑），完成后重新探测并把结果推给界面。切换响应体新增 `plugins_auto_installing: true/false`，让前端知道后台在跑。
- **C. 结果回传。** 新增事件 `omni://profile-plugins`，payload 为「profile / attempted / ok / still_missing / 每包退出码」。前端 `ProfileBadge` 订阅该事件：全部就绪就清掉缺件卡并弹一次成功提示；仍有缺项就把缺件卡刷新成最新清单，并说明哪几项失败。提示复用 `StatusToast`（右下角矮平长条、60 秒自动关、可手动关）。
- **D. 一个显式开关。** 设置文件加 `profile_auto_install_plugins`（默认 `true`，与用户本次意图一致；`deny_unknown_fields` 下旧文件缺字段走 serde 默认，不需要迁移脚本）。设置页「Profile 资产」分组里放一行开关，文案说明「切换 Profile 时自动联网补齐缺失插件；关掉后只提示，由你点按钮安装」。关闭时行为与现在 114 完全一致。
- **E. 界面口径。** 设置页缺件区改为：主按钮仍是「一键安装缺失插件（N）」，旁边一行小字说明自动补装是否开启、上次自动补装的结果；自动补装期间按钮显示进行中。沿用现有直角卡片与 `data-hover-hint` 悬停说明体系，不新增圆角气泡。
- **F. 基础标准恒不触发。** `required_plugins` 对 standard 已返回空表，自动补装在标准 Profile 上是纯空操作；补装任务入口再加一道 `profile == standard` 的短路，避免以后改动误触。

#### 设计参考（OD-ARCH-035 要求，2 句以内）

参考 MixPanel / Linear 一类设置面板：依赖项旁就地显示逐项状态与单一主操作，进度用细条加计数表达，不用模态打断。
沿用本项目既有直角卡片 + 右下角 `StatusToast` 的高度与配色，不引入外来视觉语言。

#### 验收口径

- 单测：`run_install_command` 的超时分支能在人为慢命令上按时 `kill` 并返回失败（用 `sh -c 'sleep 30'` 之类可控命令，deadline 设 1 秒）；正常命令 stdout/stderr 完整回收；`profile_auto_install_plugins` 缺字段默认为 `true`、写读往返稳定。
- 单测：切换路径在「缺项非空 + 开关开」时置 `plugins_auto_installing=true`，开关关或缺项为空时为 `false`；基础标准恒为 `false`。
- `cargo test --offline --lib` 全绿（当前基线 435，允许增加但不得减少）；`cargo test --offline --tests` 集成全绿（当前 2/1/13/6/5/10/21/4/3/1）。
- `OMNIDESK_NET_TESTS=1` 联网端到端：在 `/tmp` 隔离根里让一个缺插件的自定义 Profile 走「切换 → 自动补装 → 就绪」，断言第二次切换不再触发安装。
- `python3 -m pytest tests -q` 全绿；`verify_profile_assets_109.py` 41/41；真实窗口脚本覆盖「开关开 / 开关关」两种缺件提示，全 true。
- 全程只写 `/tmp` 隔离根，不碰 `~/OmniDesk`、不读真实工作记录 / 知识库 / 凭证；`runtime/` 必须零差异。

#### 明确不做

- 不把安装塞进切换的同步路径：切换的响应时间不受网络影响，这是本轮硬要求。
- 不改 `runtime/` 里的 DSH 与插件管理器，不写逐包 `allowBuilds` 白名单（沿用 120 的 `strictDepBuilds` 开关）。
- 不做安装重试风暴：单包一次自动重试（pnpm 放行那次）之外不再循环，失败就如实报。
- 不自动升级已就绪插件，不改用户 `package.json` 里自己写的依赖。
- 不在基础标准上装任何插件。
- 不做"停放区手工归位/删除"与旧线程 tag 迁移（既有遗留，另立计划）。

#### 实施与验收

**已由本助手亲自实施完毕**（2026-10-29），结果与实跑证据见 STATUS.md 顶部「OD-ARCH-121」、REVIEW.md 顶部审查结论，架构决策记为 docs/decisions.md 的 D-049。

实施中与原计划的两处偏差，均已实测确认：

1. **超时回收要连进程组一起收。** 原计划只写「kill + wait」。单测实测发现：只杀 sh 时孙进程（sleep 30）仍攥着 stdout/stderr 管道，收尾被拖到 30.006923168 秒——页面上就是「装完了但卡着不动」。改为 Unix 下 process_group(0) + killpg(SIGKILL)，管道读取附 2 秒排空宽限，同一单测降到毫秒级。非 Unix 分支退回 child.kill()。
2. **切换回执的位置要改。** 首轮窗口验收截图显示：回执绝对定位在徽标下方，与弹层正文互相压字，右端还被窗口边缘裁掉。改为「弹层开着时回执在弹层顶部，弹层关着时才挂在徽标下方」，并给回执与缺件卡补了 aria-label，让读屏与无障碍断言都能拿到完整文本。
### OD-ARCH-120：「缺插件点安装」必须真装得上：版本钉到运行时，并放行 pnpm 的构建脚本拦截（计划者与实施者：本助手；用户授权自行制定并批准，2026-10-29）

> **触发：** 用户要求「切换 Profile 时自动写入配置、skill 和插件，如有缺失插件要能自动安装」。109–119 证明的是「探测说得清缺什么」，但没人真的点过一次安装并装成功。本轮在 `/tmp` 隔离根里走真实 CLI，一次撞出两个硬阻断。

#### 已核实的前提（本人实跑，非推测）

1. **不钉版本必然被拒。** `dsh plugin --profile probe add @deepseek-ai/dsh-plugin-manager` 退出 1：
   `installation rejected: Plugin @deepseek-ai/dsh-plugin-manager@0.1.6-alpha.2 is incompatible with dsh 0.2.1-alpha.2 ... dsh: nothing was installed.`
   而探测出的清单多半不带版本，于是「点安装」在用户眼里永远是失败。
2. **钉版本后又撞 pnpm 11 的构建脚本拦截。** `add @deepseek-ai/dsh-skill-office@0.2.1-alpha.2` 退出 1：
   `[ERR_PNPM_IGNORED_BUILDS] Ignored build scripts: koffi@3.1.1`。DSH 的安装是非交互的，界面上没有 `pnpm approve-builds` 的入口。
3. **拦截信息写在 stdout。** 只在 stderr 里找关键字会漏判——第一次修完重跑仍退出 1，正是因为找错了流。
4. DSH 启动器的模块解析是双锚点（安装目录 + Profile 目录），因此「单独装一个包、在裸 node 里 import 失败」不能当成缺陷。

#### 方案

- `profile_assets.rs` 新增 `dsh_runtime_version`（从 CLI 入口向上找 `@deepseek-ai/dsh` 的 `package.json` 取 `version`）与 `install_version`（显式版本优先，否则钉运行时版本）。
- `install_plugin` 用 `install_version` 组出 `<pkg>@<ver>`；失败且输出里出现 `ERR_PNPM_IGNORED_BUILDS` / `Ignored build scripts` 时，把该 Profile 的 `pnpm-workspace.yaml` 写成 `strictDepBuilds: false`（保留原有键、不重复写），**自动重试一次**，并在回执里注明。
- 单测覆盖：版本读取、显式/回退/CLI 缺失、放行函数保留原有键且幂等、关键字判定。
- 新增联网端到端集成测试 `ensure_plugins_installs_over_network_when_enabled`，默认跳过，`OMNIDESK_NET_TESTS=1` 才跑；走应用真实入口 `ensure_plugins` → `install_plugin`，断言退出码 0、包装进 Profile 的 `node_modules`、第二次不再重装。

#### 验收口径

- `cargo test --offline --lib` 435 passed；`--tests` 集成全绿；
- `OMNIDESK_NET_TESTS=1` 跑通那条联网测试；
- `pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 41/41；
- 真实窗口重跑 `preview_profile_custom_118.py` 全绿；
- 全程只写 `/tmp` 隔离根，不碰 `~/OmniDesk` 与 `runtime/`。

#### 明确不做

- 不在切换 Profile 时自动联网安装：仍是用户点「安装」才触发；
- 不改 `runtime/` 里的 DSH 与插件管理器；
- 不改 DSH 的 `allowBuilds` 逐包批准表，只关 strict 开关。

#### 实施与验收（2026-10-29，本人执行）

见 STATUS.md 顶部同号条目。

### OD-ARCH-119：补丁里的子路径导出不是独立插件，探测必须折回包根（计划者与实施者：本助手；用户授权自行制定并批准，2026-10-29）

> **触发：** 118 实跑回执里，从「基础标准」新建的 `my-desk` 带着 `plugins: ["@deepseek-ai/dsh-tool-subagent-control/list-agents", "@deepseek-ai/dsh-plugin-manager/tools"]`，资产页出现「一键安装缺失插件（2）」和两颗「安装」按钮。而这两个条目是**同一包内的子路径导出**，不是能安装的 npm 包——`install_plugin` 的两段式校验会直接判为不合法。用户点下去只会看到「插件名或 Profile 名不合法」。

#### 已核实的前提（本人实读代码）

1. `profile_lifecycle.rs::detect_plugins` 把 `cordis.patch.yml` 里 `name: '@scope/...'` 整串当包名，不过滤子路径。
2. `profile_assets.rs::probe_package` 用 `node_modules/<name>/package.json` 判断就绪；`.../list-agents` 在磁盘上不存在这样的目录，永远 `missing`。
3. `install_plugin` 只接受 `@scope/pkg` 两段式；因此这是「永远报缺、永远装不上」的死角，不是偶发。
4. 两个包根 `@deepseek-ai/dsh-tool-subagent-control`、`@deepseek-ai/dsh-plugin-manager` 都随运行时提供（已实地确认 `package.json` 存在）。

#### 方案

- `profile_lifecycle.rs` 新增 `pub fn package_root(name) -> &str`：scoped 名字取前两段，非 scoped 原样返回。
- `detect_plugins` 的两处来源（`package.json` 依赖、补丁 `name:`）都过一遍 `package_root`，最后排序去重。
- `profile_assets::required_plugins` 读登记表时同样归一，旧登记表里已写死的子路径也能被纠正。
- 补两条单测：子路径折叠 + 排序去重；归一化只动 scoped 子路径。

#### 验收口径

- 单测覆盖子路径与两段式、非 scoped 的边界；
- Rust 全量与集成不回归；
- 真实窗口重跑 118：新建的 Profile 资产页不再出现「一键安装缺失插件」，`plugins` 为空；
- 不联网安装、不改 `runtime/`。

#### 明确不做
- 不自动安装任何插件（仍是用户点「安装」才触发）；
- 不改补丁文件本身：`name:` 写子路径在 DSH 侧是合法的模块入口，改文件会改掉引擎的实际加载行为。

#### 实施与验收（2026-10-29，本人执行）

- 新增 `package_root`；`detect_plugins` 两个来源都归一后排序去重；`required_plugins` 读登记表时归一。
- `cargo test --offline --lib profile_lifecycle` **10 passed**（新增 2 条）；`--lib` 全量 **432 passed**；`--tests` 集成全绿（2+1+13+6+5+10+20+4+3+1）；`pytest` 89 passed / 30 subtests；`verify_profile_assets_109.py` **41/41**。
- 真实窗口重跑 `preview_profile_custom_118.py` **26/26 true、EXIT=0**；资产页文本里「一键安装缺失插件（2）」与「安装」按钮消失，`profile_entry.plugins` 从两项子路径变为 `[]`。


### OD-ARCH-118：自定义 Profile 的「建 → 切 → 看 → 删」整条链路实跑（计划者与实施者：本助手；用户授权自行制定并批准，2026-10-29）

> **触发（用户口径）：**「我想 omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能。」
> 109–117 证明的一直是「在出厂的两三个 Profile 之间切」。用户真正要用的是**自己建一个工作台**：顶栏新建 → 立刻切过去 → 资产页看得见 → 不要了点移除、还能找回来。这条链路此前只有 Rust 单测，没有一条真实窗口证据。

#### 已核实的前提（本人实读代码）

1. `profile_lifecycle.rs::create` / `remove` 已有单测，但没有任何验收脚本从界面点进去过一次。
2. `lib.rs::profile_remove` 会拒绝对「当前 Profile」的移除，因此「建完直接删」走不通，必须先切回底座——这条业务约束只有真点一次才会暴露。
3. 共用定位函数 `open_profile_popover` 只认「标准 / 专业」两种徽标文案；自建 Profile 的徽标写的是自定义显示名，直接复用会点空。

#### 方案

单脚本 `tests/verification/desktop/preview_profile_custom_118.py`，隔离 `/tmp` 根、一次性 Xvfb、`OMNIDESK_DSH_PROFILE=""` 冷启动，按用户的实际动作走完：

1. 从 `omnidesk-base` 起手，点徽标 → 「新建自定义 Profile」→ 填 `my-desk` / `My Desk` → 创建；
2. 断言注册表有条目、`dsh-home/profiles/my-desk/` 有 `package.json` + `cordis.patch.yml`、`skills/` 为空（不凭空带业务技能）；
3. 确认切换 → 断言 `engine_profile == my-desk`、分类只剩 `plain`、Dock 不长出「工作记录」；
4. 设置 → Profile 资产 → 断言列出该自建项且有「移除」；
5. 回工作台、重开弹层切回「基础标准」→ 断言 `engine_profile` 为 `None`；
6. 再进资产页 → 点「移除」→ 断言注册表已删、`app-data/profiles/.trash/my-desk-*` 存在、`dsh-home/profiles/my-desk` 已清。

脚本内部单独实现 `open_popover_any`（按传入的任意显示名找徽标、且必须看到弹层真的展开才算成功），不改共用函数——110 那条路径只用出厂文案，动它没有收益。

#### 验收口径

- 上述 26 项全部为 true，脚本 `EXIT=0`；
- `pytest` 与 `verify_profile_assets_109.py` 不回归；
- 清理 dev server / Xvfb / `/tmp` 隔离根，不碰正式实例与 `runtime/`。

#### 明确不做
- 不测真实联网装插件（另计，见已知限制）；
- 不改产品代码：本条只补验收证据；
- 不 commit / push / reset / clean。

#### 实施与验收（2026-10-29，本人执行）

- 首跑 16 项通过、9 项失败，三项都是脚本口径问题、不是产品缺陷：①设置页里徽标文案是 `My Desk`，共用定位器点空；②「基础标准」列表项文字带描述，精确匹配点不中；③资产页行的 span 文字不进 AT-SPI 树，`custom_still_listed` 误判。
- 依次修正：本地 `open_popover_any`（先回工作台首页避免与资产页同名文字混淆）、改用已经验证过的 `pick_profile` 包含匹配、把「还在列表里」改为「行内『移除』按钮可见 + 注册表仍有条目」两处并证。
- 终跑 **26/26 true、EXIT=0**；`python3 -m pytest tests -q` **89 passed / 30 subtests**；`verify_profile_assets_109.py` **41/41**。
- 已清理 5273 dev server、Xvfb 与 `/tmp/od118-*`；正式实例（pid 107733 / 引擎 107812）未动。
- 未做：真实模型端到端、真实联网装插件、commit / push。


### OD-ARCH-117：复制 Profile 带上分类表，停放区在设置页可见（计划者与实施者：本助手；用户授权自行制定并批准，2026-10-29）

> **触发（用户口径）：**「我想 omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能。」
> 116 之后分类表已经是 Profile 资产，但**只有切换路径在用**。两处漏网：复制一个 Profile 时不带分类表（新 Profile 靠「继承当前生效表」碰运气）；被移进 profiles/.parked-skills/ 的 Skill 在界面上完全看不见——用户切回基础标准后只发现业务技能「消失了」，不知道它们去哪了、什么时候回来。

#### 已核实的前提（本人实读代码）

1. profile_lifecycle.rs 的 create 只复制 decl.yml / rules.json，全文零处提及 categories。
2. profile_categories::load 缺文件返回 None，apply_for_switch 才走「继承一次」；也就是说从 A 复制出 B 时，B 的分类取决于用户当时正好停在哪个 Profile。
3. profile_assets.rs 有 PARKED_SKILLS_DIR / restore_parked / retire_unwanted，全部私有，没有任何只读出口；停放区目录在真实数据根里当前不存在（用户尚未切走过）。

#### 方案

**S1｜复制即带分类表**：create 在复制 decl/rules 之后一并写 profiles/<name>/categories.json。取值优先级：来源正在生效时用当前生效设置（盘上是上次切走的旧快照，可能少一截）→ 来源的分类文件 → 出厂内嵌清单。用 profile_categories::save 重新序列化，把 profile 字段改成新名字，避免 parse 以「归属不符」拒绝。失败不阻断创建，与 decl/rules 同口径。

**S2｜停放区只读可见**：profile_assets::list_parked(app_root) 返回名字、路径、条目数，目录不存在返回空表且**绝不建目录**；新命令 profile_assets_parked 只读暴露；设置页「Profile 资产」页在有停放项时多一节「已停放」，说明「不属于当前 Profile，切回需要它的 Profile 会自动归位」。不提供删除按钮——停放本身就是「移走不删除」。

#### 验收口径

- 从 A 复制出 B，B 拿到 A 的分类表且 profile 字段是 B；无分类来源时创建仍成功；
- list_parked 在停放目录不存在时返回空表且磁盘不变；有内容时按名字排序；
- 停放项出现在设置页，tsc / 构建 / Rust 全量 / pytest / 109/110 脚本全绿。

#### 明确不做
- 不在停放区提供删除入口；
- 不改既有线程 tag、不改写用户文件、不碰 runtime/。

#### 实施与验收（2026-10-29，本人执行）

- profile_lifecycle.rs 的 create：复制 decl/rules 之后按「生效设置 → 来源文件 → 内嵌清单」取分类表，用 profile_categories::save 重写（归属自动改成新名字）；新增单测 create_from_profile_copies_categories（8 passed）。
- profile_assets.rs：新增 pub struct ParkedSkill 与 pub fn list_parked（只读、排序、目录不存在不建）；新增单测 list_parked_is_read_only_and_sorted。
- lib.rs：新增 tauri 命令 profile_assets_parked 并注册。
- 前端：types.ts 加 ParkedSkillView、api.ts 加 profileAssetsParked()、SettingsPanel.tsx 在 refreshAssets 里并发读停放清单并在资产页渲染「已停放」一节。
- 实跑：cargo test --offline --lib **430 passed**；cargo test --offline --tests 集成 2+1+13+6+5+10+**20**+4+3+1 全绿；npx tsc --noEmit 无输出；npm run build 通过；python3 -m pytest tests -q 89 passed / 30 subtests；verify_profile_assets_109.py **41/41**（原 38 + 新增 3 条：复制带分类、停放清单只读、停放区在 UI 可见）；preview_profile_switch_110.py --display :139 --root /tmp/od117-preview **16/16 true**，回执里 parked_skills 列出 8 个被停放的业务 Skill。
- 未做：真实模型端到端仍待用户实测；未 commit、未 push。


### OD-ARCH-116：对话分类随 Profile 走，基础标准不再残留业务分类（计划者与实施者：本助手；用户授权自行制定并批准，2026-10-29）

> **触发（用户口径）：**「我想 omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件。」
> 114 / 115 之后，配置、场景、规则、Skill、插件在切换时都跟着 Profile 走了。**对话分类没有跟。** 它躺在全局 `config/settings.v1.json` 里，切回 `omnidesk-base` 后仍是「未分类 / 工作记录 / 文案写作 / 项目跟进」——首页输入框左边的胶囊、历史会话页的归类下拉、设置页的分类卡片全都还在说业务话。干净底座在「分类」这一层漏了。

#### 已核实的前提（本人实读代码）

1. `settings.rs:63` `thread_categories` 是**全局**字段，默认四类由 `default_thread_categories()` 给死；`profiles/<name>/` 目录下没有任何分类文件。
2. `profile_assets.rs` 全文零处提及 categories——`materialize` / `reconcile_managed_skills` 都不碰它。
3. 前端消费点三处同源：`threadCategories.ts`（`useThreadCategories`）、`HomeCanvas.tsx:369`、`Workspace.tsx:30`、`ScenePage.tsx:135`。`sceneOptions()` 靠**场景声明**挡掉 weekly/writing/project 三个内置分类，自定义分类挡不掉（声明里隐藏业务场景时，自定义业务分类照样出现）。
4. `profile_choice_set` 现在做三件事：装配资产、写 `engine_profile`、停旧引擎；没有任何分类同步。

#### 方案

**S1｜分类表按 Profile 分文件存放**
新增 `<app_root>/profiles/<name>/categories.json`（schema `omnidesk.profile-categories.v1`）。出厂 Profile 的清单编译期嵌入（`samples/profiles/discipline.categories.json`），自定义 Profile 由用户在该 Profile 下编辑时落盘。

**S2｜切换时双向同步，基础标准只留「未分类」**
`profile_choice_set` 在写入 `engine_profile` 前后各半步：
- **切走前**：若离开的是非标准 Profile，把当前生效分类表**回存**到该 Profile 的 `categories.json`（用户在该 Profile 下的改名、新增分类不会被切丢）。
- **切入后**：非标准 Profile 用它的 `categories.json`（首次用出厂清单或从当前分类继承一份）；基础标准收敛为**只有「未分类」一类**。
- 现有线程的 tag 一个都不改（历史对话仍按原 tag 存着，标签显示回落为「未分类」，切回 Profile 就恢复真名）。

**S3｜内置四键的约束松动为「至少 plain」**
现在 `validate_categories` 强制要求四个内置键齐全，基础标准没法只留 `plain`。改为：`plain` 必须存在且键唯一；`weekly/writing/project` 缺失时按「该 Profile 没有这个业务分类」处理，不算错误。

#### 具体改动

1. `settings.rs`：`validate_categories` 只要求 `plain`；新增 `standard_thread_categories()`（仅 plain）与 `DEFAULT_CATEGORIES` 区分。
2. 新模块 `profile_categories.rs`：`load/save`（`<app_root>/profiles/<name>/categories.json`，原子写，损坏即报错不猜）、`embedded_for(profile)`（出厂清单，编译期嵌入）、`apply_for_switch`。
3. `samples/profiles/discipline.categories.json`：纪检专业的分类表（四类，带 weekly-report / material-writing / project-management 映射）。
4. `lib.rs::profile_choice_set`：装配段之后调用 `apply_for_switch`；返回值加 `categories` 字段；失败整次切换回滚（与现有 `engine_profile` 回滚口径一致）。
5. 前端 `App.tsx` 的 `onProfileChanged` 补 `refreshThreadCategories()`；`SettingsPanel` 的 `profileEpoch` effect 清掉分类缓存；设置页分类区说明「这些分类属于当前 Profile」。
6. `SettingsPanel.tsx:2205` 的「保存后需重启应用生效」在 110 之后已不成立，改为准确说法。

#### 验收口径

- 切到 `omnidesk-base` 后 `settings.v1.json.thread_categories` 只剩 `plain`；首页胶囊、历史会话、设置页都不再出现三个业务分类；
- 切到 `discipline` 后四类回来，归档目录映射与 062 一致；
- 在某 Profile 下改名/新增分类 → 切走 → 切回，改动仍在（`categories.json` 回存）；
- 基础标准态下切换**不创建任何 `profiles/<name>/` 文件**，既有线程 tag 不被改写；
- 新增集成测试覆盖以上四条 + 损坏 `categories.json` 被拒绝；`cargo test` / `tsc` / `npm run build` / `pytest` / 109/110 脚本全绿；`preview_profile_switch_110.py` 增加「base 态不含业务分类」断言并真实窗口跑通。

#### 明确不做
- 不迁移、不改写既有线程的 tag 与消息；
- 不删除任何分类对应的成果目录或文件；
- 不在切换时联网、不碰 `runtime/`、不碰 `~/.dsh`。

#### 实施与验收（2026-10-29，本人执行）

- `settings.rs`：新增 `REQUIRED_CATEGORY_KEY = "plain"` 与 `standard_thread_categories()`（仅 plain）；`validate_categories` 不再强制四键，只要求 `plain` 存在且唯一；`thread_categories` 的 serde 默认值与 `Settings::default()` 都改为干净底座，`default_thread_categories()` 保留为专业 Profile 的出厂清单。
- 新模块 `profile_categories.rs`：`load / save / embedded_for / apply_for_switch`，走 `model_settings::write_atomic` 原子写，schema 与归属双重校验，损坏即报错不猜；`apply_for_switch` 先回存离开的 Profile，再按「文件 → 内嵌 → 继承一次」取目标值。
- `samples/profiles/discipline.categories.json`：四类 + 归档目录映射，编译期内嵌。
- `lib.rs::profile_choice_set`：在 `set_engine_profile` 之前同步分类，失败则整次切换中止并给出「分类表同步失败，未切换」；回执新增 `categories`。
- 前端：`App.tsx` 切换成功后 `refreshThreadCategories()`；`SettingsPanel.tsx` 的 `profileEpoch` effect 清缓存与草稿；`threadCategories.ts` 默认值改为只有 plain。
- **首跑暴露的真实缺口：** 真实窗口首轮 `boot_base_categories_only_plain` 为 false——全新数据根没有设置文件，`Settings::default()` 仍给四类。只读测试全绿也掩盖不了这一点，改默认值后第二轮才转绿。
- 实跑：`cargo test --offline --lib profile_categories` 5 passed；`--test profile_categories_host` 4 passed；`cargo test --offline --tests` 428 lib + 集成 2+1+13+6+5+10+20+4+3+1 全绿；`npx tsc --noEmit` 无输出；`npm run build` 通过；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` **38/38**；`preview_profile_switch_110.py` 22/22 静态 + 真实窗口（`:137`，`/tmp/od116-preview`）**16/16 true**。
- 未做：真实模型端到端仍待用户实测；未 commit、未 push。


### OD-ARCH-114：切换 Profile 时把缺失插件摆到台面上，并给一键安装（已实施并通过验收，2026-10-29；用户授权自行制定并批准）

> **触发（用户口径）：**「我想 omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能。」
> 109–113 之后，配置与 Skill 在切换时就写好了；插件只做到「探测」，缺失项只有进设置页才看得见，切换回执里一个字都不提。

#### 已核实的前提（本人实读代码）

1. `profile_choice_set` 装配完只回 `assembled_changes`（写了几项），插件状态在 `assembly.items` 里，但前端只用它算「已装配 N 项」，用户看不到「有插件没装上」。
2. 安装能力已存在（`profile_plugin_install` → `dsh plugin --profile <p> add`），但一次只装一个；批量只在设置页的「一键安装缺失插件」里有，且要先切进设置页。
3. 安装是联网写操作，不能在装配里静默执行——必须让用户看得见「要装什么、为什么、会联网」。

#### 方案

**S1｜新命令 `profile_plugins_ensure`**
入参 Profile 名；按出厂清单（自定义 Profile 走登记表）算出缺的插件，逐个调 `install_plugin`，返回每项的 `InstallReceipt` 与安装后重新探测的状态。单个失败不中断其余，最终 `ok` 只在全部就绪时为 true。

**S2｜切换回执带缺失插件清单**
`profile_choice_set` 返回值加 `plugins_missing: [{package, status, detail}]`；基础标准恒为空。

**S3｜切换后的一次性提示**
切换成功且 `plugins_missing` 非空时，在 Profile 弹层里显示一条「还缺 N 个插件」的条，附「安装」按钮（带 hover 说明「会联网」），点完把逐项回执与最终就绪状态显示出来。用户不点就不联网。

**S4｜明确不做**
- 不在装配里自动联网安装；
- 不装清单外的包，不禁用用户自己装的插件；
- 不改 `runtime/`、不碰 `~/.dsh`。

#### 验收口径
- 切换到一个依赖插件但本地没有的 Profile，回执里列出缺的包名，界面出现安装入口；
- 点安装后逐项回执，全部成功时再探测为 `ready`；单个失败不中断其余；
- 基础标准切换始终零插件请求、零写入；
- cargo test / tsc / npm run build / pytest / 109+110 验证脚本全绿。

#### 实施与验收（2026-10-29，本人执行）

- `profile_assets.rs` 新增 `PluginSpec`（所有权类型，不用 `Box::leak`）、`required_plugins`、`EnsureReceipt`、`ensure_plugins`；`lib.rs` 新增 `profile_plugins_ensure` 命令并加入 `generate_handler!`；`profile_choice_set` 返回值加 `plugins_missing`。
- 界面：`TopNav.tsx` 在切换后若仍缺插件，弹层不自动关闭，列出缺的包并提供「安装缺失插件 / 稍后」，均带 `data-hover-hint`；`SettingsPanel.tsx` 的「一键安装缺失插件」改走批量命令，N 次 IPC 收敛为 1 次。
- 实跑：`npx tsc --noEmit` 无输出；`npm run build` 通过；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` **34/34**（新增 3 项：回执带缺失清单、批量命令已注册、装配路径不调 `ensure_plugins`）。
- 未做：真实联网安装未在正式实例上跑（会拉包），留给用户实测时确认网络与镜像。


### OD-ARCH-115：切回基础标准时把 OmniDesk 写的 Skill 收起来——底座要真的是空的（已实施并通过验收，2026-10-29；用户授权自行制定并批准）

> **触发（用户口径）：**「我想 omnidesk 是干净和初始的环境，而 profile 是定制的。」
> 现状：8 个业务 Skill 写在 `<dsh_home>/skills/`，而 DSH 的 `skill-filesystem` 把该目录当**全局用户根**扫描。于是切回基础标准后，底座仍会加载纪检的业务 Skill——「干净底座」只体现在配置层，没体现在 Skill 层。

#### 方案

**S1｜登记 + 停放**
装配时把写过的 Skill 名记到 `<dsh_home>/skills/.omnidesk-managed`。切到不需要这些 Skill 的 Profile（基础标准或不绑这些 Skill 的自定义 Profile）时，把这些目录**移动到** `<app_root>/profiles/.parked-skills/<name>`，并从登记表移除。移动不删除，用户内容仍可取回。

**S2｜切回来时先归位再装配**
装配前先把目标 Profile 需要的、仍停在 park 目录里的 Skill 移回原位，再走既有的三态写入。这样用户在 Skill 里做过的修改仍然按 `drift` 保护，不会被内嵌副本覆盖。

**S3｜零写入不破**
基础标准且没有登记项时，`retire` 是空操作：不建目录、不写文件。原有「基础标准零写入」的测试继续有效。

#### 验收口径
- 空根 → 切 discipline：8 个 Skill 在 `skills/`，登记表 8 项；
- 再切基础标准：8 个目录移入 `.parked-skills/`，`skills/` 为空，登记表为空；
- 再切回 discipline：8 个目录归位，内容与出厂一致；
- 用户在 Skill 里改过正文，停放再归位后内容仍是用户版（drift 不被覆盖）；
- cargo test 全绿，109/110 脚本通过。

#### 实施与验收（2026-10-29，本人执行）

- `profile_assets.rs` 新增 `wanted_skills` / `load_managed_skills` / `save_managed_skills` / `restore_parked` / `retire_unwanted` / `reconcile_managed_skills`，以及装配后补登记的 `record_managed_skills`。
- `lib.rs::profile_choice_set` 两条分支都收敛 Skill：切回基础标准调 `reconcile`，切到 Profile 时 `materialize` 后调 `record`；`profile_assets_repair` 修复后同样补登记。`engine_supervisor.rs::assemble_profile_assets` 启动时先 `reconcile` 再装配。
- **首跑暴露的真实缺口：** 登记原先只写在 `reconcile` 里，而 `reconcile` 跑在装配前——那时 Skill 还没落盘，登记表永远为空，收回无从谈起。三条新集成测试首跑全红，补 `record_managed_skills` 后才绿。该边界已写入 D-042。
- 实跑：`cargo test --offline --test profile_assets_host` **20 passed**（新增 3 条）；`cargo test --offline` 423 lib + 集成 2+1+13+6+5+10+20+3+1 全绿、EXIT=0。


### OD-ARCH-113：资产页就地修复 + Skill 巡检比内容——「缺了就点，改了就还原」（已实施并通过验收，2026-10-29；用户授权自行制定并批准）

> **触发（用户口径）：**「我想 omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能。」
> 109–112 把「切换即装配」做通了，但资产页仍然是**只读**的：丢了文件只能靠再切一次或重启；想还原自己改过的文件只能手工删。Skill 项还只看 SKILL.md 在不在，改了正文依然显示「就绪」。

#### 已核实的前提（本人实读代码，非推断）

1. inspect（profile_assets.rs）对 Skill 只判 dst/SKILL.md 是否 is_file() → 用户改了正文仍报 ready，是假绿；配置类（package.json / cordis.patch.yml / decl.yml / rules.json）走摘要三态，是准的。
2. 缺项文案是「切换 Profile 或重启时会补写」，界面没有就地修复入口；drift 文案是「需要出厂内容时请先备份再删除该文件」，纯手工。
3. materialize 系列函数没有「显式覆盖」通道：sync_bytes 在 drift 分支无条件返回不动。
4. 插件安装已有单条入口（profile_plugin_install），但没有「一次装完所有缺失插件」的批量入口。

#### 方案

**S1｜显式覆盖通道**
sync_bytes / sync_skill_files / materialize_references / record_file / materialize_custom 与新增 materialize_with 都接受 overwrite_drift: bool。日常路径传 false（行为不变），修复路径传 true：drift 分支先 backup_of 成 .bak-<epoch>，再按出厂内容写回，状态记 updated。

**S2｜Skill 巡检逐文件比摘要**
新增 expected_skill_files(name, repo_skills)（仓库优先、内嵌兜底，与装配共用同一来源）与 inspect_skill(...)：逐文件比 sha256，优先报 drift（用户改过）→ missing（缺文件）→ stale（有新版）→ ready。STATUS_STALE 补为公开常量。

**S3｜新命令 profile_assets_repair**
后端校验 Profile 名合法、非标准，调 materialize_with(..., true)，返回 { ok, profile, standard, changed, remaining, report, note }。changed 统计 created|updated，remaining 统计修复后仍为 missing|missing_source|drift 的项。不联网、不动插件。

**S4｜界面**
「Profile 资产」卡头加「修复缺失 / 恢复出厂内容」（基础标准下不渲染），回执写明补写几项、还剩几项。若有缺失插件，另出一条「一键安装缺失插件（N）」，逐个跑、逐项回执、失败不中断。api.ts / types.ts 同步加方法与 ProfileAssetsRepairReceipt。

**S5｜明确不做**
- 修复不装插件（插件联网，必须独立触发）；
- 修复不改 runtime/、不碰 ~/.dsh；
- 不放松基础标准零写入。

#### 验收口径
- 改过 Skill 正文后巡检报 drift（不再是 ready），且巡检本身不写盘；
- 删除的 Skill 点修复即补齐；被改过的文件修复后内容为出厂版，且同目录留 .bak-<epoch> 且备份里是用户原文；
- 基础标准点修复仍然零写入（不建目录）；
- cargo test / tsc / npm run build / pytest / 109+110 验证脚本全绿。


### OD-ARCH-112：引擎与插件探测同源——分发后「面板说有插件」必须等于「引擎真能起来」（已实施并通过验收，2026-10-29；用户授权自行制定并批准）

> **触发（用户口径）：**「我想 omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能。」
> 111 解决了「Skill 正文随二进制走」，但引擎自己的路径解析仍只认源码树。二者的差就是：**面板说插件已就绪，引擎却起不来**。

#### 已核实的前提（本人实读代码，非推断）

1. `EngineConfig::from_env`（`engine_supervisor.rs`）的引擎目录与 DSH CLI 都从 `repo_root_hint()` 推：引擎取 `<repo>/engine`，CLI 取 `<repo>/runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js`；仓库根不可用时直接返回 `Err`。
2. 资产侧（`profile_assets::default_dsh_cli`）在 111 之后已有安装位置兜底（`<app_root 的上一级>/runtime/...`、`$HOME/OmniDesk/runtime/...`）。
3. 于是 deb / AppImage 装到用户机器上会得出两条互相矛盾的结论：插件探测按兜底路径找到 CLI → 面板显示「已就绪」；引擎按仓库路径推不出 CLI → 第一个任务起不来。
4. `validate()` 只校验路径存在性与 `omnidesk_engine/` 目录，没有「换个位置再找一次」的能力。

#### 方案

**S1｜引擎目录按安装布局兜底**
新增纯函数 `locate_engine_dir(repo_root, app_root)`：依次试 `<repo>/engine`、`<app_root 上一级>/engine`（即 `~/OmniDesk/engine`）、`$HOME/OmniDesk/engine`，只返回真正含 `omnidesk_engine/` 的候选；都不中则返回第一候选，由 `validate()` 如实报「引擎包目录缺少 omnidesk_engine/」，不伪造可用。

**S2｜CLI 与插件探测共用一套候选**
`profile_assets` 抽出显式版本 `default_dsh_cli_for(repo_root, app_root)` 与纯推导 `locate_cli(repo_root, app_root, home)`（不读全局环境变量，便于单测比对）；`default_dsh_cli()` 退化为读环境后委托过去。引擎启动改用同一个 `default_dsh_cli_for`，两边不再各写一遍推导。

**S3｜测试只做纯路径推导**
新增 `distributed_layout_resolves_engine_and_cli_without_repo`：构造 `<base>/{app,engine,runtime}` 的装机布局、`repo_root` 指向不存在的目录，断言引擎目录与 CLI 都被正确定位。该用例**不读写进程级环境变量**——最初版本改 `OMNIDESK_REPO_ROOT` 造成并行测试互相污染，已改掉。

**S4｜明确不做**
- 不改 `validate()` 的判定，也不为「路径不存在」放水。
- 不内嵌 DSH 运行时与 `node_modules`（体积与许可边界）。
- 不改安装器布局；换布局时仍以 `OMNIDESK_ENGINE_DIR` / `OMNIDESK_DSH_CLI` 显式指定。

#### 验收口径
- 分发布局（仓库目录不存在）下引擎目录与 CLI 都能定位，且与插件探测得出同一个 CLI；
- 都找不到时仍是明确报错，不静默降级成「能跑」；
- `cargo test` / `tsc` / `npm run build` / `pytest` 全绿；
- 重跑 111 的真实分发预览 7/7（证明内嵌与路径兜底同时成立）。


### OD-ARCH-111：Profile 自带资产随二进制走——干净环境下也能一次切换到专属工作台（已实施并通过验收，2026-10-29；用户授权自行制定并批准）

> **触发（用户口径）：**「我想 omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能。」
> 110 之后「切换即生效」已成立，但**「自动写入」只在源码树里成立**。本条补的就是这个前提。

#### 已核实的前提（本人实读代码，非推断）

1. **Skill 只从仓库目录复制。** `materialize`（`profile_assets.rs:474`）走 `scope.repo_root.join("skills")`；`scope.repo_root` 来自 `task_shell::repo_root_hint()`（`task_shell.rs:329`），后者优先读 `OMNIDESK_REPO_ROOT`，否则用 `env!("CARGO_MANIFEST_DIR")` 反推。`CARGO_MANIFEST_DIR` 是**编译期的源码绝对路径**，deb/AppImage 装到用户机器上时该目录不存在。
2. **结果就是「干净环境下 Skill 全缺」。** `repo_skills == None` 时每个 Skill 落 `missing_source`，配置、场景、规则仍然装得上——用户能切过去，但三个场景要用的 8 个 Skill 一个都没有。
3. **写作 references 同理。** `materialize_references`（`profile_assets.rs:275`）读 `templates/discipline`、`knowledge/writing-style`、`knowledge/excellent-cases`，同样只走仓库路径。
4. **体积允许内嵌。** 8 个 Skill 共 8 个文件 / 76K；写作资产 9 个文件 / 约 32K，合计约 110K，对当前 7.5MB 二进制无实质影响。
5. **内嵌是本项目既有做法。** `profile_assets.rs` 的 `manifest` / `patch` / `decl` / `rules` 已经全部 `include_str!`。
6. **插件探测与安装就绪但依赖 CLI 路径。** `default_dsh_cli()` 同样从 `repo_root_hint()` 推 `runtime/dsh-runtime/.../bin.js`；用户机器上运行时不在该路径时，探测全报 `missing`、点「安装」也会因「找不到 DSH CLI」失败。

#### 方案

**S1｜Skill 正文内嵌**
新增 Skill 正文的编译期内嵌表，`materialize` 在仓库源不可用时改用内嵌正文写入 `<dsh_home>/skills/<name>/SKILL.md`，并照常走 `sync_bytes` 三态判定（不存在→写入、摘要一致→更新、被改过→`drift` 不覆盖）。仓库源可用时仍优先用仓库，开发期能看到改动。

**S2｜写作 references 内嵌**
`templates/discipline`、`knowledge/writing-style`、`knowledge/excellent-cases` 三处资产同样内嵌，仓库源缺失时用内嵌内容物化到 `<skill>/references/`。

**S3｜CLI 路径兜底**
`default_dsh_cli()` 在仓库根推不出可用文件时，回落到常见安装位置（`<app_root>/runtime/dsh-runtime/...`、`~/OmniDesk/runtime/dsh-runtime/...`）；仍找不到就返回空路径，由上层如实报「找不到 DSH CLI」，不伪造成功。

**S4｜来源如实标注**
`AssetItem.detail` 说明该 Skill 来自「内嵌」还是「仓库」；内嵌补齐与仓库补齐在界面上语义一致（都是已写入）。

**S5｜明确不做**
- 不内嵌 `node_modules` 与 DSH 运行时本身（体积与许可边界都不合适）。
- 不改 Profile 白名单、不改切换时序、不改干净底座零写入的结论。
- 不覆盖用户改过的文件：内嵌内容同样受 `drift` 保护。

#### 验收口径
- 新增集成测试：仓库根不可用时，discipline 仍装出 8 个 Skill 与写作 references；
- 内嵌内容与仓库源逐字节一致（防两边漂移）；
- 用户改过的 Skill 仍不覆盖；基础标准仍零写入；
- 既有 `cargo test` / `tsc` / `npm run build` / `pytest` / 109+110 验证脚本全绿。

### OD-ARCH-110：切换 Profile 立即生效——换引擎、换场景、换规则，不用重启（已批准并实施，2026-10-29；用户授权自行制定并批准）

> **触发（用户口径）：**「omnidesk 是干净和初始的环境，而 profile 是定制的」——Profile 的语义必须是「换一个工作台」，而不是「存一个名字等下次启动」。

#### 已核实的前提（本人实读代码，非推断）

1. `profile_choice_set`（lib.rs:1056）在装配与写盘之后**只**调 `settings::set_engine_profile`；不动 supervisor 里的内存配置，也不停引擎。
2. `profile_asset_scope()`（lib.rs:793）取的是**运行中引擎**的 `config().profile`；`engine_submit_task` 的规则/隐私/注入 Skill 也都读它。所以切换后场景、规则、注入技能仍按旧 Profile。
3. `App.tsx` 的 `profileDecl()` 只在挂载时读一次；`TopNav` 切换成功后只刷新自己的列表，Dock 入口与规则不变。
4. 后端 `profile_choice_view` 的 note 与 `TopNav` 的确认文案都写着「下次启动引擎时生效」，即当前实现是已知的半成品。
5. 先例：`model_settings_save`（lib.rs:1589）走「写盘 → 同步两份配置快照 → `begin_change()` → `stop_engine(10s)` → `succeed()/fail()` → `from_env` 重读」。Profile 切换缺的正是这一段。

#### 方案

**S1｜后端切换即换引擎（`profile_choice_set`）**
装配 + 校验 + 写盘通过后：
- `EngineConfig::from_env(&app_root)` 重读（此时 resolve 会选中新 Profile）；
- 把新配置同时写回 `supervisor.config_mut()` 与 `HostSnapshot.config`，保证「界面看到的 Profile」「准入判定用的 Profile」「引擎启动用的 Profile」是同一个值；
- 用 `lifecycle.begin_change()` 独占生命周期，`stop_engine(10s)` 停掉旧引擎；成功 `succeed()`，失败 `fail(reason)` 并保留屏障（失败关闭）。
- 返回新增 `engine_stopped`、`effective`（切换后实际生效名），note 改成「已切换到 X；旧引擎已停止，下一条任务按新配置启动」，失败时如实说明「旧引擎未能确认停止」。
- 需给命令加 `app: tauri::AppHandle` 参数以触达 `HostSnapshot`。

**S2｜前端切换后立即刷新**
- `ProfileBadge` 增加 `onProfileChanged` 回调；`App.tsx` 把读声明的那段抽成 `reloadDecl()` 并传下去。切换成功后：刷新徽标列表 + 让 App 重读 `profileDecl()` + 重算 Dock/场景。
- 确认弹窗与徽标提示改为「切换后旧引擎停止，下一条任务即按新 Profile 运行（正在生成的内容会被中断）」。
- `profile_choice_view` 的 note 改为「当前生效的 Profile」。

**S3｜明确不做**
- 不做「排队等旧任务跑完再切」：那会让切换看起来没反应，且排队语义需要新的任务队列，超出本轮范围；
- 不新增 `engine_restart` 命令：复用 `stop_engine` + 下一条任务惰性启动，已有成熟路径；
- 不改变 Profile 白名单、资产装配规则与干净底座结论。

#### 验收口径
- `cargo test`：新增「切换后 supervisor 与快照的 profile 一致」用例；
- `tsc --noEmit` + `npm run build`；
- `pytest` 与 109 的 verify 脚本继续全绿；
- 真实窗口：切到 discipline → Dock 立刻出现专业场景、规则生效；切回标准 → 立即回到干净底座；运行中任务被停止且界面如实说明。

### OD-ARCH-109：Profile 资产自装配——切换即写入配置、Skill、插件，并检测/安装缺失插件（已批准并实施，2026-10-29；用户授权自行制定并批准）

> **触发（用户口径）：** 「omnidesk 是干净和初始的环境，而 profile 是定制的，所以切换 profile 时要能自动写入所需的配置、skill 和插件，还有如有缺失插件需要提供自动安装的功能。」
> 分类：架构 + 功能实现。核心是把「Profile 是一份可安装的资产包」这件事落到代码里，不再靠手工同步。

#### 已核实的前提（本人实读代码/实测，非推断）

1. **切换 Profile 目前只写一个名字。** `profile_choice_set`（`lib.rs:1040`）校验 `profiles/<name>/package.json` 存在后，只调 `settings::set_engine_profile`；profile 目录缺失时直接**报错拒绝切换**，不会补建。
2. **仓库里没有任何安装路径。** `samples/profiles/discipline.profile.patch.yml` 只被测试脚本读取；`engine_supervisor::seed_profile_if_missing`（`engine_supervisor.rs:222`）只播种 `web`，且 `package.json` 存在即返回。这就是 104 记下的「出厂资产缺口」。
3. **插件包其实随运行时就有。** `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-skill-office@0.2.1-alpha.2` 是 `@deepseek-ai/dsh` 的依赖（lockfile 可证），`libreoffice-kit` 同层。插件解析先查 installAnchor、再查 profile 目录（`resolveBundleDir`），所以**随运行时携带的插件无需 pnpm 安装**，缺的只是那一行 patch。用户机器 `~/OmniDesk` 没有 `runtime/dsh-runtime`，运行时在仓库内，这一点必须写清楚。
4. **官方安装通道存在但不该默认触发。** `dsh plugin --profile <name> add <pkg>` 会把参数转发给 pnpm、在 profile 目录里装依赖（`plugin-BGnVfe_D.js:58-96`），需要 pnpm 与网络；离线/无 pnpm 时退出码非 0。
5. **Skill 目录是共享的。** `skill-filesystem` 的 roots 含 `<dshHome>/skills`（`skills.js:171`），与 Profile 无关；107 已按此定稿「都在盘上，只是预注入不同」。宿主 UI 也只读 `<dsh-home>/skills`（`capabilities.rs:413`）。
6. **默认资产已编译期内嵌。** `DEFAULT_DECL`、`DEFAULT_RULES_JSON` 都是 `include_str!`，没有走 Tauri 资源打包，说明内嵌是既有做法。

#### 方案

**S1｜资产清单与内嵌默认值（新模块 `profile_assets.rs`）**
每个非标准 Profile 由一张静态表描述：Profile 名 → { dsh 清单、cordis.patch.yml、decl.yml、rules.json、所需 Skill 名、所需插件包 }。默认内容全部 `include_str!`，随二进制走，不依赖仓库路径（Skill 正文除外，见 S3）。

**S2｜幂等装配（核心）**
`materialize(dsh_home, app_root, profile)` 逐项处理：
- 文件**不存在** → 写入默认值，记为「已补写」；
- 文件存在且摘要 == 上次我们写入的摘要（无用户改动）→ 更新为新默认值，记为「已更新」；
- 文件存在但摘要不符（用户改过）→ **一律不覆盖**，记为「已修改，未覆盖」并给出原因；
- 动手前备份为 `.bak-<epoch>`，与 `write_profile_rules` 同口径。
装配状态记在 `<app-root>/profiles/<name>/.assets.v1.json`（含每项摘要与新写入值），这样「是用户改的」还是「是旧的出厂版」可判定，而不是靠猜。

**S3｜Skill 跟随 Profile**
按 S1 表把该 Profile 需要的 Skill 目录从 `<repo>/skills` 复制到 `<dsh-home>/skills`（缺则补，已有则不覆盖，只比对不删除）。**不改共享目录模型、不加 customSkillDirs**——理由见「明确不做」。基础标准完全不执行本步。

**S4｜插件就绪探测 + 一键安装**
`probe_plugins` 只做本地解析：先查 installAnchor（`runtime/dsh-runtime/node_modules`）、再查 profile 目录，返回 { 名称、期望版本、实际版本、状态 }。三态：`ready` / `missing` / `version_mismatch`。
`install_plugin` 仅在 `missing` 时可用，执行 `dsh plugin --profile <name> add <pkg>@<version>`，**必须由用户点击触发**（网络写操作），回传真实 stdout/stderr 与退出码，失败原样展示、不吞错。随运行时自带的包永远是 `ready`，不会被拉去联网。

**S5｜两条执行时机**
- **切换时**：`profile_choice_set` 改为「先装配、再校验、后保存」，不再因为目录缺失直接拒绝；装配失败带原因返回，不静默降级。
- **启动时**：`supervisor.start()` 在 `validate` 之后跑同一函数（自愈：文件被删会补回；用户改过则不动）。基础标准直接跳过，保持 106 的「干净底座」事实。

**S6｜设置页新增「Profile 资产」面板**
按软件现有方角卡片与悬停说明口径呈现：每项一行（配置 / 场景声明 / 规则包 / Skill / 插件），状态用既有配色语义（就绪、已补写、已修改未覆盖、缺失、版本不符），缺失插件旁给「安装」按钮，结果用右下角矮条提示（沿用 096）。基础标准下显示一句「基础标准不安装任何 Profile 资产」。

**S7｜验证**
新增 `verify_profile_assets_109.py`（静态 + 隔离根实跑）：空 dsh_home 切换到 discipline 后五项齐备；二次运行零改写；用户改过的文件不被覆盖且状态为 drift；删文件后启动自愈；omnidesk-base 全程零写入（目录都不创建）；插件探测在「随运行时携带」与「人为移走」两种情形下分别给出 ready / missing。再补一次真实窗口像素/AX 检查面板状态。`cargo test` 覆盖装配三态与备份。

#### 明确不做

- **不做 Profile 级 Skill 目录隔离。** skill-filesystem 在 web app 里由 `preset-standard` 拥有，而补丁对 `config` 是整体替换（104 已证），要隔离就得每个 Profile 复制整份 30 余项 preset 配置，升级即漂移。保持 107 的共享目录 + 预注入绑定模型。
- 不改 `runtime/`，不内置插件市场，不做插件版本自动升级。
- 不在启动时静默联网装包；不覆盖用户改过的任何文件；不删用户文件。
- 不动基础标准的任何资产（106 的边界）。

#### 风险与已知限制

1. Skill 复制依赖 `<repo>/skills` 存在；打包分发（deb/appimage）后仓库不在原位时需要另做内嵌，本轮沿用「引擎也依赖 repo_root」的既有前提，不扩大。
2. 摘要判定只能识别「我们写过、之后没被人动过」。若用户在别的机器上手工放了同样内容，摘要相同也会被视作出厂版而更新——这种情况下内容本就一致，影响可接受，会在文档写明。
3. 插件安装依赖 pnpm 与网络，失败路径只能如实报错，不做重试编排。

#### 原定验收

空目录上切到 discipline，配置/场景/规则/Skill/插件五项自动就位且可启动；重复切换无副作用；用户改动不被覆盖；基础标准零写入；缺失插件可一键安装并展示真实结果；tsc / build / cargo test / pytest 全绿；证据落 `evidence/od-arch-109/`。

#### 实施结论（2026-10-29，本助手亲自实施）

计划已按 S1–S7 落地，另有四处按实况修正，均已在代码与文档中体现：

1. **装配时机从「引擎启动」前移到「应用启动」。** 计划的 S5 写「启动时跑 `supervisor.start()`」，实测发现引擎是**按需启动**（提交首条任务才 `start()`），冷启动后什么都不做就永远不装配，真实窗口第一轮资产页全空。现在 `lib.rs::build_state()` 在窗口创建前同步调用 `EngineSupervisor::assemble_profile_assets()`，结果写 `logs/profile-assets.log`；`start()` 里保留同一次调用做幂等兜底。失败只记日志、`eprintln`，不阻断启动。
2. **应用根改由 `engine_state_dir` 的父目录推导，不再读 `OMNIDESK_APP_DATA_ROOT`。** 原写法让单测直接往真实 `~/OmniDesk/app` 写装配状态，且测试夹具无法隔离。`engine_state_dir` 默认就是 `<app_root>/engine-state`，与生产路径等价。修正时真实目录只被误写过一个 `.assets.v1.json`（其余资产哈希与真实文件一致、`decl.yml` 一条未登记），无资产被改动。
3. **新增 `profile_lifecycle.rs`（计划外补充，服务于「自建 Profile」）。** 登记表 `app_root/profiles/index.v1.json`（schema `omnidesk.profiles.v1`）记录自建 Profile 的来源与插件；支持从 `omnidesk-base` 或指定源复制配置新建、移除（拒绝出厂/当前/未登记，移到 `profiles/.trash/` 可恢复）。自建 Profile 只按自身 `rules.json` 的 `skill_bindings` 补 Skill，不写配置文件，缺失插件以 `plugin:<pkg>` 列出。
4. **巡检改为纯只读。** `inspect` / `inspect_custom` 只报状态、不落任何文件，资产页的「重新读取」不会改动磁盘。

其余偏差：插件条目 `version=None` 时只校验包存在、不校验版本；`install_plugin` 显式传 `DSH_HOME`，确保不会装进 `~/.dsh`；`material-writing` 与 `document-review` 额外物化 `references/`（含 `README.md`）；装配报告带 `summary_note` 供界面直接显示。

实跑结果：`cargo test --offline`（420 lib + 14 集成）全通过；`npx tsc --noEmit`、`npm run build` 通过；`pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 24/24；真实窗口两轮（discipline 自动装配 13/13、omnidesk-base 零写入）全部通过，证据在 `tests/verification/desktop/evidence/od-arch-109/`。

### OD-ARCH-104（插件挂载）：discipline Profile 挂载办公插件（已批准并实施，2026-10-28）

> **触发：** 用户研判三场景插件需求。核实（static）：当前 omnidesk-base 插件面与官方 preset-standard 零差异（30 项全保留）；运行时无 dsh-viewer / univer-office 包；办公能力由 dsh-skill-office（Word/PPT/Excel 工作流，未启用）、dsh-office-to-pdf（web-app 默认依赖，已随附）承担。discipline.profile.patch.yml 现为空 []，Profile 无插件差异。

#### 方案（最小改动）

- S1：discipline.profile.patch.yml 增加 skill-office 条目（**实施纠正：不是往 preset-standard 的 plugins 里追加，而是补丁层的顶层 insert**），仅专业 Profile 启用；基础 Profile 保持纯净。supervisor seed 与 patches 传参链路已支持，无需改 Rust。
- S2：验证——skill-office 在 discipline 生效（隔离环境启动后 capability 清单含 office skill）、omnidesk-base 不含；cargo test 相关用例 + tsc + build。
- S3：证据落 evidence/od-arch-104/。

#### 明确不做

- 不引入 dsh-viewer / univer-office（当前 DSH 版本无此包；预览由官方 documentpreview 与自研 ArtifactExplorer 承担）。
- 甘特图不加插件：OmniDesk 自研确定性渲染已实现（OD-ARCH-102），模型只供数据。
- 不开插件管理界面：维持「插件固化、随升级更新」既定策略。

#### 实施结论（2026-10-28）

**已落地：** 仓库样例与实机 ~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.patch.yml 同口径改成一条顶层 insert（id: skill-office / name: @deepseek-ai/dsh-skill-office），未改 Rust、未改 runtime/。实机改前备份 cordis.patch.yml.bak-od104-20261028-234012。

**计划措辞纠正：** 原文「继承 preset-standard 的 plugins 数组追加」不可行。dsh-app-boot 的 applyEntryPatches 对 config 是整体替换，只有 group 且 config 为数组的条目可定位，因此不存在向 preset 的 plugins 增量追加的路径。实际用的是同一函数第 73–88 行的无 id insert，直接 push 到宿主层，与 skill-filesystem / tool-skill 同级；好处是不必复制 preset 里 30 余条插件声明。

**验证：** 新增 tests/verification/desktop/verify_plugin_scope_104.py，34/34 通过；dsh --dump-config 在 discipline 的输出含 skill-office（顶格、无 disabled、跟随补丁层注释），在 omnidesk-base 的输出不含，两者 stderr 均无 disabling profile plugin；真实启动 --profile discipline --no-open --port 0 正常起服务并打印 web 地址；模块直连探针注册出 office-docx / office-pptx / office-xlsx，正文含 Installed LibreOffice Kit。证据 tests/verification/desktop/evidence/od-arch-104/plugin-scope-104.json。

**落盘缺口（未在本计划内修）：** engine_supervisor.rs 的 seed_profile_if_missing 只播种 web Profile，且 package.json 存在即返回；samples/profiles/discipline.profile.patch.yml 没有任何安装路径复制到用户机器。因此新装与升级都需要手工同步该文件；本轮只保证仓库样例与当前实机一致。

**未做：** 不引入 dsh-viewer / univer-office（当前 DSH 版本无此包）；不动甘特图与插件管理界面；真实模型下未实测 Office 技能的端到端调用。

**验收：** 专业 Profile 下引擎能力清单含 office 三个技能、基础标准完全不含；tsc、build、cargo test --offline（405 lib + 41 集成）、pytest（89 passed / 30 subtests）全部通过，详见 STATUS.md 与 REVIEW.md 顶部。

#### 原定验收

- 专业 Profile 下模型可产出/读取 docx、xlsx；基础 Profile 行为不变；runtime/ 零差异；不提交不推送。

### OD-ARCH-108：绑定只留工作流底线，其余交给模型按需加载（已批准并实施，2026-10-28）
> **实施结论：** S1–S7 全部落地，无架构偏离。两条流程各绑各的底线：随手记只注入 `memory-agent`，周报只注入 `memory-report`；`anysearch` / `knowledge-search` / `document-review` / `knowledge-agent` 已从所有绑定中移除，改由模型按需自读。真实窗口两轮对照 15/15 通过（证据 `tests/verification/desktop/evidence/od-arch-108/flow-injection.json`）。
> **技能正文字数变化（各 Skill 的 SKILL.md 全文，非整段提示词）：** 周报 —— 实机旧规则（`weekly` 绑 `memory-agent / knowledge-search / anysearch`，`max_injected: 3`）4901 → 1099；仓库样例旧口径（`memory-agent / memory-report / knowledge-search`）4402 → 1099。随手记 —— 1281 → 1408（多了「按需加载」那一小段）。计划原先估计的「约 5.9k」，按新正文把四个技能相加才是 6127，实机从未这样绑过，以本节的两个实测值为准。真实窗口实测的**整段提示词**是随手记 9699 字符、周报 9912 字符，含场景规则与安全边界，不能当成技能注入量。
> 执行记录见 STATUS.md 顶部，验收见 REVIEW.md 顶部。
> 分类：注入链路 + 规则资产。把「每场景一份绑定」细化为「每条流程只绑底线技能」，其余能力由模型按需自行加载。
> 用户口径（2026-10-28）：「场景绑定 skill，然后模型根据需要调用额外的 skill。比如我现在在 weekly 场景，随手记时就只绑定 memory-agent，当大模型判断还需要搜索资料时，就自己加载 anysearch；同样周报也只绑定 memory-report，知识库就只绑定 knowledge-search。这是工作流的底线要求，其他以大模型的能力来判断！」

#### 已核实的前提（本人实读代码 + 引擎配置实测）

1. **模型本来就有按需加载通道。** 用 OmniDesk 隔离根导出 `dsh --profile discipline --dump-config`：host 层的 `skill-filesystem` / `tool-skill` 被 `dsh-web-app` 关掉，但**每个 preset 内各挂一份**，`agent-preset-registry.default = standard`，`preset-standard` 里两者均启用；`dsh-skill-filesystem` 的扫描根含 `$DSH_HOME/skills`（`~/OmniDesk/runtime/dsh-home/skills`，8 个 Skill 全在）。模型随时能自己读任一 Skill 正文。
2. **现在的问题不是「预加载」，是「全文注入」且绑得过多。** `profile_rules_view` 按 `skill_bindings` 读盘把正文拼进 preset，`conversationPrompt`（随手记）与 `aiDraft`（周报/初稿）再塞进当轮提示词。`weekly` 一份绑定同时服务两条流程：随手记取第 1 个，周报取 `max_injected` 以内**全部**——四个全绑会让周报凭空背上 `memory-agent`（它自己写着「周报见 memory-report」）。
3. **知识库页面今天没有模型调用。** `KnowledgeVaultPage` 只有扫描 / 读取 / 搜索 / 保存等宿主命令，没有 `submitTask`；检索是确定性的本地全文检索。**因此不存在「知识库场景绑定 knowledge-search」的注入点**，只能纯按需加载。
4. **绑定键可以按流程拆。** `validate()` 只校验键非空、值合法、数量上限，不限制键名集合；`HomeCanvas` 用 `def.preset` 查表，`conversationPrompt` 的 note 分支只取首个。新增 `weekly-note` / `weekly-report` 属规则资产层变化，不改注入骨架。
5. **上限 3 是硬闸，本轮不需要放宽。** `MAX_SKILL_BINDINGS_PER_SCENE = 3`；每条流程只绑 1 个，远低于上限。

#### 方案

**S1 绑定按流程拆，每条只留底线技能。**

| 流程键 | 绑定 | 说明 |
| --- | --- | --- |
| `weekly-note` | `memory-agent` | 随手记底线，平台注入 |
| `weekly-report` | `memory-report` | 周报底线，平台注入 |
| `writing` | `material-writing` | 起草底线 |
| `project` | `project-agent` | 项目底线 |
| （无） | — | 知识库无模型调用，`knowledge-search` 不进任何绑定，纯按需 |

`anysearch`、`knowledge-search`、`document-review`、`knowledge-agent` **不进任何绑定**，全部由模型判断后自行加载。

**兼容别名方向：** 旧键 `weekly` 保留并指向随手记（`memory-agent`），不指向周报。历史语义里 `weekly` 首位就是 `memory-agent`，这样旧规则文件在随手记路径上的行为不变；若指向周报，旧数据会让随手记注入 `memory-report`，正是要避免的错误。

**S2 前端按流程选绑定。** 意图在**提交时**才判定（`HomeChat` 的 `detectWorkIntent`），preset 在提交前已构建，所以不能在 preset 里定型。做法：preset 同时带出 `skillsByFlow`（`note` ← `weekly-note`、`report` ← `weekly-report`，缺则回退 `weekly` / 场景键），`composeConversationPrompt` 按 `mode` 选对应那一组；`writing` / `project` 仍走单组。组装结构与注入位置不变，只是取值多一层，随手记「最多注入 1 个」的既有约束继续生效。

**S3 Skill 正文写明「何时自己加载什么」。** 在 `memory-agent`、`memory-report`、`material-writing`、`project-agent` 的 SKILL.md 各补一小段可选能力提示：需要联网查公开资料时自行加载 `anysearch`，需要查工作台知识库卡片时自行加载 `knowledge-search`，需要成稿审校时自行加载 `document-review`。只写「什么时候该去读」，不写死调用条件、不给固定触发词——目录里只有名称和一句描述，写明时机才算真正的按需加载。字数控制在每条 3 行以内，避免把省下的注入量又加回去。

**S4 两份规则文件同改。** 仓库样例 `samples/profiles/discipline.rules.json`（编译期嵌入，供「恢复默认」用）与实机 `~/OmniDesk/app/profiles/discipline/rules.json`。实机那份属用户数据，改前备份 `rules.json.bak-od108-<时间戳>`，只改 `skill_bindings` 段；写作铁律、红线词表、掩码、scenes 逐字保留。实机文件与样例当前已经漂移（实机 weekly 是 `memory-agent / knowledge-search / anysearch`，样例是 `memory-agent / memory-report / knowledge-search`），本次一并收敛到新口径。

**S5 界面标签可读。** `SettingsPanel` 的 `sceneLabel()` 增加 `weekly-note` → 「工作记录·随手记」、`weekly-report` → 「工作记录·周报」映射，能力管理页的绑定标签不再显示原始键名；`verify_skill_scope_107.py` 的场景标签断言同步。

**S6 旧断言与文案同步。** `verify_worklog_089.py`（`weekly_three_bindings` 等）、`verify_skill_bindings_082.py`（`weekly_does_not_preinject_anysearch` / `weekly_binds_memory_report`）、`tests/verification/phase-worklog/verify_env_085.py`（`weekly_binding_has_knowledge_search` 等）改为新口径；`profile_rules.rs` 内嵌样例相关单测同步；`workstations/discipline/skills/README.md` 表格补「绑定 / 按需」一列。历史证据文件不改写。

**S7 记一条决定。** `docs/decisions.md` 增补 D-035：绑定只表达工作流底线，其余能力由模型按需加载；附带实测证据（`--dump-config` 中 preset 内的 skill 插件与扫描根）。

#### 影响（如实列出）

- 周报的技能正文从实机旧口径（三个绑定全上，4901 字，原估「约 5.9k」偏高）降到**仅 `memory-report`（1099 字）**；随手记维持 `memory-agent`（1281 → 1408 字，多的是按需加载那一段）。省下的 token 是每轮都省的。
- 代价：模型需要额外一次工具往返去读 `anysearch` / `knowledge-search`，且**依赖它自己的判断**。判断不到位时不会检索——这是用户明确选择的取舍，验收里按「已知限制」如实记录，不算缺陷。
- 设置页「本 Profile 预注入」从 6 条降到 4 条（memory-agent / memory-report / material-writing / project-agent），其余归入「可被模型按需加载」。
- 知识库页面维持「无模型、确定性检索」，不因本轮改变。若以后要给知识库加对话式问答，那是独立计划（需要模型调用与授权）。

#### 范围外

- 不改 `MAX_SKILL_BINDINGS_PER_SCENE`（不再需要 4）。
- 不改 DSH runtime 的 preset 与插件面；不改 `runtime/` 内容。
- 不给知识库页面新增模型问答入口。
- 不动 `writing` / `project` 的注入链路，只改绑定内容与 Skill 文案。
- 不改写任何历史证据文件。

#### 验收口径

1. 两份 `rules.json` 里 `weekly-note` / `weekly-report` / `writing` / `project` 各只绑 1 个 Skill，`anysearch`、`knowledge-search`、`document-review`、`knowledge-agent` 不在任何绑定内。
2. 真实窗口：weekly 场景随手记一轮，注入正文只含 `memory-agent`；同一会话说「写周报」一轮，只含 `memory-report`；两轮都不含 `anysearch` 正文。
3. 按需加载通道可用：`--dump-config` 里 preset-standard 的 `skill-filesystem` + `tool-skill` 仍启用，扫描根指向隔离 `dsh-home/skills`；若做一轮需要联网的周报请求，观察模型是否自行读取 `anysearch`，读不到就如实记为「依赖模型判断」的已知限制。
4. 回归全绿：`cargo test --offline`、`pytest tests -q`、`npx tsc --noEmit`、`npm run build`、`verify_skill_scope_107.py` 与其窗口脚本、上述三个改过的验证脚本。
5. 实机 `rules.json` 有备份，`skill_bindings` 以外的字段 diff 为空。

### OD-ARCH-107：能力管理页说清「本 Profile 预注入哪些 Skill、哪些只是按需加载」（已批准并实施，2026-10-28）
> **实施结论：** S1–S4 全部落地，无架构偏离。基础标准页明确写「零预注入」、Skill 全部归入按需加载；discipline 出现「本 Profile 预注入 6」与方形场景标签；注入链路与 Skill 目录结构未动。执行记录见 STATUS.md 顶部，验收见 REVIEW.md 顶部。
> 分类：界面口径 + 只读数据汇总。不改注入链路，不改 Skill 目录结构。
> 用户口径（2026-10-28）：「我切换了 base profile，然后场景就恢复到默认状态了，但在设置页面看 skill 还是和纪检 profile 一样，我不知道哪些加载了，哪些没加载？」

#### 现状（已核实，本人实读代码与实机数据）

1. **设置页列的是磁盘清单，不是注入清单。** `capabilities::list_skills` 直接扫 `<DSH_HOME>/skills`（capabilities.rs:345），`CapabilityView.skills_dir` 也是这个目录；`SettingsPanel.tsx` 能力管理页（约 1917 行）原样渲染 `caps.skills`，只有名称、描述、修改时间。Skill 目录全局一份、所有 Profile 共用，这是 DSH 官方 `skill-filesystem` 语义，不是缺陷。
2. **Profile 真正影响的是「预注入」。** 来源是 `profiles/<Profile>/rules.json` 的 `skill_bindings` 与 `max_injected`；`profile_rules_load` 把绑定 Skill 的 SKILL.md 读进 `skill_contents`。基础标准没有 rules.json 时 `rulesView.loaded=false` → preset=null → **零 Skill 预注入**，但模型仍可按需加载任一 Skill。
3. **注入量按流程不同。** 周报/初稿（aiDraft.ts:115）注入 `max_injected` 以内的全部绑定；随手记（conversationPrompt.ts:123）只取第 1 个绑定。
4. **实机状态。** 引擎跑 `--profile omnidesk-base`；`profiles/omnidesk-base/` 目录不存在（无 decl.yml、无 rules.json），符合标准模式不造文件的约定。`profiles/discipline/` 有 3 条绑定。
5. **用户文件与仓库样例存在漂移。** live `discipline/rules.json` 的 weekly 绑定是 `memory-agent / knowledge-search / anysearch`，仓库样例是 `memory-agent / memory-report / knowledge-search`（memory-report 是 c1e823f 之后才加进样例的）。属用户数据，本轮不改，只在报告里说明。
6. 界面目前没有任何「绑定 / 预注入 / 可选用」的区分，所以切 Profile 后这一页看起来完全没有变化。

#### 方案

**S1 只读汇总，不新增解析。** `capability_view` 命令复用 `profile_rules_load` 同一套读取（含迁移与 skill_contents 回填），把「rules.loaded / rules.error / skill_bindings / 实际读到的正文」压成一份 `SkillScope` 传入 `capabilities::load`；capabilities 模块不自己读 rules 文件，避免两处解析口径分叉。

**S2 每条 Skill 带状态。** `SkillEntry` 增加 `bindings: Vec<{scene, order, max_injected, effective}>`，`effective = order <= max_injected`（即会进入预注入）；`CapabilityView` 增加 `rules_loaded / rules_error / standard / scenes / missing_bindings`。

**S3 缺失绑定要暴露。** 绑定了但磁盘上找不到 SKILL.md 的条目单列提示。现在这种故障是静默的——注入时只是没内容，界面看不出来。

**S4 界面分组。** Skill 列表分成「本 Profile 预注入（N）」与「可被模型按需加载（M）」，每条显示绑定的场景标签；基础标准下页顶一句话说明「基础标准不预注入任何 Skill，下面的 Skill 仍可由模型按需加载」；并注明随手记只注入第 1 个绑定。规则文件损坏时如实显示损坏，不退化成「未加载」。

#### 范围外

- 不按 Profile 隔离 Skill 目录（会破坏「base = 官方 DSH 底座」这条 106 定下的边界）。
- 不改注入链路、不改 rules.json 内容，不自动替用户对齐 live 与样例的绑定差异。
- 不新增 Skill 编辑与启用开关（D-030 口径不变）。

#### 验收口径

1. 基础标准（无 rules.json）：页面明确显示零预注入，Skill 仍列为可按需加载；探针断言页面没有「预注入」条目。
2. discipline：weekly / writing / project 的绑定逐个出现在对应场景标签下，effective 与实际注入顺序一致；随手记只取第 1 个的口径在界面上可读。
3. 绑定指向不存在的 Skill → 界面给出缺失提示，不静默。
4. 回归：`tsc --noEmit`、`npm run build`、`cargo test`、`pytest` 全绿；新增 `verify_skill_scope_107.py` 静态断言与 capabilities 侧单测；标准模式 Dock 3 入口基线不变。

### OD-ARCH-106：基础标准 = 干净 DSH 底座，场景/Skill/规则随 Profile 挂载（已批准并实施，2026-10-28）
> **实施结论：** S1–S5 全部落地，无架构偏离。基础标准下 Dock 只剩 3 个平台固有入口、无声明时不再自动造文件、不装行业规则包；切到 discipline 后场景与规则按原基线恢复。执行记录见 STATUS.md 顶部，验收见 REVIEW.md 顶部。

> 分类：架构调整（Profile 分层）。用户口径（2026-10-28）：「不加载纪检等自定义 profile 时，初始就是基础标准（omnidesk-base，默认），这个状态的 omnidesk 就是自定义 UI + DSH cli 底座，和使用官方 DSH-desktop 没差别；场景功能、skill 和配套插件都跟随 profile，切为自定义 profile 时自动加载。」补充确认：「默认 profile 可以保持最基础的 DSH 默认配置 skill 和插件。」

#### 现状（已核实的事实，2026-10-28）

1. 插件与 bundle 已随 Profile：宿主按 `engine_profile` 解析 Profile 名（profile_choice.rs resolve()），以 `dsh --profile <名>` 启动引擎（engine_supervisor.rs），下次启动生效。
2. 业务场景（Dock 入口、对话预设、右栏）不随 Profile：入口来自应用数据根下唯一一份 `profiles/discipline.yml`（profile_decl.rs，注释明确「只有一个生效文件」）；规则资产与 skill_bindings 也来自唯一一份 `profiles/discipline.rules.json`（profile_rules.rs）。切回基础标准后这些仍然生效。
3. Skill 文件是隔离 DSH home 下全局一份（scripts/sync-skills.sh 同步到 `runtime/dsh-home/skills/`），所有 Profile 共用。
4. 与官方 DSH 的偏离：`omnidesk-base.profile.patch.yml` 显式禁用了 `skill-filesystem` 与 `tool-skill` 两个插件；官方 dsh-base 里这两个默认启用。这使基础标准下的模型无法按需加载技能，与「和官方 DSH 只差 UI」的目标冲突。
5. 对照官方 DSH Desktop（docs/user-guide.md）：Profile 是隔离单位（bundle + 依赖 + patch），切换有序重启；skill-filesystem 从 DSH home 的 skills/ 扫描技能。DSH 没有「场景」层，场景挂载属于 OmniDesk 产品层，需自建。

#### 方案

**S1 默认 Profile 回归官方默认配置**：`omnidesk-base.profile.patch.yml` 移除对 `skill-filesystem` / `tool-skill` 的禁用，保持与官方 dsh-base 一致的插件面。「不重复注入、省 token」的取舍改由场景层承担（S4），不再用禁插件实现。

**S2 场景声明随 Profile**：Dock 入口声明从固定 `profiles/discipline.yml` 改为按 Profile 取 `profiles/<profile>/decl.yml`（文件名待实现时定，目录按 Profile 隔离）。基础标准下无此文件 → 只显示平台固有入口：首页、历史会话、知识库、设置。现有单文件 `~/OmniDesk/app/profiles/discipline.yml` 做一次性兼容迁移（移入 discipline Profile 的声明位置），不静默丢弃。

**S3 规则资产随 Profile**：`discipline.rules.json` 同样按 Profile 目录取（`profiles/<profile>/rules.json`）。基础标准下无文件 = 纯净底座（零关键词、零铁律、零绑定，现有 RulePolicy::clean() 语义）。现有文件一次性迁移进 discipline Profile 目录，保留 .bak 备份机制。

**S4 场景预设与 Skill 注入按「场景存在」判定**：HomeCanvas 的 preset 判定改为——场景入口不存在（声明里没有）或规则资产未加载时，preset = null，对话按普通对话处理，不套场景提示词组装器、不注入 Skill 原文、不进周报/随手记流程。`workIntent.ts` 的意图路由只在 weekly 场景 preset 存在时参与。

**S5 界面口径诚实**：基础标准下，顶栏徽标仍显示「标准」；首页与设置页明确显示「标准模式：DSH 底座 + OmniDesk UI，无业务场景」。切换到 discipline 后显示其 display_name 与场景清单。Profile 选择仍沿用「下次启动引擎生效」语义，不新增热切换。

#### 范围外（本轮不做）

- 不做 Profile 热切换；不改模型路由 overlay 机制（各 Profile 的模型选择本就互不相通）。
- 不新增 Profile 管理器（不复制 Profile、不创建 Profile）；白名单仍为 omnidesk-base / discipline / acp。
- 不改 skills/ 全局目录结构；S2/S3 的按 Profile 隔离只覆盖声明与规则资产，Skill 文件仍共享一份（与官方 DSH skill-filesystem 行为一致）。

#### 验收口径

1. `engine_profile` 为 omnidesk-base（或空）启动：Dock 无工作记录/文案写作/项目跟踪入口；对话不出现场景预设与 Skill 注入（探针断言提示词不含场景规则、铁律、Skill 原文）；规则资产视为未加载（rulesLoaded=false）。
2. 切到 discipline 并重启引擎：场景入口、规则、Skill 绑定、场景预设全部恢复现有行为（与 105 验收基线一致）。
3. `omnidesk-base.profile.patch.yml` 不再含 disabled: true 的 skill 插件；用 `--dump-config` 或候选读取确认插件面与官方 dsh-base 一致。
4. 旧数据迁移：既有 discipline.rules.json / discipline.yml 在新位置生效，旧文件保留为迁移备份，无用户数据丢失。
5. 回归：105 随手记（verify_worklog_104/089）、082 skill 绑定、088 意图探针全部通过；tsc、cargo test、pytest 全绿。

### OD-ARCH-105：把随手记的业务判断全部下放到 Skill，平台只留数据与写盘边界（已批准，已实施，2026-10-28）
> **实施结论：** S1–S4 全部落地。判定协议由四分类（record/move/confirm/answer）收成通用三字段（append/move/note），平台提示词不再含任何业务规则；随手记平台段实测 248 字（不含注入的 Skill），真实模型准确率仍属未验证项。执行记录见 STATUS.md，验收见 REVIEW.md 顶部。

> 分类：架构调整（平台通用性）。用户口径（2026-10-28）：「我希望这种判断和改变都只是基于 skill，因为这个 omnidesk 平台是通用的，不能有这种特殊限制……注意保持省 token 和快回复。」

#### 问题（事实）

1. OD-ARCH-104 把七条判断原则、JSON 示例、六要素追问都写进了 `conversationPrompt.ts`，与 `skills/memory-agent/SKILL.md` 重复。平台因此带上了「工作记录」这个场景的业务口径——换一个场景或换一套 Skill，平台仍按同一套规则约束模型。
2. 判定协议本身也带业务分类：`action ∈ {record,move,confirm,answer}`，以及「record 却没有记录」一类的 action 一致性校验。这些都是业务语义，不是平台契约。

#### 方案

**S1 平台提示词只留三件事**：宿主读到的数据（今天/当周文件/条目索引）、写盘边界（写入由工作台执行、模型不碰文件）、绑定 Skill 原文注入。没有绑定时只说「不要臆测判断口径」。

**S2 判定协议通用化**：`{"append":[…],"move":[…],"note":"…"}`。两数组都空 = 只回一句话（确认、追问、回答），宿主不碰文件；同时给出也可以（先挪后加）。平台不认识「事实」「指令」「确认」这些概念。

**S3 校验只守确定性边界**（不下放也不上收）：正文须为用户原话连续片段、日期在本 ISO 周且不晚于今天、move 的 date+index 须在索引里、同一位置不得移两次、同一段话不得既追加又移动；任一不合规整份作废走降级。

**S4 Skill 承载全部判断**：事实与指令的区分、续述跟随、确认句、纯提问、六要素追问、去向默认，全部只在 `memory-agent/SKILL.md`。

#### 验收口径

- 平台随手记段（去掉注入 Skill 后）≤ 700 字，且不含 `判断原则`/`六要素`/`逐字取自用户原话`/`omnidesk-verdict`，这四项须能在 Skill 里找到。
- `append`/`move`/`note` 的解析、校验、降级、回执行为由探针覆盖；宿主侧落盘不变。

### OD-ARCH-104：随手记改由模型判定「指令 / 内容」，代码只负责校验与落盘（已批准，已实施，2026-10-28）
> **实施结论：** S1–S5 全部按计划落地，无架构偏离。模型只在正文后附判定块，宿主校验通过才落盘；无判定或校验不过时原话按当天记录并明说未做指令识别。执行记录见 STATUS.md，验收结论见 REVIEW.md 顶部；**真实模型的分类准确率属未验证项**，需用户端到端实测。下文为原始计划，保留不改写。


> 分类：功能修复 + 一处架构调整（记录写入路径）。
> 用户口径（2026-10-28）：不要用一个规则去判断指令和内容，那是模型该做的事；机械执行导致「上杭教学和参观紫金矿业都是本周一的行动，你全部调整至周一的记录。」被当成正文写进文件。

#### 现状与问题（只读排查，事实）

1. 现在的随手记是「先按规则判断 → 再写文件 → 最后叫模型追问缺项」。判断完全在前端正则里：`detectRedateInstruction` 有 40 字上限与「剩余 ≤6 字」两道门槛，`isNoteContinuation` 只认白名单词，`CORRECTION_MARKERS` 漏掉「就是周一上午」这类确认句。规则没覆盖到的说法一律按正文追加，模型从头到尾没有参与判断。
2. 这条路径本来就**每轮调用一次模型**（只做六要素追问），所以把判断交给模型不增加调用次数。
3. 引擎会话的 `mcpServers` 恒为 `[]`（`acp_adapter.py:673/690/1123`），模型没有宿主命令通道，只能把判定结果交回界面，由界面执行写入。
4. 宿主只有单条 `worklog_redate`，没有批量原子移动。

#### 方案：模型判语义，代码守边界

**S1 模型返回结构化判定（`conversationPrompt.ts` + `memory-agent` Skill）**

随手记提示词改为：带上一份**当周条目索引**（日期 + 序号 + 正文摘要，最近 7 天、每条截断 80 字、上限 60 条）与今天日期，要求模型先给一段 JSON：

```json
{ "action": "record" | "move" | "confirm" | "answer",
  "records": [{ "text": "原话片段", "date": "2026-10-26" }],
  "moves": [{ "date": "2026-10-28", "index": 2, "to": "2026-10-26" }],
  "reply": "一句给用户的话，含缺项追问" }
```

判断原则写进 Skill，不再写进正则：

- 一句话里混着事实与命令时，只把**事实**放进 `records`，命令部分只体现在 `moves`／`reply`。
- 用户说「都是周一的」「全部调整到周一」时，先根据索引找出对应的旧条目再给 `moves`；找不到就 `action=answer` 并说明，不要新建一条记录冒充。
- 无日期的细节句，若明显在续述上一条（「还参观了」「随后」），沿用上一条日期；若是全新主题，用今天。
- 「就是周一上午」这类确认句：`action=confirm`，只确认日期，不写新内容。
- 纯提问、查记录：`action=answer`，不写文件。

**S2 代码只做校验，不做语义判断（新增 `desktop/src/workRecordVerdict.ts`，纯函数）**

- 解析模型 JSON；解析失败或字段不合规 → 视为无判定。
- `records[].text` 必须是用户原话里的连续片段（去空白与标点后比对）；凭空生成的句子一律拒绝。这条防的是模型改写与编造，不是替它做判断。
- 日期必须在本 ISO 周内且不晚于今天；`moves` 的 `date/index` 必须能在当周索引里对上，且 `to` 合法。
- 校验通过才执行；任何一条不合规，整份判定作废。

**S3 宿主批量移动（`worklog.rs` + `lib.rs`）**

- 新增 `move_entries(root, week, moves[])` 与命令 `worklog_move_many`：一次读入、逐条按 `expect_text` 校验、全部通过才重写；任一条对不上整批拒绝，文件一字不动。
- 沿用 `move_entry` 的保证：同周、不晚于今天、正文原样、条目数守恒、只允许增删相关日期标题、备份、tmp + fsync + rename、写后回读。多份备份合成一份。

**S4 前端执行（`HomeChat.tsx`）**

- 随手记顺序改为：读当周索引 → 调模型拿判定 → 校验 → 执行（`record` 逐条带日期追加；`move` 调 `worklog_move_many`）→ 显示模型那句回复，并附上「实际写入/移动了哪几条」的确定性回执。
- 移动结果逐条列出「原日期 → 目标日期 + 正文摘要」，用户能一眼核对。
- **失败不丢数据**：模型超时（15 秒）或判定不可解析时，按原文原样追加到当天，并标明「模型未给出判定，已按原文原样记录，未做指令识别」；该轮不执行任何移动，避免重复落盘。
- 删除依赖旧正则的判定分支（`detectRedateInstruction` / `isNoteContinuation` 的调用）；`extractEntryDate` 保留给降级路径与既有测试。

**S5 架构决策留痕**

- `docs/decisions.md` 增加一条：随手记的语义判定交模型，写入与校验留在宿主；写明理由、影响与复审触发条件（模型判定错得比规则多时回退）。

#### 代价（如实说明）

- 记录在模型返回后才落盘（通常几秒），不再是「敲完立刻入库」。换来的是语义判断不再依赖正则。
- 提示词多一份当周索引（约 300–1500 字符），只在该场景、该模式下出现。
- 模型判定质量需在真实使用中观察；`action=answer` 与 `moves` 的准确率是复审重点。

#### 非目标

- 不自动删除文件里已被误写的两句指令（需另行授权）。
- 不改 `runtime/`；不做跨周移动；不动周报流程。

#### 验证

- 新增 `tests/verification/desktop/verify_worklog_104.py`：用死数据 fixture 跑 `workRecordVerdict.ts` 的校验与执行计划，覆盖原话片段校验、跨周/未来日期、索引越界、JSON 残缺、降级路径。
- Rust 单测：批量移动成功、部分不符整批拒绝、跨周拒绝、目标段已存在、空段清理、备份唯一、CRLF。
- 回归 103/101/094/089；`tsc`、`npm build`、`cargo test --offline`、`pytest`。
- 真实模型准确率作为实测项留给你端到端验证，不写进「已通过」。
- 证据落 `evidence/od-arch-104/`；提交并推送。

#### 验收标准

- 用户原三句：指令句不产生正文条目；「上杭教学」「紫金矿业」两条被移到周一并逐条回执；无日期细节跟随；「就是周一上午」只确认。
- 普通随手记、纯提问、模型失败降级三条路径行为与描述一致。

### OD-ARCH-103：随手记补录日期语义化——输入日期识别 + 本地纠正通道（已批准，已实施，2026-10-28）

> **实施结论：** S1/S2 按计划完成。S3 的**触发点**由「模型工具调用」改为「界面确定性纠正」——读码发现 `acp_adapter.py:673/690/1123` 的会话请求 `mcpServers` 恒为 `[]`，模型没有宿主命令通道，原路径不可达；命令、`expect_text` 校验、备份与守恒自证全部保留。另修三处审查发现的问题（`isoWeekKeyOf` 正则转义、前后端时区口径、跨会话 `lastNoteRef` 泄漏）。执行记录见 STATUS.md，验收结论见 REVIEW.md 顶部。下文为原始计划，保留不改写。

> **触发：** 用户实测发现，周三与 AI 对话补录周一课程（含追加 2 条细节），三条全部记到周三标题下。根因（读码事实）：`worklog_append`（lib.rs:2086-2087）日期固定取当天 `date_label(now)`，前端 `worklogAppend(text)` 不传日期，「周一」等日期语义从输入到落盘无任何通道。

#### 一、方案（确定性为主，LLM 语义为辅，不增加每条记录的模型调用）

**S1 输入日期识别（前端纯函数，零模型成本）**

- `workIntent.ts` 新增 `extractEntryDate(text, now, offsetMinutes)`：识别「周一~周日」（取本周内最近一个过去的日期）、「昨天/前天/今天」、「M月D日」「YYYY-MM-DD」（当年）。命中且属当前 ISO 周 → 作为 entryDate 传给宿主；命中但属其他周或指向未来（如周三说「周五」）→ 不猜，仍用当天，交由 S3 模型追问或用户口述。
- 示例验证：周三输入含「周一上午」→ entryDate=本周一；三条同日对话（原文 + 2 条细节）都命中周一 → 全部落进 `## 周一日期` 段（首条建段，其余「（续）」），即用户要的「周一的记录」。

**S2 宿主接收 entryDate（最小改动）**

- `worklog_append` 增加可选参数 `entry_date`：校验格式 YYYY-MM-DD、属当前周文件对应 ISO 周、不晚于当天；不合法一律拒绝并回明确错误（不静默回退当天）。缺省行为不变（当天）。

**S3 模型纠正通道（LLM 语义兜底，覆盖 S1 认不出的表达）**

- 新宿主命令 `worklog_redate`：把某条记录（`expect_text` 定位，同 `worklog_set_destination` 的校验思路）从错误日期段移到正确日期段，仅限同一周文件内；移动 = 原行删除 + 目标段追加，内容不改写，保留防覆盖自证。
- `memory-agent` Skill 增加一条规则：回执中的落盘日期与原文时间语义不符时（如原文「周一」但回执是周三），调用 `worklog_redate` 纠正，并告知用户「已把这条挪到周一」；无法确定目标日期时按缺项追问。
- 前端 api.ts 增加对应封装，模型经工具调用触发。

#### 二、边界说明（如实）

- 多条对话各自成行、同归一个日期段：不把三条合并成一行——记录层保持追加只增不改动内容，「合成为一段话」是周报提炼的职责，不是随手记的。
- 跨周补录（如「上周一」）：本期只落当前周文件并让模型提示用户周次不符；跨文件移动待有真实需求再做。

#### 三、实施与验证

- S1 前端纯函数 + 单测（含用户原始三条输入的用例）；S2 Rust 校验单测（合法/他周/未来/格式错）；S3 redate 单测（定位不符拒绝、正常移动、（续）段）。
- 更新 verify_worklog_089.py 增补日期识别断言；tsc、cargo test 相关模块、npm build 通过。
- 证据落 evidence/od-arch-103/（新目录，不覆盖旧证据）。

#### 四、验收标准

- 用户原始场景复现：周三输入三条（含「周一上午」），全部归入周一日期段。
- 不含日期词的随手记行为与今天完全一致（回归）。
- runtime/ 零差异；不提交不推送。

### OD-ARCH-102：项目跟进页 v3——指标卡移除、左栏双区与事项 CRUD（已批准，已实施，2026-10-28）

> 分类：界面改版 + 功能新增（一条新的本地持久化写入路径）。用户在已验收的 OD-ARCH-098 两栏版式上提三点：①顶部的指标卡先去掉；②左栏分上下两部分——第一是项目事项，第二是历史对话；③项目事项卡片要有录入入口，卡片可修改、可删除，录入后右侧才显示项目时间线。
> 设计沿用 098 已定版式语言（rounded-2xl 白卡、英文小字幕、状态语义色、无百分比数字、里程碑 vs 区间条），本计划只改信息架构与数据来源，不再引入新视觉体系。

#### 已查清的事实（只读排查，2026-10-28）

1. **顶部指标卡**：`ScenePage.tsx:658` 对所有场景渲染 `SceneStatusStrip`；project 分支的指标卡逻辑在 `SceneStatusStrip.tsx:435` 起。移除只影响 project 场景。
2. **左栏现状**：`ProjectBoard.tsx`（452 行）左栏只有搜索 + 卡片列表；无历史对话区。
3. **数据现状**：卡片数据来自首页最近一条用户消息经 `task_project_summary` 的确定性汇总——**只读、无持久化实体**，因此 098 的页面只读；要支持录入/修改/删除，必须有新的持久化存储，这是本计划唯一的功能性新增。
4. **持久化先例**：thread_store（应用数据 JSON、原子写）、pending_writes（校验 + 确认）。

#### 改版方案

- **R1 顶部指标卡移除**：`SceneStatusStrip` 在 project 场景不渲染（writing/weekly 不动）。
- **R2 左栏上部「项目事项」**：搜索框 + 「＋ 新增事项」内联表单（事项 / 负责人代号 / 期限 / 可选开始日期 / 状态 / 下一步）+ 卡片列表。卡片在 098 基础上增加编辑（表单回填）与删除（ConfirmDialog 确认）动作。
- **R3 左栏下部「历史对话」**：project 标签的场景会话常显列表（数据 = sceneThreads，同 094 灵感架），点击回首页并定位对应会话；不新增存储。
- **R4 右栏条件渲染**：事项数为 0 时不渲染甘特，显示引导空态「在左侧录入第一条项目事项后，这里会出现项目时间线」；有事项时甘特即刷新（新增/修改/删除同步）；「甘特 / 成果」次级切换保留。
- **R5 本地事项存储（唯一写入路径）**：新增宿主命令 `project_items_read` / `project_items_save`，持久化为应用数据根下 `project-items.v1.json`（schema `omnidesk.project-items.v1`，原子写 + 写前 `.bak` 备份）。条目字段：item / owner / due / start(可选) / status / next_step / created_at / updated_at。校验复用 `summarize_project` 规则（日期合法、负责人 P+数字、状态枚举），非法条目整批拒绝并报行级错误。该文件是应用数据（与 threads 同级），不是用户知识文件；CRUD 不触碰成果目录与知识库。旧链路（首页对话文本 → task_project_summary）保留不动，但项目页 v3 不再以它为数据源。

#### 实施步骤

**S01（OD-ARCH-102-S01）**

1. Step ID：OD-ARCH-102-S01
2. 单一动作描述：实现 `project_items_read` / `project_items_save` 宿主命令与 `project-items.v1.json` 持久化（原子写、`.bak`、条目校验），补 Rust 单测；types.ts 与 api.ts 同步。
3. 输入来源：task_shell.rs 校验实现、thread_store 持久化先例。
4. 输出目标：两个宿主命令 + 单测（含损坏 JSON 走 `.bak`、非法条目拒绝、空文件首读）；前端类型与 API 封装。
5. 验收标准：保存非法条目整批拒绝且报行级错误；损坏文件恢复路径有测试；读写不触碰其它目录；不调模型。

**S02（OD-ARCH-102-S02）**

1. Step ID：OD-ARCH-102-S02
2. 单一动作描述：ProjectBoard 改 v3——左栏上下双区（事项 CRUD + 历史对话）、右栏条件渲染甘特、project 场景移除 SceneStatusStrip。
3. 输入来源：S01 命令；ProjectBoard.tsx 与 projectGantt.ts 既有实现；sceneThreads；ConfirmDialog。
4. 输出目标：v3 渲染与交互；窄窗单列顺序 左栏 → 右栏；错误/空态如实显示。
5. 验收标准：新增/编辑/删除各有端到端路径（表单 → 宿主 → 文件 → 列表与甘特刷新）；事项 0 时甘特不渲染且空态文案正确；历史对话点击回首页定位会话；project 顶部无指标卡且 weekly/writing 不回归。

**S03（OD-ARCH-102-S03）**

1. Step ID：OD-ARCH-102-S03
2. 单一动作描述：更新静态断言与窗口验证脚本，执行全量回归并落盘证据。
3. 输入来源：S01/S02 差异；既有 verify/preview 脚本与一次性 Xvfb 模式。
4. 输出目标：`evidence/od-arch-102/`（空态引导、录入→甘特出现、编辑改期条目移动、删除→行消失、搜索过滤、历史对话区、窄窗截图与 facts.json）；STATUS.md 执行记录。
5. 验收标准：窗口断言覆盖空态、录入后甘特出现、卡片数 = 存储数、编辑/删除重读验证；回归 091/092/093/094、tsc、build、pytest、cargo 全过；`runtime/` 零差异；只用 /tmp 合成夹具。

**S04（OD-ARCH-102-S04）**

1. Step ID：OD-ARCH-102-S04
2. 单一动作描述：完成实施记录，等待 ARCHITECT 独立验收。
3. 输入来源：S01–S03 差异与测试输出。
4. 输出目标：STATUS.md 执行记录；REVIEW.md 结论另行写入。
5. 验收标准：Git 状态如实记录；既有未提交改动全部保留；不提交、不推送。

**依赖关系：** S01 → S02 → S03 → S04。

**关键取舍：** ① 存储定位为应用数据（同 threads），不是用户知识文件——CRUD 是用户新要求，页面不再只读，但写入路径收敛到单一 JSON 文件，删除有确认 + `.bak`。② 旧数据不迁移——v3 页面从空清单开始，首页对话 → 确定性汇总的旧链路保留但不再驱动本页；避免把对话文本猜成结构化条目。③ 不虚构进度百分比、里程碑不猜工期（098 取舍延续）。④ 不做拖拽改期与多人协作（无数据源）；编辑统一走表单。

**阶段门槛：** 用户已批准并授权执行；S01–S04 已实施完毕。ARCHITECT 已独立审查并验收通过（结论见 REVIEW.md 顶部 OD-ARCH-102 章节）。

### OD-ARCH-101：全局悬停说明统一为软件自绘方角卡片（已批准，已实施，2026-10-28）

> 分类：界面一致性 + 全局交互组件。用户要求：把软件里所有悬停说明都改成与 OmniDesk 界面一致的样式，沿用四角直角、白底细边框的卡片语言。
> **状态：用户本轮明确同意执行；S01–S03 实施完毕，验证证据见 STATUS.md，审查结论见 REVIEW.md 顶部。**

#### 已查清的事实（只读排查，2026-10-28；HEAD `3e2bd17`）

1. 全仓 `title=` 共 132 处，其中 **91 处是原生 DOM 标签上的悬停说明**，分布在 23 个文件；另外 **41 处是组件 prop**（如 `<CardHead title=…>`、`<ConfirmDialog title=…>`），不是浏览器悬停提示，不能一并替换。
2. 91 处里有 **13 处位于 `disabled` 控件上**（设置页删除/测试连接、分类选择、附件增删、新对话、发送、模型路由、去向开关、打开原文）。原生 disabled 控件不派发鼠标事件，接管层必须用命中测试兜底，否则这些说明永远不出现。
3. 现有可复用的视觉源头有两处：`StatusToast` 的右下角提示条（直角、`border-outline-variant/70`、白底、轻投影、11.5px）与 `TurnRail` 的轮次预览卡（直角、白底、细边框）。字体令牌为 Inter Variable + IBM Plex Mono，颜色走 `@theme` 的 surface/outline/primary。
4. 浮层层级已有：确认框 60、窗口控件 70、场景状态条 80、原文预览 90、分类选择 95、状态提示 110；全局悬停提示取 **105**，位于常规浮层之上、状态提示之下。
5. 会受影响的既有断言：`verify_weekly_093.py:218/260`、`verify_weekly_097.py:114`、`verify_weekly_099.py:97/120–121` 都在断言 `title={…}`，需同步改判为 `data-hover-hint={…}`。

#### 方案

- **V1 全局接管**：新增 `desktop/src/shell/HoverHint.tsx`，在 `App` 顶层挂载一次。用事件委托读取 `[data-hover-hint]`，通过 `createPortal` 渲染到 `body`，避免被滚动容器裁切；悬停延迟约 320ms，移出、滚动、窗口缩放、Esc 即消失；视口边缘自动翻到下方并水平夹紧。
- **V2 原生标签迁移**：只把**小写开头的原生标签**上的 `title=` 换成 `data-hover-hint=`，删除原生 `title` 以免系统黑色气泡与自绘卡片同时出现；组件 prop 的 `title` 一律不动。`undefined` 条件文案保持不渲染。
- **V3 样式与可达性**：方角、纯白底、1px `outline-variant/70` 边框、轻投影、11.5px 正文、最大宽 320px，与 StatusToast / TurnRail 同源；`role="tooltip"`、`pointer-events-none`，展示期间给触发元素补 `aria-describedby`；disabled 控件通过 `elementFromPoint` 命中兜底，不额外包一层容器、不改布局。

#### 实施步骤

**S01（OD-ARCH-101-S01）**：新增 `HoverHint` 组件并在 `App.tsx` 顶层挂载；实现延迟、翻转、夹紧、Esc/滚动关闭、`aria-describedby` 与 disabled 命中兜底。
**S02（OD-ARCH-101-S02）**：批量迁移 91 处原生 `title=` → `data-hover-hint=`，核对 41 处组件 prop 未被误改。
**S03（OD-ARCH-101-S03）**：新增 `verify_hover_hint_101.py`，修订 5 处冲突断言，跑前端构建、静态回归与窗口证据，更新 STATUS/REVIEW。

**影响范围：** 新增 `desktop/src/shell/HoverHint.tsx`，修改 `App.tsx` 与其余 22 个含原生 `title` 的 tsx（合计 23 个文件、91 处），`tests/verification/desktop/verify_hover_hint_101.py`，修订 `verify_weekly_0{93,97,99}.py`，以及 `PLANS.md` / `STATUS.md` / `REVIEW.md`。

**不做的事：** 不改任何组件 prop 的 `title`；不改 TurnRail 的轮次预览卡（它已是同款方角卡片）；不改数据、模型、工作流；不碰 `runtime/`。

**设计借鉴（OD-ARCH-035）：** 已查看 Linear（悬停说明只留一行短句、贴近触发元素）与 Airtable（白底细边框、直角、投影很轻）以及现有 TurnRail/StatusToast。落点：①统一成白底直角小卡，不用深色系统气泡；②说明文字只承载补充信息，触发元素本身的标签与无障碍名称继续由 `aria-label` 承担。

### OD-ARCH-100：周报页五处视觉与减冗修订——图标去底、英文字体、圆形去向、成果库去搜索、去掉归档提示（已批准，已实施，2026-10-28）

> 分类：界面微调 + 字体资源 + 一处去功能。用户本轮提五点：①顶部四张指标卡的图标去掉背景色块，图标放大到原来色块那么大；②所有英文字体换一个更清晰、更厚实的，现在太单薄；③去向点选的方形按钮改圆形，「进周报」三个字去掉，改成鼠标悬停提示；④成果文档库的搜索框与相关功能去掉，顶部导航已有搜索，重复了；⑤「另有 N 个归档文件名不符合约定，未列入、也未改动。」这句提示去掉。
> **状态：用户已批准；S01–S06 全部实施完毕，本人实跑验证与窗口证据见 STATUS.md，审查结论见 REVIEW.md 顶部。**

#### 已查清的事实（只读排查，2026-10-28；HEAD `9989d4d`）

1. **指标卡图标**在 `SceneStatusStrip.tsx` 的 `BandMetric`：图标座是 56×56 的 `w-14 h-14 rounded-2xl` 加 `skin.chip`（`bg-primary/10` 这类浅色底）。图标本体 28px（`w-7 h-7`）。四张卡的色相来自 `BAND_TONES` 的 `chip` 字段；逾期告警态用 `BAND_ALERT.chip`。
2. **英文字体**分两条链路：① `theme.css` 的 `--font-*` 令牌用 Sora / Plus Jakarta Sans / JetBrains Mono，`styles.css` 的 `.mono` 也写死 JetBrains Mono；② 字体文件由 `main.tsx` 从 `@fontsource/*` 自托管引入，仅 latin 子集，CJK 走系统字体。**本机无外网依赖问题**：`npm install @fontsource-variable/inter` 实测可用（已安装成功后又恢复，未留在工作区）。
3. **去向开关**在 `WeeklyTimeline.tsx` 的 `DestinationSwitch`：`rounded-none` 方角按钮，内含 13px 方角勾选框 + 可见文字「进周报」。`aria-label` 与 `title` 已经带完整语义，所以去掉可见文字不损失可读性。
4. **成果库工具栏**在 `ArtifactExplorer.tsx:298–380`：第一行是搜索输入框（`placeholder="搜索成果或归档"`），第二行是卡片/列表切换与「含子目录」。搜索状态 `query` 参与 `filtered` 计算，`includeSubdirs` 是目录范围开关、与搜索无关。`ArtifactExplorer` 被工作记录页、写作页、项目跟进页与兜底分支共四处复用，`compactToolbar` 只影响时间范围那组按钮。
5. **归档提示**在 `ArtifactExplorer.tsx:384–388`，来自 `archiveSkipped`（`ScenePage.tsx:862` 传 `archive?.skipped ?? 0`）。这只是可见文案；`skipped` 计数在宿主侧仍用于判断归档目录是否可读，去掉文案不影响读取逻辑。
6. **会受影响的既有断言**：`verify_weekly_093.py:192` 的 `artifact_search_kept_in_compact` 与 `:292` 的 `artifact_toolbar_two_rows` 都断言搜索框存在；`verify_weekly_099.py` 有 `band_icon_seat_larger`、`dest_square_button`、`band_icon_column_no_text` 等与本次改动直接冲突的断言。这些必须同步修订。

#### 方案

- **V1 图标去底放大器（第 1 条）**：图标座去掉 `skin.chip` 的底色与圆角块，只保留色相文字（`text-primary` 这类），图标本体从 `w-7 h-7` 放大到 `w-14 h-14`，正好占满原来 56px 的色块位置。列宽仍是 `w-14`，右侧文字与数字的位置不变，四卡等高关系不变。`BAND_TONES` 里 `chip` 字段改为只含文字色的 token，逾期告警态同样处理。
- **V2 英文字体换厚（第 2 条）**：主字体由 Sora / Plus Jakarta Sans 换成 **Inter**（自托管 `@fontsource-variable/inter`，只引 latin 子集，仍是本地文件、零外链），Inter 在小字号下笔画更实、辨识度更高。标签/序号这类等宽小字由 JetBrains Mono 换成 **IBM Plex Mono**（同样自托管），比 JetBrains Mono 的细笔画更清楚；若你更想保留等宽观感，也可只换正文、序号继续用等宽。`--font-*` 令牌与 `styles.css` 的 `.mono` 同步改，CJK 回退栈不动。所有字号、行高、字距不变，只换字族。
- **V3 圆形去向开关（第 3 条）**：改成圆形按钮（`rounded-full`），去掉可见的「进周报」三字，只保留勾选态图形；未选中是空心圆环，选中是实心 primary 圆 + 白色对勾。`aria-label`、`title`、`role="switch"`、`aria-checked` 全部保留，鼠标悬停即可看到说明，键盘与读屏仍可操作。宽度收窄后正文左移，条目换行位置随之变化。
- **V4 成果库去搜索（第 4 条）**：删掉 `ArtifactExplorer` 的搜索输入框、`query` 状态与其在 `filtered` 里的过滤分支；工具栏第一行如果只剩时间范围就与第二行合并成一行，避免留一条空行。保留卡片/列表切换、时间范围（非精简模式）与「含子目录」——这些不是搜索。四个复用点一并生效。
- **V5 去掉归档提示（第 5 条）**：删掉 `ArtifactExplorer.tsx` 里渲染 `archiveSkipped` 的那段文案与 `archiveSkipped` prop，`ScenePage.tsx` 不再传。宿主仍解析并返回 `skipped` 计数（供读屏/诊断与既有测试），只是不在界面上提示。

#### 实施步骤

**S01（OD-ARCH-100-S01）**

1. Step ID：OD-ARCH-100-S01
2. 单一动作描述：`BandMetric` 图标去掉底色块，图标放大到 56px 占满原色块位置，列宽与右侧排版不变。
3. 输入来源：`SceneStatusStrip.tsx` 的 `BandMetric` 与 `BAND_TONES` / `BAND_ALERT`。
4. 输出目标：无底色图标座 + 56px 图标；`chip` 字段只保留文字色。
5. 验收标准：截图上图标看不到浅色圆角底块；图标明显大于现在的 28px；四张卡图标尺寸一致、等高关系不变；写作页 `WritingMetric` 零改动。

**S02（OD-ARCH-100-S02）**

1. Step ID：OD-ARCH-100-S02
2. 单一动作描述：安装 `@fontsource-variable/inter` 与 `@fontsource/ibm-plex-mono`，把 `--font-*` 令牌、`.mono` 类与 `main.tsx` 的字体引入换成新字族。
3. 输入来源：`theme.css` 的 `--font-*`、`styles.css` 的 `.mono`、`main.tsx` 的 `@fontsource` 引入、`package.json`。
4. 输出目标：Inter（正文/标题）+ IBM Plex Mono（序号/标签）+ 原 CJK 回退栈；两个字体包进 `package.json`，仍是本地资源。
5. 验收标准：`npm run build` 通过且字体文件被产出到 `dist/assets`；界面截图里英文小字比改动前更清楚；字号与行高不变，无布局溢出；不新增任何外链请求。

**S03（OD-ARCH-100-S03）**

1. Step ID：OD-ARCH-100-S03
2. 单一动作描述：`DestinationSwitch` 改为圆形按钮，删掉可见的「进周报」文字，保留悬停提示与无障碍语义。
3. 输入来源：`WeeklyTimeline.tsx` 的 `DestinationSwitch`。
4. 输出目标：圆形勾选按钮（空心/实心两态），`rounded-full`、无可见文案、`title` + `aria-label` 保留，失败原因仍就地显示。
5. 验收标准：截图上是圆形而不是方角；悬停出现「已进周报 / 不进周报」说明；键盘 Space/Enter 可切换、焦点环可见；筛选状态下点选仍对到正确条目；无去向条目仍是静态标记。

**S04（OD-ARCH-100-S04）**

1. Step ID：OD-ARCH-100-S04
2. 单一动作描述：删掉成果库搜索框、`query` 状态与相关过滤分支，工具栏合并为紧凑布局。
3. 输入来源：`ArtifactExplorer.tsx:298–380` 与 `filtered` 计算。
4. 输出目标：无搜索输入框；卡片/列表、时间范围、含子目录仍在；`filtered` 只按范围与 `includeSubdirs` 处理。
5. 验收标准：界面上找不到搜索框与「搜索成果或归档」字样；四个复用页都正常；列表数量与改动前（清空搜索时）一致；分页、标题读取、点击预览不受影响。

**S05（OD-ARCH-100-S05）**

1. Step ID：OD-ARCH-100-S05
2. 单一动作描述：删掉「另有 N 个归档文件名不符合约定…」提示与对应 prop。
3. 输入来源：`ArtifactExplorer.tsx:384–388`、`ScenePage.tsx:862`。
4. 输出目标：该文案不再渲染；`archiveSkipped` prop 移除；宿主侧 `skipped` 计数与归档读取逻辑不变。
5. 验收标准：界面与 AX 树里都找不到该文案；归档条目仍正常列出；`verify_weekly_093.py` 的归档相关断言（`archive_*`）不受影响。

**S06（OD-ARCH-100-S06）**

1. Step ID：OD-ARCH-100-S06
2. 单一动作描述：同步修订冲突的既有断言，新增 `verify_weekly_100.py`，跑全量回归与窗口截图，落盘证据并更新状态/审查记录。
3. 输入来源：`verify_weekly_093.py:192/292`、`verify_weekly_099.py`、`preview_weekly_099.py`。
4. 输出目标：新增静态断言（图标无底色块、图标 56px、Inter/IBM Plex Mono 生效、圆形开关且无可见「进周报」、无搜索框、无归档提示）；`evidence/od-arch-100/` 截图与 facts.json；STATUS.md 与 REVIEW.md 更新。
5. 验收标准：`tsc --noEmit`、`npm run build`、`cargo test --offline`、`verify_weekly_091/092/093/097/099/100`、`verify_worklog_089`、`verify_writing_094` 全绿；窗口截图确认五处都改到位；`runtime/` 零差异。

**依赖关系：** S01–S05 相互独立；S01–S05 → S06。

**影响范围：** `desktop/src/shell/SceneStatusStrip.tsx`、`WeeklyTimeline.tsx`、`ArtifactExplorer.tsx`、`ScenePage.tsx`、`desktop/src/theme.css`、`desktop/src/styles.css`、`desktop/src/main.tsx`、`desktop/package.json`（+ lock）、`tests/verification/desktop/verify_weekly_0{93,99,100}.py`、新增 `preview_weekly_100.py`、`PLANS.md` / `STATUS.md` / `REVIEW.md`。

**不做的事：** 不改任何数据文件与去向取值；不改周报取数口径；不删顶部导航的会话内搜索（用户说那是保留的那一个）；不改 `WritingMetric`；不动成果库的卡片/列表、时间范围与「含子目录」；不碰 `runtime/`；不读真实工作记录或凭证。

**第 2 条的执行取舍：** 已按「英文正文/标题换 Inter（variable，300–700 全档）、序号与等宽小标换 IBM Plex Mono」实施，两个字体包都由 `@fontsource` 自托管，运行时零外链；若之后要全部统一成 Inter，只改 `--font-label-caps` 与 `.mono` 两处即可。

**设计借鉴（OD-ARCH-035 要求）：** 已检索并实际查看 Baremetrics（指标卡的图标直接作色相符号、不套底板）、Linear（勾选控件用一个圆点表达开合，说明只在悬停时出现）、Notion（列表工具栏一行到底，重复的检索入口只留全局一处）三组同类界面。落点：①图标去掉色块后靠自身笔画的色相与体积承担识别，卡内不再多一层装饰；②圆形单点开关与时间线的圆点节奏对齐，说明退到 `title` 与读屏语义；③成果库工具栏合并成一行，搜索只保留顶部导航那一个入口。

**实施结果（2026-10-28）：** 五处改动与 S06 的断言同步、回归、窗口证据全部完成；`tsc --noEmit`、`npm run build`、`cargo test --offline`、`verify_weekly_091/092/093/097/099/100`、`verify_worklog_089`、`verify_writing_094` 全绿，`preview_weekly_100.py` 窗口读数符合预期。细节见 STATUS.md 的 OD-ARCH-100 章节。

### OD-ARCH-099：周报页四项微调——指标卡图标列、去向点选、成果卡序号、预览居中（已批准，已实施，2026-10-28）

> 分类：界面微调 + 一处受控的数据写入 + 新宿主命令。用户本轮提四点：①顶部四张指标卡图标放大、独占左侧、图标下不再有内容，文字整体右移；②时间线的「本周报引用」改成圆形点选按钮，选中即进周报、不选即不进；③右侧成果库卡片的 01–05 序号居中放大，序号区底色比正文区深，文件名只显示单行；④点成果文件名弹出的预览卡改为居中（现贴在窗口底部）。
> **状态：用户已批准；S01–S05 全部实施完毕，本人实跑验证与窗口证据见 STATUS.md，审查结论见 REVIEW.md 顶部。**

#### 第 2 条的语义（用户 2026-10-28 明确澄清，以此为准）

点选**不写周报文档**。它的作用只有两件：一是改这条记录在周文件里的「去向」字段，使顶部第 2 张卡「进周报条目」的统计随之增减；二是让这条记录在用户说「整理成周报」时进入模型的上下文——周报 Skill 取数口径本来就是「只取去向为本周报引用 的条目」。因此选中 = 该条被统计并带入周报上下文，取消 = 两者都不发生。周报正文仍由用户确认后单独落盘，本轮不碰。

#### 已查清的事实（只读排查，2026-10-28）

1. **指标卡**在 `SceneStatusStrip.tsx` 的 `BandMetric`（约 140–262 行）：现在是「36px 图标座 + 标题/副标题」同一行，右侧数字，底行提示 + 进度轨。weekly 四张卡在 467–540 行调用。写作页用独立的 `WritingMetric`，本轮改动不牵连写作页。
2. **去向标签**在 `WeeklyTimeline.tsx:196–214`，只渲染成只读胶囊，没有任何交互。解析在 `weekTimeline.ts`（`DEST_LINE`，字段 `destination: string | null`）；统计在 `weeklyQuadrants.ts:classifyWorklog`（`destination === "本周报引用"` 进「进周报」两格），指标带第 2 卡由 `ScenePage.tsx:478` 从这两格求和得来，因此改「去向」会自然带动统计，不需要另做一套计数。
3. **落盘契约**是 `worklog.rs::append_entry`：只追加（`append(true)`）、写前读旧文、写后校验「旧字节一字不变 + 长度等于旧长 + 新增长度」，任何异常即报错。**没有任何改既有行的通道。** 因此第 2 条必须新增一个显式的「改去向」命令，且要有自己的自证，不能借用追加的自证。
4. **去向取值**是 `worklog.rs::DESTINATIONS = [本周报引用, 知识卡, inbox, 归档]`，写入时不在表内即回退默认值 `本周报引用`；`skills/memory-agent/SKILL.md:29` 也把四值写死。取消选中若沿用现有取值，只能落成「知识卡 / inbox / 归档」之一——三者都不是用户的意思。
5. **成果卡**在 `ArtifactExplorer.tsx:392–478`：序号是 `w-9 text-[15px]` 左对齐，与标题同底色，标题是 `line-clamp-2`。归档行走同一个卡片分支（`:400` 起）。列表视图是另一套结构，本轮不动。
6. **预览浮层**在 `SourcePeekSheet.tsx`：外层 `fixed inset-0 flex items-end justify-center … pt-6`（第 53 行），内卡 `max-h-[70vh] max-w-4xl rounded-none border-b-0`（66–69 行）——贴底是外层 `items-end` 加内卡去掉下边框共同造成的。
7. **会受影响的既有断言**：`verify_weekly_093.py` 的 `band_larger_icon`（`w-9 h-9 rounded-xl`）、`band_card_is_horizontal`、`band_card_compact_height`；`verify_weekly_097.py` 的 `sheet_fixed_overlay`（断言 `items-end`）与 `destination_before_text`。这些必须同步修订，不能只改界面。

#### 方案

- **R1 指标卡（第 1 条）**：`BandMetric` 改成两列结构——左侧固定 56px 的图标列（图标座放大到 56×56、水平居中、上对齐，列内下方留空），右侧一列自上而下放标题、英文副标题 + 序号、底行提示与进度轨，数字仍在右侧同行末端的顶部对齐区。四张卡保持等高（`items-stretch`），窄窗两列 / 手机单列时不清零左侧留白。图标只放图标，其下不叠任何文字。
- **R2 去向点选（第 2 条）**：
  - 新增宿主命令 `worklog_set_destination(week, date, index, destination, expect_text)`。实现放在 `worklog.rs`，只做一件事：在周文件里把**目标条目的那一行** `**去向**：` 换成新值。定位依据是「日期段（含（续）段）里第 N 个 `- **事**` 条目」，并用 `expect_text` 校验该条目正文与界面读到的一致；任一处不吻合（找不到、有多条候选、正文不符、该条目没有去向行）一律**拒绝并原样返回原因，不写文件**。
  - 写前把它复制成 `YYYY-Www.md.bak-<epoch>`；写入走同目录临时文件 + fsync + rename；写后自证：行数不变、逐行比对只允许目标行不同、该行前缀与缩进不变、新值等于请求值；任一不符则用备份回滚并报错。CRLF 文件保留原换行。
  - 新增去向取值 **`仅记录`**（追加到 `DESTINATIONS` 末尾）：取消选中写 `仅记录`，意思是「留在本周记录里，但不进周报」。不借用 `inbox / 知识卡 / 归档`，因为那三个都是用户明确归档到别处的意思，替用户做这个决定不诚实。`skills/memory-agent/SKILL.md` 同步补一句「界面上可以点选切换，模型仍只写默认的 本周报引用」。
  - 界面：`WeeklyTimeline` 把去向胶囊换成方形点选按钮（`role="switch"`、`aria-checked`、可见焦点环、hover 与选中态沿用 primary 紫色体系、方角，与提示条/预览卡同一套语言），右侧保留文字说明「进周报 / 不进周报」。点击调命令 → 成功后由 `ScenePage` 重新读当周文件（统计、四象限、时间线三处同时更新，不各算一套）→ 失败时在卡片内显示可读原因，并把勾选状态回滚到服务端真实值。**没有去向行的旧条目**显示静态「无去向」标记、不提供点选（避免为它插入新行改变既有结构），悬浮提示说明可在原文补。
- **R3 成果卡（第 3 条）**：卡片改成两段底色——左侧序号块 48×48、方角（`rounded-[8px]`，与四象限一致）、`bg-surface-container-high/70` 这类比正文区更深的既有 token、序号 22px 等宽居中；右侧正文区保持现在的浅底与标题/日期排版。标题由 `line-clamp-2` 改为单行 `truncate`（完整标题仍在 `title` 悬浮里）；归档卡走同一套。列表视图不动。
- **R4 预览居中（第 4 条）**：`SourcePeekSheet` 外层由 `items-end` 改 `items-center` 并留上下安全边距（`py-8`），内卡 `max-h-[80vh] w-full max-w-3xl`、四边补齐边框、方角保留、阴影改双向；正文仍独立滚动。Esc、关闭按钮、点遮罩、焦点归还全部保留。

#### 实施步骤

**S01（OD-ARCH-099-S01）**

1. Step ID：OD-ARCH-099-S01
2. 单一动作描述：`BandMetric` 改为「左侧独占图标列 + 右侧内容列」，图标放大、图标下留空，文字与数字右移；四卡等高。
3. 输入来源：`SceneStatusStrip.tsx` 的 `BandMetric` 与 weekly 四张卡的调用；参考图 Baremetrics 指标卡的「标识一列、数值一列」层次。
4. 输出目标：新的两列结构与尺寸常量；`WritingMetric` 与其它场景的 `grid` 分支零改动。
5. 验收标准：四卡图标尺寸一致且明显大于现在；图标列内除图标外无任何文字；宽窗四列、1000px 两列、手机单列都不溢出、不塌陷、卡高一致；数字与卡右缘的对齐关系不变。

**S02（OD-ARCH-099-S02）**

1. Step ID：OD-ARCH-099-S02
2. 单一动作描述：在 `worklog.rs` 实现 `set_destination`（单行受控改写 + 备份 + 写后自证 + 失败回滚），并新增 `仅记录` 取值；在 `lib.rs` 注册 `worklog_set_destination` 命令，`api.ts` 加同名方法，`types.ts` 补回执类型。
3. 输入来源：`append_entry` 的既有自证写法、`model_settings::write_atomic`、`capabilities.rs` 的 `.bak-<epoch>` 约定。
4. 输出目标：新函数 + 新命令 + 前端方法；Rust 单测覆盖「正常切换 / 同日多条只改目标那条 / 正文不符拒绝 / 日期不存在拒绝 / 无去向行拒绝 / 备份内容等于原文 / 换行与缩进保持 / 未知去向回退」。
5. 验收标准：成功路径下除目标行外全文件逐字节相同、行数不变；所有失败路径都不留半截写入（断言文件字节与调用前一致）；`append_entry` 的只追加契约与其既有测试零改动；不读凭证、不碰 `runtime/`。

**S03（OD-ARCH-099-S03）**

1. Step ID：OD-ARCH-099-S03
2. 单一动作描述：时间线去向胶囊换成方形点选开关，接线到 S02 的命令，成功后由 `ScenePage` 重读当周文件。
3. 输入来源：`WeeklyTimeline.tsx:196–214`、`ScenePage.tsx` 的 `weekContent` 状态与 `worklogReadWeek` 调用、S02 的回执。
4. 输出目标：可点选的开关（选中 = 进周报，未选 = 仅记录）、逐条的加载态与失败原因、无去向条的静态标记；统计卡与四象限随重读更新。
5. 验收标准：点选后第 2 张卡数字与四象限计数同步变化，且只变这一条；取消后回到未选中并写入 `仅记录`；失败时勾选回滚且显示原因；键盘可操作（Space/Enter）且有无障碍状态；筛选状态下点选不错位到别的条目。

**S04（OD-ARCH-099-S04）**

1. Step ID：OD-ARCH-099-S04
2. 单一动作描述：成果卡序号块居中放大、加深底色，标题改单行；预览浮层改居中。
3. 输入来源：`ArtifactExplorer.tsx:392–478`（卡片与归档两分支）、`SourcePeekSheet.tsx:53–69`。
4. 输出目标：序号块 48×48 方角、22px 居中、深一档底色；正文区不变；标题单行截断；浮层 `items-center` + 四边边框 + `max-h-[80vh]`。
5. 验收标准：序号在块内水平垂直都居中、字号明显大于现在；序号区底色肉眼可辨地深于正文区；标题只占一行，完整文字仍在悬浮提示；浮层在 1600×980 与 1000px 窄窗下都居中且不触边，长文仍独立滚动，三种关闭方式与焦点归还都还在。

**S05（OD-ARCH-099-S05）**

1. Step ID：OD-ARCH-099-S05
2. 单一动作描述：同步修订受影响的静态断言，新增 `verify_weekly_099.py`，跑全量回归与窗口预览，落盘证据。
3. 输入来源：`verify_weekly_093.py`（指标卡三条）、`verify_weekly_097.py`（浮层一条、去向一条）、`preview_weekly_093*.py`。
4. 输出目标：新增 `verify_weekly_099.py`（图标列独占、图标下无内容、开关语义与无障碍属性、仅记录取值、序号块尺寸与底色、标题单行、浮层居中、Rust 命令注册）；`evidence/od-arch-099/` 的截图与 facts.json；STATUS.md 执行记录。
5. 验收标准：`tsc --noEmit`、`npm run build`、`cargo test --offline`（含新增单测）、`verify_weekly_091/092/093/097/099`、`verify_worklog_089`、`verify_writing_094` 全绿；窗口预览验证居中浮层与被放大后的指标卡；`runtime/` 零差异；只用 `/tmp` 合成数据。

**依赖关系：** S01、S04 相互独立；S02 → S03；S01–S04 → S05。

**影响范围：** `desktop/src/shell/SceneStatusStrip.tsx`、`WeeklyTimeline.tsx`、`ArtifactExplorer.tsx`、`SourcePeekSheet.tsx`、`ScenePage.tsx`、`desktop/src-tauri/src/worklog.rs`、`lib.rs`、`desktop/src/api.ts`、`types.ts`、`skills/memory-agent/SKILL.md`（一句取值说明）、`tests/verification/desktop/verify_weekly_0{93,97,99}.py`、`PLANS.md` / `STATUS.md` / `REVIEW.md`。

**不做的事：** 不往周报文档写任何内容；不改周报 Skill 的取数口径（仍是「只取 本周报引用」）；不改 `append_entry` 的只追加契约；不改四象限归类规则；不改成果库列表视图；不改 `WritingMetric`；不碰 `runtime/`；不读真实工作记录或凭证（窗口与单测一律 `/tmp` 合成数据）。

**需要在批准时确认的一处取舍：** 取消选中写 `仅记录`（新增第五个去向值，语义为「留在周记里但不进周报」）。备选是写 `inbox`（不改取值表，但等于替用户选了他没选的目的地）。我按 `仅记录` 出计划；若你更想要 `inbox`，请批准时指明。

**设计借鉴（OD-ARCH-035 要求）：** 已检索并在本轮实际查看三组同类界面——Baremetrics 的指标卡（左侧一列标识、右侧数值，卡内不再堆第二段文字）、Crowdin 的会话列表（方形选择控件贴着每一条）、Station 的文档列表与居中提示层（列表左列序号/图标、内容单行；弹层浮在窗口中央而不是贴边）。三点落点：①图标独占一列后，卡内左侧不再有二级文字，扫读只横向一次；②选择控件用方角开关而不是胶囊标签，状态一眼可辨且有文字说明；③居中浮层与「序号块 + 单行标题」让右栏卡片更像索引而不是段落。

**预计规模：** 中等批次，五个步骤，本助手亲自实施。


### OD-ARCH-098：项目跟进页两栏改版——项目卡片列表 + 甘特图（已批准实施，2026-10-28，审查通过）

> 分类：产品设计 + 界面实现 + 一处向后兼容的数据格式扩展。用户提供参考图（Task tracking dashboard：左侧项目卡列表 + 右侧甘特时间线），要求项目跟进页改为两栏：左侧搜索栏与项目卡片，右侧项目甘特图；功能与 UI 由本仓库按参考图版式细节与 OmniDesk 统一视觉自行设计。
> 设计借鉴（screenshots-design-inspiration MCP，2026-10-28，实际查看 Flow PM 系列 PM/TC/资源管理截图）：Flow 用「左列实体卡 + 右侧时间线」把导航与执行视图分开，卡片只承载状态要素、时间线按行对齐条目并突出今日；弹出详情挂在时间线条目上而不另开页面。落点：左栏卡片 = 确定性汇总的事项条目（事项/负责人/期限/状态），右栏甘特 = 同一批条目的时间视图，两侧点击互相联动；配色沿用本仓库 status 语义色 token，不引入参考图的珊瑚色新色值。

#### 已查清的事实（只读排查，2026-10-28）

1. **数据源**：项目页进度走既有确定性命令 `task_project_summary`（`summarize_project`，task_shell.rs:1559），输入是首页最近一条用户消息的「事项｜负责人代号｜期限｜状态｜下一步」行，输出 `ProjectSummary{items, total, done, …}`。**事项条目只有期限（due）一个日期，没有开始日期，也没有进度百分比**；`overdue` 由宿主按当天日期判定。
2. **现状 UI**：project 分支仍是旧版页签卡（进度 = markdown 文本 `pre`，阶段成果 = ArtifactExplorer），与 092/093/094 已改版的 weekly/writing 形成明显落差。
3. **参考图要素与现实的差距**：卡片上的「进度 30%」在现有数据里没有来源——事项行不含进度字段；甘特条目需要起止区间，现有事项只有期限。两条都只能诚实处理：状态驱动配色而不虚构百分比；区间缺开始日期时按「里程碑」单日标记呈现，不猜一段假工期。

#### 改版方案

- **R1 两栏布局**：project 分支替换为 `grid-cols-[340px_1fr]`（窄窗单列，顺序 左列表 → 甘特图），沿用 093 的卡片体系（rounded-2xl 白卡、同阴影、英文小字幕 PROJECTS / TIMELINE）。指标带（SceneStatusStrip project 分支）保持不动。
- **R2 左栏：搜索 + 项目卡列表**。数据 = `task_project_summary` 返回的 items（不调模型）。顶部搜索框按「事项 / 负责人」过滤（只改视图）。每张卡：事项标题、状态徽标（未开始 / 进行中 / 受阻 / 已完成，语义色）、负责人代号、期限日期、下一步一行截断、逾期红标。卡下方一条**状态色带**（颜色 = 状态语义，不标百分比数字——现有数据无进度来源，不虚构 30% 这类数字；未来事项行若新增可选进度字段再升级，另行计划）。点击卡片 = 在右栏甘特中高亮对应条目并滚动到位。
- **R3 右栏：甘特时间线**。纯前端组件 `ProjectGantt.tsx` + 纯函数 `projectGantt.ts`：
  - 时间窗：取全部条目最早开始/期限与最晚期限，向两侧各留 2 天；条目超出视窗可横向滚动；顶部日期轴（日粒度，超 30 天自动切周粒度），今日竖线常显。
  - 条目条：有开始日期的画区间条，颜色按状态；只有期限的画**菱形里程碑**标记（不虚构区间）；逾期条目加红色描边或角标。
  - 行对齐：每条事项一行，行高固定，与左栏卡片一一对应；空数据/汇总失败显示与现有口径一致的空态文案。
  - 交互：条目 hover 显示标题 + 期限浮层；与左栏选中态双向联动；不承载编辑（事项在首页对话里写，页面只读，OD-ARCH-057 原则不变）。
- **R4 数据格式扩展（向后兼容）**：`summarize_project` 每行支持可选第 4 列「开始日期」：`事项｜负责人｜期限｜开始｜状态｜下一步`（6 列）；既有 5 列行完全不变，缺开始日期 = 里程碑呈现。Rust 侧解析 + 单测覆盖 5/6 列、非法日期、混合输入；契约文档同步。
- **R5 阶段成果入口保留**：原「阶段成果」ArtifactExplorer 不删除，收进右栏甘特卡头的一个次级切换（甘特 / 成果），避免两栏改版把既有能力藏没。

#### 实施步骤

**S01（OD-ARCH-098-S01）**

1. Step ID：OD-ARCH-098-S01
2. 单一动作描述：扩展 `summarize_project` 支持可选「开始日期」列（5 列向后兼容 6 列），`ProjectItem` 增加可选 `start` 字段；补齐 Rust 单测并同步契约注释。
3. 输入来源：task_shell.rs 汇总实现与既有测试；OD-ARCH-026 场景规则文档。
4. 输出目标：Rust 解析与单测；`ProjectItem.start: Option<String>` 序列化；types.ts 同步字段。
5. 验收标准：5 列旧行为逐字节不变；6 列合法/非法日期、混合行均有确定性结果与测试；不调模型、不读时钟（today 仍由调用方传入）。

**S02（OD-ARCH-098-S02）**

1. Step ID：OD-ARCH-098-S02
2. 单一动作描述：实现 `projectGantt.ts` 纯函数（时间窗计算、日/周轴刻度、条目定位、里程碑判定、逾期标记）。
3. 输入来源：S01 的 items 结构；writingStudio.ts 的纯函数边界先例。
4. 输出目标：projectGantt.ts + 单测（node --test 模式，参照 knowledgeGraph.test.mjs）。
5. 验收标准：空列表、单条、跨度 60 天、未来条目、逾期条目均为确定性输出；不依赖真实时钟（today 注入）。

**S03（OD-ARCH-098-S03）**

1. Step ID：OD-ARCH-098-S03
2. 单一动作描述：project 分支改两栏：左栏搜索 + 卡片列表（R2），右栏 ProjectGantt（R3）+ 成果切换（R5）。
3. 输入来源：S01/S02；ScenePage project 分支现状；status 语义色 token。
4. 输出目标：两栏渲染与双向联动；窄窗单列顺序 左列表 → 甘特；错误/空态如实显示。
5. 验收标准：无进度百分比值出现在界面；里程碑与区间条视觉可区分；选中联动只改视图；weekly/writing 布局零回归。

**S04（OD-ARCH-098-S04）**

1. Step ID：OD-ARCH-098-S04
2. 单一动作描述：新增静态断言 `verify_project_098.py` 与窗口验证 `preview_project_098.py`，执行全量回归并落盘证据。
3. 输入来源：S01–S03 差异；tests/verification/desktop 既有脚本与一次性 Xvfb 模式。
4. 输出目标：`evidence/od-arch-098/`（两栏、搜索过滤、联动高亮、里程碑 vs 区间、逾期标记、窄窗截图与 facts.json）；STATUS.md 执行记录。
5. 验收标准：窗口断言覆盖卡片条数 = items 总数、搜索过滤、甘特今日线存在、5 列旧输入的里程碑渲染；回归 091/092/093/094、tsc、build、pytest、cargo 全过；`runtime/` 零差异；只用 /tmp 合成夹具。

**S05（OD-ARCH-098-S05）**

1. Step ID：OD-ARCH-098-S05
2. 单一动作描述：完成实施记录，等待 ARCHITECT 独立验收。
3. 输入来源：S01–S04 差异与测试输出。
4. 输出目标：STATUS.md 执行记录；REVIEW.md 结论另行写入。
5. 验收标准：Git 状态如实记录；既有未提交改动全部保留；不提交、不推送。

**依赖关系：** S01 → S02 → S03 → S04 → S05；S01 的 6 列扩展独立可测，先行。

**关键取舍：** ① 不虚构进度百分比——参考图卡片的「30%」在现有数据里没有来源，状态色带只表达状态；要真进度需扩展事项行格式，属另一项计划。② 甘特区间诚实呈现——没有开始日期的事项画里程碑，不猜工期；开始日期作为可选第 4 列向后兼容加入。③ 卡片实体 = 汇总事项条目，不自造「项目」对象（ProjectRail 的引擎项目与本页无关，不混用）。④ 不做条目编辑、拖拽改期、负责人头像/多人协作——事项在首页对话产生，页面保持只读；参考图 these 要素依赖多人数据，本产品无此数据源。⑤ 甘特不自绘新色值，状态色全部映射既有语义 token，与 092–094 视觉体系统一。

**阶段门槛：** 用户已批准（2026-10-28）并实施完毕；批准未放宽范围。审查结论见 REVIEW.md 顶部。范围外需求（进度字段、条目编辑、多人协作）另行立项。

### OD-ARCH-097：周报页四象限重绘、时间线瘦身与原文浮层（已批准实施，2026-10-28，审查通过）

> 分类：界面改版 + 版式取舍。用户拿一张效率看板参考图（`/mnt/data/Desktop/zhoubao.png`）提五点：①四象限做成框外有 URGENT/IMPORTANT 轴标的矩阵，每格最多两条；②去掉格内说明文字与统计数字；③时间线里删掉「转周报」文案与链接，「本周报引用」标签挪到条目左侧；④「打开本周原文」不再占用页面底部，改成弹出卡片；⑤成果库只显示标题与时间，不重复显示文件名。
> **现状为只读排查，未改任何源码。** 参考图只作版式参考，不照搬其配色与内容。

#### 已查清的事实（只读排查，2026-10-28）

1. **四象限**在 `desktop/src/shell/WeeklyQuadrants.tsx`。当前格内从上到下是：序号 + 中文标题 + 计数、一行动作说明（`q.action`，如「已过日期 · 进周报」）、最多两条预览（`slice(0, 2)`）与「还有 N 条」。格内没有轴标；轴语义只写在 `weeklyQuadrants.ts` 的 `QUADRANTS[].action` 文案里。`counts` 与 `unclassified` 也被卡片头部统计使用。
2. **时间线条目**在 `WeeklyTimeline.tsx:196-227`：正文 + 右侧去向胶囊（`entry.destination`，值来自记录文件的「去向」字段，含「本周报引用」）+ 悬停出现的「转周报」按钮。`转周报` 文案全仓仅此一处界面使用（另一处是 `ScenePage.tsx:369` 的注释）。
3. **「本周记录」进度条**：用户所指的「进度条中文字」实际指第 2 张指标卡「进周报条目」底部那行 `占全部记录的 N%` 及其进度轨（`SceneStatusStrip.tsx` 的 `BandMetric`），配合时间线条目的「本周报引用」胶囊。这两处是本轮第 3 点的实际落点。
4. **原文查看**在 `ScenePage.tsx:881-896`：`opened` 状态渲染成页面流里的一块 `<section>`（`mt-2`，跟在主区后面），`max-h-[40vh]` 内部滚动。它挤压页面高度，与用户「不要占用周报 UI 空间」冲突。`opened` 还被成果文件、周报文件的预览复用（`:286`、`:346`）。
5. **成果库**在 `ArtifactExplorer.tsx`：卡片视图渲染「标题」+ 第二行「文件名 · 日期 · 字数」，列表视图渲染「标题」+ 第二行文件名 + 右侧日期 + 字节数。标题取 frontmatter `title`，缺失时回退 `file_name`，所以文件名会在标题与第二行重复出现。归档行同理（`周标签 + kindLabel` 与 `file_name` 并存）。
6. **既有校验**：`tests/verification/desktop/verify_weekly_093.py` 断言 `quadrant_preview_single_line`（`truncate opacity-85` 与 `title={i.text}`）与三栏结构；`preview_weekly_093.py` / `preview_weekly_093_s10.py` 是真实窗口预览脚本。本轮改动会触及这两类断言，必须同步修订，不能只改界面。

#### 实施步骤

**S01（OD-ARCH-097-S01）**

1. Step ID：OD-ARCH-097-S01
2. 单一动作描述：把四象限改成「框外轴标」的矩阵，格子内只留标题与至多两条条目。
3. 输入来源：`WeeklyQuadrants.tsx`（结构）、`weeklyQuadrants.ts` 的 `QUADRANTS`（顺序与 key）、参考图 `zhoubao.png` 的轴标位置。
4. 输出目标：
   - 在 2×2 网格外新增一圈轴标：上方 `URGENT` / `NOT URGENT`，左侧 `IMPORTANT` / `NOT IMPORTANT`（竖排），中文语义在 `title` 与 `aria-label` 里保留，不因英文标签丢失可读性。
   - 格内删除 `q.action` 说明行与右上角计数；保留格序号（1–4）与中文标题，条目仍最多 2 条、超出给「还有 N 条」。
   - `QUADRANTS[].action` 字段保留（`title` 悬浮提示与既有测试仍引用），但不再渲染到格内。
5. 验收标准：格内无「已过日期 · 进周报」类说明文字、无统计数字；每格条目 ≤ 2；四格与轴标对齐，1936 宽与 1000 窄窗下都不溢出、不重叠；点击筛选与取消筛选行为不变。

**S02（OD-ARCH-097-S02）**

1. Step ID：OD-ARCH-097-S02
2. 单一动作描述：从时间线条目里移除「转周报」按钮，并把去向标签挪到正文左侧。
3. 输入来源：`WeeklyTimeline.tsx:196-227` 的条目结构；`ScenePage.tsx` 传入的 `onRequestReport`。
4. 输出目标：
   - 删除条目右下的「转周报」按钮及其文案，条目交互只保留点击查看原文/筛选（与现状一致）。
   - 把 `entry.destination` 胶囊从正文右侧移到正文左侧行首，做成窄标签；无去向的条目不留空槽（左侧直接起正文）。
   - `onRequestReport` 仍保留给卡片头的周报动作行与空状态按钮使用，不从 `ScenePage` 删除接线。
5. 验收标准：时间线里全仓 grep 不到界面上的「转周报」；「本周报引用」等去向标签出现在正文左侧且不挤压正文换行；卡片头「去生成周报 / 重新生成」与空状态入口不受影响。

**S03（OD-ARCH-097-S03）**

1. Step ID：OD-ARCH-097-S03
2. 单一动作描述：把「打开本周原文」与周报/成果预览从页面流改为底部弹出卡片。
3. 输入来源：`ScenePage.tsx:881-896` 的 `opened` 区块；`SceneStatusStrip.tsx:654-697` 的浮层写法（portal 式 fixed 遮盖 + 方角内卡 + Esc 关闭）。
4. 输出目标：
   - 新增 `SourcePeekSheet`（`desktop/src/shell/SourcePeekSheet.tsx`）：`fixed inset-0` 半透明遮罩，卡片贴底（居中的宽卡片、最大高度约 70vh、正文独立滚动），右上角关闭按钮 + `Esc` 关闭 + 点遮罩关闭；`role="dialog"`、`aria-modal`、`aria-label` 用文件名。
   - `ScenePage.tsx` 改为渲染该浮层，删掉页面流里的 `opened` 的 `<section>`；`opened` 状态与 `setOpened` 的调用点保持不变，四个预览来源（原文 / 周报 / 成果 / 失败提示）都走同一浮层。
   - 浮层不改变主区高度：打开前后页面滚动位置与三栏高度一致。
5. 验收标准：点「打开本周原文」出现底部卡片，三栏不再被推低；`Esc`、关闭按钮、点遮罩三种方式都能关闭；`tests/verification/desktop/preview_weekly_093*.py` 与新校验能定位到该浮层。

**S04（OD-ARCH-097-S04）**

1. Step ID：OD-ARCH-097-S04
2. 单一动作描述：成果库卡片与列表只显示标题和时间，不再重复文件名。
3. 输入来源：`ArtifactExplorer.tsx` 的卡片视图（`:430-470` 区间）与列表视图（`:480-520` 区间）、归档行。
4. 输出目标：
   - 卡片：主行标题（无 title 时回退文件名，此时第二行不再重复同名）、次行只保留日期；`showTypeLabels`（写作页）与 `stalePaths` 标记继续保留，字数与字节数移出可见文本。
   - 列表：单行标题 + 右侧日期，删除第二行文件名与字节数；归档行同理，只留「周标签 + 类型」与日期。
   - 保持 `title={相对路径}` 悬浮提示，路径信息不丢，只是不占版面。
   - 归档与成果混排、分页、搜索、含子目录开关行为不变。
5. 验收标准：卡片与列表都不出现「标题 + 同名文件」重复；每条仍能看到标题与日期；搜索仍能按文件名命中（搜索用 haystack，不改）；写作页的类型标签与 90 天标记仍在。

**S05（OD-ARCH-097-S05）**

1. Step ID：OD-ARCH-097-S05
2. 单一动作描述：同步修订静态校验与真实窗口预览，补齐本轮证据。
3. 输入来源：`verify_weekly_093.py` 的四象限/三栏断言、`preview_weekly_093.py` 与 `preview_weekly_093_s10.py` 的截图流程。
4. 输出目标：新增 `verify_weekly_097.py`（轴标存在且英文常量、格内无 `q.action`/计数渲染、条目 ≤ 2、时间线无「转周报」、去向标签在正文前、浮层组件被 `ScenePage` 渲染且页面流区块已删、成果行不含文件名字段）；更新 `verify_weekly_093.py` 中与本轮冲突的断言；窗口预览增补 `10-source-sheet.png`、`11-quadrant-axes.png`、`12-artifact-rows.png` 与 AX 快照。
5. 验收标准：`verify_weekly_093.py` 与 `verify_weekly_097.py` 全绿；`tsc --noEmit`、`npm run build`、`cargo test --offline` 通过；窗口预览 facts 显示轴标、浮层、瘦身后的条目行；`runtime/` 零差异。

**依赖关系：** S01、S02、S04 相互独立；S03 独立；S01–S04 → S05。

**影响范围：** `desktop/src/shell/WeeklyQuadrants.tsx`、`WeeklyTimeline.tsx`、`SourcePeekSheet.tsx`（新增）、`ScenePage.tsx`、`ArtifactExplorer.tsx`、`tests/verification/desktop/verify_weekly_093.py`、`verify_weekly_097.py`（新增）、`preview_weekly_093*.py` 与其证据、PLANS/STATUS/REVIEW。

**不做的事：** 不改 `weeklyQuadrants.ts` 的归类规则（仍是「已过日期 × 进周报」两条可见事实，不臆造重要度评分）；不改四象限筛选与时间线筛选联动逻辑；不改指标带四张卡的数据口径（第 3 点只删时间线条目里的重复入口与去重标签位置）；不改 `runtime/`；不读真实工作记录与凭证；窗口预览继续用 `/tmp` 合成数据；按用户长期偏好，本轮完成后一并提交并推送 GitHub。

**已确认的一处取舍（2026-10-28 按本计划执行）：** 第 3 点「一周进度条里文字的『转周报』」——当前界面里「转周报」只出现在时间条目上，指标带第 2 张卡底部是「占全部记录的 N%」。本计划按「删掉时间条目的转周报，并把去重标签移到左侧；指标带那行百分比文案保留」执行。若你要连指标带里的百分比说明也一并去掉，请在批准时指明，我把它并入 S02。

**预计规模：** 中等批次，五个步骤，本助手亲自实施。

**设计借鉴（OD-ARCH-035 要求）：** 已检索 `task priority matrix eisenhower grid labels outside cells`（Asana 等项目管理产品）与 `bottom sheet drawer peek card file preview`（ChatGPT、Amazon 等）、`document library list title and date only`（Craft、Condens 等）。三点借鉴：①轴标放在矩阵框外，成为版式骨架而非格内说明，格内因此可以只剩标题与条目；②底部卡片用遮罩 + 贴近底部的宽卡，正文自己滚动，不推动主内容；③文档列表一行一个标题、日期右对齐，文件名退到 hover 提示里，避免与标题重复。


### OD-ARCH-096：构建警告清理与模型切换提示条（已批准实施，2026-10-27 审查通过）

> 分类：构建清洁 + 界面微调。用户本轮提两点：①构建输出里的 `AppendReceipt::failure` dead_code 警告要不要修；②模型切换「成功/失败」那句话现在常驻在输入框下方，改为右下角矮平长方形提示条，60 秒自动关闭、可手动关闭。
> **状态：用户已批准执行（2026-10-27）；S01–S03 已实施并审查通过，结论见 REVIEW.md 顶部。**

#### 已查清的事实（只读排查，2026-10-27）

1. **`AppendReceipt::failure` 是死代码，不是构建错误。** `desktop/src-tauri/src/worklog.rs:54-72` 的该函数全仓零调用（`rg AppendReceipt` 仅命中结构体定义、`impl` 与 `worklog.rs:259` 一处内联构造）。失败回执由 `lib.rs::worklog_append` 的错误分支用 `serde_json::json!({ "ok": false, … })` 内联构造。该函数自 `c1e823f` 起即存在且未被使用；cargo 只报 `warning: associated function … is never used`，不影响编译产物与运行。
2. **提示文案常驻的原因**：`HomeChat.tsx:512` 的 `modelNote` state 只在「下一次切换动作」时被 `setModelNote(null)` 覆盖，渲染在 `:2595` 的输入框下方小字（`text-[10.5px] text-outline`）。因此成功/失败文案会一直挂着，直到用户再操作一次。
3. **`modelNote` 现有 6 个写入点**：空 Profile 提示（`:725`，仅用户主动触发）、成功文案（`:740`）、失败文案（`:748`、`:770`）、端点未识别（`:1564`，仅用户主动触发）、场景思考档自动应用（`:1603`，**自动触发**）。
4. **`SettingsPanel.tsx` 有一套独立的同名 `modelNote`**（`:242`、`:745`、`:1438`），与首页互不影响，本轮不动。

#### 实施步骤

**S01（OD-ARCH-096-S01）**

1. Step ID：OD-ARCH-096-S01
2. 单一动作描述：删除 `AppendReceipt::failure` 死代码，消除该条 `dead_code` 警告。
3. 输入来源：`desktop/src-tauri/src/worklog.rs:54-72`；`lib.rs` 与 `worklog.rs:259` 的既有回执构造方式。
4. 输出目标：删除该 `impl` 块；不改 `AppendReceipt` 字段、不改 `worklog_append` 的 JSON 形状、不改失败分支语义。
5. 验收标准：`cargo test --offline` 仍全绿且**不再出现**该 warning；回执 JSON 字段与现有断言不变（若未来需要统一失败回执口径，另开计划）。

**S02（OD-ARCH-096-S02）**

1. Step ID：OD-ARCH-096-S02
2. 单一动作描述：新增一个可复用的「右下角方角提示条」组件，把模型切换的结果文案从输入框下方的常驻小字迁进去。
3. 输入来源：`HomeChat.tsx` 的 `modelNote` 六处写入点；`theme.css` 现有色彩令牌（`--color-primary`、`--color-error`、`surface-container-lowest` 等）；项目既有方角先例 `.turn-rail-tooltip { border-radius: 0 !important; }`（OD-ARCH-083）与 `CategoryPicker.tsx` 的 `createPortal` 定位写法。
4. 输出目标：
   - 新文件 `desktop/src/shell/StatusToast.tsx`：`createPortal` 挂 `body`，`position: fixed` 右下角，单行矮平长方形，四角直角、细边框 + 浅投影、品牌色左侧细标识；成功用 `secondary`/`primary` 系、失败用 `error` 系；右侧显式关闭按钮（`aria-label="关闭提示"`，键盘可聚焦）。
   - 60 秒后自动关闭；新提示替换旧提示并重置 60 秒计时；可手动关闭。`role="status"`（成功）/ `role="alert"`（失败）。
   - `HomeChat.tsx` 删掉 `:2595` 那行常驻渲染，改为渲染该组件；状态改带级别（`success`/`error`），六处写入点按性质归类。
   - **自动触发不打扰**：`:1603` 场景思考档自动应用不再弹 60 秒横幅（静默，或仅保留在设置/状态行可查），只有用户主动操作才弹条。
   - `sendNote`、`starting`、`skillHint` 相邻块不动。
5. 验收标准：切换成功/失败文案出现在右下角方角长条内、不占输入框位置；60 秒自动消失；点击关闭立即消失；用户不做任何操作时自动触发的场景思考档不再弹条；`npx --no-install tsc --noEmit` 与 `npm run build` 通过。

**S03（OD-ARCH-096-S03）**

1. Step ID：OD-ARCH-096-S03
2. 单一动作描述：同步更新验证脚本并补齐本轮证据。
3. 输入来源：`tests/verification/desktop/verify_model_route_095.py:303`（等待 `重启` 文案）、`:320`（`success_note_present` 断言）、`:321`（`note_after` 收集）。
4. 输出目标：窗口脚本改为断言提示条存在（含关闭按钮）、文案仍在、点击关闭后消失；新增静态断言锁定 60 秒时长与方角样式（`border-radius` 为 0 的类或 CSS）；`evidence/` 增补提示条截图与 AX 快照。
5. 验收标准：`verify_model_route_095.py` 全绿；自动关闭的长延时以静态断言 + 手动关闭的窗口实测共同佐证，并在 STATUS 中如实写明「60 秒自动关闭未在窗口里等满实测」。

**依赖关系：** S01 独立；S02 → S03。

**影响范围：** `desktop/src-tauri/src/worklog.rs`、`desktop/src/shell/StatusToast.tsx`（新增）、`desktop/src/shell/HomeChat.tsx`、`tests/verification/desktop/verify_model_route_095.py` 与其证据、PLANS/STATUS/REVIEW。

**不做的事：** 不改 `SettingsPanel` 的独立提示；不改模型下拉版式与切换逻辑本身；不重构 `sendNote`/`skillHint`；不改 `runtime/`；不读凭证；不因本轮调整场景思考档默认值；不提交、不推送（等用户指示）。

**预计规模：** 一个小批次，本助手亲自实施。

**设计借鉴（OD-ARCH-035 要求）：** 已用 `screenshots-design-inspiration` 检索 `toast notification bottom right corner` 与 Toast 标签（Demio、Additor 等后台产品截图）。两点借鉴：①提示条压在右下角、不挤占主内容与输入区，单行短句即可读完；②方角 + 细边框 + 轻投影，与工作台的卡片体系同源，不用圆角气泡。

**实施记录（2026-10-27）：** 用户批准执行。S01（删除 `AppendReceipt::failure` 死代码）、S02（新增 `StatusToast.tsx` 并接线 `HomeChat.tsx`）、S03（静态校验脚本 + 窗口证据）已全部完成。范围未扩大：未动 `SettingsPanel` 的同名提示、未改模型切换逻辑、未改 `runtime/`。执行详情见 STATUS.md 顶部，审查结论见 REVIEW.md 顶部。

**阶段决定：OD-ARCH-096 审查通过。**

### OD-ARCH-095：模型切换两处报错的修复与诊断可见性（已验收并推送，2026-10-27；提交 `5dce7d4`）

> 分类：功能修复（缺陷 + 可观测性）。用户报告两个真实运行现象：①登录后对话框底部立即显示「切换失败：请选择要使用的 Profile、provider 与模型。」；②手动切换模型显示「切换失败：DSH 配置转储失败（退出码 1）」。
> **现状为只读排查，未改任何源码。** 以下根因由本机真实配置复现得出（脚本运行证据），不是推测。

#### 已复现的根因（只读排查，2026-10-27）

1. **候选枚举漏了实际生效的覆盖文件（导致报错①）。** `desktop/src-tauri/src/model_routes.rs::list_routes` 只把 `dshCli` / `dshHome` / `profiles` 交给脚本，**没有传 `config.patch_files`**；而引擎启动时一定会附加 `~/OmniDesk/app/config/omnidesk-model.yml`。实测同一 Profile：不带补丁转储 → `providers: []`、默认 `deepseek-official/deepseek-flash`；带补丁转储 → 正确列出 `opendesign` 与五个模型。界面因此拿到空候选。
2. **候选为空时前端仍以空 Profile 触发切换（导致报错①的可见文案）。** `desktop/src/shell/HomeChat.tsx` 的场景思考档自动应用（`selectEffort`，约 `:1538`）在 `modelRoutes` 里找不到当前 provider 时执行 `route?.profile ?? ""`，把空串交给 `selectModel`，被 `model_routes.rs:209` 的空值校验拦下。该路径在登录后场景加载时自动触发，所以用户什么都没点就看到报错。
3. **目录分支把 provider 当成 Profile 传下去（导致报错②）。** `desktop/src/shell/ModelPicker.tsx` 在模型目录可用时调用 `onSelectModel(g.id, m.provider, m.id)`，其中 `g.id` 是 **provider 分组 id**（如 `opendesign`），却被 `task_set_model_route` 当作 `source_profile`。实测 `--profile opendesign --dump-config` 退出码 1（stderr：`profile "opendesign" does not exist`）；该文本不含「没有这个 provider」，因此不会走其它 Profile 的回退，直接原样抛给界面。
4. **诊断信息被整条链路丢弃。** `desktop/src-tauri/scripts/model-routes.mjs::dumpConfig` 未捕获 stderr，只留「退出码 N」；宿主 `model_routes.rs::run_script` 又把 node 的 stderr 设为 `Stdio::null()`。同仓 `dump-model-route.mjs:268-290` 已捕获 stderr 并附在错误里，可作参照。

#### 修复范围

**S01（OD-ARCH-095-S01）**

1. Step ID：OD-ARCH-095-S01
2. 单一动作描述：候选枚举带上引擎实际加载的补丁集合，使界面候选与引擎真实路由一致。
3. 输入来源：`model_routes.rs::list_routes` 与 `base_routes` 的现有 payload 构造、`EngineConfig.patch_files`。
4. 输出目标：`list_routes` 传入 `config.patch_files`（含 OmniDesk 覆盖文件）的改动与注释；不改写任何 Profile 文件。
5. 验收标准：同一 Profile 在「有覆盖文件」场景下列出覆盖后的 provider 与模型；无覆盖文件时行为不变；改动不读取凭据值。

**S02（OD-ARCH-095-S02）**

1. Step ID：OD-ARCH-095-S02
2. 单一动作描述：修正目录分支把 provider 当作 Profile 传参的缺陷，让选择项带上正确的来源 Profile。
3. 输入来源：`ModelPicker.tsx` 目录分支、`ModelRouteListing.engine_profile`、`HomeChat.tsx` 的 `selectModel`。
4. 输出目标：目录分支改用引擎当前 Profile；`ModelPicker` 新增显式入参（如 `engineProfile`）并由 `HomeChat` 传入；不改下拉的版式与信息层级。
5. 验收标准：从目录切换任一模型，宿主收到的 `source_profile` 是真实存在的 Profile；`--dump-config` 不再以 provider 名作为 Profile 执行。

**S03（OD-ARCH-095-S03）**

1. Step ID：OD-ARCH-095-S03
2. 单一动作描述：为「没有可用 Profile 就发起切换」加前置守卫，杜绝空值调用与登录即报错。
3. 输入来源：`HomeChat.tsx` 的 `selectModel` / `selectEffort` / 场景思考档 useEffect；`model_routes.rs::select_route` 空值校验。
4. 输出目标：`selectModel` 入口对空 profile 直接给出可操作文案并返回，不调用宿主；场景思考档在候选未就绪时**不自动切换**，候选就绪后仅在满足既有三条安全线时应用一次；重新载入候选后允许重试。
5. 验收标准：冷启动（候选为空）不产生任何切换请求与失败文案；候选就绪后才按场景应用思考档；手动选档行为不变；`appliedSceneThinkingRef` 不会因一次失败而永久锁死。

**S04（OD-ARCH-095-S04）**

1. Step ID：OD-ARCH-095-S04
2. 单一动作描述：补齐诊断链路与可读错误，把「退出码 1」换成能指引下一步的原因。
3. 输入来源：`model-routes.mjs::dumpConfig`、`model_routes.rs::run_script`、`dump-model-route.mjs` 的既有做法、DSH 的 profile 不存在报错文本。
4. 输出目标：脚本捕获 stderr 并附**有界尾部**（限长、去重、剔除疑似令牌）到错误消息；宿主不再吞 stderr，失败时把尾部并入返回错误；对「profile 不存在」给出中文可操作提示（指明是哪个 Profile、应改用哪个）。错误文本不写入日志文件，不落盘。
5. 验收标准：构造不存在的 Profile 时错误信息包含真实原因而非裸退出码；超长 stderr 被截断且总长度有上限；成功路径的返回结构不变。

**S05（OD-ARCH-095-S05）**

1. Step ID：OD-ARCH-095-S05
2. 单一动作描述：执行回归与真实窗口验证，落盘证据供 ARCHITECT 独立审查。
3. 输入来源：S01–S04 差异；`desktop/src-tauri` 单测、`tests/verification/desktop/` 既有窗口脚本与一次性 Xvfb 模式。
4. 输出目标：针对 S01/S03/S04 的 Rust 与脚本单测；`verify_model_route_095.py` 与 `evidence/od-arch-095/`（冷启动无失败文案、候选数量、目录切换）；STATUS.md 执行记录。
5. 验收标准：cargo test、tsc、前端 build、既有窗口回归全过；`runtime/` 零差异；验证脚本若触发工作记录写入必须指向临时 `OMNIDESK_KNOWLEDGE_ROOT`；真实模型切换的联网结果如实报告（成功与否都记录）。

**依赖关系：** S01 → S03（候选就绪的判断依赖 S01 的候选来源）；S02 独立；S04 独立；S01–S04 → S05。

**不做的事：** 不改 `runtime/` 内 DSH；不改用户的 DSH Profile 与凭证；不重做模型下拉的版式；不因本次修复调整场景思考档的默认值；不动工作区里 OD-ARCH-093/094 的未提交改动。

**实施记录（2026-10-27）：** 用户批准执行。S01、S02、S03、S04、S05 已全部实施；真实窗口验证暴露并补上了一个计划内未尽之处——**写覆盖文件（`select_route`）此前也没有带补丁集合**，导致只存在于覆盖文件里的 provider 在候选中可见、点选却报「该 Profile 里没有这个 provider」。该修正仍属 S01「候选与引擎真实路由一致」的范围。执行详情见 STATUS.md 顶部；待 ARCHITECT 独立审查，未标为通过。

**阶段门槛：** 本计划须经用户明确批准后才实施；范围外需求另行立项（本轮已获批，范围未扩大）。

### OD-KB-001：OmniDesk 知识库总体规划基线（已批准，2026-10-26）

> 分类：产品设计与架构规划。现状审计和分阶段方案见 [docs/OmniDesk-Knowledge-Base-Plan.md](docs/OmniDesk-Knowledge-Base-Plan.md)。本项批准的是知识库规划基线；任何实现仍须按阶段形成具体范围、验收条件和实施记录，并经对应阶段计划批准后开始。P1 已经独立验收通过；P2 详细计划已由 GLM-5.3-FLASH 草拟并经 ARCHITECT 审核，用户于 2026-10-26 批准由 DeepSeek-V4.1-Flash 执行，ARCHITECT 负责独立验收。全程未读取真实知识资产；知识库测试只使用合成目录。

**批准记录：** 用户于 2026-10-26 批准 OD-KB-001，并指定本助手负责总体规划及 P0–P7 阶段性检查，GLM-5.3-FLASH 负责逐阶段制定小计划。P0 定向审计完成，证据与复用边界见总体规划第 2、8 节。P1 已通过独立审查。P2 修复计划 P2-F01～F03 与 EOF 后文件增长补验均完成；ARCHITECT 于 2026-10-26 独立重跑窗口与相关回归，P2 现已通过。P3-S01～S06 已批准并实施完成；ARCHITECT 于 2026-10-26 独立复跑全部测试与窗口验证，P3 已通过（结论见 REVIEW.md 顶部 P3 章节）。P4 已于 2026-10-26 获用户批准并实施，ARCHITECT 于 2026-10-27 独立重跑全部测试与窗口验证后判定通过（结论见 REVIEW.md 顶部 P4 章节）；首期 MVP（P1–P4）完成，P5 需用户单独批准。既有 OD-ARCH 计划及工作区改动保持原状。

#### P1 小计划：单库根目录和只读访问边界（已批准，实施并通过 ARCHITECT 独立验收，2026-10-26）

本计划由 GLM-5.3-FLASH 提供草案，经 ARCHITECT 对照源码和总体基线收敛；用户于 2026-10-26 批准执行，实施由本助手亲自完成，执行与独立验收记录分别见 STATUS.md、REVIEW.md 顶部。各任务严格限于下列五项字段。P1 不实现笔记编辑、持久索引、链接解析或知识图谱。

**P1-S01**

1. Step ID：P1-S01
2. 单一动作描述：静态核查 library_root、OMNIDESK_KNOWLEDGE_ROOT、thread_categories[].vault、artifact_root 与现有文件引用/工作记录接口的全部消费者，记录语义、配置优先级及受影响边界。
3. 输入来源：desktop/src-tauri/src/settings.rs、task_shell.rs、lib.rs、worklog.rs、desktop/src/SettingsPanel.tsx、desktop/src/api.ts、全仓引用检索结果及总体规划第 2–3 节。
4. 输出目标：P1 消费者核查记录，列出源码位置、实际行为和待核实项；作为 STATUS.md 的 P1 执行证据。
5. 验收标准：消费者及已有配置覆盖关系逐项有源码依据；尤其核实 OMNIDESK_KNOWLEDGE_ROOT 对 library_root 的影响；不能静态确认的行为明确标待核实；本步骤不改功能代码。

**P1-S02**

1. Step ID：P1-S02
2. 单一动作描述：在既有设置体系中增加一个可选的知识库根配置及对应读写/校验能力，同时保持现有 library_root、环境变量覆盖和其他消费者行为不变。
3. 输入来源：P1-S01 核查记录、settings.rs 的 Settings/schema/持久化/路径校验实现，以及 task_library_root_info、task_set_library_root 命令模式。
4. 输出目标：独立的可选知识库根设置字段及 Rust 单元测试；字段命名和环境变量交互须由 S01 证据决定并在实施记录中说明。
5. 验收标准：旧设置缺少新字段仍可加载；配置、读取、清除（如支持）均有测试；根路径校验拒绝相对路径、越界路径及总体基线定义的禁用位置；旧消费者回归测试证明语义未改变；不迁移、创建或改写用户知识文件。

**P1-S03**

1. Step ID：P1-S03
2. 单一动作描述：新增只读扫描核心，遍历所选根并返回可展示的文件条目、扫描统计、跳过项和逐项错误。
3. 输入来源：P1-S01 核查记录、task_shell.rs 现有有界遍历/符号链接处理先例，以及总体规划 P1 的文件访问边界。
4. 输出目标：Rust 扫描模块和纯合成临时目录单测；排除规则、可读文件类型、深度/数量/单文件大小限制及符号链接策略在实现前以证据确定并固定在契约中。
5. 验收标准：测试覆盖缺失/不可读根、路径穿越、根内及根外符号链接、非普通文件、隐藏或明确排除目录、受支持文件和大小边界；每个被跳过或失败条目都有可解释统计/错误；扫描只读、不建立持久索引。具体界限须基于现有能力和合成基线记录，不能无依据臆定。

**P1-S04**

1. Step ID：P1-S04
2. 单一动作描述：通过 Tauri 宿主命令暴露根目录设置、扫描/刷新状态、根内文件枚举与单文件只读读取，并统一进行宿主侧路径和类型复核。
3. 输入来源：P1-S02 设置能力、P1-S03 扫描核心、lib.rs 既有命令及 generate_handler! 注册模式。
4. 输出目标：Rust 宿主命令与集成测试，返回结构化扫描状态、文件元数据和错误；附件内容按已批准类型安全打开或读取。
5. 验收标准：隔离配置与合成根的集成测试覆盖设置→扫描→刷新→读取；根外/穿越/符号链接/非普通文件/超限请求在宿主被拒绝；应用接口不包含写入、删除或批量改写能力；错误不泄漏文件内容。

**P1-S05**

1. Step ID：P1-S05
2. 单一动作描述：在现有设置界面提供知识库根目录选择/状态查看与手动扫描刷新入口，不新增知识库分页或编辑流程。
3. 输入来源：P1-S04 宿主命令、SettingsPanel.tsx 与 api.ts 现有目录配置模式、已安装的 Tauri dialog 插件；按 AGENTS.md 在界面实施前通过 screenshots-design-inspiration MCP 检索 2–3 个同类布局，并在 STATUS.md 记录不超过两句的借鉴要点。
4. 输出目标：设置面板、API 类型/封装和必要 UI 验证；显示根路径、扫描状态、文件数、跳过数和错误摘要。
5. 验收标准：用户可选已有目录；未设置、扫描中、成功、有错误各状态清晰；设置页不触发未授权路径访问；不新增依赖、不改 Obsidian 配置、不添加知识库分页。

**P1-S06**

1. Step ID：P1-S06
2. 单一动作描述：构造隔离的 Obsidian 风格与普通 Markdown 合成根，并执行 Rust、宿主及窗口级只读访问验证和冷扫描测量。
3. 输入来源：P1-S02 至 P1-S05、现有 desktop/src-tauri/tests/ 集成测试和 tests/verification/desktop/verify_*.py 隔离验证模式。
4. 输出目标：可重复的合成测试夹具/脚本及新建的 tests/verification/desktop/evidence/ 阶段证据，记录扫描规模、环境和实测耗时；不覆盖旧证据。
5. 验收标准：Obsidian 风格目录中的 .obsidian 配置不被读取或修改；两类根均可扫描；路径边界验收矩阵逐项通过；冷扫描结果来自重复实测并报告样本规模/环境，不将未测得数据写成目标；所有访问限定在临时合成目录。

**P1-S07**

1. Step ID：P1-S07
2. 单一动作描述：完成 P1 回归与阶段记录，供 ARCHITECT 独立验收。
3. 输入来源：P1-S01 至 P1-S06 的源码差异、测试输出、证据文件和实施前记录的 Git 状态/HEAD。
4. 输出目标：STATUS.md 中的执行记录及 REVIEW.md 中的独立审查结论；更新本计划状态前先确认实际工作区差异。
5. 验收标准：相关 Rust/TypeScript/构建和隔离窗口测试结果如实记录；runtime/ 零差异；既有暂存、未暂存、未跟踪改动均保留；源码与 Git diff 经检查；任何失败或未验证项均阻止 P2 开始。

**依赖关系：** P1-S01 → P1-S02/P1-S03（可并行）；P1-S03 → P1-S04；P1-S02/P1-S04 → P1-S05；P1-S02 至 P1-S05 → P1-S06 → P1-S07。

**实施后的对应结论：** 新根为独立字段 `knowledge_vault_root`，`OMNIDESK_KNOWLEDGE_ROOT` 只影响 `library_root`（settings.rs `effective_library_root`）。路径在每次状态、扫描与读取时重走 `assert_vault_root_allowed`，逐层拒绝符号链接与根外落点；文件类型、512 KiB 单文件上限、深度 12/条目 20,000/文件 2,000/明细 200 的边界已固定在 `knowledge_vault.rs` 并有合成目录实测。目录选择经 Tauri dialog 插件在窗口级验证（`verify_knowledge_vault_p1.py`，14/14）。

#### P2 小计划：笔记浏览、属性、关键词搜索与安全编辑（GLM 草拟、ARCHITECT 审核通过；实施完成，独立验收未通过）

本计划由 GLM-5.3-FLASH（`xhigh`）提供草案，经 ARCHITECT 对照 P1 实际代码、总体规划和仓库规则收敛；用户于 2026-10-26 批准由 DeepSeek-V4.1-Flash 执行，ARCHITECT 负责独立验收。实施者须在开工前记录 Git 状态与 HEAD，并保留既有差异。全部夹具和窗口数据必须是隔离的合成临时目录；不得读取真实 Vault 或业务资料。P2 不做双链、持久索引、图谱、重命名/移动/删除、批量操作、附件内容提取、AI/向量或通用 Runtime。

**P2-S01**

1. Step ID：P2-S01
2. 单一动作描述：为单篇笔记定义并实现只读 Frontmatter/属性解析，产出原文、Frontmatter 原始片段、正文边界、可识别标签/别名/属性及解析警告。
3. 输入来源：P1 `knowledge_vault_read_note` 的 UTF-8 有界读取契约；总体规划第 4 节语法基线；仓库现有生产依赖与解析代码审查结果。
4. 输出目标：Rust 侧纯解析模块与合成 Markdown 黄金夹具；仅对有效且受支持的 YAML 结构暴露属性，未识别结构和格式错误保留原文并给出警告，不做规范化写回。
5. 验收标准：覆盖无 Frontmatter、常见标量/列表/标签/别名、未知键、无效 YAML、正文含分隔符、CRLF 与 UTF-8 边界；已知字段和正文位置与夹具期望一致；所有输入原始字节可无损取得；不得为省事静默丢弃笔记或改写内容；是否引入解析依赖须由现有生产依赖能力和测试证据决定。

**P2-S02**

1. Step ID：P2-S02
2. 单一动作描述：在 Rust 宿主实现单篇笔记创建与编辑保存接口，强制根内 Markdown 路径、目标类型、内容大小和冲突检查，并以同目录临时文件及原子替换完成写入。
3. 输入来源：P1 根准入、路径解析和单篇大小限制；P2-S01 原始文本/属性解析契约；仓库现有原子设置写入和成果写入失败清理先例。
4. 输出目标：最小化的 `create` / `save` 宿主接口及临时目录集成测试；保存请求带有打开时读取的原文哈希或等价版本标识，宿主保存前重新读取并比对。
5. 验收标准：拒绝越界、穿越、隐藏/排除路径、符号链接、非 Markdown 目标、超限正文和已存在的新建目标；哈希不匹配时不写入且返回冲突；成功写入后文件为普通 UTF-8 Markdown；注入临时写失败时原文件保持不变；取消编辑和应用重启前未保存的草稿不改变源文件；未知/无效 Frontmatter 未被解析层自动重排。并发外部编辑与最终替换间无法由普通文件 API 原子比较交换的边界须记录，不得宣称完全消除跨进程竞态。

**P2-S03**

1. Step ID：P2-S03
2. 单一动作描述：实现用户触发的根内附件安全打开宿主命令，仅将已校验的普通文件交给系统默认处理程序。
3. 输入来源：P1 根准入与 `resolve_vault_file` 边界；仓库现有 Tauri shell/open 插件及其权限配置（先审计后复用）。
4. 输出目标：按相对路径调用的宿主 API 与合成附件测试；不返回附件正文，不提取 PDF/DOCX 内容。
5. 验收标准：根外、穿越、符号链接、隐藏/排除路径及不存在目标一律拒绝；仅用户明确点击时触发；不会把路径拼成可执行命令或允许调用方提供任意 URL/命令；未授权或未安装系统处理程序时错误可见，知识库其他功能仍可用。

**P2-S04**

1. Step ID：P2-S04
2. 单一动作描述：在 OmniDesk 现有主导航中新增知识库分页和浏览工作区，展示扫描到的 Markdown 笔记列表、选中笔记的原文/渲染预览及属性状态，并提供内存草稿编辑、新建和显式保存动作。
3. 输入来源：P1 扫描/读取 API；P2-S01 解析结果；P2-S02 保存接口；现有主导航模式、`MarkdownText`、Tauri API 封装和界面设计规范。
4. 输出目标：知识库分页/工作区、前端 API 类型与封装、空态/错误/加载/冲突 UI，以及合成数据窗口验证；实现界面前按 AGENTS.md 先通过 screenshots-design-inspiration MCP 检索 2–3 个同类产品布局并在 STATUS.md 写不超过两句借鉴要点。
5. 验收标准：用户可从主导航进入知识库分页；未设置根时有明确设置引导；无 Obsidian、LLM 或网络时可浏览、查看和编辑草稿；列表与属性值可定位来源路径；预览安全渲染 Markdown；保存成功后重新读取显示落盘内容；取消或冲突不会静默丢弃/覆盖源文件；重启不恢复未保存草稿；附件仅调用 P2-S03；页面不触发批量写入或删除。

**P2-S05**

1. Step ID：P2-S05
2. 单一动作描述：在已扫描的笔记集合上实现可解释的 Frontmatter/标签筛选和关键词搜索，不建立持久索引。
3. 输入来源：P1 有界扫描与读取接口；P2-S01 属性结果；P2-S04 工作区列表；P1 每篇 512 KiB、最多 2,000 篇笔记的资源边界。
4. 输出目标：属性/标签筛选器、关键词匹配与结果上下文；明确的大小写/匹配行为和总读取预算，超出预算时显示可理解的状态。
5. 验收标准：合成 Vault 的正文与可识别属性均可命中，结果标明文件路径、命中区域和上下文；无效 Frontmatter 不丢正文搜索能力；标签大小写/层级及多条件组合规则固定并测试；扫描刷新后搜索反映增删改；不缓存成需迁移的事实数据，不发生无界读取/内存聚合；性能数据记录文件数量、总字节数、环境和实测值。

**P2-S06**

1. Step ID：P2-S06
2. 单一动作描述：完成 P2 合成 Vault 的宿主、前端、窗口和回归验收，并形成阶段实施记录。
3. 输入来源：P2-S01 至 P2-S05 的代码差异、合成夹具、测试输出、P1 只读边界证据和实施前 Git 状态/HEAD。
4. 输出目标：新增隔离测试与 `tests/verification/desktop/evidence/od-kb-001-p2/` 证据，更新 STATUS.md；由 ARCHITECT 独立审查并将结论写入 REVIEW.md。
5. 验收标准：验证原始及无效 Frontmatter 保真、列表/筛选/搜索、创建/保存、外部修改冲突、取消/失败/重启保护、附件安全打开、重建扫描可见性和越界拒绝；Rust/TypeScript/构建及仓库相关回归通过；真实用户资料不进入夹具、日志、截图或 Git；未审查前不得标记 P2 通过或启动 P3。

**依赖关系：** P1 → P2-S01、P2-S02、P2-S03；P2-S01/P2-S02/P2-S03 → P2-S04；P2-S01/P2-S04 → P2-S05；P2-S01 至 P2-S05 → P2-S06。

**阶段门槛：** 初验发现并修复了搜索实际读取预算、Markdown 写入类型以及读取期间文件增长完整性问题。ARCHITECT 于 2026-10-26 独立复验 P2 窗口 **33/33**、Rust 全量 **357 项**、pytest **89 项 + 30 subtests**、TypeScript/生产构建均通过；结论见 REVIEW.md「P2 补验与阶段验收」。**P2 已通过，现开放 P3 计划审查；P3 实施仍需单独批准。**

#### P2 修复计划：实际读取预算与 Markdown 写入类型（已批准；F01～F03 执行完成，ARCHITECT 复验发现额外读取竞态，待补验）

**P2-F01**

1. Step ID：P2-F01
2. 单一动作描述：将关键词搜索总读取预算改为按宿主实际读入字节强制执行，并保证扫描元数据过期、文件扫描后增长时不超过 32 MiB。
3. 输入来源：`desktop/src-tauri/src/vault_search.rs::search_scanned`、`knowledge_vault.rs::read_vault_note` 的单篇 512 KiB 限制、P2-S05 资源契约及 REVIEW.md 记录。
4. 输出目标：有界搜索读取策略与隔离合成回归测试，覆盖扫描时元数据低估、随后多个文件增长的累计预算情形。
5. 验收标准：读取前可证明不会越过剩余额度，或以等价受限读取方式保证总上限；预算根据实际读取量记账；过期扫描元数据不能扩大预算；超额时明确列出未检查文件；测试证明 `bytes_read <= budget_bytes`，且部分读取不会被误报为完整搜索。

**P2-F02**

1. Step ID：P2-F02
2. 单一动作描述：将笔记创建和保存目标扩展名收敛为 `.md` / `.markdown`，使 `.txt` 只读浏览和搜索，不经 Markdown 写入接口创建或覆盖。
3. 输入来源：P2-S02 已批准计划的 Markdown 目标与非 Markdown 拒绝要求；`vault_write.rs::check_target_shape`、P1 `VAULT_NOTE_EXTS` 与宿主集成测试。
4. 输出目标：写入白名单修正及宿主集成测试，覆盖 `.txt` 新建和保存拒绝且文件内容不变。
5. 验收标准：`.md` / `.markdown` 创建保存仍可用；`.txt`、附件和其他非 Markdown 类型在宿主层拒绝；拒绝不产生临时文件且不改目标；P1 `.txt` 读取与搜索不受影响。

**P2-F03**

1. Step ID：P2-F03
2. 单一动作描述：对 F01/F02 修复执行知识库相关 Rust、宿主、窗口与仓库回归，并提交独立复验记录。
3. 输入来源：F01/F02 源码差异、合成临时目录、P2 既有测试/窗口脚本、实施前 Git 状态。
4. 输出目标：修复实测记录写入 STATUS.md，ARCHITECT 独立复验结论写入 REVIEW.md，并更新 P2 阶段门槛。
5. 验收标准：预算增长场景与 `.txt` 拒绝测试通过；知识库 Rust/宿主测试、前端构建、窗口回归及 pytest 通过；`runtime/` 零差异；不访问真实 Vault；复验通过前不启动 P3。

**依赖关系：** F01 与 F02 可并行，二者均完成后执行 F03。

**实施后的对应结论（2026-10-26，ARCHITECT 独立验收通过）：** F01 新增 read_vault_note_within / BoundedNoteRead，以剩余预算与 512 KiB 单篇上限的较小值限制实际读取，按 bytes_read 记账；无法完整读取或文件读取期间变化时不返回部分正文，并报告未检查项。F02 将写入白名单限定为 `.md` / `.markdown`，`.txt` 保持只读。独立复验实跑：EOF 增长分类回归 1/1、搜索测试 9/9、写入宿主 10/10、Cargo 全量 357 项、pytest 89 passed + 30 subtests、TypeScript 检查与生产构建通过；P2 窗口 **33/33**，21 张截图，证据在 `tests/verification/desktop/evidence/od-kb-001-p2-recheck/`。搜索规模 1200 篇合成笔记、24,937,046 字节，在 33,554,432 字节预算内。完整独立结论与未验证边界见 REVIEW.md「P2 补验与阶段验收」。**P2 通过，开放 P3 计划审查，实施仍待用户批准。**

#### P3 小计划：双链解析、可重建索引与关系导航（已批准，已实施，ARCHITECT 独立验收通过，2026-10-26）

本计划依据总体基线 P3 与 P0 现状审计、P2 实际文件/Frontmatter/扫描/搜索接口制定。范围仅为本地 Markdown 双链事实解析、可重建的内存索引和反向链接关系浏览；不新增知识图谱可视化（归 P4）、持久索引数据库、后台监视服务、AI、向量检索、跨库联邦、批量重写或文件名/路径操作。索引唯一事实来源是当前 Vault Markdown 原文，每次刷新/重建后可丢弃并重新生成。用户已于 2026-10-26 批准执行 P3-S01～S06；实施已按本计划完成，实测记录见 STATUS.md 顶部 P3 章节。ARCHITECT 于 2026-10-26 独立复跑 Cargo 全量 397 项、P3 窗口 24/24、P1 回归 14/14、P2 回归 33/33、TypeScript/构建/pytest 均通过，验收结论见 REVIEW.md 顶部 P3 章节。**P3 已通过。P4 随后获用户批准并实施，ARCHITECT 于 2026-10-27 独立验收通过（见 REVIEW.md 顶部 P4 章节）。**

**P3-S01**

1. Step ID：P3-S01
2. 单一动作描述：定义并实现覆盖 OmniDesk 首期范围的链接语法解析器，从正文识别标准 Markdown 链接/图片、`[[目标]]`、`[[目标|显示名]]`、`[[目标#标题]]`、`[[目标#^块标识]]` 与 `![[附件或笔记]]`，为每个引用保留原文区间、来源行、上下文和语法类型；代码围栏、行内代码及 Frontmatter 中的相似文本不得误判为正文链接。
3. 输入来源：总体规划第 4 节 K01/K03；P2 `vault_note::ParsedNote` 的 `body`、`body_start_byte` 与 `body_start_line`；`knowledge_vault::scan_vault` 和有界读取 API；现有 Markdown 渲染依赖及其链接语法能力审查。
4. 输出目标：无文件写入副作用的解析数据结构与语法矩阵；固定合成 Markdown 样例及逐项黄金预期；对不支持/不完整语法保留原文并显式产生 unsupported 状态。
5. 验收标准：解析精确识别批准范围内的语法和原文位置；多行、嵌套括号、转义、URL 编码与 Obsidian 扩展的支持边界由矩阵明确；代码/Frontmatter 不产出链接；标题链接、块引用和嵌入分别保留类型；未知语法不删改、不静默伪装成标准链接；黄金夹具逐项比对通过。

**P3-S02**

1. Step ID：P3-S02
2. 单一动作描述：根据解析的链接引用与 P2 扫描/解析数据构建可重建的 Vault 内存索引，解析目标为已解析、未解析、歧义、外部或附件，并构建正向/反向关系、孤立笔记和链接健康记录。
3. 输入来源：P3-S01 语法矩阵与引用结构；P2 `VaultScan.files`、`ParsedNote.aliases`、路径及有界读取接口；总体规划 P3 的别名、重名和失效链接要求。
4. 输出目标：版本化但不落盘的索引快照/查询接口；目标解析规则文档；确定性合成 Vault，覆盖同名文件、别名碰撞、路径限定目标、扩展名、省略目录、大小写及标准相对链接。
5. 验收标准：规则按路径、精确标题/别名与 Markdown 相对路径的有序优先级明确；重复标题或别名不任选错误目标而标记歧义；别名冲突、大小写差异、无扩展名目标均有固定规则；正反链互相一致；无入链笔记与未解析/歧义引用可定位；附件不作为笔记节点；不同类别引用可区分；索引可从 Vault 文件重建且不写入 `.obsidian` 或知识库。

**P3-S03**

1. Step ID：P3-S03
2. 单一动作描述：为笔记标题与块引用构建确定性定位表，并在失效时返回分层回退结果供导航界面解释。
3. 输入来源：P3-S01 的标题/块引用及字节区间；`vault_note::ParsedNote` 的正文偏移与行号；P2 窗口导航与安全渲染行为；Obsidian 标题锚点/块 ID 语法矩阵。
4. 输出目标：标题 slug、重复标题消歧、显式块 ID、标题/块缺失回退的纯 Rust 解析/查询接口及合成测试。
5. 验收标准：标题跳转行号稳定；重复标题依文档顺序生成可复现锚点；显式块 ID 精确匹配；块 ID、标题大小写/标点/Unicode 规则有固定夹具；目标缺失时清楚标注并回退到笔记顶部或保持未定位，绝不打开错误笔记/段落；原始 Markdown 不被修改。

**P3-S04**

1. Step ID：P3-S04
2. 单一动作描述：通过 Tauri 宿主提供显式刷新触发的链接索引构建/全量重建及按笔记读取正向链接、反向链接、未解析/歧义项和孤立状态的只读 API，并在响应中返回源文件版本指纹以支持一致性核验。
3. 输入来源：P3-S01～S03 的纯解析/索引接口；P2 `knowledge_vault_scan`/`knowledge_vault_read_note` 宿主边界、知识库命令注册模式、搜索预算实践。
4. 输出目标：宿主侧重验根和文件、受单次文件/总字节/条目数预算约束的 link-index 命令与 TypeScript API 类型；构建期间不暴露半成品快照，不启动后台常驻监听。
5. 验收标准：索引只读取允许根内的笔记并遵守有界读取/搜索实际预算模式；文件在扫描与读取间变化时拒绝该次快照或标为不完整；全量重建原子发布且可重复；读取/构建零写入用户知识目录和设置文件；超限、解析错误、被跳过文件均可见，不把不完整结果冒充完整索引；Obsidian 不运行也可用。

**P3-S05**

1. Step ID：P3-S05
2. 单一动作描述：在知识库笔记工作区添加链接、反向链接、未解析链接、歧义和孤立状态浏览，支持点击跳转到解析结果及标题/块定位，并对未解析/失效引用显示可解释预览。
3. 输入来源：P3-S04 前端 API 与类型；P3-S01～S03 的状态、行号、上下文和定位回退；现有 `KnowledgeVaultPage`、导航、Markdown 展示组件和设计规范。
4. 输出目标：仅限当前笔记及关系面板的轻量 UI；明确加载、索引不完整、无关系、歧义和错误状态；不实现 P4 图谱画布。
5. 验收标准：点击已解析链接打开正确笔记/段落；反向引用列出来源与上下文；未解析/歧义条目不生成错误跳转；孤立提示与索引结果一致；刷新反映外部增删改；无 Obsidian/LLM/网络可正常使用；关系事实与索引接口一致。

**P3-S06**

1. Step ID：P3-S06
2. 单一动作描述：执行 P3 合成 Vault 的解析、关系、预算、宿主、窗口和回归验证并记录实际证据，供 ARCHITECT 独立验收。
3. 输入来源：P3-S01～S05 差异、语法矩阵、固定黄金夹具、当前 P2/P1 证据、实施前 Git 状态与 HEAD。
4. 输出目标：新增 P3 合成验证脚本/测试及 `tests/verification/desktop/evidence/od-kb-001-p3/` 证据，STATUS.md 执行记录和 REVIEW.md 阶段审查输入；不覆盖已有证据。
5. 验收标准：黄金 Vault 的正链、反链、未解析、歧义、别名、孤立、标题/块定位逐项匹配；外部新增/修改/删除后增量刷新与全量重建结果一致；超预算或读取时变化不会显示为完整快照；确认源文件、附件及 `.obsidian` 字节不变；相关 Cargo、TypeScript、构建、pytest 和窗口回归通过；`runtime/` 零差异；实际测试输出和任何未验证边界如实记录，ARCHITECT 验收前不标记 P3 通过。

**依赖关系：** P3-S01 → P3-S02、P3-S03；P3-S02/P3-S03 → P3-S04；P3-S04 → P3-S05；P3-S01～S05 → P3-S06。

**阶段门槛：** P3 已于 2026-10-26 通过。P4 详细计划见下节，已于 2026-10-27 通过独立验收。

#### P4 小计划：全局/局部知识图谱与关系导航（2026-10-26 批准实施，2026-10-27 实施完成并通过独立验收）

本计划依据总体基线 P4 与知识库总体规划「P4：知识图谱与链接健康视图」制定，基于 P3 已验收的 `LinkIndex` 快照、`note_links` 查询与知识库页面现状。范围仅为图谱可视化与关系导航：全局图谱、当前笔记局部图谱、筛选、聚焦、节点点击打开笔记、孤立/未解析/高连接度状态显示与轻量性能基准。**不实现**：主题聚类、AI 建议关联（K04 后续增强）、第二套索引事实来源、持久化图谱数据、后台布局计算服务、专用图数据库、新的文件写入。图谱数据唯一来源是 P3 索引快照。

技术路线（依据总体规划「先用现有前端能力；达到实测瓶颈后才引入专用图可视化依赖」）：首选用现有 React + SVG 实现力导向布局；**仅在合成基准实测达不到阈值后**才评估引入轻量图库，候选方案登记 docs/decisions.md 候选条目，不预先引入。

**P4-S01**

1. Step ID：P4-S01
2. 单一动作描述：实现从 `LinkIndex` 快照到图谱数据的纯转换层：节点 = 笔记（含孤立标记、反链数、目录、标签、别名属性），边 = resolved 内链（保留方向与锚点信息），unresolved/ambiguous 引用生成带状态标记的虚节点或边，attachment/external 不入图。
3. 输入来源：`vault_index::LinkIndex`/`IndexLink`/`IndexNote` 结构与 P3 语义；`ParsedNote` 的 tags/aliases；P2 属性解析结果。
4. 输出目标：纯 TypeScript 转换模块 + 固定夹具单测；节点/边数据结构定义；转换不发起任何 I/O。
5. 验收标准：节点/边数量与索引 stats 逐项一致；unresolved/ambiguous 状态在图数据中可区分；孤立节点标记与 `orphans` 一致；纯函数可测、无副作用。

**P4-S02**

1. Step ID：P4-S02
2. 单一动作描述：实现全局图谱视图：SVG 力导向布局（确定性初始布局 + 前端模拟迭代上限）、节点拖动、缩放（滚轮/按钮）、平移、节点点击打开笔记。
3. 输入来源：P4-S01 图数据；现有知识库页面布局与设计规范；浏览器 SVG 能力现状（无新依赖）。
4. 输出目标：`KnowledgeGraphView` 组件接入知识库分页，带加载/空态/无索引引导。
5. 验收标准：图谱边与链接索引一致（窗口抽查对照）；点击节点打开对应笔记；拖动/缩放/平移不丢布局；未建索引时显示引导而非空图。

**P4-S03**

1. Step ID：P4-S03
2. 单一动作描述：实现筛选与聚焦：按目录、标签、孤立/未解析/高连接度（阈值可调）过滤节点与边；当前笔记局部图谱（N 度可调，默认 1 度）与全局图谱切换。
3. 输入来源：P4-S01 图数据属性；P2 标签/属性筛选交互模式。
4. 输出目标：筛选控件与局部图谱模式；筛选结果计数；筛选纯逻辑单测。
5. 验收标准：筛选只影响视图、不改索引事实；孤立/未解析/高连接度状态可单独高亮或过滤；局部图谱以当前笔记为中心且切换笔记时更新。

**P4-S04**

1. Step ID：P4-S04
2. 单一动作描述：性能基准与降级：合成 Vault 分级（200/1000/2000 篇及对应边数）实测图谱转换、首次布局与交互帧时间；超过登记阈值时按需加载（先渲染高分/中心节点，交互时逐步展开）。
3. 输入来源：P1/P2 既有合成规模基准模式；P4-S01～S03 实现。
4. 输出目标：基准脚本或测试记录（实测帧时间/构建耗时写入验证证据）；降级策略实现与说明。
5. 验收标准：2000 篇合成规模下首次可用视图不阻塞主线程超过登记阈值；降级时界面仍响应且可打开笔记；基准数据入 evidence。

**P4-S05**

1. Step ID：P4-S05
2. 单一动作描述：执行 P4 窗口级验证与 P1/P2/P3 回归，记录证据供 ARCHITECT 独立验收。
3. 输入来源：P4-S01～S04 差异；P3 验证脚本模式（合成 Vault、一次性 Xvfb、AT-SPI 断言）。
4. 输出目标：`verify_knowledge_vault_p4.py`（覆盖图谱渲染一致性抽查、筛选、聚焦、点击打开、2000 篇降级）与 `evidence/od-kb-001-p4/`；STATUS.md 执行记录；不覆盖既有证据。
5. 验收标准：图谱抽查边与索引一致；筛选/聚焦/打开笔记窗口实测通过；P1/P2/P3 回归全过；Cargo/TypeScript/pytest 通过；`runtime/` 零差异；不读真实 Vault；ARCHITECT 验收前不标 P4 通过。

**依赖关系：** P4-S01 → P4-S02、P4-S03；P4-S02/S03 → P4-S04；P4-S01～S04 → P4-S05。

**关键取舍说明：** ① SVG 而非 Canvas：2000 节点级 SVG 可控且 DOM 可测（AT-SPI 窗口断言依赖可访问性树），Canvas 性能更好但窗口验证困难；基准不达标时先做按需加载/虚拟化，而不是立即换库。② 图数据每次从当前快照现算、不落盘：符合「索引是可重建派生数据」原则，2000 篇级转换成本由 P4-S04 基准实测。③ 不做布局持久化与后端计算：首期规模不需要，引入即违反「无真实触发不加基础设施」。

**阶段门槛：** P4 已通过独立验收（2026-10-27），首期 MVP（P1–P4）完成。进入 P5 需用户对 AI 范围与材料授权单独批准；本轮不实施 P5。
### OD-ARCH-094：文案写作页改版——创作管线与灵感架（已批准实施，2026-10-27 审查通过）

> 分类：产品设计 + 界面实现。定位改写：文案写作页从「与周报同构的档案页」改为「创作控制台」。用户的内容主要在首页与 AI 对话中产生，到本页的诉求是**回顾已有作品**与**找可续写的灵感**；本页不做编辑器、不承载对话（OD-ARCH-057 档案页原则不变，「去对话」回首页的跳转不变）。
> 设计借鉴（screenshots-design-inspiration MCP，2026-10-27，实际查看 Ghost / Notion / SubStack 截图）：Ghost 发布后台用「顶部统计卡 + 按状态分组的内容列表」组织稿件，状态即导航；Notion 数据库看板用属性分组让同一批内容切换视角；SubStack 仪表盘以「统计卡 + 最近文章」突出最近动作。落点：指标卡换创作口径、中栏按状态管线分组、左栏把既有会话历史重新定位为灵感架；三栏骨架与卡片体系复用 OD-ARCH-093 成果，不新增依赖与色值。

#### 现状问题（读码事实，2026-10-27）

1. writing 与 project 共用 ScenePage 结构：指标带为通用三项（成果数 / 待确认写入 / 场景会话），无创作口径；成果区只有 artifacts 页签，列表是平铺目录树，看不出哪些是草稿、哪些已成文。
2. 会话历史默认收起在页面底部，承担不了「找灵感」的入口作用；「继续上次的写作」没有落点。
3. 成果库无类型维度：学习心得、AI 学习知识、blog、网站内容混排，只能靠文件名辨认。
4. 无回顾信号：一篇成果长期未动，页面不会提示。

#### 改版方案

- **R1 指标带（writing 分支四卡）**：本月成文（本月 material_writing 成果数）、进行中草稿（status=draft 计数）、灵感待用（灵感架条目数）、最近更新（最近一次成果修改的相对时间）。全部为宿主可推导数据，不调模型；无数据显示「—」。
- **R2 三栏骨架**：复用 093 的 `max-w-7xl + 1 / 1.4 / 1` 网格与窄窗单列顺序，仅换内容。
  - **左栏（窄）：继续创作 + 灵感架。**「继续创作」卡取最近修改且非已发布的成果，一键打开文件或回首页续聊；灵感架 = writing 标签的最近会话常显列表（数据即现有 sceneThreads 会话历史，仅重新定位、不再默认收起），每条可「继续对话」跳回首页对应会话。不新增任何存储格式。
  - **中栏（宽）：状态管线。**成果按 frontmatter `status` 分为 草稿 / 成文 / 已发布 三组，组头显示计数并可筛选；**未标注 status 的一律归「成文」组，界面写明该缺省口径**。文件是事实来源，状态解析只读、不回写。
  - **右栏：成果文档库**沿用 ArtifactExplorer，增加内容类型标签（学习心得 / AI 知识 / blog / 网站内容），取自 frontmatter `type`，未标注显示中性「未分类」。
- **R3 旧文回顾提醒**：成文/已发布且超过 90 天未修改的成果，在中栏计数行下提示「N 篇超过 90 天未动」，点击筛出列表内条目并加视觉标记。

#### 实施步骤

**S01（OD-ARCH-094-S01）**

1. Step ID：OD-ARCH-094-S01
2. 单一动作描述：为 SceneStatusStrip 增加 writing 分支四卡（本月成文 / 进行中草稿 / 灵感待用 / 最近更新），数据来源复用 scene_overview 聚合或既有宿主命令扩展，不调模型。
3. 输入来源：SceneStatusStrip.tsx 093 实现、scene_overview 聚合命令、S03 的 status 解析结果。
4. 输出目标：writing 指标四卡与对应单测/类型；weekly、project 渲染保持不变。
5. 验收标准：四项指标各有明确宿主来源并在实现记录中写明；无数据显「—」不写 0；weekly/project 不回归。

**S02（OD-ARCH-094-S02）**

1. Step ID：OD-ARCH-094-S02
2. 单一动作描述：writing 场景改为三栏布局：左栏「继续创作 + 灵感架」（灵感架为最近 writing 会话常显列表，含继续对话动作），中栏状态管线容器，右栏成果文档库。
3. 输入来源：ScenePage.tsx 三栏网格、sceneThreads 会话打开机制、ArtifactExplorer。
4. 输出目标：writing 专属三栏渲染；窄窗单列顺序 左栏 → 中栏 → 右栏；project/weekly 布局不变。
5. 验收标准：灵感架条目点击可回首页并定位对应会话；「继续创作」卡数据为最近修改成果；窄窗无横向溢出；不新增存储格式与依赖。

**S03（OD-ARCH-094-S03）**

1. Step ID：OD-ARCH-094-S03
2. 单一动作描述：在宿主成果列表链路中对 writing 场景的 .md 成果做只读 frontmatter 轻解析，提取 `status` 与 `type` 两个可选字段。
3. 输入来源：task_artifact_list 现有实现、knowledge_vault.rs 的读取边界先例（只读、单文件读取上限）。
4. 输出目标：Rust 解析逻辑与单元测试；列表条目携带可选 status/type；不回写、不缓存持久化。
5. 验收标准：无 frontmatter、字段缺失、非法值均有确定性缺省（status 缺省=成文、type 缺省=未分类）；解析只读文件前部、有大小上限；全部为合成目录测试。

**S04（OD-ARCH-094-S04）**

1. Step ID：OD-ARCH-094-S04
2. 单一动作描述：实现中栏状态管线（三组计数 + 筛选 + 按组列表）与右栏类型标签、90 天回顾提醒。
3. 输入来源：S02 布局、S03 解析字段、现有成果列表数据。
4. 输出目标：中栏管线组件与右栏标签/提醒；空组、全缺省、混排三种状态可用。
5. 验收标准：缺省状态下全部成果落在「成文」组且界面注明口径；筛选只改视图派生不改数据；回顾提醒可点击筛选；旧 092/093 断言按新布局同步改写并说明理由。

**S05（OD-ARCH-094-S05）**

1. Step ID：OD-ARCH-094-S05
2. 单一动作描述：新增 writing 页窗口验证脚本并执行全量回归，落盘阶段证据。
3. 输入来源：S01–S04 差异；tests/verification/desktop/ 既有验证脚本与一次性 Xvfb 模式。
4. 输出目标：`verify_writing_094.py` 与 `evidence/od-arch-094/`（宽窗三栏、状态筛选、灵感架跳转、窄窗单列截图与 facts.json）；STATUS.md 执行记录。
5. 验收标准：窗口断言覆盖四指标来源、三组计数一致性（组内条数=成果总数）、灵感架跳转、缺省口径文案；091/092/093 回归全过；cargo/pytest/tsc/build 全过；`runtime/` 零差异；只用合成夹具。

**S06（OD-ARCH-094-S06）**

1. Step ID：OD-ARCH-094-S06
2. 单一动作描述：完成实施记录供 ARCHITECT 独立验收。
3. 输入来源：S01–S05 差异、测试输出与证据。
4. 输出目标：STATUS.md 执行记录；REVIEW.md 审查结论由 ARCHITECT 另行写入。
5. 验收标准：执行前后的 Git 状态如实记录；既有未提交改动全部保留；不提交、不推送。

**依赖关系：** S01 与 S03 可并行（S01 的草稿计数依赖 S03 字段，可先占位后接线）；S02 依赖既有组件；S03 → S04；S01/S02/S04 → S05 → S06。

**关键取舍：** ① 灵感架不新建「灵感」存储——writing 会话历史本来就在，重新定位为常显入口即可，等 KB P9/K09 收件箱落地后再考虑升级为收件箱视图。② 状态与类型都走 frontmatter 可选字段，文件优先、缺省口径写明，不引入私有数据库。③ 素材回链（成果 ↔ 知识库双链）**本计划不做**：成果在 library_root、知识库索引只覆盖 knowledge_vault_root，跨库索引属 KB 后续阶段，另行计划。④ 不做编辑器、编辑日历、创作统计图表与 AI 复用建议（后者是对话能力，页面只留入口，另行评估）。

**阶段门槛：** 本计划为草案，用户批准后实施；ARCHITECT 审查通过前不得视为完成。批准不放宽本计划范围，范围外需求另行立项。

---

### OD-ARCH-093：工作记录页按参考图重排版式（已实施，2026-10-27 审查通过；2026-10-27）

> 分类：界面实现（版式重构）。用户提供参考图 `/mnt/data/Desktop/zhoubao.png`（当日效率看板：顶部四栏指标带、下方三栏、左栏四象限矩阵与清单、中栏带时间轨的时间线、右栏编号目标卡）。
> **状态：已按用户直接指令实施完成（含四次设计修订，S01～S08），正在等待 ARCHITECT 独立审查。** 用户在本文仍是计划草案时直接下达了实施指令，因此**不主张本计划已获正式批准**，实施只覆盖用户点名与本文已写的 S01～S08 范围，未扩展到 P5 或其它计划。
> 实施口径的偏差已记录在 STATUS.md 的 OD-ARCH-093 节：①顶部第二栏改为「进周报条目」（原写「已完成/总事项」，现有数据无对应分母）；②四象限为新增而非保留；③左栏顺序按用户原话为 四象限 → 历史对话（归档已按二次反馈并入成果库）。
>
> 设计借鉴（screenshots-design-inspiration，2026-10-27）：Jumprun 与 Asana 的共同点是多栏但留白大、标题层级清晰；指标带靠分隔线组织，而不是四张拥挤的小卡。落点：加大栏间距与卡片内边距，标题/正文/标签拉开三级字号，归档并入成果库并用徽标区分。
> **2026-10-27 二次设计修订（用户直接指令，仍是 093 范围内的修订）**：①留白不足——三栏 `gap-2` → `gap-4/lg:gap-5`，卡片内边距 `px-3 py-3` → `px-4 py-4 lg:px-5 lg:py-5`；②字体、字号、排版缺乏设计感——区块标题统一 16px semibold 并加 `tracking-tight`，英文副标题 9.5px + `tracking-[0.16em]`，顶部指标数字 22→28px、图标 32→40px，四象限格最小高 104→128px、标题 12→14px、预览 10.5→11.5px，时间线条目行高提升到 `leading-[1.7]`；③历史归档与成果文档库合并——删除独立 `WeeklyArchive` 卡，往期周报与往期周记录作为条目并入 `ArtifactExplorer`，统一按修改时间倒序，显示「归档」徽标与「周报/周记录」来源，搜索、全部/本周/本月、卡片/列表都覆盖归档，原有打开周报与打开周记录动作保留。

#### 需求转写（用户原话 → 可验收项）

1. 顶部四栏，每栏有图标、文字、数字。
2. 下方三栏：左栏四象限（改中文）+ 其下历史对话记录；中栏本周记录，条目文字最多两行；右栏成果文档库，参考样式。
3. 中文为正文语言，英文副标题按需配置（可留空）。
4. 颜色用 OmniDesk 自身色调，不照搬参考图的深蓝灰。

#### 与源码证据的冲突（必须先说明，不得擅自扩大解释）

**冲突一：四象限在当前源码中不存在。** 已核对：`desktop/src/shell/ScenePage.tsx`（622 行）、`WeeklyTimeline.tsx`、`WeeklyReportPanel.tsx`、`SceneStatusStrip.tsx`；全仓 `rg '象限|四象限|quadrant|PriorityMatrix|艾森豪' ` 无命中（仅 `runtime/` 内 DSH 自带无关测试出现 quadrant 字样）；`git log --all -S'四象限'` 全历史无结果；`_design/stitch_omnidesk/DESIGN.md`、`code.html` 与模板、Profile 目录均无此区块。091/092 两版窗口截图（`evidence/od-arch-091/01-weekly-timeline.png`、`evidence/od-arch-092/01-weekly-two-column.png`）也只有指标带 + 时间线 + 成果库。
> 因此「保留四象限」在源码上无对应物，实际是**新增**。计划按新增处理，并请用户确认它不是指另一份未提交的构建。

**冲突二：四象限需要的两个维度当前没有数据。** 参考图的矩阵靠「重要 / 紧急」二维划分。现有确定性汇总只有一维状态与逾期：`task_shell::WeeklySummary` 字段为 `total/done/pending/blocked`，`ProjectSummary` 为 `not_started/in_progress/blocked/done/overdue`；周记录文件格式固定为「`- **事**：… / **去向**：…`」（`worklog.rs::render_block`），不含重要度或紧急度。
> 本计划不臆造这两个维度，采用**可由现有事实确定性推导**的口径，并在界面上写明口径来源（见 S02）。

**冲突三：参考图顶部四栏的指标语义与 OmniDesk 现有数据不同。** 参考图的 FOCUS SCORE / TASKS COMPLETED / FOCUS TIME / PRODUCTIVITY 分别需要评分、任务总数、专注时长、效率百分比。OmniDesk 没有专注计时与效率评分数据源，现成可用的是：本周记录条数、周报状态、逾期条数、成果份数（`SceneStatusStrip` 已在用，均有宿主来源）。

#### 设计借鉴（screenshots-design-inspiration，2026-10-27）

参考图本身已给出结构；补充参照 Notion「Weekly Review」与 Linear Cycles：指标带用细分隔线而非四张独立卡片、条形进度只作视觉强调不承载未校验数据；三栏用同一栏宽网格、栏内独立滚动，避免整页被最长一栏推高。落点：顶部四栏等宽、栏间 1px 分隔线、每栏「圆形图标 + 中文标签 + 英文副标题（可选）+ 大号数字 + 细进度条」；下方三栏固定 `左 1 / 中 1.4 / 右 1`，各栏自带标题行与滚动区。

#### 配色与子标题约定

- 只用 `theme.css` 既有令牌：主色 `--color-primary #3525cd`、次色 `--color-secondary #00687a`、卡片 `bg-white/80 + border-white/90 + shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)]`（与现有场景页一致）。四象限按「重要且紧急 → 重要不紧急 → 不重要但紧急 → 都不」由主色向 surface-container 递减，不引入新色值。
- 中文为默认；英文副标题通过场景配置字段 `subtitle?: string` 提供，缺省不渲染（不留空行）。默认值：工作记录 `WEEKLY LOG`、成果文档库 `ARTIFACTS`、历史对话 `HISTORY`、四象限 `PRIORITY`。

#### OD-ARCH-093 S01 指标带四栏化

1. Step ID：OD-ARCH-093-S01
2. 单一动作描述：把 `SceneStatusStrip` 的 weekly 分支由三栏紧凑指标改为四栏卡片式指标带（图标 + 中文标签 + 可选英文副标题 + 数字/单位 + 说明 + 细进度条），其他场景保持现有三栏不变。
3. 输入来源：`SceneStatusStrip.tsx`、`Icon.tsx` 现成图标（`checklist`/`note`/`clock`/`trend-up`）、`SceneOverview` 与 `weekEntries`/`reportLabel` 既有只读数据。
4. 输出目标：四栏指标：①本期工作记录（条，图标 `note`）②进周报条目（`checklist`，按「去向 = 本周报引用」确定性计数；原计划的「已完成/总事项」因现有汇总无逐条对应分母而改为本项，实施记录见 STATUS.md）③本周周报（`clock`，值取未生成/草稿/已归档，附「去生成」）④逾期（`trend-up`，`alert` 色调）。进度条只在分子分母真实存在时绘制，否则整条隐藏。
5. 验收标准：四栏等宽、窄窗（<640px）纵向堆叠；数字全部有宿主来源，读不到显示 `—` 而非 0；英文副标题缺省不占位；既有「待确认写入」格仍在且有记录时才出现；其他两个场景渲染与现在一致。

#### OD-ARCH-093 S02 四象限（新增，中文）

1. Step ID：OD-ARCH-093-S02
2. 单一动作描述：新增「事项四象限」区块，用中文标签呈现四格，并按**可解释的口径**把本周记录归入四格。
3. 输入来源：`parseWorklog` 已解析的周记录条目（含「去向」）、`SceneOverview.week_start/week_deadline`、宿主 `task_project_summary` 的逾期与状态字段。
4. 口径（必须写在界面与文档里，不写成「AI 判断」）：
   - **紧急** = 该事项所属日期已过或当天为最后一工作日，且状态不是 done；即「已到或已过期但未完成」。
   - **重要** = 条目「去向」属于本周报引用（`本周报引用`）或该事项在项目汇总里为非 blocked 的进行中事项；其余归为不重要。
   - 无法归类的条目（无去向、原文行、解析失败行）统一落入「待归类」并在格下方显示条数，不硬塞进四格。
   - 四格标题：①重要且紧急 · 先做 ②重要不紧急 · 排期 ③不重要但紧急 · 委托或压缩 ④不重要不紧急 · 可删可留。
5. 输出目标：四格卡片（格内显示条数与最多 3 条标题，超出显示「还有 N 条」），点击某格把中栏时间线过滤到该象限（再次点击取消）；无数据时给「本周还没有可归类记录」而不是空格子。
6. 验收标准：四格计数之和 + 待归类数 = 本周记录条数（单测断言）；口径说明在界面上可读且与实现一致；筛选与中栏联动不修改任何文件；纯函数可测。

#### OD-ARCH-093 S03 三栏布局与历史对话归位

1. Step ID：OD-ARCH-093-S03
2. 单一动作描述：把 weekly 主区改为三栏：左栏（四象限 + 历史对话记录）、中栏（本周记录）、右栏（成果文档库）；历史对话由页面底部移到左栏四象限下方。
3. 输入来源：`ScenePage.tsx` weekly 分支、`WeeklyTimeline.tsx`、`ArtifactExplorer`、现有 `threadsSection`。
4. 输出目标：`grid lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)]`；左栏自四象限向下为 历史对话；`<lg` 单列堆叠且顺序为 四象限 → 本周记录 → 成果库 → 历史对话；历史对话默认收起逻辑保留，移动后仍能「去对话 · 新开」与改挂场景。**（2026-10-27 修订：历史归档不再单独成栏成卡，并入右栏成果库，见 S06。）**
5. 验收标准：三栏各自独立滚动、不互相推高（`min-h-0` + `overflow-y-auto`）；窄窗顺序正确且无横向滚动；会话历史功能（打开、改挂、删除）与现在等价。

#### OD-ARCH-093 S04 中栏时间线与右栏成果库样式对齐

1. Step ID：OD-ARCH-093-S04
2. 单一动作描述：中栏按参考图改为「左侧时间刻度 + 条形条目卡」，条目正文最多两行截断；右栏成果库改为编号卡样式。
3. 输入来源：`WeeklyTimeline.tsx`（现有连接轨与日期分组）、`ArtifactExplorer`、参考图右栏编号卡。
4. 输出目标：中栏每条记录为浅底圆角条（`bg-surface-container-low/60`，今日条主色左缘），正文 `line-clamp-2`，悬停 `title` 显示全文；保留「去向」标签与「转周报」动作；空日期保留灰点与短横线。右栏成果为编号卡（`01/02…` + 文件名 + 类型/时间），保留现有搜索、全部/本周/本月筛选与「卡片/列表」切换。
5. 验收标准：两行截断生效且不丢原文（`title` + 点击仍在下方预览区完整显示）；条数与筛选、空态文案不回归；窄窗仍单列。

#### OD-ARCH-093 S05 窗口验证与回归

1. Step ID：OD-ARCH-093-S05
2. 单一动作描述：新增 `tests/verification/desktop/verify_weekly_093.py`，用合成工作记录与合成成果验证新布局，并回归既有周报页断言。
3. 输入来源：`verify_knowledge_vault_p4.py` / `verify_profile_rules_068.py` 的合成夹具与 Xvfb + AT-SPI 模式、`evidence/od-arch-092/verify_weekly_092.json`。
4. 输出目标：`evidence/od-arch-093/`（截图：宽窗三栏、窄窗单列、四象限筛选联动、两行截断、成果编号卡）+ facts.json。
5. 验收标准：四栏指标在 AX 树可读；四象限计数自洽（含待归类）；中栏两行截断实测；三栏无横向溢出；合成目录字节不变；`npm run build`、`npx tsc --noEmit`、`python3 -m pytest tests -q`、`cargo test --offline` 通过；`runtime/` 零差异；STATUS.md 如实记录，结论由 ARCHITECT 审查后写入 REVIEW.md。

#### OD-ARCH-093 S06 历史归档并入成果文档库（2026-10-27 用户二次反馈）

1. Step ID：OD-ARCH-093-S06
2. 单一动作描述：删除左栏独立「历史归档」卡，把往期周报与往期周记录并入右栏成果文档库的同一列表，用「归档」徽标与来源类型区分，不新增独立区块。
3. 输入来源：`ScenePage.tsx` 已拉取的 `WorklogArchiveList`（`reports` / `weeks` / `current_week` / `skipped`）、`WeeklyReportPanel` 的只读打开动作、`ArtifactExplorer` 现有筛选与视图状态。
4. 输出目标：`ArtifactExplorer` 新增可选 `archiveEntries` 通道（类型 `ArtifactArchiveEntry`），与成果合并后按 `modified_at` 倒序；归档项显示序号、`周标签 + 周报/周记录`、文件名、日期与「归档」徽标；搜索覆盖文件名/周标签/来源/「归档」，时间筛选与卡片/列表视图同时作用于两类条目；点击仍分别走 `worklogReadReport` 与 `worklogReadWeek` 只读命令；归档读取失败与不合约定文件数如实显示。当周记录留在中栏，不重复塞进成果库。
5. 验收标准：页面上不存在独立「历史归档」区块；成果库内可见带「归档」标记的往期条目且可打开；搜索能命中归档；窄窗单列为 四象限 → 本周记录 → 成果库 → 历史对话且无横向溢出；不读取真实成果目录，预览只用 `/tmp` 合成数据。

#### OD-ARCH-093 S07 留白与字号层级（2026-10-27 用户二次反馈）

1. Step ID：OD-ARCH-093-S07
2. 单一动作描述：按参考图放大栏间距、卡片内边距与标题/正文/元信息的字号层级，不改变既有信息口径与数据来源。
3. 输入来源：参考图 `/mnt/data/Desktop/zhoubao.png`、本轮同类布局检索（Jumprun 卡片式周复盘、Asana 多栏任务板）、`theme.css` 既有字体与颜色令牌。
4. 输出目标：主区 `gap-4 lg:gap-5`；四象限、本周记录、成果库、历史对话卡片 `px-4 py-4 lg:px-5 lg:py-5`；区块标题 16px semibold + `tracking-tight`，标题行加底部分隔线与 14px 下留白；英文副标题 9.5px + `tracking-[0.16em]`；顶部指标图标 40px、标签 12.5px、数字 28px、进度条 4px、栏内 `px-4 sm:px-5 py-2.5 sm:py-3`；四象限格 `min-h-[128px]`、标题 14px、预览 11.5px、格间距 `gap-2.5`；时间线条目 `leading-[1.7]`、条目间距 `gap-2.5`、内边距 `px-3.5 py-2.5`；成果卡 `px-4 py-3.5`、标题 13.5px、元信息 10.5px、卡间距 `gap-2.5`。全部沿用 OmniDesk 紫/青令牌，不引进外链字体或新色值。
5. 验收标准：宽窗三栏视觉复核无文字重叠、无信息挤压；1000px 窄窗单列顺序正确且无横向溢出；两行截断仍生效；类型检查、构建与既有回归全部通过。

#### OD-ARCH-093 S08 横向指标卡与右栏工具栏精简（2026-10-27 用户第四次反馈）

1. Step ID：OD-ARCH-093-S08
2. 单一动作描述：按用户第四次反馈，把顶部四张竖长指标卡改成保留原四色的横向长方卡，三栏主区与顶部导航条同宽，右栏工具栏去掉时间筛选与「含子目录」勾选并按固定近 30 天、含子目录出数；只改版式与控件可见性，不改指标口径、数据来源与读取命令。
3. 输入来源：参考图 `/mnt/data/Desktop/zhoubao.png`、用户第四次反馈原话、`SceneStatusStrip.tsx` 的 `BandMetric`、`ScenePage.tsx` 的 weekly 三栏容器、`ArtifactExplorer.tsx` 的筛选与视图状态。
4. 输出目标：
   - 顶部四卡容器保持 `max-w-6xl`（整排窄于导航 `max-w-7xl` 并在其下居中，卡间 `gap-3 sm:gap-4`），卡内改为横向 KPI 两区：左「36px 图标座 + 中文标题 + 01–04 序号与英文副标题」，右「大号数字 + 单位」，底行说明与动作，底部 4px 进度轨；`px-4 py-3`，实测卡高约 90px；数字按长度分档（≤2 字 30px、3 字 24px、≥4 字 20px）且不折行。
   - 四色保留：紫（本期工作记录）/ 青（进周报条目）/ 靛（本周周报）/ 琥珀（逾期；真有过期转玫红），不新增色值。
   - 三栏容器 `max-w-7xl mx-auto md:-translate-x-8` + `data-board-width="nav"`，与顶部导航左右边界基本重合（实测差 1px）；栏宽 `1 / 1.4 / 1`，`lg` 以下单列。
   - `ArtifactExplorer` 新增可选 `compactToolbar`，仅工作记录页启用：隐藏「全部/本周/本月」与「含子目录」，固定 `now - 30 天` 滚动窗口并强制包含子目录；工具栏只留搜索框与卡片/列表切换，计数行写明「当前只列近 30 天（共 N 条）」。其他场景不传该属性，行为与现在一致。
5. 验收标准：宽窗四卡为横向长方形、卡高约 90px、四条英文副标题与状态值不被截断；三栏与导航同宽同轴且无横向溢出；右栏 AX 树只出现搜索框与卡片/列表按钮，不含时间筛选与子目录勾选；固定窗口按 30 天过滤、含子目录生效；其他场景筛选与子目录行为不回归；类型检查、前端构建、093/092/091 窗口验证与 pytest 全部通过；`runtime/` 零差异。

#### OD-ARCH-093 S09 周报页展示层清理与可读性修订（2026-10-27，用户直接指令，已授权执行）

1. 单一动作描述：清理周报页的冗余说明模块，保留待确认写入安全链路并改为本页内 80% 宽弹窗；重排成果库工具栏与历史对话分类控件；将周报页可见小字提高到至少 12px。
2. 范围：`AnnouncementBar` 不再渲染规则提醒横条但保留调度心跳；四象限移除口径说明模块；成果库移除成果计数/Obsidian说明模块；`ArtifactExplorer` 精简模式固定近 30 天但恢复「含子目录」开关；搜索行与卡片/列表/子目录行分开；历史对话使用既有 `CategoryPicker` 胶囊浮层；待确认写入只以触发条 + 80% 宽弹窗呈现。
3. 约束：不读取真实知识库、成果目录或正式资料；不改 `runtime/`；不新增模型调用、数据格式或宿主写入路径；继续使用 OmniDesk 现有色彩令牌。
4. 验收标准：规则提醒、口径说明、成果库说明三类可见模块消失；待确认写入可打开/关闭弹窗且保留确认、放弃、过期语义；右栏控件满足两行布局并能切换含子目录；历史分类控件不再是原生方框；类型检查、构建、091/092/093 回归与合成窗口验证通过；STATUS.md 如实记录，REVIEW.md 等待独立审查后再下结论。

**依赖关系：** S01 与 S02 可并行；S02 → S03；S03 → S04；S01～S04 → S05；S06 依赖 S03 的左栏结构与 `WorklogArchiveList`；S07 覆盖 S01/S02/S04 的样式参数，与 S06 同轮实施；S08 覆盖 S01 的指标卡版式、S03 的容器宽度与 S04 的右栏工具栏，依赖 S01/S03/S04 已实现；S09 基于 S01～S08 的现有页面，仅做展示层清理、控件恢复与可读性修订。

#### OD-ARCH-093 S10 待确认入口并入逾期卡与历史对话操作修订（2026-10-27，用户直接指令，已授权执行，审查通过）

1. Step ID：OD-ARCH-093-S10
2. 单一动作描述：按用户第五次反馈，把「待确认写入」入口并入第四张「逾期」卡并删除四卡下方的独立触发条；弹窗内条目底色改为方形；历史对话的分类下拉改为不被容器裁切、可完整显示；单条对话的删除按钮移到标题之前；页面可见小字保持至少 12px。
3. 范围：`SceneStatusStrip.tsx`（逾期卡内动作 + 删除独立触发条 + 提示文案）、`PendingWriteCard.tsx`（条目与按钮去圆角、场景中文名）、`CategoryPicker.tsx`（portal + fixed 定位 + 上下翻转 + 视口夹取）、`ScenePage.tsx`（删除按钮前置）；验证脚本与合成窗口证据同步。
4. 不改：待确认的安全链路与字段契约、场景聚合口径、四象限归类规则、宿主命令与数据格式、`runtime/`。
5. 验收标准：待确认入口只出现在逾期卡内，弹窗仍保留确认 / 放弃 / 过期 / 回执语义；弹窗条目底色为方形；历史对话分类下拉全部选项在视口内可见；删除按钮位于每条标题左侧且可点击；首页输入区的分类下拉不回归；类型检查、前端构建、091/092/093 校验、pytest、Rust 离线测试与合成窗口验证通过；`runtime/` 零差异；STATUS.md 如实记录，结论等待 ARCHITECT 独立审查。

**S10 依赖：** 基于 S08 的四卡版式与 S09 的待确认弹窗、分类胶囊控件；只改这些既有部件的展示与定位，不新增数据源。

**关键取舍：** ① 四象限按可解释规则归类，不引入「重要度」新字段、不调模型——现有记录格式无法承载人工标注，硬造评分会违反「数字必须有来源」；若用户要真正的人工四象限，需要在记录格式里新增字段，那是另一项计划。② 顶部四栏不强行复刻 FOCUS SCORE / FOCUS TIME，因为无数据源；改用有来源的四项。③ 条目「最多两行」用 CSS `line-clamp` 截断显示，原文不丢失，点击仍可全文查看。

**阶段门槛：** S01～S10 均已由 ARCHITECT 独立审查通过（见 REVIEW.md）。S10 之外的改动（P5、真实数据源、记录格式新增重要度字段）均未开始，需要单独计划与批准。


### OD-ARCH-092：工作记录页版式重构（两栏 + 指标带 + 复盘入口收起）（实施完成待验收，2026-10-26）

> 触发：用户 2026-10-26 四点改版要求（时间线与成果库两栏、自动复盘去留、四个模块重设计、归档与会话两栏）。设计借鉴（screenshots-design-inspiration MCP，2026-10-26）：Kairn（左主内容 + 右窄栏归档的列表/详情分栏）、Timely（周视图按日分段、弱化装饰、单色轴）、Teamweek（时间线作主阅读区）。借鉴要点：①左主右辅约 1.6:1，右栏自成一个可滚动卡片；②指标不做等宽大卡，改为一行「小标题 + 数字 + 一句灰字」，只有异常项着色。

#### 一、现状问题（读码事实）

1. weekly 分页仍是页签切换：`ScenePage.tsx` 的 `tab` 三态里 timeline 与 artifacts 互斥，看时间线时看不到成果库。
2. 三张状态卡等宽（`SceneStatusStrip.tsx`：17px 数字 + 10px 标签 + 三列通栏），密度低；且「本期工作记录」数的是 `overview.artifacts_this_week`（成果文件数），不是本周随手记条数，与用户预期不符。
3. 自动复盘默认关闭（`scheduler.rs` `review_enabled = false`），用户机无 `~/OmniDesk/config/scheduler.v1.json`（全默认）。它占 `AnnouncementBar` 一整行（开关 + 频率 + 立即复盘 + 建议行），并且是 `pendingWrites.create` 的唯一调用点 —— 复盘关掉后「待确认写入」恒为 0。
4. 历史归档与会话历史各占一条通栏，纵向浪费。

#### 二、自动复盘去留（用户已裁定：移除界面，保留心跳与规则提醒，2026-10-26）

**用户裁定：移除自动复盘的界面入口，保留心跳与规则提醒。** 移除范围 = 场景页那行开关 / 频率 / 立即复盘 / AI 复盘建议行 + 设置页「定时与提醒」里的复盘分组。依据：工作流已是「随手记逐条落盘 → 明确说整理周报才生成周报」，复盘是第三条重复路径且输入材料与周报重叠；默认关闭且从未启用。

配套：`pending_writes.rs` / `scheduler.rs` 的字段与单测**保留**（安全契约与回退能力），只收前端入口；「待确认写入」从常驻卡降级为**有记录才出现的窄条**（pending + expired > 0 时渲染）；`AnnouncementBar` 瘦身为一行提醒，规则文本为空时整行不渲染。

#### 三、改版方案

**R1 指标带（替代三张卡）**：`SceneStatusStrip` 改为一行紧凑指标（小标题 + 数字 + 一句灰字）。weekly 四项：本期工作记录（本周时间线 `entryCount` 条，不再是成果文件数）、本周周报（未生成/草稿/已归档，复用 `reportState`）、逾期（`pending_expired`）、待确认写入（仅 >0 时出现，点击展开 `PendingWriteCard`）。project/writing 保持原字段语义，仅换样式。

**R2 左右两栏主区**：weekly 取消页签，`lg:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]`，左「本周时间线」（周报状态收进卡片头作为动作行），右「成果文档库」（卡片内独立滚动）；`lg` 以下堆叠单列。

**R3 时间线字号 +2**：条目 11.5→13.5px，日期 10→12px，星期 10→11.5px，去向小标 9.5→11px，空日占位 10.5→12px；行距与左轨位置同步微调。

**R4 历史归档 + 会话历史两栏**：`lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]`，归档在左、会话在右；会话仍默认收起，展开后在自身栏内滚动，不再下推整页。

**R5 卡片体系统一**：沿用 `bg-white/80 border border-white/90 rounded-2xl` 与既有阴影，栏内分块用分隔线 + 留白，不新增颜色变量、不引依赖。

#### 四、实施步骤

S1 `SceneStatusStrip.tsx` 重写为指标带（props 增 `weekEntries` / `reportLabel` / `onRequestReport`；pending 为 0 不渲染该项）。
S2 `ScenePage.tsx` weekly 去掉 tab 改两栏；`tab` 仅保留给 project/writing；归档与会话历史合为一个两栏 section。
S3 `WeeklyTimeline.tsx` 调字号，卡片头合并周报状态动作行（`WeeklyReportStatus` 收为紧凑行组件）。
S4 `AnnouncementBar.tsx` 收为一行提醒（移除复盘 UI 与 `runReview` 调用链）；`SettingsPanel.tsx` 移除复盘开关组，保留心跳组与状态行。
S5 测试同步：`verify_weekly_091.py` 的 `timeline_is_default_tab` 等页签断言改为两栏断言；新增 `verify_weekly_092.py` 覆盖字号值、无页签、指标来源、两栏结构；`verify_scene_overview_060.py` 先用 `--probe` 校准新坐标再更新点击点。
S6 窗口级：`preview_weekly_092.py`（隔离 `OMNIDESK_KNOWLEDGE_ROOT` + 一次性 Xvfb）抓三帧（默认态、展开待确认、1280 窄窗）。

#### 五、验收标准

- weekly 页时间线与成果文档库同屏两栏、页签消失；栏宽比 1.6:1 ±0.1；`1280×820` 无横向溢出与文字截断。
- 「本期工作记录」显示本周记录条数，与时间线 `entryCount` 一致（合成 5 条即 5）。
- 时间线条目 ≥13.5px、日期 ≥12px。
- 自动复盘在场景页与设置页均不可见；心跳与规则提醒仍在；`pending_writes` / `scheduler` 单测保持通过。
- 历史归档与会话历史同屏两栏；会话展开不下推整页。
- `npx tsc --noEmit` 0 错、`npm run build` 通过、`cargo test --offline` 全绿、`pytest tests -q` 全绿、`verify_ui_084/088` 与更新后的 091 及新增 092 全过。
- `runtime/` 零差异；不提交、不推送、不打标签。

#### 六、风险

- 版式改动会让 `verify_scene_overview_060.py` 的固定坐标点击失效，必须重新校准；坐标不稳则改按 AX 名称定位。
- 复盘入口移除属功能删减，需用户明确同意；后端字段保留，可随时恢复入口。
- 真实模型端到端（OD-ARCH-080 清单）不在本轮范围。

#### 七、实施结果（2026-10-26，ARCHITECT 自行实施并复验）

S1–S6 全部落地：`SceneStatusStrip` 改为一行紧凑指标（weekly 四项、weekly 以外两项，待确认写入仅 >0 时出现）；`ScenePage` weekly 取消页签，改 `minmax(0,1.6fr)/minmax(0,1fr)` 两栏，归档与会话历史也并列两栏；`tab` 收窄为 `"progress" \| "artifacts"`，weekly 不再读写它；`WeeklyTimeline` 字号整体 +2；`AnnouncementBar` 与设置页的复盘入口全部移除，心跳与规则提醒保留，`pending_writes` / `scheduler` 后端契约未动。

实施中修正的两个真实缺陷：①「去对话」按钮文案被误改成「新开对话」，恢复为「去对话 · 新开」，否则 `verify_ui_088.py` 的入口断言失效；②指标带里 `Metric` 自身是 `flex-1`，未被等宽容器包裹时第一项吃掉剩余宽度，「去生成」压在「逾期」上——改为外层 `div.flex-1` 等宽包裹、动作移到说明行右端、分隔线提高到 `surface-variant/70`。

验证（本人执行）：`npx tsc --noEmit` 0 错；`npm run build` 通过；`pytest tests -q` 89 passed / 30 subtests；`cargo test --offline` 299 passed；`verify_weekly_091.py` 67/67；`verify_weekly_092.py` 29/29；`verify_ui_088.py` 30/30（真实窗口）；`verify_ui_084.py` 全通过（真实窗口）；`verify_scene_overview_060.py` 重新标定坐标后通过，列表视图改用 AX 定位；`preview_weekly_092.py` 三帧落 `tests/verification/desktop/evidence/od-arch-092/`。`runtime/` 零差异；未提交、未推送、未打标签。

`verify_scene_overview_060.py` 顺带修好两处早已失效的环境问题：补一次性 `dbus-daemon`（缺它窗口起不来）、改 `reuse_running_dev_server=True` 并隔离 `OMNIDESK_KNOWLEDGE_ROOT`（原来自起 vite 会因 5273 被占直接失败，且没隔离库根）。
### OD-ARCH-089：工作记录改为意图自动判定 + 随手记确定性落盘（实施完成待验收，2026-10-26）
### OD-ARCH-091：周报分页改版（时间线 + 报告状态 + 历史归档）（实施完成待验收，2026-10-26）

> **背景：** 089 已由用户实测通过（N2 关闭）。用户要求改造周报场景分页设计，参考最新设计趋势。设计借鉴（来源 screenshots-design-inspiration MCP，代理故障仅取得标签）：Feedly 的分组列表 + 弱化装饰、日历/日志类应用的时间线流；要点：按日分组的时间线替代碎片胶囊、报告状态卡置顶。

#### 一、现状问题（读码事实）

1. weekly 分页现状 = 状态条 + 公告栏 + 「本期记录碎片」胶囊 chips + 成果文档库。chips 只显示保存的成果文件，不含随手记原始记录（记录落在库根 工作记录/，不在 artifacts 列表里），用户在分页里看不到本周记了什么。
2. 周报本身的状态（本周是否已生成、草稿在哪）无处可查，要回首页对话才知道。
3. 历史周报混在成果文档库里，没有按周归档视图。

#### 二、改版方案（三块，只改 weekly 分页，其他场景不动）

**W1：本周记录时间线（替代 chips 区）**

- 左主栏顶部新增「本周记录」时间线：按天分组（周一~周日），每天下方按时间列出随手记条目。
- 数据源：宿主已有 worklog_read_week 命令读当周 md 文件，按条目分隔渲染；当天无记录显示灰点占位。
- 每条右侧放「转周报素材」「打开原文」两个轻操作（前者=打开首页对话框并预填「整理成周报」，后者=打开 md 原文定位）。
- 新增宿主命令 worklog_list_weeks（列库根 工作记录/ 下全部周文件，纯文件系统读，不调模型），供 W3 用。

**W2：本期周报状态卡**

- 时间线下方一张卡：本周周报状态（未生成 / 草稿已存在 / 已归档）+ 主按钮「去生成周报」（跳首页预填 report 意图）+ 次按钮「查看草稿」。
- 状态判定为确定性文件检查（库根对应路径是否存在当周周报文件），不调模型。

**W3：历史周报归档栏（右栏或下方，按空间取舍）**

- 按月分组列出历史周报文件（来自 W1 的 worklog_list_weeks 成果侧），点击预览。
- 成果文档库保留但降为页签，默认显示时间线视图。

#### 三、实施步骤

- S1 Rust：新增 worklog_list_weeks 命令（复用 worklog.rs 路径解析与库根逻辑），单元测试覆盖空目录/多文件/非法文件名。
- S2 前端：新建 WeeklyTimeline 组件；ScenePage.tsx weekly 分支重排为「状态卡 + 时间线 + 归档」；chips 区删除；成果库改页签。样式沿用现有 surface 体系，不引新依赖。
- S3 验证：tsc 0 错、npm build 通过、cargo test 相关模块通过、verify_ui_084/088 不回归、新增 verify_weekly_091.py（离线验证组件渲染逻辑 + 命令行为）。证据落 evidence/od-arch-091/（新目录）。
- S4 窗口级 Xvfb 截图一帧（时间线有合成数据态），供用户上机前预览。

#### 四、验收标准

> **实施结果（2026-10-26）：** S1–S3 完成，脚本级全过：cargo 279 passed / 0 failed，`npx tsc --noEmit` 0 错，`npm run build` 通过，`verify_weekly_091.py` 65/65，`verify_worklog_089.py` 61/61，`verify_ui_084.py` 全项 PASS，`verify_ui_088.py` 30/30。
>
> S4 已完成：`tests/verification/desktop/preview_weekly_091.py` 在隔离根的合成数据上抓了两帧，`evidence/od-arch-091/01-weekly-timeline.png`（本周时间线 5 条）与 `02-weekly-history.png`（历史归档 W43/W42）。
>
> **过程中发现并修掉一处验证隔离缺陷（非 091 代码问题）：** `verify_ui_088.py` 与预览脚本此前只隔离了成果目录，没隔离库根（`OMNIDESK_KNOWLEDGE_ROOT`），导致合成指令经 `worklog_append` 写进用户真实 `~/OmniDesk/库/工作记录/2026-W44.md`，共 8 条合成条目。已按数据保护规则定点删除这 8 条、保留用户原记录（备份 `/tmp/od-w44-backup-20261026022816.md`），并给两个脚本补上库根隔离，复跑后真实文件哈希未变。另修 `_mock_model_server.py` 的触发词匹配：089 之后用户原话落在提示词末尾，原先只读前 200 字符会漏判。

- weekly 分页可见：本周按天时间线、周报状态卡、历史归档；随手记当天记录 5 分钟内出现在时间线（复用现有落盘，不新增写入路径）。
- 其他场景分页零改动；runtime/ 零差异；不提交不推送。

#### 五、风险

- worklog md 的条目格式由 Skill 约定而非机器 schema，时间线解析按现有追加格式做容错，解析失败的行原样显示不丢弃。
- worklog_list_weeks 需要库根读权限，已有 worklog_read_week 同级先例，风险低。

### OD-ARCH-090：夜间无人值守收尾批次（待批准，2026-10-26）

> **目标：** 在无人值守条件下完成三项可自动化收尾（087 验收、089 窗口级验证、未提交改动安全扫描），并把状态文档与现状对齐。真实模型输出验证不属于无人值守范围，留给用户实测。用户批准后由本助手亲自执行，不派子代理。

#### 范围与执行顺序

**N1：OD-ARCH-087 验收收尾（release 启动器）**

- 步骤：eval 工具链环境后，在 desktop/src-tauri 跑 release 构建（或复用已有产物做冒烟），隔离环境（独立数据根 + 合成端点）启动 release 二进制，确认窗口出现、进程退出后无遗留引擎进程、无遗留假引擎。结论写入 REVIEW.md（087 条目）。
- 验收：启动成功、退出干净、证据落 tests/verification/desktop/evidence/od-arch-087/（新文件，不覆盖旧证据）。
- 失败处理：连续两轮修复未解决则记录根因并停止该项，不阻塞 N2。

**N2：OD-ARCH-089 窗口级自动化验证（Xvfb）**

- 步骤：Xvfb 启动应用（合成模型端点），验证：① weekly 场景输入「随手记一句」→ 提示「按随手记记录」且工作记录文件追加「（续）」段；② 输入「整理成周报」→ 提示「按周报生成」且注入含周报 Skill；③ 空周报路径：库内无当周记录时生成周报不报错、给出缺项说明；④ 现有窗口回归 verify_ui_084/088 不回归。
- 证据落 tests/verification/desktop/evidence/od-arch-090/（新目录）。
- 验收：四项全过则 089 的窗口级缺口关闭，仅剩真实模型实测留给用户；任何一项失败记录现象与根因，不改产品逻辑（除非是一行级明显接线错误且修复不改架构）。

**N3：47 项未提交改动安全扫描（只读，不提交）**

- 步骤：对全部未提交/未跟踪文件做红线扫描（身份证号模式、密钥模式、真实姓名高频词、绝对路径泄漏）；输出清单到 audit/workspace-scan-20261026.md，标注每项「可安全提交 / 需人工确认 / 不应提交」。不执行 git add/commit/push。
- 验收：扫描覆盖 47 项、每项有标注、零写入 Git。

**N4：状态文档对齐（小步）**

- GOALS.md：仅把「模型端点准入保持关闭」等与 088/089 后现状（用户已在设置页自助开启外部模型）不符的一两处表述改为「模型权限由用户在设置页自助管理，默认从严」；不重写其他段落。
- STATUS.md：追加 N1–N3 执行记录。
- 验收：diff 仅限上述两文件的小段落。

#### 全局约束

- 不改 runtime/；不碰凭证与模型路由；不提交、不推送、不打标签；既有 47 项改动一律保留。
- 所有验证脚本复跑前先确认证据落点不覆盖历史证据（082 教训）。
- 单项失败不滚动放大：记录后继续下一项，最终汇总报告。
- 总量控制：每项一次性完成，不重复规划、不长篇自检。

#### 风险

- Xvfb 下 release 构建可能与开发模式行为差异（N1 已隔离验证过 dev 模式，风险低）。
- 窗口级验证依赖合成端点时序，偶发超时重试一次后仍失败即记录为未验证，不伪造通过。


> **状态：代码与文档改动已完成，ARCHITECT 脚本级复核通过；待用户真实模型实测验收。** 触发来源：用户 2026-10-26 三条新需求（取消手动开关、由关键词判定意图、随手记逐条增量落盘、只做缺项分析），追加第四项（anysearch 移出预注入、周报与随手记拆两个 Skill）。本计划取代同日早先「只修按钮初值」的草案。用户已批准执行（OD-ARCH-089）。
>
> **实施结果摘要（2026-10-26）：** S1 意图判定 `desktop/src/workIntent.ts`；S2 宿主确定性落盘 `desktop/src-tauri/src/worklog.rs` 与 `worklog_append` / `worklog_read_week` 两个命令；S3 提示词瘦身并把 Skill 拆分：`memory-agent` = 随手记（1432 字符），`memory-report` = 周报（988 字符）。weekly 绑定改为 `[memory-agent, memory-report, knowledge-search]`；anysearch 移出预注入，但仍随安装同步、仍在白名单、运行时按需加载。实测注入字符数：随手记 1684、周报 5627；改造前同条件为 1534 与 5154，周报多出的部分是当周工作记录正文（合成样本 300 字符）。脚本级验收项全部通过，窗口级与真实模型级结论见 REVIEW.md 顶部。

#### 一、已核实事实（分级）

**文件事实（读码）：**

1. **手动开关接线反了。** `HomeChat.tsx:520` `noteMode` 初值 `false`；`:1370` 判 `preset.key === "weekly" && noteMode ? "note" : "report"`；`:2434` 选项数组把 `false` 标为「随手记」。界面高亮「随手记」→ 实际发 `report`；点「整理成周报」→ 也是 `report`。两个按钮同一条路。
2. **产品代码里没有任何一处写 `库/工作记录/`。** `desktop/src-tauri/src/` 全量检索：追加写入能力只有 `append_usage_record`（task_shell.rs:1039，写 usage.jsonl）、`pending_writes`（待确认写入）、`save_offline_artifact`（`create_new`，且拒绝覆盖）。工作记录目录的创建与追加只存在于 `tests/verification/phase-worklog/worklog_store.py`（测试参考实现）与 Skill 文字说明中。**即当前落盘完全依赖模型自行调工具，产品层无保证。**
3. **随手记注入的 Skill 含周报规则。** `conversationPrompt.ts:99-121` 的 note 分支会注入第 1 个绑定 Skill 全文（最多 2500 字符）。weekly 第一个绑定是 `memory-agent`，全文含「四、周报（Report Mode）」四步，因此随手记也带着周报流程。
4. **report 模式不读本周记录。** `HomeChat.tsx:1370` 调 `composeConversationPrompt` 时 `material` 只有 `buildContext(turns, current)`，没有任何读 `库/工作记录/2026-Wxx.md` 的路径。用户说「整理本周工作」时，模型手里没有本周记录。
5. **无本地时区换算。** 宿主只有 `utc_parts/civil_from_days`（task_shell.rs:1132-1170，纯 UTC，不引入 chrono）；ISO 周计算只存在于测试脚本 `worklog_store.py:iso_week`。scheduler.rs 有 `utc_offset_minutes`（默认 480）可复用。
6. **库根已可解析。** `settings::effective_library_root`（settings.rs:371，优先级 env > 用户设置 > `~/OmniDesk/库`），已注册为 `task_library_root_info` 命令。工作记录应落在库根下，而不是 `prepare_workspace_dir` 返回的成果目录（后者是 `~/OmniDesk/workspace/artifacts`）。
7. **现有测试只测组装函数，不测界面默认态。** `verify_profile_rules_068.py:58/66` 与 `verify_skill_bindings_082.py:92` 都直接传 `mode`，因此接线错反仍全绿；`verify_profile_rules_068.py:82` 还断言「不传 mode = report」，改默认后该断言必须同步改。
8. `verify_ui_088.py`/`verify_ui_084.py` 不引用「随手记/整理成周报」文案，移除标签不会破坏这两个窗口测试；但 `tests/verification/desktop/evidence/od-arch-088/*.json` 的 AX 快照含这两个按钮名，属历史证据，按规则**不覆盖不改写**。

#### 二、方案（三步，对应用户三条需求）

**S1 意图判定：改由输入关键词决定，取消手动开关。**

- 新增 `desktop/src/workIntent.ts`（纯函数、无依赖，便于离线验证）：`detectWorkIntent(text)` → `"report" | "note"`。
- report 触发词按用户口径收敛为「明确要求生成周报或总结本周工作」：`生成周报`/`整理成周报`/`写周报`/`出周报`/`本周周报`/`总结本周`/`本周总结`/`汇总本周`/`本周汇总`/`这周总结`/`本周工作小结` 等，匹配前统一去空白并全角转半角。其余一律 `note`。
- 只在 `preset.key === "weekly"` 时启用；其它场景与普通对话不进入该判定，行为不变。
- 判定结果在发送时算一次，并作为可见提示显示在回复上方（例如「按随手记记录」/「按周报生成」），让用户知道这次走了哪条路，不需要标签。
- **删除** `HomeChat.tsx` 的 `noteMode` 状态、两按钮块（:2428-2450）、`noteMode` 依赖项，以及判断表达式里的开关。

**S2 随手记确定性落盘：宿主提供追加写入，模型只做缺项分析。**

- 宿主新增 `worklog_append`（`desktop/src-tauri/src/worklog.rs` + lib.rs 注册）：
  - 落点 = `effective_library_root()/工作记录/<ISO年>-W<周次>.md`，周次按本地时区（复用 `utc_offset_minutes`，默认 +480）计算 ISO 周。
  - 行为：文件不存在则建（含 `# 2026-Wxx` 标题 + 当日 `## YYYY-MM-DD` 段）；已存在则**只 `OpenOptions::append(true)` 追加**，绝不 truncate/重写；同日再记另起 `## YYYY-MM-DD（续）`。
  - 条目格式沿用现有文件既有约定：`- **事**：<原文>` + `  **去向**：本周报引用|inbox`（去向由前端按本轮判定给默认值，用户不需要回答）。
  - 写入前过红线扫描与隐私掩码（复用 `red_line_scan_with` + `mask_privacy`），命中红线则拒绝并返回原因，不静默落盘。
  - 返回结构化回执：`{ ok, path, week, date, appended: bool, was_new_file, masked: boolean, error }`。
- 前端 note 分支流程改为：
  1. 先调 `worklog_append`（原文，不经过模型）；
  2. 落盘成功后才调模型，提示词是**极短**的缺项分析指令（见 S3），注入的 Skill 只保留「记录与缺项」部分；
  3. 界面显示「已写入 库/工作记录/2026-Wxx.md」+ 模型对缺项的追问；
  4. 用户补充的下一条输入，同样先追加一条新条目再分析——天然是「逐条增量」。
- 落盘失败时**明确报错并停止**，不假装已记录（避免用户以为记下了其实没有）。

**S3 提示词拆分：note 不再携带周报规则。**

- `conversationPrompt.ts` 的 note 分支指令改为用户表述的语义：「已记录。只判断时间/地点/人物/事件/影响/感受里缺哪几项，一句话问用户是否补充；不要复述、不要扩写、不要生成周报。」不注入章节、铁律、周报流程。
- **实现方式：拆成两个 Skill 文件，不做运行时按标题切分。**（2026-10-26 用户问「拆文件 vs 代码按节裁剪哪个省 token」，实测后定为拆文件，理由见下「S3 取舍」。）
  - `skills/memory-agent/SKILL.md` 保留随手记职责：「一、记录」+ 样例 + 「二、提炼强度」+ 「三、去向怎么定」+ 新增「缺项六要素」（时间/地点/人物/事件/影响/感受，只问缺项）+ 「五、数据边界」+ 「六、自检」。删掉原「四、周报（Report Mode）」。
  - 新增 `skills/memory-report/SKILL.md`：只装周报职责（读当周记录、只取 `本周报引用`、五段模板、全文展示待确认、确认后才落盘 `库/周报/`、同名不覆盖）。
  - `samples/profiles/discipline.rules.json` 的 weekly 绑定改为 `["memory-agent", "memory-report", "knowledge-search"]`，`max_injected` 仍为 3。**anysearch 移出 weekly 预注入**，但仍随 `scripts/sync-skills.sh` 安装、仍在引擎 `available_skills` 清单里，模型需要联网核对时仍可自行加载（「未预注入 ≠ 不可用」，见 OD-ARCH-088 审查记录）。
  - `profile_rules.rs` 的 `ALLOWED_SKILLS` 增加 `memory-report`（该清单是白名单，不加则整份规则文件被拒）。`MAX_SKILL_BINDINGS_PER_SCENE` 与 `default_max_injected()` 不动。
  - `conversationPrompt.ts` 的 note 分支保持「只注入第 1 个绑定 Skill」不变（现在第 1 个是 memory-agent，正好是随手记部分）；report 分支注入全部 3 个，正好是 memory-agent + memory-report + knowledge-search。**不需要任何标题切分或多模式声明代码**——拆分放在数据层，注入规则一行不改。
  - 同步更新两处当前配置断言（属跟随配置变更，不是放宽）：`verify_skill_bindings_082.py` 的 `weekly_binds_anysearch` 改为断言「anysearch 已安装且在白名单内、但不在 weekly 预注入中」，`verify_env_085.py:130` 同理。历史证据 `tests/verification/**/evidence/*.json` 一律不改写。
- report 分支补上本周记录来源：前端在发送前按 `worklog_append` 同源逻辑读当周文件（新增只读命令 `worklog_read_week`），把内容作为 `material` 传入，模型不再空手写周报。

**S3 取舍（实测字符数，2026-10-26 ARCHITECT 脚本核实）：**

现状 `memory-agent/SKILL.md` 共 1534 字符，按节：头部 178、一·记录 200、日期样例 113、二·提炼强度 247、三·去向 246、**四·周报 384**、五·数据边界 104、六·自检 62。weekly 三个绑定字符数为 memory-agent 1534 + knowledge-search 2133 + anysearch 1487 = 5154。

| 做法 | 随手记注入 | 周报注入 | 额外代价 |
|---|---|---|---|
| 现状（单文件全文） | 1534 | 5154 | — |
| 代码按标题切分 | 1150 | **5154（一分不省）** | 切分代码约 40 行；标题改名即静默失配，须同步改代码并加断言 |
| **拆两个文件（选定）** | **1150** | **3845** | weekly 3 槽位重新分配；两处配置断言随改 |

（拆分后：随手记文件 1150 = 现有内容去掉「四、周报」；周报文件 562 = 头部 + 四。周报注入 1150+562+2133 = 3845。**1150 尚未含待新增的「六要素缺项」小节**，该节控制在 150 字符内，则随手记实际约 1300，仍比现状 1534 少约 230；周报侧不受影响。）

结论：**按节裁剪只对随手记有用，周报一分不省，还要背上标题耦合**；拆文件是两条路径同时省，且把复杂度留在数据层而不是代码里。所以拆。

**实施后的真实字数（2026-10-26 ARCHITECT 亲测，`len(SKILL.md)`）：** memory-agent 1432、memory-report 988、knowledge-search 2133、anysearch 1487。周报侧注入从 5154 降到 4553（−601）；随手记侧从 1534 变为 1432（−102），但多了「六要素只问缺项」这一职责。上文预算表里「随手记 1150 / 周报 3845」是拆分设计时的估算，实际写出来比估算多：随手记多了六要素整节，周报多了确认门、命名、数据边界与自检。以实施后实测为准。

另一条可选（此处不选）：随手记在 S2 之后由宿主确定性落盘，模型只剩「问缺哪几项」，理论上随手记可以不注入任何 Skill（省下 1432），weekly 绑定变成 `[memory-report, knowledge-search]` 再加运行时加载。不选的理由是那样必须把「不硬扩／去向／六要素」搬进代码里的提示词，用户就没法再像现在这样直接改 Skill 文本调行为，与用户「我想自己修改反馈逻辑和 Skill 内容」的意图相反。若实测发现随手记质量下降，再回来评估这条。

#### 三、影响范围（实际）

`desktop/src/workIntent.ts`（新增）、`desktop/src/composerIntent.ts` 或并入 conversationPrompt、`desktop/src/conversationPrompt.ts`、`desktop/src/shell/HomeChat.tsx`、`desktop/src/api.ts`（两个新命令的类型与封装）、`desktop/src-tauri/src/worklog.rs`（新增）、`desktop/src-tauri/src/lib.rs`（注册）、`desktop/src-tauri/src/profile_rules.rs`（白名单加 `memory-report`）、`samples/profiles/discipline.rules.json`（weekly 绑定）、`skills/memory-agent/SKILL.md`（删周报节、补六要素缺项）、`skills/memory-report/SKILL.md`（新增）、`tests/verification/desktop/verify_worklog_089.py`（新增）、`tests/verification/desktop/verify_profile_rules_068.py`（同步默认断言）、`tests/verification/desktop/verify_skill_bindings_082.py` 与 `tests/verification/phase-worklog/verify_env_085.py`（anysearch 绑定断言随配置更新）、`STATUS.md`、`PLANS.md`、`REVIEW.md`。

**不做：** 不改 runtime/（零差异）；不改模型路由与凭证；不改 `MAX_SKILL_BINDINGS_PER_SCENE`（仍 3）与 `max_injected`；不迁移/覆盖 `~/OmniDesk/库` 既有任何文件（测试只用 `/tmp` 隔离根）；不改动 `tests/verification/**/evidence/` 下历史证据；不新增依赖（Rust 侧不引入 chrono，时区换算沿用现有算法）；不新增第四种输入模式声明（weekly 绑定本身即模式区分）。

#### 四、验收标准

1. **意图判定（脚本级，合成文本）：** 「生成周报」「帮我总结本周工作」「本周工作汇总一下」→ `report`；「下午在古田听课，题目是古田会议」「今天开了个会」「记一下：周三交材料」→ `note`。全角/空格/中英标点混写不影响判定。
2. **界面无标签：** 工作记录场景输入区 AX 树中不存在「随手记」「整理成周报」两个按钮（真实窗口 AT-SPI 断言）；场景切换、发送、附件、语音等既有按钮不受影响。
3. **落盘正确（真实窗口 + 隔离库根）：** 连续输入两条随手记 → `2026-Wxx.md` 出现两条独立 `- **事**` 条目，文件只增长、旧字节不变（前后字节比对）；第二条不覆盖第一条；同日第二次记录使用「（续）」段。
4. **跨周跨日：** 注入不同日期/时区 → 文件名与文件内日期按本地时间正确分周分日。
5. **note 提示词瘦身：** 汇编后的 note 提示词不含「本周概述/事项进展/问题与风险/下步计划/待确认事项」任一章节名，不含 7 条写作铁律，不含「Report Mode」四步；长度较改造前同输入显著下降（给出实测字符数）。
6. **缺项分析行为：** note 回复只问缺失要素，不复述原文、不扩写；用户补充后再记一条新条目（不是改原条目）。
7. **report 有米：** 当周文件存在时，report 提示词包含本周记录内容；文件不存在时如实说明「本周还没有记录」，不虚构。
8. **红线与隐私：** 含红线词输入 → 拒绝落盘并说明原因；身份证/手机号 → 落盘时为掩码形态，回执标注已掩码。
9. **回归：** tsc 0；`verify_profile_rules_068.py`、`verify_skill_bindings_082.py`、`verify_intent_088.py` 全绿（068 的「默认=report」断言按新语义改为「不传 mode 时 note 组装仍不含周报章节」）；cargo 全量 0 failed；`cargo test worklog` 覆盖追加不覆盖、同日续段、跨周命名。
10. **Skill 拆分（字符级）：** 随手记提示词不含 `Report Mode`、`本周概述`、`事项进展`；周报提示词含两者之一且不含 six-element 追问句；`memory-agent` 与 `memory-report` 各自 ≤ 2500 字符截断线（断言沿用 `verify_worklog_085.py` 的 budget 检查）；weekly 绑定 = `[memory-agent, memory-report, knowledge-search]`，三者均在 `ALLOWED_SKILLS` 内，anysearch 仍在白名单但不在 weekly 预注入。

#### 五、风险与取舍

- 关键词判定会有误判边界（例如「总结一下这次会议」算不算周报）。计划采用保守口径：只有明确指向「周报/本周」的措辞才进 report，其余都当随手记；判定结果在回复上方明示，误判用户可当场看出。若实测仍有误判，可按证据调整词表，不改架构。
- 随手记改为「先落盘再分析」，意味着模型调用失败时记录仍已在库里（这正是「先把事实留住」的设计意图），界面需明确区分「已记录，分析失败」这两种状态。
- 落盘由宿主做，模型不再直接写文件，行为与 `memory-agent` 原文「由模型追加」不同；Skill 需同步说明「写入由工作台完成，你只做缺项分析」，否则模型可能重复写一次。这是本轮必须同步改 Skill 的原因。
- 报告模式的「本周记录来源」只读当周一个文件，不做跨周汇总；跨周需求另行立项。

#### 六、验收进度（2026-10-26 ARCHITECT 自测）

| 验收项 | 状态 | 证据 |
|---|---|---|
| 1 意图判定 | 通过 | `verify_worklog_089.py` 编译真实 `workIntent.ts`，61/61 |
| 2 界面无两标签 | 待真实窗口实测 | 静态断言已无 `noteMode` 与按钮文案；AT-SPI 实测未做 |
| 3 落盘只增长不覆盖 | 脚本级通过，窗口级待实测 | `worklog.rs` 单测：追加不覆盖、字节前缀自证 |
| 4 跨周跨日 | 通过（单测） | ISO 周已知值、跨周分文件、跨日续段 |
| 5 note 提示词瘦身 | 通过 | 不含五段章节与写作铁律；随手记 1684 / 周报 5627 |
| 6 缺项分析行为 | 待真实模型实测 | 提示词与 Skill 已约束，模型输出未验证 |
| 7 report 有米 | 脚本级通过，真模型待实测 | 当周记录并入 material；无记录时要求如实说明 |
| 8 红线与隐私 | 通过（单测 + 读码） | 命中红线拒绝且不回显原文；落盘前 `mask_privacy` |
| 9 回归 | 通过 | tsc 0、build 0、pytest 89 passed、cargo 294 passed 0 failed |
| 10 Skill 拆分 | 通过 | 双向断言、字符数实测、weekly 三绑定、anysearch 仍在白名单 |

### OD-ARCH-087：正式启动器 release 包误判开发模式（Connection refused，已修复待验收，2026-10-25）

> **状态：代码已修复并在隔离显示环境实测启动成功；未提交、未推送、未验收。** 本条目在排查用户实测卡死问题时补记。

**症状**：scripts/omnidesk.sh 启动后窗口报 Could not connect to 127.0.0.1: Connection refused。

**根因（文件事实）**：desktop/src-tauri/Cargo.toml 没有声明 Tauri 的 custom-protocol 特性。Tauri 只按该特性判定生产模式，因此即使 cargo build --release，生成的二进制仍按开发模式加载 build.devUrl（http://127.0.0.1:5273）；正式启动器不起 dev server，连接必然被拒。

**修复**：Cargo.toml 增加 [features] default=[] 与 custom-protocol=["tauri/custom-protocol"]；scripts/omnidesk.sh 的 release 构建带 --features custom-protocol，并写 target/release/.build-features 标记；--no-build 遇到标记不符的旧二进制明确报错退出，避免静默复用坏包。

**已验证**：构建日志 cargo:dev=false；二进制内嵌前端资源；Xvfb 真实窗口截图 /tmp/odverify-shots/boot.png 界面完整渲染（无 dev server 也能显示）；正式启动器可正常启动。

**未做**：未改 runtime/，未动模型路由与凭证；未提交推送。

### OD-ARCH-088：场景提问卡闭环 + 场景 intent 透传 + Skill 绑定可见性（已验收通过，2026-10-25）

> **状态：已验收通过（REVIEW.md 顶部，2026-10-25）。** ARCHITECT 独立复跑全部通过：pytest 89/30、tsc 0、npm build、cargo 全量 0 failed、verify_intent_088 22/22、verify_ui_088 真实窗口 30/30、082 绑定 35/35、085 119/119 与 085 环境 22/22。触发来源：用户实测「工作记录」场景时界面停在「生成中」，提示「引擎在等待一项未被识别的确认（user-questions/request）」。**开工基线：** main，HEAD 0d17b1f，工作区既有未提交/未跟踪改动全部保留；冻结 runtime/；desktop/ 与 engine/omnidesk_engine/ 解冻。**遗留决策：** Profile 的 tool-ask-user 仍是 legacy 模式（无超时自动继续），是否改 timed 需另立计划获批。

#### 一、实测问题的定性（ARCHITECT 独立查证，2026-10-25）

1. **不是窗口卡死，是任务在等一个没人能回答的提问。** 任务状态文件 ~/OmniDesk/app/engine-state/omni-b80f4367-xxxx.json：intent=material_writing、state=interrupted。会话日志 session-949cefc3-xxxx 显示模型先以 skill 工具加载 memory-agent 成功，随后调用 ask_user_question，问「初稿是否要先追加到 库/工作记录/2026-W43.md？」，给出两个选项。
2. **外壳没有回答通道。** engine/omnidesk_engine/web_api_adapter.py:370-375：除 approval/request 外的所有 waterfall 一律走 _on_engine_ask()；该方法（约 :1739-1770）只发一条 PROGRESS kind=engine_ask，注释写明「不自动应答」。前端 desktop/src/shell/HomeChat.tsx:759 把它渲染成「未被识别的确认」。
3. **应答协议已存在，缺的只是接线。** 审批走 $events/result（respond_approval()，约 :1830 起），payload 为 {clientId, eventId, outcome:{kind:"result", value}}；提问的答案结构见 runtime/deepseek-harness/packages/interaction/user-questions/src/types.ts:56-69：{answers:[{id, selected:[...], custom?}]}。同一 $events/result 通道即可回答提问。
4. **三个 Skill 没有「默认绑定」，原因是用户实盘规则包仍是 v1。** 用户文件 ~/OmniDesk/app/profiles/discipline.rules.json：schema=omnidesk.profile-rules.v1，**无 skill_bindings**（mtime 10-23 13:10）；出厂包 samples/profiles/discipline.rules.json 才把 weekly 绑到 memory-agent / knowledge-search / anysearch。注入逻辑见 desktop/src/shell/HomeCanvas.tsx:300-315：只有 skill_bindings[preset] 存在，才会把 SKILL.md 正文预拼进提示词。**但技能仍出现在引擎的 available_skills 清单里，模型可自行加载——本次就是它自己加载了 memory-agent。「未预注入」不等于「不可用」。**
5. **另一个真实缺陷（不导致卡死）：场景 intent 未透传。** HomeChat.tsx:1176 调 submitTask() 未传 intent，desktop/src/api.ts:221 与宿主 desktop/src-tauri/src/lib.rs:426 双双默认成 material_writing；因此工作记录任务在台账与成果归类里被记成「文案写作」。

#### 二、目标与范围（三项，缺一不可）

**A. 提问卡闭环（核心）**

1. 适配层解析 user-questions/request 的 request.questions，把提问登记为该任务的待答项，并把结构化题目（id / header / question / detail / options 的 label 与 description / multiSelect）随进度事件下发给界面。
2. 新增回答命令，逐条链路：前端 → Tauri 命令 → 引擎 JSON-RPC（omni.question_answer）→ 适配层 → $events/result，value 为 {answers:[{id, selected, custom?}]}。
3. 前端渲染提问卡：题干、选项按钮（multiSelect 时多选）、可选自定义文本、提交按钮；提交中禁用，失败保留卡片并显示真实原因（沿用审批卡「宿主确认送达才改状态」的口径）。同时保留「取消本次任务」出口。
4. **不自动选择默认项、不自动跳过、不伪造已答。** 引擎侧超时或取消时，卡片必须落成可见的「已超时/已取消」，不能一直显示可答。
5. **隐私边界**：题干与答案只留在本机内存与界面；任务状态文件 _record_payload 只存类型、id 与计数（沿用既有「不存请求正文」口径）；app.log 不写题干与答案正文。题面、选项、自定义文本均设上限（题面与详情 ≤ 2,000 字符，选项 ≤ 12 个且标签 ≤ 200 字符，自定义 ≤ 4,000 字符），超限如实报错，不静默截断。
6. 非提问类未知 waterfall 仍走既有 engine_ask 提示，不回归成静默。

**B. 场景 intent 透传**

HomeChat 提交任务时按当前生效场景传 intent = SCENES[effectiveTag].scenario；无场景（未分类或自定义）时维持既有默认值。附带一条断言：工作记录场景提交的 intent 为 weekly_report。

**C. Skill 绑定可见性与启用**

1. 设置页「规则资产」在规则包为 v1（无 skill_bindings）时明确提示：出厂规则包含场景 Skill 绑定，当前规则包为旧版，未启用预注入；模型仍可按需自行加载技能。点「恢复默认规则包」可启用。
2. 恢复动作**仍由用户显式点击**，走既有 profileRulesRestoreDefault（已有备份机制）；**不做静默迁移、不自动改写用户规则文件**。
3. 对话区在场景预设存在但未注入绑定 Skill 时，如实说明「本场景技能未预注入（规则包为旧版），模型可按需加载」，让用户能分辨「没绑定」与「技能不可用」。

#### 三、实施步骤

- Step 1：读 web_api_adapter.py 的 waterfall 分发与 $events/result 调用形态，读 user-questions 契约，确认回答 value 的精确形状；写适配层单测（合成 waterfall 帧 → 待答项登记 → 回答帧 payload 断言 → 超时与取消清理）。
- Step 2：加 serve.py 的 omni.question_answer、Rust 桥命令与 Tauri 命令注册，逐层测试。
- Step 3：前端提问卡（组件、事件订阅、提交态、失败态），沿用现有审批卡的视觉语言与状态口径。
- Step 4：HomeChat 传场景 intent；设置页规则资产提示与对话区未注入说明。
- Step 5：回归 cargo test、npm run build、tsc --noEmit、pytest tests、既有 desktop 验证脚本；真实窗口跑一次提问卡端到端（隔离数据根加合成提问，不使用真实业务材料）。

#### 四、验收标准

1. 合成提问到达后界面出现可操作提问卡；提交后引擎收到 $events/result，任务继续至终态（脚本级证据，含帧断言）。
2. 不提交而放任超时或取消时，卡片与任务状态一致落成「超时/已取消」，不留永久「生成中」。
3. 状态文件与 app.log 中无题干、选项、答案正文（以文件内容断言为证）。
4. 超长题面、选项、自定义文本被明确拒绝，不静默截断。
5. 工作记录场景提交的 intent 为 weekly_report（单测或集成断言）。
6. v1 规则包下设置页出现旧版提示；恢复出厂包后 weekly 三项绑定生效（沿用 082 注入验证脚本）。
7. runtime/ 零差异；cargo test、npm run build、tsc、pytest 全过；STATUS.md 记录实测命令与结果；不提交、不推送。

#### 五、风险与边界

- $events/result 的 value 形状依赖 runtime 版本；若与探针不符，适配层必须报错并在 STATUS 如实记录，不得猜测或伪造「已答」。
- 提问卡引入新的 IPC 命令，需与既有审批通道共用 _ApprovalChannel 的连接与生命周期，避免第二套事件流。
- 前端新增卡片涉及界面，动工前必须用 screenshots-design-inspiration 搜 2–3 个同类问答卡参考，借鉴要点不超过 2 句写入任务记录（OD-ARCH-035）。
- 真实业务材料不进测试、不进仓库、不进日志；端到端验证一律用合成提问文本。

### OD-ARCH-086：anysearch 白名单扩容 + 库根绑定（两项，2026-10-25，已验收通过）

> **状态：已验收通过（REVIEW.md，2026-10-25）。** 用户已确认「两项都做」。批准后派 OpenDesign/deepseek-v4.1-flash（high）子代理执行。**开工基线：** main，HEAD 0d17b1f，工作区既有未提交改动一律保留；不修改 runtime/。src-tauri/ 本轮解冻。

#### 一、目标

1. 将 anysearch 纳入白名单并绑定到 discipline 的 weekly 场景，使工作记录场景具备联网检索能力。
2. 建立库根绑定：库根 = ~/OmniDesk/库，使 Skill 写入库的文件可被应用成果列表与 @ 引用读取；默认知识库根从 ~/YvanAI/knowledge 切换到库根（旧路径不迁移、不覆盖、不删除）。

#### 二、已核实事实（ARCHITECT 复核，2026-10-25）

1. desktop/src-tauri/src/profile_rules.rs:29 ALLOWED_SKILLS 为六项编译期固定清单，校验失败整份规则包判非法（:238-239）；MAX_SKILL_BINDINGS_PER_SCENE=3（:38），weekly 加 anysearch 后为 3 个，不超限。
2. anysearch 实体在 ~/.codex/skills/anysearch/（JSON-RPC 2.0 单端点 CLI，runtime.conf 指向 python3 scripts/anysearch_cli.py），不在本仓 skills/ 六目录中，也未被 sync-skills.sh 同步。DSH 靠隔离数据根的 skills 目录发现技能。
3. task_shell.rs:2455 knowledge_root() 默认返回 $HOME/YvanAI/knowledge（:2469）；reference_roots()（:2474）= 成果根 + 知识库根，是 @ 引用的来源。
4. settings.rs 已有 artifact_root（成果根）与 ThreadCategory.vault（对话分类 vault 绑定）机制；settings schema omnidesk.settings.v1 且 deny_unknown_fields，新字段必须 #[serde(default)] 向后兼容。
5. 数据根已有 ~/OmniDesk/库/{工作记录,周报,知识库/{概念,方法},inbox} 及索引头（085 建成）。

#### 三、实施步骤

- Step 1（anysearch 发现机制）：在仓库 skills/ 下新建薄包装 skills/anysearch-wrapper/（或按 BUILDER 判断的更合规方式），SKILL.md 转发调用到本机 ~/.codex/skills/anysearch 的 CLI（路径可在包装层读环境变量或配置，不得复制其全部脚本进仓）；验证 DSH 隔离根经 sync-skills.sh 同步后能发现并执行一次 search 冒烟调用。若 DSH 发现机制不支持该方式，如实记录并回退为「anysearch 不入 weekly 绑定」，不得改 runtime/。
- Step 2（白名单）：profile_rules.rs ALLOWED_SKILLS 增加 "anysearch"（若 Step 1 用包装名则加包装名）；samples/profiles/discipline.rules.json weekly 绑定加 anysearch（共 3 个）。
- Step 3（库根绑定）：Settings 增加 library_root: Option<String>（#[serde(default)]），默认解析为 ~/OmniDesk/库；沿用 artifact_root 的 assert_artifact_root_allowed 同等边界校验（绝对路径、拒绝 ~/.dsh 等保护路径）。knowledge_root() 默认值改为库根（环境变量 OMNIDESK_KNOWLEDGE_ROOT 覆盖优先级不变）；reference_roots() 追加库根（去重）。设置页增加库根显示与修改入口（desktop/src/，样式随现设置页）。
- Step 4（回归）：cargo test 全量（先 eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)"）；npm run build；sync-skills.sh --check 无漂移；旧 settings 文件（无 library_root 字段）可正常加载；写一条集成测试覆盖「库根内文件出现在 reference_roots」。

#### 四、验收标准

1. cargo 全量通过（含新增测试）；npm build 通过；sync-skills.sh --check 无漂移。
2. 出厂默认规则包解析通过且 weekly 含 anysearch；anysearch 冒烟 search 有真实返回或如实记录不可用原因（无 API key 匿名限流可接受）。
3. 设置页可见并修改库根；修改后 @ 引用可列出库根内文件；旧 YvanAI 路径零改动。
4. STATUS.md 记录实测命令与结果；不提交、不推送。

#### 五、风险

- anysearch 经 DSH 发现机制的真实可用性未验证——Step 1 允许回退。
- knowledge_root 默认值切换影响 @ 引用来源——旧数据不删不迁，仅指向变化，风险低但需在 STATUS 说明。
- 设置 schema 兼容——必须 #[serde(default)]，验收含旧文件加载测试。

### OD-ARCH-085：工作记录场景全链路——读/存/取/输出与知识库省 token 联动（已验收通过，2026-10-25）

> **状态：已验收通过（REVIEW.md 顶部，2026-10-25）。两处裁定待用户决定：anysearch 白名单、库根绑定。** 设计已与用户逐条确认（索引卡+两跳检索可行；零碎条目 AI 三选一处理不需用户确认；周报复核后落盘；专项写作归文案场景）。批准后派 OpenDesign/deepseek-v4.1-flash 子代理执行。**开工基线：** main，HEAD 0d17b1f，工作区 12 项未提交改动一律保留；runtime/ 无差异为前提。

#### 用户定调（六条，均为硬约束）

1. 知识库是给 AI 读的，不是给人看的——省 token 优先，采用**索引卡 + 两跳检索**。
2. 读/存/取要快、少打扰；**审核只发生在输出（周报落盘）一处**。
3. 工作记录**每周一个 md 文件追加**，不拆散；工作记录不进知识库，**只有复核后的周报才进**。
4. 周报名称/模板/记录文档规则**写死在 Skill 文件里**，改文件即升级，不改代码。
5. 发言稿、简报等专项写作归文案写作场景，本场景不做。
6. 零碎记录 AI 三选一处理（成知识卡 / 进 inbox / 归档），**不需要用户确认**。

#### 存储结构（Obsidian 兼容，绑定用户 vault；数据根内为默认路径）

- 库/工作记录/2026-W43.md ← 每周一个文件，只追加
- 库/周报/2026-W43-周报.md ← 用户复核后落盘，此后可被知识库检索
- 库/知识库/索引.md ← 全库卡片目录，AI 第一步只读这一个文件
- 库/知识库/概念/ 与 库/知识库/方法/ ← 知识卡片
- 库/inbox/ ← 静默放入，不打扰

**索引卡规则（省 token 核心）：** 索引.md 只记「标题 + 标签 + 一句话摘要」，每张卡一行。模型读取顺序：索引 → 按标签定位 → 只加载命中的 1–2 张卡 → 禁止整目录扫描/整库加载。卡片为机器阅读优化：一卡一概念、正文 ≤300 字、自包含、用双链关联相关卡。卡片结构：frontmatter（tags/sources/date）+ 一句话定义 + 展开 + 相关链接；外部依据附来源，自判断标「待核实」；写卡必须同步更新索引.md；写卡前先查索引防重复建卡。

#### 三个 Skill（改数据根 ~/OmniDesk/runtime/dsh-home/skills/ 下现有文件，改文件即生效）

1. **memory-agent（条目整理，重写现有文件）**：会话 → 条目，追加进当周 md。一句话记录不硬扩；多轮讨论提炼「问题 → 结论」；长文只记要点。每条标记去向（本周报引用 / 知识卡 / inbox / 归档）。只追加、按行合并，禁止整文件重写。
2. **knowledge-search（知识卡片，改造现有文件）**：按上述索引卡规则重写，含卡片格式、索引维护、防重复、两跳检索顺序。
3. **周报生成（memory-agent 的 Report Mode 或独立文件，由 BUILDER 按 082 已有绑定契约决定，不新增绑定槽位）**：写死规则 = 读当周 md + 模板 → 起草 → 界面展示全文 → 用户确认后才落盘到 周报/。模板命名规则改 Skill 文件即可升级。

#### 两个集成（按官方手册核实过的结论）

- **Obsidian CLI**：是 Obsidian 官方桌面版 1.12+ 内置 CLI（设置中开启），不是 DSH npm 插件。用途：建卡、移动文件、双链存在性检查、列 inbox。未装 Obsidian 时降级为普通文件读写，功能不受阻。不进 D-030 冻结的 npm 插件清单。
- **anysearch**：本机已存在完整 Skill 包（/home/kevin/.codex/skills/anysearch/，SKILL.md v2.1.0 + scripts，JSON-RPC 2.0 单端点 CLI，无 key 可匿名低限额）。以 Skill 方式集成进 Discipline Profile，不进 npm 插件清单。websearch 已有（profile 内 web-search-deepseek），不重复装。

#### 实施步骤

1. 重写 memory-agent、knowledge-search 两个 Skill 文件；按绑定契约落实周报生成（Report Mode 或独立 Skill 文件）。
2. 在数据根初始化目录骨架（工作记录/周报/知识库/概念/方法/inbox）与空的 知识库/索引.md；不迁移、不覆盖任何已有用户数据。
3. 把 anysearch 以 Skill 方式挂入 Discipline Profile 的 Skill 注入机制（走 082 的 skill_bindings 契约，遵守每场景 ≤3 的上限；若上限冲突，本场景优先 knowledge-search / memory-agent / anysearch，周报生成并入 memory-agent）。
4. Obsidian CLI 做能力探测：可用则走 CLI，不可用则降级普通文件读写；两种路径都要有行为。
5. 写合成评测用例：一句话记录、多轮讨论记录、长文要点、周报起草→确认→落盘、建卡防重复、索引同步、两跳检索命中、无 Obsidian 降级。副本隔离数据根上全流程跑通。

#### 验收标准

- [ ] 当周 md 只追加不重写（含「两次追加后文件为纯追加结果」断言）。
- [ ] 索引.md 与卡片集始终一致（建卡/改卡后索引条目与卡片数对应）。
- [ ] 两跳检索：模型第一步只读索引，第二步只加载命中卡；评测断言未发生整目录读取（以读取记录为证）。
- [ ] 周报未经用户确认不落盘；确认后文件出现在 周报/ 且命名符合写死规则。
- [ ] 零碎条目三选一去向无需用户确认即完成。
- [ ] 无 Obsidian 环境下全流程降级可用。
- [ ] runtime/ 无差异；不提交不推送；STATUS.md 记录实测命令与结果。

#### 风险与边界

- Skill 注入受 082 每场景 ≤3 绑定上限约束，anysearch 挤占名额是设计取舍，用户已知情。
- Obsidian CLI 依赖用户本机 Obsidian ≥1.12 并手动开启 CLI；未开启不阻塞功能，只损失双链校验等增强。
- 真实模型端到端质量（提炼好坏、卡面质量）不在本轮验收内，待用户实测反馈迭代。


### OD-ARCH-084：会话内搜索的命中定位、逐处切换与焦点保持（已验收通过，2026-10-25）

> **当前状态：已验收通过（见 REVIEW.md 顶部终审）。** 用户 2026-10-25 批复「批准执行084」，BUILDER 按 `docs/BUILDER-Task-Instruction-Template.md` 完成后，ARCHITECT 独立复跑 `verify_ui_084.py` 21 项全 pass、failures=[]、exit 0，并回归 082（17/17）、083（17/17）、tsc/build/pytest 75/cargo 261。审查中退回修复两项：正则元字符致整窗黑屏（改为 `indexOf` 字面量切分）、未搜索态常驻 35vh 空白（改为检索态条件生效）。**尚未做真实模型端到端实测（OD-ARCH-080 清单），本轮未提交、未推送。** 以下原批准口径与验收标准保留作存档。

**需求来源：** 用户 2026-10-25 实测反馈「搜索功能没法自动聚焦到已搜索到的词上，而且点上下箭头时切换得不顺」，要求先参考 DSH 原生搜索、再结合当前设计综合分析后修复。分类：**缺陷修复 + 交互打磨**（082 S8 的实现未达可用标准）。

**开工基线：** 分支 main，HEAD `0d17b1f`，工作区干净（081/082/083 已分批提交并推送至 `origin/main`）；`git status --porcelain runtime/` 为 0。本计划不含提交、推送、打标签、改 Profile、改模型路由。

#### 已核实事实（分级）

**文件事实（读码）：**

- `desktop/src/shell/HomeChat.tsx:317-330`：`searchMatches` 只记 `{ messageId, index }`，**不记命中词在消息内的位置**。
- `HomeChat.tsx:336-344`：滚动目标是整条消息容器 `#chat-message-<id>`，`scrollIntoView({ block: "center" })`。同一消息内多个命中共享同一目标 → 切换时滚动是幂等操作。
- `HomeChat.tsx:1654 / 1687`：当前命中只给**整条消息**套 `ring-2 ring-amber-400`。长消息里这条环表现为左右两条竖线，命中词毫无标识。
- `desktop/src/shell/MarkdownText.tsx:63-100`：`highlightTextChildren` 只处理 `p`/`li` 的**直接字符串子节点**，所有 `<mark>` 同一外观、无 id / data 属性，无法区分「当前项」与「其余项」，也无法被查询定位。标题、表格单元格、引用、行内代码不参与高亮。
- `desktop/src/shell/TopNav.tsx:512-540`：上一处/下一处/清空按钮只有 `onClick`，**没有 `onMouseDown` 阻止默认行为**，点一下焦点即离开输入框；三个按钮还带原生 `title`（WebKitGTK 另弹系统提示框，与 083 已消除的同类问题同源）。
- `desktop/src/App.tsx:197-205`：`searchCurrentIndex` 只在改查询时归零；`searchMatchCount` 变化时不做夹取。

**脚本运行（真实 Tauri 窗口，Xvfb 一次性显示，合成数据，非真实模型）：**

| 探针 | 场景 | 观察结果 |
| --- | --- | --- |
| `:104` 探针 1 | 长消息内唯一命中词位于末尾 | 计数 `1/1`；视口内琥珀连通块只有一个 1×505px 竖条，即整条消息的右侧描边；命中词在视口外，看不见 |
| `:104` 探针 1 | 同一长消息内 8 处命中 | 连按回车 4 次计数 `1/10→5/10`；视口内命中块 y 只在 329/475/622 三点间 ±1px 摆动，等于在两个「整条消息居中」位置之间抖动，**没有逐处推进**；同屏 3 个 mark 外观完全一致 |
| `:107` 探针 4 | 用 AT-SPI 取按钮真实外接矩形后点击「下一个」 | 1/60 → **2/60**（按钮本身工作正常；探针 2 的「点一下跳到 60/60」是坐标点偏，作废）；随后按 Enter → 3/60；**再键入 `x`，计数仍是 3/60** → 焦点已被按钮接管，键盘输入丢失 |

**读码推断（未单独复现，随修复一并消除）：** 计数按原文统计、高亮只覆盖 `p`/`li` 的部分子节点，因此标题/表格/代码块里的词会出现「计数有、正文没有」；连续 `smooth` 滚动会互相打断。

**结论（三条根因，均有上面证据支撑）：** ①滚动目标是消息而非命中词；②同一消息内所有命中共享同一目标，切换必然幂等；③当前项没有独立视觉标识，且焦点会被箭头按钮抢走。

#### 定案方案

1. **命中项成为可定位的实体。** `MarkdownText` 的高亮从「只认 `p`/`li` 直接字符串」改为**递归遍历文本子节点**，覆盖标题、段落、列表、引用、表格单元格、行内代码；每个 `<mark>` 输出 `data-search-hit`（消息内从 0 递增，单次渲染内局部计数，不跨组件共享），并统一 class `md-search-hit`。围栏代码块（`pre`）不参与高亮，并在计数中一致排除，避免「计到但看不到」。
2. **计数以渲染结果为准。** `HomeChat` 用 `useLayoutEffect`（依赖 query / currentIndex / messages）在 `.chat-scroll` 内按 DOM 顺序查询 `[data-search-hit]`，以此作为全局命中序列并上报长度。计数与可见高亮由此天然一致（含 Markdown 结构调整、流式增长）。`App` 侧在 `searchMatchCount` 变化时夹取 `searchCurrentIndex`，杜绝 `3/2` 这类越界。
3. **当前项两级视觉。** 当前命中独占 `data-search-active`（在 `useLayoutEffect` 中标注，先于绘制，重渲染后立即重标）：当前项用琥珀 300/400 系实底（蓝通道 < 120，保持 082 脚本既有判据的含义），其余命中降为更浅的琥珀 200 系。**删除整条消息的 `ring-2`**，长消息不再出现两条竖线。
4. **滚动换成按命中词定位。** 以 `.chat-scroll` 容器为基准（沿用 `HomeChat.tsx:1627-1636` `jumpToTurn` 的几何写法）计算目标 `scrollTop + markRect.top - containerRect.top - (可视高 × 0.32 - 半行高)`，把命中词放到视口约 1/3 高度，留 ≥24px 安全边。命中已在安全区内时不动，避免来回抖。切换时先 `behavior:"instant"` 落位取消上一次动画，再按需 `smooth`；`prefers-reduced-motion` 下全程 instant。
5. **一次短强调，可被用户打断。** 滚动停稳（`scrollend`，或 150ms 无滚动）后给当前项加一次约 700ms 的单次动画（底色/描边过渡，不改布局宽度），随后撤除；`wheel`/`touchstart`/`pointerdown`/方向键任一发生即取消。参考 DSH `experimental/session-inspector/.../reveal.ts`：先精确滚动、停稳再闪、不向内容节点插元素、减少动画时直接到位。
6. **焦点不再被箭头抢走。** 上一处/下一处/清空三个按钮加 `onMouseDown={(e) => e.preventDefault()}`，点击后光标仍留在输入框可继续键入；移除这三个按钮的原生 `title`，改用 `aria-label`，消除系统圆角提示框。Enter = 下一处、Shift+Enter = 上一处，循环方向与按钮一致。0 命中时按钮 `disabled`，按钮与 Enter 均为空操作。

#### 影响范围（预计）

`desktop/src/shell/MarkdownText.tsx`、`desktop/src/shell/HomeChat.tsx`、`desktop/src/shell/TopNav.tsx`、`desktop/src/App.tsx`、`desktop/src/shell/HomeCanvas.tsx`（仅透传，如无需要不动）、`desktop/src/markdown.css` 与 `desktop/src/theme.css`（新增命中两级与短强调样式）、新增 `tests/verification/desktop/verify_ui_084.py`、`tests/verification/desktop/evidence/od-arch-084/`、`STATUS.md`、本文件、`REVIEW.md`。

**不做：** 不改 `runtime/`、Profile、凭证、模型路由、DSH 数据根；不改 `verify_ui_082.py`/`verify_ui_083.py`，不覆盖 081/082/083 证据；不新增依赖；不重做顶栏布局与配色；不引入查找面板/侧栏等新容器。

#### 验收标准（每项都要有测量值与阈值）

1. **唯一命中可见：** 长消息内唯一命中词位于末尾时，搜索后当前命中块的像素矩形完整落在 `.chat-scroll` 视口内（上下各留 ≥24px），中心 y 位于可视区 25%–45%；截图为证。
2. **逐处推进：** 同一长消息 8 处命中，连按「下一处」7 次，当前命中块中心 y 单调递增，每次位移 > 单行高（~24px）；计数严格 `1/N…8/N`。反向连按「上一处」对称。
3. **循环与边界：** 末项按下一处回到首项，首项按上一处到末项；0 命中显示 `0/0` 且两个箭头按钮 `disabled`。
4. **焦点保持：** 用 AT-SPI 取「下一个」真实外接矩形后点击，计数 `k→k+1`；随后键入 `zzz`，计数变为 `0/0`（证明字符进入输入框）。
5. **两级可辨、无竖线：** 同屏 ≥3 命中时，像素扫描同时存在当前项色族（蓝通道 < 120）与非当前项浅色族；且不存在任一 x 列上跨度 >300px 的琥珀像素柱（整条消息描边已去除）。
6. **计数与高亮一致：** 同一个词分别出现在标题、表格单元格、行内代码中，计数与可定位命中数一致，两处不同命中能把视口滚到不同位置。
7. **Esc 清空：** 高亮像素归零。
8. **回归：** `tsc --noEmit` exit 0；`npm run build` exit 0；`cargo test --offline -q --lib` ≥261 passed；`python3 -m pytest tests -q` 全绿；`verify_ui_082.py` 全部断言仍通过（其琥珀阈值含义不变）；`verify_ui_083.py` 通过；`git status --porcelain runtime/` 为 0 行。

#### 设计参考（OD-ARCH-035 要求，已用 `screenshots-design-inspiration` 检索）

关键词 `developer tool text editor find bar`、`document reader in-page search highlight`；参考 Warp（密集工具条里的等宽小计数 + 上下箭头，不抢正文）、Claude Design（对话流里用底色深浅区分当前项）、Condens（文档型阅读区的留白与单色 mark）。

**借鉴要点（≤2 句）：** 查找状态收进顶栏同一个控件里，计数用等宽小字、当前项只比其余命中深一档，不额外加浮层；滚动先把命中词带到视口安全区，停稳后只做一次短强调，用户一滚动就让位。

#### 风险与停止条件

- 若 DOM 计数在流式渲染中与视觉出现跳变，改为在滚动容器附近加 `aria-live="polite"` 的稳定计数并记录差异，不靠放宽断言掩盖。
- 若按命中词定位在 WebKitGTK 下出现连续切换抖动，保留「instant 落位」为默认并记录 `smooth` 的实测差异，不退回整条消息居中。
- 若两级配色在深色问句气泡上对比不足，仅调整该 tone 下的 mark 配色，不改气泡底色与布局。
- 发现需要改 `runtime/`、Profile、凭证或模型路由才能解决时，停止并回报，不自行扩范围。
### OD-ARCH-083：顶栏品牌标换成 OmniDesk 自有 Logo + 轮次预览卡改直角并统一视觉（已执行并复核通过，2026-10-25）

> **当前状态：** 已验收通过（REVIEW.md 顶部终审）。自有 Logo 32×32 入顶栏并做细边过渡、整条顶栏节奏未变；轮次预览卡四角直角、白底细描边小字号，系统原生圆角小窗已消除，键盘 Tab / Shift+Tab 均能驱动卡片。验证脚本经两轮返工，17 项断言有可复核的测量值与阈值。两项遗留限制（`PREVIEW_H` 仍为 96、几何扫描为启发式）登记在 REVIEW，不阻断本轮。本节保留原批准口径与验收标准，作为该计划的存档。

**批准依据：** 用户 2026-10-25 提出两条 UI 小改动，同日在看到 A/B 两版视觉对照后明确答复「A 款」，并追加要求：**要考虑新 Logo 如何融入、过渡到导航条其它内容，保持整体美观**。该追加要求属本计划内的视觉融合约束，不扩大文件范围。**开工状态：** 分支 main，HEAD `0ac9edf`，工作区 43 项既有未提交改动全部保留，`git status --porcelain runtime/` 为 0。本计划不含提交、推送、打标签、改 Profile、改模型路由。

**目标：**

1. 顶栏左侧现有的通用 spark 图标（`Icon name="brand"` 套渐变圆底）换成 OmniDesk 自己的 logo。
2. 右侧轮次索引条的悬停预览文本框：**四角直角**（`border-radius: 0`），视觉语言与软件既有卡片一致（白底、细描边、同一套字号与灰阶），不再出现圆角。

**已核实的代码事实（本计划前提）：**

- `desktop/src/shell/TopNav.tsx:447-449`：顶栏品牌图标是 `w-8 h-8 rounded-full bg-gradient-to-tr` 底 + `Icon name="brand"`（通用四圆点放射符号）。品牌文字「OmniDesk」在 450-452 行。
- `desktop/public/omnidesk-icon.svg` 与 `desktop/public/omnidesk-monomark.svg` **sha256 完全相同**（`7a5b7e01…`），都是 512×512、深色圆角方底（`#0B0F17`, rx=116）+ O+D 环 + 五节点；`desktop/src-tauri/icons/icon.png` / `128x128.png` 是同一标识的 Dock/窗口图标。
- `desktop/src/components/BrandMark.tsx` 是同一 O+D 标识的**去掉深色底**版本（透明描边），当前用于输入区「+」左侧（`HomeChat.tsx:1949`，22px）。
- `desktop/src/shell/TurnRail.tsx:154-170` 预览卡：`rounded-xl` + `border-primary/30` + `bg-surface-container-lowest` + `shadow-lg`，内含标题一行（12px）与回答摘要（11.5px，最多 3 行）。
- `TurnRail.tsx:129` 刻度按钮上还有原生 `title` 属性。WebKitGTK 会另外弹一个系统绘制的小提示框（自带圆角、单行），与自定义卡片**同时出现**，这正是「看起来是圆角小文本框」的来源之一。
- `HomeChat.tsx:1671` 用户问句旁的 28px 小头像也用 `Icon name="brand"`。
- 回归脚本 `tests/verification/desktop/verify_ui_082.py` 的 `turnrail_hover_card_appears` 以区域 (950,420)-(1290,580) 像素差 >1500 判定卡片浮现；`turnrail_card_focus_shot` 只检查截图存在。改样式后像素差会变，但阈值是下界，预计仍成立；本条须实跑确认。

**影响范围（预计）：** `desktop/src/shell/TopNav.tsx`（品牌区）、`desktop/src/shell/TurnRail.tsx`（预览卡 JSX 与原生 `title`）、`desktop/src/theme.css`（如需要可新增 `turn-rail-tooltip` 类承载直角与描边，避免把视觉细节散在 className 里）、新增 `tests/verification/desktop/verify_ui_083.py`、`tests/verification/desktop/evidence/od-arch-083/`、本文件、STATUS.md。**不动** `runtime/`、Profile、凭证、模型路由、DSH 数据根。

**定案口径：**

1. **品牌标（已定方案 A）：** 顶栏改用 `public/omnidesk-icon.svg`（与 Dock/窗口图标同一份资产，深色圆角方底），以 `<img>` 呈现，基准尺寸 30–32px，保留 `alt="OmniDesk"`；去掉渐变圆底包裹，因为图标自身已带底。不新画 logo、不新增第三方资产、不改 SVG 源文件。**方案 B 已由用户否决，不实施。**
   **融入与过渡（用户追加要求，必做，不可省）：** 图标自身的深色方底与白色胶囊顶栏之间存在明显的明度落差，直接贴上去会像一块外来补丁。实现须让它成为导航条色阶的一部分，而不是孤立的色块：
   - 形状呼应：外轮廓的圆角半径与胶囊/徽标（现有 `rounded-full` 圆底、`plate-soft` 徽标）保持同一套圆角语言；图标 12% 的圆角在 32px 下不宜再加外框，避免出现「方中套方」。
   - 尺寸与光学对齐：图标视觉中线与「OmniDesk」文字的字面中线对齐（不是与行高盒对齐），与右侧 Profile 徽标同处一条水平基线上；图标与文字的间距与「徽标到徽标」的间距同值，保持等距节奏。
   - 明暗过渡：允许在图标周围加一圈极浅的中性缓冲（例如 `ring-1 ring-black/5` 或 1px 同色描边），使深底与白顶栏的硬边变成有层次的一条细边；**不得**加彩色光晕、渐变外框或投影堆叠。
   - 全条一致性：改完后顶栏依次为「深色圆底 Logo → 名称 → Profile 徽标 → IPC 胶囊 → 搜索框 → 日期 → 按钮」。Logo 是全条唯一的深色元素，须由它承担视觉起点，其余控件保持既有浅色 plate 体系，不因这次改动重新调色、不新增分隔线、不改顶栏高度与圆角。
   - 尺寸兜底：若 32px 下 O+D 细节（五节点、中央缝）糊成一团，可降到 28px 或改用图标内留白较紧的裁切方式；以真实窗口截图为判据，不以推测为准。
2. **预览卡：** 直角（无圆角），采用与设置弹层/板卡同一套 tokens：`bg-surface-container-lowest` 白底、1px `outline-variant` 级细描边、克制的投影、`font-label-caps` 小号等宽字标「第 N 轮」、正文 12px / 摘要 11.5px 沿用现有灰阶与行高。宽度与定位（`right-6`、`top` 夹取）不变，避免影响导航可用性。
3. **去重原生提示：** 删掉刻度按钮的 `title`，只保留自定义卡片 + `aria-label`，避免系统圆角提示框叠在卡片上；`aria-describedby` → `role="tooltip"` 关系保持。
4. **不改的范围：** 家聊天区用户头像（`HomeChat.tsx:1671`）与输入区 `BrandMark` 保持不变；刻度节距、滚动、跳转、无障碍键盘聚焦行为不变。若用户希望头像同步换成新 logo，需另行批准。

**步骤与验收：**

1. **S1 品牌标：** 顶栏左侧显示 OmniDesk 自有 logo，30–32px 下 O+D 环清晰可辨、与相邻「OmniDesk」文字基线对齐，深浅背景下不发虚、不被 `rounded-full` 裁切。
2. **S2 预览卡：** 悬停与键盘 Tab 聚焦均可看到预览卡；四角为直角，样式与软件其余卡片同源；不再出现系统原生圆角提示框。
3. **S3 回归：** `tsc --noEmit` exit 0；`npm run build` exit 0；`cargo test --offline -q --lib` ≥261 passed；`python3 -m pytest tests -q` 全绿；`verify_ui_082.py` 17 项仍全 pass（若仅像素差数值变化，记录新值，不降低判定标准）；`git status --porcelain runtime/` 为 0。
   **S1b 融合验收（用户追加要求）：** 顶栏整体截图 1:1 裁切后目视确认——Logo 与名称、Profile 徽标、IPC 胶囊在一条基线上；Logo 与名称间距等于徽标间距；深色底与白胶囊之间只有一层细边过渡，无光晕/重投影；顶栏高度、圆角、其余控件位置相对 082 截图无位移（用 082 顶栏裁切做逐像素对照，允许像素差异仅出现在 Logo 与预览卡两块区域）。若 28px 后仍不清晰，记录实际尺寸与裁切参数，不靠放大顶栏解决。
4. **S4 真实窗口证据：** 一次性 Xvfb 显示里截图「顶栏品牌标（改后）」「悬停预览卡（含四角特写裁切）」「键盘聚焦预览卡」，落 `tests/verification/desktop/evidence/od-arch-083/`，并写 `facts.json` 记录实际像素数值与断言结果。改前对照可引用 `evidence/od-arch-082/turnrail-card.png` 与 `00-rail-before.png`，不覆盖 082 证据。
5. **S5 新增 `verify_ui_083.py`：** 断言 ①顶栏品牌位渲染的是自有 logo（DOM 属性 / 截图裁切像素非空且非纯渐变圆底）；②悬停后卡片浮现；③卡片四角像素与背景交界处为直角（对角点采样，圆形化会在角上留下背景色三角）；④AX 树中刻度不再暴露原生 `title` 文案作为独立提示，`described-by` 仍指向 `role=tooltip`。不修改 082 脚本的既有断言，避免改动 082 证据口径。

**设计参考（OD-ARCH-035 要求，`screenshots-design-inspiration` 检索）：** 关键词 `chat turn navigator`、`sidebar tooltip preview card`；参考 ChatGPT Web（对话轨 + 悬浮摘要，弱化边框只留一层浅描边）、Aside（侧栏驱动的窄条导航 + 贴近指针的说明卡）、Condens（极简侧栏、小字号等宽标签）。借鉴要点：**预览卡靠留白与小字号等宽标签分层，不用大圆角和大阴影抢注意力；贴边浮动卡保持单层细描边、与宿主卡片同一套表面色。**

**风险与停止条件：** ①图标换成深色方底后与圆角胶囊顶栏不协调 → 先按上面的「明暗过渡 / 光学对齐」收敛一轮（细边、尺寸、间距），仍不协调则在 STATUS 记录实际截图与偏差，停回报 ARCHITECT，**不自行改回方案 B、不改顶栏形状**；②直角卡片在浅色对话背景上边界不清 → 加细描边补偿，不加回圆角；③改样式导致 `verify_ui_082.py` 悬停断言跌破阈值 → 停并按实际像素差调整采样区域，保留判定严格性，不直接降阈值；④真实窗口无法启动 → 记录为未验证，不拿静态检查冒充运行证据；⑤为「融入」而改动顶栏高度、胶囊圆角、其他控件配色或新增装饰元素 → 越界，停手并写 STATUS 等裁定。

### OD-ARCH-081：首页三项小修复（问候常驻 / 品牌标 / Markdown 正文）（已执行并复核通过，2026-10-25）

**批准依据：** 用户明确提出三条小修复并委托执行。本计划不扩大范围。

**目标：** ①「夜深了 · 欢迎回到 OmniDesk」与其后的待办条**常驻顶部**，不再因指针进入对话区而隐藏、也不再把对话整页上移；②输入框「+」左侧的装饰性 spark 图标换成 OmniDesk 品牌标（复用既有 O+D 标识，非交互、带无障碍标注）；③助手回复与用户问句的正文按 Markdown 渲染（标题、列表、表格、代码块、引用、链接），参照 DSH 原生正文的排版密度；流式增量仍逐段呈现、不丢字。

**影响范围：** `desktop/src/shell/SceneScaffold.tsx`（去掉两层互斥与 5 秒静止恢复，改为固定上下两段）、`desktop/src/shell/HomeChat.tsx`（正文渲染入口、输入区图标）、新增 `desktop/src/components/BrandMark.tsx`、`desktop/src/shell/MarkdownText.tsx`、`desktop/src/markdown.css`、`desktop/src/main.tsx`（样式引入）、`desktop/package.json` + lock（新增 `react-markdown`、`remark-gfm`，见下）、本文件、STATUS.md。**不动** `runtime/`、DSH 数据根、凭证、模型路由、真实材料。

**依赖理由：** 现有前端依赖中没有 Markdown 渲染能力，自研渲染器在表格、嵌套列表、代码块上易出错误，因此引入 `react-markdown` + `remark-gfm` 两个纯渲染依赖。不启用 `rehype-raw`，模型或用户文本中的 HTML 不会被执行或注入（无 `dangerouslySetInnerHTML`）。

**步骤与验收：**

1. S1 布局：问候 + 待办固定在对话上方，指针移动、输入框聚焦、静止等待都不改变其可见性；对话区在其下方自然占满剩余高度。
2. S2 品牌标：输入区「+」前显示 OmniDesk 标识（纯装饰，`aria-hidden`），点击不承担动作。
3. S3 Markdown：助手正文支持标题 / 无序与有序列表 / 粗体 / 行内代码 / 围栏代码块（带复制）/ 表格（横向滚动）/ 引用 / 链接；用户问句按 Markdown 渲染，但纯文本换行仍保留；流式输出时逐字显现与 Markdown 排版可共存、不出现乱码。
4. S4 回归：`tsc --noEmit`、`npm run build`、`cargo test --offline -q --lib`（≥257）、`python3 -m pytest tests -q`（≥75）、`git status --porcelain runtime/` 为 0。
5. S5 真实窗口证据：一次性 Xvfb 显示里截图确认问候与待办常驻（指针移入对话区前后均可见）、输入区显示品牌标、含表格/代码/列表的合成回复排版正确；证据存 `tests/verification/desktop/evidence/od-arch-081/`。不提交、不推送。

**风险与停止条件：** 新增依赖导致构建或 CSP 失败 → 停并报告，不退回自研渲染器硬凑；布局改动引起对话区高度异常或输入框不可见 → 停并保留原骨架；真实窗口无法启动 → 记录为未验证，不把静态检查当运行证据。

### OD-ARCH-065：多提供商模型档案 + 探测沿用已存密钥（已批准并执行完毕，2026-10-22；ARCHITECT 复核通过见 REVIEW.md）

> **批准依据：** 用户实测 064 通道时点「测试连接」得到 `https://amr-link.open-design.ai/v1/models answered 401`。排查结论（源码事实）：该端点无凭据直连必回 401；用已登记的 `opendesign_Key` 直连返回 200 与 12 个模型。问题不在密钥，而在探测请求没带凭据。用户同时要求：能长期维护多个提供商/模型及其地址与密钥，并明确不得泄露。

**目标：** ①「测试连接」在未输入新密钥时自动沿用该提供商已保存的密钥，不再误报 401；②模型配置从「单套覆盖」升级为「多提供商档案」：每个提供商各存一组模型、各自的地址与凭据引用，用户可增删改与切换默认；③全程沿用 DSH 官方凭据机制，OmniDesk 侧永不读取、缓存、回显或落盘密钥明文。

**已由源码证实的事实（本计划成立的前提，BUILDER 须先复核再动手）：**

1. DSH 的 provider 配置只接受 `apiKeyEnv` 引用名，明文写不进去（`packages/llm/llm-pi-ai/src/index.ts::resolveApiKey`）；密钥只能经官方 `credentials/set` 进入 `$DSH_HOME/.credentials.yaml`。
2. `credentials` Remote 命名空间只有 `describe / set / unset`，**没有任何取回明文的读路径**（`packages/api/settings-controller/src/credentials.ts`，注释明写 "Secret values cross in one direction only"）。因此「在 OmniDesk 里读回密钥再转发」不可实现，也不能去实现。
3. DSH 的 `llm/discoverModels` 已内置正确做法：请求里带 `provider` 时，它用该路由已登记的凭据解析密钥（`discoverModels(request, storedProfile)`，`discovery.ts` 第 308–315 行）；只有「草稿未带 key 且未指 provider」时才会裸请求。当前 OmniDesk 恰好走的是后者——`probe_models` 有意不传 `provider`（`dsh_settings.rs` 注释：避免命中内置 catalog 而绕过真实网络请求）。这是本缺陷的直接成因。
4. 传 `provider` 的唯一副作用：若该 provider 名正好是 pi-ai 内置目录里的厂商（openai / deepseek / openrouter / zai / qwen 等），DSH 会直接返回内置模型表而不发网络请求。当前及计划内的提供商（opendesign、agnescn、glm 网关等自建路由名）不在内置目录中，因此不影响真实连通性判定；但这一点必须在实现中显式处理，不得默认忽略。
5. `llm-pi-ai` 的 `providers` 是**以路由名为键的字典**（`config.ts::resolveProfiles`），天然支持多提供商并列；补丁层按 entry id 合并，因此多个提供商可写进同一份 OmniDesk 覆盖文件。当前 `render_overlay()` 只渲染一个 provider，是本计划要扩的地方。
6. **补丁层是浅合并，不是深合并**：`applyEntryPatches` 对非 insert 补丁执行逐键赋值（`runtime/deepseek-harness/vendor/include/src/index.ts` 约 114–119 行），所以补丁里的 `config` 会**整体替换**原 entry 的 `config`。实测 `dsh --profile web --dump-config --patch <覆盖文件>` 后，`providers` 下只剩覆盖文件里的那条路由，Profile 自带的 `agnescn` 消失，`agent-default-model` 同样被整体替换。因此「多 provider 渲染」必须按下面的合并口径写，否则会静默抹掉用户已有的其它路由。
7. 「测试连接」走的设置服务是独立起的 `dsh web --port 0 --no-open`，**不带 `--profile` 也不带 `--patch`**（`dsh_settings.rs::spawn` 约 394–420 行），Profile 取 DSH 默认（`web`）。所以该进程内存里没有覆盖文件里的 `opendesign` 路由：即使请求带上 `provider`，也解析不到该路由的 storedProfile，仍会退化成裸请求。要让「沿用已保存密钥」真正成立，设置服务必须以与引擎相同的 Profile + 同一份覆盖文件启动。

**影响范围：** `desktop/src-tauri/src/model_settings.rs`（数据结构与渲染，含迁移）、`desktop/src-tauri/src/lib.rs`（`model_settings_probe` 增加 provider 参数；保存/加载命令适配多档案）、`desktop/src-tauri/src/dsh_settings.rs`（`probe_models` 透传 provider，并区分“内置目录答复”与“真实网络答复”）、`desktop/src/SettingsPanel.tsx`（提供商列表 + 增删改 + 设为默认）、`desktop/src/shell/ModelPicker.tsx` 仅当现有目录数据不足时才动、`docs/user-testing-checklist.md` 对应步骤、`docs/OmniDesk-Desktop-Data-Layout.md`（配置文件结构说明）、PLANS/STATUS/REVIEW。通用红线：runtime/ 零差异、凭证零读取零打印零落盘、真实材料不进 Git/evidence、一次性授权分批提交、推送一次。

**步骤：**

1. **S1 探测沿用已存密钥（先修 bug，独立可验收）：** `model_settings_probe` 增加可选 `provider` 入参，前端把当前表单的 `provider` 一并传入；`probe_models` 在草稿里带上 `provider`（仅当该 provider 已存在于覆盖文件时才带，避免凭空造路由）。判定口径：界面文案改为「沿用已保存密钥 / 本次输入的新密钥 / 未带密钥」三态之一，让用户知道这一次到底用了什么。若 DSH 返回的是内置目录而非网络结果，界面须明确标注「该提供商名命中 DSH 内置目录，未发起真实网络请求」，不得当作连通性证明。验收：不填新密钥点测试，OpenDesign 应返回 12 个模型；填入错误密钥点测试，应明确报鉴权失败；两者都不改任何已保存配置。**前提（源码事实 7）：** 设置服务现在不带 `--profile` / `--patch`，进程里的路由表没有覆盖文件里的 provider，只带 `provider` 仍解析不到已存凭据。因此 S1 必须同时让设置服务按引擎同 Profile + 同一份覆盖文件启动：参数只取自 `EngineConfig`（`profile` 与 `sync_overlay()` 后的 `patch_files`），不接受任何外部输入；覆盖文件不存在时退回现有启动参数。此改动只影响 OmniDesk 自己拉起的设置服务进程，不改 Profile 文件、不改 DSH Runtime、不碰原版 DSH 数据。
2. **S2 多提供商档案数据结构与迁移：** 配置升级为 `omnidesk.model-settings.v2.json`，结构为 `providers: [{ provider, display_name, base_url, api, api_key_ref, models: [...], reasoning_effort }]` + `default: { provider, model }`。**必读**：读取到 v1 文件时自动就地升级为 v2（保留全部字段与老文件名作为 `.v1.bak`），升级失败不得删除或覆盖原文件，界面明确报错并只读。每个 provider 的 `api_key_ref` 必须互不冲突，重名直接拒绝（否则两个提供商共用一把密钥会导致串号）。
3. **S3 覆盖文件多路由渲染：** `render_overlay()` 渲染全部 providers，`acp` 与 `agent-default-model` 两段指向 `default`。保留既有“只写非机密字段”的约束与单测（`overlay_contains_only_non_secret_fields` 必须继续通过，并扩充为多 provider 场景）。保存前必须校验：路由名唯一、模型列表非空、baseURL 合法、api 在允许集合内。

   **合并口径（因源码事实 6，不得省略）：** 因为补丁层对 `config` 是整体替换，写覆盖文件前必须先把“覆盖生效后的完整路由集合”算出来：读取引擎 Profile 的 `llm-pi-ai` providers（用现有 `--dump-config` 通道，不读凭据），以 OmniDesk 数据根里的 provider 集合覆盖同名项、保留其它项，然后把合并结果整体写入覆盖文件；合并失败（Profile 读不到、结构异常）时直接报错不写盘，不得退回“只写 OmniDesk 自己那几条”（那会静默抹掉其它路由）。若合并后的路由集与引擎实际加载不一致，以 `--dump-config` 的实测结果为准并记录差异。验收必须包含一条：合并后 `providers` 下同时存在 `opendesign` 与 Profile 自带的 `agnescn`（或当前 Profile 实际自带的路由）。
4. **S4 设置页多提供商管理：** 「模型与端点」页改为左侧提供商列表（显示名 + 是否已配密钥 + 是否默认）+ 右侧编辑区。支持新增、编辑、删除（删除仅移除配置与该路由，**不删除 DSH 凭据**，界面须明说）、设为默认。删除最后一个提供商时须明确警告将导致无可用路由。所有密钥输入框仍为只写：保存后清空、回显只有「已配置 / 未配置 / 读取失败」。密钥仅写入当前选中 provider 的 `api_key_ref`，切换提供商时不得回填或复用上一家的密钥；不提供“复制另一个 provider 的密钥”能力。
5. **S5 模型选择器与对话路由：** 确认 `ModelPicker` 能列出多提供商下的全部模型并正确切换（当前已有 `providers[]` 结构，预计改动很小）；若切换某提供商模型后引擎未按其路由启动，记录为独立缺陷并停下报告，不在本计划内扩大修复。
6. **S6 回归与证据：** 单测覆盖 v1→v2 迁移、多路由渲染、密钥引用冲突拒绝、探测三态判定；`tsc` / `npm build` / `cargo test` / `pytest` 全绿；055/060 联动脚本复跑；真实窗口截图存 `evidence/od-arch-065/`（提供商列表、探测成功沿用已存密钥、探测错误密钥失败）；`facts.json` 落盘。**证据中不得出现任何密钥字符或其片段。**

**存储方式（回答“是否用 DSH 原版保存方式”）：** 是。密钥仍由 DSH 官方凭据服务保存在 OmniDesk 隔离数据根的 `.credentials.yaml`（权限 0600），OmniDesk 不另建加密库，也不把密钥写进自己的 JSON/YAML。多提供商的差异只体现在引用名：每个 provider 有自己的 `api_key_ref`（即 DSH 的 `apiKeyEnv` 名），每个名字在 DSH 凭据库对应一条密钥；同一提供商下的多个模型共用该引用名，不重复存密钥。因此新增提供商 = 新增一条路由配置 + 一个新引用名 + 一次官方 `credentials/set`，彼此互不覆盖。

**验收：** 不填新密钥点「测试连接」能成功并显示模型列表；填错密钥能明确报鉴权失败；至少能并列保存两个提供商并各自记住模型列表、地址与凭据引用；切换默认模型后对话走对应路由；v1 配置可无损升级；`runtime/` 零差异；仓库与证据中不存在任何密钥明文；`overlay_contains_only_non_secret_fields` 类单测通过。

**安全要求（不可打折）：** 不实现任何“读回密钥”的接口，无论前后端；不在日志、错误信息、测试夹具、截图、`facts.json` 中出现密钥；不打日志打印请求头；不把 DSH 的 `.credentials.yaml` 复制进 OmniDesk 数据根或仓库；判断“是否已配置”只允许用 `credentials/describe` 的 `configured` 布尔值。

**风险与停止条件：** ①`llm/discoverModels` 在带 `provider` 时对自建路由仍不解析凭据 → 停下报源码位置与实际错误码，不改成前端读密钥；②v1→v2 迁移在真实用户配置上出现字段丢失 → 立即停，保留原文件不动；③多 provider 渲染导致 DSH 启动失败 → 保留单 provider 回退路径并报告；④同一问题连续两轮修复未解决即停。⑤设置服务按 `--profile` + `--patch` 启动后，原先依赖 DSH 默认 `web` 行为的探测或保存流程失效（例如覆盖文件不合法导致设置服务起不来）→ 立即停，退回不带 `--patch` 的启动方式并报告，不得为了让探测通过而写一份不完整的覆盖文件；⑥合并后的覆盖文件在真实 Profile 上造成路由意外丢失或被顶掉 → 停，保留原文件，提交差异清单等 ARCHITECT 裁定。

### OD-ARCH-064：用户实测通道整备（启动入口 + 清单校准）（已批准，2026-10-22）

### OD-ARCH-066：响应速度诊断收尾 + 搜索授权链路（待批准，2026-10-23）

**背景（REVIEW.md 2026-10-23 已诊断）：** ①40 条任务台账实测成功任务中位 11 秒，本地链路毫秒级，30 秒感受来自模型推理与中转网络，无需架构改动；②场景里触发需要审批的工具（联网搜索）后卡住——OmniDesk 的 Web-API 通道没有审批 UI，前端也不消费 tool_activity 事件，turn 挂在等审批直到超时或用户取消。本计划补齐审批链路并加首字延迟埋点。

**目标：** ①模型审批请求在 OmniDesk 界面可见、可允许 / 拒绝，允许后任务继续；②任务卡片显示「等待审批中」状态，不再无限静默；③提交到首字到达（TTFB）有埋点，下一轮实测可直接对比模型侧耗时。

**影响范围：** engine/omnidesk_engine/web_api_adapter.py（把 DSH 审批请求转成归一化事件 + 提供 approve/reject 响应方法）、desktop/src-tauri/src/lib.rs（透传审批事件与用户决定命令）、desktop/src/shell/HomeChat.tsx（审批卡片渲染）、desktop/src/types.ts、desktop/src/api.ts；可选 engine_state 台账增加审批字段。不改 runtime/、不改 DSH 凭据、不新增通信层。

**步骤：**
1. **S1 审批事件贯通：** 适配层识别 DSH 的审批请求事件（approval seam / user-approval 相关事件），转成 EventType.PROGRESS（kind="approval_request"，带 toolName/reason/callId）；前端 HomeChat 渲染审批卡片（工具名 + 原因 + 允许 / 拒绝按钮）。
2. **S2 审批应答：** 适配层新增 respond_approval(callId, allow) 方法，经 DSH 通道回传决定；允许后任务继续流式输出，拒绝后 turn 以引擎语义落定（大概率 blocked→failed，界面如实显示）。Rust 侧加 omni.approval_respond 命令透传。
3. **S3 TTFB 埋点：** 适配层记录 submit → 首个 text_delta 的秒数，进任务台账（新字段 ttfb_seconds）；前端任务详情或 usage 面板显示。不读凭证、不记录请求内容。
4. **S4 测试与证据：** 单测覆盖审批事件归一化 + 应答；合成端点无法产真实审批请求时，用录制帧回放测试；真实审批流程由用户实测验收（触发一次搜索授权并允许）。证据：pytest 结果、tsc、cargo test、台账字段示例（无凭证）。

**验收：** 用户在 OmniDesk 里触发搜索授权 → 界面出现审批卡片 → 点允许 → 任务继续并完成；点拒绝 → 任务以失败/中断落定且界面如实显示；台账含 ttfb_seconds；runtime/ 零差异；未批准的工具默认仍是 ask（不改权限预设）。

**风险：** ①DSH web-api 通道的审批事件格式未实测 → S1 先加录制探针确认事件结构，再写归一化；②审批应答 RPC 可能不在 web 协议里 → 若 DSH 不支持，停下报告，方案改为「界面明示该工具需要到 DSH 原生界面批准」；③不扩大到权限预设管理（已有设置页管）。

> **批准依据：** 061~063 三轮界面收敛验收通过，用户进入实测阶段。本计划只整备实测通道与清单，不做任何功能扩张。用户既定指示：任务合并、不零碎派工。

**目标：** 用户可一键进入应用开始实测；上手清单与 061~063 后的真实界面一致；实测入口唯一。

**影响范围：** scripts/omnidesk.sh（核对即可，若无缺陷不改）、desktop/src/shell/Dock.tsx（若残留设置入口则移除）、docs/user-testing-checklist.md（按当前界面校准：Dock 5 入口名称、设置页 6 分页、分类下拉与发送/取消合并按钮、模型与端点连通测试三态）、PLANS/STATUS/REVIEW。通用红线：runtime/ 零差异、凭证不碰、真实材料不进 Git/evidence、一次性授权分批提交（每批前红线扫描 + 文件清单核对）、推送一次。

**步骤：**
1. **S1 启动入口核对：** 验证 scripts/omnidesk.sh 从干净 shell 一键启动 Desktop 窗口（合成数据根）；如已有缺陷停下报告，不擅自改。若有第二入口（dev-ui.sh 等）在文档中标注用途差异，实测统一走 omnidesk.sh。
2. **S2 Dock 残留核对：** Dock 5 入口与 aria 标签逐一核对；如发现设置入口残留（061 后不应再有），移除并复跑 055 联动脚本。
3. **S3 实测清单校准：** user-testing-checklist.md 按当前界面逐条校准：①Dock 5 入口与名称（工作台首页/工作记录/文案写作/项目跟踪/历史会话）；②设置唯一入口=顶栏齿轮，6 分页（模型与端点/对话分类/成果与文档库/权限/引擎诊断/关于）；③分类选择=左下角下拉胶囊（色点+勾选列表）；④发送/取消合并按钮行为；⑤端点连通测试三态预期不变；⑥分类管理与 vault 初始化入口指引（设置→对话分类）；⑦每步预期结果与「不算数」口径。清单不新增实测步骤之外的内容。
4. **S4 回归与证据：** tsc / cargo test / pytest / npm build 全绿；055/060 联动脚本复跑；omnidesk.sh 启动成功截图 1 张存 evidence/od-arch-064/ 并把事实写入 facts.json；STATUS 登记执行记录；REVIEW 的 064 验收结论由 ARCHITECT 复核后补写。

**验收：** omnidesk.sh 一键启动成功且 Dock 5 入口；清单与真实界面零偏差（每条对应一张现势截图或既有 063 证据）；全量回归绿；runtime/ 零差异；facts.json 落盘。

**风险与停止条件：** ①omnidesk.sh 启动失败 → 停下报告根因，不擅自改启动链路；②清单与界面有出入 → 以界面为准改清单，不反向改界面。

### OD-ARCH-063：分类 chip 下拉化 + 清理收尾（已批准，2026-10-22）

> **批准依据：** 用户已拍板分类 chip 改下拉（色点 + 勾选弹出列表 + 与模型选择器同套样式）；062 复核发现的证据勘误与仓库残留一并收尾。继续合并派工，不零碎。

**目标：** ①对话分类选择器改为下拉胶囊（色点 + 弹出勾选列表，对齐模型选择器样式），分类多时不占行、宽度恒定；②清掉 062 复核发现的仓库残留与命名尾巴；③证据落盘口径修正。

**影响范围：** desktop/src/shell/HomeChat.tsx（chip 改下拉按钮 + 弹出列表）、desktop/src/components/Icon.tsx（如需下拉箭头）、desktop/src/SettingsPanel.tsx 与 styles.css（.wb-header 类名收尾，仅设置页在用才改名，否则注明）、desktop/src/offline-flows.css 与 .ofl-v2 命名（仅设置页在用则改名，牵动布局回归时停下列清单）、删除仓库根 `./0` 空文件、删除 tests/verification/desktop/evidence/od-arch-060/00-home.png (deleted) 残留文件（原 00-home.png 正常保留）、evidence 命名核对、PLANS/STATUS/REVIEW。通用红线同 062：runtime/ 零差异、凭证不碰、旧数据零迁移零覆盖零删除、一次性授权分批提交（每批前红线扫描 + 文件清单核对）、推送一次。

**步骤：**
1. **S1 分类下拉：** 平铺 chip 行改为单枚下拉胶囊：左侧色点（分类色，未分类用中性色）、中间当前分类名、右侧下拉箭头；点开弹出列表（勾选态 + 色点 + 分类名，滚动容纳），单选后立即收起并生效（复用既有 retag/归档逻辑，不改持久化格式与键）。样式与模型选择器同套（复用其类或同规格新类，不引第三方组件库）。新增分类后下拉列表自动可见。键盘可达（Enter/Esc）与 aria 属性齐备，沿用 data-testid 钩子。
2. **S2 仓库残留清理：** 删 `./0` 空文件；删 evidence/od-arch-060/00-home.png (deleted) 残留；.wb-header 仅设置页在用则改名（全引用核对），否则保留并注明；.ofl-v2/offline-flows.css 若仅设置页在用则同步改名，若牵动 >3 处布局回归则停下报清单。
3. **S3 回归与证据：** tsc / cargo test / pytest / npm build 全绿；055/060 联动脚本复跑；真实窗口截图存 evidence/od-arch-063/：下拉收起态（色点可见）1 张、展开列表（含勾选与新增分类）1 张、选中后收起生效 1 张、设置页布局 1 张；**验收事实统一写入 evidence/od-arch-063/facts.json**（命令、结果、截图清单），修正 062 的事实未落盘问题；STATUS 登记执行记录；REVIEW 的 063 验收结论由 ARCHITECT 复核后补写。

**验收：** cargo test 全绿；tsc 0 错误；pytest 通过；npm build 通过；runtime/ 零差异；chip 下拉在 4 类与新增后 ≥5 类两种状态下宽度不变且列表完整可滚；选中变更仍走既有归档链路（不改 thread_store 数据）；facts.json 落盘齐全；仓库根与 evidence 目录无残留文件。

**风险与停止条件：** ①下拉展开与模型选择器/语音按钮层叠冲突（z-index/遮挡）→ 停下报截图与定位，不硬改全局层叠；②分类改名后旧会话显示回退失败 → 如实显示内部键并报问题，不静默改数据；③.ofl-v2 改名牵动面大 → 停下等裁定。
@@
### OD-ARCH-062：分类管理持久化 + workbench 清理收尾（已批准，2026-10-22）

> **批准依据：** 061 复核两项裁定（REVIEW.md 顶部：分类管理高级能力、取消测试解耦方案）+ 一项修正要求（SettingsPanel 样式类去 workbench 语义、OfflineFlows 处置）。用户既定指示：任务合并、不零碎派工。

**目标：** ①对话分类管理落成宿主持久化能力，设置页从展示升级为可管理（显示名/新增分类/文件夹与 vault 映射/初始化骨架）；②彻底清理 workbench 遗留（源文件、样式类、无引用组件），并让取消测试不再依赖待删文件。

**影响范围：** desktop/src-tauri/src/settings.rs（只增字段与读写，不改既有字段与命令语义）、命令注册、desktop/src/SettingsPanel.tsx（分类页管理 UI；根节点样式类改名去 `.workbench ofl-v2` 语义）、desktop/src/shell/HomeChat.tsx（chip 显示名读宿主配置）、新增 engine-cancel 区域独立模块（位置由 BUILDER 按现有结构定，如 src/engineCancelModel.tsx）、tests/verification/desktop/verify_engine_cancel.py（改抽取源）、删除 desktop/src/workbench.tsx 与 workbench-ui.css、OfflineFlows.tsx 确认无引用后删除、styles.css 中仅设置页在用的 .workbench 类同步改名（历史文档与既有 evidence 不动）、PLANS/STATUS/REVIEW。通用红线同 061：runtime/ 零差异、凭证不碰、旧 YvanAI 与既有成果零迁移零覆盖零删除、一次性授权分批提交（每批前红线扫描 + 文件清单核对）、推送一次。

**步骤：**
1. **S1 分类持久化与管理页：** settings.v1 新增 thread_categories 持久化（默认四类 plain/weekly/writing/project，每类含显示名、归档目标文件夹、vault 绑定），读写命令不改既有字段语义，附单测（默认值、roundtrip、越界拒绝）。设置页分类页实现：显示名编辑、新增分类（只进 chip 与归类下拉，不自动加 Dock 图标）、每类映射成果文件夹与 Obsidian vault（目录必须限定在成果根或已绑定 vault 内，越界拒绝并明示原因）、初始化骨架按钮（只在目标根内建目录骨架，已存在跳过并提示，不触碰既有文件）。HomeChat chip 与归档归类显示名从宿主配置读取，内部键不变。
2. **S2 取消测试解耦与 workbench 删除：** 把 workbench.tsx 中 `#region engine-cancel-model` 区域**逐字**迁到独立模块（不改逻辑、不改行为），verify_engine_cancel.py 改从新模块抽取；先跑该测试确认通过，再删 workbench.tsx 与 workbench-ui.css（删前列文件清单与全 desktop/src 引用核对）。SettingsPanel 根节点样式类改名，styles.css 内 .workbench 类若仅设置页在用则同步改名，否则保留并注明。
3. **S3 OfflineFlows 处置：** 确认全仓无 import 后删除该文件；若发现仍被引用，停下列引用清单报 ARCHITECT，不擅自改引用方。
4. **S4 回归与证据：** tsc / cargo test / pytest / npm build 全绿；改源后的 verify_engine_cancel.py 真实通过；055/060 联动脚本复跑；新增截图存 evidence/od-arch-062/：分类页编辑态 1 张、新增分类在 chip 下拉生效 1 张、初始化骨架后成果目录 1 张；STATUS 登记执行记录（含删除文件清单与迁移对照说明）；REVIEW 的 062 验收结论由 ARCHITECT 复核后补写。

**验收：** cargo test 全绿且 settings 新字段单测覆盖（默认四类/roundtrip/越界拒绝）；tsc 0 错误；pytest 通过；npm build 通过；runtime/ 零差异；desktop/src 内 workbench 与 OfflineFlows 引用 0 处；verify_engine_cancel.py 通过且抽取源为新模块；真实窗口完成一次「新增分类 → chip 下拉可见 → 成果映射保存 → 重启后仍在」链路验证。

**风险与停止条件：** ①settings 既有序列化格式与新增结构不兼容（读旧文件失败）→ 停下报兼容方案，不擅自迁移用户数据；②迁移取消区域发现依赖 workbench 内部状态或上下文 → 停下报告，不得改写逻辑迁就；③新增分类持久化写失败或读取冲突 → 界面明确报错，不静默降级。

### OD-ARCH-061：界面精简与设置归一（已批准，2026-10-22）

> **批准依据：** 用户实测 060 后给出的五条微调，即本计划范围：①对话左下角归类固定为 未分类/工作记录/文案写作/项目跟进，设置中提供分类管理与初始化（含本地文件夹与 Obsidian 适配），样式匹配现有 UI；②设置入口唯一化（顶部导航设置按钮），设置页按功能分页；③去掉挂载库；④Dock 只留工作场景（周报已改工作记录）+ 历史会话；⑤删任务工作台及其相关功能，成果目录与引擎诊断并入设置。

**目标：** 收敛界面入口与设置职责：首页对话保留四类归类，Dock 只剩场景入口，设置一处分页管理。内部 scenario 键（plain/weekly_report/writing/project）、数据目录名、既有命令语义一律不变，只动展示层与设置承载。

**影响范围：** desktop/src/shell/Dock.tsx、HomeChat.tsx、App.tsx（路由）、SettingsPanel.tsx（分页重构）、TopNav.tsx（必要时）、Workspace.tsx 与 EngineTechSlice.tsx（随 workbench 下线，删代码列清单）、desktop/src-tauri（仅当分类映射需要持久化字段时新增，不改既有命令语义）、samples/profiles/discipline.yml（如入口声明需同步）、PLANS/STATUS/REVIEW。通用红线同 059：runtime/ 零差异、凭证不碰、旧 YvanAI 与既有成果零迁移零覆盖零删除、一次性授权分批提交（每批前红线扫描+文件清单核对）。

**步骤：**
1. **S1 对话归类升级：** HomeChat 左下角场景 chip 固定四类（未分类/工作记录/文案写作/项目跟进），样式对齐现有设计语言（胶囊形/surface 色阶）；选中即会话场景标签（复用 retagThread）。设置页新增"对话分类"分页：四类默认展示、显示名可改、可新增未来分类（新增分类只进归类下拉与归档映射，不自动加 Dock 图标）、每类映射成果文件夹与 Obsidian vault（复用 059 vault 绑定机制）、提供"初始化文件夹"按钮（只在成果根/vault 内建目录骨架，不触碰既有文件）。
2. **S2 设置唯一入口与分页：** SettingsPanel 改分页：模型与端点 / 对话分类 / 成果与文档库（成果目录管理自 Dock settings 页迁入）/ 权限 / 引擎诊断（只读，自 engine 页迁入）/ 关于。顶部导航设置按钮为全应用唯一设置入口；Dock 移除 settings 与 engine 两项。
3. **S3 去挂载库：** HomeChat 输入区"挂载库"按钮及相关无引用状态移除（语音置灰占位保留，不在本计划范围）。
4. **S4 Dock 精简与任务工作台下线：** Dock 固定 5 入口：工作台首页/工作记录/文案写作/项目跟踪/历史会话（"项目与会话"改名"历史会话"）。删除 workbench 入口与路由及其独有组件（EngineTechSlice 面板迁入设置后原页面移除；Workspace.tsx 自带 HomeChat 等 060 遗留死代码一并清理，删改清单写入执行记录）。声明文件场景列表只覆盖 5 个业务场景，基础设施入口由代码固定（沿用 026 规则）。
5. **S5 证据与记录：** 截图存 evidence/od-arch-061/：Dock 5 入口、设置分页各 1 张、归类 chip 至少两种选中态、挂载库移除后输入区 1 张；复跑 verify_scene_decl_linkage_055.py 与 verify_scene_overview_060.py；STATUS 登记执行记录；REVIEW 中已写入的 051/060 复核结论随本批入库提交（061 验收结论由 ARCHITECT 复核后补写）。

**验收：** tsc 0 错误；cargo test 全绿（沿用既有 PKG_CONFIG_PATH 环境命令）；pytest 通过；npm build 通过；git status runtime/ 零差异；Dock 渲染 5 入口且无 workbench/settings/engine；HomeChat 归类四值与 retag 下拉一致；"挂载库"grep 全仓 0 处；workbench 路由与组件无引用残留（grep 清单核对）；055/060 联动脚本复跑通过。

**风险与停止条件：** ①分类管理需新增 Rust 持久化字段 → 先停下报方案再动手；②删除 workbench 发现被 055/声明联动或其他模块引用 → 停下列引用清单等 ARCHITECT 裁定；③设置分页重构若导致既有设置项丢失 → 优先用原组件分页承载，不重写设置逻辑。

### OD-ARCH-060：场景页 v2——状态条 + 文档库 + 时间线（已批准，2026-10-22）

> **背景：** 用户看过方案讨论后拍板按 ARCHITECT 推荐（A/B 融合骨架）开发，看效果再调。三场景同一骨架，差异用模块开关；会话历史降为次级资源。本计划同时把 059 收尾材料（059 执行记录、059 复核结论、首页下拉框修正）一并入库提交。

**目标：** 场景页从"公告栏 + 双栏"升级为：场景状态条（待确认/逾期/截止）+ 左文档树 + 右成果视图（卡片/列表 + frontmatter 筛选）+ 底部折叠会话历史；工作记录页加"本期记录碎片"区，项目页加"进度"页签（现有）。

**影响范围：** desktop/src/shell/ScenePage.tsx（重排）、AnnouncementBar.tsx（状态条化改造，保留 046 降级规则）、新增 ArtifactExplorer.tsx（文档树+视图）、HomeCanvas.tsx（"今日重点推进"接真数据，摘掉"未接入数据源"空态徽标）、desktop/src-tauri（新增场景汇总命令：按场景聚合 pending_writes 数量与本周截止；复用现有命令，不改既有命令语义）、PLANS/STATUS/REVIEW。提交沿用一次性授权分批模式。通用红线同 059（runtime 零差异、凭证不碰、旧数据零迁移）。

**步骤：**
1. **S1 状态条：** AnnouncementBar 顶部改为三段状态卡：待确认写入 N（点击展开 PendingWritePanel）/ 逾期 N（pending_writes 过期项）/ 周期提醒（工作记录页=本期周报·周五截止·进行中；项目页=最近节点·剩余天数；文案页=本周产出计数）。数据全部来自既有确定性命令（pending_writes list、weekly/project 汇总），**不新增模型调用、不编造数字**；046 的 AI 复盘建议区与降级规则原样保留在状态条下方。
2. **S2 文档库视图：** 新增 ArtifactExplorer：左侧目录树（树状模式数据已有）；右侧成果卡片/列表切换，卡片显示 title（frontmatter）、日期、场景标签、字数；顶部筛选条按 scenario 标签 + 本周/本月时间过滤（frontmatter 数据从 task_read_offline_artifact 读，列表页懒加载标题，避免全量读正文）。搜索框过滤文件名/标题。
3. **S3 会话历史折叠：** 场景页左栏 300px 会话列表移到底部折叠区（默认收起，展开显示现有列表 + 去对话）；retag/删除功能原样保留。
4. **S4 场景差异模块：** 工作记录页右侧新增"本期记录碎片"区（本周对话中已确认的事项条目，来自现有确定性汇总）；项目页保留"进度"页签（现有，不重做）。
5. **S5 首页今日重点接真数据：** HomeCanvas"今日重点推进"卡片摘掉"未接入数据源"徽标，接入跨场景待确认+逾期汇总（与 S1 同数据源）；无数据时显示"今天没有要处理的"诚实空态。ThreeWeekCalendar 不动。
6. **S6 证据与记录：** 真实窗口截图（三场景各 1 张全页 + 状态条展开待确认 1 张 + 筛选生效 1 张）存 evidence/od-arch-060/；STATUS 登记；REVIEW 写 059 复核结论与 060 验收结论。

**验收：** tsc / cargo lib（含新增汇总单测）/ pytest / npm build 全绿；runtime/ 零差异；状态条数字与 pending_writes 实际条数一致（单测断言）；frontmatter 筛选对 059 新格式成果生效（用合成成果验证）；旧成果（无 frontmatter）在视图中按文件名显示不报错（兼容单测）；三场景入口隐藏声明联动不回退（复跑 verify_scene_decl_linkage_055.py）。

**风险与停止条件：** ①懒加载标题若导致视图首屏卡顿或读文件过多（>100 文件），停下报告，改为按需分页；②frontmatter 解析对用户手改文件失败时按"无元数据"降级显示，不报错不阻塞；③发现需改既有命令语义才能取数 → 停下报告，不擅自改。

### OD-ARCH-059：综合批次（056 修复 + 057 对话归一 + 058 Obsidian 兼容）（已批准合并执行，2026-10-22）

> **背景：** 用户指示将 056/057/058 三项合并为一份计划交 BUILDER（DS v4.1-flash）执行。三块内容互相独立（设置修复 / 界面重构 / 成果库格式），可分批提交、独立验收。**本计划获批即视为 056/057/058 一并批准**，其详细步骤以下文 S1–S3 三节为准（各节原文来自对应计划，此处为合并后的权威版本）；若 BUILDER 发现三节间有冲突，以后文为准并报告冲突点。

**目标：** ①修掉夜间批次复核遗留缺陷；②全应用对话归一到首页、场景页纯档案化、"周报"改名"工作记录"；③成果库成为 Obsidian 兼容 vault。三项完成后用户即可开始真实场景试用。

**总影响范围：** desktop/src（前端）、desktop/src-tauri（pending_writes frontmatter、settings vault 绑定；**不改 lib.rs 既有命令语义**）、scripts/setup.sh 与 init-yvan-data.sh（仅注释）、samples/profiles/discipline.yml（入口改名）、docs/OmniDesk-Desktop-Data-Layout.md、docs/user-testing-checklist.md（对话与 vault 新流程）、PLANS/STATUS/REVIEW。

**通用红线（三节共用）：** 不碰 runtime/（零差异验收）；不碰凭证、不改路由；真实材料不进 Git/evidence；旧 YvanAI 与既有成果文件零迁移零覆盖零删除；内部 scenario 键（weekly_report 等）与数据目录名不变；提交沿用一次性授权分批进行，每批前红线扫描+文件清单核对。

---

**S1 = 056 复核遗留修复（先行，最小批）：**

1. 凭据状态收敛：前端在 view 已加载且 keyStatus 为 null 且无 key_error 时显示"尚未配置"（现状：永久"读取中…"）。不新增命令、不改 Rust。核对三态文案齐全：未配置 / 已配置（含来源与可否改写）/ 读取失败。
2. setup.sh 口径对齐：仅改 setup.sh 与 init-yvan-data.sh 顶部注释，写明 ~/YvanAI 为项目早期数据根、现存数据只读使用、新部署仍初始化到该路径以保持脚本兼容、桌面端成果不写此目录。不改行为、不改默认路径。
3. Dock 图标可访问名：核对 Dock.tsx 每个图标是否带 aria-label；缺则补（纯属性，同值用于自动化定位）。
4. STATUS/REVIEW 登记；REVIEW.md 顶部 051~055 复核结论随本批一并入库提交。

**S2 = 057 对话中枢归一（S1 全绿后）：**

1. 首页 HomeChat 输入区加场景选择 chip（默认"自由对话"，可选 工作记录/文案写作/项目跟进，与模型选择器同排）。选定后会话携带场景预设与归档目标；会话中途可改标签（复用 retagThread）。首页历史列表按场景小标签展示、可筛选。
2. 场景页纯档案化：ScenePage 移除 HomeChat，布局单层化（提醒/公告栏 + 会话列表（每条带"去对话"跳首页加载该会话）+ 成果档案；项目跟进保留进度页签）。SceneScaffold 双层逻辑仅首页保留。
3. 改名"周报"→"工作记录"：仅展示层（Dock 入口名、前端文案、空态提示、声明样例 discipline.yml）；内部键 weekly_report 与成果目录不动，旧数据零迁移。代码 grep 核对展示层无"周报"残留。
4. 首页卡片层（今日概览三模块 + 合并提醒）保留现状，不重做。
5. 声明联动：隐藏某场景入口后，首页场景选择器同步隐藏对应选项。

**S3 = 058 Obsidian 兼容（S2 全绿后）：**

1. frontmatter 标准（仅新文件）：pending_writes 与保存成果共用的写入口（write_new_file_durable 调用前）注入 YAML frontmatter：scenario / title / created(ISO8601) / tags / omnidesk_version。字段固定小集合，供 Obsidian Dataview 直接用；frontmatter 不进红线扫描正文（扫描仍只对用户原文）。
2. vault 绑定：设置页成果库区新增"vault 目录"显示与修改（默认 ~/OmniDesk/workspace/artifacts；改绑后新成果落新目录，旧目录文件不动不迁）。不做多 vault。
3. 接入指引：成果档案页与设置页各一步说明（在 Obsidian 中"打开文件夹作为 vault"指向绑定目录）。Obsidian 本体不安装、不检测、不拉起。
4. 边界声明：OmniDesk 只写自己的成果文件，不读不改用户在 vault 中的其他笔记；用户在 Obsidian 端的整理/改名/删除，OmniDesk 不感知不干预。

---

**验收（每 S 节收尾独立跑）：**

| 节 | 必跑 | 专项核对 |
| --- | --- | --- |
| S1 | tsc 0 / cargo lib 全绿 / pytest / runtime 零差异 | 凭据三态文案；setup.sh --check 行为不变 |
| S2 | 同上 + npm build | 首页选工作记录后成果落 weekly_report 目录；旧会话"去对话"跳转可续聊且历史完整；Dock 改名生效；隐藏入口联动 |
| S3 | 同上 | 新成果首行为可解析 frontmatter（serde_yaml 单测）；既有成果文件哈希不变（单测断言）；vault 改绑后新文件落新目录、旧目录无写入 |

全部证据截图存 tests/verification/desktop/evidence/od-arch-059/（分 S1/S2/S3 子目录），密钥与真实材料不得出现在任何证据中。

**风险与停止条件（合并版）：** ①S2 移除场景页对话后发现某能力依赖场景页对话上下文 → 停下报告，不擅自加回；②retagThread 中途改标签丢历史映射 → 停下报告；③S3 发现保存入口不止 write_new_file_durable 一处 → 停下报告清单，不擅自全改；④vault 改绑后读档列表越界 → 沿用 resolve_local_data_root 校验，不够用就停；⑤同一问题连续两轮修复未解决即停，写根因与阻塞报告；⑥发现疑似真实材料/凭证、runtime/ 出现差异、需要改 DSH Runtime → 立即停并报告。

**执行顺序与批次建议：** S1（1~2 批）→ S2（2~3 批：结构重构 / 改名+联动 / 证据）→ S3（2 批：frontmatter+单测 / vault 绑定+指引+证据）→ 收尾批（STATUS 总结 + REVIEW 材料 + 交接）。预计 7~9 个提交。完成后停止等 ARCHITECT 复核，不自行开新计划。

<details>
<summary>原始分计划归档（056/057/058 条文，已由 059 取代）</summary>


### OD-ARCH-057：对话中枢归一 + 场景页纯档案化（待批准，2026-10-22）

> **背景：** 用户实测后提出界面重构方向：每个场景页都重复挂对话框，只有卡片不同——应对话归一：首页对话框内选场景（周报改名"工作记录"），内容和成果按所选场景归档；Dock 场景页只做文件/文档管理，不再出现对话框。本计划将该方向落为实施项。

**目标：** 全应用只保留首页一个对话实例；场景选择收进对话框；场景页转为纯档案管理页。

**影响范围：** desktop/src 前端（App.tsx、HomeChat.tsx、ScenePage.tsx、SceneScaffold.tsx、Dock 相关）、samples/profiles/discipline.yml（改 Dock 入口名）与数据根声明样例、PLANS/STATUS/REVIEW。不碰 Rust 宿主（thread store 与成果按 scenario 归档的机制现成）、不碰 runtime/（零差异验收）、不改内部 scenario 键与数据目录（weekly_report 等键不变，**不做任何数据迁移**）。

**步骤：**
1. **S1 首页对话场景选择器：** HomeChat 输入区加场景选择 chip（默认"自由对话"，可选 工作记录/文案写作/项目跟进，与模型选择器同排）。选定后该会话携带场景预设与归档目标；会话中途可改标签（复用现有 retagThread）。首页历史列表按场景小标签展示、可筛选。
2. **S2 场景页纯档案化：** ScenePage 移除 HomeChat，布局改为单层：提醒/公告栏 + 会话列表（每条带"去对话"跳首页加载该会话）+ 成果档案（项目跟进保留进度页签）。SceneScaffold 双层焦点/静止恢复逻辑仅首页保留，场景页不再挂载。从档案页跳转 = onNavigate 到 home 并预载对应 thread。
3. **S3 改名"周报"→"工作记录"：** 声明样例 discipline.yml 的 Dock 入口名、前端展示文案、空态提示全部改用"工作记录"；内部 scenario 键 weekly_report、成果目录、线程标签**一律不动**（旧数据零迁移零覆盖，只改显示名）。声明样例与前端回退文案保持一致。
4. **S4 首页卡片层保留现状：** 今日概览三模块与合并提醒作为首页空闲卡片层（现有双层逻辑已实现，不重做）；公告栏随场景页保留（S2）。
5. **S5 记录与证据：** STATUS 登记、REVIEW 复核结论；真实窗口截图（首页含场景选择器、场景页无对话框、改名后 Dock 与空态）存 tests/verification/desktop/evidence/od-arch-057/。

**验收：** tsc、cargo lib、pytest、npm build 全绿；runtime/ 零差异；行为核对：首页选"工作记录"对话后成果落 weekly_report 目录；旧会话从档案页"去对话"跳转后可继续且历史完整；"周报"字样在 UI 不再出现（代码 grep 核对展示层）；隐藏场景入口后首页选择器同步隐藏对应选项（声明联动）。

**风险与停止条件：** ①场景页移除对话后若发现某能力依赖场景页内的对话上下文（如项目进度与对话联动），停下报告，不擅自加回；②retagThread 中途改标签若发现会丢历史映射，停下报告；③改名仅限展示层，若发现内部键有"周报"字样外泄到用户可见的文件路径，记录但不改（避免数据迁移），在执行记录中说明。

### OD-ARCH-058：成果库 Obsidian 兼容（待批准，2026-10-22）

> **背景：** 用户提出用 Obsidian 兼容文件夹作为本地知识库底座，让用户绑定 Obsidian 获得成熟的知识管理能力，OmniDesk 不自建。方向成立：成果已是 Markdown 文件、数据在仓库外用户目录，天然贴近 Obsidian；差距在目录语义与元数据（无 frontmatter），且须守住红线（不自动上传、不反向改用户手写内容）。

**目标：** 成果目录即 Obsidian vault；成果带标准 frontmatter；用户可把任意 vault 目录绑定为成果库；同步向用户提供接入指引。**不做**任何"发送到 Obsidian/同步服务"的自动上传功能。

**影响范围：** desktop/src-tauri（pending_writes 写入时补 frontmatter、settings 增加 vault 绑定项）、SettingsPanel（成果库区显示绑定路径与"在 Obsidian 打开"指引）、docs/OmniDesk-Desktop-Data-Layout.md（vault 口径）、PLANS/STATUS/REVIEW。不碰 runtime/（零差异验收）、不动旧 YvanAI 与既有成果文件（frontmatter 只对**新写入**的文件生效，已有文件不改）。

**步骤：**
1. **S1 frontmatter 标准（新文件生效）：** pending_writes 与保存成果共用写入口（write_new_file_durable 调用前）注入 YAML frontmatter：scenario、title、created（ISO8601）、tags（场景标签）、omnidesk_version。字段固定小集合，供 Obsidian Dataview 类插件直接用。已有文件零改动；frontmatter 不进红线扫描正文（扫描仍只对用户原文）。
2. **S2 vault 绑定：** 设置页成果库区新增"vault 目录"显示与修改（默认即 ~/OmniDesk/workspace/artifacts；改为其他目录时新成果落新目录，旧目录文件不动不迁）。不做多 vault，就一个绑定项。
3. **S3 打开指引：** 成果档案页与设置页提供一步说明：在 Obsidian 中"打开文件夹作为 vault"指向绑定目录；Obsidian 本体不在本计划范围（不自动安装、不检测、不拉起）。
4. **S4 边界声明：** 设置页与文档写明：OmniDesk 只写入自己的成果文件，不读取不改写用户在 vault 中的其他笔记；用户在 Obsidian 端的整理、重命名、删除 OmniDesk 不感知不干预（读档列表按场景过滤时自然反映文件现状）。

**验收：** tsc、cargo lib（含 frontmatter 生成单测：字段齐全、非法字符转义）、pytest、npm build 全绿；runtime/ 零差异；新成果文件首行为 frontmatter 且 Obsidian 语义可解析（单测断言 YAML 可被 serde_yaml 反序列化）；旧成果文件哈希不变（单测断言不触碰既有文件）。

**风险与停止条件：** ①发现现有保存入口不止 write_new_file_durable 一处（会漏 frontmatter），停下报告清单不擅自全改；②vault 改绑后若发现读档列表跨目录权限边界问题（场景目录校验），沿用现有 resolve_local_data_root 校验逻辑，不够用就停；③不做 Obsidian 安装检测与自动拉起（超出范围）。

### OD-ARCH-056：复核遗留修复 + YvanAI 目录口径定案（待批准，2026-10-22）

> **背景：** 056 起夜间批次收尾后进入修复小批模式（用户 2026-10-22 口径：小修不设长计划，验收按实际改动范围跑必要回归）。本计划内容即夜间批次复核发现的 054/055 两处缺陷 + 一项待用户拍板事项的定案执行。REVIEW.md 顶部 051 通过结论与 053/054/055 复核结论随本计划记录批次一并提交。

**目标：** 修掉凭据状态不收敛与 setup 脚本目录口径矛盾；把 YvanAI 初始化问题按用户决定收口。

**影响范围：** desktop/src/SettingsPanel.tsx（凭据状态收敛）、scripts/setup.sh（注释与目录口径）、docs/OmniDesk-Desktop-Data-Layout.md（如需同步口径）、PLANS/STATUS/REVIEW。提交推送沿用一次性授权模式（每批前红线扫描+文件清单核对）。不碰 runtime/（零差异验收）、凭证、路由、用户数据。

**步骤：**
1. **S1 凭据状态收敛（修 054 缺陷）：** 现状（源码核实）：Rust 侧 key_status 仅在有已存配置时查询，未配置时返回 null 且无 key_error；前端渲染链 keyStatus→key_error→"读取中…" 落到永久"读取中"。修法：前端在 view 已加载且 keyStatus 为 null 且无 key_error 时显示"尚未配置"，与 placeholder 口径一致；不新增命令、不改 Rust。
2. **S2 setup.sh 口径对齐（不改行为，改声明）：** 现状（源码核实）：setup.sh 第 22 行注释明确"只在仓库内与 ~/YvanAI（外部数据）写入"，第 2 步调 init-yvan-data.sh 默认根 ~/YvanAI——行为与注释一致，但与数据布局文档"OmniDesk 统一命名、YvanAI 只读不迁移"的迁移后口径矛盾。修法：在 setup.sh 与 init-yvan-data.sh 顶部注释中写明历史原因（该目录为项目早期数据根，现存用户数据继续只读使用，新部署同样初始化到该路径以保证既有脚本兼容；OmniDesk 桌面端成果不写此目录）。不改脚本行为、不改默认路径、不迁移数据。
3. **S3 Dock 选择器核对（只核对，不改）：** 核对 Dock.tsx 图标是否已带稳定可访问名（aria-label 或 title）；已有则记录结论即可，没有则为每个图标补 aria-label（纯属性，不影响布局，同值用于自动化定位）。
4. **S4 记录：** STATUS 登记、REVIEW 写 053/054/055 复核结论与本计划验收结论。

**验收：** tsc 0、cargo lib 全绿、pytest 通过、runtime/ 零差异；凭据状态三态（未配置/已配置/读取失败）在代码审查中确认各有明确文案；setup.sh --check 行为不变（可跑一次对照）。

**风险与停止条件：** S1 若发现需改 Rust 返回结构才能修复（与源码核实结论不符），停下报告不擅自改；S2 仅注释改动，若发现脚本实际行为与注释不符的更深问题，停下报告。

> **夜间批次（用户 2026-10-22 已批准连夜执行）：** 053 → 054 → 055 依序执行；每个计划收尾先跑本计划全部回归，全绿才进入下一个；任一步失败或触发停止条件，立即停下写阻塞报告，不自行扩大范围。三份计划共含一次性分批提交+推送授权（沿用 047/049/051 模式：每批提交前跑红线扫描+文件清单核对，附在执行记录中；发现疑似真实材料或凭证即停）。

### OD-ARCH-053：原生设置页替代 DSH 设置窗口（已批准，2026-10-22）

> **背景：** 用户实测确认「打开 DSH 设置」实际弹出完整 DSH Web 应用首页（缺陷；设置面板是 DSH 应用内弹层，官方无直达入口）。用户拍板方案 A：OmniDesk 自建原生设置页，DSH 完整应用入口降级隐藏。本计划修订决策 D-026（执行完成后在 docs/decisions.md 追加取代记录）。

**目标：** 模型端点/密钥/权限全部在 OmniDesk 原生设置页内完成；设置不再依赖 DSH UI；场景声明（profile）入口进设置页。

**影响范围：** desktop/src-tauri（新增宿主配置命令）、desktop/src/SettingsPanel.tsx（重构为原生表单）、docs/decisions.md（D-026 追加取代记录）、PLANS/STATUS/REVIEW。不碰 runtime/ 源码（零差异验收）、不改 DSH Runtime、凭证不入日志/Git/evidence。

**步骤：**
1. **S1 Rust 配置层：** 新增宿主命令读写模型配置（BUILDER 先核实现有 provider 配置实际加载源——预计为数据根 ~/OmniDesk/runtime/dsh-home/profiles/web/cordis.patch.yml，确认后以实际为准并在执行记录写明）。字段：provider 名称、base_url、api_key、默认模型、分区权限开关。密钥只落盘，日志/命令返回/evidence 一律打码（仅留尾 4 位）；写入用临时文件+原子替换；文件损坏时回退只读并明确报错，不静默重置。
2. **S2 原生设置页：** SettingsPanel 改为四区表单：模型与端点 / 场景声明 / 权限 / 关于（高级）。「刷新端点信息」改读 S1 层；保留 052 重启提示逻辑不动。
3. **S3 场景声明区：** 显示 ~/OmniDesk/app/profiles/discipline.yml 绝对路径与当前载入指纹；「在系统编辑器打开」按钮（打开数据根文件，非仓库文件）；内嵌编辑框（载入→校验→写前自动备份→保存），校验失败禁止保存并显示原因；保存后显示"重启应用生效"（复用 decl_digest 现有机制）。
4. **S4 DSH 入口降级：** 主界面移除「打开 DSH 设置」按钮；在"关于（高级）"折叠区保留「打开 DSH 原生界面（诊断用）」，默认折叠，文案注明会打开完整 DSH 应用。

**验收：** cargo（含新增单测：原子写/密钥打码/损坏回退/patch 读写往返）、tsc、npm run build 全绿；git status --porcelain runtime/ 为空；代码审查确认设置页不请求 DSH 端口；真实窗口截图四区各一张（存 tests/verification/desktop/evidence/）；扫描全部 evidence 与日志确认无密钥明文。

**风险与停止条件：** ①写坏 patch 文件导致 DSH 起不来 → S1 先完成只读解析+往返单测，再开放写入，并用一次性副本实测写入后 dsh web 仍能启动；②实际配置加载源与预期不符 → 停下报告，不猜；③权限字段与 051 已有权限自管实现冲突 → 以已实现语义为准，不新造第二套。

### OD-ARCH-054：设置页深化与首次运行引导（已批准，053 全绿后执行）

**目标：** 用户拿到新构建后，不开文档即可自助完成"配模型 → 测连通 → 跑场景"。

**步骤：**
1. **S1 连接测试：** 设置页「测试连接」按钮——对 base_url 发 OpenAI 兼容最小请求（/models 或最短补全，载荷固定合成短句），超时 10 秒，结果显示成功/失败原因（网络/鉴权/格式三类起步）。
2. **S2 模型列表拉取：** 鉴权成功后拉取模型列表填充"默认模型"下拉；失败降级为手动输入，不算测试失败。
3. **S3 首次运行引导：** 启动检测无 provider 配置时，主界面显示一步引导卡（跳设置页），配置完成后消失；已有配置的用户不受影响。
4. **S4 权限开关核对：** 核对 051 权限自管项在原生设置页可见可改；已可用则只核对不重做，结论写 STATUS。
5. **S5 文档：** docs/user-testing-checklist.md 设置段落改为原生页流程。

**验收：** 同 053 回归口径；对不可达端点与错误密钥各有明确失败态截图（载荷不含任何真实材料）。

### OD-ARCH-055：场景页收尾核对（已批准，054 全绿后执行；只核对+小修，不新增功能）

**目标：** 三场景（周报/文案/项目跟进）在用户实测前排掉低级问题。

**步骤：**
1. **S1 成果路径核对：** 逐场景验证成果实际保存位置符合统一命名（OmniDesk 数据根；旧 YvanAI 路径只读兼容不迁移）；发现不符仅修路径常量/文档，不动数据。
2. **S2 空状态检查：** 各场景页在无历史会话、无成果时的空状态文案与引导是否明确；缺失则补文案（一处文案一提交粒度）。
3. **S3 场景声明联动核对：** 在 discipline.yml 隐藏一个场景入口 → 重启后入口消失且对应页面不可直达 → 恢复文件。全程用副本演练，结束后声明文件与演练前一致。

**验收：** 同 053 回归口径；S3 演练前后 decl 指纹一致记录。

**夜间总停止条件（三计划共用）：** 同一问题连续两轮修复未解决即停，写根因与阻塞报告；发现疑似真实材料/凭证、runtime/ 出现差异、需要改 DSH Runtime 才能继续时，立即停并报告，等 ARCHITECT 与用户决定。

### OD-ARCH-052：上手清单刷新 + 重启提示 + 移交用户实测（待批准，2026-10-21）

> **背景：** 051 复核通过（见 REVIEW.md 顶部）后，技术链路（会话续接、Profile 声明、设置内嵌、权限自管）全部就位。剩余关键路径是**用户真实模型端到端与三场景日常试用**——只能由用户本人执行。052 是移交前最后一小批。

**目标：** 让用户拿到一份与当前构建一致的实测清单；补上"声明改动需重启"的界面提示；把球交到用户手里。

**影响范围：** docs/user-testing-checklist.md（重写对齐 050/051 现状）、desktop 前端一处提示（条件渲染+文案）、PLANS/STATUS/REVIEW。提交推送沿用一次性授权模式（**用户批准本计划即批准执行中的分批提交与推送**）。不碰 runtime/、凭证、路由。

**步骤：**
1. **重写 user-testing-checklist**：开权限（应用内 DSH 设置窗口，"跟随 DSH 设置"为默认）→ 选模型 → 三场景各跑一单（周报/文案/项目跟进）→ 写确认卡流程 → 强杀进程重启恢复 → 改 profiles/discipline.yml 验证重启生效 → 记录卡点。每步标注预期现象与"不算数"的情形（如离线模板冒充模型产出）。
2. **前端重启提示**：声明文件被修改后（decl 与上次载入不一致或用户改文件后）界面显示"重启应用生效"提示，一处文案不改逻辑。
3. **STATUS.md 登记** + 051 遗留清单核对（清单化确认无遗漏）。

**验收：** 清单每步可在当前构建上照做；cargo / tsc / build 全绿；runtime/ 零差异。

**风险与停止条件：** 无架构风险；清单描述与实测不符时以实测为准修正；不新增功能面。

> **执行状态（BUILDER，2026-10-21）：三步完成，待 ARCHITECT 复核。**
> - ①`docs/user-testing-checklist.md` 已按 050/051 现状重写为 6 步 + 记录表，每步含预期现象与"不算数"的情形；界面文案逐条对照源码核对。②新增"重启生效"提示：`LoadedDecl` 增 `content_sha256`、新增只读命令 `profile_decl_digest`，前端 5 秒比对一次，仅在启动时载入的指纹与磁盘不一致时显示提示；**未引入依赖、未改解析语义、未热重载、未自动覆盖用户文件**。③STATUS.md 已登记，051 五项遗留逐条核对（均不阻塞）。
> - 回归：cargo 172（lib，含新增 3 个指纹单测）+2+13+3+1 全绿 0 失败；cargo build 通过；tsc 0；npm run build 通过；pytest 26 passed；runtime/ 零差异。
> - 真实窗口：`verify_decl_restart_hint.py` 6 项自动断言全 true（改文件出提示 / 用户改动未被覆盖 / 删文件换文案 / 不热重载 / 重启后提示消失且像素与冷启动一致 / 重启才重建默认声明）。
> - 限制：真实模型端到端仍未验证（需用户本人按清单执行）；"启动卡死"仍未复现、未认定修复。
</details>


### OD-ARCH-051：UI 合一 + 原生多轮会话 + 纪检 Profile 固化 + 设置页外观统一（待批准→派工，2026-10-21）

> **背景（用户 2026-10-21 口径修正后的定位）：** ①纪检是**一个** profile，周报/文案/项目跟进是它内部三个场景，不是三个 profile；②会话与历史存 OmniDesk 自有数据根、不依赖 DSH（已实现），缺的是接 DSH 原生会话续接（当前每轮新开会话+重放历史）；③旧 DSH 桌面会话导入功能**不存在、本计划不做**，需用户明确要求才立项；④设置页继续用 DSH 原生页面（D-026），仅外观向 OmniDesk 统一。

**目标：** UI 只留一份（合并用户实际使用的 omniharness-ui 版本）；对话不再每轮重放；纪检 profile 以文件形式固化（Skill+插件+默认设置声明）；设置页窗口外观统一。

**影响范围：** `desktop/`（Rust 宿主 + React 前端）、`engine/omnidesk_engine/web_api_adapter.py`、配置文件（纪检 profile 声明 + 场景入口注册）、PLANS/STATUS/REVIEW/GOALS/decisions.md、提交推送（沿用 047/049 一次性授权模式：**用户批准本计划即批准执行中的分批提交与推送**）。不碰 `runtime/`（零差异验收）、不碰凭证/路由、不接真实工作材料。

**批次 1（先行，有依赖顺序）：**

1. **S1 备份与合并 UI**：`omniharness-ui`（HEAD `5ffc710`，71 个未提交文件）先在它自己的仓库分批提交（每批前跑红线扫描 + 文件清单核对，清单附在执行记录中供用户过目；发现疑似真实材料/凭证即停）→ 按 OD-ARCH-026 既定方式（逐文件审查后择取必要前端，不整支强并）合入主仓库 `desktop/` 前端，旧界面文件冻结归档不删除。验收：合并后 `npm run build` 通过、cargo 全量绿、UI 行为与 omniharness-ui 版本一致（三场景入口 + 首页对话中枢骨架）。
2. **S2 接 DSH 原生会话续接**：web_api_adapter 由「每轮 session/new + 历史重放」改为 DSH Web API 的会话续接（045 第 5 步探针已实测该能力存在）；失败降级为现有行为并记日志，不静默丢轮。验收：多轮对话 token 消耗对比记录（同任务重放版 vs 续接版，合成端点下输入量应显著下降）；对话历史正确性不回退（现有 thread_store 回归全绿）；会话中断恢复行为不回退。

**批次 2（批次 1 验收后）：**

3. **S3 纪检 Profile 固化**：新增一份 profile 声明文件（数据根 `~/OmniDesk/app/profiles/discipline.yml` 或仓库配置目录，位置由 BUILDER 按现有配置加载机制选定并在执行记录中说明），内容：Skill 清单、插件开关、默认设置；周报/文案/项目跟进注册为该 profile 下三个场景入口；预留第二个 profile 的加载位置（只留结构，不实现多 profile 管理界面）。验收：改声明文件、不动代码，场景入口可见变化（新增或隐藏一个场景入口即算证明）；配置加载失败时回退默认行为并明确提示。
4. **S4 设置页外观统一**：D-026 内嵌 DSH 设置窗口的标题、配色、字体向 OmniDesk 主界面统一；不改 DSH 配置读写逻辑、不改会话令牌处理。验收：真实窗口截图；DSH 配置读写回归测试不回退。

**风险与停止条件：** ①omniharness-ui 合并若发现与主仓库前端结构性冲突（同文件不同骨架），停下报告冲突清单，不自行取舍；②S2 若 DSH 续接接口与 045 探针结论不符（接口漂移），停下报告，不猜内部协议；③S3 profile 声明若需动 Rust 代码才能加载，停下报告（说明为什么文件不够）；④批次 1 验收不过，批次 2 不开工。

**验收总表：** 合并清单已过目并推送到 omniharness-ui 仓库；cargo / tsc / build 全绿；runtime/ 零差异；S2 token 对比数据落盘 audit/；S3 改文件即生效的证据；STATUS.md 执行记录 + REVIEW.md ARCHITECT 复核记录。

**明确不做（本计划）：** 旧 DSH 桌面会话导入；多 profile 管理界面；安装包重打；真实模型端到端（等用户批准端点后按 docs/user-testing-checklist.md 走）。

> **执行状态（BUILDER，2026-10-21）：批次 1 完成，S1 按用户裁决改为窄口径整合。**
> - **S1 收口：命中停止条件①（结构性冲突）后由用户裁决，按窄口径完成。** `omniharness-ui` HEAD `5ffc710`（10-16）的未提交前端，已被主仓库 `2705148`（10-20「引入桌面 React 前端」）按**同一份 Aetheria OS 设计稿**落地并继续演进到 048/049/050；两边 `App.tsx`（76/161 行）、Dock 场景（4/8 个）、`api.ts`（239/828 行）骨架不同，整支合并会让主仓库丢掉会话、项目、设置、引擎诊断、线程持久化与流式对话。**未改任何前端源码文件，`omniharness-ui` 的 71 个未提交文件保持原状。**
>   **用户已裁决（2026-10-21）：「按最新的版本来，哪边最新用哪边，如果整合两边也行」→ 以主仓库现版为基线，只整合该分支独有且仍有用的资产。** 实际纳入：`_design/stitch_omnidesk/`（设计稿原件 7 个文件，主仓库 `theme.css` 的令牌注释本就指向这里但目录缺失）、`docs/omnidesk-ui-v2-handoff.md`、`tests/verification/desktop/verify_launcher_identity.sh`、`verify_port_ownership.sh`。未纳入：三个 `host_*_admission.rs`（依赖已被 048 重构掉的 `*_after_admission` 符号，直接复制编译不过；主仓库 `admission_classification.rs` 与 `host_integration.rs::admission_refuses_before_any_process_state_exists` 已覆盖同等能力）、`verify_engine_admission.py`（断言旧常量 `ENDPOINT_ISOLATION_CONFIRMED`，与 021 之后的宿主侧判定口径冲突）、`_preview.html`（临时预览）、`desktop/src/components/Icon.tsx` 等两边逐字一致的文件。因为不再整支合并，`omniharness-ui` 侧的 71 个未提交文件保持原状、未提交。
> - **S2 实现已在 048 落地，本轮补齐验收要求的对比数据。** 证据：`tests/verification/desktop/verify_resume_token_savings.py` → `tests/verification/desktop/evidence/od-arch-051/s2-token-savings.json`，11/11 通过。
>   **实测推翻了计划里的假设：** 前端组装层面三轮输入从 252 字符降到 78（省 69%），但**模型实际收到的量反而更大**（续接轮 6488 字符/6 条消息 vs 重放轮 623 字符/2 条消息）——DSH 续接会话时会把会话历史一起送给模型。因此本计划的「合成端点下输入量应显著下降」只在提示词拼装层面成立，**不能表述为节省 token 或节省费用**。
> - **S3、S4 未开工**，等 ARCHITECT 复核批次 1 后再决定是否开启批次 2（→ 用户 2026-10-21 指令「如果详细计划已经有了就继续往下」后开启，结果见下段）。
> - 未提交、未推送、未打标签；`runtime/` 零差异；全程合成数据 + 一次性数据根。

> **执行状态（BUILDER，2026-10-21）：批次 2 完成（S3 + S4），待 ARCHITECT 独立复核。**
> - **S3 完成，文件驱动。** 声明落 `<应用数据根>/profiles/discipline.yml`（首次用 `create_new` 创建，永不覆盖）。只决定 Dock 上的业务场景与品牌名；不改插件/路由/Skill/判定。**解析器是自写的受限 YAML 子集，未新增依赖**（离线构建，锁文件无 YAML crate）——遇到超出子集的写法（制表符、未知键、重复字段、嵌套映射、未闭合引号）一律报错，不猜测兼容。失败时回退内置入口并在界面显式报错，坏文件原样保留。内置工具入口（任务工作台/设置/引擎诊断）由代码固定，声明不可删改。
> - **S4 只完成窗口外壳**：标题 `OmniDesk · DSH 设置`、底色 `#faf8ff`、1160×820。**DSH 页面内部的字体与配色未改**——该窗口加载 DSH 原生页（D-026），样式由 DSH 控制，CSP 不允许本应用注入脚本/样式。**「外观统一」目前仅到窗口外壳这一层**，验收记录不得写成页面已主题化。DSH 配置读写与令牌处理未动。
> - **测试中发现并修复的缺陷：** 声明异步到达时，原校准逻辑会把正停留在设置页的用户强行切回首页（内置工具页不在声明管辖范围内）。已改为只校准 5 个业务场景。属普通实现缺陷，未改架构。
> - **真实窗口验收（非仅单测）**：①默认启动 → 宿主创建声明文件且与 `samples` 逐字一致，Dock 5 个业务入口；②改文件（去掉「文案写作」、改名「我的纪检工作台」）→ 重启后入口消失、品牌名变化，**未改代码**；③写坏文件 → 顶部红条报出路径与原因、回退内置入口、坏文件未被覆盖、进程存活；④设置窗口标题与尺寸符合，主窗口存活。证据：`tests/verification/desktop/evidence/od-arch-051/profile-decl-run.json`、`settings-shell-run.json` 与 `shots/`。
> - **回归：** cargo `169 passed / 0 failed`（lib，含新增 9 个声明单测）+ 2 + 13 + 3 + 1 全绿；`cargo build` 通过；tsc 0；`npm run build` 通过；pytest 26 passed；`runtime/` 零差异。
> - **限制：** 声明改动需重启应用生效（启动读一次，无热重载，界面未提示）；设置页内部外观未统一（见 S4）；`11-settings-window.png` 因 Xvfb 无窗口管理器只作旁证，以 `12-settings-window-only.png` 为准；用户报过的「启动卡死」在本批次两轮真实窗口中**未复现**，不认定已修复。声明文件是界面层配置，不是 DSH Profile。
> - 按计划授权范围分批提交并推送（A 代码 / B 测试与证据 / C 记录）。
> - **需 ARCHITECT 裁定的三处（BUILDER 未自行扩大）：** ①声明只做「场景入口 + 品牌名」，**未**把 Skill 清单/插件开关/默认设置写进去——那三样在 DSH 侧已有权威源，抄进界面声明即成双写；若要求声明一并掌管，属新增数据契约。②仓库原本无「读声明文件」机制，必须新增 Rust 模块（`settings.rs` 只读设置 JSON）；计划的停止条件③与影响范围（含 Rust 宿主）存在张力，按影响范围判断为在界内。③S4 只统一窗口外壳，页面内部受 CSP 限制未改。
> - **Git：** `18d133f` 代码 / `31b6d4a` 测试与证据 / `3f41f4e` 记录，已推送 `origin/main`。

### OD-ARCH-050
### OD-ARCH-050：修「新建任务后模型消失」+「打开 DSH 设置」改为应用内独立窗口（已执行，2026-10-21）

> **执行结果（ARCHITECT，2026-10-21）：用户批准（内嵌 DSH 设置页 + 同意清理旧进程）后执行。**
> **额外发现并修复的根因（计划外，用户当场症状的真正来源）：** 引擎子进程「启动后立刻退出」——`web_api_adapter._ensure_started()` 起 DSH 时没收回 stdin，DSH 继承了引擎的协议管道，Node 把这份共享的「打开文件描述」设为非阻塞，引擎随后 `for line in sys.stdin` 读到假 EOF（rc=0、无栈）安静退出；表现为模型目录永远读不到、引擎一启动就没。修复：`stdin=subprocess.DEVNULL` + 回归单测 `test_dsh_child_stdin_is_devnull`。
> **S1：** 宿主把 `EngineConfig` 快照放进独立 managed state（`HostSnapshot`），`engine_admission_status` 不再抢 `SharedHost` 锁 ⇒ 引擎冷启动期间也能读到端点事实（实测：应用启动 12 秒、引擎仍 `ENGINE OFFLINE` 时，界面已显示 `agnescn / agnes-3.0-flash`）。
> **S2：** 前端区分「读取中 / 已读到 / 读失败」，刷新期间保留上次事实；首次加载显示「读取中…」而不是「未识别」；失败时保留旧值并附「可重试」说明。
> **S3：** 「打开 DSH 设置」改为：复用本进程已起的实例 → 否则 `dsh web --port 0 --no-open`（输出按行打码后落日志）→ Rust 侧建**应用内独立窗口**「DSH 设置」。会话令牌只留宿主内存，日志 `token=***`；窗口不授予任何 IPC/文件能力；关闭窗口**不再**触发引擎收尾（只允许主窗口收尾，新增源码级测试）；退出时停掉自起的设置服务。
> **S4：** 已停掉 2026-10-20 18:27 起占用 127.0.0.1:3080 的旧 `dsh web`（pid 491674，旧版本按钮遗留），端口已释放。
> **S5（用户新要求）：** 新增「跟随 DSH 设置（推荐）」——归档 OmniDesk 覆盖文件（改名保留、不删除）+ 停引擎，之后新建任务/重启直接使用 DSH 里配置的 provider 与默认模型，不再要求在本应用重选；覆盖文件存在时仍以用户在本应用里的显式选择为准。修了随之暴露的两个问题：覆盖文件变化后 supervisor/事实快照里的 patch 列表不同步（会导致分类器拿到已不存在的补丁路径而拒绝），新增 `EngineConfig::sync_overlay` 并在写/归档后同步。
>
> **验证：** cargo 160 passed / 0 failed（新增 3 个源码级不变量测试 + 2 个 dsh_settings 单测）；`--test thread_store_restart` 1 passed；test_web_api_unit.py **26 passed**（含新增 stdin 隔离回归）；tsc 0；`npm run build` 通过；真实窗口实测：应用启动 12s 模型已知、引擎存活（`omnidesk_engine` 持续运行 3 分钟以上）、模型目录下拉显示 DeepSeek-V4.1-Flash/V4-Pro/AGNESCN、设置窗口独立打开并复用同一实例（同一端口）、关闭设置窗口后引擎仍在、日志无明文 token、`runtime/` 零差异。
>
> **未验证/遗留：** 真实模型端到端回答（本轮不接真实端点做生成）；思考等级在「跟随 DSH」下由 DSH 配置决定（原覆盖文件里的 `reasoningEffort: off` 已随归档保留在 `.bak-*`，如需恢复可切回或重新选一次）；`omnidesk-model.yml.bak-1792578256` 留在用户数据根供回滚。


> **证据（代码事实，非推断）：**
> - `lib.rs` `with_engine_call` 在持有全局 `SharedHost` 锁时调用 `ensure_initialized`；后者持 supervisor 锁执行 `initialize(Duration::from_secs(90))`（`engine_bridge.rs`）。web-api 通道冷启动要起 DSH web 服务，耗时数十秒。
> - `engine_admission_status` 为取 `EngineConfig` 需要同一把宿主锁 ⇒ 引擎启动期间该读取一直排队到 init 结束（最长 90s）。前端此时 `facts=null` ⇒ 「模型：未识别」；设置页进入分支 `!facts && !factsError` 直接显示「端点事实还没读出来」（`SettingsPanel.tsx:344`），把「加载中」误报成「读不出来」。用户看到的「几十秒后模型又出现」＝ init 完成、锁释放。
> - `task_shell::open_dsh_settings` 每次点击都 `spawn node <dsh_cli> web`（固定端口 3080、默认打开系统浏览器）。2026-10-20 18:27 起的旧实例（pid 491674）仍占 3080；之后每次点击在约 70s 后 `EADDRINUSE` 失败，只写 `~/OmniDesk/app/logs/dsh-settings.log`，界面只显示「已启动」，没有任何窗口。日志首行含 `?token=…`（会话令牌，不得外泄）。
> - 现有 UI 没有「应用内 DSH 设置窗口」；OD-ARCH-031 的设计是「跳系统浏览器」。

1. **S1 端点事实与引擎启动解耦（宿主，P0）：** 宿主启动时固定一份 `EngineConfig` 快照放入独立 managed state；`engine_admission_status` 只读快照 + `config_dir`，**不再取宿主锁**。提交路径的准入与守卫语义一字不改（仍走 `with_engine_call`）。新增源码级不变量测试（该命令内不得出现 `state.lock()`）+ 并发分类单测。
2. **S2 事实读取 UI 三态（前端，P0）：** HomeChat / SettingsPanel 区分 `loading / ready / error`；刷新期间保留上次事实（显示「正在刷新」），只有首次加载才显示「加载中」；读取加 5s 超时，超时给重试提示但不清空已知事实。模型选择器在目录不可用时仍显示 facts 里的 provider/model，不再退成「未识别」。
3. **S3「打开 DSH 设置」改为应用内独立窗口（宿主 + 前端，P0）：** 新命令按以下顺序工作——读数据根 `runtime/dsh-settings.json`（pid/port/started_at）→ 进程活着且端口可连则复用 → 否则 `dsh web --port 0 --no-open`（日志落盘前把 `token=` 打码），15s 内解析 `dsh web: http://127.0.0.1:<port>/?token=…` → 由 Rust 侧建 `dsh-settings` 窗口（标题「DSH 设置」）并直接传入 URL（**不经前端、不写日志明文**）。失败返回结构化原因（未启动/超时/端口/日志路径）。窗口不授予任何 IPC 与文件能力；应用退出时停掉自起的设置服务。旧固定 3080 路径移除。
4. **S4 现状清理（需用户单独同意）：** 停掉 2026-10-20 18:27 起占着 3080 的旧 `dsh web`（pid 491674）——那是旧版本按钮留下的进程；不删任何数据、不动 FSH 数据根。

**验收：** ①冷启动首个任务期间连续 3 次打开设置页，「模型与端点」始终显示 `agnescn / agnes-3.0-flash`、事实读取 ≤1s、不出现「未识别/读不出来」；②点「打开 DSH 设置」≤15s 出现独立窗口并显示 DSH 设置页，关闭再点复用同一实例，重复点击只有 1 个服务进程，日志无明文 token；③失败路径（如 DSH CLI 缺失）给明确错误与日志路径；④`cargo test --offline` 全绿、`tsc` 0、`npm run build` 通过、`runtime/` 零差异。

**风险与停止条件：** 远程页面在 Tauri 窗口里的 CSP/权限行为需实测——若被 app CSP 拦住且无法在不放宽主窗口 CSP 的前提下解决，停下报告，改用「系统浏览器 + 界面给出地址」的降级方案等用户裁定；token 是会话令牌，禁止写入日志/仓库/前端可见区域，解析方式若随 CLI 输出变化则停下报告、不猜。

**需用户裁定：** ①设置页形态＝内嵌 DSH 自有设置页（推荐，复用 DSH 本体、不重复实现）／自研模型提供方设置页（会与 DSH 配置双写，违背 D-024 复用原则）；②是否同意 S4 停掉旧 `dsh web` 进程。

### OD-ARCH-049：备份提交 + 会话日志版本化 + 终检设计（已执行，2026-10-21）

> **执行结果（ARCHITECT，2026-10-21）：三段按序完成。**
> ① **备份提交**：63 个未提交路径（44 改 + 19 新）按可编译单元分 5 批提交并推送 `origin/main`（`5ba0738` 引擎 / `f3a5dae` 宿主 / `a728907` 前端与启动器 / `5a0664a` 测试与证据 / `b3802bb` 文档与审计），每批提交前跑 `/tmp/od029/scan-staged.py`（5 批全 OK，零关键凭证命中）并人工核对文件清单；`runtime/` 零差异；工作区 0 项。
> ② **会话日志版本化**：`thread_store` 写入统一打 `version: 1`；读取端 `migrate_thread_record` 只读迁移（v0→v1 只补版本号，**不改写原文件**），未来版本拒绝写入并逐条报原因（`ThreadListing.issues`，前端 `storageNote` 显示）。新增 4 个单测（旧格式样例加载成功、写入打版本、未来版本拒绝+报错、强杀残留 tmp/半截文件下好记录仍可读）。
> ③ **执行入口终检**：`engine_submit_task` 在红线扫描之后、触达引擎之前做一次 `pending_writes::scene_gate`；命中同场景未决卡即整条任务拒绝（错误码 `pending_write_blocked`，消息点名卡片摘要），未命中原样执行；非场景 intent（首页闲聊等）不进入拦截范围。新增 4 个单测（同场景拒绝+指名、他场景放行、已结算/过期放行、坏记录拒绝）+ 1 个源码级接线测试（终检必须先于 `with_engine_call`）。
>
> **两处实现口径偏差（均已登记 REVIEW.md）：** ①「同一资源」在提交入口只能精确到**场景成果目录**——提交请求里没有目标文件名，故以场景为最小可判定边界；②待确认记录目录里出现**无法解析**的记录时选择**拦住并说明**（fail-closed），而不是当作没有记录继续执行。
>
> **未验证：** 真实窗口点选验证（本轮为单元/源码级证据）；真实模型端点下「有未决卡」的现场拦截体验待用户实测。

1. **备份提交（沿用 047 授权模式，用户批准本计划即视为批准本轮提交推送）：** 048 全部未提交改动（44 文件：代码/证据/文档含 048 复核记录、dsh-java 审计报告、audit/dsh-java-review/）按“可编译单元”分批提交并推送 origin/main。每批前跑红线扫描与文件清单核对；runtime/ 保持零差异确认。
2. **会话日志版本化（借鉴 dsh-java SessionFormatMigrationTest，P1-2）：** ~/OmniDesk 线程持久化文件写入时加 version 字段（当前 v1）；读取端遇到无版本/旧版本文件走迁移函数——只增不改写原文件，失败保留原样报错，不静默跳过；新增单测"无版本旧格式样例→加载成功"。验收：单测通过 + 强杀恢复回归通过 + cargo 全量绿。
3. **执行入口终检（借鉴 dsh-java 任务链兜底，P1-1，按用户三条语义条款收紧）：** 宿主任务执行入口加一次状态校验——检查结果只有两种：通过→任务原样执行；不通过→整个任务拒绝，错误信息指名道姓（如"有一条未确认写入（AI 复盘建议保存），请先处理"），**永不静默跳过部分内容**。拦截范围最小化：仅同场景、同一资源的未决确认卡才拦截本任务，其他场景卡片不挡。新增测试用例钉住"同场景未决卡→拒绝+指名错误"与"他场景未决卡→正常执行"两条。验收：两用例通过 + cargo 全量绿 + tsc 0。

**验收：** 提交分批清晰、远端同步；版本化与终检各有独立提交可单独回滚；全部回归绿（cargo/tsc/build）；runtime/ 零差异；STATUS.md 登记。

**完成后：** 真实模型端到端实测移交用户（按 docs/user-testing-checklist.md），产品命运转入使用数据裁决。


## 当前待批准计划与最近复核

### OD-ARCH-082：三大场景 Skill 深化 + Profile 资产绑定（用户已批准 2026-10-25，已执行、ARCHITECT 复核通过）

> **批准依据：** 用户 2026-10-25 明确「批准执行 OD-ARCH-082」。本节经 ARCHITECT 修订三处已过期前提后为唯一实施口径；批准不放宽范围，未列入本节的事项不属于本轮。
>
> **修订说明（ARCHITECT，2026-10-25，均为文件事实）：** ①数据根 `~/OmniDesk/runtime/dsh-home/profiles/` 实际只有 acp / headless / omnidesk-test / web / yvan-work，**没有 discipline Profile**；仓库里 `samples/profiles/discipline.yml` 是 `omnidesk.profile-decl.v1` 的 Dock 场景声明，不是 DSH Profile。②`TurnRail.tsx` 的 tooltip **已有完整视觉样式**（圆角、边框、阴影、最大宽度、问句一行/回复三行截断），真实缺口只有主题色边与 `aria-describedby`。③`TopNav.tsx` 搜索框当前是 `disabled` 的诚实占位（「全局检索（未实现）」）。
>
> **派工前定案（四条口径，实施不得自行更改）：**
> 1. **两态 Profile 组成：** web-api 通道白名单为 `omnidesk-base`（默认）与 `discipline`。两者 bundles 均为 `@deepseek-ai/dsh-base` + `@deepseek-ai/dsh-web-app`；web-app 是本地 Web API 与前端托管的**运行基础设施**，不算业务插件。不复制 web Profile 的第三方插件（free-search / cost-meter），不复制 yvan-work 的 inspector。acp 通道保持单一 `acp`，不受白名单影响。
> 2. **Skill 可见性由 agent preset 决定，不由 Profile 顶层补丁决定（已实跑验证）：** web-app bundle 在 preset 层挂载 `skill-filesystem` / `tool-skill`，默认 preset 是 `standard`；Profile 顶层 `- id: skill-filesystem` 的 `disabled` 只作用于宿主层，**拦不住 preset 内的那一份**（`--dump-config` 实测：顶层已 disabled，preset-standard 内两条仍在）。因此 `omnidesk-base` 的补丁必须覆盖 `preset-standard` 的 `config.plugins`（补丁层对 `config` 是整体替换），保留上游条目与顺序、把 `skill-filesystem` 与 `tool-skill` 标记 `disabled: true`。该覆盖有漂移风险，必须配一个校验脚本比对上游 `web-app/presets/standard.patch.yml` 的条目 id 序列。
> 3. **安全底线不随 Profile 关闭：** 红线词表与隐私掩码仍由「规则包是否安装」唯一决定，**本轮不改** `RulePolicy` 的加载语义。因此「默认模式」的准确表述是：不挂业务插件、不注入任何 Skill 正文、不使用场景 Skill 绑定；若用户装过规则包，红线与掩码照常生效。界面文案必须按此如实显示，不得宣传成「等同全新 DSH」。
> 4. **已有保存值不静默改写：** 当前 `settings.v1.json` 的 `engine_profile` 是 `yvan-work`，不在新白名单内。解析时**保留保存值原样**，本次启动回落默认 `omnidesk-base` 并在徽标/弹层显式说明「当前保存的 Profile 不在可选清单，本次使用默认」；不得自动写回、不得删除该目录。
>
> **正式启动路径的既有缺陷（本轮 S6 必须一并修，属 S6 前置）：** `scripts/omnidesk.sh` 在子进程里无条件 `export OMNIDESK_DSH_PROFILE="${OMNIDESK_DSH_PROFILE:-web}"`，而宿主 `engine_supervisor.rs` 以环境变量为最高优先级，导致设置里的 Profile 在正式启动时永远不生效。改为：脚本不再注入该变量（外部显式设置仍优先），由宿主读取 settings；acp 通道的默认仍为 acp。

**背景（用户定调）：** 桌面体验已达到类 DSH-desktop 水平，进入「场景 + 配套 Skill + 必要插件」阶段。OmniDesk 有了定制的 profile 后才是真正的 DSH 二开作品。当前 6 个 Skill 已存在于隔离数据根（material-writing / knowledge-search / knowledge-agent / document-review / memory-agent / project-agent），但质量与场景绑定深度不足，且 Discipline Profile 的规则资产与 Skill 之间缺一次系统性对齐。

**目标：** ①把三大场景（工作记录/文案写作/项目跟踪）的 Skill 内容按真实工作流深化到可用水平；②Skill 与 Profile 规则资产显式绑定（每个场景启动时加载哪些 Skill、哪些规则段），不再靠隐式发现；③每个 Skill 带合成评测用例，回归可跑。

**影响范围：** 数据根内 skills/ 目录内容（受 .omnidesk-managed 管理）、samples/profiles/ 模板、设置页规则资产分页展示、engine/ 侧 skill 注入链路（如需）；不改 runtime/、不改协议、不碰凭证。

**步骤：**
1. **S1 Skill 现状盘点（只读）：** 逐个读 6 个 skill 的 `SKILL.md`，记录触发条件、输入输出、当前与场景的绑定方式、质量缺口。只按文件事实，不按记忆判断。盘点简表写入 STATUS.md。
2. **S2 场景-Skill 绑定契约（schema v1 → v2，向后兼容）：** 在 `discipline.rules.json` 增加 `skill_bindings`：每个场景声明 `skills`（skill id 数组，允许空数组）与可选的注入上限。同步改 Rust `profile_rules.rs`（结构仍 `deny_unknown_fields`）、TS `types.ts`、`SettingsPanel` 规则资产分页、`verify_profile_rules_068.py`。校验：id 必须在六项清单内、同场景不得重复、单场景上限 3 个。**v1 文件必须原样可加载**（无绑定即不注入），不得自动改写用户已装规则文件。
3. **S3 三场景 Skill 深化 + 合成用例：** 按已确认工作流（工作记录=随手记轻装+快速整理；文案写作=素材收集+结构生成+多轮打磨；项目跟踪=事项登记+进度汇总+逾期提醒）重写对应 `SKILL.md`，含触发词、输入要求、输出结构、质量自检、数据边界引用规则包。顺带修正 `memory-agent`、`project-agent` 里残留的「Yvan AI Workbench」与 `~/YvanAI/...` 路径（改为 OmniDesk 事实）。每个 Skill 附不依赖模型的合成评测用例（输入样例 + 期望行为要点），固化为可跑回归。
4. **S4 注入链路（前端提示词组装，不动 runtime）：** 注入点在 `desktop/src/conversationPrompt.ts`（`composeConversationPrompt`）、`desktop/src/aiDraft.ts`（`buildDraftPrompt`）与 `HomeChat` 的组装处。按「当前 Profile + 当前场景绑定」把对应 Skill 正文拼入提示词，受 S2 上限与既有 `CONTEXT_MAX_CHARS` 约束；随手记分支保持轻装（最多一个绑定 Skill）。校验：切换场景后注入内容确实不同；续答（continuation）每轮重新注入并有用例覆盖；无规则包/无绑定时提示词与现状一致。
5. **S5 回归与三场景证据：** 三场景各用一条合成输入走真实窗口，落提示词组装事实（含命中的 Skill id 与片段），不依赖真实模型；全量回归见验收。
6. **S6 两态 Profile（可运行，create-new-only）：**
   - 新建 `omnidesk-base`（默认）与 `discipline` 两个 Profile 目录，bundles 均为 `@deepseek-ai/dsh-base` + `@deepseek-ai/dsh-web-app`；**只允许新建**，不得覆盖或删除既有 `acp` / `headless` / `omnidesk-test` / `web` / `yvan-work`。
   - `omnidesk-base` 补丁覆盖 `preset-standard` 的 `config.plugins`，保留上游条目与顺序，仅把 `skill-filesystem`、`tool-skill` 标 `disabled: true`；配校验脚本比对上游 `web-app/presets/standard.patch.yml` 的条目 id 序列，漂移即失败。
   - `profile_choice.rs` 加白名单：web-api 通道只出 `omnidesk-base` 与 `discipline`（默认前者），acp 通道仍 `acp`；保存值 `yvan-work` 保留原样，本次回落默认并在徽标/弹层显示「当前保存的 Profile 不在可选清单，本次使用默认」。
   - 修 `scripts/omnidesk.sh`：去掉无条件 `export OMNIDESK_DSH_PROFILE` 默认值，改为由宿主读 settings（外部显式设置仍优先）。
   - 徽标与 plate-soft 视觉体系统一，不新造视觉语言。
   - 证据：两态各启动一次引擎并用 `--dump-config` 比对插件与 skill 可见性差异（合成端点即可），落 `evidence/od-arch-082/` 的 facts JSON 与截图。
7. **S7 TurnRail 悬停卡（只补缺口）：** tooltip 已有形状、底色、阴影、宽度与截断，**只加主题色边与 `aria-describedby` 关联**，保持现有 plate / surface token，不重写组件、不改布局。验收：悬停与键盘聚焦都出现该卡，截图一张。
8. **S8 顶栏当前会话搜索：** TopNav 搜索框去掉禁用态，改为「当前会话内查找」：查询态由 App 持有，同时传给 TopNav 与 HomeChat（两者是兄弟组件，需该最小通道）；按消息轮次/段落文本匹配，命中高亮、上/下跳转、计数 `n/m`，无命中显示 `0/0`，Esc 退出。不解析 HTML、不做跨会话检索、不做虚拟化。验收：多轮合成会话命中并跳转、计数正确；命中态与无结果态各一张截图。

**明确不做（本轮范围外）：** REVIEW.md 顶部预检 A/B/C/D 列举的修复事项（另立计划）；修改 `runtime/` 内 DSH Runtime；改动凭证、模型路由或真实业务材料；跨会话全局检索；重写 TurnRail 或 TopNav 其余部分；删除/迁移数据根已有 Profile 目录；提交、推送、打标签。

**过夜批次（用户批准一次跑完、不需逐步审核）：** 按顺序执行 S1→S8；S1 盘点、S6 两态差异、S8 搜索口径都写进 STATUS.md；S3 用例与 S6/S7/S8 证据落 `tests/verification/desktop/evidence/od-arch-082/`；S8 完成后统一跑全量回归；完成后在 STATUS.md 写完整记录并停下等 ARCHITECT 复核。遇阻塞（skill 发现机制与注入方式冲突、白名单影响已有用户配置）按 AGENTS.md 停止规则处理：记录、不硬编、不扩大范围——涉及 `runtime/`、凭证、模型路由或数据根破坏性操作则立即停。

**验收标准（合并后）：** ①六 Skill 盘点有文件事实依据，绑定契约 v1 可读、v2 校验生效；②三场景注入内容确实不同，随手记保持轻装，续答每轮注入；③`omnidesk-base` 与 `discipline` 两态 `--dump-config` 差异可复现，preset 校验脚本能发现漂移；④下拉只出现两个选项，已保存的 `yvan-work` 原样保留且有显式提示；⑤TurnRail 卡片仅补边与 aria，悬停/聚焦均可见；⑥顶栏搜索命中、跳转、计数、无结果 0/0 均可用；⑦tsc 0、npm build 成功、cargo lib 全绿、pytest 全绿、`sync-skills.sh --check` 通过、`runtime/` 零差异；⑧evidence 目录含两态 facts、三场景提示词事实与 S6/S7/S8 截图。

**风险与限制（合并后）：** ①preset 覆盖属对上游结构的复制，升级 DSH 后可能漂移——由校验脚本兜住，漂移时停下报告而非静默放宽；②skill 全局发现仍存在：本轮以绑定注入为主，`discipline` 下模型仍可能自行发现未绑定的 Skill，不承诺强制隔离；③规则包可能仍生效：默认模式只是「不挂业务插件、不注入 Skill 正文、不用场景绑定」，装过规则包时红线与掩码照常；④会话搜索在数千轮时可能卡顿，本轮标注已知限制；⑤S8 命中定位以段落/轮次文本为主，不做 HTML 解析，Markdown 富文本内的跨节点命中可能漏配，需在 STATUS 如实记录。
### OD-ARCH-080：真实模型端到端实测准备 + 未提交改动分批推送（已执行并推送，2026-10-25，ARCHITECT 复核通过）

**执行结果：** 127 项改动按 4 批提交并推送（`4f8c578` 文档与状态 → `a4327ee` Rust 宿主 → `bd6afd8` 前端 → `328a498` 引擎与验证证据），`origin/main` = `328a498`，工作区干净。提交前把本地真实制度抽取目录 `.kb_extract/` 加入忽略并移出暂存，凭证模式扫描零命中。推送前全量回归：tsc 0、npm build 成功、cargo 257 passed、pytest 75 passed、`runtime/` 零差异。详见 STATUS.md 同编号条目。

**背景（承接 075–079 复核结论）：** ①首页对话区、附件、分支、索引条已对齐 DSH 原生，界面层打磨暂告段落；②工作区累计 127 项未提交改动，持续累积单点丢失风险；③用户下一步核心诉求是真实模型端到端实测（此前所有验证均为合成端点）。

**目标：** ①分批整理并推送未提交改动到私有仓库（备份目标，最高优先）；②为用户真实模型实测做好入口准备（不接触凭证、不修改路由）。

**影响范围：** Git 提交与推送操作、docs/user-testing-checklist.md 更新；不改 runtime/、不新增功能代码、不碰凭证与模型路由。

**步骤：**
1. **S1 分批提交推送：** 按逻辑分 3–4 批（①文档与状态文件；②Rust 宿主改动；③前端 UI 改动；④引擎与测试证据），每批提交前跑敏感信息扫描（sk-/Bearer/api_key/凭证关键词）+ 文件清单核对，确认无真实材料、无凭证后提交。推送前跑一次全量回归（cargo --lib + pytest + tsc + npm build）确认基线健康。推送目标为既有私有仓库远端。
2. **S2 实测清单更新：** 在 docs/user-testing-checklist.md 补入 078/079 新能力的实测步骤（附件选择框、答句五项操作条、轮次索引条、分支到新对话），并标注「真实模型端到端」为本次实测重点。
3. **S3 状态文件同步：** STATUS.md 记录提交批次与哈希；PLANS.md 标注计划状态。

**验收标准：** ①origin/main 与本地同步，无遗漏文件；②敏感扫描零命中；③全量回归全绿；④runtime/ 零差异；⑤实测清单含 078/079 新能力。

**风险与限制：** ①推送是不可逆操作，须用户明确批准后才执行；②若扫描发现真实材料或凭证，立即停止该批提交并报告，不自行处理；③真实模型实测本身仍需用户自行配置端点与凭证（065 已开放入口），BUILDER 不接触。

### OD-ARCH-079：四项实测修复（已执行，2026-10-25，已复核通过）

**背景（用户实测反馈 078 收尾）：** ①点「+」选仓库内文件（如 `AGENTS.md`）报「拒绝读取：该路径位于被禁止的目录（DSH / agent / 仓库内）」；②问句下方仍有「撤回 / 修改」两个不可用入口；③「分支到新对话」当时是本地重放的新线程，与 DSH 原生 `session/fork` 语义不一致；④缺少 DSH 原生对话右侧的轮次索引条。

**目标：** 附件读取边界只拦真正的运行数据根；问句操作条只保留可用项；分支走引擎原生 fork；补齐原生轮次索引条。

**影响范围：** `desktop/src-tauri/src/task_shell.rs`、`desktop/src/shell/{HomeChat.tsx,HomeCanvas.tsx}`、新增 `desktop/src/shell/TurnRail.tsx`、`desktop/src/theme.css`、`desktop/src/{api.ts,types.ts}`、`desktop/src-tauri/src/{engine_bridge.rs,lib.rs}`、`engine/omnidesk_engine/{contract.py,task_core.py,web_api_adapter.py,serve.py}`、`tests/verification/desktop/verify_home_polish_078.py`、`tests/verification/engine-interface/` 用例。不改 `runtime/`、不碰 `~/.dsh`、`~/.agents`、凭证、模型路由、真实数据。

**步骤与验收：**
1. **S1 附件边界拆分：** 新增仅供附件使用的禁止片段表（保留 `.dsh` / `.agents` / `.codex` / DSH Desktop 数据 / `deepseek-harness` / `omniharness`，移除 `myworkstation`）；产物落盘与 `read_material` 仍拒绝仓库内路径。验收：选仓库内 `AGENTS.md` 出现附件 chip 且无红色拒绝提示；产物路径仍被拒绝（单测各一条）。
2. **S2 问句操作条：** 问句下方只留「复制」，答句保留复制 / 朗读 / 重新生成 / 分支 / 耗时。验收：窗口截图里问句只有一项目可用操作。
3. **S3 原生分支：** 引擎侧转发 `session/fork {sessionId, atSeq}`，切点取实际 `turn/end` 序号记录在任务结果里；ACP 通道返回 `unsupported`；无切点时按钮禁用且不新建线程，失败如实提示。验收：`W11.json` 真实 fork（父结果不变、子会话继承前缀）、`W12.json` 无效切点如实报错；窗口负向证据不新建线程。
4. **S4 轮次索引条：** 新增 `TurnRail`，节距 / 留白 / 刻度比例 / 悬停预览沿用 DSH 原生 `TurnNavigator` 口径，`<lg` 隐藏、≥2 轮才出现、点击回到该轮。验收：刻度数等于问句数，点击首/末轮视图真的变化。
5. **S5 回归：** `tsc`、`npm run build`、`cargo test --offline --lib`、`python3 -m pytest tests -q`、真实窗口验收脚本全绿；`runtime/` 零差异。

**风险与限制：** ①附件放开到仓库内不等于放开任意敏感目录，运行数据根仍拒绝；②引擎侧 fork 的正向证据来自接口级真实调用，窗口内未跑通完整正向分支；③索引条悬停预览卡未单独截图验证；④本轮未用真实模型，全部为合成素材。

### OD-ARCH-078：附件入口放开 + 首页顶栏与对话细节打磨（已执行并复核通过，2026-10-25）

**背景（用户实测反馈 075–077）：** ①附件当前限数据根内 + 文本白名单，用户要求「+」号弹文件选择框、放开常见类型、允许桌面等任意外部路径（用户是唯一用户+开发者，多模态模型识别不了会自报）；②顶栏与对话区 8 处 UI 细节需打磨。

**目标：** 附件入口易用且不再误拒用户文件；顶栏信息收敛；对话区对齐 DSH 原生操作习惯。

**影响范围：** desktop/src/{HomeChat.tsx, attachmentPrompt.ts, shell/TopNav.tsx, HomeCanvas.tsx}、desktop/src-tauri/task_shell.rs（读取路径校验段）；不新增通信层、不改 runtime/、不碰凭证与模型路由。

**步骤：**
1. **S1 附件入口与类型放开：** 「+」号接宿主文件选择对话框（多选），保留拖拽；文件类型白名单改为开放校验（txt/md/word/wps/pdf/excel/et/csv/图片/视频等常见扩展放行），不再预判「不支持」；读取仍走宿主命令，正文原样进提示词+红线扫描；二进制格式读取失败时如实报错并提示换文本格式，不假装摘要；越界路径放开（允许数据根外，如桌面），但**红线扫描不豁免**（扫描对象仍含附件正文）。单轮数量（5）与总字数上限（120k）保留。
2. **S2 顶栏收敛：** ①Profile 徽标改窄（只显示「专业」二字，具体 profile 名移入下拉选项）；②删除顶栏人物头像占位块；③「LOCAL IPC ACTIVE」改「IPC ON/OFF」紧凑胶囊（按实际状态变色），腾出宽度给搜索栏；④问候语「晚上好·欢迎回到 OmniDesk」首页常态显示；⑤删机器人图片。
3. **S3 对话区对齐 DSH 原生：** ①历史对话去渐隐；②问句下加 tips：复制/撤回/修改；③答句下加 tips：复制/朗读/重新生成/分支到新对话/任务累计完成时间；④问答正文字号 13px→15px；撤回/修改/重新生成/分支均走既有引擎会话能力（撤回=删本地并同步引擎删除该轮，若引擎不支持则明确说明，不假装）。
4. **S4 验证：** tsc 0、npm build 通过、cargo 定向（task_shell 相关测试）；真实窗口截图（顶栏、问答 tips、附件 chip）；runtime/ 零差异。不提交不推送。

**验收标准：** ①拖桌面任意 txt/pdf/图片→不误拒，正文或诚实报错进对话；②顶栏仅保留：品牌/专业徽标/IPC 胶囊/搜索/日期/设置/通知/窗口控制；③问答 tips 与 DSH 原生功能对齐；④字号≥15px；⑤runtime/ 零差异。

**风险：** ①多模态文件（图片/视频）当前引擎是文本注入路线，无法视觉理解——本轮诚实处理（文件名+提示模型「请用户改用文本描述」），真多模态注入另评估引擎协议再立项；②越界路径读取涉及安全边界，已明确红线扫描覆盖；③「撤回」「分支」若引擎能力不支持，按钮置灰+说明，不硬编。

### OD-ARCH-077：功能补齐批次（已执行并复核通过，2026-10-24；偏差：附件正文由前端拼进提示词而非新增协议字段，数据边界裁定见 REVIEW.md）

**背景（用户指令）：** 对照 DSH 官方桌面把除插件与 Skill 外的功能补齐。经只读核查，当前缺口：①对话无文件附件/拖拽/@引用（官方桌面有，材料写作场景刚需）；②语音输入是占位按钮；③无托盘与全局快捷键。同时用户定调稳定性政策：**插件清单冻结**（出厂即定，增量只能走版本升级流程，不再运行时随意新增）；**Skill 平时可修改、不能随意增加**（新增走受控流程）。

**政策落点（先于功能）：**
1. **插件冻结：** 代码层不再提供「新增插件」入口；现有能力管理页只保留查看/启停/更新提示。更新流程 = 升级整个 OmniDesk 版本（版本内插件清单固定）。docs/decisions.md 记为 D-030。
2. **Skill 受控：** Skill 文件平时可编辑（改内容即时生效，重启或重新加载即用），但**新增 Skill 文件**在界面上不提供入口；新增通过「版本升级 + 更新说明」进行。能力管理页对新增动作做提示文案（说明该政策），不做强制文件锁（用户手改文件系统仍自由，但产品语义是受控变更）。

**功能步骤：**
1. **S1 文件引用与拖拽（优先级最高）：** 对话输入框支持：①拖拽本地文件到输入区（多文件）；②输入 @ 弹出最近文件选择器（限当前场景成果目录 + 知识库目录，经宿主路径校验）；③选中文件以附件 chip 显示，可删除；④发送时文件路径列表随消息体传给引擎，引擎侧把文件内容读出注入该轮 prompt（文本类直接读，二进制类给摘要占位）。路径校验复用宿主现有 `assert_local_root_allowed` 语义（只允许数据根内路径），越界拒绝并在 chip 上标红。不新增协议字段——复用现有消息结构，附件信息放在消息元数据里。
2. **S2 语音输入（按当前实现诚实处理）：** 当前按钮是占位（disabled）。本轮不做真实语音识别（涉及模型端点与授权），改为：按钮保留但点击弹出说明「语音输入需要配置语音模型端点，当前版本未开放」，不再显示禁用灰色样式误导用户以为即将可用。语音能力列入打包阶段评估项。
3. **S3 托盘 + 全局快捷键（可选，用户未强烈要求）：** 最小实现——系统托盘图标（左键显示/隐藏主窗口，右键菜单：打开主窗口/退出）；全局快捷键暂不做（涉及系统级权限，收益存疑）。若托盘实现代价超预期（依赖冲突/平台差异），标记「推迟到打包阶段」即完成本步，不硬做。
4. **S4 验证与证据：** cargo 定向 + tsc 0 + build 通过；拖拽/引用真实窗口截图；附件注入 prompt 的引擎侧单测；runtime/ 零差异。

**影响范围：** desktop/src/HomeChat.tsx（输入区）、desktop/src-tauri（新增 2-3 个命令：读场景目录文件列表、校验并读取附件内容）、engine/omnidesk_engine（消息体附件注入）、tauri.conf.json（托盘配置，若做 S3）。不改 ACP 协议、不改 runtime/、不碰凭证。

**验收标准：** ①拖拽 2 个文本文件到输入框→chip 出现→发送→引擎收到文件内容且回复引用了内容；②@ 选择器只列数据根内文件，越界路径被拒；③语音按钮说明文案准确；④插件冻结/Skill 受控文案在能力管理页可见；⑤cargo/tsc/build 全绿；⑥runtime/ 零差异。

**风险：** ①附件注入位置如果引擎侧结构不支持，可能需要在 engine 消息组装层加字段（最小侵入，不动协议帧格式）；②托盘依赖（tauri-plugin-tray）与当前版本兼容性——失败则推迟不硬做；③Skill 受控目前靠文案说明而非代码强制，用户手改文件系统无拦截——这是预期行为（用户就是开发者本人）。

### OD-ARCH-075：设置页通道能力说明（已执行并复核通过，2026-10-24；偏差：说明落在引擎诊断页，因界面本无通道选择器，通道由环境变量决定）

**背景：** 074 复核通过后，Luna 审计的 P1 项——通道能力差异说明——仍未落地。用户在 Web API 通道下会以为 ACP 通道同样支持界面审批，实际不支持且取消语义不完整，存在误操作风险。

**目标：** 在设置页现有通道选择位置加简短说明，与实际实现一致，不改代码行为。

**影响范围：** 仅设置页前端组件（SettingsPanel 或对应文件）+ 一处文案。不改引擎、不改协议、不改默认通道。

**步骤：**
1. 在通道选择控件下方加两行说明：Web API —— 支持界面审批与引擎取消；ACP —— 当前不支持界面化审批，取消请求不代表底层已确认终止。
2. 纯文案改动；若有 i18n 文件则同步补 key。
3. 验证：tsc 0、npm build 通过；截图一张（不需要全量回归，纯静态文案）。

**验收：** 文案与实际行为一致；不新增协议、不改默认通道；改动最小（预计 1 文件）。

**预计规模：** 单文件小任务，可与下一批次合并执行。

### OD-ARCH-076：单实例运行 + 移除 DSH 原生界面入口（已执行并复核通过，2026-10-24）

**背景（用户实测反馈）：** ①用户要求本应用同一时间只允许开一个窗口（当前无任何单实例限制，可开多窗并发触发引擎互斥报错）；②模型设置已全部收回 OmniDesk 原生设置页（D-027），设置页里「DSH 原生界面（诊断用）」折叠区仍会起一个完整 DSH Web 应用窗口（`task_open_dsh_settings` → `open_settings_window`），与「打开后看不到 DSH UI」的产品口径冲突，应移除。

**目标：** 第二次启动 OmniDesk 时自动聚焦已有窗口并退出新进程；移除 DSH 原生界面入口与相关死代码；不影响凭据 RPC（`credentials_rpc` 仍无头复用 SettingsService，不开窗口）。

**影响范围：** desktop/src-tauri/{Cargo.toml,lib.rs}、desktop/src/SettingsPanel.tsx、desktop/src/api.ts；`dsh_settings.rs` 保留（凭据 RPC 依赖），仅删窗口路径；docs/decisions.md 补记。

**步骤：**
1. **S1 单实例：** 引入 tauri-plugin-single-instance（需联网拉一个 crate；先核对与当前 Tauri 版本兼容，不兼容则用启动时检测已有主窗口的自研轻方案）；回调聚焦主窗口并退出新实例。验证：连续两次启动 → 第二个进程退出、已有窗口聚焦。
2. **S2 移除 DSH 原生界面入口：** 删 SettingsPanel「DSH 原生界面（诊断用）」折叠区与 openDshSettings 回调；删 lib.rs 的 task_open_dsh_settings 命令与 open_settings_window 函数及注册；删 api.ts openDshSettings 与相关类型。SettingsService 与 dsh-settings.json 记录保留（credentials_rpc 无头使用）。引擎/协议/runtime 不动。
3. **S3 验证与证据：** cargo 定向单测 + tsc 0 + npm build 通过；真实启动两次验证单实例聚焦；截图一张设置页（无 DSH 入口）。不提交不推送。

**验收：** 单实例生效（聚焦已有窗口）；设置页无任何打开 DSH 原生界面的入口；凭据保存/探测功能不受影响；runtime/ 零差异。

**风险：** ①插件兼容性失败则回退自研轻方案（检测已有主窗口即退出）；②误删 credentials_rpc 依赖的代码——S2 只删窗口路径，cargo 编译错误即为回归信号。

### OD-ARCH-047：夜间连续任务——045 补复核 + 备份提交 + 真实模型实测准备（待批准→派工，2026-10-21）

> **执行方式：** 三步按序，每步独立验证登记；失败记录后跳下一步。不碰凭证、不接真实端点、不改 runtime/。
>
> **执行结果（2026-10-21，用户已批准含推送）：** 三步全部完成。①045 补复核结论已写入 `REVIEW.md`；②84 个未提交路径按“可编译单元”分 3 批提交并推送 `origin/main`（`ba77c2d` 代码 / `a3bf0ad` 证据 / `bb09af1` 文档）——未按 045/046 逐块拆代码，因两轮改同一批函数，逐块拆会留下无法编译的中间提交；③新增 `docs/user-testing-checklist.md`，只读核实引擎当前实际路由为 `agnescn / agnes-3.0-flash`（与 `yvan-work` Profile 里的 `deepseek-v4.1-flash` 不同，因 OmniDesk 覆盖文件优先），未改动任何路由/凭证。回归：tsc 0、build 通过、cargo 165 passed。未打标签。

1. **045 补复核（ARCHITECT 职责，BUILDER 仅准备材料）：** 按 045 计划六步清单逐项核对（线程落数据根/成果目录树开关/首提卡顿修复/AI 复盘打通/DSH web 验证报告/整批回归），已覆盖项标证据、缺项列出；不重复已由 046 复核覆盖的部分。
2. **分批备份提交（需用户在此明确批准后执行）：** 25 项未提交改动按主题分 3-4 批提交——①045 线程持久化+成果目录树；②046 四项机制（pending_writes/scheduler/设置页/公告栏）；③借鉴研究与计划文档；④证据目录。每批提交前跑红线扫描与文件清单核对；提交后推送到 origin/main（沿用 OD-ARCH-029 一次性授权模式，本轮为第二次明确授权）。
3. **真实模型实测准备（只读准备）：** 检查 DSH 侧端点配置现状（不读凭证值）；整理"用户实测清单"——开权限→设端点→跑周报起草→看公告栏定时复盘→确认写卡流程，作为用户上手指南写入 docs/user-testing-checklist.md。

**验收：** 提交历史清晰分批、远端同步；清单文档可直接照做；045 复核结论写入 REVIEW.md。


### OD-ARCH-048：换接缝（D-024 路径 B）——对话通道切到 DSH 本地 Web API，继承逐字流式与思考链（已执行，2026-10-21，待 ARCHITECT 复核）

> **执行结果（BUILDER，2026-10-21）：四项目标全部实现，未提交、未推送、未打标签。** 引擎新增 `web_api_adapter.py`（与 ACP 并存，`dsh.channel` 切换，默认 web-api）+ 抽出 `task_core.py`；宿主加 `channel`/`engine_list_models`；前端 `ModelPicker` 走真实目录、`HomeChat` 消费真实增量。实测：W01–W10 一次性全 pass（逐字流式 0.381/0.615/0.866s 三块到达、取消 0.032s 落定、失败不伪装成功、重启恢复、未知会话即时拒绝、模型目录归一化）；ACP E01–E20 全 pass 未回归；`test_web_api_unit` 25 passed；Rust 165 passed；tsc 0、构建通过、分类器自检 24/24；`runtime/` 无差异。
>
> **四处计划偏差**（详见 `STATUS.md` 与证据 README）：①`turn/end` 实际 `completed`，`stop` 在 finish 帧；②路由分类器只认 `acp`，web Profile 被判 `route_missing`，故给脚本与 `route_classifier.rs` 加 `channel`（计划外最小修复）；③失败态实为 `error`，原映射会落 unknown，已修并补 `max-tokens`/`interrupted`/`blocked`；④`session/not-found` 是顶层错误帧，原实现拖到超时，已修并补单测。另修测试夹具 `admission_classification.rs`（同时声明两条路由、plugin name 需匹配）。
>
> **未验证：** 思考块流式（合成端点不产）、personaPrefix 是否真正送达模型（dump-config 生效但请求体探针未命中）、真实模型端点流式、真实窗口人工抽验、`wss://`（仅 `ws://`）。

> **卡死修复（同计划内追加，2026-10-21）：** 用户报「启动后整窗无响应、场景切不动、只能强制退出」。根因是 Tauri 2 同步命令在 UI 主线程执行，49 个同步命令持全局宿主锁等引擎冷启动，堵住重绘。已把它们的属性改为 `#[tauri::command(async)]`（函数体、签名、锁语义不变），首屏读数据根加 4 秒上界（超时先用本地缓存，后台补上），并新增回归测试 `every_tauri_command_runs_off_the_ui_thread` 钉住这条不变量。实测（Xvfb 隔离显示 + 合成 40s 慢启动、修复后重新构建的二进制）：五个场景来回切换在 0.22–0.84s 内完成且画面正确，等待期间窗口全程可交互，40s 后引擎正常接通。回归 166 passed / 0 failed、tsc 0、构建通过、`runtime/` 无差异。**同轮更正：** 早先记录的 0.17–0.43s 是旧二进制 + 只截 Dock 区域的错误判据所得，已在证据 README 标注并替换。**未验证：用户真实机器同一故障状态未复现、真实 DSH 冷启动耗时未测。** 证据见 `STATUS.md` 同章节与 `tests/verification/desktop/evidence/od-arch-048/README.md` 第 7 节。

- **背景：** 045 第 5 步探针已实测 DSH 本地 Web API 具备：token 级流式（WebSocket /api/remote.mux 的 assistant-stream 帧：block-start/text-delta/finish/usage）、同 sessionId 连续两轮、取消（19ms 落定）、重启后会话持久、模型目录。现 UI 走 ACP 窄接口，整段到达、只能本地模拟打字机。用户批准走 D-024 路径 B。
- **目标：**
  1. **引擎侧新增 Web API 适配通道**（与 ACP 通道并存，配置切换）：会话创建/续接/取消/历史读回/模型目录直连 DSH 本地 HTTP+WebSocket API；参数格式按探针记录（args.request 包装、throughSeq 需 follow 快照 cursor）。
  2. **逐字流式端到端**：assistant-stream 的 text-delta 帧经引擎→宿主→前端实时推进对话区；打字机模拟退役（保留为降级）。思考块流式若模型端点支持则同样透传（合成端点不产思考内容，此项标注未验证即可）。
  3. **模型目录直读**：模型选择器改用 session/modelCatalog 真实数据（provider/分组/reasoning efforts），替换本地手搓列表。
  4. **ACP 通道保留可切**：配置项选择通道（默认 web-api；异常时一键切回 acp），确保升级风险可控。
- **影响范围：** engine/omnidesk_engine 新增 web_api 适配模块与通道切换；宿主 engine_bridge 增加对应转发；前端对话区流式渲染改造+模型选择器数据源替换。**不改 runtime/、不改 DSH 源码、不动凭证与路由文件；DSH 本地 API 属内部接口，已在 decisions.md 记录"锁定 runtime 版本"约束。**
- **验收标准：**
  - 合成端点（带 chunk 间延迟）下真实窗口逐字流式：文本分多次到达而非一次性；连续两轮同 sessionId；取消 1 秒内界面落定。
  - 进程重启后恢复会话并继续对话（resume 路径走 web-api）。
  - 模型选择器显示 modelCatalog 真实内容（含 reasoning efforts）。
  - 切回 acp 通道后既有行为不回归（全量测试+真实窗口抽验）。
  - Rust/tsc/构建全绿；新增适配层单测覆盖帧解析与参数包装；runtime/ 无差异。
- **风险与停止条件：** DSH 本地 API 为内部接口——若发现接口行为与探针记录不符（版本漂移），停下报告不猜；若流式帧格式需要 runtime 改动才能适配，停下报告；凭证注入方式沿用探针的隔离环境变量方案，不新增凭证读取面。
- **执行方式：** 建议分两段连续执行——第一段引擎+宿主通道打通（含单测），第二段前端流式渲染+模型目录（含真实窗口证据）；ARCHITECT 中途不介入，完成后一次性复核。

### OD-ARCH-046：借鉴四项落地——写确认制 / 自动降级 / 定时·心跳 / Agent 式设置页（已执行，2026-10-21，待 ARCHITECT 复核）

> **执行结果（BUILDER，2026-10-21）：四项全部实现，未提交、未推送、未打标签。** 回归：tsc 0 错误、`npm run build` 通过、`cargo test` 165 passed / 0 failed（lib 146 + 2 + 13 + 3 + 1）、`cargo build` 通过。真实窗口实测：设置页五卡、once 定时触发、无确认不落盘、确认后落 `weekly-report/` 子目录、权限关闭时定时不执行、模型断开时降级、起草无模板假输出。逐项证据见 `tests/verification/desktop/evidence/od-arch-046/README.md`；`cargo fmt` 未运行（本机无 rustfmt）；"过期记录界面提示"仅有单测证据。

- **来源：** docs/research-borrow-cherry-lingxi.md（灵犀工作台 + Cherry Studio 对标）；用户圈定四项。
- **范围与实现要点：**
  1. **写确认制（宿主强制）**：新增统一的"待执行写操作"机制——凡是模型/定时任务发起的写动作（保存成果、更新任务状态、写入建议文件），宿主生成待确认记录（含动作摘要与影响范围），前端渲染确认卡片，用户点击后宿主校验该确认记录存在且未过期才执行；无确认记录的写请求一律拒绝（代码级，不靠提示词）。人工在对话中点"保存"的既有路径不变。
  2. **自动降级（分级诚实）**：提醒/巡检/AI 复盘类功能在模型不可用、超时或权限关闭时自动退回本地确定性结果（规则提醒本来就是本地计算），公告栏明示"AI 暂不可用（原因），已用本地规则结果"。**内容生成类（周报/文案起草）不降级为模板**——失败就明确失败，不用模板冒充模型产出（数据诚实红线）。
  3. **定时/心跳**：宿主内置应用运行期调度器（不做开机后台守护）。两类：**定时任务**（cron 表达式或预设，如每周一 09:00 生成周报 AI 复盘建议，须权限开+确认制放行）；**心跳**（可配间隔，默认 2 小时，扫描拖延/超期/日程，刷新公告栏规则提醒，不调模型）。配置存 ~/OmniDesk 数据根设置文件；错过触发下次打开补跑一次。
  4. **Agent 式设置页**：设置页重构为卡片组——模型与端点（现有跳转 DSH+刷新保留）、模型权限三档、定时与提醒（新）、成果目录与下级开关（已有能力收拢）、数据与日志（保留期限展示）。每卡显示当前状态摘要与主操作；不改权限默认关闭语义。
- **影响范围：** desktop/src 前端（设置页、公告栏、确认卡片组件）、Rust 宿主新增 3-4 个命令（待确认写操作 CRUD+校验、调度器配置读写、触发事件）；engine 侧不动；不动 runtime/、协议、凭证、路由。UI 实现前按派工模板先搜同类产品设置页参考。
- **验收标准：**
  - 定时/心跳：合成端点下，把定时任务触发时间改到 1 分钟后实测触发；权限关闭时定时 AI 任务明确不执行并在公告栏说明原因；心跳扫描产出与手动"逾期扫描"结果一致。
  - 写确认制：模拟一次定时任务尝试写建议文件——无确认时不产生文件；点确认后落盘到数据根该场景目录；确认记录过期（如 24h）自动失效。
  - 降级：断开引擎/关权限两种情况下，公告栏出现规则提醒+可见降级说明；周报起草请求明确报错，无模板假输出。
  - 设置页：五张卡片状态真实反映当前配置；改动即时生效；截图佐证。
  - 回归：Rust/tsc/构建全绿；不提交不推送。
- **风险与停止条件：** 调度器不得自起常驻进程或系统级定时（越界即停）；确认机制若发现需要改 engine 协议，停下报告。

### OD-ARCH-045：功能完善夜间批量任务（已执行，2026-10-21，待 ARCHITECT 复核）

> **执行方式：** 六步按序执行，每步独立验证并登记；某一步失败记录原因后跳到下一步，不反复重试。全程不提交、不推送（等用户晨间复核）；不碰凭证、不接真实模型端点（一律合成端点验证）；不改 runtime/。
>
> **执行结果（BUILDER，2026-10-21）：六步全部执行完毕，未提交、未推送。** 第 1/2/3/5/6 步按验收通过；第 4 步功能通过但有 1 处计划偏差（复盘成果落在数据根顶层，未按场景分目录）。回归 `cargo test --offline` 154/154、tsc 0、构建通过；第 5 步思考块与工具事件未验证；首页对话尚未接入线程持久化。逐项证据与缺口见 `STATUS.md` 同名小节与 `tests/verification/desktop/evidence/od-arch-045/README.md`。

1. **线程持久化到数据根（收 033 遗留①）：** 新增宿主存储命令，把工作台线程/会话记录落到 ~/OmniDesk 数据根、按项目分目录；前端本地存储降为缓存，启动时从数据根恢复。旧数据只读兼容，不删除不覆盖。验收：新建线程→重启应用→从数据根恢复；git status 不出现任何线程记录文件。
2. **成果目录树与下级开关（收 033 遗留②）：** 宿主新增成果目录列举命令（带目录层级）；每个场景设置"是否含下级目录"开关（默认不含）；三个场景页成果右栏按开关显示并可打开文件。验收：开关切换后右栏树状/平铺正确变化。
3. **首次提交卡顿修复（收 041 遗留）：** engine_submit_task 改为异步启动引擎（UI 立即给"正在启动"状态，不冻结）；权限校验保持同步语义不变。验收：冷启动后首次提交 UI 不冻结，可取消；既有测试全绿。
4. **AI 复盘建议打通（默认关闭，权限开时可用）：** 公告栏"AI 复盘"按钮接通引擎（走现有权限三档开关）；关闭时按钮明确置灰并说明原因；用合成端点验证一次真实调用与建议文件落盘（存仓库外数据根该场景目录）。验收：开/关两种状态行为正确；建议文件在数据根不在仓库。
5. **D-024 第 2 步：DSH 自带 Web 应用本地 API 验证（只读实验，零产品代码）：** 在隔离环境起 dsh-host-webserver，用合成端点经本地 API 发消息，验证 token 级流式、思考块、模型目录、会话续接是否真实存在，形成证据报告。验收：报告含逐项"存在/不存在"结论与复现命令；不改任何产品代码与 runtime/。
6. **整批回归与证据包：** Rust/tsc/构建全绿；真实窗口截图（首页+三场景页+成果右栏开关两态）；更新 STATUS.md 与 PLANS.md 各步登记。验收：证据目录齐全，晨间可独立复核。

**停止条件：** 任一步需要改 runtime/、协议、凭证或真实端点，停下登记；需要 git 操作时停下等用户。
### OD-ARCH-035：接入免费 UI 设计参考 MCP（screenshots-design-inspiration）（用户已批准；BUILDER 已执行；待 ARCHITECT 复核，2026-10-20）

> **执行口径修正（2026-10-20，用户当场纠正）：** 目标环境是 **Codex**，不是 OpenCode。
> 实际配置写在 `~/.codex/config.toml`（`[mcp_servers.screenshots_design_inspiration]`），
> OpenCode 配置未作任何改动。下文步骤 1、2 中「OpenCode」一词按此口径替换为「Codex」。
> 另因本机走代理而 Node 默认不读代理环境变量，该 MCP 专属 env 增加了 `NODE_USE_ENV_PROXY=1`。
> 证据与未验证项见 `tests/verification/desktop/evidence/od-arch-035/README.md`。

- **目标：** 为 BUILDER 接入开源设计参考 MCP，使后续 UI 任务实现前能先搜真实产品界面作参考。不涉及产品代码改动。
- **步骤：**
  1. 在 Codex 的 MCP 配置（`~/.codex/config.toml`）中添加：command = "npx"，args = ["--yes", "screenshots-design-inspiration-mcp@latest"]（Node.js 已具备即可，无需克隆仓库）。
  2. 重启 Codex 后验证：调用 search_inspiration 搜 "AI chat workspace dashboard" 类关键词，确认能返回截图结果。
  3. 在任务指令模板中加入固定条款：UI 实现前先用该 MCP 搜 2-3 个同类产品布局（关键词示例：chat workspace / sidebar dock / task dashboard），给出 2 句以内的借鉴要点，再动工。
- **验收：** Codex 会话中能成功调用该 MCP 并返回截图；未成功时报告失败原因（网络/Node 版本），不降级硬编。
- **风险：** npx 首次运行需联网下载包；该 MCP 数据为静态截图库（410+ 产品），覆盖有限，效果不足再评估 Mobbin 付费版。
### OD-ARCH-042：顶部卡片默认显示 / 静止恢复（**已执行，2026-10-21，ARCHITECT**）

- **状态：已执行。** 打开即显示卡片；指针进对话区卡片隐藏（与对话互斥）；
  鼠标静止满 5 秒且输入框无焦点 → 卡片回来；输入框有焦点 → 不恢复。
- **验证：** 四态真实窗口截图（`evidence/od-arch-042/shots/`）；tsc 0、构建通过；未改 Rust。

### OD-ARCH-041：场景切换卡顿（**已执行，2026-10-21，ARCHITECT**）

- **状态：已执行。** 根因＝`engine_admission_status` / `task_list_model_routes` 在主线程同步 spawn node，
  而对话区每次场景切换重新挂载 ⇒ 每次切换重拉 ⇒ 整页卡住。处置＝宿主侧改 async（用 AppHandle 取状态）
  + 前端缓存（模型候选缓存到失效、端点事实 2 秒短缓存）。
- **实测：** 首页→周报 6.07s→0.62s；周报→首页 3.51s→0.48s；首页→项目跟踪 6.03s→0.46s；
  项目跟踪→首页 3.35s→0.73s。回归 Rust 143/143、tsc 0、构建通过。
- **未解决（另立计划）：** 首次提交仍会卡一下（`engine_submit_task` 同步且要启动引擎进程）。

### OD-ARCH-034：真多轮接线——去掉上文重放（**已执行，2026-10-20，ARCHITECT**）

- **状态：已执行（用户批准后由 ARCHITECT 执行；提交见 REVIEW/STATUS 同名小节）。**
  同一线程复用同一引擎会话（`new` / `continued` / `resumed` / `degraded` 四种会话取得方式，
  回执与台账带 `session_mode`），前端不再重放上文；降级时下一轮自动补重放。
  **按事实修正了计划措辞：** DSH 的 `session/resume` 只接受已落盘可重开的会话，对活会话报
  `invalid_argument` ⇒ 进程内续发直接再发 `session/prompt`，进程重启后才 `session/resume`。
  验证：单元 27/27、真实引擎+合成端点 33/33（消息条数 3→5→7）、提示词 24/24、
  真实窗口台账 `new→continued` 且会话号相同、Rust 143/143、tsc 0、构建通过。
  证据：`tests/verification/desktop/evidence/od-arch-034/`。
  **本轮不含 D-024 第 2/3 步**（DSH web API 验证与路径 A/B 裁定）——按计划另行立项。

- **背景：** D-024 复盘已核实：ACP 本身支持 session/prompt 与 session/resume，我方适配层也已实现 recover_session，但 submit_task 每轮新建引擎会话、前端靠"上文重放"模拟多轮。这是自造绕路，不是引擎限制。接线后可获得真多轮与原版延迟。
- **目标：** 同一工作台线程复用同一引擎会话：线程首次对话用 session/new 并记住 engine_session_id；后续轮次经 session/resume + session/prompt 续发，不再重放全部上文。前端线程记录持有 engine_session_id 并持久化。
- **影响范围：** engine/omnidesk_engine/acp_adapter.py（submit_task 接受既有 engine_session_id）与 engine_bridge.rs 的传参打通；前端线程存储加字段。不新增通信层、不改协议、不动 runtime/、不动模型路由与凭证。
- **验收标准：**
  - 同一线程连续三轮对话，适配层日志显示第一轮 session/new、后两轮 session/resume，界面无需重放即有完整上下文（第二轮能引用第一轮内容）。
  - 线程重启应用后恢复，续发仍走 resume；会话被关闭（close）后 resume 仍可用（ACP 已注明支持）。
  - 旧线程（无 engine_session_id）首次续聊自动降级为新建会话并开始持有 id，不报错、不丢历史。
  - Rust 与 tsc 测试全绿；新增适配层单测覆盖 new/resume 分支。
- **风险与停止条件：** 若 resume 后上下文语义与预期不符（如引擎侧不带历史），停下报告而不是保留两套机制；本轮不做 D-024 第 2/3 步（DSH web API 验证与路径 A/B 裁定），那两步另行立项。
- **执行方式：** 本计划与「D-024 第 2 步验证」（只起 DSH 自带 web 应用验证流式/思考/模型目录是否存在，不改产品代码）可合并为一个任务交给 BUILDER 连续执行；第 2 步只产验证报告，不产产品代码。

### OD-ARCH-033：场景模块页设计 v3（已执行，2026-10-20，ARCHITECT）

- **状态：已执行（2026-10-20，ARCHITECT 执行；原计划派 super-worker，因该会话 agent 目录发现不到项目 agent 而改由主会话执行）。**
  已实现：骨架插槽化（首页与场景页共用 `SceneScaffold`，首页版式未变）、周报/文案双栏 + 项目跟进「进度/阶段成果」页签、
  顶部提醒/建议栏（规则提醒＝既有确定性汇总；AI 复盘建议＝可选模型调用、默认关闭、成本透明）、
  会话按入口打标与事后改挂。**按停止条件停下并报告两项：** ①线程记录真正落到 `~/OmniDesk/...` 并按项目分目录
  （现为浏览器本地存储，已确认不在仓库内）——需宿主新增读写能力；②成果目录「是否含下级目录」开关——
  宿主成果列举是平铺匹配、无目录树能力——需宿主新增目录列举命令。另「数据分析占位页」未新增（Dock 现有「任务工作台」
  即分析试点）。证据：`tests/verification/desktop/evidence/od-arch-033/`（脚本 + 6 张真实窗口截图 + 索引）；
  回归 Rust 144/144、tsc 0、构建通过、verify_conversation_prompt 12/12、verify_ai_draft 27/0。

### OD-ARCH-033：场景模块页设计 v3（设计确认稿，待用户批准，2026-10-20）

- **新增需求（用户提出）：** 周报/文案页在双栏之上再加一条横向「提醒/建议栏」（公告栏式），动态展示 AI 定时复盘成果给出的建议；周报侧重拖延/超时/进度提醒，文案侧重结合趋势热点的写作建议。
- **提醒/建议栏设计定稿：**
  1. **两类信息分开来源：** ①「规则提醒」= 纯本地确定性计算（对照截止时间/最近更新时间判断拖延与超期），不花模型 token、不依赖模型在线，项目跟进页的进度区同样复用；②「AI 复盘建议」= 定时任务触发模型，对本场景近期成果与对话做一次复盘，结论写成一份建议文件存入该场景数据目录，公告栏只展示最新一条并可点开历史。
  2. **定时方式（第一版克制）：** 应用运行期间由前端定时器触发（用户在设置里配置频率，如每天/每周、具体时间点错过则下次打开时补跑一次）；不做开机后台常驻进程。AI 复盘走现有引擎与权限开关，遵守模型权限三档设置，复盘输入仅限本场景成果与对话记录（位于仓库外数据根），不扩大范围。
  3. **文案「趋势热点」单独标记为受限能力：** 需要引入外部网络数据源（联网检索），涉及外部服务与合规边界，第一版不做。文案页公告栏第一版只做「AI 复盘写作建议」（基于用户已有成果与方法沉淀）；趋势热点等用户明确数据来源与授权后再议。
  4. **成本透明：** 每次 AI 复盘前公告栏显示预估说明（将调用一次模型），设置里可单独关闭 AI 复盘只留规则提醒；复盘产生的 token 用量计入现有用量显示。
- 

- **背景：** 用户细化场景模块页需求：周报/文案为双栏版式；项目跟进需进度与阶段成果；数据分析流程未定；成果目录只读本场景文件夹（是否含下级目录可设置）；对话历史要从 DSH 拆出、由 OmniDesk 自主持有；并确认复用首页骨架。另提出"场景对话能否自动对应到项目"的设计问题。
- **设计定稿（v2）：**
  1. **骨架复用（用户已同意）：** 首页与场景页共用同一布局组件，顶栏/Dock/下部对话区完全一致，仅上部两栏按场景注入。
  2. **周报页 / 文案页（同版式双栏）：** 左栏=该场景近期历史对话（点击加载到下方对话区）；右栏=该场景成果档案。右栏只读取本场景自己的成果文件夹；是否包含下级子目录做成设置项（默认不含），保证各场景文件管理边界清晰。
  3. **项目跟进页（三块内容）：** 同版式，右栏改为带两个页签的面板——「进度」（事项状态、逾期与阶段汇总，数据来自现有确定性汇总逻辑）与「阶段成果」（本场景成果文件，下级目录设置同上）。左栏仍为历史对话。
  4. **数据分析页（占位）：** Dock 保留入口，页面显示"该场景规划中"空态说明，不实现功能，等用户定流程。
  5. **对话历史持有（回应"从 DSH 拆出来"）：** 现行机制本就是工作台每轮新建引擎会话、把上文重放进下一轮提示，完整对话史已由工作台本地线程持有，不依赖 DSH 内部状态。本计划做两件事：①确认（必要时迁移）线程/会话记录存放位置在仓库外的 OmniDesk 数据根、按项目分目录，不在仓库内；②迁移只做只读兼容，不删除、不覆盖旧数据。DSH 内部会话继续视为可丢弃缓存。不动 runtime/、不新增通信层。
  6. **场景自动归类（回应"对话内容能否自动对应到项目"）：** 采用"入口即标签，事后可改"方案：从哪个场景页/Dock 入口新建的会话，创建时自动打上该场景标签并归入其项目；对话过程中不用模型猜归类（不可靠）。从普通对话入口新建时先选场景（默认普通，可选具体场景）；支持事后手动把会话改挂到其他场景/项目（会话列表处改标签）。标签随会话持久保存，作为左栏过滤与成果归类的唯一依据。模型自动识别归类列为后续可选增强，默认不做。
- **目标：**
  1. 抽取首页布局为可复用骨架，上部两栏成为按场景注入的插槽。
  2. 实现周报/文案页（双栏：历史对话+成果）、项目跟进页（历史对话+进度/成果页签）、数据分析占位页。
  3. 会话创建时按入口自动打场景标签并归入项目；支持事后手动改挂；标签持久化。
  4. 工作台线程/会话记录确认或迁移到仓库外数据根（按项目分目录）；旧数据不删不改。
  5. 场景成果目录读取范围（是否含下级目录）提供设置项，默认不含。
- **影响范围：** 以 desktop/src 前端为主（骨架插槽化+场景页+会话打标+设置项）；若线程记录当前存放在仓库内，迁移只调整宿主存储路径常量（属路径配置调整，不属新增通信层）；无 runtime/ 改动，不动模型路由与凭证。
- **验收标准：**
  - 三场景页与首页同版式；左栏点击历史对话可加载到下方对话区。
  - 周报/文案右栏只显示本场景文件夹成果并可打开；设置开关能切换是否含下级目录。
  - 项目跟进右栏可切换「进度/阶段成果」页签，进度数据与现有汇总一致。
  - 从周报页新建的会话自动带周报标签；普通入口新建时可选场景；事后可改挂标签并立即反映到左栏列表。
  - 会话记录文件位于仓库外数据根；git status 不出现任何会话记录文件。
  - 构建与既有测试全绿；真实窗口截图（首页+周报页+项目跟进页）佐证。
- **风险与停止条件：** 若会话打标需要新增宿主命令或改协议，停下报告；若线程记录迁移涉及既有数据格式，只做新增不重写，遇冲突停下报告。

### OD-ARCH-032：首页定版还原 + 项目/会话模块页（2026-10-20 已执行，待 ARCHITECT 正式复核）

- **状态：已执行完成（2026-10-20，ARCHITECT 执行，提交 `828c5a6`）。** 三件事：①**首页还原定版**——移除首页里的
  ProjectRail 与左右分栏，首页恢复 `HomeCanvas` 全宽对话版式（**没有**项目树、**没有**分栏，左侧仍只有 Dock 进各场景）；
  ②**项目/会话模块页**——项目分组、会话列表、进行中的工作台对话、新建对话与预设选择，整体迁入一个 Dock 模块页
  （改造既有 sessions 页，入口标签改为「项目与会话」；模块页内保留左栏结构，那是模块页自己的布局，不影响首页）；
  ③**构建修复**——`App.tsx` / `HomeChat.tsx` / `ProjectRail.tsx` 的 TS 错误已清零，未提交的 ProjectRail 修改已收敛。
  验收：真实窗口截图确认首页无项目树/无分栏、模块页可用；`npm run build` 0 错、`tsc` 0、Rust 144/144；
  诚实性标注（每轮新 DSH 会话 + 上文重放）保留在界面说明与 REVIEW.md 中。
- **背景：** OD-ARCH-031 Part A 通过（设置跳转 DSH 实测可用，含一个最小宿主命令 `task_open_dsh_settings` 的计划偏离
  ——已审查认可：无参数、固定白名单、不碰凭证与协议）。Part B 结构方向正确（项目-会话-对话流、三场景降为开场预设），
  但**摆放位置违反用户裁定**：项目/会话栏被放进了首页，形成用户不要的左右分栏。用户明确：首页就是全宽对话中枢，
  项目与会话集中到一个 Dock 模块页。
- **目标（三件事，一批完成）：**
  1. **首页还原定版**：移除首页中的 ProjectRail 与左右分栏，首页恢复 HomeCanvas 全宽对话版式；预设芯片（周报/文案/项目跟踪/普通对话）可留在首页对话区，项目树不进首页。
  2. **项目/会话模块页**：项目分组、会话列表、进行中的工作台对话、新建对话与预设选择，整体迁入一个 Dock 模块页（优先改造现有 sessions 页）。模块页内可保留左栏结构（那是模块页自己的布局，不影响首页）。
  3. **修复构建**：修掉 App.tsx / HomeChat.tsx / ProjectRail.tsx 的 4 个 TS 错误；未提交的 ProjectRail 修改一并完成或收敛。
- **影响范围：** desktop/src 前端（App/Workspace/ProjectRail/HomeChat/sessions 页），无宿主改动、无协议改动、无 runtime/ 改动。
- **验收标准：** 真实窗口截图确认首页无项目树、无分栏、版式与用户定版一致；Dock 模块页内项目/会话/新建对话可用；npm build 0 错误、tsc 0、Rust 测试全绿；诚实性标注（每轮新 DSH 会话 + 上文重放）保留在界面说明中。
- **风险与停止条件：** 仅前端布局与状态迁移；若迁移需要新增宿主命令或协议改动，停下报告。
- **完成后交 ARCHITECT 复核；Part B 的自动化缺口（真实窗口对话端到端）继续由用户人工点验覆盖。**

### OD-ARCH-031：模型设置跳转 DSH + 对话流工作区改造（Part A 已执行；Part B 的摆放位置经用户纠正后由 OD-ARCH-032 重做）

- **状态（2026-10-20，ARCHITECT 执行）：** Part A **已执行并实测**（提交 `32200a8`）——设置页常驻「打开 DSH 设置（登记 key）」与
  「刷新端点信息」；点击**真的把 DSH Desktop 拉起来**；未安装时明确报错；端点读不出时两个入口同样可见。
  宿主为此新增**一个**无参数命令 `task_open_dsh_settings`（固定白名单，不拼接命令、不碰凭证），
  属相对「宿主不新增命令」的**必要偏离**，已审查认可（见 REVIEW.md）。
  Part B 的**结构方向**正确（项目-会话-对话流、三场景降为开场预设），但**摆放位置违反用户裁定**：
  项目/会话树被放进了首页、首页变成左右分栏；用户明确首页不做分栏、不出目录树，
  DSH 的项目要收进**一个 Dock 模块页**。Part B 已按此重做（OD-ARCH-032，提交 `828c5a6`）。

- **背景：** 用户首次实测反馈两点：①设置页没有"刷新端点信息"按钮（根因：端点解析失败时整个开关区不渲染，只剩提示行；且把模型配置定位成"只能去 DSH 侧改的外部事实"，违背"在工作台里能用起来"的初衷）；②三场景的"表单+分步点选"向导式设计不符合真实用法——用户要的是 DSH/Codex 式对话流：项目 → 新会话 → 直接说事，AI 自主调 Skill 干活，材料随对话带上。
- **用户已定取舍：** 模型设置**跳转 DSH 原有设置界面**（不自建编辑器）；工作台本身做对话流改造。

#### Part A：模型设置跳转 DSH（小改动，先行）

1. 设置页"模型权限"区块改为引导 + 跳转：一个"打开 DSH 设置"按钮（复用系统打开方式启动 DSH Desktop 设置界面或其设置页；若 DSH Desktop 未运行则给出明确提示，不静默失败）。
2. 修掉"端点解析失败就不显示刷新按钮"的坑：刷新入口在解析失败时也必须可见可点，失败原因明示。
3. 保留现有权限三类开关与默认全关语义不变。
- **验收：** 真实窗口点跳转能到达 DSH 设置；返回 OmniDesk 点刷新，权限区在端点可识别/不可识别两种状态下都可用；关闭权限时提交仍被明确拒绝。

#### Part B：对话流工作区（主改动）

1. **界面结构改为「项目 → 会话」**：左侧变为项目列表（复用现有工作区/项目概念与 engine_list_sessions 的项目分组能力），项目内为会话列表；新建会话 = 直接进入对话。
2. **对话即工作**：会话内就是对话流——用户直接写要求、贴材料路径、传文件；模型自主推进，流式输出。不再要求用户走"表单 → 点按钮 → 逐步确认"的向导。
3. **三场景降级为开场预设**：新建会话时可选一个场景（周报/文案写作/项目跟踪）作为开场上下文——系统自动带入对应 Skill 提示、材料位置约定与写作要求，之后完全交给对话。不选场景就是普通对话。
4. **底层复用，不新增通信层**：会话创建/列表/恢复/流式输出全部走现有 engine 适配层与 DSH ACP 会话机制（这些接口已存在并有测试）；宿主不新增命令，前端不改协议。
5. **向导界面去留后置**：现有三场景向导页保留为可选入口（Dock 内折叠），等用户实际用一段时间确认对话流好用后，再由用户决定删除或保留。
- **验收：** 真实窗口内：新建项目 → 新建会话（选周报预设）→ 对话中给出合成材料路径与写作要求 → 模型流式回复且自动遵循预设的写作要求 → 会话关闭重开后可恢复继续对话；不选场景的普通会话同样可用；现有向导入口不受影响；Rust/tsc 测试通过；runtime/ 无差异。
- **影响范围：** desktop/src 前端结构（App/Dock/会话视图为主）、少量 api.ts 绑定、无宿主新命令、无协议改动、无 runtime/ 改动、不动凭证与路由。
- **风险：** ACP 流式输出在界面的呈现是本次主要技术风险——适配层已有 session/update 事件处理，但前端尚未以对话流形式渲染过，若事件粒度不适配，以最小改动在前端聚合，不倒逼协议扩面。若发现 DSH 会话能力无法支撑"AI 自主调 Skill"（例如 profile 无 Skill 加载机制），停下报告，不擅自改 DSH。
- **执行顺序：** Part A 与 Part B 可一批交给 BUILDER 连续完成（Part A 小，先做热身也先解决你设置端点的急迫需求），完成后 ARCHITECT 一次性复核。

> **当前状态更新（2026-10-19）：** OD-ARCH-025 的技术链路有证据，但产品验收不通过；OD-ARCH-026 的模型场景与 UI 实现已由 OpenCode 报告完成，功能部分仍待 ARCHITECT 独立复核。首页框架已由用户在本机确认并定为 UI 基线，详见 REVIEW.md 顶部与 GOALS.md。OD-ARCH-027 Step 2–4 已按范围复核通过，Step 5 待批准。Token 消耗审计阶段一完成；**OD-ARCH-028 阶段二试点已获用户批准（2026-10-19），已执行完成并通过 ARCHITECT 独立复核**（三场景真实窗口闭环 + 失败/取消负例，证据见 `evidence/od-arch-028/`）。**OD-ARCH-029（备份收口 + 双 Agent 复测 + 使用记录通道自检）已获用户批准并执行完成**：8 批提交已推送到 `origin/main`（远端与本地 HEAD 均为 `3f84ca5`）。除该次**一次性**备份授权外，仍不授权合并 worktree、提交、推送或打标签（见 docs/decisions.md D-022）。

- **状态：已执行（2026-10-20，ARCHITECT）。** Part A 完成并实测（设置页常驻「打开 DSH 设置（登记 key）」与
  「刷新端点信息」；点击**真的把 DSH Desktop 拉起来**；未安装时明确报错；端点读不出时两个入口同样可见）；
  为此新增**一个**无参数宿主命令 `task_open_dsh_settings`（固定白名单启动器，不拼接命令、不碰凭证），
  属相对「宿主不新增命令」的**必要偏离**，已标注待复核。Part B 完成：首页改为「项目 → 会话 → 对话」，
  三场景降级为开场预设（向导页保留在 Dock），新增 `conversationPrompt.ts` 纯函数（上下文累积 + 预设注入），
  脚本级验证 12/12。**未自动化：** 真实窗口的合成键盘注入进不了输入框，端到端打字链路需人工点一次；
  「同一 DSH 会话多轮」在当前协议下不存在（工作台每轮一个新会话 + 重放上文），要真多轮须给适配层加动词（另行批准）。
  证据见 `evidence/od-arch-031/`，结论见 REVIEW.md 同名小节。

### OD-ARCH-030：真实使用前置收口——端点引导 + 记录持久化 + 杂物清理 + 文档登记提交（已执行完毕，ARCHITECT 复核通过）

- **状态：已执行完成（2026-10-20，ARCHITECT 执行）。** 四件事：①**端点引导与验证通过**——设置页引导 8 项要点逐条核对为真（本机回环/非回环分类、默认全部关闭、无法确认位置永不放行、自行确认授权与数据去向、端点来自 DSH 侧、不读取凭证值等），并**修正一处会误导人的表述**（「在隔离环境中把模型配置指向明确的地址」→「先把 DSH 侧的模型配置指向一个明确的地址」）；假端点验证打通「端点判定 → 权限开关 → 配置落盘 → 提交放行 → 模型返回」，且权限关闭时提交被明确拒绝、**不产生任何任务**（不出错、不静默降级）。②**使用记录持久化到正式根通过**——落盘 `~/OmniDesk/app/logs/usage.jsonl`（仓库外，**产品代码无需改动**，此前落在 /tmp 只是测试覆盖了环境变量），内容只含枚举与时间，界面可读回，验证用的 2 条合成记录已用产品自带删除路径清空（文件 0 字节）；`quarantine/`（真实案件材料）与 `runtime/`（含凭证）经目录快照比对**未新增未改动**；保留期限与清理规则登记在 `docs/OmniDesk-Desktop-Data-Layout.md` §5.1。③**遗留 0 字节文件已删除**。④**登记文件与本轮改动已按红线扫描流程提交并推送**（提交号见 STATUS.md 同名小节与 `git log`）。遗留：真实端点与真实模型未验证；T-001 ④（双 agent 派工）因 OpenCode 子代理深度限制未通过，待用户决策（路径 A/B 已列在 `docs/agent-toolchain-retest.md`）。结论见 REVIEW.md 同名小节。
- **背景：** 用户下一步将进行首次真实使用并给出需求反馈；此前的使用记录自检写到了一次性 /tmp 隔离根，正式使用前须把数据落到 ~/OmniDesk 正式数据根；T-001 双 agent 因免费模型额度持续限流，暂不依赖其派工。
- **目标（四件事，一次做完）：**
  1. **模型端点设置引导与验证**：在设置页核实「成果目录与模型权限」的端点配置引导文案完整（本地/外部端点分别说明、权限开关默认关闭、开启需用户自行确认）；用一次合成材料 + 回环假端点验证配置写入与生效路径，不出错不静默降级。不改路由代码与凭证。
  2. **使用记录持久化到正式数据根**：把使用记录写入位置从临时隔离根确认/切换为 ~/OmniDesk 正式数据根的 logs/usage.jsonl（仓库外），验证写入、读回、格式仍为枚举+时间；保留期限与清理规则在文档中登记（暂不实现自动清理）。
  3. **遗留杂物清理**：删除工作区遗留 0 字节文件「当前没有正在执行中的」（用户批准本计划即视为授权）。
  4. **提交并推送**：把 029 复核后的 5 个登记文件（PLANS/REVIEW/STATUS/decisions/agent-toolchain-retest）与本轮新增改动，按既有红线扫描流程提交并推送。
- **影响范围：** 设置页文案与少量 Rust/前端验证逻辑、使用记录落盘路径、1 个杂物文件、状态文档、Git 提交推送。不改 runtime/、不新增通信层、不扩大模型路由功能面、不合并 omniharness-ui worktree。
- **验收标准：** 端点配置引导文案覆盖本地/外部与默认关闭语义；假端点验证通过；使用记录在 ~/OmniDesk 正式根写入读回成功且无正文；杂物文件已删除；红线扫描通过后推送，本地与远端 HEAD 一致；工作区仅剩明确说明的未跟踪项。
- **风险与停止条件：** 涉及真实数据根写入，须验证不触碰既有 app/quarantine/runtime 子目录内容，发现冲突即停；端点配置若牵动 IPC 契约形状则停下报告，不擅自扩面。
- **复核结论（2026-10-20，REVIEW.md 顶部）：通过。** 四项全部完成并入库推送（HEAD 581c964，远端一致）。附带收益：T-001 双 agent 四项复测全部闭环（subagent_depth: 2 派工链路调通）。真实服务商端点与真实模型质量未验证，属用户首次真实使用范围。**当前无待批准计划：等待用户首次真实使用反馈**（docs/pilot-first-real-use.md），下一轮需求以体验为输入制定。

### OD-ARCH-029：备份收口 + 双 Agent 复测 + 使用记录通道自检（已执行完毕，ARCHITECT 复核通过，见 REVIEW.md 顶部）

- **状态：已执行完成（2026-10-20，ARCHITECT 执行）。** 三件事：①**备份收口完成**——提交前对 681 个待提交文件做敏感信息扫描（命中 28 条全部为无害：红线词表本身、合成测试号码、函数调用、文档说明），分 8 批提交（`2705148` 前端 / `90e2731` 宿主 / `1be6992` 引擎 / `b5e7a1e` 样例 / `bfff57f` 脚本 / `593c49f` 双 agent / `2b4be2d` 文档与审计 / `3f84ca5` 验收装置与证据），并已推送 `origin/main`（`5ffc710..3f84ca5`），**远端 refs/heads/main 与本地 HEAD 一致**；提交前 682 项与提交后 681 已提交 + 1 剩余未跟踪逐项对齐，**无丢失**。②**T-001 三项复测未能在本会话执行**（subagent 仍返回 `Unknown agent: super-plan`；两个 agent 文件创建时间晚于本会话的 agent 目录），按风险条款延后，清单见 docs/agent-toolchain-retest.md，**新开会话即可跑完**。③**使用记录通道自检通过**：记录数 0→1，落盘在仓库外数据根，内容只有枚举与时间（无输入/输出/正文/路径），界面「查看记录（1）」可读回。剩余未跟踪项仅 1 个 2026-10-16 遗留的 0 字节空文件 `当前没有正在执行中的`（未提交、未删除）。结论见 REVIEW.md 同名小节。
- **背景：** OD-ARCH-028 三场景真实窗口闭环已复核通过；项目未偏离。当前最大风险是工作区 115 项改动长期未提交（单点丢失）；T-001 三项复测被主动挂起（清单在 docs/agent-toolchain-retest.md）；用户首次真实使用尚未开始。
- **目标（三件事，一次做完）：**
  1. **分批提交并推送备份（最优先）**：把工作区改动按 evidence/docs/desktop/tests 等逻辑分组，先对每批做敏感词+凭证启发式扫描并人工核对文件清单，再分批提交到 main 并推送 github.com/klzone/omniharness。会话日志、真实材料、凭证、仓库外数据一律不提交。每批记录提交号；完成后 git status 仅剩余明确说明的未跟踪项。
  2. **T-001 三项复测**：按 docs/agent-toolchain-retest.md 清单，在新会话中执行三项只读复测（git rev-parse 放行、管道命令匹配语义、双 agent 实际模型标识），结果原样登记 STATUS.md。
  3. **使用记录通道自检**：用合成数据点一次「使用记录（可选）」，验证记录写入仓库外数据根、只含枚举与粗耗时、不含输入输出正文，并能读回显示；结果登记。
- **影响范围：** 仅 Git 提交/推送、docs 证据登记、一次新会话只读复测、一次使用记录写入读回。不修改业务代码、runtime/、模型路由、凭证、用户数据；不合并 omniharness-ui worktree；不扩大授权范围。
- **验收标准：** 远端与本地 HEAD 一致且推送成功；红线扫描记录在案；三项复测有真实运行输出（失败原样记录）；使用记录写入/读回成功且无正文落盘；既有未提交改动在提交前后不丢失不覆盖。
- **风险与停止条件：** 发现疑似真实材料或凭证内容 → 停止该批提交并报告；双 agent 限流 → 复测延后不阻塞备份；远端认证失败 → 停止并报告，不重试绕过。
- **后续（不在本计划内）：** 复核通过后，由用户按 docs/pilot-first-real-use.md 完成首次真实使用并记录体验数据；UI 微调与真实模型端点启用另行单独批准。

### 工具链计划 T-001：OmniDesk 开发固定双 Agent「超级Plan / 超级劳工」（已部署；运行复测待补）

- **状态：用户已批准并已部署完成（2026-10-19），文件已入库（提交 `593c49f`）。** 复测**2026-10-20 晚全部闭环**：① `git rev-parse` 放行**通过**；② 管道/复合命令语义**已明确——被拒（白名单仅按单条命令前缀匹配）**；③ `git diff --check` **通过**；④ 派工链路**通过**（用户批准 B 方案：新增 `.opencode/opencode.json` 设 `experimental.subagent_depth: 2`，同会话即时生效，ARCHITECT→super-plan→super-worker 端到端调通）。结果表见 [docs/agent-toolchain-retest.md](docs/agent-toolchain-retest.md)。既有 111 项差异已由 OD-ARCH-029/030 会话提交入库；本计划不接触业务文件。
- **分类：功能实现（OpenCode 项目级协作配置，非 OmniDesk 产品代码）。** 调研结论：工作区尚无 `.opencode/` 目录；OpenCode V2 支持 `.opencode/agents/<id>.md` 项目级 agent，frontmatter 声明 `description / mode / model / permissions`，正文为 system prompt；权限规则 `action/resource/effect`，后匹配者优先（宽规则在前、例外在后）。
- **目标：** 创建两个固定 agent，均固定模型（用户 2026-10-19 追加指定）：超级Plan = `opencode/glm-5.3-flash-free`；超级劳工 = `opencode/deepseek-v4.1-flash-free`（均已核实为 active 引用）：
  1. **超级Plan**（`.opencode/agents/super-plan.md`，mode: all，可作主 agent 也可被子 agent 调用）：专职 OmniDesk 规划与设计。system prompt 内嵌协作纪律（先读 Git 状态与代码、分类需求、产出计划、证据区分、不伪造运行结论、越界/高风险即停、不读凭证）。权限矩阵：
     - `* / * → deny`（默认全拒），随后放行：`read / grep / glob → allow`；`edit → PLANS.md allow`（其余路径仍 deny）；`shell → allow` 仅限只读命令 `git status* / git log* / git diff* / git show*`；`subagent → 超级劳工（ID: super-worker）allow`；`read → deny` 凭证类文件（`**/.env*`、`**/*credentials*`、`**/*secret*`、`**/*.pem`、`**/*.key`）。`webfetch / websearch / web`、其余 edit/shell、其他 subagent 皆拒绝，如需另批。
  2. **超级劳工**（`.opencode/agents/super-worker.md`，mode: all）：专职在获批计划范围内执行。system prompt 内嵌：只做获批范围、开工前记录 git status/HEAD、保留全部既有未提交/未跟踪/暂存改动、执行记录写 STATUS.md、区分证据类型、计划外高风险差异停手报告、不提交/推送/打标签、不改 runtime/ 与模型路由凭证、不接触真实业务材料与用户数据。权限不加额外限制（等同 build 默认：工具默认可用，敏感路径如环境文件、工作区外访问弹确认）。
- **交接机制：** 超级Plan 调研后把计划写入 PLANS.md（它唯一可写的文件）→ 用户明确批准 → 超级Plan 用 subagent 调用「超级劳工」执行 → 完成回报 → ARCHITECT（本会话/REVIEW.md 流程）独立复核。不设 `default_agent`（不改变现在的默认会话主 agent），不改 `opencode.json`、模型路由与凭证。
- **影响范围：** 仅新增 2 个文件 + 本次 PLANS.md 登记本身。明确不做：不设 default_agent、不改 provider/model 配置、不提交/推送/打标签、不动 runtime/ 与既有工作区差异。
- **验收方式与标准：** (1) 新会话/子 agent 列表能发现两个 agent；(2) 超级Plan 实测只能读代码库、写 PLANS.md、跑只读 git、只能调 超级劳工；(3) 超级劳工可编辑与执行 shell；(4) 两者的实际模型为 `opencode/glm-5.3-flash-free`。可在批准后用一次真实小任务（如某个只读调研 + 一小步执行）端到端试用并记录证据。
- **风险与回退：** agent ID 兼容性风险已通过用户选择英文文件名规避；如实测仍发现 ID/资源匹配问题，向用户报告并按实际表现修清单经批准再改。权限规则是配置级防线，不是代码级安全控制；两个 agent 的 prompt 均写入「不读取/打印凭证、不扩大用户答复适用范围」的纪律。本计划全程不提交、不推送。
- **交付：** 两个 agent 文件、本计划状态更新、（如用户同意）一次端到端试用记录；最终交 ARCHITECT 复核。
- **执行结果（2026-10-19 → 2026-10-20 补记）：** 部署与静态校验已完成（见下）。**三项运行复测（`git rev-parse*` 放行、管道/复合命令匹配语义、super-plan→super-worker 派工链路）仍未执行**：2026-10-20 实测确认两个 agent 文件的目录正确（OpenCode V2 文档：项目级 agent 放 `.opencode/agents/<name>.md`），但**当前会话的 agent 目录是在文件创建之前建立的**，subagent 工具返回 `Unknown agent: super-plan`，故无法在本会话内复测。复测步骤已写成可直接照做的清单：[docs/agent-toolchain-retest.md](docs/agent-toolchain-retest.md)；**新开会话即可发现这两个 agent**，届时三分钟可跑完。

- **执行结果（2026-10-19，ARCHITECT 本会话直接实施，非 BUILDER 派工）：** 新增 `.opencode/agents/super-plan.md`、`.opencode/agents/super-worker.md`（未设 `default_agent`、未改 `opencode.json`、未提交推送）。模型按用户指定固定：super-plan = `opencode/glm-5.3-flash-free`，super-worker = `opencode/deepseek-v4.1-flash-free`（均已核实 active）。**静态校验：** 两文件 frontmatter 用 YAML 解析通过，键为 description/mode/model/permissions，super-plan 16 条权限规则按「先宽后窄」顺序正确落地（`*/*` deny → read/grep/glob allow → edit 仅 PLANS.md → 5 条只读 git allow → subagent 仅 super-worker → 5 条凭证类 read deny），super-worker 无 permissions 段（= build 默认）。**运行实测：** 两个 agent 均可被 subagent 工具发现并加载；super-plan 的 read/glob/grep 可用、单条 `git status --porcelain` 可执行、`git rev-parse HEAD` 与 `find` 等未放行命令被正确拒绝、未越界写任何文件；super-worker 可加载并自述模型为 DeepSeek V4.1 Flash Free。**挂起项（用户指示暂缓，非失败）：** 补入 `git rev-parse*` 后的复测、含管道/复合命令的匹配语义、super-plan→super-worker 派工端到端链路——待模型限流解除后按 REVIEW.md「下一步」补做。

### OD-ARCH-028：阶段二试点——定版首页接入工作模块，以周报完成首个体验闭环

- **状态：已执行完成，并通过 ARCHITECT 独立复核（2026-10-19）。** 前置门（OD-ARCH-026 独立复核）通过后继续；三场景真实窗口闭环成立，失败/取消不产生成果文件，回归全绿。证据：`tests/verification/desktop/evidence/od-arch-028/`（脚本 + JSON + 29 张截图 + README）；结论见 REVIEW.md 同名小节。遗留：真实模型质量、真实 Wayland 桌面、真实使用体验指标（澄清/返工）与 Token（引擎未上报 usage，记未知）未验证。本计划承接 Token 审计阶段二，采用集中实施、集中验收。用户已说明 OpenCode 完成首页 UI 初稿并将其定版；这确认产品方向，不代表批准整体合并另一 worktree。开工快照：主工作区 HEAD 5ffc710，工作区 111 项差异（含既有开发改动和 Token 审计文件，必须保留）；候选 worktree /mnt/data/Code/omniharness-ui 有独立未提交差异，文件逐项核对。
- **目标：** 把已确认首页作为稳定外壳，接入周报与随记、文案写作、项目跟踪入口；以周报完成首个体验闭环：用户提供公开/合成材料，模型整理起草，用户修改确认并保存。其余两个模块接通现有模型工作流并回归，不扩展专业规则或新架构。数据分析和模型调用 Python 暂缓。
- **前置门：** ARCHITECT 先根据 OD-ARCH-026 实际源码、差异与测试作独立复核。若材料边界、模型结果真实性、权限或保存条件不通过，停止本计划功能实施，先提交有边界的修复计划。
- **影响范围：** 通过前置审查后，按需选择性移植/修改主工作区 desktop/src/App.tsx、desktop/src/shell/**、三场景现有 UI 与流程接线、必要前端依赖，以及本计划证据和状态文档。先只读对比候选 worktree；不整体复制/合并，不覆盖既有改动。不得改 runtime/、模型路由/凭证、通信协议、真实用户数据或旧数据目录；不提交、推送、打标签。
- **实施步骤：** (1) 独立审查 OD-ARCH-026，只有通过后继续。(2) 对照用户现场确认的首页与候选交接说明，列出逐文件移植清单；以本工作区已确认 UI 基线为准，不恢复被后续反馈取代的右栏、假数据、持续动画或过时状态提示。(3) 接通左侧三个入口，复用现有任务 API、取消和状态生命周期；仅修复可观察到的不一致和使用阻断，不做全面视觉重设计。(4) 用合成或公开材料在真实桌面窗口完成周报闭环：选材、模型起草、编辑、人工确认、仓库外保存并读回；验证失败与取消不伪装成功。(5) 文案与项目模块覆盖材料传递、模型起草、编辑/保存的定向回归。(6) 集中执行计划相关前端构建、Rust/宿主和引擎回归、桌面成功/失败/取消、隔离与 runtime 无差异检查、真实窗口逐模块截图；更新状态和证据后一次性 Review。
- **验收标准：** 首页保持定版布局和交互；三个模块都可从左侧进入；周报完成真实桌面合成/公开材料下的模型起草、人工编辑确认及仓库外保存闭环；文案和项目通过材料到模型、结果呈现及保存回归；失败/取消不产生成功状态或伪造正文；结果标注未复核；必要回归通过，所有既有工作区差异完整保留。
- **试点评价：** 记录执行请求数、人工澄清数、返工轮数、耗时和测试结果。只有遥测能可靠映射到本计划线程时才记录输入/输出/缓存 Token，否则标未知、不估算。与历史任务比较时说明范围差异，不从单次试点推导普遍比例。
- **风险与停止条件：** 026 审查不通过、两个 worktree 的重叠无法安全分离、候选设计与已确认基线冲突，或合成模型链路不能稳定验证时，停止相关实施并报告。若场景页面已满足目标，只做必要入口接线。真实业务材料、真实端点授权和同事试用不在本计划范围。
- **交付：** 精简执行摘要和证据路径，包含 UI 截图、闭环结果、命令与通过数、Token 可测性、遗留问题及 Git 状态；完成后停止并等待 ARCHITECT 独立验收。

### OD-ARCH-026：AI 对话为中枢的产品校正与三项工作场景闭环

- **状态：用户已批准；BUILDER（OpenCode）已执行完成，待 ARCHITECT 独立复核。** 产品方向校正与桌面功能计划。OD-ARCH-025 的执行证据和产品缺口见 REVIEW.md、STATUS.md 与 [证据索引](tests/verification/desktop/evidence/od-arch-025/README.md)；本轮执行记录与证据见 [OD-ARCH-026 证据索引](tests/verification/desktop/evidence/od-arch-026/README.md)。
- **执行结论（执行方记录，非审查结论）：** 首页已改为对话中枢，模型权限由宿主回报并在提交时强制执行（关闭时提交关闭并给出设置入口）；Dock 新增周报与随记、文案写作、项目跟踪三个入口；三个场景改为模型起草——提示词由用户输入、**用户明确选择的材料文件**正文与可编辑流程包规则组成，确定性核对单独成栏且不作为成果，成果仅来自模型初稿并经人工确认后保存；流程包新增可选可编辑规则 `draft.rules/sections`；宿主新增只读单文件材料读取 `task_read_material`。前端 `tsc`/构建通过，Rust 当前源码全测 142/142，`verify_ai_draft` 27/0，新增 `verify_scenario_materials` 5 场景 0 failed，桌面链路/工作台引擎路径/取消/会话回归全部通过；一次性真实窗口完成周报场景「选择材料 → 模型初稿 → 人工确认 → 保存」闭环（产物含模型正文与材料来源 sha256）。未验证项见证据索引 §3。
- **目标：** 让 OmniDesk 成为以大模型对话和执行指令为中枢的定制 Agent，而非文件管理器或本地汇总器。首页约半屏为主要对话区并可直接启动工作；左侧悬浮图标进入周报与随记、文案写作、项目跟踪。模型基于用户输入和用户明确选择的文档/表格材料，完成整理、提炼、归纳与起草；代码负责权限、任务状态、文件读写、来源记录及确定性校验。周报、文案、项目管理共享交互骨架，各自采用可编辑流程规则。数据分析最后建设，后续另行规划由模型按任务调用预制 Python 处理表格、文档或数据库；本计划不实现该能力。
- **影响范围：** 主工作区现有 React/Tauri/Rust/Python 引擎连接、三类工作场景、可编辑 Skill/任务配置与合成回归；将 `/mnt/data/Code/omniharness-ui` 作为 UI v2 设计和候选前端来源，只读审查后选择性移植，不整体合并其 worktree。不得修改 `runtime/`，不得读取/打印凭证、改变模型路由、接触真实业务材料或用户数据；保留两边全部既有差异，不提交、推送、打标签或合并 worktree。
- **实施步骤：** (1) 记录两个 worktree 的 HEAD、状态和改动边界；只读对照 UI v2 首页对话、Dock 与主工作区 UI/API/权限代码，选择性移植，不整体合并。 (2) 校准 GOALS.md 和 docs/decisions.md：大模型对话是首页与任务执行中枢；首次模型权限严格关闭，用户可按自身授权在设置中开放；不得把有限单位答复扩大到周报、写作、项目跟踪或具体端点。 (3) 接入首页主要对话区及左侧悬浮图标导航；首页对话和各工作场景复用现有引擎 API、取消与任务状态，不另造通信层。 (4) 让周报、文案写作、项目跟踪的模型任务都真实经过当前适配链路：模型读取用户输入及用户明确选择的文档/表格内容，按可编辑、带版本的 Skill/场景配置整理提炼并起草；结果回到界面供用户修改、确认和保存。代码只做日期、状态、数值等确定性工作和文件边界控制，不允许本地模板拼接伪装模型产出。表格/文档解析采用现有可用能力；不得借机开放任意目录扫描或未授权数据库访问。 (5) 为三个场景各设合成模型回归，证明所选材料进入模型任务、模型结果被展示并可保存；同时覆盖失败与取消。真实工作数据/真实模型不作为本计划测试条件。 (6) 在隔离数据根的真实桌面窗口验证对话首页、三类场景、文件选择、保存及重开。运行当前源码 Rust 全测、前端类型检查与构建、桌面 success/failure/cancel、准入/会话/草稿回归、`git diff --check HEAD` 和 `runtime/` 无差异检查；更新 STATUS.md 与证据索引，完成后停止等待 ARCHITECT Review。复杂数据分析、AI 调用预制 Python 对表格/文档/数据库处理，及一般数据库接入均不在本计划实施范围，另行规划。
- **验收标准：** 首页打开即有约半屏、可直接执行任务的主要对话区；左侧悬浮图标能进入周报与随记、文案写作、项目跟踪。三个场景均以真实合成模型调用作为内容处理步骤，所选文档/表格材料参与模型输入，结果可见、可编辑、人工确认并保存；Skill/流程规则可在不改代码时调整。用户设置控制模型权限且 UI 与宿主一致；无模型输出时明确失败/阻塞，不能展示本地汇总冒充 AI 结果。计划要求的测试通过，所有现存工作区改动保留，`runtime/` 无变化。
- **风险与停止条件：** 两个 worktree 均有大量差异，不能整体复制或合并；文件归属或重叠不清时停止移植并报告。不要读取凭证或推定具体模型、服务商、设备和真实工作用途获准；无安全的合成模型链路时停止 UI 提交测试并报告缺口，不以绕过准入或伪造模型回复代替。文件解析仅限用户明确选择的文件；本计划不允许任意扫描目录或接入数据库。若现有适配层不支持需求，先说明契约缺口，不改 Runtime、不自造通信协议。
- **交付：** 集中完成 UI 校正与三类 AI 场景闭环，提供变更文件、真实窗口证据和测试结果；不提交、不推送、不合并 worktree。独立 Review 通过后，由用户做实际工作体验反馈。

#### 首页版式回归：对话为主 + 顶部三模块（用户已批准，2026-10-19；**仍属 OD-ARCH-026，不另开新编号**）

- **状态：已由 ARCHITECT 复核按获批口径通过（2026-10-19）；仍属 OD-ARCH-026。** 按下列口径落地：新增 `HomeCanvas.tsx`、`App.tsx` 首页改渲染、删除 `HomeDashboard.tsx`、`HomeChat.tsx` 仅改展示；`tsc`/`npm run build` 通过；真实窗口（一次性 Xvfb + 隔离数据根 + 回环合成端点）四页截图、首页 5 帧签名=1、空闲 60 s `WebKitWebProcess` 0.08%；工作区条目仍 108。执行记录见 `STATUS.md` 同名小节，证据见 `evidence/od-arch-026/README.md` §4 与 `home-layout/env.json`。未验证项：真实模型质量、Wayland/release、窗口缩放、输入法/粘贴。

- **授权与归属：** 用户确认按 `_design/stitch_omnidesk` 的原始版式恢复首页，并明确「继续 026」；本项是 OD-ARCH-026 的产品校正延续，不登记为 OD-ARCH-028。执行完成后写入 STATUS.md 与证据索引。
- **目标版式：** 删除右栏整列（周报/文案/项目三张场景卡、当前版本边界、查看引擎状态、「OMNIDESK · LOCAL FIRST」）；删除「对话中枢」容器头、说明文字与三个快捷入口；顶部一行 = 机器人（`/mascot.png`）+ 今日提醒（今日重点推进）+ 日程节律（三周日历）；下半屏整幅 = 对话消息流 + 输入框，消息流顶部向上渐隐。
- **用户已确认的三项口径：** (1) **默认静态**：不使用 `animate-float-mascot`、`animate-pulse`、卡片 `backdrop-blur-md`，保持 OD-ARCH-027 Step 2/4 的成果；要动效须另行批准。(2) **不伪造数据**：今日提醒沿用诚实空态「未接入数据源」，日程只给三周日期骨架，不写演示性待办/统计数字。(3) **权限提示压缩**：模型权限信息收为输入框底部一行状态（未开启时含「去设置」），保留宿主事实，不保留整块说明。
- **影响范围：** 新增 `desktop/src/shell/HomeCanvas.tsx`（从只读 worktree 选择性移植并适配本树）；`desktop/src/App.tsx` 首页路由；删除 `desktop/src/shell/HomeDashboard.tsx`；`desktop/src/shell/HomeChat.tsx` 空态与权限提示。复用 `ThreeWeekCalendar.tsx`、`public/mascot.png`。不改 Rust、`runtime/`、模型路由、凭证、用户数据、第二 worktree；不提交、不推送、不打标签。
- **验收：** 首页无右栏与蓝框内容；顶部三模块 + 下半屏全宽对话成立；消息流顶部渐隐；默认无持续动画；`tsc`/`npm run build` 通过；真实窗口逐页截图无回退；未验证项与既有改动保留情况如实记录。

#### 窗口外框一体化（frameless，用户已批准并验收，2026-10-19）

- **授权：** 用户要求把系统标题栏（最小化/最大化/关闭）融入软件界面，类似无边框应用。本项只改窗口外框与顶栏控件，不动页面内容与业务逻辑。
- **改动：** `src-tauri/tauri.conf.json` 主窗口 `decorations: false`；`src-tauri/capabilities/default.json` 增加窗口控件所需最小权限（minimize / toggle-maximize / close / is-maximized / start-dragging / start-resize-dragging / internal-toggle-maximize）；新增 `src/shell/WindowChrome.tsx`（自绘 — □ × 与 8 个边缘/角落缩放热区，只调用真实 Tauri 窗口 API）；`TopNav.tsx` 顶栏加 `data-tauri-drag-region` 并嵌入控件；`App.tsx` 挂载缩放热区。
- **验收：** 无系统标题栏；顶栏可拖动、双击最大化；— □ × 真实生效；无边框后仍可从四边/四角缩放；最大化时缩放热区隐藏；不遮挡现有顶栏交互。
- **风险与回退：** 无边框在 Wayland/不同 WM 下的拖动与缩放存在平台差异；若窗口无法移动或缩放，回退为 `decorations: true` 并移除顶栏控件后重建。需要重新 `cargo build` 并重启应用（仅前端 HMR 不生效）。
- **状态：已完成并经用户现场验收（2026-10-19）。** `tsc`/`npm run build`/`cargo build` 通过，Xvfb 冒烟确认控件渲染与页面完整；用户重启后确认系统标题栏消失、拖动、最小化/最大化/关闭均正常。该外框与当前首页共同作为**已确认 UI 基线**（见 GOALS.md、docs/decisions.md D-020）。

### OD-ARCH-027：桌面端 WebKit 渲染性能治理（OD-ARCH-026 支线）

- **状态：计划已建立（Step 0 证据归档与 Step 1 计划完成）；Step 2 已由 ARCHITECT 独立复核按范围通过（附关键限定，见 REVIEW.md 顶部：原始空闲 ~100% 症状未复现，本步无可宣称的应用级收益）；Step 3 已由 ARCHITECT 复核按范围通过（零产品代码改动，诚实空结果见 REVIEW.md）；Step 4 已由 ARCHITECT 复核按范围通过（含最终「状态点默认静态」改动，满足 §5 的 <10% 目标）；Step 5（真实桌面回归）已于 2026-10-20 在用户自己的 Wayland 会话执行完成：原生 Wayland × DMABUF{1,0} 与 XWayland × DMABUF{1,0} 四组分别记录，空闲 60 s 的 `WebKitWebProcess` 均值 0.08% / 0.17%（原生 Wayland）、0.15% / 0.28%（XWayland），导航 P50 0.31–1.22 s、P95 ≤1.63 s、0 超时，5 帧签名 = 1，设置页连续输入逐字落盘（exact_hit）。**原始 ~100% 症状在真实桌面同样未复现，不得表述为性能提升。** 完整计划见 [docs/OmniDesk-Desktop-Render-Performance-Plan.md](docs/OmniDesk-Desktop-Render-Performance-Plan.md)，证据索引见 [evidence/od-arch-027](tests/verification/desktop/evidence/od-arch-027/README.md)（Step 2 见 §6、Step 3 见 §7、Step 4 见 §8、Step 5 见 §9）。未提交、未推送、未打标签，本支线不与 OD-ARCH-026 的功能开发或复核混同。
- **Step 4 执行结论（执行方记录，非审查结论）：** 单因素对照（6 个同构页 ×3 次 ×30 s）显示：静态模糊/玻璃 **0.00%**、纯动画 **26.00%**、动画+模糊 **98.48%**、**「静态模糊 + 唯一一个 6×6 脉冲点」= 100.19%** —— 成本来自「持续重绘 × 大面积模糊」，与动画元素大小无关，这正是原「~100%」的成因。当前 JSX 已不再使用 `.animate-pulse-ambient`/`.animate-float-mascot`，但**顶栏状态点在 `engineHealthy` 时曾用 `animate-pulse`**（顶栏保留着 Step 2 的 `backdrop-blur-2xl`）：预置权限使状态点脉冲后，应用空闲 `WebKitWebProcess` **18.82%**、UI 进程 10.26%（合计约 29%，**不满足 §5 的 <10%**）；加 `prefers-reduced-motion` 后 **0.10% / 0.13%**。**最终改动：** `.status-dot-pulse` 默认 `animation: none`（状态点默认静态、颜色语义与位置/尺寸/布局/文案不变、键帧保留、一行可回退），并保留 reduced-motion 守卫；`TopNav.tsx` 抽出 `NavClock` 使 30 s tick 只重渲染时钟（本轮未再改 TopNav）。**最终复测（60.07 s）：** 引擎健康 **0.10%（UI 0.12%）**（曲线 0.60→…→0.10，5 帧签名 1/1）、出厂默认 **0.03%（UI 0.10%）**，网络 0.00/0.02%、均无 GPU 进程 —— **满足 <10% 目标**。`SettingsPanel` 丢字**未复现**，未改。`tsc`/build/4 页冒烟/输入均通过；导航 P50/P95 引擎健康 2.548/2.597 s、默认 2.567/2.593 s，但该指标被整窗截图读回（约 2.5 s）饱和、不具亚秒分辨力，**不据此下性能结论**；未见视觉回退。未验证项见证据索引 §8.6。
- **Step 3 执行结论（执行方记录，非审查结论）：** `WEBKIT_DISABLE_DMABUF_RENDERER` = `1`（现状默认）vs `0`，一次性 Xvfb 上各 3 次：页面级 `heavy.html` 100.17/100.15/100.18% vs 100.22/100.26/98.76%（`simple.html` 两档≈0%）；应用级空闲 `WebKitWebProcess` 0.07/0.07/0.08% vs 0.07/0.08/0.08%，导航 P50/P95 与设置页输入（6/6 无丢字）一致；**两档都无 WebKitGPUProcess，82 张截图 0 异常帧**。环境探针显示该变量在本沙箱**不可控**（`unset`/`0`/`1` 均 ~100%；`libEGL: DRI3 error`，EGL/加速渲染不可用），只有 `WEBKIT_FORCE_DMABUF_RENDERER=1` 改变代价（62.07%）——因此 A/B 的「无差异」是构造性空结果，**不能读成「DMABUF 在真实桌面不重要」，也不得仅凭 CPU 下降判定为改进**。**Wayland↔XWayland 对照未做**（沙箱无 GPU，且不得由 BUILDER 连接用户真实 Wayland 会话），留待 Step 5。未验证项见证据索引 §7.8。
- **Step 2 执行结论（执行方记录，非审查结论）：** 全屏光斑由 `filter: blur(120–140px)` 改为静态径向渐变光晕；删除 `plate-sculpted`、`.wb-v2 .pane`、`.ofl-v2 .pane` 与首页三处卡片的 `backdrop-filter`；只保留顶栏 `backdrop-blur-2xl` 一个关键视觉层。实测（debug + vite dev，Xvfb，DMABUF=1，60 s）`WebKitWebProcess` 0.33% → 0.07%，导航延迟 P50/P95 1.821/3.350 s → 1.941/2.577 s（含截图读回开销）。**重要限定：当前工作区首页空闲 CPU 本来就远低于 10% 目标，计划 §1.2 的「约 100%」未复现，不宣称性能提升**；MiniBrowser 单因素对照显示模糊是「有无限动画页面」的成本放大器（100.2% → 19.6%）。逐页前后截图无视觉回退。未验证项见证据索引 §6.4。
- **目标：** 消除空闲时渲染进程持续占用一个 CPU 核；交互延迟相对基线明显下降；连续输入不丢字；Dock/顶栏/卡片层级与主要渐变保留。**不**全面删除玻璃特效，**不**顺带重构 UI 架构。
- **影响范围：** 仅桌面样式的渲染成本、渲染后端配置对照、无限动画与前端重渲染治理，以及配套测量脚本与证据。不得修改 `runtime/`、模型路由、凭证、真实材料、用户数据、第二 worktree；不得提交、推送、打标签或合并。
- **执行顺序与门禁：** Step 0 证据归档（已完成）→ Step 1 计划（已完成）→ **Step 2 CSS 性能优化**（仅样式与既有类，可独立回退）→ Step 3 DMABUF/显示后端独立对照（零代码改动，黑屏即回退）→ Step 4 无限动画与 React 重渲染治理（已执行完成，待复核）→ Step 5 真实 Wayland 桌面回归。**严格分阶段验收，禁止一次性改完。**
- **验收阈值（首轮目标，非保证）：** 空闲 `WebKitWebProcess` CPU 稳定后 60 s 平均 <10%；导航延迟相对基线明显下降且记录 P50/P95；固定测试串连续输入无丢字；视觉效果保留；DMABUF 启用/禁用分别记录并可回退；Xvfb 与真实桌面分别记录、不混为一组。CPU 下降本身不构成成功判据。
- **结论纪律：** CSS 特效与 DMABUF 是两个独立变量，必须分别对照测量；不得因相关性把全部成本归给其中一方；若真实桌面仍高于目标，继续采样定位剩余原因，不得改口径或结案。
- **交付：** 分步改动（每步可独立回退）、落盘的测量数据与前后截图、性能报告、回退记录、证据索引与状态更新；不提交、不推送。

### OD-ARCH-025：个人试用前桌面闭环验收（已执行；产品验收不通过）

- **状态：** 底层技术与窗口自动化证据有记录；但写作任务实为合成数据解释、项目流程为本地确定性汇总，首页与用户要求不符。未验证目标产品行为，不作为实际应用试用的通过依据。详情见 REVIEW.md 顶部与证据索引。
- **执行结论（执行方记录，非审查结论）：** 四阶段全部 pass——(A) 真实窗口内完成 AI 辅助写作任务与项目事项流程，两份成果均落在**用户设置的**成果目录并保存；(B) 关闭重启后设置/权限/任务台账/成果与重开都保持；(C) 可控慢关闭下真实关闭窗口，进程 0.25 s 自行退出、无孤儿进程、被打断任务未伪造成功终态；(D) 当前源码重建的 `.deb` 解包后启动入口可拉起真实窗口且沿用同一数据根。Rust 离线全测 134/134、前端构建与类型检查、桌面任务/取消/会话/草稿回归、`git diff --check HEAD`、`runtime/` 无差异均通过。本轮未改动任何 Rust 或前端产品源码。未验证项与边界见证据索引 §1.2/§6。

### OD-ARCH-024：修复生命周期自锁并补齐宿主并发验证

- **状态：用户已批准；BUILDER 四轮执行完成，ARCHITECT 按获批范围通过。** 完成生命周期自锁和关闭期间并发边界修复，并统一窗口销毁关闭入口；证据、限制与独立复核见 REVIEW.md、STATUS.md 与 [证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)。本计划不授权提交或推送。
- **目标：** 让所有实际触达引擎的宿主操作在权限变更和引擎停止期间遵守明确、可测试的并发规则；确认引擎调用包装器不会因重入生命周期锁而卡死。
- **影响范围：** 仅现有 Tauri Rust 宿主、EngineLifecycle、准入/bridge 接线测试、OD-ARCH-023/024 测试证据和共享状态文件。不得修改 runtime/、模型路由、凭证、前端业务范围、用户数据、真实材料或第二 worktree；不得提交、推送或打标签。
- **实施步骤：** (1) 用最小改法消除持有 CallGuard 时再次锁同一 mutex 的路径；世代校验应使用守卫已持有的世代或等价的不可重入校验，不得删除权限世代一致性保障来“修复”卡死。 (2) 逐项盘点 engine_task_status、engine_task_result、engine_cancel_task 与其他已注册引擎命令；为每个命令记录权限变更/关闭期间的预期行为，特别区分取消操作是否必须继续可用，并避免权限变更等待任务结束时把取消永久堵住。 (3) 用真实宿主包装器配合假 bridge/假引擎添加回归测试：至少验证 with_engine_call 和 with_call_gate 正常返回、不自锁；覆盖描述/提交的准入到动作闭环、权限撤销与调用的互斥、停止失败持续拒绝和恢复后放行；验证状态/结果/取消命令按定义的生命周期语义工作。测试须使用确定性同步，不以源码字符串检查或纯生命周期单元测试替代组合路径。 (4) 修正 OD-ARCH-023 的执行报告中可能过度表述的覆盖结论，并建立单一证据索引；如发现测试只是复用旧预编译二进制，应标明其不能代表新源码。 (5) 在不安装系统依赖、不提权的条件下，运行 Rust 完整离线测试及相关宿主集成测试、前端构建、路由/桌面/取消/草稿/会话连续性回归、git diff --check HEAD 和 runtime/ 无差异检查；记录环境限制、实际测试二进制是否由当前源码编译、精确通过数。
- **验收标准：** 两个包装器的组合路径无自锁；权限世代校验仍有效；权限变更完成后旧调用不能启动/使用引擎；命令级并发行为由测试覆盖，取消不会因错误的生命周期锁设计而失效；Rust 与既有回归结果可追溯且对应当前源码；所有既有工作区改动保留。
- **风险与停止条件：** 若修复需要修改 DSH Runtime、协议、模型路由、凭证或数据格式，停止并报告；若本机无法编译 Rust，不得声称当前源码 Rust 全量通过，需提供可复现失败原因和其余独立检查证据；不安装系统开发包、不提权。发现未授权数据或秘密时立即停止相关检查。
- **交付方式：** 集中完成修复、测试、证据和状态更新后停止。不得自行合并、提交或推送；由 ARCHITECT 按实际源码、当前 HEAD 差异和测试证据独立复核。

#### 窗口销毁关闭路径补齐（已完成并复核通过）

- **目标：** 让窗口关闭与命令触发的引擎停止共享同一并发语义，避免耗时关闭时持有宿主锁。
- **影响范围：** 仅 `desktop/src-tauri/src/lib.rs` 现有窗口销毁回调、共享关闭入口、对应 Rust 宿主测试和 OD-ARCH-024 证据/状态记录。
- **步骤：** (1) 复用 `stop_engine()` 或等价共享函数，使窗口销毁回调不在持有 `SharedHost` 锁时等待关闭；(2) 添加确定性宿主级测试，验证关闭等待期间宿主锁仍可取得，状态/结果/取消按既定语义快速返回；(3) 重跑 Rust 离线全测及本计划规定的桌面回归，更新单一证据索引和状态记录。
- **验收标准：** 生产窗口销毁入口不持有宿主锁执行耗时收尾；回归能覆盖该入口的并发关闭行为；计划规定检查通过，或将环境限制标记为未验收。
- **风险与边界：** 不改 `runtime/`、协议、模型路由、凭证或用户数据；保留全部既有工作区改动，不提交、不推送、不打标签。
- **状态：** 用户已批准并由 BUILDER 完成；ARCHITECT 按范围复核通过，结论见 REVIEW.md 顶部。窗口回调仍在事件循环线程同步等待关闭，此行为沿用既有设计；本次只修复不持有宿主锁的并发问题。
- **执行结论（执行方记录，非审查结论）：** 新增 `on_window_destroyed_shutdown(&SharedHost)` 复用共享入口
  `stop_engine()`，`WindowEvent::Destroyed` 回调只调它，不再 `state.lock()` 后直连 `bridge.shutdown()`；
  关闭入口唯一。新增确定性宿主级回归
  `window_destroy_shutdown_does_not_hold_host_lock_while_waiting`：8 s 关闭窗口期间宿主锁 <1.5 s 可取得、
  取消/状态/结果 <1.5 s 返回“关闭中、未发送”、关闭开始后引擎未收到取消。变异验证（改回持锁等待）令该测试以
  `关闭等待期间宿主锁应可立刻取得（等了 8.041135831s）` 失败。回归 **134/134**，详见
  [证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)「第四轮」。

**ARCHITECT 复核后的限定补测/修正任务（用户已批准；已于 2026-10-19 执行完成并复核通过）：**

> **执行结论（2026-10-19，执行方记录，非审查结论）：** 三项要求均已落实。并发边界改为：`EngineSupervisor` 在
> **supervisor 锁内**原子完成「置位“正在关闭” + 摘除通道」（`begin_stop()`），耗时的协议关闭与进程收尾
> （`finish_stop()`）在**锁外**执行；任务命令的判定与发送同在这一把锁内（`EngineBridge::rpc`/`with_channel`），
> 因此要么在关闭前送达、要么在锁内立即得到 `engine_shutting_down`（“未发送”），不存在“检查通过后被关闭抢先”的窗口。
> 确定性同步点测试把取消停在“已进入发送路径、尚未取得 supervisor 锁”，让关闭先完成 `begin_stop()`，取消仍 <1.5 s
> 返回明确结论且**未送达**引擎；变异验证（把耗时关闭放回锁内）令该测试以「等了 8.042621669s」失败。完整回归见
> [证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)「第三轮」。

> 继续完成 OD-ARCH-024 尚未通过的并发验收，不要扩展产品功能。先记录 HEAD 与完整工作区状态并保留全部改动。只检查相关 Rust bridge、shutdown、任务状态/结果/取消实现及 OD-ARCH-024 证据。当前缺口：任务命令先检查关闭标志，再获取 supervisor 锁；关闭流程可能在两者之间先取得该锁，使请求排队到关闭之后。
>
> 分成三项集中完成：(1) 用确定性测试同步点卡在“关闭标志检查通过、请求尚未取得 supervisor 锁”之间，并建立真实合成在途任务；令关闭流程先取得 supervisor 锁，验证 status/result/cancel 的延迟、是否送达和错误语义。(2) 将任务命令与关闭操作整理成可线性化的最小并发边界：请求要么在关闭前送达，要么快速明确返回“关闭中、未发送”，不能等关闭结束后才普通失败。不得改 Runtime、外部协议、路由或凭证；若需要扩大架构则停止并报告。(3) 留存可复现证据，运行完整 Rust 离线测试、前端构建、桌面链/取消/草稿/会话回归、路由自测、`git diff --check HEAD` 和 `runtime/` 无差异检查；更新 STATUS.md、PLANS.md 与单一证据索引，完成后停止等待 ARCHITECT 复核，不提交、不推送。

**交给 OpenCode 的执行说明（由用户在批准后转发）：**

> 执行 OD-ARCH-024。先记录 HEAD 与完整工作区状态并保留全部差异。先阅读 AGENTS.md、本计划 OD-ARCH-024、REVIEW.md 顶部 OD-ARCH-023 结论以及相关 Rust 源码；不要全仓库扫描。修复 with_engine_call / with_call_gate 的生命周期锁自锁，不能牺牲权限世代检查；为状态、结果、取消命令写明并落实权限变更和关闭期间的并发规则，特别确保取消不会被不合理地堵塞。用宿主包装器和假 bridge/引擎做确定性测试，覆盖成功、拒绝、权限撤销竞争、停止失败与恢复，以及状态/结果/取消行为；不能只加源码字符串测试或生命周期纯单测。再跑计划要求的 Rust 离线测试和相关前端、路由、桌面、取消、草稿、会话回归、diff 检查及 runtime/ 无差异检查。每项结果记录实际命令、精确通过数和证据文件，并区分从当前源码新编译的测试与旧预编译二进制。若 Rust 编译受系统依赖限制，如实记录，不提权、不安装系统依赖。更新 STATUS.md、PLANS.md 和单一证据索引，完成后停止等待 ARCHITECT 复核；不改 Runtime/凭证/模型路由/真实材料，不提交或推送。

### OD-ARCH-023：修复模型权限变更与引擎启动之间的并发竞态

- **状态：BUILDER 已执行；ARCHITECT 独立复核不通过；修复已由 OD-ARCH-024 完成，待复核。** 本条保留为执行范围记录；问题证据见 REVIEW.md 顶部，修复见本文件顶部 OD-ARCH-024。
- **目标：** 确保权限设置更新、准入判定和引擎生命周期操作有一致的并发边界；撤权开始后，任何拿着旧准入结果的命令都不能重新启动引擎或接收新请求。
- **影响范围：** 仅桌面 Rust 宿主的模型权限变更、准入与引擎启动入口、相关并发测试、OD-ARCH-022 证据/共享状态记录。不得修改 `runtime/`、模型路由、凭证、真实材料或第二 worktree；不得提交、推送或打标签。
- **实施步骤：** (1) 盘点所有会初始化/触达引擎的宿主命令，覆盖任务提交、描述、会话列表/恢复、任务台账/恢复；健康状态等只读快照单独确认不会启动进程。 (2) 为授权设置变更与所有引擎启动/请求建立可证明的串行化边界；准入结果必须在同一生命周期保护下校验并使用，权限变更过程中先拒绝新的启动与提交，不能靠稍后设置屏障来补救。避免仅加 UI 锁或只在 `engine_submit_task` 检查屏障。 (3) 增加确定性并发回归：暂停某调用于准入完成之后，执行撤权并确认旧引擎已停，再释放该调用；证明旧 grant 不会启动引擎、创建会话或提交任务。分别覆盖提交和至少一个非提交启动入口，并覆盖停止失败时持续拒绝、恢复后才允许。 (4) 验证权限持久化失败、撤权、重新授权和并发多次切换的结果一致，错误状态不会假报成功。 (5) 运行 Rust 完整离线测试、前端构建、现有准入/桌面/会话回归、`git diff --check HEAD` 与 `runtime/` 无差异检查，更新证据和状态；若 Rust 工具链不可用，提供可复现的替代运行证据并明确独立验证限制。
- **验收标准：** 并发测试能稳定复现并覆盖旧 grant 时序；撤权操作完成后，任何在其开始前取得的 grant 都不能启动或调用引擎；停止失败期间所有会启动/提交模型工作的入口均被屏障拒绝，确认停止且完成恢复后才重新开放；整个计划要求的回归通过，既有工作区改动完整保留。
- **风险与停止条件：** 若要改 DSH Runtime、协议、模型路由或读取凭证才能建立边界，停止并提交架构证据，不扩大范围。若无法构造确定性并发测试，不得以“竞态很窄”或纯函数单测替代验收。
- **交付方式：** 一次集中完成修复、测试与证据；BUILDER 完成后停止，由 ARCHITECT 独立审查源码、差异和运行结果。

### OD-ARCH-022：端点去向说明、权限切换一致性与安装验收

- **状态：已执行；ARCHITECT 独立复核为部分通过，暂不验收。** 端点类别、分类测试、前端构建与隔离安装证据通过；权限切换存在并发竞态，后续修复见 OD-ARCH-023。结论见 [REVIEW.md](REVIEW.md)；执行事实见 [STATUS.md](STATUS.md) 与 [证据索引](tests/verification/desktop/evidence/od-arch-022/README.md)。
- **目标：** 修正模型端点类别的过度承诺；确保权限切换失败时不能让旧引擎继续处理新提交；完成隔离安装验证，让 OD-ARCH-021 达到可验收状态。
- **影响范围：** 仅主工作区现有路由分类、准入/设置、引擎停止状态、相关前端说明、测试、OD-021 证据索引和共享状态文件。不得改 `runtime/`、模型路由或凭证，不调用真实模型、不使用真实材料、不改第二工作区、不提交或推送。
- **实施步骤：** (1) 将端点事实与网络归属分开表达：回环可标为本机；非回环地址不得仅凭 RFC1918、链路本地、`.local` 或公网域名声称“单位内部”或确定数据是否离开单位。采用最小且诚实的类别/设置文案；无法确认组织归属或实际转发路径时明确标“无法确认”，仍由用户显式选择是否允许普通任务。 (2) 调整权限更新顺序与宿主状态机：只有旧引擎停止并确认不再接收任务后才报告变更生效；停止失败时拒绝新任务、显示明确失败状态，并提供可恢复路径，不返回假成功。覆盖启用、撤销、运行任务中变更及停止失败。 (3) 更新分类测试与证据说明，按脚本实际结果校准为 18 项；补充私有地址不等于单位网络的断言。 (4) 在临时隔离根/用户级可控位置完成 `.deb` 安装、启动和卸载验证；禁止系统级安装或权限提升，若环境不支持则停止并如实保留“未完成”。 (5) 运行计划要求的前端类型检查与构建、Rust 完整离线测试、相关 Python/路由/桌面回归、`git diff --check HEAD` 及 `runtime/` 无差异检查，集中更新状态与证据索引。
- **验收标准：** UI 和宿主不把网络地址特征说成单位授权或网络归属；遇到无法确认的数据路径时清楚告知；权限切换期间没有沿用旧路由处理新输入的路径，失败后保持拒绝并可恢复；测试覆盖成功与失败边界；OD-021 的分类用例数与实际一致；隔离安装后可从应用入口启动（若系统限制阻塞，单列未通过）；完整回归通过且既有工作区改动保留。
- **风险与停止条件：** 端点分类若需要读取凭证、修改 DSH 路由、Runtime 或读取用户数据才能完成，停止并报告；安装若需要管理员权限或系统级改动，停止该步骤；发现权限绕行、真实材料或秘密时停止相关操作。不得为追求通过而把地址猜测写成确定事实。
- **交付方式：** 由用户选择 BUILDER；一次集中完成本计划和定向修复。执行完成后停止，由 ARCHITECT 按实际源码、diff、测试和安装证据复核。

---


### OD-ARCH-021：可配置模型权限与 AI 任务闭环、桌面安装验收

- **状态：用户已批准；BUILDER 已执行（2026-10-18）；ARCHITECT 复核为部分通过，整体未验收。** HEAD 仍为 `5ffc710`，未提交、未推送、未打标签。执行记录见 [STATUS.md](STATUS.md) 顶部，复核见 [REVIEW.md](REVIEW.md) 顶部，证据见 [tests/verification/desktop/evidence/od-arch-021/README.md](tests/verification/desktop/evidence/od-arch-021/README.md)。
- **目标：** 首次严格关闭模型权限且明确告知数据去向，同时让用户能自行决定已配置模型是否可处理普通任务；以合成端点验证周报、文案和项目任务的 AI 提交到结果、编辑、人工确认和保存闭环，并完成 Linux 安装入口验收。
- **影响范围：** 主工作区现有设置页、准入判断、Rust 宿主和既有 DSH ACP 适配链、合成测试及打包验收。不得修改 `runtime/`、读取/打印/复制凭证、改变现有模型路由、接入真实工作材料、调用真实外部模型、修改第二 worktree、提交或推送。
- **实施步骤：** (1) 只读查明现有宿主能否从非秘密配置中可靠获知有效模型类别与数据去向，不读取凭证值。 (2) 若可可靠识别，为本机/外部/未知类别实现首次关闭、用户逐项开启与可撤销设置；界面说明内容是否离开本机/组织；宿主在任务提交时强制执行；未知类别默认拒绝。不得只信任用户任意填写的 provider 名称。 (3) 如端点实际类别无法可靠判断，停止该部分并提交证据与最小架构选择，不实现伪分类或旁路。 (4) 用合成端点从自有桌面界面验证三类任务的提交、成功、错误、取消、编辑、确认保存；验证首次拒绝、开启、撤销、未知端点及禁入任务拒绝，并证明拒绝不启动引擎或泄露输入。 (5) 构建并安装本机 `.deb` 到隔离或用户级可控环境，验证启动器名称、图标和启动；不改系统级配置。若必须提升权限或改系统依赖，停止并报告。 (6) 执行完整前端/Rust/Python相关回归、`git diff --check HEAD`、`runtime/` 无差异检查，保留合成证据并更新状态。
- **验收标准：** 首次模型权限明确关闭且设置可持久化、可撤销；用户能理解并选择外部数据发送风险；宿主执行与 UI 一致；未知端点及禁入范围拒绝；合成端点证明完整任务闭环；桌面包安装后可启动；计划内回归通过且既有工作区差异保留。真实模型质量、真实业务材料授权和跨平台仍单独标为未验证。
- **风险与停止条件：** 若端点类别/数据去向只能通过读取凭证、修改路由或 Runtime 才能判断，停止该部分并报告；若安装需要系统级依赖或权限，停止安装部分；发现真实材料、秘密或计划外高风险差异时停止相关操作。
- **交付方式：** BUILDER 集中交付实现、测试结果、单一证据索引与遗留限制，之后停止等待 ARCHITECT 复核。

> OD-ARCH-021 已由 ARCHITECT 部分通过复核；整体验收待 OD-ARCH-022 修复并补齐安装证据。OD-ARCH-020 已由 ARCHITECT 部分通过：发行包产出、成果目录设置与离线保存/重开通过；模型权限和 AI 任务闭环已有合成端点证据，限制见 REVIEW.md。

## OD-ARCH-020 执行计划（已复核，部分通过）

### OD-ARCH-020：个人试用版闭环——安装入口、成果目录设置与 AI 工作流程（已部分复核）

- **状态：用户已批准；BUILDER 已执行；ARCHITECT 于 2026-10-18 部分通过复核。** 当前 HEAD 为 `5ffc710`，未提交、未推送、未打标签。主工作区原有未提交/未跟踪改动全部保留；`/mnt/data/Code/omniharness-ui` worktree 未改动、未合并。AI 准入与真实任务闭环未完成，后续列入 OD-ARCH-021。详情见 [REVIEW.md](REVIEW.md) 顶部。
- **实际执行结果（摘要，详细证据见 [STATUS.md](STATUS.md) 顶部与 [证据索引](tests/verification/desktop/evidence/od-arch-020/README.md)）：** Linux `.deb` 已产出（`omni-desk` 0.1.0），AppImage 因 `linuxdeploy` 失败未产出，`.deb` 未安装；设置页可用路径输入框选择成果目录，重启后保留，非法路径被拒绝，既有文件哈希不变；同标题再次保存按秒级时间戳新建文件而不覆盖；三个离线场景的人工确认与保存、只读重开已实测；AI 初稿在准入关闭时被宿主拒绝且未发送到任何模型；使用记录只含非正文字段。**未完成/未验证：** 真实模型提交、生成、取消与质量（准入仍为 `false`），原生目录选择器，AppImage，`.deb` 安装后启动，Windows/macOS。
- **分类：** 产品化功能与端到端工作流程。
- **目标：** 用户能从可识别的桌面入口启动 OmniDesk；在设置中选择并更改新成果保存目录；周报、一般材料写作、项目管理从现有独立界面进入 AI 协助流程，能提交任务、查看结果、继续修改、人工复核并保存。完成后再由用户评估实际工作体验，而非承担技术验收。
- **影响范围：** 以 `/mnt/data/Code/MyWorkStation` 为唯一实施目标，只改桌面前端、Rust 宿主/现有适配器必要接口、打包配置、合成样例和定向验证及共享状态文件。UI worktree 只读参考；按文件审查后择取必要前端设计，不整支合并。不得修改 `runtime/`、DSH 私有数据格式、现有用户数据、凭证或模型路由；不得提交、推送、打标签、安装系统级软件或使用真实工作材料。
- **实施步骤：**
  1. 复核并记录两工作区 HEAD、状态和差异边界；确认当前桌面入口、任务提交/取消/错误处理、保存路径和准入检查。以主工作区实际代码为准，先做短清单，不重扫无关文档。
  2. 补齐桌面发行入口：构建本机 Linux 可安装包，核对包内图标、应用名、启动器条目和启动命令；提供可复现的构建与安装说明。只产出安装包，不替用户安装，不改桌面或系统配置。
  3. 增加应用设置中的成果目录选择和当前路径显示。选择路径时继续拒绝仓库内、DSH/Agent/Codex 私有目录及不安全软链接；检查目录可写时使用一次性探针并清理。更改只影响之后新建的成果，清楚提示旧目录文件不会自动移动、覆盖或删除；提供打开当前目录的入口。应用自身配置目录与成果目录分开。
  4. 将三个优先场景接入一致的 AI 任务体验：表单收集用途/受众/素材，调用现有引擎适配链路，展示生成过程与结果，支持取消、失败重试、人工编辑/复核和保存；提示词、写作规则使用现有可编辑流程包，语言工作交给模型，日期/统计等确定性计算留在代码。不得新增通信层或多 Agent 平台。
  5. **客户可配置的模型准入。** 首次使用采用严格默认：本机和外部模型都不自动获得工作材料处理权限；用户必须显式选择端点类型、可处理的任务/材料范围和确认方式。设置页以通俗语言说明本机、单位内部和外部服务的差别，以及内容是否可能离开设备/单位环境；外部模型默认关闭，开启前由用户确认其授权与后果。用户可在已获授权或理解风险后自行放宽普通工作场景的设置，软件不得把外部模型永久一刀切禁用。端点类型未知时默认拒绝；案件、线索、人物画像及明确禁入内容仍按当前产品边界拒绝，除非未来用户正式调整产品目标并另行审查。端点身份不能只凭用户填写的名称判断；以实际 provider/endpoint 配置来源进行分类，不能确认时显示“未知”。设置与授权确认可在本地留最小审计记录，不保存提示词正文或凭证；准入由宿主执行，前端负责解释和交互。
  6. **真实模型测试边界。** BUILDER 不读取或写入凭证，不擅自修改现存模型路由。先用合成端点验证 UI 到适配层的请求、结果、取消和错误链路，并验证严格默认、用户逐项开启、关闭/撤销、未知端点拒绝及禁入范围拒绝。产品实现可支持客户自行配置获准端点；真实模型质量测试仅在用户指定端点、用途和环境并明确授权后执行。在测试前不得称 AI 写作已通过质量验证。
  7. 以虚构内容进行完整桌面纵向验证：安装包信息与启动入口、保存目录选择/重启后保留/拒绝非法路径、三场景任务提交与取消/错误/结果编辑/保存、未知端点和禁入任务默认拒绝。跑计划要求的前端构建、Rust 完整测试、Python 适配器定向回归、`git diff --check HEAD` 和 `runtime/` 无差异检查，证据不含私人路径、会话正文或凭证。
- **验收标准：** (1) 本机发行包包含 OmniDesk 桌面启动器与图标，构建和安装说明可复现；(2) 用户可在应用内改成果目录，设置重启后仍生效，非法/越界路径被拒绝，既有文件不被迁移或删除；(3) 三个场景走同一套清楚的 AI 任务交互，提交、取消、错误、编辑和保存均有真实窗口证据；(4) 宿主端按模型类型与任务范围执行准入，未知端点、外部端点未确认及禁入任务均被拒绝；(5) 所有测试通过且 `runtime/`、模型路由、凭证和用户数据无变化；(6) 真实模型调用与输出质量需在独立授权后验证，未验证前不得宣称 AI 功能完成或交付实际材料测试。ARCHITECT 独立 Review 通过后，再交用户做应用体验反馈。
- **风险与停止条件：** 桌面打包或目录选择可能需要新依赖；如需安装系统依赖或改系统配置，停止并报告；如果现有准入关闭使真实任务提交无法运行，保留关闭状态并用合成替身验证，不设隐藏旁路；若实现需改 Runtime、DSH 私有会话格式、触碰凭证/真实材料或覆盖任一工作区现存差异，立即停止相关部分。
- **交付方式：** 一次完成范围内的应用闭环和回归，集中提交差异清单、构建/测试命令、桌面证据和未验证项；不拆成多个微任务。完成后停止等 ARCHITECT 独立审查，不自行合并或提交。

## 最近复核完成

### OD-ARCH-019：可编辑离线流程包与轻量个人试用记录

- **状态：BUILDER 于 2026-10-18 执行；ARCHITECT 有条件通过复核。** 个人可开始用合成或公开材料试用离线步骤。真实模型写作、真实工作材料、跨平台和长期使用均未验证；未提交、未推送、未打标签。执行记录见 [STATUS.md](STATUS.md) 顶部，独立审查见 [REVIEW.md](REVIEW.md) 顶部，证据见 [tests/verification/desktop/evidence/od-arch-019/README.md](tests/verification/desktop/evidence/od-arch-019/README.md)。
- **分类：** 个人试用能力与本地数据最小化。
- **目标：** 让作者能根据实际使用调整周报、一般文案结构和项目跟进的离线流程内容，无需改代码或重新编译；以不收集正文的方式记录使用时长、返工和可用性，便于后续决定是否值得接入模型能力。
- **影响范围：** 当前主工作区的 `desktop/`、Rust 桌面宿主、必要的仓库内默认流程包与合成验证、桌面定向测试，以及 `PLANS.md`、`STATUS.md`、`REVIEW.md`；若形成新架构决定，再由 ARCHITECT 更新 `docs/decisions.md`。实施前须先核对现有数据目录和使用记录格式，优先复用现有目录。不得改 `runtime/`、第二 worktree、模型路由/准入、凭证、DSH 用户数据或已有用户成果；不得使用真实工作材料、调用外部模型、提交/推送/打标签。
- **实施步骤：**
  1. 记录 HEAD、完整工作区状态和暂存区；只检查当前离线流程的输入、计算、渲染、配置及数据落点，区分固定计算逻辑与可编辑文案，不重扫无关目录。
  2. 定义一个小型、带版本的声明式流程包，承载现有可配置内容（名称、字段说明、输出标题/章节等）；放在仓库外现有 OmniDesk 配置目录，默认文件可从仓库内合成样例初始化。首次创建后不覆盖用户修改。提供显式重载；格式错误时给出可读错误并拒绝该配置，不静默回退成看似成功。
  3. 让三个离线流程读取该配置并在界面和结果中反映修改；日期校验、状态计数、逾期判定等确定性计算仍由代码完成。配置只允许数据和文本，不允许运行脚本、加载外链、改变路径权限或触发模型调用；页面继续明确标注“不调用模型”。若实现需要通用工作流引擎、任意脚本执行或新增通信层，停止并报告。
  4. 增加明确可选的使用记录：仅记录场景枚举、开始/结束时间或用户确认的耗时、结果可用性枚举、返工次数和是否愿意再用；不得记录输入/输出正文、自由文本评论、标题、会话标识、路径、模型或凭证。记录留在仓库外本地目录，提供查看摘要和清除入口；不自动上传或同步。
  5. 用合成数据验证三场景：仅改流程包并重载即可改变可见流程/输出；未知版本、缺项和格式错误被安全拒绝；使用记录字段严格白名单且可清除；主计算结果与既有测试一致。完成一次隔离桌面窗口验收，并运行计划要求的完整前端、Rust、Python相关回归、`git diff --check HEAD` 和 `runtime/` 差异检查。保存脱敏证据后停止交 ARCHITECT 复核。
- **验收标准：** (1) 三个流程的规定可编辑内容可在不改源码、不重编译的情况下生效；(2) 配置不执行代码、不调用模型，非法配置不会静默产生误导结果；(3) 确定性计算、人工确认、成果保存/重开行为不回退；(4) 试用记录只含列出的非正文字段，默认可不记录，能本地查看与清除；(5) 桌面交互与计划要求的完整回归有可复核证据；(6) `runtime/`、端点准入、凭证、用户数据无变化，保留所有既有工作区差异，不提交或推送。
- **风险与限制：** 可编辑配置仅能调整已实现的离线流程表达，不能被描述为 AI Skill 执行或 AI 写作；使用时长由个人确认时可能不精确；试用元数据本身仍是本地记录，需可查看、清除且不入 Git。会话页因端点准入关闭仍不可用，旧 DSH 会话导入仍不支持，本计划不改变这两项边界。
- **停止条件：** 发现配置会触发模型/外部工具、需扩大授权、需要访问真实材料或旧会话、需修改 Runtime/其他工作区，或发现疑似凭证/真实业务内容进入仓库时，停止相关部分并报告；不通过放宽边界继续。
- **集中交付：** BUILDER 一次性交付代码差异、使用说明、单一证据索引和完整测试结果；不拆成多个小任务。ARCHITECT 独立复核前，不宣称该能力已实现或验收。

## 最近完成并复核

### OD-ARCH-018：个人试用准备与桌面交互验收

- **状态：BUILDER 已执行；ARCHITECT 于 2026-10-18 有条件通过复核。** 三个离线流程在 Linux 虚拟桌面完成保存与重开；Rust 92/92 与窗口证据由 BUILDER 留存，ARCHITECT 因环境无 `cargo` 未能独立重跑 Rust。真实模型生成、真实窗口取消、真实工作材料、跨平台和长期使用均未验证。未提交、未推送、未打标签。执行记录见 [STATUS.md](STATUS.md) 顶部，独立审查见 [REVIEW.md](REVIEW.md) 顶部，证据索引与截图见 [tests/verification/desktop/evidence/od-arch-018/README.md](tests/verification/desktop/evidence/od-arch-018/README.md)。
- **分类：** 个人工作台可用性、安全边界收口与合成/公开材料场景验收。
- **目标：** 让用户能在自己的桌面界面中安全地试走三个优先场景；不把离线流程称为 AI 写作，不开放模型或真实工作材料；收紧会话与任务列表显示的元数据，并验证取消交互的完整状态。
- **影响范围：** 仅当前主工作区的 `desktop/`、`engine/omnidesk_engine/` 必要字段、相关定向测试、合成样例与 `tests/verification/desktop/evidence/od-arch-018/`，以及 `STATUS.md`、本计划和 `REVIEW.md`。保留所有现有工作区改动。不得修改 `runtime/`、其他 worktree、模型路由/准入常量/凭证、DSH 用户数据或旧会话；不得接入真实工作材料、调用外部模型、提交/推送/打标签，也不得安装系统依赖或更改用户环境。
- **实施步骤：**
  1. **最小化会话/任务信息。** 会话列表默认仅显示稳定标识和不含绝对路径的工作区名称；任务台账不向界面返回或显示自由文本 `intent`。恢复所需的内部路径仍由受控 IPC 使用，不进入界面文案、错误提示或证据。用合成敏感哨兵验证 API 响应、可见界面与日志/证据均不泄露完整路径、项目名或 intent。
  2. **验证取消按钮交互。** 以可控的前端宿主替身覆盖首次取消、等待期间重复点击、超时后重试、任务先完成/失败、取消确认与取消未确认；断言每个阶段实际发出的 IPC 次数和用户看到的状态。不得只验证状态纯函数，也不得启动已准入关闭的真实模型链路。
  3. **准备三个可复用桌面试用样例。** 为周记录汇总/周报、一般文案、轻量项目管理各准备一组虚构或公开输入；确认用户可从自有界面找到入口、完成离线流程、人工编辑并将成果保存到统一的仓库外成果目录。若现有流程只是模板/确定性整理，应在界面明确标注“不调用模型”，不得暗示具备生成式写作或真实工作能力。
  4. **一次性桌面纵向验收。** 在本机隔离的合成数据根中实际打开桌面窗口，完成以上三个场景、保存与重新打开成果，并检查取消交互及会话页的隐私显示。只要桌面构建受当前系统依赖限制，就记录具体限制、可复现命令和未完成项；不得用浏览器构建或脚本通过冒充窗口验收。
  5. **集中回归与交接。** 执行相关前端构建/测试、Rust 与 Python 定向回归、三个场景的端到端检查、`git diff --check HEAD` 与 `runtime/` 差异检查；确认准入仍关闭、数据均在仓库外且仓库没有合成正文残留。统一更新状态与证据索引，停止等待 ARCHITECT 复核。
- **验收标准：** (1) 会话页不展示绝对工作区路径，任务页不展示自由文本 intent，敏感哨兵在响应/UI/证据中不可见；(2) 取消状态的实际 IPC 次数符合“等待中不重复、明确超时才可重试”；(3) 三个场景均从桌面窗口完成输入、编辑、保存、重新打开，产物位于仓库外统一目录，且界面如实说明离线能力；(4) 计划要求的测试通过，或将环境限制明确记录为未验收，不得降格替代；(5) `runtime/`、准入常量、凭证和用户数据无变化，不提交、不推送。
- **风险与限制：** 当前执行环境此前缺少 GTK/GDK/Cairo/Pango/ATK 开发库，因此真实窗口构建可能仍受阻；未解决该环境阻塞前，不得宣称桌面可用。三个离线样例只能验证流程、保存和交互，不能证明模型写作质量、实际节省工时、单位授权或真实工作适用性。旧 DSH 会话导入仍不支持，不在本计划内自行开发格式转换。
- **停止条件：** 若测试必须读取真实会话/工作材料、连接外部端点、放宽准入或修改 Runtime，立即停止该部分并报告；其余隔离合成检查可完成。
- **BUILDER 完成后交付：** 一份集中执行记录、一个证据索引、实际差异及完整测试结果；不拆成多个小任务交接。完成后由 ARCHITECT 基于源码、差异和可复核结果独立审查。

**执行记录（BUILDER；独立复核结论见 REVIEW.md）：** 三个离线场景均在隔离合成数据根的真实 Tauri 窗口中完成输入、本地处理、人工确认、保存与重新打开；成果落盘在仓库外，界面明确标注“不调用模型”。会话页与任务页在准入关闭时只显示拒绝原因，未显示绝对路径、自由文本 intent 或正文。实测中修复了一个真实缺陷：成果列表原先因文件名解析错误恒为空，现已能列出并重开全部成果。`cargo test --offline` 92/92、前端类型检查与构建通过、会话连续性 37/37、取消状态与 IPC 34/34，`runtime/` 无差异、两侧准入常量为 `false`。取消按钮的真实窗口点击与真实模型生成仍为未验证项。

## 最近完成计划

### OD-ARCH-017：会话连续性纵向能力（已执行；ARCHITECT 有条件通过复核）

- **状态：已执行并经 ARCHITECT 有条件通过（2026-10-18）。** 合成适配层及前端构建通过；真实桌面窗口交互与完整 Rust 独立复验未完成。旧 DSH 会话导入因无公开导入接口而不支持。独立审查见 [REVIEW.md](REVIEW.md) 顶部；执行记录见 [STATUS.md](STATUS.md) 顶部；证据见 [tests/verification/desktop/evidence/od-arch-017/README.md](tests/verification/desktop/evidence/od-arch-017/README.md)。未提交、未推送、未打标签。
- **分类：** 桌面工作台会话/任务生命周期功能。
- **目标：** 让用户能在 OmniDesk 中找到并重新打开 DSH 已持久化会话；在隔离数据根中显式导入用户选定的旧 DSH 会话；重启后如实区分已完成、未完成和状态未知的任务；修正取消中重复发送问题。复用 DSH 原生持久化与 ACP，不另存一份逐消息正文日志，不自动重放中断任务。
- **影响范围：** 当前主工作区 `desktop/`、`engine/omnidesk_engine/` 的适配层、相关契约/测试、专项合成证据及 `STATUS.md`；由 ARCHITECT 更新 `REVIEW.md`。不得修改 `runtime/`、DSH 原生数据格式、模型端点/路由/凭证、第二 worktree、真实用户数据或现存旧会话；不得把会话正文、真实路径或凭证写入 Git、报告或通用日志；不得提交、推送、打标签。会话数据只在一次性仓库外合成根中验证，旧数据源只读。
- **实施步骤：**
  1. **一次性确认接口与数据路径。** BUILDER 只查相关锁定版 DSH ACP/持久化入口、当前 Adapter 与 Tauri 调用链，确定 `session/list`、`session/resume` 和旧数据导入可用的原生路径；不得遍历用户数据或读取真实会话正文。沿用已确定决策：原生 DSH 会话是持久数据，禁止擅自删除、覆盖；不新建重复正文日志。
  2. **会话清单与重新打开。** 在 OmniDesk 界面显示会话必要元数据并按规范化工作区归组；可重开既有会话。列表遗漏、工作区路径别名、会话不可恢复和无效/不兼容记录须有明确状态；只读清单不应擅自修改来源文件或注册表。
  3. **旧 DSH 会话导入。** 提供由用户明确选择来源和目标的导入路径，先预览可导入元数据、版本兼容及冲突，再经明确确认导入。导入须保留原始来源只读、避免覆盖目标、支持重复导入检测，并记录可核验的数量/完整性结果，不把会话正文复制进仓库或模型。优先使用锁定版 DSH 已支持的机制；如果只能自行复制/改写其发布格式、需要改 `runtime/`，或无法保证来源不变与目标可恢复，则停止导入实现，提交兼容性结论，不自行发明迁移格式。
  4. **重启状态与取消闭环。** 从已有 OmniDesk 任务记录恢复可证实的终态；无终态证据的任务标为 `unknown/interrupted`，允许用户打开其原会话查看，但不自动重放或声称续跑。取消按钮仅在首次请求前、或明确超时后允许重试；请求已发出但仍在取消中的任务不得再次发送。用确定性宿主替身或隔离合成 ACP 端点验证取消请求次数、终态竞争、超时重试和重启状态。
  5. **完整纵向验证。** 在一次性仓库外合成数据根中，从会话创建、写入、列表、关闭、重启恢复、旧会话导入到再次打开完整走通；覆盖重复/冲突/不兼容/损坏元数据和中断任务。验证来源未变、目标不被覆盖、仓库无会话正文、端点准入仍关闭。执行前端构建、完整 Rust/Python 相关回归及计划内桌面交互；保存不含真实正文或私人路径的证据后停止，等待 ARCHITECT 审查。
- **验收标准：** (1) 合成会话可在 OmniDesk 列出并按工作区找回；(2) 选定旧 DSH 合成会话可按支持的原生机制导入、重开，来源保持只读且哈希/清单未变；重复导入不覆盖、不产生含糊副本；(3) 不兼容或不确定内容明确拒绝并说明原因；(4) 重启后任务状态可信，未知状态不自动重放；(5) 取消等待期间只发一次请求，只有明确超时后可重试；(6) 前端、宿主、适配层及相关回归通过，隔离桌面关键路径有运行证据；`runtime/`、模型路由和用户数据无差异；无真实会话或正文进入 Git；未提交、未推送。
- **风险：** DSH ACP 列表与本地持久化、workspace 索引可能有过滤差异；会话元数据中的路径和标题本身也可能敏感；旧会话格式虽为发布格式，跨版本导入未必受支持；重开会话不代表中断任务续跑。报告仅记录脱敏后的计数与结果。
- **停止条件：** 若必须修改 DSH Runtime/格式、导入要求读入或发出真实会话正文、无法在仓库外隔离数据、来源可能被修改/覆盖、模型端点或凭证被触达，立即停止相关部分并报告；其余独立合成部分可完成。不得以范围扩大或放松安全条件来达成验收。
- **BUILDER 完成后交付：** 一份集中状态记录、一个统一证据索引、实际差异和全部测试结果；不分多个小交付，不在中途要求逐点 ARCHITECT 复核。遇硬停止条件才提前交接。

## 最近完成计划

### OD-ARCH-010：封住当前工作区准入缺口，并启动优先工作场景的离线验证

- **状态：已执行并完成 ARCHITECT 独立验收（2026-10-18），仅离线合成流程按范围通过。** 用户在本轮明确下达「OD-ARCH-010 授权执行」。执行记录见 [STATUS.md](STATUS.md) 顶部，证据见 [tests/verification/desktop/evidence/od-arch-010/README.md](tests/verification/desktop/evidence/od-arch-010/README.md)；真实业务与模型使用仍未验收。
- **分类：** 安全边界修复 + 个人工作场景流程验证。
- **目标：** 先在用户指定的唯一工作区确认并封住引擎描述、任务提交、目录准备和直接引擎启动路径的宿主拒绝入口；随后仅用合成/公开材料验证周记录与周报、一般文案撰写、轻量项目管理三个场景的本地流程。复杂数据分析暂缓，不作为本阶段重点。
- **影响范围：** 只允许在用户指定的单一工作区修改当前桌面宿主、必要的准入测试、场景流程和合成样例，以及 STATUS.md、证据材料。不得复制或整体合并另一 worktree 的改动；不得修改 runtime/、端点常量、模型路由、凭证、真实业务材料或既有用户数据；不得调用云端/单位模型、解除准入关闭、提交/推送/打标签或启动用户正在使用的实例。
- **实施步骤：**
  1. 执行前记录唯一目标工作区的 HEAD、状态与差异清单；确认其与另一工作区差异不被覆盖。只核对所需规则、入口源码和相关测试。
  2. 通过宿主代码在任何引擎子进程启动、描述、提交任务、创建工作目录之前统一拒绝；测试直接 Tauri 命令入口和底层启动入口，拒绝路径不能创建目录、进程或任务。保持端点准入关闭，不新增可配置旁路。
  3. 做三个离线合成场景的最小纵向验证：每日事项记录汇总为周报；按用途/受众要求组织一般文案并标注缺失信息；轻量项目管理包含事项、负责人代号、期限、状态和下一步。不得将真实业务内容用于夹具。
  4. 在端点闸门保持关闭时，只验证本地表单、流程状态、确定性校验和本地成果保存；模型撰写步骤标为“受准入阻塞、未验证”，不得声称已完成真实生成或真实模型质量评估。
  5. 复杂数据分析只登记为后续候选，不新增功能、不扩大评测；记录场景测试是否能开始、各自阻塞点和下一步需用户提供的条件。
  6. 更新 STATUS.md 与证据索引；由 ARCHITECT 按实际差异和测试独立审查后决定是否进入用户个人试用。
- **验收标准：** 唯一工作区身份与改动边界清楚；准入关闭时所有宿主启动/提交入口都 fail-closed 且有定向测试；runtime/、模型路由和凭证未改；三个合成场景可重复走完已实现的本地步骤，模型步骤明确标未验证；复杂分析未扩围；证据区分静态检查、离线运行和模型未验证；未提交、未推送。
- **风险：** 两个 worktree 均有大量不同未提交改动，误选或整体复制可能覆盖工作；闸门保持关闭会阻止模型生成，因此本阶段只能完成无模型的流程测试。用户转述的授权仅限特定条件下的个人数据分析与生态研判，不能推定周报、一般写作、项目管理或某个模型端点已获准。
- **停止条件：** 若无法指定唯一工作区、发现未覆盖的子进程启动入口或有任何未经允许的真实材料/端点调用路径，停止相关实现并报告，不得放开闸门。

**已完成并独立验收。** 模型调用与真实业务材料准入保持关闭。

## 最近完成计划

### OD-ARCH-012：补齐成果写入失败清理验证，关闭 OD-ARCH-011 验收

- **状态：已执行并经 ARCHITECT 独立验收通过（2026-10-18）。** 定向测试 45/0，完整 Rust 回归 lib 72 / host_integration 13 / e2e 3 全通过；前端构建与离线示例通过，`runtime/` 无差异。两次受控变异证明失败测试有效，源码已还原。证据见 `tests/verification/desktop/evidence/od-arch-012/`。未提交、未推送，不构成合并授权。
- **分类：** 缺失验收证据补齐与范围性复验。
- **目标：** 仅补足 OD-ARCH-011 要求的写入失败和刷盘失败清理证据，并完成该计划要求的回归；不顺带审计或合并其他 worktree，不实现会话恢复、旧会话导入或 Skill 加载。
- **影响范围：** 仅限当前主工作区成果写入器及其 Rust 测试、必要的状态/证据记录。不得读取旧会话或旧成果正文，回显凭证，提交/推送/打标签，修改 `runtime/`、模型路由或凭证，启动用户实例，或改动第二工作区。
- **实施步骤：**
  1. BUILDER 为保存写入器补最小测试专用故障注入，分别验证写入失败与刷盘失败；注入不得从 UI、环境变量、普通配置或生产请求触发。检查本次新文件清理结果、既有同名文件未改动、错误不含内容。
  2. 运行完整 Rust 离线测试、前端构建、离线场景示例、差异空白检查及 `runtime/` 无差异检查；任一失败则记录并停止，不扩大修复范围。
  3. BUILDER 仅更新 `STATUS.md` 和本计划相关证据索引；完成后停止，由 ARCHITECT 独立检查源码、测试和实际差异，再决定是否在 `REVIEW.md` 关闭 OD-ARCH-011。
- **验收标准：** 写入失败与刷盘失败各有可重复测试；失败时只清理由本次新建的半成品，既有文件不变；测试注入不能由 UI、环境变量或普通生产配置触发；计划要求的 Rust、前端和离线流程回归通过；`runtime/` 无差异；模型准入继续关闭；未提交、未推送。
- **风险：** 测试注入若进入生产可触发路径会制造绕行；写入失败后的文件删除本身也可能失败，测试和报告必须如实区分“清理成功”与“清理失败”，不得承诺超出文件系统保证的原子性。
- **停止条件：** 若需修改生产配置才能注入故障、发现疑似真实材料/凭证或准入绕行，立即停止相关步骤并报告；不得通过放宽准入、读取用户数据或修改 `runtime/` 完成测试。

**OD-ARCH-012 已关闭。双工作区保护盘点与 UI 评审尚待单独制定并获用户批准；本计划未授权审计第二工作区、合并、提交或推送。**

## 当前计划状态

### OD-ARCH-014：以主工作区为基线选择性整合 UI 外壳（已执行；有条件通过）

- **OD-ARCH-013 状态：** 只读审计已由 BUILDER 执行，并经 ARCHITECT 独立复核按范围通过（2026-10-18）。两边改动保持原样；整分支合并不通过。审查结论见 [REVIEW.md](REVIEW.md) 顶部，执行事实见 [STATUS.md](STATUS.md) 顶部。
- **分类：** 前端整合与产品体验验证。
- **状态：已执行；ARCHITECT 独立复核有条件通过（2026-10-18）。** 结论与执行偏差见 [REVIEW.md](REVIEW.md) 顶部；执行事实见 [STATUS.md](STATUS.md) 顶部。未提交、未推送、未打标签。
- **目标：** 保留主工作区安全闸门、离线场景、成果保存和现有宿主行为，选择性接入 UI worktree 的视觉外壳，使用户能在同一桌面窗口进入首页、任务/分析工作台、离线周报与项目流程、只读引擎诊断，并用构建、测试和窗口交互证明没有功能回退。
- **影响范围：** 仅在 `/mnt/data/Code/MyWorkStation` 主工作区实施；UI worktree 只读，作为文件来源参考。允许改动主工作区 `desktop/` 前端、锁文件、必要启动器和 UI 专项测试/证据，以及 `STATUS.md`、`REVIEW.md`。默认可引入 `theme.css`、`workbench-ui.css`、`components/`、`shell/` 中与现有视图相容的组件、自托管字体依赖和 Vite Tailwind 插件、前端统一提交提示。不得整体覆盖主工作区 `App.tsx`、`api.ts`、`types.ts`、`workbench.tsx`、Rust 宿主、Python 适配层、离线流程或成果保存实现。不得改 UI worktree、`runtime/`、准入常量、模型路由、凭证、真实材料或旧用户数据；不得提交、推送、打标签。
- **实施步骤：**
  1. 记录两工作区执行前 HEAD、状态和计划文件清单；确认交接文档已校正为旧后端基线。逐个选取文件，不做分支整体合并。
  2. 在主工作区接入视觉令牌、兼容组件、字体和构建插件，保留主工作区现有 IPC 契约与页面逻辑；不接入第二套首页引擎对话，避免重复维护任务提交、取消和终态状态。
  3. 接入新导航与视觉外壳，并保留任务工作台、分析工作台、离线流程和只读技术诊断入口；接入前端准入提示，宿主拒绝仍为最终边界。
  4. 比较既有启动器和 UI worktree 的 `scripts/dev-ui.sh`，只吸收经核验的隔离检查；明确唯一开发入口，不保留可能串载其他 worktree 页面或误停其他进程的路径。
  5. 保持端点准入关闭，运行完整前端构建、Rust 离线回归、离线流程评估，检查 `runtime/` 无差异和旧成果未受影响。
  6. 在隔离环境启动真实 Tauri 窗口，验证导航、离线周报/文案/项目流程、手动确认与保存、引擎拒绝提示；将截图和结果存入专项证据。BUILDER 完成后停止，等待 ARCHITECT 基于实际 diff 和运行证据审查。
- **验收标准：** 主工作区前端构建、Rust 测试、离线场景评估通过；窗口关键路径可交互；首页、任务、分析、离线流程和诊断入口均可到达且未丢失既有功能；引擎准入仍 fail-closed，界面拒绝与宿主一致；启动器不误停其他实例；`runtime/` 无差异；原有改动未被覆盖；仅用合成/公开数据；未提交、未推送。缺少运行证据的项目不得标通过。
- **风险：** 两边多个前端和宿主文件已分叉，整段搬运会回退主工作区修复；Tailwind 级联层可能影响旧样式；新增依赖可能需要在线安装；当前窗口证据仅适用于本机平台。
- **停止条件：** 交接文档仍误称已包含主工作区成果；出现准入放宽、离线保存逻辑丢失或端点配置变化；需改 runtime、路由或真实数据；启动器可能终止非本次启动的进程。停止并报告，不自行扩大范围。

### OD-ARCH-015：核对 UI 整合边界并收口共享状态（已独立复核通过）

- **状态：只读核对按范围通过（2026-10-18）。** 未改应用代码、UI 工作区、`runtime/`、用户数据或证据原件；未提交、推送或打标签。BUILDER 执行事实见 [STATUS.md](STATUS.md)；独立审查及限制见 [REVIEW.md](REVIEW.md)。取消按钮缺陷未在本计划内修复。

- **分类：** 只读审查与计划状态校准。
- **目标：** 不改业务代码，基于可用的 OD-BUILD-005 证据、当前源码与 OD-ARCH-014 执行记录，核对 `workbench.tsx` 引擎支路与既有手动/保存流程的边界；如缺少执行前快照，明确标为无法追溯，不猜测。完成 014 的偏差记录与共享状态校准，为之后安排用户合成数据试用或会话持久化工作提供可信基线。
- **影响范围：** 只读检查主工作区相关代码、当前 Git 差异、OD-BUILD-005 与 OD-ARCH-014 证据；只允许更新 `PLANS.md`、`REVIEW.md` 和必要的 `STATUS.md` 交接摘要。不得修改应用代码、UI worktree、`runtime/`、用户数据或证据原件；不得提交、推送、打标签；模型准入保持关闭。
- **步骤：** (1) 核对工作台引擎支路、手动解释、人工确认保存与准入调用点；(2) 对照现存 OD-BUILD-005 报告和测试，列明已证实及无法恢复的执行前行为；(3) 检查共享文档中 014 状态和停止条件记录一致；(4) 写入短审查结论，若发现实际功能回退或安全边界问题则停止并提出单独修复计划。
- **验收标准：** 代码事实与历史报告分开陈述；关键路径没有发现准入放宽或保存路径丢失，或明确列出具体问题；无法取证的事项明确标为未验证；共享状态一致；应用代码和用户数据未变。
- **风险：** 未保存的执行前工作区快照可能无法恢复，历史报告只能证明其记载范围，不能替代源码。
- **停止条件：** 发现端点闸门被放开、保存功能回退、疑似真实材料/凭证，或需要改代码才能继续核验时，停止并报告，另立修复计划。

- **执行结论（BUILDER，待独立复核）：**
  - 准入未放宽：前端与宿主两个 `ENDPOINT_ISOLATION_CONFIRMED` 均为 `false`；界面提交点只有 `workbench.tsx` 一处，仍经过 `api.ts` 的前端拒绝与宿主 `admission.rs` 的最终拒绝。
  - 人工确认与保存边界仍在：界面必须勾选确认且解释非空才可保存；宿主再次校验 `user_confirmed`、来源、统计指纹、仓库外路径和禁止覆盖。
  - 发现一项真实缺陷：`runEngineExplanation` 在提交后一直保持 `engineBusy = true`，轮询到终态或 180 秒超时才结束；取消按钮同时以 `engineBusy` 和“非终态”为可点击条件。结果是在运行时被前者禁用，运行结束后被后者禁用，界面上的取消按钮实际无法使用。底层取消能力存在（OD-BUILD-005 历史脚本记录过取消成功），缺的是界面触发点。该问题不是端点准入放宽，也不能证明是 OD-ARCH-014 引入的回退（执行前 HEAD 没有引擎解释支路）；需要单列小范围修复计划，本计划未改代码。
  - 无法逐行证明 OD-ARCH-014 未改变 `workbench.tsx` 原有业务逻辑：执行前未提交版本没有找到快照或 stash；现有 OD-BUILD-005 截图与报告只能证明引擎支路在 014 之前已经存在，不能替代逐行差异。
  - 角色分工冲突：本计划允许更新 `REVIEW.md`，但根目录 `AGENTS.md` 规定 `REVIEW.md` 是 ARCHITECT 的独立审查记录。BUILDER 未写 `REVIEW.md`，将核对结论记入 `STATUS.md`，由 ARCHITECT 决定是否补写审查。

OD-ARCH-015 已执行并独立复核通过。OD-ARCH-016 已由 BUILDER 执行完毕并停止，等待 ARCHITECT 独立复核；剩余两项工作「会话持久化」与「旧 DSH 会话导入」暂未排期。模型端点继续 no-go。

### OD-ARCH-016：修复工作台引擎任务取消入口（复核未通过，纳入 OD-ARCH-017 修正）

- **状态：BUILDER 已按批准执行完毕，等待 ARCHITECT 独立复核；审查通过前不宣称验收完成。** 用户本轮明确「批准 OD-ARCH-016 执行」。执行记录见 [STATUS.md](STATUS.md) 顶部，证据见 [tests/verification/desktop/evidence/od-arch-016/README.md](tests/verification/desktop/evidence/od-arch-016/README.md)。
- **分类：** 工作台可用性缺陷修复与取消状态验证。
- **目标：** 让正在执行的工作台引擎任务可以发出取消请求，并在正常完成、取消确认、取消未确认/超时等情况下如实显示状态；保持“请求取消”不等于“取消成功”的既有语义。
- **影响范围：** 仅限 `desktop/src/workbench.tsx`、必要的前端状态测试/测试夹具、专项运行证据，以及 `STATUS.md`。如发现必须改 `desktop/src/types.ts`、IPC 契约或 Rust/Python 适配层才能完成，应停止并报告，不在本计划内扩展。不得改 `runtime/`、准入常量、模型路由或凭证；不调用真实模型、不输入真实业务材料、不启动用户正在使用的实例；不得提交、推送或打标签。
- **实施步骤：**
  1. 记录执行前 HEAD、工作区状态及目标文件差异；检查现有取消阶段和任务状态契约，只针对取消按钮生命周期设计最小改动。
  2. 允许活动任务在运行中点击取消；避免重复取消请求，并保留任务轮询，以宿主回读状态为最终事实。不得让取消按钮因等待函数仍在运行而失效。
  3. 处理取消请求与正常完成竞争：若任务已进入终态，显示真实终态；只有宿主明确返回 `confirmed_cancelled` / `cancelled` 才显示取消成功；请求失败或超时须显示未确认，不能伪称成功，也不能覆盖已有解释内容。
  4. 用隔离的合成任务或确定性前端测试覆盖：活动任务可点击取消、重复点击受控、取消确认、任务先正常完成、取消未确认/超时、任务输出与现有解释状态不被误写。测试不依赖外部模型凭证。
  5. 运行定向测试及完整前端构建；若变更触及共享状态机或 IPC，再运行计划要求的 Rust 离线回归。检查端点准入仍关闭、`runtime/` 无差异、`git diff --check HEAD` 通过，保存精简证据后停止，等待 ARCHITECT 独立审查。
- **验收标准：** 运行中的任务可触发一次取消；取消与成功完成竞争时 UI 以宿主终态为准；超时/失败明确为未确认；原解释文本不被取消路径破坏；前端构建与定向测试通过；端点仍 fail-closed，`runtime/` 无差异，既有改动保留；无凭证/真实材料访问，无提交推送。
- **风险：** 取消 IPC 可能等待最多配置超时，UI 需保持可响应且避免重复提交；当前模型准入关闭，无法在产品端点上复验模型任务，只能用隔离合成任务验证状态交互。
- **停止条件：** 若需新增 IPC/通信层、修改引擎契约、开放端点或接触真实数据才能修复，立即停止并交由 ARCHITECT 重新规划。

## 已完成并独立审查

### OD-ARCH-011：修正离线流程的日期与保存边界，准备安全的个人合成数据试用

- **状态：已执行并按范围通过独立审查（OD-ARCH-011 主体及 OD-ARCH-012 失败清理补充项）。** 模型写作、真实工作材料用途、长期使用与跨平台体验仍未验证。
- **分类：** 离线场景可靠性修复与用户边界澄清。
- **目标：** 修正影响周记录/项目逾期结果的日历日期校验；为新保存成果明确 OmniDesk 的规范位置，同时保护旧 YvanAI 路径内所有现存文件；让使用者一眼看清当前仅可用合成、公开或明确获准内容，以及红线扫描的局限。
- **影响范围：** `desktop/src-tauri/src/task_shell.rs`、`desktop/src/OfflineFlows.tsx` 及所需定向测试/证据、`docs/decisions.md`、数据布局文档、`STATUS.md`。不得修改 `runtime/`、端点准入、模型路由、凭证或引擎通信；不得读取/迁移/覆盖/删除既有用户成果；不得提交、推送或打标签。若发现新路径会造成旧数据读写歧义，停止并报告，不自行迁移。
- **实施步骤：**
  1. 在不读成果正文的前提下，复核现有数据路径决策和代码默认值；明确只影响未来新文件的规范默认位置，旧路径维持原样。
  2. 修正日期解析，按实际月份天数及闰年规则拒绝非法日期；周记录、期限和判定日共用校验。覆盖闰年、非闰年、月末、非法日期及跨年比较。
  3. 在离线流程醒目提示：当前模型不可用；输入框不等于授权入口；只能输入合成、公开或已明确获准的内容；红线扫描不能可靠识别所有个人身份和工作细节。避免把启发式扫描描述成数据保护。
  4. 使用合成样例验证三个流程、日期边界和新成果保存路径；证明旧路径文件未被改动、保存不覆盖同名成果、失败时无半成品。不得调用模型或输入真实业务内容。
  5. 更新执行证据与 STATUS.md 后停止，由 ARCHITECT 独立审查。
- **验收标准：** 非法日历日期全部拒绝、合法闰日接受；逾期判断边界正确且可复现；新文件只写入明确的 OmniDesk 位置，旧成果未读取正文、迁移或变更；界面准确说明适用范围和扫描局限；Rust 测试、前端构建及计划要求的完整回归通过；模型闸门保持关闭。
- **风险：** 真实成果可能仍在旧目录；任何路径更改都不能隐式迁移或宣称资产统一完成。UI 文案只能降低误用，不能构成代码级授权或完整检测。真实工作场景的模型效果仍取决于后续有效授权及端点准入独立审查。
- **停止条件：** 若实现需要改写旧数据、开放模型调用、读取真实工作材料，或无法确认新保存路径不覆盖既有成果，立即停止相关步骤并交由 ARCHITECT 重新规划。

**已执行；BUILDER 停止并等待 ARCHITECT 独立审查。审查通过前不得据此计划宣称验收通过或进入下一阶段。**

## 历史计划与记录

> **历史状态快照（OD-ARCH-009 执行后）：** 当时其结论为 no-go、待独立审查；随后审查与主工作区复核已记录在 [REVIEW.md](REVIEW.md)。OD-ARCH-008 已在 BUILDER worktree 的计划范围内通过独立复核；构建重跑限制与剩余验证见 REVIEW.md。该段不代表当前计划状态。

## 执行结果（待审查）

OD-ARCH-009 已按下列条文执行完毕，交付记录见 [STATUS.md](STATUS.md) 顶部与 [REVIEW.md](REVIEW.md) 顶部。

- 结论 **no-go**：宿主无法据代码确认模型端点、工具或数据去向已限定在批准范围内；`ENDPOINT_ISOLATION_CONFIRMED` 保持 `false`。
- 已确认的旁路：子进程与 Python Adapter 继承完整宿主环境（`DEEPSEEK_BASE_URL`、`DEEPSEEK_API_KEY`、代理变量）；`OMNIDESK_DSH_PATCHES`、`OMNIDESK_DSH_CLI`、`OMNIDESK_DSH_PROFILE` 无二次校验。
- 已确认 DSH 内置 `web`/`web-search`/`web-fetch`/`bash`/`fs` 与遥测通道默认存在；ACP 请求的 `mcpServers: []` 只约束 MCP，不约束内置工具。
- 建议的最小 fail-closed 方向：宿主白名单环境 + `env_clear()`、显式端点与工具配置、patch 内容校验、回环与真实凭证互斥、无批准记录即拒绝，并配套环境继承/patch 注入/真实端点拒绝等测试。
- 下一步需由用户或单位提供端点、服务商、设备、运行环境与用途的书面证据，再决定是否立项实现端点守卫。本计划不授权该实现。

## 历史待批准计划

### OD-ARCH-009：审计端点准入所需证据并形成最小可执行方案（已审查）

- **状态：已执行并完成静态审查（2026-10-18）；no-go。未改代码、未开放准入。** 审查发现执行报告所述宿主拒绝闸门只存在于另一 worktree，当前主工作区没有该代码接线；详见 REVIEW.md 顶部。
- **分类：** 安全边界架构审计与方案决策，不是开放模型调用的授权。
- **目标：** 查清当前 DSH 引擎启动链中，端点、配置和子进程环境如何影响数据外发风险；明确什么可由宿主代码验证、什么仍需单位或用户提供依据；提出最小 fail-closed 准入方案，使后续是否启用本机合成测试或许可范围内个人分析有可审查依据。
- **影响范围：** 只读检查桌面宿主、Engine Adapter、启动配置与相关技术文档；交付限 STATUS.md、REVIEW.md、PLANS.md 和必要的 docs/decisions.md 记录。不得改 ENDPOINT_ISOLATION_CONFIRMED、模型路由、凭证、DSH Runtime、用户数据或运行实例；不得读取或输出秘密/凭证值、真实业务材料；不得新增准入旁路、调用模型、合并或提交。
- **实施步骤：**
  1. 沿代码追踪 Tauri 宿主到 Python Adapter、DSH 子进程的启动环境和配置来源，只记录路径、变量名与控制流，不读取配置/凭证内容。
  2. 列出可能改变数据去向的入口（模型端点、MCP/工具、配置继承、日志与任务保存），区分源码事实、可运行验证、用户/单位授权证据及未知项。
  3. 对照现有宿主拒绝测试，评估可核实的最小准入判定与无法由本机代码证明的前提；检查是否存在环境变量、普通配置、UI 或测试模式可绕行的风险。
  4. 只输出一个明确的 go/no-go 设计：准入事实来源、宿主强制点、默认拒绝条件、所需测试及启动前置条件。若证据不足，保持 no-go 并列出最少待补证据；不自行实施或开放入口。
  5. 更新执行记录和审查文档，明确方案不会扩大已获授权的用途、设备、模型服务商或环境范围。
- **验收标准：** 启动链路与潜在数据去向有代码路径依据；秘密值及真实业务内容未读取/记录；所有已知准入旁路得到检查；交付可供 ARCHITECT 作出下一阶段批准/否决的短方案；代码中的准入常量保持关闭，且没有任何模型调用。
- **风险：** DSH 可能从仓库外配置或继承环境加载端点/工具设置，单靠静态审查无法证明隔离；不得把“本机运行”或“脱敏”表述为端点已隔离或已获授权。此时结论应为 no-go，而不是放宽检查标准。

**完成后停止并交 ARCHITECT 审查；该计划本身不批准实现准入或调用模型。**

### OD-ARCH-008：端点准入关闭时阻止引擎描述命令启动子进程

- **状态：已执行，计划范围内通过；独立重跑限制见 REVIEW.md。**
- **分类：** 架构安全边界审查与最小生命周期修复。
- **目标：** `engine_describe` 当前会初始化并启动 DSH 子进程，且应用启动时会自动调用。端点准入固定关闭期间，应让窗口可正常打开并展示明确的“引擎未启动/端点未确认”状态；不得因读取能力描述而启动引擎。
- **影响范围：** `desktop/src-tauri/src/lib.rs`、必要的 `engine_bridge.rs` 或描述符类型、前端应用启动/技术诊断显示、宿主准入测试、OD-ARCH-008 隔离证据与 `STATUS.md`。不得修改 Runtime、端点常量、模型路由、凭证、IPC-1/IPC-2 既有字段契约（除非先在计划内说明兼容调整）、用户数据或其它运行实例；不得提交、推送、合并、打标签或清理工作区。
- **实施步骤：**
  1. 只读追踪所有 `engine_describe` 调用点及当前引擎生命周期，确认窗口初始渲染实际依赖的描述字段。区分纯静态能力元数据与必须通过启动子进程才能取得的运行时信息。
  2. 设计最小 fail-closed 行为：在宿主准入关闭时，启动/描述调用不得启动 Python/DSH 子进程，不得创建 DSH/Agent 数据根或任务状态；前端改为显示清晰的未准入状态。保留健康状态读取和安全关闭能力。不得加环境变量、UI 参数或普通配置旁路。
  3. 增加宿主层直接调用回归，证明拒绝时进程未启动、路径/数据状态未变化、返回错误可由 UI 识别；若可从静态数据返回描述符，证明该做法不虚报运行能力或可用状态。
  4. 只改应用启动与诊断所需的 UI 状态处理；不扩大为完整分区、端点许可机制或工作台重构。
  5. 运行 Rust 定向与完整离线测试、前端类型检查和构建、准入检查及必要的桌面启动/E2E 回归；更新证据和 `STATUS.md`，不触碰 Runtime/用户数据。
- **验收标准：** 准入关闭时，直接调用描述/启动相关宿主入口不启动引擎子进程，不创建数据目录或任务；UI 可正常打开并准确说明引擎不可用原因；健康读取及关闭不回归；无配置旁路；计划要求的定向与完整回归通过，未验证事项如实记录。
- **风险：** 当前前端可能假设 `engine_describe` 总能返回真实初始化描述符。若无法在不虚报能力、不改变既有协议语义的前提下实现安全空态，停止并提交选项供 ARCHITECT 重新规划，不自行放宽准入。

**完成后停止并交 ARCHITECT 独立复核；不自动进入提交或合并。**

### OD-ARCH-007：封闭工作目录准备命令的宿主绕行

- **状态：计划范围内通过，见 REVIEW.md 顶部 OD-ARCH-007 审查。**
- **执行证据：** `/mnt/data/Code/omniharness-ui/tests/verification/desktop/evidence/od-arch-007/`；独立定向复跑 3/3 通过。

### OD-ARCH-007：封闭工作目录准备命令的宿主绕行（原计划，已执行）

- **状态：已执行并通过计划范围审查；当前状态见本文件上方与 REVIEW.md。** 以下保留原计划条文供历史对照。
- **目标：** 当宿主端点准入关闭时，直接调用 `task_prepare_workspace_dir` 也必须在创建目录或触达其他数据前拒绝；保持默认拒绝与零副作用一致。
- **影响范围：** `desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/admission.rs`（或已有共享宿主闸门）、对应 Rust 测试、必要的前端错误呈现、OD-ARCH-007 验证证据与 `STATUS.md`。不触碰 `runtime/`、凭证、模型路由、真实业务材料或既有用户数据；不提交、推送、合并、打标签或清理工作区。
- **实施步骤：**
  1. 只读核对所有 Tauri 命令中会创建工作区/任务目录或启动引擎的入口，确认最小受影响范围；不要把只读状态查询或用户明确触发的保存操作无差别封锁。
  2. 让 `task_prepare_workspace_dir` 在调用 `prepare_local_data_root()` 前经过宿主准入；拒绝时返回清晰、无敏感输入的结构化错误，并确保不初始化引擎、不创建目录、不写任务或用户数据。不得添加环境变量、UI 参数或普通配置旁路。
  3. 增加直接覆盖宿主命令顺序的回归：使用不存在的一次性临时路径，调用拒绝路径后断言路径仍不存在、引擎未运行且无任务副作用。测试不得删除或改写既有用户数据。
  4. 确认前端在提交前准备工作目录的流程能正确显示拒绝原因，且不会把失败显示为成功；如宿主拒绝后该路径不可用，保持端点未获准时的预期禁用状态。
  5. 运行 Rust 定向与完整离线测试、前端构建及相关准入/E2E 回归；更新 `STATUS.md` 记录实际命令、结果和证据，留待 ARCHITECT 独立 Review。
- **验收标准：** 直接调用宿主工作目录准备命令时，拒绝先于文件系统副作用；断言目标目录不存在、引擎未启动且没有任务创建；无普通配置旁路；现有 UI 显示准确；相关测试通过；保留所有既有工作区改动，不提交、不推送、不合并。
- **风险：** 此修复会使当前端点未获许可时的 UI 引擎解释路径在准备目录步骤被拒绝，这是预期的 fail-closed 行为；不能为通过合成模型测试而提供生产绕行开关。

**完成后停止并交 ARCHITECT 独立复核，不自动进入提交或合并。**

### OD-ARCH-006：宿主提交默认拒绝、开发工作区隔离和状态文档校准

- **状态：OD-ARCH-006 与 OD-ARCH-007 的拒绝边界修复已通过各自计划范围审查；应用启动时引擎描述仍会拉起子进程，后续审查计划见 OD-ARCH-008。**

### OD-ARCH-006：原批准计划（已执行，审查暂缓验收）

- **状态：已执行；提交拒绝、端口隔离和架构文档校准有执行证据。目录准备绕行由 OD-ARCH-007 修复；应用启动时引擎描述的生命周期问题列入 OD-ARCH-008。** 本节保留原计划内容供对照。
- **目标：** 在端点隔离事实缺失期间，让 Rust 宿主也默认拒绝 UI 引擎提交；防止并行 worktree 的桌面窗口误加载另一工作区的前端；校准过时的桌面架构现状说明。
- **影响范围：** `desktop/src-tauri/src/lib.rs` 及对应 Rust 测试、必要的前端错误提示、`scripts/dev-ui.sh` 及其隔离测试、`docs/OmniDesk-Desktop-Architecture.md`、`STATUS.md`。只允许合成数据与隔离临时目录。不修改 IPC 形状、模型路由、凭证、`runtime/`、真实业务材料或用户数据；不提交、推送、合并、打标签或清理工作区。
- **实施步骤：**
  1. 在 Tauri `engine_submit_task` 命令的宿主入口增加 fail-closed 检查：目前没有宿主可核实的端点许可证明，故应用命令必须在初始化/提交引擎任务前拒绝，并返回清晰、非敏感的原因。不得新增可由环境变量、UI 参数或普通配置打开的绕行开关。专用合成回归脚本是否仍能直接验证引擎链路，先检查现状，避免为测试留下生产绕行口。
  2. 增加宿主层回归：直接覆盖命令拒绝路径，并证明未启动/调用引擎、未创建任务或写入用户数据；确认所有 Tauri 暴露的提交命令都受同一拒绝规则。前端提示应与实际宿主行为一致。
  3. 修复 `dev-ui.sh` 固定开发端口冲突：若 `127.0.0.1:5273` 已被非本次启动器拥有的服务占用，启动前清楚失败，不启动 Tauri 窗口、不向占用服务发送信号，也不复用其页面。用两个隔离临时 worktree/受控服务复现并验证；不得停止用户的其他实例。
  4. 仅根据现有代码和已运行检查，更新桌面架构文档中“工程尚未创建/工具链缺失”等已过时陈述；区分当前 Linux 实测、其他平台未验证和端点未确认。
  5. 跑完定向测试及计划要求的完整回归；将命令、通过数、证据路径、失败/未验证项写入 `STATUS.md`，不把静态检查写成运行验证。
- **验收标准：** Rust 宿主直接调用提交命令时也 fail-closed，且没有普通配置旁路；拒绝前不启动引擎或产生任务副作用；端口被其他工作区占用时启动器安全退出且不碰占用进程；文档准确反映现状；Cargo 离线测试、前端构建及准入/启动器/E2E 相关检查通过，或逐项给出可复核的环境阻塞；所有既有改动保留。
- **风险：** 所有桌面引擎提交（包括合成端点的 UI 手动路径）都会保持关闭，直到另行设计并验证宿主可核实的端点许可来源；这是当前授权事实不足下的预期行为。固定端口策略意味着两个 worktree 不能同时启动桌面 UI，但会明确提示而不是串载。
- **停止条件：** 若无法在不改变 IPC 形状、不增加端点/路由或凭证的前提下做宿主拒绝，停止并报告；若现有测试依赖可由普通构建打开的提交开关，不得自行加白名单或弱化拒绝。

## 已完成并复核的近期计划

### OD-ARCH-005：封闭技术切片自由文本绕行并校准启动器身份核验

- **状态：** OpenCode worktree 的计划范围内通过；宿主提交入口仍未强制拒绝，见 REVIEW.md。证据在 `tests/verification/desktop/evidence/od-arch-005/`（OpenCode worktree）。

### OD-UI-002：独立审查工作台视觉改造

- **状态：** OpenCode worktree 的计划范围内通过；截图与证据在 `tests/verification/desktop/evidence/od-ui-002/`（OpenCode worktree）。未验证跨平台和真实模型输出。

### OD-ARCH-005：封闭技术切片的自由文本提交绕行并校准启动器身份核验

- **状态：待用户批准；批准后由用户指定 BUILDER。**
- **目标：** 确保所有可调用引擎的 UI 入口都受同一准入边界保护，并使启动器实际身份检查与其报告一致。
- **范围：** `desktop/src/shell/EngineTechSlice.tsx`、必要的现有前端准入组件/测试、`scripts/dev-ui.sh`、OD-ARCH-004 证据说明、`STATUS.md`。不改变 IPC-1，不增加新的模型端点或配置，不触碰凭证、runtime、真实业务材料、用户数据或其他运行实例。
- **步骤：**
  1. 技术切片改为只读诊断，或复用与首页完全相同的提交准入门槛；未得到宿主可核实的端点隔离事实时，自由文本输入与提交均禁用，并在处理函数内二次拒绝。
  2. 审计前端全部 `host.submitTask` 调用点，证明每条 UI 路径都受相同限制；合成测试仍可通过专用验证脚本调用引擎，不需保留面向用户的绕行入口。
  3. `dev-ui.sh` 停止进程时核验记录中的命令身份，或删去“命令行已验证”相关记录并使用更强且可测试的身份核验；严禁身份不符时发送信号。
  4. 校准执行报告与代码实际行为；补测技术切片禁用/二次拦截与启动器身份不符场景。运行必要前端构建和既定回归，受环境限制的检查如实登记。
- **验收：** 所有 UI 提交入口在未获准时禁用并有处理层二次拦截；扫描无未受保护的 UI 提交调用；启动器不会停止命令身份不符的进程；报告与代码一致；保留全部现存工作区改动，不提交/推送/打标签。
- **风险：** 技术切片将不能再手工发起任务；这是端点准入事实缺位时的必要限制。若实现需要新增宿主端点证明协议，停止并报告，另立架构计划。

### OD-UI-002：独立审查 OpenCode 工作台视觉改造

- **状态：待用户批准；可由用户交 OpenCode 执行。** 这不是代码合并授权。
- **目标：** 在不碰业务流程的前提下，验证并校准「本地数据分析」工作台与首页的视觉一致性及可读性。
- **范围：** 仅 `desktop/src/workbench.tsx` 的样式挂载点与 `desktop/src/workbench-ui.css`，以及新增的截图/简短证据说明。不得改五阶段状态、文本/数字处理、IPC、Rust、准入逻辑、共享设计令牌或既有首页样式。
- **步骤：**
  1. 在可写构建环境验证 `npm run build`；若当前依赖目录只读，不删除或重装依赖，改用仓库既有可写构建方式或报告阻塞。
  2. 以合成样例启动隔离窗口，切换到分析工作台并留截图；检查中文对比度、页面滚动、窄窗口、统计表格、错误/禁用提示及五阶段入口可见性。
  3. 必要时只调整局部 CSS；确认未隐藏边界提示、数字单位、来源说明、失败信息和人工确认门槛。
  4. 记录改动文件、构建结果、截图与未验证项。
- **验收：** 构建通过；窗口截图证明工作台视觉清晰且局部样式不泄漏到首页；五阶段行为与 IPC 不变；仅合成数据；不提交、不推送、不合并。
- **风险：** 字体回退与 Tauri WebView 在其他平台的呈现未必一致；本项只验当前 Linux，不作跨平台承诺。

### OD-ARCH-004：收紧 UI 输入边界并修复开发启动器（已执行，部分验收）

- **状态：已执行；ARCHITECT 独立复核发现遗留绕行，未整体验收。** 实施证据见 STATUS.md 和 `tests/verification/desktop/evidence/od-arch-004/`；审查结论见 REVIEW.md。
- **分类：安全边界修正 + 可靠性修复；不改产品方向。**
- **目标：** 在决定是否保留 UI v2 前，确保首页对话不会让人误以为可输入任意工作内容；开发启动器仅能安全使用隔离合成环境；随后独立验收 UI v2 与 OD-BUILD-005。
- **影响范围：** HomeChat.tsx、必要的提示文案/组件；dev-ui.sh；针对准入提示、轮询故障和启动/停止行为的定向测试或验证记录；STATUS.md。不得触碰 runtime/、真实材料、模型路由、凭证、上游或其他运行实例；不得提交、推送或打标签。
- **实施步骤：**
  1. 在首页对话入口清楚显示当前用途及准入限制，明确仅供公开/合成内容；说明用户转述的有限许可不覆盖一般对话、写作或项目管理。不得把文案冒充代码级隔离；若无法确认本入口实际使用模型/运行环境边界，则默认禁用提交，先只保留 UI 预览。
  2. 修复对话状态与结果轮询的静默吞错：有限重试/超时后显示可理解的失败状态和恢复动作，不伪报成功、不无限显示生成中。
  3. 修订 dev-ui.sh：移除会写入仓库 runtime/ 的软链接回退；不得默认复用不明来源的 5273 服务；PID 文件须按运行目录独立、记录进程身份并在停止时验证归属，避免覆盖或误杀其他实例；仅保留隔离合成端点路径，移除或拒绝 --real 以免绕过边界。脚本失败时应明确停止，不留下半启动实例。
  4. 对 UI worktree 与主工作区分别核对差异；仅在有可写构建目录的条件下运行前端构建。独立重跑合成引擎路径和 Tauri 窗口验收；若受环境限制无法运行，标记未验证，不以另一 worktree 的产物替代证明。不得因验证失败直接清理或重建既有 node_modules、运行时或工作区。
  5. 将 UI v2、OD-BUILD-005 引擎切片、边界限制及全部未验证项分开记录；保留未跟踪设计资产，不删除、不擅自纳入提交候选。
- **验收标准：**
  1. UI 明示准入范围和边界；没有被确认的分区与端点执行隔离时，不允许把通用输入提交模型。
  2. 轮询中断与结果读取失败都有可见、有限、不会误导的结果；构建通过。
  3. 启动器的成功、端口占用、重复启动和停止场景有隔离验证；不修改 runtime/、不覆盖他人 PID 文件、不停止非本任务进程、不连接真实端点。
  4. Rust 回归、前端构建及合成 E2E 按可运行条件重跑；证据逐项注明是新复跑还是既有报告。窗口内路径有新证据；若环境不能运行，明确列为未验收项。
  5. 保留所有工作区既有差异；不提交、不推送、不打标签；审查结果仅由 ARCHITECT 在 REVIEW.md 独立记录。
- **风险：** 为安全起见，首页对话可能暂时只能展示而不能提交，直到确认模型和运行环境适用范围；启动器变更会影响开发体验，故须覆盖端口冲突和停止场景。UI 设计稿与应用素材的版权/入库决定仍未解决，本计划不删除或擅自提交这些资产。

**本计划完成后停止并交 ARCHITECT 复核；不自动进入提交/合并。**

### OD-BUILD-005：把已实现的引擎链路接进日常任务流程（第一个能真用的纵向流程）

- **状态：已获批并实施；ARCHITECT 审查暂缓整体验收。** 原计划如下留存供对照，当前缺陷与复核条件见 REVIEW.md 顶部及 OD-ARCH-004。
- **分类：功能实现（既有能力的接线与可用性），非架构变更。**
- **立项依据（均为已核实事实）：**
  1. 引擎链路本身已通：`engine_submit_task` → Python `serve.py` → ACP → DSH，隔离桌面 E2E chain（提交/失败/取消）已通过。证据等级：script。
  2. 但**任务外壳 UI 从不调用引擎**：`desktop/src/workbench.tsx` 只使用 `shell.*` 本地命令（统计、提示词、保存、使用记录），全文未出现 `host.submitTask`；引擎调用只存在于技术切片视图 `App.tsx` 的事件流面板。证据等级：static。
  3. 后果：用户看到的「无法选数据、无法下指令、看不到结果」不是能力缺失，而是**已实现的能力没有连到日常流程上**；模型解释目前只能靠人工复制提示词、再把结果手工贴回来。
  4. 与用户明确的首要目标一致：先做到「我自己愿意天天用」，再谈后续扩展。
- **目标：** 让作者在桌面应用内，用合成/公开/已批准数据走完一次完整任务：输入 → 确定性统计 → 由应用发起引擎解释 → 人工确认 → 本地保存；失败或未配置端点时明确报错，不出现假成功。
- **影响范围：**
  - `desktop/src/workbench.tsx`（在「阶段 3 · 解释」接入引擎调用与结果回填）。
  - `desktop/src/api.ts`、`desktop/src/types.ts`（仅在确需新绑定时）。
  - `desktop/src-tauri/src/lib.rs`与`task_shell.rs`（仅在需要把已冻结的提示词交给引擎时；优先复用现有 `build_explanation_prompt`，不新建提示词来源）。
  - 桌面测试与验证脚本（`desktop/src-tauri/tests/`、`tests/verification/desktop/`）及其证据。
  - `STATUS.md`。
  - **不得**修改 `runtime/`、模型路由、凭证、DSH 数据或任何真实业务材料；不得改变 IPC-1 契约形状。
- **实施步骤：**
  1. **确认落点（只读）**：核对 workbench 阶段 3 的状态机（`analysis` 指纹、`promptText`、`explanation`、`source`、`confirmed`），确定唯一插入点；保持「输入变化即失效下游产物」的现有约束。
  2. **最小接线**：在阶段 3 增加一个「用引擎生成解释」动作，把 `build_explanation_prompt` 产出的**冻结提示词**提交给引擎（`intent` 用该任务类型标识，`workspace_dir` 指向隔离数据根），订阅事件，任务到达终态后把 assistant 文本回填到解释框；`source` 仍记为 `model_unverified`，不得引入「已复核模型」来源。
  3. **失败与无端点路径**：未配置凭证/端点不可用时按现有错误语义明确呈现（复用 `normalizeError`）；保留现有「生成提示词 → 手动粘贴」路径作为回退，两条路径并存且互不污染状态；超时与取消复用现有 `cancelTask` 语义，不得把「已请求取消」写成取消成功。
  4. **证据（关键）**：用**本机回环合成端点**（不接真实模型、不用凭证）跑通完整路径并留证据：输入合成样例 → 统计 → 引擎解释回填 → 人工确认 → 保存；随后在**真实 Tauri 窗口**里重做一次并留截图。若窗口路径走不通，如实记录阻塞，不降低标准。
  5. **回归**：`cargo test --offline`、`npm run build`、桌面 E2E chain 全部重跑；确认 workbench 仍只经 IPC-1，界面代码不出现 ACP/DSH 私有概念。
  6. 更新 `STATUS.md`：按文件事实 / 静态检查 / 脚本运行 / 真实模型（预计仍为未验证）/ 未验证 分级记录。
- **本计划明确不做（附理由）：**
  - 不做任务列表、项目管理、周报等 `reserved` 类型——尚未授权且未实现。
  - 不做本地文件选择器（打开磁盘上的 CSV）。它会引入新的 Tauri dialog 能力与安全边界评审，属独立决策；**最适合作为 OD-BUILD-006 候选**，本计划先用粘贴与内置合成样例。
  - 不做会话持有、追加日志、备份恢复演练（v0.4 议题，另有触发条件与审计结论）。
  - 不缩短非 ACP Profile 的 30 s 等待（裁决见 REVIEW.md）。
  - 不把「已实现任务类型」从 1 个扩展为多个。
- **验收标准：**
  1. 在真实 Tauri 窗口内，用合成样例可走完「输入 → 确定性统计 → 引擎解释 → 人工确认 → 保存」，每步有可追溯运行证据（脚本输出与截图）。
  2. 统计数值仍**只**来自 `task_run_analysis`；解释阶段保持指纹比对，不得重算或补齐数字。
  3. 无端点/失败时界面明确显示原因，不显示成功；解释来源下拉不出现任何「已复核」选项；未人工确认不得保存。
  4. workbench 与前端代码不新增对 ACP、DSH 私有格式或 DSH 配置的直接依赖；IPC-1 形状不变。
  5. `cargo test --offline`、`npm run build`、桌面 E2E chain 全部通过；`git diff --stat -- runtime/` 为空；既有未提交差异全部保留，未提交、未推送、未打标签。
- **风险：**
  - 第一次在窗口内接引擎会暴露事件订阅、终态判定与状态时序问题（流式文本与状态卡刷新竞争）。范围止于「解释」这一步；出现时序问题单独记录并最小修复，不顺手重构 workbench。
  - 若本机没有可用模型端点，本次只能验证到「提示词 + 手动粘贴」并如实标记模型段未验证；仍是有效进展，不得为凑结论而调用未获批准的端点。
  - 合成端点通过不等于真实模型输出质量合格；质量与合规仍按 GOALS.md 与 REVIEW.md 的限制另行判断。

**原计划已由用户批准并执行；不构成 OD-ARCH-004 的批准。**

## 最近执行计划

### OD-BUILD-004：闭合引擎 Profile 启动与错误验收

- **状态：已通过（计划范围）。** ARCHITECT 独立复核结论见 REVIEW.md 顶部同名章节。执行事实见 [STATUS.md](STATUS.md) 顶部。
- **实施结果摘要：** 缺口 1 由新增单测 `engine_config_default_profile_chain_is_acp` 直接以 `EngineConfig::from_env()` 证明默认链产出 `acp`（`child_env()`、`initialize_params()` 均传递），不依赖脚本注入；缺口 2 在 `acp_adapter.py` 的 TIMEOUT 分支写入所配 Profile、预期协议（ACP）与可读原因（落于 `detail`，因 `serve.py` 的 JSON-RPC `message` 字段被用作错误码），并与“Profile 不存在”错误可区分。等待时长按计划**未缩短**。
- **已裁决：** 启动前预检不作为当前验收阻断项，登记为低优先级后续项，理由见 REVIEW.md。

### OD-BUILD-004 原计划（原始条文，保留供对照）

- **分类：功能修复与验收补强（引擎启动配置）。**
- **目标：** 证明桌面宿主不设置 OMNIDESK_DSH_PROFILE 时，实际传给引擎的默认值为 acp；当显式配置了不能讲 ACP 的 Profile 时，在有界等待后返回包含 Profile 名称和预期协议的清楚错误，而不是只显示通用超时。
- **影响范围：** desktop/src-tauri/src/engine_supervisor.rs、与宿主配置/启动相关的必要测试、Profile 场景验证脚本及其证据、STATUS.md。确需改引擎错误映射时，只限现有 engine/omnidesk_engine/ ACP 初始化错误路径。不得修改 runtime/、DSH 数据、模型路由或凭证；不开发 GUI、不新增协议、不扫描或依赖 DSH Profile 内部 bundle 清单；保留现有未提交改动，不提交、不推送、不打标签。
- **实施步骤：**
  1. 增加隔离、可重复的宿主配置测试，直接通过 EngineConfig::from_env() 构造配置，并验证 child_env() 与 initialize_params() 都传递 acp；测试不依赖手工注入 acp，也避免并行测试争用全局环境变量。
  2. 检查当前 ACP 握手超时和子进程退出信息如何呈现。采用最小方案，让非 ACP/无响应 Profile 的失败信息指出所配 Profile、预期 ACP，并在现有有界超时后结束本次启动；只有在不影响正常 ACP 启动的证据支持时才缩短超时。不通过读取 DSH Profile 私有结构来预判。
  3. 用隔离数据根验证：默认 Profile 完成 ACP 初始化与 session/new；未知 Profile 和已存在但非 ACP 的 Profile 都得到可区分、可读的错误；确认子进程退出且不会遗留服务。
  4. 重跑相关 Rust 测试、Profile 场景及前端构建。E20 最近一次记录显示测试期间 `~/.dsh` 有原版 DSH 的后台写入且留有临时场景目录；若需重跑 E20，必须区分测试自身痕迹与并发后台变化，不得将其表述为全局零变化，也不得清理用户数据或旧临时目录。若隔离边界代码未变，可引用当前 E20 证据，避免无必要重跑。
  5. 更新执行记录和审查记录时，将较早章节中“待修复/待批准”的状态明确标注为当时的历史状态，保留原始审查事实，确保当前结论只以文档顶部的最新审查为准。
- **验收标准：**
  - Rust 宿主默认配置链经直接测试证明为 acp，不靠测试脚本手动传入该值；隔离运行中默认启动可完成 ACP 初始化和会话创建。
  - 未知 Profile 与非 ACP Profile 都不返回成功；后者失败时的错误说明所配 Profile 与 ACP 预期，并且启动子进程已结束。等待时间有上限；若调整上限，默认 acp 的实测启动必须证明不会被误判。
  - 原有 cwd 规范化代码及 R3/R5 验证保持通过；不引入 DSH 私有格式依赖。
  - runtime/ 与用户数据根无本轮痕迹；保留所有既有差异。
- **风险：** 缩短 ACP 握手等待可能误伤启动较慢的有效 Profile；需用默认 acp 的隔离真实启动回归证明新判断不会误报。跨平台行为仍不由 Linux 测试覆盖。
- **验收后：** ARCHITECT 根据实际 diff 与测试独立复核；本计划获批后仍由用户指定 BUILDER 执行。

## 已完成计划

### OD-BUILD-003：引擎 Profile 默认与 cwd 规范化最小修复

- **状态：实现完成；ARCHITECT 部分通过。** 具体审查见 REVIEW.md 顶部。
- 以下为**当时**状态，其余项已由 OD-BUILD-004 闭合：cwd 规范化修复由 R3/R5 和源码检查支持；默认 Profile 的源代码设置正确，但当时 R1 手动注入 acp 并显式传入该值，未验证 Rust 宿主默认配置链；R2 显示未知 Profile 可读报错，但已存在的非 ACP web Profile 约 30 秒后以通用超时失败，未满足原清晰报错标准。
- **后续：** 两项剩余缺口已由 OD-BUILD-004 聚焦闭合；Finding B、会话正文独立日志和桌面产品化不在范围内。

### OD-ARCH-002：真实 Tauri 窗口的合成数据任务路径验证（暂缓）

- 状态：未批准、未执行；因 v0.4 将会话持有与故障根因审计前移，暂缓至 OD-ARCH-003 后重新判断。
- 原目标：用合成数据验证真实 Tauri 窗口的输入、分析、解释和保存边界。
- 暂缓不构成通过或否决；原计划细节仍保留在本节之后供后续恢复。

### OD-ARCH-002 原计划（暂缓）

- **状态：待用户批准；未授权执行。**
- **目标：** 在不接触真实业务材料或凭证、不改变模型路由的条件下，校准项目当前状态记录，并取得真实 Tauri 窗口中使用内置合成样例完成任务外壳流程的可复核证据，为后续是否进入严格限定的个人自用试点提供依据。
- **影响范围：** `README.md`、`GOALS.md`、`docs/decisions.md`、`STATUS.md`、`REVIEW.md`、`PLANS.md`；必要时只读检查 `desktop/` 配置与 UI。不得修改 `runtime/`、凭证、真实用户数据或 DSH 原有数据。不得把本计划扩展成新功能或产品化开发。
- **实施步骤：**
  1. ARCHITECT 已在本轮将 `README.md`、`GOALS.md`、`docs/decisions.md`、`REVIEW.md` 和 `PLANS.md` 的当前状态校准，并保留 OD-BUILD-001/R4 历史审查。BUILDER 只需检查 `STATUS.md` 与其他入口无现行冲突；不得改写或删除历史执行事实。
  2. 静态核对 Tauri 启动配置、任务工作台状态转换、解释/保存桥接和应用数据根设置；只记录配置来源与当前默认行为，不读取密钥或凭证值。
  3. 在隔离的临时应用数据根中用**内置合成样例**启动真实 Tauri 窗口，记录从输入、确定性分析、解释提示词、人工确认到保存的实际可行路径。只有当现有模型端点已明确获准用于该次合成数据验证时才调用模型；否则停在提示词预览/人工输入路径，不读取凭证、不调用外部模型、不改路由。
  4. 核对产物落点在仓库外、保存确认按预期生效、使用记录不含输入输出正文；清理仅限本次创建且路径明确的临时测试数据。
  5. 根据证据更新 `STATUS.md`、`REVIEW.md` 和共享状态入口，作出“可进入何种受限个人试用 / 尚不能进入”的明确结论，并列明用户授权摘要不能覆盖的范围。
- **验收标准：**
  - 当前状态入口对 OD-BUILD-001、OD-BUILD-002 和 MVP 验收状态没有相互矛盾的现行陈述，历史审查保留且清楚标为历史。
  - 真实 Tauri 窗口路径有可追溯运行记录及截图或等价证据；不以 Vite 浏览器截图代替。模型调用是否执行、端点是否已获准必须单独记录。
  - 保存位置、人工确认、非覆盖行为及使用记录数据范围均由实际代码/运行证据支持；任何未完成模型步骤均如实标为未验证。
  - 全程不读取/打印凭证，不调用真实业务材料，不改变路由，不触碰 DSH Runtime 或原有用户数据。
  - 最终审查只批准被证据覆盖的合成数据或许可范围内个人数据分析与生态研判路径，不推定分发、周报/材料写作/项目管理、特定模型服务商、工具、设备或环境已获授权。
- **风险：** 当前桌面切片可能无法在真实窗口内完成端到端任务；模型端点授权也可能不足以执行真实调用。此时只记录已验证的本地流程和阻塞，不临时扩展实现或更改路由。桌面截图只能证明界面与流程，不证明模型输出质量或合规授权。

**批准后由用户指定 BUILDER 执行；当前 ARCHITECT 不直接实施。**

## 已完成计划记录

### OD-ARCH-003：v0.4 架构校准与会话丢失机制审计

- 状态：已执行完成（只读审计 + 隔离一次性合成验证）。结论与证据见 [REVIEW.md](REVIEW.md) 顶部同名章节。
- 交付：会话丢失六类归因；原生持久化能力确认（历史可读、可列出、可 resume、恢复后可持续写入）；独立逐消息追加日志维持 **no-go（暂缓）** 并给出采用条件；登记 Finding A（默认 Profile 非 ACP）与 Finding B（分析功能对人不可用），**只登记不修复**；v0.4 边界校准（V4 已发布格式 → DSH 内部状态非可丢弃；合规政策须绑定下层）。
- 边界：未扩大用户转述授权；未运行真实模型；未修改 Runtime、业务代码或用户数据。
- 后续：OD-BUILD-003 已获批并完成实现；独立审查部分通过，补充要求与下一项计划 OD-BUILD-004 见本文件顶部。

### OD-BUILD-002：拒绝桌面应用数据根落入仓库

- 状态：已实施，ARCHITECT Review 通过。
- 目标：在 `AppPaths` 数据根解析和目录创建阶段拒绝仓库内路径、指向仓库的符号链接和仿真禁区，同时允许干净的仓库外路径。
- 实施文件：`desktop/src-tauri/src/app_paths.rs`、`STATUS.md`；无须修改 `lib.rs`。
- 独立验证：本轮 `cargo test --offline` 60 项通过；`npm run build` 通过。checkpoint 的 `git diff --check HEAD^ HEAD` 有历史行尾空白/末尾空行问题，详见 `REVIEW.md`。
- 限制：常规路径校验不能消除检查与创建之间的并发替换竞态；当前仅在 Linux 实测，不代表跨平台已验证。

### OD-BUILD-001：个人 MVP 保存确认、来源真实性与输入处理边界

- 状态：已实施；独立 Review 中 R1–R3 通过，R4 经 OD-BUILD-002 补齐后通过。
- 详细结论保留在 `REVIEW.md`，执行事实保留在 `STATUS.md`。

### OD-ARCH-068：平台中立规则资产化（红线/铁律迁出代码 + 随手记轻装 + 隐私脱敏）（已执行完毕，ARCHITECT 复核通过，2026-10-23）

**批准依据：** 用户明确本轮定调——OmniDesk 是通用个人工作台平台（DSH 底座 + 自有 UI），纪检内容只属于 Discipline Profile；装 profile = 纪检工作台，不装 = 纯净底座。当前两处业务规则硬编码在平台代码里（红线词、写作铁律），违反平台中立；同时用户实测反馈：随手记不需要铁律/章节（应快）、身份证/手机号不应拦截（应自动脱敏后放行）、思考等级按场景默认。

**目标：**
1. 平台代码零业务内容：红线词表与写作铁律从 Rust/TS 硬编码迁出，写入 Profile 规则资产文件；无 profile 时平台为纯净底座（零红线、零铁律），行为与官方 DSH-desktop 无实质区别。
2. 隐私策略调整（用户明确）：身份证号、手机号不再拒绝提交，改为自动脱敏（替换 * 号，保留前 3 后 2 位）后放行；界面提示「已脱敏」；本地线程原文保留不变；纪检关键词（案件/线索/立案等 16 词）仍拦截不变。
3. 随手记轻装：工作记录场景（weekly）不经 buildDraftPrompt 的铁律/章节组装，改为一句轻提示词（整理指令），思考档默认 off；出周报才带完整规则+铁律+章节+本周已归档条目，思考档默认 low。
4. 思考档按场景默认：场景配置文件增加 `reasoning_effort` 字段；用户手动选档优先于场景默认；清除 `~/OmniDesk/app/config/omnidesk-model.yml` 遗留的 `reasoningEffort: high`（065 用户追加项，现应回归场景控制）。
5. Profile 规则资产用户可编辑：设置页「对话分类」或新增「规则资产」分页中可查看/编辑红线词表、写作铁律、场景规则；保存后写入 profile 规则文件（不重启）；文件缺失或为空时平台零规则放行（纯净底座行为）。

**影响范围：**
- Rust：`desktop/src-tauri/src/task_shell.rs`（红线词表外移）、`desktop/src-tauri/src/flow_pack.rs`（如果铁律在此）
- TS：`desktop/src/aiDraft.ts`（铁律迁出）、`desktop/src/shell/HomeChat.tsx`（场景默认思考档、脱敏提示）、`desktop/src/conversationPrompt.ts`（轻提示词路径）
- Python：`engine/omnidesk_engine/task_core.py` 或 `web_api_adapter.py`（如引擎侧还有规则逻辑）；`desktop/src-tauri/src/lib.rs`（配置加载透传）
- 配置：`~/OmniDesk/app/config/offline-flows.v2.json`（新增 reasoning_effort + 脱敏策略 + 红线词表 + 写作铁律字段，v1 迁移）、`~/OmniDesk/app/config/omnidesk.model.yml`（清理 reasoningEffort 遗留）
- Profile：`~/OmniDesk/app/profiles/discipline.rules.json`（新文件：global.writing_rules、global.redline_keywords、scenes.weekly.rules/sections/thinking、scenes.writing.*、scenes.project.*）
- 文档：`docs/decisions.md` 新增 D-028「平台中立与 profile 规则资产化」；`GOALS.md` 若有平台定位表述则校准；PLANS/STATUS/REVIEW

**Profile 规则资产布局（用户可编辑，文件不存在时=纯净底座零规则）：**

```json
{
  "schema": "omnidesk.profile-rules.v1",
  "profile": "discipline",
  "global": {
    "writing_rules": ["铁律 1", "铁律 2", ...],
    "redline_keywords": ["案件", "线索", ...],
    "privacy": {
      "id_card": "mask",        // mask=脱敏后放行；reject=仍拒绝
      "phone": "mask"
    }
  },
  "scenes": {
    "weekly":  { "rules": [...], "sections": [...], "thinking": "low"  },
    "writing": { "rules": [...], "thinking": "low" },
    "project": { "rules": [...], "thinking": "off" }
  }
}
```

**步骤：**
1. **S1 规则资产定义与迁移：** 定义 `omnidesk.profile-rules.v1` schema；创建 `discipline.rules.json` 并把现有 16 红线词、7 条铁律、weekly/writing/project 的现有 rules+sections 全部迁入；平台代码里的对应硬编码改为「读此文件，无文件=零规则」；v1 offline-flows 仍保留（它管的是周报章节结构等可编辑流配置），与 profile-rules 分工——offline-flows 管「周报怎么组装」，profile-rules 管「什么不能说 + 什么规则必须带上」。
2. **S2 隐私脱敏：** 身份证/手机号识别正则已存在（task_shell.rs 已区分 id_card/phone_number/case_keyword 三类），把这两类的动作从「拒绝」改为「掩码后放行」：保留前 3 后 2 位（如 `310*********45`），中间用 `*` 填充；掩码在引擎层执行（不动本地线程原文），界面 toast/批注提示「已自动脱敏」；`privacy` 字段允许用户改回 reject；case_keyword 行为不变（仍拒绝）。
3. **S3 随手记轻装 + 场景思考档：** conversationPrompt.ts 新增「轻提示词」路径：场景=weekly 且模式=随手记时，不带铁律/章节/已归档条目，只带一句整理指令；HomeChat.tsx 场景默认思考档（weekly=low、writing=low、project=off），用户手动选档优先；清除 omnidesk-model.yml 遗留 `reasoningEffort: high`（该项已在 065 被用户追加，但按本轮定调应回归场景控制，不由模型全局默认）。
4. **S4 设置页规则资产分页：** 「对话分类」分页新增或独立「规则资产」分页：红线词表、写作铁律、每场景 rules/sections/thinking/privacy 均可查看与编辑；保存后写入 profile-rules 文件（不重启）；文件损坏时只读报错不覆盖；恢复默认按钮（重新写入出厂默认值）。
5. **S5 测试与证据：**
   - 单测：词表空=放行（纯净底座）、16 词行为不变、掩码样例（310***45 类）、场景思考档默认、v1→profile-rules 迁移无损。
   - verify 脚本：真实窗口跑一遍（无规则文件 → 纯净底座放行；有规则文件 → 纪检词拦截 + 隐私脱敏 + 周报组装）。
   - ttfb 对比：同一随手记任务在「轻装 vs 完整铁律」两种模式下的 ttfb_seconds 记录，落 evidence/od-arch-067/。
   - 回归：cargo test / pytest / tsc / npm build 全绿；runtime/ 零差异。

**验收标准：**
1. 把 `discipline.rules.json` 临时移走 → 重启 → 平台对任意输入零拦截零铁律（纯净底座行为），界面对应提示「未加载 profile 规则」。
2. `discipline.rules.json` 在位 → 案件词仍拒、身份证/手机号掩码放行且提示「已脱敏」。
3. 随手记模式下 ttfb 明显低于带完整铁律模式（记录两组 ttfb_seconds）。
4. 设置页改规则（如删除一条红线词）→ 保存 → 不重启 → 下一任务生效。
5. 全量回归绿；runtime/ 零差异；`omnidesk-model.yml` 的 `reasoningEffort: high` 已清除。

**风险与停止条件：**
1. 纯净底座（无规则）在合规上可接受吗？——本轮定调：profile 缺失=纯净底座属预期行为，平台不内置任何行业规则；需在设置页「关于」或「权限」分页明示「当前未加载行业规则包」，避免用户误以为 OmniDesk 本身有合规拦截。
2. 掩码可能误伤（如把不是身份证的数字串掩掉）→ 正则先保持现有精度（已有 id_card/phone 识别），不扩大匹配范围。
3. 思考档默认值可能与用户习惯不符 → 用户手动选档始终优先，场景默认仅在新会话首次使用时生效。
4. 迁移只补字段不删字段：v1 offline-flows 中已有 rules/sections 保留在原文件，profile-rules 只额外管理铁律/红线词/思考档/privacy；两文件并存时以 profile-rules 为权威（红线词/铁律），offline-flows 继续管周报章节。
5. 同一问题连续两轮修复未解决即停。

**预计规模：** 半天 BUILDER 批次（BUILDER 由用户指定，建议 DSV4.1-flash 或同级）；完成后等用户批准提交推送（与 066 一起分批）。

**后续批次预告（待 068 验收后逐个规划，不在本计划内）：**
- **OD-ARCH-069（候选）：** 周报全链路用户实测（用户亲自操作，走合成或公开材料，按 user-testing-checklist.md 记录 ttfb、返工、卡点）；验收后决定是否进入真实周报试用（需单位书面许可）。
- **OD-ARCH-070（候选）：** 文案写作与项目跟踪的场景化打磨（根据 069 反馈收敛）。

---

### OD-ARCH-069：工具执行可见化 + 超时治理 + 用户实测清单更新（已执行，2026-10-23；ARCHITECT 复核通过，超时项被 OD-ARCH-071 作废）

**背景（用户实测反馈）：** 工作记录/项目跟进正常（ttfb 2.8–5s），但文案写作中模型发起联网检索、用户授权后界面卡住 3 分钟以上。台账证据 omni-21ca40e9：ttfb 2.86s、183.6 秒后落 unknown（timeout）、pending_approvals=0。诊断（REVIEW.md 2026-10-23 条目）：①工具执行期间前端不渲染 tool_activity，界面零活动可见性；②DSH 的非 approval 类 waterfall 询问（钩子/其他权限）被适配层静默忽略，引擎等应答 forever；③180 秒总超时对“搜索+抓取+长文”偏短，超时后无重试提示。三件事叠加造成“看起来死机”。

**目标：** 让工具执行期间界面可见、可理解、可恢复；把“引擎问了但没人答”从静默挂起变成显式提示。

**影响范围：** engine/omnidesk_engine/web_api_adapter.py（waterfall 处理、超时策略）；desktop/src/shell/HomeChat.tsx（tool_activity 渲染、unknown 态文案与重试入口）；desktop/src/types.ts（如需扩展 payload 类型）；测试与证据目录。

**实施步骤：**
1. **前端渲染 tool_activity：** HomeChat.tsx 事件订阅新增 tool_activity 分支，在生成中的助手消息下方显示“正在调用工具：{tool_name}（{status}）”，任务结束自动清理。不改数据结构，不加新组件文件。
2. **waterfall 静默忽略改为显式进度：** _on_frame 对非 approval 的 waterfall 事件发一条 PROGRESS（kind:"engine_ask"，附 event 原文类型），前端显示“引擎在等待一项未被识别的确认（{event}），如长时间无响应请取消重试”。不自动应答、不假装已答。
3. **超时策略调优：** writing/project 场景 default_timeout 180→300 秒（配置化，读 omnidesk.model-settings.v2.json 新字段 task_timeout_seconds，缺省 300）；超时落 unknown 时事件 note 附“已等待 N 秒；可点取消后重发或降低工具使用”。
4. **unknown 态界面文案：** 任务落 unknown 时，生成中的消息尾部显示“引擎在 {N} 秒内未返回结果，本次未自动重试”，提供“取消”按钮（已有）与“重发”按钮（复用现有发送路径）。
5. **回归测试：** 既有 pytest/cargo 全量回归；新增单测覆盖 waterfall engine_ask 事件映射与超时 note 文案。

**验收标准：**
- 工具执行期间界面显示工具名与状态（合成端点可测）。
- 非 approval waterfall 事件在界面可见，不再静默。
- 超时从 180 秒变为可配置（默认 300），unknown 态有明确文案与重发入口。
- 全量回归通过；runtime/ 零差异。

**风险：** waterfall 事件类型枚举来自 DSH 上游，若未来新增类型需继续“显式忽略 + 可见提示”策略，不自动应答未知询问（安全边界）。

**验收后：** 用户按 docs/user-testing-checklist.md 重跑文案写作 + 联网检索场景，确认体验改善。

---

### OD-ARCH-070：DSH 插件生态装载 + 上游错误透传（已执行，2026-10-23；ARCHITECT 复核通过，超时项被 OD-ARCH-071 作废）

**背景（用户对比实测 + 本机核查）：** OmniDesk 隔离环境的 web profile 只装 dsh-base + dsh-web-app，无任何搜索插件；用户原生 DSH 桌面 profile 装 dshmarket + dsh-free-search + dsh-cost-meter + web-search-deepseek patch，同题搜索 8 分 43 秒成功且质量高。OmniDesk 同题 300 秒超时零产出；原生 DSH 无插件版 3 分钟后 400「Content Exists Risk」（OpenDesign 网关内容过滤，非 OmniDesk bug）。**根因：配置差距，不是架构缺陷。**

**目标：** 让 OmniDesk 的隔离 DSH 具备与用户已验证环境相同的插件能力，且上游真实错误不再被吞。

**影响范围：** desktop/src-tauri/src/engine_supervisor.rs（profile 引导）、engine/omnidesk_engine/web_api_adapter.py（错误透传）、samples/profiles/（出厂资产）、~/OmniDesk/runtime/dsh-home/profiles/web/（运行时落盘）；不动 runtime/ 源码。

**实施步骤：**
1. **web profile 装载插件：** 在 OmniDesk 隔离 dsh-home 的 web profile 安装 dsh-free-search（免费多引擎，无 key 可用）与 dsh-cost-meter；pnpm 依赖写入该 profile 的 package.json（与用户 desktop profile 同版本）；cordis.patch.yml 增加 web-search-deepseek patch（apiKeyEnv DEEPSEEK_API_KEY，maxUses 20，用户后续可自行配 key 或用免费引擎）。dshmarket 视依赖体积与离线可用性决定，不阻塞本步骤。
2. **profile 资产化：** 出厂副本 samples/profiles/web.profile.json + web.profile.patch.yml（与 discipline.rules.json 同模式）；启动时若运行时 profile 缺失则从出厂副本落盘，已有则不覆盖（用户可改）。
3. **上游错误透传：** web_api_adapter.py 的 _frame_error/_raise_follow_error 把「HTTP 400 + code=upstream_error + message」作为 error_message 完整落台账（截断至 300 字符），事件发 task_failed 而非 unknown/timeout；前端 unknown/failed 均显示错误原文摘要。
4. **超时与搜索匹配：** 检测到任务使用搜索类工具时，task_timeout_seconds 建议值提升（读配置字段 search_task_timeout_seconds，默认 600）；不影响非搜索任务默认 300。
5. **回归与证据：** 全量 pytest/cargo 回归；证据落 tests/verification/desktop/evidence/od-arch-070/（profile 装载清单 + 错误透传单测 + 合成端点回归）。

**验收标准：**
- OmniDesk 隔离环境对话中模型可用 web_search（free-search 引擎回退链），无需用户配 key。
- 上游 400 错误在台账与界面可见完整原因（非 unknown/timeout）。
- 搜索类任务超时默认 600 秒，其他 300 秒。
- runtime/ 零差异；已有用户 profile 不被覆盖。

**风险：** dsh-free-search 引擎可用性随外网波动（已有回退链）；插件版本与 DSH 0.1.7-rc.1+ 的兼容性以实跑为准；DEEPSEEK_API_KEY 由用户自行配置，软件不读不存。

**验收后：** 用户在 OmniDesk 重跑“政治监督沿革”搜索题，对照原生 DSH 结果评估质量与耗时。
- **OD-ARCH-071（已归档）：** 用户直接指令——取消任务自动超时（任务终态由底座决定，用户手动取消；详见 STATUS.md 顶部与 REVIEW.md 复核，均通过）。
- **数据分析功能：** 继续延后（用户明确），取得使用证据与授权进展后再立项。

---

### OD-ARCH-072（合并版）：能力管理分页 + 首页信息架构改版（待批准，2026-10-23）

> 本版合并原 073（首页改版）并新增构建修复步骤；073 计划撤销，内容并入本计划。

**背景：** OD-ARCH-070 已把插件绑定进 profile（出厂播种、不覆盖用户改动），但应用内没有管理入口：Skill 只能改目录文件，插件只能手改 profile。用户选定方案 B，并要求兼容 DSH 原生插件安装方式。核查确认：DSH 原生安装通道即 dshmarket bundle（用户 desktop profile 已用 ^1.66.35 装过 multi_search/anysearch），可直接复用。

**目标：** 设置页新增「能力管理」分页，集中查看和管理 Skill 与插件；提供「接入 DSH 插件市场」开关；同时完成首页信息架构五项改版。修复当前「能力清单：Command capability_view not found」——根因已查明：用户桌面启动的是 10 月 19 日构建的 release 旧程序，该命令注册于其后，旧程序未注册所致（前端 dist 为 10-23 21:41 新构建；debug 二进制为 21:39 新构建；release 二进制停在 10-19 08:18）。

**影响范围：** desktop/src/SettingsPanel.tsx（新增分页）；desktop/src-tauri（新增 2–3 个只读/受控写命令）；~/OmniDesk/runtime/dsh-home/profiles/web/{package.json,cordis.patch.yml}（受控写）；不动 runtime/ 源码；不碰 .dsh / .agents / deepseek-harness / ~/.dsh。

**实施步骤（Part A：能力管理）：**
1. **只读列出能力清单：** Skill = 扫描 ~/OmniDesk/runtime/dsh-home/skills/ 目录（名称、一行说明（如有）、最近修改时间，不造假版本号）；插件 = 读 profile package.json 的 bundles + dependencies（名称与版本）。
2. **插件启用/停用：** 停用 = 从 bundles 移除该项（dependencies 保留），写前自动存 .bak；启用 = 加回 bundles。改动需重启 DSH 生效，界面注明「重启后生效」。仅允许写 ~/OmniDesk/runtime/dsh-home/profiles/web/ 两个文件，复用 seed_profile_if_missing 同款路径硬拒绝校验，原子写入。
3. **「接入 DSH 插件市场」开关：** 开 = bundles 加入 dshmarket（版本对齐用户 desktop profile 的 ^1.66.35）；关 = 移除。市场内的浏览/安装/更新由 DSH 原生界面承担，OmniDesk 不复制其功能。
4. **Skill 只读展示 + 打开目录：** 提供「打开 Skill 目录」按钮（宿主 open 目录能力已有先例），不做在线编辑器。

**实施步骤（Part B：构建与部署修复）：**
5. **重建并重启：** npm run build + cargo build --release（或按项目现有打包命令）；确认 release 二进制含 capability_view/capability_set_plugin/capability_market/capability_open_skills 注册（strings 或运行验证）；用户桌面启动方式指向新构建。此为用户报错的直接修复。

**实施步骤（Part C：首页信息架构改版，原 073 内容）：**
6. **品牌与 Profile 分离：** TopNav 左侧固定「OmniDesk」软件名；其后新增 profile 徽标（样式与软件名明显区分，如小胶囊/描边风格），默认显示「无 · 标准模式」，点击弹层列出可用 profile（当前已有：无/discipline；弹层预留后续扩展）；选择后确认弹窗说明「重启引擎生效」，确认后走现有 profile 切换链路。
7. **日程节律悬浮：** ThreeWeekCalendar 卡片从常驻布局移除；改为点击 TopNav 日期区域显示悬浮卡片，鼠标移开 1 秒后淡出；不改日历数据源。
8. **删除今日重点推进卡片：** HomeCanvas 移除卡片及相关代码路径；待办条目改由欢迎语「欢迎回到 OmniDesk」下方展示，仅显示一条，多条时逐条轮动（间隔约 5 秒，淡入淡出）；无待办时不占位。
9. **小机器人移位：** 从当前位置移到对话框上方、历史记录列表左侧；具体锚点随对话框落位联动。
10. **对话框落位：** 无对话历史时输入框垂直居中于首页中部（参考 DSH 原版首页）；产生历史后自动落到底部（历史区上方）；动画平滑（如 200ms），不改消息发送逻辑。

**回归与证据：** cargo/pytest/tsc/build 全量；新增单测覆盖 bundles 增删与路径拒绝；桌面 e2e 与截图脚本适配新布局后复跑；证据落 tests/verification/desktop/evidence/od-arch-072/（能力清单 + 插件启停 + 市场开关 + 首页四态截图：初始、有历史、悬浮日历、profile 弹层）。

**验收标准：**
- 设置页能看到全部 6 个 Skill 与当前 4 个 bundle（含版本）。
- 停用 dsh-cost-meter → 重启 → 启动日志无 dsh-cost-meter；启用 → 恢复。
- 打开市场开关 → 重启 → dshmarket bundle 出现在启动日志/界面；关闭 → 移除。
- 所有写操作有 .bak 备份；对 .dsh/.agents/deepseek-harness 的写入被硬拒绝；runtime/ 零差异。
- 桌面启动的不再是 10-19 旧 release；能力清单正常显示（不再报 Command not found）。
- 标题栏「OmniDesk」+ profile 徽标可点击切换、默认「无 · 标准模式」；日历悬浮；无今日重点卡片、待办轮动；机器人位于对话框上方历史左侧；对话框初始居中、有历史落底。
- 全量回归通过。

**风险与注意事项：**
- dshmarket 体积较大，打包阶段再评估是否默认带出；当前按需开关。
- 社区插件纪律不变：一次装一个、装完跑回归、核查来源与权限。
- 改 profile 仅动 bundles，不改 dependencies 与 cordis.patch.yml 内容，避免与 070 出厂播种逻辑打架（播种只在文件缺失时触发，用户已有文件不被覆盖）。
- 首页改版涉及布局重构，注意窗口失焦/最小化时暂停轮动计时器，避免无效渲染。
- profile 切换沿用现有链路，弹层交互与现有引擎状态冲突时以现有状态机为准，不重写。

**预计规模：** 一个 BUILDER 批次（合并后约 1–1.5 天）；完成后等 ARCHITECT 复核 + 用户批准提交推送。

---

### OD-ARCH-074：引擎进程生命周期修复（孤儿回收 + 所有权登记 + 实例互斥）（已执行，ARCHITECT 复核通过，2026-10-24；双组偏差认可，见 REVIEW.md 与 docs/decisions.md D-029）

**背景（Luna 审计 + ARCHITECT 根因核查）：** 宿主 spawn python 引擎、引擎 spawn node DSH（孙进程，未设进程组）。正常收尾链完整（Drop→terminate→atexit→杀 node），但异常路径（引擎被 SIGKILL、宿主崩溃）会让 node 成孤儿：继续监听端口、持有 dsh-home，历次测试累积过 22 个（约 3.5GB）。无所有权登记与实例互斥，多引擎可并发写同一 dsh-home（已实测干扰账本）。

**目标：** 引擎进程树在任何退出路径下都不留孤儿；同数据根同时只有一个活跃引擎；崩溃后下次启动能安全回收遗留进程；不误伤用户自己的 DSH。

**影响范围：** engine/omnidesk_engine/{web_api_adapter.py,acp_adapter.py,serve.py}（进程组 + 所有权文件）、desktop/src-tauri/src/{engine_supervisor.rs,engine_ipc.rs}（启动前回收 + 收尾兜底）、desktop/src-tauri/src/lib.rs（RunEvent 退出钩子已有，补异常路径）；不改协议、不改架构、不动 runtime/。

**实施步骤：**
1. **进程组绑定：** 引擎 Popen node 时设 start_new_session=True（POSIX 进程组）；AC P 适配器同理；引擎自身启动时（serve.py）也建组。清理时用 os.killpg 组杀，覆盖整棵进程树。
2. **所有权登记：** 引擎启动时在数据根写 owner.json（pid、进程组 pgid、boot_id（/proc/sys/kernel/random/boot_id 防 PID 跨启复用）、宿主侧无）、通道、时间戳、命令行指纹；引擎正常退出时删除。宿主启动引擎前读该文件核验身份（pid 活着 + pgid 匹配 + boot_id 一致 + 命令行含 omnidesk_engine），全部匹配才认定「遗留 OmniDesk 引擎」。
3. **启动前回收：** 宿主 spawn 引擎前执行回收：身份核验通过 → killpg(SIGTERM) → 等待 ≤5s → 不退则 SIGKILL → 删除 owner.json；核验不过（含文件缺失、字段不匹配、boot_id 不同）→ 视为非我方进程，不动它，若同数据根仍有活跃进程则停止启动并明确报错（防并发写）。
4. **实例互斥：** 引擎侧启动时若发现同数据根存在身份核验通过的活跃引擎（非遗留、非自己），拒绝启动并返回明确错误；宿主展示原因。合法多实例（不同数据根）不受影响。
5. **宿主收尾兜底：** 现有 Drop 与 RunEvent::Exit 之外，在引擎侧补 SIGTERM/SIGINT 处理器（serve.py），确保信号路径也走 shutdown；宿主侧不做名字模糊扫描，只信任 owner.json。
6. **测试（隔离数据根 + 可控假引擎进程，不碰用户真实 DSH）：**
   - 正常启动/退出：owner.json 建与删、node 随引擎退出。
   - kill -9 引擎 → node 成孤儿但组回收仍可达（进程组语义）→ 再次启动时被回收。
   - kill -9 宿主 → 引擎收 SIGTERM（前台进程组）→ 走 atexit → node 清理。
   - PID 复用伪造：owner.json 写假 pid/不匹配 pgid → 拒绝回收、停止启动。
   - 合法第二实例（不同数据根）正常启动互不干扰。
   - 既有全量回归（cargo/pytest/tsc/build/e2e）。
7. **证据：** tests/verification/desktop/evidence/od-arch-074/（各场景日志与回收结果，不含凭证）。

**验收标准：**
- 任何退出路径后，系统无 omnidesk_engine/node(bin.js) 孤儿（以 owner.json 与进程组核验为准）。
- 同数据根双引擎启动被拒绝且原因可见；不同数据根不受影响。
- 用户原版 DSH（~/.dsh、DeepSeek Desktop）全程零接触、零误杀（测试断言）。
- 全量回归通过；runtime/ 零差异；不提交不推送。

**风险与边界：**
- killpg 只对「身份核验通过」的进程组执行；boot_id 防跨重启 PID 复用误杀。
- ACP 通道的取消语义不完整属另一问题（Luna 列 P1/暂缓），本任务不处理。
- 不做通用进程管理框架，owner.json 是唯一所有权事实源。

**预计规模：** 一个 BUILDER 批次（约一天）；Phase 2（通道能力说明，P1）单独成计划，待本计划验收后另行批准。
- **提交推送：** 066 + 068 + 累积的 audit/ 目录 + README 改动一并分批入库，推送一次（推送前红线扫描 + 文件清单核对）。
