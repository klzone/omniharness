# 旧 DSH 会话导入兼容性结论（OD-ARCH-017）

日期：2026-10-18　状态：**仅记录结论，未实现导入**

## 结论

锁定版 DSH（`runtime/dsh-runtime`，`@deepseek-ai/dsh` **0.2.1-alpha.2**）的公开 ACP 接口**没有**会话导入入口。因此 OmniDesk 不实现旧会话导入，也不复制、改写或自造 DSH 原生会话格式。

该结论只影响“把旧 DSH 会话搬入 OmniDesk 隔离数据根”的能力。**在同一个数据根内**发现并重开会话是支持的，已用合成数据实际验证（见 `tests/verification/desktop/verify_session_continuity.py` 与其证据）。

## 源码依据（文件事实）

已核对 `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-acp/lib/index.js` 的 ACP 方法注册（第 1362–1365 行）。除 initialize / authenticate 外，注册的会话方法只有：

| 方法 | 用途 | 是否可用于导入 |
| --- | --- | --- |
| `session/new` | 新建空会话 | 否，不能携带历史消息 |
| `session/list` | 分页列出会话元数据（`sessionId`、`cwd`、创建时间） | 否，只读元数据 |
| `session/resume` | 按 `sessionId` + `cwd` 在当前引擎数据根内恢复既有会话 | 否，仅限本数据根已有会话 |
| `session/close` | 关闭活动会话 | 否 |
| `session/setConfigOption` | 修改会话配置项 | 否 |
| `session/prompt` | 发起一轮对话 | 否；用它“重放”旧内容等于伪造历史 |
| `session/cancel`（通知） | 请求取消 | 否 |

另外核对了 `lib/types/index.d.ts`：ACP 插件配置只有 `provider`、`model`、`sessionListPageSize`、`stream`，没有任何导入路径、导入目录或历史注入配置。

在模块内检索 `session/import`、`importSession` 均无命中。

## 为什么不能自行复制

DSH 的持久化在 `<DSH_HOME>/sessions/<编码后的工作区>/<sessionId>/session.v4.jsonl.zstd`，并伴随 `storages/session_projcache/` 下的投影缓存。这是带版本号、压缩、且与投影缓存联动的内部格式。自行复制或改写会：

1. 依赖未承诺稳定的内部格式，升级即可能损坏；
2. 无法在不启动引擎的情况下证明目标可恢复；
3. 与“不修改 DSH 原生数据格式、不把内部实现当产品边界”的项目原则冲突。

计划已明确：若只能自行复制/改写发布格式、需要改 `runtime/`，或无法保证来源不变与目标可恢复，则**停止导入实现，只提交兼容性结论**。本轮据此停止。

## 已验证与被排除的边界

- **已实测（脚本，真实引擎 + 合成端点）：** 同一隔离数据根内，会话能被 `session/list` 发现、按工作区归组、按 `session/resume` 重开；未知会话返回 `unknown_session`；重开不新建会话、不重放提示词。
- **已实测：** `session/list` 前后，隔离 `DSH_HOME` 全部文件哈希不变（只读）。
- **已实测：** `session/resume` 会由 DSH 自身在其会话存储中追加恢复记录（`sessions/…/session.v4.jsonl.zstd` 与 `storages/session_projcache/…` 两个文件变化）。这是引擎的原生行为，**不是 OmniDesk 改写用户数据**；报告不得把它写成“恢复过程零写入”。
- **未验证：** 跨版本（旧版 DSH → 当前锁定版）会话的兼容性。没有官方导入入口，也没有做任何跨数据根复制实验。
- **未验证：** 真实用户旧会话。本轮未读取、未列出、未触碰 `~/.dsh` 或任何真实会话正文。

## 对产品的影响

桌面端不提供“导入旧会话”按钮，界面会说明原因。可在 OmniDesk 自己的隔离数据根内继续工作后，用“会话与任务”页重开既有会话。若未来 DSH 提供公开导入机制或稳定的迁移工具，应按新计划重新评估，并以官方机制为准。
