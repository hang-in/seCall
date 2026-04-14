---
type: task
status: ready
plan: p25-semantic-graph-obsidian-phase-0-1
task_number: 3
title: 세션 조회 + 상태바
updated_at: 2026-04-14
---

# Task 03 — 세션 조회 + 상태바

## 목표

검색 결과 클릭 시 vault 내 세션 MD 파일을 열거나 세션 상세 뷰를 표시하고,
하단 상태바에 seCall 인덱스 상태를 표시한다.

## Changed files

### 1. `obsidian-secall/src/search-view.ts` (수정)

검색 결과 아이템 클릭 핸들러 추가:

```typescript
// secall-result-item 클릭 이벤트
item.addEventListener("click", () => {
  this.openSession(r);
});

async openSession(result: SearchResult) {
  // 1. vault_path가 있으면 vault 내 MD 파일 열기
  if (result.vault_path) {
    const file = this.app.vault.getAbstractFileByPath(result.vault_path);
    if (file) {
      await this.app.workspace.getLeaf(false).openFile(file as TFile);
      return;
    }
  }

  // 2. vault_path가 없으면 SessionView에서 API로 조회
  const leaf = this.app.workspace.getLeaf(false);
  await leaf.setViewState({
    type: SESSION_VIEW_TYPE,
    state: { sessionId: result.session_id },
  });
  this.app.workspace.revealLeaf(leaf);
}
```

**vault_path 매핑 주의**: REST API recall 결과의 `vault_path`는 절대 경로.
Obsidian vault 기준 상대 경로로 변환 필요:
```typescript
// vault root 경로 제거하여 상대 경로 추출
const vaultRoot = (this.app.vault.adapter as any).basePath;
const relativePath = result.vault_path.replace(vaultRoot + "/", "");
const file = this.app.vault.getAbstractFileByPath(relativePath);
```

### 2. `obsidian-secall/src/session-view.ts` (신규)

vault에 MD 파일이 없는 세션용 상세 뷰:

```typescript
import { ItemView, WorkspaceLeaf, MarkdownRenderer } from "obsidian";
import type SeCallPlugin from "./main";

export const SESSION_VIEW_TYPE = "secall-session";

export class SessionView extends ItemView {
  plugin: SeCallPlugin;
  sessionId: string;

  constructor(leaf: WorkspaceLeaf, plugin: SeCallPlugin) {
    super(leaf);
    this.plugin = plugin;
  }

  getViewType() { return SESSION_VIEW_TYPE; }
  getDisplayText() { return `Session: ${this.sessionId || "..."}` ; }
  getIcon() { return "file-text"; }

  async setState(state: { sessionId: string }, result: any) {
    this.sessionId = state.sessionId;
    await this.render();
    await super.setState(state, result);
  }

  getState() {
    return { sessionId: this.sessionId };
  }

  async render() {
    const container = this.containerEl.children[1];
    container.empty();

    if (!this.sessionId) {
      container.createEl("div", { text: "No session selected." });
      return;
    }

    container.createEl("div", { text: "Loading...", cls: "secall-loading" });

    try {
      const data = await this.plugin.api.get(this.sessionId, true);
      container.empty();

      // 메타데이터 헤더
      const header = container.createDiv({ cls: "secall-session-header" });
      header.createEl("h3", { text: data.summary || this.sessionId });
      header.createEl("div", {
        text: `${data.project || "?"} · ${data.agent} · ${data.start_time}`,
        cls: "secall-result-meta",
      });

      // 본문 (Markdown 렌더링)
      if (data.content) {
        const contentEl = container.createDiv({ cls: "secall-session-content" });
        await MarkdownRenderer.render(
          this.app, data.content, contentEl, "", this.plugin
        );
      }
    } catch (e) {
      container.empty();
      container.createEl("div", {
        text: `Error: ${e instanceof Error ? e.message : String(e)}`,
        cls: "secall-error",
      });
    }
  }
}
```

### 3. `obsidian-secall/src/main.ts` (수정)

상태바 + SessionView 등록:

```typescript
import { SessionView, SESSION_VIEW_TYPE } from "./session-view";

export default class SeCallPlugin extends Plugin {
  statusBarEl: HTMLElement;

  async onload() {
    // ... 기존 코드 ...

    // SessionView 등록
    this.registerView(SESSION_VIEW_TYPE, (leaf) => new SessionView(leaf, this));

    // 상태바
    this.statusBarEl = this.addStatusBarItem();
    this.statusBarEl.setText("seCall: connecting...");
    this.refreshStatus();

    // 5분마다 상태 갱신
    this.registerInterval(
      window.setInterval(() => this.refreshStatus(), 300_000)
    );
  }

  async refreshStatus() {
    try {
      const stats = await this.api.status();
      const vectorIcon = stats.vectors > 0 ? "✓" : "✗";
      this.statusBarEl.setText(
        `seCall: ${stats.sessions} sessions, vectors ${vectorIcon}`
      );
    } catch {
      this.statusBarEl.setText("seCall: offline");
    }
  }
}
```

### 4. `obsidian-secall/styles.css` (수정)

세션 뷰 스타일 추가:
```css
.secall-session-header { padding: 12px; border-bottom: 1px solid var(--background-modifier-border); }
.secall-session-header h3 { margin: 0 0 4px 0; }
.secall-session-content { padding: 12px; }
```

## Change description

1. `search-view.ts`: 검색 결과 클릭 → vault MD 파일 열기 (vault_path 있을 때) 또는 SessionView 열기
2. `session-view.ts` 신규: GET /api/get 호출 → 세션 메타 + Markdown 본문 렌더링
3. `main.ts`: SessionView 등록, 상태바 위젯 (status API 주기적 호출)
4. `styles.css`: 세션 뷰 스타일 추가

## Dependencies

- Task 02 완료 필수 (플러그인 scaffold + SearchView 기반)
- Task 01 완료 필수 (GET /api/get, GET /api/status 엔드포인트)

## Verification

```bash
# 1. TypeScript 타입 체크
cd obsidian-secall && npx tsc --noEmit --skipLibCheck

# 2. esbuild 번들 생성
cd obsidian-secall && npm run build

# 3. 번들 파일 크기 확인 (너무 크지 않은지)
ls -la obsidian-secall/main.js

# 4. Manual: Obsidian에서 플러그인 테스트
# - secall serve --port 8080 실행 상태에서
# - Obsidian 재시작 → 플러그인 활성화
# - 하단 상태바에 "seCall: N sessions, vectors ✓" 표시 확인
# - Cmd+P → "seCall: Search" → 검색 → 결과 클릭 → MD 파일 열림 확인
# - vault_path 없는 세션 클릭 → SessionView에 메타+본문 표시 확인
```

## Risks

- **vault_path 절대→상대 경로 변환**: Obsidian vault의 basePath 접근 방식이 `(adapter as any).basePath`로 비공식. `vault.adapter.getBasePath()`가 있는지 확인 필요. 없으면 설정에서 vault root 경로를 입력받는 대안.
- **MarkdownRenderer.render**: 5번째 인자 (Component)가 버전에 따라 다를 수 있음. obsidian@1.7.0 기준 확인.
- **상태바 polling**: 5분 간격이면 서버 부하 미미. offline 시 에러 무시 처리 완료.

## Scope boundary

수정 금지 파일:
- `crates/` — Rust 코드 일체 변경 없음
- `obsidian-secall/src/api.ts` — Task 02에서 완성, 이 Task에서 변경 불필요
- `obsidian-secall/src/settings.ts` — Task 02에서 완성, 이 Task에서 변경 불필요
