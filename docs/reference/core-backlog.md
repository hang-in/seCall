---
type: reference
status: in_progress
updated_at: 2026-05-19
---

# secall-core 백로그 / 알려진 이슈

> secall-core (Rust lib + CLI + REST API) 관련 미해결 / 추적 항목.
> web 전용 항목은 `web-backlog.md` 참조.

---

## hot

(현재 없음)

> 직전 해소: cargo test 가 production config.toml 을 덮어쓰는 회귀 — **P82 (PR 작성 중)** 에서 `Config::save()` 의 `#[cfg(test)]` 가드를 runtime env (`SECALL_TEST_MODE`) 로 확장해 integration test 까지 보호. 신규 `tests/config_save_guard.rs` 가 가드 동작 검증. 참조: [docs/plans/p82-config-save-guard.md](../plans/p82-config-save-guard.md).

## debt

(현재 없음)

## watch

(현재 없음)

---

## 처리 절차

1. 새 항목 발견 → 분류 (hot / debt / watch) 후 본 문서 해당 섹션에 추가
2. 항목 처리 시 별도 PR + 커밋 메시지에 본 문서의 항목 명시
3. PR 머지 후 본 문서에서 항목 제거 (또는 done 섹션으로 잠시 이동)
