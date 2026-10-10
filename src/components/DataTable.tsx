// La tabla de AdminOps: misma cabecera, misma densidad y ordenar por columna en
// todas las pantallas. Antes cada página pintaba la suya, cada una a su manera.
import { ArrowDown, ArrowUp } from "lucide-react";
import { Fragment, useDeferredValue, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { SlideMark } from "./SlideMark";

/** Filas que se pintan de golpe; el resto, a medida que se desplaza (listas de miles). */
export const FIRST_ROWS = 200;
const MORE_ROWS = 300;

/** Mueve el foco a la fila de arriba o de abajo (flechas, J y K) dentro de la misma tabla. Con `follow`, la fila nueva se abre como si se pulsara. */
function moveFocus(e: KeyboardEvent<HTMLTableRowElement>, dir: 1 | -1, follow = false) {
  let el: Element | null = e.currentTarget;
  do el = dir === 1 ? el.nextElementSibling : el.previousElementSibling;
  while (el && !(el as HTMLElement).hasAttribute("tabindex"));
  if (el) {
    e.preventDefault();
    (el as HTMLElement).focus();
    if (follow) (el as HTMLElement).click();
  }
}

const COLS_KEY = "adminops.cols.";
const MIN_COL = 56;

function loadWidths(key: string | undefined): Record<string, number> {
  if (!key) return {};
  try {
    const raw = JSON.parse(localStorage.getItem(COLS_KEY + key) ?? "{}");
    return raw && typeof raw === "object" ? raw : {};
  } catch {
    return {};
  }
}

export interface Column<T> {
  id: string;
  header: ReactNode;
  align?: "left" | "right" | "center";
  /** Valor por el que se ordena. Sin él, la columna no se puede ordenar. */
  sortBy?: (row: T) => string | number | null | undefined;
  cell: (row: T) => ReactNode;
  /** Clases de la celda (color, fuente); puede depender de la fila. */
  className?: string | ((row: T) => string);
  /** Clases de la cabecera (p. ej. un ancho fijo). */
  headClass?: string;
  /** El clic en esta celda no cuenta como clic en la fila (lleva botones o un campo). */
  stopClick?: boolean;
}

export interface SortState {
  id: string;
  desc: boolean;
}

type SortValue = string | number | null | undefined;

/** Compara dos valores de una columna. Lo vacío va siempre al final. */
export function compareValues(a: SortValue, b: SortValue, desc: boolean): number {
  const emptyA = a === null || a === undefined || a === "";
  const emptyB = b === null || b === undefined || b === "";
  if (emptyA || emptyB) return emptyA === emptyB ? 0 : emptyA ? 1 : -1;
  const r = typeof a === "number" && typeof b === "number" ? a - b : String(a).localeCompare(String(b), "es", { numeric: true, sensitivity: "base" });
  return desc ? -r : r;
}

/** Filas ordenadas por una columna (sin tocar el original; orden estable). */
export function sortRows<T>(rows: T[], by: ((row: T) => SortValue) | undefined, desc: boolean): T[] {
  if (!by) return rows;
  return rows
    .map((row, i) => ({ row, i, v: by(row) }))
    .sort((x, y) => compareValues(x.v, y.v, desc) || x.i - y.i)
    .map((x) => x.row);
}

const ALIGN = { left: "text-left", right: "text-right", center: "text-center" };

export function DataTable<T>({
  columns,
  rows,
  rowKey,
  initialSort,
  onRowClick,
  rowClass,
  expanded,
  empty,
  padded = false,
  sticky = false,
  mono = false,
  size = "sm",
  alignTop = false,
  followFocus = false,
  selected,
  resizable,
}: {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T, index: number) => string | number;
  /** Orden con el que se abre. Sin él, las filas salen como llegan. */
  initialSort?: SortState;
  onRowClick?: (row: T) => void;
  rowClass?: (row: T) => string;
  /** Detalle que se abre bajo la fila (null: cerrado). */
  expanded?: (row: T) => ReactNode | null;
  /** Lo que se enseña cuando no hay filas. */
  empty?: ReactNode;
  /** Con margen a los lados: para tablas que llenan su propio recuadro. */
  padded?: boolean;
  /** La cabecera se queda arriba al desplazar (el contenedor tiene que hacer scroll). */
  sticky?: boolean;
  /** Cifras alineadas (IP, tiempos, tamaños). */
  mono?: boolean;
  size?: "xs" | "sm";
  alignTop?: boolean;
  /** Con las flechas (o J y K) la fila nueva se abre sola: para listas con un panel de detalle al lado. */
  followFocus?: boolean;
  /** La fila que tiene su detalle abierto. */
  selected?: (row: T) => boolean;
  /** Nombre con el que se recuerda el ancho de las columnas que se ensanchan arrastrando. Sin él, no se pueden ensanchar. */
  resizable?: string;
}) {
  const [sort, setSort] = useState<SortState | null>(initialSort ?? null);
  // Mientras se escribe en un filtro, la tabla se pone al día un instante después:
  // el campo de texto responde a cada tecla aunque haya miles de filas.
  const deferred = useDeferredValue(rows);
  const [limit, setLimit] = useState(FIRST_ROWS);
  const sentinel = useRef<HTMLTableRowElement>(null);
  const sorted = useMemo(() => {
    const col = sort ? columns.find((c) => c.id === sort.id) : undefined;
    return sort && col?.sortBy ? sortRows(deferred, col.sortBy, sort.desc) : deferred;
    // Las columnas se crean en cada render de la página; lo que importa es por cuál se ordena.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [deferred, sort?.id, sort?.desc]);
  // Con otras filas (otro filtro), se vuelve a empezar por las primeras.
  useEffect(() => setLimit(FIRST_ROWS), [deferred]);
  // Al llegar al final de lo pintado, se pintan más.
  useEffect(() => {
    const el = sentinel.current;
    if (!el || sorted.length <= limit) return;
    const io = new IntersectionObserver((entries) => entries.some((x) => x.isIntersecting) && setLimit((l) => l + MORE_ROWS), { rootMargin: "600px" });
    io.observe(el);
    return () => io.disconnect();
  }, [sorted.length, limit]);
  const visible = sorted.length > limit ? sorted.slice(0, limit) : sorted;

  // Filas que acaban de llegar (un escaneo que encuentra otro equipo): bajan y se resaltan un instante.
  const known = useRef<Set<string | number> | null>(null);
  const keys = visible.map((r, i) => rowKey(r, i));
  const fresh = new Set<string | number>();
  if (known.current && known.current.size > 0) for (const k of keys) if (!known.current.has(k)) fresh.add(k);
  if (fresh.size > 8) fresh.clear();
  useEffect(() => {
    known.current = new Set(keys);
  });

  // Fila con el foco del teclado: el resaltado se desliza hasta ella.
  const wrap = useRef<HTMLDivElement>(null);
  const [focusKey, setFocusKey] = useState<string | number | null>(null);

  // Ancho de las columnas, si se pueden ensanchar.
  const [widths, setWidths] = useState(() => loadWidths(resizable));
  const startResize = (e: React.PointerEvent, id: string) => {
    e.preventDefault();
    e.stopPropagation();
    const th = (e.currentTarget as HTMLElement).closest("th");
    const from = th?.getBoundingClientRect().width ?? 120;
    const x0 = e.clientX;
    let last = from;
    const move = (ev: PointerEvent) => {
      last = Math.max(MIN_COL, Math.round(from + ev.clientX - x0));
      setWidths((w) => ({ ...w, [id]: last }));
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      try {
        localStorage.setItem(COLS_KEY + resizable, JSON.stringify({ ...loadWidths(resizable), [id]: last }));
      } catch {
        /* dura esta sesión */
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
  };

  const toggle = (c: Column<T>) =>
    setSort((cur) => {
      if (cur?.id === c.id) return { id: c.id, desc: !cur.desc };
      // Los números empiezan por el mayor; los textos, por la A.
      const sample = rows.map((r) => c.sortBy?.(r)).find((v) => v !== null && v !== undefined && v !== "");
      return { id: c.id, desc: typeof sample === "number" };
    });

  const pad = padded ? "px-3" : "pr-3 last:pr-0";
  return (
    <div ref={wrap} className="relative isolate">
    {onRowClick && <SlideMark within={wrap} selector={focusKey !== null ? 'tr[data-focused="true"]' : 'tr[data-selected="true"]'} />}
    <table className={`w-full ${size === "xs" ? "text-xs" : "text-[13px]"}`}>
      <thead className={sticky ? "sticky top-0 z-10 bg-panel shadow-[0_1px_0_var(--color-line)]" : undefined}>
        <tr className="text-[11px] text-mute">
          {columns.map((c) => {
            const on = sort?.id === c.id;
            return (
              <th
                key={c.id}
                onClick={c.sortBy ? () => toggle(c) : undefined}
                aria-sort={on ? (sort.desc ? "descending" : "ascending") : undefined}
                style={resizable && widths[c.id] ? { width: widths[c.id], minWidth: widths[c.id], maxWidth: widths[c.id] } : undefined}
                className={`${resizable ? "relative" : ""} ${pad} ${padded ? "py-2" : "pb-2"} font-medium ${ALIGN[c.align ?? "left"]} ${c.sortBy ? "cursor-pointer select-none hover:text-ink" : ""} ${c.headClass ?? ""}`}
              >
                <span className={`inline-flex items-center gap-1 ${on ? "text-neon" : ""}`}>
                  {c.header}
                  {on && (sort.desc ? <ArrowDown size={10} /> : <ArrowUp size={10} />)}
                </span>
                {resizable && (
                  <span
                    role="separator"
                    aria-orientation="vertical"
                    aria-label={`Ancho de la columna ${c.id}`}
                    onPointerDown={(e) => startResize(e, c.id)}
                    onClick={(e) => e.stopPropagation()}
                    onDoubleClick={() => {
                      setWidths((w) => {
                        const { [c.id]: _gone, ...rest } = w;
                        void _gone;
                        try {
                          localStorage.setItem(COLS_KEY + resizable, JSON.stringify(rest));
                        } catch {
                          /* sin almacenamiento */
                        }
                        return rest;
                      });
                    }}
                    title="Arrastra para ensanchar · doble clic para el ancho de siempre"
                    className="absolute top-0 -right-1 bottom-0 z-20 w-2 cursor-col-resize hover:bg-neon/30"
                  />
                )}
              </th>
            );
          })}
        </tr>
      </thead>
      <tbody className={mono ? "font-mono tabular" : undefined}>
        {visible.map((row, i) => {
          const detail = expanded?.(row);
          return (
            <Fragment key={rowKey(row, i)}>
              <tr
                onClick={onRowClick ? () => onRowClick(row) : undefined}
                tabIndex={onRowClick ? 0 : undefined}
                data-focused={focusKey === rowKey(row, i) ? "true" : undefined}
                data-selected={selected?.(row) ? "true" : undefined}
                onFocus={onRowClick ? () => setFocusKey(rowKey(row, i)) : undefined}
                onBlur={
                  onRowClick
                    ? (e) => {
                        if (!e.currentTarget.parentElement?.contains(e.relatedTarget as Node | null)) setFocusKey(null);
                      }
                    : undefined
                }
                onKeyDown={
                  onRowClick
                    ? (e) => {
                        if (e.target !== e.currentTarget) return;
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          onRowClick(row);
                        } else if (e.key === "ArrowDown" || e.key === "j") moveFocus(e, 1, followFocus);
                        else if (e.key === "ArrowUp" || e.key === "k") moveFocus(e, -1, followFocus);
                      }
                    : undefined
                }
                className={`border-t border-line/60 ${alignTop ? "align-top" : ""} ${onRowClick ? "cursor-pointer hover:bg-panel-2/60" : ""} ${fresh.has(rowKey(row, i)) ? "row-in" : ""} ${rowClass?.(row) ?? ""}`}
              >
                {columns.map((c) => (
                  <td
                    key={c.id}
                    onClick={c.stopClick ? (e) => e.stopPropagation() : undefined}
                    className={`${pad} py-1.5 ${ALIGN[c.align ?? "left"]} ${typeof c.className === "function" ? c.className(row) : (c.className ?? "")}`}
                  >
                    {c.cell(row)}
                  </td>
                ))}
              </tr>
              {detail && (
                <tr>
                  <td colSpan={columns.length} className="bg-void/40 px-3 py-2">
                    {detail}
                  </td>
                </tr>
              )}
            </Fragment>
          );
        })}
        {sorted.length > limit && (
          <tr ref={sentinel}>
            <td colSpan={columns.length} className="px-3 py-3 text-center font-sans text-xs text-mute">
              Mostrando {limit.toLocaleString("es-ES")} de {sorted.length.toLocaleString("es-ES")}…{" "}
              <button onClick={() => setLimit(sorted.length)} className="text-neon hover:underline">
                Ver todas
              </button>
            </td>
          </tr>
        )}
        {sorted.length === 0 && empty && (
          <tr>
            <td colSpan={columns.length} className="px-3 py-6 text-center font-sans text-sm text-mute">
              {empty}
            </td>
          </tr>
        )}
      </tbody>
    </table>
    </div>
  );
}
