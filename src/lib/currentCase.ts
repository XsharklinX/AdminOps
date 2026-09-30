// El caso de ahora, visto desde cualquier sitio de la interfaz.
//
// La barra del caso vive en App; desde la ficha de una persona, Ctrl+K o un
// botón de la cabecera se le pide que abra uno nuevo con estos avisos, sin
// tener que pasar funciones de mano en mano por medio árbol de componentes.
import type { Case } from "./api";

/** Pide abrir el diálogo de «Nuevo caso», con lo que ya se sepa rellenado. */
export const OPEN_CASE_EVENT = "adminops:open-case";
/** El caso abierto cambió (se abrió, se editó o se cerró): hay que releerlo. */
export const CASE_CHANGED_EVENT = "adminops:case-changed";

export function openCase(prefill: Partial<Case> = {}) {
  window.dispatchEvent(new CustomEvent<Partial<Case>>(OPEN_CASE_EVENT, { detail: prefill }));
}

export function caseChanged() {
  window.dispatchEvent(new Event(CASE_CHANGED_EVENT));
}

/** «14 min», «1 h 05 min»: lo que lleva abierto un caso. */
export function elapsed(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  if (h > 0) return `${h} h ${String(m).padStart(2, "0")} min`;
  return `${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}`;
}
