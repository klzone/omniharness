import { useEffect, useRef, useState } from "react";

import { host, shell } from "../api";
import { Icon, type IconName } from "../components/Icon";
import { requestNewThread } from "../sceneThreads";
import { TERMINAL_STATES } from "../types";
import type {
  DailySummaryRead,
  PeriodReviewPeriod,
  RecordFilter,
  RecordListing,
  RecordPatch,
  RecordBacklinks,
  WorkRecord,
  WorkRecordAiStatus,
  WorkRecordRevision,
  WorkRecordSaved,
  WorkTask,
} from "../types";
import type { Scene } from "./Dock";
import { ConfirmDialog } from "./ConfirmDialog";
import { KnowledgeLinkPanel } from "./KnowledgeLinkPanel";
import { MarkdownText } from "./MarkdownText";
import { RecordTaskLinkDialog } from "./RecordTaskLinkDialog";

type Notice = { kind: "ok" | "err"; text: string };

interface EditForm {
  title: string;
  record_date: string;
  category: string;
  tags: string;
  body: string;
  draft: boolean;
}

interface RecordDateGroup {
  date: string;
  records: WorkRecord[];
}

interface GroupSummaryState {
  read?: DailySummaryRead;
  open: boolean;
  loading: boolean;
  error: string | null;
}

const STATUS_LABEL: Record<WorkRecordAiStatus, string> = {
  unanalyzed: "未解析",
  analyzed: "已解析",
  failed: "解析失败",
};

const STATUS_TONE: Record<WorkRecordAiStatus, string> = {
  unanalyzed: "is-pending",
  analyzed: "is-ok",
  failed: "is-failed",
};

const STATUS_ICON: Record<WorkRecordAiStatus, IconName> = {
  unanalyzed: "clock",
  analyzed: "check",
  failed: "x",
};

/** 分类色只做轻量区分；未列出的自定义分类落回中性色，不写死分类集合。 */
const CATEGORY_TONE: Record<string, string> = {
  工作事项: "is-blue",
  学习记录: "is-violet",
  学习思考: "is-violet",
  工作思考: "is-violet",
  问题与经验: "is-amber",
  个人生活: "is-green",
};

function categoryToneClass(category: string | null): string {
  const label = category?.trim() ?? "";
  return CATEGORY_TONE[label] ?? "is-neutral";
}

/** 建议分类（OD-WR-500 模块 A）：只是快捷填充，输入框仍接受任意自定义分类。 */
const SUGGESTED_CATEGORIES = ["工作事项", "学习记录", "工作思考", "问题与经验", "个人生活"];

const REVIEW_PERIOD_LABEL: Record<PeriodReviewPeriod, string> = {
  weekly: "周",
  monthly: "月",
  quarterly: "季",
  annual: "年",
};

const FIELD_LABEL: Record<string, string> = {
  title: "标题",
  record_date: "日期",
  category: "分类",
  tags: "标签",
  source: "来源",
  destination: "去向",
  ai_analysis_status: "解析状态",
  linked_task_ids: "关联任务",
  draft: "草稿",
  body: "正文",
};

const CARD = "bg-white/80 border border-white/90 rounded-[20px] shadow-[0_16px_40px_-12px_rgba(15,23,42,0.04)]";
const FIELD =
  "rounded-full border border-outline-variant/50 bg-white px-3 py-1.5 text-[11.5px] text-on-surface-variant focus:outline-none focus:ring-1 focus:ring-primary/30";
const FIELD_BLOCK =
  "w-full rounded-xl border border-outline-variant/50 bg-white px-3 py-2 text-[12.5px] text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30";

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function todayLocal(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return String(now.getFullYear()) + "-" + pad(now.getMonth() + 1) + "-" + pad(now.getDate());
}

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function maskNote(saved: WorkRecordSaved): string | null {
  if (!saved.masked) return null;
  const parts: string[] = [];
  if (saved.masked_id_card > 0) parts.push(saved.masked_id_card + " 处身份证号");
  if (saved.masked_phone > 0) parts.push(saved.masked_phone + " 处手机号");
  return parts.length > 0 ? "落盘前已掩码 " + parts.join("、") : "落盘前已做隐私掩码";
}

/** UTC ISO8601 → 本地时间；无法解析时原样显示，不把坏数据改写成「现在」。 */
function revisionTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    date.getFullYear() +
    "-" +
    pad(date.getMonth() + 1) +
    "-" +
    pad(date.getDate()) +
    " " +
    pad(date.getHours()) +
    ":" +
    pad(date.getMinutes())
  );
}

/* ---- OD-WR-504 时间线：纯函数，供列表分组与卡片文案使用 ---- */

/** 按 record_date 分组：日期倒序，组内按当天时间正序（同刻按 id 正序）。 */
export function groupRecordsByDate(items: WorkRecord[]): RecordDateGroup[] {
  const groups = new Map<string, WorkRecord[]>();
  for (const record of items) {
    const bucket = groups.get(record.record_date);
    if (bucket) bucket.push(record);
    else groups.set(record.record_date, [record]);
  }
  const result = Array.from(groups, ([date, records]) => ({ date, records }));
  for (const group of result) {
    group.records.sort((a, b) => {
      if (a.created_at !== b.created_at) return a.created_at < b.created_at ? -1 : 1;
      if (a.id !== b.id) return a.id < b.id ? -1 : 1;
      return 0;
    });
  }
  result.sort((a, b) => (a.date < b.date ? 1 : a.date > b.date ? -1 : 0));
  return result;
}

/** created_at → 本地时分；坏数据返回 null，由卡片只显示 record_date。 */
export function recordTimeLabel(value: string): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  const pad = (n: number) => String(n).padStart(2, "0");
  return pad(date.getHours()) + ":" + pad(date.getMinutes());
}

/** 卡片摘要：body 首行，最多 60 字；空正文给出省略兜底。 */
export function recordPreview(body: string): string {
  const firstLine = (body || "")
    .replace(/\r\n/g, "\n")
    .trim()
    .split("\n")[0]
    .replace(/\s+/g, " ")
    .trim();
  if (!firstLine) return "（无正文）";
  return firstLine.length > 60 ? firstLine.slice(0, 60) + "…" : firstLine;
}

export function recordCategoryLabel(category: string | null): string {
  return category?.trim() || "未分类";
}

export function recordSourceLabel(record: Pick<WorkRecord, "source" | "raw_source">): string | null {
  return record.source?.trim() || record.raw_source?.trim() || null;
}

/** 本地日期 → YYYY-MM-DD。 */
export function isoDateOf(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate());
}

/** 月份筛选 → 该月首尾日期；非法输入返回 null。 */
export function monthRange(month: string): { from: string; to: string } | null {
  const match = /^(\d{4})-(\d{2})$/.exec(month.trim());
  if (!match) return null;
  const year = Number(match[1]);
  const mon = Number(match[2]);
  if (mon < 1 || mon > 12) return null;
  const last = new Date(year, mon, 0).getDate();
  const pad = (n: number) => String(n).padStart(2, "0");
  return { from: match[1] + "-" + match[2] + "-01", to: match[1] + "-" + match[2] + "-" + pad(last) };
}

/** 日期 → 周几（中文单字标签）。 */
export function weekdayOfDate(date: string): string {
  const d = new Date(date + "T00:00:00");
  if (Number.isNaN(d.getTime())) return "";
  return ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"][d.getDay()];
}

/** 日期 → 「11月5日」；用于时间线组头大字。 */
export function dayLabelOf(date: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (!match) return date;
  return Number(match[2]) + "月" + Number(match[3]) + "日";
}

function changedFieldsLabel(fields: string[]): string {
  if (fields.length === 0) return "无字段变更";
  return fields.map((field) => FIELD_LABEL[field] ?? field).join("、");
}

