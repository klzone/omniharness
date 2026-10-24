"""引擎进程所有权登记与进程组回收（OD-ARCH-074）。

唯一事实源是引擎状态目录下的 ``owner.json``。它只记录本机进程身份，不含
任何凭证、会话内容或用户数据。回收前必须同时核验：

  * 记录里的 pid 仍存活，且 /proc/<pid>/stat 的进程组号与记录一致；
  * 命令行与记录完全一致（防同机 pid 复用）；
  * boot_id 与当前内核一致（防跨重启 pid 复用）；
  * 进程组的每个成员都是 OmniDesk 引擎或它启动的 node DSH。

任何一项不满足都不杀进程：只清理已经死掉的登记，或拒绝启动并给出原因。
本模块只依赖 Python 标准库。
"""

from __future__ import annotations

import json
import os
import signal
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Iterable, Optional

OWNER_FILENAME = "owner.json"
_BOOT_ID_PATH = "/proc/sys/kernel/random/boot_id"


class OwnerConflict(RuntimeError):
    """同一数据根已有活跃引擎，或登记无法安全核验。"""


@dataclass
class OwnerRecord:
    pid: int
    pgid: int
    host_pid: int
    boot_id: str
    channel: str
    started_at: float
    cmdline: list[str] = field(default_factory=list)
    state_dir: str = ""
    #: 引擎拉起的 DSH 子进程（独立进程组）。崩溃后按它回收整棵子树。
    child_pid: int = 0
    child_pgid: int = 0

    def to_dict(self) -> dict:
        return asdict(self)


@dataclass
class OwnerInspection:
    #: none | active_engine | orphan_group | pid_reused | unknown_live | invalid
    status: str
    record: Optional[OwnerRecord] = None
    reason: str = ""
    group_members: tuple[tuple[int, tuple[str, ...]], ...] = ()

    @property
    def file_present(self) -> bool:
        return self.status not in ("none",)


def boot_id() -> str:
    try:
        return Path(_BOOT_ID_PATH).read_text(encoding="ascii").strip()
    except OSError:
        return ""


def owner_path(state_dir: str | os.PathLike[str]) -> Path:
    return Path(state_dir) / OWNER_FILENAME


def _proc_stat(pid: int) -> Optional[tuple[str, int]]:
    """返回 (进程状态, 进程组号)；进程不存在或不可读时返回 None。"""
    try:
        raw = Path("/proc/%d/stat" % pid).read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    # comm 可能包含空格与括号，必须从最后一个 ')' 之后切分。
    end = raw.rfind(")")
    if end < 0:
        return None
    fields = raw[end + 2 :].split()
    if len(fields) < 3:
        return None
    try:
        return fields[0], int(fields[2])
    except ValueError:
        return None


def _proc_cmdline(pid: int) -> list[str]:
    try:
        raw = Path("/proc/%d/cmdline" % pid).read_bytes()
    except OSError:
        return []
    return [part.decode("utf-8", "replace") for part in raw.split(b"\0") if part]


def is_alive(pid: int) -> bool:
    stat = _proc_stat(pid)
    return stat is not None and stat[0] != "Z"


def is_engine_cmdline(cmdline: Iterable[str]) -> bool:
    args = list(cmdline)
    if any("omnidesk_engine" in arg for arg in args):
        return True
    exe = args[0].rsplit("/", 1)[-1] if args else ""
    return exe.startswith("node") and any(arg.endswith("bin.js") for arg in args[1:])


def group_members(pgid: int) -> list[tuple[int, list[str]]]:
    if pgid <= 1:
        return []
    members: list[tuple[int, list[str]]] = []
    try:
        entries = os.listdir("/proc")
    except OSError:
        return members
    for name in entries:
        if not name.isdigit():
            continue
        pid = int(name)
        stat = _proc_stat(pid)
        if stat is None or stat[0] == "Z" or stat[1] != pgid:
            continue
        members.append((pid, _proc_cmdline(pid)))
    return members


def group_is_ours(pgid: int) -> bool:
    members = group_members(pgid)
    return bool(members) and all(is_engine_cmdline(cmd) for _, cmd in members)


def child_group_is_ours(pid: int, pgid: int) -> bool:
    """子进程组的可信判据：组长就是当初由本引擎亲手拉起、独占会话的那个 pid。"""
    if pid <= 1 or pgid != pid:
        return False
    stat = _proc_stat(pid)
    if stat is None or stat[0] == "Z" or stat[1] != pgid:
        return False
    return is_engine_cmdline(_proc_cmdline(pid))


