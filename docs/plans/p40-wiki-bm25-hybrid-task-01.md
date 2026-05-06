---
type: task
plan_slug: p40-wiki-bm25-hybrid
task_id: 01
title: DB v9 마이그레이션 — wiki_vectors 테이블
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-06
---

# Task 01 — DB v9 마이그레이션 (`wiki_vectors`)

## Changed files

신규/수정:

- `crates/secall-core/src/store/schema.rs:1` — `CURRENT_SCHEMA_VERSION` 8 → 9
- `crates/secall-core/src/store/schema.rs` — 새 SQL const `CREATE_WIKI_VECTORS` 추가 (파일 하단)
- `crates/secall-core/src/store/db.rs:48` (`fn migrate`) — `if current < 9 { ... }` 블록 추가 (line 122 의 v8 블록 다음)
- `crates/secall-core/src/store/db.rs:12` (use) — `CREATE_WIKI_VECTORS` import 추가
- `crates/secall-core/tests/db_migrations.rs` (신규 또는 기존 마이그레이션 테스트 파일) — v8→v9 회귀 테스트 1건

> 위 마지막 항목의 정확 경로는 Developer 가 기존 마이그레이션 테스트 파일 존재 여부를 확인 후 결정 (없으면 신규).

## Change description

1. **Schema 상수 추가** — `schema.rs` 에 다음 SQL 추가:
   ```sql
   CREATE TABLE IF NOT EXISTS wiki_vectors (
       wiki_path     TEXT PRIMARY KEY,    -- vault 기준 상대경로 (e.g., "wiki/projects/secall.md")
       embedding     BLOB NOT NULL,        -- f32 little-endian, dim 모델별 (bge-m3 = 1024)
       model_id      TEXT NOT NULL,        -- e.g., "bge-m3" — 모델 변경 감지
       content_hash  TEXT NOT NULL,        -- SHA-256 of page text — incremental skip 용
       updated_at    TEXT NOT NULL         -- RFC3339 UTC
   );
   CREATE INDEX IF NOT EXISTS idx_wiki_vectors_model ON wiki_vectors(model_id);
   ```
2. **CURRENT_SCHEMA_VERSION** 9 로 증가.
3. **db.rs `migrate`** 에 v9 블록 추가:
   ```rust
   if current < 9 {
       self.conn.execute_batch(CREATE_WIKI_VECTORS)?;
   }
   ```
   기존 v8 블록 (line 117–122) 패턴을 그대로 따름.
4. **회귀 테스트** — 기존 v8 DB 를 (테스트용 in-memory) 만든 후 `migrate()` 호출 → `wiki_vectors` 테이블 존재 확인 + `schema_version=9` 확인.

## Dependencies

- 없음 (마이그레이션은 본 plan 의 첫 task)
- crate dep 추가 없음 (rusqlite + sha2 워크스페이스에 이미 있음, sha2 는 task 02 에서 사용)

## Verification

```bash
# 1. 컴파일
cargo check -p secall-core

# 2. 마이그레이션 회귀 (신규 또는 기존 파일에 추가)
cargo test -p secall-core --test db_migrations migrate_v8_to_v9
# 또는 마이그레이션 테스트가 inline 이면:
cargo test -p secall-core schema::tests::migrate_v8_to_v9

# 3. 전체 store 테스트 — v9 추가가 기존 회귀를 깨지 않는지
cargo test -p secall-core --lib store::
```

기대 결과: 회귀 테스트 1건 신규 통과, 기존 store 테스트 모두 그대로 통과.

## Risks

- **다른 마이그레이션 블록 변경**: 절대 v1~v8 블록 건드리지 말 것. v9 블록만 추가.
- **column 타입 결정**: `embedding BLOB` 으로 통일 (turn_vectors 와 동일 방식). `dim` 컬럼 추가 검토 → 보류 (model_id 로 추론 가능, schema 단순 유지).
- **PRIMARY KEY 결정**: `wiki_path` 단일 PK. 페이지 이름/위치 변경 시 새 row → orphan 가능 → task 02 의 인덱서가 fs 스캔 시 DB 에 없는 row 정리 (cleanup).

## Scope boundary (수정 금지)

- `crates/secall-core/src/store/db.rs` 의 v1~v8 마이그레이션 블록 (line 66–122) — 변경 X
- `crates/secall-core/src/store/vector_repo.rs` (turn_vectors) — 본 task 영역 외
- `crates/secall-core/src/store/schema.rs` 의 기존 SQL 상수 — 변경 X (추가만)
