---
type: reference
status: in_progress
updated_at: 2026-07-02
---

# secall-core 백로그 / 알려진 이슈

> secall-core (Rust lib + CLI + REST API) 관련 미해결 / 추적 항목.
> web 전용 항목은 `web-backlog.md` 참조.

---

## hot

(현재 없음)

> 직전 해소: cargo test 가 production config.toml 을 덮어쓰는 회귀 — **P82 (PR 작성 중)** 에서 `Config::save()` 의 `#[cfg(test)]` 가드를 runtime env (`SECALL_TEST_MODE`) 로 확장해 integration test 까지 보호. 신규 `tests/config_save_guard.rs` 가 가드 동작 검증. 참조: [docs/plans/p82-config-save-guard.md](../plans/p82-config-save-guard.md).

## debt

- **wiki 생성 중단-재개(resume) 부재 — agentic 백엔드가 대량 배치에서 사용량 캡에 취약.**
  codex/claude 백엔드는 `--since`/배치 모드에서 **배치 전체를 단일 agentic 호출**로 처리(`process_generic_backend`, `commands/wiki.rs`). 대량 생성 시 (a) codex/claude 구독의 사용량 캡(예: codex 5h) 또는 (b) `wiki.generation_timeout_secs`(기본 1800s)에 걸려 중단됨. 중단 시 이미 쓴 페이지는 vault 에 남지만 **완료 세션을 추적하지 않아** 재실행이 처음부터 재작업(→ 다시 캡). 프롬프트는 "기존 페이지 보강"만 지시할 뿐 "완료 세션 skip"이 없음.
  → **fix**: generic 백엔드도 haiku 경로(`process_haiku_batch`)처럼 **세션/프로젝트 단위로 나눠 호출·저장**하고, "wiki-covered" 상태를 기록해 재실행이 완료분을 skip(진짜 resume). 각 호출이 작아져 캡/timeout 무관.
  → **즉효 우회(코드 무변경)**: `--since` 날짜 창으로 쪼개 실행 또는 `--session <id>` 세션 단위. (참고: ollama/ollama_cloud/lmstudio 는 생성 불가 — 도구 호출 능력 없어 fail-fast, review 전용.)

- **임베딩 무효화 키에 model_id 부재 (잠복 버그).** turn-vector 재임베딩 skip 판정(`search/vector.rs:99-102, 117-120`)이 `(turn_index, seq)` 청크 키만 보고 **model_id/차원을 비교하지 않음**. 임베딩 모델 교체 후 증분 `secall embed` 를 돌리면 옛 모델 벡터를 그대로 유지(stale) → 모델 교체 시 반드시 `embed --all` 필요. wiki 벡터(`wiki/indexer.rs:94`)는 `(content_hash, model_id)` 둘 다 비교하는데 turn-vector 만 누락. → fix: turn_vectors 무효화 키에 model_id(+dim) 포함(스키마 변경 동반).

- **Phase B — 검색 심화 (tunaRound 이식 잔여).** (1) **Kiwi Windows 활성화**: seCall 은 Windows 에서 kiwi-rs 를 cfg 로 배제 중인데 오진임 — kiwi-rs 0.1.4 는 순수 Rust 라 빌드됨. 실제 이슈는 런타임 자산(libkiwi.dll+모델). `KIWI_RS_VERSION=v0.22.2` 핀 또는 사전 설치 + base-tag POS 매칭(`VA-I`/`VV-R` 변종). Cargo target-cfg(secall-core/Cargo.toml) + `search/tokenizer.rs` 7곳 cfg 를 linux-aarch64 만 제외로 완화. (2) **raw-token FTS 색인**: `fts_index` 에 raw 토큰 추가(POS 탈락 외래어 보강) — FTS 재색인 동반. 현 PR #118 은 질의측(fts_query)만 반영(무재색인).

## watch

- **1293 복구불가 세션.** `reindex --from-vault --repair-missing-turns` 후 잔여(2399→1293). vault md 파일 부재 + 로컬 원본 로그 없음(`ingest --auto --force` 로 0건 치유 확인). 복구는 소스 머신에서 vault sync push 또는 vault git 히스토리 복원만 가능. parked.
- **Fable 위키 생성 부산물 정리.** 위키 재빌드 중 Fable(claude -p)이 MEMORY 훅 지시로 개인 메모리(`~/.claude/.../memory/gemento-paper-arxiv-first.md` + 루프 중 추가분)를 생성함 — 위키 에이전트 부산물이라 세션 메모리에서 검토·정리 필요.

---

## 처리 절차

1. 새 항목 발견 → 분류 (hot / debt / watch) 후 본 문서 해당 섹션에 추가
2. 항목 처리 시 별도 PR + 커밋 메시지에 본 문서의 항목 명시
3. PR 머지 후 본 문서에서 항목 제거 (또는 done 섹션으로 잠시 이동)
