import { Fragment, type ReactNode, useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import type { SceneThread } from "../sceneThreads";
import type {
  OfflineArtifactEntry,
  WritingExportFormat,
  WritingOutlineConfirmed,
  WritingSectionState,
  WritingTask,
  WritingTaskStatus,
  WritingVersionSummary,
} from "../types";
import { ArtifactExplorer } from "./ArtifactExplorer";
import { WritingArchivePanel } from "./WritingArchivePanel";
import { WritingCollectPanel } from "./WritingCollectPanel";
import { WritingFactCheckPanel } from "./WritingFactCheckPanel";
import { WritingInspirationShelf } from "./WritingStudio";
import { WritingMaterialBasket } from "./WritingMaterialBasket";
import { WritingOutlineEditor } from "./WritingOutlineEditor";
import { WritingSectionEditor } from "./WritingSectionEditor";
import { WritingVersionPanel } from "./WritingVersionPanel";
import { WritingWorkbench } from "./WritingWorkbench";

const ACTIVE_KEY = "omnidesk.writing.active.v1";

type LibraryTab = "tasks" | "artifacts" | "threads";
type DrawerState =
  | { kind: "library"; tab: LibraryTab }
  | { kind: "outline" }
  | { kind: "section"; sectionNo: number }
  | { kind: "collect" }
  | { kind: "materials" }
  | { kind: "factcheck" }
  | { kind: "versions" }
  | { kind: "brief" }
  | { kind: "context" }
  | { kind: "archive" };

type Notice = { kind: "ok" | "err"; text: string };

export type WritingDeskProps = {
  artifacts: OfflineArtifactEntry[];
  artifactsLoaded: boolean;
  artifactsError: string | null;
  threads: SceneThread[];
  nowSeconds: number;
  includeSubdirs: boolean;
  onToggleSubdirs: (next: boolean) => void;
  stalePaths?: Set<string>;
  emptyHint: string;
  onOpenArtifact: (relativePath: string) => void;
  onOpenThread: (id: string) => void;
  onNewThread: () => void;
  onNavigateHome: () => void;
};

function errText(value: unknown): string {
  return value instanceof Error ? value.message : String(value);
}

function shortTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getMonth() + 1}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function countWords(content: string | null | undefined): number {
  return content ? content.replace(/\s/g, "").length : 0;
}

function paragraphsOf(content: string): string[] {
  return content
    .split("\n\n")
    .map((paragraph) => paragraph.trim())
    .filter(Boolean);
}

function isMissingOutline(message: string): boolean {
  return message.includes("尚未确认大纲") || message.includes("outline.md");
}

function expectedLabel(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "—";
  return trimmed.replace(/字$/, "").trim() || "—";
}

/** 写作流程的六步：每一步都对应任务状态，界面用它告诉用户现在在哪、下一步做什么。 */
const FLOW_STEPS: Array<{ label: string; hint: string; next: string }> = [
  { label: "需求", hint: "确认写什么、给谁看", next: "素材收集" },
  { label: "素材", hint: "采纳材料与依据", next: "大纲" },
  { label: "大纲", hint: "确认章节结构", next: "正文" },
  { label: "正文", hint: "逐章写正文", next: "核验" },
  { label: "核验", hint: "核对事实与引用", next: "定稿" },
  { label: "定稿", hint: "导出与归档", next: "完成" },
];

/** 每个步骤对应的任务状态；第 6 步用「待审核」作为进入定稿操作的入口。 */
const FLOW_STATUSES: WritingTaskStatus[] = [
  "需求待整理",
  "素材收集中",
  "大纲待确认",
  "写作中",
  "待核验",
  "待审核",
];

function stepOf(status: WritingTaskStatus | undefined): number {
  switch (status) {
    case "需求待整理":
      return 1;
    case "素材收集中":
      return 2;
    case "大纲待确认":
      return 3;
    case "写作中":
      return 4;
    case "待核验":
    case "待审核":
      return 5;
    case "已定稿":
    case "已归档":
      return 6;
    default:
      return 1;
  }
}

const CN_NUMBERS = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];

function cnNumber(value: number): string {
  if (value <= 10) return CN_NUMBERS[value] ?? String(value);
  if (value < 20) return "十" + (CN_NUMBERS[value - 10] ?? "");
  return String(value);
}

function readActiveId(): string | null {
  try {
    return window.localStorage.getItem(ACTIVE_KEY);
  } catch {
    return null;
  }
}

function writeActiveId(value: string | null) {
  try {
    if (value) window.localStorage.setItem(ACTIVE_KEY, value);
    else window.localStorage.removeItem(ACTIVE_KEY);
  } catch {
    // 本机缓存不可写只影响下次默认打开，不影响当前任务。
  }
}

function Drawer({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <div className="wd-drawer-backdrop" onClick={onClose} data-writing-drawer="">
      <div className="wd-drawer" onClick={(event) => event.stopPropagation()}>
        <div className="wd-drawer-head">
          <div className="wd-drawer-title">{title}</div>
          <button type="button" className="wd-drawer-close wr-icon-button" onClick={onClose} aria-label="关闭">
            ×
          </button>
        </div>
        <div className="wd-drawer-body">{children}</div>
      </div>
    </div>
  );
}

