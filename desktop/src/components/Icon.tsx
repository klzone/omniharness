import type { JSX } from "react";

/**
 * 本地内联图标集。
 *
 * 设计稿原本使用 Google 的 Material Symbols 可变字体（外链），会被 Tauri CSP
 * （style-src/font-src 'self'）拦截且离线不可用，故一律改为内联 SVG：
 * - 24×24 viewBox，stroke=currentColor，随文字颜色 / 尺寸变化；
 * - 不引入任何外部依赖或字体。
 */
export type IconName =
  | "brand"
  | "search"
  | "sun"
  | "settings"
  | "bell"
  | "note"
  | "book"
  | "terminal"
  | "radar"
  | "plus"
  | "checklist"
  | "calendar"
  | "spark"
  | "arrow-up"
  | "stop"
  | "arrow-right"
  | "bot"
  | "history"
  | "shield-check"
  | "folder"
  | "mic"
  | "trend-up"
  | "lock";

const PATHS: Record<IconName, JSX.Element> = {
  brand: (
    <>
      <circle cx="12" cy="12" r="8" />
      <circle cx="12" cy="12" r="2.5" />
      <path d="M12 4v3M12 17v3M4 12h3M17 12h3" />
    </>
  ),
  search: (
    <>
      <circle cx="11" cy="11" r="6.5" />
      <path d="m20 20-3.6-3.6" />
    </>
  ),
  sun: (
    <>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M2 12h2M20 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4" />
    </>
  ),
  settings: (
    <>
      <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
      <circle cx="16" cy="7" r="2" />
      <circle cx="10" cy="17" r="2" />
    </>
  ),
  bell: (
    <>
      <path d="M6 9a6 6 0 0 1 12 0c0 4 1.5 5 1.5 5H4.5S6 13 6 9Z" />
      <path d="M10 18a2 2 0 0 0 4 0" />
    </>
  ),
  note: (
    <>
      <rect x="4" y="4" width="16" height="16" rx="3" />
      <path d="M8 9h8M8 13h6M8 17h4" />
    </>
  ),
  book: (
    <>
      <path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H19v15H6.5A2.5 2.5 0 0 0 4 20.5Z" />
      <path d="M19 18v3H6.5" />
    </>
  ),
  terminal: (
    <>
      <rect x="3" y="4" width="18" height="16" rx="3" />
      <path d="m7.5 9.5 2.5 2.5-2.5 2.5M12.5 15h4" />
    </>
  ),
  radar: (
    <>
      <circle cx="12" cy="12" r="8" />
      <circle cx="12" cy="12" r="4" />
      <circle cx="12" cy="12" r="1" />
      <path d="M12 12 18 6" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  checklist: (
    <>
      <rect x="4" y="4" width="16" height="16" rx="3" />
      <path d="m8 12 2.5 2.5L16 9" />
    </>
  ),
  calendar: (
    <>
      <rect x="3.5" y="5" width="17" height="15" rx="3" />
      <path d="M3.5 9.5h17M8 3v4M16 3v4" />
    </>
  ),
  spark: (
    <>
      <path d="M12 3v5M12 16v5M3 12h5M16 12h5" />
      <path d="M12 9.5 13.4 12 12 14.5 10.6 12Z" />
    </>
  ),
  "arrow-up": <path d="M12 20V5M6 11l6-6 6 6" />,
  /** 生成中替换发送箭头：实心方块（停止/取消），轮廓与其它图标一致由外层 stroke 控制。 */
  stop: <rect x="7" y="7" width="10" height="10" rx="2.5" fill="currentColor" stroke="none" />,
  "arrow-right": <path d="M5 12h13M13 6l6 6-6 6" />,
  bot: (
    <>
      <rect x="4" y="7" width="16" height="12" rx="4" />
      <path d="M12 3v4M9 13h.01M15 13h.01M9.5 16.5h5" />
    </>
  ),
  history: (
    <>
      <path d="M3.5 12a8.5 8.5 0 1 0 2.6-6.1" />
      <path d="M4 4v4h4" />
      <path d="M12 8v4l3 2" />
    </>
  ),
  "shield-check": (
    <>
      <path d="M12 3 19 6v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6Z" />
      <path d="m9 12 2 2 4-4" />
    </>
  ),
  folder: (
    <path d="M3.5 6.5A2 2 0 0 1 5.5 4.5h4l2 2.5h7A2 2 0 0 1 20.5 9v8A2 2 0 0 1 18.5 19h-13A2 2 0 0 1 3.5 17Z" />
  ),
  mic: (
    <>
      <rect x="9.5" y="3" width="5" height="10" rx="2.5" />
      <path d="M6.5 11a5.5 5.5 0 0 0 11 0M12 16.5V20" />
    </>
  ),
  "trend-up": (
    <>
      <path d="M4 16.5 9.5 11l3.5 3.5L20 7.5" />
      <path d="M15 7.5h5v5" />
    </>
  ),
  lock: (
    <>
      <rect x="5" y="10.5" width="14" height="9.5" rx="2.5" />
      <path d="M8 10.5V8a4 4 0 0 1 8 0v2.5" />
    </>
  ),
};

export function Icon({
  name,
  className,
  strokeWidth = 1.7,
}: {
  name: IconName;
  className?: string;
  strokeWidth?: number;
}) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
      focusable="false"
    >
      {PATHS[name]}
    </svg>
  );
}
