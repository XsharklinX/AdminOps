// Último diagnóstico y el que esté en curso, compartidos entre la página de
// Diagnóstico y el arranque de la app («diagnosticar al abrir»). Vive aparte
// para no cargar la página entera solo por lanzarlo.
import { listen } from "@tauri-apps/api/event";
import { diagApi, type Diagnostics } from "./api";

let cached: Diagnostics | null = null;

/** El análisis guardado se completó después (llegaron las actualizaciones de programas). */
export const DIAG_UPDATED = "adminops:diagnostics-updated";

// El programa avisa cuando añade a un análisis ya hecho lo que aún se estaba
// buscando: se sustituye el que se tiene, si es ese mismo.
void listen<Diagnostics>("diagnostics-updated", ({ payload }) => {
  if (cached && cached.timestamp !== payload.timestamp) return;
  cached = payload;
  window.dispatchEvent(new CustomEvent(DIAG_UPDATED, { detail: payload }));
}).catch(() => {});
let inflight: Promise<Diagnostics> | null = null;

export const lastDiagnostics = () => cached;

/** Una ejecución en curso se comparte (StrictMode monta los efectos dos veces). */
export function analyze(force = false): Promise<Diagnostics> {
  inflight ??= diagApi
    .run(force)
    .then((d) => (cached = d))
    .finally(() => (inflight = null));
  return inflight;
}

/** Cambió lo que cuenta del diagnóstico (un hallazgo aceptado o vuelto a avisar). */
export const DIAG_COUNT_CHANGED = "adminops:diagnostics-count";
export const notifyDiagCount = () => window.dispatchEvent(new Event(DIAG_COUNT_CHANGED));

/** Una primera mirada en segundos. No sustituye al último análisis completo. */
export const analyzeQuick = (): Promise<Diagnostics> => diagApi.run(false, true);

export const analyzing = () => inflight !== null;
