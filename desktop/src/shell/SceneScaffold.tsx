import type { ReactNode } from "react";

/**
 * 场景骨架（OD-ARCH-033 目标 1）。
 *
 * 首页与三个场景页**共用**这一套布局：顶栏、Dock、下部对话区完全一致，
 * 只有「上部」按场景注入（首页注入今日概览三模块；场景页注入提醒栏 + 双栏）。
 */
export function SceneScaffold({
  upper,
  chat,
}: {
  upper: ReactNode;
  chat: ReactNode;
}) {
  return (
    <main className="flex-1 w-full max-w-7xl mx-auto px-6 sm:px-12 md:pl-28 md:pr-12 flex flex-col min-h-0 overflow-clip">
      <div className="flex-shrink-0 pt-1">{upper}</div>
      <section className="flex-1 flex flex-col min-h-0 pb-3 pt-1">{chat}</section>
    </main>
  );
}
