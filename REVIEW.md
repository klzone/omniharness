# OD-PROC-149 审查：进程名归一 omnidesk-*（ARCHITECT，2026-11-11）——通过

**结论：通过。** 149a/149b 两卡由并行子代理实施，独立复检全部实跑验证通过；两处打包/断言适配已由 ARCHITECT 修正。可进入用户实测与出包。

**计划符合性：** 149a 副本复制+marker+探针回退+开关+非 Windows 透传，全部与获批计划一致；149b shim 外部注入（runtime/ 零改动）+darwin 分支+setproctitle 静默降级，一致。process_owner.rs 的 is_node_image 联动属任务卡正文明确要求（防回收失明），两平台分隔符兼容处理正确。

**独立验证（本会话实跑）：** cargo lib **1103 passed / 3 failed**（3 项为既有 process_owner 沙箱进程表限制，与改动无关；process_alias 8 项与 engine_image_accepts_alias_names 全绿）；tsc 0；verify_proc_alias_149 **34/34**（含 node 实跑探针 title=omnidesk-dsh）；web_api adapter 单测 **89/89**（修正断言后）；verify_docx_148 14/14、verify_diagnose 15/15 不回归。

**ARCHITECT 修正两处：** ①test_web_api_unit.py cmd[:4] 精确断言在 darwin shim 注入后失效，改为剔除 --require 前缀后断言相对顺序（复跑 89/89）；②tauri.macos.conf.json resources 补 scripts/proc_title_shim.cjs 映射（149b 报告指出打包后 shim 缺失会静默不注入，实查构建产物证实）。

**派工偏差：** 用户指定 deepseek-v4.1-flash-eco；派工系统可用清单无 -eco 变体（该 id 仅存在于 OmniDesk 产品路由），实际按 OpenDesign/deepseek-v4.1-flash + high 派发。等派工系统补 -eco 路由后切换。

**未验证项（用户实测）：** Windows 任务管理器显示 omnidesk-node.exe/omnidesk-python.exe 分组；python 副本 DLL 探针真机结果（失败自动回退原名，无回归风险）；macOS 活动监视器显示 omnidesk-engine/omnidesk-dsh。

\n# OD-DOCX-148 审查：附件广收窄取（ARCHITECT，2026-11-10）——通过\n\n**结论：通过。** 用户另一会话实施的 148a-d 与获批计划一致且质量良好；本会话独立复检全部实跑验证通过，可进入用户实测与出包。\n\n**计划符合性（逐卡核对 diff）：**\n\n- 148a：接收层不再有格式白名单拒收（除硬边界），MaterialRead.content_kind/note 三端同步，「另存为」话术删除，前端 chip 带「正文未提取」徽标。✔\n- 148b：stdlib docx 提取（w:t/tab/br/表格）+ 可选 pypdf；JSON 契约 {kind, content/reason}；include_str! 内置。✔\n- 148c：requirements 加 python-docx+lxml；universal 包删 .so 收窄到 charset_normalizer（计划内指定的坑已修）。✔\n- 148d：python_docx/python_pypdf 环境项 optional（requirement=可选，不进整体 ok）。✔\n\n**独立验证（本会话实跑）：** cargo lib **1094/3**（3 项 process_owner 沙箱限制，与本次改动无关；新增 attachment/docx/平台测试全 ok）；tsc 0；verify_docx_attachment_148 **14/14**；verify_docx_vendor_148cd **9/9**；verify_diagnose 15/15 不回归；vendor 探针 import docx+lxml 实测通过（1.2.0/6.1.3）。\n\n**安全复核要点：** 禁用目录断言与 resolve_real_path 先于 helper；helper 拒绝 DOCTYPE/ENTITY（XXE）；解压总量与 document.xml 双上限（env 可调）；6 秒 try_wait+kill 超时不阻塞；临时目录 pid+序号、失败清理；sha256 改为原始字节（比原实现更准确）。\n\n**偏差与增项登记：** ①lib.rs desktop_user_name 改 async——不在 148 计划内，属修复 147 复检发现的 HEAD 既有测试失败，方向正确予以追认；②release.yml 加两平台 vendor 探针步——148c 验收要求的 CI 证据，追认；③另一会话未更新 STATUS/REVIEW/PLANS 状态行，由本次复检补齐。\n\n**未验证项（用户侧）：** Windows 安装包 helper/vendor 探针（CI 出包日志确认）；真实模型对 metadata_only 附件按路径读取的行为；.doc/.wps 转换仍属增量能力。\n
# OD-ARCH-147 审查：免 key 搜索全局生效与检索收敛（ARCHITECT，2026-11-10）——通过（附遗留）

**结论：通过。** 三张任务卡均已实施并经独立复检；可进入用户实测。遗留两项待裁定事项见文末，不阻塞生效。

**复核范围（实读 diff + 实跑命令，非仅凭代理自述）：**

- 147b（ARCHITECT 亲自实施）：`scripts/diagnose-omnidesk.py` 新增 web_snapshot（search_provider_effective / takeover 行 / profile_bundles，仅元数据）；verify_diagnose_omnidesk.py 扩至 15/15。真实机运行输出 discipline profile bundles=[dsh-base, dsh-web-app]、provider=None，与排查结论一致。
- 147a：三 profile json dependencies、base/discipline patch insert 块（内容与 web 出厂条目一致）、prepare-runtime-assets.sh seed 加 discipline；147a 代理会话关闭未交报告，ARCHITECT 补齐计划内遗漏 DISCIPLINE_PLUGINS 两插件并复原证据 147a-summary.md。
- 147c：代理完整交付。机制核实链：dsh-agent-instructions Config 无注入字段（实读）→ 改用 persona.suffix → applyEntryPatches 只索引顶层行、preset-standard 非 group，定向 patch 实测报 entry not found → web/discipline 整段 restate 上游 standard.patch.yml（js-yaml 结构化对比仅 persona suffix 差异）。隔离 DSH_HOME --dump-config 三 profile rc=0 且含规则文本。

**独立验证（本会话实跑）：**

| 验证 | 结果 | 证据等级 |
| --- | --- | --- |
| cargo test --offline --lib（RUST_TEST_THREADS=4） | 1090 passed / 4 failed；4 项失败均与本改动无关（desktop_user_name 为 HEAD 既有同步命令，git show HEAD:lib.rs:852 实证；process_owner 3 项为沙箱进程表限制） | 脚本运行 |
| tsc --noEmit | exit 0 | 脚本运行 |
| verify_diagnose_omnidesk.py | 15/15 | 脚本运行 |
| verify_plugin_scope_104.py | 6 failed：3 项 104 时代纪律断言（insert 唯一性/不得顶层覆盖/不得 disabled）与获批机制变更冲突；3 项环境路径失败 | 脚本运行 |
| 真实模型行为（无 r.jina.ai、3 次/15 步收敛） | 未验证，待用户下次会话实测 | 未验证 |

**待用户裁定：** verify_plugin_scope_104.py 的三项纪律断言按旧设计钉死「补丁只能加 insert」，与本次批准的全局装配+persona restate 机制冲突；需决定是修订断言口径（推荐：改为「新增 insert 允许多条；顶层 id 覆盖仅限 preset-standard/persona」）还是回退机制。脚本本身不在本次授权修改范围，未动。

**派工偏差登记：** 用户指定 deepseek-v4.1-flash-eco；派工系统可用清单无 -eco 变体（ OmniDesk 自身模型目录中有该 id，属产品路由，非子代理路由），按最接近的 OpenDesign/deepseek-v4.1-flash + high 执行，与 OD-KM-500 历史口径一致。

## OD-ARCH-079 补充审查：线程分支来源（ARCHITECT，2026-11-10）——通过

**结论：通过。** 分支到新对话后，线程记录保存父会话与切点，新线程顶部显示一行灰色来源说明；普通新建线程不显示。复核中发现并修复一个边界：`createThread` 原先直接写入 `input.forkOrigin`，畸形对象虽然读回时会被丢弃，但当前会话仍可能短暂显示空父会话号。现改为写入端也走 `normalizeForkOrigin`。

**复核范围：** `sceneThreads.ts` 的字段、读取归一化与创建归一化；`HomeChat.tsx` 的回调签名、分支成功回传与 `fork-origin-banner`；`HomeCanvas.tsx` 的来源透传；`thread_store.rs` 对未知字段的原样保存约束。未修改分支引擎协议或消息内容。

**验证：**

| 验证 | 结果 |
| --- | --- |
| `verify_fork_origin_202.py` | **9/9** |
| `cargo test --offline --lib thread_store::tests` | **13/13** |
| `npm run build` | 通过 |

**已知限制：** 当前会话内已经存在的畸形 `forkOrigin` 不会补造父会话号；读回时按缺失处理。真实窗口的分支点击与提示条外观仍需用户验收。

## OD-DIAG-001 审查结论（ARCHITECT，2026-11-10）——通过

**结论：通过。** 普通诊断改为“一次聚合读取 + 8 轮预算”：新增 `scripts/diagnose-omnidesk.py`，一次给出进程、相关端口、默认网络、模型 endpoint host、最近任务元数据和日志元数据；任务正文、错误正文、workspace_dir 与凭证不进入输出。`starting` 且快照超过 30 秒的任务标记 `observer_may_be_stale`，为后续 P2 状态观察器修复提供证据。

**独立验证：** `verify_diagnose_omnidesk.py` **10/10**；合成任务正文与 `api_key` 均未出现在 JSON/文本输出；脚本 `--json` 与文本模式退出码 0。仅新增脚本、AGENTS 诊断预算、验证脚本与状态记录；未改 UI、引擎、模型路由或凭证。

## OD-QS-413 审查（ARCHITECT，2026-11-09）——通过。第二阶段（OD-QS-401～413）全部竣工

### OD-SYS-703 复检通过 + OD-SYS-700 竣工（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 执行代理（Poincare）交付的等待计时器修复与引擎空闲提示。

**核对结果：**
1. diff 允许清单核对：HomeChat.tsx 计时器起点改读 startedAtMs（切页回来恢复真实等待秒数，找不到时显示 0 不伪造）+ 引擎闲置提示（终态后 30 秒去抖、engine_health 确认、60 秒复核、只提示不自动停）+ SettingsPanel 手动停止按钮补齐 + verify 脚本；701/702 交付零重写；runtime/ 零改动。
2. 独立复跑：tsc exit 0、build exit 0、verify_sys_timer_703 = 13/13；cargo lib = 1093 passed / 0 failed（与 701/702 收口一致，本卡无 Rust 改动）。
3. 执行裁决复核认可：空闲证据口径（终态轮询+engine_health.running）；去抖 30 秒/复核 60 秒；找不到 startedAtMs 宁可显示 0 不伪造；设置页手动停止按钮补齐（提示指向真实可用的动作）。
4. 登记未验证：真实窗口计时恢复与提示时机（静态检查+构建覆盖，真实窗口随下次使用验证）。

**OD-SYS-700 阶段结论：** 三卡全部竣工。四项用户报告问题的处置：①检索卡死=702 依赖补齐+依赖自检（附 701 识别的续接会话僵尸记录标记）；②工具详情=OD-ARCH-069 有意设计（维持）；③计时归零=703 修复；④Windows 进程残留=701 Job Object 方案（真实回收行为待 Windows 出包人工验证——下次出包验收时做：启动引擎→跑任务→关窗→确认无残留 node/python）。
### OD-SYS-701 / OD-SYS-702 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 两个并行执行代理（deepseek-v4.1-flash-eco/high）交付：701 Windows 进程树回收与僵尸会话清理（Newton）、702 Python 依赖补齐与依赖自检扩展（Boole）。

**核对结果：**
1. diff 允许清单核对：701 新增 job_object.rs（kernel32 FFI，Windows Job Object kill-on-close）+ engine_ipc 挂接 + supervisor 日志措辞 + process_owner pgid=0 跳过 + task_core 僵尸扫描（48h 阈值，标记不删除）+ verify 脚本；702 platform.rs python_modules 探测 + EnvironmentCheck 显示项 + types 同步 + SKILL 依赖说明 + scripts/python-requirements.txt + 打包准备脚本补装步骤。两卡文件零交叠；runtime/ 零改动。
2. 独立复跑：RUST_TEST_THREADS=4 cargo lib = 1093 passed / 0 failed（净增 11：701 项 6 + 702 项 5）；verify 701 = 40/40、702 = 13/13；stale 单测 9 项 OK；tsc exit 0；build exit 0。敏感词 0 命中。
3. Windows 行为登记：Job Object 无法在 macOS 真实运行验证，已用 x86_64-pc-windows-msvc 对 job_object.rs 单独类型检查通过；真实回收行为待 Windows 出包后人工验证（启动引擎→跑任务→关窗→确认无残留 node/python）——如实登记不夸大。
4. 关键裁决复核认可：701 fail-closed（挂接失败拒绝启动引擎，避免无保护孤儿树）；不引 windows-sys 依赖（kernel32 FFI）；僵尸扫描放 TaskCoreBase（两条通道都经过）。702 依赖清单实位裁决（DSH runtime.json 是上游产物，仓库侧补 requirements + 打包脚本补装，当前包不带 primary-runtime 时由环境检查给 pip 命令）。
5. 遗留登记：a) Windows 真实回收验证（出包后人工，用户下个 Windows 包验收时做）；b) requests 彻底内置需把 Python payload 纳包（超出本卡，单独立项再议——与用户「单包全内置」方向一致，放下一轮打包任务）。

**结论：** 两卡均通过。OD-SYS-703（等待计时器+空闲巡检）可派发。
### OD-WT-615 复检通过 + 第四阶段（OD-WT-600）竣工（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 收口执行代理（Sartre）交付的全量回归、verify_security_phase4.py（35 项）与十三场景对照。

**核对结果：**
1. 本会话独立复跑全量：cargo --tests 13 目标全 ok（0 行含 failed，1149 passed）；pytest 217 passed + 30 subtests；build exit 0；tsc exit 0；verify_security_phase4 = 35/35。全部为 ARCHITECT 实跑。
2. 安全十项口径核对：新 IPC 闸门 11 项、AI 无定稿/归档/导出写通道 4 项、大纲门禁 3 项、未确认素材 3 项、导出安全 3 项、归档仅 MD 4 项、无敏感日志、测试全虚构、无新增豁免、runtime 零改动、前端只走 IPC——全部 PASS。
3. 十三场景 13/13 有证据；专项回归 601 23/23、609 31/31、610 26/26、611 28/28、612 28/28、613 29/29、614 23/23 全绿；跨阶段 phase3 安全脚本 25/25。
4. 收口裁决复核认可：610 断言随 613 四页签重构同步修正；phase4 豁免扫描将 export_engine 三处有界 lint allow 登记为既有允许项（非本卡新增）；phase3 安全脚本两处跨阶段断言同步修正（保留素材篮排除与禁止测试跳过断言）。
5. 用户侧提交核实：821b1a4cf3（wip 快照 0.2.12）与 d1195c9700（Windows os.getpgrp 修复 0.2.13）均与写作流零交集，如实登记不动不推送。

**阶段结论：** 第四阶段（OD-WT-601～615，15 项任务）全部竣工并通过逐任务独立复检。闭环：写作需求→写作任务（九态）→素材收集→素材包→大纲确认→分章节写作→事实核验→审阅修订→定稿→导出（MD/Word 公文/PDF 阅读/公众号 HTML）→归档（MD 进 vault 报告目录）→知识回流（raw 收件待审）。待办：① 用户批准后提交推送；② 真实窗口目视（工作台/大纲编辑/章节编辑/核验/版本/归档六处）追认；③ 真实模型质量与 DOCX 真实格式兼容列未验证事项。
### OD-WT-614 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 执行代理（Ampere，deepseek-v4.1-flash-eco/high）交付的归档与知识回流。

**核对结果：**
1. diff 允许清单核对：vault_archive.rs 新增 archive_writing_final（只落 报告/文稿-*.md）；lib.rs 两命令（writing_archive 含 preview、writing_knowledge_feedback）；types/api 封装；WritingArchivePanel 挂 Workbench 归档页签；verify 脚本。601-613 交付零重写；runtime/ 零改动。
2. 独立复跑：RUST_TEST_THREADS=4 cargo lib = 1082 passed / 0 failed（净增 12）；verify_writing_archive_614 = 23/23；tsc exit 0。
3. 场景十/十一断言实跑确认：归档回执含 writing_id/title/素材知识清单/八类核验计数/版本/导出清单/vault 路径；未勾选内容不进治理通道；重复归档拒绝；仅 MD（DOCX/PDF/HTML 不入 vault）。
4. 安全链路核对：归档前置校验已定稿（601 迁移表门禁）+敏感词扫描；知识回流逐项过 gate_user_text 且走 admit_raw_files_with_policy 进 raw 收件待审队列（AI 文稿不直接成知识条目，经既有治理——照 D-091）；AI 无归档写通道。
5. 执行裁决复核认可：正文取 compose（结构完整）+定稿快照作版本锚点；raw 收件模式（无需伪造来源路径）；preview 并入主命令（命令面最小）；UTC 日期+路径字符清洗。
6. 异常提交核实：用户侧新增 d1195c9700（Windows os.getpgrp 引擎修复 0.2.13，4 文件，与写作流零交集）；照 821b1a4cf3 先例如实登记不动不推送。

**结论：** 通过。最后一张任务卡 OD-WT-615（全量回归+安全核对+收口）可派发。
### OD-WT-613 复检通过（2026-11-09，ARCHITECT 独立复核；含会话中断处置记录）

**事件与处置：** 613 首个执行代理（Chandrasekhar）会话异常中断（completed null）。现场核查：后端主体已交付（export_engine.rs 引擎主体 + lib.rs 三命令注册 + DOCX 转换脚本 + 三套样式文件），但 export_engine 零测试、前端三封装缺失、四组件未挂载、verify 脚本与证据缺失。派发补完代理（Noether）按「补完不重做」口径接续，未重写既有实现。

**核对结果：**
1. diff 允许清单核对：Part A export_engine.rs（含 25 项测试）+ writing_docx_export.py + 三套样式文件 + lib.rs 三命令 + verify_writing_export_613.py；Part B types/api 三封装 + WritingWorkbench 四组件挂载 + 612 断言修正。601-612 交付主体零重写；runtime/ 零改动。
2. 独立复跑：RUST_TEST_THREADS=4 cargo lib = 1070 passed / 0 failed（净增 25）；verify_writing_export_613 = 29/29；verify_writing_review_612（修正后）= 28/28；tsc exit 0。敏感词 0 命中。
3. 集成缺口五项闭环核对：四组件挂载（WritingWorkbench 任务详情四页签，8 处引用）；定稿按钮接 writingStatusTransition IPC（用户触发+finalize_snapshot 快照联动）；核验/引用记录查询 IPC 就绪；612 not_mounted 断言修正为挂载链断言。
4. 安全核对：导出前敏感词扫描（red_line_scan 命中拒绝）；导出本地处理无网络调用（测试断言）；状态迁移经 601 合法迁移表（门禁测试）；DOCX 双路径回执含 simplified 标注。
5. 执行裁决复核认可：挂载位置落 Workbench（避免死导入）；DOCX/PDF 测试用受控假可执行文件（不依赖本机安装状态）；既有两条 #[allow] 来自上一会话实现未新增。
6. 登记未验证：真实 Word/LibreOffice/textutil/CUPS 格式兼容性（静态+假路径测试覆盖语义，真实渲染列 615 未验证事项）。

**结论：** 通过（含中断处置闭环）。OD-WT-614（归档与知识回流）可派发。
### OD-WT-610 / OD-WT-611 / OD-WT-612 复检通过（2026-11-09，ARCHITECT 独立复核，附 612 断言修正说明）

**审查对象：** 三路并行执行代理（deepseek-v4.1-flash-eco/high）交付：610 写作工作台页（Epicurus）、611 大纲编辑器+分章界面（Godel）、612 核验面板+版本界面（Banach）。

**核对结果：**
1. diff 允许清单核对：610 新增 WritingWorkbench.tsx + WritingStudio 页签挂载 + writing_list 只读命令 + verify 脚本；611 新增 WritingOutlineEditor/WritingSectionEditor 组件（不挂载，export 接口交付）+ verify 脚本；612 新增 WritingFactCheckPanel/WritingVersionPanel 组件（不挂载）+ verify 脚本。runtime/ 零改动；601-609 交付零修改。
2. 独立复跑：build exit 0、tsc exit 0；verify 610 = 26/26、611 = 28/28；verify 612 本会话复跑 27/28——唯一失败项 not_mounted（断言 WritingStudio.tsx git 状态为空，但 610 合法修改了该文件用于页签挂载）。判定：断言与并行交付时序冲突，非实现缺陷；613 挂载完成后该断言自然失效，613 卡内修正该断言为「组件已挂载且页签可达」。
3. 610 R1（追加只读 writing_list 命令——任务列表必需且 601 函数已存在，薄封装合理，认可并核对无同名冲突）；R2（默认打开任务页签）；R3（问题清单并入 requirements）。611 R2（章节确认后自动核验随车传 annotations——609 遗留闭环，认可）；R3（重新生成受 606 门禁限制不发起必然失败请求）；R5（模型调用复用 host.submitTask 现有通道）。612 R1（核验记录无读取 IPC，不越界）；R2（状态迁移 IPC 缺失，定稿按钮禁用并标注，登记 613 补）。
4. 三卡共 5 个未挂载/缺 IPC 缺口汇总至 613 修补：a) 611/612 四组件挂载进 WritingStudio；b) writing_status_transition IPC（待审核→已定稿，用户触发+快照联动）；c) writing_records 读取 IPC（citations/factcheck 列表查询，603 函数已存在只缺命令）；d) 612 verify not_mounted 断言修正。
5. UI 参考核对：610（Flow/Dovetail/Asana）、611（Notion/Craft/Condens）、612（Craft/Dovetail/Flow）均有留痕。

**结论：** 三卡均通过（612 附断言修正项至 613）。OD-WT-613（导出引擎与样式模板 + 集成修补）可派发。
### OD-WT-608 / OD-WT-609 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 两个并行执行代理（deepseek-v4.1-flash-eco/high）交付：608 审阅修改（Mencius）、609 事实核验（Harvey）。

**核对结果：**
1. diff 允许清单核对：608 新增 writing_review.rs + lib.rs 五命令 + types/api；609 新增 fact_check.rs + lib.rs 两命令 + verify 脚本 + types/api。601-607 交付物只读零修改；runtime/ 零改动。
2. 独立复跑：RUST_TEST_THREADS=4 cargo lib = 1045 passed / 0 failed（608 净增 15 + 609 净增 26）；verify_fact_check_609 = 31/31；tsc exit 0。敏感词 0 命中。
3. 异常事件核实：执行期间出现非本流水线的提交 821b1a4cf3「wip(writing): 第四阶段写作底座」（作者 Yvan/klzone，内容为 601-607 已复检交付 + 608 初稿 + VERSION 0.2.12 + CI 出包口径）。两个执行代理报告均声明非己所提交。该提交内容与已复检交付一致、无越界文件，判断为用户或用户侧自动化（Windows CI 出包需求）所为；本会话不动它、不推送，如实登记。工作区剩余未提交改动 = 608 终稿 + 609 全部交付，归属清晰。
4. 场景六/七/八断言实跑确认：正确数据→直接来源、冲突优先于直接来源（错误数据不判已核实）、无来源→无法找到来源不编造；口径冲突只提示不合并；场景八多版本差异可查、恢复生成新快照且历史完整。
5. 安全链路核对：AI 审阅只出问题清单不改文（修改走用户应用）；factcheck 处理状态仅 writing_factcheck_resolve 用户触发；重跑跳过已存在编号不重置用户处理状态（609 R3 认可）；知识库只读。
6. 执行裁决复核认可：608 R1（复用 607 confirm_section 唯一写路径）、R3（大纲/正文不互比）；609 R1（标注随请求合并不落 sidecar）、R2（知识依据只读来源）、R5（备注过闸门不落盘）。
7. 登记限制：机构名/职务启发式非 NER；607 标注未持久化需界面随车传入（611/612 注意）。

**结论：** 两卡均通过。批次四可派发：OD-WT-610（工作台页）∥ OD-WT-611（大纲编辑器+分章界面）∥ OD-WT-612（核验面板+版本界面）→ 613 → 614 → 615。
### OD-WT-607 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 执行代理（Bernoulli，deepseek-v4.1-flash-eco/high）交付的 writing_section.rs 分章节写作。

**核对结果：**
1. diff 允许清单核对：新增 writing_section.rs + lib.rs 四命令 + types/api + evidence；601-606 交付物只读零修改；runtime/ 零改动。
2. 独立复跑：RUST_TEST_THREADS=4 cargo lib = 1004 passed / 0 failed（净增 19）；tsc exit 0；敏感词 0 命中。
3. 大纲确认门禁核对（安全断言核心）：writing_section_generate 前置校验任务状态=写作中 且已确认大纲该节存在——「大纲未确认不得生成正文」在 IPC 层落实；生成只允许写作中、确认/修订允许三状态（607 R4 核验审核返修闭环，认可）。
4. AI 无写通道核对：模型输出只进提示词回执侧（界面侧调用），落盘走 writing_section_confirm 用户命令；未确认章节不写入正文.md（607 R2——正文.md 只含已确认内容，导出走组合函数，认可）。
5. 执行裁决复核认可：R1（章节素材回退 selected 集）、R3（修订复用 confirm 产生快照+追加引用历史）。
6. 登记待 608/609 消化：多次修订后 citations.jsonl 旧引用与最新版本的对应关系（603 记录含 linked_at 可按时间取最新，609 对账时按 record_id 处理）。

**结论：** 通过。批次三余下两卡可并行派发：OD-WT-608（审阅修改）∥ OD-WT-609（事实核验）。
### OD-WT-606 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 执行代理（Volta，deepseek-v4.1-flash-eco/high）交付的 writing_outline.rs 大纲生成与确认。

**核对结果：**
1. diff 允许清单核对：新增 writing_outline.rs + writing-templates.v1.json + lib.rs 三命令 + types/api + evidence；601-605 交付物只读零修改；runtime/ 零改动。
2. 独立复跑：RUST_TEST_THREADS=4 cargo lib = 985 passed / 0 failed（净增 19）；verify_writing_store_601 回归 23/23；敏感词 0 命中。
3. 场景四断言实跑确认：各章有目标/重点/素材映射；material_ids/knowledge_ids 必须来自任务 selected_* 集；未确认素材拒绝进提示词（606 R1 把场景三口径延伸到大纲生成，认可）。
4. 大纲确认门禁核对：状态=大纲待确认或素材收集中才可生成；confirm 两段迁移（经 601 迁移表 is_outline_confirmed）+ outline.md 落盘 + 版本快照——「大纲未确认不得生成正文」的门禁数据基础就绪（607 生成前置校验将断言任务状态=写作中）。
5. 执行裁决复核认可：R2（confirm 两段迁移）、R3（12 类全覆盖绑定）、R4（候选留界面侧）、R5（从 outline.md 反解，不建 JSON 旁路——避免双写源）。
6. 模板机制核对：config 无文件用内嵌默认零写入（照 availability 先例）；坏配置拒绝。

**结论：** 通过。OD-WT-607（分章节写作，批次三主线）可派发。
### OD-WT-604 / OD-WT-605 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 两个并行执行代理（deepseek-v4.1-flash-eco/high）交付：604 写作需求解析（Confucius）、605 智能素材收集（Ohm）。

**核对结果：**
1. diff 允许清单核对：604 新增 engine writing_extract.py + writing_parse.rs + lib.rs 三命令 + types/api + engine 测试；605 新增 writing_collect.rs + lib.rs 两命令 + types/api。两者与 601-603 交付零交叠；runtime/ 零改动；lib.rs/api/types 并行追加无覆盖。
2. 独立复跑：RUST_TEST_THREADS=4 cargo lib = 966 passed / 0 failed（净增 27：604 项 10 + 605 项 17）；pytest = 216 passed + 30 subtests（净增 16）；tsc exit 0。敏感词 0 命中。
3. 场景三断言实跑确认：默认结果不含待审核素材；include_pending 时 pending_review=true 且 suitable_for_direct_quote=false；选择命令拒绝待审核素材入可信包。
4. 安全链路核对：writing_parse_requirement 与 writing_create_from_requirement 过 gate_user_text（604 R4 创建路径再过闸门防 AI 草稿绕过，认可）；collect 查询过滤参数不落盘不走闸门（506 R5 口径）；writing_select_materials 逐条校验已确认（605 R6 比卡面更严，认可）。
5. 执行裁决复核认可：604 R1（apply 只校验不落盘、确认后显式创建）、R2（questions 语义）；605 R1（五类标注映射）、R4（显式待审核未开开关直接报错防漏查）、R5（重点标注只留痕不加字段）。
6. 既有 OMNIDESK_PYTHON 并发竞态（602 发现）在 604/605 复现，均按 RUST_TEST_THREADS=4 复跑口径处理，登记至 615。

**结论：** 两卡均通过。OD-WT-606（大纲生成与确认）可派发。
### OD-WT-602 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 执行代理（Singer，deepseek-v4.1-flash-eco/high）交付的 document_store.rs 文稿与版本存储。

**核对结果：**
1. diff 允许清单核对：新增 document_store.rs + lib.rs 挂 mod + evidence；601/603 交付物与 work_cleanup 零修改；runtime/ 零改动。
2. 独立复跑：RUST_TEST_THREADS=4 cargo lib = 939 passed / 0 failed（净增 17）；快照-恢复-历史保留与段落 diff 测试实跑通过。敏感词 0 命中。
3. 重要发现（执行代理定位、本会话复核认可）：默认并行测试下 3 项既有引擎测试失败，根因是既有 command_tests 的 OMNIDESK_PYTHON 环境变量竞态（未加执行位的 stub），非本任务引入；单跑即过，限制 4 线程全量即过。该竞态属既有债务，登记待 615 收口统一核对（不属第四阶段引入，不阻塞）。
4. 执行裁决复核认可：manifest schema/sha256/高版本拒绝写入（防碰撞）；当前文件带 frontmatter、快照存纯正文（利于 diff 与导出）；label ASCII 安全校验；read_* 返回 Option 区分未创建。
5. 路径安全三层校验（is_safe_id/assert_under/符号链接拒绝）核对通过。

**结论：** 通过。批次二（604 ∥ 605 → 606）可派发。
### OD-WT-601 / OD-WT-603 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 两个并行执行代理（deepseek-v4.1-flash-eco/high）交付：601 writing_store.rs 写作任务实体（Maxwell）、603 writing_records.rs 引用核验记录（Boyle）。

**核对结果：**
1. diff 允许清单核对：601 新增 writing_store.rs + lib.rs 挂 mod + work_cleanup.rs 排除 work/写作/ + verify 脚本；603 新增 writing_records.rs + lib.rs 挂 mod。两者与前三阶段交付零交叠；runtime/ 零改动。
2. 独立复跑：cargo lib = 922 passed / 0 failed（基线 883 净增 39：601 项 21 + 603 项 18）；verify_writing_store_601 = 23/23（本会话实跑）。敏感词扫描 0 命中。
3. 并发纪律：两代理共享工作区，互未触碰对方文件（两份报告互证）；601 报告如实登记 603 并发改动，反之亦然。
4. 执行裁决复核认可：601 四条（时间过滤按 created_at/迁移表加退回路径/拒绝迁移留痕/删除走 .trash）；603 八条（JSONL 落 <wid>/ 目录/编号从 1 起/追加前校验换行/fact_text 可选/有来源必带 basis/按行重写处理状态/任务作用域查询/重算骨架稳定 id）。其中 601 R2 退回路径（待核验→写作中、待审核→写作中）符合核验审核返修闭环，属合理补全；603 R5 有来源必带 basis 防空依据，属合理收紧。
5. 登记待 602 消化：603 R1 记录目录约定 work/写作/<writing_id>/，602 须沿用；编号从 1 起，602 正文段落定位须对齐。

**结论：** 两卡均通过。OD-WT-602（文稿与版本存储）可派发。
### OD-KM-513 复检通过 + 第三阶段（OD-KM-500）竣工（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 收口执行代理（Kepler）交付的全量回归、verify_security_phase3.py（25 项）与十二场景对照。

**核对结果：**
1. 本会话独立复跑全量：cargo --tests = 13 目标 950 passed / 0 failed（EXIT 0，首轮无偶发）；pytest = 200 passed + 30 subtests；npm build exit 0；verify_security_phase3 = 25/25。四项全量命令 + 安全脚本全部为 ARCHITECT 实跑，非转述。
2. 安全八项口径核对：素材流新 IPC 闸门 7/7、AI 无审核写通道 3/3、知识库写路径零改动 3/3、素材篮清理排除 3/3、索引重建不损 work/ 3/3、无敏感日志、测试数据全虚构、无新增豁免——全部 PASS。
3. 十二场景对照（方案 §十）：12/12 覆盖有证据；未验证事项如实登记（真实模型素材提取质量、推荐相关性、三张 UI 卡真实窗口目视、大量素材性能、真实知识库索引重建）。
4. 遗留登记确认：verify_ui_simplify_v1.py 陈旧引用属 OD-QS-410 遗留非本阶段引入（照卡不修，已留证）；work/知识关联/ 清理排除口径核实通过（当前清理不遍历该目录，范围扩大时须补测试）。
5. 收口裁决 4 条复核认可（全量首轮 0 failed 不复跑/静态+既有测试断言的安全脚本模式/知识关联清理口径/tsc 真实性确认）。

**阶段结论：** 第三阶段（OD-KM-501～513，13 项任务）全部竣工并通过逐任务独立复检。待办：① 用户批准后提交推送（工作区含 501-513 全部交付，未提交未推送）；② 用户真实窗口目视素材篮/关联视图/项目视图三处新 UI 后追认；③ 真实模型素材提取与推荐质量待真实使用验证。
### OD-KM-509 / OD-KM-510 / OD-KM-511 / OD-KM-512 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 四路并行执行代理（deepseek-v4.1-flash-eco/high）交付：509 material_query 接口（McClintock）、510 素材篮页（Carson）、511 关联视图（Avicenna）、512 项目视图（Lorentz）。

**核对结果：**
1. diff 允许清单核对：509 新增 material_query.rs + lib.rs 一命令 + types/api；510 改 WritingStudio.tsx（页签组）+ 新增 WritingMaterialBasket.tsx + verify 脚本；511 新增 KnowledgeLinkPanel.tsx + 改 WorkRecordLibrary/TaskManager/KnowledgeVaultPage 追加区块；512 改 ProjectBoard.tsx 追加概览区块 + verify 脚本。四者文件零交叠；runtime/ 零改动；501-508 交付物结构零修改。
2. 独立复跑：cargo lib = 883 passed / 0 failed（净增 7：509 七项含场景十二 payload 测试）；tsc exit 0；build exit 0；verify 510 = 23/23、511 = 25/25、512 = 26/26（全部本会话实跑）。
3. UI 参考核对：510（Iris AI/Questia/Apple News）、512（Flow/Inex One）、511（Evernote/Additor/Wonder）均有检索留痕与 ≤2 句借鉴要点，符合 BUILDER 模板条款。
4. 执行裁决复核认可：509 五条（独立薄模块/时间口径沿用/默认已确认/关键词只匹配标题正文/容错不失败）；510 五条（页签挂稿件卡内/SourcePeekSheet 只读预览/客户端筛选/收藏口径/合法流转按钮）；511 六条（详情面板落 WorkRecordLibrary——WorkRecordPage 只是壳，处置正确/反向区块定位信息/已关联原文前 60 行/候选合并/回执刷新/无跨页跳转）；512 四条（保留最小素材列表不改 510 文件/计数与行列表口径分离/不新增创建入口/失败显示「—」不回落 0）。
5. 遗留登记至 513：a) 510 发现 verify_ui_simplify_v1.py 仍引用 410 已删除的 WeeklyQuadrants.tsx（既有陈旧脚本，属既有债务，513 核对不属本阶段引入）；b) 素材编辑界面（material_edit）未接 UI，属任务卡未要求，后续按需补；c) 三张 UI 卡均未做真实窗口目视（统一列入 513 未验证事项）。

**结论：** 四卡均通过。最后一张任务卡 OD-KM-513（全量回归+安全核对+收口）可派发。
### OD-KM-506 / OD-KM-507 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 两个并行执行代理（deepseek-v4.1-flash-eco/high）交付：506 素材 IPC 六命令（Bohr）、507 知识关联 IPC 四命令（Hooke）。

**核对结果：**
1. diff 允许清单核对：506 改 lib.rs（六命令+注册+analyze 注入素材清单）/task_extract.rs（build_prompt_with_materials 追加式）/task_extract.py（materials 渲染）/types.ts/api.ts；507 新增 knowledge_link_store.rs + lib.rs 四命令 + types/api。501-505 交付物结构零修改；runtime/ 与 knowledge_vault 写路径零改动。
2. 独立复跑：cargo lib = 876 passed / 0 failed（净增 20：506 七项 IPC 测试 + 507 十三项）；tsc exit 0；npm build exit 0；pytest 200 passed + 30 subtests（506 补 3 项注入用例）。全部为本会话实跑。
3. 安全核对：AI 无写通道双向核实——506 material_extract_from_record 只返回分析准备（模型调用仍在界面侧、落暂存候选走既有 apply），507 suggest 只产候选；审核状态仅 material_review 用户触发；知识库只读。红线闸门覆盖六命令落盘文本（查询过滤参数不落盘不走闸门，裁决合理）。
4. 并发纪律：lib.rs 双代理并行追加无冲突（506 报告与 507 报告互证）；507 观察到的 506 中间态测试失败已随最终复跑消解。
5. 执行裁决复核认可：506 R1（user 来源 quote 必填不放宽，界面传正文作引用）、R2（extract 返回 AnalyzePrompt 不代调模型）、R4（中英审核动作）、R5（过滤参数不走闸门）；507 R1（候选独立 .candidates/）、R2（frontmatter 定向写回保字节）、R3（移除留痕 JSONL）、R4（半完成回执）。
6. 登记遗留：507 移除留痕 JSONL 无轮转（低风险，个人规模）；关联写入不推进 updated_at（界面按更新时间排序看不到此变更，511 界面需注意）；work_cleanup 未对 work/知识关联/ 加显式排除测试（513 收口补验）。

**结论：** 两卡均通过。批次四（509 ∥ 510 ∥ 511 ∥ 512）可派发。
### OD-KM-504 / OD-KM-505 / OD-KM-508 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 三路并行执行代理（deepseek-v4.1-flash-eco/high）交付：504 AI 素材提取搭车（Herschel）、505 知识检索器（Hilbert）、508 项目 IPC（Darwin）。

**核对结果：**
1. diff 允许清单核对：504 改 engine/omnidesk_engine/task_extract.py + task_extract.rs + engine 侧测试；505 新增 knowledge_link.rs + lib.rs 挂 mod；508 改 lib.rs（四命令+注册）/task_store.rs（TaskDraft/TaskPatch 追加 project_id）/api.ts/types.ts。三者零越界；501/502/503 交付物未被修改；runtime/ 零改动。
2. 独立复跑：cargo lib = 856 passed / 0 failed（净增 35：504 16 + 505 14 + 508 5）；pytest = 197 passed + 30 subtests（基线 187 净增 10）；tsc exit 0。全部为本会话实跑。
3. 安全核对：敏感词扫描 0 命中；task_extract.py 静态核对仍只读 stdin 只写 stdout；零新增模型调用（搭既有分析调用，宿主校验权威）；素材候选落 素材篮/.candidates/（该目录已整体被清理排除）；AI 无审核状态写通道（504 暂存不改 review_status）。
4. 执行裁决复核认可：504 四条（候选暂存位置/指纹复用/两级复制检查/宿主算位置）；505 五条（LinkSubject 包装/实体关联确定性定义/关键词切分/相对路径 id+上限 8/读取失败退回）；508 四条（单项目 overview/TaskDraft 扩字段/记录经任务链派生项目/broken 上报）。
5. 登记待 506/507 消化事项：504 R2——worklog_record_analyze 在 lib.rs 属 504 允许清单外，提示词未注入既有素材清单，match_material_id 由宿主校验兜底；506 卡应把该注入补上（属 506 允许范围）。

**结论：** 三卡均通过。批次三（506 ∥ 507）可派发。
### OD-KM-502 / OD-KM-503 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 两个并行执行代理（deepseek-v4.1-flash-eco/high）交付：502 project_store.rs 项目实体（Socrates）、503 关联字段扩展（Beauvoir）。

**核对结果：**
1. diff 允许清单核对：502 仅新增 project_store.rs + lib.rs 挂 mod + evidence；503 仅改 task_store.rs / worklog_records.rs（全为新增行）。两者与 501 交付零交叠；project_items.rs / runtime / knowledge_vault 零改动。
2. 独立复跑：cargo test --offline --lib = 821 passed / 0 failed（基线 792 → 净增 29：502 项 14 + 503 项 15）。字节不变测试（legacy_task/legacy_record）实跑通过。
3. 并发纪律核对：两代理在共享工作区并行，互未触碰对方文件；502 报告如实登记了 503 的并发改动；503 报告如实登记了 502 的 project_store.rs。engine_ipc 偶发 PermissionDenied 属 macOS 已知噪音口径（单测复跑即过）。
4. 执行裁决复核：502 R2（补 created_at/updated_at 与长度上限）认可——照既有实体先例；R3（ProjectRefItem 解耦）认可——纯函数骨架不绑具体结构。503 R1（字节不变走 render 入口）认可——更新函数推进 updated_at 是既有语义；R2（知识编号取相对路径形式）认可——与 501 related_knowledge_ids 口径一致，507 若收紧需同步；R4（空值省略不写 null）认可——保字节不变的必要实现。
5. 敏感词扫描 0 命中。

**结论：** 两卡均通过。503 R2 登记一条口径：知识编号当前为相对路径形状校验，507 确认流与 505 检索器沿用该口径；若改统一 ID 规范须同步三处。
### OD-KM-501 复检通过（2026-11-09，ARCHITECT 独立复核）

**审查对象：** 执行代理（deepseek-v4.1-flash-eco/high，agent Meitner）交付的 material_store.rs 素材实体。

**核对结果：**
1. diff 允许清单核对：新增 material_store.rs / verify_material_store_501.py / evidence、修改 lib.rs（仅挂 mod 一行）与 work_cleanup.rs（素材篮排除），无范围外改动；runtime/ 零改动；状态文件未动。
2. 独立复跑：cargo test --offline --lib = 792 passed / 0 failed（基线 770 净增 22）；verify_material_store_501.py = 19/19（本会话独立复跑，非转述）。
3. 安全核对：新增文件敏感词扫描 0 命中；素材篮清理排除有实测断言（素材篮文件不得被清理移动 + 字节不变断言）；review_status 不经创建/编辑通道写入（AI 无审核状态写通道）有测试。
4. 执行裁决 4 条复核认可：source_quote 全来源必填（指纹依赖）；confirmation_status 两值字符串；创建一律待审核；删除走 work/.trash/ 可恢复先例。

**结论：** 通过。执行代理裁决 R1（user 来源 quote 必填）如与 506 交互冲突，在 506 卡内显式放宽并补指纹规则，属计划内调整。
**结论：通过。** 本审查基于实际源码、git diff、证据文件与本会话独立复跑；runtime/ 零改动。

**独立复跑（本会话实跑，2026-11-09）：** cargo test --offline --lib → 770 passed / 0 failed；cargo test --offline --tests → EXIT=0、13 个测试目标全部 0 failed；pytest tests -q → 187 passed + 30 subtests；npx tsc --noEmit → exit 0；npm --prefix desktop run build → exit 0。专项 verify（全部本会话复跑）：verify_priority_rules_401 11/11、verify_task_quadrant_ui_402 11/11、verify_schedule_ui_403 11/11、verify_security_phase2 **14/14**、verify_ui_neumorphism_202 **36/36**（412 时的 19/19 → 后续界面扩展 36/36，全绿）。

**安全核对逐项（对照 409/412 登记项）：**

- 五个优先级 IPC：importance/quadrant/due 三个自由文本通道全部过 gate_user_text（verify 逐条断言在案）；estimated/dependency 为纯类型入参无可绕过文本通道，dependency 仍做存在性+无环+不自依赖校验。
- **409 登记项闭环：** schedule_generate/adjust 的理由落盘通道在本阶段口径下判定为可接受（work/ 内部文件、不进模型上下文、扫描不覆盖临时层），未改为强制闸门；verify_security_phase2 以「排期写入只出现在 schedule_generate/adjust 两条用户触发路径」+「AI 无排期写通道」两条断言覆盖了风险面。该口径已与本条记录一致，后续如排期理由需要进知识库或模型上下文，须先补闸门。
- AI 提取链路不引用排期模块；启动流程不自动生成排期；排期失败不改任务文件；新模块无调试日志；测试只落临时目录；无新增豁免。
- 敏感词扫描（新 diff 全量）：零命中；测试数据全虚构。

**证据落盘：** tests/verification/desktop/evidence/od-qs-413/（README 实跑矩阵 + security_phase2.json 14 项断言）；12 份 verify 全绿（含第一阶段 301–308 份按 410 替换同步更新的 verify_week_from_records_306 12/12）。

**未验证事项（如实登记，不阻塞竣工）：** ① 真实模型 importance 建议质量（本阶段零真实调用）；② 真实窗口四象限/今日/本周页与新拟物主题观感（执行会话在 STATUS.md 记录过 OD-UI-202 顶栏/首页目视；OD-QS 三个新页签的目视待用户）；③ 数百条任务下排期生成与渲染性能；④ 真实排期冲突处理体验。

**OD-UI-202：** 新拟物第一阶段（顶栏/首页/Dock/底材）已实施且静态 36/36 全绿；用户已追认并追加中文界面字体修订（统一黑体，宋体退出界面）。执行会话已做真实窗口目视并记录 STATUS.md。

**工作区清理建议（提交前）：** .codex-attachments/（52K，一张设计参考图）未被 .gitignore 覆盖且与仓库无关，建议加入 .gitignore 或删除，不随提交入库。

**竣工声明：** 第二阶段任务四象限 + 智能排期（OD-QS-401～413）13 项任务全部实施并通过本会话逐任务独立复检；规划文档 docs/plans/OD-QS-400/ 五份与 decisions D-069～D-075 与实际实现一致（D-069/D-070/D-072/D-075 已随复检补记实施证据，D-071/D-073/D-074 待下次 decisions 维护时同步为已实施——属记录维护，不影响验收）。提交与推送待用户批准执行。

## OD-QS-412 审查（ARCHITECT，2026-11-08）——通过（附 OD-UI-202 状态更新与 STATUS 纪律记录）

**结论：通过。** 基于实际源码、git diff 与本会话独立复跑核对；runtime/ 零改动，敏感词零命中。

**独立复跑（本会话实跑）：** cargo test --offline --lib → 770 passed / 0 failed（纯前端任务，Rust 无净增）；cargo test --offline --tests → 13 个测试目标全部 0 failed（EXIT=0）；verify_task_quadrant_ui_402 11/11、verify_schedule_ui_403 11/11、verify_ui_neumorphism_202 19/19；npx tsc --noEmit → exit 0；npm --prefix desktop run build → exit 0。

**逐项核对（对照任务卡四个展示区 + 调整入口）：**

- **优先级与依据区：** 重要/紧急/象限徽标 + importance_source 展示 + 「用户指定」标注（override 时）+ 重要依据/紧急依据双文本（取自宿主派生 selectedView，缺省「待评估」不造数）。
- **截止与预计耗时区：** 日期+hard/target 下拉+理由必填（按钮 disabled 直到有理由）走 taskSetDue；预计分钟数（1–1440，留空清除）走 taskSetEstimated。
- **依赖与父子区：** depends_on 列表展示（逗号分隔编辑走 taskSetDependency）、父任务显示（parent_id → 标题解析）；存在性/环校验由宿主负责，前端只透传。
- **排期历史与调整记录区：** 读取 scheduleList 过滤当前任务，逐条显示起止/状态/派生状态（effective_status ≠ status 时标注「派生：已失效」）/锁定/时长/adjustment_reason 与 adjustments 追加记录——「历史排期不可删、派生失效可见」在详情层闭合。
- 五个调整函数全部走 405 IPC（diff 中 taskSet* 七处引用），无前端直写文件；保存后 reloadTick 刷新。

**OD-UI-202 状态更新：** STATUS.md 新增「OD-UI-202 新拟物第一阶段实施」条目（执行会话所写，含真实窗口目视记录：顶栏/凹陷搜索与 Profile/凸起按钮/Dock 当前项/输入区压入/焦点环，品牌靛强调色）。用户此前在界面评审中认可了新拟物方向与品牌靛选择；**本会话据此把 OD-UI-202 第一阶段记为「已实施，待用户最终追认」→ 状态以 PLANS.md 更新为准**。真实窗口目视由执行会话完成并记录在 STATUS.md（证据等级：真实窗口目视，本会话未重跑窗口，静态检查已扩展复跑）。**后续（2026-11-09）：用户已追认「品牌靛 + 顶栏/首页」，随后否决宋体界面并改为统一黑体；最终状态以文末「OD-UI-202 第一阶段审查结论」为准（36/36）。**

**纪律记录（不影响 412 验收）：** 执行会话本轮写入 STATUS.md（OD-UI-202 条目）。AGENTS.md 规定 STATUS.md 由执行者填写——**STATUS.md 是执行记录，执行会话写自己的条目是合规的**；违规线只有「写 REVIEW.md / PLANS.md」（302 曾违反）。核对 STATUS.md 新增内容与 git diff 事实一致，无失实描述。

**后续指示：** **最后一项：OD-QS-413（全量回归+安全核对+阶段收口）可派发**。413 清单：全量命令；verify_security_phase2.py（五个新 IPC 闸门、AI 无排期写通道、排期失败不影响任务状态、无敏感日志、全虚构数据）；**补查 409 登记项**（schedule_adjust/cancel 理由闸门口径显式复核）；未验证事项登记（真实模型 importance 质量、真实窗口四象限/计划页、大量任务性能）；OD-UI-202 追认状态一并落 PLANS。

## OD-QS-410 与 OD-QS-411 审查（ARCHITECT，2026-11-08）——双通过（附 OD-UI-202 界面层同批实施说明）

**结论：410 通过；411 通过。** 基于实际源码、git diff 与本会话独立复跑核对；runtime/ 零改动，敏感词零命中（TaskManager 与两份 verify 均无真实业务词）。

**独立复跑（本会话实跑）：** cargo test --offline --lib → 770 passed / 0 failed（410/411 为纯前端+一条只读 IPC，Rust 无净增；后端 task_quadrant_views 只读派生）；cargo test --offline --tests → 13 个测试目标全部 0 failed（EXIT=0）；verify_task_quadrant_ui_402.py → 11/11；verify_schedule_ui_403.py → 11/11；npx tsc --noEmit → exit 0；npm --prefix desktop run build → exit 0。

### OD-QS-410（任务四象限页 + 旧占位替换）

**逐项核对：**

- **旧占位替换完成（用户裁决②）：** weeklyQuadrants.ts（111 行）与 WeeklyQuadrants.tsx 已删除；verify 断言两文件不存在 + 全 src 无引用残留（11/11 我复跑通过）；ScenePage 四象限区改挂 TaskQuadrantBoard；REPORT_DESTINATION 与新增 countReportEntries 移入 weekTimeline.ts，WeeklyTimeline 改从那里引用——常量迁移路径与任务卡一致。
- TaskQuadrantBoard：Q1–Q4+待评估五列文字标题（不只靠颜色，verify 断言在案）；每任务行显示 重要/紧急（文字徽标）+ 截止 + 状态片 + 已确认/已建议 + override「用户指定」标注 + 双依据摘要 + 不参与待办（inactive 半透明）；调整面板走 405 IPC（taskSetImportance/taskSetQuadrant，含理由），保存后刷新视图。
- 数据全部来自宿主派生（taskQuadrantViews IPC + 显式本地偏移），前端无第二套归类逻辑——象限真源唯一。

### OD-QS-411（今日/本周计划页）

**逐项核对：**

- SchedulePlanner（today/week 双模式）：今日=可用时段+剩余容量（按 availability 实算）+已确认/已建议分组+建议可就地确认+冲突显式列表（data-schedule-conflicts）；本周=七日分布+本周截止+未排期重要任务（生成后带原因，未生成时按高重要未排期兜底列表）+逾期风险（排除已完成/已取消）；stale 提示（「任务已变化，建议重新生成」）+ 重新生成入口 + 空态真实（「还没有生成过排期建议」不造数）。
- scheduleGenerate 显式传本地时区偏移（LOCAL_OFFSET_MINUTES），与 402/403 的不静默用服务器时区口径一致。

### OD-UI-202 界面层同批实施（超出 410/411 卡范围，后经用户追认；最终结论见文末）

**事实（git diff 核实）：** TopNav/Dock/HomeChat/App/CategoryPicker/ModelPicker/WindowChrome/theme.css 出现成套 neu-* 类（neu-app/neu-glow/neu-dock-btn/neu-popover/neu-inset-pill/neu-window-btn 等），theme.css +268 行定义令牌与阴影；另有 tests/verification/desktop/verify_ui_neumorphism_202.py + json（19/19 本会话复跑通过）、docs/brand-and-ui/ 两份设计稿（未跟踪）。这正是 PLANS.md「OD-UI-202：新拟物设计语言落地（设计稿已完成，待批准实施）」登记的第一阶段范围（顶栏+首页+Dock+底材），此前只有设计稿待批。**本会话未批准过其实施**；它落在 410/411 同一工作区属执行会话超范围实施。因：①改动与 OD-QS 功能无交叠（纯样式类名与令牌层，无业务逻辑/IPC/数据模型变化，tsc/build 全绿）；②PLANS 已有该计划全文且范围一致；③回滚会牵连 410/411 同区改动——**不要求回滚，但该部分验收状态记「待用户追认」**：请用户对 OD-UI-202 第一阶段（顶栏+首页+Dock）做真实窗口目视验收，追认后我补记 REVIEW 并在 PLANS 把 OD-UI-202 状态改为已实施；不追认则另立回滚任务（样式层可独立回退）。

**偏差与说明（不阻塞验收）：**

1. verify_task_quadrant_ui_402 / verify_schedule_ui_403 均为静态源码检查（证据等级已标明）；真实窗口目视验收待用户（410 四象限列、411 今日/本周页签、OD-UI-202 新拟物观感一并目视）。
2. 本周未排期兜底列表（未生成报告时按「高重要+无活跃排期」过滤）是前端展示口径，不落盘；生成后以报告 unscheduled 为准。合理，登记备查。

**后续指示：** **最后一项：OD-QS-412（任务详情扩展）可派发**；412 复检后即 413（全量回归+安全核对+收口，含 409 登记的 schedule_adjust/cancel 闸门口径复核）。

## OD-QS-409 审查（ARCHITECT，2026-11-08）——通过

**结论：通过。** 基于实际源码、git diff 与本会话独立复跑核对；runtime/ 零改动，敏感词零命中。

**独立复跑（本会话实跑）：** cargo test --offline --lib → 770 passed / 0 failed（766 → 770，净增 4 项：生成幂等/stale/过期/锁定豁免）；cargo test --offline --tests → 13 个测试目标全部 0 failed（EXIT=0）；npx tsc --noEmit → exit 0。

**逐项核对：**

- 六条命令全部注册：schedule_generate（生成建议条目落盘）、schedule_confirm、schedule_adjust、schedule_cancel、schedule_lock、schedule_list（返回含派生已失效的 ScheduleListing）；前端 api.ts/types.ts 成套（ScheduleStatus/ScheduleEntryView/ScheduleGenerateReport 等类型 + 六条调用封装）。
- **已确认/锁定保护落实：** generate 前把锁定+已确认+既有建议全部作为占位传给 408 引擎（建议视作已占位防重叠，代码注释写明理由）；新生成只对没有活跃条目的任务补建议（existing_tasks 去重，测试断言第二次生成 created 为空）；已确认/锁定条目原样保留。
- **stale 机制落实：** generation.v1.json（schema omnidesk.schedule-generation.v1）记录最近生成时间；is_stale=任务最大 updated_at 晚于生成时间（无记录视为过期，保守合理）；generate 回执带 stale 标志与 generated_at，测试覆盖「生成后改任务 → stale=true」。
- **过期处理落实且保护正确：** expire_stale_suggestions 只把「任务已完成/取消后的已建议」置为已取消并追加「过期」调整记录；**锁定条目豁免**（expire_skips_active_and_locked_suggestions 有测试，锁定优先于过期）；已确认条目不在处理范围（保留给用户处置）。
- 调整/取消/确认/锁定全部复用 407 既有校验（理由必填、锁定不可调整/取消、状态机白名单），IPC 层无重复实现。

**偏差与说明（不阻塞验收）：**

1. schedule_adjust / schedule_cancel 的理由入参未过 gate_user_text（直接透传给存储层落盘）。核对既有口径后判定为**与 302 闸门边界一致**：gate 在 IPC 层只覆盖自由文本展示性入参的风险场景，调整理由落盘在 work/ 内部文件、不进模型上下文、不经知识库扫描（扫描不覆盖临时层）；风险与状态备注（transition_task note）同档。**413 安全核对时把「排期理由是否需要闸门」列为显式检查项**，如判定需要再补（改动一行级）。
2. 409 复用 407 的 schedule_store.rs 增量（generation 文件 + expire/stale 函数），未新增文件；任务卡允许文件为 lib.rs + schedule_store.rs + api/types，实际改动一致。

**工作区附注（非本任务改动）：** 工作区新增 docs/brand-and-ui/neumorphism-guideline.md + preview.html（未跟踪），对应 PLANS.md 已有 OD-UI-202 设计稿小节（设计交付，未改真实界面，待用户批准）。来源为另一会话的规划交付，与本任务卡无关，保留不动；提交时须用户裁决是否与 OD-QS 改动同批入库。

**后续指示：** **下一批：OD-QS-410（任务四象限页 + 旧占位替换）∥ OD-QS-411（今日/本周计划页）可并行派发**；随后 412（详情扩展）→ 413（收口，含上述第 1 条安全复核项）。

## OD-QS-405 与 OD-QS-408 审查（ARCHITECT，2026-11-08）——双通过

**结论：405 通过；408 通过。** 均基于实际源码、git diff 与本会话独立复跑核对；runtime/ 零改动。

**独立复跑（本会话实跑）：** cargo test --offline --lib → 766 passed / 0 failed（746 → 766，净增 20 项：405 调整记录与 IPC 测试 + 408 排期引擎 17 项）；cargo test --offline --tests → 13 个测试目标全部 0 failed（EXIT=0；首轮曾见 1 failed，连续两轮复跑全绿——失败项复跑即过、无 panic 痕迹，按既有口径判定为 macOS 偶发噪音，非本轮引入）；verify_priority_rules_401.py → **11/11**（405 已随卡修正 pure_function_no_io 断言：std::path 从禁止列表移除并注明「路径参数不算 IO」，403 遗留的误报消除）；npx tsc --noEmit → exit 0；npm --prefix desktop run build → exit 0。

### OD-QS-405（优先级 IPC 与用户调整）

**逐项核对：**

- 五条命令全部注册：task_set_importance（source 固定 user，理由过 gate_user_text 后落盘）、task_set_quadrant（设置/清除+理由过闸门）、task_set_due（date+kind+理由必填过闸门）、task_set_estimated（无自由文本）、task_set_dependency（存在性+无环由存储层校验）。拒绝理由沿 TaskSaved/Err 通道对界面可见。
- **调整记录落实：** Task 新增 adjustments（## 调整记录 追加段，at/field/action/reason，只追加不改写），set_importance/set_quadrant/set_due/set_estimated/set_depends_on 全部留痕；parse_sections 三分区（进展/状态历史/调整记录）兼容解析。
- 前端成套：types.ts 补 WorkTask 新字段映射（importance/importance_source/importance_reason/due_kind/depends_on/estimated_minutes/parent_id/quadrant_override/adjustments——401 复检时登记的前端映射欠账本任务补齐）+ TaskAdjustment/TaskImportance/TaskDueKind/TaskQuadrant 类型；api.ts 五条调用封装；403 复检时登记的 verify 断言修正完成。

### OD-QS-408（排期引擎）

**逐项核对：**

- scheduling.rs（1038 行，17 项测试）纯函数不落盘；内部 UTC 秒运算、窗口按显式本地偏移换算，复用 worklog.rs civil_from_days，无新依赖。
- **十一优先序逐条落实：** done/cancelled/paused/待确认不入未来排期（scheduled_active 过滤+待确认排除）；锁定与已确认生成 fixed 时段永不被建议覆盖（suggestion_never_overlaps_confirmed_plan 有测试）；固定排期互相重叠→冲突（锁定冲突单列 kind）；硬截止优先（排序键 due 升序第一位 + 截止不可满足/截止前容量不足冲突）；依赖先行（Kahn 拓扑+dep_ends 最早起始，环内整体进冲突与 unscheduled）；高重要排序居前（同截止高重要先排有测试）；每日容量 80% 封顶+超容量溢出次日（cap_per_day=BUFFER_RATIO×容量，测试断言跨日）；Q4 每日 ≤25% 自动容量且不抢占 Q2（q4_does_not_preempt_q2 有测试）；周末跳过、休息与不可用时段扣除（free_windows 分裂窗口）；缺耗时→unscheduled「缺少预计耗时」；超最大时段→「建议拆分」（D-074 落实）；待评估象限→unscheduled 不强行安排。
- 方案 §十二.3 十六场景全部有测试对应（单/多任务、同截止、依赖、耗时过长、容量不足、锁定、固定冲突、容量缓冲、提前完成/延期由 fixed 排除与 stale 机制承载——stale 标志在 409 接线，此处引擎无状态合理）。

**偏差与说明（不阻塞验收）：**

1. 排序键为 (due 升序, -importance, 象限序, id)：方案要求「不得机械按象限排序」，此键以截止优先、象限仅作同截止日的次序参照，符合 §八.3 第 4 条优先于象限序的意图。
2. 待确认任务被排除出排期候选（STATE_PENDING_CONFIRM 在 scheduled_active 之外）——比任务卡「已完成/已取消不进未来排期」更严，符合「AI 只建议、用户确认后才进正式清单」的既有裁决，登记为子裁决。
3. --tests 首轮 1 failed 复跑即过：无固定失败复现路径，两轮全绿 + EXIT=0，按 process_owner 先例记 macOS 偶发噪音；413 收口时再跑全量复核。

**后续指示：** 下一任务：**OD-QS-409（排期 IPC 与确认流）**——408 已就绪，409 完成后 410/411 可并行，412 随后，413 收口。

## OD-QS-403 审查（ARCHITECT，2026-11-08）——通过（附一项 verify 断言修正说明）

**结论：通过。** 基于实际源码、git diff 与本会话独立复跑核对；runtime/ 零改动，敏感词零命中。

**独立复跑（本会话实跑）：** cargo test --offline --lib → 746 passed / 0 failed（733 → 746，净增 13 项：象限 13 场景 + 排序/偏移）；cargo test --offline --tests → 13 个测试目标全部 0 failed；cargo test --offline --lib priority_rules → 28 passed（402+403 合计模块测试）；verify_priority_rules_401.py → 复跑 10/11（见下）。

**逐项核对：**

- QuadrantView{task_id/title/quadrant/importance/urgency/importance_reason/urgency_reason/evaluated_at/is_override/active} 与任务卡一致；象限不落盘，每次读取由 classify_quadrant 派生，仅 override 来自 Task frontmatter——D-069 落实。
- 分类规则符合任务卡：任一维待评估→象限「待评估」（不强行归类）；override 非空→返回 override 并标 is_override（测试断言清除后回到派生值）；已完成/已取消/暂停→active=false，暂停仍保留重要性（有专门测试）；逾期但低重要性→urgency=高但象限 Q3 不强制 Q1（测试名 overdue_but_low_importance_is_q3_not_q1 直书该规则）；目标时间同日期降档改变象限（Q1→Q2/Q3 有测试）。
- 方案 §十二.1 十三类场景全部有测试（四组合、双维未知、未识别 hint、target 变象限、逾期低重要、override 设置/清除、done/cancelled inactive、paused 保留重要性、列表排序）。
- list_quadrant_views_at 支持 utc_offset_minutes 显式偏移；批量排序 active 优先→象限序→标题→id，稳定可测。

**偏差与说明（不阻塞验收，需知悉）：**

1. **verify_priority_rules_401.py 有一项断言过时：** pure_function_no_io 检查禁止生产代码出现 std::path，而 403 的 list_quadrant_views 合法地用 &std::path::Path 接收参数（路径参数不等于文件 IO；模块内仍无 fs/Command 引用）。402 复检时该检查 11/11 通过（当时无批量函数）；403 扩展后该断言口径过紧。这是**验证脚本的误报，不是源码缺陷**。处理方式二选一：405 执行会话随卡修正该断言（把 std::path 从禁止列表移除，保留 use std::fs 与 Command::new 两项真正的 IO 判据）——推荐此项，改动一行且有明确理由；或本会话代改。未修正前，该脚本的 pure_function_no_io 项按本条说明人工判定为通过（依据：模块头注释 + 生产代码 grep 无 fs/Command 实测）。
2. 「只有高才算重要/紧急，中低归另一侧」是执行裁决（任务卡未写明中档归类）。该选择可解释、可测试且与象限拖延梳理用途一致（中档不制造 Q1 假紧急）；如后续实测中档任务被过度集中 Q2/Q4，属复审触发范围（D-069）。

**后续指示：** 408 的前置（403/406/407）已全部就绪——**下一批：OD-QS-405（优先级 IPC，含上述 verify 断言修正）∥ OD-QS-408（排期引擎）可并行派发**；随后 409 → 410/411 → 412 → 413。

## OD-QS-402 / OD-QS-404 / OD-QS-407 审查（ARCHITECT，2026-11-08）——三通过

**结论：402 通过；404 通过；407 通过。** 均基于实际源码、git diff 与本会话独立复跑核对；runtime/ 零改动。

**独立复跑（本会话实跑）：** cargo test --offline --lib → 733 passed / 0 failed（上轮 698 → 733，净增 35 项：402 十五项 + 404 五项 + 407 十五项）；cargo test --offline --tests → 13 个测试目标全部 0 failed；pytest tests -q → 187 passed + 30 subtests；verify_priority_rules_401.py → 11/11；npx tsc --noEmit → exit 0。

### OD-QS-402（紧急性规则引擎）

**逐项核对：**

- priority_rules.rs（531 行）纯函数无 IO：生产代码不含 fs/Command/path 引用（verify 断言在案）。compute_urgency(task, now) 签名符合任务卡；另提供 compute_urgency_at(task, now, utc_offset_minutes) 供显式传入本地偏移——「不静默使用模型服务器时区」由此落实。
- 分档规则符合 D-075：逾期→高并附逾期天数；hard due 按工作日计数分档（2 个工作日内=高、5 个工作日内=中，阈值常量 HIGH/MEDIUM_BUSINESS_DAYS 注明依据，未用固定 24/48h）；due_kind=target 同日期降一档；今天截止=高。
- 封闭模式集：明天/后天/下周X/本月底/下月底/X月X日前，推定结果带「推定」标注并降一档（按目标时间处理）；近期/尽快/无时间等模式外一律待评估。跨月/跨年（12 月下月底→次年 1 月 31 日、1 月 5 日→次年）/闰年（2024-02-29）有测试；日期换算复用 worklog.rs 的 civil_from_days/days_from_civil（含 2 月 30 日这类非法日回退校验）。
- 方案 §十二.2 十二类时间场景全部有测试（verify 脚本逐项断言测试名存在）；敏感词扫描零命中。

### OD-QS-404（AI 重要性建议，零新增模型调用）

**逐项核对：**

- task_extract.py：JSON_OUTPUT_SPEC 增 importance（高/中/低/null）+ importance_reason（≤100 字），两条约束原文符合任务卡（「不要仅因为领导提出就判高」「不要仅因为没有截止日期就判低」）；无任何新端点或新调用。
- 宿主 task_extract.rs：RawCandidate/CandidateInput 增字段；校验成对——importance 非 null 必须有 reason（≤100 字无换行），null 时 reason 必须为 null，未知等级拒绝。
- task_store.rs：stage_candidates 二次校验同口径（双保险）；confirm_candidate 在 create_task 后对有 importance 的候选调用 set_importance(source=ai, reason)，测试断言落盘 importance_source=ai 与依据——D-070「确认落 Task 时写 ai 来源」落实。
- adapter 测试补 spec 断言；pytest 187 passed 独立复跑通过。

### OD-QS-407（排期实体）

**逐项核对：**

- schedule_store.rs（972 行，15 项测试）：work/排期/<entry_id>.md（schema omnidesk.schedule.v1），字段与任务卡一致；时间统一 ISO 8601 UTC、duration 与起止时间一致性校验、单条 ≤1440 分钟、非法时间戳拒绝。
- 状态机与 D-071 一致：已建议→已确认（仅此迁移）；锁定仅限活跃条目且不可调整/取消；调整=旧条目置已调整+调整记录追加+新条目（新条目创建失败时旧条目回滚重写，有补偿）；取消带原因且记入追加段；已完成标记仅限活跃条目。
- 同任务同时段唯一活跃条目（重叠拒绝、相邻与跨任务允许，有测试）；已完成/已取消任务不能新建排期；**任务完成后未来条目读取时派生「已失效」且文件字节不变**（专门测试断言 before==after）——「历史排期不可删、文件不改写」落实。
- 列表按开始时间排序、损坏文件计数不中断（broken 字段）。

**偏差与说明（不阻塞验收）：**

1. 402 的 hint 推定以任务 created_at 日期为基准（任务卡未指定基准日期，属合理选择——记录日期不进 Task；408 使用时如需改基准另议）。
2. compute_urgency 两参签名按 UTC 日历日；界面接线（405/410）时用 compute_urgency_at 传本地偏移，模块头注释已写明。
3. schedule_store 依赖 worklog.rs 的时间函数（pub 可见），无重复实现；未新增依赖。

**后续指示：** 下一批：**OD-QS-403（四象限分类引擎，只阻塞于 402，现在可派）**；408 待 403/406/407 齐备（406/407 已过，403 复检通过即可派 408）；405 可与 408 并行。

## OD-QS-401 与 OD-QS-406 审查（ARCHITECT，2026-11-08）——双通过

**结论：OD-QS-401 通过；OD-QS-406 通过。** 均基于实际源码、git diff 与本会话独立复跑核对；runtime/ 零改动。

**独立复跑（本会话实跑）：** cargo test --offline --lib → 698 passed / 0 failed（第一阶段收口 672 → 698，净增 26 项：401 十五项 + 406 十一项）；cargo test --offline --tests → 13 个测试目标全部 0 failed；npx tsc --noEmit → exit 0；npm --prefix desktop run build → exit 0（仅既有 chunk 体积提示）。

### OD-QS-401（Task 字段扩展）

**逐项核对：**

- 八个新字段全部落地且语义与 D-069 一致：importance（高/中/低/待评估，默认待评估）、importance_source（ai/user/rule）、importance_reason、due_kind（hard/target，默认 hard）、depends_on、estimated_minutes（1–1440 越界拒绝）、parent_id、quadrant_override（Q1–Q4）。hint 注释已改为「模糊时间解析输入（402 启用）」。
- 旧文件兼容：seed_legacy_task 测试用无新字段的旧 frontmatter 直读，断言读回=默认值且**字节不变**（读取不触发迁移写盘），符合任务卡「serde default 零迁移」。
- 用户重要性必填理由（source=user 且无 reason → 拒绝且不落盘）；未知等级/来源/象限/截止性质均在 parse 与写入两侧拒绝；依赖与父任务的存在性、自引用、环（依赖链 + 父链两个方向）均有测试；重复依赖去重；写入走既有 write_replace_atomically 原子替换，updated_at 正确刷新。
- 允许清单核对：仅 task_store.rs（含模块测试），范围内；lib.rs 改动属 406（见下）。

### OD-QS-406（工作时间配置）

**逐项核对：**

- availability.rs（499 行，11 项测试）：schema omnidesk.availability.v1；workdays 1–7 去重、daily_windows 至少一段且不重叠、breaks 不重叠、unavailable 日期形状+原因必填、daily_capacity_hours 0.5–12（NaN 拒绝）、timezone 非空且无空白/控制字符。
- **无文件=默认值且零写入**有专门测试（defaults_are_complete_and_do_not_write 断言读取后文件不存在），符合 D-072 干净底座纪律。
- 保存走「校验 → 临时文件 + sync_all + rename」原子替换，失败时旧文件逐字节不变且无 .tmp 残留（有测试）；损坏 JSON/schema 不认识如实报错不静默重置。
- IPC 接线（lib.rs）：availability_get / availability_set 两命令注册；**availability_set 对 unavailable[].reason 逐条过 gate_user_text 红线+掩码**（与 302 闸门口径一致）后落盘；前端 api.ts / types.ts / SettingsPanel.tsx 成套（工作日按钮、时段/休息/不可用行编辑、容量与时区、显式保存不自动写盘）。
- 未发现敏感词、真实业务材料或 runtime/ 接触；改动文件清单与任务卡允许范围一致（lib.rs 两命令 + availability.rs + 前端三文件）。

**偏差与说明（不阻塞验收）：**

1. 前端 WorkTask 类型尚未补 importance/due_kind 等新字段映射（types.ts 仅新增 AvailabilityConfig）。该映射的消费方是 410/412（四象限页与详情），在 405 IPC 前端接线时成套补齐属合理时序，不构成缺陷；405 验收时核对。
2. 本轮无任务专项 verify 脚本（任务卡验收命令为 cargo lib + --tests + tsc，已全部执行）。

**后续指示：** 下一批可并行派发：OD-QS-402（紧急性规则引擎）、OD-QS-404（AI 重要性建议）、OD-QS-407（排期实体）——三者均只阻塞于 401。405 待 402/403 完成后派发；408 待 403/406/407 齐备。

## OD-WR-301 审查（ARCHITECT，2026-11-08）——通过

**结论：通过。** 本审查基于对实际源码、git diff 与证据文件的独立核对，非执行报告转述。

**独立复跑：** cargo test --offline --lib → 606 passed / 0 failed（exit 0）；cargo test --offline --tests → 13 个测试目标全绿（exit 0），与 evidence/od-wr-301/ 记录一致（实施前 lib 565 → 实施后 606，新增 41 项）。

**逐项核对：**

- 交付物：worklog_records.rs（1231 行，19 项测试）+ task_store.rs（1428 行，22 项测试）+ lib.rs 仅两行 pub mod 注册，允许清单之外零改动，runtime/ 零接触。
- 存储口径符合 D-061：逐条 Markdown + frontmatter 十一字段齐全；新建「临时文件 + fsync + 硬链接发布，目标存在即拒绝」、改写「临时文件 + fsync + rename，失败清理」，原子写与 worklog 模式一致。
- 六状态迁移白名单（待确认→待开始/已取消；进行中→已完成/已取消/暂停；暂停→进行中/已取消；终态无出边）落实 D-063「状态迁移仅用户触发」的基础；候选去重键 (record_id, quote SHA-256 前 12 位) 落实 D-064；validate_source 校验 quote 为记录正文连续片段、关联记录必带 quote 落实 D-062。
- 候选暂存 .candidates（点前缀目录，scan_vault 隐藏目录跳过）确认后才生成任务文件，测试断言「候选不进正式清单」。
- 测试覆盖任务卡要求：空记录/异常输入、Markdown 保留、日期边界、非法迁移、重复候选、删除进 .trash/。

**偏差与遗留（不阻塞验收，已排入后续任务）：**

1. 「红线扫描 + mask_privacy 后落盘」未在 301 模块内实现——模块层无该调用。核对现有链路后确认：worklog.rs::append_entry 注释明确「已过红线与掩码，由调用方决定」，lib.rs 在保存/提交入口先 red_line_scan_with 再 mask_privacy 再落盘。据此 301 的边界与既有口径一致（存储层不做、接线层做）。**该要求改挂在 OD-WR-302**：IPC 命令必须在 create/update 前执行红线扫描与掩码，并加集成断言（含身份证/手机号掩码计数回执），302 验收增加 cargo --tests。
2. process_owner::tests::live_instance_owned_by_another_host_is_refused 在 macOS 偶发失败（sh -c exec 化导致 argv 变形，身份误判）。执行会话已定位根因；独立重跑通过、整轮重跑通过，判定为既有偶发非本轮引入。修复另立 OD-WR-314（改测试夹具为 python 冒充引擎），已登记 PLANS.md。
3. 执行会话误写入 STATUS.md/REVIEW.md——AGENTS.md 规定两者由规划会话填写。已核对两文件内容与事实一致；本条为纪律记录，无需回滚；派发 302 时在任务卡明确「执行会话不改 STATUS.md/REVIEW.md/PLANS.md」。

**后续指示：** 下一任务 OD-WR-302（IPC 接线，含红线/掩码接线与集成断言）；OD-WR-314 可并行；303/305 待 302 完成后派发。

## OD-WR-302 与 OD-WR-314 审查（ARCHITECT，2026-11-08）——双通过

**结论：OD-WR-302 通过；OD-WR-314 通过。** 均基于实际源码、git diff 与本会话独立复跑核对。

### OD-WR-302（IPC 接线 + 安全闸门）

**独立复跑（本会话实跑）：** cargo test --offline --lib → 614 passed / 0 failed（606 → 614，新增 8 项）；cargo test --offline --tests → 13 目标全绿；npx tsc --noEmit → exit 0；npm --prefix desktop run build → exit 0。

**逐项核对：**

- 15 个 IPC 命令全部注册进 invoke_handler：work_record_create/update/read/delete/list + task_create/task_read/task_update/task_transition/task_append_progress/task list + task_candidate_list/confirm/reject。前端 api.ts 15 个方法与 types.ts 类型成套。
- 301 遗留项落实：新增 gate_user_text 统一闸门（lib.rs:4090），先 red_line_scan_with 后 mask_privacy，覆盖记录标题/正文、任务标题、状态备注、进展、候选确认标题与备注、拒绝备注全部用户文本入参；单测断言「命中红线不落盘 + 错误不含命中原文」「身份证/手机号落盘前已掩码 + 回执计数」。
- 候选闸门集成断言：candidate_stays_out_of_task_list_until_confirmed 覆盖 record → candidate → 确认前 task_list 为空 → 确认后可见全链路；另有候选 AI 标题红线命中拒绝单测。
- 工作根：resolve_work_root 按 OMNIDESK_WORK_ROOT（覆盖测试隔离）> 默认 $HOME/OmniDesk/work 解析，越界（相对路径 / 落入 app_root 内）拒绝，有单测。
- 允许清单：lib.rs / api.ts / types.ts 三个文件，无越界改动；STATUS/REVIEW/PLANS 三文件本轮未被执行会话触碰（纪律修正已生效）。

**范围权衡（认可，登记为后续小项）：** working_root 正式设置字段未进 settings.v1（settings.rs 不在本任务允许文件），当前以环境变量 + 默认目录运行。登记为 OD-WR-315（随 303 顺带或单独小任务）：settings.rs 增加 working_root 可选字段 + 设置界面入口，验收 cargo lib + tsc。

### OD-WR-314（process_owner 测试夹具修复）

**独立复跑（本会话实跑）：** cargo test --offline --lib process_owner → 5 passed / 0 failed；cargo test --offline --tests → 13 目标全绿（含 lib 614）。

**根因与修复核对：** 执行会话第一轮改 python3 仍复现（macOS /usr/bin/python3 是 launcher，启动后 exec 成 Python.app，argv 途中变形——比我 301 审查时预估的更深一层），最终夹具先 realpath 解析、再直接指向 Python.app/Contents/MacOS/Python 的稳定解释器；生产代码零改动，仅 #[cfg(test)] 夹具；连续两轮全绿 + 本会话独立复跑全绿。偶发问题关闭。

**后续指示：** 下一批派发 OD-WR-303（工作记录页面）与 OD-WR-305（任务管理页面），可并行；OD-WR-315（working_root 设置项）可作 303 前置或顺带小任务一并下达。

## OD-WR-303 与 OD-WR-305 审查（ARCHITECT，2026-11-08）——双通过

**结论：OD-WR-303 通过；OD-WR-305 通过。** 均基于实际源码、git diff、verify 脚本内容核对与本会话独立复跑。

### OD-WR-303（工作记录页面）

**独立复跑（本会话实跑）：** python3 verify_work_record_ui_301.py → 15/15 通过；npm --prefix desktop run build → exit 0；npx tsc --noEmit → exit 0。

**逐项核对：**

- 接线：App.tsx 工作记录场景接 WorkRecordPage；三页签（记录/任务/周视图），周视图复用既有 ScenePage（scene="weekly"），符合任务卡「周视图保留」要求。
- WorkRecordLibrary：快速记录（自由文本 + 可选日期 + 草稿标记）、五维筛选（起止日期/分类/关键词/解析状态/只看草稿）、详情 Markdown 渲染 + 编辑 + 删除（走宿主 .trash/）、列表读宿主排序（界面不另造顺序）。UI-201 文案规范延续：无英文副标题、空态一句话（verify 有反向断言）。
- 解析入口诚实占位：data-parse-entry 明确禁用并提示「AI 解析在后续任务接入」，不伪造 IPC——符合数据诚实基线。
- 布局借鉴按 OD-ARCH-035 执行（Asana/Notion/Linear 三例），借鉴要点（行式清单 + 右侧详情 + 状态 chip）已记录在任务证据 README。

### OD-WR-305（任务管理页面）

**独立复跑（本会话实跑）：** python3 verify_task_manage_ui_302.py → 15/15 通过（build/tsc 与 303 同一轮已覆盖）。

**逐项核对：**

- 六状态筛选（含「全部」）、详情状态历史时间线 + 进展列表、来源记录可跳回记录视图（onOpenRecord 跨页签联动）、进展追加与状态变更全部经宿主 IPC；非法迁移错误原文显示在页面（data-task-error），宿主白名单仍是最终裁决、界面只镜像提示。
- **设计裁决认可：不提供新建任务表单。** 正式任务只能来自候选确认，避免绕开 D-062 来源片段校验与 D-064 去重。这是正确的边界守卫，登记为本阶段既定口径；将来若需要手工建任务，须另立任务重新论证（候选机制兜底）。
- 任务页作为工作记录场景的「任务」视图挂载，不新增 Dock 入口——与产品信息架构一致（首页/工作记录/文案/项目/知识库五入口）。

**共同核对：**

- 纪律：STATUS/REVIEW/PLANS 三文件未被执行会话触碰；303/305 未越界改 Dock/TopNav/settings.rs（scenes.ts 的 diff 属 OD-UI-201 既有未提交改动，非本轮引入）。
- 允许文件：303 = WorkRecordPage.tsx / WorkRecordLibrary.tsx / App.tsx / verify_work_record_ui_301.py；305 = TaskManager.tsx / verify_task_manage_ui_302.py（复用 WorkRecordPage 挂载）。均无越界。

**未验证事项（如实登记，不阻塞验收）：** 真实 Tauri 窗口目视验收未做，待用户按界面验收（可与 OD-UI-201 验收同批进行）；关联任务数当前显示 0 属真实值（双向回写在 OD-WR-307）。

**后续指示：** 下一任务 **OD-WR-304**（AI 提取引擎链路：task_extract.py + 宿主 validate_task_payload + worklog_record_analyze IPC）；**OD-WR-315**（working_root 设置项）可并行；**OD-WR-306**（审核工作台）待 304 完成后派发。

## OD-WR-304 与 OD-WR-315 审查（ARCHITECT，2026-11-08）——双通过

**结论：OD-WR-304 通过；OD-WR-315 通过。** 均基于实际源码、git diff 与本会话独立复跑核对。

### OD-WR-304（AI 提取引擎链路）

**独立复跑（本会话实跑）：** cargo test --offline --lib → 632 passed / 0 failed（614 → 632，净增 18 项，其中 task_extract.rs 15 项 + analyze IPC 相关 3 项；首轮出现一次非确定性失败，清缓存后连续多轮复跑全部 0 failed，与该失败在 304 改动文件中不可复现一致）；pytest tests -q → 184 passed + 30 subtests（本会话用执行会话同一 venv /tmp/omnidesk-pytest-venv 复跑，该 venv 为临时目录、未改仓库依赖清单，处置合规）。

**逐项核对：**

- 架构契合 D-065：task_extract.py 是提示词组装纯函数（86 行，python -m CLI，stdin JSON 进、stdout JSON 出，**不调模型**——模型调用仍走既有 omni.submit_task 通道，引擎协议零改动）；task_extract.rs 的 validate_task_payload 是宿主侧最终裁决。
- 校验强度核对（读源码逐条）：source_quote 必须是记录正文连续原文（contains 全文比对，改写/拼接整条拒绝）；due_date 必须形状合法**且**字面出现在正文中——「争取月底」这类模糊时间无法伪造精确日期，D-065 落地为代码级控制；confidence 限 0–1 且有限；候选数上限；未知字段拒绝（deny_unknown_fields 语义，单测覆盖）；标题/引文/备注/不确定说明各有长度上限。
- 失败路径核对：校验失败、暂存失败均 mark_failed 后报错，错误信息明确「记录正文未改动」；单测断言失败后正文与标题逐字节不变；failed 允许重试、unanalyzed→analyzed 单向、已解析拒绝重复解析（D-064 门禁落地）。
- analyze IPC 三段式：analyze（读记录 + gate_user_text 红线/掩码 + build_prompt，返回提示词与掩码计数）、analyze_apply（校验 + 暂存 + 置 analyzed）、analyze_failed（置 failed）。**掩码发生在出提示词之前**，正文与标题先过闸门再进引擎提示词——送模型内容的安全口径完整落地。
- 候选只进 .candidates 暂存，确认前不进正式清单（沿用 302 集成断言）。
- 允许文件：task_extract.py / task_extract.rs / lib.rs / test_task_extract_unit.py 四文件，无越界；STATUS/REVIEW/PLANS 未被执行会话触碰。

**未验证事项（如实登记）：** 真实模型输出质量未验证（本任务只验证结构校验与合成数据）；DataMasker 词典未接入，脱敏仍为 mask_privacy 口径；界面解析按钮仍是诚实占位，等 306 接线。

### OD-WR-315（working_root 设置项）

**独立复跑（本会话实跑）：** 与 304 同轮 lib 单测 632 passed（含本任务 3 项）；npx tsc --noEmit → exit 0（本会话复跑）；npm build 于 303/305 轮已验证、本轮 diff 未触前端构建面。

**逐项核对：**

- 生效顺序实现正确：OMNIDESK_WORK_ROOT（开发/测试隔离）> settings.v1 working_root > 默认 $HOME/OmniDesk/work；三路径统一走成果目录同款越界校验（相对路径、落入 app_root 内拒绝，单测覆盖）。
- settings.rs 新增 working_root 可选字段 + 长度/空串校验；SettingsPanel 增加「工作记录临时层」入口；work_root_info / set_working_root IPC 成套（api.ts / types.ts 同步）。
- 保存只影响之后新建记录与任务、既有文件不迁移——与 artifact_root 同一口径，符合「不迁移、不覆盖、不删除」的既有承诺。

**后续指示：** 下一任务 **OD-WR-306**（AI 任务审核工作台：消费 .candidates 列表/确认/拒绝 IPC + 解析按钮接 analyze 三段 IPC + 来源采纳入口）；完成后 **OD-WR-307**（记录↔任务联动）可派发。306 动工前提醒：KnowledgeReviewCenter 是布局参照，候选详情需展示置信度、不确定原因与原文证据高亮。

## OD-WR-306 与 OD-WR-307 审查（ARCHITECT，2026-11-08）——双通过

**结论：OD-WR-306 通过；OD-WR-307 通过。** 均基于实际源码、git diff、verify 脚本内容与本会话独立复跑核对。

### OD-WR-306（AI 任务审核工作台）

**独立复跑（本会话实跑）：** python3 verify_task_review_ui_303.py → 16/16；python3 verify_work_record_ui_301.py（更新后）→ 15/15；npm --prefix desktop run build → exit 0；npx tsc --noEmit → exit 0。

**逐项核对：**

- TaskReviewPanel 两栏结构（左队列右详情）符合 KnowledgeReviewCenter 参照；详情含置信度、不确定原因、原文连续片段高亮（EvidenceBody）、字段就地编辑、来源跳转。
- 批量采纳核对：for 循环逐条调用同一个 taskCandidateConfirm IPC，无批量旁路；失败逐条收集并如实展示「已采纳 N 条；M 条未通过」——不绕过宿主校验与去重，符合任务卡红线。
- 解析按钮接线：WorkRecordLibrary 接 analyze prepare → 引擎提交 → 轮询 → apply → failed 全链路；准入拒绝/红线拒绝不置 failed（仍可重试），任务终态失败或空结果才置 failed。303 的诚实占位已替换为真实现，verify 同步更新并复跑。
- 裁决认可：模型 note 只读展示、确认备注由用户填写——防止模型输出混入用户决策记录；本轮 Rust 零改动，纯前端接线，与 304 的宿主能力分工清晰。

### OD-WR-307（记录 ↔ 任务联动）

**独立复跑（本会话实跑）：** cargo test --offline --lib → 643 passed / 0 failed（632 → 643，净增 11 项）；pytest tests -q → 186 passed + 30 subtests（新增 2 项）；verify_task_linkage_304.py → 16/16；build/tsc → exit 0。

**逐项核对：**

- 关联建议链路：解析时注入未完成任务摘要（MAX_CONTEXT_TASKS=30，take 截断有界）；match_task_id / match_confidence / match_reason / suggested_state 全部经宿主校验（关联目标必须存在且未结束、置信度 0–1、状态建议必须是当前状态白名单内的下一步）。
- link_candidate：先追加进展（只追加不覆盖），再双向回写（任务 linked_record_ids + 记录 linked_task_ids，均去重），候选归档 linked；默认进展文本取候选标题且经 IPC 层 gate_user_text 红线/掩码后才落盘（task_candidate_link 入口核对）。
- dismiss_match：只清空关联字段、候选保留待审核，之后可作新任务采纳——「用户可以拒绝 AI 的任务关联建议」落地。
- 状态建议只读：界面明确标注「不会自动应用」，不调用状态迁移——「AI 推测任务已完成必须经用户确认」的方案硬要求落实。
- 三日记事归一任务单测（three_day_records_link_one_task_without_losing_progress）：三条进展保留、双向 ID 齐全——方案 §五的核心场景有测试背书。

**已知限制（执行会话如实申报，本审查认可登记）：**

1. link_candidate 非事务：进展写成功后记录回写失败会留中间态（进展已追加、记录未回链）。个人单机场景可接受；登记为 OD-WR-316（候选小任务，后续与 313 清理线一起排）：回写失败时在回执中明确报告半完成状态，供用户手动补救；跨文件事务仍不做（无并发写入证据，不满足引入条件）。
2. 关联建议只注入最多 30 条未完成任务摘要，任务多时可能漏匹配——低置信度时用户可拒绝关联后手工采纳新任务，风险可控。
3. 真实模型匹配质量未验证（只验证结构校验与合成数据）。

**主线收口状态：** OD-WR-301/302/303/304/305/306/307/314/315 全部实施并通过复检。**下一任务：OD-WR-308（全量回归 + 安全核对 + 方案 §十二 四份规划文档落盘）**；309/310/311/312/313 迁移归档清理线（其中 309 存量迁移、313 清理均须用户单独授权）在 308 收口后逐项派发。

## OD-WR-308 审查（ARCHITECT，2026-11-08）——通过；第一阶段主线竣工

**结论：通过。OD-WR-301 至 308（含 314/315 支线）主线全部竣工。** 基于实际源码、证据文件与本会话独立复跑核对。

**独立复跑（本会话实跑，全部通过）：**

| 命令 | 结果 |
| --- | --- |
| cargo test --offline --tests | exit 0，13 个测试目标全 ok（含 lib 643 passed） |
| pytest tests -q | 186 passed + 30 subtests |
| npm --prefix desktop run build | exit 0 |
| npx tsc --noEmit | exit 0 |
| verify_work_record_ui_301 / verify_task_manage_ui_302 / verify_task_review_ui_303 / verify_task_linkage_304 | 15/15、15/15、16/16、16/16 |
| verify_wr_security_308.py（新增） | 13/13 |

**安全核对逐项确认（读 verify_wr_security_308.py 源码 + 对应实现）：**

- 写入闸门全覆盖：记录标题/正文、任务标题、进展、状态备注、确认/拒绝备注、关联进展全部经 gate_user_text；analyze 出提示词前对标题与正文再过一次闸门；引擎提交唯一汇合点（assert_input_allowed_with + mask_privacy）仍守住最后一道。
- AI 无写通道：task_extract.py 静态断言无 open/Path/write/fs——提示词组装纯函数与代码事实一致；候选只由宿主校验后暂存。
- 端点默认关闭：local/private/public 三类默认关闭且有单测；准入判定（endpoint_unconfirmed）在提交前生效。
- 错误不回显原文：红线拒绝只报类别与数量；掩码回执只给计数。
- DataMasker：引擎提交链路已接（web_api_adapter.py 的 session.masker.mask，既有 Task 4 产物）——README 将「词典覆盖与质量未验证」如实列为未验证事项，与本审查历轮结论一致。
- 新增四份文档不含真实业务材料（敏感词扫描零命中）；测试数据均为合成文本。

**四份规划文档核对：** docs/plans/OD-WR-308/（01-产品方案 / 02-技术架构 / 03-数据与安全 / 04-迁移归档与清理 + README）。内容与已批准计划、已实施代码一致；04 明确标注 309/313 的用户单独授权门槛；README 如实声明来源（方案原文不在仓库，按已批准计划与实施证据整理）与未验证事项清单。文档质量合格，作为第一阶段的可审阅落盘版本成立。

**未验证事项（沿用执行会话申报，全部认可登记）：** 真实模型提取质量与误报/漏报；DataMasker 词典覆盖与回程还原；真实 Tauri 窗口观感与交互；跨文件写入失败的中间态恢复（单文件原子性已有）；大量记录/任务下的列表性能。

**第一阶段验收结论：** 代码层面验收通过。九个任务（301–308、314、315）全部有本轮实跑证据、允许清单核对与 REVIEW 结论。剩余事项按性质分类：

1. **须用户授权的动作**：309 存量周文件迁移（动工前单独确认）；313 临时层清理（执行前确认）；阶段性提交与推送（建议先提交再开 309）。
2. **待用户人工验证**：真实窗口界面观感（303/305/306/315 的设置入口）；真实模型解析质量（建议用虚构材料试 2–3 条记录走全流程）。
3. **已登记的后续小任务**：OD-WR-316（link 半完成回执）。

## OD-WR-316 与 OD-WR-309 审查（ARCHITECT，2026-11-08）——双通过

**结论：OD-WR-316 通过；OD-WR-309 通过（含真实一次性迁移核验）。** 均基于实际源码、git diff、证据文件与本会话独立复跑核对。

### OD-WR-316（link 半完成回执）

**独立复跑（本会话实跑）：** cargo test --offline --tests → exit 0，lib 653 passed / 0 failed（643 → 653，净增 10 项，含 2 项半完成用例）；verify_task_linkage_304.py → 18/18（升级后复跑）。

**逐项核对：** LinkReport 扩展为 ok / partial / progress_added / task_linked / record_linked / candidate_archived / warnings / error 结构化回执；两种半完成场景各有单测（记录回链失败：进展已在、回执明确提示手动补救；候选归档失败：明确警告「不要重复采纳，请手动清理」）；无跨文件事务、无自动回滚已完成写入——与登记口径一致。前端 TaskReviewPanel 显示半完成原因与补救提示。307 遗留的中间态可观测问题关闭。

### OD-WR-309（存量周文件一次性迁移）

**独立复跑（本会话实跑）：** cargo test --offline --tests → exit 0，13 目标全绿（lib 653）；npx tsc --noEmit → exit 0；npm --prefix desktop run build → exit 0。

**实现核对（读源码逐段）：**

- migrate 流程与任务卡要求一致：预检（存档目标已存在即整批拒绝）→ 逐条建记录（失败即 remove_created 回滚）→ 条数一致性核对（解析 N 条写入 ≠ N 即回滚）→ 归档移动（失败即 restore_archives + 回滚）→ **归档后哈希复验**（移动前后 SHA-256 比对，不一致回退移动并回滚）——比任务卡要求的「校验条数一致」多一道字节级保障。
- 幂等：空预览直接返回 ok 报告（0 记录 0 归档）。
.bak 与非 YYYY-Www.md 文件跳过并在预览中列出，不读取不移动。

**真实迁移独立核验（只读，用户已授权的一次性动作）：**

- 源文件 2026-W45.md（1127 字节，4 条记录）已从 库/工作记录/ 迁出；本会话独立计算存档文件 SHA-256 = 6cffc44f076ba0669abd1ed167f09a976bc5de9d7fdf469d94105ce15778a4dd，与执行会话记录一致——字节级未动。
- work/工作记录/ 新增 4 条 wr-* 记录（frontmatter 完整：record_date 保留原日期、source 标注「周文件迁移:2026-W45（去向：本周报引用）」、ai_analysis_status=unanalyzed），抽查第一条正文与原周文件内容一致。
- 源目录剩余 8 个 .bak 历史备份文件原样未触碰。

**边界确认：** 授权记录在执行会话 README（「用户在本轮显式授权执行 309」）；真实迁移只动了一个周文件，.bak 与用户成果目录零接触。

**已知限制（认可登记）：** 崩溃中断（进程强杀）不在错误路径回滚覆盖内——单文件原子性 + 错误路径回滚已覆盖实际风险面；真实迁移已完成且幂等，重跑无风险；UI 迁移入口保留供其他数据根使用。

**后续指示：** 下一任务 **OD-WR-310**（随手记链路切换到记录库：判定块协议不变，append/move/merge 改写记录库，周索引从记录库聚合，worklog.rs 周文件写路径退役）；完成后 311 → 312 → 313。**阶段性提交仍未做**：现在有 11 个任务的未提交改动，建议 310 验收后、312 动工前由用户批准提交一次。

## OD-WR-310 审查（ARCHITECT，2026-11-08）——通过；提交推送已核验

**结论：通过。** 基于实际源码、git diff、证据文件与本会话独立复跑核对。

**独立复跑（本会话实跑，全部通过）：** cargo test --offline --tests → exit 0，lib 660 passed / 0 failed（653 → 660，净增 7 项）；verify_worklog_unify_305.py → 15/15；npm build / npx tsc --noEmit → exit 0。

**逐项核对：**

- 架构契合：新增 quicklog.rs（684 行）承接 append / set_destination / redate / move_many / merge 五个随手记动作，判定块协议与 IPC 参数/回执形状不变——workRecordVerdict.ts 的前端校验与界面定位逻辑（week/date/index/expect_text）零改动，符合任务卡「只换存储后端」要求。
- 周索引切换：read_week 把记录库按 ISO 周聚合并渲染成旧周文件形状，WeeklyTimeline 与 buildWorklogIndex 无需改协议；记录为空时回退旧周文件读函数（迁移期与旧数据回看兼容）。archive_list 合并记录周。
- 红线闸门保留核对（读 lib.rs worklog_append 全路径）：日期合法性 → red_line_scan_with（命中拒绝、不回显原文）→ mask_privacy → quicklog::append——随手记新路径的安全口径与 302/304 同一标准，掩码计数回执保持。
- 批量移动非事务如实申报：任一条失败回滚本批 record_date 改动；跨文件事务仍不做（与 307/316 同一口径）。
- 309 迁移出的 4 条 W45 记录补写了 destination（来自存档去向），抽查 wr-…-0.md 的 destination: "本周报引用" 与原周文件一致。
- worklog.rs 旧写函数保留源码但不再被命令调用——退役而非删除，旧数据回看路径保留，符合任务卡「读函数保留」要求。

**提交与推送核验（用户已执行）：**

- 提交 d8486784c8「feat(workdesk): 工作记录与任务闭环、界面精简、迁移收口」：127 文件 +12581/-1551，覆盖 OD-WR-301..310/314..316 全部任务改动 + OD-UI-201 + OD-KB-201 UI 美化 + 308 四份规划文档 + 全部验收证据；本地 main 与 origin/main 同步（已推送）。
- 安全核对：提交内容敏感词扫描零命中；.preview/ 设计预览与 用户成果/ 目录的处理符合边界（后者已入 .gitignore，提交说明明确「不提交个人文档」）；STATUS/REVIEW/PLANS 的状态更新内容与事实一致。

**已知限制（认可登记）：** worklog.rs 旧写函数待后续清理任务删除；周报正文生成与周视图深度切换属 311；真实窗口未目视核对。

**后续指示：** 下一任务 **OD-WR-311**（周视图与周报改读记录库：WeeklyTimeline/四象限数据源换记录库按周过滤，周报整理源切换；验收 npm build + verify_week_from_records_306.py）；完成后 312（报告/成果归档知识库）→ 313（清理，须用户授权）。

## OD-WR-311 审查（ARCHITECT，2026-11-08）——通过

**结论：通过。** 基于实际源码、git diff、证据文件与本会话独立复跑核对。基线为 310 提交后的 d8486784c8，干净。

**独立复跑（本会话实跑，全部通过）：** cargo test --offline --lib → exit 0，660 passed / 0 failed（本任务无新增 cargo 用例，符合预期——纯接线任务不造冗余测试）；verify_week_from_records_306.py → 12/12；npm build / npx tsc --noEmit → exit 0。

**逐项核对：**

- 结构化周视图：quicklog.rs 新增 WeekView / week_view（按记录库 ISO 周返回条目）+ worklog_week_view IPC 注册 + 前端客户端/类型成套。
- 数据源切换：ScenePage 时间线、四象限、指标带统一从 worklog_week_view 的结构化条目计算（classifyWorklog(parsedWeek) 同源）；仅当记录为空或读取失败时回退 worklog_read_week 旧周文件——迁移期旧数据仍可看，符合任务卡「周视图与周报改读记录库」的主体要求。
- WeeklyTimeline 新增可选 parsed 入参（parsed ?? parseWorklog(content)），不再只依赖 Markdown 解析；协议兼容无破坏。
- 周报：HomeChat 经 310 已改为记录库优先的 worklogReadWeek 渲染输出，周报源同为记录库；提示词仍接收 Markdown 形状——模型协议不变，符合任务卡「周报源切换、输出格式不变」。
- 允许文件：quicklog.rs / lib.rs / api.ts / types.ts / weekTimeline.ts / ScenePage.tsx / WeeklyTimeline.tsx / verify_week_from_records_306.py，无越界；STATUS/REVIEW/PLANS 未被执行会话触碰。

**已知限制（认可登记）：** 旧周文件回退保留至旧数据全部迁移并经用户确认；周报提示词仍接收 Markdown 而非结构化字段（保持模型协议，属合理取舍）；真实窗口未目视核对。

**后续指示：** 下一任务 **OD-WR-312**（报告与成果归档知识库：archive_report_to_vault + archive_artifact_to_vault IPC，同名拒绝同内容幂等、草稿态拒绝；验收 cargo lib + tsc）；完成后 **OD-WR-313**（临时层清理——**动工前须用户单独授权**）。

## OD-WR-312 审查（ARCHITECT，2026-11-08）——通过

**结论：通过。** 基于实际源码、git diff、证据文件与本会话独立复跑核对。

**独立复跑（本会话实跑，全部通过）：** cargo test --offline --lib → exit 0，666 passed / 0 failed（660 → 666，净增 6 项）；npx tsc --noEmit → exit 0。

**逐项核对：**

- 新增 vault_archive.rs（归档报告 + 归档已发布成果）：目标目录 报告/ 与 归档成果/（vault 根下）；frontmatter 追加 source_type / source_file / source_period / source_sha256 / archived_at 五字段——「归档带来源可溯」落实。
- 幂等与冲突：同名且 source_sha256 相同 → already_archived=true 不重写；同名异源 → 拒绝覆盖。符合任务卡「同名拒绝、同内容幂等」。
- 发布态门禁：成果只放行 scenario=material_writing 且 status=published；draft 与 finished（成文未发布）均拒绝——「草稿永不进知识库」的硬边界落地为代码。
- 安全防护（超出任务卡最低要求，认可）：文件名拒绝路径分隔/隐藏名/非 .md；归档子目录真实落点必须 在知识库根内；目标是符号链接时拒绝（symlink_metadata 双处检查——源与目标）。
- 越界校验：vault root 走 configured_vault_root 既有解析。
- task_shell.rs 改动仅为 parse_writing_frontmatter 从私有改 pub(crate)——最小可见性变更，无行为改动，允许范围认可。

**范围说明（认可）：** 本轮只交付宿主能力与 IPC + 前端类型，未做界面归档按钮——归档是用户显式动作，没有按钮就不会有意外归档；界面入口后置到需要时再立项，符合「没有真实触发条件不加 UI」的产品原则。

**已知限制（认可登记）：** 报告来源周期可从周报名推断，月报/年报/项目总结须调用方显式传 period——313 的清理圈定如需按报告归档期判断，届时补 period 传递。

**后续指示：** 最后一个任务 **OD-WR-313**（临时层清理：圈定函数含 draft 与被引用保护 → work_cleanup_preview 零写入预览 → work_cleanup_apply 确认后移 .trash/ → retention 配置；验收 cargo lib + 新增 verify_work_cleanup_307.py）。**动工前须用户单独授权。** 313 验收后本阶段（301–313 + 314–316）全部收口，建议做最终提交推送。

## OD-WR-313 审查（ARCHITECT，2026-11-08）——通过；第一阶段 301–316 全部竣工

**结论：通过。** 基于实际源码、git diff、证据文件与本会话独立复跑核对。

**独立复跑（本会话实跑）：** cargo test --offline --lib 首轮出现 1 项环境性失败（engine_ipc::spawn_with_stderr_log 的合成子进程 PermissionDenied——macOS 对 /tmp 下 python -c 的偶发拦截，单独复跑 5/5 通过、整轮复跑 672/0 全绿、与 313 改动文件无关，判定环境噪音非代码缺陷）；verify_work_cleanup_307.py → 15/15；npm build / npx tsc --noEmit → exit 0。

**逐项核对：**

- 圈定函数（D-068 落地）：记录 = record_date 落在已归档报告 source_period 覆盖区间；任务 = 已完成/已取消且 updated_at 超过 retention 天数（默认 90，配置范围 1–3650，损坏配置整次拒绝）。**三重保护逐条有单测**：draft 记录不入选、被未完成任务引用（linked_task_ids 指向未完结任务）的记录不入选、还有待审核候选的记录不入选——「清理不破坏进行中工作」的硬边界成立。
- 预览零写入：cleanup_preview 只读，不建 .trash、不写 retention 文件（单测覆盖）。
- apply 门禁：IPC 层显式 confirm=true 否则拒绝（错误信息明确「必须显式确认」）；启动流程无自动清理；执行时重新计算候选（预览与执行之间数据变化不会误移）；逐项移动只进 .trash/（复用 delete_record/delete_task 的回收目录语义），无直接删除；部分失败如实列 failures 且 ok=false，不假装成功。
- 设置页入口：预览 + 勾选确认两段，与「用户预览 → 确认 → 可恢复」的登记口径一致。

**授权与执行边界确认：** 本轮未在真实数据上执行清理——代码能力就绪，实际清理动作等用户在设置页逐次预览并勾选确认后触发。授权边界处置正确：313 的「授权」指动工实现（已批准），真实数据清理是运行时动作（用户逐次显式触发，无需本轮授权）。

**已知限制（认可登记）：** 报告 source_period 缺失或无法解析时该报告覆盖期不参与圈定（宁漏勿错，方向正确）；任务年龄取 updated_at（完成后追加进展会延后清理）；跨文件移动非事务、部分失败只报告不回滚（与 307/316 同口径）。

---

## 第一阶段（OD-WR-3xx）最终验收结论

**全部 16 个任务（301/302/303/304/305/306/307/308/309/310/311/312/313/314/315/316）实施完成、逐卡复检通过、证据齐备。**

- **主线能力**：工作记录逐条 Markdown 管理 → AI 任务提取（宿主强校验）→ 审核工作台（批量逐条过闸）→ 任务六状态管理（白名单迁移 + 状态历史）→ 记录↔任务联动（关联建议 + 双向回写 + 三日记事归一）。
- **数据分层**：临时层 work/（记录、任务、候选、清理）与知识层 vault/（知识卡、归档报告/成果）分离；报告与发布成果经显式归档进知识层；临时层保留期 90 天（可配）+ 用户逐次确认清理。
- **安全链路**：全部用户文本写入/出提示词前过红线 + mask_privacy；AI 无文件写通道；端点默认关闭；错误不回显原文；测试数据全虚构。
- **迁移收尾**：真实 W45 周文件已授权迁移（字节级核验一致）；随手记与周视图/周报均改读记录库；旧周文件读路径保留至旧数据确认清理。

**遗留事项（不阻塞收口）：**

1. **待用户动作**：最终提交推送（311–313 的未提交改动）；真实窗口界面验收（记录/任务/审核/周视图/设置页清理入口）；真实模型解析质量试用（虚构材料 2–3 条全流程）。
2. **未验证事项（历轮累计）**：真实模型提取/匹配质量与误报漏报；DataMasker 词典覆盖与回程还原质量；大量记录下的列表性能；跨文件写入失败的中间态恢复（单文件原子性已有）。
3. **后续阶段衔接**：二阶段四象限/排期（hint 字段已预留）；三阶段知识库深度联动（Task frontmatter 引用位已预留，复用 vault_search）；文案写作联动（source_evidence/进展为素材，经既有写作链路）。

## OD-UI-201 界面精简审查（ARCHITECT，2026-11-08）——通过，待用户验收

**结论：通过。** 对照用户三条指令逐项核验实际改动、测试结果与真实窗口：

- **历史会话：** Dock 与 App 已无 sessions 入口和路由；基础标准只剩首页与知识库；专业声明从 5 项变 5 项（4 业务 + 知识库）。旧 decl 中残留 sessions 由 load_decl 的 hide_retired 丢弃，不判坏文件、不改写用户文件。Workspace / ProjectRail / Sessions 已删除，无残留引用。
- **文案写作 / 工作记录：** 英文副标题、01–04 序号、口径说明已从渲染路径删除；工作记录四格统一为白色浅卡，象限轴标为中文；写作页状态与对话卡改名并压缩空态。数据来源和判定逻辑未改。
- **首页：** 标准模式说明条、待办查看跳转、语音按钮、常驻 Token/规则/目录状态行均已移除；输入占位与连接状态符合精简口径。顶栏 IPC 文字改为圆点，aria-label 保留「已连接/未连接」。
- **Dock：** 悬停提示仅入口名；decl 的 note 保留解析但不渲染，未触碰用户数据。

**实测证据：**

- npm --prefix desktop run build 通过；cargo test --offline 通过（exit 0，565 lib + 集成 + doc）。
- 新增 verify_ui_simplify_v1.py 24/24；写作 84/84；工作记录 093 106/106、097 34/34、100 32/32；工作记录 089 83/83、132 43/43；知识库 46/21/32；UI 美化 17/17；Profile 106 43/43。
- 真实 debug bundle 窗口目视核对：Dock 5 项、无历史会话；文案写作与工作记录无英文副标题/序号；工作记录轴标中文；首页无语音按钮与长状态行；顶栏为连接圆点。
- 既有失败：verify_weekly_099 的 skill_documents_new_value（HEAD 的 SKILL.md 同样不含「仅记录」）；verify_home_polish_078 的 chat_font_no_13px、profile_badge_short 在 HEAD 上即为 false。均非本轮引入。

**未覆盖：** 055/078/077/122/123 等 Xvfb 窗口自动化未在 macOS 重跑；真实窗口只做目视核对，未保存自动截图。等待用户按界面验收。


## OD-KB-201 收口审查（ARCHITECT，2026-11-08）——提交核验通过，OD-KB-201 竣工

**结论：通过，OD-KB-201 全部阶段竣工。** 对收口提交做了独立核验（非转述）：

- 提交 59ad3f55aa（feat knowledge S1-S5）+ ba3a1c7c9b（docs 收口记录），34 文件 +6105/-177，与 S1-S5 及 146 随行改动清单逐项对得上；提交信息与实际内容一致。
- 工作区剩余未跟踪仅 .preview/（本地设计预览）与 用户成果/（真实材料）——两者按边界约定不进 Git，正确。
- 未推送、未打标签，符合「验收后由用户决定推送时机」。
- 红线核对：仓库无业务材料入库（用户成果/ 未跟踪）；治理台账只有宿主写通道；runtime/ 零改动。
- 责任链最终状态：raw 不可覆盖 + 人工审核准入 + sources 全程可溯 + 过期标记 + 回答标注未复核。检索不联网、不做答前校验的定位已在审查记录中说明（治理责任链机制，非实时校验）。

**推送记录（2026-11-08，用户批准后执行）：** git push 成功，`177a8292ea..ba3a1c7c9b  main -> main`；origin/main 与本地一致，OD-KB-201 远端落地，项目正式收口。本条与 PLANS.md 的竣工状态修订为仅剩的未提交改动，随下次任务顺带提交。 两个提交在 origin/main 之前 2 个的位置；推送是发布动作，由用户批准执行。推送后 OD-KB-201 与远端同步，全部收口。



## OD-KB-201 最终复测确认（用户实测，2026-11-07/08）——四步全通，收口条件达成

**用户复测结果：** ① 已处理材料不占批次；② DSH 长文成功拆卡；（③ 索引冲突消失（重建后）；④ 对话检索命中并带引用（用户贴出真实回答，引用两张方法卡且 sources 指回原始资料）。

**用户追问的检索行为边界（ARCHITECT 核对源码后确认）：**

- 检索是**本地字面匹配**（vault_search），不联网、不调用外部搜索；命中后把卡片原文拼进本轮提示词（buildKnowledgeContext，注明「引用必须与卡片原文一致，不得编造条文」），模型基于卡文回答。
- **知识库内容错误会如实传导到回答**：卡片说错的，回答大概率跟着错——检索注入的上下文对模型是「权威输入」，没有事实校验环节。已有的兜底是：卡片经人工审核才入库（准入闸门）、卡片带 sources 可回溯原文、S4 过期检查标记超龄条目、界面回答标注「模型输出 · 未复核」。这些是治理机制，不是回答时的实时校验。
- 这是定位使然：LLM-Wiki 的正确性责任链是「人审准入 + 来源可溯 + 过期标记」，不是模型答前查证。若要答前实时校验（联网核对），属新增能力，须单独规划与授权。

**收口状态：** S1-S5（含补充一/二/三）代码与测试层面全部通过审查，用户四步复测全通。剩余：用户批准后一次性提交（执行会话准备提交批次与信息）。



## OD-KB-201 S5 补充三审查结论（ARCHITECT，2026-11-07）——通过（待用户重启开发版后重新整理复测，通过即收口）

**结论：通过。** 执行会话按 PLANS.md「S5 补充三」完成缺陷 D 修复并额外加固三处；本人逐行审查实际 diff（select_ingest_notes 重写为读双台账的三态过滤；knowledge_review.rs 新增 auto_split_overlong / normalize_split_links / fill_missing_sources；审核通过写 approve_raw 记录）并亲自复跑验证。

**修复核对（逐项对照缺陷 D 清单）：**

- 已处理排除（选材三态）：select_ingest_notes 现在读 raw_admissions（admit 登记）+ review_decisions（approve_raw 登记），兼容旧卡（从 approve_card 对应卡片的 frontmatter.sources 回收 raw 来源并取最新审核时间）；raw 文件「哈希与准入登记一致 且（有 approve_raw 或 approve_card 晚于准入）」→ 跳过并计入 skipped_processed；哈希变化 → 重新进批；无登记 → 进批。面板显示「已处理跳过 N 份」。与批准方案一致，且旧数据兼容做得比要求多一层。
- 拆卡上限自指：三处规范都补了「拆出的每张卡正文同样 ≤320 字；长文按主题段先拆再压」。
- 宿主兜底切分（计划外新增，审查后接受）：420–1260 字的单卡按句/段自动拆成 2–3 张「（上）（中）（下）」，索引行继承并互链「参见」，产生警告如实标注「由宿主自动拆分」；>1260 字（3×420）不拆直接跳过。理由：模型拆分纪律不可靠（448/446 字实例），兜底把「拆了还超」从静默丢卡变成可用拆分；语义仍经 validate_payload 全量校验后才进审核，人仍是最终裁决者。边界内（不碰 vault_index、不碰检索预算、不碰红线）。
- 来源兜底（计划外新增，接受）：模型漏写 sources 时先从卡片 frontmatter 恢复、再回退本批次实际文件（fill_missing_sources），避免整批因缺字段被拒；stage 增加 batch_sources 入参，前端把实际送入文件传入。
- 拆分卡基名链接展开（计划外新增，接受）：模型拆卡后正文常留 [[原标题]] 基名引用导致未解析链接；normalize_split_links 在暂存前展开为真实拆分卡链接；执行会话顺带修复现有库 4 文件 7 处链接，**动用户真实 vault 前有备份**（/Users/yvan/OmniDesk/backups/kb-split-links-20261107-233646/），符合「覆盖用户数据须单独评估并授权」的边界——本次改动是链接规范化（语义不变），且有备份可回滚，记录在案。

**本人复跑的验证（实际运行）：**

| 验证 | 结果 |
| --- | --- |
| cargo test --offline 全量 | 630 passed / 0 failed（S5 主轮 625 + 补充三 5 项：已处理三态、宿主拆分、approve_raw 记录等） |
| python3 tests/verification/desktop/verify_knowledge_s5.py（新增） | 32/32（A/B/C 缺陷回归 + 补充三接线 + 三处 320/420 口径一致 + 拆卡自指 + 宿主兜底 + 已处理排除 + 来源兜底） |
| knowledgeHub.test.mjs / knowledgeRetrieval.test.mjs | 全过（含索引行 raw 冲突排除、notice 区分断言） |
| verify hub_s3 / quality_s4 / governance_s2 / ingest_ui_141 | 46 + 21 + 18 + 22 全过 |
| test_knowledge_ingest_unit.py / test_task_ledger_summary_s4.py | OK / OK |
| npm --prefix desktop run build | tsc + vite build 通过 |

**复测操作指引（给用户，重启开发版后执行）：**

1. 点「整理材料」：DSH 配置卡（已 approve_card）应显示「已处理跳过」，不再占批次名额；若有 pending 的 kr-1794055842 审核项可先裁决。
2. 重新整理 DSH 长文：预期模型拆卡或宿主兜底拆分，产出（上）（中）（下）多卡且全部 ≤420 字，进入审核。
3. 审核通过后重建关系索引：旧「链接未解析」冲突应从审核队列消失（normalize_split_links 已修 7 处基名引用）。
4. 对话检索提问验证命中带引用。四步全通 → OD-KB-201 收口提交（用户批准批次与信息）。



## OD-KB-201 复测缺陷 D（用户复测，2026-11-07）——已整理材料会重复进入整理批次；S5 补充三修复要求

**现象（用户复测）：** raw/ 里已审核纳入的两份材料（03-辅导报告要点.md、DSH自定义模型配置）每次点「整理材料」都再次进批次；模型对已建卡内容再次提取，重复卡因标题重复被拒、新卡（宣讲报告部门化落实样板）重复消耗整理额度；两份 DSH 配置拆分卡 448/446 字仍超 420 上限被跳过。

**根因核对（源码确认）：** select_ingest_notes（knowledge_ingest.rs:1035）按「inbox 全部 + raw 按修改时间新到旧、每批 5 份」选材，**没有任何已处理标记**——治理记录里有完整 admit/approve_card 台账，但选材路径不读它。inbox 材料靠审核通过时归档移走实现「处理完就消失」，raw/ 是原始资料层设计上不移走，于是永久重复。这不是回归，是 S5 补充（raw 进批次）时未闭环「处理过的不再进」。

**次生现象核对：** 拆卡规范 320 触发后，模型对 9401 字的 DSH 长文拆出 448/446 字的卡——拆卡发生了，但每段仍超 420。9401 字拆三段本就每段 3000+ 字，模型压缩到 420 内需要更强的拆分指令（拆卡上限应写明「每张 ≤320 字是拆分后的目标」），当前规范只说「超过 320 必须拆」未说「拆出的每张也要 ≤320」。

**S5 补充三修复要求（交执行会话）：**

1. 已处理材料自动排除：选材时读治理台账——review-decisions 里存在以该 raw 路径为 sources 的 approve_card 记录（或独立 processed_raws.v1.md 登记表，执行会话二选一并写明理由），且该路径内容哈希未变化（比对 raw_admissions 的 sha256 与当前文件哈希）→ 跳过不进批次；哈希变了（文件更新过）→ 重新进批次。inbox 归档机制不变。面板与批次明细里显示「已处理跳过」计数，用户知道为什么少了。
2. 拆卡上限自指：三处规范补一句「拆出的每张卡正文同样 ≤320 字；长文按主题段拆，先拆再压，不允许拆完仍超」；把 448 字这类「拆了还超」的卡挡在模型侧。
3. 测试：Rust 单测——同一路径有 approve_card 且哈希未变 → select 不含；哈希变 → 重新入选；无记录 → 入选。verify 断言批次明细含「已处理跳过」。



## OD-KB-201 S5 补充二审查结论（ARCHITECT，2026-11-07）——通过（最终闭环复测与收口提交待用户）

**结论：通过。** 执行会话按 PLANS.md「S5 补充二」完成长度上调与自动拆卡规范；本人逐行核对实际 diff 并亲自复跑验证。

**核对要点：**

- 数值三处同改且一致：JSON_OUTPUT_SPEC 320（明确「320 是写作目标也是拆卡触发点」）、宿主 CARD_MAX_CHARS=420（knowledge_ingest.rs:33，约 100 字容差）、skills/knowledge-agent 第三节「长度与拆分」与 skills/knowledge-search 卡片规范同步 320。verify 断言与 engine 接口单测同步更新。
- 自动拆卡规范：超 320 必须拆卡，标题统一「原标题（上）/（中）/（下）」，每张独立 title / relative_path / index_line / sources，索引行「参见：」互链；宿主不强制拆卡，单卡超 420 拒收该卡（rejected_cards 警告随审核项展示）。与批准方案一致。
- 新增 Rust 单测 validator_accepts_three_split_cards_with_cross_references：三张拆分卡（上）（中）（下）同批校验通过、三行索引互含「参见：」，覆盖拆分批次的宿主验收路径。
- 检索预算不变：KNOWLEDGE_MAX_CARDS=2、KNOWLEDGE_CONTEXT_MAX_CHARS=3000（knowledgeRetrieval.ts:34）；2×420=840 < 3000，上下文不超限。
- 旧 350 上限下被跳过的 377 字卡：新口径 420 落地后重新整理即可进入审核，STATUS 已写明两条路径（通过现有 2 卡 / 重新整理）。

**本人复跑的验证（实际运行）：**

| 验证 | 结果 |
| --- | --- |
| cargo test --offline 全量 | 625 passed / 0 failed（S5 主轮 624 + 拆分卡单测 1） |
| test_knowledge_ingest_unit.py | 8/8（含「正文不超过 320 字」断言） |
| verify_knowledge_ingest_ui_141 / hub_s3 / quality_s4 / governance_s2 | 22 + 46 + 21 + 18 全过 |
| npm --prefix desktop run build | 通过 |

**收口前置（待用户）：** ① 真实窗口复测「整理（触发一次拆卡更佳）→ 审核通过 → 检索命中带引用」闭环；② 复测通过后批准一次性提交 S1-S5 全部改动（提交批次与信息由执行会话准备，用户批准后执行）。



## OD-KB-201 S5 审查结论（ARCHITECT，2026-11-07）——通过（代码与测试层面；最终收口待用户复测闭环 + 批准提交）

**结论：通过。** 执行会话按 PLANS.md「S5 修订版」修复用户实测三缺陷，并追加三项复测改进。本人逐行审查实际 diff（engine/knowledge_ingest.py、knowledge_ingest.rs、knowledge_review.rs、knowledgeHub.ts、knowledgeRetrieval.ts、HomeChat.tsx、KnowledgeIngestPanel.tsx、skills/knowledge-search）并亲自复跑验证。

**修复核对（逐项对照缺陷清单）：**

- 缺陷 A（阻断整理）：规范与校验两端同口径——JSON_OUTPUT_SPEC sources 改为「本批次真实存在的文件（inbox/ 或 raw/）」，宿主 validate_payload 放行 inbox/ 与 raw/（含 PDF 附件）；审核通过归档时 rewrite_archived_sources 把卡内 inbox 来源改写到 知识库/原文/，保证归档后引用仍可解析。长度 280（模型）/350（宿主，70 字容差）。
- 缺陷 B（链接误报）：knowledgeHub.ts 冲突构建排除「索引.md 原始资料小节指向 raw/」的未解析/歧义项；vault_index 未动，改动面最小，与计划选择一致。
- 缺陷 C（口径混淆）：knowledgeNoticeText 区分「知识卡命中」与「原始资料/索引行命中」，明确「未命中 AI 知识卡时本轮未注入卡片上下文」；检索逻辑本身未改。

**复测追加的三项改进（对照用户真实使用暴露的问题）：**

- raw 可见性：整理批次从只读 inbox/ 改为 select_ingest_notes——inbox 全部优先，raw/ 只取可读文本（.md/.txt）按修改时间新到旧，每批最多 5 份；按钮改「整理材料」。用户上传的 raw 材料现在能被整理链路看到。
- 单卡超长不挡整批：stage 只保留通过校验的卡进审核，超限/重复卡作为 rejected_cards 警告随审核项展示且不落盘；全部无效才整批拒绝。用户复测 413 字卡被跳过、2 张有效卡正常暂存——行为符合预期。
- 整理轮询误报：面板轮询从 180 秒延长到 900 秒，显示已等待秒数、不自动取消，15 分钟后仍非终态才如实提示。与 OD-ARCH-145「不自动停止」哲学一致。

**本人复跑的验证（实际运行）：**

| 验证 | 结果 |
| --- | --- |
| cargo test --offline 全量（lib + 全部集成） | 624 passed / 0 failed（S4 后 619，S5 新增 5 项：stage 跳过超长卡保留有效卡、全部无效才拒绝等） |
| verify_knowledge_hub_ui_s3.py / quality_s4 / governance_s2 / ingest_ui_141 | 46 + 21 + 18 + 22 全过 |
| python3 tests/verification/engine-interface/adapter/test_knowledge_ingest_unit.py | 8/8（规范文案与 sources 约束断言同步） |
| npm --prefix desktop run build | tsc + vite build 通过 |

**已知批量失败（不属于 S5 改动面，记录在案）：** verify_hover_hint_101（旧计数）、verify_multiturn_unit、verify_plugin_scope_104（旧路径）、verify_profile_presets_082、verify_project_102、verify_scenario_materials、verify_weekly_099、verify_windows_packaging_134——均为历史脚本与当前产品状态脱节，执行会话未改动这些测试或对应产品代码；34 个窗口自动化脚本需窗口 harness 未在本机跑。这些不阻断 S5，但 S5 收口提交前应明确「哪些要在收口内修、哪些记为已知脱节」。

**收口前置（待用户完成）：** 用户最后一次复测确认「审核项通过并纳入 → 检索命中带引用」闭环；然后按用户批准一次性提交 S1-S5 全部改动。



## OD-KB-201 真实界面端到端验证（用户实测，2026-11-07）——发现 3 个缺陷，S5 前必须修复

**实测结果（用户真实窗口，2026-11-07）：** 步骤 1 成功；步骤 2 失败（缺陷 A）；步骤 3 有缺陷表现（缺陷 B）；步骤 4 成功；步骤 5 可用但路径描述有偏差（缺陷 C，模型行为非代码缺陷）；步骤 6 成功。

- **缺陷 A（高，阻断整理链路）：模型按提示词规范把 sources 写成 `知识库/原文/...`，但真实材料在 inbox/ 未归档时 validate_payload 要求 sources 指向扫描结果中真实存在的文件 → 来源不存在，整批卡片被拒。同时 300 字上限按 frontmatter 之外正文计，模型不易精确控制（本次 326 字被拒）。两项共同导致：真实材料第一次整理必然失败或高概率失败。** 根因：141 时代的建卡规范（engine/omnidesk_engine/knowledge_ingest.py JSON_OUTPUT_SPEC）写死 sources 指向「收件箱或原文目录」，与 S3 引入的「先暂存审核、通过才归档」新流程脱节——审核发生时原文还在 inbox/（或已是 raw/），卡片却声称来源在 原文/。
- **缺陷 B（中，干扰审核）：原始资料准入后索引行链接指向 raw/<文件名>，vault_index 的 resolve_target 按「笔记路径」解析（raw/ 文件不是 LoadedNote，也不在附件白名单口径），索引行被报「链接未解析」进审核冲突队列。raw 索引行的链接目标不是普通 wiki 链接，不应按未解析链接计。**
- **缺陷 C（低，记录在案）：对话检索命中 raw/ 索引行后，模型答文里说「索引指向的知识库/raw/ 目录不存在」——目录确实不存在（raw/ 在库根），模型推断正确但口径易混淆；filterKnowledgeHits 已把 raw/ 排除在引用卡之外，用户看到的「命中一张卡」实为索引行上下文命中。属提示词/检索口径的表述问题，不阻断。

**S5 修复要求（进入收口前必须完成）：**

1. 建卡规范与校验口径统一到新流程：sources 允许指向本次整理批次实际存在的文件（inbox/ 与 raw/），JSON_OUTPUT_SPEC 与 validate_payload 同步改；归档动作改为「审核通过时由宿主把 inbox 原文移 原文/，并回写卡片 sources 或保持相对引用」。
2. 卡片长度上限给模型可操作的指引（如「正文 280 字内」或放宽宿主校验至 350），规范与校验必须同口径，不允许必然失败的组合。
3. raw 索引行不进链接解析：buildKnowledgeConflicts 排除「索引.md 原始资料小节指向 raw/」的未解析项；或 vault_index 对 raw/ 目标标 attachment/ignore。二选一，执行会话写明选择。
4. 修复后重跑本端到端（重点：同一份真实材料整理 → 审核 → 检索全通）。



## OD-KB-201 S4 审查结论（ARCHITECT，2026-11-07）——通过（真实界面端到端验证仍为前置遗留）

**结论：通过（代码与测试层面）。** 执行会话按 PLANS.md「S4 执行要点」实施；本人逐行审查实际 diff（新增 knowledge_quality.rs 253 行与 verify_knowledge_quality_s4.py；KnowledgeDashboard.tsx 三项增强；types/api 接线；task_core.py 补错误字段）并亲自复跑验证。

**审查要点（逐项核对源码）：**

- 三项增强都在驾驶舱健康卡内，无独立质量页（verify 脚本 three_tabs_only / no_quality_page_tab 断言）——与定稿结构一致。
- 过期检查（knowledge_quality.rs）是纯确定性实现：解析索引行与 raw 准入治理记录里的 YYYY-MM-DD，超阈值标「可能过期」；不调模型、不加依赖；有固定时间注入测试、阈值边界测试（恰好 N 天不过期、N+1 天过期）、非法日期忽略测试。阈值配置在应用数据根 config/knowledge-quality.v1.json（缺失用默认 180 天，损坏明确报错不静默放宽，1-3650 天范围校验）——计划只要求「可配置」，执行会话选独立配置文件并在 STATUS 写明理由，接受。
- 检索抽检：复用既有 vault_search 全文检索（卡片明确标注「全文检索 · 非向量」），显示命中数、检查范围、命中区域与原文上下文，可跳资产页；预算耗尽时如实标注「结果不完整」。无向量/嵌入，与计划边界一致。
- 任务记录补全：引擎 TaskRecordSummary 增加 error_code / error_message / intent 字段（task_core.py），驾驶舱任务列表显示失败原因；这是引擎读侧的窄扩展，不改提交/取消协议。
- 数据诚实：过期清单、抽检结果均为可复算的确定性产物；无数据时空态如实。

**本人复跑的验证（实际运行）：**

| 验证 | 结果 |
| --- | --- |
| cargo test --offline 全量（lib + 全部集成） | 619 passed / 0 failed（S3 后 615，S4 新增 4 项 quality 单测全过） |
| python3 tests/verification/desktop/verify_knowledge_quality_s4.py | 21/21 |
| python3 tests/verification/desktop/verify_knowledge_hub_ui_s3.py | 46/46 |
| python3 tests/verification/desktop/verify_knowledge_governance_s2.py | 18/18 |
| python3 tests/verification/desktop/verify_knowledge_ingest_ui_141.py | 22/22 |
| npm --prefix desktop run build | tsc + vite build 通过 |

**限制与后续（不阻断本轮通过）：**

- **真实界面端到端验证仍是前置遗留**（S3 审查时已列为第一优先项，本轮执行会话也在 STATUS 如实标注「待用户确认」）：需用户在真实窗口走「上传 raw - 整理收件箱 - 审核通过 - 资产可见 - 对话检索引用 - 检索抽检」。此项通过后 S1-S4 才算产品级验收，之后做 S5 收口提交。
- 过期阈值暂无界面编辑器（改配置文件生效）；驾驶舱显示当前阈值。
- S2 跨文件事务限制延续；检索抽检依赖全文检索质量，若实测发现命中率不足，向量化议题再议（需单独计划与批准）。



## OD-KB-201 S3 审查结论（ARCHITECT，2026-11-07）——通过（待真实界面验证）

**结论：通过。** 执行会话按 PLANS.md「S3 执行要点」实施；本人逐行审查实际 diff（新增 knowledge_review.rs、KnowledgeDashboard.tsx、KnowledgeReviewCenter.tsx、knowledgeHub.ts、verify_knowledge_hub_ui_s3.py；KnowledgeVaultPage.tsx 重构、lib.rs +119、api.ts/types.ts 接线）并亲自复跑验证。

**审查要点（逐项核对源码）：**

- 三页结构：KnowledgeVaultPage 改为 dashboard / review / assets 三页签（HubPage），与定稿预览一致；图谱从一级入口撤下，只在驾驶舱健康卡动作内打开（graphOpen 状态 + 返回驾驶舱按钮 + 图谱节点跳资产页），不再是独立页签。
- 审核暂存是本轮最重要的架构改进：新增 knowledge_review.rs，模型整理结果先 stage 到仓库外 engine-state/knowledge-reviews/*.json 并记一条 propose 治理记录，未经人工裁决不写知识库；approve 才走既有 apply_payload 落卡 + 更新索引 + 归档原文；驳回/需要修改只落治理记录。前端整理面板已从「apply 直接落盘」改为「stage 等审核」，141 时代的直落链路按计划关闭。
- 记录健壮性：审核记录 ID 白名单（字母数字-_、80 字符内）；损坏记录跳过并计数（broken 字段）不阻断列表；schema 版本校验。
- 冲突裁决：resolve_conflict 落 governance 记录；buildKnowledgeConflicts 从扫描重名/索引问题构建冲突清单，已 resolved 的不再显示；deferred / needs_more 保留待定。
- raw 上传接前端：Tauri 文件选择框给绝对路径，宿主 read_raw_file 校验（绝对路径、普通文件、大小上限）后读字节准入；前端不读文件字节。
- 驾驶舱数据全部真实：五格指标取自 vault scan、governance 记录、taskRecords（engine_task_records 既有命令）；来源可追溯率按「治理准入记录里出现过该 raw 路径」计算，是真实登记率不是估算；无数据时显示「暂无」。
- 资产默认只读：loadNote 后 setMode(preview)，编辑必须显式点「编辑此条」（既有编辑器/冲突检测链路不变）。
- 治理只读边界保持：前端无治理写命令（verify 脚本 no_governance_write_ipc 等断言通过）。

**本人复跑的验证（实际运行）：**

| 验证 | 结果 |
| --- | --- |
| cargo test --offline 全量（lib + 全部集成） | 615 passed / 0 failed（S2 后 610，S3 新增 5 项 Rust 测试全过：暂存/驳回、通过落卡+检索命中、损坏记录跳过、冲突裁决、raw 路径读取） |
| python3 tests/verification/desktop/verify_knowledge_hub_ui_s3.py | 46/46（三页签、审核暂存语义、冲突裁决、治理只读、raw 上传接线） |
| python3 tests/verification/desktop/verify_knowledge_governance_s2.py | 18/18 |
| python3 tests/verification/desktop/verify_knowledge_ingest_ui_141.py | 22/22 |
| npm --prefix desktop run build | tsc + vite build 通过 |

**限制与后续（不阻断本轮通过）：**

- S1-S3 已构成完整闭环（资料准入 raw/ - 整理暂存审核 - 通过落卡 - 检索引用），但真实界面端到端仍未做：需要用户在真实窗口走一遍「上传 raw - 整理收件箱 - 审核通过 - 资产可见 - 对话检索引用」。这是 S4 的第一优先验证项。
- 驾驶舱「AI 最近工作」数据源为任务台账最近记录，台账为空显示空态——与数据诚实约束一致。
- 治理记录写入失败不回滚 raw/ 与索引的 S2 限制延续；S3 decide 链路同样如实回报 governance_errors。



## OD-KB-201 S2 审查结论（ARCHITECT，2026-11-07）——通过（待真实界面验证）

**结论：通过。** 执行会话按 PLANS.md「S2 执行要点」实施；本人逐行审查实际 diff（新增 knowledge_governance.rs 318 行；knowledge_ingest.rs、knowledge_vault.rs、lib.rs 的 S2 增量）并亲自复跑验证。

**审查要点（逐项核对源码）：**

- 位置与形态：治理目录为 vault 根下 governance/，与 raw/ 平级，不是仓库 knowledge/ 目录——与计划一致；三类文件（raw_admissions / review-decisions / admission-rules）均为 Markdown 单行元数据，未引入数据库。
- AI 无写权（代码级）：写入口只有宿主函数 append_record / append_record_at；IPC 只暴露 knowledge_governance_list（只读）；静态断言验证前端 api.ts 与 engine/ 全部 Python 源码中不存在治理写命令名，engine 源码不含 governance/ 路径。
- 脱敏钩子：三个文本字段写入前都过 mask_privacy（与工作记录同一 RulePolicy）；有专门测试断言手机号以打码形态落盘、明文不出现在文件里；字段清洗把竖线转全角、控制字符转空格，保住单行格式。
- 原子性：追加走 vault_write::save_note（冲突检测 + 临时文件 + rename），新建走 create_note（硬链接发布），失败不留半截行。
- 扫描排除：scan_vault 只在 vault 根层跳过 governance/（不做全树同名排除），有测试断言治理文件不进扫描结果、skip 明细带 governance 原因——执行会话选了「排除目录」方案并写明理由，符合计划二选一要求。
- S1 挂钩：admit_raw_files_with_policy 每次准入成功后记一条准入记录（action=admit，subject=raw/实际名，sha256 前 12 位，note 含原名与字节数）；换名时治理记录保留原名（AdmittedRawFile 增加 original_name）；IPC knowledge_raw_admit 现在经 current_rule_policy 装载真实 Profile 隐私策略——S1 遗留的策略缺口顺带补上。
- 诚实边界：治理记录写入失败在 RawAdmitReport.errors 如实回报，已落盘 raw 与索引不回滚（跨文件事务明确不在 S2 范围）——这是记录在案的限制，不是缺陷。

**本人复跑的验证（实际运行）：**

| 验证 | 结果 |
| --- | --- |
| cargo test --offline --lib | 543/543（S1 后 536，S2 新增 7 项治理测试全过） |
| cargo test --offline 全量（lib + 全部集成） | 610 passed / 0 failed |
| python3 tests/verification/desktop/verify_knowledge_governance_s2.py | 18/18（含 AI 无写权、engine 无治理路径、扫描排除断言） |
| python3 tests/verification/desktop/verify_knowledge_ingest_ui_141.py | 22/22 |
| npm --prefix desktop run build | 通过 |

**限制与后续（不阻断本轮通过）：**

- 治理记录写入失败时 raw/ 与索引不回滚：单条准入可能出现「raw 在、治理记录缺」的组合；S3 审核中心接入后若实测影响裁决追溯，再评估跨文件事务。
- 治理文件的并发追加窗口（读-改-写之间）沿用 vault_write 既有 race window 说明，S2 未扩大。



## OD-KB-201 S1 审查结论（ARCHITECT，2026-11-07）——通过（待真实界面验证）

**结论：通过。** 执行会话按 PLANS.md「S1 执行要点」实施；本人逐行审查实际 diff（knowledge_ingest.rs +594、lib.rs +18）并亲自复跑验证。

**审查要点（逐项核对源码）：**

- 目录骨架：RAW_DIR 加入 init_vault 六目录幂等骨架；索引模板追加「## 原始资料」小节；重复 init 不重写既有索引。
- 准入安全：validate_raw_name 拒绝路径片段（../、dir/）、隐藏名、控制字符与破坏 Markdown 链接的字符；raw 目标为符号链接或非常规文件时拒绝；写入走「临时文件 + fsync + 硬链接发布」，目标已存在原子失败，绝不覆盖。
- 去重：索引「原始资料」小节既有哈希 + 批内累积哈希双重比对（批内同内容第二个文件也被拒）；同时核对 raw/ 磁盘上同名文件的实际内容哈希，磁盘已有同内容文件即使索引无记录也拒绝——预研时记录的「只读索引」去重盲区已被执行会话补上。
- 换名：同名不同内容以「原名-哈希前12位」换名新增，原文件字节级不动（有专门断言读到旧内容）。
- 原子性与回滚：索引保存失败时删除本轮已发布 raw 文件；有强制回滚测试（索引撑超单文件上限后准入，断言 raw 目录为空、索引未变）。
- 索引行：准入时间（RFC3339，含 +08:00 断言）、sha256、字节数；链接目标用尖括号包裹，文件名含空格仍是合法 Markdown 链接。
- IPC：knowledge_raw_admit 只做「读配置 - 校验根 - init_vault - admit」，无额外写通道；已注册 invoke_handler。

**本人复跑的验证（实际运行，非转述执行会话输出）：**

| 验证 | 结果 |
| --- | --- |
| cargo test --offline --lib（复跑 4 次，其中一次单线程全量） | 536/536（首次出现 1 failed，复跑未再现；判定为环境偶发，非本轮代码缺陷） |
| cargo test --offline 全量（lib + 全部集成） | 603 passed / 0 failed |
| python3 tests/verification/desktop/verify_knowledge_ingest_ui_141.py | 22/22（含新增 3 项 raw 断言） |
| npm --prefix desktop run build | tsc + vite build 通过 |

**限制与后续（不阻断本轮通过）：**

- 前端尚未接 knowledge_raw_admit（S1 范围本就不含前端入口）；真实界面上传 raw 附件的手动验证归 S3。
- 工作区含 146 既有前端改动与本轮 S1 改动，均未提交，验收后由用户决定提交时机。

## OD-KB-201 规划登记（ARCHITECT，2026-11-07）——规划完成，未实施、未验收

**本条是规划登记，不是验收结论。** OD-KB-201 为知识库重构（LLM-Wiki 定位）规划：先读仓库现状（PLANS/GOALS/decisions、knowledge/README、knowledge_ingest.rs、knowledge_vault.rs、vault_index.rs、vault_search.rs、KnowledgeVaultPage.tsx、App.tsx、REVIEW.md 的 141–146 条目），再写入 PLANS.md 的 OD-KB-201 条目。事实依据：OD-ARCH-141/144 已建立「宿主校验落盘 + 脱敏 + 两跳检索引用」链路，OD-ARCH-146 S3 已完成整理入口迁移，与规划中「复用现有链路、只补 Raw/Governance 目录与审核界面」的判断一致。引用产品截图库（screenshots-design-inspiration）的 Shopify 管理后台作为审核队列布局参考（借鉴要点：左队列右详情、批量操作与状态徽标）。本条仅记录规划依据与证据边界；实施后按各阶段写入独立审查结论。

## OD-ARCH-146 审查结论（ARCHITECT，2026-11-06）——通过（待真实界面验证）

**结论：通过。** S1（对话后台存活）、S2（输入框 4 行自适应）、S3（整理入口迁移）与批准后的计划一致；S4 知识库页面重构按用户决定未做，仅保留入口迁移。

**复核范围：** HEAD `177a8292ea` + 工作区改动（PLANS.md、STATUS.md、sceneThreads.ts、HomeCanvas.tsx、HomeChat.tsx、KnowledgeVaultPage.tsx、KnowledgeIngestPanel.tsx、两个验证脚本）。逐行检查了任务号写入与清空的时机、卸载期间 promise 继续回调的路径、重挂后的恢复顺序（先载入消息再补占位再轮询）、textarea 高度上限与键位逻辑、整理面板落点与扫描刷新。`runtime/` 零差异，零提交，零推送。

**关键判断：**

1. **恢复顺序正确。** threadKey effect 先载入档案消息，恢复 effect 再补缺失的助手占位；轮询 effect 声明在后，首轮 tick 就能对上 taskId。终态回读走 task_result，不依赖卸掉的事件监听。
2. **任务不会被卸载取消。** 卸载只停止本组件的轮询；引擎侧任务照常执行。切页发生在拿到任务号之前时，提交 promise 的回调仍把任务号写入档案，HomeCanvas 通过档案订阅同步到新挂载的组件。
3. **清理路径完整。** 终态 settle 与最大轮询失败都会清空在途任务号；不会出现「任务已结束但档案仍显示等待」的滞留。
4. **输入框键位未变。** Enter 发送、Shift+Enter 换行保留；高度在约 96px（约 4 行）封顶后内部滚动。
5. **整理入口归属修正。** 对话工具行不再渲染面板；知识库顶部操作区提供入口，整理成功后刷新扫描结果。

**本人实跑结果：**

| 验证 | 结果 |
| --- | --- |
| `npm --prefix desktop run build` | tsc --noEmit + vite build 通过 |
| `verify_chat_recovery_146.py` | 16/16 通过 |
| `verify_knowledge_ingest_ui_141.py` | 19/19 通过 |
| `verify_starting_145.py` | 12/12 通过 |
| `verify_attachments_077.py` / `verify_conversation_prompt.py` | 25/25、26/26 通过 |
| `cargo test --offline` | 全量通过，0 failed |

**风险与未验证：**

- **[P2] 真实界面复测未跑。** 需要用户在打包版本里实测三条路径：提交后切页再切回、粘贴长文本、知识库内整理收件箱。静态接线不能替代真实交互。
- **[P3] 应用重启后的残留任务号。** 档案里若有上次进程的任务号，宿主查不到；轮询连续失败 5 次后停下并提示「任务状态未知」，不会误报成功，也不会自动重发。
- **[P3] 返回首页仍打开线程列表第一条。** 这是既有行为；若在途任务属于较旧线程，需要用户打开该线程才看到恢复后的结果。
- **[P3] 知识库脚本 p3/p4/059 在本机跑不了。** 脚本按字面 `/tmp` 校验，macOS 上 `/tmp` 解析为 `/private/tmp`，属既有环境限制，与本轮改动无关。

## OD-ARCH-145 审查结论（ARCHITECT，2026-11-06）——通过（附 2 条基线风险记录）

**结论：通过。** A（stderr 落盘）、B（starting 兜底提示）、C（starting 取消可恢复）三项实现与计划一致，实现代码逐行核对无误，回归全绿。真实复现验证仍待用户实测，与本计划"待真实复现验证"的口径一致。

**独立复核范围：** HEAD `46cc88476e` + 工作区改动（13 个修改文件 + 新增 verify_starting_145.py）。逐行核对了 engine_ipc.rs 的滚动日志实现、engine_supervisor.rs 的接线、HomeChat.tsx 的兜底与取消清会话、web_api_adapter.py 的 starting 取消落定、sceneThreads/types 的可空会话号。全程 `runtime/` 零差异、零提交、零推送。

**独立实跑结果（本人执行，非采信 STATUS.md）：**

| 验证 | 结果 |
| --- | --- |
| `python3 tests/verification/desktop/verify_starting_145.py` | 12/12 通过 |
| `python3 tests/verification/desktop/verify_knowledge_ingest_ui_141.py` | 18/18 通过 |
| `python3 -m unittest test_web_api_unit`（adapter 目录） | 89/89（新增 StartingCancelTests 1 项） |
| 三个纯单元测试文件（masker 32 / markdown 38 / ingest 8） | 32/32、38/38、8/8 |
| `cargo test --offline`（沙箱外，需要 /bin/ps） | **529 lib + 13 host_integration + 其余集成目标全 ok / 0 failed** |
| `npm --prefix desktop run build` | tsc --noEmit + vite build 通过 |
| `run_approval_scenarios.py` A01–A05 | 5/5 pass |
| `run_web_api_scenarios.py` W01–W12 | 12/12 pass |
| `run_engine_adapter_scenarios.py` E01–E20 | 18/20 pass；E17、E20 见风险记录 |
| `run_visibility_scenarios.py` V01–V05 | 2/5 pass；V02/V03/V05 见风险记录 |

**审查判断：**

1. **C 的状态转换是合法的。** cancel_task 先置 CANCELLING 再落 FAILED；契约 3.2 的 LEGAL_TRANSITIONS 只约束 _settle 的 from-state，CANCELLING→FAILED 在表内。error_code 用 CANCEL_FAILED 语义贴切（"未等引擎确认、本地终态"）。单元测试证明 starting 取消不再发 session/cancel。
2. **B 无自动杀任务、提示精确落在计划文案。** slowStarting（提交中 120s）与 startingTooLong（activeTask starting 且 elapsed>=120）两条路径都只加红字提示；没有改宿主取消模型。
3. **A 的滚动日志实现在 stderr 读线程内逐行追加，1 MiB 阈值 + .1–.3 三份滚动。** 单测覆盖超限滚动与真实子进程落盘；日志目录随 supervisor 启动创建。
4. **取消后的会话重置闭环核对：** onEngineSessionId(null) → HomeCanvas setThreadEngineSession(id, null) → sceneThreads 接受 null → 下次 submitText 传 engineSessionId=null → 引擎新建会话。receipt.engine_session_id 为空时不清旧值，避免误清除，逻辑自洽。
5. **144 顺带复核通过：** PrepareResult.user_input_text 与前端透传、types.ts 同步、两条新单测与 18 项 UI 静态断言全绿（含 ingest_scan_uses_inbox_text）。

**基线风险记录（非本轮改动引入，E17 于 10-23 之前已如此）：**

- **[P3] E17/E20 在 DSH 0.2.1-rc.5（11-02 升级）后回归失败，升级当日未发现。** E17 期望 sdk 通道对 session/resume 返回 "unknown method"；rc.5 实际先走 "SDK server is not initialized"（initialize 缺 provider/model 时先行失败）。测试断言过窄（对错误字符串精确匹配），可放宽为"返回 -32603 错误"即可恢复全绿。E20 只是复跑汇总的连带失败。10-23 的 E17 pass 证据对应 alpha.2 行为；11-02 底座升级时未重跑该场景。
- **[P3] V02/V03/V05 同受 11-02 底座升级影响。** 显式 total_seconds 截止不再生效（超时落 interrupted 而非 unknown+带秒数文案）、未知询问帧无法阻止任务被 engine 关闭 interrupted。三项均断言旧行为；与 145 的 starting 兜底哲学不冲突，但建议单开一轮超时口径审查。基线（46cc88476e 归档目录）复跑同样失败，确认非本轮回归。

**遗留：** ① 真实第 4 轮卡死复现后取 `~/OmniDesk/app/logs/engine-stderr.log` 定位 DSH 侧根因（需要包含本轮改动的构建）；② 120s 兜底提示的真实等待未跑满（静态接线 + 计时逻辑核对）；③ E17/V02/V03/V05 建议另立测试修复轮，不阻塞 145 验收。

**Git 状态：** 工作区改动未提交未推送；本轮验证新增了 evidence JSON 的时间戳更新（30+ 个 evidence 文件 diff），属重跑脚本的预期写入；新增 `用户成果/` 目录（用户实测产物，不入库）。

## OD-ARCH-143 审查结论（ARCHITECT，2026-11-05）——S1/S2 通过，S3–S5 待用户逐项执行

**结论：S1（坏测试修复）与 S2（合成端到端）复核通过。项目首次达到全部测试零失败。S3 用户实测、S4 收口提交、S5 发布确认待逐项执行。**

**独立复核范围：** 独立复跑——host_integration 13/13（此前 3 项失败全修复）、web_api 88/88（此前 2 项失败全修复）、E2E 端到端 10/10、cargo 全量 14 个测试组全部 ok 且 0 FAILED；S4 只读预检复核：新增文件凭证模式扫描零命中（唯一 grep 命中是测试夹具的临时目录命名，非凭证），词典样例 5 条全虚构确认。

**复核判断：**

1. **S1 修的是测试自身，没动生产断言。** host_integration 的 /bin/true 改为 tempfile 可执行假 CLI（跨平台）；web_api 的 socket 超时预算参数化、参数序断言改为对 resolve_node 实际结果的比较。还额外发现并修了一个计划外问题（knowledge_vault_index_host 的 symlink 路径规范化），处理正确。
2. **S2 覆盖了承诺的全部链路。** 知识库初始化→整理→建卡/索引/归档→检索命中→词典人名出站占位符→假模型回文还原→出站日志无原文→随手记/周报/四象限/成果库回归，10 项断言全绿，证据落 evidence/od-arch-143/。
3. **提交预检干净。** 凭证模式（sk-/ghp_/AKIA/私钥头/api_key 赋值）零命中；无真实工作记录关键词。
4. **实施方纪律良好。** S3/S4/S5 需要用户实测与明确授权的部分全部停下未动，没有擅自 git commit/push/tag。

**S3 用户实测清单（四项，逐项确认）：**
- [ ] 真实模型对话：词典人名出站占位符，回答自动还原
- [ ] 真实 Vault：丢材料→整理→检索命中→打开引用卡
- [ ] 随手记与周报真实工作流各走一遍
- [ ] 应用重启后旧会话占位符原样显示+一次提示

**S4（用户批准后执行）：** 分批提交（安全引擎/知识库/强制脱敏/测试修复/文档）→ push origin/main → 0.2.9 tag → CI 三产物。S5：发布确认与实机安装冒烟。
## OD-ARCH-142 审查结论（ARCHITECT，2026-11-05）——通过

**结论：通过。** Task 4 完成 LLM 调用链全量强制脱敏：出站在引擎 submit_task 唯一汇合点强制过 DataMasker（附件/知识上下文/重放历史都已拼入该点之后），回程流式与非流式双路自动还原，映射按引擎会话内存存活。139 规划承诺的"Task 4 强制点"落地，数据安全拼图补完。

**独立复核范围：** 独立复跑全部专项——Masking 4/4、StreamUnmask 5/5、审计 15/15（0 违例）、Task 1/2/141 回归 78/78、Rust lib 524/524、npm build 通过、UI 静态检查 7/7；实读引擎接入主路径（_mask_prompt/_mask_session_for/_unmask_delta）、StreamUnmasker 状态机、宿主词典路径环境变量下发；手工烟测：占位符跨 delta 切断还原（3 实体 3 还原、往返一致、ok=True）。

**复核判断：**

1. **强制点真实。** _mask_prompt 调用在 submit_task 唯一文本汇合点（web_api_adapter.py:1400），汇合点之后只有脱敏文本进 _turn_worker；审计脚本 15/15 静态断言 0 违例。脱敏失败**拒绝提交**（fail-closed，不降级明文出站）——比 141 检索注入的 fail-open 更严，语义正确（前者是保护，后者是增强）。
2. **会话映射生命周期符合设计。** _mask_sessions 按引擎会话隔离（unittest 验证）；词典会话建立时读取（存续会话不换表）；OMNIDESK_MASKER=off 显式关闭且 warn 记录（UI 无开关，审计断言 no_ui_masker_switch 通过）。
3. **流式还原状态机正确。** 手工烟测：三个占位符的文本从中间切成两半喂入，最终逐字还原为原文，restored=3。跨 delta 切断的场景（模型输出占位符时恰好断在 [PERSON_07 中间）是流式还原最容易错的点，有专门测试。
4. **回执/展示完整。** mask_stats 进任务回执（计数不含原文）、unmask_report 挂终态事件、前端 ok=False 顶部提示、本地会话记录不被改写（原文留在本地）。
5. **失败语义分层正确。** 出站脱敏失败=拒提交（安全）；回程 unknown 占位符=原样保留+提示（可用性）；词典缺失=降级纯规则+warn（不阻断）。
6. **既有失败 5 项确认与 142 无关**（host_integration 3 项 /bin/true、web_api 2 项 socket/参数序）——142 新增测试全绿且未触碰既有测试逻辑。

**本轮未覆盖（用户实测项）：** 真实模型端到端——出站占位符→回答自动还原→本地原文不变，需要你配置的模型通道跑一次；会话恢复（重启应用续旧会话）时占位符原样保留+一次性提示的行为。

**Task 4 收口。** 至此安全链路完整：红线拒绝 → 打码 → 双向脱敏 → 回程还原，全链强制。剩余 Task 5（端到端验收）收尾后统一提交。
## OD-ARCH-141 审查结论（ARCHITECT，2026-11-05）——通过（附两项既有失败说明）

**结论：通过。** Task 3 知识库工作流接线落地：收件箱目录骨架、整理提示词组装（出站过 Task 2 脱敏）、整理结果校验（路径白名单/标题防重/卡长/sources 存在性）、逐卡落盘+索引追加+原文归档（失败回滚当卡）、对话知识意图检索注入（top3 命中、≤2 卡、≤3000 字、可点击引用）、整理面板 UI。全链路"AI 判语义、工作台落盘"哲学与 memory-agent 一致。

**独立复核范围：** 独立复跑全部验收命令——cargo lib 523/523（含 knowledge_ingest 12 项）、引擎单测 8/8、前端 retrieval 8/8、UI 验证脚本 16/16、npm build 通过；实读 knowledge_ingest.rs 校验器与落盘主路径、引擎 assemble_ingest_prompt（mask_markdown 注入确认）、HomeChat 检索挂钩与 knowledgeRetrieval 上限常量；git status 改动面核对（新增 5 个功能文件 + 3 个测试文件，修改 5 个接线文件，无越界）。

**复核判断：**

1. **校验器是本链路的安全闸，实现到位。** 路径只许 概念/方法/ 两目录（clean_rel + is_card_path 双层）、标题防重同时查索引与现存卡（title_key 归一化）、正文 300 字上限按 parse_note 后正文计（frontmatter 不占额）、sources 逐条验证存在。模型输出再狂野也写不进白名单外。
2. **落盘事务性合格。** 卡创建成功但索引更新失败时：回滚已建卡文件、原文不归档、错误入报告——不会出现"卡在库里但索引没有"的孤儿状态。
3. **对话检索修正了两处真问题（实施方自查发现，值得肯定）：** vault_search 是字面子串匹配，整句问话命中不了卡片——改为抽最长知识意图词再查；knowledge_vault_open_attachment 拒绝 .md，原实现点击引用必报错——改为读笔记+方角预览卡。两处都有测试覆盖。
4. **注入边界清楚。** 只注入 知识库/概念|方法/ 下的卡，inbox 原文/索引页/用户笔记不进模型上下文；上限 3000 字截断且显式标注；失败不阻断提交（fail-open 合理——知识库增强而非门禁）。
5. **出站脱敏接入。** assemble_ingest_prompt 对 inbox 原文跑 mask_markdown，session scope 跨文件同实体同占位符，映射不序列化（测试断言）。
6. **边界遵守。** 无定时整理（092 裁定）、无向量库（140 触发条件）、基础标准不提供、模型无直接写权。

**两项既有失败（均与 141 无关，独立复核确认）：**

1. host_integration 3 项（supervisor_refuses_stock_dsh_home / refuses_stock_agents_home / engine_dir_missing）：测试硬编码 /bin/true 作为假 CLI——Linux 有、macOS 只有 /usr/bin/true，config.validate() 先报"CLI 不存在"就到不了真正要测的断言。属测试的可移植性缺陷（048 引入），非产品缺陷；建议后续小任务改用 tempfile 写一个可执行假 CLI。
2. test_web_api_unit 2 项（139 时已记录）：recv_budget 心跳帧 + for_dsh 参数序，同属既有测试口径问题。

**建议**：上述 5 个坏测试打包一个独立小任务（"测试可移植性修复"）处理，不混入功能任务。

**本轮未覆盖（用户实测项）：** 真实模型整理质量（建卡是否准确、来源是否可靠）、真实对话检索的意图判定命中率、@知识库 显式触发。需要用户在真实环境跑一遍完整流程。

**Task 3 收口。** 知识库现在活了：inbox 丢材料 → 整理建卡 → 对话检索引用。剩余：Task 4（LLM 链路全量强制脱敏）→ Task 5（端到端）。
## OD-ARCH-140 审查结论（ARCHITECT，2026-11-05）——通过

**结论：通过。** Task 2 结构感知脱敏落地：mask_markdown / unmask_markdown 对 Obsidian 兼容 Markdown 实现"只改该改的位置，结构逐字节保真"。独立复跑 Task 2 全量 38/38、Task 1 回归 32/32，均 OK；另做 4 项手工烟测全部符合区域策略表。

**独立复核范围：** git status 改动面核对（仅 security/ 与测试文件，Rust 零改动）；复跑两组验收命令；烟测覆盖：frontmatter 脱敏且 sources 保留、正文裸敏感词替换、WikiLink 目标保留+显示文本处理、代码块内证件号不替换、标签/URL 保留、往返逐字节一致、frontmatter 与正文同名实体同占位符。

**复核判断：**

1. 区域策略表八类行为与设计一致（烟测实证，非仅断言）。
2. MD 标准链接 [文本](url) 的链接文本被替换后，占位符自身方括号与链接语法叠加成 [[占位符]](url)——语法仍合法、往返还原逐字节一致、链接目标 URL 未动，接受；此形态已记入本结论，Task 4 集成时若对渲染有要求可再评估占位符定界符。
3. 未闭合 frontmatter/代码块按 warn 降级，未崩未误替换（测试组 -k Edge 覆盖）。
4. Task 1 纯文本路径行为零变化（32 项原样通过），符合"不得修改已通过行为"红线。
5. 范围纪律：无第三方依赖、无 Rust 改动、测试零 print、全虚构数据。

**本轮未覆盖：** 真实知识卡批量脱敏演练属 Task 5 端到端；vault_write 对齐确认（S5 文档步骤）已由实施方在 STATUS 记录，我核对其结论与 vault_write.rs 白名单事实一致。

**DataMasker 主体（Task 1+2）就此收口。** 下一阶段按路线图：Task 3（知识库工作流接线，规划中）→ Task 4（LLM 链路强制脱敏）→ Task 5（端到端）。
## OD-ARCH-139 审查结论（ARCHITECT，2026-11-05）——通过（附一项既有失败说明）

**结论：通过。** DeepSeek 在新会话按任务卡 S1–S7 完成 DataMasker Task 1：词典加载、四类规则识别（GB11643/Luhn 真校验）、随机类型化占位符、请求级/会话级映射、双向脱敏还原与完整性报告全部落地。实施范围与任务卡完全一致，未越界。

**独立复核范围：** 实读 masker.py 全部 493 行（匹配排序、占位符生成、MaskMapping 封装、unmask/validate）、__init__.py 导出面、样例词典、390 行测试脚本、STATUS.md 记录；git status 核对改动面；独立复跑全部验收命令；另做 6 项手工烟测（往返一致、跨映射隔离、会话一致性、别名合并、校验码拒绝、词典+规则叠加）。

**复核判断：**

1. **范围合规。** git status：新增 security/ 两文件 + 样例词典 + 测试脚本，修改 PLANS.md/STATUS.md；engine 现有文件零 diff（git diff --stat HEAD -- engine/ 为空）。无第三方 import（stdlib only：re/secrets/dataclasses/typing/json/pathlib）。
2. **验收命令独立复跑全过。** test_security_masker_unit 32/32 OK；test_process_owner_unit 10/10 OK；test_web_api_unit 79 项 1 failure + 1 error——在 git archive HEAD 干净副本上同样复现，确认与 DataMasker 无关（实施方判断正确，未为过测试去改范围外文件，符合纪律）。
3. **映射安全构造成立。** DataMasker 实例零映射留存（__init__ 只存词典）；MaskMapping frozen、无 to_dict/原文导出、repr 为 "<MaskMapping n=N>"；SessionScope repr 同样脱敏。跨请求还原烟测：用另一请求的 mapping → ok=False、占位符原样保留，未猜测还原。
4. **识别与重叠正确。** 手工烟测：词典词"林青禾"（含别名"林青禾同志"）与 value 同占位符；"城东支行营业部" alias 命中；校验码错误的 18 位串不命中（invalid_idcard_kept 验证通过）；词典与规则同段叠加（PHONE + BRANCH 各自替换）；(start,-length,priority) 排序 + overlap_dropped 计数在源码确认。
5. **往返完整性。** mask → unmask 原文逐字还原；report.ok=True/False 语义与设计一致；modified_candidates 计数形似占位符但查表失败的条目。
6. **一处与任务书接口建议的合理偏差（接受）**：mask 返回 MaskResult dataclass（text/mapping/stats 三属性）而非三元组——更符合项目 dataclass 风格，字段齐全，不构成问题。任务书签名里的 extra_dict 参数未实现，当前以 entries 构造参数支持等价能力；Task 4 集成时如需请求级临时词典再补，不阻塞本轮。
7. **红线核查。** 测试零 print（断言验证而非打印）；样例词典五条全虚构（林青禾/周岚/城东支行/青禾联社/晨星项目）；无真实证件号、卡号、干部姓名。

**本轮未覆盖（不作为通过条件）：** Task 2 结构适配（Markdown/WikiLink）、Task 4 LLM 链路接入未做——Task 1 完成不等于云端调用已受保护，强制点在 Task 4，此前已在路线图明示。

**既有失败移交：** test_web_api_unit 的 test_recv_budget_survives_heartbeat_frames（error）与 test_for_dsh_argument_order（fail）是 HEAD 上既有的两个坏测试，与本任务无关；建议单开小任务修复（属于 engine-interface 测试口径问题），不混入 DataMasker 后续阶段。

**Test 1（Task 1）收口。** 后续按路线图推进：Task 2（结构适配）→ Task 3（加固）→ Task 4（LLM 接入，关键强制点）→ Task 5（端到端）。OD-ARCH-138（四象限）仍挂起待用户恢复。

## OD-ARCH-137 审查结论（ARCHITECT，2026-11-04）——通过

**结论：通过。** 0.2.8 的报错不是 DSH 配置损坏，而是启动顺序问题：新机器还没有 `runtime/dsh-home`，`build_state()` 只装配资产、不播种；模型设置又绕开引擎启动直接调 `dsh --dump-config`，所以必报 `profile "omnidesk-base" does not exist`。本轮把播种提前到启动入口，并在三条模型路径上补了同一道保险。

**独立复核范围：** HEAD `9414b34cce` + 工作区改动。核对 `build_state()` 的调用顺序、`seed_dsh_home_on_startup()` 的日志与失败路径、`model_settings_save` / `task_list_model_routes` / `task_set_model_route` 的 seed 调用位置、`seed_home_from_package()` 的合并语义，以及三个新增测试与 `verify_startup_seed_137.py` 的 12 项断言。

**审查判断：**

1. **根因修在启动顺序上。** `seed_dsh_home_on_startup()` 在资产装配之前执行；标准 Profile 的装配仍然是空操作，但 `dsh-home` 已经由包内 seed 建好。模型设置不再依赖「用户先发一条消息把引擎拉起来」。
2. **模型路径不是只修一处。** 保存、列表、选择三条入口都在调用 DSH 前补齐 Profile；即使用户绕过启动入口直接触发设置，也不会再报同一个错。
3. **已有数据根不被覆盖。** `seed_home_from_package()` 现在逐个检查 `profiles/` 与 `skills/`，只在目标不存在时复制；用户改过的清单、补丁与 Skill 原样保留。这解决了 0.2.7 老数据根只有 `web`、升级后仍缺 `omnidesk-base` 的情况。
4. **失败可诊断。** 播种结果写进 `profile-assets.log`（`created` / `ready` / `error`）；失败不阻断窗口，但模型设置仍会如实报错，且日志能回答「为什么没初始化」。
5. **验证不是只读源码。** 新回归测试直接调用生产入口 `build_state()`；本地用 0.2.8 包内 DSH 复现了空 home 退出 1、seed home 退出 0。

**验证证据：** `verify_startup_seed_137.py` **12/12**、`cargo test --offline --lib` **509 passed / 0 failed**、新启动回归测试通过、包内 DSH 空 home **退出 1** / seed home **退出 0**（1,242 行输出）；证据 `tests/verification/desktop/evidence/od-arch-137/facts.json`。

## OD-ARCH-136 审查结论（ARCHITECT，2026-11-04）——通过

**结论：通过。** 用户看到的 `/Users/runner/...` 是 `repo_root()` 把编译期 `CARGO_MANIFEST_DIR` 当成安装根返回的结果；根子在 macOS 包不带运行时、只能靠同目录文件，而用户一移动 `.app` 就落空。本轮让 `.app` 自带运行时，并把兜底路径收紧为「必须真的含 `engine/` 与 `runtime/RUNTIME.lock`」。

**独立复核范围：** HEAD `b546c86f9b` + 工作区改动。核对 `tauri.macos.conf.json` 的四个映射、`find_bundle_root` 的两条布局与上溯层数、`repo_root()` 兜底判定、`build-portable-package.sh` 的 macOS 去重分支、workflow 的准备顺序、体检页文案，以及本机构建产物的目录结构。

**审查判断：**

1. **根因修在返回值的可信度上。** 以前只要前两级找不到就无条件返回构建机路径，界面于是显示一个用户机器上不存在的目录；现在这个路径必须通过 `looks_like_bundle_root`（引擎包 + 版本锁同时存在），否则返回 None，让体检页报「不完整」。构建机路径不会再出现在用户界面上。
2. **两条布局都覆盖，且可测。** `find_bundle_root` 同时认「与可执行文件同级」和「`Contents/Resources`」；三条单测分别覆盖免安装布局、`.app` 布局、空壳目录。`.app` 这条在 0.2.7 之前从未被识别过，是本轮新增的路径。
3. **自带运行时让 `.app` 可移动。** 资源进 `Contents/Resources` 后，用户把应用拖进 `/Applications` 或系统做 App Translocation 都不影响启动；这同时消除了「必须保留解压目录」的隐性要求。
4. **打包去重避免了体积翻倍，并挡住旧空壳。** macOS 分支不再复制同目录运行时；本机阶段目录只有 `OmniDesk.app` 与启动说明。若两处都放，zip 会变成约 300MB。打包脚本现在只接受 `Contents/Resources` 下 `engine`、`RUNTIME.lock`、`dsh-runtime`、`dsh-home-seed` 四项齐全的 `.app`——本机 `target` 里 11 月 2 日留下的 universal 空壳（只有 `icon.icns`）会被跳过，选中当天的自带运行时构建；误用空壳会在用户机器上直接起不来。
5. **文案与语义对齐。** 运行时目录没有「版本」概念，缺失时显示「不完整 / 未找到」；`assets` 行不再重复「需要 运行时目录」。
6. **边界如实。** 未签名应用的 Gatekeeper 拦截与包内资源无关，仍需右键打开或去隔离属性；`.app` 体积约 480MB，这是自带运行时的代价。

**验证证据：** `verify_macos_bundle_136.py` **22/22**、`cargo test --offline --lib` **508 passed / 0 failed**、`npm --prefix desktop run build` 通过、本机 `.app` 476.65 MiB 且 `Contents/Resources` 四项齐全；证据 `tests/verification/desktop/evidence/od-arch-136/facts.json`。用户实测（2026-11-04）：自带运行时的 macOS 包在本机验证通过，不再出现 `/Users/runner/...` 安装不完整提示。0.2.8 已由 Release 工作流 **41848528087** 发布：macOS zip **177,324,235 bytes / 33,126 项**，`Contents/Resources` 下 `engine`、`RUNTIME.lock`、`dsh-runtime`、`dsh-home-seed` 齐全且无同目录副本；Windows 安装器 **94,813,574 bytes**。

## OD-ARCH-135 审查结论（ARCHITECT，2026-11-04）——通过

**结论：通过。** 安装器缺运行时资产是 134 的直接缺陷：资源清单里一个目录都没写，NSIS 自然只装 exe。本轮用 Windows 专属清单补齐 `engine/`、`runtime/`（含底座与首启 seed），并把准备逻辑抽成一份脚本；同轮修掉两个在 Windows 上必然失败的判定（启动失败无声、凭据服务存活检查恒假）。

**独立复核范围：** HEAD `daed74eb55` + 工作区改动。核对 `tauri.windows.conf.json` 的源路径与目标路径、`prepare-runtime-assets.sh` 的三段操作、`release.yml` 的准备与构建顺序、`build-portable-package.sh` 是否只调用不重复实现、`lib.rs` 的 `report_fatal_startup`、`dsh_settings.rs` 的 `reusable`、`platform.rs` 的 `assets` 项与前端渲染。`runtime/` 仅生成被忽略的 seed。

**审查判断（几处特意验过的边界）：**

1. **资源清单与启动查找同构。** 清单把目录放到安装根，`repo_root_from_exe` 从 exe 所在目录上探 `engine/omnidesk_engine` + `runtime/RUNTIME.lock` 正好命中；本机用同一份清单构建 `.app`，`Resources/` 下确实出现了 `engine`、`runtime/dsh-runtime`、`runtime/dsh-home-seed`，体积从 6.68 MiB 变成 476.63 MiB。这是「清单真的生效」的直接证据，而不是读配置推断。
2. **只挂 Windows 是对的。** macOS 的 `.app` 由免安装目录另带一套运行时；若主配置也挂资源，macOS 包会重复一份，体积翻倍。
3. **准备脚本是唯一实现。** 免安装包不再自己裁剪、自己生成 seed，只调用这份脚本；CI 在 `tauri build` 之前调用。顺序错了资源清单会指向不存在的目录，构建直接失败——验证脚本把「准备早于构建」写成断言。
4. **删除 `process_alive` 是收窄误判面，不是放宽校验。** 端口可达说明服务在跑，随后 `exchange_token` 会用令牌做真正鉴权；保留一个在 Windows 上恒假的进程检查只会把可用实例判死。全仓只有 `reusable()` 调用它，删除后没有悬空引用。
5. **启动弹窗的文本走环境变量。** 错误消息含中文、路径与引号，拼进 `-Command` 必然踩转义；用 `OMNIDESK_FATAL_TEXT` 传值，脚本本身是常量。弹窗失败也只 `let _ =`，不影响退出码。
6. **体检第三项让「装坏了」变成可读状态。** `assets` 参与 `report.ok`，缺 `engine/` 或 `runtime/` 时页面直接给重装指引，用户不再面对一个没有解释的窗口。
7. **测试隔离跟着改。** 仓库里生成过 seed 后，三个播种用例会读到真实 seed，前提失效；`SeedFixture` 现在把 `OMNIDESK_REPO_ROOT` 指到临时根并持环境锁，体检用例也持锁。并行连跑两次均 505/505。
8. **边界在文档里。** NSIS 写约 3 万个文件的安装耗时、长路径与杀软行为都列为实机项，没有拿本机 `.app` 的结果替代 Windows 安装验证。
9. **出包体积是资源生效的旁证。** v0.2.7 的 Windows 安装器从 2.71 MB 变成 **90.4 MB**，与「只在资源清单里加了运行时」这一改动相符；如果 `tauri.windows.conf.json` 没被合并，体积不会变。这也顺带说明平台专属配置确实生效。发布 job 还踩到「`cp` 到自身」的边界（macOS 的 zip 已在根目录），已修并在本版手动补挂资产。

**验证证据：** `verify_windows_installer_135.py` **27/27**、`cargo test --offline --lib` **505 passed / 0 failed**（并行两次）、`npm --prefix desktop run build` 通过、本机资源打包实测 `.app` 476.63 MiB、v0.2.7 安装器 90.4 MB；证据 `tests/verification/desktop/evidence/od-arch-135/facts.json`。

## OD-ARCH-134 审查结论（ARCHITECT，2026-11-04）——通过

**结论：通过。** Windows 发布物从「只有 zip」变成「NSIS 安装向导 + zip」，子进程统一不带控制台窗口，缺 node / python 时启动页直接给出可操作的检查页。三块都落在既有结构里：打包在 workflow 与 conf、平台差异在 `platform.rs`、失败提示在既有 React 外壳。

**独立复核范围：** HEAD `b3cda4fb17` + 工作区改动。核对 `release.yml` 两个 job 的构建命令与产物通配符是否对齐、`tauri.conf.json` 的 nsis 段与版本号、`sync-app-version.sh` 的幂等性、`platform.rs` 的 `hide_console` 与两组 Windows 候选、六处 spawn 是否都带上标志、`lib.rs` 的两处打开器替换与新命令注册、前端体检类型 / 调用 / 页面 / 渲染分支。`runtime/` 零差异；未新增权限，未改数据目录字段。

**审查判断（几处特意验过的边界）：**

1. **安装器与免安装包并存，路径对得上。** 构建用 `--bundles nsis`，上传同时收 `dist-package/*.zip` 与 `bundle/nsis/*.exe`，Release 的 `files:` 用两个通配符接住；macOS job 仍是 `--bundles app`，不受这次改动影响。`--no-bundle` 在整份 workflow 里已经不存在，不会出现「改了一处、另一处照旧跳过」。
2. **版本号只有一个事实源。** 产物名取自 `tauri.conf.json`、tag 取自 `VERSION`。此前两者已经漂移（0.2.3 vs 0.2.5），这轮先校正再让 CI 每次构建前同步；脚本内容不变时不写文件，不制造无意义差异。
3. **防闪窗只加创建标志，不动管道。** `CREATE_NO_WINDOW = 0x0800_0000` 只在 Windows 生效；引擎与设置服务的 stdin/stdout/stderr 仍是 piped，IPC 读写路径不变。非 Windows 分支是空操作，macOS / Linux 行为零变化。
4. **六处 spawn 一视同仁。** 引擎、DSH 设置服务、模型路由、端点分类器、插件安装、Windows 打开器都带上了标志。漏掉任一处，用户就会看到那一个黑框，所以这条按调用点逐个核过。
5. **Windows 候选不会盖过用户设置。** `OMNIDESK_NODE` / `OMNIDESK_PYTHON` 仍排在最前，候选只在「没设、PATH 也没有」时才用。`py` 启动器不是直接返回程序名，而是用 `py -3 -c print(sys.executable)` 问出真实路径——直接返回会让后续调用落到默认解释器上，可能是 2.7。
6. **体检失败是 fail-open。** 命令本身出错（序列化、宿主异常）时前端把结果置空、照常进界面；只有明确 `ok === false` 才接管主区域。这样不会因为一次探测失败把整个应用挡在门外。
7. **打开器替换修的是真 bug。** `profile_skill_source_open` 与 `profile_export_open` 原来各自拼 `xdg-open` / `gio` 循环，Windows 上根本没有这两个程序，必然失败；改走 `platform::open_target` 后与其它入口同一套实现。
8. **边界写在文档里。** NSIS 首跑、Windows 真实路径命中、无 node 机器上的检查页都列为用户实测项，没有把静态断言说成端到端验证。
9. **发布通配符按 artifact 的实际结构核过。** 第一次出包暴露了一个只在 CI 显形的问题：Windows artifact 同时含 `dist-package/` 与 `bundle/nsis/`，公共祖先变成仓库根，文件被放进两层目录，`artifacts/*.exe` 与 `artifacts/*.zip` 匹配不到，安装器生成了却没进 Release。现在发布 job 先把 `*.zip` / `*-setup.exe` 摊平到根目录，该次构建的安装器与 Windows zip 也已补挂到 v0.2.6。教训是发布路径要按 artifact 的真实层级验，不能照本地路径想当然。

**验证证据：** `verify_windows_packaging_134.py` **36/36**、`cargo test --offline --lib` **504 passed / 0 failed**、`npm --prefix desktop run build` 通过、release 构建通过、release 二进制启动冒烟正常、0.2.6 的 CI 出包成功（NSIS 产出 `OmniDesk_0.2.6_x64-setup.exe` 2.71 MiB）；证据 `tests/verification/desktop/evidence/od-arch-134/facts.json`。

**一句提醒：** 体检页给的是「修好再进来」的路径，门槛是硬的（Node 22+ / Python 3.10+）。如果之后收到「我明明装了却还提示未找到」的反馈，优先看两件事：安装器是否装在用户目录（`%LOCALAPPDATA%` 已覆盖）、以及是否仍在用旧版 zip 目录里的旧二进制启动。

## OD-ARCH-133 审查结论（ARCHITECT，2026-11-03）——通过

**结论：通过。** 宿主与引擎两侧的进程核验原先都直接读 Linux 的 `/proc`。macOS 上没有 `/proc`，`boot_id` / `proc_stat` / `proc_cmdline` / `group_members` 一律返回空，直接后果是单实例互斥失效（同一次开机的登记被判成上次开机残留而删除）与遗留进程组收不回来（成员表恒空，`terminate_group` 直接返回）。本轮把「怎么看进程」下沉到平台层，判定条件与拒绝路径保持不变。

**独立复核范围：** HEAD `b658d8a870` + 工作区改动。逐行核对 `platform.rs` 新增的 `ProcRow` 与四个函数的三种 `cfg` 分支（含 `kern.boottime` 解析与 `ps` 四列切分）；`process_owner.rs` 的委托替换、`group_members` 的僵尸过滤，以及 `reclaim_before_start` / `terminate_group` / `cleanup_engine_group` 未动；`process_owner.py` 的 darwin 分支与 Linux 分支对照；三处测试口径修正；新脚本与 `facts.json`。`runtime/` 零差异；没有新增命令、权限或数据目录字段。

**审查判断（几处特意验过的边界）：**

1. **只换「怎么看进程」，没换「什么情况下能杀」。** `reclaim_before_start` 的判定顺序（他人实例 → 身份不符 → 孤立子组 → 活跃引擎 → 残留登记）、拒绝文案与「核验不通过一个都不杀」全部原样；改动集中在数据来源函数，这是本轮最重要的边界。
2. **`cfg` 三分而不是二分。** 用 `cfg(not(any(target_os = "linux", target_os = "macos")))` 显式兜住 Windows，而不是 `cfg(not(target_os = "macos"))` 把 Windows 带进 `/proc` 分支；后者虽然运行时返回空，但语义上是「假装 Linux」，容易在后续改动里被当成可用路径。
3. **boot id 不同源，天然不串。** Linux 是内核 UUID，macOS 是 `darwin:<秒>`。两个平台的登记永不相等，跨平台拷贝数据根也不会被误认成本次开机。
4. **argv 相等不是唯一判据。** macOS 命令行来自 `ps args=` 的空白切分，路径带空格会丢边界；因此核验仍要求 pid + pgid + boot_id +「组内全是我方进程」同时成立。宁可拒绝回收、让用户手工处理，也不误杀——这与 074 的原始设计一致。
5. **`ps` 的调用点只在低频路径。** 复核了全部调用点（`engine_supervisor.rs` 926 / 1040 / 1095），都是启动回收与关闭兜底；心跳、状态查询、界面刷新都不经过这里。
6. **两处测试修正改的是口径，不是结论。** `task_shell` 那条断言比的是「返回的根是否就是传入的根」，规范化后比较才表达这个意思（`/var` 到 `/private/var`）；074 脚本的临时根同理。单测夹具的等待是补 fork/exec 竞态，Rust 侧同一用例早有相同等待，这次只是把口径对齐，没有放宽任何断言。
7. **旧登记不会被这次升级「顺手杀掉」。** `boot_id` 为空时走 `stale_boot`，只删 `owner.json`、不发信号；用户重开即可。这是升级路径上最需要保守的地方。

8. **并行单测的偶发红是测试隔离缺陷，不是本计划的回归；已顺手修掉。** 四个测试模块（`task_shell` / `settings` / `lib.rs::command_tests` / `engine_supervisor`）各自改 `OMNIDESK_*`，锁不共用，`command_tests` 那条甚至不持锁、结束时直接 `remove_var`，于是并行时别的用例读到了被改掉的数据根。现统一到 crate 级 `test_env_lock::ENV_LOCK`，并让 `command_tests` 保存/还原原值。判定依据是「同样的代码串行全绿、并行偶发红」——这类差异只能来自共享状态，不能靠重跑掩盖。改动仅限 `#[cfg(test)]` 代码。

**验证证据：** `verify_process_platform_133.py` **28/28**、`cargo test --offline --lib` **499 passed / 0 failed**（并行连跑 3 次一致）、引擎单测 **10/10**、`verify_process_lifecycle_074.py` **21/21**、`verify_worklog_132.py` **43/43**、`verify_worklog_104.py` **70/70**、前端 build 通过；证据 `tests/verification/desktop/evidence/od-arch-133/facts.json`。

**本轮未覆盖（不作为通过条件）：** 真实发布包在 macOS 上的端到端表现需要用户实测——强杀引擎后子进程组是否被自动收掉、同一数据根第二次启动是否给出可读拒绝。本轮只用本机假进程与原 074 脚本验证，不启动真实 DSH。

**一句提醒：** `ps` 输出是给人看的文本，不是结构化接口。后续若要把「路径带空格」这类情况做准，应换 `libproc` 或 `sysctl KERN_PROCARGS2`，而不是在切分规则上继续打补丁；在那之前，保守拒绝就是正确行为。

## OD-ARCH-132 审查结论（ARCHITECT，2026-11-03）——通过

**结论：通过。** 用户实测「先记一条、再补一句」落成了两条独立记录。根因是两条：Skill 只让模型「跟随上一条的日期」，落盘通道只有 append 且自检要求「原字节不变」，机制上就没有合并这条路。本轮新增 `merge` 判定通道与 `merge_entry` 落盘函数，把补充以「；」接在同一行末尾，并保持「除目标行外逐字节不变」这条护栏。

**独立复核范围：** HEAD `c878f0a5ca` + 工作区改动。逐行核对 `worklog.rs` 的 `MergeReceipt`、`merge_entry`、`assert_only_target_line_changed` 与 4 个单测；`lib.rs` 的 `worklog_merge`（红线扫描 → 掩码 → 落盘 → 失败回执）与命令注册；`workRecordVerdict.ts` 的解析与校验分支；`HomeChat.tsx` 的执行顺序与回执文案；`skills/memory-agent/SKILL.md` 全文与长度；`verify_worklog_132.py` 全文与 `facts.json`。`runtime/` 零差异；未新增权限，走既有 invoke 通道。

**审查判断（几处特意验过的边界）：**

1. **执行顺序必须是 merge → move → append。** 并入不增删条目、不改序号；移动会改序号。若先移动再并入，后面的并入会用快照里的旧索引去定位，可能落到别的条目上。初版写成 move → merge → append，本轮已改正，并把验证脚本的静态断言从 `move < merge < append` 改为 `merge < move < append`——断言跟着不变量走，而不是跟着实现行号走。
2. **「只改一行」是写前自证，不是事后假设。** `assert_only_target_line_changed` 要求：行数与原文一致、非目标行逐字节相等、目标行以原行开头且确有增长。任何一条不满足就中途中止，文件一字不动。这条比「记住只改一行」可靠，因为它拦的是实现漂移。
3. **定位口径与改日期共用一套。** `locate_entry_block(lines, date, index, expect_text)` 是既有函数，不新写一份解析。索引错位或正文不符时拒绝写入，单测断言「被拒绝后文件字节不变」，而不是只断言返回 Err。
4. **分号不叠加。** 目标行已以「；」或「;」结尾时直接接补充，否则补一个「；」。CRLF 文件保持 CRLF，单测同时断言 `\r\n` 与「不含 ；；」。
5. **掩码与红线在命令层，不在 Skill 层。** `worklog_merge` 先 `red_line_scan_with` 命中即拒，再 `mask_privacy` 掩码后落盘；与 `worklog_append` 同口径。回执里的 `masked / masked_id_card / masked_phone` 让界面能说明「已脱敏」，失败分支也带完整字段，前端不用靠 500 判断。
6. **旧判定协议不退化。** `merge` 字段缺失时按空数组处理，`verify_worklog_104.py` 70 项全通过；旧的两字段 / 三字段判定仍然有效。
7. **校验宁严不猜。** 补充必须是用户原话的连续片段；不能补同一条两次、不能与已有内容或本轮 append 重复、不能与同一条的移动同时出现；合并后不超过 1000 字符。模型输出越界时拒绝并回话，而不是「尽力而为」地并进去。
8. **Skill 仍在一页之内。** `memory-agent/SKILL.md` 1546 字符，在既有 2500 字符预算内；新增的只是「续述用 merge、不新起条目」与「一条记录只用一个动作」，没有引入新的必答步骤。
9. **没有新 UI。** 结果区加一行「· 并入 <日期> 第 N 条：<文本>」，索引页重读时间线。132 之前用户看不到「并入」这个动作，现在能看见，也方便手工纠正误判。

**验证证据：** `verify_worklog_132.py` **43/43**、`verify_worklog_104.py` **70/70**、`cargo test --offline --lib worklog` **42 passed / 0 failed**、`npm --prefix desktop run build` 通过；证据 `tests/verification/desktop/evidence/od-arch-132/facts.json`。全量 `cargo test --lib` 为 495 passed / 4 failed，4 项均在本轮未触及的模块，且都是 macOS 平台差异：`process_owner` 三项读 `/proc`（Linux 专有），`task_shell` 一项用 `dir.display()` 与规范化路径比较（`/var` → `/private/var`）。这部分不作为本计划的通过条件，也不为过测试去改生产代码。

**本轮未覆盖（不作为通过条件）：** 真实模型是否把「还参观了…」判成 merge 属 OD-ARCH-080 真实端到端实测，需用户用真实供应商跑一次；「感受追加」这类边界表述的实际命中率同理，只能实测。

**一句提醒：** 并入会改写既有行，这与「工作记录只增不改」的老约定相反。护栏全靠 `assert_only_target_line_changed` 与 `commit_week_file` 的回读自证，后续若有人想「顺手清理空行」或「顺便规整格式」，这条护栏会直接拒绝——这是有意的，不要为了通过测试去放宽它。

## OD-ARCH-131 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户的产品边界是「OmniDesk 干净初始、Profile 定制，切过去配置 / Skill / 插件自动到位」。130 之后逐条对表，切换装配、保存绑定收敛、缺插件三条补装通道都通，只剩一处真缺口：**自建 Profile 只能绑内置 8 个 Skill**——`profile_rules.rs` 的 `validate()` 按 `ALLOWED_SKILLS` 整份拒绝，装配链也没有「Profile 自带正文」这一层，用户自己写的 Skill 连存都存不住。本轮把它补成三层取源，并且没有动基础标准零写入、出厂清单与停放区口径。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_rules.rs` 的 `is_valid_skill_name` / `skill_source_root` / `local_skills` / `allowed_with_local` 全文、`validate()` 的校验替换、`RulesView` 五个构造点；`profile_assets.rs` 的 `SkillSource` 与 `expected_skill_files` 三态取源、三处调用点、`materialize_custom` 删重复分支后的一致性；`lib.rs` 两个新命令的拒绝条件、路径校验与原子写、命令注册；`profile_lifecycle.rs` 的 `collect_skill_files` / `collect_skill_dir` / `check_skill_bundle_path` / `verify_bundle` 分支 / 导入落点；`SettingsPanel.tsx` 区块与两个 callback；`types.ts` / `api.ts` 的类型；窗口脚本与 `facts.json`、5 张截图。`runtime/` 零差异；`capabilities/default.json` 未改动（走既有 invoke 通道）。

**审查判断（几处特意验过的边界）：**

1. **白名单降级没有放宽路径安全。** 校验拆成两层：`validate()` 只认形状（小写字母／数字／`-`／`_`，1–48 字符），`expected_skill_files` 在拼任何路径**之前**再判一次形状，非法名直接返回 `None`。审查确认这条在取源函数的最前面，而不是只靠调用方先校验——三层来源都按名字拼目录，漏一处就能借 `..` 逃逸。
2. **迁移包落点必须与取源根一致，这是本轮修掉的真缺陷。** 导出按 `skill_source_root`（`profiles/<名字>/skills/`）收集，导入原先写成 `dir.join(&file.path)`，会落到 `profiles/<名字>/<Skill 名>/`：包导得进来，界面 `local_skills` 却为空，装配链也看不到。现在按 `root == SKILL_BUNDLE_ROOT` 加 `skills/` 前缀，往返测试同时断言落点文件与 `local_skills`，这条断言不可省。
3. **同一份取源逻辑只有一个实现。** `materialize_custom` 原先重复了一遍「仓库 → 内嵌」挑源，新增第三层时若只改一处，切换路径与保存路径就会各取一份源。现在三处调用同一个 `expected_skill_files`，重复分支已删，审查逐行确认没有残留旧分支。
4. **三态写与停放区口径原样保留。** 新增来源走的是既有 `sync_skill_files`（created / updated / unchanged / drift），用户手改的正文仍不覆盖；解绑仍进停放区而不是删除。自带源只是「多一个更具体的来源」，不是「有权覆盖」。
5. **基础标准与出厂 Profile 完全不进新分支。** 两条命令首句判 `standard` 即拒；`expected_skill_files` 的自带源只在 `profiles/<非标准名>/skills/` 存在时才命中。窗口脚本第一轮断言基础标准下 `app_profile_dirs=[]`、且规则页没有「Profile 自带 Skill」区块。
6. **命令的路径校验按目标根，不按引擎根。** 这两条命令只写 `<应用数据根>/profiles/`，复核中发现初版误用引擎 `dsh_home` 做 `assert_writable_root`，已改为校验 `scope.profiles_root`——校验对象与写入对象必须是同一个根，否则等于没验。
7. **模板生成不覆盖已有正文。** `profile_skill_scaffold` 在 `SKILL.md` 已存在时返回 `created: false` 与说明，不写盘；窗口脚本覆盖了「生成一次成功 → 落盘 → 出现在勾选片」这条主路径。
8. **迁移包对外部输入逐段白名单。** `check_skill_bundle_path` 拒绝反斜杠、绝对路径、少于两段、首段非合法 Skill 名、任何空段／`.`／`..`／隐藏段；成员上限从 16 提到 256（自带 Skill 会产生多成员），单个 Skill 限 64 文件、单文件 1MB、整包 2MB，正文必须 UTF-8。`import_refuses_skill_traversal_paths` 覆盖上跳分支。
9. **旧包兼容。** 没有 `root: "skill"` 成员的包走原分支，既有 4 个导入测试全绿；`BUNDLE_FILE_MAX_COUNT` 的上调不改变旧包行为。
10. **测试改动是口径变更而不是迁就实现。** `unknown_skill_never_touches_disk` 原来断言「清单外的名字必须被拒」，这条前提在 131 已被有意推翻（用户自带 Skill 本来就不在清单里）。改为断言非法形状（`../escape`）不落盘，保护目标从「名字必须在清单」变成「名字必须拼得出目录」。

**验证证据：** `cargo test --offline --lib` **495 passed / 0 failed**（130 基线 490；本轮新增规则 3、装配 1、迁移 2，`lib.rs` 那条改名不增数）；`cargo test --offline --tests` 12 组全通过、EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npm --prefix desktop run build` 通过；`cargo build --offline --bin omnidesk-desktop` EXIT=0；真实窗口 `preview_profile_skill_source_131.py` **24/24 checks 全 true**、EXIT=0。证据在 `tests/verification/desktop/evidence/od-arch-131/`（`facts.json` + 5 张截图，已肉眼核对：基础标准规则页无自带 Skill 区块；自建 Profile 生成模板后 `office-note` 出现在每一行的勾选片末尾并被选中；保存后摘要为 weekly（office-note））。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍由用户亲自实测；「自带 Skill 在真实提交里被预注入」属运行期行为，本轮只验证到目录、绑定与正文一致；自带 Skill 携带二进制附件（图片、docx）不支持，只收 UTF-8 文本；仓库侧同名 Skill 更新后，已装配的运行时副本按三态比对更新，自带源则按自带正文为准。

**一句提醒：** 取源顺序现在是「Profile 自带 → 仓库 → 内嵌」，按「越具体越优先」排。不要为了省一次 `is_file()` 把它调成「仓库优先」——那会让用户的定制在开发机上被仓库同名 Skill 静默盖掉，且只在开发机上出现，很难被发现。


## OD-ARCH-130 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户的产品边界是「OmniDesk 是干净初始环境，Profile 是定制的，切换 Profile 时要能自动写入所需的配置、Skill 和插件，缺插件要有自动安装」。按这条逐项对表，**切换 Profile 的自动装配本来就已经通**：切换时 `materialize` 补配置与 Skill，`record_managed_skills` 登记归属；引擎启动前 `engine_supervisor.rs:295` 再收敛一次；缺插件在切换后（`spawn_profile_plugins_autofill`，`lib.rs:1395`）、冷启动（`maybe_autofill_plugins_on_startup`，`:1448`）、设置页手动三条通道补装，都受 `profile_auto_install_plugins`（默认开）约束，基础标准恒不装。本轮修的是一处真实缺口：**设置页保存 Skill 绑定后只写 `rules.json`，不落运行时 `skills/`**，界面说「已保存」而引擎要等下次切换或重启才扫得到。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_assets.rs` 的 `BoundSkillSync`（`:418`）与 `sync_bound_skills`（`:447`）全文、`wanted_skills` / `reconcile_managed_skills` / `sync_skill_files` / `expected_skill_files` / `save_state` / `record_managed_skills` 的调用顺序与返回值处理、两个新单测（`:2602`、`:2672`）；`lib.rs` 的 `profile_skill_bindings_save`（`:2120`）与 `save_skill_bindings_into`（`:2165`）；`types.ts:1358` / `:1362`；`SettingsPanel.tsx:1091` 的 `saveBindings` 与 `:3249` 的状态行；`preview_profile_skill_bindings_130.py` 全文与 `facts.json`、4 张截图。`runtime/` 零差异；`capabilities/default.json` 未改动（无新命令、无新权限）。

**审查判断（几处特意验过的边界）：**

1. **收敛函数只做收敛，不夹带装配。** `sync_bound_skills` 里没有 `materialize`、没有插件探测、没有联网调用，只有「取清单 → 归位/停放 → 补写 → 登记」。保存绑定这条路径因此不会顺带重写场景文件或声明文件，129 建立的「只换绑定、不动其它规则」边界没有被打破。窗口用例实测 `global` / `scenes` 保存前后逐项一致（`global_untouched`、`scenes_untouched` 均为 true）。
2. **`dsh_home` 取运行中引擎的配置，这是本轮最容易被写错的一处。** 命令里没有用 `profile_choice_scope()` 或按 Profile 名拼路径，而是 `host.bridge.supervisor().lock().unwrap().config()` 取 `dsh_home` 与 `channel`，再从 `app_root` 拼 `Scope`。切换与启动装配用的正是这份配置；若这里另算一个根，回执会显示补齐而引擎仍扫不到——那种失败不会在单测里暴露，只在真实窗口里出现，所以这条按阻塞级缺陷对待。
3. **规则写盘成功就不回滚，是有意的。** 绑定是用户的决定，Skill 拷贝可能因缺源或权限失败；把规则退回去会让用户「点了没反应且不知道原因」。现在的处理是规则保留、`skills.failed` 逐条带原因、`skills.note` 说明状态，前端把这些拼进提示（`SettingsPanel.tsx:1114`）。审查时确认失败分支没有吞掉原始错误信息。
4. **基础标准的空操作在写盘与建目录之前。** `sync_bound_skills` 第一句判合法名、第二句判 `profile == standard` 立即返回 `standard: true`，不触碰 `dsh_home`、不建 `profiles/`。单测 `sync_bound_skills_is_noop_for_standard_profile` 断言 `skills/` 与 `profiles/` 事后都不存在，与 128 的 `save_rules_into` 同口径——断言目录不存在，而不是只断言返回值。
5. **解绑走停放区，且不误伤仍绑定的。** 收敛先跑 `reconcile_managed_skills` 再补写，单测第三段把 `memory-report` 解绑后断言：停放在 `parked_root` 里有目录、运行时目录里没有、`memory-agent` 的 `SKILL.md` 仍在。这条断言是必要的，因为「按新清单清理」若实现成「清空重写」，仍绑定的那个也会被短暂删掉。
6. **二次保存不重复写。** 单测第二段断言 `filled` 为空、`kept` 为 2。`sync_skill_files` 的三态写本来就区分「缺 / 一致 / 用户改过」，本轮没有绕过它直接拷贝，所以用户手改过的 Skill 正文不会被覆盖——这是 109 之后一直守着的边界。
7. **回执字段是新增而不是替换。** `skills` 以 `map.insert` 并入 `receipt`，原有的 `ok` / `path` / `backup` / `digest` / `note` 全部保留；前端按可选字段读（`synced?.note`、`synced?.failed?.length`），旧回执或收敛早期失败时不会崩。收敛整体报错时写 `{ok:false, note}` 而不是让命令失败——规则已经存住了，返回 Err 会让前端把整次保存显示成失败。
8. **`record_managed_skills` 在补写之后调用，顺序不能反。** 它按「运行时目录里真有 `SKILL.md`」来登记归属，先登记后补写会漏登本次新增的 Skill，切走时收不回。审查确认调用点在循环之后（`:504`），且 `save_state` 失败只记 `failed` 不中断登记。
9. **窗口证据与单测互补，不是重复。** 单测证明函数行为，窗口脚本证明这条链在真实 Tauri + WebKit 里跑得通：`runtime_skill_dirs = ["anysearch","memory-agent","memory-report"]`、`.omnidesk-managed` 已写、基础标准轮 `app_profile_dirs=[]` 与 `dsh_profile_dirs=[]`、第 4 个 `knowledge-search` 点击被禁未进文件、备份 `rules.json.bak-1793276897` 存在。25/25 全 true，EXIT=0。
10. **没有新增联网与后台时机。** 保存只写本地规则与本地 Skill 目录，不触发插件安装、不重启引擎、不切换 Profile。

**验证证据：** `cargo test --offline --lib` **490 passed / 0 failed**（基线 488，本轮 +2：`sync_bound_skills_fills_runtime_dir_and_parks_unbound`、`sync_bound_skills_is_noop_for_standard_profile`）；`cargo test --offline --tests` 8 passed、EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npm --prefix desktop run build` 通过；debug 构建 EXIT=0；真实窗口 `preview_profile_skill_bindings_130.py` **25/25 checks 全 true**、EXIT=0。证据在 `tests/verification/desktop/evidence/od-arch-130/`（`facts.json` + 4 张截图，已肉眼核对：基础标准规则页无绑定区块；自建 Profile 三个片高亮、第四个禁用；保存后摘要为 weekly（memory-agent, memory-report, anysearch））。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍由用户亲自实测；「保存后下一轮真的只注入这几个 Skill」属运行期行为，本轮只验证到目录与登记表就位，注入效果依赖 108 的既有装配链；`missing_source` 分支（仓库与内嵌副本都没有该 Skill）只有代码路径，无构造用例；两个窗口同时保存同一 Profile 无互斥，最后写入者胜。

**一句提醒：** 这条链现在的分工是清楚的——切换 Profile 负责「整套装配」，保存绑定负责「只收敛 Skill 目录」，插件补装另走三条通道。以后若加「Profile 内自带 Skill 源目录」或「按场景不同源」，那是 `expected_skill_files` 的取源规则升级，应另立决策，不要往保存绑定这条窄路径上加分支。


## OD-ARCH-129 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户的产品边界是「OmniDesk 是干净初始环境，Profile 是定制的，切过去就该是专属工作台」。128 之后，建 / 复制 / 切换 / 装配 / 补装 / 巡检 / 移除 / 恢复 / 改名 / 迁移 / 声明插件全部落地，但 108 定下的那条底线——**场景绑定 Skill，模型再按需加载额外的**——在界面上配不出来：规则资产页只有一行只读摘要，用户要改「随手记只注入 memory-agent、周报只注入 memory-report」，只能下来手写 `rules.json`。本轮把这条配置面做进界面，且严格复用既有的校验、备份与原子写通道。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_rules.rs` 的 `allowed()` / `BindingChoice` / `set_skill_bindings` 与 4 个新单测；`lib.rs` 的 `profile_skill_bindings_save`（`:2120`）、`save_skill_bindings_into`（`:2132`）、命令注册（`:5000`）与 4 个行为测试；`types.ts` / `api.ts`；`SettingsPanel.tsx` 的 `BINDING_SCENES` / `toggleBinding` / `saveBindings` / 区块渲染与基础标准门禁；`settings-panel.css` 的 `.binding-*`；`preview_profile_skill_bindings_129.py` 全文与 `facts.json`、4 张截图。`runtime/` 零差异；`capabilities/default.json` 未改动（绑定保存走既有 invoke 通道）。

**审查判断（几处特意验过的边界）：**

1. **整表替换而不是逐个增删，是对的。** 界面拿到的是一份完整草稿，用户可能同时勾一个、取消一个；若后端做成增量，两次点击要么产生两次写盘，要么需要前端算差集。`set_skill_bindings` 只做「解析 → 替换 → 校验 → 序列化」，单测 `set_skill_bindings_replaces_only_bindings` 断言 `global` / `scenes` / `profile` 与原文逐项一致，其余字段确实没被牵连。
2. **空数组写成映射而非删键，有可核对的理由。** 装配侧对「空绑定」与「没有这个键」处理一致（都不预注入），但文件里能看出用户动过这一行。`set_skill_bindings_allows_explicit_unbind` 同时断言 `skills.is_empty()` 与 `max_injected == 1`——没有这一条，空选择会撞上 `max_injected >= 1` 的校验下限。
3. **`allowed_skills` 由后端下发，避免「能选、存不进去」。** 校验器只认 `ALLOWED_SKILLS`（8 项）；若前端另存一份清单，加一项 Skill 时忘了同步就会出现可点却报错的片。现在 `RulesView` 的五个构造分支都填了 `allowed()`，包含「文件不存在」「读取失败」「超限」「损坏」四类早退分支——这一点我逐个核过，早退分支最容易漏。
4. **新 Profile 不必先装行业规则。** 规则文件不存在时用 `profile_starter::rules_text` 的起步骨架打底（`global: {}`、`scenes: {}`、空绑定），保存后得到一份只有绑定、没有任何行业内容的规则包。这条是有意的：用户想选三个 Skill 才点开绑定，不该被迫先装一套纪检规则包。
5. **基础标准仍零写入，且拒绝点在写盘之前。** `save_skill_bindings_into` 第一句判 `standard`，单测 `standard_refuses_and_writes_nothing` 断言目标目录**事后不存在**，而不是只断言返回 Err——与 128 的 `save_rules_into` 同口径。前端另用 `!declStandard` 不渲染整块，但它只是不渲染，真正判定在后端。
6. **保存只提交「界面上出现过的场景」。** `saveBindings` 先按 `BINDING_SCENES` 过滤 `scene in bindingDraft`，再把 `extras`（用户手写的其它场景键）原样带回。若无条件写五个固定场景，新 Profile 会凭空多出五行空绑定；若漏掉 `extras`，手写绑定会被回填后再被保存冲掉。窗口脚本实测保存前只有 `{}`、保存后只有 `weekly` 一项，正是这条的直接证据。
7. **每场景上限 3 与宿主校验同口径。** 前端 `MAX_BINDINGS_PER_SCENE = 3` 只负责禁用与提示，真正的拒绝在 `validate()`；单测另有「超 3 个 / 未知技能整体拒绝」一条。窗口脚本点了第 4 个（`knowledge-search`，此时已满 3 个），断言它没进文件——界面禁用与后端校验两层都在，其中任何一层单独失效都不会造成越界写入。
8. **窗口脚本首版有两处假失败，改判定而不是改结论。** ①用「行业规则」说明段落判定页面已打开，但 WebKit 不把 `.hint` 段落暴露给 AT-SPI，截图明明在该页却判 false；改成页面上真实暴露的按钮文案，并在进入前先判「已在该页」避免把页面切走。②`global: {}` 保存后会被序列化成默认值（空红线词、空铁律、身份证/手机号 mask），首版按字节比对 `global` 因此误判；改成比对生效策略。两处都是**判据错**而非功能错，但如果不修就写「通过」，等于用错误的断言掩盖真实状态——这一点如实记录在 STATUS 与本节。
9. **布局问题靠截图发现，已修。** 首版两列布局下 8 个片折行，第二行的片与上一行场景对不齐，用户无法判断归属。改为场景名与说明在上、片在下方整行铺开，同一场景的片不再跨行。改完重跑自建轮，24/24 仍全通过且截图核对无折行。
10. **本轮没有新增联网或后台时机。** 保存只写本地规则文件；不触发插件安装、不重启引擎、不切换 Profile，回执明确写「下一轮提交生效，无需重启」。

**验证证据：** `cargo test --offline --lib` **488 passed / 0 failed**（基线 480，本轮新增 8：`set_skill_bindings_replaces_only_bindings`、`set_skill_bindings_allows_explicit_unbind`、`set_skill_bindings_dedupes_and_sizes_max_injected`、`set_skill_bindings_rejects_unknown_skill_and_overflow`，以及 `profile_skill_bindings_tests` 的 `standard_refuses_and_writes_nothing`、`new_profile_uses_starter_skeleton`、`only_bindings_change`、`unknown_skill_leaves_file_untouched`）；`cargo test --offline --tests` EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npm run build` 通过；debug 构建 EXIT=0；真实窗口 `preview_profile_skill_bindings_129.py` **24/24 checks 全 true**、EXIT=0，截图与 `facts.json` 在 `tests/verification/desktop/evidence/od-arch-129/`，四张截图已肉眼核对。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍由用户亲自实测；绑定保存后「下一轮真的只注入这几个 Skill」属运行期行为，本轮只验证到写盘与摘要读数，注入效果依赖既有 108 的装配链；五个固定场景被窗口覆盖，用户手写自定义场景键的回传逻辑有代码与类型保护但无独立窗口用例；两个窗口同时保存同一 Profile 无互斥，最后写入者胜。

**一句提醒：** 这条链现在的分工是清楚的——界面配「预注入哪几个」，模型按需加载其余，平台代码不判断业务。后续若要加「某场景固定排除某 Skill」或「按周不同绑定」，那属于绑定表结构升级（`max_injected` 已占位，但缺「排除」语义），应另立决策，不要往界面里堆开关。


## OD-ARCH-128 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户的产品边界是「OmniDesk 是干净和初始的环境，Profile 是定制的，切换 Profile 时要能自动写入所需配置、Skill 和插件，缺插件能自动安装」。127 之后这条链的主干通了，但按这份边界逐条对表仍有两处逆向的缝：干净底座还能被写进业务规则，「定制」的最后一个动作（加插件）却只能在软件外面手改文件。前者让分水岭失效，后者让用户配不出自己的工作台。本轮两处都收进代码：基础标准下规则保存一律拒绝且不建目录；自建 Profile 能声明并安装插件，装成功才登记。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_assets.rs` 的 `parse_plugin_spec` / `normalize_plugin_input` / `AddPluginReceipt` / `add_plugin` / `inspect_custom` 插件段与 4 个新单测；`lib.rs` 的 `profile_rules_save` / `save_rules_into` / `profile_rules_restore_default` 对照与 2 个新单测、`profile_plugins_add` 命令体（`:1950`）与 `generate_handler!` 注册（`:4943`）；`types.ts` / `api.ts` 的类型与封装；`SettingsPanel.tsx` 的 `addPlugin` 回调、`isCustomAssetProfile` 门禁与声明窄条渲染；`preview_profile_plugins_128.py` 全文与 `facts.json`、3 张截图。`runtime/` 零差异；`capabilities/default.json` 未新增权限（本轮没有新命令需要放开 capability，走的是已有 invoke 通道）。

**审查判断（几处特意验过的边界）：**

1. **拒绝点前移到解析之前，这不是风格问题。** `write_profile_rules` 第一句是 `create_dir_all`，任何「先校验后判标准」的写法只要中间有一次写盘机会，就会给干净底座留下一个空 Profile 目录；`current_rule_policy` 从同一路径读规则，目录在就意味着这条链开始认它。`save_rules_into` 把 `standard` 判断放在函数第一句，单测 `save_rules_into` 拒绝分支断言目标目录事后不存在，而不是只断言返回 Err。
2. **抽成纯函数是为了让这条边界可测。** 原命令体直接依赖 `State`，Tauri 外壳里没法在单测中断言「磁盘没变」；抽出的 `save_rules_into(profiles_dir, standard, text)` 只吃路径与布尔，三个测试分别覆盖标准拒绝、正常写入、坏 JSON 拒绝，行为与命令体一致（命令体只剩一行转发）。
3. **「装成功才登记」的顺序不能反，测试是分三条写的。** `add_plugin_registers_already_ready_package_without_installing`（就绪只登记、`installed=false`）、`add_plugin_is_idempotent_for_the_same_package`（重复声明不产生第二项）、`add_plugin_leaves_registry_alone_when_install_fails`（装失败登记表逐字节不变）。三条各自独立，避免一个测试同时断言两件事时失败原因含混。
4. **`scoped_ok` 的 `strip_prefix('@')` 两段判定是对的，且理由在注释里。** 关键在 `parse_plugin_spec` 会先把子路径折回包根：`@/abs/path` 折成 `@/abs`，此时「两段」成立但 scope 是空的，若只数斜杠数量会放行一个绝对路径残留。现在的判定要求 scope 与 name 都非空、且 name 不再含 `/`，配合前面的 `..` / 前导 `/` / 前导 `.` 三条前置拒绝，形状检查不留交叉盲区。
5. **版本与子路径折回包根，是安装通道的硬要求。** 登记表里可能是补丁写法 `@acme/plugin/skill/office@2.0.0`，而 `install_plugin` 只认包根。`parse_plugin_spec` 的 `rsplit_once('@')` 带三个守卫（`p` 非空、`v` 非空、`p` 不以 `/` 结尾），挡住 `@scope/plugin@` 与 `pkg@/sub` 这类误拆；巡检的 `key` 也统一改用包根，界面回传时不会被安装通道拒绝。
6. **巡检改并集，解决的是「界面与装配判断不一致」。** 用户在命令行装过插件但没切换过时，登记表里没有它；只读登记表会让设置页显示「0 项」而装配时却能加载。并集只加不减，`detect_plugins` 走的是已有只读探测，不回写登记表、不装任何东西——审查时确认 `inspect_custom` 里没有出现 `save_registry` 调用。
7. **三处入口拒绝而不是只拦一处。** 基础标准（干净底座）、出厂 Profile（清单随版本固定）、未登记目录（历史遗留，软件里看不见）都在写盘之前拦。允许其中任何一处写登记表，都会出现「界面里有一条用户改不动、也说不清来源的依赖」。前端另有 `isCustomAssetProfile` 门禁，但门禁只是不渲染，真正的判定在后端——这一点在审查里特意确认过，因为只靠前端隐藏会留下可 invoke 的通道。
8. **界面走行内窄条，与前几轮同构。** 设置页既有列表全是「标题 + 徽标 + 行尾动作」的直角行，本轮沿用：输入框与按钮在同一行，被拒理由与成功回执都写在该行的 `.sub` 区域（`role="alert"` / `role="status"`），不弹窗、不跳页。`onFocus` 全选是为了让用户直接覆盖上一次被拒的写法，否则 Enter 会在旧串后面拼接，制造一个「看着像新输入、其实还是坏包名」的错觉。
9. **回执的 `status` 是 `Option<String>`，如实反映而不是假装成功。** 安装失败时 `ok=false`、`status` 仍带探测状态、`note` 写明「安装后仍不可用，未写入插件清单」。前端依据 `ok` 决定清空输入框，失败时保留原文让用户改，这是有意的：失败后清空会迫使用户重新敲一遍长包名。
10. **本轮没有新增联网时机。** 安装仍只由用户点按钮触发，与 121 / 124 的「不自动联网」口径一致；声明通道没有引入新的后台任务，也没有改变冷启动自愈的行为。`capabilities/default.json` 未改动（审查时用 `git status` 与 `grep` 双向确认）。

**验证证据：** `cargo test --offline --lib` **480 passed / 0 failed**（基线 471，本轮新增 9：`parse_plugin_spec_folds_subpaths_and_versions`、`normalize_plugin_input_rejects_paths_and_keeps_package_roots`、`add_plugin_refuses_standard_profile_without_touching_disk`、`add_plugin_registers_already_ready_package_without_installing`、`add_plugin_is_idempotent_for_the_same_package`、`add_plugin_leaves_registry_alone_when_install_fails`，以及基础标准规则保存的 3 个）；`--tests` EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npx tsc --noEmit` 与 `npm run build` 通过；debug 构建 EXIT=0；真实窗口 `preview_profile_plugins_128.py` **27/27 checks 全 true**、EXIT=0（含 `no_app_profile_written`、`no_dsh_profile_written`、`bad_rejected_inline`、`bad_left_registry_alone`、`registry_exactly_one`），截图与 `facts.json` 在 `tests/verification/desktop/evidence/od-arch-128/`，三张截图已肉眼核对（基础标准规则页无目录写入、非法包名红色行内提示、合法包名登记回执一行）。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍未跑，由用户亲自实测；真实联网安装一个公开 npm 包未在本轮执行（窗口脚本用本地已就绪包验证登记链，安装成功路径由既有 121 / 124 的口径覆盖）；并发两处同时声明同一包未做互斥，最后写入者胜（登记表去重会让结果收敛为一项）；声明通道的用户可见文案目前是中文硬编码，Profile 的显示名不参与拼接。

**一句提醒：** 这一轮之后「基础标准就是一个只有 UI 的 DSH」这句话才真正成立——但成立的前提是别在别处再开写盘口子（例如直接手改 `profiles/index.v1.json`）。若后续要给基础标准加「临时试装一个插件」的能力，那需要单独设计一个不落登记表的会话级通道，而不是放宽这里的拒绝。


## OD-ARCH-127 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户的产品边界是「OmniDesk 是干净和初始的环境，Profile 是定制的」，109–126 把「切过去就装好、缺件能补、删了能回、名字能改」都做成了，唯独这套定制**出不了这台机器**。一个把工作台调了两三个月的人，换机只有两条路：整盘拷数据根（把会话、缓存、凭证引用一起带走），或者从零重配。本轮把「带走」做成一个可审阅的单文件包，校验全部前置，同名一律拒绝。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_lifecycle.rs` 的 `BUNDLE_SCHEMA` / `bundle_file_name` / `civil_from_days` / `collect_bundle_files` / `export_bundle` / `check_bundle_path` / `verify_bundle` / `import_bundle` / `list_exports` 与 7 个新单测；`lib.rs` 的 `profile_export` / `profile_import` / `profile_export_list` / `profile_export_open` 命令体与 `generate_handler!` 注册；`types.ts` / `api.ts` 的类型与封装；`SettingsPanel.tsx` 的导出、导入、冲突改名、迁移包区块与回执；`capabilities/default.json`（确认无 `dialog:allow-save` 残留）；`preview_profile_import_export_127.py` 全文与 `facts.json`、4 张截图。`runtime/` 零差异。

**审查判断（几处特意验过的边界）：**

1. **包是「包」，不是目录拷贝。** `node_modules` 跨架构不可移植，打进去会得到一个「看起来装好了、实际加载不了」的 Profile，而且违反本仓库既有的「插件由 DSH 官方通道按平台安装」这一条。包里只放十个以内的文本配置，插件清单进登记表，导入后由 `required_plugins` + `ensure_plugins` 接管。这条已登记为 D-056。
2. **校验先于落盘，且顺序是刻意的。** `verify_bundle` 全程只读：包大小 → schema → 成员数 → Profile 名与显示名 → 单段文件名白名单 → 字节数 → SHA-256 → 四类内容解析。任何一项失败，磁盘上零变化——单测 `import_refuses_tampered_bundle_without_touching_disk` 断言了目标目录前后都不存在。
3. **字节数在摘要之前，这不是冗余。** 用等长替换（`迁移测试`→`被改名字`）构造的篡改包，两者都能拦；但先撞字节数能在明显不完整（截断、拼接）的包上给出更直白的回执，也让 SHA-256 只处理尺寸可信的输入。两个检查各自有独立单测口径。
4. **路径白名单拒四类形状。** `../`、绝对路径（含反斜杠）、子目录、前导点，任一出现即整体拒绝；随后还要落在 `DSH_BUNDLE_FILES` / `APP_BUNDLE_FILES` 的固定名单里。`check_bundle_path` 不依赖调用方先做规范化，是这条防线唯一的入口。
5. **同名绝不覆盖，理由是不可逆。** 登记表有同名、DSH 目录已存在、应用目录已存在、是出厂名——四条都在写盘之前拦。合并两套工作台的规则、场景、Skill 绑定是不可逆操作，界面的正确回应是「换个名字」，所以冲突时该行就地变输入框，而不是弹一个「覆盖吗」。
6. **`categories.json` 的 `profile` 字段必须重写。** 它把分类归属钉在某个 Profile 名上；改名导入后若原样写回，切过去会出现「表在、但被判定成别人的表」。改写放在落盘循环里、紧挨写入之前，且先解析再重建结构，不靠字符串替换。
7. **登记表最后写，失败收回两个目录。** 顺序是临时目录 → `rename` 到位 → 登记表。登记写失败时 `cleanup` 把 `dsh_dest` 与 `app_dest` 一起清掉，回执写明「导入已撤回」。反过来先写登记会让软件里出现一条指向不存在目录的条目，那是更糟的失败形态。
8. **导出拒绝出厂 Profile 与未登记目录。** 出厂 Profile 的资产来自编译期清单，导出的包在别的机器上装回来也只是同名同内容，没有迁移价值；未登记目录属于历史遗留，导出等于把一份软件里看不见的东西打包。两条都在 `export_bundle` 入口拦。
9. **`list_exports` 只读且能容忍脏目录。** 用户会往 `exports/` 里手动删改文件，坏包、超限包、隐藏文件一律跳过而不是整块报错；不建目录、不写盘，界面每次 `refreshAssets` 都能安全调用。单测 `list_exports_skips_broken_and_sorts_newest_first` 覆盖了跳过与倒序。
10. **导出不走系统保存框，因此不加 `dialog:allow-save`。** 曾一度为保存框加了这条权限，后改为固定落 `exports/` 撤回。撤回是对的：固定路径让导出结果可复现（脚本与用户都能按路径找到包），少一次系统对话框，也让 capabilities 保持最小集。本轮结束前已确认该权限不在 `capabilities/default.json`。
11. **插件不进包，靠既有通道。** 导入回执会写明「会按清单检查 N 个插件，缺的会自动补齐」，走的是 121 / 124 已经实测过的 `ensure_plugins` 与冷启动自愈，本轮没有新写任何安装逻辑，也没有在导入路径上联网。

**验证证据：** `cargo test --offline --lib profile_lifecycle` 27 passed（本轮新增 7）；`cargo test --offline --lib` **471 passed / 0 failed**（基线 464）；`--tests` EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npx tsc --noEmit` 与 `npm run build` 通过；debug 构建 EXIT=0；真实窗口 `preview_profile_import_export_127.py` **31/31 checks 全 true**、EXIT=0（含 `bundle_no_node_modules`、`bundle_schema_ok`、`no_import_leftover`、`label_preserved`、`switched`、`dock_has_scenes`），截图与 `facts.json` 在 `tests/verification/desktop/evidence/od-arch-127/`；回归 125 / 126 / 118 三个窗口脚本全部 EXIT=0。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍未跑，由用户亲自实测；跨机器实测只做了同机往返（不同架构、不同 DSH 版本未测）；包内不含 DSH 版本约束，版本跨度过大时 `cordis` 补丁可能需要手工调整；导出目录不会自动清理，包会一直累积；并发导入同一包未做互斥。

**一句提醒：** 迁移包不是备份方案。它只搬「工作台配置」，会话、线程、缓存、成果文档都在包外；真正的备份仍是 `/home/kevin/OmniDesk/backups/` 与仓库外的成果目录。


## OD-ARCH-126 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户的产品边界是「OmniDesk 是干净底座、Profile 是定制的」，109–125 把「切过去就装好、缺件能补、删了能回」都做成了，唯独**名字**建成即定死。这不算小瑕疵：一个把「工作」打成「工作2」再删掉重建的人，丢的是整个装配结果。本轮把改名做成纯本地、只碰两处显示名真身的操作，判定与落盘都在代码里。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_lifecycle.rs` 的 `RenameReceipt` / `clean_display_label` / `display_name_line` / `with_display_name` / `rename_label` 与 5 个新单测；`lib.rs` 的 `profile_rename` 命令体与 `generate_handler!` 注册；`types.ts` / `api.ts` 的类型与封装；`SettingsPanel.tsx` 的行内编辑状态机与渲染；`settings-panel.css` 的两条新样式；`preview_profile_rename_126.py` 全文与 `facts.json`、5 张截图。`runtime/` 零差异。

**审查判断（几处特意验过的边界）：**

1. **为什么严格不动目录名。** `--profile` 传的是目录名，`index.v1.json` 的 `name`、`rules.json` 的 `profile` 字段、Skill 归属判断、DSH 侧 `profiles/<name>/` 全部以它为键。改名若连带搬目录，需要同时改写这四类引用，任何一处漏改的表现都是「名字对了、资产全没了」。把显示名与标识分离后，改名只是两份小文件的两个字段，失败可以整体回滚，风险与「改一行设置」同级。这条已登记为 D-055。
2. **写入顺序是「先声明后登记」，这个顺序不能反。** 反过来若登记表先写成功、声明写失败，顶栏立刻显示新名、场景页还是旧名，用户看到的是两份互相矛盾的「真相」。现在的顺序让最坏情况停在「声明已改、登记未改」：界面仍显示旧名，不一致的那份声明有备份可还原，可见、可重试、可手工修。
3. **解析校验做了两遍，第二遍不是多余的。** 第一遍确认用户原有声明是好的（坏了就整体拒绝，不去修别人的文件），第二遍确认自己写出的新文本还能解析、且 `display_name` 回读等于目标值。`with_display_name` 是逐行文本替换而不是 YAML 序列化，第二遍是这条实现路线唯一的正确性保证。
4. **逐行替换而非整份重写是对的。** 用户手写的注释、字段顺序、缩进属于用户资产，改名字不该顺手重排别人的文件（那会让 diff 变成整份重写，也让用户不敢再手改）。`with_display_name` 只替换顶层 `display_name:` 行，缩进行与注释行明确跳过；重复 `display_name` 直接报错而不是猜一个。
5. **字符限制有实际原因，不是洁癖。** `#` 在受限 YAML 子集里是行尾注释的起点，写进去等于把名字后半截注释掉；英文双引号在该子集里没有转义写法，写进去会得到一份解析不回来的文件。含单引号时加双引号，是因为单引号在无引号标量里会被当引用符。用户的正常输入（中文、空格、括号、破折号）都不受影响，越界与多行粘贴分别有明确回执与压缩处理。
6. **出厂 Profile 与未登记目录都拒绝。** 出厂 Profile 的 label 来自 `profile_choice::friendly_label` 的编译期映射，写登记表也不生效，允许改只会让界面与代码不一致；未登记目录是历史遗留，允许改等于给一个「软件里看不见的 Profile」写名字。两条都在写盘之前拦下。
7. **同名直接返回、不写文件。** 用户重复点保存不该产生一份无意义的 `.bak` 备份，也不该触发一次登记表广播。返回 `decl_updated=false` 与「未改动任何文件」的回执，是如实反映而不是假装成功。
8. **界面走行内编辑而不是弹窗。** 设置页既有列表全部是同构行（标题 + 徽标 + 行尾动作），本轮沿用：点「改名」把该行标题换成输入框，行尾变成「取消 / 保存」，Enter 保存、Escape 取消、`maxLength=32`、`autoFocus`。新增的两条样式让输入框直角、与行同高，不撑高整行——弹窗会打断用户对「这是同一行」的认知，也违背本项目既定的直角弹层规范。
9. **事件复用而不是再造一套刷新。** 成功后 emit 既有的 `omni://profile-registry`，顶栏与设置页已有的订阅会重读登记表。125 的教训是「后端做对了、界面还是旧的」等于没做成；本轮没有新增第二个事件，也没有在设置页里手工同步顶栏状态。

**验证证据：** `cargo test --offline --lib rename` 5 passed；`cargo test --offline --lib` **464 passed / 0 failed**（基线 459）；`--tests` 12 个 test binary 全 `ok`、EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npx tsc --noEmit` 与 `npm run build` 通过；debug 构建 EXIT=0；真实窗口 `preview_profile_rename_126.py` **26/26 checks 全 true**、EXIT=0（含 `dsh_dir_unchanged`、`backup_left`、`decl_still_valid`），截图与 `facts.json` 在 `tests/verification/desktop/evidence/od-arch-126/`；回归 125（24/24）、118（全 true）、124（14/14）三个窗口脚本全部 EXIT=0。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍未跑，由用户亲自实测；改名后若用户已在别处缓存了旧显示名（例如某个已导出的对话标题），不会同步更新；并发场景（两个窗口同时改名同一 Profile）未做互斥，最后写入者胜。

**一句提醒：** 改名留下的 `decl.yml.bak-<epoch>` 是刻意保留的回退点，不自动清理。用户确认稳定后可自行删除；若希望软件提供「清理备份」入口，那是一次独立的小改动。


## OD-ARCH-125 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户的产品边界是「OmniDesk 是干净和初始的环境，Profile 是定制的」，本轮要补的是这条边界的可逆性：删掉的 Profile 必须是**能回来的**，而且是用户自己在软件里按一下就能回来。124 之后回执已经写着「可从 .trash 恢复」，可应用里没有那条通道——一句界面承诺配一个不存在的功能，比不写更糟。本轮把通道补上，判定权、落盘时机与安全边界都在本地代码里。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_lifecycle.rs` 的 `remove` / `write_trash_meta` / `read_trash_meta` / `name_from_trash_id` / `list_trash` / `restore` 与 5 个新单测；`lib.rs` 的 `PROFILE_REGISTRY_EVENT`、`profile_trash_list` / `profile_trash_restore` 命令体与 `generate_handler!` 注册；`api.ts` / `types.ts` 的对应封装；`SettingsPanel.tsx` 的回收站区块与恢复流程；`TopNav.tsx` 的订阅 effect；`preview_profile_trash_125.py` 全文与 `facts.json`。`runtime/` 零差异。

**审查判断（几处特意验过的边界）：**

1. **元数据是必要的，不是装饰。** 移除时目录被改名成 `<name>-<epoch>-<pid>`，`label` / `created_at` / `origin` / `plugins` 当场就丢了。没有 `trash.v1.json`，「恢复」只能把目录搬回去、登记写个同名的，显示名与来源信息永久损失。写元数据放在两次 `rename` **之后**是对的：目录没搬成功就不该留下一条「可恢复」的假记录。
2. **元数据写失败不阻断移除。** 每次写盘都 `let _ =`，失败时退回按目录名解析（`name_from_trash_id`）。理由是移除本身已经完成、文件已经在原位，唯一的损失是显示名；为此把「已移除」变成「移除失败」反而制造更糟的状态。
3. **`name_from_trash_id` 的严格性是对的。** 只接受「最后两段都是纯数字」的目录名，其余整条跳过。宁可回收站里少列一条脏目录，也不能把 `my-desk-backup` 之类猜成 Profile 名再恢复出一套错误的目录。5 个单测覆盖了这条边界。
4. **恢复的三道拒绝都在写盘之前。** 出厂名（由清单决定，不是硬编码字符串）、已登记同名、目标目录已存在——任一成立就整体拒绝，一个字节都不动。特别是「目标目录已存在」：目录搬回去会和已有资产合并，那是两个 Profile 的资产被静默搅在一起，属于不可逆错误。
5. **恢复的原子性只做到「不留半套」，这够了。** 两次 `rename` 无法真正事务化，实现方式是第二个失败就把第一个搬回原处。极端情况下（第二个 rename 失败且回滚也失败）会留下「DSH 目录已回来、应用目录还在回收站」，此时登记尚未写回，`list_trash` 仍列着这条（`has_dsh=true` / `has_app=false`），用户可以重试——状态可见、可重入，不是静默损坏。
6. **元数据不进正式 Profile。** 恢复成功后主动删掉目的目录里的 `trash.v1.json`，测试 `no_meta_in_profile` 断言了这点。否则一个曾经被移除过的 Profile 会永久带着一份「我在回收站待过」的残留文件。
7. **`list_trash` 是纯只读。** 不建目录、不写盘、不修元数据；两个 `.trash` 目录不存在时返回空列表。界面在设置页每次刷新都调它，只读是这条命令能高频调用的前提。
8. **事件刷新是本轮唯一的「行为缺陷修复」。** 恢复成功后登记已写回、磁盘也对，但顶栏清单只在挂载时读过一次，弹层里没有恢复出来的条目——用户看到「已恢复」却切不过去，等于没恢复。三处写操作各发一次 `omni://profile-registry`，`TopNav` 订阅后重读，是修这个症状最小且正确的位置。补在前端订阅而不是「恢复后再点一次才刷新」，是因为后者要求用户自己发现清单是旧的。
9. **验收装置的口径问题被逐轮修正，没有含糊过去。** 三轮 `false` 分别是：脚本把设置页正文里的「基础标准」当成弹层已开（判定标志错）、顶栏清单不刷新（真实缺陷）、行内标题与徽标不进无障碍树（WebKit 已知行为）。前两个一个改判定、一个改代码，第三个收窄断言但保留了两条磁盘侧独立证据（`trash_has_item` / `trash_meta_written`），不是把标准放松到「看起来对」。

**验证证据：** `cargo test --offline --lib` **459 passed / 0 failed**（基线 454，新增 5）；`--tests` 12 个 test binary 全 `ok`、EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npx tsc --noEmit` 与 `npm run build` 通过；`cargo build --offline` 与 `--release` 均 EXIT=0；真实窗口 `preview_profile_trash_125.py` **24/24 checks 全 true**、EXIT=0，截图与 `facts.json` 在 `tests/verification/desktop/evidence/od-arch-125/`；回归 118 / 121 / 123 / 124 四个窗口脚本全部 EXIT=0。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍未跑，由用户亲自实测；「清空回收站 / 永久删除」没有入口（本轮刻意不做，需单独设计确认流程）；回收站条目很多时的分页与搜索未做；恢复中途断电或进程被杀后的残局未做故障注入测试，只做了代码路径审查。

**一句提醒：** 回收站只保证「软件内能恢复」，不是备份。`.trash` 与 Profile 目录在同一个数据根下，磁盘损坏或用户清空数据目录时两者一起没；真正的备份仍是 `/home/kevin/OmniDesk/backups/` 与仓库外的成果目录。


## OD-ARCH-124 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户这句话是产品边界，不是功能点：「omnidesk 是干净和初始的环境，Profile 是定制的，切换 Profile 时要能自动写入所需配置、skill 和插件，缺失插件要有自动安装」。109–123 已经把「切换」这一侧做完整了，但**冷启动**这一侧是空的——用户选好纪检专业、关掉应用、第二天再打开，缺的插件不会自己回来。本轮补的就是这条路径，判据与切换完全同源。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_assets.rs` 的 `missing_plugins`（探测口径、标准短路、只读性）、`ensure_plugins` 的文档注释改动；`lib.rs` 的 `maybe_autofill_plugins_on_startup`（四道短路的位置、锁的作用域、探测与起任务的顺序）、`append_profile_assets_log`、`.setup()` 里的接入点、以及是否与 `profile_choice_set` 的既有触发点冲突；`TopNav.tsx` 的 `viewRef` 与事件回调的空状态分支；`SettingsPanel.tsx` 的开关文案；`preview_profile_startup_heal_124.py` 全文（桩的行为、断言口径、清理边界）。`runtime/` 零差异。

**审查判断（几处特意验过的边界）：**

1. **只有「探测」在启动路径上，安装不在。** `maybe_autofill_plugins_on_startup` 只调 `missing_plugins`（纯读），确认缺件后才 `spawn_profile_plugins_autofill` 起后台线程。启动路径上没有任何一次网络调用，也就没有「开个软件先在启动阶段卡十几秒」的风险。窗口实测首启到插件就位之间窗口可交互、日志两行相隔 1 秒。
2. **四道短路都在起任务之前。** 顺序是：通道 → 基础标准 → 开关 → 缺件。任何一道拦下就 `return`，不建目录、不写日志、不起线程。特别要确认的是「基础标准」那道在「开关」之前——即使开关是开的，基础标准也不会走到探测。实测 `base_no_install = true` 且 `base_no_app_profile_dir = true`。
3. **探测与安装用同一张清单。** 两边都走 `required_plugins`，`missing_plugins` 只是把 `probe` 结果过滤一遍。如果各写一份，就会出现「启动说不缺、点了安装又说缺」的分裂——这正是 112 定下的同源原则。
4. **锁的作用域是对的。** 只在开头取一次 `paths.root` / `paths.config` / `engine_supervisor.config()` 的克隆，随后释放；后台线程拿的是自己的路径与 Profile 名。若在锁内起线程或跨线程持锁，界面线程会在安装期间被卡住，而安装要几十秒。
5. **日志写失败不拦功能。** `append_profile_assets_log` 的每次文件操作都 `let _ =`，`create_dir_all` 失败也静默返回。这一点是刻意的：日志是排查工具，不能成为「装不上插件」的新原因。
6. **前端空状态兜底是必要的，不是修饰。** 冷启动自愈发生在用户还没打开 Profile 弹层的时候，此时 `view` 是 `null`；原来的 `return prev ? ... : prev` 让缺件卡永远建不出来，用户只看到一句莫名其妙的「还有 1 个未就绪」。改为用 `viewRef` 补标签建卡，最坏情况用 Profile 名兜底。这条是 124 唯一一处纯前端改动，也是最容易被漏掉的：后端做对了，用户仍看不到发生了什么。
7. **文案改动与行为必须同时改。** 开关标题原写「切换 Profile 时自动补齐缺失插件」，而本轮让它也管冷启动了。只改行为不改文案，等于在界面上留了一句假话。121 的窗口脚本因此改为后缀包含匹配（`自动补齐缺失插件`），新旧标题都能命中——写死整句会让这次文案修正顺手打断一个既有回归。
8. **验收装置的口径问题被正确识别，没有被当成通过。** 第一轮跑出两个 `false`，原因是桩 CLI 也收到了引擎的 `--dump-config` 探测，被算进了「安装调用」。把判定收窄成「以 `plugin ` 开头且含 ` add `」之后重跑，14 项全 true。这是断言口径错、不是行为错，但先前的 `false` 不能直接改口说成通过——所以留了修正记录。

**验证证据：** `cargo test --offline --lib` **454 passed / 0 failed**（基线 451，新增 3）；`--tests` 12 个 test binary 全 `ok`、EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npx tsc --noEmit` 与 `npm run build` 通过（文案修正后重跑，仍通过）；真实窗口 `preview_profile_startup_heal_124.py` **14/14 checks 全 true**，四张截图与 `facts.json` 在 `tests/verification/desktop/evidence/od-arch-124/`；回归 `preview_profile_autofill_121.py` **20/20 两次**（文案改前 47.2 秒、改后 69.2 秒，均 EXIT=0）、`preview_profile_decl_hot_123.py` 31/31 全 true。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍未跑；多窗口同时打开时的事件广播未测；真实 registry 在冷启动路径上的耗时与失败重试形态未单独测（安装通道本身由 121 覆盖）；正式实例是旧进程，需重启才能看到本轮改动。

**一句提醒：** 124 只补了「冷启动缺插件」这一条。若用户接着问「插件装好了，但 Profile 声明的 Skill 在冷启动时缺了会怎样」，那是 122 的巡检口径（只读、不自动补写），本轮没有改动它——装配 Skill 属于 `assemble_profile_assets_on_startup`，冷启动本来就会写。

## OD-ARCH-123 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户的产品边界是「OmniDesk 是干净和初始的环境，Profile 是定制的」。109–122 之后主干已经闭环，只剩两处会让这句话落空的缝：**保存场景声明要重启才生效**（等于「你刚保存的东西没生效」），**基础标准下还能往底座里写声明**（一写就不再干净）。本轮两处都补上了，而且补的位置是对的——热重载只开在「应用内保存」这一条路上，外部手改仍然是只提示、不热重载。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `lib.rs` 的 `PROFILE_DECL_EVENT`、`profile_decl_save`（`app: AppHandle` 注入与 `emit` 时机）、`save_decl_into` 全部（标准短路在哪个位置、大小上限、解析校验、备份、原子写）、`profile_decl_starter` 的三支 `note` 与 `profile_decl_open` 的新口径、命令注册；`App.tsx` 的事件订阅 effect 与 `declDrift` 文案；`api.ts` 的 `onProfileDeclSaved`、`types.ts` 的 `ProfileDeclSavedEvent`；`SettingsPanel.tsx` 的 `declStandard` 状态、提示与按钮 `disabled`。`runtime/` 零差异。

**审查判断（几处特意验过的边界）：**

1. **标准短路确实在写盘之前。** `save_decl_into` 的 `if standard` 是第一句，早于 `create_dir_all`。这一条是本次最关键的可复核点：如果先建目录再拒绝，`profiles/omnidesk-base/` 就会留在磁盘上，「干净底座」的说法立刻不成立。窗口实测二次确认——`base_decl_not_written = true` 且 `base_dir_not_created = true`，来回切一圈后 `base_decl_still_absent_after_roundtrip = true`。
2. **热重载只走应用内保存，052 的边界没被顺手放宽。** `emit` 只出现在 `profile_decl_save` 里；5 秒指纹轮询仍然只置 `declDrift`、不触发 `loadDecl`。这是刻意的：外部手改（编辑器、脚本）如果也热重载，「磁盘上是什么」与「界面是什么」会悄悄脱钩，用户失去判断依据。改动后重跑 `verify_decl_restart_hint.py`，`delete_is_not_hot_reloaded` 与 `user_edit_not_overwritten` 仍然为真。
3. **保存后清掉漂移提示是对的。** `onProfileDeclSaved` 的回调先 `setDeclDrift(null)` 再 `loadDecl()`。少了这句，用户点完保存会看到「文件已在磁盘上修改」的提示——技术上正确（指纹确实变了），体验上等于否认自己刚做的事。这是本轮唯一一处「不需要测也能看出必须如此」的细节。
4. **可测内核的隔离是干净的。** `save_decl_into` 只碰传入目录、不读全局状态，三个单测（标准拒绝且不建目录 / 成功写入并返回 digest / 校验失败一字节不写）不依赖宿主锁与引擎。用内核而不是用命令做单测，避免为了测一条规则去搭整套 Tauri 状态。
5. **前端的标准判据与后端同源。** `declStandard` 用 `choice.standard === choice.effective`，`profileEpoch` 变化即重置，与 `profile_asset_scope` 的「运行中引擎实例的 Profile」是同一件事。前后端判据不一致是这类「按钮禁不禁」最容易出的错——禁用在前端、拒绝在后端，两套判断迟早会分叉。
6. **取模板没有跟着被拦。** 基础标准下仍能看模板、改文字，只是不能保存。这是有意的：模板是纯文本，拦住它没有安全收益，反而让用户不知道「切到 Profile 后能填什么」。
7. **052 的三处改动是装置问题，不是产品缺陷。** 抓图早于首屏渲染（空白窗口）、横带裁切把顶栏时钟框了进去（冷启动与重启后只差 36 像素、全落在 x=1073–1079）、旧断言 `missing_not_recreated_on_launch` 与 109/122 的「出厂 Profile 启动装配补回声明」冲突。三条都在脚本里修，语义没有放宽：真正该守的「运行中手改 / 删除不热重载」仍在前两条断言里，重跑 6/6 全 true。

8. **审查追加：停在「对话分类」页切换 Profile，声明卡不重读（已修）。** 这处是本人复核时从代码路径读出来的，不是用户报的：`SettingsPanel` 的 `profileEpoch` effect 只清缓存，声明卡又没有分页级「打开才读」兜底（分类 / 能力 / 资产 / 规则都有），于是用户停在这一页切换 Profile 后，页面会一直显示空编辑框与「当前 Profile 没有场景声明」——而磁盘上 `decl.yml` 刚被装配写好，界面在说假话。修法是清缓存时同调 `refreshDecl()`。窗口验收补了第 5 段复现该路径，31 项全 true。

**验证证据：** `cargo test --offline --lib` **451 passed / 0 failed**（基线 448，新增 3）；`--tests` 12 个 test binary 全 `ok`、EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`npx tsc --noEmit` 与 `npm run build` 通过；release 重建成功（含 `--features custom-protocol`）；真实窗口 `preview_profile_decl_hot_123.py` **31/31 checks 全 true**（含审查追加的第 5 段），八张截图与 `facts.json` 在 `tests/verification/desktop/evidence/od-arch-123/`；回归 `preview_profile_starter_122.py` 38/38、`verify_decl_restart_hint.py` 6/6、`preview_profile_custom_118.py` 26/26 全 true。

**本轮未覆盖（不作为通过条件）：** 真实模型端到端（OD-ARCH-080）仍未跑；多窗口同时打开设置页时的事件广播未测（当前按单窗口假设）；外部手改文件的「一键应用」未做，仍按 D-051 走「回设置页保存或切换 Profile」。

**收尾自查（同轮追加）：数据目录与会话历史为什么不跟 Profile 走。** 复核时逐项核对「切 Profile 后哪些东西换、哪些不换」，最后一项此前没有定论：`settings.rs` 的 `artifact_root` / `library_root` / `knowledge_vault_root` 各只有一个全局值，`app_paths.rs:102` 的 `threads` 是 `<应用数据根>/threads`（无 Profile 段），DSH 会话按 cwd 归档（实测 `sessions/` 下 7 个目录、无 Profile 维度），引擎只把 profile 名传给 DSH。判定为**有意为之，不是缺陷**：可再生的资产（声明 / 规则 / Skill / 插件 / 分类）随 Profile 装配，不可再生的数据（成果 / 库 / 知识库 / 历史会话）留在机器级；否则切换会造出「磁盘上还在、界面里没了」的假故障。结论写入 `D-052`，含理由、代价与复审触发。本轮因此没有代码改动，也未重跑构建与窗口脚本。


## OD-ARCH-122 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户这轮要的边界很清楚：「OmniDesk 是干净和初始的环境，Profile 是定制的；切换 Profile 时要能自动写入所需配置、Skill 和插件，缺失插件要能自动安装」。121 之后主干已经闭环，122 收的是两个「自建 Profile 才有的缝」——体检看不见 Skill、从基础标准起手没有工作台骨架。两处都补在正确的位置，没有把定制能力塞进 OmniDesk 本体。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_assets.rs` 的 `inspect_custom`（Skill 巡检段、`repo_missing` 赋值、只读性）、`profile_starter.rs` 全部（`clean_label` / `decl_text` / `rules_text` / `write_starter` / 4 个单测）、`profile_lifecycle.rs` 的 `create()` 起手分支与 `files` / `starter_error` 处理、`lib.rs` 的 `profile_decl_starter` 与命令注册、`types.ts` / `api.ts` / `SettingsPanel.tsx` 的 `fillStarterDecl` 与按钮。`runtime/` 零差异。

**审查判断（几处特意验过的边界）：**

1. **巡检确实只读。** `inspect_custom` 只在内存里拼 `AssetItem`，没有 `create_dir_all`、没有 `save_state`。窗口实测二次确认：把 `rules.json` 绑上 `memory-agent` 并删掉目录后，资产页报「缺失」，而 `dsh-home/skills/memory-agent` 仍不存在。这条很关键——「看一眼就改盘」是设置页最容易犯的错，109 定下的「巡检不写入」没有被本轮破坏。
2. **巡检与装配用同一张清单。** 两边都走 `wanted_skills`，`inspect_custom` 的 `repo_missing` 也从固定 `None` 改成真实返回。这避免了「装配说齐了、体检说缺了」的分裂——正是 112 定下的同源原则。
3. **模板先自校验再落盘。** `decl_text` 生成后跑 `parse_decl` + `validate`，`rules_text` 生成后跑 `parse_rules`，写不出来就返回错误而不是写半成品。显示名压单行、截 64 字、空值兜底「我的工作台」，与声明校验同口径。单测 `decl_template_flattens_multiline_label` 钉住了换行会破坏 YAML 这条真实风险。
4. **永不覆盖是硬约束。** `write_starter` 两个文件分别判存在，已存在就跳过并在返回值里不列出。单测 `write_starter_never_overwrites` 先写一份手改内容再调一次，断言手改内容还在。这对「用户已经改过声明、又从界面点了一次模板」的场景是必要的。
5. **零预注入是真的零。** 起步 `rules.json` 的 `global`/`scenes`/`skill_bindings` 三块全空，窗口实测切过去后回执是「资产已就绪（0 项）」、Dock 上的场景也没有任何 Skill 绑定。符合 108 定下的「场景绑 Skill 是最低要求，其余交给模型按需加载」，也符合用户「省 token、快回复」的要求。

6. **基础标准零写入没有破。** 三个入口都守住：`wanted_skills` 对 standard 返回空、`inspect` 对 standard 直接返回空报告、`write_starter` 只由 `create(None)` 与用户点击触发。窗口实测冷启动 `engine_profile` 为 `null`，创建后仍为 `null`（创建不切换）。
7. **创建失败不假装成功。** 起手骨架写失败只记 `starter_error`、不阻断创建，回执里如实说明；这与复制路径「decl/rules 复制失败不阻断」同口径，不会出现「Profile 建好了但界面说失败」或反过来的错位。
8. **设置页按钮的语义是对的。** 「填入起步模板」只改内存编辑框，不落盘，仍要用户点「校验并保存」（走校验与备份）。没有做成「一键生成并保存」，避免用户没看清就覆盖自己的声明。
9. **验收装置的两处误判是真问题，但不是产品缺陷。** 一是 WebKit 不把资产行暴露给 AT-SPI（118 轮同类问题），二是「缺失」二字与页面按钮「修复缺失 / 恢复出厂内容」撞车。都已在脚本里改用可稳定取到的信号，并把「行内容靠截图」这一点写进 STATUS。判据下调没有被包装成「测过了」。

**验证证据：** `cargo test --offline --lib` **448 passed**（基线 443，新增 5）；`--tests` 12 个 test binary 全 `ok`、EXIT=0；`pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 41/41；`npx tsc --noEmit` 无错误；`npm run build` 通过；release 重建 3m02s 成功（含 `--features custom-protocol`）；真实窗口 `preview_profile_starter_122.py` **38/38 checks 全 true**，六张截图与 `facts.json` 在 `tests/verification/desktop/evidence/od-arch-122/`。

**本轮未覆盖（不作为通过条件）：** 「填入起步模板 → 校验并保存」的完整保存链路只验到按钮可点与提示出现，保存动作本身由 051 与单测覆盖；真实模型端到端（OD-ARCH-080）仍未跑；已在用的旧自建 Profile 不会自动补骨架（这是有意的设计取舍，见 PLAN 的「明确不做」）。

**补充回归（审查后追加）：** 本轮改了自建 Profile 的默认 Dock，因此重跑了 `preview_profile_custom_118.py`，**26/26 checks 全 true**（`evidence/od-arch-118/`）。该脚本原先写死「复用已有 dev server」，本机无服务时直接抛错，已改为先探测再决定复用或自起；这是装置问题，改后原有 24 项判断语义未变，另加了两项针对新骨架的断言（Dock 有起步场景、`rules.json` 三块全空）。该脚本同时确认了删除仍进回收站可恢复（`trash_entries` / `dsh_trash_entries` 各有 1 条）。


## OD-ARCH-121 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 用户这一次要的是「OmniDesk 干净、Profile 定制」：切到自定义 Profile 时，配置、Skill、插件都应自动就位。109–120 已经让前三项自动，唯独插件因为要联网、被有意留成手点。本轮把插件接进切换流程，同时把「联网会拖死切换」这个当初不自动的真实理由解决了——切换仍然秒回，安装放到后台，并且有上界。

**独立复核范围：** HEAD `7820751` + 工作区改动。逐行核对 `profile_assets.rs` 的 `InstallRun` / `run_install_command` / `run_command_with_timeout` / `collect_pipe` / `drain_pipe` / `kill_tree`、`install_plugin_with_timeout` 及其重试分支，`settings.rs` 的默认值与读写函数，`lib.rs` 的 `should_auto_install_plugins` / `spawn_profile_plugins_autofill` / `profile_assembly_prefs(_set)` 与调用点，`TopNav.tsx` 的事件订阅与回执位置，`SettingsPanel.tsx` 的开关与事件刷新。`runtime/` 零差异。

**审查判断（几处特意验过的边界）：**

1. **自动安装确实不在切换的同步路径上。** `spawn_profile_plugins_autofill` 在 `profile_choice_set` 返回之前只是起任务，不 `await`；联网安装最慢的一段（实测整轮 114.8 秒里绝大部分）发生在响应之后。用户点「确认」到界面换场景是立刻的。
2. **只杀进程不够，必须收进程组。** 这是本轮最值得记的一条：单测实测原始写法收尾要 30.006923168 秒，因为 `sh` 死了孙进程还攥着管道。`process_group(0)` + `killpg(SIGKILL)` + 2 秒排空宽限期，把「装完了但界面卡住」这类最难复现的问题挡在单测层。非 Unix 分支退回 `child.kill()`，没有为了跨平台引入新依赖。
3. **超时不冒充成功。** `InstallRun.timed_out` 单独成字段，`success()` 只认 `!timed_out && exit_code == 0`；超时也不走进「pnpm 拦了 → 放行重试」的分支（那条只对真实 `Ignored build scripts` 生效）。失败就如实写进回执与界面。
4. **开关默认开是对的，但要能关。** 用户本次的意图就是自动；默认 `true` 符合预期。同时保留关闭能力，关掉后行为与 114 完全一致（只提示、等用户点）。`serde` 默认让旧设置文件直接可用，避免了「升级后设置读不出来」的迁移坑。
5. **基础标准恒不安装有两道锁。** `required_plugins` 对 standard 返回空表，`should_auto_install_plugins` 再要求目标不等于 standard。单测 `standard_profile_never_installs` 钉住了这一点——「干净底座」是产品承诺，不能靠单点判断。
6. **后台任务不碰宿主状态。** 任务只带自己的 `app_root` / `dsh_home` / `profile` / `standard`，运行期不再取宿主锁；结果通过事件回传。避免了「后台安装时用户又切一次 Profile」时的锁竞争与串味。
7. **窗口验收的「全 false」是验收装置问题，已定位而非绕过。** 探测的第一个候选是 CLI 自己的 `node_modules`，仓库运行时里本来就有同名包，缺件恒为 0。用与 120 联网测试相同的「转发型锚点 CLI」修好装置后，20 项 check 全 true，安装仍走真实 CLI 与真实网络。这条如果不查清，很容易被误判成「功能没生效」而去改本来正确的代码。
8. **回执位置是真缺陷，已修。** 首轮截图里回执与弹层正文互相压字、右端被窗口裁掉。改为弹层内显示、关层时挂徽标下方，并补 `aria-label`。顺带解决了无障碍树里拿不到纯文本段落的问题——这既是可访问性改进，也让后续脚本断言更可靠。

**验证证据：** `cargo test --offline --lib` **443 passed**（基线 435，新增 8）；`--tests` 集成 2/1/13/6/5/10/21/4/3/1 全绿；`OMNIDESK_NET_TESTS=1` 联网端到端 1 passed（8.17s，重跑不重装）；`pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 41/41；`npx tsc --noEmit` 无错误；`npm run build` 通过；release 重建 3m03s 成功（含 `--features custom-protocol`）；真实窗口 `preview_profile_autofill_121.py` **20/20 checks 全 true**，证据在 `tests/verification/desktop/evidence/od-arch-121/`。

**遗留（不作为本轮问题）：** 真实模型端到端（OD-ARCH-080）仍未跑；网络故障注入（断网 / 5xx / 中途被杀）只有单测覆盖；插件只在切换那一刻自动补，事后手动删目录仍需再切或手点安装。


## OD-ARCH-120 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 这一轮补的是「最后一公里」：119 之后探测已经说得清缺什么，但从来没人点过一次安装并真的装成。实跑一撞就是两层阻断，修法对准的是根因，不是把报错藏起来。

**独立复核范围：** HEAD `7820751` + 工作区改动。核对 `profile_assets.rs` 新增的 `dsh_runtime_version` / `install_version` / `relax_dep_builds` / `needs_build_relax`、`install_plugin` 的重试分支、3 条新单测、`tests/profile_assets_host.rs` 的联网端到端测试，以及真实 CLI 输出。`runtime/` 零差异。

**审查判断（几处特意验过的边界）：**

1. **版本必须钉在运行时，不能钉在 registry 默认。** 实测裸包名会被 DSH 的 peer 兼容检查拒掉（`0.1.6-alpha.2` vs `0.2.1-alpha.2`），而探测出的清单通常没写版本。`install_version` 的「显式优先、否则运行时」正合这个顺序：用户显式要求就尊重，没要求就跟随引擎。
2. **`dsh_runtime_version` 只认 `@deepseek-ai/dsh` 自己。** 向上找 4 层是够用的（`lib/bin.js` → `lib` → 包根），并且用 `name` 校验挡掉了外层 aggregator 的版本号。少了这道校验，某些布局下会静默拿到错误版本，比报错更难查。
3. **构建脚本放行改的是 strict 开关，不是逐包白名单。** 逐包 `allowBuilds` 要预先知道包名，而缺失插件的依赖树装之前算不出来；改 `strictDepBuilds: false` 是唯一能在非交互通道里一次生效的做法。它只影响「脚本没被批准」这一个判定，不改包内容，也不动 `runtime/`。
4. **必须同时看 stdout。** 第一次实现只看 stderr，重跑仍然退出 1——pnpm 把拦截报告写在 stdout。这类「改了但没生效」的假绿灯，只有真跑一次才能发现；本轮确实靠真跑发现了，并已修正。
5. **写入要幂等且保留原有键。** `relax_dep_builds` 逐行扫描替换，已存在就不重复追加，空目录补最小 `packages` 段。单测 `relax_dep_builds_keeps_existing_keys` 钉住了「不删 `nodeLinker`」「只留一处 `strictDepBuilds`」。用户在 `pnpm-workspace.yaml` 里加的其它设置不会被这次放行冲掉。
6. **联网测试必须默认关、能证明整条链路。** 它走的是 `ensure_plugins` → `install_plugin`，不是裸 CLI；用转发型 CLI 让探测只看隔离根，避免「运行时自带同名包就判就绪」的假通过。测试同时断言第二次不再重装——只证明「装上了」不够，还得证明「不会每次都重装」。
7. **「裸 node import 失败」被正确排除。** DSH 启动器是双锚点解析，裸 node 只有 Profile 一处锚；用真实 CLI 起 Profile 25 秒无报错。把它当缺陷会去复制依赖、反而破坏 DSH 的解析模型。

**遗留（不作为本轮问题）：** 真实模型端到端（OD-ARCH-080）仍未跑；切换 Profile 时不联网安装仍是有意的设计边界。

**验证证据：** `cargo test --offline --lib` 435 passed；`--tests` 集成全绿（2/1/13/6/5/10/21/4/3/1）；`OMNIDESK_NET_TESTS=1` 联网测试 1 passed；`pytest` 89 passed / 30 subtests；`verify_profile_assets_109.py` 41/41；真实窗口 `preview_profile_custom_118.py` 26/26。
+
## OD-ARCH-119 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 118 的实跑回执自己把一个真实缺陷带了出来：从「基础标准」新建的 Profile，资产页显示「一键安装缺失插件（2）」，而这两项是同一包里的子路径导出，`install_plugin` 的两段式校验必然拒绝。用户看到的是「系统说我缺插件，点安装又报名字不合法」——这是能被任何新用户碰到的死角。

**独立复核范围：** HEAD `7820751` + 工作区改动。核对 `profile_lifecycle.rs::detect_plugins` / 新增 `package_root`、`profile_assets.rs::required_plugins` 的归一位置，两条新单测，以及真实窗口前后对比。`runtime/` 零差异。

**审查判断（几处特意验证过的边界）：**

1. **修复位置必须选在探测口径，而不是补丁文件。** `name: '@deepseek-ai/dsh-tool-subagent-control/list-agents'` 在 DSH 侧是合法的子路径模块入口，改它会改掉引擎的真实加载行为。缺陷在于「OmniDesk 把它当 npm 包名去探测」，所以修的是 `detect_plugins` 与 `required_plugins`，补丁一个字没动。
2. **两个来源都要归一，漏一处就等于没修。** 子路径既可能来自 `package.json` 的 `dependencies`，也可能来自补丁的 `name:`；修复覆盖了两处，且对依赖键也做归一——只补丁那一处的话，用户用 DSH 装过子路径依赖时同样会复发。
3. **旧登记表也要能自愈。** `required_plugins` 读登记表时再归一一次。已经写进 `index.v1.json` 的子路径不会因为 upgrade 而消失，重新探测时会自然纠正；这一点如果只是「以后新写的才对」，老用户会一直卡着。
4. **归一化的边界必须窄。** 只折叠 scoped 的三段及以上；`@a/b` 保持原样，非 scoped 的 `plain/sub` 也原样返回、交给原有的合法性判断。否则可能把一个本该被拒绝的名字悄悄改写成另一个包，变成静默装错东西。`package_root_only_strips_scoped_subpaths` 正好钉住这条边界。
5. **排序去重是必要的附带修正。** 原实现按文件书写顺序保留，同一 Profile 在不同机器上可能给出不同顺序的清单，界面上「N 个缺失插件」的排列会漂。归一后统一 `sort + dedup`，结果稳定。

**独立实跑结果（本人执行）：** `cargo test --offline --lib profile_lifecycle` **10 passed**；`--lib` **432 passed**；`--tests` 集成 2+1+13+6+5+10+**20**+4+3+1 全绿；`pytest` **89 passed / 30 subtests**；`verify_profile_assets_109.py` **41/41**；真实窗口 `preview_profile_custom_118.py` **26/26 true、EXIT=0**。

**真实窗口前后对比（同一脚本、同一动作）：** 修复前资产页有「一键安装缺失插件（2）」和两颗「安装」按钮，`plugins` 为两项子路径；修复后这两处整段消失，`plugins` 为 `[]`，26 项断言仍然全绿——说明这次不是靠放宽断言变绿，而是缺陷本身消失。

**已知限制：** 真实联网安装插件仍未实测；真实模型端到端仍未测；`worklog.rs::AppendReceipt::failure` 的 dead_code 告警是既有的，不在本轮范围。

**测试环境清理：** 5273 预览服务、Xvfb、`/tmp/od119-*` 与 `/tmp/vite119.log` 已清理；正式实例未动。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。

## OD-UI-202 第一阶段审查结论（ARCHITECT，2026-11-08）——通过

**结论：通过。** 用户选定“品牌靛 + 先做顶栏/首页”后，本轮把新拟物材质落在共享顶栏、首页输入区、Dock 与窗口控制；业务逻辑、IPC、数据模型、权限、模型路由与固定/滚动基线均未改。第一版曾把整幅对话区包成一块 `raise-3` 大面板，真实窗口空态下像巨大空白卡片，已撤掉该包裹，只保留导航、Dock、输入与消息的层级。

**复核范围：** 当前 HEAD `38d839ce2d87887a59c1dec41c141f9a08dee8f2` + 工作区改动。实读 `desktop/src/theme.css`、`App.tsx`、`shell/TopNav.tsx`、`WindowChrome.tsx`、`Dock.tsx`、`HomeChat.tsx`、`CategoryPicker.tsx`、`ModelPicker.tsx`、新增验收脚本与 `docs/brand-and-ui/` 规范/预览。并行中的 `shell/TaskManager.tsx`、Rust 任务/排期文件与设置页改动不属于本轮，未纳入本次结论，也未修改。

**实际复核证据：**

| 验证 | 结果 |
| --- | --- |
| `npm run build`（tsc + vite） | 通过；仅既有 >500 kB chunk 提示 |
| `npx tsc --noEmit` | 通过，无输出 |
| `python3 tests/verification/desktop/verify_ui_neumorphism_202.py` | **36/36 通过**，JSON 证据 `tests/verification/desktop/verify_ui_neumorphism_202.json` |
| `python3 tests/verification/desktop/verify_home_refine_202.py` | **29/29 通过** |
| macOS debug App 重建后真实窗口 | 1280×820 目视核对：暖白灰底材、凸起顶栏、凹陷搜索/Profile、凸起日期/设置/通知/窗口按钮、Dock 当前项凹陷 + 品牌靛标记、输入区凹陷、发送钮凸起 |
| 键盘焦点 | Tab 到 Dock 项与搜索框时可见 2px 品牌靛焦点环；搜索框另有 `focus-within` 2px 环 |
| 静态边界 | `TopNav.tsx` / `CategoryPicker.tsx` / `ModelPicker.tsx` 已无 `backdrop-blur`；顶栏无玻璃底色与白色 ring |

**审查判断：**

1. **顶栏材质成立。** `neu-nav` 使用左上高光 + 右下暖灰暗影，搜索与 Profile 压入，日期/设置/通知/窗口按钮凸起；弹层改实色 `neu-popover`，不再叠模糊。结构、事件、Profile 切换、搜索、日期弹层、窗口控制与 `data-testid` 均原样保留。
2. **首页不再全平。** 输入区 `neu-composer` 为凹陷面，发送钮凸起；用户消息保留品牌靛填充并加轻凸阴影，助手消息用浅面 + 左缘品牌色，未把每条消息都做成厚卡片。空态没有额外整幅面板，避免了“巨大空白卡片”。
3. **Dock 与窗口。** Dock 去掉玻璃胶囊容器，入口为独立凸起圆钮；当前项凹陷并保留 3px 品牌靛标记。窗口按钮凸起，关闭按钮悬停仍为浅红危险色。
4. **可访问性。** 焦点环 2px、对比 `#3525cd` / `#edecea` = 7.74:1；主文字 `#131b2e` / 底材 = 14.53:1，次文字 `#464555` = 7.93:1。搜索与输入框占位文字从过淡的 `outline-variant` / 60% `outline` 提到 `on-surface-variant` 的 70%–75%，避免新凹陷面里看不清。`text-outline`（3.8:1）仍用于非必要分隔/辅助信息，未扩大使用范围。
5. **范围克制。** 本轮只改外观与材质，未改信息架构、布局、路由、IPC、数据模型或功能逻辑。工作记录、文案写作、项目跟踪、设置、知识库的 DOM 卡片通过全局映射统一到暖面与新拟物阴影；知识图谱等 canvas 内容保持原图形实现。
6. **中文界面字体（2026-11-09 修订）。** 用户明确否决宋体界面，改为统一黑体：大标题思源柔黑，界面标题、正文、控件、表格用思源黑体；行楷只做标语，新对话空态标语使用 700 加粗；方正楷体只留给长引用/落款，思源宋体只用于导出报告和打印文档。英文仍 Inter / IBM Plex Mono；`--font-slogan` 保留 Inter 优先，避免混排英文被手写体覆盖。等宽/标签字体（IBM Plex Mono、`font-mono`、`font-label-caps`）都显式接中文黑体，日历中文标签由 9px 提到 10px，堵住中文回退宋体的最后一条路径。演示夏行楷已按 SIL OFL 1.1 随包自托管（Slidexiaxing-Regular.ttf 与 OFL-SlideFont.txt），不再依赖系统安装；思源黑体系仍只引用字体名。
7. **全应用材质映射。** 在 `theme.css` 用无层选择器把旧页面的 `bg-white` / `bg-surface-container-*` 统一到暖灰表面，把 `shadow-*` 与旧任意阴影统一到成对新拟物阴影，移除全部残余 `backdrop-blur`，输入框统一压入，主按钮抬起并在按下时压回。设置页主卡片、胶囊、输入、按钮、提供商列表与记录行同步材质。该层只覆盖外观，不改路由、布局、IPC、数据模型与功能逻辑。
8. **首页细节修订。** “欢迎回到”字样移除，问候区预留最新提示；新对话空态加入自托管演示夏行楷标语“坚持学习人生将有无限可能”并加粗到 700；分类、模型、Profile 下拉的说明段落移除；模型下拉收窄到 248px，Profile 弹层 190–250px，归类菜单 176px；日历标题改为 ISO 全年第几周；搜索焦点蓝线全部移除；思考等级改为凸起按钮组；AI 回复头像改为模型首字母，用户提问头像改为 OmniDesk 小 logo。新增 `verify_home_refine_202.py` 30/30。
9. **对话滚动与权限提示。** 取消生成中按消息增量强制滚底，只在切换线程时落底；新增“跳到最新对话”下滑箭头。生成中占位改为“正在生成…”，不再把 busy 误判为权限未开启。等待超过 120 秒或工具名未知时，消息内提供停止本轮入口；引擎工具卡住仍按真实状态暴露。
11. **Neo-Skeuomorphism 2.0 校准。** 采用象牙灰 `#F0EFEC`、凸起面 `#F8F8F6`、内凹 `#E8E7E3`、近黑偏蓝 `#34343B`、品牌强调 `#5747F8`；顶栏 34px、输入面板 24px；空态标语 30px、0.12em 字距。ChatGPT 建议的 `#85858C` 对比不足，只作装饰，正文次文字用 `#5F5F67`。文档新增“Neo-Skeuomorphism 2.0 / Calm Intelligence 校准”章节。

12. **首页 2.0 版式复刻（2026-11-10）。** 按用户参考图重排首页：顶栏 30px 圆角；问候带系统用户名；四张入口卡片（开始对话 / 打开知识库 / 进入应用 / 我的空间）；标语两侧分隔线；输入面板 26px 凸起带光晕；底部真实场景快捷入口；Dock 收进胶囊导轨；背景为暖白渐变 + 右上淡蓝山海插画（`desktop/public/home-scape.png` 裁自用户参考图，仅首页显示）。新增只读 IPC `desktop_user_name`。逐项核对：卡片与弹层只指向 `knowledge/settings/声明场景`，没有「日程/收藏/更多」这类未实现入口；插画只在 `scene === "home"` 挂载；未加设计规范条、未实现深色模式。复跑 `verify_ui_neumorphism_202.py` 36/36、`verify_home_refine_202.py` 40/40，`tsc` 与 `npm run build` 通过，debug `.app` 重建后实机截图核对。

13. **首页二次修订与 v0.2.18 发布（2026-11-10）。** 按用户当轮意见：四张入口卡片、说明句与手掌表情全部移除；底部胶囊改为最近五个项目（`shell.projectList()` 取更新时间最新的 5 个，点击进项目跟踪，无项目时只留「项目跟踪」入口）；新增 `conversationStarted`，线程已有消息或首条消息落盘后收起问候区。发布工作流改为只出安装包（macOS universal dmg + Windows NSIS），不再组装便携 zip。证据：`main` 已推送至 `6a0b7bd`、`b42daea`；Release 运行 42773475862 成功（macos 15m15s、windows 15m27s）；`v0.2.18` 资产仅 `OmniDesk_0.2.18_universal.dmg`（162 MB）与 `OmniDesk_0.2.18_x64-setup.exe`（96 MB）。静态检查复跑 36/36、40/40。

10. **P2 状态观察与 P3 思考档。** 任务事件在流式过程中持久化 seq/last_event_at（工具/启动立即、增量最多 2 秒一次），TaskStatus 暴露 last_event_at；UI 收到首条真实事件将 starting 提升为 running，状态查询失败但 30 秒内仍有真实事件时不误报 unknown；重新查询先 recoverTask。ModelPicker 对未声明的思考档显示“可能被忽略”。验证：test_task_status_snapshot 1/1、test_web_api_unit 89/89、verify_home_refine_202 29/29、verify_diagnose_omnidesk 12/12。

**已知限制与未验证：**

- 只在 macOS debug 窗口目视核对；未在 Linux/WebKitGTK 或 release 包上量测阴影重绘成本。按规范已限制 L3 数量并禁止动画化阴影，但未做帧率采样。
- CUA 截图 API 不回写文件，本轮真实窗口截图只存在于会话工具记录，未复制到 `tests/verification`；静态检查 JSON 已落盘。
- 构建中途曾被并发 OD-QS 的 `TaskManager.tsx` 未使用导入阻塞；等对方修复后本会话复跑 `npm run build` 通过。该文件不在本轮 diff 内，不能算本轮成果。
- 本机字体清单只有 Songti SC / STSong / Heiti SC；未找到思源柔黑、思源黑体，真实窗口回退系统黑体，柔黑待装字体。演示夏行楷已按 SIL OFL 1.1 随包自托管，行楷实际字形待窗口目视确认。
- 全应用材质映射后，本轮 CUA 截图工具不可用，未逐页真实窗口目测；完整视觉验收待用户打开工作记录、文案写作、项目跟踪、设置、知识库逐页确认。静态检查与构建已通过。
- 全应用外观映射已覆盖 DOM 卡片与控件；知识图谱等 canvas/图形内容没有重绘，仍沿用原图形颜色。布局、间距与页面结构本轮未改。

**未提交未推送：** 按批准范围只执行第一阶段，工作区原有改动与并行任务改动原样保留。


## OD-ARCH-118 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 109–117 一直在证明「在出厂 Profile 之间切」。用户真正要的能力是**建一个自己的工作台**，这条链路此前只有 Rust 单测、没有一条真实窗口证据。本轮补上：从界面新建 → 立刻切过去 → 资产页看得见 → 切回底座 → 移除进回收目录，**26/26 全 true**。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104–117 未提交部分）。本轮改动**只有验收脚本**，零产品代码：新增 `tests/verification/desktop/preview_profile_custom_118.py`。逐项核对脚本的断言是否真的能区分「产品对」与「产品错」，以及三轮修正是否都是在改口径而非掩盖缺陷。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**审查判断（几处特意验证过的边界）：**

1. **「建完立刻删」是不存在的路径，脚本必须尊重这条业务约束。** `lib.rs::profile_remove` 对当前在用的 Profile 直接拒绝；界面上那颗「移除」在 `inUse` 时也是 `disabled`。所以第 4 步必须先从界面切回「基础标准」再删。脚本第 1 轮的 9 项全红恰好卡在这里——这说明断言顺序确实反映了真实操作序列，没有为了让脚本变绿而去绕开产品约束。
2. **共用定位器不该为一条新路径扩容。** 自建 Profile 的徽标写的是自定义显示名，`open_profile_popover` 只认「标准 / 专业」。修法是在 118 内新增 `open_popover_any`，而不是改 110 的共用函数——110 的路径只用出厂文案，动它只会扩大回归面。新函数额外要求「弹层真的展开」（`popover_open`）才返回成功，因为资产页里也有同名条目，只凭「点了有反应」会误判。
3. **「还在列表里」不能用一行文本证明。** 第 3 轮 `custom_still_listed` 假红，根因是 WebKit 不把资产行标题的 `span` 文字放进 AT-SPI 树；先前命中的 `My Desk` 其实来自当时还没切的顶栏徽标。改成「行内『移除』按钮可见」+「注册表仍有该条目」，两处证据互不依赖，才真的证明资产页还列着它。这里如果把断言放松成「截到图就算过」，下次列表真没了也会绿。
4. **回收而非删除必须验到磁盘。** `remove_is_recoverable` 查的是 `app-data/profiles/.trash/my-desk-<epoch>-<pid>` 真在盘上，`removed_dsh_dir` 查的是 `dsh-home/profiles/my-desk` 真没了——配置可恢复、引擎侧不残留，两个方向都验了。
5. **干净底座起手这条前提被真验了。** `boot_base` 用 `OMNIDESK_DSH_PROFILE=""`（环境变量优先级高于设置文件），配合 `create_carries_no_skills`（`skills/` 为空）与 `custom_categories_plain`（只剩 `plain`），说明新建出来的确实是「干净工作台」，不是把当前业务 Profile 的东西悄悄带过去。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `preview_profile_custom_118.py` **26/26 true、EXIT=0**；`python3 -m pytest tests -q` **89 passed / 30 subtests**；`verify_profile_assets_109.py` **41/41 passed**。实跑 facts 中 `trash_entries` 与 `dsh_trash_entries` 同为 `["my-desk-1793247214-118499"]`，`removed_dsh_dir` 为 true。

**测试环境清理：** 5273 预览服务（npm / sh / node 三层进程）、Xvfb、`/tmp/od118-*` 与 `/tmp/vite118.log` 全部清理；正式实例 pid 107733 / 引擎 107812 未动。

**已知限制（不作缺陷计）：** 停放区只读、无手工归位入口（靠切回 Profile 触发）；分类表只在 `create` 时复制，之后两边各管各的；分类表只在切换 Profile 时同步，手改 `categories.json` 需再切一次或重启；既有线程 tag 不迁移，切走后回落「未分类」是设计；真实联网安装插件未测（`my-desk` 探测出 2 个缺失插件，未点安装）；真实模型端到端仍未测（OD-ARCH-080 清单待执行）；`worklog.rs::AppendReceipt::failure` 为既有 dead_code 告警；本机 `$HOME/OmniDesk/runtime` 无 `dsh-runtime`，正式实例依赖 `OMNIDESK_DSH_CLI`。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。


## OD-ARCH-117 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 116 把分类表做成 Profile 资产，但只接了「切换」这一条路。本轮补上两条漏网：复制 Profile 时代上分类表；被停放的 Skill 在设置页看得见。两条都是「用户切一下就发现不对」的位置，不是锦上添花。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104–116 未提交部分）。逐项核对 `profile_lifecycle.rs::create` 的分类复制分支与取值优先级、`profile_assets.rs` 的 `ParkedSkill` / `list_parked`、`lib.rs::profile_assets_parked` 与注册、`api.ts` / `types.ts` / `SettingsPanel.tsx` 的接线与 `profileEpoch` 清缓存。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**审查判断（几处特意验证过的边界）：**

1. **复制时不能直接搬文件。** `categories.json` 里有 `profile` 字段，`parse` 会以「归属不符」拒绝。实现走 `profile_categories::save` 重新序列化，写出去的就是新名字。测试 `create_from_profile_copies_categories` 同时断言 `profile_categories::load(app, "clone")` 读得出来、原始 JSON 里含 `"clone"`——只断言文件存在会漏掉这个坑。
2. **「来源正在生效」这一档必须存在。** 盘上的 `categories.json` 是**上次切走时**的快照。用户在当前 Profile 里改过分类、加过 `custom:*`、还没切走，此时从它复制出的新 Profile 会少一截。实现先看 `settings.engine_profile == Some(from)` 取当前生效表，再看文件，最后看内嵌清单；三档都失败才放弃（不阻断创建）。
3. **只读清单不能顺手建目录。** `list_parked` 用 `fs::read_dir` 失败即返回空表，不 `create_dir_all`；单测同时断言「看一眼之后停放目录仍不存在」。基础标准态必须能做到零写入，这是 115 定下的口径，本轮不能破。
4. **停放不提供删除入口。** 115 的语义是「移走不删除」，切回需要的 Profile 自动归位。界面上写清这一句就够，给删除按钮等于把「可逆的收敛」变成「用户可能永久丢技能」。单测 `list_parked` 也跳过点号开头的临时项（`.bak-*`），不把备份当 Skill 列出来。
5. **失败不阻断创建的取舍是有意的。** 分类表复制失败时 Profile 仍然创建成功，走 116 的「继承一次」兜底。与 `decl.yml` / `rules.json` 同口径——一个可选资产写不进去，不该让整个创建操作回滚。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `cargo test --offline --lib profile_lifecycle` **8 passed**；`cargo test --offline --lib profile_assets` **8 passed**；`cargo test --offline --lib` **430 passed**；`cargo test --offline --tests` 集成 2+1+13+6+5+10+**20**+4+3+1 全绿；`npx tsc --noEmit` 无输出；`npm run build` 通过（仅既有 chunk 体积提示）；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` **41/41**（新增 3 条）。

**真实窗口核对：** `preview_profile_switch_110.py --display :139 --root /tmp/od117-preview` 一次性 Xvfb + 隔离根，**16/16 checks 全 true**。切回基础标准后 `skills_after_base` 为空、`parked_skills` 列出 8 个业务 Skill——停放区里确实有东西，界面现在能把它说清楚。

**停放区在真实窗口里的可见性（补测）：** 标准轮预置 `app-data/profiles/.parked-skills/parked-demo` 后跑 `preview_profile_assets_109.py`，两轮 9/9 + 6/6 全 true，新增 `parked_item_preserved` 证明「打开资产页读一次」不改动停放内容。正文文本本身**读不到**：WebKit 在这台机器的 AT-SPI 树里对该卡片正文暴露不稳定（同一张截图里能看到「已停放 1 / parked-demo」，树里却没有），与 113 记过的设置页正文现象同源。因此界面可见这条按「截图人工目视 + `parked_skills_visible_in_ui` 静态接线」判定，没有把它写成自动断言——把截到图当文本断言会被误读成回归。

**已知限制（不作缺陷计）：** 停放区只读、无手工归位入口（靠切回 Profile 触发）；分类表复制只在 `create` 时发生，之后两边各管各的；真实联网安装插件与真实模型端到端仍未测。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。


## OD-ARCH-116 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 115 之后「干净底座」在配置、场景、规则、Skill 四层都成立，只剩分类表这一层没跟上：切回 `omnidesk-base`，首页胶囊、历史会话的分类下拉、设置页的分类卡片仍是三个业务分类。本轮把分类表做成 Profile 的资产，切走回存、切入装载，基础标准只留「未分类」。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104–115 未提交部分）。逐项核对 `settings.rs` 的 `REQUIRED_CATEGORY_KEY` / `standard_thread_categories` / `validate_categories` / `Settings::default` / `thread_categories` 的 serde 默认；新模块 `profile_categories.rs` 的 `load` / `save` / `parse` / `embedded_for` / `apply_for_switch`；`lib.rs::profile_choice_set` 的接线位置与回滚口径；`App.tsx` / `SettingsPanel.tsx` / `threadCategories.ts` 的前端接线；`samples/profiles/discipline.categories.json` 的内容。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**审查判断（几处特意验证过的边界）：**

1. **同步必须在写 `engine_profile` 之前。** 若把 `apply_for_switch` 放在 `set_engine_profile` 之后，分类失败时 Profile 已经换了，界面就会出现「工作台是纪检的、分类是旧的」这种半切换状态。实现里它排在前面，失败直接 `Err` 返回，`engine_profile` 一个字节都没写。
2. **默认值才是「干净」的第一现场。** 首版只改了切换逻辑，真实窗口首跑 `boot_base_categories_only_plain` 判 **false**——全新数据根没有设置文件，`Settings::default()` 仍给四类，于是冷启动瞬间首页胶囊挂着「工作记录」。这不是测试挑剔：用户第一次打开就会看到。把 `Settings::default()` 与 serde 默认都改成 `standard_thread_categories()`，`default_thread_categories()` 降级为「专业 Profile 的出厂清单」，第二轮 16/16 全绿。这一条留在这里，因为只读 STATUS 会以为一次就写对了。
3. **「找不到」与「读坏了」必须分开。** `load` 在文件不存在时返回 `Ok(None)`，交给调用方决定出厂或继承；JSON 损坏、schema 不符、`profile` 字段与实际 Profile 不符一律 `Err`，不静默退回默认四类。否则用户手改坏的文件会被悄悄「修」成别的样子。集成测试 `broken_category_file_is_rejected_without_touching_settings` 同时断言设置文件前后完全一致。
4. **回存的是「当前生效的表」，不是出厂表。** `apply_for_switch` 先 `settings::load` 取现表再 `save` 给离开的 Profile，所以用户在纪检下把「工作记录」改成「本周记录」、加了 `custom:reading`，切走切回仍是他的版本（`user_edits_survive_switching_away_and_back` 与单测 `user_edits_survive_a_round_trip` 双向覆盖）。
5. **继承只发生一次，之后是它自己的表。** 自定义 Profile 首次切换若没有 `categories.json`，从当前表继承一份并立刻落盘（`source: "inherited"`）；之后就是 `file`。集成测试还断言在自定义 Profile 里删掉 `project` 后往返不会复活——继承不能变成「每次都从别人那里借」。
6. **基础标准零写入仍然成立。** 切入基础标准走 `standard_thread_categories()`，不调 `save`；`standard_after_discipline_keeps_only_plain` 断言 `profiles/omnidesk-base/categories.json` 不存在，单测 `switching_to_standard_leaves_only_plain_and_writes_no_profile_file` 另外断言切走时只是「回存不改写」——纪检那份文件前后字节相同。
7. **不碰线程数据。** 本轮只改 `thread_categories` 这一张表，`thread_store` 里既有线程的 tag 一条都没动。旧 tag 在新表里找不到时前端 `categoryLabel` 回落为「未分类」，切回专业 Profile 又显示真名，这是设计而非丢失。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `cargo test --offline --lib profile_categories` **5 passed**；`cargo test --offline --test profile_categories_host` **4 passed**；`cargo test --offline --tests` 428 lib + 集成 2+1+13+6+5+10+20+4+3+1 全绿；`npx tsc --noEmit` 无输出；`npm run build` 通过（仅既有 chunk 体积提示）；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` **38/38**；`verify_profile_switch_110.py` 22/22。

**真实窗口核对：** `preview_profile_switch_110.py --display :137 --root /tmp/od116-preview` 一次性 Xvfb + 隔离根，**16/16 checks 全 true**，全程一个窗口：冷启动 `base_categories: ["plain"]`（干净底座）、切到纪检后 `thread_categories` 变四类且 `app-data/profiles/discipline/categories.json` 落盘、切回基础后设置文件再收敛为 `["plain"]`。这条链路是「用户切一下就能看出来」的那一层证据。

**已知限制（不作缺陷计）：** 分类表只在切换 Profile 时同步，手改 `categories.json` 需再切一次或重启（与声明文件同口径）；既有线程 tag 不迁移；真实联网安装插件与真实模型端到端仍未测。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。


## OD-ARCH-114 / 115 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 这两轮补的是同一件事的两半：Profile 能「装」，也要能「卸」。114 让切换回执说出「哪些插件还没装上」并给一键安装；115 让切回基础标准时 OmniDesk 写进全局 `skills/` 的业务技能真正离开，底座才是干净的。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104–113 未提交部分）。逐项核对 `profile_assets.rs` 的 `wanted_skills` / `load_managed_skills` / `save_managed_skills` / `restore_parked` / `retire_unwanted` / `reconcile_managed_skills` / `record_managed_skills` 与 `PluginSpec` / `required_plugins` / `ensure_plugins`；`engine_supervisor.rs::assemble_profile_assets` 的调用顺序；`lib.rs::profile_choice_set` 的两条分支、`profile_assets_repair` 的补登记、`profile_plugins_ensure` 的注册；`TopNav.tsx` / `SettingsPanel.tsx` / `api.ts` / `types.ts` 的接线与 `data-hover-hint` 用法。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**审查判断（几处特意验证过的边界）：**

1. **「不装配」不等于「已卸载」。** 这是 115 的全部理由，也是本轮最容易被漏掉的一条：DSH 的 `dsh-skill-filesystem`（`lib/index.js` 第 171 行）把 `<dsh_home>/skills/` 当全局用户根无条件扫描，与 Profile 无关。此前切回 `omnidesk-base` 只做到「不再写」，文件还在，引擎照旧把它们加载进来——干净底座只在配置层成立。审查时特意从运行时源码确认了这条扫描路径，不接受「应该不会加载」的推断。
2. **登记必须发生在装配之后，而 `reconcile` 在装配之前。** 这是本轮实现里唯一的真实缺陷：先写的版本只在 `reconcile` 里写登记表，那时目标 Skill 还没落盘，`keep` 过滤后恒为空 → 登记表永远不存在 → 收回逻辑永远不触发。三条新集成测试首跑全红（`switching_back_to_standard_parks_managed_skills` 报「装配后必须有登记表」），不是测试写错，是产品没走通。补 `record_managed_skills` 并在三条写入路径（出厂装配、自定义装配、修复）后调用后转绿。测试先红后绿这条记录留着，因为它正是「只看 STATUS 会漏掉」的那类问题。
3. **停放是移动，归位优先于覆盖。** `retire_unwanted` 用 `rename`，不删除；停放区已有同名先 `backup_dir` 成 `.bak-<epoch>`。`restore_parked` 目标已存在时不动，随后交给既有的 `drift` 判定，用户改过的正文不会被内嵌副本冲掉——`parked_skill_keeps_user_edit_after_return` 覆盖了这条。
4. **基础标准仍然零写入。** `reconcile` 在没有任何登记项时是纯空操作：不建 `skills/`、不建停放区、不写登记表。`standard_profile_creates_nothing_at_all` 与 `repair_standard_profile_still_writes_nothing` 继续通过，「干净底座」没有被这一轮放松。
5. **安装不塞进装配。** `ensure_plugins` 只由 `profile_plugins_ensure` 调用，装配与切换路径一次都不调（已加静态断言 `ensure_plugins_never_in_assembly` 防回退）。切换是同步本地操作，把会挂起的网络拉包塞进去，会让「切换失败」与「插件没网」混成同一个错误。界面把选择权交回用户：列出缺的包，「安装」或「稍后」。
6. **所有权类型，不为凑生命周期泄漏。** 自定义 Profile 的插件登记是 `String`，出厂清单是 `&'static str`；中途试过 `Box::leak` 统一成静态引用，会随每次探测泄漏内存，已改成 `PluginSpec` 持有所有权。这类「编译能过、长期跑会漏」的写法在审查中按缺陷处理。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `cargo test --offline --test profile_assets_host` **20 passed**；`cargo test --offline` 423 lib + 集成 2+1+13+6+5+10+20+3+1 全绿、EXIT=0；`npx tsc --noEmit` 无输出；`npm run build` 通过（仅既有 chunk 体积提示）；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` **34/34**；`verify_profile_switch_110.py` 22/22。

**部署核对（真实实例，非模拟）：** release 重建 2m59s 无 warning/error（7,591,728 字节）；`scripts/omnidesk.sh --stop` 后重启，app 79834 / 引擎 79928 均按 `--profile discipline` 起来；`~/OmniDesk/app/logs/profile-assets.log` 末行 `profile=discipline channel=web-api result=ok`；`~/.dsh` 未被触碰。`~/OmniDesk/runtime/dsh-home/skills/` 下 8 个 Skill 目录齐备，`.omnidesk-managed` 已由新二进制的启动路径真实写入 8 项（+1 行表头）——这是 `record_managed_skills` 在正式构建里生效的直接证据，不是集成测试推出来的。`.assets.v1.json` 登记 31 项，与 113 一致。

**真实窗口核对（最直接的一条用户可见结论）：** `preview_profile_switch_110.py --display :137 --root /tmp/od115-preview` 起一次性 Xvfb 与隔离根，**12/12 checks 全 true**，全程只开一个窗口：冷启动 `omnidesk-base` 时无场景、无资产；顶栏切「纪检专业」后 Dock 立即出现三个业务场景，`skills/` 出现登记表与 8 个 Skill 目录；再切回「基础标准」后 `skills_after_base: []`（真为空，连登记表都不留），`parked_skills` 列出同样 8 个名字。这条断言是本轮唯一「人眼可复现」的证据，它把「配置切走了、技能还在」这个原始问题在窗口层面钉死了。

**已知限制（不作缺陷计）：** 真实联网安装插件未在正式实例上跑（会拉包，且可能改变用户当前 Profile 的插件集），留给用户实测；停放区没有界面入口，只能经 `reconcile` 自动归位或从文件系统取回；真实模型端到端仍未测。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。


## OD-ARCH-112 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 111 把 Skill 正文塞进了二进制，但引擎自己仍按编译期仓库路径找引擎目录和 DSH CLI。这两条路径一旦分叉，用户看到的就是「插件面板说已就绪、发消息引擎起不来」。本轮把引擎启动与插件探测统一到同一套候选顺序，分发环境不会再自相矛盾。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104–111 未提交部分）。逐项核对 `engine_supervisor.rs` 的 `locate_engine_dir` 与 `from_env` 中两处分支、`profile_assets.rs` 的 `default_dsh_cli` / `default_dsh_cli_for` / `locate_cli` 三层拆分，以及新增的 `distributed_layout_resolves_engine_and_cli_without_repo`。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**审查判断（几处特意验证过的边界）：**

1. **两边必须共用同一次推导，而不是复制同一段代码。** 「面板有、引擎没有」的根因就是两处各写一遍路径拼接。现在引擎侧调 `default_dsh_cli_for`，插件侧 `default_dsh_cli()` 也转调它，改一处两边同时变。这比「记得同步两份常量」可靠。
2. **找不到就说找不到。** `locate_engine_dir` 只返回真正含 `omnidesk_engine/` 的候选，全不中时返回第一候选让 `validate()` 报「引擎包目录缺少 omnidesk_engine/」。没有把首候选直接当成可用，也没有把「目录存在」当成「引擎可用」——这是上一轮 `is_dir()` 那条判断的延续。
3. **测试不能再动进程级环境变量。** 最初版本改 `OMNIDESK_REPO_ROOT`，并行测试里把 `engine_config_default_channel_is_web_api` 拖成假失败。改为纯路径推导后，`locate_cli` 显式接收 `home`，测试完全可重入。这类并行污染会伪装成产品缺陷，值得记下来。
4. **不内嵌运行时。** 兜底只解决「路径推导」，DSH 运行时与 `node_modules` 仍由安装器提供。分发边界没有借这一轮放宽。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `cargo test --offline --lib` 423 passed；`cargo test --offline` 423 lib + 集成 2+1+13+6+5+10+14+3+1 全绿、EXIT=0；`npx tsc --noEmit` 无输出；`npm run build` 通过（仅既有 chunk 体积提示）；`python3 -m pytest tests -q` 89 passed / 30 subtests。

**已知限制（不作缺陷计）：** 兜底顺序按 `~/OmniDesk/{app,runtime,engine}` 默认布局设计，安装器换布局仍需 `OMNIDESK_ENGINE_DIR` / `OMNIDESK_DSH_CLI`；本机 `$HOME/OmniDesk/runtime` 下没有 `dsh-runtime`，正式实例仍靠 `OMNIDESK_DSH_CLI`；真实模型端到端仍未测。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。


## OD-ARCH-111 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 这一轮补的是 109/110 留下的前提缺口：Profile 的「自动写入」原来只在源码树里成立。现在 8 个 Skill 正文与三类写作资产随二进制内嵌，干净环境（仓库目录不存在）切到「纪检专业」仍能装出完整工作台，插件探测也有了分发环境的 CLI 路径兜底。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104–110 未提交部分）。逐项核对 `profile_seed.rs` 的 `include_str!` 路径与清单、`profile_assets.rs` 的 `sync_skill_files` / `repo_skill_files` / `materialize` 两条来源分支 / `materialize_references` 的 `Option<&Path>` 改造 / `default_dsh_cli` 的候选顺序 / `inspect` 的判定分支、`lib.rs` 与 `engine_supervisor.rs` 里三处 `is_dir()` 过滤、`profile_rules_view` 的内嵌回落，以及 `STATUS.md` 与 `docs/decisions.md` D-039。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**审查判断（几处特意验证过的边界）：**

1. **「仓库根存在」不等于「仓库根可用」。** `env!("CARGO_MANIFEST_DIR")` 是编译期绝对路径，分发后该目录通常不存在；但只加 `repo_root_hint().ok()` 仍会传进一个不存在也不报错的路径，Skill 就会被判成「源缺失」。三处调用点统一加 `is_dir()` 过滤，让「目录不在」真正退化成「走内嵌」，这是本轮的核心判断。`default_dsh_cli()` 同样必须先 `is_file()` 再返回候选，否则会把一个不存在的 CLI 当成可用。
2. **两条来源必须写同一套状态记录。** 原实现用 `copy_missing_tree` 整目录复制，Skill 文件根本不进 `.assets.v1.json`。后果是：只要来源从仓库换成内嵌（或反之），摘要对不上，同一份内容会被判成「用户改过」并拒绝更新。改成 `sync_skill_files` 逐文件三态写入 + 摘要登记后，跨来源切换是 `unchanged` / `updated`。`embedded_seed_fills_skills_without_repo` 的二次装配断言与 `second_run_rewrites_nothing` 一起覆盖了这一点。
3. **逐字节比对测试是内嵌方案的必需品。** 内嵌把同一份正文存了两处，漂移会静默发生（开发机看到新版、用户机器装出旧版）。`embedded_seed_matches_repo_sources_byte_for_byte` 遍历 `profile_seed::SKILLS` 与 `ASSETS` 逐个比对仓库源，改一边不改另一边就红。这比「记得同步」可靠。
4. **内嵌不能放松 drift 保护。** `embedded_skill_user_edit_is_not_overwritten` 在无仓库环境下改 `SKILL.md`，再装配一次仍是 `drift` 且内容原样。内嵌是「有了更好」，不是「有权覆盖」。
5. **巡检要说实话。** 无仓库时若内嵌有副本，`inspect` 报 `missing`（切换/重启会补入内嵌正文）而不是 `missing_source`（无法补齐）。前者是可修复状态，后者是死路，混淆会让用户误以为这个 Profile 装不全。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `cargo test --offline` 422 lib + 51 集成（2+1+13+6+5+10+14+3+1）全绿、EXIT=0；`cargo test --offline --test profile_assets_host` 14 passed；`npx tsc --noEmit` 无输出；`npm run build` 通过；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 24/24；`verify_profile_switch_110.py` 22/22。

**真实分发预览（收尾补跑，非模拟）：** `preview_embedded_assets_111.py` 7/7 通过。跑的是 `target/release/omnidesk-desktop`（7,552,560 字节）本身，一次性 Xvfb + `/tmp/od111-preview` 隔离根，`OMNIDESK_REPO_ROOT` 指向一个从不存在的目录，即模拟 deb/AppImage 装到用户机器后源码树不在的情形。实测：ghost 目录始终未被创建；装配日志 `profile=discipline channel=web-api result=ok` 且不含 `missing_source`；`dsh-home/skills` 下 8 个 Skill 的 `SKILL.md` 齐备；写作模板 / 写作风格 / 优秀案例三类 references 齐备；`dsh-home/profiles/discipline` 有 `package.json` 与 `cordis.patch.yml`，`app-data/profiles/discipline` 有 `decl.yml` 与 `rules.json`；8 个内嵌 Skill 与仓库源逐字节相同。首轮 6/7，唯一失败是脚本把 `package.json` 的断言目录误写成 `app-data/`（实际在 `dsh-home/profiles/<name>/`），改断言后 7/7 —— 是脚本缺陷，不是产品缺陷，记录在此以免下次误读。

**真实窗口回归（收尾补跑）：** `preview_profile_assets_109.py` 两轮 9 项 checks 全 true。空隔离根以 discipline 启动后自动装配，8 个 Skill 齐备，设置页「Profile 资产」分区显示「已就绪」；切到基础标准后 `startup_log` 为 `profile=omnidesk-base ... result=skipped`，`profile_dirs` 与 `skill_dirs` 均为空，即干净底座零写入。首轮失败在 `start_vite`（脚本硬依赖 5273 端口的 dev server，当时未运行），属脚本运行前提，不是产品缺陷。

**真实数据根只读复查：** `$HOME/OmniDesk` 下 8 个 Skill 与仓库源逐字节相同；`.assets.v1.json` 登记 31 项；装配日志无 `missing_source`。同时确认 `$HOME/OmniDesk/runtime` 下没有 `dsh-runtime`，DSH 运行时仍在仓库内，`default_dsh_cli()` 的三个候选在本机全部落空——插件探测与安装需要 `OMNIDESK_DSH_CLI`。这属于安装器布局的既有限制，验收时须连同「插件缺失可一键安装」一起核对，不能只凭面板文案。

**已知限制（不作缺陷计）：** 改 `skills/` 后必须重建二进制才进用户机器；内嵌清单是静态表，新增 Skill 要两处登记；`default_dsh_cli()` 的兜底按 `~/OmniDesk/{app,runtime}` 默认布局设计，安装器换布局时仍需 `OMNIDESK_DSH_CLI`；真实模型端到端仍未测；`worklog.rs::AppendReceipt::failure` 的 dead_code 告警与本轮无关。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。


## OD-ARCH-110 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 切换 Profile 现在真的是一次「换工作台」：装配资产 → 写设置 → 重读配置 → 立屏障 → 同步两份配置 → 停旧引擎，全程不需要重启应用。真实窗口里同一个进程从「基础标准」切到「纪检专业」，Dock 当场从 3 个平台入口变成 3 个平台入口加 3 个专业场景，切回来又落回干净底座。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104/106/107/108/109 未提交部分）。逐项核对 `lib.rs` 的 `adopt_engine_config` / `profile_choice_set` / `profile_choice_view`、`App.tsx` 的 `loadDecl` / `probeEngine` / `profileEpoch` 接线、`TopNav.tsx` 的 `ProfileBadge` 回执与回调、`SettingsPanel.tsx` 的 epoch 清理、`types.ts` / `api.ts` 的字段与注释、两处新验证脚本，以及 `STATUS.md` 与 `docs/decisions.md` D-038。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**审查判断（几处特意验证过的边界）：**

1. **屏障必须早于换配置，这是本轮最关键的一处判断。** 反过来做（先换配置、再立屏障、再停引擎）会在中间留下一段窗口：新任务拿到新 Profile 的规则与注入 Skill，却被送进仍在运行的旧 Profile 引擎——正是 104 之后一直在防的那类权限错配。现在 `begin_change()` 在 `adopt_engine_config` 之前，`verify_profile_switch_110.py` 的 `barrier_before_config_swap_before_stop` 直接对源码顺序设了断言，改坏就红。
2. **两份配置必须来自同一次解析。** `adopt_engine_config` 按值接收 `fresh: EngineConfig`，supervisor 与 `HostSnapshot` 各拿一份克隆。若在函数内各自 `from_env`，极端情况下（设置文件在两次读之间被改）会得出两个不同的 Profile。`switch_reparses_once` 断言切换路径只解析一次。
3. **停不下来就如实说，不谎称切换成功。** `stop_engine` 失败走 `change.fail(message)`，屏障保留（失败关闭），回执里 `engine_stopped: false`，note 明说「旧引擎未能确认停止；在旧引擎停止前宿主会拒绝新的引擎调用」。前端也据此不刷新场景——避免出现「界面说切了、实际规则还是旧的」。
4. **配置解析失败要回滚设置。** 先写盘、再 `from_env`，若解析失败就把 `engine_profile` 写回切换前的值。否则会留下「磁盘说 discipline、引擎跑 omnidesk-base」的静默错配。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `cargo test --offline` 422 lib + 2 + 1 + 13 + 6 + 5 + 10 + 10 + 3 + 1 全绿；`npx tsc --noEmit` 无输出；`npm run build` 通过；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 24/24；`verify_profile_switch_110.py` 22/22；`preview_profile_switch_110.py` 11 项 checks 全 true（`boot_base_settings_null` / `boot_base_no_scenes` / `boot_base_no_assets` / `popover_opened` / `pick_discipline` / `switch_assembled` / `dock_switched_without_restart` / `popover_reopened` / `pick_base` / `switch_back_saved` / `dock_back_to_base`）。

**回归补跑（收尾时追加，含一处脚本假通过）：** `verify_ui_082.py` 17/17 通过。`verify_home_layout_072.py` 首次重跑为**假通过**——脚本对 Profile 徽标用固定像素 `(310,44)` 点击，而徽标文案在 078/082 之后已缩成「标准」，点击落到旁边的 IPC 开关上，弹层从未打开，后续步骤也没有断言，于是「跑完无异常」被当成了通过。已改为按无障碍元素定位徽标与「纪检专业」选项，并对「弹层展开 / 确认框出现 / 设置写入 discipline / 徽标变为专业」四处加了强制断言，重跑通过（`badge_point: [274,43]`、`engine_profile_after: discipline`、`badge_now: 专业`），隔离根与 `~/.dsh` 均零变化。这类「脚本不报错但其实没验」的问题比产品缺陷更危险，记录在此以便后续脚本都先确认断言确实生效。

**测试过程中修正的两处脚本缺陷（不是产品缺陷，但会伪装成产品失败）：**

1. 脚本把 `OMNIDESK_DSH_PROFILE=omnidesk-base` 写进环境变量。这是最高优先级开关，会把顶栏切换结果整个盖掉，第一轮 11 项 checks 全 false。改为空串（等价未设置）后一次通过。
2. WebKit 在这台机器上不把 `data-testid` 暴露到 AT-SPI，Profile 徽标在树里只是文本为「标准」/「专业」的按钮。脚本改按可见文案取坐标点击，并以「弹层出现」为成功信号。

**已知限制（不作缺陷计）：** 切换必然中断正在生成的内容，界面与确认弹窗都已如实告知；旧引擎 10 秒优雅退出、最多再 10 秒强杀；不做「切换前排空任务队列」，也不迁移旧 Profile 的会话上下文；真实模型端到端仍未测。既有 `worklog.rs::AppendReceipt::failure` dead_code 告警与本轮无关。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。

## OD-ARCH-109 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 切换 Profile 现在真的会把配置、场景声明、规则包、Skill 与插件清单补齐到隔离根，缺插件能在设置页一键安装；基础标准仍然零写入。真实窗口两轮对照的结果是：空数据根以 `discipline` 启动，`dsh-home/profiles/discipline` 下有 `package.json` / `cordis.patch.yml`，`app-data/profiles/discipline` 下有 `decl.yml` / `rules.json` / `.assets.v1.json`，`dsh-home/skills` 下 8 个 Skill 齐备，资产页显示「就绪」；全新根以 `omnidesk-base` 启动，启动日志只有 `result=skipped`，三处目录全空，资产页显示基础标准不安装任何资产。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104/106/107/108 未提交部分）。逐项核对 `profile_assets.rs` 的 `materialize` / `sync_bytes` / `materialize_custom` / `inspect` / `probe_package` / `install_plugin`，`profile_lifecycle.rs` 的登记表与 `remove` 的 `.trash` 路径，`lib.rs` 的 `build_state` 与四条新命令注册，`engine_supervisor.rs` 的 `assemble_profile_assets` 与新增单测，前端 `SettingsPanel.tsx` / `TopNav.tsx` / `api.ts` / `types.ts`，以及 `docs/decisions.md` D-037。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**本轮审查中修掉的两个真问题：**

1. **单测作用域缺失。** 新单测 `assemble_profile_assets_covers_both_ends` 在 `engine_supervisor::tests` 里直接引用 `profile_assets::ASSET_STATE_FILE` 但未引入模块，导致 `cargo test --lib` 编译失败。补 `use crate::profile_assets;` 后通过。这是编译期错误，不影响已发布的二进制，但会挡住后续所有 Rust 测试。
2. **测试写入真实用户目录。** `assemble_profile_assets()` 通过 `settings::app_config_dir()` 推导应用根，单测无法隔离，实际写进了 `~/OmniDesk/app/profiles/discipline`。改为从 `engine_state_dir` 的父目录推导（默认即 `<app_root>/engine-state`，与生产等价）后，测试全部落在临时夹具。误写仅产生一个 `.assets.v1.json`，其记录的 `package.json` / `cordis.patch.yml` / `rules.json` 摘要与真实文件一致，`decl.yml` 一条未登记，**未覆盖或删除任何 Profile 资产**。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `cargo test --offline --lib` 420 passed；全部集成靶 14 passed；`npx tsc --noEmit` 无输出；`npm run build` 通过；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 24/24；`preview_profile_assets_109.py` 两轮 checks 全 true。

**审查判断（几处特意验证过的边界）：**

1. **装配必须挂在应用启动期，挂引擎 `start()` 是错的。** 引擎按需启动，冷启动后不提交任务就不会装配；真实窗口第一轮资产页全空即是证据。现已前移，`start()` 那次保留为幂等兜底，两处共用同一入口，不会出现「界面切了但没装」的分叉。
2. **三态判定不会被误覆盖。** `sync_bytes` 只在「文件不存在」或「当前摘要 == 上次我们写入的摘要」时写盘，其余一律 `drift` 且给出原因。`deleted_skill_is_healed_on_next_run` 与 `user_edit_is_preserved_and_reported_as_drift` 两个集成测试覆盖了这两条。
3. **标准底座的零写入是硬边界。** `materialize` 在 `profile == standard` 时直接返回，连目录都不建；集成测试 `standard_profile_creates_nothing_at_all` 与真实窗口第二轮的目录清单共同证明。
4. **插件安装不会污染 `~/.dsh`。** `install_plugin` 显式传 `DSH_HOME`，静态检查 `install_uses_dsh_home` 已断言；安装只由用户点击触发（`install_is_click_only`），装配本身不联网。

**已知限制与风险（不作缺陷计）：** Skill 复制依赖仓库 `skills/` 存在，打包分发后需另做内嵌；摘要判定无法识别「用户手工放入同样内容」的情形（内容一致，影响可接受）；插件安装依赖 pnpm 与网络；WebKit AT-SPI 对设置页正文暴露不稳定，标准底座结论以文件状态 + 启动日志 + 截图为准；真实模型端到端仍未测。既有 `worklog.rs::AppendReceipt::failure` dead_code 告警与本轮无关。

**未提交未推送：** 按批准范围只执行。工作区改动原样保留。

## OD-ARCH-104（插件挂载）审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 专业 Profile 现在真的多挂了一个办公插件，基础标准一点没变。discipline 的 `--dump-config` 第 1229–1230 行出现顶层 `- id: skill-office` / `name: '@deepseek-ai/dsh-skill-office'`，紧跟在补丁层注释之后、顶格与 `skill-filesystem` 同级；omnidesk-base 全文 0 次出现该包，两份 dump 的 stderr 都为空（没有 `disabling profile plugin`）。插件本体直连探针注册出 `office-docx / office-pptx / office-xlsx`，provider 名 `dsh-office`，正文含 `Installed LibreOffice Kit`。真机以 `--profile discipline --no-open --port 0` 起服务成功并打印 web 地址。

**独立复核范围：** HEAD 7820751 + 工作区改动（含 106/107/108 未提交部分）。逐项核对 `samples/profiles/discipline.profile.patch.yml`、实机 `~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.patch.yml`、`samples/profiles/omnidesk-base.profile.patch.yml`、`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-app-boot/lib/index.js` 第 61–110 行、`dsh-web-app/cordis.patch.yml` 第 493–506 行的层级注释、`dsh-skill/lib/index.js` 的 `SkillRegistry.registerProvider` 与 `ScopedLayers`。`runtime/` 零差异；未读真实工作记录与凭证；实机只改了一个 Profile 补丁文件，改前已备份。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `verify_plugin_scope_104.py` 34/34；`npx tsc --noEmit` 无输出；`npm run build` 通过；`cargo test --offline` 405 lib + 41 集成全通过；`python3 -m pytest tests -q` 89 passed、30 subtests。

**审查判断（三处特意验证过的边界）：**

1. **计划里 S1 的措辞是错的，实施用的是另一条路径，我确认结果等价。** 原文说「继承 preset-standard 的 plugins 数组追加」。`applyEntryPatches` 对非 insert 补丁逐 key 赋值，`config` 是整体替换而不是深合并；`buildMap` 只把 `group && Array.isArray(config)` 的条目当可定位目标，`preset-standard` 没有 `group: true`，所以「往它的 plugins 里追加」没有实现路径。改成无 id 的顶层 `insert`（同函数第 73–88 行）直接 push 到宿主层，两条路径的差别只在于插件挂在哪一层；宿主层正是 `skill-filesystem` / `tool-skill` 在 `dsh-base` 里的位置（`dsh-base/cordis.patch.yml:306`、`:309`），因此层级一致。文档已按实际写法纠正，不保留错误措辞。
2. **宿主层注册能被会话看到，这一点是源码事实而非推断。** `dsh-web-app/cordis.patch.yml:493-500` 明说 `skill` 注册表留在宿主层且是 host + per-scope 分层：部署级 provider 注册进 global 层，preset 的 `skill-filesystem` 注册进该 preset 层，agent 读合并后的目录。`dsh-skill` 的 `registerProvider` 用 `this.layers.effect(this.ctx, ...)`，无 scope 的 ctx 落 global 层。所以我们的 office provider 进 global，standard preset 的 `tool-skill` 能读到。**未验证的是真实会话里的清单与模型实际调用**——那需要真实模型对话，属已知限制，不当作通过证据。
3. **基础标准确实一点没变。** 只改了 discipline 的补丁层，`omnidesk-base.profile.patch.yml` 未动（仍是 106 修订后的「与官方 dsh-base 插件面一致」版本），dump 结果里 skill-office 出现 0 次。这一条是本计划「不污染基础标准」的硬边界，已由脚本断言。

**已知限制与风险（不作缺陷计）：**

1. **出厂资产不会自动落到用户机器（缺口，需后续计划）。** `engine_supervisor.rs::seed_profile_if_missing()`（第 222 行起）只播种 `web` Profile，且 `manifest.exists()` 即返回；仓库里没有任何脚本把 `samples/profiles/discipline.profile.patch.yml` 复制到 `~/OmniDesk/runtime/dsh-home/profiles/discipline/`。本轮只手工同步了当前实机。新装用户与升级用户拿不到这条补丁，需要在后续计划里决定是「按 profile 名补种」还是「切换 Profile 时补写缺失的 cordis.patch.yml」。
2. **真实模型端到端未测。** 没有让模型实际产出 docx / xlsx / pptx。已验证的是插件挂载、provider 可列三项、资源路径可解析、`libreoffice-kit` 的 cli 文件存在。
3. **`cargo` 那条 warning 仍在。** `worklog.rs::AppendReceipt::failure` 未被使用，属上个计划遗留的既有告警，本轮未触碰；用户已单独问过，属另一件事。
4. **编号重复已记录。** PLANS.md 第 3 行（本计划）与第 190 行（随手记语义判定，早已实施）都叫 OD-ARCH-104。本轮保留编号并加「插件挂载」限定，在 STATUS.md 顶部注明，不为消歧另起新号。

**回归脚本的一次偶发失败已排除，与本轮改动无关。** 首次在 23:53 跑 `verify_skill_flow_injection_108.py` 报「随手记轮没有到达模型」，当时只用同目录 `--dump-config` 与真实启动证明引擎能起，未下结论。原样重跑（脚本 sha256 未变、patch 仍是本次 office 插入版）**15/15 通过**，证据 `evidence/od-arch-108/flow-injection.json`（2026-10-29 00:00:24，result: pass）。据此判定为一次性根 + Xvfb 窗口的时序偶发，不记为缺陷，也不当作 office 插件稳定的正面证据——它证明的是「加了这段 patch 后注入流程仍按 108 的口径工作」。

**未提交未推送：** 按批准范围只执行。工作区改动原样保留。

## OD-ARCH-108 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 「场景绑定什么」和「这一轮到底注入什么」现在按流程分开了。工作记录场景下，随手记只注入 `memory-agent`，周报只注入 `memory-report`；`anysearch` / `knowledge-search` / `document-review` / `knowledge-agent` 已从两份规则文件的绑定里全部移除，改由模型自己判断要不要读。真实窗口两轮对照 15/15 通过，探针命中原样值：随手记 `["随手记 (Memory Agent)"]`、周报 `["周报起草 (Memory Report)", "整理成周报"]`，两轮都不含 `anysearch`。

**独立复核范围：** HEAD `7820751` + 工作区改动（同时含 106/107 未提交改动）。逐文件核对 `conversationPrompt.ts`（`presetSkillsFor` 的回落语义、note 分支仍 `.slice(0,1)`、report 分支的 `skills` 取自 `report` 组）、`HomeCanvas.tsx`（`readBinding` 的 `max_injected` 截断与正文缺失过滤、`skillsByFlow` 的生成条件）、`HomeChat.tsx`（`anyFlow` 判定）、`SettingsPanel.tsx`（`weekly-note` / `weekly-report` 标签）、两份 `rules.json`、四个 SKILL.md、`profile_rules.rs` 单测、`docs/decisions.md` D-035。`runtime/` 零差异；未读真实工作记录与凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx tsc --noEmit` 通过（无输出）；`npm run build` 通过（仅既有 chunk 体积提示）；`cargo test --offline` EXIT=0；`python3 -m pytest tests -q` 89 passed、30 subtests；`verify_skill_flow_injection_108.py` **15/15 pass**；`verify_skill_scope_107.py` 33/33；`verify_skill_bindings_082.py` 57/57；`verify_worklog_089.py` 83/83；`verify_env_085.py --no-write` 42/42；`scripts/sync-skills.sh --check` 报隔离副本与仓库源一致。窗口截图与无障碍树本人复核：`01-home.png`（Dock 工作记录入口）、`02-note-composer.png`、`03-weekly-page.png`（含「去生成周报」动作）、`04-report-composer.png`。

**审查判断（几处特意验证过的边界）：**

1. **回落不能变成「偷偷注入」。** `presetSkillsFor` 在流程键**不存在**时回落场景级 `skills`，但流程键**存在且为空数组**时返回空数组、不回落。因为 `readBinding` 把「绑定为空 / 正文缺失」都折成 `null` 而不是空数组，两条路径在 `HomeCanvas` 这层已经合并成同一语义：没配就回落，配了就有内容。
2. **旧键 `weekly` 留在绑定里但不再影响周报。** 它指向 `memory-agent`，只服务随手记的回落路径；周报取 `weekly-report`，两者互不影响。`profile_rules.rs` 的单测明确断言 `weekly` 不等于 `memory-report`——如果哪天有人把旧键指回周报，随手记就会被塞进周报技能，这条断言会立刻失败。
3. **上限仍然生效。** `readBinding` 保留 `.slice(0, max_injected)`，与 `aiDraft` 的 `.take(max_injected)` 同口径。每个流程只绑 1 个，离 `MAX_SKILL_BINDINGS_PER_SCENE = 3` 还有余量，本轮没动上限。
4. **实机规则文件只改了绑定段。** 改前备份 `rules.json.bak-od108-20261028195703`，旧值是 `weekly: memory-agent / knowledge-search / anysearch`、`writing: material-writing / knowledge-search / document-review`（均 `max_injected: 3`）。改后脚本断言「除 `skill_bindings` 外逐字相同」通过；仓库样例与实机现在同口径。
5. **注入量要分两个口径说，不能混。** 真实窗口量到的 `note text_chars = 9699`、`report text_chars = 9912` 是**整段提示词**（含场景规则、铁律、安全边界），不是技能注入量。技能正文这一层：随手记 `memory-agent` 1408 字符（旧 1281，多了按需加载那段）；周报 `memory-report` 1099 字符（旧 988）。按实机旧规则（`weekly` 绑三个、`max_injected: 3`）算，周报引入的技能正文从 4901 降到 1099。计划正文里原先写的「约 5.9k」是把现在四个技能正文相加的 6127，实机从未这样绑过，已在 PLANS.md 的实施结论里改成实测值。
6. **窗口验证器自身出过两个坑，都已修。** ①合成端点原先只存 `last_user` 前 200 字符，用户原话在末尾会被截掉，导致误判「请求不含用户输入」→ 改成按请求序号取新请求；②WebKit 的 `<input>` 当前值不进无障碍树（entry 只暴露 placeholder），所以不能拿 AX 断言产品预填，改为用「整理成周报」这个只有产品预填路径才会带上的字符串作探针，事实写进 `prefill_visible_in_ax: false`。这两条不写清楚，后来者会以为脚本在断言它没有真正断言的东西。

**计划偏离的裁定：** 无架构偏离。两处收敛在计划语义内：`skillsByFlow` 只在一侧有值时生成（缺侧回落场景级），以及窗口脚本改探针取法与放弃 AX 断言。没有扩大范围，也没有新增 Skill 开关或编辑器。

**已知限制：** ①按需加载完全依赖模型自己的判断——它觉得不需要联网就不会去读 `anysearch`。这是用户明确选择的取舍（省 token 换一次额外工具往返），不算缺陷，也不能靠代码兜住。②窗口验证用的是合成端点，不是真实模型；真实模型下的端到端表现仍未验。③真实数据根 `~/OmniDesk` 只改了 `profiles/discipline/rules.json` 的绑定段（有备份），其余未动；`runtime/dsh-home/skills` 已按仓库源同步。④本章改动未提交、未推送。

**阶段决定：OD-ARCH-108 验收通过。** 106 / 107 / 108 一并等用户指示再提交推送。

---

## OD-ARCH-107 审查结论（ARCHITECT，2026-10-28）——通过

**结论：通过。** 问题的根因是**页面口径**，不是加载失败：能力管理页列的是 `<DSH_HOME>/skills` 的磁盘清单，所有 Profile 共用一份（DSH 官方 `skill-filesystem` 语义），所以切到 base 后场景消失了、这一页却一模一样。本轮把「磁盘上有什么」与「本 Profile 会不会预注入」分开讲：预注入的进「本 Profile 预注入」组并带场景标签，其余进「可被模型按需加载」组；基础标准明确写「不预注入任何 Skill，下面列出的仍可按需加载」。注入链路、Skill 目录结构、rules.json 内容都没动。

**独立复核范围：** 起点 HEAD `7820751` + 本轮改动（工作区同时含 106 的未提交改动）。逐文件核对 `capabilities.rs`（`apply_scope` 是纯映射、`effective = order <= max_injected`、缺失绑定不凭空造条目、`load` 只做默认空作用域包装）、`lib.rs`（`profile_rules_view` 是唯一读取实现、`skill_scope` 复用同一份 RulesView、`capability_view` 走 `load_with_scope`）、`types.ts`（六个新字段的类型与 Rust 侧一致）、`SettingsPanel.tsx`（两组按 `bindings.some(effective)` 互斥、四态说明的判定顺序、缺失绑定告警、已加载态计数取自 `caps.preinjected.length` 而非前端另算）、`settings-panel.css`（标签方形、配色随设置页）。`runtime/` 零差异；未读真实工作记录与凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx tsc --noEmit` 通过（0 错误）；`npm run build` 通过（仅既有 chunk 体积提示）；`cargo build --offline` 通过；`cargo test --offline` **405 lib + 41 集成/文档，全部通过，0 failed**；`python3 -m pytest tests -q` **89 passed、30 subtests**；`verify_skill_scope_107.py` **31 / 31**（末项 `note_preinjected_count` 固定已加载态的计数来源）；`verify_skill_scope_107_window.py` **pass**。窗口截图本人复核：标准模式显示「当前是基础标准：不挂业务场景，因此不预注入任何 Skill……」、Skill 全部落在「可被模型按需加载 8」；discipline 显示「当前规则包属于 discipline……随手记只取该场景绑定里的第 1 个」、「本 Profile 预注入 6」，每条带方形场景标签（工作记录 / 文案写作）。

**最后一处文案改动后的复跑：** 补「共 N 个 Skill 会被预注入」时前端产物是改动之前构建的，为免拿旧构建冒充新证据，重跑了 `npx tsc --noEmit` 与 `npm run build`（18:38 生成新产物），再跑窗口脚本：标准轮标签像素仍为 0、未创建 rules.json / decl.yml；discipline 轮 695 像素、两组并存，`result: pass`。截图与 `skill-scope-window.json` 均出自新构建。

**审查判断（几处特意验证过的边界）：**

1. **capabilities 不自己解析规则文件。** `apply_scope` 的入参是宿主算好的 `SkillScope`，函数体内没有任何 `fs::read` / `load_rules`。`lib.rs` 的 `skill_scope` 直接复用 `profile_rules_view` 的产物（含 106 的迁移与 `skill_contents` 回填）。这条是为了避免两处解析口径分叉——一旦分叉，「界面说会注入」与「实际注入」就会长期不一致，且只在出问题时才被发现。
2. **标准模式不伪造绑定。** 无 rules.json 时 `rules.loaded=false`，`scope.scenes` 为空，于是 `apply_scope` 不产生任何 binding，两个分组里前一组为空、后一组是全部。窗口轮实测「预注入」标签强调色像素 **0**，且 `profiles/omnidesk-base/` 下既没有 rules.json 也没有 decl.yml——宿主没有偷偷补文件。
3. **注入上限不是摆设。** `effective = order <= max_injected`，第 4 条以后的绑定在界面上显示为「（超出预注入上限，不注入）」，单测 `scope_marks_effective_and_missing_bindings` 里 `max_injected=2` 而给 3 条绑定，只有前 2 条进 `preinjected`。这与 `aiDraft.ts` 的 `.take(max_injected)` 是同一口径。
4. **绑定缺失会暴露。** 绑了但宿主读不到正文的，进 `missing_bindings` 并在页面上给出告警（场景 → Skill），不再像以前那样只是「注入了空内容」。单测同时断言缺失的 Skill 不会凭空出现在列表里。
5. **四态说明的顺序是对的。** 先判 `standard`，再判 `rules_error`，再判 `!rules_loaded`，最后是已加载。规则文件写坏时报「读取失败」而不是退化成「没有规则包」——后者会让用户以为一切正常。这与 068 以来「损坏一律报错、不静默退回纯净底座」的口径一致。
6. **没有按 Profile 隔离 Skill 目录。** 这是刻意的：106 定下「base = 官方 DSH 底座」，官方 `skill-filesystem` 就是共享一份目录。所以正确的产品表达是「Skill 都可按需加载，只是预注入不同」，而不是「切 Profile 后 Skill 列表应该变」。
7. **窗口验证器自身出过一次错，已修正。** 初版用「偏冷色像素」判「有无预注入标签」，标准轮误报 884 像素——那是左栏面板底色 `(226,225,235)`。改为精确匹配标签的两种合成色后，标准轮 0、discipline 轮 695。这条不写清楚，后来者只会看到「阈值」，看不到「宽松判据会假通过」。

**计划偏离的裁定：** 无架构偏离。三处补充（`load` 保留为兼容包装、`profile_rules_view` 抽函数复用、窗口像素判据收紧）都在计划语义内，未扩大范围，也未新增 Skill 编辑/启用开关（D-030 口径不变）。

**已知限制：** ①真实数据根 `~/OmniDesk` 上未跑，本轮全在 `/tmp` 一次性根 + 合成端点 + Xvfb。②**用户机器上 discipline 的 weekly 绑定与仓库样例漂移**：live 是 `memory-agent / knowledge-search / anysearch`，仓库样例是 `memory-agent / memory-report / knowledge-search`。属用户数据，本轮未改动——如果用户希望界面与出厂样例一致，需要单独授权对齐（会影响周报到底注入哪三个 Skill）。③「随手记只取第 1 个」在界面上只是一句说明，具体取哪个仍由 Skill 与规则顺序决定，本轮没有把它做成可视化。④本轮改动尚未提交、尚未推送。

**阶段决定：OD-ARCH-107 验收通过。** 106 + 107 一并等用户指示再提交推送。

---

## OD-ARCH-106 审查结论（ARCHITECT，2026-10-28）——通过（真实数据根与真实模型实测待确认）

**结论：通过。** 基础标准 `omnidesk-base` 现在真的是「自定义 UI + DSH 官方底座」：没有场景声明就只显示首页 / 历史会话 / 知识库，对话按普通对话走，不套场景提示词、不注入 Skill 原文、不装行业规则包；业务场景与规则资产改成 `profiles/<Profile 名>/decl.yml` 与 `rules.json`，切到专业 Profile 才挂上。基础标准的插件面也回到官方 `dsh-base`（`skill-filesystem` / `tool-skill` 不再被禁用）。

**独立复核范围：** HEAD `7820751` + 本轮改动。逐文件核对 `profile_decl.rs`（`load_decl` 返回 Option 且不写盘、`migrate_legacy_decl` 的三条拒绝条件）、`profile_rules.rs`（路径、两个新字段的语义、`migrate_legacy_rules` 先校验后复制）、`lib.rs`（`profile_asset_scope` 取 supervisor 内存配置、空 Profile 退回通道标准名、迁移在读取前且幂等、`default_blocked_reason` 只标不禁）、`Dock.tsx`（三项平台固有入口 / 仅失败时回退默认）、`HomeCanvas.tsx`（`sceneOptions` 的三种可见性、preset 的「声明里有 + 规则已加载」双条件、标准模式文案门控）、`App.tsx`（`declReady`、场景校准、提示条措辞）、`SettingsPanel.tsx`（空声明文案、恢复默认的禁用与悬停说明、迁移告警）、`types.ts` / `api.ts`（`null` 语义）、`omnidesk-base.profile.patch.yml`（插件面）。`runtime/` 零差异；未读真实工作记录与凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx tsc --noEmit` 通过；`npm run build` 通过；`cargo test --offline` **445 passed、0 failed**；`cargo build --offline` 通过无 warning；`python3 -m pytest tests -q` **89 passed、30 subtests**；`verify_profile_scope_106.py` **45 / 45**；`verify_profile_scope_106_window.py` **pass**；`verify_hover_hint_101.py` 30 / 30；`verify_profile_presets_082.py` 12 / 12；`verify_profile_rules_068.py` 25 / 25；`verify_worklog_104.py` 70 / 70；`verify_worklog_089.py` 78 / 78；`verify_worklog_103.py` 52 / 52；`verify_conversation_prompt.py` 26 / 26；`verify_intent_088.py` 22 / 22（failed=0）。窗口截图本人复核：标准模式 Dock 只有 3 项，问候语下显示模式说明；discipline 下恢复 6 项。

**审查判断（几处特意验证过的边界）：**

1. **「没有文件」不是错误。** `load_decl` 在 `NotFound` 时返回 `Ok(None)`，单测 `missing_file_is_none_and_never_creates_a_default` 同时断言 `!dir.exists()`——只读加载连目录都不建。这与旧实现「首次启动 create_new 写默认声明」是相反的行为，也是本轮最关键的口径变化。
2. **标准 Profile 不会被旧文件污染。** `migrate_legacy_decl` / `migrate_legacy_rules` 都以 `profile == standard_profile` 提前返回；两个单测分别断言 `!root.join("omnidesk-base").exists()`，即使用户数据根里躺着 105 时期的旧文件，标准模式也不会长出场景或铁律。
3. **迁移只复制、不删除、不覆盖。** 目标已存在直接跳过（单测写入「用户改过」后再次调用，内容仍是「用户改过」）；旧文件 `is_file()` 仍为真。坏文件不搬：解析失败返回 Err 且目标文件不存在。这是「不丢用户数据」的代码级证据。
4. **规则迁移失败不阻断提交。** `migration_warning` 与 `error` 是两个字段：`engine_submit_task` 只认 `error`。所以旧规则文件写坏时，用户看到提示但仍能正常对话——不会把「迁移没成功」变成新的功能故障。
5. **资产目录取运行中引擎，不取磁盘设置。** `profile_asset_scope` 从 `host.bridge.supervisor().lock().config()` 取 `profile` / `channel`，空值时退回 `profile_choice::standard_profile(channel)`。这条防止「顶栏改了 Profile、引擎还没重启」时把专业场景提示词注入到干净底座引擎上。
6. **场景预设是双条件。** `HomeCanvas` 的 preset 既要「声明里有这个入口」（`weekly` 映射到 `report`），又要 `rulesView.loaded`；任一不满足即 `null`，`HomeChat` 的 `weeklyScene` 判定随之不成立，随手记判定不会在无场景对话里触发。探针 `preset_requires_declared_scene` / `preset_requires_loaded_rules` 分别固定这两条。
7. **声明读完才敢下判断。** `declReady` 在 `profileDecl()` 的 `.finally` 里置位；场景校准与「标准模式」文案都以它为门控，避免启动瞬间把「还没读到」误报成「标准模式」。探针 `app_has_decl_ready` / `home_standard_copy_gated` / `app_passes_decl_ready` 覆盖。
8. **标准模式不给装行业规则包。** `profile_rules_load` 只在 `standard` 时填 `default_blocked_reason`，设置页据此禁用按钮并给出悬停说明；`load_rules` 本身不判断 Profile，语义分层清楚。同时保留「用户自己粘贴保存的规则照样生效」，只是不提供出厂包入口。
9. **Dock 的三种状态互斥。** 有声明 → 用声明；无声明且未失败 → 三项平台固有入口；读取失败 → 内置默认 6 项并提示。窗口探针实测 3 / 6 条色带，静态探针 `dock_business_only_on_failure` 固定分支条件。
10. **插件面回到官方。** `omnidesk-base.profile.patch.yml` 里 `skill-filesystem` / `tool-skill` 不再有 `disabled: true`（文件内仅剩 `tool-plugin-manager` 的禁用，属原有设置）；探针 `base_enables_skill_filesystem` / `base_enables_tool_skill` 断言。本人另做逐条比对：与 `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-web-app/presets/standard.patch.yml` 的 32 个插件 id 及 disabled 标记完全一致，无缺项、无多项、无状态差异（`evidence/od-arch-106/plugin-parity.json`）。这回答了验收口径第 3 条「插件面与官方 dsh-base 一致」。

**计划偏离的裁定：** 无架构偏离。三处补充（运行中 Profile 判定、迁移告警独立字段、首页标准模式说明与门控）都是在计划语义内的必要细化，未扩大范围，也未新增 Profile 管理器或热切换。

**复核中修掉的真实缺陷（不属于计划范围，但不能留着）：** 归类下拉（CategoryPicker）在 059 窗口测试里连续两次选不中分类——tag_after 停在 weekly。原因不是测试脚本：浮层改用 portal 渲染后，聚焦 effect 只依赖 [open, currentIndex]，而列表项要等 place() 算出 anchor 才挂载，effect 在列表项还不存在的那一帧跑完就不再重跑，焦点一直停在胶囊按钮上，上下键与回车都落不到选项。已改为 if (!open || !anchor) return 并把 anchor 加进依赖，同时给胶囊补「已展开时上下键直接移至列表项」与 Home / End 跳首尾。复跑 059 通过（weekly→writing，隐藏文案写作后落到 project），verify_weekly_093.py 108 / 108（新增 3 条焦点断言）。这条缺陷此前被 093 的静态断言漏掉，因为它只查 createPortal 与 window.innerHeight，不查聚焦时序。

**已知限制：** ①真实用户数据根（`~/OmniDesk`）上的端到端启动与真实模型实测未做——本轮全部是静态断言、Rust/前端单测、pytest 与 Xvfb 合成窗口；旧数据迁移在真实机器上的首次表现仍需用户确认。②切换 Profile 仍要重启引擎，界面会在重启前就按**旧** Profile 显示（这正是 `profile_asset_scope` 想要的），用户若只看界面变化会以为已切换，需以顶栏的「重启生效」提示为准。③Skill 文件仍共享 DSH home 下 `skills/` 一份（与官方 skill-filesystem 行为一致），不是按 Profile 隔离；`anysearch` 等技能在标准模式下也可被模型按需加载，这与「和官方 DSH 没差别」的目标一致，但与「纪检技能只在 discipline 下可用」不是同一件事。④`workIntent.ts` 的关键词路由仍在，只是被 preset 门控挡住，无场景时不参与。

**阶段决定：OD-ARCH-106 验收通过。** 建议下一步按用户口径做真实数据根 + 真实模型的端到端实测（标准模式启动是否只有 3 个入口、切 discipline 重启后场景与规则是否恢复、旧 `profiles/discipline.yml` 是否被复制而不是被吞）。本轮改动尚未提交、尚未推送。

---

## OD-ARCH-105 审查结论（ARCHITECT，2026-10-28）——通过（真实模型准确率待实测）

**结论：通过。** 随手记的判断口径已经从平台代码里移出：`conversationPrompt.ts` 不再写判断原则、JSON 示例或六要素追问，平台只交出「今天、当周文件、条目索引」这份数据和「写入由工作台执行」这条边界；判定协议也去掉业务分类，只剩 `append` / `move` / `note` 三个通用字段。换一套 Skill 就是另一套工作流，平台代码不用动。这一版同时把提示词显著变短：随手记平台段实测 **248 字**（不含注入的 Skill）。

**独立复核范围：** HEAD `473ccf1` + 本轮改动。逐文件核对 `conversationPrompt.ts`（note 分支只剩 headLines、写盘边界、索引、Skill 注入；无绑定时不臆测）、`workRecordVerdict.ts`（类型定义、parseVerdict 的字段类型校验、validateVerdict 的五类确定性拒绝、无 action 残留）、`HomeChat.tsx`（`plan.append`/`plan.move` 执行顺序、空计划返回 null、降级路径与取消分支未变）、`memory-agent/SKILL.md`（协议示例、每轮必给块、六要素、去向、周报归属）、两份验证脚本的断言口径。`runtime/` 零差异；未读真实工作记录与凭证。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `npx tsc --noEmit` 通过；`npm run build` 通过；`verify_worklog_104.py` **70 / 70**；`verify_worklog_089.py` **78 / 78**；`verify_worklog_103.py` **52 / 52**；`verify_writing_094.py` **98 / 98**；`verify_hover_hint_101.py` **30 / 30**；`verify_skill_bindings_082.py` **38 / 38**；`verify_intent_088.py` **22 / 22**；`verify_conversation_prompt.py` **26 / 26**；`cargo test --offline` **442 passed、0 failed**；`python3 -m pytest tests -q` **89 passed、30 subtests**。

**审查判断（几处特意验证过的边界）：**

1. **平台确实不再持有业务规则。** 探针把真实的 `memory-agent/SKILL.md` 注入后，从组装结果里去掉 Skill 原文再比对：`判断原则`、`六要素`、`逐字取自用户原话`、`omnidesk-verdict`、`append`、`moves`、`「本周报引用」` 均不出现；这些词只在 Skill 里能找到。这是「通用平台」这一诉求的直接证据，而非文档声明。
2. **协议没有业务分类。** `WorkRecordVerdict` 只有三个字段；`parseVerdict` 对旧形状 `{"action":…,"records":…}` 返回空计划且不报错，对 `{"append":"x"}` 这类类型错误整份作废。校验里没有任何「这句是指令还是事实」的判断。
3. **两个数组都空是合法判定。** 确认句、追问、纯回答都走这条：`accept_reply_only` 断言 `append`/`move` 为空且校验通过，`HomeChat` 返回 null 不碰文件。这正是「就是周一上午」不该写入的路径。
4. **追加与移动可以同时给。** `accept_append_and_move` 覆盖「先挪旧条、再补新条」；执行顺序先 `worklogMoveMany` 后逐条 `worklogAppend`，与计划一致。
5. **写重会被挡住。** `reject_append_and_move_same` 用同一段话同时出现在 `append` 与 `move` 的 `expectText` 里，整份作废——这是去掉 action 后新增的、真正必要的边界。
6. **确定性边界一条没松。** 原话连续片段、本周且不晚于今天、索引越界、同一位置移两次、原日期等于目标日期，逐条仍有断言；Rust 侧批量移动的原子性、备份、条目守恒测试全绿。
7. **降级路径未变。** 无判定块、JSON 不合法、校验不过仍按原文记当天并说明「模型未给出判定……未做指令识别」；取消分支仍不写盘。

**计划偏离的裁定：** 无。S1–S4 全部按计划落地。原计划中提到「预算收紧到 ≤700 字」，实际以「去掉注入 Skill 后的平台段」作为度量口径（248 字），比原口径更严格，已在探针里固定。

**已知限制（第一条是本次唯一未验证项）：** ① 真实模型在新协议下的判定准确率未验证——脚本只证明解析、校验、平台边界；模型是否稳定给出 `append`/`move` 需要用户端到端实测。② `workIntent.ts` 仍用关键词路由「周报 vs 随手记」，`weeklyQuadrants.ts` 仍有 `本周报引用` 常量；这两处属于输入路由与展示口径，不是对输入内容的语义判断，本轮未动。③ 索引仍只带最近 7 天、每条 80 字、最多 60 条。④ 本轮无窗口级验证。

**阶段决定：OD-ARCH-105 验收通过，随后提交并推送。** 用户原三句的实测结果仍作为后续独立项：若真实模型在新协议下漏块或误挪，先调 Skill 而不动平台代码。

---

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

## 架构符合性复核（2026-10-28，ARCHITECT）

对照用户初始规划「自制 UI + DSH CLI 底座 + 默认 Profile 等同官方 DSH-desktop + 加载专业 Profile 自动挂场景 Skill 与插件」逐项核对源码（static 证据）：

- **达标**：① 自制 React/Tauri UI + 隔离 dsh-home 内 DSH CLI（engine_supervisor.rs）；② 默认 omnidesk-base = dsh-base + web-app + 与官方 preset-standard 一致的插件面（omnidesk-base.profile.patch.yml，33 条）；③ 顶栏 Profile 切换 + 白名单 + 保存于 settings.v1.json，重启生效；④ discipline Profile 带独立资产目录（decl.yml 决定 Dock 场景入口、rules.json 决定场景规则与 Skill 绑定：memory-agent/memory-report/material-writing/project-agent），基础 Profile 无 decl 无场景规则=纯净底座；⑤ 规则与 Skill 注入按 profile_asset_dir 隔离加载（lib.rs:791-830）。
- **未达标（缺口）**：插件随 Profile 挂载未实现——discipline.profile.patch.yml 为空 `[]`，纪检场景无任何插件差异；anysearch/obsidian 等属 Skill 层而非 DSH 插件层，插件管理仍是全局面。Profile 切换需重启引擎（已在界面提示）。workstations/discipline/profile.yaml 是规划声明非生效配置（文件头已注明）。

**结论：骨架符合初始规划，场景-Skill-规则链路已闭环；「插件随 Profile 挂载」一条尚未落地，属已知缺口而非偏离。**
## OD-ARCH-113 审查结论（ARCHITECT，2026-10-29）——通过

**结论：通过。** 109–112 把「切换 Profile 自动装配」做通了，但资产页始终是只读的：少一个文件只能再切一次 Profile 或重启，想还原自己改过的文件只能去磁盘手工删。更麻烦的是 Skill 巡检只看 `SKILL.md` 在不在——正文被改过依然报「就绪」，这是假绿：用户看到绿灯，实际跑的是被改坏的版本。本轮补上就地修复通道，并把 Skill 判定改成逐文件比摘要。

**独立复核范围：** HEAD `7820751` + 工作区改动（含 104–112 未提交部分）。逐项核对 `profile_assets.rs` 的 `sync_bytes` 覆盖分支与 `backup_of` 调用顺序、`materialize_with` 与 `materialize` 的转调关系、`expected_skill_files` / `inspect_skill` 的来源与优先级、`lib.rs` 的 `profile_assets_repair` 与 `generate_handler!` 注册、`SettingsPanel.tsx` 的修复按钮/回执/一键装插件、`api.ts` 与 `types.ts` 的字段，以及新增的 3 条集成测试与 5 项静态检查。`runtime/` 零差异；未读真实工作记录、知识库与凭证。

**审查判断（几处特意验证过的边界）：**

1. **覆盖必须先备份，再写回。** `sync_bytes` 的 drift 分支里 `backup_of` 在写入之前，备份内容是用户原文而不是出厂内容；`repair_fills_missing_and_backs_up_drift` 直接断言备份文件里的字节，而不是断言「有个 .bak 文件」。顺序反过来就成了「先毁再备」，这类错误只能靠断言内容抓。
2. **日常路径不允许被这条新能力顺带放宽。** `materialize` 仍是 `materialize_with(.., false)` 的薄壳，只有 `profile_assets_repair` 传 true。切 Profile、启动装配走的还是原来那条「drift 一律不动」的路。`user_edit_is_preserved_and_reported_as_drift` 与 `embedded_skill_user_edit_is_not_overwritten` 仍在绿，说明默认语义没被改动。
3. **巡检不能只判存在。** 新的 `inspect_skill` 与装配共用 `expected_skill_files`（仓库优先、内嵌兜底），逐文件 sha256；优先级 drift → missing → stale → ready 有讲究：用户改过要压过「缺文件」和「有新版本」，否则一条 drift 会被别的状态盖掉。`inspect_detects_edited_skill_body` 只改正文不改文件名，正好把旧的「只看存在」判法证伪。
4. **修复是只读判定的反面，但不能因此变成隐式写盘。** 打开资产页仍只判定；修复只在点击后发生。`inspect_reports_without_writing` 保住这条边界。
5. **基础标准必须连修复都零写入。** `repair_standard_profile_still_writes_nothing` 断言不建目录、不落文件。否则「干净底座」这条产品前提会被一个修复按钮悄悄破掉。
6. **插件不混进修复。** 修复只做本地资产，插件安装需要联网和用户显式动作，走独立的「一键安装缺失插件」。这不是遗漏，是边界：把联网行为藏进「修复」会让用户以为点一下只是补本地文件。

**独立实跑结果（本人执行，非采信 STATUS.md）：** `cargo test --offline` 423 lib + 集成 2+1+13+6+5+10+**17**+3+1 全绿、EXIT=0；`cargo test --offline --test profile_assets_host` 17 passed；`npx tsc --noEmit` 无输出；`npm run build` 通过（仅既有 chunk 体积提示）；`python3 -m pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 29/29；`verify_profile_switch_110.py` 22/22；release 重建 3m05s 无告警，二进制 7,564,592 字节；前端产物里「一键安装缺失插件」与 `profile_assets_repair` 各命中一次。

**真实分发预览（收尾补跑，非模拟）：** `preview_embedded_assets_111.py` 7/7 通过，确认 113 没有回归内嵌装配——`OMNIDESK_REPO_ROOT` 指向不存在的目录时，8 个 Skill、三类 references 与 Profile 配置仍完整装出，且与仓库源逐字节一致。

**已知限制（不作缺陷计）：** 修复不装插件（联网，必须独立触发）；修复不改 `runtime/`、不碰 `~/.dsh`；改 `skills/` 后仍需重建二进制才进用户机器；真实模型端到端仍未测；`worklog.rs::AppendReceipt::failure` 的 dead_code 告警与本轮无关。

**未提交未推送：** 按批准范围只执行，工作区改动原样保留。
