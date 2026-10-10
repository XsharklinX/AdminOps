// Borradores que no se pierden: lo escrito en las notas de un informe o en el
// caso se guarda mientras se escribe y vuelve al reabrir AdminOps. Se borra al
// usarlo (informe generado, caso cerrado).
import { useCallback, useEffect, useRef, useState } from "react";

const PREFIX = "adminops.draft.";

export function readDraft(key: string): string | null {
  try {
    return localStorage.getItem(PREFIX + key);
  } catch {
    return null;
  }
}

export function writeDraft(key: string, value: string) {
  try {
    if (value.trim()) localStorage.setItem(PREFIX + key, value);
    else localStorage.removeItem(PREFIX + key);
  } catch {
    /* sin almacenamiento: dura mientras esté abierta */
  }
}

/**
 * Como useState, pero lo escrito sobrevive a cerrar AdminOps. `forget` borra el
 * borrador guardado y deja el texto en pantalla (para volver a usarlo ahora,
 * sin que aparezca en la próxima visita).
 */
export function useDraft(key: string, initial = ""): [string, (v: string) => void, () => void] {
  const [value, setValue] = useState(() => readDraft(key) ?? initial);
  const timer = useRef(0);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const set = useCallback(
    (v: string) => {
      setValue(v);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => writeDraft(key, v), 400);
    },
    [key],
  );
  const forget = useCallback(() => {
    window.clearTimeout(timer.current);
    writeDraft(key, "");
  }, [key]);
  return [value, set, forget];
}
