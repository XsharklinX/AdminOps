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

export type Tool =
  | "deviceManager"
  | "eventViewer"
  | "reliability"
  | "windowsUpdate"
  | "windowsSecurity"
  | "activation"
  | "storage";

export type FindingAction =
  | { kind: "page"; label: string; page: string; focus: string | null }
  | { kind: "tool"; label: string; tool: Tool };

export interface Finding {
  severity: Severity;
  area: string;
  title: string;
  detail: string | null;
  actions: FindingAction[];
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
  openTool: (tool: Tool) => invoke<void>("open_system_tool", { tool }),
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
  custom: boolean;
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
  cancelTask: (task: string) => invoke<boolean>("cancel_task", { task }),
  readLog: (lines = 400) => invoke<string>("read_log", { lines }),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
};

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
  latencyMs: number;
  jitterMs: number;
  downloadMbps: number;
  uploadMbps: number;
  downloadLatencyMs: number | null;
  uploadLatencyMs: number | null;
  transferredMb: number;
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

export interface SpaceEntry {
  name: string;
  path: string;
  size: number;
  files: number;
  hasChildren: boolean;
}

export interface SpaceView {
  root: string;
  size: number;
  files: number;
  denied: number;
  seconds: number;
  children: SpaceEntry[];
  largestFiles: SpaceEntry[];
}

export const toolsApi = {
  processes: () => invoke<ProcessView[]>("list_processes"),
  killProcess: (pid: number, name: string, tree: boolean) => invoke<void>("kill_process", { pid, name, tree }),
  openProcessLocation: (pid: number) => invoke<void>("open_process_location", { pid }),
  network: () => invoke<NetworkReport>("network_diagnostics"),
  speedtest: () => invoke<SpeedResult>("run_speedtest"),
  cancelSpeedtest: () => invoke<void>("cancel_speedtest"),
  speedHistory: () => invoke<SpeedResult[]>("list_speedtests"),
  softwareUpdates: () => invoke<SoftwareUpdate[]>("list_software_updates"),
  upgradeSoftware: (ids: string[]) =>
    invoke<{ id: string; name: string; ok: boolean; message: string }[]>("upgrade_software", { ids }),
  scanSpace: (path: string) => invoke<SpaceView>("scan_space", { path }),
  cancelSpaceScan: () => invoke<void>("cancel_space_scan"),
  spaceChildren: (path: string) => invoke<SpaceEntry[]>("space_children", { path }),
  revealInExplorer: (path: string) => invoke<void>("reveal_in_explorer", { path }),
  backupDrivers: () => invoke<string>("backup_drivers"),
};

// ---------- Fase 9: técnico, clientes, sesiones, perfiles propios ----------

export interface Settings {
  technician: string;
  company: string;
  phone: string;
  email: string;
  website: string;
  logo: string | null;
  conditions: string;
  checklist: string[];
}

export interface Machine {
  host: string;
  os: string;
  firstSeen: number;
  lastSeen: number;
}

export interface SessionRecord {
  id: string;
  host: string;
  started: number;
  ended: number;
  report: string | null;
  badBefore: number;
  badAfter: number;
  warnBefore: number;
  warnAfter: number;
  workItems: number;
  notes: string;
}

export interface Client {
  id: string;
  name: string;
  contact: string;
  phone: string;
  email: string;
  address: string;
  notes: string;
  created: number;
  machines: Machine[];
  sessions: SessionRecord[];
}

export interface ChecklistItem {
  text: string;
  done: boolean;
}

export interface ActiveSession {
  id: string;
  clientId: string;
  clientName: string;
  host: string;
  started: number;
  baseline: number;
  checklist: ChecklistItem[];
  notes: string;
}

export interface ProfileDef {
  id: string;
  name: string;
  icon: string;
  description: string;
  items: string[];
  custom: boolean;
}

export const workApi = {
  settings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  clients: () => invoke<Client[]>("list_clients"),
  saveClient: (client: Partial<Client> & { name: string }) =>
    invoke<Client>("save_client", {
      client: { id: "", contact: "", phone: "", email: "", address: "", notes: "", created: 0, machines: [], sessions: [], ...client },
    }),
  deleteClient: (id: string) => invoke<void>("delete_client", { id }),
  session: () => invoke<ActiveSession | null>("get_session"),
  startSession: (clientId: string) => invoke<ActiveSession>("start_session", { clientId }),
  updateSession: (checklist: ChecklistItem[], notes: string) => invoke<void>("update_session", { checklist, notes }),
  cancelSession: () => invoke<void>("cancel_session"),
  finishSession: () => invoke<string>("finish_session"),
  saveProfile: (profile: ProfileDef) => invoke<ProfileDef>("save_custom_profile", { profile }),
  deleteProfile: (id: string) => invoke<void>("delete_custom_profile", { id }),
  exportProfiles: (ids?: string[]) => invoke<string>("export_custom_profiles", { ids: ids ?? null }),
  importProfiles: (json: string) => invoke<number>("import_custom_profiles", { json }),
};
