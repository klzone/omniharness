import { Icon, type IconName } from "../components/Icon";
import type { ProfileSceneEntry } from "../types";

/**
 * 左侧 Dock（OD-ARCH-026）：列出已实现的入口。
 *
 * 首页对话是主要工作入口；「工作记录 / 文案写作 / 项目跟踪」三个场景由模型整理与起草，
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

/** 内置默认的业务入口：声明文件缺失或读取失败时使用（界面另有明确提示）。 */
export const DEFAULT_BUSINESS_ITEMS: DockItem[] = [
  { scene: "home", icon: "brand", label: "工作台首页" },
  { scene: "report", icon: "note", label: "工作记录", note: "模型起草" },
  { scene: "writing", icon: "book", label: "文案写作", note: "模型起草" },
  { scene: "project", icon: "checklist", label: "项目跟踪", note: "模型起草" },
  {
    scene: "sessions",
    icon: "history",
    label: "项目与会话",
    note: "引擎项目 · 对话",
  },
];

/**
 * 基础设施入口：不属于纪检业务场景，由代码固定，声明文件不能删改。
 * 声明只决定上面那组业务场景显示什么、按什么顺序显示。
 */
const BUILTIN_TOOL_ITEMS: DockItem[] = [
  {
    scene: "workbench",
    icon: "terminal",
    label: "任务工作台",
    note: "合成数据 · 五阶段",
  },
  { scene: "settings", icon: "folder", label: "成果目录与模型权限" },
  { scene: "engine", icon: "settings", label: "引擎诊断", note: "只读" },
];

export function Dock({
  scene,
  onSelect,
  declared,
}: {
  scene: Scene;
  onSelect: (scene: Scene) => void;
  /** 声明文件里的业务场景；null/undefined 表示用内置默认（调用方负责提示原因）。 */
  declared?: ProfileSceneEntry[] | null;
}) {
  const business: DockItem[] =
    declared && declared.length > 0
      ? declared.map((s) => ({
          scene: s.id as Scene,
          icon: s.icon as IconName,
          label: s.label,
          note: s.note || undefined,
        }))
      : DEFAULT_BUSINESS_ITEMS;
  const items = [...business, ...BUILTIN_TOOL_ITEMS];

  return (
    <aside className="fixed left-5 top-1/2 -translate-y-1/2 z-40 hidden md:flex flex-col items-center bg-surface-container-lowest/95 ring-1 ring-white/95 rounded-full py-3 px-2 shadow-[0_16px_40px_-8px_rgba(79,70,229,0.12),0_4px_16px_rgba(0,0,0,0.04)] space-y-2.5">
      {items.map((item) => {
        const active = item.scene === scene;
        return (
          <div key={item.scene} className="relative group flex items-center">
            <button
              onClick={() => onSelect(item.scene)}
              aria-current={active ? "page" : undefined}
              // 用 aria-label 而不是 title：title 会让浏览器再弹一个大号原生提示，
              // 与图标旁边那条小字提示重复（OD-ARCH-044）。可访问名保持不变。
              aria-label={item.note ? `${item.label}（${item.note}）` : item.label}
              // data-scene 是稳定的自动化定位锚点（OD-ARCH-056）：展示名会随声明改，
              // 内部场景键不会。纯属性，不影响布局与可访问名。
              data-scene={item.scene}
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
