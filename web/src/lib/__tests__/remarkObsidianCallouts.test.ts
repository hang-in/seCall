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

// R1 — 본문 없는 콜아웃 숨김은 thinking 타입에 한정한다. 제목만 있는 다른
// 타입(tool/warning 등)까지 지우면 헤더(제목) 정보가 사라지는 content-loss 회귀.
describe("remarkObsidianCallouts — R1 empty-callout 숨김 (thinking 한정)", () => {
  async function render(md: string): Promise<string> {
    const out = await remark()
      .use(remarkObsidianCallouts)
      .use(remarkHtml, { sanitize: false })
      .process(md);
    return String(out);
  }

  it("본문 없는 thinking 콜아웃은 제거 (redacted/빈 thinking)", async () => {
    const html = await render("> [!thinking]- Thinking");
    expect(html).not.toContain("callout-thinking");
    expect(html).not.toContain("<summary>Thinking</summary>");
  });

  it("본문 없는 tool 콜아웃(제목만)은 보존 — 헤더 유지", async () => {
    const html = await render("> [!tool]- Bash");
    expect(html).toContain('<details class="callout callout-tool">');
    expect(html).toContain("<summary>Bash</summary>");
  });

  it("본문 없는 warning 콜아웃(제목만)은 보존", async () => {
    const html = await render("> [!warning] Deprecated API");
    expect(html).toContain('<details class="callout callout-warning" open>');
    expect(html).toContain("<summary>Deprecated API</summary>");
  });

  it("본문 있는 thinking 콜아웃은 정상 렌더", async () => {
    const html = await render("> [!thinking]- Thinking\n> real reasoning");
    expect(html).toContain('<details class="callout callout-thinking">');
    expect(html).toContain("real reasoning");
  });
});
