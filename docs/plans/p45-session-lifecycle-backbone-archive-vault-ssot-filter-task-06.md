---
type: task
plan_slug: p45-session-lifecycle-backbone-archive-vault-ssot-filter
task_id: 06
title: 회귀 테스트 — round-trip + filter 동작 + cross-host re-ingest sync
parallel_group: C
depends_on: [03, 04, 05]
status: pending
updated_at: 2026-05-12
---

# Task 06 — 통합 회귀 테스트

## Changed files

수정:

- 없음 (또는 본 task 가 발견한 minor fix 한정).

신규:

- `crates/secall-core/tests/session_archive.rs` (신규 통합 테스트) — tempfile 기반 mini vault + sqlite 환경에서 end-to-end 시나리오 검증:
  1. `archive_round_trip_updates_db_and_vault_and_excludes_from_list` — 세션 ingest → archive_session 호출 → DB `is_archived=1` + vault frontmatter 두 라인 + `list_sessions_filtered` (default) 에서 제외 + `include_archived=true` 시 포함.
  2. `restore_round_trip_clears_db_and_vault` — 1번 후 restore_session → DB `is_archived=0` + vault frontmatter 두 라인 제거 + default list 에 복귀.
  3. `cross_host_archive_via_re_ingest_syncs_db` — 머신 A 가 vault frontmatter 의 `archived: true` 를 git 으로 받았다고 가정하고 (수동으로 frontmatter 수정 후) `insert_session_from_vault` 호출 → DB row 의 `is_archived` 가 자동 1 로 sync.
  4. `archive_excludes_from_bm25_search` — archived 세션의 본문 단어가 BM25 검색에서 hit 0건. `include_archived=true` 시 hit 1건.
  5. `archive_excludes_from_hybrid_search` — hybrid (BM25 + vector RRF) 결과에서 archived 제외 동일 확인. (벡터 인덱스 setup 비용이 크면 BM25-only 모드로 단순화 가능.)

회귀 테스트:

- 위 통합 테스트가 본 task 의 핵심.

## Change description

### 1. 테스트 인프라

`tempfile::tempdir` 기반 vault root + 별도 sqlite path. P38 / P44 의 통합 테스트 패턴 (`crates/secall-core/tests/wiki_cross_host_resolve.rs`, `crates/secall/tests/wiki_review_resolve.rs`) 그대로 차용.

공통 helper:

```rust
use std::path::Path;
use tempfile::TempDir;

use secall_core::ingest::{Session, /* ... */};
use secall_core::store::{Database, SessionRepo};
use secall_core::vault::Vault;

struct Harness {
    _dir: TempDir,
    db: Database,
    vault: Vault,
}

fn setup_harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("secall.sqlite");
    let db = Database::open(&db_path).expect("open db");
    let vault = Vault::new(dir.path().to_path_buf());
    vault.init().expect("init vault");
    Harness { _dir: dir, db, vault }
}

fn ingest_dummy_session(h: &Harness, id: &str, body: &str) -> String {
    // Session struct 만들고 vault.write_session + db.insert_session_from_vault
    // → return vault_path (rel)
}
```

### 2. archive_round_trip

```rust
#[test]
fn archive_round_trip_updates_db_and_vault_and_excludes_from_list() {
    let h = setup_harness();
    let vault_rel = ingest_dummy_session(&h, "sess-A", "hello world");

    let repo = SessionRepo::new(&h.db.conn());
    repo.archive_session("sess-A", &h.vault, chrono_tz::UTC).unwrap();

    // DB
    let archived: i64 = h.db.conn()
        .query_row("SELECT is_archived FROM sessions WHERE id = 'sess-A'",
                   [], |r| r.get(0)).unwrap();
    assert_eq!(archived, 1);

    // Vault frontmatter
    let abs = h.vault.path().join(&vault_rel);
    let content = std::fs::read_to_string(&abs).unwrap();
    assert!(content.contains("\narchived: true\n"));
    assert!(content.contains("archived_at:"));

    // list_sessions_filtered default 에서 제외
    let filter = SessionListFilter { page: 1, page_size: 10, ..Default::default() };
    let page = repo.list_sessions_filtered(&filter).unwrap();
    assert!(page.items.iter().all(|it| it.id != "sess-A"));

    // include_archived=true 면 포함
    let filter_inc = SessionListFilter { include_archived: true, page: 1, page_size: 10, ..Default::default() };
    let page_inc = repo.list_sessions_filtered(&filter_inc).unwrap();
    assert!(page_inc.items.iter().any(|it| it.id == "sess-A"));
}
```

### 3. cross_host_archive_via_re_ingest_syncs_db

