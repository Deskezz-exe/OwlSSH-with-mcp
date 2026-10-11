import { api, type Snippet } from "../api";

class SnippetsState {
  list = $state<Snippet[]>([]);

  async load() {
    this.list = await api.listSnippets().catch(() => []);
  }

  async save(input: Omit<Snippet, "id"> & { id: string | null }): Promise<Snippet> {
    const saved = await api.saveSnippet(input);
    const i = this.list.findIndex((s) => s.id === saved.id);
    if (i >= 0) this.list[i] = saved;
    else this.list.push(saved);
    return saved;
  }

  async remove(id: string) {
    await api.deleteSnippet(id);
    this.list = this.list.filter((s) => s.id !== id);
  }
}

export const snippets = new SnippetsState();

/** Lets the command palette paste text into the terminal that is on screen. */
export const terminalPaste = new Map<string, (text: string) => void>();

/** Opening state of the command palette. */
export const palette = $state({ open: false });
