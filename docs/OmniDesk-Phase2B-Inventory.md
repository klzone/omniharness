# OmniDesk Phase 2B 只读盘点与去重方案（待确认）

- 日期：2026-10-15
- 仓库：`git@github.com:klzone/omniharness.git`，分支 `main`，基线 `cebb4a8`（工作区含 Phase 2A 未提交改动）
- 本阶段性质：**只读盘点**。未删除、未迁移、未提交、未推送任何文件。

> 标签说明：【实测】= 本机实际执行并观测；【源码/文件】= 读取文件或源码得出；【推断】= 尚未证实的判断。

---

## 〇、盘点中发现的、需要先说明的事实

**1. 原 Phase 2 计划的「三组副本逐字节相同」结论已经过时。**【实测】

该结论写在 `docs/OmniDesk-Phase2-Plan.md` 第 232 行，但 Phase 2A 修改了仓库根副本后，只有模板一组仍完全相同：

| 重复组 | Phase 2 结论 | 当前实测 |
| --- | --- | --- |
| `templates/discipline/` vs `workstations/discipline/templates/` | 相同 | **仍完全相同**（3 文件） |
| `knowledge/writing-style/` vs `workstations/.../writing-style/` | 相同 | **已分叉**：`style-guide.md` 不同，其余 4 个相同 |
| `skills/*/SKILL.md` vs `workstations/discipline/skills/*.md` | 相同 | **已分叉**：两个文件都不同 |

这直接影响处置方式：分叉后的副本不能按「等价副本」处理，必须先判定哪一份是权威源、另一份是否含独有内容。

**2. 存在原清单未记录的第 5 组重复：`profiles/desktop/` 与 `profiles/yvan-work/` 两个目录树完全相同。**【实测】

**3. `runtime/deepseek-harness` 是 gitlink 但仓库没有 `.gitmodules`。**【实测】

```text
git ls-files -s runtime/deepseek-harness
  160000 d743267388641bc76f17c45ce8b4c231aed1d32c 0  runtime/deepseek-harness
  （无 .gitmodules 文件）
```

这意味着 Git 只记录了一个指向嵌套仓库的提交指针，却没有声明它是子模块。**新机器克隆后该目录会是空的**，无法复现运行底座。这是长期维护的真实风险，需要在本次或紧随的提交中定夺（补 `.gitmodules`，或改为忽略嵌套仓库并在文档中固定版本号）。

**4. `data-template/` 是空目录，但 README 与架构文档把它当作既有资产引用。**【实测】

`data-template/` 下只有 5 个空子目录、0 个文件；Git 无法跟踪空目录，因此仓库里实际**不存在**这个目录。而 `README.md:76` 与 `docs/ARCHITECTURE.md:74` 都在描述它。属于「文档承诺了、仓库里没有」的落差。

---

## 一、重复资产清单与建议

逐项给出：内容差异、引用者、用途、独有内容判定、建议。

### 1.1 写作风格库

| 项 | 仓库根（权威候选） | workstation 副本 |
| --- | --- | --- |
| 路径 | `knowledge/writing-style/` | `workstations/discipline/knowledge/writing-style/` |
| 文件数 | 5 | 5（文件名集合相同） |
| 内容 | 含铁律 1–14 | `style-guide.md` 缺铁律 11–14，其余 4 个逐字节相同 |
| 被谁引用 | `scripts/sync-skills.sh:34`（作为资产源）；Skill 正文通过运行时 `references/` | 无脚本引用【实测：全仓 grep 仅命中文档】 |
| **独有内容** | — | **0 行**（纯旧版子集） |
| 建议 | **定为唯一权威源** | **可删除**（无独有内容） |

### 1.2 模板库

| 项 | 仓库根 | workstation 副本 |
| --- | --- | --- |
| 路径 | `templates/discipline/` | `workstations/discipline/templates/` |
| 文件数 | 3 | 3 |
| 内容 | **逐字节相同** | 同左 |
| 被谁引用 | `scripts/sync-skills.sh:33` | 仅 `workstations/discipline/profile.yaml` 的 `workspacePaths.templates`（该文件无人加载） |
| **独有内容** | — | **0 行** |
| 建议 | **定为唯一权威源** | **可删除** |

### 1.3 Skill 正文

