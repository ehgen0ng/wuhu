import { call } from "../lib/tauri";
import type {
  AppRelease,
  AppState,
  HubcapQuota,
  InstallStatus,
  ManifestStatus,
  SteamSearchResult,
} from "../types";

export function getInitialState() {
  return call<AppState>("get_initial_state");
}

export function detectSteamPath() {
  return call<string | null>("detect_steam_path");
}

export function setSteamPath(path: string) {
  return call<AppState>("set_steam_path", { path });
}

export function importPackageFromBytes(fileName: string, dataBase64: string) {
  return call<AppState>("import_package_from_bytes", { fileName, dataBase64 });
}

export function importPackageFromPath(path: string) {
  return call<AppState>("import_package_from_path", { path });
}

export function setHubcapApiKey(apiKey: string) {
  return call<AppState>("set_hubcap_api_key", { apiKey });
}

export function setDepotboxApiKey(apiKey: string) {
  return call<AppState>("set_depotbox_api_key", { apiKey });
}

export function checkHubcapManifestStatuses(appIds: number[]) {
  return call<ManifestStatus[]>("check_hubcap_manifest_statuses", { appIds });
}

export function checkDepotboxManifestStatuses(appIds: number[]) {
  return call<ManifestStatus[]>("check_depotbox_manifest_statuses", { appIds });
}

export function getHubcapQuota() {
  return call<HubcapQuota>("get_hubcap_quota");
}

export function getLatestAppRelease() {
  return call<AppRelease>("get_latest_app_release");
}

export function addRemoteManifest(appId: number, title: string, imageUrl?: string | null) {
  return call<AppState>("add_remote_manifest", { appId, title, imageUrl });
}

export function updateRemoteManifest(id: string) {
  return call<AppState>("update_remote_manifest", { id });
}

export function setPackageEnabled(id: string, enabled: boolean) {
  return call<AppState>("set_package_enabled", { id, enabled });
}

export function deletePackage(id: string) {
  return call<AppState>("delete_package", { id });
}

export function extractTicket(appId: number, title: string) {
  return call<AppState>("extract_ticket", { appId, title });
}

export function importTicketsTxt(fileName: string, dataBase64: string) {
  return call<AppState>("import_tickets_txt", { fileName, dataBase64 });
}

export function importTicketsTxtFromPath(path: string) {
  return call<AppState>("import_tickets_txt_from_path", { path });
}

export function exportTicketsTxt(appId: number, path: string) {
  return call<void>("export_tickets_txt", { appId, path });
}

export function deleteTicket(appId: number) {
  return call<AppState>("delete_ticket", { appId });
}

export function installOpenSteamTool() {
  return call<AppState>("install_opensteamtool");
}

export function launchSteamWithOpenSteamTool() {
  return call<AppState>("launch_steam_with_opensteamtool");
}

export function getOpenSteamToolStatus() {
  return call<InstallStatus>("get_opensteamtool_status");
}

export function restoreOpenSteamTool() {
  return call<AppState>("restore_opensteamtool");
}

export function setSteamClientVersionLocked(locked: boolean) {
  return call<AppState>("set_steam_client_version_locked", { locked });
}

export function addSteamGame(appId: number, title: string) {
  return call<AppState>("add_steam_game", { appId, title });
}

export function searchSteamGames(query: string) {
  return call<SteamSearchResult[]>("search_steam_games", { query });
}

export function searchSteamSuggestGames(query: string) {
  return call<SteamSearchResult[]>("search_steam_suggest_games", { query });
}

export function searchCheapsharkGames(query: string) {
  return call<SteamSearchResult[]>("search_cheapshark_games", { query });
}

export function searchIsthereanydealGames(query: string) {
  return call<SteamSearchResult[]>("search_isthereanydeal_games", { query });
}
