import type { ManifestStatus, PackageItem, PackageUpdateCheck, SteamSearchResult } from "../types";
import { formatFileSize, formatManifestTime } from "./display";

export function isManifestAvailable(status: ManifestStatus | null | undefined) {
  return Boolean(
    status?.available &&
      status.manifestFileExists &&
      !status.updateInProgress &&
      (!status.status || status.status.toLowerCase() === "available"),
  );
}

export function canAddManifest(item: SteamSearchResult) {
  return Boolean(item.manifestChecked && isManifestAvailable(item.manifestStatus));
}

export function hasPackageManifestUpdate(pkg: PackageItem, status: ManifestStatus | null) {
  if (!status?.available || !status.fileModified || !pkg.manifestUpdatedAt) return false;

  const remoteTime = new Date(status.fileModified).getTime();
  const localTime = new Date(pkg.manifestUpdatedAt).getTime();
  if (Number.isNaN(remoteTime) || Number.isNaN(localTime)) return false;

  return remoteTime > localTime;
}

export function buildPackageUpdateCheck(
  pkg: PackageItem,
  status: ManifestStatus | null,
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
  const hasUpdate = hasPackageManifestUpdate(pkg, status);

  if (hasUpdate) {
    const remoteTime = formatManifestTime(status.fileModified);
    return {
      status,
      checkedAt,
      hasUpdate,
      kind: "warning",
      message: `发现更新：${remoteTime}${suffix}`,
    };
  }

  if (!status.fileModified) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "success",
      message: `清单可用，更新时间未知${suffix}。`,
    };
  }

  const remoteTime = formatManifestTime(status.fileModified);
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
