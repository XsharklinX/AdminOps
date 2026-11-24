// Lógica de «Grabar el fallo» sin interfaz: tiempos, posición y color de las marcas, eje de la línea de tiempo.
import type { FaultInput, FaultMark, FaultSample, FaultMarkKind, FaultRecMonitor, FaultRecording, FaultRegion } from "./api";
import { timeOfDay } from "./format";

/** Lo más que dura una grabación. */
export const MAX_SECS = 120;
/** Al saltar a una marca se empieza unos segundos antes, para ver lo previo. */
export const LEAD_SECS = 2;

/** «0:41», «1:12»: minutos y segundos. */
export function clock(secs: number): string {
  const s = Math.max(0, Math.floor(Number.isFinite(secs) ? secs : 0));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** «45 s», «1 min 12 s», «2 min»: cuánto dura. */
export function lengthLabel(secs: number): string {
  const s = Math.max(0, Math.round(Number.isFinite(secs) ? secs : 0));
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  return s % 60 === 0 ? `${m} min` : `${m} min ${s % 60} s`;
}

/** Dónde cae una marca en la barra, de 0 a 100. Con una grabación de 0 s todo cae al principio. */
export function markPercent(t: number, secs: number): number {
  if (!(secs > 0)) return 0;
  return Math.min(100, Math.max(0, (t / secs) * 100));
}

/** El segundo al que salta el vídeo al pulsar una marca: dos segundos antes (nunca antes del principio). */
export function seekTime(t: number): number {
  return Math.max(0, t - LEAD_SECS);
}

/** Cada tipo, con su nombre y su color: disco y procesador en ámbar, «no responde» y errores en rojo. */
export const KIND_LABEL: Record<FaultMarkKind, string> = {
  disk: "Disco",
  cpu: "Procesador",
  hang: "No responde",
  error: "Error de Windows",
};

/** Las clases van escritas enteras para que Tailwind las encuentre. */
export const KIND_STYLE: Record<FaultMarkKind, { dot: string; text: string }> = {
  disk: { dot: "bg-warn", text: "text-warn" },
  cpu: { dot: "bg-warn", text: "text-warn" },
  hang: { dot: "bg-bad", text: "text-bad" },
  error: { dot: "bg-bad", text: "text-bad" },
};

/** «0:24 · Disco al 100 %». */
export function markLine(m: Pick<FaultMark, "t" | "title">): string {
  return `${clock(m.t)} · ${m.title}`;
}

/** Las marcas por orden de aparición (sin tocar la lista original). */
export function sortedMarks(marks: FaultMark[]): FaultMark[] {
  return [...marks].sort((a, b) => a.t - b.t);
}

/** Cada cuántos segundos va una marca del eje, según lo larga que sea la grabación. */
export function axisStep(secs: number): number {
  if (secs <= 20) return 5;
  if (secs <= 40) return 10;
  if (secs <= 90) return 15;
  return 30;
}

/** Las marcas del eje: «0:00», «0:15»…, con su posición. */
export function axisTicks(secs: number): { t: number; pct: number; label: string }[] {
  if (!(secs > 0)) return [{ t: 0, pct: 0, label: clock(0) }];
  const step = axisStep(secs);
  const out: { t: number; pct: number; label: string }[] = [];
  for (let t = 0; t <= secs; t += step) out.push({ t, pct: markPercent(t, secs), label: clock(t) });
  return out;
}

/** «Hoy 10:42», «Ayer 18:05» o «3 oct 09:30». */
export function whenLabel(startedSecs: number, now: Date = new Date()): string {
  const d = new Date(startedSecs * 1000);
  if (Number.isNaN(d.getTime())) return "";
  const dayStart = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((dayStart(now) - dayStart(d)) / 86_400_000);
  const time = timeOfDay(startedSecs);
  if (days === 0) return `Hoy ${time}`;
  if (days === 1) return `Ayer ${time}`;
  const date = d.toLocaleDateString("es", { day: "numeric", month: "short", ...(d.getFullYear() !== now.getFullYear() ? { year: "numeric" as const } : {}) });
  return `${date} ${time}`;
}

/** «3 marcas», «1 marca», «sin marcas». */
export function marksCount(n: number): string {
  return n === 0 ? "sin marcas" : n === 1 ? "1 marca" : `${n} marcas`;
}

/** Cómo se llama un monitor en el selector: «Monitor principal» o «Monitor 2 · DELL U2419H». */
export function monitorLabel(m: FaultRecMonitor, total: number): string {
  if (m.primary) return total > 1 ? `Monitor principal · ${m.name}` : "Monitor principal";
  return `Monitor ${m.number} · ${m.name}`;
}

/** Lo que dice la fila «Se borra»: con caso abierto, al cerrarlo; sin caso, al cerrar AdminOps. */
export function eraseWhen(hasCase: boolean): string {
  return hasCase ? "al cerrar el caso" : "al cerrar AdminOps";
}

/** Lo que dice el contador flotante. */
export function pillText(secs: number): string {
  return `Grabando ${clock(secs)}`;
}

/** Cuánto queda hasta el corte automático. */
export function remaining(secs: number, max = MAX_SECS): number {
  return Math.max(0, max - Math.max(0, Math.floor(secs)));
}

/** La grabación más reciente de una lista (o la que se pide si está). */
export function pickRecording(list: FaultRecording[], id?: string | null): FaultRecording | null {
  if (id) {
    const found = list.find((r) => r.id === id);
    if (found) return found;
  }
  return [...list].sort((a, b) => b.started - a.started)[0] ?? null;
}

// ---------- Qué se graba, clics y teclas, voz (1.4, G2) ----------

export type RecTarget = "screen" | "window" | "region";

/** Lo que se eligió la última vez (el botón de arriba lo usa también). */
export interface RecOptions {
  target: RecTarget;
  /** Apuntar los clics y las teclas. De serie, sí. */
  input: boolean;
  /** Grabar el micrófono. De serie, no. */
  mic: boolean;
}

const OPTIONS_KEY = "adminops.faultrec.options";
export const DEFAULT_REC_OPTIONS: RecOptions = { target: "screen", input: true, mic: false };

export function getRecOptions(): RecOptions {
  try {
    const v = JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? "null") as Partial<RecOptions> | null;
    if (!v || typeof v !== "object") return { ...DEFAULT_REC_OPTIONS };
    return {
      target: v.target === "window" || v.target === "region" ? v.target : "screen",
      input: typeof v.input === "boolean" ? v.input : DEFAULT_REC_OPTIONS.input,
      mic: typeof v.mic === "boolean" ? v.mic : DEFAULT_REC_OPTIONS.mic,
    };
  } catch {
    return { ...DEFAULT_REC_OPTIONS };
  }
}

