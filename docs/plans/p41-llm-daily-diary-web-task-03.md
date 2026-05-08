---
type: task
plan_slug: p41-llm-daily-diary-web
task_id: 03
title: REST `/api/config` (read + 선택적 write)
parallel_group: B
depends_on: [01, 02]
status: pending
updated_at: 2026-05-08
---

# Task 03 — REST `/api/config`

## Changed files

수정:
- `crates/secall-core/src/mcp/rest.rs:140` 근처 — `.route("/api/config", get(api_config_get).patch(api_config_patch))` 추가.
- `crates/secall-core/src/mcp/rest.rs:266` 근처 — 새 핸들러 `api_config_get`, `api_config_patch` 추가.
- `crates/secall-core/src/mcp/server.rs:300` 근처 — `do_config_get(sanitize: bool)` + `do_config_patch(section: &str, body: serde_json::Value)` 메서드 추가.
- `crates/secall-core/src/vault/config.rs` — 새 메서드: `pub fn save(&self) -> Result<()>` (config.toml 위치에 직렬화 후 atomic 쓰기). 기존 `Config::load_or_default` 와 한 쌍.
- `crates/secall-core/src/mcp/server.rs` 의 `SeCallMcpServer` — `allow_config_edit: bool` 필드 추가 (default false).
- `crates/secall/src/commands/serve.rs` — `--allow-config-edit` flag 추가, `SeCallMcpServer::new_with_options` (또는 builder) 에 전달.

신규:
- `crates/secall-core/tests/rest_config.rs` (신규) — `GET /api/config` sanitize 동작 + `PATCH /api/config/<section>` round-trip + write disabled 시 403 회귀.

## Change description

### 1. `GET /api/config`

- 응답: 전체 Config 의 sanitized JSON. **secret 필드 마스킹**:
  - `graph.gemini_api_key` → `"<masked>"` 또는 `null`
  - 환경변수 (`ANTHROPIC_API_KEY` 등) 는 응답 별도 섹션 `env_indicators: { ANTHROPIC_API_KEY: true|false }` (값 노출 X, 존재 여부만).
- query param `?include_secrets=true` 는 **lokal 요청 (Origin / X-Forwarded-For 검증)** 에만 허용 — default false. 본 plan 에선 포함 X (보안 boundary).

### 2. `PATCH /api/config/<section>`

- `<section>` ∈ {`wiki`, `graph`, `log`, `embedding`}. 그 외는 404.
- body: 부분 업데이트. 예: `{"default_backend": "haiku"}` 만 보내면 `[wiki].default_backend` 만 갱신.
- 처리: 현재 Config 로드 → 해당 섹션 부분 머지 → `Config::save()`.
- **`allow_config_edit = false` (default)** 면 405 또는 403 반환. 활성화하려면 `secall serve --allow-config-edit`.
- API key 같은 secret 은 **무시** — body 에 포함돼도 마스킹된 placeholder 일 가능성 높으므로. 명시적 secret 갱신은 CLI / .env 권장.

### 3. `Config::save()`

- 위치: `vault/config.rs::Config::config_file_path()` 가 반환하는 경로 (예: `~/Library/Application Support/secall/config.toml` macOS).
- 직렬화: `toml::to_string_pretty(self)?` 후 atomic write (`tempfile + rename`).
- **주의**: 기존 사용자 주석 손실 가능. 본 task 는 단순 직렬화. 주석 보존은 별도 (`toml_edit` crate 도입) plan 으로 미룸.

### 4. `--allow-config-edit` 플래그

```
secall serve --port 8080 --allow-config-edit
```

config 편집 enable 시 stdout 에 강한 경고 출력:
```
WARN: --allow-config-edit 활성화. 외부에 노출 금지.
```

### 5. 회귀 테스트 (`tests/rest_config.rs`)

- `GET /api/config` 에 `gemini_api_key` 포함되지 않음 (sanitized).
- `PATCH /api/config/wiki` body `{"default_backend": "ollama"}` → 200 + 후속 GET 결과 반영.
- `--allow-config-edit` 없이 PATCH → 403 또는 405.
- 잘못된 section (`/api/config/foo`) → 404.

## Dependencies

- **task 01 + 02 필수** — `[log]` 섹션 + hard-coded default constants 가 본 task 의 sanitize 대상. 본 task 가 그 위에서 동작.
- crate dep: 추가 없음. `toml` 은 이미 워크스페이스 dep.

## Verification

```bash
cargo check -p secall-core
cargo test -p secall-core --test rest_config

# (수동, server 실행 후)
# 1. GET sanitize
curl -s http://localhost:8080/api/config | jq '.graph.gemini_api_key'   # null 또는 "<masked>"

# 2. write disabled
curl -s -X PATCH http://localhost:8080/api/config/wiki \
  -H 'content-type: application/json' \
  -d '{"default_backend":"haiku"}' | jq '.error // .'   # 403/405

# 3. write enabled (별도 server)
secall serve --port 8081 --allow-config-edit &
curl -s -X PATCH http://localhost:8081/api/config/wiki \
  -H 'content-type: application/json' \
  -d '{"default_backend":"haiku"}'
curl -s http://localhost:8081/api/config | jq '.wiki.default_backend'    # "haiku"
```

## Risks

- **toml save 가 사용자 주석 손실** — 본 task 의 명시적 한계. 사용자가 손으로 작성한 주석/공백이 사라질 수 있음. 현실적으로 `secall config` 으로 관리되는 파일이라 주석 적을 가능성. 향후 `toml_edit` 도입 별도 plan.
- **race** — 사용자가 손으로 toml 편집 + web settings 동시 PATCH 시 마지막 write 가 win. 단일 사용자 가정.
- **API key 갱신 경로 부재** — secret 은 `.env` / 환경변수 / `secall config set` CLI 만. web UI 는 unable. 의도적 — UI 에 secret 노출/입력 자체를 피함. task 04 의 form 도 마스킹 placeholder + "환경변수로 설정" 안내만.
- **Origin 검증 없음** — `secall serve` 가 default 로 `127.0.0.1` 만 listen 한다 가정 (로컬 단일 사용자). 외부 노출 시 PATCH 가 위험 → `--allow-config-edit` flag 의 강한 경고 + doc 에 명시.

## Scope boundary (수정 금지)

- `crates/secall-core/src/store/` — DB 영역. config 는 toml 파일.
- `crates/secall-core/src/wiki/` — backend 본체.
- `crates/secall/src/commands/wiki.rs`, `commands/log.rs` — task 01 / 02 영역.
- `web/src/` — task 04 영역.
