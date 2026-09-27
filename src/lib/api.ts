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
  index: () => invoke<{ id: string; name: string; description: string; category: string }[]>("tweak_index"),
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
  latest: () =>
    invoke<{
      timestamp: number;
      findings: Finding[];
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
  startPage: string | null;
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
  supportPackage: () => invoke<string>("support_package"),
  logError: (message: string) => invoke<void>("log_frontend_error", { message }).catch(() => {}),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
  openFolder: (kind: "data" | "reports" | "logs") => invoke<void>("open_app_folder", { kind }),
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
  onboarded: boolean;
  defaultDomain: string;
  currency: string;
  taxName: string;
  taxRate: number;
  laborWarrantyDays: number;
  maintenanceMonths: number;
  quoteValidityDays: number;
  catalog: CatalogItem[];
  techSignature: string | null;
}

export interface CatalogItem {
  name: string;
  price: number;
  part: boolean;
  warrantyDays: number;
}

export type Template = "client" | "technical";
export type DocKind = "none" | "quote" | "receipt";

export interface Line {
  description: string;
  part: boolean;
  qty: number;
  price: number;
  warrantyDays: number;
}

export interface Billing {
  kind: DocKind;
  lines: Line[];
  discount: number;
  payment: string;
}

export const EMPTY_BILLING: Billing = { kind: "none", lines: [], discount: 0, payment: "" };

export interface ReportOptions {
  baseline: number | null;
  technician: string;
  clientId: string | null;
  client: string;
  notes: string;
  template: Template;
  billing: Billing;
  problem: string;
  recommendations: string;
  archive: boolean;
}

export interface Warranty {
  item: string;
  until: number;
}

export interface VisitMetrics {
  bad: number;
  warn: number;
  security: number | null;
  sysFree: number | null;
  sysTotal: number | null;
  startup: number | null;
  updates: number | null;
  bootMs: number | null;
  ramTotal: number;
  batteryHealth: number | null;
}

export interface Machine {
  host: string;
  os: string;
  firstSeen: number;
  lastSeen: number;
  hardware: string;
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
  hardwareChange: string | null;
  number: string;
  docKind: DocKind;
  total: number;
  currency: string;
  warranties: Warranty[];
  nextMaintenance: number | null;
  signed: boolean;
  metrics: VisitMetrics | null;
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
  problem: string;
  recommendations: string;
  template: Template;
  billing: Billing;
  signature: string | null;
  signer: string;
  laborWarrantyDays: number;
  maintenanceMonths: number;
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
  updateSession: (session: ActiveSession) => invoke<void>("update_session", { session }),
  setNextMaintenance: (clientId: string, date: number | null) => invoke<void>("set_next_maintenance", { clientId, date }),
  cancelSession: () => invoke<void>("cancel_session"),
  finishSession: () => invoke<string>("finish_session"),
  saveProfile: (profile: ProfileDef) => invoke<ProfileDef>("save_custom_profile", { profile }),
  deleteProfile: (id: string) => invoke<void>("delete_custom_profile", { id }),
  exportProfiles: (ids?: string[]) => invoke<string>("export_custom_profiles", { ids: ids ?? null }),
  importProfiles: (json: string) => invoke<number>("import_custom_profiles", { json }),
};

// ---------- Hardware (v0.10) ----------

export interface MemoryModule {
  slot: string;
  bank: string;
  capacity: number;
  speed: number | null;
  configuredSpeed: number | null;
  manufacturer: string;
  partNumber: string;
  kind: string;
}

export interface Inventory {
  manufacturer: string;
  model: string;
  serial: string;
  boardManufacturer: string;
  boardProduct: string;
  biosVendor: string;
  biosVersion: string;
  biosDate: string | null;
  firmware: string;
  cpu: string;
  cores: number;
  threads: number;
  maxMhz: number;
  socket: string;
  virtualization: boolean | null;
  ramTotal: number;
  ramSlots: number;
  ramMax: number | null;
  modules: MemoryModule[];
  gpus: { name: string; driverVersion: string; driverDate: string | null; vram: number | null; resolution: string | null }[];
  monitors: { name: string; manufacturer: string; year: number | null }[];
  os: string;
  osVersion: string;
  osBuild: string;
  architecture: string;
  productKey: string | null;
}

