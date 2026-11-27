import type { RocketBoost } from "./api/rocket";
import { bytes } from "./format";

const MB = 1024 * 1024;

/** La frase de después de optimizar: lo que se notó, sin cifras de relleno. */
export function rocketSummary(b: RocketBoost): { title: string; line: string; sub: string } {
  const parts: string[] = [];
  // Menos de 50 MB de memoria es ruido de la medida: no se presume de ello.
  if (b.ramFreed >= 50 * MB) parts.push(`${bytes(b.ramFreed)} de memoria liberados`);
  if (b.tempFreed >= MB) parts.push(`${bytes(b.tempFreed)} de temporales borrados`);
  else if (b.tempFiles > 0) parts.push(b.tempFiles === 1 ? "1 temporal borrado" : `${b.tempFiles} temporales borrados`);
  const secs = Math.max(1, Math.round(b.ms / 1000));
  const sub = [`Sin cerrar nada · ${secs} s`];
  if (b.tempSkipped > 0) sub.push(`${b.tempSkipped} en uso, se quedan`);
  if (!parts.length) return { title: "Listo", line: "El equipo ya estaba ligero", sub: sub.join(" · ") };
  return { title: "Listo", line: parts.join(" y "), sub: sub.join(" · ") };
}

/** Se ve la animación entera aunque la limpieza termine antes (y no parece que no hizo nada). */
export const ROCKET_MIN_MS = 2400;
/** Cuánto se queda el resultado a la vista antes de plegarse. */
export const ROCKET_RESULT_MS = 6500;

/** Memoria en uso → color del anillo: verde, ámbar desde el 80 %, rojo desde el 90 %. */
export function memLevel(pct: number): "ok" | "warn" | "bad" {
  return pct >= 90 ? "bad" : pct >= 80 ? "warn" : "ok";
}

/** Dos minutos seguidos por encima del 90 %: el cohete late y pide que lo toquen. */
export const NUDGE_AFTER_MS = 120_000;
export function shouldNudge(highSince: number | null, now: number): boolean {
  return highSince !== null && now - highSince >= NUDGE_AFTER_MS;
}

/** Tamaño de cada panel al lado de la burbuja (px de la página, antes del tamaño elegido). */
export const BUBBLE = 68;
export const PANEL_W = 344;
export const PANEL_H = { result: 68, nudge: 68, auto: 68, error: 68, hung: 100, menu: 232, deep: 222, details: 236, call: 170 } as const;
export type PanelKind = keyof typeof PANEL_H;

/** Sumas por línea del desglose: la barra más larga es la del que más soltó. */
export function share(n: number, max: number): number {
  return max <= 0 ? 0 : Math.max(4, Math.round((n / max) * 100));
}
