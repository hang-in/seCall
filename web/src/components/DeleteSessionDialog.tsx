import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { SessionListItem as Session } from "@/lib/types";

interface Props {
  /** null이면 닫힘. 세션이 지정되면 열림. */
  session: Session | null;
  isDeleting: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * 세션 삭제 확인 모달.
 *
 * - 제목: "세션을 삭제하시겠습니까?"
 * - 삭제(destructive=빨강) / 취소(outline=중립) 버튼
 * - 삭제는 되돌릴 수 없음을 명시.
 */
export function DeleteSessionDialog({
  session,
  isDeleting,
  onConfirm,
  onCancel,
}: Props) {
  return (
    <Dialog
      open={!!session}
      onOpenChange={(open) => {
        if (!open && !isDeleting) onCancel();
      }}
    >
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>세션을 삭제하시겠습니까?</DialogTitle>
          <DialogDescription>
            이 작업은 되돌릴 수 없습니다. 세션과 모든 turn·검색 인덱스가 영구
            삭제됩니다.
            {session ? (
              <span className="mt-2 block font-mono text-xs text-text-2">
                {session.project ? `${session.project} · ` : ""}
                {session.agent} · {session.date}
              </span>
            ) : null}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={isDeleting}>
            취소
          </Button>
          <Button
            variant="destructive"
            onClick={onConfirm}
            disabled={isDeleting}
          >
            {isDeleting ? "삭제 중…" : "삭제"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
