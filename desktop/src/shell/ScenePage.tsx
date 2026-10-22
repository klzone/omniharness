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
import { AnnouncementBar } from "./AnnouncementBar";
import { ArtifactExplorer } from "./ArtifactExplorer";
import type { Scene } from "./Dock";
import { SceneStatusStrip } from "./SceneStatusStrip";

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

/** 本周一 00:00（本机时区）的 Unix 秒，用于「本期记录碎片」的窗口。 */
function weekStartSecs(now: Date): number {
  const monday = new Date(now);
  monday.setHours(0, 0, 0, 0);
  monday.setDate(monday.getDate() - ((now.getDay() + 6) % 7));
  return monday.getTime() / 1000;
}

/** 文件名 → 可读标题（去掉 .md，下划线/连字符换成空格）。只做展示，不改文件。 */
function titleFromFile(name: string): string {
  return name.replace(/\.md$/i, "").replace(/[_-]+/g, " ").trim() || name;
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
 * 版式自上而下：场景状态条 → 规则提醒/AI 复盘（AnnouncementBar，046 的降级规则保留）
 * → 成果文档库（项目跟进保留「进度」页签）→ 底部折叠会话历史（默认收起）。
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
  const def = SCENES[scene];
  const [threads, setThreads] = useState<SceneThread[]>(() => listThreadsByTag(scene));
  const [tab, setTab] = useState<"progress" | "artifacts">(def.hasProgress ? "progress" : "artifacts");
  const [artifacts, setArtifacts] = useState<OfflineArtifactEntry[]>([]);
  const [artifactsError, setArtifactsError] = useState<string | null>(null);
  const [opened, setOpened] = useState<{ name: string; body: string } | null>(null);
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

  /**
   * 「本期记录碎片」（OD-ARCH-060 S4，仅工作记录页）：
   * 本周已保存的成果条目。只按修改时间窗口过滤已列出的元数据，
   * 不调模型、不推断「事项」内容——标题就是文件名，点开才读正文。
   */
  const weekFragments = useMemo(() => {
    const from = weekStartSecs(new Date());
    return artifacts
      .filter((a) => !a.is_dir && a.modified_at >= from)
      .sort((a, b) => b.modified_at - a.modified_at);
  }, [artifacts]);

  return (
    <main className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
      <SceneStatusStrip
        scene={def}
        overview={overview}
        overviewError={overviewError}
        refreshKey={pendingTick}
      />
      <AnnouncementBar
        scene={def}
        itemsText={itemsText}
        onOpenArtifact={(f) => void openArtifact(f)}
        onPendingChanged={bumpPending}
      />

      {scene === "weekly" ? (
        <section className="mb-2 bg-white/70 border border-white/90 rounded-2xl px-3 py-2">
          <div className="flex items-center gap-2">
            <h2 className="text-[11.5px] font-semibold text-on-surface">本期记录碎片</h2>
            <span className="text-[10px] text-outline">
              本周已保存 {weekFragments.length} 条（按文件保存时间统计，不调模型）
            </span>
          </div>
          {weekFragments.length === 0 ? (
            <p className="pt-1 text-[10.5px] text-outline leading-relaxed">
              本周还没有保存的记录。保存后的成果会出现在这里；这里不推测你做过什么。
            </p>
          ) : (
            <ul className="pt-1 flex flex-wrap gap-1">
              {weekFragments.slice(0, 8).map((f) => (
                <li key={f.relative_path}>
                  <button
                    className="px-2 py-0.5 rounded-full bg-surface-container text-[10.5px] text-on-surface-variant hover:bg-surface-container-high truncate max-w-[240px]"
                    title={f.relative_path}
                    onClick={() => void openArtifact(f.relative_path)}
                  >
                    {titleFromFile(f.file_name)}
                  </button>
                </li>
              ))}
              {weekFragments.length > 8 ? (
                <li className="text-[10.5px] text-outline px-1 py-0.5">另有 {weekFragments.length - 8} 条</li>
              ) : null}
            </ul>
          )}
        </section>
      ) : null}

      {/* 主区：成果文档库（项目跟进保留「进度」页签） */}
      <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-[320px]">
        {def.hasProgress ? (
          <div className="flex items-center gap-1 pb-2 border-b border-surface-variant/20">
            <button
              className={
                "px-2 py-1 rounded-full text-[11px] " +
                (tab === "progress" ? "bg-primary text-on-primary" : "bg-surface-container")
              }
              onClick={() => setTab("progress")}
            >
              进度
            </button>
            <button
              className={
                "px-2 py-1 rounded-full text-[11px] " +
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

        {opened ? (
          <div className="mt-2 border-t border-surface-variant/20 pt-2">
            <div className="flex items-center justify-between">
              <span className="mono text-[10.5px] text-outline truncate" title={opened.name}>
                {opened.name}
              </span>
              <button className="text-[10.5px] text-outline hover:underline" onClick={() => setOpened(null)}>
                收起
              </button>
            </div>
            <pre className="text-[11px] text-on-surface-variant whitespace-pre-wrap leading-relaxed mt-1 max-h-[40vh] overflow-y-auto">
              {opened.body}
            </pre>
          </div>
        ) : null}
      </section>

      {/* 底部折叠区：本场景历史对话（OD-ARCH-060 S3，默认收起） */}
      <section className="mt-2 bg-white/70 border border-white/90 rounded-2xl px-3 py-2">
        <div className="flex items-center gap-2">
          <button
            className="text-[11.5px] text-on-surface-variant hover:text-primary shrink-0"
            onClick={() => setShowThreads((v) => !v)}
            aria-expanded={showThreads}
          >
            {showThreads ? "▾" : "▸"} 会话历史（{threads.length} 条）
          </button>
          <span className="text-[10px] text-outline truncate">对话只在首页发生；这里是本场景已归档的会话</span>
          <button
            className="ml-auto text-[11px] text-primary hover:underline shrink-0"
            onClick={newThread}
            title="回首页，按本场景开一段新对话"
          >
            去对话 · 新开
          </button>
        </div>
        {showThreads ? (
          <div className="pt-2 flex flex-col gap-1 max-h-[240px] overflow-y-auto">
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
                    <option value="weekly">工作记录</option>
                    <option value="writing">文案</option>
                    <option value="project">项目</option>
                    <option value="plain">普通</option>
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
    </main>
  );
}
