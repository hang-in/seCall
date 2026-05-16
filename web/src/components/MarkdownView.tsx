import {
  Fragment,
  type ReactNode,
  useMemo,
  useState,
  type MouseEvent,
  type KeyboardEvent,
} from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
// remark-wiki-link / rehype-raw / rehype-highlight / rehype-sanitize: 외부 plugin.
import remarkWikiLink from "remark-wiki-link";
import rehypeRaw from "rehype-raw";
import rehypeHighlight from "rehype-highlight";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import { NavLink } from "react-router";
import "highlight.js/styles/github-dark.css";
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
 *
 * P66 — 확장:
 * - rehype-raw: `<details><summary>` 등 raw HTML 통과 (폴딩 동작)
 * - rehype-highlight: 코드블록 syntax highlighting (highlight.js github-dark)
 * - remark-wiki-link: `[[Page Name]]` Obsidian 스타일 wikilink → /wiki/...
 * - h2/h3 click → 직후 sibling 그룹 collapse 토글 (▶/▼ 마커)
 *
 * P34 Task 02 — query가 있으면 본문 안의 매칭 토큰에 `<mark>`를 적용.
 * react-markdown components override는 `p / li / code` 의 children에서만 동작.
 * heading / link 안 매칭은 acceptable한 누락 (Risks 참조).
 */
