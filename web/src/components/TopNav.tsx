import { NavLink, useLocation, useNavigate } from "react-router";
import { Keyboard, Moon, Settings, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { useTheme } from "@/lib/useTheme";
import { useUi } from "@/lib/store";
import { HeaderSearch } from "@/components/HeaderSearch";
import { api } from "@/lib/api";

/**
 * 상단 네비게이션 — Calm/Editorial top nav (height = --nav-h, 48px).
 *
 * 디자인: docs/prompts/2026-05-06/web-redesign.md (Stage 2a)
 * 좌측: brand (logo dot + secall + version) + 라우트 버튼들
 * 우측: 단축키 도움말 + 다크/라이트 토글
 */

const NAV_ITEMS = [
  { to: "/sessions", label: "Sessions", hint: "g s" },
  { to: "/wiki", label: "Wiki", hint: "g w" },
  { to: "/daily", label: "Daily", hint: "g d" },
  { to: "/graph", label: "Graph", hint: "g g" },
  { to: "/commands", label: "Commands", hint: "g c" },
] as const;

export function TopNav() {
  const { dark, toggle } = useTheme();
  const setHelpOpen = useUi((s) => s.setHelpDialogOpen);
  const location = useLocation();
  const navigate = useNavigate();
  // P62: server 의 빌드 시점 버전 (CARGO_PKG_VERSION) 을 SSOT 로 사용.
  // 이전에는 APP_VERSION 이 컴포넌트에 hardcoded 되어 있어 release 마다 잊혀졌다.
  const [appVersion, setAppVersion] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    api
      .status()
      .then((s) => {
        if (!cancelled) setAppVersion(`v${s.version}`);
      })
      .catch(() => {
        if (!cancelled) setAppVersion(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // sessions / wiki 라우트에서만 헤더 검색 노출. mode 후보는 라우트별로 다름.
  const path = location.pathname;
  const onSessions = path === "/" || path === "/sessions" || path.startsWith("/sessions/");
  const onWiki = path === "/wiki" || path.startsWith("/wiki/");
  const showSearch = onSessions || onWiki;
  const modes = onWiki
    ? (["keyword", "semantic", "hybrid"] as const)
    : (["keyword", "semantic"] as const);

  return (
    <header className="h-nav-h shrink-0 border-b border-hairline bg-[var(--surface)] sticky top-0 z-30">
      <div className="h-full flex items-center gap-ds-6 px-ds-4">
        {/* Brand */}
        <div className="flex items-center gap-ds-3 shrink-0">
          <div className="flex items-center gap-ds-2">
            <span className="size-1.5 rounded-full bg-brand" aria-hidden />
            <span className="text-t-h2 font-medium tracking-tight">secall</span>
          </div>
          {appVersion && (
            <span
              className="font-mono text-t-mono text-text-3"
              aria-label={`Version ${appVersion}`}
            >
              {appVersion}
            </span>
          )}
        </div>

        {/* Primary nav */}
        <nav className="flex items-center gap-ds-1" aria-label="Primary">
          {NAV_ITEMS.map(({ to, label, hint }) => (
            <NavLink
              key={to}
              to={to}
              title={`${label} (${hint})`}
              className={({ isActive }) =>
                [
                  "px-ds-3 py-ds-1 rounded-md text-t-body transition-colors duration-fast ease-ds",
                  isActive
                    ? "text-text font-medium bg-surface-2"
                    : "text-text-3 hover:text-text hover:bg-surface-2",
                ].join(" ")
              }
            >
              {label}
            </NavLink>
          ))}
        </nav>

        {/* Center search slot — sessions/wiki 라우트에서만 노출 */}
        <div className="flex-1 flex items-center justify-center min-w-0">
          {showSearch && <HeaderSearch modes={modes} />}
        </div>

        {/* Right icons */}
        <div className="flex items-center gap-ds-1 shrink-0">
          <button
            type="button"
            aria-label="설정"
            title="Settings (g x)"
            onClick={() => navigate("/settings")}
            className="size-7 inline-flex items-center justify-center rounded-md text-text-3 hover:text-text hover:bg-surface-2 transition-colors duration-fast ease-ds"
          >
            <Settings className="size-4" />
          </button>
          <button
            type="button"
            aria-label="단축키 도움말"
            title="단축키 (?)"
            onClick={() => setHelpOpen(true)}
            className="size-7 inline-flex items-center justify-center rounded-md text-text-3 hover:text-text hover:bg-surface-2 transition-colors duration-fast ease-ds"
          >
            <Keyboard className="size-4" />
          </button>
          <button
            type="button"
            aria-label={dark ? "라이트 모드로 전환" : "다크 모드로 전환"}
            title={dark ? "라이트" : "다크"}
            onClick={toggle}
            className="size-7 inline-flex items-center justify-center rounded-md text-text-3 hover:text-text hover:bg-surface-2 transition-colors duration-fast ease-ds"
          >
            {dark ? <Sun className="size-4" /> : <Moon className="size-4" />}
          </button>
        </div>
      </div>
    </header>
  );
}
