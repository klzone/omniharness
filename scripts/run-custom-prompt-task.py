#!/usr/bin/env python3
"""OD-ARCH-174：把桌面组装器产物送引擎（真实或合成端点），验证指针→skill 读取链路。

只输出任务元数据（状态/耗时/工具事件数/token）；提示词正文不打印。
用法：
  python3 scripts/run-custom-prompt-task.py --prompt-file p.json --intent weekly_report --root /tmp/od174-real

提示词 JSON 格式：{"prompt": "..."}；prompt 由 desktop 组装器产出
（见 tests/verification/desktop/evidence/od-arch-174/stageA_probe.js 的组装方式）。
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
EVIDENCE_173 = HERE.parent / "tests" / "verification" / "desktop" / "evidence" / "od-arch-173"
sys.path.insert(0, str(EVIDENCE_173))

import run_synthetic_tasks as S  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--root", type=Path, default=Path("/tmp/od174-real"))
    ap.add_argument("--prompt-file", type=Path, required=True)
    ap.add_argument("--intent", default="weekly_report")
    ap.add_argument("--timeout", type=float, default=300.0)
    args = ap.parse_args()

    prompt_text = json.loads(args.prompt_file.read_text())["prompt"]
    S.TASKS = [{"key": "custom", "intent": args.intent, "prompt": prompt_text}]
    sys.argv = ["run-custom-prompt-task.py", "--root", str(args.root), "--timeout", str(args.timeout)]
    return S.main()


if __name__ == "__main__":
    raise SystemExit(main())
