# OmniDesk Phase 2A 变更报告：材料写作 Skill 端到端调用链修复

- 日期：2026-10-15
- 仓库：`git@github.com:klzone/omniharness.git`，分支 `main`，基线 commit `cebb4a8`
- 运行时底座：`runtime/deepseek-harness`，tag `dsh-v0.2.1-alpha.2`（**只读，未改动**）
- 本阶段状态：**未提交、未推送、未合并任何重复文件**；完成后暂停等待确认

> 证据分级说明：本报告区分【运行时实测】= 在本机实际执行并观察到结果；【静态验证】= 仅检查文件正文/源码，**未**经过模型运行；【源码已证实】= 来自 DSH 源码的事实。**静态检查结果不等同于运行时测试通过。**

---

## 一、修改文件清单与关键 diff 摘要

| 文件 | 变更量 | 关键改动 |
| --- | --- | --- |
| `scripts/sync-skills.sh` | +186 / -18 | 重写为「隔离安全 + 资产物化 + 漂移检查」版本（详见下） |
| `scripts/omnidesk-dev.sh` | +45 / -1 | 新增 `--sync`；默认启动只做只读检查，不再无条件覆盖 |
| `skills/material-writing/SKILL.md` | +112 / -38 | 升级为 v0.3：数据边界、资产表、用途优先流程、交付格式 |
| `skills/document-review/SKILL.md` | +11 / -3 | 资产引用改为 `references/`；新增资产与数据边界章节 |
| `knowledge/writing-style/style-guide.md` | +9 | 新增流程性铁律 11–14（原有 1–10 未改动） |
| `knowledge/excellent-cases/README.md` | +11 / -2 | 明确声明目录当前为空、禁止伪造、脱敏前置 |
| `skills/{knowledge-agent,knowledge-search,memory-agent,project-agent}/SKILL.md` | 各 -1 | 删除无效的 `model-invocable: true`（见下「源码已证实」） |
| `workstations/discipline/skills/{material-writing,document-review}.md` | 各 -1 | 同上 |

### 关键 diff 摘要

**1. `sync-skills.sh`（同步链路核心）**

保留的原有安全护栏：目标由已校验的 `OMNIDESK_ROOT` 推导、拒绝写入 `~/.dsh`/`~/.agents`、拒绝软链接目标、拒绝空源执行 `--delete`、`rsync -a --delete --itemize-changes`。

新增能力：

- `--check`：只读漂移比对，有漂移则退出码 1，**不写入**；
- 源清单：列出每个 Skill，并对「缺 `SKILL.md`」或「使用已废弃 camelCase frontmatter 键」的情况打标；
- 资产物化：把写作资产同步进各绑定 Skill **自己的** `references/` 目录（`templates/`、`writing-style/`、`excellent-cases/`，并生成说明用 `README.md`）；
- **清除目标中已不存在于源目录的 Skill**（本次实测中发现并补充修复，见第四节场景 7）；
- `--check` 增加**资产内容级**比对，而非仅检查 `references/` 目录是否存在（同样是本次补充修复）。

**2. `omnidesk-dev.sh`（启动器）**

- 新增 `--sync` 旗标；默认情况下启动前只执行 `sync-skills.sh --check`（只读），发现不一致时**警告并提示手动命令，不覆盖**；
- 只有显式传入 `--sync` 才会真正执行同步覆盖，避免抹掉用户在隔离实例中的合法改动；
- 同步失败不阻断启动。

**3. `material-writing/SKILL.md` v0.3（结构）**

```text
〇、数据边界（最高优先级，先于一切写作任务）
一、参考资产：位置、权威源与失败行为  ← 运行时路径/权威源/加载方式/缺失时行为 四列表
二、写作流程：第1步 识别用途 → 第2步 决定是否使用模板 → 第3步 结构大纲
    → 第4步 正文初稿 → 第5步 区分 事实/分析/判断/建议 → 第6步 完稿检查
三、交付物格式【模板使用】【风格规则】【参考资料】【事实来源】【待确认事项】【保存位置】
四、底线要求
```

