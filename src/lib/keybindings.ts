// Shortcuts are stored as text like "Ctrl+Shift+C".
//
// Keys are matched by their physical position (KeyboardEvent.code), not by the
// character they type. Otherwise Ctrl+Shift+C stops working as soon as the
// keyboard layout is switched to Russian, where the same key types "С".

const MODIFIERS = ["Ctrl", "Alt", "Shift", "Meta"] as const;
const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "AltGraph", "CapsLock"]);

/** The layout-independent name of the key that was pressed. */
function keyName(e: KeyboardEvent): string {
  const code = e.code;
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Numpad")) return code;
  const named: Record<string, string> = {
    Space: "Space",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Backquote: "`",
  };
  if (named[code]) return named[code];
  return e.key.length === 1 ? e.key.toUpperCase() : e.key;
}

/** "Ctrl+Shift+C" for the pressed keys, or null while only modifiers are held. */
export function comboFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Meta");
  parts.push(keyName(e));
  return parts.join("+");
}

/** Splits a stored shortcut into its modifiers and key, in canonical order. */
export function parseCombo(combo: string): { modifiers: Set<string>; key: string } {
  const parts = combo.split("+").map((p) => p.trim()).filter(Boolean);
  const key = parts.pop() ?? "";
  return { modifiers: new Set(parts), key: key.length === 1 ? key.toUpperCase() : key };
}

export function matches(e: KeyboardEvent, combo: string): boolean {
  if (!combo) return false;
  const { modifiers, key } = parseCombo(combo);
  return (
    e.ctrlKey === modifiers.has("Ctrl") &&
    e.altKey === modifiers.has("Alt") &&
    e.shiftKey === modifiers.has("Shift") &&
    e.metaKey === modifiers.has("Meta") &&
    keyName(e) === key
  );
}

/** Shortcuts without a modifier would steal normal typing in the terminal. */
export function hasModifier(combo: string): boolean {
  const { modifiers, key } = parseCombo(combo);
  const functionKey = /^F\d{1,2}$/.test(key) || key === "Insert";
  return functionKey || MODIFIERS.some((m) => modifiers.has(m));
}

export function sameCombo(a: string, b: string): boolean {
  const pa = parseCombo(a);
  const pb = parseCombo(b);
  return pa.key === pb.key && pa.modifiers.size === pb.modifiers.size && [...pa.modifiers].every((m) => pb.modifiers.has(m));
}
