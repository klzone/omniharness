import { useState } from "react";
import { Icon } from "../components/Icon";
import type { SessionGroup } from "../types";

/**
 * 项目 / 会话栏（OD-ARCH-031 Part B 的**模块页**内部左侧栏）。
 *
 * 这里列的**就是 DSH 原本的项目与会话**：
 *   * 「项目」= 引擎按工作区分组的组（宿主只给不透明标识与 token，界面不拼路径、不显示原始目录）；
 *   * 「会话」= 该组下的 DSH 真实会话（只读元数据）。
 *
 * 界面**不自造**项目：不能凭空加一个目录当项目，也不能把路径当项目键——
 * 工作区是引擎的事实，界面只按宿主回报的分组呈现。
 * 「新会话」开的是一段新的工作台对话；它第一轮提交后由引擎产生一个 DSH 会话，
 * 届时关联回这里（不谎称是既有会话）。
 */

export interface WorkThread {
  id: string;
  title: string;
  preset: string | null;
  updatedAt: number;
}

export interface ProjectRailProps {
  groups: SessionGroup[];
  activeGroupToken: string | null;
  activeSessionId: string | null;
  /** 正在进行的**工作台对话**（尚未由引擎产生 DSH 会话的那些）。 */
  drafts: WorkThread[];
  activeDraftId: string | null;
  presets: Array<{ key: string; label: string }>;
  presetKey: string;
  loading: boolean;
  error: string | null;
  onSelectGroup: (token: string) => void;
  onSelectSession: (id: string) => void;
  onSelectDraft: (id: string) => void;
  onNewThread: () => void;
  onSelectPreset: (key: string) => void;
  onRefresh: () => void;
}

export function ProjectRail(props: ProjectRailProps) {
  const {
    groups,
    activeGroupToken,
    activeSessionId,
    drafts,
    activeDraftId,
    presets,
    presetKey,
    loading,
    error,
    onSelectGroup,
    onSelectSession,
    onSelectDraft,
    onNewThread,
    onSelectPreset,
    onRefresh,
  } = props;
  const [tab, setTab] = useState<"projects" | "drafts">("projects");

  return (
    <aside className="w-[260px] shrink-0 flex flex-col gap-3 min-h-0">
      <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-0">
        <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
          <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">项目与会话</h2>
          <button
            className="text-[11px] text-primary hover:underline flex items-center gap-1"
            onClick={onRefresh}
            title="重新读取引擎的会话清单"
          >
            {loading ? "读取中…" : "刷新"}
          </button>
        </div>

        {/* 两个分页：引擎的项目/会话，以及本机尚未落成引擎会话的对话草稿。 */}
        <div className="pt-2 flex gap-1">
          <button
            className={
              "px-2 py-1 rounded-full text-[10.5px] " +
              (tab === "projects"
                ? "bg-primary text-on-primary"
                : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high")
            }
            onClick={() => setTab("projects")}
          >
            引擎项目
          </button>
          <button
            className={
              "px-2 py-1 rounded-full text-[10.5px] " +
              (tab === "drafts"
                ? "bg-primary text-on-primary"
                : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high")
            }
            onClick={() => setTab("drafts")}
            title="本机尚未产生引擎会话的对话"
          >
            本机对话
          </button>
        </div>

        {error ? <p className="text-[11px] text-error pt-2 leading-relaxed">{error}</p> : null}

        <div className="flex flex-col gap-1 pt-2 min-h-0 overflow-y-auto flex-1">
          {tab === "projects" ? (
            groups.length === 0 ? (
              <p className="text-[11px] text-outline leading-relaxed">
                引擎还没有任何工作区会话。先在下面开一段新对话，它产生的会话会归到对应项目下。
              </p>
            ) : (
              groups.map((g) => (
                <div key={g.workspace_token} className="flex flex-col">
                  <button
                    onClick={() => onSelectGroup(g.workspace_token)}
                    className={
                      "text-left px-2 py-1.5 rounded-lg text-[11.5px] truncate " +
                      (g.workspace_token === activeGroupToken
                        ? "bg-primary/10 text-primary font-medium"
                        : "text-on-surface-variant hover:bg-surface-container")
                    }
                    title={g.workspace_label || g.workspace_token}
                  >
                    {g.workspace_label || "（未命名工作区）"}
                  </button>
                  {g.workspace_token === activeGroupToken
                    ? (g.sessions ?? []).map((s) => (
                        <button
                          key={s.engine_session_id}
                          onClick={() => onSelectSession(s.engine_session_id)}
                          className={
                            "text-left pl-4 pr-2 py-1 rounded-lg text-[11px] truncate " +
                            (s.engine_session_id === activeSessionId
                              ? "bg-primary/10 text-primary"
                              : "text-outline hover:bg-surface-container")
                          }
                          title={s.engine_session_id}
                        >
                          {s.engine_session_id.slice(0, 12)}
                          {s.resume_status === "resumed"
                            ? " · 已恢复"
                            : s.resume_status === "failed"
                              ? " · 恢复失败"
                              : " · 恢复未验证"}
                        </button>
                      ))
                    : null}
                </div>
              ))
            )
          ) : (
            <>
              <div className="flex items-center justify-between pt-1">
                <span className="text-[10px] text-outline px-1">本机对话草稿</span>
                <button
                  className="text-[11px] text-primary hover:underline flex items-center gap-1"
                  onClick={onNewThread}
                  title="开一段新对话"
                >
                  <Icon name="plus" className="w-3 h-3" />
                  新会话
                </button>
              </div>
              {drafts.length === 0 ? (
                <p className="text-[11px] text-outline px-1 leading-relaxed">
                  还没有本机对话草稿。点「新会话」开一段，第一轮提交后它会成为引擎会话并移到「引擎项目」里。
                </p>
              ) : (
                drafts.map((d) => (
                  <button
                    key={d.id}
                    onClick={() => onSelectDraft(d.id)}
                    className={
                      "text-left px-2 py-1.5 rounded-lg text-[11.5px] truncate " +
                      (d.id === activeDraftId
                        ? "bg-primary/10 text-primary font-medium"
                        : "text-on-surface-variant hover:bg-surface-container")
                    }
                    title={d.title}
                  >
                    {d.title}
                    {d.preset ? <span className="ml-1 text-[9px] text-outline">· {d.preset}</span> : null}
                  </button>
                ))
              )}
            </>
          )}
        </div>

        {/* 开场预设：选一个场景即把它的规则带进新对话（不再是逐步点选的向导）。 */}
        <div className="pt-2 mt-1 border-t border-surface-variant/20 flex flex-wrap gap-1">
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
      </section>
    </aside>
  );
}
