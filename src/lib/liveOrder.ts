// Listas en vivo (Procesos, Puestos, Dispositivos): mientras el puntero está
// encima, el ORDEN se congela, no los datos. Las cifras siguen cambiando en su
// sitio; al apartar el puntero (o tras unos segundos quieto) la lista se recoloca
// con animación.
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { motionOk } from "../components/motion";

/** Segundos sin mover el ratón tras los que el orden vuelve a ponerse al día. */
export const HOLD_IDLE_MS = 8000;

/**
 * Pone `items` en el orden anterior (`prevKeys`): lo nuevo va al final (en el
 * orden en que llega) y lo que ya no está desaparece.
 */
export function applyFrozenOrder<T>(items: T[], prevKeys: string[], keyOf: (t: T, i: number) => string): T[] {
  const byKey = new Map<string, T>();
  items.forEach((it, i) => byKey.set(keyOf(it, i), it));
  const out: T[] = [];
  const used = new Set<string>();
  for (const k of prevKeys) {
    const it = byKey.get(k);
    if (it !== undefined && !used.has(k)) {
      out.push(it);
      used.add(k);
    }
  }
  items.forEach((it, i) => {
    if (!used.has(keyOf(it, i))) out.push(it);
  });
  return out;
}

/** Con `frozen`, `items` sale en el orden con el que se enseñó la última vez. */
export function useFrozenOrder<T>(items: T[], keyOf: (t: T, i: number) => string, frozen: boolean): T[] {
  const shown = useRef<string[]>([]);
  return useMemo(() => {
    const out = frozen ? applyFrozenOrder(items, shown.current, keyOf) : items;
    shown.current = out.map(keyOf);
    return out;
    // `keyOf` se crea en cada pintado; lo que importa son los datos y si está congelado.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, frozen]);
}

/**
 * ¿Está el puntero dentro de `ref`? Verdadero mientras está dentro y se mueve;
 * pasados `idleMs` sin moverse vuelve a falso (y a verdadero al moverlo otra vez).
 */
export function usePointerHold(ref: RefObject<HTMLElement | null>, enabled = true, idleMs = HOLD_IDLE_MS): boolean {
  const [held, setHeld] = useState(false);
  useEffect(() => {
    if (!enabled) return;
    let timer = 0;
    const set = (v: boolean) => setHeld((h) => (h === v ? h : v));
    const move = (e: PointerEvent) => {
      window.clearTimeout(timer);
      const inside = !!ref.current && e.target instanceof Node && ref.current.contains(e.target);
      set(inside);
      if (inside) timer = window.setTimeout(() => set(false), idleMs);
    };
    const leave = () => {
      window.clearTimeout(timer);
      set(false);
    };
    document.addEventListener("pointermove", move, { passive: true });
    document.documentElement.addEventListener("pointerleave", leave);
    window.addEventListener("blur", leave);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("pointermove", move);
      document.documentElement.removeEventListener("pointerleave", leave);
      window.removeEventListener("blur", leave);
    };
  }, [ref, enabled, idleMs]);
  return enabled && held;
}

/**
 * Anima las filas (`[data-flip="clave"]` dentro de `ref`) hacia su sitio nuevo
 * cuando se acaba la pausa. Con movimiento reducido, no anima.
 */
export function useReorderFlip(ref: RefObject<HTMLElement | null>, held: boolean) {
  const spots = useRef<Map<string, number>>(new Map());
  const was = useRef(false);
  useLayoutEffect(() => {
    const root = ref.current;
    if (!root) return;
    if (held) {
      // Durante la pausa se apunta dónde está cada fila.
      const m = new Map<string, number>();
      root.querySelectorAll<HTMLElement>("[data-flip]").forEach((el) => m.set(el.dataset.flip!, el.getBoundingClientRect().top));
      spots.current = m;
    } else if (was.current && motionOk()) {
      root.querySelectorAll<HTMLElement>("[data-flip]").forEach((el) => {
        const before = spots.current.get(el.dataset.flip!);
        if (before === undefined) return;
        const dy = before - el.getBoundingClientRect().top;
        if (Math.abs(dy) < 1) return;
        el.animate([{ transform: `translateY(${dy}px)` }, { transform: "none" }], { duration: 240, easing: "ease-out" });
      });
    }
    was.current = held;
  });
}
