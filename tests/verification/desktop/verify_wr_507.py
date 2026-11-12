#!/usr/bin/env python3
"""OD-WR-507「交给 AI 继续分析」：静态契约验证。

证据等级：
  * 静态源码检查：记录/复盘详情按钮、data 属性、预填文本包含对象编号、
    只预填不发送、场景键与导航、全局对话采集保持保守的注释；
  * 运行证据由验收命令单独提供：tsc/build 的实际输出落在
    tests/verification/desktop/od-wr-507/。

用法：python3 tests/verification/desktop/verify_wr_507.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"


def function_body(source: str, name: str) -> str:
    match = re.search(r"function " + re.escape(name) + r"\(\) \{(.*?)\n  \}", source, re.S)
    return match.group(1) if match else ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": detail})

    library = (SRC / "shell" / "WorkRecordLibrary.tsx").read_text(encoding="utf-8")
    archive = (SRC / "shell" / "ReviewArchive.tsx").read_text(encoding="utf-8")
    page = (SRC / "shell" / "WorkRecordPage.tsx").read_text(encoding="utf-8")
    thread = (SRC / "sceneThreads.ts").read_text(encoding="utf-8")
    canvas = (SRC / "shell" / "HomeCanvas.tsx").read_text(encoding="utf-8")
    scenes = (SRC / "scenes.ts").read_text(encoding="utf-8")

    record_hand = function_body(library, "handSelectedToAi")
    review_hand = function_body(archive, "handReviewToAi")

    # 1. 记录详情：按钮、属性与预填上下文
    check("record_button_present", "data-hand-record={selected.id}" in library)
    record_button = library.split("data-hand-record={selected.id}", 1)[-1].split("</button>", 1)[0]
    check("record_button_text", "交给 AI 继续分析" in record_button)
    check("record_button_style_consistent", "wr-ghost-button" in record_button)
    check("record_prompt_prefix", '"请继续分析工作记录 #"' in record_hand)
    check("record_prompt_object_id", "selected.id" in record_hand and '"："' in record_hand)
    check("record_prompt_title", "selected.title" in record_hand)
    check("record_prompt_source_scope", "原始素材与正式记录见工作记录库" in record_hand)
    check("record_prompt_read_only", "不要修改任何文件，只做分析。" in record_hand)

    # 2. 记录详情：场景键、导航、只预填不发送、不写数据层
    record_tag = re.search(r'requestNewThread\(\s*"([^"]+)"', record_hand)
    check("record_scene_key_weekly", bool(record_tag) and record_tag.group(1) == "weekly", record_tag.group(1) if record_tag else "missing")
    check("record_navigates_home", 'onNavigate("home")' in record_hand)
    check(
        "record_no_autosend",
        not any(word in record_hand for word in ("submitTask", "sendMessage", "send(", "taskStatus")),
    )
    check(
        "record_no_data_write",
        not any(word in record_hand for word in ("workRecordUpdate", "workRecordCreate", "workRecordDelete", "invoke(")),
    )

    # 3. 复盘详情：按钮、属性与预填上下文
    check("review_button_present", "data-hand-review={reviewKey(selected)}" in archive)
    review_button = archive.split("data-hand-review={reviewKey(selected)}", 1)[-1].split("</button>", 1)[0]
    check("review_button_text", "交给 AI 继续分析" in review_button)
    check("review_button_style_consistent", "wr-ghost-button" in review_button)
    check("review_prompt_prefix", '"请继续分析"' in review_hand and "PERIOD_FULL_LABEL[detail.period]" in review_hand)
    check("review_prompt_range", '"复盘（"' in review_hand and "detail.range" in review_hand)
    check("review_prompt_doc_id", '"），文档 #"' in review_hand and "detail.period" in review_hand and '"/"' in review_hand)
    check("review_prompt_source_scope", "引用记录与任务见复盘档案" in review_hand)
    check("review_prompt_read_only", "不要修改任何文件，只做分析。" in review_hand)

    # 4. 复盘详情：场景键、导航、只预填不发送、prop 接线
    review_tag = re.search(r'requestNewThread\(\s*"([^"]+)"', review_hand)
    check("review_scene_key_weekly", bool(review_tag) and review_tag.group(1) == "weekly", review_tag.group(1) if review_tag else "missing")
    check("review_navigates_home", 'onNavigate("home")' in review_hand)
    check(
        "review_no_autosend",
        not any(word in review_hand for word in ("submitTask", "sendMessage", "send(", "taskStatus")),
    )
    check("review_navigate_prop", "onNavigate: (scene: Scene) => void" in archive)
    check("review_navigate_wired", "<ReviewArchive" in page and "onNavigate={onNavigate}" in page.split("<ReviewArchive", 1)[1].split("/>", 1)[0])

    # 5. 场景键语义与请求槽只预填
    check("scene_home_not_thread_tag", 'export type SceneKey = "weekly" | "writing" | "project";' in scenes)
    check("scene_weekly_is_work_record", 'key: "weekly"' in scenes and 'label: "工作记录"' in scenes)
    new_thread = thread.split("export function requestNewThread", 1)[1].split("export function takePendingThreadRequest", 1)[0]
    check("request_new_thread_prompt_only", "pendingNewThreadPrompt = prompt?.trim() ? prompt : null" in new_thread)
    check("request_new_thread_no_autosend", not any(word in new_thread for word in ("submit", "invoke(", "fetch(")))
    check("home_consumes_as_draft", 'setInitialInput(req.prompt ?? "")' in canvas)

    # 6. 设计决策：全局对话采集保持保守
    check("design_comment_record", "全局对话采集保持保守不扩" in library)
    check("design_comment_review", "全局对话采集保持保守不扩" in archive)
    check("design_comment_reason", "避免任何输入被误记" in library and "避免任何输入被误记" in archive)

    failed = [case for case in cases if not case["ok"]]
    result = {
        "plan": "OD-WR-507",
        "evidence_level": "静态源码检查（运行证据见同目录 README 与命令输出）",
        "passed": not failed,
        "total": len(cases),
        "passed_count": len(cases) - len(failed),
        "failed": failed,
        "cases": cases,
    }
    for case in cases:
        print(("PASS  " if case["ok"] else "FAIL  ") + case["name"])
    print(f"\n{len(cases) - len(failed)}/{len(cases)} 通过")
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
