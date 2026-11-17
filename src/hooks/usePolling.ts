// Repetir una lectura cada cierto tiempo, pero solo mientras se mira.
//
// Se para cuando la ventana está minimizada u oculta (`document.hidden`) o la
// pantalla no es la que se ve (`usePageActive`: las páginas visitadas siguen
// montadas), y al volver hace UNA sola lectura para ponerse al día en vez de
// esperar al siguiente turno. Si la lectura anterior no ha terminado, no pide
// otra encima.
import { useEffect, useRef, useState } from "react";
import { usePageActive } from "../lib/pageActive";

export interface PollingOptions {
  /** Leer también al montarse (y al activarse `enabled`), no solo al volver. Por defecto no. */
  immediate?: boolean;
  /** Falso = en pausa (p. ej. el botón «Pausar» de una lista en vivo). Por defecto verdadero. */
  enabled?: boolean;
}

/** ¿Se está viendo la ventana? (no minimizada ni en otro escritorio). */
export function useWindowVisible(): boolean {
  const [shown, setShown] = useState(() => !document.hidden);
  useEffect(() => {
    const f = () => setShown(!document.hidden);
    document.addEventListener("visibilitychange", f);
    return () => document.removeEventListener("visibilitychange", f);
  }, []);
  return shown;
}

export function usePolling(fn: () => unknown, ms: number, { immediate = false, enabled = true }: PollingOptions = {}): void {
  const pageActive = usePageActive();
  const shown = useWindowVisible();
  const on = pageActive && shown && enabled;
  const latest = useRef(fn);
  useEffect(() => {
    latest.current = fn;
  });
  // Evita lecturas solapadas: la ref sobrevive a pausas y reanudaciones.
  const busy = useRef(false);
  // Hubo una pausa desde la última lectura: al volver toca una lectura de repaso.
  const catchUp = useRef(false);
  useEffect(() => {
    if (!on) return;
    const run = () => {
      if (busy.current) return;
      busy.current = true;
      Promise.resolve(latest.current())
        .catch(() => undefined)
        .finally(() => {
          busy.current = false;
        });
    };
    if (immediate || catchUp.current) run();
    catchUp.current = false;
    const t = window.setInterval(run, ms);
    return () => {
      window.clearInterval(t);
      catchUp.current = true;
    };
  }, [on, ms, immediate]);
}
