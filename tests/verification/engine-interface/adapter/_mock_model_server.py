#!/usr/bin/env python3
"""
OmniDesk P1 第三步 —— 本机回环合成模型端点（纯标准库）。

用途：为 ACP 适配器的端到端场景（E01-E19）提供一个**不使用任何真实凭证、不访问外网**
      的 OpenAI 兼容端点，使验证可确定性复现。

安全边界：
  * 只监听 127.0.0.1 的随机端口。
  * 不写任何用户目录；不读取、不记录任何凭证。
  * 只回答固定合成文本，不含真实人员、机构、案件或内部信息。
  * 记录的请求信息仅用于证明"引擎确实发起了模型调用"。

行为模式（运行时由控制对象切换，供不同场景复用）：
  ok                 -> 流式返回固定文本，finish_reason=stop
  slow               -> 分块返回，块间有延迟（用于观察流式 seq 与取消时序）
  hang               -> 不返回任何响应，保持连接（用于取消 / 关闭超时）
  length             -> 返回文本但 finish_reason=length（用于 max_tokens 截断）
  error_400          -> 返回 HTTP 400 错误体（用于执行失败）
  partial_then_error -> 先发一块文本，再发一个流内错误事件（用于部分结果 + 断流）
  tool_call          -> 首次请求返回一个 bash 工具调用（带 sandbox_permissions 升级，
                        用于触发 DSH 审批）；请求里已含 tool 角色消息时返回普通文本
"""

from __future__ import annotations

import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class MockController:
    """运行时控制合成端点的行为，并记录收到的请求（不含凭证）。"""

    def __init__(self) -> None:
        self.mode = "ok"
        self.chunks = ["合成", "回复", "内容"]
        self.delay = 0.0
        self.hang_seconds = 120.0
        self.requests: list = []
        # 标记探针：记录请求文本中是否出现给定标记串，用于证明某配置
        # （例如 personaPrefix）确实被送到模型请求里，而不是只写在配置文件。
        # 只记录命中与否，不保存请求正文。
        self.probes: list = []
        self.lock = threading.Lock()
        # tool_call 模式下发给模型的工具调用参数（纯合成，不含真实路径/命令）。
        self.tool_call_id = "call-synth-1"
        self.tool_call_name = "bash"
        self.tool_call_arguments = {
            "command": "echo synthetic-approval-probe",
            "description": "synthetic approval probe",
            "sandbox_permissions": "danger-full-access",
            "justification": "synthetic approval probe, no real data",
        }
        self.tool_call_followup = ["合成", "已", "继续"]
        # 空 = 仅首个请求触发；设置某标记后，最后一条用户消息含它即触发。
        self.tool_call_trigger = ""

    def set(self, **kwargs) -> None:
        with self.lock:
            for key, value in kwargs.items():
                if not hasattr(self, key):
                    raise AttributeError("unknown mock setting: " + str(key))
                setattr(self, key, value)

    def reset_requests(self) -> None:
        with self.lock:
            self.requests = []

    def new_requests(self) -> list:
        with self.lock:
            return list(self.requests)


CTL = MockController()


def _last_user_text(req: dict, limit: int | None = None) -> str:
    # limit 只用于记录日志；判定触发词必须看全文——OD-ARCH-089 之后场景提示词
    # 把规则写在前面、用户原话落在消息末尾，按 200 字符截断会漏掉本轮指令。
    messages = req.get("messages") or []
    for msg in reversed(messages):
        if isinstance(msg, dict) and msg.get("role") == "user":
            content = msg.get("content")
            if isinstance(content, str):
                return content if limit is None else content[:limit]
            if isinstance(content, list):
                parts = [c.get("text", "") for c in content if isinstance(c, dict)]
                joined = "".join(parts)
                return joined if limit is None else joined[:limit]
    return ""


def _all_text(req: dict) -> str:
    parts = []
    for msg in req.get("messages") or []:
        if not isinstance(msg, dict):
            continue
        content = msg.get("content")
        if isinstance(content, str):
            parts.append(content)
        elif isinstance(content, list):
            for item in content:
                if isinstance(item, dict) and isinstance(item.get("text"), str):
                    parts.append(item["text"])
    return "\n".join(parts)


