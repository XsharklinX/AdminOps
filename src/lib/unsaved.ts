// Recuperar archivos → Sin guardar de Office: cómo se dice cada fila y qué se puede hacer con ella.
// Lo que encuentra (carpetas de autorrecuperación, temporales, versiones anteriores) está en Rust (unsaved.rs).
import type { UnsavedFile } from "./api";
import { ago, timeOfDay } from "./format";

const DAY = 86_400_000;

/** «hoy 11:52», «ayer 18:03» o «12 nov, 10:20». */
export function whenText(modified: number, now = Date.now()): string {
  if (!modified) return "fecha desconocida";
  const t = modified * 1000;
  const startOfToday = new Date(now);
  startOfToday.setHours(0, 0, 0, 0);
  const clock = timeOfDay(modified);
  if (t >= startOfToday.getTime()) return `hoy ${clock}`;
  if (t >= startOfToday.getTime() - DAY) return `ayer ${clock}`;
  return `${new Date(t).toLocaleDateString("es", { day: "numeric", month: "short" })}, ${clock}`;
}

/** «Hace 18 min» (con la primera en mayúscula). */
export function agoText(modified: number, now = Date.now()): string {
  if (!modified) return "";
  const s = ago(modified, now);
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** «Presupuesto 2027.xlsx · Autorrecuperación · hoy 11:52 · Hace 18 min». */
export function rowLine(f: UnsavedFile, now = Date.now()): string {
  return [f.name, f.origin, whenText(f.modified, now), agoText(f.modified, now)].filter(Boolean).join(" · ");
}

/** Lo más reciente primero (a igual hora, por nombre). */
export function sortUnsaved(files: UnsavedFile[]): UnsavedFile[] {
  return [...files].sort((a, b) => b.modified - a.modified || a.name.localeCompare(b.name, "es"));
}

/** Qué se dice cuando no hay nada en la lista. */
export const EMPTY_UNSAVED = "Office no dejó nada sin guardar en este equipo: ni autorrecuperación, ni documentos sin guardar, ni temporales.";

/** Las versiones anteriores: qué decir según lo que hay. */
export function versionsSentence(snapshots: number, versions: number): string | null {
  if (snapshots === 0) return "Windows no tiene copias de versiones anteriores en esa unidad.";
  if (versions === 0) return snapshots === 1 ? "El archivo no está en la única copia de esa unidad." : `El archivo no está en ninguna de las ${snapshots} copias de esa unidad.`;
  return null;
}

/** ¿Se abrirá la copia con su programa? Si no se sabe qué tipo es, solo se enseña dónde queda. */
export function canOpenCopy(f: UnsavedFile): boolean {
  return f.copyExt !== null;
}
