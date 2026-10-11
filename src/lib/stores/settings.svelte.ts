import { api, type Settings } from "../api";
import { i18n } from "../i18n/index.svelte";

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

class SettingsState {
  value = $state<Settings | null>(null);
  /** The theme actually shown, with "system" resolved. */
  resolvedTheme = $state<"dark" | "light">("dark");

  async load() {
    this.apply(await api.getSettings());
    systemDark.addEventListener("change", () => this.value && this.applyTheme(this.value));
  }

  /** Applies a change right away and persists it. */
  async update(change: (s: Settings) => void) {
    if (!this.value) return;
    const next = structuredClone($state.snapshot(this.value)) as Settings;
    change(next);
    this.apply(next);
    this.apply(await api.saveSettings(next));
  }

  private apply(s: Settings) {
    this.value = s;
    i18n.set(s.locale);
    this.applyTheme(s);
  }

  private applyTheme(s: Settings) {
    const theme = s.theme === "system" ? (systemDark.matches ? "dark" : "light") : s.theme;
    this.resolvedTheme = theme;
    document.documentElement.dataset.theme = theme;
  }
}

export const settings = new SettingsState();
