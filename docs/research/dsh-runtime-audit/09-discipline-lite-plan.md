# Discipline 精简 Profile / Preset 优化计划

- 状态：**已批准并实施（OD-PROFILE-092）；待用户实测默认 preset 与真实业务体验**
- 体积/内存实测：0.2.23 构建包 `du -sh` 631 MB；旧 0.2.22 安装包当前不可用，未作整包 A/B。本次没有删除插件包，Profile 只增加约 2.6 KB 配置。同版本 DSH 空载三轮中位数：旧完整 preset RSS `377,936 KB` / footprint `334 MB`，`discipline-core` 且 built-in presets 全部 disabled 后为 `361,488 KB` / `316 MB`，约减 **16.1 MiB RSS / 18 MB footprint**。
- 基线时间：2026-11-12
- 目标版本：当前仓库产品源码 `0.2.23`，锁定 DSH `0.2.1-rc.5`
- 当前已安装 OmniDesk：`0.2.22`；本计划不以已安装旧包为准
- 当前指令：不升级 DSH、不比较 Pi、不引入子 Agent、不改产品源码前先完成配置层减法

## 1. 目标

把 `discipline` Profile 从“复制完整的 standard preset”改为“一个产品 Profile + 一个精简核心 Preset + 按动作加载 Skill”。目标是减少模型可见工具面、降低 Prompt/Schema 复杂度、消除无使用场景的子 Agent 与通用 workflow，同时保留四组业务能力：

1. 工作记录与复盘
2. 文案写作
3. 知识库
4. 项目跟踪

## 2. 目标结构

```text
Profile: discipline
  bundles: @deepseek-ai/dsh-base, @deepseek-ai/dsh-web-app
  patch: discipline / discipline
  core preset: discipline-core
```

### 2.1 核心 Preset 候选

**保留：**

- `persona`
- `agent-instructions`
- `time-context`
- `skill-filesystem`
- `tool-skill`
- `tool-web`
- `tool-fs`（已落实 read-only：`read=true`、`readImage=false`、`write=false`、`edit=false`）
- `tool-ask-user`（用于必要的澄清）
- `compaction-basic`
- `tool-result-pruner`
- `web-search-free`
- `cost-meter`
- `skill-office`（作为按需 Skill 提供者，不额外暴露多余工具）

**移除：**

- `delegation` 整组
- `tool-subagent-control`
- `tool-subagent-list-agents`
- `tool-subagent`
- `tool-subagent-fork`
- `workflow-ptc`
- `tool-workflow`
- `tool-bash` / `tool-pwsh`
- `tool-jobs`
- `tool-schedule`
- `tool-todo`
- `tool-goal`
- `tool-present`
- `tool-fs-search`（待确认：如果不需要目录级搜索则移除）
- `planning` / `plan-mode`
- `tool-plugin-manager` / `tool-plugin-market`

**待确认后保留：**

- `tool-fs`：如果 Skill references、知识库材料和写作导出必须读文件，保留但缩小路径权限；如果所有材料都由宿主传入，则删除。
- `tool-ask-user`：如果 OmniDesk 自己的交互层已处理澄清问题，可删除。
- `tool-fs-search`：如果知识检索已由宿主索引完成，删除；只有确有目录搜索需求时保留。
- `skill-office`：如果 Word/PPT/Excel 导出仍是产品能力，保留为按需 Skill 提供者。

## 3. Skill 路由

| 业务动作 | 绑定 Skill | 说明 |
| --- | --- | --- |
| 随手记 | `memory-agent` | 只做语义判定；宿主写盘 |
| 当日摘要 | `daily-summary` | 只读当天正式记录 |
| 周报 | `memory-report` | 只读当周标记为引用的记录 |
| 周期复盘 | `period-review` | 工作记录、任务、既往复盘的汇总 |
| 文案起草 | `material-writing` | 先判断用途/受众，再起草 |
| 文稿质检 | `document-review` | 独立质检动作，不和要求起草同一轮注入 |
| 知识查询 | `knowledge-search` | 两跳检索：索引 → 1–2 张卡 |
| 知识整理/建卡 | `knowledge-agent` | 仅在用户明确要求萃取或建卡时加载 |
| 项目跟踪 | `project-agent` | 日期、状态、逾期由宿主计算 |
| 公开信息检索 | `anysearch` | 只查当前公开信息，不做知识库替代 |
| Office 导出 | `skill-office` 的对应 Skill | 只在导出时加载 |

原则：一个动作最多预注入一个主 Skill；写作和质检分开，知识查询和建卡分开。

## 4. 实施步骤

### Step 1：冻结当前实际工具面（只读）

- 从仓库 `samples/profiles/discipline.profile.patch.yml` 复制一个临时 Profile，不读取真实 `~/OmniDesk` 数据根。
- 在 `/tmp` 的独立 `DSH_HOME` 下运行 `--dump-config`，记录实际 preset、工具名、schema 数量/大小、config 警告。
- 读取桌面 `scenes.ts`、`profile_rules.rs`、`SettingsPanel.tsx`，确认 `weekly-*`、`writing`、`project` 的绑定键和知识库实际的 Prompt 入口。
- 记录当前 `discipline` 的 bundle、依赖、patch 层级和默认 preset 名称。

