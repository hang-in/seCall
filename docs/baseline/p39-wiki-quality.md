---
type: baseline
status: in_progress
updated_at: 2026-05-05
plan_slug: p39-wiki-sync-auto-commit-fix
task_id: 02
---

# P39 Wiki 페이지 품질 spot check (2026-05-05)

> Task 02 산출물. vault `wiki/` 19 개 페이지 중 무작위 10 개 페이지를 spot check.
> 자동 검증은 `secall lint --json` + Python 분석 스크립트로 수행. **사용자 manual review 점수는 placeholder (`_`) 로 시작 — 사용자 직접 평가 후 보고서 채움.**

## 환경 / 도구

- vault: `/Users/d9ng/Documents/Obsidian Vault/seCall`
- 자동 검증 도구: `secall lint --json` (frontmatter / sources / session_id 무결성)
- 보조 분석: Python (frontmatter 존재, wikilink 해석, dataview inline field grep)
- 샘플링: Python `random.sample(seed=20260505)` — `shuf` 가 macOS 기본 미설치
- markdownlint: 미설치 환경 (`markdownlint`/`markdownlint-cli2` 없음). sync 단계의 lint 결과는 별도 로그에 남고, 본 보고서는 `secall lint` 결과로 대체

## 샘플 (10 페이지)

| 페이지 | 자동 검증 | 정확성 | 완전성 | 톤 | 중복 | Obsidian | 종합 |
|---|---|---|---|---|---|---|---|
| decisions/2026-04-09-windows-sync-analysis.md | FAIL (lint err 1) | _ | _ | _ | _ | _ | _ |
| projects/tunainsight.md | FAIL (lint err 12) | _ | _ | _ | _ | _ | _ |
| projects/gemento.md | FAIL (broken wikilink 2 — 따옴표 placeholder) | _ | _ | _ | _ | _ | _ |
| projects/cheonhwaga.md | FAIL (lint err 2) | _ | _ | _ | _ | _ | _ |
| projects/tunaflow-mobile.md | OK | _ | _ | _ | _ | _ | _ |
| projects/docad.md | FAIL (lint err 1) | _ | _ | _ | _ | _ | _ |
| topics/llm-wiki-pattern.md | FAIL (lint err 4 + broken link 1 — 예시 경로) | _ | _ | _ | _ | _ | _ |
| projects/tunaflow.md | FAIL (lint err 29) | _ | _ | _ | _ | _ | _ |
| projects/secall.md | FAIL (lint err 29 + broken link 3 — 템플릿 placeholder) | _ | _ | _ | _ | _ | _ |
| decisions/2026-03-19-tunadish-architecture.md | FAIL (lint err 3) | _ | _ | _ | _ | _ | _ |

> Manual 점수 컬럼 (정확성·완전성·톤·중복·Obsidian) 은 1=poor … 5=excellent. 사용자 입력 대기.
> "종합" 은 manual 5 항목 평균.

## 자동 검증 요약

- frontmatter 결손 (빈 frontmatter / `sources` 없음): **0 / 10**
- secall lint 에러/경고 발생 페이지: **8 / 10** (모두 `L009 wiki references non-existent session`)
- 깨진 wikilink: **6 건 / 6 페이지** (대다수 placeholder — 아래 분류)
- dataview inline field 오염 (`^[a-z_]+::`): **0 건**

### 깨진 wikilink 분류

| 페이지 | 깨진 target | 분류 |
|---|---|---|
| projects/gemento.md | `"spanish"` | 인용부호 포함 placeholder (잘못된 link 형식) |
| projects/gemento.md | `"provid…"` | 절단된 인용 텍스트 placeholder |
| topics/llm-wiki-pattern.md | `raw/sessions/2026-04-05/claude_seCall_a1b2c3` | 예시 경로 (실제 파일 없음) |
| projects/secall.md | `{full_uuid}` | 템플릿 변수 placeholder |
| projects/secall.md | `raw/sessions/YYYY-MM-DD_session-id` | 예시 경로 placeholder |
| projects/secall.md | `페이지명` | 일반 명사 placeholder |

