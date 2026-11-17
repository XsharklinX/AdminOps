// Adelantarse al clic: al pasar el ratón por una pantalla de la barra lateral se
// empieza a cargar su código (y, en las de consulta, su primera lectura), para
// que al pulsar ya esté listo.
//
// Con calma: el ratón tiene que quedarse ~120 ms sobre la entrada (pasar de
// largo no carga nada), solo hay un turno pendiente a la vez, el código de una
// pantalla se pide una sola vez y su lectura no se repite antes de 2 minutos ni
// mientras otra lectura adelantada sigue en curso. Nunca en bucle.
import { cachedValue, remember } from "./cachedRead";
import type { PageId } from "../components/Sidebar";

let loader: ((p: PageId) => void) | null = null;
const done = new Set<PageId>();
const readers = new Map<PageId, { key: string; read: () => Promise<unknown> }>();
const lastRead = new Map<PageId, number>();
let readBusy = false;
let timer: number | null = null;

/** Lo que se espera con el ratón encima antes de adelantar nada. */
export const HOVER_DELAY_MS = 120;
/** Una lectura adelantada no se repite antes de esto. */
export const READ_EVERY_MS = 2 * 60_000;

/** La aplicación dice cómo se carga cada pantalla. */
export function setPrefetcher(f: (p: PageId) => void) {
  loader = f;
}

/** Pantalla de solo consulta: su primera lectura se adelanta y queda en la memoria de `cachedRead`. */
export function setPrefetchRead(p: PageId, key: string, read: () => Promise<unknown>) {
  readers.set(p, { key, read });
}

/** ¿Toca adelantar la lectura? (no hay una reciente, ni en memoria ni adelantada hace poco). */
export function readDue(now: number, lastPrefetch: number | undefined, cachedAt: number | undefined): boolean {
  const newest = Math.max(lastPrefetch ?? 0, cachedAt ?? 0);
  return now - newest >= READ_EVERY_MS;
}

function run(p: PageId) {
  if (document.hidden) return;
  if (!done.has(p) && loader) {
    done.add(p);
    loader(p);
  }
  const r = readers.get(p);
  if (!r || readBusy) return;
  const now = Date.now();
  if (!readDue(now, lastRead.get(p), cachedValue(r.key)?.at)) return;
  readBusy = true;
  lastRead.set(p, now);
  r.read()
    .then((data) => remember(r.key, data))
    .catch(() => undefined)
    .finally(() => (readBusy = false));
}

/** El ratón entró en una pantalla de la barra lateral. */
export function prefetchPage(p: PageId) {
  cancelPrefetch();
  timer = window.setTimeout(() => {
    timer = null;
    run(p);
  }, HOVER_DELAY_MS);
}

/** El ratón salió antes del retardo: no se adelanta nada. */
export function cancelPrefetch() {
  if (timer !== null) window.clearTimeout(timer);
  timer = null;
}
