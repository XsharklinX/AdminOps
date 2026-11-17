// Listas largas sin botón «Ver más»: cuando el aviso del final de la lista
// llega a la vista, se pintan `step` filas más.
import { useEffect, useRef } from "react";

/** Devuelve la `ref` del aviso del final (`<div ref={…}>`); solo hace falta pintarlo mientras `total > limit`. */
export function useGrowOnScroll(total: number, limit: number, grow: () => void) {
  const end = useRef<HTMLDivElement>(null);
  const latest = useRef(grow);
  useEffect(() => {
    latest.current = grow;
  });
  useEffect(() => {
    const el = end.current;
    if (!el || total <= limit) return;
    const io = new IntersectionObserver((entries) => entries.some((x) => x.isIntersecting) && latest.current(), { rootMargin: "400px" });
    io.observe(el);
    return () => io.disconnect();
  }, [total, limit]);
  return end;
}
