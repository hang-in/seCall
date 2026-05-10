---
type: task
plan_slug: p44-wiki-cross-host-sources
task_id: 02
title: merge_with_existing 본문 concat 제거 (sources 만 합집합)
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-10
---

# Task 02 — `merge_with_existing()` 본문 concat 제거

## Changed files

수정:
- `crates/secall-core/src/wiki/lint.rs:52-108` 의 `pub fn merge_with_existing` — 본문 concat (`format!("{}\n\n---\n\n{}", existing_body, new_body)`) 제거. 새 본문 (`new_body`) 그대로 사용. sources 합집합 로직은 유지.

신규:
- 없음.

회귀 테스트:
- `crates/secall-core/src/wiki/lint.rs` 의 `#[cfg(test)] mod tests` (기존 또는 신규) 에 4 test fn 추가:
  1. `merge_replaces_body_keeps_sources_union` — 기존 페이지 + 새 페이지 (sources 다름) → 본문은 new_body, sources 는 합집합.
  2. `merge_skips_when_all_sessions_already_present` — 새 session_ids 가 모두 기존 sources 에 포함되면 기존 페이지 그대로 반환 (기존 동작 유지).
  3. `merge_creates_when_existing_absent` — wiki 파일이 없으면 new_content 그대로 반환 (기존 동작 유지).
  4. `merge_preserves_sources_order` — 기존 sources 순서 유지 + 새 sources 는 끝에 append.

## Change description

### 1. 본문 처리 변경

기존 (`lint.rs:104-107`):

```rust
merged_fm.updated_at = chrono::Utc::now().format("%Y-%m-%d").to_string();

// 본문 병합: 기존 + 구분선 + 새 내용
let merged_body = format!("{}\n\n---\n\n{}", existing_body.trim(), new_body.trim());
Ok(format_with_frontmatter(&merged_fm, &merged_body))
```

신규:

```rust
merged_fm.updated_at = chrono::Utc::now().format("%Y-%m-%d").to_string();

// 본문은 새 내용으로 교체 (사용자 수동 편집 가정 없음 — 누적 방지).
// sources 는 합집합으로 보존 — provenance 손실 방지.
Ok(format_with_frontmatter(&merged_fm, new_body.trim()))
```

`existing_body` 는 더 이상 사용 안 함 — `let (_, _) = split_frontmatter(&existing)` 로 변경 또는 `_existing_body` prefix.

### 2. 빠른 경로 (기존 동작 유지)

`lint.rs:80-86` 의 `all_already_present` 빠른 경로는 그대로 유지:

```rust
let all_already_present = !session_ids.is_empty()
    && session_ids.iter().all(|sid| merged_fm.sources.contains(sid));
if all_already_present {
    return Ok(existing.to_string());
}
```

이 경로는 `wiki update` 가 같은 session 으로 두 번 호출됐을 때 no-op. 본 task 의 변경과 충돌 없음.

