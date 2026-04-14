---
type: task
status: ready
plan: p25-semantic-graph-obsidian-phase-0-1
task_number: 2
title: Obsidian 플러그인 scaffold + recall
updated_at: 2026-04-14
---

# Task 02 — Obsidian 플러그인 scaffold + recall

## 목표

obsidian-sample-plugin 템플릿 기반으로 `obsidian-secall/` 디렉토리를 생성하고,
REST API를 통한 검색 기능(recall)을 구현한다.

## Changed files

### 1. `obsidian-secall/package.json` (신규)

```json
{
  "name": "obsidian-secall",
  "version": "0.1.0",
  "description": "seCall session search for Obsidian",
  "main": "main.js",
  "scripts": {
    "dev": "node esbuild.config.mjs",
    "build": "tsc -noEmit -skipLibCheck && node esbuild.config.mjs production"
  },
  "devDependencies": {
    "@types/node": "^22.0.0",
    "esbuild": "^0.24.0",
    "obsidian": "^1.7.0",
    "typescript": "^5.6.0",
    "tslib": "^2.7.0"
  }
}
```

### 2. `obsidian-secall/tsconfig.json` (신규)

obsidian-sample-plugin 표준 설정:
- `target: ES2018`, `module: ESNext`, `moduleResolution: node`
- `strict: true`, `noImplicitAny: true`
- `outDir: .` (esbuild가 번들링하므로 tsc는 타입 체크용)

### 3. `obsidian-secall/esbuild.config.mjs` (신규)

obsidian-sample-plugin 표준 esbuild 설정:
- entry: `src/main.ts`
- output: `main.js`
- format: `cjs`, platform: `node`
- external: `obsidian`, `electron`
- banner: `/* obsidian-secall */`

### 4. `obsidian-secall/manifest.json` (신규)

```json
{
  "id": "obsidian-secall",
  "name": "seCall",
  "version": "0.1.0",
  "minAppVersion": "1.5.0",
  "description": "Search and browse seCall agent sessions",
  "author": "d9ng",
  "isDesktopOnly": false
}
```

### 5. `obsidian-secall/src/main.ts` (신규)

Plugin 클래스 + 설정 패널:

```typescript
import { Plugin, WorkspaceLeaf } from "obsidian";
import { SeCallSettingTab, SeCallSettings, DEFAULT_SETTINGS } from "./settings";
import { SearchView, SEARCH_VIEW_TYPE } from "./search-view";
import { SeCallApi } from "./api";

export default class SeCallPlugin extends Plugin {
  settings: SeCallSettings;
  api: SeCallApi;

  async onload() {
    await this.loadSettings();
    this.api = new SeCallApi(this.settings.serverUrl);

    // 검색 뷰 등록
    this.registerView(SEARCH_VIEW_TYPE, (leaf) => new SearchView(leaf, this));

    // 커맨드 팔레트: seCall: Search
    this.addCommand({
      id: "secall-search",
      name: "Search",
      callback: () => this.openSearchView(),
    });

    // 리본 아이콘
    this.addRibbonIcon("search", "seCall Search", () => this.openSearchView());

    // 설정 탭
    this.addSettingTab(new SeCallSettingTab(this.app, this));
  }

  async openSearchView() {
    const existing = this.app.workspace.getLeavesOfType(SEARCH_VIEW_TYPE);
    if (existing.length > 0) {
      this.app.workspace.revealLeaf(existing[0]);
      return;
    }
    const leaf = this.app.workspace.getRightLeaf(false);
    if (leaf) {
      await leaf.setViewState({ type: SEARCH_VIEW_TYPE, active: true });
      this.app.workspace.revealLeaf(leaf);
    }
  }

  async loadSettings() {
    this.settings = Object.assign({}, DEFAULT_SETTINGS, await this.loadData());
  }

  async saveSettings() {
    await this.saveData(this.settings);
    this.api = new SeCallApi(this.settings.serverUrl);
  }
}
```

### 6. `obsidian-secall/src/settings.ts` (신규)

설정 인터페이스 + SettingTab:

```typescript
export interface SeCallSettings {
  serverUrl: string;
}

export const DEFAULT_SETTINGS: SeCallSettings = {
  serverUrl: "http://127.0.0.1:8080",
};
```

SettingTab: 서버 주소 입력 필드 1개.

### 7. `obsidian-secall/src/api.ts` (신규)

REST API 클라이언트 (Obsidian requestUrl 사용):

```typescript
import { requestUrl } from "obsidian";

export class SeCallApi {
  constructor(private baseUrl: string) {}

  async recall(query: string, limit = 10) {
    const resp = await requestUrl({
      url: `${this.baseUrl}/api/recall`,
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        queries: [{ type: "keyword", query }],
        limit,
      }),
    });
    return resp.json;
  }

  async get(id: string, full = false) {
    const resp = await requestUrl({
      url: `${this.baseUrl}/api/get`,
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id, full }),
    });
    return resp.json;
  }

  async status() {
    const resp = await requestUrl({
      url: `${this.baseUrl}/api/status`,
      method: "GET",
    });
    return resp.json;
  }
}
```

### 8. `obsidian-secall/src/search-view.ts` (신규)

ItemView: 사이드바 검색 패널.

