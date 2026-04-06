---
type: reference
status: draft
updated_at: 2026-04-06
---

# GitHub Vault Sync — 설정 가이드

멀티기기 환경에서 seCall vault를 GitHub로 동기화하는 방법을 안내합니다.

## 개요

```
Mac A (회사)                    Mac B (집)
secall ingest --auto            secall sync
  → MD 생성 → git push           → git pull → reindex → 검색 가능
```

seCall의 vault(마크다운 파일)는 source of truth이며, SQLite DB는 파생 캐시입니다.
vault를 GitHub에 동기화하면 어떤 기기에서든 전체 세션을 검색할 수 있습니다.

## 1. GitHub 저장소 생성

```bash
# GitHub에서 private 저장소 생성 (웹 또는 CLI)
gh repo create secall-vault --private --clone
```

> **반드시 private 저장소를 사용하세요.** vault에는 AI 에이전트와의 대화 내용이 포함됩니다.

## 2. seCall 초기화 (첫 번째 기기)

```bash
# vault 초기화 + git 연동
secall init --vault ~/Documents/Obsidian\ Vault/seCall \
            --git git@github.com:YOUR_USER/secall-vault.git

# 기존 세션 수집
secall ingest --auto

# 동기화 (push)
secall sync
```

## 3. 다른 기기 설정

```bash
# vault 디렉토리에 저장소 clone
git clone git@github.com:YOUR_USER/secall-vault.git \
    ~/Documents/Obsidian\ Vault/seCall

# seCall 초기화 (git은 이미 설정됨)
secall init --vault ~/Documents/Obsidian\ Vault/seCall

# 동기화 (pull + reindex + 로컬 세션 ingest + push)
secall sync
```

## 4. 자동 동기화 설정

### Claude Code Hook (권장)

`~/.claude/settings.json`에 추가:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Initialize",
      "hooks": [{
        "type": "command",
        "command": "secall sync --local-only"
      }]
    }],
    "PostToolUse": [{
      "matcher": "Exit",
      "hooks": [{
        "type": "command",
        "command": "secall sync"
      }]
    }]
  }
}
```

- **세션 시작 시**: `sync --local-only` — pull + reindex만 수행 (빠름)
- **세션 종료 시**: `sync` — 전체 동기화 (push 포함)

### Cron (대안)

```bash
# 5분마다 동기화 (백그라운드)
crontab -e
*/5 * * * * /usr/local/bin/secall sync >> /tmp/secall-sync.log 2>&1
```

## 5. SSH 키 설정

GitHub push/pull에 SSH 키가 필요합니다:

```bash
# SSH 키 생성 (없는 경우)
ssh-keygen -t ed25519 -C "your-email@example.com"

# 공개 키 복사
cat ~/.ssh/id_ed25519.pub | pbcopy

# GitHub → Settings → SSH and GPG keys → New SSH key에 붙여넣기
```

각 기기에서 동일한 과정을 반복합니다.

## 6. Obsidian 연동

### Obsidian에서 vault 열기

1. Obsidian → Open folder as vault
2. `~/Documents/Obsidian Vault/seCall` 선택
3. Settings → Files & Links → Excluded files에 `raw/sessions` 추가 (선택)

### Obsidian Git 플러그인 (선택)

Obsidian 내에서 자동 commit/push를 원하면:

1. Community plugins → Obsidian Git 설치
2. Settings:
   - Auto pull interval: 5 minutes
   - Auto push interval: 5 minutes
   - Pull on startup: enabled
3. seCall의 hook 동기화와 **중복 실행되지 않도록** 주의

> Obsidian Git을 사용하면 seCall의 `secall sync` 대신 Obsidian이 git을 관리합니다. 둘 중 하나만 사용하는 것을 권장합니다.

## 7. .gitignore

`secall init --git` 실행 시 자동 생성됩니다:

```
# seCall vault .gitignore
*.db
*.db-wal
*.db-shm
*.usearch
.DS_Store
.obsidian/
```

| 제외 항목 | 이유 |
|---|---|
| `*.db*` | SQLite DB는 로컬 캐시 — reindex로 재구축 가능 |
| `*.usearch` | ANN 인덱스 파일 — 로컬 빌드 |
| `.obsidian/` | Obsidian 설정은 기기별로 다름 |

## 8. 문제 해결

### Push 실패: "rejected — non-fast-forward"

다른 기기에서 먼저 push한 경우:

```bash
cd ~/Documents/Obsidian\ Vault/seCall
git pull --rebase origin main
secall sync
```

### DB와 vault 불일치

DB를 재구축하면 해결됩니다:

```bash
# DB 삭제 후 vault에서 재구축
rm ~/.config/secall/secall.db
secall reindex --from-vault
```

### "git not found" 에러

```bash
# macOS
xcode-select --install

# 또는 Homebrew
brew install git
```

## 9. 보안 고려사항

- **반드시 private 저장소**를 사용하세요
- vault에는 AI 대화 내용, 프로젝트 경로, 코드 스니펫이 포함됩니다
- GitHub Enterprise 또는 self-hosted Git을 사용하면 추가 보안을 확보할 수 있습니다
- 민감한 프로젝트의 세션은 `.secallignore` (향후 구현 예정)로 ingest에서 제외할 수 있습니다

## 요약

```
첫 번째 기기:  secall init --vault <path> --git <remote>
다른 기기:     git clone <remote> <path> && secall init --vault <path>
매일:          secall sync (또는 Claude Code hook으로 자동)
복구:          secall reindex --from-vault
```