**4. 关于 `model-invocable: true`（源码已证实）**

`packages/skill/skill-filesystem/src/index.ts` 的 `parseInvocationPolicy` 只认 kebab 键 `disable-model-invocation` 与 `user-invocable`；camelCase 的 `modelInvocable` / `userInvocable` / `disableModelInvocation` 会导致**整个 Skill 被静默忽略**，而 kebab 的 `model-invocable` 则是无效键。原先各 Skill 中残留的 `model-invocable: true` 属无效声明，本次统一清除，不影响 `user-invocable: true` 的生效。

---

## 二、Skill 权威源与运行时副本路径图

唯一权威源为仓库 `skills/`；运行时副本位于 OmniDesk 隔离数据根（默认 `~/OmniDesk`，可用 `OMNIDESK_ROOT` 覆盖）。

| 用途 | 仓库权威源 | 运行时路径 | 加载方式 |
| --- | --- | --- | --- |
| Skill 正文 | `skills/<name>/SKILL.md` | `~/OmniDesk/runtime/dsh-home/skills/<name>/SKILL.md` | DSH 扫描 `user-dsh` 根 |
| 写作模板 | `templates/discipline/` | `.../skills/{material-writing,document-review}/references/templates/` | Skill 用 read 工具按相对路径读取 |
| 风格规则 | `knowledge/writing-style/` | `.../references/writing-style/` | 同上 |
| 历史范例 | `knowledge/excellent-cases/` | `.../references/excellent-cases/` | 同上 |

**为什么资产必须放进 Skill 目录内**（源码已证实）：`packages/skill/skill/src/index.ts` 的 `renderSkillContent()` 在注入时会附带 `Base directory for this skill: <skill dir>` 与「按该基准目录解析相对路径」的提示；同时 `discoverRoot()` 是**非递归**的——只认 `<dir>/SKILL.md` 或扁平 `*.md`。因此资产放在 Skill 目录的 `references/` 子目录内，是既符合 DSH 注入约定、又不被扫描器误认的稳定做法。

隔离边界：以上全部路径位于 `OMNIDESK_ROOT` 之下，**不触碰** `~/.dsh`、`~/.agents` 与原 Desktop 数据。

---

## 三、同步流程说明

```text
scripts/sync-skills.sh            # 实际同步：skills/ + 写作资产 → 隔离根
scripts/sync-skills.sh --check    # 只读：仓库源与隔离副本是否一致（退出码 1 = 有漂移）
scripts/sync-skills.sh --dry-run  # 预览将要发生的改动，不写入
scripts/sync-skills.sh --to-agents  # 改同步到隔离根的 agents skills
scripts/omnidesk-dev.sh --sync    # 启动前显式同步
```

流程与保护：

1. **目标推导**：目标永远由已校验的 `OMNIDESK_ROOT` 推导，不接受任意目标路径；
2. **危险根拒绝**：落在 `~/.dsh`、`~/.agents` 之下或等于原版数据根，一律拒绝；
3. **软链接拒绝**：目标若为软链接则拒绝，避免穿透到未校验位置；
4. **空源保护**：源目录为空时拒绝执行 `--delete` 同步；
5. **不默认覆盖**：启动流程默认只做只读检查；只有显式 `--sync` 才覆盖；
6. **同步顺序**：先逐 Skill 同步（含清除源中已不存在的 Skill），**再**物化 `references/` 资产，确保资产不被 `--delete` 清掉。

---

## 四、验收场景测试结果

测试基线：隔离根 `~/OmniDesk/runtime/dsh-home`；Skill 发现由真实 `FileSystemSkillProvider` 无头驱动（`list()` + `get()`），不经过 LLM。

