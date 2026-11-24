import { invoke } from "./core";

// ---------- Uso de la red (Red → Uso de la red) ----------

export interface UsageDestination {
  /** El nombre (DNS inverso) o, si no hay, la IP. */
  host: string;
  ip: string;
  bytes: number;
}

export interface ProgramUsage {
  /** Identificador estable: el nombre del archivo en minúsculas o `svchost:wu`. */
  key: string;
  name: string;
  /** Ruta del programa, para pedir su icono. */
  path: string | null;
  upMbps: number;
  downMbps: number;
  /** Desde que se empezó a mirar. */
  totalUp: number;
  totalDown: number;
  /** Hoy, mientras AdminOps miraba. */
  todayUp: number;
  todayDown: number;
  /** Segundos que se lleva viendo este programa. */
  watchedSecs: number;
  destinations: UsageDestination[];
  /** Con tráfico en los últimos 10 s. */
  active: boolean;
}

export interface UsageError {
  kind: "needsAdmin" | "failed";
  message: string;
}

export interface OneDrivePause {
  /** Ms desde 1970 de la hora en que se vuelve a abrir solo. */
  untilMs: number;
  /** La entrada del diario: «Reanudar ahora» la deshace. */
  entryId: number;
}

export interface UsageSnapshot {
  running: boolean;
  /** `etw` (TCP y UDP), `estats` (solo TCP) o `none`. */
  method: "etw" | "estats" | "none";
  tcpOnly: boolean;
  error: UsageError | null;
  programs: ProgramUsage[];
  totalUpMbps: number;
  totalDownMbps: number;
  startedMs: number;
  nowMs: number;
  onedrivePause: OneDrivePause | null;
  /** Eventos que Windows no pudo entregar: si hay, las cifras se quedan cortas. */
  lostEvents: number;
}

export interface PauseResult {
  untilMs: number;
  entryId: number;
  message: string;
}

export const netUsageApi = {
  /** Empieza a medir. La pantalla tiene que seguir preguntando (`snapshot`); si no, se suelta sola. */
  start: () => invoke<UsageSnapshot>("netusage_start"),
  snapshot: () => invoke<UsageSnapshot>("netusage_snapshot"),
  stop: () => invoke<void>("netusage_stop"),
  /** El icono del programa como imagen en línea, o nada. */
  icon: (path: string) => invoke<string | null>("netusage_icon", { path }),
  /** Cierra OneDrive y lo vuelve a abrir a las 2 h. Con «Deshacer» en el diario. */
  pauseOneDrive: () => invoke<PauseResult>("onedrive_pause"),
};
