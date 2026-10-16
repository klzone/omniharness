# OmniDesk P1 第二步交付报告：引擎接口契约设计

- 报告日期：2026-10-16
- 阶段：OmniDesk P1 第二步（引擎接口契约设计）
- 文档状态：**本阶段为设计与静态检查阶段**，未实现适配器、未运行新的模型调用或在线协议探测
- **后续进展（2026-10-16，P1 第三步）**：本报告记录的是**第二步当时的状态**（仅设计、未实现）。其后已按本契约实现最小适配器 `engine/omnidesk_engine/`，
  E01–E20 测试矩阵已在隔离环境全部运行并留证（**20/20 pass**），矩阵文档已更新为 v0.2「已执行」。因此本报告中「适配器原型尚未开始」「E01–E20 全部未执行」
  等表述**仅是第二步的历史状态**，不代表当前状态；当前状态以 [../../docs/OmniDesk-Engine-Interface-Test-Matrix.md](../../docs/OmniDesk-Engine-Interface-Test-Matrix.md)
  与 [p1-engine-adapter-prototype-report.md](p1-engine-adapter-prototype-report.md) 为准。
- 上游依据：[docs/OmniDesk-Engine-Interface-Audit.md](../../docs/OmniDesk-Engine-Interface-Audit.md)（第一步）

> 本报告严格区分**设计完成**、**静态检查完成**与**真实运行验证完成**。本阶段只做到前两者；文末明确指出哪些能力仍未验证。

---

## 1. 本次实际检查了哪些文件

**规范与依据文档**：

- 任务书：`/home/kevin/.codex/attachments/8c2a76c9-84a3-4f02-abd0-90c169709f07/pasted-text-1.txt`
- [docs/OmniDesk-Design-Principles-v0.3.md](../../docs/OmniDesk-Design-Principles-v0.3.md)（项目宪法）
- [docs/OmniDesk-Product-and-Architecture-Roadmap.md](../../docs/OmniDesk-Product-and-Architecture-Roadmap.md)
- [docs/OmniDesk-Next-Phase-Brief.md](../../docs/OmniDesk-Next-Phase-Brief.md)
- [docs/OmniDesk-Engine-Interface-Audit.md](../../docs/OmniDesk-Engine-Interface-Audit.md)
- [docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md)
- [docs/OmniDesk-DSH-Isolation-Audit.md](../../docs/OmniDesk-DSH-Isolation-Audit.md)（隔离数据根结论来源）
- [docs/OmniDesk-Knowledge-Retrieval-Acceptance-Spec.md](../../docs/OmniDesk-Knowledge-Retrieval-Acceptance-Spec.md)

**引擎证据与探测脚本**：

- `tests/verification/engine-interface/evidence/channel-probe.json`
- `tests/verification/engine-interface/evidence/headless-recovery.json`
- `tests/verification/engine-interface/evidence/isolation-sideeffects.json`
- `tests/verification/engine-interface/evidence/raw/`（A0 帮助/版本、A1/A2 原始 stdout+stderr）
- `tests/verification/engine-interface/probes/{_common,probe_engine_channels,probe_isolation_sideeffects,probe_headless_recovery}.py`

**生效配置**：

- `profiles/yvan-work/cordis.patch.yml`、`profiles/yvan-work/package.json`
- `runtime/RUNTIME.lock`（底座版本锁定）

---

## 2. 新增和修改了哪些文件

| 文件 | 类型 | 说明 |
| --- | --- | --- |
| `docs/OmniDesk-Engine-Interface-Contract.md` | 新增 | 本阶段第一项交付：引擎抽象边界 + 最小接口契约（545 行） |
| `docs/OmniDesk-Engine-Interface-Test-Matrix.md` | 新增 | 本阶段第二项交付：E01–E20 测试矩阵（422 行，全部未执行） |
| `tests/verification/engine-interface/evidence/profile-persona-composition.txt` | 新增 | personaPrefix 有效值的离线合成证据（`static` + `script`，未调模型） |
| `docs/OmniDesk-Product-and-Architecture-Roadmap.md` | 修改 | 阶段 2 增加「当前进展」状态段（仅状态描述） |
| `docs/OmniDesk-Next-Phase-Brief.md` | 修改 | 下一步候选工作表增加第一步/第二步/第三步分项状态（仅状态描述） |

**未修改**：`runtime/deepseek-harness/`、`runtime/dsh-runtime/`、`.credentials.yaml`、任何凭证或用户数据。工作区既有 22 项改动全部保留，未 `git add`、未提交、未推送、未打标签。

---

## 3. 引擎接口契约的主要决定

1. **九条抽象边界原则**：OmniDesk 独立产品、自有产品层、DSH 为可替换引擎、ACP 为首选通道但非产品 API、headless 为受限补充、Web 私有协议不作契约、UI 只依赖内部契约、引擎可替换、契约与实现语言无关。
2. **标识分层**：`omni_task_id`（OmniDesk，稳定）与 `engine_session_id` / `engine_request_id` / `engine_turn_id`（适配层内部）严格分离，UI 不得依赖后者。
3. **状态模型**：九态 `queued/starting/running/cancelling/succeeded/failed/cancelled/interrupted/unknown`，含合法转换表、冲突仲裁优先级，以及三条禁止推断（进程退出≠成功、最终文本片段≠成功、kill≠优雅取消）。
4. **九组接口**（4.1–4.9）：能力与初始化、submit_task、get_task_status、stream_events、get_task_result、cancel_task、recover_session/recover_task、shutdown、错误模型。能力标记三态 `protocol_visible` / `implementation_supported` / `verified`。
5. **取消五阶段**、**恢复与新建分离且不自动重放**、**结果完整性标记**、**十三类标准错误 + 五属性**。

