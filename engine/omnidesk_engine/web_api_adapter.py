"""OmniDesk Web API 引擎适配器（OD-ARCH-048，可替换通道）。

与 ACP 适配器共享 task_core 的任务/会话语义，只把 DSH 的本地 Web API
（HTTP RPC + WebSocket 事件流）翻译成同一组归一化调用。

已由探针证实（script，合成端点、隔离数据根）：
  * 启动：`node bin.js --profile web --port 0 --no-open`，stdout 出现
    `dsh web: http://127.0.0.1:<port>/?token=...`；`web` profile 自动生成。
  * 鉴权：GET `/?token=...`（不跟随重定向）返回 303 + `Set-Cookie: dsh_auth=...`。
    token 与 cookie **只留在内存**，不写日志、不进台账、不进错误 detail。
  * RPC：`POST /api/<method>`，body
    `{"type":"client-request","rpcId":...,"method":...,"payload":{"args":{...}}}`，
    响应 `{"result":{"ok":true,"value":...}}` 或 `{"result":{"ok":false,"error":...}}`。
  * `session/follow` 只能走 WebSocket `/api/remote.mux`；首帧 snapshot，
    之后是 event / assistant-stream 帧。
  * assistant-stream：`start` → `chunk{block-start|text-delta|block-end|usage|finish}`
    → `end{outcome:{kind:"committed"}}`；text-delta 是真正的增量文本。
  * `turn/end.data.reason.kind` ∈ completed | aborted | blocked | failed。

已知限制（如实记录，不伪造）：
  * Web API 是 DSH 内部接口；调用形态随 runtime 版本变化。行为与探针不符时
    适配层报 `event_parse_error` / `unsupported`，不猜测。
  * 合成端点不产生 reasoning 内容，思考块流式未验证。
  * `system/message` 含完整系统提示词，**必须丢弃**，绝不进入结果或事件。

证据等级：static / script / model / unverified。
"""

from __future__ import annotations

import base64
import json
import os
import queue
import socket
import struct
import subprocess
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping, Optional, Sequence

from .contract import (
    TERMINAL_STATES,
    CapabilityState,
    CancelPhase,
    EngineCapability,
    EngineDescriptor,
    EngineError,
    EngineErrorCode,
    EventType,
    TaskState,
    new_omni_task_id,
    now,
)
from .task_core import (
    CancelOutcome,
    SessionFork,
    SessionSummary,
    SessionListing,
    SessionRecovery,
    ShutdownReport,
    SubmitTaskReceipt,
    SubmitTaskRequest,
    TaskCoreBase,
    TaskRecovery,
    _TaskRecord,
)
from .acp_adapter import canonical_cwd, workspace_label_for, workspace_token_for
from .process_owner import record_child_group, terminate_group

__all__ = [
    "WEB_API_PROTOCOL_VERSION",
    "WebApiAdapter",
    "WsClient",
]

WEB_API_PROTOCOL_VERSION = "dsh-web-api/1"

# 适配层声明（4.1）。只有脚本/真实窗口跑过的才算 verified，见测试报告。
ADAPTER_CAPABILITIES: Mapping[EngineCapability, CapabilityState] = {
    EngineCapability.SUBMIT_TASK: CapabilityState.IMPLEMENTATION_SUPPORTED,
    EngineCapability.GET_TASK_STATUS: CapabilityState.IMPLEMENTATION_SUPPORTED,
    EngineCapability.STREAM_EVENTS: CapabilityState.IMPLEMENTATION_SUPPORTED,
    EngineCapability.CANCEL_TASK: CapabilityState.IMPLEMENTATION_SUPPORTED,
    EngineCapability.RECOVER_SESSION: CapabilityState.IMPLEMENTATION_SUPPORTED,
    # 与 ACP 同理：能重开会话，不等于能重建未落盘那一轮的结论。
    EngineCapability.RECOVER_TASK: CapabilityState.PARTIAL,
    EngineCapability.GET_TASK_RESULT: CapabilityState.IMPLEMENTATION_SUPPORTED,
}


class WsClient:
    """最小 WebSocket 文本帧客户端（纯标准库）。

    只实现本适配器需要的能力：文本帧收发、分片重组、ping/pong、close。
    不做扩展协商、不做压缩。任何帧解析异常都向上抛，由适配层转成标准错误。
    """

    def __init__(self, url: str, cookie: str, timeout: float = 30.0) -> None:
        if not url.startswith("ws://"):
            raise ValueError("only ws:// is supported")
        rest = url[len("ws://"):]
        hostport, _, path = rest.partition("/")
        host, _, port_text = hostport.partition(":")
        port = int(port_text or "80")
        self.sock = socket.create_connection((host, port), timeout=timeout)
        key = base64.b64encode(os.urandom(16)).decode("ascii")
        request = (
            "GET /"
            + path
            + " HTTP/1.1\r\nHost: "
            + hostport
            + "\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: "
            + key
            + "\r\nSec-WebSocket-Version: 13\r\nCookie: "
            + cookie
            + "\r\n\r\n"
        )
        self.sock.sendall(request.encode("utf-8"))
        buffer = b""
        while b"\r\n\r\n" not in buffer:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise OSError("websocket handshake closed before headers")
            buffer += chunk
        head, _, remainder = buffer.partition(b"\r\n\r\n")
        status_line = head.split(b"\r\n")[0].decode("latin-1", "replace")
        if " 101 " not in status_line:
            self.sock.close()
            raise OSError("websocket handshake failed: " + status_line)
        self._buf = bytearray(remainder)
        self._fragments = bytearray()
        self.closed = False

    def send(self, obj: Mapping[str, Any]) -> None:
        data = json.dumps(obj).encode("utf-8")
        header = bytearray([0x81])
        mask = os.urandom(4)
        n = len(data)
        if n < 126:
            header.append(0x80 | n)
        elif n < 65536:
            header.append(0x80 | 126)
            header += struct.pack("!H", n)
        else:
            header.append(0x80 | 127)
            header += struct.pack("!Q", n)
        header += mask
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(data))
        self.sock.sendall(bytes(header) + masked)

    def _read_exact(self, n: int) -> bytes:
        while len(self._buf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise OSError("websocket closed by peer")
            self._buf += chunk
        out = bytes(self._buf[:n])
        del self._buf[:n]
        return out

    def recv(self, timeout: Optional[float] = None) -> Optional[Mapping[str, Any]]:
        """返回一个 JSON 文本帧；对端关闭时返回 None。"""
        if self.closed:
            return None
        # OD-ARCH-069 修复：timeout 必须是「整次 recv 的总预算」，而不是
        # 每一小段读出（含心跳/pong 处理）各自的预算。DSH 每 2 秒发一次
        # ping，若按段计时，心跳会把等待无限续期，调用方的总超时永远到不了。
        deadline = None if timeout is None else time.monotonic() + max(0.01, float(timeout))

        def _arm() -> None:
            if deadline is None:
                # 无等待预算时必须显式切回阻塞模式：套接字握手时带着
                # create_connection 的 30 秒超时，不重置的话长任务会被误判断流。
                self.sock.settimeout(None)
                return
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("websocket recv budget exhausted")
            self.sock.settimeout(max(0.01, remaining))

        _arm()
        while True:
            first, second = self._read_exact(2)
            fin = bool(first & 0x80)
            opcode = first & 0x0F
            masked = bool(second & 0x80)
            length = second & 0x7F
            if length == 126:
                length = struct.unpack("!H", self._read_exact(2))[0]
            elif length == 127:
                length = struct.unpack("!Q", self._read_exact(8))[0]
            mask = self._read_exact(4) if masked else None
            payload = self._read_exact(length)
            if mask:
                payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
            if opcode == 0x8:
                self.closed = True
                return None
            if opcode == 0x9:
                _arm()
                self.sock.sendall(bytes([0x8A, 0x80]) + os.urandom(4))
                _arm()
                continue
            if opcode == 0xA:
                _arm()
                continue
            if opcode in (0x1, 0x0):
                self._fragments += payload
                if not fin:
                    continue
                raw = bytes(self._fragments)
                self._fragments = bytearray()
                text = raw.decode("utf-8", "replace")
                try:
                    parsed = json.loads(text)
                except ValueError as exc:
                    raise ValueError("websocket text frame is not JSON: " + str(exc)) from exc
                return parsed if isinstance(parsed, Mapping) else {"value": parsed}
            # 二进制帧本适配器不使用：忽略而不是猜测其语义。
            continue

    def close(self) -> None:
        try:
            if not self.closed:
                self.sock.sendall(bytes([0x88, 0x80]) + os.urandom(4))
        except OSError:
            pass
        finally:
            self.closed = True
            try:
                self.sock.close()
            except OSError:
                pass


@dataclass
class _Follow:
    """一个已打开的 session/follow 流（本适配器每轮用一个，轮末关闭）。"""

    client: WsClient
    stream_id: str
    session_id: str
    header_id: Optional[str] = None
    snapshot_cursor: Optional[int] = None
    # 打开 follow 时快照里的已完成轮数；本轮目标 turn = base_turn + 1。
    base_turn: int = 0
    target_turn: Optional[int] = None

    def close(self) -> None:
        self.client.close()


class _ApprovalChannel:
    """常驻 `$events` 订阅：把 DSH 的审批请求转成适配层的待办（OD-ARCH-066）。

    事实来源（script 探针，见 evidence/web-api/approval-probe/probe-facts.json）：
      * 打开 `{type:'open', streamId, endpoint:'$events', payload:{args:{}}}` 后首帧是
        `{type:'ready', clientId, host:{home}}`；
      * 需要人工决定的请求以 `{type:'waterfall', event:'approval/request', eventId,
        agentId, request:{toolName, callId?, reason?, displayReason?}}` 下发，
        且 `agentId` 与本任务的 `engine_session_id` 一致；
      * 应答走 `$events/result`，args 为 `{clientId, eventId, outcome:{kind:'result',
        value:'allowed-once'|'rejected'}}`；
      * 请求被撤回时下发 `{type:'cancel', eventId}`。

    这里只保留归一化后需要的字段，不保存 request 原文。
    """

    def __init__(self, adapter: "WebApiAdapter") -> None:
        self._adapter = adapter
        self._client: Optional[WsClient] = None
        self._stream_id = "stream-events-" + new_omni_task_id()
        self._thread: Optional[threading.Thread] = None
        self._stop = threading.Event()
        self._ready = threading.Event()
        self._lock = threading.Lock()
        self.client_id: Optional[str] = None
        self.last_error: Optional[str] = None

    def ensure_started(self, timeout: float = 15.0) -> str:
        """打开（或复用）事件流并等 ready 帧；返回 clientId。"""
        with self._lock:
            if self.client_id:
                return self.client_id
            if self._thread is not None and self._thread.is_alive():
                deadline = time.monotonic() + float(timeout)
                while time.monotonic() < deadline:
                    if self._ready.wait(0.05) and self.client_id:
                        return self.client_id
                    if self._thread is not None and not self._thread.is_alive():
                        break
                raise EngineError(
                    EngineErrorCode.TIMEOUT,
                    "DSH $events stream did not become ready in " + str(timeout) + "s",
                    detail={"channel": "web-api", "last_error": self.last_error},
                )
            self._stop.clear()
            self._ready.clear()
            try:
                client = self._adapter._open_mux_socket()
                client.send(
                    {
                        "type": "open",
                        "streamId": self._stream_id,
                        "endpoint": "$events",
                        "payload": {"args": {}},
                    }
                )
            except (OSError, ValueError) as exc:
                raise EngineError(
                    EngineErrorCode.ENGINE_UNAVAILABLE,
                    "cannot open DSH $events stream: " + type(exc).__name__,
                    detail={"channel": "web-api"},
                ) from None
            self._client = client
            self._thread = threading.Thread(
                target=self._run, name="omni-web-approvals", daemon=True
            )
            self._thread.start()
        if not self._ready.wait(float(timeout)):
            raise EngineError(
                EngineErrorCode.TIMEOUT,
                "DSH $events stream did not become ready in " + str(timeout) + "s",
                detail={"channel": "web-api", "last_error": self.last_error},
            )
        if not self.client_id:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "DSH $events stream closed before ready",
                detail={"channel": "web-api", "last_error": self.last_error},
            )
        return self.client_id

    def _run(self) -> None:
        assert self._client is not None
        client = self._client
        while not self._stop.is_set():
            try:
                message = client.recv(timeout=1.0)
            except TimeoutError:
                continue
            except (OSError, ValueError) as exc:
                self.last_error = type(exc).__name__
                break
            if message is None:
                self.last_error = "stream_closed"
                break
            if message.get("streamId") not in (None, self._stream_id):
                continue
            value = message.get("value")
            if not isinstance(value, Mapping):
                continue
            self._on_frame(value)
        self._ready.set()

    def _on_frame(self, value: Mapping[str, Any]) -> None:
        frame_type = value.get("type")
        if frame_type == "ready":
            head = value.get("clientId")
            if isinstance(head, str) and head:
                self.client_id = head
            self._ready.set()
            return
        if frame_type == "waterfall":
            if str(value.get("event") or "") != "approval/request":
                # OD-ARCH-088：结构化提问单独登记为可答项；其余 waterfall
                # （钩子等）仍只上报、不自动应答。
                if str(value.get("event") or "") == "user-questions/request":
                    if self._adapter._on_question_request(value):
                        return
                # OD-ARCH-069：其它 waterfall（提问、钩子等）仍不自动应答，
                # 但必须让界面看得见「引擎在等人回答」，否则表现为卡死。
                self._adapter._on_engine_ask(value)
                return
            self._adapter._on_approval_request(value)
            return
        if frame_type == "cancel":
            event_id = str(value.get("eventId") or "")
            # 同一个 cancel 帧只对应一个 eventId；两处清理互不影响。
            self._adapter._on_question_cancel(event_id)
            self._adapter._on_approval_cancel(event_id)
            return
        if frame_type == "error":
            self.last_error = "event_stream_error"
        return

    def close(self) -> None:
        self._stop.set()
        client = self._client
        self._client = None
        if client is not None:
            client.close()


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    """禁止跟随 303：token 换 cookie 只需要读取 Set-Cookie。"""

    def redirect_request(self, req, fp, code, msg, headers, newurl):  # noqa: D401
        return None


