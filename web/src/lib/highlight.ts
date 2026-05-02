import { createElement, Fragment, type ReactNode } from "react";

/**
 * 검색 쿼리를 매칭에 사용할 토큰 배열로 분리.
 *
 * - 공백 + 일반 구두점으로 split
 * - 길이 1 이하는 잡음이 너무 많아 제거 (한글 단일 음절 매칭은 Phase 3+)
 * - 중복 제거 + 길이 내림차순 정렬 → 긴 토큰 우선 매칭으로 부분 매칭 우선순위 안정화
 * - lowercased 형태로 보관 (실제 매칭은 case-insensitive regex로)
 */
export function tokenizeQuery(query: string): string[] {
  return Array.from(
    new Set(
      query
        .toLowerCase()
        .split(/[\s,.;:!?()[\]{}<>"'/\\]+/)
        .filter((t) => t.length > 1),
    ),
  ).sort((a, b) => b.length - a.length);
}

/**
 * 텍스트에서 토큰 매칭 부분을 `<mark>`로 감싼 ReactNode 배열을 반환.
 *
 * - 매칭은 case-insensitive substring (regex 특수문자 escape)
 * - `parts.split(re-with-g)` 패턴 사용. `re.test`는 stateful (g flag) 이므로
 *   파트 매칭 판별은 별도의 stateless `^...$` regex (no g) 로 수행
 * - 토큰이 없거나 텍스트가 비면 원본 그대로 반환
 *
 * 본 파일은 작업 지시서의 신규 파일 경로 (`web/src/lib/highlight.ts`) 명세를 따라
 * .ts 확장자를 유지하기 위해 React.createElement를 직접 사용한다.
 */
export function highlightTerms(text: string, terms: string[]): ReactNode[] {
  if (terms.length === 0 || !text) return [text];
  const escaped = terms.map(escapeRegex).join("|");
  const splitRe = new RegExp(`(${escaped})`, "gi");
  const matchRe = new RegExp(`^(?:${escaped})$`, "i");
  const parts = text.split(splitRe);
  return parts.map((part, i) =>
    matchRe.test(part)
      ? createElement(
          "mark",
          {
            key: i,
            className: "bg-amber-500/30 text-amber-100 px-0.5 rounded",
          },
          part,
        )
      : createElement(Fragment, { key: i }, part),
  );
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
