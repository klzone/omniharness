// 对话归类（OD-ARCH-061 S1）：界面上固定四类，内部仍是既有的场景键。
//
// 边界：这里只描述**展示**。分类的持久化仍是 thread_store 里的 tag（plain / weekly /
// writing / project），不改数据格式；自定义名称与新增分类需要宿主持久化字段，
// 属 OD-ARCH-061 风险①，未随本批实现（见 STATUS.md）。

import { SCENES } from "./scenes";
import type { ThreadTag } from "./sceneThreads";

export interface ThreadCategory {
  key: ThreadTag;
  /** 短名：胶囊按钮上显示。 */
  label: string;
  /** 补全说明：悬停提示与设置页用。 */
  hint: string;
}

export const THREAD_CATEGORIES: ThreadCategory[] = [
  { key: "plain", label: "未分类", hint: "不归档到任何场景，只留在本机对话记录里" },
  {
    key: "weekly",
    label: SCENES.weekly.label,
    hint: "按工作记录场景的规则整理，成果归到该场景档案",
  },
  {
    key: "writing",
    label: SCENES.writing.label,
    hint: "按文案写作场景的规则起草，成果归到该场景档案",
  },
  {
    key: "project",
    label: SCENES.project.label,
    hint: "按项目跟进场景的规则整理，成果归到该场景档案",
  },
];

/** 场景键 → 归类短名（未知键按未分类显示，不抛错）。 */
export function categoryLabel(tag: ThreadTag | string): string {
  return THREAD_CATEGORIES.find((c) => c.key === tag)?.label ?? THREAD_CATEGORIES[0].label;
}
