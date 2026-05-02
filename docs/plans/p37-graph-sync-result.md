# Implementation Result: P37 — 시맨틱 Graph Sync 자동화

> Developer: unknown
> Branch: N/A
> Date: 2026-05-03 07:25
> Plan Revision: 1

---

## Summary

## Rework 완료 — 2건 모두 해결

**수정 내역:**

| # | 파일 | 변경 |
|---|---|---|
| 1 | `crates/secall/src/commands/ingest.rs:619-665` | sub-loop 진입 전 `let semantic_now_secs = chrono::Utc::now().timestamp();` 한 번 계산. `ExtractOneResult::Extracted(n)` 분기에 `db.update_semantic_extracted_at(session_id, semantic_now_secs)` 호출 추가 (실패는 warn 로그 — 자가 치유). graph rebuild 경로(graph.rs:280)와 동일 동작 → ingest 후 NULL 잔존 + `--retry-failed` 의미 손상 해결. |
| 2 | `web/src/components/JobOptionsDialog.tsx:354-362` | session 입력란 placeholder `(특정 session_id 한정)` → `full session_id (UUID)`. 안내 문구 `전체 또는 prefix(8+자리) ID` → `전체 session_id 정확히 입력 (backend 는 exact match — prefix 미지원)`. backend `WHERE id = ?1` exact match 와 일관. |

**Recommendation #1 처리:** `docs/plans/p37-graph-sync-task-03.md` 의 Changed files 와 Scope boundary 에 `JobItem.tsx` (exhaustive switch 강제) + `CommandsRoute.tsx` (grid 조정) 명시 — 계약과 구현 일관성 확보.

**Recommendation #2 처리:** axum route-level 통합 테스트는 별도 phase 권장 (현재 jobs_rest 테스트는 executor 직접 호출 수준 — 변경 없음).

**Verification:**
```
✅ cargo check --all-targets — exit 0
✅ cargo clippy --all-targets --all-features — secall (bin test) 의 사전 ENV_LOCK across await 경고 2건 (Task 01 보고에서 알려진 사항, 신규 코드 결함 아님)
✅ cargo fmt --all -- --check — clean (자동 적용 후)
✅ cargo test commands::graph::tests — 2 passed
✅ cargo test --test jobs_rest — 8 passed (graph_rebuild start + cancel 포함)
✅ pnpm typecheck — exit 0
```

**Scope 준수:** Task 00 (DB), Task 02 (REST), Task 04 (README) 코드 무수정. Task 01 영역 (ingest.rs 시맨틱 sub-loop) + Task 03 영역 (JobOptionsDialog UI 안내문) 만 수정.

## Subtask Results

### 1. ## Rework 완료 — 2건 모두 해결

**수정 내역:**

| # | 파일 | 변경 |
|---|---|---|
| 1 | `crates/secall/src/commands/ingest.rs:619-665` | sub-loop 진입 전 `let semantic_now_secs = chrono::Utc::now().timestamp();` 한 번 계산. `ExtractOneResult::Extracted(n)` 분기에 `db.update_semantic_extracted_at(session_id, semantic_now_secs)` 호출 추가 (실패는 warn 로그 — 자가 치유). graph rebuild 경로(graph.rs:280)와 동일 동작 → ingest 후 NULL 잔존 + `--retry-failed` 의미 손상 해결. |
| 2 | `web/src/components/JobOptionsDialog.tsx:354-362` | session 입력란 placeholder `(특정 session_id 한정)` → `full session_id (UUID)`. 안내 문구 `전체 또는 prefix(8+자리) ID` → `전체 session_id 정확히 입력 (backend 는 exact match — prefix 미지원)`. backend `WHERE id = ?1` exact match 와 일관. |

**Recommendation #1 처리:** `docs/plans/p37-graph-sync-task-03.md` 의 Changed files 와 Scope boundary 에 `JobItem.tsx` (exhaustive switch 강제) + `CommandsRoute.tsx` (grid 조정) 명시 — 계약과 구현 일관성 확보.

**Recommendation #2 처리:** axum route-level 통합 테스트는 별도 phase 권장 (현재 jobs_rest 테스트는 executor 직접 호출 수준 — 변경 없음).

**Verification:**
```
✅ cargo check --all-targets — exit 0
✅ cargo clippy --all-targets --all-features — secall (bin test) 의 사전 ENV_LOCK across await 경고 2건 (Task 01 보고에서 알려진 사항, 신규 코드 결함 아님)
✅ cargo fmt --all -- --check — clean (자동 적용 후)
✅ cargo test commands::graph::tests — 2 passed
✅ cargo test --test jobs_rest — 8 passed (graph_rebuild start + cancel 포함)
✅ pnpm typecheck — exit 0
```

**Scope 준수:** Task 00 (DB), Task 02 (REST), Task 04 (README) 코드 무수정. Task 01 영역 (ingest.rs 시맨틱 sub-loop) + Task 03 영역 (JobOptionsDialog UI 안내문) 만 수정.

