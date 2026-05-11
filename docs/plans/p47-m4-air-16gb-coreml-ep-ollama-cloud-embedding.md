---
type: plan
plan_id: P47
title: P47 — M4 Air 16GB 임베딩 부담 완화 (CoreML EP + Ollama Cloud embedding)
status: draft
updated_at: 2026-05-12
version: 1
---

# P47 — M4 Air 16GB 임베딩 부담 완화 (CoreML EP + Ollama Cloud embedding)

## Description

M4 Air 16GB 환경에서 `secall sync` 의 임베딩 단계가 시스템 swap / CPU 100% 점유를 유발해 사용 불가 수준의 슬로다운 발생.

근본 원인 (P46 이후 점검):

1. `OrtEmbedder::new()` 가 `pool_size = 4` 하드코딩 (`crates/secall-core/src/search/embedding.rs:122`) → bge-m3 ONNX (~2.3GB) × 4 ≈ 9GB 메모리 점유
2. `Session::builder()` 가 execution provider 미지정 (`embedding.rs:133, 144`) → ORT 기본 CPUExecutionProvider 사용 → Apple Silicon ANE / GPU 미활용
3. Ollama 백엔드는 데몬에 bge-m3 가 `keep_alive=5m` 상주 — ingest 끝나도 메모리 안 풀림 (시맨틱 단계 진입 시점에만 unload 발사: `ingest.rs:637`)

해결 방향: **로컬 머신 최적화 (Apple Silicon 가속 + pool 크기 합리화) + Ollama Cloud embedding 분기 추가 + ingest 종료 후 embed 모델 unload**.

## Expected Outcome

- M4 Air 16GB 에서 `secall sync` 실행 중 시스템 응답성 유지
- macOS aarch64 빌드에서 `coreml` feature 활성화 시 ORT 가 ANE / GPU 사용 (CoreMLExecutionProvider 등록), 실패 시 CPU 폴백
- `config.embedding.backend = "ollama_cloud"` 옵션 추가, `OLLAMA_CLOUD_API_KEY` env 가 graph/log 와 마찬가지로 `embedding.cloud_api_key` 에도 propagate
- ingest 가 임베딩 단계 종료 시점에 Ollama 로컬 모델을 `keep_alive=0` 으로 unload (graph 패턴 재사용)
- `config.embedding.pool_size` 가 config 로 노출되어 16GB 환경에서 1~2 로 줄일 수 있음

## Subtasks (요약)

1. **Task 01 — ORT CoreML EP 옵션 추가** (`coreml` feature, `Session::builder` 에 EP 등록 + CPU 폴백)
2. **Task 02 — OrtEmbedder pool_size 조정 + config 노출** (`config.embedding.pool_size`, 자동 휴리스틱)
3. **Task 03 — OllamaEmbedder cloud 모드 지원** (`api_key`, `cloud_host`, `cloud_model`, `cloud_api_key` 필드 + `"ollama_cloud"` 백엔드 분기)
4. **Task 04 — ingest 종료 후 Ollama embed unload** (embed sub-loop 종료 직후 `keep_alive=0` 호출)
5. **Task 05 — 문서 + web UI + 회귀 테스트** (`docs/reference/llm-config.md`, `SettingsRoute.tsx`, OllamaEmbedder cloud 에러 회귀)

## Constraints

- **Apple Silicon 외 환경 회귀 방지** — `coreml` feature 는 `#[cfg(all(target_os = "macos", target_arch = "aarch64"))]` 가드. x86_64 / Linux / Windows 빌드는 영향 없음
- **`ort = "=2.0.0-rc.10"` 의 CoreML EP API** — Task 01 첫 단계에서 `ort::execution_providers::CoreMLExecutionProvider` 시그니처 검증. API 가 다르면 task 안에서 폴백 처리
- **Ollama Cloud embedding 가용성 의존** — Task 03 manual smoke 에서 Cloud 가 `/api/embed` 엔드포인트와 bge-m3(또는 동등 모델)를 지원하지 않으면 해당 task 만 dispatch 추가 + 명확한 에러로 마무리하고 plan 은 계속 진행 (Task 01/02/04 만으로도 M4 Air 문제 해결 가능)
- **bge-m3 외 임베딩 모델 교체 X** — 별도 plan

## Non-goals

- bge-m3 → MiniLM 등 더 작은 모델 교체
- OpenVINO 백엔드 개선 (이미 작동, M4 Air 와 무관)
- ANN 인덱스 재설계
- secall-web UI 디자인 변경
- secall sync 명령 자체의 재구조화 (P47 은 임베딩 단계만 손댐)
