# Review Report: P43 — Wiki review backend 확장 + 로컬 전용 wiki 파이프라인 완성 — Round 3

> Verdict: fail
> Reviewer: 
> Date: 2026-05-09 11:17
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall/src/commands/wiki.rs:926 — `resolve_review_model()`의 기본값이 여전히 `"sonnet"`인데, `build_reviewer()`는 이 값을 `HaikuReviewer`에 그대로 넘깁니다. 그래서 `--review-backend haiku`를 기본 설정으로 쓰면 Anthropic 요청의 `model`이 `"sonnet"`으로 전송되어 유효한 Haiku 모델 ID가 아니게 됩니다. `haiku` backend가 기본값만으로 동작하지 않아 Task 03의 dispatcher가 실사용에서 깨집니다.
2. crates/secall-core/src/vault/config.rs:443 — `sync_section()`은 직렬화된 키만 upsert하고, 현재 `Config`에 없는 키는 삭제하지 않습니다. 그 결과 기존 파일에 있던 optional 키(`wiki.review_backend` 등)를 `None`으로 되돌린 뒤 `save()`해도 파일에서 제거되지 않아, 저장 결과가 메모리 상태와 불일치합니다.
3. crates/secall-core/src/vault/config.rs:472 — `toml_value_to_item()`가 모든 `toml::Value::Table`을 `InlineTable`로 바꿔 저장합니다. `wiki.backends`처럼 원래 `[wiki.backends.lmstudio]` / `[wiki.backends.ollama]` 하위 테이블로 쓰는 설정은 저장 시 `backends = { ... }` 형태로 접혀 버려 기존 섹션 구조와 주석/배치를 보존하지 못합니다. Task 05의 “주석/구조 보존” 목표와 충돌합니다.

## Recommendations

1. `haiku` review backend에는 `review_model` 기본값을 별도 해석하거나, `sonnet`/`opus` 같은 legacy alias를 Haiku용 Anthropic model ID로 매핑하지 말고 명시적으로 검증해서 친절한 에러를 반환하세요.
2. `Config::save()`는 섹션 merge 시 누락된 optional 키 삭제 경로를 추가하고, nested table은 inline table로 축약하지 말고 `toml_edit::Table` 구조를 유지하는 방식으로 갱신하는 편이 안전합니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | WikiReviewer trait 도입 | ✅ done |
| 2 | 5 backend reviewer 구현 | ✅ done |
| 3 | config + CLI 통합 | ✅ done |
| 4 | review prompt 외부화 | ✅ done |
| 5 | toml_edit 도입 | ✅ done |
| 6 | P42 review recommendations | ✅ done |
| 7 | Documentation | ✅ done |