| # | 场景 | 结果 | 证据 |
| --- | --- | --- | --- |
| 1 | 模板+规则+参考齐全，生成工作总结 | **未执行·阻塞** | 触发器需模型调用；隔离环境返回 `MISSING_CREDENTIAL`。资产存在性已【运行时实测】 |
| 2 | 无适配模板 → 按用途设计结构 | **静态验证通过**，运行时未执行 | Skill 第 2 步明确「只有匹配才套用，无适配则提出结构方案」 |
| 3 | 关键数据缺失 → 标注待确认 | **静态验证通过**，运行时未执行 | 第 1/5 步 + 交付格式【待确认事项】 |
| 4 | 两条事实矛盾 → 指出冲突 | **静态验证通过**，运行时未执行 | `style-guide.md` 铁律 13 + 第 1 步「矛盾时先追问或标注」 |
| 5 | 历史范例目录为空 → 诚实说明 | **运行时实测（目录）+ 静态（正文）** | 实测目录仅含 `README.md`；README 与 Skill 正文均强制声明「未使用历史范例」 |
| 6 | 提交禁止范围材料 → 执行数据边界 | **静态验证通过**，运行时未执行 | Skill 〇 章节规则 1–3（停止处理、禁自动脱敏、提示合规系统） |
| 7 | 改测试 Skill 标记并同步 → 隔离实例加载新版；原 DSH 不变 | **运行时实测通过** | 见下 |

### 场景 7 实测明细（完整执行）

```text
A. 基线：~/.dsh = 8c77dafe6f14df1f   ~/.agents = abcfa6a9d4df344d
B. 在仓库新增临时 Skill（MARKER-V1）
   --check  → rc=1，报 [漂移] zz-phase2a-test     （正确检出新增）
C. sync     → rc=0，隔离根出现 zz-phase2a-test
   隔离实例发现 → COUNT=7，包含 zz-phase2a-test，正文含 MARKER-V1
D. 仓库标记改为 MARKER-V2
   隔离副本仍为 V1（grep V1=3, V2=0）              （证明副本非实时读取）
   --check  → rc=1，报漂移
E. sync     → rc=0，隔离副本变为 V2
   隔离实例实际加载的正文 = MARKER-V2               （同步生效，端到端证实）
F. 删除临时 Skill 后的收敛问题：
   首次 sync 未能清除目标残留（旧实现缺陷）→ 已修复为显式清除
   修复后 sync → 输出「删除多余 Skill」，隔离根已清除
   终态 --check → rc=0「未发现漂移」，隔离实例 COUNT=6
G. 原版数据复核：
   ~/.agents            = abcfa6a9d4df344d  （与基线一致，未变）
   ~/.agents/skills     = abcfa6a9d4df344d
   ~/.dsh 整体           = 8c77dafe6f14df1f  （与基线一致，未变）
```

**场景 1–4、6 的阻塞证据**（在隔离环境实际执行模型调用）：

```text
dsh: MISSING_CREDENTIAL: llm-deepseek: no API key for provider route "deepseek-official";
store DEEPSEEK_API_KEY through the credentials service (the web Models page writes it),
or export DEEPSEEK_API_KEY in the launching environment
```

隔离根 `.credentials.yaml` 中仅含 `client-connection/browser-session`，无 LLM 密钥；环境变量中的各类模型密钥均未设置。因此涉及模型生成与判断的场景**无法在本机跑通**，按任务要求如实记为未执行，并**未降低验收标准、未伪造输出**。

### 附加验证

- **隔离自检回归** `scripts/omnidesk-isolation-check.sh`：**37 项通过 / 0 项失败**（含「sync 的 dry-run 不产生任何写入」「拒绝写入 `~/.dsh` / `~/.agents`」）。
- **漂移检测正对照**：向 `knowledge/writing-style/style-guide.md` 追加一行后，`--check` 报出两个 Skill 的 `references/writing-style` 漂移（rc=1）；还原后 sha256 完全一致，`--check` 恢复 rc=0。
- **dry-run 无写入**：新增清除逻辑后，`--dry-run` 前后隔离目录哈希不变；且 dry-run 只提示「将删除」，不真的删除多余 Skill。
- **启动命令行**：`omnidesk-dev.sh --dry-run` 仍输出 `... --profile yvan-work --host 127.0.0.1 --port 0 --no-open`，且不含原版 `~/.dsh` 路径。

