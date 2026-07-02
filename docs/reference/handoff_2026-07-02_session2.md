---
type: reference
status: draft
updated_at: 2026-07-02
canonical: true
supersedes: handoff_2026-07-02.md
---

# seCall 세션 핸드오프 — 2026-07-02 (session2, Windows)

> 이어지는 세션. 오전(session1, `handoff_2026-07-02.md`)의 zero-turn healing(PR #115)
> 이후, 이 세션은 **tunaRound/memora 조사 → 검색 이식 + wiki claude 모델 pass-through +
> 위키 5-카테고리 전면 재빌드(Fable 5)**를 진행. **위키 재빌드가 아직 진행 중**(5h 캡으로
> 분할 실행)이라 아래 §2 resume 이 최우선 맥락.

---

## 1. 현재 상태 (cold-start)
- 브랜치 `main`. 머지됨: #115(healing), #116(README/api), **#119(wiki claude model pass-through)**.
- **Open PR: #118** (`feat/search-loanword-fts`) — tunaRound 검색 이식(음역 alias + OR/prefix 질의) + core-backlog debt/watch 기록. 리뷰/머지 대기.
- 설치 바이너리: `secall` **v0.6.4** (D:\.cargo\bin, #119 pass-through 포함). `claude -p` headless 정상(Max 200, CLI 2.1.198) — `--model sonnet`/`fable` 둘 다 확인됨.
- config(`%APPDATA%\secall\config.toml`): `[wiki] generation_timeout_secs = 5400` 추가함. embedding=ollama qwen3-embedding:0.6b.
- 운영 DB: healing 완료(3142→6403 세션, zero-turn 2399→1293, 임베딩 344,743 청크). **남은 1293은 vault 파일 부재로 복구불가**(watch).

## 2. ⚠️ 진행 중: 위키 5-카테고리 재빌드 (RESUME 필수)
**무엇**: vault `wiki/`를 5-카테고리(**projects/topics/decisions/tools/workflows**)로 fresh 재빌드. SCHEMA.md(vault, 커밋됨) + 런타임 프롬프트 오버라이드로 구동. 모델 **Fable 5**(초기), 이후 유지는 Sonnet 5.
- **완료(deep)**: gemento(206), seCall(252), tunaFlow(201). pass-1 얇은 페이지들도 존재.
- **남은 15개 프로젝트 심화 대기**: tunaInsight, tunaflow-mobile, dsp_cad_gcs, tunaRound, tunaChat, tunapi, tunaReader, tunaSalon, tunaDish, tunaLlama, dbox_cad_gcs, destinyTribe, solgrbit, takopi-discord, sshc.
- **제약**: Claude **5h 롤링 윈도우 캡** → 윈도우당 대형 Fable 패스 **~3개**만. (주간예산 아님.) 리셋 후 이어감.
- **RESUME 방법**: `bash <scratchpad>/deep_resume.sh` — 완료분(`deep_done.txt`) skip + 캡 만나면 즉시 중단. 캡이면 재예약(5h 리셋 대기), 진행되면 계속. `deep_loop.log`에 진행. (scratchpad = `C:\Users\사자\AppData\Local\Temp\claude\D--privateProject-seCall\3fa42ce8-4d01-4cbf-96c3-3c6f085242b1\scratchpad`; 프롬프트 템플릿 `deep_template.md`, `SECALL_PROMPTS_DIR` 오버라이드 사용, `--no-pull`.)
- **백업/복원**: 새 위키 → `scratchpad/wiki_new_fable_2026-07-02`. 옛 위키 → **origin/main에 tracked**(복원: `git -C vault checkout origin/main -- wiki/`) + 로컬 태그 `wiki-pre-rebuild-2026-07-02`(SCHEMA만). **vault의 wiki/는 로컬 HEAD에서 미추적 상태**(fragile) — 확정 시 `git add wiki/` 로 추적 시작 필요.
- **위키 완주 후**: 옵시디언 검토 → vault git 커밋/push(+untracked 정정) → **repo `docs/prompts/wiki-update.md`를 5-카테고리로 영구화(코드 PR)** — 지금은 런타임 오버라이드에만 있어 다른 머신 미적용.

## 3. 우선순위 순서 (backlog)
1. **(자동) 위키 심화 루프 완주** — §2. deep_resume.sh + 예약 재개. 사람 개입 최소.
2. **Task C — model discovery 고도화** [지금 병행 최적, Rust라 claude 캡 무관]: `secall config llm list [--backend][--refresh]` CLI + `KNOWN_BACKENDS` 상수 + claude id parse/trim 이식(tunaFlow `src-tauri/src/commands/model_discovery.rs` 참고) + REST `/api/models` 전백엔드. (조사 완료, 하드코딩 지점: `llm/defaults.rs`, `wiki/review.rs`.)
3. **PR #118 머지** (리뷰 후) — 검색 이식 + backlog 기록 반영.
4. **위키 확정 마무리** — §2 완주 후 커밋/push + repo 프롬프트 영구화.
5. **Phase B** — Kiwi Windows 활성화 + raw-token FTS 색인 (core-backlog debt).
6. **임베딩 model_id 무효화 키 수정** (잠복 버그, 스키마 변경).
7. **wiki resume 구조 개선** — generic 백엔드 per-session 구동(5h 캡 근본 fix). 지금 겪는 문제.
8. **(parked) 1293 복구불가 세션** — 소스 머신 sync/git 히스토리.
9. **(chore) Fable 생성 메모리 파일 정리**.

## 4. 핵심 결정·교훈 (이번 세션)
- **secall 자체 검색(BM25+ollama 벡터)은 정상.** 매 프롬프트 상단의 `⚠ Semantic layer: vec-daemon/bge-daemon down`은 **tunaFlow rawq 레이어**(별개)지 secall 아님. Fable이 이걸 오인해 첫 위키 패스를 BM25-only로 돌려 커버리지가 얕았음(gemento 통째 누락 등) → **프로젝트별 hybrid recall 심화 패스**로 해결(gemento 0→206줄/20소스 검증).
- **위키 5-카테고리 근거**: Tobi Lütke "LLM Wiki"(Karpathy 아님)는 컴파일 철학만 주고 taxonomy는 미처방 → "지식 종류별 + 파일링 규칙 명확"으로 재설계(topics 잡탕을 tools/workflows로 분리).
- **모델**: 초기 클린 빌드=Fable 5(복리 효과), 이후 증분=Sonnet 5. claude 백엔드는 model 문자열 pass-through(#119)라 alias/full-id 자유.
- **wiki 생성 백엔드**: ollama/ollama_cloud/lmstudio는 **생성 불가**(도구 호출 없어 fail-fast, review 전용). 생성=claude/codex/haiku만.
- **`ingest --force`는 벡터 삭제** → `--no-embed`와 조합 시 stale (memory: knowledge_ingest_force_wipes_vectors).

## 5. 작업 방식
- 검증 게이트: `cargo fmt --check` + `clippy --workspace --all-targets -D warnings` + `cargo test --workspace`. 검증/push 분리.
- Windows 빌드: `web/dist` 선행(`cd web && pnpm build`). `cargo install --path` 은 실행 중 secall.exe(MCP) 잠그면 실패 → 프로세스 종료 후.
- vault git: `wiki/` 로컬 미추적 divergence 주의. wiki 쓰기 중 `git pull` 이 새 페이지 덮을 수 있어 `--no-pull` 사용.
- destructive/push/머지는 위임받음. config.toml 편집 시 값 보존.

## 6. 한 줄 요약
> 위키 5-카테고리 fresh 재빌드 진행 중(Fable 5, 5h 캡으로 분할 — gemento/seCall/tunaFlow 완료, 15개 남음, `deep_resume.sh`로 자동 재개). Open PR #118(검색 이식). 다음 병행 최적=Task C(discovery, Rust). 위키 완주 후 vault 커밋/push + repo 프롬프트 영구화.
