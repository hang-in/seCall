---
type: plan
status: in_progress
updated_at: 2026-04-06
version: 1
---

# seCall Refactor P3 — 품질 기반 + 즉시 실행

## Description

통합 분석 보고서 재검토 결과 유효한 finding 중 Small 공수 항목을 묶어 즉시 실행한다.
CI/CD 파이프라인 구축, async Mutex 안전성 확보, 입력 검증 강화, 쿼리 확장 캐싱 도입.

## Expected Outcome

- GitHub Actions CI가 PR마다 check / test / clippy / fmt 자동 실행
- OrtEmbedder가 `spawn_blocking` 내에서 ONNX 추론 실행 (tokio 워커 블로킹 해소)
- 잘못된 `--since` 입력 시 명시적 경고, 세션 ID 정확 매칭, project명 sanitize
- 동일 쿼리 재검색 시 캐시 히트로 subprocess 생략
- 토큰 1000 미만 시 실제 수치 표시 (0k 문제 해소)

## Subtasks

| # | Title | 공수 | parallel_group | depends_on |
|---|-------|------|---------------|------------|
| 01 | CI/CD GitHub Actions 구축 | Small | A | — |
| 02 | async Mutex → spawn_blocking | Small | A | — |
| 03 | 입력 검증 강화 | Small | A | — |
| 04 | 쿼리 확장 캐싱 | Small | B | 01 (CI 안전망) |

## Constraints

- `cargo test` 기준 현재 122 passed / 9 ignored 유지
- clippy 경고 0건 달성 (기존 7건 경고 정리 포함)
- 기존 테스트 회귀 없음

## Non-goals

- CD (배포 자동화) — CI만 우선
- ort stable 마이그레이션 — stable 미출시
- ANN 인덱스 — P4로 분리
- Database Repository 패턴 — P4로 분리
- typed error 도입 — P4로 분리
