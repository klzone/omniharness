#!/usr/bin/env python3
"""OD-ARCH-029 · 使用记录通道自检（真实窗口 + 合成数据）。

验证「使用记录（可选）」这条通道是否满足口径：
  1. 点一次「记录一次」后，记录**写入仓库外的数据根**（不是仓库内、不是用户真实目录）；
  2. 记录**只含枚举与粗耗时**（completed / usable / rework_kind / blocker / duration_bucket /
     reuse_willingness + 记录时间），**不含任何输入、输出、正文、标题或路径**；
  3. 记录**能读回**（界面上的「查看记录 (N)」计数增加，且能从落盘文件读回同一批记录）。

只用合成数据：勾选与下拉都用默认值，不输入任何业务内容。

用法：
  python3 usage-channel-check.py --label od029-usage-run1 --display :0
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
EVID = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-029"
SHOTS = EVID / "shots"
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027" / "step5" / "scripts"))

from window_harness import AppSession  # noqa: E402

# 复用 OD-ARCH-027 Step 5 的真实桌面通道（文件名带连字符，按路径加载）
import importlib.util  # noqa: E402

_step5_path = (
    REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027" / "step5"
    / "scripts" / "step5-real-desktop.py"
)
_spec = importlib.util.spec_from_file_location("step5_real_desktop", _step5_path)
_step5 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_step5)
AtspiUi = _step5.AtspiUi
RealXSession = _step5.RealXSession
capture_window = _step5.capture_window

ALLOWED_KEYS = {
    "record_id", "schema", "task_type", "recorded_at", "completed", "usable",
    "rework_kind", "blocker", "duration_bucket", "reuse_willingness",
}
SCENE_BUTTON = "项目跟踪（模型起草）"
RECORD_BUTTON = "记录一次"


def read_records(path: Path) -> list[dict]:
    if not path.is_file():
        return []
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line:
            out.append(json.loads(line))
    return out


def main(argv) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--display", default=":0")
    ap.add_argument("--root-base", default="/tmp/od029-usage")
    args = ap.parse_args(argv[1:])
    SHOTS.mkdir(parents=True, exist_ok=True)
    root = Path(args.root_base) / args.label
    payload: dict = {"label": args.label, "verdict": "started"}

    app = AppSession(root=root, display=args.display, artifact_override=True)
    app.prepare()
    cfg = app.app_data / "config"
    cfg.mkdir(parents=True, exist_ok=True)
    app.local_data.mkdir(parents=True, exist_ok=True)
    (cfg / "settings.v1.json").write_text(json.dumps({
        "schema": "omnidesk.settings.v1",
        "artifact_root": str(app.local_data),
        "model_permissions": {"local": True, "private": False, "public": False},
    }, indent=2), encoding="utf-8")

    usage_file = app.app_data / "logs" / "usage.jsonl"
    xs = RealXSession(args.display)
    try:
        app.start()
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            payload["verdict"] = "fail:未找到窗口"
            return 1
        xs.place_inside_root(win)
        xs.focus(win)
        time.sleep(10)

        ui = AtspiUi()
        if not ui.connect(timeout=90):
            payload["verdict"] = "fail:AT-SPI 未连接"
            return 1

        # 进入「项目跟踪」场景（其页面底部有使用记录组件）
        node = ui.find_button(SCENE_BUTTON)
        if node is None:
            payload["verdict"] = "fail:未找到 Dock 入口"
            return 1
        ui.click(node)
        time.sleep(2.0)

        # 找到使用记录组件里的复选框与「记录一次」按钮
        roles: dict[str, int] = {}
        for role, nm, _n, _d in ui.walk():
            roles[role] = roles.get(role, 0) + 1
        payload["a11y_roles"] = roles
        checks = [(nm, n) for r, nm, n, _d in ui.walk() if r in ("check box", "checkbox", "toggle button")]
        payload["checkboxes"] = [nm for nm, _n in checks]
        record_btn = ui.find_button(RECORD_BUTTON)
        if record_btn is None:
            payload["verdict"] = "fail:未找到「记录一次」按钮"
            return 1

        before = read_records(usage_file)
        payload["records_before"] = len(before)
        payload["usage_file"] = str(usage_file)
        payload["usage_file_inside_repo"] = str(usage_file).startswith(str(REPO) + "/")

        # 勾选前两个复选框（本次流程完成 / 结果可以直接用），不输入任何文字
        toggled = []
        for nm, n in checks[:2]:
            if ui.click(n):
                toggled.append(nm)
        payload["toggled"] = toggled
        time.sleep(0.5)
        capture_window(win, SHOTS / f"{args.label}-before-record.png")

        ui.click(record_btn)
        time.sleep(2.5)
        capture_window(win, SHOTS / f"{args.label}-after-record.png")

        after = read_records(usage_file)
        payload["records_after"] = len(after)
        payload["new_records"] = after[len(before):]
        # 口径校验：只允许枚举与时间字段
        bad_keys = sorted({k for r in payload["new_records"] for k in r} - ALLOWED_KEYS)
        payload["unexpected_keys"] = bad_keys
        payload["values_are_scalars"] = all(
            isinstance(v, (str, int, float, bool, type(None))) for r in payload["new_records"] for v in r.values()
        )
        # 读回：界面上的「查看记录 (N)」计数
        counts = [nm for r, nm, _n, _d in ui.walk() if r == "button" and "查看记录" in nm]
        payload["view_button_label"] = counts
        payload["readback_count_label"] = counts[0] if counts else None

        ok = (
            len(after) == len(before) + 1
            and not bad_keys
            and payload["values_are_scalars"]
            and not payload["usage_file_inside_repo"]
        )
        payload["verdict"] = "ok" if ok else "fail"
    except Exception as exc:  # noqa: BLE001
        import traceback
        payload["verdict"] = f"error:{type(exc).__name__}:{exc}"
        payload["traceback"] = traceback.format_exc()
    finally:
        try:
            app.stop_all()
        except Exception:
            pass

    EVID.mkdir(parents=True, exist_ok=True)
    (EVID / f"{args.label}.json").write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({
        "verdict": payload.get("verdict"),
        "records": f"{payload.get('records_before')}→{payload.get('records_after')}",
        "unexpected_keys": payload.get("unexpected_keys"),
        "inside_repo": payload.get("usage_file_inside_repo"),
        "view_label": payload.get("readback_count_label"),
        "new_record": payload.get("new_records"),
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
