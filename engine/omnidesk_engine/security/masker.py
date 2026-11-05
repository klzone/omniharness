"""双向可还原的敏感数据脱敏核心（OD-ARCH-139）。

设计边界：
  * 只依赖 Python 标准库，Python 3.9 可运行；
  * 不保存任何历史映射，映射只存在于单次 ``mask`` 返回值或调用方显式传入的
    :class:`SessionScope`；
  * 不提供把原文导出为 dict/JSON 的公开接口，``MaskMapping.__repr__`` 只显示数量；
  * 本模块不接入 LLM 调用链，也不改动 Rust 侧脱敏逻辑。
"""

from __future__ import annotations

import json
import re
import secrets
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, Optional, Sequence, Set, Tuple

SCHEMA = "omnidesk.sensitive-dict.v1"
ALLOWED_TYPES: Tuple[str, ...] = ("PERSON", "BRANCH", "ORG", "CUSTOM")

_TYPE_KEY_RE = re.compile(r'"type"\s*:')

_IDCARD_RE = re.compile(r"(?<!\d)\d{17}[\dXx](?!\d)")
_BANKCARD_RE = re.compile(r"(?<!\d)\d{13,19}(?!\d)")
_PHONE_RE = re.compile(r"(?<!\d)1[3-9]\d{9}(?!\d)")
_EMAIL_RE = re.compile(
    r"(?<![A-Za-z0-9.!#$%&'*+/=?^_`{|}~-])"
    r"[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]{1,64}@"
    r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?"
    r"(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)+"
    r"(?![A-Za-z0-9-])"
)

_ID_WEIGHTS = (7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2)
_ID_CHECK_CODES = "10X98765432"

_RULE_PRIORITY = {"IDCARD": 0, "BANKCARD": 1, "PHONE": 2, "EMAIL": 3}
_DICT_PRIORITY = 10

_PLACEHOLDER_TYPES = "|".join(("IDCARD", "BANKCARD", "PHONE", "EMAIL", "PERSON", "BRANCH", "ORG", "CUSTOM"))
_PLACEHOLDER_RE = re.compile(r"\[(%s)_([0-9A-F]{8})\]" % _PLACEHOLDER_TYPES)
_PLACEHOLDER_PREFIX_RE = re.compile(r"\[(?:%s)_" % _PLACEHOLDER_TYPES)


def is_valid_idcard(value: str) -> bool:
    """GB11643 校验：17 位数字 + 数字/X 校验码。"""

    if len(value) != 18 or not value[:17].isdigit():
        return False
    last = value[17].upper()
    if last not in "0123456789X":
        return False
    total = sum(int(digit) * weight for digit, weight in zip(value[:17], _ID_WEIGHTS))
    return _ID_CHECK_CODES[total % 11] == last


def is_valid_luhn(value: str) -> bool:
    """Luhn 校验；只接受纯数字。"""

    if not value.isdigit():
        return False
    total = 0
    for index, char in enumerate(reversed(value)):
        digit = int(char)
        if index % 2 == 1:
            digit *= 2
            if digit > 9:
                digit -= 9
        total += digit
    return total % 10 == 0


@dataclass(frozen=True)
class _Candidate:
    type: str
    start: int
    end: int
    original: str
    canonical: str
    priority: int
    order: int = 0

    @property
    def length(self) -> int:
        return self.end - self.start


@dataclass(frozen=True)
class DictEntry:
    """词典中的一个实体。``aliases`` 与 ``value`` 共用同一占位符。"""

    type: str
    value: str
    aliases: Tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.type not in ALLOWED_TYPES:
            raise ValueError("非法敏感类型：%r" % (self.type,))
        if not isinstance(self.value, str) or not self.value.strip():
            raise ValueError("词典 value 不能为空")
        clean: List[str] = []
        for term in (self.value,) + tuple(self.aliases):
            if isinstance(term, str) and term.strip() and term not in clean:
                clean.append(term)
        object.__setattr__(self, "value", clean[0])
        object.__setattr__(self, "aliases", tuple(clean[1:]))

    @property
    def terms(self) -> Tuple[str, ...]:
        """value + aliases 的去重匹配集。"""

        return (self.value,) + tuple(self.aliases)


