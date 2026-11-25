// La tabla de AdminOps: misma cabecera, misma densidad y ordenar por columna en
// todas las pantallas. Antes cada página pintaba la suya, cada una a su manera.
import { ArrowDown, ArrowUp, Search, X } from "lucide-react";
import { Fragment, useDeferredValue, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { moved, toggled, visibleIds, type ColInfo, type ColSaved } from "../lib/colset";
import { useFrozenOrder, usePointerHold, useReorderFlip } from "../lib/liveOrder";
import { usePageMemory } from "../lib/pageMemory";
import { useRowWindow } from "../lib/rowWindow";
import { layoutOf, registerTable, sanitizeLayout } from "../lib/tableViews";
import { matchesFilter, nodeText, TYPEFILTER_EVENT, type TypeFilterDetail } from "../lib/typeFilter";
import { SlideMark } from "./SlideMark";

/** Filas que se pintan de golpe en una tabla con detalles desplegables; el resto, a medida que se desplaza. Sin detalles, a partir de aquí la lista es virtual: solo se pintan las filas que se ven. */
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
const COLSET_KEY = "adminops.colset.";
const MIN_COL = 56;
const MAX_FIT = 640;
/** Tablas con al menos tantas filas reciben lo que se escribe sin tener el foco en un campo. */
const TYPE_MIN_ROWS = 8;

function loadColset(key: string | undefined): ColSaved | null {
  if (!key) return null;
  try {
    const raw = JSON.parse(localStorage.getItem(COLSET_KEY + key) ?? "null");
    return raw && Array.isArray(raw.order) && Array.isArray(raw.shown) ? { order: raw.order.map(String), shown: raw.shown.map(String) } : null;
  } catch {
    return null;
  }
}

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
  /** Oculta de fábrica: se ofrece en el menú de columnas (clic derecho en la cabecera). */
  hidden?: boolean;
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
  live = false,
  typeFilter = true,
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
  /** Lista que cambia sola (procesos, puestos): con el puntero encima el orden se queda quieto, aunque las cifras sigan cambiando. */
  live?: boolean;
  /** Con la tabla a la vista, lo que se escribe sin tener el foco en un campo la filtra. */
  typeFilter?: boolean;
}) {
  const uid = useId();
  // El orden se recuerda por pantalla (con «atrás», o al volver a abrirla).
  const [sort, setSort] = usePageMemory<SortState | null>(`sort:${resizable ?? uid}`, initialSort ?? null);
  // Mientras se escribe en un filtro, la tabla se pone al día un instante después:
  // el campo de texto responde a cada tecla aunque haya miles de filas.
  const deferred = useDeferredValue(rows);
  const [limit, setLimit] = useState(FIRST_ROWS);
  const sentinel = useRef<HTMLTableRowElement>(null);
  const body = useRef<HTMLTableSectionElement>(null);
  const ordered = useMemo(() => {
    const col = sort ? columns.find((c) => c.id === sort.id) : undefined;
    return sort && col?.sortBy ? sortRows(deferred, col.sortBy, sort.desc) : deferred;
    // Las columnas se crean en cada render de la página; lo que importa es por cuál se ordena.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [deferred, sort?.id, sort?.desc]);
  // Fila con el foco del teclado: el resaltado se desliza hasta ella.
  const wrap = useRef<HTMLDivElement>(null);
  const [focusKey, setFocusKey] = useState<string | number | null>(null);

  // Columnas: las que se ven y su orden (clic derecho en la cabecera, arrastrar).
  const [saved, setSaved] = useState<ColSaved | null>(() => loadColset(resizable));
  // La primera columna con nombre (la de siempre) y las que no tienen nombre (casillas, botones) no se pueden ocultar.
  const firstNamed = columns.findIndex((c) => nodeText(c.header).trim());
  const infos: ColInfo[] = columns.map((c, i) => ({ id: c.id, hidden: c.hidden, locked: i === 0 || i === firstNamed || !nodeText(c.header).trim() }));
  const shownIds = visibleIds(infos, saved);
  const shownKey = shownIds.join("|");
  const cols = shownIds.map((id) => columns.find((c) => c.id === id)!);
  const colsRef = useRef(cols);
  colsRef.current = cols;
  const saveColset = (next: ColSaved | null) => {
    setSaved(next);
    if (!resizable) return;
    try {
      if (next) localStorage.setItem(COLSET_KEY + resizable, JSON.stringify(next));
      else localStorage.removeItem(COLSET_KEY + resizable);
    } catch {
      /* dura esta sesión */
    }
  };
  // Las vistas guardadas de la pantalla leen y ponen columnas, orden y ordenación (solo tablas con nombre).
  const layoutLive = useRef({ infos, saved, sort, columns, saveColset, setSort });
  layoutLive.current = { infos, saved, sort, columns, saveColset, setSort };
  useEffect(() => {
    if (!resizable) return;
    return registerTable(resizable, {
      get: () => layoutOf(layoutLive.current.infos, layoutLive.current.saved, layoutLive.current.sort),
      apply: (raw) => {
        const live = layoutLive.current;
        const layout = sanitizeLayout(raw, live.columns.map((c) => c.id));
        if (layout.cols) live.saveColset(layout.cols);
        if (layout.sort !== undefined) live.setSort(layout.sort);
      },
    });
  }, [resizable]);
  const [colMenu, setColMenu] = useState<{ x: number; y: number } | null>(null);
  const [dragCol, setDragCol] = useState<string | null>(null);
  const [dropAt, setDropAt] = useState<{ id: string; after: boolean } | null>(null);

  // Filtro rápido: lo que se escribe sin tener el foco en un campo.
  const [tf, setTf] = useState("");
  const matched = useMemo(() => {
    // `shownKey` cambia al mostrar u ocultar una columna: el filtro se vuelve a pasar con las que se ven.
    if (!tf.trim() || !shownKey) return ordered;
    return ordered.filter((row) => matchesFilter(colsRef.current.map((c) => `${nodeText(c.cell(row))} ${c.sortBy?.(row) ?? ""}`).join(" "), tf));
  }, [ordered, tf, shownKey]);
  const matchRef = useRef({ rows: matched, onRowClick });
  matchRef.current = { rows: matched, onRowClick };
  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const on = (ev: Event) => {
      const k = (ev as CustomEvent<TypeFilterDetail>).detail.key;
      if (k === "Backspace") setTf((t) => t.slice(0, -1));
      else if (k === "Escape") setTf("");
      else if (k === "Enter") {
        const m = matchRef.current;
        if (m.rows.length === 1 && m.onRowClick) m.onRowClick(m.rows[0]);
      } else setTf((t) => t + k);
    };
    el.addEventListener(TYPEFILTER_EVENT, on);
    return () => el.removeEventListener(TYPEFILTER_EVENT, on);
  }, []);

  // Lista en vivo: con el puntero dentro, el orden espera.
  const held = usePointerHold(body, live);
  useReorderFlip(body, held);
  const sorted = useFrozenOrder(matched, (r, i) => String(rowKey(r, i)), held);

  // Lista larga y sin detalles que se abren bajo las filas: solo se pintan las filas a la vista.
  const virtual = !expanded && sorted.length > FIRST_ROWS;
  const win = useRowWindow(body, sorted.length, virtual);
  // Con otras filas (otro filtro), se vuelve a empezar por las primeras.
  useEffect(() => setLimit(FIRST_ROWS), [deferred, tf]);
  // Al llegar al final de lo pintado, se pintan más.
  useEffect(() => {
    const el = sentinel.current;
    if (!el || virtual || sorted.length <= limit) return;
    const io = new IntersectionObserver((entries) => entries.some((x) => x.isIntersecting) && setLimit((l) => l + MORE_ROWS), { rootMargin: "600px" });
    io.observe(el);
    return () => io.disconnect();
  }, [sorted.length, limit, virtual]);

  // La fila con el foco se mantiene montada aunque salga del rango visible,
  // así J/K y el resaltado siguen funcionando al desplazar la lista.
  const focusIndex = useMemo(() => {
    if (!virtual || focusKey === null) return -1;
    return sorted.findIndex((row, i) => rowKey(row, i) === focusKey);
  }, [virtual, focusKey, sorted, rowKey]);

  const visible = useMemo(() => {
    if (!virtual) return (sorted.length > limit ? sorted.slice(0, limit) : sorted).map((row, i) => ({ row, index: i }));
    const indices = new Set<number>();
    for (let i = win.start; i < win.end; i++) indices.add(i);
    if (focusIndex >= 0) indices.add(focusIndex);
    return Array.from(indices)
      .sort((a, b) => a - b)
      .map((i) => ({ row: sorted[i], index: i }));
  }, [virtual, sorted, limit, win.start, win.end, focusIndex]);

  const firstIndex = virtual ? visible[0]?.index ?? 0 : 0;
  const lastIndex = virtual ? visible[visible.length - 1]?.index ?? sorted.length - 1 : sorted.length - 1;
  const topSpacer = virtual && firstIndex > 0 ? firstIndex * win.rowPx : 0;
  const bottomSpacer = virtual ? Math.max(0, sorted.length - lastIndex - 1) * win.rowPx : 0;

  // Filas que acaban de llegar (un escaneo que encuentra otro equipo): bajan y se resaltan un instante.
  const known = useRef<Set<string | number> | null>(null);
  const keys = visible.map(({ row, index: i }) => rowKey(row, i));
  const fresh = new Set<string | number>();
  if (known.current && known.current.size > 0) for (const k of keys) if (!known.current.has(k)) fresh.add(k);
  if (fresh.size > 8 || virtual) fresh.clear();
  useEffect(() => {
    known.current = new Set(keys);
  });

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

  /** Doble clic en el borde: el ancho justo para lo más largo de la columna. */
  const fitColumn = (e: React.MouseEvent, id: string) => {
    const th = (e.currentTarget as HTMLElement).closest("th");
    const rowsEl = body.current?.querySelectorAll<HTMLTableRowElement>("tr:not([data-spacer])");
    if (!th) return;
    let w = th.scrollWidth;
    const at = th.cellIndex;
    let n = 0;
    rowsEl?.forEach((tr) => {
      const td = tr.cells[at];
      if (!td || n++ > 120 || tr.cells.length !== cols.length) return;
      const cs = getComputedStyle(td);
      const pad = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
      w = Math.max(w, td.scrollWidth, ...[...td.children].map((ch) => (ch as HTMLElement).scrollWidth + pad));
    });
    const fit = Math.min(MAX_FIT, Math.max(MIN_COL, Math.ceil(w) + 2));
    setWidths((cur) => {
      const next = { ...cur, [id]: fit };
      try {
        localStorage.setItem(COLS_KEY + resizable, JSON.stringify(next));
      } catch {
        /* dura esta sesión */
      }
      return next;
    });
  };

  const resetColumns = () => {
    saveColset(null);
    setWidths({});
    try {
      if (resizable) localStorage.removeItem(COLS_KEY + resizable);
    } catch {
      /* sin almacenamiento */
    }
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
    <div
      ref={wrap}
      className="relative isolate"
      data-typefilter={typeFilter && (ordered.length >= TYPE_MIN_ROWS || tf) ? "" : undefined}
      data-tf-active={tf ? "1" : undefined}
    >
    {(tf || live) && (
      <div className="mb-1.5 flex min-h-6 items-center gap-2 text-xs">
        {tf && (
          <span className="inline-flex max-w-[60%] items-center gap-1.5 rounded-md border border-neon/40 bg-panel px-2 py-0.5 text-ink">
            <Search size={12} className="shrink-0 text-mute" />
            <span className="truncate">{tf}</span>
            <button onClick={() => setTf("")} aria-label="Quitar el filtro" title="Quitar el filtro (Esc)" className="shrink-0 text-mute hover:text-ink">
              <X size={12} />
            </button>
          </span>
        )}
        <span className="ml-auto flex items-center gap-2">
          {tf && (
            <span className="text-mute tabular">
              {sorted.length} de {ordered.length}
            </span>
          )}
          {live && (
            <span aria-hidden={!held} className={`rounded-full border border-neon/40 bg-neon/10 px-2.5 py-0.5 text-[11px] text-neon ${held ? "" : "invisible"}`}>
              Orden en pausa mientras apuntas
            </span>
          )}
        </span>
      </div>
    )}
    {onRowClick && <SlideMark within={wrap} selector={focusKey !== null ? 'tr[data-focused="true"]' : 'tr[data-selected="true"]'} />}
    <table className={`w-full ${size === "xs" ? "text-xs" : "text-[13px]"}`}>
      <thead data-no-ctx className={sticky ? "sticky top-0 z-10 bg-panel shadow-[0_1px_0_var(--color-line)]" : undefined}>
        <tr
          className="text-[11px] text-mute"
          onContextMenu={(e) => {
            e.preventDefault();
            setColMenu({ x: e.clientX, y: e.clientY });
          }}
        >
          {cols.map((c) => {
            const on = sort?.id === c.id;
            const drop = dropAt?.id === c.id && dragCol !== c.id ? (dropAt.after ? "inset -2px 0 0 0 var(--color-neon)" : "inset 2px 0 0 0 var(--color-neon)") : undefined;
            return (
              <th
                key={c.id}
                onClick={c.sortBy ? () => toggle(c) : undefined}
                onDragOver={
                  dragCol
                    ? (e) => {
                        e.preventDefault();
                        const r = e.currentTarget.getBoundingClientRect();
                        const after = e.clientX > r.left + r.width / 2;
                        setDropAt((d) => (d?.id === c.id && d.after === after ? d : { id: c.id, after }));
                      }
                    : undefined
                }
                onDrop={
                  dragCol
                    ? (e) => {
                        e.preventDefault();
                        if (dropAt) saveColset(moved(infos, saved, dragCol, dropAt.id, dropAt.after));
                        setDragCol(null);
                        setDropAt(null);
                      }
                    : undefined
                }
                aria-sort={on ? (sort.desc ? "descending" : "ascending") : undefined}
                style={{ ...(resizable && widths[c.id] ? { width: widths[c.id], minWidth: widths[c.id], maxWidth: widths[c.id] } : null), boxShadow: drop }}
                className={`${resizable ? "relative" : ""} ${pad} ${padded ? "py-2" : "pb-2"} font-medium ${ALIGN[c.align ?? "left"]} ${c.sortBy ? "cursor-pointer select-none hover:text-ink" : ""} ${c.headClass ?? ""}`}
              >
                <span
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData("text/plain", c.id);
                    e.dataTransfer.effectAllowed = "move";
                    setDragCol(c.id);
                  }}
                  onDragEnd={() => {
                    setDragCol(null);
                    setDropAt(null);
                  }}
                  className={`inline-flex items-center gap-1 ${on ? "text-neon" : ""}`}
                >
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
                    onDoubleClick={(e) => fitColumn(e, c.id)}
                    title="Arrastra para ensanchar · doble clic para ajustar al contenido"
                    className="absolute top-0 -right-1 bottom-0 z-20 w-2 cursor-col-resize hover:bg-neon/30"
                  />
                )}
              </th>
            );
          })}
        </tr>
      </thead>
      <tbody ref={body} className={mono ? "font-mono tabular" : undefined}>
        {virtual && topSpacer > 0 && <tr data-spacer="1" aria-hidden style={{ height: topSpacer }} />}
        {visible.map(({ row, index: i }) => {
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
                data-flip={live ? String(rowKey(row, i)) : undefined}
                className={`border-t border-line/60 ${alignTop ? "align-top" : ""} ${onRowClick ? "cursor-pointer hover:bg-panel-2/60" : ""} ${fresh.has(rowKey(row, i)) ? "row-in" : ""} ${rowClass?.(row) ?? ""}`}
              >
                {cols.map((c) => (
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
                  <td colSpan={cols.length} className="bg-void/40 px-3 py-2">
                    {detail}
                  </td>
                </tr>
              )}
            </Fragment>
          );
        })}
        {virtual && bottomSpacer > 0 && <tr data-spacer="1" aria-hidden style={{ height: bottomSpacer }} />}
        {!virtual && sorted.length > limit && (
          <tr ref={sentinel}>
            <td colSpan={cols.length} className="px-3 py-3 text-center font-sans text-xs text-mute">
              Mostrando {limit.toLocaleString("es-ES")} de {sorted.length.toLocaleString("es-ES")}…{" "}
              <button onClick={() => setLimit(sorted.length)} className="text-neon hover:underline">
                Ver todas
              </button>
            </td>
          </tr>
        )}
        {sorted.length === 0 && (tf ? ordered.length > 0 : empty) && (
          <tr>
            <td colSpan={cols.length} className="px-3 py-6 text-center font-sans text-sm text-mute">
              {tf && ordered.length > 0 ? `Nada coincide con «${tf}»` : empty}
            </td>
          </tr>
        )}
      </tbody>
    </table>
    {colMenu && (
      <ColumnMenu
        at={colMenu}
        items={columns
          .map((c, i) => ({ id: c.id, label: nodeText(c.header).trim(), locked: infos[i].locked, on: shownIds.includes(c.id) }))
          .filter((x) => x.label)}
        onToggle={(id) => saveColset(toggled(infos, saved, id))}
        onReset={() => {
          resetColumns();
          setColMenu(null);
        }}
        onClose={() => setColMenu(null)}
      />
    )}
    </div>
  );
}

