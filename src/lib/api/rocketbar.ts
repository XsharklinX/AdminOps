// La barra de herramientas del cohete (rocketbar.rs).
import { invoke } from "./core";

/** Todas, en su orden. Las de después de «app» son opcionales: apagadas de fábrica, se encienden en «Personalizar». */
export const BAR_TOOLS = ["rec", "clip", "speed", "note", "mon", "fix", "card", "help", "teams", "mail", "app", "contacts", "printer", "screens", "wifi", "keyboard", "folders", "lock", "restart", "cmd"] as const;
export type BarTool = (typeof BAR_TOOLS)[number];
/** La barra de fábrica (con administrador). */
export const DEFAULT_TOOLS: BarTool[] = ["rec", "clip", "speed", "note", "mon", "fix", "card", "help", "teams", "mail", "app"];
/** La barra de los usuarios sin administrador (en sus PC solo está el cohete): lo que usan ellos. */
export const USER_TOOLS: BarTool[] = ["rec", "clip", "speed", "note", "help", "mail", "teams"];

/** Ctrl+Alt+T: saca o esconde la barra. */
/** El nombre de cada herramienta de la barra (el aviso del botón, Personalizar y Ajustes). */
export const TOOL_LABEL: Record<BarTool, string> = {
  rec: "Grabar pantalla",
  clip: "Captura de pantalla",
  note: "Notas",
  mon: "Mini monitor",
  fix: "Arreglos rápidos",
  teams: "Teams",
  mail: "Correo",
  app: "Abrir AdminOps",
  speed: "Velocidad de internet",
  card: "Datos del equipo",
  help: "Asistencia rápida",
  contacts: "Contactos",
  printer: "La impresora",
  screens: "Pantallas: duplicar o extender",
  wifi: "Wi-Fi y Bluetooth",
  keyboard: "El teclado, en español",
  folders: "Mis carpetas",
  lock: "Bloquear el equipo",
  restart: "Reiniciar sin miedo",
  cmd: "CMD",
};

export const BAR_EVENT = "rocket://bar";
/** Ctrl+Alt+Mayús+1…9: la herramienta de ese puesto (0 a 8). */
export const BAR_TOOL_EVENT = "rocket://tool";

/** Qué barra le toca a quien la abre: administrador del equipo (completa) o usuario (la elegida por el técnico). */
export interface BarProfile {
  admin: boolean;
  userTools: string[] | null;
}

export const barApi = {
  profile: () => invoke<BarProfile>("bar_profile"),
  /** Impresora predeterminada, antivirus (y si está al día) y última actualización de Windows. */
  ticketExtras: () => invoke<{ printer: string; antivirus: string; lastUpdate: string }>("bar_ticket_extras"),
  /** Ajustes → la barra de los usuarios de este equipo; null vuelve a la de fábrica. */
  setUserTools: (tools: string[] | null) => invoke<void>("bar_set_user_tools", { tools }),
  /** Los arreglos rápidos activos (los del menú del reloj): [id, nombre]. */
  fixes: () => invoke<[string, string][]>("bar_fixes"),
  openComm: (kind: "teams" | "mail" | "outlook") => invoke<void>("bar_open_comm", { kind }),
  /** Asistencia rápida de Windows. */
  quickAssist: () => invoke<string>("bar_quick_assist"),
  /** «Wi-Fi «Oficina» · señal 68 %» o «Cable». */
  network: () => invoke<string>("bar_network"),
  /** Guarda la grabación en el Escritorio del usuario; devuelve el nombre del archivo. */
  saveDesktop: (id: string) => invoke<string>("bar_save_desktop", { id }),
  /** La herramienta de recortes; si no está, Win+Mayús+S. */
  screenClip: () => invoke<void>("bar_screen_clip"),
  /** Lo de Win+P. */
  displayMode: (mode: "clone" | "extend" | "internal" | "external") => invoke<void>("bar_display_mode", { mode }),
  /** La Wi-Fi y el Bluetooth (null: no hay o no se pudo leer) y la red conectada. */
  wireless: () => invoke<BarWireless>("bar_wireless"),
  setRadio: (kind: "wifi" | "bluetooth", on: boolean) => invoke<boolean>("bar_set_radio", { kind, on }),
  wifiReconnect: () => invoke<string>("bar_wifi_reconnect"),
  /** «desktop», «downloads», «documents» o la ruta completa de una carpeta elegida. */
  openFolder: (which: string) => invoke<void>("bar_open_folder", { which }),
  lock: () => invoke<void>("bar_lock"),
  console: (kind: "cmd" | "cmd-admin" | "powershell") => invoke<void>("bar_console", { kind }),
  /** «now», «tonight», «cancel» o «» para leer si hay un reinicio programado. */
  restart: (when: "now" | "tonight" | "cancel" | "") => invoke<{ tonight: number | null }>("bar_restart", { when }),
};

export interface BarWireless {
  wifi: boolean | null;
  bluetooth: boolean | null;
  line: string;
}