| 项 | 仓库根 | workstation 副本 |
| --- | --- | --- |
| 路径 | `skills/material-writing/SKILL.md`、`skills/document-review/SKILL.md` | `workstations/discipline/skills/material-writing.md`、`document-review.md` |
| 内容 | material-writing 为 **v0.3**；document-review 含资产与数据边界章节 | material-writing 为 **v0.2**；document-review 缺资产章节且仍引用旧路径 |
| 被谁引用 | `scripts/sync-skills.sh`（唯一同步源）；DSH 实际加载 | **无任何脚本引用**；DSH 不扫描此目录 |
| **独有内容** | — | material-writing 36 行独有，document-review 1 行独有（见下） |

**逐行核对独有内容的性质**（这是「不能只因相似就删」的关键检查）：

- `workstations/discipline/skills/material-writing.md` 的 36 行独有内容，全部是 **v0.2 的旧写法**：旧 description、标题「v0.2」、「三位一体参考源驱动 (Tri-Source Context Binding)」章节，以及指向 `templates/discipline/`、`knowledge/writing-style/` 的说明。这些信息在 v0.3 中以「参考资产四列表」和「写作流程」形式**已被覆盖且更完整**，不构成 v0.3 缺失的知识。
- `workstations/discipline/skills/document-review.md` 唯 1 行独有内容，是旧路径 `knowledge/writing-style/avoid-list.md`——正是 Phase 2A 判定为「运行时无法解析」而修正掉的那一处。属**被修正掉的错误**，不是独特价值。

| 建议 | **定为唯一权威源** | **可删除**（独有内容均为被取代的旧版或已修正的错误） |

### 1.4 知识库说明文件（6 类目录）

`knowledge/{excellent-cases,meeting-materials,methodology,policies,regulations}/README.md` 与 workstation 副本对比【实测】：

| 目录 | 差异 |
| --- | --- |
| `meeting-materials/`、`methodology/`、`policies/`、`regulations/` | README **逐字节相同** |
| `excellent-cases/` | **不同**：workstation 副本为旧版（无「当前尚无实际范例」声明） |

`workstations/discipline/knowledge/excellent-cases/README.md` 独有 2 行（旧标题「## 包含品类」与旧 Few-shot 措辞），均为被新版取代的旧表述。

建议：`knowledge/` 为权威源，`workstations/discipline/knowledge/` 整棵**可删除**。

### 1.5 Profile 目录（原清单未记录）

`profiles/desktop/` 与 `profiles/yvan-work/` 两个目录树**完全相同**（`cordis.patch.yml` 与 `package.json` 均一致）。【实测】

背景【源码/文件】：隔离审计报告已证实「仓库里的 `profiles/desktop/cordis.patch.yml` **不是**原版 Desktop 正在读取的那一份」——原版读的是 `~/.dsh/profiles/desktop/`。因此仓库内的 `profiles/desktop/` 是一份**静态快照**，不是运行配置。

建议：保留 `profiles/yvan-work/`（真实生效的隔离 Profile 源）；`profiles/desktop/` 建议删除或改名为明确的参考快照。**此项置信度较低**，因它可能是你有意留作对照的备份，需你确认。

### 1.6 其他

| 项 | 观察 | 建议 |
| --- | --- | --- |
| `workstations/discipline/profile.yaml` | 7 个顶层键在 DSH Profile 格式中不存在；`cordisPatch` 与 yvan-work 有 4 处条目重复；`workspacePaths` 指向的 `~/OmniDesk/workspaces/discipline`、`~/OmniDesk/knowledge`、`~/OmniDesk/memory` **均不存在**【实测】 | **不删除**，改为明确定位的规划文件（见第三节） |
| `workstations/discipline/memory/usage-log/` | 仅 README；用途是 Phase 3 使用记录 | **保留**，但需澄清它属于「项目内记录」还是「外部数据层 `~/YvanAI/memory`」 |
| `skills/coding-agent/`、`skills/writing-agent/` | **空目录，0 文件**【实测】 | Git 不跟踪空目录，实际不存在；需补齐或删除 README 中的承诺 |
| `data-template/` | 空目录，README/ARCHITECTURE 引用 | 补齐占位文件或修正文档 |
| `scripts/__pycache__/` | 已被 `.gitignore` 忽略 | 无需处理 |
| `profiles/yvan-work/package.json` 的 `name` | 仍为 `dsh-profile-desktop` | 改为 `dsh-profile-yvan-work`（不影响加载，仅可读性） |
| `tests/reports/citation-audit-report.md` | 声称 30/30 抽查、96.7% 准确率 | 该报告早于隔离验证，其真实性未经本次复核，**不据此下结论** |

