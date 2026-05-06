# secall 바이너리 dotenv 자동 로드 부재

- **Category**: stability (STA)
- **Severity**: minor
- **Fix Difficulty**: easy
- **Status**: resolved
- **Resolved At**: 2026-05-05
- **Resolved By**: crates/secall/src/main.rs:382 (P39 hot-fix)
- **File**: crates/secall/src/main.rs:382

## Description

`secall` CLI 바이너리가 부팅 시 `.env` 파일을 자동 로드하지 않아, 프로젝트 루트의 `.env` 에 보관한 `GEMINI_API_KEY` / `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` 등을 사용자가 별도 셸에서 `source .env` 또는 `export` 로 주입해야만 했습니다. P39 baseline 측정 중 `graph rebuild --since` 실행 시 Gemini 백엔드가 키를 찾지 못하고 실패.

## Evidence

- `secall graph rebuild --since 2026-05-05` 실행 → "GEMINI_API_KEY not set" 에러로 28 sessions 미처리.
- 프로젝트 루트 `.env` 에 키는 정상 보관됨. 다른 도구 (e.g. dev 서버) 는 자체 dotenv 로더로 동작.
- README 빠른 시작 가이드에 `source .env` 명시 없음 → 사용자 혼란.

## Fix

`crates/secall/src/main.rs:382` 의 `main()` 진입 직후 `let _ = dotenvy::dotenv();` 추가. `Cargo.toml` 에 `dotenvy = "0.15"` 의존성 추가. 실패 (파일 없음) 는 silent — 환경변수가 이미 셸에 있으면 그대로 사용. CI/배포 환경 영향 없음.

수정 후 `graph rebuild --since 2026-05-05` 재실행 → 28 sessions / 840 edges 백필 정상 완료.
