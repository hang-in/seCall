# Implementation Result: P43 — Wiki review backend 확장 + 로컬 전용 wiki 파이프라인 완성

> Developer: codex
> Branch: N/A
> Date: 2026-05-11 05:44
> Plan Revision: 0

---

## Summary

Developer Codex · Implementer입니다. rework 대상은 Task 03과 Task 05뿐이므로 해당 작업지시서와 `wiki.rs`, `config.rs`의 지정 구간만 먼저 대조해 허용된 변경 범위 안에서 수정하겠습니다.작업지시서의 허용 변경 범위와 현재 구현 차이를 맞추는 중입니다. `resolve_review_model()` 기본값 처리와 `Config::save()`의 section merge 로직이 이번 rework 핵심이라 그 두 지점만 좁혀서 읽겠습니다.허용 범위는 맞았습니다. 이제 `haiku` 기본 모델 해석과 `sync_section`의 삭제/중첩 테이블 보존을 같이 고쳐도 Task 03·05 범위 안에 들어가므로 그 두 파일만 패치하겠습니다.`wiki.rs`에서는 review backend별 기본 모델 해석과 haiku alias 검증을 넣고, `config.rs`에서는 키 삭제와 재귀 table merge로 바꾸겠습니다. 테스트도 같은 두 파일 범위 안에서 같이 보강합니다.패치는 들어갔습니다. 지금은 컴파일 오류나 범위 이탈이 없는지, 그리고 새 테스트가 실제로 rework 내용을 잡는지 확인 중입니다.이제 문서에 적힌 검증만 순서대로 실행합니다. 먼저 컴파일·clippy·타깃 테스트를 끝내고, 그 다음 CLI/help와 config save round-trip 확인으로 넘어가겠습니다.Developer Codex · Implementer입니다.

변경은 rework 범위 안에서 3개 파일만 건드렸습니다. `crates/secall/src/commands/wiki.rs`에서는 `resolve_review_model()`이 review backend별 기본 모델을 해석하도록 바꿨고, `haiku` backend에 `sonnet`/`opus` 같은 legacy alias가 들어오면 명시적 에러를 내도록 막았습니다. `crates/secall-core/src/vault/config.rs`에서는 `Config::save()`의 section merge를 재귀 테이블 동기화로 바꿔서 `None`으로 비운 optional key가 실제 파일에서 제거되도록 했고, `[wiki.backends.*]` 같은 중첩 테이블이 inline table로 접히지 않게 유지했습니다. `crates/secall/tests/wiki_review_resolve.rs`에는 backend별 review model 기본값 회귀 테스트를 추가했습니다.

Verification results for Task 03:
✅ `cargo check -p secall-core` — exit 0  
✅ `cargo check -p secall` — exit 0  
✅ `cargo clippy --all-targets` — exit 0, 기존 `crates/secall-core/tests/rest_config.rs`의 `await_holding_lock` 경고 7건만 출력  
✅ `cargo test -p secall --test wiki_review_resolve` — 2 passed  
✅ `./target/debug/secall wiki update --help | grep -E "review-backend|review-model"` — 두 flag 모두 출력 확인  
✅ `./target/debug/secall config set wiki.review_backend ollama` — 설정 저장 성공  
✅ `./target/debug/secall config show | grep -A2 Wiki | grep review_backend` — `review_backend: ollama` 확인  
❌ `ollama serve` / `secall wiki update <session> --review --review-backend ollama` — task 문서가 background daemon 실행과 placeholder `<session>`를 요구해 현재 오케스트레이터 규칙상 그대로 실행할 수 없었습니다. 외부 Ollama 서버와 실제 session id가 준비된 환경에서만 수동 검증 가능합니다.

