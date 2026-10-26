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
  | "sessions"
  | "knowledge"
  | "settings";

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
    label: "历史会话",
    note: "引擎会话 · 本机对话",
  },
];

/**
 * 知识库是应用内置入口（OD-KB-001 P2-S04），不写入用户 Profile 声明：
 * 声明只描述模型整理的三个业务场景与历史会话，知识库由宿主文件边界直接支撑。
 * 追加在声明入口之后，既有入口顺序与声明语义保持不变。
 */
export const BUILTIN_KNOWLEDGE_ITEM: DockItem = {
  scene: "knowledge",
  icon: "book",
  label: "知识库",
  note: "本机 Markdown",
};

/**
 * Dock 只放业务入口（OD-ARCH-061）：设置与引擎诊断都在顶栏设置页里，
 * 任务工作台已下线，不再有「基础设施」图标占位。
 */

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
  const declaredItems: DockItem[] =
    declared && declared.length > 0
      ? declared.map((s) => ({
          scene: s.id as Scene,
          icon: s.icon as IconName,
          label: s.label,
          note: s.note || undefined,
        }))
      : DEFAULT_BUSINESS_ITEMS;
  // 声明里即使手工写了 knowledge 也不重复；声明校验另有允许场景列表，本条只是兜底。
  const items = declaredItems.some((item) => item.scene === "knowledge")
    ? declaredItems
    : [...declaredItems, BUILTIN_KNOWLEDGE_ITEM];

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