### 3. 회귀 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn write_page(dir: &std::path::Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn merge_replaces_body_keeps_sources_union() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "topic.md",
            "---\ntype: topic\nstatus: draft\nupdated_at: 2026-05-09\nsources:\n  - sess-A\n---\n\n## 기존 본문",
        );
        let new_content = "---\ntype: topic\nstatus: draft\nupdated_at: 2026-05-10\nsources:\n  - sess-B\n---\n\n## 새 본문";
        let merged = merge_with_existing(
            dir.path(),
            "topic.md",
            new_content,
            &["sess-B".to_string()],
        )
        .unwrap();
        assert!(merged.contains("## 새 본문"));
        assert!(!merged.contains("## 기존 본문"), "old body should be replaced");
        assert!(merged.contains("sess-A"));
        assert!(merged.contains("sess-B"));
    }

    #[test]
    fn merge_skips_when_all_sessions_already_present() {
        let dir = tempfile::tempdir().unwrap();
        let existing = "---\ntype: topic\nstatus: draft\nupdated_at: 2026-05-09\nsources:\n  - sess-A\n---\n\n## 기존 본문";
        write_page(dir.path(), "topic.md", existing);
        let new_content = "---\ntype: topic\nstatus: draft\nupdated_at: 2026-05-10\nsources:\n  - sess-A\n---\n\n## 새 본문";
        let merged = merge_with_existing(
            dir.path(),
            "topic.md",
            new_content,
            &["sess-A".to_string()],
        )
        .unwrap();
        assert_eq!(merged, existing, "existing page kept verbatim when no new sessions");
    }

    #[test]
    fn merge_creates_when_existing_absent() {
        let dir = tempfile::tempdir().unwrap();
        let new_content = "---\ntype: topic\nsources:\n  - sess-A\n---\n\n## 본문";
        let merged = merge_with_existing(
            dir.path(),
            "topic.md",
            new_content,
            &["sess-A".to_string()],
        )
        .unwrap();
        assert_eq!(merged, new_content);
    }

    #[test]
    fn merge_preserves_sources_order() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "topic.md",
            "---\ntype: topic\nsources:\n  - sess-A\n  - sess-B\n---\n\n## old",
        );
        let new_content = "---\ntype: topic\nsources:\n  - sess-C\n---\n\n## new";
        let merged = merge_with_existing(
            dir.path(),
            "topic.md",
            new_content,
            &["sess-C".to_string()],
        )
        .unwrap();
        let pos_a = merged.find("sess-A").expect("A");
        let pos_b = merged.find("sess-B").expect("B");
        let pos_c = merged.find("sess-C").expect("C");
        assert!(pos_a < pos_b && pos_b < pos_c, "preserve insertion order");
    }
}
```

`tempfile` 은 `secall-core` 의 `[dev-dependencies]` 에 이미 있음 (P43 task 05 도 사용). 추가 dep X.

## Dependencies

- 의존 task 없음 (parallel_group A 시작).
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. 본 task 신규 테스트
cargo test -p secall-core --lib wiki::lint::tests::merge_

# 3. lint.rs 의 기존 테스트 회귀 (있으면)
cargo test -p secall-core --lib wiki::lint::tests

# 4. wiki 모듈 전체 회귀
cargo test -p secall-core --lib wiki::
```

## Risks

- **파괴적 변경 — 본문 누적 의도였던 사용자 영향** — 메모리 (`user_wiki_edit_policy`) 에 따르면 사용자는 위키를 수동 편집 안 함. 따라서 누적된 과거 본문이 손실되더라도 위키는 곧 sources 로부터 재생성 가능. risk acceptable.
- **첫 실행 후 본문 손실** — 동일 host 에서 같은 토픽을 두 번 호출하면, 두 번째 호출의 새 본문이 첫 번째 본문을 덮어씀. 이전엔 두 본문이 나란히 있었음. 사용자 워크플로 (`secall wiki update --session X`) 에서는 의도된 새 본문이라 OK.
- **`updated_at` 갱신 — `all_already_present` 빠른 경로** — 빠른 경로에선 `updated_at` 갱신 안 함 (기존 동작 유지). 새 sessions 가 있을 때만 갱신.
- **build_wiki_page 의 출력 형식 가정** — `new_body` 가 항상 valid frontmatter 없는 본문이라 가정. 실제로는 `validate_frontmatter()` 거친 후 `merge_with_existing` 호출 — frontmatter 가 있을 수 있음. `split_frontmatter` 가 정상 분리 → `new_body` 는 frontmatter 제외. 안전.
- **빈 `new_body` 처리** — LLM 응답이 비면 `new_body.trim()` 가 빈 문자열. 기존 본문이 빈 문자열로 덮어써짐. mitigation: 호출자 (`wiki.rs:317-326`) 에서 이미 LLM 출력 검증 후 호출 — 본 task 영역 외.

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/lint.rs` 의 `validate_frontmatter`, `insert_obsidian_links`, `split_frontmatter`, `parse_frontmatter_fields`, `format_with_frontmatter` — 본 task 는 `merge_with_existing` 만.
- `crates/secall/src/commands/wiki.rs` — task 01 / 03 영역.
- `crates/secall-core/src/vault/git.rs` — task 03 영역.
- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio,review}.rs` — backend 영역.
- README / docs — task 04 영역.
