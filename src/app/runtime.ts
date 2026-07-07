import appPackage from "../../package.json";

export const APP_VERSION = appPackage.version;

export function isTauriRuntime() {
  return typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);
}

export function waitForTauriRuntime(timeoutMs = 2500) {
  return new Promise<boolean>((resolve) => {
    if (isTauriRuntime()) {
      resolve(true);
      return;
    }

    const startedAt = Date.now();
    const timer = window.setInterval(() => {
      if (isTauriRuntime()) {
        window.clearInterval(timer);
        resolve(true);
        return;
      }

      if (Date.now() - startedAt >= timeoutMs) {
        window.clearInterval(timer);
        resolve(false);
      }
    }, 50);
  });
}

function parseVersionParts(version: string) {
  return version
    .trim()
    .replace(/^v/i, "")
    .split(/[.-]/)
    .map((part) => Number.parseInt(part, 10))
    .map((part) => (Number.isFinite(part) ? part : 0));
}

export function isVersionNewer(remoteVersion: string, currentVersion: string) {
  const remoteParts = parseVersionParts(remoteVersion);
  const currentParts = parseVersionParts(currentVersion);
  const length = Math.max(remoteParts.length, currentParts.length);

  for (let index = 0; index < length; index += 1) {
    const remotePart = remoteParts[index] ?? 0;
    const currentPart = currentParts[index] ?? 0;
    if (remotePart > currentPart) return true;
    if (remotePart < currentPart) return false;
  }

  return false;
}
