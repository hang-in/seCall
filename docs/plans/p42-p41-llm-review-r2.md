# Review Report: P42 — P41 후속 정리 + LLM 신뢰성 강화 — Round 2

> Verdict: fail
> Reviewer: 
> Date: 2026-05-09 09:55
> Plan Revision: 0

---

## Verdict

**fail**

## Recommendations

1. crates/secall-core/src/graph/semantic.rs:274,417 — `pub` 대신 `pub(crate)` 권장 (task 지시서의 "가시성 최소한 확장" 원칙). 현재는 외부 crate 에서도 접근 가능하나 기능 오류 없음.
2. crates/secall/src/commands/config.rs:259-590 — `test_backend` 함수가 300+ 줄 단일 함수. 가독성 개선 여지 있으나 본 plan 범위 외.
3. crates/secall-core/tests/rest_config.rs:241 — `assert!(saved.contains(r#"backend = \"haiku\""#))` 는 현재 테스트에서 위험 없으나, 향후 `default_backend = "haiku"` 같은 값이 toml 에 생기면 false-positive 가능. 정규식 또는 섹션 헤더 포함 substring 으로 강화 권장.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | log.rs 하드코딩 default 정리 | ✅ done |
| 2 | semantic.rs LLM 분기 단위 테스트 | ✅ done |
| 3 | `secall config llm test` 구현 | ✅ done |
| 4 | Settings UI 폴리싱 | ✅ done |
| 5 | REST `/api/config` 추가 회귀 | ✅ done |

