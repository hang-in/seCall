import { useHotkeys } from "react-hotkeys-hook";
import { useNavigate } from "react-router";
import { useUi } from "@/lib/store";

/**
 * 전역 단축키. Layout에서 한 번 호출.
 *
 * - `?` (shift+/): 도움말 다이얼로그 토글
 * - `/`: 검색 입력 포커스 (`input[data-hotkey="search"]`)
 * - `g d` / `g w` / `g s` / `g c`: 라우트 이동 (chord)
 * - `g g`: 그래프 오버레이 토글 (chord)
 * - `f`: 즐겨찾기 토글 (`[data-hotkey="favorite"]` 클릭)
 * - `e`: 노트 편집 (`[data-hotkey="notes"]` 포커스 + 가까운 details 펼침)
 *
 * 입력 필드 안에서는 react-hotkeys-hook 기본값으로 비활성. 단 `?`/도움말은
 * `enableOnFormTags` 옵션으로 input/textarea에서도 동작하게 둔다.
 */
export function useGlobalHotkeys() {
  const navigate = useNavigate();
  const toggleGraph = useUi((s) => s.toggleGraphOverlay);
  const toggleHelp = useUi((s) => s.toggleHelpDialog);

  // ?로 도움말 (form tags 안에서도 동작)
  useHotkeys(
    "shift+/",
    (e) => {
      e.preventDefault();
      toggleHelp();
    },
    { enableOnFormTags: ["input", "textarea"] },
  );

  // / 검색 포커스
  useHotkeys(
    "/",
    (e) => {
      e.preventDefault();
      const input = document.querySelector<HTMLInputElement>(
        'input[data-hotkey="search"]',
      );
      input?.focus();
      input?.select();
    },
  );

  // g d, g w, g s, g c — 라우트 이동 (chord)
  useHotkeys("g>d", () => navigate("/daily"));
  useHotkeys("g>w", () => navigate("/wiki"));
  useHotkeys("g>s", () => navigate("/sessions"));
  useHotkeys("g>c", () => navigate("/commands"));

  // g g — 그래프 오버레이 토글
  useHotkeys("g>g", () => toggleGraph());

  // f — 즐겨찾기 토글 (DOM 위임)
  useHotkeys("f", () => {
    const btn = document.querySelector<HTMLButtonElement>(
      '[data-hotkey="favorite"]',
    );
    btn?.click();
  });

  // e — 노트 편집기로 포커스. 가까운 details 요소가 닫혀있으면 펼친다.
  useHotkeys("e", () => {
    const target = document.querySelector<HTMLElement>(
      '[data-hotkey="notes"]',
    );
    if (!target) return;
    const anchor = target.closest<HTMLDetailsElement>(
      'details[data-hotkey-anchor="notes"]',
    );
    if (anchor && !anchor.open) {
      anchor.open = true;
    }
    // 다음 프레임에서 포커스 — details 펼친 직후에도 안정적으로 동작
    requestAnimationFrame(() => {
      target.focus();
      if (target instanceof HTMLTextAreaElement || target instanceof HTMLInputElement) {
        const len = target.value.length;
        target.setSelectionRange(len, len);
      }
    });
  });
}
