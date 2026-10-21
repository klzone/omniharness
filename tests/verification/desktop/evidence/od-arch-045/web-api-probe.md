# OD-ARCH-045 第 5 步：DSH 自带 Web 应用本地 API 只读验证

- 日期：2026-10-21
- 范围：只读实验，零产品代码改动，未改动 runtime/
- 数据根：/tmp/od045-evidence/step5v1/{dsh-home,agents,work}（仓库外、一次性）
- 模型端点：本机回环合成端点 127.0.0.1:42191（tests/verification/engine-interface/adapter/_mock_model_server.py，固定文本、chunk 间 0.4s 延迟），未使用任何真实凭证或外网端点
- 实例：dsh --profile omnidesk-web --patch overlay.yml --port 3217 --no-open，profile 由锁定 runtime 的 web 默认模板派生

> 启动时打印的 ?token= 只存在于本次 shell 日志，未写入本目录任何证据文件。

## 结论一览

| 能力 | 结论 | 证据等级 |
| --- | --- | --- |
| 本地 HTTP API 存在（JSON RPC /api/ns/method） | 存在 | 脚本运行 |
| 启动 token 换 HttpOnly cookie | 存在（303 + Set-Cookie） | 脚本运行 |
| session/modelCatalog | 存在，返回默认 provider/model、routableProviders、分组与 reasoning efforts | 脚本运行 |
| session/create | 存在，返回 sessionId；实际参数是 { request: { cwd } } | 脚本运行 |
| session/prompt | 存在，返回 {accepted:true}；参数需包在 request 里 | 脚本运行 |
| token 级流式（assistant-stream 帧） | 存在，逐段到达，间隔与端点延迟相符 | 脚本运行 |
| 会话续接（同一 sessionId 连续两轮） | 存在，两轮都在同一会话内完成 | 脚本运行 |
| 历史读回（session/page） | 存在，需要 follow 快照给出的 cursor 作为 throughSeq | 脚本运行 |
| 取消（session/cancel） | 存在，turn/end 的 reason.kind = aborted，19ms 内落定 | 脚本运行 |
| 进程重启后会话仍在 | 存在，session/list 仍列出该会话，历史可读回，可继续 prompt | 脚本运行 |
| 思考块（reasoning delta）流式 | 未验证：合成端点不产出思考内容，模型目录里只有 reasoning.efforts 声明 | 文件事实 + 未验证 |
| 工具调用事件 | 未验证（本轮只发纯文本，未触发工具） | 未验证 |

## 关键细节

1. 网关参数校验按 descriptor 比对字段名：session/create、session/prompt 等必须写成
   {"args": {"request": {...}}}；多包一层数组会被拒（must contain exactly one plain-object args field）。
   session/list 的参数名是 _request。
2. session/page 的 throughSeq 不能猜。正确做法是先用 WebSocket session/follow 拿到 snapshot.cursor，
   再用它作为 throughSeq；用过期游标只会返回会话初始的三条策略事件。
3. 流式走 WebSocket /api/remote.mux，帧格式为
   {type:"open", streamId, endpoint:"session/follow", payload:{args:{request:{address:{kind:"session",sessionId}, assistantStream:true}}}}，
   回包是 item / end / error，item.value.type 为 snapshot、event 或 assistant-stream。
4. 助手正文在两部分都有：assistant-stream 的 chunk（block-start / text-delta / block-end / usage / finish），
   以及持久事件 assistant/message 的 data.message.content。

## 复现步骤

1. 起合成端点（端口 42191）：
   python3 tests/verification/engine-interface/adapter/_mock_model_server.py 的包装脚本 /tmp/od045-mockserve.py
2. 起隔离 web 实例（首次先 --from-default-profile web --dump-config 生成 profile）：

   DSH_HOME=/tmp/od045-evidence/step5v1/dsh-home
   DSH_AGENTS_HOME=/tmp/od045-evidence/step5v1/agents
   OMNI_FAKE_KEY=synthetic-placeholder-not-a-credential
   node runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js --profile omnidesk-web --patch /tmp/od045-evidence/step5v1/overlay.yml --port 3217 --no-open

3. 探测脚本（从启动日志取 token，不打印）：
   node /tmp/od045-probe6.mjs   两轮 + 流式 + page
   node /tmp/od045-probe12.mjs  取消
   node /tmp/od045-probe13.mjs create|resume  重启后恢复

## 证据文件

- web-api/turns.json：两轮时序、turn/end、流式 delta 文本、跟随游标
- web-api/stream-log3.json：逐帧时间戳与 chunk 类型（含 0.4s 间隔）
- web-api/durable-history.json：session/page 读回的 user / assistant / turn-end 事件
- web-api/cancel.json：取消回执与 turn/end 的 aborted 原因
- web-api/resume-proof.json：重启前后两轮用户消息均可在历史中读回
- web-api/overlay.yml：本次用的合成 provider 补丁（无凭证，只有 env 名）

## 局限

- 合成端点不产生思考内容，思考块流式与工具调用事件没有实测。
- 以上结论只描述 DSH 自带 Web 应用在本机隔离实例中的行为，不构成 OmniDesk 产品已接入该 API 的证据；本轮零产品代码改动。
