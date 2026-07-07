import { checkDepotboxManifestStatuses } from "../../api/commands";
import { fetchBatchedStatuses, hasApiKey, type ManifestSource } from "./types";

export const depotboxManifestSource: ManifestSource = {
  id: "depotbox",
  batchSize: 100,
  isConfigured: (settings) => hasApiKey(settings.depotboxApiKey),
  fetchStatuses: (appIds) => fetchBatchedStatuses(appIds, 100, checkDepotboxManifestStatuses),
};
