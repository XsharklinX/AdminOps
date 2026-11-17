// Ctrl+K que hace las cosas: «desbloquear maria», «reiniciar PC-CONTA-03»,
// «cola recepción», «laps legal-04», «liberar espacio».
//
// Ninguna acción existe solo aquí: son los mismos comandos que los botones de
// Personas, Puestos, Impresoras y Limpieza. Lo que cambia algo pide Intro dos
// veces (la confirmación va dentro de la misma caja) y respeta el modo
// auditoría. Entender la frase es de recognize.ts (parseAction).
import { useEffect, useState } from "react";
import { auditApi, peopleApi, printersApi, stationsApi, tweaksApi } from "./api";
import { parseAction, type ActionIntent } from "./recognize";

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
  const entry = (key: string, title: string, subtitle: string, changes: boolean, run: QuickEntry["run"]): QuickEntry => ({ key: `quick:${key}`, title, subtitle, changes, query, run });
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
        }),
      ];
    }
    case "queue": {
      const list = (await printersApi.list()).filter((p) => !p.virtual && nameMatches(p.name, intent.arg)).slice(0, 4);
      return list.map((p) =>
        entry(`queue:${p.name}`, `Vaciar la cola · ${p.name}`, p.jobs > 0 ? `${p.jobs} ${p.jobs === 1 ? "trabajo" : "trabajos"} en la cola` : "La cola está vacía ahora mismo", true, async () => {
          await printersApi.clearQueue(p.name);
          return { ok: true, text: `Cola de ${p.name} vaciada` };
        }),
      );
    }
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
          return { ok: true, text: r.message, undo: id == null ? undefined : () => tweaksApi.revertEntry(id).then((x) => x.message) };
        }),
      ];
  }
}

/** Ejecuta una acción: comprueba el modo auditoría y devuelve siempre un resultado (también el error). */
export async function execute(e: QuickEntry): Promise<QuickResult> {
  try {
    if (e.changes && (await auditApi.get().catch(() => false))) return { ok: false, text: AUDIT_TEXT };
    const r = await e.run();
    if (r.ok) rememberRecent({ label: e.title, query: e.query });
    return r;
  } catch (err) {
    return { ok: false, text: String(err) };
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
  return enabled && found.query === query && parseAction(query) ? found.entries : NONE;
}
