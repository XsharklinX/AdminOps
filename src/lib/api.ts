import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { trackCall } from "./perf";
import { humanError, onInternalError } from "./errors";

/** Todas las llamadas al sistema pasan por aquí: los errores llegan ya traducidos. */
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await trackCall(command, tauriInvoke<T>(command, args));
  } catch (e) {
    throw humanError(command, e);
  }
}

// Los fallos internos se anotan en el registro técnico (sin pasar por `invoke`, para no repetirse).
onInternalError((message) => void tauriInvoke("log_frontend_error", { message }).catch(() => {}));

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
  /** Arreglar un hallazgo del diagnóstico (aplica o ejecuta, según el ajuste). */
  fixFinding: (id: string) => invoke<string>("fix_finding", { id }),
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
  /** Se arregla en el sitio: ejecuta ese ajuste del catálogo. `safe`: entra en «Arreglar todo lo seguro». */
  | { kind: "fix"; label: string; id: string; safe: boolean }
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
  /** Diferencia con el análisis anterior de este equipo. */
  changes?: DiagChanges;
}

export interface DiagChanges {
  since: number;
  /** Títulos de los problemas que antes no estaban. */
  new: string[];
  resolved: Finding[];
}

export interface SnapshotInfo {
  timestamp: number;
  bad: number;
  warn: number;
}

export const diagApi = {
  /** `force`: vuelve a consultar hardware y winget en vez de reutilizar lo de hace unos minutos. */
  run: (force = false) => invoke<Diagnostics>("run_diagnostics", { force }),
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

export interface StartupTiming {
  steps: { name: string; atMs: number; ms: number }[];
  /** ms desde que arrancó el proceso hasta la llamada. */
  nowMs: number;
}

export const appApi = {
  info: () => invoke<AppInfo>("get_app_info"),
  /** Pasos del arranque del programa (Ajustes → Rendimiento). */
  startupTiming: () => invoke<StartupTiming>("startup_timing"),
  logTiming: (summary: string) => invoke<void>("log_timing", { summary }),
  /** La interfaz ya está pintada: el programa enseña la ventana. */
  uiReady: () => invoke<void>("ui_ready").catch(() => {}),
  /** El código de la interfaz arrancó (aún sin pintar): WebView2 está vivo. */
  uiBooting: () => invoke<void>("ui_booting").catch(() => {}),
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
  /** Última modificación en segundos (solo archivos). */
  modified?: number;
}

export interface SpaceView {
  root: string;
  size: number;
  files: number;
  denied: number;
  seconds: number;
  children: SpaceEntry[];
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
  spaceChildren: (path: string) => invoke<SpaceEntry[]>("space_children", { path }),
  revealInExplorer: (path: string) => invoke<void>("reveal_in_explorer", { path }),
  /** Dónde se puede recuperar espacio en este equipo, medido de verdad. */
  spaceFreeable: () => invoke<Freeable[]>("space_freeable"),
  /** Manda a la papelera lo marcado en el análisis; devuelve los bytes liberados. */
  spaceRecycle: (paths: string[]) => invoke<number>("space_recycle", { paths }),
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
  notifyTasks: boolean;
  restorePoints: "risky" | "always" | "never";
  autoCleanupMonths: number;
  checkUpdates: boolean;
  watchWindows: boolean;
  visitTypes: VisitType[];
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
  inventory: MachineInventory | null;
}

export interface MachineInventory {
  manufacturer: string;
  model: string;
  serial: string;
  cpu: string;
  cores: number;
  ramGb: number;
  disks: string;
  gpu: string;
  os: string;
  biosYear: number | null;
  installed: string;
  tpm: boolean | null;
  secureBoot: boolean | null;
  security: number | null;
  battery: number | null;
  ip: string;
  mac: string;
  verdict: "ok" | "upgrade" | "replace";
  reasons: string[];
  updated: number;
}

export interface MapDevice {
  ip: string;
  mac: string;
  name: string;
  vendor: string;
  alias: string;
}

export interface NetworkMap {
  name: string;
  gateway: string;
  saved: number;
  devices: MapDevice[];
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
  visitType: string;
  contactId: string;
}

/** Qué es cada cosa de la agenda. */
export type AgendaKind = "visit" | "task" | "call" | "meeting";

export interface Visit {
  id: string;
  /** visit · task · call · meeting ("" en las antiguas: visita). */
  kind: AgendaKind | "";
  /** Qué es. Obligatorio si no hay cliente. */
  title: string;
  /** Cliente, si es una visita a uno ("" si no). */
  clientId: string;
  clientName: string;
  /** Inicio en segundos (UTC). */
  start: number;
  minutes: number;
  machines: number;
  notes: string;
  status: "planned" | "done" | "cancelled";
  reminded: boolean;
  /** Tipo de visita configurado en Ajustes (trae su checklist a la sesión). */
  visitType: string;
  /** "" no se repite · weekly · biweekly · monthly · quarterly · semiannual · yearly */
  repeatEvery: string;
  /** Dónde es: sede, planta, sala. */
  place: string;
  /** Id del evento de Outlook, si se puso en el calendario ("" si no). */
  outlookEvent?: string;
  /** Cuándo se marcó como hecha (segundos); 0 o ausente si no se sabe. */
  doneAt?: number;
}

/** Cada cuánto se repite una visita, para el desplegable. */
export const REPEATS: { value: string; label: string }[] = [
  { value: "", label: "No se repite" },
  { value: "weekly", label: "Cada semana" },
  { value: "biweekly", label: "Cada 2 semanas" },
  { value: "monthly", label: "Cada mes" },
  { value: "quarterly", label: "Cada 3 meses" },
  { value: "semiannual", label: "Cada 6 meses" },
  { value: "yearly", label: "Cada año" },
];

export interface DueClient {
  clientId: string;
  name: string;
  date: number;
  machines: number;
  phone: string;
  email: string;
}

export const agendaApi = {
  list: () => invoke<{ visits: Visit[]; due: DueClient[] }>("list_agenda"),
  /** `conflict`: cliente de la visita con la que se pisa ("" si ninguna). */
  save: (visit: Visit) => invoke<{ visit: Visit; conflict: string }>("save_visit", { visit }),
  /** Al darla por hecha devuelve la siguiente de la serie, si se repite. */
  setStatus: (id: string, status: Visit["status"]) => invoke<Visit | null>("set_visit_status", { id, status }),
  remove: (id: string) => invoke<void>("delete_visit", { id }),
  /** Aplaza (o adelanta, con días negativos) una visita. */
  postpone: (id: string, days: number) => invoke<void>("postpone_visit", { id, days }),
};

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
  network: NetworkMap | null;
  /** Plantilla de informe de este cliente. */
  report?: ClientReport;
}

