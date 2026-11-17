# 执行状态

## OD-ARCH-173 阶段 6：验证、发布与回退（2026-11-17，待 ARCHITECT 复核）

- 全量回归：`cd desktop && npx tsc --noEmit` exit 0；`npm run build` exit 0（361 modules，仅既有 chunk 体积提示）；`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib -- --test-threads=1` 1200 passed / 0 failed / 1 ignored（49.41s）；`cargo check --offline --all-targets` exit 0；`PATH="$HOME/.cargo/bin:$PATH" npm run tauri build -- --no-bundle` exit 0，release 二进制 1m 33s；`bash scripts/sync-app-version.sh` 确认 tauri.conf 已是 0.2.30；`git diff --check` exit 0。
- UI/Profile 脚本：运行 54 个非 Xvfb 脚本，41 通过、13 失败；失败项全部在 `git archive HEAD` 快照复现或属 macOS 环境限制（`runtime/deepseek-harness` 子模块未初始化、`TMPDIR` 不在 /tmp、已移除的 WeeklyQuadrants.tsx），无本版本引入的失败。逐项分类见 `tests/verification/desktop/evidence/od-arch-173/stage6.md`。
- 遗留断言更新（3 个脚本，均已通过语法检查）：`verify_skill_bindings_082.py` 把「六要素全文在提示词」改为「指针 + 读取指令在、正文不在」，证据 snippets 改为指针；58/58 通过。`verify_worklog_089.py` 把全文包含拆成指针/读取指令/正文不注入三条，剥离对象改为指针，并把 HEAD 既有失效的索引文案断言对齐为「已有记录（最近 14 天」；85/85 通过。`verify_skill_flow_injection_108.py` 探针改为「主技能：memory-agent/memory-report」，新增正文不得命中的两条断言；本机 macOS 无 Xvfb，未运行，仅 py_compile。
- 版本与日志：`VERSION` 0.2.30；`desktop/src-tauri/tauri.conf.json` 0.2.30；`desktop/package.json` 0.2.30；`CHANGELOG.md` 的 `[Unreleased]` 收口为 `[0.2.30] - 2026-11-17`，写清效率（system 5396→2983、四动作 user 段降 61%–72%）、安全（free-search 依赖清理、输出上限、保存/切换预检）、成熟（配置解释页）与 Known limitations。
- 播种迁移：`seed_home_from_package` 只在缺失时复制，不覆盖已有 Profile；日常装配把内容不同的文件标 drift 并保留；只有用户点「修复缺失 / 恢复出厂内容」才备份后写回。本机 `~/OmniDesk/runtime/dsh-home/profiles/discipline` 仍含 `dsh-free-search`（只读核对，未改用户数据）。发布工作流会在 tauri build 前用 `prepare-runtime-assets.sh` 重建 seed，新装包干净；现有用户需点恢复出厂或重装，记为已知事项。
- 未验证：真实模型 smoke（402 余额不足）；Windows 安装器；`verify_skill_flow_injection_108.py` 真实窗口运行；配置解释页真实窗口截图；GUI/真实模型下的 Skill 读取行为。
- 交付：本次提交与推送结果见下方「0.2.30 发布提交」小节；未打标签，未 force push。

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
