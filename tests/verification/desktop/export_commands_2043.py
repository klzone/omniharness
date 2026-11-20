#!/usr/bin/env python3
"""OD-HARDEN-2043 任务 D：导出 lib.rs 的命令名基线。

用法：python3 export_commands_2043.py [--write 基线文件路径]

输出 JSON：{"count": N, "names": [...], "handler_count": N, "handler_names": [...]}
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
LIB = ROOT / "desktop" / "src-tauri" / "src" / "lib.rs"
SRC = ROOT / "desktop" / "src-tauri" / "src"


def command_names(text: str) -> list[str]:
    names = re.findall(
        r"#\[tauri::command[^]]*\]\s*(?:pub(?:\([^)]*\))?\s*)?(?:async )?fn ([a-zA-Z0-9_]+)\(",
        text,
    )
    return sorted(set(names))


def handler_names(text: str) -> list[str]:
    start = text.find("tauri::generate_handler![")
    if start < 0:
        return []
    end = text.find("])", start)
    block = text[start:end]
    found = re.findall(r"\b([a-zA-Z0-9_:]+)\s*,", block)
    return sorted(set(name.split("::")[-1] for name in found if name != "tauri"))


def main() -> int:
    text = LIB.read_text(encoding="utf-8")
    commands = []
    for path in sorted(SRC.rglob("*.rs")):
        commands.extend(command_names(path.read_text(encoding="utf-8")))
    commands = sorted(set(commands))
    handlers = handler_names(text)
    payload = {
        "count": len(commands),
        "names": commands,
        "handler_count": len(handlers),
        "handler_names": handlers,
        "missing_in_handler": sorted(set(commands) - set(handlers)),
        "missing_in_commands": sorted(set(handlers) - set(commands)),
    }
    if "--write" in sys.argv:
        target = pathlib.Path(sys.argv[sys.argv.index("--write") + 1])
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {target} count={payload['count']} handler={payload['handler_count']}")
    else:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
