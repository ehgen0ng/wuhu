import { checkHubcapManifestStatuses } from "../../api/commands";
import { fetchBatchedStatuses, hasApiKey, type ManifestSource } from "./types";

export const hubcapManifestSource: ManifestSource = {
  id: "hubcap",
  batchSize: 24,
  isConfigured: (settings) => hasApiKey(settings.hubcapApiKey),
  fetchStatuses: (appIds) => fetchBatchedStatuses(appIds, 24, checkHubcapManifestStatuses),
};
