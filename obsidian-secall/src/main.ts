import { Plugin } from "obsidian";
import { SeCallSettingTab, type SeCallSettings, DEFAULT_SETTINGS } from "./settings";
import { SearchView, SEARCH_VIEW_TYPE } from "./search-view";
import { SessionView, SESSION_VIEW_TYPE } from "./session-view";
import { SeCallApi } from "./api";

export default class SeCallPlugin extends Plugin {
  settings!: SeCallSettings;
  api!: SeCallApi;
  statusBarEl!: HTMLElement;

  async onload() {
    await this.loadSettings();
    this.api = new SeCallApi(this.settings.serverUrl);

    this.registerView(SEARCH_VIEW_TYPE, (leaf) => new SearchView(leaf, this));
    this.registerView(SESSION_VIEW_TYPE, (leaf) => new SessionView(leaf, this));

    this.addCommand({
      id: "secall-search",
      name: "Search",
      callback: () => this.openSearchView(),
    });

    this.addRibbonIcon("search", "seCall Search", () => this.openSearchView());

    this.addSettingTab(new SeCallSettingTab(this.app, this));

    // 상태바
    this.statusBarEl = this.addStatusBarItem();
    this.statusBarEl.setText("seCall: connecting...");
    this.refreshStatus();

    // 5분마다 상태 갱신
    this.registerInterval(
      window.setInterval(() => this.refreshStatus(), 300_000)
    );
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

  async refreshStatus() {
    try {
      const stats = await this.api.status();
      const vectorIcon = stats.vectors > 0 ? "\u2713" : "\u2717";
      this.statusBarEl.setText(
        `seCall: ${stats.sessions} sessions, vectors ${vectorIcon}`
      );
    } catch {
      this.statusBarEl.setText("seCall: offline");
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
