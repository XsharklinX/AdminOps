// Estado de cada vista web (portales): dirección, título, carga, historial,
// errores y descargas. Se escucha una sola vez para toda la app, así una vista
// precargada o que se vuelve a mostrar conserva su barra sin parpadeos.
//
// También distingue los problemas de la red de los de AdminOps: sabe si el
// equipo está sin conexión (y recarga solo lo que falló cuando vuelve) y marca
// una página como lenta si tarda demasiado en cargar.
import { listen } from "@tauri-apps/api/event";
import { useSyncExternalStore } from "react";
import { portalsApi } from "./api";

export interface PortalDownload {
  /** Número propio de la descarga (no es su posición en la lista). */
  download: number;
  name: string;
  state: "running" | "done" | "failed";
}

export interface PortalView {
  url: string | null;
  title: string;
  loading: boolean;
  /** Lleva más de SLOW_MS cargando (la conexión o el servidor van lentos). */
  slow: boolean;
  canBack: boolean;
  canForward: boolean;
  /** Por qué no cargó la última página (null si cargó bien). */
  error: string | null;
  /** Sitio que el portal quiso abrir y se mandó al navegador de fuera (null: ninguno). */
  blocked: string | null;
  downloads: PortalDownload[];
}

/** A partir de cuánto una carga se considera lenta. */
export const SLOW_MS = 10_000;

const EMPTY: PortalView = { url: null, title: "", loading: false, slow: false, canBack: false, canForward: false, error: null, blocked: null, downloads: [] };
const views = new Map<string, PortalView>();
const slowTimers = new Map<string, ReturnType<typeof setTimeout>>();
let subs: (() => void)[] = [];

const get = (id: string) => views.get(id) ?? EMPTY;
/**
 * Cambia el estado de una vista y avisa a quien lo esté mirando. Si el valor es
 * el mismo de antes no se avisa: webs como Outlook repiten el título y el estado
 * del historial constantemente, y cada aviso repintaba la página entera.
 */
function patch(id: string, p: Partial<PortalView> | ((v: PortalView) => Partial<PortalView>)) {
  const cur = get(id);
  const next = { ...cur, ...(typeof p === "function" ? p(cur) : p) };
  if (same(cur, next)) return;
  views.set(id, next);
  subs.forEach((s) => s());
}

/** ¿Son iguales dos estados? (las descargas, por referencia: solo cambian al llegar una). */
const same = (a: PortalView, b: PortalView) => (Object.keys(b) as (keyof PortalView)[]).every((k) => a[k] === b[k]);

const subscribe = (cb: () => void) => {
  subs.push(cb);
  return () => {
    subs = subs.filter((s) => s !== cb);
  };
};

export const portalView = get;

/**
 * Quita de la vista un portal al salir de su página. Normalmente basta con
 * ocultarlo (se conserva su sesión y vuelve al instante). Pero si nunca llegó a
 * arrancar, ocultarlo no sirve: su ventana nativa se queda invisible encima de
 * la interfaz tragándose los clics. Esa se destruye.
 */
export function stowPortal(id: string) {
  if (get(id).url === null) void portalsApi.reset(id).catch(() => {});
  else void portalsApi.hide(id).catch(() => {});
}

/** Clave donde se recuerda el último portal usado de cada tipo (también para precargarlo). */
export const lastPortalKey = (kind: "" | "inventory" | "mail" | "teams") => (kind ? `adminops.lastPortal.${kind}` : "adminops.lastPortal");
export const clearPortalError = (id: string) => patch(id, { error: null });
export const clearPortalBlocked = (id: string) => patch(id, { blocked: null });
/** Marca un portal como fallido desde la interfaz (p. ej. la vista no llegó a crearse). */
export const failPortal = (id: string, message: string) => patch(id, { error: message, loading: false, slow: false });

/** Mensajes nuevos según el título de la página («(3) Correo…»); null si no lo dice. */
export function unreadFromTitle(title: string): number | null {
  const m = /^\((\d+)\)/.exec(title.trim());
  return m ? Number(m[1]) : null;
}

/** Vistas que hay que recargar al volver la conexión: las que se quedaron con error. */
export function toRetry(all: Iterable<[string, PortalView]>): string[] {
  return Array.from(all)
    .filter(([, v]) => v.error !== null)
    .map(([id]) => id);
}

export function usePortalView(id: string | null): PortalView {
  return useSyncExternalStore(subscribe, () => (id ? get(id) : EMPTY));
}

// ---------- Conexión del equipo ----------

let online = typeof navigator === "undefined" ? true : navigator.onLine;

/** ¿Tiene el equipo conexión de red? (sin red, ningún portal puede cargar). */
export function useOnline(): boolean {
  return useSyncExternalStore(subscribe, () => online);
}

function setLoading(id: string, loading: boolean) {
  clearTimeout(slowTimers.get(id));
  slowTimers.delete(id);
  if (loading) slowTimers.set(id, setTimeout(() => get(id).loading && patch(id, { slow: true }), SLOW_MS));
}

let started = false;
/** Empieza a escuchar a las vistas web (una vez, al abrir la app). */
export function watchPortals() {
  if (started) return;
  started = true;
  void listen<{ id: string; url: string; loading: boolean }>("portal-load", ({ payload: p }) => {
    setLoading(p.id, p.loading);
    patch(p.id, { url: p.url, loading: p.loading, ...(p.loading ? {} : { slow: false }) });
  });
  void listen<{ id: string; title: string }>("portal-title", ({ payload: p }) => patch(p.id, { title: p.title }));
  void listen<{ id: string; canBack: boolean; canForward: boolean }>("portal-state", ({ payload: p }) => patch(p.id, { canBack: p.canBack, canForward: p.canForward }));
  void listen<{ id: string; message: string }>("portal-error", ({ payload: p }) => {
    setLoading(p.id, false);
    patch(p.id, { error: p.message || null, loading: false, slow: false });
  });
  void listen<{ id: string; host: string }>("portal-blocked", ({ payload: p }) => patch(p.id, { blocked: p.host }));
  void listen<{ id: string } & PortalDownload>("portal-download", ({ payload: p }) =>
    patch(p.id, (v) => {
      const d = { download: p.download, name: p.name, state: p.state };
      const rest = v.downloads.filter((x) => x.download !== p.download);
      return { downloads: [d, ...rest].slice(0, 20) };
    }),
  );
  window.addEventListener("offline", () => {
    online = false;
    subs.forEach((s) => s());
  });
  // Al volver la conexión, lo que falló por la red se recarga solo.
  window.addEventListener("online", () => {
    online = true;
    for (const id of toRetry(views)) {
      clearPortalError(id);
      portalsApi.nav(id, "reload").catch(() => {});
    }
    subs.forEach((s) => s());
  });
}