@dataclass(frozen=True)
class DictionaryLoad:
    entries: Tuple[DictEntry, ...] = ()
    warn: Optional[str] = None

    @property
    def ok(self) -> bool:
        return self.warn is None


def _line_of_type(raw: str, index: int) -> int:
    """第 index 个 ``"type"`` 键所在行；找不到时退化为第 1 行。"""

    matches = list(_TYPE_KEY_RE.finditer(raw))
    if 0 <= index < len(matches):
        return raw.count("\n", 0, matches[index].start()) + 1
    return 1


def load_dictionary(path: Optional[Path]) -> DictionaryLoad:
    """加载 ``omnidesk.sensitive-dict.v1``。

    任何失败都不抛异常：返回空词表 + ``warn``，由 DataMasker 降级为纯规则模式。
    """

    if path is None:
        return DictionaryLoad((), "未提供敏感词典")
    path = Path(path)
    if not path.is_file():
        return DictionaryLoad((), "敏感词典不存在：%s" % path)
    try:
        raw = path.read_text(encoding="utf-8")
    except OSError as exc:
        return DictionaryLoad((), "读取敏感词典失败：%s" % exc)
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        return DictionaryLoad((), "第 %d 行：JSON 解析失败：%s" % (exc.lineno, exc.msg))
    if not isinstance(payload, dict):
        return DictionaryLoad((), "敏感词典顶层必须是对象")
    if payload.get("schema") != SCHEMA:
        return DictionaryLoad(
            (), "schema 必须是 %s，收到 %r" % (SCHEMA, payload.get("schema"))
        )
    entries = payload.get("entries")
    if not isinstance(entries, list):
        return DictionaryLoad((), "entries 必须是数组")

    merged: Dict[Tuple[str, str], List[str]] = {}
    order: List[Tuple[str, str]] = []
    for index, item in enumerate(entries):
        line = _line_of_type(raw, index)
        if not isinstance(item, dict):
            return DictionaryLoad((), "第 %d 行：词典条目必须是对象" % line)
        kind = item.get("type")
        value = item.get("value")
        aliases = item.get("aliases", [])
        if kind not in ALLOWED_TYPES:
            return DictionaryLoad(
                (), "第 %d 行：非法敏感类型 %r（允许：%s）" % (line, kind, ", ".join(ALLOWED_TYPES))
            )
        if not isinstance(value, str) or not value.strip():
            return DictionaryLoad((), "第 %d 行：value 必须是非空字符串" % line)
        if not isinstance(aliases, list) or any(not isinstance(a, str) for a in aliases):
            return DictionaryLoad((), "第 %d 行：aliases 必须是字符串数组" % line)
        key = (kind, value.strip())
        if key not in merged:
            merged[key] = []
            order.append(key)
        for alias in aliases:
            alias = alias.strip()
            if alias and alias != key[1] and alias not in merged[key]:
                merged[key].append(alias)

    loaded: List[DictEntry] = []
    for kind, value in order:
        loaded.append(DictEntry(kind, value, tuple(merged[(kind, value)])))
    return DictionaryLoad(tuple(loaded), None)


@dataclass(frozen=True)
class MappingEntry:
    type: str
    original: str

    def __repr__(self) -> str:
        return "<MappingEntry %s>" % self.type


