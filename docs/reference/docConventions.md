---
type: reference
status: done
updated_at: 2026-05-19
canonical: true
---

# seCall 문서 규약 (docConventions)

이 문서는 `CLAUDE.md §3 Documentation Rules` 의 expanded 버전이다.
CLAUDE.md 는 룰의 1~2줄 요약만 담고, 판단 기준·예시·체크리스트는 이 문서를 본다.
모든 에이전트(Claude, Codex, Gemini, OpenCode)와 사람이 같은 규칙으로 문서를 만들고 유지하기 위한 기준이다.

핵심 판단:

- 좋은 문서 관리는 "파일을 적게 만드는 것" 이 아니라 "에이전트가 현재 문서를 빠르게 고를 수 있게 하는 것" 이다.
- 따라서 seCall 은 `파일명 + frontmatter + index.md` 세 축으로 문서 상태를 관리한다.

---

## 1. 문서 타입 (8개 디렉토리)

seCall 의 `docs/` 는 다음 8개 서브디렉토리로 구성된다.
각 디렉토리의 역할과 읽는 법이 다르므로, 새 문서를 만들 때 가장 먼저 위치를 정해야 한다.

### `reference/` — 현재 기준 사실 (SSOT)

- 역할: 구현 현황, 데이터 모델, 설정 가이드, ADR, 백로그 등 "지금 옳다고 합의된 사실"
- 읽는 법: 새 세션에서 가장 먼저 읽는 문서군
- 예시: `core-backlog.md`, `web-backlog.md`, `llm-config.md`, `wiki-setup.md`, `docConventions.md`

### `plans/` — 앞으로 할 일

- 역할: 작업 계획, 목표/비목표/완료 기준
- 읽는 법: 현재 작업과 관련된 plan 만 읽는다. 전체를 다 읽지 않는다.
- 예시: `p18-rev-2-regex.md`, `p18-rev-2-regex-task-01.md`

### `prompts/` — 실행 에이전트용 작업 지시문

- 역할: Claude/Codex 등 실행 에이전트에게 넘기는 프롬프트
- 읽는 법: 반드시 대응하는 `plans/` 문서와 함께 읽는다.
- 위치: `prompts/YYYY-MM-DD/short_name.md`

### `agents/` — 에이전트 역할 정의

- 역할: 각 에이전트(Developer, Architect, Reviewer 등) 의 행동 규칙
- 읽는 법: 새 에이전트 역할을 부여받을 때

### `baseline/` — 기준선 데이터/스냅샷

- 역할: 성능 벤치마크, 회귀 비교용 기준선
- 읽는 법: 회귀 검증·성능 측정 시

### `community/` — 외부 공개·커뮤니티용 문서

- 역할: 다모앙 등 외부 공유용 글, 사용자 가이드 초안
- 읽는 법: 외부 공개 작업 시

### `insight/` — 분석·회고·인사이트

- 역할: 사후 분석, 회고, 데이터 분석 결과
- 읽는 법: 의사결정 근거가 필요할 때

### `reviews/` — 코드/PR 리뷰 기록

- 역할: PR 리뷰 요약, 코드 리뷰 메모
- 읽는 법: 같은 영역 재작업 시 과거 리뷰 참고용

---

## 2. 파일명 규칙

### Reference (`docs/reference/`)

- **날짜 없는 안정 이름** 을 기본으로 한다 (예: `core-backlog.md`, `llm-config.md`).
- 2~4 토큰의 camelCase를 기본으로 한다. (기존 kebab-case 파일들과의 일관성이 필요한 경우에만 예외적 허용)
- 예외: 시간 흐름 자체가 중요한 핸드오프 문서는 `handoff_YYYY-MM-DD.md` 허용 (예: `handoff_2026-05-19.md`).

### Plan (`docs/plans/`)

- 기본: `{slug}.md` (예: `p18-rev-2-regex.md`)
- 날짜 구분이 필요한 경우: `{slug}_YYYY-MM-DD.md`
- 하위 작업 지시서: `{slug}-task-NN.md`

### Prompt (`docs/prompts/`)

- 위치 고정: `YYYY-MM-DD/short_name.md`
- 같은 날 여러 프롬프트는 같은 날짜 폴더에 모은다.

### Brainstorm / Review / Memo

- 성격이 드러나는 이름 + 날짜 (예: `gemini-vs-anthropic-review_2026-05-15.md`)
- frontmatter 에 `canonical: false` 명시 (3절 참고).

### 피해야 할 것

- 같은 주제의 reference 를 날짜 파일로 계속 복제 (`backlog_2026-05-01.md`, `backlog_2026-05-15.md` …)
- 의미 없는 일반명 (`notes.md`, `temp.md`, `draft.md`)
- 버전/날짜 없이 비슷한 이름이 공존 (`config.md`, `config2.md`)

---

## 3. 필수 frontmatter

모든 문서 상단에 YAML frontmatter 를 둔다.

```yaml
---
type: reference        # reference | plans | prompts | reviews | insight | baseline | community | agents
status: done           # 아래 §3.1 참고
updated_at: 2026-05-19 # YYYY-MM-DD
canonical: true        # false 이면 SSOT 아님 (브레인스토밍/비교 문서)
superseded_by: docs/reference/new_file.md # 대체된 경우만
---
```

### 3.1 status 값

| 값 | 의미 |
|-----|------|
| `draft` | 작성 중, 아직 합의 전 |
| `in_progress` | 진행 중 (plan/prompt 주로 사용) |
| `partial` | 일부만 완료, 잔여 작업 있음 |
| `done` | 완료, 현재 기준으로 유효 |
| `archived` | 더 이상 현재 기준 아님 (`superseded_by` 권장) |

