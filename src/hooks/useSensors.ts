import { usePageActive } from "../lib/pageActive";
import { useEffect, useRef, useState } from "react";
import { hwApi, type Sensors } from "../lib/api";

/** A partir de aquí una lectura se considera lenta y la siguiente se espacia. */
const SLOW_MS = 1500;
/** Tope de la espera entre lecturas cuando el equipo va justo. */
const MAX_WAIT_MS = 60_000;

/** Cuánto esperar hasta la siguiente lectura, según lo que tardó la última. */
export function nextSensorWait(intervalMs: number, tookMs: number) {
  if (tookMs < SLOW_MS) return intervalMs;
  // Una lectura lenta ocupa la consola compartida: se le da cinco veces su
  // duración de descanso, para que no pase más tiempo leyendo que libre.
  return Math.min(MAX_WAIT_MS, Math.max(intervalMs, Math.round(tookMs * 5)));
}

/**
 * Lee temperaturas/ventiladores cada `intervalMs` mientras la ventana está
 * visible. La primera lectura tarda ~1 s (carga LibreHardwareMonitor); las
 * siguientes, unos milisegundos. Si una tarda mucho (equipo ocupado, arranque
 * desde un pendrive), las siguientes se espacian solas.
 */
export function useSensors(intervalMs = 5000) {
  const active = usePageActive();
  const activeRef = useRef(active);
  activeRef.current = active;
  const [sensors, setSensors] = useState<Sensors | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let stopped = false;
    let timer: number | undefined;
    const tick = async () => {
      let wait = intervalMs;
      if (!document.hidden && activeRef.current) {
        const t0 = performance.now();
        try {
          const s = await hwApi.sensors();
          if (!stopped) {
            setSensors(s);
            setError(null);
          }
        } catch (e) {
          if (!stopped) setError(String(e));
        }
        wait = nextSensorWait(intervalMs, performance.now() - t0);
      }
      if (!stopped) timer = window.setTimeout(tick, wait);
    };
    void tick();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }, [intervalMs]);

  return { sensors, error };
}

/** Color según temperatura: normal, alta (≥75) o crítica (≥88). */
export function tempColor(t: number | null | undefined) {
  if (t == null) return "var(--color-mute)";
  if (t >= 88) return "var(--color-bad)";
  if (t >= 75) return "var(--color-warn)";
  return "var(--color-ok)";
}