@dataclass(frozen=True)
class MaskMapping:
    """单次 mask 的占位符 → 原文映射。

    没有 ``to_dict`` / ``json`` 导出接口；不要写入日志、不要随请求外发、不要跨请求复用。
    """

    _entries: Tuple[Tuple[str, MappingEntry], ...] = field(
        default_factory=tuple, repr=False, compare=False
    )

    def __post_init__(self) -> None:
        entries = self._entries
        if isinstance(entries, Mapping):
            entries = tuple(entries.items())
        else:
            entries = tuple(entries)
        object.__setattr__(self, "_entries", entries)

    @classmethod
    def empty(cls) -> "MaskMapping":
        return cls(())

    def __len__(self) -> int:
        return len(self._entries)

    def __contains__(self, placeholder: object) -> bool:
        return any(key == placeholder for key, _ in self._entries)

    def __repr__(self) -> str:
        return "<MaskMapping n=%d>" % len(self._entries)

    def _lookup(self, placeholder: str) -> Optional[MappingEntry]:
        for key, entry in self._entries:
            if key == placeholder:
                return entry
        return None


@dataclass(frozen=True)
class MaskStats:
    found: int = 0
    replaced: int = 0
    overlap_dropped: int = 0
    by_type: Dict[str, int] = field(default_factory=dict)
    warn: Tuple[str, ...] = ()


@dataclass(frozen=True)
class MaskResult:
    text: str
    mapping: MaskMapping
    stats: MaskStats


class SessionScope:
    """调用方持有的会话级占位符表；DataMasker 不保存任何实例级映射。"""

    def __init__(self) -> None:
        self._entities: Dict[Tuple[str, str], str] = {}
        self._placeholders: Set[str] = set()

    def __repr__(self) -> str:
        return "<SessionScope n=%d>" % len(self._entities)