### 3.2 관계 메타 (선택)

- `related: [path1, path2]` — 함께 읽으면 좋은 문서
- `paired_plan: docs/plans/foo.md` — 이 prompt 와 짝인 plan
- `paired_prompt: docs/prompts/2026-05-19/foo.md`
- `supersedes: docs/reference/oldFoo.md` — 이 문서가 대체한 과거 문서
- `superseded_by: docs/reference/newFoo.md` — 이 문서를 대체한 신규 문서
- `read_before: [path]` — 이 문서를 읽기 전에 먼저 봐야 할 문서

> 경로는 프로젝트 루트 기준 (`docs/...`)으로 통일한다.

관계 메타가 있어야 에이전트가 낡은 문서를 덜 읽는다.

### 3.3 canonical 사용

- `canonical: true` — 현재 기준 (대부분의 reference, 완료된 plan)
- `canonical: false` — 외부 레퍼런스, 비교 메모, 아이디어 수집, 브레인스토밍
  - "실행 계획처럼 읽히는 표현" 을 자제하고, 현재 구현 기준 문서 경로를 명시한다.

---

## 4. 인덱스 규칙

각 폴더의 `index.md` 는 단순 파일 목록이 아니다. 다음 네 가지를 알려줘야 한다.

1. **무엇이 기준 문서인가** — 현재 SSOT 식별
2. **무엇이 현재 유효한가** — done / partial / archived 구분
3. **어떤 순서로 읽어야 하는가** — 추천 읽기 순서
4. **새 문서 추가 시 링크 반영** — plan/prompt 생성 시 동시 갱신 필수

### index.md 에 반드시 있어야 할 것

- 짧은 폴더 설명 (1~2줄)
- 문서별 한 줄 설명 + 상태 라벨
- 추천 읽기 순서 (또는 카테고리 분류)
- 아카이브 문서는 별도 섹션에 분리하거나 경고 표기

---

## 5. 버전관리 원칙

### 5.1 Reference 는 같은 파일을 갱신한다

- 대상: `docs/reference/*.md`, 프로젝트 루트 `CLAUDE.md`
- 같은 주제의 현재 기준 문서는 하나만 유지하고 `updated_at` 만 갱신한다.
- 새 날짜 파일을 계속 만들지 않는다.
- 예외: 성격이 완전히 다른 새 reference 가 생길 때만 새 파일 허용.

### 5.2 Plan / Prompt 는 작업 단위별 새 문서 가능

- 작업 단위가 독립적이면 날짜 또는 slug 기반 새 문서 허용.
- 단 **`index.md` 동반 갱신 필수**. 새 plan 만들고 index 안 고치면 안 된다.

### 5.3 브레인스토밍은 SSOT 가 아니다

- 외부 레퍼런스, 비교 메모, 아이디어 수집은 `canonical: false`.
- 현재 구현 기준 문서 경로를 frontmatter 또는 본문 상단에 명시.

### 5.4 핸드오프 문서는 예외적으로 날짜 분리

- 세션 종료 시점의 스냅샷은 시간이 의미를 가지므로 `handoff_YYYY-MM-DD.md` 로 새로 만든다.
- 단 index.md 에 최신 핸드오프를 명시한다.

---

## 6. 아카이브 정책

오래된 문서는 **삭제보다 아카이브 우선**.

### 절차

1. frontmatter 의 `status: archived` 로 변경
2. 가능하면 `superseded_by: docs/reference/newFoo.md` 로 대체 문서 연결
3. 본문 최상단에 한 줄 경고 추가 (선택)
   ```
   > 이 문서는 아카이브되었습니다. 현재 기준: [newFoo.md](newFoo.md)
   ```
4. index.md 에서 별도 "Archived" 섹션으로 이동 (또는 명시적 라벨)

### 삭제가 정당화되는 경우

- 잘못된 정보가 들어가서 보존 가치도 없는 경우 (드뭄)
- 임시 디버깅 메모처럼 처음부터 보존 의도가 없던 파일

---

## 7. 에이전트 작업 시 판단할 것 (체크리스트)

문서를 생성·수정하기 전에 반드시 다음 4가지를 자문한다.

1. **갱신 vs 신규** — 같은 주제의 기존 문서가 있나? 있으면 갱신이 맞다.
2. **current vs 참고용** — 이 문서는 SSOT 인가, 비교/브레인스토밍인가? 후자라면 `canonical: false`.
3. **index 업데이트 필요?** — plan/prompt 신규 생성이면 반드시 index.md 동반 수정.
4. **supersede 관계 남겼나?** — 기존 문서를 대체한다면 `supersedes` / `superseded_by` 로 연결.

### 특히 피해야 할 패턴

- 같은 주제 reference 를 날짜 파일로 계속 복제
- 새 plan/prompt 만들고 index.md 갱신 누락
- 아카이브 문서를 현재 기준처럼 방치
- 브레인스토밍 문서를 구현 현황처럼 작성

---

## 8. 최소 실무 규칙 (강제)

지금 당장 모든 에이전트가 지켜야 할 최소 규칙은 다음 네 가지다.

1. reference 는 **기존 파일 우선 갱신**
2. plan/prompt 는 새 파일 가능, 단 **index 필수 업데이트**
3. 브레인스토밍/외부 참고는 **`canonical: false`** 및 성격 명시
4. 대체 문서는 **`superseded_by`** 또는 본문 상단 경고 추가
