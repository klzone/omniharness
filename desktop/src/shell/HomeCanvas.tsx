import { Icon } from "../components/Icon";
import type { Scene } from "./Dock";
import { HomeChat } from "./HomeChat";
import { PeekPanel } from "./PeekPanel";
import { SceneScaffold } from "./SceneScaffold";
import { ThreeWeekCalendar } from "./ThreeWeekCalendar";

/**
 * 首页画布（OD-ARCH-026 首页版式回归：对话为主 + 顶部三模块）。
 *
 * 版式来自 `_design/stitch_omnidesk`（只读设计原件）：
 *   顶部问候 → 一行三模块（机器人 + 今日重点推进 + 日程节律）→ 下半屏整幅对话。
 *
 * 与设计稿的**刻意差异**（诚实性 / 性能约束）：
 * - 问候语之后不写「今天有 N 项…」这类数字：待办与项目数据源尚未接入，不伪造业务统计。
 * - 「今日重点推进」保留版式但为**明确标注的空态**（徽标「未接入数据源」），不展示演示性待办。
 * - 「日程节律」复用三周日历：只按本机日期给出已过/今天/将来的骨架，徽标「未同步」。
 * - **默认静态**（OD-ARCH-027 Step 2/4）：不使用浮动吉祥物动画、脉冲点动画，
 *   卡片也不用背景模糊；层次由半透明底色 + 描边 + 阴影表达。要加动效须另行批准。
 * - 不显示右上角的装饰性状态串（「OMNIDESK · LOCAL FIRST」等），此处只保留真实信息。
 */
function greeting(date: Date): string {
  const h = date.getHours();
  if (h < 6) return "夜深了";
  if (h < 12) return "早上好";
  if (h < 14) return "中午好";
  if (h < 18) return "下午好";
  return "晚上好";
}

export function HomeCanvas({ onNavigate }: { onNavigate: (scene: Scene) => void }) {
  const now = new Date();

  return (
    <SceneScaffold
      upper={
        <>
      {/* 上部：问候 + 一行三模块（收进悬停面板，把纵向空间让给对话） */}
      <section className="flex flex-col flex-shrink-0 pt-1 pb-1 mb-1">
        <div className="pb-3 flex flex-col sm:flex-row sm:items-baseline justify-between gap-2">
          <h1 className="font-display-hero text-2xl sm:text-3xl font-light text-on-surface tracking-tight">
            {greeting(now)}
            <span className="text-on-surface-variant font-light text-base ml-3 tracking-normal">
              · 欢迎回到 OmniDesk
            </span>
          </h1>
        </div>

        <PeekPanel label="今日概览" hint="机器人 · 重点推进 · 日程节律">
        <div className="relative flex flex-col lg:flex-row items-stretch gap-4">
          {/* 左：机器人（本地透明背景资源），独立一列、底部对齐 */}
          <div className="hidden lg:flex w-[112px] shrink-0 flex-col items-center justify-end">
            <img
              src="/mascot.png"
              alt="OmniDesk 吉祥物"
              className="w-[112px] h-auto drop-shadow-[0_16px_24px_rgba(79,70,229,0.18)]"
            />
          </div>

          {/* 中：今日重点推进（诚实空态，保留版式） */}
          <div className="flex-1 bg-white/80 p-5 border border-white/90 shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] rounded flex flex-col justify-between min-h-[220px]">
            <div className="flex items-center justify-between pb-2.5 border-b border-surface-variant/20">
              <div className="flex items-center space-x-2">
                <div className="w-6 h-6 rounded-full bg-primary/10 text-primary flex items-center justify-center">
                  <Icon name="checklist" className="w-[15px] h-[15px]" />
                </div>
                <h2 className="font-headline-md text-[13px] font-semibold text-on-surface">
                  今日重点推进
                </h2>
                <span className="px-1.5 py-0.5 rounded-full bg-surface-container text-outline font-label-caps text-[9px]">
                  未接入数据源
                </span>
              </div>
            </div>
            <div className="flex-1 flex flex-col items-center justify-center text-center gap-3 py-5">
              <div className="w-11 h-11 rounded-full bg-surface-container-low flex items-center justify-center text-outline">
                <Icon name="history" className="w-5 h-5" />
              </div>
              <p className="text-[13px] text-on-surface-variant max-w-md leading-relaxed">
                待办与项目数据源尚未接入，这里不展示任何待办事项或统计数字。
                本版本已实现的纵向试点是
                <span className="text-primary font-medium">「任务工作台」</span>
                （合成数据 · 五阶段）。
              </p>
              <button
                onClick={() => onNavigate("workbench")}
                className="inline-flex items-center space-x-1.5 px-3.5 py-1.5 rounded-full bg-primary text-on-primary font-label-caps text-[11px] hover:bg-primary-container active:scale-95 transition-all shadow-sm"
              >
                <Icon name="terminal" className="w-[14px] h-[14px]" />
                <span>前往任务工作台</span>
              </button>
            </div>
          </div>

          {/* 右：日程节律（三周日期骨架，不显示业务内容） */}
          <div className="w-full lg:w-[330px] flex-shrink-0 bg-white/80 p-3.5 border border-white/90 shadow-[0_4px_20px_-2px_rgba(15,23,42,0.03)] rounded flex flex-col justify-between min-h-[220px]">
            <div className="flex items-center justify-between pb-1.5 border-b border-surface-variant/20">
              <div className="flex items-center space-x-1.5">
                <div className="w-5 h-5 rounded-full bg-secondary-fixed/50 text-secondary flex items-center justify-center">
                  <Icon name="calendar" className="w-[13px] h-[13px]" />
                </div>
                <span className="font-headline-md text-[12px] font-semibold text-on-surface">
                  日程节律
                </span>
              </div>
              <span
                className="inline-flex items-center px-1.5 py-0.5 rounded-full bg-surface-container text-outline font-label-caps text-[9px]"
                title="日程同步未实现"
              >
                未同步
              </span>
            </div>

            <div className="py-1.5 w-full">
              <ThreeWeekCalendar now={now} />
            </div>
          </div>
        </div>
        </PeekPanel>
      </section>
        </>
      }
      chat={
        /* 下部：整幅对话（全宽），消息区顶部向上渐隐由 chat-fade-mask 提供 */
        <HomeChat onNavigate={onNavigate} />
      }
    />
  );
}
