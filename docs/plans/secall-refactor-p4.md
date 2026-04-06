---
type: plan
status: draft
updated_at: 2026-04-06
version: 1
---

# seCall Refactor P4 — 아키텍처 개선

## Description

P3 CI 안전망 확보 후 진행하는 Medium 공수 아키텍처 개선.
typed error 도입으로 에러 처리 정밀도 향상, Database impl 분산을 Repository 패턴으로 정리, `--vec` 전용 검색의 O(n) 스캔을 ANN 인덱스로 대체.

## Expected Outcome

- `secall-core`가 `SecallError` enum을 반환하여 MCP 서버가 에러 종류별 적절한 응답 코드 반환
- `impl Database` 블록 3개가 trait 기반으로 정리되어 코드 탐색성 향상
- `--vec` 전용 검색이 O(n) → O(log n) (usearch HNSW 기반)

## Subtasks

| # | Title | 공수 | parallel_group | depends_on |
|---|-------|------|---------------|------------|
| 01 | typed error 도입 (SecallError enum) | Medium | A | — |
| 02 | Database Repository 패턴 | Medium | B | 01 |
| 03 | ANN 인덱스 도입 (--vec 전용) | Medium | B | 01 |

## Constraints

- P3 CI 통과 상태에서 시작
- 기존 테스트 전체 통과 (P3 추가분 포함)
- `secall-core` public API 하위 호환성 유지 (trait 추가 OK, 기존 함수 삭제 NO)

## Non-goals

- Vault index.md/log.md 원자성 — 잔존 리스크이나 실제 데이터 손실 사례 없으므로 보류
- 청킹 알고리즘 개선 — LOW 우선순위, 별도 검토
- ort stable 마이그레이션 — stable 미출시
- sqlite-vec 재도입 — macOS arm64 C 컴파일 이슈 미해결
