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
  /** Lo que ha subido en el último mes. */
  rising: string[];
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
