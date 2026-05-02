---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p37-graph-sync
task_id: 00
parallel_group: A
depends_on: []
---

# Task 00 — DB 스키마 v8 + state tracking (`semantic_extracted_at`)

## Changed files

수정:
- `crates/secall-core/src/store/schema.rs:1` — `CURRENT_SCHEMA_VERSION` 7 → 8.
- `crates/secall-core/src/store/schema.rs:3` — `CREATE_SESSIONS` 본문에 `semantic_extracted_at INTEGER` 컬럼 추가 (NULL 허용 — 미처리 세션 표현). 신규 DB 는 처음부터 v8 스키마.
- `crates/secall-core/src/store/db.rs:113` 인접 — v7 마이그레이션 분기 다음에 `if current < 8 && !self.column_exists("sessions", "semantic_extracted_at")?` ALTER TABLE 분기 추가. P34 v7 패턴 그대로.
- `crates/secall-core/src/store/db.rs` — 기존 마이그레이션 회귀 + 신규 v8 회귀 테스트 추가:
  - `test_v8_semantic_extracted_at_column_exists` — open 후 컬럼 존재 검증
  - `test_v8_migrates_v6_db` — v6 schema 로 만든 DB 를 open 시 v8 까지 마이그레이션 적용 + 기존 row 보존
  - `test_update_semantic_extracted_at_*` — 신규 helper 회귀
  - `test_list_sessions_for_graph_rebuild_*` — 신규 helper 회귀 (since / session / all / retry-failed 4 케이스)
- `crates/secall-core/src/store/session_repo.rs` — 신규 helper 메서드 2 개 (기존 `list_sessions_filtered` 패턴 따라):
  - `update_semantic_extracted_at(&self, session_id: &str, ts: i64) -> Result<()>` — 단일 세션 timestamp set
  - `list_sessions_for_graph_rebuild(&self, filter: GraphRebuildFilter) -> Result<Vec<String>>` — 처리 대상 세션 id 목록 반환
- `crates/secall-core/src/store/session_repo.rs` — 신규 구조체:
  - `pub struct GraphRebuildFilter { pub since: Option<String>, pub session: Option<String>, pub all: bool, pub retry_failed: bool }`

신규: 없음 (기존 파일 확장만)

## Change description

### 1. 스키마 업그레이드 (schema.rs)

`CURRENT_SCHEMA_VERSION` 상수 1 증가. `CREATE_SESSIONS` SQL 본문 끝에 `semantic_extracted_at INTEGER` 한 줄 추가 (NULL 허용 = 미처리). FOREIGN KEY 등 기존 제약 변경 없음.

### 2. 마이그레이션 (db.rs)

P34 의 v7 (notes 컬럼) 마이그레이션 패턴을 그대로 모방:
- `if current < 8 && !column_exists("sessions", "semantic_extracted_at")?` 분기 안에서 단일 `ALTER TABLE sessions ADD COLUMN semantic_extracted_at INTEGER` 실행.
- 기존 row 의 `semantic_extracted_at` 은 NULL 로 초기화 → "처리 안 됨" 의미. ingest 시점 추출이 성공한 세션도 NULL — Task 02 가 시작 시 NULL 인 모든 세션을 retry-failed 모드로 처리하면 일괄 backfill 가능.
- 멱등성 보장: 재실행 시 column_exists 체크.
- 마이그레이션 끝에서 schema_version row 갱신은 기존 코드(line 117-120) 가 처리.

### 3. session_repo helper 시그니처 계약

```rust
pub struct GraphRebuildFilter {
    /// "YYYY-MM-DD" — 이 날짜 이후 시작된 세션만. None 이면 모든 날짜.
    pub since: Option<String>,
    /// 단일 세션 ID. 다른 필터 무시.
    pub session: Option<String>,
    /// true 면 모든 세션 (since/retry_failed 무시). session 보다 우선순위 낮음.
    pub all: bool,
    /// true 면 `semantic_extracted_at IS NULL` 인 세션만.
    pub retry_failed: bool,
}
```

