// Procesos, red, software, espacio y drivers.
import { invoke } from "./core";

// ---------- Fase 8: procesos, red, software, espacio, drivers ----------

export interface ProcessView {
  pid: number;
  parent: number | null;
  name: string;
  exe: string | null;
  user: string | null;
  cpu: number;
  memory: number;
  diskReadPerSec: number;
  diskWritePerSec: number;
  started: number;
  protection: "none" | "sensitive" | "critical" | "self";
  note: string | null;
}

export interface Adapter {
  name: string;
  description: string;
  kind: string;
  linkSpeed: string;
  mac: string;
  ipv4: string[];
  gateway: string[];
  dns: string[];
  dhcp: boolean | null;
  ssid: string | null;
  signal: number | null;
}

export interface PingResult {
  label: string;
  target: string;
  sent: number;
  received: number;
  avgMs: number | null;
  minMs: number | null;
  maxMs: number | null;
}

export interface NetworkReport {
  adapters: Adapter[];
  pings: PingResult[];
  dns: { host: string; ok: boolean; ms: number; addresses: string[] }[];
  internet: boolean;
  captivePortal: boolean;
}

export interface SpeedResult {
  timestamp: number;
  server: string;
  isp: string | null;
  ip: string | null;
  ipv4?: string | null;
  ipv6?: string | null;
  location?: string | null;
  latencyMs: number;
  jitterMs: number;
  downloadMbps: number;
  uploadMbps: number;
  downloadLatencyMs: number | null;
  uploadLatencyMs: number | null;
  transferredMb: number;
}

export interface SpeedMeta {
  clientIp: string | null;
  ipv4: string | null;
  ipv6: string | null;
  isp: string | null;
  location: string | null;
  colo: string | null;
  country: string | null;
}

export interface SpeedProgress {
  phase: "meta" | "latency" | "download" | "upload" | "done";
  mbps: number;
  progress: number;
  latencyMs: number | null;
}

export interface SoftwareUpdate {
  name: string;
  id: string;
  version: string;
  available: string;
  source: string;
}

export interface Freeable {
  /** Clave del sitio: temporales, papelera, Windows.old… */
  key: string;
  name: string;
  detail: string;
  size: number;
  /** Se puede borrar sin consecuencias para el usuario. */
  safe: boolean;
}

export interface SpaceEntry {
  name: string;
  path: string;
  size: number;
  files: number;
  hasChildren: boolean;
  /** Última modificación en segundos: la del archivo o, en una carpeta, la más reciente de dentro. */
  modified?: number;
}

/** Lo que ocupa un tipo de archivo (vídeo, correo…). */
export interface KindSize {
  id: string;
  size: number;
}

/** Varios archivos con el mismo contenido. */
export interface DupGroup {
  /** Lo que ocupa cada copia. */
  size: number;
  /** Comparados por muestras y no enteros (más de 256 MB). */
  sampled: boolean;
  files: SpaceEntry[];
}

export interface Duplicates {
  groups: DupGroup[];
  /** Lo que se recupera dejando una copia de cada grupo. */
  wasted: number;
  checked: number;
  /** Archivos que solo están en la nube: no se leen para no descargarlos. */
  cloudSkipped: number;
  seconds: number;
  truncated: boolean;
}

/** Una carpeta del análisis: sus subcarpetas y de qué está llena. */
export interface SpaceFolder {
  path: string;
  size: number;
  files: number;
  children: SpaceEntry[];
  kinds: KindSize[];
}

export interface SpaceView {
  root: string;
  size: number;
  files: number;
  denied: number;
  seconds: number;
  folder: SpaceFolder;
  largestFiles: SpaceEntry[];
  /** Grandes y sin tocar en más de un año. */
  oldFiles: SpaceEntry[];
}

export const toolsApi = {
  processes: () => invoke<ProcessView[]>("list_processes"),
  killProcess: (pid: number, name: string, tree: boolean) => invoke<void>("kill_process", { pid, name, tree }),
  openProcessLocation: (pid: number) => invoke<void>("open_process_location", { pid }),
  network: () => invoke<NetworkReport>("network_diagnostics"),
  speedtest: () => invoke<SpeedResult>("run_speedtest"),
  cancelSpeedtest: () => invoke<void>("cancel_speedtest"),
  speedHistory: () => invoke<SpeedResult[]>("list_speedtests"),
  softwareUpdates: (refresh = false) => invoke<SoftwareUpdate[]>("list_software_updates", { refresh }),
  ignoredUpdates: () => invoke<string[]>("ignored_updates"),
  cachedUpdates: () => invoke<SoftwareUpdate[] | null>("cached_software_updates"),
  setUpdateIgnored: (id: string, ignored: boolean) => invoke<string[]>("set_update_ignored", { id, ignored }),
  upgradeSoftware: (ids: string[]) =>
    invoke<{ id: string; name: string; ok: boolean; message: string }[]>("upgrade_software", { ids }),
  scanSpace: (path: string) => invoke<SpaceView>("scan_space", { path }),
  cancelSpaceScan: () => invoke<void>("cancel_space_scan"),
  /** Una carpeta del último análisis. */
  spaceFolder: (path: string) => invoke<SpaceFolder>("space_folder", { path }),
  /** Los archivos sueltos de una carpeta, de mayor a menor. */
  spaceFiles: (path: string) => invoke<SpaceEntry[]>("space_files", { path }),
  /** Archivos repetidos dentro del último análisis. Tarea «duplicates». */
  spaceDuplicates: () => invoke<Duplicates>("space_duplicates"),
  /** Los archivos más grandes de un tipo en el último análisis. */
  spaceKindFiles: (kind: string) => invoke<SpaceEntry[]>("space_kind_files", { kind }),
  revealInExplorer: (path: string) => invoke<void>("reveal_in_explorer", { path }),
  /** Dónde se puede recuperar espacio en este equipo, medido de verdad. */
  spaceFreeable: () => invoke<Freeable[]>("space_freeable"),
  /** Manda a la papelera lo marcado en el análisis; devuelve los bytes liberados. */
  spaceRecycle: (paths: string[]) => invoke<number>("space_recycle", { paths }),
  backupDrivers: () => invoke<string>("backup_drivers"),
};