---

## 二、唯一权威源映射

下表的左侧是「人编辑的唯一位置」，右侧是「由脚本生成的运行时副本」。规则：**只改左侧，右侧永不手改。**

| 资产 | 唯一权威源（仓库） | 当前引用者 | 运行时副本（生成物） |
| --- | --- | --- | --- |
| Skill 正文 | `skills/<name>/SKILL.md` | `scripts/sync-skills.sh` | `~/OmniDesk/runtime/dsh-home/skills/<name>/` |
| 模板 | `templates/discipline/` | `scripts/sync-skills.sh` | `.../skills/{material-writing,document-review}/references/templates/` |
| 风格规则 | `knowledge/writing-style/` | `scripts/sync-skills.sh` | `.../references/writing-style/` |
| 历史范例 | `knowledge/excellent-cases/` | `scripts/sync-skills.sh` | `.../references/excellent-cases/` |
| 知识库分类 | `knowledge/{meeting-materials,methodology,policies,regulations}/` | 暂无脚本（供 `knowledge-search` 使用） | 未物化 |
| DSH Profile | `profiles/yvan-work/` | `scripts/omnidesk-dev.sh:173` | `~/OmniDesk/runtime/dsh-home/profiles/yvan-work/` |
| 数据底座规范 | `config/yvan-work.yaml` | `scripts/init-yvan-data.sh:10` | `~/YvanAI/yvan-work.yaml` |

**这套规则其实已经被 `scripts/sync-skills.sh` 编码实现**（第 32–35 行的三处 `ASSET_*_SRC` 与 `SOURCE_DIR`）。Phase 2B 要做的是让**文档与其余目录**与脚本保持一致，而不是新发明一套规则。

### 需要修正的过时引用

| 位置 | 当前内容 | 应改为 |
| --- | --- | --- |
| `README.md:106` | 链接 `workstations/discipline/` 作为「工作站工作目录」 | 指向权威源；工作站目录改为「规划与说明」定位 |
| `README.md:76` | 描述 `data-template/` | 补齐该目录或删去该行 |
| `README.md` 目录规划段 | 未列出 `templates/`、`knowledge/`、`workstations/` | 补全，与第二节映射一致 |
| `CHANGELOG.md:10` | 「`workstations/discipline/knowledge/` 与 `knowledge/`」并列 | 收敛为单一来源表述 |
| `docs/OmniDesk-Phase2-Plan.md:232` | 「三组逐字节相同」 | 标注已过时（或加修订说明） |
| `docs/OmniDesk-Web-Notice-Error-Fix.md:54` | 列 `workstations/discipline/profile.yaml` 为需同步的配置 | 改为「规划文件，不生效」 |

---

## 三、计划删除或迁移的文件及理由

**本节仅为计划，尚未执行任何删除。** 每一项都已先核实独有内容。

### 3.1 建议删除（替代为指向性说明文件）

| # | 路径 | 文件数 | 删除理由 | 独有内容核实 |
| --- | --- | --- | --- | --- |
| 1 | `workstations/discipline/knowledge/`（整棵 6 目录） | 10 | 全部为 `knowledge/` 的等价或旧版副本；无脚本引用 | `writing-style/style-guide.md` 为旧版子集（0 行独有）；`excellent-cases/README.md` 2 行旧表述；其余逐字节相同 |
| 2 | `workstations/discipline/templates/` | 3 | 与 `templates/discipline/` 逐字节相同 | 0 行独有 |
| 3 | `workstations/discipline/skills/` | 2 | 旧版 Skill 副本；DSH 不扫描此目录 | material-writing v0.2 已被 v0.3 覆盖；document-review 1 行是已修正的错误路径 |

删除后在这些位置各放一个 `README.md`，写明「内容已收敛到仓库根 `<权威源>`，此处不再维护副本」。**保留 `workstations/discipline/` 这个工作站概念本身**，符合 Phase 2 Plan Step 6 的原始意图。

### 3.2 建议迁移/改定位（不删除）

