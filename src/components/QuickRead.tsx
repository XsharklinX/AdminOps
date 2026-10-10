// «Lectura rápida»: una línea bajo el título que resume lo que importa de la
// pantalla antes del detalle («Todo bien salvo 2 cosas: …»). Sale de los mismos
// estados que la barra lateral pone al lado de cada sección; un clic lleva a cada una.
import { Eye } from "lucide-react";
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
  return items.length === 1 ? "Todo bien salvo 1 cosa:" : `Todo bien salvo ${items.length} cosas:`;
}

export function QuickRead({ page, badges, onGo }: { page: PageId; badges: Record<string, Badge>; onGo: (section: string) => void }) {
  const items = quickItems(page, badges);
  if (!items.length) return null;
  return (
    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 border-b border-line bg-panel/40 px-8 py-1.5 text-xs">
      <span className="flex items-center gap-1.5 text-mute">
        <Eye size={12} /> Lectura rápida · {quickSentence(items)}
      </span>
      {items.map((it) => (
        <button
          key={it.section}
          onClick={() => onGo(it.section)}
          title={it.badge.title}
          className={`rounded-md border px-2 py-0.5 transition-colors hover:bg-panel-2 ${it.badge.tone === "bad" ? "border-bad/40 text-bad" : "border-warn/40 text-warn"}`}
        >
          {it.label}: {it.badge.title || it.badge.text}
        </button>
      ))}
    </div>
  );
}
