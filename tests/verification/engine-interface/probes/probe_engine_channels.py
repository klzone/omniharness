#!/usr/bin/env python3
"""
OmniDesk P1 —— 引擎接口四通道只读探测（可重复执行）

目的：在不修改 DSH、不触碰凭证与用户数据的前提下，实测 DSH 公开入口
      （headless / sdk / acp / web）对最小引擎契约六项能力的真实支持度。

只读保证：
  * 全部底层调用使用一次性 DSH_HOME/DSH_AGENTS_HOME（/tmp 下），并带 cwd=/tmp。
  * 不传凭证、不复制凭证、不打印凭证；无凭证时记录 MISSING_CREDENTIAL 为预期结果。
  * 不启动 web profile 的常驻服务；web 通道只做静态事实核对（源码/帮助文本）。
  * 全程使用合成占位文本。

用法：
  python3 tests/verification/engine-interface/probes/probe_engine_channels.py \
      --out tests/verification/engine-interface/evidence/channel-probe.json

证据等级：本脚本产出 script 级证据（真实执行 DSH 进程，不含模型推理）。
"""
import argparse, json, sys, time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import (DSH_BIN, REPO, AUDIT_HOME, AUDIT_AGENTS, JsonRpcPeer, one_shot, write_json)  # noqa: E402

CLIENT_CAPS = {"fs": {"readTextFile": False, "writeTextFile": False}}


def probe_headless():
    """A 通道：headless one-shot。"""
    out = {"channel": "A:headless", "evidence": "script", "findings": {}}

    r = one_shot("headless", ["--json", "ping"], cwd="/tmp")
    lines = [l for l in r["stdout"].splitlines() if l.strip()]
    events = []
    for l in lines:
        try:
            events.append(json.loads(l))
        except Exception:
            events.append({"_unparsed": l})
    types = [e.get("type") for e in events]
    out["findings"]["submit_and_stream"] = {
        "rc": r["rc"],
        "event_types": types,
        "first_event": events[0] if events else None,
        "terminal_event": events[-1] if events else None,
        "stream_is_ndjson_on_stdout": bool(types) and types[0] == "session",
        "note": "无凭证时底层如实返回 turn_end(reason=error)+final(text=''), 退出码 1",
    }
    out["findings"]["stderr_head"] = r["stderr"].splitlines()[:1]

    r2 = one_shot("headless", ["--json", "--session-id", "session-does-not-exist", "ping"], cwd="/tmp")
    ev2 = []
    for l in r2["stdout"].splitlines():
        if l.strip():
            try:
                ev2.append(json.loads(l))
            except Exception:
                ev2.append({"_unparsed": l})
    out["findings"]["recover_unknown_id"] = {
        "rc": r2["rc"],
        "events": ev2,
        "note": "未知 session id 直接报错，不会静默新建会话",
    }
    out["findings"]["cancel"] = {
        "supported": False,
        "note": "CLI 帮助面没有取消旗标；源码只提供进程生命周期与 Session 写锁，取消只能由外部结束进程实现",
    }
    return out


def probe_sdk():
    """B 通道：sdk JSON-RPC stdio。"""
    out = {"channel": "B:sdk", "evidence": "script", "findings": {}}
    sid = "audit-p1-sdk-session"
    peer = JsonRpcPeer("sdk", cwd="/tmp")
    peer.request(1, "initialize", {"cwd": "/tmp", "provider": "deepseek-official", "model": "deepseek-v4-flash"})
    init = peer.wait_response(1, 20)
    out["findings"]["initialize"] = init

    peer.request(2, "session/prompt", {"sessionId": sid, "contentBlocks": [{"type": "text", "text": "ping"}]})
    prompt = peer.wait_response(2, 25)
    out["findings"]["submit_task"] = {
        "response": prompt,
        "note": "只返回入队回执 messageId；不直接返回最终结果",
    }
    time.sleep(4)

    notif = peer.notifications()
    methods = sorted({n.get("method") for n in notif})
    status_frames = [n for n in notif if n.get("method") == "session.status"]
    turn_end = [n for n in notif if n.get("method") == "session.event"
                and n.get("params", {}).get("event", {}).get("type") == "turn/end"]
    out["findings"]["stream_events"] = {
        "notification_methods": methods,
        "status_frames": [f.get("params") for f in status_frames],
        "turn_end_seen": bool(turn_end),
        "turn_end_reason": turn_end[0]["params"]["event"]["data"] if turn_end else None,
        "note": "事件按持久化 Session 事件推送；状态为 idle/running 两态",
    }

    peer.request(3, "session/cancel", {"sessionId": sid})
    cancel = peer.wait_response(3, 10)
    out["findings"]["cancel_task"] = {
        "supported": not bool(cancel and cancel.get("error")),
        "response": cancel,
        "note": "实测：协议无 session/cancel 方法",
    }

    peer.request(4, "session/wait", {"sessionId": sid})
    wait = peer.wait_response(4, 30)
    out["findings"]["get_task_result"] = {
        "wait_response": wait,
        "note": "session/wait 只等待静默，不返回最终结果；最终文本需客户端从 session.event 自行拼装",
    }
    out["findings"]["get_task_status"] = {
        "supported": True,
        "note": "由 session.status（idle/running）通知提供",
    }
    out["findings"]["recover_task"] = {
        "supported": "partial",
        "note": "SDK 协议没有 list/resume；未知 sessionId 在 prompt 时会惰性创建，因此不能区分恢复与新建",
    }
    out["exit_code"] = peer.close()
    return out


