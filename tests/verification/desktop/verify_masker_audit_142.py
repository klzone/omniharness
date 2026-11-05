#!/usr/bin/env python3
"""OD-ARCH-142 S5：出站文本流向审计（静态）。

断言所有对话文本在引擎 submit_task 的同一个汇合点先过 DataMasker，之后才
进入 session/prompt；流式与非流式回程都走同一份会话映射还原。不读真实材料。
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SUPERVISOR = (REPO / "desktop/src-tauri/src/engine_supervisor.rs").read_text(encoding="utf-8")
ADAPTER = (REPO / "engine/omnidesk_engine/web_api_adapter.py").read_text(encoding="utf-8")
TASK = (REPO / "engine/omnidesk_engine/task_core.py").read_text(encoding="utf-8")
HOME = (REPO / "desktop/src/shell/HomeChat.tsx").read_text(encoding="utf-8")


def once(source: str, needle: str) -> bool:
    return source.count(needle) == 1


def main() -> int:
    cases = [
        ("host_passes_dictionary_path", "OMNIDESK_SENSITIVE_DICT" in SUPERVISOR),
        ("engine_default_on", 'OMNIDESK_MASKER' in ADAPTER and '!= "off"' in ADAPTER),
        ("mask_before_worker", ADAPTER.index("self._mask_prompt(text, session_id)") < ADAPTER.index("worker = threading.Thread")),
        ("worker_gets_masked_text", "args=(record, follow, masked_text)" in ADAPTER),
        ("session_prompt_single_exit", once(ADAPTER, '"session/prompt"')),
        ("mask_call_inside_submit", ADAPTER.index("def submit_task") < ADAPTER.index("self._mask_prompt(text, session_id)") < ADAPTER.index("def _turn_worker")),
        ("stream_unmask_before_append", ADAPTER.index("self._unmask_delta(record, raw)") < ADAPTER.index("record.text_parts.append(text)")),
        ("nonstream_unmask", "self._unmask_delta(record, final_text)" in ADAPTER),
        ("result_uses_unmasked_parts", '"content": "".join(record.text_parts)' in TASK),
        ("receipt_reports_counts", "masker=record.mask_stats" in ADAPTER and '"masker": dict(self.masker)' in TASK),
        ("frontend_single_submit_call", once(HOME, "await host.submitTask")),
        ("attachments_inside_prompt", "composeWithAttachments(text, picked)" in HOME),
        ("knowledge_context_inside_prompt", "knowledgeContext," in HOME),
        ("no_ui_masker_switch", "OMNIDESK_MASKER" not in HOME),
        ("unmask_report_reaches_ui", 'p.kind === "masker_unmask"' in HOME),
    ]
    failed = [name for name, ok in cases if not ok]
    for name, ok in cases:
        print(("PASS  " if ok else "FAIL  ") + name)
    print("%d/%d 通过" % (len(cases) - len(failed), len(cases)))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
