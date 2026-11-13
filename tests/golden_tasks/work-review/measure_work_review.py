#!/usr/bin/env python3
"""Work Review 指标测量（OD-HAR-232）。

两种数据源：
  --fixtures        用合成基准输出跑指标管线（证明口径可复现，不代表真实模型表现）
  --candidates DIR  读真实候选 JSON（work/.candidates），只统计状态、置信度与验收结论，不打印标题
  --events FILE     可选：读 events.jsonl，按类型计数
  --ledger DIR      可选：读 engine-state 台账，统计 usage 是否上报（None 计未知）

输出只有数字与枚举，不含正文、标题或人员信息。
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


def fixture_metrics() -> dict:
    bundle = json.loads((HERE / "cases.json").read_text(encoding="utf-8"))
    total = 0
    grounded = 0
    confidences: list[float] = []
    needs_review_count = 0
    refused = 0
    for case in bundle["cases"]:
        output = json.loads((HERE / "outputs" / f"{case['id']}.json").read_text(encoding="utf-8"))
        if output.get("refused"):
            refused += 1
            continue
        body = case["input"]["body"]
        for cand in output.get("candidates") or []:
            total += 1
            quote = str(cand.get("source_quote") or "")
            if quote and quote in body:
                grounded += 1
            if isinstance(cand.get("confidence"), (int, float)):
                confidences.append(float(cand["confidence"]))
            confidence = cand.get("confidence")
            if isinstance(confidence, (int, float)) and (
                confidence < 0.6 or (confidence < 0.7 and not (cand.get("uncertainty") or "").strip())
            ):
                needs_review_count += 1
    return {
        "source": "fixtures",
        "candidates": total,
        "grounded_quotes": grounded,
        "grounding_rate": round(grounded / total, 4) if total else None,
        "avg_confidence": round(sum(confidences) / len(confidences), 4) if confidences else None,
        "needs_review": needs_review_count,
        "refused_cases": refused,
    }


def candidate_metrics(directory: Path) -> dict:
    items = []
    broken = 0
    for path in sorted(directory.glob("*.json")):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            broken += 1
            continue
        if isinstance(data, dict):
            items.append(data)
    status_counts: dict[str, int] = {}
    verdict_pass = verdict_needs = verdict_absent = 0
    confidences: list[float] = []
    for item in items:
        status = str(item.get("status") or "unknown")
        status_counts[status] = status_counts.get(status, 0) + 1
        verdict = item.get("verdict")
        if isinstance(verdict, dict):
            if verdict.get("conclusion") == "pass":
                verdict_pass += 1
            else:
                verdict_needs += 1
        else:
            verdict_absent += 1
        if isinstance(item.get("confidence"), (int, float)):
            confidences.append(float(item["confidence"]))
    return {
        "source": "candidates",
        "candidates": len(items),
        "broken": broken,
        "status": dict(sorted(status_counts.items())),
        "verdict_pass": verdict_pass,
        "verdict_needs_review": verdict_needs,
        "verdict_absent": verdict_absent,
        "avg_confidence": round(sum(confidences) / len(confidences), 4) if confidences else None,
    }


def event_metrics(path: Path) -> dict:
    by_type: dict[str, int] = {}
    broken = 0
    if not path.exists():
        return {"source": "events", "present": False}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except ValueError:
            broken += 1
            continue
        kind = str(row.get("type") or "unknown")
        by_type[kind] = by_type.get(kind, 0) + 1
    return {"source": "events", "present": True, "total": sum(by_type.values()), "by_type": dict(sorted(by_type.items())), "broken": broken}


def ledger_metrics(directory: Path) -> dict:
    usage_known = usage_unknown = 0
    for path in sorted(directory.glob("omni-*.json")):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if isinstance(data.get("usage"), dict):
            usage_known += 1
        else:
            usage_unknown += 1
    return {"source": "ledger", "records": usage_known + usage_unknown, "usage_known": usage_known, "usage_unknown": usage_unknown}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixtures", action="store_true")
    parser.add_argument("--candidates", type=Path)
    parser.add_argument("--events", type=Path)
    parser.add_argument("--ledger", type=Path)
    args = parser.parse_args()

    parts = []
    if args.fixtures:
        parts.append(fixture_metrics())
    if args.candidates:
        parts.append(candidate_metrics(args.candidates))
    if args.events:
        parts.append(event_metrics(args.events))
    if args.ledger:
        parts.append(ledger_metrics(args.ledger))
    if not parts:
        parser.error("至少指定 --fixtures / --candidates / --events / --ledger 之一")
    print(json.dumps({"parts": parts}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
