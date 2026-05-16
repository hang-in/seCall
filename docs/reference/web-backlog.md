---
type: reference
status: in_progress
updated_at: 2026-05-16
---

# secall-web 백로그 / 알려진 이슈

> secall-web (`secall serve` 가 serve 하는 React + Vite SPA) 관련 미해결 / 추적 항목.
> GitHub Issues 트래커가 SSOT 가 아닌 동안 (사용자 운영 부담 고려) 이 markdown 이 단일 진실 원천.
> 발견 즉시 추가하고, 처리 완료 시 "완료" 섹션으로 이동 (또는 PR 머지 후 삭제).

---

## 분류

각 항목은 다음 4분류 중 하나:

- **🔥 hot**: 사용자 보고/관찰 기반의 명백한 결함 — 다음 작업 단위에 포함
- **🟡 debt**: 구조적/장기적 부채 — 시간 날 때 처리
- **🔵 watch**: 관찰 항목 — 현재는 문제 아니지만 회귀 가능성
- **✅ done**: 처리 완료 (다음 정리 사이클 때 삭제 후보)

---

## hot

(현재 없음 — P62 에서 `APP_VERSION` hardcode SSOT 통합으로 해결됨)

## debt

### dist 빌드 누락 회귀
- **위치**: `web/dist/` + `crates/secall-core/src/web/embed.rs` (`rust_embed`)
- **현상**: `web/dist/` 는 `rust_embed::RustEmbed#[folder = "../../web/dist/"]` 로 secall binary 에 build 시점 embedded. cargo 는 `web/dist/` 디렉토리 변경을 **자동 감지 안 함** — `pnpm build` 를 선행하지 않고 `cargo install --path crates/secall --force` 만 돌리면 옛 dist 스냅샷이 그대로 embed.
- **재현**: 2026-05-15 시점 `web/dist/index.html` 빌드 시각이 `2026-05-09 06:36`. P54 web redesign 머지 후 `cargo install` 만 돌린 환경에선 옛 번들 (v0.4.2 hardcode 포함) 이 serve 됨. 2026-05-16 P62 작업 중 발견.
- **현재 회피책**: release / install 절차에 `cd web && pnpm build && cd .. && cargo install --path crates/secall --force` 순서 명시. 단순 `cargo install` 만 돌리면 미반영.
- **후속 액션 후보**:
  - `crates/secall-core/build.rs` 에 `cargo:rerun-if-changed=../../web/dist/index.html` 추가해 dist 변경 시 cargo rebuild 트리거
  - 또는 `cargo xtask build` 류 wrapper 가 pnpm build + cargo build 묶어 실행
  - 또는 release 절차에 "pnpm build 후 cargo install" 체크리스트 박제

### `/api/status` 가 stats 쿼리와 묶임
- **위치**: `crates/secall-core/src/mcp/server.rs:do_status`
- **현상**: version 만 확인하려는 client 도 DB lock + stats 쿼리 (sessions/turns/vectors count) 비용을 지불.
- **후속 액션 후보**: client 가 version 만 필요한 경우용 cheap `/api/version` 신설, 또는 web 측 react-query cache (long stale time).
- **우선순위**: 현재 호출 빈도 낮음 (TopNav mount 시 1회). 회귀 모니터링 후 결정.

## watch

### server-side version 과 web build 시점 version 불일치
- **시나리오**: server 만 `cargo install --force` 로 새 버전 깔고 `secall serve` 띄우면 — server 의 `/api/status` 는 새 버전, web bundle (dist) 은 옛 빌드 시점에 박힌 자산 (CSS/JS layout). 표시되는 version 은 server 가 SSOT 이므로 일치하지만, 실제 화면 동작은 옛 dist 기준.
- **현재 상태**: `Build web/dist` CI job 이 PR 마다 빌드해 dist 가 src 와 sync 유지. 단 release tag 자동화는 없음 — 수동 release 시 dist 누락 가능.

---

## 처리 절차

1. 새 항목 발견 → 분류 (hot / debt / watch) 후 본 문서 해당 섹션에 추가
2. 항목 처리 시 별도 PR + 커밋 메시지에 본 문서의 항목 명시
3. PR 머지 후 본 문서에서 항목 제거 (또는 done 섹션으로 잠시 이동)
4. 분기 (월 1회 등) 마다 본 문서를 가볍게 청소
