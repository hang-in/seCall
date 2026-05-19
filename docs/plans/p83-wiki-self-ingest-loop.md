---
type: plan
status: in_progress
updated_at: 2026-05-19
canonical: true
---

# P83 — Wiki 호출이 만든 codex/claude 세션을 ingest 가 자동 archive

## 배경

Issue #82 (dicebattle / 2026-05-18) 보고:

> codex를 이용해서 세션을 요약시키니, 세션 요약 과정에서 새로운 codex 세션이 생기고, 그걸 다시 새로운 대화세션으로 인식해서 LLM 요약 과정을 다시 위키에 저장하는 상황

진단 결과 codex + claude 두 wiki 백엔드 모두 동일 결함:

1. `wiki update` → `backend.generate(prompt)` 호출
2. `tokio::process::Command::new("codex"|"claude")` subprocess 실행 (cwd = `vault_path`)
3. CLI 가 자체 세션 파일 (`~/.codex/sessions/...` 또는 `~/.claude/projects/...`) 생성
4. `secall sync` 가 그 디렉토리 스캔 (`find_codex_sessions` / `find_claude_sessions`)
5. wiki 호출로 생긴 세션도 일반 사용자 세션과 동일하게 ingest
6. 다음 wiki 호출 또는 자동 hook 실행 시 그 세션도 분석 대상에 포함 → 중복 wiki / 무한 루프
7. `ingest/{codex,claude}.rs` 어디에도 wiki 호출 식별 / skip 룰 없음

다른 백엔드 (haiku, ollama, lmstudio) 는 HTTP API 라 영향 없음.

## 목표

- codex + claude wiki 호출이 생성한 세션을 ingest 시 자동으로 `archived = true` 로 마킹.
- 기존에 이미 ingest 된 세션도 사후 정리 가능 (lint 또는 sync --force).
- false positive 최소화 (사용자가 의도해서 만든 일반 세션은 영향 없음).

## 비목표

- 사용자가 vault 디렉토리에서 직접 codex/claude 를 일반 작업으로 사용한 경우까지 archive 하지 않는다 (단, marker prefix 가 robust 한 식별자라 이게 우선 적용).
- wiki 호출 자체의 동작은 변경 없음 (prompt 에 marker prefix 만 추가).

## Fix 방향 — 기존 `is_noise_session()` 에 marker 룰 추가

조사 결과 `ingest/mod.rs:48` 의 `is_noise_session()` 이 이미 P49 에서 비슷한 self-ingest 패턴 (tmpdir cwd + Claude Code summary prompt) 을 차단하고 있고, `commands/ingest.rs:976` 에서 호출되어 매치 시 **skip 처리** (archive 가 아닌 skip). 즉 parser 시그니처 변경 없이 룰 한 줄 추가만으로 완성.

| 검출 룰 | 적용 |
|---|---|
| (기존) cwd tmpdir | `/private/var/folders`, `/var/folders`, `/tmp` |
| (기존) Claude Code summary prompt | `SECALL_SUMMARY_PROMPT_PREFIX` |
| **(신규) wiki invocation marker** | 첫 user turn content 에 `WIKI_INVOCATION_MARKER` 포함 |

처리 방식: skip (DB insert 자체 안 함). archive 보다 깔끔.

### cwd vault 룰 미채택 사유

옵션 2 (cwd 가 vault path 면 match) 도 검토했으나:
- 사용자가 vault 디렉토리에서 직접 codex/claude 작업 시 false positive
- marker 룰만으로도 robust (wiki 호출 prompt 는 항상 marker prefix)
- false positive 위험이 추가 안전 마진 대비 큼

별도 후속 PR (lint 신규 옵션 또는 sync --force) 에서 cwd 룰 검토 가능.

## 구현 절차

### 1. `crates/secall-core/src/wiki/mod.rs` — 공통 marker 상수

```rust
/// P83: wiki 호출 prompt 첫 줄에 prefix 로 추가되는 식별자.
/// codex/claude CLI 가 만든 세션을 ingest 가 자동 archive 하는 marker.
pub const WIKI_INVOCATION_MARKER: &str = "<!-- secall:wiki-update -->";
```

### 2. `crates/secall-core/src/wiki/codex.rs` 와 `wiki/claude.rs` — prompt prefix

각 `generate()` 시작에서:
```rust
let marked_prompt = format!("{}\n\n{}", crate::wiki::WIKI_INVOCATION_MARKER, prompt);
```
그리고 `marked_prompt` 를 stdin 으로.

### 3. `crates/secall-core/src/ingest/mod.rs` 의 `is_noise_session()` 룰 확장

기존:
```rust
if first_user.content.trim_start().starts_with(SECALL_SUMMARY_PROMPT_PREFIX) {
    return Some("secall summary prompt");
}
```

추가:
```rust
if first_user.content.contains(crate::wiki::WIKI_INVOCATION_MARKER) {
    return Some("secall wiki invocation");
}
```

`contains` 로 검사 (codex/claude 가 system prompt 를 앞에 prepend 하는 경우에도 robust).

### 4. 사후 정리 (기존 세션)

P83 1차 PR 에서는 **신규 ingest 만 처리** — 무한 루프 차단이 시급. 기존 wiki invocation 세션 정리는 별도 fast-follow PR 또는 사용자 가이드 (issue 응답에 명시) 로 분리.

### 5. 신규 테스트

`ingest/mod.rs::tests` 에 3건 추가:
- `is_noise_wiki_invocation_marker_at_start` — marker 가 prompt 맨 앞
- `is_noise_wiki_invocation_marker_in_middle` — marker 가 중간에 있어도 검출
- `is_not_noise_without_wiki_marker` — marker 없으면 통과

## 변경 파일

| 파일 | 변경 |
|---|---|
| `crates/secall-core/src/wiki/mod.rs` | `WIKI_INVOCATION_MARKER` 상수 추가 |
| `crates/secall-core/src/wiki/codex.rs` | `generate()` 가 marker prefix |
| `crates/secall-core/src/wiki/claude.rs` | 동일 |
| `crates/secall-core/src/ingest/mod.rs` | `is_noise_session()` 에 marker 룰 추가 + 신규 unit test 3건 |
| `docs/plans/index.md` | P83 등록 |

## 검증

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p secall-core --lib ingest::codex
cargo test -p secall-core --lib ingest::claude
cargo test -p secall-core --lib wiki
cargo test --workspace --no-fail-fast
```

## 리스크

- 사후 정리 (기존 wiki invocation 세션) 는 본 PR 범위 외 — 사용자가 직접 `secall archive` 또는 `secall list --recent` 로 처리 가능 (issue #82 응답에 명시).
- marker 가 prompt 첫 부분에 노출 — codex/claude 는 marker 를 일반 HTML 주석으로 처리하므로 출력에 영향 없음 (검증 필요).

## 후속 (별도 PR)

- 기존 wiki invocation 세션 일괄 archive lint 옵션 또는 sync --force 자동 처리.
- core-backlog 에 fast-follow 항목 등록 (필요 시).
