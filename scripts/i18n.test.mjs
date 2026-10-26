// Verify translation contracts and user-visible language behavior without a desktop runtime.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, test } from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";

const server = await createServer({ server: { middlewareMode: true }, appType: "custom", optimizeDeps: { noDiscovery: true, include: [] } });
after(() => server.close());
const { DEFAULT_LOCALE, t, message, renderText, formatDateTime } = await server.ssrLoadModule("/src/i18n/index.ts");
const { LocalizedError, errorText, renderError } = await server.ssrLoadModule("/src/i18n/errors.ts");
const { buildPackageUpdateCheck } = await server.ssrLoadModule("/src/domain/manifest.ts");
const { LanguageProvider, useLanguage } = await server.ssrLoadModule("/src/app/LanguageProvider.tsx");

const { MantineProvider } = await import("@mantine/core");
const { SettingsPage } = await server.ssrLoadModule("/src/features/settings/SettingsPage.tsx");
const { manifestStatusText } = await server.ssrLoadModule("/src/domain/manifestText.ts");
const { hasConfiguredManifestSource } = await server.ssrLoadModule("/src/integrations/manifests/index.ts");


test("Chinese and English have identical keys and interpolation parameters", () => {
  const zh = JSON.parse(readFileSync(new URL("../src/i18n/locales/zh-CN.json", import.meta.url)));
  const en = JSON.parse(readFileSync(new URL("../src/i18n/locales/en.json", import.meta.url)));
  assert.deepEqual(Object.keys(en).sort(), Object.keys(zh).sort());
  const placeholders = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
  for (const key of Object.keys(zh)) {
    assert.ok(zh[key].trim() && en[key].trim(), key);
    assert.deepEqual(placeholders(en[key]), placeholders(zh[key]), key);
  }
});

test("default and invalid language preferences use Chinese; saved English is respected", () => {
  const original = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  function CurrentLanguage() {
    return React.createElement("span", null, useLanguage().locale);
  }
  try {
    for (const [saved, expected] of [[null, "zh-CN"], ["fr", "zh-CN"], ["", "zh-CN"], ["en", "en"], ["zh-CN", "zh-CN"]]) {
      Object.defineProperty(globalThis, "localStorage", { configurable: true, value: { getItem: () => saved } });
      assert.equal(renderToStaticMarkup(React.createElement(LanguageProvider, null, React.createElement(CurrentLanguage))), `<span>${expected}</span>`);
    }
    Object.defineProperty(globalThis, "localStorage", { configurable: true, get() { throw new Error("Storage denied"); } });
    assert.equal(renderToStaticMarkup(React.createElement(LanguageProvider, null, React.createElement(CurrentLanguage))), "<span>zh-CN</span>");
    assert.equal(DEFAULT_LOCALE, "zh-CN");
  } finally {
    if (original) Object.defineProperty(globalThis, "localStorage", original);
    else delete globalThis.localStorage;
  }
});

test("stored notices change language without altering user content or interpolating it twice", () => {
  const title = "中文游戏 {count} <script> $&";
  const notice = message("packages.savedNeedsPath", { subject: message("packages.added", { title }) });
  assert.equal(renderText("zh-CN", notice), `已添加 ${title}，已保存到本地；设置 Steam 路径后可启用。`);
  assert.equal(renderText("en", notice), `Added ${title}. Saved locally; set the Steam path to enable it.`);
  assert.equal(t("en", "search.resultCount", { count: 0 }), "Results: 0");
});

test("manifest update notices format dates in the selected language at render time", () => {
  const pkg = { manifestUpdatedAt: "2025-01-01T00:00:00Z" };
  const status = { available: true, manifestFileExists: true, fileModified: "2026-10-08T10:30:00Z", fileSize: 2048 };
  const check = buildPackageUpdateCheck(pkg, status);
  assert.equal(check.hasUpdate, true);
  for (const locale of ["zh-CN", "en"]) {
    assert.ok(renderText(locale, check.message).includes(formatDateTime(locale, new Date(status.fileModified))));
  }
  assert.match(renderText("en", check.message), /^Update available:/);
  assert.equal(renderText("en", buildPackageUpdateCheck(pkg, null).message), "Manifest status unknown. Please try again later.");
});

