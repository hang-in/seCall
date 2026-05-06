# graph.rs 커맨드에 테스트 모듈 없음 (188 LOC)

- **Category**: test
- **Severity**: major
- **Fix Difficulty**: auto
- **Status**: open
- **File**: crates/secall/src/commands/graph.rs:10

## Description

4개의 공개 함수(run_semantic, run_build, run_stats, run_export)를 포함한 188줄 파일에 #[cfg(test)] 모듈이 전혀 없습니다. run_semantic은 백엔드 오버라이드 로직(CLI 플래그 > 환경변수 > config 우선순위)이 복잡하게 얽혀 있고, run_build/run_stats/run_export는 DB와 파일시스템 양쪽에 영향을 줍니다. 회귀 감지 수단이 없어 config 오버라이드 우선순위 버그가 프로덕션에서 silently 잘못된 백엔드로 연결될 수 있습니다.

**Evidence**: `pub async fn run_semantic(
    delay_secs: f64,
    limit: Option<usize>,
    backend: Option<String>,
    api_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
) -> Result<()> {
    ...
    if let Some(b) = backend {
        config.graph.semantic_backend = b;
    }`

## Snippet

```
// 파일 전체에 #[cfg(test)] mod tests { ... } 없음
```
