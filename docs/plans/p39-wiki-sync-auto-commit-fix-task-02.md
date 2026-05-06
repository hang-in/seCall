---
type: task
status: draft
updated_at: 2026-05-05
plan_slug: p39-wiki-sync-auto-commit-fix
task_id: 02
parallel_group: B
depends_on: []
---

# Task 02 — wiki 페이지 품질 spot check

## Changed files

신규:
- `docs/baseline/p39-wiki-quality.md` — 무작위 10 페이지 샘플 품질 검토 결과. 자동 검증 + 사용자 manual review 항목 결합.

수정: 없음 (production 코드 무수정)

## Change description

### 샘플링 전략

`vault/wiki/` 디렉터리 전체 페이지에서 10개 무작위 선정 — 디벨로퍼 명령 (sync 완료 후):
```text
find <vault.path>/wiki -name "*.md" | shuf -n 10 > /tmp/p39-quality-sample.txt
```

vault.path 는 `~/Library/Application Support/secall/config.toml` 의 `[vault] path` 값 (현재 `/Users/d9ng/Documents/Obsidian Vault/seCall`).

### 자동 검증 항목 (도구 활용)

각 샘플 페이지에 대해:
1. **Frontmatter validity** — `secall lint <page-path>` 실행 → frontmatter 형식 오류 / session_id 참조 무결성 검증
2. **Lint 결과** — `markdownlint` (sync 가 자동 호출하는 것과 동일) 출력 캡처
3. **Wikilink 깨짐** — 본문의 `[[...]]` 링크가 vault 안에 실재 파일/페이지 존재하는지 grep
4. **Dataview field 오염** — `^[a-z_]+::` 패턴 grep — body 안에 inline field 가 의도 외 위치에 있는지

### Manual review 항목 (사용자)

각 샘플 페이지를 사용자가 직접 열어 다음 5점 척도로 평가 (1=poor, 5=excellent):
1. **요약 정확성** — wiki 페이지가 원본 세션 의도를 정확히 반영했는가
2. **요약 완전성** — 핵심 결정/액션 누락 여부
3. **톤 일관성** — "정리" 톤 (P22 prompt tuning 결과) 유지
4. **중복 콘텐츠** — 같은 정보 반복 여부
5. **Obsidian 호환성** — 렌더링 깨짐 / 인라인 field 오염

### 보고서 형식

```text
# P39 Wiki 페이지 품질 spot check

## 샘플 (10 페이지)
| 페이지 | 자동 검증 | 정확성 | 완전성 | 톤 | 중복 | Obsidian | 종합 |
|---|---|---|---|---|---|---|---|
| projects/seCall.md | ✅ | 4 | 5 | 5 | 5 | 5 | 4.8 |
...

## 자동 검증 요약
- frontmatter 오류: N/10
- lint 경고: N/10
- 깨진 wikilink: N건
- dataview 오염: N건

## Manual review 요약
- 평균 점수: ...
- 분류별 issue: ...

## 발견된 패턴
1. ...
2. ...

## 후속 액션 제안 (있으면)
- 별도 phase 후보 (예: prompt tuning 추가 / lint 강화 / 등)
```

### sync 에서 spot check 분리

원래 sync 가 review/regen 으로 일부 quality 검증 — 본 task 는 그 외 사후 검증. review-regen 통과한 페이지도 manual 평가에서 낮은 점수 가능 (LLM review 의 한계).

### Manual review 결과 입력 방법 (사용자 가이드)

- 사용자가 10 페이지 직접 열고 위 5 항목 평가
- 평가 후 점수를 task 보고서 표에 기입
- 디벨로퍼는 자동 검증 + 표 양식 만들고, manual 점수는 placeholder (`사용자 입력 대기`) 로 시작
- 사용자가 점수 채우면 보고서 완성

## Dependencies

- 외부 도구: `markdownlint`, `secall lint` (이미 설치)
- 내부 task: sync 완료 (Task 02 와 동일)
- Task 00 / 01 / 03 무관 (병렬 가능)

## Verification

```bash
ls -la docs/baseline/p39-wiki-quality.md
grep -qE "샘플|자동 검증|Manual review|발견된 패턴" docs/baseline/p39-wiki-quality.md
grep -c "^| " docs/baseline/p39-wiki-quality.md   # 표 라인 수 — 11+ (header + 10 pages)
```

수동: 사용자 manual review 점수 입력 후 평균 계산 / 패턴 정리 단계 점검.

## Risks

- **샘플링 편향**: 10 페이지가 vault 전체 (수백 페이지) 대표성 못 가질 수 있음. 본 task 는 spot check (전수 X) 임을 보고서에 명시.
- **LLM review-regen 통과 페이지의 manual 낮은 점수**: 자동 review 한계 노출 — 후속 phase 후보 (review prompt 강화).
- **사용자 manual review 시간 부담**: 10 페이지 × 5 항목 = 50 평가. 1페이지당 5분 = 50분 소요. 사용자에게 미리 공지 (보고서 placeholder + 평가 항목 명확).
- **vault 위치 의존**: 디벨로퍼가 vault.path 정확히 읽어야 (config.toml 경로). Task 01 / 03 과 같은 데이터 소스.
- **wikilink 검증 정확성**: vault 안 다른 페이지로의 링크가 모두 명시적 file path 가 아닐 수 있음 (Obsidian 의 alias 또는 페이지 이름 매칭). 단순 file 존재 여부만 검증 (1차 안전망).

## Scope boundary

수정 금지:
- production 코드 전체
- vault dir 안 wiki 페이지 자체 (read-only 검토)
- 다른 task 보고서 / 영역
- 자동 review prompt / lint 규칙 (별도 phase)
