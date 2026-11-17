# 执行状态

## OD-ARCH-175 阶段 3 执行记录（子代理，2026-11-17）

- 开工基线：`git status --short` 为 `M PLANS.md`、`M REVIEW.md`；HEAD `5754da202c`。执行期间 STATUS.md 出现阶段 1 子代理追加的只读核查记录，本阶段原样保留，未改写其内容。
- 改动清单：
  - `desktop/src/api.ts`：新增 `planModelRouteEffort` 纯函数与 `collectModelEffortEvidence`；`setModelRoute` 移除两处阻断式 throw。目录非空声明按集合判定（情形 2 正常、情形 3 改发 `default`）；目录不可用/模型未列出/目录空声明时再看 Profile `modelEntries.reasoningEfforts`（`false` 或空对象视为声明为空）；两处都无法确证时照所选档位提交并返回 advisory。返回值新增 `reasoningEffort` 与 `advisory`，并留阶段 2/4 TODO。
  - `desktop/src/types.ts`：`ModelCatalogEntry.efforts` 改为可选（`ModelCatalogEffort[] | null`），反映归一化目录可能缺项。
  - `desktop/src/shell/ModelPicker.tsx`：档位提示改为「未声明思考档位；若不支持会按默认档切换。」/「该模型不支持「X」，切换会按默认档。」；保留 `data-testid="effort-advisory"`；模型列表对 `efforts` 可选做空值保护。
  - `desktop/src/shell/HomeChat.tsx`：切换成功时优先展示 `r.advisory`（一句话、info 提示条）；情形 3 用 `setModelEffort(r.reasoningEffort)` 同步界面到实际生效的默认档；留阶段 2/4 运行期回退通知 TODO。
  - `tests/verification/desktop/verify_model_route_precheck_175.py`（新增）：用 `tsc --ignoreConfig` 将真实 `api.ts`/`engineAdmission.ts` 转译到临时目录，在 Node 中 mock `invoke` 调用真实 `shell.setModelRoute`；覆盖情形 1（未列出/目录不可用/efforts 缺失/空目录+Profile 未声明）、情形 2（目录集合内/Profile 集合内）、情形 3（目录集合外/Profile false/Profile 空对象/未列出但 Profile 集合外）、requested default 不做预检；断言实际下发档位与 advisory 文案。
  - `tests/verification/desktop/verify_home_refine_202.py`：更新 `effort_advisory` 断言到新文案（原断言依赖「可能被忽略」）。
  - `STATUS.md`：本小节。
- 判定口径：DSH 归一化目录把「没有 reasoning 声明」与「显式声明为空」都压成 `efforts: []`（`engine/omnidesk_engine/web_api_adapter.py` 只读核对），所以空目录不能单独当必败证据；能读到 Profile `reasoningEfforts: false` 或空对象时才本地改发默认档。无法区分 raw `reasoning` 是否显式为空数组的情形，仍可能把「声明为空」当作「未声明」而尝试一次；属已知限制，未改 engine/runtime。
- 实际命令与结果：
  - `cd desktop && npx tsc --noEmit` → exit 0。
  - `python3 tests/verification/desktop/verify_model_route_precheck_175.py` → cases 12/12、static 11/11，exit 0；证据 `tests/verification/desktop/evidence/od-arch-175/precheck.json`（evidence 被 `.gitignore` 忽略）。
  - `python3 tests/verification/desktop/verify_home_refine_202.py` → 39/40，唯一失败 `home_scene_backdrop`（`App.tsx` 未含 `app-scape-art`；本轮未改该文件，属既有失败）。
  - `git diff --check` → exit 0。
- 证据等级：文件事实（源码与 `web_api_adapter.py` 只读核对）；静态检查（`verify_home_refine_202` 文案断言）；脚本运行（TypeScript 编译 + Node 调用真实 `setModelRoute` + mock invoke）。未做真实桌面窗口点击与真实模型调用。
- 未验证项：真实窗口下 ModelPicker/HomeChat 提示条的渲染与自动消失；真实 volcengine 切换后 DSH 是否按预期接受 `off` 或被阶段 2 回退链路处理（阶段 2/4）；raw `session/modelCatalog` 中 `reasoning` 显式为空与未声明的区分（归一化目录丢失该信息）。
- Git：HEAD `5754da202c`；未提交、未推送、未打标签。`desktop/src-tauri/`、`runtime/`、`engine/` 未改；模型路由与凭证未动；未接触真实业务材料。

## OD-ARCH-174 阶段 C：核心技能正文重写（2026-11-17，待复核）

- 重写三个技能正文，frontmatter 未动：`memory-agent` 1754 → 1228 字符（正文 1578 → 1052，66.7%）；`memory-report` 1099 → 749（正文 950 → 600，63.2%）；`period-review` 1820 → 1268（正文 1668 → 1116，66.9%）。三份全文比例均在 68%–70%。
- 删除：平台写盘边界/材料已提供等重复说明、真实地名示例、占位符还原机制、周报中转说明、按需加载其他技能（与主指针冲突）、自检清单、四组 Markdown 围栏（标题与顺序逐字改写成紧凑序列）、草稿/确认的平台流程说明。保留：7 条随手记动作规则、日期窗口、verdict 块与四字段 JSON、六要素、去向、“本机是工作记录唯一存储地，可记录案件/审查调查日常工作事实”；周报五段标题、`本周报引用`、`不得补造`、精确短语“只有用户明确确认后才落盘”、`不覆盖`、`-2` 规则；复盘四周期完整标题序列、`#wr-`/`#t-` 引用格式、`不补造`/`不得虚构`、只交 Markdown 正文、数据边界。
- verify：`verify_skill_bindings_082.py` 69/69、`verify_worklog_089.py` 86/86、`verify_wr_503.py` 58/58、`verify_wr_505.py` 71/71 全过。
- mock：`run_mock_session.py --label stageC` 的 excel / weekly / knowledge `succeeded`（weekly 对应 memory-report），tool_probe 30 秒上限内仍 `running`；note 与 review 用 `stage2_prompt_session.py` 补跑，user 634 / 701 字符，均 `succeeded`；`skill-catalog` 2401。
- 真实模型（overlay `~/OmniDesk/app/config/omnidesk-model.yml`，weekly 与 knowledge 分开 root 跑）：weekly `succeeded` 20.07s / ttfb 17.56s / 8375 tokens（cacheRead 7808）；knowledge `succeeded` 66.24s / ttfb 61.99s / 14310 tokens（cacheRead 13568），三张卡落 `workspace/staging/kb/知识库/方法/`。weekly 产出四节（本周完成/进行中/风险/下周计划），与 0.2.30 smoke 的四节行为一致；`run_synthetic_tasks.py` 发的是裸提示词，不经过桌面五段契约。
- **关键限制：** 两次真实任务 `tool_event_count=0`，模型没有调用 `skill`，因此重写后的正文没有被真实模型读取；真实回归只证明任务可跑通、产出仍正常，正文质量与模型读后行为未验证。
- 同步：`sync-skills.sh`（沿用 `/tmp/od174-bin/realpath` 包装）exit 0，`--check` 一致；未改其他 7 个技能、`runtime/`、`aiDraft.ts`、凭证或模型路由。
- 证据：`tests/verification/desktop/evidence/od-arch-174/stageC.md`、`mock-run-stageC.log`、`mock-prompt-note-review-stageC.log`、`real-weekly-stageC.log`、`real-knowledge-stageC.log`（evidence 被 `.gitignore` 忽略）。
- Git：HEAD `f1e06a3`；新增 `M skills/memory-agent/SKILL.md`、`M skills/memory-report/SKILL.md`、`M skills/period-review/SKILL.md`；未提交、未推送、未打标签。

## OD-ARCH-174 阶段 B：目录瘦身（2026-11-17，已按裁决收缩，待复核）

- 最终改动：仅 `skills/document-review/SKILL.md` 保留 `disable-model-invocation: true`；`material-writing` / `knowledge-search` / `anysearch` 的标记已撤回（前两个是绑定指针要求模型读正文，anysearch 是阶段 1 明确保留的按需检索能力）。其余 6 个技能未动；未改 `runtime/`、`aiDraft.ts`、凭证或模型路由。
- 裁决依据：全量 4 标记曾把目录压到 2039 字符（−479），但实测 `skill` 工具会拒绝模型读取被标记技能（`Error: skill "material-writing" is not available for model invocation`，69 字符、无正文），与阶段 A/A2 指针冲突。ARCHITECT 裁决“能力优先、目录开销其次”，仅保留无绑定指针指向的 `document-review`。
- 最终 mock（`run_mock_session.py --label stageB-final`）：`skill-catalog` 2518 → 2401 字符（−117，−4.65%），目录技能 19 → 16；只有 `document-review` 消失，`anysearch` / `material-writing` / `knowledge-search` 仍在目录中。4 条会话均为 2401。
- 同步：`sync-skills.sh` 在 macOS 上因 BSD `realpath` 不支持 `-m` 直接失败，未改脚本，用 `/tmp/od174-bin/realpath` 兼容包装后执行；最终状态先报 3 个 Skill 漂移，同步后 `--check` 一致（exit 0）。隔离副本核对：仅 `document-review` 带标记。同步副作用：全量同步时删除旧备份 `knowledge-search/SKILL.md.bak-1794761541` 并重新物化两个 references 目录（同步前已有漂移），均为隔离运行时副本。
- 回归（最终状态补跑）：`verify_conversation_prompt.py` 33/33、`verify_skill_bindings_082.py` 69/69、`verify_skill_scope_107.py` 33/33、`verify_worklog_089.py` 86/86 通过；`sync-skills.sh --check` 一致。
- 未验证：真实模型行为；用户手点 `document-review` 的真实窗口路径；macOS 下 `sync-skills.sh` 的直接可运行性。
- 证据：`tests/verification/desktop/evidence/od-arch-174/stageB.md`、`mock-run-stageB-final.log`、`stageB-probe-material-writing.log`（全量试验的阻断证据，evidence 被 `.gitignore` 忽略）。
- Git：HEAD `f1e06a3`；`skills/` 下唯一改动是 `M skills/document-review/SKILL.md`；未提交、未推送、未打标签。

