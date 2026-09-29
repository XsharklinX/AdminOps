import { HelpCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";

/** «?» junto al título de la página: qué es y para qué sirve. Se abre al pasar el ratón o al hacer clic. */
export function PageHelp({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  const [pinned, setPinned] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!pinned) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) {
        setPinned(false);
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [pinned]);

  if (!text) return null;
  return (
    <span ref={ref} className="relative inline-flex" onMouseEnter={() => setOpen(true)} onMouseLeave={() => !pinned && setOpen(false)}>
      <button
        onClick={() => {
          setPinned(!pinned);
          setOpen(!pinned);
        }}
        className={`rounded-full transition-colors ${open ? "text-neon" : "text-mute hover:text-ink"}`}
        aria-label="Qué es esta página"
      >
        <HelpCircle size={16} strokeWidth={1.8} />
      </button>
      {open && (
        <span className="absolute top-full left-0 z-40 mt-2 w-80 rounded-lg border border-line-2 bg-panel px-3.5 py-2.5 text-[13px] leading-relaxed font-normal tracking-normal text-dim shadow-2xl">
          {text}
        </span>
      )}
    </span>
  );
}
