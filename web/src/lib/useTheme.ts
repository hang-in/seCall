import { useEffect, useState } from "react";

/**
 * 다크/라이트 모드 토글 — `<html class="dark">` 토글 + localStorage 저장.
 *
 * 디자인 시스템 기본값은 다크 (web/src/lib/design-tokens.md 참고).
 * 첫 로드 시 localStorage 의 'theme' 우선 → 없으면 시스템 prefers-color-scheme → 없으면 dark.
 */
const STORAGE_KEY = "secall.theme";

function readStoredTheme(): "dark" | "light" {
  if (typeof window === "undefined") return "dark";
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (stored === "dark" || stored === "light") return stored;
  if (window.matchMedia?.("(prefers-color-scheme: dark)").matches) return "dark";
  return "dark";
}

export function useTheme() {
  const [dark, setDark] = useState<boolean>(() => readStoredTheme() === "dark");

  useEffect(() => {
    const cls = document.documentElement.classList;
    cls.toggle("dark", dark);
    window.localStorage.setItem(STORAGE_KEY, dark ? "dark" : "light");
  }, [dark]);

  return {
    dark,
    toggle: () => setDark((v) => !v),
    setDark,
  };
}
