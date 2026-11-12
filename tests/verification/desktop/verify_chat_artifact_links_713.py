#!/usr/bin/env python3
"""OD-UI-713：回复里的成果文件链接（静态接线 + 纯函数行为）。

证据等级：文件事实 + 脚本运行。纯函数用 tsc 单独编译 artifactLinks.ts，
再由 node 跑识别用例；Rust 侧只做源码结构检查（单元测试由 cargo 覆盖）。
不读文件、不访问网络、不启动模型。
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
DESKTOP = REPO / "desktop"
HOME = DESKTOP / "src/shell/HomeChat.tsx"
LINKS = DESKTOP / "src/shell/artifactLinks.ts"
API = DESKTOP / "src/api.ts"
LIB = DESKTOP / "src-tauri/src/lib.rs"
OPEN = DESKTOP / "src-tauri/src/artifact_open.rs"
TSC = DESKTOP / "node_modules/.bin/tsc"

CASES: list[dict[str, object]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": detail})


def behavioral() -> None:
    if not TSC.exists():
        check("tsc_available", False, str(TSC))
        return
    with tempfile.TemporaryDirectory(prefix="od713-links-") as raw_tmp:
        tmp = Path(raw_tmp)
        compiled = subprocess.run(
            [
                str(TSC),
                "--ignoreConfig",
                str(LINKS),
                "--outDir",
                str(tmp),
                "--target",
                "ES2022",
                "--module",
                "ES2022",
                "--moduleResolution",
                "bundler",
                "--skipLibCheck",
            ],
            cwd=str(DESKTOP),
            text=True,
            capture_output=True,
            check=False,
        )
        if compiled.returncode != 0:
            check("links_compile", False, compiled.stderr.strip()[:300])
            return
        check("links_compile", True)
        (tmp / "package.json").write_text('{"type":"module"}\n', encoding="utf-8")
        (tmp / "run.mjs").write_text(
            r"""
import { artifactLinksInText } from "./artifactLinks.js";

const root = "/Users/someone/OmniDesk/workspace/artifacts";
const cases = [];
function expect(name, condition, detail = "") {
  cases.push({ name, ok: condition, detail });
}

const fence = "\u0060\u0060\u0060";
const text = [
  "已生成表格：/Users/someone/OmniDesk/workspace/artifacts/任务分解表.xlsx。",
  "同一条再提一次 /Users/someone/OmniDesk/workspace/artifacts/任务分解表.xlsx",
  "代码示例里不算：",
  fence + "python",
  "open('/Users/someone/OmniDesk/workspace/artifacts/sample.py')",
  fence,
  "目录外的文件 /Users/someone/other/notes.docx 不算",
  "未知扩展名 /Users/someone/OmniDesk/workspace/artifacts/run.sh 不算",
].join("\n");

const links = artifactLinksInText(text, root);
expect("plain_path_found", links.length === 1, JSON.stringify(links));
expect("basename", links[0]?.name === "任务分解表.xlsx", links[0]?.name);
expect("dedup", links.filter((l) => l.path.endsWith("任务分解表.xlsx")).length === 1);
expect("code_fence_ignored", !links.some((l) => l.name === "sample.py"));
expect("outside_root_ignored", !links.some((l) => l.name === "notes.docx"));
expect("unknown_ext_ignored", !links.some((l) => l.name === "run.sh"));

const many = Array.from({ length: 12 }, (_, i) =>
  "文件 /Users/someone/OmniDesk/workspace/artifacts/f" + i + ".md",
).join(" ");
expect("cap_eight", artifactLinksInText(many, root).length === 8);
expect("empty_root", artifactLinksInText(text, "").length === 0);
expect("no_path", artifactLinksInText("纯文本没有路径", root).length === 0);

for (const c of cases) {
  console.log((c.ok ? "PASS " : "FAIL ") + c.name + (c.detail ? " " + c.detail : ""));
}
""",
            encoding="utf-8",
        )
        ran = subprocess.run(
            ["node", str(tmp / "run.mjs")], cwd=str(tmp), text=True, capture_output=True, check=False
        )
        if ran.returncode != 0:
            check("links_run", False, (ran.stderr or ran.stdout).strip()[:300])
            return
        for line in ran.stdout.strip().splitlines():
            if line.startswith("PASS "):
                check(line[5:], True)
            elif line.startswith("FAIL "):
                check(line[5:], False)


def main() -> int:
    for path, name in (
        (HOME, "homechat_exists"),
        (LINKS, "links_exists"),
        (API, "api_exists"),
        (LIB, "lib_exists"),
        (OPEN, "open_exists"),
    ):
        check(name, path.exists(), str(path))
    if not all(p.exists() for p in (HOME, LINKS, API, LIB, OPEN)):
        print(json.dumps({"total": len(CASES), "failed": 1, "result": "fail"}))
        return 1

    home = HOME.read_text(encoding="utf-8")
    links_src = LINKS.read_text(encoding="utf-8")
    api = API.read_text(encoding="utf-8")
    lib = LIB.read_text(encoding="utf-8")
    opener = OPEN.read_text(encoding="utf-8")

    check("home_imports_links", 'from "./artifactLinks"' in home and "artifactLinksInText" in home)
    check("home_reads_root", "localRootInfo()" in home and "artifact_root" in home)
    check("home_merges_task_files", "listGeneratedFiles(sinceMs, untilMs)" in home)
    check("home_renders_links", "<ArtifactLinks" in home and "sinceMs={m.startedAtMs}" in home
          and "untilMs={m.endedAtMs}" in home)
    check("home_click_opens", "shell.openLocalFile(link.path)" in home)
    check("links_strip_code", "stripCodeBlocks" in links_src and (chr(96) * 3) in links_src)
    check("links_ext_whitelist", '"xlsx"' in links_src and '"docx"' in links_src and '"pdf"' in links_src)
    check("links_cap", "links.length < 8" in links_src)
    check("api_command", '"task_open_local_file"' in api and "task_list_generated_files" in api)
    check("lib_register", "task_open_local_file," in lib and "fn task_open_local_file(" in lib
          and "task_list_generated_files," in lib)
    check("open_validated", "resolve_inside" in opener and "starts_with(&root_real)" in opener
          and "is_file()" in opener)
    check("open_no_shell", "open_target" in opener and "Command::new" not in opener)
    check("open_has_tests", "#[cfg(test)]" in opener and "rejects_paths_outside_root_and_traversal" in opener)
    check("open_walker", "list_recent_files" in opener and "WALK_MAX_ENTRIES" in opener
          and "skips_hidden_and_temp_files" in opener)

    behavioral()

    failed = [case for case in CASES if not case["ok"]]
    for case in CASES:
        print("[%s] %s%s" % ("pass" if case["ok"] else "fail", case["name"],
                             (" " + str(case["detail"])) if case["detail"] else ""))
    print("合计 %d 项，失败 %d 项" % (len(CASES), len(failed)))
    print(json.dumps({"total": len(CASES), "failed": len(failed)}, ensure_ascii=False))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
