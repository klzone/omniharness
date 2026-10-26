// 引擎提问卡（OD-ARCH-088）。
//
// 界面参考（OD-ARCH-035，screenshots-design-inspiration：ChatGPT / Claude / Catalant）：
// 提问卡直接长在对话流里、紧贴触发它的助手消息，不做居中模态；选项做成整行可点的
// 列表项，和现有审批卡共用同一套圆角、间距与字号刻度。
//
// 诚实性约束：点选项只改变本地选择，绝不自动提交；只有宿主确认送达后才显示「已答」。
// 失败保留卡片供重试并显示真实原因；引擎撤回/超时或任务终态时禁用全部交互。

import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Icon } from "../components/Icon";
import type { QuestionAnswerItem, QuestionRequest } from "../types";

const MAX_CUSTOM = 4000;

interface QuestionCardProps {
  request: QuestionRequest;
  onSubmit: (taskId: string, questionId: string, answers: QuestionAnswerItem[]) => void;
  onCancelTask: (taskId: string) => void;
  /** 取消请求是否已发出（此时仍保持可答，等引擎确认）。 */
  cancelRequested?: boolean;
}

/** 每道题当前的选择（选项 label 集合 + 自定义文本）。 */
type Draft = Record<string, { selected: string[]; custom: string }>;

function emptyDraft(request: QuestionRequest): Draft {
  const draft: Draft = {};
  for (const question of request.questions) {
    draft[question.id] = { selected: [], custom: "" };
  }
  return draft;
}

