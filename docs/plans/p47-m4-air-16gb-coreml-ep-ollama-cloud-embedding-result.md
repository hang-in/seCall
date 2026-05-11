# Implementation Result: P47 — M4 Air 16GB 임베딩 부담 완화 (CoreML EP + Ollama Cloud embedding)

> Developer: unknown
> Branch: N/A
> Date: 2026-05-12 05:24
> Plan Revision: 0

---

## Summary

전체 최종 verification:

```
Verification results (Rework):
✅ `cargo check -p secall-core` — 0 errors
✅ `cargo check -p secall` — 0 errors
✅ `cargo test -p secall-core --test rest_config` — 10 passed
✅ `cargo test -p secall-core --lib vault::config::tests` — 16 passed (신규 2개: pool_size round-trip, default=None)
✅ `cargo test -p secall-core --lib search::vector::tests` — 16 passed (신규 3개: resolve_pool_size 휴리스틱)
✅ `cargo test -p secall-core --lib search::embedding` — 6 passed
✅ `cd web && npm run typecheck` — 0 errors
✅ `npm test -- --run` — 9 passed (신규 4개: embedding 섹션 테스트)
```

수정 내역 요약:
- **Fix 1** (`embedding.rs:98`): `is_available()`이 `send().is_ok()` 대신 `resp.status().is_success()`를 확인하도록 변경 — HTTP 401/403/404는 이제 `false` 반환
- **Fix 2** (`SettingsRoute.tsx:108`): `sectionErrors.graph` 계산에 `graphForm.cloud_host` URL 검증 추가 — 잘못된 Cloud host 입력 시 Save 버튼 비활성화
- 권고 사항: `SettingsRoute.test.tsx`에 embedding 섹션 테스트 4개, `vault::config::tests`에 `pool_size` round-trip/default 테스트 2개, `vector::tests`에 `resolve_pool_size` 테스트 3개 추가

## Subtask Results

### 1. 전체 최종 verification:

```
Verification results (Rework):
✅ `cargo check -p secall-core` — 0 errors
✅ `cargo check -p secall` — 0 errors
✅ `cargo test -p secall-core --test rest_config` — 10 passed
✅ `cargo test -p secall-core --lib vault::config::tests` — 16 passed (신규 2개: pool_size round-trip, default=None)
✅ `cargo test -p secall-core --lib search::vector::tests` — 16 passed (신규 3개: resolve_pool_size 휴리스틱)
✅ `cargo test -p secall-core --lib search::embedding` — 6 passed
✅ `cd web && npm run typecheck` — 0 errors
✅ `npm test -- --run` — 9 passed (신규 4개: embedding 섹션 테스트)
```

수정 내역 요약:
- **Fix 1** (`embedding.rs:98`): `is_available()`이 `send().is_ok()` 대신 `resp.status().is_success()`를 확인하도록 변경 — HTTP 401/403/404는 이제 `false` 반환
- **Fix 2** (`SettingsRoute.tsx:108`): `sectionErrors.graph` 계산에 `graphForm.cloud_host` URL 검증 추가 — 잘못된 Cloud host 입력 시 Save 버튼 비활성화
- 권고 사항: `SettingsRoute.test.tsx`에 embedding 섹션 테스트 4개, `vault::config::tests`에 `pool_size` round-trip/default 테스트 2개, `vector::tests`에 `resolve_pool_size` 테스트 3개 추가

