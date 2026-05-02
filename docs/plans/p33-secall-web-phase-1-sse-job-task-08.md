---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 08
parallel_group: G
depends_on: [03, 05, 06]
---

# Task 08 — README + CI 업데이트

## Changed files

수정:
- `README.md` — Web UI Phase 1 안내 (명령 트리거, 진행 상태, Job 시스템), 신규 엔드포인트 목록
- `README.en.md` — 동일 내용 영문판
- `crates/secall-core/Cargo.toml` (선택) — 신규 deps 정리
- `.github/workflows/ci.yml` (선택) — 큰 변경 없음, P33 신규 테스트 자동 실행 확인

신규: 없음

## Change description

### 1. README.md — "Web UI" 섹션 갱신

기존 "Phase 0 기능" 부분을 두 단계로 나눔:

```markdown
## Web UI

`secall serve`는 REST API와 함께 웹 UI를 동일 포트에서 제공합니다 (단일 진입점).

```bash
secall serve --port 8080
# 브라우저에서 http://127.0.0.1:8080 접속
```

**Phase 0 기능** (P32):
- 검색 / 세션 브라우징 (2-pane 레이아웃)
- 일일 일기 / 위키 페이지 열람 (전체 본문)
- 그래프 탐색 (사이드바 Graph 버튼 → 풀스크린 오버레이)
- 태그 / 즐겨찾기 편집

**Phase 1 기능** (P33):
- 명령 트리거 — Sync / Ingest / Wiki Update를 웹에서 실행
- 진행 상태 SSE 스트리밍 — phase별 실시간 표시
- 글로벌 진행 배너 + 완료/실패 toast 알림
- 부분 성공 명시 (예: "ingest까지 OK / push 실패")
- 한 번에 하나의 mutating 작업만 실행 (단일 큐)
- 탭 닫고 재접속 시 진행 중 작업 자동 복원

### 명령 사용

웹 UI에서 좌측 사이드바 **Commands** 메뉴 → 원하는 명령 + 옵션 → 시작.

CLI에서도 동일하게 사용 가능 (Job 시스템은 웹 UI 전용):
```bash
secall sync --local-only --dry-run
secall ingest --auto --auto-graph
secall wiki update --backend claude
```
```

### 2. README.md — 엔드포인트 목록 갱신

기존 "Web UI + REST API + Obsidian 플러그인" 섹션의 endpoints 목록:
```
**엔드포인트**:
- 읽기 (Phase 0): /api/recall, /api/get, /api/status, /api/daily, /api/graph
- 위키 (Phase 0+1): /api/wiki (검색), GET /api/wiki/{project} (본문 — Phase 1)
- 세션 메타 (Phase 0): /api/sessions, /api/projects, /api/agents, PATCH /api/sessions/{id}/{tags,favorite}
- 명령 (Phase 1): POST /api/commands/{sync,ingest,wiki-update}
- Job 관리 (Phase 1): GET /api/jobs, GET /api/jobs/{id}, GET /api/jobs/{id}/stream (SSE), POST /api/jobs/{id}/cancel (501, v1.1)
```

### 3. README.md — Job 시스템 동작 설명

새 섹션 추가:
```markdown
### Job 시스템 동작

명령 트리거(sync/ingest/wiki update)는 백그라운드 Job으로 실행됩니다:

1. **POST /api/commands/{kind}** → `{ job_id, status: "started" }` 즉시 응답
2. 진행 중 상태는 메모리에 저장되어 빠른 폴링 / SSE 가능
3. 완료/실패 시 `jobs` 테이블에 영구 기록
4. **단일 큐**: 동시에 mutating 작업은 1개만 — 두 번째 요청은 `409 Conflict`
5. **Read 작업** (검색, 세션 조회 등)은 동시 무제한
6. 서버 재시작 시 `running`/`started` 상태 jobs는 `interrupted`로 갱신
7. 7일 이상된 완료/실패/중단 jobs는 시작 시 자동 cleanup

### Phase 분리 (sync 예시)

```
sync = pull → reindex → ingest → graph → push
```

각 phase 완료마다 SSE 이벤트 발행. push 실패 시 ingest까지의 결과는 보존되며 결과 JSON에 명시:

```json
{
  "pulled": 3,
  "reindexed": 5,
  "ingested": 2,
  "pushed": null,
  "error": "push failed: <message>"
}
```
```

### 4. README.en.md 동기화

위 한글 섹션을 영문으로 동등하게 추가:
- "Web UI" → Phase 0/1 features
- "Endpoints" → 갱신된 목록
- "Job System" → 새 섹션

### 5. CI 변경 (확인만)

P33은 신규 테스트(jobs_rest, graph_incremental 등)가 추가됨. CI는 `cargo test --all`로 모두 자동 실행 — 별도 설정 불필요. 단, P33 런타임 의존성(uuid 등)이 cargo audit에서 경고 없는지 확인.

CI workflow 자체 변경은 없음 (P32에서 이미 web-build job + matrix 갖춤).

### 6. 변경 이력 갱신 (선택)

`README.md`의 "업데이트 이력" 표에 P33 라인 추가:
```markdown
| 2026-XX-XX | v0.4.0 | Web UI Phase 1: 명령 트리거 (Sync/Ingest/Wiki Update), SSE 진행 스트리밍, Job 시스템 (단일 큐 + 7일 cleanup), 그래프 자동 증분 (--auto-graph), 위키 본문 GET 엔드포인트 |
```

버전 bump (`Cargo.toml workspace.package.version`)는 별도 release 작업 — 본 task 범위 외.

## Dependencies

- Task 04, 06, 07 완료 (실제 동작이 README 사양과 일치해야 함)
- 외부 crate / 도구 추가 없음

## Verification

```bash
# 1. README 마커 확인
grep -q "Phase 1" /Users/d9ng/privateProject/seCall/README.md && echo "README.md updated"
grep -q "Phase 1" /Users/d9ng/privateProject/seCall/README.en.md && echo "README.en updated"
grep -q "/api/commands/" /Users/d9ng/privateProject/seCall/README.md && echo "endpoints listed"
grep -q "Job 시스템" /Users/d9ng/privateProject/seCall/README.md && echo "job system documented"

# 2. workflow YAML 변경 없음 확인 (또는 actionlint)
git diff --stat .github/workflows/ | head -3

# 3. cargo audit 회귀 없음 (선택)
cargo install cargo-audit --locked 2>/dev/null || true
cargo audit 2>&1 | tail -5

# 4. 전체 회귀
cargo test --all
cd web && pnpm typecheck && pnpm build
```

## Risks

- **README 일관성**: 사용자가 실제 보는 동작과 README 설명이 다르면 신뢰 저하. Task 04/06/07 검증 통과 후 본 task 작성
- **의존성 보안 audit**: uuid, tokio-stream 등 신규 deps의 known CVE 없는지 cargo audit으로 확인
- **CI YAML 변경 최소**: P33은 코드 추가만 — CI가 자동으로 빌드/테스트. 변경 없으면 회귀 위험도 없음
- **버전 bump**: README 변경은 v0.4.0 가정. 실제 bump 시점은 별도 commit (release tagging)에서

## Scope boundary

수정 금지:
- `crates/`, `web/src/` 코드 — Task 01~08 완료 후 본 task는 문서만
- `crates/secall-core/src/web/`, `mcp/rest.rs`의 라우트 본체 — Task 04
- `web/src/routes/CommandsRoute.tsx`, `components/{CommandButton,JobItem,JobBanner}.tsx` — Task 06, 07
- DB 스키마 — Task 01
- 기존 마이그레이션 분기 — Task 01에서 v6만 추가
