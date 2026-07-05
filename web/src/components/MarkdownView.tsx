import { Fragment, type ReactNode, useMemo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkFrontmatter from "remark-frontmatter";
import remarkMath from "remark-math";
import { remarkObsidianCallouts } from "@/lib/remarkObsidianCallouts";
// remark-wiki-link / rehype-raw / rehype-highlight / rehype-sanitize / rehype-katex: 외부 plugin.
import remarkWikiLink from "remark-wiki-link";
import rehypeRaw from "rehype-raw";
import rehypeHighlight from "rehype-highlight";
import rehypeKatex from "rehype-katex";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import { NavLink } from "react-router";
import "highlight.js/styles/github-dark.css";
import "katex/dist/katex.min.css";
import { rehypeRawCode } from "@/lib/rehypeRawCode";
import { MermaidBlock } from "@/components/MermaidBlock";
import { HtmlPreview } from "@/components/HtmlPreview";
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
      // node prop 은 react-markdown 의 hast 노드 — DOM element 에 spread 하면
      // `node="[object Object]"` 가 그대로 attribute 로 박힌다. 모든 override
      // 에서 명시적으로 destructure.
      p: ({ node: _n, children }) => <p>{wrapChildren(children, terms)}</p>,
      li: ({ node: _n, children, ...rest }) => (
        <li {...rest}>{wrapChildren(children, terms)}</li>
      ),
      code: ({ node, children, className: cls, ...rest }) => {
        const lang =
          typeof cls === "string" ? /language-(\w+)/.exec(cls)?.[1] : undefined;
        // rehypeRawCode 가 mermaid/html 원본을 properties.dataRaw 로 보존(highlight 이전 텍스트).
        const dataRaw = node?.properties?.dataRaw as string | undefined;
        if (lang === "mermaid" && typeof dataRaw === "string") {
          return <MermaidBlock code={dataRaw.replace(/\n$/, "")} />;
        }
        if (lang === "html" && typeof dataRaw === "string") {
          return <HtmlPreview html={dataRaw} />;
        }
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
      a: ({ node: _n, href, children, ...rest }) => {
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
      // remark-frontmatter: vault md 의 `---\nyaml\n---` 를 frontmatter 노드로
      // 인식해 본문 렌더링에서 제외. 이전엔 닫는 `---` 가 setext H2 underline
      // 으로 잘못 해석되어 frontmatter 전체가 거대한 H2 로 표시되고 본문 시작
      // 위에 sources/tags 가 노출됐다.
      remarkFrontmatter,
      remarkGfm,
      // remark-math: `$…$` / `$$…$$` 를 math 노드로 (rehype-katex 가 렌더).
      remarkMath,
      // remarkObsidianCallouts: vault session md 의 `> [!tool]- Title` 같은
      // Obsidian callout 을 `<details class="callout callout-tool">` 로 변환.
      // claude-code session md 의 thinking/tool 블록이 폴딩 가능한 박스로
      // 표시되도록 한다. P49 의 `### Turn N` (같은 role 연속) 강등과 결합되어
      // 본문이 turn 별 callout 묶음으로 자연스럽게 펼침/접힘.
      remarkObsidianCallouts,
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
        // P81: remarkObsidianCallouts 가 details 에 className="callout callout-<type>"
        // 을 부여하므로 className 허용 (callout / callout-* 접두사 한정).
        details: [
          ...((defaultSchema.attributes ?? {}).details ?? []),
          "open",
          ["className", "callout", /^callout-/],
        ],
        code: [
          ...((defaultSchema.attributes ?? {}).code ?? []),
          ["className", /^language-/, /^hljs/],
        ],
        span: [
          ...((defaultSchema.attributes ?? {}).span ?? []),
          // hljs-* (highlight) + math* (remark-math 중간 노드 → rehype-katex seam).
          ["className", /^hljs-/, "math", "math-inline", "math-display"],
        ],
      },
    };
    // 순서가 load-bearing:
    //   rehypeRaw → rehypeSanitize → rehypeRawCode(mermaid/html 원본 보존) →
    //   rehypeKatex(sanitize 뒤라 katex 출력이 스트립되지 않음) → rehypeHighlight.
    // unified 의 PluggableList 타입이 tuple form ([plugin, options]) 을 정확히
    // 매칭하지 못해 cast.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return [
      rehypeRaw,
      [rehypeSanitize, schema],
      rehypeRawCode,
      rehypeKatex,
      rehypeHighlight,
    ] as any;
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
