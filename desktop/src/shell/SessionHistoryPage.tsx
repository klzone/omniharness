import { useCallback, useEffect, useMemo, useState } from "react";

import { Icon } from "../components/Icon";
import {
  groupSessionThreads,
  sessionCategoryLabel,
  threadPreview,
  threadUpdatedLabel,
  type SessionThreadGroup,
} from "../sessionHistory";
import {
  listThreads,
  onThreadStoreChange,
  removeThread,
  requestOpenThread,
  retagThread,
  threadStorageNote,
  type SceneThread,
} from "../sceneThreads";
import { useThreadCategories } from "../threadCategories";
import type { Scene } from "./Dock";
import { ConfirmDialog } from "./ConfirmDialog";

type Notice = { kind: "ok" | "err"; text: string };

const CATEGORY_TONES = ["tone-blue", "tone-violet", "tone-amber", "tone-green"] as const;

/**
 * 分类色只做轻量识别：内置分类固定，未来新增分类按键名稳定散列。
 * 不新增色板；取工作记录页已经使用的四组柔和色。
 */
function categoryToneClass(key: string): string {
  if (key === "plain") return "tone-neutral";
  const builtin = ["weekly", "writing", "project"].indexOf(key);
  if (builtin >= 0) return CATEGORY_TONES[builtin];
  let hash = 0;
  for (let index = 0; index < key.length; index += 1) {
    hash = (hash * 31 + key.charCodeAt(index)) % 997;
  }
  return CATEGORY_TONES[hash % CATEGORY_TONES.length];
}

function threadTitle(thread: SceneThread): string {
  return thread.title.trim() || "未命名对话";
}

/**
 * 历史会话页（OD-UI-714）。
 *
 * 入口由 Dock 内置，不受 Profile 场景声明管辖；分类块来自当前 Profile 的分类表，
 * 但未知旧标签也会显示。点击卡片打开右侧操作面板：继续、归档到分类、删除。
 */
