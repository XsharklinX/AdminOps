// El aviso pequeño de la cabecera («1 aviso», «2 avisos»): lo que importa de la
// pantalla, en la misma línea que el título (1.4, idea 4; antes era una franja
// de «Lectura rápida» debajo). Sale de los mismos estados de cada sección; al
// pulsarlo se ve la lista y un clic lleva a cada una.
import { useEffect, useRef, useState } from "react";
import type { Badge } from "../lib/machineState";
import { navKey, sectionsOf } from "../lib/sections";
import type { PageId } from "./Sidebar";

export interface ReadItem {
  section: string;
  label: string;
  badge: Badge;
}

/** Lo que merece atención en una pantalla, lo peor primero. */
export function quickItems(page: PageId, badges: Record<string, Badge>): ReadItem[] {
  const rank = { bad: 0, warn: 1 } as Record<string, number>;
  return sectionsOf(page)
    .map((s) => ({ section: s.id, label: s.label, badge: badges[navKey(page, s.id)] }))
    .filter((x): x is ReadItem => !!x.badge && (x.badge.tone === "bad" || x.badge.tone === "warn"))
    .sort((a, b) => rank[a.badge.tone] - rank[b.badge.tone]);
}

export function quickSentence(items: ReadItem[]): string {
  if (!items.length) return "";
  return items.length === 1 ? "1 aviso" : `${items.length} avisos`;
}

export function QuickRead({ page, badges, onGo }: { page: PageId; badges: Record<string, Badge>; onGo: (section: string) => void }) {
  const items = quickItems(page, badges);
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!open) return;
    const down = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", down);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("mousedown", down);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);
  useEffect(() => setOpen(false), [page]);

  if (!items.length) return null;
  const bad = items[0].badge.tone === "bad";
  // Con uno solo no hace falta lista: lleva directo.
  const one = items.length === 1;
  return (
    <span ref={box} className="relative shrink-0">
      <button
        onClick={() => (one ? onGo(items[0].section) : setOpen((v) => !v))}
        title={items.map((it) => `${it.label}: ${it.badge.title || it.badge.text}`).join("\n")}
        aria-expanded={one ? undefined : open}
        className={`flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] font-medium transition-colors ${bad ? "bg-bad/15 text-bad hover:bg-bad/25" : "bg-warn/15 text-warn hover:bg-warn/25"}`}
      >
        <span className={`size-1.5 rounded-full ${bad ? "bg-bad" : "bg-warn"}`} />
        {quickSentence(items)}
      </button>
      {open && (
        <span className="absolute top-full left-0 z-40 mt-1.5 flex w-80 flex-col gap-0.5 rounded-lg border border-line-2 bg-panel p-1.5 shadow-2xl">
          {items.map((it) => (
            <button
              key={it.section}
              onClick={() => {
                setOpen(false);
                onGo(it.section);
              }}
              className="flex items-start gap-2 rounded-md px-2 py-1.5 text-left text-xs hover:bg-panel-2"
            >
              <span className={`mt-1 size-1.5 shrink-0 rounded-full ${it.badge.tone === "bad" ? "bg-bad" : "bg-warn"}`} />
              <span className="min-w-0">
                <span className="font-medium text-ink">{it.label}</span>
                <span className="block text-dim">{it.badge.title || it.badge.text}</span>
              </span>
            </button>
          ))}
        </span>
      )}
    </span>
  );
}
