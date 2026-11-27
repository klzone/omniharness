#!/usr/bin/env python3
"""Work Review 提示词 A/B 对照跑分。

两种用法：

1. 跑两版提示词
   python3 ab_runner.py \
     --variant baseline=variants/baseline.txt \
     --variant strict=variants/strict.txt \
     --runner-cmd "python3 my_model_runner.py"

2. 只比较两批已有输出
   python3 ab_runner.py --score a=dir-a --score b=dir-b

runner 从 stdin 读取一个 JSON 对象，字段为 case_id、variant、prompt，
并把候选 JSON 写到 stdout。stdout 只放 JSON，日志走 stderr。
脚本只打印分数、耗时与输出目录，不打印提示词正文或候选正文。
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import time
from datetime import datetime
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "engine"))

from omnidesk_engine.task_extract import assemble_task_extract_prompt  # noqa: E402

SCORER = HERE / "score_work_review.py"
DEFAULT_CASES = HERE / "cases.json"

METRICS = (
    ("fact_extraction_accuracy", "fact"),
    ("candidate_count_accuracy", "count"),
    ("needs_review_accuracy", "review"),
    ("duplicate_match_accuracy", "match"),
    ("sensitive_refusal_rate", "refusal"),
    ("knowledge_hint_accuracy", "hints"),
)


def named_path(flag: str):
    def parse(raw: str) -> tuple[str, Path]:
        if "=" not in raw:
            raise argparse.ArgumentTypeError(f"{flag} 需要 NAME=PATH")
        name, path = raw.split("=", 1)
        name = name.strip()
        if not name or any(ch not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_" for ch in name):
            raise argparse.ArgumentTypeError(f"{flag} 的名称只能包含字母、数字、连字符和下划线")
        if not path:
            raise argparse.ArgumentTypeError(f"{flag} 缺少路径")
        return name, Path(path)

    return parse


def parse_model_output(text: str) -> dict[str, Any]:
    text = text.strip()
    if text.startswith("```"):
        lines = text.splitlines()
        if lines and lines[0].startswith("```"):
            lines = lines[1:]
        if lines and lines[-1].startswith("```"):
            lines = lines[:-1]
        text = "\n".join(lines).strip()
    data = json.loads(text)
    if not isinstance(data, dict):
        raise ValueError("runner 输出必须是 JSON 对象")
    return data


def build_prompt(case: dict[str, Any], variant_text: str) -> str:
    record = case.get("input") or {}
    prompt = assemble_task_extract_prompt(
        title=str(record.get("title") or ""),
        body=str(record.get("body") or ""),
        record_date=str(record.get("record_date") or ""),
        category=record.get("category"),
        tags=[str(tag) for tag in (record.get("tags") or [])],
    )
    if variant_text.strip():
        prompt += "\n\n---实验追加要求---\n" + variant_text.strip() + "\n"
    return prompt


def score_variant(name: str, outputs: Path, cases: Path) -> dict[str, Any]:
    proc = subprocess.run(
        [sys.executable, str(SCORER), "--cases", str(cases), "--outputs", str(outputs)],
        text=True,
        capture_output=True,
        cwd=str(REPO),
        timeout=60,
    )
    try:
        result = json.loads(proc.stdout.strip())
    except ValueError as error:
        raise RuntimeError(
            f"{name} 评分脚本没有输出可解析的 JSON（退出码 {proc.returncode}）：{error}"
        ) from error
    if not isinstance(result, dict) or "metrics" not in result:
        raise RuntimeError(f"{name} 评分结果缺少 metrics")
    return result


def run_variant(
    name: str,
    variant_text: str,
    cases: list[dict[str, Any]],
    runner_cmd: str,
    outputs: Path,
    timeout: float,
    show_stderr: bool,
) -> float | None:
    outputs.mkdir(parents=True, exist_ok=True)
    durations: list[float] = []
    for case in cases:
        case_id = str(case["id"])
        prompt = build_prompt(case, variant_text)
        payload = json.dumps(
            {"case_id": case_id, "variant": name, "prompt": prompt},
            ensure_ascii=False,
        )
        started = time.monotonic()
        try:
            proc = subprocess.run(
                runner_cmd,
                shell=True,
                input=payload,
                text=True,
                capture_output=True,
                cwd=str(REPO),
                timeout=timeout,
            )
        except subprocess.TimeoutExpired as error:
            raise RuntimeError(f"{name}/{case_id} 超时（{timeout:g}s）") from error
        durations.append(time.monotonic() - started)
        if proc.returncode != 0:
            detail = proc.stderr.strip()[-200:] if show_stderr else ""
            suffix = f"：{detail}" if detail else ""
            raise RuntimeError(f"{name}/{case_id} runner 退出码 {proc.returncode}{suffix}")
        try:
            data = parse_model_output(proc.stdout)
        except ValueError as error:
            raise RuntimeError(f"{name}/{case_id} 输出不是有效 JSON：{error}") from error
        target = outputs / f"{case_id}.json"
        target.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return sum(durations) / len(durations) if durations else None


def fmt(value: Any) -> str:
    if value is None:
        return "—"
    if isinstance(value, float):
        return f"{value:.2f}"
    return str(value)


def print_table(summary: dict[str, Any]) -> None:
    header = "| variant | cases | " + " | ".join(label for _, label in METRICS) + " | avg_s | failures |"
    separator = "|---" * (len(METRICS) + 4) + "|"
    print(header)
    print(separator)
    for item in summary["variants"]:
        metrics = item["metrics"]
        cells = [item["name"], fmt(metrics.get("cases"))]
        cells.extend(fmt(metrics.get(key)) for key, _ in METRICS)
        cells.append(fmt(item.get("avg_seconds")))
        cells.append(fmt(len(item.get("failures") or [])))
        print("| " + " | ".join(cells) + " |")


def main() -> int:
    parser = argparse.ArgumentParser(description="Work Review 提示词 A/B 对照跑分")
    parser.add_argument("--variant", action="append", default=[], type=named_path("--variant"), metavar="NAME=PATH", help="提示词变体文本文件，可重复")
    parser.add_argument("--score", action="append", default=[], type=named_path("--score"), metavar="NAME=DIR", help="直接评分已有输出目录，可重复")
    parser.add_argument("--runner-cmd", default=None, help="每道题执行一次的命令，prompt 走 stdin")
    parser.add_argument("--cases", type=Path, default=DEFAULT_CASES)
    parser.add_argument("--out", type=Path, default=None, help="输出根目录，默认新建临时目录")
    parser.add_argument("--timeout", type=float, default=120.0, help="单题墙钟上限（秒）")
    parser.add_argument("--show-runner-stderr", action="store_true", help="runner 失败时打印 stderr 末尾")
    parser.add_argument("--json", action="store_true", help="输出完整 JSON 摘要")
    args = parser.parse_args()

    args.cases = args.cases.resolve()
    cases = json.loads(args.cases.read_text(encoding="utf-8"))["cases"]
    if args.variant and not args.runner_cmd:
        parser.error("使用 --variant 时必须同时给出 --runner-cmd")
    if len(args.variant) + len(args.score) < 2:
        parser.error("至少需要两个变体，才能做 A/B 对照")
    names = [name for name, _ in args.variant] + [name for name, _ in args.score]
    if len(set(names)) != len(names):
        parser.error("变体名称不能重复")

    run_root = args.out or Path(tempfile.mkdtemp(prefix="omnidesk-work-review-ab-"))
    run_root = run_root / datetime.now().strftime("%Y%m%d-%H%M%S")

    variants: list[dict[str, Any]] = []
    for name, path in args.variant:
        text = "" if str(path) == "-" else path.read_text(encoding="utf-8")
        outputs = run_root / name
        avg_seconds = run_variant(
            name,
            text,
            cases,
            args.runner_cmd,
            outputs,
            args.timeout,
            args.show_runner_stderr,
        )
        result = score_variant(name, outputs, args.cases)
        variants.append(
            {
                "name": name,
                "kind": "run",
                "outputs": str(outputs),
                "metrics": result["metrics"],
                "failures": result.get("failures") or [],
                "avg_seconds": avg_seconds,
            }
        )

    for name, path in args.score:
        result = score_variant(name, path.resolve(), args.cases)
        variants.append(
            {
                "name": name,
                "kind": "score",
                "outputs": str(path),
                "metrics": result["metrics"],
                "failures": result.get("failures") or [],
                "avg_seconds": None,
            }
        )

    summary = {
        "schema": "omnidesk.work-review-ab.v1",
        "cases": len(cases),
        "variants": variants,
    }
    if args.json:
        print(json.dumps(summary, ensure_ascii=False))
    else:
        print_table(summary)
        if args.variant:
            print(f"outputs: {run_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
