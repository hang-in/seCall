import { useJobLifecycle } from "@/hooks/useJobLifecycle";

/**
 * P33 Task 06 — 전역 Job lifecycle toast listener.
 *
 * recent jobs를 5초 단위로 폴링하여 상태 변화 (active → completed/failed/interrupted)
 * 감지 시 sonner toast를 발행한다. 페이지 어디에서도 작업 결과 알림이 동작하도록
 * `Layout`의 main 영역에 한 번만 마운트되어야 한다.
 *
 * 본 컴포넌트는 UI 출력이 없는 listener-only다 — `useJobLifecycle` 훅에 모든 로직이 있고,
 * 컴포넌트는 마운트 위치 + lifecycle 측면에서 React 트리에 명시적으로 보이도록 하는 wrapper다.
 */
export function JobToastListener(): null {
  useJobLifecycle();
  return null;
}
