import React from "react";

type Props = { children: React.ReactNode };
type State = { error: Error | null; componentStack: string };

/**
 * 界面崩溃兜底（OD-UI-718）：页面渲染或其副作用抛出未捕获异常时，
 * 显示错误文本与重载入口，避免整窗黑屏后无从诊断。
 */
export class ErrorBoundary extends React.Component<Props, State> {
  state: State = { error: null, componentStack: "" };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    this.setState({ componentStack: info.componentStack ?? "" });
    console.error("OmniDesk 界面异常：", error, info);
  }

  render() {
    if (this.state.error === null) return this.props.children;
    const stackLines = (this.state.error.stack ?? "").split("\n").slice(0, 6);
    const firstStack = stackLines.slice(1).join("\n");
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-3 p-8 text-on-surface">
        <p className="text-[13px] font-semibold">界面加载出错，已拦截以免整窗黑屏。</p>
        <pre className="max-h-[40vh] w-full max-w-[760px] overflow-auto rounded-lg bg-surface-container-low p-4 text-[11.5px] leading-relaxed whitespace-pre-wrap">
          {this.state.error.name}: {this.state.error.message}
          {firstStack ? "\n" + firstStack : ""}
          {this.state.componentStack ? "\n组件栈：" + this.state.componentStack.slice(0, 600) : ""}
        </pre>
        <button
          type="button"
          onClick={() => window.location.reload()}
          className="wr-primary-button"
        >
          重新加载界面
        </button>
      </div>
    );
  }
}