```rust
#[test]
fn cross_host_archive_via_re_ingest_syncs_db() {
    let h = setup_harness();
    let vault_rel = ingest_dummy_session(&h, "sess-X", "hello");

    // 시뮬레이션: 다른 머신에서 archive → 우리 머신이 git pull 로 frontmatter 변경 받음
    let abs = h.vault.path().join(&vault_rel);
    let body = std::fs::read_to_string(&abs).unwrap();
    let modified = inject_archived_into_frontmatter(&body, true, "2026-05-12T15:00:00Z");
    std::fs::write(&abs, modified).unwrap();

    // re-ingest
    let fm = parse_session_frontmatter(&std::fs::read_to_string(&abs).unwrap()).unwrap();
    let body_text = extract_body_text(&std::fs::read_to_string(&abs).unwrap());
    let repo = SessionRepo::new(&h.db.conn());
    repo.insert_session_from_vault(&fm, &body_text, &vault_rel).unwrap();

    let archived: i64 = h.db.conn()
        .query_row("SELECT is_archived FROM sessions WHERE id = 'sess-X'",
                   [], |r| r.get(0)).unwrap();
    assert_eq!(archived, 1);
}
```

### 4. archive_excludes_from_bm25_search

```rust
#[test]
fn archive_excludes_from_bm25_search() {
    let h = setup_harness();
    ingest_dummy_session(&h, "sess-q", "unique-token alpha");
    let repo = SessionRepo::new(&h.db.conn());
    repo.archive_session("sess-q", &h.vault, chrono_tz::UTC).unwrap();

    let bm25 = secall_core::search::bm25::Bm25Searcher::new(/* ... */);
    let hits = bm25.search("unique-token", 10, /* include_archived */ false).unwrap();
    assert!(hits.iter().all(|h| h.session_id != "sess-q"));

    let hits_inc = bm25.search("unique-token", 10, true).unwrap();
    assert!(hits_inc.iter().any(|h| h.session_id == "sess-q"));
}
```

### 5. hybrid 테스트의 단순화

벡터 인덱스 setup (BGE-M3 ollama 호출) 이 통합 테스트에 부담이면 BM25-only 검증으로 단순화 가능. 또는 hybrid 의 BM25 path 만 검증하고 vector path 는 단위 테스트로 위임.

## Dependencies

- task 03 — `insert_session_from_vault` 의 archive sync 가 완성.
- task 04 — `archive_session` / `restore_session` 가 사용 가능.
- task 05 — `list_sessions_filtered` 의 `include_archived` 옵션과 BM25 / hybrid 의 archive 필터가 적용됨.
- crate dep: `tempfile` 는 이미 `[dev-dependencies]` 에 있음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. 신규 통합 테스트
cargo test -p secall-core --test session_archive

# 3. 영향 모듈 회귀 (task 01-05 가 통과하는지 재확인)
cargo test -p secall-core --lib store::
cargo test -p secall-core --lib search::
cargo test -p secall-core --lib ingest::
cargo test -p secall-core --lib vault::

# 4. 전체 workspace 빠른 회귀
cargo test --workspace --lib
```

## Risks

- **BM25 검색 테스트의 한국어 토크나이저 의존** — seCall 의 BM25 는 lindera/kiwi-rs (macOS/Linux) 사용. CI ubuntu 환경에선 OK. 영문 "unique-token" 같은 단어로 회피.
- **벡터 인덱스 테스트의 ollama 의존** — 실제 BGE-M3 호출 시 외부 service 필요. mitigation: vector path 는 단위 테스트로, 통합 테스트는 BM25 path 만 검증.
- **`SessionRepo::new` API 가정** — 실제 시그니처가 다르면 helper 수정 필요. 본 task 구현 시 `crates/secall-core/src/store/session_repo.rs` 의 실제 생성 패턴을 확인 후 사용.
- **테스트 격리** — tempfile 기반이라 병렬 실행 OK. 단 한국어 토크나이저의 ENV var (`LINDERA_KO_DIC_PATH` 등) 가 환경 공유면 ENV_LOCK 필요 — 기존 search 테스트 패턴 참조.
- **`Database::open` 의 schema migration 자동 실행** — v9 → v10 자동 진행. 테스트 시 schema v10 보장됨.
- **간헐 실패 — vault index 갱신** — `Vault::write_session` 가 index/log 도 갱신. 동일 세션 두 번 ingest 시 idempotent 보장 필요 — `INSERT OR IGNORE` 패턴이 처리.

## Scope boundary (수정 금지)

- task 01-05 의 영역 (이미 완성된 코드). 본 task 는 통합 검증만.
- 만약 본 task 가 task 01-05 에 버그 발견 시 해당 task 의 변경으로 fix — 본 task 의 scope 가 아닌 commit 으로 분리.
- `crates/secall/src/commands/*` — REST/CLI (P46).
- `web/*` — Web UI (P46).
- `crates/secall-core/src/wiki/*`, `crates/secall-core/src/graph/*` — P49.
