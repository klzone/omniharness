import json, os, shutil, sys, tempfile, time
from pathlib import Path

REPO = Path("/mnt/data/Code/MyWorkStation")
ADAPTER = REPO / "tests/verification/engine-interface/adapter"
sys.path.insert(0, str(ADAPTER))
import _harness as H
from _mock_model_server import start_mock
from omnidesk_engine import SubmitTaskRequest, TaskState

src = Path.home() / "OmniDesk/runtime/dsh-home/profiles/web"
base = Path(tempfile.mkdtemp(prefix="omni070_tool_"))
root = {"base": base, **{k: base / v for k, v in (("dsh_home","dsh-home"),("agents","agents"),("cwd","work"),("state","state"))}}
for key, p in root.items():
    if key != "base":
        p.mkdir(parents=True, exist_ok=True)
shutil.copytree(src, root["dsh_home"] / "profiles" / "web", symlinks=True)

base_url, ctl, server = start_mock()
ctl.set(mode="tool_call", tool_call_name="web_search",
        tool_call_arguments={"query": "DeepSeek Harness 插件", "maxResults": 3},
        tool_call_followup=["已完成", "检索", "（合成）"])
overlay = H.write_overlay(root, "probe", H.web_overlay_text(base_url))

adapter = None
out = {"evidence_level": "script+network", "mock_mode": "tool_call"}
try:
    adapter = H.make_web_adapter(root, [overlay], init_timeout=90, session_timeout=120, default_timeout=180)
    adapter.initialize()
    rc = adapter.submit_task(SubmitTaskRequest(
        intent="tool-probe", workspace_dir=str(root["cwd"]),
        prompt_text="合成任务：请使用 web_search 检索一次公开资料。",
        timeout_policy={"total_seconds": 180, "search_seconds": 600}))
    out["task_id"] = rc.omni_task_id
    events, deadline = [], time.monotonic() + 180
    while time.monotonic() < deadline:
        events = [e.to_dict() for e in adapter.stream_events(rc.omni_task_id, until_terminal=False, timeout=0.0)]
        st = adapter.get_task_status(rc.omni_task_id)
        if st.status in (TaskState.SUCCEEDED, TaskState.FAILED, TaskState.CANCELLED, TaskState.INTERRUPTED, TaskState.UNKNOWN):
            break
        time.sleep(0.5)
    out["final_status"] = adapter.get_task_status(rc.omni_task_id).to_dict()
    out["tool_events"] = [{"kind": (e.get("payload") or {}).get("kind"),
                           "tool": (e.get("payload") or {}).get("tool_name"),
                           "status": (e.get("payload") or {}).get("status")} for e in events
                          if e.get("type") == "tool_activity"]
    reqs = ctl.new_requests()
    out["model_requests"] = [{"has_tool_message": q.get("has_tool_message"),
                              "text_chars": q.get("text_chars")} for q in reqs]
finally:
    if adapter is not None:
        try: adapter.shutdown(mode="graceful", grace_period=5)
        except Exception: pass
    server.shutdown()
    shutil.rmtree(base, ignore_errors=True)

Path("/tmp/omni070_tool_probe.json").write_text(json.dumps(out, ensure_ascii=False, indent=2))
print(json.dumps(out, ensure_ascii=False, indent=2)[:3000])