class DataMasker:
    """纯文本脱敏器。

    ``mask`` 返回 :class:`MaskResult`（``text`` / ``mapping`` / ``stats``）。
    映射只在返回值或调用方显式传入的 ``SessionScope`` 里，实例本身不保存历史。
    """

    def __init__(
        self,
        dictionary_path: Optional[Path] = None,
        *,
        entries: Optional[Iterable[DictEntry]] = None,
    ) -> None:
        if dictionary_path is not None and entries is not None:
            raise ValueError("dictionary_path 与 entries 只能二选一")
        if entries is not None:
            self._entries: Tuple[DictEntry, ...] = tuple(entries)
            self._dictionary_warn: Optional[str] = None
        elif dictionary_path is not None:
            loaded = load_dictionary(dictionary_path)
            self._entries = loaded.entries
            self._dictionary_warn = loaded.warn
        else:
            self._entries = ()
            self._dictionary_warn = None

    def _match_rules(self, text: str) -> List[_Candidate]:
        out: List[_Candidate] = []
        for match in _IDCARD_RE.finditer(text):
            value = match.group(0)
            if is_valid_idcard(value):
                out.append(_Candidate("IDCARD", match.start(), match.end(), value, value, 0))
        for match in _BANKCARD_RE.finditer(text):
            value = match.group(0)
            if is_valid_luhn(value):
                out.append(_Candidate("BANKCARD", match.start(), match.end(), value, value, 1))
        for match in _PHONE_RE.finditer(text):
            value = match.group(0)
            out.append(_Candidate("PHONE", match.start(), match.end(), value, value, 2))
        # 先看有没有 @：1MB 的普通文本不必让邮箱正则逐字符回溯。
        if "@" in text:
            for match in _EMAIL_RE.finditer(text):
                value = match.group(0)
                if len(value) <= 254:
                    out.append(_Candidate("EMAIL", match.start(), match.end(), value, value, 3))
        return out

    def _match_dictionary(self, text: str) -> List[_Candidate]:
        out: List[_Candidate] = []
        for entry_index, entry in enumerate(self._entries):
            for term_index, term in enumerate(entry.terms):
                if not term:
                    continue
                for match in re.finditer(re.escape(term), text):
                    out.append(
                        _Candidate(
                            entry.type,
                            match.start(),
                            match.end(),
                            match.group(0),
                            entry.value,
                            _DICT_PRIORITY,
                            entry_index * 100 + term_index,
                        )
                    )
        return out

    @staticmethod
    def _placeholder(kind: str, used: Set[str]) -> str:
        for _ in range(1000):
            candidate = "[%s_%s]" % (kind, secrets.token_hex(4).upper())
            if candidate not in used:
                return candidate
        raise RuntimeError("占位符生成冲突超过 1000 次")

    def mask(self, text: Optional[str], session: Optional[SessionScope] = None) -> MaskResult:
        if text is None:
            text = ""
        if not isinstance(text, str):
            raise TypeError("text 必须是 str 或 None")
        warn: Tuple[str, ...] = (self._dictionary_warn,) if self._dictionary_warn else ()
        candidates = self._match_rules(text) + self._match_dictionary(text)
        candidates.sort(key=lambda c: (c.start, -c.length, c.priority, c.order))

        accepted: List[_Candidate] = []
        end = -1
        for candidate in candidates:
            if candidate.start < end:
                continue
            accepted.append(candidate)
            end = candidate.end

        local_entities: Dict[Tuple[str, str], str] = {}
        local_entries: Dict[str, MappingEntry] = {}
        local_used: Set[str] = set()
        by_type: Dict[str, int] = {}
        pieces: List[str] = []
        cursor = 0
        for candidate in accepted:
            pieces.append(text[cursor:candidate.start])
            key = (candidate.type, candidate.canonical)
            if session is not None and key in session._entities:
                placeholder = session._entities[key]
            elif key in local_entities:
                placeholder = local_entities[key]
            else:
                used = set(session._placeholders) if session is not None else set()
                used.update(local_used)
                placeholder = self._placeholder(candidate.type, used)
                local_entities[key] = placeholder
                local_used.add(placeholder)
                if session is not None:
                    session._entities[key] = placeholder
                    session._placeholders.add(placeholder)
            local_entries.setdefault(
                placeholder, MappingEntry(candidate.type, candidate.canonical)
            )
            by_type[candidate.type] = by_type.get(candidate.type, 0) + 1
            pieces.append(placeholder)
            cursor = candidate.end
        pieces.append(text[cursor:])

        stats = MaskStats(
            found=len(candidates),
            replaced=len(accepted),
            overlap_dropped=len(candidates) - len(accepted),
            by_type=by_type,
            warn=warn,
        )
        return MaskResult("".join(pieces), MaskMapping(local_entries), stats)

    @staticmethod
    def _count_modified(text: str, exact_spans: Sequence[Tuple[int, int]]) -> int:
        """统计形似占位符、但不是完整精确占位符的片段。"""

        if not exact_spans:
            return len(_PLACEHOLDER_PREFIX_RE.findall(text))
        spans = sorted(exact_spans)
        modified = 0
        index = 0
        for match in _PLACEHOLDER_PREFIX_RE.finditer(text):
            start = match.start()
            while index < len(spans) and spans[index][1] <= start:
                index += 1
            if index < len(spans) and spans[index][0] <= start < spans[index][1]:
                continue
            modified += 1
        return modified

    def unmask(self, masked: Optional[str], mapping: MaskMapping) -> Tuple[str, Dict[str, object]]:
        """按显式传入的 mapping 还原。

        未知或格式被改动的占位符原样保留并写进 report；``ok=False`` 时，调用方不得
        把返回文本当作“已完整还原”的内容使用。
        """

        if masked is None:
            masked = ""
        if not isinstance(masked, str):
            raise TypeError("masked 必须是 str 或 None")
        if not isinstance(mapping, MaskMapping):
            raise TypeError("mapping 必须是 MaskMapping")
        pieces: List[str] = []
        cursor = 0
        restored = 0
        unknown: List[str] = []
        exact_spans: List[Tuple[int, int]] = []
        for match in _PLACEHOLDER_RE.finditer(masked):
            exact_spans.append((match.start(), match.end()))
            pieces.append(masked[cursor:match.start()])
            placeholder = match.group(0)
            kind = match.group(1)
            entry = mapping._lookup(placeholder)
            if entry is not None and entry.type == kind:
                pieces.append(entry.original)
                restored += 1
            else:
                pieces.append(placeholder)
                unknown.append(placeholder)
            cursor = match.end()
        pieces.append(masked[cursor:])
        report: Dict[str, object] = {
            "restored": restored,
            "unknown_placeholders": unknown,
            "modified_candidates": self._count_modified(masked, exact_spans),
            "ok": not unknown,
        }
        return "".join(pieces), report

    def validate_mapping(self, masked: str, mapping: MaskMapping) -> Dict[str, object]:
        """只检查完整性，不修改文本。"""

        if not isinstance(masked, str):
            raise TypeError("masked 必须是 str")
        if not isinstance(mapping, MaskMapping):
            raise TypeError("mapping 必须是 MaskMapping")
        unknown: List[str] = []
        exact_spans: List[Tuple[int, int]] = []
        by_type: Dict[str, int] = {}
        total = 0
        for match in _PLACEHOLDER_RE.finditer(masked):
            total += 1
            exact_spans.append((match.start(), match.end()))
            placeholder = match.group(0)
            kind = match.group(1)
            entry = mapping._lookup(placeholder)
            if entry is None or entry.type != kind:
                unknown.append(placeholder)
            else:
                by_type[kind] = by_type.get(kind, 0) + 1
        return {
            "ok": not unknown,
            "unknown": unknown,
            "missing_in_mapping": len(unknown),
            "stats": {
                "total_placeholders": total,
                "mapped": total - len(unknown),
                "unknown": len(unknown),
                "modified_candidates": self._count_modified(masked, exact_spans),
                "by_type": by_type,
            },
        }

    # ------------------------------------------------------------------
    # Markdown / Obsidian 结构适配（OD-ARCH-140）
    # ------------------------------------------------------------------

    def mask_markdown(
        self, text: Optional[str], session: Optional[SessionScope] = None
    ) -> MaskResult:
        """按区域策略脱敏 Markdown；非脱敏区逐字节原样拼回。"""

        if text is None:
            text = ""
        if not isinstance(text, str):
            raise TypeError("text 必须是 str 或 None")
        scan = scan_markdown(text)
        scope = session if session is not None else SessionScope()
        out: List[str] = []
        mapping_entries: Dict[str, MappingEntry] = {}
        found = replaced = overlap = 0
        by_type: Dict[str, int] = {}

        for start, end, kind in scan.regions:
            if kind not in ("frontmatter", "text", "wikilink_display"):
                out.append(text[start:end])
                continue
            cursor = start
            for protected_start, protected_end in scan.protected:
                if protected_end <= start or protected_start >= end:
                    continue
                protected_start = max(protected_start, start)
                protected_end = min(protected_end, end)
                if protected_start > cursor:
                    chunk = self.mask(text[cursor:protected_start], session=scope)
                    out.append(chunk.text)
                    mapping_entries.update(dict(chunk.mapping._entries))
                    found += chunk.stats.found
                    replaced += chunk.stats.replaced
                    overlap += chunk.stats.overlap_dropped
                    for key, value in chunk.stats.by_type.items():
                        by_type[key] = by_type.get(key, 0) + value
                out.append(text[protected_start:protected_end])
                cursor = protected_end
            if cursor < end:
                chunk = self.mask(text[cursor:end], session=scope)
                out.append(chunk.text)
                mapping_entries.update(dict(chunk.mapping._entries))
                found += chunk.stats.found
                replaced += chunk.stats.replaced
                overlap += chunk.stats.overlap_dropped
                for key, value in chunk.stats.by_type.items():
                    by_type[key] = by_type.get(key, 0) + value

        warn: List[str] = []
        if self._dictionary_warn:
            warn.append(self._dictionary_warn)
        warn.extend(scan.warnings)
        stats = MaskStats(
            found=found,
            replaced=replaced,
            overlap_dropped=overlap,
            by_type=by_type,
            warn=tuple(dict.fromkeys(warn)),
        )
        return MaskResult("".join(out), MaskMapping(mapping_entries), stats)

    def unmask_markdown(self, masked: Optional[str], mapping: MaskMapping) -> Tuple[str, Dict[str, object]]:
        """全篇扫描还原；占位符被模型挪到别的区域也照常还原。"""

        return self.unmask(masked, mapping)


