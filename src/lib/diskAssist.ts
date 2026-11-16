// Asistente para un disco con errores: qué falla de verdad, en qué orden se
// prueba y el veredicto. Lógica pura (se prueba sin disco): recibe lo que ya se
// lee (SMART, tendencia, errores por disco) y devuelve el plan.
import type { DiskReport, JournalEntry } from "./api";

/** Errores de un disco por tipo, como los devuelve `disk_error_history`. */
export interface DiskErrors {
  disk: number | null;
  total: number;
  byKind: Partial<Record<ErrorKind, number>>;
  days: [string, number][];
  lastDay: string;
}

export type ErrorKind = "surface" | "retry" | "connection" | "controller" | "fileSystem";

export const KIND_LABEL: Record<ErrorKind, string> = {
  surface: "Superficie (bloques que no se leen)",
  retry: "Reintentos de lectura o escritura",
  connection: "Conexión (la controladora no respondió o el disco se fue)",
  controller: "Reinicios de la controladora",
  fileSystem: "Sistema de archivos dañado",
};

/** Lo que de verdad falla, que es lo que decide qué hacer. */
export type Cause = "surface" | "connection" | "power" | "filesystem" | "none";

export interface Evidence {
  label: string;
  value: string;
  tone: "ok" | "warn" | "bad";
}

/** Un paso del plan. `action` dice qué botón lleva. */
export interface Step {
  id: string;
  title: string;
  detail: string;
  action:
    | { kind: "goto"; target: "rescue" | "clone" | "drivers" }
    | { kind: "tweak"; tweak: string }
    | { kind: "check" | "repair"; letter: string }
    | { kind: "scan"; mode: "quick" | "full" }
    | { kind: "manual" };
}

export interface Assessment {
  cause: Cause;
  /** Una frase que dice qué falla. */
  headline: string;
  why: string;
  evidence: Evidence[];
  plan: Step[];
  verdict: { level: "ok" | "warn" | "bad"; title: string; text: string };
}

const n = (v: number | null | undefined) => (typeof v === "number" && v > 0 ? v : 0);

