import { useEffect, useRef, useState } from "react";
import { Icon } from "../components/Icon";

/**
 * 对话分类选择器（OD-ARCH-063 S1）：单枚下拉胶囊，替代原先平铺的 chip 行。
 *
 * 为什么不继续平铺：分类可以新增，平铺会随数量吃掉整行宽度、把输入区挤变形。
 * 视觉与 `ModelPicker` 同套（plate-soft-inset 胶囊 + 自绘浮层），不引第三方组件库。
 *
 * 交互：点胶囊展开；列表项单击即选中并收起；Esc 收起；上下键移动、Enter 选中。
 * 归类语义完全复用外部 `onChangeTag`（既有 retag/归档链路），本组件不碰持久化。
 */
export interface CategoryOption {
  key: string;
  label: string;
}

/** 未分类用中性灰；内置三类各有一色；自定义分类按 key 稳定取色（同一 key 永远同色）。 */
const KNOWN_COLORS: Record<string, string> = {
  plain: "#94a3b8",
  weekly: "#4f46e5",
  writing: "#0e7490",
  project: "#b45309",
};
const PALETTE = ["#7c3aed", "#059669", "#db2777", "#ea580c", "#0284c7", "#65a30d"];

/** 分类色。只用于展示识别，不写进数据、不影响归档。 */
export function categoryColor(key: string): string {
  const known = KNOWN_COLORS[key];
  if (known) return known;
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) % 100003;
  return PALETTE[h % PALETTE.length];
}

export function CategoryPicker({
  options,
  value,
  onChange,
  disabled = false,
}: {
  options: CategoryOption[];
  value: string;
  onChange: (key: string) => void;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const current = options.find((o) => o.key === value);
  const currentLabel = current?.label ?? (value ? value : "未分类");
  const currentIndex = Math.max(0, options.findIndex((o) => o.key === value));

  // 点外部关闭：与 ModelPicker 同一套做法。
  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  // 打开后把焦点放到当前项，键盘用户可以直接上下移动。
  useEffect(() => {
    if (!open) return;
    const el = itemRefs.current[currentIndex];
    // preventScroll：浮层向上展开，聚焦列表项不该带动整页滚动。
    el?.focus({ preventScroll: true });
  }, [open, currentIndex]);

  const close = (refocus: boolean) => {
    setOpen(false);
    // preventScroll：不然焦点回到按钮时浏览器会把整个对话区滚一下，看起来像页面乱跳。
    if (refocus) buttonRef.current?.focus({ preventScroll: true });
  };

  const move = (delta: number) => {
    if (options.length === 0) return;
    // 从当前焦点项算，避免列表顺序与选中项不一致时跳错。
    const focused = itemRefs.current.findIndex((el) => el === document.activeElement);
    const from = focused >= 0 ? focused : currentIndex;
    const next = (from + delta + options.length) % options.length;
    itemRefs.current[next]?.focus({ preventScroll: true });
  };

  return (
    <div className="relative" ref={rootRef}>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setOpen((v) => !v)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            setOpen(true);
          } else if (e.key === "Escape" && open) {
            e.preventDefault();
            close(true);
          }
        }}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`本次对话归到哪一类，当前：${currentLabel}`}
        data-testid="scene-category-picker"
        data-tag={value}
        className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-full plate-soft-inset text-on-surface-variant font-label-caps text-[12px] hover:bg-surface-container transition-colors disabled:opacity-60"
        title="归类只改这段对话的组织方式，不改模型与路由；可选分类在设置页维护"
      >
        <span
          className="w-[7px] h-[7px] rounded-full flex-shrink-0 ring-1 ring-black/5"
          style={{ backgroundColor: categoryColor(value) }}
          aria-hidden="true"
        />
        <span className="max-w-[96px] truncate">{currentLabel}</span>
        <Icon
          name="chevron-down"
          className={"w-[12px] h-[12px] opacity-70 transition-transform " + (open ? "rotate-180" : "")}
        />
      </button>

      {open ? (
        <div
          className="absolute left-0 bottom-full mb-2 w-[240px] rounded-2xl bg-white/95 backdrop-blur-sm ring-1 ring-outline-variant/30 shadow-[0_18px_48px_-12px_rgba(15,23,42,0.25)] p-2 z-50"
          data-testid="scene-chips"
        >
          <div className="px-1.5 pb-1.5 text-[12px] uppercase tracking-wide text-outline">
            归类到
          </div>
          <ul
            role="listbox"
            aria-label="本次对话归到哪一类"
            className="max-h-[240px] overflow-y-auto"
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                close(true);
              } else if (e.key === "ArrowDown") {
                e.preventDefault();
                move(1);
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                move(-1);
              }
            }}
          >
            {options.map((o, i) => {
              const on = o.key === value;
              return (
                <li key={o.key} role="none">
                  <button
                    ref={(el) => {
                      itemRefs.current[i] = el;
                    }}
                    role="option"
                    aria-selected={on}
                    data-testid={`scene-chip-${o.key}`}
                    data-tag={o.key}
                    onClick={() => {
                      onChange(o.key);
                      close(true);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        onChange(o.key);
                        close(true);
                      } else if (e.key === "Tab") {
                        // Tab 交给浏览器：浮层不应把焦点困住。
                        setOpen(false);
                      }
                    }}
                    className={
                      "w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left text-[12px] transition-colors " +
                      (on
                        ? "bg-primary/10 text-primary font-medium"
                        : "text-on-surface-variant hover:bg-surface-container")
                    }
                    title={
                      on
                        ? "这段对话当前归到这一类"
                        : "归到这一类后，这段对话按该场景的规则组织，成果也归到该场景的档案里"
                    }
                  >
                    <span
                      className="w-[7px] h-[7px] rounded-full flex-shrink-0 ring-1 ring-black/5"
                      style={{ backgroundColor: categoryColor(o.key) }}
                      aria-hidden="true"
                    />
                    <span className="flex-1 truncate">{o.label}</span>
                    {on ? <Icon name="check" className="w-[13px] h-[13px] flex-shrink-0" /> : null}
                  </button>
                </li>
              );
            })}
          </ul>
          <p className="mt-1.5 pt-1.5 border-t border-surface-variant/20 px-1.5 text-[12px] text-outline leading-relaxed">
            分类可在设置里改名或新增；这里只决定这段对话怎么归类。
          </p>
        </div>
      ) : null}
    </div>
  );
}
