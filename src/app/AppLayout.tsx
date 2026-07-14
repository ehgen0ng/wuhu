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
  const statusText = hasLoadedState
    ? installSupported
      ? launchRequired
        ? launchedViaWuhu
          ? "Steam 已通过 wuhu 启动"
          : "Steam 未通过 wuhu 启动"
        : installed
          ? "组件已安装"
          : "等待安装组件"
      : "组件不支持当前系统"
    : "状态未读取";
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
              label="清单管理"
              leftSection={<Archive size={19} />}
              onClick={() => onPageChange("packages")}
              variant="light"
            />
            <NavLink
              active={page === "tickets"}
              className="app-nav-link"
              label="D 加密管理"
              leftSection={<KeyRound size={19} />}
              onClick={() => onPageChange("tickets")}
              variant="light"
            />
            <NavLink
              active={page === "settings"}
              className="app-nav-link"
              label="设置"
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