/** El menú de columnas (clic derecho en la cabecera): una casilla por columna y «Restablecer». */
function ColumnMenu({
  at,
  items,
  onToggle,
  onReset,
  onClose,
}: {
  at: { x: number; y: number };
  items: { id: string; label: string; locked?: boolean; on: boolean }[];
  onToggle: (id: string) => void;
  onReset: () => void;
  onClose: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState(at);
  useEffect(() => {
    const r = box.current?.getBoundingClientRect();
    if (r) setPos({ x: Math.max(6, Math.min(at.x, window.innerWidth - r.width - 6)), y: Math.max(6, Math.min(at.y, window.innerHeight - r.height - 6)) });
  }, [at]);
  useEffect(() => {
    const down = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) onClose();
    };
    const esc = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("mousedown", down);
    window.addEventListener("blur", onClose);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("mousedown", down);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("keydown", esc, true);
    };
  }, [onClose]);
  return createPortal(
    <div ref={box} role="menu" aria-label="Columnas" data-no-ctx className="fixed z-[70] min-w-48 max-w-72 rounded-lg border border-line-2 bg-panel p-1 text-[13px] shadow-2xl" style={{ left: pos.x, top: pos.y }}>
      <div className="px-2.5 pt-1 pb-1.5 text-[11px] font-medium text-mute">Columnas</div>
      {items.map((it) => (
        <label key={it.id} role="menuitemcheckbox" aria-checked={it.on} className={`flex items-center gap-2.5 rounded-md px-2.5 py-1.5 ${it.locked ? "text-mute" : "cursor-pointer text-dim hover:bg-neon/10 hover:text-ink"}`}>
          <input type="checkbox" checked={it.on} disabled={it.locked} onChange={() => onToggle(it.id)} className="accent-(--color-neon)" />
          <span className="min-w-0 flex-1 truncate">{it.label}</span>
        </label>
      ))}
      <div className="my-1 border-t border-line" />
      <button role="menuitem" onClick={onReset} className="flex w-full rounded-md px-2.5 py-1.5 text-left text-dim hover:bg-neon/10 hover:text-ink">
        Restablecer columnas
      </button>
    </div>,
    document.body,
  );
}
