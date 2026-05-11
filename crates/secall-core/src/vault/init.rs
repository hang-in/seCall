use std::path::Path;

use anyhow::Result;

pub fn init_vault(vault_path: &Path) -> Result<()> {
    // Create directory structure
    // P49 follow-up: `.sessions` dot-prefix 면 obsidian 이 자동으로 무시한다.
    std::fs::create_dir_all(vault_path.join("raw").join(".sessions"))?;
    std::fs::create_dir_all(vault_path.join("wiki").join("projects"))?;
    std::fs::create_dir_all(vault_path.join("wiki").join("topics"))?;
    std::fs::create_dir_all(vault_path.join("wiki").join("decisions"))?;

    // SCHEMA.md (only if not exists)
    let schema_path = vault_path.join("SCHEMA.md");
    if !schema_path.exists() {
        std::fs::write(&schema_path, schema_md())?;
    }

    // wiki/overview.md (only if not exists)
    let overview_path = vault_path.join("wiki").join("overview.md");
    if !overview_path.exists() {
        std::fs::write(&overview_path, overview_md())?;
    }

    // index.md (only if not exists)
    let index_path = vault_path.join("index.md");
    if !index_path.exists() {
        std::fs::write(&index_path, index_md())?;
    }

    // log.md (only if not exists)
    let log_path = vault_path.join("log.md");
    if !log_path.exists() {
        std::fs::write(&log_path, log_md())?;
    }

    Ok(())
}

fn schema_md() -> String {
    format!(
        r#"---
type: schema
updated_at: {}
---

# seCall Wiki Schema

## 페이지 구조

모든 wiki 페이지는 YAML frontmatter를 포함해야 합니다:

```
---
title: "페이지 제목"
type: project | topic | decision
created: YYYY-MM-DD
updated: YYYY-MM-DD
sources: ["session-id-1", "session-id-2"]
tags: ["tag1", "tag2"]
---
```

## 디렉토리 규칙

- `wiki/projects/` — 프로젝트별 페이지 (예: secall.md, tunaflow.md)
- `wiki/topics/` — 주제별 페이지 (예: rust-unsafe-patterns.md, korean-nlp.md)
- `wiki/decisions/` — 의사결정 기록 (예: 2026-04-05-embedder-trait.md)
- `wiki/overview.md` — 전체 위키 요약 + 페이지 목록

## 링크 규칙

- 세션 참조: `[[raw/.sessions/YYYY-MM-DD_session-id]]`
- 위키 내부 링크: `[[wiki/topics/topic-name]]`
- sources 배열에 참조한 세션 ID를 반드시 포함

## 파일명 규칙

- kebab-case (예: rust-unsafe-patterns.md)
- decision은 날짜 prefix (예: 2026-04-05-embedder-trait.md)

## 수정 금지

- `raw/.sessions/` 파일은 절대 수정하지 마세요 (immutable)
"#,
        chrono::Utc::now().format("%Y-%m-%d")
    )
}

fn overview_md() -> String {
    format!(
        r#"---
title: "Wiki Overview"
type: overview
created: {today}
updated: {today}
---

# seCall Wiki

에이전트 세션에서 추출된 지식 위키입니다.

## 프로젝트
<!-- 메타에이전트가 자동 갱신 -->

## 주제
<!-- 메타에이전트가 자동 갱신 -->

## 최근 결정
<!-- 메타에이전트가 자동 갱신 -->
"#,
        today = chrono::Utc::now().format("%Y-%m-%d")
    )
}

fn index_md() -> String {
    format!(
        r#"---
type: index
updated_at: {}
---

# seCall Index

## Sessions

"#,
        chrono::Utc::now().format("%Y-%m-%d")
    )
}

fn log_md() -> String {
    format!(
        r#"---
type: log
updated_at: {}
---

# seCall Ingest Log

"#,
        chrono::Utc::now().format("%Y-%m-%d")
    )
}
