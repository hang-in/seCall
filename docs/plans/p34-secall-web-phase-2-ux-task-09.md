---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 09
parallel_group: C
depends_on: [00, 01, 04]
---

# Task 09 — README + CI 업데이트

## Changed files

수정:
- `README.md` — Phase 2 섹션 추가 (시맨틱 검색, 검색어 하이라이트, 다중 태그/날짜 quick range, 키보드 단축키, 관련 세션 패널, 그래프 시각화 강화, mini-chart, 노트), 엔드포인트 목록에 `/api/sessions/{id}/notes` 추가
- `README.en.md` — 동일 영문판
- `.github/workflows/ci.yml` (선택) — 변경 없음 (P34 신규 테스트는 `cargo test --all`로 자동 실행)

신규: 없음

## Change description

### 1. README — Web UI 섹션 확장

기존 Phase 0/1 다음에 Phase 2 추가:

```markdown
**Phase 2** (P34, 뷰어 강화):
- 시맨틱 검색 모드 토글 (Ollama 사용 시)
- 검색어 하이라이트 — 리스트 + 마크다운 본문 양쪽
- 다중 태그 AND 필터 + 날짜 quick range (오늘/이번 주/이번 달)
- 키보드 단축키 — `?` 도움말, `j/k` 리스트 이동, `/` 검색 포커스, `g d/w/s/c` 라우트, `[/]` 세션 prev/next, `f` 즐겨찾기, `e` 노트
- 관련 세션 패널 — 그래프 인접 + 같은 프로젝트/태그 추천 (세션 상세 하단)
- 그래프 시각화 강화 — dagre 자동 레이아웃 + 노드 타입별 색상/아이콘 + 엣지 라벨 토글 + 범례
- 세션 메타 mini-chart — turn role 분포 (user/assistant/system) + tool 사용 빈도 top 5
- 사용자 노트 편집 — 세션별 markdown 노트 (autosave 1s)
```

### 2. README — 엔드포인트 목록 갱신

기존 19개에서 1개 추가:
```
- 세션 메타 (Phase 0): /api/sessions, /api/projects, /api/agents,
  PATCH /api/sessions/{id}/{tags,favorite}
- 세션 노트 (Phase 2): PATCH /api/sessions/{id}/notes
```

### 3. README — 키보드 단축키 표

```markdown
### 키보드 단축키 (Phase 2)

| 키 | 동작 |
|---|---|
| `?` | 단축키 도움말 |
| `/` | 검색 포커스 |
| `j` / `k` | 리스트 다음/이전 항목 |
| `[` / `]` | 세션 prev/next |
| `g d` | Daily 화면 |
| `g w` | Wiki 화면 |
| `g s` | Sessions 화면 |
| `g c` | Commands 화면 |
| `g g` | 그래프 오버레이 토글 |
| `f` | 현재 세션 즐겨찾기 토글 |
| `e` | 현재 세션 노트 편집 |
| `Esc` | 다이얼로그/오버레이 닫기 |
```

### 4. README.en.md 동기화

위 한글 섹션을 영문으로 동등하게 추가.

### 5. 변경 이력 갱신 (선택)

```markdown
| 2026-XX-XX | v0.5.0 | Web UI Phase 2 (P34): 시맨틱 검색 모드 활성, 검색어 하이라이트, 다중 태그+날짜 quick range, 키보드 단축키, 관련 세션 패널, 그래프 시각화 강화 (dagre + 노드 색상), 세션 메타 mini-chart, 사용자 노트 편집 (PATCH /api/sessions/{id}/notes), DB 스키마 v7 |
```

### 6. CI 변경 (확인만)

`cargo test --all`이 P34 신규 테스트 (`test_v7_*`, `test_get_session_stats_*`, `test_list_sessions_multi_tag_*`)를 자동 실행. workflow 변경 없음.

## Dependencies

- 외부 도구: 없음
- 내부 task: Task 01 (DB v7 + notes 엔드포인트), Task 02 (시맨틱 검색), Task 05 (단축키) 완료 후 README 정확성 보장

## Verification

```bash
# README 마커
grep -q "Phase 2" /Users/d9ng/privateProject/seCall/README.md && echo "ko OK"
grep -q "Phase 2" /Users/d9ng/privateProject/seCall/README.en.md && echo "en OK"
grep -q "/api/sessions/.*/notes" /Users/d9ng/privateProject/seCall/README.md && echo "notes endpoint listed"
grep -qE "키보드 단축키|Keyboard shortcuts" /Users/d9ng/privateProject/seCall/README.md && echo "hotkey doc present"

# CI workflow 변경 없음 확인
git diff --stat .github/workflows/ | head -3

# 회귀
cargo test --all
cd web && pnpm typecheck && pnpm build
```

## Risks

- **README 일관성**: 사용자가 보는 동작과 README 설명이 어긋나면 신뢰 저하. Task 01/02/05/09 검증 통과 후 본 task 진행
- **단축키 표가 실제 구현과 다름**: Task 05의 useGlobalHotkeys 등록과 일치해야 함 — 표 업데이트 시 cross-check
- **버전 bump**: 본 task에서 Cargo.toml 버전은 변경 안 함. release tagging은 별도

## Scope boundary

수정 금지:
- `crates/`, `web/src/` 코드 — Task 01~09 완료 후 본 task는 문서만
- DB 스키마 — Task 01에서 v7 추가
- 기존 마이그레이션 분기 — Task 01만 v7 추가
