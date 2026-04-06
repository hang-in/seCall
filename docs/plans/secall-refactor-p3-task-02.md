---
type: task
status: draft
plan: secall-refactor-p3
task_number: 2
title: "async Mutex → spawn_blocking"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: async Mutex → spawn_blocking

## 문제

`OrtEmbedder`가 `std::sync::Mutex<ort::session::Session>`을 async 함수(`embed()`, `embed_batch()`) 내에서 직접 lock하여 tokio 워커 스레드를 블로킹한다. MCP HTTP 서버에서 동시 요청 시 스레드 풀 고갈 위험이 있다.

### 현재 코드

```rust
// embedding.rs:110
session: Mutex<ort::session::Session>,

// embedding.rs:212-216 — async fn embed()
async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    let mut session = self.session.lock()
        .map_err(|_| anyhow!("ort session lock poisoned"))?;
    // ... ONNX 추론 (CPU-bound) ...
}

// embedding.rs:220-224 — async fn embed_batch()
async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    let mut session = self.session.lock()
        .map_err(|_| anyhow!("ort session lock poisoned"))?;
    // ... ONNX 추론 (CPU-bound) ...
}
```

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/search/embedding.rs:110` | 수정 | `Mutex<Session>` → `Arc<Mutex<Session>>` |
| `crates/secall-core/src/search/embedding.rs:212-230` | 수정 | `embed()`, `embed_batch()`를 `spawn_blocking` 래핑 |

## Change description

### Step 1: OrtEmbedder 필드를 Arc로 래핑

```rust
// embedding.rs:110 — 변경 전
session: Mutex<ort::session::Session>,

// 변경 후
session: Arc<Mutex<ort::session::Session>>,
```

`new()` 생성자에서도 `Arc::new(Mutex::new(session))`으로 변경.

### Step 2: embed()를 spawn_blocking으로 래핑

```rust
// embedding.rs:212 — 변경 전
async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    let mut session = self.session.lock()
        .map_err(|_| anyhow!("ort session lock poisoned"))?;
    // ... 동기 ONNX 추론 ...
}

// 변경 후
async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    let session = Arc::clone(&self.session);
    let text = text.to_string();
    tokio::task::spawn_blocking(move || {
        let mut session = session.lock()
            .map_err(|_| anyhow!("ort session lock poisoned"))?;
        // ... 동기 ONNX 추론 (기존 로직 그대로) ...
    })
    .await
    .map_err(|e| anyhow!("spawn_blocking join error: {e}"))?
}
```

### Step 3: embed_batch()도 동일 패턴 적용

```rust
// embedding.rs:220 — 변경 후
async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    let session = Arc::clone(&self.session);
    let texts: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
    tokio::task::spawn_blocking(move || {
        let mut session = session.lock()
            .map_err(|_| anyhow!("ort session lock poisoned"))?;
        // ... 동기 ONNX 추론 (기존 로직 그대로) ...
    })
    .await
    .map_err(|e| anyhow!("spawn_blocking join error: {e}"))?
}
```

### Step 4: Send 요구사항 확인

`spawn_blocking` 클로저는 `Send + 'static`을 요구한다. `Arc<Mutex<Session>>`은 Send를 만족하므로 문제 없음. `ort::session::Session`이 Send인지 확인 — `OrtEmbedder`에 이미 `unsafe impl Send`가 있으므로 (`embedding.rs:113`) 기존 패턴 유지.

## Dependencies

- 없음 (독립 실행 가능)
- `tokio`는 이미 의존성에 포함

## Verification

```bash
# 1. 컴파일 확인
cargo check -p secall-core

# 2. embedding 관련 테스트 통과
cargo test -p secall-core embedding

# 3. 전체 테스트 회귀 없음
cargo test --all

# 4. clippy 통과 (새 코드에 경고 없음)
cargo clippy -p secall-core -- -D warnings
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **`ort::session::Session`의 Send 안전성**: 현재 `unsafe impl Send for OrtEmbedder`가 존재(`embedding.rs:113`). ONNX Runtime C API는 thread-safe로 문서화되어 있으나, `spawn_blocking`으로 이동 시 다른 스레드에서 실행될 수 있으므로 기존 Send 보장이 더 중요해진다.
- **String 복사 오버헤드**: `text.to_string()` 및 `texts.iter().map(|t| t.to_string())` 복사가 발생한다. 임베딩 텍스트는 보통 수KB 이하이므로 무시할 수준.
- **JoinError**: `spawn_blocking`이 패닉하면 `JoinError` 반환. `map_err`로 anyhow 에러로 변환하여 처리.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/tokenizer.rs` — `KiwiTokenizer`의 Mutex는 동기 메서드에서만 사용되므로 이 task 범위 외
- `crates/secall-core/src/mcp/server.rs` — DB Mutex는 embed 후 lock하는 패턴이므로 이미 안전
- `crates/secall-core/src/search/vector.rs` — VectorIndexer 호출자 변경 없음 (embed 내부만 변경)