export interface Sensors {
  cpuName: string | null;
  cpuTemp: number | null;
  cpuTempMax: number | null;
  cpuPower: number | null;
  gpus: { name: string; temperature: number | null; hotspot: number | null; load: number | null; fanRpm: number | null }[];
  otherTemps: [string, number][];
  fans: [string, number][];
  cpuNeedsDriver: boolean;
  pawnioInstalled: boolean;
}

export interface SmartDisk {
  model: string;
  predictFailure: boolean;
  reallocated: number | null;
  pending: number | null;
  uncorrectable: number | null;
  crcErrors: number | null;
  powerOnHours: number | null;
  temperature: number | null;
  attributes: { id: number; name: string; current: number; worst: number; raw: number }[];
}

export interface MemoryTest {
  time: string;
  passed: boolean;
  message: string;
}

export const hwApi = {
  inventory: () => invoke<Inventory>("hardware_inventory"),
  sensors: () => invoke<Sensors>("read_sensors"),
  smart: () => invoke<SmartDisk[]>("smart_status"),
  memoryTest: () => invoke<MemoryTest | null>("memory_test_result"),
  installPawnio: () => invoke<string>("install_pawnio"),
  openNotices: () => invoke<void>("open_third_party_notices"),
};

// ---------- Herramientas, usuarios y Wi-Fi (v0.11) ----------

export type ToolGroup = "console" | "admin" | "diag" | "panel" | "settings" | "folders" | "boot" | "remote";

export interface ToolView {
  id: string;
  name: string;
  description: string;
  group: ToolGroup;
  icon: string;
  keywords: string;
  asAdmin: boolean;
  needsAdmin: boolean;
  confirm: string | null;
  unavailable: string | null;
}

export type CustomKind = "program" | "folder" | "url";

export interface CustomTool {
  id: string;
  name: string;
  kind: CustomKind;
  target: string;
  args: string;
  icon: string;
}

export interface ToolboxView {
  tools: ToolView[];
  custom: CustomTool[];
  favorites: string[];
  elevated: boolean;
}

export interface LocalUser {
  name: string;
  fullName: string;
  description: string;
  sid: string;
  enabled: boolean;
  admin: boolean;
  microsoft: boolean;
  lastLogon: string | null;
  passwordLastSet: string | null;
  passwordExpires: string | null;
  hasProfile: boolean;
  signedIn: boolean;
  builtin: "administrator" | "guest" | "default" | "wdag" | null;
  isTarget: boolean;
  isSelf: boolean;
}

export interface NewUser {
  name: string;
  fullName: string;
  password: string;
  admin: boolean;
  passwordNeverExpires: boolean;
  mustChange: boolean;
}

export interface WifiProfile {
  name: string;
  ssid: string;
  authentication: string;
  password: string | null;
  protected: boolean;
  autoConnect: boolean;
  connected: boolean;
}

export const toolboxApi = {
  list: () => invoke<ToolboxView>("list_tools"),
  launch: (id: string) => invoke<void>("launch_tool", { id }),
  setFavorite: (id: string, favorite: boolean) => invoke<string[]>("set_tool_favorite", { id, favorite }),
  saveCustom: (tool: CustomTool) => invoke<CustomTool>("save_custom_tool", { tool }),
  deleteCustom: (id: string) => invoke<void>("delete_custom_tool", { id }),
  pickTarget: (kind: CustomKind) => invoke<string | null>("pick_tool_target", { kind }),
};

export const usersApi = {
  list: () => invoke<LocalUser[]>("list_users"),
  create: (user: NewUser) => invoke<void>("create_user", { user }),
  setPassword: (sid: string, password: string, mustChange: boolean) =>
    invoke<void>("set_user_password", { sid, password, mustChange }),
  setEnabled: (sid: string, enabled: boolean) => invoke<void>("set_user_enabled", { sid, enabled }),
  setAdmin: (sid: string, admin: boolean) => invoke<void>("set_user_admin", { sid, admin }),
  profileSize: (sid: string) => invoke<{ exists: boolean; size: number; files: number }>("user_profile_size", { sid }),
  remove: (sid: string, deleteProfile: boolean) => invoke<void>("delete_user", { sid, deleteProfile }),
};

