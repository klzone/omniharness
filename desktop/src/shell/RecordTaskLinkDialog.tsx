import { useEffect, useMemo, useState } from "react";

import { shell } from "../api";
import { Icon } from "../components/Icon";
import type { WorkRecord, WorkTask } from "../types";

const ACTIVE_STATES = new Set(["待确认", "待开始", "进行中", "暂停"]);

function errText(value: unknown): string {
  return value instanceof Error ? value.message : String(value);
}

/**
 * 把一条工作记录关联到已有任务（OD-WR-509）。
 *
 * 只写 linked_task_ids，沿用宿主既有补丁与修订历史；不会改动任务状态，
 * 也不会新建任务。任务候选仍走审核浮层。
 */
export function RecordTaskLinkDialog({
  record,
  onClose,
  onSaved,
}: {
  record: WorkRecord;
  onClose: () => void;
  onSaved: (record: WorkRecord) => void;
}) {
  const [tasks, setTasks] = useState<WorkTask[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [ids, setIds] = useState<string[]>(record.linked_task_ids);
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    shell
      .taskList({})
      .then((listing) => {
        if (!alive) return;
        setTasks(listing.items);
        setLoadError(null);
      })
      .catch((e) => {
        if (!alive) return;
        setLoadError(errText(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, []);

  const byId = useMemo(() => {
    const map = new Map<string, WorkTask>();
    for (const task of tasks) map.set(task.id, task);
    return map;
  }, [tasks]);

  const ordered = useMemo(() => {
    const keyword = search.trim().toLowerCase();
    const matched = tasks.filter((task) => {
      if (!keyword) return true;
      return (
        task.title.toLowerCase().includes(keyword) ||
        task.id.toLowerCase().includes(keyword) ||
        (task.category ?? "").toLowerCase().includes(keyword)
      );
    });
    return matched.sort((a, b) => {
      const activeA = ACTIVE_STATES.has(a.state) ? 0 : 1;
      const activeB = ACTIVE_STATES.has(b.state) ? 0 : 1;
      if (activeA !== activeB) return activeA - activeB;
      return a.title.localeCompare(b.title, "zh-Hans-CN");
    });
  }, [tasks, search]);

  const missingIds = ids.filter((id) => !byId.has(id));

  function toggle(id: string) {
    setIds((prev) => (prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]));
    setError(null);
  }

  async function save() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await shell.workRecordUpdate(record.id, { linked_task_ids: ids });
      onSaved(saved.record);
    } catch (e) {
      setError(errText(e));
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center p-4" data-task-link-dialog="">
      <button
        type="button"
        aria-label="关闭关联任务"
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-[rgba(24,26,40,0.26)]"
      />
      <section
        role="dialog"
        aria-label="关联任务"
        className="wr-dialog relative flex max-h-[82vh] w-[min(660px,94vw)] flex-col overflow-hidden"
      >
        <header className="wr-dialog-head">
          <span className="wr-dialog-mark" aria-hidden="true">
            <Icon name="branch" className="h-4 w-4" />
          </span>
          <div className="min-w-0 flex-1">
            <h2 className="text-[16px] font-semibold tracking-tight text-on-surface">关联任务</h2>
            <p className="mt-0.5 truncate text-[10.5px] text-outline" title={record.title}>
              {record.title}
            </p>
          </div>
          <button type="button" onClick={onClose} className="wr-icon-button" aria-label="关闭">
            <Icon name="x" className="h-4 w-4" />
          </button>
        </header>

        <div className="px-4 pt-3">
          <div className="wr-search w-full">
            <Icon name="search" className="h-3.5 w-3.5 text-outline" />
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="搜索任务标题、编号或分类"
              aria-label="搜索任务"
              className="w-full min-w-0 bg-transparent text-[11.5px] text-on-surface placeholder:text-outline/70 focus:outline-none"
            />
          </div>
          <div className="mt-2 flex items-center gap-2 text-[10.5px] text-outline">
            <span className="mono">{ids.length} 个已关联</span>
            <button
              type="button"
              onClick={() => setIds([])}
              className="ml-auto text-[10.5px] text-outline hover:text-primary"
              disabled={ids.length === 0}
            >
              全部解除
            </button>
          </div>
        </div>

        <div className="mt-2 flex-1 min-h-0 overflow-y-auto px-4 pb-3">
          {loadError ? (
            <p className="py-4 text-[11.5px] leading-relaxed text-error">
              读取任务失败：{loadError}
            </p>
          ) : loading ? (
            <p className="py-4 text-[11.5px] text-outline">正在读取任务…</p>
          ) : ordered.length === 0 && missingIds.length === 0 ? (
            <p className="py-5 text-[11.5px] text-outline">
              {search.trim() ? "没有匹配的任务。" : "还没有任务。任务候选经审核采纳后会出现在这里。"}
            </p>
          ) : (
            <div className="flex flex-col gap-1">
              {missingIds.map((id) => (
                <label key={id} className="wr-task-row is-missing">
                  <input
                    type="checkbox"
                    checked
                    onChange={() => toggle(id)}
                    className="h-3.5 w-3.5 shrink-0 accent-[color:var(--color-primary)]"
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block text-[11.5px] font-medium text-on-surface">
                      任务已不存在
                    </span>
                    <span className="mono block text-[10px] text-outline">{id}</span>
                  </span>
                  <span className="wr-task-state is-missing">缺失</span>
                </label>
              ))}
              {ordered.map((task) => {
                const checked = ids.includes(task.id);
                return (
                  <label
                    key={task.id}
                    className={"wr-task-row" + (checked ? " is-checked" : "")}
                    data-task-link-row={task.id}
                  >
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggle(task.id)}
                      className="h-3.5 w-3.5 shrink-0 accent-[color:var(--color-primary)]"
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[11.5px] font-medium text-on-surface">
                        {task.title}
                      </span>
                      <span className="mt-0.5 flex flex-wrap items-center gap-x-2 text-[10px] text-outline">
                        <span className="mono">#{task.id}</span>
                        {task.category ? <span>{task.category}</span> : null}
                        {task.due_date ? <span>截止 {task.due_date}</span> : null}
                      </span>
                    </span>
                    <span className="wr-task-state">{task.state}</span>
                  </label>
                );
              })}
            </div>
          )}
        </div>

        {error ? (
          <p className="px-4 pb-2 text-[11px] leading-relaxed text-error">{error}</p>
        ) : null}

        <footer className="wr-dialog-foot">
          <span className="text-[10.5px] text-outline">只更新记录的关联任务，不改任务状态。</span>
          <button type="button" onClick={onClose} className="wr-ghost-button">
            取消
          </button>
          <button
            type="button"
            onClick={() => void save()}
            disabled={busy}
            className="wr-primary-button"
          >
            {busy ? "保存中…" : "保存关联"}
          </button>
        </footer>
      </section>
    </div>
  );
}
