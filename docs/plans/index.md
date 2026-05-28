# Plans

Plan document index. Register new plans here.

## Active

### seCall MVP — 에이전트 세션 검색 인프라

- [전체 계획서](secall-mvp.md) — draft, v2.0, 2026-04-05

| # | Title | Phase | Depends | Status |
|---|---|---|---|---|
| [01](secall-mvp-task-01.md) | Rust workspace 초기화 | 0 | — | draft |
| [02](secall-mvp-task-02.md) | SQLite 스키마 설계 + 초기화 | 0 | 01 | draft |
| [03](secall-mvp-task-03.md) | Claude Code JSONL 파서 | 1 | 01 | draft |
| [04](secall-mvp-task-04.md) | Markdown 렌더러 | 1 | 03 | draft |
| [05](secall-mvp-task-05.md) | Vault 구조 초기화 + index/log | 1 | 04 | draft |
| [06](secall-mvp-task-06.md) | 한국어 BM25 인덱서 | 2 | 02,03 | draft |
| [07](secall-mvp-task-07.md) | 벡터 인덱서 + 검색 | 2 | 02,03 | draft |
| [08](secall-mvp-task-08.md) | 하이브리드 검색 (RRF) | 2 | 06,07 | draft |
| [09](secall-mvp-task-09.md) | CLI 완성 | 3 | 05,08 | draft |
| [10](secall-mvp-task-10.md) | MCP 서버 | 3 | 08 | draft |
| [11](secall-mvp-task-11.md) | Ingest 완료 이벤트 + hook | 3 | 09 | draft |

---

### seCall Extensions — 멀티에이전트 + 로컬 NLP

- [전체 계획서](secall-extensions-nlp.md) — draft, v1.0, 2026-04-06

| # | Title | Group | Depends | Status |
|---|---|---|---|---|
| [01](secall-extensions-nlp-task-01.md) | Codex CLI 파서 | A | — | draft |
| [02](secall-extensions-nlp-task-02.md) | Gemini CLI 파서 | A | — | draft |
| [03](secall-extensions-nlp-task-03.md) | ort ONNX 로컬 임베딩 | B | — | draft |
| [04](secall-extensions-nlp-task-04.md) | kiwi-rs 토크나이저 | B | — | draft |
| [05](secall-extensions-nlp-task-05.md) | secall lint | C | 01,02,03,04 | draft |

---

### seCall Wiki — Claude Code 메타에이전트 기반 위키 생성

- [전체 계획서](secall-wiki-claude-code.md) — draft, v1.0, 2026-04-06

| # | Title | Group | Depends | Status |
|---|---|---|---|---|
| [01](secall-wiki-claude-code-task-01.md) | Wiki Vault 구조 초기화 | A | — | draft |
| [02](secall-wiki-claude-code-task-02.md) | 메타에이전트 프롬프트 설계 | A | — | draft |
| [03](secall-wiki-claude-code-task-03.md) | secall wiki CLI 커맨드 | B | 01,02 | draft |
| [04](secall-wiki-claude-code-task-04.md) | post-ingest hook 연동 | B | 03 | draft |
| [05](secall-wiki-claude-code-task-05.md) | 위키 품질 검증 (lint 확장) | B | 01 | draft |

---

### seCall Phase 4 — 검색 고도화 + 인프라 완성

- [전체 계획서](secall-phase-4.md) — draft, v1.0, 2026-04-06

| # | Title | Group | Depends | Status |
|---|---|---|---|---|
| [01](secall-phase-4-task-01.md) | ort 모델 자동 다운로드 | A | — | draft |
| [02](secall-phase-4-task-02.md) | OpenAI 임베딩 API embedder | A | — | draft |
| [03](secall-phase-4-task-03.md) | MCP HTTP transport | A | — | draft |
| [04](secall-phase-4-task-04.md) | LLM 쿼리 확장 | A | — | draft |

---

### seCall Refactor P0 — 검색 정확성 결함 수정

- [전체 계획서](secall-refactor-p0.md) — draft, v1.0, 2026-04-06

| # | Title | Group | Depends | Status |
|---|---|---|---|---|
| [01](secall-refactor-p0-task-01.md) | BM25 turn_index 수정 | A | — | draft |
| [02](secall-refactor-p0-task-02.md) | vault_path 상대경로 전환 | A | — | draft |
| [03](secall-refactor-p0-task-03.md) | Lint L002 session_id 추출 수정 | A | — | draft |

---

### seCall Refactor P1 — 에러 처리 + 데이터 정합성

- [전체 계획서](secall-refactor-p1.md) — draft, v1.0, 2026-04-06

| # | Title | Group | Depends | Status |
|---|---|---|---|---|
| [01](secall-refactor-p1-task-01.md) | ingest.rs 에러 전파 | A | — | draft |
| [02](secall-refactor-p1-task-02.md) | db.rs Result 반환 전환 | A | — | draft |
| [03](secall-refactor-p1-task-03.md) | ingest 트랜잭션 래핑 | B | 01,02 | draft |
| [04](secall-refactor-p1-task-04.md) | Codex/Gemini 타임스탬프 복원 | A | — | draft |

---

### seCall Refactor P2 — 인프라 + 성능

- [전체 계획서](secall-refactor-p2.md) — draft, v1.0, 2026-04-06

