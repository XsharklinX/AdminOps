// Ventanas que quedaron fuera de la pantalla (lostwin.rs).
import { invoke } from "./core";

export interface LostWin {
  hwnd: number;
  title: string;
  /** El programa, con nombre amable («Excel»). */
  app: string;
}

export const lostWinApi = {
  list: () => invoke<LostWin[]>("lost_windows"),
  /** Las lleva al centro del monitor principal; devuelve cuántas movió. */
  bring: (hwnds: number[]) => invoke<number>("bring_windows", { hwnds }),
};
