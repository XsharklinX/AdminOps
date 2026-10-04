// En qué sección (pestaña) está cada pantalla, para que la barra lateral y la
// ruta de arriba lo marquen, y la orden de cambiar a una sección desde fuera.
//
// Las pestañas guardan su estado dentro de cada página; aquí solo se anota cuál
// está a la vista. Abrir una sección que ya era el «foco» de la página no cambia
// ese foco (React no ve cambio), por eso hay además un aviso explícito.
import { createContext, useContext, useSyncExternalStore } from "react";
import type { PageId } from "../components/Sidebar";

let current: Partial<Record<PageId, string>> = {};
let subs: (() => void)[] = [];

const subscribe = (cb: () => void) => {
  subs.push(cb);
  return () => {
    subs = subs.filter((s) => s !== cb);
  };
};

/** Una página dice qué sección enseña. */
export function reportSection(page: PageId, section: string) {
  if (current[page] === section) return;
  current = { ...current, [page]: section };
  subs.forEach((s) => s());
}

/** La sección a la vista de cada página (las que tienen secciones). */
export const useCurrentSections = () => useSyncExternalStore(subscribe, () => current);

const OPEN_EVENT = "adminops:open-section";

/** Pide a una página que cambie a una de sus secciones. */
export function requestSection(page: PageId, section: string) {
  window.dispatchEvent(new CustomEvent(OPEN_EVENT, { detail: { page, section } }));
}

/** Escucha las peticiones de cambio de sección para una página. */
export function onSectionRequest(page: PageId, cb: (section: string) => void): () => void {
  const h = (e: Event) => {
    const d = (e as CustomEvent<{ page: PageId; section: string }>).detail;
    if (d?.page === page) cb(d.section);
  };
  window.addEventListener(OPEN_EVENT, h);
  return () => window.removeEventListener(OPEN_EVENT, h);
}

/** Qué página es esta (la pone App alrededor de cada una). */
export const PageIdContext = createContext<PageId | null>(null);
export const usePageId = () => useContext(PageIdContext);
