import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useUi } from "@/lib/store";

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
  { group: "이동", keys: "g g", desc: "그래프 오버레이 토글" },
  { group: "리스트", keys: "j / k", desc: "다음 / 이전 항목" },
  { group: "리스트", keys: "Enter", desc: "선택 확정" },
  { group: "세션", keys: "[ / ]", desc: "이전 / 다음 세션" },
  { group: "세션", keys: "f", desc: "현재 세션 즐겨찾기 토글" },
  { group: "세션", keys: "e", desc: "현재 세션 노트 편집" },
];

const GROUP_ORDER = ["도움말", "전역", "이동", "리스트", "세션"];

function Kbd({ keys }: { keys: string }) {
  // "g s" 또는 "j / k" 같이 공백/슬래시로 분리해서 각 키를 <kbd>로 감싼다.
  // 슬래시는 구분자로 노출.
  const tokens = keys.split(/(\s+|\/)/).filter((t) => t.trim() !== "");
  return (
    <span className="inline-flex items-center gap-1">
      {tokens.map((tok, i) =>
        tok === "/" ? (
          <span key={i} className="text-muted-foreground text-xs">
            /
          </span>
        ) : (
          <kbd
            key={i}
            className="inline-flex h-5 min-w-[1.25rem] items-center justify-center rounded border border-border bg-muted px-1.5 text-[11px] font-mono text-foreground"
          >
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
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>키보드 단축키</DialogTitle>
          <DialogDescription>
            <kbd className="inline-flex h-5 min-w-[1.25rem] items-center justify-center rounded border border-border bg-muted px-1.5 text-[11px] font-mono">
              ?
            </kbd>
            를 다시 눌러 닫을 수 있습니다.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4 max-h-[60vh] overflow-y-auto pr-2">
          {grouped.map(({ group, items }) => (
            <section key={group} className="space-y-1.5">
              <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                {group}
              </h3>
              <table className="w-full text-sm">
                <tbody>
                  {items.map((h) => (
                    <tr key={`${group}-${h.keys}`} className="border-t border-border/50">
                      <td className="py-1.5 pr-4 w-32">
                        <Kbd keys={h.keys} />
                      </td>
                      <td className="py-1.5 text-foreground">{h.desc}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
