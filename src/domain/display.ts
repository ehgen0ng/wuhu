import type { HubcapQuota, PackageItem, SteamSearchPrice, SteamSearchResult } from "../types";

export function packageSubtitle(pkg: PackageItem) {
  const app = pkg.appId ? `AppID ${pkg.appId}` : "未识别 AppID";
  if (!pkg.manifestFiles.length) return app;
  return `${app} · ${pkg.manifestFiles.length} 个 manifest`;
}

export function formatSteamPrice(price: SteamSearchPrice | null) {
  if (!price) return null;
  if (price.final === 0) return "免费";
  const value = (price.final / 100).toFixed(2);
  if (price.currency === "CNY") return `¥ ${value}`;
  return `${price.currency} ${value}`;
}

export function searchResultSubtitle(item: SteamSearchResult) {
  const price = formatSteamPrice(item.price);
  return price ? `AppID ${item.id} · ${price}` : `AppID ${item.id}`;
}

export function formatManifestTime(value: string | null | undefined) {
  if (!value) return "未知";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
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

export function formatSteamVersion(version: string | null | undefined) {
  if (!version) return "未识别";
  return version;
}

export function formatSteamBuildDate(seconds: number | null | undefined) {
  if (!seconds) return "未识别";
  const buildDate = new Date(seconds * 1000);
  if (Number.isNaN(buildDate.getTime())) return "未识别";

  const weekdays = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
  const year = buildDate.getFullYear();
  const month = buildDate.getMonth() + 1;
  const day = buildDate.getDate();
  const weekday = weekdays[buildDate.getDay()];
  const hour = buildDate.getHours();
  const minute = String(buildDate.getMinutes()).padStart(2, "0");
  const offsetMinutes = -buildDate.getTimezoneOffset();
  const offsetSign = offsetMinutes >= 0 ? "+" : "-";
  const offsetAbsolute = Math.abs(offsetMinutes);
  const offsetHour = String(Math.floor(offsetAbsolute / 60)).padStart(2, "0");
  const offsetMinute = String(offsetAbsolute % 60).padStart(2, "0");

  return `${year}年${month}月${day}日${weekday} ${hour}:${minute} UTC${offsetSign}${offsetHour}:${offsetMinute}`;
}
