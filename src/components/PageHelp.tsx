import { HelpCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";

const WIDTH = 320;
const MARGIN = 12;

/** «?» junto al título de la página: qué es y para qué sirve. Se abre al pasar el ratón o al hacer clic. */
export function PageHelp({ text }: { text: string }) {
  const [pinned, setPinned] = useState(false);
  // Posición en la ventana. Fija y no absoluta: la tira de pestañas tiene
  // scroll horizontal, y eso recorta todo lo que sobresale (también hacia abajo).
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  const ref = useRef<HTMLSpanElement>(null);

  const show = () => {
    const r = ref.current?.getBoundingClientRect();
    if (!r) return;
    // Se abre hacia la izquierda si no cabe a la derecha (el «?» de las pestañas está al borde).
    const left = Math.max(MARGIN, Math.min(r.left, window.innerWidth - WIDTH - MARGIN));
    setPos({ top: r.bottom + 8, left });
  };
  const hide = () => setPos(null);

  useEffect(() => {
    if (!pinned) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) {
        setPinned(false);
        hide();
      }
    };
    // Al hacer scroll o cambiar el tamaño la posición ya no vale: se cierra.
    const drop = () => {
      setPinned(false);
      hide();
    };
    document.addEventListener("mousedown", close);
    window.addEventListener("resize", drop);
    window.addEventListener("scroll", drop, true);
    return () => {
      document.removeEventListener("mousedown", close);
      window.removeEventListener("resize", drop);
      window.removeEventListener("scroll", drop, true);
    };
  }, [pinned]);

  if (!text) return null;
  const open = pos !== null;
  return (
    <span ref={ref} className="inline-flex" onMouseEnter={show} onMouseLeave={() => !pinned && hide()}>
      <button
        onClick={() => {
          if (pinned) {
            setPinned(false);
            hide();
          } else {
            setPinned(true);
            show();
          }
        }}
        className={`rounded-full transition-colors ${open ? "text-neon" : "text-mute hover:text-ink"}`}
        aria-label="Qué es esta página"
      >
        <HelpCircle size={16} strokeWidth={1.8} />
      </button>
      {pos && (
        <span
          style={{ top: pos.top, left: pos.left, width: WIDTH }}
          className="fixed z-50 rounded-lg border border-line-2 bg-panel px-3.5 py-2.5 text-[13px] leading-relaxed font-normal tracking-normal text-dim shadow-2xl"
        >
          {text}
        </span>
      )}
    </span>
  );
}
