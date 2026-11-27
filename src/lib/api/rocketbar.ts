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
export const BAR_EVENT = "rocket://bar";

export const barApi = {
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
