// 对话归类（OD-ARCH-061 S1 起步，OD-ARCH-062 S1 接管持久化）。
//
// 边界：这里只描述**展示**。线程标签仍是 thread_store 里的既有键
// （plain / weekly / writing / project，以及新增的 custom:...）；
// 显示名、归档映射由宿主 settings.v1 的 thread_categories 持久化。
// 改显示名不迁移任何线程标签、成果目录或文件。

import { useEffect, useState } from "react";
import { shell } from "./api";
import { SCENES } from "./scenes";
import type { ThreadTag } from "./sceneThreads";
import type { ThreadCategoryView } from "./types";

export interface ThreadCategory {
  key: ThreadTag;
  /** 短名：胶囊按钮上显示。 */
  label: string;
  /** 补全说明：悬停提示与设置页用。 */
  hint: string;
}

const BUILTIN_HINTS: Record<string, string> = {
  plain: "不归档到任何场景，只留在本机对话记录里",
  weekly: "按工作记录场景的规则整理，成果归到该场景档案",
  writing: "按文案写作场景的规则起草，成果归到该场景档案",
  project: "按项目跟进场景的规则整理，成果归到该场景档案",
};

/** 内置四类（宿主配置读回之前的显示用；键与顺序与宿主默认值一致）。 */
export const THREAD_CATEGORIES: ThreadCategory[] = [
  { key: "plain", label: "未分类", hint: BUILTIN_HINTS.plain },
  { key: "weekly", label: SCENES.weekly.label, hint: BUILTIN_HINTS.weekly },
  { key: "writing", label: SCENES.writing.label, hint: BUILTIN_HINTS.writing },
  { key: "project", label: SCENES.project.label, hint: BUILTIN_HINTS.project },
];

export function isCustomKey(key: string): boolean {
  return key.startsWith("custom:");
}

function hintOf(view: ThreadCategoryView): string {
  const base = BUILTIN_HINTS[view.key];
  if (base) return base;
  const parts: string[] = ["自定义分类：只进归类下拉与归档映射，不会出现在 Dock"];
  if (view.vault) parts.push("vault " + view.vault);
  else if (view.folder) parts.push("归档到成果根下的 " + view.folder);
  if (view.resolve_error) parts.push("目录不可用：" + view.resolve_error);
  return parts.join(" · ");
}

function toCategory(view: ThreadCategoryView): ThreadCategory {
  return { key: view.key as ThreadTag, label: view.label, hint: hintOf(view) };
}

// —— 宿主配置的运行时缓存 ——
//
// 初次渲染用内置四类，读回后替换为宿主列表。读取失败时保留旧值并记下错误，
// 由调用方决定怎么显示；这里不静默假装成功。
let current: ThreadCategory[] = THREAD_CATEGORIES;
let currentViews: ThreadCategoryView[] | null = null;
let currentError: string | null = null;
let inFlight: Promise<void> | null = null;
const listeners = new Set<() => void>();

function publish(next: ThreadCategory[], views: ThreadCategoryView[] | null, error: string | null) {
  current = next;
  currentViews = views;
  currentError = error;
  for (const fn of listeners) fn();
}

/** 从宿主重读分类表。并发调用合并为一次请求。 */
export function refreshThreadCategories(): Promise<void> {
  if (inFlight) return inFlight;
  inFlight = shell
    .categorySettingsLoad()
    .then((r) => {
      publish(r.categories.map(toCategory), r.categories, r.ok ? null : "读取分类设置失败");
    })
    .catch((e) => {
      publish(current, currentViews, e instanceof Error ? e.message : String(e));
    })
    .finally(() => {
      inFlight = null;
    });
  return inFlight;
}

/** 当前分类列表（不含 React 状态，供事件回调读取）。 */
export function getThreadCategories(): ThreadCategory[] {
  return current;
}

/** 宿主持久化视图（含映射与错误）；尚未读回时为 null。 */
export function getThreadCategoryViews(): ThreadCategoryView[] | null {
  return currentViews;
}

export function threadCategoriesError(): string | null {
  return currentError;
}

/** 订阅宿主分类表。挂载时自动重读一次；设置页保存后也会主动刷新。 */
export function useThreadCategories(autoLoad = true): ThreadCategory[] {
  const [list, setList] = useState<ThreadCategory[]>(current);
  useEffect(() => {
    const sync = () => setList(current);
    listeners.add(sync);
    if (autoLoad) void refreshThreadCategories();
    sync();
    return () => {
      listeners.delete(sync);
    };
  }, [autoLoad]);
  return list;
}

/** 场景键 → 归类短名（未知键按未分类显示，不抛错）。 */
export function categoryLabel(tag: ThreadTag | string): string {
  return current.find((c) => c.key === tag)?.label ?? current[0]?.label ?? "未分类";
}
