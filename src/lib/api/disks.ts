// Discos: salud, reparación y rescate.
import { invoke } from "./core";

// ---------- Discos: salud, reparación y rescate ----------

export interface DiskVolume {
  letter: string;
  label: string;
  fs: string;
  size: number;
  free: number;
  health: string;
  /** Windows lo marcó como dañado («Reparar disco»). null: no se pudo mirar (sin administrador). */
  dirty: boolean | null;
  system: boolean;
  /** "" sin cifrar o no se sabe · on · suspended · encrypting · decrypting */
  bitlocker: string;
  bitlockerPercent: number;
}

/** Foto diaria de las cifras de desgaste de un disco. */
export interface DiskPoint {
  day: number;
  reallocated: number | null;
  pending: number | null;
  uncorrectable: number | null;
  crc: number | null;
  readErrors: number;
  wear: number;
}

export interface DiskVerdict {
  level: "ok" | "warn" | "bad";
  title: string;
  text: string;
  advice: string[];
}

export interface DiskReport {
  number: number;
  model: string;
  /** USB, SATA, NVMe… */
  bus: string;
  /** HDD, SSD o Unspecified. */
  media: string;
  size: number;
  health: string;
  system: boolean;
  /** -1: no se sabe. */
  temperature: number;
  hours: number;
  readErrors: number;
  writeErrors: number;
  wear: number;
  volumes: DiskVolume[];
  predictFailure: boolean;
  reallocated: number | null;
  pending: number | null;
  uncorrectable: number | null;
  crcErrors: number | null;
  verdict: DiskVerdict;
  /** Últimos 90 días (una foto por día). */
  trend: DiskPoint[];
  mediaErrors: number;
  /** Textos listos para copiar. */
  texts: { client: string; ticket: string };
  nvme: NvmeHealth | null;
  /** Nota de 0 a 100 con su desglose y la vida que le queda. */
  score: HealthScore;
  /** Lo que ha subido en el último mes. */
  rising: string[];
}

/** Una fila de la tabla de salud (atributo SMART o dato de un NVMe). */
export interface SmartRow {
  id: string;
  name: string;
  explain: string;
  current: number | null;
  worst: number | null;
  threshold: number | null;
  raw: string;
  status: "ok" | "warn" | "bad";
  action: string;
}

export interface NvmeHealth {
  criticalWarning: number;
  temperature: number;
  availableSpare: number;
  spareThreshold: number;
  percentUsed: number;
  tbRead: number;
  tbWritten: number;
  powerCycles: number;
  powerOnHours: number;
  unsafeShutdowns: number;
  mediaErrors: number;
  errorLogEntries: number;
}

export interface SelfTest {
  state: "idle" | "running" | "passed" | "failed" | "aborted";
  percent: number;
  text: string;
  shortMinutes: number;
  extendedMinutes: number;
}

export interface SmartFull {
  number: number;
  model: string;
  /** ata · nvme · wmi (sin umbrales) · none */
  kind: string;
  rows: SmartRow[];
  nvme: NvmeHealth | null;
  selfTest: SelfTest | null;
  note: string;
  canSelfTest: boolean;
}

export interface ScorePart {
  name: string;
  score: number;
  text: string;
}

export interface HealthScore {
  total: number;
  label: string;
  sentence: string;
  parts: ScorePart[];
  lifeYears: number | null;
  lifeText: string;
  tbWritten: number | null;
}

export interface ScanSummary {
  finished: number;
  slow: number;
  verySlow: number;
  bad: number;
}

export interface ScanResult {
  number: number;
  model: string;
  size: number;
  mode: "quick" | "full";
  /** Una letra por zona: . bien · s lento · v muy lento · x error · ? sin mirar. */
  cells: string;
  done: number;
  total: number;
  cellBytes: number;
  curve: number[];
  avgMbps: number;
  minMbps: number;
  maxMbps: number;
  slow: number;
  verySlow: number;
  bad: number;
  started: number;
  /** 0: sin terminar (se puede seguir). */
  finished: number;
  volumesHit: string[];
  previous: ScanSummary | null;
  level: "ok" | "warn" | "bad";
  text: string;
}

export interface ScanLive {
  cells: string;
  done: number;
  total: number;
  percent: number;
  mbps: number;
  etaSecs: number;
  slow: number;
  verySlow: number;
  bad: number;
}

export interface WatchConfig {
  enabled: boolean;
  intervalMin: number;
  freePct: number;
  tempHdd: number;
  tempSsd: number;
  muted: string[];
}

export interface DiskSpeed {
  writeMbps: number;
  readMbps: number;
  randomIops: number;
  level: "ok" | "warn" | "bad";
  text: string;
}

export interface DiskCapacity {
  tested: number;
  good: number;
  firstError: number | null;
  writeMbps: number;
  readMbps: number;
  fake: boolean;
  text: string;
}

export interface DiskCheck {
  level: "ok" | "warn" | "bad";
  text: string;
}

export interface RescueResult {
  /** Archivos y bytes que hay ahora en el destino. */
  copied: number;
  bytes: number;
  /** Lo que no se pudo leer y se quedó en el disco. */
  failed: string[];
}

