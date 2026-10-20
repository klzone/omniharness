import { useCallback, useEffect, useMemo, useState } from "react";
import { host, shell } from "../api";
import type { OfflineFlowPack, SessionGroup } from "../types";
import { HomeChat, type ChatMessage } from "./HomeChat";
import { ProjectRail, type WorkThread } from "./ProjectRail";
import Sessions from "../Sessions";

/**
 * 项目与会话模块（OD-ARCH-031 Part B）。
 *
 * **这里**才是 DSH 原本那套「项目 / 会话」的落点：**一个模块、一个分页**。
 * 首页不出现项目树、也不做左右分栏——首页仍是用户已确认的对话中枢版式；
 * 左侧 Dock 继续是进入周报 / 撰写材料 / 项目跟进等场景的入口。
 *
 * 模块内的两条线，都如实标注：
 *   * **引擎项目 / 引擎会话**：宿主从 DSH 读回的真实分组与会话（只读元数据，界面不拼路径）。
 *   * **本机对话**：本工作台自己的对话线程；第一轮提交后由引擎产生一个 DSH 会话，
 *     它随即出现在「引擎项目」里 —— 不会把两者混为一谈。
 *
 * 诚实性约束：
 *   * 本机对话的每一轮都是一次**独立任务**（宿主按工作区新建 DSH 会话）；
 *     连续性由工作台把上文重放进下一轮提示实现，不是同一个引擎会话里的多轮。
 *   * 工作台对话的上下文保存在**本地存储**（只在本机、只在这份前端里），不进 Git、不进成果目录。
 */

const DRAFTS_KEY = "omnidesk.workspace.drafts.v1";
const CONTEXT_KEY = "omnidesk.workspace.context.v1";

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
    /* 存储不可用只是不持久化，不影响本次使用 */
  }
}

