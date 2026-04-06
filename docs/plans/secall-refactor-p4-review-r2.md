# Review Report: seCall Refactor P4 — 아키텍처 개선 — Round 2

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 17:24
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/ingest/mod.rs:20 — `SessionParser::parse`가 여전히 `anyhow::Result<Session>`를 반환합니다. 이 상태에서는 파서 계층에서 발생한 실패를 `SecallError::Parse`로 일관되게 전파할 수 없어, Task 01의 typed error 도입 계약이 완결되지 않았습니다.
2. crates/secall-core/src/mcp/server.rs:103 — semantic recall 경로가 `search_with_embedding(...).unwrap_or_default()`로 벡터 검색 오류를 삼켜 버립니다. 또한 embed 실패도 111-113행에서 로그만 남기고 성공 응답으로 진행하므로, Task 01의 MCP 에러 매핑 개선 목표와 달리 클라이언트는 실제 장애를 빈 결과로 오인할 수 있습니다.
3. crates/secall-core/src/search/vector.rs:292 — ANN 인덱스 파일 경로가 고정(`ann_index.usearch`)인데, 로드 시 현재 embedder의 차원/모델과 기존 인덱스의 호환성을 검증하지 않습니다. 사용자가 1024차원 모델과 1536차원 모델 사이를 전환하면 stale 인덱스를 그대로 붙여 Task 03의 `--vec` 검색이 실패하거나 결과가 누락될 수 있습니다.

## Recommendations

1. `SessionParser` 경계부터 `crate::error::Result`를 사용하고, 각 파서 구현에서 경로 정보를 포함한 `SecallError::Parse`로 래핑하세요.
2. MCP `recall`의 keyword/semantic 경로 모두 `to_mcp_error` 기반으로 실패를 반환하도록 맞추고, `unwrap_or_default()`로 오류를 숨기지 마세요.
3. ANN 파일은 모델명/차원별로 분리하거나, 파일 메타데이터를 저장해 로드 시 차원 불일치면 폐기 후 재생성하도록 처리하세요.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | typed error 도입 (SecallError enum) | ✅ done |
| 2 | Database Repository 패턴 | ✅ done |
| 3 | ANN 인덱스 도입 (--vec 전용) | ✅ done |

