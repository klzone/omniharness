"""流式回程占位符还原（OD-ARCH-142 S3）。

模型返回的 text-delta 可能把 ``[PERSON_ABCD1234]`` 切成多段。本模块只缓存
可能是占位符前缀的尾部，完整占位符到了就立刻查表还原，未识别占位符原样保留。
"""

from __future__ import annotations

import re
from typing import Dict, List, Optional, Tuple

from .masker import DataMasker, MaskMapping

PLACEHOLDER_RE = re.compile(
    r"\[(?:IDCARD|BANKCARD|PHONE|EMAIL|PERSON|BRANCH|ORG|CUSTOM)_[0-9A-F]{8}\]"
)
_PARTIAL_TAIL_RE = re.compile(r"\[(?:[A-Z]{0,8})?(?:_[0-9A-F]{0,8})?$")
# 人名占位是普通文本（张** / 张**2），流式里可能被切在星号或序号中间。
_NAME_TAIL_RE = re.compile(r"[一-龥]\*{1,2}\d*$")
_MAX_HOLD_CHARS = 80


class StreamUnmasker:
    """按会话映射还原跨帧占位符；report 只含计数与占位符本身。"""

    def __init__(self, masker: DataMasker, mapping: MaskMapping) -> None:
        self._masker = masker
        self._mapping = mapping
        self._buffer = ""
        self._restored = 0
        self._unknown: List[str] = []
        self._modified = 0

    @property
    def report(self) -> Dict[str, object]:
        return {
            "ok": not self._unknown,
            "restored": self._restored,
            "unknown_placeholders": list(self._unknown),
            "modified_candidates": self._modified,
        }

    def feed(self, text: str) -> str:
        """吃一段增量，返回现在可以安全下发的文本（可能为空）。"""

        if not text:
            return self._drain(final=False)
        self._buffer += text
        return self._drain(final=False)

    def flush(self) -> str:
        """本轮结束：把缓冲里的残余全部下发；未闭合片段按原文保留。"""

        return self._drain(final=True)

    def _drain(self, *, final: bool) -> str:
        if not self._buffer:
            return ""
        hold = None if final else self._hold_start(self._buffer)
        if hold is None:
            safe = self._buffer
            self._buffer = ""
        else:
            safe = self._buffer[:hold]
            self._buffer = self._buffer[hold:]
        if not safe:
            return ""
        text, report = self._masker.unmask(safe, self._mapping)
        self._restored += int(report.get("restored") or 0)
        self._unknown.extend(str(x) for x in report.get("unknown_placeholders") or [])
        self._modified += int(report.get("modified_candidates") or 0)
        return text

    @staticmethod
    def _hold_start(text: str) -> Optional[int]:
        """只缓存最后一个可能是占位符开头、且还没有闭合的尾部。"""

        name_match = _NAME_TAIL_RE.search(text[-8:])
        if name_match is not None:
            return len(text) - len(name_match.group(0))
        # 人名占位可能刚露出一个姓字，下一帧才是星号：先留住最后一个汉字。
        if text and "一" <= text[-1] <= "龥":
            return len(text) - 1
        start = text.rfind("[")
        if start < 0:
            return None
        tail = text[start:]
        if len(tail) > _MAX_HOLD_CHARS:
            return None
        if PLACEHOLDER_RE.fullmatch(tail):
            return None
        return start if _PARTIAL_TAIL_RE.match(tail) else None

