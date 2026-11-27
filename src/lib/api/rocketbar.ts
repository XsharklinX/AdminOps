// La barra de herramientas del cohete (rocketbar.rs).
import { invoke } from "./core";

export const BAR_TOOLS = ["rec", "clip", "speed", "note", "mon", "fix", "card", "help", "teams", "mail", "app"] as const;
export type BarTool = (typeof BAR_TOOLS)[number];

/** Ctrl+Alt+T: saca o esconde la barra. */
export const BAR_EVENT = "rocket://bar";

export const barApi = {
  /** Los arreglos rápidos activos (los del menú del reloj): [id, nombre]. */
  fixes: () => invoke<[string, string][]>("bar_fixes"),
  openComm: (kind: "teams" | "mail") => invoke<void>("bar_open_comm", { kind }),
  /** Asistencia rápida de Windows. */
  quickAssist: () => invoke<string>("bar_quick_assist"),
  /** La herramienta de recortes; si no está, Win+Mayús+S. */
  screenClip: () => invoke<void>("bar_screen_clip"),
};
