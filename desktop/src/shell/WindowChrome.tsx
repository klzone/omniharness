import { useEffect, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** 与 @tauri-apps/api 的 Window.startResizeDragging 参数一致（该类型未从包内导出）。 */
type ResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

/**
 * 无系统装饰窗口的自绘外框（frameless）。
 *
 * 目的：让「最小化 / 最大化 / 关闭」融入软件界面，而不是系统标题栏。
 * - `tauri.conf.json` 里 `decorations: false` 关闭系统装饰；
 * - 顶栏（TopNav）加上 `data-tauri-drag-region` 作为拖动区域；
 * - 本文件提供真实的窗口按钮，以及无边框后系统不再提供的边缘/角落缩放热区。
 *
 * 约束：只调用 Tauri 真实的窗口 API，不做假按钮；失败时静默不改变界面状态。
 */

/** 延迟获取，避免在非 Tauri 环境（纯浏览器预览）导入时抛错。 */
function appWindow() {
  return getCurrentWindow();
}

function useMaximized(): boolean {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    const sync = () => {
      appWindow()
        .isMaximized()
        .then((value) => {
          if (!disposed) setMaximized(value);
        })
        .catch(() => {
          /* 非 Tauri 环境：保持默认未最大化 */
        });
    };

    sync();
    appWindow()
      .onResized(() => sync())
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, []);

  return maximized;
}

function Glyph({ children }: { children: ReactNode }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="12"
      height="12"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

/** 顶栏右侧的窗口按钮（最小化 / 最大化还原 / 关闭）。 */
export function WindowControls() {
  const maximized = useMaximized();
  const base =
    "w-7 h-7 rounded-full flex items-center justify-center transition-colors text-on-surface-variant";

  return (
    <div
      className="flex items-center space-x-0.5 pl-2 ml-1 border-l border-outline-variant/25"
      onMouseDown={(e) => e.stopPropagation()}
    >
      <button
        type="button"
        data-hover-hint="最小化"
        aria-label="最小化"
        className={`${base} hover:bg-surface-container`}
        onClick={() => void appWindow().minimize()}
      >
        <Glyph>
          <path d="M5 12h14" />
        </Glyph>
      </button>
      <button
        type="button"
        data-hover-hint={maximized ? "还原" : "最大化"}
        aria-label={maximized ? "还原" : "最大化"}
        className={`${base} hover:bg-surface-container`}
        onClick={() => void appWindow().toggleMaximize()}
      >
        <Glyph>
          {maximized ? (
            <>
              <rect x="8" y="4" width="11" height="11" rx="1.5" />
              <rect x="4" y="8" width="11" height="11" rx="1.5" />
            </>
          ) : (
            <rect x="6" y="6" width="12" height="12" rx="1.5" />
          )}
        </Glyph>
      </button>
      <button
        type="button"
        data-hover-hint="关闭"
        aria-label="关闭"
        className={`${base} hover:bg-red-500 hover:text-white`}
        onClick={() => void appWindow().close()}
      >
        <Glyph>
          <path d="M6 6l12 12M18 6L6 18" />
        </Glyph>
      </button>
    </div>
  );
}

/**
 * 无边框窗口的缩放热区。
 *
 * 关掉系统装饰后，窗口管理器不再提供边框拖拽缩放，因此用 8 个不可见热区
 * 调用 Tauri 的 startResizeDragging。最大化时隐藏，避免与边缘交互冲突。
 */
const EDGES: Array<{ dir: ResizeDirection; cls: string; cursor: string }> = [
  { dir: "North", cls: "top-0 left-3 right-3 h-1.5", cursor: "ns-resize" },
  { dir: "South", cls: "bottom-0 left-3 right-3 h-1.5", cursor: "ns-resize" },
  { dir: "West", cls: "left-0 top-3 bottom-3 w-1.5", cursor: "ew-resize" },
  { dir: "East", cls: "right-0 top-3 bottom-3 w-1.5", cursor: "ew-resize" },
  { dir: "NorthWest", cls: "top-0 left-0 w-3 h-3", cursor: "nwse-resize" },
  { dir: "NorthEast", cls: "top-0 right-0 w-3 h-3", cursor: "nesw-resize" },
  { dir: "SouthWest", cls: "bottom-0 left-0 w-3 h-3", cursor: "nesw-resize" },
  { dir: "SouthEast", cls: "bottom-0 right-0 w-3 h-3", cursor: "nwse-resize" },
];

export function WindowResizeHandles() {
  const maximized = useMaximized();
  if (maximized) return null;

  return (
    <>
      {EDGES.map((edge) => (
        <div
          key={edge.dir}
          aria-hidden="true"
          className={`fixed z-[70] ${edge.cls}`}
          style={{ cursor: edge.cursor }}
          onMouseDown={(e) => {
            e.preventDefault();
            void appWindow().startResizeDragging(edge.dir);
          }}
        />
      ))}
    </>
  );
}
