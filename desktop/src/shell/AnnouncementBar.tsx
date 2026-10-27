import { useEffect, useRef } from "react";
import { scheduler } from "../api";

/**
 * 场景心跳桥接（OD-ARCH-033 v3；OD-ARCH-046 心跳；OD-ARCH-095）。
 *
 * 周报页不再显示说明横条。这个组件只保留宿主调度心跳与待确认状态刷新，
 * 避免展示层变化影响既有写入安全链路。
 *
 * OD-ARCH-092：用户裁定移除自动复盘的界面入口（开关 / 频率 / 立即复盘 / 建议行）。
 * 理由是复盘与「随手记落盘 → 明确说整理周报才生成周报」的新链路重复，且默认关闭、
 * 从未启用。宿主侧的 `scheduler.rs` / `pending_writes.rs` 字段与单测全部保留，
 * 这里只是不再提供触发入口；心跳仍是纯本地扫描。
 *
 * 定时由宿主的调度配置判定（宿主命令 scheduler_tick），前端运行期定时器只负责问
 * “到点了吗”。应用关闭期间不执行；错过则下次打开补跑一次。
 */

export function AnnouncementBar({ onPendingChanged }: { onPendingChanged?: () => void }) {
  // 定时与心跳：前端运行期定时器只向宿主问“到点了吗”。
  // 应用关闭期间不执行；错过由 last_*_at 判定，下一次打开补跑一次。
  const runningRef = useRef(false);
  useEffect(() => {
    let alive = true;
    const tick = async () => {
      if (!alive || runningRef.current) return;
      runningRef.current = true;
      try {
        const t = await scheduler.tick();
        if (!alive) return;
        if (t.heartbeat_due) {
          // 心跳仍只做本地调度记录，不调用模型、不写成果文件。
          await scheduler.markHeartbeat();
        }
        // 每轮 tick 顺带刷新待确认列表。
        onPendingChanged?.();
      } catch {
        // 调度不可用时不再渲染一条错误横幅；下一轮 tick 会继续尝试。
      } finally {
        runningRef.current = false;
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), 30000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [onPendingChanged]);

  return null;
}
