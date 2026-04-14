import { ItemView, TFile, WorkspaceLeaf } from "obsidian";
import type SeCallPlugin from "./main";
import { SESSION_VIEW_TYPE } from "./session-view";

interface SearchResultMeta {
  agent: string;
  project?: string;
  date: string;
  vault_path?: string;
  summary?: string;
}

interface SearchResult {
  session_id: string;
  snippet?: string;
  metadata: SearchResultMeta;
}

export const SEARCH_VIEW_TYPE = "secall-search";

export class SearchView extends ItemView {
  plugin: SeCallPlugin;
  resultsEl!: HTMLElement;

  constructor(leaf: WorkspaceLeaf, plugin: SeCallPlugin) {
    super(leaf);
    this.plugin = plugin;
  }

  getViewType() {
    return SEARCH_VIEW_TYPE;
  }

  getDisplayText() {
    return "seCall Search";
  }

  getIcon() {
    return "search";
  }

  async onOpen() {
    const container = this.containerEl.children[1] as HTMLElement;
    container.empty();

    const searchBar = container.createDiv({ cls: "secall-search-bar" });
    const input = searchBar.createEl("input", {
      type: "text",
      placeholder: "Search sessions...",
      cls: "secall-search-input",
    });
    input.addEventListener("keydown", (e: KeyboardEvent) => {
      if (e.key === "Enter") this.doSearch(input.value);
    });

    this.resultsEl = container.createDiv({ cls: "secall-results" });
  }

  async openSession(result: SearchResult) {
    const vaultPath = result.metadata.vault_path;
    if (vaultPath) {
      // vault_path는 절대 경로 — vault root 기준 상대 경로로 변환
      const adapter = this.app.vault.adapter as any;
      const vaultRoot: string = adapter.basePath || "";
      const relativePath =
        vaultRoot && vaultPath.startsWith(vaultRoot + "/")
          ? vaultPath.slice(vaultRoot.length + 1)
          : vaultPath;
      const file = this.app.vault.getAbstractFileByPath(relativePath);
      if (file instanceof TFile) {
        await this.app.workspace.getLeaf(false).openFile(file);
        return;
      }
    }

    // vault 파일이 없으면 SessionView로 API 조회
    const leaf = this.app.workspace.getLeaf(false);
    await leaf.setViewState({
      type: SESSION_VIEW_TYPE,
      state: { sessionId: result.session_id },
    });
    this.app.workspace.revealLeaf(leaf);
  }

  async doSearch(query: string) {
    if (!query.trim()) return;
    this.resultsEl.empty();
    this.resultsEl.createEl("div", {
      text: "Searching...",
      cls: "secall-loading",
    });

    try {
      const data = await this.plugin.api.recall(query);
      this.resultsEl.empty();

      if (!data.results || data.results.length === 0) {
        this.resultsEl.createEl("div", { text: "No results found." });
        return;
      }

      for (const r of data.results) {
        const meta = r.metadata;
        const item = this.resultsEl.createDiv({ cls: "secall-result-item" });
        item.createEl("div", {
          text: meta.summary || r.session_id,
          cls: "secall-result-title",
        });
        item.createEl("div", {
          text: `${meta.project || "?"} \u00b7 ${meta.agent} \u00b7 ${meta.date}`,
          cls: "secall-result-meta",
        });
        if (r.snippet) {
          item.createEl("div", {
            text: r.snippet,
            cls: "secall-result-snippet",
          });
        }
        item.addEventListener("click", () => this.openSession(r));
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
