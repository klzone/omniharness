# 阶段一证据适用性索引（DSH / 引擎接口材料）

- step_id: DSH-P1-05
- generated_at: 2026-11-12T07:22:30Z
- 范围：现有 DSH / 引擎接口文档、决策与测试报告元数据，不重新执行历史测试。
- 证据口径：本文件中的 `R（历史）` 仅表示原报告记录过一次脚本运行；本次没有复跑，不能当作当前 rc.5 行为证据。

## 证据等级

| 标记 | 本索引中的含义 |
| --- | --- |
| F | 可复核的文件、版本、日期或路径事实 |
| S | 文档内引用的静态源码结论，未由本卡复查 |
| R（历史） | 原报告记录的历史脚本运行结果，本卡未复跑 |
| D | 设计说明、决策记录或官方/项目说明 |
| U | 未验证或版本不适用 |

## 适用性索引

| 材料 | 版本/日期 | 可保留结论 | 失效或不适用 | 证据等级 | 下一步 |
| --- | --- | --- | --- | --- | --- |
| `docs/DSH-Architecture-Analysis.md` | v0.2.1-alpha.2；2026-10-14 | 组件分层、Skill/工具/会话概念地图可作为阅读线索 | 精确 API、插件面、模型路由和行为结论均不能用于 rc.5；部分环境路径是另一台主机 | D；U（机制行为） | 只保留概念图；rc.5 结论回到编译产物或精确源码复核 |
| `docs/OmniDesk-DSH-Isolation-Audit.md` | v0.2.1-alpha.2；2026-10-15 | `DSH_HOME` 覆盖整棵数据树、app.asar 需执行探测而非文件存在性判断、第三方插件会在 home 落盘 | 版本、PID、路径与插件清单已过时；未覆盖 rc.5 / 0.2.2-rc.1 | D/F；U（当前隔离） | 在 rc.5 与当前宿主重跑隔离自检和前后哈希对照 |
| `docs/OmniDesk-DSH-Isolation-Implementation.md` | v0.2.1-alpha.2；2026-10-15 | 隔离启动器、数据根解析与零改动检查的历史实现思路 | 不是当前运行时证据；命令、PID 和 `--version` 结果属于 alpha.2 | D；R（历史） | 以 rc.5 CLI 重做隔离启动与只读副作用检查 |
| `docs/OmniDesk-Engine-Interface-Audit.md` | v0.2.1-alpha.2；2026-10-16 | ACP / headless / SDK 三通道差异、会话恢复边界、错误模型的早期观察 | 推荐 ACP 为默认通道的结论已被 D-024 第 3 步的 web-api 路径取代；文件行号和 rc.5 实现不一致；未覆盖 web-api 细节 | D/S；U（rc.5） | 以 rc.5 编译 JS 复核 ACP 与 web-api 的入口、能力和错误码 |
| `docs/OmniDesk-Engine-Interface-Contract.md` | v0.2；2026-10-16 | OmniDesk 自有任务状态、错误归一、取消/恢复仲裁、UI 不解析协议等契约语义 | 上游 alpha.2 行号、ACP 默认路径和 `engine_version` 示例已过时；契约未证明 web-api 实现完全满足 | D（产品契约）；U（rc.5 实现） | 保留契约语义；对 web-api 适配器逐条做实现映射与运行验证 |
| `docs/OmniDesk-Engine-Interface-Test-Matrix.md` | v0.2；2026-10-16 | E01–E20 场景设计、检查项、E18 model 臂阻塞的诚实标注 | 矩阵的 20/20 pass 是 alpha.2 历史结果；rc.5 升级后 E17/E20 回归失败；E18 仍无真实模型证据 | R（历史）；U（rc.5） | 复跑 E01–E20，先修正 E17 过窄断言，再对 E20 做独立复跑汇总 |
| `docs/OmniDesk-Session-Import-Compatibility.md` | v0.2.1-alpha.2；2026-10-18 | 锁定版 ACP 没有会话导入入口；不应复制或改写 DSH 原生会话格式 | 只覆盖 ACP 路径；当前 web-api 路径、rc.5 格式和跨版本兼容均未验证 | D/F；U（web-api / 跨版本） | 对 rc.5 的 web-api 会话接口与持久化格式单独审计；不执行导入实验 |
| `docs/decisions.md` D-002 | 已定（无单列日期） | DSH 是可替换执行引擎依赖；不修改 `runtime/` 内 DSH Runtime；任务与成果独立保存 | 不涉及具体版本能力；不能用它证明某一版本行为 | D | 保持为架构边界；rc.5 审计继续遵守 |
| `docs/decisions.md` D-012 | 已定（无单列日期） | DSH 会话恢复不等于 OmniDesk 任务续跑；无可靠终态证据不得自动重放 | 具体锁定版本能力须逐版本验证；当前 rc.5 / web-api 未复核 | D；U（当前版本） | 在 web-api 适配器上验证 session resume、终态和重放边界 |
| `docs/decisions.md` D-016 | 历史决策（无单列日期） | 曾确定 Rust 桌面宿主默认使用 ACP Profile，并要求默认值经 `EngineConfig::from_env()` 等路径验证 | 已被 D-024 第 3 步的 web-api 路径取代；当前 `engine_supervisor.rs` 的通道默认是 web-api | D（历史）；U（当前默认） | 作为历史记录保留；当前默认以 rc.5 源码和实际装配为准 |
| `docs/decisions.md` D-024 与第 3 步裁定 | 2026-10-20 / 2026-10-21 | 换接缝的权衡、ACP 窄接口限制、路径 B（DSH 本地 Web API）获批；内部接口漂移即停 | ACP 行号与具体代码引用是当时版本；web-api 内部接口仍未形成稳定契约 | D/S（历史）；U（rc.5 API 稳定性） | 以 rc.5 审计 web-api 接口；把版本锁定和漂移停止条件写成阶段二门禁 |
| `docs/decisions.md` D-025 | 2026-10-21 | 线程日志带版本、旧版本只在内存补字段、坏记录 fail-closed、提交前未决确认卡终检 | 证据是当时 `cargo test` 175 passed；本次未复跑；与 DSH Runtime 版本无直接关系 | D；R（历史） | 保持产品侧约束；在 rc.5 接入后重跑相关宿主测试 |
| `tests/reports/p1-engine-adapter-prototype-report.md` | 2026-10-16（alpha.2 时期） | 最小 ACP 适配器、E01–E20 场景驱动、E18 model 臂阻塞的历史证据 | 20/20 pass 不能代表 rc.5；E17 的 sdk 错误字符串断言在 rc.5 已失效 | R（历史）；U（rc.5） | 复跑 E01–E20；保留报告作为 alpha.2 快照 |
| `tests/reports/p1-engine-interface-contract-report.md` | 2026-10-16 | 契约设计阶段的范围、静态检查与未实现声明 | 报告正文的「未实现 / 未执行」是第二步历史状态；不构成运行证据 | D（设计）；U（运行） | 只引用其设计范围；运行结论以复跑证据为准 |
| `tests/reports/p2-desktop-baseline-report.md` | 2026-10-16（alpha.2，commit d7432673） | 桌面切片起步时的版本、子模块和工具链基线 | 已被后续 Step 5–17 与 rc.5 升级取代；当前 gitlink 仍为 d7432673 但 lock 已是 b830ec95 | F；R（历史） | 作为历史基线保留；当前来源图以 DSH-P1-02 为准 |
| `tests/reports/p2-desktop-engine-slice-report.md` | 2026-10-16 | 桌面引擎技术切片的历史验收范围与证据组织 | 不是当前 0.2.22 安装包或 rc.5 行为证据 | R（历史）；U（当前） | 只作为历史切片记录；当前宿主另做运行验证 |
| `tests/reports/phase1-verification-report.md` | 2026-10-15（alpha.2） | Phase 1 只读副作用与材料写作端到端的历史验证方法 | 底座为 alpha.2；模型结果和环境均不可直接复用 | R（历史）；U（当前） | 需要时在隔离根重跑 Phase 1 验证；不把历史结果写成当前通过 |
| `tests/reports/v0.1-validation-report.md` | 2026-10-14（DSH v0.2.1-alpha.2） | v0.1 场景验证的范围与评测口径 | 评测环境为 Ubuntu / alpha.2；不是本机 rc.5 或 DSH Desktop 证据 | R（历史）；U（当前） | 保留为历史基准；当前版本不复用其结论 |
| `audit/dsh-java-review/*`（4 份） | 2026-10-21；dsh-java HEAD 3b9f729 | 任务链终检、日志版本化、Token 快照投影等设计借鉴点 | **非 DSH 实现证据**；dsh-java 是另一套 Java 延伸实现，不能证明 DSH rc.5 的行为 | D；U（DSH 行为） | 只作为设计参考；不得进入 DSH Runtime 源码证据链 |

