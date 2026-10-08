import { useLanguage } from "./LanguageProvider";
import { t } from "../i18n";
import { AppShell, Box, Group, Image, NavLink, Stack, Text, ThemeIcon } from "@mantine/core";
import { Archive, KeyRound, Settings } from "lucide-react";
import type { ReactNode } from "react";
import appIcon from "../assets/icon.png";
import type { Page } from "../types";

type AppLayoutProps = {
  page: Page;
  installed: boolean;
  installSupported: boolean;
  launchRequired: boolean;
  launchedViaWuhu: boolean;
  hasLoadedState: boolean;
  onPageChange: (page: Page) => void;
  children: ReactNode;
};

export function AppLayout({
  page,
  installed,
  installSupported,
  launchRequired,
  launchedViaWuhu,
  hasLoadedState,
  onPageChange,
  children,
}: AppLayoutProps) {
  const { locale } = useLanguage();
  const statusText = hasLoadedState
    ? installSupported
      ? launchRequired
        ? launchedViaWuhu
          ? t(locale, "steam.launched")
          : t(locale, "steam.notLaunched")
        : installed
          ? t(locale, "component.installedStatus")
          : t(locale, "component.awaiting")
      : t(locale, "component.unsupported")
    : t(locale, "common.notLoaded");
  const statusColor = installSupported
    ? launchRequired
      ? launchedViaWuhu
        ? "green"
        : "red"
      : installed
        ? "green"
        : "red"
    : "gray";

  return (
    <AppShell navbar={{ width: 260, breakpoint: "sm" }} className="app-shell">
      <AppShell.Navbar className="app-navbar" p="lg">
        <Stack h="100%" gap="xl">
          <Group className="brand" gap="sm">
            <Image className="brand-mark" src={appIcon} alt="" />
            <Text className="brand-title">wuhu</Text>
          </Group>

          <Stack gap={8}>
            <NavLink
              active={page === "packages"}
              className="app-nav-link"
              label={t(locale, "nav.packages")}
              leftSection={<Archive size={19} />}
              onClick={() => onPageChange("packages")}
              variant="light"
            />
            <NavLink
              active={page === "tickets"}
              className="app-nav-link"
              label={t(locale, "nav.tickets")}
              leftSection={<KeyRound size={19} />}
              onClick={() => onPageChange("tickets")}
              variant="light"
            />
            <NavLink
              active={page === "settings"}
              className="app-nav-link"
              label={t(locale, "nav.settings")}
              leftSection={<Settings size={19} />}
              onClick={() => onPageChange("settings")}
              variant="light"
            />
          </Stack>

          <Group className="sidebar-footer" mt="auto" gap="sm">
            <ThemeIcon color={statusColor} radius="xl" size={12} variant="filled" />
            <Text c="dimmed" size="sm">
              {statusText}
            </Text>
          </Group>
        </Stack>
      </AppShell.Navbar>

      <AppShell.Main className="app-main">
        <Box className="app-content">{children}</Box>
      </AppShell.Main>
    </AppShell>
  );
}
