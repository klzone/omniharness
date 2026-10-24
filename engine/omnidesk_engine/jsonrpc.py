"""最小 JSON-RPC 2.0 over stdio 传输（OmniDesk 引擎适配层内部使用）。

职责边界：只负责「把请求帧写出去、把响应/通知帧收回来」，不做任何 ACP 语义映射。
所有 ACP 语义（方法名、参数、事件归一化）都留在 acp_adapter.py。

约束：
  * 只依赖 Python 标准库。
  * 读线程持续消费 stdout；进程退出后调用方必须能区分「正常关闭」与「异常中断」。
  * 不解析、不打印任何凭证内容（本层只看到协议帧）。
"""

from __future__ import annotations

import json
import subprocess
import threading
import time
from typing import Any, Callable, Mapping, Optional, Sequence

from .process_owner import terminate_group


class PeerExit(RuntimeError):
    """引擎进程在预期之外退出（管道断开）。"""


class JsonRpcPeer:
    """以换行分隔的 JSON-RPC 与子进程对话的最小客户端。

    * request(id, method, params) 只负责写出，不阻塞等待；
    * wait_response(id, timeout) 轮询读取结果；
    * 所有收到/发出的帧都保留在 self.frames 中，作为原始证据。
    """

    def __init__(
        self,
        cmd: Sequence[str],
        *,
        cwd: str,
        env: Optional[Mapping[str, str]] = None,
        on_frame: Optional[Callable[[Mapping[str, Any]], None]] = None,
    ) -> None:
        self.cmd = list(cmd)
        self.cwd = cwd
        self.frames: list[Mapping[str, Any]] = []
        self.sent: list[Mapping[str, Any]] = []
        self.stderr_chunks: list[str] = []
        self.exited = False
        self.exit_code: Optional[int] = None
        self._on_frame = on_frame
        self._lock = threading.Lock()
        self._write_lock = threading.Lock()
        self.proc = subprocess.Popen(
            self.cmd,
            cwd=cwd,
            env=dict(env) if env is not None else None,
            # OD-ARCH-074：DSH 独占进程组，崩溃后按登记的可信组长回收整棵树。
            start_new_session=True,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self._t_out = threading.Thread(target=self._read_stdout, daemon=True)
        self._t_err = threading.Thread(target=self._read_stderr, daemon=True)
        self._t_out.start()
        self._t_err.start()

    # -- 读侧 ---------------------------------------------------------------
    def _read_stdout(self) -> None:
        assert self.proc.stdout is not None
        for line in self.proc.stdout:
            line = line.rstrip("\n")
            if not line:
                continue
            try:
                obj: Mapping[str, Any] = json.loads(line)
            except Exception:
                obj = {"_unparsed": line}
            with self._lock:
                self.frames.append(obj)
            if self._on_frame is not None:
                try:
                    self._on_frame(obj)
                except Exception:
                    # 回调异常不得影响读线程；由上层自行记录。
                    pass
        self._mark_exited()

    def _read_stderr(self) -> None:
        assert self.proc.stderr is not None
        for line in self.proc.stderr:
            with self._lock:
                self.stderr_chunks.append(line)

    def _mark_exited(self) -> None:
        code = self.proc.poll()
        if code is None:
            try:
                code = self.proc.wait(timeout=2)
            except Exception:
                code = None
        with self._lock:
            self.exited = True
            if self.exit_code is None:
                self.exit_code = code

    # -- 写侧 ---------------------------------------------------------------
    def send(self, obj: Mapping[str, Any]) -> None:
        if self.proc.poll() is not None:
            raise PeerExit(
                f"engine process already exited (rc={self.proc.returncode}); frame not sent"
            )
        payload = json.dumps(obj, ensure_ascii=False)
        with self._write_lock:
            try:
                assert self.proc.stdin is not None
                self.proc.stdin.write(payload + "\n")
                self.proc.stdin.flush()
            except (BrokenPipeError, ValueError) as exc:
                self._mark_exited()
                raise PeerExit(f"engine stdin closed mid-conversation: {exc}") from exc
            with self._lock:
                self.sent.append(obj)

    def request(self, req_id: Any, method: str, params: Optional[Mapping[str, Any]] = None) -> None:
        self.send({"jsonrpc": "2.0", "id": req_id, "method": method, "params": dict(params or {})})

    def notify(self, method: str, params: Optional[Mapping[str, Any]] = None) -> None:
        self.send({"jsonrpc": "2.0", "method": method, "params": dict(params or {})})

    def respond(self, req_id: Any, result: Optional[Mapping[str, Any]] = None) -> None:
        self.send({"jsonrpc": "2.0", "id": req_id, "result": dict(result or {})})

    def respond_error(self, req_id: Any, code: int, message: str) -> None:
        self.send({"jsonrpc": "2.0", "id": req_id, "error": {"code": code, "message": message}})

    # -- 读缓冲查询 ---------------------------------------------------------
    def snapshot(self) -> list[Mapping[str, Any]]:
        with self._lock:
            return list(self.frames)

    def find_response(self, req_id: Any) -> Optional[Mapping[str, Any]]:
        with self._lock:
            for f in self.frames:
                if f.get("id") == req_id and ("result" in f or "error" in f):
                    return f
        return None

    def wait_response(
        self, req_id: Any, timeout: Optional[float]
    ) -> Optional[Mapping[str, Any]]:
        """等响应帧。timeout=None 表示一直等到收到响应或子进程退出。

        任务型调用（session/prompt）用 None：只要引擎还在跑就不能按墙钟放弃；
        握手/列举这类有界调用仍传具体秒数。
        """
        end = None if timeout is None else time.monotonic() + float(timeout)
        while end is None or time.monotonic() < end:
            found = self.find_response(req_id)
            if found is not None:
                return found
            if self.proc.poll() is not None and self.find_response(req_id) is None:
                # 给读线程一点时间把最后一帧刷出来
                time.sleep(0.05)
                return self.find_response(req_id)
            time.sleep(0.02)
        return self.find_response(req_id)

    def notifications(self) -> list[Mapping[str, Any]]:
        with self._lock:
            return [f for f in self.frames if "method" in f and "id" not in f]

    def pending_requests_from_engine(self) -> list[Mapping[str, Any]]:
        """引擎发起的、需要客户端回应的请求帧（id + method 均有，且非我们的请求 id）。"""
        with self._lock:
            return [
                f for f in self.frames
                if "method" in f and "id" in f and isinstance(f.get("id"), str)
            ]

    def stderr_text(self) -> str:
        with self._lock:
            return "".join(self.stderr_chunks)

    # -- 生命周期 -----------------------------------------------------------
    def close_stdin(self) -> None:
        try:
            if self.proc.stdin is not None:
                self.proc.stdin.close()
        except Exception:
            pass

    def wait_exit(self, timeout: float) -> Optional[int]:
        try:
            code = self.proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            return None
        self._mark_exited()
        return code

    def terminate(self, grace: float = 5.0) -> Optional[int]:
        """先关 stdin（请求优雅退出），超时后按进程组 kill。返回退出码或 None。"""
        self.close_stdin()
        code = self.wait_exit(grace)
        self.kill_group(grace)
        if code is None:
            code = self.wait_exit(grace)
        self._mark_exited()
        return code

    def kill_group(self, grace: float = 5.0) -> bool:
        """回收子进程独占的进程组；身份不可核验时只跳过，不误杀。"""
        try:
            return terminate_group(self.proc.pid, grace=grace, trusted_leader=self.proc.pid)
        except Exception:
            return False

    def hard_kill(self) -> None:
        """模拟客户端强制退出：不给引擎任何收尾机会。"""
        self.kill_group(0.5)
        if self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait()
        with self._lock:
            self.exited = True
            self.exit_code = self.proc.returncode