## OD-ARCH-174 阶段 A2：knowledge 专用绑定键（2026-11-17，待 ARCHITECT 复核）

- 改动：`samples/profiles/discipline.rules.json` 新增 `"knowledge": {"skills":["knowledge-search"],"max_injected":1}`；`conversationPrompt.ts` 的 `ConversationPreset` 与组装输入增加可选 `knowledgeSkills`，知识指针改从该字段取首个非空项（不再从 `preset.skills` 找）；`HomeCanvas.tsx` 独立读取 `skill_bindings["knowledge"]` 并透传；`HomeChat.tsx` 只加字段透传。未改 `aiDraft.ts`、`runtime/`、凭证或模型路由。
- 普通对话的透传理由：未分类/自定义分类对话 `preset=null`，知识检索随时可能发生；因此 `HomeCanvas` 用独立 `useMemo` 读绑定，`HomeChat` 透传 `knowledgeSkills` 给组装器。不混入 `preset.skills`/`skillsByFlow`，四场景主指针不受影响。
- 规则校验：`profile_rules.rs` 的绑定键校验无场景白名单（只要求非空、每场景 ≤3、总键数 ≤12），`knowledge` 键直接通过；出厂规则单测同步改为「`knowledge-search` 只进专用 `knowledge` 绑定，其余场景绑定仍不得预注入它」。设置页把非固定流程键作为额外键原样保留，整表保存不会冲掉。
- 决策记录：`docs/decisions.md` 在 D-035 之后新增 D-108，写明指针机制使注入成本从 4901 字正文降为 151 字指针、原「没有注入点」前提失效；`knowledge` 绑定独立于四场景主指针；复审触发四条。
- 探针（真实源码编译后 Node 调用，与 `HomeCanvas` 同构）：weekly-note 无检索 → `memory-agent`；weekly-report → `memory-report`；writing → `material-writing`；project → `project-agent`；weekly-note + 检索上下文 → 主指针仍 `memory-agent` + 知识指针（855 字符）；writing + 检索上下文 + `knowledgeSkills` → 主指针 `material-writing` + 知识指针（1115）；普通对话（`preset=null`）+ 检索上下文 + `knowledgeSkills` → 只有知识指针（235）。知识指针 151 字符。
- 结果：`cd desktop && npx tsc --noEmit` exit 0；`verify_conversation_prompt.py` 33/33、`verify_skill_bindings_082.py` 69/69、`verify_worklog_089.py` 86/86、`verify_work_review_213.py` 25/25、`verify_skill_scope_107.py` 33/33、`verify_profile_assets_109.py` 41/41 通过；`verify_profile_rules_068.py` 26/27，唯一失败仍是既有 `16_keywords`（本地拦截词期望 16、实际 2）；`verify_capabilities_072.py` 因本机无 Xvfb 未运行；`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1200 passed / 0 failed / 1 ignored（49.02s）。
- 未验证：真实模型检索行为；现有安装的 `~/OmniDesk/app/profiles/discipline/rules.json` 未迁移（只改仓库出厂资产，按约束不动用户数据），需「恢复默认规则包」或重装后 `knowledge` 绑定才生效；设置页不显示可勾选的 `knowledge` 固定行（仅作为额外键保留）；真实窗口提交链路未运行。
- 证据：`tests/verification/desktop/evidence/od-arch-174/stageA.md` 第 6 节与 `stageA_probe.json`（evidence 被 `.gitignore` 忽略）。
- Git：HEAD `f1e06a3`；本阶段在阶段 A 工作树上新增 `M docs/decisions.md`、`M desktop/src/shell/HomeCanvas.tsx`、`M desktop/src/shell/HomeChat.tsx`、`M desktop/src-tauri/src/profile_rules.rs`、`M samples/profiles/discipline.rules.json`、`M verify_worklog_089.py`；未提交、未推送、未打标签。

## OD-ARCH-174 阶段 A：指针补全与绑定修正（2026-11-17，待 ARCHITECT 复核）

- 改动：`desktop/src/conversationPrompt.ts` 新增 `defaultSceneSkills()` 与 `knowledgeSkillPointer()`（`KNOWLEDGE_POINTER_MAX_CHARS = 260`）。默认分支不再无条件取 `presetSkillsFor(preset, "report")`：仅当 `preset.key === "weekly"` 且 `mode="report"` 或宿主传了 `worklog` 时用 `weekly-report` 绑定，writing / project / 无意图一律取本场景 `preset.skills`。`knowledgeContext` 非空且 `preset.skills` 绑定 `knowledge-search` 时，在检索上下文后追加 151 字符检索指针（先读 `库/知识库/索引.md`、按标签定位、只加载 1–2 张卡、未命中如实说明）；未绑定或没上下文不注入，正文不入提示词。note / daily-summary / period-review 分支与 weekly-report 指针不变。
- 选择最小改法的理由：`preset.key` 已是场景键（与 `scenes.ts`、`HomeCanvas` 一致），周报流程用宿主已传入的 `worklog`/`mode` 判定，不改 `HomeCanvas`/`HomeChat`/`aiDraft.ts`，也不新增 preset 字段。
- 验证脚本：`verify_skill_bindings_082.py` 新增 4 条断言（writing/project 残留 `skillsByFlow.report` 时仍只取场景绑定；weekly 无 mode/worklog 用 `memory-agent`；weekly 有 worklog 用 `memory-report`）；`verify_conversation_prompt.py` 新增 7 条 knowledge 指针断言（上下文顺序、≤260、正文不入、无绑定/无上下文不注入）。旧断言全部保留。
- 四场景直接组装（真实源码编译后 Node 调用，与 `HomeCanvas` 同构）：weekly-note → `memory-agent`（642 字符）、weekly-report → `memory-report`（918）、writing → `material-writing`（899）、project → `project-agent`（865）；knowledge（写作场景追加 `knowledge-search` 绑定 + 检索上下文）→ 主指针 `material-writing` + 检索指针 `knowledge-search`（151 字符）。
- 结果：`cd desktop && npx tsc --noEmit` exit 0；`verify_conversation_prompt.py` 33/33、`verify_skill_bindings_082.py` 62/62、`verify_worklog_089.py` 85/85、`verify_attachments_077.py` 25/25、`verify_resume_token_savings.py` 11/11、`verify_ai_draft.py` 27/27 通过。`verify_profile_rules_068.py` 26/27，唯一失败 `16_keywords`（期望 16、实际 2）只读 `discipline.rules.json` 的本地拦截词数量，HEAD 资产即本地零拦截，属既有失效断言；`verify_skill_flow_injection_108.py` 未运行（macOS 无 Xvfb，启动即因 `/private/tmp` 路径判定拒绝清理）。
- 未验证：真实模型行为；真实窗口提交链路（`HomeCanvas` 构造与探针同构，静态比对）；默认 Profile 的 knowledge 指针尚未可达——出厂规则包没有任何 `knowledge-search` 绑定，按任务约束未改 `samples/profiles/`，需规则资产绑定后才生效；主技能指针仍带“只用这一个技能”，knowledge 检索指针追加后存在两条指引的文案张力，留待真实模型观察/阶段 C 收敛。
- 证据：`tests/verification/desktop/evidence/od-arch-174/stageA.md`、`stageA_probe.js`、`stageA_probe.json`（evidence 被 `.gitignore` 忽略）。
- Git：HEAD `f1e06a3`；开始时已有 `M PLANS.md`，本阶段新增 `M desktop/src/conversationPrompt.ts`、`M verify_conversation_prompt.py`、`M verify_skill_bindings_082.py`；未提交、未推送、未打标签。

## OD-ARCH-173 真实模型 smoke 补录（2026-11-17，用户授权换 key 后执行）

- **根因修正（文件事实 + 脚本运行）：** OmniDesk 的 OD_Key 是旧账号 key（OpenDesign 侧 free 档、钱包 0），与用户当前 Codex 通道的新 key 不同。已把 OpenDesign 路由配置中的现行 key 同步到 ~/OmniDesk/runtime/dsh-home/.credentials.yaml（原文件已备份 .bak-odkey-*），并移除 ~/OmniDesk/app/config/omnidesk-model.yml 中端点不支持的 reasoningEffort: off 两行（备份 .bak-odarch173-*）。全程未读取或打印任何密钥值，只比对前缀与长度。
- **逐模型实测（合成周报任务 ×5，阶段 0 的 402/403 结论复现）：** eco/glm/gpt-6-luna/minimax 均 403 free_workspace_flash_only；deepseek-v4.1-flash 402 余额不足（旧 key）。换新 key 后 eco 通过。
- **真实模型 smoke（opendesign/deepseek-v4.1-flash-eco，隔离根 /tmp/od-real-final，证据等级：真实模型输出）：**
  - Excel：succeeded，ttfb 54.0s；模型先调 skill(office-xlsx) 读正文再动手——阶段 2 的 skill-first 行为在真实模型 + PTC preset 下确认；产物 demo-budget.xlsx 落盘。
  - 周报：succeeded，ttfb 10.6s/总 14.1s，8.7k tokens；模型按材料直接成稿（四节结构，未调工具）。
  - 知识整理：succeeded，ttfb 16.0s；三张卡片落 cards/。缓存命中正常（cacheRead 7.9k/11.5k）。
  - 应用自身配置复测：修复后的 omnidesk-model.yml 走同引擎路径，周报 succeeded（22.1s，ttfb 18.3s）。
- **剩余未验证：** Windows 构建、配置解释页真机截图（锁屏）、verify_skill_flow_injection_108.py 真机运行、知识类任务未触发 skill 读取（模型判断直接落卡，非回归，记录观察）。

## OD-ARCH-173 阶段 6：验证、发布与回退（2026-11-17，待 ARCHITECT 复核）

- 全量回归：`cd desktop && npx tsc --noEmit` exit 0；`npm run build` exit 0（361 modules，仅既有 chunk 体积提示）；`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` 1200 passed / 0 failed / 1 ignored（49.41s）；`cargo check --offline --all-targets` exit 0；`PATH="$HOME/.cargo/bin:$PATH" npm run tauri build -- --no-bundle` exit 0，release 二进制 1m 33s；`bash scripts/sync-app-version.sh` 确认 tauri.conf 已是 0.2.30；`git diff --check` exit 0。
- UI/Profile 脚本：运行 54 个非 Xvfb 脚本，41 通过、13 失败；失败项全部在 `git archive HEAD` 快照复现或属 macOS 环境限制（`runtime/deepseek-harness` 子模块未初始化、`TMPDIR` 不在 /tmp、已移除的 WeeklyQuadrants.tsx），无本版本引入的失败。逐项分类见 `tests/verification/desktop/evidence/od-arch-173/stage6.md`。
- 遗留断言更新（3 个脚本，均已通过语法检查）：`verify_skill_bindings_082.py` 把「六要素全文在提示词」改为「指针 + 读取指令在、正文不在」，证据 snippets 改为指针；58/58 通过。`verify_worklog_089.py` 把全文包含拆成指针/读取指令/正文不注入三条，剥离对象改为指针，并把 HEAD 既有失效的索引文案断言对齐为「已有记录（最近 14 天」；85/85 通过。`verify_skill_flow_injection_108.py` 探针改为「主技能：memory-agent/memory-report」，新增正文不得命中的两条断言；本机 macOS 无 Xvfb，未运行，仅 py_compile。
- 版本与日志：`VERSION` 0.2.30；`desktop/src-tauri/tauri.conf.json` 0.2.30；`desktop/package.json` 0.2.30；`CHANGELOG.md` 的 `[Unreleased]` 收口为 `[0.2.30] - 2026-11-17`，写清效率（system 5396→2983、四动作 user 段降 61%–72%）、安全（free-search 依赖清理、输出上限、保存/切换预检）、成熟（配置解释页）与 Known limitations。
- 播种迁移：`seed_home_from_package` 只在缺失时复制，不覆盖已有 Profile；日常装配把内容不同的文件标 drift 并保留；只有用户点「修复缺失 / 恢复出厂内容」才备份后写回。本机 `~/OmniDesk/runtime/dsh-home/profiles/discipline` 仍含 `dsh-free-search`（只读核对，未改用户数据）。发布工作流会在 tauri build 前用 `prepare-runtime-assets.sh` 重建 seed，新装包干净；现有用户需点恢复出厂或重装，记为已知事项。
- 未验证：真实模型 smoke（402 余额不足）；Windows 安装器；`verify_skill_flow_injection_108.py` 真实窗口运行；配置解释页真实窗口截图；GUI/真实模型下的 Skill 读取行为。
- 交付：release 提交 `1d07d91bba`（`docs(release): 0.2.30 OD-ARCH-173 成熟高效安全基线`），已推送 `origin/main`（`296b160d0c..1d07d91bba`）；未打标签，未 force push；提交后工作树干净，无遗漏改动。

## OD-ARCH-173 阶段 5：可观测性与配置解释（2026-11-17）

- 改动：新增 `desktop/src-tauri/src/diagnostics.rs` 与只读命令 `diagnostics_metadata`（`lib.rs` 模块声明/注册/命令体）；`SettingsPanel.tsx` 新增「配置解释」分页；`api.ts` 增加绑定；`types.ts` 增加 `DiagnosticsMetadata` 类型。只读展示生效路由、接口/地址、思考档位与声明档位、上下文窗口、最近会话的提示词分段字符与注入字符、工具插件声明与实际工具名、Skill 概况；每项标注来源。未改 `HomeChat.tsx`/`timelineModel.ts`、`runtime/`、模型路由、凭据、`samples/` 或时间线布局。
- 数据口径：路由读 `config/omnidesk-model.yml`（provider/model/reasoningEffort + api/baseURL），不读 `.credentials.yaml`；提示词分段对齐 `scripts/diagnose-omnidesk.py` 的 8 类分桶与 `\n\n` 分段；会话优先 `zstd -dc`，无 `zstd` 退到未压缩 `.jsonl`；超 96 MB 不解析。返回值只含字符数、工具名与事件计数，不含提示词原文、用户正文、工具参数或凭据；单测对序列化结果断言不含正文。
- 验证：`cd desktop && ./node_modules/.bin/tsc --noEmit` → exit 0；`npm run build` → exit 0（仅既有 chunk 体积提示）；`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1200 passed / 0 failed / 1 ignored（`diagnostics` 7 个单测 + 1 个隔离根证据测试，后者需显式环境变量）。隔离 fixture（`/tmp/od-stage5`，合成内容）跑通 `diagnostics_metadata`，输出 `stage5-metadata.json`：preset `discipline-core`、9 个工具插件、system 1551 字符/6 段、工具 3 次、审批/重试/压缩各 1 次。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage5.md`、`stage5-metadata.json`、`cargo-test-lib-stage5.log`、`tsc-stage5.log`、`build-stage5.log`（evidence 被 `.gitignore` 忽略）。
- 未验证：当前 macOS 会话锁屏，真实窗口截图失败（`screencapture` 全黑，窗口抓图报 `could not create image from window`），也没有可用的浏览器/Playwright 环境；Tauri IPC 仅由单测直接调用 `build()` 验证，未从 WebView 点击；真实 `session.v4.jsonl.zstd` 解压、真实模型 `compat` 展示与 token 估算未验证；逐轮时间线/审批等待/缓存命中未展示（按任务说明不做时间线改版）。
- 计划偏差：UI 截图未取得；不提 token 估算；不展示当前压缩状态，只展示 compact 事件计数；不重做任务时间线。
- 未提交、未推送、未打标签。

## OD-ARCH-173 阶段 4：模型接入成熟度（2026-11-17）

- 改动：`desktop/src-tauri/src/model_settings.rs` 新增 `declared_efforts()` / `check_effort_capability()`，`save()` 在写盘前校验默认提供商/默认模型的 `reasoningEfforts`；`desktop/src/SettingsPanel.tsx` 保存前增加端点与档位预检；`desktop/src/api.ts` 的 `setModelRoute` 在写覆盖文件前用本地模型目录校验显式档位；`desktop/src/types.ts` 补 `modelEntries` 只读字段。未改 `lib.rs`、`dsh_settings.rs`、`HomeChat.tsx`、`runtime/`、`samples/`、模型路由与凭据。
- 预检口径：保存时先调既有 `model_settings_probe`（10 秒超时，只传凭据引用名或本次输入，不读明文）；401 / 402 / 403 阻止保存并分别给认证、额度、套餐权限文案，404/405 只提示（部分端点不提供模型列表），网络/协议不支持/内置目录命中不阻止。档位预检读 Profile 的 `reasoningEfforts`：声明 `false` 时收敛为“跟随模型默认”并提示，声明集合不含所选档位时阻止并列出可用档位；已保存的自定义路由未声明档位时收敛并提示；不能判定时只提示“未能本地验证”。切换路径读 DSH `session/modelCatalog` 的 `efforts`，未声明或不在集合内时阻止切换并说明原因。
- 验证：`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1193 passed / 0 failed；默认并行仍与阶段 3 一样有 3 个 spawn 用例因本机并发权限失败，单线程通过。`cd desktop && ./node_modules/.bin/tsc --noEmit` → exit 0。新增 Rust 单测覆盖档位声明缺失 / `false` / 集合命中 / 集合未命中 / `default` / 思考模式关闭六类分支，失败时断言不写盘。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage4.md`、`cargo-test-lib-stage4.log`、`tsc-stage4.log`（evidence 目录被 `.gitignore` 忽略）。
- 未验证：真实端点对 402/403/404 的响应文本与预检效果（阶段 6 smoke test）；GUI 端到端；切换路径的认证/额度预检（当前只做本地档位预检）；尚未保存的新端点档位能力；模型目录持久化缓存（DSH 未提供 ETag/checkedAt）。
- 未提交、未推送、未打标签。

## OD-ARCH-173 阶段 3：工具面与安全约束（2026-11-17）

- 改动：`samples/profiles/discipline.profile.json` 删除 `dsh-free-search` 依赖（保留 `dsh-cost-meter`）；`desktop/src-tauri/src/profile_assets.rs` 的 `DISCIPLINE_PLUGINS` 同步删除该插件需求；`samples/profiles/discipline.profile.patch.yml` 为 `discipline-core` 与 `work-review-core` 钉住 `tool-fs`（2000 行 / 51200 字节 / 单行 2000 字符）与 `tool-fs-search`（glob 100、grep 250 条、单行 500 字节）上限。未改 `conversationPrompt.ts`、`runtime/`、TS/React、模型路由与凭证。
- 工具面收敛：discipline-core 保留 11 个模型可见工具（bash/pwsh 按平台二选一、read/write/edit、glob/grep、skill、ask_user_question、web_fetch、run_code、present）；work-review-core 保留 7 个且无 shell/PTC/present。逐项保留理由与参数、输出、路径、并发、安全现状、缺口见 `evidence/od-arch-173/tool-matrix.md`。
- 审计结论：写盘边界为 `session cwd + /tmp + os.tmpdir`，OmniDesk 任务 cwd 即本地数据根，知识库仍走 OD-ARCH-152 暂存→宿主落库，单测无回归；读不受根约束、工具结果无统一脱敏、ask-user 无宿主卡片、preset 不在 turn 间切换，均记缺口，未改 runtime。
- 验证：`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` → 1187 passed / 0 failed（默认并行时有 3 个 spawn 用例因本机并发权限问题失败，单独与单线程均通过）；`run_mock_session.py` 用 sample patch+manifest 启动 excel/weekly/knowledge/tool_probe 四条会话全部 succeeded（tool_probe 实际 1 次 bash 调用）；`verify_tool_surface_154.py` 7/7、`verify_work_review_213.py` 25/25、`verify_profile_assets_109.py` 41/41、`verify_conversation_prompt.py` 26/26 通过。`verify_plugin_scope_104.py` 27/34，7 个失败均为阶段 1 或 office 包漂移造成的既有失效断言，与本阶段无关。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage3.md`、`tool-matrix.md` 与同目录 cargo/mock/verify 日志（evidence 被 `.gitignore` 忽略）。
- 未验证：真实模型行为；既有安装与 `runtime/dsh-home-seed` 仍含 free-search，迁移归阶段 6；未跑 tsc 与 GUI 端到端。未提交、未推送、未打标签。

