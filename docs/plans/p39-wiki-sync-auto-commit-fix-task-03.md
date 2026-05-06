---
type: task
status: draft
updated_at: 2026-05-05
plan_slug: p39-wiki-sync-auto-commit-fix
task_id: 03
parallel_group: B
depends_on: []
---

# Task 03 — wiki 콘텐츠 양 측정 + P40 (wiki 벡터화) 우선순위 데이터

## Changed files

신규:
- `docs/baseline/p39-p40-decision.md` — wiki 콘텐츠 양 측정 + P40 (wiki 벡터화) 진행 여부 결정 보고서. 외부 컨트리뷰터 답변 (Task 05) 의 데이터 소스.

수정: 없음 (production 코드 무수정)

## Change description

### 측정 데이터 카테고리

**1. 페이지 수 — 분류별 카운트**
- 총 페이지 수 (`find vault/wiki -name "*.md" | wc -l`)
- `vault/wiki/projects/` 카운트
- `vault/wiki/topics/` 카운트
- 기타 카테고리 (있으면)

**2. 페이지 길이 분포**
- 평균 / 중앙값 / 최대 / 최소 (line 수 또는 word count)
- token 추정 (한국어 평균 1 word ≈ 1.5 token, 영어 1 word ≈ 1.3 token)
- 분포 hist (10개 구간 — 0-100 / 100-500 / 500-1000 / ... / 5000+ words)

**3. 검색 빈도 (가능하면)**
- DB `query_cache` 테이블 또는 access log 에서 wiki_search 호출 카운트 (최근 30일)
- 데이터 없으면 `N/A — 추적 인프라 부재` 명시 (별도 phase 후보)

**4. P40 진행 여부 결정 기준**

다음 3 기준 중 **어느 하나라도 충족** 시 P40 즉시 진행 권장:
- 페이지 수 ≥ 100
- 평균 길이 ≥ 5000 tokens
- 검색 빈도 ≥ 10/day (인프라 있을 때)

미충족 시 → P40 보류 + 외부 컨트리뷰터에게 데이터 + 의견 회신 (Task 05).

### 결정 트리 (보고서 마지막 섹션)

```text
## P40 진행 여부 결정

측정 결과:
- 페이지 수: N
- 평균 길이: M tokens
- 검색 빈도: K/day (또는 N/A)

기준 충족:
- 페이지 수 ≥ 100: [Y/N]
- 평균 길이 ≥ 5000 tokens: [Y/N]
- 검색 빈도 ≥ 10/day: [Y/N]

결정: [P40 즉시 진행 / 보류]
근거: ...

후속 액션:
- (즉시 진행 시) P40 plan-proposal 초안 작성 — chunker 재사용, embedding backend Ollama bge-m3, DB v9 wiki_vectors, hybrid mode
- (보류 시) 외부 컨트리뷰터에게 데이터 회신 + 6 개월 후 재측정 cron 후보
```

### 측정 도구 (디벨로퍼 가이드)

```text
# 1. 페이지 수
find <vault>/wiki -name "*.md" | wc -l
find <vault>/wiki/projects -name "*.md" | wc -l
find <vault>/wiki/topics -name "*.md" | wc -l

# 2. 길이 분포 (word count)
find <vault>/wiki -name "*.md" -exec wc -w {} + | awk '{print $1}' | sort -n
# → 평균 / 중앙값 / 최대 / 분포

# 3. 검색 빈도 (DB query 또는 access log)
sqlite3 <db-path> "SELECT COUNT(*) FROM query_cache WHERE created_at >= date('now','-30 days')"
# query_cache 가 wiki_search 만 추적하지 않으면 N/A
```

대용량 출력 회피 — `wc -l`, `awk` 통계 함수 사용. 개별 page 내용 read 금지.

### 외부 컨트리뷰터 답변 데이터 (Task 04 입력)

본 보고서의 다음 항목을 Task 05 답변 초안에서 인용:
- 현재 페이지 수 + 길이 분포 (BM25 한계 도래 시점 추정)
- 결정 (P40 진행 / 보류)
- 보류 시 재측정 시점 약속

## Dependencies

- 외부: 없음 (find / awk / sqlite3 도구만)
- 내부 task: sync 완료
- Task 01, 02 와 동일 데이터 소스 (병렬 가능)

## Verification

```bash
ls -la docs/baseline/p39-p40-decision.md
grep -qE "페이지 수|길이 분포|검색 빈도|결정|후속 액션" docs/baseline/p39-p40-decision.md
grep -E "결정:[ ]+(P40 즉시 진행|보류)" docs/baseline/p39-p40-decision.md
```

수동: 결정 근거가 측정값과 일치하는지 검토.

## Risks

- **검색 빈도 측정 인프라 부재**: `query_cache` 가 wiki_search 추적하지 않으면 핵심 결정 기준 1개 사용 불가. → 페이지 수 + 길이 만으로 결정. 추적 인프라 추가는 별도 phase.
- **P40 결정의 외부 영향**: 외부 컨트리뷰터 요청이 강한 경우 측정 미충족이라도 P40 진행 가능. 결정 섹션에 "데이터 vs 외부 요청" 명시.
- **token 추정 부정확**: word → token 변환은 backend 별 다름 (Gemini ≈ 1.3, Claude ≈ 1.5, Haiku ≈ 1.4). 추정 단위 명시 ("~tokens, ±20%").
- **vault 구조 가정**: `wiki/projects/`, `wiki/topics/` 분류는 P22 wiki structure 기준. 현재 vault 가 다른 구조면 카테고리 명만 보고서에 적시.

## Scope boundary

수정 금지:
- production 코드 전체
- vault wiki 페이지 자체 (read-only 측정)
- DB (read-only query)
- P40 plan 문서 자체 — 본 task 는 결정 데이터 만, 실제 P40 plan-proposal 은 별도
- 다른 task 영역 / 보고서
