// Hardware, herramientas, usuarios y Wi-Fi.
import { invoke } from "./core";

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

/** Una semana del historial de la batería que apunta Windows. */
export interface BatteryPoint {
  day: string;
  full: number;
  design: number;
}

export interface BatteryHistory {
  name: string;
  design: number;
  full: number;
  cycles: number | null;
  points: BatteryPoint[];
  /** Puntos de capacidad que pierde al mes (último año). */
  lossPerMonth: number | null;
  /** Meses hasta quedar a la mitad, a este ritmo. */
  monthsToHalf: number | null;
}

export const hwApi = {
  /** null: el equipo no tiene batería. */
  batteryHistory: () => invoke<BatteryHistory | null>("battery_history"),
  inventory: () => invoke<Inventory>("hardware_inventory"),
  sensors: () => invoke<Sensors>("read_sensors"),
  smart: () => invoke<SmartDisk[]>("smart_status"),
  memoryTest: () => invoke<MemoryTest | null>("memory_test_result"),
  installPawnio: () => invoke<string>("install_pawnio"),
  openNotices: () => invoke<void>("open_third_party_notices"),
};

// ---------- Dispositivos conectados (Hardware → Conectados) ----------

export interface ConnectedDevice {
  /** Id de instancia de Windows. */
  id: string;
  name: string;
  /** La clase en español (Audio, Cámaras, USB…). */
  class: string;
  manufacturer: string;
  status: "ok" | "noDriver" | "error";
  /** Código de error de Windows (0: ninguno). */
  code: number;
  /** El estado dicho en una frase. */
  problem: string;
  driverVersion: string | null;
  /** AAAA-MM-DD. */
  driverDate: string | null;
  system: boolean;
}

export interface ConnectedList {
  devices: ConnectedDevice[];
  /** Cuántos del sistema se ocultaron. */
  hidden: number;
}

export interface RecentChange {
  kind: "arrived" | "removed";
  /** Milisegundos desde 1970. */
  at: number;
  device: ConnectedDevice;
}

export const connectedApi = {
  list: (showAll: boolean) => invoke<ConnectedList>("connected_list", { showAll }),
  recent: () => invoke<RecentChange[]>("connected_recent"),
  /** `scan` (buscar driver en Windows Update), `restart` o `enable`. Pide administrador. */
  action: (action: "scan" | "restart" | "enable", id: string) => invoke<string>("connected_action", { action, id }),
};

// ---------- Pantallas y monitores (Estado del equipo → Pantallas) ----------

export interface DisplayMonitor {
  /** Ruta del dispositivo de Windows. */
  id: string;
  /** «LG 24MK430». */
  name: string;
  maker: string;
  model: string;
  /** Número de pantalla de Windows. */
  number: number;
  width: number;
  height: number;
  /** Posición en el escritorio (píxeles). */
  x: number;
  y: number;
  hz: number;
  /** Lo más que admite, si se sabe. */
  maxHz: number | null;
  nativeWidth: number | null;
  nativeHeight: number | null;
  /** HDMI, DisplayPort, VGA, Interna… */
  connection: string;
  /** Escala de Windows en porcentaje. */
  scale: number;
  primary: boolean;
  /** 0, 90, 180 o 270. */
  rotation: number;
}

export interface DisplayList {
  monitors: DisplayMonitor[];
  /** Dos monitores enseñan lo mismo. */
  duplicated: boolean;
  /** Un cambio sin confirmar: se deshace solo al llegar la hora. */
  pending: { untilMs: number; title: string } | null;
}

export interface DisplayChange {
  message: string;
  /** Cuándo vuelve atrás solo (ms desde 1970). */
  untilMs: number;
  seconds: number;
}

export const displaysApi = {
  list: () => invoke<DisplayList>("displays_list"),
  /** `native` (con el id del monitor), `extend` o `clone`. Deja 15 s para confirmar. */
  apply: (action: "native" | "extend" | "clone", id?: string) => invoke<DisplayChange>("display_apply", { action, id: id ?? null }),
  confirm: () => invoke<string>("display_confirm"),
  undo: () => invoke<string>("display_undo"),
  restartDriver: () => invoke<string>("display_restart_driver"),
};

// ---------- Teclado e idioma (Estado del equipo → Teclado e idioma) ----------

export interface KbdRow {
  /** Etiqueta del idioma (es-DO). */
  tag: string;
  language: string;
  /** «Latinoamericano», «Inglés (EE. UU.)»… */
  layout: string;
  klid: string | null;
  langId: number;
  /** Es la que escribe ahora. */
  active: boolean;
}

export interface KbdState {
  rows: KbdRow[];
  languageCount: number;
  /** País de Windows (DO, ES…). */
  region: string;
  /** `1` Alt+Mayús, `2` Ctrl+Mayús, `3` ninguno, `4` acento grave. */
  shortcut: { code: string; label: string };
  hasSpanish: boolean;
  /** AdminOps está abierto con otra cuenta: estos son los idiomas del cliente y no se pueden cambiar. */
  otherUser: boolean;
  /** El idioma de Windows que se queda en la lista sin teclado (sin paquete en español), si lo hay. */
  kept: string | null;
  /** Aviso para la confirmación: quitar el idioma de Windows puede cambiarlo a Español. */
  warning: string | null;
  /** Por qué un idioma se queda sin teclado. */
  note: string | null;
}

export const kbdApi = {
  state: () => invoke<KbdState>("kbd_state"),
  /** Deja solo el primer español y quita el atajo. `entryId`: la entrada del diario, para deshacer. */
  onlySpanish: () => invoke<{ message: string; entryId: number }>("kbd_only_spanish"),
  /** Escribir con ese teclado ya (en lo abierto y en lo que se abra), sin quitar ninguno. */
  use: (langId: number, klid: string) => invoke<string>("kbd_use", { langId, klid }),
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
