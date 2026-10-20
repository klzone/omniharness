import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { shell } from "../api";
import type { OfflineFlowPack } from "../types";
import { HomeCanvas } from "./HomeCanvas";
import type { ChatMessage } from "./HomeChat";
import type { Scene } from "./Dock";
import { ProjectRail, loadEngineSessions, type WorkThread } from "./ProjectRail";

/**
 * 工作区（OD-ARCH-031 Part B）：项目 → 会话 → 对话流。
 *
 * 结构：左侧「项目 / 会话」栏，右侧沿用用户已确认的首页版式（顶部三模块 + 全宽对话）。
 * 三场景不再是「表单 + 逐步点选」的向导页，而是**开场预设**：选一个预设即把该场景的
 * Skill 规则、期望章节与材料位置约定带进对话，之后完全交给对话流。
 *
 * 诚实性约束：
 *   * 工作台对话的**每一轮**都是一次独立任务（宿主按 cwd 新建 DSH 会话）；
 *     连续性由本工作台把上文重放进下一轮提示实现，**不是**同一个 DSH 会话里的多轮。
 *   * 工作台对话保存在**本地存储**（只在本机、只在这份前端里），不进 Git、不进成果目录。
 *   * 「引擎会话」是 DSH 真实会话的只读元数据；能否恢复由宿主判定，界面不猜。
 */

const PROJECTS_KEY = "omnidesk.workspace.projects.v1";
const THREADS_KEY = "omnidesk.workspace.threads.v1";
const MESSAGES_KEY = "omnidesk.workspace.messages.v1";

export type PresetKey = "plain" | "weekly" | "writing" | "project";

const PRESETS: Array<{ key: PresetKey; label: string; packKey: "weekly" | "writing" | "project" | null }> = [
  { key: "plain", label: "普通对话", packKey: null },
  { key: "weekly", label: "周报与随记", packKey: "weekly" },
  { key: "writing", label: "文案写作", packKey: "writing" },
  { key: "project", label: "项目跟踪", packKey: "project" },
];

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = window.localStorage.getItem(key);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown) {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* 存储不可用时只是不持久化，不影响本次使用 */
  }
}

export function Workspace({ onNavigate }: { onNavigate: (scene: Scene) => void }) {
  const [projects, setProjects] = useState<string[]>(() => readJson<string[]>(PROJECTS_KEY, []));
  const [threads, setThreads] = useState<WorkThread[]>(() => readJson<WorkThread[]>(THREADS_KEY, []));
  const [activeProject, setActiveProject] = useState<string | null>(null);
  const [activeThreadId, setActiveThreadId] = useState<string | null>(null);
  const [presetKey, setPresetKey] = useState<PresetKey>("plain");
  const [pack, setPack] = useState<OfflineFlowPack | null>(null);
  const [engineSessions, setEngineSessions] = useState<ReturnType<typeof loadEngineSessions> extends Promise<infer T> ? T : never>(
    { sessions: [], error: null } as never,
  );
  const [messagesByThread, setMessagesByThread] = useState<Record<string, unknown[]>>(() =>
    readJson<Record<string, unknown[]>>(MESSAGES_KEY, {}),
  );

  // 场景预设的规则来自可编辑的流程包（缺省时由 buildDraftPrompt 回落内置默认）。
  useEffect(() => {
    shell
      .offlineFlowPack()
      .then((r) => setPack(r.ok ? r.pack : null))
      .catch(() => setPack(null));
  }, []);

  // 选中的项目决定引擎会话列表（只读）。
  useEffect(() => {
    let alive = true;
    void loadEngineSessions(activeProject).then((r) => {
      if (alive) setEngineSessions(r);
    });
    return () => {
      alive = false;
    };
  }, [activeProject]);

  useEffect(() => writeJson(PROJECTS_KEY, projects), [projects]);
  useEffect(() => writeJson(THREADS_KEY, threads), [threads]);
  useEffect(() => writeJson(MESSAGES_KEY, messagesByThread), [messagesByThread]);

  const preset = useMemo(() => {
    const def = PRESETS.find((p) => p.key === presetKey) ?? PRESETS[0];
    const flow = def.packKey && pack ? pack[def.packKey] : null;
    return {
      key: def.key,
      label: def.label,
      rules: flow?.draft?.rules ?? null,
      sections: flow?.draft?.sections ?? null,
    };
  }, [presetKey, pack]);

  const newThread = useCallback(() => {
    const id = "t" + Date.now().toString(36);
    const project = activeProject ?? projects[0] ?? "";
    setThreads((prev) => [
      { id, projectPath: project, title: "新对话", preset: presetKey, updatedAt: Date.now() },
      ...prev,
    ]);
    setActiveThreadId(id);
    setMessagesByThread((prev) => ({ ...prev, [id]: [] }));
  }, [activeProject, projects, presetKey]);

  const selectThread = useCallback((id: string) => {
    setActiveThreadId(id);
    const t = threads.find((x) => x.id === id);
    if (t) setPresetKey((t.preset as PresetKey) ?? "plain");
  }, [threads]);

  const addProject = useCallback((path: string) => {
    setProjects((prev) => (prev.includes(path) ? prev : [...prev, path]));
    setActiveProject(path);
  }, []);

  const handleMessages = useCallback(
    (id: string, messages: unknown[]) => {
      setMessagesByThread((prev) => ({ ...prev, [id]: messages }));
      setThreads((prev) =>
        prev.map((t) => {
          if (t.id !== id) return t;
          const firstUser = (messages as Array<{ role?: string; text?: string }>).find(
            (m) => m.role === "user" && m.text,
          );
          return {
            ...t,
            updatedAt: Date.now(),
            title: firstUser ? firstUser.text!.slice(0, 18) : t.title,
          };
        }),
      );
    },
    [],
  );

  const rail: ReactNode = (
    <ProjectRail
      projects={projects}
      activeProject={activeProject}
      threads={threads}
      activeThreadId={activeThreadId}
      engineSessions={engineSessions.sessions}
      engineSessionsError={engineSessions.error}
      onSelectProject={setActiveProject}
      onSelectThread={selectThread}
      onNewThread={newThread}
      onAddProject={addProject}
      presets={PRESETS.map((p) => ({ key: p.key, label: p.label }))}
      presetKey={presetKey}
      onSelectPreset={(key) => setPresetKey(key as PresetKey)}
    />
  );

  return (
    <HomeCanvas
      onNavigate={onNavigate}
      rail={rail}
      chat={{
        preset,
        projectDir: activeProject,
        threadKey: activeThreadId ?? "new",
        initialMessages: (activeThreadId
          ? (messagesByThread[activeThreadId] ?? [])
          : []) as ChatMessage[],
        onMessagesChange: (messages) => {
          if (activeThreadId) handleMessages(activeThreadId, messages);
        },
      }}
    />
  );
}
