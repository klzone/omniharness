import { Icon, type IconName } from "../components/Icon";

/**
 * 左侧 Dock（OD-ARCH-026）：列出已实现的入口。
 *
 * 首页对话是主要工作入口；「周报与随记 / 文案写作 / 项目跟踪」三个场景由模型整理与起草，
 * 代码负责校验、任务状态与文件读写。其余未实现的业务入口一律不放在这里。
 */
export type Scene =
  | "home"
  | "report"
  | "writing"
  | "project"
  | "workbench"
  | "sessions"
  | "settings"
  | "engine";

interface DockItem {
  scene: Scene;
  icon: IconName;
  label: string;
  note?: string;
}

const ITEMS: DockItem[] = [
  { scene: "home", icon: "brand", label: "工作台首页" },
  { scene: "report", icon: "note", label: "周报与随记", note: "模型起草" },
  { scene: "writing", icon: "book", label: "文案写作", note: "模型起草" },
  { scene: "project", icon: "checklist", label: "项目跟踪", note: "模型起草" },
  {
    scene: "workbench",
    icon: "terminal",
    label: "任务工作台",
    note: "合成数据 · 五阶段",
  },
  {
    scene: "sessions",
    icon: "history",
    label: "会话与任务",
    note: "只读元数据",
  },
  { scene: "settings", icon: "folder", label: "成果目录与模型权限" },
  { scene: "engine", icon: "settings", label: "引擎诊断", note: "只读" },
];

export function Dock({
  scene,
  onSelect,
}: {
  scene: Scene;
  onSelect: (scene: Scene) => void;
}) {
  return (
    <aside className="fixed left-5 top-1/2 -translate-y-1/2 z-40 hidden md:flex flex-col items-center bg-surface-container-lowest/95 ring-1 ring-white/95 rounded-full py-3 px-2 shadow-[0_16px_40px_-8px_rgba(79,70,229,0.12),0_4px_16px_rgba(0,0,0,0.04)] space-y-2.5">
      {ITEMS.map((item) => {
        const active = item.scene === scene;
        return (
          <div key={item.scene} className="relative group flex items-center">
            <button
              onClick={() => onSelect(item.scene)}
              aria-current={active ? "page" : undefined}
              title={item.note ? `${item.label}（${item.note}）` : item.label}
              className={
                "w-10 h-10 rounded-full flex items-center justify-center transition-all " +
                (active
                  ? "bg-primary text-on-primary shadow-md"
                  : "text-on-surface-variant hover:text-primary hover:bg-surface-container active:scale-90")
              }
            >
              <Icon name={item.icon} className="w-5 h-5" />
            </button>
            <div className="absolute left-14 px-3 py-1.5 rounded-full bg-inverse-surface/95 text-inverse-on-surface text-[11px] font-medium whitespace-nowrap opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity duration-200 shadow-lg flex items-center space-x-1.5 z-50">
              <span>{item.label}</span>
              {item.note ? (
                <span className="font-label-caps text-[9px] opacity-70">
                  · {item.note}
                </span>
              ) : null}
            </div>
          </div>
        );
      })}
    </aside>
  );
}
