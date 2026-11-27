# 中央 Host 可靠运行机制设计（Step 13）

- 日期：2026-11-27
- 覆盖：开机启动、常驻、异常恢复、任务恢复、系统重启、健康检查、日志、备份、数据恢复、磁盘监控。

## 1. 进程可靠性

| 机制 | 设计 | 复用/新增 |
| --- | --- | --- |
| 开机启动 | launchd LoadPlist（RunAtLoad=true），用户级域（~/Library/LaunchAgents），不 root | 新增 plist + install 脚本 |
| 常驻 | KeepAlive=true（崩溃自动拉起）；ThrottleInterval=10s 防风暴 | 新增 |
| 崩溃恢复 | launchd 重启 Host Core；Host Core 复用 engine_supervisor 重启 Engine | 复用 supervisor |
| 单实例互斥 | process_owner claim_ownership（已有），Host Core 启动时抢锁，失败即退出等 launchd 下轮 | 复用 |
| 孤儿清理 | 沿用窗口收尾逻辑：重启时清扫孤儿 DSH node 进程 | 复用 D-111 逻辑 |
| 系统重启 | RunAtLoad 拉起；DSH 会话数据在磁盘，Engine 冷启动后 session/resume 可用 | 已有（Step 3） |
| Agent 任务恢复 | 启动时扫 engine-state 台账：running/starting 的 omni-*.json → 标 interrupted（终态）→ 按 Task Continuity 设计列"可续做清单" | 复用台账 + 新增扫描 |
| 健康检查 | /api/health：Host 进程、Engine 心跳（RPC ping）、DSH 可达性、数据根可写；launchd 侧加轻量 script 检测可选 | 新增 |
| 优雅停机 | SIGTERM → omni.shutdown(graceful) → 等任务终态 → 退出；launchd 默认 20s 强杀 | 复用 shutdown 契约 |

## 2. 日志

| 日志 | 落点 | 说明 |
| --- | --- | --- |
| Host Core 日志 | ~/OmniDesk/app/logs/host.log（轮转 10MB x 5） | 复用现有 logs 目录 |
| Engine 日志 | 已有 engine-state 日志/事件游标 | 不变 |
| DSH 日志 | DSH 自身 | 不变 |
| 审计 | events.jsonl（操作级） | 已有 |
| 访问日志 | API 层：时间、端点、令牌指纹（非明文）、状态码 | 新增，含打码规则（沿用 local_http.rs 的密钥打码原则） |

## 3. 备份与恢复

| 对象 | 备份方式 | 频率 | 恢复 |
| --- | --- | --- | --- |
| work/ + workspace/ + 库/ + reading/ | 文件级快照（rsync --link-dest 或 cp -c APFS clone）到 ~/OmniDesk/backups/<date>/ | 每日 + 手动 | 目录级回拷 |
| dsh-home（会话） | 同上（含 projcache 一致性：停机窗口内备份或接受 Engine 空闲时备份） | 每日 | 目录级回拷 |
| engine-state 台账 | 同上 | 每日 | 目录级回拷 |
| 凭证 | **不备份**（DSH 自管 + 用户另录） | — | 手动重录 |
| 备份保留 | 近 7 天日备 + 近 8 周周备 | 脚本 | — |

恢复演练：Phase 5 验收含一次"从日备恢复到临时数据根并启动"的实测。

## 4. 磁盘空间监控

- /api/health 附带数据根所在卷剩余空间。
- 低于阈值（建议 10GB）：Host 日志警告 + 事件表记录 + API/Web 显示。
- 备份脚本空间不足时跳过并告警（绝不删权威数据换空间）。
- workspace/staging/ 与缓存目录定期清理（复用 work_cleanup.rs 思路）。

## 5. 数据恢复分级

1. 单文件损坏：从当日备份单文件回拷（文件即权威的好处）。
2. 目录损坏：目录级回拷 + Host 重启重扫。
3. 全盘故障：新机装 Tailscale + Host Core + 从备份恢复数据根 + 重录凭证，预期恢复时间 < 1 小时（含备份拷贝时间）。

## 6. 验收口径（Phase 5）

- 杀 Host 进程 → launchd 10s 内拉起 → API 健康 OK → 中断任务出现在可续做清单。
- 系统重启 → 服务自启 → 会话可 resume。
- 备份恢复实测通过。
- 磁盘水位告警触发可见。