export function MarkdownView({ content, query, className }: Props) {
  const terms = useMemo(() => tokenizeQuery(query ?? ""), [query]);

  const components = useMemo<Components>(() => {
    return {
      p: ({ children }) => <p>{wrapChildren(children, terms)}</p>,
      li: ({ children }) => <li>{wrapChildren(children, terms)}</li>,
      code: ({ children, className: cls, ...rest }) => {
        // rehype-highlight 가 코드블록 (pre > code) 에 hljs / language-* 클래스를 부여.
        // 검색 하이라이트는 inline code 만 적용 (코드블록은 highlight.js 가 점유).
        const isBlock = typeof cls === "string" && cls.includes("language-");
        if (isBlock) {
          return (
            <code className={cls} {...rest}>
              {children}
            </code>
          );
        }
        return (
          <code className={cls} {...rest}>
            {wrapChildren(children, terms)}
          </code>
        );
      },
      h2: ({ children }) => (
        <CollapsibleHeading level={2}>{children}</CollapsibleHeading>
      ),
      h3: ({ children }) => (
        <CollapsibleHeading level={3}>{children}</CollapsibleHeading>
      ),
      a: ({ href, children, ...rest }) => {
        // remark-wiki-link 가 [[Page]] 를 a[href=/wiki/Page] 로 만들어줌 (hrefTemplate 으로 강제).
        if (typeof href === "string" && href.startsWith("/wiki/")) {
          return (
            <NavLink to={href} className="text-brand hover:underline">
              {children}
            </NavLink>
          );
        }
        if (typeof href === "string" && /^https?:\/\//.test(href)) {
          return (
            <a href={href} target="_blank" rel="noreferrer" {...rest}>
              {children}
            </a>
          );
        }
        return (
          <a href={href} {...rest}>
            {children}
          </a>
        );
      },
    };
  }, [terms]);

  const remarkPlugins = useMemo(
    () => [
      remarkGfm,
      [
        remarkWikiLink,
        {
          pageResolver: (name: string) => [normalizeWikiName(name)],
          hrefTemplate: (permalink: string) => `/wiki/${permalink}`,
          aliasDivider: "|",
        },
      ],
    ],
    [],
  );

  const rehypePlugins = useMemo(() => {
    // P66 follow-up (Gemini PR #75 보안 리뷰):
    // rehype-raw 가 raw HTML 을 hast 노드로 변환 → rehype-sanitize 가 XSS
    // 위험 태그/속성 제거 (rehypeRaw 직후, highlight 전에 sanitize 해야 함) →
    // rehype-highlight 가 코드블록 syntax 처리.
    //
    // sanitize schema: defaultSchema (script/style/event handler 차단) 에
    // <details>/<summary> 와 highlight.js 의 class 속성 허용 추가.
    const schema = {
      ...defaultSchema,
      tagNames: [...(defaultSchema.tagNames ?? []), "details", "summary"],
      attributes: {
        ...(defaultSchema.attributes ?? {}),
        // Gemini PR #77 리뷰: <details open> 의 open 속성 + 브라우저 토글 시
        // 추가되는 open 속성도 허용해야 함.
        details: [
          ...((defaultSchema.attributes ?? {}).details ?? []),
          "open",
        ],
        code: [
          ...((defaultSchema.attributes ?? {}).code ?? []),
          ["className", /^language-/, /^hljs/],
        ],
        span: [
          ...((defaultSchema.attributes ?? {}).span ?? []),
          ["className", /^hljs-/],
        ],
      },
    };
    // unified 의 PluggableList 타입이 tuple form ([plugin, options]) 을 정확히
    // 매칭하지 못해 cast. plugin/option 호환성은 rehype-sanitize 의 런타임 시그
    // 니처와 일치.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return [rehypeRaw, [rehypeSanitize, schema], rehypeHighlight] as any;
  }, []);

  return (
    <div
      className={[
        // Tailwind typography prose + design-tokens 매핑.
        // dark 모드는 prose-invert 가 처리. surface/text 색은 토큰 그대로 흐름.
        "prose prose-sm dark:prose-invert max-w-none",
        // code/pre 토큰 정렬 (mono + surface-2 + radius)
        "prose-code:before:content-none prose-code:after:content-none",
        "prose-code:font-mono prose-code:text-t-mono prose-code:text-text",
        "prose-pre:bg-surface-2 prose-pre:border prose-pre:border-hairline prose-pre:rounded-lg prose-pre:text-text",
        // headings: 본문이 prose 본문 톤보다 약간 묵직하게
        "prose-headings:text-text prose-headings:font-medium prose-headings:tracking-tight",
        // body
        "prose-p:text-text-2",
        // links: brand
        "prose-a:text-brand prose-a:no-underline hover:prose-a:underline",
        // hr / blockquote
        "prose-hr:border-hairline",
        "prose-blockquote:border-l-2 prose-blockquote:border-l-border-soft prose-blockquote:text-text-2 prose-blockquote:not-italic",
        className ?? "",
      ].join(" ")}
    >
      <ReactMarkdown
        // @ts-expect-error remark-wiki-link 의 plugin 시그니처가 unified 의 PluggableList 와 정확히 안 맞음
        remarkPlugins={remarkPlugins}
        rehypePlugins={rehypePlugins}
        components={components}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}

/**
 * heading 자체에 collapse 상태를 둠. 클릭 시 ▶/▼ 마커만 토글하고
 * 직후 sibling 의 시각적 숨김은 CSS sibling selector + data attribute 로 처리.
 * 단순화: heading 다음 형제 그룹 숨김은 CSS [data-collapsed=true] ~ * { display:none }
 * 까지는 React tree 에서 어렵기 때문에, 본 구현은 details/summary 폴딩이
 * 메인 폴딩 메커니즘. heading collapse 는 marker + aria 만 토글 (MVP).
 *
 * 다음 sibling 그룹 숨김은 후속 작업 (DOM 직접 조작 필요).
 */
function CollapsibleHeading({
  level,
  children,
}: {
  level: 2 | 3;
  children: ReactNode;
}) {
  const [collapsed, setCollapsed] = useState(false);
  const onClick = (e: MouseEvent<HTMLElement>) => {
    // a/code 등 inline element 클릭은 무시 (링크 동작 보전).
    const target = e.target as HTMLElement;
    if (target.closest("a") || target.closest("code")) return;
    setCollapsed((v) => !v);
    // 다음 sibling 그룹 토글: 같은 또는 더 높은 레벨 heading 나올 때까지 숨김.
    const headingEl = e.currentTarget;
    let next = headingEl.nextElementSibling as HTMLElement | null;
    const stopTags = new Set(["H1", "H2"].concat(level === 2 ? [] : ["H3"]));
    while (next && !stopTags.has(next.tagName)) {
      next.style.display = collapsed ? "" : "none";
      next = next.nextElementSibling as HTMLElement | null;
    }
  };
  // P66 follow-up (Gemini PR #75 a11y 리뷰):
  // role="button" + tabIndex=0 만으론 키보드 사용자가 폴딩 못 함.
  // Enter / Space 가 onClick 과 동일하게 작동하도록.
  //
  // Gemini PR #77 추가 리뷰: heading 내부 link (`<a>`) 의 Enter 키 동작 보전을
  // 위해 onClick 과 동일하게 target 이 a/code 면 early return.
  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== "Enter" && e.key !== " ") return;
    const target = e.target as HTMLElement;
    if (target.closest("a") || target.closest("code")) return;
    e.preventDefault();
    onClick(e as unknown as MouseEvent<HTMLElement>);
  };
  const marker = collapsed ? "▶" : "▼";
  const props = {
    onClick,
    onKeyDown,
    "aria-expanded": !collapsed,
    role: "button",
    tabIndex: 0,
    style: { cursor: "pointer", userSelect: "none" as const },
    children: (
      <>
        <span className="text-text-2 mr-2 select-none">{marker}</span>
        {children}
      </>
    ),
  };
  if (level === 2) return <h2 {...props} />;
  return <h3 {...props} />;
}

function normalizeWikiName(name: string): string {
  // [[Some Page]] → "Some Page" → "Some_Page" (URL-safe, NavLink to=/wiki/Some_Page).
  // WikiRoute 가 hash/path 어떤 형식을 받는지는 별개 — 현재는 raw permalink 만 전달.
  return name.trim().replace(/\s+/g, "_");
}

/**
 * react-markdown children은 string | ReactElement | array 형태.
 * string 노드만 highlight 적용, 그 외 (em/strong/link 등 inline element) 는 그대로 둔다.
 */
function wrapChildren(children: ReactNode, terms: string[]): ReactNode {
  if (terms.length === 0) return children;
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