export function QuestionCard({ request, onSubmit, onCancelTask, cancelRequested }: QuestionCardProps) {
  const [draft, setDraft] = useState<Draft>(() => emptyDraft(request));
  const cardRef = useRef<HTMLDivElement | null>(null);

  // 同一个 questionId 的题面不应变化；换成新请求时重置选择。
  useEffect(() => {
    setDraft(emptyDraft(request));
  }, [request.questionId]);

  const editable = request.status === "pending" || request.status === "failed";
  const submitting = request.status === "submitting";

  const missing = useMemo(
    () =>
      request.questions.filter((q) => {
        const entry = draft[q.id];
        return !entry || (entry.selected.length === 0 && entry.custom.trim() === "");
      }),
    [draft, request.questions],
  );

  const tooLong = request.questions.some(
    (q) => (draft[q.id]?.custom ?? "").length > MAX_CUSTOM,
  );

  const canSubmit =
    editable && request.answerable && missing.length === 0 && !tooLong;

  const toggle = (questionId: string, label: string, multi: boolean) => {
    if (!editable) return;
    setDraft((prev) => {
      const entry = prev[questionId] ?? { selected: [], custom: "" };
      const selected = multi
        ? entry.selected.includes(label)
          ? entry.selected.filter((v) => v !== label)
          : [...entry.selected, label]
        : entry.selected.includes(label)
          ? []
          : [label];
      return { ...prev, [questionId]: { ...entry, selected } };
    });
  };

  const setCustom = (questionId: string, value: string) => {
    if (!editable) return;
    setDraft((prev) => ({
      ...prev,
      [questionId]: { ...(prev[questionId] ?? { selected: [], custom: "" }), custom: value },
    }));
  };

  const submit = () => {
    if (!canSubmit) return;
    const answers: QuestionAnswerItem[] = request.questions.map((q) => {
      const entry = draft[q.id] ?? { selected: [], custom: "" };
      const item: QuestionAnswerItem = { id: q.id, selected: [...entry.selected] };
      if (entry.custom.trim() !== "") item.custom = entry.custom;
      return item;
    });
    onSubmit(request.omniTaskId, request.questionId, answers);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    // Esc 不提交，也不偷偷关闭：提问仍等用户明确选择或取消任务。
    if (event.key === "Escape") {
      event.stopPropagation();
      return;
    }
    if (event.key === "Enter" && !event.shiftKey && event.target instanceof HTMLInputElement) {
      event.preventDefault();
      submit();
    }
  };

  const disabledReason = !request.answerable
    ? "题面超出可答边界（" + (request.violations.join("、") || "未知原因") + "），本条无法提交；请取消任务或让引擎改用更短的提问。"
    : tooLong
      ? `自定义内容超过 ${MAX_CUSTOM} 字符，请缩短后再提交。`
      : missing.length > 0
        ? "请为每道题选择一项，或填写自定义内容。"
        : null;

  // WebKit 不会把卡片里的普通段落暴露成无障碍文本，只露控件。给卡片和结果提示
  // 一个合并后的无障碍名称，读屏用户与验收脚本都能确认真实题面与状态。
  const cardLabel = [
    "引擎需要你回答一个问题",
    request.note || "选择后提交；不会自动替你选。",
    ...request.questions.map((q) =>
      [q.header, q.question, q.detail].filter(Boolean).join("："),
    ),
  ].join("。");

  return (
    <div
      ref={cardRef}
      className="rounded-lg bg-surface-container px-3 py-2.5 space-y-2.5 text-[12px] leading-relaxed"
      role="group"
      aria-label={cardLabel}
      data-testid="question-card"
      data-question-id={request.questionId}
      data-status={request.status}
      onKeyDown={onKeyDown}
    >
      <div className="flex items-start space-x-2">
        <Icon name="spark" className="w-4 h-4 text-primary flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          <p className="text-on-surface font-medium">引擎需要你回答一个问题</p>
          <p className="text-on-surface-variant">
            {request.note || "选择后提交；不会自动替你选。"}
          </p>
        </div>
      </div>

      {request.questions.map((q) => {
        const entry = draft[q.id] ?? { selected: [], custom: "" };
        const multi = q.multi_select;
        return (
          <fieldset key={q.id} className="space-y-1.5" disabled={!editable || submitting}>
            <legend className="sr-only">{q.header || q.question}</legend>
            {q.header ? (
              <p className="text-[10.5px] uppercase tracking-wide text-outline">{q.header}</p>
            ) : null}
            <p className="text-on-surface font-medium whitespace-pre-wrap break-words">
              {q.question}
            </p>
            {q.detail ? (
              <p className="text-on-surface-variant whitespace-pre-wrap break-words">{q.detail}</p>
            ) : null}
            <div className="space-y-1" role={multi ? "group" : "radiogroup"}>
              {(q.options ?? []).map((option) => {
                const checked = entry.selected.includes(option.label);
                return (
                  <label
                    key={option.label}
                    className={
                      "flex items-start space-x-2 rounded-md border px-2.5 py-1.5 cursor-pointer " +
                      (checked
                        ? "border-primary bg-primary/5"
                        : "border-surface-variant hover:border-outline")
                    }
                  >
                    <input
                      type={multi ? "checkbox" : "radio"}
                      name={q.id}
                      className="mt-0.5"
                      checked={checked}
                      disabled={!editable || submitting}
                      onChange={() => toggle(q.id, option.label, multi)}
                      data-testid="question-option"
                    />
                    <span className="min-w-0">
                      <span className="block text-on-surface">{option.label}</span>
                      {option.description ? (
                        <span className="block text-[11px] text-on-surface-variant">
                          {option.description}
                        </span>
                      ) : null}
                    </span>
                  </label>
                );
              })}
            </div>
            <label className="block">
              <span className="text-[11px] text-outline">
                {multi ? "补充说明（可选，可与上面选项同时填）" : "或者自己填写（可选）"}
              </span>
              <input
                type="text"
                value={entry.custom}
                disabled={!editable || submitting}
                maxLength={MAX_CUSTOM + 1}
                onChange={(e) => setCustom(q.id, e.target.value)}
                placeholder="合成测试时请使用虚构内容"
                className="mt-1 w-full rounded-md border border-surface-variant bg-surface px-2 py-1 text-[12px] text-on-surface"
                data-testid="question-custom"
              />
            </label>
          </fieldset>
        );
      })}

      {request.status === "pending" || request.status === "submitting" ? (
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={!canSubmit || submitting}
            onClick={submit}
            className="px-3 py-1 rounded-full bg-primary text-on-primary text-[11.5px] font-medium disabled:opacity-50"
            data-testid="question-submit"
          >
            {submitting ? "正在提交…" : "提交回答"}
          </button>
          <button
            type="button"
            disabled={cancelRequested}
            onClick={() => onCancelTask(request.omniTaskId)}
            className="px-3 py-1 rounded-full bg-surface-container-high text-on-surface text-[11.5px] font-medium disabled:opacity-50"
            data-testid="question-cancel-task"
          >
            {cancelRequested ? "取消请求已发送" : "取消本次任务"}
          </button>
          {submitting ? <span className="text-[11px] text-outline">正在等待宿主确认…</span> : null}
        </div>
      ) : request.status === "answered" ? (
        <p
          className="text-[11px] text-outline"
          role="status"
          aria-label="已提交，引擎已收到回答。"
          data-testid="question-answered"
        >
          已提交，引擎已收到回答。
        </p>
      ) : request.status === "closed" ? (
        <p
          className="text-[11px] text-outline"
          role="status"
          aria-label={
            request.closedReason === "cancelled"
              ? "该提问已失效（引擎撤回或已超时），未提交任何答案。"
              : "任务已结束，该提问不再可答。"
          }
          data-testid="question-closed"
        >
          {request.closedReason === "cancelled"
            ? "该提问已失效（引擎撤回或已超时），未提交任何答案。"
            : "任务已结束，该提问不再可答。"}
        </p>
      ) : null}

      {request.status === "failed" ? (
        <div className="space-y-1">
          <p className="text-[11px] text-error" data-testid="question-error">
            回传失败，仍待回答：{request.error || "宿主未返回原因。"}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={!canSubmit}
              onClick={submit}
              className="px-3 py-1 rounded-full bg-primary text-on-primary text-[11.5px] font-medium disabled:opacity-50"
              data-testid="question-retry"
            >
              重试提交
            </button>
            {/* 提交失败时仍要留一条退出路径，否则用户只能一直重试。 */}
            <button
              type="button"
              disabled={cancelRequested}
              onClick={() => onCancelTask(request.omniTaskId)}
              className="px-3 py-1 rounded-full bg-surface-container-high text-on-surface text-[11.5px] font-medium disabled:opacity-50"
              data-testid="question-cancel-task"
            >
              {cancelRequested ? "取消请求已发送" : "取消本次任务"}
            </button>
          </div>
        </div>
      ) : null}

      {request.cancelError ? (
        <p className="text-[11px] text-error" data-testid="question-cancel-error">
          取消请求失败，任务仍在等待：{request.cancelError}
        </p>
      ) : null}

      {editable && disabledReason ? (
        <p className="text-[11px] text-outline">{disabledReason}</p>
      ) : null}
    </div>
  );
}
