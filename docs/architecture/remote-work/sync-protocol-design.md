# 双向同步、离线缓存及冲突处理机制设计（Step 11）

- 日期：2026-11-27
- 问题：Central Authority、Local Cache、Sync Queue、Revision、乐观并发、幂等、Change Log、冲突处理、删除同步；区分中央实时数据与允许离线修改的数据。

## 1. 总体模型

```
中央 Host（权威）                      客户端（缓存）
┌────────────────────┐                ┌────────────────────┐
│ 文件域（work/等）    │◄── API 读写 ──►│ Local Cache（副本） │
│ revision: hash+seq │   etag/If-Match │ revision: 收到时快照 │
│ change log: events │                │ Sync Queue（离线写） │
└────────────────────┘                └────────────────────┘
```

- **Central Authority**：中央文件 + events.jsonl（change log 本体，已有）。
- **Local Cache**：只读镜像 + 少量可离线写文件（成果/知识新建）。
- **Sync Queue**：离线期间本地写操作队列（操作 + 目标 + 基线 revision），重连按序提交。

## 2. 版本与并发

| 机制 | 设计 |
| --- | --- |
| Revision | 文件级：SHA-256（已有 sha256.rs）+ updated_at；集合级：events.jsonl 的 seq（全局单调） |
| 乐观并发 | 读写请求带 If-Match: <hash>；服务端不匹配返回 409 + 当前版本；客户端三路合并或人工选择 |
| 幂等 | 写操作带 client_op_id（UUID）；服务端去重表（内存 + events 落盘 ref）防重复提交 |
| Change Log | events.jsonl 即 change log（追加、seq 单调、不存正文）；客户端按 seq 增量拉取失效列表 |
| 同步队列 | 仅"允许离线域"（成果新建、知识新建/编辑）产生队列项；中央实时域离线只读 |

## 3. 冲突处理规则

| 数据 | 冲突场景 | 处理 |
| --- | --- | --- |
| 任务/项目元数据字段（state、due 等） | 双端同时改 | 409；提示双方值，用户选（单用户场景，低频） |
| 任务追加段（## 进展/## 状态历史） | 双端同时追加 | 追加天然可合并（行级 append），服务端合并两条并记 actor |
| 成果/知识既有文件 | 双端改同文件 | 409 + 三方对比（中央/本地/基线快照），默认保留中央，本地另存副本 |
| 成果/知识新建文件 | 各自新建 | 文件名唯一化（时间戳后缀），无真冲突 |
| 删除 vs 编辑 | 一端删一端改 | 删除是软删（移目录）；编辑命中软删对象 → 409，用户裁决恢复或丢弃 |
| events.jsonl | 永远只有中央单写者 | 无冲突 |

## 4. 删除同步

- 业务实体删除 = 状态迁移（已取消）或移入 quarantine 目录，产生 task.deleted 类事件。
- 客户端缓存收到事件后移除缓存项。
- 物理清理只由中央定期维护（保留 quarantine，不自动销毁）。

## 5. 第三方同步工具评估（Syncthing 等）

| 问题 | 回答 |
| --- | --- |
| 为什么（可能）需要 | 仅当"成果 vault/知识库"需要**离线全量副本 + 后台自动同步**时 |
| 现有架构能否实现 | API 拉取已可覆盖在线场景；离线全量是增量需求 |
| 更简单替代 | 阶段 4 前用"API + 按需缓存"完全够用 |
| 额外维护成本 | 常驻进程 + 配置 + 与 API 写路径的双写冲突风险（Syncthing 不知道 API 的锁语义） |
| 实施阶段 | Phase 4 之后按需评估，仅限 workspace/库 两个纯文件目录，且排除 work/（含 events.jsonl 单写者约束） |
| 必要性 | 非必须。若离线编辑通过 Sync Queue 满足，则永远不需要 |

## 6. 明确禁止同步的数据

- ~/OmniDesk/runtime/dsh-home/（DSH 内部格式 + projcache）
- ~/OmniDesk/app/engine-state/（中央 Engine 台账）
- work/events.jsonl 作为文件对象（只能经 API 按 seq 读）
- 凭证、ui-state、logs

## 7. 结论

现有原子写 + 哈希校验 + 追加式事件表已提供 80% 基础设施；本设计新增的只有 If-Match 检查、client_op_id 去重和客户端 Sync Queue，全部是小改动。不需要 CRDT、不需要 Operational Transformation、不需要复杂数据库。
