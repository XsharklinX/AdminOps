// «Lo que se gana antes de cambiarlo», en los programas de inicio: Windows
// apunta cuánto retrasa cada programa el arranque (sucesos 101 a 110); se cruza
// con los programas que arrancan, se estima lo que se ganaría quitándolos y,
// tras el siguiente arranque, se mide lo que se ganó de verdad.
import type { BootAnalysis, StartupItem } from "./api";

export interface GainItem {
  item: StartupItem;
  delayMs: number;
  times: number;
}

const base = (s: string | null | undefined) =>
  (s ?? "")
    .replace(/^"|"$/g, "")
    .split(/[\\/]/)
    .pop()!
    .split(/\s/)[0]
    .toLowerCase()
    .replace(/\.exe$/, "");

/** Los programas de inicio activos con el retraso que Windows les apunta, de más a menos. */
export function gains(items: StartupItem[], culprits: BootAnalysis["culprits"]): GainItem[] {
  const apps = culprits.filter((c) => c.kind === "app");
  const out: GainItem[] = [];
  for (const item of items.filter((i) => i.enabled)) {
    const names = [base(item.target), base(item.command), item.name.toLowerCase()].filter(Boolean);
    const hit = apps.find((c) => {
      const n = base(c.name);
      return n.length > 2 && names.some((x) => x === n || x.startsWith(n) || n.startsWith(x));
    });
    if (hit) out.push({ item, delayMs: hit.avgDelayMs, times: hit.times });
  }
  return out.sort((a, b) => b.delayMs - a.delayMs);
}

/** Media de los arranques (ms). */
export const average = (boots: BootAnalysis["boots"]) => (boots.length ? boots.reduce((a, b) => a + b.totalMs, 0) / boots.length : 0);

/** Lo que se apuntó al quitar programas, para medir después. */
export interface Baseline {
  at: number;
  avgMs: number;
  estimateMs: number;
  names: string[];
}

/** El primer arranque después de quitar programas, frente a la media de antes. */
export function measured(boots: BootAnalysis["boots"], b: Baseline | null): { beforeMs: number; afterMs: number; estimateMs: number } | null {
  if (!b) return null;
  const after = boots.filter((x) => Date.parse(x.time) / 1000 > b.at);
  if (!after.length) return null;
  return { beforeMs: b.avgMs, afterMs: average(after), estimateMs: b.estimateMs };
}

/** «2 min 10 s», «48 s». */
export function secsText(ms: number): string {
  const s = Math.round(ms / 1000);
  return s >= 60 ? `${Math.floor(s / 60)} min ${s % 60} s` : `${s} s`;
}
