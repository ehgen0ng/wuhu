import {
  searchCheapsharkGames,
  searchIsthereanydealGames,
  searchSteamGames,
  searchSteamSuggestGames,
} from "../../api/commands";
import type { SteamSearchPlatforms, SteamSearchPrice, SteamSearchResult } from "../../types";

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

export type SteamSearchSource = {
  id: string;
  search: (query: string) => Promise<SteamSearchResult[]>;
};

type SearchCommand = (query: string) => Promise<RawSteamSearchItem[]>;

const SEARCH_SOURCE_COMMANDS: Array<{ id: string; command: SearchCommand }> = [
  { id: "steam-store", command: searchSteamGames },
  { id: "steam-suggest", command: searchSteamSuggestGames },
  { id: "cheapshark", command: searchCheapsharkGames },
  { id: "isthereanydeal", command: searchIsthereanydealGames },
];

export function createGameSearchSources(): SteamSearchSource[] {
  return SEARCH_SOURCE_COMMANDS.map(({ id, command }) => ({
    id,
    search: async (query) => normalizeSteamSearchResults(await command(query)),
  }));
}

export async function searchSteamStore(query: string) {
  return normalizeSteamSearchResults(await searchSteamGames(query));
}

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
