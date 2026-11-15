#!/usr/bin/env python3
"""OD-ARCH-154：PTC 工具面、工具时间线归并与播种三态的验证入口。

证据等级：
  * patch 与代码结构：静态读取源码；
  * profile_assets 播种三态：实际运行既有 Rust 单测（temp 合成根）；
  * 引擎适配层 callId/工具名提取：直接调用纯函数，合成事件，不启动引擎。

不读取用户数据目录，不写仓库内业务数据。
"""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PATCH = REPO / "samples" / "profiles" / "discipline.profile.patch.yml"
ADAPTER = REPO / "engine" / "omnidesk_engine" / "web_api_adapter.py"
TAURI = REPO / "desktop" / "src-tauri"
# 本机 cargo 由 rustup 装在 ~/.cargo/bin，非登录 shell 的 PATH 可能没有它。
CARGO = shutil.which("cargo") or str(Path.home() / ".cargo" / "bin" / "cargo")

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def block(text: str, start_marker: str, stop_marker: str) -> str:
    start = text.find(start_marker)
    if start < 0:
        return ""
    stop = text.find(stop_marker, start + len(start_marker))
    if stop < 0:
        return text[start:]
    return text[start:stop]


def load_adapter():
    engine_dir = str(REPO / "engine")
    if engine_dir not in sys.path:
        sys.path.insert(0, engine_dir)
    from omnidesk_engine import web_api_adapter

    return web_api_adapter


class AdapterExtractionTest(unittest.TestCase):
    """OD-ARCH-154：合成 DSH 事件，验证只读展示字段的提取链。"""

    @classmethod
    def setUpClass(cls) -> None:
        cls.adapter = load_adapter()

    def payload(self, event_type: str, data: dict) -> dict:
        return self.adapter.WebApiAdapter._tool_activity_payload(
            event_type, data, {"type": event_type, "data": data}
        )

    def test_call_event_reads_top_level_name_and_call_id(self) -> None:
        payload = self.payload("tool/call", {"name": "read_file", "callId": "call-1"})
        self.assertEqual(payload["tool_name"], "read_file")
        self.assertEqual(payload["call_id"], "call-1")
        self.assertEqual(payload["title"], "read_file")

    def test_result_event_reads_message_tool_call_id(self) -> None:
        payload = self.payload(
            "tool/result",
            {"message": {"toolCallId": "call-1", "source": {"callId": "call-1"}}},
        )
        self.assertEqual(payload["tool_name"], "")
        self.assertEqual(payload["call_id"], "call-1")
        self.assertEqual(payload["title"], "")

    def test_result_event_falls_back_to_source_call_id(self) -> None:
        payload = self.payload(
            "tool/result", {"message": {"source": {"callId": "call-2"}}}
        )
        self.assertEqual(payload["call_id"], "call-2")
        self.assertEqual(payload["tool_name"], "")

    def test_payload_never_carries_arguments_or_result_text(self) -> None:
        data = {
            "name": "read_file",
            "callId": "call-3",
            "arguments": {"secret": "绝不能进入事件"},
            "result": "绝不能进入事件",
            "content": "绝不能进入事件",
        }
        payload = self.payload("tool/call", data)
        serialized = json.dumps(payload, ensure_ascii=False)
        self.assertNotIn("绝不能进入事件", serialized)
        self.assertEqual(
            set(payload),
            {"kind", "tool_name", "title", "call_id", "status", "note"},
        )


def run_rust_test(filter_expr: str) -> tuple[bool, str]:
    proc = subprocess.run(
        [CARGO, "test", "--offline", "--lib", filter_expr],
        cwd=TAURI,
        capture_output=True,
        text=True,
    )
    output = (proc.stdout or "") + (proc.stderr or "")
    return proc.returncode == 0, output


def main() -> int:
    text = read(PATCH)
    discipline = block(text, "preset-discipline-core", "preset-work-review-core")
    review = block(text, "preset-work-review-core", "preset-standard")

    check(
        "discipline_core_declares_tool_presentation_ptc",
        "          - id: tool-presentation\n"
        "            name: '@deepseek-ai/dsh-agent-tool-presentation'\n"
        "            config:\n"
        "              mode: ptc\n" in discipline,
        "discipline-core 以两空格层级追加 tool-presentation（mode: ptc）",
    )
    check(
        "discipline_core_declares_present_tool",
        "          - id: present\n"
        "            name: '@deepseek-ai/dsh-tool-present'\n" in discipline,
        "discipline-core 追加 present 工具",
    )
    check(
        "preset_ptc_remains_disabled",
        "- id: preset-ptc\n  disabled: true\n" in text,
        "上游整体预设 preset-ptc 仍保持 disabled",
    )
    check(
        "work_review_core_has_no_ptc",
        review != ""
        and "dsh-agent-tool-presentation" not in review
        and "dsh-tool-present" not in review,
        "work-review-core 保持窄工具面，不挂 PTC",
    )

    # 播种三态：实际跑既有 Rust 单测（temp 合成根，不碰用户数据）。
    updated_ok, updated_out = run_rust_test("previously_written_file_is_updated_with_backup")
    check(
        "seed_previous_ours_is_updated",
        updated_ok and "1 passed" in updated_out,
        "上次由我们写入的 cordis.patch.yml 会被更新为当前出厂内容并留备份",
    )
    drift_ok, drift_out = run_rust_test("user_edit_is_not_overwritten_and_reported_as_drift")
    check(
        "seed_user_edited_patch_is_preserved",
        drift_ok and "1 passed" in drift_out,
        "用户改过的 patch 保留原样并报告 drift",
    )

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(AdapterExtractionTest)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    check(
        "adapter_extraction_chain_unit_tests",
        result.wasSuccessful(),
        f"适配层纯函数单测 {result.testsRun} 条",
    )

    failed = [name for name, ok, _ in CHECKS if not ok]
    for name, ok, detail in CHECKS:
        print(("PASS  " if ok else "FAIL  ") + name + ("  — " + detail if detail and not ok else ""))
    print(f"\n{len(CHECKS) - len(failed)}/{len(CHECKS)} 通过")
    if not updated_ok:
        print("--- seed update test output ---")
        print(updated_out)
    if not drift_ok:
        print("--- seed drift test output ---")
        print(drift_out)
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
