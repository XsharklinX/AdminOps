// Memoria de cada pantalla durante la sesión: filtros, búsquedas, orden y pestaña.
//
// Una pantalla que se descarta (solo siguen vivas las últimas) y se vuelve a abrir
// sale con lo que tenía, y el historial (Alt+←) guarda y devuelve esto mismo.
// Es como `useState`, pero el valor se guarda por pantalla y nombre.
import { useCallback, useEffect, useRef, useState } from "react";
import { usePageId } from "./sectionState";

const SEP = "\u0000";
const store = new Map<string, unknown>();
const listeners = new Set<(id: string) => void>();

const idOf = (page: string, key: string) => `${page}${SEP}${key}`;

/** Lo guardado para una pantalla y un nombre (o `fallback`). */
export function memGet<T>(page: string, key: string, fallback: T): T {
  const id = idOf(page, key);
  return store.has(id) ? (store.get(id) as T) : fallback;
}

/** Guarda un valor para una pantalla y un nombre. */
export function memSet(page: string, key: string, value: unknown) {
  store.set(idOf(page, key), value);
}

/** Todo lo guardado de una pantalla, para el historial. */
export function memSnapshot(page: string): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  const prefix = `${page}${SEP}`;
  for (const [id, v] of store) if (id.startsWith(prefix)) out[id.slice(prefix.length)] = v;
  return out;
}

/** Deja la memoria de una pantalla tal como estaba en `snap` y avisa a lo que la esté mostrando. */
export function memRestore(page: string, snap: Record<string, unknown>) {
  const prefix = `${page}${SEP}`;
  const touched = new Set<string>();
  for (const id of [...store.keys()]) {
    if (id.startsWith(prefix) && !(id.slice(prefix.length) in snap)) {
      store.delete(id);
      touched.add(id);
    }
  }
  for (const [k, v] of Object.entries(snap)) {
    store.set(idOf(page, k), v);
    touched.add(idOf(page, k));
  }
  touched.forEach((id) => listeners.forEach((l) => l(id)));
}

/** Nombres de los valores que cuentan como «el filtro» de una pantalla en el botón de volver. */
const LABEL_KEYS = ["filter", "q"];
const LABEL_MAX = 18;

/** El filtro de una pantalla en pocas palabras («Lento»), o null si no hay o es largo. */
export function memLabel(snap: Record<string, unknown>): string | null {
  for (const k of LABEL_KEYS) {
    const v = snap[k];
    if (typeof v !== "string") continue;
    const t = v.trim();
    if (t && t !== "all" && t.length <= LABEL_MAX) return t;
  }
  return null;
}

/** Vacía la memoria (pruebas). */
export function memClear() {
  store.clear();
}

/**
 * Como `useState`, pero el valor se recuerda por pantalla durante la sesión:
 * al volver a la pantalla (por la barra lateral o con «atrás») sale como estaba.
 * `key` distingue varios valores dentro de la misma pantalla. Los nombres
 * `filter` y `q` salen en el botón de volver de la cabecera.
 */
export function usePageMemory<T>(key: string, initial: T | (() => T)): [T, (v: T | ((prev: T) => T)) => void] {
  const page = usePageId() ?? "_";
  const first = useRef<{ v: T } | null>(null);
  if (!first.current) first.current = { v: typeof initial === "function" ? (initial as () => T)() : initial };
  const [value, setValue] = useState<T>(() => memGet(page, key, first.current!.v));
  const latest = useRef(value);
  latest.current = value;
  const set = useCallback(
    (v: T | ((prev: T) => T)) => {
      const next = typeof v === "function" ? (v as (p: T) => T)(latest.current) : v;
      latest.current = next;
      memSet(page, key, next);
      setValue(next);
    },
    [page, key],
  );
  // «Atrás» o «adelante» devuelven la pantalla a como estaba.
  useEffect(() => {
    const id = idOf(page, key);
    const on = (changed: string) => {
      if (changed !== id) return;
      const next = memGet(page, key, first.current!.v);
      latest.current = next;
      setValue(next);
    };
    listeners.add(on);
    return () => void listeners.delete(on);
  }, [page, key]);
  return [value, set];
}
