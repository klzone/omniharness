import { Icon } from "../components/Icon";
import type { SessionGroup } from "../types";
import type { SceneThread, ThreadTag } from "../sceneThreads";

/**
 * 项目 / 会话栏（OD-ARCH-033 模块页的左侧栏）。
 *
 * 列的**就是** DSH 原本那套「项目 / 会话」：
 *   * 「引擎项目」= 引擎按工作区分组的组（宿主只给不透明标识与 token，界面不拼路径）；
 *   * 「引擎会话」= 该组下的 DSH 真实会话（只读元数据）。
 * 另有一栏「本机对话」：工作台自己的对话线程（带场景标签），打标后即归入对应场景页。
 *
 * 界面**不自造**项目：不能凭空加一个目录当项目，也不能把路径当项目键。
 */
export interface ProjectRailProps {
  groups: SessionGroup[];
  activeGroupToken: string | null;
  activeSessionId: string | null;
  drafts: SceneThread[];
  activeDraftId: string | null;
  tagOptions: Array<{ key: ThreadTag; label: string }>;
  newTag: ThreadTag;
  loading: boolean;
  error: string | null;
  onSelectGroup: (token: string) => void;
  onSelectSession: (id: string) => void;
  onSelectDraft: (id: string) => void;
  onNewThread: () => void;
  onSelectTag: (tag: ThreadTag) => void;
  onRetag: (id: string, tag: ThreadTag) => void;
  onRefresh: () => void;
}

export function ProjectRail(props: ProjectRailProps) {
  const {
    groups,
    activeGroupToken,
    activeSessionId,
    drafts,
    activeDraftId,
    tagOptions,
    newTag,
    loading,
    error,
    onSelectGroup,
    onSelectSession,
    onSelectDraft,
    onNewThread,
    onSelectTag,
    onRetag,
    onRefresh,
  } = props;

  return (
    <aside className="w-[268px] shrink-0 flex flex-col min-h-0">
      <section className="bg-white/80 border border-white/90 rounded-2xl shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] p-3 flex flex-col min-h-0">
        <div className="flex items-center justify-between pb-2 border-b border-surface-variant/20">
          <h2 className="font-headline-md text-[12px] font-semibold text-on-surface">项目与会话</h2>
          <button
            className="text-[11px] text-primary hover:underline"
            onClick={onRefresh}
            title="重新读取引擎的会话清单"
          >
            {loading ? "读取中…" : "刷新"}
          </button>
        </div>

        {error ? <p className="text-[11px] text-error pt-2 leading-relaxed">{error}</p> : null}

        <div className="pt-2 flex flex-col gap-1 min-h-0 overflow-y-auto flex-1">
          <span className="text-[10px] text-outline px-1">引擎项目</span>
          {groups.length === 0 ? (
            <p className="text-[11px] text-outline leading-relaxed">
              引擎还没有任何工作区会话。先开一段新对话，它产生的会话会归到对应项目下。
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
          )}

          <div className="flex flex-col gap-1 pt-2 border-t border-surface-variant/20">
            <div className="flex items-center justify-between pt-1">
              <span className="text-[10px] text-outline px-1">本机对话（未归类）</span>
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
                没有未归类的本机对话。给新会话选一个场景标签，它就归到那个场景页的左栏。
              </p>
            ) : (
              drafts.map((d) => (
                <div key={d.id} className="flex items-center gap-1">
                  <button
                    onClick={() => onSelectDraft(d.id)}
                    className={
                      "flex-1 text-left px-2 py-1.5 rounded-lg text-[11.5px] truncate " +
                      (d.id === activeDraftId
                        ? "bg-primary/10 text-primary font-medium"
                        : "text-on-surface-variant hover:bg-surface-container")
                    }
                    title={d.title}
                  >
                    {d.title}
                  </button>
                  <select
                    className="text-[10px] px-1 py-0.5 rounded border border-surface-variant/30 bg-surface-container-lowest/60"
                    value={d.tag}
                    onChange={(e) => onRetag(d.id, e.target.value as ThreadTag)}
                    title="改挂到别的场景"
                  >
                    {tagOptions.map((o) => (
                      <option key={o.key} value={o.key}>
                        {o.label}
                      </option>
                    ))}
                  </select>
                </div>
              ))
            )}
          </div>
        </div>

        {/* 新会话的场景标签（入口即标签，事后可改） */}
        <div className="pt-2 mt-1 border-t border-surface-variant/20">
          <span className="text-[10px] text-outline">新会话归入</span>
          <div className="pt-1 flex flex-wrap gap-1">
            {tagOptions.map((o) => (
              <button
                key={o.key}
                onClick={() => onSelectTag(o.key)}
                className={
                  "px-2 py-1 rounded-full text-[10.5px] " +
                  (o.key === newTag
                    ? "bg-primary text-on-primary"
                    : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high")
                }
                title="新会话的场景标签"
              >
                {o.label}
              </button>
            ))}
          </div>
        </div>
      </section>
    </aside>
  );
}
