/** index 라우트 — 세션이 선택되지 않았을 때 우측 pane 안내 (Stage 3). */
export function SessionEmptyState() {
  return (
    <div className="h-full flex flex-col items-center justify-center text-text-3 px-ds-6">
      <div className="text-t-h2 text-text-2 mb-ds-2">세션을 선택하세요</div>
      <p className="text-t-small text-text-3 max-w-md text-center">
        왼쪽 리스트에서 항목을 클릭하면 turns, 메타, 관련 세션이 여기에 펼쳐집니다.
      </p>
      <div className="mt-ds-4 flex items-center gap-ds-3 text-t-meta text-text-3">
        <span className="inline-flex items-center gap-1">
          <kbd className="kbd">j</kbd>
          <kbd className="kbd">k</kbd>
          이동
        </span>
        <span aria-hidden className="text-text-4">·</span>
        <span className="inline-flex items-center gap-1">
          <kbd className="kbd">/</kbd>
          검색
        </span>
        <span aria-hidden className="text-text-4">·</span>
        <span className="inline-flex items-center gap-1">
          <kbd className="kbd">f</kbd>
          즐겨찾기
        </span>
      </div>
    </div>
  );
}
