// El hueco de la cabecera donde la pantalla a la vista pone sus pestañas: así
// el título, el aviso y las pestañas van en una sola línea (1.4, idea 4).
//
// App crea un hueco nuevo por pantalla (con `key`), y cada pantalla pinta sus
// pestañas ahí solo si el hueco es el suyo. Las que están vivas pero no a la
// vista (o a la derecha de la vista dividida) no tienen hueco y las pintan dentro.
import { useSyncExternalStore } from "react";
import type { PageId } from "../components/Sidebar";

let slot: { page: PageId; el: HTMLElement } | null = null;
let subs: (() => void)[] = [];

const subscribe = (cb: () => void) => {
  subs.push(cb);
  return () => {
    subs = subs.filter((s) => s !== cb);
  };
};

/** App: el hueco de la pantalla `page` (null al desmontarse). */
export function setTabsSlot(page: PageId, el: HTMLElement | null) {
  if (el) slot = { page, el };
  else if (slot?.page === page) slot = null;
  else return;
  subs.forEach((s) => s());
}

/** El hueco de esta pantalla, si está en la cabecera ahora. */
export function useTabsSlot(page: PageId | null): HTMLElement | null {
  return useSyncExternalStore(subscribe, () => (page && slot?.page === page ? slot.el : null));
}
