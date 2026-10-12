import { openUrl } from "@tauri-apps/plugin-opener";

/** Opens a link from the terminal in the default browser. Only web links pass. */
export async function openLink(uri: string) {
  if (!/^https?:\/\//i.test(uri)) return;
  try {
    await openUrl(uri);
  } catch {
    // The dev mock runs in a plain browser.
    window.open(uri, "_blank", "noopener");
  }
}