export const disksApi = {
  status: () => invoke<DiskReport[]>("disks_status"),
  /** Comprueba el sistema de archivos sin cambiar nada. */
  check: (letter: string) => invoke<DiskCheck>("disk_check", { letter }),
  /** Repara el sistema de archivos (en el disco de Windows, al reiniciar). */
  repair: (letter: string) => invoke<DiskCheck>("disk_repair", { letter }),
  /** chkdsk /r: tarea «disk-surface:LETRA». */
  surfaceScan: (letter: string) => invoke<DiskCheck>("disk_surface_scan", { letter }),
  /** Copia lo legible: tarea «disk-rescue». */
  rescue: (source: string, dest: string) => invoke<RescueResult>("disk_rescue", { source, dest }),
  pickFolder: () => invoke<string | null>("disk_pick_folder"),
  /** Claves de recuperación de BitLocker de un volumen de este equipo (queda en el diario). */
  bitlockerKey: (letter: string) => invoke<{ id: string; password: string }[]>("bitlocker_local_key", { letter }),
  /** Tarea «disk-speed:LETRA». */
  speed: (letter: string, media: string, bus: string) => invoke<DiskSpeed>("disk_speed_test", { letter, media, bus }),
  /** Tarea «disk-capacity:LETRA». Llena el espacio libre y lo comprueba. */
  capacity: (letter: string) => invoke<DiskCapacity>("disk_capacity_test", { letter }),
  /** Tabla SMART completa (SATA) o registro de salud (NVMe). */
  smartFull: (number: number, bus: string, model: string) => invoke<SmartFull>("smart_full", { number, bus, model }),
  /** "short" · "extended" · "abort" */
  selfTest: (number: number, action: "short" | "extended" | "abort") => invoke<SelfTest>("smart_selftest", { number, action }),
  selfTestStatus: (number: number) => invoke<SelfTest>("smart_selftest_status", { number }),
  /** Tarea «disk-scan:N». Solo lectura. */
  scan: (number: number, model: string, hdd: boolean, mode: "quick" | "full", resume: boolean) => invoke<ScanResult>("disk_scan", { number, model, hdd, mode, resume }),
  scanLive: (number: number) => invoke<ScanLive | null>("disk_scan_live", { number }),
  scanLast: (number: number, model: string, size: number) => invoke<ScanResult | null>("disk_scan_last", { number, model, size }),
  watchGet: () => invoke<WatchConfig>("diskwatch_get"),
  watchSet: (config: WatchConfig) => invoke<WatchConfig>("diskwatch_set", { config }),
  eject: (letter: string) => invoke<void>("disk_eject", { letter }),
  format: (letter: string, fs: string, label: string) => invoke<void>("disk_format", { letter, fs, label }),
};

/** Dónde guarda AdminOps sus datos (Ajustes → Datos). */
export interface StorageInfo {
  /** Los datos viajan con el programa. */
  portable: boolean;
  /** "removable" (está en un pendrive) · "marker" (se pidió) · "" (instalado) */
  reason: string;
  /** Unidad del programa («E:»). */
  drive: string;
  canSwitch: boolean;
  migrated: { at: number; files: number; secrets: number; browser: boolean; fromHost: string } | null;
  /** El navegador interno (sesiones de Correo, Teams…) se guarda en el pendrive. */
  browserOnUsb: boolean;
}

export const storageApi = {
  info: () => invoke<StorageInfo>("storage_info"),
  /** Guarda todo en la carpeta del programa al volver a abrir AdminOps. */
  makePortable: () => invoke<void>("storage_make_portable"),
  /** Surte efecto al volver a abrir AdminOps. */
  setBrowserOnUsb: (on: boolean) => invoke<void>("storage_set_browser_on_usb", { on }),
};

export interface MigrateEstimate {
  id: string;
  label: string;
  bytes: number;
  files: number;
  note: string | null;
  available: boolean;
}

export interface MigrateSummary {
  folder: string;
  files: number;
  bytes: number;
  cloudOnly: number;
  unchanged: number;
  renamed: number;
  errors: string[];
  notes: string[];
  cancelled: boolean;
}

export interface BackupManifest {
  app: string;
  version: string;
  created: string;
  computer: string;
  user: string;
  items: { id: string; label: string; files: number; bytes: number; cloudOnly: number; errors: number }[];
}

export const migrateApi = {
  profiles: () => invoke<{ sid: string; name: string; isTarget: boolean }[]>("migrate_profiles"),
  estimate: (sid: string) => invoke<MigrateEstimate[]>("migrate_estimate", { sid }),
  pickFolder: () => invoke<string | null>("migrate_pick_folder"),
  backup: (sid: string, items: string[], dest: string) => invoke<MigrateSummary>("migrate_backup", { sid, items, dest }),
  readBackup: (folder: string) => invoke<BackupManifest>("migrate_read_backup", { folder }),
  restore: (folder: string, items: string[]) => invoke<MigrateSummary>("migrate_restore", { folder, items }),
};
