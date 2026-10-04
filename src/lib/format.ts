const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function bytes(n: number, digits = 1): string {
  let i = 0;
  while (n >= 1024 && i < UNITS.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toFixed(i === 0 ? 0 : digits)} ${UNITS[i]}`;
}

export function rate(n: number): string {
  return `${bytes(n)}/s`;
}

export function pct(n: number): string {
  return `${Math.round(n)}%`;
}

export function duration(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** Color semántico según la carga: normal → neutro, alta → ámbar, crítica → rojo. */
export function loadColor(p: number): string {
  if (p >= 90) return "var(--color-bad)";
  if (p >= 70) return "var(--color-warn)";
  return "var(--color-dim)";
}

/** 1234.5 → "RD$ 1,234.50" (igual que en el informe PDF). */
export function money(v: number, currency: string): string {
  const n = Number.isFinite(v) ? v : 0;
  const s = Math.abs(n).toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const cur = currency.trim();
  return `${n < 0 ? "-" : ""}${cur}${cur ? " " : ""}${s}`;
}

/** Ruta sin el nombre del usuario de Windows: "C:\Users\ana\Documents\x" → "Carpeta personal\Documents\x". */
export function friendlyPath(p: string): string {
  return p.replace(/^[a-z]:\\users\\[^\\]+/i, "Carpeta personal");
}

/** Un instante: segundos desde 1970 (lo que manda el backend) o una fecha ISO. */
export type When = number | string | Date;

function toDate(t: When): Date {
  return t instanceof Date ? t : typeof t === "number" ? new Date(t * 1000) : new Date(t);
}

const valid = (d: Date) => !Number.isNaN(d.getTime());

/** «3 oct» (con el año si no es el actual). */
export function shortDate(t: When): string {
  const d = toDate(t);
  if (!valid(d)) return String(t);
  const year = d.getFullYear() !== new Date().getFullYear() ? { year: "numeric" as const } : {};
  return d.toLocaleDateString("es", { day: "numeric", month: "short", ...year });
}

/** «3 oct 2026». */
export function fullDate(t: When): string {
  const d = toDate(t);
  return valid(d) ? d.toLocaleDateString("es", { day: "numeric", month: "short", year: "numeric" }) : String(t);
}

/** «3 oct 2026, 14:05». */
export function dateTime(t: When): string {
  const d = toDate(t);
  return valid(d) ? d.toLocaleString("es", { dateStyle: "medium", timeStyle: "short" }) : String(t);
}

/** «14:05». */
export function timeOfDay(t: When): string {
  const d = toDate(t);
  return valid(d) ? d.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" }) : String(t);
}

/** «hace un momento», «hace 5 min», «hace 3 h», «ayer», «hace 4 días», «hace 2 meses», «hace 1 año». */
export function ago(t: When, now = Date.now()): string {
  const d = toDate(t);
  if (!valid(d)) return String(t);
  const mins = Math.floor((now - d.getTime()) / 60_000);
  if (mins < 1) return "hace un momento";
  if (mins < 60) return `hace ${mins} min`;
  if (mins < 24 * 60) return `hace ${Math.floor(mins / 60)} h`;
  const days = Math.floor(mins / (24 * 60));
  if (days === 1) return "ayer";
  if (days < 60) return `hace ${days} días`;
  if (days < 730) return `hace ${Math.floor(days / 30)} meses`;
  const years = Math.floor(days / 365);
  return `hace ${years} ${years === 1 ? "año" : "años"}`;
}
