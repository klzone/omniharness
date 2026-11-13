"""工作记录 → 任务与素材候选提示词组装（OD-WR-304 / OD-KM-504）。

纯函数 + 可选 python -m CLI；不读磁盘、不启动 DSH、不调用模型。
红线扫描、mask_privacy、JSON 校验与落盘全部由宿主负责；本模块只组装
「把记录交给模型提取候选」的提示词，并提供素材候选的形状校验工具。
任务候选与素材候选搭同一次分析调用返回，不新增模型调用。
"""

from __future__ import annotations

import json
import sys
from typing import Any, Mapping, Optional, Sequence

EXTRACT_SCHEMA = "omnidesk.task-candidates.v1"

MATERIAL_TYPES = (
    "工作事实",
    "工作成果",
    "数据素材",
    "经验做法",
    "典型案例",
    "问题不足",
    "制度依据",
    "工作计划",
    "领导部署",
    "特色亮点",
)
MAX_MATERIAL_CANDIDATES = 20
MAX_MATERIAL_CONTENT_CHARS = 20_000
MAX_MATERIAL_QUOTE_CHARS = 500
MAX_MATERIAL_HINTS = 8
MAX_MATERIAL_HINT_CHARS = 24
MAX_MATERIAL_MATCH_REASON_CHARS = 200

JSON_OUTPUT_SPEC = """只输出一个 JSON 对象，不要 Markdown 代码围栏，不要解释文字。
结构必须是：
{
  "candidates": [
    {
      "title": "任务标题",
      "source_quote": "记录正文里的一段原文",
      "category": null,
      "due_date": null,
      "note": null,
      "confidence": 0.8,
      "uncertainty": null,
      "match_task_id": null,
      "match_confidence": null,
      "match_reason": null,
      "suggested_state": null,
      "importance": null,
      "importance_reason": null
    }
  ],
  "material_candidates": [
    {
      "material_type": "工作事实",
      "content": "提炼后的素材陈述",
      "source_quote": "记录正文里的一段原文",
      "event_date": null,
      "match_material_id": null,
      "match_reason": null,
      "knowledge_hints": ["主题关键词"],
      "confidence": 0.6
    }
  ]
}

约束：
- source_quote 必须逐字复制记录正文中的连续片段，不得改写、拼接或概括；宿主会做原文比对。
- due_date 只有在正文明确写出具体日期时才填 YYYY-MM-DD；任何模糊时间（下周、月底、尽快）一律填 null。
- 每条候选只对应一个可执行事项；不要把同一件事拆成多条，也不要按标题相似度合并不同事项。
- confidence 取 0 到 1；信息不足时调低，并在 uncertainty 用一句话说明缺什么。
- 如果记录与「已有未完成任务」中的某一项明显是同一件事，填 match_task_id、match_confidence（0 到 1）与 match_reason；否则三个都留 null。
- 不要因为标题相似就强行关联；把握不大时降低 match_confidence，最终由人决定。
- suggested_state 只能填该关联任务当前状态允许的下一步；不确定就留 null。状态建议不会自动应用。
- importance 只能填 高 / 中 / 低 或 null，判断依据是这件事的影响与后果；不要仅因为领导提出就判「高」，也不要仅因为没有截止日期就判「低」。
- importance 非 null 时，importance_reason 用一句话（不超过 100 字）写清依据；importance 为 null 时 importance_reason 也留 null。
- 没有可执行事项时 candidates 为空数组；没有可写入写作素材的可靠内容时 material_candidates 为空数组。两个数组都必须输出。

material_candidates 约束：
- 只从记录正文提取有明确事实依据的内容；记录没有写到的数字、案例、荣誉、成效一律不得凭空生成。
- 严格区分事实、判断、计划与推测：写明已完成且可核对的事项才可归为工作事实或工作成果；只有计划的事项必须归为工作计划，不得把计划写成成果；判断与推测不得包装成事实。
- 不得把过程夸大为成效，不得把「准备做」写成「已经做成」，不得无依据拔高。
- source_quote 必须逐字复制记录正文中的连续片段，不得改写、拼接或概括；宿主会做原文比对，并据此保存原始位置。
- content 是提炼后的素材陈述，content 与 source_quote 不能完全相同；同一条 material_candidate 只能对应一个素材类型。
- material_type 只能填以下十类之一：工作事实 / 工作成果 / 数据素材 / 经验做法 / 典型案例 / 问题不足 / 制度依据 / 工作计划 / 领导部署 / 特色亮点。
- event_date 只在正文明确写出具体日期时填 YYYY-MM-DD，否则留 null。
- knowledge_hints 是 1 到 8 个主题关键词，供后续知识检索使用；没有合适关键词时给空数组。
- 提示词没有提供「已有素材」清单时，match_material_id 与 match_reason 必须为 null；不得猜测或编造素材编号。
- 提供了「已有素材」清单时，match_material_id 只能填清单里的编号；判断与某条已有素材重复时给出 match_reason，否则两者都留 null。
"""