export function SessionHistoryPage({ onNavigate }: { onNavigate: (scene: Scene) => void }) {
  const categories = useThreadCategories();
  const [threads, setThreads] = useState<SceneThread[]>(() => listThreads());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const refresh = useCallback(() => setThreads(listThreads()), []);
  useEffect(() => onThreadStoreChange(refresh), [refresh]);

  const groups = useMemo(
    () => groupSessionThreads(threads, categories),
    [threads, categories],
  );
  const selected = selectedId ? threads.find((thread) => thread.id === selectedId) ?? null : null;
  const selectedCategory = selected
    ? sessionCategoryLabel(selected.tag, categories)
    : null;
  const storageNote = threadStorageNote();
  const latestUpdatedAt = threads.reduce(
    (latest, thread) => Math.max(latest, thread.updatedAt),
    0,
  );

  const closeSelected = () => {
    setSelectedId(null);
    setConfirmDelete(false);
  };

  const continueThread = (thread: SceneThread) => {
    requestOpenThread(thread.id);
    onNavigate("home");
  };

  const archiveThread = (
    thread: SceneThread,
    target: Pick<SessionThreadGroup, "key" | "label">,
  ) => {
    if (target.key === thread.tag) return;
    retagThread(thread.id, target.key as SceneThread["tag"], false);
    refresh();
    setNotice({ kind: "ok", text: `已归档到「${target.label}」。` });
    closeSelected();
  };

  const deleteSelected = () => {
    if (!selected || selected.inflightTaskId) return;
    removeThread(selected.id);
    refresh();
    setNotice({ kind: "ok", text: `已删除「${threadTitle(selected)}」。` });
    setConfirmDelete(false);
    setSelectedId(null);
  };

  return (
    <div
      className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-10"
      data-session-history-page=""
    >
      <div className="mx-auto w-full max-w-[1180px]">
        <header className="sh-masthead" data-session-history-hero="">
          <div className="sh-masthead-main">
            <span className="sh-mast-mark" aria-hidden="true">
              <Icon name="history" className="h-6 w-6" />
            </span>
            <div className="min-w-0">
              <h1 className="sh-mast-title">历史会话</h1>
              <p className="sh-mast-meta">
                <span>{groups.length} 个分类</span>
                <span className="sh-mast-dot" aria-hidden="true" />
                <span>{threads.length} 段对话</span>
                {latestUpdatedAt > 0 ? (
                  <>
                    <span className="sh-mast-dot" aria-hidden="true" />
                    <span>最近更新 {threadUpdatedLabel(latestUpdatedAt)}</span>
                  </>
                ) : null}
              </p>
            </div>
          </div>
          <div className="sh-mast-total" aria-label="对话总数">
            <span className="sh-mast-num mono" data-session-count="">
              {threads.length}
            </span>
            <span className="sh-mast-unit">段对话</span>
          </div>
        </header>

        {notice ? (
          <div
            role="status"
            className={"wr-notice " + (notice.kind === "err" ? "is-error" : "")}
          >
            {notice.text}
          </div>
        ) : null}

        {storageNote ? (
          <div role="status" className="wr-notice">
            {storageNote}
          </div>
        ) : null}

        {groups.length === 0 ? (
          <section className="wr-empty-state sh-empty" data-session-empty="">
            <p>还没有对话。回首页开始一段对话，之后会按分类出现在这里。</p>
            <button
              type="button"
              onClick={() => onNavigate("home")}
              className="wr-primary-button mt-3"
            >
              去首页
            </button>
          </section>
        ) : (
          <div className="sh-board" data-session-groups="">
            {groups.map((group) => {
              const open = expanded[group.key] === true;
              const visible = open ? group.threads : group.threads.slice(0, 4);
              const hidden = group.threads.length - visible.length;
              return (
                <section
                  key={group.key}
                  className="sh-category"
                  aria-label={group.label}
                  data-session-category={group.key}
                >
                  <header className="sh-category-head">
                    <span
                      className={"sh-category-mark " + categoryToneClass(group.key)}
                      aria-hidden="true"
                    >
                      <Icon name="folder" className="h-4 w-4" />
                    </span>
                    <div className="sh-category-heading">
                      <h2 className="sh-category-title" data-hover-hint={group.hint}>
                        {group.label}
                      </h2>
                      <span className="sh-category-count">
                        {group.threads.length} 段
                      </span>
                    </div>
                  </header>

                  <div className="sh-entry-list">
                    {visible.map((thread) => (
                      <button
                        key={thread.id}
                        type="button"
                        onClick={() => {
                          setConfirmDelete(false);
                          setSelectedId(thread.id);
                        }}
                        aria-label={`${threadTitle(thread)}，${threadUpdatedLabel(thread.updatedAt)}`}
                        data-session-thread={thread.id}
                        className={"sh-entry" + (selectedId === thread.id ? " is-active" : "")}
                      >
                        <span className="sh-entry-rail" aria-hidden="true">
                          <span className="sh-entry-dot" />
                        </span>
                        <span className="sh-entry-body">
                          <span className="sh-entry-top">
                            <span className="sh-entry-time mono">
                              {threadUpdatedLabel(thread.updatedAt)} · {thread.messages?.length ?? 0} 条
                            </span>
                            <span className="sh-entry-arrow" aria-hidden="true">
                              <Icon name="chevron-down" className="h-3.5 w-3.5" />
                            </span>
                          </span>
                          <span className="sh-entry-title">{threadTitle(thread)}</span>
                          <span className="sh-entry-preview">{threadPreview(thread.messages ?? [])}</span>
                          {thread.inflightTaskId ? (
                            <span className="sh-entry-running">
                              <Icon name="clock" className="h-3 w-3" />
                              执行中
                            </span>
                          ) : null}
                        </span>
                      </button>
                    ))}
                  </div>

                  {hidden > 0 || open ? (
                    <button
                      type="button"
                      className="sh-show-more"
                      aria-expanded={open}
                      onClick={() =>
                        setExpanded((prev) => ({ ...prev, [group.key]: !open }))
                      }
                      data-session-expand={group.key}
                    >
                      {open ? "收起" : `展开其余 ${hidden} 段`}
                      <Icon
                        name={open ? "chevron-up" : "chevron-down"}
                        className="h-3.5 w-3.5"
                      />
                    </button>
                  ) : null}
                </section>
              );
            })}
          </div>
        )}
      </div>

      {selected ? (
        <div className="fixed inset-0 z-50" data-session-actions="">
          <button
            type="button"
            aria-label="关闭对话操作"
            onClick={closeSelected}
            className="absolute inset-0 bg-[rgba(24,26,40,0.24)]"
          />
          <section
            aria-label="对话操作"
            role="dialog"
            aria-modal="true"
            className="wr-drawer sh-drawer absolute right-4 top-[10%] flex h-[80%] w-[min(440px,94vw)] flex-col overflow-y-auto px-5 py-5"
            data-session-detail=""
          >
            <div className="sh-drawer-head">
              <span className="sh-drawer-mark" aria-hidden="true">
                <Icon name="chat" className="h-5 w-5" />
              </span>
              <div className="min-w-0 flex-1">
                <span className="wr-eyebrow">对话记录</span>
                <h2 className="sh-drawer-title">{threadTitle(selected)}</h2>
              </div>
              <button
                type="button"
                onClick={closeSelected}
                aria-label="关闭"
                className="wr-icon-button"
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </div>
            <div className="wr-detail-facts">
              <span className="mono">{threadUpdatedLabel(selected.updatedAt)}</span>
              {selectedCategory ? (
                <span className="wr-category is-neutral">{selectedCategory}</span>
              ) : null}
              <span>{selected.messages?.length ?? 0} 条消息</span>
            </div>

            <button
              type="button"
              onClick={() => continueThread(selected)}
              data-session-continue=""
              className="sh-primary-action mt-4"
            >
              <Icon name="chat" className="h-4 w-4" />
              继续对话
              <Icon name="arrow-right" className="ml-auto h-4 w-4" />
            </button>

            <section aria-label="归档对话" className="sh-section">
              <div className="sh-section-head">
                <Icon name="folder" className="h-3.5 w-3.5" />
                <h3>归档对话</h3>
              </div>
              <div className="sh-archive-grid">
                {categories.map((category) => {
                  const current = category.key === selected.tag;
                  return (
                    <button
                      key={category.key}
                      type="button"
                      disabled={current}
                      aria-current={current ? "true" : undefined}
                      onClick={() => archiveThread(selected, category)}
                      data-hover-hint={category.hint}
                      data-session-archive={category.key}
                      className={"sh-archive-chip" + (current ? " is-current" : "")}
                    >
                      <span>{category.label}</span>
                      {current ? <Icon name="check" className="h-3 w-3" /> : null}
                    </button>
                  );
                })}
              </div>
              {categories.length <= 1 ? (
                <p className="sh-muted">当前 Profile 只有这一个对话分类。</p>
              ) : null}
            </section>

            {(selected.messages?.length ?? 0) > 0 ? (
              <section aria-label="最近消息" className="sh-section">
                <div className="sh-section-head">
                  <Icon name="chat" className="h-3.5 w-3.5" />
                  <h3>最近消息</h3>
                </div>
                <div className="sh-message-list">
                  {(selected.messages ?? [])
                    .filter((message) => (message.text ?? "").trim())
                    .slice(-4)
                    .map((message) => (
                      <div key={message.id} className="sh-message">
                        <span
                          className={
                            "sh-message-role " +
                            (message.role === "user" ? "is-user" : "is-assistant")
                          }
                        >
                          {message.role === "user" ? "你" : "助手"}
                        </span>
                        <p>{threadPreview([message])}</p>
                      </div>
                    ))}
                </div>
              </section>
            ) : null}

            <div className="sh-danger">
              <button
                type="button"
                onClick={() => setConfirmDelete(true)}
                disabled={Boolean(selected.inflightTaskId)}
                data-session-delete=""
                className="sh-danger-button"
              >
                <Icon name="x" className="h-4 w-4" />
                删除对话
              </button>
              {selected.inflightTaskId ? (
                <p className="sh-muted">这段对话还有任务在跑，结束或取消后才能删除。</p>
              ) : null}
            </div>
          </section>
        </div>
      ) : null}

      <ConfirmDialog
        open={confirmDelete && selected !== null}
        title="删除这段对话？"
        description="只删除本机对话记录；引擎侧会话不会在这里改动。删除后无法从应用内恢复。"
        confirmLabel="删除"
        onCancel={() => setConfirmDelete(false)}
        onConfirm={deleteSelected}
        testId="session-delete-confirm"
      />
    </div>
  );
}
