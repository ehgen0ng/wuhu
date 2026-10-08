import zhCN from "./locales/zh-CN.json";
import en from "./locales/en.json";

export type Locale = "zh-CN" | "en";
export type MessageKey = keyof typeof zhCN;
export type LocalizedText = string | Message;
export type Variables = Record<string, string | number | Date | Message>;
export type Message = { key: MessageKey; variables: Variables };

export const DEFAULT_LOCALE: Locale = "zh-CN";
const resources: Record<Locale, Record<MessageKey, string>> = { "zh-CN": zhCN, en };

export function isLocale(value: unknown): value is Locale {
  return value === "zh-CN" || value === "en";
}

/** Translation only: the application owns language selection and persistence. */
export function t(locale: Locale, key: MessageKey, variables: Variables = {}): string {
  const template = resources[locale]?.[key] ?? zhCN[key];
  return template.replace(/\{(\w+)\}/g, (placeholder, name: string) =>
    Object.prototype.hasOwnProperty.call(variables, name) ? formatValue(locale, variables[name]) : placeholder,
  );
}

export function message(key: MessageKey, variables: Variables = {}): Message {
  return { key, variables };
}

export function renderText(locale: Locale, text: LocalizedText): string {
  return typeof text === "string" ? text : t(locale, text.key, text.variables);
}

function formatValue(locale: Locale, value: Variables[string]): string {
  if (value instanceof Date) return formatDateTime(locale, value);
  return typeof value === "number" ? String(value) : renderText(locale, value);
}

export function formatDateTime(locale: Locale, date: Date) {
  return date.toLocaleString(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
