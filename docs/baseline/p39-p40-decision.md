---
type: baseline
status: done
updated_at: 2026-05-06
plan_slug: p39-wiki-sync-auto-commit-fix
task_id: 03
---

# P39 Wiki 벡터화 (P40) 우선순위 결정 (2026-05-06)

> 측정 대상: `~/Documents/Obsidian Vault/seCall/wiki/`
> DB 경로: `~/Library/Caches/secall/index.sqlite` (실측 — 기본 경로 `~/Library/Application Support/secall/` 미사용)
> 측정 단위: word count (한국어/영어 혼재). token 추정 = word × 1.4 (Haiku 기준, ±20%).

---

## 1. 페이지 수

| 카테고리 | 카운트 |
|---|---|
| 총 | 19 |
| projects | 9 |
| topics | 4 |
| decisions | 5 |
| overview (root) | 1 |

분류 기준: vault `wiki/` 하위 폴더 (`projects/`, `topics/`, `decisions/`) + 루트 `overview.md`.

---

## 2. 길이 분포

- 평균: **3478.7 words** (≈ **4870 tokens**, ±20%)
- 중앙값: **436 words** (≈ 610 tokens)
- 최대: **26353 words** (`projects/tunaflow-mobile.md`)
- 최소: **133 words**
- 총합: 66096 words (≈ 92534 tokens)

### 히스토그램 (word 구간)

| 구간 (words) | 카운트 |
|---|---|
| 0 – 100 | 0 |
| 100 – 500 | 12 |
| 500 – 1000 | 2 |
| 1000 – 2000 | 0 |
| 2000 – 3000 | 0 |
| 3000 – 4000 | 0 |
| 4000 – 5000 | 0 |
| 5000 – 7500 | 1 |
| 7500 – 10000 | 2 |
| 10000+ | 2 |

**관찰**: bimodal 분포 — 대다수 (14/19, 74%) 가 1000 words 미만, 소수 (5/19) 가 5000 words 이상 대용량. 평균은 5개 대용량 페이지 (특히 26k words tunaflow-mobile) 가 끌어올림. 중앙값 436 이 실제 페이지 크기를 더 잘 대표.

### Top 5 (대용량)

1. `projects/tunaflow-mobile.md` — 26353 words
2. `projects/gemento.md` — 10937
3. `projects/secall.md` — 10683
4. `overview.md` — 9413
5. `projects/tunaflow.md` — 6709

---

## 3. 검색 빈도

- 최근 30일 wiki_search 호출: **N/A — 추적 인프라 부재**
- 근거: DB `query_cache` 테이블은 존재하나 (`query_hash`, `original`, `expanded`, `created_at` 4-컬럼 schema), wiki_search 가 아닌 **쿼리 확장 캐시 전용**이며 현재 row count = 0. wiki 검색 호출 자체에 대한 access log / 카운터 없음.
- 측정 인프라 추가는 별도 phase 후보 (P40 결정 기준에서 본 항목은 평가 불가).

---

## 4. P40 진행 결정

기준 충족:
- 페이지 수 ≥ 100: **N** (19, 약 19%)
- 평균 길이 ≥ 5000 tokens: **N** (≈ 4870 tokens, 약 97% — 거의 임계지만 미달, 게다가 5개 outlier 가 끌어올린 평균)
- 검색 빈도 ≥ 10/day: **N/A — 추적 부재**

결정: P40 즉시 진행 (단순 스코프)

(원본 마커: **P40 즉시 진행**)

근거: 측정 데이터(페이지 19, 평균 4870 tokens)는 임계 미달이지만, **strategic 결정**으로 진행:
1. **외부 컨트리뷰터 신호** — BM25 → vector hybrid 요청 명시 (`/api/recall` 와의 비대칭 지적)
2. **코드 일관성** — recall (hybrid) 와 wiki_search (BM25) 의 패턴 통일. 향후 검색 변경 시 두 path 분기 부담 제거
3. **Early infrastructure** — 페이지 100+ 도래 후 진행하면 그 시점에 사용자 검색 경험 저하 후행. 미리 인프라 + 19 페이지로 벡터화 자체 검증
4. **ROI 정당화**: 19 페이지 임베딩 비용 무시 (bge-m3 1회), DB v9 마이그레이션 단일 사용자 영향 최소

**범위 제한 (단순 스코프)**:
- 페이지 단위 임베딩만 (chunker 분리 X — 평균 길이 짧아 1 페이지 = 1 chunk 충분)
- 섹션 단위 chunking 은 페이지 100+ 또는 평균 길이 8000+ tokens 도래 후 별도 phase
- hybrid mode 옵션 (`?mode={keyword|semantic|hybrid}`) 으로 BM25 fallback 보존

---

## 5. 외부 컨트리뷰터 답변 입력 데이터 (Task 04)

답변 초안에서 인용할 핵심 수치:

- **현재 wiki 규모**: 19 pages, 평균 3478 words, 중앙값 436 words, 최대 26k words
- **분포 특성**: bimodal — 14/19 (74%) 가 1000 words 미만 short note, 5/19 가 대용량 project doc
- **결정**: P40 (wiki 벡터화) **즉시 진행** — 단순 스코프
- **근거 요약**: 측정 미충족 (페이지 19) 이지만 strategic 결정 — 외부 컨트리뷰터 신호 + recall/wiki_search 일관성 + early infrastructure
- **범위**: 페이지 단위 임베딩만 (섹션 chunker X), hybrid mode 옵션 (BM25 fallback 보존). 챗nker 분리는 페이지 100+ 도래 후 별도 phase
- **시점**: P39 머지 후 P40 plan-proposal 진행 예정

---

## 6. 후속 액션

> 결정: **P40 즉시 진행**. 이전 "보류" 분기 액션은 제거됨.

- 외부 컨트리뷰터에게 본 측정 데이터 + 즉시 진행 결정 회신 (Task 04 답변 초안 `docs/community/p39-wiki-vector-response.md`)
- P40 plan-proposal 초안: chunker 재사용, embedding backend Ollama bge-m3, DB v9 `wiki_vectors`, hybrid mode (`?mode={keyword|semantic|hybrid}`)
- (선택) wiki_search 호출 카운터 (DB column 추가 또는 access log) — P40 효과 측정 / 향후 chunker 분리 결정 인프라. 별도 phase 후보 (~1일 작업)
- 6 개월 후 또는 페이지 수 50 도달 시 본 보고서 갱신 (`docs/baseline/p39-p40-decision.md` 새 버전) — chunker 분리 phase 진입 시점 판단 위함

---

## 부록: 측정 명령

```bash
# 1. 페이지 수
find "$VAULT/wiki" -name "*.md" | wc -l
find "$VAULT/wiki/projects" -name "*.md" | wc -l
find "$VAULT/wiki/topics" -name "*.md" | wc -l
find "$VAULT/wiki/decisions" -name "*.md" | wc -l

# 2. 길이 분포 (Python — awk asort 미지원 환경 우회)
python3 -c "
import os, glob, statistics
files = glob.glob(os.path.expanduser('$VAULT/wiki/**/*.md'), recursive=True)
wc = [len(open(f, encoding='utf-8', errors='ignore').read().split()) for f in files]
print('count:', len(wc), 'avg:', round(sum(wc)/len(wc),1), 'median:', statistics.median(wc), 'max:', max(wc), 'min:', min(wc))
"

# 3. 검색 빈도 (query_cache 가 wiki_search 추적 안 함 → N/A)
sqlite3 ~/Library/Caches/secall/index.sqlite "SELECT COUNT(*) FROM query_cache"
```
