import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./ErrorBoundary";
// 自托管字体（@fontsource，运行时零外链，符合 CSP）：Inter 做正文/标题，
// IBM Plex Mono 做序号与等宽小标；CJK 走系统字体，不额外下载中文字集。
import "@fontsource-variable/inter/wght.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-600.css";
// 阅读设置可选的中文字体（SIL OFL 1.1，自托管、无运行时外链）：
// 思源黑体/宋体对应 Noto Sans/Serif SC，霞鹜文楷为 LXGW WenKai。
import "@fontsource-variable/noto-sans-sc";
import "@fontsource-variable/noto-serif-sc";
import "lxgw-wenkai-webfont/lxgwwenkai-regular.css";
// 设计系统（Tailwind + 视觉令牌）必须先于旧样式加载。
import "./theme.css";
import "./styles.css";
import "./settings-panel.css";
import "./markdown.css";
import "./reading.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
