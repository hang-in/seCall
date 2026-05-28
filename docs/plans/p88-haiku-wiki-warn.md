---
type: plan
status: in_progress
updated_at: 2026-05-29
canonical: true
---

# P88 — claude+haiku wiki generation 경고 (issue #93)

## 배경

Issue #93 (cakel, v0.6.0): `[wiki.backends.claude] model = "haiku"` 설정 시 `secall wiki update` 가 작업을 안 하고 "뭘 원하나요?" 라고 되물은 뒤 `✓ Wiki update complete` 로 빈 결과 종료. `model = "sonnet"` 은 정상.

## 원인 (코드 버그 아님 — 모델 capability)

`wiki update` batch/incremental prompt 는 MCP 도구 (`secall recall`/`get`/`status`) 능동 호출 + `wiki/` 파일 생성을 요구하는 복잡한 instruction. claude CLI 자체는 haiku/sonnet 모두 도구 호출 가능하나, **haiku 는 instruction-following 이 약해** 이 prompt 를 받으면 작업을 시작하지 않고 되묻는 경우가 잦다. #88(ollama, 도구 호출 자체 불가)과 달리 haiku 는 가능은 하므로 **완전 차단(fail-fast)은 부적절** — review backend 로는 정상 사용되기 때문.

## 목표

- generation 경로에서 claude+haiku 조합 감지 시 **경고 출력** (sonnet/opus 권장 + haiku 는 review 용 안내).
- silent 빈 결과로 끝나 사용자가 원인 모르는 혼란 제거.

## 비목표

- haiku 차단 안 함 (review backend 로 적합, generation 도 가능은 함).
- prompt 강화로 haiku 가 따르게 만드는 작업은 별도 (효과 불확실).

## 구현

`crates/secall/src/commands/wiki.rs` 의 `run_update_with_sink` — P86 fail-fast 블록 다음:

```rust
if backend_name == "claude" && resolved_model.trim().eq_ignore_ascii_case("haiku") {
    eprintln!("⚠️  wiki generation 에 haiku 모델은 권장되지 않습니다 ...");
}
```

generation 경로(`run_update_with_sink`)에만 위치 → review backend (별도 `build_reviewer`) 는 영향 없음.

## 변경 파일

- `crates/secall/src/commands/wiki.rs` — 경고 추가
- `docs/plans/p88-haiku-wiki-warn.md` (신규) + `docs/plans/index.md`

## 검증

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
# 수동: [wiki.backends.claude] model="haiku" 로 secall wiki update → 경고 출력 확인
```

## 리스크

- eprintln 경고만 추가 — 동작 변경 없음, 회귀 위험 최소.
- `model = "sonnet "` (trailing space) 같은 입력은 `claude.rs` 가 `_ => sonnet` 으로 처리하므로 경고 대상 아님 (`trim().eq_ignore_ascii_case("haiku")` 가 정확히 haiku 만 매칭).