function LibraryDrawer({
  tab,
  onTab,
  onClose,
  task,
  onOpenTask,
  artifacts,
  artifactsLoaded,
  artifactsError,
  emptyHint,
  includeSubdirs,
  onToggleSubdirs,
  stalePaths,
  onOpenArtifact,
  threads,
  nowSeconds,
  onOpenThread,
  onNewThread,
  onNavigateHome,
  initialRequirement,
  onUseForWriting,
  onOpenOutline,
  onOpenSection,
}: {
  tab: LibraryTab;
  onTab: (tab: LibraryTab) => void;
  onClose: () => void;
  task: WritingTask | null;
  onOpenTask: (writingId: string) => void;
  artifacts: OfflineArtifactEntry[];
  artifactsLoaded: boolean;
  artifactsError: string | null;
  emptyHint: string;
  includeSubdirs: boolean;
  onToggleSubdirs: (next: boolean) => void;
  stalePaths?: Set<string>;
  onOpenArtifact: (relativePath: string) => void;
  threads: SceneThread[];
  nowSeconds: number;
  onOpenThread: (id: string) => void;
  onNewThread: () => void;
  onNavigateHome: () => void;
  initialRequirement?: string;
  onUseForWriting?: (thread: SceneThread) => void;
  onOpenOutline: () => void;
  onOpenSection: () => void;
}) {
  const tabs: Array<{ key: LibraryTab; label: string }> = [
    { key: "tasks", label: "稿件" },
    { key: "artifacts", label: "成果" },
    { key: "threads", label: "对话" },
  ];
  // 从写作对话带入时只显示新建需求这一段，避免在"所有稿件"列表里找入口。
  const composing = Boolean(initialRequirement) && tab === "tasks";
  return (
    <div className="wd-drawer-backdrop" onClick={onClose} data-writing-library="">
      <div className="wd-drawer wd-drawer-left" onClick={(event) => event.stopPropagation()}>
        <div className="wd-drawer-head">
          <div className="wd-drawer-title">{composing ? "带入写作" : "所有稿件"}</div>
          <button type="button" className="wd-drawer-close wr-icon-button" onClick={onClose} aria-label="关闭">
            ×
          </button>
        </div>
        {composing ? null : (
          <div className="wd-library-tabs" role="tablist" aria-label="稿件历史">
            {tabs.map((item) => (
              <button
                key={item.key}
                type="button"
                role="tab"
                aria-selected={tab === item.key}
                className={"wd-library-tab " + (tab === item.key ? "active" : "")}
                onClick={() => onTab(item.key)}
                data-writing-library-tab={item.key}
              >
                {item.label}
              </button>
            ))}
          </div>
        )}
        <div className="wd-drawer-body">
          {composing ? (
            <WritingWorkbench
              onOpenTask={onOpenTask}
              initialRequirement={initialRequirement}
              composeOnly
            />
          ) : tab === "tasks" ? (
            <>
              {task ? (
                <div className="mb-3 flex items-center gap-2 rounded-xl bg-[color:var(--od-neu-sunken)] px-3 py-2 shadow-[var(--od-neu-inset-1)]">
                  <span className="shrink-0 text-[9.5px] text-[color:var(--od-neu-ink-3)]">继续上次</span>
                  <span className="min-w-0 flex-1 truncate text-[11px] font-medium text-[color:var(--od-neu-ink)]">
                    {task.title}
                  </span>
                  <button type="button" className="wr-text-button shrink-0" onClick={() => onOpenTask(task.writing_id)}>
                    打开
                  </button>
                </div>
              ) : null}
              <WritingWorkbench
                onOpenTask={onOpenTask}
                initialRequirement={initialRequirement}
              />
            </>
          ) : tab === "artifacts" ? (
            <>
              {!artifactsLoaded && !artifactsError ? (
                <p className="mb-2 text-[10.5px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
                  正在读取成果目录…
                </p>
              ) : null}
              {artifactsError ? (
                <p className="mb-2 text-[10.5px] leading-relaxed text-error">
                  读取成果目录失败：{artifactsError}。这里不把它当成「没有成果」。
                </p>
              ) : null}
              <ArtifactExplorer
                files={artifacts}
                emptyHint={emptyHint}
                includeSubdirs={includeSubdirs}
                onToggleSubdirs={onToggleSubdirs}
                showTypeLabels
                stalePaths={stalePaths}
                onOpen={onOpenArtifact}
              />
            </>
          ) : (
            <>
              <section className="wd-rail mb-3">
                <div className="wd-rail-head">
                  <div className="wd-rail-title">本次写作</div>
                </div>
                <p className="mt-2 text-[10px] leading-relaxed text-[color:var(--od-neu-ink-3)]">
                  {task ? "当前文档：" + task.title : "还没有打开稿件。"}
                </p>
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <button type="button" className="wr-ghost-button" onClick={onOpenOutline} disabled={!task}>
                    大纲
                  </button>
                  <button type="button" className="wr-ghost-button" onClick={onOpenSection} disabled={!task}>
                    本章
                  </button>
                  <button type="button" className="wr-text-button ml-auto" onClick={onNavigateHome}>
                    回首页继续讨论
                  </button>
                </div>
              </section>
              <WritingInspirationShelf
                threads={threads}
                nowSeconds={nowSeconds}
                onOpenThread={onOpenThread}
                onNewThread={onNewThread}
                onUseForWriting={onUseForWriting}
              />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

/**
 * 写作台（OD-WD-001）：文档居中，左边大纲，右边当前章节上下文。
 * 大纲/本章/核验/版本/归档继续复用既有编辑器，放进独立抽屉，
 * 主画布只保留已确认正文和下一步动作。
 */
export function WritingDesk({
  artifacts,
  artifactsLoaded,
  artifactsError,
  threads,
  nowSeconds,
  includeSubdirs,
  onToggleSubdirs,
  stalePaths,
  emptyHint,
  onOpenArtifact,
  onOpenThread,
  onNewThread,
  onNavigateHome,
}: WritingDeskProps) {
  const [tasks, setTasks] = useState<WritingTask[]>([]);
  const [listLoading, setListLoading] = useState(true);
  const [listError, setListError] = useState<string | null>(null);
  const [writingId, setWritingId] = useState<string | null>(() => readActiveId());
  const [task, setTask] = useState<WritingTask | null>(null);
  const [outline, setOutline] = useState<WritingOutlineConfirmed | null>(null);
  const [outlineLoading, setOutlineLoading] = useState(false);
  const [outlineError, setOutlineError] = useState<string | null>(null);
  const [sectionStates, setSectionStates] = useState<Record<number, WritingSectionState>>({});
  const [sectionsLoading, setSectionsLoading] = useState(false);
  const [activeSectionNo, setActiveSectionNo] = useState<number | null>(null);
  const [versions, setVersions] = useState<WritingVersionSummary[]>([]);
  const [drawer, setDrawer] = useState<DrawerState | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [prefillRequirement, setPrefillRequirement] = useState("");
  const [contextTab, setContextTab] = useState<"materials" | "factcheck" | "versions">("materials");
  const [reloadTick, setReloadTick] = useState(0);

  useEffect(() => {
    let alive = true;
    setListLoading(true);
    shell
      .writingList({})
      .then((listing) => {
        if (!alive) return;
        setTasks(listing.items);
        setListError(null);
      })
      .catch((error) => {
        if (!alive) return;
        setListError(errText(error));
      })
      .finally(() => {
        if (alive) setListLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [reloadTick]);

  useEffect(() => {
    if (tasks.length === 0) {
      if (!listLoading) {
        setTask(null);
        setWritingId(null);
        writeActiveId(null);
      }
      return;
    }
    const remembered = writingId ? tasks.find((item) => item.writing_id === writingId) : null;
    if (remembered) {
      setTask(remembered);
      return;
    }
    const sorted = [...tasks].sort((left, right) => right.updated_at.localeCompare(left.updated_at));
    const candidate =
      sorted.find((item) => item.status !== "已归档" && item.status !== "已取消") ?? sorted[0] ?? null;
    if (!candidate) {
      setTask(null);
      setWritingId(null);
      writeActiveId(null);
      return;
    }
    setWritingId(candidate.writing_id);
    setTask(candidate);
    writeActiveId(candidate.writing_id);
  }, [tasks, writingId, listLoading]);

  const activeWritingId = task?.writing_id ?? null;

  useEffect(() => {
    if (!activeWritingId) {
      setOutline(null);
      setOutlineError(null);
      setSectionStates({});
      setActiveSectionNo(null);
      setOutlineLoading(false);
      setSectionsLoading(false);
      return;
    }
    let alive = true;
    const writingIdForLoad = activeWritingId;
    setOutlineLoading(true);
    setOutlineError(null);
    setOutline(null);
    setSectionStates({});
    setSectionsLoading(false);
    setActiveSectionNo(null);
    void (async () => {
      try {
        const confirmed = await shell.writingOutlineGet(writingIdForLoad);
        if (!alive) return;
        setOutline(confirmed);
        setOutlineLoading(false);
        setSectionsLoading(true);
        const results = await Promise.allSettled(
          confirmed.outline.sections.map((section) =>
            shell.writingSectionGet(writingIdForLoad, section.no),
          ),
        );
        if (!alive) return;
        const next: Record<number, WritingSectionState> = {};
        for (const result of results) {
          if (result.status === "fulfilled") next[result.value.section_no] = result.value;
        }
        setSectionStates(next);
        setSectionsLoading(false);
        setActiveSectionNo((previous) => {
          if (previous !== null && confirmed.outline.sections.some((section) => section.no === previous)) {
            return previous;
          }
          const firstOpen = confirmed.outline.sections.find((section) => !next[section.no]?.confirmed);
          return firstOpen?.no ?? confirmed.outline.sections[0]?.no ?? null;
        });
      } catch (error) {
        if (!alive) return;
        const text = errText(error);
        setOutline(null);
        setOutlineError(isMissingOutline(text) ? null : text);
        setOutlineLoading(false);
        setSectionsLoading(false);
        setSectionStates({});
        setActiveSectionNo(null);
      }
    })();
    return () => {
      alive = false;
    };
  }, [activeWritingId, reloadTick]);

  useEffect(() => {
    if (!activeWritingId) {
      setVersions([]);
      return;
    }
    let alive = true;
    shell
      .writingVersionList(activeWritingId)
      .then((result) => {
        if (alive) setVersions(result.versions);
      })
      .catch(() => {
        if (alive) setVersions([]);
      });
    return () => {
      alive = false;
    };
  }, [activeWritingId, reloadTick]);

  useEffect(() => {
    if (activeSectionNo === null) return;
    const target = document.getElementById("wd-section-" + String(activeSectionNo));
    target?.scrollIntoView({ block: "nearest" });
  }, [activeSectionNo]);

  const activeSection = useMemo(
    () => outline?.outline.sections.find((section) => section.no === activeSectionNo) ?? null,
    [outline, activeSectionNo],
  );
  const activeState = activeSectionNo !== null ? sectionStates[activeSectionNo] ?? null : null;
  const activeWords = countWords(activeState?.content);
  const wordCount = Object.values(sectionStates).reduce(
    (sum, state) => sum + countWords(state.content),
    0,
  );
  const confirmedCount = outline
    ? outline.outline.sections.filter((section) => sectionStates[section.no]?.confirmed).length
    : 0;
  const progress = outline && outline.outline.sections.length > 0
    ? Math.round((confirmedCount / outline.outline.sections.length) * 100)
    : 0;
  const currentVersion = versions.find((entry) => entry.current) ?? versions[0] ?? null;
  const readOnly =
    task?.status === "已定稿" || task?.status === "已归档" || task?.status === "已取消";
  const stepNo = stepOf(task?.status);
  const stepInfo = FLOW_STEPS[stepNo - 1];

  /** 点步骤回到对应页面；退回时按状态表迁移，向前仍走「下一步」主按钮。 */
  async function goToStep(target: number) {
    if (!task || target > stepNo) return;
    if (target === 1) {
      setDrawer({ kind: "brief" });
      return;
    }
    if (target === 6) {
      if (task.status === "已定稿" || task.status === "已归档") {
        setDrawer({ kind: "archive" });
      } else {
        setExportOpen(true);
      }
      return;
    }
    const to = FLOW_STATUSES[target - 1];
    if (task.status !== to) {
      try {
        await shell.writingStatusTransition({
          writing_id: task.writing_id,
          to,
          reason: "用户从写作流程返回：" + FLOW_STEPS[target - 1].label,
        });
        bump();
      } catch (error) {
        setNotice({ kind: "err", text: errText(error) });
        return;
      }
    }
    if (target === 2) setDrawer({ kind: "collect" });
    else if (target === 3) setDrawer({ kind: "outline" });
    else if (target === 4) setDrawer({ kind: "section", sectionNo: activeSectionNo ?? 1 });
    else if (target === 5) setDrawer({ kind: "factcheck" });
  }

  function openLibrary(tab: LibraryTab) {
    setPrefillRequirement("");
    setDrawer({ kind: "library", tab });
  }

  async function selectTask(writingIdToOpen: string) {
    const local = tasks.find((item) => item.writing_id === writingIdToOpen) ?? null;
    setWritingId(writingIdToOpen);
    writeActiveId(writingIdToOpen);
    setPrefillRequirement("");
    setDrawer(null);
    // 创建、确认、生成都可能刚改过状态，打开前先刷新一次列表，避免拿到旧状态。
    try {
      const listing = await shell.writingList({});
      setTasks(listing.items);
      const next = listing.items.find((item) => item.writing_id === writingIdToOpen) ?? null;
      setTask(next);
      setNotice(next ? null : { kind: "err", text: "稿件列表里没有找到这份任务。" });
    } catch (error) {
      setTask(local);
      setNotice({ kind: "err", text: "刷新任务状态失败：" + errText(error) });
    }
  }

  function bump() {
    setReloadTick((value) => value + 1);
  }

  async function finalizeTask() {
    if (!task) return;
    try {
      await shell.writingStatusTransition({
        writing_id: task.writing_id,
        to: "已定稿",
        reason: "用户定稿",
      });
      setNotice({ kind: "ok", text: "已定稿。" });
      bump();
    } catch (error) {
      const text = errText(error);
      setNotice({ kind: "err", text });
      throw error;
    }
  }

  async function exportDoc(format: WritingExportFormat) {
    if (!task) return;
    setExportOpen(false);
    try {
      const outcome = await shell.writingExport({ writing_id: task.writing_id, format });
      setNotice({ kind: "ok", text: `已导出 ${format.toUpperCase()}：${outcome.path}` });
      bump();
    } catch (error) {
      setNotice({ kind: "err", text: errText(error) });
    }
  }

  async function confirmDemand() {
    if (!task) return;
    try {
      await shell.writingStatusTransition({
        writing_id: task.writing_id,
        to: "素材收集中",
        reason: "用户确认写作需求",
      });
      setNotice({ kind: "ok", text: "需求已确认，开始收集素材。" });
      bump();
    } catch (error) {
      setNotice({ kind: "err", text: errText(error) });
    }
  }

  function useThreadForWriting(thread: SceneThread) {
    const messages = thread.messages
      .filter((message) => message.role === "user" || message.role === "assistant")
      .filter((message) => message.text.trim().length > 0);
    // 只带最近十轮，并保留最早一条用户发言；过长对话会拖慢解析、也容易让模型
    // 把整段原文抄成写作要点，所以逐条截断后再拼。
    const firstUser = messages.find((message) => message.role === "user");
    const recent = messages.slice(-10);
    if (firstUser && !recent.includes(firstUser)) recent.unshift(firstUser);
    const lines = recent.map((message) => {
      const limit = message.role === "user" ? 600 : 900;
      const body = message.text.trim();
      const clipped = body.length > limit ? body.slice(0, limit) + "…" : body;
      return (message.role === "user" ? "【我】" : "【AI】") + " " + clipped;
    });
    const text = ("对话主题：" + thread.title + "\n\n" + lines.join("\n\n")).slice(0, 6000);
    setPrefillRequirement(text);
    setDrawer({ kind: "library", tab: "tasks" });
  }

  const selectedPackCount = task
    ? task.selected_material_ids.length + task.selected_knowledge_ids.length
    : 0;

  type NextStep = { title: string; detail: string; label: string; run: () => void };
  let nextStep: NextStep = {
    title: "材料写作",
    detail: "选择一份稿件继续，或从一段写作需求开始。",
    label: "所有稿件",
    run: () => openLibrary("tasks"),
  };
  if (task) {
    switch (task.status) {
      case "需求待整理":
        nextStep = {
          title: "需求待整理",
          detail: "先核对左侧文档简报；确认需求后进入素材收集。",
          label: "确认需求，开始收集素材",
          run: () => void confirmDemand(),
        };
        break;
      case "素材收集中":
        nextStep = selectedPackCount > 0
          ? {
              title: "素材收集中",
              detail: "素材包已就绪；也可以继续联网检索或采纳候选，再生成大纲。",
              label: "生成大纲",
              run: () => setDrawer({ kind: "outline" }),
            }
          : {
              title: "素材收集中",
              detail: "在素材收集里联网检索、采纳候选，或先确认素材。",
              label: "收集素材",
              run: () => setDrawer({ kind: "collect" }),
            };
        break;
      case "大纲待确认":
        nextStep = {
          title: "大纲待确认",
          detail: "生成或修改大纲，确认后进入写作。",
          label: "编辑大纲",
          run: () => setDrawer({ kind: "outline" }),
        };
        break;
      case "写作中":
        nextStep = {
          title: "写作中",
          detail: "逐章生成或编辑正文。",
          label: "编辑本章",
          run: () => setDrawer({ kind: "section", sectionNo: activeSectionNo ?? 1 }),
        };
        break;
      case "待核验":
      case "待审核":
        nextStep = {
          title: "待核验",
          detail: "核验事实与引用，处理冲突后再提交审阅。",
          label: "开始核验",
          run: () => setDrawer({ kind: "factcheck" }),
        };
        break;
      case "已定稿":
        nextStep = {
          title: "已定稿",
          detail: "导出 Word、PDF、Markdown 或公众号 HTML。",
          label: "导出",
          run: () => setExportOpen(true),
        };
        break;
      case "已归档":
        nextStep = {
          title: "已归档",
          detail: "这份材料已归档，可查看成果或复制为新稿。",
          label: "查看成果",
          run: () => setDrawer({ kind: "archive" }),
        };
        break;
      case "已取消":
        nextStep = {
          title: "已取消",
          detail: "这份任务已取消。",
          label: "查看任务",
          run: () => openLibrary("tasks"),
        };
        break;
      default:
        break;
    }
  }
  function renderDrawer() {
    if (!drawer) return null;
    if (drawer.kind === "library") {
      return (
        <LibraryDrawer
          tab={drawer.tab}
          onTab={(tab) => setDrawer({ kind: "library", tab })}
          onClose={() => {
            setPrefillRequirement("");
            setDrawer(null);
          }}
          task={task}
          onOpenTask={selectTask}
          artifacts={artifacts}
          artifactsLoaded={artifactsLoaded}
          artifactsError={artifactsError}
          emptyHint={emptyHint}
          includeSubdirs={includeSubdirs}
          onToggleSubdirs={onToggleSubdirs}
          stalePaths={stalePaths}
          onOpenArtifact={onOpenArtifact}
          threads={threads}
          nowSeconds={nowSeconds}
          onOpenThread={onOpenThread}
          onNewThread={onNewThread}
          onNavigateHome={onNavigateHome}
          initialRequirement={prefillRequirement}
          onUseForWriting={useThreadForWriting}
          onOpenOutline={() => setDrawer({ kind: "outline" })}
          onOpenSection={() => setDrawer({ kind: "section", sectionNo: activeSectionNo ?? 1 })}
        />
      );
    }
    if (!task) return null;
    const close = () => setDrawer(null);
    switch (drawer.kind) {
      case "outline":
        return (
          <Drawer title="编辑大纲" onClose={close}>
            <WritingOutlineEditor
              writingId={task.writing_id}
              onOutlineConfirmed={() => {
                bump();
                close();
              }}
            />
          </Drawer>
        );
      case "section":
        return (
          <Drawer title={`编辑本章 · 第 ${drawer.sectionNo} 章`} onClose={close}>
            <WritingSectionEditor
              writingId={task.writing_id}
              initialSectionNo={drawer.sectionNo}
              onSectionConfirmed={() => {
                bump();
                close();
              }}
            />
          </Drawer>
        );
      case "collect":
        return (
          <Drawer title="素材收集" onClose={close}>
            <WritingCollectPanel
              writingId={task.writing_id}
              canNext={task.status === "素材收集中" || task.status === "大纲待确认"}
              nextHint={task.status === "需求待整理" ? "先确认需求，再生成大纲。" : undefined}
              searchTopic={[task.topic, task.title].find((value) => value.trim().length > 0) ?? ""}
              onSelected={() => bump()}
              onNext={() => setDrawer({ kind: "outline" })}
              onOpenMaterials={() => setDrawer({ kind: "materials" })}
            />
          </Drawer>
        );
      case "materials":
        return (
          <Drawer title="素材库" onClose={close}>
            <WritingMaterialBasket />
          </Drawer>
        );
      case "brief":
        return (
          <Drawer title="文档简报" onClose={close}>
            <div className="flex flex-col gap-3">
              <div className="wd-brief-grid">
                <div>类型<strong>{task.document_type}</strong></div>
                <div>周期<strong>{task.writing_period || "—"}</strong></div>
                <div>受众<strong>{task.audience || "—"}</strong></div>
                <div>目的<strong>{task.writing_purpose || "—"}</strong></div>
                <div>篇幅<strong>{task.expected_length || "—"}</strong></div>
                <div>风格<strong>{task.writing_style || "—"}</strong></div>
              </div>
              {task.requirements ? (
                <div>
                  <div className="wd-rail-title text-[12px]">写作要点</div>
                  <p className="mt-1 whitespace-pre-wrap text-[12px] leading-relaxed text-[color:var(--od-neu-ink-2)]">
                    {task.requirements}
                  </p>
                </div>
              ) : null}
              <div className="text-[11.5px] text-[color:var(--od-neu-ink-3)]">
                素材 {task.selected_material_ids.length} · 知识 {task.selected_knowledge_ids.length}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <button type="button" className="wr-primary-button" onClick={() => setDrawer({ kind: "collect" })}>
                  素材管理
                </button>
                <button type="button" className="wr-ghost-button" onClick={() => setDrawer({ kind: "outline" })}>
                  大纲
                </button>
              </div>
            </div>
          </Drawer>
        );
      case "context":
        return (
          <Drawer title="本章资料" onClose={close}>
            <div className="flex min-h-0 flex-col">
              <div className="wd-context-tabs" role="tablist" aria-label="章节上下文">
                {(["materials", "factcheck", "versions"] as const).map((tab) => (
                  <button
                    key={tab}
                    type="button"
                    role="tab"
                    aria-selected={contextTab === tab}
                    className={"wd-context-tab " + (contextTab === tab ? "active" : "")}
                    onClick={() => setContextTab(tab)}
                    data-writing-context-tab={tab}
                  >
                    {tab === "materials" ? "资料" : tab === "factcheck" ? "核验" : "版本"}
                  </button>
                ))}
              </div>
              <div className="wd-context-body">
                {contextTab === "materials" ? (
                  <>
                    <div className="flex items-center gap-2 text-[11.5px] text-[color:var(--od-neu-ink-3)]">
                      <span>素材 {task.selected_material_ids.length} · 知识 {task.selected_knowledge_ids.length}</span>
                      <button type="button" className="ml-auto text-[color:var(--od-neu-accent-ink)]" onClick={() => setDrawer({ kind: "collect" })}>
                        管理
                      </button>
                    </div>
                    <div className="wd-context-card">
                      <div className="label">本章映射</div>
                      <div className="value">
                        {activeSection ? "第 " + activeSection.no + " 章 · " + activeSection.heading : "尚未选择章节"}
                      </div>
                      <div className="note">
                        {activeSection
                          ? "大纲映射 " + activeSection.material_ids.length + " 条素材 · " + activeSection.knowledge_ids.length + " 条知识"
                          : "大纲确认后，这里显示本章用的素材与知识。"}
                      </div>
                    </div>
                    <div className="wd-context-card">
                      <div className="label">章节进度</div>
                      <div className="value">
                        {confirmedCount} / {outline?.outline.sections.length ?? "—"} 章已确认
                      </div>
                      <div className="note">本章已写 {activeWords} 字 · 全文进度 {progress}%</div>
                    </div>
                    <div className="wd-context-card">
                      <div className="label">文档简报</div>
                      <div className="value">{task.document_type} · {task.writing_period || "未注周期"}</div>
                      <div className="note">受众 {task.audience || "—"} · 目的 {task.writing_purpose || "—"}</div>
                    </div>
                    <button type="button" className="wr-ghost-button mt-3 w-full" onClick={() => setDrawer({ kind: "materials" })}>
                      打开素材库
                    </button>
                  </>
                ) : contextTab === "factcheck" ? (
                  <>
                    <div className="wd-context-card">
                      <div className="label">事实核验</div>
                      <div className="value">本章与全文引用</div>
                      <div className="note">核验结论在独立面板里逐条处理。冲突左右并列，不自动合并。</div>
                    </div>
                    <button type="button" className="wr-primary-button mt-3 w-full" onClick={() => setDrawer({ kind: "factcheck" })}>
                      打开核验
                    </button>
                  </>
                ) : (
                  <>
                    <div className="wd-context-card">
                      <div className="label">当前版本</div>
                      <div className="value">{currentVersion ? "第 " + currentVersion.version + " 版" : "—"}</div>
                      <div className="note">{currentVersion ? shortTime(currentVersion.at) : "还没有版本记录。"}</div>
                    </div>
                    <button type="button" className="wr-ghost-button mt-3 w-full" onClick={() => setDrawer({ kind: "versions" })}>
                      打开版本
                    </button>
                  </>
                )}
              </div>
              <div className="wd-context-foot">
                <span>{currentVersion ? "第 " + currentVersion.version + " 版" : "—"}</span>
                <span className="ml-auto">{shortTime(task.updated_at)} 保存</span>
              </div>
            </div>
          </Drawer>
        );
      case "factcheck":
        return (
          <Drawer title="事实核验" onClose={close}>
            <WritingFactCheckPanel writingId={task.writing_id} />
          </Drawer>
        );
      case "versions":
        return (
          <Drawer title="版本" onClose={close}>
            <WritingVersionPanel
              writingId={task.writing_id}
              status={task.status}
              onRestored={() => bump()}
              onFinalize={finalizeTask}
            />
          </Drawer>
        );
      case "archive":
        return (
          <Drawer title="导出与归档" onClose={close}>
            <WritingArchivePanel
              writingId={task.writing_id}
              status={task.status}
              onArchived={() => bump()}
            />
          </Drawer>
        );
      default:
        return null;
    }
  }

  if (!task) {
    return (
      <div className="wd-page" data-writing-desk="" data-writing-board="">
        <header className="wd-head" data-writing-desk-head="">
          <div className="wd-kicker">
            <span>材料写作</span>
            <span className="h-3 w-px bg-[color:var(--od-neu-line-strong)]" />
            <span>写作台</span>
          </div>
          <div className="wd-title-row">
            <div className="min-w-0">
              <h1 className="wd-doc-title">材料写作</h1>
              <div className="wd-meta">
                <span>选择一份稿件继续，或从一段写作需求开始。</span>
              </div>
            </div>
            <div className="wd-actions">
              <button type="button" className="wr-ghost-button" onClick={() => openLibrary("artifacts")}>
                成果
              </button>
              <button type="button" className="wr-ghost-button" onClick={() => openLibrary("threads")}>
                对话
              </button>
              <button type="button" className="wr-primary-button" onClick={() => openLibrary("tasks")}>
                所有稿件
              </button>
            </div>
          </div>
        </header>
        {notice ? (
          <div className={"wd-notice " + (notice.kind === "err" ? "err" : "")} role="status">
            <div className="flex items-start gap-2">
              <span className="min-w-0 flex-1 break-words">{notice.text}</span>
              <button type="button" className="shrink-0 text-[10px]" onClick={() => setNotice(null)}>
                ×
              </button>
            </div>
          </div>
        ) : null}
        <div className="flex min-h-0 flex-1 items-center justify-center py-4">
          <div className="wd-empty w-full max-w-[640px]">
            <h2 className="wd-empty-title">还没有打开的稿件</h2>
            <p className="wd-empty-copy">
              一段写作需求就能开始。写作台按需求、素材、大纲、正文、核验、定稿六步带你走完，每一步只做一件事。
            </p>
            <div className="wd-empty-flow">
              {FLOW_STEPS.map((item, index) => (
                <Fragment key={item.label}>
                  <span className="wd-empty-step">{item.label}</span>
                  {index < FLOW_STEPS.length - 1 ? <span className="wd-empty-arrow">→</span> : null}
                </Fragment>
              ))}
            </div>
            {listError ? (
              <p className="wd-empty-error">读取稿件列表失败：{listError}</p>
            ) : null}
            <div className="wd-empty-actions">
              <button type="button" className="wr-primary-button" onClick={() => openLibrary("tasks")}>
                新建材料
              </button>
              <button type="button" className="wr-ghost-button" onClick={() => openLibrary("tasks")}>
                所有稿件
              </button>
            </div>
          </div>
        </div>
        {renderDrawer()}
      </div>
    );
  }

  return (
    <div className="wd-page" data-writing-desk="" data-writing-board="">
      <header className="wd-head" data-writing-desk-head="">
        <div className="wd-kicker">
          <button type="button" onClick={() => openLibrary("tasks")}>
            ← 所有稿件
          </button>
          <span className="h-3 w-px bg-[color:var(--od-neu-line-strong)]" />
          <span>材料写作</span>
        </div>
        <div className="wd-title-row">
          <div className="min-w-0">
            <h1 className="wd-doc-title break-words">{task.title}</h1>
            <div className="wd-meta">
              <span className="wr-chip is-active">{task.document_type}</span>
              <span className="wr-chip">{task.status}</span>
              {task.writing_period ? <span>{task.writing_period}</span> : null}
              <span>第 {currentVersion?.version ?? outline?.version ?? "—"} 版</span>
              <span>
                {wordCount} / {expectedLabel(task.expected_length)} 字
              </span>
              <span>已保存 {shortTime(task.updated_at)}</span>
            </div>
          </div>
          <div className="wd-actions">
            <div className="wd-steps" role="group" aria-label="写作流程" data-writing-steps="">
              {FLOW_STEPS.map((item, index) => {
                const number = index + 1;
                const current = number === stepNo;
                return (
                  <button
                    key={item.label}
                    type="button"
                    className={
                      "wd-step-chip " +
                      (number < stepNo ? "done " : "") +
                      (current ? "current" : "")
                    }
                    onClick={() => void goToStep(number)}
                    disabled={number > stepNo}
                    aria-current={current ? "step" : undefined}
                    data-writing-step={number}
                  >
                    {item.label}
                  </button>
                );
              })}
            </div>
            {outline ? (
              <button
                type="button"
                className="wr-ghost-button"
                onClick={() => setDrawer({ kind: "context" })}
                data-writing-context=""
              >
                本章
              </button>
            ) : null}
            <button type="button" className="wr-ghost-button" onClick={() => openLibrary("threads")}>
              对话
            </button>
            <div className="wd-export-wrap">
              <button
                type="button"
                className="wr-ghost-button"
                onClick={() => setExportOpen((value) => !value)}
                aria-expanded={exportOpen}
              >
                导出
              </button>
              {exportOpen ? (
                <>
                  <button
                    type="button"
                    className="fixed inset-0 z-30 cursor-default"
                    aria-label="关闭导出菜单"
                    onClick={() => setExportOpen(false)}
                  />
                  <div className="wd-export-pop">
                    <button type="button" onClick={() => void exportDoc("docx")}>Word 公文</button>
                    <button type="button" onClick={() => void exportDoc("pdf")}>PDF 阅读版</button>
                    <button type="button" onClick={() => void exportDoc("md")}>Markdown</button>
                    <button type="button" onClick={() => void exportDoc("html")}>公众号 HTML</button>
                  </div>
                </>
              ) : null}
            </div>
            <button
              type="button"
              className="wr-icon-button"
              title="更多"
              onClick={() => setDrawer({ kind: "archive" })}
            >
              ⋯
            </button>
          </div>
        </div>
      </header>

      <section className="wd-next" data-writing-next="">
        <div className="wd-next-index">
          <span className="n">{stepNo}</span>
        </div>
        <div className="wd-next-copy">
          <span className="wd-next-kicker">下一步 · 第 {stepNo} 步</span>
          <h2>{nextStep.title}</h2>
          <p>{nextStep.detail}</p>
        </div>
        <button type="button" className="wr-primary-button wd-next-action" onClick={nextStep.run}>
          {nextStep.label}
        </button>
        <div className="wd-next-after">完成后进入「{stepInfo?.next ?? "下一步"}」</div>
      </section>

      {notice ? (
        <div className={"wd-notice " + (notice.kind === "err" ? "err" : "")} role="status" data-writing-notice={notice.kind}>
          <div className="flex items-start gap-2">
            <span className="min-w-0 flex-1 break-words">{notice.text}</span>
            <button type="button" className="shrink-0 text-[10px]" onClick={() => setNotice(null)}>
              ×
            </button>
          </div>
        </div>
      ) : null}

      {outlineLoading ? (
        <p className="wd-rail-note">正在读取大纲…</p>
      ) : null}
      {outlineError ? (
        <p className="wd-rail-note err">大纲读取失败：{outlineError}</p>
      ) : null}

      <div className="wd-body">

        <section className="wd-paper-wrap" aria-label="正文">
          <article className="wd-paper" data-writing-paper="">
            <h1>{outline?.outline.title || task.title}</h1>
            <div className="sub">{task.writing_period || task.document_type}</div>
            <div className="rule" />
            {outline ? (
              <>
                {outline.outline.sections.map((section) => {
                  const state = sectionStates[section.no];
                  const active = section.no === activeSectionNo;
                  const body = state?.content?.trim() ? paragraphsOf(state.content) : [];
                  return (
                    <section
                      key={section.no}
                      id={"wd-section-" + String(section.no)}
                      className={"wd-paper-section " + (active ? "current" : "")}
                      data-writing-paper-section={section.no}
                    >
                      <h2>
                        {cnNumber(section.no)}、{section.heading}
                        {active ? <span className="wd-current-mark">正在写</span> : null}
                      </h2>
                      {body.length > 0 ? (
                        <div className="body">
                          {body.map((paragraph, index) => (
                            <p key={index}>{paragraph}</p>
                          ))}
                        </div>
                      ) : (
                        <div className="wd-empty-section">
                          {sectionsLoading ? "正在读取本章…" : "这一章还没写。"}
                          {!sectionsLoading && !readOnly ? (
                            <button
                              type="button"
                              className="wr-ghost-button mt-2"
                              onClick={() => setDrawer({ kind: "section", sectionNo: section.no })}
                            >
                              生成本章
                            </button>
                          ) : null}
                        </div>
                      )}
                      <div className="wd-section-actions">
                        <span className="meta">
                          {state?.confirmed ? "已确认 · 第 " + state.version + " 版" : readOnly ? "只读" : "未确认"}
                        </span>
                        {!readOnly ? (
                          <button
                            type="button"
                            className="wr-ghost-button"
                            onClick={() => setDrawer({ kind: "section", sectionNo: section.no })}
                          >
                            编辑本章
                          </button>
                        ) : null}
                        {active ? (
                          <button
                            type="button"
                            className="wr-text-button"
                            onClick={() => setDrawer({ kind: "factcheck" })}
                          >
                            核验
                          </button>
                        ) : null}
                      </div>
                    </section>
                  );
                })}
              </>
            ) : (
              <div className="wd-paper-empty">
                <span className="wd-paper-empty-tag">
                  第 {stepNo} 步 · {stepInfo?.label ?? "下一步"}
                </span>
                <h2>{nextStep.title}</h2>
                <p>{nextStep.detail}</p>
                {!readOnly ? (
                  <button type="button" className="wr-primary-button" onClick={nextStep.run}>
                    {nextStep.label}
                  </button>
                ) : null}
                <div className="wd-paper-empty-next">
                  这一步完成后进入「{stepInfo?.next ?? "下一步"}」
                </div>
              </div>
            )}
          </article>
        </section>


      </div>

      {renderDrawer()}
    </div>
  );
}
