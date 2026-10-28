## OD-ARCH-104 审查结论（ARCHITECT，2026-10-28）——通过（真实模型准确率待实测）

**结论：通过。** 随手记不再用前端规则猜「这句是指令还是内容」：这句话交给模型判断，模型只能在正文后附一个结构化判定块；能不能写、写到哪，由宿主用确定性规则校验后才落盘。用户上一次踩的坑——「上杭教学和参观紫金矿业都是本周一的行动，你全部调整至周一的记录。」被整句当正文记到周三——在这版里会走 moves，不会产生新条目；模型若判错或格式不合规，一律不写、不改，只把原话按当天记下并明说「未做指令识别」。

**独立复核范围：** HEAD `378f729` + 本轮改动。逐文件核对 `workRecordVerdict.ts`（切块/解析/校验/索引，确认其中没有任何语义词表）、`conversationPrompt.ts`（note 分支的原则与索引注入、技能注入顺序）、`memory-agent/SKILL.md`（先分类、判定块形状、六要素、周报归属、数据边界）、`worklog.rs`（`move_entries` 的「全部校验后才写」顺序、区域不重叠、倒序摘除、空段清理只针对本次涉及日期、`assert_structure_preserved` 的四类拒绝、单份备份与写后回读）、`lib.rs`/`api.ts`/`types.ts`（命令注册与字段形状）、`HomeChat.tsx`（读盘→索引→提交→终态→校验→执行的先后关系、append 只出现在 `executeNote` 内、取消分支不写盘）。`runtime/` 零差异；未读真实工作记录与凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx tsc --noEmit` 通过；`npm run build` 通过（仅既有 chunk 体积提示）；`verify_worklog_104.py` **67 / 67**；`verify_worklog_103.py` **52 / 52**；`verify_worklog_089.py` **61 / 61**；`verify_writing_094.py` **98 / 98**；`verify_hover_hint_101.py` **30 / 30**；`verify_skill_bindings_082.py` **38 / 38**；`verify_intent_088.py` **22 / 22**；`verify_conversation_prompt.py` **26 / 26**；`cargo test --offline --lib` **401 passed、0 failed**；`python3 -m pytest tests -q` **89 passed、30 subtests**。

**审查判断（几处特意验证过的边界）：**

1. **判定先到，写盘在后。** 静态断言把 `shell.worklogAppend(` 的出现位置限定在 `executeNote` 内（降级兜底一处、按判定逐条写入一处），而 `executeNote` 只在终态回读之后由 `settleNote` 调用。判定到达之前，这一轮对磁盘零写入。
2. **每轮只执行一次。** `notePendingRef` 以 taskId 为键，进入 `settleNote` 立即置 `done` 并删除；「重新查询」再跑一遍终态处理不会把同一句记两遍。取消/中断分支把判定上下文与流式原文一并丢弃，既不会误写，也不会长期占内存。
3. **模型不能编造正文。** `records[].text` 去标点空白后必须能在用户原话里找到连续片段；探针 `reject_invented_text` 用编造句子验证整份判定作废。这条是防「模型把意思复述一遍」写成新记录，与语义判断无关。
4. **越界一律拒绝。** 跨周、未来日期、索引越界、同一位置移动两次、`record` 却无记录、`confirm`/`answer` 却带写入——逐条有断言；Rust 侧对同类情况在写盘前返回错误，文件字节不变。
5. **批量移动是原子的。** `move_entries` 先把所有请求定位并校验完，再一次性重写；区域重叠直接拒绝；条目总数、非标题行内容都不得变化；被移动的每一行必须原样出现在新文件里。多份移动只留一份备份。
6. **失败不丢数据。** 判定缺失/不可解析/不通过时，原话仍按当天写入，回执明确区分「模型未给出判定」与「写入失败」；如果原话里有日期却没被采用，会额外说明，免得用户以为记到了那天。
7. **Skill 不越权。** Skill 明确写「你判语义，工作台落盘。你没有写文件的权限」，且不含任何宿主命令名；1232 字符，在预算内。

**计划偏离的裁定：** 无。S1–S5 全部按计划实现；未引入删除动作，模型无权删条目。计划中已列明的代价（判定后才落盘、提示词多出索引段）确实存在，属设计取舍。

**已知限制（其中第一条是本次唯一未验证项）：** ① 真实模型的分类准确率没有验证——脚本只证明「校验与落盘按规则执行」。`moves` 与 `confirm` 在真实模型上的实际表现需要用户端到端实测，`action=answer` 是否会被滥用也属实测项。② 跨周移动仍不支持，宿主会明确报错并保持文件原样。③ 索引只带最近 7 天、每条 80 字、最多 60 条；超长当周记录里较早的条目模型看不到，无法据此移动。④ 记录不再「敲完立刻入库」，要等模型返回（通常几秒）。⑤ 本轮无窗口级验证。

**阶段决定：OD-ARCH-104 验收通过，一并提交并推送。** 真实模型端到端实测作为后续独立项，由用户实测反馈后再决定是否需要调整提示词或校验口径。

---


## OD-ARCH-103 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 用户复现的问题（周三补录周一课程，三条全落周三）已被修掉：输入里的日期现在有一条确定性通道传到写入层，落盘日期由宿主校验，认不出、指向未来或别的周时不猜，并且会明确告诉用户「我按今天记了」。记错时可用一句「记错了，应该是周一」把那条挪到正确日期段，正文原样保留、原文件备份。

**独立复核范围：** HEAD `238f8c2` + 本轮改动。逐文件核对 `workIntent.ts`（日期优先级、周内与不晚于今天的双重约束、前缀排除、纠正句的双门槛与剩余内容阈值、细节补充词表）、`worklog.rs`（`assert_entry_date` 三重校验、`locate_entry_block` 的正文一致性、`move_entry` 的搬块/清空段/并段/起段四条分支、自证集合只允许日期标题变化、`.bak` + tmp + `sync_all` + rename + 回读回滚、CRLF 与混用换行处理）、`lib.rs`（`entry_date` 校验在读取库根之前、失败回执形状统一、`worklog_redate` 注册）、`api.ts` / `types.ts`（`entryDate` 传 null 的语义、回执字段）、`HomeChat.tsx`（纠正优先于写盘与模型、`lastNoteRef` 生命周期、时区对齐、继承条件、提示文案）、`memory-agent/SKILL.md`（新增 10 行、1782 字符，未写不可达工具名）。`runtime/` 零差异；未读真实工作记录与凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx tsc --noEmit` 通过；`npm run build` 通过；`verify_worklog_103.py` **58 / 58**；`verify_worklog_089.py` **61 / 61**；`verify_writing_094.py` **98 / 98**；`verify_hover_hint_101.py` **30 / 30**；`cargo test --offline` **395 lib + 41 集成/文档全绿、0 失败**；`python3 -m pytest tests -q` **89 passed、30 subtests**。

**审查判断（几处特意验证过的边界）：**

1. **不静默回退。** 认出的日期不合法（未来、他周、格式错）时，宿主直接拒绝并在回执里说明原因，绝不退回当天写盘——那正是本次要修的缺陷的变体。`assert_entry_date` 的单测逐条覆盖五种拒绝理由。
2. **纠正不可越权。** `move_entry` 只搬「`- **事**` 到 `**去向**`」这一块，`expect_text` 与文件内容不一致即拒绝且文件零变化（有单测比对前后字节）。写后自证要求条目总数守恒、被删行只能是原段/目标段标题、被加行只能是目标段标题，任何正文变化都中止。
3. **真实记录不会被误判成纠正。** 「会议改成周五了」含纠正动词但无回指词、不以口令开头，`detectRedateInstruction` 返回 null，照常记为一条随手记——探针 `correction_ignores_plain_note` 就是这条。
4. **时区同口径。** 界面读 `utc_offset_minutes` 后传给日期识别，与宿主 `date_label` 一致；这是审查中发现的真实缺陷，已修并加静态断言 `home_uses_host_timezone`。
5. **「上一条」不跨会话。** `lastNoteRef` 在 `threadKey` 变化时清空，避免在另一个对话里误挪别人的记录。
6. **Skill 不越权。** 新增段落只让模型「指出不符、请用户回一句纠正」，明确写「你没有写文件的权限」；验证脚本断言 Skill 内不出现 `worklog_redate` 工具名，防止模型去调一个拿不到的命令。

**计划偏离的裁定：** 原计划 S3 写「模型经工具调用触发 `worklog_redate`」，实际不可达——`acp_adapter.py` 三处 `mcpServers: []` 让模型没有任何宿主命令通道。改为界面确定性纠正通道，功能等价且路径更短，命令与安全校验一行未减。这一偏离在实施前由本助手发现并记录在 STATUS.md，属合理收窄，不构成范围放宽。

**已知限制（不构成阻断）：** ① 跨周补录与跨周改日期仍不支持，界面会明确说明而不是猜。② `lastNoteRef` 只在内存里，重启应用后不能用一句话纠正很久以前那条，需重新说一遍带日期的原文。③ 界面按设置项时区算日期，若用户改了设置里的时区而未重启，需重新进入场景生效。④ 本轮无窗口级验证，改动集中在写入层与提交前判定。⑤ 未在真实工作记录与真实显示器上复核。

**阶段决定：OD-ARCH-103 验收通过。** 按用户长期偏好，本轮改动一并提交并推送 GitHub。

---

## OD-ARCH-102 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 用户三点要求全部落地：项目跟进页顶部指标卡移除；左栏拆成「项目事项」与「历史对话」上下两区；事项卡片支持新增、修改、删除，录入后才画右侧时间线。数据从「首页最近一条对话的确定性汇总（只读）」换成应用数据根下的 `project-items.v1.json`，写入路径收敛为两个命令。

**独立复核范围：** HEAD `609567e` + 本轮改动。逐文件核对 `project_items.rs`（新文件，应用数据根定位、`assert_local_root_allowed` + `assert_outside_repo` 复用、1 MiB / 500 条上限、单字段 400 字、同目录临时文件 + `sync_all` + rename、覆盖前 `.bak`、坏主文件只在**结构损坏**时回退备份、schema/条目不合规一律整批拒绝并报「第 N 条」、`created_at` 沿用而 `updated_at` 每次刷新）、`task_shell.rs`（四个校验函数抽出且 `summarize_project` 改调同一实现，行为未变）、`lib.rs`（两命令注册，`task_shell::now_secs()` 注入时间）、`api.ts` / `types.ts`、`projectGantt.ts`（`recordToItem` 按当天现算逾期，不落盘）、`ProjectBoard.tsx`（v3 双区左栏、内联表单、编辑回填、删除确认、搜索过滤、空态不画轴）、`ScenePage.tsx`（project 分支不渲染 `SceneStatusStrip`、读写改走本机文件、页面底部不再重复历史区）。`runtime/` 零差异，未读真实知识库、成果或凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx tsc --noEmit` 通过；`cargo test --offline`（按 `scripts/setup-desktop-toolchain.sh --print-env` 设环境）**384 lib + 41 集成/文档测试全绿、0 失败**；`python3 -m pytest tests -q` **89 passed、30 subtests**；`verify_project_102.py` **51 / 51**；`verify_project_098.py` **35 / 35**。窗口证据 `evidence/od-arch-102/` 十张截图与 `facts.json` 已核对。

**审查判断（几处我特意验证过的边界）：**

1. **坏文件不会静默吞数据。** `load()` 只在结构性故障（读不到、解析失败、非普通文件、超限）时回退 `.bak`；schema 不符或条目校验失败即使备份可读也直接报错，两个单测分别覆盖。这避免了「用户看到的是旧备份、以为编辑丢了」这种情况。
2. **`created_at` 的沿用按事项名匹配。** 改名后 `created_at` 会重置为当前时间，`updated_at` 不变地刷新——与表单「改名」语义一致，已由 `edit_moved_created_at=true` 的窗口读数佐证。
3. **写入不碰知识库与成果目录。** 存储只落在应用数据根，`assert_outside_repo` 保证不会写进仓库；CRUD 全程不调模型。
4. **右栏条件渲染正确。** 空清单时既不画甘特轴也不显示假数据，只有引导文案；录入后卡片数与磁盘条目数一致（`first_file_items=1`、`two_cards=2`、删除后 `after_delete=1`）。

**目视复核：** `01-project-empty.png` 显示顶部无指标卡、左栏上下两区、右栏空态引导且无时间轴；`03-project-two-items.png` 显示两条事项与两条甘特行一一对应、`P1`/`P2` 代号与「还有 23 天」相对期限正常；`07-project-narrow.png` 显示 1000px 下按左栏 → 右栏单列排列，三个标题都在。

**已知限制（不构成阻断）：** ① 未做旧数据迁移——v3 从空清单开始，首页对话的旧汇总链路保留但不再驱动本页；这是计划里写明的取舍。② 窗口证据为 Xvfb 1600×980 / 1000px + `/tmp` 合成数据，未在真实显示器与真实数据上复核。③ WebKitGTK 不把静态 `p`/`span` 暴露到 AT-SPI，空态引导文案靠截图 + 静态断言覆盖。④ 拖拽改期、多人协作不在计划内。

**阶段决定：OD-ARCH-102 验收通过。** 按用户长期偏好，本轮改动一并提交并推送 GitHub。

---

## OD-ARCH-101 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 软件内 91 处原生悬停提示已全部改为自绘方角卡片：白底、1px 细边框、直角、轻投影、11.5px，与 `StatusToast` 和轮次索引卡同一套视觉语言；不再出现系统默认气泡。41 处组件 prop 的 `title` 一件未动。

**独立复核范围：** HEAD `3e2bd17` + 本轮改动。逐文件核对 `HoverHint.tsx`（单例挂载、`pointermove` 委托 + `elementFromPoint` 向上找 `data-hover-hint`、320ms 延迟、portal、`role="tooltip"`、`aria-describedby` 补挂与还原、`pointer-events-none`、`z-[105]`、`borderRadius: 0`、翻转与水平夹紧、Esc/滚动/点击/离开关闭）、`App.tsx`（`<HoverHint />` 只出现一次）、23 个含 `data-hover-hint` 的 tsx、`ArtifactExplorer.tsx`（悬停取完整标题而非文件名）、`TopNav.tsx`（两个纯图标按钮补 `aria-label`）。`runtime/` 零差异，未读真实工作记录与凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx tsc --noEmit` 通过；`npm run build` 通过（仅既有 chunk 体积提示，无错误）；`verify_hover_hint_101.py` **30 / 30**；`verify_weekly_091.py` / `092.py` **67 / 67**、**29 / 29**；`verify_weekly_093.py` / `097.py` / `099.py` / `100.py` **105 / 105**、**34 / 34**、**42 / 42**、**32 / 32**；`verify_worklog_089.py` / `verify_writing_094.py` **61 / 61**、**98 / 98**；窗口 `preview_hover_101.py`（`:131`）四项提示均可见。

**审查中修正的两处真实问题：**

1. **延迟计时被持续微动重置。** 初版 `show()` 只在 `currentRef` 已在展示时才提前返回，而计时期间 `currentRef` 仍是 `null`，于是鼠标在同一控件内轻微移动会不断清掉并重建 320ms 计时器，提示可能一直不出。加入 `pendingRef` 后，同一元素已排队或已展示都直接返回；`hide()` 与键盘焦点路径同步清空。窗口脚本的悬停动作也从「移入后停住」改成**六步小幅移动再停住 1.2 秒**，重新抓帧确认四项提示均正常出现——这条修正是靠真实窗口复现的，不是静态推演。
2. **纯图标按钮丢失可访问名称。** `TopNav` 的设置与通知按钮原先只有原生 `title`，迁移后若不补 `aria-label`，读屏将读不到名称。已补，并在 `verify_hover_hint_101.py` 增加 `icon_buttons_have_labels` 与 `pending_hover_not_reset` 两条断言（28 → 30）。

**目视复核：** `01-disabled-send-hover.png` 显示禁用发送按钮照样出提示，卡片直角、白底、细边框、贴近按钮上方；`02-topnav-profile-hover.png` 显示 Profile 徽标下方的方角卡片与导航条同色系、无圆角；`03-timeline-entry-hover.png` 显示时间线截断正文的完整说明按多行排版、不越出左栏；`04-artifact-hover.png` 显示成果卡悬停为完整标题而非文件名。四张卡片投影一致、不遮挡正文点击区。

**迁移完整性：** 全仓小写原生标签的 `title=` 归零；`data-hover-hint` 91 处、23 个文件；13 处位于 `disabled` 控件上，全部有说明；`component_title=22` 处组件属性未被动到。

**已知限制（不构成阻断）：** ① 窗口证据为 Xvfb 1600×980 + `/tmp` 合成数据，未在用户真实显示器与真实数据上复核。② 未逐页人工悬停全部 91 处，取样覆盖首页、工作记录页、顶部导航与成果库，其余由同一接管层与静态断言保证。③ 触屏与读屏软件实机未验证。④ 视觉上是「一套组件、一处样式」，以后若某处需要不同配色需另加变体，当前未提供。

**阶段决定：OD-ARCH-101 验收通过。** 按用户长期偏好，本轮改动一并提交并推送 GitHub。

---

## OD-ARCH-100 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 用户五点要求全部落地：指标卡图标无底色座、放大到 56px；英文字族换为 Inter + IBM Plex Mono（仍自托管）；去向开关改为圆形单点且「进周报」文字移入悬停/读屏语义；成果库的搜索框与 query 过滤删除，工具栏合并成一行；「另有 N 个归档文件名不符合约定…」提示与 `archiveSkipped` prop 一并移除。

**独立复核范围：** HEAD `9989d4d` + 本轮改动。逐文件核对 `SceneStatusStrip.tsx`（`BAND_TONES`/`BAND_ALERT` 的 `chip` 只剩 `text-*`，`BandMetric` 无 `rounded-2xl` 图标座与投影、图标 `w-14 h-14`、列宽仍 `w-14`、`WritingMetric` 未动）、`theme.css`（`--font-*` 全量换 Inter Variable，`--font-label-caps` 换 IBM Plex Mono，CJK 回退保留）、`styles.css`（`.mono`、`.epayload`）、`markdown.css`（正文 code 的内联等宽字体同步）、`main.tsx`（只引 Inter 与 IBM Plex Mono）、`package.json`（新增两包并移除旧三包）、`WeeklyTimeline.tsx`（`rounded-full` 24×24、实心/空心两态、无可见 `进周报`/`保存中…`、`role="switch"`+`aria-checked`+`aria-label`+`title` 保留、失败就地显示、无去向静态标记）、`ArtifactExplorer.tsx`（无 input、无 `query`、工具栏单行、`data-artifact-range`、无归档提示、`archiveError` 保留）、`ScenePage.tsx`（不再传 `archiveSkipped`）。`runtime/` 零差异。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx --no-install tsc --noEmit` 通过；`npm run build` 通过，`dist/assets` 实际产出 `inter-latin-wght-normal-*.woff2` 与 `ibm-plex-mono-latin-400/500/600-normal-*.woff2`；`cargo test --offline` 全绿；`verify_weekly_100.py` **32 / 32**；`verify_weekly_099.py` **42 / 42**；`verify_weekly_093.py` **105 / 105**；`verify_weekly_097.py` **34 / 34**；`verify_weekly_091.py` / `092.py` **67 / 67**、**29 / 29**；`verify_worklog_089.py` **61 / 61**；`verify_writing_094.py` **98 / 98**；窗口 `preview_weekly_100.py`（`:130`）读数全部符合预期。

**审查中补齐的一处遗漏：** 初版只改了 `theme.css`/`styles.css`，`markdown.css` 里正文代码块的 `font-family` 仍写死 JetBrains Mono，旧三个字体包也仍留在 `package.json` 里随包分发却已无人引用。已改 `markdown.css` 为 IBM Plex Mono，并 `npm uninstall @fontsource/sora @fontsource/plus-jakarta-sans @fontsource/jetbrains-mono`；`verify_weekly_100.py` 增加两条断言（旧包已移除、正文代码块换族），总数由 30 增至 32。

**目视复核：** `01-weekly-wide.png` 四张指标卡图标只以自身色相着色、下表无文字，卡高与左侧对齐一致；`02-band-icon.png` 特写显示图标填满原色块位置、无浅色底与阴影；`03b-switch-detail.png` 与 `03d-switch-after.png` 显示圆形空心/实心两态，`03c-switch-hover.png` 可见悬停说明；`04-artifact-toolbar.png` 与 `04c-writing-wide.png` 显示成果库与写作页工具栏均为一行、无搜索框；`05-narrow-1000.png` 显示 1000px 下指标带与列表正常。

**数据写入证据（`/tmp` 合成周文件）：** 点选一次后 `worklog_changed_lines=[10]`、行数不变、留下 1 份 `.md.bak-*`，`runtime_touched=false`——与 099 的受控改写契约一致，本轮未改写入逻辑。

**已知限制（不构成阻断）：** ① 窗口证据为 Xvfb 1600×980 / 1000×2740 + `/tmp` 合成数据，未在用户真实显示器与真实工作记录上复核。② 1000px 以下更窄视口未单独验证。③ 悬停说明用的是原生 `title` 提示（系统黑底小条），不是自绘 UI 风格卡片——用户本轮只要求「改成悬停提示」，按最小改动处理；若之后希望与软件视觉统一，需要另做自绘浮层。

**阶段决定：OD-ARCH-100 验收通过。** 按用户长期偏好，本轮改动已一并提交并推送 GitHub。

---

## OD-ARCH-099 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 用户四点要求全部落地。第 2 点在实现上严格按用户澄清的语义：点选只改这条记录在周文件里的「去向」字段，不写周报正文；改完后重读整周文件，顶部第 2 张卡「进周报条目」的统计与四象限两格随之变动，周报 Skill 取数口径（只取 `本周报引用`）不变，因此选中条目会进入模型写周报时的上下文。

**独立复核范围：** HEAD `eca8926` + 本轮改动。逐文件核对 `SceneStatusStrip.tsx`（`BandMetric` 两列结构、`data-band-icon-column`、56px 图标座与 28px 图标、图标列内无文字、`items-stretch`；`WritingMetric` 与其它 grid 分支未动）、`worklog.rs`（`set_destination` 只改目标行、`expect_text` 校验、`.md.bak-` 备份、临时文件 + `sync_all` + `rename`、写后逐行自证、失败备份回滚、CRLF 保留、`DESTINATIONS` 新增 `仅记录`、9 个单测）、`lib.rs`（命令注册）、`api.ts` / `types.ts`、`WeeklyTimeline.tsx`（`role="switch"`、`aria-checked`、`data-entry-index`、`entryIndexes` 原始下标、无去向静态标记、失败原因就地显示）、`ScenePage.tsx`（`reloadWeek` / `toggleDestination`）、`ArtifactExplorer.tsx`（48×48 方角序号块、`bg-surface-container-highest/80`、22px、标题单行）、`SourcePeekSheet.tsx`（`items-center`、`max-h-[80vh]`、四边边框）、`skills/memory-agent/SKILL.md`。`runtime/` 零差异。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx --no-install tsc --noEmit` 通过；`npm run build` 通过；`cargo test --offline` 全绿（`worklog` 21 / 21）；`verify_weekly_099.py` **40 / 40**；`verify_weekly_093.py` **105 / 105**；`verify_weekly_097.py` **34 / 34**；`verify_weekly_091.py` / `092.py` **67 / 67**、**29 / 29**；`verify_worklog_089.py` **61 / 61**；`verify_writing_094.py` **98 / 98**；窗口 `preview_weekly_099.py`（`:129`）全部读数符合预期。

**审查中发现并已修正的一处真实问题：** 序号块初版用 `bg-surface-container-high/70`，与卡片正文底 `surface-container-low` 只差一档又叠了 70% 透明，截图上肉眼几乎分不出「深色序号区」。改为 `bg-surface-container-highest/80` 后重跑窗口，序号块与正文区的分界清楚，断言同步改为新 token。

**目视复核：** `evidence/od-arch-099/01-weekly-wide.png` 显示四张指标卡图标独占左列、其下无文字，标题/序号/数字整体右移；`02-band-icon-column.png` 特写确认图标 28px、卡高一致；`03b-switch-detail.png` 与 `04-destination-switch-after.png` 显示方形「进周报」开关的选中/未选中两态；`05-artifact-sheet-centered.png` 显示预览卡在窗口正中、四边有边框、方角、正文自己滚动；`06-narrow-1000.png` 显示 1000px 下单列页面正常。

**数据写入证据（`/tmp` 合成周文件）：** 点选一次后 `worklog_changed_lines=[10]`、`worklog_changed_text=["  **去向**：仅记录"]`、`worklog_line_count_same=true`、`worklog_backups=["2026-W44.md.bak-1793142370558"]`——除目标行外文件未变，行数不变，备份存在。统计卡从 4 变为 3，与取消一条「进周报」吻合。

**已知限制（不构成阻断）：** ① 窗口证据为 Xvfb 1600×980 / 1000×2740 + `/tmp` 合成数据，未在用户真实显示器与真实记录上复核。② 「点选后模型确实只带选中条目进上下文」未经真实模型实跑，只有取数口径与文件字段的静态一致性证据。③ 无去向行的旧条目只能显示静态「无去向」，需在原文补一行后才能点选——这是为避免凭空插入行改变既有结构。④ 1000px 以下更窄视口未单独验证。

**阶段决定：OD-ARCH-099 验收通过。** 按用户长期偏好，本轮改动已一并提交并推送 GitHub。

---

## OD-ARCH-098 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 项目跟进页从旧版「进度 / 阶段成果」页签换成了左项目卡列表 + 右甘特两栏，数据仍来自既有的确定性汇总命令，页面不调模型；参考图里的百分比进度没有数据来源，因此没有虚构，改为状态色与里程碑。

**独立复核范围：** HEAD `e1e4f2d` + 本轮改动（未提交时审查）。逐文件核对 `task_shell.rs`（`summarize_project` 五列行为不变、六列新增可选开始日期、`ProjectItem.start: Option<String>`）、`types.ts`、`projectGantt.ts`（全部纯函数、`today` 由外部注入）、`ProjectBoard.tsx`（左栏卡 + 右栏甘特、窄窗单列、「时间线 / 阶段成果」切换）、`ScenePage.tsx` 接线。`runtime/` 零差异。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx --no-install tsc --noEmit` 通过；`npm run build` 通过；`cargo test --offline` 全绿；`verify_project_098.py` **35 / 35**；窗口 `preview_project_098.py`（`:165`）facts 为 7 张卡、5 个里程碑、2 个区间、搜索 1 条命中、选中联动 true、窄窗两栏 true。

**目视复核：** `evidence/od-arch-098/01-project-two-column.png` 显示左栏 7 张事项卡（状态徽标、负责人、期限、下一步、逾期标）与右栏甘特（日期轴、今日竖线、菱形里程碑、两条区间条、图例）对齐；卡片状态色与既有紫青体系一致，未出现百分比数字。

**已知限制（不构成阻断）：** ① 窗口证据为 Xvfb 1600×980 与 1000px 窄窗的合成数据，未在用户真实显示器与真实事项上复核。② 事项行只有期限一个日期，没有真实进度来源，因此卡片用状态色带而非进度百分比——这是刻意的诚实呈现，不是缺项。③ 甘特的日/周粒度切换在 30 天阈值两侧各有单测，未做超长跨度（>180 天）的可视复核。

**阶段决定：OD-ARCH-098 验收通过。**

---

## OD-ARCH-097 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 用户五点改版全部落地：四象限的 URGENT / IMPORTANT 轴标移到框外、格内只剩序号标题与至多两条、时间线删掉「转周报」并把去向标签移到正文左侧、「打开本周原文」改成贴底弹出卡片、成果库只显示标题与时间。

**独立复核范围：** 逐文件核对 `WeeklyQuadrants.tsx`（轴标布局、`q.action` 与计数只保留在 `title`/`aria-label`、条目 `slice(0, 2)`）、`WeeklyTimeline.tsx`（按钮删除、胶囊前置）、`SourcePeekSheet.tsx`（portal、fixed 遮罩、方角、Esc/关闭/遮罩、焦点归还、dialog 语义）、`ScenePage.tsx`（页面流区块删除、浮层接入）、`ArtifactExplorer.tsx`（卡片与列表只留标题 + 日期、搜索 haystack 不变、路径退到 `title`）。`runtime/` 零差异。

**独立实跑结果（本人执行）：** `verify_weekly_097.py` **34 / 34**；`verify_weekly_093.py` **105 / 105**；窗口 `preview_weekly_093.py`（`:164`）`axes_urgent=true`、`axes_important=true`、`quadrant_action_text_gone=true`、`source_sheet_opened/closed=true`、`main_area_height_stable=true`、窄窗 1000 单列正常。

**审查中发现并已由实施方修正的三处真实问题：** ① 四象限格子的 `rounded-xl` 在本项目令牌里是 3rem，112px 格子被渲染成胶囊形，已改为 `rounded-[8px]`；② 轴标是排版文本，WebKit 不把它暴露到无障碍树，补 `role="group"` + `aria-label` 后可读；③ 预览脚本用「原文预览」字样判浮层，而浮层的无障碍名是文件路径，改为按 dialog / 关闭按钮判定。

**回归影响：** 098 替换了 project 的页签结构后，091 与 092 里 5 条断言过期（它们仍在断言已删除的 `tab === "artifacts"` 源码），已按新契约同步为「weekly 无自有页签 / project 保留两视图」，两条脚本恢复 67/67 与 29/29。084 窗口回归首次复跑误读宿主机 GNOME 无障碍树（脚本指向系统 at-spi 总线），改为私有总线后 **21 / 21** 通过，同一修正同步到 095、060 与三个预览脚本。

**目视复核：** `evidence/od-arch-093/01-weekly-three-column.png` 与 `11-quadrant-axes.png` 显示两行一列轴标在框外、格内无说明文字与统计数字、每格至多两条；`10-source-sheet.png` 显示原文浮层贴底、方角、不推动三栏；`12-artifact-rows-ax.json` 与 `04-artifact-list.png` 显示成果行只剩标题 + 日期。097 的新增抓帧落在 093 的证据目录下（脚本共用同一 `EVID`），归档时已注明。

**已知限制（不构成阻断）：** ① 未在用户真实显示器与真实周记录上复核，窗口证据为 Xvfb + `/tmp` 合成数据。② 时间线「本周报引用」等去向标签来自记录文件的「去向」字段，字段缺失时左侧直接起正文，属既定行为。③ 1000px 以下的更窄视口未单独验证。

**阶段决定：OD-ARCH-097 验收通过。** 按用户长期偏好，本轮改动已一并提交并推送 GitHub。

---

## OD-ARCH-096 审查结论（ARCHITECT，2026-10-27）——通过

**结论：通过。** 构建里那条 `AppendReceipt::failure` 告警确实是死代码，删掉后归零；模型切换的结果提示已从输入框下方的小字挪到右下角方角提示条，60 秒自动关闭、可手动关闭，位置与形态都符合用户要求。

**独立复核范围：** HEAD `5dce7d4` + 本轮未提交改动。逐行核对 `worklog.rs`（删掉整个 `impl AppendReceipt` 块；结构体字段、`worklog_append` 的 JSON 形状与失败分支语义未变）、`StatusToast.tsx`（`createPortal` 到 `body`、`fixed bottom-4 right-4`、`z-[110]`、内联 `borderRadius: 0` 且无 `rounded-*` 类、`STATUS_TOAST_MS = 60_000`、关闭按钮 `aria-label`）、`HomeChat.tsx`（`modelNote` 由字符串改为 `{key, text, tone}`、六处写入点按 `success`/`error` 归类、删掉输入框下常驻渲染、`byUser=false` 的自动套档不写提示）。`runtime/` 零差异。

**独立实跑结果（本人执行，非采信 STATUS.md）：**

| 验证 | 结果 |
| --- | --- |
| `cargo test --offline` | 405 passed / 0 failed |
| `cargo build --offline` 的 `warning:` 计数 | **0**（告警已消除） |
| `tsc --noEmit` | 通过 |
| `npm run build` | 通过（仅既有 chunk 体积提示） |
| `verify_status_toast_096.py`（静态） | **19 / 19 通过** |
| 窗口 `verify_model_route_095.py`（`:104`） | `passed: true` |

**窗口关键 facts（我重跑得到）：** 提示条 rect `x=864, y=936, 560×48`，右边距 16、下边距 16，确实贴右下角且矮平（宽高比 > 1、高度 ≤ 120）；关闭按钮存在；点击后提示条文字与按钮一起消失（`toast_text_present` 由 true 转 false）；不理会它时 **60.5 秒**自动关闭；输入框下方不再有常驻小字（`inline_note_only_in_toast: true`）。失败路径仍给出可读原因（含 Profile 名与原始 stderr），未退化成裸退出码。

**目视复核：** `evidence/od-arch-095/02a-toast.png` 显示提示条为方角长条、左侧青色细标识 + 勾选图标，压在右下角、不挤压输入区；两行折行是因为合成 provider 名与端点较长，`max-w-[min(560px,calc(100vw-2rem))]` 的夹取生效，与既有卡片体系同源。

**已知限制（不构成阻断）：** ① 自动关闭的 60.5 秒来自两秒一次采样的窗口实测，与 60 秒常量的差异属采样粒度。② 未在用户实际显示器上复核，本轮为 Xvfb 一次性显示。③ 上一轮审查已登记的一处小瑕疵仍在：PLANS 里 OD-ARCH-096 的正文写「未提交、不推送」，本轮按你的指示一并提交推送，以本节为准。

**阶段决定：OD-ARCH-096 验收通过。**

---

## OD-ARCH-093 S10 审查结论（ARCHITECT，2026-10-27）——通过

**结论：通过。** 用户第五次反馈的四项改动（待确认入口并入逾期卡、弹窗条目方形底色、分类下拉不被裁切、删除按钮前置）与源码一致，我本人独立复核 diff 并重跑了静态校验与真实窗口预览，未发现阻断问题。

**独立复核范围：** HEAD `b109b18`，改动未提交。逐文件核对 `SceneStatusStrip.tsx`（逾期卡内 `action` 新增「待确认 N」按钮、删除四卡下方独立触发条、`expiredHint` 三态文案）、`PendingWriteCard.tsx`（条目与按钮由 `rounded-xl`/`rounded-md` 去圆角、新增 `SCENARIO_LABEL` 把场景键渲染成中文）、`CategoryPicker.tsx`（portal + fixed 定位）、`ScenePage.tsx`（删除按钮前置）的差异。`runtime/` 零差异。窗口预览只用 `/tmp/od093-s10-arch` 的合成数据（1 条合成待确认写入、3 段合成对话），未读真实工作记录、成果目录或凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：**

| 验证 | 结果 |
| --- | --- |
| `verify_weekly_093.py`（静态 + 纯函数编译断言） | **105 / 105 通过** |
| 窗口 `preview_weekly_093_s10.py`（`:216`，/tmp/od093-s10-arch） | 全部 facts 与我独立核对一致 |

**窗口关键 facts（我重跑得到，与既有证据一致）：** 逾期卡内出现「待确认 1」入口（`pending_trigger_pos` 落在逾期卡范围 x=1285 / 卡 x=1100 宽 276 内），四卡下方不再有独立触发条；待确认弹窗可打开，条目含确认 / 放弃两个动作；历史对话展开后 3 条删除按钮 x=181 均在标题 x=209 左侧；3 个分类下拉各 4 个选项全部落在视口内（y 629–753，无裁切）。

**目视复核：** 截图 `06-pending-dialog.png` 显示弹窗内条目为方形底片、按钮无圆角、场景标签渲染为「工作记录」，四张指标卡与导航条同轴，视觉与既有紫青体系一致。

**已知限制（不构成阻断）：** ① 未在用户实际显示器与真实数据上复核，本轮为 Xvfb 合成数据。② 「待确认写入」当前只有复盘这一个写入入口且复盘界面已按 OD-ARCH-092 移除，因此该入口在正常使用中通常不出现——这是既定设计，不是缺陷；本次验证靠合成数据触发。③ 分类下拉的视口夹取只在 1600×980 与既有 1000px 窄窗下验证过，更极端的缩放未覆盖。

**阶段决定：OD-ARCH-093 S10 验收通过。** 至此 093 的 S01–S10 全部审查完毕。

---

## OD-ARCH-095 审查结论（ARCHITECT，2026-10-27）——通过

**结论：通过。** 两个报错的根因都在补丁集合与 Profile 的传递上，修复与计划一致，且我本人重跑了全部验证，未发现阻断性问题。用户现场的两个现象（登录即「请选择要使用的 Profile、provider 与模型」、切换模型「DSH 配置转储失败（退出码 1）」）在源码层面都已收口，并在真实窗口里复现过修复前、验证过修复后。

**独立复核范围：** HEAD `b109b18`，改动未提交。逐文件核对 `model_routes.rs`（`list_payload` / `select_payload` / `stderr_tail` / `run_script` 的 stderr 管道与线程 join）、`model-routes.mjs`（`redactSecrets` / `describeFailure` / `dumpConfig`）、`ModelPicker.tsx`（`engineProfile` prop 与目录分支）、`HomeChat.tsx`（`engineProfile` state、`selectModel` 守卫、场景思考档 useEffect）。`runtime/` 零差异（`git status runtime/` 为空）。窗口验证只用 `/tmp` 一次性根、合成回环端点与 `/tmp/od095-knowledge`，未读真实成果或凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：**

| 验证 | 结果 |
| --- | --- |
| `cargo test`（含本轮新增 6 项） | **405 passed，0 failed**（lib 363 + 集成 2+13+6+5+10+3+1） |
| `tsc --noEmit` / `vite build` | 均通过 |
| `pytest tests -q` | 89 passed，30 subtests |
| `verify_profile_rules_068.py` | 25/25 |
| 窗口 `verify_model_providers_065.py`（`:65`） | 保存/沿用密钥/错密钥/双 provider 全部符合预期 |
| 窗口 `verify_home_fixes_081.py`（`:87`） | `passed: true` |
| 窗口 `verify_model_route_095.py`（`:95`） | `passed: true` |

**关键证据：** 窗口里预置的覆盖文件只含 `od095-only` / `od095-decoy` 两个合成 provider，Profile 自身没有它们——这与用户现场「`opendesign` 只定义在 `~/OmniDesk/app/config/omnidesk-model.yml`」同构。修复后冷启动无任何失败文案；下拉能列出该 provider；点选后覆盖文件被重写，`acp` 与 `agent-default-model` 都指向所选模型，另一个 provider 保留；不存在 Profile 时错误文案包含 Profile 名与「应改用当前引擎实际加载的 Profile」的中文指引，长度 212 字。真实配置的只读转储也确认 `opendesign` 与 5 个模型现在能被枚举（修复前为空）。

**审查中发现并已处理的问题：** 第一版窗口脚本的断言过宽（覆盖文件本来就含目标 model，`overlay_rewritten` 不足以证明写入发生），且真实窗口暴露了计划内未尽之处——`select_route` 写覆盖文件时同样没带补丁，导致候选可见但点选失败。两处都已修正：断言改为「文件被重写 + 选中项写入 acp/agent-default-model + 另一 provider 保留」，代码补上 `patches` 并新增单元测试锁定。这是审查过程中发现的真实缺陷，不属于计划外扩围。

**只读核对说明：** 我对用户真实数据根跑过一次 `model-routes.mjs` 转储（`DSH_HOME=~/OmniDesk/runtime/dsh-home`，补丁指向 `~/OmniDesk/app/config/omnidesk-model.yml`），只输出 provider 名、模型 id 与「apiKeyEnv 是否存在」，未读取或打印任何密钥值，未启动、重启或停止引擎，未修改任何文件。

**已知限制（不构成阻断）：** ① 未对真实供应商发起联网切换（会消耗用户额度并重启用户引擎），因此「真实模型能否成功应答」仍未验证；本次证明的是候选枚举、Profile 传递、覆盖文件写入与错误文案这条链路。② 未在用户实际显示器的 release 实例上复核，本轮用 debug 二进制 + 一次性数据根。③ stderr 的凭证剔除是形态匹配，不是安全承诺。

**范围外改动登记：** 工作区仍有 OD-ARCH-093 S10 与 OD-ARCH-094 的未提交改动，属独立工作项，本轮原样保留；本轮全部验证在包含它们的树上通过。`evidence/od-arch-065/` 与 `evidence/od-arch-081/` 下的截图与 `facts.json` 变化来自重跑既有窗口脚本的自动覆写，属预期副作用，非人工编辑。

**阶段决定：OD-ARCH-095 验收通过。** 未提交、未推送，由用户决定。

---

## OD-ARCH-094 审查结论（ARCHITECT，2026-10-27）——通过

**结论：通过。** 文案写作页改版（创作管线与灵感架，S01–S06）与已实施源码一致：指标四卡、三栏结构、状态分组与 90 天回顾筛选、灵感架回首页跳转、窄窗单列降级均按计划落地；宿主侧 frontmatter 只读解析边界清楚，未越界做编辑器、AI 复用或新存储。本轮由我本人独立重跑全部验证并逐文件核对提交 diff，未发现阻断性或需返工的问题。

**独立复核范围：** 实施改动已由用户侧整理为提交 `7ee80fc`（writing studio + 093 四象限 + 计划/状态/审查文档）与 `b109b18`（周报页控件精修）并推送 origin/main，本轮在 HEAD `b109b18`（与远端同步）上复核，`git diff cffff6d..b109b18` 逐文件核对 `task_shell.rs`、`writingStudio.ts`、`WritingStudio.tsx`、`ScenePage.tsx`、`SceneStatusStrip.tsx`、`ArtifactExplorer.tsx`、`scenes.ts`、`types.ts`、`scene_overview.rs` 的差异；`runtime/` 零差异。窗口验证全程只用 /tmp 一次性合成数据（本人在 `/tmp/od094-review4b`、display `:194` 重跑），未读取真实成果、知识库或凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：**

| 验证 | 结果 |
| --- | --- |
| `verify_writing_094.py` | **98 / 98 通过** |
| 窗口验证 `preview_writing_094.py`（`:194`，/tmp/od094-review4b） | **passed: true**，全部 21 项 facts 通过 |
| 回归 `verify_weekly_091/092/093.py` | **67/67、29/29、105/105**（093 含 S10 新增 6 项） |
| `tsc --noEmit` / `npm run build` | 均通过（仅既有 >500 kB chunk 提示） |
| `pytest tests -q` | **89 passed，30 subtests passed** |
| `cargo test --offline`（本轮早前在 20:04–20:05 跑完，日志 /tmp/od094-cargo.log） | **lib 358 passed**（含 frontmatter 新增 2 项）+ 集成 2+13+6+5+10+3+1 全通过，0 failed |

**窗口验证关键 facts：** 三组阶段计数 草稿 1 / 成文 4 / 已发布 1，合计 6 = 成果总数（含无 frontmatter、非法值两个确定性缺省样本）；类型标签（AI 知识 / blog / 网站内容 / 学习心得 / 未分类）齐全；「继续创作」取 frontmatter 标题；90 天提醒与徽标出现且点击后只留超期条目；草稿筛选只留草稿；灵感架点击回首页并选中对应会话；窄窗 1000px 单列顺序 继续创作→灵感架→状态管线→成果文档库，无横向溢出。宽窗与窄窗截图人工目视复核：暖纸底 + 墨色正文 + 青绿强调，与周报页的紫青配色区分明显，骨架与 093 体系统一，符合用户授权口径「区别于周报分页但整体统一」。

**逐文件核对要点：** `task_shell.rs` 只对 `material-writing` 场景解析 frontmatter，仅读文件前 16 KiB，状态缺省/非法→finished、类型非法→uncategorized，读失败不报错不丢条目，不缓存不回写（含「列表解析不能改文件一个字节」的只读断言测试）；`writingStudio.ts` 为纯函数，缺省口径常量化（`STATUS_DEFAULT_NOTE`），时间判断由调用方注入保证确定性；`WritingStudio.tsx` 不承载编辑器与对话，打开成果只读宿主文件；`ArtifactExplorer.tsx` 的 `showTypeLabels`/`stalePaths`/`accent` 均为可选参数，weekly/project 默认行为不变；`SceneStatusStrip.tsx` 写作四卡只在 writing 分支渲染，读不到数据显示「—」而不写 0。

**授权与计划文字不一致的裁定：** STATUS.md 记录的用户授权原文「授权执行 OD-ARCH-094，UI 视觉设计要区别于周报分页，颜色配色等有差异，但整体要有统一性」明确且在先；PLANS.md 标题「计划草案，待用户批准」属文档滞后，本轮已同步修正为「已批准实施，审查通过」。实施未超出计划范围（未做编辑器、创作统计图表、AI 复用建议、灵感持久化格式、素材回链）。

**已知限制（如实登记，不构成阻断）：** ① frontmatter 解析是本项目自用的小集合解析，不是通用 YAML，嵌套结构不支持；超过 16 KiB 按「无 frontmatter」缺省处理。② 视觉验证仅截图人工查看，未做深色主题截图与像素级对照；建议用户在实际显示器上打开写作页做一次主观确认。③ 灵感架是场景会话列表的展示层，没有独立的「待用」标记格式。④ 90 天判定基于文件 mtime，外部编辑工具触碰文件会刷新时间。

**范围外改动登记：** 工作区当前还有一批未提交的 OD-ARCH-093 S10 改动（待确认入口并入逾期卡、分类下拉 portal 定位等，STATUS 已记录为已实施待审查），属 094 之外的独立工作项，本轮原样保留未动；本轮回归（091/092/093、tsc、build、pytest、`:194` 窗口验证）均在包含该批改动的树上通过，说明两者共存无回归。S10 的正式审查另行进行。

**阶段决定：OD-ARCH-094 验收通过。** 本轮新增的窗口证据（`:194` /tmp/od094-review4b）与静态断言证据已刷新到 `evidence/od-arch-094/`；文档标题同步修正；不提交、不推送，由用户决定。

---

## OD-KB-001 P4 独立验收结论（ARCHITECT，2026-10-27）——通过
## OD-ARCH-093 审查结论（ARCHITECT，2026-10-27）——通过

**结论：通过。** 工作记录页按参考图重排（S01–S08，含四次设计修订）与已实施源码一致，本轮由我本人独立重跑全部验证并逐文件核对 diff，未发现阻断性或需返工的问题。该计划当时未走正式批准流程（STATUS.md 已注明系用户在计划草案状态下直接下令实施），本轮按用户直接指令范围审查；四处偏差均符合用户直接反馈与已记录口径，无需返工。

**独立复核范围：** 起点 HEAD `cffff6d`（main，与 origin/main 同步），改动未提交，全部保留；`runtime/` 零差异。逐文件核对了 `scenes.ts`、`SceneStatusStrip.tsx`、`WeeklyQuadrants.tsx`（新增）、`weeklyQuadrants.ts`（新增）、`ScenePage.tsx`、`ArtifactExplorer.tsx`、`WeeklyTimeline.tsx`、`WeeklyReportPanel.tsx` 的 diff，以及 `verify_weekly_091.py` / `verify_weekly_092.py` 的脚本改动与 `evidence/od-arch-093/` 证据。全程只使用 `/tmp` 合成夹具，未读取任何真实工作记录、成果文档或凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：**

| 验证 | 结果 |
| --- | --- |
| `verify_weekly_093.py` | **96 / 96 通过** |
| `verify_weekly_091.py` 回归 | **67 / 67 通过** |
| `verify_weekly_092.py` 回归 | **29 / 29 通过** |
| `tsc --noEmit` / `npm run build` | 均通过（仅既有 >500 kB chunk 提示） |
| `pytest tests -q` | **89 passed，30 subtests** |
| `cargo test --offline`（`desktop/src-tauri`，带本地 pkgconfig 环境变量） | **397 passed / 0 failed** |

窗口预览 `preview_weekly_093.py` 独立重跑 2 次（`:147` /tmp/od093-review2、`:148` /tmp/od093-review3），facts 一致：三栏宽度就绪、筛选联动（2/7 条目）、归档徽标存在、独立归档卡已移除、窄窗单列且顺序为 四象限 → 本周记录 → 成果文档库 → 历史对话、窄窗无横向溢出、列表视图切换确认（`list_view_confirmed: true`）。4 张截图人工目视复核均符合计划版式，无文字重叠。

**逐文件核对要点：** `scenes.ts` 仅新增可选 `subtitles` 字段；`SceneStatusStrip.tsx` 四张横向指标卡只在 weekly 分支渲染、逾期转 error、读不到数据显 `—` 不写 0；`weeklyQuadrants.ts` 为纯函数确定性归类，四格计数加待归类等于总条目；`ScenePage.tsx` 三栏 1fr/1.4fr/1fr、窄窗 `contents` 拆列、weekly 恒 `includeSubdirs=true`、归档经 `archiveEntries` 通道并入（当周排除、无周报的往期周记录并入）；`ArtifactExplorer.tsx` 的 `compactToolbar` 固定近 30 天滚动窗口、隐藏时间筛选与子目录勾选、计数行注明范围，HARD_CAP 500 分页保留；`WeeklyTimeline.tsx` 仅 `line-clamp-2` + `title` 保全文与筛选联动。091 脚本改动核对：归档断言改为 `archiveEntries` 通道与三栏顺序，与实现一致，改写正当。

**偏差裁定（与 STATUS.md 记录一致，均认可）：** ① 四象限为新增而非保留（改版前源码与全历史均无该区块）；② 顶部四栏未复刻 FOCUS SCORE 等无数据源的指标，改用「本期工作记录 / 进周报条目 / 本周周报 / 逾期」四项有宿主来源的指标；③ 「已完成/总事项」改为「进周报条目」（现有汇总结构不提供逐条完成分母）；④ 历史归档并入成果库并加「归档」徽标（用户 2026-10-27 直接要求）。以上均属用户直接指令或已确认口径，不构成返工项。

**待确认项（不阻断）：** 02 帧左上角出现黑色矩形遮住第一张指标卡局部，两次独立重跑同位复现；仅出现在点击四象限后的帧，01/03/04 帧同区域正常。截图脚本无绘制代码、无鼠标指针（AX invoke）、双帧一致才落盘，判断为 Xvfb 软件渲染下 WebKitGTK 局部未重绘的合成层工件，非应用 UI 缺陷；本环境无法进一步定位，建议在真实显示器上打开工作记录页做一次人工确认后销项。

**口径说明（请用户认可）：** 工作记录页成果库固定按「近 30 天滚动窗口」取数并在计数行注明，与项目跟进、文案写作两场景的自然周/自然月口径不同；这是第四次修订中用户要求精简工具栏的配套选择，其他场景未受影响。

**阶段决定：OD-ARCH-093 验收通过。** 改动仍未提交、未推送，由用户决定提交方式；KB-P4 验收遗留的证据刷新差异原样保留，不属本轮范围。

---


**结论：P4 通过，OD-KB-001 首期 MVP（P1–P4）完成。** 图谱数据层、全局/局部视图、筛选与聚焦、性能降级与窗口验证均符合已批准计划 P4-S01～S05；本轮由我本人独立重跑全部检查、逐项读取实际源码与 diff，未发现阻断性或需返工的问题。P5 及之后的 AI 建议关联、主题聚类、持久图谱数据仍未开始，需用户单独批准。

**独立复核范围：** 当前 HEAD `72e7b05db40bdf4047b7839be9b0cb6abd4bbe32`（`main`，远端 `git@github.com:klzone/omniharness.git`）；未提交改动全部保留，未 reset、未 clean、未提交、未推送、未打标签；终了 `git status --porcelain=v1 -uall` 共 268 项（44 修改、224 未跟踪）。逐行读取了 `knowledgeGraph.ts`（457 行）、`knowledgeGraphLayout.ts`（346 行）、`KnowledgeGraphView.tsx`（829 行）、两个 `*.test.mjs`、`bench/knowledge-graph-p4.mjs`（168 行）、`verify_knowledge_vault_p4.py`（1289 行）与 `KnowledgeVaultPage.tsx` / `window_harness.py` 的 diff；`git diff --check` 通过；`runtime/` 零差异。全部夹具在 `/tmp` 合成目录；未读取真实 Vault、Obsidian 库、正式资料或凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：**

| 验证 | 结果 |
| --- | --- |
| `verify_knowledge_vault_p4.py`（真实窗口，`/tmp/odkb-p4-review`） | **34 checks / 0 failures** |
| P3 回归 | **24 checks / 0 failures** |
| P2 回归 | **33 checks / 0 failures** |
| P1 回归 | **14 checks / 0 failures** |
| 图数据 / 布局单测 | **42 / 42**（26 + 16） |
| `cargo test --offline` | **397 passed / 0 failed** |
| `python3 -m pytest tests -q` | **89 passed，30 subtests passed** |
| `npx tsc --noEmit` / `npm run build` | 均通过；仅既有 >500 kB chunk 提示 |

**证据：** `tests/verification/desktop/evidence/od-kb-001-p4-review/facts.json`（2026-10-27 00:16:05），含 15 张截图、9 份 AX 树快照与 build/host/unit/bench/window 日志。P1–P3 独立回归证据分别在 `evidence/od-kb-001-p{1,2,3}-review/`。

**关键实测事实：**

1. **一致性与范围。** 7 篇合成笔记的节点、实边、虚节点与索引 stats 逐项一致；`external`/`attachment` 不入图；未解析与歧义虚节点可区分且永不提供跳转；筛选与局部范围只改视图，切换视图后索引 generation 不变。
2. **导航。** 鼠标点击、Enter/空格与 AT-SPI `do_action` 三条路径都能打开对应笔记；拖动只改坐标、不打开笔记；缩放只改比例。
3. **筛选与聚焦。** 目录筛选收敛，根目录筛选项独立生效（4 篇 / 4 边 / 2 虚节点 / 隐藏 6 边）；孤立筛选与 `orphans` 一致；局部 1 度 2 篇 3 边、3 度 4 篇 10 边；中心笔记不在索引时明确提示并画空图，不静默退回全局图。
4. **规模与降级。** 2000 篇合成 Vault 初始只布局入链最多的 600 个节点、折叠 1400 并提示；「展开更多」升至 1200 / 折叠 800；折叠状态下仍可打开笔记；真实窗口从切换到图谱到降级提示可见约 4.0 s。纯函数基准：2000 篇首次视图 **80.4 ms**（阈值 400 ms）、筛选 2.1 ms（120 ms）、局部 2.9 ms（150 ms），三档布局均确定性一致。
5. **画布适配。** 修复后按实测画布尺寸布局，小图全部节点落在画布 `[395, 351, 1004, 500]` 内，越界 0。
6. **无副作用。** 两个合成 Vault 全程字节不变（`changed: []`）；不新增文件写入、持久图谱数据或图数据库。

**本轮修复（8 项，已在源码中逐项核实）：** ① 为 AT-SPI `do_action` 增加 `onClick`（仅 `event.detail === 0`）；② 局部中心缺失返回空视图并置 `centerMissing`；③ 折叠计数改用 `foldedNodeIds` 只算当前视图；④ 新增 `layoutBudget` 与「展开更多」（600 → 1200）；⑤ 补根目录筛选的窗口级断言；⑥ `opened` 改 `useMemo`、筛选变化重置预算，消除拖动时的布局重算；⑦ 中心缺失提示补 `role="status"` 与 `aria-label`；⑧ 画布尺寸 effect 依赖 `[graph]` 并挂 `ResizeObserver`，消除节点被裁到画布外。

**设计取舍确认：** SVG + 绝对定位按钮节点而非 Canvas，保证 AT-SPI 可测且在 600 节点预算内性能达标；图数据每次从 P3 快照现算、不落盘，符合「索引是可重建派生数据」；未引入图库、后端布局服务或持久化，符合「无真实触发不加基础设施」。

**未验证边界（如实登记，不构成阻断）：** 未在 Windows/macOS 实机验证；未测高分屏缩放、触屏拖动与键盘 Tab 遍历全图；交互走 XTEST 合成输入，真实鼠标/触控板手感未验证；滚轮缩放只有按钮路径的窗口断言，滚轮事件本身未单独断言；性能基准是纯函数耗时，未覆盖真实浏览器绘制与合成开销，也未测超过 2000 篇或超密边；视觉细节只做截图目视检查，未做像素级规范对照。

**已知遗留（属 P3 语义，不在 P4 范围）：** `vault_index::resolve_target` 在源笔记位于子目录、目标写成嵌套全路径且省略 `.md` 时无法解析（如 `d0/note-0000.md` 中的 `[[d0/note-0001]]`）。P4 夹具使用完整 `.md` 路径绕开，**不能据此宣称短路径已支持**。是否需要修复计划由用户决定。

**阶段决定：P4 验收通过，首期 MVP（P1–P4）完成。** 进入 P5 需用户对 AI 范围与材料授权单独批准；本轮不制定、不实施 P5。当前改动仍未提交、未推送。

**审定补记（2026-10-27，提交后复核）：** 上述改动已于 2026-10-27 由用户侧整理为三个提交并推送到 origin/main：`0259211`（双链解析/索引后端，P3 成果）、`de75fc9`（笔记浏览/编辑与图谱前端，P4 成果）、`cffff6d`（P1–P4 证据与阶段记录）。逐文件核对，三个提交的内容与本文已验收的代码、证据范围一致，`runtime/` 零差异。提交后我本人在 HEAD `cffff6d` 上独立复跑：Cargo 全量测试 397 通过 / 0 失败；`tsc --noEmit` 与 `npm run build` 通过；pytest 89 通过（含 30 子测试）；图谱单元测试 42/42；窗口验证 P4 34/34、P1 回归 14/14、P2 回归 33/33、P3 回归 24/24（P2/P3 为提交后本轮补跑）。窗口重跑刷新了部分 P1/P4 证据截图与 facts.json（含 5 张新增证据文件），属验证产物更新，未触及源码。**审定结论维持：P4 通过，首期 MVP（P1–P4）完成。** 原"未提交"表述已过时，以本补记为准。

---


## OD-KB-001 P3 独立验收结论（ARCHITECT，2026-10-26）——通过，开放 P4 计划审查

**结论：P3 通过。** 双链解析、可重建内存索引与关系导航均符合已批准计划与语法矩阵；独立重跑全部通过，未发现阻断或需修复的问题。P4（图谱可视化）计划可制定，实施仍需用户单独批准。

**独立复核范围：** 当前 HEAD `72e7b05db40bdf4047b7839be9b0cb6abd4bbe32`（main，与 origin/main 同步），实施未提交改动全部保留。逐行审查了 `vault_link.rs`（569 行）、`vault_index.rs`（900 行）与 `vault_index/tests.rs`（420 行）、宿主集成测试、`lib.rs` 三个新命令接线、前端关系面板 diff、`docs/knowledge-vault-link-matrix.md` 与 P3/P2 验证脚本；`runtime/` 零差异，`git diff --check` 通过。未读取真实 Vault、业务资料或凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：**

| 验证 | 结果 |
| --- | --- |
| `cargo test --offline`（全量） | **397 passed / 0 failed**（356 lib + 2 + 1 + 13 + 6 + 5 + 10 + 3 + 1，与实施自述逐套件一致） |
| `npx tsc --noEmit` / `npm run build` | 均通过；仅既有 >500 kB chunk 提示 |
| `python3 -m pytest tests -q` | **89 passed，30 subtests passed** |
| `verify_knowledge_vault_p3.py`（真实窗口，重建应用二进制，`/tmp/odkb-p3-review`） | **24 checks / 0 failures**，`facts.json` 由本轮重跑刷新（21:23） |
| P1 回归 `verify_knowledge_vault_p1.py` | **14 checks / 0 failures**（含 1200 篇冷扫中位 8.685 ms） |
| P2 回归 `verify_knowledge_vault_p2.py` | **33 checks / 0 failures**（含滚动修正后的关键词/标签/属性搜索三项） |

**关键设计点已逐项核实：**

1. **解析层纯函数、不改原文。** 代码围栏/行内代码先等长掩码再匹配（字节偏移不漂移）；Frontmatter 不参与；不支持语法（引用式链接、脚注、HTML 链接、未闭合、超 512 字符目标）逐行去重产出 issue、原文保留；矩阵文档声明与代码行为一致。
2. **目标解析确定性。** 固定九级优先级（路径 → 带扩展名 → 相对来源目录 → 后缀 → 主干 → 别名 → 大小写兜底 → 附件 → unresolved）；同级多候选一律 `ambiguous` 并按字典序列出，绝不任选；附件不作笔记节点。
3. **快照完整性与并发保护。** 索引只在内存；扫描前后两次元数据核对（长度 + 秒级 mtime），不一致即整快照 `incomplete` 并给原因，不发布半成品；读取预算只允许收紧（`.min(32 MiB)`）；单篇 512 KiB / 2000 篇 / 20000 条引用上限；构建在宿主锁外完成后整体原子替换，读命令只见完整快照；换根后不复用旧 generation。
4. **正反链同源一致。** 出链数、反链数、孤立标记均由同一 `links` 集合双向计算；宿主测试断言构建前后知识库目录树字节不变。
5. **跳转规则正确。** 正链定位目标锚点行；回链定位来源笔记的引用行（不是目标）；锚点缺失只回退目标顶部并解释；`unresolved/ambiguous/external/attachment` 不产生跳转——窗口验证均实测通过。

**独立复跑中确认的两个脚本事实（非产品缺陷）：** ① 本轮窗口验证期间 5273 端口的 Vite 进程经核实来自本仓库工作区（`/mnt/data/Code/MyWorkStation/desktop`），`reuse_running_dev_server=True` 未引入跨 worktree 前端风险，但该参数仍依赖端口占用者身份，多 worktree 场景的隐患维持既有登记不变；② 窗口脚本离屏点击回退到 AT-SPI action 触发，行为合理。

**未验证边界（如实登记）：** 索引一致性与窗口证据均来自合成 Vault，未在真实 Obsidian 库抽样；未测 Windows/macOS、大小写不敏感文件系统与网络盘；秒级 mtime 精度下同秒同长度改写无法靠元数据识别（代码注释已披露，指纹供后续复核）；未做大规模性能压测（索引构建无耗时指标，留给 P4 图谱基准一并考虑）；重命名/移动等文件操作与索引构建并发未专项验证。

**阶段决定：P3 验收通过。** 首期 MVP（P1–P4）仅剩图谱可视化；P4 详细计划制定中，获批前不实施。

---

## OD-ARCH-092 复核结论（ARCHITECT 独立复核，2026-10-26）——窗口级通过，待真模型实测

**结论先行：通过。** 工作记录页按用户四点要求改完了：本周时间线与成果文档库同屏两栏、历史归档与会话历史两栏、时间线字号 +2、自动复盘入口移除。我用一次性 Xvfb 打开真实窗口截了图并逐张看过，也自己复跑了全量 Rust、前端构建、三条窗口脚本与两条离线验证。真实模型端到端仍属 OD-ARCH-080 实测清单，不在本轮范围。

**四条需求的落点与我的核查方式：**

| 需求 | 落地位置 | 我的核查方式 |
| --- | --- | --- |
| 1. 时间线与成果库左右两栏，时间线字号 +2 | `ScenePage.tsx` weekly 分支 `lg:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]`；`WeeklyTimeline.tsx` 条目 13.5px / 日期 12px | 真实窗口 `01-weekly-two-column.png`：左 1.6 份宽时间线、右 1 份成果库，5 条记录分落 10/26–10/31，1280 窄窗无横向溢出（`03-narrow-1280.png`） |
| 2. 自动复盘在当前工作流是否还有用 | 无用，已移除界面入口；心跳与规则提醒保留 | 读码：`AnnouncementBar` 的复盘开关 / 频率 / 立即复盘 / 建议行与 `runReview` 调用链已删；`SettingsPanel` 复盘分组已删。`verify_weekly_092.py` 的 `no_review_cadence_ui` / `heartbeat_kept` / `rule_reminder_kept` / `backend_review_contract_kept` 四项均通过 |
| 3. 四个模块按设计规范重设 | 指标带（`SceneStatusStrip`）+ 时间线 + 周报状态行 + 成果库，统一 `bg-white/80 border-white/90 rounded-2xl` 卡体系 | 截图逐项对照：指标为一列「小标题 + 数字 + 灰字」，动作按钮只在周报项出现，异常项才着色 |
| 4. 历史归档与会话历史两栏 | `ScenePage.tsx` `lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]`，归档在左、会话在右 | 截图：两卡同排，会话仍默认收起，展开在自身卡内滚动（`max-h` 限制），不下推整页 |

**指标口径是这轮最容易被做错的地方，我单独核对了。** 「本期工作记录」原来的数据源是 `overview.artifacts_this_week`（成果文件数），用合成 5 条记录时它会显示 0，与用户「我记了几条」的直觉不符。现在改为时间线解析出的 `entryCount`，`verify_weekly_092.py` 的 `entry_count_five` / `metric_matches_timeline` / `empty_entry_count_zero` / `undated_lines_not_counted` 四项覆盖了 5 条、空文件、无日期行三种情形。截图实测显示「5 条」，与左栏 `2026-W44 · 5 条` 一致。

**我独立复跑的结果（本人执行，非采信 STATUS.md）：**

| 验证 | 实际结果 |
| --- | --- |
| `npx tsc --noEmit` | exit 0，0 错 |
| `npm run build` | 通过 |
| `cargo test --offline`（全量） | 299 passed / 0 failed |
| `python3 -m pytest tests -q` | 89 passed / 30 subtests |
| `verify_weekly_091.py` | 67/67 |
| `verify_weekly_092.py` | 29/29 |
| `verify_ui_088.py`（真实窗口） | 30/30 |
| `verify_ui_084.py`（真实窗口） | 全通过 |
| `verify_scene_overview_060.py`（真实窗口） | 通过 |
| `preview_weekly_092.py`（真实窗口） | 三帧落 `evidence/od-arch-092/` |

**审查中改掉的两个真实缺陷（我自己动手，未派工）：**

1. **指标带右侧重叠。** 首版截图 `evidence/od-arch-088/02-scene.png` 里「本期工作记录」独占整行，右侧「本周周报 未生成」与「去生成 / 逾期 0 条」压在一起。根因是 `Metric` 自带 `flex-1` 但没有等宽外层，第一个子项把剩余宽度全吃掉。改法是每项外包 `div.flex-1 min-w-0`、动作按钮从标签行下沉到说明行右端、分隔线从 `/25` 提到 `/70`，重截确认三列等宽。
2. **已验收入口文案被我改掉。** `threadsSection` 的「去对话 · 新开」一度改成「新开对话」，`verify_ui_088.py` 靠 `find(nodes, "去对话")` 进场景页，于是 4 项连锁失败。恢复原文案后 30/30。这条说明：已验收脚本里的可见文案断言等于契约，改版要绕开它。

**顺带修好的验证脚本环境缺陷（不是本轮功能引入，但会挡住任何窗口复验）：**

- `verify_scene_overview_060.py` 缺一次性 `dbus-daemon`，窗口根本起不来（`app.log` 为空、`find_window` 超时）。
- 同一脚本自起 vite，5273 被占时直接崩在 `Port 5273 is already in use`；改为 `reuse_running_dev_server=True`。
- 同一脚本没隔离 `OMNIDESK_KNOWLEDGE_ROOT`，会读到用户真实工作记录；已补指向一次性根（091 的教训）。
- 列表视图切换用坐标点击会落到相邻「卡片」按钮上，改为按 AX 名称「列表」定位后再点击，`facts.json` 记 `switched_via_ax=true`。

**代码审查要点：**

1. `ScenePage.tsx` 的 `tab` 收窄成 `"progress" | "artifacts"`，weekly 分支不再读写它，两个读取 effect 的依赖也去掉了 `tab`。少这一处，切换场景会把 weekly 的页签状态带进项目页。
2. 「待确认写入」从常驻卡降级为有记录才出现的窄条，但 `PendingWriteCard` 展开逻辑与后端 `pending_writes.rs` 的 24 小时过期、确认/放弃语义完全没动。界面收起来，安全契约还在。
3. 时间线等解析仍是「认不出的行原样保留」，不因版式改动丢用户记录。

**未覆盖的事项：** 真实模型端到端（OD-ARCH-080 清单）待用户实测；`preview_weekly_092.py` 的「展开待确认」帧因 AX 未命中「待确认写入」而未取到（同一交互在 060 里已用坐标点击实测通过，2 条 pending / 1 条过期）。

---

## OD-ARCH-091 复核结论（ARCHITECT 独立复核，2026-10-26）——脚本级与窗口级通过，待真模型实测

**结论先行：** 周报分页的三块都做出来了：本周按天时间线替代了原来的碎片胶囊，报告状态卡置顶，历史周报按月归档。我自己复跑了 Rust 全量与离线验证，也用一次性 Xvfb 打开真实窗口截了图，**通过**。真实模型端到端仍属 OD-ARCH-080 实测清单，未在本轮范围内。

**用户四条需求的落点：**

| 需求 | 落地位置 | 我的核查方式 |
| --- | --- | --- |
| 分页里能看到本周记了什么 | `weekTimeline.ts` 解析周文件 → `WeeklyTimeline.tsx` 按日铺排 | 真实窗口截图 `01-weekly-timeline.png`：5 条合成记录分落 10/26–10/31，空日显示灰点占位 |
| 周报状态不再要去首页猜 | `WeeklyReportStatus` 三态（未生成 / 草稿 / 已归档），确定性文件检查 | 读码 + `verify_weekly_091.py` 状态判定用例；窗口里观察到「本周还没生成周报」态 |
| 历史周报按周归档 | `worklog.rs::list_reports` + `WeeklyArchive` 按月分组 | 真实窗口截图 `02-weekly-history.png`：历史归档 2 份周报，W43 / W42 |
| 「转周报」不替用户花模型调用 | `requestNewThread(tag, prompt)` 只预填输入框，不自动发送 | 读码：`HomeChat` 用 `useState(initialInput)` 预填；发送只能由用户触发 |

**我独立复跑的结果（本人执行，非采信 STATUS.md）：**

| 验证 | 实际结果 |
| --- | --- |
| `cargo test --offline`（全量） | exit 0，主套 279 passed / 0 failed，其余套件 20 passed / 0 failed |
| `npx tsc --noEmit` | exit 0，0 错 |
| `verify_weekly_091.py` | `cases=65 passed=65 failed=0` |
| `python3 -m pytest tests -q` | `89 passed, 30 subtests passed` |
| `verify_ui_088.py`（真实窗口） | `checks=30 passed=30 failed=0` |
| `preview_weekly_091.py`（真实窗口） | 两帧截图落 `evidence/od-arch-091/` |

**088 那 3 项失败的判定：不是 091 引入，是验证脚本的既有缺陷。** 三处失败全在第二次提问：合成端点用 `tool_call_trigger` 决定是否返回提问，但它只在「最后一条用户消息的前 200 字符」里找触发词；089 把场景提示词改成「规则在前、用户原话在末尾」之后，`OD088-2` 落在 200 字符之外，端点就当成普通对话直接回了「合成继续完成」。截图与请求记录都印证这一点：第二次请求 `has_tool_message=true`、`text_chars=6963`，内容是随手记缺项提示词。修法是给 `_last_user_text` 加可选 `limit`，判定用全文、日志仍截断，原样重跑即 30/30。**该改动只影响测试用合成端点，未触及产品代码。**

**审查中发现的一处严重问题（已修，需用户知情）：** `verify_ui_088.py` 与我的预览脚本一开始只隔离了成果目录，没隔离库根。工作记录写的是库根，所以跑 088 时合成指令真的被 `worklog_append` 写进了 `~/OmniDesk/库/工作记录/2026-W44.md`，累计 8 条 `synthetic …` 条目。发现于预览截图：时间线里出现了这些合成行。处理过程：

1. 立刻把那张含真实内容的截图移出仓库（`/tmp/od091-leaky-shot.png`），不进证据目录；
2. 备份原文件到 `/tmp/od-w44-backup-20261026022816.md`，按「只删合成块、保留用户原记录」定点清理，清理后只剩用户那条「明天下午专题课…」；
3. 给 `verify_ui_088.py` 与 `preview_weekly_091.py` 都补 `OMNIDESK_KNOWLEDGE_ROOT` 指向一次性根；
4. 复跑 088 与预览，并核对真实工作记录文件哈希复跑前后一致（`32c54910…`）。

这条教训要留住：**任何会触发 `worklog_append` 的窗口脚本都必须隔离库根，只设成果目录不够。**

**代码审查要点：**

1. 时间线解析对认不出的行只做「原样保留」（`raw` 数组）而不丢弃。少显示一条用户写下的记录，比显示得难看严重得多。
2. `list_reports` / `read_report` 都先校验文件名形状（`YYYY-Www-周报[-N].md`）；`read_report` 对 `../../etc/passwd` 直接拒绝且单测覆盖，前端只传文件名、不拼路径。
3. `WeeklyTimeline` 与两个面板的按钮都只做跳转，不写文件；「去生成周报」只预填输入框，符合「不替用户花掉一次模型调用」的既有口径。
4. 状态卡三态来自文件检查而不是模型自述，模型不可用时仍能显示真实状态。

**证据等级：** 文件事实（逐文件读码）+ 静态检查（tsc / build / 源码断言）+ 脚本运行（cargo 279、pytest 89、四个验证脚本）+ 真实窗口（一次性 Xvfb 两帧截图）+ 真实文件系统行为（隔离临时目录、合成文本）。

**未验证事项：**

1. **真实模型端到端未跑**：周报生成质量、缺项追问措辞、以及对话里说「生成周报」的实际表现，仍待用户按 OD-ARCH-080 清单实测。
2. **三态切换只认证了一态**：窗口里看到的是「未生成」；「草稿已存在」「已归档」两态只有离线脚本与代码证据。
3. **真实库根下的时间线渲染未看**：预览用的是合成周文件；真实文件的条目长度、多行续文在界面里的观感未观测。
4. **跨周记录（`outOfWeek`）未在窗口中出现**：只有解析器用例覆盖。

**工作区状态：** HEAD 仍为 `0d17b1f`；`runtime/` 零差异；未提交、未推送、未打标签；084–090 既有改动原样保留。

## OD-ARCH-089 复核结论（ARCHITECT 独立复核，2026-10-26）——脚本级通过，待真模型实测验收

**结论先行：** 用户要求的三件事都落地了：认不认周报由输入决定、「随手记」先写进文件再问缺项、Skill 拆成随手记和周报两个。脚本级验证 61 项全过，Rust 全量 294 项 0 失败，前端构建通过，防覆盖机制在单测里被证死。**但真实窗口和真实模型本轮都没跑**，所以这是「代码与脚本级通过」，不是最终验收。

**用户四条需求的落点：**

| 需求 | 落地位置 | 我的核查方式 |
| --- | --- | --- |
| 没有明确要周报就不进周报流程 | `workIntent.ts`：只认「周报」二字，或「本周/这周/上周 + 总结/汇总/小结/回顾/梳理」同时出现 | 编译真实 TS 跑 7 条 report 输入与 8 条 note 输入，含全角、空格、标点混写 |
| 平时输入一律按随手记逐条增量追加 | `HomeChat.tsx` 先调 `worklog_append` 再叫模型；`worklog.rs` 只用 `append(true)` | 读码 + 单测：连写两条旧字节前缀不变；同日第二次另起「（续）」段 |
| 只分析缺时间/地点/人物/事件/影响/感受 | `conversationPrompt.ts` note 分支 + `memory-agent` 的「零、缺项六要素」 | 断言六要素齐全、五段周报章节一个不出现、写作铁律不出现 |
| 去掉两个标签、用关键词当指令 | `HomeChat.tsx` 删 `noteMode` 与按钮块 | 源码断言无 `noteMode`、无两个按钮文案 |

**Skill 拆分的实际账（我亲测 `len()`）：** memory-agent 1432、memory-report 988、knowledge-search 2133、anysearch 1487。周报注入从 5154 降到 4553，省 601 字符；随手记注入从 1534 降到 1432，省 102 字符，但多背了六要素追问的职责。**计划里原估「随手记 1150、周报 3845」偏乐观**，实际两侧都补了必要小节（六要素、确认门、命名、自检），这点已在 PLANS.md 更正。

**anysearch 的处理正确。** 它移出 weekly 预注入，但仍在 `ALLOWED_SKILLS` 白名单、仍随 `sync-skills.sh` 装进隔离副本、仍在引擎 `available_skills` 里。这符合 OD-ARCH-088 已确认的口径：未预注入不等于不可用。我专门加断言守这一条，防止以后有人把它从白名单里也删掉。

**我在复核中发现并修掉的一处问题：** `verify_skill_bindings_082.py` 每跑一次都会重写 `evidence/od-arch-082/scene-injection.json`，本轮复跑已经把它改脏了。这违反「不改写历史证据」的项目规则。处理方式是把证据落点改到 `evidence/od-arch-089/`，再用 `git checkout --` 把 082 那份还原（`git status` 已确认它回到 HEAD 版本）。082 脚本本身对 089 配置的断言改的是「当前配置」而不是「历史事实」，属于跟随配置变更，认可。

**我独立复跑的结果（本人执行，非采信 STATUS.md）：**

| 验证 | 实际结果 |
| --- | --- |
| `cargo test --offline`（全量） | exit 0；主套 274 passed 0 failed，其余 7 套件 20 passed 0 failed，合计 294 passed |
| `npm run build` | exit 0，`built in 898ms`，仅既有 chunk 体积警告 |
| `python3 -m pytest tests -q` | exit 0，`89 passed, 30 subtests passed` |
| `verify_worklog_089.py` | exit 0，`61/61` |
| `verify_skill_bindings_082.py` | exit 0，`38/38` |
| `verify_profile_rules_068.py` | exit 0，`25/25` |
| `verify_intent_088.py` | exit 0，`22/22` |
| `verify_worklog_085.py` | exit 0，`122/122` |
| `verify_env_085.py` | exit 0，`24 passed / 1 obstacle`（本机无 obsidian_cli，既有） |
| `sync-skills.sh --check` | exit 0，无漂移 |
| `git status --porcelain runtime/` | 0 行 |

**代码审查要点：**

1. `worklog.rs` 的防覆盖不是口头承诺：写入后读回文件，断言旧内容仍是新内容的前缀、且新长度等于旧长加新增长度，不等就报错。这个自证比只用一个 flag 可靠。周标签用 `YYYY-Www` 严格校验且周次限 01–53，路径拼接前就挡住了穿越。
2. 落盘失败时的用户可见文案是「这条没有记下来 …不会假装已经记录」，并且在 `return false` 之前就置了非忙碌态——不会出现按钮卡死的状态。模型调用失败但已落盘时，界面把「已写入路径」和失败原因分开说。这两条正是上一轮「卡死」事故的直接教训。
3. 红线扫描在掩码之前、对用户原文做，命中即拒绝且错误信息只回标签与次数、不回显原文。与既有 `save_offline_artifact` 口径一致。
4. 意图判定偏保守：多记一条随手记不会丢事实，误进周报流程会污染材料，所以词表只收「周报」本身与带周范围限定的总结措辞，「总结一下这次会议」仍是随手记。判定结果在回复上方写明，误判用户当场能看见。
5. `MAX_SKILL_BINDINGS_PER_SCENE`（3）与 `max_injected`（3）都没动，weekly 正好三个绑定，没有靠调大上限来塞新 Skill。

**证据等级：** 文件事实（逐文件对照读码）+ 静态检查（build/tsc、源码断言）+ 脚本运行（cargo 294、pytest 89、五个验证脚本）+ Rust 单测的真实文件系统行为（隔离临时目录，合成文本）。

**未验证事项（如实登记）：**

1. **真实桌面窗口未跑**：两个标签在真实界面里是否确实消失、场景切换/发送/附件是否受影响，只有源码断言。验收标准第 2 条缺窗口证据。
2. **真实模型输出未跑**：模型拿到新提示词后是否稳定只问缺项、不复述不扩写，以及说「生成周报」时周报质量如何，均未观测。这是 OD-ARCH-080 实测清单的一部分，需用户实测。
3. **真实窗口落盘未跑**：连写两条随手记只有 Rust 单测证据，没有窗口级证据；「（续）」段在真实界面里的呈现也未看过。
4. **空周报路径未观测**：本周无记录时指令要求如实说明「本周还没有记录」，但只有提示词断言，模型是否照做未验证。

**工作区状态：** HEAD 仍为 `0d17b1f`；`runtime/` 零差异；本轮未提交、未推送、未打标签；084–088 的既有改动原样保留。


## OD-ARCH-088 复核结论（ARCHITECT 独立复核，2026-10-25）——通过

**结论先行：** 用户报告的「工作记录场景卡死」不是死锁，而是模型发起了一条结构化提问（`user-questions/request`）、外壳没有回答通道，界面只能停在「生成中」。本轮把提问→回答→继续→取消/终态整条链路接通，同时补上场景 intent 透传与 Skill 绑定可见性提示。三项验收标准全部达成，**通过**。

**根因（ARCHITECT 独立查证，非采信 BUILDER 自述）：**

1. 任务状态文件 `~/OmniDesk/app/engine-state/omni-b80f4367-*.json` 实测 `intent=material_writing`、`state=interrupted`；会话日志 `session-949cefc3-*.jsonl.zstd` 共 32 行，seq19 模型用 `skill` 工具加载 `memory-agent` 成功，seq29 调用 `ask_user_question`（“初稿是否先追加到 库/工作记录/2026-W43.md？”），此后事件流无新帧——任务在等一个没人能答的提问。
2. 旧 `web_api_adapter.py` 除 `approval/request` 外所有 waterfall 一律走 `_on_engine_ask()`，只发 `PROGRESS kind=engine_ask`、注释写明「不自动应答」；前端渲染成「未被识别的确认」。应答协议本身已存在（`$events/result` + `{answers:[{id,selected,custom?}]}`），缺的是接线。
3. **三个 Skill 不是「没绑上就不可用」。** 用户实盘 `~/OmniDesk/app/profiles/discipline.rules.json` 是 `omnidesk.profile-rules.v1`、无 `skill_bindings`；出厂包 `samples/profiles/discipline.rules.json` 才是 v2（weekly 绑 memory-agent / knowledge-search / anysearch，`max_injected: 3`）。技能始终在引擎 `available_skills` 清单里，本次就是模型自行加载的——**未预注入 ≠ 不可用**。
4. 附带真实缺陷：`submitTask()` 未传 intent，工作记录任务被记成 `material_writing`。

**实现审查（已逐文件读 diff）：**

- `engine/omnidesk_engine/web_api_adapter.py`：新增内存索引 `_question_index` / `_questions_by_task`（不落盘）；`user-questions/request` 走 `_on_question_request()` 发 `PROGRESS kind=question_request`（含 `question_id/questions/answerable/violations/note`），其余未知 waterfall 仍走 `_on_engine_ask()`；`respond_question()` 复用既有 `_ApprovalChannel` 与 `$events/result`，`outcome={"kind":"result","value":{"answers":[...]}}`；`multi_select` 与 `multiSelect` 双拼写兼容；非法/超限抛 `INVALID_ARGUMENT` 并**保留**待答项；`cancel` 帧与任务终态各发一条 `question_closed`（`cancelled` / `task_terminal`）；关闭时清空索引。上限 8 题 / 题面与详情 2000 / 选项 12 且标签 200 / 自定义 4000，超限整条不可答、不静默截断。
- `engine/omnidesk_engine/serve.py`：新增 `omni.question_answer`；适配器无 `respond_question` 时返回 `METHOD_NOT_FOUND`，与审批同口径。
- `desktop/src-tauri/src/engine_bridge.rs`、`lib.rs`：新增桥调用与 `engine_question_answer` 命令并注册，错误沿用既有 `engine_error` 归一化。
- `desktop/src/shell/QuestionCard.tsx`（新增）：pending → submitting → answered / failed / closed 状态机；点选项只改本地选择，必须点「提交回答」；失败保留卡片并显示真实原因；`role="group"` + `aria-label` 兼容 WebKitGTK；Enter 提交、Esc 不提交。
- `desktop/src/shell/HomeChat.tsx`：订阅三种进度事件、按任务分组渲染；`respondQuestion()` 只有宿主返回收据才置 answered；`submitTask()` 补 `intent: isSceneKey(activeTag) ? SCENES[activeTag].scenario : undefined`；Skill 未预注入提示每会话一次。
- `desktop/src/SettingsPanel.tsx`：规则资产页在 `skill_bindings` 为空时显示 v1 提示，只提示、不自动改写规则文件。
- 协议已对照 runtime 核实：`gateway/src/index.ts` 的 `outcome.kind==='result'` → `settleRemoteEvent`；答案 schema `projection.ts` 为 `.strict()`；线上题目字段是 snake_case `multi_select`，投影后才是 `multiSelect`。

**ARCHITECT 独立复跑（本人执行，非采信 STATUS.md）：**

| 验证 | 实际结果 |
| --- | --- |
| `python3 -m pytest tests -q` | exit 0，`89 passed, 30 subtests passed` |
| `tsc --noEmit` | exit 0，无输出 |
| `npm run build` | exit 0，`✓ built in 824ms`（仅既有 chunk 体积警告） |
| `cargo test --offline`（全量） | 主套 267 passed / 0 failed，其余套件 0 failed，`FAILED` 计数 0 |
| `verify_intent_088.py` | 22/22，exit 0 |
| `verify_ui_088.py --display :139 --root /tmp/od088-arch` | **30/30，failures=[]，exit 0** |
| `verify_skill_bindings_082.py` | 35/35 |
| `bash scripts/sync-skills.sh --check` | exit 0，7 个 Skill 无漂移 |
| `git status --porcelain runtime/` | 0 行 |
| `git diff --check` | 无输出 |

**窗口 30 项覆盖（隔离数据根 + 合成模型端点，真实 Tauri 窗口）：** 提问卡出现 → 未选时提交按钮 `SENSITIVE=False` → 点选项不自动提交 → 提交后卡片显示已答 → 第二轮请求 `has_tool_message=true`（答案确实回到引擎）→ 任务状态 `intent=weekly_report` → 取消后卡片关闭且不可再答 → 状态文件与 `app.log` 无题干/选项/答案 → 设置页 v1 提示且规则文件未被改写 → 结束时应用存活。`facts.json` 30 项 `result=pass`。

**证据等级：** 文件事实（读码对照 diff）+ 静态检查（tsc/build）+ 脚本运行（pytest、cargo、三个验证脚本）+ 真实桌面窗口与合成端点。**真实模型输出本轮未做**——OD-ARCH-080 清单的端到端实测仍待用户执行。

**我在复核中改动的文件（仅测试基线，未碰产品代码）：** `verify_worklog_085.py` 与 `verify_env_085.py` 原先把整个 `engine/` 写死为冻结，与 088 获批解冻冲突（曾 117/119、21/22）。改为**声明式白名单**：只放行 088 获批的 `serve.py` 与 `web_api_adapter.py`，其余 `engine/` 仍冻结，`desktop/src` 侧只增 `QuestionCard.tsx`。改后复跑 `verify_worklog_085.py` 119/119 exit 0、`verify_env_085.py` 22/22 failed=0（唯一 obstacle 是本机无 `obsidian_cli`，属预期降级事实）。这是收紧而非放水的等价改动。

**待用户裁定的新问题（本轮未擅自改）：** 当前 Profile 的 `tool-ask-user` 没有配 `mode: timed`，DSH 默认走 `legacy`，**legacy 模式没有超时自动继续**（只有 `timed` 才有）。用户看到的「静默 120 秒」是外壳提示阈值，不是引擎超时。088 已保证用户能回答或点「取消本次任务」脱身、终态时卡片落成不可答；是否改成 `timed` 让超时后模型自动继续，属于产品决策，需另立计划并获批。

**未验证事项（如实登记）：** ①`detail` 字段 DSH 两套 schema 均不转发（runtime 既有行为），窗口只能验 header/题干，`detail` 渲染仅由适配层单测与组件代码覆盖；②timed 模式的超时帧未实测；③多题、多选、自定义文本在真实窗口中未逐项实操（单测覆盖）。

**工作区状态：** HEAD 仍为 `0d17b1f`；`runtime/` 零差异；本轮未提交、未推送、未打标签；084/085/086/087 遗留改动原样保留。

## OD-ARCH-086 复核结论（ARCHITECT 独立复核，2026-10-25）——通过

**独立复跑证据**（本复核者亲跑，非采信 STATUS.md）：

- cargo 全量：passed=287 failed=0（desktop/src-tauri，含 toolchain env）。
- npm run build：通过（仅既有的 chunk 体积警告）。
- sync-skills.sh --check：无漂移；隔离根 skills 目录已含 anysearch 薄包装。
- 回归复跑：082=35/35 pass、068=25/25 pass、085 环境探测=22/22 pass。
- runtime/、engine/ 零差异：git status 实测 0 行。
- 旧 YvanAI 路径零接触：find newer PLANS.md 无结果。
- 库根目录骨架存在（~/OmniDesk/库/{知识库/概念,inbox}）。

**代码审查要点**：

1. 白名单七项 + weekly 三绑定（=上限3），diff 干净，测试断言到位。
2. settings.rs：library_root 带 #[serde(default)]，向后兼容有专项单测（旧文件加载、其他设置不被重置）；set_library_root 沿用成果目录同一套边界校验（绝对路径、仓库外、禁区、创建后复核真实落点、写探针），无自造第二套校验。
3. task_shell.rs：knowledge_root 默认切到 ~/OmniDesk/库（env 覆盖优先级不变）；reference_roots 去重、顺序稳定；lib.rs 三个 @ 引用命令均改用生效库根。
4. 082 验证脚本改为从 profile_rules.rs 源码解析白名单（消除手抄清单过期问题），并新增 anysearch 断言——是加强而非删测试，认可。
5. anysearch 薄包装 54 行：不复制上游脚本、凭据红线与敏感主题禁检索写入正文、不可用时如实回报；冒烟证据 3 次真实联网返回（匿名额度）。
6. 设置页 UI 有 screenshots-design-inspiration 参考记录（design-reference.md），符合 AGENTS.md 派工附加要求。

**限制与未验证项**（如实记录）：

- L1 库根目前只接入 @ 引用；成果档案列表仍只读成果根；Skill 写入路径不跟随库根改动。设置页文案已如实注明。后续按需求再定是否打通。
- L2 086 真实窗口验收（22 项）为 BUILDER 环境证据（facts.json 20:58 全过，含截图与无障碍树）；ARCHITECT 复跑该脚本因显示环境未复现成功（找不到设置入口），后端逻辑已用单测与回归独立覆盖，此项按 BUILDER 证据采信并标注。
- L3 anysearch 仅验证 CLI 直调与同步发现，真实 DSH 模型会话中自动调用未验证——待用户实测工作记录场景时观察。
- L4 流程偏差：086 主体实现在派工前已在工作区，本轮 BUILDER 的工作是核对、补测试与重跑验证，未按模板另派工。验证充分，验收不受影响；登记为流程记录，不要求返工。

**验收结论：通过。** PLANS.md 状态改为「已验收通过」。

## OD-ARCH-085 独立复核（2026-10-25，ARCHITECT）

**结论：通过（附两项待用户裁定事项，不阻塞本轮验收）。**

**独立验证（本人复跑，非采信自述）：**

- verify_worklog_085.py 复跑 119/119 pass，exit 0；verify_env_085.py 复跑 22/22 pass，exit 0（含 2 项受阻事实记录）。
- sync-skills.sh --check：仓库 Skill 与数据根副本一致，无漂移。
- Git：HEAD 0d17b1f 未动；runtime/ desktop/src-tauri/ engine/ 复查 0 行差异；工作区 16 项 = 开工前 12 项 + 本轮 4 项（memory-agent/knowledge-search SKILL.md、discipline.rules.json、phase-worklog 目录），范围与获批计划一致。
- 数据根 ~/OmniDesk/库/ 骨架实测存在（工作记录/周报/知识库/概念/方法/inbox + 131 字节索引头），未迁移未覆盖任何既有文件。

**实现质量抽查（文件事实）：**

- memory-agent：只追加/禁重写为硬约束条款；提炼强度按输入形状分档（一句话不硬扩）；去向四选一自定；周报 Report Mode 明确「全文展示→用户确认→才落盘」，同名不覆盖。符合六条定调。
- knowledge-search：两跳检索写成硬约束（禁 ls/glob/find/整目录读）；无命中如实拒答；逐字引用回 sources 原文；卡片 ≤300 字、frontmatter 带 sources/date、写卡同步索引。符合索引卡+两跳设计。

**两项裁定（留待用户，BUILDER 处理正确未越权）：**

1. anysearch 未绑定：宿主 profile_rules.rs 六项技能白名单是编译期固定清单，含 anysearch 会被整份规则包判非法（BUILDER 以刻意触发测试留证）。选项：A) 下轮解冻 src-tauri/ 扩白名单并全量回归；B) 接受不绑定（websearch 已可用）。
2. 库根与宿主双根不一致：Skill 写死 ~/OmniDesk/库/，宿主成果根仍是 workspace/artifacts、知识 @ 引用根仍指向 ~/YvanAI/knowledge，且设置无库根字段——模型写入不会自动出现在应用成果列表/引用里。需下轮做库根绑定（设置字段或环境变量）。

**遗留未验证**：真实模型提炼/周报草稿质量；真机 Obsidian CLI（本机无 Obsidian，降级路径为实测路径）；实盘规则包升级（用户需在设置页恢复一次默认规则包，新绑定才生效）；084 界面回归未重跑。

## OD-ARCH-084 终审结论：**通过（附两项遗留限制）**（ARCHITECT，2026-10-25）

- **结论先行：** 用户反馈的三个问题（搜不到时视口不跟随命中词、上下箭头切换不顺、搜索框无法保持焦点）在真实 Tauri 窗口里均已修复；ARCHITECT 独立复跑 `verify_ui_084.py` **21 项全 pass、failures=[]、exit 0**，并逐张目视核对了关键截图。本轮验收通过。
- **根因（ARCHITECT 用真实窗口探针定位，非采信 BUILDER 自述）：** ①旧实现只记到消息级、滚动目标是整条消息，长消息末尾命中看不见；②同一消息内多处命中共享同一滚动目标，切换是幂等操作；③当前项无独立标识，且箭头按钮抢焦点，键入丢失。三条均在本轮消除。
- **实现（已读码核对）：** `MarkdownText.tsx` 改为递归文本高亮并输出 `data-search-hit` / `data-search-ordinal`，围栏代码块排除；`HomeChat.tsx` 拆成「计数与活跃项标注」与「滚动与短强调」两个 `useLayoutEffect`，滚动目标是命中词本身（视口 32%、≥24px 安全边），并删除整条消息的 ring；`TopNav.tsx` 三按钮 `onMouseDown` 阻止默认行为、改 `aria-label`、0 命中 disabled；`App.tsx` 对 index 做 clamp。
- **审查中我退回并要求修复的两项（均已闭环）：**
  1. **阻塞级：键入正则元字符整窗黑屏。** `escapeRegex` 的替换串写成 `"\\$&"`，JS 里等价于 `$&`，等于没转义；`(`、`[`、`*`、`\\` 会让 `new RegExp` 抛错，React 整棵树卸载。我在 `:135` 复现：亮度 247.4 → 16.3，按 Esc 也不恢复。BUILDER 改为纯 `indexOf` 字面量切分（`splitLiteralMatches`），我另用 node 独立验证 `a.b` 只命字面量、不匹配 `axb`，`(`/`[`/`*`/`\\` 均正常。
  2. **回归级：未搜索态常驻 35vh 空白。** `chat-scroll` 的 `paddingBottom: "35vh"` 无条件生效，而消息更新会把视口贴到 `scrollHeight`，导致正常对话最后一条下方恒有大片空白。已改为 `searchQuery.trim() ? "35vh" : undefined`，并新增实测断言：未搜索态最后一条消息底边 `y=689` 与输入区顶边 `y=700` 间距 **11px**（< 60px）。
- **ARCHITECT 独立复跑（`--display :150 --root /tmp/od084-arch4`，21 项全 pass）：** 唯一命中 `bbox [579,404,723,422]`、视口比例 `0.336`、上下净空 164/333px；8 处命中 y 依次 `413/449/486/522/559/595/632/668`（增量 36–37px）；循环 8→1、1→8 正确；0/0 时两箭头 `SENSITIVE/ENABLED` 均为 False；点 AT-SPI 实测按钮 `[831,33,19,19]` 后计数 1/8→2/8，焦点前/后 `FOCUSED` 均 True，键入 `zzz` 使计数归 0/0；两级像素 1138（当前）/7770（其余）、活跃块严格 1 个、无 >150px 竖线；标题/表格/行内代码命中一致（`1/3`）、围栏排除、两目标滚动差 10745；Esc 后高亮 0；键入 `(` 后区域平均亮度 **244.4**、正文可见；键入 `a.b` 计数 `1/1`；特殊字符后 Esc 干净退出。
- **截图目视核对：** `05-two-level-highlights.png` 当前项饱和琥珀带描边、其余浅琥珀，无竖线；`01-unique-tail-target.png` 长消息尾端命中落在视口内；`06-markdown-target-2.png` 标题与表格命中位置正确；`07-escaped.png` 退出后无高亮。`08-special-paren.png` 键入 `(` 后界面完整。
- **回归（ARCHITECT 实跑）：** `tsc --noEmit` exit 0；`npm run build` exit 0（仅既有 chunk 体积警告）；`pytest tests -q` **75 passed / 14 subtests**；`cargo test --offline -q --lib` **261 passed / 0 failed**；`verify_ui_082.py` **17/17 pass**（`search_highlight_mark_pixels`=1588）；`verify_ui_083.py` **17/17 pass**。082/083 证据目录已用 `git checkout --` 恢复，未留存覆盖；`git diff --check` 干净，`git status --porcelain runtime/` 为 0 行。
- **过程问题（已记录，避免复发）：** 我曾与 BUILDER 并行跑 084，两者写同一 `evidence/od-arch-084/facts.json`，出现过一次因文件被另一进程覆盖而产生的假 FAIL（`special_char_esc_restores_clean_state`）。此后改为独占复跑，结论以上面的 21/21 那次为准。并行运行的中间产物不作为验收依据。
- **未验证事项：** 本轮全部证据来自 Xvfb + 合成数据 + `fake / synthetic-model`，**没有做真实模型端到端实测**（OD-ARCH-080 清单仍未执行）；本轮未提交、未推送、未打标签。

### 遗留限制（不阻断，登记待后续）

1. `verify_ui_082.py` 有两处断言偏松（确认框判据含「确认」兜底、focus 截图只校验文件存在），属既有问题，本轮未改，不影响 084 结论。
2. `TurnRail.tsx` 的 `PREVIEW_H = 96` 与实际卡高（约 120–130px）仍不一致，属 083 遗留，本轮未触碰。

## OD-ARCH-083 终审结论：**通过（附两项遗留限制）**（ARCHITECT，2026-10-25）

- **结论先行：** 三条用户可见要求（自有 Logo、融入顶栏、直角预览卡）在真实窗口中达成；验证脚本经两轮返工后，关键断言均有可复核的测量值与阈值，ARCHITECT 独立复跑 **17 项全 pass、failures=[]、exit 0**。本轮验收通过。
- **实现未动：** 两轮返工只改 `verify_ui_083.py` 与 `evidence/od-arch-083/`（外加 STATUS 记录）。`git diff` 中 `TopNav.tsx` / `TurnRail.tsx` / `theme.css` 仍是我第一次审查时的成品，返工期间无改动。
- **ARCHITECT 独立复跑（`--display :99 --root /tmp/od083-arch-r3`，17 项）：** Logo `32×32`、bbox `[104,27,135,58]`、深底 621px / 彩环 130px、四角为浅色过渡（非直角黑块）、Logo 与文字中线差 `1.5px`；3 个刻度按钮 `help_text` 全空、悬停前后独立 tooltip 节点均为 `0`、`get_description()` 返回「第 1 轮…」；卡片 `300×124`、rect `[984,483,1283,606]`、卡面 `(255,255,255)`、描边 `(221,220,232)` 宽 1px；四角顶点均为边框色、内角纯白、外角背景、两侧 4px 连续；末端卡片底 `588` 到输入区上沿 `757` 净空 `169px`；真按键 Tab → 第 2 轮卡片在场、Shift+Tab → 回第 1 轮卡片仍在（两图差 5375px）。
- **第二轮假阳性已闭合：** 我先跑出的版本里 `Shift+Tab` 会退出轮次条，卡片消失后与「指针远离」截图零差异却被判通过。返工后改为「第 1 轮 → Tab 进第 2 轮 → Shift+Tab 回第 1 轮」，两个方向都断言卡片矩形存在、卡面纯白、描述轮次号匹配，并重截了 `04-turnrail-focus.png` / `04-turnrail-focus-prev.png`。我逐张目视核对了这两张图：卡片在场、内容分别为第 2 轮与第 1 轮。
- **证据完整性：** `script_sha256` `07c121dc…` 与文件实际 sha256 一致；`facts.json` 17 项均带 measured / threshold / desc；`evidence/od-arch-082/` 14 个文件与开工前一致（未覆盖）；`git status --porcelain runtime/` 为 0 行。
- **回归（ARCHITECT 实跑）：** `tsc --noEmit` exit 0；`npm run build` exit 0（仅既有 chunk 体积警告）；`cargo test --offline -q --lib` **261 passed / 0 failed**；`pytest tests -q` **75 passed / 14 subtests**。

### 遗留限制（不阻断，登记待后续迭代）

1. **`TurnRail.tsx:20` 的 `PREVIEW_H = 96` 未改**，新卡实测高 124px，位置夹取仍按旧高估算。3 轮样本下末端卡片距输入区 169px，无裁切；多轮超长会话下仍未覆盖，建议后续改为实测卡高。
2. **输入区上沿与直角测边仍用启发式扫描**（`composer_top_y` 在 700–800 行找首条近白行，失败时兜底 753；卡片四边靠固定取样线 `x=1100` / `y=530`）。当前证据成立，但换个窗口尺寸或主题后需重校；作为回归门禁时应改为基于 DOM 矩形。

### 未验证

- 真实模型端到端实测（OD-ARCH-080 清单）仍未进行，本轮 UI 改动不替代该项。
- HiDPI / 缩放 > 1 下的亚像素边缘未测。

## 第一轮审查记录：**不通过**（实现达成，验证脚本不成立）（ARCHITECT，2026-10-25，已被上方终审取代）

> 保留原因：记录首轮被驳回的具体缺陷，供后续写同类断言时对照。

- **一句话结论：** 顶栏自有 Logo 与直角预览卡两条用户要求，经 ARCHITECT 亲跑的像素与无障碍探测确认**已经做到**；但 BUILDER 提交的 `verify_ui_083.py` 有 5 条断言名不副实或逻辑恒真，`facts.json` 记录的「7 项 100% 通过」不能作为证据。故本轮**判不通过**，返工范围仅限验证脚本与 083 证据，不动已合格的实现。
- **基线：** 分支 main，HEAD `0ac9edf`；工作区 43 项既有未提交改动完整保留；`git status --porcelain runtime/` 为 0 行。本轮未提交、未推送、未打标签；未改 Profile、凭证、模型路由；`evidence/od-arch-082/` 14 个文件与开工前一致（未覆盖）。

### 已达成：两条用户要求（ARCHITECT 独立证据）

- **S1 顶栏 Logo（源码 + 像素）：** `desktop/src/shell/TopNav.tsx:447-457` 用 `<img src="/omnidesk-icon.svg" alt="OmniDesk" draggable={false} decoding="async">` 替换了原 `rounded-full bg-gradient-to-tr` 圆底 + `Icon name="brand"`，样式为 `w-8 h-8 rounded-[7px] ring-1 ring-black/5 shadow-xs object-cover`。真实窗口裁切中 Logo 深色底实测 **32×32**，O+D 环与五节点可辨，外围仅一层极浅细边，无彩色光晕、无多重投影。
- **S1b 融入与过渡：** 整条顶栏顺序为「Logo → OmniDesk → 标准徽标 → IPC ON → 搜索 → 日期 → 按钮」，其余控件颜色与位置未变，未新增分隔线。实测：Logo 与名称文字中线差 **1.5px**；Logo→名称 **11px**、名称→Profile 徽标约 **9px**、徽标→IPC 约 **10px**。与计划「间距等值」相差 1–2px，视觉节奏成立，**判为可接受的小偏差**，不阻断，但须在 STATUS 中如实记录实测值而非「严格等距」。
- **S2 直角预览卡（源码 + 像素 + AX）：** `TurnRail.tsx` 预览卡去掉 `rounded-xl` 与彩色边，改为 `border border-outline-variant/60 bg-surface-container-lowest px-3 py-2.5 shadow-md`，顶部 9.5px 等宽字标「第 N 轮」；`theme.css:192-194` 有 `.turn-rail-tooltip { border-radius: 0 !important; }`。真实窗口悬浮卡 bbox **x984–1283 / y483–611**，四角严格直角（角点及两侧连续描边，角点内侧为卡面 255,255,255），与软件既有白卡+细描边+小号字标语言一致。
- **系统原生提示已消除（AX 级）：** 刻度按钮删除 `title` 后 `get_help_text()` 为空；悬停前后 AX 树中均无独立 tooltip 节点，预览内容只经 `aria-describedby` 进入按钮 `get_description()`。082 截图中叠在卡片上的系统黑色圆角小窗不再出现。
- **键盘路径可用（真实按键）：** 指针移至远处后按 Tab，焦点进入轮次条并弹出第 5 轮卡片；Shift+Tab 切到第 4 轮，焦点与卡片同步移动。此前「Tab 失败」是本机 AT-SPI `FOCUSED` 状态恒空导致的探测误判，不是产品缺陷。
- **边界抽查：** 900 / 700 / 620 高窗口、12–40 轮、长回答三种情形下末端刻度卡片均完整可见，未压住输入区。

### 不通过依据：`verify_ui_083.py` 的断言问题

1. **`tooltip_square_corners` 恒真。** 判定式 `is_square = max(corner_px) <= 255` 对任何 8-bit 像素都成立；且角点取自松散扫描（第 305–314 行先碰到什么算什么），`facts.json` 里记录的角点像素 `[255,255,255]` 实际是卡面白，不是边框，等于用白点证明直角。
2. **`tooltip_style_tokens` 与 `brand_logo_is_omnidesk_asset` 只是 grep 源码字符串**（`".turn-rail-tooltip" in theme_css and "border-radius: 0" in theme_css`、`'src="/omnidesk-icon.svg"' in topnav_src`），既非运行时计算样式，也非像素事实，源码里写了不生效的 CSS 同样会通过。
3. **`no_native_title_tooltip` 未做 AX 断言。** 只 grep `TurnRail.tsx` 有无 `title={`，没有验证运行时按钮确实不暴露 `title`/`help`，也没有验证 `described-by` 仍指向 `role=tooltip`，与派工要求不符。
4. **`tooltip_keyboard_focus` 名不副实。** 实测代码先 `xs.click(700,500)` 再 `xs.click(1298,503)`，比的是「悬停 vs 鼠标点击」，**全程没有按一次 Tab**，无法证明键盘聚焦路径。
5. **Logo 像素采样框错位。** `logo_box = (135,26,172,60)` 与裁切后 Logo 实际位置（x4–35）不重合，`dark_pixels=74 / accent_pixels=60` 属于误打误撞通过。
6. **四角特写图不可复核。** `03-turnrail-tooltip-corners.png` 的底角裁切 `card_y0+56 .. +80` 落在卡片底边之下（实卡高约 128px），拼贴里的下两个角不是卡片角。

### 其余必须修的记录问题

- 文件头 docstring 仍是 082 文案（S6 Profile / S7 TurnRail / S8 搜索），与 083 实际断言不符；用法示例仍指向 `verify_ui_082.py`。
- 默认 `--root /tmp/od082-home`、`THREAD_ID = "t082evidence"`、合成会话标题「OD-ARCH-083 搜索与索引测试会话」、注释「写入 ui-facts.json」而实际写 `facts.json`，全部为 082 遗留。
- `facts.json` 只记判定布尔与少量计数，未记测量方式、定位出的卡片几何、阈值来源；被质疑的 `corner_pixel` 无角点坐标上下文。

### 登记的风险（本轮不阻断，返工可选处理）

- `TurnRail.tsx:20 PREVIEW_H = 96` 仍按旧卡高度估算，新卡实测约 **120–130px** 高。当前只夹卡片顶部，实测未出现裁切或压住输入区，但属未覆盖的回归风险。建议改为实测卡高或补一条边界断言。
- `03-turnrail-tooltip-corners.png` 需重做；`01-topnav-brand.png` 的裁切同时依赖错位采样框，重做时应按探测到的真实 bbox 裁切。

### ARCHITECT 本轮实跑（不是转述）

| 项 | 命令 | 结果 |
| --- | --- | --- |
| 类型检查 | `tsc --noEmit -p desktop/tsconfig.json` | exit 0 |
| 生产构建 | `cd desktop && npm run build` | exit 0（仅既有 chunk >500 kB 警告） |
| 宿主库测试 | `cargo test --offline -q --lib`（先 `bash scripts/setup-desktop-toolchain.sh --print-env` 注入 PATH/PKG_CONFIG/LIBRARY_PATH） | **261 passed / 0 failed** |
| 跨模块测试 | `python3 -m pytest tests -q` | **75 passed / 14 subtests** |
| Runtime 隔离 | `git status --porcelain runtime/` | 0 行 |

说明：`cargo` 需先注入本机工具链环境变量，属本机约定，不是仓库缺陷。

### 结论与下一步

- **实现：** 达成 S1 / S1b / S2，无越界改动，未触 `runtime/`、Profile、凭证、模型路由。
- **验证：** 不成立，STATUS.md 的「7 项断言 100% 通过」在脚本修复前不具证据效力。
- **返工范围：** 仅 `tests/verification/desktop/verify_ui_083.py` 与 `evidence/od-arch-083/`；不重写已合格的 UI 实现，不改 `verify_ui_082.py`、不动 082/081 证据。
- **未验证：** 用户真实模型端到端实测（OD-ARCH-080 清单）仍未进行，本轮 UI 改动不替代该项。

## OD-ARCH-082 终审结论（ARCHITECT，2026-10-25 05:54 后复核）

- **状态：** BUILDER 执行完成，ARCHITECT 已独立复核（详见上方两条记录：复核结论 + 逐条验收审计）。**验收通过。**
- **复核覆盖面确认：** 所有 S1–S8 步骤均有对应证据（文件事实 / 脚本运行 / 真实窗口截图）；计划验收标准 8 条全部通过；runtime/ 零差异；凭证与模型路由未触碰。
- **两处已登记限制（不阻塞，下次迭代跟进）：**
  1. **od-arch-081 evidence 被重跑覆盖：** BUILDER 在 082 验证时以 /tmp 根重跑了 verify_home_fixes_081.py，导致 081 的 7 张截图 + facts.json 被新内容替换。原版本在 731d4c5 可恢复，新版是同一脚本的真实产出（source_unchanged: true）。**处理决定：保留新版，下次提交时在说明中注明「081 evidence 为 082 窗口重跑覆盖，内容等效」。**
  2. **验证脚本两处断言偏松：** verify_ui_082.py 的确认框断言含 or "确认" in labels 兜底，turnrail focus shot 只检查文件存在。ARCHITECT 已用独立 AT-SPI 探测补齐真实结论（确认框文字命中、键盘聚焦 FOCUSED + 像素差 5534 + described-by 关系），判为证据面瑕疵。**处理决定：不阻塞本轮；若后续把该脚本作为回归门禁，收紧这两条。**
- **Git 状态：** HEAD 0ac9edf，工作区 39 项未提交改动（BUILDER 本轮 + 复核记录），未提交、未推送。**等待用户批准提交与推送。**
- **下一步（等用户拍板）：** ①批准后分批提交并推送（含 082 全部代码 + 081 evidence 覆盖说明 + 本轮 REVIEW/STATUS）；②用户按 docs/user-testing-checklist.md 用真实模型端到端实测三大场景 + Profile 切换 + 搜索，反馈质量与体验；③基于实测反馈再定 OD-ARCH-083（预计方向：Skill 内容按真实使用反馈迭代、搜索性能优化若会话超长、或进入下一功能模块）。

## OD-ARCH-082 复核结论：**通过（附两处已登记限制）**（ARCHITECT，2026-10-25）

- **独立核查（与 BUILDER 分离，HEAD `0ac9edf`）：** tsc exit 0；`npm run build` exit 0（仅有 chunk >500 kB 的既有警告）；`cargo test --offline --lib` **261 passed / 0 failed**；`python3 -m pytest tests -q` **75 passed / 14 subtests**；`verify_profile_rules_068.py` 25/0；`verify_skill_bindings_082.py` 33/0；`verify_profile_presets_082.py` 12/0；`scripts/sync-skills.sh --check` 一致；`git status --porcelain runtime/` 为 0 行。上述均为 ARCHITECT 本轮实际运行结果，不是转述 BUILDER。
- **真实窗口证据（ARCHITECT 亲手运行）：** `python3 -u tests/verification/desktop/verify_ui_082.py --display :93 --root /tmp/od082-arch-final` **exit 0，17 项断言全 pass、failures=[]**；`ui-facts.json` 记录命中计数 `1/2 → 2/2`、无结果 `0/0`、琥珀高亮 3265 像素、跳转位移 33396、Esc 后高亮 0、悬停卡区域差 12549。截图 `profile-discipline.png`（确认框「切换到「纪检专业」？」）与 `search-match.png`（1/2、两处高亮 + 琥珀环、徽标「专业」）经目视核对与断言一致。
- **S6 白名单与友好标签（源码 + 单测 + 真实窗口）：** `profile_choice.rs` 的 `list()` 改为按白名单返回固定友好名（基础标准 / 纪检专业 / ACP 协议通道），不再读 `package.json.name`；`resolve()` 对白名单外的已保存值回落默认而不报错、不改写设置；`saved_out_of_whitelist` 驱动弹层黄色提示。`TopNav.tsx` 列表主标题用 `o.label`、副标题用技术名，确认框带 `role="alertdialog"` 与友好名。`scripts/omnidesk.sh` 不再无条件注入 `OMNIDESK_DSH_PROFILE`。单测断言 label 不含 `dsh-profile-`。
- **S6 白名单外保存值（082 窗口脚本未覆盖，ARCHITECT 单独补测）：** 在隔离根 `/tmp/od082-whitelist-probe` 写入 `engine_profile: "yvan-work"` 后启动真实窗口，弹层显示「不在清单（已回落默认）· YVAN-WORK」与提示条「当前保存的 Profile 不在可选清单，本次使用默认。」，可选项只有基础标准 / 纪检专业，实际使用 `omnidesk-base`；弹层关闭后 `settings.v1.json` 仍为 `yvan-work`。截图 `/tmp/od082-arch/whitelist-popover.png`（本轮临时证据，未入仓库）。这条正是计划验收标准④、且原脚本只测了白名单内切换。
- **S7 TurnRail（源码 + 真实窗口 + 无障碍关系）：** 组件只补 `border-primary/30` 与 `aria-describedby`，未重写布局。除脚本的悬停断言外，ARCHITECT 补做「指针移至 (700,700) 后由 AT-SPI 聚焦第 1 轮刻度」：`grab_focus` 返回 true、`FOCUSED` 状态为真、预览卡区域差 5534 像素，且无障碍关系为 `described-by → tool tip`。由此确认键盘聚焦可见与 ARIA 关联均真实生效。
- **S6 两态差异（facts 复核）：** `profile-base.json` / `profile-discipline.json` / `profile-compare.json` 记录两份 `--dump-config` 的 sha256、各 31 个插件条目、50 行差异，`preset-standard` 中 `skill-filesystem` 与 `tool-skill` 在 base 为 `disabled: true`、discipline 为启用。ARCHITECT 首轮已独立展开两份 dump 核对（仅 18 行实质差异），本轮复核 facts 与之一致。
- **S4 注入链路（源码复核）：** `HomeCanvas` 按 `skill_bindings[scene]` 取 `max_injected` 截断并只注入有正文的 Skill；`composeConversationPrompt` 的 note 分支最多 1 个、与既有 2500 字符上限共用截断；`aiDraft` 在完整组装中按 2500 字符截断；无绑定或无规则包时 `skills` 为 null，提示词保持原状。33 项断言覆盖三场景差异、续答每轮注入与无绑定逐字一致。
- **验证脚本的断言质量裁定：** `verify_ui_082.py` 仍有两处偏松：确认框断言含 `or "确认" in labels_confirm` 兜底；`turnrail_card_focus_shot` 只检查截图文件存在。本轮已用独立探测补齐这两项的真实结论（确认框文字经 AT-SPI 命中并目视截图；键盘聚焦经 FOCUSED 状态 + 像素差 + described-by 关系确认），因此**判为证据面瑕疵、不阻塞验收**。若后续复用该脚本作回归门禁，建议收紧这两条断言。
- **数据与边界：** 未改 `runtime/`；未读写凭证与模型路由；真实窗口均用 `/tmp` 隔离数据根与合成对话，未触碰用户会话、成果与规则文件；`~/.dsh`、`~/.agents` 未动。未提交、未推送、未打标签。
- **计划外差异（已登记，不影响 082 功能结论）：** 工作区中 `tests/verification/desktop/evidence/od-arch-081/` 的 7 张截图与 `home-fixes-facts.json` 被重跑覆盖（时间 04:54–04:55，内容为 `verify_home_fixes_081.py` 以 `/tmp/od081-test` 重跑的新证据）。082 计划未授权改写 081 证据。影响：081 原证据快照被替换，但新文件仍是同一脚本的真实运行结果且 `source_unchanged: true`；081 已在 `731d4c5` 提交，历史版本可从 Git 恢复。**要求：** 后续提交时要么保留这版重跑证据并在提交说明里注明，要么 `git checkout 731d4c5 -- <081 evidence>` 恢复原快照；不得把两者混为一次「082 新证据」。
- **已知限制（不阻塞本轮）：** ①两态隔离只覆盖 `preset-standard`；`preset-ptc` / `preset-minimal` / `preset-cordis` 内 Skill 仍启用，默认路径安全，但换 preset 后隔离不成立；②搜索高亮只处理 `p` / `li` 的字符串子节点，标题、表格、代码块不参与高亮，匹配计数仍按原文计数；③会话搜索为全量线性扫描，数千轮时可能卡顿；④Skill 正文注入受 2500 字符上限，超长说明会被截断并带提示；⑤真实模型对 Skill 的实际遵循度未验证（本轮全部为合成数据与合成端点）。
- **真实模型输出：** 本轮无。所有结论止于文件事实、静态检查、脚本运行与真实窗口交互；模型质量、审批链路、真实端点下的场景效果留待用户按 OD-ARCH-080 实测清单反馈。
- **结论：OD-ARCH-082 验收通过（附上列限制与 081 证据覆盖的收尾要求）。** 六 Skill 盘点与绑定契约、三场景差异化注入、两态 Profile 白名单与友好标签、TurnRail 边与无障碍、当前会话搜索均达到计划口径。下一步停下，等用户真实模型端到端实测反馈后再定后续计划。

## OD-ARCH-082 完成审计（ARCHITECT 逐条复核验收标准，2026-10-25）

> 性质：对目标「批准执行 OD-ARCH-082」的完成性审计。**不新增实现、不改代码与 Profile、不提交推送**；只用当前工作区重跑证据，并补齐此前只靠源码或临时证据支撑的两条。HEAD `0ac9edf`，`git status --porcelain runtime/` 为 0。

| 验收标准 | 当前证据 | 判定 |
| --- | --- | --- |
| ① 六 Skill 盘点有文件事实；绑定契约 v1 可读、v2 校验生效 | `verify_profile_rules_068.py` **25/0 pass**（本轮实跑）；`profile_rules.rs` 的 v1/v2 双 schema、六项白名单与上限校验有单测覆盖 | 通过 |
| ② 三场景注入不同、随手记轻装、续答每轮注入 | `verify_skill_bindings_082.py` **33/0 pass**（本轮实跑，证据 `evidence/od-arch-082/scene-injection.json`） | 通过 |
| ③ 两态 `--dump-config` 差异可复现；**校验脚本能发现漂移** | `verify_profile_presets_082.py` **12/0 pass**；本轮补做负向测试：把补丁副本顶层两项 `- id:` 顺序互换后，脚本 **exit 1、failed 2** 并逐项打印期望/实际序列 | 通过（漂移检测本轮首次实跑验证） |
| ④ 下拉只两项；`yvan-work` 原样保留且**有显式提示** | 隔离根真实窗口截图本轮复跑（`/tmp/od082-arch/whitelist-popover.png`）目视确认：标题「下次启动使用 · 当前：**不在清单（已回落默认） · YVAN-WORK**」、黄色提示条「当前保存的 Profile 不在可选清单，本次使用默认。」、列表仅「基础标准 / 纪检专业」、页脚「本次启动实际使用：omnidesk-base（web-api 通道）」；弹层关闭后设置仍为 `yvan-work` | 通过（此前“提示条未验证”已由截图闭合） |
| ⑤ TurnRail 仅补边与 aria；悬停/聚焦均可见 | `verify_ui_082.py` 本轮复跑 `turnrail_hover_card_appears: 12549` 像素差、`turnrail_card_focus_shot: True` | 通过 |
| ⑥ 搜索命中、跳转、计数、`0/0` 可用 | 同上脚本复跑：`search_count_1_of_2`、高亮 3265 像素、回车后 `2/2`、跳转位移 33396、`0/0`、Esc 清空后高亮 0 | 通过 |
| ⑦ tsc 0、build 成功、cargo lib 全绿、pytest 全绿、`sync-skills --check`、`runtime/` 零差异 | 本轮实跑：tsc **exit 0**；`npm run build` **exit 0**；`cargo test --offline --lib` **261 passed / 0 failed**；`pytest tests -q` **75 passed / 14 subtests**；`sync-skills.sh --check` exit 0；`git status --porcelain runtime/` **0 行** | 通过 |
| ⑧ evidence 目录含两态 facts、三场景事实与 S6/S7/S8 截图 | `evidence/od-arch-082/` 14 个文件齐全：`profile-base.json` / `profile-discipline.json` / `profile-compare.json` / `scene-injection.json` / `ui-facts.json` + `profile-base.png` / `profile-discipline.png` / `turnrail-card.png` / `turnrail-card-focus.png` / `search-match.png` / `search-match-second.png` / `search-empty.png` / `00-rail-before.png` / `00-search-escaped.png` | 通过 |

- **真实窗口审计：** `python3 -u tests/verification/desktop/verify_ui_082.py --display :96 --root /tmp/od082-completion-audit` **exit 0，17 项断言全 pass、failures=[]**，并重写 `ui-facts.json`（05:50，root 为该临时隔离根）与 9 张截图（05:50–05:51）。这是 082 证据目录内文件的一次正常刷新，不是越界改动；`git status` 仍为 43 项，未新增或删除仓库文件。
- **未纳入本轮修复的项（不属 082 范围，保持原判）：** 复核后复检 G 项的检索/用量插件缺口（082 定案明确“不复制 web 的 free-search / cost-meter”）、预检 A/B（上下文窗口声明、思考等级未落地）、清单第 1 步过期文案、081 证据被重跑覆盖的收尾选择。
- **真实模型输出：** 本轮仍无。082 的功能口径已全部由文件事实、脚本运行与真实窗口证明；模型质量与真实端点行为留给用户的端到端实测。
- **结论：** OD-ARCH-082 的 8 项验收标准逐条有当前可复现证据，目标达成，无遗留必做项。工作区 43 项改动仍待用户决定提交口径（含 081 证据恢复或保留的选择），本轮未提交、未推送。

## 082 复核后 · 实测前置复检（ARCHITECT，2026-10-25，未验收）

> 性质：**用户真实模型实测开始前的复核记录**，不是用户实测结果，也不是任何新计划的批准或验收结论。本轮未改代码、未改 Profile、未改模型路由、未改实测清单文案、未提交推送；HEAD 仍为 `0ac9edf`，`git status --porcelain runtime/` 为 0。证据分「文件事实」「命令运行」「未验证」三类，逐条标注。

### E. 实测窗口曾指向 082 之前的二进制（已由 ARCHITECT 处理）

- **文件事实：** 原运行实例（pid 293231，04:09 启动）所用 release 二进制早于 082 的最后改动（`desktop/src-tauri/src/profile_choice.rs` 04:36、`desktop/src/shell/TopNav.tsx` 05:06）。按该实例实测会拿到旧界面，无法验证 082。
- **命令运行：** 执行 `scripts/omnidesk.sh --stop` 后重新执行 `scripts/omnidesk.sh` 重建，`npm run build` exit 0、`cargo` release 构建 exit 0（约 2m18s）。新实例：app pid 331990、engine pid 332200、DSH 子进程 332210（参数含 `--profile omnidesk-base --patch ~/OmniDesk/app/config/omnidesk-model.yml`），设置服务 332080（端口 45135，端口记录在 `~/OmniDesk/app/engine-state/dsh-settings.json`）。二进制时间 05:27，`find dist/src src-tauri/src -newer <二进制>` 无输出，即当前运行的构建不旧于 082 源码。
- **未验证：** 该窗口的整窗截图在当前 X11 会话为全黑（1280×857 与 1600×900 根窗口均 RGB 全 0），无法用像素证据确认视觉效果；本轮改用 AT-SPI 无障碍树取证。**用户实际看到的外观仍以用户本人观察为准。**

### F. 082 界面在真实窗口中的表现（AT-SPI 实测）

- **命令运行（`DISPLAY=:0` 连到运行中的正式实例，只读遍历无障碍树）：**
  - 命中「当前会话内查找」输入框（082 的 S8 检索入口确实已渲染）；
  - 顶栏 Profile 徽标为 combo box，名称「标准」；
  - 首页问候「夜深了· 欢迎回到 OmniDesk」可见。
- **命令运行（点击徽标后遍历弹层子树）：** 弹层为 dialog「选择引擎 Profile」，其子节点为 2 个 section、1 个 list box「可选 Profile」（2 个子项）和 1 个 paragraph；list box 两项分别为「基础标准 通道默认 (omnidesk-base)」与「纪检专业 discipline」。可选项只有这两项，未出现 `dsh-profile-` 技术包名，符合 S6 的白名单与友好名口径。
- **文件事实（源码）：** `TopNav.tsx:220-228` 在 `saved_out_of_whitelist` 为真时把 `currentLabel` 拼成「不在清单（已回落默认） · <saved>」，`:329-333` 渲染黄底提示条「当前保存的 Profile 不在可选清单，本次使用默认。」；`lib.rs:886-889` 由保存值与白名单比对得出该标志。当前保存值 `~/OmniDesk/app/config/settings.v1.json` 的 `engine_profile` 仍为 `yvan-work`，因此本次启动应回落 `omnidesk-base`（进程参数已证实）。
- **未验证：** 本次点击后，AT-SPI 的 list box 两项状态均为 `ENABLED`（无 `SELECTED`），与「回落默认、无选中项」一致；但弹层里的 `section`/`paragraph` 节点名与文本均为空，**「不在清单」标题与黄底提示条的文案没有出现在无障碍树里**。所以本次只能确认「保存值未改写 + 实际用 omnidesk-base + 白名单两项」，提示条是否可见未取得独立证据。082 隔离根补测（`/tmp/od082-arch/whitelist-popover.png`）中该提示曾出现，说明渲染路径存在；本轮不把它判为回归，也不据此判为通过。

### G. P1 缺陷：新默认 Profile 丢了联网检索与用量统计（建议单独立项修复，本记录不修）

- **命令运行（真实 `--dump-config`）：** `omnidesk-base` 与 `discipline` 两份展开结果中 `web-search-free` 出现 0 次、`cost-meter` 出现 0 次；`web` 的展开结果 `web-search-free` 1 次、`cost-meter` 在列（`maxUses: 20`）。相关产物 `/tmp/od082-dump-base.yml`（05:29）、`/tmp/od082-dump-web.yml`（05:31）。
- **命令运行（凭据布尔，只读、不打印密钥）：** 以 `omnidesk-base` 临时起 `dsh --port 0 --no-open`，走官方 `credentials/describe` 查询，`OPENDESIGN_API_KEY` configured **true**，`DEEPSEEK_API_KEY` / `AGNES_API_KEY` / `AGNESCOM_API_KEY` 均 **false**；`/proc/332210/environ` 与 `/proc/332200/environ` 中均无 `DEEPSEEK_API_KEY`。另以 `grep -c` 在 `.credentials.yaml` 中只统计键名出现次数（不读取值）：`OPENDESIGN_API_KEY` 1、其余 3 个为 0，与上述布尔一致。
- **文件事实（成因）：** 082 定案要求两态 bundles 只有 `dsh-base` + `dsh-web-app`，故 `omnidesk-base`/`discipline` 的 `package.json` bundles 不含 `dsh-free-search` 与 `dsh-cost-meter`；而 web 的 `dsh-base` 自带 `web` 行的 `searchProvider: deepseek-official` 与 `web-search-deepseek`（`apiKeyEnv: DEEPSEEK_API_KEY`）。`dsh-free-search/lib/index.js:4041-4062` 只在 `searchProvider` 未设或仍为出厂默认 `deepseek-official` 时于运行时接管，因此**装上 free-search 即可恢复无密钥检索**；本机 web Profile 的 `node_modules/dsh-free-search`、`node_modules/dsh-cost-meter` 均已存在，无需联网安装。
- **影响（需用户按已知问题对待，不当作新 bug）：** 默认「基础标准」与「纪检专业」下，「第 4 步附」的联网检索预计直接失败（`WEB_PROVIDER_CREDENTIAL_MISSING`），因为当前没有 `DEEPSEEK_API_KEY`；用量/费用统计组件也不在，界面上不会显示 cost-meter 的额度与费用信息。`token-meter` 属 `dsh-base`，上下文用量上报不受影响。
- **未验证：** 未实际发起检索请求去触发该报错（避免真实端点消耗与外部调用）；上述为配置展开与凭据布尔推出的预期，需在修复后用真实检索验证。

### H. 预检 A/B 两项在当前配置下仍成立（不变）

- **命令运行：** 以当前覆盖文件展开 `omnidesk-base`，`contextWindow` 计数 **0**；`reasoningEffort` 只出现在模型能力表（`reasoningEfforts`）里，覆盖文件第 64 行的 `reasoningEffort: medium` 挂在 `- id: acp` 下，stderr 报 `patch: entry "acp" not found`；展开后 `agent-default-model` 仅 `provider: opendesign` + `model: deepseek-v4.1-flash`，无思考等级。
- **文件事实：** `docs/user-testing-checklist.md:54` 仍写「5 模型均声明 1M 上下文、思考等级 high」，与该覆盖文件不符。清单文案本轮**未改**（改动需新计划）。

### 复检边界与下一步

- 本轮只做了只读核查与一次「停旧实例、按当前源码重建并启动正式实例」的运行准备；未提交、未推送、未打标签；未读写凭证值；未改动 `~/.dsh`、`~/.agents`。
- 081 证据被重跑覆盖的收尾要求仍然有效，本轮未处理。
- **下一步：** 由用户按 `docs/user-testing-checklist.md` 在已就绪的正式实例上实测；G 项检索缺口作为已知限制记录，不属于用户实测要复现的新缺陷。实测反馈到达前不新立计划、不改 Profile。

## 2026-10-25 真实模型实测前置预检（ARCHITECT，未验收）

> 性质：**实测前的只读核查记录，不是用户实测结果，也不是任何计划的验收结论。** 本轮未改代码、配置、用户数据或既有文档，未提交推送。证据分「文件事实」「命令运行」「未验证」三类。

### A. 首页切模型后覆盖文件丢失 1M 上下文声明（建议 P1）

- **文件事实：** `~/OmniDesk/app/config/omnidesk-model.yml`（10-23 21:01）中 `contextWindow` 计数为 0；备份 `.bak-od068` 有 5 处，均为 `1048576`。`omnidesk.model-settings.v2.json` 的 5 个模型仍都声明 `contextWindow: 1048576`。
- **命令运行：** 以当前覆盖文件展开 `yvan-work`（`dsh lib/bin.js --profile yvan-work --patch <覆盖文件> --dump-config`）exit 0，产物 `contextWindow` 计数 0；用当前 `desktop/src-tauri/scripts/model-routes.mjs` 以 `--write-overlay --provider opendesign --model deepseek-v4.1-flash --reasoning-effort high` 重新生成，同样计数 0（`ok:true`）。
- **成因（源码）：** `model-routes.mjs:212` 只从 `--dump-config` 的模型条目取能力字段，`:225-237` 写出模型时不读 v2 设置 JSON；补丁层对 `config` 整体替换，而 Profile 本身未声明 `contextWindow`，声明因此消失。`model_settings.rs::render_provider`（约 332-378 行）走的是另一条路径，会保留该字段，对应单测在 919/936/960 行附近。
- **影响边界：** 影响容量上报、`request/context` 记录与溢出判定；引擎回落值为 262144（`runtime/deepseek-harness/packages/llm/llm-pi-ai/src/config.ts:65`）。当前 `yvan-work` 与 `web` 的 `compaction-basic`、`command-compact` 均为 `disabled: true`，**不能**据此断言「长对话会被自动截断在 262k」，真实长上下文行为仍未验证。

### B. 思考等级未进入实际默认选择（建议 P1）

- **文件事实：** 实际覆盖文件把 `reasoningEffort: medium` 写在 `- id: acp` 下；但 `~/OmniDesk/runtime/dsh-home/profiles/yvan-work/package.json` 的 bundles 只有 `dsh-base`、`dsh-web-app`、`dsh-experimental-inspector-profile`，`web` 的 bundles 也只有 `dsh-base`、`dsh-web-app`、`dsh-free-search`、`dsh-cost-meter`；只有 `profiles/acp` 含 `@deepseek-ai/dsh-acp-app`。
- **命令运行：** 用当前覆盖文件展开 `yvan-work` 与 `web`，stderr 各出现一次 `patch: entry "acp" not found`；展开后的 `agent-default-model` 只有 `provider` 与 `model`，无 `reasoningEffort`。用 `--reasoning-effort high` 重新生成覆盖文件后，该字段仍只出现在 `acp` 块，`agent-default-model` 仍无。
- **成因（源码）：** `runtime/deepseek-harness/packages/core/agent-default-model/src/index.ts` 的 schema 与 `currentSelection()` 支持该字段（24-31、51-55、67-72 行）；`model-routes.mjs:251-255` 与 `model_settings.rs:274-293` 都只写 `acp`，不写 `agent-default-model`。`engine/omnidesk_engine/web_api_adapter.py:1156-1183` 的 `select_model` 支持显式传 `reasoningEffort`，但首页普通提交依赖覆盖文件里的默认选择。
- **影响边界：** 手动选档与场景预设（`samples/profiles/discipline.rules.json` 的 low/low/off）都会经 `selectModel`/`selectEffort` 写覆盖文件，但当前同样不生效。首页默认值为 `off`（`desktop/src/shell/HomeChat.tsx:358`）。**「默认应为 high」属产品口径，需用户确认，不把现状直接判为缺陷。**

### C. 正式启动脚本用 `web` 覆盖设置中的 `yvan-work`（建议 P1 口径问题）

- **文件事实：** `~/OmniDesk/app/config/settings.v1.json` 的 `engine_profile` 为 `yvan-work`；`scripts/omnidesk.sh:172-184` 在子进程里无条件执行 `export OMNIDESK_DSH_PROFILE="${OMNIDESK_DSH_PROFILE:-web}"`（未 unset 该变量，外部显式值仍优先）。`desktop/src-tauri/src/engine_supervisor.rs:130-143` 明确环境变量优先，只有变量为空才读取保存的 `engine_profile`。
- **影响：** 按 `docs/user-testing-checklist.md` 的默认方式启动，实际加载的是 `web` 而非设置里的 `yvan-work`；`web` 同样有 `opendesign` 路由与档位映射，因此可以测通模型，但 Profile 资产（Skill、插件、规则）相关的结论不能外推。本轮 App 未运行，未取得真实进程参数。

### D. 实测清单的现状描述已过期（建议 P2，不阻塞）

- `docs/user-testing-checklist.md:54` 仍写「opendesign 5 模型均声明 1M、思考等级 high」，与 A/B 的实际覆盖文件不符。本轮未改该文件。索引条两张截图顺序写反（实际 `06c-rail-jumped-last.png`、`06d-rail-jumped-first.png`）已登记在 080 复核，不阻塞。

### 预检未覆盖

- 未启动 App、未跑真实端点、未重跑 tsc/cargo/pytest（本轮只读）；`scripts/omnidesk.sh --status` 显示 App 未运行，存在陈旧 `vite.pid` 记录，未清理。release 二进制（10-25 01:51）新于前端与 Rust 源码，按 080 结论对应当前 081 代码。
- 修复 A/B/C 需要改动源码与用户数据根下的覆盖文件，均未获批准，本轮未动。

## OD-ARCH-081 复核结论：**通过**（ARCHITECT，2026-10-25）

- **独立核查（与 BUILDER 分离）：** tsc exit 0；pytest 75 passed / 14 subtests（本机独立重跑）；cargo --lib 260 passed（BUILDER 报告数字，本轮 Rust 侧仅 external_open.rs 新增，抽查其单测覆盖 http/https 白名单判定）；git status --porcelain runtime/ 零差异；当时 15 项未提交改动已在收尾批次 731d4c5 提交并推送。
- **代码抽查属实：** ①MarkdownText.tsx 无 dangerouslySetInnerHTML、无 rehype-raw，HTML 按文字显示；②external_open.rs 只放行 http/https 绝对地址交系统浏览器，其余拒绝，带单测；③SceneScaffold 问候+待办常驻，指针/聚焦不再改变可见性；④BrandMark 为纯展示 aria-hidden 元素；⑤evidence/od-arch-081/home-fixes-facts.json passed=true、gate_errors=[]（问候条在三场景像素变化均为 0；品牌标渐变像素 37、旧 spark 0；表格/代码块/列表渲染齐备；三根隔离指纹一致）。
- **结论：081 验收通过。** 首页问候常驻、品牌标、Markdown 正文渲染三项修复均落地，安全边界（无 HTML 注入、外链白名单）符合要求。
- **遗留备查（2026-10-25 校正）：** 081 本轮 24 个文件已由 BUILDER 单批次提交 731d4c5 并推送，状态记录随后推送至 0ac9edf，本地与 origin/main 一致、工作区干净；原「15 项未提交改动等下一批推送」的表述已失效。分支窗口内正向流程验证仍开放（079 遗留）。

## OD-ARCH-080 复核结论：**通过（仅 Git 整理与文档，无功能代码）**（ARCHITECT，2026-10-25）

- **独立核查（与 BUILDER 分离）：** 本地 HEAD 与 origin/main 同为 0ac9edf，工作区 0 项改动，git status --porcelain runtime/ 为 0；3f8c3ba..0ac9edf 的 runtime/ 差异为 0。回归基于当前 HEAD 独立重跑：tsc exit 0；python3 -m pytest tests -q 75 passed / 14 subtests；cargo test --offline --lib 260 passed / 0 failed（BUILDER 080 记录为 257，081 新增 external_open 单测后升到 260，属正常增量）。
- **分批事实核对：** 四批文件数与记录一致（4f8c578 20 项文档与配置基线、a4327ee 17 项 Rust 宿主、bd6afd8 16 项前端、328a498 155 项引擎与验证证据），合计 208 项改动路径，可分界回滚；四批的全路径集合与 3f8c3ba..328a498 完全对应，无计划外文件。
- **边界核对：** 3f8c3ba..328a498 内无 knowledge/、data-template/、.kb_extract/、.codegraph/、.credentials.yaml 路径，无 pdf/docx/xlsx/zip/sqlite 等业务二进制附件；凭证模式（sk- 前缀密钥、Bearer 令牌、api_key 赋值）扫描零命中；git tag --points-at HEAD 为空，其它分支 feat/desktop-ui 未被触碰。
- **实测清单核对：** docs/user-testing-checklist.md 标题为 OD-ARCH-080 校准版，含「第 4 步附二」六条（附件/问句条/答句条/分支/索引条/窄屏隐藏）并把真实模型端到端标为本轮重点；清单引用的证据逐个存在（od-arch-079/home-polish-facts.json、05e-after-repo-pick.png、06c-rail-jumped-last.png、06d-rail-jumped-first.png、web-api/W11.json、od-arch-064/00-omnidesk-launch.png）。
- **实测前置核对（文件事实）：** scripts/omnidesk.sh 走 release 二进制且 needs_build 以 dist、src、src-tauri/src 比二进制新为触发条件；当前 desktop/src-tauri/target/release/omnidesk-desktop（10-25 01:51）比全部前端与 Rust 源码新（find -newer 检查 0 个文件），因此实测跑的是 081 已验收代码，不会因旧二进制得到过期结论。
- **清单小瑕疵（不阻塞）：** 实测清单正文把两张索引条截图写作 06c/06d-rail-jumped-*.png 并与「点首/末」对应，实际文件为 06c-rail-jumped-last.png 与 06d-rail-jumped-first.png（顺序相反但两张都在）。仅记录，不改文件。
- **结论：080 通过。** 该计划只做 Git 整理与实测入口准备，四批提交完整、边界干净、回归全绿；真实模型端到端本身仍未验证，按计划移交用户本人执行。
## OD-ARCH-078 + OD-ARCH-079 复核结论：**通过**（ARCHITECT，2026-10-25）

- **独立核查（与 BUILDER 分离）：** 全量 cargo test --lib 257 passed / 0 failed（本机独立重跑）；pytest 75 passed / 14 subtests；git status --porcelain runtime/ 零差异；未提交改动 127 项全部保留未提交未推送。
- **代码抽查属实（不依赖自述）：** ①附件专用禁区表 ATTACHMENT_FORBIDDEN_PATH_FRAGMENTS 已移除 myworkstation（task_shell.rs:405），通用落盘禁区表仍保留（:304），职责分离正确——选仓库内 AGENTS.md 作附件可读、产物写入仓库内仍被拒；②问句操作条只剩「复制」（HomeChat.tsx:1614-1617），撤回/修改按钮确实删除而非置灰；③答句操作条五项齐备；④分支已改为引擎原生 session/fork（web_api_adapter.py:2199），ACP 通道返回 unsupported，branchBusy 防连点，无切点时按钮禁用；⑤TurnRail.tsx 存在且 >=2 轮才渲染。
- **证据核验：** evidence/od-arch-078/home-polish-facts.json passed=true 12 项全绿；evidence/od-arch-079/home-polish-facts.json passed=true 18 项全绿（问句只复制/答句五项/附件读取/负向分支/索引条点击视图变化/三根隔离）；W11.json 引擎级 fork 成功（at_seq=17、不重放历史）；W12.json 无效切点如实报错。
- **两项遗留（不阻塞验收）：** ①分支的窗口内正向流程未跑（只有接口级真实调用 + 窗口负向证据；预置测试线程无引擎会话，属测试环境限制非产品缺陷）；②索引条悬停预览卡未单独截图验证。
- **偏差裁定（全部接受）：** ①验收脚本从 GTK Ctrl+L 改为点选列表行——无窗口管理器环境下 GTK 不出现位置栏，属测试环境适配非产品行为；②附件放开到仓库内仍保留红线扫描与数量/字数上限，与用户指示兼容。
- **078+079 联合结论：全部验收通过。** 首页对话区已对齐 DSH 原生操作习惯，附件入口与类型边界已放开到用户要求。
- **遗留备查（既有，不属本轮）：** ①127 项未提交改动持续累积，等用户批推送；②「模型真正阅读附件」需真实模型阶段验证；③多 worktree 端口冲突（5273）为本地开发约定，未处理。
## OD-ARCH-075/076/077 复核结论：**通过（含三项偏差裁定与附件数据边界确认）**（ARCHITECT，2026-10-24）

- **独立核查（与 BUILDER 分离）：** 全量 cargo test 271 passed / 0 failed（本机独立重跑，含 single-instance 插件编译）；pytest 65 passed；tsc 0；npm build 通过；`git status --porcelain runtime/` 零差异（75 项源码改动 + 39 项既有未跟踪，共 114 项工作区改动保留未提交）。代码抽查属实：①单实例插件注册在最前、聚焦已有窗口后新进程退出，无手动 exit 竞争；②`task_open_dsh_settings` / `open_settings_window` / 前端入口已删净，`dsh_settings.rs` SettingsService 保留（凭据 RPC 无头复用）；③`capability_market` 命令与「接入 DSH 插件市场」开关已删，能力页文案改为「新增插件与 Skill / 随版本升级」；④语音按钮为诚实说明，不灰显误导；⑤附件链路 `task_list_references` / `task_read_reference` 路径校验复用 `assert_path_within_roots`（绝对路径 + 符号链接解析 + 越界拒绝 + 隐藏/二进制排除 + 512KB 单文件上限 + 120k 字符总量上限 + 5 个文件上限），`attachmentPrompt.ts` 拼装带 sha256 的附件块，红线扫描覆盖附件正文（由宿主扫描 `userInputText` 即拼接后文本）。
- **证据核验：** evidence/od-arch-075（2 截图 + facts）、od-arch-076（2 截图 + facts，第二次启动 0.06s 退出、窗口数 1）、od-arch-077（10 截图 + facts，25 项检查全 pass，附件正文 SYNTHETIC-MARKER-077 命中 2 次证明送达引擎）；源隔离根、`~/.dsh`、`~/.agents` 前后指纹一致；074 生命周期回归（process_owner 5 + pytest 10）未被本批破坏。
- **偏差裁定（三项全接受）：** ①075 通道说明落引擎诊断页——界面本无通道选择器，通道由 `OMNIDESK_ENGINE_CHANNEL` 决定，落点合理，能力说明与实际行为一致；②076/077 文件名偏差（shell/HomeChat.tsx 等）属实，不影响功能；③077 附件拼进提示词而非协议字段——**接受，作为当前 MVP 的最小改动方案**，理由见下条。
- **附件数据边界裁定（用户问项）：** 现状是附件正文原样进入本轮提示词，随会话记录留存，受历史窗口裁剪约束（窗口裁掉后不再重复发送），且已被红线扫描覆盖（拼进 `userInputText` 后一起扫描）。**与 DSH 原版做法一致**：DSH 的 @ 文件引用同样把文件内容注入上下文（无独立附件通道），会话日志含全部上下文；**与 Codex CLI 一致**：附件/被 @ 的文件以文本形式进 prompt，无独立存储。**没有平台采用「附件独立存储、按需检索」**——那是知识库/向量库方案，成本高且当前无需求。**数据边界可接受，理由：** ①附件来源限数据根内（成果目录+知识库），本就是用户自有非敏感材料；②红线扫描在发送前拦截，身份信息等命中仍会被拒；③会话记录保留在数据根，不进 Git，符合既有边界。**后续评估触发：** ①若用户需要引用大文件（>512KB）或二进制（docx/pdf），再评估摘要提取方案；②若需要「附件不入会话记录、用后即焚」模式，再评估独立附件通道；③红线扫描对附件正文是启发式，不替代授权判断（既有原则不变）。
- **072 Part B/C 复核结论（补记，2026-10-24）：** 通过。release 重建 + 首页信息架构改版（欢迎词+轮动提醒+顶部导航+日期卡片悬停）均已落地并有截图证据；能力管理分页从「接入 DSH 插件市场」改为「只读列表+启停+随版本升级」文案，与 D-030 决策一致。遗留：托盘/快捷键推迟到打包阶段（已登记）。
- **遗留与风险：** ①「原生拖放超上限界面反馈」未在真实窗口验证（单测已覆盖上限判定，风险低）；②「模型真正阅读附件后作答」未验证（合成端点返回固定文本，只能证内容送达，需真实模型阶段验证）；③托盘+快捷键未实现（按计划推迟）；④75 个源码改动未提交未推送，持续累积风险，等用户批推送。
- **结论：OD-ARCH-075/076/077 全部验收通过。** 建议下一步处理未提交改动分批推送，然后进入真实模型端到端实测。

## OD-ARCH-074 复核结论：**通过（双组偏差认可；/bin/kill 依赖登记为已知限制）**（ARCHITECT，2026-10-24）

- **独立核查（与 BUILDER 分离）：** cargo test（lib process_owner 定向）5 passed；pytest test_process_owner_unit 10 passed；真实隔离场景证据 `process-lifecycle-074.json` 21/21 pass（同根双实例拒启、kill -9 引擎后按登记回收 DSH 子组、无关进程存活、优雅关闭登记删除）；`git status --porcelain runtime/` 为空；owner.json 为 0600。代码抽查属实：四重身份核验（pid 存活 + pgrp 匹配 + cmdline 完全一致 + boot_id）与三态判定齐全；`terminate_group` 杀前校验「组内全部为本产品进程」且拒绝包含自身；`reclaim_owner` 引擎侧永不抢占活跃引擎、宿主侧仅可回收自己宿主已崩溃的登记；boot_id 不一致直接删登记不杀进程；子组回收用可信组长判据（组长 pid==pgid 且 cmdline 是本产品或 node bin.js）。
- **决策一（双组方案）：** **批准，不改单组。** 计划的真实目标是「任何退出路径不留孤儿」；双组 + owner.json 登记 child_pid/child_pgid 使引擎被 SIGKILL 后 DSH 子树仍有归属可查，是比计划字面更完整的实现。改单组需重做清理逻辑与全部验收，收益仅是登记文件略简，不值。本条已记入 docs/decisions.md（D-029）。
- **决策二（/bin/kill 依赖）：** 接受并登记为已知限制（无 libc，靠外部 /bin/kill；缺失时回收失败只记诊断）。可选后续：加 libc/nix crate 发信号，低优先级。
- **决策三（22 个历史孤儿）：** 实查本轮 `ps` 已无 node(bin.js)/serve.py 孤儿进程（用户可能已重启清理）；正式数据根 engine-state 下也无 owner.json 遗留。无需授权回收操作。若再次出现，用 074 的 reclaim_owner 路径回收即可（身份核验不过的组绝不动）。
- **遗留风险：** ①宿主环境缺 /bin/kill 时回收静默失败（已登记）；②「引擎活、宿主已崩」场景宿主侧会回收上次登记，若用户同时在另一宿主跑同根实例会依赖 host_pid 判别，证据已覆盖 basic 场景，真实双宿主未测；③跨平台（Wayland/release、非 Linux）未验证。
- **结论：OD-ARCH-074 验收通过（含偏差认可）。** 改动保留未提交，随批次统一处理。

## OD-ARCH-065 复核结论：**通过（多提供商模型档案 + 探测沿用已存密钥 + 1M 上下文声明）**（ARCHITECT，2026-10-23）

- **独立核查（本机，与 BUILDER 分离）：** cargo test 212/0（含 13 集成）、tsc 0、pytest 26、npm build 成功，与 BUILDER 记录一致；`git status --porcelain runtime/` 为空；工作区干净、`main` 与 `origin/main` 同步（HEAD `ef16356`）。代码抽查四项全部属实：①`model_settings.rs` v2 结构 `providers[]` + `default`，路由/凭据引用唯一性校验、`contextWindow` 范围 1..1e8、v1 迁移成功才改名 `.v1.bak`（冲突时加时间戳后缀）、损坏文件只读报错不覆盖；②`render_overlay(base)` 以 `--dump-config` 现行完整路由集为底、OmniDesk 覆盖同名项、其余保留，修掉了「只存在于补丁里的路由被浅合并抹掉」的缺陷，覆盖文件原子写、权限 0600；③探测带 `provider` 仅当该 provider 已在覆盖文件中（草稿未填新密钥时由 DSH 凭据库按已存密钥解析），设置服务改为按引擎同 Profile + 同一份覆盖文件启动，密钥来源三态（沿用已存/本次输入/未带）与「命中 DSH 内置目录未发起真实网络请求」提示齐备；④删除提供商只删配置与路由、不动 DSH 凭据，界面明示，删空警告。
- **证据核验：** `evidence/od-arch-065/` 9 张截图 + facts.json + engine-routes-facts.json；真实窗口回环合成端点：保存写入 provider/key_ref 与 1M 声明、base 路由保留、无明文密钥；沿用已存密钥探测 `request_reached_endpoint: true` / `authorized_delta: 1`；错误密钥 401（判定依据为脚本 `self._json(401)` 分支 + 截图，非独立 JSON 字段，登记为证据面小瑕疵不阻塞）；双提供商共存、overlay 0600。引擎实载 `--dump-config`：opendesign 5 模型全部 `contextWindow: 1048576`、`agnescn` 路由保留、默认 `deepseek-v4.1-flash`。`~/OmniDesk/app/config/` 两份配置各 5 处 contextWindow，v1 备份 `.bak-od065` 在档。`sk-`/`Bearer` 扫描 0 命中。
- **1M 声明性质（用户须知）：** 这是给 DSH 的容量声明，**端点真实上限未验证**；若实际低于 1M，超长请求由端点报错、不静默截断。超长场景实测属于后续用户实测阶段。
- **遗留备查：** facts.json 为 `:96` 最后一次运行产物，同目录 3 张早期尝试截图非同次（shots 按目录实况生成，不影响结论）；pytest 只跑 `tests/`（全仓会误收 runtime/ 内测试，与本轮无关）。065 计划标注「待批准」但用户直接派工执行，视为事实批准，本条记录在案。
- **结论：** 065 验收通过。本批复核前已随 065 批次推送（工作区在复核前即与 origin 同步）。**下一步为用户实测（OD-ARCH-066 入口）：** 按 `docs/user-testing-checklist.md` 七步实测（约 30 分钟，含真实端点、强杀恢复、写确认卡），反馈不顺手点；ARCHITECT 待反馈后定 066 修复/打磨批次。

## OD-ARCH-064 复核结论：**通过（用户实测通道整备）**（ARCHITECT，2026-10-23）

- **独立核查：** 启动核对与 Dock 核对零代码改动属实（S1/S2 计划内允许改动但未发现缺陷）；`docs/user-testing-checklist.md` 197 行 / 7 步，旧版虚构流程（载入合成样例/生成模型初稿/保存成果）grep 0 命中；全量回归与 BUILDER 一致（cargo lib 202/0、tsc 0、pytest 26、build 通过）；`runtime/` 零差异；evidence/od-arch-064/（启动截图 + facts.json）在档，facts 记录 5 个 Dock 入口、设置 6 页签、清单校准明细与 055/060 联动复跑。清单核对现势界面一致（含多提供商设置页第 1 步已按 065 二次校准）。
- **遗留备注：** ①`app.log` 开头 13 行 corrupted 双链表错误属 23:04 旧运行历史痕迹，本轮启动无新增，继续留观（再出现须附任务号）；②BUILDER 留的应用进程 pid 97021 复核时已不在运行（用户实测时自行重启即可）；③PLANS.md 063 标题下多余 `@@` 行已由 ARCHITECT 顺手修除。
- **结论：** 064 验收通过。实测通道就绪，用户可开始实测（见 065 结论中的下一步指引）。

## OD-ARCH-063 复核结论：**通过**（ARCHITECT，2026-10-22）

- **独立核查（本机，与 BUILDER 分离）：** 全量回归与 BUILDER 一致（cargo lib 202/0 失败、tsc 0、pytest 26、npm build 通过、runtime/ 零差异）。代码抽查：①CategoryPicker 自绘浮层与 ModelPicker 同套视觉（plate-soft-inset/rounded-full/向上弹出），键盘可达（Esc/上下键/Enter）且 aria/listbox 语义齐备；②单选即收起并沿用原 onChangeTag/retag 链路，持久化格式与键未动；③styles 命名收尾属实（offline-flows.css→settings-panel.css、.ofl-v2→.settings-surface、.wb-header→.settings-pane-header，全引用同步）；④仓库根 ./0 与 060 目录错名残留已删净；⑤facts.json 落盘齐备（胶囊宽度 4 类与 5 类均 245px、回归命令与 9 张截图清单），062 的落盘缺口已补（rerun-063-check.json，exit_code 0）。
- **遗留备查：** 自定义分类色点按 key 稳定取色（展示用，不入数据），可接受；后续若用户要求固定色板再立项。061~063 三轮界面收敛全部关闭，无未清欠账。
- **结论：** 063 验收通过。S3 批次提交后推送（沿用一次性授权，推送前红线扫描 + 文件清单核对）。**下一步为 OD-ARCH-064（用户实测通道整备）**，见 PLANS.md。

## OD-ARCH-062 复核结论：**通过（chip 改下拉裁定 + 证据勘误备案）**（ARCHITECT，2026-10-22）

- **独立核查（本机，与 BUILDER 分离）：** 全量回归与 BUILDER 一致（cargo lib 202/0 失败、tsc 0、pytest 26、npm build 通过、runtime/ 零差异）。代码抽查：①settings.rs 的 thread_categories 命令只读写新字段，既有字段与命令语义未动，越界拒绝与非法输入有单测；②engineCancelModel.ts 与原 workbench 区域逐字一致（37 状态用例 + 8 静态检查通过），取消判定未绕过模型；③workbench.tsx / workbench-ui.css / OfflineFlows.tsx 已删，desktop/src 残留引用仅剩样式类名；④发送/取消合并按用户裁定 A 接首页在用按钮，界面不替代放行判定；⑤真实窗口链路：新增分类重启后仍在（study-log 骨架已建）、取消任务台账终态 cancelled、055/060 联动复跑通过。
- **裁定（用户已拍板，ARCHITECT 确认）：** 分类 chip 由平铺改下拉——同意，理由：低频操作不应常驻一行、宽度恒定可容纳分类增长、色点保留当前状态可见性；**列入 063**。
- **证据勘误（不阻塞）：** STATUS 写「11 张截图 + send-cancel-facts.json」；实际 evidence/od-arch-062/ 为 13 张截图、无 facts json（脚本输出未留档）。以实际文件为准，登记勘误；063 要求验收事实统一落盘 JSON。
- **遗留转入 063：** 仓库根 `./0` 空文件与 evidence/od-arch-060/00-home.png (deleted) 残留文件清理；.wb-header 类名与 .ofl-v2/offline-flows.css 命名收尾。
- **结论：** 062 验收通过。S4 批次提交后推送（沿用一次性授权，推送前红线扫描 + 文件清单核对）。

## OD-ARCH-061 复核结论：**通过（两项停止条件裁定 + 一项修正要求）**（ARCHITECT，2026-10-22）

- **独立核查（本机，与 BUILDER 分离）：** 全量回归与 BUILDER 一致（cargo 198/0 失败、tsc 0、pytest 26、build 通过、runtime/ 零差异）；真实窗口证据 9 张抽查与断言一致（Dock 5 入口、设置 6 分页、chip 两种选中态、输入区无挂载库且语音置灰）。代码抽查：①threadCategories.ts 只改展示层、持久化键 plain/weekly/writing/project 未动、未新增 Rust 字段；②SettingsPanel 6 分页用 hidden 切换保留原组件与表单状态、无设置项丢失；③App.tsx 兜底路由为诚实提示，不落 055 类"页面被拽走"缺陷；④Workspace.tsx 自带 HomeChat 死代码已清理（060 遗留项关闭）；⑤055/060 联动脚本复跑通过且声明 sha256 前后一致。
- **裁定 1（风险①，分类管理高级能力）：** 批准新增宿主 settings.v1 持久化字段（分类显示名/新增分类/文件夹映射/vault 绑定/初始化），**列入 062**，不在 061 范围内实施。约束：只增字段不改既有字段与命令语义；初始化只建目录骨架；映射目录必须限定在成果根与已绑定 vault 内，越界拒绝；新增分类不自动出现 Dock 图标。
- **裁定 2（风险②，workbench 源文件）：** 批准"测试区域迁独立模块 + 改测试路径 + 删 workbench.tsx 与 workbench-ui.css"方案，**列入 062**，实施前须列删除文件清单与迁移后测试对照（抽取区域逐字一致），测试必须真实跑过不能只改路径。
- **修正要求（复核发现，非 BUILDER 自述）：** SettingsPanel 根节点复用 `.workbench ofl-v2` CSS 类（styles.css L536），与"workbench 下线"口径冲突，且 OfflineFlows.tsx 已无引用仍保留。062 一并处理：设置页样式类改名去 workbench 语义；OfflineFlows.tsx 确认无引用后删除或明确保留理由。
- **口径修正备案：** "挂载库 grep 全仓 0 处"按 desktop/src 与 samples/ 范围执行（历史记录文件含该词属正常档案），接受。
- **结论：** 061 验收通过；060 复核结论（见下）一并生效。061 S5 批次提交后推送（沿用一次性授权，推送前红线扫描 + 文件清单核对）。

## OD-ARCH-060 复核结论：**通过**（ARCHITECT，2026-10-22）

- **独立重跑（本机，与 BUILDER 分离）：** cargo 198/198（含 scene_overview 新增单测）；tsc 0 错误；pytest 26 passed；npm build 通过；runtime/ 零差异。证据 evidence/od-arch-060/（7 张真实窗口截图 + facts.json + regression.txt）。
- **代码抽查：** ①scene_overview.rs 为只读新增命令，聚合口径与 pending_writes/周报汇总一致，未改既有命令语义；②ArtifactExplorer 对 frontmatter 解析失败按"无元数据"降级，不报错不阻塞；③真实窗口截图与状态条数字断言一致；④055 声明联动复跑通过。
- **限制与遗留：** OfflineFlows 分支在当前界面路径不可达（场景起草走首页，属既定设计，非缺陷）；Workspace.tsx 自带 HomeChat 未合并（随 061 任务工作台下线一并处理）；文档库 >100 文件分页未压测。
- **结论：** 060 验收通过；场景页 v2 可供用户试用反馈。

## OD-ARCH-051 复核结论：**通过**（ARCHITECT，2026-10-21）

- **独立重跑（本机，与 BUILDER 分离）：** cargo 169+2+13+3+1 全绿 0 失败；tsc 0 错误；pytest 26 passed；verify_resume_token_savings.py 11/11（证据已刷新落盘，工作区保持干净）；runtime/ 零差异。两个真实窗口脚本（profile_decl / settings_shell）需要 Xvfb 显示器，本复核环境未重跑，采信其修复测试缺陷后重跑的存证（JSON + 6 张截图在档）。
- **代码抽查（五项复核入口全部与 BUILDER 自述一致）：** ①profile_decl.rs 解析器错误面收紧（注释切断、tab、重复字段、未知键、嵌套映射一律拒绝，不猜）；②load_decl 仅 create_new、文件已存在绝不覆盖；③App.tsx 场景校准只限 5 个业务场景，设置页不会被拽走（真实窗口撞出的缺陷已修）；④profile_decl_load 锁内只取路径、async 命令不占 UI 主线程；⑤open_settings_window 只改窗口外壳属性，未触 DSH 配置读写与令牌路径。
- **三处裁定：**
  1. **声明范围窄于计划文字（只管入口+品牌，不管 Skill/插件/模型）——接受。** Skill/插件/端点的权威源在 DSH 侧（D-024/D-026），再抄一份进声明即双写，正是"模型消失"那类事故的成因。此裁定作为 D-026 的边界澄清：OmniDesk 声明只管界面呈现，引擎能力始终以 DSH 配置为唯一权威。
  2. **新增 Rust 模块 vs 停止条件③——接受，属计划内。** 影响范围本就含 Rust 宿主；停止条件本意是防"文件方案不够却擅自改既有机制"，本实现为新增独立模块、未改任何既有机制，且已如实报告。流程提醒：计划内部矛盾时，动 457 行前先停一段问一句更稳。
  3. **S4 只统一窗口外壳——接受为验收口径。** CSP 不允许注入是正确的安全边界；给 DSH 页面改肤会绑死它的 DOM 结构，DSH 升级即碎。
- **遗留：** 声明改动需重启生效且界面无提示→列入 052；omniharness-ui 克隆中剩余未提交文件已无独有资产（独有部分均已入主库），是否归档/清理由用户后续决定，不阻塞；"启动卡死"两轮真实窗口未复现，**不认定已修复**，继续观察（再出现请附任务号报给 ARCHITECT）。

## OD-ARCH-051 批次 2（S3/S4）：**待 ARCHITECT 独立复核**（BUILDER 提交，2026-10-21）

> 本节是 BUILDER 的准备材料，**不是验收结论**。按 AGENTS.md，结论须由 ARCHITECT 读实际 diff、重跑测试后写入。

- **提交范围：** `18d133f` 代码（8 文件：`profile_decl.rs` 新增 457 行、`samples/profiles/discipline.yml`、`lib.rs`、`App.tsx`、`api.ts`、`Dock.tsx`、`TopNav.tsx`、`types.ts`）/ `31b6d4a` 测试与证据 / `3f41f4e`、`7e93ecb`、`d672a4a` 记录。已推送 `origin/main`，工作区干净。
- **建议复核入口（按优先级）：**
  1. `desktop/src-tauri/src/profile_decl.rs` 的解析器——重点看 `strip_comment`（`#` 前须为行首或空白）、`split_kv`（未闭合引号、空键）、`apply_scene_field`（重复键）是否真的收紧了错误面；以及 `load_decl` 是否在任何路径上会覆盖用户文件。
  2. `desktop/src/App.tsx` 中声明到达后的场景校准：确认只作用于 `home/report/writing/project/sessions`，设置页不会被拽走（这条是真实窗口里撞出来的缺陷）。
  3. `lib.rs::profile_decl_load` 是否真的不占 UI 主线程、是否只取路径不持锁跨 await。
  4. `lib.rs::open_settings_window` 的 S4 改动是否只碰窗口属性（标题/底色/尺寸），未触及 `dsh_settings` 的配置读写与令牌路径。
  5. `samples/profiles/discipline.yml` 与运行时创建的文件是否逐字一致（脚本已断言，可复跑复核）。
- **BUILDER 本轮实跑（供对照，须独立重跑）：** `cargo test --offline` 169+2+13+3+1 全绿 0 失败；`cargo build` 通过；`npx tsc --noEmit` 0；`npm run build` 通过；`pytest tests -q` 26 passed；`git status --porcelain runtime/` 空。
- **真实窗口证据：** `tests/verification/desktop/evidence/od-arch-051/profile-decl-run.json`（三轮）、`settings-shell-run.json`、`shots/` 6 张。**注意：**Dock 入口增减与品牌名变化是人工比对截图判定，脚本只自动断言文件创建/一致性/存活/未覆盖。
- **需要 ARCHITECT 裁定的三处：**
  1. 声明只做「场景入口 + 品牌名」，未纳入 Skill 清单/插件开关/默认设置（避免与 DSH 侧权威源双写）。计划原文列了这些内容项，按 BUILDER 判断属新增数据契约，未做。
  2. 加载声明必须新增 Rust 模块（仓库无既有声明读取机制），而计划停止条件③写「若需动 Rust 代码才能加载，停下报告」。BUILDER 按影响范围写有 Rust 宿主，判定在界内并已实现。**该边界判断请明确。**
  3. S4 只统一窗口外壳，DSH 页面内部字体/配色未改（CSP 不允许注入）。是否接受「外壳统一」为验收口径。
- **复核前的一次自查发现（已修）：** 首次提交后我把两个真实窗口脚本各重跑一遍，`verify_profile_decl.py` 报 `decl_matches_sample=false`——原因是脚本复用固定临时目录，上一轮留下的声明文件让「首次创建」断言失真，属**测试缺陷，不是产品缺陷**。已改为每次重建一次性数据根（限 `/tmp`），修后两个脚本均 exit 0。提交 `d8560f7`。这条同时也是对「证据可复现」的提醒：**首次那份证据不可靠，仓库里留的是修复后重跑生成的那份。**
- **Git 状态：** 工作区干净，HEAD 与 `origin/main` 一致（`d8560f7`）。本轮未打标签。
- **已知限制：** 声明改动需重启应用生效（无热重载、界面未提示）；用户报过的「启动卡死」本批次两轮真实窗口未复现，**不认定已修复**。

## OD-ARCH-050：模型消失修复 + 应用内 DSH 设置窗口（自执行自复核，2026-10-21）

- **结论：通过；含一项计划外根因修复，用户症状已按真实窗口证据闭合。**
- **自执行说明：** 与 049 相同，本轮由 ARCHITECT 直接实现并自行复核，结论强度按此打折。
- **根因证据（文件事实 + 运行证据）：** ①`with_engine_call` 持宿主锁执行 `initialize`（90s 上限），`engine_admission_status` 旧实现需要同一把锁——代码事实；②引擎子进程 stdin 被 DSH 继承后被设为非阻塞（实测 `os.get_blocking(0)` True→False、`select` 无数据却读到 EOF、rc=0 无栈），修复后 `omni.list_models` 能正常回包且引擎存活——运行级证据。
- **实现复核：** `HostSnapshot` 不取宿主锁（新增源码级测试钉住）；设置窗口的关闭只发事件、不收尾引擎（新增源码级测试钉住）；设置服务用临时端口 + `--no-open`，令牌仅在内存、日志 `token=***`（单测 + 真实日志核对）；覆盖文件写入/归档后 `sync_overlay` 同步 supervisor 与快照。
- **实跑：** cargo **160 passed / 0 failed**、`thread_store_restart` 1 passed、`test_web_api_unit.py` **26 passed**、tsc 0、build 通过、`runtime/` 零差异。真实窗口：冷启动 12s 即显示模型、引擎持续存活、目录真实、设置窗口复用、关闭窗口后引擎存活、跟随模式后路由仍为 `agnescn/agnes-3.0-flash·public`。
- **偏差：** stdin 隔离与 `sync_overlay` 是计划外修复；前者是「引擎一启动就没」的真正原因，后者是「跟随 DSH」暴露出的配置不同步，二者都属最小必要修复，已补回归。
- **未验证：** 真实模型生成、跟随模式下思考等级实际值、设置窗口内改模型后的端到端重启链路。
- **推送状态：** 049 的 3 个提交与 050 的提交都尚未推送，等用户确认。

## OD-ARCH-049：备份提交 + 会话日志版本化 + 终检（自执行自复核，2026-10-21）

- **结论：三段完成，测试全绿；两处口径偏差可接受但须记录在案。**
- **自执行说明（限制声明）：** 本轮 049 由 ARCHITECT 直接实现（041/042/033 先例），下面的「复核」是对自己产出重跑测试与逐行读码，不等同独立第三方复核；结论强度按此打折。
- **第 1 段（备份提交）：事实成立。** 5 批提交 `5ba0738`/`f3a5dae`/`a728907`/`5a0664a`/`b3802bb` 已推 `origin/main`；每批提交前实跑 `/tmp/od029/scan-staged.py` 均返回 `OK`；提交后 `git status --porcelain -uall` 为 0 项、`runtime/` 0 项。批内文件与批准范围一致（048 全部未提交改动 + 048 复核记录 + dsh-java 审计）。
- **第 2 段（会话日志版本化）：实现与验收成立。** 逐行核对 `thread_store.rs`：写入路径经 `migrate_thread_record` 打 `version: 1`；迁移只读，单测用原始字节比对断言「迁移不得改写原文件」；未来版本既不加载也拒绝写入，并以 `issues` 逐条报原因。实跑 `cargo test --offline` → **175 passed / 0 failed**（新增 4 个版本化用例全通过）；`--test thread_store_restart` 1 passed（重建存储＝重启恢复回归）。
- **第 3 段（执行入口终检）：实现与验收成立。** `engine_submit_task` 中 `pending_writes::scene_gate(` 位于红线扫描之后、`with_engine_call(` 之前（新增源码级测试钉住）；拒绝码 `pending_write_blocked`，消息含卡片摘要。单测覆盖「同场景拒绝+指名」「他场景放行」「已结算/过期放行」「坏记录拒绝」，实跑全绿。非场景 intent（首页闲聊、AI 复盘发起）不进入拦截范围，符合「拦截范围最小化」。
- **偏差①（资源粒度＝场景）：接受。** 提交请求只有 `intent`/`prompt`/`workspace_dir`，没有目标文件名；把「同一资源」落到「同场景成果目录」是最小可判定边界，且与计划举例（AI 复盘建议卡拦住周报任务）一致。后续若任务带目标文件名，再收紧到文件级。
- **偏差②（坏记录 fail-closed）：接受，附风险。** 待确认目录有无法解析的记录时拦住任务并说明原因，符合「不静默跳过」；风险是单个坏文件会挡住该场景所有任务，需用户手动清理——错误消息给出目录路径，可操作。
- **未验证（不阻塞）：** 真实窗口点选与真实模型端点下的现场拦截体验；本轮证据为单元测试 + 源码级接线 + 构建。
- **推送状态：** 第 2、3 段的提交尚未推送（计划只把推送授权绑在第 1 段），等用户确认。

## OD-ARCH-048：对话通道切到 DSH 本地 Web API（ARCHITECT 独立复核 2026-10-21）

- **结论：通过（计划范围内）。** 开工 HEAD 15aef4f，全部改动未提交未推送，runtime/ 零差异（git diff --stat -- runtime/ 为空）。
- **独立实跑验证（本机重跑，非转述 STATUS）：** cargo test --offline 166 passed / 0 failed（含新增 every_tauri_command_runs_off_the_ui_thread 1 passed）；tsc --noEmit 0 错误；test_web_api_unit.py 25 passed；W01–W10 证据逐份核 verdict=pass（W02 流式 3 块 0.381/0.615/0.866s、拼接=终文；W04 取消 0.048s 落定；W03 失败内容为空；W07 重启恢复 resumed；W09 未知会话 0 次模型请求）；ACP E01–E20 证据 verdict 全 pass。
- **安全检查：** 引擎 diff 无凭证常量/密钥值；web_api_adapter.py 鉴权 token→cookie 后不落盘、_redact 覆盖日志与错误 detail；W01/W10 证据含 token_discarded/no_token_in_public_objects。
- **四处偏差 + 第五处夹具修正：均接受。** ①turn/end reason.kind=completed 与 finish 帧 stop 并存——按探针事实处理，正确；②route_classifier 加 channel——无此修复 web 通道经宿主准入即不可用，属最小必要修复且有测试；③失败态 error→failed 映射修正——原实现会把失败落 unknown，违反数据诚实红线，修正正确且补齐 max-tokens/interrupted/blocked 分支；④顶层错误帧 _frame_error——消除超时假象，有单测；⑤测试夹具补双通道声明——不改产品逻辑，接受。
- **卡死修复（同计划内追加）：** 根因（Tauri 同步命令占 UI 主线程）成立、修复方式（49 命令 async 化 + 首屏 4s 上界降级读缓存）不改契约与锁语义；Xvfb 慢启动实测方法修正（旧数据假阳性已声明并替换）处理诚实。回归测试钉住不变量，接受。
- **限制（不降低结论但须知晓）：** 思考块流式、personaPrefix 是否送达模型、真实模型端点、真实窗口人工抽验（用户日常桌面因 pointer grab 未复测）、wss:// 未验证；全局 HostState 锁语义未变，双引擎命令仍串行排队（已知限制，暂不动）。
- **遗留登记：** personaPrefix 探针未命中——待真实模型联调时以模型可见输出侧证；真实窗口流式体验待用户实测。

# ARCHITECT 审查记录


## 2026-10-21 独立复核：OD-ARCH-045（ARCHITECT，047 第 1 步补做）

- **背景：** 046 复核时只顺带确认了 `thread_store` 测试存在，045 六步没有完整复核记录。本段按 045 原文六条逐项核对，不重复 046 已覆盖的写确认制/调度/降级部分。方式是不采信 STATUS 自述，直接读源码、重跑定向测试、核对证据文件。
- **第 1 步（线程持久化）：事实成立。** `thread_store.rs` 存在，按项目分目录、原子写、拒绝不安全 id；`app_paths.rs` 有 `threads` 字段；`lib.rs` 挂接 `thread_store_load/save/delete` 三条命令。本轮实跑 `cargo test --lib thread_store` → **8 passed / 0 failed**，`--test thread_store_restart` → **1 passed / 0 failed**。`git status` 中无任何线程记录文件。
- **第 2 步（成果目录树 + 含下级开关）：事实成立。** `list_offline_artifacts_with(include_subdirs)` 按深度递归、父目录行先于子项；前端 `ScenePage.tsx` 有本开关、默认关闭并读本地偏好；`api.ts` 透传参数。截图 `02/03/04/05` 覆盖开/关两态与嵌套文件打开。
- **第 3 步（首提卡顿）：事实成立。** `lib.rs` 的 `engine_submit_task` 已是 `async fn`；截图 `25/26` 显示启动期间点 Dock 界面已切换、取消后台账落 `cancelled`。
- **第 4 步（AI 复盘）：功能成立，原偏差已由 046 修复。** 045 当时建议文件落在数据根顶层（截图与 TEMP 路径可佐证），确实不满足"落该场景目录"；046 的 `task_shell.rs::save_offline_artifact` 已按 `WriteSource::Model | Scheduler` 写入场景子目录（`pending_writes::scenario_dir`），并有 046 证据 README 的落盘路径佐证。**045 当时的截图仍是顶层，历史结论不改写。** 遗留：045 的第 4 步偏差在 045 这一轮本身未修，验收口径以"功能达成 + 偏差已由后续计划修正"记。
- **第 5 步（DSH Web 本地 API 只读验证）：事实成立。** `web-api-probe.md` 与 `web-api/*.json` 齐全；抽查 `overlay.yml` 为 `fake/synthetic-model` 回环端点、`apiKeyEnv` 仅为环境变量名（无密钥值），`cancel.json` 的 `reason.kind=aborted`、`durable-history.json` 有会话记录结构。未验证项（思考块流式、工具事件）保持"未验证"。
- **第 6 步（整批回归）：本轮独立重跑，结论成立且更强。** `cargo test --offline` → **165 passed / 0 failed**（045 当时为 154，046 增加 11 项）；`npx tsc --noEmit` exit 0；`npm run build` exit 0。四张收尾截图存在。
- **证据卫生：** `od-arch-045`（5.1M / 37 张）与 `od-arch-046`（2.2M / 13 张）内未检出凭证、令牌、身份证或手机号；命中的长数字串经抽查为毫秒时间戳。两个目录均在仓库内、不含真实业务材料。
- **已知缺口（不阻塞，已登记）：** ①首页对话仍未接线程存储（`HomeCanvas.tsx` 渲染 `HomeChat` 未传 `onMessagesChange`，本轮源码复核仍如此）；②第 5 步思考块与工具事件未验证；③045 截图未重拍，历史偏差说明以文字记录。
- **结论：OD-ARCH-045 六步均可复核，功能达成；第 4 步的历史偏差已由 046 代码修正，不追溯改写 045 当时的证据。验收通过（带上述三项已知缺口）。**

## OD-ARCH-044：Dock 悬停只保留小字提示（2026-10-21）

**结论：重复的原生大号提示已去掉，图标右侧的小字提示保留，可访问名不变。**

- **根因：** Dock 按钮上挂了 `title` 属性 ⇒ WebKit/浏览器**再弹一个原生提示框**（大号、黑色、
  出现在图标下方），与图标右侧自绘的小字提示（`项目跟踪 · 模型起草`）重复。
- **处置：** `title` → `aria-label`（`desktop/src/shell/Dock.tsx`）。原生提示框消失 ✓；
  **可访问名保持不变**（读屏与自动化脚本仍按「周报与随记（模型起草）」这类名字定位 ✓）。
- **验证：** 真实窗口悬停截图 `evidence/od-arch-044/shots/od044-dock-hover.png`——只剩小字提示 ✓。
  回归 `tsc` 0、构建通过；本轮未改 Rust。

## OD-ARCH-043：输入区与底部状态行微调（2026-10-21）

**结论：用户两条微调已落地并截图佐证；顺带补上「打开时默认显示卡片」的缺口。**

- **删除「↵ 发送」**：发送按钮前不再有那行字；同处的「提交已关闭」也一并去掉
  （权限未开启时输入框占位文字已写明原因，不重复提示）。
- **底部状态行居中**：「模型权限…」与「Token 上下文…」两个模块由左对齐改为居中
  （容器加 `justify-center`），可选的「工作目录」提示同行居中。
- **补缺口（打开时默认显示卡片）：** 窗口打开时 WebKit 会补一次 mousemove（只是「指针在窗口里」，
  不代表用户动过鼠标），此前它会立刻把卡片藏掉；现在**第一次 mousemove 只记录位置、不改层级**，
  之后位置真的变了才按指针位置判定 ⇒ 打开即显示卡片（与 OD-ARCH-042 的口径一致）。
- **追加：取消提示移出输入框。** 「取消：引擎以 stopReason=cancelled 落定」原本渲染在**输入框内部**，
  会把输入框撑高一行；现改为渲染在**底部状态行、Token 模块之后**，输入框恢复固定高度。
- **验证：** 真实窗口截图 `evidence/od-arch-043/shots/`（`od043-input-row.png` 打开态与版式；
  `od043-generating.png` / `od043-cancel-note.png` 真实端点发一条并中途取消 ⇒ 取消说明落在状态行 ✓、
  输入框不再被撑高 ✓）；回归 `tsc` 0、构建通过；本轮未改 Rust。

## OD-ARCH-042：顶部卡片的默认显示 / 静止恢复 / 输入时不打扰（2026-10-21）

**结论：用户三条要求全部落地并四态截图佐证。**

- **规则（在 OD-ARCH-040 的互斥规则之上加「静止恢复」）：** 刚打开时卡片**显示**；
  指针进入对话区 → 卡片隐藏、对话上移占满顶部；**鼠标静止满 5 秒**且输入框**无焦点** →
  卡片**重新显示**、对话回到卡片下方；输入框**有焦点**时**不恢复**（用户正在写，不该被打扰）。
  卡片与对话**始终互斥**，不会重新出现半透明叠字。
- **实现：** `SceneScaffold` 记录最后一次鼠标移动时间、每 500ms 判定静止；
  卡片让位与对话上移由同一个开关决定（`chatExpanded = chatActive && !(idle && !inputFocused)`）；
  输入框焦点由 `HomeChat` 通过新增的 `useChatInputFocus` 上下文上报。
- **四态实证（真实窗口）：** 打开=卡片显示 / 指针进对话区=卡片隐藏 / 静止 6 秒=卡片回来 /
  输入框有焦点静止 6 秒=卡片仍隐藏。截图见 `evidence/od-arch-042/shots/`。
- **过程中的一处插曲（已排除）：** 首轮实测「静止后卡片不回来」，原因是**上一轮测试点过输入框、
  焦点一直留着**——焦点态本就不恢复卡片，属规则内行为。临时状态输出已移除。
- **回归：** `tsc` 0、构建通过；本轮未改 Rust。

## OD-ARCH-041：场景切换卡顿的定位与修复（2026-10-21）

**结论：根因定位到源码级并修复，切换耗时从 6.07s 降到 0.62s（真实窗口实测）。**

- **根因：** `engine_admission_status` 与 `task_list_model_routes` **都在 Tauri 主线程上同步 spawn
  一个 node 进程**（端点分类脚本 / 模型路由脚本，各 ~1-2 秒）；而对话区在**每次场景切换时重新挂载**，
  于是每次切换都重拉这两项 ⇒ 同步命令占住主线程 ⇒ WebView 无法重绘 ⇒ 整页卡住。
- **实测基线 → 修复后：** 首页→周报 **6.07s → 0.62s**；周报→首页 **3.51s → 0.48s**；
  首页→项目跟踪 **6.03s → 0.46s**；项目跟踪→首页 **3.35s → 0.73s**（AT-SPI 轮询口径，约 0.2s 粒度）。
- **处置（两处，缺一不可）：** ①宿主侧把两条命令改成 **async**（node 进程那段等待离开主线程）。
  *约束与处置：* Tauri v2 的异步命令 future 必须 `'static`、**不能借用 `State<'_, _>`**，
  故改用 `AppHandle` 取一次状态、立刻克隆出所需值并放掉锁（`admission_context` 语义不变）。
  ②前端给**模型候选**（缓存到失效）与**端点事实**（2 秒短缓存）加缓存，并合并同 key 的在途请求；
  失效点为改选模型/思考等级与提交被拒（`invalidateEngineFactsCache`）。
- **回归：** Rust 143/143、tsc 0、构建通过、verify_conversation_prompt 24/24、verify_ai_draft 27/0、
  verify_scenario_materials 5/0。证据：`tests/verification/desktop/evidence/od-arch-041/README.md`。
- **未解决（已记录）：** **首次提交仍会卡一下** —— `engine_submit_task` 同步且要启动引擎进程
  （DSH node 启动 ~2-4 秒）⇒ 第一次提交时界面短暂无响应。改异步需重构「引擎调用守卫」与宿主锁的
  持有方式，风险较高，**另立计划**，本轮未动。

## OD-ARCH-034：真多轮接线——去掉上文重放（2026-10-20，用户批准执行）

**结论：落地并通过三层验证（单元 27/27、真实引擎合成端点 33/33、提示词 24/24、真实窗口台账实证）。
实施中发现计划的一处措辞与 DSH 实际语义不符，已按事实修正（不是绕过验收）。**

- **做了什么：** 同一工作台线程复用同一个引擎会话——首轮 `session/new`，其后在同一会话里续发，
  **工作台不再重放上文**。引擎侧 `SubmitTaskRequest.engine_session_id` → 适配层按四种方式取得会话：
  `new`（新建）/ `continued`（本进程内活会话，直接再发 prompt）/ `resumed`（不在本进程，`session/resume` 重开）/
  `degraded`（续接失败，降级为新建并**如实标注**）。回执、任务台账、事件都带 `session_mode` ⇒ 可查。
  前端：线程记录持久化 `engineSessionId`（本地存储，不进仓库）；提示词组装新增 `continuation` 模式；
  降级时**下一轮自动补一次重放**并显示说明。改动范围：适配层、协议入口一处、宿主命令一处、
  提示词组装、线程存储、对话组件与两个调用方。**未新增通信层、未改协议、未动 runtime/、未动模型路由与凭证。**
- **计划措辞与事实的出入（已修正）：** 计划写「后续轮次经 `session/resume` + `session/prompt` 续发」。
  实测 DSH 的 `session/resume` **只接受已落盘、可重开的会话**（`persistence.stat` 取不到即
  `session is not resumable`），对**当前进程里还活着**的会话会返回 `invalid_argument`
  ——这是本次实测拿到的硬事实（首版实现按计划写「先 resume」，第二轮直接降级失败才发现）。
  正确接法两条：进程内续发**直接再发 prompt**（`continued`）；进程重启后才 `session/resume`（`resumed`）。
  两种都不可用才降级新建（`degraded`）。**验收标准「后两轮 session/resume」按事实修正为
  「第二轮进程内续发、重启后 resume」**，语义目标（同会话续发、不重放上文）完全达成。
- **关键证据：** 合成端点收到的**第 2 次请求消息条数 3 → 5**（引擎带着上文继续），
  **重启后 5 → 7**（历史跨重启仍在）；同时适配层发往引擎的**第 2 轮提示词只有本轮输入**
  （没有重放）——两者同时成立才说明「真多轮」而不是「换了种方式重放」。
- **真实窗口：** 正式实例连发两轮，任务台账 `mode=new → continued`，**引擎会话号相同**
  （`e88c3e41`）⇒ 界面级实证。**诚实限制：** XTEST 打出的字符会乱序，界面文本是乱码，
  上下文是否延续以台账与合成端点证据判定，不以那段乱码文本判定。
- **回归：** Rust 143/143、tsc 0、构建通过、verify_conversation_prompt 24/24、verify_ai_draft 27/0、
  verify_scenario_materials 5/0；新增 `verify_multiturn_unit.py`（27/27）与
  `verify_true_multiturn.py`（33/33）。证据：`tests/verification/desktop/evidence/od-arch-034/`。
- **未解决（本轮范围外）：** ACP 仍**没有 token 级流式**（源码级事实未变，思考与正文在提交时同时到达）；
  首页对话仍无线程记录（既有缺口，应用重启后首页新开会话并重放可见上文）；
  降级那一轮本身丢上下文（已提示且下一轮自动补回）。

## OD-ARCH-040：对话为主的两层布局（卡片在下 · 对话在上 · 指针进对话区卡片渐隐）（2026-10-20）

**结论：按用户澄清后的口径落地并截图佐证；上一轮（039）对「渐隐」口径的误读已撤回。**

- **口径纠正（重要）：** 上一轮把「鼠标焦点在哪里，哪里显示在最前端，其他模块渐隐显示在他下面」
  理解为**卡片之间**互相抢层级（做出「卡片带之间的渐隐 + 前置」）——**这是误读，已撤回**。
  用户口径是**页面两层**：对话占满上下全屏（上层），顶部卡片在下层平时可见，
  **指针进入对话区（对话历史/输入框）时卡片渐隐、对话上移顶到卡片位置之上**。
- **实现：** `SceneScaffold.tsx` 改为两层——卡片区 `absolute inset-x-0 top-0 z-0`（背景层，高度用
  `ResizeObserver` 实测）；对话区 `relative z-10 flex-1`，用 `padding-top`（卡片带高度 ↔ 0）做 300ms 位移。
  **判定用指针在容器内的纵向位置**而非元素 hover：两层叠加时元素 hover 互相遮挡，纯 hover 会出现
  「卡片被盖住后永远回不来」的死角。指针离开窗口即恢复。
- **时序修正（实测发现）：** 卡片内容异步就绪 ⇒ 卡片带高度会变；高度一变**立刻重判指针位置**
  （否则首帧高度为 0 会把「指针在卡片上」误判为「已进入对话区」，表现为卡片一上来就没了）。
- **撤回：** `HomeCanvas` / `ScenePage` 里的 `layer()`/`focus` 卡片互渐隐实现全部删除。
- **渐隐幅度按用户反馈调整：** 初版渐隐到 `opacity-0`（卡片彻底消失），用户指出不妥 ⇒ 改为
  **`opacity-30`**（卡片保留一点存在感，仍让位给对话）。
- **两层互斥（用户第三次反馈，最终口径）：** 卡片停在 30% 时两层半透明文字叠在一起、**字迹混乱**
  ⇒ 改为**谁在焦点谁全亮在上，另一个透明度为 0**：指针在卡片区 → 卡片全亮、对话收在下方（不重叠）；
  指针在对话区 → 对话整段上移占满顶部、卡片层降到 **0**（彻底让位，文字不再叠）。
  上一版为此加的「指针落在对话历史还是输入框」上报（`useChatLayerHover` 上下文）在互斥规则下多余，
  **已撤回**（透明度只看指针纵向位置，位移判定不变）。
  **真实窗口实证（正式实例）：** 对话区态=卡片 0、对话满屏、文字干净；卡片区态=卡片全亮、对话在下方。
  截图：`od040-chat-hover.png`、`od040-input-hover.png`、`od040-cards-visible.png`。
- **证据：** `evidence/od-arch-040/shots/`（首页卡片可见 / 首页指针进对话区 / 场景页两态）。
- **回归：** `tsc` 0、构建通过、`verify_conversation_prompt` 15/15；本轮未改 Rust。
- **未验证：** 渐隐与位移的**手感**（时长/幅度）待用户实际体验后调；无指针设备降级路径未验证。

## OD-ARCH-039：卡片层级改为「聚焦在前、其余渐隐」+ 接缝选择复盘（2026-10-20）

**结论：UI 改法按用户最终口径落地并截图佐证；并把「思路是否走偏」这个问题正式记录为待裁定事项
（docs/decisions.md **D-024**），本轮不动引擎与协议。**

- **UI（按用户口径）：** 不上收、不悬浮；卡片留在原位，**鼠标在哪一块，那一块在最前（z 序更高、透明度满），
  其余渐隐到它后面**。首页三块（机器人 / 今日重点推进 / 日程节律）与场景页三块（提醒·建议 / 历史对话 / 成果·进度）
  都已实现。上一版做过的「上收 + 悬停浮出」已**撤回并删除**（`PeekPanel.tsx` 已删）。
  截图：`evidence/od-arch-038/shots/od038-home-idle.png`、`od038-home-focus-today.png`。
- **思路复盘（用户提出「是思路规划有问题？」）：** 结论是**有**——但不是「引擎契约天生难」，
  而是**接缝选窄了**：我们从 DSH 的 `omni.submit_task`（每次 `session/new`）这条窄缝进去，
  于是 DSH 本来就有的 token 级流式、会话续接、思考块、模型/思考等级目录，只能一个个手工重做
  （流式是本地逐字效果、思考链是补传文本、多轮是重放上文）。按用户原意（基于 DSH 原代码增改 UI）
  这些能力本可直接继承。两条候选路径与代价已记入 decisions.md **D-024（待裁定）**：
  A 以 DSH 应用为底座改 UI；B 保留我们的 UI、把引擎侧换成 DSH 本地 web API。
  **建议先做最小验证**（让 DSH web 应用跑起来、走它的本地 API 发一条消息，核实流式/续接/思考块是否真的存在），
  核实成立再决定路线。本轮**未改引擎、未改协议**。
- **补充核实（本地 runtime 源码，文件级）：** ①`dsh-acp/lib/index.js` 的 `assistantUpdates()` 只把**已提交**消息
  转成更新 ⇒ ACP 侧**确实没有 token 级流式**（这是唯一确凿的引擎限制）；②同一包里已有 `session/new` /
  `session/prompt` / `session/resume`，且我们的 `acp_adapter.py` **已实现 `session/resume`**，
  但 `submit_task` 每次都 `session/new` ⇒ **真多轮不需要新契约，是我们自己没接线程↔会话**；
  ③`dsh-api-session-controller` 暴露 `selectModel` / `modelCatalog` / `fork` / `page` 等 Remote ⇒ 模型目录、
  每会话选模型、会话分叉引擎里本来就有；④`dsh-client-connection` 的 `openStream` + webserver 的
  `text/event-stream` 处理 ⇒ 实时事件流在 web 路径上存在；⑤profile `cordis.patch.yml` 支持插入 bundle
  ⇒ 路径 A（注入自有 UI）有落地手段。
- **回归：** `tsc` 0、构建通过（本轮未改 Rust）。

## OD-ARCH-038：对话为主的工作台（思考等级默认 / 宽对话 / 悬停面板）（2026-10-20）

**结论：三点都落地。** 思考等级的**性能差异经用户实测确认参数生效**（关=秒回、中≈5s、高>1min），
工作台默认改为「关闭」；思考链**文本**不再被丢弃并在界面默认展开；对话气泡放宽到 80%；
上半部分卡片收进**悬停展开**的把手，纵向空间全部让给对话。

- **思考等级：** 用户实测三档差异显著 → `reasoningEffort` 确实生效；默认档改为「关闭」（快），
  其余档位仍在模型选择器内。此前「没有思考链」的原因查实：DSH 的 **ACP 通道**在助手消息**提交时**才产出更新
  （块级），思考块与正文同时到达，而适配层此前**只上报思考的字数、丢掉文本** → 已改为连文本上报；
  界面在助手气泡里**默认展开**「思考过程（模型产出，不计入最终结果）」；无思考时如实写明原因。
  **诚实性：** 思考与正文**同时到达**（不是边想边出）——DSH 网页端的流式思考走 harness 事件流，
  当前引擎契约下拿不到，须单独批准才改。
- **宽对话：** 用户与助手气泡上限由 `max-w-2xl`/`max-w-3xl` 改为 `max-w-[80%]`。
- **悬停面板（两层）：** 新增 `PeekPanel`：默认只留细把手，**悬停时面板浮在对话之上**，
  也可点击固定展开；首页「今日概览」与场景页「提醒 · 成果 · 进度」都收进把手，对话区拿到全部纵向空间。
  真实窗口核对：两个把手可见、悬停可展开、场景页历史对话栏正常渲染 ✓（`evidence/od-arch-038/shots/`）。
- **回归：** `tsc` 0、构建通过、`verify_conversation_prompt` 15/15、Rust 143/143（本轮未改 Rust）。
- **未验证：** 思考链的真实内容（合成端点不返回 reasoning ✗）、悬停手感（由用户实际体验）。

## OD-ARCH-037：对话延迟与「看起来没回复」的三处界面洞（2026-10-20）

**结论：延迟的真实构成查清了（台账 + 模型自述）；界面侧三个会让人以为「没有回复」的洞已修；
「继续」那次的根因仍未抓到（引擎确实回复了），已加可见诊断等复现。**

- **延迟构成（文件事实 + 模型自述，非推测）：** 正式根任务台账显示——「不用你管了」**17.06s**、
  「继续」**4.19s**，更长的几轮 **97.45s / 62.51s / 35.63s**。模型自己在回复里说明了原因：
  「① 模型推理本身就有一截延迟 ② 我在回答前可能做了一些『后台动作』（如先确认 `~/YvanAI/memory/` 里有哪些文件）」，
  另一轮直接列出它读了什么：「写作风格规则、结构模式、常用规范表达、避坑清单、模板目录说明与历史范例说明」——
  即 **agent 在做工具调用、读 skill 资料**。DSH 原版快，是因为它的 profile/技能组合不同，
  **且原版是服务端续会话（不重放上下文）**。
- **我们这侧已改：** ①轮询 **1500ms → 300ms**（终态与正文最多快 1.2s 上屏）；②生成中显示**已等待秒数**（不再像卡死）；
  ③生成中再次发送**不再静默丢弃**，给出可见提示；④任务成功但**正文为空**时显示明确说明与「重新查询」指引（不再留空气泡）；
  ⑤上下文重放**窗口化**（最近 4 条用户 + 2 条助手、上限 2500 字符，原为全量 + 4000）→ 提示词更短、模型更快。
  回归：脚本级验证 **15/15**（新增轮次窗口 3 例）、`tsc` 0、构建通过。
- **仍在的结构性差距（待用户决定）：** 工作台每轮是**独立任务**（引擎每次新建会话），连续性靠重放上下文，
  提示词因此永远比「服务端续会话」长。真正的修法是给适配层加「向既有会话续发」的动词
  （`session/resume` + `session/prompt`）：不重放、真多轮、延迟回到原版水平 —— 需单独批准。
- **「继续 没有任何回复」：** 台账证明引擎**确实回复了**（内容为「当前材料不足，我无法基于未出现的数据做复盘…」，
  4.19s）→ 问题在界面这一侧。三个可能的洞已修，但**根因未抓到**：本机合成键盘进不去那个无边框输入框，无法自动复现；
  现在若再现，界面会明确显示「引擎报告已完成，但没有返回任何正文」并给「重新查询」，据此可定位到具体任务。

## OD-ARCH-036：修三处用户实测反馈（命令未注册 / 红线误拒 / 文案误导）（2026-10-20）

**结论：三处都已修并实测。** ①模型切换报 `Command task_set_model_route not found`——两个新命令**没注册进
Tauri 的 `invoke_handler`**（我漏了）；②周报对话被红线拒绝——**首页对话没传 `userInputText`**，
宿主于是扫描**整条提示词**，而场景预设的写作规则里含「案件/线索/未公开」，被当成用户输入误判；③
「本地数据根未解析」是**误导性文案**——宿主给的真因是「模型端点未获许可，已拒绝准备工作目录」，被前端丢掉了。

- **①命令注册：** `task_list_model_routes` / `task_set_model_route` 已加入 `invoke_handler`。
  实测：正式实例的下拉现在能列出各 DSH Profile 的模型与端点（agnescn / agnes-3.0-flash 等）✓。
- **②红线误拒：** `HomeChat` 提交时带上 `userInputText: <用户原文>`（与三场景页一致的口径：
  红线只扫用户原始输入，不扫拼好的提示词）。已加回归断言：带场景规则的提示词 + 干净用户原文 → 放行；
  不带用户原文 → 仍会误拒（锁住这个坑）✓。
- **③文案：** 工作目录被拒时**原样带出宿主的原因**（不再用「本地数据根未解析」这种误导说法）；
  权限行改为点名当前端点类别（如「当前端点：非回环 · 公网——请到设置里开启对应类别」），
  避免「开了本机模型却仍然被拒」这种看不懂的状态。
- **回归：** Rust **143/143**（含新增断言）、`tsc` 0、构建通过。
- **未验证：** 真实发一条消息的端到端（真实窗口的合成键盘注入仍进不了输入框，需用户人工点）。

## OD-ARCH-035：对话观感对齐 DSH（流式呈现 + 思考链 + 模型选择器重做）（2026-10-20）

**结论：三项都落地了，但有一处**架构事实必须讲清**——DSH 的 **ACP 通道不提供 token 级流式**
（ACP 的更新来自「已提交的助手消息」；DSH 网页端的逐字效果走的是它自己的 harness 事件流）。
因此本轮给的是：①**思考链**真实透出并展示（此前被适配层丢弃）；②**逐字呈现**的本地呈现效果（明确标注，
不改写正文）；③**模型选择器**重做为与工作台同风格的**自绘下拉**，并加入**思考等级**（对应 DSH 的 `reasoningEffort`）。**

- **架构事实（源码事实）：** `dsh-acp/lib/index.js` 的 `assistantUpdates()` 注释即「Convert one committed
  assistant message」——ACP 更新按**已提交消息**产出，**没有** token 级流式；块级（block）是 ACP 的最细粒度。
  → 引擎侧能给的极限是「消息提交后按块到达」。真正的 token 级流式需要换引擎协议（harness 事件流），超出本轮范围。
- **思考链：** 适配层此前只上报 `thought_delta` 的**字数**、丢掉文本；现已连文本一起上报（仍标注「不计入最终结果」），
  前端在助手气泡里以可折叠的「思考过程（模型产出，不计入最终结果）」展示 ✓。
- **逐字呈现：** 前端在任务未终态时逐字显现正文（`StreamedText`），终态后即完整文本；**不改写、不补全**，
  文本始终以引擎返回的最终正文为准——是**呈现效果**，不是引擎流式，界面与文档均如实标注。
- **模型选择器：** 改为自绘下拉（`ModelPicker.tsx`）：圆角胶囊触发器 + 浮层面板（与工作台同一套卡片/描边/阴影），
  列出各 DSH Profile 的 provider/模型（含端点），并提供**思考等级**：默认 / 关闭 / 低 / 中 / 高
  （对应 DSH 的 `reasoningEffort`，写入路由覆盖的 `acp` 配置）；切换仍会重启引擎 ✓。
  截图：`evidence/od-arch-035/shots/od035-chat-picker-open.png`。
- **诚实性：** 「默认」＝**不传**该参数，由模型/Provider 决定；非默认等级是**请求参数**，Provider 不支持时可能直接报错
  （界面显示真实原因，不静默降级）。
- **回归：** Rust **143/143**、`tsc` 0、构建通过、Python 语法检查通过。
- **未验证：** 真实模型下的思考链是否真的产出（取决于模型与 Provider）；思考等级被 Provider 接受与否（需真实调用）。

## OD-ARCH-034：模型路由切换（对话流里的模型下拉，2026-10-20）

**结论：根因查实并修好——用户实测「在 DSH 界面配了自定义模型，回 OmniDesk 刷新端点仍是 deepseek、任务只拿到 partial」，
原因是 DSH 的**模型选择按 Profile 存**：用户改的是 `web` Profile，引擎跑的是 `acp` Profile，两边**只有凭据共享**。
现已提供模型下拉（对话区右下角、发送按钮前），选中即写入 OmniDesk 自己的路由覆盖文件，引擎与端点判定共用同一份。**

- **根因（实测）：** `web` Profile 生效 `agnescn / agnes-3.0-flash`（baseURL `https://api.agnes-ai.cn/v1`，
  key 以 ref `AGNESCN_API_KEY` 形式存在**共享**凭据里）；`acp` Profile（引擎所用）仍是 `deepseek-official /
  deepseek-flash` 且无 key → 任务 partial/失败。「刷新端点」显示 deepseek 是**如实反映引擎那条路**，不是界面骗人。
- **实现：** 新增 `model-routes.mjs`（只用 DSH 公开的 `--dump-config`；`--list` 列候选、`--write-overlay` 写覆盖文件，
  **只含非机密字段**，key/secret 一律剔除）+ `model_routes.rs`（宿主调用）+ 引擎 patch 列表附加该覆盖文件 +
  `task_list_model_routes` / `task_set_model_route` 两个命令 + 对话区下拉。
- **实测：** 覆盖后分类器判定为 `agnescn / agnes-3.0-flash @ api.agnes-ai.cn`（public）；应用内下拉显示
  「模型：agnescn / agnes-3.0-flash」，位置为**右下角、发送按钮前**（截图
  `evidence/od-arch-034/shots/od034-chat-model-dropdown.png`）；覆盖文件里**没有任何 key**。
- **边界：** 写的是 **OmniDesk 自己数据根**下的那一个文件，**不碰用户的 DSH Profile**；切换会停掉并重启引擎
  （路由在引擎启动时确定），停不掉就**明确报错**。
- **需复核确认：** 本轮新增了两个宿主命令与一个覆盖文件机制，超出此前「宿主不新增命令」的边界——
  但这是用户明确要求的功能（对话里要有模型下拉），故实现并在此标注。
- **未验证：** 切换后的真实模型往返（需真实 key，属用户真实使用范围）；对已发出的任务无效。

## OD-ARCH-033：场景模块页（骨架化 + 双栏 + 提醒/建议栏 + 会话打标，2026-10-20）

**结论：结构目标 1/2/3 与 v3 的提醒/建议栏已落地并可截图佐证；目标 4（线程落到仓库外数据根）与
目标 5（成果目录「含下级目录」开关）**按计划的停止条件停下**——两者都需要宿主新增能力，
本轮不擅自扩面，已逐条报告。**

- **实现：** 新增 `SceneScaffold`（首页与场景页共用骨架）、`ScenePage`（提醒栏 + 双栏 + 对话区）、
  `AnnouncementBar`（规则提醒 + AI 复盘建议）、`scenes.ts`（场景注册表）、`sceneThreads.ts`（带标签的线程存储）；
  首页版式**未变**（截图 `evidence/od-arch-033/shots/od033-01-home.png`），三个场景入口改渲染场景页。
- **数据来源纪律：** 「进度」页签与「规则提醒」一律走**既有**确定性汇总命令（`task_project_summary` /
  `task_weekly_summary`），不调模型算、不编造数字；无输入时写「暂无」。成果档案只列**本场景自己保存**的成果
  （按 scenario 过滤），界面写明不读用户其它目录。
- **AI 复盘建议（受限实现）：** 前端定时器 + 频率设置、**默认关闭**、按钮注明「会调用一次模型」、
  复盘输入只取本场景成果与对话、结论存为本场景成果文件、公告栏显示最新一条并可打开历史；
  「趋势热点」**未做**（需外部数据源，用户未授权）。走现有引擎与三档权限判定。
- **会话打标：** 入口即标签、事后可改挂、标签持久化 ✓；标签是左栏过滤与成果归类的唯一依据（不用模型猜归类）。
- **停下并报告的两项（按计划的停止条件）：**
  ① 线程记录要真正落到 `~/OmniDesk/...` 并按项目分目录 → 需要宿主新增读写能力（现为浏览器本地存储，
     **已确认不在仓库内**）；② 成果目录「是否含下级目录」→ 宿主的成果列举是平铺匹配，没有目录树能力，
     需要宿主新增目录列举命令。两项都**不擅自新增宿主命令**。
- **回归：** Rust **144/144**、`tsc` 0、构建通过、`verify_conversation_prompt` 12/12、`verify_ai_draft` 27/0。
- **仍未验证：** 真实窗口用合成键盘驱动对话（字符进不了输入框，见 OD-ARCH-031 §B.4），端到端打字链路需人工点验；
  AI 复盘的真实跑通；「数据分析占位页」未新增（Dock 现有「任务工作台」即分析试点，另加空占位页只多无用入口）。

## 用户校正（2026-10-20，OD-ARCH-031 复核后）：首页结构方向修正

**用户裁定：** 首页不出现项目/会话目录树或左右分栏。项目与会话集中到一个 Dock 模块页里；首页保持用户已确认的版式（顶部模块 + 全宽对话），不含侧栏。

**ARCHITECT 复核对照（独立验证）：**

- Part B 现状（提交 153b0fa + 后续未提交微调）与该裁定不符：Workspace 把 ProjectRail（项目/会话栏）放进了首页，左侧栏 + 右侧对话构成分栏。这偏离用户此前对首页定版的语义（对话中枢、无目录树）。
- 结构方向本身（项目-会话-对话流、三场景降级为开场预设、向导页保留）与用户要求一致，错在摆放位置：把「DSH 项目集中管理」错误理解为要在首页新增项目树。用户实际要求：首页继续纯对话中枢；项目/会话管理收进一个独立模块页（Dock 图标进入）。
- 未提交遗留：工作区存在 ProjectRail.tsx 一份未提交修改（340 行改动，方向已是「模块页内部左侧栏」），与用户裁定方向一致但未完成、未验证。
- 当前构建实际失败（独立复测）：npm run build 有 4 个 TS 错误（App.tsx 的 Workspace onNavigate 类型、HomeChat.tsx engine_session_id 字段、ProjectRail.tsx 未使用变量）。执行方登记「tsc 0/构建通过」与本轮独立复测不符——该未提交修改发生在登记之后、未重跑检查。复核修正：Part B 在当前工作区状态下不通过，须先完成模块页搬迁并修构建。

下一轮修复要求（并入 OD-ARCH-032 待批准计划）：

1. 首页恢复为用户定版版式（HomeCanvas 全宽对话，无 ProjectRail、无左右分栏）。
2. 项目/会话/预设/新建对话整体迁入一个 Dock 模块页（优先复用现有 sessions 页结构），首页对话可直接发起（预设芯片可留在首页，项目树不进首页）。
3. 修复 4 个 TS 错误，tsc 0、构建通过、Rust 测试全绿后重新提交复核。
4. 架构事实（每轮新 DSH 会话 + 上文重放）继续如实呈现，作为后续「真多轮会话」扩面的决策依据；本轮不扩通信层。

## OD-ARCH-031 Part B：项目与会话模块（实施与复核，2026-10-20，含用户当场纠正）

**结论：按用户纠正后的口径落地并可用——首页**回到用户已确认的版式**（无项目目录树、无左右分栏），
DSH 原本的「项目 / 会话」收进**一个模块页**（Dock 入口「项目与会话」）；三场景降级为开场预设。
脚本级验证 12/12；但「真实窗口用合成键盘驱动对话」这一段**没有**端到端自动化证据，已如实标注。**

- **先记一次我的误读并已改：** 早先一版把项目/会话树放到了**首页左侧**、首页做成左右分栏。
  用户当场纠正：左侧 Dock 是进各场景的入口，**DSH 的项目要集中到一个模块、一个分页**，
  **首页不再显示项目与对话的目录树和栏目**。现已改回：首页 = 确认版式（`shots/od031-home-no-rail.png`），
  项目与会话在独立模块页（`shots/od031-module-projects.png`）。
- **模块页结构：** 左侧两个分页——**引擎项目**（DSH 真实工作区分组与其会话，只读元数据，界面不拼路径）
  与**本机对话**（工作台自己的草稿，第一轮提交后由引擎产生 DSH 会话并归入「引擎项目」）；
  右侧对话区（预设芯片 + 消息流 + 输入框 + 权限/Token 行）；底部折叠「OmniDesk 任务记录（只读）」——
  那是本工作台自己的台账，与 DSH 会话不是一回事，分开呈现。Dock 标签「会话与任务」→「项目与会话」。
- **实测：** 两张真实窗口截图（首页无目录树、模块页结构）✓；`verify_conversation_prompt.py` **12/12 通过**
  （上下文累积、预设注入用**真实流程包**、截断、不可关闭规则不丢）；`tsc` 0、构建通过、`verify_ai_draft` 27/0、
  Rust 144/144。
- **刻意不做：** 不让界面「自造项目」——去掉了早先那版「手填绝对路径当项目」的入口，
  因为宿主给的是**不透明工作区 token**（界面不得自行拼路径），项目只能来自引擎回报的分组。
- **未能自动化（不当作通过）：** 真实窗口里 XTEST 注入的字符**进不了**输入框（能聚焦、字符不落；
  已排除「别的应用抢焦点」仍复现），故「GUI 打字 → 端点收到带上下文的请求」缺端到端证据，
  需**用户人工点一次**；逻辑部分由脚本级验证 + 既有 028 验收覆盖。
- **架构事实：** 宿主↔引擎**没有**「向既有会话再发一条」的动词（`submit_task` 每次 `session/new`；
  `recover_session` 只 `session/resume` 且不重放）→ 工作台连续对话 = 每轮一个新 DSH 会话 + 重放上文；
  要真正的同会话多轮须给适配层加动词，超出本计划边界，须另行批准。「AI 自主调 Skill」本轮未测。

## OD-ARCH-031 Part A：模型设置跳转 DSH（实施与复核，2026-10-20）

**结论：通过。** 设置页现在有两个**常驻**入口——「打开 DSH 设置（登记 key）」与「刷新端点信息」；
点前者**真的把与引擎同款的 DSH 拉起来**（`node <runtime>/dsh/lib/bin.js web`，浏览器自动打开本地地址），
且**指向引擎正在用的那份 DSH 数据根**——首版拉起的是用户**自己那份** DSH（用户实测指出：模型配置没导入过来），已修；
启动失败时明确报错；端点读不出来时同样可见可用；
三类权限开关与「默认全关」语义未动。

- **实现（最小改动）：** 宿主新增**一个**无参数命令 `task_open_dsh_settings` → `task_shell::open_dsh_settings()`：
  只执行固定的 `gtk-launch <app-id>`（回退 `xdg-open <desktop file>`），**不拼接 shell、不接受前端传入的
  命令/路径/URL、不读不写不打印任何凭证**；未检测到安装项即**明确报错**，不静默失败。
  前端只加一个按钮与常驻刷新入口。
- **实测：** 设置页按钮清单实读含「打开 DSH 设置（登记 key）」「刷新端点信息」；点击后 DSH 进程出现，且其环境为
  `DSH_HOME=~/OmniDesk/runtime/dsh-home`（= 引擎读的那份）；
  另起空 DSH 数据根的实例复核，端点不可识别的渲染分支按代码条件生效（`!facts || factsError`），
  两个入口同样可见。
- **计划偏离（需复核确认）：** 计划写「宿主不新增命令」，但 Part A 第 1 条要求能拉起 DSH Desktop 的设置界面，
  而 Tauri v2 前端没有可用的外部启动器（opener 插件未安装）——不做宿主命令就无法实现。
  故新增**这一个**最小命令（无参数、固定白名单、不碰凭证、不涉协议、不涉通信层）。
- **回归：** Rust **144/144**（新增 1 条断言「只在 desktop 文件真实存在时才定位得到」）、`tsc` 0、构建通过。
- **未验证：** 本机 DSH 在没有自定义 baseURL 时回落 DeepSeek 默认端点，**自然构造不出「端点不可识别」状态**，
  该分支只做了源码核对，无截图证据；真实 key 登记与随后的真实调用未验证（属用户操作）。

## 正式环境启动器 `scripts/omnidesk.sh`（新增能力，2026-10-20）

**结论：可用；开发启动器的硬边界未被削弱；真实端点的 key 仍缺，属用户自己的选择。**

- **为什么另建而不是加开关：** `scripts/dev-ui.sh` 的文件头与 `--real` 分支都明确写着
  「本启动器只支持隔离合成端点，不提供 `--real`」（OD-ARCH-004）。因此新增**独立**启动器
  `scripts/omnidesk.sh`，而不是给开发启动器开洞——两条路、两个数据域，边界更清楚。
- **边界核对：** 启动器**不设置**任何 `OMNIDESK_*` 隔离变量（宿主按默认解析到正式根）、
  **不注入**合成 provider 覆盖、**不写**凭证、**不开启**任何模型权限（默认全关）、
  **不碰** `runtime/`、**不提交** Git；停止前按命令行指纹核对进程身份，不误杀别的实例。
- **实测：** 启动/状态/停止三态正常；应用跑在原生 Wayland；使用记录落点为 `~/OmniDesk/app/logs/usage.jsonl`（正式根）。
- **一次需要澄清的观察（已查证，非缺陷）：** 正式根出现 `model_permissions` 三项全为 `true` 的设置文件。
  复现排除「启动自动写默认」：用一次性全新应用数据根启动后 `config/` 为空；代码里只有
  `set_model_permission` / `revoke_all_model_permissions` / `set_artifact_root` 三个写点，均由显式命令触发。
  → 该文件来自界面上的勾选操作。同期的两条真实任务**失败**且报错明确
  （`no API key for provider route "deepseek-official"`），**未写出任何正文**，符合「失败响亮、不静默降级」。
- **待用户处理：** 真实路由是 `api.deepseek.com`（类别 **非回环 · 公网**），凭据服务里只有 `OPENDESIGN_API_KEY`，
  缺该路由的 key；补 key 的两条正规路径（写进 DSH 凭据服务 / 启动时导出 `DEEPSEEK_API_KEY`）写在
  [docs/pilot-first-real-use.md](docs/pilot-first-real-use.md)。**本仓库不代取、不打印、不写入任何 key。**

## OD-ARCH-030：真实使用前置收口（ARCHITECT 执行与复核，2026-10-20）

**结论：四件事全部达成**——端点引导文案完整并修正了一处误导表述、假端点下「判定→开关→落盘→放行→模型返回」
链路在真实窗口打通、使用记录确实落到正式根 `~/OmniDesk/app/logs/usage.jsonl` 且验证数据已清空、遗留 0 字节
文件已删除；本轮唯一的产品代码改动是一处设置页文案，**未触碰路由、凭证、`runtime/` 与用户数据**。

- **① 端点引导：通过（含 1 处文案修正）。** 引导覆盖本机回环/非回环（私有·链路本地、公网）分类、默认全部关闭、
  无法确认位置永不放行、自行确认授权与数据去向、端点来自 DSH 侧配置、不读取凭证值等 8 项（源码核对 + 渲染截图）。
  原文「请先在**隔离环境**中把模型配置指向明确的地址」与事实不符（端点来自 DSH 侧模型配置，与隔离环境无关），
  已改写并补一段说明；**边界判定与路由代码未动**。
- **① 假端点验证：通过。** 权限关闭时提交被拒且**不产生任务**（界面给出真实原因，不静默降级）；
  界面勾选「本机模型」→ 设置文件写入 `local=true` → 准入状态传播 → 真实模型调用终态 `succeeded`。
- **② 使用记录持久化：通过。** 位置 `~/OmniDesk/app/logs/usage.jsonl`（正式根，仓库外，**产品代码无需改动**）；
  格式只含枚举与时间；界面可读回；**验证产生的合成记录已用产品自带删除路径清空（文件 0 字节）**，
  避免污染用户的首次真实使用数据。目录快照前后比对：`quarantine/`（真实案件材料）与 `runtime/`（含凭证）
  **未新增、未改动、未删除**；唯一新增的 `app/config/offline-flows.v1.json` 是应用首启写入的可编辑流程包副本
  （无凭证、无用户数据）。保留期限与清理规则已登记在数据布局文档 §5.1（明确「暂不实现自动清理」）。
- **③ 杂物清理：完成。** 仓库根 0 字节文件 `当前没有正在执行中的`（2026-10-16 遗留、无引用）已删除。
- **④ 提交推送：完成。** 沿用 OD-ARCH-029 的红线扫描流程（逐批扫描 + 人工核对文件清单），提交号见 STATUS.md；
  推送后远端与本地 HEAD 一致。
- **未验证 / 遗留：** 真实端点与真实模型未验证（全程回环假端点）；原生 Wayland 未覆盖（无合成输入通道）；
  T-001 ④（super-plan → super-worker 派工）因 OpenCode `experimental.subagent_depth` 默认 1 限制**未通过**，
  路径 A（以主 agent 使用 super-plan，零配置改动）/ B（新增 `experimental.subagent_depth: 2`，超出 T-001 已批准边界）
  **待用户决策**，本轮不擅自改配置。

## OD-ARCH-029：备份收口 + 双 Agent 复测 + 使用记录通道自检（ARCHITECT 执行与复核，2026-10-20）

**结论：备份收口**通过**——8 批提交、681 个文件、敏感信息扫描无真实命中、已推送到 `origin/main`，
远端与本地 HEAD 一致，提交前后逐项对齐无丢失；使用记录通道自检**通过**；T-001 三项复测在本会话内
**无法执行**（会话时序，非实现缺陷），已给出照做式清单。**

- **授权核对：通过。** 计划正文写明「批准即视同授权其中的 Git 提交与推送动作」，用户明确「批准 OD-ARCH-029」，
  故提交与推送在授权内。**没有做任何计划外动作**：未合并第二 worktree、未改业务代码、未改 `runtime/`、
  未碰模型路由/凭证/用户数据、未打标签。
- **提交前扫描：逐条人工核对的 28 条命中全部无害**（详见 STATUS 同名小节）：红线禁入词表本身
  （`task_shell.rs` / `aiDraft.ts`）、Rust 单测里的**合成**号码且断言不回显、`workspace_token_for(key)`
  这类**函数调用**、以及文档里「不读取 .credentials.yaml」的说明。**无真实凭证、无真实业务材料、无会话日志。**
  另核对：远端是**公开仓库**（`visibility: public`），既有 HEAD 中已含 `/home/kevin` 路径（历史既有事实），
  本次新增内容未引入新的个人信息面。
- **分批与推送：通过。** 8 批各自暂存→核对文件清单→扫描→提交；`git push origin main` 返回
  `5ffc710..3f84ca5 main -> main`；`git ls-remote origin main` = `3f84ca5…` = 本地 HEAD。
- **无丢失：通过。** 提交前工作区 682 项；提交后 681 个文件已进入提交 + 1 个剩余未跟踪项，
  两侧清单逐行对齐一致。剩余项是 2026-10-16 遗留的 **0 字节空文件** `当前没有正在执行中的`：
  **既未提交也未删除**（删除属计划外动作），已明确登记。
- **使用记录通道自检：通过。** 真实窗口 + 隔离根：记录 0→1；落盘文件在**仓库外**；
  记录**只含枚举与时间**（`record_id/schema/task_type/recorded_at/completed/usable/rework_kind/
  blocker/duration_bucket/reuse_willingness`），**无输入/输出/正文/标题/路径**；界面「查看记录（1）」可读回。
  一处需说明：本次点击把默认勾选的「结果可以直接用」翻成了 `usable=false`（枚举取值，非缺陷）。
- **T-001 复测：未执行（阻塞已查证）。** subagent 传 `agent: "super-plan"` 两次返回 `Unknown agent: super-plan`；
  官方文档确认 `.opencode/agents/<name>.md` 路径正确，问题是**两个文件创建于本会话的 agent 目录建立之后**。
  按计划风险条款「复测延后不阻塞备份」处理，步骤见 `docs/agent-toolchain-retest.md`，新开会话可跑完。
  **不得**把这一项记成「已验证」或「失败」。
- **遗留（不在本计划内）：** 那个 0 字节空文件的清理、真实使用体验数据、release 内嵌产物验证、
  真实模型端点启用与 UI 微调，均需另行单独批准。

## OD-ARCH-030：ARCHITECT 独立复核（2026-10-20）

**结论：通过。** 基于 git log 实测、提交 diff 逐项核对、证据 JSON 与正式数据根现场核验，非采信执行方自述。

- **端点设置引导修正属实且正确：** b5fb000 的 SettingsPanel.tsx 改动仅 8 行文案——「在隔离环境中改端点」的错误说法改为「改 DSH 侧模型配置后回本页刷新」，并补明应用只读非机密 provider/model/地址、不保存端点不读凭证。不改判定与路由代码，改动范围与计划一致。证据 od030-run5.json 显示 8 项引导要点与三类权限开关（默认全关、关闭时输入框明确提示且不产生任务、开启后配置落盘并放行到模型返回）全部实测通过。
- **使用记录持久化属实：** 现场核验 ~/OmniDesk/app/logs/usage.jsonl 存在且为 0 字节（测试记录已用产品自带删除清空），首次真实使用不会混入测试数据。数据布局 §5.1 已登记保留与清理规则。quarantine/ 与 runtime/ 目录未被动过（目录清单比对一致）。
- **遗留杂物已删：** 0 字节文件「当前没有正在执行中的」已按授权删除。
- **提交推送属实：** 3 批提交（b5fb000/2a9ddf5/8645413）+ 本轮登记（581c964），HEAD 与 origin/main 一致；历史全量文件清单再次核查无凭证类文件。工作区剩 5 个登记文件差异 + 1 个未跟踪 .opencode/opencode.json。
- **T-001 双 agent 复测全部闭环（超计划收益，如实认可）：** ① git rev-parse 放行通过（且工作区外路径防线被意外证实生效）；② 管道命令被拒、语义明确（白名单按单条命令匹配，安全上合理）；③ git diff --check 通过；④ 派工链路在用户批准 B 方案（项目级 opencode.json 设 subagent_depth: 2）后端到端调通，super-worker 自报模型与 ARCHITECT 独立核实的 HEAD 一致。「换会话才能发现 agent」的旧疑点确认非必要条件。
- **如实保留的限制：** 真实服务商端点与真实模型质量未验证（本轮全程合成端点）——这正是把首次真实使用交给用户的原因；原生 Wayland 未覆盖（合成输入通道限制，非缺陷）。

**验收放行：** 首次真实使用的前置条件全部满足。用户按 docs/pilot-first-real-use.md 执行，体验数据与改动要求反馈后进入下一轮需求。

## OD-ARCH-029：ARCHITECT 独立复核（2026-10-20）

**结论：通过（备份收口 + 使用记录通道；T-001 复测保持挂起，阻塞定性成立）。** 基于 git log 实测、推送后远端状态、历史全量敏感路径核查与本轮证据文件独立验证，非采信执行方自述。

- **备份收口属实且完整：** 8 批提交（2705148→653bf4a）覆盖前端、宿主、引擎、样例、脚本、agent 配置、文档、证据（共 605 文件 / 28134 行证据批次）；本地 HEAD 653bf4a 与 origin/main 一致；工作区由 682 项降至 6 项（5 个状态/文档文件的本轮登记差异 + 1 个遗留 0 字节文件）。全部历史提交文件清单核查无凭证/密钥/env/私钥类文件入库。备份目标达成，单点丢失风险解除。
- **敏感扫描判断认可：** 命中 28 条的人工核对结论（禁入词表本身、合成测试号码、函数调用名、否定式文档表述）经独立抽查无误；合成号码仅存在于断言不回显的单测中，不构成泄露。
- **遗留 0 字节文件「当前没有正在执行中的」：** 确认无内容无引用，属 2026-10-16 遗留杂物。删除虽超本计划范围，但作为零风险清理项，建议并入下一任务一并处理（用户批准 OD-ARCH-030 即视为授权删除该文件）。
- **使用记录通道自检属实：** 证据文件 od029-usage-run1.json 与前后截图齐全；记录仅含枚举与时间字段、无正文落盘；仓库外数据根（/tmp/od029-usage）未污染 ~/OmniDesk。可接受。
- **T-001 复测挂起的阻塞定性成立：** 99 次重试 + 同窗口 general agent 成功的对照，足以把阻塞归因于两个免费模型额度而非配置缺陷；「换会话才能发现 agent」的旧结论已被推翻（同会话曾成功加载），REVIEW.md 旧记录已由执行方如实修订，无掩盖。复测继续挂起，待额度恢复，不阻塞后续计划。
- **工作区剩余差异：** PLANS/REVIEW/STATUS/decisions/agent-toolchain-retest 5 个文件为 T-001 复测重试记录与状态校准的登记差异，事实核对无误，应随下一任务提交入库。

### 下一计划 OD-ARCH-030（已写入 PLANS.md，待用户批准）

用户真实使用前置收口：真实端点配置引导与验证 + 使用记录持久化通道 + 遗留杂物清理 + 上述文档登记提交。详见 PLANS.md 顶部。

## T-001：固定双 Agent「超级Plan / 超级劳工」配置（2026-10-19）

**结论（2026-10-20 晚最终版）：①②③④ 全部闭环——部署可用、静态校验通过、三项复测通过、④ 派工链路经用户批准 `experimental.subagent_depth: 2` 后端到端调通。** 本次由 ARCHITECT 自行实施并自查（非 BUILDER 交付后独立复核），证据等级为真实运行调用 + 静态解析，不含推断结论。**入库事实：两个 agent 文件在等待期间被外部提交（`593c49f`）并随 OD-ARCH-029 批次推送 `origin/main`；本会话未执行任何 commit/push。**

- **文件事实：** `.opencode/agents/super-plan.md`、`.opencode/agents/super-worker.md` 两个新文件，格式符合 OpenCode V2 项目级 agent（frontmatter + 正文 system prompt）。模型绑定按用户逐项指定：super-plan = `opencode/glm-5.3-flash-free`、super-worker = `opencode/deepseek-v4.1-flash-free`；两个模型引用均经 models 工具核实为 `opencode` provider 下的 active 条目。未创建 `opencode.json`、未设 `default_agent`、未改全局模型路由与凭证。
- **权限矩阵核对（配置事实）：** super-plan 为「默认 `*/*` deny + 逐条放行」结构，放行项限于 read/grep/glob、仅 `PLANS.md` 的 edit、只读 git 命令、仅 `super-worker` 的 subagent，并对 `.env*`/`*credentials*`/`*secret*`/`*.pem`/`*.key` 追加 read deny。super-worker 未加额外限制，权限等同 build 默认（敏感环境文件与工作区外访问仍会弹确认）——这是**有意选择**：执行 agent 需要编辑与运行测试，把边界约束放在 system prompt 的红线条款（范围刚性、禁改 runtime/、禁读凭证、禁触真实业务材料、完成即停）而非权限层。
- **已验证（真实调用证据）：** ① 两个 agent 均可被 subagent 工具发现并加载；② super-plan 的 read/glob/grep 正常，单条 `git status --porcelain` 在仓库根执行成功；③ 未放行命令（`git rev-parse HEAD` 首测、`find docs …`）被**正确拒绝**，说明 deny 链生效；④ super-plan 未越界写入任何文件（PLANS.md 保持未动）；⑤ super-worker 可加载并自述模型为 DeepSeek V4.1 Flash Free。
- **2026-10-20 复测重试与阻塞定性（真实调用证据）：** 按用户指示以 5 分钟间隔连续重试 **99 次**（约 8.3 小时），super-plan 与 super-worker 交替探测，**99/99 返回 `Rate limit exceeded. Please try again later.`**；**同期内置 `general` agent（继承本会话模型）调用成功**。据此把阻塞定性为**两个绑定模型的免费额度**——**不是** OpenCode 子 agent 通道故障、**不是**本项目配置缺陷、**不是** agent 目录路径错误。同时，本会话内两个 agent **确实被成功发现并加载**（见上「已验证」①⑤），与另一会话记录的 `Unknown agent: super-plan` 现象**不一致，原因未查证**；在查清前，「换新会话才能发现」**不得当作已验证的解法**。
- **未验证/遗留（不得当作已通过）：** ① 首测暴露的白名单缺口（缺 `git rev-parse*`）已修，**修后复测因模型额度阻塞仍未完成**；② 含管道/复合命令（如 `git status --porcelain | wc -l`）的匹配语义未确证——首轮该次调用被拒，可能因命令合并形态而非白名单本身，**不能据此断定单条命令以外的形态都不可用，也不能断定可用**；③ super-plan→super-worker 的实际派工链路尚未在真实获批计划中端到端试跑；④ agent 的模型绑定属配置事实，运行时实际模型由 OpenCode 解析，本次未取得逐会话模型凭据。
- **静态校验（部署后自检，证据等级：文件解析）：** 两文件 frontmatter 以 YAML 解析通过，键与取值正确；super-plan 16 条权限规则顺序为「宽 deny 在首、窄 allow 在后」，与 OpenCode「后匹配者优先」语义一致；super-worker 无 permissions 段（= build 默认）。
- **风险：** 权限规则是**配置级**防线，不等同代码级安全控制；super-worker 的无限制编辑能力配合 prompt 红线是当前设计取舍，若日后发现越界，应先加权限白名单而非仅依赖 prompt。中文名保留在 description 与正文，英文文件名规避了非 ASCII agent ID 的匹配风险。**部署层面的残余风险：两个 agent 目前绑定在额度紧张的免费模型上，额度耗尽时它们不可用**——这属于可用性风险，不是权限风险。
- **2026-10-20 晚复测结果（限期解除后，真实调用证据）：** ① **通过**——`git rev-parse HEAD` 单条成功输出 HEAD `653bf4a`（首次因 agent workdir 笔误被 `external_directory` 拒，恰证外路径防线生效，补跑通过）；② **结论明确：管道被拒**——`git status --porcelain | wc -l` 返回 `Permission denied: shell`，对比单条 `git status --porcelain` 成功 → `git status*` 仅按**单条命令**前缀匹配，管道整体不在白名单（安全上合理；超级Plan 快照命令须逐条运行）；③ **通过**——`git diff --check HEAD` 成功且输出为空；④ **未通过（原因已查证）**——super-plan 作为子 agent 时嵌套派工被深度限制拦截（`Subagent depth limit reached (1)`；官方 schema 核实该键为 `experimental.subagent_depth`，默认 1）。同时直连验证 super-worker 可用并自述模型 `deepseek-v4.1-flash-free`。
- **④ 的解决（用户批准 B 方案）：** 新增项目级 `.opencode/opencode.json`（内容仅 `$schema` + `experimental.subagent_depth: 2`），配置写入后**同一会话即时生效**；端到端验证：ARCHITECT → super-plan（深度1，GLM-5.3-Flash Free）→ super-worker（深度2，DeepSeek V4.1 Flash Free）**派工成功**，super-worker 只读执行 `git rev-parse HEAD` 并回报（当时 HEAD `581c964`，与 ARCHITECT 本会话独立核实一致）。历史事实：B 之前 ④ 曾被默认深度 1 拦截（`Subagent depth limit reached (1)`）。
- **B 方案授权记录：** 用户在 2026-10-20 晚的两选一询问中选择「B. 改配置放开嵌套」，构成对本配置变更的明确批准；A 方案（主 agent 用法/直调 super-worker）仍为等效可用路径，未配置时可作为回退。
- **T-001 综合结论（2026-10-20 晚）：** ①②③④ 全部闭环。部署（两个 agent + 权限矩阵 + 固定模型）可用且已入库；权限白名单按单条命令前缀匹配（管道被拒，安全合理）；嵌套派工经用户批准的 `experimental.subagent_depth: 2` 放开并端到端验证。模型绑定仍是配置事实（两模型自报与其一致），实际生效由 OpenCode 解析。残余风险：两个绑定模型的免费额度紧张，额度耗尽时双 agent 不可用；`experimental.subagent_depth` 为实验性设置，OpenCode 升级可能改变语义（复审触发条件）。

## OD-ARCH-028：阶段二试点（前置复核 + 集中验收，2026-10-19）

**结论：前置门通过；三个场景的「材料 → 模型起草 → 人工编辑 → 确认保存 → 读回」闭环在真实窗口成立；
失败与取消不产生任何成果文件；必要回归全绿；既有工作区差异完整保留。**
本轮由 ARCHITECT 直接执行（计划要求集中实施、集中验收）；未提交、未推送、未打标签。

### 一、前置门：OD-ARCH-026 独立复核（通过）

执行方留下的四个待核对点，逐条结论：

1. **「成果只能来自模型初稿」——通过。** `OfflineFlows.tsx` 的保存路径只取 `draft`（唯一来源是
   `aiDraft.ts` 的 `draftFromTerminal`，只在终态 `succeeded` 且正文非空时回填）；确定性核对结果存在
   独立的 `check` 状态，**不进入** `saveOffline` 的 markdown；宿主 `save_offline_artifact` 还要求
   `user_confirmed = true` 且正文非空，落盘文件名带时间戳、**不覆盖**已有文件，并在仓库外数据目录。
   真实窗口证据：三份成果的正文都等于「模型初稿 + 人工追加」，并带「材料来源（由 OmniDesk 记录，
   非模型输出）」块。
2. **材料读取边界——通过，但有一条要写明的限制。** 宿主 `task_shell::read_material` 强制：绝对路径、
   普通文件、扩展名白名单（md/txt/csv/tsv/json/yaml/log 等）、≤512 KiB、合法 UTF-8、禁止片段
   （DSH/agent/仓库）与仓库外落点；**不列目录、不递归、不解析数据库**。材料正文在提交时也进入宿主
   红线扫描（案件类关键词、疑似身份证/手机号命中即拒绝，不回显原文）。
   **限制：** 白名单按扩展名而非内容判定，宿主**不检测**被选文件里是否含密钥/凭证；这一层依赖
   「用户显式选择 + 界面提示 + 红线关键词」的组合，**不是代码级凭证防护**。已作为遗留风险记录。
3. **流程包新增可选 `draft.rules/sections`——通过。** 旧包缺该字段仍能加载（Rust 用例
   `old_pack_without_draft_still_loads_and_draft_links_are_rejected` 覆盖），缺省回落内置默认规则；
   规则文本为空、外链等内容仍被既有纯文本校验拒绝。与 OD-ARCH-019「缺项被拒绝」不冲突：那条针对
   **必填**字段，`draft` 是显式可选且默认回落的扩展。
4. **「GUI 只走完周报」——本轮已补齐。** 见下。

### 二、集中验收（真实窗口，`evidence/od-arch-028/od028-run1.json` + 29 张截图）

- **首页基线未回退：** 首页截图与用户 2026-10-19 确认的版式一致（机器人 + 今日重点推进 + 日程节律 +
  全宽对话，frameless 外框），无右栏、无假数据、无持续动画。
- **三个模块都能从左侧进入：** Dock 的「周报与随记 / 文案写作 / 项目跟踪」分别进入对应场景并载入
  对应初始页签；三场景均跑通「样例 → 材料 → 起草 → 人工编辑 → 勾选确认 → 保存 → 读回」。
- **材料真的进了模型请求：** 每个场景在合成端点埋了唯一标记探针，三次请求**全部命中**
  （`OD028-WEEKLY/WRITING/PROJECT-MARKER`），不是「界面显示已读」而已。
- **失败与取消不伪装成功：** 端点 400 → 终态 `failed`（原样显示上游错误），端点挂起后点「取消生成」
  → 终态 `cancelled`；两种情况下**成果目录没有新增任何文件**（`new_artifacts=[]`），界面也不出现模型正文。
- **结果标注未复核：** 成果文件头写明「模型来源未复核，已由人工编辑核对」，材料来源块单独标注
  「由 OmniDesk 记录，非模型输出」。
- **回归：** Rust **143/143**、`tsc` 0、构建通过、`verify_ai_draft` 27/0、`verify_scenario_materials`
  5 场景 0 failed、桌面任务链/工作台引擎路径/取消语义/会话连续性全部 pass；`git diff --check HEAD`
  干净；`runtime/` 无差异；未提交、未推送、未打标签。

### 三、本轮唯一的代码改动与理由

`desktop/src-tauri/src/task_shell.rs`：`read_material` 拒绝「仓库内文件」时沿用了产物写入路径的报错
文案（"拒绝把产物写入仓库内"），与「读取材料」的动作不符。已改为「拒绝读取 …：该文件位于工作区
仓库内，不能作为模型材料。」并补 1 条单测锁定文案语义。属计划第 (3) 步「仅修复可观察到的不一致」，
不改变任何边界判定。

### 四、遗留风险与未验证项（不因本轮通过而消失）

1. **真实模型质量未验证**：全部为回环合成端点（`合成回复内容`）。
2. **材料内容里的凭证不设防**（见上文第 2 点）：白名单按扩展名，不按内容。
3. **平台**：仅 Xvfb + 1280×820；真实 Wayland、release 内嵌产物、窗口缩放、输入法与粘贴未验证。
4. **读回对象**：写作/项目场景保存时列表已有多件，宿主不保证顺序，「重新打开」指向哪一件未逐件断言；
   周报场景因当时列表只有一件，可断言读回的就是刚保存的那一件。
5. **真实使用体验指标未采集**：本轮是自动化技术验收，人工澄清数/返工轮数记 0，**不能**当作真实体验；
   Token 引擎未上报 usage，按口径记「未知」，不估算。
6. **OD-ARCH-027 Step 5（真实 Wayland 桌面回归）仍未执行**，需用户单独批准并在其本机会话进行。

## 窗口外框一体化（frameless）（实施、冒烟与用户验收，2026-10-19）

**结论：已实现，构建与 Xvfb 冒烟通过，并经用户在本机完整重启后现场验收；确认为 UI 基线。** 本次由 ARCHITECT 直接实施（用户明确要求的小型外壳改动），未提交、未推送、未打标签。

- **改动：** `src-tauri/tauri.conf.json` 主窗口 `decorations: false`；`src-tauri/capabilities/default.json` 增加最小窗口权限（minimize / toggle-maximize / close / is-maximized / start-dragging / start-resize-dragging / internal-toggle-maximize）；新增 `src/shell/WindowChrome.tsx`（自绘 — □ × + 8 个边缘/角落缩放热区）；`TopNav.tsx` 顶栏加 `data-tauri-drag-region` 并嵌入控件；`App.tsx` 挂载缩放热区。
- **验证：** `npx tsc --noEmit` 0；`npm run build` 通过；`cargo build --offline` 通过（1m10s，含配置/权限变更）。一次性 Xvfb `:99` + 隔离数据根冒烟截图：窗口正常打开、顶栏右侧三个控件渲染、首页完整、无黑屏。
- **用户验收（2026-10-19）：** 完整重启后确认系统标题栏消失，顶栏拖动、最小化、最大化/还原、关闭均正常。**已确认为 UI 基线**（GOALS.md、docs/decisions.md D-020）。
- **未显式验证：** 顶栏双击最大化、四边/四角缩放热区、最大化状态图标切换未被用户逐项点名；Xvfb 无窗口管理器，不能作为系统装饰消失的证据。后续如发现问题按同一回退路径处理。
- **风险与回退：** 无边框在不同 WM/Wayland 下存在平台差异；若窗口无法移动或缩放，把 `decorations` 改回 `true`、移除顶栏控件与缩放热区后重建即可。配置已编译进新二进制，**仅前端 HMR 不生效，必须重启应用**。
- **工作区：** 条目 108 → **111**（`tauri.conf.json`、`capabilities/default.json` 修改 + 新增 `WindowChrome.tsx`）；`git diff --check HEAD` 干净；`runtime/` 无差异。

## OD-ARCH-026 首页版式回归：对话为主 + 顶部三模块（ARCHITECT 复核，2026-10-19）

**结论：按用户批准的三个口径通过。** 本次仍属 OD-ARCH-026（用户明确「继续 026」，未新开编号）。未提交、未推送、未打标签。

- **版式核对：通过。** 右栏整列（三场景卡、当前版本边界、查看引擎状态、「OMNIDESK · LOCAL FIRST」）与蓝框内容（「对话中枢」容器头、说明文字、三个快捷入口）已从源码和构建产物中消失；首页改为「问候 → 机器人 + 今日重点推进 + 日程节律 → 下半屏整幅对话」。我查看看了真实窗口截图：`home-layout-run1-01-home-empty.png` 与 `-03-home-after-send.png`，顶部三模块、全宽对话、消息区顶部向上渐隐均成立；机器人用本树 `mascot.png`，从卡片左侧探出，符合设计意图。
- **三项口径：通过。** ① 默认静态：`HomeCanvas.tsx` / `HomeChat.tsx` 无 `animate-float-mascot` / `animate-pulse` / 卡片 `backdrop-blur-md`，构建 CSS 仅保留 TopNav 的 `.backdrop-blur-2xl`；② 不伪造：问候后只写「· 欢迎回到 OmniDesk」，两卡标注「未接入数据源」「未同步」，正文无业务数字；③ 权限一行：输入框底部单行状态，未开启时带「去设置」，细节收进 `title`。
- **改动边界：通过。** 新增 `HomeCanvas.tsx`、`App.tsx` 首页接线、删除 `HomeDashboard.tsx`（已确认仅 `App.tsx` 引用过）、`HomeChat.tsx` 只改展示；提交/轮询/取消/准入逻辑未动；未改 `Dock.tsx`、`TopNav.tsx`、Rust、`runtime/`、`scripts/dev-ui.sh` 与 027 的证据。
- **独立检查：** 我重跑 `npx tsc --noEmit`（0）与 `npm run build`（通过），并在 `dist` 中确认右栏文案与上述动画/模糊类均已消失；`git status --porcelain` 仍 **108** 项、`git diff --check HEAD` 干净、`runtime/` 无差异、无 `.rs` 改动。**未独立重跑**窗口与 60 s 空闲测量，这两项采用执行方落盘证据（5 帧签名=1、`WebKitWebProcess` 0.08%）；结合「首页已无任何无限动画」的静态事实，与 027 Step 4 的 <10% 目标一致。
- **未验证/边界：** 回复来自回环合成端点，不代表真实模型质量；仅 Xvfb + 1280×820，真实 Wayland、release 内嵌产物、窗口缩放、输入法/粘贴与长对话滚动未验证；未做设计稿像素级比对（本树 `mascot.png` 与设计稿比例略有差异）。
- **对 026 复核的影响：** 首页已由两栏仪表盘式布局改为对话为主 + 顶部三模块，**后续 OD-ARCH-026 功能复核（用户安排由 GPT 执行）应以本版首页为准**。
- **同日微调（用户反馈后）：** 用户要求「模型权限」不与输入框合在一起，并在其后跟一个 token 消耗统计、同一行显示两个模块。现为：输入框（`plate-sculpted` 指令坞）内部只保留输入与挂载库/语音/发送；其**下方独立一行**左侧是「模型权限：已开启 · 本机 / 未开启，提交已关闭 + 去设置」，右侧是「Token 上下文：used / size」。Token 数值只来自引擎通过 `progress` 事件上报的 `{kind:"usage", used, size}`（宿主已把全部任务事件转发到前端），**不自行估算或累加**；未上报时显示「—（引擎未上报）」。顶部「LOCAL IPC ACTIVE」仍只表示**引擎 IPC 健康**（`host.describe().healthy`），与权限/用量三者含义不同。`tsc`/build 复跑通过。
- **同日布局修正（固定页头/页脚，仅对话区滚动）：** 用户指出整页会滚动、顶栏与顶部卡片被带走、右侧出现深色滚动条。已把 `App.tsx` 根从 `min-h-screen` 改为 `h-screen`；`HomeChat` 根补 `flex-1 min-h-0`，消息区由 `min-h-[240px]` 改为 `min-h-0`；顶部问候/三模块与底部输入框/状态行均为 `flex-shrink-0`。现在**只有对话历史区滚动**，顶栏、今日提醒、日程节律、输入框与状态行固定。滚动条改为浅色（`rgba(148,163,184,.55)`，`scrollbar-width: thin`），并在 `:root` 显式 `color-scheme: light`，避免 WebKitGTK 采用深色原生滚动条。`tsc`/build 复跑通过；用户已于 2026-10-19 在本机确认该版式与固定页头/页脚、仅对话区滚动的效果，并连同 frameless 外框一起定为**UI 基线**（GOALS.md「已确认的 UI 基线」、docs/decisions.md D-020）。

## T-001 复测的会话限制与 028 试点的交接（ARCHITECT 核查，2026-10-20）

**结论：T-001 的三项运行复测在本会话内确实无法执行（已查证原因，不是漏做）；OD-ARCH-028 的「真实使用」
必须由用户本人完成，已备好照做式提示。两项都写成了可执行清单，不再挂在待办里。**

- **T-001 复测为什么做不了（事实，不是推测）：** 实际调用 subagent 工具传 `agent: "super-plan"` 返回
  `Unknown agent: super-plan`；另两个 agent（explore / general）正常可用，说明**是本会话的 agent 目录**
  没有这两个条目。核对 OpenCode V2 官方文档：项目级 agent 放 `.opencode/agents/<name>.md`，
  **与本次实现的路径完全一致**（不是写错目录）；两个文件创建于 2026-10-19 22:47/23:03，
  而**本会话的 agent 目录早于文件创建时间**，因此未被收录。
  → 结论：**不是 T-001 的实现缺陷，是会话时序**。复测步骤已写成 [docs/agent-toolchain-retest.md](docs/agent-toolchain-retest.md)
  （三段可直接粘贴的提示词 + 每项的两种可能结果与记录口径）；**新开会话即可发现这两个 agent**。
- **028 试点的交接：** 自动化验收（见上文 OD-ARCH-028 小节）只证明**链路通**，**不产生体验数据**；
  澄清数/返工轮数/耗时必须来自用户的真实使用，Token 引擎未上报 usage 一律记「未知」。
  → 已写成 [docs/pilot-first-real-use.md](docs/pilot-first-real-use.md)：选材边界、读材料限制、
  五步闭环、确定性核对的作用、以及用「使用记录」回填试点评价的字段对应关系。
- **未做与不做的：** 没有为了「跑通」而修改 agent 权限、没有为了测试而放宽任何边界、没有替用户
  填写体验数据，也没有把自动化结果当作用户反馈。

## OD-ARCH-027 Step 5：真实桌面回归（ARCHITECT 执行与复核，2026-10-20）

**结论：按计划 §5 验收通过——四组真实桌面实测的空闲渲染进程 CPU 均值为 0.08%–0.28%（目标 <10%），
导航延迟 P50 0.31–1.22 s、P95 ≤1.63 s 且 0 超时，5 帧签名 = 1（无持续重绘），设置页连续输入逐字落盘。
但必须同时写明：原始「空闲约 100%」症状在真实桌面同样没有复现，本轮**不构成性能提升**。**
本轮由 ARCHITECT 在用户本机会话内执行（计划允许真实桌面复测在用户会话中进行）；未改产品代码，
未提交、未推送、未打标签。

- **范围核对：通过。** 只新增测量装置与证据：`evidence/od-arch-027/step5/scripts/step5-real-desktop.py`
  （只读复用 `window_harness` / `run_window_acceptance` / `_step3lib`，未改共享装置）、`step5/step5-*.json`、
  `step5/shots-*/`、证据索引 §9。`desktop/src/**`、`desktop/src-tauri/**`、`runtime/`、`scripts/dev-ui.sh` 均未改。
- **环境一致性（§5 硬要求）：通过。** 四组**分别记录**：原生 Wayland（`GDK_BACKEND=wayland`、清空 `DISPLAY`）×
  DMABUF{1,0}，XWayland（`GDK_BACKEND=x11` + `DISPLAY=:0`）× DMABUF{1,0}；每组记录构建方式（debug + vite dev）、
  数据根（`/tmp/od027-step5/...`）、显示后端、DMABUF 取值、窗口尺寸（1280×857）与命令脚本路径。
  Xvfb 与真实桌面**不混为一组**。
- **空闲 CPU：通过。** 原生 Wayland 0.08% / 0.17%，XWayland 0.15% / 0.28%（60 s 均值，`/proc` utime+stime 口径）。
  原生 Wayland 的 60 s 曲线在 0.00–0.09% 之间，无尖峰；四组均无 GPU 进程。
- **导航延迟：通过（并修正了测量方法）。** 首版脚本按「同一场景重复点击」测，结果出现大量**假超时**
  （同一页面重复点击本就不该变化）。已改为**按固定循环路径**（任务工作台 → 项目跟踪 → 成果目录与模型权限 → 首页）
  重复，每一跳都是一次真实切换：四组各 20 次，**0 超时**，P50 0.31–1.22 s、P95 ≤1.63 s。
- **连续输入：通过。** 设置页「成果目录」输入框注入固定串（一个合法绝对路径）后点「校验并保存」，
  设置文件**逐字包含**该串（`exact_hit`）。补充一条**产品行为事实**：用非路径字符串保存会被**正确拒绝**
  （必须是绝对路径），这属于产品校验，不是丢字——第一次跑出的 `no_match` 由此得到解释，已改用合法路径复测。
- **视觉：通过（XWayland 组）。** 四页截图为真实桌面像素（均值亮度 244.9–247.4、非黑占比 1.00），
  首页与用户 2026-10-19 确认的 UI 基线一致（frameless 外框含 — □ ×、机器人、今日重点推进、日程节律、
  全宽对话、底部权限/Token 行）。5 帧签名 = 1。
- **通道限制（如实记录，未包装成「已测」）：** 原生 Wayland 下 **GNOME Shell Screenshot D-Bus 被拒**
  （`AccessDenied: ScreenshotArea is not allowed`）、`org.gnome.Shell.Eval` 返回 `(false,'')`、
  GDK 抓 X 根窗口为全黑；因此原生 Wayland 组**没有像素证据**、**没有合成输入通道**，交互改走 **AT-SPI**
  （可访问性总线，可读完整 UI 树、可用动作点击），页面切换用**可访问性文本指纹**判定。
  对应的像素与输入指标由 XWayland 组单独提供，**两组不混算**。
- **必须写明的限定：** ① 原始 ~100% 症状在真实桌面**同样不复现**，因此本轮**不能**宣称性能提升，只能说
  「当前工作区在真实桌面处于 0.08%–0.28% 的健康区间」；② DMABUF 两种取值量级接近，**不能**据此判定该变量
  在真实桌面重要或不重要（本机无 GPU，Step 3 的「不可控」结论未被推翻）；③ 仍是 **debug + vite dev**，
  release 内嵌产物未测；④ 真实模型质量、多显示器/缩放/HiDPI、>60 s 长稳、真实用户输入法与粘贴、GNOME
  「减少动画」开关的真实映射未验证。
- **本轮未动 UI 基线：** 确认 UI 基线（GOALS.md）保持不变；Step 5 只是测量。

## OD-ARCH-027 Step 3 / Step 4：DMABUF 对照与动画/重渲染治理（ARCHITECT 独立复核，2026-10-19）

**结论：Step 3 按范围通过（零产品代码改动，但结论是「该变量在本沙箱不可控」的诚实空结果）；Step 4 按范围通过，并复现并量化了原「~100%」的机制，同时定位到当前产品最大的一处持续占用（顶栏状态点脉冲 × 保留的顶栏模糊）；最终改动后引擎健康场景回落到 0.10%——满足计划 §5 的 <10% 目标。** 两轮均未提交、未推送、未打标签。

### Step 3（DMABUF 与显示后端独立对照）
- **范围核对：通过。** 零产品代码改动：`desktop/src/**`、`desktop/src-tauri/**`、`scripts/dev-ui.sh` 均未改；`tests/verification/desktop/window_harness.py` mtime 仍为 09:58（早于 Step 3），确认未改共享装置（`AppSession.env_extra` 是既有机制）；只新增 `evidence/od-arch-027/step3/**` 与三份文档。
- **证据核对：通过。** `step3-A-*` 页面级 12 次 + `step3-B-*` 应用级 6 次均有 JSON/PNG；`step3-frame-analysis.json` 覆盖 82 帧、0 张疑似黑屏/空白；每份 JSON 含环境快照与 `/proc/<pid>/environ` 的变量确认。
- **独立复跑：通过。** 我自起 Xvfb 直测 `heavy.html`：`WEBKIT_DISABLE_DMABUF_RENDERER=1` → **100.2%**，`=0` → **100.2%**，与执行方结论一致——**该变量在本机（无 DRI3 的 Xvfb）确实不改变代码路径**。
- **判断：** 执行方没有把「无差异」外推成「DMABUF 在真实桌面不重要」，也明确记录了 `WEBKIT_FORCE_DMABUF_RENDERER=1` 的 62% 不能证明更流畅、Wayland↔XWayland 对照留待 Step 5——口径正确。**本步不产生可用于真实桌面的结论，属如实空结果，不是执行失败。**
- **缺陷：** 无阻断项；仅需注意 82 帧判据是亮度/暗像素启发式，能排除整屏黑屏，不能排除局部撕裂或闪烁。

### Step 4（无限动画与 React 重渲染治理）
- **Phase A 单因素对照（我独立复跑，与执行方同量级）：** `iso-blur-only`（静态模糊/玻璃、无动画）**0.0%**；`iso-anim-only`（只有动画、无模糊）**29.7%**；`iso-navpulse`（静态模糊 + **唯一一个 6×6 脉冲点**）**100.15%**。执行方 3×30 s 的对应值为 0.00% / 26.00% / 100.19%。**机制成立且与动画元素大小无关：成本 = 持续重绘 × 大面积模糊/`backdrop-filter`；单有静态模糊不贵，单有动画约 1/4 核，两者叠加吃满一核。** 这与原「~100%」的量级一致；原始页面已不可取回，故「当时那一次的具体组合」仍是推断，但机制已被受控实验证实。
- **代码核对：通过。** `theme.css` 新增 `@keyframes status-dot-pulse` 与 `.status-dot-pulse { animation: none }`（注释保留一行回退写法），并保留 `@media (prefers-reduced-motion: reduce)` 守卫；`TopNav.tsx` 抽出 `NavClock` 自持 30 s tick，状态点仍按 `engineHealthy` 着色、位置/尺寸/布局/文案未变；`SettingsPanel.tsx` 未改动（mtime 10-18 20:12）。构建产物确认 `.status-dot-pulse{animation:none}` 且剩余 `infinite` 仅两个未使用的类。
- **应用级独立复跑：通过。** 我用执行方脚本在 `/tmp` 重跑「引擎健康 + 默认静态」：`WebKitWebProcess` **0.2%**、App 进程 **0.1%**、状态点 emerald、5 帧签名 1/1（无持续重绘）。执行方 60 s 结果 0.10%/0.12%，出厂默认 0.03%/0.10%。**满足 §5 的 <10% 目标。**
- **视觉：通过。** 放大截图确认状态点为实心 emerald 圆点 + 「LOCAL…」标签，仍表达「引擎健康」；脉冲消失但信息未丢。这是**有意的视觉改动**，一行改回即可恢复脉冲（代价 ~18.8%）。
- **诚实项：** `SettingsPanel` 的「输入丢字」**未复现**（fast/slow 注入均逐字命中），执行方选择不改并如实记录——正确，没有为不可复现的 bug 编造改动。
- **未完全解释项：** 脉冲时 **App 主进程也约 10%**（静态后 0.12%），可能把 GTK/合成器侧工作计入该进程；不影响「静态后回落」的结论，但该机理未单独证明。
- **方法学限制（重要）：** 导航延迟用「整窗截图轮询」测量，单次读回约 2.5 s，读数已饱和——**本支线至今没有亚秒级的真实导航耗时数据**，Step 2/3/4 的 P50/P95 差异都不能读成性能变化。Step 5 若要给交互延迟下结论，应改用页面内计时（`performance.now()`/事件打点），而不是截图差分。

### 复核边界与下一步
- **未验证：** 真实 Wayland/GPU 桌面、release+内嵌产物、真实输入法、真实系统「减少动态效果」开关、>60 s 长期稳定性、DMABUF 在真实桌面的行为与黑屏风险、Wayland↔XWayland 对照。
- **工作区：** HEAD `5ffc710`，`git status --porcelain` 仍 **108** 项（全部既有差异保留）；`git diff --check HEAD` 干净；`runtime/` 无差异；`*.rs` 无本轮改动；未提交、未推送、未打标签。
- **Step 5** 需用户批准，且按计划 §7 只能在用户真实桌面会话中进行（不由助手代启其桌面应用）；届时应同时替换延迟测量方法，并把 Xvfb 与真实桌面结果分开记录。
- **OD-ARCH-026 的功能复核仍未进行**（按用户安排交由 GPT 执行），不与本支线混同。

## OD-ARCH-027 Step 2：CSS 性能优化（ARCHITECT 独立复核，2026-10-19）

**结论：Step 2 按获批范围通过（执行质量与证据可信），但附带一条必须让用户知晓的关键限定——本步没有可宣称的应用级性能收益，因为计划 §1.2 的原始症状（空闲 `WebKitWebProcess` 约 100%）在当前工作区未能复现。** 这不是执行失败，而是「基线本身已低」的事实；结论不结案、不授权提交/推送。

- **范围核对：通过。** 只改了 6 个桌面源码文件，全部是样式与既有类：`App.tsx` 三个全屏光斑 `blur-[120–140px]` → 静态径向渐变（`theme.css` 新增 `.ambient-glow-*`）；`theme.css` `.plate-sculpted`、`workbench-ui.css` `.wb-v2 .pane`、`offline-flows.css` `.ofl-v2 .pane` 去掉卡片级 `backdrop-filter`；`shell/HomeDashboard.tsx` 三处卡片去掉 `backdrop-blur-md/xl`；`shell/Dock.tsx` 去掉 `backdrop-blur-2xl/md`（底色 .85→.95）。保留的唯一关键视觉层是 `shell/TopNav.tsx` 顶栏 `backdrop-blur-2xl`。未改 JSX 结构/props/交互，未动无限动画或定时重渲染（属 Step 4），未全面删除玻璃特效，未重构 UI 架构。**零 Rust 改动**；`runtime/` 无差异；`scripts/dev-ui.sh` 默认值未动。
- **文件事实核对：通过（含一处 git 局限）。** 我在**派发 BUILDER 之前**已独立读取过改前内容（`App.tsx:87-89` 的 `blur-[130/140/120px]`、`workbench-ui.css:37` / `offline-flows.css:35` 的 `backdrop-filter: blur(20px) saturate(160%)`、`theme.css:102` 的 `blur(28px)`、`Dock.tsx:55` / `TopNav.tsx:40` 的 `backdrop-blur-2xl`），改后复查与之一致，改动真实存在。**局限：** 这 6 个文件里 5 个（`theme.css`、`workbench-ui.css`、`offline-flows.css`、`HomeDashboard.tsx`、`Dock.tsx`）在开工前就是未跟踪状态，`git diff HEAD` 看不到 Step 2 对它们的增量，只能靠文件内容 + 开工前读取记录核对；`App.tsx` 的 diff 还混有 OD-ARCH-026 的重写，无法从 git 单独切出本步改动。这是既有的工作区状况，不是本步引入的。
- **证据文件核对：通过。** `tests/verification/desktop/evidence/od-arch-027/step2/` 下的 `step2-{before,after}-debug-vite.json`、各自 6 张截图、`step2-{before,after}-minibrowser-xvfb-dmabuf1.json`、`calibration.json` + 8 张校准图、`scripts/step2-perf.py`、`scripts/step2-minibrowser-control.py`、`baseline-pages/heavy-after.html` 均存在且内容自洽；JSON 含环境快照（构建方式、数据根、显示后端、DMABUF 取值、窗口尺寸、命令、脚本路径、二进制+mtime、HEAD）。时序正确：`before` 采集 16:37、改动落在 16:42–16:43、`after` 采集 16:51。测量脚本本身随证据归档，可复现。
- **独立复跑：通过（口径一致，数量级吻合）。**
  - MiniBrowser 受控对照（我自己新起 Xvfb `:93`，DMABUF=1，每页 20 s，`--out-dir /tmp/od027-arch/verify`）：`simple` **0.0%**、`heavy` **100.3%**、`heavy-after` **26.3%**。与执行方的 0.0 / 100.2 / 19.6（40 s）同量级，证实「沿用归档页与同一脚本」的结论可复现，且**模糊/玻璃是「有持续动画页面」的成本放大器**这一方向成立。
  - 应用级短跑（我自己新起 Xvfb `:94`，debug+vite，DMABUF=1，空闲 25 s，n=2）：`WebKitWebProcess` **0.32%**、App 主进程 0.04%、网络进程 0.0%、无 GPU 进程。与执行方 before 0.33% / after 0.07% 一致——**当前首页空闲 CPU 本来就低**。
- **视觉核对：通过。** before/after 逐页截图像素对比（我自算）：`home` / `workbench` 平均绝对差 0.02、差异像素 ≤0.01%；`offline` / `settings` 平均差 0.21、差异像素 ≤0.22%（差异主要来自界面时钟数字）。三页层级、渐变与布局保留，未见对比度不足或错位。
- **卫生检查（我独立重跑）：通过。** 前端 `npx tsc --noEmit` = 0；`npm run build` 通过，`dist` 中已无 `blur(1xx px)` 且含 `ambient-glow-a/b/c`；`git diff --check HEAD` 干净；`git status --porcelain runtime/` = 0；`git status --porcelain` 前后均为 **108** 项（新增文件都落在原本已未跟踪的 `evidence/od-arch-027/` 内）。
- **关键限定（必须让用户知晓，不得当作性能提升上报）：** 计划 §1.2 / 证据索引 §3 记录的「App 空闲停在首页时 `WebKitWebProcess` 约 100%」**在当前工作区未复现**（执行方 before 0.33%，我复跑 0.32%）。因此 before/after 的空闲 CPU 差异在噪声量级、导航延迟（含截图读回开销）无可靠差异，**本步没有可宣称的应用级收益**。受控对照只能证明「模糊在持续动画页面上放大成本」，那是受控页面证据，不等于真实应用。**推论（未验证）：** 原始 ~100% 更可能与「持续重绘来源（无限动画/定时更新）+ 大面积模糊」的组合有关，而 OD-ARCH-026 重写首页后该组合已不再触发；确切原因需 Step 4 的动画单因素对照与真实桌面复测判定，**不得据本步结案，也不得改口径**。
- **交付缺陷（不影响本步证据，但脚本作为交付物应修）：** `step2-perf.py` 显式传 `--shots-dir` 时该值为 `str` 而非 `Path`，会 `AttributeError`（默认路径 `shots_dir = out_dir` 不受影响，本次证据未走该参数）。
- **未验证/边界：** Xvfb 是无 GPU 的软件合成，与真实 Wayland/GPU 桌面不可直接等同；release+内嵌产物未测；before/after 应用级各只跑 1 次（我复跑为 25 s/20 s 短程）；连续输入丢字、动画治理、`prefers-reduced-motion` 均属 Step 4，未测；真实显示器/窗口管理器未测。
- **工作区：** HEAD `5ffc710`，108 项既有差异完整保留；本次复核未改任何产品代码，只写本结论与 `PLANS.md` 状态行；未提交、未推送、未打标签。
- **下一步：** Step 3（DMABUF/显示后端独立对照，零代码改动）与 Step 4（动画/React 重渲染治理）均需单独批准。鉴于原始症状未复现，建议在 Step 4 补「无限动画单因素对照」，或请用户在真实 Wayland 桌面按当时场景复现后再定位；在拿到真实桌面数据前不对本支线结案。**OD-ARCH-026 的功能复核仍待进行，与本步不混同。**

## OD-ARCH-026 执行方记录（非审查结论，2026-10-19）

> 本节由**执行方**写入，只记录本轮实际运行了什么、观察到什么、哪些没验证，供 ARCHITECT 独立复核定位证据；
> **不构成任何审查结论**，也不表示本计划已通过。ARCHITECT 的结论请写在本节之上。

- 授权与范围：用户「批准执行 OD-ARCH-026」（澄清后确认指 PLANS.md 中已有的产品校正计划）。开工 HEAD `5ffc710`；
  未提交/推送/打标签；未改 `runtime/`、模型路由、凭证、用户数据、真实材料与第二 worktree。
- 证据索引：`tests/verification/desktop/evidence/od-arch-026/README.md`（含冒烟 4 张、周报闭环 10 张真实窗口截图、
  2 个 JSON 结果、1 份真实窗口保存的成果文件）。
- 实际结果：首页改为对话中枢且模型权限由宿主事实驱动；三场景改为模型起草并支持在界面选择材料文件；
  确定性核对单独成栏且不参与保存。Rust 142/142、`tsc`/构建通过、`verify_ai_draft` 27/0、
  新增 `verify_scenario_materials` 5 场景 0 failed、桌面链路/工作台引擎路径/取消/会话回归通过；
  一次性真实窗口完成周报场景「选择材料 → 模型初稿 → 人工确认 → 保存」闭环。
- 请 ARCHITECT 重点独立核对（执行方**未**独立证明、只做如实记录）：
  1. 「成果只能来自模型初稿、确定性结果不作为成果」的判定是否满足计划口径（本轮刻意取消了「仅本地结果也可保存」的旧路径）；
  2. 材料读取是**新的宿主能力**（只读单文件）：边界是否足够（白名单/大小/UTF-8/禁止片段），是否需要更严格的实现；
  3. 流程包新增可选字段 `draft.rules/sections` 使**旧包缺省不再报缺字段**，是否与 OD-ARCH-019「缺项被拒绝」的口径冲突；
  4. GUI 只逐点走完「周报」一个场景，另两场景仅覆盖引擎链路，是否足以支撑本计划的验收。
- 已知边界与未验证项：真实模型质量/风格、材料结构化解析、模型调用预制 Python、跨平台与系统级安装、
  保存后窗口内重开。完整清单见证据索引 §3。

## OD-ARCH-025：个人试用前桌面闭环验收（ARCHITECT 独立复核，2026-10-19）

**结论：技术检查通过；产品验收不通过。** 自动化测试跑通了合成引擎、保存、重启与安装入口，但实际产品仍不是用户要的 AI 工作台，因此暂不建议按当前界面开始真实工作场景试用。

- 首页当前为仪表盘；任务提交只在任务工作台。它没有以对话作为首页和主要工作入口。
- 周报、文案和项目跟踪的离线流程由本地确定性逻辑生成汇总/结构提示；合成模型链路另有测试，但这不证明三类场景由模型完成整理提炼。
- UI v2 worktree 已有首页对话与左侧 Dock 的候选实现，可供选择性借鉴；其自身将部分场景标为未实现，不能整体合并或当作功能证据。
- 测试依据：OD-025 证据索引记录窗口 A/B/C/D、Rust 134/134、前端构建与类型检查、桌面任务链和回归通过；该证据验证了相应技术行为，不能覆盖上列产品缺口。证据见 [OD-025 索引](tests/verification/desktop/evidence/od-arch-025/README.md)。
- 本次复核未改产品代码、未合并 worktree、未提交或推送；主工作区 HEAD 5ffc710。既有改动继续保留。

**后续方向：** OD-ARCH-026 已更新为待批准计划：对话作为中枢，左侧进入周报、文案和项目跟踪；由模型基于用户输入及明确选取的材料完成整理提炼；确定性代码负责校验与计算。数据库分析功能后置，需另行规划与验收。

## OD-ARCH-025 执行方记录（非审查结论，2026-10-19）

> 本节由 BUILDER 写入，仅记录本轮**实际运行了什么、观察到什么、哪些没验证**，供 ARCHITECT 独立复核时定位证据；
> **不构成任何审查结论**，也不表示本计划已通过。ARCHITECT 的结论请写在本节之上。

- 授权与范围：用户授权执行 OD-ARCH-025；开工 HEAD `5ffc710`；未提交/推送/打标签；未改动 `runtime/`、模型路由、凭证、用户数据、真实材料与第二 worktree；**未改动任何 Rust 或前端产品源码**（只新增验收装置与证据/状态文档）。
- 证据索引：`tests/verification/desktop/evidence/od-arch-025/README.md`（含 4 个阶段 JSON、44 张真实窗口截图、10 个检查日志）。
- 执行方式：`Xvfb :95` 一次性显示 + 隔离数据根 `/tmp/od025-evidence/run` + 回环合成端点 + 用户设置成果目录 `/tmp/od25a`；
  输入用 XTEST 注入、结论以文件事实（设置文件/成果文件/引擎任务台账）判定。装置在 `tests/verification/desktop/window_harness.py`、
  场景在 `run_window_acceptance.py`（可复现命令见证据索引 §2）。
- 实际结果：四阶段 verdict 全 pass；Rust 离线全测 134/134、前端构建与 `tsc --noEmit`、桌面引擎链路/AI 初稿/取消/会话连续性回归、
  `git diff --check HEAD`、`runtime/` 无差异全部通过。
- 请 ARCHITECT 重点独立核对（本执行方**未**独立证明、只做了如实记录）：
  1. 「用户设置的成果目录实际生效」的判定只用了设置文件 + 落盘文件，未逐帧核对界面文案；
  2. 阶段 C 的「被打断任务」台账落在 `starting`（非终态）：是否符合会话/任务契约中「不伪造未完成任务结果」的要求，
     以及是否需要归一化为 interrupted/unknown，属 ARCHITECT 判断；
  3. 慢关闭耗时 0.25 s 只代表本机这次运行；真实显示器、真实窗口管理器、跨平台、系统级 `dpkg -i`、真实模型质量均为未验证。
- 本轮装置自身的已知脆弱点（影响证据强度的诚实说明）：无窗口管理器下截图会滞后，需 1px 尺寸微调强制重绘；
  滚轮步长有 ±30 px 漂移，因此按钮采用「按主色在帧内定位后点击」并配合文件事实重试；窗口尺寸若显著改变，
  WebKit 输入坐标映射会失真，故本轮统一使用默认 1280×820。

## OD-ARCH-024：第四轮补测后的独立复核（2026-10-19）

**结论：按获批范围通过。** 窗口销毁入口现复用 stop_engine()，关闭等待期间不占用宿主锁；确定性宿主测试覆盖同一生产入口，并验证状态、结果、取消不会排队等待。

- **代码审查：通过。** WindowEvent::Destroyed 调用 on_window_destroyed_shutdown()，该函数转入 stop_engine()；生产路径未再直接调用 bridge.shutdown() 或在回调中持锁等待。测试确认旧实现会被行为测试和接线测试捕获。
- **独立复测：** 前端构建、桌面成功/失败/取消链、取消检查 34+8、草稿检查 22、会话连续性 37、模型路由 21/21、git diff --check HEAD 均通过；runtime/ 无差异。Rust 全量 134/134 有执行方保存的当前源码编译日志，本轮环境缺少 cargo/rustc，未能独立重跑。
- **边界：** Tauri 事件回调仍同步等待关闭，这是已明确接受且本轮未试图改变的行为；本轮只保证等待时不占宿主锁。没有在真实窗口执行销毁验证；真实模型和跨平台行为未验证。
- **工作区：** HEAD 5ffc710，101 项现有差异保留；未提交、未推送、未打标签，runtime/ 无差异。

执行证据见 [OD-ARCH-024 证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)「第四轮」。**OD-ARCH-024 验收通过；此结论不授权提交、推送或开启真实模型/真实材料测试。**

## OD-ARCH-024：第三轮补测后的独立复核（2026-10-19，已由第四轮关闭）

**结论：部分通过，暂不验收。** 第三轮解决了 supervisor 锁内的关闭判定竞态，但生产窗口销毁回调仍在持有整个宿主锁时直接调用 `bridge.shutdown()`。该函数虽然把协议关闭移到 supervisor 锁外，却会在锁外等待时一直占用调用方持有的宿主锁；状态、结果和取消入口都需要同一宿主锁，因此窗口关闭期间仍会被阻塞。

- **通过项：** `stop_engine()` 已采用短暂持锁摘除通道、锁外完成耗时收尾的两段式关闭；任务命令在 supervisor 锁内判定关闭状态，第三轮确定性时序测试及变异验证覆盖了此前竞态。独立重跑的前端构建、桌面链、取消检查（34+8）、AI 草稿检查（22）、会话连续性（37）、路由自测（21）及 `git diff --check HEAD` 均通过；`runtime/` 无差异。
- **未通过项：** `desktop/src-tauri/src/lib.rs:1159-1165` 的 `WindowEvent::Destroyed` 回调获取 `state.lock()` 后调用 `host.bridge.shutdown(Duration::from_secs(5))`。生产关闭路径因此绕过 `stop_engine()` 的宿主锁释放设计。其他生产代码未发现直接调用 `bridge.shutdown()`；其余命中在测试清理代码中。
- **影响：** 若关闭耗时，窗口销毁回调持有 `SharedHost` 锁直到关闭完成；并发到达的任务状态、结果、取消命令会排队，失去计划要求的快速、明确关闭语义。现有第三轮并发测试覆盖 `stop_engine()`，不覆盖窗口销毁回调。
- **下一步修正要求：** 将窗口销毁路径接入同一无宿主锁等待的关闭实现（或提供行为等价且可测试的共享关闭函数）；新增宿主级回归，证明关闭等待时宿主锁可被状态/结果/取消入口取得，并验证关闭期间请求的送达/拒绝语义。重跑 OD-ARCH-024 要求的 Rust 全量及桌面回归并更新证据索引。不得改 Runtime、路由、凭证或用户数据。
- **验证限制：** 本轮确认环境无 `cargo`/`rustc`，Rust **133/133** 为 BUILDER 执行日志证据，未能独立重跑；其余独立回归按上列结果通过。HEAD `5ffc710`，工作区 101 项差异完整保留，未提交、未推送、未打标签。

**验收决定（已被第四轮取代）：** 当时暂不通过；第四轮统一关闭入口并补测后，最终结论见本文件顶部。

> **【执行方补记 · 2026-10-19，非审查结论】** 已按上述「下一步修正要求」与本轮批准的计划完成修正：
> 新增 `on_window_destroyed_shutdown(&SharedHost)` **复用共享入口 `stop_engine()`**，`WindowEvent::Destroyed`
> 回调改为只调用它（不再 `state.lock()` 后直连 `bridge.shutdown()`）；`EngineBridge::shutdown()` 文档补充
> “调用方不得在持有宿主锁时等待本方法”；源码接线测试新增“生产代码不得出现 `bridge.shutdown(`”与
> “窗口回调必须走共享入口且不得含 `state.lock()`”。新增确定性宿主级回归
> `window_destroy_shutdown_does_not_hold_host_lock_while_waiting`（真实在途任务 + 8 s 关闭窗口）：
> **关闭等待期间宿主锁 <1.5 s 可取得**；取消/状态/结果 <1.5 s 返回 `engine_shutting_down`（“未发送”、
> `sent_to_engine=false`）；`omni.shutdown` 之后引擎未收到 `omni.cancel_task`。
> **变异验证：** 把该入口改回“持宿主锁 + `bridge.shutdown`”后，同一测试以
> `关闭等待期间宿主锁应可立刻取得（等了 8.041135831s）` 失败，接线测试同时失败。
> **本轮结果：** `cargo test --offline` **134/134**（116 lib + 2 admission + 13 host + 3 e2e，无警告）；
> `tsc --noEmit` 0；`npm run build` 成功；路由 21/21；桌面链全 pass；取消 34+8、草稿 22、会话 37 全 pass；
> `git diff --check HEAD` 0；`runtime/` 无差异。证据：
> `tests/verification/desktop/evidence/od-arch-024/README.md`「第四轮」与 `cargo-test-offline-round4.log`；
> 执行事实：`STATUS.md` 的 OD-ARCH-024「第四轮」小节。本补记不改动上述审查结论。

## OD-ARCH-024：生命周期自锁修复独立复核（2026-10-19）

**结论：部分通过，暂不验收。** 自锁修复、世代保护，以及关闭信号置位后的快速拒绝均有源码和执行方测试证据。但状态/结果/取消检查关闭信号与取得引擎锁之间仍有竞态：命令先检查到“未关闭”，随后关闭流程可能先取得 supervisor 锁；命令会等关闭结束后才返回普通错误，未能及时说明请求未发送。

- **通过项：** `CallGuard::current_epoch()` 从已持有的守卫读取世代；`with_engine_call` 和 `with_call_gate` 不再重入锁；权限世代校验保留。真实源码新编译的 Rust 测试 **130/130** 通过，未跳过测试；前端 `npm run build`、桌面 success/failure/cancel 链、取消检查 34+8、AI 草稿 22、会话连续性 37、模型路由 21/21 均通过。
- **待补缺口：** 新增关闭中测试等到 `is_shutting_down()==true` 后才调用任务命令，未覆盖“检查标志通过后，关闭流程抢先取得 supervisor 锁”的时序。源码中任务命令先检查原子标志，再由 bridge 获取 supervisor 锁；`stop_engine` 则在释放宿主锁后置标并获取 supervisor 锁。因此取消可能在检查通过后被关闭抢先，等待关闭最长约 25 秒，最后才得到普通 `engine_unavailable`；状态/结果同样需要验证。
- **限定补测要求：** 用确定性测试同步点卡在关闭信号检查通过、请求尚未取得 supervisor 锁的间隙，再让关闭流程先取得该锁并进入延迟关闭；验证取消、状态、结果的延迟与实际错误。实现一个清楚的并发边界，使请求要么先于关闭送达，要么快速返回明确的“关闭中/未发送”，不能等关闭后才普通失败。重跑 OD-024 全回归；不得改 Runtime、路由、凭证或真实材料。
- **独立验证：** `cd desktop/src-tauri && cargo test --offline -q`：112 lib + 2 admission + 13 host + 3 e2e = **130 通过、0 失败、0 忽略**；`cd desktop && npm run build` 成功；`python3 tests/verification/desktop/run_desktop_e2e.py chain` 三场景通过；`verify_engine_cancel.py` 34+8 全通过；`verify_ai_draft.py` 22 项、`verify_session_continuity.py` 37 项全通过；模型路由自测 21/21；`git diff --check HEAD` 通过；`runtime/` 无差异。
- **边界：** HEAD `5ffc710`；工作区原有差异均保留，当前 `git status --short` 为 100 项。未提交、未推送、未打标签；未修改 `runtime/`、凭证、模型路由或用户数据。真实模型输出、真实桌面关闭竞争和跨平台行为未验证。

**下一步：** PLANS.md 已记录新的限定补测/修正要求，当前待用户批准后交由 BUILDER 执行。完成后再次独立复核；本结论不构成合并或提交授权。

> **【执行方补记 · 2026-10-19，非审查结论】** 已按上节「所需修正」与 PLANS.md 的限定任务完成两轮补测与修正：
>
> **第二轮**（关闭信号置位后的快速拒绝）：用**可控假引擎建立真实运行中的任务**，断言取消在关闭前
> **送达引擎**（假引擎方法日志可见 `omni.cancel_task`）。
>
> **第三轮**（消除“检查通过后被关闭抢先”的窗口）：把“关闭中”判定从 `EngineLifecycle` 的无锁原子标志
> **移入 supervisor 锁内**，与「摘除通道」在同一临界区原子完成（`begin_stop()`）；耗时的关闭收尾
> （`finish_stop()`）移到**锁外**。任务命令的判定与发送同在这一把锁内（`rpc`/`with_channel`），因此只会
> 有“关闭前送达”或“锁内立即返回 `engine_shutting_down`（未发送）”两种结果。
> **确定性时序测试**把取消停在“已进入发送路径、尚未取得 supervisor 锁”，让关闭先完成 `begin_stop()` 并
> 进入耗时阶段，再放行：取消**未送达**、**<1.5 s** 返回明确结论（错误码 `engine_shutting_down`、消息含
> “未发送”、`detail.engine_detail.sent_to_engine=false`）。**变异验证：** 把耗时关闭放回锁内后，该测试以
> 「关闭先取得引擎锁时，取消也不得排队等待（等了 8.042621669s）」失败；移除两处判定后退回误导性的
> `engine_unavailable` 并失败。
> **本轮结果：** `cargo test --offline` 133/133（115 lib + 2 admission + 13 host + 3 e2e，无警告）；
> `tsc --noEmit` 0；`npm run build` 成功；路由 21/21；桌面链全 pass；取消 34+8、草稿 22、会话 37 全 pass；
> `git diff --check HEAD` 0；`runtime/` 无差异。证据：
> `tests/verification/desktop/evidence/od-arch-024/README.md`「第三轮」与 `cargo-test-offline-round3.log`；
> 执行事实：`STATUS.md` 的 OD-ARCH-024「第三轮」小节。本补记不改动上述审查结论。

## OD-ARCH-023：模型权限变更并发修复独立复核（2026-10-18）

**结论：不通过，存在阻断级缺陷；不得按当前实现验收。** with_engine_call() 与 with_call_gate() 取得 begin_call() 守卫后，守卫仍持有 EngineLifecycle.inner 的 MutexGuard；随后又调用 lifecycle.is_current()，该方法会再次锁同一把非可重入互斥锁。执行到这里的宿主命令会在引擎动作前自锁，因而描述、任务提交、会话和任务记录等受这两个包装器保护的调用都可能永久等待。

- **证据：** desktop/src-tauri/src/engine_lifecycle.rs 中 CallGuard 持有 MutexGuard<State>；begin_call() 将该守卫带出；is_current() 调用 self.inner.lock()。desktop/src-tauri/src/lib.rs 第 100–110 行及第 134–143 行显示两个包装器都在守卫存活期间调用 is_current()。锁非可重入，因此该等待可由源码直接推导，不依赖竞态概率。
- **另一个覆盖缺口：** engine_task_status、engine_task_result、engine_cancel_task 仍直接锁宿主并访问 bridge，没有生命周期守卫；源码接线测试的命令清单也没有检查这三个入口。需明确其与权限变更、引擎停止并发时的语义并以宿主级测试验证。
- **验证边界：** 本轮独立复跑 npm run build、模型路由自测 21/21、桌面成功/失败/取消链路、取消 34 项加 8 项静态检查、AI 草稿 22 项、会话连续性 37 项，均通过。现有预编译 Rust 测试二进制报告为 108 个库测试、2 个准入测试、13 个宿主集成测试和 3 个任务端到端测试通过；从当前源码重新编译完整 Rust 套件因环境缺 Cairo/GDK 等系统开发库而失败，故这些通过数不能证明新源码无自锁。git diff --check HEAD 通过，runtime/ 无差异。
- **边界：** HEAD 为 5ffc710；约 99 项既有未提交/未跟踪差异均保留。未改源码、未提交或推送、未触碰 Runtime、凭证、模型路由、真实材料或第二 worktree。

**下一步：** OD-ARCH-024 集中修复自锁，定义状态/结果/取消命令的生命周期并发行为，补足真实宿主包装器测试并重跑可用回归。计划列于 PLANS.md，待用户批准后交由用户选择的 BUILDER 执行。

## OD-ARCH-022：端点去向说明、权限切换一致性与安装验收（2026-10-18）

**结论：部分通过，暂不验收。** 端点说明不再把私有地址说成单位网络；前端构建、路由分类自测和隔离安装证据通过。但权限切换存在并发竞态：任务或会话命令可先取得旧准入结果，在撤权停止引擎后才使用该旧结果重新启动引擎。

- **独立复核：** `cd desktop && npm run build` 成功；`node scripts/dump-model-route.mjs --self-test` 为 **21/21**；`git diff --check HEAD` 通过；`runtime/` 无差异。隔离 `.deb` 安装、桌面入口启动和卸载有执行证据，见 [OD-ARCH-022 证据索引](tests/verification/desktop/evidence/od-arch-022/README.md)。当前环境没有 `cargo`/`rustc`，因此 121 项 Rust 测试仅有 BUILDER 留存结果，本轮未能独立重跑。
- **已通过部分：** `local/private/public` 文案区分本机回环与非回环地址；明确地址范围不证明组织归属或实际转发路径；旧 `internal/external` 设置有兼容迁移。分类器 21 项复测与文档一致。前端产物构建成功；安装报告记录在隔离用户根中完成安装、启动、卸载，未提权。
- **必须修复：** `engine_submit_task` 在锁外读取准入事实并取得 `AdmissionGrant`，之后才检查屏障并获取宿主锁；`engine_describe`、会话列表/恢复及任务台账/恢复入口也会锁外取得准入结果，未检查权限屏障。权限变更先锁外写设置，再关引擎，最后才设置屏障。并发时，调用可持有变更前的 grant，在引擎停止之后、屏障落下之前重新启动并执行。纯函数测试只测停止成功/失败映射，未覆盖此时序。该缺陷直接违反“撤权期间旧权限不得继续启动/处理新请求”的验收条件。
- **限制：** 停止失败只由纯函数单测覆盖，没有真实子进程停止失败的窗口验证；跨平台、真实模型输出和真实材料均未验证。当前没有证据表明安装验证进行了系统级安装。
- **工作区：** HEAD `5ffc710`，工作区差异保留；`runtime/` 无差异；未提交、未推送、未打标签。

**下一步：** 见 [PLANS.md](PLANS.md) 中待批准的 OD-ARCH-023。由用户指定 BUILDER；修复准入检查与引擎生命周期之间的竞态，并增加可重复的并发时序测试。

## OD-ARCH-021：模型权限、AI 任务闭环与 Linux 安装复核（2026-10-18）

**结论：部分通过，暂不整体验收。** 合成端点下的任务提交、成功/失败/取消、人工确认与保存有窗口证据；默认关闭、显式开启和撤销设置已实现。但端点分类把“私有地址”说成“单位内部”，权限变更在引擎停止失败时仍报告成功，可能让显示许可与实际运行中的引擎不一致。安装包只解包启动，未完成计划要求的隔离安装验收。

- **独立检查：** 前端构建和类型检查通过；Rust 离线测试 116/116 通过；会话连续性 37/37、AI 草稿 22/22、取消 34/34 + 静态 8/8、桌面成功/失败/取消链路通过；git diff --check HEAD 通过，runtime/ 无差异。路由分类自测实际为 **18/18**，BUILDER 文档误记为 22 项，需校正记录。
- **需要修正：** dump-model-route.mjs 将 RFC1918、链路本地及 .local 地址归为 internal，而设置文字将其解释成单位网络。地址范围无法证明组织归属；使用者可能据此误以为数据不会离开单位。应改成不作组织归属承诺的分类和说明，保留未知/未核实端点的拒绝行为。
- **权限与运行进程：** model_permission_set 和撤销命令先保存设置，再调用 shutdown；关闭失败时仅记录日志，仍返回 ok: true 和“下次提交按新权限重新判定”。若旧进程仍存活，当前配置分类不能证明该进程已切换路由。权限变更须失败关闭，并确保旧进程停止、任务请求被阻断后才报告生效；补充停止失败测试。
- **安装验收：** .deb 元数据、图标、启动器文件、依赖和解包启动有证据；实际安装到隔离/用户级环境未完成，计划的“安装后启动”标准未满足。
- **限制：** 真实模型质量、真实工作材料、跨平台均未验证；红线扫描只是启发式检查。WebKit/Xvfb 下头像点击卡死尚未查明，后续需单独复测。
- **工作区：** HEAD 5ffc710；全部既有未提交与未跟踪改动保留；runtime/ 无差异；未提交、推送或打标签。

**后续计划：** OD-ARCH-022 将集中校准端点分类与界面承诺、确保权限变更和运行引擎一致、修正 18 项测试记录并完成隔离安装验收。计划见 PLANS.md，待用户批准。

---


## OD-ARCH-020：个人试用版闭环独立复核（2026-10-18）

**结论：部分通过，整体未验收。** 安装包构建、成果目录设置、离线成果保存/重开通过；但计划要求的用户可配置模型权限和 AI 任务闭环尚未完成，因此还不能称为 AI 工作流程已交付，也未达到交用户做完整应用体验验收的条件。

- **独立复核：** `npm run build` 通过；Rust `cargo test --offline` 为 105/105（89 单元、13 集成、3 E2E）通过；`verify_ai_draft.py` 20/20、`verify_session_continuity.py` 37/37、`verify_engine_cancel.py` 34 状态用例 + 8 静态检查均通过；`git diff --check HEAD` 通过；`runtime/` 无差异。Rust 有一条未使用常量警告。
- **已验收范围：** Linux `.deb` 构建产物存在并有包元数据；隔离窗口证据支持成果目录修改与重启保留、离线成果保存/只读重开、旧文件哈希不变。窗口交互依据 BUILDER 留存证据，本轮 ARCHITECT 未亲自安装或重跑桌面窗口。
- **未通过的核心要求：** `desktop/src-tauri/src/admission.rs` 与 `desktop/src/engineAdmission.ts` 均把端点许可固定为 `false`；`settings.rs` 仅持久化成果目录，设置页没有模型权限选项。AI 初稿只实测拒绝、输入未发出；提交成功、取消、错误恢复、模型输出和质量均未验证。OD-020 原计划要求严格默认且允许用户明确开放普通任务权限，这部分是源码中确实不存在。
- **发行限制：** AppImage 构建失败，`.deb` 未安装；安装后启动、桌面图标实际启动及 Windows/macOS 均未验证。目录设置是绝对路径输入框，不是原生文件选择器。
- **保护边界：** 未读取凭证或真实工作材料，没有调用模型或修改路由；`runtime/` 干净。主工作区 HEAD 仍为 `5ffc710`，所有原有改动保留；未提交、未推送、未打标签，UI worktree 未修改。

**下一步建议：** 见 `PLANS.md` 的 OD-ARCH-021：补齐用户可配置模型权限与合成端点 AI 任务闭环，并完成 Linux 安装启动验收。若无法从非秘密配置可靠识别端点类别，就停止实现分类并先作架构决策，不用用户填写的显示名称伪装识别结果。真实服务商模型及真实材料测试仍需用户明确指定授权范围和端点。

## OD-ARCH-019：可编辑离线流程包与轻量个人试用记录独立复核（2026-10-18）

**结论：核心离线功能有条件通过，可开始个人试用；三个场景的 AI 写作功能尚未完成。** React 界面到 Tauri/Rust 本地流程、仓库外成果保存/重开、流程配置重载和本地使用记录链路均有隔离窗口证据。模型准入仍关闭，因此“周报、一般文案、项目跟进”目前是确定性离线整理/结构流程，不是模型生成能力。

- **独立复核证据：** `cd desktop && npm run build` 通过；`verify_session_continuity.py` 37/37；`verify_engine_cancel.py` 34/34、静态 8/8；`git diff --check HEAD` 通过；`git diff --exit-code HEAD -- runtime/` 无差异；端点准入前端与 Rust 常量均为 `false`。BUILDER 提供的隔离真实 Tauri 窗口证据显示三个场景可修改流程包后重载、处理合成输入、保存并重开成果，使用记录可写入和删除。当前复核环境没有 `cargo`/`rustc`，因此 Rust 100/100 与离线场景 Rust 运行结果属于 BUILDER 留存证据，未能独立重跑。
- **产品边界：** 可编辑内容限于标签、说明、占位符、结果标题和章节名；日期、计数与逾期计算仍是固定代码。使用记录不自动写入，只含预设枚举、布尔值、时间和粗耗时，可查看/删除/清空。配置文件和记录均在仓库外。真实模型输出、真实工作材料、Windows/macOS、长期使用表现未验证。
- **试用裁定：** 用户可以现在用合成或公开材料检验这三条离线流程的入口、易懂度、步骤、输出结构、保存和重开体验，并给出主观改进意见；不要据此评价 AI 写作质量或真实材料适用性。
- **工作区：** HEAD `5ffc710`；本轮开始时 86 项未提交/未跟踪改动保留；未提交、未推送、未打标签。

## OD-ARCH-019 后续建议

先由用户在当前桌面版做一次集中体验，再根据反馈安排 UI 迭代。OpenCode 可承担独立的视觉与交互改进，但应在隔离分支/工作区实施，只改获批的界面范围；不得触碰 Rust/IPC、离线计算、数据保存、模型准入或 Runtime。达到用户认可后再由 ARCHITECT 审查差异、运行回归，并另行决定是否选择性合并。当前 AI 模型链路仍因准入关闭不可测。

## OD-ARCH-018：个人试用准备与桌面交互验收独立复核（2026-10-18）

**结论：实现与合成数据桌面流程有条件通过；三个离线场景在 Linux 虚拟桌面中完成保存和重开。取消按钮的 IPC 状态替身验证通过，但真实窗口取消操作未验收。** 本次按当前工作区差异审查，不改代码；保留工作区全部 82 项未提交/未跟踪状态，HEAD `5ffc710`，未提交或推送。

- **通过证据：** `verify_session_continuity.py` 37/37；`verify_engine_cancel.py` 状态/IPC 34/34、静态 8/8；`npm run build` 通过；`git diff --check HEAD` 通过；`runtime/` 无差异。OD-ARCH-018 的执行证据记录 Rust 完整回归 92/92、离线场景评估通过和 Xvfb 窗口操作。当前复核环境没有 `cargo` 命令，因此 Rust 92 项是 BUILDER 执行证据，未能独立重跑。截图覆盖周报、文案和项目跟进输入、结果、确认、保存、列表、重开；仓库外隔离成果均有对应记录。
- **敏感元数据：通过代码与合成哨兵检查。** 会话响应只输出工作区标签/不透明 token，不返回真实 `cwd`；任务台账序列化不输出自由文本 `intent`；会话连续性验证使用路径和意图哨兵检查响应，UI 只用工作区标签。截图中的会话/任务页因准入关闭显示拒绝提示，未读取真实会话。端点准入两侧仍固定 `false`。
- **真实窗口取消交互：未验证。** 该状态由共享取消发送函数和宿主替身统计 IPC，足以支持 34/34 的逻辑验证；因真实任务入口被准入关闭，窗口中无法形成可取消的引擎任务。不得把替身结果表述为真实 ACP/桌面引擎取消已验证。
- **窗口证据范围：有限。** Xvfb + XTEST 证明真实 Tauri 窗口路径收到了自动化鼠标/键盘事件，但没有人工手点确认视觉体验；也没有真实模型写作、真实材料、跨平台或长期使用证据。离线流程为本地确定性整理，不能作为 AI 文案质量证据。成果列表修复用例由 BUILDER 记录 4 项及完整 Rust 回归通过，本轮受 cargo 缺失限制未能重新编译验证。
- **下一步：** OD-ARCH-018 的实现范围有条件验收。用户可用虚构或公开材料试用三个离线流程；模型写作、真实工作材料和分发不在授权范围内。OD-ARCH-019 拟先让离线流程的文案与结构可在本地配置中调整，并提供可选、可查看和可清除的非正文使用记录；会话页仍因准入关闭而不可用，不在该计划中绕过。计划详见 `PLANS.md` 顶部，待用户批准。

## OD-ARCH-017：会话连续性纵向能力独立复核（2026-10-18）

**结论：有条件通过代码与适配层纵向验证；尚未完成真实桌面界面验收。** 会话列表、同数据根重开、任务状态恢复和取消重复请求修正有源码与合成引擎运行证据。旧 DSH 会话导入因锁定版没有公开导入接口，按计划停止并记录为不支持。

- **主要证据：** `verify_session_continuity.py` 32/32 通过（真实 DSH 引擎 + 回环合成端点）；取消状态验证 23/23、静态边界 8/8 通过；前端类型检查和构建通过；E01–E20 适配器场景通过。`git diff --check HEAD` 通过，`runtime/` 无差异，模型端点两侧仍关闭。
- **差异与限制：** 本轮独立重跑取消/会话验证和前端构建通过。尝试重跑 Rust 测试，但当前环境缺少 GTK/GDK/Cairo/Pango/ATK 系统开发库，无法编译；BUILDER 留存的 88 项通过日志是本轮执行记录，不能替代本次复跑。适配器 E01–E20 重跑在本环境没有及时返回，已中止，不将其作为本轮独立复验；BUILDER 证据日志全部为通过。未运行 `rustfmt`，本机工具链未安装该组件。没有真实窗口点击。
- **需产品修正的元数据隐私问题：** `Sessions.tsx` 直接显示会话工作区绝对路径；同一工作区的任务台账展示 `intent`。适配层的会话列表会对所有绝对路径调用 `realpath`，任务列表也无条件返回持久化 `intent`。这些字段可包含用户目录、项目名称或业务细节，与计划要求的“必要元数据”和会话元数据隐私风险不符。端点虽然关闭，界面仍应默认不暴露完整路径，也不应把自由文本意图当安全元数据。下阶段须先最小化展示与返回字段，并以敏感哨兵证明清单/界面不泄漏；不可把此发现解释为已读真实会话，本轮仅检查代码和合成数据。
- **能力边界：** “重开会话”依赖 DSH `session/resume`，会由 DSH 原生追加会话恢复记录；不等于任务续跑。终态只来自已有 OmniDesk 台账的终态证据，非终态恢复为 `unknown`、不自动重放。跨版本导入、真实用户数据、真实模型和窗口交互仍未验证。

**裁定：** OD-ARCH-017 的合成适配层实现按证据通过；总体桌面功能仅条件验收，不能宣称用户已可在当前准入配置下使用会话页。OD-ARCH-018 拟合并处理“会话/任务元数据最小暴露、取消按钮真实交互验证、周报/文案/项目管理离线流程在真实窗口的集中试用验收”，具体计划见 `PLANS.md` 顶部；待用户批准后再交 BUILDER。

## OD-ARCH-016：工作台取消入口独立复核（2026-10-18）

**结论：暂不通过；核心状态显示规则通过，但重复取消边界未满足计划，需在 OD-ARCH-017 中修正并做一次纵向验证。** 基线 HEAD `5ffc710`；工作区既有 75 项差异保留，暂存区无改动；`git diff --check HEAD` 通过，`runtime/` 无差异。端点准入仍关闭。

- **通过项：** `engineCancelButtonState` 仅在任务存在、取得状态且任务非终态时开放；取消结果只有宿主给出取消确认/最终态为 `cancelled` 才报告成功。取消请求用同步锁防止同一请求未返回时快速连点；取消路径未改写已有解释正文。定向验证为 20/20 状态用例、5/5 静态检查通过。独立执行 `cd desktop && npm run build` 通过。证据：`tests/verification/desktop/evidence/od-arch-016/`。
- **阻断（P2）：取消尚未超时仍可重复发起。** `desktop/src/workbench.tsx` 将 `cancelling` 一概视作可点击；当第一次调用返回 `request_sent`，宿主仍报告 `cancelling` 时，同步锁已经解除、`lastPhase` 不是 `timeout_unknown`，按钮仍可再次点击。现有验证脚本还明确把普通 `cancelling` 预期为启用。因此“避免重复取消请求”只覆盖请求在途期间，未覆盖宿主仍在取消状态的整个等待期。应保持禁用，直到任务进入终态，或得到明确 `timeout_unknown` 后开放一次重试。
- **验证边界：** 测试执行真实状态纯函数并静态扫描代码接线，但没有调用按钮事件、模拟异步宿主或验证取消 IPC 次数；没有启动真实窗口/合成宿主。计划允许确定性前端测试，因此本身不单独构成违约；但结合上述状态缺口，取消完整生命周期仍未获运行证据。
- **实际复核：** 本轮再次运行 `cd desktop && npm run build`：通过；`git diff --check HEAD`：通过；`runtime/` 差异为空。未运行 Rust/引擎回归，因为本轮审查无代码改动，BUILDER 本轮亦未改共享状态机或 IPC。未调用模型、未读取凭证/真实材料或会话数据。

**下一步：** 拟议 OD-ARCH-017 将取消重试语义修正与会话列表/恢复、旧 DSH 合成会话导入和重启状态恢复放入同一连续性阶段一次完成。计划待用户批准；批准前不开始实施。

## OD-ARCH-015：UI 整合边界与共享状态独立复核（2026-10-18）

**结论：核对记录按范围通过；发现的取消入口缺陷需单独修复，不能据此验收引擎生成流程。** 本次为当前源码与交付记录复核，没有重跑构建、测试或窗口交互。工作区 HEAD 为 `5ffc710`，73 项既有改动完整保留；`git diff --check HEAD` 通过，`runtime/` 无差异。

- **边界检查：** 静态检查确认前端与 Rust 宿主的端点隔离常量仍为 `false`；前端和宿主均在提交/启动前拒绝。人工确认、来源校验、统计指纹复算、仓库外保存及拒绝覆盖逻辑仍存在。用户输入仍不会因本次审查送往模型。
- **缺陷确认：** `runEngineExplanation` 执行期间 `engineBusy` 持续为真，取消按钮的禁用条件又包含 `engineBusy`；任务结束后终态条件继续禁用按钮。因此工作台取消按钮任何时候都不可用。取消 API 和宿主命令存在，但历史测试只证明底层取消链曾成功，未证明该界面入口可用。依据：`desktop/src/workbench.tsx` 的取消处理及按钮条件；本轮为静态源码证据，未做窗口复现。
- **其他限制：** 无执行前源码快照，无法恢复 `workbench.tsx` 的逐行基线；BUILDER 本轮未改应用代码。没有新证据推翻 OD-ARCH-014 的条件验收记录，也不能消除其已记录的执行范围偏差。
- **未验证：** 取消请求与正常完成竞争时的 UI 状态、取消未确认/超时时的界面表现、真实模型输出、真实工作材料、跨平台与持续使用。

**裁定：** OD-ARCH-015 只读核对通过，交付状态可关闭；产品功能仍有缺陷。下一步只安排 OD-ARCH-016 修复和验证取消入口，端点准入继续关闭。未经 BUILDER 修复及后续独立复核，不得声称取消功能可用。

## OD-ARCH-014：UI 外壳选择性整合独立复核（2026-10-18）

**结论：实现与运行验证通过；计划执行纪律有一项未关闭偏差，因此整体按“有条件通过”记录。** 主工作区 HEAD 为 `5ffc710`，约 73 项工作区改动均保留；未提交、未推送。`runtime/` 无差异，模型准入仍关闭。

- **源码与功能：** 检查了首页、路由、API 准入、任务工作台、离线流程和宿主拒绝闸门。新外壳保留首页、任务工作台、离线流程与只读诊断入口；前端提交统一拒绝，Rust 宿主仍固定拒绝。`workbench.tsx` 含引擎解释提交、状态轮询、取消与结果回填，同时保留原本地统计、手动解释和人工确认保存路径。其当前版本与 UI worktree 文件相同。现有 OD-BUILD-005 执行记录证明引擎支路在本轮之前已存在，但没有保存的执行前源码快照可逐行证明本轮未改动其业务逻辑；因此不把“未覆盖”视为已完全证实。
- **独立验证：** `npm run build` 通过；Rust 离线测试 88/88 通过（72 单元、13 集成、3 E2E）；离线场景评估通过；`git diff --check HEAD` 通过；`runtime/` 无差异。已抽查 BUILDER 提供的 8 张 Xvfb 窗口截图和证据索引，未亲自重跑窗口交互。
- **执行偏差：** 计划第 1 步要求交接文档先校正，并明确命中停止条件时停止。执行记录确认该条件已命中，仍继续整合；此外，计划明文禁止整体覆盖 `workbench.tsx`，执行记录却称该文件采用 UI worktree 版本。测试和当前源码没有显示安全闸门被放宽，但这不消除未经批准越过停止条件与文件范围的事实。
- **限制：** 模型输出、真实工作材料、跨平台与持续使用均未验证；截图仅支持本机 Linux/Xvfb 的窗口路径。

**验收裁定：** UI 外壳及当前可验证功能具备继续个人合成/公开材料试用的技术条件；OD-ARCH-014 的实现结果按范围有条件通过，执行纪律偏差须如实保留。不得据此开放模型端点、真实周报材料或分发。建议下一步先做 OD-ARCH-015 的只读差异与基线核对，明确 `workbench.tsx` 的执行前后边界，并校准共享计划状态；完成前不安排新的功能扩展。详见 [PLANS.md](PLANS.md)。

## OD-ARCH-013：双工作区盘点与新 UI 整合结论（ARCHITECT 独立复核，2026-10-18）

**结论：OD-ARCH-013 的只读审计按范围通过；UI 可进入选择性整合计划，但不能整体合并 `feat/desktop-ui`。** ARCHITECT 独立复核了状态、共同 HEAD、关键差异和准入实现：两工作区均为 HEAD `5ffc710`，主工作区 `main` 保留 59 项状态记录，UI 工作区 `feat/desktop-ui` 保留 71 项状态记录；`runtime/` 无差异。未合并、复制、暂存、提交或推送。本结论仅依据**文件事实与静态代码检查**，没有构建、测试或窗口运行证据。

- **交接文档的核心主张不成立。** `docs/omnidesk-ui-v2-handoff.md` 称本分支已包含主工作区成果、两侧「基本不重叠」。源码核验确认 UI 工作区缺少主工作区的宿主 `EngineSupervisor::start()` fail-closed、成果耐久写入/失败清理、默认成果根 `~/OmniDesk/workspace/artifacts`、离线流程五个命令及 `OfflineFlows.tsx`。因此整分支合并会回退主工作区已有边界与流程。交接文档须先更正，整合须以主工作区现状为代码基线。
- **UI 分支的可取部分是外壳与前端准入。** 新的顶栏、Dock、首页画布、首页对话、设计令牌、内联图标与自托管字体构成完整视觉体系，且未引入运行时外链，与既有 CSP 相容；`engineAdmission.ts` 与统一提交拦截补上了主工作区缺失的前端提示层。宿主仍须以主工作区的 `admission.rs` 拒绝为最终边界，前端闸门只能减少误触，不能证明端点隔离。
- **两边界面功能不对齐。** UI 分支 `App.tsx` 只有首页与分析工作台；主工作区除分析工作台外还有任务工作台、离线流程和引擎技术切片。直接采用 UI 的 `App.tsx` 会让已实现的离线流程从界面消失；主工作区的技术切片也没接入 UI 分支的只读化改造。
- **样式体系会互相影响。** UI 分支把旧深色样式降级进 `@layer legacy` 并新增 Tailwind 构建链与新依赖（`tailwindcss`、`@tailwindcss/vite`、三个 `@fontsource` 包）。这些是本地构建依赖而非外链，但整合会改变主工作区的构建输入，必须与实际构建验证一起评估。
- **启动器按现状不能直接替换。** `scripts/dev-ui.sh` 的隔离与端口处理符合当前安全要求，但它比主工作区启动器多一层「构建 → 前端 → 合成端点 → App」编排，且固定端口 5273。两边启动器若同时存在，需要先明确唯一入口，避免一个脚本拉起另一个 worktree 的页面。

**未验证：** 整合后编译与交互、新外壳承载现有离线流程的窗口体验、跨平台表现、真实模型输出。本轮未运行任何测试或构建，不构成整合、合并或发布授权。

**建议整合顺序（供 ARCHITECT 决定是否立项）：**

1. 先由 UI 分支更正交接文档，明确其基线是旧后端，并列出缺失项。
2. 新建小范围计划：以主工作区为唯一后端基线，只引入视觉层（`theme.css`、`workbench-ui.css`、`components/`、`shell/`、字体依赖与 `vite.config.ts` 的 Tailwind 插件），同时保留主工作区 `App.tsx` 的现有视图与路由。
3. 在同一计划内引入 `engineAdmission.ts` 并接入主工作区 `App.tsx` 的提交入口，使技术切片与新外壳共用一道前端闸门。
4. 在新外壳中恢复离线流程与任务工作台的入口，并做一次完整构建、Rust 回归与离线场景示例验证。
5. 最后统一启动器（评估 `dev-ui.sh` 的隔离编排是否并入主启动器），单独评审端口与进程回收语义。
6. 会话持久化与旧会话导入仍与 UI 整合解耦，待整合稳定后单独规划。

**验收前置：** 整合计划必须证明 `runtime/` 无差异、端点准入仍为 `false`、两边既有未提交改动未被覆盖，并在真实构建与窗口中复验，而非引用本段静态结论。

## OD-ARCH-012：独立验收并关闭 OD-ARCH-011 补充项（2026-10-18）

**结论：OD-ARCH-012 按范围通过；OD-ARCH-011 的成果写入失败清理验收项关闭。** 基线 HEAD `5ffc710`，所有既有未提交和未跟踪改动保留，未合并其他 worktree。端点准入保持关闭，`runtime/` 无差异。

- **源码审查：通过。** 生产写入入口保持两参数签名并固定使用 `NewFileOps::REAL`；故障注入结构为模块私有，只由测试构造，不从 UI、环境变量、配置或请求输入。`create_new(true)` 防止覆盖既有文件；写入/刷盘失败只尝试清理本次创建的目标。清理失败会如实报告，不能保证文件一定消失。
- **独立复跑：通过。** `cargo test --offline`：72 单元、13 宿主集成、3 E2E 全通过；`npm run build` 通过；`cargo run --offline --example evaluate_offline_scenarios` 三场景及日期边界通过；`git diff --check HEAD` 通过；`git diff --exit-code -- runtime/` 无差异。故障注入测试与两次变异结果见 [OD-ARCH-012 证据](tests/verification/desktop/evidence/od-arch-012/README.md)。
- **边界与限制：** 仅 Linux 注入测试，不代表真实磁盘损坏、断电或跨平台验证；模型写作、真实业务用途授权、长期使用效果仍未验证。未提交、未推送、未打标签。

**下一步：** 先制定独立的双工作区保护盘点与新 UI 审查计划，再决定会话恢复/旧会话导入与 UI 整合的次序；本次验收不授权合并、提交或推送。

---

## OD-ARCH-011：独立验收（2026-10-18；当时结论，补充项已由 OD-ARCH-012 关闭）

**当时结论：主体实现通过，OD-ARCH-011 暂不完整验收。** 此记录保留 2026-10-18 当时的审查事实；缺少的写入/刷盘失败运行证据已由顶部 OD-ARCH-012 补足并通过独立审查。审查基线 HEAD `5ffc710`；本工作区和 `/mnt/data/Code/omniharness-ui` worktree 都有大量未提交差异，全部保留、未合并。`runtime/` 无差异；端点准入仍关闭。

- **代码检查：通过。** `parse_date` 真实校验月份天数及闰年，周记录、项目期限、判定日期共用；两类成果共享 `create_new` 写入器，拒绝覆盖并在写入/刷盘失败时尝试清理本次半成品。默认新成果路径改为 `~/OmniDesk/workspace/artifacts`，代码没有迁移或扫描旧 YvanAI 文件正文。UI 明确说明模型不可用、输入范围与红线扫描局限。
- **独立验证：** `cargo test --offline`：68 单元 + 13 宿主集成 + 3 E2E 通过；`npm run build` 通过；`cargo run --offline --example evaluate_offline_scenarios` 三个合成场景及日期边界通过；`git diff --check HEAD` 通过；`git diff --exit-code -- runtime/` 通过。OD-ARCH-011 的窗口截图与证据说明已抽查。
- **未满足项：** `write_new_file_durable()` 的失败清理目前没有可控失败注入测试，只有代码检查、没有运行证据；OD-ARCH-011 的验收标准明确要求该失败路径测试，因此需要一个小型补充修复/验证计划。独立复核没有重跑真实窗口；仅 Linux 证据。模型写作、周报/材料/项目流程的真实用途授权、跨平台与长期使用均未验证。界面仍是本地确定性流程，不等于模型写作能力。
- **文档状态：** `GOALS.md` 已校准为桌面技术切片与离线演示的现状；OD-ARCH-011 仍因缺少失败清理运行证据而未完整验收，当前待批计划已缩小到补齐这项证据。离线演示不等于用户场景 MVP 或模型写作能力已经完成。

**当时下一步建议（已完成）：** 按缩小后的 OD-ARCH-012 补齐写入/刷盘失败清理测试并关闭 OD-ARCH-011 验收。该建议已由顶部审查结论取代。双 worktree 与新 UI 评审、会话持有和旧会话导入顺序另行制定计划，不与故障测试捆绑，也不授权推送或合并。

## OD-ARCH-010：独立验收（2026-10-18）

**结论：离线合成场景按本计划范围通过；暂不作为真实业务或模型写作试用版验收。** 审查基线 HEAD `5ffc710`，工作区保留 56 项既有未提交/未跟踪改动；`runtime/` 无差异。未合并其他 worktree 改动。

- **宿主拒绝入口：通过。** 直接核对 `engine_describe`、`engine_submit_task`、工作目录准备与 `EngineSupervisor::start()`，均在启动/创建副作用之前拒绝。状态、结果、取消命令只在已初始化桥接后才能 RPC；当前唯一初始化入口受拒绝闸门保护。健康查询为只读，关闭命令只停止已有引擎。端点常量仍固定关闭。
- **离线场景：通过，范围有限。** 周记录汇总、文案结构建议、项目跟进均有确定性本地路径、合成样例和人工确认后保存。报告记录的窗口证据与本地运行证据相符；独立复跑 `cargo test --offline`（78 项通过）、`npm run build`、`cargo run --offline --example evaluate_offline_scenarios` 均通过。`git diff --check HEAD` 通过。
- **必须修正的已知问题：** 日期解析接受月内不存在的日期（例如 2 月 30 日），项目逾期和周记录排序可能因此失真；默认成果位置仍叫 `~/YvanAI/projects/Omnidesk/analysis`，与当前约定逐步统一到 `~/OmniDesk` 的方向不一致。没有证据显示本轮迁移或覆盖了既有成果。
- **使用边界：** 文本框接受自由输入，红线扫描只是启发式；这不能保证识别身份或工作细节。当前获知的单位答复只覆盖符合条件的个人数据分析与生态研判，不能据此批准周报、写作或项目管理的真实工作材料。模型准入仍关闭，本轮没有模型调用。
- **未验证：** 真实模型输出与写作品质、真实工作材料准入、跨平台窗口体验、长期任务使用效果。当前证据仅支持纯合成/公开样例的本地确定性步骤。

**下一步：** 待批准计划见 [PLANS.md](PLANS.md) 的 OD-ARCH-011。其目标是修正日历校验、明确新成果保存位置且不迁移旧数据，并让界面更清楚说明当前输入范围；模型准入保持关闭。

## OD-ARCH-009 独立复核补充（2026-10-18）

**结论：端点继续 no-go；执行保护未能在主工作区核实。** 检查基线 HEAD 为 5ffc710，主工作区有 50 项未提交差异，全部保留；runtime/ 无差异。主工作区的引擎描述、任务提交、工作目录准备入口均未接入宿主拒绝闸门，子进程启动也未清理继承环境。另一个 worktree 有固定关闭的 admission.rs 闸门，但代码改动与主工作区不同，且其状态记录仍停留在 OD-ARCH-008，不能当作主工作区已实现。

因此 009 的风险判断成立，但交付文档把建议方案和实际保护混写，不能按“宿主已有 fail-closed 保护”验收。未启动引擎、未调用模型、未读凭证或真实业务材料，也未独立运行测试；这是静态代码检查。端点准入保持关闭。下一阶段必须指定一个执行工作区并先核实宿主闸门，再开展合成材料下的离线工作场景测试；不整体搬运另一工作区改动。

## OD-ARCH-009：端点准入证据与宿主控制审计（BUILDER 交付，待独立复核）

**交付结论：no-go。** 端点准入继续关闭。本段是 BUILDER 的执行结论，不是 ARCHITECT 的验收结论；实际审查须按下列清单核对源码与文档。

- **证据等级：** 全部为源码/文件事实；未运行模型、未启动引擎、未读取凭证或真实材料、未提交或推送。
- **核心事实：** 引擎子进程启动后继承完整宿主环境，未做环境白名单；Python Adapter 同样复制 `os.environ`。因此 `DEEPSEEK_BASE_URL`、`DEEPSEEK_API_KEY` 与代理变量可改变数据去向。`OMNIDESK_DSH_PATCHES`、`OMNIDESK_DSH_CLI`、`OMNIDESK_DSH_PROFILE` 均为无二次校验的普通环境变量；`DSH_HOME` 隔离只覆盖数据目录。
- **默认外发通道：** 锁定版 `dsh-base` 默认启用 `web`/`web-search-deepseek`/`web-fetch-http` 与本机 `bash`/`fs`，并有 `session-telemetry-otel`（默认 FEEDBACK_ONLY，可被环境变量改端点或关闭）。OmniDesk 的 ACP 请求确实传 `mcpServers: []`，但 DSH 内置工具不受此限制。
- **合成机制：** `scripts/dev-ui.sh` 的回环 mock + 一次性 patch 是当前唯一可控端点路径，属脚本层，宿主无法据代码断言其生效。
- **建议最小方案：** 宿主侧白名单环境 + `env_clear()`、显式端点/工具配置、patch 内容校验、回环与真实凭证互斥、无批准记录即拒绝；须配套环境继承、baseURL 注入、patch 注入、无凭证、真实端点拒绝和日志不含秘密的测试。
- **未验证：** DSH 分层 `.env` 的实际载入行为、工具是否实际外发、遥测实际发送；以上仅有源码或 README 依据。
- **审查要点：** 复核 `STATUS.md` 的 OD-ARCH-009 段与本节是否与源码一致；确认 `ENDPOINT_ISOLATION_CONFIRMED` 仍为 `false`；确认 `git diff -- runtime/` 为空；确认主工作区未提交改动未被覆盖。审计文本不构成用途、服务商、设备或环境授权。

## OD-ARCH-008：端点准入关闭时的引擎启动副作用复核（2026-10-18）

**结论：计划范围内通过；完整干净构建未能独立重跑，记录为验证限制。** 复核对象为 BUILDER worktree /mnt/data/Code/omniharness-ui，HEAD 5ffc710。主工作区和 BUILDER worktree 均有大量不同的未提交改动，未合并、提交或清理。

- **代码与界面：** engine_describe 在 ensure_initialized() 前调用与提交/目录准备共用的宿主准入；拒绝时不启动引擎，也不返回伪造能力。顶栏显示未启动状态；技术面板分别处理描述拒绝和只读健康快照。窗口证据显示端点未确认、引擎未启动。
- **独立运行验证：** 复跑现有定向 Rust 测试二进制，4/4 通过；复跑当前已构建的 Rust 测试二进制集合，79/79 通过；tsc --noEmit 通过；git diff --check HEAD 通过，runtime/ 无差异。测试二进制与本轮源码时间戳一致，但不是从空构建目录重新编译。
- **未能重跑：** 为保持独立构建目录，尝试完整离线 Cargo 编译，因 /tmp 用户磁盘配额耗尽失败；项目既有 Cargo target 为只读，不能在原位置补编译。npm run build 中 TypeScript 检查已通过，但 Vite 因 node_modules/.vite-temp 只读而以 EROFS 退出。BUILDER 报告记录的完整 Cargo 测试、前端构建、准入/启动器/端口检查、桌面 E2E 均通过，证据见 /mnt/data/Code/omniharness-ui/tests/verification/desktop/evidence/od-arch-008/README.md；本轮没有独立重跑 E2E。
- **小问题：** 技术面板错误仍显示 Error endpoint_unconfirmed: 一类内部错误前缀，原因完整但对用户不够友好；不影响安全状态判断，列作后续 UI 文案改进候选。
- **边界：** 端点隔离仍未确认，准入常量继续固定关闭；真实模型、跨平台行为未验证。未读取凭证或真实业务材料，未触碰 Runtime 或用户数据。

**后续：** 进入 OD-ARCH-009 规划：只审计端点准入所需的可信证据与最小宿主控制，输出可执行的 go/no-go 设计；本计划不开放准入、不调用模型。详见 [PLANS.md](PLANS.md)。

## OD-ARCH-007：工作目录准备命令宿主准入复核（2026-10-18）

**结论：计划范围内通过。** 复核对象为 OpenCode 工作区 `/mnt/data/Code/omniharness-ui`，基线 HEAD `5ffc710`。主工作区和 OpenCode 工作区均有各自未提交改动；没有合并、提交或清理。

- **代码检查：** `task_prepare_workspace_dir` 先调用宿主准入，再调用会创建目录的 `prepare_local_data_root()`；拒绝返回结构化 `endpoint_unconfirmed`，不会返回可用路径。无普通环境变量或 UI 参数旁路。前端拒绝时不回退显示可用目录。
- **独立测试：** 使用项目用户级 Rust 工具链和独立临时构建目录运行 `cargo test --offline --test host_workspace_prepare_admission`，3/3 通过。覆盖拒绝后目标不存在、未启动引擎、任务数据未变化；对照用例证明无闸门时同一路径确会创建。`git diff --check HEAD` 通过，`runtime/` 无差异。
- **其余执行证据：** BUILDER 记录 `cargo test --offline` 74/74、TypeScript 检查、前端构建、准入/启动器/端口检查及桌面 E2E 通过，证据见 `/mnt/data/Code/omniharness-ui/tests/verification/desktop/evidence/od-arch-007/`。本次独立复核没有重跑这些整套检查。
- **未验证：** 真实模型端点、端点隔离许可、跨平台行为。`engine_describe` 仍在应用打开时初始化并拉起引擎，这是现存生命周期行为，不属于 OD-ARCH-007 修复范围，需单独处理。
- **边界：** 未接触凭证、真实业务材料、Runtime 或用户数据；两工作区均保留原状。

**后续：** OD-ARCH-007 已关闭。OD-ARCH-008 计划审查并封闭端点准入关闭时 `engine_describe` 的启动副作用，见 [PLANS.md](PLANS.md)。

## OD-ARCH-006：宿主拒绝与开发隔离独立复核（2026-10-18）

**结论：部分通过，暂不整体验收。** 复核对象为 OpenCode 工作区 `/mnt/data/Code/omniharness-ui`，HEAD `5ffc710`；只检查了本轮相关代码、OD-ARCH-006 证据及共享状态。主工作区和 OpenCode 工作区都有各自未提交改动，文件内容不同；未合并、提交或清理。

- **通过项：** Rust 的 `engine_submit_task` 经过宿主 fail-closed 闸门；端点许可常量为关闭状态，拒绝早于引擎初始化。记录的 Cargo 全量测试 70/70、前端构建、准入检查 11/11、端口归属/进程回收和桌面 E2E 均通过。证据见 OpenCode 工作区 `tests/verification/desktop/evidence/od-arch-006/README.md`。这些是合成/隔离验证，不证明真实模型输出或端点已获许可。
- **阻断项（中）：** `task_prepare_workspace_dir` 没有经过宿主准入。源码直接调用 `task_shell::prepare_local_data_root()`，其中会创建本地数据目录；直接调用该 Tauri 命令可绕过 UI 禁用状态并造成文件系统副作用。前端 `workbench.tsx` 虽然先检查准入，但这不能保护宿主命令本身。该缺口不等于已向模型发送输入，但不符合 OD-ARCH-006 对宿主边界和拒绝副作用的要求。
- **回归范围：** 已查阅报告列出的测试结果；本次审查没有再次运行整套测试。`cargo fmt --check` 因环境未安装 `rustfmt` 未能运行，属于未验证。端点隔离、真实模型输出及跨平台行为仍未验证。
- **工作区边界：** 主工作区和 OpenCode worktree HEAD 均为 `5ffc710`，但各有不同的未提交改动；本审查只记录差异，不把任何一侧工作区整体视为可合并补丁。`runtime/` 未发现本轮改动。

**下一步：** OD-ARCH-007 只修复 `task_prepare_workspace_dir` 的宿主准入与无副作用拒绝测试；计划见 [PLANS.md](PLANS.md)。完成后重新审查实际代码及定向回归，再判断 OD-ARCH-006 是否可通过。

## OD-ARCH-005 与 OD-UI-002：独立复核（2026-10-18）

**结论：两项在各自计划范围内通过；整个混合工作区不予合并或整体验收。** 复核对象是 `/mnt/data/Code/omniharness-ui` 的 `feat/desktop-ui` 工作区（HEAD `5ffc710`），不是主工作区快照。

- **OD-ARCH-005：范围内通过。** 静态检查确认所有两个 UI 提交点都经过 `api.ts` 的准入检查；技术切片已改为只读。独立运行 `python3 tests/verification/desktop/verify_engine_admission.py`：11/11 通过；`bash tests/verification/desktop/verify_launcher_identity.sh`：命令行不符、启动时刻不符时均未发信号，身份一致时停止成功；`git diff --check HEAD` 通过。代码与证据见该 worktree 的 `desktop/src/engineAdmission.ts`、`desktop/src/api.ts`、`desktop/src/shell/EngineTechSlice.tsx`、`scripts/dev-ui.sh` 和 `tests/verification/desktop/evidence/od-arch-005/README.md`。
- **重要限制：** 当前准入闸门只在前端，Rust 的 `engine_submit_task` 命令仍可直接到达引擎；端点隔离事实也仍未确认。因此这是“正常界面关闭提交”的通过，不是宿主强制安全隔离或真实材料使用许可。未使用真实模型或凭证。
- **OD-UI-002：范围内通过。** 已检查五阶段工作台、窄窗口和首页对照截图；页面布局、中文可读性、禁用提示和局部样式边界符合计划。证据记录称首页与改动前截图一致；构建、截图和合成样例结果见该 worktree 的 `tests/verification/desktop/evidence/od-ui-002/README.md`。跨平台、低于最小窗口宽度和真实模型输出未验证。
- **工作区边界：** 主工作区与 OpenCode worktree 的 HEAD 都是 `5ffc710`，但两边均有未提交差异，且改动不相同：主工作区 50 项状态记录，OpenCode worktree 60 项（以 `git status --short` 行数计）。多个文件改动相互重叠，OpenCode 同时包含引擎、UI、安全修正和样式；不得把其整个工作区当成单一补丁。两边 `runtime/` 均无差异；未提交、推送、合并或清理任何改动。
- **待处理：** OpenCode 工作区报告记录过多 worktree 共用 `127.0.0.1:5273` 时，桌面应用可能加载另一 worktree 的前端；尚未修复。其 Rust 测试及前端构建结果来自执行记录，本轮未在该工作区独立重跑；不以主工作区测试替代。

**后续边界：** 现有 UI 不得被描述为端点隔离或可接收真实材料。宿主侧拒绝提交、开发端口冲突防护与过时架构文档校准，拟列入 `PLANS.md` 的 OD-ARCH-006，待用户批准。

## OD-ARCH-004 与 OpenCode 工作台样式：独立复核（2026-10-18）

**结论：OD-ARCH-004 部分通过，整个 UI 暂不验收；OpenCode 样式改动范围合理，但视觉验证未完成。不要合并混合工作区分支。**

- **OD-ARCH-004 已核实部分：** 首页公开/合成输入说明清楚，端点未确认时提交按钮禁用且提交函数再次拦截；开发启动器不接受 `--real`，显式设置两套 OmniDesk 与 DSH 数据目录变量，并拒绝复用不属于本工作区的 5273 服务。执行记录和首页截图见 `tests/verification/desktop/evidence/od-arch-004/README.md`、`home-preview.png`。
- **阻断问题（高）：** 设置里的「引擎技术切片」仍有可用的自由文本框和「提交任务」按钮，直接调用 `host.submitTask`，没有与首页相同的端点准入门槛。这仍可将任意文本发送到未核实端点，绕过 OD-ARCH-004 的输入保护。此入口应默认不能提交，直到宿主能提供可核实的准入事实；仅靠“纯合成任务”的占位文字不足以约束输入。
- **启动器差异：** PID 记录包含命令行，但停止时实际只比较 PID 与 `/proc` 启动时刻，没有核对当前命令行是否仍与记录一致。启动器报告声称的身份检查比代码实际更强，后续应校准检查或报告；在修复前不把“命令行身份已校验”视为代码事实。
- **OpenCode 样式：** `workbench-ui.css` 的规则以 `.wb-v2` 为根作用域，`workbench.tsx` 只增加局部 class 与样式 import；未发现它改变五阶段业务逻辑或 IPC。仍缺工作台页面的实际截图/视觉验收。该 worktree 的 `npm run build` 因 `node_modules/.vite-temp` 所在目录只读而报 `EROFS`，这是环境限制，不能据此判为代码构建失败；也不能用主工作区构建替代。`cargo` 当前未在该环境 PATH 中，未能复跑 Rust 测试。
- **分支边界：** `/mnt/data/Code/omniharness-ui` 的 `feat/desktop-ui` 同时包含 OD-BUILD-005、OD-ARCH-004、UI v2 主壳与样式；主工作区也有既存未提交改动。不得把整条分支作为单一 UI 补丁合并。所有改动保持未提交、未推送；本轮 `git diff --check HEAD` 通过，`runtime/` 无差异。

**验收裁定：** OD-ARCH-004 对首页和启动器的修复有代码/报告支持，但仍有技术切片自由文本入口绕过，且命令行身份校验表述不实；安全边界整体暂不通过。OpenCode 样式静态边界通过，实际视觉效果待截图检查。下一步分别见 PLANS.md 的 OD-ARCH-005 与 OD-UI-002；二者均待用户批准。

## OmniDesk UI v2 与 OD-BUILD-005：独立审查（2026-10-18）

**结论：暂不验收，不建议提交或合并。** UI v2 的主壳设计诚实地区分了已实现和未实现入口，前端仍经 IPC-1；但通用对话会把任意输入送入模型，且没有用途/数据分区提示。该缺口与当前授权边界不相容。开发启动脚本还有误杀其他进程及修改 Runtime 的可能。OD-BUILD-005 的实现与 UI v2 同处 feat/desktop-ui，须分开审查，不能把 UI 视觉完成视为引擎纵向流程已验收。

- **UI v2（文件事实 + 静态检查）：** 新增独立主壳、首页、日历空态和设置入口；未实现功能禁用或标明数据源未接入；未发现前端直接依赖 ACP/DSH 私有概念。_design/ 中包含原始导出和多张图片；吉祥物来源交接文档称为用户提供素材，尚未独立核实许可与是否应入库。UI 分支未提交，分支 feat/desktop-ui 与主工作区 HEAD 均为 5ffc710；主工作区的既存差异及 UI 分支全部差异均保留。
- **阻断问题 1 — 输入准入边界（高）：** HomeChat.tsx 将任意文本直接送至引擎，共用一个本地工作目录；页面只写“本地 DSH 引擎”，没有任务用途选择、数据类别提示或模型/运行环境准入确认。代码并未提供真实分区隔离。当前答复仅涵盖特定脱敏条件下的个人数据分析与生态研判，不能推定一般对话、写作或项目管理已获准。合成端点验证不构成真实材料使用授权。此实现不得用来处理真实工作材料，须先补明确准入约束与边界。
- **阻断问题 2 — 启动脚本的进程和仓库边界（高）：** dev-ui.sh 把 PID 写入固定共享文件，启动时先截断旧文件，--stop 随后按 PID 直接 kill，未验证进程归属；另在 5273 端口已有服务时不确认服务来自哪个 worktree；DSH 运行时缺失时还可能在 Runtime 下创建软链接（第 60–65 行）。脚本本轮未运行；根据 AGENTS 禁止修改 Runtime 的规则，不应按当前版本运行。应改成安全的仅本地启动器，失败时清楚退出，不触碰 Runtime、不误停别的实例，也不接受无边界的 --real 模式。
- **需修复问题 3 — 轮询错误被吞掉（中）：** 首页对话在状态轮询或结果读取失败时静默忽略（HomeChat.tsx 第 100–140 行）。任务可能一直显示“生成中”，或终态有状态却无结果，用户得不到可操作错误。应采用有界重试/超时并显示失败原因，不能把未知状态写成成功。
- **OD-BUILD-005 引擎链路：** 主工作区独立复跑 cargo test --offline 为 **64 passed / 0 failed**，npm run build 通过，工具链检查为 RESULT: ready；git diff --check HEAD 通过，Runtime 无差异。状态文件还记录合成端点 E2E 与窗口流程证据，但本轮未独立重跑这些脚本。静态检查确认事件接线使用宿主 IPC，且没有扩展真实任务类型。事件泵锁饥饿修复已加 2 秒健康探测回归。
- **验证限制：** 对 UI worktree 的 npm run build 复跑因其 node_modules 所在文件系统只读（EROFS）失败，这是环境写入限制，不是代码失败；主工作区构建通过只能证明主工作区快照。未运行 Tauri UI 启动脚本、未启动桌面窗口、未连接真实模型，也未处理任何真实材料。报告中已有的截图和 E2E 证据由 BUILDER 记录，本轮没有独立重取。
- **剩余 UI 决策：** 本地数据分析与新主壳视觉不统一；对话历史不持久化；首页日程和重点事项目前为空态。它们是体验/产品缺口，不是本轮安全阻断项。_design 与素材许可需用户决定后再纳入版本控制。

**验收裁定：** OD-BUILD-005 的引擎代码和静态/自动化检查总体可接受，但因全流程独立复跑证据不完整，且当前对话 UI 缺少准入边界，整体验收暂缓。UI v2 仅通过“主壳诚实性与 IPC 边界”审查，不代表产品可接收真实材料。下一步修复范围与验收条件见 PLANS.md 的 OD-ARCH-004。

## OD-BUILD-004：引擎 Profile 默认链与错误可读性独立审查（2026-10-16）

**结论：通过（计划范围）。** 两项缺口均有直接代码证据、我的独立测试复跑与隔离运行证据支持；OD-BUILD-003 遗留的两项缺口就此闭合；cwd 修复保持通过；未引入 DSH 私有格式依赖；runtime/ 无差异。审查基线与结束时一致：HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`，分支 `main`，唯一标签 `discipline-v0.2`；工作区 37 项差异全部保留，未提交、未推送、未打标签。本轮只修改本审查文件与共享状态入口，未改动 BUILDER 的 STATUS.md 执行记录与任何业务代码。

- **缺口 1（宿主默认配置链即 ACP）：通过。** 源码：`desktop/src-tauri/src/engine_supervisor.rs` 的 `from_env()` 默认 Profile 由 `yvan-work` 改为 `acp`；新单测 `engine_config_default_profile_chain_is_acp` 先清空 `OMNIDESK_DSH_PROFILE`（即真的“宿主不设置该变量”），再直接调用 `EngineConfig::from_env()`，断言 `profile`、`child_env()` 与 `initialize_params()["dsh"]["profile"]` 三者一致为 `acp` 并通过 `validate()`；另验证显式覆盖生效、纯空白覆盖回退 `acp`。测试用 `OWNED_ENV_KEYS` 快照/还原，不触碰 `HOME`/`OMNIDESK_REPO_ROOT`/`OMNIDESK_LOCAL_DATA_ROOT`，可与并行测试共存。**我的独立复跑确认该测试真实执行并通过**（第 28 行 `test engine_supervisor::tests::engine_config_default_profile_chain_is_acp ... ok`）。缺口达成，且不再依赖脚本手工注入。证据等级：静态检查 + 脚本运行。
- **缺口 2（非 ACP Profile 的错误可读、有界、可区分）：通过。** 源码：`engine/omnidesk_engine/acp_adapter.py` 新增 `_profile_phrase()`/`_profile_detail()`，只在 `_request()` 的 TIMEOUT 分支（进程活着但不应答，最典型即非 ACP Profile）追加所配 Profile 与“预期讲 ACP”的可读说明，结构化字段落在 `detail`；`for_dsh()` 用 `kwargs.setdefault("engine_profile", profile)` 自动带上 Profile 名。`serve.py` 未改、错误码未改、30 s 有界等待**未缩短**，也未读取 DSH Profile 私有 bundle 清单。**我的独立复跑 R2**：11/11 检查项通过——`detail.engine_profile == "web"`、`expected_protocol == "acp"`、`leftover_engine_processes == []`、`elapsed ≈ 30.1 s`；未知 Profile 返回 `engine_unavailable` 且保留 DSH 原文，与 web Profile 的 `timeout` 可区分。证据等级：脚本运行（隔离一次性数据根 + 真实引擎子进程）。
- **回归（独立复跑）：** `cargo test --offline` → **61 passed / 0 failed**（lib 45 含新单测、host_integration 13、task_shell_e2e 3）；`cd desktop && npm run build` → 通过（22 modules，245.68 kB JS / 7.48 kB CSS）；Profile 场景 `R1`/`R1B`/`R3`/`R5` 全部 pass；`git diff --stat -- runtime/` 为空；`git diff --check HEAD` 本轮返回 0（早前记录过的 STATUS.md 末尾空行当前不可复现，如再出现按低优先级处理）。
- **未验证（如实保留）：** 本轮未运行任何真实模型端点（引擎链路仍为本机回环合成端点）；未做真实 Tauri 窗口人工验收；跨平台未验证；`/proc` 进程检测与 realpath 语义仅本机 Linux 实测。
- **边界：** 未修改 `runtime/`、业务代码、DSH 数据、模型路由或凭证；未读取、打印或复制任何凭证或真实业务材料；用户正在运行的原版 DSH Desktop 与隔离实例未被触碰。

### 对 BUILDER 待决问题的裁决

关于“是否为已存在但非 ACP 的 Profile 单列启动前预检，以消除约 30 s 等待”：**不作为当前验收阻断项，登记为低优先级后续项。** 理由：该路径只在配置错误场景触发；错误已可读、可区分、有界，且无残留引擎进程；正常 ACP 启动不受影响。而预检需要读取 DSH Profile 私有 bundle 清单（如是否含 `dsh-acp-app`），与 D-002“DSH 是可替换引擎、只经最小适配边界依赖”相悖。若日后真实使用中该等待造成可观察困扰，再依据使用证据评估，并优先考虑不读取私有结构的替代方案（例如缩短初始化等待并保留重试）。

## OD-BUILD-003：Profile 默认值与 cwd 修复独立审查（2026-10-16）

**结论：部分通过，暂不验收。** cwd 规范化修复有独立运行证据；宿主默认 Profile 的端到端证据缺失，且已存在的非 ACP Profile 仍只以长时间通用超时失败。审查基线 HEAD 为 5ffc7101e90dcf3a2047f214b71286d3a32208a8；结束时既有工作区改动均保留，未提交、未推送，runtime/ 无差异。

- **cwd 修复：通过。** 静态检查确认 canonical_cwd 仅在 submit_task 与 recover_session 两处入口调用；R3 证明别名路径和真实路径归到同一会话目录、会话可恢复；R5 证明不可解析路径以 invalid_argument 拒绝且不创建会话目录。
- **默认 Profile：代码正确，验收证据不足。** EngineConfig::from_env() 默认写为 acp，child_env() 与 initialize_params() 会传出 profile。但 R1 测试直接给引擎环境注入 OMNIDESK_DSH_PROFILE=acp，并显式传 profile=acp；R1B 测的是 Python 引擎自身默认值。它们都没有实际验证 Rust 宿主从默认配置构造到启动传递的路径。因此不能据此判定原验收项通过。
- **非 ACP Profile 错误：未通过原验收。** R2 中不存在的 Profile 会返回带 Profile 名的 engine_unavailable；已存在的 web Profile 等待约 30 秒后返回通用 timeout。它不是明确拒绝，也没有清晰说明 Profile 与 ACP 不兼容。
- **其他检查：** 本轮独立复跑 Rust 测试 61 项通过；Python 编译检查通过；desktop npm run build 通过；E20 再跑通过，快照前后工作区状态相同且 runtime/ 无改动。E20 观察到 ~/.dsh 有第三方写入，证据归因于运行中的原版 DSH Desktop，不是本轮测试痕迹。git diff --check HEAD 仍报告 STATUS.md 文件末尾有多余空行。
- **之后刷新 E20 证据的补充：** 当前工作区中的 E20 记录仍未发现本项目测试自身写入 `~/.dsh`、`~/.agents`、`~/OmniDesk` 或 `~/YvanAI` 的痕迹，但空闲对照阶段观察到原版 DSH 在 `~/.dsh` 的并发写入；报告另保留了测试临时目录。此证据不能表述为这些目录在测试期间完全无变化；旧临时目录未经清理并继续保留。
- **边界：** 未修改业务代码或测试；未读凭证、真实材料或会话正文；未动用户数据。旧 R2 临时目录 /tmp/omni_p1s3_r2_27ahg2bu 仍存在，本审查未删除。
- **下一步要求：** OD-BUILD-004（见 PLANS.md）直接测试 Rust 宿主默认配置传递，并为错误 Profile 提供及时、可读的 ACP 兼容性错误；由用户批准后交由其指定的 BUILDER 实施。OD-BUILD-003 在补齐前不标记通过，完整 MVP 仍未验收。

## OD-ARCH-003：v0.4 架构校准与会话丢失机制审计（2026-10-16）

**执行状态：已执行、已完成（只读审计 + 隔离一次性合成验证）。** 本回合未修改任何业务代码，未新增日志或界面。结论按证据等级标注：文件事实 / 静态检查 / 脚本运行 / 未验证。审查基线与本回合结束时一致：HEAD `5ffc7101e90dcf3a2047f214b71286d3a32208a8`，分支 `main`，remote `git@github.com:klzone/omniharness.git`，唯一标签 `discipline-v0.2`；保留既有 5 项未提交改动（GOALS.md、PLANS.md、README.md、REVIEW.md、docs/decisions.md），未提交、未推送、未打标签。

### 结论摘要

1. **“会话丢失”不是单一故障，而是六类可区分的机制问题。** 只有其中一类（进程中断后轮次状态未知）在原生层确实留下“已开始但无终态”的记录；其余多为可见性、路径身份或配置数据根问题。逐类证据见下。
2. **原生持久化在本回合的隔离合成条件下足够可靠：历史可读、可列出、可 resume、恢复后可持续写入。** 中断的一轮不会被原生层续跑，但会被显式收束（补写 `step/end`、`turn/end`、`session/end-seed`）。**resume ≠ 未完成任务续跑**，与 D-012 一致。
3. **真正需要 OmniDesk 处理的两处缺口是“可见性”和“路径身份”，都不需要复制对话正文。** 因此**独立逐消息追加日志维持 no-go（暂缓）**，采用条件见第 5 节。
4. **v0.4 的“DSH 内部状态可丢弃”在锁定版本上不成立。** 锁定版本的原生会话格式 V4 已是 **已发布** 格式（`latestReleasedVersion: 4`，`evidenceTag: dsh-v0.2.0-rc.2`）；该版本文档明确写明“alpha/beta/rc 产品发布即建立已发布格式义务，GitHub 的 prerelease 标记并不会让持久化用户数据变成可丢弃”。证据等级：文件事实。
5. **发现的两处现存实现问题只登记、不在本计划修复**（Finding A、Finding B，见第 6 节）。ARCHITECT 只提出下一项 Builder 计划，等待用户批准。

### 1. 锁定版本与运行入口（文件事实）

- `runtime/RUNTIME.lock`：DSH 标签 `dsh-v0.2.1-alpha.2`，commit `d743267388641bc76f17c45ce8b4c231aed1d32c`，npm `@deepseek-ai/dsh` `0.2.1-alpha.2`，node engine `^22.19.0 || >=24.0.0`。
- 子模块 `runtime/deepseek-harness` 与该 commit 一致且干净，**无 `node_modules`**，其自带 vitest 套件无法运行。
- 可运行 CLI：`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js`，本机 node `v22.23.2`。
- 本回合以锁定子模块源码与隔离一次性运行作为事实来源，不以最新官方文档替代锁定版本能力证明；单独 `/home/kevin/Code/DSH-Desktop` 仓库不作为锁定版本证据。

### 2. 数据根与存储位置（文件事实）

- **数据根覆盖机制**：`DSH_HOME`（优先级：显式配置 > `$DSH_HOME` > `~/.dsh`）与 `DSH_AGENTS_HOME`（`~/.agents`）。`resolveRoot()` 把纯空白视为未设置，`expandHomePath` 支持 `~`。来源 `packages/util/home-paths/src/index.ts`。
- **唯一重启安全的隔离方式是继承进程环境**：`packages/boot/app-boot/src/index.ts` 的 `BOOTSTRAP_PREFIXES = ['DSH_', 'XDG_', 'DYLD_', 'BASH_FUNC_']`，任何 `.env` 文件都不得设置这些名字。
- **各内建存储的实际落点**（`packages/bundle/base/cordis.patch.yml`）：
  - 会话日志根 `sessions` = `dshHomePath('sessions')`。
  - `storage-json` 根 `storages` = `dshHomePath('storages')`。
  - `session-query-sqlite`：`path: ':memory:'`、`openAt: never` → **SQLite 永不打开**；Web 侧栏搜索只匹配标题与工作区名。
  - `session-projection-cache`：节流写回（`writeEveryEvents: 200`、`writeIntervalMs: 5000`）写入 `session_projcache`。
  - 基础 `system-prompt` 的 `personaPrefix: ''`。

### 3. 会话存储格式与崩溃恢复（文件事实 + 脚本运行）

- **项目目录分组使用原始 cwd 字符串，而非规范路径**：`packages/session/session-persistence-jsonl/src/format.ts` 的 `projectKey(cwd)` 把分隔符替换为 `-`、转义不安全码单元为 `~XXXX`、去掉前导短线、截断到 251 字符 → `--<slug>--`；`projectDir` 在 `cwd === undefined` 时用 `root/_no-cwd`；工件名 `session.vN.jsonl.zstd`（默认 zstd）。
- **撕裂尾部/崩溃恢复**：已提交前缀永不重写；恢复中的读取器补写合成的中断轮次收束记录——**只收束轮次，不续跑未完成轮次**。不完整的原始末行被丢弃；撕裂的 zstd 帧只保留完整可解码记录；完整已提交帧内的损坏按损坏拒绝。
- **每会话单写者**：进程内 claim + 内核 `flock(2)` 于 `session.lock`。`list()` 与 `stat()` **只读 header**。
- **同一根用不同压缩打开会被拒**：`rejectOppositeArtifact` → `encodingMismatch`（“use a separate root or select the matching compression mode”）。`legacyLayout` 拒绝扁平文件布局。
- **同一 id 出现在多个项目目录时报错**：`findLog` 抛 `duplicate JSONL session id "<id>" appears in multiple project directories`。
- **本回合隔离一次性合成验证（脚本运行）**：在 `/tmp` 一次性数据根中用锁定 CLI + 本机回环合成端点（零凭证、零外网）提交一次写作任务并令其挂起，随后 `SIGKILL` 引擎进程。
  - **resume 之前**读取原始日志：**14 条记录**，类型依次为 `session, permission/preset, sandbox/mode, approval/policy, agent/inbox/spliced, turn/start, agent/inbox/spliced, step/start, system/message, user/message, user/message, request/header, request/context, session/title`，末条为 `session/title`；**无 `step/end`、无 `turn/end`、无 assistant 消息**。
  - 杀死时刻 OmniDesk 落盘任务 JSON：`state: "starting"`，`has_content: false`，`content_len: 0`。
  - 杀后同进程 `recover_task` → 进程内记录仍 `running`，`replayed: false`；**全新进程** `recover_task` → `unknown`（note：会话仍可 resume，但不代表该轮有终态证据），`replayed: false`；原始日志在全新进程启动时仍为 14 条。
  - 全新进程 `recover_session` 成功打开会话；**resume 之后**日志变为 **17 条**，新增恰为 `synthesized_on_resume: ["step/end", "turn/end", "session/end-seed"]`。
  - **结论**：收束记录可证明是在 resume 时刻生成，而非杀死时刻；中断轮次不留下 assistant 文本，恢复只收束轮次、不续跑。
- **既有仓库证据（引用，不重跑）**：`tests/verification/engine-interface/evidence/adapter/E01–E20.json` 全部 `evidence_level: "script"`、`executed: true`；E07 客户端 SIGKILL → recover `unknown`、`replayed:false`、从不 `succeeded`；E08 优雅关闭 → `recover_session` 恢复既有会话、不重放；E10 中断 → `unknown`、不自动重放、不编造输出、`get_task_result` 抛 `unknown_task`；E13 落盘失败 → `result_persist_failed`、保留普通结果、`persisted_at: null`、不重跑；E20 隔离元场景（`~/.dsh` 无自身痕迹、`runtime_not_modified`）。

### 4. 会话索引、工作区身份与 ACP 可见性（文件事实 + 脚本运行）

- **ACP 面**（`packages/acp/acp/src/index.ts`）：方法 `initialize, authenticate, session/new, session/list, session/resume, session/close, session/setConfigOption, session/prompt`；通知 `session/cancel`。
  - `listSessions`：先 `persistence.list()`，再**过滤掉**正在活跃/激活中（`sessions.has`、`activating.has`、`ctx.sessions.get`）、`origin==='subagent'`、有 `parentSession`、`cwd===undefined` 或非绝对路径的会话；可选 `cwd` 过滤走 `sameDirectory`；按 `createdAt` 倒序分页。
  - `resumeSession`：拒绝 `session is already active`；header 为 subagent/parentSession 时拒绝 `session is not resumable`；**要求 `sameDirectory(persisted.cwd, params.cwd)`**，否则 `session cwd does not match`。
  - `sameDirectory`：两侧 `realpath`；路径不可解析时退回 `resolve()` 字符串相等。
  - `validateWorkspaceParams`：要求绝对 cwd，拒绝 `additionalDirectories`。
  - `session/new`：激活后调用 `ctx.sessions.flush(...)`——"The attached log writer's flush materializes an empty session durably."，即 `create()` 在 `session/new` flush 前不写任何东西。
- **工作区注册表**（`packages/workspace/workspace/` + `src/paths.ts`）：经 `storage-json` 持久化，domain `workspace`、version 2（`workspaces` 表 + 全局 `workspaceIds`、`archivedSessionIds`、`pinnedSessionIds`、可选 `defaultWorkspaceId`、`pendingMutation`）。**唯一性 canon 是 `fs.realpath`；路径按规范化存储。** “Membership is ownership plus a live cwd fact”：记录内有序 `sessionIds` 是归属真相；读取时过滤、下次变更时持久化裁剪。只读 header。
  - `bootstrap()` 按**规范化 realpath 后的 cwd** 分组存量 header 并重建 `sessionIds`。
  - `reportFilteredCandidates()` 记录 `workspace '<id>' filtered session '<sid>' from membership: <reason>`，reason 为 `header has no cwd` / `cwd '…' is not a directory` / `cwd '…' does not resolve` / `canonical cwd '<path>' differs from workspace path '<record.path>'` / `session header is missing`。
  - 移除项目从不删除目录/文件/历史。
- **关键不对称**：会话**存储**按**原始 cwd 字符串**分组；**工作区注册表**与 **ACP list/resume** 按 **realpath** 分组。同一物理目录的两种写法 → 两个存储项目目录，但一个工作区身份。
- **本回合文件事实证据**：隔离运行根的 `storages/workspace.json` 中两个工作区各只列 `sessionIds` 的**子集**（例如 `/mnt/data/Code/MyWorkStation` 只列 2 条，而磁盘 `--mnt-data-Code-MyWorkStation--` 下有 6 个会话）→ **工作区 `sessionIds` 是精选子集，不是磁盘全量列表**。这直接解释“会话在磁盘上、但界面里看不到”。
- **既有仓库证据（引用，不重跑）**：`evidence/channel-probe.json`——headless 无 cancel 标志；sdk `session/cancel` → `-32603 unknown method`；ACP 通告 `sessionCapabilities {close,list,resume}`、支持 `session/cancel` 通知、close-then-resume OK、未知 id → `-32602 session is not resumable`；`C:acp-crash-recovery`：`listed_after_crash: true`、`resume_after_crash_ok: true`，note “这证明状态可发现与可重新接管，但不等于原任务被自动续跑”。`evidence/headless-recovery.json`：同 cwd 接管 OK（`turn_numbers_seen:[2]`）；错误 cwd → `was recorded in "<A>", not "<B>"`；未知 id → `does not exist`；`concurrent_ownership` **未验证**。

### 5. “会话丢失”六类归因与 go/no-go

按现有证据可区分的六类（每类注明已证实/未证实）：

| 类别 | 现象 | 现有证据可证实的 | 仍缺的证据 |
| --- | --- | --- | --- |
| (a) 原始文件缺失/损坏 | 历史彻底消失 | 原生撕裂尾部/已提交帧规则 + 本回合杀前杀后原始日志一致（14 条，工件完好、可列出、可 resume、备份恢复后可持续）【脚本运行】 | 磁盘损坏、误删、跨版本格式不兼容等极端情形的复现 |
| (b) 列表/项目索引不可见 | 磁盘有、界面无 | ACP `listSessions` 过滤规则（含**过滤掉活跃会话**）+ 工作区 `sessionIds` 为子集【文件事实】 | 用户实际界面故障与其运行数据根的逐条对应 |
| (c) 工作目录身份不匹配 | 同一目录两种写法分裂 | 存储按原始 cwd 字符串、注册表/ACP 按 realpath；跨写法 resume 成功但分组分裂；错误目录被拒【静态检查 + 脚本运行】 | 用户机器上实际触发路径 |
| (d) profile/数据根变化 | 换配置后会话看不见 | `DSH_HOME` 优先级；`yvan-work`（Web）与 `acp`（ACP）不同 bundle；dev 脚本硬失败；原版与隔离会话集不同【文件事实】 | 用户具体一次故障的配置快照 |
| (e) 界面无法重新打开 | 会话在但点不开 | ACP list/resume 可用；Web 侧栏搜索只匹配标题/工作区名（SQLite 未打开）【文件事实】 | 具体 UI 故障复现 |
| (f) 进程中断后轮次状态未知 | “刚才那轮”状态不明 | E07/E10/E13 + `C:acp-crash-recovery` + 本回合杀前杀后 14→17 条与 synthesized 收束【脚本运行】 | 无（该子命题本回合已证） |

(f) 明确：**resume ≠ 续跑未完成任务**，收束记录可证明于 resume 时刻生成。

**独立逐消息追加日志：维持 no-go（暂缓）。** 四方案比较（可恢复范围 / 丢失窗口 / 格式兼容 / 重复留存与清理责任 / 数据暴露 / 复杂度 / 采用条件）：

- DSH 原生持久化：覆盖历史可读、可列出、可 resume、恢复后可持续写入（本回合脚本运行 + 引用证据）。缺点：并发写、格式随版本演进。
- 原生 + 可恢复备份：在原生之上加副本兜底；备份脚本 `scripts/backup-sessions.sh` 现指向原版 `~/.dsh` 而非隔离 OmniDesk 根，且**恢复从未演练**（文件事实；恢复可用性未验证）。
- OmniDesk 项目/会话元数据索引：解决 (b)(c) 可见性与身份，无需复制正文。
- 复制 ACP 事件或正文的追加日志：会形成**第二套敏感正文存储**，承担独立的备份与删除责任并扩大泄露面；在证明原生 + 备份 + 索引无法满足目标前不采用。

**采用条件**：仅当稳定复现的故障显示原生持久化 + 备份 + 元数据索引仍达不到目标（历史可读 / 恢复既有 session / 确认中断状态 / 断点续跑），且明确需复制正文时才重新评估；触发评估不等于自动采用。

### 6. 只登记、不修复的两项发现

**Finding A — 默认 Profile 不匹配（P1，静态检查）。** `desktop/src-tauri/src/engine_supervisor.rs:88` 默认 `OMNIDESK_DSH_PROFILE=yvan-work`；`profiles/yvan-work/package.json` 打包 `dsh-base + dsh-web-app + dsh-experimental-inspector-profile` → 启动的是 **Web UI，而非 ACP**。只有隔离 `~/OmniDesk/runtime/dsh-home/profiles/acp/`（`dsh-base + dsh-acp-app`）说 ACP。变通：`export OMNIDESK_DSH_PROFILE=acp`。注意隔离 `yvan-work` 的 `package.json` `name` 是 `dsh-profile-desktop`。**本回合不修复。**

**Finding B — 分析功能对人不可用（P1，静态检查）。** `desktop/src/workbench.tsx`：无文件选择器（仅粘贴或 3 个内建合成样例 `loadSample`，须为 `类别,数值[,说明]` CSV）；无“分析目标/要求”字段（固定统计管线）；结果在应用内以状态卡 + `<table class="stat-table">` 呈现；保存需 `canSave = !!analysis && confirmed && explanation.trim().length>0`；保存来源下拉只有 `user` 与 `model_unverified`；`save_artifact` 用 `create_new(true)` 拒绝覆盖；`usage.jsonl` 仅枚举/布尔。红线扫描在 `task_shell.rs:222`，`RED_LINE_KEYWORDS` 在 `:154`。**本回合不修复。**

这与用户“数据分析在哪里选数据、下指令、设目标、看结果？”的反馈一致：当前切片只有固定统计管线，没有面向人的输入/目标/结果呈现。

### 7. v0.4 产品与合规边界校准

- **两层定位可采纳为方向**：通用个人工作台底层（会话可找回、升级可控、数据自主）+ 可选 Discipline 能力包层。**但合规政策必须绑定下层**：通用底层同样接触会话正文、配置、插件权限、备份与模型路由，必须继承数据分类、保存、清理、端点与环境约束；不能只放在 Discipline 能力包。
- **“DSH 内部状态可丢弃”不成立**（见结论 4）：V4 已发布，原生状态按已发布格式承担兼容义务；不得据此把隔离运行根当作可随意丢弃。
- **Git 只纳入审查过的配置与样例**；真实会话正文、凭证、真实工作材料、`~/OmniDesk` 的真实会话内容都不入 Git。
- **Tauri 技术切片 ≠ 产品化批准**；桌面产品化须另行满足联合门槛（净节省 >20%、质量与合规达标、至少一位同事实际愿用、存在桌面可解决的具体痛点）并获用户批准。
- **不扩大用户转述授权**：仅“满足所述脱敏条件、个人数据分析与生态研判、不得分发”。其他用途、模型/服务商、工具、设备、运行环境均不由该摘要自动获准；仓库未保存授权原件，本次证据来源是用户提供的答复摘要。

### 8. 本回合运行过的检查与隔离验证

- 只读源码核对：`home-paths`、`bundle/base/cordis.patch.yml`、`session-persistence-jsonl/format.ts`、`acp/acp/src/index.ts`、`workspace/workspace/src/{index,paths}.ts`、`session-format-status.md`、`engine_supervisor.rs`、`serve.py`、`acp_adapter.py`、`profiles/*/package.json`、隔离 `storages/workspace.json`。
- 隔离一次性合成运行：`/tmp` 一次性数据根 + 本机回环合成端点，SIGKILL 中断前后原始日志读取、全新进程 `recover_task`/`recover_session`（输出 `/tmp/arch003/out/{probe-summary,raw-log-probe,restart-recovery}.json`）。
- **隔离副作用对照（脚本运行）**：对 `~/.dsh`、`~/.agents`、`~/OmniDesk`、`~/YvanAI` 取前后快照（11,422 行）。45 秒空转窗口 `ambient_diff_lines=0`；受控对照（快照 → 跑完整探针 rc=0 → 快照）`controlled_diff_lines=0` → **探针不改动上述任一目录之外的内容**。期间观测到的 5 处文件变化已根因为用户自己正在运行的 DSH 应用的环境写入，**不归因于探针**。本回合结束时仓库 Git 状态仍为同样 5 项改动。
- **未运行**：真实模型调用、真实 Tauri 窗口人工验收、跨平台验收、并发写者（`concurrent_ownership`）、备份恢复演练、真实用户会话故障复现。

### 9. 未解决问题（OD-ARCH-003 审计当时的状态）

- 当时 Finding A（默认 Profile）与 Finding B（分析功能不可用）均未修复；此后 Finding A 已由 OD-BUILD-003 改为 `acp` 默认值，但其宿主传递证据仍待补齐；Finding B 仍未列入当前计划。
- 授权准入仍待用户逐项给出书面状态：模型/服务商、工具/MCP、个人设备、运行环境、以及其他用途；仓库无授权原件。
- 备份脚本 `scripts/backup-sessions.sh` 指向原版 `~/.dsh` 而非隔离 OmniDesk 根；**恢复从未演练**。
- 存储按原始 cwd 分组 vs 注册表/ACP 按 realpath 的不对称：需要 OmniDesk 规范化 cwd 才能消除 (c) 类分裂（见 docs/decisions.md D-013）。
- `tests/reports/citation-audit-report.md` 声称 30/30、96.7% 准确率，**无运行证据支持**，已在 `docs/OmniDesk-Phase2B-Inventory.md` 声明存疑；本记录不将其作为证据。

### 10. 下一步（提议，待用户批准；历史段落）

【历史状态：本段撰写时，ARCHITECT 提出 OD-BUILD-003 且等待批准；该计划后来已获批、实施并经独立复核，详见本文件顶部。】

## OmniDesk 综合规划 v0.4：初步架构研判（2026-10-16）

**结论：方向可作为下一轮审计基线，暂不将 v0.4 全文冻结为架构决策，也不据此实现逐消息日志。** v0.4 抓住了作者实际痛点——会话可找回、升级可控、项目化组织——并把上层 Discipline 能力与通用个人工作台分开，优先级比继续扩充专业 Skill 更符合当前证据。但“DSH 内部状态可丢弃”以及“适配层拦截事件后能够完整恢复会话”尚未由当前实现证明。

### 当前代码与版本证据

- Git 基线：HEAD 5ffc7101e90dcf3a2047f214b71286d3a32208a8；审查开始时已有 GOALS.md、PLANS.md、README.md、REVIEW.md、docs/decisions.md 未提交差异，本轮保留。未运行代码测试，本轮为文档与静态审查。
- runtime/RUNTIME.lock 锁定 DSH 0.2.1-alpha.2，commit d743267388641bc76f17c45ce8b4c231aed1d32c；子模块工作树与该 commit 一致且干净。证据等级：文件/版本事实。当前官方文档描述的是可演进的最新实现，不能替代锁定版本能力证明。
- OmniDesk engine/omnidesk_engine/acp_adapter.py 在配置 state_dir 时以原子替换写每任务一个 JSON；内容包括任务元数据、状态、已拼装 assistant 输出 content 等。提交前先写 queued 记录，收到 `session/new` 后写入 session 信息，再启动 prompt；收到普通 ACP 事件时会更新进程内事件列表和文本缓冲，但不会逐事件调用 `_persist()`；终态/取消或中断收束时再落盘。因此进程在任务中途崩溃，最后一次落盘之后已收到的输出和状态可能丢失。它不是逐事件追加日志，也不记录可重建 DSH 内部完整事件的契约。`recover_task()` 对已持久化终态恢复元数据；对非终态可探测 session，但仍标 `unknown`，且不自动重放。证据等级：静态代码检查。
- 适配器接口事实区分 recover_session（恢复底层既有 session）与 recover_task（恢复 OmniDesk 任务状态）；测试矩阵明确中断任务不得报成功或自动重放。证据等级：仓库文档/测试契约；本轮未重跑其场景。
- scripts/backup-sessions.sh 将 ~/.dsh/sessions 和 workspace 索引复制到 ~/YvanAI/backups/sessions/<timestamp>，作为副本式兜底。脚本没有本轮恢复演练证据，且此旧路径不等同于 v0.4 提议的统一 ~/OmniDesk。证据等级：文件事实；恢复可用性未验证。
- 单独的 /home/kevin/Code/DSH-Desktop 仓库 HEAD 为 474d3936d1ce498147ba444c0b182d5e877f5c2d，存在用户未提交修改；它不能作为 OmniDesk 锁定 DSH 版本，也不能据其本地实现推断用户当前运行桌面的具体故障。其会话文件定位仍须隔离后只读核查，并与用户所用二进制/数据根建立版本对应关系。

### 架构判断

1. **会话自主持有值得优先解决，完整正文镜像日志暂不 go。** 首先要定义目标是“历史仍可见/可读”“重启后恢复既有 DSH session”“知道中断任务的真实状态”还是“从中断点续跑”；四者所需机制不同。ACP 语义事件流不能未经证据视为 DSH 可重放的持久事件源。复制所有输入输出还会形成第二套敏感正文存储、备份和删除责任，并扩大日志泄露面。
2. **优先诊断并利用 DSH 原生持久化，之后再决定 OmniDesk 自有层。** 若故障是列表索引或项目路径身份，则自有项目/session 映射或索引可能解决可发现性，无需复制正文；若原始日志确实丢失/损坏，才比较原生日志备份和独立副本。ACP 输出写入 OmniDesk 状态 JSON 的丢失窗口、持久化时机和恢复一致性仍需针对性演练。
3. **v0.4 的两层产品定位有帮助，但合规政策不能只放在 Discipline 能力包。** 通用底层也接触会话正文、配置、插件权限、备份和模型路由，因此必须继承数据分类、保存、清理、端点和环境约束。现有用户转述仍仅覆盖特定脱敏条件下的个人数据分析与生态研判且不得分发，不能推及周报、普通材料、同事试用或分享。
4. **项目化会话列表可作为自研 UI 的首个用户价值，但先确定持久化来源与项目身份。** v0.4 同时说“桌面壳后置”，而仓库已有 Tauri/Rust/React 技术切片；这不构成架构冲突：可将该切片视作验证资产，暂不承诺发布或继续产品化。网页或 Tauri 路径由真实使用痛点和稳定性证据决定。
5. **市场证据仍只支持个人自用验证。** 作者亲历的对话难找/升级不稳是合理的首发问题假设，但没有持续使用、其他用户或付费证据，不应据此宣称通用工作台已有市场验证，也不应扩建成通用 Agent 平台。

### 下一步与边界

- 已在 PLANS.md 拟定 **OD-ARCH-003：v0.4 架构校准与会话丢失机制审计**，状态为待用户批准；本轮没有授权或开始实施。**【历史状态：OD-ARCH-003 其后已获批准并执行完成，结论见本文件顶部同名章节。】**
- 原 OD-ARCH-002 真实 Tauri 窗口合成数据任务路径验证暂缓，待会话持久化审计后决定是否仍有必要；这不是否决该项验证。
- 本轮没有读取用户会话正文、凭证或真实业务材料，没有修改 DSH Runtime、其他 DSH 仓库或代码，没有运行模型或修改路由。

## OD-BUILD-002 独立复验：桌面应用数据根拒绝仓库内落点（2026-10-16）

**验收结论：通过（OD-BUILD-002 计划范围）。** 复验基线 `HEAD` 为 `5ffc710`；检查开始时工作区和暂存区干净。实际代码已在 `AppPaths::assert_isolated()` 同时检查词法路径与解析后的真实落点，并在 `ensure()` 创建目录前重新校验。计划列明的普通仓库路径、软链接指向仓库、软链接指向仿真禁区、仓库外路径及拒绝后无目录残留均有测试覆盖。审查后本 ARCHITECT 回合只修改了共享状态与审查文档。

### 本轮独立验证

- 使用项目用户级 Rust 工具链和 Tauri 依赖环境运行 `cd desktop/src-tauri && cargo test --offline`：**60 passed，0 failed**（44 单元、13 宿主集成、3 任务外壳 E2E；doc-tests 0）。
- 运行 `cd desktop && npm run build`：通过，包含 TypeScript 检查与 Vite 构建（22 modules；JS 245.68 kB，CSS 7.48 kB）。
- 静态检查 `desktop/src-tauri/src/app_paths.rs`：仓库词法边界和 canonical 真实落点边界均由 `starts_with` 的路径组件比较；`ensure()` 在任何 `create_dir_all()` 前调用 `assert_isolated()`。
- 检查起始时 `git status --short --branch` 为 `## main...origin/main`，HEAD 与 origin/main 均为 `5ffc7101e90dcf3a2047f214b71286d3a32208a8`，无工作区或暂存区差异。
- `git diff --check HEAD^ HEAD` **未通过**：检查到本 checkpoint 大范围文件中的行尾空白和文件末尾空行，涉及 SVG 及多份新增文件；这不是本轮产生的未提交差异，也不改变本计划功能验收，但该提交级格式检查不能记为通过。

### 复核结论与适用边界

- OD-BUILD-002 的实现和回归验证通过，OD-BUILD-001 遗留的 R4 仓库边界遗漏已关闭。
- 仍有两项已知技术限制：`OMNIDESK_REPO_ROOT` 可改变运行时检查采用的仓库根；路径解析与目录创建间仍存在并发替换竞态。当前用例仅在 Linux 验证。它们已在下文历史审查和本计划的适用限制中记录；本计划未要求抵御可控制进程环境者或同机恶意并发替换。
- 本结论**只验收 OD-BUILD-002 的计划范围**，不等于真实 Tauri 窗口下的完整使用流程已验收，不验证真实模型输出质量，也不自动扩大用户转述的材料、用途、模型端点、设备或运行环境授权。完整 MVP 的受限自用准入仍须以实际窗口操作证据及当前授权边界核对为依据。
- 本轮已将 `README.md`、`GOALS.md`、`docs/decisions.md` 和本审查记录的当前状态校准为 OD-BUILD-002 已通过、完整桌面路径仍待复核；OD-BUILD-001/R4 的旧结论保留为历史。`STATUS.md` 中的执行记录和历史段落需继续按历史事实阅读。

下一步候选计划见 [PLANS.md](PLANS.md) 的 **OD-ARCH-003**；尚未批准，不授权开始计划内审计或后续 Builder 实施。**【历史状态：该计划其后已执行完成，见本文件顶部；当前待批准计划为 OD-BUILD-003。】**

## OD-BUILD-002：桌面应用数据根拒绝仓库内落点

**验收结论：通过（本计划范围）。** OD-BUILD-001 遗留的 R4 漏项已关闭：桌面应用数据根在词法路径和解析后的实际落点两侧检查 OmniDesk 仓库边界；`ensure()` 在创建目录之前重新执行隔离校验；回归用例覆盖普通仓库路径、软链接指向仓库、软链接指向仿真禁区、仓库外路径和拒绝后无仓库残留。

### 审查基线与独立验证

- 审查时 HEAD：`17efd20be01b2a01cacf11f9523ccff302acce61`。工作区保留此前全部修改、删除和未跟踪内容；暂存区为空。`desktop/` 是未跟踪目录，普通 `git diff` 不会包含它；因此以实际源码、测试、BUILDER 状态记录进行审查，并将该可审计性限制保留在记录中。
- 独立运行 `cargo test --offline`：**60 passed，0 failed**（44 单元、13 宿主集成、3 E2E；doc-tests 0）。Rust 使用已配置的用户级工具链及本机 Tauri 依赖前缀。
- 独立运行 `npm run build`：TypeScript 检查与 Vite 构建通过（22 modules）。
- `git diff --check` 与 `git diff --cached --check` 通过。由于实现目录未跟踪，这两项不能替代其源码审查。
- 未运行真实模型、未读取凭证或真实工作材料，未修改 `runtime/`。

### 按验收条件复核

- **词法仓库路径拒绝：通过。** `assert_outside_repo_boundary()` 先对词法归一和原始路径做前缀比较；普通仓库路径回归测试覆盖。
- **符号链接真实落点拒绝：通过。** `assert_isolated()` 先调用 `resolve_real_path()`，仓库边界函数再对数据根真实落点与仓库根解析结果做组件级 `starts_with`；符号链接指回仓库测试用词法干净的别名路径。
- **禁区软链接与正常外部路径：通过。** 仿真 `.dsh` 真实落点被既有真实路径禁区检查拒绝；干净仓库外临时根通过。
- **创建前拒绝：通过。** `AppPaths::ensure()` 首先调用 `assert_isolated()`，失败映射为 `PermissionDenied` 后才会执行目录创建；回归测试确认仓库内目标路径不存在。
- **错误处理边界：通过但记录一项非阻断限制。** 数据根真实落点解析失败即拒绝；仓库根解析失败会退回词法归一比较。实现当前会优先读取运行时 `OMNIDESK_REPO_ROOT`，其值必须被当作可信部署配置；若启动者可任意将它设为与实际仓库无关的位置，就能改变此边界检查所依据的仓库根。该实现沿用本计划要求的 `task_shell` 仓库根约定，适用于当前自用开发场景，但不构成抵御可控制进程环境者的安全边界。未来若把此检查作为对不可信启动环境的硬性保护，应另立计划，将仓库根固定或验证为可信路径。

### 剩余限制

- `resolve_real_path()` 与 `create_dir_all()` 之间仍有检查后路径替换竞态；没有使用目录句柄和禁止跟随链接的原子创建方案。本计划明确未要求消除该风险，因此不阻断本次验收，但不得把它描述成可抵御同机恶意并发替换。
- 符号链接行为仅在当前 Linux 环境验证；Windows/macOS 及真实 Tauri 窗口仍未验收。
- 本次验收只关闭 OD-BUILD-002 和 OD-BUILD-001 R4 的仓库边界遗漏，不代表整个个人 MVP 已可接入真实工作材料。启发式红线扫描不构成完整准入或合规控制；须继续遵守经用户转述的用途和不得分发边界。

### 结论与后续

OD-BUILD-002 在其计划范围内通过。当前没有已批准或执行中的下一项实施计划；[PLANS.md](PLANS.md) 仅记录后续待评估领域，不授权 Builder 开始工作。

---

## OD-BUILD-001：修复个人 MVP 的保存确认、来源真实性与处理边界

**验收结论：不通过（R4 尚未完整满足）。** R1–R3 通过本轮源码审查；R4 的任务产物路径检查已处理词法路径与解析后的实际落点，但桌面应用数据根 `AppPaths` 未验证仓库边界，仍能将数据目录指向仓库。MVP 继续保持未验收，仅可使用合成数据和许可范围内公开数据。

### 审查基线与证据

- 当前分支 `main`，HEAD `17efd20be01b2a01cacf11f9523ccff302acce61`。本轮开始时工作区 181 项，暂存区为空；沿用并保留所有现有差异。`desktop/` 源码文件目前为未跟踪文件，因此常规 `git diff` 不包含这些文件，也无法从 Git 基线确定 BUILDER 单独引入的逐行差异。本次以实际文件、BUILDER 状态记录、回归用例及 `git diff --no-index` 的文件内容为审查证据；此限制降低改动归属的可审计性。
- 已独立运行 `cargo test --offline`（使用仓库已有的用户级 Rust/Tauri 依赖）：55 passed，0 failed（39 单元、13 宿主集成、3 任务 E2E）。
- 已独立运行 `npm run build`：TypeScript 检查与 Vite 构建通过。`git diff --check` 通过，但不覆盖未跟踪文件。
- 本轮未调用真实模型，未读取凭证、真实工作材料或修改 `runtime/`。

### 按发现项复核

#### R1 — 通过（按本计划定义的显式确认合同）

- `SaveArtifactRequest.user_confirmed` 是必需的反序列化字段；缺失时 JSON 解析失败，`save_artifact()` 对 `false` 拒绝；注释不能代替布尔值。前端保存请求传入该值。
- `save_requires_explicit_confirmation`、`save_request_rejects_missing_confirmation_field` 与宿主边界回归测试均通过。
- 限制：布尔值表示调用方声明已确认，不是对本地 IPC 调用者身份或真实鼠标操作的密码学证明。本轮满足 OD-BUILD-001 的接口合同，不应将其表述为抵御同机恶意调用者的安全认证。

#### R2 — 通过

- 可保存来源只允许 `user` 和 `model_unverified`；`model` 与客户端提供的 `task_state` 路径已移除，前端没有“已验证模型”选项。测试拒绝 `model` 来源并允许明确标为未复核的来源。
- 可信模型结果仍未与真实引擎终态绑定，状态如实标为未实现。

#### R3 — 通过（限已识别的启发式命中）

- 宿主分析命令使用 `analyze_input()`；分析、解释提示词和保存入口均先执行同一红线扫描闸门。命中用例不会返回统计结果或提示词，错误不回显命中原文；干净合成输入统计指纹保持一致。
- 扫描器仍是启发式，不能识别全部身份、岗位或工作细节，也不等于授权或合规判断。

#### R4 — 不通过（P1）桌面应用数据根缺少仓库边界拒绝

- `desktop/src-tauri/src/app_paths.rs:106-121` 的 `AppPaths::assert_isolated()` 检查词法路径及 `resolve_real_path()` 的解析落点，并拒绝若干 DSH/agent 禁止目录；但没有检查真实落点是否位于 OmniDesk 仓库内。该模块也没有指向仓库或模拟禁区的符号链接回归测试。
- `desktop/src-tauri/src/lib.rs:284-290` 的 `build_state()` 在 `AppPaths::resolve()` 后调用 `ensure()` 创建目录。由于仓库根本身不包含 `FORBIDDEN_FRAGMENTS` 中的路径片段，把 `OMNIDESK_APP_DATA_ROOT` 设为指向仓库的干净别名（或仓库内普通目录）可通过当前隔离校验，随后 `ensure()` 会在仓库内创建 `ui-state/`、`logs/` 等目录。
- 产物/使用记录侧的 `task_shell::assert_outside_repo()` 已检查解析后的落点，且测试覆盖软链接指向仓库；但它不能替代 `AppPaths` 自己的数据根校验。两类数据根边界不一致，OD-BUILD-001 的“所有数据根入口”要求未满足。
- 证据等级：静态源码审查。尚未运行桌面启动级攻击复现；无需通过真实用户数据验证即可确认漏掉仓库边界比较。
- 修复要求：桌面应用数据根必须对词法路径及解析后的真实落点都执行仓库外检查；`AppPaths::ensure()` 不得在未通过此检查的路径上创建目录；新增普通仓库路径、软链接指向仓库、软链接指向仿真禁区、普通仓库外路径的测试。

### 当前适用限制

- OD-BUILD-001 的 R4 未通过前，MVP 仍未验收；不得因 R1–R3 通过而扩大真实材料使用。真实 Tauri 窗口验收、真实模型解释质量及跨平台行为仍未验证。
- BUILDER 报告中的 55 项结果经本轮复跑确认；独立复跑未执行 `cargo build --offline`，构建覆盖由 `cargo test --offline` 和已报告的构建记录分别支持。

### 下一步

`PLANS.md` 已拟定 OD-BUILD-002，限于补齐桌面应用数据根的仓库边界检查及回归测试，当前待用户批准。

---

## OD-ARCH-001：个人 MVP 独立审查与文档基线收敛（历史审查）

## OD-ARCH-001：个人 MVP 独立审查与文档基线收敛

**当时验收结论：MVP 审查不通过；OD-ARCH-001 文档工作完成。** 这是基于当时 HEAD `17efd20` 的历史判断；R4 后由 OD-BUILD-002 补齐并在本文件顶部独立复验通过。当前是否适合进入受限个人试用，应结合顶部 OD-BUILD-002 复验和待批准的 OD-ARCH-002 真实窗口合成数据验证，不以本历史段落替代当前结论。用户转述的有限许可仍不得扩大解释，且不得分发。

### 审查基线与验证

- 仓库分支：main；审查时 HEAD：17efd20be01b2a01cacf11f9523ccff302acce61。
- 本轮开始时 Git 状态：175 项（11 项已修改、164 项未跟踪，暂存区为空）。所有原有差异均保留；本轮文档调整以外的改动未重置、覆盖或提交。
- 已检查 desktop/src-tauri/src/lib.rs、task_shell.rs、app_paths.rs、desktop/src/workbench.tsx、API/types 及相邻测试。
- 既有实测：cd desktop && npm run build 通过；cd desktop/src-tauri && cargo test --offline 49/49 通过（34 单元、13 集成、2 E2E）。这说明已有测试断言通过，不代表下列安全路径已满足产品要求。
- 未运行真实模型调用，未使用真实工作材料，未读取凭证或修改 DSH Runtime；真实 Tauri 窗口视觉验收和跨平台验收未完成。

### 阻断发现

#### R1 — P1：Rust 保存边界未强制人工确认

- 证据：desktop/src/workbench.tsx 的 95 行、304 行以 UI 状态 confirmed 控制按钮；desktop/src-tauri/src/task_shell.rs 的 SaveArtifactRequest 655–666 行没有确认布尔字段；save_artifact() 680–683 行及其校验没有确认状态。Tauri 保存命令 task_save_artifact（desktop/src-tauri/src/lib.rs 228–233 行）直接接受该请求。
- 影响：绕过 UI 直接调用已注册命令即可保存未确认产物；可选备注不能证明用户确认。
- 证据等级：静态代码审查；此前测试未覆盖该绕过。
- 修复要求：Rust 保存入口必须要求显式确认，缺失或 false 均拒绝；增加直接保存函数/命令路径测试。

#### R2 — P1：解释来源与模型成功状态可由 UI 伪造

- 证据：desktop/src/workbench.tsx 217–237 行在保存时自行提交 explanationSource 和 taskState；desktop/src-tauri/src/task_shell.rs 652–665 行允许 model 与调用方提供的 task_state，695 行只按字符串 succeeded 判断，未绑定真实引擎任务或结果。
- 影响：手写文本可被记为模型成功结果，来源记录不可信。
- 证据等级：静态代码审查；当前测试不能证明 UI 所报状态来自引擎。
- 修复要求：删除客户端可伪造的 model/succeeded 路径，只允许 user、model_unverified；同步 UI 和产物记录。可信模型来源另行设计。

#### R3 — P1：分析和提示词入口未统一执行红线闸门

- 证据：desktop/src-tauri/src/lib.rs 195–215 行的 task_run_analysis() 收集 red_line_scan() 命中后仍调用 deterministic_analysis()；desktop/src-tauri/src/task_shell.rs 624–646 行的 build_explanation_prompt() 未调用 assert_input_allowed()。保存入口 701 行才调用闸门。
- 影响：已被扫描器识别的输入仍可在分析及提示词生成路径中被处理。扫描本身为启发式，不能证明完整识别个人信息或合规性。
- 证据等级：静态代码审查；未用真实业务输入测试。
- 修复要求：所有处理输入的 Rust 入口统一先执行闸门，命中时不返回统计或提示词；补直接调用测试，并明确提示扫描局限与准入责任。

#### R4 — P1：禁区检查未验证规范化后的真实路径

- 证据：desktop/src-tauri/src/task_shell.rs 287–305 行 normalize_path() 在 canonicalize() 失败时退回词法归一；322–338 行 assert_local_root_allowed() 只检查词法路径字符串；341–352 行仓库边界检查虽比较规范化路径，但未将该路径再交给禁区校验。保存前 710–725 行创建根后仍按传入路径做词法禁区检查。独立桌面路径 desktop/src-tauri/src/app_paths.rs 105–138 行同样只检查根路径字符串。
- 影响：符号链接指向禁区或仓库时存在绕过风险。
- 证据等级：静态推断；未实际尝试符号链接绕过，不把风险写成已复现攻击。
- 修复要求：所有数据根入口在读写前校验解析后的实际落点；无法安全解析时默认拒绝；用临时符号链接指向仿真禁区和仓库的测试覆盖两类路径。

### 其他验收限制

- 真实 Tauri 窗口曾在虚拟显示环境呈空白；浏览器或 Vite 预览不能替代桌面窗口验收。
- 真实模型解释质量未验证；跨平台未验证。
- 仓库未保存单位授权原件。现有许可范围仅来自用户转述，限于满足所述脱敏条件、用于个人数据分析与生态研判且不得分发；不得据此推定周报、材料写作、项目管理、模型服务商、工具、设备或运行环境均已获准。

### OD-ARCH-001 文档基线收敛验收

- 当前协作基线集中在仓库根目录 AGENTS.md、GOALS.md、PLANS.md、STATUS.md、REVIEW.md 与 docs/decisions.md；README.md 提供索引。
- 五份旧路线/阶段计划已移至 docs/archive/ 保留，不作删除。归档说明将它们标为历史快照；详细契约、审计报告、阶段结果和运行证据保留在 docs/ 与 tests/ 下。
- 活动文档未发现对已归档规划文件的规范性引用。报告中作为当时记录的旧文件路径保留为历史事实。
- 验证：当前 Git 状态重新清点为 181 项，暂存区为空；空白差异检查通过；runtime/ 无差异。本轮没有修改业务代码，因此未重跑代码测试。
- 结论：OD-ARCH-001 的文档收敛部分完成。旧归档内部的历史相对链接不保证都可用；归档文件不作为当前项目状态来源。

### 下一步

当时的下一步为拟议中的 OD-BUILD-001。该计划已实施，其 R4 漏项后经 OD-BUILD-002 补齐；本段保留历史计划状态，不代表当前待办。


## 2026-10-20 独立复核：交接文档核实 + D-024 源码级复核（ARCHITECT）

- **对象：** docs/handover-verify-2026-10-20.md（提交 e3fd4a8，工作区干净 0 项）。复核方式：不采信文档自述，直接读源码。
- **已独立核实（源码级）：**
  1. **ACP 无 token 级流式 = 属实。** dsh-acp/lib/index.js 的 assistantUpdates()（562 行起）把**已提交**消息的 content 块整块转为 agent_thought_chunk / agent_message_chunk，无增量输出。界面"逐字效果"为本地模拟，文档标注诚实。
  2. **真多轮是我方欠账 = 属实。** ACP 有 session/new、session/prompt、session/resume；我方适配层已实现 recover_session（engine_bridge.rs:285"重开既有会话（resume）"），但 submit_task 每轮新会话+上文重放。补上"线程持有 engine_session_id"即可去重放，不需要新契约、不扩通信层。
  3. **仓库混淆提醒 = 有效。** 本工作区 HEAD e3fd4a8；/mnt/data/Code/omniharness-ui 是旧克隆（5ffc710+未提交改动），两者是不同仓库不同状态。
- **D-024 研判（供用户拍板）：** 第 1 步（接线去重放）低风险高收益，建议立即做。第 2 步（只验证 DSH web 应用本地 API，不改产品）可并行。第 3 步路径 A/B 涉及"体验继承 vs 产品独立性"取舍，等第 2 步证据再定，不预设结论。
- **结论：** 交接文档结论与源码事实一致，未发现夸大或失实标注；已验证/未验证分类干净。D-024 第 1 步建议立项为 OD-ARCH-034，待用户批准。


## 2026-10-21 独立复核：OD-ARCH-035（ARCHITECT）

- **方式：** 不采信 STATUS 自述。①直接调用本会话已加载的 screenshots_design_inspiration MCP 工具（search_inspiration 查询 "AI chat workspace dashboard"）→ 真实返回 20 个产品（Dovetail/ChatGPT/Harvey AI/Asana 等），带图库与截图直链；②核对 docs/BUILDER-Task-Instruction-Template.md 已含"UI 实现前先搜参考"条款与借鉴要点模板；③AGENTS.md 已有派工规则行。
- **范围修正确认：** 装在 Codex（~/.codex/config.toml，带备份）而非 OpenCode，符合用户实际要求；未动 OpenCode 配置。
- **结论：通过。** 035 完成且真实可用；"UI 先搜参考"条款已固化进派工模板（OD-ARCH-041 之后的新 UI 任务自动适用）。
- **顺带确认：** HEAD 6dff9dc（OD-ARCH-041 场景切换性能修复已提交），工作区 1 项改动；041 复核另行安排。

## 2026-10-21 独立复核：OD-ARCH-046（ARCHITECT）

- **方式：** 不采信 STATUS/证据自述。独立复测：tsc 0 错（exit 0）；cargo test --lib **146/146 通过**（用 setup-desktop-toolchain.sh --print-env 注入工具链环境后实跑）；源码抽查 pending_writes.rs/scheduler.rs/thread_store.rs 存在且 lib.rs 挂接 5 个 pending_write 命令。
- **核对计划四项：** ①写确认制——pending/confirm/reject/expiry 机制落地，5 项单测覆盖一次性消费/过期拒绝/负载不一致拒绝；定时任务写建议文件先入待确认卡、确认后落盘、目录前后对比（3→3→新增）属实。②自动降级——端点退出后公告栏明示降级且无假文件；权限全关时起草明确拒绝、无模板冒充（含"未创建目录、未启动引擎"说明），符合数据诚实红线。③定时/心跳——once 定时实测 30 秒 tick 触发、权限关时跳过并说明原因、last_review_at 真实推进；心跳 5 分钟间隔推进且与手动扫描一致性有单测。④设置页五卡——截图与配置文件佐证，权限开关联动宿主判定真实反转。
- **遗留（不阻塞验收）：** 窗口侧心跳提醒逐字比对未做（有单测等效覆盖）；真实过期记录的界面提示未窗口实测；取消中任务的降级分支未单独截图。均已在证据 README 如实标注。
- **工作区注意：** 本轮含 OD-ARCH-045 线程持久化遗留改动（thread_store.rs 等）与 046 改动共存（25 项未提交）。045 的登记此前未见独立复核记录，本复核顺带确认 thread_store 测试存在（thread_store_restart.rs）；045 完整复核仍需单独做。
- **结论：OD-ARCH-046 验收通过。** 全部改动未提交未推送，等用户晨间决定提交与推送安排。

## 2026-10-21 独立复核：047（推送结果 + 045 补复核 + 实测清单）

- **推送：** 远端 origin/main 与本地一致（领先 0），工作区干净 0 项。三批提交（ba77c2d 功能/a3bf0ad 证据/bb09af1 文档）主题清晰。
- **045 补复核：** 六步证据齐全；其中第 5 步（DSH web API 只读探针）为**关键新证据**——token 级流式（assistant-stream 帧）、同会话连续两轮、取消、重启后会话持久、模型目录**全部实测存在**（脚本运行级）；思考块流式与工具调用事件未验证（合成端点不产思考内容）。D-024 第 2 步成立，路径 B（换接缝用 DSH 本地 web API）已具备证据基础，待用户对第 3 步拍板。
- **实测清单：** docs/user-testing-checklist.md 已就位，照做即可开始真实模型体验。
- **结论：047 验收通过。**

## 2026-10-23 响应速度诊断与 README 修订（ARCHITECT）

**背景：** 用户实测反馈提问到回复约 30 秒；场景里让 AI 找新闻，授权后卡住 3 分钟无回复。要求对比裸模型 / 有审查 / 有工作流的速度。

**速度诊断（源码 + 台账证据）：**
1. **本地链路不是瓶颈。** 提交路径只有红线扫描、写确认 gate（毫秒级本地操作），然后 spawn_blocking 进引擎。引擎经 Web-API 通道与 DSH 通信，text-delta 是真实流式，前端直渲不等完成。
2. **40 条任务台账实测：成功任务中位耗时 11 秒，最短 2 秒，最长 107 秒（长文输出 2000+ 字）。** 用户感到的 30 秒主要是模型推理 + OpenDesign 中转网络 + 输出长度，不是 OmniDesk 链路开销。
3. **思考等级：** 当前 overlay 中 ACP profile 带 reasoningEffort: high（会慢）；Web-API 通道由前端逐次传，界面默认"off"映射为 none（不慢）。用户在模型选择器里选了"高"的任务会明显变慢，属模型侧行为。
4. **对比测试结论：** 裸模型 / 有审查 / 有工作流三者的差异只在本地毫秒级操作（扫描 + gate + 事件归一化），端到端耗时由模型决定。**无需为速度做架构改动**；要快，选低思考等级或更快模型。

**搜索授权卡住诊断：**
1. DSH 工具调用需审批时走 approval seam（dsh-tools serviceAsk）；OmniDesk 的 Web-API 通道**没有实现审批 UI**，前端也不消费 tool_activity 事件（types.ts 定义了但无渲染）。
2. 用户点"授权"后 turn 仍挂在等审批确认；适配层把未映射的 turn/end reason（如 blocked）落为 unknown，前端只提示"状态未知"不重试 → 表现为卡住。
3. **这是真实功能缺口，需要补：** ①适配层把审批请求转为可见事件；②前端渲染审批卡片（允许 / 拒绝按钮）；③允许后 DSH 继续执行。在此之前，建议用户不要在对话里触发需要审批的工具（如联网搜索），或在 DSH 设置里把权限预设改为自动允许（用户自担风险）。

**README 修订（用户已批准）：**
- README.md 已按 audit/product-direction-review/04-github-copy.md 的结构重写：一句话定位、解决什么问题、5 条核心特征、当前状态（已实现/未实现分开）、快速开始、架构表、项目边界、状态入口。
- GitHub About 与 Topics 需用户自行在网页更新（gh CLI 未登录）：建议 Description「Personal AI workspace for professional writing & project tracking — conversation-driven, with scene workflows, verified outputs, and local-first data」；Topics「ai-workspace, tauri, personal-knowledge, local-first, deepseek」。

**遗留：**
1. tests/verification/desktop/run_desktop_e2e.py 当前在本机失败（startup_tail: unknown option --port）——测试启动器传参方式与运行时不匹配，需修测试脚本；不影响产品运行时（实测 --port 0 --no-open 正常启动）。
2. 适配层无 TTFB（首字延迟）埋点；如需精确定位 30 秒感受，可在下一任务加计时埋点（不读凭证，只记时间）。
3. 未提交改动：README.md 重写 + 本 Review 追加 + audit/ 目录未跟踪。等用户批准提交。

## OD-ARCH-066 复核结论：**通过**（审批链路 + TTFB 埋点）（ARCHITECT，2026-10-23）

- **独立核查（本机，与 BUILDER 分离）：** cargo test 212/0 + 2/0 + 13/0 + 3/0 + 1/0（合计 231，与 STATUS 一致，PKG_CONFIG_PATH 指向用户级依赖目录）；pytest 37 通过；tsc 0 错误；`git status --porcelain runtime/` 为空（DSH Runtime 零差异）。
- **代码抽查（全部属实）：**
  1. `_ApprovalChannel`（web_api_adapter.py:239）常驻 `$events` 订阅；按 agentId 归属到 OmniDesk 任务；eventId 幂等去重；伪造审批编号被拒（A04 通过，pending 不被误改）。
  2. `respond_approval`（同文件:1592）校验任务归属 + pending 列表 + allow 必须为 bool；RPC 失败保留 pending 用户可重试，界面不假装已生效；任务终态拒绝应答并如实报错。
  3. TTFB：submit 记 monotonic 时间戳，首个 text_delta 到达时计算 `ttfb_seconds` 保留 3 位，写入任务台账（task_core.py）、契约（contract.py:321）、前端显示「首字 x.xs」（HomeChat.tsx:1029）。
  4. 前端审批卡片挂任务、允许/拒绝二态、已决定与已失效独立文案；`approval_decided` / `approval_cancelled` 双事件驱动状态收敛。
- **偏差认可（两条，均合理）：** ①计划写 callId，实际协议用 eventId 定位——按真实协议实现，契约语义未变；②计划推测「拒绝后大概率 blocked→failed」，实测拒绝后 turn 仍 succeeded——按实测修正界面文案「引擎按自己的语义结束本轮」，A02 验收改为「到达终态且 pending 清零」。
- **待用户实测（不阻塞验收）：** 真实窗口点允许/拒绝的交互观感；真实模型端点验证（本轮全部走合成端点）。
- **遗留登记（后续决策）：** ①终态清空 pending 与前端「已失效」文案的语义统一为显式 `approval_cancelled` 事件，属优化非缺陷；②「拒绝即中止本轮」需引擎侧能力，属跨层改动，单列决策；③审批卡片是否展开工具参数摘要——涉及正文落盘边界，先定策略再做。
- **结论：** 066 验收通过。真实窗口验收由用户在下次实测中补做；三项遗留不阻塞 MVP。

## OD-ARCH-068 复核结论：**通过**（平台中立规则资产化）（ARCHITECT，2026-10-23）

- **独立核查（本机复跑，与 BUILDER 分离）：** cargo test 223 lib + 2 admission + 13 host_integration + 3 task_shell_e2e + 1 doc 全通过；pytest 37 通过；tsc 0 错误；`git status --porcelain runtime/` 为空；`verify_profile_rules_068.py` 25 cases / 0 failed（证据 tests/verification/desktop/evidence/od-arch-068/profile-rules-068.json）。
- **四项目标逐一落实（代码抽查属实）：**
  1. **平台中立：** 新增 profile_rules.rs（RulePolicy::clean = 纯净底座零规则零拦截）；红线词表与写作铁律全部迁出 Rust/TS 硬编码，装在 ~/OmniDesk/app/profiles/discipline.rules.json（schema omnidesk.profile-rules.v1）；文件缺失 = clean policy，损坏 = fail-closed 拒绝提交（profile_rules_invalid），不会半拦不放。
  2. **隐私改脱敏：** mask_privacy 在宿主出站 prompt_text 上掩码（保留前 3 后 2 位），身份证/手机号不再拦截；toast 明示脱敏处数；本地对话原文保留。隐私关键词（身份证/电话号码）与 14 个业务红线词分列，互不影响。
  3. **随手记轻装：** composeConversationPrompt 新增 mode:"note"|"report"；note = 112 字符轻提示词，无铁律/章节/场景规则；weekly 场景默认 note。**这直接回应了用户 30 秒响应感受的另一半**——随手记不再带完整写作规则包。
  4. **思考档场景化：** 场景配置带 reasoning_effort（weekly=low / writing=low / project=off）；用户手动选档优先（effortTouchedRef）；引擎空闲时才应用默认档（避免 setModelRoute 重启打断任务）；omnidesk-model.yml 遗留 reasoningEffort: high 已清理。
  5. **设置页规则资产分页：** 指纹/词数/铁律数/隐私模式可视化，JSON 可编辑（.bak 备份）+ 恢复默认；保存不重启生效（单测级验证，窗口级待用户实测）。
- **出厂副本同步：** samples/profiles/discipline.rules.json 与运行时副本字节一致（2484B，13:10 vs 12:52 修改时间差属两次落盘，内容经哈希比对一致——如不一致以 verify 脚本 asset_cases 为准）。
- **偏差认可（BUILDER 如实登记 5 条，均合理无需返工）：** offline-flows.v2.json 未升级 schema（v1 保留）；证据目录 od-arch-068 取代计划笔误的 067；掩码位置在 Rust 宿主而非引擎层（效果等同且更靠近数据出口）；场景默认档仅引擎空闲时应用（防重启打断）；16 词拆 14 红线 + 2 隐私关键词（计划本身要求的变更）。
- **待用户实测（不阻塞验收）：** ①真实模型端点跑随手记 vs 周报的 ttfb 对比；②设置页改规则 → 不重启 → 下一任务生效；③移走规则文件 → 重启 → 零拦截（纯净底座）。前两项有单测等效覆盖，窗口级体验由用户日常使用中自然验证。
- **遗留（登记不阻塞）：** 「规则资产与 offline-flows.v1 双份场景规则」存在理论漂移面，当前靠 verify 脚本比对，后续可自动化；脱敏 toast 仅覆盖 mask 模式，Reject 模式的拦截提示文案未单独验证。
- **结论：OD-ARCH-068 验收通过。** 平台中立定调已在代码层面成立：无 profile = 纯净底座，装 profile = 纪检工作台。全部改动未提交未推送，等用户晨间决定。

## 2026-10-23 用户实测反馈诊断（ARCHITECT，三场景）

用户实测三个场景（工作记录正常 / 文案写作授权搜索后卡 3 分钟 / 项目跟进 5 秒回复但内容不合预期）。本机核查台账证据：

1. **工作记录：正常。** 台账 ttfb 2.8–5 秒，短任务全部 succeeded。
2. **文案写作卡住：已定位到三个环节。** 台账 omni-21ca40e9（writing）：ttfb 2.86s（开场白很快到达），created→state_since 差 **183.6 秒**，state=unknown、evidence_source=timeout——命中适配层 180 秒总超时（web_api_adapter.py:390 default_timeout=180）。即：模型说“我要联网检索”→ 用户授权成功 → 引擎执行搜索类工具但 **180 秒内未给出 turn/end** → 看门狗落 unknown 并停止监听（不自动重放）。三个可能原因按可能性排序：①DSH 的搜索/抓取工具本身执行慢或挂起（网络/上游源）；②DSH 发出了**非 approval/request 类型的 waterfall 询问**（钩子、其他权限），适配层 _on_frame 对其“如实忽略”（web_api_adapter.py:344 注释），既不显示也不应答 → 引擎等 forever；③授权应答被引擎忽略（概率低，UI 无报错）。台账按隐私设计不存事件历史，无法事后区分 ①②，需复现取证。
3. **项目跟进：速度正常。** ttfb 4.99s、总耗时 33s、2680 字 succeeded。内容不合预期属提示词/Skill 调优问题，不是链路缺陷。
4. **界面两个真实缺口（与速度无关，与“看起来卡死”直接相关）：** ①tool_activity 事件引擎已发（web_api_adapter.py:1355），前端 HomeChat.tsx **不渲染**——工具执行期间界面只有“生成中”，零活动可见性；②turn 超时落 unknown 后，界面未把“引擎超时未返回，可重试”讲清楚。

**结论：** 链路本身没有坏——快场景证明管线健康；慢/卡场景是“工具执行不可见 + 非标准询问被静默忽略 + 180 秒超时偏短”三件事叠加的体验问题。修复方案见 OD-ARCH-069（待批准）。

## 2026-10-23 联网搜索对比诊断（ARCHITECT，OD-ARCH-069 验收 + OmniDesk vs 原生 DSH 差距根因）

**用户实测：** OmniDesk 未分类场景搜“政治监督”沿革 → 工具活动可见（069 修复生效），但 300 秒超时终止、零正文。原生 DSH 界面同题 → ~3 分钟后 400 upstream_error「Content Exists Risk」。用户自己的 DSH（桌面 profile，装 multi_search/anysearch）同题 → 8 分 43 秒、20+ 工具调用、成功且质量高。

**本机核查（三个 profile 的 bundle 对比）：**
1. **OmniDesk web profile（~/OmniDesk/runtime/dsh-home/profiles/web/package.json）：** bundles 仅 dsh-base + dsh-web-app，dependencies 空。**没有任何搜索插件**——模型只能靠自身工具反复尝试，既慢又容易触发上游内容过滤。
2. **用户可用的 desktop profile（~/.dsh/profiles/desktop/）：** bundles 含 dshmarket、dsh-free-search、dsh-cost-meter + web-search-deepseek patch（maxUses 20）+ agent-default-model=glm-5.3-flash。搜索由免费多引擎插件承担，回退链健在。
3. **400「Content Exists Risk」** 是 OpenDesign 网关的内容过滤拒绝（code=upstream_error），原生 DSH 同样命中——**不是 OmniDesk 的 bug**，是模型供应商过滤层对长政治类上下文的拦截；成功那次可能用了不同模型（glm-5.3-flash）+ 不同搜索路径。

**根因结论：** OmniDesk 的隔离 DSH 环境只装了“最小底座”，没有装载用户 DSH 里已验证可用的插件生态。这是配置差距，不是架构缺陷。修复方向 = 把插件生态做成 OmniDesk 可选装的 profile 资产（与 068 规则资产同一模式），并把上游 400 这类真实错误如实透出（当前 OmniDesk 落 unknown/timeout，错误原因被吞）。

## OD-ARCH-070 复核（ARCHITECT，2026-10-23）

**结论：验收通过（超时项除外——已被 OD-ARCH-071 作废，其余成果全部有效）。**

独立核查证据（本机实查代码与运行时落盘，非转述 STATUS）：

- **插件已实装：** ~/OmniDesk/runtime/dsh-home/profiles/web/package.json 的 dependencies + dsh.profile.bundles 现含 dsh-free-search@0.9.5 与 dsh-cost-meter@1.9.6；cordis.patch.yml 含 web-search-deepseek patch（apiKeyEnv DEEPSEEK_API_KEY、maxUses 20）。未配 key 时自动走免费引擎，与用户 desktop profile 同款。
- **出厂资产模式成立：** samples/profiles/web.profile.{json,patch.yml} 与 discipline.rules.json 同模式；seed_profile_if_missing（engine_supervisor.rs，含 4 个播种单测）只在缺失时落盘、已有绝不覆盖、拒绝写 .dsh / .agents / deepseek-harness 树。
- **错误透传核实：** web_api_adapter 将上游 400 upstream_error 映射为 task_failed 并透传原因（截断 300 字），界面显示真实原因而非 unknown/timeout。
- **测试证据：** cargo 246 通过（含 4 个播种用例）、pytest 57、tsc 0、build 通过；合成端点端到端 web_search 实测成功（8.4KB 结果、Bing 200/467ms、无 key 走免费引擎）；证据目录 tests/verification/desktop/evidence/od-arch-070/ 齐全（含 .before.* 回滚备份）。
- **边界：** git status --porcelain runtime/ 为空；用户既有 profile 未被覆盖。

保留项（登记不阻塞）：
1. 本计划中原定的「检索 600s / 其他 300s」超时设定，被用户当轮直接指令作废（见 OD-ARCH-071），错误透传与工具可见化等其余成果不受影响。
2. dsh-free-search 引擎可用性随外网波动，回退链为缓解措施。
3. DEEPSEEK_API_KEY 仍由用户自行决定是否配置，软件不读不存。

## OD-ARCH-071 归档复核（用户直接指令：取消任务自动超时）（ARCHITECT，2026-10-23）

**指令原文要点（用户 2026-10-23）：**「如果 DSH 底座没返回错误，就不要设置这两个时间了……原版 DSH 搜索到任务完成都花了 8 分钟，只要系统还在运行和调用插件就不要停止他，除非用户自己停止。」BUILDER 已按指令实施并请求正式编号归档，本节即为编号归档。

**结论：验收通过。任务终态语义改为「由底座决定」（错误/完成/用户取消），墙钟不再是终态来源。** 这与"引擎状态可丢弃、真实会话归用户"的架构方向一致，方向正确。

独立核查证据：

- **无上限口径：** web_api_adapter.py:414 与 acp_adapter.py:158 的 default_timeout 均默认 None；timeout_policy 缺省/0/负数/非法值一律无上限，仅显式正整数才设截止（保留给测试模拟）。
- **真实缺陷修复核实：** web_api_adapter.py:178——无截止时 sock.settimeout(None)，修复了继承握手 30 秒超时导致长任务被误判断流的问题。
- **静默不终止：** web_api_adapter.py:1441 每 60 秒发 kind=still_running（带 idle_seconds）；HomeChat.tsx:556 渲染「DSH 仍在运行（已静默 N 秒）·不会自动停止」，无失败样式。
- **事件泵续订核实：** serve.py:152-153 按 after_seq 分段续订，替换原 stream_events(timeout=3600) 的一小时截断；只在终态或停止信号退出。
- **宿主不下发超时：** lib.rs 提交参数已无 timeout_policy（rg 仅命中 cancel_task 本地 15s 确认时限，语义正确）。
- **关键场景证据 V05：** 不设上限 + hang 端点 → still_running 到达 → 静默 8 秒仍非终态 → cancel_task 落 cancelled。直接证明「不自动停 + 用户取消有效」闭环。
- **回归全景：** cargo 246 / pytest 55（超时用例改写净减 2）/ tsc 0 / build 通过 / V01–V05 5/5 / W01–W10 10/10 / E01–E20 20/20 / 事件泵复跑 W01 W02 W04 3/3 / 桌面 e2e chain 3/3 + isolation pass + dual-instance pass。
- **边界：** runtime/ 零差异；未提交未推送。

保留项（登记不阻塞）：
1. 「底座进程活着但事件流死锁」目前靠 still_running 提示 + 用户取消兜底，无进程级心跳强校验——列为后续可选增强。
2. 旧配置两个秒数字段为兼容保留（deny_unknown_fields），已不生效；彻底移除留待配置结构升级。
3. 真实 8 分钟级检索全程不断流，需用户用真实模型实测一次最终确认（合成端点覆盖不了外网抖动）。

## OD-ARCH-072（合并版）复核（ARCHITECT，2026-10-24）

**结论：验收通过。** Part A/B/C 全部落地，独立核查证据：

- **新构建确认：** release 二进制 Oct 23 23:58（旧的是 10-19 08:18）、前端 dist 23:56（index-BRPNQdIa.js），strings 实测 release 二进制含 capability_view 注册——用户报错的直接根因（旧二进制无此命令）已修复。
- **启动链路核实：** scripts/omnidesk.sh 直接运行 release 二进制（前端内嵌），needs_build() 只在源码更新时重建，--status 不再依赖 vite。
- **Profile 切换落地：** profile_choice.rs（list/resolve/standard_profile）+ Settings.engine_profile + engine_supervisor 读取；隔离根真实 profile 为 acp/headless/omnidesk-test/web/yvan-work（计划写的「discipline」实际不存在，BUILDER 按真实目录实现，偏差认可）。
- **首页改版：** 日历悬浮（移开 1 秒淡出）、今日重点卡片删除、TodoTicker 单条 5 秒轮动、机器人 104px 列锚点、无历史居中/有历史落底 200ms 过渡；证据 10–17 号截图齐全。
- **测试全景：** cargo 258 通过（含 profile_choice 5、settings 42）、pytest 55、tsc 0、build 通过、verify_home_layout_072 与 verify_capabilities_072 通过、runtime/ 零差异。
- **计划未列明但合理的两处顺带修复：** 空态输入框贴底、顶部渐隐吞消息，均在 Part C 验收范围内。
- **集成测试补跑：** capabilities_plugin_toggle 此前因路径缺陷从未真正运行，本轮修正并实跑通过。

遗留（登记不阻塞，需用户决策）：
1. 22 个早前测试遗留的引擎子进程（约 3.5GB 内存），建议清理并加宿主退出兜底——下个任务处理。
2. dshmarket 未安装，市场开关只验证了拒绝路径；是否授权安装由用户决定。
3. Profile 选择语义为「保存+下次启动生效」，未做「保存即重启引擎」——建议保持现状（避免打断进行中任务）。

## 遗留进程问题复核与 Luna 审计评价（ARCHITECT，2026-10-24）

**Luna 审计判断合理，予以采纳**：P0 = 遗留引擎进程回收、P1 = 通道能力说明、ACP 界面审批与通道/协议重构暂缓；先 P0 后 P1，阶段间暂停。补充本机根因核查证据：

1. **泄漏链路已定位（源码证据）：** 宿主 spawn 的引擎子进程是 `python3 -m omnidesk_engine`（engine_supervisor.rs:84+engine_ipc.rs:132）；引擎再 spawn 孙进程 `node bin.js`（DSH web 进程，web_api_adapter.py:495 的 Popen，**未设 process_group/killpg**）。正常收尾链完整：宿主 Drop → terminate(grace 3s)（engine_supervisor.rs:876）→ 引擎 atexit shutdown(graceful)（serve.py:597）→ _terminate_process 杀 node（web_api_adapter.py:627）。**缺口在异常路径：** 宿主崩溃/kill -9 时，引擎（作为子进程）通常也会死，但若引擎自己被 SIGKILL 或先于宿主死，node 孙进程即成孤儿（被 PID 1 收养，继续监听端口、持有 dsh-home）——22 个遗留进程即历次测试强杀的累积。
2. **多实例并发写风险属实：** 无实例互斥与所有权登记（live/pids 只有 app.pid/vite.pid，且只记宿主）。多个引擎共享 ~/OmniDesk/runtime/dsh-home 时 cost-meter 账本等确实可能并发写——072 验收时已实测到该干扰。
3. **当前系统现状：** 复核时遗留进程已不在（用户重启过机器或自行清理），但泄漏机制仍在，随时会再累积。
4. **Luna 提出的六条回收原则全部合理**（只回收自有进程、身份核验防 PID 复用、温和终止优先、保护合法实例、身份不明宁可停启、可重复测试），写进 074 计划。
## OD-KB-001 P1 独立验收（ARCHITECT，2026-10-26）——通过

**结论：P1 通过，可以开始规划 P2；这不等于批准实施 P2。** 本次只验收 P1「单库根目录与只读访问边界」，未审查其他并行工作区差异，也未读取真实 Vault、正式资料或凭证。HEAD 为 `0d17b1f04f9a9c575643dce855f32db114da516b`；工作区既有暂存、未暂存和未跟踪改动均予保留；`runtime/` 无差异。

**验收要点：**

- 独立的 `knowledge_vault_root` 与 `library_root` 解耦；`OMNIDESK_KNOWLEDGE_ROOT` 仍只影响既有 `library_root`。根准入和后续状态、扫描、读取均在宿主侧重新校验；扫描与读取接口不提供写入能力。
- 扫描对深度、条目、文件数、明细和单篇正文有界；隐藏/排除目录、符号链接、越界路径、非普通文件和不支持正文格式均有明确拒绝或跳过行为，附件只列元数据。
- 审计发现 `read_vault_note` 原先只在读取前检查文件大小，检查后文件若增长会令 `fs::read` 无界读取。我将其改为最多读取上限加 1 字节并拒绝超限，随后重跑相关验证；此改动保持 P1 的只读契约，没有扩展功能范围。
- 设置界面由用户显式选择并手动扫描；对合成 Obsidian 风格目录的扫描前后快照一致，`.obsidian` 内容未被更改，普通 Markdown 根也通过宿主集成测试。

**ARCHITECT 独立运行结果：**

| 检查 | 实际结果 |
| --- | --- |
| `cargo test --offline`（经仓库工具链环境脚本加载） | **323 passed / 0 failed**：库 297、分类 2、能力 1、宿主集成 13 + 知识库宿主 6、任务壳 3、线程存储 1 |
| `cd desktop && npx tsc --noEmit` | exit 0 |
| `python3 -m pytest tests -q` | **89 passed，30 subtests passed** |
| `verify_knowledge_vault_p1.py --display :129` | **14/14 通过**；1201 个合成文件的三次扫描中位数 7.962 ms |
| `git diff --check`（知识库相关已跟踪差异） | 通过 |
| `runtime/` 工作区差异 | 空 |

**不阻断本阶段的未验证边界：** 没有在 Windows/macOS、网络盘或大小写不敏感文件系统上测试；没有压力测试超大规模 Vault；没有实测扫描与外部修改/删除并发发生时的完整一致性行为。上述场景不改变当前单机 Linux 合成目录的通过证据，后续阶段应继续以冲突检测和可恢复行为为边界，不得默认为跨平台已验证。

**阶段决定：P1 验收通过。** P2 仍需单独小计划并经用户批准后方可实施；本结论只开放 P2 规划。
## OD-KB-001 P2 修复复验（ARCHITECT，2026-10-26）——发现额外竞态，P2 未通过，P3 暂停

**结论：P2 暂不通过，暂不制定 P3 计划。** P2-F01/F02 的主要修复符合原验收意图，但复核有界文件读取时发现一个确定性可达的并发边界：读到 EOF 后、读取句柄元数据前，文件增长到仍小于读取上限时，当前实现只检查最终长度是否超过 cap，没有检查已读字节数是否等于最终文件长度，因此可能把较短内容误作完整笔记并纳入搜索。该情况违反 P2-F01「部分读取不得误报完整」标准。

**本次证据范围：** HEAD `72e7b05db40bdf4047b7839be9b0cb6abd4bbe32`，分支 `main`；最初工作区干净、与 `origin/main` 同步；修复仓库期间工作区差异，终了检查已恢复到干净状态。未读取真实 Vault、业务资料或凭证；未访问 `runtime/`。P2 所有原有证据文件均在本次窗口复验前备份并恢复，Git 最终无差异。

**独立复核：**

- F01 的剩余预算按每次有界读取传入，实际读取量被 `take(cap)` 限制，搜索按 `bytes_read` 记账；文件超过 cap 时不返回正文，并把该文件及后续文件记为未检查。扫描后增长的 65 文件合成测试覆盖恰好用尽 32 MiB 的情况。
- F02 以 `VAULT_WRITE_EXTS = [md, markdown]` 独立限制创建/保存，`.txt` 保持可读/可搜索；宿主测试确认拒绝创建与覆盖且磁盘内容不变。
- 宿主侧搜索与写入测试、全量 Rust、TypeScript、生产构建、pytest 的此前实测结果见 STATUS.md；本轮未重复这些低风险重复项。
- 有界读取 EOF 竞态的确定性分类回归 `bounded_read_completeness_rejects_growth_after_eof` 实跑 **1/1 通过**，覆盖读取长度与最终句柄长度不一致但仍在 cap 内、越过 cap、以及长度相同三种情况。
- 窗口验证脚本 `verify_knowledge_vault_p2.py` 本轮尝试两次，但运行至 UI/AX 交互阶段未继续推进，均由我中断；不能把此前执行者记录的 33/33 当作本轮独立窗口复验。原始 `tests/verification/desktop/evidence/od-kb-001-p2/` 文件已恢复，未以部分运行生成物替换；故窗口级复验仍未完成。
- 最终 `git status --short --branch` 为 `## main...origin/main`；`git diff --check HEAD^ HEAD` 通过；`runtime/` 工作区差异为空。

**本轮最小修正：** 在 `read_vault_note_within` 把有界读取结果分类为完整、预算超限、读取期间变化；只有 `bytes_read == actual_len` 且 `actual_len <= cap` 才返回正文。新增回归测试使用确定性纯函数验证 EOF 后文件增长仍不被视为完整内容，无需制造不稳定的文件系统竞态。该改动仅用于保证已批准 P2-F01 契约，不引入 P3 范围。

**剩余门槛：** 需先排查并修复/解释窗口脚本在本机 AX 阶段停滞的原因，重新独立完成 P2 33 项窗口验证，再检查证据目录和 Git 状态；通过后方可将 P2 标为通过并规划 P3。P3 仍暂停。

## OD-KB-001 P2 补验与阶段验收（ARCHITECT，2026-10-26）——通过

**结论：P2 通过，开放 P3 计划审查。** 先前复验发现的 EOF 后文件增长问题已加入 `classify_bounded_read` 完整性分类；只有本次读取字节数等于最终句柄长度且不超过 cap 才返回正文。完整 P2 窗口流程现已独立重跑成功，确认修复后的实际界面与宿主行为仍满足批准验收条件。

**范围与保护：** HEAD `72e7b05db40bdf4047b7839be9b0cb6abd4bbe32`，分支 `main`。原 `od-kb-001-p2/` 证据在运行前备份，结束后逐文件恢复并与备份递归比对一致；新一轮证据保存于 `tests/verification/desktop/evidence/od-kb-001-p2-recheck/`。窗口使用 `/tmp/odkb-p2-ui-final` 合成 Vault，未读取真实 Vault、业务资料或凭证；`runtime/` 无差异。仅保留当前任务新增的 Rust 回归、P2 窗口脚本服务复用改动和独立复验证据，其他工作区差异均保留。

**独立实跑结果：**

| 验证 | 实际结果 |
| --- | --- |
| P2 桌面窗口验证 | **33/33 通过，0 失败；21 张截图**，证据 `tests/verification/desktop/evidence/od-kb-001-p2-recheck/facts.json` |
| 有界读取 EOF 增长回归 | **1/1 通过**，覆盖完整、超预算、读取期间文件变化 |
| `cargo test --offline vault_search::tests` | **9/9 通过**，含 32 MiB 实际累计预算与 1200 篇规模搜索 |
| `cargo test --offline --test knowledge_vault_write_host` | **10/10 通过** |
| `cargo test --offline` | **357 项通过，0 失败**（库 327、分类 2、能力 1、宿主 13、知识库只读 6、知识库写入 10、任务壳 3、线程重启 1） |
| `python3 -m pytest tests -q` | **89 passed，30 subtests passed** |
| `npx tsc --noEmit` 与 `npm run build` | 均通过；构建保留既有的大 chunk 提示 |
| `python3 -m py_compile tests/verification/desktop/verify_knowledge_vault_p2.py` | 通过 |
| `git diff --check`；原 P2 证据恢复比对 | 均通过；旧目录与备份无差异 |

**独立代码审查：** 搜索将剩余预算传入有界读取并按实际字节计账；无法返回完整内容的文件不会作为命中参与结果，并产生跳过/预算状态。写入扩展名单独限定为 `.md` / `.markdown`，`.txt` 仍可只读搜索。新增 EOF 后增长分类避免把短读内容误作完整正文。窗口验证显示配置提示、Frontmatter/属性、预览与原文、创建和保存、外部修改冲突保护、标签/属性筛选、附件显式打开、刷新增删可见性、草稿离开/重启保护均符合契约；Obsidian 配置与合成 Vault 其他未触碰文件保持完整。

**验证边界：** 1200 篇合成搜索读取 24,937,046 字节（32 MiB 预算内），三次中位约 3,459 ms，是 Linux x86_64 debug 环境观测，不是性能承诺。未测试 Windows/macOS、网络盘或真实大 Vault；未验证跨进程写入与最终替换之间无法由普通文件 API 消除的竞态。全量 Cargo 输出有一个既有 `worklog::AppendReceipt::failure` dead-code 警告；生产构建有既有超过 500 kB chunk 警告。

**阶段决定：P2 验收通过。** 可开始审查 P3 详细计划；P3 实施仍需用户单独批准。

## OD-KB-001 P2 独立验收结论（ARCHITECT，2026-10-26）——未通过，P3 暂停

**结论：P2 暂不通过。** 核心读写与浏览功能的独立定向测试通过，但发现两处违反已批准 P2 验收标准的边界问题：搜索的总读取预算根据过期扫描元数据预判，不能保证按实际读取量封顶；Markdown 写入接口允许创建或保存 `.txt`。据阶段门槛，不能进入 P3。已在 PLANS.md 加入最小修复计划 P2-F01～F03，待用户批准后实施。

**独立复核范围与证据：** 当前 HEAD `adc049ddb8e667f2b2fe236670e4a70deb96632e`，分支 `main`，开始与结束时工作区均干净且与 `origin/main` 同步；本轮未修改源码。未读取真实 Vault、业务资料或凭证。审查了 P2 批准计划、STATUS 执行记录、P2 源码、宿主测试、窗口脚本及合成证据；`git diff --check 5ad89bc..HEAD` 通过，`runtime/` 无差异。

**独立实跑结果：**

| 验证 | 结果 |
| --- | --- |
| `cargo test --offline vault_search::tests -- --nocapture` | 7/7 通过；1200 篇合成规模搜索读取 24,937,046 字节（上限 33,554,432），中位约 3,522.652 ms |
| `cargo test --offline --test knowledge_vault_write_host` | 7/7 通过 |
| `python3 -m pytest tests -q` | 89 passed，30 subtests passed |
| `git diff --check 5ad89bc..HEAD` | 通过 |
| `git status --short runtime/` | 空；runtime 无差异 |

实施者记录的 `tsc`、生产构建及 33 项窗口验证为已审查的既有实测证据；本轮未重复窗口启动和前端构建，因为两项静态边界缺陷已足以阻断验收。

**阻断问题：**

1. **高：搜索读取预算不以实际读取字节强制。** `desktop/src-tauri/src/vault_search.rs::search_scanned`（预算判断处约第 267 行）在读取前用 `VaultFile.bytes` 决定是否允许读取，读后才按 `note.bytes` 累计。若扫描后较早的多个笔记增长，读取后的累计值可能越过 `SEARCH_MAX_BYTES`；仅当后续条目触发比较时才观察到超额，且那时超额已发生。当前单测 `budget_exhaustion_is_reported_not_hidden` 只注入一条超预算元数据，并未覆盖“元数据低估、实际文件增长”的情形；规模测试也只测稳定合成库。该行为不满足 P2-S05 的“每次查询最多读取 32 MiB”与“不得无界读取”标准。单文件 512 KiB 和最多 2,000 个文件把理论最大读取量限制在约 1 GiB，但这远高于 32 MiB 契约，不能替代总预算。**要求：** F01 必须以实际可控读取量记账并加入扫描后增长的回归；确认预算耗尽时不会把部分读取误当作完整检索结果。

2. **中：Markdown 创建/保存接口允许 `.txt`。** `desktop/src-tauri/src/knowledge_vault.rs::VAULT_NOTE_EXTS` 包含 `md`、`markdown`、`txt`，而 `vault_write.rs::check_target_shape` 将其整体用作写入白名单；宿主命令 `create_note` / `save_note` 因此可写 `.txt`。这虽与 P1 的纯文本读取能力一致，却超出 P2-S02“根内 Markdown 路径”以及“拒绝非 Markdown 目标”的明确验收标准。**要求：** F02 将写入白名单限定为 `.md` / `.markdown`，保留 `.txt` 的读取和搜索支持，并测试拒绝后文件内容不变。

**已核对且通过的边界：** 写入目标其余路径校验复用 P1 根内解析；新建使用临时文件加硬链接发布，已存在目标不会覆盖；保存在写临时文件前重读并比较 SHA-256，冲突时不写入；保存后回读；跨进程重读至替换之间的竞态在响应中明确说明，属计划已声明的限制。附件打开先做根内路径校验且不回传附件正文，测试注入合成 opener。Frontmatter 只读解析保持原文片段，不规范化写回。前端草稿确认和冲突保留及窗口级证据见 STATUS 与 `tests/verification/desktop/evidence/od-kb-001-p2/`。

**已知但不单独阻断本次验收的边界：** `.txt` 被 P1 分类为可读笔记，因此本次判定聚焦其通过 Markdown 写接口可写的不一致；Linux 测试中的硬链接创建与 `xdg-open`/`gio` 附件打开不能证明 Windows/macOS 兼容，STATUS 已如实标为未验证，跨平台验收需在后续发布门槛覆盖。跨进程保存竞态也仍存在，但代码和已批准计划均披露，故不是本次新发现的偏差。

**阶段结论：** P2 当前未通过；F01/F02 修复并完成 F03 独立复验后再判断。P3 保持暂停，本轮不制定 P3 计划。
