// Qué columnas se ven en una tabla y en qué orden (clic derecho en la cabecera,
// arrastrar). Se guarda por tabla; lo que no se ha tocado sigue los valores de
// fábrica de cada columna (`hidden`), así una columna nueva en una versión
// posterior aparece sola.

export interface ColSaved {
  /** Todas las columnas conocidas al guardar, en su orden. */
  order: string[];
  /** Las que se veían. */
  shown: string[];
}

export interface ColInfo {
  id: string;
  /** Oculta de fábrica. */
  hidden?: boolean;
  /** No se puede ocultar (la primera, o las que no tienen nombre). */
  locked?: boolean;
}

/** Todas las columnas, en el orden elegido; las que no estaban guardadas, al final. */
export function fullOrder(cols: ColInfo[], saved: ColSaved | null): string[] {
  const ids = cols.map((c) => c.id);
  if (!saved) return ids;
  const known = saved.order.filter((id) => ids.includes(id));
  return [...known, ...ids.filter((id) => !known.includes(id))];
}

/** Las columnas que se ven, en orden. */
export function visibleIds(cols: ColInfo[], saved: ColSaved | null): string[] {
  const order = fullOrder(cols, saved);
  const byId = new Map(cols.map((c) => [c.id, c]));
  const out = order.filter((id) => {
    const c = byId.get(id)!;
    if (c.locked) return true;
    if (saved && saved.order.includes(id)) return saved.shown.includes(id);
    return !c.hidden;
  });
  // Nunca una tabla sin columnas.
  return out.length ? out : order.slice(0, 1);
}

/** Lo que hay que guardar tras marcar o desmarcar una columna. */
export function toggled(cols: ColInfo[], saved: ColSaved | null, id: string): ColSaved {
  const order = fullOrder(cols, saved);
  const shown = new Set(visibleIds(cols, saved));
  const c = cols.find((x) => x.id === id);
  if (!c || c.locked) return { order, shown: [...shown] };
  if (shown.has(id)) shown.delete(id);
  else shown.add(id);
  return { order, shown: order.filter((x) => shown.has(x)) };
}

/** Lo que hay que guardar tras soltar la columna `id` junto a `over` (antes, o después si `after`). */
export function moved(cols: ColInfo[], saved: ColSaved | null, id: string, over: string, after: boolean): ColSaved {
  const order = fullOrder(cols, saved);
  const shown = visibleIds(cols, saved);
  if (id === over || !order.includes(id) || !order.includes(over)) return { order, shown };
  const rest = order.filter((x) => x !== id);
  const at = rest.indexOf(over) + (after ? 1 : 0);
  rest.splice(at, 0, id);
  return { order: rest, shown };
}
