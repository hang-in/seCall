import { useEffect, useState } from "react";

/**
 * 현재 다크 모드 여부를 `<html class="dark">` 로부터 읽고 class 변화를 구독한다.
 *
 * `useTheme` 는 컴포넌트 로컬 state 라 다른 트리에서 토글을 관측하지 못한다.
 * MarkdownView 처럼 테마에 반응해야 하는 렌더(mermaid theme / iframe color-scheme)는
 * DOM class 를 단일 진실 소스로 구독하는 이 훅을 쓴다.
 */
export function useIsDark(): boolean {
  const [dark, setDark] = useState<boolean>(
    () =>
      typeof document !== "undefined" &&
      document.documentElement.classList.contains("dark"),
  );

  useEffect(() => {
    const el = document.documentElement;
    const obs = new MutationObserver(() => {
      setDark(el.classList.contains("dark"));
    });
    obs.observe(el, { attributes: true, attributeFilter: ["class"] });
    return () => obs.disconnect();
  }, []);

  return dark;
}