def probe_acp():
    """C 通道：ACP v1 JSON-RPC stdio。"""
    out = {"channel": "C:acp", "evidence": "script", "findings": {}}
    peer = JsonRpcPeer("acp", cwd="/tmp")
    peer.request(1, "initialize", {"protocolVersion": 1, "clientCapabilities": CLIENT_CAPS})
    init = peer.wait_response(1, 20)
    out["findings"]["initialize"] = init
    caps = ((init or {}).get("result") or {}).get("agentCapabilities", {})
    out["findings"]["advertised_session_capabilities"] = caps.get("sessionCapabilities")

    peer.request(2, "session/new", {"cwd": "/tmp", "mcpServers": []})
    new = peer.wait_response(2, 25)
    sid = ((new or {}).get("result") or {}).get("sessionId")
    out["findings"]["session_new"] = {"sessionId_present": bool(sid), "result_keys": sorted((new or {}).get("result", {}).keys())}

    peer.request(3, "session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "ping"}]})
    prompt = peer.wait_response(3, 40)
    out["findings"]["submit_task"] = {
        "response": prompt,
        "note": "无凭证时以 JSON-RPC error(-32603, turn failed) 返回；prompt 请求本身与最终结果同源",
    }

    peer.notify("session/cancel", {"sessionId": sid})
    time.sleep(0.8)
    peer.request(4, "session/list", {})
    lst = peer.wait_response(4, 15)
    out["findings"]["cancel_task"] = {
        "supported": True,
        "mechanism": "session/cancel 通知（通知帧无响应）；AcpSession.cancel() 取消当前 prompt 或自主工作",
        "connection_alive_after_cancel": lst is not None and not (lst or {}).get("error"),
        "stop_reason_mapping_source": "dsh-acp/lib/index.js: interrupted->cancelled",
    }
    peer.request(5, "session/close", {"sessionId": sid})
    close = peer.wait_response(5, 20)
    out["findings"]["close_task_session"] = {"response": close}
    peer.request(6, "session/resume", {"sessionId": sid, "cwd": "/tmp", "mcpServers": []})
    resume = peer.wait_response(6, 30)
    out["findings"]["recover_task"] = {
        "supported": True,
        "resume_after_close_ok": bool(resume and not resume.get("error")),
        "note": "session/list + session/resume 提供跨进程恢复；close 后仍可 resume",
    }
    peer.request(7, "session/resume", {"sessionId": "nonexistent", "cwd": "/tmp", "mcpServers": []})
    bad = peer.wait_response(7, 15)
    out["findings"]["recover_unknown_id"] = bad
    out["exit_code"] = peer.close()
    return out


def probe_acp_crash_recovery():
    """C 通道补充：进程被强杀后，新进程能否识别既有任务。"""
    out = {"channel": "C:acp-crash-recovery", "evidence": "script", "findings": {}}
    peer = JsonRpcPeer("acp", cwd="/tmp")
    peer.request(1, "initialize", {"protocolVersion": 1, "clientCapabilities": CLIENT_CAPS})
    peer.wait_response(1, 20)
    peer.request(2, "session/new", {"cwd": "/tmp", "mcpServers": []})
    new = peer.wait_response(2, 25)
    sid = ((new or {}).get("result") or {}).get("sessionId")
    out["findings"]["created_session"] = sid
    peer.hard_kill()
    time.sleep(1.0)

    peer2 = JsonRpcPeer("acp", cwd="/tmp")
    peer2.request(1, "initialize", {"protocolVersion": 1, "clientCapabilities": CLIENT_CAPS})
    peer2.wait_response(1, 20)
    peer2.request(2, "session/list", {})
    lst = peer2.wait_response(2, 20)
    rows = ((lst or {}).get("result") or {}).get("sessions", [])
    out["findings"]["listed_after_crash"] = any(r.get("sessionId") == sid for r in rows)
    peer2.request(3, "session/resume", {"sessionId": sid, "cwd": "/tmp", "mcpServers": []})
    resume = peer2.wait_response(3, 30)
    out["findings"]["resume_after_crash_ok"] = bool(resume and not resume.get("error"))
    out["findings"]["note"] = (
        "被强杀的进程没有机会写取消/终止记录；既有 Session 仍可被新进程列出并 resume。"
        "这证明状态可发现与可重新接管，但不等于原任务被自动续跑：进行中的一轮不会自动重放。"
    )
    out["exit_code"] = peer2.close()
    return out


def probe_web_static():
    """D 通道：web profile 的静态事实核对（不启动常驻服务）。"""
    out = {"channel": "D:web-static", "evidence": "static", "findings": {}}
    out["findings"]["help"] = one_shot("web", ["--help"], cwd="/tmp", timeout=60)["stdout"]
    out["findings"]["note"] = (
        "web profile 的业务能力（session cancel/follow、job kill 等）由私有 Remote 协议承载，"
        "属未发布内部接口；本审计只核对帮助面与源码事实，不对其做联机调用。"
    )
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(REPO / "tests/verification/engine-interface/evidence/channel-probe.json"))
    a = ap.parse_args()

    payload = {
        "evidence_level": "script（真实执行 DSH 进程；无模型推理）",
        "dsh_bin": str(DSH_BIN),
        "dsh_home": f"{AUDIT_HOME}（本次运行专用，进程级一次性）",
        "agents_home": f"{AUDIT_AGENTS}（本次运行专用）",
        "probes": [],
    }
    for fn in (probe_headless, probe_sdk, probe_acp, probe_acp_crash_recovery, probe_web_static):
        try:
            payload["probes"].append(fn())
        except Exception as exc:  # 失败也要留证
            payload["probes"].append({"channel": fn.__name__, "error": f"{type(exc).__name__}: {exc}"})
    write_json(a.out, payload)
    print("wrote", a.out)
    for p in payload["probes"]:
        print("-", p.get("channel"), "err" if p.get("error") else "ok")


if __name__ == "__main__":
    main()
