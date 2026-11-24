// Ctrl+K que hace las cosas: «desbloquear maria», «reiniciar PC-CONTA-03»,
// «cola recepción», «laps legal-04», «liberar espacio».
//
// Ninguna acción existe solo aquí: son los mismos comandos que los botones de
// Personas, Puestos, Impresoras y Limpieza. Lo que cambia algo pide Intro dos
// veces (la confirmación va dentro de la misma caja) y respeta el modo
// auditoría. Entender la frase es de recognize.ts (parseAction).
import { useEffect, useState } from "react";
import { auditApi, measureApi, peopleApi, printersApi, stationsApi, tweaksApi, type OpResult } from "./api";
import { parseAction, parseSteps, type ActionIntent, type ActionStep } from "./recognize";

export interface QuickResult {
  ok: boolean;
  text: string;
  /** Si la acción se puede deshacer: lo que hace Ctrl+Z. */
  undo?: () => Promise<string>;
}

export interface QuickEntry {
  key: string;
  title: string;
  subtitle: string;
  /** Cambia algo en un equipo o cuenta: pide confirmar y no corre en modo auditoría. */
  changes: boolean;
  /** Lo que se escribió, para volver a lanzarla desde «Hace poco». */
  query: string;
  run: () => Promise<QuickResult>;
  /** Nombre corto para la lista de pasos («Liberar espacio»). Sin él, el título. */
  short?: string;
  /** Detalle corto a la derecha del paso («7 trabajos»), si se sabe. */
  detail?: string;
}

/** El resultado de una acción del catálogo con su «antes → después» si midió algo. */
async function withMeasure(r: OpResult): Promise<string> {
  if (!r.measure) return r.message;
  const m = await measureApi.text(r.measure).catch(() => null);
  return m && !r.message.includes(m) ? `${r.message} · ${m}` : r.message;
}

export const AUDIT_TEXT = "Modo auditoría activo: AdminOps no hace cambios. Desactívalo en el menú «⋯» para continuar.";

const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Todas las palabras de `q` aparecen en `name` (sin tildes ni mayúsculas). */
export function nameMatches(name: string, q: string): boolean {
  const n = norm(name);
  const words = norm(q).split(/\s+/).filter(Boolean);
  return words.length > 0 && words.every((w) => n.includes(w));
}

async function lastUndoableEntry(since: number, tweakId: string): Promise<number | null> {
  const j = await tweaksApi.journal().catch(() => []);
  const ms = (t: number) => (t < 1e12 ? t * 1000 : t);
  const e = j.filter((x) => x.undoable && x.ok && !x.reverted && x.op !== "revert" && x.tweakId === tweakId && ms(x.timestamp) >= since - 2000).sort((a, b) => b.timestamp - a.timestamp)[0];
  return e ? e.id : null;
}

