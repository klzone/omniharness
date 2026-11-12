// OD-UI-713：把回复里提到的成果文件变成可点击的链接。
//
// 只做展示与识别，不读文件、不访问网络：
//   * 只认成果目录（artifact root）内的绝对路径，避免把系统路径或示例路径变成链接；
//   * 先去掉代码块与行内代码，代码示例里的路径保持原文；
//   * 扩展名白名单覆盖常见办公与文本成果；
//   * 同一路径只出现一次，最多 8 个，避免长回复铺满链接。

export interface ArtifactLink {
  /** 原文里的绝对路径（已去掉尾部标点）。 */
  path: string;
  /** 展示用文件名。 */
  name: string;
}

const DOC_EXTS = new Set([
  "md",
  "txt",
  "csv",
  "tsv",
  "json",
  "yaml",
  "yml",
  "html",
  "htm",
  "pdf",
  "doc",
  "docx",
  "xls",
  "xlsx",
  "ppt",
  "pptx",
  "png",
  "jpg",
  "jpeg",
  "gif",
  "svg",
  "zip",
  "mp4",
  "mov",
]);

const TRAILING = new Set(["，", "。", "；", "、", "）", ")", "]", "}", ">", "'", "\"", "”", "’"]);

function stripCodeBlocks(text: string): string {
  return text.replace(/```[\s\S]*?```/g, " ").replace(/`[^`\n]*`/g, " ");
}

function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
}

function baseName(relative: string): string {
  const parts = relative.split("/");
  return parts[parts.length - 1] || relative;
}

/** 从一段回复里挑出成果目录内的文件路径；不认识的内容原样忽略。 */
export function artifactLinksInText(text: string, root: string): ArtifactLink[] {
  const base = (root ?? "").trim().replace(/\/+$/, "");
  if (!text || !base) return [];
  const body = stripCodeBlocks(text);
  const links: ArtifactLink[] = [];
  const seen = new Set<string>();
  let from = 0;
  while (links.length < 8) {
    const start = body.indexOf(base, from);
    if (start < 0) break;
    let end = start;
    while (end < body.length) {
      const ch = body[end];
      if (/[\s<>|\"'`]/.test(ch) || TRAILING.has(ch)) break;
      end += 1;
    }
    const raw = body.slice(start, end).replace(/[.,;:!?]+$/, "");
    const relative = raw.slice(base.length).replace(/^\/+/, "");
    if (relative && !relative.includes("..") && DOC_EXTS.has(extensionOf(relative))) {
      const path = base + "/" + relative;
      if (!seen.has(path)) {
        seen.add(path);
        links.push({ path, name: baseName(relative) });
      }
    }
    from = start + base.length;
  }
  return links;
}
