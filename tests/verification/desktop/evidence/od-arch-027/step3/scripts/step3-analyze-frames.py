#!/usr/bin/env python3
"""OD-ARCH-027 Step 3 · 汇总画面异常判据（黑屏/空白/闪烁）。

对 step3/ 下所有运行截图重算像素判据，并**按内容区**（非暗像素的包围盒）统计，
避免把一次性 Xvfb 上未使用的黑色桌面算进「暗像素占比」。
输出 `step3-frame-analysis.json`，并列出所有疑似异常帧（没有则明确写「无」）。

用法：python3 step3-analyze-frames.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _step3lib import STEP3, write_json  # noqa: E402


def analyze(path: Path) -> dict:
    import numpy as np
    from PIL import Image

    im = np.asarray(Image.open(path).convert("RGB"))
    luma = im.astype(np.float64) @ np.array([0.2126, 0.7152, 0.0722])
    mask = luma > 20
    ys, xs = np.nonzero(mask)
    row: dict = {"png": str(path.relative_to(STEP3)), "size": [int(im.shape[1]), int(im.shape[0])],
                 "full_mean_luma": round(float(luma.mean()), 2),
                 "full_dark_fraction": round(float((luma < 40).mean()), 4)}
    if ys.size == 0:
        row.update(bbox=None, content=None, suspicion="black_screen（整幅无可视内容）")
        return row
    bbox = [int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1]
    sub = im[bbox[1] : bbox[3], bbox[0] : bbox[2]]
    sl = luma[bbox[1] : bbox[3], bbox[0] : bbox[2]]
    packed = ((sub[..., 0].astype(np.uint32) << 16) | (sub[..., 1].astype(np.uint32) << 8) | sub[..., 2].astype(np.uint32))
    row["bbox"] = bbox
    row["content"] = {
        "mean_luma": round(float(sl.mean()), 2),
        "luma_std": round(float(sl.std()), 2),
        "dark_fraction": round(float((sl < 40).mean()), 4),
        "distinct_colors_subsampled": int(np.unique(packed[::4, ::4]).size),
    }
    sus = []
    if sl.mean() < 40 or float((sl < 40).mean()) > 0.5:
        sus.append("black_screen")
    if sl.std() < 3.0:
        sus.append("blank")
    row["suspicion"] = "+".join(sus) if sus else None
    return row


def main() -> int:
    files = sorted(STEP3.glob("step3-*.png")) + sorted((STEP3 / "shots").glob("*.png"))
    rows = [analyze(p) for p in files]
    suspicious = [r for r in rows if r.get("suspicion")]
    payload = {
        "script": str(Path(__file__).resolve()),
        "note": "内容区 = 非暗像素包围盒（一次性 Xvfb 的未使用桌面为纯黑，不计入内容区判据）",
        "frames_analyzed": len(rows),
        "suspicious_frames": suspicious,
        "suspicious_count": len(suspicious),
        "verdict": "无异常帧" if not suspicious else f"存在 {len(suspicious)} 个疑似异常帧",
        "frames": rows,
    }
    write_json(STEP3 / "step3-frame-analysis.json", payload)
    print(f"分析 {len(rows)} 张截图 → 疑似异常 {len(suspicious)} 张")
    for r in suspicious:
        print("   !", r["png"], r["suspicion"], r.get("content"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
