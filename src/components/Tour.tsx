// El recorrido guiado: un foco sobre lo nuevo y una tarjeta con su explicación.
import { ChevronLeft, ChevronRight, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useState } from "react";
import { createPortal } from "react-dom";
import { openHelp } from "../lib/help";
import { onTour, tourFor, type TourStep } from "../lib/tour";

export function Tour({ version }: { version: string }) {
  const [steps, setSteps] = useState<TourStep[] | null>(null);
  const [i, setI] = useState(0);
  const [rect, setRect] = useState<DOMRect | null>(null);

  useEffect(
    () =>
      onTour((v) => {
        const s = tourFor(v ?? version);
        if (!s) return;
        // Solo los pasos cuyo sitio está a la vista (o los que van en el centro).
        setSteps(s.filter((x) => !x.target || document.querySelector(`[data-tour="${x.target}"]`)));
        setI(0);
      }),
    [version],
  );

  const step = steps?.[i];
  const measure = useCallback(() => {
    if (!step?.target) return setRect(null);
    const el = document.querySelector(`[data-tour="${step.target}"]`);
    setRect(el ? el.getBoundingClientRect() : null);
  }, [step]);
  useLayoutEffect(measure, [measure]);
  useEffect(() => {
    if (!steps) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSteps(null);
      else if (e.key === "ArrowRight" || e.key === "Enter") setI((n) => Math.min(n + 1, steps.length - 1));
      else if (e.key === "ArrowLeft") setI((n) => Math.max(n - 1, 0));
      else return;
      e.preventDefault();
      e.stopPropagation();
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("resize", measure);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("resize", measure);
    };
  }, [steps, measure]);

  if (!steps || !step) return null;
  const last = i === steps.length - 1;
  const pad = 6;
  // La tarjeta va debajo del foco si cabe; si no, encima; sin foco, en el centro.
  const cardStyle: React.CSSProperties = rect
    ? { left: Math.max(12, Math.min(rect.left, window.innerWidth - 372)), top: rect.bottom + 200 < window.innerHeight ? rect.bottom + 14 : Math.max(12, rect.top - 190) }
    : { left: "50%", top: "40%", transform: "translate(-50%, -50%)" };
  return createPortal(
    <div className="fixed inset-0 z-[90]" role="dialog" aria-modal="true" aria-label="Recorrido de novedades">
      {rect ? (
        <div className="tour-spot pointer-events-none fixed rounded-lg" style={{ left: rect.left - pad, top: rect.top - pad, width: rect.width + pad * 2, height: rect.height + pad * 2 }} />
      ) : (
        <div className="fixed inset-0 bg-black/60" />
      )}
      <div className="fixed w-[360px] rounded-xl border border-line-2 bg-panel p-4 shadow-2xl" style={cardStyle}>
        <div className="flex items-start justify-between gap-2">
          <p className="text-sm font-semibold text-ink">{step.title}</p>
          <button onClick={() => setSteps(null)} className="text-mute hover:text-ink" aria-label="Saltar el recorrido">
            <X size={14} />
          </button>
        </div>
        <p className="mt-1.5 text-[13px] text-dim">{step.text}</p>
        <div className="mt-3 flex items-center gap-2">
          <span className="mr-auto font-mono text-[11px] text-mute">
            {i + 1} / {steps.length}
          </span>
          {i > 0 && (
            <button onClick={() => setI(i - 1)} className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-dim hover:bg-panel-2 hover:text-ink">
              <ChevronLeft size={13} /> Atrás
            </button>
          )}
          {last ? (
            <>
              <button
                onClick={() => {
                  setSteps(null);
                  openHelp("news");
                }}
                className="rounded-md px-2 py-1 text-xs text-dim hover:bg-panel-2 hover:text-ink"
              >
                Todas las novedades
              </button>
              <button onClick={() => setSteps(null)} className="rounded-md bg-neon px-3 py-1 text-xs font-medium text-on-neon">
                Entendido
              </button>
            </>
          ) : (
            <button autoFocus onClick={() => setI(i + 1)} className="flex items-center gap-1 rounded-md bg-neon px-3 py-1 text-xs font-medium text-on-neon">
              Siguiente <ChevronRight size={13} />
            </button>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}
