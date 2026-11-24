// Red → Uso de la red: cómo se leen las cifras de cada programa. Lo que mide (ETW, bytes por
// proceso, Mbit/s, destinos, el total del día) está en Rust (netusage.rs); aquí, los estados
// («Satura la subida», «Fluida», «Pausado»), los textos de cada fila y el orden.
import { bytes, timeOfDay } from "./format";
import type { OneDrivePause, ProgramUsage, UsageSnapshot } from "./api";

/** Un programa que pasa de esta parte de la última prueba de velocidad satura la línea. */
export const SATURATION = 0.7;
/** Por debajo de esto, una dirección del tráfico no cuenta para decir «↑↓». */
export const QUIET_MBPS = 0.1;
/** El programa al que se le puede dar «Pausar 2 h». */
export const ONEDRIVE_KEY = "onedrive.exe";
/** Programas de videollamada: con la línea libre se dicen «Fluida». */
export const CALL_APP = /teams|zoom|meet|webex|skype/i;

export interface Speed {
  upMbps: number;
  downMbps: number;
}

/** La última prueba de velocidad guardada (el historial viene con la más reciente primero, pero no se fía de ello). */
export function lastSpeed(history: { timestamp: number; uploadMbps: number; downloadMbps: number }[]): Speed | null {
  let best: { timestamp: number; uploadMbps: number; downloadMbps: number } | null = null;
  for (const h of history) {
    if (!(h.uploadMbps > 0) && !(h.downloadMbps > 0)) continue;
    if (!best || h.timestamp > best.timestamp) best = h;
  }
  return best ? { upMbps: best.uploadMbps, downMbps: best.downloadMbps } : null;
}