## rc.5 升级后 E17/E20 回归（必须复跑）

当前 `REVIEW.md` 第 1366 行包含这段记录；git 历史提交 `d846af3a89`（2026-11-06）中的 `REVIEW.md` 也包含同一记录。以下引用以当前工作区原文为准，并用历史提交交叉核对：

- **E17/E20 在 DSH 0.2.1-rc.5（2026-11-02 升级）后回归失败，升级当日未发现。**
- E17 期望 sdk 通道对 `session/resume` 返回 `unknown method`；rc.5 实际先返回 `SDK server is not initialized`（initialize 缺 provider/model 时先行失败）。原断言对错误字符串精确匹配，范围过窄。
- E20 是同一轮复跑汇总的连带失败；不能据此单独推断隔离边界被破坏。
- 同一记录还列出 V02/V03/V05 受 11-02 底座升级影响：显式 `total_seconds` 截止不再生效，未知询问帧无法阻止任务被 engine 关闭为 interrupted。
- 该记录明确写明：10-23 的 E17 pass 证据对应 alpha.2 行为；11-02 底座升级时未重跑该场景。

**复跑要求：** E17/E20 必须在 rc.5 上重新执行并保留新证据；不得用 alpha.2 的 20/20 结果替代。E20 需要独立复跑，不能只看汇总行。

