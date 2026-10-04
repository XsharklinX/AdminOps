// Historial de rendimiento, arranques y cuelgues, vigilante de la conexión, y
// abrir Teams y el Correo fuera de AdminOps.
import { invoke } from "./core";

/** Una muestra por minuto (mientras AdminOps está abierta). */
export interface Sample {
  t: number;
  cpu: number;
  ram: number;
  disk: number;
  top?: string;
}

export interface BootEvent {
  t: number;
  kind: "boot" | "shutdown" | "unexpected" | "bsod";
  code: string | null;
  /** Nombre del código de pantallazo, si se conoce. */
  name: string | null;
  /** Qué suele haber detrás y qué mirar. */
  hint: string | null;
}

export interface BootTime {
  t: number;
  ms: number;
  mainMs: number;
}

export interface Dump {
  name: string;
  t: number;
  size: number;
  /** Lo que dice el volcado: el código y el driver probable. */
  analysis: { bugcheck: string | null; culprit: string | null; culpritHint: string | null; stackDrivers: string[] } | null;
}

export interface BootLog {
  events: BootEvent[];
  /** null: hace falta administrador para leer cuánto tardó cada arranque. */
  boots: BootTime[] | null;
  dumps: Dump[];
}

export interface Outage {
  start: number;
  end: number | null;
  kind: "router" | "internet" | "network";
}

export interface WatchPoint {
  t: number;
  ms: number | null;
  router: boolean;
}

export interface WatchStatus {
  running: boolean;
  startedAt: number | null;
  gateway: string | null;
  checks: number;
  failed: number;
  points: WatchPoint[];
  outages: Outage[];
}

export interface CommApps {
  teams: boolean;
  mail: "outlook" | "new-outlook" | null;
}

export const insightApi = {
  perfHistory: () => invoke<Sample[]>("perf_history"),
  bootHistory: () => invoke<BootLog>("boot_history"),
};

export const netwatchApi = {
  start: () => invoke<WatchStatus>("netwatch_start"),
  stop: () => invoke<void>("netwatch_stop"),
  status: () => invoke<WatchStatus>("netwatch_status"),
  clear: () => invoke<void>("netwatch_clear"),
};

export const commsApi = {
  apps: () => invoke<CommApps>("comm_apps"),
  /** En el navegador (la web del portal configurado, o la de Microsoft) o en su aplicación. */
  open: (kind: "teams" | "mail", how: "browser" | "app") => invoke<void>("open_comm", { kind, how }),
};
