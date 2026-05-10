---
type: task
plan_slug: p43-wiki-review-backend-wiki
task_id: 05
title: toml_edit 도입 (config 주석 보존)
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-09
---

# Task 05 — toml_edit 도입 (config 주석 보존)

## Changed files

수정:
- `crates/secall-core/Cargo.toml` — `[dependencies]` 에 `toml_edit = "0.22"` 추가 (또는 워크스페이스의 최신 minor).
- `crates/secall-core/src/vault/config.rs:381-397` 의 `Config::save()` — `toml::to_string_pretty(self)?` → `toml_edit` 기반 round-trip 으로 교체. 기존 toml 파일이 있으면 (a) 파싱 → DocumentMut 로 보관 (b) self 의 변경분 적용 (c) 직렬화. 파일이 없으면 from-scratch 직렬화.

신규:
- `crates/secall-core/src/vault/config.rs` 안에 helper `fn merge_into_doc(doc: &mut toml_edit::DocumentMut, config: &Config) -> Result<()>` — Config 의 각 섹션을 DocumentMut 의 해당 table 에 머지. 섹션이 없으면 새로 추가, 있으면 키별 갱신.

회귀 테스트:
- `crates/secall-core/src/vault/config.rs` 의 기존 `#[cfg(test)] mod tests` 에 4 test fn 추가:
  1. `save_preserves_top_level_comments` — 주석 포함 toml load → save → 다시 read → 원본 주석 그대로.
  2. `save_preserves_inline_comments` — 키 옆 inline 주석 (`field = "x"  # comment`) 유지.
  3. `save_writes_new_keys_in_existing_section` — 기존 `[wiki]` 섹션에 새 key 추가 시 섹션 위치 유지.
  4. `save_creates_new_section_when_absent` — `[log]` 섹션이 없는 toml 에 log.backend 설정 후 save → `[log]` 섹션 생성.

## Change description

### 1. Cargo.toml dep

```toml
[dependencies]
toml_edit = "0.22"
```

기존 `toml = "..."` 는 그대로 — `Config::load_or_default` 가 사용. `toml_edit` 은 save 만 사용.

### 2. Config::save 재구현

```rust
pub fn save(&self) -> Result<()> {
    use toml_edit::{DocumentMut, value};

    let path = Self::config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 1. 기존 파일이 있으면 파싱
    let mut doc = if path.exists() {
        let raw = std::fs::read_to_string(&path)?;
        raw.parse::<DocumentMut>()
            .context("existing config.toml is invalid")?
    } else {
        DocumentMut::new()
    };

    // 2. self 의 각 섹션을 doc 에 머지 (키별)
    merge_into_doc(&mut doc, self)?;

    // 3. atomic write (기존 패턴 유지)
    let tmp_path = path.with_extension(format!(
        "toml.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::write(&tmp_path, doc.to_string())?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}
```

### 3. merge_into_doc helper

```rust
fn merge_into_doc(doc: &mut toml_edit::DocumentMut, config: &Config) -> Result<()> {
    use toml_edit::{table, value, Item};

    // 각 섹션마다:
    // - section 이 없으면 doc.insert(section_name, table()) 로 추가
    // - section 의 keys 를 toml_edit 의 Item 으로 set

    macro_rules! sync_section {
        ($name:expr, $field:expr) => {{
            let serialized = toml::Value::try_from($field)?;
            // serialized 의 각 key 를 doc[$name] 에 set
            let table_item = doc.entry($name)
                .or_insert(Item::Table(toml_edit::Table::new()))
                .as_table_mut()
                .context(concat!("[", $name, "] is not a table"))?;
            if let toml::Value::Table(map) = serialized {
                for (k, v) in map {
                    table_item.insert(&k, toml_to_edit_item(v));
                }
            }
            Ok::<(), anyhow::Error>(())
        }};
    }

    sync_section!("vault", &config.vault)?;
    sync_section!("ingest", &config.ingest)?;
    sync_section!("search", &config.search)?;
    sync_section!("hooks", &config.hooks)?;
    sync_section!("embedding", &config.embedding)?;
    sync_section!("openvino", &config.openvino)?;
    sync_section!("output", &config.output)?;
    sync_section!("wiki", &config.wiki)?;
    sync_section!("graph", &config.graph)?;
    sync_section!("log", &config.log)?;
    Ok(())
}

fn toml_to_edit_item(v: toml::Value) -> toml_edit::Item {
    // toml::Value → toml_edit::Item 변환 (string / int / bool / array / table)
    // toml_edit 의 from impl 사용 또는 수동 매칭
}
```

