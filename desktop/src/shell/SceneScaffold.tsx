import { useLayoutEffect, useRef, useState, type ReactNode } from "react";

/**
 * 场景骨架（OD-ARCH-033 目标 1；OD-ARCH-040 两层布局）。
 *
 * 首页与三个场景页**共用**这一套布局：顶栏、Dock、下部对话区完全一致，
 * 只有「上部」按场景注入（首页注入今日概览三模块；场景页注入提醒栏 + 双栏）。
 *
 * **两层结构（用户口径）：**
 * - 下层：顶部的卡片区（问候 + 三模块 / 提醒栏 + 双栏），**平时可见**；
 * - 上层：对话，**占满上下全屏**，叠在卡片之上。
 *
 * 鼠标位置决定层级：**指针落在卡片带内 → 卡片保持显示**；
 * **指针进入对话区（对话历史 / 输入框）→ 卡片渐隐，同时对话上移占满顶部**，
 * 即「对话历史显示在卡片上面」。指针离开窗口则恢复卡片显示。
 *
 * 判定用**指针在容器内的纵向位置**而不是元素 hover：卡片带在对话层之下会互相遮挡，
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
      {/* 下层：顶部卡片区。指针进入对话区时渐隐并让出点击。
          渐隐到 30% 而不是 0：卡片保留一点存在感（不彻底消失），对话叠在其上。 */}
      <div
        ref={upperRef}
        className={
          "absolute inset-x-0 top-0 z-0 px-6 sm:px-12 md:pl-28 md:pr-12 transition-opacity duration-300 " +
          (chatActive ? "opacity-30 pointer-events-none" : "opacity-100")
        }
      >
        {upper}
      </div>

      {/* 上层：对话，占满上下；指针在其上时卡片让位、对话上移。 */}
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
