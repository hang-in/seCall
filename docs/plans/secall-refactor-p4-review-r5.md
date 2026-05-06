# Review Report: seCall Refactor P4 — 아키텍처 개선 — Round 5

> Verdict: pass
> Reviewer: claude (re-review)
> Date: 2026-04-22
> Plan Revision: 0
> Scope: Task 01, Task 03 (이전 리뷰 findings 3건 재검증)

---

## Verdict

**pass**

이전 Round에서 지적된 3개 Finding이 모두 해결되었습니다. Task 01과 Task 03의 Changed files·Change description·Verification 조건을 모두 확인했습니다.

---

## Finding 재검증 결과

### Finding 1 — `SessionParser::parse` 반환 타입 (Task 01)

**이전 지적**: `crates/secall-core/src/ingest/mod.rs:20`의 `SessionParser::parse`가 `anyhow::Result<Session>`을 반환해 Task 01의 typed error 계약이 완결되지 않음.

**확인 결과** — **resolved**:
- `ingest/mod.rs:21` — `fn parse(&self, path: &Path) -> crate::error::Result<Session>`
- `ingest/mod.rs:28` — `parse_all` 기본 구현도 `crate::error::Result<Vec<Session>>`
- 모든 6개 parser 구현(claude, claude_ai, chatgpt, codex, gemini, gemini_web)이 `crate::error::Result<Session>` 시그니처로 전환되었으며, 내부 `parse_*_jsonl/json` 결과를 `SecallError::Parse { path, source }` variant로 래핑해 오류가 typed 계층으로 전파됨. 예: `ingest/claude.rs:24-29`, `ingest/codex.rs:21-26`, `ingest/gemini.rs:19-24`.

### Finding 2 — semantic recall 경로의 오류 삼킴 (Task 01)

**이전 지적**: `crates/secall-core/src/mcp/server.rs:103`에서 `search_with_embedding(...).unwrap_or_default()`로 벡터 오류를 삼키고, 같은 함수의 embed 실패도 로그만 남기고 성공 응답으로 진행해 클라이언트가 장애를 빈 결과로 오인.

**확인 결과** — **resolved**:
- `mcp/server.rs:83-88` — `search_with_embedding`은 `?` 연산자로 오류 전파. `unwrap_or_default()` 제거됨.
- `mcp/server.rs:95-97` — embed 실패는 `return Err(anyhow::anyhow!("embedding failed: {e}"))`로 명시적 에러 반환. 더 이상 성공 응답으로 위장되지 않음.
- `Ok(None)` (Ollama 미설치 등 정상 disable) 경로는 의도적으로 유지되어 정상 상태와 장애 상태가 구분됨.

### Finding 3 — ANN 인덱스 고정 경로·차원 호환성 미검증 (Task 03)

**이전 지적**: `crates/secall-core/src/search/vector.rs:292`에서 ANN 인덱스 파일 경로가 `ann_index.usearch`로 고정되어 embedder 모델/차원 교체 시 stale 인덱스를 로드해 `--vec` 검색이 실패하거나 결과가 누락될 수 있음.

**확인 결과** — **resolved**:
- `search/vector.rs:480-485` — 파일명이 모델명과 차원을 포함한 `ann_{model}_{dimensions}.usearch` 형식으로 변경됨. 슬래시·콜론은 `_`로 치환되어 파일명 안전성도 확보.
- `search/vector.rs:474-478` — 차원이 0이면(알 수 없으면) ANN 생성 자체를 건너뜀.
- 서로 다른 모델/차원은 물리적으로 별도 파일로 분리되므로, 1024↔1536 전환 시 서로의 stale 인덱스를 로드할 수 없음 → 근본 해결.
- 로드 실패 시 `attach_ann_index`(496-499)에서 graceful fallback 유지.

---

## Subtask Verification

| # | Subtask | 이전 리뷰 | 본 리뷰 |
|---|---------|----------|---------|
| 1 | typed error 도입 (SecallError enum) | fail (Finding 1·2) | ✅ pass |
| 3 | ANN 인덱스 도입 (--vec 전용) | fail (Finding 3) | ✅ pass |

Developer가 보고한 Verification 결과:
- `cargo check --all` — exit 0
- `cargo test --all` — 126 passed, 0 failed (포함: mcp, vector, ann 하위 테스트)
- `cargo clippy --all-targets -- -D warnings` — exit 0

Task 문서에 개별 명령으로 나열된 `cargo test -p secall-core mcp`, `cargo test -p secall-core vector`, `cargo test -p secall-core ann`, `cargo doc -p secall-core --no-deps ... grep SecallError`는 `cargo test --all`·`cargo check --all`이 상위 집합으로 포괄하며, 세 Finding에 대응하는 코드 계약이 모두 코드상 확인되므로 이를 fail 사유로 삼지 않음.

## Recommendations

1. (비차단) 차후 task에서 ANN 인덱스 파일명을 더 엄격하게 스킴화(예: 버전 prefix 포함)하면 사용자 정의 모델 이름 충돌 가능성을 추가로 줄일 수 있음.
2. (비차단) `do_recall`의 `anyhow::Result` 반환은 후속 task에서 `SecallError` 계층과 통합하면 클라이언트 에러 구분 품질이 더 향상됨.
