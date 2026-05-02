import { Fragment, type ReactNode, useMemo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { highlightTerms, tokenizeQuery } from "@/lib/highlight";

interface Props {
  content: string;
  /**
   * P34 Task 02 — 검색어. 비면 하이라이트 비활성화.
   * SessionDetailRoute가 SessionsRoute outlet context의 query를 prop으로 전달.
   */
  query?: string;
  className?: string;
}

/**
 * 세션 본문 마크다운 렌더러. GFM (테이블/체크박스/취소선) 지원.
 * 코드블록 syntax highlighting은 P35+ (현재는 prose의 기본 pre/code 스타일).
 *
 * P34 Task 02 — query가 있으면 본문 안의 매칭 토큰에 `<mark>`를 적용.
 * react-markdown components override는 `p / li / code` 의 children에서만 동작.
 * heading / link 안 매칭은 acceptable한 누락 (Risks 참조).
 */
export function MarkdownView({ content, query, className }: Props) {
  const terms = useMemo(() => tokenizeQuery(query ?? ""), [query]);
  const components = useMemo<Components | undefined>(() => {
    if (terms.length === 0) return undefined;
    return {
      p: ({ children }) => <p>{wrapChildren(children, terms)}</p>,
      li: ({ children }) => <li>{wrapChildren(children, terms)}</li>,
      code: ({ children, ...rest }) => (
        <code {...rest}>{wrapChildren(children, terms)}</code>
      ),
    };
  }, [terms]);

  return (
    <div
      className={`prose prose-invert prose-sm max-w-none prose-pre:bg-muted prose-pre:text-foreground prose-code:before:content-none prose-code:after:content-none ${className ?? ""}`}
    >
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {content}
      </ReactMarkdown>
    </div>
  );
}

/**
 * react-markdown children은 string | ReactElement | array 형태.
 * string 노드만 highlight 적용, 그 외 (em/strong/link 등 inline element) 는 그대로 둔다.
 */
function wrapChildren(children: ReactNode, terms: string[]): ReactNode {
  if (typeof children === "string") {
    return <>{highlightTerms(children, terms)}</>;
  }
  if (Array.isArray(children)) {
    return children.map((c, i) =>
      typeof c === "string" ? (
        <Fragment key={i}>{highlightTerms(c, terms)}</Fragment>
      ) : (
        <Fragment key={i}>{c}</Fragment>
      ),
    );
  }
  return children;
}
