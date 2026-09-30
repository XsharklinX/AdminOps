import { Loader2, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { useTaskMessage } from "../hooks/useTask";
import { appApi } from "../lib/api";

/** Segundos transcurridos mientras `active` es true. */
export function useElapsed(active: boolean) {
  const [secs, setSecs] = useState(0);
  useEffect(() => {
    if (!active) return;
    setSecs(0);
    const start = Date.now();
    const t = window.setInterval(() => setSecs(Math.floor((Date.now() - start) / 1000)), 1000);
    return () => window.clearInterval(t);
  }, [active]);
  return secs;
}

const mmss = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;

/**
 * Estado de una tarea larga del backend: etapa actual (evento `task-progress`),
 * tiempo transcurrido y botón para cancelarla.
 */
export function TaskStatus({
  task,
  active,
  fallback = "Trabajando…",
  cancellable = true,
  className = "",
}: {
  task: string;
  active: boolean;
  fallback?: string;
  cancellable?: boolean;
  className?: string;
}) {
  const message = useTaskMessage(task, active);
  const elapsed = useElapsed(active);
  const [cancelling, setCancelling] = useState(false);

  useEffect(() => {
    if (!active) setCancelling(false);
  }, [active]);

  if (!active) return null;

  const cancel = async () => {
    setCancelling(true);
    try {
      await appApi.cancelTask(task);
    } catch {
      setCancelling(false);
    }
  };

  return (
    <span className={`flex min-w-0 items-center gap-2 text-xs text-neon ${className}`}>
      <Loader2 size={12} className="shrink-0 animate-spin" />
      <span className="truncate">{cancelling ? "Cancelando…" : (message ?? fallback)}</span>
      {elapsed >= 3 && <span className="shrink-0 font-mono text-mute">{mmss(elapsed)}</span>}
      {cancellable && elapsed >= 2 && !cancelling && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            void cancel();
          }}
          className="flex shrink-0 items-center gap-1 rounded border border-bad/40 px-1.5 py-px text-[11px] text-bad transition-colors hover:bg-bad/10"
          title="Detener la operación. Lo que ya se hubiera cambiado se deshace."
        >
          <Square size={8} fill="currentColor" /> Cancelar
        </button>
      )}
    </span>
  );
}
