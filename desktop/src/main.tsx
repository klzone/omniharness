import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// 自托管字体（@fontsource，运行时零外链，符合 CSP）；仅 latin 子集，CJK 走系统字体。
import "@fontsource/sora/latin-300.css";
import "@fontsource/sora/latin-400.css";
import "@fontsource/sora/latin-500.css";
import "@fontsource/sora/latin-600.css";
import "@fontsource/sora/latin-700.css";
import "@fontsource/plus-jakarta-sans/latin-300.css";
import "@fontsource/plus-jakarta-sans/latin-400.css";
import "@fontsource/plus-jakarta-sans/latin-500.css";
import "@fontsource/plus-jakarta-sans/latin-600.css";
import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-500.css";
import "@fontsource/jetbrains-mono/latin-600.css";
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
