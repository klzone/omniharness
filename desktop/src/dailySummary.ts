// 当日摘要的材料整理（OD-WR-503）。
//
// 纯函数、无依赖：界面与离线验证脚本执行同一份源码。模型只看到这里整理出的
// 正式记录全文；草稿、原始素材与别的对话内容不进入材料。

import type { WorkRecord } from "./types";

export const DAILY_SUMMARY_SECTIONS: readonly string[] = [
  "今日工作概况",
  "重点事项",
  "重要思考",
  "需要跟进的问题",
  "与任务管理的关联",
];

export interface DailySummaryMaterial {
  /** 交给模型的当天正式记录全文索引。 */
  material: string;
  /** 实际进入材料的记录条数。 */
  recordCount: number;
  /** 材料里的记录编号，供界面校验与展示。 */
  recordIds: string[];
}

/**
 * 整理当日正式记录。
 *
 * 只收 `draft=false` 的记录；正文原样保留，不截断、不改写。按创建时间升序排列，
 * 让摘要里的「今天先做了什么、后做了什么」与记录顺序一致。
 */
export function buildDailySummaryMaterial(
  date: string,
  records: WorkRecord[],
): DailySummaryMaterial {
  const formal = records
    .filter((record) => !record.draft)
    .slice()
    .sort((a, b) => {
      const byCreated = a.created_at.localeCompare(b.created_at);
      return byCreated !== 0 ? byCreated : a.id.localeCompare(b.id);
    });
  if (formal.length === 0) {
    return { material: "当日没有正式记录。", recordCount: 0, recordIds: [] };
  }
  const blocks = formal.map((record) => {
    const meta = [
      record.category ? "分类：" + record.category : null,
      record.tags.length > 0 ? "标签：" + record.tags.join("、") : null,
      record.linked_task_ids.length > 0
        ? "关联任务：" + record.linked_task_ids.join("、")
        : "关联任务：无",
    ]
      .filter(Boolean)
      .join("｜");
    return [
      "#" + record.id + "｜" + record.title,
      "记录日期：" + record.record_date,
      meta,
      "正文：",
      record.body.trim(),
    ].join("\n");
  });
  return {
    material: "当天日期：" + date + "\n\n" + blocks.join("\n\n---\n\n"),
    recordCount: formal.length,
    recordIds: formal.map((record) => record.id),
  };
}

/** 从摘要正文里取出 `#wr-…` 形式的记录编号；界面用它做可点击引用。 */
export function dailySummaryRecordIds(text: string): string[] {
  const ids: string[] = [];
  let rest = text;
  while (true) {
    const at = rest.indexOf("#wr-");
    if (at < 0) break;
    const tail = rest.slice(at + 1);
    let end = 0;
    while (end < tail.length) {
      const ch = tail[end];
      if (/[A-Za-z0-9_-]/.test(ch)) end += 1;
      else break;
    }
    const id = tail.slice(0, end);
    if (id && !ids.includes(id)) ids.push(id);
    rest = tail.slice(Math.max(end, 1));
  }
  return ids;
}