**交付：** `evidence/optimization/01-baseline-tool-surface.json`

### Step 2：新增 `discipline-core` Preset

- 在 `samples/profiles/discipline.profile.patch.yml` 和 `runtime/dsh-home-seed/profiles/discipline/cordis.patch.yml` 中新增一个自定义 Agent preset。
- 保留 `preset-standard` 作为可回滚的完整预设，不删除上游副本。
- 将 `agent-preset-registry` 的默认 preset 指向 `discipline-core`：只覆盖 default，不动其他 preset 注册。
- 从 `discipline-core` 的 `plugins` 列表中移除 delegation、workflow、bash/pwsh、jobs、schedule、todo、goal、present、fs-search、planning、插件管理工具。

**注意：** 如果 DSH 的 `agent-preset-registry` 在当前 Profile 中无法接受新的 preset id 或默认切换，则停止本步并报告，不退回直接改 `preset-standard`。

**交付：** profile patch、seed patch、生效 preset 清单。

### Step 3：调整 `discipline.rules.json` 的 Skill 绑定

- 保留 `weekly-note`、`weekly-report`、`weekly-daily-summary`、`weekly-period-review`。
- 保留 `writing` 绑定 `material-writing`。
- 确认并增加 `writing-review` 绑定 `document-review`；如果 UI/宿主没有对应动作键，则把质检留作按需 Skill，不强行加键。
- 确认知识库查询/建卡入口是否有独立 action key；如果没有，先在知识库页保持按需加载。
- 所有动作遵守 `max_injected`，不一次性注入多个 Skill。

**交付：** 更新后的规则包与绑定表。

### Step 4：受控更新现有运行时 Profile

- 先生成 `~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.patch.yml.bak-<时间>` 备份。
- 只替换 `cordis.patch.yml` 和必要的 rules 文件，不修改 sessions、threads、engine-state 或用户成果。
- 写入前用一次临时 Profile 的 dump/启动检查确认配置可解析。
- 若宿主有 Profile 资产同步机制，优先走该机制，不手工复制整棵 Profile。

**停止条件：** 任何写入会覆盖用户数据、无法生成备份、或配置解析失败。

### Step 5：隔离验证

至少运行：

- `--dump-config`（只对 `/tmp` 的 Profile 副本）
- `--profile discipline --port 0 --no-open` 的启动探针（独立 `DSH_HOME`）
- `profile_rules` / `skill_bindings` 相关 Rust 单元测试
- `npm run build` 或当前前端构建命令
- `verify_profile_assets_109.py`、`verify_skill_bindings_082.py` 或对应当前验收脚本
- 四组业务域的合成 smoke test：工作记录、周报/复盘、文案起草/质检、项目跟踪
- 知识库查询：先索引、命中 1–2 张卡；不递归扫库

**不执行：** 真实模型调用，除非用户另行批准。

### Step 6：对比结果

记录 before/after：

- 实际可见工具数量
- tool schema 总字节数
- system prompt + runtime context 大小
- 启动到 web 地址可用的时间
- 四组业务 smoke test 的完成/失败/需要澄清次数
- 工具误调用和 schema 相关错误

只有差异有证据，才决定是否继续保留精简 preset，或恢复某些工具。

## 5. 明确不做

- 不升级 DSH 版本。
- 不修改 DSH Runtime 源码。
- 不把 Pi 纳入本轮工作。
- 不启用子 Agent。
- 不修改模型路由、Provider 或凭证。
- 不迁移、删除或重写用户 session、成果或知识库内容。
- 不做四套 Profile；保持一个 `discipline` Profile。
- 不在 Skill 中承担权限或工具存在性控制。

## 6. 验收标准

- `discipline-core` 成为实际默认 preset。
- 模型可见工具不再包含 subagent、workflow、bash/pwsh、jobs、schedule、todo、goal、present、plugin manager/market。
- 四组业务仍可完成：工作记录/复盘、文案写作、知识库、项目跟踪。
- 写作与质检、知识查询与建卡分开加载 Skill。
- 工作记录、复盘、项目状态和日期仍由宿主确定性处理，不由模型补造。
- 真实用户数据零新增写入；只产生经批准的 Profile 备份和配置变更。
- `runtime/` 无 diff；无凭证读取；无模型调用（除非另行批准）。

## 7. 回滚

- 保留 `preset-standard`，把默认 preset 指回 `standard` 即可恢复原工具面。
- 恢复 `cordis.patch.yml.bak-*`。
- 不删除旧规则文件、不删除旧 preset、不删除旧 Skill。

## 8. 需要用户批准的点

1. 是否同意新增 `discipline-core`，并把默认 preset 从 `standard` 切到它。
2. 是否确认移除 `tool-fs-search`、`tool-todo`、`tool-goal`、`tool-present`、`planning`。
3. 是否允许受控更新现有 `~/OmniDesk/runtime/dsh-home/profiles/discipline`（先备份）。
4. 是否批准一次真实引擎启动探针；不涉及真实模型调用。

用户已批准上述四点。实施结果见 `STATUS.md` 的 `OD-PROFILE-092` 与 `docs/decisions.md` 的 D-092。