`list_sessions_for_graph_rebuild` SQL 우선순위:
1. `session.is_some()` → 해당 ID 만 (단일 row)
2. `all == true` → 모든 sessions (필터 무시)
3. `retry_failed == true` → `WHERE semantic_extracted_at IS NULL`
4. `since.is_some()` → `WHERE start_time >= ?` (date 비교, ISO format)
5. 기본값 (모든 필드 비활성) → 빈 Vec 반환 → CLI/REST 가 "처리할 세션 없음" 안내

`ORDER BY start_time DESC` 일관 정렬.

`update_semantic_extracted_at` 은 `UPDATE sessions SET semantic_extracted_at = ?1 WHERE id = ?2`. 미존재 row 는 0 affected → 호출자가 결과 무시 가능.

### 4. SessionListItem 영향 범위

세션 리스트 응답 (`/api/sessions`) 에 `semantic_extracted_at` 노출이 필요하면 SessionListItem 에 필드 추가 + REST 응답에 포함 — **본 task 범위 외 (별도 phase)**. 본 task 는 graph rebuild 내부 처리에만 사용. 외부 노출은 P38+ 에서 결정.

### 5. 테스트 시나리오 (db.rs tests)

- `test_v8_semantic_extracted_at_column_exists` — 신규 DB open → 컬럼 있음
- `test_v8_migrates_v6_db` — v6 DB 직접 생성 → open → v8 마이그레이션 후 컬럼 있음 + 기존 sessions row 보존
- `test_update_semantic_extracted_at_sets_value` — 단일 세션에 1234 set → SELECT 로 1234 확인
- `test_update_semantic_extracted_at_missing_session_no_op` — 미존재 id 는 에러 안 나고 0 affected
- `test_list_sessions_for_graph_rebuild_session_only` — session ID 단일 반환
- `test_list_sessions_for_graph_rebuild_all_overrides_filters` — all=true 면 since/retry_failed 무시하고 모든 row
- `test_list_sessions_for_graph_rebuild_retry_failed_only_null` — semantic_extracted_at IS NULL 인 세션만
- `test_list_sessions_for_graph_rebuild_since_filters_by_date` — start_time >= since

## Dependencies

- 외부 crate: 없음
- 내부 task: 없음

## Verification

```bash
cargo check --all-targets
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall-core --lib store::db::tests::test_v8
cargo test -p secall-core --lib store::db::tests::test_update_semantic_extracted_at
cargo test -p secall-core --lib store::db::tests::test_list_sessions_for_graph_rebuild
```

## Risks

- **기존 row 의 NULL 의미**: ingest 시점에 시맨틱 추출 성공한 세션도 마이그레이션 직후엔 NULL. 사용자가 `--retry-failed` 로 일괄 backfill 가능 — non-issue, 오히려 의도적 reset.
- **마이그레이션 멱등성**: `column_exists` 체크 + ALTER TABLE 한 번만. P34 v7 패턴 검증됨.
- **NULL ordering**: `ORDER BY start_time DESC` 사용 → start_time 자체는 NULL 아님 (기존 schema NOT NULL 제약 추정). 확인은 디벨로퍼.
- **future schema 충돌**: 향후 v9 추가 시 기존 분기 그대로 유지하고 `if current < 9` 추가만 하면 됨. 본 task 의 `if current < 8` 분기는 호환.
- **tests/rest_listing.rs 회귀**: P32~35 에서 SessionListItem 시그니처 변경마다 영향 받음. 본 task 는 SessionListItem 미변경 → 회귀 없음.

## Scope boundary

수정 금지:
- `crates/secall-core/src/graph/`, `crates/secall-core/src/mcp/` — Task 01/02 영역과 무관
- `crates/secall/src/commands/graph.rs` — Task 01 영역
- `crates/secall/src/main.rs` — Task 01 영역 (graph 서브커맨드)
- `crates/secall-core/src/jobs/` — Task 02 영역
- `web/`, `README*`, `.github/` — Task 03/04 영역
- `crates/secall-core/src/store/jobs_repo.rs`, `tag_normalize.rs` — 무관
