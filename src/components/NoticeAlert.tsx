import { Alert } from "@mantine/core";
import { AlertTriangle, CheckCircle2, Info } from "lucide-react";
import type { Notice } from "../types";

export function NoticeAlert({ notice }: { notice: Notice }) {
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
      {notice.text}
    </Alert>
  );
}
