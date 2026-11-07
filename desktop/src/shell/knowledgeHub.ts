import type {
  KnowledgeGovernanceRecord,
  KnowledgeVaultFile,
  KnowledgeVaultIndex,
  KnowledgeVaultScan,
} from "../types";

export interface KnowledgeConflict {
  key: string;
  kind: "duplicate" | "scan_error" | "index_issue" | "link" ;
  title: string;
  detail: string;
  left?: string;
  right?: string;
}

export function rawFiles(scan: KnowledgeVaultScan | null): KnowledgeVaultFile[] {
  return (scan?.files ?? []).filter((file) => file.relative_path.startsWith("raw/"));
}

export function wikiNotes(scan: KnowledgeVaultScan | null): KnowledgeVaultFile[] {
  return (scan?.files ?? []).filter(
    (file) =>
      file.kind === "note" &&
      !file.relative_path.startsWith("raw/") &&
      !file.relative_path.startsWith("inbox/") &&
      !file.relative_path.startsWith("governance/") &&
      !file.relative_path.startsWith("知识库/索引") &&
      !file.relative_path.startsWith("知识库/原文/"),
  );
}

export function countPendingReviews(items: Array<{ status: string }>): number {
  return items.filter((item) => item.status === "pending").length;
}

/** 冲突只取可由当前扫描/索引确认的项；已写 resolve_conflict 的不再显示。 */
export function buildKnowledgeConflicts(
  scan: KnowledgeVaultScan | null,
  index: KnowledgeVaultIndex | null,
  reviewDecisions: KnowledgeGovernanceRecord[],
): KnowledgeConflict[] {
  const resolved = new Set(
    reviewDecisions
      .filter(
        (record) =>
          record.action === "resolve_conflict" && /^resolved(?:\s|；|$)/.test(record.note.trim()),
      )
      .map((record) => record.subject),
  );
  const out: KnowledgeConflict[] = [];
  const add = (conflict: KnowledgeConflict) => {
    if (!resolved.has(conflict.key)) out.push(conflict);
  };

  const byName = new Map<string, KnowledgeVaultFile[]>();
  for (const file of wikiNotes(scan)) {
    const key = file.file_name.toLocaleLowerCase();
    byName.set(key, [...(byName.get(key) ?? []), file]);
  }
  for (const [name, files] of byName) {
    if (files.length < 2) continue;
    add({
      key: `conflict:duplicate:${name}`,
      kind: "duplicate",
      title: `同名条目：${name}`,
      detail: `${files.length} 个卡片的文件名相同，需要确认哪一份是权威来源。`,
      left: files[0].relative_path,
      right: files[1].relative_path,
    });
  }

  for (const [position, error] of (scan?.errors ?? []).entries()) {
    add({
      key: `conflict:scan:${error.relative_path}:${position}`,
      kind: "scan_error",
      title: `扫描异常：${error.relative_path || "知识库根"}`,
      detail: error.message,
      left: error.relative_path,
    });
  }

  for (const issue of index?.issues ?? []) {
    if (issue.code.startsWith("scan_skipped_")) continue;
    add({
      key: `conflict:issue:${issue.relative_path}:${issue.code}`,
      kind: "index_issue",
      title: `索引异常：${issue.code}`,
      detail: `${issue.relative_path || "知识库根"}：${issue.message}`,
      left: issue.relative_path,
    });
  }

  for (const link of index?.links ?? []) {
    if (link.status !== "unresolved" && link.status !== "ambiguous") continue;
    // OD-KB-201 S5 缺陷 B：索引.md「原始资料」小节指向 raw/ 的链接不是 wiki
    // 笔记链接，vault_index 按笔记口径解析会报未解析；这里不把它算作审核冲突。
    if (
      link.source_path.endsWith("知识库/索引.md") &&
      link.target_raw.trim().replace(/^</, "").replace(/>$/, "").startsWith("raw/")
    ) {
      continue;
    }
    add({
      key: `conflict:link:${link.source_path}:${link.line}:${link.target_raw}`,
      kind: "link",
      title: link.status === "ambiguous" ? "链接歧义" : "链接未解析",
      detail: `${link.source_path} 第 ${link.line} 行指向 ${link.target_raw || "（空）"}`,
      left: link.source_path,
      right: link.target_raw,
    });
  }

  return out.slice(0, 100);
}
