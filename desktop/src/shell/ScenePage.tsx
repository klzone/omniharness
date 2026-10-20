import { useCallback, useEffect, useMemo, useState } from "react";
import { shell } from "../api";
import { SCENES, type SceneKey } from "../scenes";
import type { OfflineFlowPack } from "../types";
import {
  createThread,
  listThreadsByTag,
  removeThread,
  retagThread,
  saveThreadMessages,
  type SceneThread,
  type ThreadTag,
} from "../sceneThreads";
import type { OfflineArtifactEntry } from "../types";
import { AnnouncementBar } from "./AnnouncementBar";
import { HomeChat } from "./HomeChat";
import { SceneScaffold } from "./SceneScaffold";
import type { Scene } from "./Dock";

/**
 * 场景页（OD-ARCH-033 目标 2/3/5）。
 *
 * 版式与首页一致（同一个 `SceneScaffold`）：
 *   提醒 / 建议栏（横向）→ 双栏（左：本场景历史对话；右：成果档案，项目跟进多一个「进度」页签）
 *   → 下部对话区（与首页同一个 `HomeChat`）。
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
  const [activeId, setActiveId] = useState<string | null>(null);
  const [tab, setTab] = useState<"progress" | "artifacts">(def.hasProgress ? "progress" : "artifacts");
  const [artifacts, setArtifacts] = useState<OfflineArtifactEntry[]>([]);
  const [opened, setOpened] = useState<{ name: string; body: string } | null>(null);
  const [progress, setProgress] = useState<string | null>(null);
  const [progressError, setProgressError] = useState<string | null>(null);
  const [pack, setPack] = useState<OfflineFlowPack | null>(null);

  // 场景预设的规则来自可编辑的流程包（缺省时由组装器回落内置默认）。
  useEffect(() => {
    shell
      .offlineFlowPack()
      .then((r) => setPack(r.ok ? r.pack : null))
      .catch(() => setPack(null));
  }, []);

  const preset = useMemo(() => {
    const flow = pack ? pack[def.preset] : null;
    return {
      key: def.preset,
      label: def.label,
      rules: flow?.draft?.rules ?? null,
      sections: flow?.draft?.sections ?? null,
    };
  }, [pack, def.preset, def.label]);

  const refreshThreads = useCallback(() => {
    setThreads(listThreadsByTag(scene));
  }, [scene]);

  // 进场景时若没有正在进行的对话，就开一段新的（带场景标签）。
  useEffect(() => {
    const existing = listThreadsByTag(scene);
    if (existing.length === 0) {
      const t = createThread({ tag: scene, preset: def.preset });
      setThreads([t]);
      setActiveId(t.id);
    } else {
      setThreads(existing);
      setActiveId((prev) => prev ?? existing[0].id);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scene]);

  // 成果档案：只列本场景自己保存的成果。
  useEffect(() => {
    let alive = true;
    shell
      .offlineArtifacts()
      .then((r) => {
        if (!alive) return;
        setArtifacts(r.items.filter((i) => i.scenario === def.scenario));
      })
      .catch(() => {
        if (alive) setArtifacts([]);
      });
    return () => {
      alive = false;
    };
  }, [def.scenario, activeId]);

  const active = threads.find((t) => t.id === activeId) ?? null;

  /** 本场景最近一次输入的事项文本（确定性汇总的输入）。 */
  const itemsText = useMemo(() => {
    const pool = active ? [active, ...threads] : threads;
    for (const t of pool) {
      for (let i = t.messages.length - 1; i >= 0; i -= 1) {
        const m = t.messages[i];
        if (m.role === "user" && m.text.trim()) return m.text;
      }
    }
    return "";
  }, [threads, active]);

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

  const newThread = useCallback(() => {
    const t = createThread({ tag: scene, preset: def.preset });
    setThreads(listThreadsByTag(scene));
    setActiveId(t.id);
  }, [scene, def.preset]);

  const retag = useCallback(
    (id: string, tag: ThreadTag) => {
      retagThread(id, tag);
      refreshThreads();
    },
    [refreshThreads],
  );

  const upper = (
    <>
      <AnnouncementBar scene={def} itemsText={itemsText} onOpenArtifact={(f) => void openArtifact(f)} />
      <div className="flex flex-col lg:flex-row gap-4 flex-1 min-h-0 max-h-[30vh]">
        {/* 左栏：本场景历史对话 */}
        <section className="lg:w-[300px] shrink-0 bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-0">
          <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
            <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">
              {def.label} · 历史对话
            </h2>
            <button
              className="text-[11px] text-primary hover:underline"
              onClick={newThread}
              title="在本格场景里开一段新对话"
            >
              新对话
            </button>
          </div>
          <div className="pt-2 flex flex-col gap-1 min-h-0 overflow-y-auto flex-1">
            {threads.length === 0 ? (
              <p className="text-[11px] text-outline leading-relaxed">
                还没有这个场景的对话。点「新对话」直接说事。
              </p>
            ) : (
              threads.map((t) => (
                <div key={t.id} className="flex items-center gap-1">
                  <button
                    onClick={() => setActiveId(t.id)}
                    className={
                      "flex-1 text-left px-2 py-1.5 rounded-lg text-[11.5px] truncate " +
                      (t.id === activeId
                        ? "bg-primary/10 text-primary font-medium"
                        : "text-on-surface-variant hover:bg-surface-container")
                    }
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
                    <option value="weekly">周报</option>
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
              <span className="ml-auto text-[10px] text-outline">数据来自确定性汇总与本场景保存的成果</span>
            </div>
          ) : (
            <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
              <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">成果档案</h2>
              <span className="text-[10px] text-outline">只列本场景保存的成果</span>
            </div>
          )}

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
                    : "还没有可汇总的事项。在下面的对话里写下事项（日期｜事项｜状态），这里会按本机时间给出进度、逾期与阶段汇总——确定性计算，不调模型。"}
                </p>
              )
            ) : artifacts.length === 0 ? (
              <p className="text-[11px] text-outline leading-relaxed">{def.emptyHint}</p>
            ) : (
              <ul className="flex flex-col gap-1">
                {artifacts.map((a) => (
                  <li key={a.file_name} className="flex items-center gap-2">
                    <span className="mono text-[11px] text-on-surface-variant truncate">
                      {a.file_name}
                    </span>
                    <span className="text-[10px] text-outline">{a.bytes} 字节</span>
                    <button
                      className="text-[10.5px] text-primary hover:underline ml-auto"
                      onClick={() => void openArtifact(a.file_name)}
                    >
                      打开
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {opened ? (
              <div className="mt-2">
                <div className="flex items-center justify-between">
                  <span className="mono text-[10.5px] text-outline">{opened.name}</span>
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
    </>
  );

  return (
    <SceneScaffold
      upper={upper}
      chat={
        <HomeChat
          key={activeId ?? "none"}
          onNavigate={onNavigate}
          preset={preset}
          threadKey={activeId ?? "none"}
          initialMessages={active?.messages ?? []}
          onMessagesChange={(messages) => {
            if (!activeId) return;
            saveThreadMessages(activeId, messages);
            refreshThreads();
          }}
        />
      }
    />
  );
}
