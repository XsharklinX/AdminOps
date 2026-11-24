// Estado del equipo → Pantallas: cómo se decide si un monitor va bien, cómo se dice en una
// línea y cómo se dibuja la disposición. Lo que lee Windows (resolución, Hz, cable, la resolución
// propia del monitor, la escala) está en Rust (displays.rs); aquí solo la lógica de la pantalla.
import type { DisplayMonitor } from "./api";

export type MonitorState = "ok" | "blurry" | "oddScale" | "lowHz";

/** Las escalas que ofrece Windows (Configuración → Pantalla → Escala). */
export const STANDARD_SCALES = [100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500];

/** ¿Va a su resolución propia? Sin saber cuál es, se da por buena. Vale también girado (vertical). */
export function isNative(m: DisplayMonitor): boolean {
  if (m.nativeWidth === null || m.nativeHeight === null) return true;
  return (m.width === m.nativeWidth && m.height === m.nativeHeight) || (m.width === m.nativeHeight && m.height === m.nativeWidth);
}

/** Lo que pasa con el monitor, de lo más claro a lo más raro. */
export function monitorState(m: DisplayMonitor): MonitorState {
  if (!isNative(m)) return "blurry";
  if (!STANDARD_SCALES.includes(m.scale)) return "oddScale";
  if (m.hz > 0 && m.hz < 50 && m.maxHz !== null && m.maxHz > m.hz) return "lowHz";
  return "ok";
}

export const STATE_LABEL: Record<MonitorState, string> = {
  ok: "Bien",
  blurry: "Borroso (no nativa)",
  oddScale: "Escala rara",
  lowHz: "Hz bajos",
};

/** La frase que explica el estado (null si está bien). */
export function stateSentence(m: DisplayMonitor): string | null {
  switch (monitorState(m)) {
    case "blurry":
      return "Va a una resolución que no es la suya: por eso se ve borroso y estirado.";
    case "oddScale":
      return `La escala es ${m.scale} %, que no es una de las habituales de Windows: los textos pueden verse borrosos.`;
    case "lowHz":
      return `Va a ${m.hz} Hz y admite hasta ${m.maxHz} Hz: el movimiento se ve a saltos. Prueba otro cable o puerto.`;
    default:
      return null;
  }
}

/** «LG 24MK430 · 1280×1024 · su nativa es 1920×1080 · HDMI · 60 Hz». */
export function monitorLine(m: DisplayMonitor): string {
  const parts = [m.name, `${m.width}×${m.height}`];
  if (!isNative(m)) parts.push(`su nativa es ${m.nativeWidth}×${m.nativeHeight}`);
  parts.push(m.connection);
  if (m.hz > 0) parts.push(`${m.hz} Hz`);
  return parts.join(" · ");
}

/** El texto del botón que pone la resolución propia, o null si no hace falta o no se sabe cuál es. */
export function nativeFixLabel(m: DisplayMonitor): string | null {
  if (isNative(m) || m.nativeWidth === null || m.nativeHeight === null) return null;
  return `Poner ${m.nativeWidth}×${m.nativeHeight}`;
}

/** La resolución pedida de un monitor como la espera el comando equivalente («1920x1080»). */
export function nativeSize(m: DisplayMonitor): string | null {
  return m.nativeWidth !== null && m.nativeHeight !== null ? `${m.nativeWidth}x${m.nativeHeight}` : null;
}

export interface Box {
  id: string;
  number: number;
  primary: boolean;
  state: MonitorState;
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Los rectángulos de la disposición, a escala y en su posición real, centrados en un espacio de
 * `width × height` con `margin` alrededor. La escala es la misma para todos: lo que cabe de la disposición entera.
 */
export function layoutBoxes(monitors: DisplayMonitor[], width: number, height: number, margin = 8): Box[] {
  const shown = monitors.filter((m) => m.width > 0 && m.height > 0);
  if (!shown.length || width <= margin * 2 || height <= margin * 2) return [];
  const minX = Math.min(...shown.map((m) => m.x));
  const minY = Math.min(...shown.map((m) => m.y));
  const maxX = Math.max(...shown.map((m) => m.x + m.width));
  const maxY = Math.max(...shown.map((m) => m.y + m.height));
  const spanW = maxX - minX;
  const spanH = maxY - minY;
  const scale = Math.min((width - margin * 2) / spanW, (height - margin * 2) / spanH);
  const offX = (width - spanW * scale) / 2;
  const offY = (height - spanH * scale) / 2;
  return shown.map((m) => ({
    id: m.id,
    number: m.number,
    primary: m.primary,
    state: monitorState(m),
    left: offX + (m.x - minX) * scale,
    top: offY + (m.y - minY) * scale,
    width: m.width * scale,
    height: m.height * scale,
  }));
}

/** Segundos que faltan (redondeados hacia arriba, nunca negativos). */
export function secondsLeft(untilMs: number, now: number): number {
  return Math.max(0, Math.ceil((untilMs - now) / 1000));
}

/** Cuántos monitores van mal (para el resumen de arriba). */
export function problemCount(monitors: DisplayMonitor[]): number {
  return monitors.filter((m) => monitorState(m) !== "ok").length;
}

/** «2 monitores · 1 con algo raro». */
export function summary(monitors: DisplayMonitor[], duplicated: boolean): string {
  const bad = problemCount(monitors);
  const parts = [`${monitors.length} ${monitors.length === 1 ? "monitor" : "monitores"}`];
  if (duplicated) parts.push("duplicados");
  if (bad) parts.push(`${bad} con algo raro`);
  return parts.join(" · ");
}
