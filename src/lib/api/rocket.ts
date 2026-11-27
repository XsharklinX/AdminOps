// El cohete de «Optimizar ya» (rocket.rs y rocketplus.rs).
import { invoke } from "./core";

export type RocketMode = "desktop" | "app" | "off";

export interface Freed {
  name: string;
  bytes: number;
}

export interface RocketBoost {
  ramFreed: number;
  ramUsedBefore: number;
  ramUsedAfter: number;
  ramTotal: number;
  trimmed: number;
  standby: boolean;
  tempFreed: number;
  tempFiles: number;
  tempSkipped: number;
  dns: boolean;
  ms: number;
  /** Qué se llevó: memoria por programa y temporales por carpeta. */
  apps: Freed[];
  folders: Freed[];
}

export interface DeepItem {
  id: "update" | "delivery" | "recycle" | "thumbs" | "chrome" | "edge" | "brave" | "firefox";
  label: string;
  bytes: number;
}

export interface RocketHung {
  pid: number;
  name: string;
  secs: number;
}

export interface AutoNote {
  at: string;
  beforePct: number;
  afterPct: number;
  reason: "memoria" | "sin usar" | "mañana";
}

export interface CallStep {
  ok: boolean;
  text: string;
}

/** Hacia dónde creció la ventanita del escritorio al desplegarse. */
export interface Grow {
  left: boolean;
  up: boolean;
}

/** Aviso del programa cuando cambia el modo del cohete (desde su menú o desde Ajustes). */
export const ROCKET_MODE_EVENT = "rocket://mode";
/** Ctrl+K, Ctrl+Alt+O o el menú del reloj: el cohete que esté a la vista despega. */
export const ROCKET_LAUNCH_EVENT = "rocket://launch";
export const ROCKET_DOCK_EVENT = "rocket://dock";
export const ROCKET_HUNG_EVENT = "rocket://hung";
export const ROCKET_AUTO_EVENT = "rocket://auto";

export const rocketApi = {
  boost: () => invoke<RocketBoost>("rocket_boost"),
  /** Tamaño de la ventanita del escritorio (px lógicos); el de la burbuja la pliega. */
  size: (width: number, height: number) => invoke<Grow>("rocket_size", { width, height }),
  peek: (out: boolean) => invoke<void>("rocket_peek", { out }),
  /** La barra: abierta (con ese tamaño) o escondida en su borde. Devuelve si está en el borde derecho. */
  bar: (open: boolean, width: number, height: number) => invoke<boolean>("rocket_bar", { open, width, height }),
  memory: () => invoke<number>("rocket_memory"),
  deepScan: () => invoke<DeepItem[]>("rocket_deep_scan"),
  deepClean: (ids: string[]) => invoke<number>("rocket_deep_clean", { ids }),
  restartHung: (pid: number, expected: string) => invoke<string>("rocket_restart_hung", { pid, expected }),
  dockState: () => invoke<"left" | "right" | null>("rocket_dock_state"),
  prepareCall: () => invoke<CallStep[]>("rocket_prepare_call"),
  startupEnabled: () => invoke<boolean>("rocket_startup_enabled"),
  setStartup: (enabled: boolean) => invoke<void>("rocket_set_startup", { enabled }),
  setMode: (mode: RocketMode) => invoke<void>("rocket_set_mode", { mode }),
  openMain: () => invoke<void>("rocket_open_main"),
};
