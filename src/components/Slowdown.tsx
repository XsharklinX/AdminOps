import { X } from "lucide-react";
import { useMemo, useState } from "react";
import { logQuietly, toolsApi, type LiveMetrics } from "../lib/api";
import { bytes, loadColor } from "../lib/format";
import { slowdown, type Hog } from "../lib/slowdown";
import { useConfirm, useToast } from "./feedback";
import { Card } from "./ui";

const DOT = { ok: "bg-ok", warn: "bg-warn", bad: "bg-bad" };

/**
 * «Qué frena el equipo ahora»: lo que más pesa en este momento, dicho en claro.
 * Lo que es de Windows se explica; lo que es un programa del usuario se puede cerrar.
 */
export function Slowdown({ metrics, onProcesses }: { metrics: LiveMetrics; onProcesses: () => void }) {
  const s = useMemo(() => slowdown(metrics), [metrics]);
  const [closing, setClosing] = useState<number | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const close = async (h: Hog) => {
    const ok = await confirm({
      title: `¿Cerrar ${h.name}?`,
      body: <p>Se cierra a la fuerza, sin preguntar si hay algo sin guardar. Lo que estuviera a medias en ese programa se pierde.</p>,
      confirmLabel: "Cerrar el programa",
      danger: true,
    });
    if (!ok) return;
    setClosing(h.pid);
    try {
      await toolsApi.killProcess(h.pid, h.name, false);
      toast("ok", `${h.name} cerrado.`);
    } catch (e) {
      toast("error", String(e));
      logQuietly("Slowdown")(e);
    } finally {
      setClosing(null);
    }
  };

  return (
    <Card
      title="Qué frena el equipo ahora"
      className="min-w-0"
      right={
        <button onClick={onProcesses} className="text-xs text-mute transition-colors hover:text-ink">
          Ver procesos
        </button>
      }
    >
      <div className="mb-1 flex items-center gap-2.5">
        <span className={`size-2.5 shrink-0 rounded-full ${DOT[s.level]}`} />
        <span className="text-[15px] font-medium text-ink">{s.headline}</span>
      </div>
      {s.hogs.length > 0 && (
        <div className="grid gap-x-10 lg:grid-cols-2">
          {s.hogs.map((h) => (
            <div key={h.pid} className="flex items-start gap-3 border-b border-line py-2 text-[13px]">
              <div className="min-w-0 flex-1">
                <div className="truncate text-ink">{h.name}</div>
                <div className="text-xs text-mute">{h.about ?? "Un programa abierto por el usuario."}</div>
              </div>
              <span className="w-12 shrink-0 text-right text-dim tabular" style={h.cpu >= 40 ? { color: loadColor(h.cpu + 40) } : undefined}>
                {Math.round(h.cpu)} %
              </span>
              <span className="w-16 shrink-0 text-right text-dim tabular">{bytes(h.memory)}</span>
              <span className="w-16 shrink-0 text-right">
                {h.closable && (
                  <button
                    onClick={() => void close(h)}
                    disabled={closing !== null}
                    className="inline-flex items-center gap-1 rounded-md border border-bad/40 px-1.5 py-0.5 text-[11px] text-bad transition-colors hover:bg-bad/10 disabled:opacity-40"
                  >
                    <X size={11} /> Cerrar
                  </button>
                )}
              </span>
            </div>
          ))}
        </div>
      )}
      {dialog}
    </Card>
  );
}
