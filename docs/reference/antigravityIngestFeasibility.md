---
type: reference
status: done
updated_at: 2026-06-26
canonical: true
---

# Antigravity CLI 세션 ingest — feasibility 조사

> 2026-06-26 실데이터 조사. 핸드오프 `handoff_2026-06-25.md` §4 백로그
> "Antigravity 세션 ingest (추측 구현 금지)" 항목의 근거 문서.
> **결론: 현재는 보류. 정식 파서 = 비공개 protobuf schema 역공학이라 fragile.**

---

## 1. 데이터 위치 / 형식

`~/.gemini/antigravity-cli/conversations/` — 대화 12개 (2026-06 기준):

| 형식 | 개수 | 비고 |
|---|---|---|
| `.pb` (protobuf 바이너리) | 11 | 212KB ~ 27MB |
| `.db` (SQLite, WAL) | 1 | 신형식으로 보임 — **마이그레이션 진행 중** (1/12만 전환) |

부수 파일:
- `~/.gemini/antigravity-cli/history.jsonl` (140줄) — user 프롬프트 인덱스
- `~/.gemini/antigravity-cli/cache/*.json` — last_conversations / projects 메타데이터

## 2. `.db` 구조 (SQLite로 직접 조회)

`.db`도 결국 **protobuf blob 컨테이너**다. 핵심 테이블:

- `trajectory_meta(trajectory_id, cascade_id, trajectory_type, source)`
- `steps(idx, step_type, status, has_subtrajectory, metadata BLOB, ..., step_payload BLOB, step_format)`
- `gen_metadata` / `executor_metadata` / `parent_references` / `trajectory_metadata_blob` / `battle_mode_infos` — 전부 `data BLOB`

대화 본문은 `steps.step_payload`(BLOB)에 **protobuf로 인코딩**되어 들어있다.
`step_type` 은 enum(관측값: 15/14/21/101/138/8/9/23/98 …), `step_format`/`trajectory_type`/`source` 도 정수 enum — **의미 미문서**.

## 3. 텍스트는 복원되나 구조는 schema-lock

- `strings` 로 본문 추출은 됨. 실제 확인된 예 (step_type 101, 16KB payload):
  `[Message] timestamp=2026-06-19T03:41:55Z sender=.../task-20 priority=MESSAGE_PRIORITY_HIGH content=...`
  + task 결과 / tool 출력 prose 포함.
- 그러나 **구조 해독 불가**:
  - 제공되는 `.proto` / descriptor **없음** (SDK 플러그인 dir·agentapi 바이너리에 타입명 미노출)
  - `protoc` 미설치 (`--decode_raw` 도 불가)
  - 데이터 모델이 **"trajectory + steps"(에이전트 스텝, `has_subtrajectory` 계층)** — Claude/Codex/Gemini-CLI 의 단순 user/assistant **turn** 모델과 근본적으로 다름. seCall `Session{turns:[{role,content}]}` 로 매핑하려면 step_type→role 해석 + sub-trajectory 평탄화가 필요.

## 4. `history.jsonl` (쉬운 경로 후보) — 불충분

- 키: `conversationId / display / timestamp / workspace`
- **`display` = user 프롬프트만**. assistant 응답·tool·turn 구조 없음.
- user-프롬프트-only "세션"은 즉시 가능하나 recall/wiki/graph 품질엔 거의 무가치.

## 5. 판정

| 경로 | 가능성 | 가치 | 평가 |
|---|---|---|---|
| `.pb`/`.db` protobuf 정식 파싱 | proto 없어 wire-format 역공학 필요 | 높음 (풀 대화) | ❌ 지금은 fragile |
| `history.jsonl` ingest | 즉시 가능 (안정 JSON) | 낮음 (프롬프트만) | △ 품질 미달 |
| 보류 (proto/schema 공개 대기) | — | — | ✅ **권장** |

Google 프리뷰 제품이라 schema churn 위험이 크고, `.pb`→`.db` 마이그레이션도 진행 중.
핸드오프의 "추측 구현 금지 / schema 안정화 후 별도 P" 가 실데이터로 확인됨.

## 6. 재개 트리거 (구체적 조건)

다음 중 하나가 충족되면 정식 파서 작성을 재검토한다 (그게 "schema 안정화"의 구체 정의):

- Antigravity 가 `.proto` 정의 또는 protobuf descriptor 를 공개 / SDK 에 포함
- `.db` 형식으로 전 대화 마이그레이션 완료 + 스키마 안정 (step_type enum 문서화)
- 공식 export 명령(`agy export` 등) 제공 → 안정 포맷으로 우회

조사 방법(재현): `.db` 는 read-only 복사 후 `sqlite3 <copy> ".schema"` + `step_payload` 를 `strings`.
원본은 절대 수정하지 말 것 (실행 중 앱이 WAL 점유).
