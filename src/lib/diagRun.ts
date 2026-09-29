// Último diagnóstico y el que esté en curso, compartidos entre la página de
// Diagnóstico y el arranque de la app («diagnosticar al abrir»). Vive aparte
// para no cargar la página entera solo por lanzarlo.
import { diagApi, type Diagnostics } from "./api";

let cached: Diagnostics | null = null;
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

export const analyzing = () => inflight !== null;
