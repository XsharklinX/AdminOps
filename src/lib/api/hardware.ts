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
