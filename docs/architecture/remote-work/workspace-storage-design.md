# 统一 Workspace 及文件访问机制设计（Step 12）

- 日期：2026-11-27
- 问题：中央 Workspace、项目文件、任务文件、知识库、附件、Agent 产物及本地缓存的统一管理。

## 1. 统一文件域规划（中央 Host）

| 文件域 | 路径 | 权威 | 访问 API |
| --- | --- | --- | --- |
| 业务域 | ~/OmniDesk/work/ | 任务、记录、项目、事件 | /api/work/*（读 + 受控写） |
| 成果域 | ~/OmniDesk/workspace/ | 成果文件（Obsidian vault） | /api/workspace/*（读 + 受控写 + Phase 4 离线新建） |
| 知识域 | ~/OmniDesk/库/ | 笔记与附件 | /api/kb/*（读 + 受控写 + Phase 4 离线编辑） |
| 阅读域 | ~/OmniDesk/reading/ | 阅读数据 | /api/reading/* |
| 产物域 | ~/OmniDesk/app/engine-state/results/ | Agent 产物 | /api/artifacts/*（只读） |
| 会话域 | ~/OmniDesk/runtime/dsh-home/ | DSH 会话 | 仅 Engine 内部 API，无文件 API |

统一约束（全部复用现有校验）：

- 根内绝对路径校验、拒绝符号链接与路径穿越（task_shell.rs::assert_local_root_allowed 已有）。
- 原子写 + 哈希校验（write_new_atomically / write_new_file_durable 已有）。
- 附件/二进制只列名 + 受限下载，不解析正文（Knowledge-Base-Plan 边界沿用）。

## 2. 本地缓存设计（客户端）

| 缓存项 | 内容 | 生命周期 |
| --- | --- | --- |
| 文件缓存 | 最近打开的任务/笔记/成果正文 | LRU + 显式固定（收藏） |
| 列表缓存 | 任务/项目/知识列表快照（带 seq） | 按 events seq 失效 |
| 产物缓存 | 只读产物副本 | 按需下载 |
| 离线写暂存 | Sync Queue（成果/知识新建） | 重连提交后清除 |

规则：

1. 缓存永不回写权威（除 Sync Queue 提交路径）。
2. 缓存键 = 相对路径 + revision hash；hash 不匹配即重新拉取。
3. 缓存可整体清除（设置项"清除本地缓存"），清除不影响任何权威数据。

## 3. 文件访问路径（安全）

```
Web/Desktop 客户端
  → GET /api/workspace/file?path=相对路径  (Bearer token)
  → Host Core 校验：域白名单 + 根内 + 非符号链接 + 大小上限
  → 读文件 + 返回内容 + ETag（hash）
  → 本地缓存 {path: {content, hash, seq}}
```

写路径（在线）：

```
PUT /api/workspace/file  If-Match: <旧hash>
  → Host 校验 + 哈希比对
  → 409（不匹配）或原子写 + events 追加 + 新 ETag
```

写路径（离线，Phase 4）：写本地缓存 + 进 Sync Queue；重连按队列序逐项提交，409 进入冲突裁决界面。

## 4. Agent 访问中央 Workspace

- Agent（Engine/DSH）本来就运行在中央 Host 上，对文件域是**本地文件访问**，路径不变、语义不变。
- 远程化后 Agent 的文件能力（bash/fs 工具）作用于中央文件，天然保证"Agent 看到的就是权威数据"。
- 客户端永远不替代 Agent 写文件（除队列通道），避免绕过事件表与门控。

## 5. 客户端访问不破坏执行环境

1. 客户端没有中央文件系统的直接挂载；一切经 API 的校验与原子写。
2. DSH 会话域无文件 API；客户端无法触碰 session 文件。
3. work/ 写 API 只暴露任务级操作（状态迁移、进展追加、新任务），暴露原始文件 PUT 的只有 workspace/ 与 库/（Phase 4），且带 If-Match。
4. 备份/清理由中央负责，客户端无权限。

## 6. 结论

统一 Workspace 的本质是"文件域白名单 + API 化访问 + 缓存与权威分离"，现有路径校验与原子写机制直接复用，新增代码集中在 API 层。
