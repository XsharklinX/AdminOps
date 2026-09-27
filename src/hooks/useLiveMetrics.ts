import { usePageActive } from "../lib/pageActive";
import { useEffect, useRef, useState } from "react";
import { api, type LiveMetrics } from "../lib/api";

const HISTORY = 60;

export interface MetricsHistory {
  cpu: number[];
  ram: number[];
  rx: number[];
  tx: number[];
}

const push = (arr: number[], v: number) => [...arr.slice(-(HISTORY - 1)), v];

/**
 * Pide métricas al backend cada `intervalMs`. Se pausa cuando la ventana está
 * oculta o minimizada para no gastar CPU en una app que pretende ahorrarla.
 */
export function useLiveMetrics(intervalMs = 1500) {
  const active = usePageActive();
  const activeRef = useRef(active);
  activeRef.current = active;
  const [metrics, setMetrics] = useState<LiveMetrics | null>(null);
  const [history, setHistory] = useState<MetricsHistory>({ cpu: [], ram: [], rx: [], tx: [] });
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);

  useEffect(() => {
    let timer: number | undefined;

    const tick = async () => {
      if (busy.current || document.hidden || !activeRef.current) return;
      busy.current = true;
      try {
        const m = await api.liveMetrics();
        setMetrics(m);
        setHistory((h) => ({
          cpu: push(h.cpu, m.cpuTotal),
          ram: push(h.ram, (m.memoryUsed / m.memoryTotal) * 100),
          rx: push(h.rx, m.netRxPerSec),
          tx: push(h.tx, m.netTxPerSec),
        }));
        setError(null);
      } catch (e) {
        setError(String(e));
      } finally {
        busy.current = false;
      }
    };

    tick();
    timer = window.setInterval(tick, intervalMs);
    document.addEventListener("visibilitychange", tick);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [intervalMs]);

  return { metrics, history, error };
}
