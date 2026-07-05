# seCall Docs

`docs/` 문서베이스의 최상위 인덱스. 어떤 문서군이 어디에 있는지 여기서 고른다.

> 문서를 어디에 두고 어떻게 관리하는지(파일명·frontmatter·index·아카이브)는
> [reference/docConventions.md](reference/docConventions.md) 가 SSOT 다.

## 새 세션 진입 순서

1. 프로젝트 루트 [`CLAUDE.md`](../CLAUDE.md) — 전체 규약 + 현재 상태 요약
2. [reference/index.md](reference/index.md) → 최신 handoff + 백로그로 현재 맥락 파악
3. 작업과 관련된 [plans](plans/index.md) / [prompts](prompts/index.md) 만 선택적으로

## 문서군 (8개 디렉토리)

| 디렉토리 | 역할 | 인덱스 |
|---|---|---|
| [reference/](reference/index.md) | 현재 기준 사실(SSOT) — 구현 현황·데이터 모델·설정·ADR·백로그 | [index](reference/index.md) |
| [plans/](plans/index.md) | 앞으로 할 일 — 작업 계획, 목표/비목표/완료 기준 | [index](plans/index.md) |
| [prompts/](prompts/index.md) | 실행 에이전트용 작업 지시문 (대응 plan과 함께 읽음) | [index](prompts/index.md) |
| [agents/](agents/) | 에이전트 역할 정의 (Developer / Architect / Reviewer) | — |
| [baseline/](baseline/) | 기준선 데이터/스냅샷 (성능·회귀 비교) | — |
| [community/](community/) | 외부 공개·커뮤니티용 문서, 릴리스 노트 | — |
| [insight/](insight/) | 분석·회고·인사이트 (의사결정 근거) | — |
| [reviews/](reviews/) | 코드/PR 리뷰 기록 (같은 영역 재작업 시 참고) | — |

## 자주 찾는 기준 문서

- [reference/docConventions.md](reference/docConventions.md) — 문서 규약 (SSOT)
- [reference/roadmap.md](reference/roadmap.md) — 전체 로드맵
- [reference/api.md](reference/api.md) — REST API 레퍼런스
- [reference/core-backlog.md](reference/core-backlog.md) · [reference/web-backlog.md](reference/web-backlog.md) — 현 우선순위