export function setRecOptions(o: RecOptions) {
  try {
    localStorage.setItem(OPTIONS_KEY, JSON.stringify(o));
  } catch {
    /* sin almacenamiento: vale para esta vez */
  }
}

/** Lo más pequeña que puede ser una zona (píxeles físicos). */
export const MIN_REGION = 64;

/**
 * El rectángulo que se dibuja arrastrando, de dos esquinas en píxeles de la página a píxeles físicos del
 * monitor (`dpr`), dentro de `maxW`×`maxH` y con tamaño par. Null si es más pequeño que MIN_REGION.
 */
export function dragRegion(a: { x: number; y: number }, b: { x: number; y: number }, dpr: number, maxW: number, maxH: number): FaultRegion | null {
  const k = dpr > 0 && Number.isFinite(dpr) ? dpr : 1;
  const x0 = Math.max(0, Math.round(Math.min(a.x, b.x) * k));
  const y0 = Math.max(0, Math.round(Math.min(a.y, b.y) * k));
  const x1 = Math.min(maxW, Math.round(Math.max(a.x, b.x) * k));
  const y1 = Math.min(maxH, Math.round(Math.max(a.y, b.y) * k));
  const w = (x1 - x0) & ~1;
  const h = (y1 - y0) & ~1;
  if (w < MIN_REGION || h < MIN_REGION) return null;
  return { x: x0, y: y0, w, h };
}

