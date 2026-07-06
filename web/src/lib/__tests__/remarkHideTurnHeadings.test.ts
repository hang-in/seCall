import { describe, it, expect } from "vitest";
import { remark } from "remark";
import remarkHtml from "remark-html";
import {
  remarkHideTurnHeadings,
  isGeneratedTurnHeading,
} from "../remarkHideTurnHeadings";
import type { Heading } from "mdast";

async function render(md: string): Promise<string> {
  const out = await remark()
    .use(remarkHideTurnHeadings)
    .use(remarkHtml, { sanitize: false })
    .process(md);
  return String(out);
}

/** depth + 단일 text child 로 heading 노드 mock. */
function heading(depth: 1 | 2 | 3, text: string): Heading {
  return {
    type: "heading",
    depth,
    children: [{ type: "text", value: text }],
  };
}

describe("remarkHideTurnHeadings — 생성된 Turn 헤더만 제거", () => {
  it("`## Turn N — Role` (depth 2) 제거", async () => {
    const html = await render("## Turn 1 — User\n\nbody");
    expect(html).not.toContain("Turn 1");
    expect(html).toContain("body");
  });

  it("`### Turn N` (depth 3) 제거", async () => {
    const html = await render("### Turn 2\n\nbody");
    expect(html).not.toContain("Turn 2");
    expect(html).toContain("body");
  });

  it("`### Turn N (HH:MM)` (depth 3, 시각) 제거", async () => {
    const html = await render("### Turn 3 (14:30)\n\nbody");
    expect(html).not.toContain("Turn 3");
  });

  it("사용자 heading `## Turn 3 회고` (em-dash 없음) 은 보존", async () => {
    const html = await render("## Turn 3 회고\n\nbody");
    expect(html).toContain("Turn 3 회고");
  });

  it("사용자 heading `# Turn 1` (depth 1) 은 보존", async () => {
    const html = await render("# Turn 1\n\nbody");
    expect(html).toContain("Turn 1");
  });

  it("`### Turntable setup` (Turn+숫자 아님) 은 보존", async () => {
    const html = await render("### Turntable setup\n\nbody");
    expect(html).toContain("Turntable setup");
  });
});

describe("isGeneratedTurnHeading — 판정 단위 테스트", () => {
  it("생성 포맷은 true", () => {
    expect(isGeneratedTurnHeading(heading(2, "Turn 1 — User"))).toBe(true);
    expect(isGeneratedTurnHeading(heading(2, "Turn 12 — Assistant"))).toBe(
      true,
    );
    expect(isGeneratedTurnHeading(heading(3, "Turn 2"))).toBe(true);
    expect(isGeneratedTurnHeading(heading(3, "Turn 3 (14:30)"))).toBe(true);
  });

  it("사용자 heading 은 false", () => {
    // depth 2 인데 em-dash 구분자 없음
    expect(isGeneratedTurnHeading(heading(2, "Turn 3 회고"))).toBe(false);
    // depth 2 인데 숫자 뒤 em-dash 없이 텍스트
    expect(isGeneratedTurnHeading(heading(2, "Turn of the century"))).toBe(
      false,
    );
    // depth 1
    expect(isGeneratedTurnHeading(heading(1, "Turn 1"))).toBe(false);
    // depth 3 인데 숫자 뒤 시각 외 텍스트
    expect(isGeneratedTurnHeading(heading(3, "Turn 2 extra"))).toBe(false);
    // Turn + 숫자 아님
    expect(isGeneratedTurnHeading(heading(3, "Turntable"))).toBe(false);
  });
});
