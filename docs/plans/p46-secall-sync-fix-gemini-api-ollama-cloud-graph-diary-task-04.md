---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P46
task_id: 04
parallel_group: 3
depends_on: [03]
---

# P46 Task 04 — 용도별 기본 모델 매핑 + diary 컨텍스트 가드

## Changed files

### Default 모델 상수

- `crates/secall-core/src/llm/defaults.rs` — 신규 상수 추가:
  - `pub const GRAPH_OLLAMA_CLOUD_DEFAULT: &str = "gemma4:31b-cloud";`
  - `pub const LOG_OLLAMA_CLOUD_DEFAULT: &str = "kimi-k2.6:cloud";`
  - `pub const LOG_CONTEXT_CHAR_LIMIT: usize = 400_000;` (보수치 100k token ≈ 400k chars)

### Default 적용 — Graph

- `crates/secall-core/src/graph/semantic.rs` — Task 03 에서 추가한 `"ollama_cloud" =>` arm 에서 `cloud_model` 이 `None` 일 때 `GRAPH_OLLAMA_CLOUD_DEFAULT` 사용. `warn_using_default("graph.cloud_model", GRAPH_OLLAMA_CLOUD_DEFAULT)` 호출.
- `crates/secall-core/src/vault/config.rs` — `pub fn default_graph_ollama_cloud_model() -> &'static str { GRAPH_OLLAMA_CLOUD_DEFAULT }` 헬퍼 추가 (다른 default helper 와 동일 패턴).

### Default 적용 — Log

- `crates/secall/src/commands/log.rs:185-209` — `resolve_log_model` 의 `"ollama_cloud" =>` arm 에서 `config.log.cloud_model` → `config.graph.cloud_model` → `LOG_OLLAMA_CLOUD_DEFAULT` 폴백.
- `crates/secall/src/commands/log.rs:1-12` — import 에 `LOG_OLLAMA_CLOUD_DEFAULT` 추가.

### Diary 컨텍스트 가드

- `crates/secall/src/commands/log.rs:122-127` — `user_prompt` 구성 직후, `system_prompt` 와 `user_prompt` 의 char 길이 합산이 `LOG_CONTEXT_CHAR_LIMIT` 을 초과하면 다음 처리:
  1. `tracing::warn!(target_date, total_chars, limit, "diary input exceeds context budget, truncating oldest project entries")` 출력 + `eprintln!` 로 사용자에게 동일 메시지.
  2. `by_project` 의 entries 중 가장 오래된 (= chronological 순서가 가장 앞선) 프로젝트부터 entries 를 제거하면서 length 재계산.
  3. 모든 entries 제거 후에도 limit 초과면 마지막 안전망으로 `user_prompt` 를 char 단위로 `LOG_CONTEXT_CHAR_LIMIT - len(system_prompt) - 1000` 까지 자르고 `"...(truncated)"` 추가.

- `crates/secall/src/commands/log.rs` — char 기반 길이 추정 헬퍼 `fn estimate_input_chars(system: &str, user: &str) -> usize { system.len() + user.len() }` 같은 단순 함수로 충분. 정확한 토큰화 도입 X.

- **트리밍 우선순위**: `by_project` 의 BTreeMap 정렬은 알파벳 순이라 "오래된 순" 과 일치하지 않을 수 있음. 가능하면 entries 의 timestamp 정보로 sort 한 후 자르거나, 단순히 entries 가 가장 많은 project 부터 잘라도 OK (이건 architect 가 결정 안 하고 implementer 가 코드 컨텍스트 보고 선택).

### 단위 테스트

- `crates/secall/src/commands/log.rs` `#[cfg(test)]` — 신규 테스트:
  - `test_resolve_log_model_ollama_cloud_uses_config_field` — `config.log.cloud_model = Some("custom:tag")` 면 `resolve_log_model(..., "ollama_cloud", None) == Some("custom:tag")`.
  - `test_resolve_log_model_ollama_cloud_falls_back_to_graph_cloud_model` — `config.log.cloud_model = None`, `config.graph.cloud_model = Some("g:m")` 면 graph 값 사용.
  - `test_resolve_log_model_ollama_cloud_falls_back_to_default` — 둘 다 `None` 이면 `LOG_OLLAMA_CLOUD_DEFAULT` 반환.
  - `test_diary_input_truncated_when_over_limit` — `LOG_CONTEXT_CHAR_LIMIT` 보다 큰 입력을 만들고 prompt builder 가 limit 안에서 끝나는지 검증.

## Change description

### 단계별 접근

1. **상수 정의** — `llm/defaults.rs` 에 cloud default 3개 추가.

