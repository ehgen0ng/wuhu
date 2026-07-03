import { Box, Button, FileButton, Group, Loader, SimpleGrid, Stack, Text, TextInput, ThemeIcon, Title } from "@mantine/core";
import { PackagePlus, RefreshCcw, Search, Upload } from "lucide-react";
import type { FormEvent } from "react";
import { NoticeAlert } from "../../components/NoticeAlert";
import { PageHeader } from "../../components/PageHeader";
import { SectionHeading } from "../../components/SectionHeading";
import type { Notice, PackageItem, PackageUpdateCheck, SteamSearchResult } from "../../types";
import { SavedPackageCard, SearchResultCard } from "./PackageCards";

type PackagesPageProps = {
  notice: Notice | null;
  packages: PackageItem[];
  packageUpdateChecks: Record<string, PackageUpdateCheck>;
  searchResults: SteamSearchResult[];
  searchTerm: string;
  hasSearched: boolean;
  hasLoadedState: boolean;
  hasSteamPath: boolean;
  busy: string | null;
  onRefresh: () => void;
  onCheckPackageUpdates: () => void;
  onImportFile: (file: File | null) => void;
  onSearch: (event: FormEvent<HTMLFormElement>) => void;
  onSearchTermChange: (value: string) => void;
  onAddSearchResult: (item: SteamSearchResult) => void;
  onUpdatePackage: (pkg: PackageItem) => void;
  onTogglePackage: (pkg: PackageItem, enabled: boolean) => void;
  onDeletePackage: (pkg: PackageItem) => void;
};

export function PackagesPage({
  notice,
  packages,
  packageUpdateChecks,
  searchResults,
  searchTerm,
  hasSearched,
  hasLoadedState,
  hasSteamPath,
  busy,
  onRefresh,
  onCheckPackageUpdates,
  onImportFile,
  onSearch,
  onSearchTermChange,
  onAddSearchResult,
  onUpdatePackage,
  onTogglePackage,
  onDeletePackage,
}: PackagesPageProps) {
  const isRefreshing = busy === "refresh";
  const isCheckingUpdates = busy === "check-package-updates";
  const isSearching = busy === "steam-search";
  const isImporting = busy === "import";

  return (
    <Box component="section" className="page">
      <PageHeader
        title="清单管理"
        actions={
          <>
            <Button
              variant="light"
              leftSection={isRefreshing ? <Loader color="steam" size={17} /> : <RefreshCcw size={17} />}
              aria-busy={isRefreshing}
              onClick={onRefresh}
            >
              刷新
            </Button>
            <Button
              variant="light"
              leftSection={isCheckingUpdates ? <Loader color="steam" size={17} /> : <RefreshCcw size={17} />}
              aria-busy={isCheckingUpdates}
              onClick={onCheckPackageUpdates}
              disabled={!packages.length || isCheckingUpdates}
            >
              检查更新
            </Button>
            <FileButton onChange={onImportFile} accept=".zip">
              {(props) => (
                <Button
                  {...props}
                  color="steam"
                  variant="filled"
                  c="#06121e"
                  leftSection={isImporting ? <Loader color="#06121e" size={18} /> : <PackagePlus size={18} />}
                  aria-busy={isImporting}
                >
                  导入清单
                </Button>
              )}
            </FileButton>
          </>
        }
      />

      {notice?.page === "packages" && <NoticeAlert notice={notice} />}

      <Box component="form" mb="xl" onSubmit={onSearch}>
        <Group align="stretch" gap="sm" wrap="nowrap" className="responsive-control-row">
          <TextInput
            value={searchTerm}
            onChange={(event) => onSearchTermChange(event.currentTarget.value)}
            leftSection={<Search size={17} />}
            className="grow-control"
          />
          <Button
            type="submit"
            color="steam"
            variant="filled"
            c="#06121e"
            leftSection={isSearching ? <Loader color="#06121e" size={17} /> : <Search size={17} />}
            aria-busy={isSearching}
            disabled={!searchTerm.trim()}
          >
            搜索
          </Button>
        </Group>
      </Box>

      {hasSearched && searchResults.length > 0 && (
        <Box component="section" mb={28}>
          <SectionHeading title="搜索结果" meta={`${searchResults.length} 个结果`} />
          <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }} spacing="lg">
            {searchResults.map((item, index) => {
              const existingPackage = packages.find((pkg) => pkg.appId === item.id || pkg.id === item.id.toString());
              return (
                <SearchResultCard
                  key={item.id}
                  item={item}
                  index={index}
                  existingPackage={existingPackage}
                  busy={busy}
                  onAdd={onAddSearchResult}
                />
              );
            })}
          </SimpleGrid>
        </Box>
      )}

      {packages.length > 0 && (
        <Box component="section" className={hasSearched && searchResults.length > 0 ? "saved-section" : undefined}>
          <SectionHeading title="已保存清单" meta={`${packages.length} 个清单`} />
          <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }} spacing="lg">
            {packages.map((pkg, index) => (
              <SavedPackageCard
                key={pkg.id}
                pkg={pkg}
                index={index}
                busy={busy}
                hasSteamPath={hasSteamPath}
                updateCheck={packageUpdateChecks[pkg.id]}
                onUpdate={onUpdatePackage}
                onToggle={onTogglePackage}
                onDelete={onDeletePackage}
              />
            ))}
          </SimpleGrid>
        </Box>
      )}

      {hasLoadedState && !packages.length && searchResults.length === 0 && (
        <Stack className="empty-state" align="center" justify="center" gap="sm">
          <ThemeIcon color="steam" radius="xl" size={54} variant="light">
            <Upload size={30} />
          </ThemeIcon>
          <Title order={2} size={22}>
            还没有清单
          </Title>
          <Text c="dimmed" size="sm">
            导入 zip 或搜索游戏添加。
          </Text>
        </Stack>
      )}
    </Box>
  );
}
