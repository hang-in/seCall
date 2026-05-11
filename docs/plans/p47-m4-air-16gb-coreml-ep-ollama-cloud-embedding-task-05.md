---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P47
task_id: 05
parallel_group: 4
depends_on: [04]
---

# P47 Task 05 — 문서 + web UI + 회귀 테스트

## Changed files

### 문서 갱신

- `docs/reference/llm-config.md` — Backend 매트릭스 표의 Embedding 컬럼 / 환경변수 표 / Config Keys 표 갱신:
  - Backend 매트릭스에 `ollama_cloud` 행의 Embedding 컬럼에 ✅ 추가.
  - Config Keys 표에 다음 행 추가:
    - `[embedding].pool_size` — `None` (auto) — "ORT session pool size. 미설정 시 시스템 RAM 기반 자동 결정"
    - `[embedding].cloud_host` — `https://ollama.com`
    - `[embedding].cloud_model` — `null` — "Ollama Cloud embedding 모델 이름"
  - 환경변수 표의 `OLLAMA_CLOUD_API_KEY` 설명 갱신: "ollama_cloud graph / log / **embedding** backend".
  - Troubleshooting 또는 신규 섹션 "Apple Silicon 가속 빌드" 추가:
    ```text
    ## Apple Silicon (M1/M2/M3/M4) 가속 빌드

    macOS aarch64 환경에서 ORT 임베딩 백엔드를 사용한다면 CoreML EP 를 활성화해
    ANE / GPU 가속을 사용할 수 있습니다.

        cargo build --release -p secall --features secall-core/coreml

    빌드 후 stderr 로그의 `ORT session pool created coreml=true` 로 EP 활성 여부를
    확인할 수 있습니다. CoreML 등록이 실패하면 ORT 가 자동으로 CPU 로 폴백합니다.
    ```

- `README.md` / `README.en.md` — Config Keys 섹션 (P46 에서 갱신한 부근, README.md:655 근처) 에 다음 키 추가:
  - `embedding.pool_size`, `embedding.cloud_host`, `embedding.cloud_model`
  - 한 줄씩, 기존 표 형식 유지.

### Web UI

- `web/src/lib/api.ts:42-48` — `ConfigDto.embedding` 인터페이스에 다음 필드 추가:
  ```text
  pool_size?: number | null;
  cloud_host?: string | null;
  cloud_model?: string | null;
  cloud_api_key?: string | null;
  ```
- `web/src/routes/SettingsRoute.tsx` Embedding 섹션:
  - backend 선택지에 `"ollama_cloud"` 추가.
  - 신규 입력 필드: `Pool size` (number), `Cloud host`, `Cloud model`.
  - `cloud_api_key` 는 P46 의 graph/log 패턴처럼 `MaskedKeyInfoModal` 로 envVar=`OLLAMA_CLOUD_API_KEY` 안내만 노출.
  - `sectionErrors` 의 embedding 블록에 `cloud_host` (URL 검증), `cloud_model` (모델명 검증) 추가 — Save 버튼 disable 연동.

- `web/src/routes/SettingsRoute.test.tsx` — embedding 섹션의 ollama_cloud 선택지 / cloud_host 입력 검증 / cloud_model invalid 시 Save disable 테스트 케이스 추가. 기존 graph/log 테스트와 동일 패턴.

### 회귀 테스트

- `crates/secall-core/tests/rest_config.rs` — 신규:
  - `test_patch_embedding_section_ignores_cloud_api_key` — `PATCH /api/config/embedding` 의 body 에 `cloud_api_key` 가 와도 sanitize 되는지 검증 (P46 graph/log 패턴 그대로 embedding 으로).
  - `test_get_config_masks_embedding_cloud_api_key` — GET 응답에서 `embedding.cloud_api_key` 마스킹 확인.
- `crates/secall-core/tests/llm_defaults.rs` — 변경 불필요 (모델 default 상수는 P47 에서 추가하지 않음).

