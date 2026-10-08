import { useLanguage } from "../../app/LanguageProvider";
import { t, formatDateTime, type Locale } from "../../i18n";
import {
  ActionIcon,
  Box,
  Button,
  Group,
  Loader,
  Modal,
  Stack,
  Table,
  Text,
  TextInput,
  ThemeIcon,
  Title,
  Tooltip,
} from "@mantine/core";
import { CheckCircle2, Download, KeyRound, RefreshCcw, Trash2, Upload, XCircle } from "lucide-react";
import { type FormEvent, useState } from "react";
import { NoticeAlert } from "../../components/NoticeAlert";
import { PageHeader } from "../../components/PageHeader";
import type { Notice, TicketItem } from "../../types";

type TicketsPageProps = {
  notice: Notice | null;
  tickets: TicketItem[];
  hasLoadedState: boolean;
  hasSteamPath: boolean;
  busy: string | null;
  onExtract: (appId: number) => void;
  onRefresh: () => void;
  onImport: () => void;
  onExport: (ticket: TicketItem) => void;
  onDelete: (ticket: TicketItem) => void;
};

export function TicketsPage({
  notice,
  tickets,
  hasLoadedState,
  hasSteamPath,
  busy,
  onExtract,
  onRefresh,
  onImport,
  onExport,
  onDelete,
}: TicketsPageProps) {
  const { locale } = useLanguage();
  const [extractOpened, setExtractOpened] = useState(false);
  const [extractAppId, setExtractAppId] = useState("");
  const [extractError, setExtractError] = useState<string | null>(null);
  const isExtracting = busy === "extract-ticket";
  const isImporting = busy === "import-ticket";
  const isRefreshing = busy === "refresh";
  const rows = [...tickets].sort((left, right) => left.title.localeCompare(right.title, locale));

  function submitExtract(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const appId = Number(extractAppId.trim());
    if (!Number.isInteger(appId) || appId <= 0) {
      setExtractError(t(locale, "tickets.invalidAppId"));
      return;
    }

    setExtractError(null);
    setExtractOpened(false);
    onExtract(appId);
  }

  return (
    <Box component="section" className="page">
      <Modal
        centered
        closeButtonProps={{ "aria-label": t(locale, "common.close") }}
        classNames={{
          body: "ticket-extract-modal__body",
          close: "ticket-extract-modal__close",
          content: "ticket-extract-modal",
          header: "ticket-extract-modal__header",
          title: "ticket-extract-modal__title",
        }}
        opened={extractOpened}
        overlayProps={{ backgroundOpacity: 0.58, blur: 3 }}
        onClose={() => setExtractOpened(false)}
        title={t(locale, "tickets.extractTitle")}
      >
        <form onSubmit={submitExtract}>
          <Stack gap="md">
            <TextInput
              autoFocus
              label="AppID"
              inputMode="numeric"
              value={extractAppId}
              error={extractError}
              onChange={(event) => {
                setExtractAppId(event.currentTarget.value);
                setExtractError(null);
              }}
            />
            <Group justify="flex-end" gap="sm">
              <Button variant="subtle" onClick={() => setExtractOpened(false)}>
                {t(locale, "common.cancel")}
              </Button>
              <Button
                color="steam"
                c="#06121e"
                type="submit"
                loading={isExtracting}
                disabled={isExtracting}
              >
                {t(locale, "tickets.extract")}
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>

      <PageHeader
        title={t(locale, "nav.tickets")}
        actions={
          <>
            <Button
              variant="light"
              leftSection={isRefreshing ? <Loader color="steam" size={17} /> : <RefreshCcw size={17} />}
              aria-busy={isRefreshing}
              onClick={onRefresh}
            >
              {t(locale, "common.refresh")}
            </Button>
            <Button
              variant="light"
              leftSection={isExtracting ? <Loader color="steam" size={17} /> : <KeyRound size={17} />}
              disabled={!hasSteamPath || isExtracting}
              aria-busy={isExtracting}
              onClick={() => setExtractOpened(true)}
            >
              {t(locale, "tickets.extract")}
            </Button>
            <Button
              color="steam"
              variant="filled"
              c="#06121e"
              leftSection={isImporting ? <Loader color="#06121e" size={17} /> : <Upload size={17} />}
              aria-busy={isImporting}
              disabled={isImporting}
              onClick={onImport}
            >
              {t(locale, "tickets.import")}
            </Button>
          </>
        }
      />

      {notice?.page === "tickets" && <NoticeAlert notice={notice} />}

      {rows.length > 0 ? (
        <Box className="ticket-table-wrap">
          <Table className="ticket-table" verticalSpacing="sm">
            <Table.Thead>
              <Table.Tr>
                <Table.Th>{t(locale, "tickets.game")}</Table.Th>
                <Table.Th>AppID</Table.Th>
                <Table.Th>AppTicket</Table.Th>
                <Table.Th>ETicket</Table.Th>
                <Table.Th>{t(locale, "tickets.expires")}</Table.Th>
                <Table.Th className="ticket-actions-head">{t(locale, "common.actions")}</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {rows.map((ticket) => {
                const exportBusy = busy === `export-ticket-${ticket.appId}`;
                const deleteBusy = busy === `delete-ticket-${ticket.appId}`;

                return (
                  <Table.Tr key={ticket.appId}>
                    <Table.Td>
                      <Text className="ticket-title" fw={700}>
                        {ticket.title || ticket.appId}
                      </Text>
                    </Table.Td>
                    <Table.Td>
                      <Text ff="monospace" size="sm">
                        {ticket.appId}
                      </Text>
                    </Table.Td>
                    <Table.Td>{ticketMark(ticket.hasAppTicket, locale)}</Table.Td>
                    <Table.Td>{ticketMark(ticket.hasETicket, locale)}</Table.Td>
                    <Table.Td>
                      <Text c={isExpired(ticket.expiresAt) ? "red.3" : "dimmed"} size="sm">
                        {formatExpiry(ticket.expiresAt, locale)}
                      </Text>
                    </Table.Td>
                    <Table.Td>
                      <Group gap={6} justify="flex-end" wrap="nowrap">
                        <Tooltip label={t(locale, "tickets.exportFile")}>
                          <ActionIcon
                            variant="light"
                            aria-label={t(locale, "tickets.exportFile")}
                            disabled={exportBusy}
                            onClick={() => onExport(ticket)}
                          >
                            {exportBusy ? <Loader color="steam" size={16} /> : <Download size={16} />}
                          </ActionIcon>
                        </Tooltip>
                        <Tooltip label={t(locale, "tickets.delete")}>
                          <ActionIcon
                            variant="light"
                            aria-label={t(locale, "tickets.delete")}
                            disabled={deleteBusy}
                            onClick={() => onDelete(ticket)}
                          >
                            {deleteBusy ? <Loader color="steam" size={16} /> : <Trash2 size={16} />}
                          </ActionIcon>
                        </Tooltip>
                      </Group>
                    </Table.Td>
                  </Table.Tr>
                );
              })}
            </Table.Tbody>
          </Table>
        </Box>
      ) : (
        hasLoadedState && (
          <Stack className="empty-state" align="center" justify="center" gap="sm">
            <ThemeIcon color="steam" radius="xl" size={54} variant="light">
              <KeyRound size={30} />
            </ThemeIcon>
            <Title order={2} size={22}>
              {t(locale, "tickets.empty")}
            </Title>
            <Text c="dimmed" size="sm">
              {t(locale, "tickets.emptyHint")}
            </Text>
          </Stack>
        )
      )}
    </Box>
  );
}

function ticketMark(enabled: boolean, locale: Locale) {
  return enabled ? (
    <CheckCircle2 className="ticket-mark ticket-mark--ok" size={20} aria-label={t(locale, "common.present")} />
  ) : (
    <XCircle className="ticket-mark ticket-mark--missing" size={20} aria-label={t(locale, "common.missing")} />
  );
}

function formatExpiry(value: number | null | undefined, locale: Locale) {
  if (!value) return t(locale, "common.unknown");
  const date = new Date(value * 1000);
  return formatDateTime(locale, date);
}

function isExpired(value: number | null | undefined) {
  return Boolean(value && value * 1000 <= Date.now());
}
