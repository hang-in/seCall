---
type: task
plan_slug: p45-session-lifecycle-backbone-archive-vault-ssot-filter
task_id: 02
title: Vault frontmatter writer 확장 — archived / archived_at 직렬화 + in-place updater
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-12
---

# Task 02 — Vault frontmatter writer 확장

## Changed files

수정:

- `crates/secall-core/src/ingest/markdown.rs:61-97` 의 `render_session` — frontmatter 생성 마지막 부근 (예: `tokens_out` 직후) 에 `Session` 에 `archived` 정보가 있을 때 `archived: true` / `archived_at: "..."` 두 줄을 조건부로 추가. 신규 세션 ingest 시엔 기본 `archived = false` 이므로 출력 X.
- `crates/secall-core/src/ingest/types.rs` (또는 `Session` struct 가 있는 위치) — `Session` struct 에 `pub archived: bool` 과 `pub archived_at: Option<chrono::DateTime<chrono::Utc>>` 필드 추가. 기본값은 `Default` impl 또는 `Session::new` 에서 false / None.
- `crates/secall-core/src/vault/mod.rs:38-61` 의 `Vault::write_session` — 변경 X (render_session 이 처리).

신규:

- `crates/secall-core/src/vault/mod.rs` 또는 `crates/secall-core/src/vault/index.rs` 에 신규 헬퍼:

  ```rust
  /// 기존 vault session markdown 의 frontmatter `archived` / `archived_at` 만 in-place
  /// 갱신하고 본문은 보존. 파일이 존재하지 않으면 NotFound 에러.
  pub fn update_session_archive_frontmatter(
      &self,
      session_id: &str,
      vault_rel_path: &str,
      archived: bool,
      archived_at: Option<chrono::DateTime<chrono::Utc>>,
  ) -> Result<()>;
  ```

  내부 동작: 파일 read → frontmatter block (`---\n...\n---`) 추출 → `archived` / `archived_at` 라인 upsert (없으면 추가, 있으면 교체, archived=false 면 두 라인 모두 제거) → 본문 그대로 → atomic write (`.tmp` → rename).

회귀 테스트:

- `crates/secall-core/src/ingest/markdown.rs` 의 기존 `#[cfg(test)] mod tests` 에:
  1. `test_render_session_archived_false_omits_field` — Session.archived=false 시 출력에 `archived:` 라인 없음.
  2. `test_render_session_archived_true_includes_field` — archived=true + archived_at=Some(...) 시 두 라인 모두 포함.
- `crates/secall-core/src/vault/mod.rs` 의 `#[cfg(test)]` 에:
  3. `test_update_archive_frontmatter_adds_lines` — archived=false 인 기존 파일을 archived=true 로 update 후 두 라인이 frontmatter 에 있고 본문은 그대로.
  4. `test_update_archive_frontmatter_removes_lines_on_restore` — archived=true 인 파일을 archived=false 로 update 후 두 라인 모두 제거됨.

## Change description

### 1. `Session` struct 필드 확장

```rust
pub struct Session {
    // ... 기존 필드 ...
    pub session_type: String,
    /// archive 상태 — vault frontmatter SSOT 와 동기화
    pub archived: bool,
    /// archive 된 시각 (archived=true 일 때만 Some)
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

기존 `Default` impl 이나 explicit 초기화 모든 곳에서 `archived: false, archived_at: None` 으로 채움. parser 들 (claude.rs / codex.rs / gemini.rs / ...) 은 신규 세션 생성 시 두 필드 모두 default 로 설정 — ingest 시점엔 항상 archived=false.

> 단, vault re-ingest (P31) 경로에서 frontmatter parser 가 archived 를 읽어와 Session 에 채우는 것은 task 03 영역.

### 2. `render_session` 의 frontmatter 출력

`markdown.rs:97` (tokens_out 출력 직후) 부근에 추가:

```rust
out.push_str(&format!("tokens_out: {}\n", session.total_tokens.output));

if session.archived {
    out.push_str("archived: true\n");
    if let Some(at) = session.archived_at {
        out.push_str(&format!(
            "archived_at: \"{}\"\n",
            at.with_timezone(&tz).format("%Y-%m-%dT%H:%M:%S%:z")
        ));
    }
}
```

> archived=false 면 두 라인 모두 출력 X — frontmatter 가 깔끔하게 유지됨 (기존 파일과 호환).

### 3. `update_session_archive_frontmatter` 헬퍼

`Vault` impl 에 메서드 추가:

```rust
impl Vault {
    pub fn update_session_archive_frontmatter(
        &self,
        vault_rel_path: &str,
        archived: bool,
        archived_at: Option<chrono::DateTime<chrono::Utc>>,
        tz: chrono_tz::Tz,
    ) -> Result<()> {
        let abs = self.path.join(vault_rel_path);
        let content = std::fs::read_to_string(&abs)?;

        let (fm_block, body) = split_frontmatter(&content)?;
        let new_fm = upsert_archive_lines(&fm_block, archived, archived_at, tz);

        let new_content = format!("---\n{new_fm}---\n{body}");
        let tmp = abs.with_extension("md.tmp");
        std::fs::write(&tmp, new_content)?;
        std::fs::rename(&tmp, &abs)?;
        Ok(())
    }
}

