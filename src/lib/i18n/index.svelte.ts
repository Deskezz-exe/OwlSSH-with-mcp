import en, { type MessageKey } from "./en";
import ru from "./ru";

export type Locale = "en" | "ru";
export type { MessageKey };

export const LOCALES: { id: Locale; label: string }[] = [
  { id: "en", label: "English" },
  { id: "ru", label: "Русский" },
];

const dictionaries: Record<Locale, Record<MessageKey, string>> = { en, ru };

class I18n {
  locale = $state<Locale>("en");

  set(locale: string) {
    this.locale = locale in dictionaries ? (locale as Locale) : "en";
    document.documentElement.lang = this.locale;
  }
}

export const i18n = new I18n();

/**
 * Translates a key and fills `{placeholders}`. Reading `i18n.locale` makes
 * every template that calls it update when the language changes.
 */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const text = dictionaries[i18n.locale][key] ?? en[key] ?? key;
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}
