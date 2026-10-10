// Detalles de la interfaz que necesitan al sistema (uxio.rs).
import { invoke } from "./core";

export interface Dropped {
  path: string;
  name: string;
  size: number;
  /** image · vault · profile · config · pdf · dump · unknown */
  kind: string;
}

export const uxApi = {
  mediaInUse: () => invoke<boolean>("media_in_use"),
  describeFile: (path: string) => invoke<Dropped>("describe_file", { path }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  openPdf: (path: string) => invoke<void>("open_dropped_pdf", { path }),
  composeMail: (subject: string, body: string) => invoke<void>("compose_mail", { subject, body }),
};

// ---------- Correo del dominio y códigos de error ----------

export interface MailLine {
  level: "ok" | "info" | "warn" | "bad";
  what: string;
  text: string;
}

export interface MailPort {
  host: string;
  port: number;
  label: string;
  ok: boolean;
  ms: number;
}

export interface MailReport {
  domain: string;
  mx: string[];
  spf: string | null;
  dmarc: string | null;
  dkim: string[];
  ports: MailPort[];
  publicIp: string;
  blacklists: [string, string][];
  lines: MailLine[];
  providerText: string;
}

export interface ErrorInfo {
  code: string;
  name: string;
  area: "windows" | "update" | "stop" | "office" | "network" | "activation";
  what: string;
  why: string;
  todo: string;
  page: string;
}

export const mailApi = {
  check: (domain: string, server: string | null) => invoke<MailReport>("mail_domain_check", { domain, server }),
};

export const errorsApi = {
  lookup: (query: string) => invoke<ErrorInfo[]>("error_lookup", { query }),
  all: () => invoke<ErrorInfo[]>("error_codes"),
};
