// Pantallas que siguen vivas al cambiar de una a otra: las últimas MAX_ALIVE
// conservan su estado (filtros, selección, scroll); las demás se desmontan y
// al volver se abren de nuevo (con la última lectura de `cachedRead` y su hora).

/** Cuántas pantallas siguen vivas a la vez. */
export const MAX_ALIVE = 3;

/**
 * Pone `page` la primera (la más reciente) y descarta las más antiguas hasta
 * dejar `max`. `keep` (la pantalla de la derecha en la vista dividida) no se
 * descarta. `busy` son páginas adicionales que tampoco se descartan mientras
 * tengan una tarea en marcha.
 */
export function touchAlive<T>(list: T[], page: T, max: number, keep: T | null = null, busy: readonly T[] | null = null): T[] {
  return trimAlive([page, ...list.filter((x) => x !== page)], max, keep, busy);
}

/** Quita desde el final lo que sobre, sin tocar la primera, `keep` ni `busy`. */
export function trimAlive<T>(list: T[], max: number, keep: T | null = null, busy: readonly T[] | null = null): T[] {
  const out = [...list];
  const keepSet = new Set(busy ?? []);
  if (keep !== null) keepSet.add(keep);
  for (let i = out.length - 1; out.length > max && i > 0; i--) {
    if (!keepSet.has(out[i])) out.splice(i, 1);
  }
  return out;
}

/** Páginas que tienen una tarea en marcha: se mantienen vivas aunque sobrepasen el límite. */
const busyCounts = new Map<string, number>();

/** Marca o desmarca que una página tiene una tarea en marcha (contador por si hay varias). */
export function markBusy(page: string, isBusy: boolean) {
  const n = (busyCounts.get(page) ?? 0) + (isBusy ? 1 : -1);
  if (n <= 0) busyCounts.delete(page);
  else busyCounts.set(page, n);
}

/** ¿Tiene la página una tarea en marcha? */
export function isBusy(page: string): boolean {
  return busyCounts.has(page);
}

/** Lista de páginas que actualmente tienen tareas en marcha. */
export function busyPages(): readonly string[] {
  return Array.from(busyCounts.keys());
}
