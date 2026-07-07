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

type SteamSearchCommand =
  | "search_steam_games"
  | "search_steam_suggest_games"
  | "search_cheapshark_games"
  | "search_isthereanydeal_games";

export type SteamSearchSource = {
  id: string;
  label: string;
  search: (query: string) => Promise<SteamSearchResult[]>;
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

function normalizeSteamSearchResults(items: RawSteamSearchItem[]) {
  return items.map(normalizeSteamSearchItem).filter((item): item is SteamSearchResult => Boolean(item));
}

async function searchSteamCommand(command: SteamSearchCommand, query: string): Promise<SteamSearchResult[]> {
  const items = await call<RawSteamSearchItem[]>(command, { query });
  return normalizeSteamSearchResults(items);
}

export async function searchSteamStore(query: string): Promise<SteamSearchResult[]> {
  return searchSteamCommand("search_steam_games", query);
}

export function createSteamSearchSources(): SteamSearchSource[] {
  return [
    {
      id: "steam-store",
      label: "Steam 商店",
      search: (query) => searchSteamCommand("search_steam_games", query),
    },
    {
      id: "steam-suggest",
      label: "Steam 建议",
      search: (query) => searchSteamCommand("search_steam_suggest_games", query),
    },
    {
      id: "cheapshark",
      label: "CheapShark",
      search: (query) => searchSteamCommand("search_cheapshark_games", query),
    },
    {
      id: "isthereanydeal",
      label: "IsThereAnyDeal",
      search: (query) => searchSteamCommand("search_isthereanydeal_games", query),
    },
  ];
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
