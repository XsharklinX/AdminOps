// El técnico: clientes, sesiones, ajustes e informes.
import { invoke } from "./core";

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
  notifyAlerts: "all" | "bad" | "none";
  closeMinimizes: boolean;
  trayIcon: boolean;
  visitTypes: VisitType[];
  currency: string;
  taxName: string;
  taxRate: number;
  laborWarrantyDays: number;
  maintenanceMonths: number;
  quoteValidityDays: number;
  catalog: CatalogItem[];
  techSignature: string | null;
  reportLayout: ReportLayout;
  /** Sin portada: el informe empieza en la primera página de contenido. */
  reportNoCover?: boolean;
  /** Frase corta de la pantalla de bloqueo. */
  tagline?: string;
  /** Color de la marca (#rrggbb) para esas pantallas. */
  brandColor?: string;
}

export interface CatalogItem {
  name: string;
  price: number;
  part: boolean;
  warrantyDays: number;
  /** Lo que le cuesta al técnico, para ver el margen (0: no se sabe). */
  cost?: number;
}

export type Template = "onePage" | "client" | "technical" | "custom";

/** La plantilla de informe propia: qué secciones lleva y en qué orden. */
export interface ReportLayout {
  name: string;
  /** Con todo el detalle técnico, como la plantilla técnica. */
  technical: boolean;
  sections: string[];
}
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

/** Evento del navegador que se lanza al guardar los ajustes. */
export const SETTINGS_SAVED = "adminops-settings-saved";

export const workApi = {
  /** Cómo queda cada equipo del cliente frente a los demás (solo cifras técnicas). */
  compareMachines: (clientId: string) => invoke<Comparison[]>("compare_client_machines", { clientId }),
  /** Qué cambió en los equipos de un cliente desde la última visita. */
  visitChanges: (clientId: string) => invoke<MachineChanges[]>("visit_changes", { clientId }),
  settings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { settings }).then(() => {
      // Quien dependa de un ajuste (el botón «Salir») se entera sin recargar.
      window.dispatchEvent(new Event(SETTINGS_SAVED));
    }),
  /** Cierra AdminOps de verdad, aunque la X esté puesta para minimizar. */
  quit: () => invoke<void>("quit_app"),
  /** La ventanita siempre encima con procesador, memoria, temperatura y red. */
  miniMonitor: () => invoke<void>("open_mini_monitor"),
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
