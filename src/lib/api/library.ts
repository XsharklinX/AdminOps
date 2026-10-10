// Conocimiento, mapa de la oficina, puestos, ficha, copias y auditoría.
import { invoke } from "./core";

// ---------- 1.1.2: conocimiento, mapa de la oficina, puestos, ficha, copias y auditoría ----------

export interface Solution {
  id: string;
  title: string;
  problem: string;
  solution: string;
  tags: string[];
  uses?: number;
  lastUsed?: number;
  created?: number;
  updated?: number;
}

export interface TextTemplate {
  id: string;
  name: string;
  category: string;
  body: string;
  uses?: number;
  created?: number;
  updated?: number;
}

export interface PlaceNote {
  id: string;
  scope: "machine" | "network";
  key: string;
  label: string;
  text: string;
  pinned?: boolean;
  created?: number;
  updated?: number;
}

export type RecipeStep =
  | { kind: "restorePoint" }
  | { kind: "bloatware" }
  | { kind: "installList"; listId: string; listName: string }
  | { kind: "profile"; profileId: string; profileName: string }
  | { kind: "tweaks"; ids: string[] }
  | { kind: "user"; name: string; fullName: string; admin: boolean }
  | { kind: "rename"; newName: string }
  | { kind: "domain"; domain: string; ou: string }
  | { kind: "diagnostics" };

export interface Recipe {
  id: string;
  name: string;
  description: string;
  steps: RecipeStep[];
  created?: number;
  updated?: number;
}

export interface StationList {
  id: string;
  name: string;
  hosts: string[];
  created?: number;
  updated?: number;
}

type LibraryKind = "solutions" | "templates" | "notes" | "recipes" | "stations";
type LibraryItem = { solutions: Solution; templates: TextTemplate; notes: PlaceNote; recipes: Recipe; stations: StationList };

export const libraryApi = {
  list: <K extends LibraryKind>(kind: K) => invoke<LibraryItem[K][]>("library_list", { kind }),
  save: <K extends LibraryKind>(kind: K, item: Partial<LibraryItem[K]>) => invoke<LibraryItem[K]>("library_save", { kind, item }),
  remove: (kind: LibraryKind, id: string) => invoke<void>("library_delete", { kind, id }),
  touch: (kind: LibraryKind, id: string) => invoke<void>("library_touch", { kind, id }),
  place: () => invoke<{ machine: string; machineLabel: string; network: string; networkLabel: string }>("this_place"),
};

export interface DeviceMeta {
  role: string;
  contactId: string;
  notes: string;
  watch: boolean;
  ip: string;
  name: string;
  updated: number;
}

export const officeMapApi = {
  get: (key: string) => invoke<Record<string, DeviceMeta>>("office_map", { key }),
  save: (key: string, mac: string, meta: Omit<DeviceMeta, "updated">) => invoke<void>("save_device_meta", { key, mac, meta: { ...meta, updated: 0 } }),
  refreshIps: (key: string, seen: [string, string][]) => invoke<void>("refresh_device_ips", { key, seen }),
  watchStatus: (key: string) => invoke<Record<string, { up: boolean; since: number; checked: number }>>("watch_status", { key }),
};

export interface Station {
  host: string;
  ip: string;
  online: boolean;
  ms: number | null;
  ports: number[];
  remote: {
    os: string;
    user: string;
    bootDays: number | null;
    freeGb: number | null;
    totalGb: number | null;
    updateDays: number | null;
    via: string;
    /** ok · warn · bad · "" */
    diskHealth: string;
    antivirus: string;
    avOk: boolean | null;
    bsods: number | null;
  } | null;
  remoteError: string;
  warnings: string[];
}

export type StationAction = "restart" | "cancelRestart" | "gpupdate" | "message" | "spooler" | "defender";

export interface StationActionResult {
  host: string;
  ok: boolean;
  detail: string;
}

export const stationsApi = {
  check: (hosts: string[], deep: boolean) => invoke<Station[]>("check_stations", { hosts, deep }),
  /** Reiniciar (con aviso), cancelar el reinicio, actualizar directivas o enviar un mensaje a varios puestos. */
  act: (hosts: string[], action: StationAction, text?: string) => invoke<StationActionResult[]>("station_action", { hosts, action, text }),
};

export interface MachineSheet {
  host: string;
  user: string;
  domain: string;
  partOfDomain: boolean;
  manufacturer: string;
  model: string;
  serial: string;
  chassis: string;
  cpu: string;
  cores: number;
  ramGb: number;
  disks: string;
  gpu: string;
  os: string;
  osVersion: string;
  installed: string;
  license: string;
  bios: string;
  tpm: boolean | null;
  secureBoot: boolean | null;
  ip: string;
  mac: string;
  warrantyUrl: string | null;
}

export const sheetApi = {
  get: () => invoke<MachineSheet>("machine_sheet"),
  openWarranty: (url: string) => invoke<void>("open_warranty", { url }),
};

export interface StorageHealth {
  portable: boolean;
  drive: string;
  label: string;
  fileSystem: string;
  health: string;
  removable: boolean;
  total: number;
  free: number;
  dataBytes: number;
  keyPresent: boolean | null;
  /** La clave del pendrive va protegida con el PIN del bloqueo. */
  keyProtected: boolean | null;
  lastBackup: number | null;
  lastContactsBackup: number | null;
  warnings: string[];
}

export const appBackupApi = {
  backup: (password: string, prefs: unknown, machines: boolean, reports: boolean) =>
    invoke<{ path: string; files: number; bytes: number } | null>("backup_app_data", { password, prefs, machines, reports }),
  restore: (password: string) => invoke<{ files: number; created: number; prefs: unknown } | null>("restore_app_data", { password }),
  health: () => invoke<StorageHealth>("storage_health"),
};

export interface AutoBackupInfo {
  enabled: boolean;
  folder: string;
  everyDays: number;
  keep: number;
  /** Se configuró en este equipo: solo aquí se hace sola. */
  here: boolean;
  configuredOn: string;
  lastBackup: number | null;
  nextDue: number | null;
  lastError: string;
  onedrive: string | null;
}

export const autoBackupApi = {
  info: () => invoke<AutoBackupInfo>("autobackup_info"),
  /** Carpeta vacía: desactiva. Contraseña vacía: mantiene la que había. */
  set: (folder: string, password: string, everyDays: number, keep: number) => invoke<void>("autobackup_set", { folder, password, everyDays, keep }),
  runNow: () => invoke<{ path: string; files: number; bytes: number }>("autobackup_run_now"),
};

export const auditApi = {
  get: () => invoke<boolean>("audit_mode"),
  set: (on: boolean) => invoke<void>("set_audit_mode", { on }),
};
