# 阶段二精确源码基线

- 建立时间：2026-11-12
- 用户决定：选择方案 1（在 `/tmp` 获取 rc.5 精确源码并只读审计）
- 状态：**G2 源码门禁已满足**

## 1. 精确来源

| 项 | 值 |
| --- | --- |
| 官方仓库 | `https://github.com/deepseek-ai/deepseek-harness.git` |
| tag | `dsh-v0.2.1-rc.5` |
| commit | `b830ec950faba0a46dda564be3e2d854afed94f1` |
| 本次检出路径 | `/tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo` |
| 检出深度 | `--depth 1 --filter=blob:none --no-checkout`，随后 detach checkout |
| 工作树状态 | `## HEAD (no branch)`，无已跟踪改动 |
| 文件数 | 16,353（`git ls-files`） |
| 目录大小 | 约 207 MB |

获取命令的实质部分：

```bash
git clone --depth 1 --branch dsh-v0.2.1-rc.5 \
  --filter=blob:none --no-checkout \
  https://github.com/deepseek-ai/deepseek-harness.git \
  /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo
git -C /tmp/omnidesk-dsh-rc5-b830ec95.O48O03/repo \
  checkout --detach b830ec950faba0a46dda564be3e2d854afed94f1
```

## 2. 校验结果

- `git rev-parse HEAD`：`b830ec950faba0a46dda564be3e2d854afed94f1`
- `git describe --tags --exact-match HEAD`：`dsh-v0.2.1-rc.5`
- 没有安装依赖、没有运行 build、没有运行测试。
- 源码位于 `/tmp`，不进入仓库；它不是产品路径 `runtime/deepseek-harness` 的替代检出。

## 3. 六个审计对象的源码位置

| 机制 | 主要源码 |
| --- | --- |
| System Prompt Assembly | `packages/core/system-prompt/src/index.ts`；`packages/core/agent-loop/src/agent.ts`；`packages/llm/llm-pi-ai/src/context.ts` |
| Agent Loop | `packages/core/agent-loop/src/{agent.ts,assistant-stream.ts,index.ts,inbox.ts,runtime-context.ts,tool-calls.ts,constants.ts}` |
| Session Replay / Projection | `packages/core/session/src/`；`packages/session/session-projection/src/`；`packages/session/session-persistence-jsonl/src/`；`packages/compaction/compaction-basic/src/` |
| Tool Description / Schema | `packages/core/tools/src/{index.ts,schema.ts,types.ts,json-schema.ts,ts-types.ts,py-types.ts,ptc.ts,ptc-calls.ts}`；`packages/core/system-prompt/src/index.ts`；`packages/llm/llm-pi-ai/src/context.ts` |
| Parallel Tool Scheduler | `packages/core/agent-loop/src/{tool-calls.ts,constants.ts,agent.ts,index.ts}`；`packages/core/tools/src/index.ts`；相关 tests |
| Subagent Lifecycle | `packages/subagent/{tool-subagent,subagent-spawn-in-process,subagent-fork-in-process}/src/`；`packages/core/agent-loop/src/agent.ts`；`packages/core/session/src/fork.ts` |

所有包版本在源码树中均为 `0.2.1-rc.5`，与运行锁定版本一致。审计报告仍须以源码行为为准，不以 README 描述代替。

## 4. 边界与限制

- `/tmp` 源码可能在后续会话或系统清理后消失；阶段二报告必须固定 commit、文件路径、行号和关键代码片段。
- 本次没有安装 TypeScript 依赖，因此不得声称跑过源码测试；静态审计与测试代码阅读是允许的，实际测试运行需要另行授权。
- 不把 `/tmp` 源码复制进仓库，也不修改源码树。
- 不读取 `.env`、凭证、真实会话或用户数据。

## 5. 门禁状态

- G2 精确源码门禁：**已满足**。
- 阶段二可以开始六个机制的单动作审计。
- 阶段三仍需单独的安装与真实模型授权。
