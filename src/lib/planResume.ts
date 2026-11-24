// El plan de «Preparar equipos» que sigue solo después de reiniciar: lo que no
// depende de React, para poder probarlo.
import type { PlanLoad, PlanWait, RecipeStep } from "./api";

let requested = false;

/** AdminOps arrancó con un plan pendiente: «Preparar equipos» lo retoma al abrirse (una sola vez). */
export function requestResume(): void {
  requested = true;
}

/** ¿Hay que retomar el plan? Lo consume quien lo pregunta. */
export function takeResumeRequest(): boolean {
  const r = requested;
  requested = false;
  return r;
}

/**
 * ¿Hay que retomar el plan ya? Solo si lo lanzó el reinicio (`--seguir-plan`) o si el equipo
 * arrancó de nuevo desde que se guardó. Si no, el plan queda esperando y se avisa en «Preparar equipos».
 */
export function shouldResume(load: Pick<PlanLoad, "fromArgs" | "rebooted">): boolean {
  return load.fromArgs || load.rebooted;
}

/** Lista de longitud `n`: lo guardado, y `fill` donde falte (la plantilla pudo cambiar de tamaño). */
export function fit<T>(saved: readonly T[] | undefined, n: number, fill: T): T[] {
  return Array.from({ length: n }, (_, i) => saved?.[i] ?? fill);
}

/**
 * ¿Hay que parar a reiniciar después de este paso? «Reiniciar el equipo» siempre;
 * cambiar el nombre o unir al dominio, solo si quedan pasos por hacer (si es el
 * último, no hay nada que seguir y vale el aviso de siempre).
 */
export function pauseNeeded(wait: PlanWait | null, index: number, total: number): boolean {
  if (!wait) return false;
  return wait.kind === "reboot" || index < total - 1;
}

/** Contraseñas que piden los pasos que quedan (no se guardan nunca: al volver se vuelven a pedir). */
export function passwordsNeeded(steps: readonly RecipeStep[], from: number): { user: boolean; domain: boolean } {
  const rest = steps.slice(from);
  return { user: rest.some((s) => s.kind === "user"), domain: rest.some((s) => s.kind === "domain") };
}

/** «Seguido tras reiniciar · paso 3 de 5» (el paso que pidió reiniciar, contando desde 1). */
export function resumeBanner(waitingStep: number, total: number): string {
  return `Seguido tras reiniciar · paso ${waitingStep + 1} de ${total}`;
}

/** Lo que dice el paso que se comprobó al volver. */
export function checkedMessage(message: string): string {
  return `Comprobado tras reiniciar: ${message}`;
}
