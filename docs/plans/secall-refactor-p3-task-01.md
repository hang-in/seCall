---
type: task
status: draft
plan: secall-refactor-p3
task_number: 1
title: "CI/CD GitHub Actions 구축"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: CI/CD GitHub Actions 구축

## 문제

`.github/workflows/` 디렉토리가 존재하지 않아 CI/CD가 전무하다. ~130개 단위 테스트와 4개 CLI 통합 테스트가 있으나 자동 실행 체계가 없어, 깨진 빌드·회귀 버그·코드 스타일 불일치가 검증 없이 main에 반영될 수 있다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `.github/workflows/ci.yml` | 신규 | CI 워크플로우 |

## Change description

### Step 1: `.github/workflows/ci.yml` 생성

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-Dwarnings"

jobs:
  check:
    name: Check & Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache cargo registry & build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: cargo fmt --check
        run: cargo fmt --all -- --check

      - name: cargo clippy
        run: cargo clippy --all-targets --all-features

      - name: cargo test
        run: cargo test --all

      - name: cargo audit
        run: |
          cargo install cargo-audit --locked || true
          cargo audit
        continue-on-error: true
```

### Step 2: clippy 경고 0건 달성

현재 `RUSTFLAGS="-Dwarnings"` 설정 시 clippy/빌드가 경고로 실패할 수 있다. 기존 경고 7건을 정리:

| 파일 | 경고 | 수정 |
|---|---|---|
| `hybrid.rs:5` | 미사용 `SessionMeta` import | import 제거 (테스트 모듈에 별도 import 있음) |
| `hybrid.rs:236-240` | 테스트 모듈 미사용 imports 5건 | 사용하지 않는 import 제거 |
| `detect.rs:181` | 미사용 `tempfile::Builder` | import 제거 |
| `bm25.rs:427` | 불필요한 `mut` | `let mut results` → `let results` |

> CI에서 `RUSTFLAGS="-Dwarnings"`를 설정하면 경고가 에러로 승격된다. 따라서 기존 경고를 모두 정리해야 CI가 통과한다.

## Dependencies

- 없음 (독립 실행 가능)

## Verification

```bash
# 1. 워크플로우 파일 존재 확인
test -f .github/workflows/ci.yml && echo "OK"

# 2. YAML 문법 검증
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo "valid YAML"

# 3. clippy 경고 0건 (CI와 동일 조건)
RUSTFLAGS="-Dwarnings" cargo clippy --all-targets --all-features 2>&1 | tail -5

# 4. fmt 통과
cargo fmt --all -- --check

# 5. 전체 테스트 통과
cargo test --all
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **cargo audit `continue-on-error`**: ort RC 버전 관련 advisory가 있을 수 있어 `continue-on-error: true`로 설정. stable 전환 시 제거.
- **ONNX Runtime 시스템 의존성**: `ort` crate가 `load-dynamic` feature를 사용하므로 CI에서 ONNX Runtime 라이브러리가 없을 수 있다. `cargo check`와 `cargo test`는 컴파일만 하면 통과하나, 런타임 테스트에서 `OrtEmbedder` 관련 ignored 테스트가 있으므로 영향 없음.
- **캐시 키 충돌**: `Cargo.lock` 해시 기반 캐시. 의존성 변경 시 자동 무효화.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `Cargo.toml` — 의존성 변경 없음
- `crates/secall-core/src/search/vector.rs` — Task 02 영역
- `crates/secall-core/src/search/query_expand.rs` — Task 04 영역
- `crates/secall/src/commands/ingest.rs` — Task 03 영역

단, clippy 경고 정리를 위해 아래 파일의 **import/mut 경고만** 수정 허용:
- `crates/secall-core/src/search/hybrid.rs` (line 5, 236-240)
- `crates/secall-core/src/ingest/detect.rs` (line 181)
- `crates/secall-core/src/search/bm25.rs` (line 427)
