---
type: baseline
status: draft
updated_at: 2026-05-05
plan_slug: p39-wiki-sync-auto-commit-fix
task_id: 01
sync_log: /tmp/sync-2026-05-03.log
sync_date: 2026-05-05
---

# P39 Wiki 파이프라인 Baseline (2026-05-05 sync)

> 본 baseline 은 **partial / hot-fixed sync** 결과 기반.
> - vault auto-commit 누락 → P39 Task 00 으로 fix (`crates/secall-core/src/vault/git.rs:146` `git add -A` 추가).
> - secall dotenv 자동 로드 부재 → P39 hot-fix 로 `crates/secall/src/main.rs:382` 에 `dotenvy::dotenv()` 삽입.
> - semantic graph 가 sync 본 phase 에서는 fallback (rules-only) 처리됨 → `secall graph rebuild --since 2026-05-05` 로 28 sessions / 840 edges 백필 완료.
>
> 따라서 본 보고서의 graph 수치는 **(sync 결과) + (rebuild 백필)** 합산임을 유의.

## 데이터 소스

| 소스 | 경로 | 비고 |
|---|---|---|
| sync log | `/tmp/sync-2026-05-03.log` | 407 lines, ~25 KB, tee 캡처 |
| sync wall-clock | 11:58:33 ~ 12:47:08 KST (UTC+0 표기) | log 첫/마지막 timestamp |
| graph backfill | `secall graph rebuild --since 2026-05-05` | 28 sessions / 840 edges added (사용자 보고) |
| vault | `~/Documents/Obsidian Vault/seCall` | read-only 측정 |

---

## 1. 처리 카운트

| 항목 | 값 | 출처 | 해석 |
|---|---|---|---|
| ingested (sessions) | 13 | log: `-> 13 ingested, 2590 skipped, 0 errors.` | 정상. 신규 세션만 처리. |
| skipped (sessions) | 2590 | 동상 | 누적 vault 의 기존 세션 (정상 dedup). |
| ingest errors | 0 | 동상 | 정상. |
| reindex new sessions | 0 | log: `0 new sessions indexed, 2941 skipped.` | vault 측 추가 분 없음 (이전 ingest 가 vault 작성 후 재스캔이라 정상). |
| wiki updates 시도 | 13 | `grep -c "^Wiki update: session"` | ingested 와 1:1. |
| wiki updates 완료 | 13 | `grep -c "✓ wiki updated for"` | 모두 성공 마크. 단, 1건은 사용자 권한 거부로 `wiki/` write 실제 발생 안 함 (세션 `6669dc04`). |
| wiki backend 분포 | claude=13 | `grep "backend:" \| sort \| uniq -c` | 단일 backend 사용. Gemini 백엔드 미사용 (사용자 환경). |
| graph nodes (sync 후 누적) | 3553 | log: `✓ graph: 3553 nodes, 10740 edges (682 sessions processed).` | 누적값. |
| graph edges (sync 후 누적) | 10740 | 동상 | 누적값. |
| graph sessions processed | 682 | 동상 | 누적값. |
| graph backfill (사후) | +28 sessions / +840 edges | 사용자 보고 (`graph rebuild --since 2026-05-05`) | hot-fix 후 LLM extraction 으로 재계산. |
| pulled | 0 (실패) | log: `Pull failed: ... 스테이징하지 않은 변경 사항이 있습니다` | **vault auto-commit 누락 사고** — Task 00 에서 fix. |
| pushed | 22 files | log: `-> 22 files pushed.` | 푸시 자체는 성공 (auto-commit 만 누락이라, sync 종료 시점에 Auto-committed pending → push). |
| Auto-committed pending | 1회 (sync 시작 시) | log line 1: `Auto-committed pending vault changes.` | sync **시작** auto-commit 만 동작. ingest/wiki 직후 commit 누락이 pull 실패의 원인. |
| partial_failure | yes | — | pull 실패 후 local-only 진행. push 는 결과적으로 성공했으나 remote→local 병합 누락 가능성. |

**해석**: 처리 카운트는 정상이나 git pull 실패는 Task 00 에서 수정한 auto-commit 누락 사고와 일치. semantic graph 는 sync 본 phase 에서는 rules-only 였고 backfill 로 보완됨.

**후속 액션 제안**:
- (이미 완료) Task 00 — `vault/git.rs:146` `git add -A` 추가.
- (이미 완료) hot-fix — `main.rs:382` dotenvy 자동 로드.
- 추가: sync 도중 LLM extraction fallback 발생 시 **종료 코드/요약에 명시 노출** (현재 WARN 만 흐르고 사용자 모름).

---

## 2. 시간 측정

| 항목 | 값 | 비고 |
|---|---|---|
| 시작 시각 | 2026-05-05T11:58:33Z | log 최초 WARN timestamp (pull 실패) |
| 종료 시각 | 2026-05-05T12:47:08Z | log 마지막 graph WARN timestamp |
| 총 elapsed | **약 48분 35초 (~2915s)** | end - start |
| 세션 단가 (ingested 13건 기준) | ~224s/session | total / 13 |
| 세션 단가 (push 22 files 기준) | ~132s/file | 참고용 |