> `toml_to_edit_item` 의 정확한 구현은 `toml_edit::Item::from(toml::Value)` 가 있으면 그대로 사용. 없으면 수동 변환 — string/int/bool/array/table 5종.

### 4. 회귀 테스트

```rust
#[test]
fn save_preserves_top_level_comments() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, r#"
# Top-level note: this is the user's comment.
# Multiple lines.

[vault]
path = "/tmp/test"
"#).unwrap();
    std::env::set_var("SECALL_CONFIG_PATH", &path);

    let mut config = Config::load_or_default();
    config.vault.path = "/tmp/changed".into();
    config.save().unwrap();
    std::env::remove_var("SECALL_CONFIG_PATH");

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("# Top-level note: this is the user's comment."));
    assert!(saved.contains("# Multiple lines."));
    assert!(saved.contains(r#"path = "/tmp/changed""#));
}

#[test]
fn save_preserves_inline_comments() { ... }

#[test]
fn save_writes_new_keys_in_existing_section() { ... }

#[test]
fn save_creates_new_section_when_absent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, r#"
[vault]
path = "/tmp/test"
"#).unwrap();
    std::env::set_var("SECALL_CONFIG_PATH", &path);

    let mut config = Config::load_or_default();
    config.log.backend = Some("ollama".into());
    config.save().unwrap();
    std::env::remove_var("SECALL_CONFIG_PATH");

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("[log]"));
    assert!(saved.contains(r#"backend = "ollama""#));
    // vault 섹션 보존
    assert!(saved.contains(r#"path = "/tmp/test""#));
}
```

## Dependencies

- 의존 task 없음. 본 task 는 config save 영역만 손댐 — task 01–04 와 disjoint.
- crate dep: `toml_edit = "0.22"` 추가. 워크스페이스 dep 등록 시 다른 crate 영향 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. 회귀 테스트
cargo test -p secall-core --lib vault::config::tests::save_

# 3. 기존 P41 task 03 의 rest_config 테스트 영향 없음
cargo test -p secall-core --test rest_config

# 4. (수동) 사용자 주석 보존 확인
echo '# user comment\n[vault]\npath = "/tmp/x"' > /tmp/config.toml
SECALL_CONFIG_PATH=/tmp/config.toml ./target/debug/secall config set log.backend haiku
cat /tmp/config.toml
# 출력에 "# user comment" 줄이 살아 있어야 함
```

## Risks

- **`toml_edit` API 의 안정성** — 0.22.x 는 안정 minor. `DocumentMut` / `Item` API 가 다음 메이저에서 깨질 수 있음 — Cargo.toml 의 minor pin 으로 대응.
- **`merge_into_doc` 의 키 순서 변경** — toml_edit 의 `insert` 가 기존 key 를 덮어쓸 때 순서 보존. 새 key 는 끝에 추가. 사용자 의도와 다를 수 있음 — 본 task 는 보수적으로 "기존 키는 위치 유지, 새 키만 끝에" 로 구현.
- **`toml::Value::try_from(&self.field)` 의 비용** — `Serialize` 호출 → 큰 config 의 경우 약간의 alloc. Config 가 작아서 (수십 키) 무시 가능.
- **`HashMap<String, WikiBackendConfig>` 의 직렬화 순서** — 비결정적. 새 key 추가 시 순서 안정성을 위해 BTreeMap 또는 sort. 본 task 는 기존 HashMap 순서 그대로 (toml_edit 가 내부 insert 순서 따름).
- **load 측은 그대로** — `Config::load_or_default` 가 `toml` crate 사용. round-trip 은 toml_edit save 후 toml load. 둘이 incompat 한 형식 (toml 1.0 spec 호환) 차이 없음 — 둘 다 spec 준수.
- **macro_rules! 의 가독성** — 10 섹션 macro 호출. clippy lint 에서 unused arg 같은 경고 가능 — 필요 시 일반 함수 + 클로저로 대체.

## Scope boundary (수정 금지)

- `crates/secall-core/src/vault/config.rs` 의 `Config::load_or_default` — toml crate 그대로.
- `crates/secall-core/src/wiki/` — task 01 / 02 / 04 영역.
- `crates/secall/src/commands/wiki.rs` — task 03 영역.
- `crates/secall-core/src/mcp/server.rs` 의 `do_config_patch` — 본 task 가 save 만 변경, patch 로직은 그대로.
- `crates/secall-core/src/graph/semantic.rs` — task 06 영역.