| # | Title | Group | Depends | Status |
|---|---|---|---|---|
| [01](secall-refactor-p2-task-01.md) | tracing 도입 | A | — | draft |
| [02](secall-refactor-p2-task-02.md) | 벡터 검색 메모리 최적화 | A | — | draft |
| [03](secall-refactor-p2-task-03.md) | 디렉토리 ingest 멀티에이전트 | A | — | draft |
| [04](secall-refactor-p2-task-04.md) | BLOB 검증 + CLI/MCP 테스트 | A | — | draft |

---

### seCall P16 — Knowledge Graph 빌드

- [전체 계획서](secall-p16-knowledge-graph.md) — in_progress, v2.0, 2026-04-10

| # | Title | Group | Depends | Status |
|---|---|---|---|---|
| [01](secall-p16-knowledge-graph-task-01.md) | DB 스키마 + 마이그레이션 | A | — | pass |
| [02](secall-p16-knowledge-graph-task-02.md) | Graph 코어 모듈 (rework) | B | 01 | rework |
| [03](secall-p16-knowledge-graph-task-03.md) | CLI 서브커맨드 | C | 01,02 | pass |
| [04](secall-p16-knowledge-graph-task-04.md) | Sync 통합 + MCP 확장 (rework) | D | 01,02,03 | rework |

---

### seCall P17 — 대화형 온보딩 + 설정 CLI + git branch 수정

- [전체 계획서](secall-p17-cli-git-branch.md) — draft, v1.0, 2026-04-10

| # | Title | Group | Depends | Status |
|---|---|---|---|---|
| [01](secall-p17-cli-git-branch-task-01.md) | git branch 하드코딩 제거 | A | — | draft |
| [02](secall-p17-cli-git-branch-task-02.md) | secall config 서브커맨드 | B | 01 | draft |
| [03](secall-p17-cli-git-branch-task-03.md) | 대화형 온보딩 (secall init 개선) | C | 01,02 | draft |
| [04](secall-p17-cli-git-branch-task-04.md) | status 설정 요약 표시 | D | 02 | draft |

---

### seCall P82 — Config::save() integration test 가드 확장

- [전체 계획서](p82-config-save-guard.md) — in_progress, 2026-05-19
- 단일 Task: `Config::save()` 의 `#[cfg(test)]` 가드를 runtime env (`SECALL_TEST_MODE`) 로 확장해 integration test 까지 보호.
- 관련: `docs/reference/core-backlog.md` hot 1건 해소 대상.

---

### seCall P83 — Wiki self-ingest 루프 차단 (issue #82)

- [전체 계획서](p83-wiki-self-ingest-loop.md) — in_progress, 2026-05-19
- 단일 Task: `wiki/{codex,claude}.rs` 의 generate() 가 prompt 앞에 `WIKI_INVOCATION_MARKER` prefix → `is_noise_session()` 이 marker 검출 시 skip → wiki 호출이 생성한 codex/claude 세션의 self-ingest 루프 차단.
- 관련: issue #82 (dicebattle).

---

### seCall P84 — `lint --fix-wiki-invocations` (P83 fast-follow)

- [전체 계획서](p84-lint-fix-wiki-invocations.md) — in_progress, 2026-05-19
- 단일 Task: `check_wiki_invocations()` (L011 신규) — cwd 가 vault path 인 codex/claude 세션 검출 + `secall lint --fix-wiki-invocations` 옵션으로 일괄 archive. P83 marker 가 없는 legacy 데이터 사후 정리.
- 관련: issue #82 fast-follow.

---

### seCall P85 — Wiki generation timeout config option (issue #87)

- [전체 계획서](p85-wiki-timeout-config.md) — in_progress, 2026-05-19
- 단일 Task: `[wiki].generation_timeout_secs` config (default 1800) — claude/codex/ollama/lmstudio backend 의 hardcoded 1800s 를 사용자 override 가능하게 함.
- 관련: issue #87 (cakel).

---

### seCall P86 — Wiki update + ollama/lmstudio 백엔드 fail-fast (issue #88)

- [전체 계획서](p86-ollama-batch-fail-fast.md) — in_progress, 2026-05-19
- 단일 Task: `commands/wiki.rs` 의 backend 선택 직후 ollama/lmstudio 차단 + 가이드 메시지. silent 30분 wait 사고 차단.
- 관련: issue #88 (cakel).

---

### seCall P87 — Windows `.cmd` 래퍼 CLI spawn 실패 fix (issue #92)

- [전체 계획서](p87-windows-cmd-spawn.md) — in_progress, 2026-05-29
- 단일 Task: `which` crate 로 `resolve_program` 추가 — Windows PATHEXT 적용해 npm `.cmd` 래퍼 (codex/claude) 를 정상 spawn. spawn 5곳 + `command_exists` 통일.
- 관련: issue #92 (cakel).

---

### seCall P88 — claude+haiku wiki generation 경고 (issue #93)

- [전체 계획서](p88-haiku-wiki-warn.md) — in_progress, 2026-05-29
- 단일 Task: generation 경로에서 claude+haiku 조합 감지 시 경고 (sonnet/opus 권장). 코드 버그 아닌 모델 capability — 차단 대신 안내.
- 관련: issue #93 (cakel).
