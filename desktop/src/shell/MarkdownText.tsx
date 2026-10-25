import { isValidElement, useState, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";
import { openExternalUrl } from "../api";

/**
 * 消息正文的 Markdown 渲染（OD-ARCH-081 S3）。
 *
 * 为什么引入 `react-markdown` + `remark-gfm`：仓库此前没有任何 Markdown 能力，
 * 自研渲染器在表格、嵌套列表、代码块上容易出错。这里只做**纯渲染**：
 * 不启用 `rehype-raw`，模型或用户文本里的 HTML 一律按文字显示，不会被注入执行
 * （全仓库不出现 `dangerouslySetInnerHTML`）。`remark-breaks` 保留单换行，
 * 与原纯文本 `whitespace-pre-wrap` 的观感一致，避免模型写的短句被粘成一段。
 */

/** 把 React 子节点里的文字取出来（代码块复制用，不依赖 DOM 查询）。 */
function textOf(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (isValidElement<{ children?: ReactNode }>(node)) return textOf(node.props.children);
  return "";
}

function CopyCodeButton({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      onClick={() => {
        void (async () => {
          try {
            await navigator.clipboard.writeText(text);
            setDone(true);
          } catch {
            setDone(false);
          }
          window.setTimeout(() => setDone(false), 1500);
        })();
      }}
      className="md-code-copy"
      title={done ? "已复制" : "复制代码"}
      aria-label={done ? "已复制" : "复制代码"}
    >
      {done ? "已复制" : "复制"}
    </button>
  );
}

function isExternal(href: string | undefined): boolean {
  return !!href && /^https?:\/\//i.test(href);
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^$\{\}()|[\]\\]/g, "\$&");
}

function highlightTextChildren(node: ReactNode, query: string): ReactNode {
  if (!query.trim()) return node;
  if (typeof node === "string") {
    const escaped = escapeRegex(query.trim());
    const regex = new RegExp(`(${escaped})`, "gi");
    const parts = node.split(regex);
    if (parts.length <= 1) return node;
    return parts.map((part, i) =>
      part.toLowerCase() === query.trim().toLowerCase() ? (
        <mark key={i} className="bg-amber-300 text-stone-900 rounded-xs px-0.5 font-medium">
          {part}
        </mark>
      ) : (
        part
      ),
    );
  }
  if (Array.isArray(node)) {
    return node.map((item, idx) => (
      <span key={idx}>{highlightTextChildren(item, query)}</span>
    ));
  }
  return node;
}

/**
 * @param tone `onPrimary` 用于深底问句气泡：字色与代码/引用底色换成浅色系，
 *   其余场景沿用正文配色。
 * @param highlightQuery OD-ARCH-082 S8 当前会话搜索高亮词
 */
export function MarkdownText({
  text,
  tone = "default",
  highlightQuery,
}: {
  text: string;
  tone?: "default" | "onPrimary";
  highlightQuery?: string;
}) {
  const cls = tone === "onPrimary" ? "md-body md-compact md-on-primary" : "md-body";
  return (
    <div className={cls} data-testid="markdown-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkBreaks]}
        components={{
          a: ({ href, children, ...rest }) => {
            if (!isExternal(href)) {
              // 相对路径 / 锚点不交给 WebView 导航：点一下会整窗跳走，后果不可控。
              return (
                <span className="md-link-inert" title={href ?? ""}>
                  {children}
                </span>
              );
            }
            return (
              <a
                {...rest}
                href={href}
                title={href}
                onClick={(event) => {
                  event.preventDefault();
                  void openExternalUrl(href as string).catch(() => {});
                }}
              >
                {children}
              </a>
            );
          },
          pre: ({ children }) => (
            <div className="md-pre-wrap">
              <CopyCodeButton text={textOf(children)} />
              <pre>{children}</pre>
            </div>
          ),
          p: ({ children }) => (
            <p>{highlightQuery ? highlightTextChildren(children, highlightQuery) : children}</p>
          ),
          li: ({ children }) => (
            <li>{highlightQuery ? highlightTextChildren(children, highlightQuery) : children}</li>
          ),
          table: ({ children }) => (
            <div className="md-table-scroll">
              <table>{children}</table>
            </div>
          ),
          input: ({ type, checked }) =>
            type === "checkbox" ? <input type="checkbox" checked={!!checked} disabled readOnly /> : null,
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
