# Changelog

## v0.2.1 (2026-04-08)

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
