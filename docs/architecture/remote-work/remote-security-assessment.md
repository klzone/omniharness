# 远程访问、安全及网络架构评估（Step 6）

- 日期：2026-11-27
- 问题：API、端口、协议、认证、授权、文件访问、命令执行权限、网络暴露、多终端安全风险。

## 1. 现状安全资产（static）

| 机制 | 位置 | 说明 |
| --- | --- | --- |
| DSH web 本地实例 | Engine 拉起 dsh --profile web --port 0，回环随机端口 + token 交换 cookie | token/cookie 只留内存；LOOPBACK_HOSTS 白名单（web_api_adapter.py:704） |
| net_guard.rs | 外联 URL 校验：只放行 http/https、拒绝凭据内嵌、解析后地址白名单（公网/私网分档）、防 DNS 重绑定、禁回环/链路本地 | D-108 |
| host_access.rs | 文件选择注册表 30 分钟 TTL，前端拿不到未登记路径 | D-HARDEN-2043 |
| action_gate.rs / admission.rs / capabilities.rs | 操作门控、准入、能力声明 | static |
| app_paths.rs assert_isolated | 数据根必须隔离于 ~/.dsh、~/.agents、仓库等 | static |
| local_http.rs | 仅 127.0.0.1 明文回环客户端（用于 credentials RPC），明确不代理不重定向 | static |
| 凭证 | DSH 自管 ~/.dsh/.credentials.yaml，Engine 不读取不传输 | static |

## 2. 远程化后的暴露面（目标态假设：中央 Host 暴露 API）

| 暴露面 | 风险 | 必要控制 |
| --- | --- | --- |
| HTTP API（任务/会话/文件读写） | 未授权访问 = 完整个人数据 + Agent 执行权 | 网络层先限定私有网络（Tailscale/VPN），API 层加认证令牌 |
| WebSocket/SSE（事件流） | 同 API | 同上，且复用 Engine 已有 seq 去重 |
| 文件读写 API | 路径穿越、符号链接越界 | 复用 host_access/assert_local_root_allowed 的根内校验；远程白名单=中央工作区根集合 |
| 命令执行（Agent bash 工具） | 远程触发任意命令 | 保持"Agent 工具面由 Profile rules.json 决定"；远程 API 不新增 shell 透传命令 |
| 凭证 | 绝不进 API/日志 | 现有 DSH 自管机制不变 |

## 3. 网络暴露现状结论

- 当前一切监听都是回环随机端口，无任何公网暴露（static）。
- 远程化**不等于**把 OmniDesk 直接暴露到公网；推荐架构是 Tailscale 私网 + 回环之上的认证代理（Step 8 比选）。

## 4. 多终端安全风险清单

1. **设备失窃**：终端缓存含业务文件。控制：缓存加密（OS 文件保险箱）+ 可远程吊销令牌。
2. **令牌泄露**：长效 API token 泄露 = 等同本机访问。控制：短效 token + 刷新，操作审计落 events 表。
3. **中间人**：私网内明文。Tailscale WireGuard 已加密；若走 SSH 隧道亦加密。禁止裸 TCP。
4. **Agent 误操作扩散**：远程提交的任务在中央无人在场执行。控制：沿用现有准入/门控（action_gate/admission），高风险动作仍需人工确认（客户端可远程确认）。
5. **webview 注入面**：Web UI 化后攻击面从 Tauri IPC 变成 HTTP；沿用 host_access 白名单思想（服务端会话绑定已授权路径集合）。
6. **多用户/多租户**：明确不做；单用户单令牌即可，不引入账户体系。

## 5. 结论

- 现有安全资产（隔离校验、回环绑定、net_guard、路径授权、凭证自管）全部可复用，是良好的远程化基础。
- 需要新增的只有：私网组网（Tailscale 首选）、API 认证令牌、审计接入既有事件表。
- 命令执行面不因远程化扩大：远程 API 只暴露任务级操作，shell 能力仍由 Agent 工具面按 Profile 决定。