/** Las acciones que ofrece una frase. Cada una llama al mismo comando que su botón. */
export async function buildEntries(intent: ActionIntent, query: string): Promise<QuickEntry[]> {
  const entry = (key: string, title: string, subtitle: string, changes: boolean, run: QuickEntry["run"], extra: Pick<QuickEntry, "short" | "detail"> = {}): QuickEntry => ({ key: `quick:${key}`, title, subtitle, changes, query, run, ...extra });
  switch (intent.action) {
    case "unlock": {
      const hits = (await peopleApi.search(intent.arg)).sort((a, b) => Number(b.lockedHint) - Number(a.lockedHint)).slice(0, 4);
      return hits.map((h) => {
        const name = h.name || h.sam;
        return entry(
          `unlock:${h.sam}`,
          `Desbloquear a ${name}`,
          h.disabled ? "Cuenta desactivada: desbloquear no la activa" : h.lockedHint ? "Tuvo un bloqueo · queda en el diario" : "Sin bloqueo conocido · queda en el diario",
          true,
          async () => {
            await peopleApi.unlock(h.sam);
            return { ok: true, text: `Cuenta de ${name} desbloqueada · queda en el diario` };
          },
          { detail: h.sam },
        );
      });
    }
    case "restart": {
      const host = intent.arg.toUpperCase();
      return [
        entry(`restart:${host}`, `Reiniciar ${host}`, "Quien lo use verá un aviso y tendrá 2 minutos para guardar · se puede cancelar", true, async () => {
          const [r] = await stationsApi.act([host], "restart");
          if (!r) return { ok: false, text: `${host}: sin respuesta` };
          return {
            ok: r.ok,
            text: `${host}: ${r.detail}`,
            undo: r.ok
              ? async () => {
                  const [c] = await stationsApi.act([host], "cancelRestart");
                  return `${host}: ${c?.detail ?? "Reinicio cancelado."}`;
                }
              : undefined,
          };
        }, { short: `Reiniciar ${host}`, detail: "aviso de 2 min" }),
      ];
    }
    case "queue": {
      const all = (await printersApi.list()).filter((p) => !p.virtual);
      const jobsText = (n: number) => (n > 0 ? `${n} ${n === 1 ? "trabajo" : "trabajos"} en la cola` : "La cola está vacía ahora mismo");
      // Sin nombre de impresora: todas las colas.
      if (!intent.arg) {
        if (all.length === 0) return [];
        const total = all.reduce((t, p) => t + p.jobs, 0);
        return [
          entry("queue:all", "Vaciar la cola de impresión", all.length === 1 ? jobsText(total) : `Todas las impresoras (${all.length}) · ${jobsText(total)}`, true, async () => {
            const failed: string[] = [];
            for (const p of all) {
              try {
                await printersApi.clearQueue(p.name);
              } catch (err) {
                failed.push(`${p.name}: ${err}`);
              }
            }
            if (failed.length) return { ok: false, text: `No se pudo vaciar la cola. ${failed.join(" · ")}` };
            return { ok: true, text: all.length === 1 ? `Cola de ${all[0].name} vaciada` : `Colas de ${all.length} impresoras vaciadas` };
          }, { detail: total > 0 ? `${total} ${total === 1 ? "trabajo" : "trabajos"}` : "vacía" }),
        ];
      }
      const list = all.filter((p) => nameMatches(p.name, intent.arg)).slice(0, 4);
      return list.map((p) =>
        entry(`queue:${p.name}`, `Vaciar la cola · ${p.name}`, jobsText(p.jobs), true, async () => {
          await printersApi.clearQueue(p.name);
          return { ok: true, text: `Cola de ${p.name} vaciada` };
        }, { short: `Vaciar la cola · ${p.name}`, detail: p.jobs > 0 ? `${p.jobs} ${p.jobs === 1 ? "trabajo" : "trabajos"}` : "vacía" }),
      );
    }
    case "explorer":
      return [
        entry("explorer", "Reiniciar el Explorador", "Barra de tareas y escritorio desaparecen unos segundos · es el botón del Panel", true, async () => {
          const r = await tweaksApi.run("repair.explorer");
          return { ok: true, text: await withMeasure(r) };
        }, { detail: "se cierra unos segundos" }),
      ];
    case "laps": {
      const host = intent.arg.toUpperCase();
      return [
        entry(`laps:${host}`, `Ver LAPS · ${host}`, "Copia la contraseña del administrador local · no se enseña en pantalla", false, async () => {
          const r = await peopleApi.laps(host);
          if (!r.found) return { ok: false, text: `${host}: no hay contraseña LAPS guardada` };
          await navigator.clipboard.writeText(r.password);
          return { ok: true, text: `Contraseña LAPS de ${host} copiada (cuenta ${r.account})` };
        }),
      ];
    }
    case "space":
      return [
        entry("space", "Liberar espacio · temporales del usuario", "Es el botón «Limpiar los temporales del usuario» de Limpieza", true, async () => {
          const started = Date.now();
          const r = await tweaksApi.run("cleanup.user-temp");
          const id = await lastUndoableEntry(started, "cleanup.user-temp");
          return { ok: true, text: await withMeasure(r), undo: id == null ? undefined : () => tweaksApi.revertEntry(id).then((x) => x.message) };
        }, { short: "Liberar espacio", detail: "temporales del usuario" }),
      ];
  }
}

