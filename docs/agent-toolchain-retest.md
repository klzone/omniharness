# T-001 复测清单：超级Plan / 超级劳工（2026-10-20 晚最终版：①②③④全部闭环）

> 这三项在 2026-10-19 部署时被**主动挂起**（用户指示「先别测试 agent，先部署好」），不是失败、也不是通过。
> 部署与静态校验已完成；缺的是**运行复测**。本清单把它们写成可直接照做的三步。

## 复测结果（2026-10-20 晚最终版，限期解除后执行，subagent 真实调用）

| 复测项 | 结果 | 关键证据 |
|---|---|---|
| ① `git rev-parse*` 放行 | **通过** | 单条 `git rev-parse HEAD`（workdir=仓库根）成功输出 HEAD `653bf4a`。首次尝试因 agent 自报 workdir 笔误（`/mmnt/...`）被 `Permission denied: external_directory` 拒绝——该拒绝**证实工作区外路径防线生效**，补跑正确 workdir 后成功 |
| ② 管道/复合命令匹配语义 | **结论明确：管道被拒** | 单条 `git status --porcelain` 曾成功；`git status --porcelain \| wc -l`（workdir 无误）返回 `Permission denied: shell`。即 `git status*` 只按**单条命令**前缀匹配放行，管道命令整体不在白名单内。影响：超级Plan 的开工快照需**逐条**运行只读命令（`git status --porcelain` 本身可跑，不能接 `\| wc -l`）；这也是安全上合理的语义 |
| ③ `git diff --check HEAD` | **通过** | 成功，输出为空（当时 HEAD 无空白类差异） |
| ④ super-plan→super-worker 派工 | **通过（用户批准 B 方案后）** | 用户批准 B：新增项目级 `.opencode/opencode.json`（`experimental.subagent_depth: 2`，含 `$schema` 引用）。配置写入后**同一会话即时生效**：ARCHITECT → super-plan（深度1，GLM-5.3-Flash Free）→ super-worker（深度2，DeepSeek V4.1 Flash Free）派工成功，super-worker 只读执行 `git rev-parse HEAD` 并回报（当时 HEAD `581c964`，与 ARCHITECT 本会话独立核实一致） |

历史事实：B 方案之前，④ 曾被运行时默认深度 1 拦截（`Subagent depth limit reached (1)`；该键已在官方 schema 核实为 `experimental.subagent_depth`，整数，默认 1）。

## 新增证据（2026-10-20 凌晨）：阻塞已被定性为「模型额度」，不是 agent 发现

在 5 分钟间隔下连续重试 **99 次**（约 8.3 小时，2026-10-19 23:5x → 2026-10-20 07:2x，super-plan 与 super-worker 交替探测），
两个 agent **每次都返回 `Rate limit exceeded. Please try again later.`**。同一次重试窗口内，
**内置 `general` agent（继承本会话模型）调用成功**。据此：

- **阻塞点是两个绑定模型（`opencode/glm-5.3-flash-free`、`opencode/deepseek-v4.1-flash-free`）的免费额度**，
  不是 OpenCode 子 agent 通道故障，不是本项目配置缺陷，也不是 agent 目录路径写错。
- **「必须换新会话才能发现 agent」并非必要条件**：同一批 agent 文件在本会话创建后**立即被成功发现并加载**
  （super-plan 完成了只读调研与单条 `git status --porcelain`，super-worker 加载并自述模型为 DeepSeek V4.1 Flash Free）。
  与本文件 §0 记录的「`Unknown agent: super-plan`」差异，**原因未查证**（可能是不同会话的 agent 目录快照时机差异），
  在查清前不要把「换会话」当作已验证的解法。
- 复测仍需在**额度恢复后**执行；在此之前，本清单三项**保持挂起**（既非通过也非失败）。
- 附带事实：等待期间工作区被**外部提交并推送**（HEAD `5ffc710` → `653bf4a`，agent 文件进入提交 `593c49f`），
  本清单与两个 agent 文件均已在 `origin/main`；复测无需再动 Git 历史。

## 0. 为什么必须换一个新会话

两个 agent 文件放在 `.opencode/agents/super-plan.md` 与 `.opencode/agents/super-worker.md`
（OpenCode V2 文档确认的项目级 agent 目录就是 `.opencode/agents/`，**路径没写错**）。
但**当前这条会话**的 agent 目录是在 `super-plan.md` 创建（2026-10-19 22:47）**之前**建立的，
因此 subagent 工具里查不到这两个 agent：实际调用返回 `Unknown agent: super-plan`。
**新开一个会话**（同一仓库目录）即可发现它们——这也是用户接下来让 GPT 制定新计划的自然时机。

## 1. 三项复测（在**新会话**里做，逐条照抄提示词）

### 复测 ①：`git rev-parse*` 是否已放行

- 调用 agent `super-plan`（或让它自己调用），提示词：

  ```
  只读自检，不改任何文件：
  1) 运行 `git rev-parse HEAD` 并回报完整输出；
  2) 运行 `git status --porcelain | wc -l` 并回报完整输出。
  若某条命令被权限层拒绝或执行失败，原样回报错误信息，不要改写命令绕过。
  回报分三块：①git rev-parse 输出或报错；②git status --porcelain | wc -l 输出或报错；③你本次实际使用的模型标识。
  ```

- 预期：`git rev-parse HEAD` **成功**并打印 HEAD（首测因白名单缺 `git rev-parse*` 被拒，2026-10-19 已补）。

### 复测 ②：管道/复合命令的匹配语义

- 同一次调用里的第二条命令就是探针：`git status --porcelain | wc -l`。
- 两种可能结果都要**如实记录**：
  - **放行**（按整条命令文本匹配 `git status*` 前缀）→ 记「管道被当作 `git status*` 放行」；
  - **拒绝** → 记「权限匹配按完整命令文本，管道不在 `git status*` 内，故被拒」。
- 关键：**不得为了让命令跑通而改写命令**（例如拆成两步或换成别的过滤器），否则测的就不是匹配语义。

### 复测 ③：super-plan → super-worker 的派工链路

- 给 `super-plan` 一条**只读**的最小任务，并要求它用 subagent 调 `super-worker` 完成：

  ```
  请用 subagent 调用「超级劳工」（super-worker）完成一件只读小事：
  读 docs/decisions.md，回报其中 D-0xx 决策条目的编号与标题清单。
  不要修改任何文件。
  ```

- 预期：super-plan 能**成功调起** super-worker，super-worker 能读到文件并回报清单；
  回报里应能看到 super-worker 的模型标识（`deepseek-v4.1-flash-free`）。
- 若 super-plan 拒绝派工或找不到 super-worker，**原样记录报错**（这正是要测的匹配语义）。

## 2. 记录方式

复测结果请按本仓库纪律落盘：

- `STATUS.md` 顶部新增小节「T-001 复测（日期、执行方）」，写明**实际运行了什么、观察到什么**；
- `REVIEW.md` 顶部新增对应的 **ARCHITECT 复核**小节（结论、通过/未通过、局限）；
- `PLANS.md` 里 T-001 的状态行改为「复测完成」并指向上面两处。

## 3. 边界（复测也不放松）

- 只做**只读**命令与一次只读派工；不写 PLANS.md 以外的文件，不改产品代码。
- 不提交、不推送、不打标签；`runtime/`、模型路由、凭证、真实材料与用户数据一律不碰。
- 权限规则是**配置级**防线，不等于代码级安全控制；复测通过也不改变这一点。