def read_owner(state_dir: str | os.PathLike[str]) -> Optional[OwnerRecord]:
    path = owner_path(state_dir)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    if not isinstance(data, dict):
        return None
    try:
        cmdline = data.get("cmdline") or []
        if not isinstance(cmdline, list) or not all(isinstance(x, str) for x in cmdline):
            return None
        return OwnerRecord(
            pid=int(data["pid"]),
            pgid=int(data["pgid"]),
            host_pid=int(data.get("host_pid") or 0),
            boot_id=str(data.get("boot_id") or ""),
            channel=str(data.get("channel") or ""),
            started_at=float(data.get("started_at") or 0.0),
            cmdline=cmdline,
            state_dir=str(data.get("state_dir") or ""),
            child_pid=int(data.get("child_pid") or 0),
            child_pgid=int(data.get("child_pgid") or 0),
        )
    except (KeyError, TypeError, ValueError):
        return None


def inspect_owner(state_dir: str | os.PathLike[str]) -> OwnerInspection:
    path = owner_path(state_dir)
    if not path.exists():
        return OwnerInspection("none")
    record = read_owner(state_dir)
    if record is None:
        return OwnerInspection("invalid", reason="owner.json 无法解析")

    members = tuple((pid, tuple(cmd)) for pid, cmd in group_members(record.pgid))
    current_boot = boot_id()

    if is_alive(record.pid):
        stat = _proc_stat(record.pid)
        actual_cmd = _proc_cmdline(record.pid)
        exact = (
            stat is not None
            and stat[1] == record.pgid
            and actual_cmd == record.cmdline
            and bool(current_boot)
            and record.boot_id == current_boot
        )
        if exact:
            return OwnerInspection("active_engine", record, "登记进程仍存活且身份一致", members)
        if is_engine_cmdline(actual_cmd) or any(is_engine_cmdline(c) for _, c in members):
            return OwnerInspection(
                "unknown_live",
                record,
                "同数据根存在无法核验身份的引擎进程；拒绝启动，不杀进程",
                members,
            )
        return OwnerInspection("pid_reused", record, "pid 已被无关进程复用", members)

    # 引擎已退出：只有同一 boot 且组里全是我们的进程时才可回收。
    if record.boot_id == current_boot:
        child = tuple(
            (pid, tuple(cmd))
            for pid, cmd in group_members(record.child_pgid)
        ) if record.child_pgid > 1 and record.child_pgid != record.pgid else ()
        if members or child:
            if all(is_engine_cmdline(cmd) for _, cmd in members + child):
                return OwnerInspection(
                    "orphan_group",
                    record,
                    "引擎已退出，进程组仍有遗留子进程",
                    members + child,
                )
            return OwnerInspection(
                "unknown_live",
                record,
                "进程组包含非 OmniDesk 进程；拒绝回收",
                members + child,
            )
    return OwnerInspection("pid_reused", record, "登记所属的内核启动已结束", members)


def ensure_process_group() -> None:
    """让本进程成为会话/进程组组长；已是组长时保持不变。"""
    if os.name != "posix":
        return
    try:
        if os.getpgrp() != os.getpid():
            os.setsid()
    except OSError:
        # 已是非会话首进程的组长：保持现状，绝不再 setsid（会失败）。
        pass


def _write_record(state_dir: Path, record: OwnerRecord) -> None:
    state_dir.mkdir(parents=True, exist_ok=True)
    path = owner_path(state_dir)
    tmp = state_dir / (OWNER_FILENAME + ".tmp")
    data = json.dumps(record.to_dict(), ensure_ascii=False, indent=2)
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        os.write(fd, data.encode("utf-8"))
        os.fsync(fd)
    finally:
        os.close(fd)
    os.replace(tmp, path)
    try:
        os.chmod(path, 0o600)
    except OSError:
        pass


def claim_ownership(state_dir: str | os.PathLike[str], channel: str) -> OwnerRecord:
    """登记本进程为唯一引擎；同根已有活跃引擎时抛出 OwnerConflict。"""
    root = Path(state_dir)
    inspection = inspect_owner(root)
    if inspection.status == "invalid":
        raise OwnerConflict("所有者登记损坏，拒绝覆盖；请人工检查 " + str(owner_path(root)))
    if inspection.status in ("active_engine", "unknown_live"):
        raise OwnerConflict(inspection.reason + "（state_dir=" + str(root) + "）")
    if inspection.status == "orphan_group":
        # 引擎侧只回收身份完全可核验的遗留组；宿主正常路径会先做同样的回收。
        pgid = inspection.record.pgid if inspection.record else 0
        terminate_group(pgid, grace=3.0)
    record = OwnerRecord(
        pid=os.getpid(),
        pgid=os.getpgrp(),
        host_pid=os.getppid(),
        boot_id=boot_id(),
        channel=str(channel),
        started_at=time.time(),
        cmdline=_proc_cmdline(os.getpid()),
        state_dir=str(root.resolve()),
    )
    _write_record(root, record)
    return record


