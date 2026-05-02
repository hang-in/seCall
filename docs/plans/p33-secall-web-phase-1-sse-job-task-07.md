---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 07
parallel_group: C
depends_on: [02]
---

# Task 07 — ingest 후 graph 자동 증분 (옵션 플래그)

## Changed files

수정:
- `crates/secall/src/commands/ingest.rs` — CLI 인자 + ingest_sessions에 `auto_graph: bool` 추가
- `crates/secall/src/commands/sync.rs` — sync 명령에서 `auto_graph` 기본 활성 (또는 `--no-auto-graph`로 비활성)
- `crates/secall/src/commands/graph.rs` — `run_build`에 `incremental: bool` 옵션 추가 (또는 새 함수 `run_build_incremental(session_ids: &[String])`)
- `crates/secall-core/src/graph/extract.rs` (또는 graph 빌드 진입점) — 단일/소수 세션 대상 증분 빌드 함수 추가

신규:
- (선택) `crates/secall-core/tests/graph_incremental.rs` — 통합 테스트

## Change description

### 1. 증분 빌드 함수 추가

기존 `run_build`는 vault 전체 스캔. 신규 세션만 graph_nodes/graph_edges에 추가하는 함수 필요:

`crates/secall-core/src/graph/extract.rs` 또는 `mod.rs`에:
```rust
/// 단일/소수 세션의 graph 노드 + 엣지 증분 추출.
/// 이미 존재하는 노드/엣지는 INSERT OR IGNORE로 skip.
pub fn extract_for_sessions(
    db: &Database,
    vault_path: &Path,
    session_ids: &[String],
) -> Result<GraphIncrementalReport> {
    // 1) 각 session_id의 frontmatter 읽기 (vault/raw/sessions/YYYY-MM-DD/{id}.md)
    // 2) 규칙 기반 노드/엣지 추출 (project, agent, file, issue, tech, topic)
    // 3) graph_nodes / graph_edges INSERT OR IGNORE
    // 4) same_project/same_agent/same_day 같은 cross-session edges는 skip (full rebuild에서만)
    //    또는 단일 세션이면 새 세션과 기존 세션 사이에만 추가
    Ok(report)
}

pub struct GraphIncrementalReport {
    pub nodes_added: usize,
    pub edges_added: usize,
    pub sessions_processed: usize,
}
```

### 2. ingest CLI 옵션

`crates/secall/src/commands/ingest.rs`:
```rust
pub async fn run(
    paths: Vec<PathBuf>,
    cwd: Option<PathBuf>,
    auto_detect: bool,
    force: bool,
    min_turns: Option<usize>,
    no_semantic: bool,
    auto_graph: bool,           // ← 추가
    output_format: OutputFormat,
) -> Result<()> {
    // ...
    let report = ingest_sessions(...).await?;
    if auto_graph && !report.new_sessions.is_empty() {
        let graph_report = extract_for_sessions(&db, &vault_path, &report.new_sessions)?;
        eprintln!(
            "Graph: {} nodes / {} edges added for {} sessions",
            graph_report.nodes_added, graph_report.edges_added, graph_report.sessions_processed
        );
    }
    Ok(())
}
```

CLI 인자에 `--auto-graph` 플래그 추가 (clap derive). 기본 false (CLI 호환). REST 호출 시 sync는 기본 true.

`ingest_sessions` 내부는 변경하지 않고, 호출부에서 결과의 새 session_id 리스트를 받아 별도 호출.

### 3. sync에서 auto_graph 기본 활성

`crates/secall/src/commands/sync.rs`의 phase 3 (ingest) 후:
```rust
sink.phase_start("ingest").await;
let ingest_report = run_auto_ingest(...).await?;
sink.phase_complete("ingest", Some(json!({
    "new_sessions": ingest_report.new_sessions.len()
}))).await;

// 신규: 그래프 증분
if !no_graph && !ingest_report.new_sessions.is_empty() {
    sink.phase_start("graph").await;
    let graph_report = extract_for_sessions(&db, &vault_path, &ingest_report.new_sessions)?;
    sink.phase_complete("graph", Some(json!({
        "nodes_added": graph_report.nodes_added,
        "edges_added": graph_report.edges_added,
    }))).await;
}
```

