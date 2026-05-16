---
type: reference
status: in_progress
updated_at: 2026-05-16
---

# secall-core 백로그 / 알려진 이슈

> secall-core (Rust lib + CLI + REST API) 관련 미해결 / 추적 항목.
> web 전용 항목은 `web-backlog.md` 참조.

---

## hot

### cargo test 가 production config.toml 을 덮어쓰는 회귀
- **위치**: `crates/secall-core/src/vault/config.rs#tests` (`save_*` 류 테스트)
- **현상**: `save_preserves_top_level_comments`, `save_removes_optional_keys_when_cleared` 등이 `Config::save()` 를 호출. `SECALL_CONFIG_PATH` env 를 tempdir 로 set 한 상태에서 호출하지만, 다른 테스트와 race / 또는 panic 으로 인한 unwind 시 `SECALL_CONFIG_PATH` 가 unset 된 채 save 가 실행되면 **production config (`~/Library/Application Support/secall/config.toml`) 를 덮어씀**.
- **재현 (2026-05-16)**: P58 race fix 머지 전 vault::config flaky test 재현 시도 (5회 반복) 도중 한 시점에 production config.toml 의 `[vault].path` 가 `/tmp/changed` (`save_preserves_top_level_comments` 의 hardcoded 값) 로 변경됨. 사용자가 web UI 에서 wiki 빈 화면 / graph 멈춤으로 인지.
- **위험도**: HIGH — 사용자의 실 환경을 깨뜨리는 부수효과. 단 P58 race fix 머지 후엔 race 자체는 차단.
- **남은 위험**: panic / 강제 종료 시 `SECALL_CONFIG_PATH` unset → save 가 production 으로 흘러갈 가능성. 테스트가 `Config::save()` 를 호출하는 한 구조적 위험 잔존.
- **후속 액션 후보**:
  1. `Config::save()` 가 `SECALL_CONFIG_PATH` 이 set 되어 있지 않으면 **production path 대신 명시 에러 또는 noop** 으로 동작하는 `#[cfg(test)]` 가드 추가
  2. 또는 test 만의 `Config::save_to_path(&path)` 헬퍼 신설해 save() 호출 자체 회피
  3. 또는 `serial_test` crate 도입해 env mutation 테스트 전체를 process-level serial 화 (현 ENV_MUTEX 는 panic 복구 불가)

## debt

(현재 없음)

## watch

(현재 없음)

---

## 처리 절차

1. 새 항목 발견 → 분류 (hot / debt / watch) 후 본 문서 해당 섹션에 추가
2. 항목 처리 시 별도 PR + 커밋 메시지에 본 문서의 항목 명시
3. PR 머지 후 본 문서에서 항목 제거 (또는 done 섹션으로 잠시 이동)