## OD-ARCH-173 阶段 2：Skill 按需加载（2026-11-17）

- 只读验证通过，已执行第二步。原生命令 `skill` 与 PTC 下 `run_code` 内 `tools.skill` 都能拿到 Skill 全文（合成会话：`tool/result` 1358 字符含 `<skill_content>`，下一轮请求命中正文标记；PTC 直接调 `skill` 会报 “only run_code is callable directly”，改程序内调用成功）。
- 改动仅 `desktop/src/conversationPrompt.ts`：新增 `skillPointer`/`skillPointers`，四个动作（note / report / daily-summary / period-review）不再注入全文，改为 ≤260 字符的 name + description + 读取方式；每个动作只留一个主 Skill。note 分支只放行一次 `skill` 读取，其他工具仍禁用。
- mock 合成会话前后（user 段字符）：note 2131→636（−70.2%）、report 1446→558（−61.4%）、summary 1450→487（−66.4%）、review 2222→613（−72.4%）；system 5396、`skill-catalog` 2518、`time-context` 261 未变。任务说明预期 `skill-catalog` 下降；实际 2518 是 DSH 注入的 19 个 Skill 目录，不是桌面全文，下降在 user 段。
- 验证：`cd desktop && npx tsc --noEmit` exit 0；`verify_conversation_prompt.py` 26/26、`verify_worklog_104.py` 72/72、`verify_wr_503.py` 58/58、`verify_wr_505.py` 71/71 通过；`verify_skill_bindings_082.py` 56/57（`note_contains_clean_directive` 仍按全文注入断言）；`verify_worklog_089.py` 81/83（`note_six_elements_from_skill` 按新设计失败，`note_has_index_data` 为 HEAD 既有的失效断言）；`verify_wr_506.py` 57/58 的 `backlink_review_jump` 在 HEAD 工作树同样失败，与本改动无关。
- 未改 `aiDraft.ts`、`HomeCanvas.tsx`、`samples/profiles/`、`desktop/src-tauri/`、`runtime/`、模型路由与凭证；未改既有验证脚本（不在允许文件清单）。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage2.md` 与同目录前后提示词快照、会话日志、探针日志（evidence 被 `.gitignore` 忽略）。
- 未提交、未推送、未打标签；HEAD `6768f8b`，Git 跟踪改动仅 `desktop/src/conversationPrompt.ts`。

## OD-ARCH-173 阶段 1：提示词装配（2026-11-17）

- 改动 `samples/profiles/discipline.profile.patch.yml`：移除顶层 `web-search-free` insert；`discipline-core` / `work-review-core` 的 persona prefix 换成 OmniDesk 台账助手一句；删除检索预算与 `advanced_search` 等工具选择规则；`tool-web` 保留 `fetch`、关闭 `search`；`preset-standard` 未动，作为回退基线。
- 未改 `desktop/src` TS/TSX、`engine/`、`runtime/`、`omnidesk-base`、模型路由与凭证。
- mock 复测（非真实模型）：system 提示词 5396 → 2983 字符（去掉 PTC 测量），`web-search` 段 1919 → 0，纪律规则 552 → 283；保留 PTC 时为 15252 字符，PTC 类型注入占 12098 字符，阶段 1 的 ≤1000 tokens 在保留 PTC 时不可达。
- 日期位置：`dsh-time-context` 以 `user/message`（`time-context`）注入，不在 system 段；会话元数据 `time-context=261` 字符。未改代码。
- 搜索入口：`desktop/src` 无 free-search/web-search UI 开关；缺口是 `discipline.profile.json` 与 `profile_assets.rs::DISCIPLINE_PLUGINS` 仍列 `dsh-free-search`（不在本阶段获批范围，未改）。
- 验证：`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib` → 1187 passed / 0 failed；未设该环境变量时 3 个 spawn 测试因 `python3` PATH 权限失败（环境问题）。未改 TS，`npx tsc --noEmit` 未跑。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage1.md` 与同目录三个 mock 日志、`cargo-test-lib.log`。该 evidence 目录已被 `.gitignore` 忽略，并曾于 2026-11-17 被归档；本任务从 `MyWorkStation-archive/2026-11-17/test-and-process-evidence/` 取回 od-arch-173 后继续追加，未恢复其余截图。
- 未提交、未推送、未打标签。

