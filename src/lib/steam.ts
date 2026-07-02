import type {
  AppState,
  PackageItem,
  SteamSearchPlatforms,
  SteamSearchPrice,
  SteamSearchResult,
} from "../types";
import { call } from "./tauri";

type RawSteamSearchItem = {
  type?: string;
  itemType?: string;
  name?: string;
  id?: number;
  tiny_image?: string;
  tinyImage?: string | null;
  price?: SteamSearchPrice | null;
  platforms?: SteamSearchPlatforms | null;
};

function normalizeSteamSearchItem(item: RawSteamSearchItem): SteamSearchResult | null {
  const itemType = item.itemType ?? item.type ?? "";
  const name = item.name?.trim() ?? "";
  if (!name || typeof item.id !== "number") return null;

  return {
    itemType,
    name,
    id: item.id,
    tinyImage: item.tinyImage ?? item.tiny_image ?? null,
    price: item.price ?? null,
    platforms: item.platforms ?? null,
  };
}

export async function searchSteamStore(query: string): Promise<SteamSearchResult[]> {
  const items = await call<RawSteamSearchItem[]>("search_steam_games", { query });
  return items.map(normalizeSteamSearchItem).filter((item): item is SteamSearchResult => Boolean(item));
}

export function steamHeaderImage(appId: number | null | undefined) {
  if (!appId) return null;
  return `https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/${appId}/header.jpg`;
}

function shouldUseSteamTitle(pkg: PackageItem, steamTitle: string) {
  const current = pkg.title.trim();
  if (!steamTitle.trim() || current === steamTitle) return false;
  if (!current || current === pkg.id || current === String(pkg.appId ?? "")) return true;
  return /^[\x00-\x7F]+$/.test(current);
}

function needsSteamMetadata(pkg: PackageItem) {
  if (!pkg.appId) return false;
  return /^[\x00-\x7F]+$/.test(pkg.title.trim());
}

export async function enrichPackageMetadata(state: AppState): Promise<AppState> {
  if (!state.packages.some(needsSteamMetadata)) return state;

  const packages = await Promise.all(
    state.packages.map(async (pkg) => {
      if (!needsSteamMetadata(pkg) || !pkg.appId) return pkg;

      try {
        const results = await searchSteamStore(pkg.appId.toString());
        const match = results.find((item) => item.id === pkg.appId);
        if (!match) return pkg;

        return {
          ...pkg,
          title: shouldUseSteamTitle(pkg, match.name) ? match.name : pkg.title,
        };
      } catch {
        return pkg;
      }
    }),
  );

  return { ...state, packages };
}
