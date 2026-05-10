import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  envVar: string;
}

export function MaskedKeyInfoModal({ open, onOpenChange, envVar }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader className="space-y-ds-1">
          <DialogTitle className="text-t-h1">Gemini API key 안내</DialogTitle>
          <DialogDescription className="text-t-small text-text-3">
            보안상 이 값은 설정 화면에서 직접 편집하지 않고 환경변수나 `.env` 파일로만 관리합니다.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-ds-4 text-t-small text-text-2">
          <section className="rounded-xl border border-hairline bg-[var(--surface)] p-ds-4">
            <div className="text-t-h2 font-medium text-text">권장 위치</div>
            <p className="mt-ds-2 text-text-3">
              사용자 전역은 `~/.config/secall/.env`, 프로젝트 한정은 작업 디렉터리의 `.env` 를 사용합니다.
            </p>
          </section>

          <section className="rounded-xl border border-hairline bg-surface-2 p-ds-4">
            <div className="text-t-h2 font-medium text-text">예시</div>
            <pre className="mt-ds-2 overflow-x-auto rounded-lg bg-[var(--bg)] p-ds-3 text-t-meta text-text">
{`${envVar}=your-secret-key
chmod 600 ~/.config/secall/.env`}
            </pre>
          </section>

          <p className="text-text-3">
            Git 추적 파일에는 secret을 넣지 마세요. UI에서는 값 존재 여부만 표시합니다.
          </p>
        </div>
      </DialogContent>
    </Dialog>
  );
}
