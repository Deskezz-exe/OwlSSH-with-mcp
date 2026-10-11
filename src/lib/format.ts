import { i18n, t } from "./i18n/index.svelte";

/** 1536 KiB -> "1.5 GB" style sizes, using the UI language for the decimal sign. */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const digits = value >= 100 || unit === 0 ? 0 : 1;
  return `${value.toLocaleString(i18n.locale, { maximumFractionDigits: digits })} ${units[unit]}`;
}

export const formatKb = (kb: number) => formatBytes(kb * 1024);

/** 266400 -> "3d 2h", 4000 -> "1h 6m". Two largest units only. */
export function formatDuration(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const parts: string[] = [];
  if (d) parts.push(`${d}${t("units.day")}`);
  if (h) parts.push(`${h}${t("units.hour")}`);
  if (!d && m) parts.push(`${m}${t("units.minute")}`);
  return parts.slice(0, 2).join(" ") || `0${t("units.minute")}`;
}

export function percent(used: number | null, total: number | null): number | null {
  if (used === null || !total) return null;
  return Math.min(100, (used / total) * 100);
}
