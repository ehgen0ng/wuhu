import type { AppSettings, ManifestStatus } from "../../types";

export type ManifestSource = {
  id: string;
  batchSize: number;
  isConfigured: (settings: AppSettings) => boolean;
  fetchStatuses: (appIds: number[]) => Promise<ManifestStatus[]>;
};

export function hasApiKey(value: string | null | undefined) {
  return Boolean(value?.trim());
}

export function uniqueAppIds(appIds: number[]) {
  return Array.from(new Set(appIds.filter((appId) => appId > 0)));
}

export async function fetchBatchedStatuses(
  appIds: number[],
  batchSize: number,
  fetchBatch: (appIds: number[]) => Promise<ManifestStatus[]>,
) {
  const ids = uniqueAppIds(appIds);
  const statuses: ManifestStatus[] = [];

  for (let index = 0; index < ids.length; index += batchSize) {
    statuses.push(...(await fetchBatch(ids.slice(index, index + batchSize))));
  }

  return statuses;
}
