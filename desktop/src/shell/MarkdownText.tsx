import { cloneElement, isValidElement, useState, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";
import { openExternalUrl } from "../api";

/**
 * 消息正文的 Markdown 渲染（OD-ARCH-081 S3 / OD-ARCH-084 S1）。
 *
 * 为什么引入 `react-markdown` + `remark-gfm`：仓库此前没有任何 Markdown 能力，
 * 自研渲染器在表格、嵌套列表、代码块上容易出错。这里只做**纯渲染**：
 * 不启用 `rehype-raw`，模型或用户文本里的 HTML 一律按文字显示，不会被注入执行
 * （全仓库不出现 `dangerouslySetInnerHTML`）。`remark-breaks` 保留单换行，
 * 与原纯文本 `whitespace-pre-wrap` 的观感一致，避免模型写的短句被粘成一段。
 *
 * OD-ARCH-084 S1 补充：支持跨标题、表格单元格、引用与列表的会话内搜索命中分词高亮，
 * 围栏代码块保持原始文本与复制代码能力不变。
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

/**
 * 使用纯字符串 indexOf 逐处切分，彻底避免正则表达式引擎解析元字符 (如 ( [ * . \ 等)
 * 时的语法报错或通配误判，确保任意符号均为严格字面量检索。
 */
function splitLiteralMatches(str: string, query: string): string[] {
  const q = query.trim();
  if (!q) return [str];
  const qLower = q.toLowerCase();
  const sLower = str.toLowerCase();
  const parts: string[] = [];
  let lastIndex = 0;
  let pos = sLower.indexOf(qLower, lastIndex);
  while (pos !== -1) {
    if (pos > lastIndex) {
      parts.push(str.slice(lastIndex, pos));
    }
    parts.push(str.slice(pos, pos + q.length));
    lastIndex = pos + q.length;
    pos = sLower.indexOf(qLower, lastIndex);
  }
  if (lastIndex < str.length) {
    parts.push(str.slice(lastIndex));
  }
  return parts;
}

/**
 * 递归遍历 React 节点树对匹配项生成 <mark className="md-search-hit" data-search-hit="" data-search-ordinal={n}>
 * 跳过围栏代码块，覆盖标题、段落、列表、表格单元格、引用与行内代码。
 */
function highlightTextChildren(
  node: ReactNode,
  query: string,
  nextOrdinal: () => number,
): ReactNode {
  const q = query.trim();
  if (!q) return node;
  if (node === null || node === undefined || typeof node === "boolean") return node;
  if (typeof node === "string" || typeof node === "number") {
    const str = String(node);
    const parts = splitLiteralMatches(str, q);
    if (parts.length <= 1 && parts[0]?.toLowerCase() !== q.toLowerCase()) return node;
    return parts.map((part, i) =>
      part.toLowerCase() === q.toLowerCase() ? (
        <mark
          key={i}
          className="md-search-hit"
          data-search-hit=""
          data-search-ordinal={nextOrdinal()}
        >
          {part}
        </mark>
      ) : (
        part
      ),
    );
  }
  if (Array.isArray(node)) {
    return node.map((item, idx) => (
      <span key={idx}>{highlightTextChildren(item, query, nextOrdinal)}</span>
    ));
  }
  if (isValidElement<{ children?: ReactNode }>(node)) {
    if (node.type === "pre") return node;
    if (node.type === "mark" && (node.props as Record<string, unknown>)["data-search-hit"] !== undefined) return node;
    const props = node.props;
    if (props && "children" in props) {
      return cloneElement(node, {}, highlightTextChildren(props.children, query, nextOrdinal));
    }
  }
  return node;
}

/**
 * @param tone `onPrimary` 用于深底问句气泡：字色与代码/引用底色换成浅色系，
 *   其余场景沿用正文配色。
 * @param highlightQuery OD-ARCH-084 S1 会话内搜索高亮词
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
  let ordinalCounter = 0;
  const nextOrdinal = () => ordinalCounter++;
  const highlight = (node: ReactNode) =>
    highlightQuery ? highlightTextChildren(node, highlightQuery, nextOrdinal) : node;

  return (
    <div className={cls} data-testid="markdown-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkBreaks]}
        components={{
          a: ({ href, children, ...rest }) => {
            if (!isExternal(href)) {
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
          p: ({ children }) => <p>{highlight(children)}</p>,
          li: ({ children }) => <li>{highlight(children)}</li>,
          h1: ({ children }) => <h1>{highlight(children)}</h1>,
          h2: ({ children }) => <h2>{highlight(children)}</h2>,
          h3: ({ children }) => <h3>{highlight(children)}</h3>,
          h4: ({ children }) => <h4>{highlight(children)}</h4>,
          h5: ({ children }) => <h5>{highlight(children)}</h5>,
          h6: ({ children }) => <h6>{highlight(children)}</h6>,
          blockquote: ({ children }) => <blockquote>{highlight(children)}</blockquote>,
          th: ({ children }) => <th>{highlight(children)}</th>,
          td: ({ children }) => <td>{highlight(children)}</td>,
          code: ({ children, className, ...rest }) => (
            <code className={className} {...rest}>
              {children}
            </code>
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
