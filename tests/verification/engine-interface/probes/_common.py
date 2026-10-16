"""
OmniDesk P1 引擎接口审计 —— 探测器公共工具。

安全边界（本目录所有脚本共同遵守）：
  * 只读探测：不写任何配置文件，不改 Profile，不碰凭证。
  * 全部 DSH 调用都强制 DSH_HOME / DSH_AGENTS_HOME 指向 /tmp 下的一次性目录，
    因此即使底层写入 Session，也只落在临时目录，不进 ~/OmniDesk、~/.dsh 或仓库。
  * 不启动 web profile，不绑定端口，不读取或打印凭证内容。
  * 探测只发送合成占位文本（"ping" 等），不含任何真实人员、机构或案件信息。
  * 无凭证时底层会返回 MISSING_CREDENTIAL；这是预期结果，脚本如实记录，不修改凭证/路由。

证据等级（写入结果文件时逐项标注）：
  static / script / model / unverified
"""
import json, os, subprocess, threading, time
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
DSH_BIN = REPO / "runtime" / "dsh-runtime" / "node_modules" / "@deepseek-ai" / "dsh" / "lib" / "bin.js"
# 每次运行使用各自独立的临时数据根，保证脚本可重复执行：
# 固定数据根会残留上一次的 Session，导致同一会话名二次提交被拒（假象），
# 也会让“首次提交”的观测被历史状态污染。
AUDIT_RUN = f"omni_p1_audit_{os.getpid()}"
AUDIT_HOME = Path("/tmp") / AUDIT_RUN / "dsh-home"
AUDIT_AGENTS = Path("/tmp") / AUDIT_RUN / "agents"


def audit_env():
    env = dict(os.environ)
    env["DSH_HOME"] = str(AUDIT_HOME)
    env["DSH_AGENTS_HOME"] = str(AUDIT_AGENTS)
    AUDIT_HOME.mkdir(parents=True, exist_ok=True)
    AUDIT_AGENTS.mkdir(parents=True, exist_ok=True)
    return env


def one_shot(profile, args, cwd, timeout=120, stdin_text=None):
    """运行一次 dsh <profile> 命令，返回退出码、stdout、stderr。"""
    cmd = ["node", str(DSH_BIN), "--profile", profile, *args]
    p = subprocess.run(cmd, cwd=str(cwd), env=audit_env(), capture_output=True,
                       text=True, input=stdin_text, timeout=timeout)
    return {"cmd": cmd, "rc": p.returncode, "stdout": p.stdout, "stderr": p.stderr}


class JsonRpcPeer:
    """以 newline-delimited JSON-RPC 与 stdio profile 对话的最小客户端。"""

    def __init__(self, profile, cwd, timeout_env=None):
        self.cmd = ["node", str(DSH_BIN), "--profile", profile]
        env = audit_env()
        if timeout_env:
            env.update(timeout_env)
        self.proc = subprocess.Popen(self.cmd, cwd=str(cwd), env=env,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True, bufsize=1)
        self.frames = []
        self._lock = threading.Lock()
        self._t = threading.Thread(target=self._reader, daemon=True)
        self._t.start()
        self.stderr_text = ""

    def _reader(self):
        for line in self.proc.stdout:
            line = line.rstrip("\n")
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception:
                obj = {"_unparsed": line}
            with self._lock:
                self.frames.append(obj)

    def send(self, obj):
        if self.proc.poll() is not None:
            raise RuntimeError(f"engine process already exited (rc={self.proc.returncode}); frame not sent")
        try:
            self.proc.stdin.write(json.dumps(obj) + "\n")
            self.proc.stdin.flush()
        except (BrokenPipeError, ValueError) as exc:
            raise RuntimeError(f"engine stdin closed mid-conversation: {exc}") from exc

    def request(self, req_id, method, params=None):
        self.send({"jsonrpc": "2.0", "id": req_id, "method": method, "params": params or {}})

    def notify(self, method, params=None):
        self.send({"jsonrpc": "2.0", "method": method, "params": params or {}})

    def wait_response(self, req_id, timeout=20):
        end = time.time() + timeout
        while time.time() < end:
            with self._lock:
                for f in self.frames:
                    if f.get("id") == req_id:
                        return f
            time.sleep(0.05)
        return None

    def wait_for(self, predicate, timeout=20):
        end = time.time() + timeout
        while time.time() < end:
            with self._lock:
                for f in list(self.frames):
                    if predicate(f):
                        return f
            time.sleep(0.05)
        return None

    def notifications(self):
        with self._lock:
            return [f for f in self.frames if "method" in f and "id" not in f]

    def all_frames(self):
        with self._lock:
            return list(self.frames)

    def close(self, grace=10):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            out, err = self.proc.communicate(timeout=grace)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            out, err = self.proc.communicate()
        self.stderr_text = err or ""
        return self.proc.returncode

    def hard_kill(self):
        self.proc.kill()
        self.proc.wait()


def write_json(path, payload):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return path
