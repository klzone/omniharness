"""OmniDesk 敏感数据脱敏模块（OD-ARCH-139）。

对外只暴露 DataMasker、SessionScope 与 MaskMapping；内部实现与词典解析在
``masker.py``。本包只依赖 Python 标准库，不接入 LLM 调用链。
"""

from .masker import (
    ALLOWED_TYPES,
    DictEntry,
    DictionaryLoad,
    DataMasker,
    MaskMapping,
    MaskResult,
    MaskStats,
    SessionScope,
    load_dictionary,
)

__all__ = [
    "ALLOWED_TYPES",
    "DataMasker",
    "DictEntry",
    "DictionaryLoad",
    "MaskMapping",
    "MaskResult",
    "MaskStats",
    "SessionScope",
    "load_dictionary",
]
