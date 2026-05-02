/** index 라우트 — 세션이 선택되지 않았을 때 우측 pane 안내. */
export function SessionEmptyState() {
  return (
    <div className="h-full flex items-center justify-center text-muted-foreground text-sm">
      좌측에서 세션을 선택하세요
    </div>
  );
}