| # | 路径 | 处置 | 理由 |
| --- | --- | --- | --- |
| 4 | `workstations/discipline/profile.yaml` | 保留，但**改名并加醒目头注**，明确「本文件无加载器读取，生效配置在 `profiles/yvan-work/`」 | 它有 4 处与 yvan-work 重复的补丁条目，且 `workspacePaths` 指向不存在的路径，容易被误当作生效配置。作为工作站**规划声明**仍有价值 |
| 5 | `profiles/yvan-work/package.json` 的 `name` 字段 | 改为 `dsh-profile-yvan-work` | 消除「看似 Desktop」的误导 |
| 6 | `workstations/discipline/memory/usage-log/` | 保留位置，但在 README 中写明它记录的是**项目内**工作记录，与外部数据层 `~/YvanAI/memory` 的长期记忆区分 | 避免两处 memory 语义混淆 |

### 3.3 待你定夺

| # | 路径 | 问题 |
| --- | --- | --- |
| 7 | `profiles/desktop/` | 与 `profiles/yvan-work/` 完全相同；是否为有意保留的 Desktop 参考快照？若否，建议删除 |
| 8 | `runtime/deepseek-harness`（gitlink） | 补 `.gitmodules` 使其可克隆复现，还是改为忽略嵌套仓库、只在文档固定 commit？ |
| 9 | `data-template/`（空） | 补占位文件让它真正存在，还是从 README 中移除？ |
| 10 | `skills/coding-agent/`、`skills/writing-agent/`（空） | 补 `SKILL.md`，还是从 README 的技能列表中移除？ |

---

## 四、同步器删除机制的风险审查

审查对象：`scripts/sync-skills.sh` 第 4.5 节「清除目标中已不存在于源目录的 Skill」及 `--check` 的对应判定。

### 4.1 已做对的部分

| 要求 | 现状 | 判定 |
| --- | --- | --- |
| 只允许删除隔离运行根中的内容 | 目标由已校验的 `OMNIDESK_ROOT` 推导，且拒绝落在 `~/.dsh`/`~/.agents`；拒绝软链接目标 | **满足** |
| 源目录为空时不得删除 | `[ "$skill_count" -gt 0 ]` 否则直接退出（`die`） | **满足** |
| 路径异常时不得删除 | 目标为空、越界、等于原版根均 `die`；`[ -d ]` 保护 nullglob | **满足** |
| `--check` 检查内容与资产漂移 | 已按文件内容 `diff -rq`，并对三类资产做内容级比对 | **满足**（Phase 2A 修复） |
| dry-run 不产生写入 | 已验证前后哈希不变，且 dry-run 只提示不删除 | **满足** |

### 4.2 发现的真实缺陷

**缺陷 A（高）：会把用户在运行时根自建的 Skill 当作「多余」删除。**【实测】

复现（在隔离根新建一个用户自有 Skill，未删任何内容，测试后已清理）：

```text
创建 ~/OmniDesk/runtime/dsh-home/skills/my-own-skill/SKILL.md
--check   → rc=1  [多余] my-own-skill（目标存在但源中不存在）
--dry-run → （dry-run）将删除多余 Skill：my-own-skill（源目录中已不存在）
```

也就是说，任何「不在仓库 `skills/` 里」的目录都会被视为待删。这**直接违反**本阶段要求「对用户在运行时根自行创建、且不属于同步器管理范围的 Skill，不得误删」。

连带影响：`--check` 会因此**永久报漂移**，`omnidesk-dev.sh` 每次启动都会警告，最终会诱导用户忽略该警告。

**缺陷 B（中）：无法区分「同步器管理」与「用户自建」。**

当前没有任何托管清单/标记文件。删除范围等于「目标目录下全部子目录减去源目录子目录」，范围过大。

**缺陷 C（中）：同步失败可能留下半更新状态。**

逐个 Skill 执行 `rsync`，中途失败时前面已更新、后面未更新，且没有事务或状态记录可供识别。用户要求「同步失败时不得留下无法识别的半更新状态；如当前实现无法保证，应提出最小修复方案」。

**缺陷 D（低）：dry-run 输出不够结构化。**

用户要求「dry-run 应能清楚列出新增、更新和删除项」。当前新增/删除有明确行，但更新依赖 `rsync --itemize-changes` 的原始字符（如 `>f+++++++++`），不够直白，也没有合计。

### 4.3 最小修复方案（建议，本阶段不实施）

**引入托管清单**，解决 A、B 两项（其余保持不变）：

