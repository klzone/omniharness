// 展示层去掉宿主内部的记录/任务引用编号（#wr-… / #t-…）。
//
// 编号仍然保留在文件里：摘要、复盘的校验依赖它们（正文必须引用真实存在
// 的记录，否则整份拒绝）。这里只影响界面渲染，不改存储、不改校验。

const REF_RE = /#(?:wr|t)-[0-9a-zA-Z-]+/g;

/** 去掉引用编号，并清理因此留下的多余空白与标点前的空格。 */
export function stripRecordRefs(text: string): string {
  if (!text) return text;
  return text
    .replace(REF_RE, "")
    .replace(/[ \t]{2,}/g, " ")
    .replace(/[ \t]+([，。；、）])/g, "$1")
    .replace(/[ \t]+$/gm, "");
}