```typescript
import { ItemView, WorkspaceLeaf, TextComponent } from "obsidian";
import type SeCallPlugin from "./main";

export const SEARCH_VIEW_TYPE = "secall-search";

export class SearchView extends ItemView {
  plugin: SeCallPlugin;
  searchInput: TextComponent;
  resultsEl: HTMLElement;

  constructor(leaf: WorkspaceLeaf, plugin: SeCallPlugin) {
    super(leaf);
    this.plugin = plugin;
  }

  getViewType() { return SEARCH_VIEW_TYPE; }
  getDisplayText() { return "seCall Search"; }
  getIcon() { return "search"; }

  async onOpen() {
    const container = this.containerEl.children[1];
    container.empty();

    // 검색 입력
    const searchBar = container.createDiv({ cls: "secall-search-bar" });
    const input = searchBar.createEl("input", {
      type: "text",
      placeholder: "Search sessions...",
      cls: "secall-search-input",
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") this.doSearch(input.value);
    });

    this.resultsEl = container.createDiv({ cls: "secall-results" });
  }

  async doSearch(query: string) {
    if (!query.trim()) return;
    this.resultsEl.empty();
    this.resultsEl.createEl("div", { text: "Searching...", cls: "secall-loading" });

    try {
      const data = await this.plugin.api.recall(query);
      this.resultsEl.empty();

      if (!data.results || data.results.length === 0) {
        this.resultsEl.createEl("div", { text: "No results found." });
        return;
      }

      for (const r of data.results) {
        const item = this.resultsEl.createDiv({ cls: "secall-result-item" });
        item.createEl("div", { text: r.summary || r.session_id, cls: "secall-result-title" });
        item.createEl("div", {
          text: `${r.project || "?"} · ${r.agent} · ${r.date}`,
          cls: "secall-result-meta",
        });
        item.createEl("div", {
          text: r.snippet || "",
          cls: "secall-result-snippet",
        });
        // 클릭 → vault 파일 열기 (Task 03에서 구현)
      }
    } catch (e) {
      this.resultsEl.empty();
      this.resultsEl.createEl("div", {
        text: `Error: ${e instanceof Error ? e.message : String(e)}`,
        cls: "secall-error",
      });
    }
  }
}
```

### 9. `obsidian-secall/styles.css` (신규)

기본 스타일:
```css
.secall-search-bar { padding: 8px; }
.secall-search-input { width: 100%; }
.secall-result-item { padding: 8px; border-bottom: 1px solid var(--background-modifier-border); cursor: pointer; }
.secall-result-item:hover { background: var(--background-modifier-hover); }
.secall-result-title { font-weight: 600; }
.secall-result-meta { font-size: 0.85em; color: var(--text-muted); }
.secall-result-snippet { font-size: 0.9em; margin-top: 4px; }
.secall-error { color: var(--text-error); padding: 8px; }
.secall-loading { color: var(--text-muted); padding: 8px; }
```

### 10. `obsidian-secall/.gitignore` (신규)

```
node_modules/
main.js
*.js.map
```

## Change description

1. 프로젝트 루트에 `obsidian-secall/` 디렉토리 생성
2. obsidian-sample-plugin 구조를 따라 빌드 설정 (esbuild + TypeScript)
3. Plugin 클래스: 커맨드 팔레트 등록, 리본 아이콘, 설정 탭
4. API 클라이언트: `requestUrl()`로 REST API 호출 (CORS 이슈 없음)
5. SearchView: 사이드바 ItemView에 검색 입력 + 결과 목록 렌더링
6. 검색 결과 클릭 동작은 Task 03으로 위임 (placeholder)

## Dependencies

- Task 01 완료 필수 (REST API 서버가 동작해야 테스트 가능)
- npm 패키지: `obsidian`, `esbuild`, `typescript` (devDependencies)

## Verification

```bash
# 1. npm install + TypeScript 타입 체크
cd obsidian-secall && npm install && npx tsc --noEmit --skipLibCheck

# 2. esbuild 번들 생성
cd obsidian-secall && npm run build

# 3. 번들 파일 존재 확인
ls -la obsidian-secall/main.js

# 4. Manual: Obsidian vault에 플러그인 심볼릭 링크 후 테스트
# ln -s $(pwd)/obsidian-secall /path/to/vault/.obsidian/plugins/obsidian-secall
# Obsidian 재시작 → 설정 → Community plugins → obsidian-secall 활성화
# Cmd+P → "seCall: Search" → 검색어 입력 → 결과 표시 확인
```

## Risks

- **Obsidian API 버전**: `obsidian@1.7.0` 기준. requestUrl, ItemView는 안정 API로 버전 이슈 낮음.
- **requestUrl vs fetch**: Obsidian에서 `fetch()`는 CORS 제한 있음. `requestUrl()`은 Obsidian 네이티브 HTTP로 CORS 무관.
- **esbuild 호환**: obsidian-sample-plugin 표준 설정 그대로 사용하여 안정성 확보.

## Scope boundary

수정 금지 파일:
- `crates/` — Rust 코드 일체 변경 없음
- `obsidian-secall/src/session-view.ts` — Task 03 영역 (이 Task에서 생성하지 않음)
