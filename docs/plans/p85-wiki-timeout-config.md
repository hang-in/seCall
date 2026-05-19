---
type: plan
status: in_progress
updated_at: 2026-05-19
canonical: true
---

# P85 — Wiki generation timeout config option (issue #87)

## 배경

Issue #87 (cakel, 2026-05-19): `claude wiki generation timed out after 300s`. cakel 환경에서 wiki update 가 timeout. 사용자 요청: option 으로 변경 가능 여부.

진단 결과:
- 현재 main 의 모든 long-generation wiki backend (claude/codex/ollama/lmstudio) timeout 은 **P59 (#69, 2026-05-15)** 에서 1800s 로 상향됨. 300s 에러는 cakel 이 P59 이전 버전 사용 의심 → release 진행 (별도) 으로 fix 가능.
- 단 1800s 도 부족한 케이스 (수만 세션 vault 또는 느린 모델) 가 있을 수 있어, **사용자가 config 로 override** 할 수 있는 옵션 추가가 정당함.

## 목표

- `config.toml` 의 `[wiki].generation_timeout_secs` 로 long-generation backend timeout 조정 가능.
- default 1800 (기존 hardcoded 값 유지) → backward-compat.
- 사용자 설정: `[wiki] generation_timeout_secs = 3600` 같이 30분 → 1시간 변경 가능.

## 비목표

- haiku (Anthropic API HTTP 120s) + reviewers (60~120s) 의 timeout 은 별도 — 단일 API call timeout 이라 long generation 과 다름.
- log/graph backend 의 timeout 도 별개 영역. 단 log 가 wiki backend struct 를 reuse 하므로 동일 값 적용 (자연스러움).

## 변경 파일

| 파일 | 변경 |
|---|---|
| `crates/secall-core/src/vault/config.rs` | `WikiConfig` 에 `generation_timeout_secs: u64` (default 1800) 추가 |
| `crates/secall-core/src/wiki/claude.rs` | `ClaudeBackend` 에 `timeout_secs` 필드 + `generate()` 가 self.timeout_secs 사용 |
| `crates/secall-core/src/wiki/codex.rs` | 동일 |
| `crates/secall-core/src/wiki/ollama.rs` | 동일 (test 3건 갱신 포함) |
| `crates/secall-core/src/wiki/lmstudio.rs` | 동일 (test 3건 갱신 포함) |
| `crates/secall-core/tests/sync_termination.rs` | ClaudeBackend / CodexBackend literal 갱신 |
| `crates/secall/src/commands/wiki.rs` | `build_wiki_backend()` 가 `config.wiki.generation_timeout_secs` 전달 (5 instantiation) |
| `crates/secall/src/commands/log.rs` | log command 가 backend 생성 시 동일 값 전달 (5 instantiation) |
| `docs/plans/p85-wiki-timeout-config.md` (신규) | 본 plan |
| `docs/plans/index.md` | P85 등록 |

## 사용 예시

```toml
# ~/Library/Application Support/secall/config.toml
[wiki]
generation_timeout_secs = 3600  # 30분 → 1시간
```

```
$ secall wiki update --backend claude
...
# (수만 세션 분석 후 1시간 한도 안에서 정상 완료)
```

## 검증

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p secall-core --lib wiki::config
cargo test --workspace --no-fail-fast
```

## 리스크

- 사용자가 너무 작은 값 (예: 60s) 설정 시 정상 생성도 timeout. 사용자 책임.
- log 도 wiki 의 timeout 사용 — log 가 별도 timeout 필요해지면 future 에 `[log].generation_timeout_secs` 추가 가능 (현재는 unify).

## 후속

- issue #87 에 머지 안내 comment.
- release notes 에 명시 (v0.6.0 changelog 항목).
