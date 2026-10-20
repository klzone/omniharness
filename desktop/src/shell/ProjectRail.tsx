import { useEffect, useState } from "react";
import { host } from "../api";
import { Icon } from "../components/Icon";
import type { SessionSummary } from "../types";

/**
 * 项目 / 会话侧栏（OD-ARCH-031 Part B）。
 *
 * 「项目」= 一个**工作目录**（引擎按 cwd 分组，宿主用不透明 token 标识，我们只用自己的路径）。
 * 「会话」分两类，都如实标注：
 *   * **工作台对话**：本工作台自己的连续对话线程，可继续、可恢复（本地存储，只在界面内）。
 *   * **引擎会话**：该目录下 DSH 的真实会话（只读元数据，不可在此续聊）。
 *
 * 边界：
 *   * 不自己造「可恢复」结论——引擎会话能否恢复由宿主 `recover_session` 判定，界面不猜。
 *   * 项目路径必须是绝对路径；这里只做**记住**，不创建、不删除任何目录。
 */

export interface WorkThread {
  id: string;
  projectPath: string;
  title: string;
  preset: string | null;
  updatedAt: number;
}

export interface ProjectRailProps {
  projects: string[];
  activeProject: string | null;
  threads: WorkThread[];
  activeThreadId: string | null;
  engineSessions: SessionSummary[];
  engineSessionsError: string | null;
  onSelectProject: (path: string) => void;
  onSelectThread: (id: string) => void;
  onNewThread: () => void;
  onAddProject: (path: string) => void;
  /** 开场预设（场景）选项。 */
  presets: Array<{ key: string; label: string }>;
  presetKey: string;
  onSelectPreset: (key: string) => void;
}