def _normalize_quote(value: str) -> str:
    return " ".join(value.split())


def _content_repeats_quote(content: str, quote: str) -> bool:
    normalized_content = _normalize_quote(content)
    normalized_quote = _normalize_quote(quote)
    if normalized_content == normalized_quote:
        return True
    compact_content = "".join(normalized_content.split())
    compact_quote = "".join(normalized_quote.split())
    return compact_content == compact_quote


def _is_date_shape(value: str) -> bool:
    if len(value) != 10 or value[4] != "-" or value[7] != "-":
        return False
    year, month, day = value[:4], value[5:7], value[8:]
    if not (year.isdigit() and month.isdigit() and day.isdigit()):
        return False
    return 1 <= int(month) <= 12 and 1 <= int(day) <= 31


def _clean_optional_string(
    value: Any, field: str, index: int, limit: int | None = None
) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise ValueError(f"第 {index} 条素材候选的 {field} 必须是字符串或 null。")
    trimmed = value.strip()
    if not trimmed:
        return None
    if limit is not None and len(trimmed) > limit:
        raise ValueError(
            f"第 {index} 条素材候选的 {field} 过长（最多 {limit} 字）。"
        )
    return trimmed


def validate_material_payload(
    payload: Mapping[str, Any], body: Optional[str] = None
) -> list[dict[str, Any]]:
    """校验模型输出中的 material_candidates 形状。

    纯形状检查：宿主仍会独立复校验生词、原文片段与既有素材。
    传入 body 时额外检查 source_quote 是否为正文连续原文、event_date 是否在正文出现。
    """

    if not isinstance(payload, Mapping):
        raise ValueError("素材提取结果必须是 JSON 对象。")
    items = payload.get("material_candidates", [])
    if items is None:
        items = []
    if not isinstance(items, list):
        raise ValueError("material_candidates 必须是数组。")
    if len(items) > MAX_MATERIAL_CANDIDATES:
        raise ValueError(
            f"素材候选过多：{len(items)} 条，超过 {MAX_MATERIAL_CANDIDATES} 条上限。"
        )
    out: list[dict[str, Any]] = []
    for index, item in enumerate(items, start=1):
        if not isinstance(item, Mapping):
            raise ValueError(f"第 {index} 条素材候选必须是对象。")
        material_type = item.get("material_type")
        if not isinstance(material_type, str) or material_type.strip() not in MATERIAL_TYPES:
            raise ValueError(
                f"第 {index} 条素材候选的类型不在十类白名单内。"
            )
        content = item.get("content")
        if not isinstance(content, str) or not content.strip():
            raise ValueError(f"第 {index} 条素材候选缺少正文。")
        content = content.strip()
        if len(content) > MAX_MATERIAL_CONTENT_CHARS:
            raise ValueError(f"第 {index} 条素材候选正文过长。")
        quote = item.get("source_quote")
        if not isinstance(quote, str) or not quote.strip():
            raise ValueError(f"第 {index} 条素材候选缺少来源片段。")
        quote = quote.strip()
        if len(quote) > MAX_MATERIAL_QUOTE_CHARS:
            raise ValueError(f"第 {index} 条素材候选的来源片段过长。")
        if body is not None and quote not in body:
            raise ValueError(
                f"第 {index} 条素材候选的来源片段不是记录正文的连续原文。"
            )
        if _content_repeats_quote(content, quote):
            raise ValueError(
                f"第 {index} 条素材候选的 content 与 source_quote 相同，未形成提炼。"
            )
        event_date = _clean_optional_string(
            item.get("event_date"), "event_date", index
        )
        if event_date is not None:
            if not _is_date_shape(event_date):
                raise ValueError(
                    f"第 {index} 条素材候选的事件日期形状不正确（应为 YYYY-MM-DD）。"
                )
            if body is not None and event_date not in body:
                raise ValueError(
                    f"第 {index} 条素材候选的事件日期不在正文里。"
                )
        match_material_id = _clean_optional_string(
            item.get("match_material_id"), "match_material_id", index
        )
        match_reason = _clean_optional_string(
            item.get("match_reason"),
            "match_reason",
            index,
            MAX_MATERIAL_MATCH_REASON_CHARS,
        )
        if match_material_id is not None and match_reason is None:
            raise ValueError(
                f"第 {index} 条素材候选给了关联素材编号，却没有关联理由。"
            )
        if match_material_id is None and match_reason is not None:
            raise ValueError(
                f"第 {index} 条素材候选没有关联素材编号，却给了关联理由。"
            )
        hints = item.get("knowledge_hints", [])
        if hints is None:
            hints = []
        if not isinstance(hints, list):
            raise ValueError(
                f"第 {index} 条素材候选的 knowledge_hints 必须是数组。"
            )
        cleaned_hints: list[str] = []
        for hint in hints:
            if not isinstance(hint, str):
                raise ValueError(
                    f"第 {index} 条素材候选的主题关键词必须是字符串。"
                )
            trimmed = hint.strip()
            if not trimmed:
                continue
            if len(trimmed) > MAX_MATERIAL_HINT_CHARS:
                raise ValueError(
                    f"第 {index} 条素材候选的主题关键词过长（最多 {MAX_MATERIAL_HINT_CHARS} 字）。"
                )
            if trimmed not in cleaned_hints:
                cleaned_hints.append(trimmed)
        if len(cleaned_hints) > MAX_MATERIAL_HINTS:
            raise ValueError(
                f"第 {index} 条素材候选的主题关键词过多（最多 {MAX_MATERIAL_HINTS} 个）。"
            )
        confidence = item.get("confidence")
        if confidence is not None:
            if isinstance(confidence, bool) or not isinstance(confidence, (int, float)):
                raise ValueError(
                    f"第 {index} 条素材候选的置信度必须是数字或 null。"
                )
            if not 0 <= float(confidence) <= 1:
                raise ValueError(
                    f"第 {index} 条素材候选的置信度必须落在 0 到 1 之间。"
                )
            confidence = float(confidence)
        out.append(
            {
                "material_type": material_type.strip(),
                "content": content,
                "source_quote": quote,
                "event_date": event_date,
                "match_material_id": match_material_id,
                "match_reason": match_reason,
                "knowledge_hints": cleaned_hints,
                "confidence": confidence,
            }
        )
    return out