### Phase 별 비율 (추정 — log 에 phase_start/phase_complete 마커 미존재)

| Phase | 추정 시간 | 추정 % | 근거 |
|---|---|---|---|
| pull (실패 폴백) | <5s | <1% | 첫 WARN 직후 reindex 시작 |
| reindex | <30s | <1% | 0 new sessions, 2941 skipped 만 — 가벼움 |
| ingest (13 sessions) | 수십초 ~ 수분 | <5% | LLM 호출 없음, BM25 + write |
| **wiki update (13 × claude CLI)** | **약 47분** | **>95%** | log 양 대부분이 wiki 출력. claude CLI subprocess 호출이 dominant cost. |
| graph (rules-only) | <1s | <1% | timestamp 12:47:07.95 → 12:47:08.02 (7 ms 간격으로 13 WARN 연속) — extraction 자체는 즉시 fallback. |
| push | <5s | <1% | 22 files |

**해석**: wiki update 가 wall-clock 의 95% 이상 차지. 세션당 평균 ~3.6분 (47분 / 13 sessions). claude CLI subprocess 의존도가 매우 높음. graph 는 fallback 으로 사실상 비용 0 (백필이 별도 비용 발생).

**후속 액션 제안**:
- sync 실행에 phase_start/phase_complete 마커 추가 — baseline 측정/회귀 감지가 어려움 (현재는 timestamp 추정).
- wiki 병렬 처리 가능성 검토 (현재는 직렬). 단 vault git 일관성 위험 있음.

---

## 3. 비용 추정

### LLM 호출 횟수

| 호출 종류 | backend | 호출 수 | 비고 |
|---|---|---|---|
| wiki update | claude (CLI subprocess) | 13 | sync 본 phase. 세션 1건당 1회 claude 실행. |
| semantic graph extract (sync 본 phase) | gemini → fallback rules-only | 13 시도 / 0 성공 | API 키 미로드로 모두 fallback. **API 비용 0**. |
| semantic graph extract (backfill) | gemini | ~28 호출 (사용자 보고) | hot-fix 후 `graph rebuild --since 2026-05-05`. |

### 비용 추정 (USD)

| 항목 | 단가 가정 | 호출 수 | 추정 비용 |
|---|---|---|---|
| claude CLI wiki update | claude-sonnet 기준 ~$3/M in, $15/M out. 세션당 평균 컨텍스트 ~30k in / ~5k out 가정 (보수적). | 13 | **~$2.10** (= 13 × (30k×$3/M + 5k×$15/M) = 13 × $0.165) |
| Gemini graph extract (backfill) | gemini-2.x flash 기준 ~$0.075/M in, $0.30/M out. 세션당 ~5k in / ~1k out 가정. | 28 | **~$0.02** (= 28 × ($0.000375 + $0.0003) ≈ $0.019) |
| Gemini graph extract (sync 본 phase) | — | 0 (fallback) | $0 |
| **합계** | | | **~$2.12 (±50% 정확도)** |

**해석**:
- 비용의 99% 가 wiki update (claude). graph 비용은 미미.
- 세션당 sync 비용 ~$0.16. 1,000 세션 누적 sync 시 ~$160 예상 (선형 가정 시).
- log 가 backend 별 토큰 수를 기록하지 않음 → 이 추정은 컨텍스트 가정에 강하게 의존. 실제와 ±50% 오차 가능.
- 이전 (5월 4일까지) 가 누적이었던 그래프 extraction 의 LLM 호출 비용은 본 sync 에 잡히지 않음.

**후속 액션 제안**:
- secall sync 가 backend 별 input/output token 누적치를 stdout 마지막에 요약 출력. (현재 ingest 단계의 token 표시는 conversation token 으로 LLM token 과 무관함을 명시할 것.)
- $/세션 모니터링용 phase 마커가 도움 됨 (Phase 2 측정과 공유).

---

## 4. Edge case 발생 빈도

| 항목 | 측정값 | 비고 |
|---|---|---|
| review-regen 발생 | **0** | `grep -c "Regenerating"` = 0. 본 sync 에 review pipeline 미실행으로 추정 (또는 모든 페이지 통과). |
| Lint 메시지 | **0** | `grep -c "Lint:"` = 0. 동상. |
| `Written:` 라인 | **0** | wiki 출력은 자유 형식 — `Written:` 표준 마커 미존재. 페이지 카운트는 claude 의 출력 텍스트로만 추정 가능. |
| merge conflict | **0** (sync 시점) | pull 실패는 dirty tree 때문이지 conflict 가 아님. |
| frontmatter 검증 실패 | **0** (보이지 않음) | log 에 frontmatter 검증 마커 없음 — 검증 자체 미실행 또는 silent pass. |
| vault 권한 거부 / claude 권한 미승인 | **1** (세션 `6669dc04`) | tail 부근 "권한 승인이 필요합니다. `ctx_batch_execute` MCP 도구..." → 사용자 승인 없어 실제 wiki write 안 함. 하지만 sync 는 `✓ wiki updated for 6669dc04` 로 성공 마크 → **silent failure mode**. |
| LLM extraction fallback (graph) | **13/13** (100%) | dotenv 미로드로 전부 rules-only. → hot-fix 후 graph rebuild 백필. |
| pull 실패 (dirty tree) | **1** | Task 00 fix 대상. |

