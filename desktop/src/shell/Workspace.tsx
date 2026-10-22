import { useCallback, useEffect, useState } from "react";
import { host } from "../api";
import {
  listThreadsByTag,
  requestNewThread,
  requestOpenThread,
  retagThread,
  type SceneThread,
  type ThreadTag,
} from "../sceneThreads";
import { THREAD_CATEGORIES } from "../threadCategories";
import type { SessionGroup } from "../types";
import type { Scene } from "./Dock";
import { ProjectRail } from "./ProjectRail";

/**
 * 历史会话模块（OD-ARCH-033，OD-ARCH-061 改名并清理）。
 *
 * 本页只做检索与定位，不再自带对话框：整个应用只有首页一处对话（OD-ARCH-057）。
 * 选中本机对话会带回首页继续；引擎会话只展示只读元数据与恢复按钮。
 *
 * 两条线分开呈现，不混为一谈：
 *   * **引擎项目 / 引擎会话**：宿主从 DSH 读回的真实分组与会话（只读元数据，界面不拼路径）。
 *   * **本机对话**：工作台自己的对话线程（带归类标签），选中后回首页打开。
 */
export type PresetKey = "plain" | "weekly" | "writing" | "project";

/** 归类标签只有一份（OD-ARCH-061）：与首页胶囊同源，避免两处各写一套文案。 */
const TAG_OPTIONS: Array<{ key: ThreadTag; label: string }> = THREAD_CATEGORIES.map((c) => ({
  key: c.key,
  label: c.label,
}));

export function Workspace({ onNavigate }: { onNavigate: (scene: Scene) => void }) {
  const [groups, setGroups] = useState<SessionGroup[]>([]);
  const [listError, setListError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [activeGroupToken, setActiveGroupToken] = useState<string | null>(null);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeDraftId, setActiveDraftId] = useState<string | null>(null);
  const [newTag, setNewTag] = useState<ThreadTag>("plain");
  const [drafts, setDrafts] = useState<SceneThread[]>([]);
  const [recovered, setRecovered] = useState<string | null>(null);

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

  const activeThread = activeDraftId ? drafts.find((t) => t.id === activeDraftId) ?? null : null;

  /** 新对话：交给首页开（唯一对话框）。 */
  const newDraft = useCallback(() => {
    requestNewThread(newTag);
    onNavigate("home");
  }, [newTag, onNavigate]);

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
              {activeSessionId ? "引擎会话" : activeThread ? "本机对话" : "历史会话"}
            </h1>
            <p className="text-[11px] text-outline truncate">
              {activeSessionId
                ? activeSessionId
                : activeThread
                  ? "这段对话在首页继续；归类标签决定它归到哪个场景。"
                  : "从左侧选一段对话，或点「新会话」回首页开一段新的。"}
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

        <div className="flex-1 min-h-0 overflow-y-auto rounded-2xl bg-white/80 border border-white/90 p-4">
          {activeThread ? (
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2 flex-wrap">
                <span className="text-[11px] text-outline">归类</span>
                <span className="px-2 py-0.5 rounded-full bg-primary/10 text-primary font-label-caps text-[10px]">
                  {TAG_OPTIONS.find((o) => o.key === activeThread.tag)?.label ?? activeThread.tag}
                </span>
              </div>
              <p className="text-[11.5px] text-on-surface-variant leading-relaxed">
                这段对话共有 {activeThread.messages.length} 条消息，保存在本机。
                点下面的按钮回首页接着聊。
              </p>
              <button
                className="self-start px-3 py-1.5 rounded-full bg-primary text-on-primary font-label-caps text-[11px] hover:bg-primary-container"
                onClick={() => {
                  requestOpenThread(activeThread.id);
                  onNavigate("home");
                }}
              >
                去首页继续
              </button>
            </div>
          ) : activeSessionId ? (
            <p className="text-[11.5px] text-on-surface-variant leading-relaxed">
              这是引擎里的会话，只显示只读元数据。本页不能直接对话。
              需要继续工作时，回首页开一段对话即可。
            </p>
          ) : (
            <p className="text-[11.5px] text-on-surface-variant leading-relaxed">
              从左侧选一段对话；这里不会显示对话内容，也不会把内容复制到别处。
            </p>
          )}
        </div>
      </section>
    </main>
  );
}
