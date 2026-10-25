# OD-ARCH-082 BUILDER 任务指令

> 依 `docs/BUILDER-Task-Instruction-Template.md` 撰写。计划正文以 `PLANS.md` 的 OD-ARCH-082 小节为唯一实施口径；本文只负责把范围、顺序、证据与禁止项说清楚，不放宽任何一条。

## 任务编号与名称

OD-ARCH-082：三大场景 Skill 深化 + Profile 资产绑定（S1–S8）

## 获批依据

- 计划位置：`PLANS.md` → 「OD-ARCH-082：三大场景 Skill 深化 + Profile 资产绑定」。
- 批准时间与批准人：2026-10-25，用户明确「批准执行 OD-ARCH-082」。
- 开工基线（ARCHITECT 实测）：HEAD `0ac9edf`，与 `origin/main` 同步；`git status --porcelain` 只有 `PLANS.md`、`STATUS.md`、`REVIEW.md` 三份文档改动；`git status --porcelain runtime/` 为空。

允许修改的文件或目录：

| 范围 | 具体路径 | 允许的改动 |
| --- | --- | --- |
| 前端 | `desktop/src/**` | S2 类型、S4 提示词注入、S6 徽标/弹层、S7 TurnRail、S8 搜索与 App 通道 |
| 宿主 | `desktop/src-tauri/src/**` | S2 规则 schema、S6 Profile 白名单与回落提示 |
| Skill | `skills/**` | S3 正文深化与合成用例 |
| 模板 | `samples/profiles/**` | S2 绑定段、S6 两个 Profile 模板 |
| 脚本 | `scripts/omnidesk.sh`、`scripts/sync-skills.sh`（仅当必须） | S6 环境变量前置修复 |
| 验证 | `tests/verification/desktop/**` | 新验证脚本、扩 068 脚本、`evidence/od-arch-082/` |
| 状态 | `PLANS.md` / `STATUS.md` / `REVIEW.md` 的 082 记录 | 追加执行记录；不得改写 080/081 结论 |

数据根写入的唯一许可：在 `~/OmniDesk/runtime/dsh-home/profiles/` 下**新建** `omnidesk-base` 与 `discipline` 两个目录（create-new-only）。不得覆盖、改名、迁移或删除既有 `acp` / `headless` / `omnidesk-test` / `web` / `yvan-work`；不得触碰数据根中的会话、成果、规则文件或 `skills/` 之外的任何目录。

明确不做：

- 不修 `REVIEW.md` 顶部预检 A/B/C/D（另立计划）。
- 不修改 `runtime/` 内 DSH Runtime，不改 `runtime/deepseek-harness/` 任何文件。
- 不碰凭证、不读 `settings.v1.json` 之外的密钥类文件、不改模型路由与 provider 配置。
- 不做跨会话全局检索、不做会话列表搜索、不做虚拟化优化。
- 不重写 TurnRail 或 TopNav 其余部分，不新造视觉 token。
- 不改 `RulePolicy` 的加载语义（红线与隐私掩码仍只由「规则包是否安装」决定）。
- 不提交、不推送、不打标签。

## 开工前检查（只读）

1. 记录 `git rev-parse --short HEAD`、`git status --porcelain`、`git status --porcelain runtime/`。
2. 保留全部既有未提交、未跟踪、暂存改动；三份状态文档的当前改动不得回滚或覆盖。
3. 只读确认以下既有事实，不要重复考古：

