// Reordenar una lista: arrastrando (con una animación al recolocarse) o con
// Alt+↑ / Alt+↓ estando en la fila. Las funciones puras se prueban aparte.
import { useLayoutEffect, useRef, useState, type DragEvent, type KeyboardEvent, type RefObject } from "react";
import { motionOk } from "../components/motion";

/** La lista con el elemento `key` puesto justo antes de `target` (o al final si `target` es null). */
export function moveBefore<T>(items: T[], key: string, target: string | null, keyOf: (t: T) => string): T[] {
  const from = items.findIndex((x) => keyOf(x) === key);
  if (from < 0 || key === target) return items;
  const rest = items.filter((x) => keyOf(x) !== key);
  const at = target === null ? rest.length : rest.findIndex((x) => keyOf(x) === target);
  if (at < 0) return items;
  return [...rest.slice(0, at), items[from], ...rest.slice(at)];
}

/** La lista con `key` movido una posición arriba (-1) o abajo (+1). */
export function shiftItem<T>(items: T[], key: string, dir: -1 | 1, keyOf: (t: T) => string): T[] {
  const i = items.findIndex((x) => keyOf(x) === key);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= items.length) return items;
  const next = [...items];
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

/**
 * Anima los hijos de un contenedor (los que llevan `data-flip="clave"`) cuando
 * cambian de sitio: cada uno desliza desde donde estaba hasta donde queda.
 * `signature` cambia cuando cambia el orden.
 */
export function useFlip(ref: RefObject<HTMLElement | null>, signature: string) {
  const before = useRef(new Map<string, { x: number; y: number }>());
  useLayoutEffect(() => {
    const root = ref.current;
    if (!root) return;
    const box = root.getBoundingClientRect();
    const next = new Map<string, { x: number; y: number }>();
    const calm = !motionOk();
    root.querySelectorAll<HTMLElement>("[data-flip]").forEach((el) => {
      const r = el.getBoundingClientRect();
      const at = { x: r.left - box.left + root.scrollLeft, y: r.top - box.top + root.scrollTop };
      next.set(el.dataset.flip!, at);
      const was = before.current.get(el.dataset.flip!);
      if (!calm && was && (Math.abs(was.x - at.x) > 1 || Math.abs(was.y - at.y) > 1) && typeof el.animate === "function") {
        el.animate([{ transform: `translate(${was.x - at.x}px, ${was.y - at.y}px)` }, { transform: "none" }], { duration: 240, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" });
      }
    });
    before.current = next;
  }, [signature, ref]);
}

/** Arrastrar y soltar (y Alt+flechas) para una lista; las propiedades van en cada fila. */
export function useReorder<T>(items: T[], keyOf: (t: T) => string, onChange: (next: T[]) => void) {
  const [drag, setDrag] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const rowProps = (item: T) => {
    const k = keyOf(item);
    return {
      "data-flip": k,
      draggable: true,
      onDragStart: (e: DragEvent) => {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", k);
        setDrag(k);
      },
      onDragOver: (e: DragEvent) => {
        if (!drag || drag === k) return;
        e.preventDefault();
        setOver(k);
      },
      onDrop: (e: DragEvent) => {
        e.preventDefault();
        if (drag) onChange(moveBefore(items, drag, k, keyOf));
        setDrag(null);
        setOver(null);
      },
      onDragEnd: () => {
        setDrag(null);
        setOver(null);
      },
      onKeyDown: (e: KeyboardEvent) => {
        if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
        e.preventDefault();
        onChange(shiftItem(items, k, e.key === "ArrowUp" ? -1 : 1, keyOf));
      },
    };
  };
  return { rowProps, dragging: drag, over };
}
