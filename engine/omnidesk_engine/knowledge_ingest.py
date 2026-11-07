"""知识库整理提示词组装（OD-ARCH-141 S2）。

纯函数 + 可选的 ``python -m`` CLI；不读磁盘、不启动 DSH、不调用模型。
原文先走 Task 1/2 的 markdown 脱敏，再把脱敏文本交给模型。映射只存在于
``IngestPrompt`` 返回值里，不序列化、不落盘。
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Mapping, Optional, Sequence, Tuple

from .security.masker import DataMasker, MaskMapping, SessionScope

JSON_OUTPUT_SPEC = """只输出一个 JSON 对象，不要 Markdown 代码围栏，不要解释文字。
结构必须是：
{
  "cards": [
    {
      "relative_path": "知识库/概念/卡片标题.md 或 知识库/方法/卡片标题.md",
      "title": "卡片标题（拆分时用 标题（上）/（中）/（下））",
      "content": "完整的 Markdown 卡片（含 frontmatter 与正文）",
      "sources": ["inbox/来源文件.md 或 raw/来源文件.pdf"]
    }
  ],
  "index_lines": [
    "- [卡片标题](概念/卡片标题.md) | 标签：标签1, 标签2 | 一句话摘要"
  ],
  "archive_moves": [
    {"from": "inbox/原文件.md", "to": "知识库/原文/原文件.md"}
  ]
}

约束：
- 卡片只能落在 知识库/概念/ 或 知识库/方法/；正文不超过 320 字（不含 frontmatter）。
- 320 字是写作目标也是拆卡触发点：超过 320 字时必须拆成多张卡，标题统一用「原标题（上）/（中）/（下）」后缀；每张拆分卡必须有独立 title、relative_path、index_line、sources，并在 index_lines 用「参见：」互相链接。宿主只在单卡超过 420 字时拒收该卡。
- 拆出的每张卡正文同样 ≤320 字；长文按主题段先拆再压，不允许拆完仍超。
- 一张卡只回答一个问题；标题不要与现有索引重复。
- sources 必须指向本批次真实存在的文件（inbox/ 或 raw/），不得指向尚未归档的 知识库/原文/。
- index_lines 必须与 cards 一一对应；archive_moves 只允许把 inbox/ 下的原文移到 知识库/原文/。
"""


@dataclass(frozen=True)
class IngestFile:
    relative_path: str
    content: str


@dataclass(frozen=True)
class IngestPrompt:
    prompt: str
    masked_files: Tuple[Tuple[str, str], ...]
    mappings: Tuple[Tuple[str, MaskMapping], ...]


def assemble_ingest_prompt(
    files: Sequence[Mapping[str, str]],
    index: str,
    card_spec: str,
    masker: Optional[DataMasker] = None,
) -> IngestPrompt:
    """只对收到的材料做脱敏，再组装可交给模型的整理提示词。"""

    active = masker if masker is not None else DataMasker()
    # 同一批材料共用一个会话作用域：同一实体跨文件复用同一占位符，
    # 模型不会把「同一个人」看成两个不同对象。
    scope = SessionScope()
    masked: List[Tuple[str, str]] = []
    mappings: List[Tuple[str, MaskMapping]] = []
    for item in files:
        relative_path = str(item.get("relative_path") or "").strip()
        content = str(item.get("content") or "")
        result = active.mask_markdown(content, session=scope)
        masked.append((relative_path, result.text))
        mappings.append((relative_path, result.mapping))

    parts: List[str] = [
        "你是 OmniDesk 的知识库整理助手。把收件箱材料整理成可检索、可引用的知识卡。",
        "",
        "【建卡规范（必须完整遵守）】",
        card_spec.strip(),
        "",
        "【当前索引】",
        index.strip() or "（索引为空）",
        "",
        "【待整理材料（inbox/ 与 raw/；已脱敏；占位符不是原文，不得自行还原或猜测）】",
    ]
    for relative_path, text in masked:
        parts.extend(["## 文件：" + relative_path, text.rstrip(), ""])
    parts.extend(["【输出约束】", JSON_OUTPUT_SPEC.strip(), ""])
    return IngestPrompt("\n".join(parts), tuple(masked), tuple(mappings))


def main(argv: Optional[Sequence[str]] = None) -> int:
    """stdin JSON → stdout prompt JSON；只供宿主一次性组装，不保存映射。"""

    raw = sys.stdin.read()
    try:
        payload = json.loads(raw)
        files = payload.get("files") or []
        dictionary = payload.get("dictionary_path")
        masker = DataMasker(dictionary_path=Path(dictionary)) if dictionary else DataMasker()
        result = assemble_ingest_prompt(
            files,
            str(payload.get("index") or ""),
            str(payload.get("card_spec") or ""),
            masker,
        )
        output = {
            "prompt": result.prompt,
            "files": [
                {"relative_path": path, "masked_chars": len(text)}
                for path, text in result.masked_files
            ],
        }
        sys.stdout.write(json.dumps(output, ensure_ascii=False))
        return 0
    except Exception as exc:  # CLI 只把可读原因交给宿主，不输出原文。
        sys.stderr.write("知识库整理提示词组装失败：%s\n" % exc)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
