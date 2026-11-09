"""写作需求 → 结构化写作任务草稿提示词（OD-WT-604）。

纯函数 + 可选 python -m CLI：只读 stdin、只写 stdout，不读磁盘、不启动 DSH、
不调用模型、不写文件。模型调用仍由界面侧提交；宿主只负责提示词组装与输出校验。

分工与 504/506 先例一致：
  * 本模块给出提示词与形状校验，缺失关键条件一律列入 questions，不臆测；
  * 宿主 writing_parse.rs 独立复校验白名单、长度与缺失项；
  * 候选草稿不落盘，用户确认后才由宿主创建写作任务。
"""

from __future__ import annotations

import json
import sys
from typing import Any, Mapping, Optional

EXTRACT_SCHEMA = "omnidesk.writing-requirement.v1"

# 十二类文稿白名单，与 desktop/src-tauri/src/writing_store.rs 的 DOCUMENT_TYPES 对齐。
DOCUMENT_TYPES = (
    "月度总结",
    "年度总结",
    "专项汇报",
    "经验交流",
    "典型案例",
    "调研报告",
    "工作方案",
    "领导讲话",
    "工作简报",
    "专题分析",
    "会议发言",
    "个人述职",
)

MAX_TITLE_CHARS = 200
MAX_TEXT_FIELD_CHARS = 400
MAX_REQUIREMENTS = 12
MAX_REQUIREMENT_CHARS = 200
MAX_QUESTIONS = 10
MAX_QUESTION_CHARS = 200
MAX_REQUEST_TEXT_CHARS = 20_000

# 缺失这些关键条件时，必须把对应问题列进 questions。
CORE_TEXT_FIELDS = (
    "title",
    "topic",
    "writing_purpose",
    "audience",
    "writing_period",
    "expected_length",
)

JSON_OUTPUT_SPEC = """只输出一个 JSON 对象，不要 Markdown 代码围栏，不要解释文字。
结构必须是：
{
  "document_type": "月度总结",
  "title": "建议标题",
  "topic": "写作主题",
  "writing_purpose": "写作目的",
  "audience": "目标受众",
  "writing_period": "时间范围",
  "expected_length": "预计篇幅",
  "writing_style": "写作风格",
  "requirements": ["用户明确提出的写作要求"],
  "questions": ["需要用户补充或确认的问题"],
  "confidence": 0.7
}

约束：
- document_type 只能填下列十二类之一；无法确定时填 null，不得猜测，也不得写空字符串。
- 用户没有说到的字符串字段一律填 null；不要编造主题、目的、受众、时间范围、篇幅或风格，也不要写「待补充」「未知」这类占位词。
- title 是建议标题，不超过 200 字；其余字符串字段每条不超过 400 字，不得含换行。
- requirements 是用户明确提出的要点列表，最多 12 条，每条不超过 200 字；不得把模型补充的建议包装成用户要求。没有明确要求时给空数组。
- questions 必须输出数组，最多 10 条，每条不超过 200 字。凡是缺失的关键条件（文稿类型、标题、主题、写作目的、受众、时间范围、篇幅）都必须逐条写入 questions，用一句完整的问题表达缺什么，不得在草稿字段里臆测。
- document_type 为 null 时，questions 必须至少包含一条询问文稿类型（文种/文体）的问题，例如「这份材料属于哪种文稿类型？」。
- 信息齐全时可以给空数组；只要还有关键条件缺失，questions 不得为空。
- confidence 取 0 到 1，只在用户需求本身足以支撑草稿时给高值；信息不足时调低。
- 只输出草稿与问题，不创建任务、不写入任何文件、不确认任何状态、不调用任何工具。
"""


def _clean_text_field(
    value: Any, field: str, limit: int
) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise ValueError(f"写作需求结果的 {field} 必须是字符串或 null。")
    trimmed = value.strip()
    if not trimmed:
        return None
    if "\n" in trimmed:
        raise ValueError(f"写作需求结果的 {field} 不得含换行。")
    if len(trimmed) > limit:
        raise ValueError(
            f"写作需求结果的 {field} 过长（最多 {limit} 字）。"
        )
    return trimmed


def _clean_string_list(
    value: Any,
    field: str,
    limit: int,
    max_items: int,
) -> list[str]:
    if not isinstance(value, list):
        raise ValueError(f"写作需求结果的 {field} 必须是数组。")
    out: list[str] = []
    for item in value:
        if not isinstance(item, str):
            raise ValueError(f"写作需求结果的 {field} 里每一项都必须是字符串。")
        trimmed = item.strip()
        if not trimmed:
            raise ValueError(f"写作需求结果的 {field} 里不能有空条目。")
        if "\n" in trimmed:
            raise ValueError(f"写作需求结果的 {field} 条目不得含换行。")
        if len(trimmed) > limit:
            raise ValueError(
                f"写作需求结果的 {field} 条目过长（最多 {limit} 字）。"
            )
        if trimmed not in out:
            out.append(trimmed)
    if len(out) > max_items:
        raise ValueError(
            f"写作需求结果的 {field} 过多（最多 {max_items} 条）。"
        )
    return out


