# graph build 시 파일 읽기/파싱 실패를 continue로 조용히 건너뜀

- **Category**: test
- **Severity**: major
- **Fix Difficulty**: auto
- **Status**: open
- **File**: crates/secall/src/commands/graph.rs:92

## Description

run_semantic에서 vault 파일 읽기 실패나 frontmatter 파싱 실패 시 tracing::warn만 기록하고 skipped += 1로 넘어갑니다. 테스트가 없어 이 분기(skipped 카운팅, 에러 메시지 포맷, 루프 진행)의 정확성을 검증할 수 없습니다. 다수의 세션이 silently 누락된 채 그래프가 부분 빌드될 수 있으며, 사용자는 최종 'X skipped' 수치만 보고 원인을 알 수 없습니다.

**Evidence**: `let content = match std::fs::read_to_string(&md_path) {
    Ok(c) => c,
    Err(e) => {
        tracing::warn!(session = short, "cannot read vault file: {}", e);
        skipped += 1;
        continue;
    }
};

let fm = match parse_session_frontmatter(&content) {
    Ok(f) => f,
    Err(e) => {
        tracing::warn!(session = short, "cannot parse frontmatter: {}", e);
        skipped += 1;
        continue;
    }
};`

## Snippet

```
skipped += 1;
continue;
```