## 证据与历史报告全面归档（2026-11-17）

- 将 `tests/verification/**/evidence/`、`tests/verification/desktop/od-*/`、`tests/reports/`、`docs/archive/`、`docs/plans/` 与 `audit/` 移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/test-and-process-evidence`，共 37 个目录、1065 个文件、约 11 MB，并生成 `MANIFEST.sha256`。
- Git 保留全部可复跑测试脚本、夹具、架构契约、数据政策与当前决策；测试运行产物改为本地生成并忽略。
- `.gitignore` 从「忽略 evidence 图片」收窄为「忽略整个 evidence 目录与 `od-*` 目录」。
- `engine/README.md` 与 `docs/ARCHITECTURE.md` 的历史报告链接改为归档说明；`tests/verification/EVIDENCE-ARCHIVED.md` 记录归档位置。
- 全树扫描已无个人路径与旧 Profile 名（许可证署名除外）；Rust 定向测试 6/6、91/91 通过；GitHub 历史尚未重写，未提交未推送。

## 公开仓库个人资料清理（2026-11-17）

- 将个人发言稿、个人写作风格库、个人数据根配置和初始化脚本移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/personal-files`，共 15 个个人资料文件，另将 `_design` 7 个设计参考文件移出，并生成 `MANIFEST.sha256`。
- 移除个人工作区、个人数据根配置、个人初始化脚本和旧 `knowledge/writing-style/`。
- `knowledge/writing-style/` 重建为 5 个通用模板，不含个人作品、案例或身份信息。
- 开发 Profile 目录改为中性名称 `profiles/omnidesk-dev/`，包名改为 `dsh-profile-omnidesk-dev`；开发启动脚本与隔离自检同步改名。
- `scripts/setup.sh` 去掉个人数据初始化步骤；`data-template/` 与 `docs/ARCHITECTURE.md` 改为通用外部数据根说明。
- 全树扫描已无个人路径与旧 Profile 名（许可证署名除外）；`bash -n` 通过；GitHub 历史尚未重写，未提交未推送。