`SyncArgs`에 `no_graph: bool` 추가 (기본 false → 그래프 증분 활성).

CLI는 `--no-graph` 플래그 추가.

### 4. 시맨틱 엣지 (LLM)는 별도

`auto_graph`는 규칙 기반만 (빠름, LLM 호출 없음). 시맨틱 엣지(`fixes_bug`, `modifies_file`, `discusses_topic` 등)는 LLM 호출이 비싸므로 별도 명령 (`secall graph semantic`)로 유지. sync의 phase에는 포함하지 않음.

→ 만약 사용자가 시맨틱까지 자동화 원하면 별도 v1.1에서 옵션 추가.

### 5. 단위 테스트

`crates/secall-core/tests/graph_incremental.rs` 신규:
- vault에 3개 세션 markdown 작성 → extract_for_sessions(["s-1"]) → s-1의 노드/엣지만 추가
- 같은 세션 두 번 호출 → 추가 변화 없음 (idempotent)
- 존재하지 않는 vault path → 에러

## Dependencies

- Task 03 완료 (sync.rs `run_with_progress`에 phase 추가하는 흐름)
- 외부 crate 추가 없음

## Verification

```bash
# 1. 컴파일
cargo check --all-targets

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. 신규 테스트
cargo test -p secall-core --test graph_incremental

# 4. 회귀
cargo test --all

# 5. 라이브 검증 (수동)
# 신규 세션 ingest 후 그래프 자동 증분 확인:
./target/release/secall ingest --auto --cwd <project> --auto-graph
# 또는 sync (기본 활성):
./target/release/secall sync --local-only
# 후
sqlite3 ~/Library/Caches/secall/index.sqlite "SELECT COUNT(*) FROM graph_nodes WHERE id LIKE 'ses_%';"
# 신규 세션 수만큼 노드 추가 확인
```

## Risks

- **idempotency**: extract_for_sessions가 INSERT OR IGNORE 사용해서 idempotent. 다만 graph_edges는 (source, target, relation) UNIQUE라 같은 edge 두 번 추가해도 무시됨. 검증 필수
- **성능**: 단일 세션 추출은 빠르지만 (수십 ms), 매 세션마다 호출하면 ingest 전체 시간 증가. 한 번에 모든 신규 세션 묶어서 한 번 호출 권장 (위 코드 그렇게 함)
- **시맨틱 엣지 vs 규칙 엣지**: full rebuild에서만 cross-session edges (`same_project`, `same_agent`, `same_day`) 생성. 증분 빌드에서는 skip하면 신규 세션이 cross-session 관계를 못 가짐 → 그래프에서 고립 노드처럼 보일 수 있음. **대안**: 신규 세션과 기존 세션 사이의 cross-session edges만 추가. 코드 복잡도 증가. P33 MVP는 same_* edge는 skip하고 신규 세션 자체 노드 + 출엣지만 추가, full rebuild는 사용자가 주기적으로 실행
- **graph_nodes 누락 세션 보정**: P32 검증에서 `ses_opencode_realtest`가 graph에 없었던 이유. 본 task로 해결. 다만 과거 누락 세션은 사용자가 한 번 `secall graph build` 또는 `--auto-graph`로 ingest 재실행 필요
- **sync에 추가된 phase 5**: `pull → reindex → ingest → graph → push`. push 직전에 graph 추가하면 git 커밋 전 graph_nodes 변경되는데, graph 데이터는 vault가 아닌 SQLite라 git에 영향 없음. 안전

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/`, `src/jobs/`, `src/mcp/` — Task 01, 02, 04
- `web/` — Task 05, 06, 07
- `.github/workflows/`, `README*` — Task 09
- 시맨틱 그래프 추출 로직 (`graph/semantic.rs`) — 본 task는 규칙 기반만
- 기존 `run_build` 동작 — 호환 유지, 옵션만 추가
