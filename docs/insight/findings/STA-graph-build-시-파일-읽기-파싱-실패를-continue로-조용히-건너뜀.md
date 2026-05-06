# graph build 시 파일 읽기/파싱 실패를 continue로 조용히 건너뜀

- **Category**: stability
- **Severity**: major
- **Fix Difficulty**: guided
- **Status**: in_progress
- **File**: crates/secall-core/src/graph/build.rs:76

## Description

build.rs에서 세션 파일 읽기 실패(fs::read_to_string)와 frontmatter 파싱 실패 시 tracing::warn 후 continue로 건너뜁니다. 대량의 파일이 깨진 경우 그래프가 불완전하게 생성되지만, 사용자에게는 성공으로 보입니다. 건너뛴 파일 수를 최종 리포트에 포함해야 합니다.

**Evidence**: `Err(e) => {
    tracing::warn!(path = %path.display(), error = %e, "failed to read session file");
    continue;
}
...
Err(e) => {
    tracing::warn!(path = %path.display(), error = %e, "failed to parse frontmatter");
    continue;
}`

## Snippet

```
tracing::warn + continue on file read/parse error
```