```text
同步成功后写出  <隔离根>/skills/.omnidesk-managed
  内容 = 本次由同步器创建/更新的 Skill 名列表

清除逻辑改为：
  只删除「清单里存在，但源目录已不存在」的目录；
  清单外、源目录也没有的目录 → 判定为用户自有，保留并在输出中提示「未托管，已跳过」。

--check 相应改为：
  未托管目录不再计入漂移，仅作信息提示。
```

**针对缺陷 C 的最小方案**（二选一，后者改动更小）：

- 方案一（更稳妥）：先同步到 `<隔离根>/skills/.staging/`，全部成功后再整体替换，失败则丢弃 staging，原副本不受影响；
- 方案二（改动最小）：保留现有逐 Skill 同步，但在全部成功后**才**写托管清单；失败时明确打印「本次同步未完成，副本可能处于半更新状态，请重新执行 `sync-skills.sh`」，使失败可被识别而非静默。

**针对缺陷 D**：把 dry-run 输出改为三分组摘要（`将新增 / 将更新 / 将删除`）+ 末尾合计，保留 `rsync` 明细作为可选的 `--verbose` 附注。

以上修复建议**单独提交**，与本阶段的「去重收敛」分开，避免一个提交同时改变目录结构与删除语义。

---

## 五、测试与提交计划

### 5.1 本次只读阶段已运行的测试

| 测试 | 结果 |
| --- | --- |
| 隔离自检 `scripts/omnidesk-isolation-check.sh` | **37 项通过 / 0 失败**【实测】 |
| `sync-skills.sh --check` | **未发现漂移**（隔离副本与仓库源一致）【实测】 |
| 同步器误删风险复现 | **已复现缺陷 A**（用户自有 Skill 被列入待删）【实测】，测试内容已清理 |
| DSH Runtime 未修改 | 嵌套仓库 HEAD 仍为 `d7432673…`，工作树干净【实测】 |
| 凭证/敏感信息扫描 | 82 个跟踪文件未发现 API key、私钥、token 模式【实测】 |
| 真实业务文档 | `knowledge/regulations/` 下的实际文件已被 `.gitignore` 排除，仅 `README.md` 入库【实测】 |

**未读取、未修改**原版 `~/.dsh` 的凭证与会话内容；本阶段未做任何模型调用。

### 5.2 确认后的提交计划

按「一次提交只做一件事」拆分，**不推送**直到你明确同意：

| 提交 | 内容 | 前置 |
| --- | --- | --- |
| C1 | Phase 2A 的全部改动（12 文件）+ Phase 2A 报告 | 已就绪 |
| C2 | 去重收敛：删除 3.1 的三处副本，替换为指向说明；更新 README/CHANGELOG/docs 的过时引用 | 你确认 3.1 |
| C3 | 定位修正：`profile.yaml` 改定位、`package.json` 改名、（若采纳）`profiles/desktop` 处置 | 你确认 3.2 / 3.3 |
| C4 | 同步器加固：托管清单 + dry-run 摘要 + 半更新提示 | 你确认 4.3 |
| C5 | 仓库可复现性：`.gitmodules` 或版本固定方案；`data-template/`、空 Skill 目录补齐 | 你确认 3.3 |

每个提交前都会重跑 5.1 的测试，并在提交说明中写明「内容等价、位置收敛」，避免被误读为删除资产。

### 5.3 提交守卫

- 不提交模型凭证，不为通过测试而复制生产凭证；
- 不提交真实业务文件（`knowledge/regulations/` 实际内容保持忽略）；
- 不修改 `runtime/deepseek-harness`；
- 去重提交**只删除已核实无独有内容的副本**，并在说明中列出被删除路径与理由。

---

## 六、结论与请求确认

可以进入实施，但有三点需要你先拍板：

1. **是否同意 3.1 的三处删除**（`workstations/discipline/` 下的 knowledge、templates、skills），替换为指向说明文件；
2. **`profiles/desktop/` 是删除还是保留为参考快照**（3.3 第 7 项）；
3. **`runtime/deepseek-harness` 采用子模块还是版本固定**（3.3 第 8 项）——这项决定新机器能否复现底座，影响最大。

另有一项与去重无关、但建议优先修：**同步器的托管清单机制（4.2 缺陷 A）**。它当前会删除用户在隔离根自建的 Skill，属于会真实造成数据丢失的问题。

按任务要求，**本阶段到此暂停**，未执行任何删除、迁移或提交。