class _Handler(BaseHTTPRequestHandler):
    server_version = "OmniSyntheticModel/0.1"
    protocol_version = "HTTP/1.0"

    @property
    def ctl(self) -> "MockController":
        # 每个 server 实例持有自己的控制对象，避免不同场景之间互相污染。
        return self.server.ctl  # type: ignore[attr-defined]

    def log_message(self, *args) -> None:  # 静默
        pass

    def _json(self, code: int, obj: dict) -> None:
        data = json.dumps(obj, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except Exception:
            pass

    def _sse(self, obj: dict) -> None:
        payload = ("data: " + json.dumps(obj, ensure_ascii=False) + "\n\n").encode("utf-8")
        self.wfile.write(payload)
        self.wfile.flush()

    def do_GET(self) -> None:
        if self.path.rstrip("/").endswith("models"):
            self._json(200, {"object": "list", "data": [{"id": "synthetic-model", "object": "model"}]})
        else:
            self._json(404, {"error": {"message": "not found"}})

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b""
        try:
            req = json.loads(raw.decode("utf-8", "replace") or "{}")
        except Exception:
            req = {}
        if not isinstance(req, dict):
            req = {}
        ctl = self.ctl
        raw_text = raw.decode("utf-8", "replace")
        with ctl.lock:
            mode = ctl.mode
            chunks = list(ctl.chunks)
            delay = ctl.delay
            hang_seconds = ctl.hang_seconds
            probes = list(ctl.probes)
            tool_call_id = ctl.tool_call_id
            tool_call_name = ctl.tool_call_name
            tool_call_arguments = json.dumps(ctl.tool_call_arguments, ensure_ascii=False)
            tool_call_followup = list(ctl.tool_call_followup)
            tool_call_trigger = ctl.tool_call_trigger
            joined = _all_text(req)
            ctl.requests.append({
                "path": self.path,
                "model": req.get("model"),
                "stream": bool(req.get("stream")),
                "message_count": len(req.get("messages") or []),
                # 模型侧实际收到的文本长度（字符数）。用于对比「每轮重放」与「续接会话」
                # 两种前端的输入量，不保存正文。
                "text_chars": len(joined),
                "last_user": _last_user_text(req, limit=200),
                "mode": mode,
                "probe_hits": [p for p in probes if p and p in joined],
                "probe_misses": [p for p in probes if p and p not in joined],
                # 整包探针：用于区分“完全没发送”与“走了 messages 之外的字段”。
                # 只记录命中与否，不保存请求正文。
                "body_probe_hits": [p for p in probes if p and p in raw_text],
                "body_keys": sorted(str(k) for k in req.keys()),
                "has_tool_message": any(
                    isinstance(m, dict) and m.get("role") == "tool"
                    for m in (req.get("messages") or [])),
            })

        if mode == "error_400":
            self._json(400, {"error": {"message": "synthetic upstream failure",
                                       "type": "invalid_request_error"}})
            return
        if mode == "hang":
            time.sleep(float(hang_seconds))
            return

        # 默认只在首个请求（还没有工具结果）时发工具调用。验收脚本可显式设置
        # `tool_call_trigger`：只要最后一条用户消息含该标记，即使会话里已有工具
        # 结果也再发一次提问——用于同一会话里的第二轮合成提问。标记只用于测试。
        if mode == "tool_call" and (
                tool_call_trigger and tool_call_trigger in _last_user_text(req)
                or not any(isinstance(m, dict) and m.get("role") == "tool"
                           for m in (req.get("messages") or []))):
            model = req.get("model") or "synthetic-model"
            try:
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.end_headers()
                first = {"id": "cmpl-synthetic", "object": "chat.completion.chunk",
                         "created": 0, "model": model,
                         "choices": [{"index": 0, "delta": {"tool_calls": [{
                             "index": 0, "id": tool_call_id, "type": "function",
                             "function": {"name": tool_call_name,
                                            "arguments": tool_call_arguments}}]},
                                       "finish_reason": None}]}
                self._sse(first)
                final = {"id": "cmpl-synthetic", "object": "chat.completion.chunk",
                         "created": 0, "model": model,
                         "choices": [{"index": 0, "delta": {},
                                       "finish_reason": "tool_calls"}]}
                self._sse(final)
                self.wfile.write(b"data: [DONE]\n\n")
                self.wfile.flush()
            except Exception:
                self.close_connection = True
            return

        if mode == "tool_call":
            chunks = tool_call_followup

        model = req.get("model") or "synthetic-model"
        try:
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            for index, piece in enumerate(chunks):
                chunk = {"id": "cmpl-synthetic", "object": "chat.completion.chunk", "created": 0,
                         "model": model,
                         "choices": [{"index": 0, "delta": {"content": piece}, "finish_reason": None}]}
                self._sse(chunk)
                if mode == "slow" and delay:
                    time.sleep(float(delay))
                if mode == "partial_then_error" and index == 0:
                    self.wfile.write(b"data: {\"error\": {\"message\": \"synthetic mid-stream failure\"}}\n\n")
                    self.wfile.flush()
                    self.close_connection = True
                    return
            finish = "length" if mode == "length" else "stop"
            final = {"id": "cmpl-synthetic", "object": "chat.completion.chunk", "created": 0,
                     "model": model,
                     "choices": [{"index": 0, "delta": {}, "finish_reason": finish}]}
            self._sse(final)
            self.wfile.write(b"data: [DONE]\n\n")
            self.wfile.flush()
        except Exception:
            self.close_connection = True


def start_mock():
    """启动合成端点，返回 (base_url, controller, server)。base_url 含 /v1 后缀。"""
    server = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    server.daemon_threads = True
    server.ctl = MockController()  # type: ignore[attr-defined]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    port = server.server_address[1]
    base_url = "http://127.0.0.1:" + str(port) + "/v1"
    return base_url, server.ctl, server  # type: ignore[attr-defined]
