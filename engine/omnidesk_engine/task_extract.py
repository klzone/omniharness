"""工作记录 → 任务候选提示词组装（OD-WR-304）。

纯函数 + 可选 python -m CLI；不读磁盘、不启动 DSH、不调用模型。
红线扫描、mask_privacy、JSON 校验与落盘全部由宿主负责；本模块只组装
「把记录交给模型提取候选」的提示词。
"""

from __future__ import annotations

import json
import sys
from typing import Any, Mapping, Optional, Sequence

EXTRACT_SCHEMA = "omnidesk.task-candidates.v1"

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
      "suggested_state": null
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
- 没有可执行事项时输出 {"candidates": []}。
"""


def assemble_task_extract_prompt(
    title: str,
    body: str,
    record_date: str,
    category: Optional[str] = None,
    tags: Sequence[str] = (),
    tasks: Sequence[Mapping[str, Any]] = (),
) -> str:
    """把一条记录组装成任务提取提示词；不含任何模型调用。"""

    if tags:
        tag_text = "、".join(tags)
    else:
        tag_text = "无"
    category_text = category or "无"
    lines = [
        "你在把一条工作记录整理成任务候选。只输出候选，不写入任何文件，不确认状态。",
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
    )
    json.dump({"schema": EXTRACT_SCHEMA, "prompt": prompt}, sys.stdout, ensure_ascii=False)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