/** «Zona de 800 × 600». */
export function regionLabel(r: Pick<FaultRegion, "w" | "h">): string {
  return `Zona de ${r.w} × ${r.h}`;
}

/** «0:18 · Ctrl + P». */
export function inputLine(e: Pick<FaultInput, "t" | "label">): string {
  return `${clock(e.t / 1000)} · ${e.label}`;
}

/** Cuánto se ve cada cosa sobre el vídeo. */
export const CLICK_SHOW_MS = 700;
export const KEYS_SHOW_MS = 1500;

/** Lo que se pinta encima del vídeo en el milisegundo `ms`: los clics recientes (con lo que llevan, de 0 a 1) y las últimas teclas. */
export function inputOverlay(events: FaultInput[], ms: number): { clicks: { e: FaultInput; age: number }[]; keys: FaultInput | null } {
  const clicks: { e: FaultInput; age: number }[] = [];
  let keys: FaultInput | null = null;
  for (const e of events) {
    if (e.t > ms) continue;
    const since = ms - e.t;
    if (e.kind === "click" && since <= CLICK_SHOW_MS) clicks.push({ e, age: since / CLICK_SHOW_MS });
    if (e.kind === "keys" && since <= KEYS_SHOW_MS && (!keys || e.t >= keys.t)) keys = e;
  }
  return { clicks, keys };
}

/** Dónde se ve la imagen dentro del reproductor (con `object-fit: contain` sobran franjas a los lados o arriba y abajo). */
export function videoBox(boxW: number, boxH: number, vidW: number, vidH: number): { left: number; top: number; width: number; height: number } {
  if (!(boxW > 0 && boxH > 0 && vidW > 0 && vidH > 0)) return { left: 0, top: 0, width: Math.max(0, boxW), height: Math.max(0, boxH) };
  const k = Math.min(boxW / vidW, boxH / vidH);
  const width = vidW * k;
  const height = vidH * k;
  return { left: (boxW - width) / 2, top: (boxH - height) / 2, width, height };
}

// ---------- Vigilar (1.4, G3) ----------

/** Lo que se guarda con «¡Acaba de pasar!». */
export const REPLAY_SECS = 60;

/** Cuánto del último minuto hay ya en memoria, de 0 a 100. */
export function bufferPercent(secs: number): number {
  if (!(secs > 0)) return 0;
  return Math.min(100, (secs / REPLAY_SECS) * 100);
}

/** «Vigilando desde las 9:15». */
export function watchSince(since: number | null): string {
  return since ? `Vigilando desde las ${timeOfDay(since)}` : "Vigilando";
}

/** De dónde salió una grabación, para la lista: «Guardada sola · Excel no responde», «El último minuto» o nada. */
export function originLabel(r: Pick<FaultRecording, "origin" | "trigger">): string {
  if (r.origin === "auto") return r.trigger ? `Guardada sola · ${r.trigger}` : "Guardada sola";
  if (r.origin === "replay") return "El último minuto";
  return "";
}

// ---------- Reproductor con pistas, recortar y lista (1.4, G4) ----------

/** Una pista del reproductor. */
export interface Track {
  key: "cpu" | "disk" | "mem" | "net";
  label: string;
  /** Un valor por segundo (null si no se midió). */
  values: (number | null)[];
  /** El tope del eje: 100 en porcentajes; en la red, lo más alto que hubo. */
  max: number;
  /** Cómo se lee un valor: «34 %», «1,2 MB/s». */
  format: (v: number) => string;
}

/** «1,2 MB/s», «340 KB/s», «0 KB/s». */
export function rateLabel(bytesPerSec: number): string {
  const b = Math.max(0, bytesPerSec || 0);
  if (b >= 1024 * 1024) return `${(b / 1024 / 1024).toLocaleString("es", { maximumFractionDigits: 1 })} MB/s`;
  return `${Math.round(b / 1024)} KB/s`;
}

const pct = (v: number) => `${Math.round(v)} %`;

