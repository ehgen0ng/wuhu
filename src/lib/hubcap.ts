import type { SteamSearchResult } from "../types";
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