## 测试截图归档（2026-11-17）

- 将 2026-11-17 之前入库的 1087 张测试截图与任务截图移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/test-evidence-screenshots`，并生成 `MANIFEST.sha256`。
- 归档体积约 152 MB；当时保留 603 个已跟踪的文字证据，同日 D-103 已将文字证据与历史报告一并移出；测试脚本与夹具保留。
- `.gitignore` 已加入 evidence 与 `od-*` 目录的图片、视频扩展名，后续测试截图不再入库。
- `AGENTS.md` 的证据保留规则同步改为「文字证据与测试脚本入库，截图和视频默认仓库外归档」。
- 未改产品代码，未运行构建；未提交未推送。

## 授权变更（2026-11-16）

- 根 `LICENSE` 改为 PolyForm Noncommercial 1.0.0，并加入项目版权与 Required Notice；新增 `COMMERCIAL-LICENSE.md`、`THIRD-PARTY-NOTICES.md`。
- `README.md` 的 License 小节、`desktop/package.json` 的 `license` 字段、`CHANGELOG.md` 与 `docs/decisions.md` 的 D-100 已同步。
- `runtime/` 未改；DeepSeek Harness 继续按上游 MIT（Copyright (c) 2026 DeepSeek）分发。
- 验证：许可证文本与 SPDX 官方文本逐字包含；package.json 可解析；`git diff --check` 通过；diff 无 `runtime/` 路径。未改代码，未运行构建。
- 未提交未推送。

> 更早的执行记录已归档到仓库外 `MyWorkStation-archive/2026-11-17/process-docs/`。

## OD-ARCH-174 Skill 优化（2026-11-17，三阶段完成，ARCHITECT 逐阶段复核）

**阶段 A/A2：** 四场景指针串号修复（defaultSceneSkills）；knowledge 专用绑定键（rules.json + HomeCanvas 独立读取 + HomeChat 透传），普通对话也注入 151 字符检索指针；D-108 决策记录。验证：tsc 0、verify_conversation_prompt 33/33、skill_bindings_082 69/69、worklog_089 86/86、cargo 1200 全过。

**阶段 B：** disable-model-invocation 实验发现会同时关闭 skill 工具执行（阻断指针链路），ARCHITECT 裁决收缩为只保留 document-review 标记；目录 2518 到 2401；能力零损失。

**阶段 C：** 三核心技能正文压至原 63-70%（memory-agent 1754 到 1228、memory-report 1099 到 749、period-review 1820 到 1268），契约逐字保留，082/089/503/505 全过。真实模型端到端：模型实际调用 skill(memory-report) 读到重写正文并按五段模板完整产出——指针、按需读取、按契约输出全链路确认。

**已知缺口：** sync-skills.sh macOS realpath 兼容未修；已安装 Profile 需恢复默认规则包获得 knowledge 绑定；其余 7 个按需技能未重写；设置页未展示 knowledge 绑定编辑。

## OD-ARCH-175 阶段 1 执行记录（子代理，2026-11-17）

> 执行主体：用户当轮批准派发的子代理（Plato，模型 deepseek-v4.1-flash-eco / high；ARCHITECT 复核修正执行主体记录——原始记录误写为 ARCHITECT 本人）。本阶段只读核查，未改代码、`runtime/`、模型路由或凭证，未执行真实模型调用。

### 范围与基线

- 开工时 `git status --short`：`M PLANS.md`、`M REVIEW.md`；HEAD `5754da202c chore(release): 0.2.31（含 OD-ARCH-174 Skill 优化）`。全部既有改动原样保留。
- **文件事实：** 仓库 `runtime/deepseek-harness/` 为空目录，`packages/llm/llm-pi-ai/src/discovery.ts` 不存在，无法取得源码行证据。实际核查对象是锁定的安装产物 `runtime/dsh-runtime/node_modules`：`@deepseek-ai/dsh-llm-pi-ai` 0.2.1-rc.5、`@earendil-works/pi-ai` 1.4.0（与 `runtime/RUNTIME.lock` 的 0.2.1-rc.5 一致）。`/Users/yvan/OmniDesk/runtime/dsh-home/profiles/omnidesk-base/node_modules` 只含 Profile 插件，不含 `dsh-llm-pi-ai`/`pi-ai` 核心包。

### a. 运行期校验、错误码与三种情形

- **文件事实：** `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm-pi-ai/lib/index.js:1785-1788` 的 `resolveReasoningLevel()` 在显式档位不在 `getSupportedThinkingLevels(model)` 中时抛：

```text
pi-ai provider "<provider>" model "<model>" does not support reasoning effort "<effort>"
```

  错误码准确为 `UNSUPPORTED_REASONING_EFFORT`。同一调用点在 `lib/index.js:1953`：`options.reasoningEffort ?? profile.reasoning`。报错前不进入 HTTP 模型请求。
- **文件事实：** `lib/index.js:585-608` 把 Profile 的 `reasoningEfforts` 物化成模型能力：未声明时取内置目录 `base.reasoning`，否则默认 `false`；声明 `false` 时固定 `reasoning: false`；声明 map 时 `reasoning: true`，未列出的基础档位在 `thinkingLevelMap` 中钉为 `null`。
- **文件事实：** `runtime/dsh-runtime/node_modules/@earendil-works/pi-ai/dist/models.js:693-703` 对 `reasoning: false` 返回唯一档位 `["off"]`；对声明 map 只返回未被 `null` 排出的档位。
- **静态检查（只读脚本）：** `builtinProviders()` 共 42 个，`hasVolcengine=false`、`getBuiltinModels("volcengine").length=0`；同脚本确认非推理模型支持 `["off"]`，声明 `low/high` 的 map 支持 `["low","high"]`。

| 情形 | 物化结果 | 行为差异与错误 |
|---|---|---|
| Profile 未声明 `reasoningEfforts`，且 provider/model 不在 pi-ai 内置目录（如 volcengine / deepseek-v4.1-flash） | `reasoning=false` | 任何非 `off` 的显式档位都会以 `UNSUPPORTED_REASONING_EFFORT` 拒绝；显式 `off` **不会**报该错误（`getSupportedThinkingLevels` 返回 `["off"]`，随后 `lib/index.js:1754-1758` 把 `off` 转成不下发 reasoning 参数）；不传档位也不报错。若 provider/model 恰好在 pi-ai 内置目录，则继承目录的 `reasoning`/`thinkingLevelMap`，不能一概判为不支持。 |
| Profile 显式声明 `reasoningEfforts: false` | `reasoning=false` | 与“自定义路由未声明且无内置目录”相同：非 `off` 拒绝，`off` 接受，不传档位接受。错误文本只含 provider/model/请求档位，**无法从运行期错误反分“未声明”与“声明为 false”**。 |
| Profile 声明非空档位集合，但请求档位不在集合内 | `reasoning=true` + `thinkingLevelMap` | 同一 `UNSUPPORTED_REASONING_EFFORT` 与同一格式的错误；集合内档位正常。`off` 仅当声明中存在 `off` 时可用（`off: null` 也视为可用）；声明未列 `off` 时，显式 `off` 也会被拒。 |

- **文件事实（相邻配置形态）：** `lib/index.js:589` 对 `reasoningEfforts: null` 或空对象 `{}` 在目录物化阶段直接报配置错误 `llm-pi-ai: provider "<provider>" model "<model>" has an empty reasoningEfforts; ...`，不是运行期 `UNSUPPORTED_REASONING_EFFORT`。只有 `off` 的集合还会在 `lib/index.js:597` 被判为“offers no level beyond off”。这不属于上述三种运行期情形，阶段 2 不应把它们误当作可回退失败。

### b. 到达宿主后的状态与 note 形态

- **文件事实：** DSH Web API 的 `session/prompt` 只做内容准入并把消息放入 Agent 队列，随后返回 `{ accepted: true }`，不等待整轮完成（`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-api-session-controller/lib/index.js:925-965`）。因此档位校验发生在后续 turn 中，失败走 `turn/end`，不走 `session/prompt` 的提交阶段错误。
- **静态检查（代码链）：** DSH `dsh-agent-loop/lib/index.js:1019-1032` 捕获 `LlmError`，把 `error.failure` 写入 `turn/end` 的 `reason: { kind: "error", error: { message, code } }`；`dsh-llm/lib/index.js:1615-1634` 证明 `failure` 同时保留 message 与 `UNSUPPORTED_REASONING_EFFORT` code。
- **文件事实：** OmniDesk `engine/omnidesk_engine/web_api_adapter.py:1823-1845` 从 `turn/end` 的 `reason.error.message` 提取文本；`:2075-2093` 将 `reason.kind=error` 落为 `TaskState.FAILED`，并生成 `engine reported turn/end=error：<DSH message>`。
- **文件事实：** `TaskState.FAILED` 的值是 `failed`（`engine/omnidesk_engine/contract.py:53`）；`task_core.py:675-689` 的 `_status_note()` 以 `stop_reason=<x>; error=<y>` 拼接 note。此案的运行期错误码只在 DSH 事件中，**不会**进入 OmniDesk 的 note（适配层只带 message）。
- **静态检查（预期形态，未实跑）：** 以 `volcengine / deepseek-v4.1-flash / high` 为例，任务状态轮询应得到：

```text
status.status = "failed"
status.note = stop_reason=error; error=engine reported turn/end=error：pi-ai provider "volcengine" model "deepseek-v4.1-flash" does not support reasoning effort "high"
```

  `HomeChat.tsx:2453-2463` 在 `failed`/`interrupted` 时直接把整条 `status.note` 当作 notice 显示，并置 `retryable=true`；它不单独解析 `error=` 段。该 string 为静态推演，未经真实模型调用或真实状态接口复核。

### c. 阶段 2 自动回退的可靠识别建议

- **静态检查：** 不要把 `UNSUPPORTED_REASONING_EFFORT` 当作 `status.note` 的匹配键；当前 note 只含 message。可靠组合是：`status.status === "failed"`、当前任务确实下发过显式档位、且 note 命中 `error=engine reported turn/end=error：pi-ai provider "..." model "..." does not support reasoning effort "..."`。建议解析 provider/model/effort 三个捕获组，并与本次路由和所选档位逐一相等后才回退；不要只按“failed 且含 reasoning effort”匹配，避免把其他上游拒绝误判成档位能力错误。
- **静态检查：** 同一正则也能识别“声明集合不含档位”，但阶段 3 的本地预检应已提前处理；运行期自动回退主要覆盖“未声明/无内置目录”的尝试失败。`off` 在非推理模型上会被接受，因此“显式档位必须非 off 才回退”不是必要条件；应以 DSH 是否实际返回该错误为准。
- 回退动作需每个任务最多一次（后续在阶段 2 的 Rust 状态机中实现），并继续使用现有 `retryable` 入口让用户手动重发，不自动重发失败任务。

### 未验证项

- **未验证（运行）：** 未真实发起 `volcengine / deepseek-v4.1-flash` 请求，未实际轮询 `omni.get_task_status` 抓取 `failed` 与完整 `note`。`b` 的原文是依据 rc.5 代码链的静态推演；仍需阶段 2 的隔离合成/真实验证确认 DSH 确实发出 `turn/end=error` 而非其他 follow 错误形态。
- **未验证（运行）：** 未验证 OmniDesk 适配层的 300 字符截断是否会影响极长 provider/model 名称；普通名称下上述消息完整保留。
- **未验证（源码）：** 仓库 `runtime/deepseek-harness` 未检出，无法引用上游 `packages/llm/llm-pi-ai/src/discovery.ts` 源码行；本结论仅基于锁定安装产物。

### 实际运行过的命令（只读）

```bash
git status --short && git log --oneline -1
find runtime/deepseek-harness -maxdepth 5 -type f -not -path '*/node_modules/*' | head -80
test -e runtime/deepseek-harness/packages/llm/llm-pi-ai/src/discovery.ts
rg -uu -n -C 12 'UNSUPPORTED_REASONING_EFFORT' runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm-pi-ai/lib/index.js
rg -n -C 8 'reasoningEfforts|reasoningEffort' runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-llm-pi-ai/lib/index.js
rg -uu -n -C 12 'LlmFailure|turn/end' engine/omnidesk_engine/web_api_adapter.py
rg -uu -n -C 10 'LlmFailure' runtime/dsh-runtime/node_modules/@deepseek-ai -g '*.js'
rg -n -C 18 'def _status_note' engine/omnidesk_engine/task_core.py
rg -n -C 8 'error=|stop_reason|status\.note' desktop/src/shell/HomeChat.tsx
node --input-type=module -e "import { builtinProviders, getBuiltinModels } from '@earendil-works/pi-ai/providers/all'; ..."  # 42 providers; volcengine absent / 0 models
node --input-type=module -e "const m=await import('file:///Users/yvan/Code/MyWorkStation/runtime/dsh-runtime/node_modules/@earendil-works/pi-ai/dist/models.js'); ..."  # nonReasoning=[off]; lowHigh=[low,high]
```

  另有一次 `@earendil-works/pi-ai/dist/models.js` 的包内路径 import 被 exports 拒绝（`ERR_PACKAGE_PATH_NOT_EXPORTED`），随后按上述绝对 `file://` 路径重跑成功；该失败不参与结论。所有命令均为本地只读检查，无网络模型调用。

