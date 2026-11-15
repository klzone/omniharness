"""OmniDesk 敏感数据脱敏模块（OD-ARCH-139）。

对外暴露 DataMasker、SessionScope、MaskMapping，以及词典/阻断词加载与检查
（OD-MASK-160）；内部实现与词典解析在 ``masker.py``。本包只依赖 Python 标准库。
"""

from .masker import (
    ALLOWED_TYPES,
    ALLOWED_BLOCK_CATEGORIES,
    BlockEntry,
    DictEntry,
    DictionaryLoad,
    DataMasker,
    MaskMapping,
    MaskResult,
    MaskStats,
    SessionScope,
    check_blocklist,
    load_dictionary,
)

__all__ = [
    "ALLOWED_TYPES",
    "ALLOWED_BLOCK_CATEGORIES",
    "BlockEntry",
    "DataMasker",
    "DictEntry",
    "DictionaryLoad",
    "MaskMapping",
    "MaskResult",
    "MaskStats",
    "SessionScope",
    "check_blocklist",
    "load_dictionary",
]
