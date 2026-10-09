import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { findRanges, step } from "../lib/pageFind";

const ALL = "adminops-find";
const CURRENT = "adminops-find-current";

/** ¿Deja el navegador marcar texto sin tocarlo? (WebView2 sí, desde hace años). */
const supported = typeof CSS !== "undefined" && "highlights" in CSS && typeof Highlight !== "undefined";

function clear() {
  if (!supported) return;
  CSS.highlights.delete(ALL);
  CSS.highlights.delete(CURRENT);
}

/**
 * Ctrl+F: busca en la pantalla actual (no en toda la app: eso es Ctrl+K).
 * Marca cada coincidencia y salta entre ellas con Intro y Mayús+Intro.
 */
export function PageFind({ page, onClose }: { page: string; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [count, setCount] = useState(0);
  const [index, setIndex] = useState(0);
  const ranges = useRef<Range[]>([]);
  const input = useRef<HTMLInputElement>(null);

  const root = useCallback(() => document.querySelector(`[data-page="${CSS.escape(page)}"]`), [page]);

  const paint = useCallback((list: Range[], at: number, scroll: boolean) => {
    if (!supported) return;
    if (!list.length) return clear();
    CSS.highlights.set(ALL, new Highlight(...list));
    CSS.highlights.set(CURRENT, new Highlight(list[at]));
    if (scroll) list[at].startContainer.parentElement?.scrollIntoView({ block: "center" });
  }, []);

  // Al escribir, y cada poco mientras está abierta: las pantallas cambian solas
  // (procesos, avisos) y las coincidencias de hace un momento pueden no existir ya.
  const search = useCallback(
    (scroll: boolean) => {
      const el = root();
      const list = el ? findRanges(el, query) : [];
      ranges.current = list;
      setCount(list.length);
      setIndex((i) => {
        const at = Math.min(i, Math.max(0, list.length - 1));
        paint(list, at, scroll);
        return at;
      });
    },
    [root, query, paint],
  );

  useEffect(() => {
    setIndex(0);
    const t = window.setTimeout(() => search(true), 120);
    return () => window.clearTimeout(t);
  }, [query, page, search]);

  useEffect(() => {
    if (!query.trim()) return;
    const t = window.setInterval(() => search(false), 2000);
    return () => window.clearInterval(t);
  }, [query, search]);

  useEffect(() => {
    input.current?.focus();
    return clear;
  }, []);

  const go = (dir: 1 | -1) => {
    const list = ranges.current;
    if (!list.length) return;
    const at = step(index, list.length, dir);
    setIndex(at);
    paint(list, at, true);
  };

  return (
    <div data-no-find className="absolute top-3 right-6 z-30 flex items-center gap-1 rounded-lg border border-line-2 bg-panel px-2 py-1.5 shadow-2xl" role="search">
      <Search size={13} className="text-mute" />
      <input
        ref={input}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            go(e.shiftKey ? -1 : 1);
          } else if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
        placeholder="Buscar en esta pantalla"
        aria-label="Buscar en esta pantalla"
        className="w-48 bg-transparent px-1 text-sm text-ink outline-none placeholder:text-mute"
      />
      <span className="min-w-14 text-right font-mono text-[11px] text-mute tabular">{query.trim() ? (count ? `${index + 1} de ${count}${count >= 2000 ? "+" : ""}` : "nada") : ""}</span>
      <button onClick={() => go(-1)} disabled={!count} className="rounded p-1 text-mute hover:text-ink disabled:opacity-30" title="Anterior (Mayús+Intro)" aria-label="Anterior">
        <ChevronUp size={14} />
      </button>
      <button onClick={() => go(1)} disabled={!count} className="rounded p-1 text-mute hover:text-ink disabled:opacity-30" title="Siguiente (Intro)" aria-label="Siguiente">
        <ChevronDown size={14} />
      </button>
      <button onClick={onClose} className="rounded p-1 text-mute hover:text-ink" title="Cerrar (Esc)" aria-label="Cerrar la búsqueda">
        <X size={14} />
      </button>
      {!supported && <span className="text-[11px] text-warn">Este equipo no deja marcar el texto.</span>}
    </div>
  );
}
