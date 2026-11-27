// Webs con permiso para mandar avisos a Windows desde el navegador (webnotif.rs).
import { invoke } from "./core";

export interface WebNotif {
  /** «Chrome», «Edge», «Brave» u «Opera». */
  browser: string;
  profile: string;
  /** La clave tal como la guarda el navegador: identifica el aviso al quitarlo. */
  origin: string;
  /** Solo el sitio («sitio.com»). */
  host: string;
  suspicious: boolean;
  trusted: boolean;
}

export const webNotifApi = {
  list: () => invoke<WebNotif[]>("web_notifications"),
  /** Bloquea el aviso de cada una: [navegador, perfil, clave]. Devuelve cuántos cambió. */
  remove: (items: [string, string, string][]) => invoke<number>("web_notifications_remove", { items }),
};