/** Las cuatro pistas medidas, alineadas por segundo con el vídeo (las que no tienen ningún dato no salen). */
export function tracksOf(samples: FaultSample[], secs: number): Track[] {
  const n = Math.max(1, Math.ceil(secs));
  const at = (pick: (s: FaultSample) => number | null) => {
    const v: (number | null)[] = Array(n).fill(null);
    for (const s of samples) if (s.t >= 0 && s.t < n) v[s.t] = pick(s);
    return v;
  };
  const net = at((s) => s.net);
  const netMax = Math.max(1, ...net.map((x) => x ?? 0));
  const all: Track[] = [
    { key: "cpu", label: "Procesador", values: at((s) => s.cpu), max: 100, format: pct },
    { key: "disk", label: "Disco", values: at((s) => s.disk), max: 100, format: pct },
    { key: "mem", label: "Memoria", values: at((s) => s.mem), max: 100, format: pct },
    { key: "net", label: "Red", values: net, max: netMax, format: rateLabel },
  ];
  return all.filter((t) => t.values.some((v) => v !== null));
}

/** El área de una pista para un SVG de `n`×`h` (viewBox): sube y baja con cada segundo; los huecos van a cero. */
export function areaPath(values: (number | null)[], max: number, h = 24): string {
  if (!values.length) return "";
  const y = (v: number | null) => {
    const k = max > 0 ? Math.min(1, Math.max(0, (v ?? 0) / max)) : 0;
    return +(h - k * h).toFixed(2);
  };
  let d = `M0 ${h}`;
  values.forEach((v, i) => {
    d += ` L${i} ${y(v)} L${i + 1} ${y(v)}`;
  });
  return `${d} L${values.length} ${h} Z`;
}

/** «0:13 → 0:27 · 14 s». */
export function rangeLabel(fromSecs: number, toSecs: number): string {
  const a = Math.min(fromSecs, toSecs);
  const b = Math.max(fromSecs, toSecs);
  return `${clock(a)} → ${clock(b)} · ${lengthLabel(b - a)}`;
}

/** Qué pasó en una grabación, para la lista: el programa que falló, lo que la disparó o la primera marca. */
export function recSummary(r: Pick<FaultRecording, "marks" | "trigger" | "origin">): { what: string; probable: string | null } {
  const marks = sortedMarks(r.marks);
  const what = marks.find((m) => m.app)?.title ?? (r.origin === "auto" ? r.trigger : null) ?? marks[0]?.title ?? "Sin marcas";
  const p = marks.find((m) => m.probable);
  return { what, probable: p ? `Causa probable: ${p.title.charAt(0).toLowerCase()}${p.title.slice(1)}` : null };
}

/** La línea que «Al caso» añade a las notas del caso. */
export function caseNote(r: Pick<FaultRecording, "started" | "marks" | "trigger" | "origin">, fromSecs: number, toSecs: number): string {
  const inside = sortedMarks(r.marks).filter((m) => m.t >= Math.floor(fromSecs) && m.t <= Math.ceil(toSecs));
  const what = inside.find((m) => m.app)?.title ?? inside[0]?.title ?? (r.origin === "auto" ? r.trigger : null);
  const cause = inside.find((m) => m.probable);
  const parts = [`Grabación del fallo (${whenLabel(r.started + Math.floor(fromSecs))}, ${rangeLabel(fromSecs, toSecs)})`];
  if (what) parts.push(what);
  if (cause && cause.title !== what) parts.push(`causa probable: ${cause.title.charAt(0).toLowerCase()}${cause.title.slice(1)}`);
  return `${parts.join(": ").replace(/: ([^:]+): /, ": $1; ")}.`;
}

/** Junta una serie larga en `n` tramos como mucho, con el valor más alto de cada uno (la voz: 720 décimas en 160 barras). */
export function bucketMax(values: number[], n: number): number[] {
  if (values.length <= n || n <= 0) return [...values];
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = Math.floor((i * values.length) / n);
    const b = Math.max(a + 1, Math.floor(((i + 1) * values.length) / n));
    out.push(Math.max(...values.slice(a, b)));
  }
  return out;
}