### CHANGELOG / 마이그레이션 안내

- `docs/reference/llm-config.md` 의 P46 마이그레이션 섹션 다음에 P47 항목 추가:
  ```text
  ## P47 — 임베딩 부담 완화 (M4 Air 16GB)

  - macOS aarch64 + ORT 백엔드 사용 시 `--features secall-core/coreml` 빌드로 ANE/GPU 가속.
  - 16GB 환경에선 `[embedding] pool_size = 1` 권장 (default 자동 결정).
  - 임베딩 자체를 Cloud 로 옮기려면 `[embedding] backend = "ollama_cloud"` + `OLLAMA_CLOUD_API_KEY` 설정.
  ```

## Change description

### 단계별 접근

1. **문서 먼저** — `llm-config.md` 와 README 표 갱신. 사용자가 즉시 확인 가능.

2. **Web UI 갱신** — `api.ts` 인터페이스 → `SettingsRoute.tsx` 입력 → 테스트.

3. **회귀 테스트 추가** — `rest_config.rs` 의 embedding 섹션 secret 필터링 검증.

4. **마이그레이션 안내** — P46 섹션 아래에 짧게 추가.

### 구현 제약

- **README 표 정렬 유지** — markdown pipe 정렬 깨지지 않도록.
- **Web UI 동작 검증** — `npm run dev` 띄우고 Settings → Embedding 에서 `ollama_cloud` 선택 / pool_size 입력 / save 흐름 manual 확인.
- **테스트 ENV 정리** — env 사용 테스트는 `ENV_MUTEX` 또는 tokio sync Mutex 사용.

## Dependencies

- **Task 01~04 완료 필수** — 모든 코드 변경이 들어간 후 문서/테스트 정리.

## Verification

```bash
# 타입체크
cargo check -p secall-core
cargo check -p secall

# 회귀 테스트
cargo test -p secall-core --test rest_config
cargo test -p secall-core --lib vault::config

# 전체 lib + 통합 (정합성 확인)
cargo test -p secall-core --lib
cargo test -p secall --lib

# Web
cd web && npm run typecheck && npm test -- --run

# Manual: 문서 시각 검증
# Manual: cat docs/reference/llm-config.md | grep -A 2 'pool_size\|cloud_host\|coreml'
# Manual: backends 표의 ollama_cloud 행 Embedding 컬럼이 ✅ 인지

# Manual: Web Settings UI
# cd web && npm run dev
# Manual: http://localhost:5173/settings → Embedding 섹션:
#   - backend 셀렉트에 ollama_cloud 표시
#   - Pool size, Cloud host, Cloud model 입력 필드 표시
#   - Ollama Cloud API key 모달이 OLLAMA_CLOUD_API_KEY 안내
```

## Risks

- **README 두 언어 동기 유지 누락** — 한/영 README 표를 함께 갱신하지 않으면 inconsistency. 둘 다 같은 PR 에 포함.
- **Web UI 테스트 셀렉터 이름 충돌** — `aria-label` 이 기존 입력과 겹치지 않게 "Embedding cloud host" / "Embedding cloud model" 처럼 prefix.
- **마이그레이션 안내 누락** — `embedding.backend = "ollama_cloud"` 로 바꾼 사용자가 `OLLAMA_CLOUD_API_KEY` 미설정 시 로컬 폴백되는 점을 문서에 명시.
- code-review-graph 영향: 문서 + 테스트 변경이므로 함수 그래프 영향 없음. Web UI 는 별도 그래프.

## Scope boundary (수정 금지)

- Task 01~04 에서 이미 작성한 코드 — 본 task 는 정리/문서/테스트만 추가.
- `crates/secall-core/src/ingest/*`, `detect.rs::find_gemini_sessions` — 외부 ingest 영역, 본 plan 과 무관.
- 결과 보고서 `docs/plans/p47-m4-air-16gb-coreml-ep-ollama-cloud-embedding-result.md` — tunaFlow 자동 생성, 작성 금지.
