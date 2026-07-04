import { useEffect, useRef, useState } from "react";
import { useIsDark } from "@/lib/useIsDark";

/**
 * ` ```mermaid ` 코드펜스를 실제 다이어그램(SVG)으로 렌더한다.
 *
 * - mermaid 는 무거워서(~수백KB) dynamic import 로 첫 블록 등장 시에만 로드 →
 *   초기 번들에서 격리 (vite manualChunks 정적 청크에 넣지 말 것).
 * - in-document SVG 라 iframe/토큰주입 불필요. 테마만 dark/default 전달.
 * - securityLevel:"strict" 로 mermaid 가 script 를 제거하므로 SVG 주입은 안전.
 */

// 모듈 단위 캐시 — mermaid 를 한 번만 로드.
let mermaidPromise: Promise<typeof import("mermaid")> | null = null;
function loadMermaid() {
  if (!mermaidPromise) mermaidPromise = import("mermaid");
  return mermaidPromise;
}

let seq = 0;

export function MermaidBlock({ code }: { code: string }) {
  const dark = useIsDark();
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const idRef = useRef(`mermaid-${(seq += 1)}`);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    setSvg(null);
    loadMermaid()
      .then(({ default: mermaid }) => {
        mermaid.initialize({
          startOnLoad: false,
          theme: dark ? "dark" : "default",
          securityLevel: "strict",
        });
        return mermaid.render(idRef.current, code);
      })
      .then((res) => {
        if (!cancelled) setSvg(res.svg);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [code, dark]);

  if (error) {
    return (
      <pre className="my-ds-3 whitespace-pre-wrap rounded-lg border border-status-danger/40 p-ds-3 text-t-mono text-status-danger">
        Mermaid 렌더 실패: {error}
        {"\n\n"}
        {code}
      </pre>
    );
  }
  if (svg === null) {
    return (
      <div className="my-ds-3 p-ds-3 text-t-caption text-text-4">
        다이어그램 렌더 중…
      </div>
    );
  }
  return (
    <div
      className="my-ds-3 flex justify-center [&_svg]:max-w-full"
      // securityLevel:strict 로 script 가 제거된 mermaid 출력 SVG.
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}
