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
  | "knowledge"
  | "settings";

interface DockItem {
  scene: Scene;
  icon: IconName;
  label: string;
}

/**
 * 内置默认入口：**只在声明读取失败**时使用（界面另有明确错误提示）。
 * 声明文件不存在不是失败——基础标准下本来就没有业务场景声明（OD-ARCH-106）。
 */
export const DEFAULT_BUSINESS_ITEMS: DockItem[] = [
  { scene: "home", icon: "home", label: "工作台首页" },
  { scene: "report", icon: "note", label: "工作记录" },
  { scene: "writing", icon: "book", label: "文案写作" },
  { scene: "project", icon: "checklist", label: "项目跟踪" },
];

/**
 * 知识库是应用内置入口（OD-KB-001 P2-S04），不写入用户 Profile 声明：
 * 声明只描述模型整理的三个业务场景，知识库由宿主文件边界直接支撑。
 * 追加在声明入口之后，既有入口顺序与声明语义保持不变。
 */
export const BUILTIN_KNOWLEDGE_ITEM: DockItem = {
  scene: "knowledge",
  icon: "book",
  label: "知识库",
};

/**
 * 平台固有入口（OD-ARCH-106）：不受 Profile 声明管辖，任何 Profile 下都在。
 * 基础标准（无场景声明）时 Dock 只有两项：首页、知识库。
 */
export const BUILTIN_PLATFORM_ITEMS: DockItem[] = [
  { scene: "home", icon: "home", label: "工作台首页" },
  BUILTIN_KNOWLEDGE_ITEM,
];

/**
 * Dock 只放业务入口（OD-ARCH-061）：设置与引擎诊断都在顶栏设置页里，
 * 任务工作台已下线，不再有「基础设施」图标占位。
 */

export function Dock({
  scene,
  onSelect,
  declared,
  declFailed = false,
}: {
  scene: Scene;
  onSelect: (scene: Scene) => void;
  /**
   * 声明文件里的业务场景。null/undefined = 当前 Profile 没有场景声明（基础标准），
   * 只显示平台固有入口——这是正常状态，不是错误。
   */
  declared?: ProfileSceneEntry[] | null;
  /** 声明读取失败（真错误）。只有这种情况才回退内置默认入口，并由调用方提示原因。 */
  declFailed?: boolean;
}) {
  const declaredItems: DockItem[] =
    declared && declared.length > 0
      ? declared
          .filter((s) => (s.id as string) !== "sessions")
          .map((s) => ({
            scene: s.id as Scene,
            icon: s.icon as IconName,
            label: s.label,
          }))
      : declFailed
        ? DEFAULT_BUSINESS_ITEMS
        : BUILTIN_PLATFORM_ITEMS;
  // 声明里即使手工写了 knowledge 也不重复；声明校验另有允许场景列表，本条只是兜底。
  const items = declaredItems.some((item) => item.scene === "knowledge")
    ? declaredItems
    : [...declaredItems, BUILTIN_KNOWLEDGE_ITEM];

  return (
    <aside className="fixed left-5 top-1/2 -translate-y-1/2 z-40 hidden md:flex flex-col items-center">
      <div className="neu-rail flex flex-col items-center space-y-2" data-testid="dock-rail">
      {items.map((item) => {
        const active = item.scene === scene;
        return (
          <div key={item.scene} className="relative group flex items-center">
            <button
              onClick={() => onSelect(item.scene)}
              aria-current={active ? "page" : undefined}
              // 用 aria-label 而不是 title：title 会让浏览器再弹一个大号原生提示，
              // 与图标旁边那条小字提示重复（OD-ARCH-044）。可访问名保持不变。
              aria-label={item.label}
              // data-scene 是稳定的自动化定位锚点（OD-ARCH-056）：展示名会随声明改，
              // 内部场景键不会。纯属性，不影响布局与可访问名。
              data-scene={item.scene}
              className={
                "neu-dock-btn w-11 h-11 rounded-full flex items-center justify-center " +
                (active
                  ? "neu-dock-active"
                  : "text-on-surface-variant hover:text-primary")
              }
            >
              <Icon name={item.scene === "home" ? "home" : item.icon} className="w-5 h-5" />
            </button>
            <div className="absolute left-14 px-3 py-1.5 rounded-full bg-inverse-surface/95 text-inverse-on-surface text-[11px] font-medium whitespace-nowrap opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity duration-200 shadow-lg flex items-center space-x-1.5 z-50">
              <span>{item.label}</span>
            </div>
          </div>
        );
      })}
      </div>
    </aside>
  );
}
