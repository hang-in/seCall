---
type: reference
status: in_progress
updated_at: 2026-07-06
---

# Handoff 2026-07-06 (2nd session) — v0.7.0 릴리즈 + 파싱 리뷰 + 임베딩 가드

아주 긴 세션. web-shadcn-pilot 머지 → 파싱 서브시스템 전면 리뷰(3 PR) → v0.7.0 릴리즈 →
정크 정리/재설치 → 임베딩 모델 불일치 가드까지.

## 1. 이번 세션 머지 (main)

| PR | 내용 |
|---|---|
| #137 | web-shadcn-pilot — Sessions Linear/반응형 + 렌더/좌측패널(정렬·달력·필터). **do_get HIGH 회귀 잡음**(셀프 코드리뷰 11건) |
| #138 | 파싱 Stage 1 — subagents 워크플로 정크 세션 ingest 차단(#1 walker + #2 turns guard + #8 codex dedup) |
| #139 | 파싱 Stage 2 — 데이터무결성(토큰 이중계산·병렬tool 유실·codex 귀속·gemini ZIP·YAML drop·비ASCII panic) |
| #140 | 파싱 Stage 3 — MED/LOW 폴리시(codex 주입컨텍스트·gemini/chatgpt 파싱·ANSI strip·display panic) |
| #141 | **v0.7.0 릴리즈** (버전 bump + CHANGELOG + README bge-m3→qwen 정정) → 태그 v0.7.0 발행(3-플랫폼 바이너리) |
| #142 | 임베딩 모델 불일치 가드 (진행 중 — CI/머지 대기) |

**파싱 리뷰**: 적대적 워크플로 7차원 → **23건 confirmed → 21건 반영**. `docs/reference/parsingReviewFindings.md` 참고.

## 2. v0.7.0 릴리즈 상태

- 태그 `v0.7.0` @ main, GitHub Release 발행 완료(Latest). assets: darwin arm64/x64 + windows x64.
- 설치본 `D:\.cargo\bin\secall.exe` = **공식 v0.7.0** (release zip 직접 설치). `secall --version` → 0.7.0.
- **⚠ 기존 사용자 임베딩 마이그레이션**: 기본 임베딩 bge-m3→qwen(#120). 기존 bge-m3 벡터 사용자는
  업그레이드 시 교차모델 불일치로 시맨틱 검색 조용히 저하 → `secall embed --all` 또는
  `config set embedding.ollama_model bge-m3`. #142 가드가 이걸 감지·경고. README(ko/en) 안내 추가됨.

## 3. 다음 세션 후보 (backlog)

1. **#142 머지 마무리** — CI green + CodeRabbit 확인 후 머지 (진행 중이었음).
2. **임베딩 가드 sync-path 경고** — 현재 embed/status 만 경고. `secall sync` 의 ingest embed 경로에도 연결(sync만 쓰는 사용자).
3. **파싱 #14/#15 재설계** — markdown parse_session_turns 의 phantom turn / unclosed fence.
   Stage 3 에서 드롭(제안된 render-이스케이프가 기존 vault reindex 백워드호환 깨짐). fence 추적 유지하며 별도 설계 필요.
4. **자동 sync 스케줄러** — `docs/plans/schedulerSyncPlan_2026-07-06.md` (경량 자주 `--no-wiki --no-graph` + 나이틀리 full, git 포함).
   codex 5h 쿼터 문제 때문에 실질 유용. 선행조건(subagents fix)은 #138 로 해결됨.
5. **커뮤니티 공지** — 0.4.0 이후 v0.7.0 소개 초안은 사용자가 정리 중(높임말 톤). 웹UI 스크린샷 추가 여지.
6. **codex fast_dedup_key ↔ #20 정합** — #140 후속(fast-path 최적화, PR #140 body 참고). 이미 #140 리뷰반영으로 처리됨(확인).

## 4. 인프라 상태 (중요)

- **DB**: `%LOCALAPPDATA%\secall\index.sqlite` (**.db 아님!**, ~2.83GB, WAL). memory `secall_db_path` 참고.
  경로 규칙 `dirs::cache_dir()/secall/index.sqlite`, `SECALL_DB_PATH` env override.
- **8080 serve**: v0.7.0 로 실행 중(캡처/개발용). **web dev vite: localhost:5180**(현재 web/ 소스, 8080 프록시).
- **임베딩 백엔드**: qwen3-embedding:0.6b, `embedding.ollama_url=http://127.0.0.1:11435`(원격 boxie GPU ssh 터널).
- **MCP**: 설치 위해 종료함. 다른 세션에서 필요 시 재기동 (`secall mcp`).
- **정크 정리**: journal 세션(workflow journal.jsonl 오인) DB 1행 + vault MD 6개 삭제 완료. sessions 6636→6635.
  vault git 삭제는 working-tree 스테이징 상태 → 다음 sync push 로 원격 정리.

## 5. 규명/학습 (재작업 방지)

- **do_get 회귀 패턴**: `server.rs do_get(full=true)` 재작성/위임 시 `resolve_session_file` 가 자주 빠져
  tool-use 렌더가 빈 `## assistant` 로 저하. memory `knowledge_do_get_resolve_regression`. #135/#137 에서 두 번 회귀.
- **임베딩 교차모델**: 차원 같으면(1024) 에러 없이 유사도만 무의미 → silent 저하. #121(위키)·#142(세션) 동일 메커니즘.
- **자동 진행 정책**: 사용자가 파싱 스테이지 머지를 "자동 진행" 승인(CI green + CodeRabbit HIGH 없으면 머지+다음 착수).
- **CI 트리거**: 브랜치가 base(main)와 충돌하면 GitHub 이 pull_request CI 를 안 돌림 → 머지 안 됨. main 머지로 해소.
- **워크플로 병렬 수정**: 파일 단위 disjoint 로 에이전트 병렬 → 중앙에서 fmt+clippy+test 일괄 검증(간섭 방지). raw-string 델리미터 충돌(`r#"` vs `"#`) 주의.
