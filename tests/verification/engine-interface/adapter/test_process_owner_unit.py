#!/usr/bin/env python3
"""OD-ARCH-074 —— 引擎所有权登记与进程组回收的纯单元测试。

全部使用 /tmp 下的临时状态根和可丢弃的假进程；不读取、不启动真实 DSH，
也不碰 ~/.dsh、~/.agents 或 ~/OmniDesk。

证据等级：static（纯 Python 单元测试，含本机假进程的真实信号路径）。
运行：python3 -m unittest -v test_process_owner_unit
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

import _harness as H  # noqa: F401 - 负责把 engine/ 加进 sys.path

from omnidesk_engine import process_owner as po


def spawn_fake_engine(tag: str = "engine") -> subprocess.Popen:
    """起一个可丢弃、独占进程组的假 DSH 进程（cmdline 含 omnidesk_engine）。"""
    return subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(120)", "omnidesk_engine-" + tag],
        start_new_session=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def spawn_bystander() -> subprocess.Popen:
    """与本产品无关的旁观进程：任何路径都不得被误杀。"""
    return subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(120)", "unrelated-bystander"],
        start_new_session=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def wait_group_gone(pgid: int, timeout: float = 8.0) -> bool:
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if not po.group_members(pgid):
            return True
        time.sleep(0.05)
    return not po.group_members(pgid)


class ProcessOwnerTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="od074-py-"))
        self.children: list[subprocess.Popen] = []

    def tearDown(self):
        for child in self.children:
            if child.poll() is None:
                child.kill()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    pass
        for item in self.root.rglob("*"):
            try:
                item.unlink()
            except OSError:
                pass
        try:
            self.root.rmdir()
        except OSError:
            pass

    def track(self, proc: subprocess.Popen) -> subprocess.Popen:
        self.children.append(proc)
        return proc

    def write_record(self, **overrides) -> dict:
        record = {
            "pid": 0,
            "pgid": 0,
            "host_pid": 0,
            "boot_id": po.boot_id(),
            "channel": "web-api",
            "started_at": time.time(),
            "cmdline": [],
            "state_dir": str(self.root),
            "child_pid": 0,
            "child_pgid": 0,
        }
        record.update(overrides)
        (self.root / po.OWNER_FILENAME).write_text(json.dumps(record), encoding="utf-8")
        return record

    # -- 1. 正常登记与释放 -------------------------------------------------
    def test_claim_writes_private_record_and_release_removes_it(self):
        self.assertEqual(po.inspect_owner(self.root).status, "none")
        record = po.claim_ownership(self.root, "web-api")
        path = po.owner_path(self.root)
        self.assertTrue(path.exists())
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(record.pid, os.getpid())
        self.assertEqual(record.pgid, os.getpgrp())
        self.assertEqual(po.inspect_owner(self.root).status, "active_engine")
        po.release_ownership(self.root)
        self.assertFalse(path.exists())

    def test_release_never_deletes_foreign_record(self):
        self.write_record(pid=os.getpid() + 999999, cmdline=["other"])
        po.release_ownership(self.root)
        self.assertTrue(po.owner_path(self.root).exists())

    def test_record_child_group_only_touches_own_registration(self):
        self.write_record(pid=os.getpid() + 999999, cmdline=["other"])
        po.record_child_group(self.root, os.getpid())
        data = json.loads(po.owner_path(self.root).read_text(encoding="utf-8"))
        self.assertEqual(data["child_pid"], 0)

    def test_record_child_group_persists_pid_and_pgid(self):
        po.claim_ownership(self.root, "web-api")
        child = self.track(spawn_fake_engine("child"))
        po.record_child_group(self.root, child.pid)
        data = json.loads(po.owner_path(self.root).read_text(encoding="utf-8"))
        self.assertEqual(data["child_pid"], child.pid)
        self.assertEqual(data["child_pgid"], child.pid)

    # -- 2. 同根第二个实例被拒 ---------------------------------------------
    def test_second_instance_on_same_root_is_refused(self):
        other = self.track(spawn_fake_engine("holder"))
        self.write_record(
            pid=other.pid,
            pgid=other.pid,
            cmdline=po._proc_cmdline(other.pid),
        )
        self.assertEqual(po.inspect_owner(self.root).status, "active_engine")
        with self.assertRaises(po.OwnerConflict):
            po.claim_ownership(self.root, "web-api")
        with self.assertRaises(po.OwnerConflict):
            po.reclaim_owner(self.root, grace=1.0)
        self.assertIsNone(other.poll(), "活跃引擎不得被抢占")
        self.assertTrue(po.owner_path(self.root).exists())

    # -- 3. 伪造身份不杀进程 -----------------------------------------------
    def test_reused_pid_with_foreign_cmdline_is_not_killed(self):
        bystander = self.track(spawn_bystander())
        self.write_record(pid=bystander.pid, pgid=bystander.pid, cmdline=["not-our-engine"])
        self.assertEqual(po.inspect_owner(self.root).status, "pid_reused")
        result = po.reclaim_owner(self.root, grace=1.0)
        self.assertEqual(result["action"], "removed_stale_record")
        self.assertIsNone(bystander.poll())
        self.assertFalse(po.owner_path(self.root).exists())

    # -- 4. 跨内核启动只删登记 ---------------------------------------------
    def test_record_from_previous_boot_only_removes_record(self):
        bystander = self.track(spawn_bystander())
        self.write_record(
            pid=bystander.pid,
            pgid=bystander.pid,
            cmdline=["not-our-engine"],
            boot_id="previous-boot",
        )
        result = po.reclaim_owner(self.root, grace=1.0)
        self.assertEqual(result["action"], "removed_stale_record")
        self.assertIsNone(bystander.poll())
        self.assertFalse(po.owner_path(self.root).exists())

    # -- 5. 引擎已死、子进程组遗留 → 按登记回收 -----------------------------
    def test_orphan_child_group_is_reclaimed_and_bystander_survives(self):
        engine = self.track(spawn_fake_engine("dead"))
        engine_cmdline = po._proc_cmdline(engine.pid)
        engine_pid = engine.pid
        engine.kill()
        engine.wait(timeout=5)

        child = self.track(spawn_fake_engine("orphan-child"))
        bystander = self.track(spawn_bystander())
        self.write_record(
            pid=engine_pid,
            pgid=engine_pid,
            cmdline=engine_cmdline,
            child_pid=child.pid,
            child_pgid=child.pid,
        )
        self.assertEqual(po.inspect_owner(self.root).status, "orphan_group")
        result = po.reclaim_owner(self.root, grace=2.0)
        self.assertIn(result["action"], ("sigterm_group", "sigkill_group"))
        self.assertTrue(wait_group_gone(child.pid), "遗留子进程组必须被回收")
        self.assertIsNone(bystander.poll(), "无关进程不得被误杀")
        self.assertFalse(po.owner_path(self.root).exists())

    # -- 6. 损坏登记与非法组号 ---------------------------------------------
    def test_corrupt_record_is_refused_and_kept(self):
        po.owner_path(self.root).write_text("{not json", encoding="utf-8")
        self.assertEqual(po.inspect_owner(self.root).status, "invalid")
        with self.assertRaises(po.OwnerConflict):
            po.claim_ownership(self.root, "web-api")
        self.assertTrue(po.owner_path(self.root).exists())

    def test_refuses_illegal_group_ids(self):
        for pgid in (0, 1, -5):
            with self.assertRaises(po.OwnerConflict):
                po.terminate_group(pgid, grace=0.1)


if __name__ == "__main__":
    unittest.main()
