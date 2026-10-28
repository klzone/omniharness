/**
 * 三周节律卡（上周 / 本周 / 下周）。
 *
 * 诚实性约束：任务与日程数据源尚未接入，因此本组件**不伪造**“已完成/进行中/待办”
 * 的业务内容；它只按本机日期给出过去 / 今天 / 将来的时间骨架，并在底部说明。
 * 待数据源接入后，可在同一骨架上挂载真实标记。
 */
const WEEK_LABELS = ["一", "二", "三", "四", "五", "六", "日"];

function startOfWeek(base: Date): Date {
  const d = new Date(base);
  d.setHours(0, 0, 0, 0);
  const offset = (d.getDay() + 6) % 7; // 周一 = 0
  d.setDate(d.getDate() - offset);
  return d;
}

function addDays(base: Date, n: number): Date {
  const d = new Date(base);
  d.setDate(d.getDate() + n);
  return d;
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

function sameDayBefore(a: Date, b: Date): boolean {
  const x = new Date(a);
  x.setHours(0, 0, 0, 0);
  const y = new Date(b);
  y.setHours(0, 0, 0, 0);
  return x.getTime() < y.getTime();
}

export function ThreeWeekCalendar({ now = new Date() }: { now?: Date }) {
  const thisMonday = startOfWeek(now);
  const weeks = [
    { label: "上周", start: addDays(thisMonday, -7) },
    { label: "本周", start: thisMonday },
    { label: "下周", start: addDays(thisMonday, 7) },
  ];

  return (
    <div className="flex flex-col gap-1.5">
      {/* 星期表头 */}
      <div className="flex items-center gap-1">
        <span className="w-9 shrink-0" />
        <div className="grid grid-cols-7 gap-1 flex-1 text-center font-label-caps text-[9px] text-outline">
          {WEEK_LABELS.map((w) => (
            <span key={w}>{w}</span>
          ))}
        </div>
      </div>

      {weeks.map((week) => (
        <div key={week.label} className="flex items-center gap-1">
          <span className="w-9 shrink-0 font-label-caps text-[9px] text-outline">
            {week.label}
          </span>
          <div className="grid grid-cols-7 gap-1 flex-1">
            {Array.from({ length: 7 }, (_, i) => addDays(week.start, i)).map((d) => {
              const today = isSameDay(d, now);
              const past = sameDayBefore(d, now);
              return (
                <div
                  key={d.toISOString()}
                  data-hover-hint={d.toLocaleDateString("zh-CN")}
                  className={
                    "relative flex items-center justify-center h-7 rounded-md font-label-caps text-[10px] transition-colors " +
                    (today
                      ? "bg-primary/10 text-primary border border-primary/25 font-semibold"
                      : past
                        ? "text-outline/55"
                        : "text-on-surface-variant hover:bg-white/70")
                  }
                >
                  {d.getDate()}
                  {today ? (
                    <span className="absolute bottom-0.5 w-1 h-1 rounded-full bg-primary" />
                  ) : past ? (
                    <span className="absolute bottom-0.5 w-1 h-1 rounded-full bg-outline-variant/60" />
                  ) : null}
                </div>
              );
            })}
          </div>
        </div>
      ))}

      {/* 图例 */}
      <div className="pt-1.5 mt-0.5 border-t border-surface-variant/20 flex items-center gap-3 font-label-caps text-[9px] text-outline">
        <span className="inline-flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-outline-variant/60" />
          已过
        </span>
        <span className="inline-flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-primary" />
          今天
        </span>
        <span className="inline-flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-transparent border border-outline-variant/60" />
          将来
        </span>
      </div>

      <p className="font-label-caps text-[9px] text-outline leading-relaxed">
        任务与日程数据源尚未接入；这里只按本机日期区分已过 / 今天 / 将来，
        不显示“做了什么、正在做什么”的业务内容。
      </p>
    </div>
  );
}
