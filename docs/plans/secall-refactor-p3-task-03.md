---
type: task
status: draft
plan: secall-refactor-p3
task_number: 3
title: "입력 검증 강화"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 03: 입력 검증 강화

## 문제

4가지 입력 검증 공백이 존재한다:

1. **`--since` silent failure**: `hybrid.rs:229`에서 인식 불가 temporal filter가 `None` 반환 → 필터 무시. 사용자 오입력 시 전체 결과가 반환되어 혼란.
2. **세션 ID substring 매칭**: `ingest.rs:199`에서 `fname.contains(id)`로 세션을 찾아 오탐 위험. 예: `abc`로 검색 시 `abc123.jsonl`과 `xyzabc.jsonl` 모두 매칭.
3. **project명 path traversal**: `markdown.rs:173`에서 project명을 파일명에 그대로 사용. `../secret` 같은 project명이 경로 탈출 가능.
4. **토큰 "0k" 표시**: `output.rs:47-48`에서 `/ 1000` 정수 나눗셈으로 999 이하 토큰이 0k로 표시.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/hybrid.rs:229` | 수정 | `_ => None` → 경고 로그 + None |
| `crates/secall/src/commands/ingest.rs:199` | 수정 | `fname.contains(id)` → stem 정확 매칭 또는 starts_with |
| `crates/secall-core/src/ingest/markdown.rs:173` | 수정 | project명 sanitize (경로 구분자·특수문자 제거) |
| `crates/secall/src/output.rs:47-48` | 수정 | 토큰 표시 형식 개선 |

## Change description

### Part A: temporal filter 경고 (hybrid.rs:229)

```rust
// hybrid.rs:229 — 변경 전
_ => None,

// 변경 후
other => {
    tracing::warn!(input = other, "unrecognized temporal filter, ignoring --since value");
    None
}
```

> 동작은 동일 (None 반환)하나, 사용자에게 경고를 출력하여 오입력을 인지할 수 있게 한다.

### Part B: 세션 ID 매칭 개선 (ingest.rs:199)

```rust
// ingest.rs:199 — 변경 전
if fname.contains(id) {

// 변경 후
let stem = p.file_stem().unwrap_or_default().to_string_lossy();
if stem == id || stem.starts_with(&format!("{id}_")) || stem.starts_with(&format!("{id}-")) {
```

> 정확 매칭 + 구분자(`_`, `-`) 기반 prefix 매칭으로 오탐 방지. UUID prefix (`abc12345`)로 검색하는 기존 UX도 유지.

### Part C: project명 sanitize (markdown.rs:173)

```rust
// markdown.rs:173 — 변경 전
let project = session.project.as_deref().unwrap_or("unknown");

// 변경 후
let raw_project = session.project.as_deref().unwrap_or("unknown");
let project: String = raw_project
    .chars()
    .map(|c| if c == '/' || c == '\\' || c == '\0' { '_' } else { c })
    .collect();
let project = if project.starts_with('.') {
    format!("_{project}")
} else {
    project
};
```

> 경로 구분자(`/`, `\`)를 `_`로 치환, `.`으로 시작하는 이름(`.git`, `..`)에 `_` prefix 추가.

### Part D: 토큰 표시 개선 (output.rs:47-48)

```rust
// output.rs:46-48 — 변경 전
println!("  Tokens:  {}k in, {}k out",
    session.total_tokens.input / 1000,
    session.total_tokens.output / 1000
);

// 변경 후
println!("  Tokens:  {} in, {} out",
    format_token_count(session.total_tokens.input),
    format_token_count(session.total_tokens.output),
);

// output.rs — 헬퍼 함수 추가
fn format_token_count(n: u64) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}
```

> 1000 미만: 실수 표시 (예: `423`). 1000 이상: `1.2k` 형식. "0k" 문제 해소.

## Dependencies

- 없음 (독립 실행 가능)

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 기존 테스트 회귀 없음
cargo test --all

# 3. temporal filter 경고 확인 (잘못된 --since 값)
RUST_LOG=warn cargo run -p secall -- recall "test" --since "invalid_value" 2>&1 | grep -i "unrecognized"

# 4. 토큰 표시 확인 (수동)
# Manual: `cargo run -p secall -- get <session-id>` 실행 후 Tokens 행에서 0k가 아닌 실제 수치 확인
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **세션 ID 매칭 변경**: 기존에 substring으로 찾던 사용자가 있다면 더 이상 매칭되지 않을 수 있다. UUID prefix는 `starts_with`로 계속 지원되므로 실사용 영향은 없을 것으로 판단.
- **project명 sanitize 부작용**: 기존에 `/`가 포함된 project명으로 생성된 vault 파일이 있다면 새 파일명과 불일치할 수 있다. 실제로 이런 project명이 존재할 가능성은 극히 낮음.
- **format_token_count 정밀도**: `f64` 변환 시 u64 큰 수에서 정밀도 손실 가능. 실용적으로 토큰 수가 2^53을 넘지 않으므로 무시 가능.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/embedding.rs` — Task 02 영역
- `crates/secall-core/src/search/query_expand.rs` — Task 04 영역
- `.github/workflows/` — Task 01 영역
- `crates/secall/src/commands/wiki.rs` — wiki 프롬프트 인젝션은 이 task 범위 외 (환경변수 기반이라 공격 벡터 낮음)
