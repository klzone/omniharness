# OD-ARCH-017 证据索引

日期：2026-10-18　执行：BUILDER　状态：**待 ARCHITECT 独立复核**

本目录只放可复核的结果与计数，不含真实会话正文、真实路径清单或凭证。

## 证据清单

| 文件 | 内容 | 证据等级 |
| --- | --- | --- |
| `engine-cancel-model.json` | 取消按钮 23 个状态分支 + 8 项静态边界检查 | 编译真实源码后 Node 执行（无引擎） |
| `session-continuity.json` | 会话列出、重开、未知会话、任务恢复、台账元数据、损坏记录、中断任务等 32 项检查 | 脚本：真实 DSH 引擎进程 + 本机回环合成模型端点 |
| `frontend-build.log` | `npm run build` 输出 | 构建实测 |
| `rust-cargo-test.log` | `cargo test --offline` 输出 | 编译 + 单元/集成测试 |
| `engine-adapter-rerun.log` | 引擎适配器 E01–E20 全场景回归输出 | 脚本：真实 DSH 引擎进程（合成根） |

## 复现命令

```bash
cd /mnt/data/Code/MyWorkStation
python3 tests/verification/desktop/verify_engine_cancel.py \
  --json tests/verification/desktop/evidence/od-arch-017/engine-cancel-model.json
python3 tests/verification/desktop/verify_session_continuity.py \
  --json tests/verification/desktop/evidence/od-arch-017/session-continuity.json
cd desktop && npx tsc --noEmit && npm run build
cd src-tauri && cargo test --offline
python3 tests/verification/engine-interface/adapter/run_engine_adapter_scenarios.py
```

## 合成验证的安全边界

- 每个场景使用 `/tmp` 下一次性数据根（`DSH_HOME`、`DSH_AGENTS_HOME`、工作区、状态目录都在其中），结束后删除。
- 模型端点为 `127.0.0.1` 随机端口的合成服务，凭证是占位字符串，不发起外网请求。
- 未读取、未列出、未修改 `~/.dsh`、`~/.agents`、`~/OmniDesk` 或任何真实会话；未读取任何真实工作材料。
- 合成素材为虚构占位文本，不含真实人员、机构、案件或内部信息。

## 已证实的结论

1. `session/list` 只返回会话元数据（`sessionId`、工作区、是否存在、结构有效、恢复状态未验证），不含正文。
2. 列表读取前后，一次性 `DSH_HOME` 全部文件哈希完全一致。
3. 重启适配器后，原会话仍能被列出并按工作区归组；重开返回同一会话，不新建会话、不重放提示词。
4. 未知会话返回标准错误码 `unknown_session`。
5. 重启后已完成任务恢复为 `succeeded`，`replayed=false`；无终态证据的合成任务落到 `unknown/interrupted`，不自动重放。
6. 任务台账只返回元数据；损坏记录计入 `skipped_invalid`，不猜造状态；写入台账的合成正文哨兵不会出现在输出中。
7. 重开会话会让 DSH 在**它自己的**会话存储里追加恢复记录（`sessions/…` 与 `storages/session_projcache/…` 两个文件变化），其它文件不变。这是引擎原生行为，已如实记录，不能描述成“恢复零写入”。

## 未验证事项

- 真实窗口点击（本轮未启动 Tauri 窗口；`npm run build` 与类型检查通过）。
- 真实用户会话、真实旧版 DSH 会话、跨版本导入（无公开导入接口，未做任何复制实验）。
- 真实模型端点上的会话与任务行为（端点准入仍关闭）。
- 用户在真实数据根内的合法手工修改是否会被同步覆盖（本轮未涉及 Skill 同步）。
