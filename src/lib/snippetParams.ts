// `{{name}}` placeholders in snippets. Docker-style `{{.Field}}` templates are
// left alone because a placeholder name has to start with a letter.
const PARAM = /\{\{\s*([A-Za-z_][\w-]*)\s*\}\}/g;

/** Placeholder names in order of first appearance. */
export function paramsOf(command: string): string[] {
  const names: string[] = [];
  for (const match of command.matchAll(PARAM)) {
    if (!names.includes(match[1])) names.push(match[1]);
  }
  return names;
}

export function fillParams(command: string, values: Record<string, string>): string {
  return command.replace(PARAM, (whole, name: string) => values[name] ?? whole);
}