export function assess(d: DiskReport, errs: DiskErrors | undefined, controllerResets = 0): Assessment {
  const usb = d.bus.toUpperCase() === "USB";
  const k = errs?.byKind ?? {};
  const pending = n(d.pending);
  const uncorrectable = n(d.uncorrectable);
  const reallocated = n(d.reallocated);
  const crc = n(d.crcErrors);
  const rising = new Set(d.rising.map((r) => r.toLowerCase()));
  const reallocRising = [...rising].some((r) => r.includes("apart") || r.includes("realloc") || r.includes("reasign"));
  const crcRising = [...rising].some((r) => r.includes("conexi") || r.includes("crc"));

  const evidence: Evidence[] = [];
  const add = (label: string, value: string | number, tone: Evidence["tone"]) => evidence.push({ label, value: String(value), tone });
  if (d.pending !== null) add("Sectores pendientes", pending, pending ? "bad" : "ok");
  if (d.uncorrectable !== null) add("Sectores que no se pudieron leer", uncorrectable, uncorrectable ? "bad" : "ok");
  if (d.reallocated !== null) add("Sectores apartados", reallocRising ? `${reallocated}, y suben` : reallocated, reallocRising ? "bad" : reallocated > 50 ? "warn" : "ok");
  if (d.crcErrors !== null) add("Errores de transmisión (cable o puerto)", crcRising ? `${crc}, y suben` : crc, crcRising ? "bad" : crc ? "warn" : "ok");
  if (n(d.mediaErrors)) add("Errores del medio (NVMe)", d.mediaErrors, "bad");
  for (const kind of Object.keys(KIND_LABEL) as ErrorKind[]) {
    const c = n(k[kind]);
    if (c) add(`${KIND_LABEL[kind]} · 30 días`, c, kind === "surface" ? "bad" : "warn");
  }
  if (controllerResets) add("Reinicios de la controladora (todos los discos) · 30 días", controllerResets, "warn");

  const surface = pending > 0 || uncorrectable > 0 || reallocRising || n(k.surface) > 0 || n(d.mediaErrors) > 0;
  const connection = crcRising || crc > 0 || n(k.connection) > 0;
  const retries = n(k.retry) > 0 || controllerResets > 0;
  const dirty = d.volumes.some((v) => v.dirty);
  const fs = n(k.fileSystem) > 0 || dirty;
  const cause: Cause = surface ? "surface" : connection ? "connection" : retries ? "power" : fs ? "filesystem" : "none";

  const letters = d.volumes.filter((v) => v.letter).map((v) => v.letter);
  const firstLetter = letters[0] ?? "";
  const lpm = usb
    ? { kind: "tweak" as const, tweak: "performance.usb-suspend" }
    : { kind: "tweak" as const, tweak: "performance.disk-link-power" };

  let plan: Step[];
  let headline: string;
  let why: string;
  let verdict: Assessment["verdict"];

  switch (cause) {
    case "surface": {
      const failing = pending > 0 || uncorrectable > 0 || reallocRising || n(d.mediaErrors) > 0;
      headline = "Es el disco: su superficie falla";
      why = "Hay zonas que no se leen. Ningún arreglo de Windows lo cura; cada lectura lo fuerza un poco más. Lo primero son los datos.";
      plan = [
        { id: "rescue", title: "Copiar lo importante a otro disco", detail: "Documentos, escritorio y lo que el cliente necesite, antes que nada: lo que se lee hoy puede no leerse mañana.", action: { kind: "goto", target: "rescue" } },
        { id: "clone", title: "Hacer una imagen completa con reintentos", detail: "Copia el disco entero sin forzarlo: lo legible primero y lo difícil al final. Se puede parar y seguir.", action: { kind: "goto", target: "clone" } },
        { id: "scan", title: "Probar la superficie entera", detail: "Con los datos ya a salvo, para saber cuánto disco queda bien. Solo lee. Tarda horas en un disco grande: mejor de noche.", action: { kind: "scan", mode: "full" } },
        { id: "replace", title: "Cambiar el disco", detail: d.media === "HDD" ? "Por un SSD: además de fiable, el equipo irá mucho más rápido." : "Por uno nuevo de la misma capacidad o más.", action: { kind: "manual" } },
      ];
      verdict = failing
        ? { level: "bad", title: "Cambiar el disco", text: "No se puede seguir usando con garantías: tiene zonas ilegibles y van a más. Saca los datos y cámbialo." }
        : { level: "warn", title: "Vigilar y tener copia", text: "Tuvo bloques dañados, pero ahora están quietos. Puede aguantar; con copia de seguridad y revisándolo cada mes." };
      break;
    }
    case "connection":
      headline = usb ? "Es la conexión: el disco USB se desconecta o el cable falla" : "Es la conexión: el cable, el puerto o la controladora";
      why = "Los datos se estropean por el camino, no en el disco. El disco puede estar perfecto: se arregla con un cable, otro puerto o un ajuste.";
      plan = [
        { id: "cable", title: usb ? "Probar otro cable USB y otro puerto (mejor uno trasero)" : "Cambiar el cable SATA y probar otro puerto de la placa", detail: "Es lo más habitual. Después, el contador de errores de transmisión no debe subir más.", action: { kind: "manual" } },
        { id: "power", title: usb ? "Que Windows no apague los USB" : "Quitar el ahorro de energía del enlace SATA", detail: "Evita que el enlace se duerma y vuelva con errores. Se deshace con un clic.", action: lpm },
        { id: "drivers", title: "Actualizar el driver de la controladora", detail: "Un driver viejo de la controladora (Intel RST, AMD SATA) da errores de conexión con discos sanos.", action: { kind: "goto", target: "drivers" } },
      ];
      verdict = { level: "warn", title: "El disco puede estar bien", text: "Los errores vienen de la conexión. Tras cambiar el cable o el ajuste, si el contador deja de subir, está resuelto." };
      break;
    case "power":
      headline = "Windows tiene que repetir operaciones, pero la superficie está bien";
      why = "Suele ser el ahorro de energía del enlace o un driver de la controladora. Se prueba un arreglo y se mira si los errores paran.";
      plan = [
        { id: "power", title: usb ? "Que Windows no apague los USB" : "Quitar el ahorro de energía del enlace SATA", detail: "La causa más habitual de «operación reintentada» con discos sanos. Se deshace con un clic.", action: lpm },
        { id: "idle", title: "Que los discos no se apaguen tras un rato sin uso", detail: "Con discos mecánicos viejos, despertarlos da errores y esperas.", action: { kind: "tweak", tweak: "performance.disk-idle" } },
        { id: "drivers", title: "Actualizar el driver de la controladora", detail: "Si los errores siguen tras los dos ajustes, suele ser el driver.", action: { kind: "goto", target: "drivers" } },
        { id: "scan", title: "Si nada funciona, probar la superficie", detail: "Para descartar el disco. Solo lee.", action: { kind: "scan", mode: "quick" } },
      ];
      verdict = { level: "warn", title: "Probablemente se arregla sin cambiar nada", text: "Aplica el primer ajuste y deja el equipo funcionando: aquí mismo se verá si los errores paran." };
      break;
    case "filesystem":
      headline = "El índice de archivos está dañado, no el disco";
      why = "Suele pasar por apagar de golpe o quitar un disco sin expulsarlo. Se repara sin tocar lo que está bien.";
      plan = [
        { id: "check", title: "Comprobar el sistema de archivos", detail: "Solo mira, no cambia nada.", action: { kind: "check", letter: firstLetter } },
        { id: "repair", title: "Repararlo", detail: "En el disco de Windows se hace al reiniciar.", action: { kind: "repair", letter: firstLetter } },
      ];
      verdict = { level: "warn", title: "Se repara", text: "No hay señales de que el disco falle. Tras reparar el sistema de archivos, debería quedar bien." };
      break;
    default:
      headline = "No hay señales de fallo";
      why = "SMART está bien y Windows no ha apuntado errores de este disco en los últimos 30 días.";
      plan = [];
      verdict = { level: "ok", title: "Sano", text: d.verdict.text };
  }
  return { cause, headline, why, evidence, plan, verdict };
}