export function ProjectRail(props: ProjectRailProps) {
  const {
    projects,
    activeProject,
    threads,
    activeThreadId,
    engineSessions,
    engineSessionsError,
    onSelectProject,
    onSelectThread,
    onNewThread,
    onAddProject,
    presets,
    presetKey,
    onSelectPreset,
  } = props;
  const [draft, setDraft] = useState("");
  const [addError, setAddError] = useState<string | null>(null);

  useEffect(() => {
    setAddError(null);
  }, [draft]);

  const addProject = () => {
    const path = draft.trim();
    if (!path) return;
    if (!path.startsWith("/")) {
      setAddError("项目目录要用绝对路径。");
      return;
    }
    onAddProject(path);
    setDraft("");
  };

  const projectThreads = threads.filter((t) => t.projectPath === activeProject);

  return (
    <aside className="w-[248px] shrink-0 flex flex-col gap-3 min-h-0 pr-1">
      <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-0">
        <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
          <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">项目</h2>
          <button
            className="text-[11px] text-primary hover:underline flex items-center gap-1"
            onClick={onNewThread}
            title="在当前项目里开一段新对话"
          >
            <Icon name="plus" className="w-3 h-3" />
            新会话
          </button>
        </div>

        {/* 开场预设：选一个场景即把它的规则带进新对话（不再是逐步点选的向导）。 */}
        <div className="pt-2 flex flex-wrap gap-1">
          {presets.map((p) => (
            <button
              key={p.key}
              onClick={() => onSelectPreset(p.key)}
              className={
                "px-2 py-1 rounded-full text-[10.5px] " +
                (p.key === presetKey
                  ? "bg-primary text-on-primary"
                  : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high")
              }
              title="新会话的开场预设"
            >
              {p.label}
            </button>
          ))}
        </div>

        <div className="flex flex-col gap-1 pt-2 max-h-[168px] overflow-y-auto">
          {projects.length === 0 ? (
            <p className="text-[11px] text-outline leading-relaxed">
              还没有项目。下面加一个工作目录（绝对路径），对话就在那里进行。
            </p>
          ) : (
            projects.map((path) => (
              <button
                key={path}
                onClick={() => onSelectProject(path)}
                className={
                  "text-left px-2 py-1.5 rounded-lg text-[11.5px] truncate " +
                  (path === activeProject
                    ? "bg-primary/10 text-primary font-medium"
                    : "text-on-surface-variant hover:bg-surface-container")
                }
                title={path}
              >
                <span className="block truncate">{path.split("/").filter(Boolean).pop() ?? path}</span>
                <span className="block truncate text-[10px] text-outline">{path}</span>
              </button>
            ))
          )}
        </div>

        <div className="pt-2 mt-1 border-t border-surface-variant/20">
          <input
            className="w-full text-[11px] px-2 py-1 rounded-md border border-surface-variant/30 bg-surface-container-lowest/60"
            placeholder="加一个项目目录（绝对路径）"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") addProject();
            }}
          />
          <div className="flex items-center gap-2 pt-1.5">
            <button
              className="text-[11px] px-2 py-1 rounded-md bg-surface-container hover:bg-surface-container-high"
              onClick={addProject}
            >
              加入
            </button>
            {addError ? <span className="text-[10px] text-error">{addError}</span> : null}
          </div>
        </div>
      </section>

      <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-0 flex-1">
        <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
          <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">会话</h2>
          <span className="text-[10px] text-outline">{activeProject ? "当前项目" : "未选项目"}</span>
        </div>

        <div className="pt-2 flex flex-col gap-2 min-h-0 overflow-y-auto">
          <div className="flex flex-col gap-1">
            <span className="text-[10px] text-outline px-1">工作台对话（可继续）</span>
            {projectThreads.length === 0 ? (
              <p className="text-[11px] text-outline px-1 leading-relaxed">
                这个项目还没有对话。点右上「新会话」直接说事。
              </p>
            ) : (
              projectThreads.map((t) => (
                <button
                  key={t.id}
                  onClick={() => onSelectThread(t.id)}
                  className={
                    "text-left px-2 py-1.5 rounded-lg text-[11.5px] truncate " +
                    (t.id === activeThreadId
                      ? "bg-primary/10 text-primary font-medium"
                      : "text-on-surface-variant hover:bg-surface-container")
                  }
                  title={t.title}
                >
                  {t.title}
                  {t.preset ? (
                    <span className="ml-1 text-[9px] text-outline">· {t.preset}</span>
                  ) : null}
                </button>
              ))
            )}
          </div>

          <div className="flex flex-col gap-1 pt-2 border-t border-surface-variant/20">
            <span className="text-[10px] text-outline px-1">引擎会话（只读）</span>
            {engineSessionsError ? (
              <p className="text-[11px] text-error px-1 leading-relaxed">{engineSessionsError}</p>
            ) : engineSessions.length === 0 ? (
              <p className="text-[11px] text-outline px-1">该项目下还没有引擎会话。</p>
            ) : (
              engineSessions.map((s) => (
                <div key={s.engine_session_id} className="px-2 py-1" title={s.engine_session_id}>
                  <span className="block truncate text-[11px] text-on-surface-variant">
                    {s.workspace_label || s.engine_session_id.slice(0, 12)}
                  </span>
                  <span className="block text-[10px] text-outline">
                    {s.metadata_valid ? "元数据有效" : "元数据不完整"} ·{" "}
                    {s.resume_status === "resumed"
                      ? "已恢复"
                      : s.resume_status === "failed"
                        ? "恢复失败"
                        : "未验证"}
                  </span>
                </div>
              ))
            )}
          </div>
        </div>
      </section>
    </aside>
  );
}

/** 读取某项目的引擎会话（只读元数据）。失败时如实回报，不静默。 */
export async function loadEngineSessions(cwd: string | null): Promise<{
  sessions: SessionSummary[];
  error: string | null;
}> {
  if (!cwd) return { sessions: [], error: null };
  try {
    const listing = await host.listSessions({ cwd });
    const sessions = listing.groups.flatMap((g) => g.sessions ?? []);
    return { sessions, error: null };
  } catch (e) {
    return { sessions: [], error: e instanceof Error ? e.message : String(e) };
  }
}
