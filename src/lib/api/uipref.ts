// Preferencias de la interfaz guardadas en un archivo de AdminOps (ver uipref.rs), no en localStorage: así no se pierden
// al pasar de modo técnico a modo usuario (cada modo tiene su propia carpeta de WebView2).
import { invoke } from "./core";

export type UiPrefKey = "noteTemplates" | "kbdUndo";

/**
 * Lo guardado para `key`. Si el archivo no lo tiene pero este localStorage sí (de antes), lo pasa al archivo y lo borra
 * de ahí. `valid` descarta lo que no sirva.
 */
export async function loadUiPref<T>(key: UiPrefKey, legacyKey: string, valid: (v: unknown) => v is T): Promise<T | null> {
  let stored: unknown;
  try {
    stored = await invoke<unknown>("ui_pref_get", { key });
  } catch {
    return null;
  }
  if (valid(stored)) return stored;
  let legacy: unknown = null;
  try {
    const raw = localStorage.getItem(legacyKey);
    if (raw !== null) {
      try {
        legacy = JSON.parse(raw);
      } catch {
        legacy = null;
      }
    }
  } catch {
    return null;
  }
  if (!valid(legacy)) {
    // Lo viejo no vale: se quita para no volver a mirarlo.
    try {
      localStorage.removeItem(legacyKey);
    } catch {
      /* sin almacenamiento */
    }
    return null;
  }
  try {
    await invoke<void>("ui_pref_set", { key, value: legacy });
    localStorage.removeItem(legacyKey);
  } catch {
    /* se queda en localStorage hasta que se pueda pasar */
  }
  return legacy;
}

export function saveUiPref(key: UiPrefKey, value: unknown): Promise<void> {
  return invoke<void>("ui_pref_set", { key, value });
}
