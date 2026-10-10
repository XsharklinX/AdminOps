// Panel de detalle (1.2.9): en vez de una ventana que tapa todo, el detalle de
// una fila se desliza desde la derecha y la lista sigue a la vista. Esc lo cierra.
// Con `onPrev` y `onNext` se pasa de una fila a la siguiente sin cerrarlo.
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { iconBtn } from "./ui";

export function Drawer({
  title,
  subtitle,
  onClose,
  onPrev,
  onNext,
  children,
  width = 440,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  onClose: () => void;
  onPrev?: () => void;
  onNext?: () => void;
  children: ReactNode;
  width?: number;
}) {
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Un diálogo abierto encima tiene prioridad: Esc cierra primero ese.
      if (e.key === "Escape" && !e.defaultPrevented && !document.querySelector('[role="dialog"]')) close.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return createPortal(
    <aside
      aria-label={typeof title === "string" ? title : "Detalle"}
      style={{ width }}
      className="drawer-in fixed top-12 right-0 bottom-0 z-30 flex max-w-[92vw] flex-col border-l border-line-2 bg-panel shadow-elev-3"
    >
      <header className="flex items-start gap-2 border-b border-line px-4 py-3">
        <div className="min-w-0 flex-1">
          <h3 className="truncate font-semibold text-ink">{title}</h3>
          {subtitle && <p className="truncate text-xs text-mute">{subtitle}</p>}
        </div>
        {onPrev && (
          <button className={iconBtn} onClick={onPrev} title="Anterior (↑)" aria-label="Anterior">
            <ChevronUp size={15} />
          </button>
        )}
        {onNext && (
          <button className={iconBtn} onClick={onNext} title="Siguiente (↓)" aria-label="Siguiente">
            <ChevronDown size={15} />
          </button>
        )}
        <button className={iconBtn} onClick={onClose} title="Cerrar (Esc)" aria-label="Cerrar">
          <X size={15} />
        </button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">{children}</div>
    </aside>,
    document.body,
  );
}