**해석**:
- review/lint 카운터가 0인 것은 본 sync 가 review pipeline 을 거치지 않는 경로(단순 wiki update CLI)였음을 시사. P22 Wiki 파이프라인의 review/lint 자동 실행은 별도 트리거 필요할 수 있음 → 확인 필요.
- 가장 위험한 edge case 는 **wiki update 의 silent success** (세션 6669dc04): claude CLI 가 권한 미승인으로 실제 vault 변경 0이지만 sync 는 성공 처리. 사용자가 인지 못함.
- graph fallback 100% 가 가장 큰 사고. dotenv hot-fix 로 차단했으나 회귀 위험 있음.

**후속 액션 제안**:
- wiki update 의 actual write 검증: claude CLI 종료 후 git status / mtime 으로 vault 실제 변경 확인 후 success 마크.
- review-regen / lint 가 정말 미작동인지 확인 (sync 경로에 P22 review pipeline 통합 여부 확인).
- LLM extraction fallback 발생 시 사용자에게 명시적 종료코드 또는 요약 라인 출력.

---

## 5. 안정성 issue

### 5.1 vault auto-commit 누락 (해결: P39 Task 00)
- 증상: sync 시작 시 `Auto-committed pending vault changes.` 한 번 동작. ingest/wiki 후 추가 commit 없이 곧장 reindex / wiki write → 다음 sync 의 pull 단계에서 dirty tree 로 `git pull --rebase` 실패.
- 영향: 본 sync 의 첫 단계 pull 실패. 다행히 push 는 성공해서 remote 가 갱신됐으나, **다음 sync** 가 또 dirty tree 로 시작할 위험.
- 위치: `crates/secall-core/src/vault/git.rs:146` (`git add -A` 누락된 commit 경로).
- 조치: P39 Task 00 에서 `git add -A` 추가 commit 으로 fix.

### 5.2 secall dotenv 자동 로드 부재 (해결: hot-fix)
- 증상: `.env` 의 `SECALL_GEMINI_API_KEY` 가 secall 프로세스에 미주입 → 13건 모두 graph LLM extraction fallback (rules-only).
- 영향: 본 sync 의 graph 데이터 품질 저하. hot-fix 후 별도 `secall graph rebuild --since 2026-05-05` 실행으로 28 sessions / 840 edges 백필 완료.
- 위치: `crates/secall/src/main.rs:382`.
- 조치: `dotenvy::dotenv()` 호출 추가.
- 잔여 위험: hot-fix 에 회귀 테스트 없음. CI 에 dotenv 로드 smoke test 추가 권장.

### 5.3 wiki update silent success (미해결, 신규 이슈 후보)
- 증상: 세션 `6669dc04` — claude CLI 가 사용자 권한 승인 대기 중 응답 후 종료. vault `wiki/` 변경 없음에도 sync 는 `✓ wiki updated for 6669dc04` 로 성공 처리.
- 영향: wiki 누락이 사용자에게 보고되지 않음. 누적 시 wiki gap 발생.
- 조치 후보: claude CLI 종료 후 vault git status / file mtime 으로 변경 확인 검증 추가.

### 5.4 graph extraction 실패 시 silent fallback (미해결)
- 증상: API 키 미로드 시 WARN 로그만 남기고 rules-only 로 진행. sync 종료 메시지에 fallback 발생 사실 미표시.
- 영향: 사용자가 graph 품질 저하를 자력으로 인지하기 어려움.
- 조치 후보: sync 종료 시 `LLM fallback occurred: 13 sessions` 같은 요약 한 줄 추가, 또는 `--strict` 모드에서는 비0 종료.

### 5.5 phase 가시성 부족 (미해결)
- 증상: log 에 phase_start/phase_complete 마커 없음. baseline 측정이 wall-clock 추정에 의존.
- 영향: 회귀 감지/성능 모니터링 어려움.
- 조치 후보: 각 phase entry/exit 에 `phase_start name=...`, `phase_complete name=... duration_ms=...` 출력.

### 5.6 panic / 강제 종료
- 없음. log 에 panic / unwrap 흔적 없음. 모든 실패가 graceful (WARN + 진행).

---

## 종합 (해석 한 줄)

본 sync 의 핵심 회귀는 (1) vault auto-commit 누락 → pull 실패, (2) dotenv 미로드 → graph 100% fallback. 두 사고 모두 P39 에서 fix 완료/백필 완료. wiki update 비용/시간이 sync 의 dominant cost (~95% wall-clock, ~99% USD). silent success / silent fallback 두 가지가 차순위 위험.
