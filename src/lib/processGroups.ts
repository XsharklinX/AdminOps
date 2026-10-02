// Procesos: agruparlos por programa. Un navegador son treinta procesos con el
// mismo nombre; lo que se quiere saber es cuánto pesa el navegador.
import type { ProcessView } from "./api";

export interface ProcessGroup {
  /** Nombre del ejecutable en minúsculas. */
  key: string;
  name: string;
  procs: ProcessView[];
  cpu: number;
  memory: number;
  /** Lectura más escritura en disco, por segundo. */
  disk: number;
  /** La protección más fuerte de sus procesos. */
  protection: ProcessView["protection"];
  /** De quién son: un nombre, «varios» o null si no se sabe. */
  user: string | null;
}

const RANK: Record<ProcessView["protection"], number> = { none: 0, sensitive: 1, self: 2, critical: 3 };

const SYSTEM_USERS = ["system", "local service", "network service", "servicio local", "servicio de red"];

/** ¿Lo ejecuta Windows (servicios, sesión del sistema) y no una persona? */
export function isSystemProcess(p: Pick<ProcessView, "user">): boolean {
  const user = (p.user ?? "").replace(/^.*\\/, "").trim().toLowerCase();
  return !user || SYSTEM_USERS.includes(user) || /^(dwm|umfd)-\d+$/.test(user);
}

export function groupProcesses(procs: ProcessView[]): ProcessGroup[] {
  const map = new Map<string, ProcessGroup>();
  for (const p of procs) {
    const key = p.name.toLowerCase();
    let g = map.get(key);
    if (!g) {
      g = { key, name: p.name, procs: [], cpu: 0, memory: 0, disk: 0, protection: "none", user: p.user };
      map.set(key, g);
    }
    g.procs.push(p);
    g.cpu += p.cpu;
    g.memory += p.memory;
    g.disk += p.diskReadPerSec + p.diskWritePerSec;
    if (RANK[p.protection] > RANK[g.protection]) g.protection = p.protection;
    if (g.user !== p.user) g.user = g.user && p.user ? "varios" : (g.user ?? p.user);
  }
  for (const g of map.values()) g.procs.sort((a, b) => b.memory - a.memory);
  return [...map.values()];
}

/** Los procesos de un grupo que se pueden finalizar (ni críticos ni el propio AdminOps). */
export const killable = (g: ProcessGroup) => g.procs.filter((p) => p.protection !== "critical" && p.protection !== "self");
