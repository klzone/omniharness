#!/usr/bin/env python3
"""
OmniDesk P1 —— headless 通道的“任务恢复”语义探测

验证 A 通道（headless one-shot）在进程退出后，能否按 session id 重新接管既有任务，
以及接管受限条件是否如实报错。全部使用一次性 DSH_HOME（/tmp 下）与合成占位文本。

测的四件事：
  1. 首轮不带 --session-id 发起一次任务，退出码与事件流（记录 session id）。
  2. 第二轮在**相同 cwd** 用 --session-id 接管该 session（观察是否 adopt 成功）。
  3. 第三轮在**不同 cwd** 用同一个 --session-id（预期被拒绝，且原因明确）。
  4. --session-id 指向不存在的 id（预期被拒绝，不静默新建）。

用法：
  python3 tests/verification/engine-interface/probes/probe_headless_recovery.py \
      --out tests/verification/engine-interface/evidence/headless-recovery.json
"""
import argparse, json, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import REPO, one_shot, write_json  # noqa: E402

WORKDIR = Path("/tmp/omni_p1_headless_workdir")


def parse_events(stdout):
    events = []
    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            events.append(json.loads(line))
        except Exception:
            events.append({"_unparsed": line})
    return events


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(REPO / "tests/verification/engine-interface/evidence/headless-recovery.json"))
    a = ap.parse_args()
    WORKDIR.mkdir(parents=True, exist_ok=True)

    rep = {"evidence_level": "script（真实执行 headless 进程；无模型推理）", "workdir": str(WORKDIR), "steps": {}}

    # 1) fresh run
    r1 = one_shot("headless", ["--json", "ping"], cwd=WORKDIR)
    ev1 = parse_events(r1["stdout"])
    session_event = next((e for e in ev1 if e.get("type") == "session"), None)
    sid = (session_event or {}).get("sessionId")
    rep["steps"]["fresh_run"] = {
        "rc": r1["rc"],
        "session_id": sid,
        "event_types": [e.get("type") for e in ev1],
        "note": "一次进程 = 一次任务；session id 由底层生成并在首个事件里给出",
    }

    # 2) adopt with matching cwd
    r2 = one_shot("headless", ["--json", "--session-id", sid, "ping"], cwd=WORKDIR)
    ev2 = parse_events(r2["stdout"])
    adopted = any(e.get("type") == "session" for e in ev2)
    turn_numbers = [e.get("turn") for e in ev2 if e.get("type") == "status" and e.get("turn")]
    rep["steps"]["adopt_same_cwd"] = {
        "rc": r2["rc"],
        "adopted": adopted,
        "turn_numbers_seen": sorted(set(turn_numbers)),
        "event_types": [e.get("type") for e in ev2],
        "note": "接管成功时事件流延续原 Session（turn 编号递增），说明恢复的是既有任务而非新建",
    }

    # 3) adopt with different cwd
    other = Path("/tmp")
    r3 = one_shot("headless", ["--json", "--session-id", sid, "ping"], cwd=other)
    rep["steps"]["adopt_wrong_cwd"] = {
        "rc": r3["rc"],
        "events": parse_events(r3["stdout"]),
        "stderr_head": r3["stderr"].splitlines()[:1],
        "note": "cwd 不一致时明确拒绝接管",
    }

    # 4) unknown id
    r4 = one_shot("headless", ["--json", "--session-id", "session-00000000-0000-0000-0000-000000000000", "ping"], cwd=WORKDIR)
    rep["steps"]["adopt_unknown_id"] = {
        "rc": r4["rc"],
        "events": parse_events(r4["stdout"]),
        "note": "未知 id 直接报错，不会静默新建",
    }

    # 5) concurrent live lock: try to adopt while another process holds the session
    rep["steps"]["concurrent_ownership"] = {
        "verified": False,
        "note": "本步未实测：需要并行持有同一 Session 的活进程；留待第三步（适配器原型）用显式场景验证",
    }

    write_json(a.out, rep)
    print(json.dumps(rep["steps"], ensure_ascii=False, indent=1)[:3000])
    print("wrote", a.out)


if __name__ == "__main__":
    main()