/** «18,4» (una cifra decimal, con coma). */
export function mbit(n: number): string {
  return (Number.isFinite(n) ? n : 0).toLocaleString("es", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
}

/** «20» o «1,6»: sin decimales si no hacen falta. */
export function mbitShort(n: number): string {
  const r = Math.round((Number.isFinite(n) ? n : 0) * 10) / 10;
  return r.toLocaleString("es", { minimumFractionDigits: Number.isInteger(r) ? 0 : 1, maximumFractionDigits: 1 });
}

/** «↑ 18,4 Mbit/s», «↓ 2,1 Mbit/s» o «↑↓ 1,2 Mbit/s» cuando las dos pasan de 0,1. */
export function rateText(p: Pick<ProgramUsage, "upMbps" | "downMbps">): string {
  const up = p.upMbps > QUIET_MBPS;
  const down = p.downMbps > QUIET_MBPS;
  if (up && down) return `↑↓ ${mbit(p.upMbps + p.downMbps)} Mbit/s`;
  if (up) return `↑ ${mbit(p.upMbps)} Mbit/s`;
  if (down) return `↓ ${mbit(p.downMbps)} Mbit/s`;
  if (p.upMbps > 0 || p.downMbps > 0) return "menos de 0,1 Mbit/s";
  return "—";
}

/** «4 min», «50 s», «1 h 5 min». */
export function spanText(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  if (s < 60) return `${s} s`;
  if (s < 3600) return `${Math.floor(s / 60)} min`;
  const m = Math.floor((s % 3600) / 60);
  return m ? `${Math.floor(s / 3600)} h ${m} min` : `${Math.floor(s / 3600)} h`;
}

/** «1.4 GB», «38 MB»: una cifra decimal solo si no es cero. */
function size(n: number): string {
  return bytes(n, 1).replace(".0 ", " ");
}

/**
 * Lo subido desde que se mira, dicho con honradez: Windows no dice cuánto le falta a OneDrive por subir,
 * así que no se promete («subiendo 38 GB»), se cuenta lo que se vio: «38 MB subidos en 4 min».
 */
export function uploadedText(p: Pick<ProgramUsage, "upMbps" | "totalUp" | "watchedSecs">): string | null {
  if (p.upMbps <= QUIET_MBPS || p.totalUp < 1_000_000) return null;
  return `${size(p.totalUp)} subidos en ${spanText(p.watchedSecs)}`;
}

/** Lo de hoy, mientras AdminOps miraba: «↑ 1,2 GB · ↓ 340 MB». */
export function todayText(p: Pick<ProgramUsage, "todayUp" | "todayDown">): string {
  if (p.todayUp <= 0 && p.todayDown <= 0) return "—";
  return `↑ ${size(p.todayUp)} · ↓ ${size(p.todayDown)}`;
}

/** Los destinos con más tráfico, en una línea. */
export function destinationsText(p: Pick<ProgramUsage, "destinations">): string {
  return p.destinations.map((d) => d.host).join(" · ");
}

/** «En pausa hasta las 13:10». */
export function pausedText(untilMs: number): string {
  return `En pausa hasta las ${timeOfDay(new Date(untilMs))}`;
}

export type RowStateKind = "saturaSubida" | "saturaBajada" | "fluida" | "pausado";

export interface RowState {
  kind: RowStateKind;
  label: string;
  /** `bad` (rojo), `ok` (verde) o `mute` (gris). */
  tone: "bad" | "ok" | "mute";
}

const LABEL: Record<RowStateKind, string> = { saturaSubida: "Satura la subida", saturaBajada: "Satura la bajada", fluida: "Fluida", pausado: "Pausado" };
const TONE: Record<RowStateKind, RowState["tone"]> = { saturaSubida: "bad", saturaBajada: "bad", fluida: "ok", pausado: "mute" };

function state(kind: RowStateKind): RowState {
  return { kind, label: LABEL[kind], tone: TONE[kind] };
}

/** Qué satura el programa (la subida o la bajada, la que más pase), o nada. Sin prueba de velocidad guardada, nunca. */
export function saturates(p: Pick<ProgramUsage, "upMbps" | "downMbps">, speed: Speed | null): "up" | "down" | null {
  if (!speed) return null;
  const up = speed.upMbps > 0 ? p.upMbps / speed.upMbps : 0;
  const down = speed.downMbps > 0 ? p.downMbps / speed.downMbps : 0;
  if (up <= SATURATION && down <= SATURATION) return null;
  return up >= down ? "up" : "down";
}

export interface UsageRow {
  p: ProgramUsage;
  /** La pausa de OneDrive, si esta fila es la suya y está en pausa. */
  paused: OneDrivePause | null;
  state: RowState | null;
  /** La barra de uso bajo la fila que satura: el porcentaje de la línea que usa. */
  bar: { pct: number; dir: "up" | "down" } | null;
}

export interface UsageView {
  /** Con tráfico en los últimos 10 s (y OneDrive en pausa), en el orden que manda Rust. */
  rows: UsageRow[];
  /** Sin tráfico ahora (plegados). */
  quiet: ProgramUsage[];
  /** Subida libre de la línea con lo medido ahora, si hay prueba de velocidad. */
  freeUpMbps: number | null;
  hasSpeed: boolean;
}

const emptyProgram = (key: string, name: string, path: string | null): ProgramUsage => ({
  key,
  name,
  path,
  upMbps: 0,
  downMbps: 0,
  totalUp: 0,
  totalDown: 0,
  todayUp: 0,
  todayDown: 0,
  watchedSecs: 0,
  destinations: [],
  active: false,
});

/** Las filas de la pantalla a partir de lo medido y de la última prueba de velocidad. */
export function buildView(snap: UsageSnapshot, speed: Speed | null): UsageView {
  const pause = snap.onedrivePause;
  const programs = [...snap.programs];
  // OneDrive en pausa no mueve nada, pero su fila tiene que verse con «En pausa hasta…».
  if (pause && !programs.some((p) => p.key === ONEDRIVE_KEY)) programs.push(emptyProgram(ONEDRIVE_KEY, "OneDrive", null));
  const anySaturates = programs.some((p) => saturates(p, speed));
  const rows: UsageRow[] = [];
  const quiet: ProgramUsage[] = [];
  for (const p of programs) {
    const paused = pause && p.key === ONEDRIVE_KEY ? pause : null;
    if (!p.active && !paused) {
      quiet.push(p);
      continue;
    }
    const sat = paused ? null : saturates(p, speed);
    let st: RowState | null = null;
    let bar: UsageRow["bar"] = null;
    if (paused) st = state("pausado");
    else if (sat && speed) {
      st = state(sat === "up" ? "saturaSubida" : "saturaBajada");
      const frac = sat === "up" ? p.upMbps / speed.upMbps : p.downMbps / speed.downMbps;
      bar = { pct: Math.min(100, Math.round(frac * 100)), dir: sat };
    } else if (!anySaturates && speed && CALL_APP.test(p.name)) st = state("fluida");
    rows.push({ p, paused, state: st, bar });
  }
  const used = programs.reduce((a, p) => a + p.upMbps, 0);
  return { rows, quiet, freeUpMbps: speed && speed.upMbps > 0 ? Math.max(0, speed.upMbps - used) : null, hasSpeed: !!speed };
}

/** «Subida libre: 1,6 de 20 Mbit/s». */
export function freeUpText(freeMbps: number, speed: Speed): string {
  return `Subida libre: ${mbitShort(freeMbps)} de ${mbitShort(speed.upMbps)} Mbit/s`;
}

/** El resumen de arriba: «↑ 21,5 · ↓ 3,9 Mbit/s en total». */
export function totalText(snap: Pick<UsageSnapshot, "totalUpMbps" | "totalDownMbps">): string {
  return `↑ ${mbit(snap.totalUpMbps)} · ↓ ${mbit(snap.totalDownMbps)} Mbit/s en total`;
}