@dataclass(frozen=True)
class MarkdownScan:
    regions: Tuple[Tuple[int, int, str], ...] = ()
    protected: Tuple[Tuple[int, int], ...] = ()
    warnings: Tuple[str, ...] = ()


_FENCE_LINE_RE = re.compile(r"(?m)^[ ]{0,3}(`{3,}|~{3,})")
_TAG_RE = re.compile(r"(?<!\w)#([\w\-/]+)")
_URL_RE = re.compile(r"https?://[^\s<>\"'`]+")


def _line_end(text: str, start: int) -> int:
    at = text.find("\n", start)
    return len(text) if at < 0 else at + 1


def _line_text(text: str, start: int, end: int) -> str:
    return text[start:end].rstrip("\r\n")


def _frontmatter_span(text: str) -> Optional[Tuple[str, int, int, int]]:
    """返回 (status, end, open_end, close_start)；status=none/open/unclosed。"""

    if text.startswith("\ufeff") or not text.startswith("---"):
        return None
    open_end = _line_end(text, 0)
    if _line_text(text, 0, open_end).rstrip("\r") != "---":
        return None
    pos = open_end
    while pos < len(text):
        end = _line_end(text, pos)
        if _line_text(text, pos, end).rstrip() in ("---", "..."):
            return ("open", end, open_end, pos)
        pos = end
    return ("unclosed", len(text), open_end, len(text))


