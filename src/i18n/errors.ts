import zhCN from "./locales/zh-CN.json";
import { renderText, t, type Locale, type LocalizedText, type Message, type MessageKey } from ".";

export class LocalizedError extends Error {
  constructor(readonly text: Message) {
    super(text.key);
  }
}

export function errorText(error: unknown): LocalizedText {
  return error instanceof LocalizedError ? error.text : String(error);
}

// Rust commands currently return strings. Translate known messages at the display
// boundary; retain paths, service responses and unknown diagnostics verbatim.
const errorKeys = new Map<string, MessageKey>(
  Object.entries(zhCN)
    .filter(([key]) => key.startsWith("error."))
    .map(([key, text]) => [text, key as MessageKey]),
);

export function renderError(locale: Locale, text: LocalizedText): string {
  if (typeof text !== "string") return renderText(locale, text);
  if (locale === "zh-CN") return text;
  const prefix = text.startsWith("Error: ") ? "Error: " : "";
  const raw = text.slice(prefix.length);
  const key = errorKeys.get(raw);
  if (key) return prefix + t(locale, key);
  const separator = raw.indexOf("：");
  const headingKey = errorKeys.get(raw.slice(0, separator));
  return separator >= 0 && headingKey
    ? prefix + t(locale, headingKey) + ": " + raw.slice(separator + 1)
    : text;
}
