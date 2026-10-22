// 场景对话线程（OD-ARCH-033 目标 3/4）。
//
// 「入口即标签，事后可改」：从哪个场景页新建的对话，创建时就带上该场景标签；
// 普通对话入口可以选场景；事后也能手动改挂。标签是左栏过滤与成果归类的**唯一依据**
// （不用模型猜归类——不可靠）。
//
// 存储位置（OD-ARCH-045 第 1 步）：
//   * **权威存储**：宿主 thread_store_* 命令写入 OmniDesk 应用数据根
//     （<数据根>/threads/<项目分目录>/<thread-id>.json），不在仓库内；
//   * 浏览器本地存储降为**缓存**：只在宿主不可用（纯 vite 开发）时使用，
//     并在数据根为空时用于一次性迁移（旧缓存原样保留，不删除）。
//
// 同步函数仍保留（调用方无需改成 async）：先改内存列表，后台再落盘。
// 启动时必须先 hydrateThreads() 一次，从数据根读回；失败则退回缓存并如实暴露状态。

import type { ChatMessage } from "./shell/HomeChat";
import { isSceneKey, type SceneKey } from "./scenes";
import { invoke, isTauri } from "@tauri-apps/api/core";

const THREADS_KEY = "omnidesk.scenes.threads.v1";

/** 迁移标记：只迁移一次，且迁移后不删除缓存。 */
const MIGRATED_KEY = "omnidesk.scenes.threads.v1.migrated";

/**
 * 归类标签（OD-ARCH-062 S1）：既有三个场景键与 plain，另允许宿主持久化的
 * 自定义分类 `custom:...`。自定义键只是归属标签：不改变场景页、不进 Dock，
 * 既有线程的标签与存档格式因此零迁移（thread_store 只要求 tag 是非空字符串）。
 */
export type ThreadTag = SceneKey | "plain" | `custom:${string}`;

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

let cache: SceneThread[] = [];
let hydrated = false;
let storageNote: string | null = null;
let writeQueue: Promise<void> = Promise.resolve();

/**
 * 首屏兜底（OD-ARCH-048 卡死修复）：读数据根的最长等待时间。
 *
 * 宿主命令现已走阻塞线程池，但首次启动时宿主可能正忙于冷启动引擎；此处再给
 * 一个上界。超时后先用本地缓存渲染，不等磁盘——线程记录稍后被读到也不会覆盖
 * 用户已经看到的内容（`hydrated` 只在成功读取后置位）。
 */
const HYDRATE_TIMEOUT_MS = 4000;

function readLocal(): SceneThread[] {
  try {
    const raw = window.localStorage.getItem(THREADS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as SceneThread[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function writeLocal(threads: SceneThread[]) {
  try {
    window.localStorage.setItem(THREADS_KEY, JSON.stringify(threads));
  } catch {
    /* 存储不可用只是不持久化，不影响本次使用 */
  }
}

function read(): SceneThread[] {
  if (!hydrated) return readLocal();
  return cache;
}

function write(threads: SceneThread[]) {
  cache = threads;
  // 缓存照写：即使宿主暂时不可用，本次会话内的记录也不丢。
  writeLocal(threads);
}

/** 宿主存储是否可用（纯浏览器开发时不调用 invoke，避免控制台噪声）。 */
function hostAvailable(): boolean {
  try {
    return isTauri();
  } catch {
    return false;
  }
}

function normalizeThread(value: unknown): SceneThread | null {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<SceneThread> & { messages?: unknown };
  if (typeof raw.id !== "string" || typeof raw.title !== "string") return null;
  if (!Array.isArray(raw.messages)) return null;
  return {
    id: raw.id,
    tag: normalizeTag(raw.tag as string | null | undefined),
    projectPath: typeof raw.projectPath === "string" ? raw.projectPath : null,
    title: raw.title,
    preset: typeof raw.preset === "string" ? raw.preset : "plain",
    messages: raw.messages as ChatMessage[],
    engineSessionId:
      typeof raw.engineSessionId === "string" ? raw.engineSessionId : null,
    createdAt: typeof raw.createdAt === "number" ? raw.createdAt : 0,
    updatedAt: typeof raw.updatedAt === "number" ? raw.updatedAt : 0,
  };
}

/** 把一次变更写入数据根。串行执行，避免并发写同一文件时互相覆盖。 */
function persist(thread: SceneThread | { remove: string }) {
  if (!hostAvailable()) return;
  writeQueue = writeQueue
    .then(async () => {
      if ("remove" in thread) {
        await invoke("thread_store_delete", { id: thread.remove });
      } else {
        await invoke("thread_store_save", { thread });
      }
      storageNote = null;
    })
    .catch((e) => {
      // 落盘失败不静默：暴露给界面（threadStorageNote 读取），但不阻断本次使用。
      storageNote =
        "线程记录未能写入数据根（本次仍保存在本地缓存）：" +
        (e instanceof Error ? e.message : String(e));
    });
}

/**
 * 启动时从数据根恢复线程（幂等；多次调用只生效一次）。
 *
 * 顺序：
 *   1. 宿主可用 → 读数据根；有记录就以数据根为准（缓存里数据根没有的旧记录仍可见）。
 *   2. 数据根为空且从未迁移过 → 把本地缓存一次性写进数据根（**不删除缓存**）。
 *   3. 宿主不可用 / 读取失败 → 退回本地缓存，并把原因记进 storageNote。
 */
export async function hydrateThreads(): Promise<void> {
  if (hydrated) return;
  const local = readLocal();
  if (!hostAvailable()) {
    cache = local;
    hydrated = true;
    storageNote = "宿主不可用，线程记录暂存在浏览器缓存。";
    return;
  }
  const pending = invoke<{
    dir: string;
    threads: unknown[];
    skipped: number;
    note: string;
  }>("thread_store_load");
  try {
    const listing = await withTimeout(pending, HYDRATE_TIMEOUT_MS);
    applyListing(listing, local);
  } catch (e) {
    cache = local;
    hydrated = true;
    if (e instanceof HydrateTimeout) {
      // 读数据根太慢：先用本地缓存让界面可用，后台继续等这次读取。
      storageNote = "读取数据根较慢，先用本地缓存显示；读到后会自动补上。";
      pending.then(
        (listing) => {
          applyListing(listing, readLocal());
        },
        () => {
          /* 后台读取最终失败：保留缓存，不再打扰用户 */
        },
      );
      return;
    }
    storageNote =
      "读取数据根的线程记录失败，暂用本地缓存：" +
      (e instanceof Error ? e.message : String(e));
  }
}

class HydrateTimeout extends Error {}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new HydrateTimeout()), ms);
    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timer);
        reject(error);
      },
    );
  });
}

