import {
  createContext,
  useContext,
  type ReactNode,
} from "react";

/** 输入框是否获得键盘焦点（由对话组件上报）。 */
type ChatFocusReporter = (focused: boolean) => void;

const ChatFocusContext = createContext<ChatFocusReporter>(() => {});

/**
 * 对话区向骨架上报「输入框是否获得焦点」。
 *
 * 保留为兼容导出：OD-ARCH-081 起布局固定、不再随焦点变化，本函数恒为空实现。
 * 对话组件仍按时调用它，后续若恢复「聚焦时收紧上部」的行为，只需改这里。
 */
export function useChatInputFocus(): ChatFocusReporter {
  return useContext(ChatFocusContext);
}

/**
 * 场景骨架（OD-ARCH-033 目标 1；OD-ARCH-081 固定两层）。
 *
 * 首页与三个场景页**共用**这一套布局：顶栏、Dock、下部对话区完全一致，
 * 只有「上部」按场景注入（首页注入今日概览三模块；场景页注入提醒栏 + 双栏）。
 *
 * **布局（OD-ARCH-081，用户口径）：** 上部卡片（问候 + 待办）**常驻**在对话之上，
 * 不因指针移动、输入聚焦或鼠标静止而隐藏、淡化或让对话上移；对话区在其下方
 * 占满剩余高度，自己滚动。原先的「两层互斥 + 5 秒静止恢复」已按用户要求移除。
 */
export function SceneScaffold({
  upper,
  chat,
}: {
  upper: ReactNode;
  chat: ReactNode;
}) {
  return (
    <main
      className="relative flex-1 w-full max-w-7xl mx-auto px-6 sm:px-12 md:pl-28 md:pr-12 flex flex-col min-h-0 overflow-clip"
      data-testid="scene-scaffold"
    >
      {/* 上部：问候 + 待办，常驻（OD-ARCH-081）。内容多高就多高，不叠在对话上。 */}
      <div className="relative z-0 flex-shrink-0" data-testid="scene-upper">
        {upper}
      </div>

      {/* 下部：对话，占满剩余高度。 */}
      <section className="relative z-10 flex-1 flex flex-col min-h-0 pb-3">
        <div className="flex-1 flex flex-col min-h-0">{chat}</div>
      </section>
    </main>
  );
}
