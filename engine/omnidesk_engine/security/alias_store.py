"""稳定别名映射（OD-ARCH-185）。

跨会话、跨文档保持同一实体到同一掩码串（张瑞山 → 张** 且长期不变），
使写入本地文档的掩码形态在换会话后仍可还原。文件只存
(类型, 原文, 别名) 三段，权限 0600，原子写；不写日志、不外发。
金额类不持久化（取值组合可能无限，避免映射表失控）。
"""

from __future__ import annotations

import json
import os
import threading
from pathlib import Path
from typing import Callable, Dict, Optional, Set, Tuple

SCHEMA = "omnidesk.alias-map.v1"
_PERSISTED_TYPES = (
    "PERSON",
    "NAME",
    "BRANCH",
    "ORG",
    "CUSTOM",
    "BANKCARD",
    "PHONE",
    "IDCARD",
    "EMAIL",
)


class AliasStore:
    """稳定别名表；同一进程内可并发调用（内部 RLock 串行）。"""

    def __init__(self, path: Optional[Path] = None) -> None:
        self._path = Path(path) if path else None
        self._lock = threading.RLock()
        self._entries: Dict[Tuple[str, str], str] = {}
        self._reverse: Dict[str, Tuple[str, str]] = {}
        self._used: Dict[str, Set[str]] = {}
        self._load()

    def _load(self) -> None:
        if self._path is None or not self._path.exists():
            return
        try:
            data = json.loads(self._path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return
        entries = data.get("entries") if isinstance(data, dict) else None
        if not isinstance(entries, list):
            return
        for item in entries:
            if not isinstance(item, dict):
                continue
            kind = str(item.get("type") or "").strip()
            original = str(item.get("original") or "")
            alias = str(item.get("alias") or "").strip()
            if not kind or not original or not alias:
                continue
            if kind not in _PERSISTED_TYPES:
                continue
            key = (kind, original)
            if key in self._entries:
                continue
            if alias in self._reverse:
                continue
            self._entries[key] = alias
            self._reverse[alias] = (kind, original)
            self._used.setdefault(kind, set()).add(alias)

    def _all_used(self) -> Set[str]:
        used: Set[str] = set()
        for values in self._used.values():
            used.update(values)
        return used

    def _save_locked(self) -> None:
        if self._path is None:
            return
        try:
            self._path.parent.mkdir(parents=True, exist_ok=True)
            payload = {
                "schema": SCHEMA,
                "entries": [
                    {"type": kind, "original": original, "alias": alias}
                    for (kind, original), alias in sorted(self._entries.items())
                ],
            }
            tmp = self._path.with_name(self._path.name + ".tmp")
            tmp.write_text(json.dumps(payload, ensure_ascii=False, indent=1), encoding="utf-8")
            try:
                os.chmod(tmp, 0o600)
            except OSError:
                pass
            os.replace(tmp, self._path)
            try:
                os.chmod(self._path, 0o600)
            except OSError:
                pass
        except OSError:
            pass

    def used(self, kind: str) -> Set[str]:
        with self._lock:
            return set(self._used.get(kind, ()))

    def alias_for(
        self,
        kind: str,
        original: str,
        propose: Callable[[Set[str]], str],
    ) -> Optional[str]:
        """取得或分配实体别名；propose 接收已占用别名集合，返回候选。"""

        if kind not in _PERSISTED_TYPES or self._path is None:
            return None
        with self._lock:
            alias = self._entries.get((kind, original))
            if alias is not None:
                return alias
            used = self._all_used()
            candidate = str(propose(used) or "").strip()
            if not candidate or candidate in used:
                return None
            self._entries[(kind, original)] = candidate
            self._reverse[candidate] = (kind, original)
            self._used.setdefault(kind, set()).add(candidate)
            self._save_locked()
            return candidate

    def origin_for(self, alias: str) -> Optional[str]:
        with self._lock:
            hit = self._reverse.get(alias)
            return hit[1] if hit else None

    def aliases(self) -> Tuple[Tuple[str, str], ...]:
        """返回 (别名, 原文) 列表；调用方不得把结果写进日志。"""

        with self._lock:
            return tuple(
                (alias, original)
                for alias, (_kind, original) in self._reverse.items()
            )

    def __repr__(self) -> str:
        return "<AliasStore n=%d>" % len(self._entries)
