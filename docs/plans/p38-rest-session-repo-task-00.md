---
type: task
status: draft
updated_at: 2026-05-03
plan_slug: p38-rest-session-repo
task_id: 00
parallel_group: A
depends_on: []
---

# Task 00 — axum Router 통합 테스트 인프라 (test helper + dev-dep)

## Changed files

수정:
- `crates/secall-core/Cargo.toml` — `[dev-dependencies]` 에 `tower = { version = "0.5", features = ["util"] }` 추가 (`ServiceExt::oneshot` 트레이트). `axum` 본체는 dependencies 에 이미 있음 → dev-dep 중복 추가 불필요. `mime` 또는 `http-body-util` 도 필요 시 추가.

신규:
- `crates/secall-core/tests/common/mod.rs` — 테스트 공유 fixture:
  - `TestEnv` 구조체 (tempdir + Database + JobExecutor + axum Router 보관)
  - `pub fn make_test_env() -> TestEnv` — fake adapters (jobs_rest 패턴 그대로) 주입 + `rest_router(...)` 빌드
  - helper: `pub async fn send_request(router: &Router, method, uri, body: Option<Value>) -> (StatusCode, Value)` — `ServiceExt::oneshot` 래핑, JSON body 직렬화/역직렬화 자동
  - helper: `pub fn insert_minimal_session(db: &Database, id: &str)` — `tests/rest_listing.rs:make_session` 같은 fixture 의 공통 버전
- `crates/secall-core/tests/rest_routes.rs` — Task 02/03 가 사용할 entry. 본 task 에서는 빈 모듈 (`mod common;` 선언 + sanity 테스트 1건 — `test_router_smoke` 가 GET /api/status 200 반환 검증).

## Change description

### 핵심 설계

axum 0.8 의 `Router` 는 `tower::Service` 트레이트를 구현 → `ServiceExt::oneshot(request)` 호출로 in-process 단발 요청/응답 가능. HTTP listen 없음 → 빠름 + 포트 충돌 없음 + 격리.

### `TestEnv` 계약

```text
TestEnv {
    _tempdir: TempDir,    // RAII drop 으로 정리
    db: Arc<Mutex<Database>>,
    executor: Arc<JobExecutor>,
    router: Router,
}
```

- `make_test_env()` 가 호출 1회당 격리된 DB + executor + router 인스턴스 반환.
- 기존 `tests/jobs_rest.rs` 의 `make_adapters()` (fake sync/ingest/wiki/graph_rebuild fn) 를 `common::make_fake_adapters()` 로 추출 → 모든 라우트 테스트 공유.
- DB 가 신규 (v8 schema) 로 초기화되므로 마이그레이션 path 도 자동 검증.

### `send_request` 시그니처

```text
pub async fn send_request(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value)
```

- body Some → `Content-Type: application/json` + JSON 직렬화 본문
- 응답 본문은 항상 JSON 으로 deserialize 시도 — 빈 본문이면 `Value::Null` 반환
- 에러 본문 (`{"error": ...}`) 도 동일 경로 → 호출자가 status + json 으로 분기 검증

### Sanity 테스트

`tests/rest_routes.rs` 에 다음 1건만 본 task 에서 추가:
- `test_router_smoke_get_status` — `GET /api/status` → 200 + JSON object (`status_*` 키 존재 정도). 인프라가 제대로 빌드되는지 확인.

다른 라우트 회귀는 Task 02/03 가 담당.

### `tests/common/mod.rs` 사용 패턴

Cargo 의 integration tests 는 각 `tests/*.rs` 가 별도 crate 로 컴파일되므로 `common` 모듈 공유는 다음 패턴:
- `tests/common/mod.rs` 정의 → 각 `tests/rest_routes.rs` / `tests/session_repo_helpers.rs` 안에서 `mod common;` 선언 + `use common::*;`
- 또는 `tests/common.rs` 단일 파일도 가능. 본 task 는 `tests/common/mod.rs` 디렉터리 형태로 (향후 fixture 추가 여지).

### dev-dep `tower`

axum 0.8 가 내부적으로 tower 0.5 사용 중 → workspace 호환. `features = ["util"]` 가 `ServiceExt` 노출.

## Dependencies

- 외부 crate: `tower` (dev-dep 신규). axum / serde_json 은 기존 dep.
- 내부 task: 없음

## Verification

```bash
cargo check --tests
cargo clippy --tests --all-features -- -D warnings
cargo fmt --all -- --check
cargo test -p secall-core --test rest_routes test_router_smoke
```

## Risks

- **`tower` 버전 호환**: axum 0.8 가 내부 사용하는 tower 와 dev-dep 버전 충돌 가능. cargo 가 두 버전 동시 사용 가능하지만 ServiceExt trait 가 다른 버전이면 oneshot 호출 안 됨. axum 0.8 → tower 0.5 매칭 권장.
- **`Body` deserialize**: axum 0.8 응답의 body 가 `http_body_util::BodyExt::collect` 로 모아야 함. `axum::body::to_bytes` (axum 0.8 부터) 가 더 단순. 둘 중 하나로 통일.
- **fake adapter sync 처리**: P36/P37 의 sync/ingest/wiki/graph_rebuild fn 이 `BroadcastSink + CancellationToken` 받음 → fake 구현은 즉시 결과 반환 (LLM/외부 호출 없이). 기존 `tests/jobs_rest.rs::make_adapters` 그대로 추출.
- **DB schema 초기화**: `Database::open(tempdir/test.db)` 가 v8 마이그레이션 자동 실행. 추가 setup 불필요.
- **테스트 격리**: 각 `make_test_env()` 가 새 tempdir 사용 → 동시 실행 안전. `SECALL_DB_PATH` env 안 건드림.

## Scope boundary

수정 금지:
- `crates/secall-core/src/` 전체 production 코드 — 본 phase 는 테스트만
- 기존 `tests/{rest_listing,jobs_rest,graph_incremental}.rs` — 보존, 새 테스트는 별도 파일
- `crates/secall/`, `web/`, `README*`, `.github/` — Task 02-05 영역 또는 무관
