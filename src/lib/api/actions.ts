// «Ver el comando», las medidas de antes y después de cada arreglo y el plan que
// sigue solo después de reiniciar (Preparar equipos).
import { invoke } from "./core";
import type { Measure } from "./system";

/** El comando equivalente de una acción (el PowerShell, comentado en español). */
export interface ActionCommand {
  id: string;
  name: string;
  script: string;
  /** AdminOps lo hace con funciones de Windows: esto es lo equivalente, no lo mismo. */
  approximate: boolean;
  admin: boolean;
  /** Qué mide al ejecutarse (espacio, cola, ping…), si mide algo. */
  measure: string | null;
  /** Nombre propuesto para «Guardar como .ps1». */
  fileName: string;
}

export const actionsApi = {
  /** `context`: lo que rellena el comando (la impresora, el programa del inicio). */
  command: (id: string, context?: string) => invoke<ActionCommand>("action_command", { id, context: context ?? null }),
  /** Guarda el comando como .ps1. El nombre del archivo, o null si se cancela. */
  saveScript: (id: string, context?: string) => invoke<string | null>("action_save_script", { id, context: context ?? null }),
};

export const measureApi = {
  /** Lee una medida suelta («espacio», «inicio», «cola:HP»…). Null si no se pudo. */
  read: (spec: string) => invoke<number | null>("measure_read", { spec }),
  /** «C: 3,1 GB → 24,8 GB libres», con el formato en español. */
  text: (measure: Measure) => invoke<string>("measure_text", { measure }),
  /** Apunta una medida en el diario (y con ello en el caso y el informe). */
  record: (title: string, measure: Measure) => invoke<void>("measure_record", { title, measure }),
};

// ---------- El plan que sigue solo ----------

/** Lo que hay que comprobar al volver del reinicio. */
export interface PlanWait {
  kind: "rename" | "domain" | "reboot";
  expect: string;
  name: string;
}

export type PlanStatus = "pending" | "running" | "ok" | "error" | "skipped";

/** El plan guardado en disco. `recipe` es la plantilla tal cual. */
export interface PlanState {
  recipe: unknown;
  statuses: PlanStatus[];
  messages: string[];
  stopOnError: boolean;
  /** El paso que pidió reiniciar. */
  waitingStep: number;
  wait: PlanWait;
  hostBefore: string;
  bootTime: number;
  savedAt: number;
}

export interface PlanLoad {
  state: PlanState;
  /** AdminOps se abrió con --seguir-plan (lo lanzó el reinicio). */
  fromArgs: boolean;
  /** El equipo arrancó de nuevo desde que se guardó el plan. */
  rebooted: boolean;
}

export interface PlanCheck {
  ok: boolean;
  message: string;
}

export const planApi = {
  /** Guarda el plan y deja la tarea que abre AdminOps al volver; dice si se pudo crear. */
  save: (state: PlanState) => invoke<boolean>("plan_state_save", { state }),
  load: () => invoke<PlanLoad | null>("plan_state_load"),
  /** Borra el plan guardado y la tarea que lo retomaba. */
  clear: () => invoke<void>("plan_state_clear"),
  check: (state: PlanState) => invoke<PlanCheck>("plan_check", { state }),
  /** Guarda el plan, deja la tarea que abre AdminOps al volver y reinicia en 5 segundos. Dice si la tarea quedó creada. */
  reboot: (state: PlanState) => invoke<boolean>("plan_reboot", { state }),
};
