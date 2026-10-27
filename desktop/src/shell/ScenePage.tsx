import { useCallback, useEffect, useMemo, useState } from "react";
import { shell } from "../api";
import { SCENES, sameScenario, type SceneKey } from "../scenes";
import {
  listThreadsByTag,
  removeThread,
  requestNewThread,
  requestOpenThread,
  retagThread,
  type SceneThread,
  type ThreadTag,
} from "../sceneThreads";
import type { OfflineArtifactEntry, ProjectSummary, SceneOverview } from "../types";
import { useThreadCategories } from "../threadCategories";
import { Icon } from "../components/Icon";
import { AnnouncementBar } from "./AnnouncementBar";
import { ArtifactExplorer, type ArtifactArchiveEntry } from "./ArtifactExplorer";
import { CategoryPicker } from "./CategoryPicker";
import type { Scene } from "./Dock";
import { SceneStatusStrip } from "./SceneStatusStrip";
import { WeeklyTimeline } from "./WeeklyTimeline";
import { WeeklyQuadrants } from "./WeeklyQuadrants";
import { SourcePeekSheet } from "./SourcePeekSheet";
import { ProjectBoard } from "./ProjectBoard";
import { WeeklyReportStatus } from "./WeeklyReportPanel";
import {
  WritingContinueCard,
  WritingInspirationShelf,
  WritingPipeline,
  WritingStudioHead,
} from "./WritingStudio";
import { staleFiles, statusCounts, type WritingStatus } from "../writingStudio";
import type { WorklogArchiveList, WorklogWeekFile } from "../types";
import { localToday, parseWorklog, reportState } from "../weekTimeline";
import {
  QUADRANTS,
  REPORT_DESTINATION,
  classifyWorklog,
  visibleIdsFor,
  type QuadrantKey,
} from "../weeklyQuadrants";

/** 每个场景独立的「是否含下级目录」偏好，仅写在本机浏览器缓存里。 */
const SUBDIRS_KEY_PREFIX = "omnidesk.scene.include-subdirs.";

function readSubdirsPref(scene: SceneKey, fallback = false): boolean {
  try {
    const stored = window.localStorage.getItem(SUBDIRS_KEY_PREFIX + scene);
    return stored === null ? fallback : stored === "1";
  } catch {
    return fallback;
  }
}

function writeSubdirsPref(scene: SceneKey, value: boolean) {
  try {
    window.localStorage.setItem(SUBDIRS_KEY_PREFIX + scene, value ? "1" : "0");
  } catch {
    // 缓存写不进去时只影响下次默认值，不影响本次结果。
  }
}

/**
 * 是否连子目录一起列：写作页固定包含，工作记录页恢复为可切换偏好；其它场景沿用本机偏好。
 */
function fixedSubdirs(scene: SceneKey): boolean {
  const byPreference = readSubdirsPref(scene, scene === "weekly");
  return scene === "writing" ? true : byPreference;
}

/**
 * 只保留本场景的成果；树状模式下由这些文件反推目录行，
 * 避免把「只装着别的场景成果」的目录也显示出来。
 */
function filterSceneArtifacts(
  items: OfflineArtifactEntry[],
  scenario: string,
  includeSubdirs: boolean,
): OfflineArtifactEntry[] {
  const files = items.filter((i) => !i.is_dir && sameScenario(i.scenario, scenario));
  if (!includeSubdirs) return files;
  const dirs = new Set<string>();
  for (const f of files) {
    const parts = f.relative_path.split("/");
    for (let i = 1; i < parts.length; i += 1) dirs.add(parts.slice(0, i).join("/"));
  }
  const base = files.length > 0 ? Math.min(...files.map((f) => f.depth)) : 0;
  const rows: OfflineArtifactEntry[] = [
    ...Array.from(dirs).map((relative) => ({
      relative_path: relative,
      file_name: relative.split("/").pop() ?? relative,
      bytes: 0,
      modified_at: 0,
      scenario: "",
      is_dir: true,
      depth: relative.split("/").length - 1 - base,
    })),
    ...files.map((f) => ({ ...f, depth: f.depth - base })),
  ];
  rows.sort((a, b) =>
    a.depth !== b.depth
      ? a.depth - b.depth
      : a.relative_path.localeCompare(b.relative_path, "zh-Hans-CN"),
  );
  return rows;
}

