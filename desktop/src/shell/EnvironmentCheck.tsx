import { openExternalUrl } from "../api";
import type { EnvironmentDependency, EnvironmentReport } from "../types";

/**
 * 环境检查页（OD-ARCH-134）。
 *
 * 引擎依赖 node 与 python，发布包不带这两样。以前缺依赖时引擎在后台起不来、
 * 用户只看到窗口里说不出话；现在启动先体检，缺什么直接摆出来，并给出安装入口
 * 与两个环境变量（OMNIDESK_NODE / OMNIDESK_PYTHON）的用法。
 */
export function EnvironmentCheck({
  report,
  busy,
  onRecheck,
}: {
  report: EnvironmentReport;
  busy: boolean;
  onRecheck: () => void;
}) {
  return (
    <div
      className="flex-1 min-h-0 overflow-y-auto px-6 md:pl-28 md:pr-12 pb-8 flex items-start justify-center"
      data-testid="environment-check"
    >
      <div className="w-full max-w-2xl mt-6 rounded-2xl bg-surface-container-high px-6 py-5 ring-1 ring-outline-variant/50">
        <p className="text-[15px] font-semibold text-on-surface">引擎环境未就绪</p>
        <p className="mt-1.5 text-[12px] leading-relaxed text-on-surface-variant">
          OmniDesk 的对话与场景都需要本机的 Node.js 与 Python。装好下面缺的组件后点「重新检测」，
          不用重装应用。
        </p>

        <div className="mt-4 space-y-3">
          <DependencyRow item={report.node} />
          <DependencyRow item={report.python} />
        </div>

        <div className="mt-5 flex flex-wrap items-center gap-2">
          <button
            type="button"
            className="rounded-lg bg-primary px-3.5 py-1.5 text-[12px] text-on-primary disabled:opacity-60"
            onClick={onRecheck}
            disabled={busy}
            data-testid="environment-recheck"
          >
            {busy ? "正在检测…" : "重新检测"}
          </button>
          <span className="text-[11.5px] text-outline">
            也可以先退出应用，装好后再打开。
          </span>
        </div>

        <p className="mt-4 border-t border-outline-variant/40 pt-3 text-[11.5px] leading-relaxed text-on-surface-variant">
          安装到了非默认目录时，在启动前设置环境变量即可：
          <code className="mx-1 rounded bg-surface px-1 py-0.5 font-mono text-[11px]">OMNIDESK_NODE</code>
          指向 node 可执行文件，
          <code className="mx-1 rounded bg-surface px-1 py-0.5 font-mono text-[11px]">OMNIDESK_PYTHON</code>
          指向 python 解释器。
        </p>
      </div>
    </div>
  );
}

function DependencyRow({ item }: { item: EnvironmentDependency }) {
  const title = item.name === "node" ? "Node.js" : "Python";
  const state = item.ok ? "就绪" : item.found ? "版本过低" : "未找到";
  return (
    <div className="rounded-xl bg-surface px-4 py-3 ring-1 ring-outline-variant/40">
      <div className="flex items-center gap-2">
        <span
          aria-hidden
          className={`inline-block h-2 w-2 rounded-full ${item.ok ? "bg-primary" : "bg-error"}`}
        />
        <span className="text-[12.5px] font-medium text-on-surface">{title}</span>
        <span className="text-[11.5px] text-outline">需要 {item.requirement}</span>
        <span
          className={`ml-auto text-[11.5px] ${item.ok ? "text-on-surface-variant" : "text-error"}`}
        >
          {state}
          {item.version ? ` · ${item.version}` : ""}
        </span>
      </div>
      {item.path ? (
        <p className="mt-1 truncate font-mono text-[10.5px] text-outline" title={item.path}>
          {item.path}
        </p>
      ) : null}
      {item.ok ? null : (
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <p className="text-[11.5px] leading-relaxed text-on-surface-variant">{item.hint}</p>
          <button
            type="button"
            className="rounded-lg px-2.5 py-1 text-[11.5px] text-primary ring-1 ring-primary/35 hover:bg-primary/5"
            onClick={() => void openExternalUrl(item.download).catch(() => {})}
          >
            打开下载页
          </button>
        </div>
      )}
    </div>
  );
}
