---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p36-job-cancellation
task_id: 03
parallel_group: C
depends_on: [00, 01, 02]
---

# Task 03 — README + CI 업데이트

## Changed files

수정:
- `README.md` — Phase 3 다음에 P36 Cancellation 항목 추가, `/api/jobs/{id}/cancel` 엔드포인트 설명 갱신 (NOT_IMPLEMENTED → 활성), v0.7.0 changelog 행 추가
- `README.en.md` — 동일 영문판
- `.github/workflows/ci.yml` — 변경 없음 (기존 cargo test job 이 신규 cancel 테스트 자동 실행)

신규: 없음

## Change description

### README 추가 항목

기존 Phase 3 섹션 다음에 P36 항목 추가. 한/영 양쪽 모두 같은 정보 반영:
- 실행 중 sync/ingest/wiki-update 작업 취소 가능
- web UI: JobBanner / JobItem 의 "취소" 버튼 + 확인 다이얼로그
- 안전 지점에서 중단 (phase 사이 / file 루프 / LLM 호출 직전)
- 부분 결과 보존 (예: ingest 50/100 진행 중 취소 → ingested=50)

### 엔드포인트 목록 갱신

기존 `POST /api/jobs/{id}/cancel` 가 README 에 NOT_IMPLEMENTED 로 표기되어 있다면 제거 후 다음 정보로 대체:
- 200 응답: `{ "cancelled": true, "job_id": "..." }`
- 404 응답: 미등록 / evict 됨
- 200 응답 (idempotent): 이미 완료/취소된 job

### Changelog 행 추가

상단에 신규 행 추가:
- 날짜: 머지 시점으로 갱신 (placeholder `2026-XX-XX`)
- 버전: v0.7.0
- 내용: P36 Job Cancellation 요약 — tokio CancellationToken 기반 + safe-point polling + REST 활성화 + web UI

Cargo.toml 버전 bump 는 별도 release tagging 시 진행 (본 task 범위 외).

### CI 변경 없음

`.github/workflows/ci.yml` 의 기존 cargo test job 이 Task 00 의 신규 cancel 통합 테스트를 자동 실행. workflow 변경 없음.

## Dependencies

- 외부: 없음
- 내부 task: Task 00 (백엔드 인프라), Task 01 (어댑터), Task 02 (web UI) 모두 완료 후 정확한 동작 반영 가능

## Verification

```bash
grep -qE "P36|Cancellation" /Users/d9ng/privateProject/seCall/README.md && echo "ko P36 OK"
grep -qE "P36|Cancellation" /Users/d9ng/privateProject/seCall/README.en.md && echo "en P36 OK"
grep -q "/api/jobs/.*/cancel" /Users/d9ng/privateProject/seCall/README.md && echo "cancel endpoint listed"
grep -q "/api/jobs/.*/cancel" /Users/d9ng/privateProject/seCall/README.en.md && echo "cancel endpoint listed (en)"
grep -qE "취소|cancel" /Users/d9ng/privateProject/seCall/README.md && echo "cancel mentioned"
git diff --stat .github/workflows/ | head -3
```

`cargo test --all` 같은 회귀는 Task 00/01 에서 이미 실행됨 → 본 task 는 docs only 라 skip.

## Risks

- **README 일관성**: 사용자가 보는 동작과 README 설명이 어긋나면 신뢰 저하. Task 00~02 검증 통과 후 본 task 진행.
- **버전 bump**: 본 task 에서 Cargo.toml 버전 변경 안 함. release tagging 별도.
- **changelog 날짜 placeholder**: `2026-XX-XX` 는 머지 시점에 정확한 날짜로 갱신.
- **이전 README "NOT_IMPLEMENTED" 문구**: 있다면 제거 또는 대체. 디벨로퍼가 grep 으로 확인.

## Scope boundary

수정 금지:
- `crates/`, `web/src/` 코드 — Task 00~02 완료 후 본 task 는 문서만
- DB 스키마 — 변경 없음
- `.github/workflows/*` — 변경 없음
