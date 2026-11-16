// Problemas de este equipo (problems.rs) puestos en columnas, y la historia de
// cada uno: lo que guarda el diagnóstico más lo que se probó (el Historial).
import type { JournalEntry, Problem } from "./api";

/** Lo que se probó para un problema: arreglos suyos aplicados desde que apareció. */
export function attempts(p: Problem, journal: JournalEntry[]): JournalEntry[] {
  const until = p.resolvedAt ?? Infinity;
  return journal.filter((j) => j.tweakId && p.fixes.includes(j.tweakId) && j.timestamp >= p.firstSeen - 60 && j.timestamp <= until + 60);
}

/** Abiertos (no se ha probado nada), en curso (se probó algo y sigue) y resueltos. */
export function problemColumns(list: Problem[], journal: JournalEntry[], _now: number) {
  const open: Problem[] = [];
  const working: Problem[] = [];
  const solved: Problem[] = [];
  for (const p of list) {
    if (p.resolvedAt) solved.push(p);
    else if (attempts(p, journal).length > 0) working.push(p);
    else open.push(p);
  }
  const bySeverity = (a: Problem, b: Problem) => (a.severity === b.severity ? b.lastSeen - a.lastSeen : a.severity === "bad" ? -1 : 1);
  return { open: open.sort(bySeverity), working: working.sort(bySeverity), solved: solved.sort((a, b) => (b.resolvedAt ?? 0) - (a.resolvedAt ?? 0)) };
}

export interface StoryLine {
  at: number;
  text: string;
  tone: "ok" | "warn" | "bad" | "info";
}

/** La historia de un problema, en orden. */
export function problemStory(p: Problem, journal: JournalEntry[]): StoryLine[] {
  const lines: StoryLine[] = p.events.map((e) => ({
    at: e.at,
    text: e.kind === "detected" ? "Apareció en el diagnóstico" : e.kind === "resolved" ? "Dejó de salir: resuelto" : "Volvió a aparecer",
    tone: e.kind === "resolved" ? "ok" : e.kind === "reopened" ? "bad" : "warn",
  }));
  for (const j of attempts(p, journal)) lines.push({ at: j.timestamp, text: `Probado: ${j.title}${j.ok ? "" : " (falló)"}`, tone: j.ok ? "info" : "bad" });
  return lines.sort((a, b) => a.at - b.at);
}
