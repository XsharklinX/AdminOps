// Diario de cambios: agruparlo por día («Hoy», «Ayer», la fecha) y filtrarlo.
import type { JournalEntry } from "./api";

export type JournalFilter = "all" | "apply" | "run" | "revert" | "failed" | "undoable";

export const JOURNAL_FILTERS: [JournalFilter, string][] = [
  ["all", "Todo"],
  ["apply", "Ajustes aplicados"],
  ["run", "Acciones"],
  ["revert", "Deshechos"],
  ["failed", "Con error"],
  ["undoable", "Se pueden deshacer"],
];

export function matchesJournal(e: JournalEntry, filter: JournalFilter, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (q && !`${e.title} ${e.message ?? ""}`.toLowerCase().includes(q)) return false;
  switch (filter) {
    case "all":
      return true;
    case "failed":
      return !e.ok;
    case "undoable":
      return e.undoable && !e.reverted && e.ok;
    default:
      return e.op === filter;
  }
}

const startOfDay = (ms: number) => {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
};

export interface JournalDay {
  /** Medianoche de ese día (ms). */
  day: number;
  label: string;
  entries: JournalEntry[];
}

/** Los cambios, del más reciente al más antiguo, por días. */
export function groupByDay(entries: JournalEntry[], now = Date.now()): JournalDay[] {
  const today = startOfDay(now);
  const out: JournalDay[] = [];
  for (const e of [...entries].sort((a, b) => b.timestamp - a.timestamp || b.id - a.id)) {
    const day = startOfDay(e.timestamp * 1000);
    const last = out[out.length - 1];
    if (last && last.day === day) {
      last.entries.push(e);
      continue;
    }
    const diff = Math.round((today - day) / 86_400_000);
    const date = new Date(day).toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long", ...(new Date(day).getFullYear() !== new Date(now).getFullYear() ? { year: "numeric" } : {}) });
    out.push({ day, label: diff === 0 ? "Hoy" : diff === 1 ? "Ayer" : date.charAt(0).toUpperCase() + date.slice(1), entries: [e] });
  }
  return out;
}
