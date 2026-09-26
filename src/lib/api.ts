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

// ---------- Diagnóstico e informes (Fase 4) ----------

export type Severity = "info" | "warn" | "bad";

export interface Finding {
  severity: Severity;
  area: string;
  title: string;
  detail: string | null;
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
}

export interface Stability {
  days: number;
  bugchecks: { time: string; code: string; name: string | null; hint: string | null }[];
  unexpectedShutdowns: string[];
  crashes: { app: string; count: number; last: string }[];
  minidumps: { name: string; time: string; size: number }[] | null;
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
}

export interface SnapshotInfo {
  timestamp: number;
  bad: number;
  warn: number;
}

export const diagApi = {
  run: () => invoke<Diagnostics>("run_diagnostics"),
  snapshots: () => invoke<SnapshotInfo[]>("list_snapshots"),
  generateReport: (baseline: number | null, technician: string, client: string, notes: string) =>
    invoke<string>("generate_report", { baseline, technician, client, notes }),
  openReport: (path: string) => invoke<void>("open_report", { path }),
  revealReport: (path: string) => invoke<void>("reveal_report", { path }),
};

// ---------- Perfiles y datos de la app (Fase 5) ----------

export interface ProfileItem {
  id: string;
  name: string;
  kind: "toggle" | "action";
  risk: Risk;
  status: TweakStatus;
  supported: boolean;
  hasBackup: boolean;
}

export interface ProfileView {
  id: string;
  name: string;
  icon: string;
  description: string;
  items: ProfileItem[];
  needsAdmin: boolean;
}

export interface ProfileResult {
  results: {
    id: string;
    name: string;
    outcome: "applied" | "reverted" | "ran" | "skipped" | "failed";
    message: string | null;
  }[];
  restorePointCreated: boolean;
  reboot: boolean;
}

export interface AppInfo {
  version: string;
  portable: boolean;
  dataDir: string;
  reportsDir: string;
}

export const profilesApi = {
  list: () => invoke<ProfileView[]>("list_profiles"),
  apply: (id: string, skipRestorePoint = false) => invoke<ProfileResult>("apply_profile", { id, skipRestorePoint }),
  revert: (id: string) => invoke<ProfileResult>("revert_profile", { id }),
};

export const appApi = {
  info: () => invoke<AppInfo>("get_app_info"),
};