→ 모두 LLM 생성 본문이 **예시·placeholder 를 wikilink 문법 안에 두면서 발생**. 실제 끊긴 페이지 참조는 0 건.

### Lint L009 에러 (`wiki references non-existent session`) 요약

전체 vault 기준으로 109 건 — 10 페이지 샘플 합 81 건. 패턴:
- `wiki/projects/*.md` 의 `sources:` 또는 본문이 vault DB 에 없는 session_id (8 자 short id) 를 참조
- 예: `secall.md` 가 `0484df32`, `26276d4d` 등 — 해당 session 이 vault DB 에 ingest 안 된 상태

원인 가설:
1. wiki 페이지가 과거 sync 시점의 session_id 를 sources 에 박아두었으나, 이후 DB rebuild / data 정리 과정에서 해당 세션이 사라짐
2. Wiki 본문 link 가 short id 8 자만 사용 — full UUID 매칭 실패 가능성

## Manual review 가이드 (사용자 입력 대기)

각 페이지를 Obsidian 으로 직접 열고 1\~5 점수 부여:

1. **정확성** — 본문이 원본 세션 의도를 정확히 반영했는가 (왜곡 / 환각 여부)
2. **완전성** — 핵심 결정·액션·근거가 빠지지 않았는가
3. **톤** — P22 prompt tuning 의 "정리" 톤 (담백·결론 우선) 유지하는가
4. **중복** — 같은 정보가 섹션을 옮겨가며 반복되지 않는가
5. **Obsidian** — frontmatter / wikilink / 헤딩 구조가 Obsidian 에서 정상 렌더되는가

**기록 방법**: 위 표의 manual 컬럼에 점수를 직접 기입 → 종합 = 평균. 페이지당 권장 시간 약 5 분, 총 ~50 분.

## Manual review 결과 (사용자 입력 후 채움)

- 평균 점수: _
- 가장 낮은 항목 카테고리: _
- 페이지별 주요 이슈: _

## 발견된 패턴 (자동 검증 기준)

1. **L009 stale session_id** 가 가장 큰 이슈 — `secall.md` / `tunaflow.md` 같이 인기 프로젝트 페이지일수록 누적 에러 많음 (각 29 건).
   - 후속 액션 후보: `secall lint --fix` 수준의 wiki source-id 자동 정리 cmd 추가, 또는 sync 시점에 stale id pruning 단계 삽입.
2. **Wikilink 안에 placeholder 텍스트** — LLM 이 본문 예시를 `[[...]]` 안에 넣음. Lint 에서 별도 코드로 잡지 못하는 잠재적 깨짐.
   - 후속 액션 후보: review prompt 에 "예시 경로/변수는 backtick 으로 감싸고 wikilink 문법 사용 금지" 명시.
3. **Frontmatter 무결성은 모든 페이지 통과** — `sources:` 키 누락 0 건. P39 sync 변경 후 frontmatter 자체는 안정.
4. **Dataview inline field 오염 0 건** — 본문이 깨끗한 prose 로 유지됨.

## 후속 액션 제안

- (P1) Wiki source-id 정리 명령 또는 sync 단계 stale-id pruning — Lint L009 109 건은 원본 세션 부재가 원인.
- (P2) Wiki review prompt 에 "예시·placeholder 는 wikilink 사용 금지" 가이드 추가.
- (P3) markdownlint(-cli2) 환경 표준화 — 본 spot check 에서는 도구 부재로 secall lint 로 대체. CI / 개발자 머신 일관 설치 필요.
- (P3) `shuf` 의존 제거 — task 본문 명령에 macOS 호환 안내 (Python 또는 `gshuf`) 추가.

## 한계 / 비고

- 10 페이지 / 19 페이지 = 약 53 % 커버 (전체가 작아서 spot check 가 비교적 대표성 있음).
- Manual 점수는 사용자 평가 전까지 placeholder. 자동 점수만으로 "통과" 여부 단정 금지.
- Lint L009 는 wiki 콘텐츠 자체보다 vault DB 와의 동기화 문제 — 본 spot check 의 quality 결론에 영향 적음.
