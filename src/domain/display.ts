import { t, formatDateTime, type Locale } from "../i18n";
import type { HubcapQuota, PackageItem, SteamSearchPrice, SteamSearchResult } from "../types";

export function packageSubtitle(pkg: PackageItem, locale: Locale) {
  const app = pkg.appId ? `AppID ${pkg.appId}` : t(locale, "packages.unknownAppId");
  if (!pkg.manifestFiles.length) return app;
  return t(locale, "packages.fileCount", { app, count: pkg.manifestFiles.length });
}

export function formatSteamPrice(price: SteamSearchPrice | null, locale: Locale) {
  if (!price) return null;
  if (price.final === 0) return t(locale, "price.free");
  const value = (price.final / 100).toFixed(2);
  if (price.currency === "CNY") return `¥ ${value}`;
  return `${price.currency} ${value}`;
}

export function searchResultSubtitle(item: SteamSearchResult, locale: Locale) {
  const price = formatSteamPrice(item.price, locale);
  return price ? `AppID ${item.id} · ${price}` : `AppID ${item.id}`;
}

export function formatManifestTime(value: string | null | undefined, locale: Locale) {
  if (!value) return t(locale, "common.unknown");
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return formatDateTime(locale, date);
}

export function formatFileSize(value: number | null | undefined) {
  if (!value) return null;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

export function formatHubcapQuota(quota: HubcapQuota | null) {
  if (!quota) return "--/--";
  return `${quota.dailyUsage}/${quota.dailyLimit}`;
}

export function formatSteamVersion(version: string | null | undefined, locale: Locale) {
  if (!version) return t(locale, "common.unidentified");
  return version;
}

export function formatSteamBuildDate(seconds: number | null | undefined, locale: Locale) {
  if (!seconds) return t(locale, "common.unidentified");
  const buildDate = new Date(seconds * 1000);
  if (Number.isNaN(buildDate.getTime())) return t(locale, "common.unidentified");

  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "long",
    day: "numeric",
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit",
    timeZoneName: "shortOffset",
  }).format(buildDate);
}