def _fence_open(text: str, start: int) -> Optional[Tuple[str, int, int]]:
    if start > 0 and text[start - 1] != "\n":
        return None
    end = _line_end(text, start)
    line = _line_text(text, start, end)
    match = re.match(r"^[ ]{0,3}(`{3,}|~{3,})", line)
    if not match:
        return None
    marker = match.group(1)[0]
    return marker, len(match.group(1)), end


def _code_block_end(text: str, start: int, marker: str, run: int, first_end: int) -> Tuple[int, bool]:
    pos = first_end
    while pos < len(text):
        end = _line_end(text, pos)
        line = _line_text(text, pos, end).strip()
        if line and set(line) == {marker} and len(line) >= run:
            return end, False
        pos = end
    return len(text), True


def _inline_code_end(text: str, start: int) -> Tuple[int, bool]:
    end = _line_end(text, start)
    run = 1
    while start + run < end and text[start + run] == "`":
        run += 1
    pos = start + run
    while pos < end:
        if text[pos] != "`":
            pos += 1
            continue
        close = pos
        while close < end and text[close] == "`":
            close += 1
        if close - pos == run:
            return close, False
        pos = close
    return end, True


def _wikilink_at(text: str, start: int) -> Optional[Tuple[List[Tuple[int, int, str]], int]]:
    embed = text.startswith("![[", start)
    open_len = 3 if embed else 2
    close = text.find("]]", start + open_len)
    if close < 0:
        return None
    inner_start = start + open_len
    inner = text[inner_start:close]
    if "\n" in inner:
        return None
    end = close + 2
    if embed:
        return [(start, end, "wikilink_target")], end
    pipe = inner.find("|")
    if pipe < 0:
        return [(start, end, "wikilink_target")], end
    target_end = inner_start + pipe + 1
    regions: List[Tuple[int, int, str]] = [(start, target_end, "wikilink_target")]
    if target_end < close:
        regions.append((target_end, close, "wikilink_display"))
    regions.append((close, end, "wikilink_target"))
    return regions, end


