import type { TransferSource } from "./api";
import { dialogs } from "./stores/dialogs.svelte";

/** "report.pdf" -> "report (1).pdf", folders and dotfiles keep their whole name as the base. */
export function numberedName(name: string, n: number, isDir: boolean): string {
  const dot = name.lastIndexOf(".");
  if (isDir || dot <= 0) return `${name} (${n})`;
  return `${name.slice(0, dot)} (${n})${name.slice(dot)}`;
}

export function baseName(path: string): string {
  return path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? path;
}

interface Item {
  path: string;
  isDir: boolean;
}

/**
 * Checks the destination for name clashes and asks the user about each one.
 * Returns the sources to transfer (possibly renamed), or null when nothing is left.
 */
export async function resolveConflicts(
  items: Item[],
  side: "remote" | "local",
  join: (name: string) => string,
  exists: (paths: string[]) => Promise<boolean[]>,
): Promise<TransferSource[] | null> {
  const names = items.map((i) => baseName(i.path));
  const taken = await exists(names.map(join));
  const clashes = items.map((_, i) => i).filter((i) => taken[i]);
  if (!clashes.length) return items.map((i) => ({ path: i.path }));

  const isTaken = async (name: string) => (await exists([join(name)]))[0] || names.includes(name);
  async function freeName(name: string, isDir: boolean): Promise<string> {
    const candidates = Array.from({ length: 30 }, (_, n) => numberedName(name, n + 1, isDir));
    const used = await exists(candidates.map(join));
    return candidates.find((c, i) => !used[i] && !names.includes(c)) ?? numberedName(name, Date.now(), isDir);
  }

  const out = new Map<number, TransferSource | null>();
  let remembered: "replace" | "rename" | "cancel" | null = null;
  for (const [k, i] of clashes.entries()) {
    const item = items[i];
    const suggestion = await freeName(names[i], item.isDir);
    let action = remembered;
    let chosenName = suggestion;
    if (!action) {
      const answer = await dialogs.askConflict({
        name: names[i],
        isDir: item.isDir,
        side,
        suggestion,
        remaining: clashes.length - k - 1,
        isTaken,
      });
      action = answer.action;
      if (answer.action === "rename") chosenName = answer.name;
      if (answer.all) remembered = answer.action;
    }
    out.set(i, action === "replace" ? { path: item.path } : action === "rename" ? { path: item.path, name: chosenName } : null);
    if (action === "rename") names.push(chosenName);
  }

  const result = items
    .map((item, i) => (out.has(i) ? out.get(i)! : { path: item.path }))
    .filter((s): s is TransferSource => s !== null);
  return result.length ? result : null;
}
