import { Suspense, lazy, useCallback, useEffect, useState } from "react";
import { host, onProfileDeclSaved, shell } from "./api";
import { hydrateThreads } from "./sceneThreads";
import { refreshThreadCategories } from "./threadCategories";
import SettingsPanel from "./SettingsPanel";
import { Dock, type Scene } from "./shell/Dock";
import { HomeCanvas } from "./shell/HomeCanvas";
import { ConfirmDialog } from "./shell/ConfirmDialog";
import { ScenePage } from "./shell/ScenePage";
import { WorkRecordPage } from "./shell/WorkRecordPage";
import { TopNav } from "./shell/TopNav";
import { EnvironmentCheck } from "./shell/EnvironmentCheck";
import { WindowResizeHandles } from "./shell/WindowChrome";
import { HoverHint } from "./shell/HoverHint";
import type { EngineDescriptor, EnvironmentReport, ProfileDeclPayload } from "./types";

// 知识库分页只在用户第一次点进来时才加载（OD-KB-001 P2-S04）：它是目前最大的
// 单个页面模块，静态引入会让每次启动都多解析一遍。首次进入时才拉取，不进首屏。
const KnowledgeVaultPage = lazy(() =>
  import("./shell/KnowledgeVaultPage").then((m) => ({ default: m.KnowledgeVaultPage })),
);

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
  // 运行时依赖体检（OD-ARCH-134）：缺 node / python 时先给出可操作的检查页，
  // 而不是让引擎在后台起不来、用户只看到「终端一闪而过」。体检失败本身不阻断
  // 界面（读设置、看记录都不需要引擎），只有明确缺依赖时才接管主区域。
  const [envReport, setEnvReport] = useState<EnvironmentReport | null>(null);
  const [envBusy, setEnvBusy] = useState(false);
  // Profile 场景声明（OD-ARCH-051 S3，OD-ARCH-106 起随 Profile 挂载）：
  // `profiles/<Profile 名>/decl.yml` 驱动 Dock 上的业务场景入口。
  // **没有这份文件是正常状态**（基础标准 = 干净 DSH 底座），此时 Dock 只显示平台固有入口；
  // 只有读取/校验真正失败时才算错误，才回退内置默认入口并把原因显示出来。
  const [decl, setDecl] = useState<ProfileDeclPayload | null>(null);
  const [declError, setDeclError] = useState<string | null>(null);
  /** 声明是否读完了。用来区分「正在读」与「确实没有声明」。 */
  const [declReady, setDeclReady] = useState(false);
  // 声明文件在运行期间被**应用外部**改动（OD-ARCH-052 第 2 步）：只提示，不热重载、
  // 不自动覆盖。应用内保存走 omni://profile-decl 事件热重载（OD-ARCH-123），两条路分开。
  const [declDrift, setDeclDrift] = useState<"modified" | "missing" | null>(null);
  /** 切换 Profile 的计数（OD-ARCH-110）：设置页靠它清掉旧 Profile 的缓存。 */
  const [profileEpoch, setProfileEpoch] = useState(0);
  // 首次运行引导（OD-ARCH-054 S3）：只在「确实没有任何模型配置」时出现一张卡。
  // 判定依据是宿主的两个只读事实：配置文件不存在、且覆盖文件不存在。
  // 已有配置（包括用户手写的覆盖文件）一律不打扰；读取失败也不猜。
  const [firstRun, setFirstRun] = useState(false);

  // 当前会话内检索状态（OD-ARCH-082 S8）
  const [searchQuery, setSearchQuery] = useState("");
  const [searchCurrentIndex, setSearchCurrentIndex] = useState(0);
  const [searchMatchCount, setSearchMatchCount] = useState(0);
  // 知识库的草稿只存在于分页组件内存里（OD-KB-001 P2-S04）：
  // 离开分页会丢弃它，因此切换入口前必须先明确确认。
  const [knowledgeDirty, setKnowledgeDirty] = useState(false);
  // 离开知识库前必须确认；用应用内对话框而不是 window.confirm（实测不阻塞）。
  const [pendingScene, setPendingScene] = useState<Scene | null>(null);

  /**
   * 重读场景声明（OD-ARCH-110）：切换 Profile 后必须立刻重建 Dock。
   * 返回取消函数，供 useEffect 在卸载／重跑时丢弃过期结果。
   */
  const loadDecl = useCallback(() => {
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
      })
      .finally(() => {
        if (!cancelled) setDeclReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => loadDecl(), [loadDecl]);

  /**
   * 用户在设置页保存了场景声明（OD-ARCH-123）：立刻重读，Dock 换成新入口。
   *
   * 只处理「应用内保存」这一条路径；外部手改文件仍走下面的指纹提示、不热重载
   * （OD-ARCH-052 第 2 步的边界：磁盘上是什么与界面是什么不能悄悄脱钩）。
   */
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void onProfileDeclSaved(() => {
      if (disposed) return;
      // 用户自己刚保存，不该同时亮「文件被改动」的提示。
      setDeclDrift(null);
      loadDecl();
    })
      .then((fn: () => void) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {
        /* 订阅失败不影响手动「重新读取」；保存回执本身已经说明结果 */
      });
    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, [loadDecl]);

  // 声明里没有当前业务场景时（例如用户删掉了「文案写作」而正停在该页，
  // 或切回了基础标准这种没有场景声明的 Profile），落到首页。设置页与知识库不受声明管辖，
  // 绝不能因为声明晚到就把用户从设置页拽走——实测过这个跳转，判定为缺陷。
  useEffect(() => {
    if (!declReady) return;
    const managed = ["home", "report", "writing", "project"] as const;
    if (!(managed as readonly string[]).includes(scene)) return;
    const ids = decl ? decl.decl.scenes.map((s) => s.id as string) : [];
    if (!ids.includes(scene)) setScene((decl?.decl.scenes[0]?.id as Scene) ?? "home");
    // scene 变化不需要重跑：只在声明到达时校准一次。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [decl, declReady]);

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

  /** 只读体检 node / python（OD-ARCH-134）；失败按「不确定」处理，不拦住界面。 */
  const checkEnvironment = useCallback(() => {
    setEnvBusy(true);
    void host
      .environmentCheck()
      .then((report) => setEnvReport(report))
      .catch(() => setEnvReport(null))
      .finally(() => setEnvBusy(false));
  }, []);

  useEffect(() => {
    checkEnvironment();
  }, [checkEnvironment]);

  /** 只读检测引擎描述；失败就如实显示“引擎未就绪”，不伪造描述。 */
  const probeEngine = useCallback(() => {
    let cancelled = false;
    void (async () => {
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

  useEffect(() => probeEngine(), [probeEngine]);



  const engineHealthy = descriptor ? descriptor.healthy : null;
  const engineLabel = descriptor
    ? `${descriptor.engine_kind} · ${descriptor.engine_version} · ${descriptor.compatibility}`
    : descriptorError
      ? `引擎未就绪：${descriptorError}`
      : "正在检测引擎…";

  /** 引擎未就绪时轻量轮询（健康快照），就绪即停：IPC 徽标不能只在启动时探一次，
   *  否则权限后开、引擎冷启动完成、Profile 切换后恢复都永远显示 OFF。 */
  useEffect(() => {
    if (engineHealthy) return undefined;
    const timer = window.setInterval(() => {
      void (async () => {
        try {
          const admission = await host.admissionStatus();
          if (!admission.ok || !admission.facts?.allowed) return;
          const d = await host.describe();
          setDescriptor(d);
          setDescriptorError(null);
        } catch {
          /* 仍未就绪：下一轮继续探 */
        }
      })();
    }, 5000);
    return () => window.clearInterval(timer);
  }, [engineHealthy]);

  const selectScene = (next: Scene) => {
    if (scene === "knowledge" && next !== "knowledge" && knowledgeDirty) {
      setPendingScene(next);
      return;
    }
    setScene(next);
  };

  return (
    <div className="relative flex flex-col h-screen w-full bg-background text-on-surface font-body-md overflow-hidden">
      {/* 全局悬停说明（OD-ARCH-101）：接管 window 的原生 title，统一为方角白底卡片。 */}
      <HoverHint />
      {/* 柔和背景层（纯装饰，不承载信息）。OD-ARCH-027 Step 2：由高半径 filter: blur 改为静态径向渐变光晕。 */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden -z-10">
        <div className="absolute -top-[15%] left-[20%] w-[60rem] h-[50rem] rounded-full ambient-glow ambient-glow-a" />
        <div className="absolute top-[45%] -right-[10%] w-[48rem] h-[48rem] rounded-full ambient-glow ambient-glow-b" />
        <div className="absolute -bottom-[20%] left-[8%] w-[42rem] h-[42rem] rounded-full ambient-glow ambient-glow-c" />
      </div>

      <TopNav
        engineHealthy={engineHealthy}
        engineLabel={engineLabel}
        onOpenSettings={() => selectScene("settings")}
        onProfileChanged={() => {
          // 切换 Profile 已停旧引擎并换规则：声明必须重读，Dock 立刻换成新工作台。
          // 不强行跳页：设置页与知识库不受声明管辖，若当前场景在新 Profile 里不存在，
          // 下面那个校准 effect 会自己落回首页。
          loadDecl();
          setProfileEpoch((n) => n + 1);
          // 分类表也随 Profile 走（OD-ARCH-116）：不重读的话，首页胶囊与场景对话
          // 会继续显示上一个 Profile 的业务分类，切回基础标准后尤其明显。
          void refreshThreadCategories();
          // 旧引擎已停：重新检测一次描述，顶栏不再显示上一个 Profile 的引擎状态。
          probeEngine();
        }}
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

      <Dock
        scene={scene}
        onSelect={selectScene}
        declared={decl?.decl.scenes ?? null}
        declFailed={Boolean(declError)}
      />

      {declError ? (
        <div
          role="status"
          className="mx-auto mt-2 max-w-4xl rounded-2xl bg-error-container/70 px-4 py-2.5 text-[11.5px] leading-relaxed text-on-error-container ring-1 ring-error/30"
          data-hover-hint={declError}
        >
          Profile 声明未能加载，当前使用内置默认入口。原因：{declError}
        </div>
      ) : null}

      {declDrift ? (
        <div
          role="status"
          className="mx-auto mt-2 max-w-4xl rounded-2xl bg-surface-container-high px-4 py-2.5 text-[11.5px] leading-relaxed text-on-surface-variant ring-1 ring-outline-variant/50"
          data-hover-hint={decl?.path}
        >
          {declDrift === "missing"
            ? "Profile 声明文件已在应用外部被删除。当前仍按已载入的入口显示；重启应用后将只显示平台固有入口（不会再自动生成声明）。"
            : "Profile 声明已在应用外部被修改，当前仍按已载入的入口显示。要应用改动，到设置页重新保存一次或切换 Profile。"}
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
            onClick={() => selectScene("settings")}
          >
            去设置
          </button>
        </div>
      ) : null}

      <div className="flex-1 min-h-0 flex flex-col">
        {envReport && !envReport.ok ? (
          <EnvironmentCheck
            report={envReport}
            busy={envBusy}
            onRecheck={checkEnvironment}
          />
        ) : !threadsReady ? (
          <div className="flex-1 min-h-0 flex items-center justify-center">
            <p className="text-[11.5px] text-outline">正在从本机数据目录恢复对话记录…</p>
          </div>
        ) : scene === "home" ? (
          <HomeCanvas
            onNavigate={selectScene}
            declaredScenes={decl?.decl.scenes ?? null}
            declFailed={Boolean(declError)}
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
        ) : scene === "report" ? (
          <WorkRecordPage onNavigate={selectScene} />
        ) : scene === "writing" || scene === "project" ? (
          <ScenePage
            scene={scene === "writing" ? "writing" : "project"}
            onNavigate={selectScene}
          />
        ) : scene === "knowledge" ? (
          <Suspense
            fallback={
              <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
                <p className="text-[12px] text-on-surface-variant">正在打开知识库…</p>
              </div>
            }
          >
            <KnowledgeVaultPage
              onOpenSettings={() => selectScene("settings")}
              onDirtyChange={setKnowledgeDirty}
            />
          </Suspense>
        ) : scene === "settings" ? (
          <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
            <SettingsPanel profileEpoch={profileEpoch} />
          </div>
        ) : (
          // 兜底：声明里出现了未知入口时回首页，不渲染任何未实现页面。
          <div className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-6">
            <p className="text-[12px] text-on-surface-variant">
              这个入口还没有对应页面。
              <button className="ml-1 text-primary hover:underline" onClick={() => selectScene("home")}>
                回首页
              </button>
            </p>
          </div>
        )}
      </div>

      <ConfirmDialog
        open={pendingScene !== null}
        title="离开知识库？"
        description="知识库里有未保存的修改。离开这个页面会丢弃草稿，但不会改动磁盘文件。"
        confirmLabel="离开并丢弃"
        onCancel={() => setPendingScene(null)}
        onConfirm={() => {
          const next = pendingScene;
          setPendingScene(null);
          setKnowledgeDirty(false);
          if (next) setScene(next);
        }}
        testId="knowledge-leave-confirm"
      />
    </div>
  );
}

