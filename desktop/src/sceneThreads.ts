// 场景对话线程（OD-ARCH-033 目标 3/4）。
//
// 「入口即标签，事后可改」：从哪个场景页新建的对话，创建时就带上该场景标签；
// 普通对话入口可以选场景；事后也能手动改挂。标签是左栏过滤与成果归类的**唯一依据**
// （不用模型猜归类——不可靠）。
//
// 存储位置：浏览器本地存储（webview 私有目录），**不在仓库内**、不进成果目录。
// 真正的「落到仓库外 OmniDesk 数据根、按项目分目录」需要宿主新增读写能力，
// 超出本计划「不新增通信层」的边界，尚未实现（见 REVIEW.md 的遗留项）。

import type { ChatMessage } from "./shell/HomeChat";
import { isSceneKey, type SceneKey } from "./scenes";

const THREADS_KEY = "omnidesk.scenes.threads.v1";

export type ThreadTag = SceneKey | "plain";

export interface SceneThread {
  id: string;
  /** 场景标签；"plain" = 未归类（普通对话）。 */
  tag: ThreadTag;
  /** 所属项目目录（绝对路径）；未指定时为空。 */
  projectPath: string | null;
  title: string;
  /** 对话开场预设键。 */
  preset: string;
  messages: ChatMessage[];
  /**
   * 引擎会话号（OD-ARCH-034）：同一线程续发时复用同一个 DSH 会话（真多轮）。
   * 没有这一项的旧线程在首次续聊时会自动降级为新建会话，并从那一刻起开始持有它。
   */
  engineSessionId?: string | null;
  createdAt: number;
  updatedAt: number;
}

function read(): SceneThread[] {
  try {
    const raw = window.localStorage.getItem(THREADS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as SceneThread[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function write(threads: SceneThread[]) {
  try {
    window.localStorage.setItem(THREADS_KEY, JSON.stringify(threads));
  } catch {
    /* 存储不可用只是不持久化，不影响本次使用 */
  }
}

export function listThreads(): SceneThread[] {
  return read().sort((a, b) => b.updatedAt - a.updatedAt);
}

/** 某个场景（含普通）的线程。 */
export function listThreadsByTag(tag: ThreadTag): SceneThread[] {
  return listThreads().filter((t) => t.tag === tag);
}

export function getThread(id: string): SceneThread | undefined {
  return read().find((t) => t.id === id);
}

export function createThread(input: {
  tag: ThreadTag;
  projectPath?: string | null;
  preset?: string;
}): SceneThread {
  const now = Date.now();
  const thread: SceneThread = {
    id: "t" + now.toString(36) + Math.random().toString(36).slice(2, 6),
    tag: input.tag,
    projectPath: input.projectPath ?? null,
    title: "新对话",
    preset: input.preset ?? "plain",
    messages: [],
    createdAt: now,
    updatedAt: now,
  };
  write([thread, ...read()]);
  return thread;
}

/** 记录一轮对话；标题取第一条用户消息的前若干字。 */
export function saveThreadMessages(id: string, messages: ChatMessage[]): void {
  const now = Date.now();
  write(
    read().map((t) =>
      t.id === id
        ? {
            ...t,
            messages,
            updatedAt: now,
            title:
              t.title !== "新对话"
                ? t.title
                : (messages.find((m) => m.role === "user" && m.text)?.text ?? t.title).slice(0, 18),
          }
        : t,
    ),
  );
}

/** 事后改挂标签（换场景）。 */
export function retagThread(id: string, tag: ThreadTag): void {
  write(read().map((t) => (t.id === id ? { ...t, tag, updatedAt: Date.now() } : t)));
}

/**
 * 记录线程的引擎会话号（OD-ARCH-034）。
 *
 * 这是**引擎侧的会话标识**，不是对话正文；与线程消息一样只存在浏览器本地存储，
 * 不进仓库、不进成果目录。换会话（新建线程）时自然另起一个会话号。
 */
export function setThreadEngineSession(id: string, engineSessionId: string): void {
  write(
    read().map((t) =>
      t.id === id ? { ...t, engineSessionId, updatedAt: Date.now() } : t,
    ),
  );
}

export function removeThread(id: string): void {
  write(read().filter((t) => t.id !== id));
}

export function normalizeTag(value: string | null | undefined): ThreadTag {
  return isSceneKey(value) ? value : "plain";
}
