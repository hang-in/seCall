import { describe, it, expect } from "vitest";
import { remark } from "remark";
import remarkHtml from "remark-html";
import { remarkObsidianCallouts } from "../remarkObsidianCallouts";

async function render(md: string): Promise<string> {
  const out = await remark()
    .use(remarkObsidianCallouts)
    .use(remarkHtml, { sanitize: false })
    .process(md);
  return String(out);
}

describe("remarkObsidianCallouts", () => {
  it("default open (no flag)", async () => {
    const html = await render("> [!tip] Heads up\n> hello body");
    expect(html).toContain('<details class="callout callout-tip" open>');
    expect(html).toContain("<summary>Heads up</summary>");
    expect(html).toContain("hello body");
    expect(html).toContain("</details>");
  });

  it("collapsed with `-`", async () => {
    const html = await render("> [!tool]- ToolSearch\n> body");
    expect(html).toContain('<details class="callout callout-tool">');
    expect(html).not.toContain('class="callout callout-tool" open');
    expect(html).toContain("<summary>ToolSearch</summary>");
    expect(html).toContain("body");
  });

  it("explicit open with `+`", async () => {
    const html = await render("> [!thinking]+ Thinking\n> brain");
    expect(html).toContain('<details class="callout callout-thinking" open>');
    expect(html).toContain("<summary>Thinking</summary>");
  });

  it("no title falls back to capitalized type", async () => {
    const html = await render("> [!thinking]-\n> body only");
    expect(html).toContain('<details class="callout callout-thinking">');
    expect(html).toContain("<summary>Thinking</summary>");
    expect(html).toContain("body only");
  });

  it("plain blockquote is unchanged", async () => {
    const html = await render("> just a quote\n> second line");
    expect(html).not.toContain("callout");
    expect(html).toContain("<blockquote>");
    expect(html).toContain("just a quote");
  });

  it("escapes special chars in summary", async () => {
    // remark 의 markdown parser 는 raw HTML 을 별도 HTML 노드로 분리하므로
    // `<script>` 같은 형태는 우리 plugin 의 regex 에 잡히지 않는다. 그건
    // rehype-sanitize 의 후처리에 위임. 여기선 일반 텍스트 (ampersand) 의
    // escape 만 검증.
    const html = await render("> [!info] A & B\n> body");
    expect(html).toContain("<summary>A &amp; B</summary>");
  });
});
