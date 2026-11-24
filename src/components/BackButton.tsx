// «← Panel · Lento»: vuelve a la pantalla anterior (Alt+←). Mantenido pulsado
// medio segundo, o con el botón derecho, enseña las diez últimas para saltar.
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

export interface BackEntry {
  /** Nombre de la pantalla. */
  label: string;
  /** El filtro que tenía, si es corto. */
  filter: string | null;
}

const HOLD_MS = 500;

const text = (e: BackEntry) => (e.filter ? `${e.label} · ${e.filter}` : e.label);

export function BackButton({ entries, onBack, onJump }: { entries: BackEntry[]; onBack: () => void; onJump: (index: number) => void }) {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const btn = useRef<HTMLButtonElement>(null);
  const hold = useRef(0);
  const held = useRef(false);

  const openMenu = (x: number, y: number) => {
    held.current = true;
    setMenu({ x, y });
  };

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const down = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) close();
    };
    const esc = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        close();
      }
    };
    window.addEventListener("mousedown", down);
    window.addEventListener("blur", close);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("mousedown", down);
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", esc, true);
    };
  }, [menu]);

  useEffect(() => () => window.clearTimeout(hold.current), []);

  if (!entries.length) return null;
  const last = entries[0];
  return (
    <>
      <button
        ref={btn}
        data-no-ctx
        title="Volver (Alt+←)"
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          held.current = false;
          window.clearTimeout(hold.current);
          hold.current = window.setTimeout(() => {
            const r = btn.current?.getBoundingClientRect();
            if (r) openMenu(r.left, r.bottom + 4);
          }, HOLD_MS);
        }}
        onPointerUp={() => window.clearTimeout(hold.current)}
        onPointerLeave={() => window.clearTimeout(hold.current)}
        onClick={() => {
          // Si se mantuvo pulsado y salió el menú, el clic de soltar no vuelve.
          if (held.current) {
            held.current = false;
            return;
          }
          onBack();
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          openMenu(e.clientX, e.clientY);
        }}
        className="mr-1 flex max-w-56 shrink-0 items-center gap-1 rounded px-1 text-dim hover:bg-panel-2 hover:text-ink"
      >
        <ArrowLeft size={12} className="shrink-0" />
        <span className="truncate">{text(last)}</span>
      </button>
      {menu &&
        createPortal(
          <div ref={box} role="menu" data-no-ctx className="fixed z-[70] min-w-48 max-w-80 rounded-lg border border-line-2 bg-panel p-1 text-[13px] shadow-2xl" style={{ left: Math.max(6, Math.min(menu.x, window.innerWidth - 330)), top: menu.y }}>
            {entries.slice(0, 10).map((e, i) => (
              <button
                key={i}
                role="menuitem"
                onClick={() => {
                  setMenu(null);
                  onJump(i);
                }}
                className="flex w-full rounded-md px-2.5 py-1.5 text-left text-dim hover:bg-neon/10 hover:text-ink"
              >
                <span className="min-w-0 flex-1 truncate">{text(e)}</span>
              </button>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}
