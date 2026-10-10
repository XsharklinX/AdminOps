// Un término técnico con subrayado de puntos: al pasar el ratón (o con el foco)
// dice qué es, si debe preocupar y qué hacer. Para el técnico que empieza y para
// explicárselo al cliente sin buscar en Google.
import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { lookupTerm } from "../lib/glossary";

export function Term({ k, children }: { k: string; children?: ReactNode }) {
  const entry = lookupTerm(k);
  const ref = useRef<HTMLSpanElement>(null);
  const tip = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  useLayoutEffect(() => {
    if (!open || !ref.current || !tip.current) return;
    const r = ref.current.getBoundingClientRect();
    const t = tip.current.getBoundingClientRect();
    const x = Math.max(8, Math.min(r.left, window.innerWidth - t.width - 8));
    const below = r.bottom + 6 + t.height < window.innerHeight;
    setPos({ x, y: below ? r.bottom + 6 : r.top - t.height - 6 });
  }, [open]);
  if (!entry) return <>{children ?? k}</>;
  return (
    <>
      <span
        ref={ref}
        tabIndex={0}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        className="cursor-help underline decoration-mute decoration-dotted underline-offset-[3px] outline-none focus-visible:decoration-neon"
        aria-describedby={open ? `term-${entry.term}` : undefined}
      >
        {children ?? k}
      </span>
      {open &&
        createPortal(
          <div
            ref={tip}
            id={`term-${entry.term}`}
            role="tooltip"
            className="pointer-events-none fixed z-[80] w-72 rounded-lg border border-line-2 bg-panel p-3 text-xs shadow-2xl"
            style={{ left: pos.x, top: pos.y }}
          >
            <p className="font-semibold text-ink">{entry.term}</p>
            <p className="mt-1 text-dim">{entry.what}</p>
            {entry.worry !== "—" && (
              <p className="mt-1.5 text-dim">
                <b className="font-medium text-warn">¿Preocupa?</b> {entry.worry}
              </p>
            )}
            <p className="mt-1.5 text-dim">
              <b className="font-medium text-ok">Qué hacer:</b> {entry.todo}
            </p>
          </div>,
          document.body,
        )}
    </>
  );
}
