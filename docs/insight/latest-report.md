# Insight Report — 2026-04-18 04:34

총 15건 발견 — 안정성 (Stability): 8건, 기술 부채 (Technical Debt): 2건, 테스트 (Testing): 5건 ($0.896, 0in/0out)

## debt

- ⬜ **Known Issues가 CLAUDE.md에 인라인으로 관리됨** [minor] — CLAUDE.md
- ⬜ **Rework 절차에 반복 실패 방지 장치가 주석 수준으로만 존재** [info] — docs/agents/developer.md

## stability

- ⬜ **배치 모드: 파일 생성·병합 파이프라인 전체 누락** [critical] — crates/secall/src/commands/wiki.rs
- ⬜ **세션 링크가 Obsidian에서 깨진 링크로 생성됨** [major] — crates/secall-core/src/wiki/lint.rs
- ⬜ **검수(review)가 post-lint 최종본이 아닌 pre-lint 초안을 받음** [major] — crates/secall/src/commands/wiki.rs
- ⬜ **review 실패 시 자동 재시도 로직 없음** [major] — crates/secall/src/commands/wiki.rs
- ⬜ **증분 프롬프트에 기존 위키 페이지 목록 미주입** [major] — crates/secall/src/commands/wiki.rs
- ⬜ **사실 정확성 검수 시 원본 세션 내용 미전달** [major] — crates/secall/src/commands/wiki.rs
- ⬜ **세션 ID 중복 등장 시 첫 번째 이후 링크 치환 누락** [minor] — crates/secall-core/src/wiki/lint.rs
- ⬜ **페이지 간 내부 링크 생성 로직 미구현** [minor] — crates/secall-core/src/wiki/lint.rs

## test

- ⬜ **graph.rs 커맨드에 테스트 모듈 없음 (188 LOC)** [major] — crates/secall/src/commands/graph.rs
- ⬜ **graph build 시 파일 읽기/파싱 실패를 continue로 조용히 건너뜀** [major] — crates/secall/src/commands/graph.rs
- ⬜ **session_repo.rs — 722줄 핵심 파일에 테스트 모듈 없음** [major] — crates/secall-core/src/store/session_repo.rs
- ⬜ **classify.rs 커맨드에 테스트 모듈 없음** [minor] — crates/secall/src/commands/classify.rs
- ⬜ **SessionRepo trait에 신규 메서드 미반영** [minor] — crates/secall-core/src/store/session_repo.rs

