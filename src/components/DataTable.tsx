// La tabla de AdminOps: misma cabecera, misma densidad y ordenar por columna en
// todas las pantallas. Antes cada página pintaba la suya, cada una a su manera.
import { ArrowDown, ArrowUp } from "lucide-react";
import { Fragment, useDeferredValue, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

/** Filas que se pintan de golpe; el resto, a medida que se desplaza (listas de miles). */
export const FIRST_ROWS = 200;
const MORE_ROWS = 300;

/** Mueve el foco a la fila de arriba o de abajo (flechas) dentro de la misma tabla. */
function moveFocus(e: KeyboardEvent<HTMLTableRowElement>, dir: 1 | -1) {
  let el: Element | null = e.currentTarget;
  do el = dir === 1 ? el.nextElementSibling : el.previousElementSibling;
  while (el && !(el as HTMLElement).hasAttribute("tabindex"));
  if (el) {
    e.preventDefault();
    (el as HTMLElement).focus();
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

  const toggle = (c: Column<T>) =>
    setSort((cur) => {
      if (cur?.id === c.id) return { id: c.id, desc: !cur.desc };
      // Los números empiezan por el mayor; los textos, por la A.
      const sample = rows.map((r) => c.sortBy?.(r)).find((v) => v !== null && v !== undefined && v !== "");
      return { id: c.id, desc: typeof sample === "number" };
    });

  const pad = padded ? "px-3" : "pr-3 last:pr-0";
  return (
    <table className={`w-full ${size === "xs" ? "text-xs" : "text-[13px]"}`}>
      <thead className={sticky ? "sticky top-0 z-10 bg-panel" : undefined}>
        <tr className="text-[11px] text-mute">
          {columns.map((c) => {
            const on = sort?.id === c.id;
            return (
              <th
                key={c.id}
                onClick={c.sortBy ? () => toggle(c) : undefined}
                aria-sort={on ? (sort.desc ? "descending" : "ascending") : undefined}
                className={`${pad} ${padded ? "py-2" : "pb-2"} font-medium ${ALIGN[c.align ?? "left"]} ${c.sortBy ? "cursor-pointer select-none hover:text-ink" : ""} ${c.headClass ?? ""}`}
              >
                <span className={`inline-flex items-center gap-1 ${on ? "text-neon" : ""}`}>
                  {c.header}
                  {on && (sort.desc ? <ArrowDown size={10} /> : <ArrowUp size={10} />)}
                </span>
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
                onKeyDown={
                  onRowClick
                    ? (e) => {
                        if (e.target !== e.currentTarget) return;
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          onRowClick(row);
                        } else if (e.key === "ArrowDown") moveFocus(e, 1);
                        else if (e.key === "ArrowUp") moveFocus(e, -1);
                      }
                    : undefined
                }
                className={`border-t border-line/60 ${alignTop ? "align-top" : ""} ${onRowClick ? "cursor-pointer hover:bg-panel-2/60" : ""} ${rowClass?.(row) ?? ""}`}
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
  );
}
