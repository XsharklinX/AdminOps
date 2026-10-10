// Biblioteca compartida entre técnicos: soluciones, plantillas de texto,
// recetas, reglas de alerta y la plantilla de informe en un solo archivo, con
// autor, versión y fecha. Al importar se ve qué es nuevo y qué cambia lo que ya
// tienes; lo tuyo no se pisa salvo que se diga.
import type { AlertRule, Recipe, ReportLayout, Solution, TextTemplate } from "./api";

export const PACK_FORMAT = "adminops-biblioteca";

export interface Pack {
  format: typeof PACK_FORMAT;
  name: string;
  author: string;
  version: string;
  created: number;
  solutions: Solution[];
  templates: TextTemplate[];
  recipes: Recipe[];
  alertRules: AlertRule[];
  reportLayout: ReportLayout | null;
}

export type PackKind = "solutions" | "templates" | "recipes" | "alertRules";
export const PACK_KINDS: { kind: PackKind; label: string }[] = [
  { kind: "solutions", label: "Soluciones" },
  { kind: "templates", label: "Plantillas de texto" },
  { kind: "recipes", label: "Recetas" },
  { kind: "alertRules", label: "Reglas de alerta" },
];

/** Lo que importa de cada cosa para saber si cambió (sin fechas ni contadores de uso). */
function essence(x: unknown): string {
  const { uses, lastUsed, created, updated, ...rest } = x as Record<string, unknown>;
  void uses;
  void lastUsed;
  void created;
  void updated;
  return JSON.stringify(rest);
}

export interface Diff<T> {
  added: T[];
  changed: T[];
  same: T[];
}

export function diff<T extends { id: string }>(mine: T[], theirs: T[]): Diff<T> {
  const out: Diff<T> = { added: [], changed: [], same: [] };
  for (const t of theirs) {
    const m = mine.find((x) => x.id === t.id);
    if (!m) out.added.push(t);
    else if (essence(m) !== essence(t)) out.changed.push(t);
    else out.same.push(t);
  }
  return out;
}

/** Lee un archivo de biblioteca; null si no lo es. */
export function parsePack(text: string): Pack | null {
  try {
    const p = JSON.parse(text) as Partial<Pack>;
    if (p.format !== PACK_FORMAT) return null;
    return {
      format: PACK_FORMAT,
      name: String(p.name ?? "Biblioteca"),
      author: String(p.author ?? ""),
      version: String(p.version ?? "1"),
      created: Number(p.created ?? 0),
      solutions: Array.isArray(p.solutions) ? p.solutions : [],
      templates: Array.isArray(p.templates) ? p.templates : [],
      recipes: Array.isArray(p.recipes) ? p.recipes : [],
      alertRules: Array.isArray(p.alertRules) ? p.alertRules : [],
      reportLayout: p.reportLayout ?? null,
    };
  } catch {
    return null;
  }
}

/** Sube la versión «3» → «4», «1.2» → «1.3». */
export function bump(version: string): string {
  const parts = version.split(".");
  const last = Number(parts[parts.length - 1]);
  if (!Number.isFinite(last)) return `${version}.1`;
  parts[parts.length - 1] = String(last + 1);
  return parts.join(".");
}
