# 이전 리뷰 지적사항 다수 해결됨 — regex 사전 컴파일, skip_embed_types, vector 필터

- **Category**: stability
- **Severity**: info
- **Fix Difficulty**: guided
- **Status**: in_progress
- **File**: crates/secall/src/commands/classify.rs:30

## Description

Previous Failure Patterns에 언급된 여러 이슈가 현재 코드에서 수정 확인됨: (1) classify.rs의 regex는 이제 사전 컴파일 + map_err로 에러 전파 (line 30-32), (2) skip_embed_types가 ingest.rs:607-615에서 정상 동작, (3) vector.rs:337-339에서 exclude_session_types 필터 적용됨. 이 항목들은 해결 완료 상태입니다.

**Evidence**: `// classify.rs:30
regex::Regex::new(pattern)
    .map(|re| super::ingest::CompiledRule::Pattern(re, rule.session_type.clone()))
    .map_err(|e| anyhow::anyhow!("invalid regex pattern {:?}: {}", pattern, e))

// ingest.rs:607-614
let skip_embed = config.ingest.classification.skip_embed_types.contains(&session.session_type);
if !skip_embed { vector_tasks.push(session); }

// vector.rs:337-339
if !filters.exclude_session_types.is_empty() && filters.exclude_session_types.contains(&meta.session_type) { return false; }`

## Snippet

```
multiple files — see evidence
```
