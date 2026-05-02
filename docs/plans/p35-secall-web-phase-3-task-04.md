---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p35-secall-web-phase-3
task_id: 04
parallel_group: C
depends_on: [00, 01, 02, 03]
---

# Task 04 — README + CI 업데이트

## Changed files

수정:
- `README.md` — Phase 3 섹션 추가 (성능 + 정확도), `/api/tags` 엔드포인트 추가, changelog 행 추가
- `README.en.md` — 동일 영문판
- (선택) `.github/workflows/ci.yml` — 변경 없음 (기존 web-build job이 typecheck + build로 자동 검증)

신규: 없음

## Change description

### 1. README — Phase 3 섹션 추가

기존 Phase 0/1/2 다음에 추가:

```markdown
**Phase 3** (P35, 성능 + 정확도):
- `/api/tags` 엔드포인트 — 모든 태그 + 사용 빈도 정확 노출 (sessions 100건 휴리스틱 제거)
- SessionList 무한 스크롤 — IntersectionObserver 기반 자동 로드
- Code-split — 라우트별 + vendor(react/query/radix/viz) chunk 분리, 초기 번들 ≤ 350 kB (gzip)
```

### 2. README — 엔드포인트 목록 갱신

기존 엔드포인트 섹션에 추가:

```markdown
- 태그 목록 (Phase 3): GET /api/tags?with_counts={true|false}
  - true (기본): { "tags": [{ "name": "rust", "count": 12 }, ...] }
  - false: { "tags": ["rust", "search", ...] }
```

### 3. README — changelog 행

기존 changelog 표 상단에 추가:

```markdown
| 2026-XX-XX | v0.6.0 | Web UI Phase 3 (P35): /api/tags 엔드포인트, SessionList 무한 스크롤, Code-split (vendor + per-route chunk, 초기 번들 ≤ 350 kB gzip) |
```

날짜는 머지 시점으로 갱신. Cargo.toml 버전 bump는 별도 release tagging 시.

### 4. README.en.md 동기화

위 한글 섹션을 영문으로 동등하게 추가:

```markdown
**Phase 3** (P35, performance + accuracy):
- `/api/tags` endpoint — accurate full tag set with usage counts (replaces 100-session heuristic)
- SessionList infinite scroll — IntersectionObserver-based auto-load
- Code-split — per-route + vendor (react/query/radix/viz) chunks, initial bundle ≤ 350 kB (gzip)
```

엔드포인트와 changelog도 동일하게 영문으로 추가.

### 5. CI 변경 없음

기존 `.github/workflows/ci.yml`의 `web-build` job:
- `pnpm install --frozen-lockfile`
- `pnpm typecheck`
- `pnpm build`

이 자동으로 다음을 검증:
- Task 03 무한 스크롤 코드 → typecheck 통과
- Task 04 manualChunks 설정 → build 성공
- chunk 분리는 빌드 산출물로만 확인 가능 (CI 출력에 chunk 파일 크기 표시됨)

별도 검증 스크립트 추가는 본 task 외.

## Dependencies

- 외부: 없음
- 내부 task: Task 00 (`/api/tags` 엔드포인트), Task 01 (`useAllTags` 전환), Task 02 (무한 스크롤), Task 03 (code-split) 모두 완료 후 정확한 정보 반영 가능

## Verification

```bash
grep -q "Phase 3" /Users/d9ng/privateProject/seCall/README.md && echo "ko Phase 3 OK"
grep -q "Phase 3" /Users/d9ng/privateProject/seCall/README.en.md && echo "en Phase 3 OK"
grep -q "/api/tags" /Users/d9ng/privateProject/seCall/README.md && echo "tags endpoint listed"
grep -q "/api/tags" /Users/d9ng/privateProject/seCall/README.en.md && echo "tags endpoint listed (en)"
grep -q "무한 스크롤\|infinite scroll" /Users/d9ng/privateProject/seCall/README.md && echo "infinite scroll mentioned"
grep -q "Code-split\|code-split\|코드 분할" /Users/d9ng/privateProject/seCall/README.md && echo "code-split mentioned"

# CI 변경 없음 확인 (P34 task 09에서 추가된 web-build job 그대로)
git diff --stat .github/workflows/ | head -3 && echo "CI workflow unchanged"
```

## Risks

- **README 일관성**: 사용자가 보는 동작과 README 설명이 어긋나면 신뢰 저하. Task 00~03 검증 통과 후 본 task 진행.
- **chunk 크기 350 kB**: README에 "초기 번들 ≤ 350 kB gzip" 표기. Task 03의 실제 빌드 결과가 다르면 README 수치 갱신 필요.
- **버전 bump**: 본 task에서 Cargo.toml 버전 변경 안 함. release tagging은 별도.
- **changelog 날짜 placeholder**: `2026-XX-XX`는 머지 시점에 정확한 날짜로 갱신.

## Scope boundary

수정 금지:
- `crates/`, `web/src/` 코드 — Task 00~03 완료 후 본 task는 문서만
- `web/vite.config.ts` — Task 03 영역
- DB 스키마 — 본 phase에 변경 없음
- `.github/workflows/*` — 변경 없음 (단, 필요 시 본 task 범위 내에서 chunk 검증 추가는 OK)
