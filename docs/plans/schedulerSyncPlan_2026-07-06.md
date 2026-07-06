---
type: plan
status: draft
updated_at: 2026-07-06
---

# secall 자동 sync 스케줄러 계획

수동 `secall sync` 를 Windows Task Scheduler 로 자동화. 기존 `secall-embed-tunnel`
(임베딩 SSH 터널) 패턴을 답습한다.

## 확정된 결정

- **주기·범위**: 경량 자주 + 나이틀리 full (2단 분리)
  - 경량: `secall sync --no-wiki --no-graph` — pull→reindex→ingest→**embed**→push (검색만 최신화, 빠름)
  - 나이틀리: `secall sync` (full) — wiki + graph LLM 심화 (gemma4:31b-cloud) 포함
- **git**: 포함 (cross-machine — 맥/윈도우 vault 공유). 경량·나이틀리 모두 git pull/push.

## 선행 조건 (blocking)

**파싱 리뷰(진행 중, `wwon4bimm`)로 subagents 정크 버그부터 수정·머지.**
- 현재 ingest 가 `.claude/projects/**/subagents/**/*.jsonl`(889개 workflow 산출물)을
  세션으로 오인 → 매 sync 마다 정크 처리 + 로그 도배. 스케줄 자동화 전에 반드시 제거.
- 순서: ① 파싱 fixes 머지 → ② 스케줄러 구성.

## 아키텍처

### 1. 래퍼 스크립트 `C:\Users\사자\secall-sync.ps1`

`secall-embed-tunnel.ps1` 패턴 답습:

- `param([ValidateSet('light','full')] $Mode = 'light')`
- **PATH**: Git 도구 선주입 (`C:\Program Files\Git\usr\bin;...`) — ssh/git 확보.
- **중복 실행 방지**: lock 파일(`$env:TEMP\secall-sync.lock`) — 이미 실행 중이면 로그 남기고 즉시 종료.
  (Task Scheduler 의 "do not start new instance" 와 이중 방어.)
- **터널 프리플라이트**: `:11435` 도달 확인. 안 되면 경고 로그 (임베딩만 실패, git+ingest 는 진행).
- **git SSH**: 필요 시 `GIT_SSH_COMMAND="ssh -F none -i C:/Users/사자/.ssh/id_ed25519"`
  (embed-tunnel 과 동일 키 — 무인 인증). vault 리모트가 SSH 라는 전제 → 롤아웃 시 확인.
- **실행**:
  - light: `secall sync --no-wiki --no-graph`
  - full:  `secall sync`
- **로그**: `C:\Users\사자\secall-logs\sync-YYYYMMDD.log` 로 stdout+stderr 리다이렉트,
  타임스탬프(`Get-Date -Format o`). N일 지난 로그 prune.
- **종료코드 캡처**: 실패 시 로그에 명확히 표시(+선택: 알림). lock 해제(finally).

### 2. Task Scheduler 태스크 2개

| 태스크 | 트리거 | 액션 | 설정 |
|---|---|---|---|
| `seCall-SyncLight` | 로그온 + 4~6h 반복 | `pwsh -File secall-sync.ps1 -Mode light` | 실행 중이면 새 인스턴스 시작 안 함 |
| `seCall-SyncNightly` | 매일 ~04:00 | `pwsh -File secall-sync.ps1 -Mode full` | 동일 |

- **로그온 세션 전용**으로 실행(무인 로그오프 상태 X): git SSH 키·임베딩 터널
  (secall-embed-tunnel 도 로그온 트리거)이 사용자 세션에 의존하므로 단순·안전.
  로그오프 중 sync 는 트레이드오프로 포기(개발 워크스테이션이라 허용).

## 고려사항

- **동시성(8080 serve / MCP 와)**: SQLite WAL — 리더 다수 + 라이터 1. sync 가 라이터.
  8080 serve/MCP 가 write(삭제/태그) 시 잠깐 lock 경합 가능하나 재시도로 흡수. 허용.
- **git 충돌**: `secall sync` 에 충돌 preflight 존재(P15). 무인 중 충돌 시 push 스킵 +
  로그 → 사용자가 나중에 해결. 브로큰 merge 상태로 방치되지 않는지 롤아웃 시 검증.
- **WikiDeepLoop 와 분리**: 나이틀리 full 의 wiki 는 "신규 세션 incremental". 기존
  `seCall-WikiDeepLoop`(기존 위키 Opus 심화, 현재 Disabled)와 별개 — 충돌 없음.
- **비용**: 나이틀리 full 만 LLM(그래프 gemma4:31b-cloud, 위키). 경량은 임베딩(로컬 GPU)뿐.

## 롤아웃 단계

1. (선행) 파싱 fixes 머지.
2. `secall-sync.ps1` 작성 → **수동 테스트**: `-Mode light` 로 1회(로그·lock·git·exit 확인),
   이어서 `-Mode full` 1회.
3. vault 리모트 인증 방식 확인(SSH 키 무인 동작?) — 필요 시 `GIT_SSH_COMMAND` 조정.
4. 두 태스크 등록(`Register-ScheduledTask` 또는 XML import). "새 인스턴스 시작 안 함" 설정.
5. 트리거 강제 실행(`Start-ScheduledTask`)으로 스케줄 경로 검증 — 로그 + DB 갱신 확인.
6. 하루 관찰 후 주기 튜닝.

## 미확정/검증 항목

- vault git 리모트 인증(SSH id_ed25519 무인?) — 롤아웃 2~3 단계에서 확인.
- 경량 sync 반복 간격(4h vs 6h) — 사용 패턴 보고 결정.
- 실패 알림 채널 필요 여부(로그만 vs Windows 알림/파일 flag).
