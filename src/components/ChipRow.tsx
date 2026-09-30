import { ChevronDown, ChevronUp } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";

/** Alto aproximado de una fila de etiquetas (chip + separación). */
const ROW = 30;

function remembered(key: string | undefined, fallback: boolean): boolean {
  if (!key) return fallback;
  try {
    const v = localStorage.getItem(`adminops.chips.${key}`);
    return v === null ? fallback : v === "1";
  } catch {
    return fallback;
  }
}

/**
 * Fila de etiquetas que se pliega cuando hay muchas: se ven las primeras y un
 * botón las despliega. Si caben todas, el botón no aparece. Con `storageKey`
 * recuerda cómo la dejó el usuario.
 */
export function ChipRow({
  children,
  storageKey,
  rows = 1,
  defaultOpen = false,
  className = "",
}: {
  children: ReactNode;
  /** Para recordar si el usuario la dejó abierta o plegada. */
  storageKey?: string;
  /** Cuántas filas se ven plegada. */
  rows?: number;
  defaultOpen?: boolean;
  className?: string;
}) {
  const [open, setOpen] = useState(() => remembered(storageKey, defaultOpen));
  const [overflows, setOverflows] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const max = rows * ROW;

  // Solo hace falta el botón si de verdad no caben: se mide al pintar y al
  // cambiar el ancho de la ventana o el número de etiquetas.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const check = () => setOverflows(el.scrollHeight > max + 4);
    check();
    const ro = new ResizeObserver(check);
    ro.observe(el);
    return () => ro.disconnect();
  }, [children, max]);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (storageKey) {
      try {
        localStorage.setItem(`adminops.chips.${storageKey}`, next ? "1" : "0");
      } catch {
        /* sin almacenamiento */
      }
    }
  };

  return (
    <div className={className}>
      <div className="flex items-start gap-2">
        <div
          ref={box}
          style={{ maxHeight: open || !overflows ? undefined : max }}
          className={`flex min-w-0 flex-1 flex-wrap items-center gap-1.5 ${open || !overflows ? "" : "overflow-hidden"}`}
        >
          {children}
        </div>
        {overflows && (
          <button
            onClick={toggle}
            aria-expanded={open}
            className="flex shrink-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] text-mute transition-colors hover:bg-panel-2 hover:text-ink"
            title={open ? "Ver menos etiquetas" : "Ver todas las etiquetas"}
          >
            {open ? (
              <>
                Ver menos <ChevronUp size={11} />
              </>
            ) : (
              <>
                Ver todas <ChevronDown size={11} />
              </>
            )}
          </button>
        )}
      </div>
    </div>
  );
}