/**
 * 场景页 v2（OD-ARCH-060）。
 *
 * 版式自上而下：场景心跳桥接（AnnouncementBar，046 的降级规则保留）→ 指标带 →
 * 本周时间线 ＋ 成果文档库两栏（项目跟进保留「进度」页签）→ 折叠会话历史（默认收起）。
 *
 * 场景页仍是**档案页**（OD-ARCH-057）：自己不承载对话框，要接着说事点「去对话」回首页。
 *
 * 诚实性：
 *   * 状态条数字来自宿主的 `scene_overview` 聚合命令（数已落盘记录，不调模型）。
 *   * 「进度」页签的数字来自**既有的确定性汇总命令**（`task_project_summary`），不是模型算的。
 *   * 「成果档案」只列**本场景自己保存**的成果（按 scenario 过滤），不读用户其它目录。
 *   * 场景归属是**入口即标签、事后可改**，不用模型猜归类。
 */
export function ScenePage({
  scene,
  onNavigate,
}: {
  scene: SceneKey;
  onNavigate: (scene: Scene) => void;
}) {
  // 改挂下拉与首页胶囊同源（OD-ARCH-062 S1）：显示名、新增分类都读宿主配置。
  // 未知旧标签不在列表里的，下拉保留原值一行的做法不成立，改由浏览器按 value 兜底。
  const tagOptions = useThreadCategories().map((c) => ({ key: c.key, label: c.label }));
  const def = SCENES[scene];
  const [threads, setThreads] = useState<SceneThread[]>(() => listThreadsByTag(scene));
  const [artifacts, setArtifacts] = useState<OfflineArtifactEntry[]>([]);
  const [artifactsError, setArtifactsError] = useState<string | null>(null);
  /** 成果元数据是否已读到（OD-ARCH-094 S01：没读到不能把草稿数写成 0）。 */
  const [artifactsLoaded, setArtifactsLoaded] = useState(false);
  const [opened, setOpened] = useState<{ name: string; body: string } | null>(null);
  /** 本周记录原文与归档清单（OD-ARCH-091）：都是宿主只读命令，不调模型。 */
  const [weekContent, setWeekContent] = useState<string>("");
  /** 宿主算出的当周标签（读周记录时一并拿到，归档命令也会给同一份）。 */
  const [weekLabel, setWeekLabel] = useState<string>("");
  const [weekLoading, setWeekLoading] = useState(false);
  const [weekError, setWeekError] = useState<string | null>(null);
  const [archive, setArchive] = useState<WorklogArchiveList | null>(null);
  const [archiveError, setArchiveError] = useState<string | null>(null);
  /**
   * 工作记录页的右栏提供「含子目录」开关；写作页继续固定包含，其余场景沿用本机偏好。
   */
  const [includeSubdirs, setIncludeSubdirs] = useState<boolean>(() =>
    fixedSubdirs(scene),
  );
  /** 项目跟进页的确定性汇总（OD-ARCH-098）：两栏卡片与甘特共用这一份数据。 */
  const [projectSummary, setProjectSummary] = useState<ProjectSummary | null>(null);
  const [projectLoading, setProjectLoading] = useState(false);
  const [projectError, setProjectError] = useState<string | null>(null);
  /** 项目页右栏视图：时间线 / 阶段成果只改视图，不重新取数。 */
  const [projectRightView, setProjectRightView] = useState<"timeline" | "artifacts">("timeline");
  const [overview, setOverview] = useState<SceneOverview | null>(null);
  const [overviewError, setOverviewError] = useState<string | null>(null);
  /** 刷新键：状态条与待确认面板共用，确认/放弃/定时登记之后一起重读。 */
  const [pendingTick, setPendingTick] = useState(0);
  /** 会话历史默认收起（OD-ARCH-060 S3）。 */
  const [showThreads, setShowThreads] = useState(false);
  /** 四象限选中格（OD-ARCH-093 S03）：只影响中栏显示，不写任何文件。 */
  const [quadrant, setQuadrant] = useState<QuadrantKey | null>(null);
  /** 写作页（OD-ARCH-094）：状态管线筛选与 90 天提醒筛选，都只改视图。 */
  const [writingStatusFilter, setWritingStatusFilter] = useState<WritingStatus | null>(null);
  const [writingStaleOnly, setWritingStaleOnly] = useState(false);
  /** 写作页的「现在」：一次渲染内固定，避免相对时间在每次重绘时抖动。 */
  const writingNow = useMemo(() => Math.floor(Date.now() / 1000), []);

  const refreshThreads = useCallback(() => {
    setThreads(listThreadsByTag(scene));
  }, [scene]);

  // 进场景时只读档案，**不**自动新建对话（对话只在首页发生）。
  useEffect(() => {
    setThreads(listThreadsByTag(scene));
  }, [scene]);

  /**
   * 换场景时页签回到该场景的默认值（OD-ARCH-091）。
   *
   * 场景页组件在 `report`/`writing`/`project` 之间是同一个 React 实例，
   * 不重置的话「项目跟进」的进度页签会带进别的场景，落在一个没有按钮的分支里。
   */
  useEffect(() => {
    setOpened(null);
    setQuadrant(null);
    setWritingStatusFilter(null);
    setWritingStaleOnly(false);
    setProjectRightView("timeline");
    setIncludeSubdirs(fixedSubdirs(scene));
  }, [scene, def.hasProgress]);

  // 成果档案：只列本场景自己保存的成果。开关决定是否含下级目录。
  useEffect(() => {
    let alive = true;
    setArtifactsLoaded(false);
    shell
      .offlineArtifacts(includeSubdirs)
      .then((r) => {
        if (!alive) return;
        setArtifacts(filterSceneArtifacts(r.items, def.scenario, includeSubdirs));
        setArtifactsError(null);
        setArtifactsLoaded(true);
      })
      .catch((e) => {
        if (!alive) return;
        // 读不到就如实报错，不能显示成「零成果」。
        setArtifacts([]);
        setArtifactsError(e instanceof Error ? e.message : String(e));
        setArtifactsLoaded(false);
      });
    return () => {
      alive = false;
    };
  }, [def.scenario, includeSubdirs]);

  // 状态条：宿主聚合命令，只数已落盘的元数据（不读正文、不调模型）。
  useEffect(() => {
    let alive = true;
    shell
      .sceneOverview([def.scenario])
      .then((r) => {
        if (!alive) return;
        const mine = r.items.find((i) => sameScenario(i.scenario, def.scenario)) ?? r.items[0] ?? null;
        setOverview(mine);
        setOverviewError(mine ? null : "宿主没有返回本场景的汇总");
      })
      .catch((e) => {
        if (!alive) return;
        setOverview(null);
        setOverviewError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      alive = false;
    };
  }, [def.scenario, pendingTick, includeSubdirs]);

  /** 本场景最近一次输入的事项文本（确定性汇总的输入）。 */
  const itemsText = useMemo(() => {
    for (const t of threads) {
      for (let i = t.messages.length - 1; i >= 0; i -= 1) {
        const m = t.messages[i];
        if (m.role === "user" && m.text.trim()) return m.text;
      }
    }
    return "";
  }, [threads]);

  // 项目跟进：既有确定性汇总（不调模型）。两栏卡片与甘特共用这份 summary。
  useEffect(() => {
    let alive = true;
    if (!def.hasProgress) return;
    if (!itemsText.trim()) {
      setProjectSummary(null);
      setProjectError(null);
      setProjectLoading(false);
      return;
    }
    setProjectLoading(true);
    shell
      .projectSummary(itemsText, null)
      .then((r) => {
        if (!alive) return;
        setProjectLoading(false);
        if (r.ok && r.summary) {
          setProjectSummary(r.summary);
          setProjectError(null);
        } else {
          setProjectSummary(null);
          setProjectError(r.error ?? "确定性汇总没有产出内容");
        }
      })
      .catch((e) => {
        if (!alive) return;
        setProjectLoading(false);
        setProjectSummary(null);
        setProjectError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      alive = false;
    };
  }, [def.hasProgress, itemsText]);

  const openArtifact = useCallback(async (fileName: string) => {
    try {
      const content = await shell.readOfflineArtifact(fileName);
      setOpened({ name: fileName, body: content.content });
    } catch (e) {
      setOpened({ name: fileName, body: "读取失败：" + (e instanceof Error ? e.message : String(e)) });
    }
  }, []);

  /**
   * 本周记录 + 归档清单（OD-ARCH-091）。两个命令都只读文件系统：
   * 周记录读当周 md 原文，归档只列元数据。失败就如实报错，不显示成空。
   */
  const currentWeek = archive?.current_week || weekLabel;

  /**
   * 重读当周记录（OD-ARCH-099 S03）：改完一条去向之后，统计卡、四象限与时间线
   * 都从这一份新内容重新算，不各自维护一套计数。
   */
  const reloadWeek = useCallback(async () => {
    setWeekLoading(true);
    try {
      const r = await shell.worklogReadWeek();
      setWeekContent(r.content ?? "");
      if (r.week) setWeekLabel(r.week);
      setWeekError(r.ok ? null : (r.error ?? "读不到本周记录"));
    } catch (e) {
      setWeekContent("");
      setWeekError(e instanceof Error ? e.message : String(e));
    } finally {
      setWeekLoading(false);
    }
  }, []);

  useEffect(() => {
    if (scene !== "weekly") return;
    let alive = true;
    setWeekLoading(true);
    shell
      .worklogReadWeek()
      .then((r) => {
        if (!alive) return;
        setWeekContent(r.content ?? "");
        if (r.week) setWeekLabel(r.week);
        setWeekError(r.ok ? null : (r.error ?? "读不到本周记录"));
        setWeekLoading(false);
      })
      .catch((e) => {
        if (!alive) return;
        setWeekContent("");
        setWeekError(e instanceof Error ? e.message : String(e));
        setWeekLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [scene, pendingTick]);

  useEffect(() => {
    if (scene !== "weekly") return;
    let alive = true;
    shell
      .worklogArchiveList()
      .then((r) => {
        if (!alive) return;
        setArchive(r);
        setArchiveError(r.ok ? null : (r.error ?? "读不到归档目录"));
      })
      .catch((e) => {
        if (!alive) return;
        setArchive(null);
        setArchiveError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      alive = false;
    };
  }, [scene, pendingTick]);

  /** 打开一份周报：按文件名交给宿主读，界面只负责显示。 */
  const openReport = useCallback(async (file: WorklogWeekFile) => {
    try {
      const r = await shell.worklogReadReport(file.file_name);
      setOpened({
        name: r.ok && r.path ? r.path : file.file_name,
        body: r.ok ? r.content : "读取失败：" + (r.error ?? "未知原因"),
      });
    } catch (e) {
      setOpened({ name: file.file_name, body: "读取失败：" + (e instanceof Error ? e.message : String(e)) });
    }
  }, []);

  /** 打开指定周的工作记录原文（同样走只读命令，不新开读取通道）。 */
  const openWeekSource = useCallback(async (week?: string) => {
    try {
      const r = await shell.worklogReadWeek(week);
      setOpened({
        name: r.path ?? `${week ?? "本周"} 工作记录`,
        body: r.exists ? r.content : "这一周还没有工作记录文件（不会为了查看而新建）。",
      });
    } catch (e) {
      setOpened({ name: week ?? "本周记录", body: "读取失败：" + (e instanceof Error ? e.message : String(e)) });
    }
  }, []);

  /**
   * 「转周报 / 去生成周报」：回首页新开一段工作记录对话，把「整理成周报」预填进输入框。
   * 只预填不发送——是否让模型跑一轮，由用户自己按发送决定。
   */
  const requestWeeklyReport = useCallback(() => {
    requestNewThread("weekly", "整理成周报");
    onNavigate("home");
  }, [onNavigate]);

  /** 回首页开一段新对话（带本场景标签）。 */
  const newThread = useCallback(() => {
    requestNewThread(scene);
    onNavigate("home");
  }, [scene, onNavigate]);

  /** 回首页继续这段对话。 */
  const openThread = useCallback(
    (id: string) => {
      requestOpenThread(id);
      onNavigate("home");
    },
    [onNavigate],
  );

  const retag = useCallback(
    (id: string, tag: ThreadTag) => {
      retagThread(id, tag);
      refreshThreads();
    },
    [refreshThreads],
  );

  const toggleSubdirs = useCallback(
    (next: boolean) => {
      setIncludeSubdirs(next);
      writeSubdirsPref(scene, next);
    },
    [scene],
  );

  const bumpPending = useCallback(() => setPendingTick((n) => n + 1), []);

  /** 回首页开一段写作对话（写作页「继续创作 / 去写点什么 / 去写」共用）。 */
  const newWritingThread = useCallback(() => {
    requestNewThread("writing");
    onNavigate("home");
  }, [onNavigate]);

  /** 写作页最近的写作对话：灵感架「继续对话」与「继续创作」的对话入口。 */
  const latestWritingThread = useMemo(
    () => [...threads].sort((a, b) => b.updatedAt - a.updatedAt)[0] ?? null,
    [threads],
  );
  const continueWritingThread = useCallback(() => {
    if (latestWritingThread) openThread(latestWritingThread.id);
    else newWritingThread();
  }, [latestWritingThread, openThread, newWritingThread]);

  /** 写作页指标：草稿数来自已读取的成果元数据；还没读到就是 null，界面显示「—」。 */
  const writingCounts = useMemo(
    () => (scene === "writing" && artifactsLoaded && !artifactsError ? statusCounts(artifacts) : null),
    [scene, artifacts, artifactsError, artifactsLoaded],
  );
  const writingStalePaths = useMemo(() => {
    if (scene !== "writing") return undefined;
    return new Set(staleFiles(artifacts, writingNow).map((f) => f.relative_path));
  }, [scene, artifacts, writingNow]);

  /** 本周周报状态（确定性文件检查，不调模型）。 */
  const reports = useMemo(() => archive?.reports ?? [], [archive]);
  const reportNow = useMemo(() => {
    const week = archive?.current_week ?? "";
    return week ? reportState(reports, week, week) : { state: "missing" as const, files: [] };
  }, [reports, archive]);

  /**
   * 指标带用的两个值（OD-ARCH-092 R1），都来自已经拉到的只读数据：
   *   * 本周记录条数取时间线解析结果，**不是**成果文件数——用户问的「我记了几条」；
   *   * 周报状态取确定性文件检查，未读到时给 null，界面显示占位而不是 0。
   */
  const weekEntries = useMemo(() => {
    if (scene !== "weekly") return null;
    if (weekError && !weekContent) return null;
    return parseWorklog(weekContent).entryCount;
  }, [scene, weekContent, weekError]);

  const reportLabel = useMemo(() => {
    if (scene !== "weekly") return null;
    if (!archive && archiveError) return null;
    return reportNow.state === "missing"
      ? "未生成"
      : reportNow.state === "draft"
        ? "草稿"
        : "已归档";
  }, [scene, archive, archiveError, reportNow]);

  /**
   * 四象限归类（OD-ARCH-093 S02）：与时间线共用同一份解析结果，避免两边数字打架。
   * 归类只用「日期是否已过」与「去向是否为本周报引用」，不调模型、不新增字段。
   */
  const classified = useMemo(
    () => classifyWorklog(parseWorklog(weekContent), localToday()),
    [weekContent],
  );

  /** 进周报条数：四象限里「去向 = 本周报引用」的两格之和。 */
  const weekReportEntries = useMemo(
    () => (scene === "weekly" ? classified.counts["past-report"] + classified.counts["future-report"] : null),
    [scene, classified],
  );

  const quadrantName = useMemo(
    () => QUADRANTS.find((q) => q.key === quadrant)?.title ?? null,
    [quadrant],
  );

  /**
   * 历史归档并入成果库（OD-ARCH-093 修订，用户 2026-10-27 要求）：
   * 往期周报与「还没有周报的往期周记录」都变成成果列表里的条目，只加一个「归档」徽标，
   * 不再单独占一张卡片。当周记录仍在中栏，不重复塞进成果库。
   */
  const archiveEntries = useMemo<ArtifactArchiveEntry[]>(() => {
    if (scene !== "weekly" || !archive) return [];
    const current = archive.current_week;
    const reports = archive.reports
      .filter((f) => f.week !== current)
      .map<ArtifactArchiveEntry>((f) => ({
        key: "report:" + f.file_name,
        kindLabel: "周报",
        week: f.week,
        file_name: f.file_name,
        path: f.path,
        modified_at: f.modified_at,
        onOpen: () => void openReport(f),
      }));
    const reportWeeks = new Set(archive.reports.map((f) => f.week));
    const weekSources = archive.weeks
      .filter((f) => f.week !== current && !reportWeeks.has(f.week))
      .map<ArtifactArchiveEntry>((f) => ({
        key: "week:" + f.file_name,
        kindLabel: "周记录",
        week: f.week,
        file_name: f.file_name,
        path: f.path,
        modified_at: f.modified_at,
        onOpen: () => void openWeekSource(f.week),
      }));
    return [...reports, ...weekSources];
  }, [scene, archive, openReport, openWeekSource]);

  const selectQuadrant = useCallback((key: QuadrantKey) => {
    setQuadrant((prev) => (prev === key ? null : key));
  }, []);

  /**
   * 点选某条记录「进周报 / 不进周报」（OD-ARCH-099 S03）。
   *
   * 只改这条记录在周文件里的去向字段，随后重读整个周文件——统计卡第 2 格、四象限
   * 两格与时间线开关都从这一份新内容重新算，不各维护一套计数。不写周报正文。
   * 失败时把宿主原因抛回时间线就地显示，勾选停在服务端真实值。
   */
  const toggleDestination = useCallback(
    async (args: {
      date: string;
      index: number;
      entry: { text: string; destination: string | null };
      next: boolean;
    }) => {
      const week = weekLabel || archive?.current_week;
      if (!week) throw new Error("还没读到当周标签，稍后再试");
      const receipt = await shell.worklogSetDestination({
        week,
        date: args.date,
        index: args.index,
        destination: args.next ? REPORT_DESTINATION : "仅记录",
        expectText: args.entry.text,
      });
      if (!receipt.ok) throw new Error(receipt.error ?? "改动未生效");
      await reloadWeek();
    },
    [archive, reloadWeek, weekLabel],
  );

  /**
   * 会话历史（OD-ARCH-060 S3，默认收起；OD-ARCH-092 可放进两栏的右栏）。
   * 展开后在自己的卡片里滚动，不再把下面整页推下去。
   */
  const threadsSection = (
    <section
      aria-label="历史对话"
      className="bg-white/85 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 min-w-0"
    >
      <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
        <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
          历史对话
        </h2>
        {def.subtitles?.history ? (
          <span className="text-[12px] uppercase tracking-[0.16em] text-outline">
            {def.subtitles.history}
          </span>
        ) : null}
        <span className="mono text-[12px] text-outline ml-auto shrink-0">{threads.length} 条</span>
      </div>
      <div className="flex items-center gap-2 pt-3.5">
        <button
          className="text-[12px] text-on-surface-variant hover:text-primary shrink-0"
          onClick={() => setShowThreads((v) => !v)}
          aria-expanded={showThreads}
        >
          {showThreads ? "▾" : "▸"} {showThreads ? "收起" : "展开"}对话列表
        </button>
        <span className="text-[12px] text-outline truncate">对话只在首页发生</span>
        <button
          className="ml-auto text-[12px] text-primary hover:underline shrink-0"
          onClick={newThread}
          title="回首页，按本场景开一段新对话"
        >
          去对话 · 新开
        </button>
      </div>
      {showThreads ? (
        <div className="pt-3.5 flex flex-col gap-1.5 max-h-[200px] overflow-y-auto">
          {threads.length === 0 ? (
            <p className="text-[12px] text-outline leading-relaxed">
              还没有这个场景的对话。去首页对话，场景选「{def.label}」即可。
            </p>
          ) : (
            threads.map((t) => (
              /*
               * 删除按钮放到单条对话最前面（2026-10-27 用户第五次反馈）。
               * 原先在行尾，被列表的纵向滚动条压住，点不到。
               */
              <div key={t.id} className="flex items-center gap-1 min-w-0 pr-1">
                <button
                  className="shrink-0 inline-flex h-6 w-6 items-center justify-center rounded-md text-outline hover:bg-error/10 hover:text-error"
                  onClick={() => {
                    removeThread(t.id);
                    refreshThreads();
                  }}
                  title="删除这条本机对话记录"
                  aria-label={`删除对话：${t.title}`}
                >
                  <Icon name="x" className="h-3.5 w-3.5" />
                </button>
                <button
                  onClick={() => openThread(t.id)}
                  className="min-w-0 flex-1 text-left px-2 py-2 rounded-lg text-[12px] truncate text-on-surface-variant hover:bg-surface-container"
                  title={t.title}
                >
                  {t.title}
                </button>
                <CategoryPicker
                  options={tagOptions}
                  value={t.tag}
                  onChange={(tag) => retag(t.id, tag as ThreadTag)}
                />
              </div>
            ))
          )}
        </div>
      ) : null}
    </section>
  );

  return (
    <main className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
      <SceneStatusStrip
        scene={def}
        overview={overview}
        overviewError={overviewError}
        refreshKey={pendingTick}
        weekEntries={weekEntries}
        weekReportEntries={weekReportEntries}
        reportLabel={reportLabel}
        onRequestReport={requestWeeklyReport}
        writingDraftCount={writingCounts ? writingCounts.draft : null}
        writingInspirationCount={scene === "writing" ? threads.length : null}
        writingNow={writingNow}
        onRequestWriting={scene === "writing" ? newWritingThread : undefined}
      />
      <AnnouncementBar onPendingChanged={bumpPending} />

      {scene === "writing" ? (
        /*
         * 文案写作主区（OD-ARCH-094 S02）：三栏同屏——左「继续创作 + 灵感架」、
         * 中「状态管线」、右「成果文档库」。窄窗按 左 → 中 → 右 单列堆叠。
         * 与工作记录页共享栏宽、圆角与间距，但用暖纸色 / 青绿强调区分气质。
         */
        <>
          <WritingStudioHead subtitle={def.subtitles?.studio} />
          <div
            className={
              "grid grid-cols-1 gap-4 lg:gap-5 items-start max-w-7xl mx-auto md:-translate-x-8 " +
              "lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)] " +
              "lg:[grid-template-areas:'left_pipeline_artifacts']"
            }
            data-writing-board=""
            data-board-width="nav"
          >
            <div className="min-w-0 flex flex-col gap-4 max-lg:order-1 lg:[grid-area:left] lg:max-h-[70vh] lg:overflow-y-auto">
              <WritingContinueCard
                files={artifacts.filter((f) => !f.is_dir)}
                nowSeconds={writingNow}
                loading={!artifactsLoaded && !artifactsError}
                subtitle={def.subtitles?.continue}
                onOpen={(p) => void openArtifact(p)}
                onContinueThread={continueWritingThread}
                onNewThread={newWritingThread}
              />
              <WritingInspirationShelf
                threads={threads}
                nowSeconds={writingNow}
                subtitle={def.subtitles?.inspiration}
                onOpenThread={openThread}
                onNewThread={newWritingThread}
              />
            </div>

            <div className="min-w-0 max-lg:order-2 lg:[grid-area:pipeline]">
              {artifactsError ? (
                <section
                  aria-label="状态管线"
                  className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-5 py-6"
                >
                  <p className="text-[11.5px] text-error leading-relaxed">
                    读取成果目录失败：{artifactsError}。这里不把它当成「没有成果」。
                  </p>
                </section>
              ) : (
                <WritingPipeline
                  files={artifacts}
                  nowSeconds={writingNow}
                  metadataLoading={!artifactsLoaded && !artifactsError}
                  subtitle={def.subtitles?.pipeline}
                  staleSubtitle={def.subtitles?.stale}
                  statusFilter={writingStatusFilter}
                  staleOnly={writingStaleOnly}
                  onStatusFilter={setWritingStatusFilter}
                  onStaleToggle={() => setWritingStaleOnly((v) => !v)}
                  onOpen={(p) => void openArtifact(p)}
                />
              )}
            </div>

            <section
              aria-label="成果文档库"
              className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 flex flex-col min-w-0 max-h-[70vh] max-lg:max-h-none max-lg:order-3 lg:[grid-area:artifacts]"
            >
              <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
                <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
                  成果文档库
                </h2>
                {def.subtitles?.artifacts ? (
                  <span className="text-[9.5px] uppercase tracking-[0.16em] text-outline">
                    {def.subtitles.artifacts}
                  </span>
                ) : null}
                <span className="mono text-[10.5px] text-outline ml-auto shrink-0">
                  {artifacts.filter((f) => !f.is_dir).length} 条
                </span>
              </div>
              <div className="pt-3.5 flex-1 flex flex-col min-h-0 overflow-y-auto">
                {artifactsError ? (
                  <p className="text-[11.5px] text-error leading-relaxed pb-2">
                    读取成果目录失败：{artifactsError}。这里不把它当成「没有成果」。
                  </p>
                ) : null}
                <ArtifactExplorer
                  files={artifacts}
                  emptyHint={def.emptyHint}
                  includeSubdirs={includeSubdirs}
                  onToggleSubdirs={toggleSubdirs}
                  showTypeLabels
                  stalePaths={writingStalePaths}
                  accent="secondary"
                  onOpen={(p) => void openArtifact(p)}
                />
              </div>
            </section>
          </div>
        </>
      ) : scene === "weekly" ? (
        /*
         * 工作记录主区（OD-ARCH-093 S03；2026-10-27 修订）：三栏同屏——左「四象限 + 历史对话」、
         * 中「本周记录」、右「成果文档库」。历史归档并入成果库，只留「归档」徽标。
         * 窄窗（lg 以下）按 四象限 → 本周记录 → 成果库 → 历史 的单列顺序堆叠。
         * 栏间距与卡片内边距按参考图放大，避免信息挤在一起（用户二次反馈）。
         */
        <div
          className={
            "grid grid-cols-1 gap-4 lg:gap-5 items-start max-w-7xl mx-auto md:-translate-x-8 " +
            "lg:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1fr)] " +
            "lg:[grid-template-areas:'left_log_artifacts']"
          }
          data-weekly-board=""
          data-board-width="nav"
        >
          {/*
           * 左栏滚动列（OD-ARCH-093 S03）：四象限 → 历史对话。归档已并入右栏成果库，
           * 这里不再单独成卡。窄窗用 `display: contents` 拆开，保证
           * 四象限 → 本周记录 → 成果库 → 历史 的阅读顺序。
           */}
          <div className="min-w-0 max-lg:contents lg:flex lg:flex-col lg:gap-4 lg:max-h-[70vh] lg:overflow-y-auto lg:[grid-area:left]">
          <div className="min-w-0 max-lg:order-1">
            <WeeklyQuadrants
              classified={classified}
              active={quadrant}
              onSelect={selectQuadrant}
              subtitle={def.subtitles?.priority}
              loading={weekLoading}
              error={weekError}
            />
          </div>

          {/* 四象限正下方就是历史对话（用户点名顺序）。 */}
          <div className="min-w-0 max-lg:order-4">{threadsSection}</div>
          </div>

          <div className="min-w-0 max-lg:order-2 lg:[grid-area:log]">
            <WeeklyTimeline
              week={currentWeek}
              content={weekContent}
              error={weekError}
              loading={weekLoading}
              onOpenSource={() => void openWeekSource()}
              onRequestReport={requestWeeklyReport}
              subtitle={def.subtitles?.log}
              visibleIds={visibleIdsFor(classified, quadrant)}
              filterLabel={quadrantName}
              onClearFilter={() => setQuadrant(null)}
              onToggleDestination={toggleDestination}
              statusRow={
                <WeeklyReportStatus
                  state={reportNow.state}
                  files={reportNow.files}
                  error={archiveError}
                  onRequestReport={requestWeeklyReport}
                  onOpenReport={(f) => void openReport(f)}
                />
              }
            />
          </div>

          <section
            aria-label="成果文档库"
            className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-4 py-4 lg:px-5 lg:py-5 flex flex-col min-w-0 max-h-[70vh] max-lg:max-h-none max-lg:order-3 lg:[grid-area:artifacts]"
          >
            <div className="flex items-baseline gap-2.5 pb-3.5 border-b border-surface-variant/25">
              <h2 className="font-headline-md text-[16px] font-semibold tracking-tight text-on-surface">
                成果文档库
              </h2>
              {def.subtitles?.artifacts ? (
                <span className="text-[12px] uppercase tracking-[0.16em] text-outline">
                  {def.subtitles.artifacts}
                </span>
              ) : null}
              <span className="mono text-[12px] text-outline ml-auto shrink-0">
                {artifacts.filter((f) => !f.is_dir).length + archiveEntries.length} 条
              </span>
            </div>
            <div className="pt-3.5 flex-1 flex flex-col min-h-0 overflow-y-auto">
              {artifactsError ? (
                <p className="text-[12px] text-error leading-relaxed pb-2">
                  读取成果目录失败：{artifactsError}。这里不把它当成「没有成果」。
                </p>
              ) : null}
              <ArtifactExplorer
                files={artifacts}
                archiveEntries={archiveEntries}
                archiveError={archiveError}
                archiveSkipped={archive?.skipped ?? 0}
                emptyHint={def.emptyHint}
                includeSubdirs={includeSubdirs}
                onToggleSubdirs={toggleSubdirs}
                compactToolbar
                onOpen={(p) => void openArtifact(p)}
              />
            </div>
          </section>
        </div>
      ) : scene === "project" ? (
        /*
         * 项目跟进（OD-ARCH-098 S03）：左「项目卡列表」+ 右「甘特时间线」，
         * 阶段成果收进右栏的次级切换。数据来自既有确定性汇总，页面不调模型。
         */
        <ProjectBoard
          summary={projectSummary}
          today={localToday()}
          error={projectError}
          loading={projectLoading}
          subtitleProjects={def.subtitles?.projects}
          subtitleTimeline={def.subtitles?.timeline}
          rightView={projectRightView}
          onRightView={setProjectRightView}
          artifactsSlot={
            <>
              {artifactsError ? (
                <p className="text-[12px] text-error leading-relaxed pb-2">
                  读取成果目录失败：{artifactsError}。这里不把它当成「没有成果」。
                </p>
              ) : null}
              <ArtifactExplorer
                files={artifacts}
                emptyHint={def.emptyHint}
                includeSubdirs={includeSubdirs}
                onToggleSubdirs={toggleSubdirs}
                onOpen={(p) => void openArtifact(p)}
              />
            </>
          }
        />
      ) : (
        /* 文案写作的兜底分支：只有成果文档库（项目跟进已改两栏，见上）。 */
        <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-[320px]">
          <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
            <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">成果文档库</h2>
          </div>

          <div className="pt-2 flex-1 flex flex-col min-h-0">
            {artifactsError ? (
              <p className="text-[11px] text-error leading-relaxed pb-1">
                读取成果目录失败：{artifactsError}。这里不把它当成「没有成果」。
              </p>
            ) : null}
            <ArtifactExplorer
              files={artifacts}
              emptyHint={def.emptyHint}
              includeSubdirs={includeSubdirs}
              onToggleSubdirs={toggleSubdirs}
              onOpen={(p) => void openArtifact(p)}
            />
          </div>
        </section>
      )}

      {/*
       * 原文 / 周报 / 成果的预览（OD-ARCH-097 S03）：不再作为页面流里的一块往下推，
       * 改成贴底浮层；主区高度与滚动位置在打开前后保持一致。
       */}
      <SourcePeekSheet
        open={opened !== null}
        title={opened?.name ?? ""}
        body={opened?.body ?? ""}
        onClose={() => setOpened(null)}
      />

      {/*
       * 归档 + 会话历史：weekly 已在三栏里（左栏四象限下方，OD-ARCH-093 S03）；
       * 其它场景仍在页面底部一条通栏，布局与原来一致。
       */}
      {scene === "weekly" || scene === "writing" ? null : (
        <div className="mt-2">{threadsSection}</div>
      )}
    </main>
  );
}

