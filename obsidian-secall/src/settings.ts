import { App, PluginSettingTab, Setting } from "obsidian";
import type SeCallPlugin from "./main";

export interface SeCallSettings {
  serverUrl: string;
}

export const DEFAULT_SETTINGS: SeCallSettings = {
  serverUrl: "http://127.0.0.1:8080",
};

export class SeCallSettingTab extends PluginSettingTab {
  plugin: SeCallPlugin;

  constructor(app: App, plugin: SeCallPlugin) {
    super(app, plugin);
    this.plugin = plugin;
  }

  display(): void {
    const { containerEl } = this;
    containerEl.empty();

    new Setting(containerEl)
      .setName("Server URL")
      .setDesc("seCall REST API server address")
      .addText((text) =>
        text
          .setPlaceholder("http://127.0.0.1:8080")
          .setValue(this.plugin.settings.serverUrl)
          .onChange(async (value) => {
            this.plugin.settings.serverUrl = value;
            await this.plugin.saveSettings();
          })
      );
  }
}
