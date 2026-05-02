---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p37-graph-sync
task_id: 04
parallel_group: D
depends_on: [00, 01, 02, 03]
---

# Task 04 — README + CI 업데이트

## Changed files

수정:
- `README.md` — P36 다음에 P37 항목 추가, `secall graph rebuild` CLI 명령 + `POST /api/commands/graph-rebuild` 엔드포인트 추가, v0.8.0 changelog 행.
- `README.en.md` — 동일 영문판.
- `.github/workflows/ci.yml` — 변경 없음 (기존 cargo test job 이 Task 00/01/02 신규 통합 테스트 자동 실행).

신규: 없음

## Change description

### README — P37 섹션 추가

기존 P36 다음에 다음 정보 반영 (한/영 동일):
- 시맨틱 그래프 sync 자동화 — 이미 ingest 된 세션의 그래프 재구축 가능
- DB 스키마 v8: `sessions.semantic_extracted_at` 컬럼 — 처리 상태 추적
- CLI: `secall graph rebuild [--since DATE] [--session ID] [--all] [--retry-failed]`
- REST: `POST /api/commands/graph-rebuild` (Job 시스템 통합, P36 cancel 지원)
- web UI: Commands 페이지에 "Graph Rebuild" 카드 + 옵션 다이얼로그

### CLI 사용 예 (README)

```
# 미처리(NULL) 세션 일괄 backfill
secall graph rebuild --retry-failed

# 특정 날짜 이후 세션
secall graph rebuild --since 2026-04-01

# 단일 세션
secall graph rebuild --session abc12345

# 전체 재구축 (기존 결과도 덮어쓰기)
secall graph rebuild --all
```

우선순위: `--session` > `--all` > `--retry-failed` > `--since`. 같이 지정하면 위 순서로 적용.

### 엔드포인트 목록 갱신

```
- 그래프 재구축 (P37): POST /api/commands/graph-rebuild
  - body: { since?, session?, all?, retry_failed? }
  - 응답: { job_id, status: "started" }
  - 단일 큐 정책: 다른 mutating job 실행 중이면 409
```

### Changelog 행

상단에 추가:
```
| 2026-XX-XX | v0.8.0 | P37 시맨틱 Graph Sync 자동화: DB v8 (semantic_extracted_at), `secall graph rebuild` CLI, `POST /api/commands/graph-rebuild` REST, web UI 카드, P33 Job + P36 cancel 통합 |
```

날짜는 머지 시점으로 갱신. Cargo.toml 버전 bump 는 별도 release tagging.

### CI 변경 없음

- Task 00: `cargo test --lib store::db::tests::test_v8` 등이 기본 cargo test 로 자동 실행
- Task 01: `cargo test commands::graph::tests` 도 자동 실행
- Task 02: `cargo test --test jobs_rest test_graph_rebuild` 도 자동 실행
- Task 03: `pnpm typecheck` + `pnpm build` 가 web-build job 으로 자동 실행

workflow 파일 변경 없음.

## Dependencies

- 외부: 없음
- 내부 task: Task 00 (DB), Task 01 (CLI), Task 02 (REST), Task 03 (web UI) 모두 완료 후 정확한 동작 반영 가능

## Verification

```bash
grep -qE "P37|Graph Sync|graph-rebuild|graph rebuild" /Users/d9ng/privateProject/seCall/README.md && echo "ko P37 OK"
grep -qE "P37|Graph Sync|graph-rebuild|graph rebuild" /Users/d9ng/privateProject/seCall/README.en.md && echo "en P37 OK"
grep -q "/api/commands/graph-rebuild" /Users/d9ng/privateProject/seCall/README.md && echo "endpoint listed"
grep -q "/api/commands/graph-rebuild" /Users/d9ng/privateProject/seCall/README.en.md && echo "endpoint listed (en)"
grep -qE "semantic_extracted_at|시맨틱.*상태" /Users/d9ng/privateProject/seCall/README.md && echo "state tracking mentioned"
git diff --stat .github/workflows/ | head -3
```

`cargo test --all` 회귀는 Task 00/01/02 에서 이미 실행됨 → 본 task 는 docs only 라 skip.

## Risks

- **README 일관성**: 사용자가 보는 동작과 README 설명이 어긋나면 신뢰 저하. Task 00~03 검증 통과 후 본 task 진행.
- **버전 bump**: Cargo.toml 변경 없음 (release tagging 별도).
- **changelog 날짜 placeholder**: `2026-XX-XX` → 머지 시점에 정확한 날짜로 갱신.
- **우선순위 명시**: 사용자가 `--all` 과 `--retry-failed` 를 동시 지정 가능 → 우선순위 안내가 README 에 명시되어야 혼란 방지.
- **embedding 모델 unload 자동화 안내**: ingest.rs:614-632 의 unload 로직이 `run_rebuild` 에도 반영됐다는 점은 사용자에게 보일 필요 없음 (내부 동작) — README 에 굳이 안내 안 함.

## Scope boundary

수정 금지:
- `crates/`, `web/src/` 코드 — Task 00~03 완료 후 본 task 는 문서만
- DB 스키마 — 변경 없음 (Task 00 영역)
- `.github/workflows/*` — 변경 없음
