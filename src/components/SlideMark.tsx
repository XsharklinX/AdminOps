// El resaltado de «dónde estoy» que se desliza hasta el nuevo elemento en vez de
// saltar (1.2.9). Se pone como primer hijo de un contenedor con `relative isolate`
// y busca dentro de él el elemento marcado con aria-current. Escribe sus medidas
// directamente en el elemento, sin estado: no vuelve a pintar nada.
import { useEffect, useLayoutEffect, useRef, type RefObject } from "react";

export type SlideKind = "row" | "rail" | "underline";

const LOOK: Record<SlideKind, string> = {
  row: "rounded-md bg-neon/10",
  rail: "rounded-lg bg-neon/10 shadow-[inset_3px_0_0_var(--color-neon)]",
  underline: "rounded-full bg-neon",
};

export function SlideMark({ within, kind = "row", selector = '[aria-current="page"]' }: { within: RefObject<HTMLElement | null>; kind?: SlideKind; selector?: string }) {
  const mark = useRef<HTMLSpanElement>(null);
  const placed = useRef(false);

  const place = () => {
    const root = within.current;
    const el = mark.current;
    if (!root || !el) return;
    const target = root.querySelector<HTMLElement>(selector);
    if (!target || target.offsetParent === null) {
      el.style.opacity = "0";
      return;
    }
    const r = root.getBoundingClientRect();
    const t = target.getBoundingClientRect();
    const x = t.left - r.left + root.scrollLeft - root.clientLeft;
    const y = t.top - r.top + root.scrollTop - root.clientTop + (kind === "underline" ? t.height - 2 : 0);
    // La primera vez se coloca sin recorrido: no tiene de dónde venir.
    if (!placed.current) el.style.transition = "none";
    el.style.transform = `translate(${x}px, ${y}px)`;
    el.style.width = `${t.width}px`;
    el.style.height = kind === "underline" ? "2px" : `${t.height}px`;
    el.style.opacity = "1";
    if (!placed.current) {
      placed.current = true;
      void el.offsetWidth;
      el.style.transition = "";
    }
  };

  // Cada vez que el contenedor se vuelve a pintar (cambió la pantalla actual, se abrió una sección…).
  useLayoutEffect(place);
  useEffect(() => {
    const root = within.current;
    if (!root || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(place);
    ro.observe(root);
    window.addEventListener("resize", place);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", place);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- place lee siempre el DOM actual
  }, []);

  return <span ref={mark} aria-hidden className={`pointer-events-none absolute top-0 left-0 -z-10 opacity-0 transition-[transform,width,height,opacity] duration-[260ms] ease-[cubic-bezier(0.34,1.3,0.64,1)] ${LOOK[kind]}`} />;
}
