export type PackageItem = {
  id: string;
  title: string;
  appId: number | null;
  luaFileName: string;
  manifestFiles: string[];
  sourceZipName: string;
  enabled: boolean;
  importedAt: number;
  manifestUpdatedAt?: string | null;
  manifestFileSize?: number | null;
  imageUrl?: string | null;
};

export type AppSettings = {
  steamPath: string | null;
  hubcapApiKey: string | null;
  depotboxApiKey: string | null;
};

export type InstallStatus = {
  installed: boolean;
  supported: boolean;
};

export type SteamClientStatus = {
  version: string | null;
  clientBuildDate: number | null;
  locked: boolean;
  lockSupported: boolean;
};

export type AppState = {
  settings: AppSettings;
  packages: PackageItem[];
  tickets: TicketItem[];
  installStatus: InstallStatus;
  packageSyncSupported: boolean;
  steamClient: SteamClientStatus;
};

export type TicketItem = {
  appId: number;
  title: string;
  hasAppTicket: boolean;
  hasETicket: boolean;
  extractedAt: number;
  expiresAt?: number | null;
  sourceFileName?: string | null;
};

export type Page = "packages" | "tickets" | "settings";

export type NoticeKind = "info" | "success" | "warning" | "error";

export type Notice = {
  page: Page;
  text: string;
  kind?: NoticeKind;
};

export type PackageUpdateCheck = {
  status: ManifestStatus | null;
  hasUpdate: boolean;
  message: string;
  kind: NoticeKind;
  checkedAt: number;
};

export type SteamSearchPrice = {
  currency: string;
  initial: number;
  final: number;
};

export type SteamSearchPlatforms = {
  windows?: boolean;
  mac?: boolean;
  linux?: boolean;
};

export type SteamSearchResult = {
  itemType: string;
  name: string;
  id: number;
  tinyImage: string | null;
  price: SteamSearchPrice | null;
  platforms: SteamSearchPlatforms | null;
  manifestChecking?: boolean;
  manifestStatus?: ManifestStatus | null;
  manifestChecked?: boolean;
};

export type ManifestProvider = "hubcap" | "depotbox";

export type ManifestStatus = {
  provider: ManifestProvider;
  appId: number;
  gameName: string | null;
  status: string | null;
  available: boolean;
  manifestFileExists: boolean;
  updateInProgress: boolean | null;
  needsUpdate: boolean | null;
  fileSize: number | null;
  fileModified: string | null;
  error: string | null;
};

export type HubcapManifestStatus = ManifestStatus;

export type HubcapQuota = {
  dailyUsage: number;
  dailyLimit: number;
};

export type AppRelease = {
  version: string;
  name: string | null;
  url: string | null;
};
