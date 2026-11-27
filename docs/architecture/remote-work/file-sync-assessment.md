# Workspace、Knowledge Base 及文件同步架构评估（Step 5）

- 日期：2026-11-27
- 问题：项目文件、知识库、附件、Agent 产物、临时文件、索引、Git 与本地缓存的存储边界。

## 1. 存储边界盘点（static + 本机 ls 证实）

| 类别 | 路径 | 内容 | 可否远程共享 |
| --- | --- | --- | --- |
| 成果 Workspace | ~/OmniDesk/workspace/（含 artifacts/） | Markdown/HTML/XLSX 成果、写作导出 | 适合中央化，文件即权威 |
| 知识库 | ~/OmniDesk/库/（library_root 可配置） | Markdown 笔记、附件 | 适合中央化，文件即权威 |
| 业务数据 | ~/OmniDesk/work/ | 任务、工作记录、项目、复盘、events.jsonl、素材篮 | 适合中央化 |
| 阅读 | ~/OmniDesk/reading/ | 书目、进度、整理稿 | 适合中央化 |
| 引擎会话 | ~/OmniDesk/runtime/dsh-home/sessions/ | session.v4.jsonl.zstd + projcache | 只随中央 Engine，禁止客户端直接同步（内部格式 + projcache 联动） |
| 引擎任务台账 | ~/OmniDesk/app/engine-state/ | omni-*.json、results/、events 游标 | 随中央 Engine，客户端经 API 读 |
| 桌面 UI 状态 | ~/OmniDesk/app/ui-state/ | 窗口/输入偏好 | 各终端本地，不同步 |
| 桌面日志 | ~/OmniDesk/app/logs/ | 宿主日志 | 各终端本地 |
| 索引/派生数据 | vault_index/（内存索引为主，派生缓存可重建） | 扫描/搜索缓存 | 可派生，不权威同步 |
| 临时文件 | workspace/staging/ 等 | 半成品 | 不同步 |
| Git | 仓库 /Users/yvan/Code/MyWorkStation | 框架/技能/配置代码 | 项目级 Git 仓库本来就是版本化对象 |

## 2. 关键机制现状

- 成果写入（static，Data-Layout §5）：write_new_file_durable()，create_new 拒绝覆盖同名，frontmatter 注入 5 字段，Obsidian vault 兼容；旧文件字节级不动。
- 知识库（static，Knowledge-Base-Plan P1/P2）：根目录校验、拒绝符号链接/路径穿越、隐藏目录跳过、保存前哈希检查防覆盖外部修改、增量扫描按相对路径/大小/mtime/哈希。
- 文件选择授权（host_access.rs）：30 分钟 TTL 注册表，防止 webview 被注入时读未登记路径。
- Git：项目代码与知识/成果是两回事；成果 vault 当前不在 Git 内，靠文件本身 + 备份。

## 3. 跨设备文件同步的评估

| 数据 | 推荐方向 | 说明 |
| --- | --- | --- |
| work/ + workspace/ + 库/ + reading/ | 中央权威 + 客户端按需缓存（读走 API，写走 API） | 文件即事实；客户端缓存只是副本 |
| DSH 会话数据 | 不同步，只随中央 Engine | 内部格式 + projcache，双端写会损坏 |
| engine-state 台账 | 不同步，只随中央 Engine | 中央 Engine 是唯一写者 |
| ui-state / logs | 各终端本地 | 无同步价值 |
| 索引缓存 | 各终端本地派生 | 可删可重建 |
| 项目代码仓库 | Git push/pull（已有工作流） | 与 OmniDesk 数据根无关 |

## 4. 不适合普通文件同步工具的原因（针对会话/台账）

1. session.v4.jsonl.zstd 是压缩追加格式，projcache 与其联动；双端进程同时触碰会损坏恢复能力。
2. engine-state omni-*.json 是中央 Engine 的台账，客户端没有写语义。
3. events.jsonl 单写者假设。

这三类必须"API 访问 + 中央单写者"，而不是文件级双向同步。

## 5. 结论

- Workspace/知识库/业务数据：文件优先 + 中央权威 + API 访问，天然契合现有设计，无需迁移存储格式。
- 真正需要避免的是把 DSH 内部数据纳入文件同步；把"中央 Engine 在数据旁边跑"作为架构约束（Phase 1）即可消解。
- Syncthing 等第三方工具只在未来离线编辑场景对"成果 vault"这类纯文件域有候选价值（Step 11 评估），首期不需要。
