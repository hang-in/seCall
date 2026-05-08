import { Outlet } from "react-router";
import { GraphOverlay } from "@/components/GraphOverlay";
import { HotkeyHelpDialog } from "@/components/HotkeyHelpDialog";
import { JobBanner } from "@/components/JobBanner";
import { JobToastListener } from "@/components/JobToastListener";
import { TopNav } from "@/components/TopNav";
import { useGlobalHotkeys } from "@/hooks/useGlobalHotkeys";

/**
 * 앱 셸 — 상단 nav + JobBanner + 본문 + 전역 overlays.
 * (Stage 2a: 좌측 사이드바 → 상단 nav 로 재구성, Calm/Editorial 톤)
 */
export default function Layout() {
  useGlobalHotkeys();

  return (
    <div className="flex h-screen flex-col bg-[var(--bg)] text-[var(--text)]">
      <JobToastListener />
      <TopNav />
      <JobBanner />
      <main className="flex-1 overflow-hidden min-w-0 flex flex-col">
        <Outlet />
      </main>
      <GraphOverlay />
      <HotkeyHelpDialog />
    </div>
  );
}