## 研究负责人复核修订

- **修订对象：** 本文件此前关于当前 `REVIEW.md` 是否包含 E17/E20 rc.5 回归记录的陈述。
- **复核证据：** `rg -n 'E17/E20|0\.2\.1-rc\.5' REVIEW.md` 返回当前文件第 1366 行；`git show HEAD:REVIEW.md` 同样返回第 1366 行；`git show d846af3a89:REVIEW.md` 也包含这段记录。
- **修订结论：** 当前 `REVIEW.md` 第 1366 行与 `d846af3a89` 历史提交均有 E17/E20 rc.5 回归记录。
- **影响范围：** 只修正引用事实，不改变已验证的版本结论、证据等级或复跑要求。

## 过期项汇总

- alpha.2 的源码行号、CLI 入口、ACP 默认路径和 `--version` 结果。
- 测试矩阵的 20/20 pass 与 p1 适配器报告：仅代表 alpha.2。
- D-016 的「桌面宿主默认 ACP Profile」：当前代码默认通道是 web-api，D-024 第 3 步已选择路径 B。
- 任何把 `dsh-java` 审计结论当作 DSH Runtime 实现的引用。
- 任何把文档自述、历史脚本结果或配置存在写成当前 rc.5 运行行为的引用。

## 复跑命令建议

以下命令仅为建议，本卡没有执行：

- `python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py`（E01–E20；先修 E17 断言）
- `python3 tests/verification/engine-interface/adapter/run_visibility_scenarios.py`（V01–V05）
- `python3 tests/verification/engine-interface/adapter/run_web_api_scenarios.py`（W01–W12）
- `cd tests/verification/engine-interface/adapter && python3 -m unittest test_web_api_unit`
- `bash scripts/omnidesk-isolation-check.sh`（隔离副作用与前后哈希检查）
- `git show d846af3a89:REVIEW.md`（复核本条历史回归记录的原文）

## 发现的反证与未验证项

- 基线写「`REVIEW.md` 已记录 rc.5 升级后 E17/E20 回归失败」，这一说法已核实：当前 `REVIEW.md` 第 1366 行与 `d846af3a89:REVIEW.md` 均有原文。此前 P1-05 把当前文件误判为不含该记录，原因是检索输出被截断；本补充卡已修订。
- 本次没有复跑任何 E/V/W 场景，也没有启动 OmniDesk 引擎；所有 rc.5 回归状态仍为 `U`，直到研究负责人安排复跑。
- 历史报告中的模型输出、真实窗口结果和跨版本会话结论均未在本卡复核。

## 边界

- 只读取文档、决策与 Git 元数据；未修改历史文档。
- 未读取会话正文、任务正文、凭证或 `.env`。
- 未启动引擎、未调用模型、未安装依赖、未初始化子模块。