### 一处环境因素导致的自检波动（如实记录）

首次运行隔离自检时出现 **1 项失败**：`原环境 ~/.dsh 发生变化`，差异落在 `sessions/.../session.v4.jsonl.zstd`。

归因实测：在**完全没有 OmniDesk 活动**的 12 秒间隔内两次快照 `~/.dsh`，`sessions` 子目录哈希发生变化，而 `skills` 与 `cache` 子目录哈希保持不变。结论：变化来自**正在运行的原版 DSH Desktop 自身写入会话数据**，与本阶段改动无关。复核运行即恢复 37/37。该断言标准未改动。

---

## 五、未解决问题与限制

1. **模型链路无法在本机验证（主要限制）**：缺少任一模型提供商凭证，场景 1–4、6 只能做到静态验证。这四项需要你在真实运行环境中补齐凭证后复测。
2. **重复文件树已出现分叉**：`knowledge/writing-style/style-guide.md` 与 `workstations/discipline/knowledge/writing-style/style-guide.md` 内容已不一致（后者为旧版）；`skills/material-writing/SKILL.md` 与 `workstations/discipline/skills/material-writing.md` 亦不一致（后者为 v0.2）。按本阶段约束**未做合并**，但需尽快决定唯一权威源，否则将来会出现「改了一处、另一处不生效」。
3. **`workstations/discipline/profile.yaml` 目前是声明式的**，没有 DSH 加载器读取它；真正生效的是 `~/OmniDesk/runtime/dsh-home/profiles/yvan-work`。这个落差需要澄清或补齐。
4. **资产为「副本」而非「直读」**：`references/` 是同步生成的物化副本，改了仓库源必须重新同步才生效。已通过 `--check` 在启动时给出提示降低遗忘风险，但不会自动覆盖。
5. **`skills/coding-agent/`、`skills/writing-agent/` 为空**（无 `SKILL.md`），DSH 不会发现它们；同步脚本现已会在源清单中标注。
6. **`profiles/yvan-work/package.json` 的 `name` 仍为 `dsh-profile-desktop`**（仅影响显示，未改）。
7. **历史范例目录为空**，按设计诚实处理；在放入脱敏真实范例前，Skill 不会声称使用了历史范例。
8. **`workstations/discipline/memory/usage-log/` 仅有 README**，尚无真实使用记录（属 Phase 3 范围）。

---

## 六、建议的下一步工作

1. **补凭证后复测场景 1–4、6**：在隔离实例中配置至少一个模型提供商，用合成非敏感素材跑通这五项，取得真实运行时结果后再判定材料写作链路是否端到端可用。
2. **确定唯一权威源并去重**（建议单独提交）：把 `workstations/discipline/knowledge/`、`workstations/discipline/templates/`、`workstations/discipline/skills/` 收敛为软链接或统一指向 `knowledge/`、`templates/discipline/`、`skills/`，避免继续分叉。
3. **给 `sync-skills.sh` 增加一个可选的 `--post-check`**：同步后自动执行 `--check` 自我确认收敛，使 CI 或手动流程都有闭环。
4. **建立启动时漂移告警的可选严格模式**：例如 `OMNIDESK_STRICT_SYNC=1` 时，检查不通过就拒绝启动，供「只跑权威版本」的场景使用。
5. **Phase 3 前置**：在补齐真实范例与使用记录目录内容后，再扩展 Golden Tasks 与知识库引用测试（本阶段未触碰）。

---

## 交付与暂停声明

- 本阶段**未提交、未推送**，改动全部保留在工作区；
- **未修改** `runtime/deepseek-harness`；
- **未修改** `~/.dsh`、`~/.agents` 或原 Desktop 数据（已按上述哈希核对）；
- **未修改**模型提供商、API 密钥或运行配置；
- **未安装**插件、**未开发** GUI、**未新增** Agent 编排；
- **未合并**任何重复文件。

请确认后再进入下一阶段（提交 / 去重 / 补凭证复测）。
