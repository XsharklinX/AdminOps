// La cola de instalación: lo que dice cada fila y el resumen del final.
// Lógica pura (sin pantalla) para poder probarla.
import type { InstallItem } from "./api";

export const isFinished = (i: InstallItem) => i.state === "done" || i.state === "skipped" || i.state === "failed";

/** «En cola», «Descargando 64 %», «Instalando…», «Listo»… */
export function statusText(i: InstallItem): string {
  switch (i.state) {
    case "queued":
      return "En cola";
    case "starting":
      return "Preparando…";
    case "downloading":
      return i.percent === null ? "Descargando…" : `Descargando ${i.percent} %`;
    case "installing":
      return "Instalando…";
    case "done":
      return i.reboot ? "Listo · pide reiniciar" : "Listo";
    case "skipped":
      return "Saltado";
    case "failed":
      return "Falló";
  }
}

export function counts(items: InstallItem[]) {
  return {
    done: items.filter((i) => i.state === "done").length,
    skipped: items.filter((i) => i.state === "skipped").length,
    failed: items.filter((i) => i.state === "failed").length,
    waiting: items.filter((i) => !isFinished(i)).length,
  };
}

/** «Instalación terminada · 3 instalados · 1 saltado · 2 fallaron» (solo cuando ya no queda nada en curso). */
export function summaryText(items: InstallItem[]): string | null {
  if (items.length === 0) return null;
  const c = counts(items);
  if (c.waiting > 0) return null;
  const n = (k: number, one: string, many: string) => `${k} ${k === 1 ? one : many}`;
  return `Instalación terminada · ${n(c.done, "instalado", "instalados")} · ${n(c.skipped, "saltado", "saltados")} · ${n(c.failed, "falló", "fallaron")}`;
}

/** Aplica un cambio de fila a la lista (la fila nueva se añade al final). */
export function mergeItem(items: InstallItem[], item: InstallItem): InstallItem[] {
  return items.some((x) => x.id === item.id) ? items.map((x) => (x.id === item.id ? item : x)) : [...items, item];
}

/** Lo que espera una decisión del técnico: fallos (con «Reintentar» y «Saltar»). */
export const needsDecision = (i: InstallItem) => i.state === "failed";

/** «Cerrar y seguir» solo cuando el motivo es que el programa está abierto. */
export const canCloseAndGo = (i: InstallItem) => i.state === "failed" && i.kind === "close-app";

/** La inicial del programa, para el cuadrito de la fila (no hay iconos de programas). */
export function initial(name: string): string {
  const c = name.trim().match(/[\p{L}\p{N}]/u)?.[0];
  return c ? c.toUpperCase() : "?";
}
