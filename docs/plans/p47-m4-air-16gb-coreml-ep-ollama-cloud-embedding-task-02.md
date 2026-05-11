---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P47
task_id: 02
parallel_group: 1
depends_on: []
---

# P47 Task 02 — OrtEmbedder pool_size 조정 + config 노출

## Changed files

### Config schema

- `crates/secall-core/src/vault/config.rs:68-91` — `EmbeddingConfig` 에 다음 필드 추가:
  ```text
  /// ORT session pool size. None 이면 시스템 메모리 기반 자동 결정.
  /// 16GB 미만 → 1, 16~32GB → 2, 그 이상 → 4
  pub pool_size: Option<usize>,
  ```
- `crates/secall-core/src/vault/config.rs:368-390` — `apply_env_overrides` 에서 `SECALL_EMBED_POOL_SIZE` env 처리 (선택). 우선순위는 CLI 플래그 없으므로 env > config 만 다루면 됨. 미설정 시 자동 휴리스틱.

### OrtEmbedder 호출부

- `crates/secall-core/src/search/vector.rs:409` — `OrtEmbedder::new(&model_dir)` 호출을 `OrtEmbedder::with_pool_size(&model_dir, resolve_pool_size(config))` 로 교체.
- `crates/secall-core/src/search/vector.rs:482` — `try_ort_cpu_fallback` 안의 동일 호출도 교체.
- `crates/secall-core/src/search/vector.rs` 어딘가 (예: `default_model_path` 함수 근처) — `fn resolve_pool_size(config: &Config) -> usize` 헬퍼 추가:
  - `config.embedding.pool_size` 가 Some 이면 그 값 사용 (최소 1 강제).
  - None 이면 `sysinfo::System::new_all().total_memory()` 또는 단순히 `sysinfo` 의존 없이 `mach::vm_statistics::vm_statistics64` (mac) / `/proc/meminfo` (linux) 로 RAM 조회.
  - 의존 추가가 부담스러우면 환경변수 `SECALL_TOTAL_MEMORY_GB` (manual override) + 기본값 16GB 로 시작 후 점진 개선.
  - **권장: `sysinfo` 크레이트 추가** (workspace 다른 크레이트 의존 부담 없는 한). 시그니처:
    ```text
    let total_gb = sysinfo::System::new_all().total_memory() / (1024 * 1024 * 1024);
    let auto = match total_gb {
        0..=15 => 1,
        16..=31 => 2,
        _ => 4,
    };
    ```

### OrtEmbedder::new 기본값

- `crates/secall-core/src/search/embedding.rs:120-123` — `OrtEmbedder::new` 의 기본 pool_size 를 `4` 에서 `Self::with_pool_size(model_dir, 1)` 로 변경 (안전한 기본). 단, 실제 호출은 `vector.rs` 에서 `with_pool_size(..., resolve_pool_size(config))` 로 하므로 `new()` 는 호환용으로만 유지.

### 단위 테스트

- `crates/secall-core/src/search/vector.rs` `#[cfg(test)]` — `resolve_pool_size` 가:
  - config 값이 있으면 그대로 반환 (`Some(3)` → `3`),
  - `Some(0)` 면 1 로 강제,
  - `None` 이면 자동 휴리스틱 결과를 반환 (테스트는 결과가 1 이상이면 통과).
- `crates/secall-core/src/vault/config.rs` `#[cfg(test)]` — `[embedding]` 섹션에 `pool_size = 2` 가 있을 때 round-trip 성공.

## Change description

### 단계별 접근

1. **Config schema 확장 먼저** — `EmbeddingConfig` 에 `pool_size: Option<usize>` 필드 추가. `#[serde(default)]` 가 이미 EmbeddingConfig 에 적용돼 있으므로 backwards-compatible.

2. **휴리스틱 헬퍼 추가** — `vector.rs::resolve_pool_size(config)` 함수. config 우선, 없으면 RAM 기반 자동 결정.

3. **호출부 두 곳 (`vector.rs:409, 482`) 갱신** — `OrtEmbedder::new` → `OrtEmbedder::with_pool_size(.., resolve_pool_size(config))`.

4. **로그 보강** — pool 생성 시 RAM 기반 자동 결정인지 config 명시인지 표시 (Task 01 의 `tracing::info!` 와 충돌 없도록 `vector.rs` 측에서 한 줄 더).

5. **단위 테스트** — config round-trip + resolve_pool_size 휴리스틱 검증.

### 구현 제약

- **sysinfo 의존 추가는 가벼움** — `sysinfo = "0.34"` 정도면 충분. workspace `Cargo.toml` 에 추가. 추가가 부담스러우면 `SECALL_TOTAL_MEMORY_GB` env 만으로도 1차 출시 가능.
- **기본값 1 = 보수적** — 자동 휴리스틱이 RAM 정보를 못 가져오면 fallback 도 1. 4 보다 1 이 항상 안전 (단일 세션 + 직렬 추론).
- **호환성** — `OrtEmbedder::new()` 는 deprecated 표시 없이 그대로 두되 내부 default 만 1 로 변경. 외부 호출자 영향 없음.

## Dependencies

- 외부: `sysinfo` 크레이트 추가 (권장) 또는 env override 로 대체.
- 다른 task: 없음 (Task 01 과 parallel group 1)

## Verification

```bash
# 타입체크
cargo check -p secall-core
cargo check -p secall

# 단위 테스트
cargo test -p secall-core --lib search::vector::tests
cargo test -p secall-core --lib vault::config::tests

# Manual: 16GB 시스템에서 default 동작 확인
# cargo run -p secall -- sync (또는 ingest)
# Manual: stderr 의 "ORT session pool created" 로그에서 pool_size=1 표시 확인
#
# Manual: config 로 명시 override
# echo '[embedding]' >> ~/.config/secall/config.toml
# echo 'pool_size = 2' >> ~/.config/secall/config.toml
# cargo run -p secall -- sync
# Manual: pool_size=2 로 로깅되는지 확인
```

## Risks

- **휴리스틱 부정확성** — `sysinfo::total_memory` 는 시스템 total 을 보지만 available 메모리는 다름. 다른 앱 메모리 점유가 큰 환경에선 여전히 부족할 수 있음. 사용자가 `pool_size = 1` 명시 override 가능하므로 1차로 충분.
- **sysinfo 의존 부담** — 컴파일 시간 1~2초 증가. Cargo.lock 갱신 필요.
- **테스트 환경 비결정성** — `resolve_pool_size` 의 None 분기 테스트는 RAM 에 따라 결과가 달라짐. 테스트는 "결과가 1 이상 4 이하" 만 검증.
- code-review-graph 영향: `OrtEmbedder::new` 시그니처 그대로라 호출자 영향 없음. `create_vector_indexer` 만 인자 추가 — 직접 호출자는 `ingest.rs`, `search` 명령 등 소수.

## Scope boundary (수정 금지)

- `crates/secall-core/src/search/embedding.rs:125-148` 의 `Session::builder` EP 등록 — Task 01 영역.
- `crates/secall-core/src/search/embedding.rs` 의 `OllamaEmbedder` 영역 — Task 03 영역.
- `crates/secall/src/commands/ingest.rs` — Task 04 영역.
- `docs/`, `web/` — Task 05 영역.
