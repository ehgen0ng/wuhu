import { Alert } from "@mantine/core";
import { AlertTriangle, CheckCircle2, Info } from "lucide-react";
import type { Notice } from "../types";
import { useLanguage } from "../app/LanguageProvider";
import { renderText } from "../i18n";
import { renderError } from "../i18n/errors";

export function NoticeAlert({ notice }: { notice: Notice }) {
  const { locale } = useLanguage();
  const kind = notice.kind ?? "info";
  const Icon = kind === "error" || kind === "warning" ? AlertTriangle : kind === "success" ? CheckCircle2 : Info;
  const color = kind === "error" ? "red" : kind === "warning" ? "yellow" : kind === "success" ? "green" : "steam";

  return (
    <Alert
      color={color}
      icon={<Icon size={17} />}
      mb="lg"
      radius="md"
      role={kind === "error" ? "alert" : "status"}
      variant="light"
    >
      {kind === "error" ? renderError(locale, notice.text) : renderText(locale, notice.text)}
    </Alert>
  );
}
