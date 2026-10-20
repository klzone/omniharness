import { useEffect, useState } from "react";
import { host } from "./api";
import OfflineFlows from "./OfflineFlows";
import SettingsPanel from "./SettingsPanel";
import Sessions from "./Sessions";
import Workbench from "./workbench";
import { Dock, type Scene } from "./shell/Dock";
import { EngineTechSlice } from "./shell/EngineTechSlice";
import { Workspace } from "./shell/Workspace";
import { TopNav } from "./shell/TopNav";
import { WindowResizeHandles } from "./shell/WindowChrome";
import type { EngineDescriptor } from "./types";

/**
 * 应用外壳（OD-ARCH-026）。
 *
 * 结构：悬浮顶栏 + 左侧 Dock + 场景区。
 * - 首页以对话为中枢（HomeChat），左侧 Dock 进入周报与随记、文案写作、项目跟踪。
 * - 三个工作场景复用离线流程页的同一套交互骨架，按入口进入对应场景；
 *   它们的模型任务都走同一条引擎适配链路，代码只做确定性校验与文件边界控制。
 * - 任务工作台仍是脱敏数据分析的纵向试点；引擎诊断是只读覆盖层。
 */
export default function App() {
  const [scene, setScene] = useState<Scene>("home");
  const [showEngine, setShowEngine] = useState(false);
  const [descriptor, setDescriptor] = useState<EngineDescriptor | null>(null);
  const [descriptorError, setDescriptorError] = useState<string | null>(null);

  // 启动时只读检测引擎描述；失败就如实显示“引擎未就绪”，不伪造描述。
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        // 先读端点与授权事实；未获授权时宿主会拒绝启动引擎，不必先拉起进程。
        const admission = await host.admissionStatus();
        if (cancelled) return;
        if (!admission.ok || !admission.facts?.allowed) {
          setDescriptor(null);
          setDescriptorError(
            admission.ok
              ? "模型权限未开启（请到设置中确认端点）"
              : admission.error?.message ?? "无法读取模型端点信息",
          );
          return;
        }
        const d = await host.describe();
        if (!cancelled) {
          setDescriptor(d);
          setDescriptorError(null);
        }
      } catch (e) {
        if (!cancelled) {
          setDescriptor(null);
          setDescriptorError(String(e));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const engineHealthy = descriptor ? descriptor.healthy : null;
  const engineLabel = descriptor
    ? `${descriptor.engine_kind} · ${descriptor.engine_version} · ${descriptor.compatibility}`
    : descriptorError
      ? `引擎未就绪：${descriptorError}`
      : "正在检测引擎…";

  const selectScene = (next: Scene) => {
    if (next === "engine") {
      setShowEngine(true);
      return;
    }
    setScene(next);
  };

  const workSceneTab: Record<string, "weekly" | "writing" | "project"> = {
    report: "weekly",
    writing: "writing",
    project: "project",
  };

  return (
    <div className="relative flex flex-col h-screen w-full bg-background text-on-surface font-body-md overflow-hidden">
      {/* 柔和背景层（纯装饰，不承载信息）。OD-ARCH-027 Step 2：由高半径 filter: blur 改为静态径向渐变光晕。 */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden -z-10">
        <div className="absolute -top-[15%] left-[20%] w-[60rem] h-[50rem] rounded-full ambient-glow ambient-glow-a" />
        <div className="absolute top-[45%] -right-[10%] w-[48rem] h-[48rem] rounded-full ambient-glow ambient-glow-b" />
        <div className="absolute -bottom-[20%] left-[8%] w-[42rem] h-[42rem] rounded-full ambient-glow ambient-glow-c" />
      </div>

      <TopNav
        engineHealthy={engineHealthy}
        engineLabel={engineLabel}
        onOpenSettings={() => setScene("settings")}
      />

      {/* frameless 窗口的边缘缩放热区（最大化时自动隐藏） */}
      <WindowResizeHandles />

      <Dock scene={scene} onSelect={selectScene} />

      <div className="flex-1 min-h-0 flex flex-col">
        {scene === "home" ? (
          <Workspace onNavigate={selectScene} />
        ) : scene === "sessions" ? (
          <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
            <Sessions />
          </div>
        ) : scene === "settings" ? (
          <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
            <SettingsPanel />
          </div>
        ) : scene === "workbench" ? (
          <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
            <Workbench />
          </div>
        ) : (
          // report / writing / project：同一套工作场景骨架，按入口进入对应场景。
          <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
            <OfflineFlows key={scene} initialTab={workSceneTab[scene]} />
          </div>
        )}
      </div>

      {showEngine ? <EngineTechSlice onClose={() => setShowEngine(false)} /> : null}
    </div>
  );
}
