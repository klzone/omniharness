import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// 自托管字体（@fontsource，运行时零外链，符合 CSP）：Inter 做正文/标题，
// IBM Plex Mono 做序号与等宽小标；CJK 走系统字体，不额外下载中文字集。
import "@fontsource-variable/inter/wght.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-600.css";
// 设计系统（Tailwind + 视觉令牌）必须先于旧样式加载。
import "./theme.css";
import "./styles.css";
import "./settings-panel.css";
import "./markdown.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