class WebApiAdapter(TaskCoreBase):
    """DSH 本地 Web API（HTTP RPC + WebSocket）→ OmniDesk 最小引擎契约。"""

    engine_kind = "dsh-web-api"

    # 无等待上限时，静默多久发一次「仍在运行」提示（只提示，不收尾）。
    _IDLE_NOTE_SECONDS = 60.0
    # OD-ARCH-088：提问题面/选项边界（超限只登记为不可答，不截断）。
    _MAX_QUESTIONS = 8
    _MAX_QUESTION_CHARS = 2000
    _MAX_OPTIONS = 12
    _MAX_OPTION_LABEL_CHARS = 200
    _MAX_CUSTOM_CHARS = 4000

    def __init__(
        self,
        *,
        command: Sequence[str],
        cwd: str,
        env: Optional[Mapping[str, str]] = None,
        state_dir: Optional[str] = None,
        # 默认不按墙钟收尾（None）；宿主若要显式设限，传秒数即可（测试用）。
        default_timeout: Optional[float] = None,
        session_timeout: float = 30.0,
        init_timeout: float = 90.0,
        engine_profile: Optional[str] = None,
        host: str = "127.0.0.1",
        port: int = 0,
    ) -> None:
        if not command:
            raise EngineError(EngineErrorCode.INVALID_ARGUMENT, "command must be a non-empty list")
        if not os.path.isabs(cwd):
            raise EngineError(EngineErrorCode.INVALID_ARGUMENT, "cwd must be an absolute path")
        self._command = list(command)
        self._cwd = cwd
        self._env = dict(env) if env is not None else None
        super().__init__(state_dir=state_dir, default_timeout=default_timeout)
        self._session_timeout = float(session_timeout)
        self._init_timeout = float(init_timeout)
        self._engine_profile = (
            str(engine_profile).strip() if engine_profile not in (None, "") else None
        )
        self._host = host
        self._port_hint = int(port)

        self._proc: Optional[subprocess.Popen] = None
        self._root: Optional[str] = None
        # token / cookie 只留在内存，不出现在任何返回值、异常 detail、台账或日志。
        self._token: Optional[str] = None
        self._cookie: Optional[str] = None
        self._startup_lines: list = []
        self._startup_error: Optional[EngineError] = None
        self._spawn_lock = threading.Lock()
        self._rpc_counter = 0
        self._workspace_tokens: dict = {}
        self._workspace_labels: dict = {}
        self._catalog: Optional[Mapping[str, Any]] = None
        self._follow_clients: list = []
        # OD-ARCH-066：审批通道与待办索引（只存归一化字段与本任务归属）。
        self._approval_channel: Optional[_ApprovalChannel] = None
        self._approval_index: dict = {}
        # OD-ARCH-088：结构化提问索引（只在内存；题干/答案不落状态文件）。
        # event_id -> 归一化待答项；task_id -> {event_id: 待答项}
        self._question_index: dict = {}
        self._questions_by_task: dict = {}
        # OD-ARCH-069：已上报过的「引擎在等人回答」事件 id（只用于去重，不存内容）。
        self._engine_ask_seen: dict = {}
        # 会话读回时用到的展示映射（只存不透明 token，与 ACP 通道一致）。
        self._session_cwd: dict = {}

    # -- 构造便利方法 -------------------------------------------------------
    @classmethod
    def for_dsh(
        cls,
        dsh_bin: str,
        *,
        cwd: str,
        profile: str = "web",
        patch_files: Sequence[str] = (),
        env: Optional[Mapping[str, str]] = None,
        **kwargs: Any,
    ) -> "WebApiAdapter":
        # `--patch` 是 launcher 级参数，必须排在 web app 参数之前。
        cmd = ["node", dsh_bin, "--profile", profile]
        for p in patch_files:
            cmd.extend(["--patch", p])
        cmd.extend(["--port", "0", "--no-open"])
        kwargs.setdefault("engine_profile", profile)
        return cls(command=cmd, cwd=cwd, env=env, **kwargs)

    # -- 进程与鉴权 ---------------------------------------------------------
    def _redact(self, text: str) -> str:
        """去掉启动行里的 token（只用于诊断文本）。"""
        if "token=" in text:
            head, _, tail = text.partition("token=")
            rest = tail.split("&", 1)
            return head + "token=<redacted>" + (("&" + rest[1]) if len(rest) > 1 else "")
        return text

    def _ensure_started(self) -> None:
        with self._spawn_lock:
            if self._cookie and self._root:
                return
            if self._startup_error is not None:
                raise self._startup_error
            env = dict(self._env) if self._env is not None else dict(os.environ)
            try:
                proc = subprocess.Popen(
                    self._command,
                    cwd=self._cwd,
                    env=env,
                    # OD-ARCH-074：让 DSH 独占一个进程组，崩溃后可按登记回收整棵树。
                    start_new_session=True,
                    # OD-ARCH-050 修复：**必须**收回 stdin。
                    #
                    # 不指定 stdin 时子进程会继承引擎自己的协议管道（fd 0）。Node 起
                    # stdin 流时会把这份共享的“打开文件描述”设为非阻塞，引擎随后的
                    # `for line in sys.stdin` 会读到一个假 EOF 并安静退出（rc=0、无栈），
                    # 表现就是“引擎子进程启动后立刻退出”。
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    bufsize=1,
                )
            except OSError as exc:
                err = EngineError(
                    EngineErrorCode.ENGINE_UNAVAILABLE,
                    "failed to start DSH web process: " + type(exc).__name__ + ": " + str(exc),
                    detail={"channel": "web-api"},
                )
                self._startup_error = err
                raise err
            self._proc = proc
            record_child_group(self._state_dir, proc.pid)

            # stdout 由 reader 线程接管：readline() 在有界等待里可能永久阻塞，
            # 这里用队列 + 超时，保证“进程起来了但不说地址”能报超时而不是卡死。
            lines: "queue.Queue[Optional[str]]" = queue.Queue()

            def pump() -> None:
                assert proc.stdout is not None
                try:
                    for line in proc.stdout:
                        lines.put(line)
                finally:
                    lines.put(None)

            threading.Thread(target=pump, name="omni-web-stdout", daemon=True).start()
            deadline = time.monotonic() + self._init_timeout
            url: Optional[str] = None
            while time.monotonic() < deadline:
                remaining = max(0.05, deadline - time.monotonic())
                try:
                    line = lines.get(timeout=min(1.0, remaining))
                except queue.Empty:
                    if proc.poll() is not None:
                        break
                    continue
                if line is None:
                    break
                text = line.strip()
                if text:
                    self._startup_lines.append(self._redact(text))
                    del self._startup_lines[:-40]
                if "dsh web:" in text:
                    url = text.split("dsh web:", 1)[1].strip()
                    break
            if url is None:
                code = proc.poll()
                tail = " | ".join(self._startup_lines[-6:])
                err = EngineError(
                    EngineErrorCode.ENGINE_UNAVAILABLE,
                    "DSH web server did not announce its address within "
                    + str(self._init_timeout)
                    + "s"
                    + (" (process exited rc=" + str(code) + ")" if code is not None else ""),
                    detail={"channel": "web-api", "startup_tail": tail},
                )
                self._startup_error = err
                self._terminate_process()
                raise err
            self._root, token = self._parse_announcement(url)
            self._token = token
            try:
                self._cookie = self._exchange_token()
            except EngineError as error:
                self._startup_error = error
                self._terminate_process()
                raise
            # token 用完即丢弃；后续只用 cookie。
            self._token = None

    def _parse_announcement(self, raw: str) -> tuple[str, str]:
        parsed = urllib.parse.urlsplit(raw)
        if parsed.scheme not in ("http", "https") or not parsed.hostname:
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "DSH web announcement is not an http URL",
                detail={"channel": "web-api"},
            )
        token = urllib.parse.parse_qs(parsed.query).get("token", [""])[0]
        if not token:
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "DSH web announcement carries no access token",
                detail={"channel": "web-api"},
            )
        root = parsed.scheme + "://" + parsed.netloc
        return root, token

    def _exchange_token(self) -> str:
        assert self._root is not None and self._token is not None
        url = self._root + "/?token=" + urllib.parse.quote(self._token, safe="")
        opener = urllib.request.build_opener(_NoRedirect)
        try:
            response = opener.open(urllib.request.Request(url, method="GET"), timeout=30)
            cookies = response.headers.get_all("Set-Cookie") or []
        except urllib.error.HTTPError as err:
            if err.code not in (301, 302, 303, 307, 308):
                raise EngineError(
                    EngineErrorCode.ENGINE_UNAVAILABLE,
                    "DSH web token exchange failed with HTTP " + str(err.code),
                    detail={"channel": "web-api"},
                ) from None
            cookies = err.headers.get_all("Set-Cookie") or []
        except (urllib.error.URLError, OSError) as exc:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "DSH web token exchange failed: " + type(exc).__name__,
                detail={"channel": "web-api"},
            ) from None
        pairs = [c.split(";", 1)[0].strip() for c in cookies if c.strip()]
        cookie = "; ".join(p for p in pairs if "=" in p)
        if not cookie:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "DSH web token exchange returned no auth cookie",
                detail={"channel": "web-api"},
            )
        return cookie

    def _terminate_process(self) -> None:
        proc = self._proc
        if proc is None:
            return
        if proc.poll() is None:
            try:
                proc.terminate()
                proc.wait(timeout=5)
            except (OSError, subprocess.TimeoutExpired):
                pass
        # 节点进程可能已退出但留下子进程：按登记过的进程组收尾。
        self._kill_child_group(3.0)
        if proc.poll() is None:
            try:
                proc.kill()
                proc.wait(timeout=5)
            except (OSError, subprocess.TimeoutExpired):
                pass

    def _kill_child_group(self, grace: float) -> bool:
        proc = self._proc
        if proc is None:
            return False
        try:
            return terminate_group(proc.pid, grace=grace, trusted_leader=proc.pid)
        except Exception as exc:  # 身份不可核验时不杀，只记诊断
            self._startup_lines.append(
                "process-group cleanup skipped: " + type(exc).__name__
            )
            return False

    # -- HTTP RPC -----------------------------------------------------------
    def _next_rpc_id(self) -> str:
        with self._lock:
            self._rpc_counter += 1
            return "omni-rpc-" + str(self._rpc_counter)

    def _rpc(
        self,
        method: str,
        args: Mapping[str, Any],
        timeout: float,
        omni_task_id: Optional[str] = None,
    ) -> Any:
        self._ensure_started()
        assert self._root is not None and self._cookie is not None
        body = json.dumps(
            {
                "type": "client-request",
                "rpcId": self._next_rpc_id(),
                "method": method,
                "payload": {"args": dict(args)},
            }
        ).encode("utf-8")
        request = urllib.request.Request(
            self._root + "/api/" + method,
            data=body,
            method="POST",
            headers={"Content-Type": "application/json", "Cookie": self._cookie},
        )
        try:
            with urllib.request.urlopen(request, timeout=float(timeout)) as response:
                raw = response.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as err:
            raise self._map_http_error(err, method, omni_task_id) from None
        except (urllib.error.URLError, OSError) as exc:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "DSH web RPC " + method + " failed: " + type(exc).__name__,
                detail={"channel": "web-api", "method": method},
                omni_task_id=omni_task_id,
            ) from None
        try:
            payload = json.loads(raw)
        except ValueError as exc:
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "DSH web RPC " + method + " returned non-JSON: " + str(exc),
                detail={"channel": "web-api", "method": method},
                omni_task_id=omni_task_id,
            ) from None
        if not isinstance(payload, Mapping):
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "DSH web RPC " + method + " returned a non-object response",
                detail={"channel": "web-api", "method": method},
                omni_task_id=omni_task_id,
            )
        result = payload.get("result")
        if not isinstance(result, Mapping):
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "DSH web RPC " + method + " response has no result object",
                detail={"channel": "web-api", "method": method},
                omni_task_id=omni_task_id,
            )
        if result.get("ok") is not True:
            raise self._map_rpc_failure(result.get("error"), method, omni_task_id)
        return result.get("value")

    def _map_http_error(
        self, err: urllib.error.HTTPError, method: str, omni_task_id: Optional[str]
    ) -> EngineError:
        detail = {"channel": "web-api", "method": method, "http_status": int(err.code)}
        if err.code in (401, 403):
            return EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "DSH web RPC rejected the session cookie (HTTP " + str(err.code) + ")",
                detail=detail,
                omni_task_id=omni_task_id,
            )
        # OD-ARCH-070：HTTP 400 常常是上游模型网关的拒绝（body 里带
        # code=upstream_error 与原因）。尽量取出原文，让用户看到真实原因。
        body = ""
        try:
            body = err.read().decode("utf-8", "replace")
        except Exception:  # noqa: BLE001 - 读不到 body 不影响判定
            body = ""
        try:
            parsed = json.loads(body) if body else None
        except ValueError:
            parsed = None
        error_obj = parsed.get("error") if isinstance(parsed, Mapping) else None
        code = ""
        message = ""
        if isinstance(error_obj, Mapping):
            code = str(error_obj.get("code") or "")
            message = str(error_obj.get("message") or "")
        if "upstream" in (code + " " + message).lower():
            detail["upstream"] = True
            return EngineError(
                EngineErrorCode.TASK_FAILED,
                self._summarize_upstream_error(message or code),
                detail=detail,
                omni_task_id=omni_task_id,
            )
        return EngineError(
            EngineErrorCode.ENGINE_UNAVAILABLE,
            "DSH web RPC " + method + " failed with HTTP " + str(err.code),
            detail=detail,
            omni_task_id=omni_task_id,
        )

    def _map_rpc_failure(
        self, error: Any, method: str, omni_task_id: Optional[str]
    ) -> EngineError:
        code = ""
        message = ""
        if isinstance(error, Mapping):
            code = str(error.get("code") or "")
            message = str(error.get("message") or "")
        low = (code + " " + message).lower()
        detail = {"channel": "web-api", "method": method, "rpc_code": code}
        if "upstream" in low:
            # OD-ARCH-070：上游网关拒绝（如 HTTP 400 Content Exists Risk）
            # 是任务失败，不是「引擎不可用」，更不该显示成状态未知。
            detail["upstream"] = True
            return EngineError(
                EngineErrorCode.TASK_FAILED,
                self._summarize_upstream_error(message or code),
                detail=detail,
                omni_task_id=omni_task_id,
            )
        if "session/not-found" in low or "unknown session" in low:
            mapped = EngineErrorCode.UNKNOWN_SESSION
        elif "fork-unavailable" in low:
            # 切点在源会话里不存在（或源会话没有完整轮次）：是参数问题，不是引擎掉线。
            mapped = EngineErrorCode.INVALID_ARGUMENT
        elif "not-found" in low:
            mapped = EngineErrorCode.UNKNOWN_TASK
        elif "unsupported" in low or "not implemented" in low:
            mapped = EngineErrorCode.UNSUPPORTED
        elif "invalid" in low or "bad-request" in low:
            mapped = EngineErrorCode.INVALID_ARGUMENT
        else:
            mapped = EngineErrorCode.ENGINE_UNAVAILABLE
        return EngineError(
            mapped,
            message or ("engine returned an error for " + method),
            detail=detail,
            omni_task_id=omni_task_id,
        )

    # -- 4.1 initialize -----------------------------------------------------
    def model_catalog(self, *, refresh: bool = False) -> Mapping[str, Any]:
        """归一化 session/modelCatalog。凭证不在此接口出现。"""
        with self._lock:
            cached = self._catalog
        if cached is not None and not refresh:
            return cached
        value = self._rpc("session/modelCatalog", {}, self._init_timeout)
        if not isinstance(value, Mapping):
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "session/modelCatalog returned a non-object value",
                detail={"channel": "web-api"},
            )
        groups: list[dict[str, Any]] = []
        models: list[dict[str, Any]] = []
        for group in value.get("groups") or []:
            if not isinstance(group, Mapping):
                continue
            group_models: list[dict[str, Any]] = []
            for model in group.get("models") or []:
                if not isinstance(model, Mapping):
                    continue
                reasoning = model.get("reasoning") or {}
                efforts = reasoning.get("efforts") if isinstance(reasoning, Mapping) else None
                normalized_efforts = [
                    {
                        "id": str(e.get("id") or ""),
                        "name": str(e.get("name") or e.get("id") or ""),
                        "description": str(e.get("description") or ""),
                    }
                    for e in (efforts or [])
                    if isinstance(e, Mapping) and e.get("id")
                ]
                item = {
                    "id": str(model.get("id") or ""),
                    "name": str(model.get("name") or model.get("id") or ""),
                    "description": str(model.get("description") or ""),
                    "provider": str(group.get("id") or ""),
                    "provider_name": str(group.get("name") or group.get("id") or ""),
                    "efforts": normalized_efforts,
                    "default_effort": (
                        str(reasoning.get("defaultEffort"))
                        if isinstance(reasoning, Mapping) and reasoning.get("defaultEffort")
                        else None
                    ),
                }
                group_models.append(item)
                models.append(item)
            groups.append(
                {
                    "id": str(group.get("id") or ""),
                    "name": str(group.get("name") or group.get("id") or ""),
                    "models": group_models,
                }
            )
        catalog = {
            "default": dict(value.get("default") or {})
            if isinstance(value.get("default"), Mapping)
            else {},
            "routable_providers": [str(p) for p in (value.get("routableProviders") or [])],
            "groups": groups,
            "models": models,
            "failures": [
                dict(f) if isinstance(f, Mapping) else {"detail": str(f)}
                for f in (value.get("failures") or [])
            ],
        }
        with self._lock:
            self._catalog = catalog
        return catalog

    def initialize(self) -> EngineDescriptor:
        with self._lock:
            if self._descriptor is not None:
                return self._descriptor
        self._ensure_started()
        catalog = self.model_catalog(refresh=True)
        descriptor = EngineDescriptor(
            engine_kind="dsh-web-api",
            engine_version="unknown",
            protocol_version=WEB_API_PROTOCOL_VERSION,
            compatibility="compatible",
            healthy=True,
            capabilities=dict(ADAPTER_CAPABILITIES),
            detail={
                "channel": "web-api",
                "profile": self._engine_profile,
                "routable_providers": list(catalog["routable_providers"]),
                "model_groups": len(catalog["groups"]),
                "catalog_failures": len(catalog["failures"]),
                "streaming": "assistant-stream/text-delta",
                "note": "capabilities 为适配层声明；实际通过与否见 048 测试报告",
                "known_limits": [
                    "DSH local web API is an internal interface; pinned runtime version only",
                    "reasoning stream is unverified with the synthetic endpoint",
                ],
            },
        )
        with self._lock:
            self._descriptor = descriptor
        return descriptor

    # -- 会话建立 / follow 流 -----------------------------------------------
    def _create_session(self, workspace_dir: str, omni_task_id: Optional[str]) -> str:
        value = self._rpc(
            "session/create",
            {"request": {"cwd": workspace_dir}},
            self._session_timeout,
            omni_task_id=omni_task_id,
        )
        if not isinstance(value, Mapping):
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "session/create returned a non-object value",
                detail={"channel": "web-api"},
                omni_task_id=omni_task_id,
            )
        session_id = value.get("sessionId")
        if not session_id:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "session/create did not return a session id",
                detail={"result_keys": sorted(str(k) for k in value.keys())},
                omni_task_id=omni_task_id,
            )
        with self._lock:
            self._session_cwd[str(session_id)] = workspace_dir
        return str(session_id)

    def _open_mux_socket(self) -> WsClient:
        """打开一条新的 remote.mux WebSocket（用内存里的 cookie 鉴权）。"""
        assert self._root is not None and self._cookie is not None
        if self._root.startswith("https://"):
            raise EngineError(
                EngineErrorCode.UNSUPPORTED,
                "wss:// is not implemented by the built-in websocket client",
                detail={"channel": "web-api"},
            )
        ws_url = "ws://" + self._root[len("http://"):] + "/api/remote.mux"
        try:
            return WsClient(ws_url, self._cookie, timeout=self._session_timeout)
        except (OSError, ValueError) as exc:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "cannot open websocket: " + type(exc).__name__,
                detail={"channel": "web-api"},
            ) from None

    def _open_follow(self, session_id: str, workspace_dir: str) -> _Follow:
        """打开 session/follow 并读到首条 snapshot。

        snapshot 之前只允许出现 error 帧；其它帧一律跳过（不猜测语义）。
        失败时关闭连接后再抛错，避免泄漏文件描述符。
        """
        self._ensure_started()
        stream_id = "stream-" + new_omni_task_id()
        try:
            client = self._open_mux_socket()
        except EngineError as error:
            raise EngineError(
                error.code,
                "cannot open session/follow websocket: " + error.message,
                detail={"channel": "web-api"},
            ) from None
        follow = _Follow(client=client, stream_id=stream_id, session_id=session_id)
        try:
            client.send(
                {
                    "type": "open",
                    "streamId": stream_id,
                    "endpoint": "session/follow",
                    "payload": {
                        "args": {
                            "request": {
                                "address": {"kind": "session", "sessionId": session_id},
                                "assistantStream": True,
                            }
                        }
                    },
                }
            )
            deadline = time.monotonic() + self._session_timeout
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise EngineError(
                        EngineErrorCode.TIMEOUT,
                        "session/follow did not deliver a snapshot within "
                        + str(self._session_timeout)
                        + "s",
                        detail={"channel": "web-api"},
                    )
                try:
                    message = client.recv(timeout=remaining)
                except TimeoutError:
                    continue
                except (OSError, ValueError) as exc:
                    raise EngineError(
                        EngineErrorCode.ENGINE_UNAVAILABLE,
                        "session/follow stream failed before snapshot: " + type(exc).__name__,
                        detail={"channel": "web-api"},
                    ) from None
                if message is None:
                    raise EngineError(
                        EngineErrorCode.ENGINE_UNAVAILABLE,
                        "session/follow stream closed before snapshot",
                        detail={"channel": "web-api"},
                    )
                if message.get("streamId") not in (None, stream_id):
                    continue
                frame_error = self._frame_error(message)
                if frame_error is not None:
                    # 顶层错误帧（含 session/not-found）必须让调用方立即知道，
                    # 不能继续等 snapshot 直到超时。
                    self._raise_follow_error({"error": frame_error}, session_id)
                    continue
                value = message.get("value")
                if not isinstance(value, Mapping):
                    continue
                kind = value.get("type")
                if kind == "error":
                    self._raise_follow_error(value, session_id)
                    continue
                if kind != "snapshot":
                    continue
                header = value.get("header")
                if isinstance(header, Mapping):
                    raw_id = header.get("id")
                    follow.header_id = str(raw_id) if raw_id else None
                cursor = value.get("cursor")
                if isinstance(cursor, int):
                    follow.snapshot_cursor = cursor
                follow.base_turn = self._highest_turn(value.get("records"))
                follow.target_turn = follow.base_turn + 1
                with self._lock:
                    self._follow_clients.append(follow)
                return follow
        except BaseException:
            follow.close()
            raise

    def _raise_follow_error(self, value: Mapping[str, Any], session_id: str) -> None:
        error = value.get("error")
        code = ""
        message = ""
        if isinstance(error, Mapping):
            code = str(error.get("code") or "")
            message = str(error.get("message") or "")
        low = (code + " " + message).lower()
        if "not-found" in low or "not found" in low:
            raise EngineError(
                EngineErrorCode.UNKNOWN_SESSION,
                "engine session is gone: " + (message or code),
                detail={"channel": "web-api", "follow_code": code},
            )
        if "unsupported" in low:
            raise EngineError(
                EngineErrorCode.UNSUPPORTED,
                message or code or "session/follow is unsupported",
                detail={"channel": "web-api", "follow_code": code},
            )
        # OD-ARCH-070：上游模型网关的失败（如 HTTP 400 + code=upstream_error +
        # 「Content Exists Risk」）过去被归成 engine_unavailable，于是任务显示为
        # 「已中断」甚至一路等到超时，用户看不到真正原因。这里把上游原文透传成
        # 明确的失败：状态 FAILED，error_message 带截断后的上游说明。
        if "upstream" in low:
            raise EngineError(
                EngineErrorCode.TASK_FAILED,
                self._summarize_upstream_error(message or code),
                detail={
                    "channel": "web-api",
                    "follow_code": code,
                    "upstream": True,
                },
            )
        raise EngineError(
            EngineErrorCode.ENGINE_UNAVAILABLE,
            "session/follow reported an error: " + (message or code or "unknown"),
            detail={"channel": "web-api", "follow_code": code},
        )

    @staticmethod
    def _summarize_upstream_error(message: str) -> str:
        """把上游错误压成一行、可展示的原因（最多 300 字符）。

        上游文案可能带 HTML 或大段堆栈；截断只影响展示，不改变判定。
        不做任何凭证替换以外的改写——用户需要看到真实原因。
        """
        text = " ".join(str(message or "").split())
        if not text:
            text = "上游模型服务返回错误，未提供原因"
        if len(text) > 300:
            text = text[:300] + "…（已截断）"
        return text

    @staticmethod
    def _frame_error(message: Mapping[str, Any]) -> Optional[Mapping[str, Any]]:
        """取错误信息。

        DSH 的 WebSocket 流有两种错误形态：一是 `{"type":"error","error":{...}}`
        直接在帧顶层（实测：session/not-found），二是包在 `value.type == "error"`
        里。只认其中一种会把“会话不存在”拖成超时（OD-ARCH-048 实测）。
        这里统一成 `{"code","message"}` 供 :meth:`_raise_follow_error` 复用。
        """
        if str(message.get("type") or "") == "error":
            error = message.get("error")
            if isinstance(error, Mapping):
                return error
            return {"message": "session/follow reported an error"}
        value = message.get("value")
        if isinstance(value, Mapping) and value.get("type") == "error":
            error = value.get("error")
            if isinstance(error, Mapping):
                return error
            return {"message": "session/follow reported an error"}
        return None

    @staticmethod
    def _highest_turn(records: Any) -> int:
        """snapshot 里出现过的最高轮次。没有轮次时为 0（下一轮就是 1）。"""
        highest = 0
        if not isinstance(records, (list, tuple)):
            return 0
        for record in records:
            if not isinstance(record, Mapping):
                continue
            candidates: list = []
            if isinstance(record.get("turn"), int):
                candidates.append(record["turn"])
            event = record.get("event")
            if isinstance(event, Mapping):
                data = event.get("data")
                if isinstance(data, Mapping) and isinstance(data.get("turn"), int):
                    candidates.append(data["turn"])
            for value in candidates:
                if value > highest:
                    highest = value
        return highest

    # -- 4.2 submit_task ----------------------------------------------------
    def select_model(
        self,
        session_id: str,
        *,
        provider: str,
        model: str,
        reasoning_effort: Optional[str] = None,
    ) -> Mapping[str, Any]:
        """按 session/selectModel 切换到目录中真实存在的模型。

        只接受调用方从 model_catalog() 得到的 provider/model/effort 组合；
        这里不读、不写任何凭证。
        """
        if not session_id or not provider or not model:
            raise EngineError(
                EngineErrorCode.INVALID_ARGUMENT,
                "session_id, provider and model are required",
            )
        request: dict[str, Any] = {
            "sessionId": session_id,
            "provider": provider,
            "model": model,
        }
        if reasoning_effort:
            request["reasoningEffort"] = reasoning_effort
        value = self._rpc(
            "session/selectModel", {"request": request}, self._session_timeout
        )
        if isinstance(value, Mapping):
            return value
        return {"ok": True}

    def submit_task(self, request: SubmitTaskRequest) -> SubmitTaskReceipt:
        if not request.intent or not isinstance(request.intent, str):
            raise EngineError(EngineErrorCode.INVALID_ARGUMENT, "intent must be a non-empty string")
        workspace_dir = canonical_cwd(request.workspace_dir, field="workspace_dir")
        text = request.prompt_text
        if text is None and request.input_file:
            try:
                text = Path(request.input_file).read_text(encoding="utf-8")
            except OSError as exc:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "input_file could not be read: " + str(exc),
                    detail={"input_file": request.input_file},
                )
        if not text:
            raise EngineError(
                EngineErrorCode.INVALID_ARGUMENT, "prompt_text or input_file is required"
            )

        with self._lock:
            if request.idempotency_key and request.idempotency_key in self._idempotency:
                existing = self._idempotency[request.idempotency_key]
                return SubmitTaskReceipt(
                    omni_task_id=existing.omni_task_id,
                    status=existing.state,
                    accepted_at=existing.created_at,
                    engine_session_id=existing.engine_session_id,
                    note="idempotency_key 命中，未重复提交",
                    session_mode=existing.session_mode,
                )
            omni_task_id = request.omni_task_id or new_omni_task_id()
            if omni_task_id in self._tasks:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "omni_task_id already exists; ids must not be reused",
                    detail={"omni_task_id": omni_task_id},
                )
            record = _TaskRecord(
                omni_task_id=omni_task_id,
                intent=request.intent,
                workspace_dir=workspace_dir,
                idempotency_key=request.idempotency_key,
                created_at=request.requested_at or now(),
                state_since=now(),
                state=TaskState.QUEUED,
                evidence_source="operator_confirm",
                timeouts=dict(request.timeout_policy),
                submitted_monotonic=time.monotonic(),
            )
            self._tasks[omni_task_id] = record
            if request.idempotency_key:
                self._idempotency[request.idempotency_key] = record
        self._persist(record)

        # 审批订阅：失败不阻断提交，但会在任务状态里如实体现缺失的审批通道。
        self._ensure_approval_channel()

        session_id: Optional[str] = None
        session_mode = "new"
        resume_note = ""
        follow: Optional[_Follow] = None

        if request.engine_session_id:
            existing_id = str(request.engine_session_id)
            with self._lock:
                live_here = existing_id in self._sessions
            if live_here:
                session_id = existing_id
                session_mode = "continued"
            else:
                try:
                    # Web API 没有单独的 session/resume；能打开 follow 即说明会话存在。
                    follow = self._open_follow(existing_id, workspace_dir)
                    session_id = existing_id
                    session_mode = "resumed"
                except EngineError as error:
                    session_mode = "degraded"
                    resume_note = "既有会话无法续接（已改为新建会话继续）：" + error.code.value

        try:
            if session_id is None:
                session_id = self._create_session(workspace_dir, omni_task_id)
            if follow is None:
                follow = self._open_follow(session_id, workspace_dir)
        except EngineError as error:
            if follow is not None:
                self._drop_follow(follow)
            self._settle_failed(record, error)
            raise

        with self._lock:
            record.engine_session_id = session_id
            record.session_mode = session_mode
            record.state = TaskState.STARTING
            record.evidence_source = "operator_confirm"
            record.state_since = now()
            self._sessions[session_id] = record
            self._session_cwd[session_id] = workspace_dir
        self._persist(record)

        worker = threading.Thread(
            target=self._turn_worker,
            args=(record, follow, text),
            name="omni-web-turn-" + omni_task_id,
            daemon=True,
        )
        worker.start()

        with self._lock:
            state = record.state
        if session_mode == "resumed":
            receipt_note = "已重开既有引擎会话并提交；等待引擎终态证据"
        elif session_mode == "continued":
            receipt_note = "已在同一引擎会话内继续本轮；等待引擎终态证据"
        elif session_mode == "degraded":
            receipt_note = resume_note + "；等待引擎终态证据"
        else:
            receipt_note = "适配层已建立会话并提交；等待引擎终态证据"
        return SubmitTaskReceipt(
            omni_task_id=omni_task_id,
            status=state,
            accepted_at=record.created_at,
            engine_session_id=session_id,
            note=receipt_note,
            session_mode=session_mode,
        )

    def _drop_follow(self, follow: _Follow) -> None:
        with self._lock:
            try:
                self._follow_clients.remove(follow)
            except ValueError:
                pass
        follow.close()

    def _settle(
        self,
        record: _TaskRecord,
        state: TaskState,
        evidence: str,
        **kwargs: Any,
    ) -> bool:
        """终态落定时清掉待审批索引：引擎已结束，旧请求不应再挂在界面上。"""
        closed_questions: list = []
        if state in TERMINAL_STATES:
            with self._lock:
                stale = list(record.pending_approvals)
                record.pending_approvals.clear()
                for approval_id in stale:
                    entry = self._approval_index.get(approval_id)
                    if entry and entry.get("omni_task_id") == record.omni_task_id:
                        self._approval_index.pop(approval_id, None)
                # 终态后同一个任务不会再收到询问：顺手清掉去重键，避免无限增长。
                for key in [k for k in self._engine_ask_seen if k[0] == record.omni_task_id]:
                    self._engine_ask_seen.pop(key, None)
                # OD-ARCH-088：终态时把仍在等答的提问逐条标记为已关闭。
                pending_questions = self._questions_by_task.pop(record.omni_task_id, {})
                for question_id in list(pending_questions):
                    entry = self._question_index.get(question_id)
                    if entry is not None and entry.get("omni_task_id") == record.omni_task_id:
                        self._question_index.pop(question_id, None)
                    closed_questions.append(question_id)
        for question_id in closed_questions:
            self._emit(
                record,
                EventType.PROGRESS,
                {
                    "kind": "question_closed",
                    "question_id": question_id,
                    "reason": "task_terminal",
                    "note": "任务已结束，提问不再可答",
                },
            )
        return super()._settle(record, state, evidence, **kwargs)

    def _task_timeout(self, record: _TaskRecord) -> Optional[float]:
        """本轮等待预算；None 表示不按时间自动收尾。

        产品口径（2026-10-23 用户指令）：只要 DSH 没报错、进程还在跑，就不能
        因为墙钟时间把任务判成 unknown。原版 DSH 跑一次联网检索转到完成要八分钟
        以上，任何固定秒数都会误杀正常任务。

        因此 `timeout_policy` 不再作为默认截断开关：只有显式给出正整数才按该值
        收尾（测试与脚本仍可用它模拟超时）；缺省时沿用构造参数 default_timeout
        （生产默认 None），0、负数、非法值一律表示无上限。
        用户点「取消」走 cancel_task，是唯一的主动终止路径。
        """
        policy = record.timeouts or {}
        total = policy.get("total_seconds")
        if total in (None, ""):
            return self._default_timeout
        if total == 0:
            return None
        try:
            value = float(total)
        except (TypeError, ValueError):
            return None
        if not (value > 0) or value != value:
            return None
        return value

    @staticmethod
    def _timeout_note(seconds: float) -> str:
        """超时文案（OD-ARCH-069）：说明等了多久、没做什么、下一步能做什么。"""
        try:
            shown = int(round(float(seconds)))
        except (TypeError, ValueError):
            shown = 0
        return (
            "引擎在 " + str(shown) + " 秒内未返回结果，本次未自动重试；"
            "可点「重发」或降低工具使用后重试"
        )

    def _turn_worker(self, record: _TaskRecord, follow: _Follow, text: str) -> None:
        """发 session/prompt，然后把 follow 流翻译成归一化事件与终态。"""
        try:
            with self._lock:
                record.state = TaskState.RUNNING
                record.state_since = now()
            self._emit(
                record,
                EventType.TASK_STARTED,
                {
                    "normalized": True,
                    "reason": "session/prompt 已发出",
                    "intent": record.intent,
                    "session_mode": record.session_mode,
                    "channel": "web-api",
                },
            )
            request_id = "prompt-" + record.omni_task_id
            with self._lock:
                record.prompt_request_id = request_id
            self._rpc(
                "session/prompt",
                {
                    "request": {
                        "requestId": request_id,
                        "sessionId": follow.session_id,
                        "mode": "queue",
                        "content": [{"type": "text", "text": text}],
                    }
                },
                self._session_timeout,
                omni_task_id=record.omni_task_id,
            )
        except EngineError as error:
            self._settle_failed(record, error)
            self._drop_follow(follow)
            return

        timeout_seconds = self._task_timeout(record)
        started_at = time.monotonic()
        # None = 不按墙钟收尾。只有调用方显式给正整数时才设截止（测试与脚本用）。
        deadline = None if timeout_seconds is None else started_at + timeout_seconds
        budget_seconds = timeout_seconds or 0
        last_frame_at = started_at
        last_idle_note_at = started_at
        final_text = ""
        finish_reason: Optional[str] = None
        try:
            while True:
                if deadline is None:
                    wait_slice = self._IDLE_NOTE_SECONDS
                else:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        timeout_note = self._timeout_note(float(budget_seconds))
                        self._settle(
                            record,
                            TaskState.UNKNOWN,
                            "timeout",
                            note=timeout_note,
                            # 状态查询以 stop_reason/error_message 组成 note；
                            # 只放进事件会让界面看不到「等了多少秒」。
                            error_code=EngineErrorCode.TIMEOUT.value,
                            error_message=timeout_note,
                        )
                        return
                    # 有截止时按截止切分等待，一次读取不超过剩余预算。
                    wait_slice = remaining
                try:
                    message = follow.client.recv(timeout=wait_slice)
                except TimeoutError:
                    # 无上限模式下这里只是「一段时间没收到帧」：进程还在、连接还在，
                    # 只发一条进度提示，绝不改任务状态，更不能收尾。
                    if deadline is None:
                        idle_for = time.monotonic() - last_frame_at
                        if idle_for >= self._IDLE_NOTE_SECONDS and (
                            time.monotonic() - last_idle_note_at >= self._IDLE_NOTE_SECONDS
                        ):
                            last_idle_note_at = time.monotonic()
                            self._emit(
                                record,
                                EventType.PROGRESS,
                                {
                                    "kind": "still_running",
                                    "idle_seconds": int(round(idle_for)),
                                    "note": "DSH 仍在运行（已静默 "
                                    + str(int(round(idle_for)))
                                    + " 秒）；不会自动停止，需要时请手动取消",
                                },
                            )
                    continue
                except (OSError, ValueError) as exc:
                    self._settle(
                        record,
                        TaskState.INTERRUPTED,
                        "process_state",
                        note="事件流异常中断（" + type(exc).__name__ + "）；未自动重放",
                    )
                    return
                if message is None:
                    self._settle(
                        record,
                        TaskState.INTERRUPTED,
                        "process_state",
                        note="引擎在给出终态前关闭了事件流；未自动重放",
                    )
                    return
                if message.get("streamId") not in (None, follow.stream_id):
                    continue
                last_frame_at = time.monotonic()
                frame_error = self._frame_error(message)
                if frame_error is not None:
                    try:
                        self._raise_follow_error({"error": frame_error}, follow.session_id)
                    except EngineError as error:
                        self._settle_failed(record, error)
                        return
                    continue
                value = message.get("value")
                if not isinstance(value, Mapping):
                    continue
                kind = value.get("type")
                if kind == "error":
                    try:
                        self._raise_follow_error(value, follow.session_id)
                    except EngineError as error:
                        self._settle_failed(record, error)
                        return
                    continue
                if kind == "assistant-stream":
                    frame = value.get("frame")
                    if isinstance(frame, Mapping):
                        reason = self._on_stream_frame(record, frame)
                        if reason:
                            finish_reason = reason
                    continue
                if kind != "event":
                    continue
                event = value.get("event")
                if not isinstance(event, Mapping):
                    continue
                event_type = str(event.get("type") or "")
                data = event.get("data")
                data = data if isinstance(data, Mapping) else {}

                if event_type == "assistant/message":
                    message_obj = data.get("message")
                    if isinstance(message_obj, Mapping):
                        final_text = self._message_text(message_obj)
                    continue
                if event_type == "system/message":
                    # 完整系统提示词：绝不进入事件、结果、日志或台账。
                    continue
                if event_type == "tool/call" or event_type.startswith("tool/"):
                    # OD-ARCH-069：工具名来自上游事件，字段名不完全稳定
                    # （tool/call 用 name，其它形态可能用 toolName/tool），
                    # 防御性取；取不到就留空，由界面显示「未知工具」。
                    tool_name = self._tool_name_of(data, event)
                    self._emit(
                        record,
                        EventType.TOOL_ACTIVITY,
                        {
                            "kind": event_type,
                            "tool_name": tool_name,
                            "call_id": str(data.get("callId") or data.get("call_id") or ""),
                            "status": str(data.get("status") or ""),
                            "note": "工具被调用不等于工具已成功执行",
                        },
                    )
                    continue
                if event_type == "turn/end":
                    reason_obj = data.get("reason")
                    reason_kind = (
                        str(reason_obj.get("kind") or "")
                        if isinstance(reason_obj, Mapping)
                        else ""
                    )
                    # 记下本轮结束事件在会话里的精确序号：DSH 的 session/fork
                    # 用「包含该切点」的事件序号作分叉边界，界面据此把这条回复
                    # 变成一条真正的子会话。序号缺失时保持 None，不猜。
                    self._record_turn_end_seq(record, event)
                    self._settle_turn(record, reason_kind, finish_reason, final_text)
                    return
                continue
        finally:
            self._drop_follow(follow)

    @staticmethod
    def _tool_name_of(data: Mapping[str, Any], event: Mapping[str, Any]) -> str:
        """从工具事件里取工具名；字段形态不稳定，取不到就返回空串。

        只读展示用的短标识，绝不把工具入参/结果放进事件。
        """
        for container in (data, event):
            for key in ("name", "toolName", "tool", "tool_name"):
                value = container.get(key)
                if isinstance(value, str) and value.strip():
                    return value.strip()
        return ""

    @staticmethod
    def _message_text(message: Mapping[str, Any]) -> str:
        content = message.get("content")
        if not isinstance(content, (list, tuple)):
            return ""
        parts: list[str] = []
        for block in content:
            if isinstance(block, Mapping) and block.get("type") == "text":
                parts.append(str(block.get("text") or ""))
        return "".join(parts)

    def _on_stream_frame(self, record: _TaskRecord, frame: Mapping[str, Any]) -> Optional[str]:
        """处理 assistant-stream 帧；返回 finish reason（若有）。"""
        frame_type = frame.get("type")
        if frame_type == "chunk":
            chunk = frame.get("chunk")
            if not isinstance(chunk, Mapping):
                return None
            chunk_type = chunk.get("type")
            if chunk_type == "text-delta":
                text = str(chunk.get("text") or "")
                if not text:
                    return None
                measure_ttfb = False
                with self._lock:
                    if record.ttfb_seconds is None and record.submitted_monotonic is not None:
                        record.ttfb_seconds = round(
                            max(0.0, time.monotonic() - record.submitted_monotonic), 3
                        )
                        measure_ttfb = True
                    record.text_parts.append(text)
                    record.text_len += len(text)
                if measure_ttfb:
                    self._persist(record)
                self._emit(
                    record,
                    EventType.TEXT_DELTA,
                    {"text": text, "chars": len(text), "streamed": True},
                )
            elif chunk_type == "usage":
                usage = chunk.get("usage")
                self._emit(
                    record,
                    EventType.PROGRESS,
                    {"kind": "usage", "usage": dict(usage) if isinstance(usage, Mapping) else {}},
                )
            elif chunk_type == "finish":
                reason = chunk.get("reason")
                if isinstance(reason, Mapping) and reason.get("kind"):
                    return str(reason.get("kind"))
            return None
        if frame_type == "end":
            outcome = frame.get("outcome")
            kind = str(outcome.get("kind") or "") if isinstance(outcome, Mapping) else ""
            if kind and kind != "committed":
                self._emit(
                    record,
                    EventType.PROGRESS,
                    {"kind": "stream_outcome", "outcome": kind},
                )
        return None

    def _settle_turn(
        self,
        record: _TaskRecord,
        reason_kind: str,
        finish_reason: Optional[str],
        final_text: str,
    ) -> None:
        if record.text_len == 0 and final_text:
            # 有的路径不给增量帧；用最终消息补齐，但如实标注不是流式得到的。
            with self._lock:
                record.text_parts.append(final_text)
                record.text_len += len(final_text)
            self._emit(
                record,
                EventType.TEXT_DELTA,
                {"text": final_text, "chars": len(final_text), "streamed": False},
            )
        truncated = str(finish_reason or "") in {"max-tokens", "max_tokens", "length"}
        if reason_kind == "completed":
            self._settle(
                record,
                TaskState.SUCCEEDED,
                "engine_terminal",
                note=(
                    "turn/end=completed"
                    + ("；输出被 max tokens 截断" if truncated else "")
                ),
                stop_reason=finish_reason or "completed",
                truncated=truncated,
            )
        elif reason_kind == "aborted":
            self._settle(
                record,
                TaskState.CANCELLED,
                "engine_terminal",
                note="turn/end=aborted（取消已由引擎确认）",
                stop_reason=finish_reason or "aborted",
            )
        elif reason_kind in ("error", "failed"):
            # DSH 的 TurnEndReasonMap 里失败态是 `error`（附 LlmFailure）；`failed`
            # 仅作兼容旧探针记录的别名，二者都按失败落定，绝不当作完成。
            self._settle(
                record,
                TaskState.FAILED,
                "engine_terminal",
                note="turn/end=" + reason_kind,
                stop_reason=finish_reason or reason_kind,
                error_code=EngineErrorCode.TASK_FAILED.value,
                error_message="engine reported turn/end=" + reason_kind,
            )
        elif reason_kind == "max-tokens":
            self._settle(
                record,
                TaskState.SUCCEEDED,
                "engine_terminal",
                note="turn/end=max-tokens（输出达到上限，按部分结果交付）",
                stop_reason=finish_reason or "max-tokens",
                truncated=True,
            )
        elif reason_kind == "interrupted":
            self._settle(
                record,
                TaskState.INTERRUPTED,
                "engine_terminal",
                note="turn/end=interrupted（引擎侧记录为中断；未自动重放）",
                stop_reason=reason_kind,
            )
        else:
            self._settle(
                record,
                TaskState.UNKNOWN,
                "engine_terminal",
                note="未映射的 turn/end reason="
                + (reason_kind or "<empty>")
                + "；不猜测为成功（blocked 等需要人工处置）",
                stop_reason=reason_kind or None,
            )

    # -- 4.6 cancel_task ----------------------------------------------------
    # -- 审批（OD-ARCH-066）-------------------------------------------------
    def _ensure_approval_channel(self) -> Optional[_ApprovalChannel]:
        """确保常驻审批订阅已就绪；失败只记录，不影响任务提交。"""
        with self._lock:
            channel = self._approval_channel
            if channel is None:
                channel = _ApprovalChannel(self)
                self._approval_channel = channel
        try:
            channel.ensure_started()
        except EngineError as error:
            # 审批通道打不开时，任务仍可跑；界面会缺少审批卡片，须在状态里体现。
            channel.last_error = error.code.value
            return None
        return channel

    def _on_engine_ask(self, value: Mapping[str, Any]) -> None:
        """OD-ARCH-069：非 approval 类的 waterfall 询问，转成可见进度，不自动应答。

        安全边界：这里**只上报**，不发任何应答帧。未知询问继续等待引擎超时或用户取消，
        但界面不再是零反馈。事件里只带类型名与归属会话，不带询问正文（可能含业务内容）。
        """
        event_type = str(value.get("event") or "")
        if not event_type:
            return
        event_id = str(value.get("eventId") or "")
        agent_id = str(value.get("agentId") or "")
        with self._lock:
            record = self._sessions.get(agent_id)
            if record is None or record.state in TERMINAL_STATES:
                return
            # 同一询问可能重复推送：按 (任务, 事件id或类型) 去重，避免刷屏。
            key = (record.omni_task_id, event_id or event_type)
            if key in self._engine_ask_seen:
                return
            self._engine_ask_seen[key] = True
        self._emit(
            record,
            EventType.PROGRESS,
            {
                "kind": "engine_ask",
                "event": event_type,
                "session_id": agent_id,
                "note": "引擎在等待一项未被识别的确认；未自动应答",
            },
        )
        self._persist(record)

    def _on_approval_request(self, value: Mapping[str, Any]) -> None:
        """把 DSH 审批请求归一化成 PROGRESS 事件；找不到归属时如实标注。"""
        event_id = str(value.get("eventId") or "")
        request = value.get("request")
        if not event_id or not isinstance(request, Mapping):
            return
        agent_id = str(value.get("agentId") or "")
        with self._lock:
            record = self._sessions.get(agent_id)
            if record is not None and record.state in TERMINAL_STATES:
                record = None
            if event_id in self._approval_index:
                return
            tool_name = str(request.get("toolName") or "")
            call_id = str(request.get("callId") or "")
            reason = str(request.get("reason") or "")
            entry = {
                "omni_task_id": record.omni_task_id if record is not None else None,
                "tool_name": tool_name,
                "call_id": call_id,
                "reason": reason,
                "session_id": agent_id,
            }
            self._approval_index[event_id] = entry
            if record is not None:
                record.pending_approvals[event_id] = dict(entry)
        if record is None:
            return
        self._emit(
            record,
            EventType.PROGRESS,
            {
                "kind": "approval_request",
                "approval_id": event_id,
                "tool_name": tool_name,
                "call_id": call_id,
                "reason": reason,
                "session_id": agent_id,
            },
        )
        self._persist(record)

    # -- 结构化提问（OD-ARCH-088）------------------------------------------
    def _on_question_request(self, value: Mapping[str, Any]) -> bool:
        """登记 `user-questions/request` 的待答项并下发 `question_request`。

        返回 True 表示这是已识别的结构化提问（无论能否回答都不再退回通用
        engine_ask 提示）；返回 False 表示题目结构无法解析，交给调用方按未知
        waterfall 如实提示。题干与答案**只留在内存事件流**，不写状态文件或日志。
        """
        event_id = str(value.get("eventId") or "")
        request = value.get("request")
        if not event_id or not isinstance(request, Mapping):
            return False
        raw_questions = request.get("questions")
        if not isinstance(raw_questions, list) or not raw_questions:
            return False
        questions: list = []
        violations: list = []
        seen_ids: set = set()
        for raw in raw_questions:
            if not isinstance(raw, Mapping):
                return False
            question_id = raw.get("id")
            text = raw.get("question")
            if not isinstance(question_id, str) or not question_id or not isinstance(text, str):
                return False
            if question_id in seen_ids:
                # 同一次请求里 id 重复，答案无法无歧义回填：按未知询问提示。
                return False
            seen_ids.add(question_id)
            options: list = []
            raw_options = raw.get("options")
            if raw_options is not None:
                if not isinstance(raw_options, list):
                    violations.append("options_not_a_list")
                else:
                    for option in raw_options:
                        if not isinstance(option, Mapping):
                            violations.append("option_not_an_object")
                            continue
                        label = option.get("label")
                        if not isinstance(label, str) or not label:
                            violations.append("option_label_missing")
                            continue
                        description = option.get("description")
                        if description is not None and not isinstance(description, str):
                            violations.append("option_description_not_text")
                            description = None
                        item = {"label": label}
                        if description is not None:
                            item["description"] = description
                        options.append(item)
            multi_raw = raw.get("multi_select", raw.get("multiSelect", False))
            multi = multi_raw is True
            header = raw.get("header")
            detail = raw.get("detail")
            if header is not None and not isinstance(header, str):
                violations.append("header_not_text")
                header = None
            if detail is not None and not isinstance(detail, str):
                violations.append("detail_not_text")
                detail = None
            normalized = {
                "id": question_id,
                "question": text,
                "multi_select": multi,
                "options": options,
            }
            if header is not None:
                normalized["header"] = header
            if detail is not None:
                normalized["detail"] = detail
            questions.append(normalized)
            if len(text) > self._MAX_QUESTION_CHARS:
                violations.append("question_too_long")
            if detail is not None and len(detail) > self._MAX_QUESTION_CHARS:
                violations.append("detail_too_long")
            if len(options) > self._MAX_OPTIONS:
                violations.append("too_many_options")
            if any(len(o["label"]) > self._MAX_OPTION_LABEL_CHARS for o in options):
                violations.append("option_label_too_long")
        if len(questions) > self._MAX_QUESTIONS:
            violations.append("too_many_questions")
        violations = list(dict.fromkeys(violations))
        agent_id = str(value.get("agentId") or "")
        wait = request.get("wait")
        call_id = ""
        if isinstance(wait, Mapping):
            call_id = str(wait.get("callId") or "")
        with self._lock:
            record = self._sessions.get(agent_id)
            if record is not None and record.state in TERMINAL_STATES:
                record = None
            if event_id in self._question_index:
                return True
            entry = {
                "omni_task_id": record.omni_task_id if record is not None else None,
                "session_id": agent_id,
                "call_id": call_id,
                "questions": questions,
                "answerable": not violations,
                "violations": violations,
            }
            self._question_index[event_id] = entry
            if record is not None:
                self._questions_by_task.setdefault(record.omni_task_id, {})[event_id] = entry
        if record is None:
            return True
        note = (
            "引擎在等待回答；请选择后提交"
            if not violations
            else "题目超出可答边界，本卡不可提交；请取消本次任务或让引擎改用更短的提问"
        )
        self._emit(
            record,
            EventType.PROGRESS,
            {
                "kind": "question_request",
                "question_id": event_id,
                "call_id": call_id,
                "session_id": agent_id,
                "questions": questions,
                "answerable": not violations,
                "violations": violations,
                "note": note,
            },
        )
        self._persist(record)
        return True

    def _on_question_cancel(self, question_id: str) -> None:
        if not question_id:
            return
        with self._lock:
            entry = self._question_index.pop(question_id, None)
            record = self._sessions.get(str((entry or {}).get("session_id") or ""))
            if entry is not None and entry.get("omni_task_id"):
                self._questions_by_task.get(str(entry["omni_task_id"]), {}).pop(question_id, None)
        if record is None or entry is None:
            return
        self._emit(
            record,
            EventType.PROGRESS,
            {
                "kind": "question_closed",
                "question_id": question_id,
                "reason": "cancelled",
                "note": "引擎已撤回该提问（可能已超时）；未提交任何答案",
            },
        )
        self._persist(record)

    @staticmethod
    def _question_answer_error(message: str, **detail: Any) -> EngineError:
        return EngineError(
            EngineErrorCode.INVALID_ARGUMENT,
            message,
            detail={"channel": "web-api", **detail},
        )

    def _validate_question_answers(self, entry: Mapping[str, Any], answers: Any) -> list:
        """校验并归一化答案批次；非法一律报错，不静默修正。"""
        if not isinstance(answers, list):
            raise self._question_answer_error("answers must be a list")
        if entry.get("violations"):
            raise self._question_answer_error(
                "question request exceeds answerable limits",
                violations=list(entry.get("violations") or []),
            )
        questions = list(entry.get("questions") or [])
        by_id = {str(q.get("id") or ""): q for q in questions}
        seen: set = set()
        normalized: list = []
        for raw in answers:
            if not isinstance(raw, Mapping):
                raise self._question_answer_error("each answer must be an object")
            extra = set(raw.keys()) - {"id", "selected", "custom"}
            if extra:
                raise self._question_answer_error(
                    "answer contains unknown fields", fields=sorted(str(k) for k in extra)
                )
            answer_id = raw.get("id")
            if not isinstance(answer_id, str) or not answer_id:
                raise self._question_answer_error("answer id is required")
            if answer_id not in by_id:
                raise self._question_answer_error("answer id does not belong to this question")
            if answer_id in seen:
                raise self._question_answer_error("answer id is repeated")
            seen.add(answer_id)
            selected = raw.get("selected")
            if not isinstance(selected, list) or not all(isinstance(v, str) for v in selected):
                raise self._question_answer_error("selected must be a list of strings")
            question = by_id[answer_id]
            labels = [str(o.get("label") or "") for o in question.get("options") or []]
            allowed = set(labels)
            for value in selected:
                if value not in allowed:
                    raise self._question_answer_error(
                        "selected value is not one of this question's options",
                        question_id=answer_id,
                    )
            if len(set(selected)) != len(selected):
                raise self._question_answer_error("selected contains duplicate values",
                                                  question_id=answer_id)
            if not question.get("multi_select") and len(selected) > 1:
                raise self._question_answer_error(
                    "single-select question accepts at most one selected value",
                    question_id=answer_id,
                )
            custom = raw.get("custom")
            if custom is not None:
                if not isinstance(custom, str):
                    raise self._question_answer_error("custom must be text", question_id=answer_id)
                if len(custom) > self._MAX_CUSTOM_CHARS:
                    raise self._question_answer_error(
                        "custom answer is too long",
                        question_id=answer_id,
                        limit=self._MAX_CUSTOM_CHARS,
                    )
            if not selected and not (isinstance(custom, str) and custom.strip()):
                raise self._question_answer_error(
                    "each question needs a selected option or custom text",
                    question_id=answer_id,
                )
            item: dict = {"id": answer_id, "selected": list(selected)}
            if isinstance(custom, str):
                item["custom"] = custom
            normalized.append(item)
        if len(normalized) != len(questions) or seen != set(by_id):
            raise self._question_answer_error(
                "answers must name each question of this request exactly once"
            )
        return normalized

    def respond_question(
        self, omni_task_id: str, question_id: str, answers: Any
    ) -> Mapping[str, Any]:
        """回传用户对结构化提问的答案；只有宿主确认送达才出队。"""
        record = self._require_task(omni_task_id)
        with self._lock:
            if record.state in TERMINAL_STATES:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "task is already terminal; question can no longer be answered",
                    omni_task_id=omni_task_id,
                )
            entry = self._question_index.get(question_id)
            if entry is None or entry.get("omni_task_id") != omni_task_id:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "question_id is not pending for this task",
                    detail={"channel": "web-api"},
                    omni_task_id=omni_task_id,
                )
            snapshot = dict(entry)
        normalized = self._validate_question_answers(snapshot, answers)
        channel = self._ensure_approval_channel()
        if channel is None:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "question channel is not available",
                detail={"channel": "web-api", "last_error": str(
                    self._approval_channel.last_error if self._approval_channel else ""
                )},
                omni_task_id=omni_task_id,
            )
        try:
            client_id = channel.ensure_started()
            self._rpc(
                "$events/result",
                {
                    "clientId": client_id,
                    "eventId": question_id,
                    "outcome": {"kind": "result", "value": {"answers": normalized}},
                },
                self._session_timeout,
                omni_task_id=omni_task_id,
            )
        except EngineError:
            # 保留待答：界面不得假装已送达，用户可以重试。
            raise
        with self._lock:
            current = self._question_index.get(question_id)
            if current is not None and current.get("omni_task_id") == omni_task_id:
                self._question_index.pop(question_id, None)
            pending_map = self._questions_by_task.get(omni_task_id) or {}
            pending_map.pop(question_id, None)
            pending = len(pending_map)
            if not pending_map:
                self._questions_by_task.pop(omni_task_id, None)
        self._emit(
            record,
            EventType.PROGRESS,
            {
                "kind": "question_answered",
                "question_id": question_id,
                "pending_questions": pending,
                "note": "答案已送达引擎；等待引擎继续本轮",
            },
        )
        self._persist(record)
        return {
            "omni_task_id": omni_task_id,
            "question_id": question_id,
            "answered_at": now(),
            "pending_questions": pending,
        }

    def _on_approval_cancel(self, event_id: str) -> None:
        if not event_id:
            return
        with self._lock:
            entry = self._approval_index.pop(event_id, None)
            record = self._sessions.get(str((entry or {}).get("session_id") or ""))
            if record is not None:
                record.pending_approvals.pop(event_id, None)
        if record is None or entry is None:
            return
        self._emit(
            record,
            EventType.PROGRESS,
            {"kind": "approval_cancelled", "approval_id": event_id},
        )
        self._persist(record)

    def respond_approval(
        self, omni_task_id: str, approval_id: str, allow: bool
    ) -> Mapping[str, Any]:
        """回传用户对审批请求的决定。只允许答复本任务仍在等待的请求。"""
        record = self._require_task(omni_task_id)
        with self._lock:
            if record.state in TERMINAL_STATES:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "task is already terminal; approval can no longer be answered",
                    omni_task_id=omni_task_id,
                )
            if approval_id not in record.pending_approvals:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "approval_id is not pending for this task",
                    detail={"channel": "web-api"},
                    omni_task_id=omni_task_id,
                )
        channel = self._ensure_approval_channel()
        if channel is None:
            raise EngineError(
                EngineErrorCode.ENGINE_UNAVAILABLE,
                "approval channel is not available",
                detail={"channel": "web-api", "last_error": str(
                    self._approval_channel.last_error if self._approval_channel else ""
                )},
                omni_task_id=omni_task_id,
            )
        try:
            client_id = channel.ensure_started()
            self._rpc(
                "$events/result",
                {
                    "clientId": client_id,
                    "eventId": approval_id,
                    "outcome": {
                        "kind": "result",
                        "value": "allowed-once" if allow else "rejected",
                    },
                },
                self._session_timeout,
                omni_task_id=omni_task_id,
            )
        except EngineError:
            # 保留 pending：用户可重试，界面不得假装已生效。
            raise
        with self._lock:
            record.pending_approvals.pop(approval_id, None)
            self._approval_index.pop(approval_id, None)
            remaining = len(record.pending_approvals)
        decision = "allowed" if allow else "rejected"
        self._emit(
            record,
            EventType.PROGRESS,
            {
                "kind": "approval_decided",
                "approval_id": approval_id,
                "decision": decision,
                "pending_approvals": remaining,
            },
        )
        self._persist(record)
        return {
            "omni_task_id": omni_task_id,
            "approval_id": approval_id,
            "decision": decision,
            "pending_approvals": remaining,
        }

    def cancel_task(self, omni_task_id: str, *, timeout: float = 15.0) -> CancelOutcome:
        record = self._require_task(omni_task_id)
        with self._lock:
            if record.state in TERMINAL_STATES:
                return CancelOutcome(
                    omni_task_id=omni_task_id,
                    phase=CancelPhase.NOT_REQUESTED,
                    requested_at=None,
                    settled_at=now(),
                    final_state=record.state,
                    evidence="already_terminal",
                    note="任务已终态，未发送取消请求",
                )
            session_id = record.engine_session_id
            record.cancel_phase = CancelPhase.REQUEST_SENT
            record.cancel_requested_at = now()
            record.state = TaskState.CANCELLING
            record.state_since = now()
        self._emit(
            record,
            EventType.CANCEL_REQUESTED,
            {
                "normalized": True,
                "note": "session/cancel 无独立 ack；确认以 turn/end=aborted 为准",
            },
        )
        if session_id is None:
            raise EngineError(
                EngineErrorCode.CANCEL_FAILED,
                "no active engine session to cancel",
                omni_task_id=omni_task_id,
            )
        try:
            self._rpc(
                "session/cancel",
                {"request": {"sessionId": session_id}},
                self._session_timeout,
                omni_task_id=omni_task_id,
            )
        except EngineError as error:
            with self._lock:
                record.cancel_phase = CancelPhase.TIMEOUT_UNKNOWN
                record.cancel_settled_at = now()
            raise EngineError(
                EngineErrorCode.CANCEL_FAILED,
                "cancel could not be delivered: " + error.message,
                detail=dict(error.detail),
                omni_task_id=omni_task_id,
            ) from error
        deadline = time.monotonic() + float(timeout)
        while time.monotonic() < deadline:
            with self._lock:
                state = record.state
            if state == TaskState.CANCELLED:
                with self._lock:
                    record.cancel_phase = CancelPhase.CONFIRMED_CANCELLED
                    record.cancel_settled_at = now()
                    requested_at = record.cancel_requested_at
                    settled_at = record.cancel_settled_at
                return CancelOutcome(
                    omni_task_id=omni_task_id,
                    phase=CancelPhase.CONFIRMED_CANCELLED,
                    requested_at=requested_at,
                    settled_at=settled_at,
                    final_state=TaskState.CANCELLED,
                    evidence="engine_terminal",
                    note="引擎以 turn/end=aborted 落定",
                )
            if state in TERMINAL_STATES:
                with self._lock:
                    record.cancel_settled_at = now()
                    requested_at = record.cancel_requested_at
                    settled_at = record.cancel_settled_at
                return CancelOutcome(
                    omni_task_id=omni_task_id,
                    phase=CancelPhase.REQUEST_SENT,
                    requested_at=requested_at,
                    settled_at=settled_at,
                    final_state=state,
                    evidence="engine_terminal",
                    note="取消未被确认，任务已按其它终态落定；保留引擎终态",
                )
            time.sleep(0.02)
        with self._lock:
            state = record.state
            requested_at = record.cancel_requested_at
            if record.cancel_phase == CancelPhase.REQUEST_SENT:
                record.cancel_phase = CancelPhase.TIMEOUT_UNKNOWN
                record.cancel_settled_at = now()
            settled_at = record.cancel_settled_at
        return CancelOutcome(
            omni_task_id=omni_task_id,
            phase=CancelPhase.TIMEOUT_UNKNOWN,
            requested_at=requested_at,
            settled_at=settled_at,
            final_state=state,
            evidence="operator_confirm",
            note="取消请求已发送但未取得可靠终态证据；不得报告为取消成功",
        )

    # -- 4.7 session 读回 ---------------------------------------------------
    def _succeeded_provenance(self, record, *, stop_reason, persist_ok):
        return (
            (
                {"kind": "engine_turn_end", "value": str(stop_reason)},
                {"kind": "engine_stream", "value": "assistant-stream"},
                {"kind": "persist", "value": "ok" if persist_ok else "failed"},
            ),
            "结果由 OmniDesk 从 assistant-stream 的 text-delta 与 assistant/message 拼装",
        )

    def _probe_session(self, session_id: str, workspace_dir: str) -> None:
        self.recover_session(session_id, cwd=workspace_dir or self._cwd)

    def list_sessions(
        self,
        *,
        after_cursor: Optional[str] = None,
        cwd: Optional[str] = None,
        max_pages: int = 20,
    ) -> SessionListing:
        """session/list 只读元数据；不解析 DSH 的会话持久化文件。"""
        filter_cwd: Optional[str] = None
        if cwd is not None:
            filter_cwd = canonical_cwd(cwd)
        cursor = after_cursor
        entries: list[Mapping[str, Any]] = []
        skipped = 0
        pages = 0
        next_cursor: Optional[str] = None
        while pages < max(1, int(max_pages)):
            request: dict[str, Any] = {}
            if cursor:
                request["cursor"] = cursor
            if filter_cwd is not None:
                request["cwd"] = filter_cwd
            try:
                page = self._rpc("session/list", {"_request": request}, self._session_timeout)
            except EngineError as error:
                if error.code == EngineErrorCode.UNSUPPORTED:
                    raise
                raise EngineError(
                    EngineErrorCode.RECOVER_FAILED,
                    "session listing failed: " + error.message,
                    detail=dict(error.detail),
                ) from error
            pages += 1
            if not isinstance(page, Mapping):
                raise EngineError(
                    EngineErrorCode.EVENT_PARSE_ERROR,
                    "session/list returned a non-object value",
                )
            raw_sessions = page.get("items")
            if not isinstance(raw_sessions, list):
                raise EngineError(
                    EngineErrorCode.EVENT_PARSE_ERROR,
                    "session/list response has no items array",
                    detail={"result_keys": sorted(str(k) for k in page.keys())},
                )
            for item in raw_sessions:
                if not isinstance(item, Mapping):
                    skipped += 1
                    continue
                session_id = item.get("sessionId")
                raw_cwd = item.get("cwd")
                if not isinstance(session_id, str) or not session_id:
                    skipped += 1
                    continue
                if not isinstance(raw_cwd, str) or not os.path.isabs(raw_cwd):
                    skipped += 1
                    continue
                updated = item.get("updatedAt")
                entries.append(
                    {
                        "sessionId": session_id,
                        "cwd": os.path.realpath(raw_cwd),
                        "cwdExists": os.path.isdir(raw_cwd),
                        "updatedAt": (float(updated) / 1000.0)
                        if isinstance(updated, (int, float))
                        else None,
                    }
                )
            raw_next = page.get("nextCursor")
            if not raw_next:
                next_cursor = None
                break
            if not isinstance(raw_next, str):
                raise EngineError(
                    EngineErrorCode.EVENT_PARSE_ERROR,
                    "session/list nextCursor is not a string",
                )
            if raw_next == cursor:
                raise EngineError(
                    EngineErrorCode.EVENT_PARSE_ERROR,
                    "session/list pagination did not advance",
                )
            cursor = raw_next
            next_cursor = raw_next
        grouped: dict[str, list[dict[str, Any]]] = {}
        for entry in entries:
            grouped.setdefault(str(entry["cwd"]), []).append(
                {
                    "engine_session_id": entry["sessionId"],
                    "workspace_label": workspace_label_for(str(entry["cwd"]), ordinal=0),
                    "workspace_token": workspace_token_for(str(entry["cwd"])),
                    "created_at": entry["updatedAt"],
                    "workspace_exists": entry["cwdExists"],
                    "metadata_valid": True,
                    # 列表可见 != 可恢复；只有 follow 打开成功才可改变该值。
                    "resume_status": "unverified",
                }
            )
        groups = []
        for ordinal, (key, value) in enumerate(sorted(grouped.items()), start=1):
            label = workspace_label_for(key, ordinal=ordinal)
            token = workspace_token_for(key)
            with self._lock:
                self._workspace_tokens[token] = key
                self._workspace_labels[token] = label
            for session in value:
                session["workspace_label"] = label
                session["workspace_token"] = token
                # 只存不透明 token，绝不把路径交给界面自行拼装。
                session.pop("cwd", None)
            groups.append({"workspace_label": label, "workspace_token": token, "sessions": value})
        groups = tuple(groups)
        return SessionListing(
            groups=groups,
            total=len(entries),
            skipped_invalid=skipped,
            next_cursor=next_cursor,
            note="只含会话元数据；列表可见不代表可恢复，重开结果以 follow 是否打开为准",
        )

    def recover_session(
        self,
        engine_session_id: str,
        *,
        cwd: Optional[str] = None,
        workspace_token: Optional[str] = None,
    ) -> SessionRecovery:
        with self._lock:
            known = self._sessions.get(engine_session_id)
            raw_cwd = cwd or (known.workspace_dir if known is not None else self._cwd)
            if workspace_token:
                resolved = self._workspace_tokens.get(str(workspace_token))
                if resolved is None:
                    raise EngineError(
                        EngineErrorCode.UNKNOWN_SESSION,
                        "unknown workspace token; list sessions again",
                    )
                raw_cwd = resolved
        target_cwd = canonical_cwd(raw_cwd)
        follow: Optional[_Follow] = None
        try:
            follow = self._open_follow(engine_session_id, target_cwd)
            # 读到 snapshot 即证明会话存在；不 prompt、不重放。
            resolved_cwd = target_cwd
        except EngineError as error:
            if error.code == EngineErrorCode.UNKNOWN_SESSION:
                raise
            raise EngineError(
                EngineErrorCode.RECOVER_FAILED,
                "session recovery failed: " + error.message,
                detail=dict(error.detail),
            ) from error
        finally:
            if follow is not None:
                self._drop_follow(follow)
        token = workspace_token_for(resolved_cwd)
        with self._lock:
            self._session_cwd[engine_session_id] = resolved_cwd
            label = self._workspace_labels.get(token, "")
        return SessionRecovery(
            engine_session_id=engine_session_id,
            resumed=True,
            cwd=resolved_cwd,
            # Web API 的 follow 快照不带 ACP 式 configOptions；不伪造数量。
            config_option_count=0,
            evidence="engine_terminal",
            note="Web API 无 session/resume；follow 打开成功即证明会话存在。恢复不触发任务重放；配置项数量在本通道不可观测",
            workspace_token=token,
            workspace_label=label,
        )

    # -- 会话分支 ---------------------------------------------------------
    def _record_turn_end_seq(
        self, record: _TaskRecord, event: Mapping[str, Any]
    ) -> None:
        """记下本轮 turn/end 在会话序列里的精确 seq（DSH session/fork 的切点）。

        只接受非负整数；缺失或字段形态异常时保持原值（默认 None），由界面禁用分支，
        绝不猜一个序号，也绝不退回本地重放冒充原生分叉。
        """
        raw_seq = event.get("seq")
        if isinstance(raw_seq, int) and not isinstance(raw_seq, bool) and raw_seq >= 0:
            with self._lock:
                record.turn_end_seq = raw_seq

    def fork_session(
        self,
        engine_session_id: str,
        *,
        at_seq: Optional[int] = None,
    ) -> SessionFork:
        """按 DSH 原生 session/fork 建立子会话。

        `at_seq` 是**包含在内**的会话事件序号（就是本轮 turn/end 的 seq）。
        省略时由引擎取最近一个完整轮次的边界。

        失败必须如实抛出：不允许在失败后悄悄退回「本地重放」，也不假造子会话号。
        """
        if not engine_session_id:
            raise EngineError(
                EngineErrorCode.INVALID_ARGUMENT, "engine_session_id is required"
            )
        request: dict = {"sessionId": engine_session_id}
        if at_seq is not None:
            if not isinstance(at_seq, int) or isinstance(at_seq, bool) or at_seq < 0:
                raise EngineError(
                    EngineErrorCode.INVALID_ARGUMENT,
                    "at_seq must be a non-negative integer",
                )
            request["atSeq"] = at_seq
        value = self._rpc("session/fork", {"request": request}, self._session_timeout)
        if not isinstance(value, Mapping):
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "session/fork returned a non-object value",
                detail={"channel": "web-api"},
            )
        child_id = value.get("sessionId")
        if not child_id:
            raise EngineError(
                EngineErrorCode.EVENT_PARSE_ERROR,
                "session/fork did not return a session id",
                detail={"result_keys": sorted(str(k) for k in value.keys())},
            )
        child = str(child_id)
        with self._lock:
            source_cwd = self._session_cwd.get(engine_session_id)
            if source_cwd:
                self._session_cwd[child] = source_cwd
        return SessionFork(
            parent_session_id=engine_session_id,
            engine_session_id=child,
            at_seq=at_seq,
            note="已在引擎侧按切点建立子会话；后续消息直接续接该会话，不重放历史",
        )

    # -- 4.8 shutdown -------------------------------------------------------
    def shutdown(
        self,
        *,
        mode: str = "graceful",
        grace_period: float = 5.0,
        cancel_in_flight: bool = False,
        cancel_timeout: Optional[float] = None,
    ) -> ShutdownReport:
        if mode not in ("graceful", "force", "detach"):
            raise EngineError(
                EngineErrorCode.INVALID_ARGUMENT, "unknown shutdown mode: " + str(mode)
            )
        with self._lock:
            in_flight = tuple(
                t.omni_task_id for t in self._tasks.values() if t.state not in TERMINAL_STATES
            )
        cancelled: list = []
        still_running: list = []
        if cancel_in_flight and in_flight:
            for task_id in in_flight:
                try:
                    outcome = self.cancel_task(
                        task_id,
                        timeout=float(
                            cancel_timeout if cancel_timeout is not None else grace_period
                        ),
                    )
                except EngineError:
                    still_running.append(task_id)
                    continue
                if outcome.phase == CancelPhase.CONFIRMED_CANCELLED:
                    cancelled.append(task_id)
                else:
                    still_running.append(task_id)
        with self._lock:
            remaining = tuple(
                t.omni_task_id for t in self._tasks.values() if t.state not in TERMINAL_STATES
            )
            follows = list(self._follow_clients)
            self._follow_clients.clear()
            approval_channel = self._approval_channel
            self._approval_channel = None
            self._approval_index.clear()
            # OD-ARCH-088：关停时丢弃内存里的待答提问（正文不落盘）。
            self._question_index.clear()
            self._questions_by_task.clear()
            proc = self._proc
        for task_id in remaining:
            if task_id not in still_running:
                still_running.append(task_id)
        connection_closed = False
        for follow in follows:
            follow.close()
            connection_closed = True
        if approval_channel is not None:
            approval_channel.close()
            connection_closed = True

        exit_code: Optional[int] = None
        forced = False
        if mode == "detach":
            with self._lock:
                self._closed = True
                self._cookie = None
                self._proc = None
            return ShutdownReport(
                mode=mode,
                in_flight_before=in_flight,
                cancelled=tuple(cancelled),
                still_running=tuple(still_running),
                connection_closed=connection_closed,
                process_exit_code=None,
                forced_terminate=False,
                note="已关闭事件流并放弃控制句柄；DSH web 进程保持运行（不重放任务）",
            )
        if proc is not None:
            exit_code = proc.poll()
            if mode == "force":
                if exit_code is None:
                    try:
                        proc.kill()
                    except OSError:
                        pass
                    forced = True
                try:
                    exit_code = proc.wait(timeout=5.0)
                except (OSError, subprocess.TimeoutExpired):
                    pass
            else:
                if exit_code is None:
                    try:
                        proc.terminate()
                        exit_code = proc.wait(timeout=max(1.0, float(grace_period)))
                    except subprocess.TimeoutExpired:
                        pass
                    except OSError:
                        pass
                # 无论节点是否已退出，都回收它独占的进程组（空组为无操作）。
                if self._kill_child_group(3.0):
                    forced = True
                if proc.poll() is None:
                    forced = True
                    try:
                        proc.kill()
                        proc.wait(timeout=5.0)
                    except (OSError, subprocess.TimeoutExpired):
                        pass
                exit_code = proc.poll() if exit_code is None else exit_code
        with self._lock:
            self._closed = True
            self._cookie = None
            self._token = None
        note = "已关闭事件流并终止隔离的 DSH web 进程"
        if forced:
            note = "宽限期内未结束，已强制终止；未落盘输出可能丢失"
        return ShutdownReport(
            mode=mode,
            in_flight_before=in_flight,
            cancelled=tuple(cancelled),
            still_running=tuple(still_running),
            connection_closed=connection_closed,
            process_exit_code=exit_code,
            forced_terminate=forced,
            note=note,
        )

    # -- 观测接口（证据采集用，不属于契约）---------------------------------
    def engine_exit_code(self) -> Optional[int]:
        proc = self._proc
        return proc.poll() if proc is not None else None

    def engine_stderr(self) -> str:
        # 子进程 stdout 与 stderr 合并；这里只给已脱敏的启动行摘要。
        return "\n".join(self._startup_lines[-20:])

    def follow_count(self) -> int:
        with self._lock:
            return len(self._follow_clients)
