// 对话附件（OD-ARCH-077 S1）：把用户选中的文件拼进这一轮提示词。
//
// 刻意不依赖 React / Tauri：界面与离线验证脚本执行同一份源码。
//
// 事实与边界：
//   * 附件正文**原样**进提示词，并带上路径与 sha256，便于用户核对模型看到的是哪一份；
//   * 附件正文同时进入红线扫描的扫描对象（由宿主把 `userInputText` 当作扫描文本）；
//   * 只支持宿主白名单里的文本/表格类型；二进制不猜内容、不生成假摘要；
//   * 有总长上限，超限就在发送前拒绝，而不是悄悄截断。

export interface AttachmentInput {
  path: string;
  fileName: string;
  sha256: string;
  content: string;
}

/** 单轮最多引用几个文件。 */
export const ATTACHMENT_MAX_FILES = 5;
/** 单轮附件正文合计字符上限（避免把提示词撑爆）。 */
export const ATTACHMENT_TOTAL_MAX_CHARS = 120_000;

export interface AttachmentCheck {
  ok: boolean;
  reason: string | null;
}

/** 发送前的整体检查：数量、长度、空文件。 */
export function checkAttachments(items: AttachmentInput[]): AttachmentCheck {
  if (items.length > ATTACHMENT_MAX_FILES) {
    return {
      ok: false,
      reason: `一次最多引用 ${ATTACHMENT_MAX_FILES} 个文件，现在有 ${items.length} 个。`,
    };
  }
  let total = 0;
  for (const item of items) {
    if (!item.content.trim()) {
      return { ok: false, reason: `${item.fileName} 是空文件，已停止发送。` };
    }
    total += item.content.length;
  }
  if (total > ATTACHMENT_TOTAL_MAX_CHARS) {
    return {
      ok: false,
      reason: `附件正文合计 ${total} 字，超过单轮上限 ${ATTACHMENT_TOTAL_MAX_CHARS} 字；请减少文件或先拆分。`,
    };
  }
  return { ok: true, reason: null };
}

/**
 * 生成附件的提示词片段。
 *
 * 逐份给出：文件名、绝对路径、sha256、正文。明确要求按原文核对、不得补造，
 * 让「用没用上文件」在输出里可被用户检查。
 */
export function formatAttachmentBlock(items: AttachmentInput[]): string {
  if (items.length === 0) return "";
  const lines: string[] = [];
  lines.push(`用户本轮引用了 ${items.length} 个文件。请以这些原文为依据；原文没有的内容不要补造。`);
  lines.push("数字与条款以原文为准。需要引用时，请指出文件名与原文中的位置。");
  items.forEach((item, index) => {
    lines.push("");
    lines.push(`--- 附件 ${index + 1}：${item.fileName} ---`);
    lines.push(`路径：${item.path}`);
    lines.push(`sha256：${item.sha256}`);
    lines.push("正文开始");
    lines.push(item.content);
    lines.push("正文结束");
  });
  return lines.join("\n");
}

/** 把用户原话与附件正文一起作为本轮提交的内容。 */
export function composeWithAttachments(text: string, items: AttachmentInput[]): string {
  const block = formatAttachmentBlock(items);
  if (!block) return text;
  return text + "\n\n" + block;
}

/**
 * 红线扫描的输入：用户原话 + 附件正文。
 *
 * 只包含这两部分，不含场景规则（规则里的边界说明词会误判）。
 */
export function attachmentScanText(text: string, items: AttachmentInput[]): string {
  if (items.length === 0) return text;
  return [text, ...items.map((item) => item.content)].join("\n");
}

/** 从输入框当前内容里解析 @ 查询词；没有触发时返回 null。 */
export function activeMentionQuery(value: string): string | null {
  const match = /(^|\s)@([^\s@]*)$/.exec(value);
  return match ? match[2] : null;
}

/** 把选中的 @ 查询词换成可读的文件名（路径在附件 chip 上）。 */
export function applyMentionPick(value: string, fileName: string): string {
  return value.replace(/(^|\s)@([^\s@]*)$/, (_all, lead: string) => `${lead}${fileName} `);
}

