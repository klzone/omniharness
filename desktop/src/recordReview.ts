// 工作记录落盘后的分析结果（OD-WR-105）。
//
// 记录已经由宿主保存；模型只产出「问题 / 缺项 / 任务建议」，不写文件。
// 结果放在 <omnidesk-review> 块里，界面解析后显示，并据此进入待回复状态。

export interface RecordReview {
  errors: string[];
  missing: string[];
  task: { suggest: boolean; title: string };
  question: string;
}

const OPEN = "<omnidesk-review>";
const CLOSE = "</omnidesk-review>";

function asStrings(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map((item) => String(item ?? "").trim()).filter(Boolean).slice(0, 8);
}

/** 从模型回复里切出可见文字与复核块；没有块时 review 为 null。 */
export function splitRecordReview(text: string): { visible: string; review: RecordReview | null } {
  const raw = text ?? "";
  const openAt = raw.indexOf(OPEN);
  if (openAt < 0) return { visible: raw, review: null };
  const bodyStart = openAt + OPEN.length;
  const closeAt = raw.indexOf(CLOSE, bodyStart);
  if (closeAt < 0) return { visible: raw.slice(0, openAt).trim(), review: null };
  const visible = (raw.slice(0, openAt) + raw.slice(closeAt + CLOSE.length)).trim();
  try {
    const parsed = JSON.parse(raw.slice(bodyStart, closeAt).trim()) as {
      errors?: unknown;
      missing?: unknown;
      task?: { suggest?: unknown; title?: unknown };
      question?: unknown;
    };
    const task = parsed.task ?? {};
    return {
      visible,
      review: {
        errors: asStrings(parsed.errors),
        missing: asStrings(parsed.missing),
        task: {
          suggest: Boolean(task.suggest),
          title: String(task.title ?? "").trim(),
        },
        question: String(parsed.question ?? "").trim(),
      },
    };
  } catch {
    return { visible, review: null };
  }
}