fn split_frontmatter(content: &str) -> Result<(String, String)> {
    let stripped = content
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow::anyhow!("session markdown missing frontmatter prefix"))?;
    let (fm, body) = stripped
        .split_once("\n---\n")
        .ok_or_else(|| anyhow::anyhow!("session markdown frontmatter not terminated"))?;
    Ok((format!("{fm}\n"), body.to_string()))
}

fn upsert_archive_lines(
    fm: &str,
    archived: bool,
    archived_at: Option<chrono::DateTime<chrono::Utc>>,
    tz: chrono_tz::Tz,
) -> String {
    // 기존 archived / archived_at 라인 제거
    let mut kept: Vec<&str> = fm
        .lines()
        .filter(|line| {
            let t = line.trim_start();
            !t.starts_with("archived:") && !t.starts_with("archived_at:")
        })
        .collect();

    if archived {
        let archived_line = "archived: true".to_string();
        kept.push(Box::leak(archived_line.into_boxed_str()));
        // archived_at 출력은 매번 다른 timestamp 이므로 String 으로 push
        // → Vec<&str> 대신 Vec<String> 으로 작성하는 게 더 적절. (구현 시 수정)
    }
    // ... 위 의사 코드는 구조 참고용. 실제 구현은 Vec<String> 으로 작성.
    kept.iter()
        .map(|l| format!("{l}\n"))
        .collect::<String>()
}
```

> 위 예시는 구조 참고용. 실제 구현 시 `Vec<String>` 으로 작성해 owned string 처리.

> `tz` 인자는 timestamp 포맷을 일관되게 (다른 frontmatter 필드와 동일 timezone). 호출자가 `Config::timezone()` 으로 전달.

### 4. atomic write

`render_session` 의 기존 `Vault::write_session` 가 이미 `.md.tmp → rename` 패턴 (mod.rs:52-54) 을 사용. `update_session_archive_frontmatter` 도 동일 패턴 적용 — 부분 write 로 인한 frontmatter corruption 방지.

## Dependencies

- 의존 task 없음 (parallel_group A 시작).
- crate dep: 추가 없음 (chrono, chrono-tz 는 이미 dep).

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. render_session 신규 테스트
cargo test -p secall-core --lib ingest::markdown::tests::test_render_session_archived

# 3. update_archive_frontmatter 신규 테스트
cargo test -p secall-core --lib vault::tests::test_update_archive_frontmatter

# 4. 기존 markdown 회귀
cargo test -p secall-core --lib ingest::markdown::tests
```

## Risks

- **Session struct 필드 추가 → 깨지는 호출자** — 기존 모든 `Session { ... }` literal 이 깨짐. parser 6개 (claude/codex/gemini/gemini_web/chatgpt/claude_ai) + tests 다수. mitigation: `Session` 에 `Default` impl 추가하고 `..Default::default()` 호출 가능하도록 — 또는 모든 literal 에 `archived: false, archived_at: None` 명시 추가. 후자가 명시적 → 권장.
- **frontmatter parser 의 strict mode** — 기존 `parse_session_frontmatter` (markdown.rs:29) 는 `serde_yaml::from_str` 에 `#[serde(default)]` 가 있으므로 unknown 필드 무시. archived/archived_at 가 SessionFrontmatter struct 에 없어도 안전 (task 03 에서 추가).
- **timestamp 포맷 일관성** — `start_time` 과 동일한 `%Y-%m-%dT%H:%M:%S%:z` 포맷 사용. tz 인자 누락 시 UTC 로 fallback 하지 말고 caller 가 명시.
- **`update_session_archive_frontmatter` 동시성** — 두 프로세스가 동시에 같은 파일 update 시 last-write-wins. seCall 은 단일 사용자 가정 → 동시성 보호 별도 plan.
- **path 트래버설** — `vault_rel_path` 가 caller (task 04) 에서 항상 DB 의 `vault_path` 컬럼 값을 사용. 외부 입력 직결 X.

## Scope boundary (수정 금지)

- `crates/secall-core/src/store/*` — task 01 / 04 / 05 영역.
- `crates/secall-core/src/ingest/{claude,codex,gemini,gemini_web,chatgpt,claude_ai}.rs` — parser 들. 단 Session literal 에 `archived/archived_at` default 추가는 본 task 영역 (해당 추가만 허용).
- `crates/secall-core/src/search/*` — task 05 영역.
- `crates/secall-core/src/mcp/*` — task 05 영역.
- `crates/secall-core/src/wiki/*` — 본 plan 영역 외.
- `crates/secall/src/commands/*` — 본 plan 영역 외 (P46).
