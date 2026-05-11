---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P47
task_id: 01
parallel_group: 1
depends_on: []
---

# P47 Task 01 — ORT CoreML EP 옵션 추가

## Changed files

### Cargo 의존성

- `Cargo.toml:40` — workspace `ort = { version = "=2.0.0-rc.10", features = ["load-dynamic"] }` 그대로 두되, `coreml` feature 활성화는 secall-core 측에서.
- `crates/secall-core/Cargo.toml:23` — `ort.workspace = true` 옆에 `# CoreML EP 사용 시 features 확장` 주석.
- `crates/secall-core/Cargo.toml:52-55` — `[features]` 섹션에 다음 추가:
  ```toml
  coreml = ["ort/coreml"]
  ```

### ORT Session builder 에 CoreML EP 등록

- `crates/secall-core/src/search/embedding.rs:125-148` — `with_pool_size` 내부 `ort::session::Session::builder()?` 체인에 `coreml` feature 분기 추가. 구체적으로:
  - 첫 세션 생성부 (`embedding.rs:133`) 와 추가 세션 루프 (`embedding.rs:144`) 양쪽에 동일하게:
    ```text
    let mut builder = ort::session::Session::builder()?;
    #[cfg(all(feature = "coreml", target_os = "macos", target_arch = "aarch64"))]
    {
        use ort::execution_providers::CoreMLExecutionProvider;
        builder = builder
            .with_execution_providers([CoreMLExecutionProvider::default().build()])?;
    }
    let session = builder
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .commit_from_file(model_dir.join("model.onnx"))?;
    ```
  - CoreML EP 가 등록 실패해도 ORT 가 자동으로 다음 등록된 provider (= CPU) 로 폴백하므로 `error_on_failure` 는 호출하지 않음.

### tracing 로그 보강

- `crates/secall-core/src/search/embedding.rs:150` — 기존 `tracing::info!(pool_size, dim, "ORT session pool created")` 호출에 `coreml = cfg!(all(feature = "coreml", target_os = "macos", target_arch = "aarch64"))` 필드 추가하여 EP 활성 여부 가시화.

### CoreML EP API 시그니처 검증 (Task 시작 시)

- 작업 시작 직후 `cargo doc --open -p ort` 또는 `cargo tree | grep ort` 로 `ort = 2.0.0-rc.10` 에서 `CoreMLExecutionProvider` 가 노출되는 정확한 경로 확인. 경로가 다르면 (`ort::execution_providers::coreml::CoreMLExecutionProvider` 등) 본 task 안에서 import 수정 후 진행.

## Change description

### 단계별 접근

1. **CoreML EP API 경로 확인** — `cargo add --dry-run ort --features coreml` 또는 ort docs.rs 페이지에서 `CoreMLExecutionProvider` 시그니처 확인. `default()` / `build()` 또는 `new()` 호출 패턴 결정.

2. **Cargo feature 추가** — `crates/secall-core/Cargo.toml` `[features]` 에 `coreml = ["ort/coreml"]` 한 줄 추가. default 에는 포함하지 않음 (수동 활성화).

3. **embedding.rs 의 두 세션 빌더 분기 추가** — `#[cfg(all(feature = "coreml", target_os = "macos", target_arch = "aarch64"))]` 가드로 CoreML EP 등록. 비활성 시 기존 코드 그대로.

4. **로그 추가** — pool 생성 시 EP 활성화 여부 한 줄 로깅. 사용자가 `secall sync` 실행 시 stderr 로 즉시 확인 가능.

5. **빌드 검증** — `cargo build -p secall-core --features coreml` (macOS aarch64) / `cargo build -p secall-core` (default) 둘 다 성공해야 함.

### 구현 제약

- **CoreML EP 등록 실패 = 자동 CPU 폴백** — `error_on_failure` 호출 X. ORT 는 등록된 EP 순서대로 시도하고 실패 시 다음 EP 로 넘어가는 것이 기본. 우리는 CoreML 만 명시 등록하고 CPU 는 ORT 기본값이므로 등록 실패 시 자연스럽게 CPU 사용.
- **feature 가드 강제** — `#[cfg(feature = "coreml")]` 만 두면 Linux/Windows 빌드에서 ort/coreml 의존이 시도될 수 있음. 반드시 `target_os = "macos"` 와 `target_arch = "aarch64"` 까지 명시.
- **pool 의 모든 세션에 동일 EP** — 첫 세션과 추가 세션 양쪽 분기 누락 시 일부만 ANE/GPU 사용 → 디버깅 어려움. helper 함수로 추출해도 OK.

## Dependencies

- 외부: `ort = "=2.0.0-rc.10"` 의 `coreml` feature 가용성 (확인 필요)
- 다른 task: 없음 (parallel_group 1)

## Verification

```bash
# 1. Default build 회귀 확인 (CoreML feature 없이)
cargo check -p secall-core
cargo check -p secall

# 2. CoreML feature 활성 빌드 (macOS aarch64 한정)
cargo check -p secall-core --features coreml

# 3. 기존 ORT 단위 테스트 회귀 없음
cargo test -p secall-core --lib search::embedding

# 4. Manual: M4 Air 에서 빌드 후 실제 동작 확인
# cargo build --release -p secall --features secall-core/coreml
# OR (개발 빌드)
# cargo build -p secall --features secall-core/coreml
# Manual: secall sync 실행 시 stderr 에 "ORT session pool created" 로그의 coreml=true 필드 확인
# Manual: Activity Monitor 에서 ANE / GPU 사용량 증가 확인 (CPU 100% → 분산)
```

## Risks

- **`ort = 2.0.0-rc.10` 의 CoreML EP API 가 stable 화 전이라 변경 가능성** — 빌드 실패 시 ort docs.rs 의 해당 버전 페이지 참조. 최악의 경우 ort 버전 업 또는 feature 임시 비활성으로 폴백 후 Task 02 로 진행.
- **CoreML EP 활성 시 정확도 미세 변동 가능** — ANE 의 fp16 추론이 CPU fp32 와 끝자리 다를 수 있음. bge-m3 벡터 검색 품질은 코사인 유사도 기준이라 무시 가능 수준이지만, 기존 인덱스와의 벡터 호환성은 1차 manual smoke 로 확인.
- **빌드 시간 증가** — ort 의 CoreML feature 가 추가 의존성을 가져올 수 있음. CI 영향 검토 필요 (현재 CI 는 default feature 만 빌드하므로 영향 없음 — coreml 빌드는 로컬 macOS 에서만).
- **code-review-graph 영향** — `OrtEmbedder::with_pool_size` 가 호출자 (`create_vector_indexer`, `try_ort_cpu_fallback`) 의 동작에 직접 영향. 단순 EP 추가라 회귀 위험 낮음.

## Scope boundary (수정 금지)

- `crates/secall-core/src/search/embedding.rs` 의 `OllamaEmbedder` / `OpenAIEmbedder` / `OpenVinoEmbedder` 영역 — Task 03 영역.
- `crates/secall-core/src/search/vector.rs::create_vector_indexer` — pool_size 인자 변경은 Task 02 영역.
- `crates/secall/src/commands/ingest.rs` — Task 04 영역.
- `docs/`, `web/` — Task 05 영역.
- `OrtEmbedder::with_pool_size` 의 시그니처 / pool_size 기본값 — Task 02 영역.