---

## 4. ACP、headless、SDK 的能力边界

| 通道 | 定位 | 关键边界 |
| --- | --- | --- |
| **ACP v1** | 首选适配通道 | 唯一同时具备取消、恢复（list/resume）、关闭与语义更新的 stdio 通道；取消/恢复必须以运行证据验证；默认人格前缀为 coding agent（配置层风险） |
| **headless** | 受限补充通道 | 一进程一任务；事件为有界投影（`final` 无损）；**无取消**；同 cwd `--session-id` 可 adopt，异 cwd/未知 id 拒绝 |
| **SDK** | 用途受限 | `session/prompt` 仅回 `messageId`；**实测 `session/cancel` 为 `-32603 unknown method`**；未知会话惰性创建，**无法区分恢复与新建** |
| **Web 私有协议** | 不作契约 | 业务最全但承载于未发布私有 Remote 协议；本阶段不做在线探测 |

以上 A/B/C 结论均来自第一步审计（`script`/`static`），属**历史审计证据**，不是本阶段实测。

---

## 5. 取消、恢复、关闭与人格前缀的风险

- **取消**：仅 ACP 支持真实取消。headless/SDK 的「取消」实为结束进程，契约要求暴露为 `process_terminated`，不得谎报优雅取消；取消后不得自动重放。
- **恢复**：只有 ACP 能区分「恢复既有」与「新建」。恢复失败不得静默新建；恢复不自动重放中断任务；无法确认时进入 `unknown`/`interrupted`。
- **关闭**：关闭连接、停止本地进程、取消远端任务三者分开定义；关闭超时后果须记录。
- **人格前缀**：`yvan-work` 的有效 `personaPrefix` 被确认为 coding agent 前缀（来自 `dsh-web-app` bundle 补丁层，用户层未覆盖）。**这只是配置层风险，不是模型行为结论**；本阶段只设计对照测试（P-A/P-B/P-C），不执行、不改 Profile。

---

## 6. 测试矩阵覆盖的场景及未执行项

- 覆盖 E01–E20 共 20 个场景，全部标注「未执行」。
- 覆盖契约全部 9 个接口章节与 6 项最小能力（见矩阵文档第四节覆盖检查表）。
- **未执行项 = 全部 20 项**。本矩阵是设计成果，不得当作测试通过。
- E18（人格前缀对照）需真实模型输出；E20（安全隔离）需进程级快照比对；其余为进程/协议级验证。

---

## 7. 静态检查的具体结果

| 检查项 | 方法 | 结果 |
| --- | --- | --- |
| Markdown 内部链接 | 解析相对链接并验证目标存在 | **0 处失效** |
| 契约引用路径 | 提取反引号路径并验证存在 | 4/4 存在（`profiles/`、两个 `runtime/`、证据文件） |
| 代码块围栏配对 | 统计 ` ``` ` 数量 | 契约 4（配对），矩阵 0 |
| 状态转换完整性 | 九态均出现在转换表中，非终态均有出边 | **通过** |
| 测试矩阵覆盖率 | 章节映射 + 能力映射 | 9/9 接口章节、6/6 能力均被覆盖 |
| 证据一致性 | 契约中的通道能力与第一步审计矩阵比对 | 一致（标注为历史证据） |
| Git 工作区差异 | `git status` / `git diff --stat` | 无 `runtime/` 或凭证改动；22 项既有改动保留 |

以上均为**静态检查**，不构成运行验证。

---

## 8. 尚未解决的问题与需要用户决策的事项

1. **是否批准进入第三步（最小 ACP 适配器原型）**。本阶段按任务书要求在此暂停。
2. **是否授权覆盖 `personaPrefix`**（通过 profile 补丁层，不改 runtime），并为 E18 运行真实模型对照测试。
3. **发布标签**：当前仅 `discipline-v0.2`，未创建新 tag；需用户明确批准。
4. **同 Session 并发活锁**（headless 两活进程持同一 Session）仍未实测，留待第三步。
5. **跨平台（Windows/macOS）可用性**为 `unverified`，只在 Linux 本机验证。

---

## 9. 是否存在任何代码实现

**预期为没有，实际也没有。** 本阶段仅新增/修改文档与一份离线配置合成证据文件，未新增运行时模块、协议客户端、数据库迁移或依赖项，未编写适配器代码。

---

## 10. 下一阶段适配器实施前必须满足的条件

1. 用户对本文档与测试矩阵的明确评审通过。
2. 用户明确批准进入第三步。
3. 明确 ACP 作为首个适配目标的版本锁定与隔离数据根约定。
4. 明确 E18 是否需要在此阶段执行，及其凭证使用方式（不得读取/复制生产凭证）。
5. 明确 `personaPrefix` 覆盖是否属于第三步范围。
6. 继续遵守：不改 DSH Runtime、不开发 GUI、不建 Core、不接入正式制度库、不提交推送打标签。

---

## 11. 结论

本阶段完成**引擎接口契约设计**与**接口测试矩阵设计**，并通过链接、路径、状态转换、覆盖率与证据一致性等**静态检查**。**没有任何接口经过本阶段的运行验证**；适配器原型尚未开始。第一步的通道探测证据属历史审计证据，已如实标注。
