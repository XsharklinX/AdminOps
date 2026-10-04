// Tickets, dominio, personas, empresa, seguimientos y el caso de ahora.
import { invoke } from "./core";

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
  /** Nombre de inicio de sesión completo (normalmente el correo). */
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

// ---------- Configuración de empresa ----------

export interface CompanyPreview {
  path: string;
  company: string;
  domain: string;
  portalsNew: string[];
  portalsExisting: number;
  visitTypes: number;
  catalog: number;
}

export const companyApi = {
  /** Nombre del archivo guardado, o null si se canceló. */
  export: () => invoke<string | null>("company_export"),
  preview: () => invoke<CompanyPreview | null>("company_import_preview"),
  apply: (path: string, settings: boolean, portals: boolean) => invoke<string>("company_import_apply", { path, settings, portals }),
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