export interface ClientReport {
  /** null: el formato de siempre (para el cliente). */
  template: Template | null;
  /** Texto al principio del informe. */
  intro: string;
  /** Otros destinatarios, además del correo del cliente. */
  to: string;
  subject: string;
  body: string;
}

export const EMPTY_CLIENT_REPORT: ClientReport = { template: null, intro: "", to: "", subject: "", body: "" };

export interface ChecklistItem {
  text: string;
  done: boolean;
  /** Tarea de AdminOps que lo marca sola (vacío: a mano). */
  auto: string;
}

export interface VisitType {
  name: string;
  items: { text: string; auto: string }[];
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
  visitType: string;
  contactId: string;
}

export interface ProfileDef {
  id: string;
  name: string;
  icon: string;
  description: string;
  items: string[];
  custom: boolean;
}

export interface VisitChange {
  label: string;
  before: string;
  after: string;
  /** null: ni mejor ni peor (otra versión de Windows, otro hardware). */
  better: boolean | null;
}

export interface MachineChanges {
  host: string;
  since: number;
  until: number;
  /** true: comparado con cómo está este equipo ahora. */
  live: boolean;
  changes: VisitChange[];
}

export interface MachineRank {
  host: string;
  value: string;
  typical: string;
  /** Cuántas veces peor que la mediana (1 = igual que el resto). */
  factor: number | null;
  worse: boolean;
}

export interface Comparison {
  label: string;
  /** Frase lista para leer. */
  summary: string;
  machines: MachineRank[];
}

export const workApi = {
  /** Cómo queda cada equipo del cliente frente a los demás (solo cifras técnicas). */
  compareMachines: (clientId: string) => invoke<Comparison[]>("compare_client_machines", { clientId }),
  /** Qué cambió en los equipos de un cliente desde la última visita. */
  visitChanges: (clientId: string) => invoke<MachineChanges[]>("visit_changes", { clientId }),
  settings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  clients: () => invoke<Client[]>("list_clients"),
  saveClient: (client: Partial<Client> & { name: string }) =>
    invoke<Client>("save_client", {
      client: { id: "", contact: "", phone: "", email: "", address: "", notes: "", created: 0, machines: [], sessions: [], network: null, ...client },
    }),
  deleteClient: (id: string) => invoke<void>("delete_client", { id }),
  session: () => invoke<ActiveSession | null>("get_session"),
  startSession: (clientId: string, visitType: string | null = null, contactId: string | null = null) =>
    invoke<ActiveSession>("start_session", { clientId, visitType, contactId }),
  updateSession: (session: ActiveSession) => invoke<void>("update_session", { session }),
  setNextMaintenance: (clientId: string, date: number | null) => invoke<void>("set_next_maintenance", { clientId, date }),
  inventoryAddThis: (clientId: string) => invoke<Client>("inventory_add_this", { clientId }),
  inventoryRemove: (clientId: string, host: string) => invoke<void>("inventory_remove", { clientId, host }),
  saveNetworkMap: (clientId: string, map: Omit<NetworkMap, "saved">) => invoke<void>("save_network_map", { clientId, map: { ...map, saved: 0 } }),
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
  /** Renombra la cuenta y edita su nombre completo y descripción. Devuelve el aviso a mostrar. */
  rename: (sid: string, name: string, fullName: string, description: string) => invoke<string>("rename_user", { sid, name, fullName, description }),
};

export interface WifiAdapter {
  name: string;
  description: string;
  instanceId: string;
  present: boolean;
  status: string;
  problem: number;
  problemText: string;
  powerSaving: boolean | null;
}

export interface WifiState {
  adapters: WifiAdapter[];
  radioOn: boolean | null;
  serviceRunning: boolean;
  summary: string;
  level: "ok" | "off" | "error" | "none";
}

