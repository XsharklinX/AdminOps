import { usePageActive } from "../lib/pageActive";
import { useEffect, useRef, useState } from "react";
import { hwApi, type Sensors } from "../lib/api";

/**
 * Lee temperaturas/ventiladores cada `intervalMs` mientras la ventana está
 * visible. La primera lectura tarda ~1 s (carga LibreHardwareMonitor); las
 * siguientes, unos milisegundos.
 */
export function useSensors(intervalMs = 5000) {
  const active = usePageActive();
  const activeRef = useRef(active);
  activeRef.current = active;
  const [sensors, setSensors] = useState<Sensors | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);

  useEffect(() => {
    const tick = async () => {
      if (busy.current || document.hidden || !activeRef.current) return;
      busy.current = true;
      try {
        setSensors(await hwApi.sensors());
        setError(null);
      } catch (e) {
        setError(String(e));
      } finally {
        busy.current = false;
      }
    };
    void tick();
    const t = window.setInterval(tick, intervalMs);
    return () => window.clearInterval(t);
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
