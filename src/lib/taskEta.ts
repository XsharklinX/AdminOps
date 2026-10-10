// Centro de tareas: cuánto lleva cada tarea y cuánto le queda, calculado con la
// velocidad real (no con una estimación fija). Si no hay datos suficientes para
// decirlo con honradez, se dice «calculando».
import type { PageId } from "../components/Sidebar";

/** Porcentaje que trae el mensaje de una etapa («Copiando… 45 %», «Bloque 120 de 400»). */
export function percentOf(message: string): number | null {
  const pct = message.match(/(\d{1,3}(?:[.,]\d+)?)\s?%/);
  if (pct) {
    const v = Number(pct[1].replace(",", "."));
    return v >= 0 && v <= 100 ? v : null;
  }
  const of = message.match(/(\d[\d.]*)\s*(?:de|\/)\s*(\d[\d.]*)/i);
  if (of) {
    const a = Number(of[1].replace(/\./g, ""));
    const b = Number(of[2].replace(/\./g, ""));
    if (b > 0 && a <= b) return (a / b) * 100;
  }
  return null;
}

export interface Sample {
  at: number;
  pct: number;
}

/**
 * Segundos que faltan con la velocidad de las últimas muestras, o null si no se
 * puede decir aún (poco avance, poco tiempo o el porcentaje ha retrocedido).
 */
export function remainingSeconds(samples: Sample[]): number | null {
  if (samples.length < 2) return null;
  const last = samples[samples.length - 1];
  // La velocidad de los últimos dos minutos: lo de hace media hora ya no cuenta.
  const recent = samples.filter((s) => last.at - s.at <= 120_000);
  const first = recent[0];
  const dt = (last.at - first.at) / 1000;
  const dp = last.pct - first.pct;
  if (dt < 4 || dp <= 0.5 || last.pct >= 100) return null;
  return Math.round(((100 - last.pct) / dp) * dt);
}

/** «2 min 10 s», «45 s», «1 h 5 min». */
export function humanDuration(s: number): string {
  if (s >= 3600) return `${Math.floor(s / 3600)} h ${Math.round((s % 3600) / 60)} min`;
  if (s >= 60) return `${Math.floor(s / 60)} min ${s % 60} s`;
  return `${s} s`;
}

/** Dónde se ve el resultado de cada tarea, por su clave. */
const TASK_PAGE: [string, PageId, string | null][] = [
  ["install-apps", "apps", "install"],
  ["software", "apps", "update"],
  ["uninstall", "apps", "uninstall"],
  ["apps", "apps", "bloatware"],
  ["wu-search", "apps", "winupdate"],
  ["migrate", "data", "migrate"],
  ["vault", "data", "vault"],
  ["encrypt", "data", "vault"],
  ["decrypt", "data", "vault"],
  ["recover", "data", "recover"],
  ["carve", "data", "recover"],
  ["wipe", "data", "wipe"],
  ["disk-", "space", null],
  ["part-scan", "space", null],
  ["boot-repair", "space", null],
  ["space", "space", null],
  ["diagnostics", "machine", "diagnostics"],
  ["profile", "recipes", "profiles"],
  ["lan-", "router", "devices"],
  ["speedtest", "router", "speed"],
  ["inventory", "inventory", null],
  ["stress", "machine", "hardware"],
];

export function pageOfTask(task: string): [PageId, string | null] | null {
  const hit = TASK_PAGE.find(([prefix]) => task.startsWith(prefix));
  return hit ? [hit[1], hit[2]] : null;
}
