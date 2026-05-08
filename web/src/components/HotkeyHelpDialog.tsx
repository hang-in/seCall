import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useUi } from "@/lib/store";

/**
 * 단축키 도움말 — Calm/Editorial 톤 (Stage 2b).
 * 그룹별 카드 형태 + design-tokens 의 `.kbd` utility 활용.
 */

interface HotkeyEntry {
  keys: string;
  desc: string;
  group: string;
}

const HOTKEYS: HotkeyEntry[] = [
  { group: "도움말", keys: "?", desc: "이 도움말 열기/닫기" },
  { group: "전역", keys: "/", desc: "검색 입력 포커스" },
  { group: "전역", keys: "Esc", desc: "다이얼로그/오버레이 닫기" },
  { group: "이동", keys: "g s", desc: "Sessions" },
  { group: "이동", keys: "g d", desc: "Daily" },
  { group: "이동", keys: "g w", desc: "Wiki" },
  { group: "이동", keys: "g c", desc: "Commands" },
  { group: "이동", keys: "g g", desc: "Graph" },
  { group: "리스트", keys: "j / k", desc: "다음 / 이전 항목" },
  { group: "리스트", keys: "Enter", desc: "선택 확정" },
  { group: "세션", keys: "[ / ]", desc: "이전 / 다음 세션" },
  { group: "세션", keys: "f", desc: "현재 세션 즐겨찾기 토글" },
  { group: "세션", keys: "e", desc: "현재 세션 노트 편집" },
];

const GROUP_ORDER = ["도움말", "전역", "이동", "리스트", "세션"];

function Kbd({ keys }: { keys: string }) {
  // "g s" 또는 "j / k" 같이 공백/슬래시로 분리해서 각 키를 .kbd 로 감싼다.
  const tokens = keys.split(/(\s+|\/)/).filter((t) => t.trim() !== "");
  return (
    <span className="inline-flex items-center gap-1">
      {tokens.map((tok, i) =>
        tok === "/" ? (
          <span key={i} className="text-text-4 text-t-caption">
            /
          </span>
        ) : (
          <kbd key={i} className="kbd">
            {tok}
          </kbd>
        ),
      )}
    </span>
  );
}

export function HotkeyHelpDialog() {
  const open = useUi((s) => s.helpDialogOpen);
  const setOpen = useUi((s) => s.setHelpDialogOpen);

  const grouped = GROUP_ORDER.map((group) => ({
    group,
    items: HOTKEYS.filter((h) => h.group === group),
  })).filter((g) => g.items.length > 0);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="max-w-2xl">
        <DialogHeader className="space-y-ds-1">
          <DialogTitle className="text-t-h1">단축키</DialogTitle>
          <DialogDescription className="text-t-small text-text-3 flex items-center gap-1.5">
            <kbd className="kbd">?</kbd>
            <span>를 다시 눌러 닫을 수 있습니다.</span>
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-ds-3 max-h-[60vh] overflow-y-auto pt-ds-2">
          {grouped.map(({ group, items }) => (
            <section
              key={group}
              className="rounded-lg border border-hairline bg-surface-2 p-ds-3"
            >
              <div className="eyebrow mb-ds-2">{group}</div>
              <ul className="space-y-ds-1">
                {items.map((h) => (
                  <li
                    key={`${group}-${h.keys}`}
                    className="flex items-center justify-between gap-ds-3 py-ds-1 text-t-small"
                  >
                    <span className="text-text-2">{h.desc}</span>
                    <Kbd keys={h.keys} />
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