/** Ejecuta una acción: comprueba el modo auditoría y devuelve siempre un resultado (también el error). */
export async function execute(e: QuickEntry): Promise<QuickResult> {
  try {
    if (e.changes && (await auditApi.get().catch(() => false))) return { ok: false, text: AUDIT_TEXT };
    const r = await e.run();
    if (r.ok) {
      rememberRecent({ label: e.title, query: e.query });
      rememberPhrase(e.query);
    }
    return r;
  } catch (err) {
    return { ok: false, text: String(err) };
  }
}

// ---------- Varios pasos ----------

export type StepState = "wait" | "run" | "ok" | "bad";

/** La primera acción que no puede correr porque el modo auditoría está activo, o null. */
export async function auditBlocked(entries: QuickEntry[]): Promise<QuickEntry | null> {
  const changing = entries.filter((e) => e.changes);
  if (changing.length === 0) return null;
  const audit = await auditApi.get().catch(() => false);
  return audit ? changing[0] : null;
}

export const auditBlockedText = (e: QuickEntry) => `No se puede: ${e.short ?? e.title} está bloqueada en modo auditoría`;

/**
 * Hace los pasos uno tras otro. Si uno falla, siguen los demás. `onStep` avisa de
 * cada cambio de estado para pintar el círculo girando, el ✓ o la ✕.
 */
export async function runSteps(entries: QuickEntry[], onStep: (i: number, state: StepState, result?: QuickResult) => void): Promise<QuickResult[]> {
  const results: QuickResult[] = [];
  for (let i = 0; i < entries.length; i++) {
    onStep(i, "run");
    const r = await execute(entries[i]);
    results.push(r);
    onStep(i, r.ok ? "ok" : "bad", r);
  }
  return results;
}

/** «{ok} de {n} hechos». */
export const stepsSummary = (results: QuickResult[]) => `${results.filter((r) => r.ok).length} de ${results.length} hechos`;

/** Ctrl+Z tras varios pasos: deshace los que se pueden, del último al primero, y dice cuántos. */
export async function undoSteps(results: QuickResult[]): Promise<QuickResult> {
  let done = 0;
  const failed: string[] = [];
  for (const r of [...results].reverse()) {
    if (!r.ok || !r.undo) continue;
    try {
      await r.undo();
      done++;
    } catch (err) {
      failed.push(String(err));
    }
  }
  const text = done === 1 ? "Deshecho 1 paso" : `Deshechos ${done} pasos`;
  return failed.length ? { ok: false, text: `${text} · No se pudo deshacer: ${failed.join(" · ")}` } : { ok: true, text };
}

// ---------- Últimas frases (flecha arriba con la caja vacía) ----------

const PHRASES_KEY = "adminops.palette.lastPhrases";
const MAX_PHRASES = 20;

export function loadPhrases(): string[] {
  try {
    const v: unknown = JSON.parse(localStorage.getItem(PHRASES_KEY) ?? "[]");
    return Array.isArray(v) ? (v as unknown[]).filter((p): p is string => typeof p === "string" && p.trim() !== "").slice(0, MAX_PHRASES) : [];
  } catch {
    return [];
  }
}

/** La más reciente primero, sin repetir, hasta 20. */
export function addPhrase(list: string[], phrase: string): string[] {
  const p = phrase.trim();
  if (!p) return list.slice(0, MAX_PHRASES);
  return [p, ...list.filter((x) => x !== p)].slice(0, MAX_PHRASES);
}

export function rememberPhrase(phrase: string) {
  try {
    localStorage.setItem(PHRASES_KEY, JSON.stringify(addPhrase(loadPhrases(), phrase)));
  } catch {
    /* sin almacenamiento */
  }
}

