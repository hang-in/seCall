---
type: task
status: draft
updated_at: 2026-05-05
plan_slug: p39-wiki-sync-auto-commit-fix
task_id: 01
parallel_group: B
depends_on: []
---

# Task 01 — wiki 파이프라인 baseline 측정 보고서

## Changed files

신규:
- `docs/baseline/p39-wiki-baseline.md` — wiki 파이프라인 baseline 측정 결과. sync 완료 후 사용자 입력 데이터 (sync log + status) 기반 분석.

수정: 없음 (production 코드 무수정)

## Change description

### 입력 데이터

사용자가 sync 완료 후 다음 3가지 제공:
1. `/tmp/sync-2026-05-03.log` — sync 전체 stdout/stderr (tee 캡처)
2. `secall status` JSON 출력 (sync 완료 직후)
3. vault `wiki/` 디렉토리 페이지 카운트 (`find vault-path/wiki -name "*.md" | wc -l`)

### 측정 카테고리 (보고서 섹션 구조)

**1. 처리 카운트 (sync outcome 추출)**
- ingested / skipped / errors / skipped_min_turns / hook_failures
- wiki_updated / wiki pages_written
- graph_nodes_added / graph_edges_added
- pulled / pushed (성공 여부)
- partial_failure 발생 여부

**2. 시간 측정**
- 총 sync elapsed (시작/종료 timestamp)
- Phase 별 비율 추정 (pull / reindex / ingest / wiki / push) — sync log 의 phase_start/phase_complete 마커 기반
- 세션당 평균 처리 시간 (683 세션 기준)

**3. 비용 추정**
- LLM 호출 횟수 (wiki update + 시맨틱 graph extract)
- backend 별 분류 (Gemini / Haiku / Claude / Codex / Ollama / LMStudio) — sync log 의 backend 라인 grep
- 토큰 단가 × 호출 횟수 → USD 추정 (대략값, 정확도 ~20%)
- Ollama / LMStudio 는 로컬 → 비용 0, GPU/CPU 시간만 표기

**4. Edge case 발생 빈도**
- review-regen 발생 횟수 (sync log 의 "Regenerating due to review errors" 라인 grep)
- lint 실패 페이지 수 (`Lint:` 라인 grep)
- merge conflict 발생 (sync log + vault git 사용자 확인)
- frontmatter 검증 실패 횟수
- vault 파일 누락으로 skipped 된 세션 수

**5. 안정성 issue 기록**
- sync 도중 발견된 사고 (예: 본 phase 의 auto_commit 누락)
- 진행 중 panic / error 가 graceful 하게 처리됐는지

### 보고서 작성 패턴

각 카테고리 섹션에:
- **측정값**: 표 또는 bullet (raw 숫자)
- **해석**: 1~2 문장 (정상 vs 이상)
- **후속 액션 제안**: 별도 phase 후보 (있으면)

### sync log 분석 방법 (디벨로퍼 가이드)

`/tmp/sync-2026-05-03.log` 가 클 가능성 (수십 MB) — 라인 단위 grep 으로 추출:
- `grep "phase_start"` → phase 진입 timestamp
- `grep "phase_complete"` → phase 종료 + result JSON
- `grep "Regenerating"` → review-regen 카운트
- `grep "Lint:"` → lint 메시지
- `grep -c "Written:"` → 작성 페이지 수
- `grep "Error\|error\|Failed"` → 에러 패턴 분류
- `head -200`, `tail -200` → 시작/끝 컨텍스트 (대신 `cat` 금지 — 토큰 폭발)

대용량 로그 직접 read 금지 — `wc -l`, `grep -c`, `head/tail` 만.

### 보고서 형식 (Markdown)

```text
# P39 Wiki 파이프라인 Baseline (2026-05-05 sync)

## 1. 처리 카운트
| 항목 | 값 | 해석 |
|---|---|---|
| ingested | ... | ... |
...

## 2. 시간 측정
...

## 3. 비용 추정
...

## 4. Edge case 빈도
...

## 5. 안정성 issue
- auto_commit 누락 (Task 01 에서 fix)
...
```

## Dependencies

- 외부: 없음 (분석 도구만)
- 내부 task: sync 완료 (사용자 진행 중) — 본 phase 외 의존
- Task 00 무관

## Verification

```bash
ls -la docs/baseline/p39-wiki-baseline.md
grep -qE "처리 카운트|시간 측정|비용 추정|Edge case|안정성" docs/baseline/p39-wiki-baseline.md
wc -l docs/baseline/p39-wiki-baseline.md   # 100+ lines 기대
```

수동: 보고서 개관 검토 — 5 섹션 모두 채워졌는지, raw 숫자 + 해석이 함께 있는지.

## Risks

- **sync log 손실**: 사용자가 `tee` 안 했거나 파일 삭제 시 측정 불가. → 사용자에게 보존 요청 (이미 `/tmp/sync-2026-05-03.log` 명시).
- **LLM 비용 추정 정확도**: backend 별 정확한 토큰 수를 sync log 가 기록 안 할 수 있음. 추정값 명시 (실제와 ±50% 이내 추정 정도면 baseline 으로 충분).
- **로그 파일 대용량**: cat / 전체 read 금지. grep / head / tail 로만 추출. 토큰 폭발 위험.
- **partial sync (auto-commit 누락 영향)**: pull 실패로 일부 세션 누락 가능. 보고서에 명시 — "이 baseline 은 partial-sync 결과" 라고 적시.

## Scope boundary

수정 금지:
- production 코드 전체 (`crates/`, `web/src/`)
- 다른 task 영역 (Task 00 의 vault/git.rs, Task 02/03 의 보고서, Task 04 의 README/Insight)
- sync log 자체 (read-only 분석만)
- vault dir (read-only 측정만, write 없음)
