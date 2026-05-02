import { useEffect, useRef } from "react";
import type { ProgressEvent } from "@/lib/types";

/**
 * Job SSE 스트림 구독 훅.
 *
 * 백엔드 엔드포인트: `GET /api/jobs/:id/stream` (text/event-stream).
 * 첫 프레임은 `initial_state`로 현재 JobState 스냅샷을 보내며, 이후 phase/progress/message/done/failed 이벤트가 따른다.
 * KeepAlive 15초.
 *
 * - `enabled = false`인 경우 연결을 만들지 않는다 (job이 이미 완료된 경우 호출자가 차단).
 * - EventSource의 기본 자동 재연결은 무한 루프를 일으킬 수 있어 onerror에서 close 후
 *   직접 setTimeout으로 5초 후 1회 재연결을 시도한다 (enabled 동안만).
 * - unmount/enabled 변경 시 close + clearTimeout으로 안전하게 정리한다.
 */
export function useJobStream(
  id: string | undefined,
  onEvent: (e: ProgressEvent) => void,
  enabled: boolean = true,
) {
  // onEvent를 ref에 저장하여 effect 의존성에서 제외 (호출자가 매 렌더 새 클로저 전달해도 재구독 안 일어남).
  const onEventRef = useRef(onEvent);
  useEffect(() => {
    onEventRef.current = onEvent;
  }, [onEvent]);

  useEffect(() => {
    if (!id || !enabled) return;

    let es: EventSource | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let closed = false;

    const open = () => {
      if (closed) return;
      es = new EventSource(`/api/jobs/${encodeURIComponent(id)}/stream`);
      es.onmessage = (m) => {
        try {
          const event = JSON.parse(m.data) as ProgressEvent;
          onEventRef.current(event);
        } catch {
          // SSE 페이로드가 JSON이 아닐 수 있음 (KeepAlive 주석 등) — 무시
        }
      };
      es.onerror = () => {
        es?.close();
        es = null;
        if (closed) return;
        // 5초 후 재연결 (cleanup에서 clearTimeout으로 막음)
        reconnectTimer = setTimeout(open, 5000);
      };
    };

    open();

    return () => {
      closed = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      es?.close();
    };
  }, [id, enabled]);
}
