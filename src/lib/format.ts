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