def assemble_task_extract_prompt(
    title: str,
    body: str,
    record_date: str,
    category: Optional[str] = None,
    tags: Sequence[str] = (),
    tasks: Sequence[Mapping[str, Any]] = (),
    materials: Sequence[Mapping[str, Any]] = (),
    history: Sequence[Mapping[str, Any]] = (),
) -> str:
    """把一条记录组装成任务与素材提取提示词；不含任何模型调用。"""

    if tags:
        tag_text = "、".join(tags)
    else:
        tag_text = "无"
    category_text = category or "无"
    lines = [
        "你在把一条工作记录整理成任务候选与写作素材候选。只输出候选，不写入任何文件，不确认状态。",
        JSON_OUTPUT_SPEC,
        "记录元数据：",
        "- 标题：" + title,
        "- 日期：" + record_date,
        "- 分类：" + category_text,
        "- 标签：" + tag_text,
    ]
    if tasks:
        lines.append("已有未完成任务（可建议关联）：")
        for task in tasks:
            lines.append(
                "- id="
                + str(task.get("id") or "")
                + " | 标题："
                + str(task.get("title") or "")
                + " | 状态："
                + str(task.get("state") or "")
            )
    else:
        lines.append("已有未完成任务：无。")
    if materials:
        lines.append("已有素材（可用于判断重复；match_material_id 只能引用下列编号）：")
        for material in materials:
            lines.append(
                "- id="
                + str(material.get("id") or "")
                + " | 标题："
                + str(material.get("title") or "")
                + " | 类型："
                + str(material.get("material_type") or "")
            )
    else:
        lines.append("已有素材：无。")
    if history:
        lines.append(
            "相关历史记录（只提供元数据，供判断是否重复或延续；"
            "不要引用未给出的正文，也不要据此编造进展）："
        )
        for item in history:
            tags_text = "、".join(str(tag) for tag in (item.get("tags") or [])) or "无"
            lines.append(
                "- id="
                + str(item.get("id") or "")
                + " | 日期："
                + str(item.get("record_date") or "")
                + " | 标题："
                + str(item.get("title") or "")
                + " | 分类："
                + str(item.get("category") or "无")
                + " | 标签："
                + tags_text
                + " | 关联依据："
                + str(item.get("reason") or "")
            )
    else:
        lines.append("相关历史记录：无。")
    lines.extend(
        [
            "记录正文（以 ---BEGIN--- 与 ---END--- 为界）：",
        "---BEGIN---",
        body,
        "---END---",
        ]
    )
    return "\n".join(lines)


def main() -> int:
    payload = json.load(sys.stdin)
    record: Mapping[str, Any] = payload.get("record") or {}
    prompt = assemble_task_extract_prompt(
        title=str(record.get("title") or ""),
        body=str(record.get("body") or ""),
        record_date=str(record.get("record_date") or ""),
        category=record.get("category"),
        tags=[str(tag) for tag in (record.get("tags") or [])],
        tasks=[task for task in (payload.get("tasks") or []) if isinstance(task, Mapping)],
        materials=[
            material
            for material in (payload.get("materials") or [])
            if isinstance(material, Mapping)
        ],
        history=[
            item
            for item in (payload.get("history") or [])
            if isinstance(item, Mapping)
        ],
    )
    json.dump({"schema": EXTRACT_SCHEMA, "prompt": prompt}, sys.stdout, ensure_ascii=False)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
