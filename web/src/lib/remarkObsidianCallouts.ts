import { visit, SKIP } from "unist-util-visit";
import type { Root, Blockquote, Paragraph, Text, RootContent, Html } from "mdast";

/**
 * Obsidian callout syntax 를 `<details>` HTML 로 변환하는 remark plugin.
 *
 * 입력 (markdown):
 * ```
 * > [!thinking]- Thinking
 * > 본문 line 1
 * > 본문 line 2
 * ```
 *
 * 출력 (mdast / html):
 * ```html
 * <details class="callout callout-thinking">
 *   <summary>Thinking</summary>
 *   <p>본문 line 1\n본문 line 2</p>
 * </details>
 * ```
 *
 * - `[!type]` — open (default)
 * - `[!type]-` — closed (collapsed)
 * - `[!type]+` — open (명시적)
 * - title 생략 시 type 을 summary 로
 *
 * rehype-raw 가 details/summary 를 HTML 노드로 통과시키고, sanitize schema
 * 에 두 태그가 허용되어 있어 그대로 렌더링됨.
 */
export function remarkObsidianCallouts() {
  return (tree: Root) => {
    visit(tree, "blockquote", (node: Blockquote, index, parent) => {
      if (
        !parent ||
        typeof index !== "number" ||
        !node.children ||
        node.children.length === 0
      ) {
        return;
      }
      const first = node.children[0];
      if (first.type !== "paragraph") return;
      const firstChild = (first as Paragraph).children?.[0];
      if (!firstChild || firstChild.type !== "text") return;

      const text = (firstChild as Text).value;
      // 첫 줄에서 [!type] 또는 [!type]- 또는 [!type]+ + 옵션 title 매칭.
      const m = text.match(/^\[!(\w+)\]([-+])?[ \t]*(.*?)(\n|$)/);
      if (!m) return;

      const [matched, type, flag, titleRaw] = m;
      const open = flag !== "-"; // default open, `-` = closed
      const title = (titleRaw || "").trim() || capitalize(type);

      // 첫 text 노드에서 matched 부분 제거. 남은 텍스트가 있으면 유지.
      const rest = text.slice(matched.length).replace(/^\n+/, "");
      if (rest) {
        (firstChild as Text).value = rest;
      } else {
        // 첫 text 노드 제거. paragraph 가 비면 paragraph 자체도 제거.
        (first as Paragraph).children.shift();
        if ((first as Paragraph).children.length === 0) {
          node.children.shift();
        }
      }

      const openAttr = open ? " open" : "";
      const openHtml: Html = {
        type: "html",
        value: `<details class="callout callout-${escapeAttr(type)}"${openAttr}><summary>${escapeHtml(
          title,
        )}</summary>`,
      };
      const closeHtml: Html = { type: "html", value: "</details>" };

      // blockquote 자체를 unwrap: details_open + (blockquote 의 children) + details_close
      const replacement: RootContent[] = [
        openHtml,
        ...(node.children as RootContent[]),
        closeHtml,
      ];
      parent.children.splice(index, 1, ...replacement);
      // 새로 삽입된 노드들은 다시 visit 안 함 — body 의 nested callout 은 별도 패스로 처리.
      return [SKIP, index + replacement.length];
    });
  };
}

function capitalize(s: string): string {
  return s.length === 0 ? s : s[0].toUpperCase() + s.slice(1);
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function escapeAttr(s: string): string {
  return s.replace(/[^a-zA-Z0-9_-]/g, "");
}