- 未提交、未推送、未打标签；本阶段写集仅 `STATUS.md`。

## OD-ARCH-175 阶段 2 执行记录（子代理，2026-11-17）

> 执行主体：本助手（按用户当轮指令亲自执行；小节标题沿用本次交付要求）。未派任何再下层子代理。开工时 `git status --short` 保留全部既有改动（`M PLANS.md`、`M REVIEW.md`、`M STATUS.md`，以及阶段 3 的 `desktop/src/` 改动与相关验证脚本），HEAD `5754da202c`。本阶段未改 `desktop/src/`、`engine/`、`runtime/`，未动模型路由的用户选择与凭证。

### 改动文件

- `desktop/src-tauri/src/model_effort_fallback.rs`（新增）：稳定错误文本解析、覆盖文件生效路由读取、精确匹配、note 追加、单次触发 guard（Started / InFlight / Applied / Failed 四态）。
- `desktop/src-tauri/src/lib.rs`：接入 `engine_task_status` 状态轮询路径；`maybe_auto_fallback_effort` 完成识别、`select_route(..., None)` 重写覆盖文件、`sync_model_overlay`、后台线程 `stop_engine`。
- `desktop/src-tauri/src/model_routes.rs`：新增生产路径用例，验证 `select_route` 不传档位时覆盖文件不含 `reasoningEffort`，传档位时仍写入（正对照）。
- `STATUS.md`：本节。

### 行为与 note 契约

- 只在同时满足时回退：任务 `status.status == "failed"`；note 精确命中 `stop_reason=error; error=engine reported turn/end=error：pi-ai provider "<provider>" model "<model>" does not support reasoning effort "<effort>"`；覆盖文件（`agent-default-model` 或 `acp` 块）确实显式下发同一 provider/model/effort。错误码不进入 note，不按错误码匹配；`has an empty reasoningEfforts` / `offers no level beyond off` 等配置错误不触发。
- 回退动作：同步 `model_routes::select_route(..., None)` 重写覆盖文件 → `sync_model_overlay` → 后台线程取得 `lifecycle.begin_change()` 后 `stop_engine`。回退成功改写后才追加告知；停止旧引擎不占用状态轮询路径。
- **前端 note 完整格式（阶段 4 验证接口）：** `<原 note>；已自动回退为默认思考档位，请重发任务`（原 note 为空时只保留该句）。同一 (覆盖文件, provider, model, effort) 只触发一次（进程内存态，重启失效）；不自动重发失败任务，不改重试按钮。

### 实际命令与结果

```bash
git status --short && git log --oneline -1
# M PLANS.md, M REVIEW.md, M STATUS.md, 阶段 3 的 desktop/src 改动；HEAD 5754da202c

~/.cargo/bin/cargo check --lib
# Finished dev profile；仅既有 warnings（integrity.rs / export_engine.rs），新增模块无 warning

~/.cargo/bin/cargo test --lib model_effort_fallback
# 8 passed; 0 failed（解析命中/不命中、配置错误文本、路由精确匹配、覆盖文件路由读取、note 追加、单次 guard）

~/.cargo/bin/cargo test --lib select_route_none_effort
# 1 passed; 0 failed（真实调用仓库内 node + DSH CLI 做配置转储，只读不联网、不读凭证）

~/.cargo/bin/cargo test --lib
# 1209 passed; 0 failed; 1 ignored; finished in 13.32s（最终代码状态）
# 说明：最终代码定稿前有一次完整运行出现 3 个失败（命令只保留尾部输出，未记录用例名），
#       未涉及新增模块的定向用例；随后连续两次完整运行均为 1209 passed（不含并发参数改动）。
```

证据路径：`tests/verification/desktop/evidence/od-arch-175/stage2-cargo-test.md`（定向与完整回归摘要、环境与日志位置）。

### 证据等级

- **文件事实 / 静态检查：** 新增模块与接入代码已读回；note 解析锚定 `engine reported turn/end=error：pi-ai provider` 完整文本，配置空档位错误文本有反例单测；guard 为进程内存态且带 InFlight/Applied/Failed 状态，避免失败后补发“已回退”告知。
- **脚本/运行测试：** 上述 `cargo test --lib` 实际运行结果；`select_route` 用例实际启动 node + 仓库内 DSH CLI，断言回退重写后覆盖文件无 `reasoningEffort`。
- **未验证（运行，归阶段 4）：** 未发起真实模型请求，未实际观察 DSH 拒绝 → 宿主状态 note → 自动回退 → 引擎重启的完整链路；未验证真实引擎关闭耗时与前端 notice 展示。`engine_task_status` 的完整回退路径目前由单元测试覆盖各判定/改写环节，未用假引擎做端到端状态注入。