def _protected_spans(text: str, start: int, end: int) -> List[Tuple[int, int]]:
    spans: List[Tuple[int, int]] = []
    for match in _TAG_RE.finditer(text, start, end):
        spans.append((match.start(), match.end()))
    for match in _URL_RE.finditer(text, start, end):
        spans.append((match.start(), match.end()))
    return spans


def _merge_spans(spans: Iterable[Tuple[int, int]]) -> Tuple[Tuple[int, int], ...]:
    ordered = sorted(spans)
    merged: List[Tuple[int, int]] = []
    for start, end in ordered:
        if start >= end:
            continue
        if merged and start <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return tuple(merged)


def _next_special(text: str, start: int) -> int:
    candidates: List[int] = []
    for token in ("`", "[[", "![["):
        at = text.find(token, start)
        if at >= 0:
            candidates.append(at)
    fence = _FENCE_LINE_RE.search(text, start)
    if fence:
        candidates.append(fence.start())
    return min(candidates) if candidates else len(text)


def scan_markdown(text: str) -> MarkdownScan:
    """切分 Markdown/Obsidian 区域；区间有序、互不重叠、并集覆盖全文。"""

    if not isinstance(text, str):
        raise TypeError("text 必须是 str")
    if not text:
        return MarkdownScan()
    regions: List[Tuple[int, int, str]] = []
    protected: List[Tuple[int, int]] = []
    warnings: List[str] = []
    length = len(text)
    pos = 0

    front = _frontmatter_span(text)
    if front is not None:
        status, end, open_end, close_start = front
        if status == "unclosed":
            warnings.append("frontmatter_unclosed")
            regions.append((0, length, "text"))
            protected.extend(_protected_spans(text, 0, length))
            return MarkdownScan(tuple(regions), _merge_spans(protected), tuple(warnings))
        regions.append((0, end, "frontmatter"))
        # 两条 --- 围栏本身保留；只脱敏中间内容。
        protected.append((0, open_end))
        protected.append((close_start, end))
        protected.extend(_protected_spans(text, open_end, close_start))
        pos = end

    while pos < length:
        fence = _fence_open(text, pos)
        if fence is not None:
            marker, run, first_end = fence
            end, unclosed = _code_block_end(text, pos, marker, run, first_end)
            if unclosed:
                warnings.append("code_unclosed")
            regions.append((pos, end, "code"))
            pos = end
            continue
        if text[pos] == "`":
            end, unclosed = _inline_code_end(text, pos)
            if unclosed:
                warnings.append("inline_code_unclosed")
            regions.append((pos, end, "inline_code"))
            pos = end
            continue
        if text.startswith("![[", pos) or text.startswith("[[", pos):
            link = _wikilink_at(text, pos)
            if link is not None:
                link_regions, end = link
                regions.extend(link_regions)
                for start, finish, kind in link_regions:
                    if kind == "wikilink_display":
                        protected.extend(_protected_spans(text, start, finish))
                pos = end
                continue
            next_pos = _next_special(text, pos + 1)
            regions.append((pos, next_pos, "text"))
            protected.extend(_protected_spans(text, pos, next_pos))
            pos = next_pos
            continue

        next_pos = _next_special(text, pos)
        if next_pos <= pos:
            next_pos = pos + 1
        regions.append((pos, next_pos, "text"))
        protected.extend(_protected_spans(text, pos, next_pos))
        pos = next_pos

    return MarkdownScan(tuple(regions), _merge_spans(protected), tuple(dict.fromkeys(warnings)))


def scan_markdown_regions(text: str) -> List[Tuple[int, int, str]]:
    """公开的区间列表入口（测试与上层检查用）。"""

    return list(scan_markdown(text).regions)