/** Errores por día antes y después de un arreglo (para «¿funcionó?»). */
export interface Effect {
  tweak: string;
  title: string;
  at: number;
  beforePerDay: number;
  afterPerDay: number;
  /** Días que han pasado desde el arreglo. */
  daysAfter: number;
  verdict: "works" | "no-change" | "too-soon";
}

const DAY = 86_400;
const dayKey = (t: number) => new Date(t * 1000).toISOString().slice(0, 10);

/**
 * Compara los errores de los 7 días antes de cada arreglo de disco con los que
 * ha habido desde entonces. `now` en segundos.
 */
export function effects(errs: DiskErrors | undefined, journal: JournalEntry[], now: number, tweaks: string[]): Effect[] {
  const days = new Map(errs?.days ?? []);
  const sum = (from: number, to: number) => {
    let s = 0;
    for (let t = from; t < to; t += DAY) s += days.get(dayKey(t)) ?? 0;
    return s;
  };
  return journal
    .filter((j) => j.ok && !j.reverted && j.op === "apply" && j.tweakId && tweaks.includes(j.tweakId))
    .map((j) => {
      const daysAfter = Math.max(0, (now - j.timestamp) / DAY);
      const before = sum(j.timestamp - 7 * DAY, j.timestamp) / 7;
      const after = daysAfter >= 1 ? sum(j.timestamp, now) / daysAfter : 0;
      const verdict: Effect["verdict"] = daysAfter < 1 ? "too-soon" : before > 0 && after <= before * 0.2 ? "works" : "no-change";
      return { tweak: j.tweakId!, title: j.title, at: j.timestamp, beforePerDay: before, afterPerDay: after, daysAfter, verdict };
    });
}

/** Los tres arreglos de disco del catálogo. */
export const DISK_TWEAKS = ["performance.disk-link-power", "performance.usb-suspend", "performance.disk-idle"];
