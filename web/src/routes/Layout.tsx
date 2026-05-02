import { NavLink, Outlet } from "react-router";
import { BookOpen, Calendar, Network, Play, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { GraphOverlay } from "@/components/GraphOverlay";
import { HotkeyHelpDialog } from "@/components/HotkeyHelpDialog";
import { JobBanner } from "@/components/JobBanner";
import { JobToastListener } from "@/components/JobToastListener";
import { useGlobalHotkeys } from "@/hooks/useGlobalHotkeys";
import { useUi } from "@/lib/store";

const NAV = [
  { to: "/sessions", icon: Search, label: "Sessions" },
  { to: "/daily", icon: Calendar, label: "Daily" },
  { to: "/wiki", icon: BookOpen, label: "Wiki" },
  { to: "/commands", icon: Play, label: "Commands" },
];

export default function Layout() {
  const toggleGraph = useUi((s) => s.toggleGraphOverlay);
  const graphOpen = useUi((s) => s.graphOverlayOpen);

  // P34 Task 04: 전역 단축키 등록
  useGlobalHotkeys();

  return (
    <div className="flex h-screen bg-background text-foreground">
      {/* P33 Task 06: 전역 job lifecycle toast listener (UI 미출력) */}
      <JobToastListener />
      <aside className="w-56 shrink-0 border-r border-border p-4 space-y-1 flex flex-col">
        <div className="text-lg font-semibold mb-6 px-3">seCall</div>
        {NAV.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `flex items-center gap-2 px-3 py-2 rounded-md text-sm ${
                isActive
                  ? "bg-accent text-accent-foreground"
                  : "hover:bg-accent/50 text-muted-foreground"
              }`
            }
          >
            <Icon className="size-4" /> {label}
          </NavLink>
        ))}
        <div className="flex-1" />
        <Button
          variant={graphOpen ? "secondary" : "ghost"}
          size="sm"
          onClick={toggleGraph}
          className="justify-start"
        >
          <Network className="size-4 mr-2" /> Graph
        </Button>
      </aside>

      {/* 본문 영역 — 자식 라우트가 좌/우 pane을 구성. 상단에 JobBanner를 sticky로 배치. */}
      <main className="flex-1 overflow-hidden min-w-0 flex flex-col">
        <JobBanner />
        <div className="flex-1 overflow-hidden min-w-0">
          <Outlet />
        </div>
      </main>
      <GraphOverlay />
      <HotkeyHelpDialog />
    </div>
  );
}
