// Diagnóstico e informes.
import { invoke } from "./core";
import { type SecurityAudit } from "./maintenance";
import { type ReportOptions } from "./work";

// ---------- Diagnóstico e informes (Fase 4) ----------

export type Severity = "info" | "warn" | "bad";

export type Tool =
  | "deviceManager"
  | "eventViewer"
  | "reliability"
  | "windowsUpdate"
  | "windowsSecurity"
  | "activation"
  | "storage";

export type FindingAction =
  /** Se arregla en el sitio: ejecuta ese ajuste del catálogo. `safe`: entra en «Arreglar todo lo seguro». */
  | { kind: "fix"; label: string; id: string; safe: boolean }
  | { kind: "page"; label: string; page: string; focus: string | null }
  | { kind: "tool"; label: string; tool: Tool };

export interface Finding {
  severity: Severity;
  area: string;
  title: string;
  detail: string | null;
  actions: FindingAction[];
  /** Identifica el problema sin sus cifras: lo que se guarda al marcarlo «Ya lo sé». */
  key: string;
}

/** Un hallazgo que el técnico da por sabido en este equipo. */
export interface Accepted {
  key: string;
  title: string;
  reason: string;
  at: number;
}

export interface Section<T> {
  data: T | null;
  error: string | null;
}

export interface PhysicalDisk {
  name: string;
  mediaType: string;
  busType: string;
  health: string;
  operational: string;
  size: number;
  temperature: number | null;
  wear: number | null;
  powerOnHours: number | null;
  readErrors: number | null;
  writeErrors: number | null;
  /** Es el disco donde está Windows. Ausente en análisis antiguos. */
  isSystem?: boolean;
}

export interface Stability {
  days: number;
  bugchecks: { time: string; code: string; name: string | null; hint: string | null }[];
  unexpectedShutdowns: string[];
  crashes: { app: string; count: number; last: string }[];
  minidumps:
    | {
        name: string;
        time: string;
        size: number;
        analysis: { bugcheck: string | null; culprit: string | null; culpritHint: string | null; stackDrivers: string[] } | null;
      }[]
    | null;
  bootTimes: { time: string; ms: number }[] | null;
}

export interface DeviceProblem {
  name: string;
  class: string | null;
  code: number;
  deviceId: string;
  problem: string;
}

export interface Battery {
  name: string;
  manufacturer: string;
  chemistry: string;
  design: number;
  full: number;
  cycles: number | null;
}

export interface SystemHealth {
  lastBoot: string;
  installDate: string;
  pendingReboot: boolean;
  lastUpdate: string | null;
  lastUpdateId: string | null;
  defenderRealtime: boolean | null;
  signatureAgeDays: number | null;
  antivirus: string[];
  activated: boolean | null;
  secureBoot: boolean | null;
  tpmReady: boolean | null;
  /** Lo que no se pudo leer (actualizaciones, antivirus, activación), si esa parte falló. */
  partial?: string | null;
}

/** Algo que el análisis no pudo comprobar, y por qué. */
export interface Unchecked {
  what: string;
  why: string;
  action: FindingAction | null;
}

export interface Diagnostics {
  timestamp: number;
  host: string;
  os: string;
  cpu: string;
  ramTotal: number;
  admin: boolean;
  volumes: { mount: string; total: number; free: number }[];
  disks: Section<PhysicalDisk[]>;
  stability: Section<Stability>;
  drivers: Section<DeviceProblem[]>;
  battery: Section<Battery | null>;
  system: Section<SystemHealth>;
  startupEnabled: Section<string[]>;
  bloatInstalled: Section<string[]>;
  tweaksApplied: number;
  findings: Finding[];
  /** La nota de seguridad con sus puntos. Ausente en análisis antiguos. */
  security?: Section<SecurityAudit>;
  /** Lo que no se pudo comprobar en este análisis. Ausente en análisis antiguos. */
  unchecked?: Unchecked[];
  /** Análisis rápido: solo lo que se lee en segundos; no se guarda. */
  quick?: boolean;
  /** Diferencia con el análisis anterior de este equipo. */
  changes?: DiagChanges;
}

export interface DiagChanges {
  since: number;
  /** Títulos de los problemas que antes no estaban. */
  new: string[];
  resolved: Finding[];
}

export interface SnapshotInfo {
  timestamp: number;
  bad: number;
  warn: number;
}

export const diagApi = {
  /** `force`: vuelve a consultar hardware y winget en vez de reutilizar lo de hace unos minutos. */
  /** `quick`: solo lo que se lee en segundos (no se guarda ni se compara). */
  run: (force = false, quick = false) => invoke<Diagnostics>("run_diagnostics", { force, quick }),
  /** Lo que el técnico ya sabe de este equipo: no cuenta en el Panel ni en el diagnóstico. */
  accepted: () => invoke<Accepted[]>("diag_accepted"),
  accept: (key: string, title: string, reason: string) => invoke<Accepted[]>("diag_accept", { key, title, reason }),
  unaccept: (key: string) => invoke<Accepted[]>("diag_unaccept", { key }),
  snapshots: () => invoke<SnapshotInfo[]>("list_snapshots"),
  latest: () =>
    invoke<{
      timestamp: number;
      findings: Finding[];
      /** Hallazgos marcados «Ya lo sé» en este equipo (no van en `findings`). */
      accepted: number;
      securityScore: number | null;
      model: string | null;
      gpus: string[];
      windows: string | null;
      activated: boolean | null;
      firmware: string | null;
      disks: { name: string; kind: string; size: number; status: "ok" | "warn" | "bad"; detail: string }[];
    } | null>("latest_findings"),
  generateReport: (options: ReportOptions) => invoke<string>("generate_report", { options }),
  emailReport: (path: string, to: string, subject: string, body: string) => invoke<void>("email_report", { path, to, subject, body }),
  emailReportManual: (path: string, to: string, subject: string, body: string) =>
    invoke<void>("email_report_manual", { path, to, subject, body }),
  openReport: (path: string) => invoke<void>("open_report", { path }),
  revealReport: (path: string) => invoke<void>("reveal_report", { path }),
  openTool: (tool: Tool) => invoke<void>("open_system_tool", { tool }),
};