def _mentions_document_type(question: str) -> bool:
    return any(
        marker in question
        for marker in ("文稿类型", "文档类型", "文种", "文体", "哪种类型")
    )


def validate_writing_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    """校验模型输出的写作需求草稿形状。

    纯形状检查：宿主 writing_parse.rs 会独立复校验同一份白名单与长度上限。
    关键条件缺失时必须列入 questions；类型无法确定时 document_type 置 null，
    且 questions 必含一条类型询问。
    """

    if not isinstance(payload, Mapping):
        raise ValueError("写作需求解析结果必须是 JSON 对象。")
    if "questions" not in payload:
        raise ValueError("写作需求解析结果缺少 questions 数组。")

    document_type = payload.get("document_type")
    if document_type is None:
        doc_type_value: Optional[str] = None
    else:
        if not isinstance(document_type, str):
            raise ValueError("写作需求结果的 document_type 必须是字符串或 null。")
        trimmed = document_type.strip()
        if not trimmed:
            raise ValueError(
                "写作需求结果的 document_type 无法确定时必须填 null，不得写空字符串。"
            )
        if trimmed not in DOCUMENT_TYPES:
            raise ValueError(
                "写作需求结果的 document_type 不在十二类白名单内："
                + trimmed
            )
        doc_type_value = trimmed

    draft: dict[str, Any] = {"document_type": doc_type_value}
    draft["title"] = _clean_text_field(
        payload.get("title"), "title", MAX_TITLE_CHARS
    )
    for field in CORE_TEXT_FIELDS:
        if field == "title":
            continue
        draft[field] = _clean_text_field(
            payload.get(field), field, MAX_TEXT_FIELD_CHARS
        )
    draft["writing_style"] = _clean_text_field(
        payload.get("writing_style"), "writing_style", MAX_TEXT_FIELD_CHARS
    )

    if "requirements" not in payload:
        raise ValueError("写作需求解析结果缺少 requirements 数组。")
    draft["requirements"] = _clean_string_list(
        payload.get("requirements"),
        "requirements",
        MAX_REQUIREMENT_CHARS,
        MAX_REQUIREMENTS,
    )
    questions = _clean_string_list(
        payload.get("questions"),
        "questions",
        MAX_QUESTION_CHARS,
        MAX_QUESTIONS,
    )

    if doc_type_value is None:
        if not questions:
            raise ValueError(
                "文稿类型无法确定时，questions 必须至少包含一条类型询问。"
            )
        if not any(_mentions_document_type(question) for question in questions):
            raise ValueError(
                "文稿类型无法确定时，questions 必须包含一条询问文稿类型的问题。"
            )
    if any(draft[field] is None for field in CORE_TEXT_FIELDS):
        if not questions:
            raise ValueError(
                "关键写作条件缺失时，questions 不得为空，必须逐条列出待补充问题。"
            )

    confidence = payload.get("confidence")
    if confidence is not None:
        if isinstance(confidence, bool) or not isinstance(
            confidence, (int, float)
        ):
            raise ValueError("写作需求结果的置信度必须是数字或 null。")
        if not 0 <= float(confidence) <= 1:
            raise ValueError("写作需求结果的置信度必须落在 0 到 1 之间。")
        confidence = float(confidence)
    draft["questions"] = questions
    draft["confidence"] = confidence
    return draft


def assemble_writing_requirement_prompt(text: str) -> str:
    """把用户的自然语言写作需求组装成解析提示词；不含任何模型调用。"""

    if not isinstance(text, str) or not text.strip():
        raise ValueError("写作需求不能为空。")
    stripped = text.strip()
    if len(stripped) > MAX_REQUEST_TEXT_CHARS:
        raise ValueError(
            f"写作需求过长（最多 {MAX_REQUEST_TEXT_CHARS} 字）。"
        )
    type_lines = ["- " + value for value in DOCUMENT_TYPES]
    lines = [
        "你在把用户的自然语言写作需求整理成一份写作任务草稿，并列出需要用户补充确认的问题。",
        "只输出草稿，不创建任务、不写入任何文件、不确认状态。",
        JSON_OUTPUT_SPEC,
        "可用文稿类型（document_type 只能从下列十二类中选，无法确定时填 null）：",
        *type_lines,
        "用户写作需求（以 ---BEGIN--- 与 ---END--- 为界）：",
        "---BEGIN---",
        stripped,
        "---END---",
    ]
    return "\n".join(lines)


def main() -> int:
    payload = json.load(sys.stdin)
    if not isinstance(payload, Mapping):
        raise SystemExit("写作需求提示词入参必须是 JSON 对象。")
    prompt = assemble_writing_requirement_prompt(str(payload.get("text") or ""))
    json.dump(
        {"schema": EXTRACT_SCHEMA, "prompt": prompt},
        sys.stdout,
        ensure_ascii=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
