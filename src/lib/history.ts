// Historial de pantallas (Alt+←/→, botones laterales del ratón, flecha de la
// cabecera). Cada entrada guarda la pantalla, su scroll y lo que recuerda
// (filtros, orden, pestaña: ver `pageMemory`).
import { memLabel, memSnapshot } from "./pageMemory";

/** Entradas que se guardan como máximo. */
export const MAX_HISTORY = 50;

export interface HistEntry<P extends string = string> {
  page: P;
  scroll: number;
  mem: Record<string, unknown>;
}

/** La pantalla tal como está ahora, para poder volver a ella. */
export function snapshotOf<P extends string>(page: P): HistEntry<P> {
  const el = typeof document === "undefined" ? null : document.querySelector<HTMLElement>(`[data-page="${page}"]`);
  return { page, scroll: el?.scrollTop ?? 0, mem: memSnapshot(page) };
}

/** Añade una entrada al final y se queda con las últimas `MAX_HISTORY`. */
export function pushHistory<E>(list: E[], entry: E, max = MAX_HISTORY): E[] {
  const next = [...list, entry];
  return next.length > max ? next.slice(next.length - max) : next;
}

export interface Step<E> {
  /** A donde se va. */
  target: E;
  /** Lo que queda en el lado de donde se sale. */
  from: E[];
  /** El otro lado, con la pantalla de ahora y las que se saltaron. */
  to: E[];
}

/**
 * Da un paso en el historial: la entrada más reciente de `from` que `ok` permita (o la que
 * está `skip` entradas más allá). `skip` cuenta solo entradas permitidas: es el lugar en la lista
 * que se enseña, de la que ya se quitó lo que `ok` no permite. Las saltadas y la pantalla actual
 * pasan al otro lado en su orden, para que «adelante» las recorra.
 */
export function stepHistory<E>(from: E[], to: E[], current: E, skip: number, ok: (e: E) => boolean): Step<E> | null {
  let at = from.length - 1;
  let left = skip;
  for (; at >= 0; at--) {
    if (!ok(from[at])) continue;
    if (left === 0) break;
    left--;
  }
  if (at < 0) return null;
  let other = pushHistory(to, current);
  for (let i = from.length - 1; i > at; i--) other = pushHistory(other, from[i]);
  return { target: from[at], from: from.slice(0, at), to: other };
}

/** Cómo se llama una entrada en la flecha de la cabecera: la pantalla y su filtro corto. */
export function entryFilter(e: HistEntry): string | null {
  return memLabel(e.mem);
}
