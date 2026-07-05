import { useIsDark } from "@/lib/useIsDark";

/**
 * ` ```html ` 코드펜스를 sandbox iframe 으로 실렌더한다 (차트/스탯카드 등 라이브 아티팩트).
 *
 * 보안 — **로컬 개인 도구 전제**(localhost, 외부 미공개):
 * - `sandbox="allow-scripts"` — `allow-same-origin` 은 절대 붙이지 않는다.
 *   (붙이면 iframe 이 자기 샌드박스를 스스로 제거할 수 있어, 부모 앱의
 *    localStorage/쿠키/DOM 격리가 무너진다.)
 * - 네트워크 개방 — Chart.js/D3/맵타일 등 외부 리소스 로드 허용. 로컬 개인이라 수용.
 * - 자동 lazy 렌더 — 'Run preview' 게이트 없음(개인 도구).
 * - color-scheme 토큰만 주입해 다크/라이트 배경을 맞춤.
 *
 * ⚠️ TODO(공유 기능 도입 시): 세션 본문은 에이전트 툴 출력발 임의 HTML/JS 가
 *    섞일 수 있어 신뢰도가 낮다. 위키/세션 공유를 만들면 여기에
 *    srcDoc 내부 CSP(`default-src 'none'`)와 '세션 vs 위키' 신뢰 경계를
 *    재도입해 신뢰 안 되는 소스의 네트워크/스크립트 실행을 차단해야 한다.
 */
export function HtmlPreview({ html }: { html: string }) {
  const dark = useIsDark();

  const doc = `<!doctype html>
<html lang="ko">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<style>
  :root { color-scheme: ${dark ? "dark" : "light"}; }
  html, body {
    margin: 0;
    padding: 12px;
    background: ${dark ? "#0b0b0c" : "#ffffff"};
    color: ${dark ? "#e5e5e5" : "#111111"};
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 14px;
  }
</style>
</head>
<body>${html}</body>
</html>`;

  return (
    <iframe
      title="HTML preview"
      sandbox="allow-scripts"
      srcDoc={doc}
      loading="lazy"
      className="my-ds-3 w-full rounded-lg border border-hairline bg-surface-2"
      style={{ height: 360, colorScheme: dark ? "dark" : "light" }}
    />
  );
}