export function WorkRecordLibrary({
  focusRecordId,
  onFocusConsumed,
  onCandidatesReady,
  onOpenAudit,
  onOpenReview,
  onOverview,
  onNavigate,
  refreshToken = 0,
}: {
  focusRecordId: string | null;
  onFocusConsumed: () => void;
  onCandidatesReady: () => void;
  onOpenAudit: () => void;
  onOpenReview: (target: { period: PeriodReviewPeriod; start: string; end: string }) => void;
  onOverview?: (overview: { total: number; month: number }) => void;
  onNavigate: (scene: Scene) => void;
  /** 审核浮层关闭后由页面递增，让列表与统计重新读盘。 */
  refreshToken?: number;
}) {
  const [listing, setListing] = useState<RecordListing>({ items: [], broken: 0 });
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [filter, setFilter] = useState<RecordFilter>({});
  const [keywordDraft, setKeywordDraft] = useState("");
  const [categoryOptions, setCategoryOptions] = useState<string[]>([]);
  const [reloadTick, setReloadTick] = useState(0);
  const [selected, setSelected] = useState<WorkRecord | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [editing, setEditing] = useState(false);
  const [form, setForm] = useState<EditForm | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [quickText, setQuickText] = useState("");
  const [quickDate, setQuickDate] = useState(todayLocal());
  const [quickDraft, setQuickDraft] = useState(false);
  const [busy, setBusy] = useState(false);
  const [parsing, setParsing] = useState(false);
  const [revisions, setRevisions] = useState<WorkRecordRevision[]>([]);
  const [revisionsLoading, setRevisionsLoading] = useState(false);
  const [revisionsError, setRevisionsError] = useState<string | null>(null);
  const [confirmRephrase, setConfirmRephrase] = useState(false);
  const [summary, setSummary] = useState<DailySummaryRead | null>(null);
  const [summaryOpen, setSummaryOpen] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);
  const [groupSummaries, setGroupSummaries] = useState<Record<string, GroupSummaryState>>({});
  const [backlinks, setBacklinks] = useState<RecordBacklinks | null>(null);
  const [backlinksLoading, setBacklinksLoading] = useState(false);
  const [backlinksError, setBacklinksError] = useState<string | null>(null);
  const [composerOpen, setComposerOpen] = useState(false);
  const [month, setMonth] = useState("");
  const [allRecords, setAllRecords] = useState<WorkRecord[]>([]);
  const [timeRangeOpen, setTimeRangeOpen] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const [taskLinkOpen, setTaskLinkOpen] = useState(false);
  const [detailTasks, setDetailTasks] = useState<Record<string, WorkTask>>({});
  const timelineRef = useRef<HTMLElement | null>(null);

  // 关键词检索做 250ms 防抖，避免每敲一个字都触发一次宿主读盘。
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setFilter((prev) => {
        const next: RecordFilter = { ...prev };
        const keyword = keywordDraft.trim();
        if (keyword) next.keyword = keyword;
        else delete next.keyword;
        return next;
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [keywordDraft]);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    shell
      .workRecordList(filter)
      .then((result) => {
        if (!alive) return;
        setListing(result);
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
  }, [filter, reloadTick, refreshToken]);

  // 分类下拉来自实际记录；新分类保存后随 reloadTick 一起刷新。
  useEffect(() => {
    let alive = true;
    shell
      .workRecordList({})
      .then((result) => {
        if (!alive) return;
        const options = Array.from(
          new Set(
            result.items
              .map((item) => item.category)
              .filter((value): value is string => Boolean(value)),
          ),
        );
        options.sort((a, b) => a.localeCompare(b, "zh-Hans-CN"));
        const suggested = SUGGESTED_CATEGORIES.filter((item) => options.includes(item));
        const extras = options.filter((option) => !SUGGESTED_CATEGORIES.includes(option));
        setCategoryOptions([...suggested, ...extras]);
        setAllRecords(result.items);
        const localToday = todayLocal();
        onOverview?.({
          total: result.items.length,
          month: result.items.filter((record) =>
            record.record_date.startsWith(localToday.slice(0, 7)),
          ).length,
        });
      })
      .catch(() => {
        // 分类只是筛选辅助；读不到时不影响列表本身。
      });
    return () => {
      alive = false;
    };
  }, [reloadTick, refreshToken]);

  // 非当天组只按需读取已有摘要；不存在不建文件，也不提供生成入口。
  function patchGroupSummary(date: string, patch: Partial<GroupSummaryState>) {
    setGroupSummaries((prev) => {
      const base: GroupSummaryState = prev[date] ?? {
        open: false,
        loading: false,
        error: null,
      };
      return { ...prev, [date]: { ...base, ...patch } };
    });
  }

  function toggleGroupSummary(date: string) {
    if (date === todayLocal()) {
      setSummaryOpen((open) => !open);
      return;
    }
    const current = groupSummaries[date];
    const opening = !current?.open;
    patchGroupSummary(date, { open: opening });
    if (!opening || current?.read || current?.loading) return;
    patchGroupSummary(date, { loading: true, error: null });
    shell
      .worklogDailySummaryRead(date)
      .then((read) => patchGroupSummary(date, { read, loading: false }))
      .catch((e) => patchGroupSummary(date, { loading: false, error: errText(e) }));
  }

  useEffect(() => {
    if (!focusRecordId) return;
    let alive = true;
    shell
      .workRecordRead(focusRecordId)
      .then((record) => {
        if (!alive) return;
        setSelected(record);
        setRevisions([]);
        setRevisionsError(null);
        setConfirmRephrase(false);
        setEditing(false);
        setConfirmDelete(false);
        setNotice(null);
      })
      .catch((e) => {
        if (!alive) return;
        setNotice({ kind: "err", text: errText(e) });
      })
      .finally(() => {
        if (alive) onFocusConsumed();
      });
    return () => {
      alive = false;
    };
  }, [focusRecordId]);

  // 记录的反向引用随所选记录读取（OD-WR-506）：只读扫描复盘与写作任务。
  useEffect(() => {
    if (!selected) {
      setBacklinks(null);
      setBacklinksError(null);
      setBacklinksLoading(false);
      return;
    }
    const recordId = selected.id;
    let alive = true;
    setBacklinksLoading(true);
    setBacklinksError(null);
    setBacklinks(null);
    shell
      .worklogRecordBacklinks(recordId)
      .then((result) => {
        if (alive) setBacklinks(result);
      })
      .catch((e) => {
        if (alive) setBacklinksError(errText(e));
      })
      .finally(() => {
        if (alive) setBacklinksLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [selected?.id]);

  // 详情里的关联任务标题按需读取；读不到时只显示编号，不阻断详情。
  useEffect(() => {
    if (!selected || selected.linked_task_ids.length === 0) {
      setDetailTasks({});
      return;
    }
    let alive = true;
    shell
      .taskList({})
      .then((listing) => {
        if (!alive) return;
        const map: Record<string, WorkTask> = {};
        for (const task of listing.items) {
          if (selected.linked_task_ids.includes(task.id)) map[task.id] = task;
        }
        setDetailTasks(map);
      })
      .catch(() => {
        if (alive) setDetailTasks({});
      });
    return () => {
      alive = false;
    };
  }, [selected?.id, selected?.linked_task_ids, reloadTick]);

  // 列表刷新后丢弃按需读取的旧摘要缓存，避免展示过期正文。
  useEffect(() => {
    setGroupSummaries({});
  }, [reloadTick]);

  // 当日摘要随列表刷新读取；不存在不建文件，读失败只在本卡内提示。
  useEffect(() => {
    let alive = true;
    shell
      .worklogDailySummaryRead(todayLocal())
      .then((read) => {
        if (!alive) return;
        setSummary(read);
        setSummaryError(null);
        if (!read.exists) setSummaryOpen(false);
      })
      .catch((e) => {
        if (!alive) return;
        setSummary(null);
        setSummaryError(errText(e));
      });
    return () => {
      alive = false;
    };
  }, [reloadTick]);

  // 修订历史随所选记录读取；缺失 revisions.jsonl 的旧记录返回空列表。
  useEffect(() => {
    if (!selected) {
      setRevisions([]);
      setRevisionsError(null);
      return;
    }
    const recordId = selected.id;
    let alive = true;
    setRevisionsLoading(true);
    setRevisionsError(null);
    shell
      .workRecordRevisions(recordId)
      .then((items) => {
        if (!alive) return;
        setRevisions(items);
      })
      .catch((e) => {
        if (!alive) return;
        setRevisions([]);
        setRevisionsError(errText(e));
      })
      .finally(() => {
        if (alive) setRevisionsLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [selected?.id, reloadTick]);

  function selectRecord(record: WorkRecord) {
    setSelected(record);
    setRevisions([]);
    setRevisionsError(null);
    setRevisionsLoading(false);
    setConfirmRephrase(false);
    setEditing(false);
    setForm(null);
    setConfirmDelete(false);
    setNotice(null);
  }

  function patchFilter(patch: RecordFilter) {
    setFilter((prev) => ({ ...prev, ...patch }));
  }

  function closeSelected() {
    setSelected(null);
    setEditing(false);
    setForm(null);
    setConfirmDelete(false);
    setConfirmRephrase(false);
  }

  /** 月份筛选：写入该月首尾日期；清空时一并清掉日期区间。 */
  function applyMonth(value: string) {
    setMonth(value);
    const range = monthRange(value);
    if (!range) {
      setFilter((prev) => {
        const next: RecordFilter = { ...prev };
        delete next.from_date;
        delete next.to_date;
        return next;
      });
      return;
    }
    patchFilter({ from_date: range.from, to_date: range.to });
  }

  function clearRange() {
    setMonth("");
    setFilter((prev) => {
      const next: RecordFilter = { ...prev };
      delete next.from_date;
      delete next.to_date;
      return next;
    });
  }

  async function saveQuick() {
    if (!quickText.trim() || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      const saved = await shell.workRecordCreate({
        body: quickText,
        record_date: quickDate || null,
        draft: quickDraft,
      });
      setSelected(saved.record);
      setQuickText("");
      setQuickDraft(false);
      setEditing(false);
      setConfirmDelete(false);
      setNotice({ kind: "ok", text: maskNote(saved) ?? "已记录。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  function startEdit() {
    if (!selected) return;
    setForm({
      title: selected.title,
      record_date: selected.record_date,
      category: selected.category ?? "",
      tags: selected.tags.join(", "),
      body: selected.body,
      draft: selected.draft,
    });
    setEditing(true);
    setConfirmDelete(false);
    setNotice(null);
  }

  async function saveEdit() {
    if (!selected || !form || busy) return;
    if (!form.title.trim()) {
      setNotice({ kind: "err", text: "标题不能为空。" });
      return;
    }
    setBusy(true);
    setNotice(null);
    const patch: RecordPatch = {
      title: form.title.trim(),
      record_date: form.record_date || selected.record_date,
      category: form.category,
      tags: form.tags
        .split(/[,，]/)
        .map((tag) => tag.trim())
        .filter(Boolean),
      body: form.body,
      draft: form.draft,
    };
    try {
      const saved = await shell.workRecordUpdate(selected.id, patch);
      setSelected(saved.record);
      setEditing(false);
      setForm(null);
      setNotice({ kind: "ok", text: maskNote(saved) ?? "已保存。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function removeSelected() {
    if (!selected || busy) return;
    setBusy(true);
    setNotice(null);
    try {
      await shell.workRecordDelete(selected.id);
      setSelected(null);
      setEditing(false);
      setForm(null);
      setConfirmDelete(false);
      setNotice({ kind: "ok", text: "已移入回收目录。" });
      setReloadTick((n) => n + 1);
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setBusy(false);
    }
  }

  async function parseSelected() {
    if (!selected || parsing) return;
    setParsing(true);
    setNotice({ kind: "ok", text: "正在提交解析任务…" });
    try {
      const prep = await shell.workRecordAnalyzePrepare(selected.id);
      const workspace = await shell.prepareWorkspaceDir();
      if (!workspace.ok || !workspace.workspace_dir) {
        throw new Error(workspace.error || "宿主未返回可用工作目录。");
      }
      const receipt = await host.submitTask({
        promptText: prep.prompt,
        intent: "worklog_extract",
        workspaceDir: workspace.workspace_dir,
        userInputText: prep.user_input_text,
      });
      const taskId = receipt.omni_task_id;
      let status = await host.taskStatus(taskId);
      for (let i = 0; i < 900 && !TERMINAL_STATES.includes(status.status); i += 1) {
        await sleep(1000);
        status = await host.taskStatus(taskId);
        if (i % 10 === 0) {
          setNotice({ kind: "ok", text: "模型正在提取候选…已等待 " + (i + 1) + " 秒" });
        }
      }
      if (status.status !== "succeeded") {
        await shell.workRecordAnalyzeFailed(selected.id).catch(() => undefined);
        throw new Error(
          "解析未完成：" +
            status.status +
            (status.note ? " · " + status.note : ""),
        );
      }
      const result = await host.taskResult(taskId);
      if (!result.content.trim()) {
        await shell.workRecordAnalyzeFailed(selected.id).catch(() => undefined);
        throw new Error("模型没有返回解析结果。");
      }
      const applied = await shell.workRecordAnalyzeApply(selected.id, result.content);
      setSelected(applied.record);
      setReloadTick((n) => n + 1);
      setNotice({
        kind: "ok",
        text:
          applied.staged.length > 0
            ? "已生成 " + applied.staged.length + " 条候选，去「审核」确认。"
            : "没有提取到任务候选。",
      });
      if (applied.staged.length > 0) onCandidatesReady();
    } catch (e) {
      setNotice({ kind: "err", text: errText(e) });
    } finally {
      setParsing(false);
    }
  }

  /** 重新整理：把原始素材预填到首页工作记录对话，只预填不自动发送。 */
  function reorganizeSelected() {
    if (!selected) return;
    const raw = selected.raw_input?.trim();
    if (!raw) return;
    requestNewThread(
      "weekly",
      "请依据以下原始素材重新整理这条记录：\n" + raw,
    );
    setConfirmRephrase(false);
    onNavigate("home");
  }

  /** 生成当日摘要：模型调用在首页对话侧，这里只预填并切过去。 */
  function goGenerateSummary() {
    requestNewThread("weekly", "生成今日摘要");
    onNavigate("home");
  }

  /**
   * 交给 AI 继续分析：只把记录编号与标题预填到首页对话的输入框，不自动发送。
   * 首页不是 ThreadTag，工作记录对应的既有场景键是 weekly。
   * 全局对话采集保持保守不扩：不把对话内容写回记录库，避免任何输入被误记。
   */
  function handSelectedToAi() {
    if (!selected) return;
    requestNewThread(
      "weekly",
      "请继续分析工作记录 #" +
        selected.id +
        "：" +
        selected.title +
        "。原始素材与正式记录见工作记录库，不要修改任何文件，只做分析。",
    );
    onNavigate("home");
  }

  function requestSummary() {
    goGenerateSummary();
  }

  /** 周期复盘入口：只把周期说法预填到首页工作记录对话，不自动发送。 */
  function generateWeeklyReview() {
    requestNewThread("weekly", "周复盘");
    onNavigate("home");
  }

  function generateMonthlyReview() {
    requestNewThread("weekly", "复盘本月");
    onNavigate("home");
  }

  /**
   * 整理选中记录：带上记录编号与已有正文/原始素材，交给首页工作记录对话。
   * 正式记录仍由用户确认后再落盘，这里不直接改写记录文件。
   */
  function organizeSelected() {
    if (!selected) return;
    const material = selected.raw_input?.trim() || selected.body;
    requestNewThread(
      "weekly",
      "请整理工作记录 #" +
        selected.id +
        "「" +
        selected.title +
        "」：\n" +
        material,
    );
    onNavigate("home");
  }

  function openTaskLink() {
    if (!selected) return;
    setTaskLinkOpen(true);
  }

  function scrollToTimeline() {
    timelineRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  /** 概览数字即筛选入口：本月总数 / 已解析 / 待解析。 */
  function focusOverview(status: WorkRecordAiStatus | null) {
    applyMonth(todayLocal().slice(0, 7));
    patchFilter({ ai_analysis_status: status ?? undefined });
    scrollToTimeline();
  }

  async function openSummaryRecord(id: string) {
    try {
      const record = await shell.workRecordRead(id);
      selectRecord(record);
    } catch (e) {
      setNotice({ kind: "err", text: "引用的记录读不到：" + errText(e) });
    }
  }

  const groups = groupRecordsByDate(listing.items);
  const today = todayLocal();
  const currentYear = today.slice(0, 4);
  const monthPrefix = today.slice(0, 7);
  const weekStart = (() => {
    const d = new Date(today + "T00:00:00");
    d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
    return isoDateOf(d);
  })();
  const monthOptions = Array.from(
    new Set(allRecords.map((record) => record.record_date.slice(0, 7)).filter(Boolean)),
  )
    .sort()
    .reverse();
  const stats = {
    total: allRecords.length,
    month: allRecords.filter((record) => record.record_date.startsWith(monthPrefix)).length,
    week: allRecords.filter(
      (record) => record.record_date >= weekStart && record.record_date <= today,
    ).length,
    pending: allRecords.filter(
      (record) => record.ai_analysis_status === "unanalyzed" && !record.draft,
    ).length,
  };
  const monthRecords = allRecords.filter((record) => record.record_date.startsWith(monthPrefix));
  const monthAnalyzed = monthRecords.filter(
    (record) => record.ai_analysis_status === "analyzed",
  ).length;
  const monthPending = monthRecords.filter(
    (record) => record.ai_analysis_status === "unanalyzed",
  ).length;
  const rangeText = filter.from_date || filter.to_date
    ? (filter.from_date ?? "…") + " — " + (filter.to_date ?? "…")
    : "时间范围";
  const extraFilterCount =
    (filter.ai_analysis_status ? 1 : 0) + (filter.draft === true ? 1 : 0);

  return (
    <div className="wr-records" data-work-record-library="">
      {notice ? (
        <div
          role="status"
          className={
            "wr-notice " +
            (notice.kind === "err" ? "is-error" : "")
          }
        >
          {notice.text}
        </div>
      ) : null}

      <div className="wr-records-grid">
        <div className="min-w-0">
          <section aria-label="筛选" className="wr-filterrow">
            <div className="wr-chipgroup" data-filter-chips="">
              <button
                type="button"
                onClick={() => patchFilter({ category: undefined })}
                aria-pressed={!filter.category}
                className={"wr-chip" + (!filter.category ? " is-active" : "")}
              >
                <Icon name="spark" className="h-3 w-3" />
                全部
              </button>
              {categoryOptions.map((option) => (
                <button
                  key={option}
                  type="button"
                  onClick={() => patchFilter({ category: option })}
                  aria-pressed={filter.category === option}
                  data-category-filter={option}
                  className={"wr-chip" + (filter.category === option ? " is-active" : "")}
                >
                  {option}
                </button>
              ))}
            </div>

            <div className="wr-searchgroup">
              <div className="relative">
                <button
                  type="button"
                  onClick={() => {
                    setTimeRangeOpen((open) => !open);
                    setFilterOpen(false);
                  }}
                  aria-expanded={timeRangeOpen}
                  data-time-range-toggle=""
                  className={"wr-pill" + ((filter.from_date || filter.to_date) ? " is-active" : "")}
                >
                  <Icon name="calendar" className="h-3.5 w-3.5" />
                  <span className="wr-pill-text">{rangeText}</span>
                  <Icon name="chevron-down" className="h-3 w-3 opacity-60" />
                </button>
                {timeRangeOpen ? (
                  <>
                    <button
                      type="button"
                      aria-label="关闭时间范围"
                      className="wr-popover-backdrop"
                      onClick={() => setTimeRangeOpen(false)}
                    />
                    <div className="wr-popover wr-popover-right" data-time-popover="">
                      <p className="wr-popover-title">时间范围</p>
                      <label className="wr-field-label">
                        月份
                        <select
                          value={month}
                          onChange={(e) => applyMonth(e.target.value)}
                          aria-label="月份筛选"
                          data-month-filter=""
                          className={FIELD}
                        >
                          <option value="">全部月份</option>
                          {monthOptions.map((option) => (
                            <option key={option} value={option}>
                              {option.slice(0, 4) + " 年 " + Number(option.slice(5, 7)) + " 月"}
                            </option>
                          ))}
                        </select>
                      </label>
                      <div className="mt-2 grid grid-cols-2 gap-2">
                        <label className="wr-field-label">
                          起始
                          <input
                            type="date"
                            value={filter.from_date ?? ""}
                            onChange={(e) => {
                              setMonth("");
                              patchFilter({ from_date: e.target.value || undefined });
                            }}
                            aria-label="起始日期"
                            className={FIELD}
                          />
                        </label>
                        <label className="wr-field-label">
                          结束
                          <input
                            type="date"
                            value={filter.to_date ?? ""}
                            onChange={(e) => {
                              setMonth("");
                              patchFilter({ to_date: e.target.value || undefined });
                            }}
                            aria-label="结束日期"
                            className={FIELD}
                          />
                        </label>
                      </div>
                      <div className="wr-popover-foot">
                        <button
                          type="button"
                          onClick={clearRange}
                          className="wr-ghost-button"
                        >
                          清除
                        </button>
                        <button
                          type="button"
                          onClick={() => setTimeRangeOpen(false)}
                          className="wr-primary-button"
                        >
                          完成
                        </button>
                      </div>
                    </div>
                  </>
                ) : null}
              </div>

              <div className="wr-search">
                <Icon name="search" className="h-3.5 w-3.5 text-outline" />
                <input
                  value={keywordDraft}
                  onChange={(e) => setKeywordDraft(e.target.value)}
                  placeholder="搜索记录…"
                  aria-label="搜索记录"
                  data-record-keyword=""
                  className="w-full min-w-0 bg-transparent text-[11.5px] text-on-surface placeholder:text-outline/70 focus:outline-none"
                />
              </div>

              <div className="relative">
                <button
                  type="button"
                  onClick={() => {
                    setFilterOpen((open) => !open);
                    setTimeRangeOpen(false);
                  }}
                  aria-expanded={filterOpen}
                  aria-label="更多筛选"
                  data-filter-toggle=""
                  className={"wr-icon-button" + (extraFilterCount > 0 ? " has-badge" : "")}
                >
                  <Icon name="settings" className="h-4 w-4" />
                  {extraFilterCount > 0 ? (
                    <span className="wr-badge">{extraFilterCount}</span>
                  ) : null}
                </button>
                {filterOpen ? (
                  <>
                    <button
                      type="button"
                      aria-label="关闭筛选"
                      className="wr-popover-backdrop"
                      onClick={() => setFilterOpen(false)}
                    />
                    <div className="wr-popover wr-popover-right" data-filter-popover="">
                      <p className="wr-popover-title">更多筛选</p>
                      <label className="wr-field-label">
                        解析状态
                        <select
                          value={filter.ai_analysis_status ?? ""}
                          onChange={(e) =>
                            patchFilter({
                              ai_analysis_status:
                                (e.target.value as WorkRecordAiStatus) || undefined,
                            })
                          }
                          aria-label="按解析状态筛选"
                          className={FIELD}
                        >
                          <option value="">全部状态</option>
                          <option value="unanalyzed">未解析</option>
                          <option value="analyzed">已解析</option>
                          <option value="failed">解析失败</option>
                        </select>
                      </label>
                      <label className="wr-check-line">
                        <input
                          type="checkbox"
                          checked={filter.draft === true}
                          onChange={(e) =>
                            patchFilter({ draft: e.target.checked ? true : undefined })
                          }
                          className="h-3.5 w-3.5 accent-[color:var(--color-primary)]"
                        />
                        只看草稿
                      </label>
                      <div className="wr-popover-foot">
                        <button
                          type="button"
                          onClick={() => {
                            patchFilter({
                              ai_analysis_status: undefined,
                              draft: undefined,
                            });
                            setFilterOpen(false);
                          }}
                          className="wr-ghost-button"
                        >
                          清除
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setFilterOpen(false);
                            onOpenAudit();
                          }}
                          data-open-audit=""
                          className="wr-primary-button"
                        >
                          <Icon name="shield-check" className="h-3.5 w-3.5" />
                          审核候选
                        </button>
                      </div>
                    </div>
                  </>
                ) : null}
              </div>

              <button
                type="button"
                onClick={() => setComposerOpen((open) => !open)}
                data-record-compose-toggle=""
                className="wr-text-button"
              >
                {composerOpen ? "收起手动记录" : "手动记一条"}
              </button>
            </div>
          </section>

          {composerOpen ? (
            <section aria-label="快速记录" className={CARD + " wr-composer"}>
              <p className="wr-composer-title">手动记录</p>
              <textarea
                value={quickText}
                onChange={(e) => setQuickText(e.target.value)}
                placeholder="记点什么…"
                className="w-full min-h-[88px] resize-y bg-transparent text-[13px] leading-relaxed text-on-surface placeholder:text-outline/70 focus:outline-none"
                data-work-record-quick=""
              />
              <div className="mt-2 flex flex-wrap items-center gap-3">
                <input
                  type="date"
                  value={quickDate}
                  onChange={(e) => setQuickDate(e.target.value)}
                  aria-label="记录日期"
                  className={FIELD}
                />
                <label className="flex items-center gap-1.5 text-[11.5px] text-on-surface-variant">
                  <input
                    type="checkbox"
                    checked={quickDraft}
                    onChange={(e) => setQuickDraft(e.target.checked)}
                    className="h-3.5 w-3.5 accent-[color:var(--color-primary)]"
                  />
                  草稿
                </label>
                <button
                  onClick={() => void saveQuick()}
                  disabled={busy || !quickText.trim()}
                  className="ml-auto rounded-full bg-primary px-4 py-1.5 text-[11.5px] font-medium text-on-primary disabled:opacity-40"
                >
                  记录
                </button>
              </div>
            </section>
          ) : null}

          <section aria-label="记录列表" className="wr-timeline" data-work-record-timeline="" ref={timelineRef}>
            {loadError ? (
              <p className="wr-load-state text-error">
                读取记录失败：{loadError}
              </p>
            ) : loading ? (
              <p className="wr-load-state">正在读取记录…</p>
            ) : (
              <>
                <div className="wr-timeline-list" data-work-record-list="">
                  {listing.items.length === 0 ? (
                    <p className="wr-empty-state">还没有记录。可以在首页对话里说今天做了什么，让 AI 整理成记录。</p>
                  ) : (
                    groups.map((group) => {
                      const isToday = group.date === today;
                      const state = groupSummaries[group.date];
                      const expanded = isToday ? summaryOpen : state?.open === true;
                      const summaryBody = isToday
                        ? summary?.exists
                          ? summary
                          : null
                        : state?.read?.exists
                          ? state.read
                          : null;
                      return (
                        <div
                          key={group.date}
                          className={"od-record-group" + (isToday ? " is-today" : "")}
                          data-record-group={group.date}
                        >
                          <div className="od-record-group-head">
                            <span className="od-timeline-day">{dayLabelOf(group.date)}</span>
                            <span className="od-timeline-weekday">{weekdayOfDate(group.date)}</span>
                            {group.date.slice(0, 4) !== currentYear ? (
                              <span className="od-timeline-year mono">{group.date.slice(0, 4)}</span>
                            ) : null}
                            <span className="od-timeline-count">
                              <span className="mono" data-record-group-count={group.records.length}>
                                {group.records.length} 条
                              </span>
                              记录
                            </span>
                            <div className="ml-auto flex items-center gap-1">
                              {isToday ? (
                                <>
                                  {summary?.exists ? (
                                    <button
                                      type="button"
                                      onClick={() => toggleGroupSummary(group.date)}
                                      data-daily-summary-view=""
                                      className="wr-summary-action"
                                    >
                                      {summaryOpen ? "收起摘要" : "查看摘要"}
                                    </button>
                                  ) : null}
                                  <button
                                    type="button"
                                    onClick={requestSummary}
                                    data-daily-summary-generate=""
                                    className="wr-summary-action is-primary"
                                  >
                                    生成当日摘要
                                  </button>
                                </>
                              ) : (
                                <button
                                  type="button"
                                  onClick={() => toggleGroupSummary(group.date)}
                                  data-record-group-summary={group.date}
                                  className="wr-summary-action"
                                >
                                  {expanded ? "收起摘要" : "查看摘要"}
                                </button>
                              )}
                            </div>
                          </div>

                          {isToday && summaryError ? (
                            <p className="px-1 py-1 text-[10.5px] leading-relaxed text-error">
                              当日摘要读取失败：{summaryError}
                            </p>
                          ) : null}

                          {expanded ? (
                            <div className="od-record-group-summary">
                              {summaryBody ? (
                                <div data-daily-summary-card="">
                                  <div className="flex items-center gap-2">
                                    <span className="wr-summary-eyebrow">当日摘要</span>
                                    {summaryBody.status ? (
                                      <span className="wr-summary-status">{summaryBody.status}</span>
                                    ) : null}
                                    {summaryBody.generated_at ? (
                                      <span className="mono ml-auto text-[10px] text-outline">
                                        {revisionTime(summaryBody.generated_at)}
                                      </span>
                                    ) : null}
                                  </div>
                                  {summaryBody.stale ? (
                                    <p className="mt-1 text-[10.5px] leading-relaxed text-amber-700">
                                      有 {summaryBody.missing_record_ids.length} 条引用记录已不存在，摘要待复核。
                                    </p>
                                  ) : null}
                                  <div className="mt-1.5" data-daily-summary-body="">
                                    <MarkdownText text={summaryBody.body} />
                                    {summaryBody.record_ids.length > 0 ? (
                                      <div className="mt-2 flex flex-wrap gap-1.5">
                                        {summaryBody.record_ids.map((id) => (
                                          <button
                                            key={id}
                                            type="button"
                                            onClick={() => void openSummaryRecord(id)}
                                            data-daily-summary-record={id}
                                            className="wr-ref-chip"
                                          >
                                            #{id}
                                          </button>
                                        ))}
                                      </div>
                                    ) : null}
                                  </div>
                                </div>
                              ) : state?.loading ? (
                                <p className="px-1 py-1 text-[10.5px] text-outline">正在读取摘要…</p>
                              ) : state?.error ? (
                                <p className="px-1 py-1 text-[10.5px] text-error">
                                  摘要读取失败：{state.error}
                                </p>
                              ) : state?.read && !state.read.exists ? (
                                <p className="px-1 py-1 text-[10.5px] text-outline">当日无摘要。</p>
                              ) : null}
                            </div>
                          ) : null}

                          <div className="od-record-group-cards">
                            {group.records.map((record) => {
                              const active = selected?.id === record.id;
                              const time = recordTimeLabel(record.created_at);
                              const source = recordSourceLabel(record);
                              return (
                                <button
                                  key={record.id}
                                  onClick={() => selectRecord(record)}
                                  data-record-row={record.id}
                                  aria-label={
                                    record.record_date +
                                    (time ? " " + time : "") +
                                    " " +
                                    (record.title.trim() || "未命名记录")
                                  }
                                  className={"od-record-card" + (active ? " is-active" : "")}
                                >
                                  <span className="wr-card-time" data-record-card-time="">
                                    <span className="mono">{time ?? "—"}</span>
                                  </span>
                                  <span className="wr-card-main">
                                    <span className="wr-card-kicker">
                                      <span
                                        className={"wr-category " + categoryToneClass(record.category)}
                                        data-record-card-category=""
                                      >
                                        {recordCategoryLabel(record.category)}
                                      </span>
                                      {record.draft ? (
                                        <span className="wr-draft" data-record-draft="">
                                          草稿
                                        </span>
                                      ) : null}
                                    </span>
                                    <span className="wr-card-title" data-record-card-title="">
                                      {record.title.trim() || "未命名记录"}
                                    </span>
                                    <span className="wr-card-preview" data-record-card-preview="">
                                      {recordPreview(record.body)}
                                    </span>
                                    <span className="wr-card-meta">
                                      {source ? (
                                        <span
                                          className="wr-meta-item"
                                          title={source}
                                          data-record-card-source=""
                                        >
                                          <Icon name="note" className="h-3 w-3" />
                                          <span className="max-w-[230px] truncate">来源：{source}</span>
                                        </span>
                                      ) : null}
                                      <span className="wr-meta-item" data-record-linked-count="">
                                        <Icon name="checklist" className="h-3 w-3" />
                                        {record.linked_task_ids.length > 0
                                          ? "关联任务 #" +
                                            record.linked_task_ids[0] +
                                            (record.linked_task_ids.length > 1
                                              ? " 等 " + record.linked_task_ids.length + " 个"
                                              : "")
                                          : "0 个任务"}
                                      </span>
                                    </span>
                                  </span>
                                  <span className="wr-card-side">
                                    <span
                                      className={"wr-status " + STATUS_TONE[record.ai_analysis_status]}
                                      data-record-status={record.ai_analysis_status}
                                    >
                                      <Icon
                                        name={STATUS_ICON[record.ai_analysis_status]}
                                        className="h-3 w-3"
                                      />
                                      {STATUS_LABEL[record.ai_analysis_status]}
                                    </span>
                                    <span className="wr-card-chevron" aria-hidden="true">
                                      <Icon name="chevron-down" className="h-3.5 w-3.5" />
                                    </span>
                                  </span>
                                </button>
                              );
                            })}
                          </div>
                        </div>
                      );
                    })
                  )}
                  {listing.broken > 0 ? (
                    <p className="px-1.5 pt-2 text-[10.5px] text-outline">
                      {listing.broken} 个文件无法读取，未计入列表。
                    </p>
                  ) : null}
                </div>
              </>
            )}
          </section>
        </div>

        <aside className="wr-side" aria-label="工作概览">
          <section className="wr-side-card" data-work-overview="">
            <div className="wr-side-head">
              <span className="wr-side-mark is-violet" aria-hidden="true">
                <Icon name="trend-up" className="h-4 w-4" />
              </span>
              <h2 className="wr-side-title">本月工作概览</h2>
              <button
                type="button"
                onClick={() => focusOverview(null)}
                className="wr-text-link"
              >
                详情
                <Icon name="arrow-right" className="h-3 w-3" />
              </button>
            </div>
            <div className="wr-stat-grid">
              <button
                type="button"
                onClick={() => focusOverview(null)}
                data-overview-stat="total"
                className="wr-stat"
              >
                <span className="wr-stat-icon is-blue" aria-hidden="true">
                  <Icon name="note" className="h-4 w-4" />
                </span>
                <span className="wr-stat-number">{stats.month}</span>
                <span className="wr-stat-label">记录总数</span>
              </button>
              <button
                type="button"
                onClick={() => focusOverview("analyzed")}
                data-overview-stat="analyzed"
                className="wr-stat"
              >
                <span className="wr-stat-icon is-green" aria-hidden="true">
                  <Icon name="check" className="h-4 w-4" />
                </span>
                <span className="wr-stat-number">{monthAnalyzed}</span>
                <span className="wr-stat-label">已解析</span>
              </button>
              <button
                type="button"
                onClick={() => focusOverview("unanalyzed")}
                data-overview-stat="pending"
                className="wr-stat"
              >
                <span className="wr-stat-icon is-amber" aria-hidden="true">
                  <Icon name="clock" className="h-4 w-4" />
                </span>
                <span className="wr-stat-number">{monthPending}</span>
                <span className="wr-stat-label">待解析</span>
              </button>
            </div>
            <button
              type="button"
              onClick={onOpenAudit}
              data-open-audit=""
              className="wr-audit-link"
            >
              <Icon name="shield-check" className="h-3.5 w-3.5" />
              AI 候选审核
              <Icon name="arrow-right" className="ml-auto h-3 w-3" />
            </button>
          </section>

          <button
            type="button"
            onClick={generateMonthlyReview}
            data-monthly-review-cta=""
            className="wr-review-cta"
          >
            <span className="wr-review-cta-mark" aria-hidden="true">
              <Icon name="star" className="h-4.5 w-4.5" />
            </span>
            <span className="min-w-0 flex-1">
              <span className="wr-review-cta-title">一键生成 · 本月复盘</span>
              <span className="wr-review-cta-sub">基于工作记录和任务，生成月度总结</span>
            </span>
            <span className="wr-review-cta-arrow" aria-hidden="true">
              <Icon name="arrow-right" className="h-4 w-4" />
            </span>
          </button>

          <section className="wr-side-card">
            <h2 className="wr-side-section-title">快捷操作</h2>
            <div className="wr-quick-grid">
              <button
                type="button"
                onClick={generateWeeklyReview}
                data-quick-action="weekly-review"
                className="wr-quick"
              >
                <span className="wr-quick-icon is-violet" aria-hidden="true">
                  <Icon name="calendar" className="h-4 w-4" />
                </span>
                <span className="min-w-0">
                  <span className="wr-quick-label">生成周复盘</span>
                  <span className="wr-quick-sub">本周工作总结</span>
                </span>
              </button>
              <button
                type="button"
                onClick={generateMonthlyReview}
                data-quick-action="monthly-review"
                className="wr-quick"
              >
                <span className="wr-quick-icon is-blue" aria-hidden="true">
                  <Icon name="calendar" className="h-4 w-4" />
                </span>
                <span className="min-w-0">
                  <span className="wr-quick-label">生成月总结</span>
                  <span className="wr-quick-sub">月度工作回顾</span>
                </span>
              </button>
              <button
                type="button"
                onClick={organizeSelected}
                disabled={!selected}
                title={selected ? "交给首页对话整理这条记录" : "先在时间线选择一条记录"}
                data-quick-action="organize"
                className="wr-quick"
              >
                <span className="wr-quick-icon is-amber" aria-hidden="true">
                  <Icon name="edit" className="h-4 w-4" />
                </span>
                <span className="min-w-0">
                  <span className="wr-quick-label">整理选中记录</span>
                  <span className="wr-quick-sub">优化记录内容</span>
                </span>
              </button>
              <button
                type="button"
                onClick={openTaskLink}
                disabled={!selected}
                title={selected ? "把这条记录关联到已有任务" : "先在时间线选择一条记录"}
                data-quick-action="link-task"
                className="wr-quick"
              >
                <span className="wr-quick-icon is-green" aria-hidden="true">
                  <Icon name="branch" className="h-4 w-4" />
                </span>
                <span className="min-w-0">
                  <span className="wr-quick-label">关联任务</span>
                  <span className="wr-quick-sub">添加相关任务</span>
                </span>
              </button>
            </div>
            {selected ? (
              <p className="wr-selected-hint" title={selected.title}>
                已选中：{selected.title}
              </p>
            ) : null}
          </section>

          <button
            type="button"
            onClick={() => {
              requestNewThread("weekly", "帮我整理工作记录");
              onNavigate("home");
            }}
            data-record-ai-hint=""
            className="wr-hint"
          >
            <span className="wr-hint-icon" aria-hidden="true">
              <Icon name="spark" className="h-4 w-4" />
            </span>
            <span className="min-w-0 flex-1">
              你可以在首页对话中，让 AI 帮你整理工作记录
            </span>
            <Icon name="arrow-right" className="h-3.5 w-3.5 opacity-60" />
          </button>
        </aside>
      </div>

      {selected ? (
        <div className="fixed inset-0 z-40" data-record-drawer="">
          <button
            type="button"
            aria-label="关闭记录详情"
            onClick={closeSelected}
            className="absolute inset-0 bg-[rgba(24,26,40,0.22)]"
          />
          <section
            aria-label="记录详情"
            className="wr-drawer absolute right-0 top-0 flex h-full w-[min(620px,94vw)] flex-col overflow-y-auto px-5 py-5"
            data-work-record-detail=""
          >
            <div className="wr-detail-top">
              <span className="wr-eyebrow">记录详情</span>
              <button
                type="button"
                onClick={closeSelected}
                aria-label="关闭"
                className="wr-icon-button"
              >
                <Icon name="x" className="h-4 w-4" />
              </button>
            </div>

            {editing && form ? (
              <div className="flex flex-col gap-2.5 min-h-0 overflow-y-auto">
                <p className="text-[11.5px] font-medium text-on-surface">编辑记录</p>
                <input
                  value={form.title}
                  onChange={(e) => setForm({ ...form, title: e.target.value })}
                  aria-label="标题"
                  className={FIELD_BLOCK}
                />
                <div
                  className="flex flex-wrap items-center gap-1.5"
                  aria-label="建议分类"
                  data-category-chips=""
                >
                  {SUGGESTED_CATEGORIES.map((category) => (
                    <button
                      key={category}
                      type="button"
                      onClick={() => setForm({ ...form, category })}
                      data-category-chip={category}
                      className={
                        "rounded-full px-2.5 py-1 text-[10.5px] transition-colors " +
                        (form.category === category
                          ? "bg-primary-fixed/70 text-primary"
                          : "bg-surface-container text-on-surface-variant hover:bg-surface-container-high")
                      }
                    >
                      {category}
                    </button>
                  ))}
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <input
                    type="date"
                    value={form.record_date}
                    onChange={(e) => setForm({ ...form, record_date: e.target.value })}
                    aria-label="记录日期"
                    className={FIELD}
                  />
                  <input
                    value={form.category}
                    onChange={(e) => setForm({ ...form, category: e.target.value })}
                    placeholder="分类"
                    aria-label="分类"
                    className={FIELD}
                  />
                  <label className="flex items-center gap-1.5 text-[11.5px] text-on-surface-variant">
                    <input
                      type="checkbox"
                      checked={form.draft}
                      onChange={(e) => setForm({ ...form, draft: e.target.checked })}
                      className="h-3.5 w-3.5 accent-[color:var(--color-primary)]"
                    />
                    草稿
                  </label>
                </div>
                <input
                  value={form.tags}
                  onChange={(e) => setForm({ ...form, tags: e.target.value })}
                  placeholder="标签，用逗号分隔"
                  aria-label="标签"
                  className={FIELD_BLOCK}
                />
                <textarea
                  value={form.body}
                  onChange={(e) => setForm({ ...form, body: e.target.value })}
                  aria-label="正文"
                  className={FIELD_BLOCK + " min-h-[280px] resize-y leading-relaxed"}
                />
                <div className="flex items-center gap-2 pt-1">
                  <button
                    onClick={() => void saveEdit()}
                    disabled={busy}
                    className="wr-primary-button"
                  >
                    保存
                  </button>
                  <button
                    onClick={() => {
                      setEditing(false);
                      setForm(null);
                    }}
                    className="wr-ghost-button"
                  >
                    取消
                  </button>
                </div>
              </div>
            ) : (
              <div className="flex flex-col min-h-0">
                <div className="mt-1 flex items-start gap-2.5">
                  <h2
                    className="min-w-0 flex-1 text-[21px] font-semibold leading-snug tracking-tight text-on-surface"
                    data-hover-hint={selected.title}
                  >
                    {selected.title}
                  </h2>
                </div>
                <div className="wr-detail-facts">
                  <span className="mono">{selected.record_date}</span>
                  {selected.category ? (
                    <span className={"wr-category " + categoryToneClass(selected.category)}>
                      {selected.category}
                    </span>
                  ) : null}
                  {selected.draft ? <span className="wr-draft">草稿</span> : null}
                  <span
                    className={"wr-status " + STATUS_TONE[selected.ai_analysis_status]}
                  >
                    <Icon
                      name={STATUS_ICON[selected.ai_analysis_status]}
                      className="h-3 w-3"
                    />
                    {STATUS_LABEL[selected.ai_analysis_status]}
                  </span>
                  <span className="flex items-center gap-1">
                    <Icon name="checklist" className="h-3.5 w-3.5" />
                    {selected.linked_task_ids.length} 个任务
                  </span>
                </div>
                {selected.tags.length > 0 ? (
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {selected.tags.map((tag) => (
                      <span key={tag} className="wr-tag">
                        {"#" + tag}
                      </span>
                    ))}
                  </div>
                ) : null}
                <KnowledgeLinkPanel
                  subjectKind="record"
                  subjectId={selected.id}
                  links={selected.related_knowledge}
                  onNotice={setNotice}
                  onLinksChanged={(links) => {
                    setSelected((prev) =>
                      prev && prev.id === selected.id
                        ? { ...prev, related_knowledge: links }
                        : prev,
                    );
                    setListing((prev) => ({
                      ...prev,
                      items: prev.items.map((item) =>
                        item.id === selected.id ? { ...item, related_knowledge: links } : item,
                      ),
                    }));
                  }}
                />

                <section className="mt-3" aria-label="关联任务" data-record-task-links="">
                  <div className="flex items-center gap-1.5">
                    <Icon name="checklist" className="h-3.5 w-3.5 text-outline" />
                    <h3 className="text-[11.5px] font-medium text-on-surface">关联任务</h3>
                    <button
                      type="button"
                      onClick={openTaskLink}
                      data-record-task-manage=""
                      className="wr-text-link ml-auto"
                    >
                      管理关联
                    </button>
                  </div>
                  {selected.linked_task_ids.length === 0 ? (
                    <p className="mt-1.5 text-[11px] text-outline">
                      还没有关联任务。可在任务候选审核或「管理关联」里添加。
                    </p>
                  ) : (
                    <div className="mt-1.5 flex flex-col gap-1">
                      {selected.linked_task_ids.map((id) => {
                        const task = detailTasks[id];
                        return (
                          <div key={id} className="wr-linked-task" data-record-task={id}>
                            <span className="mono shrink-0 text-[10px] text-outline">#{id}</span>
                            <span className="min-w-0 flex-1 truncate text-[11px] text-on-surface">
                              {task?.title ?? "任务已删除或读不到"}
                            </span>
                            {task ? <span className="wr-task-state">{task.state}</span> : null}
                          </div>
                        );
                      })}
                    </div>
                  )}
                </section>

                <section className="mt-3" aria-label="关联复盘与文稿" data-record-backlinks="">
                  <div className="flex items-center gap-1.5">
                    <Icon name="book" className="h-3.5 w-3.5 text-outline" />
                    <h3 className="text-[11.5px] font-medium text-on-surface">关联复盘 / 文稿</h3>
                    {backlinks && backlinks.items.length > 0 ? (
                      <span className="ml-auto mono text-[10.5px] text-outline">
                        {backlinks.items.length}
                      </span>
                    ) : null}
                  </div>
                  {backlinksLoading ? (
                    <p className="mt-2 text-[11px] text-outline">正在扫描关联文档…</p>
                  ) : backlinksError ? (
                    <p className="mt-2 text-[11px] leading-relaxed text-error" data-backlinks-error="">
                      关联文档读取失败：{backlinksError}
                    </p>
                  ) : !backlinks || backlinks.items.length === 0 ? (
                    <p className="mt-2 text-[11px] text-outline" data-backlinks-empty="">
                      还没有复盘或文稿引用这条记录。
                    </p>
                  ) : (
                    <div className="mt-2 flex flex-col gap-1.5">
                      {backlinks.items.map((item) => {
                        const canOpen =
                          item.kind === "review" && item.period && item.start && item.end;
                        const label = item.title ?? item.range ?? item.id;
                        const chip =
                          item.kind === "review"
                            ? "复盘" + (item.period ? REVIEW_PERIOD_LABEL[item.period] : "")
                            : "文稿";
                        const body = (
                          <>
                            <span className="wr-backlink-kind">{chip}</span>
                            <span className="min-w-0 flex-1 truncate text-[11px] text-on-surface">
                              {label}
                            </span>
                            {item.range && item.title ? (
                              <span className="mono shrink-0 text-[10px] text-outline">
                                {item.range}
                              </span>
                            ) : null}
                            {item.status ? (
                              <span className="wr-summary-status">{item.status}</span>
                            ) : null}
                          </>
                        );
                        if (canOpen) {
                          return (
                            <button
                              key={item.kind + ":" + item.path}
                              type="button"
                              onClick={() =>
                                onOpenReview({
                                  period: item.period as PeriodReviewPeriod,
                                  start: item.start as string,
                                  end: item.end as string,
                                })
                              }
                              data-record-backlink={item.kind}
                              className="wr-backlink"
                            >
                              {body}
                            </button>
                          );
                        }
                        return (
                          <div
                            key={item.kind + ":" + item.path}
                            data-record-backlink={item.kind}
                            className="wr-backlink"
                          >
                            {body}
                          </div>
                        );
                      })}
                    </div>
                  )}
                  {backlinks && backlinks.broken > 0 ? (
                    <p className="mt-1.5 text-[10px] text-outline" data-backlinks-broken="">
                      {backlinks.broken} 个文档无法读取。
                    </p>
                  ) : null}
                </section>

                <div className="mt-3 pt-3 border-t border-surface-variant/25 flex-1 min-h-0 overflow-y-auto">
                  {selected.raw_input?.trim() ? (
                    <details
                      className="mb-3 rounded-xl border border-outline-variant/35 bg-surface-container/35"
                      data-raw-input=""
                    >
                      <summary className="flex cursor-pointer select-none list-none items-center gap-2 px-3 py-2 text-[11.5px] text-on-surface-variant">
                        <Icon name="lock" className="h-3.5 w-3.5 text-outline" />
                        <span className="font-medium text-on-surface">原始输入</span>
                        <span className="ml-auto text-[10.5px] text-outline" data-raw-readonly="">
                          原始素材，不可修改
                        </span>
                      </summary>
                      <div className="border-t border-outline-variant/25 px-3 py-2.5">
                        {selected.raw_source?.trim() ? (
                          <p className="mb-1.5 text-[10.5px] text-outline" data-raw-source="">
                            {selected.raw_source}
                          </p>
                        ) : null}
                        <p className="whitespace-pre-wrap text-[11.5px] leading-relaxed text-on-surface-variant">
                          {selected.raw_input}
                        </p>
                      </div>
                    </details>
                  ) : null}
                  <MarkdownText text={selected.body} />
                  <section className="mt-4 pt-3 border-t border-surface-variant/25" data-revision-history="">
                    <div className="flex items-center gap-1.5">
                      <Icon name="history" className="h-3.5 w-3.5 text-outline" />
                      <h3 className="text-[11.5px] font-medium text-on-surface">修订历史</h3>
                      {!revisionsLoading && !revisionsError && revisions.length > 0 ? (
                        <span className="ml-auto mono text-[10.5px] text-outline">
                          {revisions.length} 次
                        </span>
                      ) : null}
                    </div>
                    {revisionsLoading ? (
                      <p className="mt-2 text-[11px] text-outline">正在读取修订历史…</p>
                    ) : revisionsError ? (
                      <p className="mt-2 text-[11px] leading-relaxed text-error" data-revision-error="">
                        修订历史读取失败：{revisionsError}
                      </p>
                    ) : revisions.length === 0 ? (
                      <p className="mt-2 text-[11px] text-outline">还没有修订记录。</p>
                    ) : (
                      <ol className="mt-2 flex flex-col gap-1.5" data-revision-list="">
                        {[...revisions].reverse().map((revision, index) => (
                          <li
                            key={revision.at + revision.changed_fields.join(",") + index}
                            className="flex items-start gap-2 rounded-lg bg-surface-container/45 px-2.5 py-1.5"
                            data-revision-item=""
                          >
                            <span className="mono shrink-0 text-[10.5px] text-outline">
                              {revisionTime(revision.at)}
                            </span>
                            <span className="text-[11px] text-on-surface-variant">
                              {changedFieldsLabel(revision.changed_fields)}
                            </span>
                          </li>
                        ))}
                      </ol>
                    )}
                  </section>
                </div>
                <div className="wr-detail-actions">
                  <button
                    onClick={() => void parseSelected()}
                    disabled={parsing || selected.ai_analysis_status === "analyzed"}
                    data-parse-entry=""
                    data-hover-hint={
                      selected.ai_analysis_status === "analyzed"
                        ? "这条记录已经解析过"
                        : "用模型提取任务候选；结果先进入审核"
                    }
                    className="wr-ghost-button"
                  >
                    {parsing ? "解析中…" : "解析"}
                  </button>
                  <button onClick={startEdit} className="wr-ghost-button is-primary">
                    编辑
                  </button>
                  {selected.raw_input?.trim() ? (
                    <button
                      onClick={() => setConfirmRephrase(true)}
                      data-reorganize-entry=""
                      className="wr-ghost-button"
                    >
                      重新整理
                    </button>
                  ) : null}
                  <button
                    onClick={handSelectedToAi}
                    data-hand-record={selected.id}
                    className="wr-ghost-button"
                  >
                    交给 AI 继续分析
                  </button>
                  <button
                    onClick={onOpenAudit}
                    className="wr-ghost-button"
                  >
                    审核候选
                  </button>
                  {confirmDelete ? (
                    <div className="ml-auto flex items-center gap-2">
                      <button
                        onClick={() => void removeSelected()}
                        disabled={busy}
                        className="rounded-full bg-error px-3 py-1.5 text-[11px] font-medium text-on-error disabled:opacity-40"
                      >
                        确认删除
                      </button>
                      <button
                        onClick={() => setConfirmDelete(false)}
                        className="text-[11px] text-on-surface-variant hover:text-primary"
                      >
                        取消
                      </button>
                    </div>
                  ) : (
                    <button
                      onClick={() => setConfirmDelete(true)}
                      className="ml-auto rounded-full px-3 py-1.5 text-[11px] font-medium text-outline hover:text-error hover:bg-error/5"
                    >
                      删除
                    </button>
                  )}
                </div>
              </div>
            )}
          </section>
        </div>
      ) : null}

      {taskLinkOpen && selected ? (
        <RecordTaskLinkDialog
          key={selected.id}
          record={selected}
          onClose={() => setTaskLinkOpen(false)}
          onSaved={(record) => {
            setSelected(record);
            setTaskLinkOpen(false);
            setReloadTick((value) => value + 1);
          }}
        />
      ) : null}

      <ConfirmDialog
        open={confirmRephrase}
        title="依据原始素材重新整理"
        description="将以这条记录的原始素材为依据重写正文。原始素材不会被修改；确认后打开首页对话并预填素材，生成的新正文仍需你确认。"
        confirmLabel="去重新整理"
        onConfirm={reorganizeSelected}
        onCancel={() => setConfirmRephrase(false)}
        testId="rephrase-dialog"
      />
    </div>
  );
}