Verification results for Task 05:
✅ `cargo check -p secall-core` — exit 0  
✅ `cargo clippy --all-targets -p secall-core` — exit 0, 기존 `crates/secall-core/tests/rest_config.rs`의 `await_holding_lock` 경고 7건만 출력  
✅ `cargo test -p secall-core --lib vault::config::tests::save_` — 6 passed  
✅ `cargo test -p secall-core --test rest_config` — 7 passed  
✅ `echo '# user comment\n[vault]\npath = "/tmp/x"' > /tmp/config.toml` / `SECALL_CONFIG_PATH=... ./target/debug/secall config set log.backend haiku` / `cat /tmp/config.toml` — 상단 `# user comment` 유지된 채 저장 확인

## Subtask Results

### 1. Developer Codex · Implementer입니다. rework 대상은 Task 03과 Task 05뿐이므로 해당 작업지시서와 `wiki.rs`, `config.rs`의 지정 구간만 먼저 대조해 허용된 변경 범위 안에서 수정하겠습니다.작업지시서의 허용 변경 범위와 현재 구현 차이를 맞추는 중입니다. `resolve_review_model()` 기본값 처리와 `Config::save()`의 section merge 로직이 이번 rework 핵심이라 그 두 지점만 좁혀서 읽겠습니다.허용 범위는 맞았습니다. 이제 `haiku` 기본 모델 해석과 `sync_section`의 삭제/중첩 테이블 보존을 같이 고쳐도 Task 03·05 범위 안에 들어가므로 그 두 파일만 패치하겠습니다.`wiki.rs`에서는 review backend별 기본 모델 해석과 haiku alias 검증을 넣고, `config.rs`에서는 키 삭제와 재귀 table merge로 바꾸겠습니다. 테스트도 같은 두 파일 범위 안에서 같이 보강합니다.패치는 들어갔습니다. 지금은 컴파일 오류나 범위 이탈이 없는지, 그리고 새 테스트가 실제로 rework 내용을 잡는지 확인 중입니다.이제 문서에 적힌 검증만 순서대로 실행합니다. 먼저 컴파일·clippy·타깃 테스트를 끝내고, 그 다음 CLI/help와 config save round-trip 확인으로 넘어가겠습니다.Developer Codex · Implementer입니다.

변경은 rework 범위 안에서 3개 파일만 건드렸습니다. `crates/secall/src/commands/wiki.rs`에서는 `resolve_review_model()`이 review backend별 기본 모델을 해석하도록 바꿨고, `haiku` backend에 `sonnet`/`opus` 같은 legacy alias가 들어오면 명시적 에러를 내도록 막았습니다. `crates/secall-core/src/vault/config.rs`에서는 `Config::save()`의 section merge를 재귀 테이블 동기화로 바꿔서 `None`으로 비운 optional key가 실제 파일에서 제거되도록 했고, `[wiki.backends.*]` 같은 중첩 테이블이 inline table로 접히지 않게 유지했습니다. `crates/secall/tests/wiki_review_resolve.rs`에는 backend별 review model 기본값 회귀 테스트를 추가했습니다.

Verification results for Task 03:
✅ `cargo check -p secall-core` — exit 0  
✅ `cargo check -p secall` — exit 0  
✅ `cargo clippy --all-targets` — exit 0, 기존 `crates/secall-core/tests/rest_config.rs`의 `await_holding_lock` 경고 7건만 출력  
✅ `cargo test -p secall --test wiki_review_resolve` — 2 passed  
✅ `./target/debug/secall wiki update --help | grep -E "review-backend|review-model"` — 두 flag 모두 출력 확인  
✅ `./target/debug/secall config set wiki.review_backend ollama` — 설정 저장 성공  
✅ `./target/debug/secall config show | grep -A2 Wiki | grep review_backend` — `review_backend: ollama` 확인  
❌ `ollama serve` / `secall wiki update <session> --review --review-backend ollama` — task 문서가 background daemon 실행과 placeholder `<session>`를 요구해 현재 오케스트레이터 규칙상 그대로 실행할 수 없었습니다. 외부 Ollama

[…truncated, original 2739 chars]

