import { visit, SKIP } from "unist-util-visit";
import type { Root, Heading, Text as MdastText } from "mdast";

/**
 * R2 — `## Turn N — Role` / `### Turn N (HH:MM)` 형태의 Turn 구분 heading 을
 * 트리에서 제거해 본문만 자연스럽게 이어지게 하는 remark plugin. showTurnHeaders 가
 * false 일 때만 remarkPlugins 에 포함된다.
 */
export function remarkHideTurnHeadings() {
  return (tree: Root) => {
    visit(tree, "heading", (node: Heading, index, parent) => {
      if (!parent || typeof index !== "number") return;
      if (isGeneratedTurnHeading(node)) {
        parent.children.splice(index, 1);
        // 노드를 제거했으므로 같은 index(다음 sibling)에서 계속.
        return [SKIP, index];
      }
    });
  };
}

/**
 * markdown.rs 가 생성한 Turn 구분 heading 인지 판정. 사용자가 본문에 쓴
 * "Turn 3 회고" 같은 heading 을 실수로 지우지 않도록 생성 포맷에 정확히 맞춘다:
 *   - `## Turn N — Role`        (depth 2, em-dash 구분자 필수)
 *   - `### Turn N` / `### Turn N (HH:MM)` (depth 3, 숫자 뒤 시각만 허용)
 */
export function isGeneratedTurnHeading(node: Heading): boolean {
  const t = headingText(node).trim();
  if (node.depth === 2) return /^turn\s+\d+\s+—\s+\S/i.test(t);
  if (node.depth === 3) return /^turn\s+\d+(?:\s+\(\d{1,2}:\d{2}\))?$/i.test(t);
  return false;
}

function headingText(node: Heading): string {
  let out = "";
  visit(node, "text", (t: MdastText) => {
    out += t.value;
  });
  return out;
}
