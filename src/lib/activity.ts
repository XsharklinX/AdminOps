// Lo que AdminOps ha terminado en esta sesión (análisis, copias, instalaciones):
// la pestaña «Actividad» de la campana. Solo de hoy y en memoria; lo que cambia
// el equipo queda además en el Historial.
import { listen } from "@tauri-apps/api/event";
import { useSyncExternalStore } from "react";
import type { PageId } from "../components/Sidebar";

export interface Activity {
  task: string;
  name: string;
  seconds: number;
  cancelled: boolean;
  /** Cuándo terminó (ms). */
  at: number;
  seen: boolean;
}

/** Las muy cortas (leer algo, buscar en la red) no se apuntan. */
const MIN_SECONDS = 3;
const KEEP = 40;

/** Dónde se ve el resultado de cada tarea: por la clave, o por cómo empieza. */
const WHERE: [string, PageId, string | null][] = [
  ["diagnostics", "machine", "diagnostics"],
  ["disk-", "space", "health"],
  ["drivers-", "machine", "diagnostics"],
  ["software", "apps", "update"],
  ["update", "apps", "update"],
  ["wu-search", "apps", "winupdate"],
  ["lan-", "router", "devices"],
  ["uninstall:", "apps", "uninstall"],
  ["install-apps", "apps", "install"],
  ["tweak:", "machine", "history"],
  ["repair:", "machine", "history"],
  ["profile:", "machine", "history"],
  ["restore-point", "machine", "history"],
  ["session", "session", "session"],
];

/** La pantalla donde mirar el resultado de una tarea, si se sabe. */
export function resultPage(task: string): { page: PageId; section: string | null } | null {
  const hit = WHERE.find(([key]) => (key.endsWith(":") || key.endsWith("-") ? task.startsWith(key) : task === key));
  return hit ? { page: hit[1], section: hit[2] } : null;
}

/** «2 min 14 s», «48 s». */
export function took(seconds: number): string {
  return seconds >= 60 ? `${Math.floor(seconds / 60)} min ${seconds % 60} s` : `${seconds} s`;
}

/** Añade una tarea terminada a la lista (la más reciente, primero). */
export function withFinished(list: Activity[], done: Omit<Activity, "seen">): Activity[] {
  if (done.seconds < MIN_SECONDS && !done.cancelled) return list;
  return [{ ...done, seen: false }, ...list].slice(0, KEEP);
}

let items: Activity[] = [];
let started = false;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());

function start() {
  if (started) return;
  started = true;
  void listen<{ task: string; name: string; seconds: number; cancelled: boolean }>("task-finished", ({ payload }) => {
    const next = withFinished(items, { ...payload, at: Date.now() });
    if (next !== items) {
      items = next;
      emit();
    }
  });
}

export function markActivitySeen() {
  if (!items.some((a) => !a.seen)) return;
  items = items.map((a) => (a.seen ? a : { ...a, seen: true }));
  emit();
}

export function clearActivity() {
  items = [];
  emit();
}

const get = () => items;

export function useActivity(): Activity[] {
  return useSyncExternalStore((l) => {
    start();
    listeners.add(l);
    return () => listeners.delete(l);
  }, get, get);
}
