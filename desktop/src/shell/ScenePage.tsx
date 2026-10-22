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
import type { OfflineArtifactEntry } from "../types";
import { AnnouncementBar } from "./AnnouncementBar";
import type { Scene } from "./Dock";

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
 * 场景页（OD-ARCH-033 目标 2/3/5）。
 *
 * 场景页是**档案页**（OD-ARCH-057）：只列本场景的对话与成果，自己不承载对话框。
 * 要接着说事，点「去对话」回首页——整个应用只有首页那一个对话框。
 *
 * 诚实性：
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
  const [opened, setOpened] = useState<{ name: string; body: string } | null>(null);
  const [includeSubdirs, setIncludeSubdirs] = useState<boolean>(() => readSubdirsPref(scene));
  const [progress, setProgress] = useState<string | null>(null);
  const [progressError, setProgressError] = useState<string | null>(null);

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
      })
      .catch(() => {
        if (alive) setArtifacts([]);
      });
    return () => {
      alive = false;
    };
  }, [def.scenario, includeSubdirs]);

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

  return (
    <main className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
      <div className="shrink-0">
        <AnnouncementBar
          scene={def}
          itemsText={itemsText}
          onOpenArtifact={(f) => void openArtifact(f)}
        />
      </div>
      <div className="flex flex-col lg:flex-row gap-4 lg:items-stretch">
        {/* 左栏：本场景历史对话 */}
        <section className="lg:w-[300px] shrink-0 bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-0">
          <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
            <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">
              {def.label} · 历史对话
            </h2>
            <button
              className="text-[11px] text-primary hover:underline"
              onClick={newThread}
              title="回首页，按本场景开一段新对话"
            >
              去对话 · 新开
            </button>
          </div>
          <div className="pt-2 flex flex-col gap-1 min-h-0 overflow-y-auto flex-1">
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
        </section>

        {/* 右栏：成果档案 / 进度 */}
        <section className="flex-1 min-w-0 bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-0">
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
              <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">成果档案</h2>
            </div>
          )}

          {tab === "artifacts" ? (
            <div className="flex items-center gap-2 pt-1.5 pb-1 border-b border-surface-variant/10">
              <span className="text-[10px] text-outline">只列本场景保存的成果</span>
              <label
                className="ml-auto flex items-center gap-1 text-[10.5px] text-on-surface-variant cursor-pointer select-none"
                title="打开后按文件夹层级列出数据根下级目录里的成果；关闭则只看最外层"
              >
                <input
                  type="checkbox"
                  className="accent-current"
                  checked={includeSubdirs}
                  onChange={(e) => {
                    const next = e.target.checked;
                    setIncludeSubdirs(next);
                    writeSubdirsPref(scene, next);
                  }}
                />
                含下级目录
              </label>
            </div>
          ) : null}

          {tab === "artifacts" ? (
            <p className="pt-1.5 text-[10px] text-outline leading-relaxed">
              成果是标准 Markdown：可在 Obsidian 里「打开文件夹作为 vault」，指向设置页显示的成果目录即可直接浏览。
              OmniDesk 只写自己生成的成果，不读取也不改动你 vault 里的其他笔记。
            </p>
          ) : null}

          <div className="pt-2 min-h-0 overflow-y-auto flex-1">
            {tab === "progress" ? (
              progress ? (
                <pre className="text-[11.5px] text-on-surface-variant whitespace-pre-wrap leading-relaxed">
                  {progress}
                </pre>
              ) : (
                <p className="text-[11px] text-outline leading-relaxed">
                  {progressError
                    ? "确定性汇总没有产出：" + progressError
                    : "还没有可汇总的事项。在首页对话里写下事项（日期｜事项｜状态），这里会按本机时间给出进度、逾期与阶段汇总——确定性计算，不调模型。"}
                </p>
              )
            ) : artifacts.length === 0 ? (
              <p className="text-[11px] text-outline leading-relaxed">
                {includeSubdirs
                  ? def.emptyHint + "（已包含下级目录；当前没有找到本场景的成果。）"
                  : def.emptyHint}
              </p>
            ) : (
              <ul className="flex flex-col gap-0.5">
                {artifacts.map((a) => (
                  <li
                    key={a.relative_path}
                    className="flex items-center gap-2"
                    style={{ paddingLeft: a.depth * 14 }}
                  >
                    {a.is_dir ? (
                      <span className="text-[11px] text-outline truncate" title={a.relative_path}>
                        ▸ {a.file_name}
                      </span>
                    ) : (
                      <>
                        <span
                          className="mono text-[11px] text-on-surface-variant truncate"
                          title={a.relative_path}
                        >
                          {a.file_name}
                        </span>
                        <span className="text-[10px] text-outline shrink-0">{a.bytes} 字节</span>
                        <button
                          className="text-[10.5px] text-primary hover:underline ml-auto shrink-0"
                          onClick={() => void openArtifact(a.relative_path)}
                        >
                          打开
                        </button>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            )}
            {opened ? (
              <div className="mt-2">
                <div className="flex items-center justify-between">
                  <span className="mono text-[10.5px] text-outline truncate" title={opened.name}>
                    {opened.name}
                  </span>
                  <button className="text-[10.5px] text-outline hover:underline" onClick={() => setOpened(null)}>
                    收起
                  </button>
                </div>
                <pre className="text-[11px] text-on-surface-variant whitespace-pre-wrap leading-relaxed mt-1">
                  {opened.body}
                </pre>
              </div>
            ) : null}
          </div>
        </section>
      </div>
    </main>
  );
}
