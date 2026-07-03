import type { HubcapManifestStatus, PackageItem, PackageUpdateCheck, SteamSearchResult } from "../types";
import { formatFileSize, formatManifestTime } from "./format";

export function canAddManifest(item: SteamSearchResult) {
  const status = item.manifestStatus;
  return Boolean(
    item.manifestChecked &&
      status?.available &&
      status.manifestFileExists &&
      !status.updateInProgress &&
      status.status?.toLowerCase() === "available",
  );
}

export function manifestStatusText(item: SteamSearchResult) {
  if (!item.manifestChecked) return null;
  const status = item.manifestStatus;
  if (!status) return null;
  if (status.updateInProgress) return null;
  if (!canAddManifest(item)) return null;
  const size = formatFileSize(status.fileSize);
  return `清单更新：${formatManifestTime(status.fileModified)}${size ? ` · ${size}` : ""}`;
}

export function manifestIssueText(item: SteamSearchResult) {
  if (canAddManifest(item)) return null;
  if (!item.manifestChecked) return "未检查清单：请先保存 Hubcap Key。";

  const status = item.manifestStatus;
  if (!status) return "清单状态未知，请稍后重试。";
  if (status.error) return status.error;
  if (status.updateInProgress) return "清单正在更新，稍后再试。";
  if (!status.manifestFileExists) return "暂未找到可用清单。";
  if (status.status) return `清单状态：${status.status}`;
  return "当前没有可用清单。";
}

export function hasPackageManifestUpdate(pkg: PackageItem, status: HubcapManifestStatus | null) {
  if (!status?.available || !status.fileModified || !pkg.manifestUpdatedAt) return false;

  const remoteTime = new Date(status.fileModified).getTime();
  const localTime = new Date(pkg.manifestUpdatedAt).getTime();
  if (Number.isNaN(remoteTime) || Number.isNaN(localTime)) return false;

  return remoteTime > localTime;
}

export function buildPackageUpdateCheck(
  pkg: PackageItem,
  status: HubcapManifestStatus | null,
): PackageUpdateCheck {
  const checkedAt = Date.now();
  if (!status) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "error",
      message: "清单状态未知，请稍后重试。",
    };
  }

  if (status.error) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "error",
      message: status.error,
    };
  }

  if (status.updateInProgress) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "warning",
      message: "清单正在更新，稍后再试。",
    };
  }

  if (!status.manifestFileExists) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "info",
      message: "暂未找到可用清单。",
    };
  }

  if (!status.available) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "warning",
      message: status.status ? `清单状态：${status.status}` : "当前没有可用清单。",
    };
  }

  const size = formatFileSize(status.fileSize);
  const suffix = size ? ` · ${size}` : "";
  const remoteTime = formatManifestTime(status.fileModified);
  const hasUpdate = hasPackageManifestUpdate(pkg, status);

  if (hasUpdate) {
    return {
      status,
      checkedAt,
      hasUpdate,
      kind: "warning",
      message: `发现更新：${remoteTime}${suffix}`,
    };
  }

  if (!pkg.manifestUpdatedAt) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "info",
      message: `远端清单：${remoteTime}${suffix}，本地版本未知。`,
    };
  }

  return {
    status,
    checkedAt,
    hasUpdate: false,
    kind: "success",
    message: `已是最新：${remoteTime}${suffix}`,
  };
}
