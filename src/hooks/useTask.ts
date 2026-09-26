import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

/**
 * Progreso de las tareas largas del backend (evento `task-progress`).
 * Un único listener global; cada componente se suscribe a su clave de tarea.
 */
const messages = new Map<string, string>();
const subscribers = new Set<() => void>();
let started = false;

function start() {
  if (started) return;
  started = true;
  listen<{ task: string; message: string }>("task-progress", (e) => {
    messages.set(e.payload.task, e.payload.message);
    subscribers.forEach((fn) => fn());
  }).catch(() => {
    started = false;
  });
}

/** Último mensaje de progreso de `task` mientras `active`; se limpia al terminar. */
export function useTaskMessage(task: string | null, active: boolean): string | null {
  const [, force] = useState(0);
  useEffect(() => {
    start();
    const fn = () => force((n) => n + 1);
    subscribers.add(fn);
    return () => {
      subscribers.delete(fn);
    };
  }, []);
  useEffect(() => {
    if (task && !active) messages.delete(task);
  }, [task, active]);
  return task && active ? (messages.get(task) ?? null) : null;
}
