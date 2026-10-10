// Ficha flotante (1.2.9): al detenerse el ratón sobre un nombre (equipo,
// dispositivo…) sale lo esencial sin abrir la pantalla; se esconde sola.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

const DELAY = 400;
const W = 288;

export function HoverCard({ card, children, className = "" }: { card: ReactNode; children: ReactNode; className?: string }) {
  const [at, setAt] = useState<{ x: number; y: number; up: boolean } | null>(null);
  const el = useRef<HTMLSpanElement>(null);
  const show = useRef(0);
  const hide = useRef(0);
  useEffect(
    () => () => {
      window.clearTimeout(show.current);
      window.clearTimeout(hide.current);
    },
    [],
  );
  const enter = (e: React.PointerEvent) => {
    if (e.pointerType !== "mouse") return;
    window.clearTimeout(hide.current);
    window.clearTimeout(show.current);
    show.current = window.setTimeout(() => {
      const r = el.current?.getBoundingClientRect();
      if (!r) return;
      const up = r.bottom + 190 > window.innerHeight;
      setAt({ x: Math.max(8, Math.min(r.left, window.innerWidth - W - 8)), y: up ? r.top - 6 : r.bottom + 6, up });
    }, DELAY);
  };
  const leave = () => {
    window.clearTimeout(show.current);
    hide.current = window.setTimeout(() => setAt(null), 140);
  };
  return (
    <span ref={el} className={className} onPointerEnter={enter} onPointerLeave={leave} onPointerDown={() => setAt(null)}>
      {children}
      {at &&
        createPortal(
          <div
            role="tooltip"
            onPointerEnter={() => window.clearTimeout(hide.current)}
            onPointerLeave={leave}
            style={{ left: at.x, top: at.y, width: W, transform: at.up ? "translateY(-100%)" : undefined }}
            className="hover-in pointer-events-auto fixed z-50 rounded-lg border border-line-2 bg-panel p-3 text-xs shadow-elev-3"
          >
            {card}
          </div>,
          document.body,
        )}
    </span>
  );
}
