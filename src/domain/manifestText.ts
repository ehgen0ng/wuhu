import { t, type Locale } from "../i18n";
import { renderError } from "../i18n/errors";
import type { SteamSearchResult } from "../types";
import { formatFileSize, formatManifestTime } from "./display";
import { canAddManifest } from "./manifest";

export function manifestStatusText(item: SteamSearchResult, locale: Locale) {
  if (!item.manifestChecked) return null;
  const status = item.manifestStatus;
  if (!status) return null;
  if (status.updateInProgress) return null;
  if (!canAddManifest(item)) return null;
  const size = formatFileSize(status.fileSize);
  if (!status.fileModified) {
    return t(locale, "manifest.availableSummary", { size: size ? ` · ${size}` : "" });
  }
  return t(locale, "manifest.updatedSummary", { time: formatManifestTime(status.fileModified, locale), size: size ? ` · ${size}` : "" });
}

export function manifestIssueText(item: SteamSearchResult, locale: Locale) {
  if (canAddManifest(item)) return null;
  if (item.manifestChecking) return t(locale, "manifest.checking");
  if (!item.manifestChecked) return t(locale, "manifest.notChecked");

  const status = item.manifestStatus;
  if (!status) return t(locale, "manifest.unknown");
  if (status.error) return renderError(locale, status.error);
  if (status.updateInProgress) return t(locale, "manifest.updating");
  if (!status.manifestFileExists) return t(locale, "manifest.notFound");
  if (status.status) return t(locale, "manifest.status", { status: status.status });
  return t(locale, "manifest.unavailable");
}