- `profile_choice.rs`：`standard_profile()` 现在 acp→acp、其余→web；`list()` 扫描带 `package.json` 的目录并补标准项；`resolve()` 对非法或缺失的保存值报错而不回退。
- `engine_supervisor.rs`：`OMNIDESK_DSH_PROFILE` 环境变量优先，空值才读 `settings.v1.json` 的 `engine_profile`。
- `desktop/src/shell/TurnRail.tsx` 153–168 行：tooltip 已有 `rounded-xl border border-surface-variant/40 bg-surface-container-lowest px-3 py-2.5 shadow-lg w-[300px] max-w-[70vw]`，问句 `line-clamp-1`、回复 `line-clamp-3`，`onFocus` 已触发预览。
- `desktop/src/shell/TopNav.tsx` 436–453 行：搜索框 `disabled`，placeholder「全局检索（未实现）」；`TopNav` 只接 `engineHealthy / engineLabel / onOpenSettings`，与 `HomeChat` 是 `App` 下的兄弟组件。
- `desktop/src/conversationPrompt.ts`（`composeConversationPrompt`，`CONTEXT_MAX_CHARS = 2500`）与 `desktop/src/aiDraft.ts`（`buildDraftPrompt`）是当前唯一提示词组装点；`HomeChat` 在提交前调用它们。
- `profile_rules.rs`：schema `omnidesk.profile-rules.v1`，文件 `discipline.rules.json`，全部结构 `deny_unknown_fields`。
- `samples/profiles/discipline.yml` 是 `omnidesk.profile-decl.v1` 的 Dock 场景声明，**不是** DSH Profile。

## 实施要求

按 S1→S8 顺序做，每步完成再进入下一步；不要把多步混成一个不可拆的改动。实施中发现计划本身的技术问题，写进 `STATUS.md` 并说明影响，不要自行改口径或扩范围。

**S1 只读盘点。** 逐个读 6 个 `skills/*/SKILL.md`，在 `STATUS.md` 写简表：skill id、触发条件、输入、输出、当前绑定方式、质量缺口。只写文件事实。

**S2 绑定契约 v1→v2。** `discipline.rules.json` 增 `skill_bindings`：场景 → `{ "skills": ["..."], "max_injected": 3 }`（`max_injected` 可选，缺省 3）。同步改 `profile_rules.rs`（保持 `deny_unknown_fields`）、`desktop/src/types.ts`、`SettingsPanel.tsx` 规则资产分页展示、`verify_profile_rules_068.py`。校验：id 属于六项清单、同场景不得重复、数量不超过上限；v1 文件必须原样可加载（视为无绑定），**不得自动改写用户已装规则文件**。v2 只在用户显式「安装/恢复默认」时写出。

**S3 Skill 深化 + 合成用例。** 重写三场景主用 Skill 的 `SKILL.md`：触发词、输入要求、输出结构、质量自检、数据边界（引用 `profile-rules` 而不复制红线词表）。顺带修正 `memory-agent`、`project-agent` 里残留的「Yvan AI Workbench」与 `~/YvanAI/...`。每个 Skill 附合成评测用例（输入样例 + 期望行为要点），固化成不依赖模型的回归。

**S4 注入链路。** 在 `conversationPrompt.ts` / `aiDraft.ts` / `HomeChat` 组装处按「当前 Profile + 当前场景绑定」注入 Skill 正文，受 `max_injected` 与 `CONTEXT_MAX_CHARS` 约束；随手记分支最多注入一个 Skill。续答（continuation）每轮重新注入，需有用例钉住。无规则包或无绑定时提示词与现状逐字一致（用 068 既有断言保护）。不得把注入逻辑放进 `runtime/`。

**S5 三场景证据。** 三个场景各一条合成输入走真实窗口，把「命中的 skill id + 注入片段」落 `evidence/od-arch-082/scene-injection.json`。不依赖真实模型输出。

**S6 两态 Profile。** 

1. 新建 `omnidesk-base`（默认）与 `discipline`，bundles 均为 `@deepseek-ai/dsh-base` + `@deepseek-ai/dsh-web-app`。`omnidesk-base` 的 `cordis.patch.yml` 覆盖 `preset-standard` 的 `config.plugins`，**保留上游条目与顺序**，只把 `skill-filesystem`、`tool-skill` 标 `disabled: true`。
2. 新增校验脚本比对上游 `web-app/presets/standard.patch.yml` 的条目 id 序列，漂移即失败。
3. `profile_choice.rs` 加白名单：web-api 通道只列 `omnidesk-base`、`discipline`（默认 `omnidesk-base`），acp 通道仍只 `acp`；保存值不在白名单时**保留原值**、本次回落默认，并通过 `profile_choice_view` 返回可显示的原因（如 `saved_out_of_whitelist: true`），由徽标/弹层如实提示「当前保存的 Profile 不在可选清单，本次使用默认」。
4. 修 `scripts/omnidesk.sh`：删除无条件注入的 `OMNIDESK_DSH_PROFILE` 默认值（acp/web 两个分支都改），外部显式设置仍优先。
5. 徽标样式并入现有 plate-soft 体系。
6. 两态各启动一次引擎（合成端点即可），`--dump-config` 对比插件与 skill 可见性，落 `evidence/od-arch-082/profile-base.json`、`profile-discipline.json` 与两张截图。

