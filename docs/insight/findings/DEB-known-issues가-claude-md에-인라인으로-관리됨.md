# Known Issues가 CLAUDE.md에 인라인으로 관리됨

- **Category**: debt
- **Severity**: minor
- **Fix Difficulty**: guided
- **Status**: open
- **File**: CLAUDE.md:97

## Description

CLAUDE.md의 Known Issues 섹션에 FTS 중복 잔존 등 미해결 기술 부채가 텍스트로 기록되어 있습니다. 이슈 트래커(GitHub Issues)와 이중 관리되면 동기화 누락 위험이 있고, 실제 수정 여부를 코드 리뷰 없이 확인하기 어렵습니다.

**Evidence**: `- 기존 DB에 FTS 중복 잔존 (--force reingest로 세션별 정리 가능)
- Issue #26 — Codex wiki 백엔드 추가 (외부 기여 PR 요청 중)`

## Snippet

```
### Known Issues
- 기존 DB에 FTS 중복 잔존 (--force reingest로 세션별 정리 가능)
- Issue #26 — Codex wiki 백엔드 추가 (외부 기여 PR 요청 중)
```
