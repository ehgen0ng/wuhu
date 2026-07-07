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
};

export type InstallStatus = {
  installed: boolean;
};

export type SteamClientStatus = {
  version: string | null;
  clientBuildDate: number | null;
  locked: boolean;
};

export type AppState = {
  settings: AppSettings;
  packages: PackageItem[];
  installStatus: InstallStatus;
  steamClient: SteamClientStatus;
};

export type Page = "packages" | "settings";

export type NoticeKind = "info" | "success" | "warning" | "error";

export type Notice = {
  page: Page;
  text: string;
  kind?: NoticeKind;
};

export type PackageUpdateCheck = {
  status: HubcapManifestStatus | null;
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
  manifestStatus?: HubcapManifestStatus | null;
  manifestChecked?: boolean;
};

export type HubcapManifestStatus = {
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

export type HubcapQuota = {
  dailyUsage: number;
  dailyLimit: number;
};

export type AppRelease = {
  version: string;
  name: string | null;
  url: string | null;
};