**S7 TurnRail。** 只加主题色边（如 `border-primary/30`）与 `aria-describedby`（tooltip id 与实际描述元素对应）。不动布局、不改截断、不重写组件。截图一张，含键盘聚焦态。

**S8 当前会话搜索。** `App` 持查询态与命中游标，经 props 传给 `TopNav` 与 `HomeChat`；`TopNav` 输入框解除 `disabled`，placeholder 改「当前会话内查找」，保留 ⌘K 聚焦。`HomeChat` 按消息轮次/段落文本匹配，命中高亮、上/下跳转（滚动到对应轮次）、计数 `n/m`，无命中 `0/0`，Esc 清空并退出。不解析 HTML、不跨会话、不做虚拟化。命中态与无结果态各一张截图。

## UI 参考（涉及界面时必填）

- 检索关键词：`chat workspace`、`in-conversation search`、`profile switcher`（`screenshots-design-inspiration`，2026-10-25 ARCHITECT 预检索，返回 ChatGPT、Linear、Notion 等 20 个产品）。
- 参考产品与链接：检索结果中的 Linear（顶栏内联查询）、Notion（分组侧栏）。
- 借鉴要点（≤2 句）：搜索沿用 Linear 的顶栏内联查询与命中高亮，把计数与上/下跳转放在同一行；导航参考 Notion 的分组侧栏，Profile 徽标保持单一短标签。只借鉴信息结构，不整页照搬、不把截图入库。

## 验证

定向验证：

```bash
cd /home/kevin/Code/MyWorkStation
python3 tests/verification/desktop/verify_profile_rules_068.py          # 期望 exit 0，v1 兼容
python3 tests/verification/desktop/verify_skill_bindings_082.py        # 新增，绑定校验与注入差异
python3 tests/verification/desktop/verify_profile_presets_082.py       # 新增，preset 条目序列校验
cd desktop && ./node_modules/.bin/tsc --noEmit -p tsconfig.json        # 期望 exit 0
```

完整回归（S8 之后统一跑，并把实际命令与结果写进 STATUS.md）：

```bash
cd /home/kevin/Code/MyWorkStation
desktop/node_modules/.bin/tsc --noEmit -p desktop/tsconfig.json
cd desktop && npm run build && cd ..
cd desktop/src-tauri && cargo test --offline -q --lib && cd ../..
python3 -m pytest tests -q
scripts/sync-skills.sh --check
git status --porcelain runtime/
```

基线（2026-10-25，ARCHITECT 实测，可作对照）：tsc exit 0；`npm run build` 成功；cargo lib 260 passed / 0 failed；pytest 75 passed；`runtime/` 0 行。

证据落盘：`tests/verification/desktop/evidence/od-arch-082/`，至少含 `scene-injection.json`、`profile-base.json`、`profile-discipline.json`、S6/S7/S8 截图，以及每条结论的证据等级标注（文件事实 / 静态检查 / 脚本运行 / 真实模型输出 / 未验证）。

## 硬约束

- 不修改 `runtime/` 内 DSH Runtime；不动模型路由与凭证；不读、不打印、不复制任何密钥。
- 不接触真实业务材料、案件线索或未获准数据；所有测试输入必须是合成的。
- 数据根只允许上面列明的两个新 Profile 目录，create-new-only；任何覆盖、迁移、删除都须停下报告。
- 不提交、不推送、不打标签；完成后停下等待 ARCHITECT 独立复核。

## 交付

回报必须包含：改动文件清单、实际运行的命令与结果、证据路径、未验证项、计划偏差（若有）、`git status --porcelain` 与 `HEAD` 快照。`STATUS.md` 写完整执行记录并明确标「待 ARCHITECT 复核」。
