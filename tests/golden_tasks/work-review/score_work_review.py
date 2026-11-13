#!/usr/bin/env python3
"""Work Review 合成基准评分（OD-HAR-231）。

只评估 fixtures 的候选输出，不调用模型。检查项与宿主现有确定性规则一致：
原文引用必须落在记录正文、日期不得凭空出现、重要性要带依据、低置信度需
显式不确定说明；敏感用例必须整体拒绝。指标口径见 docs/WORK_REVIEW_AGENT_DESIGN.md。
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load(case_id: str) -> dict:
    return json.loads((HERE / "outputs" / f"{case_id}.json").read_text(encoding="utf-8"))


def needs_review(candidate: dict) -> bool:
    confidence = candidate.get("confidence")
    if isinstance(confidence, (int, float)) and confidence < 0.6:
        return True
    if isinstance(confidence, (int, float)) and confidence < 0.7:
        return not (candidate.get("uncertainty") or "").strip()
    return False


def main() -> int:
    bundle = json.loads((HERE / "cases.json").read_text(encoding="utf-8"))
    cases = bundle["cases"]
    failures: list[dict] = []
    grounded = 0
    quotes = 0
    count_ok = 0
    needs_review_ok = 0
    needs_review_cases = 0
    match_ok = 0
    match_cases = 0
    refused_ok = 0
    refused_cases = 0
    material_ok = 0
    material_cases = 0

    for case in cases:
        output = load(case["id"])
        expected = case["expected"]
        body = case["input"]["body"]
        if expected.get("refused"):
            refused_cases += 1
            if output.get("refused") is True and not output.get("candidates"):
                refused_ok += 1
            else:
                failures.append({"case": case["id"], "check": "sensitive_refusal"})
            continue
        candidates = output.get("candidates") or []
        if len(candidates) == expected.get("candidate_count", len(candidates)):
            count_ok += 1
        else:
            failures.append({"case": case["id"], "check": "candidate_count", "got": len(candidates)})
        for cand in candidates:
            quotes += 1
            quote = str(cand.get("source_quote") or "")
            if quote and quote in body:
                grounded += 1
            else:
                failures.append({"case": case["id"], "check": "quote_not_in_body", "quote": quote[:40]})
            due = cand.get("due_date")
            if due and str(due) not in body:
                failures.append({"case": case["id"], "check": "due_date_not_grounded", "due": due})
            importance = cand.get("importance")
            if importance in ("高", "中", "低") and not (cand.get("importance_reason") or "").strip():
                failures.append({"case": case["id"], "check": "importance_reason_missing"})
            confidence = cand.get("confidence")
            if confidence is not None and not (0.0 <= float(confidence) <= 1.0):
                failures.append({"case": case["id"], "check": "confidence_range"})
        expected_needs = bool(expected.get("needs_review"))
        if expected_needs:
            needs_review_cases += 1
            if any(needs_review(cand) for cand in candidates):
                needs_review_ok += 1
            else:
                failures.append({"case": case["id"], "check": "needs_review_expected"})
        elif any(needs_review(cand) for cand in candidates):
            failures.append({"case": case["id"], "check": "unexpected_needs_review"})
        expected_match = expected.get("match_task_id")
        if expected_match:
            match_cases += 1
            if any(cand.get("match_task_id") == expected_match for cand in candidates):
                match_ok += 1
            else:
                failures.append({"case": case["id"], "check": "match_task_missing", "want": expected_match})
        hints = expected.get("knowledge_hints")
        if hints:
            material_cases += 1
            got = [h for m in (output.get("material_candidates") or []) for h in (m.get("knowledge_hints") or [])]
            if all(hint in got for hint in hints):
                material_ok += 1
            else:
                failures.append({"case": case["id"], "check": "knowledge_hints_missing", "want": hints})

    metrics = {
        "cases": len(cases),
        "fact_extraction_accuracy": round(grounded / quotes, 4) if quotes else 1.0,
        "candidate_count_accuracy": round(count_ok / (len(cases) - refused_cases), 4),
        "needs_review_accuracy": round(needs_review_ok / needs_review_cases, 4) if needs_review_cases else None,
        "duplicate_match_accuracy": round(match_ok / match_cases, 4) if match_cases else None,
        "sensitive_refusal_rate": round(refused_ok / refused_cases, 4) if refused_cases else None,
        "knowledge_hint_accuracy": round(material_ok / material_cases, 4) if material_cases else None,
        "grounded_quotes": grounded,
        "total_quotes": quotes,
    }
    result = {"result": "pass" if not failures else "fail", "metrics": metrics, "failures": failures}
    print(json.dumps(result, ensure_ascii=False))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
