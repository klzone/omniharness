import { useEffect, useState } from "react";
import { host, shell } from "./api";
import { hydrateThreads } from "./sceneThreads";
import SettingsPanel from "./SettingsPanel";
import { Dock, type Scene } from "./shell/Dock";
import { HomeCanvas } from "./shell/HomeCanvas";
import { ScenePage } from "./shell/ScenePage";
import { Workspace } from "./shell/Workspace";
import { TopNav } from "./shell/TopNav";
import { WindowResizeHandles } from "./shell/WindowChrome";
import type { EngineDescriptor, ProfileDeclPayload } from "./types";

/**
 * 应用外壳（OD-ARCH-026）。
 *
 * 结构：悬浮顶栏 + 左侧 Dock + 场景区。
 * - 首页以对话为中枢（HomeChat），左侧 Dock 进入工作记录、文案写作、项目跟踪。
 * - 三个工作场景复用离线流程页的同一套交互骨架，按入口进入对应场景；
 *   它们的模型任务都走同一条引擎适配链路，代码只做确定性校验与文件边界控制。
 * - 设置只有顶栏一个入口（OD-ARCH-061）；引擎诊断作为设置里的只读分页。
 */
export default function App() {
  const [scene, setScene] = useState<Scene>("home");
  // 线程记录以 OmniDesk 数据根为准（OD-ARCH-045 第 1 步）：启动时先读回，
  // 读完之前场景页不挂载，避免"先用缓存新建、再被数据根覆盖"的错位。
  const [threadsReady, setThreadsReady] = useState(false);
  const [descriptor, setDescriptor] = useState<EngineDescriptor | null>(null);
  const [descriptorError, setDescriptorError] = useState<string | null>(null);
  // 纪检 Profile 声明（OD-ARCH-051 S3）：文件驱动 Dock 上的业务场景入口。
  // 读不到/文件坏掉时用内置默认入口，并把原因显示出来——不静默回退。
  const [decl, setDecl] = useState<ProfileDeclPayload | null>(null);
  const [declError, setDeclError] = useState<string | null>(null);
  // 声明文件在运行期间被改动（OD-ARCH-052 第 2 步）：声明只在启动时读一次，
  // 改动要到下次启动才生效，所以这里只做提示，不做热重载、不自动覆盖。
  const [declDrift, setDeclDrift] = useState<"modified" | "missing" | null>(null);
  // 首次运行引导（OD-ARCH-054 S3）：只在「确实没有任何模型配置」时出现一张卡。
  // 判定依据是宿主的两个只读事实：配置文件不存在、且覆盖文件不存在。
  // 已有配置（包括用户手写的覆盖文件）一律不打扰；读取失败也不猜。
  const [firstRun, setFirstRun] = useState(false);

  // 当前会话内检索状态（OD-ARCH-082 S8）
  const [searchQuery, setSearchQuery] = useState("");
  const [searchCurrentIndex, setSearchCurrentIndex] = useState(0);
  const [searchMatchCount, setSearchMatchCount] = useState(0);

  useEffect(() => {
    let cancelled = false;
    shell
      .profileDecl()
      .then((value) => {
        if (cancelled) return;
        setDecl(value);
        setDeclError(null);
      })
      .catch((e) => {
        if (cancelled) return;
        setDecl(null);
        setDeclError(String(e instanceof Error ? e.message : e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 声明里没有当前业务场景时（例如用户删掉了「文案写作」而正停在该页），
  // 落到声明中的第一个入口。设置页不受声明管辖，
  // 绝不能因为声明晚到就把用户从设置页拽走——实测过这个跳转，判定为缺陷。
  useEffect(() => {
    if (!decl) return;
    const managed = ["home", "report", "writing", "project", "sessions"] as const;
    if (!(managed as readonly string[]).includes(scene)) return;
    const ids = decl.decl.scenes.map((s) => s.id as string);
    if (!ids.includes(scene)) setScene((decl.decl.scenes[0]?.id as Scene) ?? "home");
    // scene 变化不需要重跑：只在声明到达时校准一次。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [decl]);

  // 每 5 秒比一次文件指纹：只读，失败就静默跳过（提示条不能因为读不到文件而反复报警）。
  useEffect(() => {
    if (!decl) {
      setDeclDrift(null);
      return;
    }
    const loadedDigest = decl.content_sha256;
    let cancelled = false;
    const check = async () => {
      try {
        const current = await shell.profileDeclDigest();
        if (cancelled) return;
        setDeclDrift(current === null ? "missing" : current === loadedDigest ? null : "modified");
      } catch {
        /* 读不到就保持上一次的判断，不打扰用户 */
      }
    };
    void check();
    const timer = window.setInterval(() => void check(), 5000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [decl]);

  // 首次运行判定：只读一次，失败就当「不确定」，不弹引导。
  useEffect(() => {
    let cancelled = false;
    shell
      .modelSettingsLoad()
      .then((r) => {
        if (cancelled) return;
        setFirstRun(r.ok && r.settings === null && r.overlay_present === false);
      })
      .catch(() => {
        if (!cancelled) setFirstRun(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 线程记录以数据根为准（OD-ARCH-045 第 1 步）：启动时先读回一次。
  // 读完之前不挂载场景页，避免"先用缓存新建、再被数据根覆盖"的错位。
  useEffect(() => {
    let cancelled = false;
    hydrateThreads()
      .catch(() => {
        /* hydrate 内部已降级到本地缓存，并记录了原因 */
      })
      .finally(() => {
        if (!cancelled) setThreadsReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

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
    setScene(next);
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
        searchQuery={searchQuery}
        onSearchQueryChange={(q: string) => {
          setSearchQuery(q);
          setSearchCurrentIndex(0);
        }}
        searchMatchCount={searchMatchCount}
        searchCurrentIndex={searchCurrentIndex}
        onSearchPrev={() => {
          if (searchMatchCount > 0) {
            setSearchCurrentIndex((prev) => (prev - 1 + searchMatchCount) % searchMatchCount);
          }
        }}
        onSearchNext={() => {
          if (searchMatchCount > 0) {
            setSearchCurrentIndex((prev) => (prev + 1) % searchMatchCount);
          }
        }}
        onSearchClear={() => {
          setSearchQuery("");
          setSearchCurrentIndex(0);
          setSearchMatchCount(0);
        }}
      />

      {/* frameless 窗口的边缘缩放热区（最大化时自动隐藏） */}
      <WindowResizeHandles />

      <Dock scene={scene} onSelect={selectScene} declared={decl?.decl.scenes} />

      {declError ? (
        <div
          role="status"
          className="mx-auto mt-2 max-w-4xl rounded-2xl bg-error-container/70 px-4 py-2.5 text-[11.5px] leading-relaxed text-on-error-container ring-1 ring-error/30"
          title={declError}
        >
          Profile 声明未能加载，当前使用内置默认入口。原因：{declError}
        </div>
      ) : decl && decl.warnings.length > 0 ? (
        <div
          className="mx-auto mt-2 max-w-4xl rounded-2xl bg-surface-container px-4 py-2 text-[11.5px] text-on-surface-variant ring-1 ring-outline-variant/40"
        >
          {decl.warnings.join(" ")}
        </div>
      ) : null}

      {declDrift ? (
        <div
          role="status"
          className="mx-auto mt-2 max-w-4xl rounded-2xl bg-surface-container-high px-4 py-2.5 text-[11.5px] leading-relaxed text-on-surface-variant ring-1 ring-outline-variant/50"
          title={decl?.path}
        >
          {declDrift === "missing"
            ? "Profile 声明文件已被删除，重启应用后会重建默认声明。当前仍按启动时的入口显示。"
            : "Profile 声明已在磁盘上修改，重启应用后生效。当前仍按启动时的入口显示。"}
        </div>
      ) : null}

      {firstRun && scene !== "settings" ? (
        <div className="mx-auto mt-2 max-w-4xl w-[calc(100%-3rem)] rounded-2xl bg-surface-container-high px-4 py-3 ring-1 ring-outline-variant/50">
          <p className="text-[12.5px] font-medium text-on-surface">先配置一个模型端点</p>
          <p className="mt-1 text-[11.5px] leading-relaxed text-on-surface-variant">
            这台机器上还没有模型配置。到设置里填服务商地址、协议与密钥，点「测试连接」确认能通，
            保存后就能在工作记录、文案写作与项目跟进里使用模型。已有配置的机器不会再看到这条提示。
          </p>
          <button
            className="mt-2 rounded-lg bg-primary px-3 py-1.5 text-[11.5px] text-on-primary"
            onClick={() => setScene("settings")}
          >
            去设置
          </button>
        </div>
      ) : null}

      <div className="flex-1 min-h-0 flex flex-col">
        {!threadsReady ? (
          <div className="flex-1 min-h-0 flex items-center justify-center">
            <p className="text-[11.5px] text-outline">正在从本机数据目录恢复对话记录…</p>
          </div>
        ) : scene === "home" ? (
          <HomeCanvas
            onNavigate={selectScene}
            declaredScenes={decl?.decl.scenes ?? null}
            searchQuery={searchQuery}
            searchCurrentIndex={searchCurrentIndex}
            onSearchMatchesChange={(count) => {
              setSearchMatchCount(count);
              setSearchCurrentIndex((prev) => {
                if (count === 0) return 0;
                if (prev >= count) return count - 1;
                if (prev < 0) return 0;
                return prev;
              });
            }}
          />
        ) : scene === "report" || scene === "writing" || scene === "project" ? (
          <ScenePage
            scene={scene === "report" ? "weekly" : scene === "writing" ? "writing" : "project"}
            onNavigate={selectScene}
          />
        ) : scene === "sessions" ? (
          <div className="flex-1 min-h-0 overflow-hidden">
            <Workspace onNavigate={selectScene} />
          </div>
        ) : scene === "settings" ? (
          <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
            <SettingsPanel />
          </div>
        ) : (
          // 兜底：声明里出现了未知入口时回首页，不渲染任何未实现页面。
          <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
            <p className="text-[12px] text-on-surface-variant">
              这个入口还没有对应页面。
              <button className="ml-1 text-primary hover:underline" onClick={() => setScene("home")}>
                回首页
              </button>
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