test("known desktop errors translate while paths and unknown diagnostics remain intact", () => {
  assert.equal(renderError("en", "请先设置 Steam 路径"), "Set the Steam path first");
  const diagnostic = "读取清单文件失败：/Users/测试/游戏.zip: permission denied";
  assert.equal(renderError("zh-CN", diagnostic), diagnostic);
  assert.equal(renderError("en", diagnostic), "Failed to read manifest file: /Users/测试/游戏.zip: permission denied");
  assert.equal(renderError("en", "未知上游响应: error 123"), "未知上游响应: error 123");
  const error = new LocalizedError(message("settings.keyRequired"));
  assert.equal(renderError("en", errorText(error)), "Save a Key in Settings first.");
  assert.equal(renderError("zh-CN", errorText(error)), "请先在设置里保存 Key。");
});


function settingsMarkup(auth) {
  const noop = () => {};
  const props = {
    appVersion: "test", latestRelease: null, releaseCheckBusy: false, notice: null,
    state: {
      settings: { steamPath: null, hubcapApiKey: "example", depotboxApiKey: null },
      luatoolsAuth: auth,
      installStatus: {}, steamClient: {}, packages: [], tickets: [],
    },
    steamPathInput: "", hubcapKeyInput: "example", depotboxKeyInput: "",
    hubcapQuota: { dailyUsage: 2, dailyLimit: 800 },
    luaToolsInput: auth?.credential ?? "", luaToolsQuota: { dailyUsage: 3, dailyLimit: 25 },
    onSteamPathChange: noop, onHubcapKeyChange: noop, onDepotboxKeyChange: noop,
    onSaveSteamPath: noop, onDetectSteamPath: noop, onChooseSteamPath: noop,
    onSaveHubcapKey: noop, onSaveDepotboxKey: noop, onRefreshHubcapQuota: noop,
    onLuaToolsInputChange: noop, onSaveLuaToolsCredentials: noop, onRefreshLuaToolsQuota: noop,
    onCheckLatestRelease: noop, onInstallOpenSteamTool: noop, onLaunchSteamWithOpenSteamTool: noop,
    onRestoreOpenSteamTool: noop, onToggleSteamClientLock: noop,
  };
  return renderToStaticMarkup(React.createElement(MantineProvider, { theme: { colors: { steam: Array(10).fill("#66c0f4") } } },
    React.createElement(LanguageProvider, null, React.createElement(SettingsPage, props))));
}

test("LuaTools settings sit between Hubcap and DepotBox and show the masked saved token", () => {
  const html = settingsMarkup({ credential: "test-refresh-token", hasRefreshToken: true, expiresAt: 123 });
  assert.ok(html.indexOf("Hubcap Key") < html.indexOf("LuaTools Token"));
  assert.ok(html.indexOf("LuaTools Token") < html.indexOf("DepotBox Key"));
  assert.ok(html.includes("3/25"));
  assert.match(html, /type="password"[^>]*value="test-refresh-token"/);
  const accessOnly = settingsMarkup({ credential: "test-access-token", hasRefreshToken: false, expiresAt: 123 });
  assert.ok(accessOnly.includes("仅访问令牌"));
  assert.ok(settingsMarkup(null).includes("LuaTools Token"));
});

test("LuaTools alone enables manifest search and never displays a fabricated update date", () => {
  assert.equal(hasConfiguredManifestSource({ hubcapApiKey: null, depotboxApiKey: null, luatoolsConfigured: true }), true);
  assert.equal(hasConfiguredManifestSource({ hubcapApiKey: null, depotboxApiKey: null }), false);
  const item = { manifestChecked: true, manifestStatus: { provider: "luatools", source: "Luie", available: true, manifestFileExists: true, status: "available", fileModified: null, fileSize: null } };
  assert.equal(manifestStatusText(item, "zh-CN"), "LuaTools · Luie · 清单可用 · 更新时间未知");
});
