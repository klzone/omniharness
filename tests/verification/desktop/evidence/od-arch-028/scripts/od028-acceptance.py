#!/usr/bin/env python3
"""OD-ARCH-028 阶段二试点 · 真实窗口验收（三个工作场景 + 失败/取消）。

在**一次性隔离数据根 + 一次性 Xvfb 显示**里启动真实 OmniDesk 桌面窗口，用本机回环
合成端点走通三个场景的完整闭环：

  载入合成样例 → 明确选择材料文件 → 模型起草 → 人工编辑 → 人工确认 → 仓库外保存 → 读回

另做两件必须诚实记录的事：
  * **失败**：端点返回 400 时，界面不得出现模型正文，保存按钮不得可用，不得产生成果文件；
  * **取消**：端点挂起时点「取消生成」，界面不得出现模型正文，不得产生成果文件。

证据等级：真实窗口运行（XTEST 注入输入 + GDK 抓图）+ 文件事实（隔离根内的成果与任务台账）
          + 端点探针（证明材料正文真的进了模型请求）。
安全边界：只用合成内容与回环合成端点；不读凭证；不碰 runtime/、真实材料与用户桌面会话。

用法：
  python3 od028-acceptance.py --label od028-run1 --display :98 --own-xvfb
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "desktop" / "src-tauri").is_dir())
EVID = REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-028"
SHOTS = EVID / "shots"

sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop"))
sys.path.insert(0, str(REPO / "tests" / "verification" / "desktop" / "evidence" / "od-arch-027" / "step3" / "scripts"))

from window_harness import AppSession, XSession  # noqa: E402
from run_window_acceptance import Driver  # noqa: E402
from _step3lib import (  # noqa: E402
    OwnXvfb,
    assert_no_webkit,
    base_env_snapshot,
    proc_env,
    write_json,
)

NAV_Y = {"home": 235, "report": 285, "writing": 335, "project": 385}

# 每个场景的界面坐标（窗口相对，1280x820；由真实窗口截图校准）。
# sample_scroll / material_scroll 是从页面顶部起的累计滚轮步数。
SCENES = {
    "weekly": {
        "nav": "report", "sample_scroll": 0, "sample": (208, 795),
        "material_scroll": 12, "path": (670, 291), "read": (208, 353),
        "scenario": "weekly_report", "title": "OD028 weekly synthetic",
        "marker": "OD028-WEEKLY-MARKER-3a91",
    },
    "writing": {
        "nav": "writing", "sample_scroll": 0, "sample": (208, 533),
        "material_scroll": 8, "path": (670, 548), "read": (208, 610),
        "scenario": "material_writing", "title": "OD028 writing synthetic",
        "marker": "OD028-WRITING-MARKER-6c42",
    },
    "project": {
        "nav": "project", "sample_scroll": 4, "sample": (208, 520),
        "material_scroll": 12, "path": (670, 362), "read": (208, 424),
        "scenario": "project_management", "title": "OD028 project synthetic",
        "marker": "OD028-PROJECT-MARKER-9b17",
    },
}

TERMINAL = {"succeeded", "failed", "cancelled", "interrupted"}


class Gui:
    """真实窗口里的最小操作封装。"""

    def __init__(self, xs: XSession, win, geo, driver: Driver, shots: Path, tag: str):
        self.xs = xs
        self.win = win
        self.geo = geo
        self.d = driver
        self.shots = shots
        self.tag = tag
        self.n = 0

    def shot(self, name: str) -> str:
        self.n += 1
        p = self.shots / f"{self.tag}-{self.n:02d}-{name}.png"
        self.d.shot(p)
        return str(p)

    def goto(self, scene: str) -> None:
        self.xs.dismiss_overlays()
        self.xs.focus(self.win)
        self.xs.click(self.geo.x + 1240, self.geo.y + 405, settle=0.25)
        self.xs.click(self.geo.x + 48, self.geo.y + NAV_Y[scene], settle=1.8)
        self.xs.move(self.geo.x + 900, self.geo.y + 300)
        time.sleep(0.6)

    def scroll_to(self, clicks: int) -> None:
        """从页面顶部滚到指定累计步数（先回顶，保证可重复）。"""
        self.d.scroll_top()
        done = 0
        while done < clicks:
            step = min(4, clicks - done)
            self.d.scroll_down_steps(step, settle=1.0)
            done += step

    def click_raw(self, rx: int, ry: int, settle: float = 0.8) -> None:
        """不重放滚动配方的点击（用于 locate_primary 之后的定位点击）。"""
        self.xs.focus(self.win)
        self.xs.click(self.geo.x + 1240, self.geo.y + 405, settle=0.2)
        self.xs.click(self.geo.x + rx, self.geo.y + ry, settle=settle)

    def locate_primary(self, y_lo: int = 100, y_hi: int = 800):
        return self.d.locate_primary(y_lo, y_hi)


def newest_task(app_data: Path) -> dict | None:
    files = sorted((app_data / "engine-state").glob("omni-*.json"), key=lambda p: p.stat().st_mtime)
    if not files:
        return None
    try:
        return json.loads(files[-1].read_text(encoding="utf-8"))
    except Exception:
        return None


def wait_terminal(app_data: Path, timeout: float) -> tuple[dict | None, float]:
    waited = 0.0
    task = None
    while waited < timeout:
        task = newest_task(app_data)
        if task and task.get("state") in TERMINAL:
            return task, waited
        time.sleep(1.0)
        waited += 1.0
    return task, waited


def run_scenario(gui: Gui, app, cfg: dict, key: str, material: Path) -> dict:
    """一个场景的完整闭环：样例 → 材料 → 起草 → 编辑 → 确认 → 保存 → 读回。"""
    s = SCENES[key]
    out: dict = {"scenario": s["scenario"], "steps": []}
    t0 = time.monotonic()

    gui.goto(s["nav"])
    gui.scroll_to(s["sample_scroll"])
    gui.d.click(s["sample"][0], s["sample"][1], settle=1.5)
    out["steps"].append({"step": "load_sample", "shot": gui.shot(f"{key}-sample")})

    gui.scroll_to(s["material_scroll"])
    gui.d.click(s["path"][0], s["path"][1], settle=0.6)
    gui.xs.select_all_and_clear()
    gui.xs.type_ascii(str(material), settle=0.02)
    gui.d.click(s["read"][0], s["read"][1], settle=2.0)
    out["steps"].append({"step": "read_material", "shot": gui.shot(f"{key}-material-read")})

    # 材料列表出现后页面变长：多滚一段，按主色定位「生成模型初稿」。
    gui.d.scroll_down_steps(4, settle=1.0)
    loc = gui.locate_primary(100, 800)
    if loc is None:
        gui.d.scroll_down_steps(2, settle=1.0)
        loc = gui.locate_primary(100, 800)
    if loc is None:
        gui.d.scroll_up_steps(4, settle=1.0)
        loc = gui.locate_primary(250, 800)
    if loc is None:
        out["verdict"] = "fail:未定位到生成按钮"
        return out
    out["generate_button"] = list(loc)
    gui.click_raw(loc[0], loc[1], settle=1.0)
    out["steps"].append({"step": "generate", "shot": gui.shot(f"{key}-generating")})

    task, waited = wait_terminal(app.app_data, cfg.task_timeout)
    out["task"] = {k: (task or {}).get(k) for k in ("omni_task_id", "state", "stop_reason", "error_message")}
    out["task_waited_s"] = round(waited, 1)
    out["steps"].append({"step": "task_terminal", "shot": gui.shot(f"{key}-after-generate")})

    if (task or {}).get("state") != "succeeded":
        out["verdict"] = f"fail:任务终态={out['task'].get('state')}"
        out["elapsed_s"] = round(time.monotonic() - t0, 1)
        return out

    # 人工编辑：焦点仍在「生成模型初稿」按钮上，任务结束后按一次 Tab 即进入模型初稿文本框
    # （浏览器会把聚焦元素滚动到可见区域，因此不依赖固定滚动步数）。
    gui.xs.key("Tab", settle=0.5)       # → 模型初稿 textarea
    gui.xs.key("End", settle=0.3)
    gui.xs.type_ascii(" (human-edit-ok)", settle=0.02)
    out["steps"].append({"step": "edit_draft", "shot": gui.shot(f"{key}-edited")})
    gui.xs.key("Tab", settle=0.4)       # → 成果标题
    gui.xs.type_ascii(s["title"], settle=0.02)
    gui.xs.key("Tab", settle=0.4)       # → 人工确认勾选框
    gui.xs.key("space", settle=0.4)
    out["steps"].append({"step": "confirmed", "shot": gui.shot(f"{key}-confirmed")})
    gui.xs.key("Tab", settle=0.4)       # → 保存成果
    gui.xs.key("Return", settle=2.5)
    out["steps"].append({"step": "saved", "shot": gui.shot(f"{key}-saved")})

    artifacts = sorted(app.local_data.glob("offline-*.md"), key=lambda p: p.stat().st_mtime)
    out["artifacts"] = [p.name for p in artifacts]
    if not artifacts:
        out["verdict"] = "fail:保存后没有成果文件"
        out["elapsed_s"] = round(time.monotonic() - t0, 1)
        return out
    latest = artifacts[-1]
    body = latest.read_text(encoding="utf-8")
    out["artifact"] = {
        "path": str(latest),
        "bytes": latest.stat().st_size,
        "has_title": s["title"] in body,
        "has_model_text": "合成回复内容" in body,
        "has_human_edit": "(human-edit-ok)" in body,
        "has_material_provenance": "材料来源（由 OmniDesk 记录，非模型输出）" in body and material.name in body,
        "head": body[:700],
    }

    # 读回：从保存按钮 Tab 到「刷新列表」再 Tab 到「重新打开」。
    # 列表按宿主返回顺序排列，不保证最新在最前；只有列表里只有一件时，
    # 才能断言「读回的就是本场景刚保存的那一件」。
    out["artifacts_count_at_readback"] = len(artifacts)
    gui.xs.key("Tab", settle=0.4)
    gui.xs.key("Tab", settle=0.4)
    gui.xs.key("Return", settle=1.5)
    gui.d.scroll_down_steps(4, settle=1.0)
    out["steps"].append({"step": "reopened", "shot": gui.shot(f"{key}-reopened")})
    out["readback_targets_newest"] = len(artifacts) == 1

    requests = app.mock_ctl.new_requests()
    out["mock_requests"] = [
        {"probe_hits": r.get("probe_hits"), "probe_misses": r.get("probe_misses")} for r in requests
    ]
    out["material_marker_in_request"] = any(s["marker"] in (r.get("probe_hits") or []) for r in requests)
    out["elapsed_s"] = round(time.monotonic() - t0, 1)
    out["verdict"] = "ok" if (
        out["artifact"]["has_model_text"]
        and out["artifact"]["has_material_provenance"]
        and out["material_marker_in_request"]
    ) else "fail:闭环要素缺失"
    return out


def run_negative(gui: Gui, app, key: str, mode: str, cfg: dict) -> dict:
    """失败 / 取消：不得出现模型正文，不得产生成果文件。"""
    s = SCENES[key]
    out: dict = {"mode": mode, "scenario": s["scenario"]}
    before = set(app.local_data.glob("offline-*.md"))
    app.mock_ctl.set(mode=mode, hang_seconds=90.0)
    try:
        gui.goto(s["nav"])
        gui.scroll_to(s["sample_scroll"])
        gui.d.click(s["sample"][0], s["sample"][1], settle=1.5)
        gui.scroll_to(s["material_scroll"])
        gui.d.scroll_down_steps(4, settle=1.0)
        loc = gui.locate_primary(100, 800)
        if loc is None:
            out["verdict"] = "fail:未定位到生成按钮"
            return out
        gui.click_raw(loc[0], loc[1], settle=1.0)
        out["steps"] = [{"step": "generate", "shot": gui.shot(f"neg-{key}-{mode}-generating")}]
        if mode == "hang":
            time.sleep(4.0)
            # 「取消生成」紧挨主按钮右侧
            gui.click_raw(loc[0] + 78, loc[1], settle=1.5)
            out["steps"].append({"step": "cancel", "shot": gui.shot(f"neg-{key}-{mode}-cancelled")})
            task, waited = wait_terminal(app.app_data, cfg.task_timeout)
        else:
            task, waited = wait_terminal(app.app_data, cfg.task_timeout)
            out["steps"].append({"step": "failed", "shot": gui.shot(f"neg-{key}-{mode}-failed")})
        out["task"] = {k: (task or {}).get(k) for k in ("omni_task_id", "state", "stop_reason", "error_message")}
        out["task_waited_s"] = round(waited, 1)
        after = set(app.local_data.glob("offline-*.md"))
        out["new_artifacts"] = [p.name for p in sorted(after - before)]
        out["verdict"] = "ok" if (not out["new_artifacts"]) and out["task"].get("state") in {
            "failed", "cancelled", "interrupted"
        } else f"fail:终态={out['task'].get('state')} 新成果={out['new_artifacts']}"
    finally:
        app.mock_ctl.set(mode="ok", hang_seconds=120.0)
    return out


def run(args) -> dict:
    tag = args.label
    root = Path(args.root_base) / tag
    SHOTS.mkdir(parents=True, exist_ok=True)
    payload: dict = {
        "step": "od-arch-028-acceptance",
        "label": tag,
        "verdict": "started",
        "scenes": {},
    }
    app = AppSession(root=root, display=args.display, artifact_override=True)
    app.prepare()
    cfg_dir = app.app_data / "config"
    cfg_dir.mkdir(parents=True, exist_ok=True)
    app.local_data.mkdir(parents=True, exist_ok=True)
    settings = {
        "schema": "omnidesk.settings.v1",
        "artifact_root": str(app.local_data),
        "model_permissions": {"local": True, "private": False, "public": False},
    }
    (cfg_dir / "settings.v1.json").write_text(json.dumps(settings, indent=2), encoding="utf-8")
    payload["preseeded_settings"] = settings

    materials: dict[str, Path] = {}
    for key, s in SCENES.items():
        p = root / f"material-{key}.md"
        p.write_text(
            f"# 合成材料 {key}\n\n{s['marker']}\n\n"
            "2026-10-12 | 合成事项一 | done\n2026-10-13 | 合成事项二 | pending\n",
            encoding="utf-8",
        )
        materials[key] = p
    payload["materials"] = {k: str(v) for k, v in materials.items()}

    xs = XSession(args.display)
    try:
        assert_no_webkit(f"{tag} 开始前：不允许有其它 WebKit 实例（会污染测量并抢占 5273）")
        app.start()
        win = xs.find_window("OmniDesk", timeout=180)
        if win is None:
            payload["verdict"] = "fail:未找到窗口"
            return payload
        geo = xs.geometry(win)
        xs.focus(win)
        d = Driver(xs, win, geo, root / "_sig.png")
        time.sleep(args.settle)
        gui = Gui(xs, win, geo, d, SHOTS, tag)
        payload["window"] = {"title": xs.title(win), "geometry": geo.as_tuple()}
        payload["app_pid"] = app.proc.pid

        # 首页基线（用户已确认的 UI 基线，回归一次）
        gui.goto("home")
        gui.shot("home-baseline")
        payload["home_baseline_shot"] = f"{tag}-home-baseline"

        for key in SCENES:
            app.mock_ctl.reset_requests()
            app.mock_ctl.set(mode="ok", probes=[SCENES[key]["marker"]])
            payload["scenes"][key] = run_scenario(gui, app, args, key, materials[key])

        payload["negative_failure"] = run_negative(gui, app, "project", "error_400", args)
        payload["negative_cancel"] = run_negative(gui, app, "project", "hang", args)

        payload["artifacts"] = [p.name for p in sorted(app.local_data.glob("offline-*.md"))]
        payload["env"] = base_env_snapshot(args.display, "1", {
            "build": "debug + vite dev（前端 dev server，非内嵌产物）",
            "data_root": str(root),
            "artifact_root": str(app.local_data),
            "synthetic_endpoint": app.mock_base_url,
            "model_permissions": "一次性数据根内预置 local=true（合成端点）",
            "note": "真实窗口；未截取用户桌面；未使用真实材料或凭证；成果只写在一次性隔离根",
        })
        payload["app_env_check"] = {
            "OMNIDESK_LOCAL_DATA_ROOT": proc_env(app.proc.pid).get("OMNIDESK_LOCAL_DATA_ROOT"),
        }
        ok = all(payload["scenes"][k].get("verdict") == "ok" for k in SCENES)
        ok = ok and payload["negative_failure"].get("verdict") == "ok"
        ok = ok and payload["negative_cancel"].get("verdict") == "ok"
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
    return payload


def main(argv) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--display", default=":98")
    ap.add_argument("--root-base", default="/tmp/od028")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--task-timeout", type=float, default=90.0)
    ap.add_argument("--own-xvfb", action="store_true")
    args = ap.parse_args(argv[1:])
    ctx = OwnXvfb(args.display) if args.own_xvfb else _Null()
    with ctx:
        p = run(args)
    write_json(EVID / f"{args.label}.json", p)
    print(json.dumps({
        "verdict": p.get("verdict"),
        "scenes": {k: v.get("verdict") for k, v in p.get("scenes", {}).items()},
        "negative_failure": p.get("negative_failure", {}).get("verdict"),
        "negative_cancel": p.get("negative_cancel", {}).get("verdict"),
        "artifacts": p.get("artifacts"),
    }, ensure_ascii=False))
    return 0


class _Null:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
