// Ayudas de «¿Qué cambió?» y «Pregúntale al equipo»: fechas y frases.
import type { ChangeKind, ChangeView } from "./api";

/** AAAA-MM-DD en hora local. */
export function isoDay(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** El lunes más reciente anterior a hoy (si hoy es lunes, el de hace una semana). */
export function lastMonday(today: Date): Date {
  const d = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const back = ((d.getDay() + 6) % 7) || 7;
  d.setDate(d.getDate() - back);
  return d;
}

export function addDays(d: Date, n: number): Date {
  const r = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  r.setDate(r.getDate() + n);
  return r;
}

const WEEKDAYS = ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"];

/** «2026-11-16» → «lunes». */
export function weekdayOf(day: string): string {
  const [y, m, d] = day.split("-").map(Number);
  if (!y || !m || !d) return day;
  return WEEKDAYS[new Date(y, m - 1, d).getDay()];
}

/** «2026-11-16» → «16/11/2026». */
export function dayEs(day: string): string {
  const [y, m, d] = day.split("-");
  return y && m && d ? `${d}/${m}/${y}` : day;
}

export const KIND_LABEL: Record<ChangeKind, string> = {
  program: "Programas",
  driver: "Drivers",
  update: "Actualizaciones de Windows",
  service: "Servicios",
  startup: "Lo que arranca con Windows",
  power: "Energía",
};

export const KIND_ORDER: ChangeKind[] = ["program", "driver", "update", "service", "startup", "power"];

const SERVICE_MODE: Record<string, string> = { Auto: "automático", Manual: "manual", Disabled: "desactivado", Boot: "arranque", System: "sistema" };

/** La frase de una línea: «Nuevo · versión 2.0», «Versión 1.0 → 2.0», «Ya no está». */
export function describeChange(c: ChangeView): string {
  const val = (v: string | null) => (c.kind === "service" && v ? (SERVICE_MODE[v] ?? v) : (v ?? ""));
  if (c.change === "added") {
    if (c.kind === "service") return `Nuevo servicio · inicio ${val(c.new)}`;
    if (c.kind === "power") return val(c.new);
    return c.new ? `Nuevo · ${c.kind === "update" ? `instalada el ${c.new}` : `versión ${c.new}`}` : "Nuevo";
  }
  if (c.change === "removed") return "Ya no está";
  if (c.kind === "service") return `Inicio ${val(c.old)} → ${val(c.new)}`;
  if (c.kind === "power") return `${val(c.old)} → ${val(c.new)}`;
  return `Versión ${val(c.old) || "—"} → ${val(c.new) || "—"}`;
}

export function groupByKind(list: ChangeView[]): { kind: ChangeKind; items: ChangeView[] }[] {
  return KIND_ORDER.map((kind) => ({ kind, items: list.filter((c) => c.kind === kind) })).filter((g) => g.items.length > 0);
}
