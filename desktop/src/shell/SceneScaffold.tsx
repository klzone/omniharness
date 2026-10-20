import { useLayoutEffect, useRef, useState, type ReactNode } from "react";

/**
 * 场景骨架（OD-ARCH-033 目标 1；OD-ARCH-040 两层布局）。
 *
 * 首页与三个场景页**共用**这一套布局：顶栏、Dock、下部对话区完全一致，
 * 只有「上部」按场景注入（首页注入今日概览三模块；场景页注入提醒栏 + 双栏）。
 *
 * **两层结构，两层互斥（用户口径）：**
 * - 下层：顶部的卡片区（问候 + 三模块 / 提醒栏 + 双栏）；
 * - 上层：对话（对话历史 + 输入框），**占满上下全屏**，叠在卡片之上。
 *
 * **谁在焦点，谁在上，另一个透明度为 0：**
 * - 指针在**卡片区** → 卡片全亮在上，对话收在卡片下方（两者不重叠，各看各的）；
 * - 指针在**对话区**（对话历史 / 输入框）→ 对话整段上移占满顶部、**全亮在上**，
 *   卡片层**透明度降到 0**（彻底让位，不再半透明叠字）。
 *
 * 早前一版让卡片停在 30% 造成两层半透明文字叠在一起、字迹混乱；已按用户反馈改为互斥。
 *
 * 判定用**指针在容器内的纵向位置**而不是元素 hover：卡片带与对话层会互相遮挡，
 * 纯 hover 判定会出现「卡片永远回不来」的死角。
 */
export function SceneScaffold({
  upper,
  chat,
}: {
  upper: ReactNode;
  chat: ReactNode;
}) {
  const upperRef = useRef<HTMLDivElement | null>(null);
  const upperHRef = useRef(0);
  const pointerYRef = useRef<number | null>(null);
  const [upperH, setUpperH] = useState(0);
  const [chatActive, setChatActive] = useState(false);

  /** 按当前卡片带高度与指针位置重新判定层级。 */
  const refreshLayer = () => {
    const y = pointerYRef.current;
    setChatActive(y !== null && y > upperHRef.current);
  };

  // 卡片带的实际高度（内容多高就多高）：卡片内容是异步就绪的，
  // 高度变了要立刻重判一次指针位置，否则会出现「卡片带高度为 0 时误判为已进入对话区」。
  useLayoutEffect(() => {
    const el = upperRef.current;
    if (!el) return;
    const measure = () => {
      const next = el.offsetHeight;
      if (next === upperHRef.current) return;
      upperHRef.current = next;
      setUpperH(next);
      refreshLayer();
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const handleMove = (event: React.MouseEvent<HTMLElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const y = event.clientY - rect.top;
    if (y === pointerYRef.current) return;
    pointerYRef.current = y;
    setChatActive(y > upperHRef.current);
  };

  return (
    <main
      className="relative flex-1 w-full max-w-7xl mx-auto px-6 sm:px-12 md:pl-28 md:pr-12 flex flex-col min-h-0 overflow-clip"
      onMouseMove={handleMove}
      onMouseLeave={() => {
        pointerYRef.current = null;
        setChatActive(false);
      }}
    >
      {/* 下层：顶部卡片区。指针进入对话区时**彻底让位**（透明度 0），
          避免两层半透明文字叠在一起。 */}
      <div
        ref={upperRef}
        className={
          "absolute inset-x-0 top-0 z-0 px-6 sm:px-12 md:pl-28 md:pr-12 transition-opacity duration-300 " +
          (chatActive ? "opacity-0 pointer-events-none" : "opacity-100")
        }
      >
        {upper}
      </div>

      {/* 上层：对话，占满上下；指针在其上时对话上移，叠在卡片之上。 */}
      <section
        className="relative z-10 flex-1 flex flex-col min-h-0 pb-3"
        style={{
          paddingTop: chatActive ? 0 : upperH,
          transition: "padding-top 300ms ease",
        }}
      >
        {chat}
      </section>
    </main>
  );
}
