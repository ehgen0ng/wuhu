import { checkPreferredManifestStatuses } from "../../api/commands";
import { message } from "../../i18n";
import { LocalizedError } from "../../i18n/errors";
import type { AppSettings } from "../../types";
import { fetchBatchedStatuses, uniqueAppIds } from "./types";

type ManifestSettings = AppSettings & { luatoolsConfigured?: boolean };

export function hasConfiguredManifestSource(settings: ManifestSettings) {
  return Boolean(settings.hubcapApiKey?.trim() || settings.depotboxApiKey?.trim() || settings.luatoolsConfigured);
}

export async function fetchPreferredManifestStatuses(appIds: number[], settings: ManifestSettings) {
  const ids = uniqueAppIds(appIds);
  if (!ids.length) return [];
  if (!hasConfiguredManifestSource(settings)) {
    throw new LocalizedError(message("settings.keyRequired"));
  }
  // Selection lives in Rust and is shared with add/update, including timestamp comparison and fallback.
  return fetchBatchedStatuses(ids, 24, checkPreferredManifestStatuses);
}
