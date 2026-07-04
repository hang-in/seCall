import { visit } from "unist-util-visit";

/**
 * `language-mermaid` / `language-html` 코드펜스의 **원본 텍스트**를
 * `data-raw` 속성으로 보존한다.
 *
 * 이유: react-markdown v9 파이프라인에서 rehype-highlight 가 fenced code 를
 * hljs `<span>` 으로 토큰화한 뒤에야 `code` override 에 도달하므로, override 에서
 * `String(children)` 하면 하이라이트 마크업이 섞여 원본 소스를 잃는다. mermaid 는
 * 다이어그램 소스, html 은 iframe srcDoc 이 원본 그대로여야 한다.
 *
 * 배치: rehypeSanitize **뒤**, rehypeHighlight **앞**. sanitize 뒤라 data-raw 가
 * 스트립되지 않고, highlight 는 children 만 바꾸고 properties 는 유지하므로
 * override 에서 `node.properties.dataRaw` 로 원본을 읽을 수 있다.
 */
export function rehypeRawCode() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (tree: any) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    visit(tree, "element", (node: any) => {
      if (node.tagName !== "code") return;
      const raw = node.properties?.className;
      const classes: string[] = Array.isArray(raw) ? raw : raw ? [raw] : [];
      const langClass = classes.find(
        (c) => typeof c === "string" && c.startsWith("language-"),
      );
      if (!langClass) return;
      const lang = langClass.slice("language-".length);
      if (lang !== "mermaid" && lang !== "html") return;
      node.properties = node.properties ?? {};
      node.properties.dataRaw = collectText(node);
    });
  };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function collectText(node: any): string {
  if (!node) return "";
  if (node.type === "text") return node.value ?? "";
  if (Array.isArray(node.children))
    return node.children.map(collectText).join("");
  return "";
}
