import { errorText, LocalizedError } from "../i18n/errors";
import { useLanguage } from "./LanguageProvider";
import { message, t, type LocalizedText } from "../i18n";
import { downloadDir, homeDir, join } from "@tauri-apps/api/path";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import {
  addRemoteManifest,
  deletePackage as deletePackageCommand,
  deleteTicket as deleteTicketCommand,
  detectSteamPath as detectSteamPathCommand,
  exportTicketsTxt,
  extractTicket as extractTicketCommand,
  getOpenSteamToolStatus,
  getHubcapQuota,
  getInitialState,
  getLatestAppRelease,
  importPackageFromPath,
  importTicketsTxtFromPath,
  installOpenSteamTool,
  launchSteamWithOpenSteamTool,
  restoreOpenSteamTool,
  setDepotboxApiKey,
  setHubcapApiKey,
  setPackageEnabled as setPackageEnabledCommand,
  setSteamClientVersionLocked,
  setSteamPath as setSteamPathCommand,
  updateRemoteManifest,
} from "../api/commands";
import { buildPackageUpdateCheck, canAddManifest } from "../domain/manifest";
import { enrichPackageMetadata } from "../domain/packageMetadata";
import { PackagesPage } from "../features/packages/PackagesPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { TicketsPage } from "../features/tickets/TicketsPage";
import { createGameSearchSources } from "../integrations/gameSearch";
import { fetchPreferredManifestStatuses, hasConfiguredManifestSource } from "../integrations/manifests";
import { wait, waitForNextPaint } from "../lib/render";
import type {
  AppState,
  HubcapQuota,
  AppRelease,
  Notice,
  PackageUpdateCheck,
  Page,
  PackageItem,
  SteamSearchResult,
  TicketItem,
} from "../types";
import { AppLayout } from "./AppLayout";
import { APP_VERSION, isVersionNewer, waitForTauriRuntime } from "./runtime";

function packageSavedMessage(state: AppState | null | undefined | void, subject: LocalizedText) {
  if (state?.packageSyncSupported && state.settings.steamPath) return message("packages.savedNotice", { subject });
  if (state?.settings.steamPath) return message("packages.savedUnsupported", { subject });
  return message("packages.savedNeedsPath", { subject });
}

async function expandDialogDefaultPath(path: string | null | undefined) {
  const trimmed = path?.trim();
  if (!trimmed) return undefined;

  if (trimmed === "~") return homeDir();
  if (trimmed.startsWith("~/") || trimmed.startsWith("~\\")) {
    const home = await homeDir();
    const separator = home.endsWith("/") || home.endsWith("\\") ? "" : "/";
    return `${home}${separator}${trimmed.slice(2)}`;
  }

  return trimmed;
}

async function downloadsDefaultPath(fileName?: string) {
  const dir = await downloadDir();
  return fileName ? join(dir, fileName) : dir;
}

function selectedDialogPath(selected: string | string[] | null) {
  return Array.isArray(selected) ? selected[0] : selected;
}

