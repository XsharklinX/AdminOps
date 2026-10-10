// «Seguir donde lo dejaste»: al reabrir AdminOps, una cinta ofrece volver a la
// última pantalla y sección en la que se estaba. Un clic y estás.
import { History, X } from "lucide-react";
import { useState } from "react";
import { pageLabel, isPageId, type PageId } from "./Sidebar";
import { sectionLabel } from "../lib/sections";

const KEY = "adminops.lastPlace";

export interface Place {
  page: PageId;
  section: string | null;
  at: number;
}

/** Apunta dónde se está (la cinta lo ofrece la próxima vez). */
export function rememberPlace(page: PageId, section: string | null) {
  if (page === "dashboard" || page === "settings") return;
  try {
    localStorage.setItem(KEY, JSON.stringify({ page, section, at: Date.now() }));
  } catch {
    /* sin almacenamiento */
  }
}

/** El sitio de la última vez, si merece la pena ofrecerlo (de los últimos 7 días). */
export function lastPlace(now = Date.now()): Place | null {
  try {
    const p = JSON.parse(localStorage.getItem(KEY) ?? "null") as Place | null;
    if (!p || !isPageId(p.page) || now - p.at > 7 * 24 * 3600 * 1000) return null;
    return p;
  } catch {
    return null;
  }
}

/** «hace 5 min», «hace 3 h», «hace 2 días». */
export function ago(ms: number): string {
  const m = Math.round(ms / 60000);
  if (m < 1) return "hace un momento";
  if (m < 60) return `hace ${m} min`;
  const h = Math.round(m / 60);
  if (h < 24) return `hace ${h} h`;
  const d = Math.round(h / 24);
  return `hace ${d} ${d === 1 ? "día" : "días"}`;
}

/** Se lee una vez al abrir: lo que se haga después ya no cuenta. */
const atStart = lastPlace();

export function ResumeRibbon({ current, onGo }: { current: PageId; onGo: (page: PageId, section: string | null) => void }) {
  const [shown, setShown] = useState(() => atStart !== null && atStart.page !== current);
  if (!shown || !atStart) return null;
  const where = [pageLabel(atStart.page), atStart.section ? sectionLabel(atStart.page, atStart.section) : null].filter(Boolean).join(" › ");
  return (
    <div className="flex items-center gap-3 border-b border-line bg-neon/5 px-8 py-1.5 text-xs">
      <History size={13} className="shrink-0 text-neon" />
      <span className="min-w-0 flex-1 truncate text-dim">
        Seguir donde lo dejaste: <b className="font-medium text-ink">{where}</b> <span className="text-mute">· {ago(Date.now() - atStart.at)}</span>
      </span>
      <button
        onClick={() => {
          setShown(false);
          onGo(atStart.page, atStart.section);
        }}
        className="rounded-md border border-neon/50 px-2 py-0.5 font-medium text-neon hover:bg-neon/10"
      >
        Volver
      </button>
      <button onClick={() => setShown(false)} className="rounded p-0.5 text-mute hover:text-ink" aria-label="Cerrar">
        <X size={13} />
      </button>
    </div>
  );
}
