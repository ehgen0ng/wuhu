export type PackageItem = {
  id: string;
  title: string;
  appId: number | null;
  luaFileName: string;
  manifestFiles: string[];
  sourceZipName: string;
  enabled: boolean;
  importedAt: number;
};

export type AppSettings = {
  steamPath: string | null;
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
