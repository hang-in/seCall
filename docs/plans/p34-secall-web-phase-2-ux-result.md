# Implementation Result: P34 — secall-web 뷰어 본격 강화 (Phase 2: 탐색 깊이 + UX)

> Developer: unknown
> Branch: N/A
> Date: 2026-05-02 21:21
> Plan Revision: 0

---

## Summary

## Rework 완료 — 4건 모두 해결

**수정 항목:**
| # | 파일 | 수정 내용 |
|---|---|---|
| 1 | `crates/secall-core/src/mcp/rest.rs:283` | `tags: Option<String>` → `Option<Vec<String>>`, axum 기본 `Query` → `axum_extra::extract::Query` (serde_html_form, 반복 키 수용), `From` impl에서 콤마 split도 처리 → 반복·콤마·혼합 3형태 모두 지원 |
| 2 | `web/src/components/SessionList.tsx:69` | `useListHotkeys(hotkeyItems, id, navigate)` 등록 — semantic/keyword 모드 모두에서 j/k/Enter/[/] 활성 |
| 3 | `web/src/components/RelatedSessions.tsx:35` | `encodeURIComponent(it.id)` 적용 |
| 4 | `web/src/components/GraphOverlay.tsx:51` | `encodeURIComponent(nodeId)` 적용 |

**의존성 추가:** `axum-extra = "0.10" features=["query"]` (workspace + secall-core)

**Verification results:**
```
✅ cargo check --all-targets — exit 0
✅ cargo clippy --all-targets --all-features — 0 warnings
✅ cargo fmt --all -- --check — clean
✅ cargo test mcp::rest::tests — 4 passed (신규 HTTP 파싱 회귀 테스트)
   - list_query_parses_repeated_tags
   - list_query_parses_comma_delimited_tags
   - list_query_parses_mixed_repeat_and_comma_tags
   - list_query_drops_empty_and_whitespace_tag_entries
✅ cargo test rest_listing — 1 passed (기존 multi_tag_and 보존)
✅ pnpm typecheck — exit 0
✅ pnpm build — exit 0 (978.25 kB)
```

`recommendations`의 "공통 헬퍼"는 별도 리팩터로 분리 (이번 rework는 정확성 fix에 집중). grep 결과 `/sessions/${...}` 5개 위치 모두 `encodeURIComponent` 적용됨.

## Subtask Results

### 1. ## Rework 완료 — 4건 모두 해결

**수정 항목:**
| # | 파일 | 수정 내용 |
|---|---|---|
| 1 | `crates/secall-core/src/mcp/rest.rs:283` | `tags: Option<String>` → `Option<Vec<String>>`, axum 기본 `Query` → `axum_extra::extract::Query` (serde_html_form, 반복 키 수용), `From` impl에서 콤마 split도 처리 → 반복·콤마·혼합 3형태 모두 지원 |
| 2 | `web/src/components/SessionList.tsx:69` | `useListHotkeys(hotkeyItems, id, navigate)` 등록 — semantic/keyword 모드 모두에서 j/k/Enter/[/] 활성 |
| 3 | `web/src/components/RelatedSessions.tsx:35` | `encodeURIComponent(it.id)` 적용 |
| 4 | `web/src/components/GraphOverlay.tsx:51` | `encodeURIComponent(nodeId)` 적용 |

**의존성 추가:** `axum-extra = "0.10" features=["query"]` (workspace + secall-core)

**Verification results:**
```
✅ cargo check --all-targets — exit 0
✅ cargo clippy --all-targets --all-features — 0 warnings
✅ cargo fmt --all -- --check — clean
✅ cargo test mcp::rest::tests — 4 passed (신규 HTTP 파싱 회귀 테스트)
   - list_query_parses_repeated_tags
   - list_query_parses_comma_delimited_tags
   - list_query_parses_mixed_repeat_and_comma_tags
   - list_query_drops_empty_and_whitespace_tag_entries
✅ cargo test rest_listing — 1 passed (기존 multi_tag_and 보존)
✅ pnpm typecheck — exit 0
✅ pnpm build — exit 0 (978.25 kB)
```

`recommendations`의 "공통 헬퍼"는 별도 리팩터로 분리 (이번 rework는 정확성 fix에 집중). grep 결과 `/sessions/${...}` 5개 위치 모두 `encodeURIComponent` 적용됨.