export const wifiApi = {
  list: () => invoke<WifiProfile[]>("list_wifi_profiles"),
  forget: (name: string) => invoke<void>("forget_wifi_profile", { name }),
  state: () => invoke<WifiState>("wifi_state"),
  setRadio: (on: boolean) => invoke<boolean>("set_wifi_radio", { on }),
  restartAdapter: (instanceId: string) => invoke<void>("restart_wifi_adapter", { instanceId }),
  removeGhosts: () => invoke<number>("remove_ghost_wifi"),
  powerSavingOff: (name: string) => invoke<void>("wifi_power_saving_off", { name }),
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

export interface Preflight {
  online: boolean;
  free: number;
  lowSpace: boolean;
  winget: boolean;
  elevated: boolean;
  /** Avisos en claro, listos para mostrar. */
  warnings: string[];
}

export const appsApi = {
  /** Red, espacio, winget y permisos antes de una instalación larga. */
  preflight: (count: number) => invoke<Preflight>("install_preflight", { count }),
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

/** Qué le pasa a una impresora y qué hacer. */
export interface PrinterCheck {
  level: "ok" | "warn" | "bad";
  title: string;
  text: string;
  spooler: boolean;
  /** Dirección del puerto TCP/IP, si es de red. */
  host: string;
  /** Responde en la red (null: no es de red). */
  reachable: boolean | null;
  jobs: number;
  oldestJobMin: number;
  /** Lo que cuenta la propia impresora por SNMP (null si no contesta). */
  device: PrinterDevice | null;
}

export interface PrinterSupply {
  name: string;
  /** 0-100, o null si la impresora no lo sabe. */
  percent: number | null;
  /** Tóner o tinta (lo que se cambia a menudo). */
  consumable: boolean;
}

export interface PrinterDevice {
  reachable: boolean;
  name: string;
  model: string;
  pages: number | null;
  supplies: PrinterSupply[];
  /** Avisos del aparato, en español («Atasco de papel»). */
  alerts: string[];
}

/** Impresora vista en la red. */
export interface FoundPrinter {
  ip: string;
  /** Puertos de impresión abiertos: 9100 RAW, 631 IPP, 515 LPD. */
  ports: number[];
  installed: boolean;
  /** Nombre y modelo con que se anuncia ("" si no lo dice). */
  name: string;
  model: string;
  /** mdns · wsd · ports */
  via: string;
}

export const printersApi = {
  list: () => invoke<PrinterInfo[]>("list_printers"),
  check: (name: string) => invoke<PrinterCheck>("check_printer", { name }),
  findOnNetwork: () => invoke<FoundPrinter[]>("find_network_printers"),
  clearQueue: (name: string) => invoke<void>("clear_printer_queue", { name }),
  testPage: (name: string) => invoke<void>("print_test_page", { name }),
  setDefault: (name: string) => invoke<void>("set_default_printer", { name }),
  remove: (name: string) => invoke<void>("remove_printer", { name }),
};

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

// ---------- Tickets y dominio (v0.13) ----------

export interface Portal {
  id: string;
  name: string;
  url: string;
  extraDomains: string[];
  /** "" Tickets · "inventory" inventario web · "mail" correo · "teams" Teams. */
  kind?: string;
  /** 1 = 100 % (0 o ausente también). */
  zoom?: number;
  /** "" en la misma vista · "window" en una ventana aparte. */
  popups?: string;
  /** Sesión privada: se cierra al salir de AdminOps y no se guarda nada en el equipo. */
  private?: boolean;
  /** Rellenar el inicio de sesión con la cuenta guardada. */
  autofill?: boolean;
}

export type PortalAction = "back" | "forward" | "reload" | "stop" | "home" | "print";

// ---------- Personas (dominio) ----------

/** Resultado de buscar personas en el dominio. */
export interface PersonHit {
  sam: string;
  name: string;
  department: string;
  title: string;
  mail: string;
  phone: string;
  extension: string;
  disabled: boolean;
  /** Tuvo un bloqueo (puede haber caducado; el estado real está en la ficha). */
  lockedHint: boolean;
}

/** La ficha de una persona del dominio. Fechas en segundos Unix (0 = no se sabe). */
export interface Person {
  sam: string;
  /** Usuario de Microsoft 365 (normalmente el correo). */
  upn: string;
  name: string;
  department: string;
  title: string;
  office: string;
  mail: string;
  phone: string;
  extension: string;
  mobile: string;
  manager: string;
  groups: string[];
  disabled: boolean;
  locked: boolean;
  passwordExpired: boolean;
  passwordNeverExpires: boolean;
  passwordLastSet: number;
  passwordExpires: number;
  lastLogon: number;
  /** Equipos donde tiene la sesión abierta, según la última comprobación de Puestos. */
  signedInOn: string[];
}

export interface LapsPassword {
  found: boolean;
  account: string;
  password: string;
  expires: number;
  source: string;
}

export interface RecoveryKey {
  computer: string;
  name: string;
  password: string;
  created: number;
  keyId: string;
}

export const peopleApi = {
  search: (query: string) => invoke<PersonHit[]>("search_people", { query }),
  details: (sam: string) => invoke<Person>("person_details", { sam }),
  unlock: (sam: string) => invoke<void>("unlock_account", { sam }),
  /** Devuelve la contraseña temporal para dictarla. */
  resetPassword: (sam: string, mustChange: boolean, unlock: boolean) => invoke<string>("reset_domain_password", { sam, mustChange, unlock }),
  laps: (computer: string) => invoke<LapsPassword>("laps_password", { computer }),
  bitlocker: (computer: string | null, keyId: string | null) => invoke<RecoveryKey[]>("bitlocker_recovery", { computer, keyId }),
};

// ---------- Microsoft 365 (Graph) ----------

export interface GraphStatus {
  /** Hay inquilino e id de aplicación. */
  configured: boolean;
  /** Hay sesión guardada. */
  connected: boolean;
  account: string;
  tenant: string;
  clientId: string;
}

export interface DeviceCode {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

export interface LoginPoll {
  state: "pending" | "done" | "expired" | "declined" | "error";
  message: string;
  account: string;
}

/** Un inicio de sesión de Entra ID, con el motivo explicado. */
export interface SignIn {
  when: string;
  ok: boolean;
  code: number;
  reason: string;
  app: string;
  client: string;
  ip: string;
  place: string;
  mfa: string;
  conditionalAccess: string;
}

export interface AuthMethod {
  kind: string;
  id: string;
  label: string;
  /** La contraseña no se quita: el resto sí. */
  removable: boolean;
}

export interface ServiceIssue {
  id: string;
  service: string;
  title: string;
  /** incident (no funciona) · advisory (funciona con limitaciones) */
  classification: string;
  status: string;
  impact: string;
  start: string;
}

export const graphApi = {
  status: () => invoke<GraphStatus>("graph_status"),
  configure: (tenant: string, clientId: string) => invoke<void>("graph_configure", { tenant, clientId }),
  loginStart: () => invoke<DeviceCode>("graph_login_start"),
  loginPoll: () => invoke<LoginPoll>("graph_login_poll"),
  logout: () => invoke<void>("graph_logout"),
  /** Abre microsoft.com/devicelogin en el navegador. */
  openDeviceLogin: () => invoke<void>("graph_open_devicelogin"),
  signins: (upn: string) => invoke<SignIn[]>("graph_signins", { upn }),
  mfaMethods: (upn: string) => invoke<AuthMethod[]>("graph_mfa_methods", { upn }),
  mfaRemove: (upn: string, kind: string, id: string) => invoke<void>("graph_mfa_remove", { upn, kind, id }),
  revokeSessions: (upn: string) => invoke<void>("graph_revoke_sessions", { upn }),
  serviceHealth: () => invoke<ServiceIssue[]>("graph_service_health"),
  /** Devuelve el id del evento de Outlook. */
  calendarSync: (visitId: string) => invoke<string>("graph_calendar_sync", { visitId }),
  teamsSend: (upn: string, text: string) => invoke<void>("graph_teams_send", { upn, text }),
  /** Trae a la Agenda lo que se movió o borró en Outlook. */
  calendarPull: () => invoke<{ updated: string[]; unlinked: string[] }>("graph_calendar_pull"),
  /** Eventos de Outlook entre dos fechas (segundos), sin los que ya son visitas. */
  calendarView: (from: number, to: number) => invoke<OutlookEvent[]>("graph_calendar_view", { from, to }),
  /** Presencia de Teams por correo (en minúsculas). */
  presence: (emails: string[]) => invoke<Record<string, TeamsPresence>>("graph_presence", { emails }),
  /** Fotos de Microsoft 365 (data URL) por correo; las que no hay, no vienen. */
  photos: (emails: string[]) => invoke<Record<string, string>>("graph_photos", { emails }),
};

export interface OutlookEvent {
  id: string;
  subject: string;
  start: number;
  end: number;
  location: string;
  allDay: boolean;
}

export interface TeamsPresence {
  /** Available · Busy · DoNotDisturb · Away · BeRightBack · Offline · PresenceUnknown */
  availability: string;
  activity: string;
}

// ---------- Configuración de empresa ----------

export interface CompanyPreview {
  path: string;
  company: string;
  domain: string;
  portalsNew: string[];
  portalsExisting: number;
  graph: boolean;
  visitTypes: number;
  catalog: number;
}

export const companyApi = {
  /** Nombre del archivo guardado, o null si se canceló. */
  export: () => invoke<string | null>("company_export"),
  preview: () => invoke<CompanyPreview | null>("company_import_preview"),
  apply: (path: string, settings: boolean, portals: boolean, graph: boolean) => invoke<string>("company_import_apply", { path, settings, portals, graph }),
};

// ---------- Seguimientos y nota de llamada ----------

/** «Volver a mirar esto el jueves». Fechas en segundos Unix. */
export interface Followup {
  id: string;
  text: string;
  due: number;
  done: boolean;
  person: string;
  machine: string;
  created: number;
  notified: boolean;
}

export const followupsApi = {
  list: () => invoke<Followup[]>("list_followups"),
  add: (f: Pick<Followup, "text" | "due"> & Partial<Pick<Followup, "person" | "machine">>) =>
    invoke<Followup>("add_followup", { followup: { id: "", done: false, created: 0, notified: false, person: "", machine: "", ...f } }),
  setDone: (id: string, done: boolean) => invoke<void>("set_followup_done", { id, done }),
  snooze: (id: string, days: number) => invoke<void>("snooze_followup", { id, days }),
  remove: (id: string) => invoke<void>("delete_followup", { id }),
};

/** Lo que se tapó en un recorte (evento «screen-clip» y «Tapar datos»). */
export interface ClipRedacted {
  covered: number;
  words: number;
  /** "" si fue bien; si no, por qué (el recorte queda como estaba). */
  error: string;
}

export const noteApi = {
  /** La ventana de la nota de llamada (también con Ctrl+Alt+N desde cualquier sitio). */
  open: () => invoke<void>("open_quick_note"),
  close: () => invoke<void>("close_quick_note"),
  /** El recorte de pantalla de Windows: lo recortado queda en el portapapeles. */
  screenClip: () => invoke<void>("open_screen_clip"),
  /** Tapa con el OCR de Windows las rutas y nombres de la imagen del portapapeles. */
  redactClipboard: () => invoke<ClipRedacted>("redact_clipboard_image"),
};

/** Mañana a las 9:00 (hora local), en segundos Unix: la fecha por defecto de un seguimiento. */
export function tomorrowMorning(days = 1): number {
  const d = new Date();
  d.setDate(d.getDate() + days);
  d.setHours(9, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
}

// ---------- El caso de ahora ----------

export interface Case {
  id: string;
  ticket: string;
  person: string;
  sam: string;
  machine: string;
  notes: string;
  started: number;
  /** 0 mientras está abierto. */
  ended: number;
  resolution: string;
}

export interface CaseAction {
  at: number;
  title: string;
  ok: boolean;
}

export const casesApi = {
  current: () => invoke<Case | null>("case_current"),
  open: (c: Partial<Case>) => invoke<Case>("case_open", { case: emptyCase(c) }),
  update: (c: Partial<Case>) => invoke<Case>("case_update", { case: emptyCase(c) }),
  actions: () => invoke<CaseAction[]>("case_actions"),
  draft: () => invoke<string>("case_draft"),
  close: (resolution: string) => invoke<Case>("case_close", { resolution }),
  discard: () => invoke<void>("case_discard"),
  forPerson: (sam: string, person: string) => invoke<Case[]>("cases_for_person", { sam, person }),
};

function emptyCase(c: Partial<Case>): Case {
  return { id: "", ticket: "", person: "", sam: "", machine: "", notes: "", started: 0, ended: 0, resolution: "", ...c };
}

export const portalsApi = {
  list: () => invoke<Portal[]>("list_portals"),
  save: (portal: Portal) => invoke<Portal>("save_portal", { portal }),
  remove: (id: string) => invoke<void>("delete_portal", { id }),
  show: (id: string, r: { x: number; y: number; width: number; height: number }) => invoke<void>("portal_show", { id, ...r }),
  /** Cargarlo en segundo plano para que al entrar ya esté listo. */
  preload: (id: string, width: number, height: number) => invoke<void>("portal_preload", { id, width, height }),
  bounds: (id: string, r: { x: number; y: number; width: number; height: number }) => invoke<void>("portal_bounds", { id, ...r }),
  hideAll: () => invoke<void>("portal_hide_all"),
  /** Cierra las vistas que llevan mucho sin usarse (cada una es un proceso). */
  closeIdle: () => invoke<number>("portal_close_idle"),
  hide: (id: string) => invoke<void>("portal_hide", { id }),
  /** Destruye la vista (para las que no arrancaron: ocultarlas no las quita de encima). */
  reset: (id: string) => invoke<void>("portal_reset", { id }),
  /** Escribe en el campo seleccionado del portal. `false`: no había portal abierto o campo de texto seleccionado. */
  insertText: (id: string, text: string) => invoke<boolean>("portal_insert_text", { id, text }),
  nav: (id: string, action: PortalAction) => invoke<void>("portal_nav", { id, action }),
  /** `false`: la dirección está fuera del portal y se abrió en el navegador. */
  go: (id: string, url: string) => invoke<boolean>("portal_go", { id, url }),
  /** Permitir que el portal navegue por un sitio que se abrió fuera (queda guardado). */
  allowDomain: (id: string, host: string) => invoke<void>("portal_allow_domain", { id, host }),
  zoom: (id: string, zoom: number) => invoke<number>("portal_zoom", { id, zoom }),
  find: (id: string, text: string, backwards: boolean) => invoke<void>("portal_find", { id, text, backwards }),
  login: (id: string) => invoke<{ user: string; hasPassword: boolean } | null>("portal_login_get", { id }),
  /** `password`: undefined deja la guardada; "" la borra. */
  setLogin: (id: string, user: string, password?: string) => invoke<void>("portal_login_set", { id, user, password: password ?? null }),
  /** Mensaje nuevo en el Correo de AdminOps; `false` si no hay correo configurado. */
  compose: (to: string, subject?: string, body?: string) => invoke<boolean>("portal_compose", { to, subject: subject ?? null, body: body ?? null }),
  /** Chat o llamada de Teams dentro de AdminOps. `false`: no hay Teams configurado. */
  teams: (email: string, call: boolean) => invoke<boolean>("portal_teams", { email, call }),
  signOut: (id: string) => invoke<void>("portal_sign_out", { id }),
  openDownload: (download: number) => invoke<void>("portal_download_open", { download }),
  revealDownload: (download: number) => invoke<void>("portal_download_reveal", { download }),
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
  /** MSI | Inno Setup | NSIS | Propio (vacío: solo con su asistente). */
  silentKind: string;
  repairable: boolean;
  /** Runtimes, redistribuibles, drivers. */
  component: boolean;
  orphan: boolean;
  perUser: boolean;
}

export interface Leftover {
  kind: "folder" | "shortcut" | "registry";
  path: string;
  size: number;
  files: number;
}

export const programsApi = {
  list: () => invoke<InstalledProgram[]>("list_programs"),
  uninstall: (id: string, silent: boolean) => invoke<{ removed: boolean; message: string; leftovers: Leftover[] }>("uninstall_program", { id, silent }),
  removeLeftovers: (id: string, paths: string[]) => invoke<number>("remove_leftovers", { id, paths }),
  removeOrphan: (id: string) => invoke<void>("remove_orphan_entry", { id }),
  scanLeftovers: (id: string) => invoke<Leftover[]>("scan_leftovers", { id }),
  repair: (id: string) => invoke<string>("repair_program", { id }),
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
  mac: string;
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
  kind: string;
  manufacturer: string;
  model: string;
  friendly: string;
  os: string;
  services: string[];
  ports: number[];
  netbios: string;
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
  identify: (key: string, devices: LanDevice[]) => invoke<LanDevice[]>("identify_lan", { key, devices }),
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

// ---------- La oficina completa (v0.22) ----------

export interface WolAdapter {
  name: string;
  description: string;
  magicPacket: string;
  wired: boolean;
}

export interface RemoteStatus {
  rdpEnabled: boolean;
  rdpSupported: boolean;
  fastStartup: boolean;
  adapters: WolAdapter[];
  host: string;
  ip: string;
  mac: string;
}

export interface Share {
  name: string;
  path: string;
  description: string;
  access: ShareAccess[];
  openFiles: number;
  /** La carpeta compartida ya no existe en el disco. */
  missingPath: boolean;
  /** Se comparte con «Todos» pero los permisos del disco no dejan entrar. */
  ntfsBlocks: boolean;
  /** Lo que conviene revisar. */
  risks: ShareRisk[];
}

export interface ShareAccess {
  account: string;
  /** Full | Change | Read */
  right: string;
  allow: boolean;
  sid: string;
}

/** «Todos» con control total, un disco entero, carpetas personales o de Windows. */
export type ShareRisk = "everyone-full" | "whole-disk" | "profile" | "personal" | "system";
export type ShareRight = "Read" | "Change" | "Full";

/** Qué puede hacer de verdad una cuenta en una carpeta compartida, y qué se lo impide. */
export interface ShareExplain {
  verdict: "none" | "read" | "write";
  headline: string;
  shareRight: "none" | "read" | "change" | "full";
  diskRight: "none" | "read" | "write" | "unknown";
  findings: { level: "bad" | "warn" | "ok"; text: string; fix: "" | "sharing" | "permissions" }[];
}

export interface NetDrive {
  letter: string;
  path: string;
  host: string;
  /** El equipo que la sirve contesta ahora mismo. */
  reachable: boolean;
}

export interface NetDrives {
  drives: NetDrive[];
  /** Letras libres para conectar una unidad nueva. */
  free: string[];
}

export interface RemoteShare {
  name: string;
  kind: "folder" | "printer";
  remark: string;
}

export interface ShareSize {
  name: string;
  bytes: number;
  files: number;
  diskFree: number;
  diskTotal: number;
}

/** Copia diaria de una carpeta compartida a otra carpeta. */
export interface ShareBackup {
  share: string;
  dest: string;
  time: string;
  lastRun: string | null;
  lastResult: number | null;
  nextRun: string | null;
  running: boolean;
  /** La última copia terminó bien (null: aún no se ha hecho ninguna). */
  ok: boolean | null;
}

/** Archivo que alguien tiene abierto ahora mismo desde otro equipo. */
export interface OpenFile {
  name: string;
  user: string;
  locked: boolean;
}

export interface SharingStatus {
  shares: Share[];
  category: string;
  fileSharing: boolean;
  discovery: boolean;
  sessions: string[];
  open: OpenFile[];
  /** Nombre de este equipo en la red. */
  host: string;
}

export interface IpConflict {
  time: string;
  ip: string;
  mac: string;
}

export const officeApi = {
  wake: (mac: string) => invoke<void>("wake_on_lan", { mac }),
  remoteStatus: () => invoke<RemoteStatus>("remote_status"),
  enableWol: () => invoke<void>("enable_wake_on_lan"),
  setRdp: (enabled: boolean) => invoke<void>("set_remote_desktop", { enabled }),
  rdp: (host: string) => invoke<void>("open_remote_desktop", { host }),
  /** Asistencia remota de Windows (msra.exe). */
  remoteAssistance: () => invoke<void>("open_remote_assistance"),
  shares: () => invoke<SharingStatus>("list_shares"),
  createShare: (path: string, name: string, who: string, write: boolean) => invoke<void>("create_share", { path, name, who, write }),
  removeShare: (name: string) => invoke<void>("remove_share", { name }),
  enableSharing: () => invoke<void>("enable_file_sharing"),
  /** Da o cambia el acceso de una cuenta ("everyone", un usuario local o la cuenta completa). */
  shareGrant: (name: string, account: string, right: ShareRight) => invoke<void>("share_grant", { name, account, right }),
  shareRevoke: (name: string, account: string, deny: boolean) => invoke<void>("share_revoke", { name, account, deny }),
  shareExplain: (name: string, user: string) => invoke<ShareExplain>("share_explain", { name, user }),
  shareSizes: () => invoke<ShareSize[]>("share_sizes"),
  drives: () => invoke<NetDrives>("network_drives"),
  mapDrive: (letter: string, path: string) => invoke<void>("map_network_drive", { letter, path }),
  unmapDrive: (letter: string) => invoke<void>("unmap_network_drive", { letter }),
  reconnectDrive: (letter: string) => invoke<string>("reconnect_network_drive", { letter }),
  /** Abre una unidad o una ruta de red en el Explorador (Windows pide ahí las credenciales). */
  openNetworkPath: (path: string) => invoke<void>("open_network_path", { path }),
  remoteShares: (host: string) => invoke<RemoteShare[]>("remote_shares", { host }),
  shareBackups: () => invoke<ShareBackup[]>("share_backups"),
  setShareBackup: (name: string, dest: string, time: string) => invoke<void>("set_share_backup", { name, dest, time }),
  removeShareBackup: (name: string) => invoke<void>("remove_share_backup", { name }),
  runShareBackup: (name: string) => invoke<void>("run_share_backup", { name }),
  conflicts: () => invoke<IpConflict[]>("ip_conflicts"),
  exportCsv: (name: string, content: string) => invoke<string | null>("export_csv", { name, content }),
};

/** Filas → CSV con ";" (lo que espera Excel en español) y comillas donde haga falta. */
export function toCsv(rows: (string | number | null | undefined)[][]): string {
  const cell = (v: string | number | null | undefined) => {
    const t = v === null || v === undefined ? "" : String(v);
    return /[;"\n\r]/.test(t) ? `"${t.replace(/"/g, '""')}"` : t;
  };
  return rows.map((r) => r.map(cell).join(";")).join("\r\n");
}

// ---------- Bloqueo de la app y copia de la configuración (v0.23) ----------

export interface LockStatus {
  enabled: boolean;
  kind: "pin" | "password" | "";
  idleMinutes: number;
  waitSecs: number;
}

export const lockApi = {
  status: () => invoke<LockStatus>("lock_status"),
  verify: (secret: string) => invoke<boolean>("lock_verify", { secret }),
  verifyWindows: (password: string) => invoke<boolean>("lock_verify_windows", { password }),
  set: (kind: "pin" | "password", secret: string, idleMinutes: number, current: string) => invoke<void>("lock_set", { kind, secret, idleMinutes, current }),
  setIdle: (idleMinutes: number, current: string) => invoke<void>("lock_set_idle", { idleMinutes, current }),
  disable: (current: string) => invoke<void>("lock_disable", { current }),
};

export const configApi = {
  export: (prefs: unknown) => invoke<string | null>("export_config", { prefs }),
  import: () => invoke<unknown | null>("import_config"),
};

// ---------- Cuidado de la app (v1.0) ----------

export interface DataUsage {
  journalEntries: number;
  snapshots: number;
  snapshotsBytes: number;
  reports: number;
  reportsBytes: number;
  speedtests: number;
  logsBytes: number;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  newer: boolean;
  url: string;
  notes: string;
}

export const appcareApi = {
  autostart: () => invoke<boolean>("autostart_enabled"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  usage: () => invoke<DataUsage>("data_usage"),
  cleanup: (months: number, journal: boolean, snapshots: boolean, reports: boolean) =>
    invoke<{ journal: number; snapshots: number; reports: number; freed: number }>("data_cleanup", { months, journal, snapshots, reports }),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  openRelease: (url: string) => invoke<void>("open_release_page", { url }),
};

// ---------- Errores de Windows (vigilancia) ----------

export interface WindowsAlert {
  id: string;
  time: number;
  key: string;
  level: "bad" | "warn" | "info";
  title: string;
  detail: string;
  explanation: string;
  advice: string;
  page: string | null;
  count: number;
  read: boolean;
}

export const alertsApi = {
  list: () => invoke<WindowsAlert[]>("list_windows_alerts"),
  markRead: () => invoke<void>("mark_windows_alerts_read"),
  clear: () => invoke<void>("clear_windows_alerts"),
  checkNow: () => invoke<WindowsAlert[]>("check_windows_now"),
};

// ---------- Acceso remoto (agenda, herramientas) ----------

export interface RdpOptions {
  fullscreen: boolean;
  multimon: boolean;
  clipboard: boolean;
  drives: boolean;
  printers: boolean;
  audio: boolean;
}

export interface Connection {
  id: string;
  name: string;
  kind: string;
  target: string;
  username: string;
  client: string;
  notes: string;
  options: RdpOptions;
  savedPassword: boolean;
  lastUsed: number;
}

export interface Reach {
  resolved: string | null;
  pingMs: number | null;
  rdpOpen: boolean;
  hint: string;
}

export interface RemoteTool {
  id: string;
  name: string;
  installed: boolean;
  thisId: string | null;
}

export interface RdpServer {
  port: number;
  nla: boolean;
  users: string[];
}

export const remoteApi = {
  list: () => invoke<Connection[]>("list_connections"),
  save: (connection: Connection) => invoke<Connection>("save_connection", { connection }),
  remove: (id: string) => invoke<void>("delete_connection", { id }),
  setPassword: (id: string, password: string) => invoke<void>("set_connection_password", { id, password }),
  connect: (id: string) => invoke<void>("connect_saved", { id }),
  connectRdp: (target: string, username: string, options: RdpOptions) => invoke<void>("connect_rdp", { target, username, options }),
  test: (target: string) => invoke<Reach>("test_connection", { target }),
  tools: () => invoke<RemoteTool[]>("remote_tools"),
  connectTool: (tool: string, target: string) => invoke<void>("connect_tool_id", { tool, target }),
  openTool: (tool: string) => invoke<void>("open_remote_tool", { tool }),
  install: (tool: string) => invoke<void>("install_remote_tool", { tool }),
  server: () => invoke<RdpServer>("rdp_server"),
  setUser: (user: string, allow: boolean) => invoke<void>("set_rdp_user", { user, allow }),
};

// ---------- Solucionar problemas, reparación de red y línea de tiempo (1.1.1) ----------

export type Symptom = "internet" | "wifi" | "audio" | "bluetooth" | "display" | "printer" | "slow" | "winupdate";

export interface TroubleFix {
  id: string;
  label: string;
  admin: boolean;
  confirm: string | null;
}

export interface TroubleFinding {
  level: "ok" | "info" | "warn" | "bad";
  title: string;
  detail: string;
  fixes: TroubleFix[];
  page: string | null;
}

export interface NetCheck {
  connected: boolean;
  noDhcpAddress: boolean;
  adapter: string;
  gateway: boolean | null;
  internet: boolean;
  dns: boolean;
  /** IPv4 del adaptador activo. */
  ip: string;
  /** Puerta de enlace (el router). */
  gatewayIp: string;
  dnsServers: string[];
  /** La IP la da el router (DHCP) o está puesta a mano. */
  dhcp: boolean;
  wifi: boolean;
  /** Proxy configurado en Windows (vacío si no hay). */
  proxy: string;
  vpn: string[];
}

/** Qué pasa con la red y qué hacer ahora. */
export interface NetVerdict {
  level: "ok" | "warn" | "bad";
  title: string;
  text: string;
  /** Botones a ofrecer: router · dns · deep · wifi · proxy · speed. */
  next: string[];
}

export interface NetRepair {
  before: NetCheck;
  after: NetCheck;
  steps: { title: string; ok: boolean; detail: string }[];
  reboot: boolean;
  verdict: NetVerdict;
}

export const troubleshootApi = {
  check: (symptom: Symptom) => invoke<{ symptom: Symptom; findings: TroubleFinding[] }>("troubleshoot_check", { symptom }),
  fix: (id: string) => invoke<string>("troubleshoot_fix", { id }),
  repairNetwork: (deep: boolean) => invoke<NetRepair>("repair_network", { deep }),
  netCheck: () => invoke<NetCheck>("quick_net_check"),
};

export interface TimelineEvent {
  time: number;
  kind: "change" | "alert" | "scan" | "windows";
  level: "ok" | "info" | "warn" | "bad";
  title: string;
  detail: string;
  page: string | null;
}

export const timelineApi = {
  list: (days: number) => invoke<TimelineEvent[]>("machine_timeline", { days }),
};

export interface ContactChannel {
  kind: "phone" | "email";
  label: string;
  value: string;
}

export interface Contact {
  id: string;
  name: string;
  role: string;
  company: string;
  extension: string;
  phone: string;
  mobile: string;
  email: string;
  channels: ContactChannel[];
  reason: string;
  availability: string;
  substituteId: string;
  clientId: string;
  tags: string[];
  notes: string;
  favorite: boolean;
  uses: number;
  lastUsed: number;
  created: number;
  updated: number;
  deleted: number | null;
}

export type ContactBulk =
  | { op: "addTag"; tag: string }
  | { op: "removeTag"; tag: string }
  | { op: "favorite"; value: boolean }
  | { op: "delete" }
  | { op: "restore" }
  | { op: "purge" };

export const contactsApi = {
  list: () => invoke<Contact[]>("list_contacts"),
  // Las fechas y contadores van como enteros (u64/u32 en el backend).
  save: (contact: Contact) =>
    invoke<Contact>("save_contact", {
      contact: {
        ...contact,
        uses: Math.floor(contact.uses || 0),
        lastUsed: Math.floor(contact.lastUsed || 0),
        created: Math.floor(contact.created || 0),
        updated: Math.floor(contact.updated || 0),
        deleted: contact.deleted == null ? null : Math.floor(contact.deleted),
      },
    }),
  touch: (id: string) => invoke<void>("touch_contact", { id }),
  bulk: (ids: string[], action: ContactBulk) => invoke<number>("bulk_contacts", { ids, action }),
  merge: (keep: string, others: string[]) => invoke<void>("merge_contacts", { keep, others }),
  tagColors: () => invoke<{ name: string; color: string }[]>("contact_tag_colors"),
  setTagColor: (name: string, color: string) => invoke<void>("set_contact_tag_color", { name, color }),
  renameTag: (from: string, to: string) => invoke<number>("rename_contact_tag", { from, to }),
  deleteTag: (name: string) => invoke<number>("delete_contact_tag", { name }),
  backups: () => invoke<{ id: number; contacts: number }[]>("list_contact_backups"),
  backupNow: () => invoke<void>("backup_contacts_now"),
  restoreBackup: (id: number) => invoke<number>("restore_contact_backup", { id }),
  importFile: () => invoke<{ added: number; updated: number } | null>("import_contacts"),
  email: (email: string) => invoke<void>("write_email", { email }),
  teams: (email: string, call: boolean) => invoke<void>("open_teams", { email, call }),
  call: (number: string) => invoke<void>("call_number", { number }),
  saveVcard: (name: string, content: string) => invoke<string | null>("save_vcard", { name, content }),
};

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
  remote: { os: string; user: string; bootDays: number | null; freeGb: number | null; totalGb: number | null; updateDays: number | null; via: string } | null;
  remoteError: string;
  warnings: string[];
}

export type StationAction = "restart" | "cancelRestart" | "gpupdate" | "message";

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

// ---------- Cuentas (Microsoft, trabajo o escuela, Entra ID, Office, credenciales) ----------

export interface AccountsStatus {
  sessionUser: string;
  sessionKind: "local" | "microsoft" | "azuread" | "domain";
  device: {
    azureAdJoined: boolean;
    domainJoined: boolean;
    workplaceJoined: boolean;
    enterpriseJoined: boolean;
    tenantName: string;
    domainName: string;
    deviceId: string;
  };
  workAccounts: { id: string; email: string; tenant: string; scope: "device" | "user" }[];
  microsoftAccounts: string[];
  officeAccounts: { id: string; email: string; name: string; kind: string }[];
  credentials: { target: string; user: string; kind: string }[];
  hasLocalAdmin: boolean;
  otherUser: boolean;
}

export const accountsApi = {
  status: () => invoke<AccountsStatus>("accounts_status"),
  leaveAzureAd: () => invoke<string>("leave_azure_ad"),
  removeWorkAccount: (id: string) => invoke<string>("remove_work_account", { id }),
  officeSignOut: (id: string) => invoke<string>("office_sign_out", { id }),
  deleteCredential: (target: string) => invoke<void>("delete_credential", { target }),
  signOut: () => invoke<void>("sign_out_windows"),
};
