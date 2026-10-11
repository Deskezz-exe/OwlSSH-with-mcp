import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

// The Tauri clipboard plugin works without the webview's permission prompts;
// the browser API is the fallback for the dev mock.
export async function copyText(text: string) {
  try {
    await writeText(text);
  } catch {
    await navigator.clipboard.writeText(text);
  }
}

export async function pasteText(): Promise<string> {
  try {
    return (await readText()) ?? "";
  } catch {
    return navigator.clipboard.readText();
  }
}
