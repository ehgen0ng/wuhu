import { message } from "../i18n";
import type { ManifestStatus, PackageItem, PackageUpdateCheck, SteamSearchResult } from "../types";
import { formatFileSize } from "./display";

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
      message: message("manifest.unknown"),
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
      message: message("manifest.updating"),
    };
  }

  if (!status.manifestFileExists) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "info",
      message: message("manifest.notFound"),
    };
  }

  if (!status.available) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "warning",
      message: status.status ? message("manifest.status", { status: status.status }) : message("manifest.unavailable"),
    };
  }

  const size = formatFileSize(status.fileSize);
  const suffix = size ? ` · ${size}` : "";
  const hasUpdate = hasPackageManifestUpdate(pkg, status);
  const remoteTime = status.fileModified && !Number.isNaN(Date.parse(status.fileModified))
    ? new Date(status.fileModified)
    : status.fileModified ?? message("common.unknown");

  if (hasUpdate) {
    return {
      status,
      checkedAt,
      hasUpdate,
      kind: "warning",
      message: message("manifest.updateFound", { time: remoteTime, size: suffix }),
    };
  }

  if (!status.fileModified) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "success",
      message: message("manifest.availableUnknown", { size: suffix }),
    };
  }

  if (!pkg.manifestUpdatedAt) {
    return {
      status,
      checkedAt,
      hasUpdate: false,
      kind: "info",
      message: message("manifest.remoteUnknown", { time: remoteTime, size: suffix }),
    };
  }

  return {
    status,
    checkedAt,
    hasUpdate: false,
    kind: "success",
    message: message("manifest.latest", { time: remoteTime, size: suffix }),
  };
}
