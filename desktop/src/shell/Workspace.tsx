import { useCallback, useEffect, useMemo, useState } from "react";
import { host, shell } from "../api";
import { SCENES, type SceneKey } from "../scenes";
import {
  createThread,
  listThreads,
  listThreadsByTag,
  retagThread,
  saveThreadMessages,
  setThreadEngineSession,
  type SceneThread,
  type ThreadTag,
} from "../sceneThreads";
import type { OfflineFlowPack, SessionGroup } from "../types";
import { HomeChat, type ChatMessage } from "./HomeChat";
import { ProjectRail } from "./ProjectRail";

/**
 * 项目与会话模块（OD-ARCH-033 目标 3/4）。
 *
 * **这里**是 DSH 原本那套「项目 / 会话」的落点：一个模块、一个分页。
 * 首页不出现项目树、也不做左右分栏——首页仍是用户已确认的对话中枢版式；
 * 左侧 Dock 继续是进入各场景（工作记录 / 文案写作 / 项目跟进…）的入口。
 *
 * 两条线分开呈现，不混为一谈：
 *   * **引擎项目 / 引擎会话**：宿主从 DSH 读回的真实分组与会话（只读元数据，界面不拼路径）。
 *   * **本机对话**：工作台自己的对话线程（带场景标签）；打标后即归入对应场景页。
 *
 * 诚实性约束：
 *   * 本机对话的每一轮都是一次**独立任务**（任务号不同），但**引擎会话是同一个**：
 *     线程记住引擎会话号，下一轮续接同一会话（OD-ARCH-034），上下文由引擎持有，
 *     工作台不再重放上文；只有新建会话或旧线程首次续聊时才走窗口化重放。
 *   * 上下文保存在**本地存储**（只在本机、只在这份前端里），不进 Git、不进成果目录。
 */

export type PresetKey = "plain" | "weekly" | "writing" | "project";

const TAG_OPTIONS: Array<{ key: ThreadTag; label: string }> = [
  { key: "plain", label: "普通对话" },
  { key: "weekly", label: SCENES.weekly.label },
  { key: "writing", label: SCENES.writing.label },
  { key: "project", label: SCENES.project.label },
];

export function Workspace() {
  const [groups, setGroups] = useState<SessionGroup[]>([]);
  const [listError, setListError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [activeGroupToken, setActiveGroupToken] = useState<string | null>(null);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeDraftId, setActiveDraftId] = useState<string | null>(null);
  const [newTag, setNewTag] = useState<ThreadTag>("plain");
  const [pack, setPack] = useState<OfflineFlowPack | null>(null);
  const [drafts, setDrafts] = useState<SceneThread[]>([]);
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

  const refreshDrafts = useCallback(() => {
    setDrafts(listThreadsByTag("plain"));
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);
  useEffect(() => {
    refreshDrafts();
  }, [refreshDrafts]);

  const preset = useMemo(() => {
    if (newTag === "plain") {
      return { key: "plain", label: "普通对话", rules: null, sections: null };
    }
    const key = newTag as SceneKey;
    const flow = pack ? pack[key] : null;
    return {
      key,
      label: SCENES[key].label,
      rules: flow?.draft?.rules ?? null,
      sections: flow?.draft?.sections ?? null,
    };
  }, [newTag, pack]);

  const activeKey = activeSessionId ?? activeDraftId;
  const activeThread = activeDraftId ? listThreads().find((t) => t.id === activeDraftId) ?? null : null;
  const messages: ChatMessage[] = activeThread?.messages ?? [];

  const newDraft = useCallback(() => {
    const t = createThread({
      tag: newTag,
      preset: newTag === "plain" ? "plain" : newTag,
    });
    refreshDrafts();
    setActiveDraftId(t.id);
    setActiveSessionId(null);
  }, [newTag, refreshDrafts]);

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

  return (
    <main className="flex-1 min-w-0 min-h-0 flex gap-4 px-6 sm:px-12 md:pl-28 md:pr-12">
      <ProjectRail
        groups={groups}
        activeGroupToken={activeGroupToken}
        activeSessionId={activeSessionId}
        drafts={drafts}
        activeDraftId={activeDraftId}
        tagOptions={TAG_OPTIONS}
        newTag={newTag}
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
        onSelectTag={setNewTag}
        onRetag={(id, tag) => {
          retagThread(id, tag);
          refreshDrafts();
        }}
        onRefresh={() => void refresh()}
      />

      <section className="flex-1 min-w-0 flex flex-col min-h-0">
        <header className="flex items-center justify-between pb-2 pt-1 flex-shrink-0">
          <div className="min-w-0">
            <h1 className="font-display-hero text-xl font-light text-on-surface tracking-tight truncate">
              {activeSessionId ? "引擎会话" : activeDraftId ? "本机对话" : "项目与会话"}
            </h1>
            <p className="text-[11px] text-outline truncate">
              {activeSessionId
                ? activeSessionId
                : activeDraftId
                  ? "打上场景标签后，这条对话会归入对应场景页的左栏"
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
            onNavigate={() => {}}
            preset={preset}
            projectDir={null}
            threadKey={activeKey ?? "empty"}
            initialMessages={messages}
            engineSessionId={activeThread?.engineSessionId ?? null}
            onEngineSessionId={(sid) => {
              if (!activeDraftId) return;
              setThreadEngineSession(activeDraftId, sid);
              refreshDrafts();
            }}
            onMessagesChange={(next) => {
              if (!activeDraftId) return;
              saveThreadMessages(activeDraftId, next);
              refreshDrafts();
            }}
          />
        </div>
      </section>
    </main>
  );
}
