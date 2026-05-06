# post-ingest hook 실패 시 에러가 로그만 남기고 무시됨

- **Category**: stability
- **Severity**: major
- **Fix Difficulty**: guided
- **Status**: in_progress
- **File**: crates/secall/src/commands/ingest.rs:603

## Description

run_post_ingest_hook 실패 시 tracing::warn만 출력하고 Ok(())로 진행합니다. hook 자체가 부가 기능이므로 의도적일 수 있으나, 사용자가 hook에 중요 로직(알림, 동기화 등)을 넣었을 때 실패를 인지하기 어렵습니다. 최소한 에러 카운트에 포함하거나 최종 리포트에 경고를 표시해야 합니다.

**Evidence**: `if let Err(e) = run_post_ingest_hook(config, &session, &abs_path, tz) {
    tracing::warn!(session = &session.id[..8.min(session.id.len())], error = %e, "post-ingest hook failed");
}`

## Snippet

```
if let Err(e) = run_post_ingest_hook(...) { tracing::warn!(...) }
```