export default function App() {
  const { locale } = useLanguage();
  const [page, setPage] = useState<Page>("packages");
  const [state, setState] = useState<AppState | null>(null);
  const [steamPathInput, setSteamPathInput] = useState("");
  const [hubcapKeyInput, setHubcapKeyInput] = useState("");
  const [depotboxKeyInput, setDepotboxKeyInput] = useState("");
  const [hubcapQuota, setHubcapQuota] = useState<HubcapQuota | null>(null);
  const [latestRelease, setLatestRelease] = useState<AppRelease | null>(null);
  const [releaseCheckBusy, setReleaseCheckBusy] = useState(false);
  const [packageUpdateChecks, setPackageUpdateChecks] = useState<Record<string, PackageUpdateCheck>>({});
  const [searchTerm, setSearchTerm] = useState("");
  const [searchResults, setSearchResults] = useState<SteamSearchResult[]>([]);
  const [hasSearched, setHasSearched] = useState(false);
  const [isSearching, setIsSearching] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const busyRef = useRef<string | null>(null);
  const searchRunId = useRef(0);
  const stateApplyVersion = useRef(0);

  const packages = state?.packages ?? [];
  const tickets = state?.tickets ?? [];
  const hasLoadedState = state !== null;
  const manifestSettings = state?.settings ?? {
    steamPath: steamPathInput || null,
    hubcapApiKey: hubcapKeyInput || null,
    depotboxApiKey: depotboxKeyInput || null,
  };
  const appUpdateRelease =
    latestRelease && isVersionNewer(latestRelease.version, APP_VERSION) ? latestRelease : null;

  async function applyAppState(nextState: AppState) {
    const applyVersion = stateApplyVersion.current + 1;
    stateApplyVersion.current = applyVersion;
    setState(nextState);
    setSteamPathInput(nextState.settings.steamPath ?? "");
    setHubcapKeyInput(nextState.settings.hubcapApiKey ?? "");
    setDepotboxKeyInput(nextState.settings.depotboxApiKey ?? "");

    const enrichedState = await enrichPackageMetadata(nextState);
    if (stateApplyVersion.current !== applyVersion) return;
    setState(enrichedState);
    setSteamPathInput(enrichedState.settings.steamPath ?? "");
    setHubcapKeyInput(enrichedState.settings.hubcapApiKey ?? "");
    setDepotboxKeyInput(enrichedState.settings.depotboxApiKey ?? "");
  }

  async function checkLatestRelease(showResult = false) {
    if (releaseCheckBusy) return;

    setReleaseCheckBusy(true);
    try {
      const release = await getLatestAppRelease();
      const hasUpdate = isVersionNewer(release.version, APP_VERSION);
      setLatestRelease(hasUpdate ? release : null);
      if (showResult) {
        setNotice({
          page: "settings",
          text: hasUpdate ? message("release.found", { version: release.version }) : message("release.upToDate"),
          kind: hasUpdate ? "warning" : "success",
        });
      }
    } catch (error) {
      console.info("[wuhu] latest release check skipped", error);
      setLatestRelease(null);
      if (showResult) {
        setNotice({ page: "settings", text: message("release.checkUnavailable"), kind: "info" });
      }
    } finally {
      setReleaseCheckBusy(false);
    }
  }

  useEffect(() => {
    let cancelled = false;

    async function loadInitialState() {
      const hasTauriRuntime = await waitForTauriRuntime();
      if (cancelled) return;

      if (!hasTauriRuntime) {
        console.warn("[wuhu] Tauri runtime was not detected before get_initial_state.");
      }

      try {
        const nextState = await getInitialState();
        if (!cancelled) {
          await applyAppState(nextState);
        }
      } catch (error) {
        if (cancelled) return;
        console.error("[wuhu] get_initial_state failed", error);
        setNotice({ page: "packages", text: errorText(error), kind: "error" });
      }
    }

    void loadInitialState();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    void checkLatestRelease();
  }, []);

  useEffect(() => {
    if (!state?.installStatus.launchRequired) return;

    let cancelled = false;

    async function refreshOpenSteamToolStatus() {
      try {
        const installStatus = await getOpenSteamToolStatus();
        if (cancelled) return;
        setState((current) => (current ? { ...current, installStatus } : current));
      } catch (error) {
        console.info("[wuhu] OpenSteamTool status refresh skipped", error);
      }
    }

    void refreshOpenSteamToolStatus();
    const timer = window.setInterval(() => void refreshOpenSteamToolStatus(), 3_000);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [state?.installStatus.launchRequired]);

  function switchPage(nextPage: Page) {
    setPage(nextPage);
    setNotice(null);
  }

  function clearSearchState() {
    searchRunId.current += 1;
    setIsSearching(false);
    setSearchTerm("");
    setSearchResults([]);
    setHasSearched(false);
  }

  function beginAction(label: string, visual = false) {
    if (busyRef.current) return false;
    busyRef.current = label;
    if (visual) setBusy(label);
    return true;
  }

  function endAction(label: string, visual = false) {
    if (busyRef.current === label) {
      busyRef.current = null;
    }
    if (visual) setBusy(null);
  }

  async function refreshState() {
    const label = "refresh";
    if (!beginAction(label, true)) return;
    try {
      setNotice(null);
      const [nextState] = await Promise.all([getInitialState(), wait(320)]);
      await applyAppState(nextState);
      clearSearchState();
      setPackageUpdateChecks({});
    } catch (error) {
      setNotice({ page: "packages", text: errorText(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function runAction(
    label: string,
    noticePage: Page,
    action: () => Promise<AppState | void>,
    success?: LocalizedText | ((state: AppState | void) => LocalizedText),
    pending?: LocalizedText,
    visual = false,
  ) {
    if (!beginAction(label, visual)) return;
    try {
      if (pending) {
        setNotice({ page: noticePage, text: pending, kind: "info" });
        await waitForNextPaint();
      } else {
        setNotice(null);
      }
      const nextState = await action();
      if (nextState) {
        await applyAppState(nextState);
      }
      if (success) {
        const successText = typeof success === "function" ? success(nextState) : success;
        setNotice({ page: noticePage, text: successText, kind: "success" });
      }
    } catch (error) {
      setNotice({ page: noticePage, text: errorText(error), kind: "error" });
    } finally {
      endAction(label, visual);
    }
  }

  async function handleImportFile() {
    try {
      const selected = await open({
        title: t(locale, "packages.import"),
        defaultPath: await downloadsDefaultPath(),
        filters: [{ name: "ZIP", extensions: ["zip"] }],
        multiple: false,
      });
      const path = selectedDialogPath(selected);
      if (!path) return;

      await runAction(
        "import",
        "packages",
        () => importPackageFromPath(path),
        (nextState) => packageSavedMessage(nextState, message("packages.imported")),
      );
      setPackageUpdateChecks({});
    } catch (error) {
      setNotice({ page: "packages", text: errorText(error), kind: "error" });
    }
  }

  async function handleImportTicketsFile() {
    try {
      const selected = await open({
        title: t(locale, "tickets.importFile"),
        defaultPath: await downloadsDefaultPath(),
        filters: [{ name: "tickets.txt", extensions: ["txt"] }],
        multiple: false,
      });
      const path = selectedDialogPath(selected);
      if (!path) return;

      await runAction(
        "import-ticket",
        "tickets",
        () => importTicketsTxtFromPath(path),
        message("tickets.imported"),
        undefined,
        true,
      );
    } catch (error) {
      setNotice({ page: "tickets", text: errorText(error), kind: "error" });
    }
  }

  async function searchSteamGames(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const query = searchTerm.trim();
    if (!query) {
      setNotice({ page: "packages", text: message("search.enterName"), kind: "warning" });
      return;
    }

    const runId = searchRunId.current + 1;
    searchRunId.current = runId;
    const sources = createGameSearchSources();
    const seenAppIds = new Set<number>();
    let failedSourceCount = 0;
    let resultCount = 0;

    setIsSearching(true);
    setNotice(null);
    setHasSearched(true);
    setSearchResults([]);

    const hasManifestKey = hasConfiguredManifestSource(manifestSettings);

    await Promise.all(
      sources.map(async (source) => {
        try {
          const results = await source.search(query);
          if (searchRunId.current !== runId) return;

          const newResults = appendSearchSourceResults(results, seenAppIds, hasManifestKey);
          resultCount += newResults.length;

          if (newResults.length && hasManifestKey) {
            void checkSearchResultManifestStatuses(newResults, runId);
          }
        } catch (error) {
          console.warn(`[wuhu] search failed: ${source.id}`, error);
          failedSourceCount += 1;
        }
      }),
    );

    if (searchRunId.current !== runId) return;

    setIsSearching(false);
    if (resultCount === 0) {
      setNotice({
        page: "packages",
        text: failedSourceCount === sources.length ? message("search.failed") : message("search.noResults"),
        kind: failedSourceCount === sources.length ? "error" : "info",
      });
    }
  }

  function appendSearchSourceResults(
    results: SteamSearchResult[],
    seenAppIds: Set<number>,
    checkManifests: boolean,
  ) {
    const newResults = results
      .filter((item) => {
        if (!item.id || seenAppIds.has(item.id)) return false;
        seenAppIds.add(item.id);
        return true;
      })
      .map((item) => ({
        ...item,
        manifestChecking: checkManifests,
        manifestChecked: false,
        manifestStatus: null,
      }));

    if (newResults.length) {
      setSearchResults((current) => [...current, ...newResults]);
    }

    return newResults;
  }

  async function checkSearchResultManifestStatuses(results: SteamSearchResult[], runId: number) {
    try {
      const statuses = await fetchPreferredManifestStatuses(results.map((item) => item.id), manifestSettings);
      if (searchRunId.current !== runId) return;

      const byAppId = new Map(statuses.map((status) => [status.appId, status]));
      const checkedAppIds = new Set(results.map((item) => item.id));

      setSearchResults((current) =>
        current.map((item) => {
          if (!checkedAppIds.has(item.id)) return item;
          return {
            ...item,
            manifestChecking: false,
            manifestChecked: true,
            manifestStatus: byAppId.get(item.id) ?? null,
          };
        }),
      );
    } catch (error) {
      if (searchRunId.current !== runId) return;

      console.warn("[wuhu] search result manifest status check failed", error);
      const checkedAppIds = new Set(results.map((item) => item.id));
      setSearchResults((current) =>
        current.map((item) =>
          checkedAppIds.has(item.id)
            ? {
              ...item,
              manifestChecking: false,
              manifestChecked: true,
              manifestStatus: null,
            }
            : item,
        ),
      );
    }
  }

  async function addSearchResult(item: SteamSearchResult) {
    const label = `add-manifest-${item.id}`;
    if (!beginAction(label, true)) return;
    try {
      setNotice({ page: "packages", text: message("packages.addPending", { title: item.name }), kind: "info" });

      if (!canAddManifest(item)) {
        throw new LocalizedError(message("manifest.unavailable"));
      }

      await waitForNextPaint();

      const nextState = await addRemoteManifest(item.id, item.name, item.tinyImage);
      await applyAppState(nextState);
      setPackageUpdateChecks({});

      setNotice({
        page: "packages",
        text: packageSavedMessage(nextState, message("packages.added", { title: item.name })),
        kind: "success",
      });
    } catch (error) {
      setNotice({ page: "packages", text: errorText(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function togglePackage(pkg: PackageItem, enabled: boolean) {
    setState((current) =>
      current
        ? {
          ...current,
          packages: current.packages.map((item) => (item.id === pkg.id ? { ...item, enabled } : item)),
        }
        : current,
    );

    await runAction(`toggle-${pkg.id}`, "packages", () => setPackageEnabledCommand(pkg.id, enabled));
  }

  async function deletePackage(pkg: PackageItem) {
    const confirmed = window.confirm(
      t(locale, "packages.confirmDelete", { title: pkg.title }),
    );
    if (!confirmed) return;

    await runAction(`delete-${pkg.id}`, "packages", () => deletePackageCommand(pkg.id), message("packages.deleted"));
    setPackageUpdateChecks((current) => {
      const next = { ...current };
      delete next[pkg.id];
      return next;
    });
  }

  async function updatePackage(pkg: PackageItem) {
    const label = `update-manifest-${pkg.id}`;
    if (!beginAction(label, true)) return;

    try {
      setNotice({ page: "packages", text: message("packages.updatePending", { title: pkg.title }), kind: "info" });
      await waitForNextPaint();

      const nextState = await updateRemoteManifest(pkg.id);
      await applyAppState(nextState);
      setPackageUpdateChecks((current) => {
        const next = { ...current };
        delete next[pkg.id];
        return next;
      });

      setNotice({ page: "packages", text: message("packages.updated", { title: pkg.title }), kind: "success" });
    } catch (error) {
      setNotice({ page: "packages", text: errorText(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function extractTicketByAppId(appId: number) {
    if (!Number.isInteger(appId) || appId <= 0) {
      setNotice({ page: "tickets", text: message("tickets.invalidAppId"), kind: "warning" });
      return;
    }

    const title =
      packages.find((pkg) => pkg.appId === appId)?.title ??
      tickets.find((ticket) => ticket.appId === appId)?.title ??
      appId.toString();

    await runAction(
      "extract-ticket",
      "tickets",
      () => extractTicketCommand(appId, title),
      message("tickets.extracted", { title }),
      message("tickets.extractPending", { title }),
      true,
    );
  }

  async function exportTicket(ticket: TicketItem) {
    try {
      const path = await save({
        title: t(locale, "tickets.exportFile"),
        defaultPath: await downloadsDefaultPath(`${ticket.appId}.tickets.txt`),
        filters: [{ name: "tickets.txt", extensions: ["txt"] }],
      });
      if (!path) return;

      await runAction(
        `export-ticket-${ticket.appId}`,
        "tickets",
        () => exportTicketsTxt(ticket.appId, path),
        message("tickets.exported"),
        undefined,
        true,
      );
    } catch (error) {
      setNotice({ page: "tickets", text: errorText(error), kind: "error" });
    }
  }

  async function deleteTicket(ticket: TicketItem) {
    const confirmed = window.confirm(t(locale, "tickets.confirmDelete", { title: ticket.title }));
    if (!confirmed) return;

    await runAction(
      `delete-ticket-${ticket.appId}`,
      "tickets",
      () => deleteTicketCommand(ticket.appId),
      message("tickets.deleted"),
      undefined,
      true,
    );
  }

  async function checkPackageUpdates() {
    const label = "check-package-updates";
    if (!beginAction(label, true)) return;

    try {
      const checkablePackages = packages.filter((pkg) => typeof pkg.appId === "number" && pkg.appId > 0);
      const skippedCount = packages.length - checkablePackages.length;

      if (!checkablePackages.length) {
        setNotice({ page: "packages", text: message("packages.noCheckable"), kind: "warning" });
        return;
      }

      if (!state || !hasConfiguredManifestSource(state.settings)) {
        setNotice({ page: "packages", text: message("packages.keyRequired"), kind: "warning" });
        return;
      }

      setNotice({ page: "packages", text: message("packages.checking"), kind: "info" });
      await waitForNextPaint();

      const statuses = await fetchPreferredManifestStatuses(checkablePackages.map((pkg) => pkg.appId ?? 0), state.settings);
      const byAppId = new Map(statuses.map((status) => [status.appId, status]));
      const nextChecks: Record<string, PackageUpdateCheck> = {};
      for (const pkg of checkablePackages) {
        nextChecks[pkg.id] = buildPackageUpdateCheck(pkg, byAppId.get(pkg.appId ?? 0) ?? null);
      }

      setPackageUpdateChecks((current) => ({ ...current, ...nextChecks }));

      const updatedPackages = checkablePackages.filter((pkg) => nextChecks[pkg.id]?.hasUpdate);
      const unknownTimeCount = checkablePackages.filter((pkg) => {
        const status = nextChecks[pkg.id]?.status;
        return status?.available && !status.fileModified;
      }).length;
      const skippedText = skippedCount ? message("packages.skipped", { count: skippedCount }) : "";
      if (updatedPackages.length) {
        const examples = updatedPackages
          .slice(0, 3)
          .map((pkg) => pkg.title)
          .join(locale === "en" ? ", " : "、");
        const suffix = updatedPackages.length > 3 ? message("common.andMore") : "";
        const unknownText = unknownTimeCount ? message("packages.unknownTimes", { count: unknownTimeCount }) : "";
        setNotice({
          page: "packages",
          text: message("packages.updatesFound", { count: updatedPackages.length, examples, more: suffix, unknown: unknownText, skipped: skippedText }),
          kind: "warning",
        });
      } else {
        const checkedText = unknownTimeCount
          ? message("packages.checkedUnknown", { count: unknownTimeCount, skipped: skippedText })
          : message("packages.checked", { skipped: skippedText });
        setNotice({
          page: "packages",
          text: checkedText,
          kind: "success",
        });
      }
    } catch (error) {
      setNotice({ page: "packages", text: errorText(error), kind: "error" });
    } finally {
      endAction(label, true);
    }
  }

  async function saveSteamPath() {
    await runAction(
      "steam-path",
      "settings",
      () => setSteamPathCommand(steamPathInput.trim()),
      message("steam.pathSaved"),
    );
  }

  async function saveHubcapKey() {
    const label = "hubcap-key";
    if (!beginAction(label)) return;
    try {
      setNotice(null);

      const nextState = await setHubcapApiKey(hubcapKeyInput.trim());
      await applyAppState(nextState);

      if (hubcapKeyInput.trim()) {
        setHubcapQuota(await getHubcapQuota());
      } else {
        setHubcapQuota(null);
      }

      setNotice({ page: "settings", text: message("settings.keySaved"), kind: "success" });
    } catch (error) {
      setHubcapQuota(null);
      setNotice({ page: "settings", text: errorText(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function saveDepotboxKey() {
    const label = "depotbox-key";
    if (!beginAction(label)) return;
    try {
      setNotice(null);

      const nextState = await setDepotboxApiKey(depotboxKeyInput.trim());
      await applyAppState(nextState);
      setPackageUpdateChecks({});

      setNotice({ page: "settings", text: message("settings.keySaved"), kind: "success" });
    } catch (error) {
      setNotice({ page: "settings", text: errorText(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function refreshHubcapQuota() {
    const label = "hubcap-quota";
    if (!beginAction(label)) return;
    try {
      setNotice(null);
      setHubcapQuota(await getHubcapQuota());
    } catch (error) {
      setHubcapQuota(null);
      setNotice({ page: "settings", text: errorText(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function detectSteamPath() {
    const label = "detect-steam";
    if (!beginAction(label)) return;

    try {
      const path = await detectSteamPathCommand();
      if (!path) throw new LocalizedError(message("steam.notDetected"));

      setSteamPathInput(path);
      if (path === state?.settings.steamPath) {
        setNotice({ page: "settings", text: message("steam.pathUnchanged"), kind: "success" });
        return;
      }

      const nextState = await setSteamPathCommand(path);
      await applyAppState(nextState);
      setNotice({ page: "settings", text: message("steam.pathSaved"), kind: "success" });
    } catch (error) {
      setNotice({ page: "settings", text: errorText(error), kind: "error" });
    } finally {
      endAction(label);
    }
  }

  async function chooseSteamPath() {
    try {
      const defaultPath = await expandDialogDefaultPath(
        state?.settings.steamPath ?? (await detectSteamPathCommand()),
      );
      const selected = await open({
        title: t(locale, "steam.choosePath"),
        directory: true,
        multiple: false,
        defaultPath,
      });
      const selectedPath = Array.isArray(selected) ? selected[0] : selected;
      if (!selectedPath) return;

      setSteamPathInput(selectedPath);
      await runAction(
        "steam-path",
        "settings",
        () => setSteamPathCommand(selectedPath),
        message("steam.pathSaved"),
      );
    } catch (error) {
      setNotice({ page: "settings", text: errorText(error), kind: "error" });
    }
  }

  async function toggleSteamClientLock(locked: boolean) {
    await runAction(
      "steam-client-lock",
      "settings",
      () => setSteamClientVersionLocked(locked),
      locked ? message("steam.lockedNotice") : message("steam.unlockedNotice"),
    );
  }

  return (
    <AppLayout
      page={page}
      installed={Boolean(state?.installStatus.installed)}
      installSupported={Boolean(state?.installStatus.supported)}
      launchRequired={Boolean(state?.installStatus.launchRequired)}
      launchedViaWuhu={Boolean(state?.installStatus.launchedViaWuhu)}
      hasLoadedState={hasLoadedState}
      onPageChange={switchPage}
    >
      {page === "packages" ? (
        <PackagesPage
          notice={notice}
          packages={packages}
          packageUpdateChecks={packageUpdateChecks}
          searchResults={searchResults}
          searchTerm={searchTerm}
          hasSearched={hasSearched}
          hasLoadedState={hasLoadedState}
          hasSteamPath={Boolean(state?.settings.steamPath)}
          packageSyncSupported={Boolean(state?.packageSyncSupported)}
          busy={busy}
          isSearching={isSearching}
          onRefresh={refreshState}
          onCheckPackageUpdates={checkPackageUpdates}
          onImportFile={handleImportFile}
          onSearch={searchSteamGames}
          onSearchTermChange={setSearchTerm}
          onAddSearchResult={addSearchResult}
          onUpdatePackage={updatePackage}
          onTogglePackage={togglePackage}
          onDeletePackage={deletePackage}
        />
      ) : page === "tickets" ? (
        <TicketsPage
          notice={notice}
          tickets={tickets}
          hasLoadedState={hasLoadedState}
          hasSteamPath={Boolean(state?.settings.steamPath)}
          busy={busy}
          onExtract={extractTicketByAppId}
          onRefresh={refreshState}
          onImport={handleImportTicketsFile}
          onExport={exportTicket}
          onDelete={deleteTicket}
        />
      ) : (
        <SettingsPage
          appVersion={APP_VERSION}
          latestRelease={appUpdateRelease}
          releaseCheckBusy={releaseCheckBusy}
          notice={notice}
          state={state}
          steamPathInput={steamPathInput}
          hubcapKeyInput={hubcapKeyInput}
          depotboxKeyInput={depotboxKeyInput}
          hubcapQuota={hubcapQuota}
          onSteamPathChange={setSteamPathInput}
          onHubcapKeyChange={(value) => {
            setHubcapKeyInput(value);
            setHubcapQuota(null);
          }}
          onDepotboxKeyChange={setDepotboxKeyInput}
          onSaveSteamPath={saveSteamPath}
          onDetectSteamPath={detectSteamPath}
          onChooseSteamPath={chooseSteamPath}
          onSaveHubcapKey={saveHubcapKey}
          onSaveDepotboxKey={saveDepotboxKey}
          onRefreshHubcapQuota={refreshHubcapQuota}
          onCheckLatestRelease={() => checkLatestRelease(true)}
          onInstallOpenSteamTool={() =>
            runAction(
              "install",
              "settings",
              () => installOpenSteamTool(),
              state?.installStatus.updateAvailable
                ? message("component.updated")
                : message("component.installedNotice"),
            )
          }
          onLaunchSteamWithOpenSteamTool={() =>
            runAction(
              "launch-steam",
              "settings",
              () => launchSteamWithOpenSteamTool(),
              message("steam.launchedNotice"),
            )
          }
          onRestoreOpenSteamTool={() =>
            runAction(
              "restore",
              "settings",
              () => restoreOpenSteamTool(),
              state?.installStatus.launchRequired ? message("steam.restored") : message("component.removed"),
            )
          }
          onToggleSteamClientLock={toggleSteamClientLock}
        />
      )}
    </AppLayout>
  );
}