// ---------- Hace poco ----------

export interface Recent {
  label: string;
  query: string;
}

const RECENT_KEY = "adminops.palette.recent";
const MAX_RECENT = 5;

export function loadRecent(): Recent[] {
  try {
    const v: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(v) ? (v as Recent[]).filter((r) => r && typeof r.label === "string" && typeof r.query === "string").slice(0, MAX_RECENT) : [];
  } catch {
    return [];
  }
}

/** La más reciente primero, sin repetir. */
export function addRecent(list: Recent[], r: Recent): Recent[] {
  return [r, ...list.filter((x) => x.query !== r.query || x.label !== r.label)].slice(0, MAX_RECENT);
}

function rememberRecent(r: Recent) {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(addRecent(loadRecent(), r)));
  } catch {
    /* sin almacenamiento */
  }
}

/**
 * Las acciones de lo que se está escribiendo. Solo se devuelven si son de esta
 * misma frase: así Intro nunca actúa sobre el resultado de una búsqueda anterior.
 */
const NONE: QuickEntry[] = [];

export function useQuickEntries(query: string, enabled: boolean): QuickEntry[] {
  const [found, setFound] = useState<{ query: string; entries: QuickEntry[] }>({ query: "", entries: [] });
  useEffect(() => {
    const intent = enabled ? parseAction(query) : null;
    if (!intent) return;
    let alive = true;
    const t = window.setTimeout(
      () => {
        buildEntries(intent, query)
          .then((entries) => alive && setFound({ query, entries }))
          .catch(() => alive && setFound({ query, entries: [] }));
      },
      intent.action === "unlock" ? 250 : 80,
    );
    return () => {
      alive = false;
      window.clearTimeout(t);
    };
  }, [query, enabled]);
  return enabled && found.query === query && parseAction(query) && !parseSteps(query) ? found.entries : NONE;
}

/** Un paso de la frase y lo que se hará: una sola acción, o el motivo de que no se pueda. */
export interface StepPlan {
  step: ActionStep;
  entry: QuickEntry | null;
  problem: string | null;
}

/** Cómo queda un paso según lo que se encontró: una sola coincidencia, ninguna o varias. */
export function planStep(step: ActionStep, entries: QuickEntry[]): StepPlan {
  if (entries.length === 1) return { step, entry: entries[0], problem: null };
  return { step, entry: null, problem: entries.length === 0 ? `«${step.text}»: no encuentro a qué se refiere` : `«${step.text}»: hay ${entries.length} coincidencias, concreta` };
}

/**
 * Varias acciones en la misma frase. `steps` sale al momento; `plans`, cuando se
 * ha buscado a qué se refiere cada una (null mientras tanto y si la frase cambió).
 */
export function useQuickSteps(query: string, enabled: boolean): { steps: ActionStep[]; plans: StepPlan[] | null } | null {
  const [found, setFound] = useState<{ query: string; plans: StepPlan[] }>({ query: "", plans: [] });
  const steps = enabled ? parseSteps(query) : null;
  useEffect(() => {
    const parsed = enabled ? parseSteps(query) : null;
    if (!parsed) return;
    let alive = true;
    const t = window.setTimeout(
      () => {
        Promise.all(parsed.map((s) => buildEntries(s.intent, query).then((e) => planStep(s, e), (err) => ({ step: s, entry: null, problem: `«${s.text}»: ${err}` }) as StepPlan)))
          .then((plans) => alive && setFound({ query, plans }))
          .catch(() => alive && setFound({ query, plans: [] }));
      },
      parsed.some((s) => s.intent.action === "unlock") ? 250 : 80,
    );
    return () => {
      alive = false;
      window.clearTimeout(t);
    };
  }, [query, enabled]);
  if (!steps) return null;
  return { steps, plans: found.query === query ? found.plans : null };
}
