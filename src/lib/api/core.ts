import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { trackCall } from "../perf";
import { humanError, isUnexpected, onInternalError } from "../errors";
import { getPrefs } from "../prefs";

/** Todas las llamadas al sistema pasan por aquí: los errores llegan ya traducidos. */
export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  // Modo demostración: datos de ejemplo y nada se guarda en los del técnico.
  if (getPrefs().demo) {
    const demo = await import("../demo");
    if (demo.demoBlocks(command)) throw demo.DEMO_BLOCKED;
    const answer = demo.demoAnswer(command);
    if (answer !== undefined) return answer as T;
  }
  try {
    return await trackCall(command, tauriInvoke<T>(command, args));
  } catch (e) {
    const message = humanError(command, e);
    // Un fallo inesperado de una orden (no «requiere administrador» ni «sin conexión»): al registro de errores de la app.
    if (isUnexpected(e)) void tauriInvoke("error_log_record", { source: "orden", place: command, message: String(typeof e === "string" ? e : e instanceof Error ? e.message : e) }).catch(() => {});
    throw message;
  }
}

// Los fallos internos se anotan en el registro técnico (sin pasar por `invoke`, para no repetirse).
onInternalError((message) => void tauriInvoke("log_frontend_error", { message }).catch(() => {}));

/**
 * Para lecturas de fondo cuyo fallo no merece un aviso (rellenar un desplegable,
 * una cifra secundaria): no se interrumpe al técnico, pero queda anotado en el
 * registro técnico, que va en el paquete de soporte. Antes se perdían.
 */
export const logQuietly = (where: string) => (e: unknown) => {
  void tauriInvoke("log_frontend_error", { message: `${where}: ${String(e)}` }).catch(() => {});
};