def release_ownership(state_dir: str | os.PathLike[str] | None) -> None:
    """只删除属于本进程的登记；别人的登记绝不碰。"""
    if not state_dir:
        return
    record = read_owner(state_dir)
    if record is None or record.pid != os.getpid():
        return
    try:
        owner_path(state_dir).unlink()
    except OSError:
        pass


def record_child_group(state_dir: str | os.PathLike[str] | None, pid: int) -> None:
    """登记引擎所拉起子进程的 pid/pgid；只改属于自己的登记。"""
    if not state_dir or pid <= 0:
        return
    record = read_owner(state_dir)
    if record is None or record.pid != os.getpid():
        return
    stat = _proc_stat(pid)
    if stat is None:
        return
    record.child_pid = int(pid)
    record.child_pgid = int(stat[1])
    try:
        _write_record(Path(state_dir), record)
    except OSError:
        pass


def terminate_group(
    pgid: int, *, grace: float = 5.0, trusted_leader: Optional[int] = None
) -> bool:
    """按记录中的进程组回收整棵树。返回是否使用了 SIGKILL。

    只允许杀「组内全部是本产品进程」的组；否则抛 OwnerConflict，绝不误伤。
    """
    if pgid <= 1:
        raise OwnerConflict("拒绝终止非法的进程组号：%r" % (pgid,))
    members = group_members(pgid)
    if not members:
        return False
    if trusted_leader is not None:
        if is_alive(trusted_leader):
            if not child_group_is_ours(trusted_leader, pgid):
                raise OwnerConflict("子进程组组长身份不一致，拒绝终止")
        elif not all(is_engine_cmdline(cmd) for _, cmd in members):
            # 组长已退出：只能凭「组内全是本产品进程」这一条来认定归属。
            raise OwnerConflict("子进程组包含非 OmniDesk 进程，拒绝终止")
    for pid, cmd in members:
        if pid == os.getpid():
            raise OwnerConflict("拒绝终止包含自身的进程组")
        if trusted_leader is None and not is_engine_cmdline(cmd):
            raise OwnerConflict("进程组包含非 OmniDesk 进程，拒绝终止")
    try:
        os.killpg(pgid, signal.SIGTERM)
    except ProcessLookupError:
        return False
    deadline = time.monotonic() + max(0.1, float(grace))
    while time.monotonic() < deadline:
        if not group_members(pgid):
            return False
        time.sleep(0.05)
    try:
        os.killpg(pgid, signal.SIGKILL)
    except ProcessLookupError:
        return False
    deadline = time.monotonic() + 5.0
    while time.monotonic() < deadline and group_members(pgid):
        time.sleep(0.05)
    return True


def reclaim_owner(
    state_dir: str | os.PathLike[str],
    *,
    grace: float = 5.0,
    our_host_pid: Optional[int] = None,
) -> dict:
    """宿主启动前回收遗留引擎；返回可写入证据的动作摘要。"""
    root = Path(state_dir)
    inspection = inspect_owner(root)
    result = {
        "state_dir": str(root),
        "status": inspection.status,
        "action": "none",
        "pgid": None,
        "child_pgid": None,
    }
    if inspection.status == "none":
        return result
    if inspection.status == "invalid":
        raise OwnerConflict("所有者登记损坏，拒绝覆盖：" + str(owner_path(root)))
    record = inspection.record
    assert record is not None
    result["pgid"] = record.pgid
    result["child_pgid"] = record.child_pgid or None

    if inspection.status == "active_engine":
        host_alive = bool(record.host_pid) and is_alive(record.host_pid)
        if our_host_pid is None:
            # 引擎侧从不抢占仍登记为活跃的引擎。
            raise OwnerConflict("另一个 OmniDesk 实例正在使用同一数据根，未启动新引擎")
        # 宿主侧只有「别的宿主还在用」才拒绝；自己的上一次登记（宿主已崩溃）可回收。
        if host_alive and record.host_pid != our_host_pid:
            raise OwnerConflict("另一个 OmniDesk 实例正在使用同一数据根，未启动新引擎")
    if inspection.status in ("active_engine", "orphan_group"):
        if record.boot_id and record.boot_id != boot_id():
            owner_path(root).unlink(missing_ok=True)
            result["action"] = "removed_stale_record"
            return result
        forced = terminate_group(record.pgid, grace=grace)
        child_forced = False
        if record.child_pgid and record.child_pgid != record.pgid:
            child_forced = terminate_group(
                record.child_pgid, grace=grace, trusted_leader=record.child_pid or None
            )
        forced = forced or child_forced
        result["action"] = "sigkill_group" if forced else "sigterm_group"
    elif inspection.status in ("pid_reused",):
        result["action"] = "removed_stale_record"
    else:  # unknown_live
        raise OwnerConflict(inspection.reason + "（state_dir=" + str(root) + "）")
    owner_path(root).unlink(missing_ok=True)
    return result
