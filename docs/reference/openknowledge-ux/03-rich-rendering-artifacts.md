# 03. ★ 리치 렌더링 / 라이브 "아티팩트" — 최대 하이라이트

> OK의 진짜 차별점. **평범한 마크다운 파일**이 폴리시된 인터랙티브 문서로 렌더됨. LLM이 이 마크다운을 생성하면 = 위키가 "아티팩트"처럼 보임. (사용자가 "미려하다/아티팩트처럼 멋지다"고 한 게 이것.)

## 3.1 자동 승격 컴포넌트 (마크다운-네이티브 → 테마 컴포넌트)

| 원하는 것 | 마크다운-네이티브로 씀 | 승격 |
|---|---|---|
| Callout/admonition | `> [!NOTE]` + 본문 (NOTE/TIP/IMPORTANT/WARNING/CAUTION 등 **15종**, 뒤에 `+`/`-`로 접기) | 아이콘+색 테마 콜아웃 |
| Collapsible | `<details><summary>제목</summary>…</details>` | 테마 아코디언 |
| Diagram | ` ```mermaid ` (flowchart/sequence/class/state/ER/gantt/pie) | Mermaid, 테마 색 |
| Math | `$x$` 인라인 / `$$…$$` 블록 | KaTeX |
| Wiki embed | `![[file]]` | 문서/자산 인라인 임베드 |
| Tabs | `<Tabs><Tab label="…">…</Tab></Tabs>` (JSX, 네이티브 없음) | 테마 탭 |

> 규칙: 캐노니컬(테마·접근성·그래프 통합)이 있으면 JSX 대신 마크다운-네이티브를 씀.

## 3.2 ` ```html preview ` = 라이브 아티팩트 (★★핵심)

- **HTML/CSS/JS 펜스를 샌드박스 iframe으로 실렌더** — 차트·스탯카드·커스텀 SVG·계산기·인터랙티브 데모. **Claude Artifacts를 위키 문서에 인라인 임베드**한 것.
- **테마 토큰 주입**: iframe에 `var(--foreground)`·`var(--card)`·`var(--border)`·`var(--radius)`·`var(--chart-1..5)`·`var(--muted-foreground)`·`var(--primary)`·`var(--background)` 등 주입 → **라이트/다크 토글 시 임베드도 자동 리스킨**.
  - **하드코딩 hex/rgb 금지**가 명시 규칙(안 지키면 다크 배경에 흰 박스). JS 차트는 런타임에 `getComputedStyle(document.documentElement).getPropertyValue('--chart-1')`로 토큰 읽어 라이브러리에 넘김.
- **auto-size** iframe, `h=`/`w=`로 고정(예 ` ```html preview h=400px `).
- **네트워크 열림**: 외부 CSS/웹폰트/`fetch`/Leaflet/Chart.js/D3/맵타일 로드 가능. but **null-origin 샌드박스** — KB·쿠키·인증엔 접근 불가, `unsafe-eval` 미허용(런타임 표현식 컴파일 라이브러리는 못 씀).
- 작성자는 `palette`가 주는 **embedPatterns(차트/스탯카드/SVG/인터랙션 스타터)** 복사해 데이터만 채움("hand-roll 금지").

### 최소 예시 (테마 토큰 스탯카드)
```html preview
<div style="font-family:system-ui;padding:20px;color:var(--foreground)">
  <div style="display:flex;gap:12px">
    <div style="flex:1;padding:16px;background:var(--card);border:1px solid var(--border);border-radius:var(--radius)">
      <div style="font-size:30px;font-weight:700;color:var(--chart-1)">24</div>
      <div style="color:var(--muted-foreground);font-size:13px">정상</div>
    </div>
  </div>
</div>
```

## 3.3 실렌더 검증 (2026-07-04, 데모 문서)

평범한 `.md` 하나(Callout + Mermaid + html preview 스탯카드 + KaTeX)를 열었더니 전부 폴리시 렌더:
- `[!TIP]` → 초록 콜아웃(아이콘) · Mermaid → 보라 플로우차트 · html preview → 테마 스탯카드 3개(chart 색·보더·radius 연동) · `$E=mc^2$`·∑ → KaTeX.
- **네이티브로 쓴 `.md`도 파일와처가 즉시 인제스트해 렌더** (외부 편집/LLM 생성 파일도 바로 반영).

## 3.4 "show, don't tell" 저작 규칙

- 정량/비교/추세/분포/전후/랭킹은 **프로즈 대신 시각화**(차트·스탯카드 html preview, Mermaid, 표, 헤드라인 Callout)로 내라는 게 OK 저작 가이드. research/consolidate 문서에서 특히.

## 3.5 seCall 적용 (★★ 최우선)

- **이게 "미려함"의 정체.** seCall이 **LLM 생성 위키를 이 수준으로 렌더**하면 웹UI 격차 대부분 해소.
- 최소 구현: (1) `> [!TYPE]` 콜아웃 (2) ` ```mermaid ` (3) **` ```html preview ` 샌드박스 iframe + 테마 토큰 주입**(핵심·최고난도) (4) KaTeX (5) `![[embed]]`.
- **시너지**: seCall은 이미 LLM·세션 데이터 보유 → **세션 요약을 스탯카드/차트 아티팩트로** 생성·렌더. LLM 프롬프트가 정량 내용을 html preview로 내도록 유도.
- 보안: 임베드는 **null-origin 샌드박스 iframe** + 테마 토큰 주입 방식 그대로 따를 것(KB 접근 차단).
