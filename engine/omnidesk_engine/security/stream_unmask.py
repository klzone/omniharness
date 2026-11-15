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
        # 自然掩码与它们的所有真前缀预先摊平：每个 delta 只查有限长度的尾部。
        self._natural: Tuple[str, ...] = tuple(
            placeholder
            for placeholder, _entry in mapping._entries
            if not PLACEHOLDER_RE.fullmatch(placeholder)
        )
        self._natural_prefixes = frozenset(
            placeholder[:length]
            for placeholder in self._natural
            for length in range(1, len(placeholder))
        )
        self._natural_max = max((len(p) for p in self._natural), default=0)

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

    def _natural_hold(self, text: str) -> Optional[int]:
        """缓存「只露出自然掩码前缀」的尾部（张 / 张* / 芭某 / 芭某公…）。"""

        limit = min(self._natural_max - 1, len(text))
        for length in range(limit, 0, -1):
            if text[-length:] in self._natural_prefixes:
                return len(text) - length
        return None

    def _hold_start(self, text: str) -> Optional[int]:
        """只缓存最后一个可能是占位符开头、且还没有闭合的尾部。"""

        natural = self._natural_hold(text)
        if natural is not None:
            return natural
        name_match = _NAME_TAIL_RE.search(text[-8:])
        if name_match is not None:
            tail = name_match.group(0)
            # 只有映射里确实存在更长的同前缀掩码（张** → 张**2）时才继续等；
            # 否则完整掩码应立即还原，不必等下一帧。
            if tail in self._natural_prefixes:
                return len(text) - len(tail)
        # 不再盲留任意尾部汉字：自然掩码的前缀已由 _natural_hold 按映射覆盖；
        # 映射里没有的尾部汉字本来也无法还原，盲留只会让完整掩码晚一帧下发。
        start = text.rfind("[")
        if start < 0:
            return None
        tail = text[start:]
        if len(tail) > _MAX_HOLD_CHARS:
            return None
        if PLACEHOLDER_RE.fullmatch(tail):
            return None
        return start if _PARTIAL_TAIL_RE.match(tail) else None

