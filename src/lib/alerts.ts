import type { WindowsAlert } from "./api";

/** Qué se ve en la campana: todo, o solo un nivel. */
export type AlertFilter = "all" | "bad" | "warn" | "info";

export const LEVEL_LABEL: Record<WindowsAlert["level"], string> = { bad: "Graves", warn: "Avisos", info: "Información" };

/** Cuántos hay de cada nivel (para las pestañas del filtro). */
export function countByLevel(alerts: WindowsAlert[]) {
  const n = { all: alerts.length, bad: 0, warn: 0, info: 0 };
  for (const a of alerts) if (a.level in n) n[a.level] += 1;
  return n;
}

export function filterAlerts(alerts: WindowsAlert[], filter: AlertFilter, onlyUnread: boolean) {
  return alerts.filter((a) => (filter === "all" || a.level === filter) && (!onlyUnread || !a.read));
}

const startOfDay = (ms: number) => {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
};

/** «Hoy», «Ayer» o la fecha, según el día del aviso. */
export function dayLabel(seconds: number, now = Date.now()): string {
  const days = Math.round((startOfDay(now) - startOfDay(seconds * 1000)) / 86_400_000);
  if (days <= 0) return "Hoy";
  if (days === 1) return "Ayer";
  if (days < 7) return new Date(seconds * 1000).toLocaleDateString("es", { weekday: "long" }).replace(/^./, (c) => c.toUpperCase());
  return new Date(seconds * 1000).toLocaleDateString("es", { day: "numeric", month: "long" });
}

/** Los avisos por días, del más reciente al más antiguo. */
export function groupByDay(alerts: WindowsAlert[], now = Date.now()): { label: string; items: WindowsAlert[] }[] {
  const groups: { label: string; items: WindowsAlert[] }[] = [];
  for (const a of [...alerts].sort((x, y) => y.time - x.time)) {
    const label = dayLabel(a.time, now);
    const last = groups[groups.length - 1];
    if (last?.label === label) last.items.push(a);
    else groups.push({ label, items: [a] });
  }
  return groups;
}

/** El color del contador de la campana: rojo si hay algo grave sin leer. */
export function worstUnread(alerts: WindowsAlert[]): "bad" | "warn" | null {
  const unread = alerts.filter((a) => !a.read);
  if (!unread.length) return null;
  return unread.some((a) => a.level === "bad") ? "bad" : "warn";
}
