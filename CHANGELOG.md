# Changelog

> NOTE: v0.3.x ~ v0.4.x 의 상세 변경 이력은 `README.md` 의 "버전 히스토리" 표 참고. CHANGELOG.md 는 v0.2.x 시점에서 README 로 SSOT 이전됨.

## v0.6.0 (2026-05-19)

P56 ~ P86 누적 — wiki self-ingest 루프 차단 (issue #82), wiki backend timeout config (issue #87), ollama/lmstudio fail-fast (issue #88), web 디자인 / model discovery / graph snapshot 개선, CI 시간 단축, 문서 룰 정리.

### ⚠️ Behavior changes (non-breaking)

- **`wiki update --backend ollama` / `--backend lmstudio` 명시적 에러** (#89, P86): 이전엔 silent fail (모델이 "임무 이해" 응답 후 종료, 사용자가 timeout 까지 wait). 이제 즉시 가이드 메시지 출력 (issue #88). 기본 동작 변경이지만 작동하지 않던 조합이라 사용자 일반 사용 영향 없음.
- **wiki 호출 codex/claude 세션 자동 skip** (#85, P83): `wiki/{codex,claude}` 가 prompt 앞에 `WIKI_INVOCATION_MARKER` prefix → `is_noise_session()` 이 검출 시 skip → wiki self-ingest 루프 차단 (issue #82). 정상 사용자 세션 영향 없음.

### ✨ Features

- **wiki cloud (`ollama_cloud`) backend + claude haiku alias** (#66, P56): `WikiBackendConfig` 에 `cloud_api_key`/`cloud_host` 필드. graph/log 와 분리된 wiki 전용 키 / 엔드포인트 가능.
- **claude stdout line-stream 실시간 표시** (#68, P58 follow-up): wiki update 시 claude CLI 출력을 매 line `[claude]` prefix 로 stderr 에 echo. 5분 timeout 동안 "아무 반응 없음" 으로 보이던 사용자 인식 개선. Windows CRLF 호환.
- **ollama / lmstudio HTTP response streaming** (#70, P60): `/api/generate` (NDJSON) + LM Studio SSE 스트림 파싱. ollama 도 `[ollama]` prefix line-by-line stderr echo.
- **TopNav version SSOT** (#72, P62): web 의 version 표시를 `/api/status` 가 server-side 단일 출처로 통합. `web-backlog.md` 신설.
- **MarkdownView 폴딩 / highlight / wikilink 확장** (#75, P66): `rehype-raw` + `rehype-highlight` + `remark-wiki-link` 통합. session/wiki 페이지가 Obsidian 호환 markdown 풀 렌더.
- **`/api/graph/snapshot` edge_limit + 우선순위 sampling** (#76, P64): 대용량 graph 응답 멈춤 회피. edge 우선순위 (degree + path importance) sampling.
- **backend 별 model discovery + cache + REST endpoint** (#78, P65): `/api/models?backend=...` — wiki 설정 화면이 사용 가능 모델 list 자동 표시. 캐시 TTL 적용.
- **Obsidian callout (`> [!type]-`) → `<details class="callout callout-<type>">`** (#81, P81): 자체 remark plugin. note/warning/info 등 callout 타입별 시각 구분.
- **`secall lint --fix-wiki-invocations`** (#86, P84, issue #82 fast-follow): L011 신규 — cwd 가 vault path 인 codex/claude 세션 (legacy wiki invocation) 을 일괄 archive. P83 marker 가 없는 머지 전 데이터 사후 정리.
- **`[wiki].generation_timeout_secs` config** (#90, P85, issue #87): claude/codex/ollama/lmstudio backend timeout 사용자 override. 기본값 1800 (hardcoded 와 동일, backward-compat).

### 🐛 Fixes

- **wiki backend timeout 300s → 1800s** (#69, P59): 5분 timeout 동안 정상 케이스도 SIGKILL 회귀 (sync-monitor 2026-05-15 관측). 30분으로 상향 + `kill_on_drop` 유지.
- **`Config::save()` production config 덮어쓰기 차단 가드 (unit test)** (#74, P68): `#[cfg(test)]` 가드 — `SECALL_CONFIG_PATH` 미설정 시 production 경로 거부. cargo test flaky 시도 중 사용자 vault path 덮인 사고 (2026-05-16) 재발 차단.
- **`Config::save()` 가드 integration test 확장** (#84, P82): runtime env (`SECALL_TEST_MODE`) 로 확장 — `#[cfg(test)]` 가 false 인 integration test 컨텍스트까지 보호. `tests/common::ensure_test_mode()` + `make_test_env()` 자동 호출.
- **Wiki self-ingest 루프 차단** (#85, P83, Closes #82): `wiki/{codex,claude}` 가 prompt 앞에 marker prefix + `is_noise_session()` 이 검출 시 skip. 무한 wiki 재생성 / 중복 항목 차단.
- **`wiki update + ollama/lmstudio` 명시적 fail-fast** (#89, P86, Closes #88): MCP 도구 호출 능력이 없는 백엔드 + batch/incremental prompt 조합을 silent fail 대신 즉시 가이드 메시지.
- **Gemini P66 리뷰: security/a11y** (#77): `rehype-sanitize` 적용 + onKeyDown 키보드 접근성 + `<details open>` 허용.
- **Gemini P66 follow-up: wiki frontmatter strip + ModelInput dropdown** (#79): heading collapse 폐기 + 자체 dropdown (chevron + 키보드 nav + click outside).

### 🧹 Refactor / Internal

- **README history table 정리** (#67, P57): git tag SSOT 로 정리.
- **ja/zh README full sync** (#71, P61): 한국어 README 를 SSOT 으로 일본어 / 중국어 README 전체 동기화.
- **core-backlog 신설 + web-backlog 전수 조사** (#73, P63): 도메인별 backlog 분리.
- **CI 시간 단축** (#80, P80): `cargo-audit` / `cargo-nextest` binary install (`taiki-e/install-action`) + `--all-features` 제거. ubuntu/windows 워크플로우 단축.
- **문서 룰 정리 + index 보강 + web-backlog 청소 + handoff** (#83): `docConventions.md` 신설 (tunaflow versioning + navigation 정책 압축) + `reference/index.md` 누락 10개 파일 등록 + `handoff_2026-05-19.md` cold-start 인수인계.

---

## v0.5.0 (2026-05-15)

P49 ~ P55 누적 — 데이터 품질 / 클라우드 LLM 통합 / 거대 함수 분해 / wiki hang 차단.

### ⚠️ Breaking changes

- **vault 디렉토리 rename**: `raw/sessions/` → `raw/.sessions/` (#57, P49). obsidian 의 core 인덱서 + dataview / graph 가 dot-prefix 디렉토리를 자동 hidden 처리 → vault freeze (1259+ 새 md 한번에 들어올 때) 회피. 기존 vault 사용자는 `mv raw/sessions raw/.sessions` + `UPDATE sessions SET vault_path = REPLACE(vault_path, 'raw/sessions/', 'raw/.sessions/')` 마이그레이션 필요.
- **`[graph] semantic_backend` / `[log] backend` default cloud** (#60, P51): 디폴트가 `"ollama_cloud"`. `OLLAMA_CLOUD_API_KEY` 없는 환경은 config 에 `backend = "ollama"` 명시 필요.
- **`OllamaReviewer` 시그니처 변경** (#64, P55): `api_key: Option<String>` 필드 추가. 외부 사용처는 `api_key: None` 필요.

### ✨ Features

- **TMPDIR/secall-prompt 노이즈 ingest 차단** (#57, P49): cwd 가 `$TMPDIR` 또는 첫 user turn 이 secall summary prompt prefix 면 skip. 자기참조 ingest 루프 회피.
- **vault 렌더링 헤더 강등** (#57, P49): 같은 role 의 연속 turn 헤더 h2 → h3 (role 명 생략). 한 LLM 응답이 tool_use 별로 쪼개져 `## Turn N — Assistant` 가 도배되던 노이즈 제거.
- **`LlmBackend` trait + 4 백엔드 통합** (#58, P50-B): graph semantic 추출의 Anthropic / Ollama / Ollama-Cloud / OpenAI-compat 직접 HTTP 호출이 trait 한 곳으로 통합. wiki/mod.rs 의 WikiBackend 패턴 차용.
- **wiki/ingest 거대 함수 분해** (#59, P50-C/D/E): `run_update_with_sink` (405L) / `ingest_sessions` (369L) 를 dispatcher + 4-5 helper 로 분리. `write_wiki_page` / `maybe_review_with_regen` 중복 제거.
- **graph/log 디폴트 cloud + wiki review haiku** (#60, P51): config 미설정 시 cloud 사용 (`gemma4:31b-cloud` / `kimi-k2.6:cloud`). review default sonnet → haiku.
- **`--fix-orphan-vault` 옵션** (#63, P54): `secall lint --fix-orphan-vault` 가 L002 finding (vault md ↔ DB session 불일치) 의 md 를 `<vault>/archive/orphan-<YYYY-MM-DD>/` 로 이동 (삭제가 아닌 archive).
- **`ollama_cloud` wiki review/generation backend** (#64, P55): Anthropic 키 없는 환경에서도 cloud 로 review/wiki 가능. `OLLAMA_CLOUD_API_KEY` + bearer auth.

### 🐛 Fixes

- **wiki backend `generate()` 4종 모두 300s timeout** (#61, P52): claude / codex CLI 와 ollama / lmstudio HTTP 가 timeout 없이 무한 hang 가능했음. "sonnet 계속 로딩" 사용자 보고의 root cause 차단. `kill_on_drop` + tokio timeout 조합.
- **`wiki update --since` 표시 정확화** (#62, P53): stderr 메시지가 `--since` 옵션을 반영 안 하고 무조건 `all sessions` 로 표기되던 이슈. `build_target_label` helper 로 통일.

### 🧹 Refactor / Internal

- `graph/llm.rs` 신규 (P50-B): trait + 4 backend impl, 단위 테스트 5건
- `commands/wiki.rs` 모드별 dispatcher 분리 (P50-D): preflight_vault_git / process_haiku_batch / process_haiku_incremental / process_generic_backend
- `commands/ingest.rs` 분해 (P50-E): compile_classification_rules / ingest_path / embed_vector_tasks / extract_semantic_edges_batch
- secall-core unwrap audit (821건) — production 위험 0 확인

### 🔍 검증 (sync-monitor 2026-05-15)

- graph rebuild cloud: 1240 sessions 100% 성공 (3469 edges)
- log diary cloud: kimi-k2.6:cloud 15s 한국어 일기 정상
- wiki claude CLI 5분 hang → P52 timeout 정확히 300s 발동 + SIGKILL 차단
- embed 1240/26946 chunks 정상 (local Ollama)

## v0.2.3 (2026-04-09)

### Added
- ChatGPT export 파서 (`ChatGptParser`) — `conversations.json` ZIP/JSON 파싱
- `mapping` HashMap → `current_node` 부모 체인 추적으로 대화 선형화 (재생성 분기 자동 처리)
- 멀티 content type 지원: text, code, multimodal_text, execution_output, reasoning_recap, thoughts, tether_browsing_display, user_editable_context
- `AgentKind::ChatGpt` variant
- `detect.rs`에 ChatGPT export 자동 탐지 로직 추가
- **Windows 빌드 지원** (`x86_64-pc-windows-msvc`)
  - CI에 `windows-latest` 매트릭스 추가 (fmt, clippy, test)
  - Release 워크플로우에 Windows 타겟 추가 + ORT DLL (`onnxruntime.dll 1.22.0`) 번들링
  - `tokenizers` 피처 `onig` → `fancy-regex` (순수 Rust, 전 플랫폼 통일)
  - `kiwi-rs`를 `cfg(not(target_os = "windows"))` 조건부 의존성으로 분리 (Windows에서는 lindera fallback)

## v0.2.2 (2026-04-08)

### Added
- `config.toml`에 `[output] timezone` 설정 추가 — IANA 타임존(예: `Asia/Seoul`)으로 vault 마크다운 타임스탬프 렌더링. 기본값 UTC.

### Changed
- vault 디렉토리 경로(`raw/sessions/YYYY-MM-DD/`)가 설정된 타임존 기준 날짜로 생성
- frontmatter `start_time`/`end_time`에 동적 UTC 오프셋 적용 (예: `+09:00`)

## v0.2.1 (2026-04-08)

### Added
- `secall ingest --force` — 이미 인덱싱된 세션도 강제 재수집. vault MD 재생성 + DB 재삽입. claude.ai 재export나 렌더링 변경 적용 시 사용.

### Fixed
- Dataview inline field 오염 방지 — vault 마크다운 body의 `::` 패턴에 zero-width space 삽입하여 Dataview가 인라인 필드로 해석하지 않도록 처리. fenced code block / inline code 내부는 보존.

## v0.2.0 (2026-04-07)

### Added
- claude.ai 공식 export JSON 파서 (`ClaudeAiParser`)
- ZIP 자동 해제 지원 (`secall ingest <export.zip>`)
- `AgentKind::ClaudeAi` variant
- `SessionParser::parse_all()` — 1:N 파싱 지원

### Changed
- `AgentKind` enum에 `ClaudeAi` variant 추가
- `detect.rs`에 claude.ai export 자동 탐지 로직 추가

## v0.1.0 (2026-04-06)

### Added
- 초기 릴리스
- Claude Code / Codex CLI / Gemini CLI 파서
- BM25 + 벡터 하이브리드 검색 (RRF k=60)
- MCP 서버 (stdio + HTTP)
- Obsidian 호환 vault 구조
- Git 기반 멀티 기기 동기화 (`secall sync`)
- ANN 인덱스 (usearch HNSW)
- CI/CD GitHub Actions
