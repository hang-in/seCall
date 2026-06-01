---
type: plan
status: in_progress
updated_at: 2026-06-01
canonical: true
---

# P89 — 검색 품질: vector snippet enrich + 관찰 세션 랭킹 강등 (issue #100)

## 배경

Issue #100 (hang-in, codex 검색 품질 스모크 테스트). 전체 임베딩 완료 후 검색 품질 점검에서 두 개선점 확인:
1. `--vec` (vector-only) 결과의 `snippet` 이 비어 검증성 낮음
2. observer/관찰성 세션이 일반 검색 결과 상위에 섞이는 노이즈

## 원인 (코드 확인)

- **snippet 빈 값**: `vector.rs:299` (ANN 경로) + `341` (BLOB 스캔) 의 `SearchResult.snippet = String::new()`. BM25 (`bm25.rs:147`) 는 FTS content 로 `extract_snippet` 을 채우지만 vector 경로엔 그 단계가 없음. vector 검색은 `(session_id, turn_index)` 까지 알면서도 turn content 를 안 가져옴.
- **관찰 노이즈**: `recall.rs:55-58` 기본 `exclude_session_types = ["automated"]` 로 automated 만 제외. classify (`commands/classify.rs`, config rule 기반) 가 못 잡은 관찰성 세션은 `interactive` 로 남아 검색에 섞임.

## 목표

- A: vector 결과에 turn content 기반 snippet 채워 검증성 확보.
- B2: automated 제외는 유지 + classify 가 못 잡은 관찰성 세션(짧은 turn 수)을 랭킹에서 soft 강등 (제외 아님).

## 구현

### A. vector snippet enrich

- `bm25.rs:195` `fn extract_snippet` → `pub(crate) fn extract_snippet` (search 모듈 공유). 빈 query 시 `find("")` 가 `Some(0)` → 앞부분 추출이라 안전.
- `vector.rs`:
  - `search_with_embedding` 시그니처에 `query: Option<&str>` 추가.
  - ANN/BLOB 두 경로에서 `db.get_turn(&session_id, turn_index)` 로 turn content 가져와 `extract_snippet(content, query.unwrap_or(""), 200)` 으로 snippet 채움. get_turn 실패 시 빈 문자열 유지 (graceful).
  - `search` (async, line 240) 가 `Some(query)` 전달.
- `hybrid.rs:225` `vi.search_with_embedding(...)` 호출에 query 전달 (보유 시 Some, 없으면 None).

### B2. 관찰 세션 랭킹 강등

- `hybrid.rs` 의 `reciprocal_rank_fusion` 결과 정렬/정규화 **전**에, 각 result 의 `metadata.turn_count` 가 임계값 미만(예: `< 3`) 이면 RRF score 에 penalty (`*= 0.5`) 적용.
  - 근거: 관찰/요약 세션은 turn 수가 매우 적음. automated 로 분류 안 된 짧은 노이즈 세션을 하위로.
  - 임계값/penalty 는 상수로 (`OBSERVER_TURN_THRESHOLD = 3`, `OBSERVER_PENALTY = 0.5`).
- automated 완전 제외(`recall.rs`)는 그대로 유지 — 이건 별개 레이어.

## 변경 파일

- `crates/secall-core/src/search/bm25.rs` — `extract_snippet` pub(crate)
- `crates/secall-core/src/search/vector.rs` — search_with_embedding query param + snippet 채움 (2 경로)
- `crates/secall-core/src/search/hybrid.rs` — search_with_embedding 호출 갱신 + RRF turn_count penalty
- `docs/plans/p89-search-quality.md` (신규) + `docs/plans/index.md`

## 검증

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p secall-core --lib search::
```

- 신규 unit test: RRF penalty (turn_count < 3 인 result 가 강등되는지), extract_snippet 빈 query 동작.
- ⚠️ context-mode hook 이 Read 출력을 오염시키는 환경 → 검증은 cargo (Bash) 결과로.

## 리스크

- snippet 채우려 result 마다 `get_turn` 쿼리 1회 추가 → N+1. limit 작아(기본 10) 영향 미미. 필요 시 batch 조회 후속.
- turn_count 강등이 짧지만 중요한 세션을 누를 수 있음 → penalty 0.5 로 완만하게 (제외 아님).

## 후속 (별도)

- hybrid 결과에 BM25/vector hit 근거 표시 (#100 제안 4).
- 품질 회귀 테스트용 고정 쿼리 세트 (#100 제안 5).
