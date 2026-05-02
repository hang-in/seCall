---
type: task
status: draft
updated_at: 2026-05-03
plan_slug: p38-rest-session-repo
task_id: 03
parallel_group: C
depends_on: []
---

# Task 03 — `session_repo` helper 회귀 통합 — P32~P37 신규 메서드

## Changed files

신규:
- `crates/secall-core/tests/session_repo_helpers.rs` — 단일 통합 테스트 파일. P32~P37 동안 추가된 session_repo helper 들을 한 자리에 모아 회귀.

수정: 없음 (production 코드 무수정)

## Change description

### 대상 helper (12+ 메서드)

| Helper | 도입 phase | 검증 시나리오 |
|---|---|---|
| `update_session_tags(id, tags)` | P32 | 정규화 (소문자 + `-`), dedup, 미존재 id error |
| `update_session_favorite(id, bool)` | P32 | true/false 토글, 미존재 id error |
| `update_session_notes(id, notes)` | P34 | text/null 양쪽, 미존재 id error |
| `list_sessions_filtered(filter)` | P32 + P34 | project/agent/tag/tags(다중)/favorite/since/page 단독 + 조합 |
| `list_all_tags()` | P35 | 빈 DB → 빈 Vec, 다중 세션 + 다중 tag → count DESC + name ASC 정렬 |
| `get_session_stats(id)` | P34 | turn role 분포 + tool 사용 빈도 카운트 정확성 |
| `get_session_list_item(id)` | P32 | 단일 세션 메타 (id/tags/is_favorite/turn_count/start_time/summary) |
| `update_semantic_extracted_at(id, ts)` | P37 | timestamp set, 미존재 0 affected (no error) |
| `list_sessions_for_graph_rebuild(filter)` | P37 | session/all/retry_failed/since 우선순위 5 분기 모두 행사 |
| `list_projects()` | P32 | distinct + sort |
| `list_agents()` | P32 | distinct + sort |
| `count_sessions()` | P32 | 0/N |

기존 inline 테스트 (P32~P37 task 별로 흩어진 회귀) 와 의도적 일부 중복 — 본 파일이 "단일 진입점" 역할.

### 공통 fixture

```text
fn make_db() -> (Database, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::open(dir.path().join("test.db")).expect("open");
    (db, dir)
}

fn insert_minimal_session(db: &Database, id: &str, project: &str, idx: i64) {
    // P32~P37 호환 minimal session row insert
}
```

`tests/rest_listing.rs::make_session` 와 동일 패턴 — 직접 import 안 하고 본 파일에 helper 복제 (integration test crate 분리 특성).

### 시나리오 카테고리

**1. 태그/즐겨찾기/노트 (P32+P34)** — 8 tests
- update_*_*: 양쪽 값 + 미존재 + 정규화/dedup

**2. 필터링 (P32+P34)** — 6 tests
- 단독 필터: project/agent/tag/tags/favorite/since/page
- 조합: project + favorite, multi-tag + project, since + tag

**3. 통계 (P34)** — 3 tests
- get_session_stats: turn 0 / 다양한 role / 다양한 tool 사용

**4. /api/tags (P35)** — 3 tests
- list_all_tags: 빈/단일/다중 세션, 정렬 검증

**5. 그래프 sync (P37)** — 5 tests
- update_semantic_extracted_at + list_sessions_for_graph_rebuild 5 분기 모두 (`tests/rest_listing.rs` 의 `rest_list_all_tags*` 와 일부 중복 OK)

**6. 메타 (P32)** — 3 tests
- get_session_list_item / list_projects / list_agents

총 28 tests 목표.

### 응답 형태 검증

- DB 조회 결과: Vec 길이, 단일 row 의 주요 필드 값, 정렬 순서
- 미존재: Result::Err 또는 0 affected (helper contract 에 따라)

## Dependencies

- 외부 crate: 없음
- 내부 task: 없음 (Task 00 의 인프라 의존하지 않음 — DB 직접 접근)

## Verification

```bash
cargo check --tests
cargo clippy --tests --all-features -- -D warnings
cargo fmt --all -- --check
cargo test -p secall-core --test session_repo_helpers
```

20+ tests 통과 목표.

## Risks

- **기존 inline 테스트와 중복**: 의도적. 본 파일이 P32~P37 helper "한 자리 회귀" 역할 — 신규 helper 추가 시 본 파일에 먼저 추가하는 컨벤션 정착.
- **schema 의존**: v8 마이그레이션이 자동 실행 → semantic_extracted_at 컬럼 자동 생성. 추가 setup 불필요.
- **Integration test crate 분리**: `tests/common/mod.rs` (Task 00) 를 본 파일에서 import 가능하지만, 본 task 는 Task 00 의존성 회피 위해 helper 복제. 의도적.
- **rusqlite Connection !Send**: Database 가 내부적으로 Mutex<Connection> 보유 → 테스트에서 직접 새 인스턴스 사용 OK.
- **정렬 순서 fragility**: list_all_tags 가 count DESC, name ASC. count tie 일 때 name 기준 검증.

## Scope boundary

수정 금지:
- `crates/secall-core/src/` 전체 production 코드
- 기존 `tests/{rest_listing,jobs_rest,graph_incremental}.rs` — 보존
- `crates/secall-core/tests/common/mod.rs` (Task 00) — 본 task 는 의존하지 않음
- `crates/secall-core/tests/rest_routes.rs` — Task 01/02 영역
- `crates/secall/`, `web/`, `README*`, `.github/`