export const wifiApi = {
  list: () => invoke<WifiProfile[]>("list_wifi_profiles"),
  forget: (name: string) => invoke<void>("forget_wifi_profile", { name }),
};

// ---------- Instalar programas y red avanzada (v0.12) ----------

export interface CatalogApp {
  id: string;
  name: string;
  category: string;
  source: string;
}

export interface AppList {
  id: string;
  name: string;
  description?: string;
  apps: CatalogApp[];
}

export interface AppCatalog {
  apps: CatalogApp[];
  presets: { id: string; name: string; description: string; apps: string[] }[];
  lists: AppList[];
}

export interface InstallResult {
  id: string;
  name: string;
  ok: boolean;
  message: string;
}

export const appsApi = {
  catalog: () => invoke<AppCatalog>("app_catalog"),
  installed: () => invoke<string[]>("installed_apps"),
  search: (query: string) => invoke<(CatalogApp & { version: string })[]>("search_apps", { query }),
  install: (apps: CatalogApp[]) => invoke<InstallResult[]>("install_apps", { apps }),
  saveList: (list: AppList) => invoke<AppList>("save_app_list", { list }),
  deleteList: (id: string) => invoke<void>("delete_app_list", { id }),
};

export interface Probe {
  seq: number;
  from: string | null;
  ms: number | null;
  status: "ok" | "timeout" | "unreachable" | "error";
  reached: boolean;
}

export interface PortEntry {
  protocol: "TCP" | "UDP";
  localAddress: string;
  localPort: number;
  remoteAddress: string | null;
  remotePort: number | null;
  state: string;
  pid: number;
  process: string | null;
}

export interface DnsAdapter {
  index: number;
  name: string;
  description: string;
  virtual: boolean;
  dns: string[];
  manual: boolean;
}

export const netApi = {
  ping: (host: string, count: number) => invoke<{ target: string; ip: string }>("start_ping", { host, count }),
  trace: (host: string) => invoke<{ target: string; ip: string }>("start_trace", { host }),
  stop: () => invoke<void>("stop_probe"),
  ports: () => invoke<PortEntry[]>("list_ports"),
  dnsAdapters: () => invoke<DnsAdapter[]>("list_dns_adapters"),
  setDns: (index: number, servers: string[]) => invoke<void>("set_dns", { index, servers }),
  readHosts: () => invoke<string>("read_hosts"),
  saveHosts: (content: string) => invoke<void>("save_hosts", { content }),
};

export interface PrinterInfo {
  name: string;
  driver: string | null;
  port: string | null;
  default: boolean;
  network: boolean;
  shared: boolean;
  offline: boolean;
  status: number;
  error: string | null;
  jobs: number;
  virtual: boolean;
}

