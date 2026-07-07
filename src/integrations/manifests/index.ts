import { isManifestAvailable } from "../../domain/manifest";
import type { AppSettings, ManifestStatus } from "../../types";
import { depotboxManifestSource } from "./depotbox";
import { hubcapManifestSource } from "./hubcap";
import type { ManifestSource } from "./types";
import { uniqueAppIds } from "./types";

export const manifestSources = [hubcapManifestSource, depotboxManifestSource];

export function hasConfiguredManifestSource(settings: AppSettings) {
  return manifestSources.some((source) => source.isConfigured(settings));
}

export async function fetchPreferredManifestStatuses(appIds: number[], settings: AppSettings) {
  const ids = uniqueAppIds(appIds);
  if (!ids.length) return [];

  const configuredSources = manifestSources.filter((source) => source.isConfigured(settings));
  if (!configuredSources.length) {
    throw new Error("请先在设置里保存 Key。");
  }

  const [primarySource, ...fallbackSources] = configuredSources;
  try {
    const primaryStatuses = await primarySource.fetchStatuses(ids);
    return mergeFallbackStatuses(ids, primaryStatuses, fallbackSources);
  } catch (error) {
    if (!fallbackSources.length) throw error;
    console.warn("[wuhu] manifest status check failed, trying another configured source", error);
    return fetchFirstAvailableStatuses(ids, fallbackSources);
  }
}

async function fetchFirstAvailableStatuses(appIds: number[], sources: ManifestSource[]) {
  let lastError: unknown = null;

  for (let index = 0; index < sources.length; index += 1) {
    const source = sources[index];
    try {
      const statuses = await source.fetchStatuses(appIds);
      return mergeFallbackStatuses(appIds, statuses, sources.slice(index + 1));
    } catch (error) {
      lastError = error;
    }
  }

  throw lastError ?? new Error("清单状态未知，请稍后重试。");
}

async function mergeFallbackStatuses(
  appIds: number[],
  primaryStatuses: ManifestStatus[],
  fallbackSources: ManifestSource[],
) {
  let statuses = primaryStatuses;

  for (const source of fallbackSources) {
    const byAppId = new Map(statuses.map((status) => [status.appId, status]));
    const fallbackAppIds = appIds.filter((appId) => {
      const status = byAppId.get(appId);
      return !status || !isManifestAvailable(status);
    });
    if (!fallbackAppIds.length) break;

    try {
      const fallbackStatuses = await source.fetchStatuses(fallbackAppIds);
      const fallbackByAppId = new Map(fallbackStatuses.map((status) => [status.appId, status]));
      statuses = appIds
        .map((appId) => {
          const primaryStatus = byAppId.get(appId) ?? null;
          if (isManifestAvailable(primaryStatus)) return primaryStatus;
          return fallbackByAppId.get(appId) ?? primaryStatus;
        })
        .filter((status): status is ManifestStatus => Boolean(status));
    } catch (error) {
      console.warn("[wuhu] fallback manifest status check failed", error);
    }
  }

  return statuses;
}
