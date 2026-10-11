<!-- Small line icons drawn on a 24px grid, stroke follows the text color. -->
<script lang="ts" module>
  const PATHS = {
    folder: "M3 7.5A1.5 1.5 0 0 1 4.5 6H9l2 2.2h8.5A1.5 1.5 0 0 1 21 9.7v8.8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z",
    file: "M6 3h8l4 4v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM14 3v4h4",
    text: "M6 3h8l4 4v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM14 3v4h4M8.5 12h7M8.5 15h7M8.5 18h4",
    code: "M6 3h8l4 4v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM14 3v4h4M10 12l-2 2.5 2 2.5M14 12l2 2.5-2 2.5",
    image: "M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zM3.5 16l5-5 4 4 2.5-2.5 5.5 5.5M15.5 9.5h.01",
    archive: "M4 4h16v4H4zM5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8M10 12h4",
    link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
    up: "M12 19V5M6 11l6-6 6 6",
    refresh: "M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7",
    folderPlus: "M3 7.5A1.5 1.5 0 0 1 4.5 6H9l2 2.2h8.5A1.5 1.5 0 0 1 21 9.7v8.8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5zM12 11.5v5M9.5 14h5",
    pencil: "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
    laptop: "M5 5h14a1 1 0 0 1 1 1v9H4V6a1 1 0 0 1 1-1zM2 18h20",
    server: "M4 4h16v6H4zM4 14h16v6H4zM8 7h.01M8 17h.01",
    right: "M5 12h14M13 6l6 6-6 6",
    left: "M19 12H5M11 6l-6 6 6 6",
    x: "M6 6l12 12M18 6L6 18",
    check: "M5 12.5l4.5 4.5L19 7",
    chevron: "M9 6l6 6-6 6",
    sortUp: "M12 6l-5 6h10z",
    sortDown: "M12 18l-5-6h10z",
    trash: "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
  } as const;

  export type IconName = keyof typeof PATHS;

  const EXT: Record<string, IconName> = {};
  for (const e of ["txt", "md", "log", "csv", "conf", "cfg", "ini", "env", "yml", "yaml", "toml", "json", "xml"]) EXT[e] = "text";
  for (const e of ["js", "ts", "jsx", "tsx", "py", "rs", "go", "php", "rb", "java", "c", "cpp", "h", "sh", "bash", "sql", "html", "css", "scss", "svelte", "vue", "lua"]) EXT[e] = "code";
  for (const e of ["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp", "avif"]) EXT[e] = "image";
  for (const e of ["zip", "gz", "tgz", "tar", "bz2", "xz", "7z", "rar", "zst", "deb", "rpm"]) EXT[e] = "archive";

  /** Icon for a file name by its extension. */
  export function iconForFile(name: string): IconName {
    const ext = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
    return EXT[ext] ?? "file";
  }
</script>

<script lang="ts">
  let { name, size = 16, stroke = 1.7 }: { name: IconName; size?: number; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={PATHS[name]} />
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
