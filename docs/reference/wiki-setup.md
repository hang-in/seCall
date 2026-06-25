---
type: reference
status: draft
updated_at: 2026-04-06
---

# seCall Wiki 설정 가이드

Claude Code 메타에이전트를 활용해 에이전트 세션 로그에서 Obsidian 위키를 자동 생성·유지하는 방법을 설명합니다.

---

## 사전 요구사항

1. **Claude Code CLI 설치**

   ```bash
   npm install -g @anthropic-ai/claude-code
   claude --version
   ```

2. **MCP 서버 등록** — `~/.claude/settings.json`에 secall MCP 서버 추가

   ```json
   {
     "mcpServers": {
       "secall": {
         "command": "secall",
         "args": ["mcp"]
       }
     }
   }
   ```

3. **Vault 초기화**

   ```bash
   secall init
   ```

   이 명령은 `wiki/`, `wiki/projects/`, `wiki/topics/`, `wiki/decisions/`, `wiki/overview.md`, `SCHEMA.md`를 생성합니다.

---

## 백엔드 호환성

`wiki update` 의 **생성(generation)** 과 **검토(review)** 는 요구하는 능력이 다릅니다.

- **생성(batch / incremental)** — prompt 가 MCP 도구 호출(`secall recall`/`get`/`status`)과 `wiki/` 파일 쓰기를 능동적으로 수행해야 합니다. 따라서 **도구 호출이 가능한 에이전트 CLI** 만 동작합니다.
- **검토(`--review`)** — 생성된 페이지 텍스트를 받아 평가만 하므로 일반 LLM HTTP 백엔드로도 됩니다.

| backend | 생성 (`wiki update`) | 검토 (`--review`) | 비고 |
|---|---|---|---|
| `claude` | ✅ | ✅ | Claude Code CLI (MCP 통합). 단 2026-06-15 이후 구독은 credit pool 소진 — 대량 생성 시 비용 주의 |
| `codex` | ✅ | ✅ | Codex CLI |
| `haiku` | ✅ | ✅ | Anthropic API — 세션 데이터를 prompt 에 inline (도구 불필요). `ANTHROPIC_API_KEY` 필요 |
| `ollama` | ❌ | — | HTTP, 도구 호출 불가 → 생성 시 명시적 에러 (issue #88) |
| `lmstudio` | ❌ | — | 위와 동일 |
| `ollama_cloud` | ⚠️ 비권장 | ✅ | review 는 정상. 생성은 도구 호출 불가라 빈 결과가 날 수 있음 — `--review` 백엔드로만 권장 |

> **참고**: graph 시맨틱 추출과 log 일기는 별개로, 이미 `ollama_cloud` (`OLLAMA_CLOUD_API_KEY`) 를 기본으로 외부 에이전트 CLI 없이 동작합니다. 즉 **외부 CLI 가 꼭 필요한 건 wiki 본문 생성 뿐**입니다.

---

## 수동 실행

### 전체 배치 업데이트

전체 세션을 분석해 위키를 처음부터 생성·갱신합니다:

```bash
secall wiki update
```

| 옵션 | 설명 | 기본값 |
|---|---|---|
| `--model opus` | 고품질, 느림 | `sonnet` |
| `--model sonnet` | 빠름, 일상 사용 | (기본) |
| `--since YYYY-MM-DD` | 특정 날짜 이후 세션만 처리 | 전체 |
| `--session <ID>` | 특정 세션만 처리 (증분 모드) | - |
| `--dry-run` | 실제 실행 없이 프롬프트만 출력 | - |

### 증분 업데이트 (특정 세션)

```bash
secall wiki update --session abc12345 --model sonnet
```

### 프롬프트 확인 (dry-run)

```bash
secall wiki update --dry-run
```

### 위키 현황 확인

```bash
secall wiki status
```

---

## 자동 실행 (post-ingest hook)

`secall ingest` 후 자동으로 위키를 갱신하려면 hook을 설정합니다.

### 1. hook 스크립트 복사

```bash
mkdir -p ~/.config/secall/hooks
cp examples/hooks/wiki-update.sh ~/.config/secall/hooks/wiki-update.sh
chmod +x ~/.config/secall/hooks/wiki-update.sh
```

### 2. config.toml 설정

`~/.config/secall/config.toml`:

```toml
[hooks]
post_ingest = "~/.config/secall/hooks/wiki-update.sh"
hook_timeout_secs = 300  # 5분 (Opus 기준)
```

### 3. 동작 확인

```bash
secall ingest --auto
# 로그에 "[wiki-hook] Wiki updated for session ..." 메시지 확인
```

---

## 비용 고려사항

| 시나리오 | 권장 모델 | 이유 |
|---|---|---|
| 증분 업데이트 (일상) | Sonnet | 빠르고 저렴, 단일 세션 |
| 첫 배치 생성 | Opus | 복잡한 클러스터링 품질 |
| 주 1회 전체 재생성 | Opus | 누적 인사이트 정리 |

---

## 프롬프트 커스텀

기본 프롬프트를 커스텀하려면:

```bash
mkdir -p ~/.config/secall/prompts

# 배치 모드 프롬프트 커스텀
cp docs/prompts/wiki-update.md ~/.config/secall/prompts/wiki-update.md

# 증분 모드 프롬프트 커스텀
cp docs/prompts/wiki-incremental.md ~/.config/secall/prompts/wiki-incremental.md
```

또는 환경변수로 디렉토리를 지정:

```bash
export SECALL_PROMPTS_DIR=/path/to/my/prompts
```

---

## 문제 해결

**"wiki/ directory not found" 오류**

```bash
secall init  # vault 재초기화
```

**"Claude Code CLI not found" 오류**

```bash
npm install -g @anthropic-ai/claude-code
which claude
```

**hook timeout 초과**

`config.toml`에서 `hook_timeout_secs` 값을 늘리세요 (예: `600`).
