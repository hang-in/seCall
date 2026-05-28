---
type: plan
status: in_progress
updated_at: 2026-05-29
canonical: true
---

# P87 — Windows `.cmd` 래퍼 CLI spawn 실패 fix (issue #92)

## 배경

Issue #92 (cakel, Windows 10): `secall wiki update --backend codex` 실행 시:
```
Wiki update: all sessions (backend: codex)
  Launching codex...
Error: program not found
```
`where codex` 는 `codex.cmd` + `codex` 를 찾고 `codex` 명령 자체도 동작하는데 secall 만 실패.

## 원인

- codex 는 npm 으로 설치되어 `C:\Users\User\AppData\Roaming\npm\codex.cmd` (배치 래퍼) 형태다.
- `std::process::Command::new("codex")` 는 Windows 에서 **PATHEXT 를 적용하지 않아** `codex.exe` 만 시도하고 `codex.cmd` 는 못 찾아 "program not found".
- 반면 `command_exists` 는 `where.exe codex` 외부 호출로 `.cmd` 를 찾으므로 **통과** → "존재하는데 spawn 실패" 불일치.

## 목표

- Windows 에서 npm `.cmd` 래퍼 CLI (codex / claude) 를 정상 spawn.
- macOS / Linux 회귀 없음.
- "존재 확인" 과 "실제 spawn" 의 탐색 규칙 일치.

## 비목표

- codex/claude 외 다른 외부 명령 (ollama, git) 의 동작 변경 없음 (영향 받지만 회귀 없음).

## 구현

### 1. `which` crate 도입

`Cargo.toml` workspace dep `which = "8"` + `secall-core/Cargo.toml` `which.workspace = true`.

`which` 는 Windows 에서 PATHEXT (`.CMD`/`.EXE`/`.BAT`) 를 적용해 실제 경로를 탐색한다.

### 2. `lib.rs` — `resolve_program` + `command_exists` 재구현

```rust
pub fn resolve_program(cmd: &str) -> std::path::PathBuf {
    which::which(cmd).unwrap_or_else(|_| std::path::PathBuf::from(cmd))
}

pub fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}
```

`resolve_program` 이 `codex.cmd` 풀경로를 반환 → Rust 1.77+ 가 `.cmd`/`.bat` 확장자 경로를 `Command` 로 실행 시 cmd.exe 경유로 안전 실행 (인자 escaping 포함).

### 3. spawn 사이트 5곳 — `Command::new(resolve_program(X))`

| 파일 | 명령 |
|---|---|
| `wiki/codex.rs:30` | codex |
| `wiki/claude.rs:39` | claude |
| `wiki/reviewers/codex.rs:32` | codex |
| `wiki/reviewers/claude.rs` (`run_review_cli`) | bin (claude/codex) |
| `search/query_expand.rs:31` | claude |

## 변경 파일

- `Cargo.toml`, `crates/secall-core/Cargo.toml` — which dep
- `crates/secall-core/src/lib.rs` — resolve_program + command_exists
- `crates/secall-core/src/wiki/{codex,claude}.rs`
- `crates/secall-core/src/wiki/reviewers/{codex,claude}.rs`
- `crates/secall-core/src/search/query_expand.rs`
- `docs/plans/p87-windows-cmd-spawn.md` (신규) + `docs/plans/index.md`

## 검증

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p secall-core --lib   # command_exists 회귀 (3건)
```

⚠️ **Windows 실제 spawn 검증 불가** (개발 환경 macOS). 머지 전 cakel (issue #92 보고자) 에게 Windows 빌드 검증 요청 필요. macOS/Linux 는 `which` 가 확장자 없는 실행파일 그대로 resolve → 회귀 없음 (위 테스트 통과 확인).

## 리스크

- `which` resolve 실패 시 입력 문자열 그대로 fallback → 기존 동작 유지 (최소 회귀).
- Rust 1.77+ 의 `.cmd` 자동 cmd.exe 경유 실행에 의존 — secall MSRV 1.75 보다 높음. 단 실제 빌드/CI 는 최신 toolchain (1.94) 사용. MSRV 표기는 별도 검토 (본 PR 범위 외 — 코드는 1.75 에서도 컴파일됨, .cmd 실행 동작만 1.77+).
