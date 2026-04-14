import { ItemView, MarkdownRenderer, type ViewStateResult, WorkspaceLeaf } from "obsidian";
import type SeCallPlugin from "./main";

export const SESSION_VIEW_TYPE = "secall-session";

export class SessionView extends ItemView {
  plugin: SeCallPlugin;
  sessionId!: string;

  constructor(leaf: WorkspaceLeaf, plugin: SeCallPlugin) {
    super(leaf);
    this.plugin = plugin;
  }

  getViewType() {
    return SESSION_VIEW_TYPE;
  }

  getDisplayText() {
    return `Session: ${this.sessionId || "..."}`;
  }

  getIcon() {
    return "file-text";
  }

  async setState(state: { sessionId: string }, result: ViewStateResult) {
    this.sessionId = state.sessionId;
    await this.render();
    await super.setState(state, result);
  }

  getState() {
    return { sessionId: this.sessionId };
  }

  async render() {
    const container = this.containerEl.children[1] as HTMLElement;
    container.empty();

    if (!this.sessionId) {
      container.createEl("div", { text: "No session selected." });
      return;
    }

    container.createEl("div", { text: "Loading...", cls: "secall-loading" });

    try {
      const data = await this.plugin.api.get(this.sessionId, true);
      container.empty();

      const header = container.createDiv({ cls: "secall-session-header" });
      header.createEl("h3", { text: data.summary || this.sessionId });
      header.createEl("div", {
        text: `${data.project || "?"} \u00b7 ${data.agent} \u00b7 ${data.date}`,
        cls: "secall-result-meta",
      });

      if (data.content) {
        const contentEl = container.createDiv({
          cls: "secall-session-content",
        });
        await MarkdownRenderer.render(
          this.app,
          data.content,
          contentEl,
          "",
          this.plugin
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