export function Workspace() {
  const [groups, setGroups] = useState<SessionGroup[]>([]);
  const [listError, setListError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [activeGroupToken, setActiveGroupToken] = useState<string | null>(null);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeDraftId, setActiveDraftId] = useState<string | null>(null);
  const [presetKey, setPresetKey] = useState<PresetKey>("plain");
  const [pack, setPack] = useState<OfflineFlowPack | null>(null);
  const [drafts, setDrafts] = useState<WorkThread[]>(() => readJson<WorkThread[]>(DRAFTS_KEY, []));
  const [contexts, setContexts] = useState<Record<string, ChatMessage[]>>(() =>
    readJson<Record<string, ChatMessage[]>>(CONTEXT_KEY, {}),
  );
  const [recovered, setRecovered] = useState<string | null>(null);

  useEffect(() => {
    shell
      .offlineFlowPack()
      .then((r) => setPack(r.ok ? r.pack : null))
      .catch(() => setPack(null));
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const listing = await host.listSessions();
      setGroups(listing.groups);
      setListError(null);
      setActiveGroupToken((prev) => prev ?? listing.groups[0]?.workspace_token ?? null);
    } catch (e) {
      setListError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => writeJson(DRAFTS_KEY, drafts), [drafts]);
  useEffect(() => writeJson(CONTEXT_KEY, contexts), [contexts]);

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

  /** 正在展示的对话：优先选中的引擎会话，否则选中的本机草稿。 */
  const activeKey = activeSessionId ?? activeDraftId;

  const newDraft = useCallback(() => {
    const id = "d" + Date.now().toString(36);
    setDrafts((prev) => [{ id, title: "新对话", preset: presetKey, updatedAt: Date.now() }, ...prev]);
    setContexts((prev) => ({ ...prev, [id]: [] }));
    setActiveDraftId(id);
    setActiveSessionId(null);
  }, [presetKey]);

  /** 第一轮提交后：引擎新建了一个 DSH 会话 → 把草稿关联到它，并刷新清单。
   *  引擎会话号从**任务台账**读（台账里带 engine_session_id），不改宿主协议。 */
  const linkToEngineSession = useCallback(
    async (omniTaskId: string) => {
      if (!activeDraftId) return;
      let engineSessionId: string | null = null;
      try {
        const ledger = await host.taskRecords(100);
        engineSessionId =
          ledger.records.find((r) => r.omni_task_id === omniTaskId)?.engine_session_id ?? null;
      } catch {
        engineSessionId = null;
      }
      if (!engineSessionId) {
        // 关联不上就**如实留在本机对话里**，不假装它已经是引擎会话。
        void refresh();
        return;
      }
      setDrafts((prev) => prev.filter((d) => d.id !== activeDraftId));
      setContexts((prev) => {
        const next = { ...prev };
        next[engineSessionId] = next[activeDraftId] ?? [];
        delete next[activeDraftId];
        return next;
      });
      setActiveSessionId(engineSessionId);
      setActiveDraftId(null);
      void refresh();
    },
    [activeDraftId, refresh],
  );

  const recover = useCallback(async () => {
    if (!activeSessionId) return;
    const group = groups.find((g) => g.workspace_token === activeGroupToken);
    const token = group?.workspace_token ?? null;
    try {
      const r = await host.recoverSession(activeSessionId, token);
      setRecovered(
        r.resumed
          ? `已恢复该会话（引擎接受了恢复；恢复不触发任何任务重放）。${r.note}`
          : `恢复未成功：${r.note}`,
      );
    } catch (e) {
      setRecovered(`恢复失败：${e instanceof Error ? e.message : String(e)}`);
    }
  }, [activeSessionId, groups, activeGroupToken]);

  const messages = activeKey ? (contexts[activeKey] ?? []) : [];

  return (
    <main className="flex-1 min-w-0 min-h-0 flex gap-4 px-6 sm:px-12 md:pl-28 md:pr-12">
      <ProjectRail
        groups={groups}
        activeGroupToken={activeGroupToken}
        activeSessionId={activeSessionId}
        drafts={drafts}
        activeDraftId={activeDraftId}
        presets={PRESETS.map((p) => ({ key: p.key, label: p.label }))}
        presetKey={presetKey}
        loading={loading}
        error={listError}
        onSelectGroup={setActiveGroupToken}
        onSelectSession={(id) => {
          setActiveSessionId(id);
          setActiveDraftId(null);
        }}
        onSelectDraft={(id) => {
          setActiveDraftId(id);
          setActiveSessionId(null);
        }}
        onNewThread={newDraft}
        onSelectPreset={(k) => setPresetKey(k as PresetKey)}
        onRefresh={() => void refresh()}
      />

      <section className="flex-1 min-w-0 flex flex-col min-h-0">
        <header className="flex items-center justify-between pb-2 pt-1 flex-shrink-0">
          <div className="min-w-0">
            <h1 className="font-display-hero text-xl font-light text-on-surface tracking-tight truncate">
              {activeSessionId
                ? "引擎会话"
                : activeDraftId
                  ? "本机对话"
                  : "项目与会话"}
            </h1>
            <p className="text-[11px] text-outline truncate">
              {activeSessionId
                ? activeSessionId
                : activeDraftId
                  ? "第一轮提交后由引擎产生一个 DSH 会话，并归入对应项目"
                  : "从左侧选一个引擎会话，或点「新会话」开一段对话"}
            </p>
          </div>
          {activeSessionId ? (
            <button
              className="text-[11px] px-2.5 py-1 rounded-md bg-surface-container hover:bg-surface-container-high flex items-center gap-1 shrink-0"
              onClick={() => void recover()}
              title="让引擎重新加载这个会话（不重放任何任务）"
            >
              恢复会话
            </button>
          ) : null}
        </header>

        {recovered ? <p className="text-[11px] text-outline pb-1 shrink-0">{recovered}</p> : null}

        <div className="flex-1 min-h-0 flex flex-col">
          <HomeChat
            key={activeKey ?? "empty"}
            onNavigate={() => {}}            preset={preset}
            projectDir={null}
            threadKey={activeKey ?? "empty"}
            initialMessages={messages}
            onMessagesChange={(next) => {
              if (!activeKey) return;
              setContexts((prev) => ({ ...prev, [activeKey]: next }));
              setDrafts((prev) =>
                prev.map((d) =>
                  d.id === activeKey
                    ? {
                        ...d,
                        updatedAt: Date.now(),
                        title:
                          (next.find((m) => m.role === "user" && m.text)?.text ?? d.title).slice(0, 18),
                      }
                    : d,
                ),
              );
            }}
            onTaskCreated={(id) => void linkToEngineSession(id)}
          />
        </div>

        {/* OmniDesk 自己的任务台账（只读）：与 DSH 的项目/会话不是一回事，分开呈现。 */}
        <details className="mt-1 shrink-0">
          <summary className="text-[11px] text-outline cursor-pointer select-none">
            OmniDesk 任务记录（只读）
          </summary>
          <div className="max-h-64 overflow-y-auto pt-1">
            <Sessions onlyLedger />
          </div>
        </details>
      </section>
    </main>
  );
}
