#!/usr/bin/env python3
"""OD-ARCH-077 S1：对话附件与 @ 引用的离线验证。

直接编译真实源码（`attachmentPrompt.ts`、`conversationPrompt.ts`），在 Node 里跑确定性分支：

1. 附件数量 / 空文件 / 总长上限在发送前被拦住，不静默截断。
2. 附件正文连同文件名、绝对路径与 sha256 进入提示词。
3. 红线扫描文本包含附件正文（只加原话会漏掉附件里的禁入内容）。
4. @ 查询词解析与替换。
5. **关键回归**：长附件不会再被「上文截断」削掉——只截历史，本轮输入完整。

证据等级：脚本运行（编译真实 TypeScript 源码）。不验证真实窗口、真实宿主或模型输出。

用法：
  python3 tests/verification/desktop/verify_attachments_077.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SRC = DESKTOP / "src"

PROBE = r"""
const att = require(process.argv[2]);
const conv = require(process.argv[3]);
const cases = [];
function check(name, ok, detail) { cases.push({ name, ok: !!ok, detail: detail || "" }); }

function item(name, content) {
  return { path: "/tmp/od-safe/" + name, fileName: name, sha256: "a".repeat(64), content };
}

// 1) 发送前检查
const one = item("memo.md", "合成内容 ALPHA");
check("single_ok", att.checkAttachments([one]).ok);
check("empty_list_ok", att.checkAttachments([]).ok);
const many = [1,2,3,4,5,6].map((i) => item("f" + i + ".md", "x"));
const tooMany = att.checkAttachments(many);
check("too_many_rejected", !tooMany.ok && tooMany.reason.includes("最多"));
const blank = att.checkAttachments([item("blank.txt", "   ")]);
check("blank_rejected", !blank.ok && blank.reason.includes("空文件"));
const huge = att.checkAttachments([item("huge.txt", "x".repeat(att.ATTACHMENT_TOTAL_MAX_CHARS + 1))]);
check("oversize_rejected", !huge.ok && huge.reason.includes("上限"));

// 2) 提示词片段
const block = att.composeWithAttachments("请总结", [one]);
check("keeps_user_text", block.startsWith("请总结"));
check("has_file_name", block.includes("memo.md"));
check("has_abs_path", block.includes("/tmp/od-safe/memo.md"));
check("has_sha256", block.includes("a".repeat(64)));
check("has_content", block.includes("ALPHA"));
check("states_no_fabrication", block.includes("不要补造"));
check("no_attachment_keeps_text", att.composeWithAttachments("只有文字", []) === "只有文字");

// 3) 红线扫描覆盖附件正文
const scan = att.attachmentScanText("用户原话 KEYWORD-A", [item("a.md", "附件正文 KEYWORD-B")]);
check("scan_has_user_text", scan.includes("KEYWORD-A"));
check("scan_has_attachment", scan.includes("KEYWORD-B"));
check("scan_without_attachments", att.attachmentScanText("x", []) === "x");

// 4) @ 查询
check("mention_after_text", att.activeMentionQuery("总结 @制度") === "制度");
check("mention_at_start", att.activeMentionQuery("@") === "");
check("mention_needs_boundary", att.activeMentionQuery("mail@example") === null);
check("mention_closed_by_space", att.activeMentionQuery("@a b") === null);
check("mention_pick", att.applyMentionPick("总结 @制", "制度清单.md") === "总结 制度清单.md ");

// 5) 长附件不被历史截断削掉
const head = "ATTACH-HEAD-MARKER";
const tail = "ATTACH-TAIL-MARKER";
const body = head + "x".repeat(att.ATTACHMENT_TOTAL_MAX_CHARS - 200) + tail;
const longPrompt = conv.composeConversationPrompt({
  preset: null,
  turns: [{ role: "user", text: "旧上文 OLD-TURN " + "y".repeat(6000) }],
  current: att.composeWithAttachments("按附件写", [item("long.md", body)]),
});
check("long_attachment_head_kept", longPrompt.includes(head), String(longPrompt.length));
check("long_attachment_tail_kept", longPrompt.includes(tail));
check("old_history_still_trimmed", !longPrompt.includes("OLD-TURN"));
check("trim_notice_present", longPrompt.includes("上文已按长度截断"));

// 普通长输入（无附件）同样完整保留
const rawLong = "CURRENT-HEAD " + "z".repeat(4000) + " CURRENT-TAIL";
const rawPrompt = conv.composeConversationPrompt({ preset: null, turns: [], current: rawLong });
check("long_current_complete", rawPrompt.includes("CURRENT-HEAD") && rawPrompt.includes("CURRENT-TAIL"));

console.log(JSON.stringify({ cases, failed: cases.filter((c) => !c.ok).length }));
"""


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=240)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    tmp = Path(tempfile.mkdtemp(prefix="od-attach-"))
    out = tmp / "out"
    compiled = run(
        [
            "npx", "--no-install", "tsc",
            str(SRC / "attachmentPrompt.ts"),
            str(SRC / "conversationPrompt.ts"),
            "--outDir", str(out),
            "--module", "commonjs", "--target", "ES2022",
            "--skipLibCheck", "--ignoreConfig",
        ],
        DESKTOP,
    )
    att_js = out / "attachmentPrompt.js"
    conv_js = out / "conversationPrompt.js"
    if compiled.returncode != 0 or not att_js.exists() or not conv_js.exists():
        print(json.dumps({
            "compile_exit": compiled.returncode,
            "diagnostics": compiled.stdout[-2000:] + compiled.stderr[-2000:],
            "cases": 0, "failed": -1, "result": "fail",
        }, ensure_ascii=False))
        return 1

    probe_path = tmp / "probe.js"
    probe_path.write_text(PROBE, encoding="utf-8")
    probe = run(["node", str(probe_path), str(att_js), str(conv_js)], DESKTOP)
    if probe.returncode != 0:
        print(json.dumps({
            "probe_exit": probe.returncode,
            "probe_stderr": probe.stderr[-2000:],
            "cases": 0, "failed": -1, "result": "fail",
        }, ensure_ascii=False))
        return 1

    parsed = json.loads(probe.stdout.strip().splitlines()[-1])
    payload = {
        "compile_exit": 0,
        "probe_exit": 0,
        "cases": len(parsed["cases"]),
        "failed": parsed["failed"],
        "detail": parsed["cases"],
        "result": "pass" if parsed["failed"] == 0 else "fail",
    }
    if args.json:
        Path(args.json).write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in payload.items() if k != "detail"}, ensure_ascii=False))
    return 0 if payload["result"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