### 交付时 Git 状态（未提交、未推送、未打标签）

```text
M  PLANS.md
M  REVIEW.md
M  STATUS.md
M  desktop/src-tauri/src/lib.rs
M  desktop/src-tauri/src/model_routes.rs
?? desktop/src-tauri/src/model_effort_fallback.rs
（阶段 3 的 desktop/src 改动与验证脚本原样保留，未触碰）
```

### 待 ARCHITECT 复核

- 重点核对：状态轮询只读路径不因后台关闭卡住；一次性 guard 的失败态不会误发告知；note 契约与阶段 3/4 前端消费方式一致。

## OD-ARCH-176 阶段 1 执行记录（子代理，2026-11-17）

### 开工前事实

```text
cd /Users/yvan/Code/MyWorkStation && git status --short && git log --oneline -1
# 工作树已有 OD-ARCH-175 未提交改动（desktop/src、desktop/src-tauri、verify 脚本与两个新文件）；
# HEAD 5754da202c。全部原样保留，本轮改动叠加其上，未 reset / checkout / 提交 / 推送。
```

### 改动文件清单

```text
desktop/src/workIntentDecision.ts                    新增：统一意图契约解析、分流、思考档位选择（纯函数）
desktop/src/workIntent.ts                            新增：periodReviewTargetForPeriod（模型判出的周期 → 起止日期）
desktop/src/conversationPrompt.ts                    classify 模式：统一意图契约 + 本周索引 + note 技能指针
desktop/src/recordReview.ts                          抽出 parseRecordReviewValue，供统一契约复用
desktop/src/api.ts                                   submitTask 透传 reasoningEffort / provider / model
desktop/src/shell/HomeChat.tsx                       工作记录场景分类前置、note 落盘与复核同调用、指令不落盘、不确定提问、复盘/周报/摘要分流、档位管线
desktop/src/shell/QuestionCard.tsx                   本地提问可复用标题/说明/取消文案（默认行为不变）
desktop/src-tauri/src/lib.rs                         engine_submit_task 接收并按需下发档位与路由
engine/omnidesk_engine/task_core.py                  SubmitTaskRequest 增加 reasoning_effort / provider / model
engine/omnidesk_engine/serve.py                      透传上述字段
engine/omnidesk_engine/web_api_adapter.py            在 session/prompt 前按会话应用显式档位（select_model），同会话同组合去重
tests/verification/desktop/verify_arch_176.py        新增：mock 分类输出分流、档位策略、管线与提示词断言
tests/verification/desktop/verify_worklog_089.py     调整 note schema 断言（classify 分支为本计划新增契约）
tests/verification/desktop/verify_worklog_103.py     快路径移除后的日期工具与技能契约断言更新
tests/verification/desktop/verify_worklog_104.py     快路径移除、分类落盘、提问与剥离顺序断言更新
tests/verification/engine-interface/adapter/test_web_api_unit.py  新增档位应用顺序/去重/失败不发送 prompt 用例
```

未修改：`runtime/`、`skills/`、`profiles/`、凭证、模型路由覆盖文件；未新增设置项。

### 实际命令与结果

```text
cd desktop && npx tsc --noEmit
# 0（无输出）

cd desktop && npm run build
# exit 0；vite 构建成功（仅既有 chunk >500kB 提示）

cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib -- --test-threads=1
# 1209 passed; 0 failed; 1 ignored; finished in 48.00s（含 OD-ARCH-175 model_effort_fallback 既有单测）

cd desktop/src-tauri && ~/.cargo/bin/cargo test --lib
# 并发运行一次 1206 passed / 3 failed（event_pump、unstamped_grant、spawn_with_stderr 均为
# "启动假引擎失败: Permission denied"）；三条各自单独运行均 ok，属既有环境性 flaky，未涉本轮新增逻辑。

cd tests/verification/engine-interface/adapter && python3 -m unittest -v test_web_api_unit
# Ran 101 tests ... OK（含 4 条 OD-ARCH-176 档位用例）

python3 tests/verification/desktop/verify_arch_176.py
# cases=55 passed=55 failed=0

python3 tests/verification/desktop/verify_worklog_089.py
# cases=86 passed=86 failed=0
python3 tests/verification/desktop/verify_worklog_103.py
# cases=52 passed=52 failed=0
python3 tests/verification/desktop/verify_worklog_104.py
# cases=73 passed=73 failed=0
python3 tests/verification/desktop/verify_conversation_prompt.py
# cases=33 failed=0
python3 tests/verification/desktop/verify_skill_bindings_082.py
# cases=69 failed=0
python3 tests/verification/desktop/verify_wr_503.py / verify_wr_505.py
# exit 0；505 为 71/71

python3 tests/verification/desktop/verify_home_refine_202.py
# exit 0；39/40，唯一未过的 home_scene_backdrop 为既有断言（HEAD 的 App.tsx 也不含 app-scape-art，
# 该样式只在 theme.css），与 OD-ARCH-176 无关；本轮顺手更新了头像 executed 标记的断言。
```

### 证据等级

- **文件事实 / 静态检查：** 已读回 `HomeChat.tsx`、`conversationPrompt.ts`、`api.ts`、Rust 提交命令、`serve.py`、`task_core.py`、`web_api_adapter.py`；`verify_arch_176.py` 对快路径移除、分类入口、档位链、提问卡与复盘内联均做源码断言。
- **脚本运行：** 上述 tsc / 构建 / cargo / Python 单测 / verify 脚本均为实际执行结果；`verify_arch_176.py` 编译真实 TS 后喂 mock 分类 JSON，覆盖 note / instruction / chat / period-review / report / daily-summary / uncertain 与非法 JSON。
- **真实模型输出：** 未执行（阶段 3 由 ARCHITECT 组织）。

证据路径：`tests/verification/desktop/evidence/od-arch-176/verify-arch-176.json`（mock 分类输出 55 项断言结果）；脚本本体 `tests/verification/desktop/verify_arch_176.py`。

### 未验证项与已知风险

1. **DSH 按次 reasoningEffort 不存在（已核实契约）：** `SessionPromptRequest` 仅含 requestId / sessionId / mode / content / clientTimeZone，无按次档位字段；本轮按计划走 `session/selectModel` 会话级通道。
2. **selectModel 的默认持久化副作用：** DSH `selectModel` 会在后台 `agentDefaultModel.saveSelection` 写入其自身默认选择；本轮未改桌面覆盖文件、未重启引擎，但无法阻止 DSH 记录“最近一次选择”。已在代码注释中保留事实，待阶段 3 真实模型验证时观察对用户默认档位的影响。
3. **已声明档位模型的 `off`：** OD-ARCH-175 阶段 1 已证明未声明模型显式 off 可用；对明确声明档位集合但未含 off 的模型，本轮按计划仍传 off，若 DSH 拒绝则该分类轮失败并如实显示。未真实调用验证。
4. **真实分类质量：** 未用真实模型验证模型是否稳定遵守统一契约（尤其 note 的 record 必须是用户原话连续片段、review 同调用返回）。阶段 2 技能契约微调后再做真实回归。

### 交付时 Git 状态（未提交、未推送、未打标签）

```text
M PLANS.md / REVIEW.md / STATUS.md
M desktop/src-tauri/src/lib.rs / model_routes.rs
M desktop/src/api.ts / conversationPrompt.ts / recordReview.ts / types.ts / workIntent.ts
M desktop/src/shell/HomeChat.tsx / ModelPicker.tsx / QuestionCard.tsx
M engine/omnidesk_engine/serve.py / task_core.py / web_api_adapter.py
M tests/verification/desktop/verify_home_refine_202.py / verify_worklog_089.py / verify_worklog_103.py / verify_worklog_104.py
M tests/verification/engine-interface/adapter/test_web_api_unit.py
?? desktop/src-tauri/src/model_effort_fallback.rs（OD-ARCH-175）
?? desktop/src/workIntentDecision.ts
?? tests/verification/desktop/verify_model_route_precheck_175.py（OD-ARCH-175）
?? tests/verification/desktop/verify_arch_176.py
```

### 待 ARCHITECT 复核

- 重点核对：分类提示词与 memory-agent 现有输出的关系（本轮用提示词显式覆盖旧 `<omnidesk-verdict>` 格式）；note 校验失败时是否只提问、不按原文猜测落盘；第二段起草轮沿用同一引擎会话是否符合预期。

## OD-ARCH-176 阶段 2 执行记录（子代理，2026-11-17）

### 改动

