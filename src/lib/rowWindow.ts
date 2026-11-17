// Listas largas sin «Mostrar más»: solo se pintan las filas que se ven (más unas
// pocas de margen) y el resto se sustituye por dos filas vacías de la altura que
// ocuparían, así la barra de desplazamiento mide la lista entera y se puede ir
// a cualquier sitio. Sin dependencias.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";

/** Altura de una fila mientras no se ha medido ninguna. */
const DEFAULT_ROW_PX = 34;
const OVERSCAN = 12;

export interface RowRange {
  start: number;
  end: number;
}

/**
 * Qué filas pintar. `offset` es lo que ya se ha desplazado la lista por encima
 * de su primera fila (scrollTop menos donde empieza la lista); `viewport` es el
 * alto visible y `rowPx` el de una fila.
 */
export function rowRange(offset: number, viewport: number, rowPx: number, count: number, overscan = OVERSCAN): RowRange {
  if (count <= 0 || rowPx <= 0) return { start: 0, end: Math.max(0, count) };
  const first = Math.floor(Math.max(0, offset) / rowPx);
  const last = Math.ceil((Math.max(0, offset) + Math.max(0, viewport)) / rowPx);
  const start = Math.min(Math.max(0, first - overscan), Math.max(0, count - 1));
  const end = Math.min(count, Math.max(start + 1, last + overscan));
  return { start, end };
}

/** El primer ascendiente que desplaza en vertical (donde se mueve la lista). */
function scrollParent(el: HTMLElement | null): HTMLElement | null {
  for (let p = el?.parentElement ?? null; p; p = p.parentElement) {
    const o = getComputedStyle(p).overflowY;
    if (o === "auto" || o === "scroll") return p;
  }
  return null;
}

/**
 * `body` es el `<tbody>` (o el contenedor) cuyas filas se ventanean. Devuelve el
 * rango a pintar y la altura de las dos filas de relleno (`top` y `bottom`).
 * Con `enabled` falso pinta todo (`start` 0, `end` count, rellenos a 0).
 */
export function useRowWindow(body: RefObject<HTMLElement | null>, count: number, enabled: boolean) {
  const [range, setRange] = useState<RowRange>({ start: 0, end: Math.min(count, 60) });
  const rowPx = useRef(DEFAULT_ROW_PX);
  const frame = useRef(0);

  const measure = useCallback(() => {
    const el = body.current;
    if (!el || !enabled) return;
    const scroller = scrollParent(el);
    const viewport = scroller?.clientHeight ?? window.innerHeight;
    const scrollerTop = scroller?.getBoundingClientRect().top ?? 0;
    // Cuánto de la lista ha quedado por encima del borde de arriba de la zona visible.
    const offset = scrollerTop - el.getBoundingClientRect().top;
    // Altura real de una fila: la de las pintadas (sin las dos de relleno).
    const rows = Array.from(el.children).filter((c) => !(c as HTMLElement).dataset.spacer) as HTMLElement[];
    if (rows.length > 3) {
      const h = rows.reduce((a, r) => a + r.getBoundingClientRect().height, 0) / rows.length;
      if (h > 8) rowPx.current = h;
    }
    const next = rowRange(offset, viewport, rowPx.current, count);
    setRange((cur) => (cur.start === next.start && cur.end === next.end ? cur : next));
  }, [body, count, enabled]);

  const schedule = useCallback(() => {
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(measure);
  }, [measure]);

  // Al cambiar la lista (otro filtro, otra ordenación), se vuelve a calcular.
  useLayoutEffect(() => {
    if (enabled) measure();
  }, [measure, enabled]);

  useEffect(() => {
    if (!enabled) return;
    const el = body.current;
    const scroller = scrollParent(el);
    const target: HTMLElement | Window = scroller ?? window;
    target.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    return () => {
      target.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      cancelAnimationFrame(frame.current);
    };
  }, [body, enabled, schedule]);

  if (!enabled) return { start: 0, end: count, top: 0, bottom: 0, rowPx: DEFAULT_ROW_PX };
  const start = Math.min(range.start, Math.max(0, count - 1));
  const end = Math.min(Math.max(range.end, start + 1), count);
  return { start, end, top: start * rowPx.current, bottom: Math.max(0, count - end) * rowPx.current, rowPx: rowPx.current };
}
