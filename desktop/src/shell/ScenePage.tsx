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
import type { OfflineArtifactEntry, SceneOverview } from "../types";
import { useThreadCategories } from "../threadCategories";
import { AnnouncementBar } from "./AnnouncementBar";
import { ArtifactExplorer } from "./ArtifactExplorer";
import type { Scene } from "./Dock";
import { SceneStatusStrip } from "./SceneStatusStrip";
import { WeeklyTimeline } from "./WeeklyTimeline";
import { WeeklyArchive, WeeklyReportStatus } from "./WeeklyReportPanel";
import type { WorklogArchiveList, WorklogWeekFile } from "../types";
import { parseWorklog, reportState } from "../weekTimeline";

/** 每个场景独立的「是否含下级目录」偏好，仅写在本机浏览器缓存里。 */
const SUBDIRS_KEY_PREFIX = "omnidesk.scene.include-subdirs.";

function readSubdirsPref(scene: SceneKey): boolean {
  try {
    return window.localStorage.getItem(SUBDIRS_KEY_PREFIX + scene) === "1";
  } catch {
    return false;
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
 * 版式自上而下：规则提醒（AnnouncementBar，046 的降级规则保留）→ 指标带 →
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
  /**
   * 成果区页签，只给项目跟进 / 文案写作用（weekly 已改两栏，见 OD-ARCH-092 R2）。
   */
  const [tab, setTab] = useState<"progress" | "artifacts">(
    def.hasProgress ? "progress" : "artifacts",
  );
  const [artifacts, setArtifacts] = useState<OfflineArtifactEntry[]>([]);
  const [artifactsError, setArtifactsError] = useState<string | null>(null);
  const [opened, setOpened] = useState<{ name: string; body: string } | null>(null);
  /** 本周记录原文与归档清单（OD-ARCH-091）：都是宿主只读命令，不调模型。 */
  const [weekContent, setWeekContent] = useState<string>("");
  /** 宿主算出的当周标签（读周记录时一并拿到，归档命令也会给同一份）。 */
  const [weekLabel, setWeekLabel] = useState<string>("");
  const [weekLoading, setWeekLoading] = useState(false);
  const [weekError, setWeekError] = useState<string | null>(null);
  const [archive, setArchive] = useState<WorklogArchiveList | null>(null);
  const [archiveError, setArchiveError] = useState<string | null>(null);
  const [includeSubdirs, setIncludeSubdirs] = useState<boolean>(() => readSubdirsPref(scene));
  const [progress, setProgress] = useState<string | null>(null);
  const [progressError, setProgressError] = useState<string | null>(null);
  const [overview, setOverview] = useState<SceneOverview | null>(null);
  const [overviewError, setOverviewError] = useState<string | null>(null);
  /** 刷新键：状态条与待确认面板共用，确认/放弃/定时登记之后一起重读。 */
  const [pendingTick, setPendingTick] = useState(0);
  /** 会话历史默认收起（OD-ARCH-060 S3）。 */
  const [showThreads, setShowThreads] = useState(false);

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
    setTab(def.hasProgress ? "progress" : "artifacts");
    setOpened(null);
  }, [scene, def.hasProgress]);

  // 成果档案：只列本场景自己保存的成果。开关决定是否含下级目录。
  useEffect(() => {
    let alive = true;
    shell
      .offlineArtifacts(includeSubdirs)
      .then((r) => {
        if (!alive) return;
        setArtifacts(filterSceneArtifacts(r.items, def.scenario, includeSubdirs));
        setArtifactsError(null);
      })
      .catch((e) => {
        if (!alive) return;
        // 读不到就如实报错，不能显示成「零成果」。
        setArtifacts([]);
        setArtifactsError(e instanceof Error ? e.message : String(e));
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

  // 进度：既有确定性汇总（不调模型）。
  useEffect(() => {
    let alive = true;
    if (!def.hasProgress || tab !== "progress") return;
    if (!itemsText.trim()) {
      setProgress(null);
      setProgressError(null);
      return;
    }
    shell
      .projectSummary(itemsText, null)
      .then((r) => {
        if (!alive) return;
        if (r.ok && r.markdown) {
          setProgress(r.markdown);
          setProgressError(null);
        } else {
          setProgress(null);
          setProgressError(r.error ?? "确定性汇总没有产出内容");
        }
      })
      .catch((e) => {
        if (!alive) return;
        setProgress(null);
        setProgressError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      alive = false;
    };
  }, [def.hasProgress, tab, itemsText]);

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
   * 会话历史（OD-ARCH-060 S3，默认收起；OD-ARCH-092 可放进两栏的右栏）。
   * 展开后在自己的卡片里滚动，不再把下面整页推下去。
   */
  const threadsSection = (
    <section className="bg-white/70 border border-white/90 rounded-2xl px-3 py-2 min-w-0">
      <div className="flex items-center gap-2">
        <button
          className="text-[11.5px] text-on-surface-variant hover:text-primary shrink-0"
          onClick={() => setShowThreads((v) => !v)}
          aria-expanded={showThreads}
        >
          {showThreads ? "▾" : "▸"} 会话历史（{threads.length} 条）
        </button>
        <span className="text-[10px] text-outline truncate">对话只在首页发生</span>
        <button
          className="ml-auto text-[11px] text-primary hover:underline shrink-0"
          onClick={newThread}
          title="回首页，按本场景开一段新对话"
        >
          去对话 · 新开
        </button>
      </div>
      {showThreads ? (
        <div className="pt-2 flex flex-col gap-1 max-h-[200px] overflow-y-auto">
          {threads.length === 0 ? (
            <p className="text-[11px] text-outline leading-relaxed">
              还没有这个场景的对话。去首页对话，场景选「{def.label}」即可。
            </p>
          ) : (
            threads.map((t) => (
              <div key={t.id} className="flex items-center gap-1">
                <button
                  onClick={() => openThread(t.id)}
                  className="flex-1 text-left px-2 py-1.5 rounded-lg text-[11.5px] truncate text-on-surface-variant hover:bg-surface-container"
                  title={t.title}
                >
                  {t.title}
                </button>
                <select
                  className="text-[10px] px-1 py-0.5 rounded border border-surface-variant/30 bg-surface-container-lowest/60"
                  value={t.tag}
                  onChange={(e) => retag(t.id, e.target.value as ThreadTag)}
                  title="改挂到别的场景"
                >
                  {tagOptions.map((o) => (
                    <option key={o.key} value={o.key}>
                      {o.label}
                    </option>
                  ))}
                </select>
                <button
                  className="text-[10px] text-outline hover:text-error px-1"
                  onClick={() => {
                    removeThread(t.id);
                    refreshThreads();
                  }}
                  title="删除这条本机对话记录"
                >
                  ×
                </button>
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
        reportLabel={reportLabel}
        onRequestReport={requestWeeklyReport}
      />
      <AnnouncementBar scene={def} itemsText={itemsText} onPendingChanged={bumpPending} />

      {scene === "weekly" ? (
        /*
         * 工作记录主区（OD-ARCH-092 R2）：取消页签，左主右辅两栏同屏。
         * 左边看这一周记了什么，右边就是本场景的成果库——原来两者互斥，
         * 想看成果必须先切走时间线。窄窗（lg 以下）自动堆成单列。
         */
        <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)] gap-2 items-start">
          <WeeklyTimeline
            week={currentWeek}
            content={weekContent}
            error={weekError}
            loading={weekLoading}
            onOpenSource={() => void openWeekSource()}
            onRequestReport={requestWeeklyReport}
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

          <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] px-3 py-3 flex flex-col min-w-0 max-h-[70vh]">
            <div className="flex items-baseline gap-2 pb-2 border-b border-surface-variant/20">
              <h2 className="font-headline-md text-[12.5px] font-semibold text-on-surface">成果文档库</h2>
              <span className="mono text-[10px] text-outline">{artifacts.length} 份</span>
            </div>
            <div className="pt-2 flex-1 flex flex-col min-h-0 overflow-y-auto">
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
        </div>
      ) : (
        /* 项目跟进 / 文案写作：页签结构不变（OD-ARCH-092 未改这两个场景）。 */
        <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-[320px]">
          {def.hasProgress ? (
            <div className="flex items-center gap-1 pb-2 border-b border-surface-variant/20">
              <button
                className={
                  "px-2 py-1 text-[11px] " +
                  (tab === "progress" ? "bg-primary text-on-primary" : "bg-surface-container")
                }
                onClick={() => setTab("progress")}
              >
                进度
              </button>
              <button
                className={
                  "px-2 py-1 text-[11px] " +
                  (tab === "artifacts" ? "bg-primary text-on-primary" : "bg-surface-container")
                }
                onClick={() => setTab("artifacts")}
              >
                阶段成果
              </button>
            </div>
          ) : (
            <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
              <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">成果文档库</h2>
            </div>
          )}

          {tab === "artifacts" ? (
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
          ) : (
            <div className="pt-2 flex-1 overflow-y-auto">
              {progress ? (
                <pre className="text-[11.5px] text-on-surface-variant whitespace-pre-wrap leading-relaxed">
                  {progress}
                </pre>
              ) : (
                <p className="text-[11px] text-outline leading-relaxed">
                  {progressError
                    ? "确定性汇总没有产出：" + progressError
                    : "还没有可汇总的事项。在首页对话里写下事项（日期｜事项｜状态），这里会按本机时间给出进度、逾期与阶段汇总——确定性计算，不调模型。"}
                </p>
              )}
            </div>
          )}
        </section>
      )}

      {opened ? (
        <section className="mt-2 bg-white/80 border border-white/90 rounded-2xl px-4 py-2">
          <div className="flex items-center justify-between gap-2">
            <span className="mono text-[10.5px] text-outline truncate" title={opened.name}>
              {opened.name}
            </span>
            <button className="text-[10.5px] text-outline hover:underline shrink-0" onClick={() => setOpened(null)}>
              收起
            </button>
          </div>
          <pre className="text-[11px] text-on-surface-variant whitespace-pre-wrap leading-relaxed mt-1 max-h-[40vh] overflow-y-auto">
            {opened.body}
          </pre>
        </section>
      ) : null}

      {/*
       * 归档 + 会话历史（OD-ARCH-092 R4）：原来两块各占一条通栏，纵向浪费；
       * 现在并排两栏，各自在栏内滚动，会话展开也不再下推整页。
       */}
      {scene === "weekly" ? (
        <div className="mt-2 grid grid-cols-1 lg:grid-cols-2 gap-2 items-start">
          <WeeklyArchive
            reports={reports}
            weeks={archive?.weeks ?? []}
            skipped={archive?.skipped ?? 0}
            currentWeek={currentWeek}
            error={archiveError}
            onOpenReport={(f) => void openReport(f)}
            onOpenSource={(w) => void openWeekSource(w)}
          />
          {threadsSection}
        </div>
      ) : (
        <div className="mt-2">{threadsSection}</div>
      )}
    </main>
  );
}

