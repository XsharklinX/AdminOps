// Marca «Nuevo» en la barra lateral: tras actualizar, señala las pantallas y
// secciones que cambiaron desde la versión que este técnico tenía. Se va al
// entrar. Quien instala por primera vez no ve ninguna: para él todo es nuevo.
import { useSyncExternalStore } from "react";
import { RELEASES } from "./changelog";

/** En qué versión cambió cada pantalla o sección (clave de `navKey`). Se añade al publicar. */
export const NEW_IN: Record<string, string> = {
  "space:space": "1.2.3",
  dashboard: "1.2.4",
  "machine:performance": "1.2.4",
};

/** Solo se señala lo de las últimas versiones: lo de hace un año ya no es novedad. */
const KEEP_RELEASES = 3;

const FIRST_KEY = "adminops.firstVersion";
const SEEN_KEY = "adminops.newSeen";
/** La clave con la que la app recuerda la última versión abierta (App.tsx). */
const LAST_OPENED_KEY = "adminops-seen-version";

export function newerThan(a: string, b: string): boolean {
  const pa = a.split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if ((pa[i] ?? 0) !== (pb[i] ?? 0)) return (pa[i] ?? 0) > (pb[i] ?? 0);
  }
  return false;
}

/** Las claves que hay que señalar para quien empezó en `first` y ya entró en `seen`. */
export function pendingMarks(first: string, seen: string[], recent: string[] = RELEASES.slice(0, KEEP_RELEASES).map((r) => r.version)): string[] {
  return Object.entries(NEW_IN)
    .filter(([key, version]) => recent.includes(version) && newerThan(version, first) && !seen.includes(key))
    .map(([key]) => key);
}

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* sin almacenamiento: las marcas se verán otra vez, sin más */
  }
}

/** La versión con la que este técnico empezó a usar AdminOps en este equipo. */
function firstVersion(): string {
  const saved = read(FIRST_KEY);
  if (saved) return saved;
  // Si ya la había abierto antes de existir esto, viene de una versión anterior:
  // le toca ver las marcas. Si no, es una instalación nueva y no ve ninguna.
  const first = read(LAST_OPENED_KEY) ? "0" : RELEASES[0].version;
  write(FIRST_KEY, first);
  return first;
}

let marks: string[] | null = null;
const listeners = new Set<() => void>();

function current(): string[] {
  if (!marks) {
    let seen: string[];
    try {
      seen = JSON.parse(read(SEEN_KEY) ?? "[]") as string[];
    } catch {
      seen = [];
    }
    marks = pendingMarks(firstVersion(), Array.isArray(seen) ? seen : []);
  }
  return marks;
}

/** Al entrar en una pantalla o sección, su marca desaparece. */
export function markSeen(keys: string[]) {
  const now = current();
  const left = now.filter((k) => !keys.includes(k));
  if (left.length === now.length) return;
  marks = left;
  write(SEEN_KEY, JSON.stringify(Object.keys(NEW_IN).filter((k) => !left.includes(k))));
  listeners.forEach((l) => l());
}

/** Las claves que llevan la marca «Nuevo» ahora mismo. */
export function useNewMarks(): string[] {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    current,
    current,
  );
}