export const printersApi = {
  list: () => invoke<PrinterInfo[]>("list_printers"),
  clearQueue: (name: string) => invoke<void>("clear_printer_queue", { name }),
  testPage: (name: string) => invoke<void>("print_test_page", { name }),
  setDefault: (name: string) => invoke<void>("set_default_printer", { name }),
  remove: (name: string) => invoke<void>("remove_printer", { name }),
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

// ---------- Tickets y dominio (v0.13) ----------

export interface Portal {
  id: string;
  name: string;
  url: string;
  extraDomains: string[];
  kind?: string;
}

export const portalsApi = {
  list: () => invoke<Portal[]>("list_portals"),
  save: (portal: Portal) => invoke<Portal>("save_portal", { portal }),
  remove: (id: string) => invoke<void>("delete_portal", { id }),
  show: (id: string, r: { x: number; y: number; width: number; height: number }) => invoke<void>("portal_show", { id, ...r }),
  bounds: (id: string, r: { x: number; y: number; width: number; height: number }) => invoke<void>("portal_bounds", { id, ...r }),
  hideAll: () => invoke<void>("portal_hide_all"),
  nav: (id: string, action: "back" | "forward" | "reload" | "home") => invoke<void>("portal_nav", { id, action }),
  openWindow: (id: string) => invoke<void>("portal_open_window", { id }),
  openExternal: (id: string) => invoke<void>("portal_open_external", { id }),
};

export interface DomainStatus {
  computerName: string;
  partOfDomain: boolean;
  domain: string | null;
  workgroup: string | null;
  edition: string;
  caption: string;
  canJoin: boolean;
  azureAdJoined: boolean;
  tenant: string | null;
  dc: string | null;
  secureChannel: boolean | null;
  timeOffset: number | null;
  userIsDomain: boolean | null;
}

export interface DomainCheck {
  label: string;
  status: "ok" | "warn" | "fail";
  detail: string;
}

export const domainApi = {
  status: () => invoke<DomainStatus>("domain_status"),
  check: (domain: string) => invoke<DomainCheck[]>("domain_check", { domain }),
  join: (req: { domain: string; user: string; password: string; ou: string; newName: string }) => invoke<void>("domain_join", { req }),
  leave: (req: { user: string; password: string; workgroup: string }) => invoke<void>("domain_leave", { req }),
  repair: (req: { user: string; password: string }) => invoke<void>("domain_repair", { req }),
  rename: (req: { newName: string; user: string; password: string }) => invoke<void>("rename_computer", { req }),
};

// ---------- Mantenimiento a fondo (v0.15) ----------

export interface InstalledProgram {
  id: string;
  name: string;
  publisher: string | null;
  version: string | null;
  installed: string | null;
  size: number | null;
  installLocation: string | null;
  silent: boolean;
  orphan: boolean;
  perUser: boolean;
}

export interface Leftover {
  path: string;
  size: number;
  files: number;
}

export const programsApi = {
  list: () => invoke<InstalledProgram[]>("list_programs"),
  uninstall: (id: string, silent: boolean) => invoke<{ removed: boolean; message: string; leftovers: Leftover[] }>("uninstall_program", { id, silent }),
  removeLeftovers: (id: string, paths: string[]) => invoke<number>("remove_leftovers", { id, paths }),
  removeOrphan: (id: string) => invoke<void>("remove_orphan_entry", { id }),
};

export interface UpdateHistoryEntry {
  title: string;
  date: string | null;
  result: "ok" | "partial" | "failed" | "aborted" | "progress";
  code: string | null;
  explanation: string | null;
}

export interface PendingUpdate {
  id: string;
  title: string;
  kb: string | null;
  size: number;
  hidden: boolean;
  driver: boolean;
}

export const updateApi = {
  history: () => invoke<UpdateHistoryEntry[]>("update_history"),
  pauseState: () => invoke<{ pausedUntil: string | null }>("update_pause_state"),
  pause: (days: number) => invoke<void>("update_pause", { days }),
  pending: () => invoke<PendingUpdate[]>("pending_updates"),
  setHidden: (id: string, title: string, hidden: boolean) => invoke<void>("set_update_hidden", { id, title, hidden }),
};

export interface BootAnalysis {
  boots: { time: string; totalMs: number; mainMs: number; postMs: number }[];
  culprits: { kind: "app" | "driver" | "service" | "device" | "other"; name: string; times: number; avgDelayMs: number }[];
}

export interface MaintenanceSchedule {
  weeks: number;
  day: string;
  time: string;
  tasks: string[];
}

export const maintenanceApi = {
  boot: () => invoke<BootAnalysis>("boot_analysis"),
  restoreStorage: () => invoke<{ used: number; max: number; count: number; oldest: string | null; newest: string | null }>("restore_storage"),
  deleteOldRestorePoints: () => invoke<number>("delete_old_restore_points"),
  driverBackups: () => invoke<{ name: string; drivers: number; size: number }[]>("list_driver_backups"),
  restoreDrivers: (name: string) => invoke<string>("restore_drivers", { name }),
  schedule: () =>
    invoke<{
      enabled: boolean;
      schedule: MaintenanceSchedule;
      available: [string, string][];
      lastRun: string | null;
      lastResult: number | null;
      nextRun: string | null;
      portable: boolean;
    }>("maintenance_schedule"),
  setSchedule: (schedule: MaintenanceSchedule | null) => invoke<void>("set_maintenance_schedule", { schedule }),
};

// ---------- Seguridad (v0.16) ----------

export interface SecurityCheck {
  id: string;
  label: string;
  status: "ok" | "warn" | "bad" | "unknown";
  detail: string;
  weight: number;
  fix: { label: string; page: string | null; focus: string | null; tool: string | null } | null;
}

export interface SecurityAudit {
  score: number;
  checks: SecurityCheck[];
  vulnerable: SoftwareUpdate[];
}

export interface BitlockerVolume {
  drive: string;
  protection: number;
  conversion: number;
  percent: number;
  hasRecoveryKey: boolean;
}

export interface SuspiciousItem {
  kind: "task" | "service" | "startup" | "hosts";
  name: string;
  detail: string;
  reason: string;
  id: string | null;
}

export const securityApi = {
  audit: () => invoke<SecurityAudit>("security_audit"),
  bitlocker: () => invoke<BitlockerVolume[]>("bitlocker_status"),
  keys: () => invoke<{ drive: string; id: string; key: string }[]>("bitlocker_keys"),
  exportKeys: () => invoke<string | null>("bitlocker_export"),
  suspicious: () => invoke<SuspiciousItem[]>("suspicious_items"),
  disableTask: (id: string) => invoke<void>("disable_task", { id }),
  extensions: () => invoke<{ browser: string; profile: string; name: string; id: string; enabled: boolean }[]>("browser_extensions"),
};

// ---------- Mi red y dispositivos (v0.20) ----------

export interface LanInfo {
  adapter: string;
  description: string;
  index: number;
  wireless: boolean;
  linkSpeed: string;
  ip: string;
  prefix: number;
  gateway: string;
  gatewayMac: string;
  dns: string[];
  network: string;
  category: string;
  dhcp: boolean;
  ssid: string | null;
  wifiPassword: string | null;
  wifiAuth: string;
  key: string;
}

export interface PublicIp {
  ip: string;
  city: string;
  region: string;
  country: string;
  org: string;
}

export interface RouterNote {
  level: "ok" | "info" | "warn" | "bad";
  text: string;
}

export interface RouterCheck {
  url: string;
  title: string;
  brand: string | null;
  defaultHint: string;
  openPorts: number[];
  nat: "normal" | "double" | "provider" | "cgnat" | "unknown";
  notes: RouterNote[];
}

export interface RouterProfile {
  key: string;
  name: string;
  url: string;
  username: string;
  password: string;
  notes: string;
  updated: number;
  locked: boolean;
}

export interface LanDevice {
  ip: string;
  mac: string;
  name: string;
  vendor: string;
  alias: string;
  gateway: boolean;
  thisPc: boolean;
  privateMac: boolean;
  ms: number | null;
  new: boolean;
  firstSeen: number;
}

export interface LanScan {
  key: string;
  devices: LanDevice[];
  truncated: boolean;
}

export const lanApi = {
  info: () => invoke<LanInfo | null>("lan_info"),
  publicIp: () => invoke<PublicIp>("public_ip"),
  check: (gateway: string, wifiAuth: string) => invoke<RouterCheck>("router_check", { gateway, wifiAuth }),
  routers: () => invoke<RouterProfile[]>("list_routers"),
  saveRouter: (profile: RouterProfile) => invoke<RouterProfile>("save_router", { profile }),
  deleteRouter: (key: string) => invoke<void>("delete_router", { key }),
  wifiQr: (ssid: string, password: string, auth: string) => invoke<string>("wifi_qr", { ssid, password, auth }),
  scan: () => invoke<LanScan>("scan_lan"),
  setAlias: (key: string, mac: string, alias: string) => invoke<void>("set_device_alias", { key, mac, alias }),
  vendors: (key: string, macs: string[]) => invoke<Record<string, string>>("lookup_vendors", { key, macs }),
  routerPortal: (key: string, name: string, url: string) => invoke<Portal>("router_portal", { key, name, url }),
  openDevice: (ip: string) => invoke<void>("open_device_page", { ip }),
};

// ---------- Caja fuerte, borrado, recuperación y control parental (v0.21) ----------

export interface VaultSupport {
  bitlocker: boolean;
  edition: string;
  elevated: boolean;
}

export interface VaultStatus {
  id: string;
  name: string;
  path: string;
  sizeGb: number;
  created: number;
  exists: boolean;
  mounted: boolean;
  letter: string | null;
  unlocked: boolean;
  fileSize: number;
}

export interface VaultCreated {
  id: string;
  letter: string;
  recovery: string;
}

export interface ZipOutcome {
  path: string;
  files: number;
  bytes: number;
}

export interface WipeOutcome {
  files: number;
  bytes: number;
  failed: string[];
}

export interface RecoverDrive {
  letter: string;
  label: string;
  fs: string;
  size: number;
  free: number;
  kind: string;
}

export interface UserHours {
  name: string;
  fullName: string;
  admin: boolean;
  microsoft: boolean;
  isSelf: boolean;
  hours: boolean[][];
  restricted: boolean;
}

export const vaultApi = {
  support: () => invoke<VaultSupport>("vault_support"),
  list: () => invoke<VaultStatus[]>("vault_list"),
  pickLocation: (name: string) => invoke<string | null>("vault_pick_location", { name }),
  create: (name: string, path: string, sizeGb: number, password: string) => invoke<VaultCreated>("vault_create", { name, path, sizeGb, password }),
  open: (id: string, password: string) => invoke<string>("vault_open", { id, password }),
  close: (id: string) => invoke<void>("vault_close", { id }),
  changePassword: (id: string, old: string, next: string) => invoke<void>("vault_change_password", { id, old, new: next }),
  recoveryKey: (id: string, password: string) => invoke<string>("vault_recovery_key", { id, password }),
  saveRecovery: (name: string, key: string) => invoke<string | null>("vault_save_recovery", { name, key }),
  addExisting: () => invoke<VaultStatus | null>("vault_add_existing"),
  remove: (id: string, deleteFile: boolean) => invoke<void>("vault_remove", { id, deleteFile }),
  pickFolder: () => invoke<string | null>("pick_folder"),
  pickZip: () => invoke<string | null>("pick_encrypted_zip"),
  encryptFolder: (folder: string, password: string, deleteOriginal: boolean) => invoke<ZipOutcome>("encrypt_folder", { folder, password, deleteOriginal }),
  decrypt: (path: string, password: string) => invoke<ZipOutcome>("decrypt_archive", { path, password }),
};

export const wipeApi = {
  pick: (folders: boolean) => invoke<string[]>("wipe_pick", { folders }),
  items: (paths: string[]) => invoke<WipeOutcome>("wipe_items", { paths }),
  freeSpace: (drive: string) => invoke<void>("wipe_free_space", { drive }),
  openSettings: (uri: string) => invoke<void>("open_ms_settings", { uri }),
};

export const recoverApi = {
  status: () => invoke<{ installed: boolean; drives: RecoverDrive[] }>("recover_status"),
  install: () => invoke<void>("recover_install"),
  run: (source: string, dest: string, extensive: boolean, kinds: string[], folder: string) =>
    invoke<{ folder: string; files: number }>("recover_run", { source, dest, extensive, kinds, folder }),
};

export const familyApi = {
  filterStatus: () => invoke<{ active: string; adapters: string[] }>("dns_filter_status"),
  setFilter: (filter: string) => invoke<void>("set_dns_filter", { filter }),
  blocked: () => invoke<string[]>("blocked_sites"),
  setBlocked: (sites: string[]) => invoke<string[]>("set_blocked_sites", { sites }),
  hours: () => invoke<UserHours[]>("logon_hours"),
  setHours: (name: string, hours: boolean[][]) => invoke<void>("set_logon_hours", { name, hours }),
};
