---
type: plan
status: in_progress
updated_at: 2026-05-19
canonical: true
---

# P86 — Wiki update + ollama/lmstudio 백엔드 fail-fast

## 배경

Issue #88 (cakel, 2026-05-19) 보고:

> ollama (gemma4:26b) 에서 wiki update 시, 준비만하고 실제 업데이트가 안됨
> ... 알겠습니다. 저는 지금부터 seCall 위키 관리 에이전트로서 임무를 수행합니다.
> ... 작업을 시작할 준비가 되었습니다.

진단 결과:
- `docs/prompts/wiki-update.md` (batch) + `wiki-incremental.md` (incremental) 둘 다 **MCP 도구 호출** (`secall recall`, `secall get`, `secall status`) + **`wiki/` 디렉토리 파일 쓰기** 를 가정.
- Claude Code CLI = MCP 통합 / Codex CLI = 도구 호출 가능 / Haiku = 세션 데이터를 prompt 에 직접 inline (도구 불필요)
- **ollama / lmstudio = HTTP API 단순 text generation, 도구 호출 능력 없음**
- → ollama/lmstudio 백엔드는 prompt 받아서 "임무 이해, 시작 준비됨" 답하고 종료. 실제 작업 수행 안 됨

기존 코드는 silent 하게 1800s timeout 까지 또는 모델이 stop token 보낼 때까지 wait → 사용자가 빈손으로 깨닫는 사고.

## 목표

- ollama/lmstudio 백엔드 + `wiki update` 모든 모드 조합 사용 시 **즉시 명시적 에러** + 사용 가능한 백엔드 가이드.
- 사용자가 30분 silent wait 후 빈손으로 깨닫는 일 차단.

## 비목표

- ollama/lmstudio 의 wiki update 지원 자체 추가는 별도 PR. 향후 haiku 같은 데이터-inline batch 방식으로 확장 가능 (큰 작업).
- 백엔드의 다른 사용 (graph, log 등) 동작 변경 없음.

## 구현

### `crates/secall/src/commands/wiki.rs`

backend selection (line 188~191) 다음, prompt build 직전에 가드:

```rust
if matches!(backend_name.as_str(), "ollama" | "lmstudio") {
    anyhow::bail!(
        "wiki backend `{backend_name}` 는 wiki update 와 호환되지 않습니다.\n\
         도구 호출 (`secall recall`/`get`/`status` + `wiki/` 파일 쓰기) 능력이 없어\n\
         prompt 가 가정하는 작업을 수행할 수 없습니다.\n\
         \n\
         해결책:\n\
           • `--backend claude` ...\n\
           • `--backend codex` ...\n\
           • `--backend haiku` ...\n\
         \n\
         참고: issue #88, docs/plans/p86-ollama-batch-fail-fast.md"
    );
}
```

## 변경 파일

| 파일 | 변경 |
|---|---|
| `crates/secall/src/commands/wiki.rs` | backend gate + 에러 메시지 |
| `docs/plans/p86-ollama-batch-fail-fast.md` (신규) | 본 plan |
| `docs/plans/index.md` | P86 등록 |

## 검증

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
# 수동 검증: secall wiki update --backend ollama → 즉시 명시적 에러 출력
```

테스트 추가 옵션:
- `commands::wiki` 가 binary crate 라 unit test 작성이 까다로움.
- integration test 는 SQL/vault 등 fixture 가 많아 비용 큼.
- 본 PR 은 가드 메시지가 정확한 contract — 코드 변경 작아 manual smoke 로 충분.

## 리스크

- false positive 없음 — ollama/lmstudio 가 wiki update 에 동작했던 적 없음.
- 향후 ollama 데이터-inline 지원 추가 시 이 가드 수정 필요. plan 에 명시.