interface ThreadListing {
  dir: string;
  threads: unknown[];
  skipped: number;
  /** 宿主逐条报出的损坏/无法迁移文件（OD-ARCH-049 第 2 步）；不静默跳过。 */
  issues?: Array<{ file: string; reason: string }>;
  note: string;
}

/** 把数据根读到的记录并入缓存；`hydrated` 只在这里置位（表示数据根已被读过一次）。 */
function applyListing(listing: ThreadListing, local: SceneThread[]): void {
  const fromDisk = listing.threads
    .map(normalizeThread)
    .filter((t): t is SceneThread => t !== null);
  const issues = listing.issues ?? [];
  if (fromDisk.length === 0 && local.length > 0 && !migratedFlag()) {
    // 首次迁移：旧缓存一次性写入数据根，旧缓存保留（只读兼容）。
    cache = local;
    hydrated = true;
    for (const thread of local) persist(thread);
    setMigratedFlag();
    storageNote =
      "已把 " + local.length + " 条本地缓存的对话迁移到数据根（旧缓存保留）。";
    return;
  }
  const diskIds = new Set(fromDisk.map((t) => t.id));
  const leftovers = local.filter((t) => !diskIds.has(t.id));
  cache = [...fromDisk, ...leftovers];
  hydrated = true;
  storageNote =
    issues.length > 0
      ? "有 " +
        issues.length +
        " 条线程文件未能读回（" +
        issues[0].file +
        "：" +
        issues[0].reason +
        "），其余记录已正常显示。"
      : listing.skipped > 0
        ? "读取线程记录时跳过 " + listing.skipped + " 个不合规文件。"
        : leftovers.length > 0
          ? "有 " + leftovers.length + " 条旧缓存记录尚未迁移到数据根。"
          : null;
}

function migratedFlag(): boolean {
  try {
    return window.localStorage.getItem(MIGRATED_KEY) === "1";
  } catch {
    return false;
  }
}

function setMigratedFlag() {
  try {
    window.localStorage.setItem(MIGRATED_KEY, "1");
  } catch {
    /* 标记存不下最多再迁移一次（写入同样内容，幂等） */
  }
}

/** 当前存储状态（供界面如实显示；没有问题时为 null）。 */
export function threadStorageNote(): string | null {
  return storageNote;
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
  persist(thread);
  return thread;
}

/** 记录一轮对话；标题取第一条用户消息的前若干字。 */
export function saveThreadMessages(id: string, messages: ChatMessage[]): void {
  const now = Date.now();
  const next = read().map((t) =>
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
  );
  write(next);
  const changed = next.find((t) => t.id === id);
  if (changed) persist(changed);
}

/** 事后改挂标签（换场景）。 */
export function retagThread(id: string, tag: ThreadTag): void {
  const next = read().map((t) =>
    t.id === id ? { ...t, tag, updatedAt: Date.now() } : t,
  );
  write(next);
  const changed = next.find((t) => t.id === id);
  if (changed) persist(changed);
}

/**
 * 记录线程的引擎会话号（OD-ARCH-034）。
 *
 * 这是**引擎侧的会话标识**，不是对话正文；与线程消息一样只存在浏览器本地存储，
 * 不进仓库、不进成果目录。换会话（新建线程）时自然另起一个会话号。
 */
export function setThreadEngineSession(id: string, engineSessionId: string): void {
  const next = read().map((t) =>
    t.id === id ? { ...t, engineSessionId, updatedAt: Date.now() } : t,
  );
  write(next);
  const changed = next.find((t) => t.id === id);
  if (changed) persist(changed);
}

export function removeThread(id: string): void {
  write(read().filter((t) => t.id !== id));
  persist({ remove: id });
}

export function normalizeTag(value: string | null | undefined): ThreadTag {
  if (isSceneKey(value)) return value;
  if (value && value.startsWith("custom:")) return value as ThreadTag;
  return "plain";
}

/**
 * 「去对话」的交接点（OD-ARCH-057）。
 *
 * 场景页已不含对话框，只能把「打开哪条对话」交给首页。这里用一个进程内的一次性
 * 请求槽：写入后首页取走并清空。不落盘、不发事件、不改数据结构。
 */
let pendingOpenThreadId: string | null = null;
let pendingNewThreadTag: ThreadTag | null = null;

export function requestOpenThread(id: string): void {
  pendingOpenThreadId = id;
  pendingNewThreadTag = null;
}

export function requestNewThread(tag: ThreadTag): void {
  pendingOpenThreadId = null;
  pendingNewThreadTag = tag;
}

export function takePendingThreadRequest(): { open: string | null; create: ThreadTag | null } {
  const out = { open: pendingOpenThreadId, create: pendingNewThreadTag };
  pendingOpenThreadId = null;
  pendingNewThreadTag = null;
  return out;
}
