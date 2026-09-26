import { invoke } from "@tauri-apps/api/core";

export interface SystemInfo {
  hostName: string;
  osName: string;
  osVersion: string;
  kernelVersion: string;
  cpuBrand: string;
  physicalCores: number;
  logicalCores: number;
  totalMemory: number;
  bootTime: number;
}

export interface DiskInfo {
  mount: string;
  name: string;
  fileSystem: string;
  kind: string;
  total: number;
  available: number;
  removable: boolean;
}

export interface ProcessInfo {
  pid: number;
  name: string;
  cpu: number;
  memory: number;
}

export interface LiveMetrics {
  cpuTotal: number;
  cpuPerCore: number[];
  memoryUsed: number;
  memoryTotal: number;
  swapUsed: number;
  swapTotal: number;
  uptime: number;
  processCount: number;
  netRxPerSec: number;
  netTxPerSec: number;
  disks: DiskInfo[];
  topProcesses: ProcessInfo[];
}

// La UI solo puede llamar a estos comandos concretos: nunca se envían
// scripts o comandos libres al backend.
export const api = {
  isAdmin: () => invoke<boolean>("is_admin"),
  relaunchAsAdmin: () => invoke<void>("relaunch_as_admin"),
  systemInfo: () => invoke<SystemInfo>("get_system_info"),
  liveMetrics: () => invoke<LiveMetrics>("get_live_metrics"),
};

// ---------- Motor de ajustes (Fase 2) ----------

export type Risk = "low" | "medium" | "high";
export type TweakStatus = "applied" | "notApplied" | "partial" | "unavailable" | "unknown" | "action";

export interface TweakView {
  id: string;
  name: string;
  description: string;
  category: string;
  risk: Risk;
  kind: "toggle" | "action";
  note: string | null;
  reboot: boolean;
  needsAdmin: boolean;
  supported: boolean;
  status: TweakStatus;
  hasBackup: boolean;
  changes: string[];
}

export interface OpResult {
  status: TweakStatus;
  message: string;
  restorePointCreated: boolean;
}

export interface JournalEntry {
  id: number;
  timestamp: number;
  op: "apply" | "revert" | "run" | "restorePoint";
  tweakId: string | null;
  title: string;
  ok: boolean;
  message: string | null;
  reverted: boolean;
  undoable: boolean;
}

export interface RestorePoint {
  sequence: number;
  description: string;
  created: string;
}

/** Prefijo de error del backend cuando no se pudo crear el punto de restauración. */
export const RP_FAILED = "RESTORE_POINT_FAILED::";

export const tweaksApi = {
  list: (category?: string) => invoke<TweakView[]>("list_tweaks", { category }),
  apply: (id: string, skipRestorePoint = false) => invoke<OpResult>("apply_tweak", { id, skipRestorePoint }),
  revert: (id: string) => invoke<OpResult>("revert_tweak", { id }),
  run: (id: string) => invoke<OpResult>("run_action", { id }),
  journal: () => invoke<JournalEntry[]>("get_journal"),
  revertEntry: (entryId: number) => invoke<OpResult>("revert_entry", { entryId }),
  createRestorePoint: () => invoke<void>("create_restore_point"),
  listRestorePoints: () => invoke<RestorePoint[]>("list_restore_points"),
  openSystemRestore: () => invoke<void>("open_system_restore"),
};

// ---------- Bloatware, Inicio y usuario destino (Fase 3) ----------

export type Advice = "remove" | "optional" | "keep";

export interface AppView {
  package: string;
  name: string;
  description: string | null;
  note: string | null;
  advice: Advice | null;
  version: string | null;
  publisher: string | null;
  installed: boolean;
  provisioned: boolean;
  reinstallable: boolean;
}

export interface RemoveSummary {
  results: { package: string; ok: boolean; message: string }[];
  restorePointCreated: boolean;
}

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  location: string;
  source: "registry" | "folder" | "task";
  enabled: boolean;
  needsAdmin: boolean;
  publisher: string | null;
  description: string | null;
  target: string | null;
}

export interface TargetUser {
  name: string;
  sid: string;
  redirected: boolean;
}

export const systemApi = {
  listApps: () => invoke<AppView[]>("list_apps"),
  removeApps: (packages: string[], skipRestorePoint = false) =>
    invoke<RemoveSummary>("remove_apps", { packages, skipRestorePoint }),
  reinstallApp: (pkg: string) => invoke<void>("reinstall_app", { package: pkg }),
  listStartup: () => invoke<StartupItem[]>("list_startup"),
  setStartupEnabled: (id: string, enabled: boolean) => invoke<void>("set_startup_enabled", { id, enabled }),
  targetUser: () => invoke<TargetUser | null>("get_target_user"),
};
