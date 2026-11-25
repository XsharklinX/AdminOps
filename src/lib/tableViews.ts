// Lo que una vista guardada recuerda de una tabla: columnas que se ven, su orden y por cuál
// se ordena. Las tablas (DataTable con `resizable`) se registran aquí y SavedViews les pide
// su estado al guardar y se lo devuelve al abrir la vista. Las vistas antiguas no traen nada
// de esto: son campos opcionales y, sin ellos, la tabla se queda como esté.
import { fullOrder, visibleIds, type ColInfo, type ColSaved } from "./colset";

export interface TableSort {
  id: string;
  desc: boolean;
}

export interface TableLayout {
  cols?: ColSaved;
  /** null: sin ordenar; ausente: no se toca. */
  sort?: TableSort | null;
}

/** El estado actual de una tabla, completo (todas las columnas conocidas, en orden, y las que se ven). */
export function layoutOf(infos: ColInfo[], saved: ColSaved | null, sort: TableSort | null): TableLayout {
  return { cols: { order: fullOrder(infos, saved), shown: visibleIds(infos, saved) }, sort: sort ? { id: sort.id, desc: sort.desc } : null };
}

/** Lo guardado, sin las columnas que ya no existen y sin lo que no tiene la forma esperada. */
export function sanitizeLayout(raw: unknown, knownIds: string[]): TableLayout {
  const out: TableLayout = {};
  if (!raw || typeof raw !== "object") return out;
  const r = raw as { cols?: { order?: unknown; shown?: unknown }; sort?: unknown };
  const known = new Set(knownIds);
  const ids = (v: unknown) => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string" && known.has(x)) : null);
  if (r.cols && typeof r.cols === "object") {
    const order = ids(r.cols.order);
    const shown = ids(r.cols.shown);
    if (order && shown) out.cols = { order, shown };
  }
  if (r.sort === null) out.sort = null;
  else if (r.sort && typeof r.sort === "object") {
    const s = r.sort as { id?: unknown; desc?: unknown };
    if (typeof s.id === "string" && known.has(s.id)) out.sort = { id: s.id, desc: s.desc === true };
  }
  return out;
}

export interface TableHandle {
  get: () => TableLayout;
  apply: (layout: TableLayout) => void;
}

const tables = new Map<string, TableHandle>();

/** Una tabla con nombre se ofrece a las vistas guardadas. Devuelve cómo dejar de ofrecerla. */
export function registerTable(name: string, handle: TableHandle): () => void {
  tables.set(name, handle);
  return () => {
    if (tables.get(name) === handle) tables.delete(name);
  };
}

/** Cómo está ahora la tabla `name` (nada si no está a la vista). */
export function captureLayout(name: string | undefined): TableLayout | undefined {
  return name ? tables.get(name)?.get() : undefined;
}

/** Pone la tabla `name` como dice la vista. Sin columnas guardadas (vista antigua) no cambia nada. */
export function applyLayout(name: string | undefined, layout: TableLayout | undefined) {
  if (!name || !layout) return;
  tables.get(name)?.apply(layout);
}