2. **graph dispatch defaults** — Task 03 에서 cloud_model 이 None 일 때 에러 대신 default 로 폴백 + warn.

3. **log dispatch defaults** — `resolve_log_model` 의 `"ollama_cloud"` arm 에 defaults chain.

4. **diary input 가드** — `log.rs` 의 `pub async fn run` 안에서 `user_prompt` 가 만들어진 직후 (line 122-127 후) 길이 체크. 초과 시 가장 entries 가 많은 프로젝트의 오래된 entries 부터 자르기.

5. **로깅** — `tracing::warn!` + `eprintln!` 둘 다 호출 (사용자가 CLI 출력에서 즉시 확인 가능하도록).

### 구현 제약

- **정확한 토큰화 도입 X** — `len()` (byte 단위) 또는 `chars().count()` 둘 다 OK. byte 가 더 빠르지만 멀티바이트 한국어가 많으면 chars 가 더 보수적. 권장: `chars().count()`.
- **사용자 데이터 보존 우선** — 가능한 한 entries 자르기 전에 summary 길이 (`.chars().take(150)`) 를 줄이는 등 점진적 reduction. 단, 구현 복잡도가 올라가면 단순히 entries 자르기로 OK.
- **임계치는 보수적** — 모델 컨텍스트 128k 토큰 ≈ 512k chars (1 token ≈ 4 chars 한국어 보수치). 100k token 보수치 = 400k chars. system_prompt + 출력 여유분 확보.
- **defaults 는 코드 수정 없이 override 가능** — 사용자가 `config.toml` 에 `cloud_model = "다른:태그"` 적으면 그게 우선. 하드코딩 X.

## Dependencies

- **Task 03 완료 필수** — cloud dispatch arm 이 있어야 default 폴백 의미 있음.

## Verification

```bash
# 타입체크
cargo check -p secall-core
cargo check -p secall

# 단위 테스트 — defaults + resolve + 가드
cargo test -p secall-core --lib llm::defaults
cargo test -p secall --lib commands::log

# 통합 테스트 — backend resolve
cargo test -p secall --test log_backend_resolve

# Manual: defaults 적용 확인 (network 필요)
# OLLAMA_CLOUD_API_KEY=$(grep OLLAMA_CLOUD_API_KEY .env | cut -d= -f2) \
#   cargo run -p secall -- log --backend ollama_cloud 2026-05-12
# Manual: 출력 첫 줄에 "Generating work log with ollama_cloud:kimi-k2.6:cloud (...)" 가 보여야 함

# Manual: diary 가드 동작 확인 (10일치 등 큰 날짜 강제 입력)
# cargo run -p secall -- log --backend ollama_cloud 2026-04-01
# Manual: 입력 char 수가 임계치 근처면 stderr 에 "diary input exceeds context budget, truncating..." 경고
```

## Risks

- **모델 태그 stale 가능성** — `gemma4:31b-cloud`, `kimi-k2.6:cloud` 가 Ollama Cloud 카탈로그에서 변경/삭제될 수 있음. defaults 만 박혀있어서 사용자는 `cloud_model` config 로 즉시 override 가능. Task 05 문서에 "카탈로그 변경 시 config 로 override" 안내.
- **128k 컨텍스트 초과 케이스** — kimi-k2.6 의 실제 컨텍스트가 128k 가 아닐 가능성도 있음. 사용자 우려를 반영해 보수치로 100k token 가드 설정. 실제 운영하면서 임계치 튜닝 필요.
- **char→token 환산 부정확** — 한국어/영어 혼합에서 1 token ≈ 2~4 chars 범위. 가드가 너무 보수적이면 멀쩡한 일기 entries 가 잘릴 수 있음. 일기 입력이 보통 작은 편이라 영향 적을 듯.
- **trimming 으로 인한 일기 품질 저하** — entries 가 잘린 일자의 일기가 누락될 수 있음. trimming 발생 시 일기 footer 에 "(일부 entries truncated due to context limit)" 같은 표시 추가 검토.
- code-review-graph 영향: `resolve_log_model`, `generate_log_body` 가 호출 그래프 상에서 다수 path 의 root. 변경은 분기 추가만이라 회귀 위험 낮음.

## Scope boundary (수정 금지)

- `crates/secall/src/commands/sync.rs` — Task 01 영역.
- `crates/secall-core/src/graph/semantic.rs` 의 dispatch 자체 — Task 03 영역 (이 task 에서는 default 폴백만 추가).
- `crates/secall-core/src/wiki/**` — wiki 는 별개.
- 문서 갱신, web UI — Task 05.
- 외부 Gemini ingest 코드 (`ingest/gemini*.rs`, `detect.rs::find_gemini_sessions`).