- `desktop/src/workIntentDecision.ts`：`pickWorkIntentEffort` 增加两层档位证据输入。只有目录或 Profile 的非空声明集合明确不含 `off` 时，`off` 降为 `null`（跟随模型默认）；未声明、目录不可用、空集合与 `reasoningEfforts: false` 均维持传 `off`。用户手动选择 `off` 也走同一保护。
- `desktop/src/shell/HomeChat.tsx`：复用 OD-ARCH-175 的证据顺序（先目录 `efforts`，无声明再看 Profile 原始 `reasoningEfforts`），从已加载的非机密模型元数据生成证据；分析、分类、路由起草四类调用统一经 `pickRoutedWorkIntentEffort` 选档。未新增每轮 RPC、未改路由或覆盖文件。
- `skills/period-review/SKILL.md`：补充「收到随请求送达的汇集材料时直接起草，不回复等待话术；缺失或为空时如实说明，不自行找文件」，引用编号、案件材料禁入、已确认不可覆盖等硬约束保持不变。
- `tests/verification/desktop/verify_arch_176.py`：新增目录含/不含 `off`、未声明、目录不可用、Profile 含/不含 `off`、`reasoningEfforts: false`、手动 `off` 等纯函数用例，以及 HomeChat 统一预判调用点与技能文本断言。
- `STATUS.md`：仅追加本阶段记录。

### 实际命令与结果

```text
cd desktop && npx tsc --noEmit
# exit 0

python3 tests/verification/desktop/verify_arch_176.py \
  --json tests/verification/desktop/evidence/od-arch-176/verify-arch-176.json
# exit 0；cases=70 passed=70 failed=0

~/.cargo/bin/cargo test --lib        # desktop/src-tauri
# 首次：1206 通过 / 3 个合成子进程启动 PermissionDenied / 1 ignored
# 原命令重跑：exit 0；1209 passed / 0 failed / 1 ignored

scripts/sync-skills.sh --check
# 原生调用未进入比对：当前 macOS 的 /bin/realpath 不支持脚本使用的 `-m`
# 临时兼容 wrapper 仅补 `realpath -m` 后重跑：exit 1，仅 [漂移] period-review
```

### 证据等级

- **文件事实 / 静态检查：** 已读回三处实现与脚本改动；`verify_arch_176.py` 对四类 off 调用统一入口、两层证据函数与技能硬约束做源码断言。
- **脚本运行：** `tsc --noEmit`、69 项 verify、`cargo test --lib` 均为实际执行。verify 编译真实 TS 模块后运行纯函数与 mock 契约输入，不调用模型。
- **环境限制：** cargo 首跑的 3 个失败均为合成子进程 `PermissionDenied`，原命令重跑全过，未复现；`sync-skills.sh --check` 受 BSD `realpath` 缺陷影响，兼容 wrapper 下确认唯一漂移为本次修改的 `period-review`。未执行实际同步，安装副本仍是旧文本。

证据路径：`tests/verification/desktop/evidence/od-arch-176/verify-arch-176.json`（70 项断言）；脚本本体：`tests/verification/desktop/verify_arch_176.py`。

### 未验证项

1. 未用真实声明不含 `off` 的模型发起工作记录分类/分析请求；本轮只验证证据判定与提交参数。
2. 未重编译桌面应用，也未实际同步隔离 Skill 副本；用户当前安装副本需在获准后执行 `scripts/sync-skills.sh`，或在下次构建/初始化时采用内嵌新文本。
3. 未修改 `runtime/`、`engine/`、`desktop/src-tauri/`、`profiles/` 或凭证；未提交、未推送、未打标签。

### 交付时 Git 状态

```text
M PLANS.md / REVIEW.md / STATUS.md
M desktop/src-tauri/src/lib.rs / model_routes.rs
M desktop/src/api.ts / conversationPrompt.ts / recordReview.ts / types.ts / workIntent.ts
M desktop/src/shell/HomeChat.tsx / ModelPicker.tsx / QuestionCard.tsx
M engine/omnidesk_engine/serve.py / task_core.py / web_api_adapter.py
M skills/period-review/SKILL.md
M tests/verification/desktop/verify_home_refine_202.py / verify_worklog_089.py / verify_worklog_103.py / verify_worklog_104.py
M tests/verification/engine-interface/adapter/test_web_api_unit.py
?? desktop/src-tauri/src/model_effort_fallback.rs（OD-ARCH-175）
?? desktop/src/workIntentDecision.ts
?? tests/verification/desktop/verify_model_route_precheck_175.py（OD-ARCH-175）
?? tests/verification/desktop/verify_arch_176.py
```

### 待 ARCHITECT 复核

- 确认使用已加载目录/Profile 元数据而不在每轮调用 `collectModelEffortEvidence` 是否满足“复用两层证据”边界；若要求直接调用该函数，则需额外获批导出 `api.ts` 的私有收集器。
- 确认安装副本漂移的处理时机；本轮未写入运行时副本。

## OD-ARCH-177 执行记录（ARCHITECT 亲自执行，2026-11-17）

> 用户批准 OD-ARCH-177 全部执行并授权提交推送发版；按常驻指令本轮未派子代理，本助手亲自实施。

### 改动

- `desktop/src/companionPlugins.ts`（新增）：常备插件只读清单——AnySearch CLI（github.com/anysearch-ai/anysearch-skill，建议 v2.1.0）、dsh-cost-meter（github.com/Han-1413141/dsh-cost-meter，建议 1.9.6）；仓库不打包插件实体、不自动安装、不联网检查版本。
- `desktop/src/SettingsPanel.tsx`：Profile 资产页新增「配套插件推荐」只读区块（testid: companion-plugins / companion-plugin-*），列表自传达、不加长说明。
- `skills/anysearch/SKILL.md`：第五节新增引擎环境预期——无本机 Codex 技能目录与 ANYSEARCH_SKILL_DIR 属预期，检测不到 CLI 直接回退 web_fetch 并如实标注来源，不按报错指引反复重试；既有红线约束未动。
- `tests/verification/desktop/verify_companion_plugins_177.py`（新增）：清单/渲染/技能文本 11 项断言。

### 验证（实际命令与结果）

- `python3 tests/verification/desktop/verify_companion_plugins_177.py`：11/11（初版 ROOT 层级与一处拼接字符串断言有误，已修正后全过——文件事实）。
- `npx tsc --noEmit`、`npm run build`：通过（脚本运行）。
- `verify_arch_176.py` 70/70、`verify_model_route_precheck_175.py` 12/12+11/11：回归通过（脚本运行）。
- `cargo test --lib -- --test-threads=1`：1209/0（并发运行偶现 3 个引擎 spawn 类 PermissionDenied 环境性失败，单线程与复跑均不复现，非本改动回归）。

### 发布

- 版本 0.2.31 → 0.2.32（package.json + tauri.conf.json），CHANGELOG 0.2.32 条目已写；提交推送后由 release.yml（push to main）自动构建发布。
- 修正：release.yml 的发布版本源是仓库根 `VERSION` 文件（非 package.json），初推时 VERSION 仍为 0.2.31 会 publish=false 空跑；已补 commit 6e3dab1520（VERSION → 0.2.32）并推送，本次 CI 应以 v0.2.32 构建发布。发布提交链：c718aa6e65（feat）→ a1cb0da7ab（docs）→ 99cb0c70f9（release）→ 1e3ec584dc（test 补录）→ 6e3dab1520（VERSION 修正）。

## 0.2.33 线上回归修复执行记录（ARCHITECT 亲自执行，2026-11-17）

> 用户实测 0.2.32：工作记录场景每轮 engine_unavailable，其他场景正常。ARCHITECT 复现并定根因（session/selectModel 拒绝未声明模型的 off，错误兜底误映射），属 OD-ARCH-176 阶段 1 实现判断错误；详见 REVIEW.md 事故记录。本轮修复由 ARCHITECT 亲自完成。

### 改动

- `desktop/src/workIntentDecision.ts`：新增 `effortEvidenceIncludes`；`pickWorkIntentEffort` 改为「证据证明支持才下发」——未声明 / 目录不可用 / 声明不含该档位一律不传；用户手动档位同样需要证据。
- `engine/omnidesk_engine/web_api_adapter.py`：`_apply_reasoning_effort` 捕获 EngineError、记 stderr、按模型默认继续（新增 `import sys`）；`_map_rpc_failure` 将 `session/model-unavailable` 改判 INVALID_ARGUMENT。
- `tests/verification/desktop/verify_arch_176.py`：档位用例按新语义更新（71 项）。
- `tests/verification/engine-interface/verify_effort_select_degrade.py`（新增）：真实 DSH 回归——selectModel 拒绝未声明模型 off → 适配器降级 → 任务成功，4 项断言。

### 验证（实际命令与结果）

- 真实 DSH 回归：`python3 tests/verification/engine-interface/verify_effort_select_degrade.py` → 4/4（此前同参数复现出 `[engine_unavailable] provider "fake" model "synthetic-model" does not support reasoning effort "off"`，修复后任务成功完成）。
- `verify_arch_176.py` 71/71、`verify_companion_plugins_177.py` 11/11、`verify_model_route_precheck_175.py` 12/12+11/11、`cargo test --lib --test-threads=1` 1209/0、`tsc` / `npm run build` 通过。

### 发布

- 0.2.32 → 0.2.33（VERSION / package.json / tauri.conf.json），CHANGELOG 0.2.33 条目已写；推送后 CI 以 v0.2.33 构建发布（cancel-in-progress 会取消未完成的 0.2.32 构建，只发布 0.2.33）。
