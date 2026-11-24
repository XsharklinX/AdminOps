// Hardware → Conectados: cómo se ordenan y se resaltan los dispositivos que acaban de
// llegar o de irse. Lo que lee Windows y lo que traduce (estado, códigos, clases)
// está en Rust (connected.rs); aquí solo lo de la lista.
import type { ConnectedDevice, RecentChange } from "./api";

/** Lo recién conectado se resalta 2 minutos; lo recién quitado se ve 30 segundos, atenuado. */
export const HIGHLIGHT_MS = 2 * 60_000;
export const REMOVED_MS = 30_000;

export interface ConnectedRow {
  device: ConnectedDevice;
  /** Si llegó o se quitó hace poco: cuándo (ms desde 1970). */
  fresh?: { kind: "arrived" | "removed"; at: number };
}

/**
 * Las filas de la tabla: lo recién conectado arriba (lo último, primero), después lo recién
 * quitado, y el resto como llega (los problemas ya vienen delante).
 */
export function buildRows(devices: ConnectedDevice[], recent: RecentChange[], now: number): ConnectedRow[] {
  const arrived = new Map<string, number>();
  const removed = new Map<string, RecentChange>();
  for (const c of recent) {
    if (c.kind === "arrived" && now - c.at <= HIGHLIGHT_MS) arrived.set(c.device.id, c.at);
    if (c.kind === "removed" && now - c.at <= REMOVED_MS) removed.set(c.device.id, c);
  }
  const present = new Set(devices.map((d) => d.id));
  const top: ConnectedRow[] = [];
  const rest: ConnectedRow[] = [];
  for (const device of devices) {
    const at = arrived.get(device.id);
    if (at !== undefined) top.push({ device, fresh: { kind: "arrived", at } });
    else rest.push({ device });
  }
  top.sort((a, b) => b.fresh!.at - a.fresh!.at);
  const gone: ConnectedRow[] = [...removed.values()]
    .filter((c) => !present.has(c.device.id))
    .sort((a, b) => b.at - a.at)
    .map((c) => ({ device: c.device, fresh: { kind: "removed" as const, at: c.at } }));
  return [...top, ...gone, ...rest];
}

/** «hace 4 s», «hace 1 min». */
export function ago(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return s < 60 ? `hace ${s} s` : `hace ${Math.floor(s / 60)} min`;
}

/** «86 conectados · 1 sin driver · 2 con error» (solo lo que hay: sin ceros). */
export function summary(devices: ConnectedDevice[]): string {
  const noDriver = devices.filter((d) => d.status === "noDriver").length;
  const errors = devices.filter((d) => d.status === "error").length;
  const parts = [`${devices.length} ${devices.length === 1 ? "conectado" : "conectados"}`];
  if (noDriver) parts.push(`${noDriver} sin driver`);
  if (errors) parts.push(`${errors} con error`);
  return parts.join(" · ");
}

/** Qué se puede hacer con un dispositivo según su estado. */
export type DeviceAction = "scan" | "restart" | "enable";

export function actionsFor(d: ConnectedDevice): DeviceAction[] {
  if (d.status === "noDriver") return ["scan"];
  if (d.status === "error") return d.code === 22 ? ["enable"] : ["restart", "scan"];
  return [];
}

export const ACTION_LABEL: Record<DeviceAction, string> = {
  scan: "Buscar driver",
  restart: "Volver a detectar",
  enable: "Habilitar",
};
