# T-001 复测清单：超级Plan / 超级劳工（2026-10-20）

> 这三项在 2026-10-19 部署时被**主动挂起**（用户指示「先别测试 agent，先部署好」），不是失败、也不是通过。
> 部署与静态校验已完成；缺的是**运行复测**。本清单把它们写成可直接照做的三步。

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
